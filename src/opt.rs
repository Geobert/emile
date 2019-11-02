use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "emile", about = "A workflow companion for zola.")]
pub enum Opt {
    /// Create a new post in drafts folder, with current date prepended to filename.
    New {
        /// Title of the blog post. Needs to be around quotes.
        title: String,
    },
    /// Mark a post as not draft, move it to `posts` folder, set the `date` field in front, build
    /// the website, commit the changes and push them back
    Publish {
        /// Slug part of the file name
        slug: String,
    },
    /// Called by the git webhook. Performs update of the blog repo, and check last log commit
    /// message for commands:{n}
    /// * `blog_build`: build the blog{n}
    /// * `blog_sched "at-format-date" post-slug: schedule the post{n}
    /// * `blog_cancel post-slug`: cancel a previously scheduled post
    GitHook {},
    /// Schedule a post. Ex: `emile schedule "tomorrow" my-post-slug`.
    Schedule {
        /// date to publish the post, needs to be in `at` format and around quotes
        date: String,
        /// slug part of the file name
        slug: String,
    },
    /// Cancel a schedule for the post with `slug`
    Unschedule {
        /// slug part of the file name
        slug: String,
    },
}
