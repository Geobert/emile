use std::fs::{self, DirEntry};
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use time::{format_description, OffsetDateTime};

use crate::config::Config;
use crate::post::modify_post;

pub fn publish_post(slug: &str, cfg: &Config) -> Result<String> {
    if !cfg.drafts_consumption_dir.exists() {
        bail!(
            "`{}` doesn't exist",
            cfg.drafts_consumption_dir.to_string_lossy()
        );
    }
    let src = cfg.drafts_consumption_dir.join(&format!("{}.md", &slug));

    let date = OffsetDateTime::now_utc().date();
    let new_content = modify_post(&src, |cur_line: &str, in_frontmatter| {
        if in_frontmatter {
            if cur_line.starts_with("date = ") {
                let date_format = format_description::parse("[year]-[month]-[day]")?;
                Ok(format!("date = {}\n", date.format(&date_format)?))
            } else if !cur_line.starts_with("draft =") {
                Ok(format!("{}\n", cur_line))
            } else {
                Ok("".to_string())
            }
        } else {
            Ok(format!("{}\n", cur_line))
        }
    })?;

    let dest_filename = format!("{}.md", &slug);
    let dest = cfg.publish_dest.join(&dest_filename);
    if dest.exists() {
        bail!("file {} already exists.", dest.to_string_lossy());
    }
    if let Some(similar_file) = does_same_title_exist(slug, &cfg.publish_dest)? {
        eprintln!(
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
