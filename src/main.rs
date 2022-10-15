#![feature(strict_provenance)]

mod timekeeper;
mod rolling;

use tokio;
use clap::Parser;

#[derive(Parser)]
struct Cli {
    numerator: u8,
    denominator: u8,
    tempo: u8,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    println!("Time signature: {} / {}", cli.numerator, cli.denominator);
    println!("Tempo: {}", cli.tempo);

    let tk = timekeeper::Timekeeper::new(cli.numerator, cli.denominator, cli.tempo);
    tk.start();

}
