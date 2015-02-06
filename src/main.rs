extern crate getopts;
use getopts::Options;
use std::os;

fn main() {
    println!("Hello, world!");
    let mut opts = Options::new();
    opts.optmulti("f", "filters", "TOML file with filter specifications", "fl");
    opts.optflag("h", "help", "print this help and exit");
    let matches = match opts.parse(os::args().tail()) {
        Ok(m) => { m }
        Err(f) => { panic!(f.to_string()) }
    }
    if matches.opt_present("h") {
        let brief = format!("Usage: {} [options]", os::args()[0]);
        println!("{}", opts.usage(brief.as_slice()));
        return;
    }
}

