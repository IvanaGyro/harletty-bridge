//! A WASAPI loopback capture of the default render endpoint: what reaches the
//! headphones, after Windows has mixed and downmixed it. `--measure` uses it
//! to level-match two ways of decoding the same passage (plan, Phase 6,
//! test 3′), where the decoder's own output cannot be compared because
//! Windows' downmix matrix is not known.
//!
//! The level is the RMS over 100 ms blocks that are louder than -70 dBFS, so
//! silence before the player starts and after it stops does not dilute it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
    IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator, WAVEFORMATEX,
    WAVEFORMATEXTENSIBLE, eConsole, eRender,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    CoUninitialize,
};
use windows::core::GUID;

const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
const WAVE_FORMAT_PCM: u16 = 1;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
const SUBTYPE_IEEE_FLOAT: GUID = GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71);
const SUBTYPE_PCM: GUID = GUID::from_u128(0x00000001_0000_0010_8000_00aa00389b71);
/// Mean square below this (-70 dBFS) is silence for the gate.
const GATE: f64 = 1e-7;

/// What the capture heard.
pub struct Report {
    pub rate: u32,
    pub channels: usize,
    /// 100 ms blocks above the gate, and all blocks captured.
    pub blocks_used: usize,
    pub blocks_total: usize,
    /// dBFS per channel, and all channels' power summed.
    pub per_channel_db: Vec<f64>,
    pub total_db: f64,
}

pub struct Capture {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<Result<Report, String>>,
}

fn db(mean_square: f64) -> f64 {
    10.0 * mean_square.max(1e-15).log10()
}

impl Capture {
    pub fn start() -> Capture {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::spawn(move || run(&flag));
        Capture { stop, thread }
    }

    pub fn finish(self) -> Result<Report, String> {
        self.stop.store(true, Ordering::SeqCst);
        self.thread
            .join()
            .map_err(|_| "the capture thread panicked".to_string())?
    }
}

#[derive(Clone, Copy)]
enum Sample {
    Float32,
    Int16,
}

fn run(stop: &AtomicBool) -> Result<Report, String> {
    let e = |what: &str, err: windows::core::Error| format!("{what}: {err}");
    unsafe {
        let com = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let result = capture(stop, &e);
        if com {
            CoUninitialize();
        }
        result
    }
}

unsafe fn capture(
    stop: &AtomicBool,
    e: &dyn Fn(&str, windows::core::Error) -> String,
) -> Result<Report, String> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|x| e("enumerator", x))?;
        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|x| e("default endpoint", x))?;
        let client: IAudioClient = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|x| e("activate", x))?;
        let format: *mut WAVEFORMATEX = client.GetMixFormat().map_err(|x| e("mix format", x))?;
        let base = format.read_unaligned();
        let sample = match base.wFormatTag {
            WAVE_FORMAT_IEEE_FLOAT if base.wBitsPerSample == 32 => Some(Sample::Float32),
            WAVE_FORMAT_PCM if base.wBitsPerSample == 16 => Some(Sample::Int16),
            WAVE_FORMAT_EXTENSIBLE => {
                let sub = (format as *const WAVEFORMATEXTENSIBLE)
                    .read_unaligned()
                    .SubFormat;
                if sub == SUBTYPE_IEEE_FLOAT && base.wBitsPerSample == 32 {
                    Some(Sample::Float32)
                } else if sub == SUBTYPE_PCM && base.wBitsPerSample == 16 {
                    Some(Sample::Int16)
                } else {
                    None
                }
            }
            _ => None,
        };
        // The format is packed: copy the fields before using them by reference.
        let (tag, bits) = (base.wFormatTag, base.wBitsPerSample);
        let channels = base.nChannels as usize;
        let rate = base.nSamplesPerSec;
        let Some(sample) = sample else {
            CoTaskMemFree(Some(format as *const _));
            return Err(format!(
                "the endpoint mixes in a format this probe cannot read (tag 0x{:X}, {} bits)",
                tag, bits
            ));
        };
        client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK,
                10_000_000,
                0,
                format,
                None,
            )
            .map_err(|x| e("initialize loopback", x))?;
        CoTaskMemFree(Some(format as *const _));
        let reader: IAudioCaptureClient =
            client.GetService().map_err(|x| e("capture client", x))?;
        client.Start().map_err(|x| e("start", x))?;

        let block = (rate / 10).max(1) as usize;
        let (mut in_block, mut block_sum) = (0usize, vec![0f64; channels]);
        let (mut used_sum, mut used_frames) = (vec![0f64; channels], 0usize);
        let (mut blocks_used, mut blocks_total) = (0usize, 0usize);
        loop {
            let mut packet = reader
                .GetNextPacketSize()
                .map_err(|x| e("packet size", x))?;
            while packet > 0 {
                let (mut data, mut frames, mut flags) = (std::ptr::null_mut::<u8>(), 0u32, 0u32);
                reader
                    .GetBuffer(&mut data, &mut frames, &mut flags, None, None)
                    .map_err(|x| e("get buffer", x))?;
                let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0;
                for f in 0..frames as usize {
                    for (c, sum) in block_sum.iter_mut().enumerate() {
                        let v = if silent {
                            0.0
                        } else {
                            match sample {
                                Sample::Float32 => {
                                    (data as *const f32).add(f * channels + c).read_unaligned()
                                        as f64
                                }
                                Sample::Int16 => {
                                    (data as *const i16).add(f * channels + c).read_unaligned()
                                        as f64
                                        / 32768.0
                                }
                            }
                        };
                        *sum += v * v;
                    }
                    in_block += 1;
                    if in_block == block {
                        blocks_total += 1;
                        let power: f64 = block_sum.iter().sum::<f64>() / block as f64;
                        if power > GATE {
                            blocks_used += 1;
                            used_frames += block;
                            for (used, now) in used_sum.iter_mut().zip(&block_sum) {
                                *used += *now;
                            }
                        }
                        in_block = 0;
                        block_sum.iter_mut().for_each(|s| *s = 0.0);
                    }
                }
                reader.ReleaseBuffer(frames).map_err(|x| e("release", x))?;
                packet = reader
                    .GetNextPacketSize()
                    .map_err(|x| e("packet size", x))?;
            }
            if stop.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = client.Stop();
        let per: Vec<f64> = used_sum
            .iter()
            .map(|s| db(s / used_frames.max(1) as f64))
            .collect();
        let total = db(used_sum.iter().sum::<f64>() / used_frames.max(1) as f64);
        Ok(Report {
            rate,
            channels,
            blocks_used,
            blocks_total,
            per_channel_db: per,
            total_db: total,
        })
    }
}
