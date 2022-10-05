use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use lazy_static::lazy_static;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use time::OffsetDateTime;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
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
    index: Mutex<BTreeMap<PathBuf, OffsetDateTime>>,
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

                let date = extract_date(&path)?;
                if date >= now {
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
    cfg: &SiteConfig,
    tx_scheduler: UnboundedSender<SchedulerEvent>,
) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();

    debug!("getting watcher");
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
    let draft_abs_consumption_dir = current_dir.join(&cfg.drafts_consumption_dir);

    let cfg_abs = SiteConfig {
        drafts_creation_dir: draft_abs_creation_dir,
        drafts_consumption_dir: draft_abs_consumption_dir,
        drafts_year_shift: cfg.drafts_year_shift,
        draft_template: cfg.draft_template.clone(),
        publish_dest: cfg.publish_dest.clone(),
        schedule_dir: schedule_abs_dir,
    };

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

async fn process_evt(
    evt: Event,
    s: Arc<SiteWatcher>,
    cfg_abs: &SiteConfig, // config with directory as absolute Path
    cfg: &SiteConfig,
    tx_scheduler: &UnboundedSender<SchedulerEvent>,
) {
    debug!("event: {:?}", evt);

    for path in &evt.paths {
        // ignore directory changes
        if !path.is_file() {
            continue;
        }
        debug!("path: {:?}", &path);
        if path.starts_with(&cfg_abs.schedule_dir) {
            process_schedule_evt(path, &evt, s.clone(), &cfg);
            if let Err(e) = tx_scheduler.send(SchedulerEvent::Changed) {
                error!("Error sending ScheduleEvent: {:?}", e)
            }
        } else if path.starts_with(&cfg_abs.drafts_consumption_dir)
            || path.starts_with(&cfg_abs.drafts_creation_dir)
        {
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
}

fn process_schedule_evt(path: &Path, evt: &Event, s: Arc<SiteWatcher>, cfg: &SiteConfig) {
    info!("Schedule directory changed");
    let now = OffsetDateTime::now_utc();
    match evt.kind {
        EventKind::Modify(_) => match extract_date(path) {
            Ok(date) => {
                if date >= now {
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
                        (Err(err_i), Err(err_s)) => {
                            error!(
                                "Error getting lock on index and scheduled btree: {:#?}, {:#?}",
                                err_i, err_s
                            )
                        }
                        (Err(err_i), _) => {
                            error!("Error getting lock on index btree: {:#?}", err_i)
                        }
                        (_, Err(err_s)) => {
                            error!("Error getting lock on scheduled btree: {:#?}", err_s)
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
                (Err(err_i), Err(err_s)) => {
                    error!(
                        "Error getting lock on index and scheduled btree: {:#?}, {:#?}",
                        err_i, err_s
                    )
                }
                (Err(err_i), _) => {
                    error!("Error getting lock on index btree: {:#?}", err_i)
                }
                (_, Err(err_s)) => {
                    error!("Error getting lock on scheduled btree: {:#?}", err_s)
                }
            }
        }
        _ => {}
    }
}

async fn schedule_next(
    watcher: Arc<SiteWatcher>,
    tx_scheduler: UnboundedSender<SchedulerEvent>,
) -> Option<tokio::sync::oneshot::Sender<()>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    match watcher.scheduled.lock() {
        Ok(scheduled) => {
            if let Some(date) = scheduled.keys().next() {
                let date = (*date).clone();
                let now = OffsetDateTime::now_utc();
                let duration = date - now;
                let duration = std::time::Duration::from_secs(duration.whole_seconds() as u64);
                let instant = tokio::time::Instant::now() + duration;
                tokio::spawn(async move {
                    if let Err(_) = tokio::time::timeout_at(instant, rx).await {
                        tx_scheduler.send(SchedulerEvent::Scheduled(date));
                    }
                });
                return Some(tx);
            }
        }
        Err(err) => error!("Error getting lock on scheduled: {:?}", err),
    }
    return None;
}

struct Scheduled {
    cancel_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Scheduled {
    pub async fn new(
        watcher: Arc<SiteWatcher>,
        tx_scheduler: UnboundedSender<SchedulerEvent>,
    ) -> Self {
        let cancel_tx = schedule_next(watcher, tx_scheduler).await;
        Self { cancel_tx }
    }

    pub async fn reschedule(
        &mut self,
        watcher: Arc<SiteWatcher>,
        tx_scheduler: UnboundedSender<SchedulerEvent>,
    ) {
        match self.cancel_tx.take() {
            Some(cancel_tx) => {
                if let Err(e) = cancel_tx.send(()) {
                    error!("Error on cancelling schedule: {:?}", e)
                }
            }
            None => (),
        }

        match schedule_next(watcher, tx_scheduler).await {
            Some(tx) => {
                self.cancel_tx.replace(tx);
            }
            None => (),
        }
    }
}
lazy_static! {
    static ref SCHEDULED: Arc<Mutex<Option<Scheduled>>> = Arc::new(Mutex::new(None));
}
pub async fn start_scheduler(
    watcher: Arc<SiteWatcher>,
    tx_scheduler: UnboundedSender<SchedulerEvent>,
    mut rx_scheduler: UnboundedReceiver<SchedulerEvent>,
) {
    while let Some(e) = rx_scheduler.recv().await {
        match e {
            SchedulerEvent::Changed => match SCHEDULED.lock() {
                Ok(mut scheduled) => {
                    let option_scheduled = &mut *scheduled;
                    match option_scheduled {
                        Some(scheduled) => {
                            scheduled
                                .reschedule(watcher.clone(), tx_scheduler.clone())
                                .await
                        }
                        _ => {
                            scheduled.replace(
                                Scheduled::new(watcher.clone(), tx_scheduler.clone()).await,
                            );
                        }
                    }
                }
                Err(e) => error!("Failed to get lock on SCHEDULED"),
            },
            SchedulerEvent::Scheduled(date) => todo!(),
        }
    }
}
