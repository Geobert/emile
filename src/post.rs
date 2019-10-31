use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use anyhow::Result;

pub fn modify_post(
    path: &Path,
    mut operation: impl FnMut(&str, bool) -> Result<String>,
) -> Result<String> {
    let file = File::open(&path)?;
    let reader = BufReader::new(&file);
    let mut new_content = String::new();
    let mut in_frontmatter = true;
    let mut nb_sep = 0;
    for line in reader.lines() {
        let line = line.expect("Should have text");
        if in_frontmatter {
            if line.starts_with("+++") {
                nb_sep += 1;
                if nb_sep >= 2 {
                    in_frontmatter = false;
                }
            }
            new_content.push_str(&operation(&line, in_frontmatter)?);
        } else {
            new_content.push_str(&operation(&line, in_frontmatter)?);
        }
    }
    Ok(new_content)
}
