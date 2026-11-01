//! One stream's decoder, as the transform drives it. Phase 1: the session
//! decodes nothing and answers with silence, so the COM plumbing can be proven
//! in Windows Media Player before any decoder is wired in. Phase 2 puts
//! `orender_engine` behind the same interface.
//!
//! The silence covers the time between consecutive input timestamps rather than
//! a duration taken from each sample: Matroska blocks of TrueHD carry no
//! duration, and guessing one (the Phase 0 stub gave every block 100 ms) puts
//! the timeline far ahead of the audio.

use super::clock::{OutputClock, hns_to_frames};
use super::codecs::Codec;
use super::layout;

/// One block of interleaved float PCM and where it sits on the timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub start_hns: i64,
    pub duration_hns: i64,
    pub frames: usize,
    pub samples: Vec<f32>,
}

#[derive(Debug)]
pub struct Session {
    codec: &'static Codec,
    rate: u32,
    mask: u32,
    channels: u32,
    clock: OutputClock,
    /// The first timestamp of the current timeline, and the frames produced
    /// since: output is counted from that origin, not summed from rounded
    /// gaps, so nothing drifts.
    origin_hns: Option<i64>,
    emitted_frames: u64,
}

/// More than this much between two timestamps is a gap in the stream, not
/// audio to fill (and would be a huge buffer).
const MAX_GAP_SECONDS: u32 = 1;

impl Session {
    pub fn new(codec: &'static Codec, rate: u32, mask: u32) -> Self {
        let mask = layout::effective_mask(mask);
        Self {
            codec,
            rate,
            mask,
            channels: mask.count_ones(),
            clock: OutputClock::new(rate),
            origin_hns: None,
            emitted_frames: 0,
        }
    }

    pub fn codec(&self) -> &'static Codec {
        self.codec
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// The channel mask output is laid out in.
    pub fn mask(&self) -> u32 {
        self.mask
    }

    pub fn channels(&self) -> u32 {
        self.channels
    }

    /// One compressed access unit. `pts_hns` is its sample time, if it had one.
    pub fn push(&mut self, _data: &[u8], pts_hns: Option<i64>) -> Vec<Block> {
        let Some(pts) = pts_hns else {
            return Vec::new();
        };
        let Some(origin) = self.origin_hns else {
            self.start_timeline(pts);
            return Vec::new(); // the first timestamp only starts the timeline
        };
        let max_gap = (self.rate * MAX_GAP_SECONDS) as i64;
        let frames = hns_to_frames(pts - origin, self.rate) - self.emitted_frames as i64;
        if frames > max_gap || frames < -max_gap {
            self.start_timeline(pts); // a gap or a jump: start over from here
            return Vec::new();
        }
        if frames <= 0 {
            return Vec::new();
        }
        let frames = frames as usize;
        // The clock was anchored at the origin: blocks tile on from there.
        let (start_hns, duration_hns) = self.clock.place(None, 0, frames as u64);
        self.emitted_frames += frames as u64;
        vec![Block {
            start_hns,
            duration_hns,
            frames,
            samples: vec![0.0; frames * self.channels as usize],
        }]
    }

    fn start_timeline(&mut self, pts: i64) {
        self.origin_hns = Some(pts);
        self.emitted_frames = 0;
        self.clock.anchor(pts, 0);
    }

    /// End of stream: what is still held. Nothing is, in Phase 1.
    pub fn drain(&mut self) -> Vec<Block> {
        Vec::new()
    }

    /// A seek or discontinuity.
    pub fn reset(&mut self) {
        self.origin_hns = None;
        self.emitted_frames = 0;
        self.clock.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portable::clock::hns_to_frames;
    use crate::portable::codecs::CODECS;

    fn session() -> Session {
        Session::new(&CODECS[0], 48_000, 0x63F)
    }

    #[test]
    fn output_is_laid_out_for_the_mask() {
        let s = session();
        assert_eq!((s.channels(), s.mask(), s.rate()), (8, 0x63F, 48_000));
        assert_eq!(Session::new(&CODECS[0], 48_000, 0).channels(), 2);
    }

    #[test]
    fn silence_fills_the_time_between_timestamps() {
        let mut s = session();
        assert!(s.push(b"x", Some(1_000_000)).is_empty()); // starts the timeline
        let blocks = s.push(b"x", Some(1_100_000)); // +10 ms
        assert_eq!(blocks.len(), 1);
        let block = &blocks[0];
        assert_eq!(block.start_hns, 1_000_000);
        assert_eq!(block.frames, 480);
        assert_eq!(block.duration_hns, 100_000);
        assert_eq!(block.samples.len(), 480 * 8);
        assert!(block.samples.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn a_missing_duration_does_not_run_the_timeline_ahead() {
        // Blocks 0.83 ms apart (40 frames, not a whole number of 100 ns units):
        // 1000 of them cover 0.83 s, not 100 s, and not a frame short.
        let mut s = session();
        let mut frames = 0;
        for i in 0..=1000 {
            for block in s.push(b"x", Some(i * 8_333)) {
                frames += block.frames as i64;
            }
        }
        assert_eq!(frames, hns_to_frames(1000 * 8_333, 48_000));
    }

    #[test]
    fn consecutive_blocks_end_where_the_next_starts() {
        // 40 frames every 8_333 units (not a whole number of units each).
        let mut s = session();
        let mut end = None;
        for i in 0..=2000 {
            for block in s.push(b"x", Some(i * 8_333)) {
                if let Some(end) = end {
                    assert_eq!(block.start_hns, end, "block at input {i}");
                }
                end = Some(block.start_hns + block.duration_hns);
            }
        }
        assert!(end.is_some());
    }

    #[test]
    fn a_gap_or_a_jump_back_produces_nothing() {
        let mut s = session();
        s.push(b"x", Some(0));
        assert!(s.push(b"x", Some(50_000_000)).is_empty()); // 5 s: a gap
        assert!(s.push(b"x", Some(40_000_000)).is_empty()); // backwards
        assert!(s.push(b"x", None).is_empty());
    }

    #[test]
    fn reset_starts_a_new_timeline() {
        let mut s = session();
        s.push(b"x", Some(0));
        s.reset();
        assert!(s.push(b"x", Some(90_000_000)).is_empty());
        assert_eq!(s.push(b"x", Some(90_100_000))[0].start_hns, 90_000_000);
        assert!(s.drain().is_empty());
    }
}
