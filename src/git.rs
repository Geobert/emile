use std::io;
use std::process::Command;

use crate::config::Config;
use anyhow::{bail, Result};

#[derive(Debug, PartialEq, Eq)]
pub enum BlogCommand {
    BlogBuild,
    BlogSched,
    BlogUnsched,
}
#[derive(Debug, PartialEq, Eq)]
pub struct LogCommand {
    pub command: BlogCommand,
    pub date: Option<String>,
    pub slug: Option<String>,
}

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

pub fn get_last_log() -> Result<LogCommand> {
    match Command::new("git")
        .arg("log")
        .arg("-n")
        .arg("1")
        .arg("--format=%B")
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                parse_last_log(&String::from_utf8_lossy(&output.stdout).to_string())
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

// returns (command, slug, date)
fn parse_last_log(log: &str) -> Result<LogCommand> {
    let mut split_log = log.split_ascii_whitespace();
    let command = split_log.next().expect("Empty log");
    Ok(match command {
        "blog_build" => LogCommand {
            command: BlogCommand::BlogBuild,
            date: None,
            slug: None,
        },
        "blog_sched" => {
            let start = log
                .find('\"')
                .expect("Should have starting quote in schedule command");
            let end = log
                .rfind('\"')
                .expect("Should have ending quote in schedule command");
            if start >= end || end >= log.len() {
                bail!("Malformed schedule command: {}", log);
            }
            let date = log[start + 1..end].to_string();
            let slug = log[end + 1..].trim().to_string();
            LogCommand {
                command: BlogCommand::BlogSched,
                date: Some(date),
                slug: Some(slug),
            }
        }
        "blog_unsched" => {
            let slug = split_log.next().expect("No slug specified");
            LogCommand {
                command: BlogCommand::BlogUnsched,
                date: None,
                slug: Some(slug.trim().to_string()),
            }
        }
        _ => bail!("unknown command: {}", command),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_log_schedule_command() {
        let expected = LogCommand {
            command: BlogCommand::BlogSched,
            date: Some("11:11 + 3 days".to_string()),
            slug: Some("my-post".to_string()),
        };

        assert_eq!(
            expected,
            parse_last_log("blog_sched \"11:11 + 3 days\" my-post").unwrap()
        );
    }
}
