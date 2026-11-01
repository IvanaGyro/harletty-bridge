//! Sample time: Media Foundation counts in 100 ns units, the engine in
//! microseconds and frames.

pub const HNS_PER_SECOND: i64 = 10_000_000;

/// Frames at `rate` as 100 ns units, rounded down.
pub fn frames_to_hns(frames: u64, rate: u32) -> i64 {
    (frames as i128 * HNS_PER_SECOND as i128 / rate as i128) as i64
}

/// 100 ns units as frames at `rate`, rounded down (negative stays negative).
pub fn hns_to_frames(hns: i64, rate: u32) -> i64 {
    (hns as i128 * rate as i128 / HNS_PER_SECOND as i128) as i64
}

pub fn hns_to_us(hns: i64) -> i64 {
    hns / 10
}

pub fn us_to_hns(us: i64) -> i64 {
    us * 10
}

/// Where each output block sits on the timeline.
///
/// A block that carries an input timestamp re-anchors the clock: it starts at
/// that timestamp, less the renderer's latency. A block without one follows on
/// from the last. Starts are computed from the anchor and the frames since, so
/// consecutive blocks tile exactly, with no rounding drift.
#[derive(Debug, Clone)]
pub struct OutputClock {
    rate: u32,
    anchor_hns: i64,
    frames_since_anchor: u64,
}

impl OutputClock {
    pub fn new(rate: u32) -> Self {
        Self {
            rate,
            anchor_hns: 0,
            frames_since_anchor: 0,
        }
    }

    /// Forget the position: after a seek the first block re-anchors.
    pub fn reset(&mut self) {
        self.anchor_hns = 0;
        self.frames_since_anchor = 0;
    }

    /// Start a timeline at `pts_hns` (less `latency_hns`): later blocks placed
    /// with no timestamp tile on from here.
    pub fn anchor(&mut self, pts_hns: i64, latency_hns: i64) {
        self.anchor_hns = pts_hns - latency_hns;
        self.frames_since_anchor = 0;
    }

    /// The `(start, duration)` in 100 ns units of the next block of `frames`.
    /// Duration is the difference of two cumulative positions, so a block ends
    /// exactly where the next starts.
    pub fn place(
        &mut self,
        input_pts_hns: Option<i64>,
        latency_hns: i64,
        frames: u64,
    ) -> (i64, i64) {
        if let Some(pts) = input_pts_hns {
            self.anchor(pts, latency_hns);
        }
        let before = frames_to_hns(self.frames_since_anchor, self.rate);
        self.frames_since_anchor += frames;
        let after = frames_to_hns(self.frames_since_anchor, self.rate);
        (self.anchor_hns + before, after - before)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions() {
        assert_eq!(frames_to_hns(48_000, 48_000), HNS_PER_SECOND);
        assert_eq!(hns_to_frames(HNS_PER_SECOND / 2, 48_000), 24_000);
        assert_eq!(hns_to_frames(-HNS_PER_SECOND, 48_000), -48_000);
        assert_eq!(hns_to_us(us_to_hns(1234)), 1234);
    }

    #[test]
    fn blocks_without_a_timestamp_tile_exactly() {
        // 1001 frames at 44.1 kHz is not a whole number of 100 ns units: the
        // blocks must still tile, each starting where the last ended.
        let mut clock = OutputClock::new(44_100);
        let (mut end, mut duration) = clock.place(Some(5_000_000), 0, 1001);
        end += duration;
        let mut total = duration;
        for _ in 0..1000 {
            let (start, d) = clock.place(None, 0, 1001);
            assert_eq!(start, end);
            end = start + d;
            duration = d;
            total += duration;
        }
        assert_eq!(total, frames_to_hns(1001 * 1001, 44_100));
    }

    #[test]
    fn a_timestamp_re_anchors_and_subtracts_the_latency() {
        let mut clock = OutputClock::new(48_000);
        assert_eq!(clock.place(None, 0, 480), (0, 100_000));
        assert_eq!(clock.place(None, 0, 480), (100_000, 100_000));
        assert_eq!(
            clock.place(Some(10_000_000), 50_000, 480),
            (9_950_000, 100_000)
        );
        assert_eq!(clock.place(None, 0, 480), (10_050_000, 100_000));
    }

    #[test]
    fn reset_starts_over() {
        let mut clock = OutputClock::new(48_000);
        clock.place(Some(10_000_000), 0, 480);
        clock.reset();
        assert_eq!(clock.place(None, 0, 480), (0, 100_000));
    }
}
