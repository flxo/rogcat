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
        --overwrite              Overwrite output file if present
        --verbose                Print records on stdout even when using the -o option
    -c, --clear                  Clear (flush) the entire log and exit
    -g, --get-ringbuffer-size    Get the size of the log's ring buffer and exit
        --help                   Prints help information
    -S, --output-statistics      Output statistics
    -r, --restart                Restart command on exit
        --shorten-tags           Shorten tag by removing vovels if too long
        --show-date              Show month and day when printing on stdout
        --show-time-diff         Show time diff of tags after timestamp
    -s, --skip-on-restart        Skip messages on restart until last message from previous run is (re)received. Use with
                                 caution!
    -V, --version                Prints version information

OPTIONS:
    -a, --filename-format <FILENAME_FORMAT>
            Select format for output file names. By passing 'single' the filename provided with the '-o' option is used.
            'enumerate' will append a file sequence number after the filename passed with '-o' option whenever a new
            file is created (see 'records-per-file' option). 'date' will prefix the output filename with the current
            local date when a new file is created [values: single, enumerate, date]
    -f, --file-format <FILE_FORMAT>              Select format for output files [values: raw, csv]
    -l, --level <LEVEL>
            Minimum level [values: trace, debug, info, warn, error, fatal, assert, T, D, I, W, E, F, A]
    -n, --records-per-file <RECORDS_PER_FILE>    Write n records per file. Use k, M, G suffixes or a plain number
    -e, --terminal-format <TERMINAL_FORMAT>      Select format for stdout [default: human]  [values: human, raw, csv]
    -h, --highlight <HIGHLIGHT>...               Highlight pattern in RE
    -i, --input <INPUT>...
            Read from file instead of command. Use 'serial://COM0@11520,8N1 or similiar for reading a serial port
    -m, --message <MSG>...                       Message filters in RE2. The prefix ! inverts the match
    -o, --output <OUTPUT>                        Write to file and stdout
    -t, --tag <TAG>...                           Tag filters in RE2. The prefix ! inverts the match

ARGS:
    <COMMAND>    Optional command to run and capture stdout from. Pass "-" to capture stdin'. If omitted, rogcat
                 will run "adb logcat -b all

SUBCOMMANDS:
    completions    Generates completion scripts for your shell
    devices        Show list of available devices
    help           Prints this message or the help of the given subcommand(s)
```
## Bugs

There are plenty. Report them on GitHub, or - even better - open a pull request.

## Licensing

Rogcat is open source software; see ``COPYING`` for details.
