[![Build Status](https://travis-ci.org/flxo/rogcat.png)](https://travis-ci.org/flxo/rogcat)
# rogcat


A ``adb logcat`` wrapper with colors and filter and output options written in `Rust`

![Screenshot](/screenshot.png)

## Usage

```
rogcat 0.2.2
Felix Obenhuber <felix@obenhuber.de>
A 'adb logcat' wrapper

USAGE:
    rogcat [FLAGS] [OPTIONS] [COMMAND] [SUBCOMMAND]

FLAGS:
        --verbose                Print records on stdout even when using the -o option
    -c, --clear                  Clear (flush) the entire log and exit
    -g, --get-ringbuffer-size    Get the size of the log's ring buffer and exit
        --help                   Prints help information
    -S, --output-statistics      Output statistics
    -r, --restart                Restart command on exit
        --shorten-tags           Shorten tag by removing vovels if too long
        --show-date              Show month and day
        --show-time-diff         Show time diff of tags
    -s, --skip-on-restart        Skip messages on restart until last message from previous run is (re)received
    -V, --version                Prints version information

OPTIONS:
    -f, --file-format <FILE_FORMAT>              Write format to output files [values: raw, csv]
    -l, --level <LEVEL>
            Minimum level [values: trace, debug, info, warn, error, fatal, assert, T, D, I, W, E, F, A]
    -n, --records-per-file <RECORDS_PER_FILE>    Write n records per file. Use k, M, G suffixes or a plain number
    -e, --terminal-format <TERMINAL_FORMAT>      Use format on stdout [default: human]  [values: human, raw, csv]
    -h, --highlight <HIGHLIGHT>...               Highlight pattern in RE2
    -i, --input <INPUT>...
            Read from file instead of command. Use 'serial://COM0@11520,8N1 or similiar for reading serial port
    -m, --message <MSG>...                       Message filters in RE2. The prefix ! inverts the match.
    -o, --output <OUTPUT>                        Write to file and stdout
    -t, --tag <TAG>...                           Tag filters in RE2. The prefix ! inverts the match

ARGS:
    <COMMAND>    Optional command to run and capture stdout. Pass "-" to capture stdin'. If omitted, rogcat will run
                 "adb logcat -b all

SUBCOMMANDS:
    completions    Generates completion scripts for your shell
    devices        Show list of available devices
    help           Prints this message or the help of the given subcommand(s)
```
## Bugs

There are plenty. Report them on GitHub, or - even better - open a pull request.

## Licensing

Rogcat is open source software; see ``COPYING`` for details.
