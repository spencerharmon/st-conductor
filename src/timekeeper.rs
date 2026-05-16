use jack::jack_sys as j;
use st_sync;
use st_lib::jack_ptr;
use crate::rolling::jack_transport_rolling;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{thread, time::Duration};

/// Payload handed to JACK's timebase callback via the `arg` pointer.
///
/// JACK stores the `*mut c_void` we give it and passes it back on each
/// invocation. We leak a `Box<TimebasePayload>` into that pointer; the
/// payload owns a clone of the `Arc<AtomicU64>` shared with the poll loop.
struct TimebasePayload {
	numerator: f32,
	denominator: f32,
	tempo: f64,
	next_beat_frame: Arc<AtomicU64>,
}

unsafe extern "C" fn timebase_callback(
	state: j::jack_transport_state_t,
	nframes: j::jack_nframes_t,
	pos: *mut j::jack_position_t,
	_new_pos: ::libc::c_int,
	arg: *mut ::libc::c_void,
) {
	// The payload was leaked at registration time; we only borrow it here
	// for the duration of the callback. We never drop it.
	let payload = &*(arg as *const TimebasePayload);

	if (*pos).frame == 0 {
		(*pos).beats_per_bar = payload.numerator;
		(*pos).beat_type = payload.denominator;
		(*pos).beats_per_minute = payload.tempo;
		(*pos).bar = 1;
		(*pos).beat = 1;
		(*pos).tick = 0;
	} else {
		match state {
			j::JackTransportStopped => {
				println!("Stopped");
			}
			j::JackTransportRolling => {
				jack_transport_rolling(
					payload.numerator,
					payload.denominator,
					payload.tempo,
					nframes,
					pos,
					&payload.next_beat_frame,
				);
			}
			j::JackTransportStarting => {
				println!("Starting");
			}
			_ => (),
		}
	}
}

/// Handle to a running [`Timekeeper`] that the GUI thread can use to read
/// shared state without touching JACK's audio thread directly.
///
/// All fields are cheaply cloneable. The handle is `Send + Sync`; the
/// `client_addr` is the exposed-provenance address of the JACK client
/// pointer (see [`st_lib::jack_ptr`]) and is safe to read from any
/// thread, since `jack_transport_query` is documented as thread-safe.
#[derive(Clone)]
pub struct TimekeeperHandle {
	pub numerator: u16,
	pub denominator: u16,
	pub tempo: u16,
	/// Shared next-beat-frame slot, updated each timebase-callback cycle.
	pub next_beat_frame: Arc<AtomicU64>,
	/// Exposed-provenance address of the JACK client. Recover with
	/// [`st_lib::jack_ptr::recover_client`].
	pub client_addr: usize,
}

pub struct Timekeeper {
	numerator: u16,
	denominator: u16,
	tempo: u16,
}

impl Timekeeper {
	pub fn new(numerator: u16, denominator: u16, tempo: u16) -> Timekeeper {
		Timekeeper { numerator, denominator, tempo }
	}

	/// Bring up JACK, register the timebase callback, spawn the st-sync
	/// poll loop on a background thread, and return a handle the GUI
	/// (or any other thread) can use to read shared state.
	///
	/// The returned handle keeps the `Arc<AtomicU64>` alive; the JACK
	/// client itself is intentionally leaked along with the active-client
	/// guard so it lives for the rest of the process. st-conductor runs
	/// until killed, so this is the right shape.
	pub fn start(&self) -> TimekeeperHandle {
		let (client, _status) =
			jack::Client::new("st-conductor", jack::ClientOptions::NO_START_SERVER).unwrap();

		let cb: j::TimebaseCallback = Some(timebase_callback);

		let sync_controller = st_sync::controller::Controller::new();

		// Heap-allocated, atomically-updated next-beat-frame slot, shared
		// between the JACK timebase callback (producer) and the async poll
		// loop (consumer). Relaxed ordering matches st_sync's "latest wins"
		// semantics.
		let next_beat_frame = Arc::new(AtomicU64::new(0));

		let payload = Box::new(TimebasePayload {
			numerator: self.numerator as f32,
			denominator: self.denominator as f32,
			tempo: self.tempo as f64,
			next_beat_frame: Arc::clone(&next_beat_frame),
		});
		// Leak the payload so JACK can hold a stable pointer to it for the
		// lifetime of the timebase registration. This is a small, one-time
		// leak per process; the conductor runs until killed.
		let payload_ptr: *mut TimebasePayload = Box::into_raw(payload);
		let arg = payload_ptr as *mut ::libc::c_void;

		let client_addr = jack_ptr::expose_client(client.raw());

		unsafe {
			j::jack_engine_takeover_timebase(client.raw());
			j::jack_set_timebase_callback(client.raw(), 0, cb, arg);

			let mut pos: MaybeUninit<j::jack_position_t> = MaybeUninit::uninit();
			j::jack_transport_query(client.raw(), pos.as_mut_ptr());
			(*pos.as_mut_ptr()).frame = 0;
			j::jack_transport_reposition(client.raw(), pos.as_mut_ptr());
			j::jack_transport_stop(client.raw());
		}

		let process = jack::ClosureProcessHandler::new(
			move |_: &jack::Client, _ps: &jack::ProcessScope| -> jack::Control {
				jack::Control::Continue
			},
		);

		let active_client = client.activate_async((), process).unwrap();
		// Leak the active client so it (and the underlying jack::Client)
		// lives for the rest of the process. Without this the client is
		// dropped at the end of `start`, which would deactivate JACK.
		std::mem::forget(active_client);

		// Spawn the st-sync poll loop on a dedicated thread so `start`
		// can return a handle to the caller (the GUI thread).
		let nbf = Arc::clone(&next_beat_frame);
		thread::Builder::new()
			.name("st-conductor-sync-poll".into())
			.spawn(move || {
				let mut last_val: u64 = 0;
				let mut skip = true;
				loop {
					thread::sleep(Duration::from_millis(10));
					let val = nbf.load(Ordering::Relaxed);
					if val != last_val {
						last_val = val;
						if !skip {
							sync_controller.send_next_beat_frame(val);
						}
						skip = false;
					}
				}
			})
			.expect("failed to spawn st-sync poll thread");

		TimekeeperHandle {
			numerator: self.numerator,
			denominator: self.denominator,
			tempo: self.tempo,
			next_beat_frame,
			client_addr,
		}
	}
}
