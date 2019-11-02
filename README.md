# Emile

A workflow companion for [Zola](https://getzola.org).

## Build

`cargo build --release`

## Prerequisites

For the all the command but `new`, emile relies on the presence of `git` command line
client and `at` utility to be installed and in the `PATH`.

## Installation

You need to make `emile` findable in the `PATH` as well.

## Configuration

In the blog's folder, you can have a `emile.toml` file to tweak different input/output
behaviours (default values shown): 

```toml
# drafts created with `new` command will end here. Path relative to root of the blog.
drafts_creation_dir = "content/drafts/"
# drafts published with `publish` command will be picked up from here. Path relative to root of the blog.
drafts_consumption_dir = "content/drafts/"
# emile will add this amount of year to the drafts to push it to the top of the list
drafts_year_shift = 0
# emile will take this file from `template` folder to create a draft post by adding `title`, `date` and `draft = true` in the frontmatter 
draft_template = "templates/draft.html"
# Destination for `publish` command.
publish_dest = "content/"
```

## Usage

```
emile 0.1.0
A workflow companion for zola.

USAGE:
    emile <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    git-hook      Called by the git webhook. Performs update of the blog repo, and check last log commit message for
                  commands:
                   * `blog_build`: build the blog
                   * `blog_sched "at-format-date" post-slug: schedule the post
                   * `blog_unsched post-slug`: cancel a previously scheduled post
    help          Prints this message or the help of the given subcommand(s)
    new           Create a new post in drafts folder, with current date prepended to filename.
    publish       Mark a post as not draft, move it to `posts` folder, set the `date` field in front, build the
                  website, commit the changes and push them back
    schedule      Schedule a post. Ex: `emile schedule "tomorrow" my-post-slug`.
    unschedule    Cancel a schedule for the post with `slug`
```

### new

The `new` command takes the title of your new blog post, between quotes:
```
emile new "My new blog post"
```

### publish

This command takes a `slug` as parameter. It will take the file corresponding to the
`slug` in the `drafts_consumption_dir`, change its date and draft status and move it to
the `publish_dest` directory.

Then it calls `zola build`, commit the change and push them to `origin`.

```
emile publish my-new-blog-post
```

### schedule

This command needs a `date` and a `slug`. `date` need to be between quotes and in the
[`at`](https://linux.die.net/man/1/at) utility format.

It schedule the post corresponding to the slug to be `publish`ed at the given `date`.

```
emile schedule "teatime tomorrow" my-new-blog-post
```

### unschedule

This cancel a previous scheduled post by its slug.

```
emile unschedule my-new-blog-post
```

### git-hook

This command is intended to be called by a git hook for my specific workflow. It may or
may not suit yours. Feel free to fork and adapt to your use case!

It updates the blog repo, and check last log commit message for commands: 
- `blog_build`: build the blog 
- `blog_sched "at-format-date" post-slug`: schedule the post 
- `blog_unsched post-slug`: cancel a previously scheduled post

