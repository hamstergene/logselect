extern crate getopts;
extern crate regex;
extern crate toml;
use regex::Regex;
use std::collections::{BitvSet};
use std::iter;
use std::old_io::{File, stdio};
use std::os;

fn main()
{
    // cli opts
    let opts = [
        getopts::optmulti("f", "filters", "TOML file(s) with filter specifications", "specs.toml"),
        getopts::optflag("h", "help", "print this help message and exit"),
    ];
    let matches = match getopts::getopts(os::args().tail(), &opts) {
        Ok(m) => { m }
        Err(f) => { panic!(f.to_string()) }
    };
    if matches.opt_present("h") {
        let brief = format!("Usage: {} [options] [logfile]", os::args()[0]);
        println!("{}", getopts::usage(brief.as_slice(), &opts));
        println!("\nIf `logfile` is not given, the standard input will be used.\n\nWhen no filter spec options are provided,\nlines containing the word \"error\" are selected.");
        return;
    }

    // parse toml config files
    let mut specs: Vec<Spec> = vec![];
    for filename in matches.opt_strs("f") {
        consume_specs_toml(filename.as_slice(), &mut specs);
    }
    if specs.len() == 0 {
        let mut spec = Spec::new();
        spec.start = Regex::new(r"\berror\b").ok();
        specs.push(spec);
    }

    // perform
    let input_string = match matches.free.len() {
        0 => { stdio::stdin().read_to_string().unwrap() },
        1 => {
            let path = Path::new(matches.free[0].clone());
            match File::open(&path) {
                Err(why) => { panic!("can't open {}: {}", matches.free[0], why.to_string()) },
                Ok(ref mut f) => f.read_to_string().unwrap(),
            }
        },
        _ => { panic!("too many filename arguments ({}), expected just one", matches.free.len()) },
    };

    let lines : Vec<&str> = iter::FromIterator::from_iter(input_string.lines_any());
    let mut selected_indexes = BitvSet::new();
    for index in range(0, lines.len()) {
        for spec in specs.iter() {
            match spec.start {
                Some(ref rx) if rx.is_match(lines[index]) => {
                    match try_select(&spec, &lines, index) {
                        Some((a, b)) => {
                            for i in (if a < b { range(a, b+1) } else { range(b, a+1) } ) {
                                selected_indexes.insert(i);
                            }
                        },
                        _ => {},
                    };
                },
                _ => {},
            }
        }
    }

    // output
    let mut prev_index = 0;
    for index in range(0, lines.len()) {
        if selected_indexes.contains(&index) {
            if prev_index > 0 {
                if index + 1 - prev_index > 1 {
                    println!("\n... ... ...\n");
                }
            }
            println!("{}", lines[index]);
            prev_index = index + 1;
        }
    }
}

fn consume_specs_toml(filename: &str, specs: &mut Vec<Spec>)
{
    let path = Path::new(filename);
    let mut file = match File::open(&path) {
        Err(why) => { panic!("can't open {}: {}", filename, why.to_string()) },
        Ok(f) => f,
    };

    let content = match file.read_to_string() {
        Err(why) => { panic!("can't read {}: {}", filename, why.to_string()) },
        Ok(c) => c,
    };

    let mut parser = toml::Parser::new(content.as_slice());
    let table = match parser.parse() {
        Some(t) => t,
        None => { panic!("parse error in {}: {}", filename, parser.errors[0]) },
    };

    consume_specs_toml_table(&table, specs);
}

struct Spec
{
    disable: bool,
    start: Option<Regex>,
    stop: Option<Regex>,
    backward: bool,
}

impl Spec
{
    fn new() -> Self
    {
        Spec { disable: false, start: None, stop: None, backward: false }
    }
}

fn consume_specs_toml_table(table: &toml::Table, specs: &mut Vec<Spec>)
{
    use toml::Value::*;
    let mut spec = Spec::new();
    for (key, value) in table {
        match key.as_slice() {
            "disable" => {
                match *value {
                    Boolean(x) => { spec.disable = x },
                    _ => { panic!("`disable` key must be boolean") },
                }
            },
            "start" => {
                match *value {
                    String(ref rxs) => {
                        match Regex::new(rxs.as_slice()) {
                            Ok(rx) => { spec.start = Some(rx) },
                            Err(why) => { panic!("cant compile regex: {}", why.to_string()); },
                        }
                    },
                    _ => { panic!("`start` key must be regex string") },
                }
            },
            "stop" => {
                match *value {
                    String(ref rxs) => {
                        match Regex::new(rxs.as_slice()) {
                            Ok(rx) => { spec.stop = Some(rx) },
                            Err(why) => { panic!("cant compile regex: {}", why.to_string()); },
                        }
                    },
                    _ => { panic!("`stop` key must be regex string") },
                }
            },
            "direction" => {
                match *value {
                    String(ref s) if s.as_slice() == "forward" => { spec.backward = false },
                    String(ref s) if s.as_slice() == "backward" => { spec.backward = true },
                    _ => { panic!("`direction` must be either \"forward\" or \"backward\"") },
                }
            },
            _ => {
                match *value {
                    Table(ref t) => { consume_specs_toml_table(&t, specs) },
                    _ => { panic!("unrecognized key: {}", key) },
                }
            },
        }
    }

    if !spec.disable && spec.start.is_some() && (spec.stop.is_some() || false) {
        specs.push(spec);
    }
}

fn try_select(spec: &Spec, lines: &Vec<&str>, index: usize) -> Option<(usize, usize)>
{
    let step = if spec.backward { -1 } else { 1 };
    let mut cursor : isize = (index as isize) + step;
    while (cursor >= 0) && (cursor < lines.len() as isize) {
        match spec.stop {
            Some(ref rx) if rx.is_match(lines[cursor as usize]) => { return Some((index, cursor as usize)) },
            _ => {},
        };
        cursor += step;
    }
    return None
}

