// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use ansi_term::Colour::*;
use ansi_term::Style;
use regex::Regex;
use std::collections::HashMap;
use std::hash::{Hash, SipHasher, Hasher};
use std::io::Write;
use std::sync::mpsc::Sender;
use std::sync::mpsc::channel;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;

const DIMM_COLOR: ::ansi_term::Colour = Fixed(243);
const STATUS_INTERVAL: u64 = 1;

enum Event {
    Clear,
    Exit,
    Record(super::Record),
    Seperator,
    Status,
}

pub struct TerminalImpl<'a> {
    beginning_of: Regex,
    color: bool,
    date_format: (&'a str, usize),
    full_tag: bool,
    process_width: usize,
    seperator: bool,
    status: String,
    status_len: usize,
    tag_timestamps: HashMap<String, ::time::Tm>,
    tag_width: usize,
    thread_width: usize,
    time_diff: bool,
    vovels: Regex,
}

pub struct Terminal {
    tx: Sender<Event>,
}

impl Terminal {
    pub fn new(configuration: &super::Configuration) -> Terminal {
        // Store terminal settings here - will be restored on drop
        let stdout = ::std::io::stdout().into_raw_mode().unwrap();

        print!("{}", ::termion::cursor::Hide);
        ::std::io::stdout().flush().unwrap();

        let (tx, rx) = channel();

        // Issue a Status event every STATUS_INTERVAL
        let sender = tx.clone();
        ::std::thread::spawn(move || {
            loop {
                sender.send(Event::Status).ok();
                ::std::thread::sleep(::std::time::Duration::new(STATUS_INTERVAL, 0));
            }
        });

        // Handle key events in this thread
        let sender = tx.clone();
        ::std::thread::spawn(move || {
            loop {
                let stdin = ::std::io::stdin();
                for c in stdin.keys() {
                    if let Ok(c) = c {
                        match c {
                            Key::Char('q') | Key::Ctrl('c') => {
                                sender.send(Event::Exit).ok();
                            }
                            Key::Char('\n') | Key::Char(' ') => {
                                sender.send(Event::Seperator).ok();
                            }
                            Key::Ctrl('l') => {
                                sender.send(Event::Clear).ok();
                            }
                            _ => {}
                        }
                    }
                }
            }
        });


        let date_format = if configuration.show_date {
            ("%m-%d %H:%M:%S.%f", 18)
        } else {
            ("%H:%M:%S.%f", 12)
        };
        let status_text = format!("{} {}", configuration.command, configuration.args.join(" "));
        let mut terminal = TerminalImpl {
            beginning_of: Regex::new(r"--------- beginning of.*").unwrap(),
            color: configuration.color,
            date_format: date_format,
            full_tag: configuration.full_tag,
            process_width: 0,
            seperator: false,
            status: DIMM_COLOR.paint(status_text.clone()).to_string(),
            status_len: status_text.len(),
            tag_timestamps: HashMap::new(),
            tag_width: 20,
            thread_width: 0,
            time_diff: configuration.time_diff,
            vovels: Regex::new(r"a|e|i|o|u").unwrap(),
        };

        ::std::thread::spawn(move || {
            loop {
                let e = rx.recv().unwrap();
                match e {
                    Event::Clear => {
                        terminal.reset_seperator();
                        print!("{}", ::termion::clear::All);
                        ::std::io::stdout().flush().ok();
                    }
                    Event::Exit => {
                        print!("{}", ::termion::cursor::Show);
                        ::std::io::stdout().flush().ok();
                        drop(stdout);
                        ::std::process::exit(0);
                    }
                    Event::Record(record) => {
                        terminal.reset_seperator();
                        terminal.print_record(&record);
                    }
                    Event::Seperator => {
                        if !terminal.seperator {
                            terminal.print_seperator();
                        }
                    }
                    Event::Status => {
                        terminal.print_status();
                    }
                }
            }
        });

        Terminal { tx: tx }
    }
}

impl<'a> TerminalImpl<'a> {
    fn reset_seperator(&mut self) {
        self.seperator = false;
    }

    fn level_color(&self, level: &super::Level) -> ::ansi_term::Colour {
        match *level {
            super::Level::Trace | super::Level::Debug => DIMM_COLOR,
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

    fn print_status(&self) {
        let size = ::termion::terminal_size().unwrap();

        let now = ::time::now();
        let time: String = ::time::strftime(self.date_format.0, &now)
                    .unwrap()
                    .chars()
                    .take(self.date_format.1 - 4) // XXX strip millis
                    .collect();
        let time = DIMM_COLOR.paint(time);
        print!("{}\r  {}{}{}",
               ::termion::clear::CurrentLine,
               time,
               ::termion::cursor::Goto(size.0 - 1 - self.status_len as u16, size.1),
               self.status);
        ::std::io::stdout().flush().unwrap();
    }

    fn print_seperator(&mut self) {
        self.seperator = true;
        let size = ::termion::terminal_size().unwrap();
        let line =
            ::ansi_term::Colour::Yellow.paint((0..size.0).map(|_| "─").collect::<String>());
        print!("{}\r{}\r\n", ::termion::clear::CurrentLine, line);
    }

    fn print_record(&mut self, record: &super::Record) {
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
            } else {
                if t.chars().count() > self.tag_width {
                    let mut t = self.vovels.replace_all(&t, "");
                    if t.chars().count() > self.tag_width {
                        t.truncate(self.tag_width);
                    }
                    format!("{:>width$}", t, width = self.tag_width)
                } else {
                    format!("{:>width$}", t, width = self.tag_width)
                }
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


        let preamble = if self.color {
            let level_color = self.level_color(&record.level);
            format!("{} {} {} {} ({}{}) {}",
                    " ",
                    level_color.paint(timestamp).to_string(),
                    level_color.paint(diff).to_string(),
                    self.hashed_color(&tag).paint(tag),
                    self.hashed_color(&pid).paint(pid),
                    self.hashed_color(&tid).paint(tid),
                    Style::new().on(level_color).paint(Black.paint(level).to_string()))
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
                    let msg_color = self.level_color(&record.level);
                    msg_color.paint(chunk).to_string()
                } else {
                    chunk
                };

                m = m.chars().skip(chunk_width).collect();
                print!("{}\r{} {} {}\r\n",
                       ::termion::clear::CurrentLine,
                       preamble,
                       sign,
                       chunk);
            }
        } else {
            if self.color {
                let color = self.level_color(&record.level);
                let msg = &record.message;
                let msg = color.paint(msg.clone());
                print!("{}\r{} {}\r\n",
                       ::termion::clear::CurrentLine,
                       preamble,
                       msg);
            } else {
                print!("{}\r{} {}\r\n",
                       ::termion::clear::CurrentLine,
                       preamble,
                       record.message);
            }
        }

        if !record.tag.is_empty() {
            self.tag_timestamps.insert(record.tag.clone(), record.timestamp);
        }

        ::std::io::stdout().flush().unwrap();
    }
}

impl super::Sink for Terminal {
    fn open(&self) {}
    fn close(&self) {}

    fn process(&mut self, record: &super::Record) {
        let r = (*record).clone();
        self.tx.send(Event::Record(r)).ok();
    }
}
