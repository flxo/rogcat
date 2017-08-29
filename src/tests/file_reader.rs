// Copyright © 2017 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use std::fs::File;
use std::io::prelude::*;
use tests::utils::*;

#[test]
fn invalid_string() {
    let path = tempfile().unwrap();
    File::create(path.clone()).unwrap().write_all(b"some invalid bytes come here: \xF0\x90\x80\nhaha").unwrap();
    let args = svec!("-i", format!("{}", path.display()));
    let output = run_rogcat(&args, None).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 2);
}
