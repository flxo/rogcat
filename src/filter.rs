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
use crate::record::{Level, Record};
use clap::ArgMatches;
use failure::format_err;
use failure::Error;
use regex::Regex;

/// Configured filters
pub struct Filter {
    level: Level,
    message: Vec<Regex>,
    message_negative: Vec<Regex>,
    tag: Vec<Regex>,
    tag_negative: Vec<Regex>,
}

pub fn from_args_profile<'a>(args: &ArgMatches<'a>, profile: &Profile) -> Filter {
    let mut tag_filter = args
        .values_of("tag")
        .map(|m| m.map(|f| f.to_owned()).collect::<Vec<String>>())
        .unwrap_or_else(|| vec![]);
    tag_filter.extend(profile.tag.clone());
    let mut message_filter = args
        .values_of("message")
        .map(|m| m.map(|f| f.to_owned()).collect::<Vec<String>>())
        .unwrap_or_else(|| vec![]);
    message_filter.extend(profile.message.clone());

    let (tag, tag_negative) = init_filter(&tag_filter).expect("Filter config error");
    let (message, message_negative) = init_filter(&message_filter).expect("Filter config error");

    Filter {
        level: Level::from(args.value_of("level").unwrap_or("")),
        message,
        message_negative,
        tag,
        tag_negative,
    }
}

impl<'a> Filter {
    pub fn filter(&self, record: &Record) -> bool {
        if record.level < self.level {
            return false;
        }

        if !self.message.is_empty() && !self.message.iter().any(|m| m.is_match(&record.message)) {
            return false;
        }

        if self
            .message_negative
            .iter()
            .any(|m| m.is_match(&record.message))
        {
            return false;
        }

        if !self.tag.is_empty() && !self.tag.iter().any(|m| m.is_match(&record.tag)) {
            return false;
        }

        if self.tag_negative.iter().any(|m| m.is_match(&record.tag)) {
            return false;
        }

        true
    }
}

fn init_filter(i: &[String]) -> Result<(Vec<Regex>, Vec<Regex>), Error> {
    let mut positive = vec![];
    let mut negative = vec![];
    for r in i {
        if r.starts_with('!') {
            let r = &r[1..];
            negative.push(Regex::new(r).map_err(|_| format_err!("Invalid regex string: {}", r))?)
        } else {
            positive.push(Regex::new(r).map_err(|_| format_err!("Invalid regex string: {}", r))?)
        }
    }
    Ok((positive, negative))
}

#[test]
fn filter_args() {
    assert!(init_filter(&vec![]).is_ok());
    assert!(init_filter(&vec!["".to_owned()]).is_ok());
    assert!(init_filter(&vec!["a".to_owned()]).is_ok());
    assert!(init_filter(&vec![".*".to_owned()]).is_ok());
    assert!(init_filter(&vec![".*".to_owned(), ".*".to_owned()]).is_ok());
    assert!(init_filter(&vec!["(".to_owned()]).is_err());
}
