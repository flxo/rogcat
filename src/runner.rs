// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use errors::*;
use futures::{Async, Poll, Stream};
use std::io::{BufReader, BufRead};
use std::process::{ChildStdout, ChildStderr, Command, Stdio};
use super::Message;
use super::record::Record;
use super::terminal::DIMM_COLOR;
use term_painter::ToStyle;
use std::sync::mpsc::{channel, Sender, Receiver};

use futures::Future;
use futures_cpupool::CpuPool;

pub struct Runner<'a> {
    _stderr: BufReader<ChildStderr>,
    cmd: Vec<String>,
    pool: &'a CpuPool,
    //last_line: Option<String>,
    restart: bool,
    //skip_on_restart: bool,
    //skip_until: Option<String>,
    stdout: BufReader<ChildStdout>,
    rx: Receiver<Message>,
    tx: Sender<Message>,
}

impl<'a> Runner<'a> {
    pub fn new(cmd: String, restart: bool, _skip_on_restart: bool, pool: &'a CpuPool) -> Result<Self> {
        let cmd = cmd.split_whitespace()
            .map(|s| s.to_owned())
            .collect::<Vec<String>>();
        let (stderr, stdout) = Self::run(&cmd)?;

        let (tx, rx) = channel();

        Ok(Runner {
               _stderr: BufReader::new(stderr),
               cmd: cmd,
               pool: pool,
               //skip_until: None,
               //last_line: None,
               restart: restart,
               //skip_on_restart: skip_on_restart,
               stdout: BufReader::new(stdout),
               rx: rx,
               tx: tx,
           })
    }

    fn run(cmd: &[String]) -> Result<(ChildStderr, ChildStdout)> {
        if cmd.is_empty() {
            Err("Invalid cmd".into())
        } else {
            let c = Command::new(&cmd[0]).args(&cmd[1..])
                .stderr(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;
            Ok((c.stderr.ok_or("Failed to open stderr")?, c.stdout.ok_or("Failed to open stdout")?))
        }
    }

    fn read(&mut self) -> Result<Message> {
        let mut buffer = Vec::new();
        self.stdout.read_until(b'\n', &mut buffer)
            .and_then(|s| {
                if s > 0 {
                    let record = Record {
                        timestamp: Some(::time::now()),
                        raw: String::from_utf8_lossy(&buffer).trim().to_string(),
                        ..Default::default()
                    };
                    Ok(Message::Record(record))
                } else {
                    Ok(Message::Done)
                }
            }).map_err(|e| e.into())
    }
}

impl<'a> Stream for Runner<'a> {
    type Item = Message;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.read().and_then(|message| {
            match message {
                Message::Done => {
                    if self.restart {
                        let text = format!("Restarting \"{}\"", self.cmd.join(" "));
                        println!("{}", DIMM_COLOR.paint(&text));
                        let (stderr, stdout) = Self::run(&self.cmd)?;
                        self._stderr = BufReader::new(stderr);
                        self.stdout = BufReader::new(stdout);
                        match self.read() {
                            Ok(Message::Record(r)) => Ok(Async::Ready(Some(Message::Record(r)))),
                            // TODO: make this nice
                            _ => Err(format!("Command {:?} exited without any output", &self.cmd).into()),
                        }
                    } else {
                        Ok(Async::Ready(None))
                    }
                }
                Message::Drop => Ok(Async::Ready(None)),
                Message::Record(r) => Ok(Async::Ready(Some(Message::Record(r)))),
            }
        })
    }
}

#[test]
fn runner() {
    assert!(Runner::new("true".to_owned(), false, false).is_ok());
    assert!(Runner::new("echo test".to_owned(), false, false).is_ok());
}
