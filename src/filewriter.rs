// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use clap::ArgMatches;
use std::fs::File;
use std::io::Write;

pub struct FileWriter {
    file: File,
}

impl FileWriter {
    pub fn new(args: &ArgMatches) -> FileWriter {
        FileWriter {
            file: match File::create(args.value_of("file").unwrap()) {
                Ok(f) => f,
                Err(e) => panic!(e),
            },
        }
    }
}

impl super::Sink for FileWriter {
    fn process(&mut self, message: &super::Record) {
        let line = format!("{}\r\n", message.to_csv());
        match self.file.write(&line.into_bytes()) {
            Ok(_) => (),
            Err(e) => panic!("{}", e),
        }
    }

    fn close(&self) {}
}
