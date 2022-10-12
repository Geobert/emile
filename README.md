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
# emile will add this amount of year to the drafts to push it to the top of the list
drafts_year_shift = 0
# emile will take this file to create a draft post by adding `title`, `date` and `draft = true` in the frontmatter 
draft_template = "templates/draft.html"
# Destination for `publish` command.
publish_dest = "content/"
# Scheduling directory (see below)
schedule_dir = "content/drafts/scheduled/"
# Timezone relative to UTC you're writing the post in
timezone = 1
```

## Usage

`emile --help` and `emile <command> --help` to get all the details.

### new

The `new` command takes the title of your new blog post, between quotes:
```
emile new "My new blog post"
```

### publish

This command takes a `slug` as parameter. It will take the file corresponding to the
`slug` in the `drafts_consumption_dir`, change its date and draft status and move it to
the `publish_dest` directory. The site is not rebuild after that.

```
emile publish my-new-blog-post
```

### watch

This command will put `emile` in watcher mode, waiting for modifications in the the blog.

On modification in the `schedule_dir`, it will schedule the post in it according to the
date in the frontmatter of the post.

On modification in `/content/posts` or anywhere not `draft_creation_dir` it will rebuild
the blog.