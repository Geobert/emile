use anyhow::{bail, Result};
use config::Config;

use structopt::StructOpt;

mod cmd;
mod config;
mod new;
mod opt;
mod post;
mod publish;

use opt::Opt;

fn main() -> Result<()> {
    let opt = Opt::from_args();
    let cfg = Config::get_config();

    match opt {
        Opt::New { title } => new::create_draft(&title, &cfg),
        Opt::Publish { slug } => {
            cmd::update_repo()?;
            publish::publish_post(&slug, &cfg)?;
            cmd::clean_jobs_list(&slug)?;
            cmd::update_remote(&slug, &cfg)
        }
        Opt::GitHook {} => {
            cmd::update_repo()?;
            let log = cmd::get_last_log()?;
            let mut log = log.split_ascii_whitespace();
            let command = log.next().expect("Empty log");
            match command {
                "blog_build" => cmd::zola_build(),
                "blog_sched" => {
                    let date = log.next().expect("No date specified");
                    let slug = log.next().expect("No slug specified");
                    cmd::schedule_publish(date, slug)
                }
                "blog_cancel" => {
                    let slug = log.next().expect("No slug specified");
                    cmd::cancel_schedule(slug)
                }
                _ => bail!("unknown command: {}", command),
            }
        }
        Opt::Unschedule { slug } => cmd::cancel_schedule(&slug),
    }
}
