use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// A workflow companion for zola (https://getzola.org)
#[derive(Debug, Parser)]
#[command(about, version)]
pub struct Opt {
    /// Log directory
    #[arg(short, long, value_name = "DIR")]
    pub log_dir: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Create a new post in drafts folder, with current date prefiled in the frontmatter.
    /// The date can be modified with the `drafts_year_shift` configuration key
    #[command(visible_alias = "n")]
    New {
        /// Title of the blog post. Needs to be around quotes.
        title: String,
    },
    /// Mark a post as not draft, move it to `posts` folder, set the `date` field in front. It must
    /// be in the draft folder
    #[command(visible_alias = "p")]
    Publish {
        /// Path to the post to publish
        post: PathBuf,
    },
    /// Launch watcher mode to manage scheduling and publication dynamically
    #[command(visible_alias = "w")]
    Watch {
        /// Path to the website to watch.
        website: PathBuf,
    },
    /// Schedule a post
    #[command(visible_alias = "s")]
    Schedule {
        /// When to publish the post. Can be relative to `now` ("tomorrow", "+3 days", "next week"),
        /// or absolute ("2024-06-27") (See the https://github.com/uutils/parse_datetime crate
        /// for supported formats)
        time: String,
        /// Path to the post to publish
        post: PathBuf,
    },
}
