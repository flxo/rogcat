// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use std::env;
use term_size::dimensions;

pub fn terminal_width() -> Option<usize> {
    match dimensions() {
        Some((width, _)) => Some(width),
        None => env::var("COLUMNS")
            .ok()
            .and_then(|e| e.parse::<usize>().ok()),
    }
}