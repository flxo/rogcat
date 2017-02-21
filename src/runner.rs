// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use errors::*;
use std::io::{BufReader, BufRead};
use std::process::{ChildStdout, Command, Stdio};
use super::node::Node;
use super::record::Record;

pub struct Runner {
    cmd: Vec<String>,
    restart: bool,
    stdout: BufReader<ChildStdout>,
}

impl Runner {
    fn run(c: &Vec<String>) -> Result<BufReader<ChildStdout>> {
        Command::new(&c[0])
            .args(&c[1..])
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Cannot run \"{}\": {}", c[0], e))?
            .stdout
            .map(|s| BufReader::new(s))
            .ok_or(format!("Cannot open stdout of \"{}\"", c[0]).into())
    }
}

impl Node<Record, (Vec<String>, bool)> for Runner {
    fn new(args: (Vec<String>, bool)) -> Result<Box<Self>> {
        let stdout = Runner::run(&args.0)?;

        Ok(Box::new(Runner {
            cmd: args.0,
            restart: args.1,
            stdout: stdout,
        }))
    }

    fn start(&mut self, send: &Fn(Record), done: &Fn()) -> Result<()> {
        loop {
            loop {
                let mut buffer: Vec<u8> = vec![];
                if self.stdout.read_until(b'\n', &mut buffer).map_err(|e| format!("{}", e))? > 0 {
                    send(Record {
                        timestamp: Some(::time::now()),
                        raw: String::from_utf8_lossy(&buffer).trim().to_string(),
                        ..Default::default()
                    });
                } else {
                    break;
                }
            }

            if self.restart {
                self.stdout = Self::run(&self.cmd)?;
            } else {
                done();
                break;
            }
        }
        Ok(())
    }
}


#[test]
fn runner() {
    assert!(Runner::new((vec!["true".to_string()], false)).is_ok());
    assert!(Runner::new((vec!["echo".to_string(), "test".to_string()], false)).is_ok());
}
