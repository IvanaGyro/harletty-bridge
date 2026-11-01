//! `regsvr32` support: the COM class under `HKLM\SOFTWARE\Classes\CLSID`, then
//! `MFTRegister` so `MFTEnumEx` finds it. Needs an administrator.

use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::Media::MediaFoundation::{
    MF_VERSION, MFAudioFormat_Float, MFMediaType_Audio, MFSTARTUP_FULL, MFShutdown, MFStartup,
    MFT_CATEGORY_AUDIO_DECODER, MFT_REGISTER_TYPE_INFO, MFTRegister, MFTUnregister,
};
use windows::Win32::System::Registry::{
    HKEY_LOCAL_MACHINE, REG_SZ, RegDeleteTreeW, RegSetKeyValueW,
};
use windows::core::{GUID, HSTRING, PCWSTR, w};

use super::{CLSID_DECODER, FRIENDLY_NAME};
use crate::portable::codecs::CODECS;

fn clsid_key() -> HSTRING {
    HSTRING::from(format!(
        "SOFTWARE\\Classes\\CLSID\\{{{}}}",
        format!("{CLSID_DECODER:?}").to_uppercase()
    ))
}

fn set_sz(subkey: &HSTRING, name: PCWSTR, value: &str) -> windows::core::Result<()> {
    let wide: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
    unsafe {
        RegSetKeyValueW(
            HKEY_LOCAL_MACHINE,
            subkey,
            name,
            REG_SZ.0,
            Some(wide.as_ptr() as *const _),
            (wide.len() * 2) as u32,
        )
        .ok()
    }
}

pub fn register() -> windows::core::Result<()> {
    let dll = super::paths::dll_file()?;
    let key = clsid_key();
    set_sz(&key, PCWSTR::null(), FRIENDLY_NAME)?;
    let server = HSTRING::from(format!("{key}\\InprocServer32"));
    set_sz(&server, PCWSTR::null(), &dll.display().to_string())?;
    set_sz(&server, w!("ThreadingModel"), "Both")?;

    let config = super::paths::load_config();
    let inputs: Vec<MFT_REGISTER_TYPE_INFO> = CODECS
        .iter()
        .filter(|codec| config.enabled(codec))
        .map(|codec| MFT_REGISTER_TYPE_INFO {
            guidMajorType: MFMediaType_Audio,
            guidSubtype: GUID::from_u128(codec.subtype),
        })
        .collect();
    let outputs = [MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Audio,
        guidSubtype: MFAudioFormat_Float,
    }];
    unsafe {
        MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
        let result = MFTRegister(
            CLSID_DECODER,
            MFT_CATEGORY_AUDIO_DECODER,
            &HSTRING::from(FRIENDLY_NAME),
            0,
            Some(&inputs),
            Some(&outputs),
            None,
        );
        let _ = MFShutdown();
        result
    }
}

/// Idempotent: what is already gone is fine, but a failure to remove something
/// that is there (no administrator rights, say) is reported, so `regsvr32 /u`
/// does not claim success and leave the decoder installed. Both removals are
/// tried before the first error is returned.
pub fn unregister() -> windows::core::Result<()> {
    unsafe {
        MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
        let mft = MFTUnregister(CLSID_DECODER);
        let _ = MFShutdown();
        let key = RegDeleteTreeW(HKEY_LOCAL_MACHINE, &clsid_key());
        let key = if key == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            key.ok()
        };
        mft.and(key)
    }
}
