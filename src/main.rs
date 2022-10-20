use std::{io::Write, sync::Arc};

use anyhow::{bail, Result};
use clap::Parser;
use config::SiteConfigBuilder;

mod config;
mod new;
mod opt;
mod post;
mod publish;
mod scheduler;
mod watcher;

use opt::{Commands, Opt};
use time::{macros::format_description, OffsetDateTime};
use tracing::error;
use tracing_subscriber::{fmt::time::UtcTime, prelude::*, util::SubscriberInitExt, EnvFilter};
use watcher::SiteWatcher;

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();
    // log setup
    let _guard = if let Some(log_dir) = opt.log_dir {
        if !log_dir.is_dir() {
            error!("{} is not a valid directory", log_dir.to_string_lossy());
            bail!("Invalid log dir");
        }
        let file_appender = tracing_appender::rolling::never(log_dir, "emile.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .compact()
                    .with_timer(UtcTime::new(format_description!(
                        "[year repr:last_two]-[month]-[day] [hour]:[minute]:[second]"
                    )))
                    .with_writer(non_blocking)
                    .with_target(false),
            )
            .with(EnvFilter::try_from_env("EMILE_LOG").or_else(|_| EnvFilter::try_new("info"))?)
            .init();
        Some(guard)
    } else {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .compact()
                    .with_timer(UtcTime::new(format_description!(
                        "[year repr:last_two]-[month]-[day] [hour]:[minute]:[second]"
                    )))
                    .with_target(false),
            )
            .with(EnvFilter::try_from_env("EMILE_LOG").or_else(|_| EnvFilter::try_new("info"))?)
            .init();
        None
    };

    match opt.command {
        Commands::New { title } => {
            let cfg = SiteConfigBuilder::get_config();
            new::create_draft(&title, &cfg)
        }
        Commands::Publish { slug } => {
            let cfg = SiteConfigBuilder::get_config();
            let dest = publish::publish_post(&slug, &cfg.drafts_creation_dir, &cfg)?;
            println!(
                "Success: post `{}` published. Call `zola build` to rebuild the site.",
                dest
            );
            Ok(())
        }
        Commands::Watch { website } => {
            std::env::set_current_dir(website)?;
            let cfg = Arc::new(SiteConfigBuilder::get_config());
            tracing::debug!("{:?}", cfg);
            let change_watcher = Arc::new(SiteWatcher::new(&cfg)?);
            let schedule_watcher = change_watcher.clone();
            let (tx_scheduler, rx_scheduler) = tokio::sync::mpsc::unbounded_channel();

            let tx_scheduler_for_spawn = tx_scheduler.clone();
            let cfg_for_spawn = cfg.clone();
            tokio::spawn(async move {
                scheduler::start_scheduler(
                    schedule_watcher,
                    cfg_for_spawn,
                    tx_scheduler_for_spawn,
                    rx_scheduler,
                )
                .await;
            });

            watcher::start_watching(change_watcher, cfg, tx_scheduler).await?;
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
                bail!(
                    "{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
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

fn format_date(date: &OffsetDateTime) -> Result<String> {
    Ok(date.format(&format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:[second][offset_hour sign:mandatory]:[offset_minute]"
    ))?)
}
