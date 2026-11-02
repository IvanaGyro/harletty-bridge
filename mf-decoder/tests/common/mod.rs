//! What the COM tests share: a transform from the class factory, types,
//! samples, and the loops that feed and drain it. Included by
//! `tests/transform.rs` and by the unit tests in `src/com/panic_tests.rs`.
#![allow(dead_code)]

use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, IClassFactory};
use windows::core::{GUID, Interface};

use harletty_mf::com::{CLSID_DECODER, DllGetClassObject};
use harletty_mf::portable::codecs::CODECS;

pub const DTS_CORE: &[u8] =
    include_bytes!("../../../harletty/tests/fixtures/dts_core_tone_10f.dts");
/// DTS-HD: the subtype Windows' Matroska source gives a DTS track.
pub const DTS: usize = 1;
/// Pieces of the stream, as a demuxer would hand them over.
pub const PIECE: usize = 1024;

pub fn decoder() -> IMFTransform {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        MFStartup(MF_VERSION, MFSTARTUP_FULL).unwrap();
        let mut factory: Option<IClassFactory> = None;
        DllGetClassObject(
            &CLSID_DECODER,
            &IClassFactory::IID,
            &mut factory as *mut _ as *mut _,
        )
        .ok()
        .unwrap();
        factory.unwrap().CreateInstance(None).unwrap()
    }
}

pub fn input_type(codec: usize, rate: u32) -> IMFMediaType {
    unsafe {
        let ty = MFCreateMediaType().unwrap();
        ty.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).unwrap();
        ty.SetGUID(&MF_MT_SUBTYPE, &GUID::from_u128(CODECS[codec].subtype))
            .unwrap();
        ty.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, rate).unwrap();
        ty
    }
}

/// One access unit; `pts_hns` is `None` for a sample with no time, which is
/// what a demuxer gives the pieces of a continuous stream after its first.
pub fn access_unit(bytes: &[u8], pts_hns: Option<i64>, discontinuity: bool) -> IMFSample {
    unsafe {
        let sample = MFCreateSample().unwrap();
        let buffer = MFCreateMemoryBuffer(bytes.len() as u32).unwrap();
        let (mut data, mut max) = (std::ptr::null_mut(), 0u32);
        buffer.Lock(&mut data, Some(&mut max), None).unwrap();
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
        buffer.Unlock().unwrap();
        buffer.SetCurrentLength(bytes.len() as u32).unwrap();
        sample.AddBuffer(&buffer).unwrap();
        if let Some(pts) = pts_hns {
            sample.SetSampleTime(pts).unwrap();
        }
        if discontinuity {
            sample
                .SetUINT32(&MFSampleExtension_Discontinuity, 1)
                .unwrap();
        }
        sample
    }
}

pub fn process_output(mft: &IMFTransform) -> windows::core::Result<(IMFSample, u32)> {
    unsafe {
        let mut buffers = [MFT_OUTPUT_DATA_BUFFER::default()];
        let mut status = 0u32;
        mft.ProcessOutput(0, &mut buffers, &mut status)?;
        let sample = std::mem::ManuallyDrop::take(&mut buffers[0].pSample).expect("a sample");
        Ok((sample, buffers[0].dwStatus))
    }
}

/// Negotiate DTS in, float out, and return the output type.
pub fn start(mft: &IMFTransform) -> IMFMediaType {
    unsafe {
        mft.SetInputType(0, &input_type(DTS, 48_000), 0).unwrap();
        let out = mft.GetOutputAvailableType(0, 0).unwrap();
        mft.SetOutputType(0, &out, 0).unwrap();
        mft.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)
            .unwrap();
        out
    }
}

/// What one output sample holds: its time, duration and floats.
pub struct Out {
    pub start: i64,
    pub duration: i64,
    pub samples: Vec<f32>,
}

pub fn read(sample: &IMFSample) -> Out {
    unsafe {
        let buffer = sample.ConvertToContiguousBuffer().unwrap();
        let (mut data, mut len) = (std::ptr::null_mut(), 0u32);
        buffer.Lock(&mut data, None, Some(&mut len)).unwrap();
        let bytes = std::slice::from_raw_parts(data, len as usize).to_vec();
        buffer.Unlock().unwrap();
        Out {
            start: sample.GetSampleTime().unwrap(),
            duration: sample.GetSampleDuration().unwrap(),
            samples: bytes
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                .collect(),
        }
    }
}

/// Take everything the transform will give without more input.
pub fn take_all(mft: &IMFTransform, into: &mut Vec<Out>) {
    while let Ok((sample, _)) = process_output(mft) {
        into.push(read(&sample));
    }
}

/// Feed the whole fixture, taking output as it comes, then drain.
pub fn decode_all(mft: &IMFTransform) -> Vec<Out> {
    let mut out = Vec::new();
    for (i, piece) in DTS_CORE.chunks(PIECE).enumerate() {
        unsafe {
            mft.ProcessInput(0, &access_unit(piece, (i == 0).then_some(0), false), 0)
                .unwrap()
        };
        take_all(mft, &mut out);
    }
    unsafe { mft.ProcessMessage(MFT_MESSAGE_COMMAND_DRAIN, 0).unwrap() };
    take_all(mft, &mut out);
    out
}

/// Feed pieces until the transform holds output, and return how many it took.
pub fn feed_until_output(mft: &IMFTransform) -> usize {
    for (i, piece) in DTS_CORE.chunks(PIECE).enumerate() {
        unsafe {
            mft.ProcessInput(0, &access_unit(piece, (i == 0).then_some(0), false), 0)
                .unwrap()
        };
        if unsafe { mft.GetOutputStatus().unwrap() } == MFT_OUTPUT_STATUS_SAMPLE_READY.0 as u32 {
            return i + 1;
        }
    }
    panic!("the fixture produced no output");
}
