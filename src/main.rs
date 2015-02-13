extern crate getopts;
extern crate toml;
use std::old_io::File;
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
    if matches.opt_present("h") || os::args().len() == 1 {
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
        spec.start = Some("\\berror\\b".to_string());
        specs.push(spec);
    }

    // perform
    println!("{} selectors", specs.len());
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
    start: Option<String>,
    stop: Option<String>,
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
                    String(ref rxs) => { spec.start = Some(rxs.clone()) },
                    _ => { panic!("`start` key must be regex string") },
                }
            },
            "stop" => {
                match *value {
                    String(ref rxs) => { spec.stop = Some(rxs.clone()) },
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
