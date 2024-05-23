use std::path::PathBuf;

use anyhow::{bail, Result};
use chrono::{DateTime, Datelike, Local, NaiveDate, Timelike};
use slug::slugify;

use crate::config::SiteConfig;
use crate::format_date;
use crate::post::modify_front;

pub fn create_draft(title: &str, cfg: &SiteConfig) -> Result<()> {
    if !cfg.drafts_creation_dir.exists() {
        std::fs::create_dir_all(&cfg.drafts_creation_dir)?;
    }

    let date = {
        let today = Local::now();
        let date = NaiveDate::from_ymd_opt(
            today.year() + cfg.drafts_year_shift,
            today.month(),
            today.day(),
        )
        .expect(&format!(
            "`drafts_year_shift` value `{}` made creation of chrono::NaiveDate fail",
            cfg.drafts_year_shift
        ))
        .and_hms_opt(today.hour(), today.minute(), today.day())
        .unwrap();
        DateTime::from_naive_utc_and_offset(date, cfg.timezone)
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
        modify_front(&src, |line: &str| {
            if line.starts_with("+++") {
                Ok(format!(
                    "+++\ntitle = \"{title}\"\ndate = {}\ndraft = true\n",
                    format_date(&date)
                ))
            } else {
                Ok(format!("{line}\n"))
            }
        })?
    } else {
        format!(
            "+++\ntitle = \"{title}\"\ndate = {}\ndraft = true\n+++\n",
            format_date(&date)
        )
    };
    std::fs::write(&dest, new_content)?;
    println!("Success: post `{}` created.", &dest.to_string_lossy());
    Ok(())
}
