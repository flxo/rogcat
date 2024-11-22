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
use failure::Error;
use regex::{RegexSet, RegexSetBuilder};
use rogcat::record::{Level, Record};

/// Configured filters
#[derive(Debug)]
pub struct Filter {
    level: Option<Level>,
    has_positive: bool,
    has_negative: bool,
    filter: FilterSet,
    filter_case_insensitive: FilterSet,
    message: FilterSet,
    message_case_insensitive: FilterSet,
    tag: FilterSet,
    tag_case_insensitive: FilterSet,
}

pub fn from_args_profile(args: &ArgMatches, profile: &Profile) -> Result<Filter, Error> {
    // Level is filtered by ffx in case of fuchsia.
    let level = (!args.is_present("fuchsia"))
        .then(|| args.value_of("level").map(Level::from))
        .flatten();

    let filter = args
        .values_of("filter")
        .unwrap_or_default()
        .chain(profile.filter.iter().map(String::as_str))
        .collect::<Vec<_>>();
    let filter_case_insensitive = args
        .values_of("filter-case-insensitive")
        .unwrap_or_default()
        .chain(profile.filter_case_insensitive.iter().map(String::as_str))
        .collect::<Vec<_>>();

    let tag = args
        .values_of("tag")
        .unwrap_or_default()
        .chain(filter.iter().copied()) // Include the filter pattern as tag filter
        .chain(profile.filter.iter().map(String::as_str)) // Include the filter pattern from the profile as tag filter
        .chain(profile.tag.iter().map(String::as_str));
    let tag_case_insensitive = args
        .values_of("tag-case-insensitive")
        .unwrap_or_default()
        .chain(filter_case_insensitive.iter().copied()) // Include the filter pattern as tag filter
        .chain(profile.filter_case_insensitive.iter().map(String::as_str)) // Include the filter pattern from the profile as tag filter
        .chain(profile.tag_case_insensitive.iter().map(String::as_str));

    let message = args
        .values_of("message")
        .unwrap_or_default()
        .chain(filter.iter().copied()) // Include the filter pattern as message filter
        .chain(profile.filter.iter().map(String::as_str)) // Include the filter pattern from the profile as message filter
        .chain(profile.message.iter().map(String::as_str));
    let message_case_insensitive = args
        .values_of("message-case-insensitive")
        .unwrap_or_default()
        .chain(filter_case_insensitive.iter().copied()) // Include the filter pattern as tag filter
        .chain(profile.filter_case_insensitive.iter().map(String::as_str)) // Include the filter pattern from the profile as tag filter
        .chain(profile.message_case_insensitive.iter().map(String::as_str));

    let filter = FilterSet::new(filter.iter().copied(), true)?;
    let filter_case_insensitive = FilterSet::new(filter_case_insensitive.iter().copied(), false)?;
    let tag = FilterSet::new(tag, true)?;
    let tag_case_insensitive = FilterSet::new(tag_case_insensitive, false)?;
    let message = FilterSet::new(message, true)?;
    let message_case_insensitive = FilterSet::new(message_case_insensitive, false)?;

    let has_positive = filter.has_positive()
        || filter_case_insensitive.has_positive()
        || tag.has_positive()
        || tag_case_insensitive.has_positive()
        || message.has_positive()
        || message_case_insensitive.has_positive();
    let has_negative = filter.has_negative()
        || filter_case_insensitive.has_negative()
        || tag.has_negative()
        || tag_case_insensitive.has_negative()
        || message.has_negative()
        || message_case_insensitive.has_negative();

    let filter = Filter {
        level,
        has_positive,
        has_negative,
        filter,
        filter_case_insensitive,
        message,
        message_case_insensitive,
        tag,
        tag_case_insensitive,
    };

    Ok(filter)
}

impl Filter {
    pub fn filter(&self, record: &Record) -> bool {
        if let Some(ref level) = self.level {
            if record.level < *level {
                return false;
            }
        }

        if self.has_positive || self.has_negative {
            let positive = !self.has_positive || self.matches_positive(record);
            let negative = self.has_negative && self.matches_negative(record);
            positive && !negative
        } else {
            true
        }
    }

    fn matches_positive(&self, record: &Record) -> bool {
        self.filter.match_positive(&record.process)
            || self.filter.match_positive(&record.thread)
            || self.filter_case_insensitive.match_positive(&record.process)
            || self.filter_case_insensitive.match_positive(&record.thread)
            || self.tag.match_positive_iter(record.tags.iter())
            || self
                .tag_case_insensitive
                .match_positive_iter(record.tags.iter())
            || self.message.match_positive(&record.message)
            || self
                .message_case_insensitive
                .match_positive(&record.message)
    }

    fn matches_negative(&self, record: &Record) -> bool {
        self.filter.match_negative(&record.process)
            || self.filter.match_negative(&record.thread)
            || self.filter_case_insensitive.match_negative(&record.process)
            || self.filter_case_insensitive.match_negative(&record.thread)
            || self.tag.match_negative_iter(record.tags.iter())
            || self
                .tag_case_insensitive
                .match_negative_iter(record.tags.iter())
            || self.message.match_negative(&record.message)
            || self
                .message_case_insensitive
                .match_negative(&record.message)
    }
}

#[derive(Debug)]
struct FilterSet {
    positive: RegexSet,
    negative: RegexSet,
}

impl FilterSet {
    fn new<'a, T: Iterator<Item = &'a str>>(
        regex: T,
        case_sensitive: bool,
    ) -> Result<FilterSet, Error> {
        let mut positive = Vec::new();
        let mut negative = Vec::new();

        for r in regex {
            if let Some(r) = r.strip_prefix('!') {
                negative.push(r);
            } else {
                positive.push(r);
            }
        }

        let positive = RegexSetBuilder::new(positive)
            .case_insensitive(!case_sensitive)
            .build()?;
        let negative = RegexSetBuilder::new(negative)
            .case_insensitive(!case_sensitive)
            .build()?;

        Ok(FilterSet { positive, negative })
    }

    fn has_positive(&self) -> bool {
        !self.positive.is_empty()
    }

    fn has_negative(&self) -> bool {
        !self.negative.is_empty()
    }

    fn match_positive<T: AsRef<str>>(&self, item: T) -> bool {
        !self.positive.is_empty() && self.positive.is_match(item.as_ref())
    }

    fn match_positive_iter<I: Iterator<Item = T>, T: AsRef<str>>(&self, mut iter: I) -> bool {
        !self.positive.is_empty() && iter.any(|i| self.positive.is_match(i.as_ref()))
    }

    fn match_negative<T: AsRef<str>>(&self, item: T) -> bool {
        !self.negative.is_empty() && self.negative.is_match(item.as_ref())
    }

    fn match_negative_iter<I: Iterator<Item = T>, T: AsRef<str>>(&self, mut iter: I) -> bool {
        !self.negative.is_empty() && iter.any(|i| self.negative.is_match(i.as_ref()))
    }
}
