//! `regsvr32` support: the COM class under `HKLM\SOFTWARE\Classes\CLSID`, then
//! `MFTRegister` so `MFTEnumEx` finds it. Needs an administrator.

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, HMODULE, MAX_PATH};
use windows::Win32::Media::MediaFoundation::{
    MF_VERSION, MFAudioFormat_Float, MFMediaType_Audio, MFSTARTUP_FULL, MFShutdown, MFStartup,
    MFT_CATEGORY_AUDIO_DECODER, MFT_REGISTER_TYPE_INFO, MFTRegister, MFTUnregister,
};
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    GetModuleFileNameW, GetModuleHandleExW,
};
use windows::Win32::System::Registry::{
    HKEY_LOCAL_MACHINE, REG_SZ, RegDeleteTreeW, RegSetKeyValueW,
};
use windows::core::{GUID, HSTRING, PCWSTR, w};

use crate::CLSID_STUB;

const NAME: PCWSTR = w!("harletty MF stub decoder (phase 0)");

/// TrueHD, DTS-HD, DTS (Microsoft's names; the Phase 0 probe confirms what the
/// MKV source really sets).
const DEFAULT_INPUTS: [GUID; 3] = [
    GUID::from_u128(0xEB27CEC4_163E_4CA3_8B74_8E25F91B517E),
    GUID::from_u128(0xA2E58EB7_0FA9_48BB_A40C_FA0E156D0645),
    GUID::from_u128(0x00000008_0000_0010_8000_00AA00389B71),
];
/// E-AC-3 (`MFAudioFormat_Dolby_DDPlus`) and AC-3 (`MFAudioFormat_Dolby_AC3`).
const DOLBY_DIGITAL_INPUTS: [GUID; 2] = [
    GUID::from_u128(0xA7FB87AF_2D02_42FB_A4D4_05CD93843BDD),
    GUID::from_u128(0xE06D802C_DB46_11CF_B4D1_00805F6CBBEA),
];

fn clsid_key() -> HSTRING {
    HSTRING::from(format!(
        "SOFTWARE\\Classes\\CLSID\\{{{}}}",
        format!("{:?}", CLSID_STUB).to_uppercase()
    ))
}

fn dll_path() -> windows::core::Result<Vec<u16>> {
    unsafe {
        let mut module = HMODULE::default();
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(register as *const () as *const u16),
            &mut module,
        )?;
        let mut buffer = vec![0u16; MAX_PATH as usize * 2];
        let len = GetModuleFileNameW(Some(module), &mut buffer) as usize;
        if len == 0 {
            return Err(windows::core::Error::from_thread());
        }
        buffer.truncate(len);
        buffer.push(0);
        Ok(buffer)
    }
}

fn set_sz(subkey: &HSTRING, name: PCWSTR, value: &[u16]) -> windows::core::Result<()> {
    let status = unsafe {
        RegSetKeyValueW(
            HKEY_LOCAL_MACHINE,
            &*subkey,
            name,
            REG_SZ.0,
            Some(value.as_ptr() as *const _),
            (value.len() * 2) as u32,
        )
    };
    status.ok()
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

pub fn register() -> windows::core::Result<()> {
    let key = clsid_key();
    set_sz(&key, PCWSTR::null(), &wide("harletty MF stub decoder"))?;
    let server = HSTRING::from(format!("{key}\\InprocServer32"));
    set_sz(&server, PCWSTR::null(), &dll_path()?)?;
    set_sz(&server, w!("ThreadingModel"), &wide("Both"))?;

    let mut inputs: Vec<MFT_REGISTER_TYPE_INFO> = DEFAULT_INPUTS.iter().map(input).collect();
    if std::env::var("HARLETTY_STUB_EAC3").is_ok_and(|v| v == "1") {
        inputs.extend(DOLBY_DIGITAL_INPUTS.iter().map(input));
    }
    let outputs = [MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Audio,
        guidSubtype: MFAudioFormat_Float,
    }];
    unsafe {
        MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
        let result = MFTRegister(
            CLSID_STUB,
            MFT_CATEGORY_AUDIO_DECODER,
            NAME,
            0,
            Some(&inputs),
            Some(&outputs),
            None,
        );
        let _ = MFShutdown();
        result
    }
}

fn input(subtype: &GUID) -> MFT_REGISTER_TYPE_INFO {
    MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Audio,
        guidSubtype: *subtype,
    }
}

/// Idempotent: whatever is already gone is fine, but a failure to remove
/// something that is there (no administrator rights, say) is reported, so
/// `regsvr32 /u` does not claim success and leave the decoder installed. Both
/// removals are tried before the first error is returned.
pub fn unregister() -> windows::core::Result<()> {
    unsafe {
        MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
        let mft = MFTUnregister(CLSID_STUB);
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
