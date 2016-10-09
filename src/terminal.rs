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
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use std::sync::{Arc, Mutex};
use std::io::Write;

pub struct Terminal {
    full_tag: bool,
    monochrome: bool,
    vovels: Regex,
    beginning_of: Regex,
    tag_width: usize,
    process_width: usize,
    thread_width: usize,
    shutdown: Arc<Mutex<bool>>,
}

impl Terminal {
    pub fn new(args: &ArgMatches) -> Terminal {
        let shutdown = Arc::new(Mutex::new(false));
        let stdout = ::std::io::stdout().into_raw_mode().unwrap();

        print!("{}", ::termion::cursor::Hide);
        ::std::io::stdout().flush().unwrap();

        let b = shutdown.clone();
        ::std::thread::spawn(move || {
            loop {
                let stdin = ::std::io::stdin();
                for c in stdin.keys() {
                    if let Ok(c) = c {
                        match c {
                            Key::Char('q') | Key::Ctrl('c') => {
                                let _l = b.lock();
                                print!("{}", ::termion::cursor::Show);
                                ::std::io::stdout().flush().unwrap();
                                drop(stdout);
                                ::std::process::exit(0);
                            }
                            Key::Char('\n') | Key::Char(' ') => print!("\r\n"),
                            Key::Ctrl('l') => {
                                print!("{}", ::termion::clear::All);
                                ::std::io::stdout().flush().unwrap();
                            }
                            _ => {}
                        }
                    }
                }
            }
        });

        Terminal {
            full_tag: args.is_present("DISABLE-TAG-SHORTENING"),
            monochrome: args.is_present("DISABLE_COLOR_OUTPUT"),
            vovels: Regex::new(r"a|e|i|o|u").unwrap(),
            beginning_of: Regex::new(r"--------- beginning of.*").unwrap(),
            tag_width: 30,
            process_width: 0,
            thread_width: 0,
            shutdown: shutdown,
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
            // filter some unreadable or nasty colors
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

    pub fn print(&mut self, record: &super::Record) {
        // for i in 0..254 {
        //    let a = Fixed(i).paint(format!("ASDFASDF ASDFSF {}", i));
        //    println!("{}", a);
        // }
        let _l = self.shutdown.lock();

        let timestamp: String = ::time::strftime("%m-%d %H:%M:%S.%f", &record.timestamp)
            .unwrap()
            .chars()
            .take(18)
            .collect();

        let tag = {
            let mut t = if self.beginning_of.is_match(&record.message) {
                record.message.clone()
            } else {
                record.tag.clone()
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

        self.process_width = ::std::cmp::max(self.process_width, record.process.chars().count());
        let pid = format!("{:<width$}", record.process, width = self.process_width);
        let tid = if record.thread.is_empty() {
            "".to_owned()
        } else {
            self.thread_width = ::std::cmp::max(self.thread_width, record.thread.chars().count());
            format!("{:>width$}", record.thread, width = self.thread_width)
        };

        let level = format!(" {} ", record.level);

        let preamble = format!("{} {} {} ({}{}) {}", " ", timestamp, tag, pid, tid, level);
        let preamble_width = preamble.chars().count();


        let preamble = if self.monochrome {
            preamble
        } else {
            let level_color = self.level_color(&record.level);
            format!("{} {} {} ({}{}) {}",
                    " ",
                    level_color.paint(timestamp).to_string(),
                    self.hashed_color(&tag).paint(tag),
                    self.hashed_color(&pid).paint(pid),
                    self.hashed_color(&tid).paint(tid),
                    Style::new().on(level_color).paint(Black.paint(level).to_string()))
        };

        let record_length = record.message.chars().count();
        let full_preamble_width = preamble_width + 3;

        let terminal_size = if let Ok(s) = ::termion::terminal_size() {
            Some(s)
        } else {
            None
        };

        if terminal_size.is_some() &&
           ((preamble_width + record_length) > (terminal_size.unwrap().0 as usize)) {
            let columns = terminal_size.unwrap().0 as usize;
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
                let chunk = if self.monochrome {
                    chunk
                } else {
                    let msg_color = self.level_color(&record.level);
                    msg_color.paint(chunk).to_string()
                };

                m = m.chars().skip(chunk_width).collect();
                print!("\r\n{} {} {}", preamble, sign, chunk);
            }
        } else {
            if self.monochrome {
                print!("\r\n{} {}", preamble, record.message);
            } else {
                let color = self.level_color(&record.level);
                let msg = &record.message;
                let msg = color.paint(msg.clone());
                print!("\r\n{} {}", preamble, msg);
            }
        }

        ::std::io::stdout().flush().unwrap();
    }
}

impl super::Sink for Terminal {
    fn open(&self) {}
    fn close(&self) {}

    fn process(&mut self, record: &super::Record) {
        self.print(record)
    }

}
