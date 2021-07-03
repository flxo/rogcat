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

use crate::profiles::Profile;
use clap::ArgMatches;
use failure::{format_err, Error};
use regex::Regex;
use rogcat::record::{Level, Record};

/// Configured filters
#[derive(Debug)]
pub struct Filter {
    level: Level,
    tag: FilterGroup,
    tag_ignore_case: FilterGroup,
    message: FilterGroup,
    message_ignore_case: FilterGroup,
    regex: FilterGroup,
}

pub fn from_args_profile<'a>(args: &ArgMatches<'a>, profile: &Profile) -> Result<Filter, Error> {
    let tag = profile.tag.iter().map(String::as_str);
    let tag_ignorecase = profile.tag_ignore_case.iter().map(String::as_str);
    let message = profile.message.iter().map(String::as_str);
    let message_ignorecase = profile.message_ignore_case.iter().map(String::as_str);
    let regex = profile.regex.iter().map(String::as_str);
    let filter = Filter {
        level: Level::from(args.value_of("level").unwrap_or("")),
        tag: FilterGroup::from_args(args, "tag", tag, false)?,
        tag_ignore_case: FilterGroup::from_args(args, "tag-ignore-case", tag_ignorecase, true)?,
        message: FilterGroup::from_args(args, "message", message, false)?,
        message_ignore_case: FilterGroup::from_args(
            args,
            "message-ignore-case",
            message_ignorecase,
            true,
        )?,
        regex: FilterGroup::from_args(args, "regex_filter", regex, false)?,
    };

    Ok(filter)
}

impl Filter {
    pub fn filter(&self, record: &Record) -> bool {
        if record.level < self.level {
            return false;
        }

        self.message.filter(&record.message)
            && self.message_ignore_case.filter(&record.message)
            && self.tag.filter(&record.tag)
            && self.tag_ignore_case.filter(&record.tag)
            && (self.regex.filter(&record.process)
                || self.regex.filter(&record.thread)
                || self.regex.filter(&record.tag)
                || self.regex.filter(&record.message))
    }
}

#[derive(Debug)]
struct FilterGroup {
    ignore_case: bool,
    positive: Vec<Regex>,
    negative: Vec<Regex>,
}

impl FilterGroup {
    fn from_args<'a, T: Iterator<Item = &'a str>>(
        args: &'a ArgMatches<'a>,
        flag: &str,
        merge: T,
        ignore_case: bool,
    ) -> Result<FilterGroup, Error> {
        let mut filters: Vec<&str> = args
            .values_of(flag)
            .map(Iterator::collect)
            .unwrap_or_default();
        filters.extend(merge);

        let mut positive = vec![];
        let mut negative = vec![];
        for r in filters.iter().map(|f| {
            if ignore_case {
                f.to_lowercase()
            } else {
                (*f).to_string()
            }
        }) {
            if let Some(r) = r.strip_prefix('!') {
                let r = Regex::new(&r)
                    .map_err(|e| format_err!("Invalid regex string: {}: {}", r, e))?;
                negative.push(r);
            } else {
                let r = Regex::new(&r)
                    .map_err(|e| format_err!("Invalid regex string: {}: {}", r, e))?;
                positive.push(r);
            }
        }

        Ok(FilterGroup {
            ignore_case,
            positive,
            negative,
        })
    }

    fn filter(&self, item: &str) -> bool {
        if !self.positive.is_empty() {
            if self.ignore_case {
                let item = item.to_lowercase();
                if !self.positive.iter().any(|m| m.is_match(&item)) {
                    return false;
                }
            } else if !self.positive.iter().any(|m| m.is_match(&item)) {
                return false;
            }
        }

        if !self.negative.is_empty() {
            if self.ignore_case {
                let item = item.to_lowercase();
                return !self.negative.iter().any(|m| m.is_match(&item));
            } else {
                return !self.negative.iter().any(|m| m.is_match(&item));
            }
        }

        true
    }
}
