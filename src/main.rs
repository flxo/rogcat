use anyhow::{Context, Result};
use clap::Parser;
use std::process::Stdio;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    pin,
    process::Command,
    select,
    signal::ctrl_c,
};

mod parser;
mod record;
mod terminal;

#[derive(Debug, Parser)]
#[clap(version, author)]
struct Opts {
    /// Command to execute
    #[clap(short, long)]
    command: Option<String>,

    /// Logd buffers
    #[clap(short, long, conflicts_with = "command")]
    buffer: Vec<String>,

    /// Restart command on exit
    #[clap(short, long)]
    restart: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let opt: Opts = Opts::parse();
    let mut terminal = terminal::Terminal::new();

    dbg!(&opt);

    let cmd = if let Some(ref cmd) = opt.command {
        cmd
    } else {
        "adb logcat"
    };

    'outer: loop {
        let mut cmd = cmd.split_whitespace();
        let bin = cmd
            .next()
            .ok_or_else(|| anyhow::anyhow!("Invalid command"))?;

        // The list of required buffers needs to be passed to logcat with a -b option
        let buffers = opt.buffer.iter().flat_map(|b| ["-b", b.as_str()]);

        // Append the list of -b buffers to the command
        let args = cmd.chain(buffers);

        // The command
        let mut adb = Command::new(bin)
            .args(args)
            .stdout(Stdio::piped())
            .spawn()
            .context("Failed to spawn adb")?;

        let stdout = adb.stdout.take().context("Failed to get stdout")?;
        let mut lines = BufReader::new(stdout).lines();

        let ctrl_c = ctrl_c();
        pin!(ctrl_c);

        loop {
            select! {
                _ = &mut ctrl_c => {
                    adb.kill().await.context("Failed to kill adb")?;
                    adb.wait().await.context("Failed to join adb")?;
                    break 'outer Ok(());
                },
                line = lines.next_line() => {
                    let line = line.context("Failed to read")?;
                    if let Some(line) = line {
                        if let Some(record) = parser::parse(line.trim()) {
                            terminal.print(&record)?;
                        } else {
                            println!("{}", line);
                        }
                    } else {
                        break;
                    }
                },
            }
        }
    }
}
