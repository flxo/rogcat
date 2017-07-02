// Copyright © 2017 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use tests::utils::*;

#[test]
fn help() {
    let args = &[
        svec!("--help"),
        svec!("bugreport", "--help"),
        svec!("completions", "--help"),
        svec!("configuration", "--help"),
        svec!("devices", "--help"),
        svec!("log", "--help"),
        svec!("profiles", "--help"),
    ];

    for a in args {
        let result = run_rogcat(a, None).expect(&a.join(" "));
        assert!(result.0);
        assert!(!result.1.is_empty());
    }
}
