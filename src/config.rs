use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_derive::Deserialize;
use time::UtcOffset;

#[derive(Debug)]
pub struct SiteConfig {
    // drafts created with `new` command will end here. Path relative to root of the blog.
    pub drafts_creation_dir: PathBuf,
    // emile will add this amount of year to the drafts to make it top of the list
    pub drafts_year_shift: i32,
    // emile will take this file to create a draft post by adding `title`, `date` and `draft = true` in the frontmatter
    pub draft_template: String,
    // Destination for `publish` command.
    pub publish_dest: PathBuf,
    // Schedule directory
    pub schedule_dir: PathBuf,
    // timezone in which the posts are dated, relative to UTC
    pub timezone: UtcOffset,
    // how long (in seconds) to wait for end of filesystem event
    pub debouncing: u64,
}

impl SiteConfig {}

#[derive(Debug, Deserialize)]
pub struct SiteConfigBuilder {
    // drafts created with `new` command will end here. Path relative to root of the blog.
    pub drafts_creation_dir: Option<PathBuf>,
    // emile will add this amount of year to the drafts to make it top of the list
    pub drafts_year_shift: Option<i32>,
    // emile will take this file to create a draft post by adding `title`, `date` and `draft = true` in the frontmatter
    pub draft_template: Option<String>,
    // Destination for `publish` command.
    pub publish_dest: Option<PathBuf>,
    // Schedule directory
    pub schedule_dir: Option<PathBuf>,
    // timezone in which the posts are dated, relative to UTC
    pub timezone: Option<i8>,
    // how long (in seconds) to wait for end of filesystem event (10s by default)
    pub debouncing: Option<u64>,
}

impl SiteConfigBuilder {
    // to be run from the website's directory
    pub fn get_config() -> SiteConfig {
        let cfg = SiteConfigBuilder::from_file("./emile.toml");
        if let Err(ref err) = cfg {
            eprintln!(
                "Warning: failed to load `emile.toml`, fallback to default values ({})",
                err
            );
        }
        cfg.unwrap_or_default()
    }

    fn from_file<P: AsRef<Path>>(path: P) -> Result<SiteConfig> {
        let mut file = File::open(&path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        SiteConfigBuilder::parse(&content)
    }

    fn parse(s: &str) -> Result<SiteConfig> {
        let cfg_builder: SiteConfigBuilder = toml::from_str(s)?;
        let config = SiteConfig {
            drafts_creation_dir: cfg_builder
                .drafts_creation_dir
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
            timezone: cfg_builder
                .timezone
                .map(|t| {
                    UtcOffset::from_hms(t, 0, 0)
                        .expect(&format!("Error constructing UtcOffset with {}", t))
                })
                .unwrap_or(UtcOffset::UTC),
            debouncing: cfg_builder.debouncing.unwrap_or(2),
        };

        Ok(config)
    }
}

impl Default for SiteConfig {
    fn default() -> Self {
        SiteConfig {
            drafts_creation_dir: PathBuf::from("content/drafts"),
            drafts_year_shift: 0,
            draft_template: "draft.html".to_string(),
            publish_dest: PathBuf::from("content/posts"),
            schedule_dir: PathBuf::from("content/drafts/schedule"),
            timezone: UtcOffset::UTC,
            debouncing: 2,
        }
    }
}
