use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, FixedOffset, Utc};
use lazy_static::lazy_static;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::{debug, error, info, warn};

use crate::{
    config::SiteConfig,
    format_date,
    post::modify_front,
    publish::{does_same_title_exist, publish_post},
    watcher::{SchedulerEvent, SiteWatcher},
};

struct Scheduled {
    // here, Option is used as a cell for a type that have no Default impl, so we can use `take()`
    cancel_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Scheduled {
    pub async fn new(
        watcher: Arc<SiteWatcher>,
        cfg: &SiteConfig,
        tx_scheduler: UnboundedSender<SchedulerEvent>,
    ) -> Result<Self, ()> {
        if let Some(cancel_tx) = schedule_next(watcher, cfg, tx_scheduler).await {
            Ok(Self {
                cancel_tx: Some(cancel_tx),
            })
        } else {
            Err(())
        }
    }
}

impl Drop for Scheduled {
    fn drop(&mut self) {
        if let Some(cancel_tx) = self.cancel_tx.take() {
            if !cancel_tx.is_closed() {
                if let Err(e) = cancel_tx.send(()) {
                    error!("Error on cancelling schedule: {:?}", e)
                }
            }
        }
    }
}

lazy_static! {
    static ref SCHEDULED: Arc<Mutex<Option<Scheduled>>> = Arc::new(Mutex::new(None));
}

pub fn schedule_post(date: &DateTime<FixedOffset>, post: &Path, cfg: &SiteConfig) -> Result<()> {
    if !post
        .canonicalize()
        .with_context(|| format!("canonicalize() of `{}` failed", post.to_string_lossy()))?
        .starts_with(cfg.drafts_creation_dir.canonicalize().with_context(|| {
            format!(
                "canonicalize() of `{}` failed",
                cfg.drafts_creation_dir.to_string_lossy()
            )
        })?)
    {
        bail!(
            "Post must be in {}",
            cfg.drafts_creation_dir.to_string_lossy()
        );
    }

    if !post
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase() == "md")
        .unwrap_or(false)
        || !post.is_file()
    {
        bail!("Post must be a markdown file with `md` extensions");
    }

    if !post.exists() {
        bail!("Post `{}` not found", post.to_string_lossy());
    }

    let content = modify_front(post, |cur_line: &str| {
        let modified = if cur_line.starts_with("date = ") {
            // modify date
            format!("date = {}\n", format_date(&date))
        } else {
            // don’t modify
            format!("{cur_line}\n")
        };
        Ok(modified)
    })?;

    let filename = post.file_name().expect("Post must be a file");
    let dest = cfg.schedule_dir.join(filename);
    if dest.exists() {
        bail!("file {} already exists.", dest.to_string_lossy());
    }

    if let Some(similar_file) =
        does_same_title_exist(&filename.to_string_lossy(), &cfg.publish_dest)?
    {
        bail!(
            "Warning: a post with a the same title exists: `{}`",
            similar_file.file_name().to_string_lossy()
        );
    }

    std::fs::write(&dest, &content)?;
    std::fs::remove_file(&post)?;
    println!(
        "Moved `{}` to scheduled folder with date {}",
        filename.to_string_lossy(),
        format_date(&date)
    );
    Ok(())
}

async fn schedule_next(
    watcher: Arc<SiteWatcher>,
    cfg: &SiteConfig,
    tx_scheduler: UnboundedSender<SchedulerEvent>,
) -> Option<tokio::sync::oneshot::Sender<()>> {
    parse_scheduled(watcher.clone(), cfg, tx_scheduler)
        .await
        .map(
            |res| match (watcher.scheduled.lock(), watcher.index.lock()) {
                (Ok(mut scheduled), Ok(mut index)) => {
                    let date_to_remove = res.date_to_remove;
                    let path_to_remove = res.path_to_remove;
                    for d in date_to_remove {
                        scheduled.remove(&d);
                    }

                    for p in path_to_remove {
                        index.remove(&p);
                    }
                    res.tx
                }
                _ => res.tx,
            },
        )
}

struct ParseResult {
    tx: tokio::sync::oneshot::Sender<()>,
    date_to_remove: Vec<DateTime<Utc>>,
    path_to_remove: Vec<PathBuf>,
}

async fn parse_scheduled(
    watcher: Arc<SiteWatcher>,
    cfg: &SiteConfig,
    tx_scheduler: UnboundedSender<SchedulerEvent>,
) -> Option<ParseResult> {
    let mut date_to_remove = Vec::new();
    let mut path_to_remove = Vec::new();
    let mut path_to_publish = Vec::new();
    let mut res = None;

    match watcher.scheduled.lock() {
        Ok(scheduled) => {
            let now = Utc::now();
            for (date, paths) in scheduled.iter() {
                let date = *date;
                if date <= now {
                    info!("Post(s) scheduled in the past, publish now");
                    date_to_remove.push(date);
                    for path in paths {
                        path_to_remove.push((*path).clone());
                        path_to_publish.push((*path).clone());
                    }
                } else {
                    let (tx, rx) = tokio::sync::oneshot::channel();

                    let duration = date - now;
                    let duration = std::time::Duration::from_secs(duration.num_seconds() as u64);
                    info!(
                        "Did a new schedule, duration until next publication: {}s ({})",
                        duration.as_secs(),
                        date
                    );
                    tokio::spawn(async move {
                        if tokio::time::timeout(duration, rx).await.is_err() {
                            debug!("Schedule due for date: {}", date);
                            let _ = tx_scheduler.send(SchedulerEvent::Scheduled(date));
                        }
                    });

                    res = Some(ParseResult {
                        tx,
                        date_to_remove,
                        path_to_remove,
                    });
                    break;
                }
            }
        }
        Err(err) => error!("Error getting lock on SiteWatcher: {:?}", err),
    }

    for path in &path_to_publish {
        let path = &cfg.schedule_dir.join(path);
        match publish_post(path, cfg).await {
            Ok(dest) => {
                info!("Scheduled post published: {}", dest)
            }
            Err(err) => error!("Error while publishing: {}", err),
        }
    }
    res
}

pub async fn start_scheduler(
    watcher: Arc<SiteWatcher>,
    cfg: Arc<SiteConfig>,
    tx_scheduler: UnboundedSender<SchedulerEvent>,
    mut rx_scheduler: UnboundedReceiver<SchedulerEvent>,
) {
    while let Some(e) = rx_scheduler.recv().await {
        match e {
            SchedulerEvent::Changed => {
                if let Ok(new_scheduled) =
                    Scheduled::new(watcher.clone(), &cfg, tx_scheduler.clone()).await
                {
                    match SCHEDULED.lock() {
                        Ok(mut locked_scheduled) => {
                            locked_scheduled.replace(new_scheduled);
                        }
                        Err(e) => error!("Failed to get lock on SCHEDULED: {:?}", e),
                    }
                }
            }
            SchedulerEvent::Scheduled(date) => {
                match SCHEDULED.lock() {
                    Ok(mut locked_scheduled) => {
                        locked_scheduled.take();
                    }
                    Err(e) => error!("Failed to get lock on SCHEDULED: {:?}", e),
                }
                {
                    let mut paths_to_publish = Vec::new();
                    match (watcher.scheduled.lock(), watcher.index.lock()) {
                        (Ok(mut scheduled), Ok(mut index)) => match scheduled.remove(&date) {
                            Some(paths) => {
                                for path in &paths {
                                    index.remove(path);
                                    paths_to_publish.push(path.clone());
                                }
                            }
                            None => {
                                warn!("Something was scheduled at this date, but no paths found")
                            }
                        },
                        _ => {
                            error!("Error getting lock on SiteWatcher")
                        }
                    }

                    for path in &paths_to_publish {
                        let path = &cfg.schedule_dir.join(path);
                        match publish_post(path, &cfg).await {
                            Ok(dest) => {
                                info!("Scheduled post published: {}", dest);
                            }
                            Err(err) => error!("Error while publishing: {}", err),
                        }
                    }
                }
            }
        }
    }
}
