use std::fmt::Display;
use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use chrono::{FixedOffset, NaiveTime};
use serde_derive::Deserialize;

#[derive(Debug)]
pub struct SiteConfig {
    // drafts created with `new` command will end here. Path relative to root of the blog.
    pub drafts_creation_dir: PathBuf,
    // on `new`, emile will add this amount of year to the drafts to make it top of the list
    pub drafts_year_shift: i32,
    // emile will take this file to create a draft post by adding `title`, `date` and `draft = true` in the frontmatter
    pub draft_template: String,
    // Destination for `publish` command.
    pub publish_dest: PathBuf,
    // Schedule directory
    pub schedule_dir: PathBuf,
    // timezone in which the posts are dated, relative to UTC
    pub timezone: FixedOffset,
    // how long (in seconds) to wait for end of filesystem event
    pub debouncing: u64,
    // time to use if no time given in schedule command
    pub default_sch_time: NaiveTime,
    // social media configuration
    pub social: Option<SocialCfg>,
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
pub enum SocialApi {
    #[serde(alias = "mastodon")]
    Mastodon,
    #[serde(alias = "bluesky")]
    Bluesky,
}

impl Display for SocialApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SocialApi::Mastodon => write!(f, "Mastodon"),
            SocialApi::Bluesky => write!(f, "Bluesky"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SocialCfg {
    // path to the template to use for posting on mastodon
    pub social_template: PathBuf,
    // default language
    pub default_lang: String,
    // base url
    pub base_url: String,
    // tag <-> language
    pub tag_lang: Option<Vec<TagLang>>,
    // tags to not put in the toot
    pub filtered_tag: Vec<String>,
    // path to the template for the link to the social post
    pub link_template: PathBuf,
    // tag to replace with expanded link_temolate
    pub link_tag: String,
    // social server to post to
    pub instances: Vec<SocialInstance>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SocialInstance {
    // host of the server to post to
    pub server: String,
    // which social network API to use
    pub api: SocialApi,
    // env var to read access token from
    pub token_var: String,
    // env var to read user’s id from
    pub handle_var: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TagLang {
    pub tag: String,
    pub lang: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SocialCfgBuilder {
    // template to use for posting on mastodon
    pub social_template: Option<PathBuf>,
    // tag <-> language
    pub tag_lang: Option<Vec<TagLang>>,
    // tags to not put in the toot
    pub filtered_tag: Vec<String>,
    // path to the template for the link to the social post
    pub link_template: Option<PathBuf>,
    // tag to replace with expanded link_temolate
    pub link_tag: Option<String>,
    // social server to post to
    pub instances: Vec<SocialInstance>,
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
    pub timezone: Option<i32>,
    // how long (in seconds) to wait for end of filesystem event (10s by default)
    pub debouncing: Option<u64>,
    // time to use if no time given in schedule command
    pub default_sch_time: Option<NaiveTime>,
    // social media configuration
    pub social: Option<SocialCfgBuilder>,
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

        let social = cfg_builder.social.map(|cfg_builder| SocialCfg {
            social_template: cfg_builder
                .social_template
                .unwrap_or_else(|| PathBuf::from("social.txt")),
            default_lang,
            base_url,
            tag_lang: cfg_builder.tag_lang,
            filtered_tag: cfg_builder.filtered_tag,
            link_template: cfg_builder
                .link_template
                .unwrap_or_else(|| PathBuf::from("social_link.txt")),
            link_tag: cfg_builder
                .link_tag
                .unwrap_or("{$ emile_social $}".to_owned()),
            instances: cfg_builder.instances,
        });

        if let Some(social) = &social {
            if social.instances.is_empty() {
                bail!("No social servers defined.")
            }
        }

        let config = SiteConfig {
            drafts_creation_dir: cfg_builder
                .drafts_creation_dir
                .unwrap_or_else(|| PathBuf::from("content/drafts")),
            drafts_year_shift: cfg_builder.drafts_year_shift.unwrap_or(0),
            draft_template: cfg_builder
                .draft_template
                .unwrap_or_else(|| "draft.txt".to_string()),
            publish_dest: cfg_builder
                .publish_dest
                .unwrap_or_else(|| PathBuf::from("content/posts")),
            schedule_dir: cfg_builder
                .schedule_dir
                .unwrap_or_else(|| PathBuf::from("content/drafts/scheduled")),
            timezone: cfg_builder
                .timezone
                .map(|t| {
                    FixedOffset::east_opt(t * 3600)
                        .unwrap_or_else(|| panic!("Error constructing FixedOffset with {t}"))
                })
                .unwrap_or(FixedOffset::east_opt(0).unwrap()),
            debouncing: cfg_builder.debouncing.unwrap_or(2),
            default_sch_time: cfg_builder
                .default_sch_time
                .unwrap_or_else(|| NaiveTime::from_hms_opt(12, 0, 0).unwrap()),
            social,
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
            timezone: FixedOffset::east_opt(0).unwrap(),
            debouncing: 2,
            default_sch_time: NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            social: None,
        }
    }
}
