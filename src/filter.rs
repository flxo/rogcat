// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.


// TODO: rewrite this

use super::node::Node;
use super::record::{Level, Record};

pub struct Filter {
    level: Level, /* tag: Vec<Regex>,
                   * msg: Vec<Regex>, */
}

impl Node<Record, ()> for Filter {
    fn new(_: ()) -> Result<Box<Self>, String> {
        // let build = |f: &Vec<String>| {
        //     f.iter().map(|e| Regex::new(e).unwrap_or_else(|_| exit(0))).collect::<Vec<Regex>>()
        // };

        Ok(Box::new(Filter { level: Level::None }))
    }

    fn message(&mut self, record: Record) -> Result<Option<Record>, String> {
        if record.level < self.level {
            return Ok(None);
        }
        // if !self.is_match(&record.tag, &self.tag) {
        //     return None;
        // }
        // if !self.is_match(&record.message, &self.msg) {
        //     return None;
        // }
        Ok(Some(record))
    }
}

// impl Filter {
//     fn is_match(&self, t: &str, regex: &[Regex]) -> bool {
//         for m in regex {
//             if m.is_match(t) {
//                 return true;
//             }
//         }
//         regex.is_empty()
//     }
// }
