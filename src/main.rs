extern crate getopts;
extern crate regex;
extern crate toml;
use regex::Regex;
use std::collections;
use std::fs;
use std::iter;
use std::io;
use std::io::{Read};

#[cfg(test)]
use std::ffi::AsOsStr;

fn main()
{
    let args : Vec<_> = std::env::args().collect();

    // cli opts
    let opts = [
        getopts::optmulti("f", "filters", "TOML file(s) with filter specifications", "specs.toml"),
        getopts::optflag("h", "help", "print this help message and exit"),
    ];
    let matches = match getopts::getopts(args.tail(), &opts) {
        Ok(m) => { m }
        Err(f) => { panic!(f.to_string()) }
    };
    if matches.opt_present("h") {
        let brief = format!("Usage: {} [options] [logfile]", args[0]);
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
        0 => {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer).unwrap();
            buffer
        },
        1 => {
            let path = Path::new(matches.free[0].clone());
            match fs::File::open(&path) {
                Err(why) => { panic!("can't open {}: {}", matches.free[0], why.to_string()) },
                Ok(ref mut f) => {
                    let mut buffer = String::new();
                    f.read_to_string(&mut buffer).unwrap();
                    buffer
                },
            }
        },
        _ => { panic!("too many filename arguments ({}), expected just one", matches.free.len()) },
    };

    logselect(&specs, input_string.as_slice(), &mut io::stdout())
}

fn logselect(specs: &Vec<Spec>, content: &str, writer: &mut io::Write)
{
    let lines : Vec<&str> = iter::FromIterator::from_iter(content.lines_any());
    let mut selected_indexes = collections::BitVec::new();
    for index in range(0, lines.len()) {
        for spec in specs.iter() {
            match spec.start {
                Some(ref rx) if rx.is_match(lines[index]) => {
                    match try_select(&spec, &lines, index) {
                        Some((a, b)) => {
                            for i in (if a < b { range(a, b+1) } else { range(b, a+1) } ) {
                                selected_indexes.set(i, true);
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
        if selected_indexes[index] {
            if prev_index > 0 {
                if index + 1 - prev_index > 1 {
                    writer.write(b"\n... ... ...\n\n").unwrap();
                }
            }
            writer.write(lines[index].as_bytes()).unwrap();
            writer.write(b"\n").unwrap();
            prev_index = index + 1;
        }
    }
}

fn consume_specs_toml(filename: &str, specs: &mut Vec<Spec>)
{
    let path = Path::new(filename);
    let mut file = match fs::File::open(&path) {
        Err(why) => { panic!("can't open {}: {}", filename, why.to_string()) },
        Ok(f) => f,
    };

    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();

    let mut parser = toml::Parser::new(&content[..]);
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
        match &key[..] {
            "disable" => {
                match *value {
                    Boolean(x) => { spec.disable = x },
                    _ => { panic!("`disable` key must be boolean") },
                }
            },
            "start" => {
                match *value {
                    String(ref rxs) => {
                        match Regex::new(&rxs[..]) {
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
                        match Regex::new(&rxs[..]) {
                            Ok(rx) => { spec.stop = Some(rx) },
                            Err(why) => { panic!("cant compile regex: {}", why.to_string()); },
                        }
                    },
                    _ => { panic!("`stop` key must be regex string") },
                }
            },
            "direction" => {
                match *value {
                    String(ref s) if &s[..] == "forward" => { spec.backward = false },
                    String(ref s) if &s[..] == "backward" => { spec.backward = true },
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

/*
#[test]
fn test_all()
{
    for entry in std::fs::read_dir(&Path::new("tests/data")).unwrap() {
        let entry_path = entry.unwrap().path();
        if entry_path.extension().unwrap().to_str().unwrap() == "toml" {
            let args : Vec<String> = vec!["logselect".to_string(), "-f".to_string(), entry_path.into_os_string().into_string().unwrap(), "tests/data/sample.txt".to_string()];
            logselect(&args);
        }
    }
}
*/
