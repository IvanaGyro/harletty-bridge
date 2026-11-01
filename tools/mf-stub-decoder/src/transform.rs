//! The stub transform: any audio input type is accepted, the output is 32-bit
//! float stereo at 48 kHz, and every input sample comes back as silence of the
//! same duration.

use std::sync::Mutex;

use windows::Win32::Foundation::{E_INVALIDARG, E_NOTIMPL};
use windows::Win32::Media::MediaFoundation::*;
use windows::core::{GUID, Ref, implement};

use crate::log;

const RATE: u32 = 48_000;
const CHANNELS: u32 = 2;
const BYTES_PER_FRAME: u32 = CHANNELS * 4;

#[derive(Default)]
struct State {
    input_type: Option<IMFMediaType>,
    output_type: Option<IMFMediaType>,
    /// Time and duration of the input sample waiting for `ProcessOutput`.
    pending: Option<(i64, i64)>,
    inputs: u64,
    outputs: u64,
}

#[implement(IMFTransform)]
pub struct Stub {
    state: Mutex<State>,
}

pub fn new() -> Stub {
    Stub {
        state: Mutex::new(State::default()),
    }
}

fn float_stereo() -> windows::core::Result<IMFMediaType> {
    unsafe {
        let ty = MFCreateMediaType()?;
        ty.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
        ty.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float)?;
        ty.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, RATE)?;
        ty.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, CHANNELS)?;
        ty.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 32)?;
        ty.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, BYTES_PER_FRAME)?;
        ty.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, BYTES_PER_FRAME * RATE)?;
        ty.SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1)?;
        Ok(ty)
    }
}

fn subtype_text(ty: &IMFMediaType) -> String {
    let subtype: Option<GUID> = unsafe { ty.GetGUID(&MF_MT_SUBTYPE).ok() };
    match subtype {
        Some(guid) => format!("{guid:?}"),
        None => "none".into(),
    }
}

impl IMFTransform_Impl for Stub_Impl {
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

    fn GetInputAvailableType(&self, _: u32, _: u32) -> windows::core::Result<IMFMediaType> {
        // No preferred input: the caller proposes one.
        Err(MF_E_NO_MORE_TYPES.into())
    }

    fn GetOutputAvailableType(&self, id: u32, index: u32) -> windows::core::Result<IMFMediaType> {
        if id != 0 {
            return Err(E_INVALIDARG.into());
        }
        if self.state.lock().unwrap().input_type.is_none() {
            return Err(MF_E_TRANSFORM_TYPE_NOT_SET.into());
        }
        if index > 0 {
            return Err(MF_E_NO_MORE_TYPES.into());
        }
        float_stereo()
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
        let test_only = flags & MFT_SET_TYPE_TEST_ONLY.0 as u32 != 0;
        let mut state = self.state.lock().unwrap();
        match ty.as_ref() {
            None => {
                state.input_type = None;
                state.output_type = None;
            }
            Some(ty) => {
                let major = unsafe { ty.GetGUID(&MF_MT_MAJOR_TYPE)? };
                if major != MFMediaType_Audio {
                    return Err(MF_E_INVALIDMEDIATYPE.into());
                }
                log::line(&format!(
                    "SetInputType subtype={} test_only={test_only}",
                    subtype_text(ty)
                ));
                if !test_only {
                    state.input_type = Some(ty.clone());
                }
            }
        }
        Ok(())
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
        let test_only = flags & MFT_SET_TYPE_TEST_ONLY.0 as u32 != 0;
        let mut state = self.state.lock().unwrap();
        if state.input_type.is_none() {
            return Err(MF_E_TRANSFORM_TYPE_NOT_SET.into());
        }
        match ty.as_ref() {
            None => state.output_type = None,
            Some(ty) => {
                let subtype = unsafe { ty.GetGUID(&MF_MT_SUBTYPE)? };
                let rate = unsafe { ty.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)? };
                let channels = unsafe { ty.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS)? };
                if subtype != MFAudioFormat_Float || rate != RATE || channels != CHANNELS {
                    return Err(MF_E_INVALIDMEDIATYPE.into());
                }
                log::line(&format!("SetOutputType ok test_only={test_only}"));
                if !test_only {
                    state.output_type = Some(ty.clone());
                }
            }
        }
        Ok(())
    }

    fn GetInputCurrentType(&self, _: u32) -> windows::core::Result<IMFMediaType> {
        self.state
            .lock()
            .unwrap()
            .input_type
            .clone()
            .ok_or_else(|| MF_E_TRANSFORM_TYPE_NOT_SET.into())
    }

    fn GetOutputCurrentType(&self, _: u32) -> windows::core::Result<IMFMediaType> {
        self.state
            .lock()
            .unwrap()
            .output_type
            .clone()
            .ok_or_else(|| MF_E_TRANSFORM_TYPE_NOT_SET.into())
    }

    fn GetInputStatus(&self, _: u32) -> windows::core::Result<u32> {
        let accepting = self.state.lock().unwrap().pending.is_none();
        Ok(if accepting {
            MFT_INPUT_STATUS_ACCEPT_DATA.0 as u32
        } else {
            0
        })
    }

    fn GetOutputStatus(&self) -> windows::core::Result<u32> {
        let ready = self.state.lock().unwrap().pending.is_some();
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
        {
            let state = self.state.lock().unwrap();
            log::line(&format!(
                "ProcessMessage {} (inputs={} outputs={})",
                message.0, state.inputs, state.outputs
            ));
        }
        if message == MFT_MESSAGE_COMMAND_FLUSH {
            self.state.lock().unwrap().pending = None;
        }
        Ok(())
    }

    fn ProcessInput(&self, _: u32, sample: Ref<IMFSample>, _: u32) -> windows::core::Result<()> {
        let sample = sample.ok()?;
        let mut state = self.state.lock().unwrap();
        if state.input_type.is_none() || state.output_type.is_none() {
            return Err(MF_E_TRANSFORM_TYPE_NOT_SET.into());
        }
        if state.pending.is_some() {
            return Err(MF_E_NOTACCEPTING.into());
        }
        let time = unsafe { sample.GetSampleTime().unwrap_or(0) };
        // A compressed sample's duration may be unset: fall back to 10 ms.
        let duration = unsafe { sample.GetSampleDuration().unwrap_or(0) };
        state.pending = Some((time, if duration > 0 { duration } else { 100_000 }));
        state.inputs += 1;
        if state.inputs <= 3 || state.inputs % 100 == 0 {
            log::line(&format!(
                "ProcessInput #{} time={time} duration={duration}",
                state.inputs
            ));
        }
        Ok(())
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
        let mut state = self.state.lock().unwrap();
        let Some((time, duration)) = state.pending.take() else {
            return Err(MF_E_TRANSFORM_NEED_MORE_INPUT.into());
        };
        state.outputs += 1;
        if state.outputs <= 3 || state.outputs % 100 == 0 {
            log::line(&format!(
                "ProcessOutput #{} time={time} duration={duration}",
                state.outputs
            ));
        }
        let frames = (duration as u64 * RATE as u64 / 10_000_000).max(1) as u32;
        let bytes = frames * BYTES_PER_FRAME;
        unsafe {
            let buffer = MFCreateMemoryBuffer(bytes)?;
            let mut data = std::ptr::null_mut();
            buffer.Lock(&mut data, None, None)?;
            std::ptr::write_bytes(data, 0, bytes as usize);
            buffer.Unlock()?;
            buffer.SetCurrentLength(bytes)?;
            let out = MFCreateSample()?;
            out.AddBuffer(&buffer)?;
            out.SetSampleTime(time)?;
            out.SetSampleDuration(duration)?;
            (*buffers).pSample = std::mem::ManuallyDrop::new(Some(out));
            (*buffers).dwStatus = 0;
            if !status.is_null() {
                *status = 0;
            }
        }
        Ok(())
    }
}
