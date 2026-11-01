//! One stream's decoder, as the transform drives it: compressed access units
//! in, interleaved float blocks out, each placed on the timeline.
//!
//! The decoding is `orender_engine`'s: it loads the family's bridge DLL, which
//! decodes the stream, and renders the result (objects included) to the speaker
//! layout generated from the channel mask. Everything here is the adaptation:
//! the layout file, the check that the engine's channels are the mask's in
//! order, the timestamps, the errors.

use std::path::{Path, PathBuf};

use bridge_api::RInputTransport;
use orender_engine::{Engine, RenderedAudio};

use super::clock::{OutputClock, frames_to_hns, hns_to_us, us_to_hns};
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

/// Where a session finds and puts its files.
#[derive(Debug, Clone)]
pub struct Setup {
    /// The folder holding `harletty_dolby_bridge` / `harletty_dts_bridge`.
    pub bridge_dir: PathBuf,
    /// Where the generated speaker layout is written.
    pub work_dir: PathBuf,
    /// An engine configuration (Omniphony's `config.yaml` schema), if any.
    /// `None` keeps the engine's defaults and reads no file.
    pub engine_config: Option<PathBuf>,
}

/// The file name of a family's bridge library on this platform.
pub fn bridge_file_name(family: &str) -> String {
    if cfg!(windows) {
        format!("harletty_{family}_bridge.dll")
    } else if cfg!(target_os = "macos") {
        format!("libharletty_{family}_bridge.dylib")
    } else {
        format!("libharletty_{family}_bridge.so")
    }
}

pub struct Session {
    engine: Engine,
    codec: &'static Codec,
    rate: u32,
    mask: u32,
    channels: u32,
    clock: OutputClock,
    rate_warned: bool,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("codec", &self.codec.name)
            .field("rate", &self.rate)
            .field("mask", &self.mask)
            .finish_non_exhaustive()
    }
}

/// Write the layout for `mask` into `dir`, unless that file is already there.
fn write_layout(dir: &Path, mask: u32) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join(format!("layout-{mask:X}.yaml"));
    let yaml = layout::layout_yaml(mask);
    if std::fs::read_to_string(&path).ok().as_deref() != Some(yaml.as_str()) {
        std::fs::write(&path, yaml).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(path)
}

/// The engine configuration that binauralises, written to `dir`.
fn write_binaural_config(dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join("engine-binaural.yaml");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(layout::BINAURAL_CONFIG) {
        std::fs::write(&path, layout::BINAURAL_CONFIG)
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(path)
}

impl Session {
    /// Start a decoder for `codec` at `rate`, rendering to `mask`'s speakers.
    /// Slow (the engine builds its panning tables): done when the input type is
    /// set, not per packet.
    pub fn open(
        codec: &'static Codec,
        rate: u32,
        mask: u32,
        setup: &Setup,
    ) -> Result<Self, String> {
        let plan = layout::plan(mask);
        let bridge = setup.bridge_dir.join(bridge_file_name(codec.bridge));
        if !bridge.is_file() {
            return Err(format!("{}: not found", bridge.display()));
        }
        let layout_path = write_layout(&setup.work_dir, plan.render_mask)?;
        // A configuration of the user's own is theirs; otherwise the one that
        // binauralises, when the output is too small to pan to.
        let engine_config = match (&setup.engine_config, plan.binaural) {
            (Some(path), _) => Some(path.clone()),
            (None, true) => Some(write_binaural_config(&setup.work_dir)?),
            (None, false) => None,
        };
        let started = std::time::Instant::now();
        let engine = Engine::from_paths(
            engine_config.as_deref(),
            Some(&layout_path),
            &[bridge],
            Some(codec.input_codec),
            rate,
        )
        .map_err(|e| format!("{e:#}"))?;
        log::info!(
            "engine for {} ready in {:.0?} ({}, output mask 0x{:X})",
            codec.name,
            started.elapsed(),
            if plan.binaural {
                "binaural"
            } else {
                "speakers"
            },
            plan.output_mask
        );

        // The engine's channels must be the output's speakers, in order: the
        // output type promises it.
        let mask = plan.output_mask;
        let (want, got) = (layout::labels(mask), engine.channel_layout());
        if want != got {
            return Err(format!(
                "the engine renders {got:?}, the output type needs {want:?}"
            ));
        }
        let channels = engine.channel_count();
        Ok(Self {
            engine,
            codec,
            rate,
            mask,
            channels,
            clock: OutputClock::new(rate),
            rate_warned: false,
        })
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

    /// One compressed access unit (a Matroska block's payload). `pts_hns` is
    /// its sample time, if it had one.
    pub fn push(&mut self, data: &[u8], pts_hns: Option<i64>) -> Result<Vec<Block>, String> {
        self.engine.set_input_pts(pts_hns.map(hns_to_us));
        let chunks = self
            .engine
            .process(data, RInputTransport::Raw, 0)
            .map_err(|e| format!("{e:#}"))?;
        self.blocks(chunks)
    }

    /// End of stream: whatever the engine still holds.
    pub fn drain(&mut self) -> Result<Vec<Block>, String> {
        let chunks = self.engine.drain().map_err(|e| format!("{e:#}"))?;
        self.blocks(chunks)
    }

    /// A seek or discontinuity: forget the stream, keep the setup.
    pub fn reset(&mut self) {
        self.engine.reset();
        self.clock.reset();
    }

    fn blocks(&mut self, chunks: Vec<RenderedAudio>) -> Result<Vec<Block>, String> {
        let engine_rate = self.engine.sample_rate();
        if engine_rate != self.rate && !self.rate_warned {
            self.rate_warned = true;
            log::warn!(
                "the stream is {engine_rate} Hz, the output type says {} Hz: not handled yet",
                self.rate
            );
        }
        let latency = frames_to_hns(self.engine.output_latency_samples(), self.rate);
        let mut blocks = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            if chunk.n_channels != self.channels {
                return Err(format!(
                    "the engine returned {} channels, expected {}",
                    chunk.n_channels, self.channels
                ));
            }
            let pts = chunk.input_pts_us.map(us_to_hns);
            let (start_hns, duration_hns) = self.clock.place(pts, latency, chunk.n_frames as u64);
            blocks.push(Block {
                start_hns,
                duration_hns,
                frames: chunk.n_frames,
                samples: chunk.samples,
            });
        }
        Ok(blocks)
    }
}
