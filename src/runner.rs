// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use clap::ArgMatches;
use errors::*;
use futures::{Async, Poll, Stream};
use std::io::{self, BufRead, BufReader};
use std::mem;
use std::process::{Command, Stdio};
use super::record::Record;
use super::terminal::DIMM_COLOR;
use super::{adb, RStream};
use term_painter::ToStyle;
use tokio_core::reactor::Handle;
use tokio_io::AsyncRead;
use tokio_process::{Child, CommandExt};

struct LossyLines<A> {
    io: A,
    buffer: Vec<u8>,
}

fn lossy_lines<A>(a: A) -> LossyLines<A>
where
    A: AsyncRead + BufRead,
{
    LossyLines {
        io: a,
        buffer: Vec::new(),
    }
}

impl<A> Stream for LossyLines<A>
where
    A: AsyncRead + BufRead,
{
    type Item = String;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<String>, io::Error> {
        let n = try_nb!(self.io.read_until(b'\n', &mut self.buffer));
        if n == 0 && self.buffer.is_empty() {
            return Ok(None.into());
        }
        self.buffer.pop();
        let mut s = String::from_utf8_lossy(&self.buffer).into_owned();
        self.buffer.clear();
        Ok(Some(mem::replace(&mut s, String::new())).into())
    }
}

type OutStream = Box<Stream<Item = String, Error = ::std::io::Error>>;

pub struct Runner {
    child: Child,
    cmd: String,
    handle: Handle,
    output: OutStream,
    restart: bool,
}

fn run(cmd: &str, handle: &Handle) -> Result<(Child, OutStream)> {
    let cmd = cmd.split_whitespace()
        .map(|s| s.to_owned())
        .collect::<Vec<String>>();

    let mut child = Command::new(&cmd[0])
        .args(&cmd[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn_async(handle)?;

    let stdout = child.stdout().take().ok_or("Failed get stdout")?;
    let stderr = child.stderr().take().ok_or("Failed get stderr")?;
    let stdout_reader = BufReader::new(stdout);
    let stderr_reader = BufReader::new(stderr);
    let output = Box::new(lossy_lines(stdout_reader).select(
        lossy_lines(stderr_reader),
    ));
    Ok((child, output))
}

pub fn runner<'a>(args: &ArgMatches<'a>, handle: Handle) -> Result<RStream> {
    let adb = format!("{}", adb()?.display());
    let (cmd, restart) = value_t!(args, "COMMAND", String)
        .map(|s| (s, args.is_present("restart")))
        .unwrap_or({
            let mut logcat_args = vec![];

            let mut restart = args.is_present("restart");
            if !restart {
                restart = ::config_get::<bool>("restart").unwrap_or(true);
            }

            if args.is_present("tail") {
                let count = value_t!(args, "tail", u32).unwrap_or_else(|e| e.exit());
                logcat_args.push(format!("-t {}", count));
                restart = false;
            };

            if args.is_present("dump") {
                logcat_args.push("-d".to_owned());
                restart = false;
            }

            let buffer = ::config_get::<Vec<String>>("buffer")
                .unwrap_or_else(|| vec!["all".to_owned()])
                .join(" -b ");

            let cmd = format!("{} logcat -b {} {}", adb, buffer, logcat_args.join(" "));
            (cmd, restart)
        });
    let (child, output) = run(&cmd, &handle)?;

    Ok(Box::new(Runner {
        child,
        cmd: cmd.trim().to_owned(),
        handle,
        output,
        restart,
    }))
}

impl Stream for Runner {
    type Item = Option<Record>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            match self.output.poll() {
                Ok(Async::Ready(t)) => {
                    if let Some(s) = t {
                        let r = Some(Record {
                            raw: s,
                            ..Default::default()
                        });
                        return Ok(Async::Ready(Some(r)));
                    } else if self.restart {
                        let text = format!("Restarting \"{}\"", self.cmd);
                        println!("{}", DIMM_COLOR.paint(&text));
                        let (child, output) = run(&self.cmd, &self.handle)?;
                        self.output = output;
                        self.child = child;
                    } else {
                        return Ok(Async::Ready(Some(None)));
                    }
                }
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(e) => return Err(e.into()),
            }
        }
    }
}
