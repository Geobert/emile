use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[clap(
    name = "emile",
    about = "A workflow companion for zola (https://getzola.org)"
)]
pub enum Opt {
    /// Create a new post in drafts folder, with current date prefiled in the frontmatter.
    /// The date can be modified with the `drafts_year_shift` configuration key
    New {
        /// Title of the blog post. Needs to be around quotes.
        title: String,
    },
    /// Mark a post as not draft, move it to `posts` folder, set the `date` field in front.
    Publish {
        /// Slug part of the file name
        slug: String,
    },
    /// Launch watcher mode to manage scheduling and publication dynamically
    Watch {
        /// Path to the website to watch.
        website: PathBuf,
    },
}
