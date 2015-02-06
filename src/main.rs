extern crate getopts;
use std::os;

fn main() {
    let opts = [
        getopts::optmulti("f", "filters", "TOML file with filter specifications", "fl"),
        getopts::optflag("h", "help", "print this help and exit"),
    ];
    let matches = match getopts::getopts(os::args().tail(), &opts) {
        Ok(m) => { m }
        Err(f) => { panic!(f.to_string()) }
    };
    if matches.opt_present("h") || os::args().len() == 1 {
        let brief = format!("Usage: {} [options]", os::args()[0]);
        println!("{}", getopts::usage(brief.as_slice(), &opts));
        return;
    }
    println!("Hello, world!");
}

