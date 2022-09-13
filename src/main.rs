use std::io::Write;

use anyhow::{bail, Result};
use clap::Parser;
use config::ConfigBuilder;

mod config;
mod new;
mod opt;
mod post;
mod publish;
mod watcher;

use opt::Opt;

fn main() -> Result<()> {
    let opt = Opt::parse();
    let cfg = ConfigBuilder::get_config();

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
        Opt::Watch { website } => todo!(),
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
