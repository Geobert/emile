use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Result};
use slug::slugify;
use time::OffsetDateTime;

use crate::config::SiteConfig;
use crate::format_date;
use crate::post::modify_post;

pub fn create_draft(title: &str, cfg: &SiteConfig) -> Result<()> {
    if !cfg.drafts_creation_dir.exists() {
        fs::create_dir_all(&cfg.drafts_creation_dir)?;
    }

    let date = {
        let today = OffsetDateTime::now_utc();
        let today = today.replace_year(today.year() + cfg.drafts_year_shift)?;
        today.replace_offset(cfg.timezone)
    };

    let slug = slugify(title);
    let filename = format!("{}.md", &slug);
    let dest = cfg.drafts_creation_dir.join(&filename);
    if dest.exists() {
        bail!("file `{}` already exists.", filename);
    }

    let mut src = PathBuf::from("./templates/");
    src.push(&cfg.draft_template);
    if src.exists() && !src.is_file() {
        bail!("`{}` is not a file.", cfg.draft_template);
    }
    let new_content = if src.exists() {
        modify_post(&src, |line: &str, in_frontmatter| {
            if in_frontmatter && line.starts_with("+++") {
                Ok(format!(
                    "+++\ntitle = \"{}\"\ndate = {}\ndraft = true\n",
                    title,
                    format_date(&date)?
                ))
            } else {
                Ok(format!("{}\n", line))
            }
        })?
    } else {
        format!(
            "+++\ntitle = \"{}\"\ndate = {}\ndraft = true\n+++\n",
            title,
            format_date(&date)?
        )
    };
    fs::write(&dest, new_content)?;
    println!("Success: post `{}` created.", &dest.to_string_lossy());
    Ok(())
}
