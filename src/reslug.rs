use std::fs;
use std::io::{BufRead, BufReader};

use anyhow::{bail, Result};
use slug::slugify;

fn _reslug(file_path: &std::path::Path) -> Result<()> {
    let file = std::fs::File::open(&file_path)?;
    let reader = BufReader::new(&file);
    let mut title = String::new();
    for line in reader.lines() {
        let line = line.expect("Should have text");
        if line.starts_with("title") {
            let start = line.find('\"').expect("Should have starting quote in toml");
            let end = line.rfind('\"').expect("Should have ending quote in toml");
            if start < end {
                title = line.clone().drain(start + 1..end).collect();
            } else {
                bail!("Corrupted title: {}", line);
            }
            break;
        }
    }
    let dest_file = file_path.with_file_name(format!("{}.md", slugify(title)));
    std::fs::rename(&file_path, &dest_file)?;
    println!(
        "Renamed `{}` to `{}`",
        file_path.display(),
        dest_file.display()
    );
    Ok(())
}

pub fn reslug(path: &std::path::Path) -> Result<()> {
    if path.is_file() {
        _reslug(&path)
    } else {
        for entry in fs::read_dir(&path)? {
            if let Ok(entry) = entry {
                let filename = entry.file_name();
                let filename = filename.to_string_lossy();
                if filename.starts_with("-") {
                    _reslug(&entry.path())?;
                }
            }
        }
        Ok(())
    }
}
