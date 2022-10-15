use jack::jack_sys as j;

fn get_incremented_bar_beat(bar: i32, beat: i32, beats_per_bar: f32) -> (i32, i32) {
    let beat_f32 = beat as f32;
    if beat_f32 < beats_per_bar {
	return (bar, beat + 1);
    }
    (bar + 1, 0)
}
pub unsafe fn jack_transport_rolling(
    numerator: f32,
    denominator: f32,
    tempo: f64,
    nframes: j::jack_nframes_t,
    pos: *mut j::jack_position_t,
    dangerous_pointer: *mut u32
	
) {
    let beats_per_bar = (*pos).beats_per_bar; 
    let bar = (*pos).bar;
    let beat = (*pos).beat; 
    let tick = (*pos).tick;

    let frames_per_minute = (*pos).frame_rate * 60; 
    let frames_per_beat =  (frames_per_minute as f64 / tempo) * 2f64;
    
    let absolute_beat: u64 = (beats_per_bar as u64 * (bar as u64 - 1)) + beat as u64;

    let this_beat_frame: u64 = absolute_beat as u64 * frames_per_beat as u64;
    let next_beat_frame: u64 = this_beat_frame + frames_per_beat as u64;

    *dangerous_pointer = next_beat_frame as u32;
    


    let start_frame =  (*pos).frame;
    let end_frame: u64 =  ((*pos).frame + nframes).into();

    let periods_per_beat = frames_per_beat as f64 / nframes as f64;
    let ticks_per_period = 1920.0 / periods_per_beat;
    if next_beat_frame > end_frame {
	(*pos).bar = bar; 
	(*pos).beat = beat;
	(*pos).tick= ((*pos).tick) + (ticks_per_period as i32)*2;
    } else if next_beat_frame <= end_frame {
        println!("next: {:?} end: {:?}", next_beat_frame, end_frame);
	((*pos).bar, (*pos).beat) = get_incremented_bar_beat(bar, beat, beats_per_bar);
	(*pos).tick = 0;
	println!("{:?}", periods_per_beat);
	println!("{:?}", ticks_per_period);
    }

    (*pos).valid = j::JackPositionBBT | j::JackTransportPosition;
    (*pos).beats_per_bar = numerator;
    (*pos).beat_type = denominator;
    (*pos).beats_per_minute = tempo;
    (*pos).ticks_per_beat = 1920.0;
    (*pos).beats_per_minute = tempo;
    (*pos).frame = end_frame as u32;
}
