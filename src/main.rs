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

use git::BlogCommand;
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
            let log_command = git::get_last_log()?;
            match log_command.command {
                BlogCommand::BlogBuild => zola_build(),
                BlogCommand::BlogSched => schedule::schedule_publish(
                    &log_command.date.expect("No date specified."),
                    &log_command.slug.expect("Missing slug."),
                ),
                BlogCommand::BlogUnsched => {
                    schedule::unschedule_publish(&log_command.slug.expect("Missing slug"))
                }
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
