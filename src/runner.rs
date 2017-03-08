// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use errors::*;
use futures::{future, Future};
use kabuki::Actor;
use std::io::{BufReader, BufRead};
use std::process::{ChildStdout, ChildStderr, Command, Stdio};
use super::Message;
use super::RFuture;
use super::record::Record;
use super::terminal::DIMM_COLOR;
use term_painter::ToStyle;

pub struct Runner {
    _stderr: BufReader<ChildStderr>,
    cmd: Vec<String>,
    last_line: Option<String>,
    restart: bool,
    skip_until: Option<String>,
    stdout: BufReader<ChildStdout>,
}

impl Runner {
    pub fn new(cmd: String, restart: bool) -> Result<Self> {
        let cmd = cmd.split_whitespace()
            .map(|s| s.to_owned())
            .collect::<Vec<String>>();
        let (stderr, stdout) = Self::run(&cmd)?;

        Ok(Runner {
            _stderr: BufReader::new(stderr),
            cmd: cmd,
            skip_until: None,
            last_line: None,
            restart: restart,
            stdout: BufReader::new(stdout),
        })
    }

    fn run(cmd: &Vec<String>) -> Result<(ChildStderr, ChildStdout)> {
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
}

impl Actor for Runner {
    type Request = ();
    type Response = Message;
    type Error = Error;
    type Future = RFuture<Message>;

    fn call(&mut self, _: ()) -> Self::Future {
        loop {
            let mut buffer = Vec::new();
            match self.stdout.read_until(b'\n', &mut buffer) {
                Ok(s) => {
                    if s > 0 {
                        let record = Record {
                            timestamp: Some(::time::now()),
                            raw: String::from_utf8_lossy(&buffer).trim().to_string(),
                            ..Default::default()
                        };
                        self.last_line = Some(record.raw.clone());

                        let skip_line = self.skip_until.clone();
                        let r = match skip_line {
                            Some(ref sl) => {
                                if *sl == *record.raw {
                                    self.skip_until = None;
                                }
                                Message::Drop
                            },
                            None => Message::Record(record),
                        };
                        return future::ok(r).boxed();
                    } else {
                        if self.restart {
                            self.skip_until = self.last_line.clone();
                            let text = format!("Restarting \"{}\"", self.cmd.join(" "));
                            println!("{}", DIMM_COLOR.paint(&text));
                            match Runner::run(&self.cmd) {
                                Ok((stderr, stdout)) => {
                                    self._stderr = BufReader::new(stderr);
                                    self.stdout = BufReader::new(stdout);
                                }
                                Err(e) => return future::err(e.into()).boxed(),
                            }
                        } else {
                            return future::ok(Message::Done).boxed()
                        }
                    }
                }
                Err(e) => return future::err(e.into()).boxed(),
            }
        }
    }
}

#[test]
fn runner() {
    assert!(Runner::new("true".to_owned(), false).is_ok());
    assert!(Runner::new("echo test".to_owned(), false).is_ok());
}
