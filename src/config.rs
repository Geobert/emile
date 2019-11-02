use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde_derive::{Deserialize, Serialize};
use toml;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    // drafts created with `new` command will end here. Path relative to root of the blog.
    pub drafts_creation_dir: Option<PathBuf>,
    // drafts published with `publish` command will be picked up from here. Path relative to root of the blog.
    pub drafts_consumption_dir: Option<PathBuf>,
    // emile will add this amount of year to the drafts to make it top of the list
    pub drafts_year_shift: Option<i32>,
    // emile will take this file to create a draft post by adding `title`, `date` and `draft = true` in the frontmatter
    pub draft_template: Option<String>,
    // Destination for `publish` command.
    pub publish_dest: Option<PathBuf>,
}

impl Config {
    pub fn get_config() -> Self {
        let cfg = Config::from_file("./emile.toml");
        if cfg.is_err() {
            dbg!(&cfg);
            eprintln!("Warning: failed to load `emile.toml`, fallback to default values");
        }
        cfg.unwrap_or_default()
    }

    fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(&path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        Config::parse(&content)
    }

    fn parse(s: &str) -> Result<Self> {
        let mut config: Config = toml::from_str(s)?;
        if config.drafts_creation_dir.is_none() {
            config.drafts_creation_dir = Some(PathBuf::from("content/drafts"));
        } else if !config.drafts_creation_dir.as_ref().unwrap().is_dir() {
            bail!("`drafts_creation_dir` is not a directory.");
        }
        if config.drafts_consumption_dir.is_none() {
            config.drafts_consumption_dir = Some(PathBuf::from("content/drafts"));
        } else if !config.drafts_consumption_dir.as_ref().unwrap().is_dir() {
            bail!("`drafts_consumption_dir` is not a directory.");
        }
        if config.draft_template.is_none() {
            config.draft_template = Some("draft.html".to_string());
        }
        if config.publish_dest.is_none() {
            config.publish_dest = Some(PathBuf::from("content/"));
        }

        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            drafts_creation_dir: Some(PathBuf::from("content/drafts")),
            drafts_consumption_dir: Some(PathBuf::from("content/drafts")),
            drafts_year_shift: None,
            draft_template: Some("draft.html".to_string()),
            publish_dest: None,
        }
    }
}
