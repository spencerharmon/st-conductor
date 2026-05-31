use jack::jack_sys as j;
use st_lib::beat_math;
use std::sync::atomic::{AtomicU64, Ordering};

fn get_incremented_bar_beat(bar: i32, beat: i32, beats_per_bar: f32) -> (i32, i32) {
    let beat_f32 = beat as f32;
    if beat_f32 < beats_per_bar {
        return (bar, beat + 1);
    }
    (bar + 1, 1)
}

/// Pure result of advancing the transport one process cycle.
///
/// This is the data that `jack_transport_rolling` would write into the
/// `jack_position_t` (and into the shared "next beat frame" slot), separated
/// out so it can be unit-tested without touching JACK pointers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RollingUpdate {
    /// Sample frame of the *next* beat boundary.
    pub next_beat_frame: u64,
    /// Next-cycle bar (1-indexed).
    pub bar: i32,
    /// Next-cycle beat within the bar (1-indexed).
    pub beat: i32,
    /// Next-cycle tick within the beat.
    pub tick: i32,
    /// Frame at the end of this process cycle (becomes `pos.frame`).
    pub end_frame: u64,
    /// `true` if a beat boundary falls within this process cycle.
    pub crossed_beat: bool,
}

/// Pure computation: variant of the rolling update that takes the
/// current sample frame explicitly. This is the testable kernel of
/// [`jack_transport_rolling`].
pub fn compute_rolling_update_at(
    nframes: u32,
    frame_rate: u32,
    current_frame: u32,
    bar: i32,
    beat: i32,
    tick: i32,
    beats_per_bar: f32,
    tempo: f64,
) -> RollingUpdate {
    let frames_per_beat = beat_math::frames_per_beat(frame_rate, tempo);
    let abs_beat = beat_math::absolute_beat(bar, beat, beats_per_bar);
    let next_beat_frame = beat_math::next_beat_frame(abs_beat, frames_per_beat);

    let end_frame: u64 = (current_frame + nframes) as u64;

    let periods_per_beat = frames_per_beat / nframes as f64;
    let ticks_per_period = 1920.0 / periods_per_beat;

    let (out_bar, out_beat, out_tick, crossed_beat) = if next_beat_frame > end_frame {
        // Still inside the current beat — accumulate ticks.
        (bar, beat, tick + (ticks_per_period as i32) * 2, false)
    } else {
        // Cross a beat boundary.
        let (b2, beat2) = get_incremented_bar_beat(bar, beat, beats_per_bar);
        (b2, beat2, 0, true)
    };

    RollingUpdate {
        next_beat_frame,
        bar: out_bar,
        beat: out_beat,
        tick: out_tick,
        end_frame,
        crossed_beat,
    }
}

/// JACK transport-rolling callback. Thin wrapper that calls
/// [`compute_rolling_update_at`] and writes the result back through the
/// supplied raw pointers. All the math is in the pure helper; everything
/// here is pointer plumbing.
///
/// `next_beat_frame_slot` is the cross-thread channel by which the freshly
/// computed next-beat frame reaches the async poll loop. Relaxed ordering
/// matches the existing "latest wins" semantics of `st_sync` (the single
/// producer is this callback; the single consumer is the poll loop).
pub unsafe fn jack_transport_rolling(
    numerator: f32,
    denominator: f32,
    tempo: f64,
    nframes: j::jack_nframes_t,
    pos: *mut j::jack_position_t,
    next_beat_frame_slot: &AtomicU64,
) {
    let update = compute_rolling_update_at(
        nframes,
        (*pos).frame_rate,
        (*pos).frame,
        (*pos).bar,
        (*pos).beat,
        (*pos).tick,
        (*pos).beats_per_bar,
        tempo,
    );

    next_beat_frame_slot.store(update.next_beat_frame, Ordering::Relaxed);

    if update.crossed_beat {
        println!("next: {:?} end: {:?}", update.next_beat_frame, update.end_frame);
    }

    (*pos).bar = update.bar;
    (*pos).beat = update.beat;
    (*pos).tick = update.tick;

    (*pos).valid = j::JackPositionBBT | j::JackTransportPosition;
    (*pos).beats_per_bar = numerator;
    (*pos).beat_type = denominator;
    (*pos).beats_per_minute = tempo;
    (*pos).ticks_per_beat = 1920.0;
    (*pos).frame = update.end_frame as u32;
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- get_incremented_bar_beat ---

    #[test]
    fn increment_within_bar() {
        assert_eq!(get_incremented_bar_beat(1, 1, 4.0), (1, 2));
        assert_eq!(get_incremented_bar_beat(1, 3, 4.0), (1, 4));
    }

    #[test]
    fn increment_wraps_to_next_bar() {
        assert_eq!(get_incremented_bar_beat(1, 4, 4.0), (2, 1));
        assert_eq!(get_incremented_bar_beat(5, 7, 7.0), (6, 1));
    }

    #[test]
    fn increment_handles_odd_meter() {
        assert_eq!(get_incremented_bar_beat(2, 6, 7.0), (2, 7));
        assert_eq!(get_incremented_bar_beat(2, 7, 7.0), (3, 1));
    }

    // --- compute_rolling_update_at ---

    /// At 48 kHz, 120 BPM, 4/4: frames_per_beat = 48000 (with the *2
    /// half-beat convention). Starting at bar 1 beat 1 (absolute beat 1),
    /// next_beat_frame = 2 * 48000 = 96000.
    #[test]
    fn rolling_update_does_not_cross_beat_within_short_cycle() {
        let u = compute_rolling_update_at(
            /* nframes      */ 1024,
            /* frame_rate   */ 48000,
            /* current_frame*/ 0,
            /* bar          */ 1,
            /* beat         */ 1,
            /* tick         */ 0,
            /* beats_per_bar*/ 4.0,
            /* tempo        */ 120.0,
        );
        assert_eq!(u.next_beat_frame, 96_000);
        assert_eq!(u.end_frame, 1024);
        assert!(!u.crossed_beat, "1024-frame cycle should not cross beat at 96000");
        assert_eq!(u.bar, 1);
        assert_eq!(u.beat, 1);
        assert!(u.tick > 0, "tick should advance within the beat");
    }

    /// Same setup, but a cycle large enough to swallow the next beat frame.
    #[test]
    fn rolling_update_crosses_beat_when_cycle_spans_boundary() {
        let u = compute_rolling_update_at(
            100_000,   // nframes covers well past 96000
            48000,
            0,
            1,
            1,
            0,
            4.0,
            120.0,
        );
        assert_eq!(u.next_beat_frame, 96_000);
        assert_eq!(u.end_frame, 100_000);
        assert!(u.crossed_beat);
        assert_eq!(u.bar, 1);
        assert_eq!(u.beat, 2);
        assert_eq!(u.tick, 0);
    }

    /// Crossing the bar boundary (4/4, beat 4 -> beat 1 of next bar).
    #[test]
    fn rolling_update_crosses_bar_boundary() {
        // At bar 1 beat 4 (abs_beat 4), next_beat_frame = 5 * 48000 = 240_000.
        let u = compute_rolling_update_at(
            300_000,
            48000,
            0,
            1,
            4,
            0,
            4.0,
            120.0,
        );
        assert_eq!(u.next_beat_frame, 240_000);
        assert!(u.crossed_beat);
        assert_eq!(u.bar, 2);
        assert_eq!(u.beat, 1);
        assert_eq!(u.tick, 0);
    }

    /// Simulate a long run of process cycles and assert that every
    /// distinct `next_beat_frame` value the timebase callback would push
    /// into the AtomicU64 is *strictly greater* than the previous one.
    /// This is the property that guarantees the st-sync `BeatPublisher`
    /// will never panic on real data from this conductor.
    #[test]
    fn next_beat_frame_sequence_is_strictly_monotonic() {
        const FRAME_RATE: u32 = 48_000;
        const TEMPO: f64 = 120.0;
        const BEATS_PER_BAR: f32 = 4.0;
        const NFRAMES: u32 = 1024;
        const TOTAL_CYCLES: u32 = 4000; // ~85 seconds at 48kHz/1024.

        let mut bar = 1i32;
        let mut beat = 1i32;
        let mut tick = 0i32;
        let mut current_frame: u32 = 0;
        let mut last_unique: u64 = 0;
        let mut distinct_beats = 0u64;

        for _ in 0..TOTAL_CYCLES {
            let u = compute_rolling_update_at(
                NFRAMES,
                FRAME_RATE,
                current_frame,
                bar,
                beat,
                tick,
                BEATS_PER_BAR,
                TEMPO,
            );
            if u.next_beat_frame != last_unique {
                assert!(
                    u.next_beat_frame > last_unique,
                    "next_beat_frame regressed: was {}, now {}",
                    last_unique,
                    u.next_beat_frame
                );
                last_unique = u.next_beat_frame;
                distinct_beats += 1;
            }
            bar = u.bar;
            beat = u.beat;
            tick = u.tick;
            current_frame = u.end_frame as u32;
        }

        // Sanity: with the *2 frames_per_beat convention (see
        // st_lib::beat_math), at 120 BPM/48kHz a beat is 48000 frames,
        // so 4000 * 1024-frame cycles ≈ 85 beats. Just check the
        // simulation was meaningful.
        assert!(
            distinct_beats > 50,
            "expected many distinct beats in the simulation, got {}",
            distinct_beats
        );
    }

    /// Same property at a fast tempo and small buffer: stress the boundary
    /// crossings, ensure monotonicity still holds when cycles often
    /// straddle multiple beats.
    #[test]
    fn next_beat_frame_monotonic_at_fast_tempo() {
        const FRAME_RATE: u32 = 48_000;
        const TEMPO: f64 = 240.0; // 4 Hz, 12000 fpb
        const BEATS_PER_BAR: f32 = 7.0; // odd meter
        const NFRAMES: u32 = 256;
        const TOTAL_CYCLES: u32 = 8000;

        let mut bar = 1i32;
        let mut beat = 1i32;
        let mut tick = 0i32;
        let mut current_frame: u32 = 0;
        let mut last_unique: u64 = 0;

        for _ in 0..TOTAL_CYCLES {
            let u = compute_rolling_update_at(
                NFRAMES,
                FRAME_RATE,
                current_frame,
                bar,
                beat,
                tick,
                BEATS_PER_BAR,
                TEMPO,
            );
            if u.next_beat_frame != last_unique {
                assert!(
                    u.next_beat_frame > last_unique,
                    "regressed at tempo={} meter={}: {} -> {}",
                    TEMPO,
                    BEATS_PER_BAR,
                    last_unique,
                    u.next_beat_frame
                );
                last_unique = u.next_beat_frame;
            }
            bar = u.bar;
            beat = u.beat;
            tick = u.tick;
            current_frame = u.end_frame as u32;
        }
    }
}
