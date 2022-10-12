use jack::jack_sys as j;
use ::libc::*;

unsafe extern "C" fn timebase_callback (
    state: j::jack_transport_state_t,
    nframes: j::jack_nframes_t,
    pos: *mut j::jack_position_t,
    new_pos: ::libc::c_int,
    arg: *mut ::libc::c_void,
) {
    let arg_raw_str = std::ffi::CStr::from_ptr(arg as *mut c_char);

    let mut numerator = 0;
    let mut denominator = 0;
    let mut tempo = 0.0;
    
    if let Ok(s) = arg_raw_str.to_str(){
        let mut arg_iter = s.split(" ");

	numerator = arg_iter.next().unwrap().parse().unwrap();
	denominator = arg_iter.next().unwrap().parse().unwrap();
	tempo = arg_iter.next().unwrap().parse().unwrap();
	    
    }

    println!("{}", numerator);
    println!("{}", denominator);
    println!("{}", tempo);

	
    if new_pos != 0 {
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
//	let arg: *mut ::libc::c_void = serial_arg.as_ptr() as *mut ::libc::c_void;
	unsafe {
	    j::jack_engine_takeover_timebase(client.raw());
    	    j::jack_set_timebase_callback(
		client.raw(),
    		0,
    		cb,
    		arg
    	    );
	}
	let process = jack::ClosureProcessHandler::new(
            move |_: &jack::Client, ps: &jack::ProcessScope| -> jack::Control {
                
                // Continue as normal
                jack::Control::Continue
            },
        );
        let active_client = client.activate_async((), process).unwrap();
	
    }
}
