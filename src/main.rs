mod timekeeper;

use std::{thread, time};

use clap::Parser;
#[derive(Parser)]
struct Cli {
    numerator: u8,
    denominator: u8,
    tempo: u8,
}

fn main() {
    let cli = Cli::parse();
    println!("Time signature: {} / {}", cli.numerator, cli.denominator);
    println!("Tempo: {}", cli.tempo);
    let tk = timekeeper::Timekeeper::new(cli.numerator, cli.denominator, cli.tempo);
    tk.start();
    let dur = time::Duration::from_millis(100);
    loop {
        thread::sleep(dur);
    }
}
