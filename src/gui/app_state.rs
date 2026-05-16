//! GUI app state for st-conductor.
//!
//! All widget state lives here, per the convention captured in plan.org.
//! Widgets are pure renderers; nothing persists inside them.

use crate::timekeeper::TimekeeperHandle;
use jack::jack_sys as j;

/// Snapshot of transport state read from JACK once per GUI frame.
///
/// Refreshed in `App::ui` via `st_lib::jack_transport::query_transport`.
#[derive(Debug, Clone, Copy, Default)]
pub struct TransportView {
	pub state: j::jack_transport_state_t,
	pub bar: i32,
	pub beat: i32,
	pub tick: i32,
	pub frame: u64,
	pub frame_rate: u32,
	pub beats_per_minute: f64,
	pub beats_per_bar: f32,
}

impl TransportView {
	pub fn state_label(&self) -> &'static str {
		match self.state {
			j::JackTransportStopped => "Stopped",
			j::JackTransportRolling => "Rolling",
			j::JackTransportStarting => "Starting",
			_ => "Unknown",
		}
	}
}

pub struct AppState {
	/// Handle to the running Timekeeper. Lets the GUI read the JACK
	/// client (for transport queries) and the shared next-beat-frame slot.
	pub tk: TimekeeperHandle,
	/// Latest transport snapshot, refreshed each frame.
	pub transport: TransportView,
	/// Last `next_beat_frame` we observed. Used by the beat indicator to
	/// detect the moment the conductor advanced to a new beat.
	pub last_next_beat_frame: u64,
	/// Wall-clock instant of the most recent beat boundary, used to fade
	/// the beat-indicator lamp.
	pub last_beat_at: std::time::Instant,
}

impl AppState {
	pub fn new(tk: TimekeeperHandle) -> Self {
		Self {
			tk,
			transport: TransportView::default(),
			last_next_beat_frame: 0,
			last_beat_at: std::time::Instant::now() - std::time::Duration::from_secs(10),
		}
	}
}
