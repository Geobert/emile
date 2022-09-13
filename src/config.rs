use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    // drafts created with `new` command will end here. Path relative to root of the blog.
    pub drafts_creation_dir: PathBuf,
    // drafts published with `publish` command will be picked up from here. Path relative to root of the blog.
    pub drafts_consumption_dir: PathBuf,
    // emile will add this amount of year to the drafts to make it top of the list
    pub drafts_year_shift: i32,
    // emile will take this file to create a draft post by adding `title`, `date` and `draft = true` in the frontmatter
    pub draft_template: String,
    // Destination for `publish` command.
    pub publish_dest: PathBuf,
    // Schedule directory
    pub schedule_dir: PathBuf,
}

impl Config {}

#[derive(Debug, Deserialize)]
pub struct ConfigBuilder {
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
    // Schedule directory
    pub schedule_dir: Option<PathBuf>,
}

impl ConfigBuilder {
    // to be run from the website's directory
    pub fn get_config() -> Config {
        let cfg = ConfigBuilder::from_file("./emile.toml");
        if cfg.is_err() {
            eprintln!("Warning: failed to load `emile.toml`, fallback to default values");
        }
        cfg.unwrap_or_default()
    }

    fn from_file<P: AsRef<Path>>(path: P) -> Result<Config> {
        let mut file = File::open(&path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        ConfigBuilder::parse(&content)
    }

    fn parse(s: &str) -> Result<Config> {
        let cfg_builder: ConfigBuilder = toml::from_str(s)?;
        let config = Config {
            drafts_creation_dir: cfg_builder
                .drafts_creation_dir
                .unwrap_or_else(|| PathBuf::from("content/drafts")),
            drafts_consumption_dir: cfg_builder
                .drafts_consumption_dir
                .unwrap_or_else(|| PathBuf::from("content/drafts")),
            drafts_year_shift: cfg_builder.drafts_year_shift.unwrap_or(0),
            draft_template: cfg_builder
                .draft_template
                .unwrap_or_else(|| "draft.html".to_string()),
            publish_dest: cfg_builder
                .publish_dest
                .unwrap_or_else(|| PathBuf::from("content/posts")),
            schedule_dir: cfg_builder
                .schedule_dir
                .unwrap_or_else(|| PathBuf::from("content/drafts/scheduled")),
        };

        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            drafts_creation_dir: PathBuf::from("content/drafts"),
            drafts_consumption_dir: PathBuf::from("content/drafts"),
            drafts_year_shift: 0,
            draft_template: "draft.html".to_string(),
            publish_dest: PathBuf::from("content/posts"),
            schedule_dir: PathBuf::from("content/drafts/schedule"),
        }
    }
}
