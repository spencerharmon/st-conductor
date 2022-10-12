use jack::jack_sys as j;

unsafe extern "C" fn timebase_callback (
    state: j::jack_transport_state_t,
    nframes: j::jack_nframes_t,
    pos: *mut j::jack_position_t,
    new_pos: ::libc::c_int,
    arg: *mut ::libc::c_void,
) {
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
    pub async fn run(&self) {
        let (client, _status) =
            jack::Client::new("st-conductor", jack::ClientOptions::NO_START_SERVER).unwrap();


	let cb: j::TimebaseCallback = Some(timebase_callback);
	let serial_arg = format!("{:?} {:?} {:?}", self.numerator, self.denominator, self.tempo);
	let arg: *mut ::libc::c_void = serial_arg.as_ptr() as *mut ::libc::c_void;
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
