# UNMAINTAINED

I stopped using Zola so I won't work on `emile` anymore. Feel free to fork it :)

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
# emile will take this file to create a draft post by adding `title`, `date` and `draft = true` in the frontmatter 
draft_template = "templates/draft.html"
# Destination for `publish` command.
publish_dest = "content/"
```

## Usage

`emile --help` and `emile <command> --help` to get all the details.

### new

The `new` command takes the title of your new blog post, between quotes:
```
emile new "My new blog post"
```

### reslug

Rename the post's filename according to its title. If the provided path is a file, change
only this file. If it's a directory, `emile` will change files starting with `-`. So you
can mark files to be updated by prepending `-` to them.

### publish

This command takes a `slug` as parameter. It will take the file corresponding to the
`slug` in the `drafts_consumption_dir`, change its date and draft status and move it to
the `publish_dest` directory. The site is not rebuild after that.

```
emile publish my-new-blog-post
```

### schedule

This command needs a `date` and a `slug`. `date` need to be between quotes and in the
[`at`](https://linux.die.net/man/1/at) utility format.

It schedules the post corresponding to the slug to be published and the site updated at
the given `date`.

```
emile schedule "teatime tomorrow" my-new-blog-post
```

### unschedule

This cancel a previous scheduled post by its slug.

```
emile unschedule my-new-blog-post
```

### publish-flow

This does a `publish`, build the website, commit the changes and push them to origin.
It is used by `schedule` command as the configured job will call `publish-flow`.

### git-hook

This command is intended to be called by a git hook for my specific workflow. It may or
may not suit yours. Feel free to fork and adapt to your use case!

It updates the blog repo, and check last log commit message for commands: 
- `blog_build`: build the blog 
- `blog_sched "at-format-date" post-slug`: schedule the post 
- `blog_unsched post-slug`: cancel a previously scheduled post

