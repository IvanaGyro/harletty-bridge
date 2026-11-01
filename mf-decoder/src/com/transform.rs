//! The `IMFTransform`: a synchronous, one-input one-output audio decoder.
//! The contract is in docs/plan-media-foundation-decoder.md ("The MFT
//! contract"); the decoding is a [`Session`].
//!
//! Every method that does work runs inside [`guard`]: a panic must not unwind
//! into the player. A caught panic drops the queue and the session's position
//! and comes back as `E_UNEXPECTED`.

use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, MutexGuard};

use windows::Win32::Foundation::{E_INVALIDARG, E_NOTIMPL, E_UNEXPECTED};
use windows::Win32::Media::MediaFoundation::*;
use windows::core::{GUID, Ref, implement};

use crate::portable::clock::HNS_PER_SECOND;
use crate::portable::codecs::{CODECS, Codec, find};
use crate::portable::config::MfConfig;
use crate::portable::layout;
use crate::portable::session::{Block, Session};

const DEFAULT_RATE: u32 = 48_000;

struct State {
    config: MfConfig,
    input_type: Option<IMFMediaType>,
    output_type: Option<IMFMediaType>,
    session: Option<Session>,
    queue: VecDeque<Block>,
}

#[implement(IMFTransform)]
pub struct Decoder {
    state: Mutex<State>,
}

pub fn new() -> Decoder {
    let config = super::paths::load_config();
    super::logger::init(&config);
    Decoder {
        state: Mutex::new(State {
            config,
            input_type: None,
            output_type: None,
            session: None,
            queue: VecDeque::new(),
        }),
    }
}

impl Decoder_Impl {
    fn state(&self) -> MutexGuard<'_, State> {
        // A poisoned lock means a method panicked: the state is reset below,
        // so carry on with what is there.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Run `body`; a panic becomes `E_UNEXPECTED` after the state is reset.
fn guard<T>(
    decoder: &Decoder_Impl,
    body: impl FnOnce() -> windows::core::Result<T>,
) -> windows::core::Result<T> {
    match catch_unwind(AssertUnwindSafe(body)) {
        Ok(result) => result,
        Err(_) => {
            log::error!("panic in the transform: session reset");
            let mut state = decoder.state();
            state.queue.clear();
            if let Some(session) = state.session.as_mut() {
                session.reset();
            }
            Err(E_UNEXPECTED.into())
        }
    }
}

fn codec_of(ty: &IMFMediaType, config: &MfConfig) -> Option<&'static Codec> {
    let subtype = unsafe { ty.GetGUID(&MF_MT_SUBTYPE) }.ok()?;
    find(subtype.to_u128()).filter(|codec| config.enabled(codec))
}

/// An independent copy of `ty`: a COM `clone` only takes another reference, so
/// a host that edits a type it was handed (or one it passed in) would edit ours.
fn copy_type(ty: &IMFMediaType) -> windows::core::Result<IMFMediaType> {
    unsafe {
        let copy = MFCreateMediaType()?;
        ty.CopyAllItems(&copy)?;
        Ok(copy)
    }
}

/// The float output type for `session`.
fn output_type(session: &Session) -> windows::core::Result<IMFMediaType> {
    let (rate, channels) = (session.rate(), session.channels());
    let align = channels * 4;
    unsafe {
        let ty = MFCreateMediaType()?;
        ty.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
        ty.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float)?;
        ty.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, rate)?;
        ty.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, channels)?;
        ty.SetUINT32(&MF_MT_AUDIO_CHANNEL_MASK, session.mask())?;
        ty.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 32)?;
        ty.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, align)?;
        ty.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, align * rate)?;
        ty.SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1)?;
        Ok(ty)
    }
}

fn block_sample(block: &Block) -> windows::core::Result<IMFSample> {
    let bytes = block.samples.len() * 4;
    unsafe {
        let buffer = MFCreateMemoryBuffer(bytes as u32)?;
        let mut data = std::ptr::null_mut();
        buffer.Lock(&mut data, None, None)?;
        std::ptr::copy_nonoverlapping(block.samples.as_ptr() as *const u8, data, bytes);
        buffer.Unlock()?;
        buffer.SetCurrentLength(bytes as u32)?;
        let sample = MFCreateSample()?;
        sample.AddBuffer(&buffer)?;
        sample.SetSampleTime(block.start_hns)?;
        sample.SetSampleDuration(block.duration_hns)?;
        Ok(sample)
    }
}

impl IMFTransform_Impl for Decoder_Impl {
    fn GetStreamLimits(
        &self,
        input_min: *mut u32,
        input_max: *mut u32,
        output_min: *mut u32,
        output_max: *mut u32,
    ) -> windows::core::Result<()> {
        unsafe {
            *input_min = 1;
            *input_max = 1;
            *output_min = 1;
            *output_max = 1;
        }
        Ok(())
    }

    fn GetStreamCount(&self, inputs: *mut u32, outputs: *mut u32) -> windows::core::Result<()> {
        unsafe {
            *inputs = 1;
            *outputs = 1;
        }
        Ok(())
    }

    fn GetStreamIDs(&self, _: u32, _: *mut u32, _: u32, _: *mut u32) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn GetInputStreamInfo(
        &self,
        id: u32,
        info: *mut MFT_INPUT_STREAM_INFO,
    ) -> windows::core::Result<()> {
        if id != 0 || info.is_null() {
            return Err(E_INVALIDARG.into());
        }
        unsafe { *info = MFT_INPUT_STREAM_INFO::default() };
        Ok(())
    }

    fn GetOutputStreamInfo(&self, id: u32) -> windows::core::Result<MFT_OUTPUT_STREAM_INFO> {
        if id != 0 {
            return Err(E_INVALIDARG.into());
        }
        // We allocate: the size of a block varies with the packet.
        Ok(MFT_OUTPUT_STREAM_INFO {
            dwFlags: (MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 | MFT_OUTPUT_STREAM_WHOLE_SAMPLES.0)
                as u32,
            cbSize: 0,
            cbAlignment: 0,
        })
    }

    fn GetAttributes(&self) -> windows::core::Result<IMFAttributes> {
        Err(E_NOTIMPL.into())
    }

    fn GetInputStreamAttributes(&self, _: u32) -> windows::core::Result<IMFAttributes> {
        Err(E_NOTIMPL.into())
    }

    fn GetOutputStreamAttributes(&self, _: u32) -> windows::core::Result<IMFAttributes> {
        Err(E_NOTIMPL.into())
    }

    fn DeleteInputStream(&self, _: u32) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn AddInputStreams(&self, _: u32, _: *const u32) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn GetInputAvailableType(&self, id: u32, index: u32) -> windows::core::Result<IMFMediaType> {
        if id != 0 {
            return Err(E_INVALIDARG.into());
        }
        let state = self.state();
        let codec = CODECS
            .iter()
            .filter(|codec| state.config.enabled(codec))
            .nth(index as usize)
            .ok_or_else(|| windows::core::Error::from(MF_E_NO_MORE_TYPES))?;
        unsafe {
            let ty = MFCreateMediaType()?;
            ty.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            ty.SetGUID(&MF_MT_SUBTYPE, &GUID::from_u128(codec.subtype))?;
            Ok(ty)
        }
    }

    fn GetOutputAvailableType(&self, id: u32, index: u32) -> windows::core::Result<IMFMediaType> {
        if id != 0 {
            return Err(E_INVALIDARG.into());
        }
        let state = self.state();
        let session = state
            .session
            .as_ref()
            .ok_or_else(|| windows::core::Error::from(MF_E_TRANSFORM_TYPE_NOT_SET))?;
        if index > 0 {
            return Err(MF_E_NO_MORE_TYPES.into());
        }
        output_type(session)
    }

    fn SetInputType(
        &self,
        id: u32,
        ty: Ref<IMFMediaType>,
        flags: u32,
    ) -> windows::core::Result<()> {
        if id != 0 {
            return Err(E_INVALIDARG.into());
        }
        guard(self, || {
            let test_only = flags & MFT_SET_TYPE_TEST_ONLY.0 as u32 != 0;
            let mut state = self.state();
            let Some(ty) = ty.as_ref() else {
                state.input_type = None;
                state.output_type = None;
                state.session = None;
                state.queue.clear();
                return Ok(());
            };
            if unsafe { ty.GetGUID(&MF_MT_MAJOR_TYPE)? } != MFMediaType_Audio {
                return Err(MF_E_INVALIDMEDIATYPE.into());
            }
            let Some(codec) = codec_of(ty, &state.config) else {
                return Err(MF_E_INVALIDMEDIATYPE.into());
            };
            let rate = unsafe { ty.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND) }
                .ok()
                .filter(|&r| r > 0)
                .unwrap_or(DEFAULT_RATE);
            log::info!(
                "input type {} at {rate} Hz (test_only={test_only})",
                codec.name
            );
            if test_only {
                return Ok(());
            }
            let mask = match state.config.speaker_mask() {
                Ok(Some(mask)) => mask,
                Ok(None) => super::endpoint::speaker_mask().unwrap_or(layout::STEREO),
                Err(e) => {
                    log::warn!("mf.yaml: {e}; using the endpoint's mask");
                    super::endpoint::speaker_mask().unwrap_or(layout::STEREO)
                }
            };
            let setup = super::paths::session_setup(&state.config).map_err(|e| {
                log::error!("{e}");
                windows::core::Error::from(MF_E_INVALIDMEDIATYPE)
            })?;
            let session = Session::open(codec, rate, mask, &setup).map_err(|e| {
                // Not an error to the player: it tries the next decoder.
                log::error!("cannot start {}: {e}", codec.name);
                windows::core::Error::from(MF_E_INVALIDMEDIATYPE)
            })?;
            log::info!(
                "output: {} channels, mask 0x{:X}",
                session.channels(),
                session.mask()
            );
            state.input_type = Some(copy_type(ty)?);
            state.output_type = None;
            state.session = Some(session);
            state.queue.clear();
            Ok(())
        })
    }

    fn SetOutputType(
        &self,
        id: u32,
        ty: Ref<IMFMediaType>,
        flags: u32,
    ) -> windows::core::Result<()> {
        if id != 0 {
            return Err(E_INVALIDARG.into());
        }
        guard(self, || {
            let test_only = flags & MFT_SET_TYPE_TEST_ONLY.0 as u32 != 0;
            let mut state = self.state();
            let Some(session) = state.session.as_ref() else {
                return Err(MF_E_TRANSFORM_TYPE_NOT_SET.into());
            };
            let Some(ty) = ty.as_ref() else {
                state.output_type = None;
                return Ok(());
            };
            let (subtype, rate, channels) = unsafe {
                (
                    ty.GetGUID(&MF_MT_SUBTYPE)?,
                    ty.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)?,
                    ty.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS)?,
                )
            };
            // The whole format, not just its headline: what we write is 32-bit
            // float in the session's layout, so a type that says otherwise
            // (or leaves these out) does not describe our buffers.
            let wanted = output_type(session)?;
            let same = |key: &GUID| {
                let theirs = unsafe { ty.GetUINT32(key) }.ok();
                theirs.is_some() && theirs == unsafe { wanted.GetUINT32(key) }.ok()
            };
            if subtype != MFAudioFormat_Float
                || rate != session.rate()
                || channels != session.channels()
                || !same(&MF_MT_AUDIO_CHANNEL_MASK)
                || !same(&MF_MT_AUDIO_BITS_PER_SAMPLE)
                || !same(&MF_MT_AUDIO_BLOCK_ALIGNMENT)
            {
                return Err(MF_E_INVALIDMEDIATYPE.into());
            }
            if !test_only {
                state.output_type = Some(copy_type(ty)?);
            }
            Ok(())
        })
    }

    fn GetInputCurrentType(&self, _: u32) -> windows::core::Result<IMFMediaType> {
        let state = self.state();
        match state.input_type.as_ref() {
            Some(ty) => copy_type(ty),
            None => Err(MF_E_TRANSFORM_TYPE_NOT_SET.into()),
        }
    }

    fn GetOutputCurrentType(&self, _: u32) -> windows::core::Result<IMFMediaType> {
        let state = self.state();
        match state.output_type.as_ref() {
            Some(ty) => copy_type(ty),
            None => Err(MF_E_TRANSFORM_TYPE_NOT_SET.into()),
        }
    }

    fn GetInputStatus(&self, _: u32) -> windows::core::Result<u32> {
        let accepting = self.state().queue.is_empty();
        Ok(if accepting {
            MFT_INPUT_STATUS_ACCEPT_DATA.0 as u32
        } else {
            0
        })
    }

    fn GetOutputStatus(&self) -> windows::core::Result<u32> {
        let ready = !self.state().queue.is_empty();
        Ok(if ready {
            MFT_OUTPUT_STATUS_SAMPLE_READY.0 as u32
        } else {
            0
        })
    }

    fn SetOutputBounds(&self, _: i64, _: i64) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn ProcessEvent(&self, _: u32, _: Ref<IMFMediaEvent>) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn ProcessMessage(&self, message: MFT_MESSAGE_TYPE, _: usize) -> windows::core::Result<()> {
        guard(self, || {
            let mut state = self.state();
            if message == MFT_MESSAGE_COMMAND_FLUSH {
                state.queue.clear();
                if let Some(session) = state.session.as_mut() {
                    session.reset();
                }
            } else if message == MFT_MESSAGE_COMMAND_DRAIN {
                if let Some(session) = state.session.as_mut() {
                    match session.drain() {
                        Ok(blocks) => state.queue.extend(blocks),
                        Err(e) => log::error!("drain failed: {e}"),
                    }
                }
            } else if message == MFT_MESSAGE_SET_D3D_MANAGER {
                return Err(E_NOTIMPL.into());
            }
            // BEGIN/END_STREAMING and START/END_OF_STREAM: nothing to do.
            Ok(())
        })
    }

    fn ProcessInput(&self, id: u32, sample: Ref<IMFSample>, _: u32) -> windows::core::Result<()> {
        if id != 0 {
            return Err(E_INVALIDARG.into());
        }
        guard(self, || {
            let sample = sample.ok()?;
            let mut state = self.state();
            if state.session.is_none() || state.output_type.is_none() {
                return Err(MF_E_TRANSFORM_TYPE_NOT_SET.into());
            }
            if !state.queue.is_empty() {
                return Err(MF_E_NOTACCEPTING.into());
            }
            let pts = unsafe { sample.GetSampleTime() }.ok();
            let discontinuity =
                unsafe { sample.GetUINT32(&MFSampleExtension_Discontinuity) }.is_ok_and(|v| v != 0);
            let buffer = unsafe { sample.ConvertToContiguousBuffer()? };
            let (mut data, mut length) = (std::ptr::null_mut(), 0u32);
            unsafe { buffer.Lock(&mut data, None, Some(&mut length))? };
            let bytes = unsafe { std::slice::from_raw_parts(data, length as usize) }.to_vec();
            unsafe { buffer.Unlock()? };

            let session = state.session.as_mut().expect("checked above");
            if discontinuity {
                session.reset();
            }
            match session.push(&bytes, pts) {
                Ok(blocks) => state.queue.extend(blocks),
                Err(e) => {
                    // One bad packet must not end playback: start over from
                    // the next one.
                    log::error!("decode failed: {e}; session reset");
                    session.reset();
                }
            }
            Ok(())
        })
    }

    fn ProcessOutput(
        &self,
        _: u32,
        count: u32,
        buffers: *mut MFT_OUTPUT_DATA_BUFFER,
        status: *mut u32,
    ) -> windows::core::Result<()> {
        if count != 1 || buffers.is_null() {
            return Err(E_INVALIDARG.into());
        }
        guard(self, || {
            let mut state = self.state();
            let Some(block) = state.queue.pop_front() else {
                return Err(MF_E_TRANSFORM_NEED_MORE_INPUT.into());
            };
            let more = !state.queue.is_empty();
            drop(state);
            let sample = block_sample(&block)?;
            unsafe {
                (*buffers).pSample = std::mem::ManuallyDrop::new(Some(sample));
                (*buffers).dwStatus = if more {
                    MFT_OUTPUT_DATA_BUFFER_INCOMPLETE.0 as u32
                } else {
                    0
                };
                if !status.is_null() {
                    *status = 0;
                }
            }
            Ok(())
        })
    }
}

#[allow(dead_code)]
const _: i64 = HNS_PER_SECOND; // the timeline is in these units
