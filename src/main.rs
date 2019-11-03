use std::io::Write;

use anyhow::{bail, Result};
use config::Config;
use structopt::StructOpt;

mod config;
mod git;
mod new;
mod opt;
mod post;
mod publish;
mod reslug;
mod schedule;

use opt::Opt;

fn main() -> Result<()> {
    let opt = Opt::from_args();
    let cfg = Config::get_config();

    match opt {
        Opt::New { title } => new::create_draft(&title, &cfg),
        Opt::Publish { slug } => {
            let dest = publish::publish_post(&slug, &cfg)?;
            println!(
                "Success: post `{}` published. Call `zola build` to build the site.",
                dest
            );
            Ok(())
        }
        Opt::PublishFlow { slug } => {
            git::update_repo()?;
            publish::publish_post(&slug, &cfg)?;
            zola_build()?;
            schedule::clean_jobs_list(&slug)?;
            git::update_remote(&slug, &cfg)
        }
        Opt::GitHook {} => {
            git::update_repo()?;
            let log = git::get_last_log()?;
            let mut log = log.split_ascii_whitespace();
            let command = log.next().expect("Empty log");
            match command {
                "blog_build" => zola_build(),
                "blog_sched" => {
                    let date = log.next().expect("No date specified");
                    let slug = log.next().expect("No slug specified");
                    schedule::schedule_publish(date, slug)
                }
                "blog_unsched" => {
                    let slug = log.next().expect("No slug specified");
                    schedule::unschedule_publish(slug)
                }
                _ => bail!("unknown command: {}", command),
            }
        }
        Opt::Unschedule { slug } => schedule::unschedule_publish(&slug),
        Opt::Schedule { date, slug } => schedule::schedule_publish(&date, &slug),
        Opt::Reslug { path } => reslug::reslug(&path),
    }
}

fn zola_build() -> Result<()> {
    match std::process::Command::new("zola").arg("build").output() {
        Ok(output) => {
            if output.status.success() {
                std::io::stdout().write_all(&output.stdout)?;
                Ok(std::io::stdout().flush()?)
            } else {
                bail!("{}", String::from_utf8_lossy(&output.stdout));
            }
        }
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => {
                bail!("`zola` was not found, please verify the PATH env.");
            }
            _ => {
                bail!("{}", e);
            }
        },
    }
}
