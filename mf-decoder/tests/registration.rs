//! `regsvr32` on the built DLL, then `MFTEnumEx` finds the decoder, then
//! `regsvr32 /u` and it is gone. Writes under HKLM, so it needs an
//! administrator (the hosted runners are) and runs only when
//! `HARLETTY_MF_REGISTRY_TEST=1`.
#![cfg(windows)]

use std::path::PathBuf;
use std::process::Command;

use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree};
use windows::core::GUID;

use harletty_mf::com::CLSID_DECODER;
use harletty_mf::portable::codecs::CODECS;

/// `target/<profile>/harletty_mf.dll`, next to `deps/` where tests run.
fn built_dll() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    exe.parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("harletty_mf.dll")
}

fn regsvr32(args: &[&str]) {
    let status = Command::new("regsvr32")
        .args(args)
        .arg(built_dll())
        .status()
        .expect("regsvr32 runs");
    assert!(status.success(), "regsvr32 {args:?}: {status}");
}

/// Whether `MFTEnumEx` lists our decoder for TrueHD in.
fn listed() -> bool {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        MFStartup(MF_VERSION, MFSTARTUP_FULL).unwrap();
        let input = MFT_REGISTER_TYPE_INFO {
            guidMajorType: MFMediaType_Audio,
            guidSubtype: GUID::from_u128(CODECS[0].subtype),
        };
        let mut activates: *mut Option<IMFActivate> = std::ptr::null_mut();
        let mut count = 0u32;
        MFTEnumEx(
            MFT_CATEGORY_AUDIO_DECODER,
            MFT_ENUM_FLAG_ALL,
            Some(&input),
            None,
            &mut activates,
            &mut count,
        )
        .unwrap();
        let mut found = false;
        for i in 0..count as usize {
            if let Some(activate) = (*activates.add(i)).take() {
                found |= activate
                    .GetGUID(&MFT_TRANSFORM_CLSID_Attribute)
                    .is_ok_and(|g| g == CLSID_DECODER);
            }
        }
        if !activates.is_null() {
            CoTaskMemFree(Some(activates as *const _));
        }
        MFShutdown().unwrap();
        found
    }
}

#[test]
fn register_enumerate_unregister() {
    if std::env::var("HARLETTY_MF_REGISTRY_TEST").as_deref() != Ok("1") {
        eprintln!("skipped: HARLETTY_MF_REGISTRY_TEST is not 1");
        return;
    }
    assert!(
        built_dll().exists(),
        "{} was not built",
        built_dll().display()
    );
    assert!(!listed(), "already registered: unregister it first");
    regsvr32(&["/s"]);
    let found = listed();
    // Clean up before asserting, so a failure does not leave it installed.
    regsvr32(&["/s", "/u"]);
    assert!(found, "the decoder was not listed after regsvr32");
    assert!(!listed(), "the decoder is still listed after regsvr32 /u");
}
