use jack::jack_sys as j;
use ::libc::*;
use crate::rolling::jack_transport_rolling;
use std::mem::MaybeUninit;

unsafe extern "C" fn timebase_callback (
    state: j::jack_transport_state_t,
    nframes: j::jack_nframes_t,
    pos: *mut j::jack_position_t,
    new_pos: ::libc::c_int,
    arg: *mut ::libc::c_void,
) {
    let arg_raw_str = std::ffi::CStr::from_ptr(arg as *mut c_char);

    let mut numerator = 0.0;
    let mut denominator = 0.0;
    let mut tempo = 0.0 as f64;
    
    if let Ok(s) = arg_raw_str.to_str(){
        let mut arg_iter = s.split(" ");

	numerator = arg_iter.next().unwrap().parse().unwrap();
	denominator = arg_iter.next().unwrap().parse().unwrap();
	tempo = arg_iter.next().unwrap().parse().unwrap();
	    
    }

    if new_pos != 0 {
	(*pos).beats_per_bar = numerator;
	(*pos).beat_type = denominator;
	(*pos).beats_per_minute = tempo;
	(*pos).bar = (*pos).bar; 
	(*pos).beat = (*pos).beat; 
	(*pos).tick = (*pos).tick; 
	    
    } else {
    match state {
	j::JackTransportStopped => {
	    println!("Stopped");
	}
	j::JackTransportRolling => {
	    jack_transport_rolling(numerator, denominator, tempo, nframes, pos);
	}
	j::JackTransportStarting => {
	    println!("Starting");
	}
	_ => ()
    }
	println!("end cycle");
    }
    ()
}
pub struct Timekeeper {
    numerator: u8,
    denominator: u8,
    tempo: u8
}
impl Timekeeper {
    pub fn new(numerator: u8, denominator: u8, tempo: u8) -> Timekeeper {
        Timekeeper { numerator, denominator, tempo}
    }
    pub fn start(&self) {
        let (client, _status) =
            jack::Client::new("st-conductor", jack::ClientOptions::NO_START_SERVER).unwrap();



	let cb: j::TimebaseCallback = Some(timebase_callback);
	let serial_arg = format!("{:?} {:?} {:?}", self.numerator, self.denominator, self.tempo);
	let arg_cstring = std::ffi::CString::new(serial_arg).unwrap();
	let arg: *mut ::libc::c_void = arg_cstring.into_raw() as *mut ::libc::c_void;

	unsafe {
	    j::jack_engine_takeover_timebase(client.raw());
    	    j::jack_set_timebase_callback(
		client.raw(),
    		0,
    		cb,
    		arg
    	    );
	    j::jack_transport_start(client.raw());
	}
	let client_pointer = client.raw();
	
	let process = jack::ClosureProcessHandler::new(
            move |_: &jack::Client, ps: &jack::ProcessScope| -> jack::Control {
                // Continue as normal
                jack::Control::Continue
            },

	);

	let active_client = client.activate_async((), process).unwrap();
	loop {
	    let mut pos: MaybeUninit<j::jack_position_t> = MaybeUninit::uninit();
	    unsafe {
		let p = pos.as_mut_ptr();
		j::jack_transport_query(client_pointer, p);
	    }
	}
    }
}
