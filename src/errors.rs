// Copyright © 2016 Felix Obenhuber
// This program is free software. It comes without any warranty, to the extent
// permitted by applicable law. You can redistribute it and/or modify it under
// the terms of the Do What The Fuck You Want To Public License, Version 2, as
// published by Sam Hocevar. See the COPYING file for more details.

error_chain! {
    foreign_links {
        Clap(::clap::Error);
        Csv(::csv::Error);
        Handlebars(::handlebars::RenderError);
        Io(::std::io::Error);
        Nom(::nom::ErrorKind);
        Regex(::regex::Error);
        Serial(::serial::Error);
        Time(::time::ParseError);
        Utf8(::std::string::FromUtf8Error);
        Zip(::zip::result::ZipError);
    }
}
