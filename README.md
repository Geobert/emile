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
timezone = 0
# number of seconds to wait before processing filesystem changes events
debouncing = 2

# Section to activate posting a toot on mastodon on publish
[mastodon]
# host of the mastodon instance of your account
instance = "mastodon.social"
# file in /template to use as the toot’s template
social_template = "mastodon.txt"
# if a tag match, use the associated lang
tag_lang = [{ tag = "english", lang = "en" }]
# tag in the list will not be in the toot
filtered_tag = ["english", "mon avis"]
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

## Mastodon support

When a post is published, it is possible to publish a toot on Mastodon.

You need to set the environment variable `EMILE_MASTODON_TOKEN` with the access token from 
the development section and 

Add the `[mastodon]` section with needed info:

- `instance`: The host of the instance where the account used to post is
- `social_template`: The file in `/template` to use for the toot’s template. Default: 
  `mastodon.txt`. In case of multiple languages, a suffix with the iso code can be added, 
  ex: `mastodon.en.txt`
- `tag_lang`: if a tag match, the associated lang will be put as the lang of the toot and 
  the template with the lang suffix will be used
- `filtered_tag`: the matching tags will not be included in the toot.

### Toot template

The template system is very rude and is a simple text replace supporting:
- `{title}`: the title of the post
- `{link}`: the link to the post
- `{tags}`: the tags of the post, filtered tags are not included, and if `#rust` is found, 
  `#RustLang` is added


