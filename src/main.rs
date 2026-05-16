mod timekeeper;
mod rolling;
mod gui;

use clap::Parser;

#[derive(Parser)]
struct Cli {
	numerator: u16,
	denominator: u16,
	tempo: u16,
	/// Run headless (no GUI window). Useful for scripted / NSM-driven runs
	/// where the GUI would just get in the way.
	#[clap(long)]
	no_gui: bool,
}

fn main() {
	let cli = Cli::parse();
	println!("Time signature: {} / {}", cli.numerator, cli.denominator);
	println!("Tempo: {}", cli.tempo);

	// Build the tokio runtime up-front and keep it alive for the rest of
	// the process. `Timekeeper::start` calls into `st_sync::Controller`,
	// which spawns tokio tasks; that must happen inside a runtime context.
	let rt = tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.build()
		.expect("failed to build tokio runtime");

	let tk = timekeeper::Timekeeper::new(cli.numerator, cli.denominator, cli.tempo);
	let handle = rt.block_on(async { tk.start() });

	if cli.no_gui {
		// Headless: the st-sync poll thread is already spawned inside
		// `Timekeeper::start`. Park the main thread forever so JACK
		// (and the tokio runtime) keep running.
		loop {
			std::thread::park();
		}
	}

	// Keep the runtime alive while the GUI runs.
	let _rt_guard = rt.enter();

	if let Err(e) = gui::run(handle) {
		eprintln!("eframe exited with error: {e}");
		std::process::exit(1);
	}

	// Explicit: keep `rt` in scope until after eframe returns.
	drop(rt);
}
