// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

use kabuki::CallError;

error_chain! {
    foreign_links {
        Utf8(::std::string::FromUtf8Error);
        Io(::std::io::Error);
        Regex(::regex::Error);
    }
}

impl<T> From<CallError<T>> for Error {
    fn from(src: CallError<T>) -> Error {
        match src {
            CallError::Full(..) => "actor inbox full!".into(),
            CallError::Disconnected(..) => "actor shutdown".into(),
            CallError::Aborted => "actor aborted request".into(),
        }
    }
}
