[![Build Status](https://travis-ci.org/flxo/rogcat.svg)](https://travis-ci.org/flxo/rogcat)
[![Build status](https://ci.appveyor.com/api/projects/status/ng8npy7ym6l8lsy0?svg=true)](https://ci.appveyor.com/project/flxo/rogcat)
[![crates.io](https://img.shields.io/crates/v/rogcat.svg)](https://img.shields.io/crates/v/rogcat.svg)

# rogcat

...is a `adb logcat` wrapper. The `Android Debugging Bridge` (`adb`) is the default way to interact with a Android
device during development. The `logcat` subcommand of `adb` allows access to `Androids` internal log buffers. `rogcat`
tries to give access to those logs in a convenient way including post processing capabilities. The main feature probably
is a painted and reformatted view. `rogcat` can read logs from

* running `adb logcat` (default)
* a custom command (`stdout` and `stderr`)
* one or multiple files
* `stdin`
* a serial port

The processing steps within a `rogcat` run include parsing of the input stream and applying filters (if provided).
`rogcat` comes with a set of implemented in and output formats including `csv` and `html` (output only).

The output destination options of `rogcat` are

* `stdout`
* file(s)

Logs dumped to files are readable by `rogcat` (except `html` format) to allow multi pass processing.

![Screenshot](/screenshot.png)

## Examples

Capture logs from a connected device and display unconditionally:

`rogcat`

Write captured (from `adb logcat`) logs to `testrun.log`:

`rogcat -o testrun.log`

Write captured logs to file `./trace/testrun-XXX.log` by starting a new file every 1000 lines. If `./trace` is not present
it is created:

`rogcat -o ./trace/testrun.log -n 1000` or `rogcat -o ./trace/testrun.log -n 1k`

Process the `stdout/stderr` output of `somecommand`:

`rogcat somecommand` or `somecommand | rogcat -`

Display logs from `adb logcat` and filter on records where the tag matches `^ABC.*` along with *not* `X` and the message includes `pattern`:

`rogcat -t "^ADB.*" -t \!X -m pattern`

Read all files matching `trace*` in alphanumerical order and dump lines matching `hmmm` to `/tmp/filtered`:

`rogcat -i trace* -m hmmm  -o /tmp/filtered

Open and read `/dev/ttyUSB0` with given settings and process:

`rogcat -i serial:///dev/ttyUSB0@115200,8N1`

...and on `Windows` this would look like this:

`rogcat -i serial://COM0@115200,8N1`

Capture a `Android` bugreport. This only works for `Android` version prior 7:

`rogcat bugreport`

Capture a `Android` bugreport and write (zipped) to `bugreport.zip`:

`adb bugreport -z bugreport.zip`

Place messages (on level `INFO`) read on `stdin` in the devices log buffer (e.g annotations during manual testing):

`rogcat log -l info  -`

List available profiles:

`rogcat profiles --list`

Live trace with profile `app`:

`rogcat -p app`

## Installation

With a working/recent `Rust` and `cargo` setup run

```
cargo install rogcat
```

## Configuration

Optionally `rogcat` reads a (`toml` formated) configuration file if present. This configuration may include tracing profiles
('-p') and settings. The possible options in the config file are a subset of the command line options. The configuration
file is read from the location set in the environment variable `ROGCAT_CONFIG` or a fixed pathes depending on your OS:

* MacOS: `Library/Application Support/rogcat/config.toml`
* Linux: `$HOME/.config/rogcat/config.toml`
* Windows: `%HOME%/AppData/Local/rogcat/config.toml`

The environment variable overrules the default path. See `rogcat configuration --example`, `rogcat configuration --help` or `rogcat profiles --help`.

Example:

```
[profile.R]
comment = "Messages starting with R"
message = ["^R.*"]

[profile."Comments are optional"]
tag = ["rogcat"]

[profile."W hitespace"]
comment = "Profile names can contain whitespaces. Quote on command line..."

[profile.complex]
comment = "Profiles can be complex. This one is probably very useless."
highlight = ["blah"]
message = ["^R.*", "!^A.*", "!^A.*"]
tag = ["b*", "!adb"]

[profile.rogcat]
comment = "Only tag \"rogcat\""
tag = ["^rogcat$"]
```

To check your setup, run `rogcat profiles --list` and select a profile for a run by passing the `-p/--profile` option.

## Usage

```
rogcat 0.2.7-pre
Felix Obenhuber <felix@obenhuber.de>
A 'adb logcat' wrapper and log processor. Your configuration file location is "/Users/felix/Library/Application Support/rogcat/config.toml".

USAGE:
    rogcat [FLAGS] [OPTIONS] [COMMAND] [SUBCOMMAND]

FLAGS:
    -c, --clear             Clear (flush) the entire log and exit
    -d, --dump              Dump the log and then exit (don't block)
        --help              Prints help information
        --no-color          Monochrome terminal output
        --no-timestamp      No timestamp in terminal output
        --overwrite         Overwrite output file if present
    -r, --restart           Restart command on exit
        --shorten-tags      Shorten tags by removing vovels if too long for human terminal format
        --show-date         Show month and day in terminal output
        --show-time-diff    Show the time difference between the occurence of equal tags in terminal output
    -V, --version           Prints version information

OPTIONS:
    -f, --file-format <file_format>              Select output file format [values: csv, html, raw]
    -a, --filename-format <filename_format>      Select a format for output file names. By passing 'single' the filename provided with the '-o' option is used (default).'enumerate' appends a file sequence
                                                 number after the filename passed with '-o' option whenever a new file is created (see 'records-per-file' option). 'date' will prefix the output filename
                                                 with the current local date when a new file is created [values: single, enumerate, date]
    -h, --highlight <highlight>...               Highlight messages that match this pattern in RE2. The prefix '!' inverts the match
    -i, --input <input>...                       Read from file instead of command. Use 'serial://COM0@115200,8N1 or similiar for reading a serial port
    -l, --level <level>                          Minimum level [values: trace, debug, info, warn, error, fatal, assert, T, D, I, W, E, F, A]
    -m, --message <message>...                   Tag filters in RE2. The prefix '!' inverts the match
    -o, --output <output>                        Write output to file
    -p, --profile <profile>                      Select profile
    -n, --records-per-file <records_per_file>    Write n records per file. Use k, M, G suffixes or a plain number
    -t, --tag <tag>...                           Tag filters in RE2. The prefix '!' inverts the match
        --tail <tail>                            Dump only the most recent <COUNT> lines (implies --dump)
    -e, --terminal-format <terminal_format>      Select format for stdout [default: human]  [values: human, raw, csv]

ARGS:
    <COMMAND>    Optional command to run and capture stdout from. Pass "-" to d capture stdin'. If omitted, rogcat will run "adb logcat -b all" and restarts this commmand if 'adb' terminates

SUBCOMMANDS:
    bugreport        Capture bugreport. This is only works for Android versions < 7.
    completions      Generates completion scripts
    configuration    Configuration options
    devices          Show list of available devices
    help             Prints this message or the help of the given subcommand(s)
    log              Add log message(s) log buffer
    profiles         Show and manage configuration profiles
```

## Bugs

There are plenty. Please report on GitHub. Patches are welcome!

## Licensing

Rogcat is open source software; see ``COPYING`` for details.
