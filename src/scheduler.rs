use std::sync::{Arc, Mutex};

use lazy_static::lazy_static;
use time::OffsetDateTime;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::{debug, error, info, warn};

use crate::{
    config::SiteConfig,
    publish::publish_post,
    watcher::{SchedulerEvent, SiteWatcher},
};

struct Scheduled {
    cancel_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Scheduled {
    pub async fn new(
        watcher: Arc<SiteWatcher>,
        tx_scheduler: UnboundedSender<SchedulerEvent>,
    ) -> Option<Self> {
        let cancel_tx = schedule_next(watcher, tx_scheduler).await;
        if cancel_tx.is_some() {
            Some(Self { cancel_tx })
        } else {
            None
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
    tx_scheduler: UnboundedSender<SchedulerEvent>,
) -> Option<tokio::sync::oneshot::Sender<()>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    match watcher.scheduled.lock() {
        Ok(scheduled) => {
            if let Some(date) = scheduled.keys().next() {
                let date = (*date).clone();
                debug!("schedule_next date: {}", date);
                let now = OffsetDateTime::now_utc();
                debug!("now: {}", now);
                let duration = date - now;
                let duration = std::time::Duration::from_secs(duration.whole_seconds() as u64);
                debug!("Duration until next publication: {}s", duration.as_secs());
                tokio::spawn(async move {
                    if let Err(_) = tokio::time::timeout(duration, rx).await {
                        debug!("Scheduled due for date: {}", date);
                        let _ = tx_scheduler.send(SchedulerEvent::Scheduled(date));
                    }
                });
                return Some(tx);
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
                if let Some(new_scheduled) =
                    Scheduled::new(watcher.clone(), tx_scheduler.clone()).await
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
                debug!("Scheduled message for date: {}", date);
                match SCHEDULED.lock() {
                    Ok(mut locked_scheduled) => {
                        debug!("remove Scheduled object");
                        locked_scheduled.take();
                    }
                    Err(e) => error!("Failed to get lock on SCHEDULED: {:?}", e),
                }
                {
                    match (watcher.scheduled.lock(), watcher.index.lock()) {
                        (Ok(mut scheduled), Ok(mut index)) => {
                            debug!("before process scheduled, len: {}", scheduled.len());
                            match scheduled.remove(&date) {
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
                                                debug!("scheduled.len: {}", scheduled.len());
                                            }
                                            Err(err) => error!("Error while publishing: {}", err),
                                        }
                                    }
                                }
                                None => {
                                    warn!(
                                        "Something was scheduled at this date, but no paths found"
                                    )
                                }
                            }
                        }
                        _ => {
                            error!("Error getting lock on SiteWatcher")
                        }
                    }
                }
            }
        }
    }
}
