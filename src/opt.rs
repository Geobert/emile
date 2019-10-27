use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "emile", about = "A workflow helper around zola.")]
pub enum Opt {
    /// Create a new post in drafts folder, with current date prepended to filename.
    New {
        /// Title of the blog post
        title: String,
    },
    /// Mark a post not draft anymore, move it to `posts` folder and build the website.
    Publish {
        /// Title of the blog post
        title: String,
    },
    // Schedule { date },
    // Unschedule { filename: String },
}
