use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "emile", about = "A workflow helper for zola.")]
pub enum Opt {
    /// Create a new post in drafts folder, with current date prepended to filename.
    New {
        /// Title of the blog post
        title: String,
    },
    /// Mark a post not draft anymore, move it to `posts` folder and build the website.
    Publish {
        /// Slug part of the file name
        slug: String,
    },
    /// Called by the git webhook
    GitHook {},
    // Schedule { date },
    Unschedule {
        slug: String,
    },
}
