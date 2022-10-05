use std::{io::Write, sync::Arc};

use anyhow::{bail, Result};
use clap::Parser;
use config::SiteConfigBuilder;
use lazy_static::lazy_static;

mod config;
mod new;
mod opt;
mod post;
mod publish;
mod watcher;

use opt::Opt;
use time::{
    format_description::{self, FormatItem},
    macros::format_description,
};
use tracing_subscriber::{
    fmt::time::UtcTime, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt,
    EnvFilter,
};
use watcher::SiteWatcher;

lazy_static! {
    static ref DATE_SHORT_FORMAT: Vec<FormatItem<'static>> =
        format_description::parse("[year]-[month]-[day]").unwrap();
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_timer(UtcTime::new(format_description!(
                    "[year]-[month]-[day] [hour]:[minute]:[second]"
                )))
                .with_target(false),
        )
        .with(EnvFilter::try_from_env("EMILE_LOG").or_else(|_| EnvFilter::try_new("info"))?)
        .init();
    let opt = Opt::parse();

    match opt {
        Opt::New { title } => {
            let cfg = SiteConfigBuilder::get_config();
            new::create_draft(&title, &cfg)
        }
        Opt::Publish { slug } => {
            let cfg = SiteConfigBuilder::get_config();
            let dest = publish::publish_post(&slug, &cfg.drafts_consumption_dir, &cfg)?;
            println!(
                "Success: post `{}` published. Call `zola build` to rebuild the site.",
                dest
            );
            Ok(())
        }
        Opt::Watch { website } => {
            std::env::set_current_dir(website)?;
            let cfg = SiteConfigBuilder::get_config();
            let change_watcher = Arc::new(SiteWatcher::new(&cfg)?);
            let schedule_watcher = change_watcher.clone();
            let (tx_scheduler, rx_scheduler) = tokio::sync::mpsc::unbounded_channel();

            let tx_scheduler_for_spawn = tx_scheduler.clone();
            tokio::spawn(async move {
                watcher::start_scheduler(schedule_watcher, tx_scheduler_for_spawn, rx_scheduler)
                    .await;
            });

            watcher::start_watching(change_watcher, &cfg, tx_scheduler).await?;
            Ok(())
        }
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
