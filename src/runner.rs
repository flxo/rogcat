// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use errors::*;
use futures::{Async, Poll, Stream};
use std::io::BufReader;
use std::process::{Command, Stdio};
use super::Message;
use super::record::Record;
use super::terminal::DIMM_COLOR;
use term_painter::ToStyle;
use tokio_core::reactor::Handle;
use tokio_io::io::{lines, Lines};
use tokio_process::{Child, ChildStdout, CommandExt};

type StdoutLines = Lines<BufReader<ChildStdout>>;

pub struct Runner {
    child: Child,
    cmd: String,
    handle: Handle,
    lines: StdoutLines,
    restart: bool,
}

impl Runner {
    pub fn new(handle: Handle, cmd: String, restart: bool, _skip_on_restart: bool) -> Result<Self> {
        let (child, lines) = Self::run(&cmd, &handle)?;
        Ok(Runner {
               child: child,
               cmd: cmd.clone(),
               handle: handle,
               lines: lines,
               restart: restart,
           })
    }

    fn run(cmd: &str, handle: &Handle) -> Result<(Child, StdoutLines)> {
        let cmd = cmd.split_whitespace()
            .map(|s| s.to_owned())
            .collect::<Vec<String>>();

        let mut child = Command::new(&cmd[0]).args(&cmd[1..])
            .stdout(Stdio::piped())
            .spawn_async(&handle)?;

        let stdout = child.stdout().take().unwrap();
        let reader = BufReader::new(stdout);
        Ok((child, lines(reader)))
    }
}

impl Stream for Runner {
    type Item = Message;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            match self.lines.poll() {
                Ok(Async::Ready(t)) => {
                    if let Some(s) = t {
                        let r = Record {
                            raw: s,
                            ..Default::default()
                        };
                        return Ok(Async::Ready(Some(Message::Record(r))));
                    } else {
                        if self.restart {
                            let text = format!("Restarting \"{}\"", self.cmd);
                            println!("{}", DIMM_COLOR.paint(&text));
                            let (child, lines) = Self::run(&self.cmd, &self.handle)?;
                            self.lines = lines;
                            self.child = child;
                        } else {
                            return Ok(Async::Ready(Some(Message::Done)));
                        }
                    }
                }
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(e) => return Err(e.into()),
            }
        }
    }
}
