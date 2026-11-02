//! The speaker mask of the default render endpoint, from its shared-mode mix
//! format. (`PKEY_AudioEndpoint_PhysicalSpeakers` would say what is plugged
//! in; the mix format says what the audio engine mixes to, which is what the
//! player's output is converted to anyway.)
//!
//! The mask is read once, when the input type is set and the session is built.
//! If the default endpoint changes while a file plays (headphones plugged in,
//! another device chosen), that file keeps the layout it started with and the
//! player's own conversion carries it to the new device; the next file is
//! built with the new endpoint's mask. `mf.yaml`'s `speaker_mask` pins a mask
//! regardless of the endpoint. Rebuilding mid-stream
//! (`MF_E_TRANSFORM_STREAM_CHANGE`) is not done: it is optional in the plan
//! and only worth it if WMP renegotiates cleanly, which no test has shown.

use windows::Win32::Media::Audio::{
    IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator, WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
    eConsole, eRender,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    CoUninitialize,
};

const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
/// Used when the mix format has no mask: mono and stereo by channel count.
fn mask_for_channels(channels: u16) -> u32 {
    match channels {
        1 => 0x4,
        _ => 0x3,
    }
}

/// Balances a successful `CoInitializeEx` (`S_OK` or `S_FALSE`); a failed one
/// (the thread is already an STA, say) is left alone.
struct ComGuard(bool);

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

pub fn speaker_mask() -> Option<u32> {
    // MTA is what Media Foundation's worker threads use. The interfaces are
    // released inside `query`, before the guard uninitialises COM.
    let _com = ComGuard(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok());
    query()
}

fn query() -> Option<u32> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None).ok()?;
        let format: *mut WAVEFORMATEX = client.GetMixFormat().ok()?;
        if format.is_null() {
            return None;
        }
        let base = format.read_unaligned();
        let mask = if base.wFormatTag == WAVE_FORMAT_EXTENSIBLE {
            let extensible = (format as *const WAVEFORMATEXTENSIBLE).read_unaligned();
            extensible.dwChannelMask
        } else {
            mask_for_channels(base.nChannels)
        };
        CoTaskMemFree(Some(format as *const _));
        (mask != 0).then_some(mask)
    }
}
