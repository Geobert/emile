use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result};
use notify::RecursiveMode;
use notify_debouncer_mini::DebouncedEvent;
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

        let scheduled: BTreeMap<OffsetDateTime, Vec<PathBuf>> = BTreeMap::new();
        let index = BTreeMap::new();

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

    debug!("starting watcher");
    // let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())
    //     .with_context(|| "Failed to create watcher")?;
    let mut debouncer = notify_debouncer_mini::new_debouncer(Duration::from_secs(2), None, tx)
        .with_context(|| "Failed to create watcher")?;
    let watcher = debouncer.watcher();

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
            Ok(evts) => {
                for evt in evts {
                    process_evt(evt, s.clone(), &cfg_abs, &cfg, &tx_scheduler).await;
                }
            }
            Err(err) => error!("watch error: {:?}", err),
        }
    }
    Ok(())
}

async fn process_evt(
    evt: DebouncedEvent,
    s: Arc<SiteWatcher>,
    cfg_abs: &SiteConfig, // config with directory as absolute Path
    cfg: &SiteConfig,
    tx_scheduler: &UnboundedSender<SchedulerEvent>,
) {
    let path = &evt.path;
    // ignore directory changes
    if path.is_dir() {
        return;
    }

    debug!("path: {:?}", &path);
    if path.starts_with(&cfg_abs.schedule_dir) {
        process_schedule_evt(path, s.clone(), &cfg);
        if let Err(e) = tx_scheduler.send(SchedulerEvent::Changed) {
            error!("Error sending ScheduleEvent: {:?}", e)
        }
    } else if path.starts_with(&cfg_abs.drafts_creation_dir) {
        // nothing to do
    } else {
        match zola_build() {
            Ok(_) => info!("Build success after filesystem event ({:?})", evt),
            Err(err) => error!(
                "Failed building after filesystem event `{:?}`: {}",
                evt, err
            ),
        }
    }
}

fn process_schedule_evt(path: &Path, s: Arc<SiteWatcher>, cfg: &SiteConfig) {
    info!("Schedule directory changed");
    match path.exists() {
        true => match extract_date(path, cfg) {
            Ok(date) => {
                let now = OffsetDateTime::now_utc();
                if date <= now {
                    info!("Post scheduled in the past, publish now");
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
                    info!("Post in the future, schedule");
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
        false => {
            info!("Unschedule {:?}", path);
            match (s.index.lock(), s.scheduled.lock()) {
                (Ok(mut index), Ok(mut scheduled)) => {
                    if let Some(date) = index.remove(path) {
                        scheduled
                            .entry(date)
                            .and_modify(|v| v.retain(|p| p.as_path() != path));
                    }
                }
                _ => {
                    error!("Error getting lock on SiteWatcher")
                }
            }
        }
    }
}
