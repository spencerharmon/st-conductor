//! GUI glue for st-conductor.
//!
//! Owns the `App` struct, the `eframe::App` impl, the per-frame
//! drain/refresh helpers, and the `run()` launcher. No business logic
//! lives here — see `transport_panel`, `beat_indicator`, etc.

pub mod app_state;
pub mod beat_indicator;
pub mod transport_panel;

use eframe::egui;
use std::sync::atomic::Ordering;
use std::time::Duration;

use app_state::AppState;
use crate::timekeeper::TimekeeperHandle;
use st_lib::{jack_ptr, jack_transport};

pub struct App {
	state: AppState,
}

impl App {
	pub fn new(tk: TimekeeperHandle) -> Self {
		Self { state: AppState::new(tk) }
	}

	/// Pull the latest transport snapshot from JACK and notice if the
	/// conductor has advanced to a new beat. Called once per frame.
	fn refresh(&mut self) {
		// Transport snapshot via the shared client pointer. The pointer
		// lives for the lifetime of the process (timekeeper leaks the
		// active client on purpose).
		let snap = unsafe {
			let client = jack_ptr::recover_client(self.state.tk.client_addr);
			jack_transport::query_transport(client)
		};
		self.state.transport = app_state::TransportView {
			state: snap.state,
			bar: snap.bar,
			beat: snap.beat,
			tick: snap.tick,
			frame: snap.frame,
			frame_rate: snap.frame_rate,
			beats_per_minute: snap.beats_per_minute,
			beats_per_bar: snap.beats_per_bar,
		};

		// Beat-edge detection: the timebase callback writes a new value
		// into `next_beat_frame` exactly when it crosses a beat boundary.
		let nbf = self.state.tk.next_beat_frame.load(Ordering::Relaxed);
		if nbf != self.state.last_next_beat_frame {
			self.state.last_next_beat_frame = nbf;
			self.state.last_beat_at = std::time::Instant::now();
		}
	}
}

impl eframe::App for App {
	fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
		self.refresh();

		let ctx = ui.ctx().clone();

		egui::Panel::bottom("status_bar").show_inside(ui, |ui| {
			ui.horizontal(|ui| {
				ui.label("st-conductor");
				ui.separator();
				ui.label(self.state.transport.state_label());
			});
		});

		ui.horizontal(|ui| {
			beat_indicator::show(ui, &self.state);
			ui.add_space(8.0);
			ui.heading("Conductor");
		});
		ui.separator();
		transport_panel::show(ui, &self.state);

		// Repaint frequently enough that the beat-indicator fade is smooth
		// and the BBT readout stays current. 30ms ≈ 33Hz; cheap.
		ctx.request_repaint_after(Duration::from_millis(30));
	}
}

/// Launch the eframe window. Blocks the calling thread until the user
/// closes the window.
pub fn run(tk: TimekeeperHandle) -> Result<(), eframe::Error> {
	let options = eframe::NativeOptions {
		viewport: egui::ViewportBuilder::default()
			.with_inner_size([520.0, 320.0])
			.with_title("st-conductor"),
		..Default::default()
	};

	eframe::run_native(
		"st-conductor",
		options,
		Box::new(move |cc| {
			cc.egui_ctx.set_visuals(egui::Visuals::dark());
			Ok(Box::new(App::new(tk)))
		}),
	)
}
