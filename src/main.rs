
mod timekeeper;
mod rolling;
mod session;

use std::env;
use std::sync::{Arc, Mutex};

use clap::Parser;
use st_lib::nsm;

#[derive(Parser)]
struct Cli {
	/// Beats per bar (time signature numerator).
	numerator: u16,
	/// Beat unit (time signature denominator).
	denominator: u16,
	/// Tempo in BPM.
	tempo: u16,
}

/// Live transport config, shared with the NSM follow-up task so that
/// Save reflects whatever the conductor is currently broadcasting.
/// Numerator / denominator / tempo are not yet user-editable at runtime;
/// once a control channel or GUI lands, this is the slot to update.
#[derive(Clone)]
struct LiveConfig(Arc<Mutex<session::Session>>);

#[tokio::main]
async fn main() {
	let cli = Cli::parse();
	let live = LiveConfig(Arc::new(Mutex::new(session::Session {
		numerator: cli.numerator,
		denominator: cli.denominator,
		tempo: cli.tempo,
	})));

	// If NSM_URL is set, wait for the first /nsm/client/open before
	// activating JACK so a saved tempo/meter can replace the CLI defaults.
	if env::var("NSM_URL").is_ok() {
		let caps = nsm::Capabilities {
			switch: true,
			optional_gui: true,
			..Default::default()
		};
		let (mut client, _handle) = nsm::Builder::new("st-conductor")
			.capabilities(caps)
			.launch();

		println!("[st-conductor] NSM detected, waiting for /nsm/client/open ...");
		let session_path = Arc::new(Mutex::new(String::new()));

		while let Some(evt) = client.rx.recv().await {
			match evt {
				nsm::Event::Open { path, ack, .. } => {
					match session::load(&path) {
						Ok(Some(s)) => {
							println!("[st-conductor] loaded session: {s:?}");
							*live.0.lock().unwrap() = s;
						}
						Ok(None) => {
							println!("[st-conductor] no saved session at {path}, using CLI defaults");
							// Write the defaults so next Save is consistent.
							let snap = live.0.lock().unwrap().clone();
							if let Err(e) = session::save(&path, &snap) {
								eprintln!("[st-conductor] could not seed session: {e}");
							}
						}
						Err(e) => {
							ack.err(-1, format!("load failed: {e}"));
							eprintln!("[st-conductor] session load error: {e}");
							return;
						}
					}
					*session_path.lock().unwrap() = path;
					ack.ok("opened");
					break;
				}
				nsm::Event::AnnounceError { code, message } => {
					eprintln!("[st-conductor] NSM rejected announce ({code}): {message}");
					return;
				}
				nsm::Event::AnnounceOk { manager_name, .. } => {
					println!("[st-conductor] NSM connected to {manager_name}");
				}
				_ => {}
			}
		}

		spawn_nsm_followup(client, session_path, live.clone());
	}

	let snap = live.0.lock().unwrap().clone();
	println!("Time signature: {} / {}", snap.numerator, snap.denominator);
	println!("Tempo: {}", snap.tempo);

	let tk = timekeeper::Timekeeper::new(snap.numerator, snap.denominator, snap.tempo);
	// Run on a dedicated thread so main()'s tokio runtime stays alive
	// for the NSM follow-up task.
	std::thread::spawn(move || tk.start())
		.join()
		.expect("timekeeper thread panicked");
}

/// After the initial Open, keep listening for Save / re-Open / GUI events.
fn spawn_nsm_followup(
	mut client: nsm::Client,
	session_path: Arc<Mutex<String>>,
	live: LiveConfig,
) {
	tokio::spawn(async move {
		while let Some(evt) = client.rx.recv().await {
			match evt {
				nsm::Event::Save { ack } => {
					let path = session_path.lock().unwrap().clone();
					if path.is_empty() {
						ack.err(-1, "no session path");
						continue;
					}
					let snap = live.0.lock().unwrap().clone();
					match session::save(&path, &snap) {
						Ok(()) => ack.ok("saved"),
						Err(e) => ack.err(-1, format!("save failed: {e}")),
					}
				}
				nsm::Event::Open { path, ack, .. } => {
					// `:switch:` — load the new session's values. Tempo /
					// meter changes cannot yet be applied to a running
					// JACK timebase (no live control channel); we update
					// LiveConfig so the next Save reflects the new path's
					// contents, but the running transport keeps the
					// previously-loaded values.
					match session::load(&path) {
						Ok(Some(s)) => *live.0.lock().unwrap() = s,
						Ok(None)    => { /* new session, keep current values */ }
						Err(e) => {
							ack.err(-1, format!("load failed: {e}"));
							continue;
						}
					}
					*session_path.lock().unwrap() = path;
					ack.ok("switched (live tempo change requires restart)");
				}
				nsm::Event::ShowGui | nsm::Event::HideGui => {
					// No GUI yet. We still advertise :optional-gui: so the
					// NSM host shows the toggle once a GUI lands.
				}
				_ => {}
			}
		}
	});
}
