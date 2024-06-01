use std::{borrow::Cow, io::Write, sync::Arc};

use anyhow::{bail, Context, Error, Result};
use chrono::{
    DateTime, Datelike, Days, FixedOffset, Local, Months, NaiveDate, NaiveTime, TimeZone, Timelike,
    Utc,
};
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
use tracing::{error, info};
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

    info!("emile {}", clap::crate_version!());

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

fn parse_time_with_ref(
    time_str: &str,
    ref_date: DateTime<Local>,
    default_time: &NaiveTime,
) -> Result<DateTime<FixedOffset>, Error> {
    let time_str = fix_time(time_str, &ref_date);
    let datetime = match human_date_parser::from_human_time(&time_str)
        .with_context(|| format!("Failure parsing `{time_str}`"))?
    {
        human_date_parser::ParseResult::DateTime(d) => d.fixed_offset(),
        human_date_parser::ParseResult::Date(d) => {
            let datetime: DateTime<FixedOffset> = d
                .and_hms_opt(
                    default_time.hour(),
                    default_time.minute(),
                    default_time.second(),
                )
                .unwrap()
                .and_local_timezone(ref_date.timezone())
                .unwrap()
                .into();
            datetime
        }
        human_date_parser::ParseResult::Time(t) => {
            let now_time = ref_date.time();
            let date = if t < now_time {
                ref_date.checked_add_days(Days::new(1)).with_context(|| {
                    format!(
                        "Failed to add one day to `{}`",
                        format_date(&ref_date.fixed_offset())
                    )
                })?
            } else {
                ref_date
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

fn parse_time(time_str: &str, default_time: &NaiveTime) -> Result<DateTime<FixedOffset>, Error> {
    let ref_date = Local::now();
    parse_time_with_ref(time_str, ref_date, default_time)
}

// We accept omitted year and month. This function construct a minimal valid input to be parsed
fn fix_time<'a>(s: &'a str, now: &DateTime<Local>) -> Cow<'a, str> {
    let fix_day = |day, now: &DateTime<Local>| -> DateTime<Local> {
        if day < now.day() {
            let d = Local
                .from_local_datetime(
                    &NaiveDate::from_ymd_opt(now.year(), now.month(), day)
                        .unwrap()
                        .and_hms_opt(0, 0, 0)
                        .unwrap(),
                )
                .unwrap();
            d.checked_add_months(Months::new(1))
                .expect(&format!("Add a month to `{}` blew up", now))
        } else {
            let diff = day - now.day();
            now.checked_add_days(Days::new(diff as u64))
                .expect(&format!("Add `{diff}` to `{now}` blew up"))
        }
    };

    let day = Regex::new("^[0-3]?[0-9]$").expect("Failure compiling day regex");
    if day.is_match(s) {
        let day = u32::from_str_radix(s, 10).unwrap();
        let date = fix_day(day, now);
        return Cow::Owned(format!("{}-{}-{}", date.year(), date.month(), date.day()));
    }

    let month_day = Regex::new(r"^(?<month>[0-1]?[0-9])\-(?<day>[0-3]?[0-9])$")
        .expect("Failure compiling month regex");
    if let Some(caps) = month_day.captures(s) {
        let day = u32::from_str_radix(&caps["day"], 10)
            .expect(&format!("`{s}` is not a valid `month-day`"));
        let month = u32::from_str_radix(&caps["month"], 10)
            .expect(&format!("`{s}` is not a valid `month-day`"));

        let date = if month < now.month() {
            let diff = now.month() - month;

            now.checked_add_months(Months::new(12 - diff))
                .expect(&format!("Adding a year to `{now}` blew up"))
        } else {
            let month_diff = month - now.month();
            let d = now
                .checked_add_months(Months::new(month_diff))
                .expect(&format!("Adding `{month_diff}` to `{now}` blew up"));
            fix_day(day, &d)
        };

        return Cow::Owned(format!("{}-{}-{}", date.year(), date.month(), date.day()));
    }

    Cow::Borrowed(s)
}

fn format_date(date: &DateTime<FixedOffset>) -> String {
    date.format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

fn format_utc_date(date: &DateTime<Utc>) -> String {
    date.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveTime, TimeZone};

    use crate::parse_time_with_ref;

    fn ref_date() -> (DateTime<Local>, NaiveTime) {
        let def_time = NaiveTime::from_hms_opt(12, 00, 00).unwrap();
        let now = Local
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2024, 06, 27)
                    .unwrap()
                    .and_time(def_time),
            )
            .unwrap();
        (now, def_time)
    }

    #[test]
    fn test_month_in_the_past() {
        let (now, def_time) = ref_date();
        let r = parse_time_with_ref("05-27", now.clone(), &def_time).unwrap();
        assert_eq!(r.year(), 2025);
        assert_eq!(r.day(), 27);
        assert_eq!(r.month(), 5);
        let r = parse_time_with_ref("04-27", now.clone(), &def_time).unwrap();
        assert_eq!(r.year(), 2025);
        assert_eq!(r.day(), 27);
        assert_eq!(r.month(), 4);
    }

    #[test]
    fn test_month_in_the_future() {
        let (now, def_time) = ref_date();
        let r = parse_time_with_ref("07-27", now.clone(), &def_time).unwrap();
        assert_eq!(r.year(), 2024);
        assert_eq!(r.day(), 27);
        assert_eq!(r.month(), 7);
    }

    #[test]
    fn test_day_in_the_past() {
        let (now, def_time) = ref_date();
        let r = parse_time_with_ref("26", now.clone(), &def_time).unwrap();
        dbg!(&r);
        assert_eq!(r.year(), 2024);
        assert_eq!(r.month(), 7);
        assert_eq!(r.day(), 26)
    }

    #[test]
    fn test_day_in_the_future() {
        let (now, def_time) = ref_date();
        let r = parse_time_with_ref("28", now.clone(), &def_time).unwrap();
        dbg!(&r);
        assert_eq!(r.year(), 2024);
        assert_eq!(r.month(), 6);
        assert_eq!(r.day(), 28)
    }
}
