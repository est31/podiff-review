# Podiff-review

A tool to review `gettext` po language file commits, written in rust.

Podiff-review is a small program that helps one checking translation changes vandalism, using the Microsoft Translator API to give you translations to give you a general overview.

## Setup

In order to be abled to run `podiff-review`, you need to set up some configs first.
First, you'll need API access to the microsoft translator API.
```toml
repo = "/path/to/git/repo"

# Language to translate to

translate-to = "en"

# Microsoft translator related settings

ms-auth-secret = "<secret here>"
ms-client-id = "<client id here>"

```

## Run

In order to run `podiff-review`, just type the following command, provided you have rust installed:

```bash
cargo run commit-id
```

The `commit-id` is the usual git hash of the commit to review in git.

The tool will then ask you about translation changes. You can answer with `y` for Ok, `n` for not ok, and `l` for "I want to look at it **l**ater". It automatically puts answered questions into answers.toml, for later inspection.
