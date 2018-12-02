// Copyright © 2016 Felix Obenhuber
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use crate::profiles::*;
use crate::record::{Format, Level, Record};
use crate::utils::config_get;
use crate::utils::terminal_width;
use crate::LogSink;
use ansi_term::Color;
use atty::Stream;
use bytes::{BufMut, BytesMut};
use clap::{values_t, ArgMatches};
use console::measure_text_width;
use failure::Error;
use futures::{Async, AsyncSink, Poll, Sink, StartSend};
use regex::Regex;
use std::cmp::max;
use std::cmp::min;
use std::io::Write;
use std::str::FromStr;
use tokio::io::stdout;

#[cfg(not(target_os = "windows"))]
pub const DIMM_COLOR: Color = Color::Fixed(243);
#[cfg(target_os = "windows")]
pub const DIMM_COLOR: Color = WHITE;

pub fn with_args<'a>(args: &ArgMatches<'a>, profile: &Profile) -> Box<LogSink> {
    let terminal = Terminal::new(args, profile)
        .sink_map_err(|e| failure::format_err!("Terminal error: {}", e));
    Box::new(terminal)
}

#[cfg(target_os = "windows")]
fn hashed_color(i: &str) -> Color {
    i.bytes().fold(42u32, |c, x| (c ^ Color::from(x))) % 15 + 1
}

#[cfg(not(target_os = "windows"))]
fn hashed_color(i: &str) -> Color {
    // Some colors are hard to read on (at least) dark terminals
    // and I consider some others as ugly.
    let c = match i.bytes().fold(42u8, |c, x| (c ^ x)) {
        c @ 0...1 => c + 2,
        c @ 16...21 => c + 6,
        c @ 52...55 | c @ 126...129 => c + 4,
        c @ 163...165 | c @ 200...201 => c + 3,
        c @ 207 => c + 1,
        c @ 232...240 => c + 9,
        c => c,
    };

    Color::Fixed(c)
}

struct Terminal {
    color: bool,
    date_format: Option<(&'static str, usize)>,
    format: Format,
    highlight: Vec<Regex>,
    process_width: usize,
    tag_width: Option<usize>,
    thread_width: usize,
    dimm_color: Color,
    buffer: BytesMut,
}

impl<'a> Terminal {
    pub fn new(args: &ArgMatches<'a>, profile: &Profile) -> Terminal {
        let mut hl = profile.highlight.clone();
        if args.is_present("highlight") {
            hl.extend(values_t!(args.values_of("highlight"), String).unwrap());
        }
        let highlight = hl.iter().flat_map(|h| Regex::new(h)).collect();

        let format = args
            .value_of("format")
            .and_then(|f| Format::from_str(f).ok())
            .unwrap_or(Format::Human);
        if format == Format::Html {
            panic!("HTML format is unsupported when writing to files");
        }

        let color = {
            match args
                .value_of("color")
                .unwrap_or_else(|| config_get("terminal_color").unwrap_or_else(|| "auto"))
            {
                "always" => true,
                "never" => false,
                "auto" | _ => atty::is(Stream::Stdout),
            }
        };
        let no_dimm = args.is_present("no_dimm") || config_get("terminal_no_dimm").unwrap_or(false);
        let tag_width = config_get("terminal_tag_width");
        let hide_timestamp = args.is_present("hide_timestamp")
            || config_get("terminal_hide_timestamp").unwrap_or(false);
        let show_date =
            args.is_present("show_date") || config_get("terminal_show_date").unwrap_or(false);
        let date_format = if show_date {
            if hide_timestamp {
                Some(("%m-%d", 5))
            } else {
                Some(("%m-%d %H:%M:%S.%f", 12 + 1 + 5))
            }
        } else if hide_timestamp {
            None
        } else {
            Some(("%H:%M:%S.%f", 12))
        };

        Terminal {
            color,
            dimm_color: if no_dimm { Color::White } else { DIMM_COLOR },
            format,
            highlight,
            date_format,
            tag_width,
            process_width: 0,
            thread_width: 0,
            buffer: BytesMut::with_capacity(1024),
        }
    }

    // Dynamic tag width estimation according to terminal width
    fn tag_width(&self) -> usize {
        let terminal_width = terminal_width();
        self.tag_width.unwrap_or_else(|| match terminal_width {
            Some(n) if n <= 80 => 15,
            Some(n) if n <= 90 => 20,
            Some(n) if n <= 100 => 25,
            Some(n) if n <= 110 => 30,
            _ => 35,
        })
    }

    fn buffer_human(&mut self, record: &Record) {
        let empty = String::new();
        let timestamp = if let Some((format, len)) = self.date_format {
            if let Some(ref ts) = record.timestamp {
                let mut ts = time::strftime(format, &ts).expect("Date format error");
                ts.truncate(len);
                ts
            } else {
                " ".repeat(len)
            }
        } else {
            empty
        };

        let tag_width = self.tag_width();
        let tag_chars = record.tag.chars().count();
        let tag = format!(
            "{:>width$}",
            record
                .tag
                .chars()
                .take(min(tag_width, tag_chars))
                .collect::<String>(),
            width = tag_width
        );

        self.process_width = max(self.process_width, record.process.chars().count());
        let pid = if record.process.is_empty() {
            " ".repeat(self.process_width)
        } else {
            format!("{:<width$}", record.process, width = self.process_width)
        };
        self.thread_width = max(self.thread_width, record.thread.chars().count());
        let tid = format!(" {:>width$}", record.thread, width = self.thread_width);

        let level = format!(" {} ", record.level);

        let (preamble, level_color) = if self.color {
            let highlight = !self.highlight.is_empty()
                && (self.highlight.iter().any(|r| r.is_match(&record.tag))
                    || self.highlight.iter().any(|r| r.is_match(&record.message)));
            let timestamp_color = if highlight {
                Color::Yellow
            } else {
                self.dimm_color
            };
            let timestamp = timestamp_color.paint(timestamp);
            let tag_color = hashed_color(&record.tag);
            let tag = tag_color.paint(tag);
            let pid_color = hashed_color(&pid);
            let pid = pid_color.paint(pid);
            let tid_color = hashed_color(&tid);
            let tid = tid_color.paint(tid);
            let level_color = match record.level {
                Level::Trace | Level::Verbose | Level::Debug | Level::None => self.dimm_color,
                Level::Info => Color::Green,
                Level::Warn => Color::Yellow,
                Level::Error | Level::Fatal | Level::Assert => Color::Red,
            };
            let level = Color::White.on(level_color).paint(level);
            let preamble = format!(
                "{timestamp} {tag} ({pid}{tid}) {level} ",
                timestamp = timestamp,
                tag = tag,
                pid = pid,
                tid = tid,
                level = level,
            );
            (preamble, Some(level_color))
        } else {
            let preamble = format!(
                "{timestamp} {tag} ({pid}{tid}) {level} ",
                timestamp = timestamp,
                tag = tag,
                pid = pid,
                tid = tid,
                level = level,
            );
            (preamble, None)
        };

        let preamble_width = measure_text_width(&preamble) + 3;
        let payload_len = terminal_width().unwrap_or(std::usize::MAX) - preamble_width;
        let message = &record.message;

        let message_len = message.chars().count();
        let chunks = message_len / payload_len + 1;

        for i in 0..chunks {
            self.buffer.extend(preamble.as_bytes());

            let c = if chunks == 1 {
                " "
            } else if i == 0 {
                "┌"
            } else if i == chunks - 1 {
                "└"
            } else {
                "├"
            };
            let begin = i * payload_len;
            let chunk_len = min(payload_len, message_len);

            self.buffer.reserve(c.len() + 1);
            self.buffer.put(c.as_bytes());
            self.buffer.put_u8(b' ');

            let chunk = message
                .chars()
                .skip(begin)
                .take(chunk_len)
                .collect::<String>();
            if let Some(level_color) = level_color {
                self.buffer.extend(level_color.paint(chunk).as_bytes());
            } else {
                self.buffer.extend(chunk.as_bytes());
            }
            self.buffer.reserve(1);
            self.buffer.put_u8(b'\n');
        }
    }

    fn buffer_format(&mut self, record: &Record) -> Result<(), Error> {
        let line = record.format(&self.format)?;
        self.buffer.reserve(line.len() + 1);
        self.buffer.put_slice(line.as_bytes());
        self.buffer.put_u8(b'\n');
        Ok(())
    }
}

impl Sink for Terminal {
    type SinkItem = Record;
    type SinkError = Error;

    fn start_send(&mut self, record: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        match self.format {
            Format::Csv | Format::Json | Format::Raw => self.buffer_format(&record)?,
            Format::Human => self.buffer_human(&record),
            Format::Html => {
                unreachable!("Unimplemented format html");
            }
        }
        stdout().write_all(&self.buffer)?;
        self.buffer.clear();
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        Ok(Async::Ready(()))
    }
}
