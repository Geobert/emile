use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use anyhow::{bail, Result};
use time::{
    format_description::well_known::Rfc3339, macros::format_description, Date, OffsetDateTime,
};

use crate::config::SiteConfig;

pub fn modify_front(
    path: &Path,
    mut operation: impl FnMut(&str) -> Result<String>,
) -> Result<String> {
    let file = File::open(path)?;
    let reader = BufReader::new(&file);
    let mut new_content = String::new();
    let mut in_frontmatter = true;
    let mut nb_sep = 0;
    for line in reader.lines() {
        let line = line.expect("Should have text");
        if in_frontmatter {
            if line.starts_with("+++") {
                nb_sep += 1;
            }

            if nb_sep >= 2 {
                in_frontmatter = false;
                new_content.push_str(&line);
                new_content.push('\n');
            } else {
                new_content.push_str(&operation(&line)?);
            }
        } else {
            new_content.push_str(&line);
            new_content.push('\n');
        }
    }

    if in_frontmatter {
        bail!("Missing `+++` delimiter")
    } else {
        Ok(new_content)
    }
}

pub fn extract_date(path: &Path, cfg: &SiteConfig) -> Result<OffsetDateTime> {
    let file = File::open(path)?;
    let reader = BufReader::new(&file);
    let mut in_front = true;
    let mut nb_sep = 0;
    for line in reader.lines() {
        let line = line.expect("Should have text");
        if in_front {
            if line.starts_with("+++") {
                nb_sep += 1;
                if nb_sep >= 2 {
                    in_front = false;
                }
            } else if line.starts_with("date") {
                let date_split: Vec<_> = line.split('=').collect();
                if date_split.len() != 2 {
                    bail!("Invalid `date`");
                }
                let date_str = date_split.get(1).unwrap().trim();
                let date = if date_str.len() == 10 {
                    Date::parse(date_str, &format_description!("[year]-[month]-[day]"))?
                        .with_hms(0, 0, 0)?
                        .assume_offset(cfg.timezone)
                } else {
                    OffsetDateTime::parse(date_str, &Rfc3339)?
                };
                return Ok(date);
            }
        } else {
            bail!("No `date` in frontmatter")
        }
    }
    bail!("No `date` in frontmatter")
}
