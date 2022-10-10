use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use lazy_static::lazy_static;
use notify::{event::RemoveKind, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use time::OffsetDateTime;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error, info};

use crate::{config::SiteConfig, post::extract_date, publish::publish_post, zola_build};

#[derive(Debug)]
pub enum SchedulerEvent {
    Changed,
    Scheduled(OffsetDateTime),
}

#[derive(Debug)]
pub struct SiteWatcher {
    pub scheduled: Mutex<BTreeMap<OffsetDateTime, Vec<PathBuf>>>,
    pub index: Mutex<BTreeMap<PathBuf, OffsetDateTime>>,
}

impl SiteWatcher {
    pub fn new(cfg: &SiteConfig) -> Result<Self> {
        // read scheduled posts and see if any needs publishing
        let sched_dir = &cfg.schedule_dir;
        let now = OffsetDateTime::now_utc();
        let mut need_publishing = Vec::new();
        let mut scheduled: BTreeMap<OffsetDateTime, Vec<PathBuf>> = BTreeMap::new();
        let mut index = BTreeMap::new();

        debug!(
            "Reading `{}` for scheduled posts",
            sched_dir.to_string_lossy()
        );
        for entry in std::fs::read_dir(sched_dir)? {
            let path = entry?.path();
            if path.is_file() {
                if path.file_name().expect("file with no name") == "_index.md" {
                    continue;
                }
                debug!("Reading {}", path.to_string_lossy());
                let date = extract_date(&path, cfg)?;
                if date <= now {
                    need_publishing.push(path);
                } else {
                    scheduled
                        .entry(date)
                        .and_modify(|e| e.push(path.clone()))
                        .or_insert_with(|| vec![path.clone()]);
                    index.insert(path, date);
                }
            }
        }

        debug!("need_publishing: {}", need_publishing.len());

        for p in need_publishing {
            info!("Publish `{:?}`", p);
            publish_post(
                &p.file_stem()
                    .expect("should have a file name")
                    .to_string_lossy(),
                &cfg.schedule_dir,
                cfg,
            )?;
        }

        Ok(Self {
            scheduled: Mutex::new(scheduled),
            index: Mutex::new(index),
        })
    }
}

pub async fn start_watching(
    s: Arc<SiteWatcher>,
    cfg: Arc<SiteConfig>,
    tx_scheduler: UnboundedSender<SchedulerEvent>,
) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();

    match DEBOUNCER.lock() {
        Ok(mut debouncer) => debouncer.debounce = Duration::from_secs(cfg.debouncing),
        Err(err) => error!("Failed to lock debouncer on `start_watching()`: {}", err),
    }

    debug!("starting watcher");
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())
        .with_context(|| "Failed to create watcher")?;

    let current_dir = std::env::current_dir().with_context(|| "Failed to get current dir")?;

    let dir = current_dir.join("content");
    watcher
        .watch(&dir, RecursiveMode::Recursive)
        .with_context(|| format!("Failed to start watching on `{:?}`", dir))?;
    let dir = current_dir.join("sass");
    watcher
        .watch(&dir, RecursiveMode::Recursive)
        .with_context(|| format!("Failed to start watching on `{:?}`", dir))?;
    let dir = current_dir.join("static");
    watcher
        .watch(&dir, RecursiveMode::Recursive)
        .with_context(|| format!("Failed to start watching on `{:?}`", dir))?;
    let dir = current_dir.join("templates");
    watcher
        .watch(&dir, RecursiveMode::Recursive)
        .with_context(|| format!("Failed to start watching on `{:?}`", dir))?;
    let dir = current_dir.join("themes");
    watcher
        .watch(&dir, RecursiveMode::Recursive)
        .with_context(|| format!("Failed to start watching on `{:?}`", dir))?;

    let schedule_abs_dir = current_dir.join(&cfg.schedule_dir);
    let draft_abs_creation_dir = current_dir.join(&cfg.drafts_creation_dir);

    let cfg_abs = SiteConfig {
        drafts_creation_dir: draft_abs_creation_dir,
        drafts_year_shift: cfg.drafts_year_shift,
        draft_template: cfg.draft_template.clone(),
        publish_dest: cfg.publish_dest.clone(),
        schedule_dir: schedule_abs_dir,
        timezone: cfg.timezone,
        debouncing: cfg.debouncing,
    };

    debug!("watcher started");
    let _ = tx_scheduler.send(SchedulerEvent::Changed);
    for res_evt in rx {
        match res_evt {
            Ok(evt) => {
                process_evt(evt, s.clone(), &cfg_abs, &cfg, &tx_scheduler).await;
            }
            Err(err) => error!("watch error: {:?}", err),
        }
    }
    Ok(())
}

struct EventRecord {
    instant: Instant,
}

#[derive(Default)]
struct Debouncer {
    evts: HashMap<PathBuf, EventRecord>,
    debounce: Duration,
}

impl Debouncer {
    fn need_processing(&mut self, path: &Path) -> bool {
        let mut res = false;
        self.evts
            .entry(path.to_path_buf())
            .and_modify(|e| {
                if e.instant.elapsed() > self.debounce {
                    res = true
                }
                e.instant = std::time::Instant::now();
            })
            .or_insert_with(|| {
                res = true;
                EventRecord {
                    instant: std::time::Instant::now(),
                }
            });

        res
    }

    fn cleanup(&mut self) {
        self.evts
            .retain(|_, r| r.instant.elapsed() < Duration::from_secs(10));
    }
}

lazy_static! {
    static ref DEBOUNCER: Mutex<Debouncer> = Mutex::new(Debouncer::default());
}

async fn process_evt(
    evt: Event,
    s: Arc<SiteWatcher>,
    cfg_abs: &SiteConfig, // config with directory as absolute Path
    cfg: &SiteConfig,
    tx_scheduler: &UnboundedSender<SchedulerEvent>,
) {
    for path in &evt.paths {
        // ignore directory changes, if the file was removed, `Path::is_file()` returns false
        // so we let pass Remove event (directories are unlikely to be removed)
        if !path.is_file() && evt.kind != EventKind::Remove(RemoveKind::Any) {
            continue;
        }

        // debouncing
        let need_process = match DEBOUNCER.lock() {
            Ok(mut debouncer) => debouncer.need_processing(&path),
            Err(_) => {
                error!("Fail to lock debouncer, let's process the event nonetheless");
                true
            }
        };

        if need_process {
            debug!("path: {:?}", &path);
            if path.starts_with(&cfg_abs.schedule_dir) {
                process_schedule_evt(path, &evt, s.clone(), &cfg);
                if let Err(e) = tx_scheduler.send(SchedulerEvent::Changed) {
                    error!("Error sending ScheduleEvent: {:?}", e)
                }
            } else if path.starts_with(&cfg_abs.drafts_creation_dir) {
                // nothing to do
            } else {
                match evt.kind {
                    EventKind::Modify(_) | EventKind::Remove(_) => match zola_build() {
                        Ok(_) => info!("Build success after filesystem event ({:?})", evt),
                        Err(err) => error!(
                            "Failed building after filesystem event `{:?}`: {}",
                            evt, err
                        ),
                    },
                    _ => (),
                }
            }
        }
    }

    match DEBOUNCER.lock() {
        Ok(mut debouncer) => debouncer.cleanup(),
        Err(_) => error!("Error on locking debouncer for cleanup"),
    }
}

fn process_schedule_evt(path: &Path, evt: &Event, s: Arc<SiteWatcher>, cfg: &SiteConfig) {
    info!("Schedule directory changed");
    match evt.kind {
        EventKind::Modify(_) => match extract_date(path, cfg) {
            Ok(date) => {
                if date <= OffsetDateTime::now_utc() {
                    match publish_post(
                        &path
                            .file_stem()
                            .expect("Should have filename")
                            .to_string_lossy(),
                        &cfg.schedule_dir,
                        &cfg,
                    ) {
                        Ok(dest) => {
                            info!("Scheduled post was due publishing: {}", dest)
                        }
                        Err(err) => error!("Error while publishing: {}", err),
                    }
                } else {
                    match (s.index.lock(), s.scheduled.lock()) {
                        (Ok(mut index), Ok(mut scheduled)) => {
                            // search if path already scheduled
                            index
                                .entry(path.to_path_buf())
                                .and_modify(|old_date| {
                                    // already scheduled, modify its date
                                    if let Some(sched_at) = scheduled.get_mut(old_date) {
                                        if sched_at.len() > 1 {
                                            sched_at.retain(|p| p.as_path() != path);
                                        }
                                    }
                                    scheduled
                                        .entry(date)
                                        .and_modify(|v| v.push(path.to_path_buf()))
                                        .or_insert_with(|| vec![path.to_path_buf()]);
                                    *old_date = date;
                                })
                                .or_insert_with(|| {
                                    // not already scheduled, add it
                                    scheduled
                                        .entry(date)
                                        .and_modify(|v| v.push(path.to_path_buf()))
                                        .or_insert_with(|| vec![path.to_path_buf()]);
                                    date
                                });
                        }
                        _ => {
                            error!("Error getting lock on SiteWatcher")
                        }
                    }
                }
            }
            Err(err) => error!("Error extracting date: {:?}", err),
        },
        EventKind::Remove(_) => {
            info!("Unschedule {:?}", path);
            match (s.index.lock(), s.scheduled.lock()) {
                (Ok(mut index), Ok(mut scheduled)) => {
                    if let Some(date) = index.get(path) {
                        scheduled
                            .entry(*date)
                            .and_modify(|v| v.retain(|p| p.as_path() != path));
                        index.remove(path);
                    }
                }
                _ => {
                    error!("Error getting lock on SiteWatcher")
                }
            }
        }
        _ => {}
    }
}
