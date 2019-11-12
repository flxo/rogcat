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

mod formats;

use formats::{FormatParser};
use crate::record::Record;

pub struct Parser {
    parsers: Vec<Box<dyn FormatParser>>,
    last: Option<usize>,
}

impl Parser {
    pub fn new() -> Self {
        Parser {
            parsers: Vec::new(),
            last: None,
        }
    }

    pub fn with_default_rules() -> Self {
        Parser {
            parsers: vec![
                Box::new(formats::DefaultParser {}),
                Box::new(formats::MindroidParser {}),
                Box::new(formats::CsvParser {}),
                Box::new(formats::JsonParser {}),
                Box::new(formats::GTestParser {}),
                Box::new(formats::BugReportParser {}),
            ],
            last: None,
        }
    }

    pub fn parse(&mut self, line: String) -> Record {
        if let Some(last) = self.last {
            let p = &self.parsers[last];
            if let Ok(r) = p.try_parse_str(&line) {
                return r;
            }
        }

        for (i, p) in self.parsers.iter().enumerate() {
            if let Ok(r) = p.try_parse_str(&line) {
                self.last = Some(i);
                return r;
            }
        }

        // Seems that we cannot parse this record
        // Treat the raw input as message
        Record {
            raw: line.clone(),
            message: line,
            ..Default::default()
        }
    }
}

#[test]
fn test_parsing() {
    let mut parse_engine = Parser::with_default_rules();

    let t = "03-01 02:19:45.207     1     2 I EXT4-fs (mmcblk3p8): mounted filesystem with \
             ordered data mode. Opts: (null)";
    let r = parse_engine.parse(t.to_string());
    assert_eq!(r.level, super::record::Level::Info);
    assert_eq!(r.tag, "EXT4-fs (mmcblk3p8)");
    assert_eq!(r.process, "1");
    assert_eq!(r.thread, "2");
    assert_eq!(
        r.message,
        "mounted filesystem with ordered data mode. Opts: (null)"
    );
}
