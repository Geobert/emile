use std::fs::{self, DirEntry, File};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use anyhow::{bail, Result};
use chrono::Local;
use slug::slugify;

use crate::config::Config;

fn does_same_title_exist(slug: &str, drafts_creation_dir: &PathBuf) -> Result<Option<DirEntry>> {
    let end_of_filename = format!("{}.md", slug);
    if let Some(res) = fs::read_dir(&drafts_creation_dir)?.find(|f| {
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

pub fn create_draft(title: &str, cfg: &Config) -> Result<()> {
    let drafts_creation_dir = cfg
        .drafts_creation_dir
        .as_ref()
        .expect("Should have a value by now.");
    if !drafts_creation_dir.exists() {
        fs::create_dir_all(drafts_creation_dir)?;
    }
    let date = if cfg.drafts_date.is_none() {
        Local::now().naive_local().date()
    } else {
        cfg.drafts_date.unwrap()
    };
    let slug = slugify(title);
    let filename = format!("{}{}.md", date.format("%Y-%m-%d-").to_string(), &slug);
    let dest = drafts_creation_dir.join(&filename);
    if dest.exists() {
        bail!("file `{}` already exists.", filename.to_string());
    }

    if let Some(similar_file) = does_same_title_exist(&slug, &drafts_creation_dir)? {
        eprintln!(
            "Warning: a post with a the same title exists: `{}`",
            similar_file.file_name().to_string_lossy()
        );
    }

    let mut src = PathBuf::from("./templates/");
    src.push(
        &cfg.draft_template
            .as_ref()
            .expect("Should have a filename at this point."),
    );
    if !src.is_file() {
        bail!("`draft_template` is not a file.");
    }
    if !src.exists() {
        bail!("can't find any template for a draft in `templates`. Please create `draft.html`.");
    }
    let file = File::open(&src)?;
    let reader = BufReader::new(&file);
    let mut new_content = String::new();
    for (idx, line) in reader.lines().enumerate() {
        if idx == 0 {
            new_content.push_str("+++\n");
            new_content.push_str(format!("title = \"{}\"\n", title).as_str());
            new_content.push_str("draft = true\n");
        } else {
            new_content.push_str(&line.expect("Should have text here."));
            new_content.push('\n');
        }
    }
    fs::write(&dest, new_content)?;
    println!("File `{}` created.", &dest.to_string_lossy());
    Ok(())
}
