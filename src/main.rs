use anyhow::Result;
use config::Config;

use structopt::StructOpt;

mod config;
mod new;
mod opt;
mod publish;

use opt::Opt;

fn main() -> Result<()> {
    let opt = Opt::from_args();
    let cfg = Config::get_config();

    match opt {
        Opt::New { title } => new::create_draft(&title, &cfg),
        Opt::Publish { title } => publish::publish_post(&title, &cfg),
    }
}
