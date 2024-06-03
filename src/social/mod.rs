use std::{
    collections::HashMap,
    fmt::Display,
    fs::File,
    io::Read,
    ops::Deref,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Result};
use reqwest::Url;
use serde_derive::Deserialize;
use tracing::{error, info};

use crate::{
    config::{SocialApi, SocialCfg},
    social::mastodon::push_to_mastodon,
};

use self::bluesky::push_to_bsky;

mod bluesky;
mod mastodon;

#[derive(Debug, Deserialize)]
struct Tags {
    tags: Vec<String>,
}

impl std::ops::Deref for Tags {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.tags
    }
}

struct Title(String);

impl Deref for Title {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct Lang(String);

impl Deref for Lang {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

struct TagsList(Vec<String>);

impl Deref for TagsList {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct StatusContent(String);

impl Deref for StatusContent {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn extract_title_lang_tags(content: &str, config: &SocialCfg) -> Result<(Title, Lang, TagsList)> {
    let mut title = String::new();
    let mut lang = String::new();
    let mut returned_tags = Vec::new();

    // extract title and lang
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("title") {
            let parts: Vec<&str> = line.split('=').collect();
            title = parts
                .get(1)
                .map(|t| t.replace('"', "").trim().to_string())
                .ok_or_else(|| anyhow!("No title after `title` line"))?;
        } else if line.starts_with("tags") {
            let tags = match toml::from_str::<Tags>(line) {
                Ok(tags) => Some(tags),
                Err(e) => {
                    error!("Error in push_to_social: {}", e);
                    None
                }
            };

            // search if a lang tag is present to change the lang of the toot
            lang = if let Some(tags) = tags.as_ref() {
                let lang = config
                    .tag_lang
                    .as_ref()
                    .map(|langs| {
                        for tag_lang in langs.iter() {
                            let lang = tags.iter().find_map(|tag| {
                                if tag.as_str() == tag_lang.tag.as_str() {
                                    Some(tag_lang.lang.to_owned())
                                } else {
                                    None
                                }
                            });

                            if let Some(lang) = lang {
                                return lang;
                            }
                        }

                        config.default_lang.to_owned()
                    })
                    .unwrap_or(config.default_lang.to_owned());

                // slugify tags
                returned_tags = tags
                    .iter()
                    .filter_map(|tag| {
                        if !config.filtered_tag.contains(tag) {
                            let tag = slug::slugify(tag);
                            let parts = tag.split('-');
                            let tag = parts.fold(String::new(), |mut acc, part| {
                                acc.push_str(&part[0..1].to_uppercase());
                                acc.push_str(&part[1..]);
                                acc
                            });
                            Some(tag)
                        } else {
                            None
                        }
                    })
                    .collect();
                lang
            } else {
                config.default_lang.clone()
            };
        }
    }
    Ok((Title(title), Lang(lang), TagsList(returned_tags)))
}

fn read_template(path: &Path, social: &SocialCfg, cur_lang: &Lang) -> Result<String> {
    fn read_file(path: &Path) -> Result<String> {
        let mut file = File::open(path)?;
        let mut template = String::new();
        file.read_to_string(&mut template)?;
        Ok(template)
    }
    // let path = path.join(&mastodon.social_template);
    if path.exists() && cur_lang.0 == social.default_lang {
        read_file(path)
    } else {
        // try with lang suffix (ex: "mastodon.fr.txt")
        let path = path_with_lang(path, cur_lang)?;
        if path.exists() {
            read_file(&path)
        } else {
            bail!("No template found: {}", path.to_string_lossy())
        }
    }
}

fn path_with_lang(path: &Path, lang: &Lang) -> Result<PathBuf> {
    Ok(path.with_file_name(format!(
        "{}.{}.txt",
        path.file_stem()
            .ok_or_else(|| anyhow!("No filename"))?
            .to_string_lossy(),
        lang
    )))
}

fn create_toot_content(
    templates_dir: &Path,
    dest: &Path,
    cfg: &SocialCfg,
    title: &Title,
    lang: &Lang,
    tags: &TagsList,
) -> Result<StatusContent> {
    let toot_tpl = templates_dir.join(&cfg.social_template);

    let template = read_template(&toot_tpl, cfg, lang)?;

    // template filling
    // fill title
    let status = template.replace("{title}", title);

    // fill link
    let link = format!(
        "{}/posts/{}/",
        cfg.base_url,
        dest.file_stem()
            .expect("Should have file_name by now")
            .to_string_lossy()
    );
    let status = status.replace("{link}", &link);

    // fill tags
    let tags_list = tags.iter().fold(String::new(), |mut res, tag| {
        res.push('#');
        res.push_str(tag);
        res.push(' ');
        if tag == "rust" {
            // both tags are used for Rust programming language
            res.push_str("#RustLang ");
        }
        res
    });
    Ok(StatusContent(
        status.replace("{tags}", &tags_list).trim().to_owned(),
    ))
}

fn create_toot_link(
    templates_dir: &Path,
    cfg: &SocialCfg,
    cur_lang: &Lang,
    links: &str,
) -> Result<String> {
    let toot_link_tpl = templates_dir.join(&cfg.link_template);
    let tpl = read_template(&toot_link_tpl, cfg, cur_lang)?;
    Ok(tpl.replace("{links}", links))
}

pub async fn push_to_social(cfg: &SocialCfg, content: &str, dest: &Path) -> Result<String> {
    if cfg.instances.is_empty() {
        bail!("No social servers defined.");
    }

    let (title, language, tags) = extract_title_lang_tags(content, cfg)?;

    let templates_dir = PathBuf::from("./templates/");
    let status = create_toot_content(&templates_dir, dest, cfg, &title, &language, &tags)?;
    let mut links = HashMap::<SocialApi, Url>::new();

    for instance in &cfg.instances {
        let url = match instance.api {
            SocialApi::Mastodon => push_to_mastodon(instance, &status, &language).await?,
            SocialApi::Bluesky => push_to_bsky(instance, &status, &language).await?,
        };
        if let Some(url) = url {
            links.insert(instance.api, url);
        }
    }

    let links = links
        .into_iter()
        .fold(String::new(), |mut acc, (api, url)| {
            if !acc.is_empty() {
                acc.push_str(", ");
            }
            acc.push_str(&format!("[{api}]({url})"));
            acc
        });

    info!("Inject social links: {links:?}");

    let new_content = content.replace(
        &cfg.link_tag,
        &create_toot_link(&templates_dir, cfg, &language, &links)?,
    );

    Ok(new_content)
}
