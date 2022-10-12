use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use lazy_static::lazy_static;
use time::OffsetDateTime;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::{debug, error, info, trace, warn};

use crate::{
    config::SiteConfig,
    publish::publish_post,
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
        match self.cancel_tx.take() {
            Some(cancel_tx) => {
                if !cancel_tx.is_closed() {
                    if let Err(e) = cancel_tx.send(()) {
                        error!("Error on cancelling schedule: {:?}", e)
                    }
                }
            }
            None => (),
        }
    }
}

lazy_static! {
    static ref SCHEDULED: Arc<Mutex<Option<Scheduled>>> = Arc::new(Mutex::new(None));
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
    date_to_remove: Vec<OffsetDateTime>,
    path_to_remove: Vec<PathBuf>,
}

async fn parse_scheduled(
    watcher: Arc<SiteWatcher>,
    cfg: &SiteConfig,
    tx_scheduler: UnboundedSender<SchedulerEvent>,
) -> Option<ParseResult> {
    let mut date_to_remove = Vec::new();
    let mut path_to_remove = Vec::new();
    match watcher.scheduled.lock() {
        Ok(scheduled) => {
            let now = OffsetDateTime::now_utc();
            for (date, paths) in scheduled.iter() {
                let date = (*date).clone();
                if date <= now {
                    info!("Post(s) scheduled in the past, publish now");
                    date_to_remove.push(date);
                    for path in paths {
                        path_to_remove.push((*path).clone());

                        match publish_post(
                            &path
                                .file_stem()
                                .expect("Should have filename")
                                .to_string_lossy(),
                            &cfg.schedule_dir,
                            &cfg,
                        ) {
                            Ok(dest) => {
                                info!("Scheduled post published: {}", dest)
                            }
                            Err(err) => error!("Error while publishing: {}", err),
                        }
                    }
                } else {
                    let (tx, rx) = tokio::sync::oneshot::channel();

                    let duration = date - now;
                    let duration = std::time::Duration::from_secs(duration.whole_seconds() as u64);
                    info!(
                        "Did a new schedule, duration until next publication: {}s ({})",
                        duration.as_secs(),
                        date
                    );
                    tokio::spawn(async move {
                        if let Err(_) = tokio::time::timeout(duration, rx).await {
                            debug!("Schedule due for date: {}", date);
                            let _ = tx_scheduler.send(SchedulerEvent::Scheduled(date));
                        }
                    });

                    return Some(ParseResult {
                        tx,
                        date_to_remove,
                        path_to_remove,
                    });
                }
            }
        }
        Err(err) => error!("Error getting lock on SiteWatcher: {:?}", err),
    }
    return None;
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
                    match (watcher.scheduled.lock(), watcher.index.lock()) {
                        (Ok(mut scheduled), Ok(mut index)) => match scheduled.remove(&date) {
                            Some(paths) => {
                                for path in paths {
                                    index.remove(&path);
                                    match publish_post(
                                        &path
                                            .file_stem()
                                            .expect("Should have filename")
                                            .to_string_lossy(),
                                        &cfg.schedule_dir,
                                        &cfg,
                                    ) {
                                        Ok(dest) => {
                                            info!("Scheduled post published: {}", dest);
                                        }
                                        Err(err) => error!("Error while publishing: {}", err),
                                    }
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
                }
            }
        }
    }
}
