// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use regex::Regex;
use std::collections::HashMap;
use std::io::Write;
use super::node::Handler;
use super::record::{Level, Record};
use super::Args;
use termion::color;

const DIMM_COLOR: u8 = 243;

pub struct Terminal<'a> {
    beginning_of: Regex,
    color: bool,
    date_format: (&'a str, usize),
    full_tag: bool,
    process_width: usize,
    tag_timestamps: HashMap<String, ::time::Tm>,
    tag_width: usize,
    thread_width: usize,
    time_diff: bool,
    vovels: Regex,
}

impl<'a> Terminal<'a> {
    fn level_color(level: Level) -> u8 {
        match level {
            Level::Trace | Level::Debug => DIMM_COLOR, // some shade of gray
            Level::Info => 2, // green
            Level::Warn => 3, // yellow
            Level::Error | Level::Fatal | Level::Assert => 1, // red
        }
    }

    fn color(color: u8) -> color::AnsiValue {
        match color {
            // filter some unreadable (on dark background) or nasty colors
            0...1 => color::AnsiValue(color + 2),
            16...21 => color::AnsiValue(color + 6),
            52...55 | 126...129 => color::AnsiValue(color + 4),
            163...165 | 200...201 => color::AnsiValue(color + 3),
            207 => color::AnsiValue(color + 1),
            232...240 => color::AnsiValue(color + 9),
            _ => color::AnsiValue(color),
        }
    }

    fn hashed_color(item: &str) -> color::AnsiValue {
        let c = item.bytes().fold(42, |c, x| c ^ x);
        Self::color(c)
    }

    fn print_seperator(&self) {
        let size = ::termion::terminal_size().unwrap();
        let line = (0..size.0).map(|_| "─").collect::<String>();
        println!("\r{}", line);
    }

    fn print_record(&mut self, record: &Record) {
        let timestamp: String = ::time::strftime(self.date_format.0, &record.timestamp)
            .unwrap()
            .chars()
            .take(self.date_format.1)
            .collect();

        let diff = if self.time_diff {
            if let Some(t) = self.tag_timestamps.get_mut(&record.tag) {
                let diff = (record.timestamp - *t).num_milliseconds() as f32 / 1000.0;
                format!("{:>9}", format!("{:4.3}", diff))
            } else {
                (0..9).map(|_| " ").collect::<String>()
            }
        } else {
            "".to_owned()
        };

        let tag = {
            let t = if self.beginning_of.is_match(&record.message) {
                self.print_seperator();
                &record.message
            } else {
                &record.tag
            };

            if self.full_tag {
                format!("{:>width$}", t, width = self.tag_width)
            } else if t.chars().count() > self.tag_width {
                let mut t = self.vovels.replace_all(t, "");
                if t.chars().count() > self.tag_width {
                    t.truncate(self.tag_width);
                }
                format!("{:>width$}", t, width = self.tag_width)
            } else {
                format!("{:>width$}", t, width = self.tag_width)
            }
        };

        self.process_width = ::std::cmp::max(self.process_width, record.process.chars().count());
        let pid = format!("{:<width$}", record.process, width = self.process_width);
        let tid = if record.thread.is_empty() {
            "".to_owned()
        } else {
            self.thread_width = ::std::cmp::max(self.thread_width, record.thread.chars().count());
            format!(" {:>width$}", record.thread, width = self.thread_width)
        };

        let level = format!(" {} ", record.level);

        let preamble = format!("{} {} {} {} ({}{}) {}",
                               " ",
                               timestamp,
                               diff,
                               tag,
                               pid,
                               tid,
                               level);
        let preamble_width = preamble.chars().count();

        let level_color = color::AnsiValue(Self::level_color(record.level.clone()));

        let preamble = if self.color {
            format!("{} {}{} {} {}{}{} ({}{}{}{}{}) {}{}{}",
                    " ",
                    color::Fg(color::AnsiValue(DIMM_COLOR)),
                    timestamp,
                    diff,
                    color::Fg(Self::hashed_color(&tag)),
                    tag,
                    color::Fg(color::Reset),
                    color::Fg(Self::hashed_color(&pid)),
                    pid,
                    color::Fg(Self::hashed_color(&tid)),
                    tid,
                    color::Fg(color::Reset),
                    color::Bg(level_color),
                    level,
                    color::Bg(color::Reset))
        } else {
            preamble
        };

        let record_length = record.message.chars().count();
        let full_preamble_width = preamble_width + 3;

        let terminal_size = ::termion::terminal_size().unwrap();

        if (preamble_width + record_length) > (terminal_size.0 as usize) {
            let columns = terminal_size.0 as usize;
            let mut m = record.message.clone();
            while !m.is_empty() {
                let chars_left = m.chars().count();
                let (chunk_width, sign) = if chars_left == record_length {
                    (columns - full_preamble_width, "┌")
                } else if chars_left <= (columns - full_preamble_width) {
                    (chars_left, "└")
                } else {
                    (columns - full_preamble_width, "├")
                };

                let chunk: String = m.chars().take(chunk_width).collect();
                let chunk = if self.color {
                    format!("{}{}{}",
                            color::Fg(level_color),
                            chunk,
                            color::Fg(color::Reset))
                } else {
                    chunk
                };

                m = m.chars().skip(chunk_width).collect();
                println!("{} {} {}", preamble, sign, chunk);
            }
        } else if self.color {
            println!("{} {}{}{}",
                   preamble,
                   color::Fg(level_color),
                   record.message,
                   color::Fg(color::Reset));
        } else {
            println!("{} {}", preamble, record.message);
        }

        if !record.tag.is_empty() {
            self.tag_timestamps.insert(record.tag.clone(), record.timestamp);
        }

        ::std::io::stdout().flush().unwrap();
    }
}

impl<'a> Handler<Record> for Terminal<'a> {
    fn new(args: Args) -> Box<Self> {
        let date_format = if args.show_date {
            ("%m-%d %H:%M:%S.%f", 18)
        } else {
            ("%H:%M:%S.%f", 12)
        };

        Box::new(Terminal {
            beginning_of: Regex::new(r"--------- beginning of.*").unwrap(),
            color: args.color,
            date_format: date_format,
            full_tag: args.full_tag,
            process_width: 0,
            tag_timestamps: HashMap::new(),
            tag_width: 20,
            thread_width: 0,
            time_diff: args.time_diff,
            vovels: Regex::new(r"a|e|i|o|u").unwrap(),
        })
    }

    fn handle(&mut self, record: Record) -> Option<Record> {
        self.print_record(&record);
        Some(record)

    }
}
