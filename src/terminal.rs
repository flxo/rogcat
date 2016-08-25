// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use ansi_term::*;
use ansi_term::Colour::*;
use clap::ArgMatches;
use regex::Regex;
use std::hash::{Hash, SipHasher, Hasher};
use std::io::Write;

#[derive (PartialEq)]
enum Format {
    Csv,
    Human,
}

impl ::std::str::FromStr for Format {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        println!("{}", s);
        match s {
            "csv" => Ok(Format::Csv),
            "human" => Ok(Format::Human),
            _ => Err("invalid format"),
        }
    }
}

pub struct Terminal {
    full_tag: bool,
    format: Format,
    monochrome: bool,
    vovels: Regex,
    beginning_of: Regex,
    tag_width: usize,
    id_width: usize,
}

impl Terminal {
    pub fn new(args: &ArgMatches) -> Terminal {
        Terminal {
            full_tag: args.is_present("DISABLE-TAG-SHORTENING"),
            format: value_t!(args, "format", Format).unwrap_or(Format::Human),
            monochrome: args.is_present("DISABLE_COLOR_OUTPUT"),
            vovels: Regex::new(r"a|e|i|o|u").unwrap(),
            beginning_of: Regex::new(r"--------- beginning of.*").unwrap(),
            tag_width: 30,
            id_width: 0,
        }
    }

    fn level_color(&self, level: &super::Level) -> ::ansi_term::Colour {
        match *level {
            super::Level::Trace | super::Level::Debug => Fixed(243),
            super::Level::Info => Green,
            super::Level::Warn => Yellow,
            super::Level::Error | super::Level::Fatal | super::Level::Assert => Red,
        }
    }

    fn color(&self, color: u8) -> ::ansi_term::Colour {
        match color {
            // filter some unreadable of nasty colors
            0...1 => Fixed(color + 2),
            16...21 => Fixed(color + 6),
            52...55 | 126...129 => Fixed(color + 4),
            163...165 | 200...201 => Fixed(color + 3),
            207 => Fixed(color + 1),
            232...240 => Fixed(color + 9),
            _ => Fixed(color),
        }
    }

    fn hashed_color(&self, item: &str) -> ::ansi_term::Colour {
        let mut hasher = SipHasher::new();
        item.hash(&mut hasher);
        self.color((hasher.finish() % 255) as u8)
    }

    fn columns() -> usize {
        match ::term_size::dimensions() {
            Some(d) => d.0,
            None => 80 as usize,
        }
    }

    pub fn print(&mut self, message: &super::message::Message) {
        // for i in 0..254 {
        //    let a = Fixed(i).paint(format!("ASDFASDF ASDFSF {}", i));
        //    println!("{}", a);
        // }
        if self.format == Format::Csv {
            println!("{}", message.to_csv());
            return;
        }

        let timestamp: String = ::time::strftime("%m-%d %H:%M:%S.%f", &message.timestamp)
            .unwrap()
            .chars()
            .take(18)
            .collect();

        let tag = {
            let mut t = if self.beginning_of.is_match(&message.message) {
                message.message.clone()
            } else {
                message.tag.clone()
            };

            if self.full_tag {
                format!("{:>width$}", t, width = self.tag_width)
            } else {
                if t.chars().count() > self.tag_width {
                    self.vovels.replace_all(&t, "");
                    if t.chars().count() > self.tag_width {
                        t.truncate(self.tag_width);
                    }
                }
                format!("{:>width$}", t, width = self.tag_width)
            }
        };

        self.id_width = ::std::cmp::max(self.id_width, message.process.chars().count());
        self.id_width = ::std::cmp::max(self.id_width, message.thread.chars().count());
        let pid = format!("{:<width$}", message.process, width = self.id_width);
        let tid = format!("{:>width$}", message.thread, width = self.id_width);

        let level = format!(" {} ", message.level);

        let preamble = format!("{} {} {} ({} {}) {}", " ", timestamp, tag, pid, tid, level);
        let preamble_width = preamble.chars().count();


        let preamble = if self.monochrome {
            preamble
        } else {
            let level_color = self.level_color(&message.level);
            format!("{} {} {} ({} {}) {}",
                    " ",
                    level_color.paint(timestamp).to_string(),
                    self.hashed_color(&tag).paint(tag),
                    self.hashed_color(&pid).paint(pid),
                    self.hashed_color(&tid).paint(tid),
                    Style::new().on(level_color).paint(Black.paint(level).to_string()))
        };

        let message_length = message.message.chars().count();
        let full_preamble_width = preamble_width + 3;

        let columns = Self::columns();
        if (preamble_width + message_length) > columns {
            let mut m = message.message.clone();
            while !m.is_empty() {
                let chars_left = m.chars().count();
                let (chunk_width, sign) = if chars_left == message_length {
                    (columns - full_preamble_width, "┌")
                } else if chars_left <= (columns - full_preamble_width) {
                    (chars_left, "└")
                } else {
                    (columns - full_preamble_width, "├")
                };

                let chunk: String = m.chars().take(chunk_width).collect();
                m = m.chars().skip(chunk_width).collect();
                println!("{} {} {}", preamble, sign, chunk);
            }
        } else {
            println!("{} {}", preamble, message.message);
        }

        ::std::io::stdout().flush().ok();
    }
}

impl super::Sink for Terminal {
    fn process(&mut self, message: &super::message::Message) {
        self.print(message)
    }

    fn close(&self) {}
}
