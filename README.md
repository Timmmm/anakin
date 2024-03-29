# Anakin

This is a tool that runs another command, and kills all the orphans it generates.

When a process is killed on Linux, its children are not automatically killed too. Instead they get reparented to the init process and live on. In many cases this is not what you want.

In particular this was written to handle `gitlab-runner` which runs CI jobs. When a job times out `gitlab-runner` only kills the process that it started. Any child processes of that may be orphaned and continue running.

Anakin uses Linux's `PR_SET_CHILD_SUBREAPER` feature to mark itself as the "child subreaper", which means any descendants that are orphaned get reparented to this process, instead of the init process. This process then periodically polls its children and kills any that have been orphaned to it.

## Installation

The easiest way to install this is with `pip`:

    python3 -m pip install anakin2

Or if you have Rust:

    cargo install anakin

## Usage

There are no arguments. Just prefix your command with `anakin`.

Instead of

    my_program --some --args

Run

    anakin my_program --some --args

## Logging

Logging is controlled by the following environment variables:

* `ANAKIN_LOG`, e.g. `ANAKIN_LOG=info` will print when orphans are killed. The default level is `error`.
* `ANAKIN_LOG_STYLE` controls the colour output. It can be `auto` (default), `always` or `never`.
* `ANAKIN_LOG_FILE` if set logs to that filename, plus the process ID. Otherwise it logs to stderr.
