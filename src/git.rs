use std::io;
use std::process::Command;

use crate::config::Config;
use anyhow::{bail, Result};

pub fn update_repo() -> Result<()> {
    match Command::new("git").arg("pull").output() {
        Ok(output) => {
            if !output.status.success() {
                bail!(
                    "issue updating repo: {}\nerr: {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
        Err(e) => match e.kind() {
            io::ErrorKind::NotFound => {
                bail!("`git` was not found, please verify the PATH env.");
            }
            _ => {
                bail!("{}", e);
            }
        },
    }
    Ok(())
}

pub fn update_remote(slug: &str, cfg: &Config) -> Result<()> {
    let dest_dir = cfg
        .publish_dest
        .as_ref()
        .expect("Should have a value by now")
        .to_string_lossy();
    Command::new("git")
        .arg("add")
        .arg(format!("{}*.md", dest_dir))
        .output()?;
    Command::new("git")
        .arg("commit")
        .arg("-a")
        .arg("-m")
        .arg(format!("\"published {}.md\"", slug))
        .output()?;
    Command::new("git").arg("push").output()?;
    Ok(())
}

pub fn get_last_log() -> Result<String> {
    match Command::new("git")
        .arg("log")
        .arg("-n")
        .arg("1")
        .arg("--format=%B")
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            } else {
                bail!("{}", String::from_utf8_lossy(&output.stdout));
            }
        }
        Err(e) => match e.kind() {
            io::ErrorKind::NotFound => {
                bail!("`git` was not found, please verify the PATH env.");
            }
            _ => {
                bail!("{}", e);
            }
        },
    }
}
