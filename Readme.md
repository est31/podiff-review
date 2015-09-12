# Podiff-review

A tool to review `gettext` po language file commits, written in rust.

Podiff-review is a small program that helps one to check git-based translation changes for vandalism, using the Microsoft Translator API.

*Note: this is my first non-hello-world program in rust, so don't look at the source :).*

If everything is set up, you only have to do
```bash
cargo run commit-id
```
and then podiff-review asks you for each changed translation whether it is acceptable or not.

## Setup

Before running `podiff-review`, you need to set up some things first.
First, you'll need API keys for the Microsoft Translator API ([Walkthrough on how to set it up](http://blogs.msdn.com/b/translation/p/gettingstarted1.aspx) ).
Microsoft gives one 2 million translated chars for free, so don't worry, you don't have to pay for "normal" amounts of translations to review.

After having obtained API keys, you should create a `settings.toml` file in the directory you want to run `podiff-review` in, with the following content:

```toml
# The repo whose commits to review
repo = "/path/to/git/repo"

# Language to translate to

translate-to = "en"

# Microsoft translator related settings

ms-auth-secret = "<client secret here>"
ms-client-id = "<client id here>"

```

## Run

In order to run `podiff-review`, just type the following command, provided you have rust installed:

```bash
cargo run commit-id
```

The `commit-id` is the usual git hash of the commit to review in git.

The tool will then ask you about translation changes. You can answer with `y` for Ok, `n` for not Ok, and `l` for "I want to look at it **l**ater".
It automatically puts answered questions into `answers.toml`, for later inspection.

`podiff-review` will display whether a commit is regarded as "approved", and provide stats about how many lines failed.
