# `logselect`: extract portions from log files

[![Build Status](https://travis-ci.org/hamstergene/logselect.svg?branch=master)](https://travis-ci.org/hamstergene/logselect)

A tool to extract compilation errors and other relevant portions of large log files, primarily to include them into build bot emails. Seeing relevant errors right in the email notification saves a lot of developer time.

### Example

For example, the following spec picks every `ld` error:

    [ld-errors]
    start = '^ld: '
    direction = 'up'
    while = '^  '
    stop_offset = -1

...producing:

    Undefined symbols for architecture x86_64:
      "_deflate", referenced from:
          _main in bar-bd6f69.o
    ld: symbol(s) not found for architecture x86_64

    ... ... ...

    duplicate symbol _hello in:
        /var/folders/ty/q2xr9zn14v9c86x_vdcknflr0000gn/T/baz1-7d80d8.o
        /var/folders/ty/q2xr9zn14v9c86x_vdcknflr0000gn/T/baz2-8e12a5.o
    ld: 1 duplicate symbol for architecture x86_64



### Why not `awk` or `sed`?

0. `logselect` is parallelized. Very large log files (over 100 MB) can take minutes to process on single core which slows reaction time of incremental build checkers. It is almost 4 times faster.

0. It allows to achieve things that are impossible with `awk`. In the example above, we first find the last line of error message and then go *backwards*.

0. The config files are human-readable and easier to maintain. 

### Usage

    Usage: logselect [options] [logfile]

    Options:
        -f, --filters specs.toml
                            TOML file(s) with filter specifications
        -h, --help          print this help message and exit


    If `logfile` is not given, the standard input will be used.

    When no filter spec options are provided,
    lines containing the word "error" are selected.

The output contains portions of the input that match filter specifications. Non-consequitive portions are separated from each other with "... ... ..." lines.

Multiple specification files are allowed. Keeping specs of each build tool in a separate file allows to speed up filtering by including only the needed set of files into each search.

### Spec format

Specs consists of sections (TOML tables), each defining a range of lines to include into the output. Section names are for clarity only and are not used.

    [my_section_name]
    start = <regex>
    while = <regex>
    stop = <regex>
    direction = <forward | backward> or <down | up>
    limit = <positive integer>
    start_offset = <integer>
    stop_offset = <regex>
    disable = <true | false>

The section may contain the following instructions:

* `start` — Starts a selection. Every input line that matches this regex selects that line for output.

* `while` — Extends the selection with adjacent lines that match given regex. The selection ends at first line that does not match the regex, *not* including that line.

* `stop` — Extends the selection with adjacent lines (unconditionally, unless `while` is also present) up to the first line that matches given regex, *including* that line.

* `direction` — The direction in which `stop` and `while` extend the selection. Default is down.

* `limit` — Limits maximum size of each selection. Default is 1000. This prevents outputting the whole input file if you accidentally screw up regexes.

* `start_offset` — Adjusts the start of the selection by given number of lines. Negative value means backwards (up). This only modifies output and does not affect `while` and `stop`.

* `stop_offset` — Adjusts the end of the selection by given number of lines. Negative value means backwards (up).

* `disable` — If `true` this section will be ignored. Basically it's a way to temporarily comment it out.

A section that contains only `start = <regex>`, `start_offset = -x` and `stop_offset = y` is essentially equivalent to `grep -Bx -Ay <regex>`.

A section that contains `start` and `stop` is equivalent to `awk '/start/,/stop/'`.

### Installation

Homebrew:

    brew install hamstergene/tap/logselect

