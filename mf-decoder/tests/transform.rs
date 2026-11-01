//! The transform driven through COM, the way a player drives it: class
//! factory, input type, output type, access units in, audio out, flush, drain.
//! The decoder is the real one: the engine and the DTS bridge library (which
//! cargo builds next to this executable because the plugin crates are
//! dev-dependencies), decoding the repository's DTS core fixture. No
//! registration, no administrator rights, no audio device.
#![cfg(windows)]

use std::sync::Once;

use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, IClassFactory};
use windows::core::{GUID, Interface};

use harletty_mf::com::{CLSID_DECODER, DllGetClassObject};
use harletty_mf::portable::codecs::CODECS;

const DTS_CORE: &[u8] = include_bytes!("../../harletty/tests/fixtures/dts_core_tone_10f.dts");
/// DTS-HD: the subtype Windows' Matroska source gives a DTS track.
const DTS: usize = 1;
/// Pieces of the stream, as a demuxer would hand them over.
const PIECE: usize = 1024;

fn decoder() -> IMFTransform {
    static ENV: Once = Once::new();
    ENV.call_once(|| {
        // The bridge libraries sit next to this executable.
        let dir = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        // SAFETY: set once, before any thread of ours reads it.
        unsafe { std::env::set_var("HARLETTY_MF_BRIDGE_DIR", dir) };
    });
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

fn input_type(codec: usize, rate: u32) -> IMFMediaType {
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
fn access_unit(bytes: &[u8], pts_hns: Option<i64>, discontinuity: bool) -> IMFSample {
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

fn process_output(mft: &IMFTransform) -> windows::core::Result<(IMFSample, u32)> {
    unsafe {
        let mut buffers = [MFT_OUTPUT_DATA_BUFFER::default()];
        let mut status = 0u32;
        mft.ProcessOutput(0, &mut buffers, &mut status)?;
        let sample = std::mem::ManuallyDrop::take(&mut buffers[0].pSample).expect("a sample");
        Ok((sample, buffers[0].dwStatus))
    }
}

/// Negotiate DTS in, float out, and return the output type.
fn start(mft: &IMFTransform) -> IMFMediaType {
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
struct Out {
    start: i64,
    duration: i64,
    samples: Vec<f32>,
}

fn read(sample: &IMFSample) -> Out {
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
fn take_all(mft: &IMFTransform, into: &mut Vec<Out>) {
    while let Ok((sample, _)) = process_output(mft) {
        into.push(read(&sample));
    }
}

/// Feed the whole fixture, taking output as it comes, then drain.
fn decode_all(mft: &IMFTransform) -> Vec<Out> {
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
fn feed_until_output(mft: &IMFTransform) -> usize {
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

#[test]
fn the_output_type_is_float_with_a_channel_mask() {
    let mft = decoder();
    // Before an input type is set there is no output type to offer.
    assert_eq!(
        unsafe { mft.GetOutputAvailableType(0, 0) }
            .unwrap_err()
            .code(),
        MF_E_TRANSFORM_TYPE_NOT_SET
    );
    let out = start(&mft);
    unsafe {
        assert_eq!(out.GetGUID(&MF_MT_SUBTYPE).unwrap(), MFAudioFormat_Float);
        assert_eq!(
            out.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND).unwrap(),
            48_000
        );
        let channels = out.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS).unwrap();
        let mask = out.GetUINT32(&MF_MT_AUDIO_CHANNEL_MASK).unwrap();
        assert_eq!(channels, mask.count_ones());
        assert_eq!(out.GetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE).unwrap(), 32);
        assert_eq!(
            mft.GetOutputAvailableType(0, 1).unwrap_err().code(),
            MF_E_NO_MORE_TYPES
        );
    }
}

#[test]
fn input_types_are_the_enabled_codecs_only() {
    let mft = decoder();
    let mut offered = 0;
    while unsafe { mft.GetInputAvailableType(0, offered) }.is_ok() {
        offered += 1;
    }
    // TrueHD, DTS-HD, DTS: Dolby Digital is opt-in.
    assert_eq!(offered, 3);
    let dolby_digital = input_type(3, 48_000);
    unsafe {
        assert_eq!(
            mft.SetInputType(0, &dolby_digital, MFT_SET_TYPE_TEST_ONLY.0 as u32)
                .unwrap_err()
                .code(),
            MF_E_INVALIDMEDIATYPE
        );
    }
}

#[test]
fn the_fixture_decodes_to_audio_that_tiles_on_the_timeline() {
    let mft = decoder();
    let channels = unsafe { start(&mft).GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS).unwrap() } as usize;
    let out = decode_all(&mft);
    assert!(!out.is_empty(), "no audio came out");
    let frames: usize = out.iter().map(|o| o.samples.len() / channels).sum();
    assert!(frames > 0);
    for o in &out {
        assert_eq!(o.samples.len() % channels, 0);
    }
    for pair in out.windows(2) {
        assert_eq!(pair[0].start + pair[0].duration, pair[1].start);
    }
    let peak = out
        .iter()
        .flat_map(|o| o.samples.iter())
        .fold(0.0f32, |m, &v| m.max(v.abs()));
    assert!(peak > 1e-3, "the output is silent");
}

#[test]
fn input_is_refused_until_the_output_is_taken() {
    let mft = decoder();
    start(&mft);
    let taken = feed_until_output(&mft);
    unsafe {
        let next = &DTS_CORE[(taken * PIECE).min(DTS_CORE.len() - 1)..];
        assert_eq!(mft.GetInputStatus(0).unwrap(), 0);
        assert_eq!(
            mft.ProcessInput(0, &access_unit(&next[..1], None, false), 0)
                .unwrap_err()
                .code(),
            MF_E_NOTACCEPTING
        );
        let mut sink = Vec::new();
        take_all(&mft, &mut sink);
        assert_eq!(
            mft.GetInputStatus(0).unwrap(),
            MFT_INPUT_STATUS_ACCEPT_DATA.0 as u32
        );
    }
}

#[test]
fn flush_drops_what_is_queued_and_decoding_starts_over() {
    let mft = decoder();
    start(&mft);
    feed_until_output(&mft);
    unsafe {
        mft.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0).unwrap();
        assert_eq!(mft.GetOutputStatus().unwrap(), 0);
        assert_eq!(
            process_output(&mft).unwrap_err().code(),
            MF_E_TRANSFORM_NEED_MORE_INPUT
        );
    }
    // The stream again from the start still decodes.
    assert!(!decode_all(&mft).is_empty());
}

#[test]
fn a_discontinuity_flag_restarts_the_stream_without_an_error() {
    let mft = decoder();
    start(&mft);
    feed_until_output(&mft);
    let mut sink = Vec::new();
    take_all(&mft, &mut sink);
    unsafe {
        mft.ProcessInput(
            0,
            &access_unit(&DTS_CORE[..PIECE], Some(500_000_000), true),
            0,
        )
        .unwrap()
    };
    take_all(&mft, &mut sink);
}

#[test]
fn garbage_in_is_not_an_error_and_does_not_stop_playback() {
    let mft = decoder();
    start(&mft);
    let garbage = vec![0xA5u8; 4096];
    unsafe {
        mft.ProcessInput(0, &access_unit(&garbage, Some(0), false), 0)
            .unwrap()
    };
    let mut sink = Vec::new();
    take_all(&mft, &mut sink);
    // A real stream after it still decodes.
    assert!(!decode_all(&mft).is_empty());
}

#[test]
fn drain_after_everything_is_taken_is_quiet() {
    let mft = decoder();
    start(&mft);
    decode_all(&mft);
    unsafe {
        mft.ProcessMessage(MFT_MESSAGE_COMMAND_DRAIN, 0).unwrap();
        assert_eq!(
            process_output(&mft).unwrap_err().code(),
            MF_E_TRANSFORM_NEED_MORE_INPUT
        );
    }
}

#[test]
fn an_output_type_that_does_not_describe_our_buffers_is_refused() {
    let mft = decoder();
    let offered = start(&mft);
    let altered = |key: &GUID, value: u32| -> IMFMediaType {
        unsafe {
            let ty = MFCreateMediaType().unwrap();
            offered.CopyAllItems(&ty).unwrap();
            ty.SetUINT32(key, value).unwrap();
            ty
        }
    };
    let test_only = MFT_SET_TYPE_TEST_ONLY.0 as u32;
    unsafe {
        // The offered type itself is fine.
        mft.SetOutputType(0, &offered, test_only).unwrap();
        // Wider samples, another layout, another alignment: all refused.
        for bad in [
            altered(&MF_MT_AUDIO_BITS_PER_SAMPLE, 64),
            altered(&MF_MT_AUDIO_CHANNEL_MASK, 0x7),
            altered(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 3),
        ] {
            assert_eq!(
                mft.SetOutputType(0, &bad, test_only).unwrap_err().code(),
                MF_E_INVALIDMEDIATYPE
            );
        }
        // One that leaves the mask out does not say what the channels are.
        let no_mask = MFCreateMediaType().unwrap();
        offered.CopyAllItems(&no_mask).unwrap();
        no_mask.DeleteItem(&MF_MT_AUDIO_CHANNEL_MASK).unwrap();
        assert_eq!(
            mft.SetOutputType(0, &no_mask, test_only)
                .unwrap_err()
                .code(),
            MF_E_INVALIDMEDIATYPE
        );
    }
}

#[test]
fn current_types_are_copies_the_host_cannot_edit_through() {
    let mft = decoder();
    let offered = start(&mft);
    unsafe {
        // Edit what we hand out: the next query is unaffected.
        let current = mft.GetOutputCurrentType(0).unwrap();
        current.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 64).unwrap();
        let again = mft.GetOutputCurrentType(0).unwrap();
        assert_eq!(again.GetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE).unwrap(), 32);

        // Edit the type we were given after accepting it: also unaffected.
        let mine = MFCreateMediaType().unwrap();
        offered.CopyAllItems(&mine).unwrap();
        mft.SetOutputType(0, &mine, 0).unwrap();
        mine.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 64).unwrap();
        let stored = mft.GetOutputCurrentType(0).unwrap();
        assert_eq!(stored.GetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE).unwrap(), 32);

        // The same for the input type.
        let input = mft.GetInputCurrentType(0).unwrap();
        input
            .SetGUID(&MF_MT_SUBTYPE, &GUID::from_u128(CODECS[0].subtype))
            .unwrap();
        let input_again = mft.GetInputCurrentType(0).unwrap();
        assert_eq!(
            input_again.GetGUID(&MF_MT_SUBTYPE).unwrap(),
            GUID::from_u128(CODECS[DTS].subtype)
        );
    }
}
