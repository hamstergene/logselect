extern crate fixedbitset;
extern crate getopts;
extern crate libc;
extern crate regex;
extern crate toml;
use regex::Regex;
use std::fs;
use std::io;
use std::io::{Read, Write, BufRead};
use std::path;
use std::sync;
use std::sync::mpsc;
use std::thread;

#[cfg(not(test))]
fn main()
{
    let args : Vec<_> = std::env::args().collect();

    // cli opts
    let mut opts = getopts::Options::new();
    opts.optmulti("f", "filters", "TOML file(s) with filter specifications", "specs.toml");
    opts.optflag("h", "help", "print this help message and exit");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => { m }
        Err(f) => { panic!(f.to_string()) }
    };
    if matches.opt_present("h") {
        let brief = format!("Usage: {} [options] [logfile]", args[0]);
        println!("{}", opts.usage(&brief[..]));
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
    let lines = match matches.free.len() {
        0 => { read_lines(&mut io::stdin()) }
        1 => {
            let path = path::Path::new(&matches.free[0]);
            match fs::File::open(&path) {
                Err(why) => { panic!("can't open {}: {}", matches.free[0], why.to_string()) },
                Ok(ref mut f) => { read_lines(f) },
            }
        }
        _ => { panic!("too many filename arguments ({}), expected just one", matches.free.len()) }
    };

    logselect(specs, lines, &mut io::stdout())
}

fn read_lines(reader: &mut io::Read) -> Vec<String>
{
    let mut rv = Vec::new();
    for line_res in io::BufReader::new(reader).lines() {
        rv.push(line_res.unwrap());
    }
    return rv
}

fn logselect(specs: Vec<Spec>, lines: Vec<String>, writer: &mut io::Write)
{
    let work = Work { lines : lines, specs : specs, index : sync::Mutex::new(0) };
    let work = sync::Arc::new(work);

    let (sender, receiver) = mpsc::channel();
    let num_cpus = num_cpus();
    let mut threads = Vec::with_capacity(num_cpus);
    for _ in 0..threads.capacity() {
        let sender = sender.clone();
        let work = work.clone();
        threads.push(thread::spawn(move|| {
            loop {
                let i = {
                    let mut p = work.index.lock().unwrap();
                    let rv = *p;
                    *p += 1;
                    rv as usize
                };
                if i >= work.specs.len() {
                    sender.send( (-1, -1) ).unwrap();
                    break;
                }
                process_spec(&work.specs[i], &work.lines, &sender);
            }
        }));
    }

    let mut selected_indexes = fixedbitset::FixedBitSet::with_capacity(work.lines.len());
    let mut num_finished = 0;
    while num_finished < threads.len() {
        match receiver.recv().unwrap() {
            (-1,-1) => { num_finished += 1 }
            (a,b) => for i in a..b {
                selected_indexes.set(i as usize, true);
            }
        }
    }

    // output
    let mut prev_index = 0;
    for index in 0..work.lines.len() {
        if selected_indexes[index] {
            if prev_index > 0 {
                if index + 1 - prev_index > 1 {
                    writer.write(b"\n... ... ...\n\n").unwrap();
                }
            }
            writer.write(work.lines[index].as_bytes()).unwrap();
            writer.write(b"\n").unwrap();
            prev_index = index + 1;
        }
    }
}

pub fn num_cpus() -> usize {
    unsafe {
        return rust_get_num_cpus() as usize;
    }

    extern {
        fn rust_get_num_cpus() -> libc::uintptr_t;
    }
}

struct Work
{
    lines: Vec<String>,
    specs: Vec<Spec>,
    index: sync::Mutex<isize>,
}

fn process_spec(spec: &Spec, lines: &Vec<String>, sender: &mpsc::Sender<(isize, isize)>)
{
    if let Some(ref rx) = spec.start {
        for index in 0..lines.len() {
            if rx.is_match(&lines[index][..]) {
                let sel_range = if spec.stop.is_some() || spec.whale.is_some() { try_select(&spec, lines, index as isize) } else { Some((index as isize,index as isize)) };
                if let Some((a0,b0)) = sel_range {
                    let (a, b) = (a0 + spec.start_offset, b0 + spec.stop_offset);

                    // std::cmp should have this function
                    fn clamp<T>(a: T, x: T, b: T) -> T where T: Ord { std::cmp::min(std::cmp::max(a, x), b) }
                    let last_index = (lines.len() - 1) as isize;
                    let (a, b) = (clamp(0, a, last_index), clamp(0, b, last_index));

                    // if after applying offsets the range remains nonempty
                    if a0 <= b0 { 
                        sender.send( (a, b+1) ).unwrap()
                    } else { 
                        sender.send( (b, a+1) ).unwrap()
                    }
                }
            }
        }
    }
}

fn consume_specs_toml(filename: &str, specs: &mut Vec<Spec>)
{
    let path = path::Path::new(filename);
    let mut file = match fs::File::open(&path) {
        Err(why) => { panic!("can't open {}: {}", filename, why.to_string()) }
        Ok(f) => f
    };

    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();

    let mut parser = toml::Parser::new(&content[..]);
    let table = match parser.parse() {
        Some(t) => { t }
        None => { panic!("parse error in {}: {}", filename, parser.errors[0]) }
    };

    consume_specs_toml_table(&table, specs);
}

#[derive(Clone)]
struct Spec
{
    disable: bool,
    start: Option<Regex>,
    start_offset: isize,
    stop: Option<Regex>,
    stop_offset: isize,
    whale: Option<Regex>,
    backward: bool,
    limit: isize,
}

impl Spec
{
    fn new() -> Self
    {
        Spec { disable: false, start: None, start_offset: 0, stop: None, stop_offset: 0, whale: None, backward: false, limit: 1000 }
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
                    Boolean(x) => { spec.disable = x }
                    _ => { panic!("`disable` key must be boolean") }
                }
            }
            "start" => {
                match *value {
                    String(ref rxs) => {
                        match Regex::new(&rxs[..]) {
                            Ok(rx) => { spec.start = Some(rx) }
                            Err(why) => { panic!("cant compile regex: {}", why.to_string()); }
                        }
                    }
                    _ => { panic!("`start` key must be regex string") }
                }
            }
            "start_offset" => { match *value {
                Integer(ofs) => { spec.start_offset = ofs as isize; }
                _ => { panic!("`start_offset` must be integer") }
            } }
            "stop" => {
                match *value {
                    String(ref rxs) => {
                        match Regex::new(&rxs[..]) {
                            Ok(rx) => { spec.stop = Some(rx) }
                            Err(why) => { panic!("cant compile regex: {}", why.to_string()); }
                        }
                    }
                    _ => { panic!("`stop` key must be regex string") }
                }
            }
            "stop_offset" => { match *value {
                Integer(ofs) => { spec.stop_offset = ofs as isize; }
                _ => { panic!("`stop_offset` must be integer") }
            } }
            "while" => {
                match *value {
                    String(ref rxs) => {
                        match Regex::new(&rxs[..]) {
                            Ok(rx) => { spec.whale = Some(rx) }
                            Err(why) => { panic!("cant compile regex: {}", why.to_string()); }
                        }
                    }
                    _ => { panic!("`while` key must be regex string") }
                }
            }
            "direction" => {
                match *value {
                    String(ref s) => { match &s[..] {
                        "forward" | "fwd" | "down" => { spec.backward = false }
                        "backward" | "backwards" | "back" | "up" => { spec.backward = true }
                        ss => { panic!("`direction` value '{}' unrecognized (must be 'forward' or 'backward')", ss) }
                    } }
                    _ => { panic!("`direction` must be a string") }
                }
            }
            "limit" => { match *value {
                Integer(lim) if lim > 0 => { spec.limit = lim as isize; }
                _ => { panic!("`limit` must be a positive integer") }
            } }
            _ => { match *value {
                Table(ref t) => { consume_specs_toml_table(&t, specs) }
                _ => { panic!("unrecognized key: {}", key) }
            } }
        }
    }

    if !spec.disable && spec.start.is_some() {
        specs.push(spec);
    }
}

fn try_select(spec: &Spec, lines: &Vec<String>, index: isize) -> Option<(isize, isize)>
{
    let step = if spec.backward { -1 } else { 1 };
    let mut cursor = index + step;
    while (cursor >= 0) && (cursor < lines.len() as isize) && (cursor - index).abs() <= spec.limit  {
        match spec.stop {
            Some(ref rx) if rx.is_match(&lines[cursor as usize][..]) => { return Some((index, cursor)) }
            _ => {}
        };
        match spec.whale {
            Some(ref rx) if !rx.is_match(&lines[cursor as usize][..]) => { return Some((index, cursor-step)) }
            _ => {}
        };
        cursor += step;
    }
    match spec.whale {
        Some(_) => { return Some((index, cursor-step)) }
        _ => { return None }
    };
}

#[test]
fn test_all()
{
    let sample_lines = read_lines(&mut fs::File::open(&path::Path::new("tests/data/sample.txt")).unwrap());

    let mut failed_files = Vec::<String>::new();
    println!(""); // cargo test prepends tab to the first line, but not the rest

    for entry in std::fs::read_dir(&path::Path::new("tests/data")).unwrap() {
        let entry_path = entry.unwrap().path();
        if entry_path.extension().unwrap().to_str().unwrap() == "toml" {
            let mut specs: Vec<Spec> = vec![];
            let toml_path_s = entry_path.clone().into_os_string().into_string().unwrap();
            print!("testing {} ... ", toml_path_s);
            let _ = io::stdout().flush();
            consume_specs_toml(&toml_path_s[..], &mut specs);

            let expected_content_path = entry_path.with_extension("txt");
            let expected_content_path_str = expected_content_path.clone().into_os_string().into_string().unwrap();
            let mut expected_content = String::new();
            match fs::File::open(&expected_content_path) {
                Err(err) => { panic!("{}: can not open file {}: {}", toml_path_s, expected_content_path_str, err); }
                Ok(ref mut f) => { f.read_to_string(&mut expected_content).unwrap(); }
            };

            let mut output = Vec::<u8>::new();
            logselect(specs.clone(), sample_lines.clone(), &mut output);

            if expected_content.as_bytes() == &output[..] {
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
