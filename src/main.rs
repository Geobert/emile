use std::{borrow::Cow, io::Write, sync::Arc};

use anyhow::{bail, Context, Error, Result};
use chrono::{DateTime, Datelike, Days, FixedOffset, Local, NaiveTime, Timelike, Utc};
use clap::Parser;
use config::SiteConfigBuilder;

mod config;
mod new;
mod opt;
mod post;
mod publish;
mod scheduler;
mod social;
mod watcher;

use opt::{Commands, Opt};
use regex::Regex;
use tracing::error;
use tracing_subscriber::{fmt::time::UtcTime, prelude::*, EnvFilter};
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
                    .with_timer(UtcTime::rfc_3339())
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
                    .with_timer(UtcTime::rfc_3339())
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
        Commands::Publish { post } => {
            let cfg = SiteConfigBuilder::get_config();
            let dest = publish::publish_post(&post, &cfg).await?;
            zola_build()?;
            println!("Success: post `{dest}` published.");
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
        Commands::Schedule { time, post } => {
            let cfg = SiteConfigBuilder::get_config();
            let date = parse_time(&time, &cfg.default_sch_time)?;
            scheduler::schedule_post(&date, &post, &cfg)
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

fn parse_time(time_str: &str, default_time: &NaiveTime) -> Result<DateTime<FixedOffset>, Error> {
    let now = Local::now();
    let time_str = fix_time(time_str, &now);
    let datetime = match human_date_parser::from_human_time(&time_str)? {
        human_date_parser::ParseResult::DateTime(d) => d.fixed_offset(),
        human_date_parser::ParseResult::Date(d) => d
            .and_hms_opt(
                default_time.hour(),
                default_time.minute(),
                default_time.second(),
            )
            .unwrap()
            .and_local_timezone(now.timezone())
            .unwrap()
            .into(),
        human_date_parser::ParseResult::Time(t) => {
            let now_time = now.time();
            let date = if t < now_time {
                now.checked_add_days(Days::new(1)).with_context(|| {
                    format!(
                        "Failed to add one day to `{}`",
                        format_date(&now.fixed_offset())
                    )
                })?
            } else {
                now
            };
            match date.with_time(t) {
                chrono::offset::MappedLocalTime::Single(dt) => dt.fixed_offset(),
                chrono::offset::MappedLocalTime::Ambiguous(_, dt) => dt.fixed_offset(),
                chrono::offset::MappedLocalTime::None => bail!("Parsing time blew up"),
            }
        }
    };

    Ok(datetime)
}

// We accept omitted year and month. This function construct a minimal valid input to be parsed
fn fix_time<'a>(s: &'a str, now: &DateTime<Local>) -> Cow<'a, str> {
    let day = Regex::new("^[0-3]?[0-9]$").expect("Failure compiling day regex");
    if day.is_match(s) {
        return Cow::Owned(format!("{}-{}-{s}", now.year(), now.month()));
    }

    let month_day =
        Regex::new(r"^[0-1]?[0-9]\-[0-3]?[0-9]$").expect("Failure compiling month regex");
    if month_day.is_match(s) {
        return Cow::Owned(format!("{}-{s}", now.year()));
    }

    Cow::Borrowed(s)
}

fn format_date(date: &DateTime<FixedOffset>) -> String {
    date.format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

fn format_utc_date(date: &DateTime<Utc>) -> String {
    date.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
