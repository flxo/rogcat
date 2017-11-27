// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use std::env;
use term_size::dimensions;

const CRNL: &[char] = &['\n', '\r'];

pub fn trim_cr_nl(s: &str) -> String {
    s.trim_matches(CRNL).to_owned()
}

pub fn terminal_width() -> Option<usize> {
    match dimensions() {
        Some((width, _)) => Some(width),
        None => {
            env::var("COLUMNS").ok().and_then(
                |e| e.parse::<usize>().ok(),
            )
        }
    }
}

#[test]
fn test_trim_crnl() {
    assert_eq!(&trim_cr_nl("abc\n"), "abc");
    assert_eq!(&trim_cr_nl("abc\r\n"), "abc");
    assert_eq!(&trim_cr_nl("\r\nabc\r\n"), "abc");
    assert_eq!(&trim_cr_nl("\nabc\n"), "abc");
}
