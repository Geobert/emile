# Emile

A workflow companion for [Zola](https://getzola.org).

## Features
* `watch` over the blog directory to:
  * rebuild site on modification
  * schedule posts
  * publish on social media (Mastodon and/or Bluesky) when a scheduled post is live 
    * templating of the post to push
    * add links to social media posts in the published blog article
* `new` command to create a new post using a predefined template into a specified dir
* `schedule` command to set the wanted publication date of a post
* `publish` command to change state of a draft to publish (deprecated)

## Build

You need to have [Rust](https://www.rust-lang.org/) installed, then:

`cargo build --release`

## Prerequisites

`emile` relies on the presence of [`zola`](https://getzola.org) to be installed and in the 
`PATH` on the server and your workstation.

## Installation

You need to make `emile` findable in the `PATH` as well, both on the server hosting the
blog and your workstation.

If using social media feature, you need to make sure that `emile` has access to the web.

## Configuration

In the blog's folder, you can have an `emile.toml` file to tweak different input/output
behaviours (default values shown): 

```toml
# drafts created with `new` command will end here. Path relative to root of the blog.
drafts_creation_dir = "content/drafts/"

# on `new`, emile will add this amount of year to the draft post.
# It’s a dirty hack to push drafts to the top of the list on the blog homepage.
drafts_year_shift = 0

# emile will take this file to create a draft post by adding `title`, `date` and 
#`draft = true` in the frontmatter 
draft_template = "draft.txt"

# Destination for `publish` command.
publish_dest = "content/"

# Scheduling directory, used by `watch` command
schedule_dir = "content/drafts/scheduled/"

# Default time for `schedule` command, if only a date is given
default_sch_time = "12:00:00"

# Timezone relative to UTC you're writing the post in, affect `publish` and `schedule` 
# commands
timezone = 0

# for `watch` command. number of seconds to wait before processing filesystem changes 
# events
debouncing = 2

# Section to activate posting on social media
[social]
# file in /template to use as the toot’s template
social_template = "social.txt"

# file in /template to use as the snippet to replace `link_tag` in the blog post
link_template = "social_link.txt"

# tag to put in the blog post, to be replaced by the `link_template` snippet to have link 
# to social media post
link_tag = "{$ emile_social $}"

# if a tag match, use the associated lang (ex: [{ tag = "english", lang = "en" }])
tag_lang = []

# tag in the list will not be in the social post (ex: ["english", "misc"])
filtered_tag = []

# social instances to post to. One per `api` (accepted values are "mastodon" or "bluesky"). 
#`*_var` are environment variable to read the needed value from. If `social` is present, 
# it cannot be empty
# ex: 
# { server = "mastodon.social", api = "mastodon", token_var = "EMILE_MASTODON_TOKEN" }, 
# { server = "bsky.social", api = "bluesky", handle_var = "EMILE_BLUESKY_ID", token_var = "EMILE_BLUESKY_PWD" }
instances = []
```

## Usage

This is how I use `emile`. On the server hosting the blog, I launch `emile` in watcher
mode (see `Commands` below) so it will catch changes that need a rebuild of the site, and
it will monitor the `schedule_dir` and parse the files inside to schedule their
publication. 

On my desktop, I use `new` and `schedule` to create and put articles to be published.

To synchronize my desktop with the server, I use `unison`, available on all platforms (but
any sync tools will do), and the watcher `emile` above takes care of everything. 

I admit that I don’t use `publish`, it is here for historical reasons and might be removed
in the future.

## Commands

`emile help` and `emile help <command>` to get all the details.

### new

The `new` command takes the title of your new blog post, between quotes:
```
emile new "My new blog post"
```

This will create a file in the `drafts_creation_dir` directory, using slugified version of
the title as the file’s name, current date + `drafts_year_shift` years in the `date`
field, using `draft_template` file as the template.

### publish (deprecated)

This command takes a file path as parameter. It will change its date to current date 
and draft status and move it to the `publish_dest` directory. Note that the site is not 
rebuild.

Since the `watch` command, the need for `publish` doesn’t seem obvious and might be
removed in the future.

```
emile publish ./content/drafts/my_new_blog_post.md
```

### schedule

This will move the given file to `schedule_dir` and change the frontmatter `date` field.
If only a date is given, `default_sch_time` will be used. The date can be provided in the
formats supported by [human-date-parser](https://github.com/technologicalmayhem/human-date-parser).

A few examples:
```
emile schedule now ./content/drafts/my_new_blog_post.md
emile schedule tomorrow ./content/drafts/my_new_blog_post.md
emile schedule "2024-06-27" ./content/drafts/my_new_blog_post.md # this uses `default_sch_time`
emile schedule "06-27" ./content/drafts/my_new_blog_post.md # this is completed with current year and `default_sch_time`
emile schedule "27" ./content/drafts/my_new_blog_post.md # this is completed with current year, month and `default_sch_time`
emile schedule "14:13" ./content/drafts/my_new_blog_post.md # this is completed with current day or next one if the hour is past
```

### watch

This command will put `emile` in watcher mode, waiting for modifications in the blog.

On modification in the `schedule_dir`, it will schedule the post in it according to the
frontmatter’s `date` field.

On modification in `/content/posts` or anywhere not `draft_creation_dir` it will rebuild
the blog.

## Social media support

When a post is published, it is possible to publish a post on social media. Currently,
Mastodon and Bluesky are supported. You need to configure the `social` section (see
`Configuration` above). 

### Social post template

The template system is very rude and is a simple text replace supporting:
- `{title}`: the title of the post
- `{link}`: the link to the post
- `{tags}`: the tags of the post, filtered tags are not included, and if `#rust` is found, 
  `#RustLang` is added

ex:
```
New post!
“{title}” 
{link}
{tags}
```

You need at least one template file in the `/template` directory, with the name specified
in `social_template`.

### Social media link

`emile` can add links to the social media posts it created so people can react on your
article. The tag defined by `link_tag` will be replaced by the expanded template
`link_template`.

The file specified in `link_template` must be in the `/template` directory. It must
contains one `{links}` (plural) tag which will be expanded to a list of links to the
social media posts.

ex:
```
 
---

React on {links}.
```

### Multilingual templates

You can add a social template of a different language by adding `.lang` before `.txt` in
the file’s name.

ex: `social.fr.txt` and `social_link.fr.txt` 

