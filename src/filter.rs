// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use regex::Regex;
use std::process::exit;
use super::Args;
use super::node::Handler;
use super::record::Level;
use super::record::Record;

pub struct Filter {
    level: Level,
    tag: Vec<Regex>,
    msg: Vec<Regex>,
}

impl Handler<Record> for Filter {
    fn new(args: Args) -> Box<Self> {
        let build = |f: &Vec<String>| {
            f.iter().map(|e| Regex::new(e).unwrap_or_else(|_| exit(0))).collect::<Vec<Regex>>()
        };

        Box::new(Filter {
            level: args.level,
            tag: build(&args.tag_filter),
            msg: build(&args.msg_filter),
        })
    }

    fn handle(&mut self, record: Record) -> Option<Record> {
        if record.level < self.level {
            return None;
        }
        if !self.is_match(&record.tag, &self.tag) {
            return None;
        }
        if !self.is_match(&record.message, &self.msg) {
            return None;
        }
        Some(record)
    }
}

impl Filter {
    fn is_match(&self, t: &str, regex: &[Regex]) -> bool {
        for m in regex {
            if m.is_match(t) {
                return true;
            }
        }
        regex.is_empty()
    }
}
