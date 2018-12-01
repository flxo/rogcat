// Copyright © 2017 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use crate::tests::utils::*;

#[test]
fn head() {
    let input = svec!("A", "B", "C", "D");
    let output = run_rogcat_with_input_file(&svec!("--head", "2"), &input).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 2);
}
