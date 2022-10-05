use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Result};
use slug::slugify;
use time::{Date, OffsetDateTime};

use crate::config::SiteConfig;
use crate::post::modify_post;
use crate::DATE_SHORT_FORMAT;

pub fn create_draft(title: &str, cfg: &SiteConfig) -> Result<()> {
    if !cfg.drafts_creation_dir.exists() {
        fs::create_dir_all(&cfg.drafts_creation_dir)?;
    }

    let date = {
        let today = OffsetDateTime::now_utc();
        Date::from_calendar_date(
            today.year() + cfg.drafts_year_shift,
            today.month(),
            today.day(),
        )?
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
        bail!("`draft_template` is not a file.");
    }
    let new_content = if src.exists() {
        modify_post(&src, |line: &str, in_frontmatter| {
            if in_frontmatter && line.starts_with("+++") {
                Ok(format!(
                    "+++\ntitle = \"{}\"\ndate = {}\ndraft = true\n",
                    title,
                    date.format(&DATE_SHORT_FORMAT)?
                ))
            } else {
                Ok(format!("{}\n", line))
            }
        })?
    } else {
        format!(
            "+++\ntitle = \"{}\"\ndate = {}\ndraft = true\n+++\n",
            title,
            date.format(&DATE_SHORT_FORMAT)?
        )
    };
    fs::write(&dest, new_content)?;
    println!("Success: post `{}` created.", &dest.to_string_lossy());
    Ok(())
}
