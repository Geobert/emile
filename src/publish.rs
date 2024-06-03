use std::fs::{self, DirEntry};
use std::path::Path;

use anyhow::{bail, Result};
use chrono::Utc;

use crate::config::SiteConfig;
use crate::format_date;
use crate::post::modify_front;
use crate::social::push_to_social;

pub async fn publish_post(post: &Path, cfg: &SiteConfig) -> Result<String> {
    if !post.exists() {
        bail!("`{}` doesn't exist", post.to_string_lossy());
    }

    if !(post.starts_with(&cfg.drafts_creation_dir) || post.starts_with(&cfg.schedule_dir)) {
        bail!(
            "Post to be published must be in `{}` or `{}`",
            cfg.drafts_creation_dir.to_string_lossy(),
            cfg.schedule_dir.to_string_lossy()
        );
    }

    let date = Utc::now().with_timezone(&cfg.timezone);
    let new_content = modify_front(&post, |cur_line: &str| {
        let modified = if cur_line.starts_with("date = ") {
            // modify date
            format!("date = {}\n", format_date(&date))
        } else if !cur_line.starts_with("draft =") {
            // don’t modify
            format!("{cur_line}\n")
        } else {
            // delete `draft` line
            "".to_string()
        };
        Ok(modified)
    })?;
    let filename = post
        .file_name()
        .expect("a Post can’t be without a file name");
    let dest = cfg.publish_dest.join(&filename);
    if dest.exists() {
        bail!("file {} already exists.", dest.to_string_lossy());
    }

    if let Some(similar_file) =
        does_same_title_exist(&filename.to_string_lossy(), &cfg.publish_dest)?
    {
        bail!(
            "Warning: a post with a the same title exists: `{}`",
            similar_file.file_name().to_string_lossy()
        );
    }

    if let Some(social_cfg) = cfg.social.as_ref() {
        match push_to_social(social_cfg, &new_content, &dest).await {
            Ok(new_content) => {
                fs::write(&dest, &new_content)?;
                fs::remove_file(&post)?;
            }
            Err(e) => {
                // write the post even if social media failed
                fs::write(&dest, &new_content)?;
                fs::remove_file(&post)?;
                return Err(e);
            }
        }
    } else {
        fs::write(&dest, &new_content)?;
        fs::remove_file(&post)?;
    }

    Ok(dest.to_string_lossy().to_string())
}

pub fn does_same_title_exist(filename: &str, dir: &Path) -> Result<Option<DirEntry>> {
    if let Some(res) = fs::read_dir(dir)?.find(|f| {
        let f = f.as_ref().expect("Should have a valid entry");
        if f.file_type().expect("Should have a FileType").is_file() {
            f.file_name().to_string_lossy().contains(filename)
        } else {
            false
        }
    }) {
        Ok(Some(res.expect("Should have DirEntry")))
    } else {
        Ok(None)
    }
}
