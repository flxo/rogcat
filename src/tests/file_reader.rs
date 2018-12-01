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

use crate::tests::utils::*;
use std::fs::File;
use std::io::prelude::*;

#[test]
fn invalid_string() {
    let path = tempfile().unwrap();
    File::create(path.clone())
        .unwrap()
        .write_all(b"some invalid bytes come here: \xF0\x90\x80\nhaha")
        .unwrap();
    let args = svec!("-i", format!("{}", path.display()));
    let output = run_rogcat(&args, None).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 2);
}

#[test]
fn multiple_files() {
    let content = svec!("A", "B", "C");
    let a = tempfile_with_content(&content)
        .unwrap()
        .display()
        .to_string();
    let b = tempfile_with_content(&content)
        .unwrap()
        .display()
        .to_string();
    let args = svec!("-i", a, "-i", b);
    let output = run_rogcat(&args, None).unwrap();
    assert!(output.0);
    assert_eq!(output.1.len(), 6);
}
