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
    pos: *mut j::jack_position_t
) {
    let beats_per_bar = (*pos).beats_per_bar; 
    let bar = (*pos).bar; 
    let beat = (*pos).beat; 
    let tick = (*pos).tick; 

    let frames_per_minute = (*pos).frame_rate * 60; 
    let frames_per_beat =  frames_per_minute as f64 / tempo;
    
    let absolute_beat = (beats_per_bar * bar as f32) + beat as f32;

    let this_beat_frame = absolute_beat as f64 * frames_per_beat;
    let next_beat_frame = this_beat_frame as f64 + frames_per_beat;


    let start_frame =  (*pos).frame;
    let end_frame =  (*pos).frame + nframes;

    println!("next: {:?} end: {:?}", next_beat_frame, end_frame);
    if next_beat_frame > end_frame.into() {
	(*pos).bar = bar; 
	(*pos).beat = beat;
    } else if next_beat_frame <= end_frame.into() {
	((*pos).beat, (*pos).bar) = get_incremented_bar_beat(bar, beat, beats_per_bar);
	(*pos).tick = 0;
    }

    (*pos).valid = j::JackPositionBBT | j::JackTransportBBT | j::JackTransportPosition | j::JackTransportState;
    (*pos).beats_per_bar = numerator;
    (*pos).beat_type = denominator;
    (*pos).beats_per_minute = tempo;
    (*pos).tick = (*pos).tick+1;
    (*pos).ticks_per_beat = 1920.0;
    (*pos).beats_per_minute = tempo;
    (*pos).frame = end_frame;
}
