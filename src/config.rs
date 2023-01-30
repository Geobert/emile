use std::fs::File;
use std::io::{prelude::*, BufReader};
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
    // Mastodon configuration
    pub mastodon: Option<MastodonCfg>,
}

#[derive(Debug, Clone)]
pub struct MastodonCfg {
    // host of the Mastodon server to post to
    pub instance: String,
    // template to use for posting on mastodon
    pub social_template: String,
    // default language
    pub default_lang: String,
    // base url
    pub base_url: String,
    // tag <-> language
    pub tag_lang: Option<Vec<TagLang>>,
    // tags to not put in the toot
    pub filtered_tag: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TagLang {
    pub tag: String,
    pub lang: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MastodonCfgBuilder {
    // host of the Mastodon server to post to
    pub instance: String,
    // template to use for posting on mastodon
    pub social_template: Option<String>,
    // tag <-> language
    pub tag_lang: Option<Vec<TagLang>>,
    // tags to not put in the toot
    pub filtered_tag: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
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
    // mastodon instance host
    pub mastodon: Option<MastodonCfgBuilder>,
}

impl SiteConfigBuilder {
    // to be run from the website's directory
    pub fn get_config() -> SiteConfig {
        let cfg = SiteConfigBuilder::from_file("./emile.toml");
        if let Err(ref err) = cfg {
            eprintln!("Warning: failed to load `emile.toml`, fallback to default values ({err})");
        }
        cfg.unwrap_or_default()
    }

    fn from_file<P: AsRef<Path>>(path: P) -> Result<SiteConfig> {
        let mut file = File::open(&path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        SiteConfigBuilder::parse(&content)
    }

    // Get (default language, base url) from Zola’s config file
    fn get_config_from_zola() -> (String, String) {
        let file = File::open("./config.toml");
        match file {
            Err(ref err) => {
                eprintln!(
                    "Warning: failed to load `config.toml`, fallback to default values ({err})"
                );
                ("en".to_string(), "localhost".to_string())
            }
            Ok(file) => {
                let reader = BufReader::new(&file);

                let mut result_lang = "en".to_string();
                let mut result_base = "localhost".to_string();

                for line in reader.lines() {
                    let line = line.expect("Should have text");
                    let line = line.trim();
                    if line.starts_with("default_language") {
                        let v: Vec<&str> = line.split('=').collect();
                        if let Some(lang) = v.get(1) {
                            result_lang = lang.replace('"', "").trim().to_string();
                        }
                    } else if line.starts_with("base_url") {
                        let v: Vec<&str> = line.split('=').collect();
                        if let Some(base) = v.get(1) {
                            result_base = base.replace('"', "").trim().to_string();
                        }
                    }
                }

                (result_lang, result_base)
            }
        }
    }

    fn parse(s: &str) -> Result<SiteConfig> {
        let cfg_builder: SiteConfigBuilder = toml::from_str(s)?;
        let (default_lang, base_url) = SiteConfigBuilder::get_config_from_zola();
        let mastodon = cfg_builder.mastodon.map(|cfg| MastodonCfg {
            instance: cfg.instance,
            social_template: cfg
                .social_template
                .unwrap_or_else(|| "mastodon.txt".to_string()),
            default_lang,
            base_url,
            tag_lang: cfg.tag_lang,
            filtered_tag: cfg.filtered_tag,
        });

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
                        .unwrap_or_else(|_| panic!("Error constructing UtcOffset with {t}"))
                })
                .unwrap_or(UtcOffset::UTC),
            debouncing: cfg_builder.debouncing.unwrap_or(2),
            mastodon,
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
            mastodon: None,
        }
    }
}
