//! The speaker mask of the default render endpoint, from its shared-mode mix
//! format. (`PKEY_AudioEndpoint_PhysicalSpeakers` would say what is plugged
//! in; the mix format says what the audio engine mixes to, which is what the
//! player's output is converted to anyway.)

use windows::Win32::Media::Audio::{
    IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator, WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
    eConsole, eRender,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
};

const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
/// Used when the mix format has no mask: mono and stereo by channel count.
fn mask_for_channels(channels: u16) -> u32 {
    match channels {
        1 => 0x4,
        _ => 0x3,
    }
}

pub fn speaker_mask() -> Option<u32> {
    unsafe {
        // MTA is what Media Foundation's worker threads use; a failure here
        // (already initialised differently) is fine.
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
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
