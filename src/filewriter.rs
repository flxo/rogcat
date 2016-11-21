// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use std::fs::File;
use std::io::Write;
use super::Args;
use super::node::Handler;
use super::record::Record;

pub struct FileWriter {
    file: File,
}

impl Handler<Record> for FileWriter {
    fn new(args: Args) -> Box<Self> {
        Box::new(FileWriter {
            file: File::create(args.output.unwrap()).unwrap_or_else(|e| {
                println!("Failed to open {}", e);
                ::std::process::exit(0)
            }),
        })
    }

    fn handle(&mut self, record: Record) -> Option<Record> {
        let timestamp: String = ::time::strftime("%m-%d %H:%M:%S.%f", &record.timestamp).unwrap();
        let line = format!("\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\r\n",
                           timestamp,
                           record.tag,
                           record.process,
                           record.thread,
                           record.level,
                           record.message);
        match self.file.write(&line.into_bytes()) {
            Ok(_) => (),
            Err(e) => panic!("{}", e),
        }
        None
    }
}
