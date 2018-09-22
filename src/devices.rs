// Copyright © 2017 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use adb;
use failure::{err_msg, Error};
use futures::Stream;
use std::io::BufReader;
use std::process::{Command, Stdio};
use tokio_core::reactor::Core;
use tokio_io::io::lines;
use tokio_process::CommandExt;

pub fn devices(core: &mut Core) -> Result<i32, Error> {
    let mut child = Command::new(adb()?)
        .arg("devices")
        .stdout(Stdio::piped())
        .spawn_async()?;
    let stdout = child
        .stdout()
        .take()
        .ok_or_else(|| err_msg("Failed to read stdout of adb"))?;
    let reader = BufReader::new(stdout);
    let lines = lines(reader);
    let result = lines.skip(1).for_each(|l| {
        if !l.is_empty() && !l.starts_with("* daemon") {
            let mut s = l.split_whitespace();
            let id: &str = s.next().unwrap_or("unknown");
            let name: &str = s.next().unwrap_or("unknown");
            println!("{} {}", id, name);
        }
        Ok(())
    });

    core.run(result)
        .map_err(|e| format_err!("{}", e))
        .map(|_| 0)
}
