// Copyright © 2017 Felix Obenhuber
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

use crate::record::{Level, Record};
use chrono::Utc;

pub fn parse(line: &str) -> Option<Record<'_>> {
    let mut split = line.split_whitespace();

    let timestamp = Utc::now().naive_local();
    let _month_day = split.next()?;
    let _timestamp = split.next()?;

    let process = split.next().and_then(|s| s.parse::<u32>().ok())?;
    let thread = split.next().and_then(|s| s.parse::<u32>().ok())?;
    let level: Level = split.next().map(Into::into)?;
    let tag = split.next().map(|t| t.trim_end_matches(':'))?;
    let message = if let Some(position) = line.rfind(':') {
        let (_, message) = line.split_at(position);
        let message = message.trim_start_matches(':');
        message.trim()
    } else {
        ""
    };

    Some(Record {
        timestamp,
        message,
        level,
        tag,
        process,
        thread,
        raw: line,
    })
}

#[test]
fn tag_space() {
    let record = parse("01-01 00:00:11.349   136   161 I Netd    : Successfully executed shell command ip link set dev lo up").unwrap();
    assert_eq!(record.tag, "Netd");
    assert_eq!(
        record.message,
        "Successfully executed shell command ip link set dev lo up"
    );

    let record = parse("01-01 00:00:11.349   136   161 I Netd: Successfully executed shell command ip link set dev lo up").unwrap();
    assert_eq!(record.tag, "Netd");
    assert_eq!(
        record.message,
        "Successfully executed shell command ip link set dev lo up"
    );
}
