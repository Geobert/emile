use std::fs::{self, DirEntry, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use once_cell::sync::Lazy;
use serde_derive::{Deserialize, Serialize};
use time::OffsetDateTime;
use tracing::{error, info};

use crate::config::{MastodonCfg, SiteConfig};
use crate::format_date;
use crate::post::modify_front;

static MASTODON_TOKEN: Lazy<Option<String>> =
    Lazy::new(|| std::env::var("EMILE_MASTODON_TOKEN").ok());

pub async fn publish_post(slug: &str, src_dir: &Path, cfg: &SiteConfig) -> Result<String> {
    let filename = format!("{}.md", &slug);
    let src = src_dir.join(&filename);
    if !src.exists() {
        bail!("`{}` doesn't exist", src.to_string_lossy());
    }

    let date = OffsetDateTime::now_utc().to_offset(cfg.timezone);
    let new_content = modify_front(&src, |cur_line: &str| {
        let modified = if cur_line.starts_with("date = ") {
            // modify date
            format!("date = {}\n", format_date(&date)?)
        } else if !cur_line.starts_with("draft =") {
            // don’t modify
            format!("{cur_line}\n")
        } else {
            // delete `draft` line
            "".to_string()
        };
        Ok(modified)
    })?;

    let dest = cfg.publish_dest.join(&filename);
    if dest.exists() {
        bail!("file {} already exists.", dest.to_string_lossy());
    }

    if let Some(similar_file) = does_same_title_exist(slug, &cfg.publish_dest)? {
        bail!(
            "Warning: a post with a the same title exists: `{}`",
            similar_file.file_name().to_string_lossy()
        );
    }

    fs::write(&dest, &new_content)?;
    fs::remove_file(&src)?;

    if cfg.mastodon.is_some() {
        info!("Some mastodon config, push to social");
        push_to_social(cfg, &new_content, &dest).await?;
    } else {
        info!("No mastodon config");
    }

    Ok(dest.to_string_lossy().to_string())
}

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

#[derive(Serialize, Debug)]
struct Toot {
    status: String,
    visibility: &'static str,
    language: String,
}

fn extract_title_lang_tags(
    content: &str,
    config: &MastodonCfg,
) -> Result<(String, String, Vec<String>)> {
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
                            Some(slug::slugify(tag))
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
    Ok((title, lang, returned_tags))
}

// token alt mamot account:3q2wjhBJlwxxp_H-XuPXo_1HFCiA-xEr3JDz8FP0-LQ
async fn push_to_social(config: &SiteConfig, content: &str, dest: &Path) -> Result<()> {
    let mastodon = config.mastodon.as_ref().unwrap(); // push_to_social called only if we have config.mastodon
    if let Some(token) = Lazy::force(&MASTODON_TOKEN) {
        fn read_template(path: &Path) -> Result<String> {
            let mut file = File::open(path)?;
            let mut template = String::new();
            file.read_to_string(&mut template)?;
            Ok(template)
        }

        fn set_path_with_lang(path: &mut PathBuf, lang: &str) -> Result<()> {
            path.set_file_name(format!(
                "{}.{}.txt",
                path.file_stem()
                    .ok_or_else(|| anyhow!("No filename"))?
                    .to_string_lossy(),
                lang
            ));
            Ok(())
        }

        let (title, language, tags) = extract_title_lang_tags(content, mastodon)?;

        // get the correct template
        let mut path = PathBuf::from("./templates/");
        path.push(&mastodon.social_template);

        let template = if path.exists() && language == mastodon.default_lang {
            // default lang doesn’t need lang suffix
            read_template(&path)?
        } else {
            // try with lang suffix (ex: "mastodon.fr.txt")
            set_path_with_lang(&mut path, &language)?;
            if path.exists() {
                read_template(&path)?
            } else {
                bail!("No template found: {}", path.to_string_lossy());
            }
        };

        // template filling
        // fill title
        let status = template.replace("{title}", &title);

        // fill link
        let link = format!(
            "{}/posts/{}",
            mastodon.base_url,
            dest.file_stem()
                .expect("Should have file_name by now")
                .to_string_lossy()
        );
        let status = status.replace("{link}", &link);

        // fill tags
        let tags_list = tags.iter().fold(String::new(), |mut res, tag| {
            res.push('#');
            res.push_str(&tag.replace('-', ""));
            res.push(' ');
            if tag == "rust" {
                // both tags are used for Rust programming language
                res.push_str("#RustLang ");
            }
            res
        });
        let status = status.replace("{tags}", &tags_list).trim().to_owned();

        // publish toot
        let toot = Toot {
            status,
            visibility: "public",
            language,
        };

        use sha2::{Digest, Sha256};
        let hash = format!("{:x}", Sha256::digest(toot.status.as_bytes()));

        reqwest::Client::new()
            .post(&format!("https://{}/api/v1/statuses", mastodon.instance))
            .bearer_auth(token)
            .header("Idempotency-Key", hash)
            .json(&toot)
            .send()
            .await?;
    } else {
        error!("No EMILE_MASTODON_TOKEN env var");
    }
    Ok(())
}

fn does_same_title_exist(slug: &str, dir: &Path) -> Result<Option<DirEntry>> {
    let end_of_filename = format!("{slug}.md");
    if let Some(res) = fs::read_dir(dir)?.find(|f| {
        let f = f.as_ref().expect("Should have a valid entry");
        if f.file_type().expect("Should have a FileType").is_file() {
            f.file_name().to_string_lossy().contains(&end_of_filename)
        } else {
            false
        }
    }) {
        Ok(Some(res.expect("Should have DirEntry")))
    } else {
        Ok(None)
    }
}
