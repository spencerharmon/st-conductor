//! Read-only transport panel: tempo, time signature, BBT, transport state.

use eframe::egui::{self, Ui};
use crate::gui::app_state::AppState;

pub fn show(ui: &mut Ui, state: &AppState) {
	ui.heading("Transport");
	ui.add_space(4.0);

	egui::Grid::new("transport_grid")
		.num_columns(2)
		.spacing([16.0, 4.0])
		.show(ui, |ui| {
			ui.label("State:");
			ui.label(state.transport.state_label());
			ui.end_row();

			ui.label("Tempo:");
			ui.label(format!("{:.2} BPM", state.transport.beats_per_minute));
			ui.end_row();

			ui.label("Time signature:");
			ui.label(format!("{} / {}", state.tk.numerator, state.tk.denominator));
			ui.end_row();

			ui.label("BBT:");
			ui.label(format!(
				"{} | {} | {:>4}",
				state.transport.bar, state.transport.beat, state.transport.tick
			));
			ui.end_row();

			ui.label("Frame:");
			ui.label(format!(
				"{} @ {} Hz",
				state.transport.frame, state.transport.frame_rate
			));
			ui.end_row();

			ui.label("Next beat frame:");
			ui.label(format!("{}", state.last_next_beat_frame));
			ui.end_row();
		});
}
