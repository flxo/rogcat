// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use clap::ArgMatches;
use errors::*;
use futures::{future, Future};
use kabuki::Actor;
use regex::Regex;
use std::cmp::max;
use std::collections::HashMap;
use std::io::Write;
use std::io::stdout;
use std::str::FromStr;
use super::Message;
use super::record::{Level, Record};
use term_painter::Attr::*;
use term_painter::{Color, ToStyle};
use terminal_size::{Width, Height, terminal_size};
use time::Tm;
use super::Format;
use super::RFuture;

#[cfg(not(target_os = "windows"))]
pub const DIMM_COLOR: Color = Color::Custom(243);
#[cfg(target_os = "windows")]
pub const DIMM_COLOR: Color = Color::White;

pub struct Terminal {
    beginning_of: Regex,
    color: bool,
    date_format: (String, usize),
    diff_width: usize,
    format: Format,
    process_width: usize,
    shorten_tag: bool,
    tag_timestamps: HashMap<String, Tm>,
    tag_width: usize,
    thread_width: usize,
    time_diff: bool,
    vovels: Regex,
}

impl<'a> Terminal {
    pub fn new(args: &ArgMatches<'a>) -> Result<Self> {
        Ok(Terminal {
            beginning_of: Regex::new(r"--------- beginning of.*").unwrap(),
            color: true, // ! args.is_present("NO-COLOR"),
            date_format: if args.is_present("show-date") {
                ("%m-%d %H:%M:%S.%f".to_owned(), 18)
            } else {
                ("%H:%M:%S.%f".to_owned(), 12)
            },
            format: args.value_of("terminal-format")
                .and_then(|f| Format::from_str(f).ok())
                .unwrap_or(Format::Human),
            shorten_tag: args.is_present("shorten-tags"),
            process_width: 0,
            tag_timestamps: HashMap::new(),
            vovels: Regex::new(r"a|e|i|o|u").unwrap(),
            tag_width: 20,
            thread_width: 0,
            diff_width: if args.is_present("show-time-diff") {
                8
            } else {
                0
            },
            time_diff: args.is_present("show-time-diff"),
        })
    }

    /// Filter some unreadable (on dark background) or nasty colors
    fn hashed_color(item: &str) -> Color {
        match item.bytes().fold(42u16, |c, x| c ^ x as u16) {
            c @ 0...1 => Color::Custom(c + 2),
            c @ 16...21 => Color::Custom(c + 6),
            c @ 52...55 | c @ 126...129 => Color::Custom(c + 4),
            c @ 163...165 | c @ 200...201 => Color::Custom(c + 3),
            c @ 207 => Color::Custom(c + 1),
            c @ 232...240 => Color::Custom(c + 9),
            c @ _ => Color::Custom(c),
        }
    }

    fn print_record(&mut self, record: &Record) -> Result<()> {
        match self.format {
            Format::Csv => {
                record.format(Format::Csv)
                    .and_then(|s| {
                        println!("{}", s);
                        Ok(())
                    })
            }
            Format::Human => {
                self.print_human(record);
                Ok(())
            }
            Format::Raw => {
                println!("{}", record.raw);
                Ok(())
            }
        }
    }

    // TODO
    // Rework this to use a more column based approach!
    fn print_human(&mut self, record: &Record) {
        let (timestamp, mut diff) = if let Some(ts) = record.timestamp {
            let timestamp = match ::time::strftime(&self.date_format.0, &ts) {
                Ok(t) => {
                    t.chars()
                        .take(self.date_format.1)
                        .collect::<String>()
                }
                Err(_) => (0..self.date_format.1).map(|_| " ").collect::<String>(),
            };

            let diff = if self.time_diff {
                if let Some(t) = self.tag_timestamps.get(&record.tag) {
                    let diff = ((ts - *t).num_milliseconds()).abs();
                    let diff = format!("{}.{:.03}", diff / 1000, diff % 1000);
                    let diff = if diff.chars().count() <= self.diff_width {
                        diff
                    } else {
                        "-.---".to_owned()
                    };
                    diff
                } else {
                    "".to_owned()
                }
            } else {
                "".to_owned()
            };

            (timestamp, diff)
        } else {
            ("".to_owned(), "".to_owned())
        };

        let tag = {
            let mut t = if self.beginning_of.is_match(&record.message) {
                diff = "".to_owned();
                self.tag_timestamps.clear();
                // Print horizontal line if temrinal width is detectable
                if let Some((Width(width), Height(_))) = terminal_size() {
                    println!("{}", (0..width).map(|_| "─").collect::<String>());
                }
                // "beginnig of" messages never have a tag
                record.message.clone()
            } else {
                record.tag.clone()
            };

            if t.chars().count() > self.tag_width {
                if self.shorten_tag {
                    t = self.vovels.replace_all(&t, "")
                }
                if t.chars().count() > self.tag_width {
                    t.truncate(self.tag_width);
                }
            }
            format!("{:>width$}", t, width = self.tag_width)
        };

        self.process_width = max(self.process_width, record.process.chars().count());
        let pid = format!("{:<width$}", record.process, width = self.process_width);
        let tid = if record.thread.is_empty() {
            "".to_owned()
        } else {
            self.thread_width = max(self.thread_width, record.thread.chars().count());
            format!(" {:>width$}", record.thread, width = self.thread_width)
        };

        let level = format!(" {} ", record.level);
        let level_color = match record.level {
            Level::Trace | Level::Verbose | Level::Debug | Level::None => DIMM_COLOR,
            Level::Info => Color::Green,
            Level::Warn => Color::Yellow,
            Level::Error | Level::Fatal | Level::Assert => Color::Red,
        };

        let color = self.color;
        let tag_width = self.tag_width;
        let diff_width = self.diff_width;
        let timestamp_width = self.date_format.1;
        let print_msg = |chunk: &str, sign: &str| {
            if color {
                println!("{:<timestamp_width$} {:>diff_width$} {:>tag_width$} ({}{}) {} {} {}",
                         DIMM_COLOR.paint(&timestamp),
                         DIMM_COLOR.paint(&diff),
                         Self::hashed_color(&tag).paint(&tag),
                         Self::hashed_color(&pid).paint(&pid),
                         Self::hashed_color(&tid).paint(&tid),
                         Plain.bg(level_color).fg(Color::Black).paint(&level),
                         level_color.paint(sign),
                         level_color.paint(chunk),
                         timestamp_width = timestamp_width,
                         diff_width = diff_width,
                         tag_width = tag_width);

            } else {
                println!("{:<timestamp_width$} {:>diff_width$} {:>tag_width$} ({}{}) {} {} {}",
                         timestamp,
                         diff,
                         tag,
                         pid,
                         tid,
                         level,
                         sign,
                         chunk,
                         timestamp_width = timestamp_width,
                         diff_width = diff_width,
                         tag_width = tag_width);
            }
        };

        if let Some((Width(width), Height(_))) = terminal_size() {
            let preamble_width = timestamp_width + 1 + self.diff_width + 1 + tag_width + 1 +
                                 2 * self.process_width + 2 +
                                 1 + 8 + 1;
            // Windows terminal width reported is too big
            #[cfg(target_os = "windows")]
            let preamble_width = preamble_width + 1;

            let record_len = record.message.chars().count();
            let columns = width as usize;
            if (preamble_width + record_len) > columns {
                let mut m = record.message.clone();
                // TODO: Refactor this!
                while !m.is_empty() {
                    let chars_left = m.chars().count();
                    let (chunk_width, sign) = if chars_left == record_len {
                        (columns - preamble_width, "┌")
                    } else if chars_left <= (columns - preamble_width) {
                        (chars_left, "└")
                    } else {
                        (columns - preamble_width, "├")
                    };

                    let chunk: String = m.chars().take(chunk_width).collect();
                    m = m.chars().skip(chunk_width).collect();
                    if self.color {
                        let c = level_color.paint(chunk).to_string();
                        print_msg(&c, sign)
                    } else {
                        print_msg(&chunk, sign)
                    }
                }
            } else {
                print_msg(&record.message, " ");
            }
        } else {
            print_msg(&record.message, " ");
        };

        if let Some(ts) = record.timestamp {
            if self.time_diff && !record.tag.is_empty() {
                self.tag_timestamps.insert(record.tag.clone(), ts);
            }
        }

        stdout().flush().unwrap();
    }
}

impl<'a> Actor for Terminal {
    type Request = Message;
    type Response = Message;
    type Error = Error;
    type Future = RFuture<Message>;

    fn call(&mut self, message: Message) -> Self::Future {
        if let Message::Record(ref record) = message {
            match self.print_record(record) {
                Ok(_) => (),
                Err(e) => return future::err(e.into()).boxed(),
            }
        }
        future::ok(message).boxed()
    }
}
