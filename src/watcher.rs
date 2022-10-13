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

use crate::{config::SiteConfig, post::extract_date, zola_build};

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
        let mut scheduled: BTreeMap<OffsetDateTime, Vec<PathBuf>> = BTreeMap::new();
        let mut index = BTreeMap::new();

        info!(
            "Reading `{}` for scheduled posts",
            sched_dir.to_string_lossy()
        );
        for entry in std::fs::read_dir(sched_dir)? {
            let path = entry?.path();
            if path.is_file() {
                let file_name = path.file_name().expect("file with no name");
                if file_name == "_index.md" {
                    continue;
                }
                let date = extract_date(&path, cfg)
                    .with_context(|| format!("error extracting date from {:?}", file_name))?;
                let file_name = PathBuf::from(file_name);
                scheduled
                    .entry(date)
                    .and_modify(|e| e.push(file_name.clone()))
                    .or_insert_with(|| vec![file_name.clone()]);
                index.insert(file_name, date);
            }
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

    info!("Starting watcher…");
    let mut debouncer =
        notify_debouncer_mini::new_debouncer(Duration::from_secs(cfg.debouncing), None, tx)
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

    info!("Watcher started");
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

    debug!("process_evt path: {:?}", &path);
    if path.starts_with(&cfg_abs.schedule_dir) {
        process_schedule_evt(&path, s.clone(), &cfg);
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
    match path.exists() {
        true => match extract_date(path, cfg) {
            Ok(date) => {
                info!("Process file modification: {:?}", path);
                match (s.index.lock(), s.scheduled.lock()) {
                    (Ok(mut index), Ok(mut scheduled)) => {
                        // search if path already scheduled
                        let file_name =
                            PathBuf::from(path.file_name().expect("Sould have file name"));
                        index
                            .entry(file_name.clone())
                            .and_modify(|old_date| {
                                debug!("path already scheduled, modify it");
                                // already scheduled, modify its date
                                if let Some(sched_at) = scheduled.get_mut(old_date) {
                                    if sched_at.len() > 1 {
                                        sched_at.retain(|p| p.as_path() != path);
                                    }
                                }

                                let mut remove_old_date = false;
                                scheduled.entry(*old_date).and_modify(|v| {
                                    let r = v.retain(|p| p != &file_name);
                                    remove_old_date = v.is_empty();
                                    r
                                });

                                if remove_old_date {
                                    scheduled.remove(old_date);
                                }

                                scheduled
                                    .entry(date)
                                    .and_modify(|v| v.push(file_name.clone()))
                                    .or_insert_with(|| vec![file_name.clone()]);
                                *old_date = date;
                            })
                            .or_insert_with(|| {
                                debug!("path not scheduled, add it");
                                // not already scheduled, add it
                                scheduled
                                    .entry(date)
                                    .and_modify(|v| v.push(file_name.clone()))
                                    .or_insert_with(|| vec![file_name]);
                                date
                            });
                    }
                    _ => {
                        error!("Error getting lock on SiteWatcher")
                    }
                }
                // }
            }
            Err(err) => error!("Error extracting date: {:?}", err),
        },
        false => {
            let file_name = PathBuf::from(path.file_name().expect("Sould have file name"));
            match (s.index.lock(), s.scheduled.lock()) {
                (Ok(mut index), Ok(mut scheduled)) => {
                    if let Some(date) = index.remove(&file_name) {
                        info!("Unschedule {}", path.to_string_lossy());
                        scheduled
                            .entry(date)
                            .and_modify(|v| v.retain(|p| p != &file_name));
                    }
                }
                _ => {
                    error!("Error getting lock on SiteWatcher")
                }
            }
        }
    }
}
