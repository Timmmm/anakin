use anyhow::{anyhow, bail, Context, Result};
use log::{error, info};
use nix::{sys::signal, unistd::Pid};
use std::process::ExitCode;
use tokio::{
    fs::{DirEntry, File},
    io::{self, AsyncBufReadExt},
    process::Command,
    select,
    time::{sleep, Duration},
};

// Single threaded async Rust is used so we don't have to deal with Linux
// commands that confusingly display threads the same as processes (since
// that's how they're implemented in Linux). For example pstree.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<ExitCode> {
    // Set up logging. ANAKIN_LOG controls the log level. ANAKIN_LOG_STYLE
    // controls colour output.
    let env = env_logger::Env::new()
        .filter("ANAKIN_LOG")
        .write_style("ANAKIN_LOG_STYLE");

    let mut builder = env_logger::Builder::from_env(env);

    // Optionally output to a file instead of stdout. Our process ID is apprended.
    if let Ok(log_file) = std::env::var("ANAKIN_LOG_FILE") {
        let filename = format!("{log_file}.{}", std::process::id());
        let target = Box::new(
            std::fs::File::create(&filename)
                .with_context(|| anyhow!("opening output log file '{filename}'"))?,
        );
        builder.target(env_logger::Target::Pipe(target));
    }
    builder.init();

    nix::sys::prctl::set_child_subreaper(true).context("setting child subreaper")?;

    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        bail!("No command provided");
    }

    let mut command = Command::new(&args[0]);
    command.args(&args[1..]);

    let mut child = command.spawn().context("spawning subprocess")?;

    let child_id = child.id().ok_or(anyhow!("error getting child PID"))?;

    let reaper = kill_children_forever(child_id);
    let child_wait = child.wait();

    let exit_code = select! {
        () = reaper => { unreachable!() },
        res = child_wait => { res?.code().unwrap_or(1) },
    };

    // Final cleanup of orphans. Don't kill process 0 (which shouldn't exist).
    kill_children(0).await?;

    Ok(ExitCode::from(exit_code as u8))
}

/// Loop forever killing all direct children except the given process.
async fn kill_children_forever(except: u32) {
    loop {
        if let Err(e) = kill_children(except).await {
            error!("{e}");
        }
        sleep(Duration::from_millis(1000)).await;
    }
}

/// Loop, killing all the children except the given process ID.
async fn kill_children(except: u32) -> Result<()> {
    /// Process a /proc/??? entry.
    async fn process_entry(entry: DirEntry, my_pid: u32, except: u32) -> Result<()> {
        // Check if the entry is a directory and represents a process ID
        if !entry.file_type().await?.is_dir() {
            return Ok(());
        }

        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();

        if let Ok(pid) = file_name.parse::<u32>() {
            if pid == except {
                return Ok(());
            }
            // Read the stat file for the process
            let stat_file = match File::open(format!("/proc/{file_name}/stat")).await {
                // Ignore file not found errors which can occur due to races with processes exiting.
                Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                other => other,
            }
            .with_context(|| anyhow!("opening /proc/{file_name}/stat"))?;
            let mut reader = io::BufReader::new(stat_file);
            let mut buf = String::new();
            reader
                .read_line(&mut buf)
                .await
                .with_context(|| anyhow!("reading /proc/{file_name}/stat"))?;

            // Extract the parent process ID from the stat file
            let parent_pid: Option<u32> =
                buf.split_whitespace().nth(3).and_then(|s| s.parse().ok());

            // Check if it's a child process and not the exception
            if let Some(parent_pid) = parent_pid {
                if parent_pid == my_pid {
                    // Kill the child process
                    info!(
                        "killing orphan {pid}: {}",
                        get_command_line(pid).unwrap_or("?".to_string())
                    );
                    // Get its command line.
                    signal::kill(Pid::from_raw(pid as i32), signal::SIGKILL)
                        .context("sending kill signal to process")?;
                }
            }
        }
        Ok(())
    }

    let my_pid = std::process::id();

    // Open the directory containing process information
    let mut entries = tokio::fs::read_dir("/proc")
        .await
        .context("reading /proc")?;

    // Iterate over each entry in the directory
    while let Some(entry) = entries.next_entry().await.context("reading dir entry")? {
        if let Err(e) = process_entry(entry, my_pid, except).await {
            error!("{e}");
        }
    }

    Ok(())
}

fn get_command_line(pid: u32) -> Result<String> {
    let cmdline = std::fs::read_to_string(format!("/proc/{pid}/cmdline"))?;
    Ok(cmdline
        .split_terminator('\0')
        .map(|s| bash_quote(s))
        .collect::<Vec<String>>()
        .join(" "))
}

/// Quote a bash string using single quotes or double quotes, unless it only
/// contains characters are definitely safe. If quoted with single quotes then
/// no character needs to be escaped, but you can't make a single quote.
/// If quoted with double quotes, only the following characters need to be
/// escaped: $, `, \, ! (and ").
fn bash_quote(s: &str) -> String {
    // Do we need to quote at all?
    if s.chars()
        .all(|c| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '-' | '_' | '/' | '.' | '='))
    {
        s.to_string()
    } else {
        // Can we single quote?
        if !s.contains('\'') {
            format!("'{s}'")
        } else {
            // Double quote.
            let mut escaped = String::with_capacity(s.len() * 2);
            escaped.push('"');
            for c in s.chars() {
                if matches!(c, '$' | '`' | '\\' | '!' | '"') {
                    escaped.push('\\');
                }
                escaped.push(c);
            }
            escaped.push('"');
            escaped
        }
    }
}
