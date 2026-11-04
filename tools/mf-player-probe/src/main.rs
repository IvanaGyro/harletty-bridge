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
//! Soak (Phase 3, step 6): `mf-player-probe FILE --soak SECONDS` plays the file
//! for that long, seeking to a pseudo-random position every `--seek-every`
//! seconds (default 20) and printing the working set and handle count every
//! `--report-every` seconds (default 30). The file loops, so a short one does
//! for a long soak. At exit it lists the DLLs loaded in
//! the process and says whether any is a harletty bridge: with the decoders
//! linked into `harletty_mf.dll` there must be none. This replaces the plan's
//! `tasklist /m` check on a machine whose Windows Media Player is 32-bit.
//!
//! Listening: `mf-player-probe FILE --start SECONDS [--for SECONDS]` opens the
//! file, jumps to that time and plays from there: for `--for` seconds, or to
//! the end of the file when it is not given (Ctrl-C stops it earlier).
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
    use windows::Win32::System::ProcessStatus::{
        EnumProcessModules, GetModuleFileNameExW, GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};
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

        let args: Vec<String> = std::env::args().skip(1).collect();
        let number = |flag: &str, default: u64| -> u64 {
            args.iter()
                .position(|a| a == flag)
                .and_then(|i| args.get(i + 1))
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        let (start_at, play_for) = (
            args.iter()
                .any(|a| a == "--start")
                .then(|| number("--start", 0)),
            args.iter()
                .any(|a| a == "--for")
                .then(|| number("--for", 0)),
        );
        let (soak, seek_every, report_every) = (
            number("--soak", 0),
            number("--seek-every", 20).max(1),
            number("--report-every", 30).max(1),
        );
        match args.first().filter(|a| !a.starts_with("--")) {
            Some(file) => {
                println!("\n== 3. MediaPlayer ({file}) ==");
                play(file, soak, seek_every, report_every, start_at, play_for)?;
                list_modules();
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

    /// Working set (MiB) and open handles of this process.
    fn resources() -> (f64, u32) {
        unsafe {
            let process = GetCurrentProcess();
            let mut counters = PROCESS_MEMORY_COUNTERS {
                cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
                ..Default::default()
            };
            let _ = GetProcessMemoryInfo(process, &mut counters, counters.cb);
            let mut handles = 0u32;
            let _ = GetProcessHandleCount(process, &mut handles);
            (counters.WorkingSetSize as f64 / 1048576.0, handles)
        }
    }

    /// The DLLs loaded in this process, and whether a harletty bridge is one.
    fn list_modules() {
        let mut names = Vec::new();
        unsafe {
            let process = GetCurrentProcess();
            let mut modules = vec![Default::default(); 1024];
            let mut needed = 0u32;
            let size = (modules.len() * std::mem::size_of_val(&modules[0])) as u32;
            if EnumProcessModules(process, modules.as_mut_ptr(), size, &mut needed).is_ok() {
                modules.truncate(needed as usize / std::mem::size_of_val(&modules[0]));
                for module in modules {
                    let mut buffer = [0u16; 512];
                    let len = GetModuleFileNameExW(Some(process), Some(module), &mut buffer);
                    names.push(String::from_utf16_lossy(&buffer[..len as usize]));
                }
            }
        }
        println!("\n== 4. modules loaded in this process ==");
        let mut bridges = Vec::new();
        for name in &names {
            let lower = name.to_lowercase();
            if lower.contains("harletty") {
                println!("  {name}");
            }
            if lower.contains("_bridge") {
                bridges.push(name.clone());
            }
        }
        println!(
            "  ({} modules in all; only harletty ones listed)",
            names.len()
        );
        if bridges.is_empty() {
            println!("  bridge DLLs loaded: none");
        } else {
            println!("  bridge DLLs loaded: {}", bridges.join(", "));
        }
    }

    /// `file:///C:/dir/name` for a Windows path, with everything that is not
    /// unreserved (or `/` or the drive's `:`) percent-escaped, so a `#` or `%`
    /// or a space in a name stays part of the name.
    fn file_uri(path: &str) -> String {
        let mut uri = String::from("file:///");
        for (i, byte) in path.replace('\\', "/").bytes().enumerate() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                    uri.push(byte as char)
                }
                b':' if i == 1 => uri.push(':'),
                _ => uri.push_str(&format!("%{byte:02X}")),
            }
        }
        uri
    }

    fn play(
        file: &str,
        soak: u64,
        seek_every: u64,
        report_every: u64,
        start_at: Option<u64>,
        play_for: Option<u64>,
    ) -> Result<()> {
        let path = std::path::absolute(file).unwrap_or_else(|_| file.into());
        let uri = Uri::CreateUri(&HSTRING::from(file_uri(&path.display().to_string())))?;
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
        let ended = tx.clone();
        player.MediaEnded(&TypedEventHandler::new(
            move |_: windows::core::Ref<MediaPlayer>, _: windows::core::Ref<IInspectable>| {
                let _ = ended.send("MediaEnded".into());
                Ok(())
            },
        ))?;
        // A soak outlasts a short file: replay it instead of idling at the end.
        if soak > 0 {
            player.SetIsLoopingEnabled(true)?;
        }
        player.SetSource(&source)?;
        player.Play()?;

        let mut start = Instant::now();
        let mut last_position = TimeSpan { Duration: 0 };
        let mut advanced = false;
        // `--for` wins; a soak runs its time; `--start` plays to the end of the
        // file; a plain run looks for ten seconds.
        let run_for = Duration::from_secs(match (play_for, soak, start_at) {
            (Some(n), _, _) => n,
            (None, n, _) if n > 0 => n,
            (None, _, Some(_)) => u64::MAX / 4,
            _ => 10,
        });
        let start_s = start_at.unwrap_or(0);
        let mut seeked = start_s == 0;
        let mut ended = false;
        // A bounded listening (--start with --for) counts its seconds from the
        // moment the file is open and at the requested position, not from Play().
        let listening = start_at.is_some() && play_for.is_some();
        let mut opened = false;
        let open_wait = Duration::from_secs(120);
        let (mut next_seek, mut next_report, mut rng) = (seek_every, report_every, 0x5EEDu64);
        let session = player.PlaybackSession()?;
        while !ended
            && start.elapsed()
                < if listening && !opened {
                    open_wait
                } else {
                    run_for
                }
        {
            if soak > 0 {
                let elapsed = start.elapsed().as_secs();
                if elapsed >= next_seek {
                    next_seek += seek_every;
                    let length = session.NaturalDuration().map(|d| d.Duration).unwrap_or(0);
                    if length > 0 {
                        rng = rng
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add(1442695040888963407);
                        // The whole 64-bit value: a shifted one tops out near 215 s.
                        let to = (rng % length as u64) as i64;
                        let _ = session.SetPosition(TimeSpan { Duration: to });
                        println!("  +{elapsed}s seek to {:.0}s", to as f64 / 1e7);
                    }
                }
                if elapsed >= next_report {
                    next_report += report_every;
                    let (mib, handles) = resources();
                    println!("  +{elapsed}s working set {mib:.1} MiB, handles {handles}");
                }
            }
            while let Ok(event) = rx.try_recv() {
                println!("  +{:.1}s {event}", start.elapsed().as_secs_f32());
                // A failed open or decode never reaches the end: stop too.
                if (event == "MediaEnded" || event.starts_with("MediaFailed")) && soak == 0 {
                    ended = true;
                }
                if event == "MediaOpened" {
                    if !seeked {
                        seeked = true;
                        let to = start_s as i64 * 10_000_000;
                        match session.SetPosition(TimeSpan { Duration: to }) {
                            Ok(()) => println!("  jumped to {start_s} s"),
                            Err(e) => println!("  could not jump to {start_s} s: {e}"),
                        }
                    }
                    if listening && !opened {
                        start = Instant::now();
                    }
                    opened = true;
                }
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
            "  position after {:.0} s: {:.2}s, advanced={advanced}",
            start.elapsed().as_secs_f32(),
            last_position.Duration as f64 / 10_000_000.0
        );
        if soak > 0 {
            let (mib, handles) = resources();
            println!("  end: working set {mib:.1} MiB, handles {handles}");
        }
        Ok(())
    }
}
