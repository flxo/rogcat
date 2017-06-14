// Copyright © 2017 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use errors::*;
use futures::Stream;
use std::io::BufReader;
use std::process::{Command, Stdio};
use super::adb;
use term_painter::{Color, ToStyle};
use terminal::*;
use tokio_core::reactor::Core;
use tokio_io::io::lines;
use tokio_process::CommandExt;

pub fn devices(core: &mut Core) -> Result<i32> {
    let mut child = Command::new(adb()?).arg("devices")
        .stdout(Stdio::piped())
        .spawn_async(&core.handle())?;
    let stdout = child.stdout()
        .take()
        .ok_or("Failed to read stdout of adb")?;
    let reader = BufReader::new(stdout);
    let lines = lines(reader);
    let result = lines.skip(1).for_each(|l| {
        if !l.is_empty() && !l.starts_with("* daemon") {
            let mut s = l.split_whitespace();
            let id: &str = s.next().unwrap_or("unknown");
            let name: &str = s.next().unwrap_or("unknown");
            println!("{} {}",
                     DIMM_COLOR.paint(id),
                     match name {
                         "unauthorized" => Color::Red.paint(name),
                         _ => Color::Green.paint(name),
                     })
        }
        Ok(())
    });

    core.run(result).map_err(|e| e.into()).map(|_| 0)
}
