//! The transform driven through COM, the way a player drives it: class
//! factory, input type, output type, samples in, samples out, flush. No
//! registration, no administrator rights, no audio device.
#![cfg(windows)]

use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, IClassFactory};
use windows::core::{GUID, Interface};

use harletty_mf::com::{CLSID_DECODER, DllGetClassObject};
use harletty_mf::portable::codecs::CODECS;

const TRUEHD: usize = 0;

fn decoder() -> IMFTransform {
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

fn access_unit(pts_hns: i64, discontinuity: bool) -> IMFSample {
    unsafe {
        let sample = MFCreateSample().unwrap();
        let buffer = MFCreateMemoryBuffer(16).unwrap();
        buffer.SetCurrentLength(16).unwrap();
        sample.AddBuffer(&buffer).unwrap();
        sample.SetSampleTime(pts_hns).unwrap();
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

/// Negotiate TrueHD in, float out, and return the output type.
fn start(mft: &IMFTransform) -> IMFMediaType {
    unsafe {
        mft.SetInputType(0, &input_type(TRUEHD, 48_000), 0).unwrap();
        let out = mft.GetOutputAvailableType(0, 0).unwrap();
        mft.SetOutputType(0, &out, 0).unwrap();
        mft.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)
            .unwrap();
        out
    }
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
fn samples_in_audio_out_with_matching_times() {
    let mft = decoder();
    let out = start(&mft);
    let rate = 48_000u64;
    let channels = unsafe { out.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS).unwrap() } as usize;
    unsafe {
        // The first sample only starts the timeline.
        mft.ProcessInput(0, &access_unit(1_000_000, false), 0)
            .unwrap();
        assert_eq!(
            process_output(&mft).unwrap_err().code(),
            MF_E_TRANSFORM_NEED_MORE_INPUT
        );

        // Blocks 0.83 ms apart, no durations: 100 of them.
        let mut frames = 0u64;
        let mut next_start = 1_000_000i64;
        for i in 1..=100 {
            mft.ProcessInput(0, &access_unit(1_000_000 + i * 8_333, false), 0)
                .unwrap();
            while let Ok((sample, _)) = process_output(&mft) {
                assert_eq!(sample.GetSampleTime().unwrap(), next_start);
                let duration = sample.GetSampleDuration().unwrap();
                let buffer = sample.ConvertToContiguousBuffer().unwrap();
                let len = buffer.GetCurrentLength().unwrap() as usize;
                assert_eq!(len % (channels * 4), 0);
                let block_frames = (len / (channels * 4)) as u64;
                frames += block_frames;
                next_start += duration;
            }
        }
        // 100 blocks cover 83.3 ms, not 10 s.
        let expected = (100 * 8_333) as u64 * rate / 10_000_000;
        assert_eq!(frames, expected);
    }
}

#[test]
fn input_is_refused_until_the_output_is_taken() {
    let mft = decoder();
    start(&mft);
    unsafe {
        mft.ProcessInput(0, &access_unit(0, false), 0).unwrap();
        mft.ProcessInput(0, &access_unit(100_000, false), 0)
            .unwrap();
        assert_eq!(
            mft.GetOutputStatus().unwrap(),
            MFT_OUTPUT_STATUS_SAMPLE_READY.0 as u32
        );
        assert_eq!(
            mft.ProcessInput(0, &access_unit(200_000, false), 0)
                .unwrap_err()
                .code(),
            MF_E_NOTACCEPTING
        );
        process_output(&mft).unwrap();
        assert_eq!(
            mft.GetInputStatus(0).unwrap(),
            MFT_INPUT_STATUS_ACCEPT_DATA.0 as u32
        );
    }
}

#[test]
fn flush_and_discontinuity_start_a_new_timeline() {
    let mft = decoder();
    start(&mft);
    unsafe {
        mft.ProcessInput(0, &access_unit(0, false), 0).unwrap();
        mft.ProcessInput(0, &access_unit(100_000, false), 0)
            .unwrap();
        mft.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0).unwrap();
        assert_eq!(
            process_output(&mft).unwrap_err().code(),
            MF_E_TRANSFORM_NEED_MORE_INPUT
        );

        // After a flush the next sample starts over, wherever it is.
        mft.ProcessInput(0, &access_unit(90_000_000, false), 0)
            .unwrap();
        assert_eq!(
            process_output(&mft).unwrap_err().code(),
            MF_E_TRANSFORM_NEED_MORE_INPUT
        );
        mft.ProcessInput(0, &access_unit(90_100_000, false), 0)
            .unwrap();
        assert_eq!(
            process_output(&mft).unwrap().0.GetSampleTime().unwrap(),
            90_000_000
        );

        // A discontinuity flag does the same.
        mft.ProcessInput(0, &access_unit(500_000_000, true), 0)
            .unwrap();
        assert_eq!(
            process_output(&mft).unwrap_err().code(),
            MF_E_TRANSFORM_NEED_MORE_INPUT
        );
    }
}

#[test]
fn drain_with_nothing_held_is_quiet() {
    let mft = decoder();
    start(&mft);
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
            .SetGUID(&MF_MT_SUBTYPE, &GUID::from_u128(CODECS[1].subtype))
            .unwrap();
        let input_again = mft.GetInputCurrentType(0).unwrap();
        assert_eq!(
            input_again.GetGUID(&MF_MT_SUBTYPE).unwrap(),
            GUID::from_u128(CODECS[TRUEHD].subtype)
        );
    }
}
