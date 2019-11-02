use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Result};
use chrono::{Datelike, Local, NaiveDate};
use slug::slugify;

use crate::config::Config;
use crate::post::modify_post;

pub fn create_draft(title: &str, cfg: &Config) -> Result<()> {
    let drafts_creation_dir = cfg
        .drafts_creation_dir
        .as_ref()
        .expect("Should have a value by now.");
    if !drafts_creation_dir.exists() {
        fs::create_dir_all(drafts_creation_dir)?;
    }

    let date = if cfg.drafts_year_shift.is_some() {
        let shift = cfg.drafts_year_shift.unwrap();
        let today = Local::now().naive_local().date();
        NaiveDate::from_ymd(today.year() + shift, today.month(), today.day())
    } else {
        Local::now().naive_local().date()
    };

    let slug = slugify(title);
    let filename = format!("{}.md", &slug);
    let dest = drafts_creation_dir.join(&filename);
    if dest.exists() {
        bail!("file `{}` already exists.", filename.to_string());
    }

    let mut src = PathBuf::from("./templates/");
    src.push(
        &cfg.draft_template
            .as_ref()
            .expect("Should have a filename at this point."),
    );
    if src.exists() && !src.is_file() {
        bail!("`draft_template` is not a file.");
    }
    let new_content = if src.exists() {
        modify_post(&src, |line: &str, in_frontmatter| {
            if in_frontmatter && line.starts_with("+++") {
                Ok(format!(
                    "+++\ntitle = \"{}\"\ndate = {}\ndraft = true\n",
                    title,
                    date.format("%Y-%m-%d")
                ))
            } else {
                Ok(format!("{}\n", line))
            }
        })?
    } else {
        format!(
            "+++\ntitle = \"{}\"\ndate = {}\ndraft = true\n+++\n",
            title,
            date.format("%Y-%m-%d")
        )
    };
    fs::write(&dest, new_content)?;
    println!("Success: post `{}` created.", &dest.to_string_lossy());
    Ok(())
}
