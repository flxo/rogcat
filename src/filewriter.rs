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

use crate::LogSink;
use clap::ArgMatches;
use failure::{err_msg, format_err, Error};
use futures::{Async, AsyncSink, Poll, Sink, StartSend};
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use rogcat::record::{Format, Record};
use std::{
    fs::{DirBuilder, File},
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
};
use time::{now, strftime};

/// Filename format
#[derive(Clone)]
enum FilenameFormat {
    Date(bool, usize),
    Enumerate(bool, usize),
    Single(bool),
}

/// Textfile with format
struct Textfile {
    file: File,
    format: Format,
}

struct FileWriter<T> {
    current_filename: PathBuf,
    file_size: usize,
    filename: PathBuf,
    filename_format: FilenameFormat,
    index: usize,
    format: Format,
    progress: ProgressBar,
    writer: Option<Box<T>>,
}

trait Writer {
    fn with_file_format(filename: &Path, format: &Format) -> Result<Self, Error>
    where
        Self: Sized;
    fn write(&mut self, record: &Record, index: usize) -> Result<(), Error>;
    fn flush(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

/// Crate a new log sink for given arguments
pub fn try_from<'a>(args: &ArgMatches<'a>) -> Result<LogSink, Error> {
    let format = args
        .value_of("format")
        .and_then(|f| Format::from_str(f).ok())
        .unwrap_or(Format::Raw);

    Ok(match format {
        Format::Csv | Format::Json | Format::Raw => {
            Box::new(FileWriter::<Textfile>::from_args(args, format)?) as LogSink
        }
        Format::Html => Box::new(FileWriter::<html::Html>::from_args(args, format)?) as LogSink,
        Format::Human => panic!("Unsupported format human in output file"),
    })
}

impl Writer for Textfile {
    fn with_file_format(filename: &Path, format: &Format) -> Result<Textfile, Error> {
        let file = File::create(filename).map_err(|e| {
            format_err!("Failed to create output file {}: {}", filename.display(), e)
        })?;
        Ok(Textfile {
            file,
            format: format.clone(),
        })
    }

    fn write(&mut self, record: &Record, _index: usize) -> Result<(), Error> {
        self.file
            .write(self.format.fmt_record(record)?.as_bytes())
            .map_err(|e| format_err!("Failed to write: {}", e))?;
        self.file
            .write(b"\n")
            .map_err(|e| format_err!("Failed to write: {}", e))?;
        Ok(())
    }
}

impl<'a, T: Writer> FileWriter<T> {
    pub fn from_args(args: &ArgMatches<'a>, format: Format) -> Result<Self, Error> {
        let filename = args
            .value_of("output")
            .and_then(|f| Some(PathBuf::from(f)))
            .ok_or_else(|| err_msg("Invalid output filename!"))?;

        let records_per_file = args.value_of("records_per_file").and_then(|l| {
            Regex::new(r"^(\d+)([kMG])$")
                .unwrap()
                .captures(l)
                .and_then(|caps| {
                    caps.get(1)
                        .map(|m| m.as_str())
                        .and_then(|size| usize::from_str(size).ok())
                        .map(|size| (size, caps.get(2).map(|m| m.as_str())))
                })
                .and_then(|(size, suffix)| match suffix {
                    Some("k") => Some(1_000 * size),
                    Some("M") => Some(1_000_000 * size),
                    Some("G") => Some(1_000_000_000 * size),
                    _ => None,
                })
                .or_else(|| usize::from_str(l).ok())
        });

        let overwrite = args.is_present("overwrite");

        let records = records_per_file.unwrap_or(std::usize::MAX);
        let filename_format = match args.value_of("filename_format") {
            Some("enumerate") => FilenameFormat::Enumerate(overwrite, records),
            Some("date") => FilenameFormat::Date(overwrite, records),
            // If records per file is set, default to enumerated even if
            // no file format argument is supplied.
            Some(_) | None => {
                if let Some(n) = records_per_file {
                    FilenameFormat::Enumerate(overwrite, n)
                } else {
                    FilenameFormat::Single(overwrite)
                }
            }
        };

        let progress = {
            let (pb, chars, template) = if let Some(n) = records_per_file {
                (
                    ProgressBar::new(n as u64),
                    "•• ",
                    "{spinner:.yellow} Writing {msg:.dim.bold} {pos:>7.dim}/{len:.dim} {elapsed_precise:.dim} [{bar:40.yellow/green}] ({eta:.dim})",
                )
            } else {
                (
                    ProgressBar::new(std::u64::MAX),
                    " • ",
                    "{spinner:.yellow} Writing {msg:.dim.bold} {pos:>7.dim} {elapsed_precise:.dim}",
                )
            };
            pb.set_style(
                ProgressStyle::default_bar()
                    .template(template)
                    .progress_chars(chars),
            );
            pb
        };

        Ok(FileWriter {
            current_filename: filename.clone(),
            file_size: 0,
            filename,
            filename_format,
            index: 0,
            format,
            progress,
            writer: None,
        })
    }

    fn next_file(&self) -> Result<PathBuf, Error> {
        match self.filename_format {
            FilenameFormat::Single(overwrite) => {
                if self.filename.exists() && !overwrite {
                    Err(format_err!(
                        "{} exists. Use overwrite flag to force!",
                        self.filename.display()
                    ))
                } else {
                    Ok(self.filename.clone())
                }
            }
            FilenameFormat::Enumerate(_overwrite, _) => {
                if self.filename.as_path().is_dir() {
                    return Err(format_err!(
                        "Output file {} is a directory",
                        self.filename.display()
                    ));
                }

                let dir = self.filename.parent().unwrap_or_else(|| Path::new(""));
                if !dir.is_dir() {
                    DirBuilder::new().recursive(true).create(dir).map_err(|e| {
                        format_err!("Failed to create outfile parent directory: {:?}", e)
                    })?
                }

                let next = |index| -> Result<PathBuf, Error> {
                    let mut name = self.filename.clone();
                    name = PathBuf::from(format!(
                        "{}-{:03}",
                        name.file_stem()
                            .ok_or_else(|| err_msg("Invalid path"))?
                            .to_str()
                            .ok_or_else(|| err_msg("Invalid path"))?,
                        index
                    ));
                    if let Some(extension) = self.filename.extension() {
                        name.set_extension(extension);
                    }
                    Ok(dir.join(name))
                };

                for index in 0.. {
                    let n = next(index)?;
                    if !n.exists() {
                        return Ok(n);
                    }
                }

                unreachable!("Could not find a file - this is proably a bug here...")
            }
            FilenameFormat::Date(overwrite, _) => {
                // If the overwrite flag is set the files are
                // enumerated from the first one to get nice
                // aligned filenames.
                let mut e: Option<u32> = if overwrite { None } else { Some(0) };

                loop {
                    let dir = self.filename.parent().unwrap_or_else(|| Path::new(""));
                    if !dir.is_dir() {
                        DirBuilder::new().recursive(true).create(dir).map_err(|e| {
                            format_err!(
                                "Failed to create outfile parent directory {}: {}",
                                dir.display(),
                                e
                            )
                        })?;
                    }

                    let now = strftime("%F-%H_%M_%S", &now())?;
                    let enumeration = e
                        .map(|a| format!("-{:03}", a))
                        .unwrap_or_else(|| "".to_owned());
                    let filename = self
                        .filename
                        .file_name()
                        .ok_or_else(|| err_msg("Invalid path"))?
                        .to_str()
                        .ok_or_else(|| err_msg("Invalid path"))?;
                    let candidate = PathBuf::from(format!("{}{}_{}", now, enumeration, filename));
                    let candidate = dir.join(candidate);
                    if !overwrite && candidate.exists() {
                        e = Some(e.unwrap_or(0) + 1);
                        continue;
                    } else {
                        return Ok(candidate);
                    }
                }
            }
        }
    }

    fn write(&mut self, record: &Record) -> Result<(), Error> {
        match self.writer {
            Some(ref mut writer) => {
                writer.write(record, self.index)?;
                self.index += 1;
            }
            None => {
                self.current_filename = self.next_file()?;
                let mut writer = T::with_file_format(&self.current_filename, &self.format)?;
                let message = format!("Writing {}", self.current_filename.display());
                self.progress.set_message(&message);
                writer.write(record, self.index)?;
                self.index += 1;
                self.writer = Some(Box::new(writer));
            }
        }

        self.file_size += 1;
        self.progress.set_position(self.file_size as u64);

        match self.filename_format {
            FilenameFormat::Enumerate(_, n) | FilenameFormat::Date(_, n) => {
                if self.file_size >= n {
                    self.flush()
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }

    fn flush(&mut self) -> Result<(), Error> {
        if let Some(ref mut writer) = self.writer {
            writer.flush()?;
        }
        self.progress
            .set_style(ProgressStyle::default_bar().template("{msg:.dim.bold}"));
        self.progress
            .finish_with_message(&format!("Dumped {} records", self.index));
        self.file_size = 0;
        self.writer.take();
        Ok(())
    }
}

impl<T: Writer> Sink for FileWriter<T> {
    type SinkItem = Record;
    type SinkError = Error;

    fn start_send(&mut self, record: Record) -> StartSend<Record, Error> {
        self.write(&record).map(|_| AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Error> {
        Ok(Async::Ready(()))
    }
}

mod html {
    use super::Writer;
    use crc::{crc32, Hasher32};
    use failure::{format_err, Error};
    use handlebars::{
        to_json, Context, Handlebars, Helper, HelperResult, JsonRender, Output, RenderContext,
        RenderError,
    };
    use rogcat::record::{Format, Record};
    use serde::Serialize;
    use serde_json::value::{Map, Value as Json};
    use std::{
        fs::File,
        path::{Path, PathBuf},
        str,
    };

    #[derive(Serialize)]
    struct HtmlRecord {
        index: usize,
        record: Record,
    }

    /// Simple static html file
    pub struct Html {
        filename: PathBuf,
        records: Vec<HtmlRecord>,
    }

    impl Html {
        // TODO: ensure readability
        fn hash_color(value: &str) -> String {
            let mut digest = crc32::Digest::new(crc32::IEEE);
            digest.write(value.as_bytes());
            let h = digest.sum32();
            let r = h & 0xFF;
            let g = (h & 0xFF00) >> 8;
            let b = (h & 0xFF_0000) >> 16;
            format!("#{:02x}{:02x}{:02x}", r, g, b)
        }
        fn color_helper(
            h: &Helper,
            _: &Handlebars,
            _: &Context,
            _: &mut RenderContext,
            out: &mut dyn Output,
        ) -> HelperResult {
            let param = h
                .param(0)
                .ok_or_else(|| RenderError::new("Param 0 is required for format helper."))?;
            let value = param.value().render();
            let rendered = if value.is_empty() || value == "0" {
                format!("<span style=\"color:grey\">{}</span>", value)
            } else {
                format!(
                    "<span style=\"color:{}\">{}</span>",
                    Self::hash_color(&value),
                    value
                )
            };
            out.write(&rendered)?;
            Ok(())
        }

        fn flush(&mut self) -> Result<(), Error> {
            let mut hb = Handlebars::new();
            let mut data: Map<String, Json> = Map::new();
            data.insert("records".to_owned(), to_json(&self.records));
            let mut output_file = File::create(&self.filename)?;
            hb.register_helper("color", Box::new(Self::color_helper));
            hb.register_template_string("t1", HTML_TEMPLATE)?;
            hb.render_to_write("t1", &data, &mut output_file)
                .map_err(|e| format_err!("Rednering error: {}", e))
                .map(|_| ())
        }
    }

    impl Writer for Html {
        fn with_file_format(filename: &Path, _: &Format) -> Result<Html, Error> {
            Ok(Html {
                filename: filename.to_owned(),
                records: Vec::new(),
            })
        }

        fn write(&mut self, record: &Record, index: usize) -> Result<(), Error> {
            self.records.push(HtmlRecord {
                index,
                record: record.clone(),
            });
            Ok(())
        }
    }

    impl Drop for Html {
        fn drop(&mut self) {
            self.flush().ok();
        }
    }

    const HTML_TEMPLATE: &str = r#"
<!doctype HTML>
<title>Rogcat</title>
<link href='http://fonts.googleapis.com/css?family=Source+Code+Pro' rel='stylesheet' type='text/css'>
<style>
body {background: black; color: #BBBBBB; font-family: 'Source Code Pro', Monaco, monospace; font-size: 12px}
.green, .I {color: #A8FF60}
.white {color: #EEEEEE}
.red, .E, .A, .F {color: #FF6C60}
.yellow, .W {color: #FFFFB6}
.black {color: #4F4F4F}
.blue {color: #96CBFE}
.cyan {color: #C6C5FE}
.magenta {color: #FF73FD}
tr.hover { background: #260041 }
table {
    border-spacing: 0;
    width: 100%;
}
td {
    vertical-align: top;
    padding-bottom: 0;
    padding-left: 2ex;
    padding-right: 2ex;
    white-space: nowrap;
}
tr:hover {
    color: yellow;
}
td.level-D {
    color: white;
    background: #555;
}
td.level-I {
    color: black;
    background: #A8FF60;
}
td.level-W {
    color: black;
    background: #FFFFB6;
}
td.level-E {
    color: black;
    background: #FF6C60;
}
td.level-A {
    color: black;
    background: #FF6C60;
}
td.level-F {
    color: black;
    background: #FF6C60;
}
table tr td:first-child + td + td {
    text-align: right
}
table tr td:first-child + td + td + td {
}
table tr td:first-child + td + td + td + td {
    text-align: right
}
table tr td:first-child + td + td + td + td + td {
}
</style>

<table>

{{#each records as |t| ~}}
    <tr>
    <td>{{t.index}}</td>
    <td>{{t.record.timestamp}}</td>
    <td><a>{{color t.record.tag}}</a></td>
    <td>{{color t.record.process}}</td>
    <td>{{color t.record.thread}}</td>
    <td class="level-{{t.record.level}}">{{t.record.level}}</td>
    <td>{{t.record.message}}</td>
    </tr>
{{/each~}}

</table>
"#;
}
