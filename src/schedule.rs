use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use anyhow::{bail, Result};

pub fn schedule_publish(date: &str, slug: &str) -> Result<()> {
    let date = date.trim_matches('"');
    let mut child = Command::new("at")
        .arg(date)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    {
        let stdin = child.stdin.as_mut().expect("Failed to open stdin");
        stdin.write_all(format!("emile publish-flow {}", slug).as_bytes())?;
    }
    match child.wait_with_output() {
        Err(e) => match e.kind() {
            io::ErrorKind::NotFound => {
                bail!("`at` was not found, please install it.");
            }
            _ => {
                bail!("{}", e);
            }
        },
        Ok(output) => {
            if output.status.success() {
                let message = String::from_utf8_lossy(&output.stderr).to_string();
                for line in message.lines() {
                    if line.starts_with("job") {
                        let mut file = OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open("jobs_list")?;
                        writeln!(file, "{} \"{}\"", line, slug)?;
                    }
                }
            } else {
                bail!(
                    "out: {}\nerr: {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
    }
    Ok(())
}

pub fn clean_jobs_list(slug: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .truncate(true)
        .open("jobs_list")?;
    let reader = BufReader::new(&file);
    let mut new_content = String::new();
    let pattern = format!("\"{}\"", slug);
    for line in reader.lines() {
        let line = line.expect("Should have text");
        if !line.ends_with(&pattern) {
            new_content.push_str(&line);
        }
    }
    file.write_all(new_content.as_bytes())?;
    Ok(())
}

pub fn unschedule_publish(slug: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .truncate(true)
        .open("jobs_list")?;
    let reader = BufReader::new(&file);
    let mut new_content = String::new();
    let pattern = format!("\"{}\"", slug);
    for line in reader.lines() {
        let line = line.expect("Should have text");
        if !line.ends_with(&pattern) {
            new_content.push_str(&line);
        } else {
            let mut iter = line.split_ascii_whitespace();
            iter.next().expect("Should have `job` word");
            let job_number = iter.next().expect("Should have job number");
            match Command::new("atrm").arg(job_number).output() {
                Ok(output) => {
                    if output.status.success() {
                        io::stdout().write_all(&output.stdout)?;
                    } else {
                        bail!(
                            "error while atrm job {} ({})",
                            job_number,
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                }
                Err(e) => match e.kind() {
                    io::ErrorKind::NotFound => {
                        bail!("`atrm` was not found, please install it.");
                    }
                    _ => {
                        bail!("{}", e);
                    }
                },
            }
        }
    }
    file.write_all(new_content.as_bytes())?;
    Ok(())
}
