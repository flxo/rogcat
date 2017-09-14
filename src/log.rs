// Copyright © 2017 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use clap::ArgMatches;
use errors::*;
use futures::{Future, Async, AsyncSink, Sink, Poll, StartSend};
use std::process::{Command, Stdio};
use super::adb;
use reader::stdin_reader;
use record::{Level, Record};
use tokio_core::reactor::{Core, Handle};
use tokio_process::CommandExt;

struct Logger {
    handle: Handle,
    tag: String,
    level: Level,
}

impl Logger {
    fn level(level: &Level) -> &str {
        match level {
            &Level::Trace | &Level::Verbose => "v",
            &Level::Debug | &Level::None => "d",
            &Level::Info => "i",
            &Level::Warn => "w",
            &Level::Error | &Level::Fatal | &Level::Assert => "e",
        }
    }
}

impl Sink for Logger {
    type SinkItem = Option<Record>;
    type SinkError = Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        if let Some(r) = item {
            let child = Command::new(adb()?)
                .arg("shell")
                .arg("log")
                .arg("-p")
                .arg(Self::level(&self.level))
                .arg("-t")
                .arg(format!("\"{}\"", &self.tag))
                .arg(&r.raw)
                .stdout(Stdio::piped())
                .output_async(&self.handle)
                .map(|_| ())
                .map_err(|_| ());
            self.handle.spawn(child);
        }
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}

pub fn run(args: &ArgMatches, core: &mut Core) -> Result<i32> {
    let message = args.value_of("MESSAGE").unwrap_or("");
    let tag = args.value_of("tag").unwrap_or("Rogcat").to_owned();
    let level = Level::from(args.value_of("level").unwrap_or(""));
    match message {
        "-" => {
            let sink = Logger {
                handle: core.handle(),
                tag,
                level,
            };

            let input = stdin_reader(core)?;
            let stream = sink.send_all(input);
            core.run(stream)
                .map_err(|_| "Failed to run \"adb shell log\"".into())
                .map(|_| 0)
        }
        _ => {
            let child = Command::new(adb()?)
                .arg("shell")
                .arg("log")
                .arg("-p")
                .arg(&Logger::level(&level))
                .arg("-t")
                .arg(&tag)
                .arg(format!("\"{}\"", message))
                .stdout(Stdio::piped())
                .output_async(&core.handle())
                .map(|_| ())
                .map_err(|_| ());
            core.run(child)
                .map_err(|_| "Failed to run \"adb shell log\"".into())
                .map(|_| 0)
        }
    }
}
