use std::fs::{self, DirEntry};
use std::path::Path;

use anyhow::{bail, Result};
use time::OffsetDateTime;

use crate::config::SiteConfig;
use crate::format_date;
use crate::post::modify_post;

pub fn publish_post(slug: &str, src_dir: &Path, cfg: &SiteConfig) -> Result<String> {
    let filename = format!("{}.md", &slug);
    let src = src_dir.join(&filename);
    if !src.exists() {
        bail!("`{}` doesn't exist", src.to_string_lossy());
    }

    let date = OffsetDateTime::now_utc().to_offset(cfg.timezone);
    let new_content = modify_post(&src, |cur_line: &str, in_frontmatter| {
        if in_frontmatter {
            if cur_line.starts_with("date = ") {
                Ok(format!("date = {}\n", format_date(&date)?))
            } else if !cur_line.starts_with("draft =") {
                Ok(format!("{}\n", cur_line))
            } else {
                Ok("".to_string())
            }
        } else {
            Ok(format!("{}\n", cur_line))
        }
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
    fs::write(&dest, new_content)?;
    fs::remove_file(&src)?;

    Ok(dest.to_string_lossy().to_string())
}

fn does_same_title_exist(slug: &str, dir: &Path) -> Result<Option<DirEntry>> {
    let end_of_filename = format!("{}.md", slug);
    if let Some(res) = fs::read_dir(&dir)?.find(|f| {
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
