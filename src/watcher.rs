use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{bail, Result};
use time::OffsetDateTime;

use crate::config::Config;

pub struct Watcher {
    scheduled: HashMap<OffsetDateTime, PathBuf>,
}

impl Watcher {
    pub fn new(site: &Path, cfg: &Config) -> Result<()> {
        std::env::set_current_dir(site)?;
        let sched_dir = &cfg.schedule_dir;
        for entry in std::fs::read_dir(sched_dir)? {
            let path = entry?.path();
        }
        todo!()
    }
}
