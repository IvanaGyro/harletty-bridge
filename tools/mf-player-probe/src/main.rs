//! Phase 0 probe (docs/plan-media-foundation-decoder.md, "Phase 0", step 4.2):
//! which Windows media stacks see a registered decoder, and does the WinRT
//! `MediaPlayer` play a file through it?
//!
//! Usage: `mf-player-probe [FILE]`. Without a file, only the two listings run.
//!
//!   1. `MFTEnumEx` for the audio decoders of each interesting input subtype,
//!      sorted and filtered as the topology loader does, then with the
//!      untrusted-store flag.
//!   2. `CodecQuery.FindAllAsync(Audio, Decoder, subtype)`: the call the
//!      FluentFin Jellyfin client makes to decide what to ask the server for.
//!   3. With a file: `Windows.Media.Playback.MediaPlayer`, `Play()`, up to ten
//!      seconds, reporting `MediaOpened` / `MediaFailed` and whether the
//!      position advanced.
//!
//! Everything is printed; nothing here decides anything. The exit code is 0
//! unless the probe itself fails to run.

#[cfg(not(windows))]
fn main() {
    eprintln!("mf-player-probe runs on Windows only");
}

#[cfg(windows)]
fn main() {
    if let Err(e) = windows_main::run() {
        eprintln!("probe failed: {e}");
        std::process::exit(2);
    }
}

#[cfg(windows)]
mod windows_main {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use windows::Foundation::{TimeSpan, TypedEventHandler, Uri};
    use windows::Media::Core::{CodecCategory, CodecKind, CodecQuery, MediaSource};
    use windows::Media::Playback::{MediaPlayer, MediaPlayerFailedEventArgs};
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree};
    use windows::core::{GUID, HSTRING, IInspectable, Result};

    /// The subtypes the plan registers (TrueHD, DTS-HD, DTS, E-AC-3, AC-3).
    const SUBTYPES: [(&str, GUID); 5] = [
        (
            "TrueHD",
            GUID::from_u128(0xEB27CEC4_163E_4CA3_8B74_8E25F91B517E),
        ),
        (
            "DTS-HD",
            GUID::from_u128(0xA2E58EB7_0FA9_48BB_A40C_FA0E156D0645),
        ),
        (
            "DTS",
            GUID::from_u128(0x00000008_0000_0010_8000_00AA00389B71),
        ),
        (
            "E-AC-3",
            GUID::from_u128(0xA7FB87AF_2D02_42FB_A4D4_05CD93843BDD),
        ),
        (
            "AC-3",
            GUID::from_u128(0xE06D802C_DB46_11CF_B4D1_00805F6CBBEA),
        ),
    ];

    fn braced(guid: &GUID) -> String {
        let text = format!("{guid:?}");
        format!("{{{}}}", text.trim_matches(|c| c == '{' || c == '}'))
    }

    pub fn run() -> Result<()> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
            MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
        }
        println!("process: {:?}", std::env::current_exe().ok());

        println!("\n== 1. MFTEnumEx (audio decoders) ==");
        for (name, subtype) in SUBTYPES {
            list_mfts(
                name,
                &subtype,
                MFT_ENUM_FLAG_ALL.0 as u32 | MFT_ENUM_FLAG_SORTANDFILTER.0 as u32,
                "ALL|SORTANDFILTER",
            );
            list_mfts(
                name,
                &subtype,
                MFT_ENUM_FLAG_ALL.0 as u32 | MFT_ENUM_FLAG_UNTRUSTED_STOREMFT.0 as u32,
                "ALL|UNTRUSTED_STOREMFT",
            );
        }

        println!("\n== 2. CodecQuery.FindAllAsync (audio decoders) ==");
        let query = CodecQuery::new()?;
        for (name, subtype) in SUBTYPES {
            match query.FindAllAsync(
                CodecKind::Audio,
                CodecCategory::Decoder,
                &HSTRING::from(braced(&subtype)),
            ) {
                Ok(op) => match op.join() {
                    Ok(codecs) => {
                        println!("{name} {}: {} codec(s)", braced(&subtype), codecs.Size()?);
                        for codec in codecs {
                            let subtypes = codec
                                .Subtypes()?
                                .into_iter()
                                .map(|s| s.to_string())
                                .collect::<Vec<_>>()
                                .join(" ");
                            println!(
                                "  {} | trusted={} | {subtypes}",
                                codec.DisplayName()?,
                                codec.IsTrusted()?
                            );
                        }
                    }
                    Err(e) => println!("{name}: FindAllAsync failed: {e}"),
                },
                Err(e) => println!("{name}: FindAllAsync not started: {e}"),
            }
        }

        match std::env::args().nth(1) {
            Some(file) => {
                println!("\n== 3. MediaPlayer ({file}) ==");
                play(&file)?;
            }
            None => println!("\n(no file given: skipping the MediaPlayer step)"),
        }
        unsafe { MFShutdown()? };
        Ok(())
    }

    fn list_mfts(name: &str, subtype: &GUID, flags: u32, flags_name: &str) {
        let input = MFT_REGISTER_TYPE_INFO {
            guidMajorType: MFMediaType_Audio,
            guidSubtype: *subtype,
        };
        let mut activates: *mut Option<IMFActivate> = std::ptr::null_mut();
        let mut count = 0u32;
        let result = unsafe {
            MFTEnumEx(
                MFT_CATEGORY_AUDIO_DECODER,
                MFT_ENUM_FLAG(flags as _),
                Some(&input),
                None,
                &mut activates,
                &mut count,
            )
        };
        if let Err(e) = result {
            println!("{name} [{flags_name}]: MFTEnumEx failed: {e}");
            return;
        }
        println!("{name} [{flags_name}]: {count} MFT(s)");
        for i in 0..count as usize {
            let Some(activate) = (unsafe { (*activates.add(i)).take() }) else {
                continue;
            };
            let friendly = unsafe {
                let mut text = windows::core::PWSTR::null();
                let mut len = 0u32;
                match activate.GetAllocatedString(&MFT_FRIENDLY_NAME_Attribute, &mut text, &mut len)
                {
                    Ok(()) => {
                        let s = text.to_string().unwrap_or_default();
                        CoTaskMemFree(Some(text.0 as *const _));
                        s
                    }
                    Err(_) => "?".into(),
                }
            };
            let clsid = unsafe { activate.GetGUID(&MFT_TRANSFORM_CLSID_Attribute) }
                .map(|g| braced(&g))
                .unwrap_or_else(|_| "?".into());
            println!("  {friendly} {clsid}");
        }
        if !activates.is_null() {
            unsafe { CoTaskMemFree(Some(activates as *const _)) };
        }
    }

    fn play(file: &str) -> Result<()> {
        let path = std::path::absolute(file).unwrap_or_else(|_| file.into());
        let uri = Uri::CreateUri(&HSTRING::from(format!(
            "file:///{}",
            path.display().to_string().replace('\\', "/")
        )))?;
        let source = MediaSource::CreateFromUri(&uri)?;
        let player = MediaPlayer::new()?;
        let (tx, rx) = mpsc::channel::<String>();

        let opened = tx.clone();
        player.MediaOpened(&TypedEventHandler::new(
            move |_: windows::core::Ref<MediaPlayer>, _: windows::core::Ref<IInspectable>| {
                let _ = opened.send("MediaOpened".into());
                Ok(())
            },
        ))?;
        let failed = tx.clone();
        player.MediaFailed(&TypedEventHandler::new(
            move |_: windows::core::Ref<MediaPlayer>,
                  args: windows::core::Ref<MediaPlayerFailedEventArgs>| {
                let text = match args.as_ref() {
                    Some(a) => format!(
                        "MediaFailed error={} extended=0x{:08X} message={}",
                        a.Error().map(|e| e.0).unwrap_or(-1),
                        a.ExtendedErrorCode().map(|e| e.0).unwrap_or(0),
                        a.ErrorMessage().unwrap_or_default()
                    ),
                    None => "MediaFailed".into(),
                };
                let _ = failed.send(text);
                Ok(())
            },
        ))?;
        player.SetSource(&source)?;
        player.Play()?;

        let start = Instant::now();
        let mut last_position = TimeSpan { Duration: 0 };
        let mut advanced = false;
        while start.elapsed() < Duration::from_secs(10) {
            while let Ok(event) = rx.try_recv() {
                println!("  +{:.1}s {event}", start.elapsed().as_secs_f32());
            }
            if let Ok(position) = player.PlaybackSession().and_then(|s| s.Position()) {
                if position.Duration > last_position.Duration {
                    advanced = true;
                }
                last_position = position;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        println!(
            "  position after 10 s: {:.2}s, advanced={advanced}",
            last_position.Duration as f64 / 10_000_000.0
        );
        Ok(())
    }
}
