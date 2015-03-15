extern crate getopts;
extern crate regex;
extern crate toml;
use regex::Regex;
use std::collections;
use std::fs;
use std::iter;
use std::io;
use std::io::{Read, Write};

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
        println!("{}", getopts::usage(&brief[..], &opts));
        println!("\nIf `logfile` is not given, the standard input will be used.\n\nWhen no filter spec options are provided,\nlines containing the word \"error\" are selected.");
        return;
    }

    // parse toml config files
    let mut specs: Vec<Spec> = vec![];
    for filename in matches.opt_strs("f") {
        consume_specs_toml(&filename[..], &mut specs);
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

    logselect(&specs, &input_string[..], &mut io::stdout())
}

fn logselect(specs: &Vec<Spec>, content: &str, writer: &mut io::Write)
{
    let lines : Vec<&str> = iter::FromIterator::from_iter(content.lines_any());
    let mut selected_indexes = collections::BitVec::from_elem(lines.len(), false);
    for index in range(0, lines.len()) {
        for spec in specs.iter() {
            match spec.start {
                Some(ref rx) if rx.is_match(lines[index]) => {
                    let sel_range = if spec.stop.is_some() || spec.whale.is_some() { try_select(&spec, &lines, index as isize) } else { Some((index as isize,index as isize)) };
                    match sel_range {
                        Some((a0, b0)) => {
                            let (a, b) = (a0 + spec.start_offset, b0 + spec.stop_offset);

                            // std::cmp should have this function
                            fn clamp<T>(a: T, x: T, b: T) -> T where T: Ord { std::cmp::min(std::cmp::max(a, x), b) }
                            let last_index = (lines.len() - 1) as isize;
                            let (a, b) = (clamp(0, a, last_index), clamp(0, b, last_index));

                            // if after applying offsets the range remains nonempty
                            for i in (if a0 <= b0 { range(a, b+1) } else { range(b, a+1) } ) {
                                selected_indexes.set(i as usize, true);
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
    start_offset: isize,
    stop: Option<Regex>,
    stop_offset: isize,
    whale: Option<Regex>,
    backward: bool,
}

impl Spec
{
    fn new() -> Self
    {
        Spec { disable: false, start: None, start_offset: 0, stop: None, stop_offset: 0, whale: None, backward: false }
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
            "start_offset" => match *value {
                Integer(ofs) => { spec.start_offset = ofs as isize; },
                _ => { panic!("`start_offset` must be integer") },
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
            "stop_offset" => match *value {
                Integer(ofs) => { spec.stop_offset = ofs as isize; },
                _ => { panic!("`stop_offset` must be integer") },
            },
            "while" => {
                match *value {
                    String(ref rxs) => {
                        match Regex::new(&rxs[..]) {
                            Ok(rx) => { spec.whale = Some(rx) },
                            Err(why) => { panic!("cant compile regex: {}", why.to_string()); },
                        }
                    },
                    _ => { panic!("`while` key must be regex string") },
                }
            },
            "direction" => {
                match *value {
                    String(ref s) => match &s[..] {
                        "forward" | "fwd" | "down" => { spec.backward = false },
                        "backward" | "backwards" | "back" | "up" => { spec.backward = true },
                        ss => { panic!("`direction` value '{}' unrecognized (must be 'forward' or 'backward')", ss) },
                    },
                    _ => { panic!("`direction` must be a string") },
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

    if !spec.disable && spec.start.is_some() {
        specs.push(spec);
    }
}

fn try_select(spec: &Spec, lines: &Vec<&str>, index: isize) -> Option<(isize, isize)>
{
    let step = if spec.backward { -1 } else { 1 };
    let mut cursor = index + step;
    while (cursor >= 0) && (cursor < lines.len() as isize) {
        match spec.stop {
            Some(ref rx) if rx.is_match(lines[cursor as usize]) => { return Some((index, cursor)) },
            _ => {},
        };
        match spec.whale {
            Some(ref rx) if !rx.is_match(lines[cursor as usize]) => { return Some((index, cursor-step)) },
            _ => {},
        };
        cursor += step;
    }
    match spec.whale {
        Some(ref rx) => { return Some((index, cursor-step)) },
        _ => { return None },
    };
}

#[test]
fn test_all()
{
    let mut sample_content = String::new();
    fs::File::open(&Path::new("tests/data/sample.txt")).unwrap().read_to_string(&mut sample_content).unwrap();
    let sample_content = sample_content; // make immutable

    let mut failed_files = Vec::<String>::new();
    println!(""); // cargo test prepends tab to the first line, but not the rest

    for entry in std::fs::read_dir(&Path::new("tests/data")).unwrap() {
        let entry_path = entry.unwrap().path();
        if entry_path.extension().unwrap().to_str().unwrap() == "toml" {
            let mut specs: Vec<Spec> = vec![];
            let toml_path_s = entry_path.clone().into_os_string().into_string().unwrap();
            print!("testing {} ... ", toml_path_s);
            io::stdout().flush();
            consume_specs_toml(&toml_path_s[..], &mut specs);

            let expected_content_path = entry_path.with_extension("txt");
            let expected_content_path_str = expected_content_path.clone().into_os_string().into_string().unwrap();
            let mut expected_content = String::new();
            match fs::File::open(&expected_content_path) {
                Err(err) => { panic!("{}: can not open file {}: {}", toml_path_s, expected_content_path_str, err); },
                Ok(ref mut f) => { f.read_to_string(&mut expected_content).unwrap(); },
            };

            let mut output = Vec::<u8>::new();
            logselect(&specs, &sample_content, &mut output);

            if (&expected_content.as_bytes() == &output) {
                println!("+");
            } else {
                failed_files.push(toml_path_s);
                println!("fail\n\t{} spec(s) recognized\n--- expected ---\n{}\n--- actual ---", specs.len(), &expected_content[..]);
                println!("{}", std::str::from_utf8(&output).unwrap());
                println!("--- end ---");
            }
        }
    }
    if failed_files.len() > 0 {
        println!("Summary of failed files:");
        for ffn in failed_files { println!("    {}", ffn); }
        panic!();
    }
}
