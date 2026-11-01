# Plan: a Media Foundation decoder variant, for Windows Media Player

Status: **planned, nothing implemented.** Written to be executed phase by
phase by whoever picks it up (a person or an agent session) without any other
context: everything a phase needs is in this file, the repository, and the
pinned Omniphony checkout.

## How to execute this plan

- **Base branch: `dev/wmp`.** Work on `dev/wmp` or on a branch cut from it,
  one pull request per phase into `dev/wmp`. Never push to `main`.
- **Phases run in order.** Do not start a phase until the previous phase's
  "Done when" list holds. Phase 0 settles facts the later phases rely on. When
  a finding contradicts this plan, update this file in the same pull request
  and say what changed, rather than working around the plan in silence.
- **Two kinds of steps.** Steps marked **(Windows desktop)** need a real
  Windows 10/11 machine with a person at it, because they play files in
  Windows Media Player. Everything else runs in CI or on Linux. An agent
  without a Windows machine implements the code, pushes, and reads the Windows
  CI jobs (`.github/workflows/windows-arm64.yml`, plus the job Phase 1 adds).
  It then leaves the Windows-desktop checks as a checklist in the pull request
  description for the owner to tick.
- **Repository conventions** (from `ci.yml`, `windows-arm64.yml`, and the
  existing crates): edition 2024; every cargo command `--locked`; CI builds
  with `RUSTFLAGS="-D warnings"` (Windows adds `-C target-feature=+crt-static`);
  comments explain *why*, in the style of the existing crates;
  `scripts/check-crate-isolation.sh` asserts which crates each artifact may
  link; Omniphony is a sibling checkout at `../Omniphony`, at the ref
  `scripts/omniphony-ref.sh` prints:

  ```sh
  git clone https://github.com/mgth/Omniphony ../Omniphony
  git -C ../Omniphony checkout "$(scripts/omniphony-ref.sh)"
  ```

- **Commits:** the executing agent is the commit author. Add the repository
  owner as co-author with a `Co-Authored-By:` trailer, as the existing commits
  on `dev/wmp` do. Never write a session link or a chat transcript link into
  a commit, a pull request, an issue, or a file.

## Goal

Ship a **Media Foundation transform (MFT)**, an audio decoder COM object
Windows loads into any Media Foundation player. **Windows Media Player
(`wmplayer.exe`, "Windows Media Player Legacy" on Windows 11)** then plays the
lossless and object soundtracks of `.mkv` files on a PC's own speakers:

- Dolby TrueHD, with its Atmos objects;
- DTS, DTS-HD MA / HRA, DTS:X, and Auro-3D over DTS-HD;
- later, opt-in: E-AC-3 / AC-3 with JOC objects (see D6).

The decoding is this repository's: the existing Dolby and DTS bridge plugins,
unchanged. Rendering objects to speakers is Omniphony's: `orender_engine`, the
engine behind `liborender` that mpv-omniphony uses. The MFT only adapts
Media Foundation's sample flow to that engine. It is the Media Foundation
counterpart of mpv's `ad_orender`.

Whether Windows' built-in WinRT player can load the same DLL is an open
question, and Phase 0 answers it (step 4) before anything is built on it. That
player is `Windows.Media.Playback.MediaPlayer`, behind `MediaPlayerElement`.
It is the default engine of the FluentFin Jellyfin client (F13) and the engine
of the Media Player app. If it can, FluentFin becomes a target alongside WMP.

### Non-goals

- **The new "Media Player" app** (the Store app, `Microsoft.ZuneMusic`), and
  "Films & TV", are not promised.
  - A decoder installed by registration is not used by the Media Player app
    (F2).
  - An *in-process* Store media extension needs a restricted capability that
    third parties cannot obtain (F3).
  - One documented route is left, an *out-of-process* (app-service) media
    extension (F12). Phase 0 step 4 tests it, and only if it works do these
    apps come into scope.
- **DirectShow filters.** WMP can use DirectShow for other formats, but MKV
  playback in WMP goes through Media Foundation (verified in Phase 0).
- **Bitstream passthrough** to an AV receiver. Windows already passes through
  over S/PDIF and HDMI. This variant decodes on the PC.
- **IAMF.** No Windows media source exposes an IAMF track (F4). Reaching it
  needs a media source of our own (Phase 5, optional).
- **Elementary streams** (`.thd`, `.dts`, `.eac3` files). Windows has no media
  source for them (Phase 5, optional).
- **Any change** to `bridge_api`, to the three bridge plugins' behaviour, to
  the offline CLI, or to Omniphony. Phases 0–4 need none. Phase 5 lists the
  optional upstream additions that would help.

## What is already known

Facts the design rests on. Items marked *verify* are checked in Phase 0.

- **F1. WMP Legacy plays MKV through Media Foundation, which picks the decoder
  among the registered MFTs.** Windows' MKV media source maps Matroska codec
  ids to MF subtypes (Microsoft, "Matroska Media Container (MKV) support",
  <https://learn.microsoft.com/en-us/windows/win32/medfound/mkv-support>). Its
  table includes:

  | Matroska id | MF_MT_SUBTYPE the source gives |
  |---|---|
  | `A_TRUEHD` | `MEDIASUBTYPE_DOLBY_TRUEHD` |
  | `A_DTS` | `MEDIASUBTYPE_DTS_HD` (WAV id `WAVE_FORMAT_DTS`) |
  | `A_EAC3` | `MFAudioFormat_Dolby_DDPlus` |
  | `A_AC3` | `MFAudioFormat_Dolby_AC3` |

  Windows ships a decoder for AC-3 and E-AC-3 (the "Microsoft Dolby Digital
  Plus Decoder MFT"). As far as public reports go, it ships none for TrueHD or
  DTS. That is the gap this variant fills. *Verify*: that WMP takes the MKV path through MF on current Windows
  11, and which subtype GUIDs the source actually sets (the Phase 0 probe
  prints them).
- **F2. The Windows 11 Media Player app ignores registered MFTs.** A developer
  reports an audio MFT registered for Media Foundation. Their own Media
  Session apps loaded it, but the Media Player app did not load the DLL at all
  (Microsoft Q&A 2028133,
  <https://learn.microsoft.com/en-us/answers/questions/2028133/register-audio-codec-imftransform-to-be-seen-in-me>).
- **F3. In-process Store media extensions are closed to third parties.** A
  codec packaged for the Store declares `uap4:MediaCodec`. To load in the
  player's own process (`wincap3:ActivatableClassId` / `wincap3:Path`), it
  also needs the restricted `inProcessMediaExtension` capability. A package
  that declares it fails to install outside Microsoft's approval path, even
  sideloaded as administrator (Microsoft Q&A 5769519,
  <https://learn.microsoft.com/en-us/answers/questions/5769519/how-to-develop-custom-decoder-media-foundation-tra>).
  Microsoft's own FFmpeg-based extension shows that packaging, as a reference
  only: <https://github.com/pl4nty/microsoft-web-media-extensions>. The
  out-of-process variant is F12.
- **F4. No in-box source exposes IAMF.** The MKV table has no IAMF codec id
  (its `A_OPUS` row is plain Opus, not IAMF).
- **F5. Decoder preference cannot be forced machine-wide.**
  `IMFPluginControl::SetPreferredClsid` changes only the calling process's
  list
  (<https://learn.microsoft.com/en-us/windows/win32/api/mfobjects/nf-mfobjects-imfplugincontrol-setpreferredclsid>),
  so an installer cannot make WMP prefer our MFT over Microsoft's Dolby
  decoder. **Corrected by Phase 0:** Windows does register "IEC-61937
  converter" MFTs in the decoder category for TrueHD, DTS-HD and
  AC-3/E-AC-3, so TrueHD and DTS-HD do have in-box candidates. They wrap a
  stream for passthrough and cannot produce the float PCM a player asks
  for. Measured (docs/media-foundation-findings.md): the WinRT `MediaPlayer`
  chose our registered decoder over the TrueHD converter. Still to check:
  which one WMP takes, and for E-AC-3/AC-3 whether it prefers Microsoft's
  "Dolby Digital Plus Decoder MFT" (listed there as well).
- **F6. Media Foundation float PCM cannot carry objects.** A decoder's PCM
  output (`MFAudioFormat_Float` with `WAVEFORMATEXTENSIBLE` and a channel
  mask) has fixed speaker channels only. Objects must therefore be rendered
  inside the MFT, which is what `orender_engine` does. The spatial-objects
  output type (`MFAudioFormat_Float_SpatialObjects`, with
  `MF_MT_SPATIAL_AUDIO_OBJECT_METADATA_FORMAT_ID`) exists. Its metadata
  format, however, is one the downstream spatial renderer must recognise, and
  it is barely documented
  (<https://learn.microsoft.com/en-us/windows/win32/medfound/mf-mt-spatial-audio-object-metadata-format-id>).
  It is Phase 5 research, not the base design.
- **F7. `orender_engine` can be embedded as a Rust crate.** The `orender` CLI
  embeds it this way; `liborender` (`orender_ffi`) is a thin C shim over it.
  The entry points are in
  `../Omniphony/omniphony-renderer/orender_engine/src/engine.rs`:
  - `Engine::from_paths(config_yaml_path, speaker_layout_path, bridge_paths,
    input_codec, sample_rate)` builds the session;
  - `set_input_pts`, `process(data, RInputTransport::Raw, 0)`, `drain`,
    `reset`, `channel_layout`, `channel_count` and `output_latency_samples`
    run it;
  - `RenderedAudio` carries interleaved `f32` samples plus `input_pts_us`.

  `orender_ffi/src/lib.rs` (`build_engine`, `orender_process`,
  `orender_drain`, `orender_reset`) shows how to drive it from a host that
  feeds demuxed packets.
- **F8. The engine renders to whatever speaker layout file it is given.**
  The format is that of `../Omniphony/layouts/7.1.4.yaml`: speakers named
  `FL FR C LFE BL BR SL SR TFL TFR TBL TBR`, normalised Cartesian `x y z`,
  `spatialize: false` for LFE. Names map to `RChannelLabel` through
  `bridge_api::labels::label_for_name`: `BL` is `Lb` (back), `SL` is `Ls`
  (side). Content without a speaker the layout lacks folds into the speakers
  it has: heights into a 7.1 layout, everything into stereo.
- **F9. The shared Omniphony config is not safe for WMP.** Opening a config
  with `Config::load_or_default_with_live` *consumes* a fresh live-handoff
  sidecar of that config (`renderer/src/config.rs`, `live_sidecar_path`).
  WMP opening Omniphony's own config could therefore steal Studio's handoff
  state. The MFT uses a config file of its own (D5).
- **F10. The engine needs bridge files, found by absolute path.**
  `Engine::from_paths` loads the bridges it is given through `abi_stable`.
  `BridgeLibs` / `LoadedBridge::open` (`orender_engine/src/bridge_loader.rs`)
  accept in-process root modules, but `from_paths` does not take them. So
  linking the bridges into one file needs a new upstream constructor
  (Phase 5).

- **F11. The WinRT player has more codec sources than WMP, and which apply to
  a desktop process is undocumented.** `Windows.Media.Playback.MediaPlayer`
  can take a decoder from four sources:
  - the in-box MFTs;
  - decoders the app itself registers with
    `MediaExtensionManager.RegisterAudioDecoder(activatableClassId,
    inputSubtype, outputSubtype)`. These are local to that app and live as
    long as its `MediaExtensionManager`
    (<https://learn.microsoft.com/en-us/uwp/api/windows.media.mediaextensionmanager>);
  - Store media extensions, in-process (F3) or app-service (F12);
  - possibly the MFTs registered machine-wide with `MFTRegister`.

  No Microsoft document says whether the WinRT player, in a full-trust
  desktop process, takes the last of these. The Media Player app, a
  sandboxed Store app, does not (F2). `MFTEnumEx` has flags no page explains:
  `MFT_ENUM_FLAG_UNTRUSTED_STOREMFT` (0x400), and
  `..._SORTANDFILTER_APPROVED_ONLY` and `..._WEB_ONLY` (and `_EDGEMODE`)
  (<https://learn.microsoft.com/en-us/windows/win32/api/mfapi/ne-mfapi-_mft_enum_flag>).
  They suggest the WinRT stack enumerates a filtered set. *Verify* in
  Phase 0, step 4.
- **F12. A Store media extension can run out of process, in an app service.**
  `uap4:MediaCodec` has an optional `AppServiceName`, "the app service that is
  launched as the codec"
  (<https://learn.microsoft.com/en-us/uwp/schemas/appxpackage/uapmanifestschema/element-uap4-mediacodec>).
  The app service's background task hands its decoder to the media stack with
  `MediaExtensionManager.RegisterMediaExtensionForAppService(extension,
  connection)`
  (<https://learn.microsoft.com/en-us/uwp/api/windows.media.mediaextensionmanager.registermediaextensionforappservice>).
  `CodecQuery` reports such a codec with `CodecInfo.IsTrusted = false`,
  meaning it "runs in a separate app service" rather than in the app's
  process
  (<https://learn.microsoft.com/en-us/uwp/api/windows.media.core.codecinfo.istrusted>).
  This variant does not need `inProcessMediaExtension`. No public sample or
  third-party codec using it was found, so whether a sideloaded package is
  accepted, and by which players, is *unverified*.
- **F13. FluentFin uses Media Foundation in one of its three engines.**
  FluentFin (Jellyfin client, `insomniachi/FluentFin` at `d44c2e5`) is a
  WinUI 3 app packaged as MSIX with `runFullTrust` (`FluentFin/
  Package.appxmanifest`): a full-trust desktop process, not sandboxed like
  the Media Player app.
  - Its `MediaPlayerType` setting picks `WindowsMediaPlayer`, the default
    (`Windows.Media.Playback.MediaPlayer`), `Flyleaf` (FFmpeg) or `Vlc`
    (libVLC). Only the default can reach an MFT; the other two decode
    everything themselves.
  - Its Jellyfin device profile (`FluentFin/WmpDeviceProfileManager.cs`)
    always declares `truehd`, so the server sends TrueHD in MKV as is. It
    declares `dts`/`dca` only when `CodecQuery` finds an audio decoder for
    `CodecSubtypes.AudioFormatDts`, which is `MFAudioFormat_DTS`. Our codec
    table registers that subtype, so if `CodecQuery` lists our MFT, the
    server stops transcoding DTS for FluentFin.
  - Streams the server repackages as HLS carry no TrueHD (not in its HLS
    codec lists): only direct play of the MKV reaches the decoder.

## Design decisions

- **D1. A new crate in this workspace: `mf-decoder/`**, package
  `harletty-mf-decoder`, library `harletty_mf` (`crate-type = ["cdylib",
  "rlib"]`), license `GPL-3.0-or-later` like the plugins. Rust, with the
  `windows` crate 0.62. Start with the features `implement`,
  `Win32_Media_MediaFoundation`, `Win32_Media_Audio`,
  `Win32_Media_KernelStreaming`, `Win32_System_Com`,
  `Win32_System_LibraryLoader`, `Win32_System_Registry`,
  `Win32_UI_Shell_PropertiesSystem` and `Win32_Foundation`, and add whatever
  the compiler asks for (the `PKEY_*` constants live in feature-gated
  modules). 0.62 matches the `windows-core` 0.62 / `windows-implement` 0.60
  already in `Cargo.lock`.
  - Add it to `[workspace] members` only, **not** `default-members`, so a
    plain `cargo build` / `cargo test` at the root is unchanged.
  - Everything COM is `#[cfg(windows)]`. The portable core (D7) builds and is
    tested on every platform.
- **D2. Decode and render through `orender_engine`, linked by path** from
  `../Omniphony/omniphony-renderer/orender_engine`, as `bridge_api` already
  is. It is pinned by `.omniphony-ref`, so its Rust API moving is a
  deliberate pin bump. We neither reimplement the render nor go through
  `liborender`'s C ABI: one DLL less to find inside `wmplayer.exe`, and no
  second ABI handshake. Never call `Engine::enable_osc`. Never call
  `orender_engine::init_live_logging`, which installs the engine's OSC
  logger.
- **D3. The bridges stay separate files, loaded from the MFT's own folder.**
  The MFT finds its folder with
  `GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
  GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, <a fn of ours>)` and
  `GetModuleFileNameW`. It passes the absolute paths of
  `harletty_dolby_bridge.dll` and `harletty_dts_bridge.dll` there to
  `Engine::from_paths`. It does not rely on the DLL search order, which in
  WMP's process starts at WMP's folder. This needs no upstream change. The
  MFT crate links no decoder, which `check-crate-isolation.sh` enforces
  (Phase 1).
  The folder can be overridden by `bridge_dir` in `mf.yaml`, and for tests by
  the `HARLETTY_MF_BRIDGE_DIR` environment variable.
- **D4. Output: `MFAudioFormat_Float`, the input's sample rate, the PC's
  speaker layout.** The channel mask is read once per stream from the default
  render endpoint (`IMMDeviceEnumerator::GetDefaultAudioEndpoint(eRender,
  eConsole)`, property `PKEY_AudioEndpoint_PhysicalSpeakers`). It falls back
  to the mix format's mask, then to stereo `0x3`. A `speaker_mask` in the
  MFT's config overrides it.
  - The MFT **generates** the speaker layout YAML from the mask (D8), with
    speakers in ascending mask-bit order. The engine's output order is then
    already `WAVEFORMATEXTENSIBLE` order and needs no reordering.
  - Generating it, rather than shipping presets, keeps the labels and the
    mask identical. A preset's back-versus-side naming could otherwise
    disagree with a 5.1-side endpoint.
- **D5. A config of its own, in two places.** The defaults are
  `mf-config.yaml`, next to the DLL, written by the installer. A per-user
  override is `%LOCALAPPDATA%\harletty\mf\config.yaml`, if present, and wins.
  - The engine config is an Omniphony config file (same schema), with
    `render.osc: false` and no `bridge_path(s)`, since bridges come from D3.
  - The MFT's own options live in a sibling file, `mf.yaml`, so the engine
    schema stays untouched: `speaker_mask`, `codecs`, `log_level`,
    `log_file`. It is looked up the same way: next to the DLL, overridden by
    `%LOCALAPPDATA%\harletty\mf\mf.yaml`.
  - Binaural for headphones is the engine's own option
    (`render.binaural.output_mode: binaural`,
    `../Omniphony/omniphony-renderer/BINAURAL.md`) and works with a stereo
    mask.
- **D6. Codecs registered:** TrueHD and DTS by default. E-AC-3 and AC-3 only
  when the installer is asked (`-WithDolbyDigital`), and only once Phase 0 has
  shown what WMP does when Microsoft's decoder is also registered (F5).
  Without objects, Microsoft's decoder already plays them.
- **D7. Split: a portable core and a thin COM shell.**
  - `portable` holds no COM (not named `core`, which would shadow the `core` crate), and every platform builds and tests it. It covers
    the subtype→codec table, mask→layout generation, sample-time arithmetic,
    the output queue, the config, and the session around `Engine`.
  - `com` (Windows only) holds the `IClassFactory`, the `IMFTransform`, the
    DLL exports, and the registration.
  - A Linux agent can therefore test the whole decode path. Only the COM
    plumbing needs Windows CI.
- **D8. Architectures:** `x86_64-pc-windows-msvc` and
  `aarch64-pc-windows-msvc`. A DLL loads only into a process of its own
  architecture. Phase 0 records which `wmplayer.exe` (x64, ARM64, or 32-bit
  x86) the Start-menu shortcut starts on each. An `i686` build is added only
  if it turns out to be the one users get.
- **D9. Machine-wide registration, by an administrator.** `DllRegisterServer`
  / `DllUnregisterServer` write and remove the COM class and call
  `MFTRegister` / `MFTUnregister`, so `regsvr32` works. `install.ps1` wraps
  it. Tests do not register anything machine-wide: they use
  `MFTRegisterLocal`, which is in-process and needs no administrator (Phase 2).

Fixed identifiers, so every phase and every machine agree:

| What | Value |
|---|---|
| CLSID of the MFT | `{AC884ED4-9379-4E5A-BAF6-3365A4F08E38}` |
| Friendly name (MFTRegister) | `harletty decoder (TrueHD, DTS)` |
| DLL | `harletty_mf.dll` |
| Install folder | `%ProgramFiles%\harletty\mf\` |
| Per-user folder | `%LOCALAPPDATA%\harletty\mf\` |

## Architecture

```
wmplayer.exe
 └─ Media Session ─ MKV media source ── compressed IMFSample (one Matroska block)
                         │
                         ▼
               harletty_mf.dll  (IMFTransform, synchronous, 1 in / 1 out)
                 com/   IMFTransform ⇄ portable::Session
                 portable/ Session ── orender_engine::Engine
                                     ├─ harletty_dolby_bridge.dll  (TrueHD, E-AC-3)
                                     ├─ harletty_dts_bridge.dll    (DTS family)
                                     └─ VBAP / binaural render to the generated layout
                         │
                         ▼  float32 PCM IMFSample, WAVEFORMATEXTENSIBLE mask = endpoint's
 └─ Streaming Audio Renderer (SAR) ─ WASAPI shared mode ─ speakers
```

### The MFT contract (what `com/transform.rs` implements)

A **synchronous** MFT (no `MF_TRANSFORM_ASYNC`), `ThreadingModel=Both`, with
one input and one output stream, fixed. All state sits behind one `Mutex`.
Every method body runs in `std::panic::catch_unwind`: a caught panic resets
the session and returns `E_UNEXPECTED`, because nothing may unwind into
`wmplayer.exe`.

| Method | Behaviour |
|---|---|
| `GetStreamLimits`, `GetStreamCount` | 1 / 1 / 1 / 1 |
| `GetStreamIDs` | `E_NOTIMPL` (ids are 0) |
| `GetInputStreamInfo` | `hnsMaxLatency 0`, `dwFlags 0`, `cbSize 0`, `cbAlignment 0` |
| `GetOutputStreamInfo` | `dwFlags = MFT_OUTPUT_STREAM_PROVIDES_SAMPLES \| MFT_OUTPUT_STREAM_WHOLE_SAMPLES`: we allocate, since a packet's output size varies |
| `GetAttributes` | `E_NOTIMPL` |
| `GetInputAvailableType(i)` | a partial type (`MFMediaType_Audio` + the i-th subtype of the codec table), else `MF_E_NO_MORE_TYPES` |
| `SetInputType` | Accept `MFMediaType_Audio` with a subtype in the table. Read `MF_MT_AUDIO_SAMPLES_PER_SECOND` (default 48000). With `MFT_SET_TYPE_TEST_ONLY`, validate only. Otherwise build the `Session` (codec name from the table, the generated layout, `Engine::from_paths`). If that fails (bridges missing, layout invalid), return `MF_E_INVALIDMEDIATYPE` and log why, so the topology loader can try another decoder. `NULL` clears the input and output types. |
| `GetOutputAvailableType(0)` | Only once the input is set (else `MF_E_TRANSFORM_TYPE_NOT_SET`): float32, the input's rate, N channels, mask, built with `MFInitMediaTypeFromWaveFormatEx` from a `WAVEFORMATEXTENSIBLE` (`KSDATAFORMAT_SUBTYPE_IEEE_FLOAT`, 32 bits, `wValidBitsPerSample` 32). `MF_MT_ALL_SAMPLES_INDEPENDENT = TRUE`. Index > 0 gives `MF_E_NO_MORE_TYPES`. |
| `SetOutputType` | Accept only a type equal to the offered one, by subtype, rate, channels and mask |
| `GetInputStatus` | `MFT_INPUT_STATUS_ACCEPT_DATA` unless output is queued |
| `GetOutputStatus` | `MFT_OUTPUT_STATUS_SAMPLE_READY` when the queue is not empty |
| `ProcessInput` | `MF_E_NOTACCEPTING` while output is queued. Otherwise: contiguous buffer; sample time; on `MFSampleExtension_Discontinuity`, `Session::reset` first; then `Session::push`, which queues the rendered blocks. |
| `ProcessOutput` | `MF_E_TRANSFORM_NEED_MORE_INPUT` when the queue is empty. Otherwise return one block as a new `IMFSample` (`MFCreateSample` + `MFCreateMemoryBuffer`) with time and duration (below). Set `MFT_OUTPUT_DATA_BUFFER_INCOMPLETE` when more is queued. |
| `ProcessMessage` | `COMMAND_FLUSH`: `Session::reset`, clear the queue. `COMMAND_DRAIN`: `Session::drain` into the queue; once it is empty, `ProcessOutput` answers `NEED_MORE_INPUT` again. `NOTIFY_BEGIN_STREAMING`, `NOTIFY_START_OF_STREAM`, `NOTIFY_END_STREAMING` and `NOTIFY_END_OF_STREAM`: no-op, `S_OK`. `SET_D3D_MANAGER`: `E_NOTIMPL`. |
| `SetOutputBounds`, `ProcessEvent` | `E_NOTIMPL` |
| `DeleteInputStream`, `AddInputStreams` | `E_NOTIMPL` |

**Timestamps.** Input sample time is in 100 ns units: `pts_us = time / 10`.
`Session::push` calls `Engine::set_input_pts(Some(pts_us))` before
`Engine::process`.

- An output block whose `RenderedAudio::input_pts_us` is `Some(p)` starts at
  `p` minus `output_latency_samples / rate`.
- A block with `None` starts where the previous block ended.
- Duration is `n_frames * 10_000_000 / rate` (in 100 ns).
- After a reset, the first block re-anchors on its own `input_pts_us`.
- Input with no time leaves the running clock as it is.

**Format changes.** The engine always renders to the layout it was built
with, so the output type never changes mid-stream, whatever the content does
(5.1 bed, objects, a presentation switch).

- If a decoded stream's rate differs from the rate the session was built
  with, the engine's behaviour must be checked first: look at how
  `orender_engine` handles `RDecodedFrame::sampling_frequency` ≠ engine rate.
- If it does not resample, rebuild the session at the decoded rate. Then
  answer `ProcessOutput` once with `MF_E_TRANSFORM_STREAM_CHANGE` and
  `MFT_OUTPUT_DATA_BUFFER_FORMAT_CHANGE`, and offer the new rate.

### The codec table (`portable/codecs.rs`)

| Subtype (name in the Windows SDK) | GUID (*verify* against the SDK headers and the Phase 0 probe) | `input_codec` | Registered |
|---|---|---|---|
| `MEDIASUBTYPE_DOLBY_TRUEHD` | `{EB27CEC4-163E-4CA3-8B74-8E25F91B517E}` | `truehd` | default |
| `MEDIASUBTYPE_DTS_HD` | `{A2E58EB7-0FA9-48BB-A40C-FA0E156D0645}` | `dts` | default |
| `MFAudioFormat_DTS` (`WAVE_FORMAT_DTS` 0x0008) | `{00000008-0000-0010-8000-00AA00389B71}` | `dts` | default |
| `MFAudioFormat_Dolby_DDPlus` | `{A7FB87AF-2D02-42FB-A4D4-05CD93843BDD}` | `eac3` | opt-in (D6) |
| `MFAudioFormat_Dolby_AC3` | `{E06D802C-DB46-11CF-B4D1-00805F6CBBEA}` | `ac3` | opt-in (D6) |

- Prefer the `windows` crate's constants where it defines them, and keep a
  literal only for the ones it lacks, with a comment naming the SDK header.
- Add rows for whatever else Phase 0 finds: MP4 `mlpa`, `dtsc`/`dtsh`/`dtsl`
  and `dtsx`; the MPEG-2 TS source of `.m2ts`.
- The `input_codec` names must be ones the bridges list (`INPUT_CODECS` in
  `bridge-family-dolby/src/dolby.rs` and `bridge-family-dts/src/family.rs`).
  A unit test asserts it.
- `MFAudioFormat_Dolby_AC3_SPDIF` and other IEC 61937-framed subtypes are not
  registered: they are passthrough formats.

### The generated layout (`portable/layout.rs`)

One speaker per mask bit, in ascending bit order. The coordinates follow
Omniphony's convention (F8, copied from `layouts/7.1.4.yaml`), and the names
are ones `label_for_name` maps to the label in the third column:

| Bit | Mask | Name | Label | x, y, z |
|---|---|---|---|---|
| `SPEAKER_FRONT_LEFT` | 0x1 | FL | L | -1, 1, 0 |
| `SPEAKER_FRONT_RIGHT` | 0x2 | FR | R | 1, 1, 0 |
| `SPEAKER_FRONT_CENTER` | 0x4 | C | C | 0, 1, 0 |
| `SPEAKER_LOW_FREQUENCY` | 0x8 | LFE | LFE | 1, 1, -1, `spatialize: false` |
| `SPEAKER_BACK_LEFT` | 0x10 | BL | Lb | -1, -1, 0 |
| `SPEAKER_BACK_RIGHT` | 0x20 | BR | Rb | 1, -1, 0 |
| `SPEAKER_FRONT_LEFT_OF_CENTER` | 0x40 | *verify a name mapping to Lsc* | Lsc | -0.5, 1, 0 |
| `SPEAKER_FRONT_RIGHT_OF_CENTER` | 0x80 | *verify a name mapping to Rsc* | Rsc | 0.5, 1, 0 |
| `SPEAKER_BACK_CENTER` | 0x100 | *verify a name mapping to Cb* | Cb | 0, -1, 0 |
| `SPEAKER_SIDE_LEFT` | 0x200 | SL | Ls | -1, 0, 0 |
| `SPEAKER_SIDE_RIGHT` | 0x400 | SR | Rs | 1, 0, 0 |
| `SPEAKER_TOP_CENTER` | 0x800 | *verify a name mapping to Tc* | Tc | 0, 0, 1 |
| `SPEAKER_TOP_FRONT_LEFT` | 0x1000 | TFL | Tfl | -1, 1, 1 |
| `SPEAKER_TOP_FRONT_CENTER` | 0x2000 | *verify a name mapping to Tfc* | Tfc | 0, 1, 1 |
| `SPEAKER_TOP_FRONT_RIGHT` | 0x4000 | TFR | Tfr | 1, 1, 1 |
| `SPEAKER_TOP_BACK_LEFT` | 0x8000 | TBL | Tbl | -1, -1, 1 |
| `SPEAKER_TOP_BACK_CENTER` | 0x10000 | — | none: bit dropped from the mask | — |
| `SPEAKER_TOP_BACK_RIGHT` | 0x20000 | TBR | Tbr | 1, -1, 1 |

- **Fewer than three speakers** (stereo, mono, headphones): the engine's
  panner needs at least three speakers to start, so the session gives it a
  7.1.4 layout and switches on the binaural stage
  (`render.binaural.output_mode: binaural`, in a small config file written
  next to the layout). In that mode the engine bypasses the VBAP panner and
  renders every bed channel and object straight to two channels through an
  HRTF (Omniphony `BINAURAL.md`); the 7.1.4 layout only places the beds. Measured on the repository's fixtures: the stereo
  output carries signal and the channel order matches. Mono output is given
  stereo.
- Unit test: for every mask the table covers, the YAML parses as an
  Omniphony layout (use the `renderer` crate's layout loader).
- Unit test: `label_for_name` of each generated name gives the table's label,
  in order.
- The YAML is written to `%LOCALAPPDATA%\harletty\mf\layout-<mask>.yaml`,
  only when absent or different, because `from_paths` takes a path. A mask
  with no speaker left falls back to stereo.

## Files

```
mf-decoder/
  Cargo.toml                 # D1; orender_engine + bridge_api by path; windows (cfg(windows))
  src/lib.rs                 # mod core; #[cfg(windows)] mod com; DLL exports re-exported
  src/portable/mod.rs
  src/portable/codecs.rs         # codec table (subtype GUID as u128, input_codec, default/opt-in)
  src/portable/layout.rs         # mask → YAML layout, mask ↔ labels
  src/portable/clock.rs          # sample-time arithmetic (100 ns ↔ µs ↔ frames)
  src/portable/config.rs         # mf.yaml + config.yaml lookup (D5)
  src/portable/session.rs        # Engine wrapper: new/push/drain/reset, output queue
  src/portable/log.rs            # file logger (log facade), off unless enabled
  src/com/mod.rs
  src/com/exports.rs         # DllGetClassObject, DllCanUnloadNow (always S_FALSE), DllRegisterServer, DllUnregisterServer
  src/com/factory.rs         # IClassFactory
  src/com/transform.rs       # IMFTransform (contract above)
  src/com/registry.rs        # CLSID + MFTRegister/MFTUnregister (D9)
  src/com/endpoint.rs        # default endpoint speaker mask (D4)
  tests/core_session.rs      # portable: real bridges, fixtures, no COM
  tests/source_reader.rs     # cfg(windows): MFTRegisterLocal + IMFSourceReader end to end
  tests/fixtures/            # small .mkv files (Phase 2), marked binary in .gitattributes
  install/install.ps1        # copy, write mf-config.yaml / mf.yaml, regsvr32
  install/uninstall.ps1
  install/mf-config.yaml     # default engine config: render.osc false, no bridge paths
  install/mf.yaml            # default MFT options
tools/mf-probe/              # Phase 0: prints what MF sources expose (a bin crate, cfg(windows))
tools/mf-stub-decoder/       # Phase 0: stub MFT (silence) that logs which process loads it
tools/mf-player-probe/       # Phase 0: MFTEnumEx, CodecQuery and the WinRT MediaPlayer on a file
scripts/make-mf-fixtures.sh  # Phase 2: wraps the existing raw fixtures into .mkv with ffmpeg
docs/media-foundation.md     # user documentation (Phase 4)
docs/media-foundation-findings.md  # Phase 0 results
```

Changed:

- `Cargo.toml`: add `mf-decoder`, `tools/mf-probe`, `tools/mf-stub-decoder`
  and `tools/mf-player-probe` to `members` only.
- `scripts/check-crate-isolation.sh`: add a check
  `harletty-mf-decoder "Media Foundation decoder"` that it does not depend on
  `truehd eac3 dca auro iamf-dec iamf-obu iamf-codecs damf clap indicatif
  env_logger` (D3: the decoders come from the bridge DLLs at run time).
- `.github/workflows/windows-arm64.yml` and a new x64 job: Phases 1, 2 and 4.
- `.github/workflows/release.yml`: Phase 4.
- `README.md`: Phase 4, one short section linking `docs/media-foundation.md`.

## Phases

### Phase 0: facts and a go/no-go

**Status.** Steps 4.1 to 4.3 are done, and the results are in
`docs/media-foundation-findings.md`:

- A decoder registered machine-wide is listed by `MFTEnumEx` and the WinRT
  `CodecQuery` in a desktop process (F11, partly verified).
- On a desktop (Windows ARM64) the WinRT `MediaPlayer` instantiated the
  registered stub for a TrueHD MKV, ahead of Windows' converter.
- The stub and the probe were throwaway and are not part of `dev/wmp`. Their
  code, with the CI workflow that ran them, is saved on the branch
  `archive/mf-phase0-tools`; Phase 1 does not depend on it. Use it, or the
  descriptions below, only if a later question needs them.
- Playback with the stub stalled at about 0.8 s. The first suspect is the
  stub's duration fallback (100 ms of silence for every input sample that has
  no duration, which TrueHD blocks do not): the real transform places each
  block by its input timestamp and the decoded frame count instead (see
  "Timestamps"), and Phase 1's desktop check watches for the same stall.

**Still open (desktop):** WMP Legacy, FluentFin (direct play) and the Media
Player app (step 4.4), the app-service route if needed (4.5), `mf-probe`
(step 1), and the go/no-go (step 5).

Settles F1, F5, F11, F12 and D8 on real machines before any MFT code is
written.

1. **`tools/mf-probe`**: a small `cfg(windows)` binary using the `windows`
   crate. Given a file, it calls `MFStartup`, then
   `MFCreateSourceReaderFromURL`. For every stream, it prints:
   - the native media types (`GetNativeMediaType`, all indices) with every
     attribute: subtype GUID and its SDK name when known, rate, channels,
     `MF_MT_USER_DATA` as hex;
   - the first 5 compressed samples: time, duration, size, flags.

   With `--dump <stream> <file>` it writes the payloads of that stream
   back-to-back.
   - Build it in CI (the Windows ARM64 job) and upload it with the
     artifacts.
   - Done when it runs on the CI runner against a sample `.mkv` made by
     `scripts/make-mf-fixtures.sh` (Phase 2's script can be written now) and
     prints a subtype for each track.
2. **Payload check (Linux or Windows).** Feed a dumped TrueHD / DTS stream to
   `harletty info` (`cargo run -p harletty -- info <dump>`). It must decode,
   which shows a Matroska block payload is the raw transport the bridges take.
3. **(Windows desktop)** Answer these, and write them into
   `docs/media-foundation-findings.md`:
   1. Windows version and architecture.
   2. Which `wmplayer.exe` the Start menu starts: path, plus the "Platform"
      column of Task Manager's Details tab.
   3. Whether WMP Legacy is installed (on Windows 11 it is an optional
      feature).
   4. For an `.mkv` with each of TrueHD, DTS-HD MA and E-AC-3 JOC: does WMP
      open it, is there audio, and what error appears? Also, the `mf-probe`
      output of each file.
   5. The same for `.mp4` (TrueHD and DTS in MP4) and `.m2ts`, to decide
      which rows the codec table gets.
4. **Which players load a registered decoder** (F1, F11, F12). Two small
   tools, both built by the Windows CI jobs and uploaded with the artifacts so
   the desktop steps only download them.
   1. **`tools/mf-stub-decoder`** (library `harletty_mf_stub`, so a
      `.dll` named `harletty_mf_stub.dll`): a stub MFT, the Phase 1 skeleton with
      `ProcessOutput` returning silence of the input's duration.
      - It has its own CLSID, so it never collides with the real one:
        `{7CB8888B-36DA-4A32-B2F9-99748D739782}`.
      - It is registered with `regsvr32` for the codec table's default rows,
        plus E-AC-3 when `HARLETTY_STUB_EAC3=1` is set at registration.
      - Each of `DllGetClassObject`, `SetInputType` and the first
        `ProcessOutput` appends one line to `%PUBLIC%\harletty-mf-stub.log`:
        time, PID, executable path, whether the process is in an AppContainer
        (`GetTokenInformation(TokenIsAppContainer)`), and the subtype.
      - A failed write is ignored: a sandboxed process may not be allowed to
        write there, and that fact is itself a finding (use `tasklist /m`
        then).
   2. **`tools/mf-player-probe`**: an unpackaged desktop executable, full
      trust like FluentFin. Given a file, it prints:
      1. `MFTEnumEx(MFT_CATEGORY_AUDIO_DECODER, MFT_ENUM_FLAG_ALL |
         MFT_ENUM_FLAG_SORTANDFILTER, <subtype in>, NULL)`, then the same
         with `MFT_ENUM_FLAG_UNTRUSTED_STOREMFT`: each MFT's friendly name and
         CLSID. Does the stub appear?
      2. `CodecQuery.FindAllAsync(Audio, Decoder, <subtype>)`, the call
         FluentFin's device profile makes (F13): each codec's display name,
         subtypes and `IsTrusted`. Does the stub appear?
      3. `Windows.Media.Playback.MediaPlayer` with
         `MediaSource.CreateFromUri(file)`, `Play()`, waiting up to 10 s. It
         reports `MediaOpened` or `MediaFailed` (error, extended HRESULT),
         whether `PlaybackSession.Position` advances, and whether the stub
         logged a load from this PID.
   3. **CI (automatic, no person):** on the `windows-11-arm` runner, which
      is a Windows 11 client with Media Foundation and administrator rights,
      `regsvr32` the stub and run the probe on the Phase 2 fixtures.
      - Parts 1 and 2 answer F11 for desktop processes.
      - Hosted runners have no audio device, so part 3 may fail with
        `MF_E_NO_AUDIO_PLAYBACK_DEVICE`, possibly before any decoder is
        looked up. Record what it reports; the desktop run below decides.
      - Unregister at the end.
   4. **(Windows desktop)** With the stub registered, play a TrueHD MKV and
      a DTS MKV, then fill in this matrix in the findings file (✓/✗, plus
      the stub log line or the error):

      | Player | Process | Stub loaded? | Listed by `CodecQuery`? | Plays (silence, no error)? |
      |---|---|---|---|---|
      | WMP Legacy (`wmplayer.exe`) | desktop | | — | |
      | `mf-player-probe` (WinRT `MediaPlayer`) | desktop, unpackaged | | | |
      | FluentFin, engine "Windows Media Player", direct play from Jellyfin | desktop, MSIX full trust | | — | |
      | Media Player app | AppContainer | | — | |

      - For WMP and FluentFin, also check `tasklist /m harletty_mf_stub.dll`
        while playing. For WMP, also check Help → About → Technical Support
        Information under "Other binaries".
      - Then re-register with E-AC-3 and record, for WMP, whether it picks
        the stub or Microsoft's decoder (F5).
   5. **(Windows desktop, only if the WinRT rows are ✗)** The app-service
      route (F12), time-boxed to two days because no public sample exists:
      - A sideloaded MSIX (developer mode, self-signed certificate) that
        declares `uap4:MediaCodec` with `Category="audioDecoder"`,
        `AppServiceName`, and `uap4:MediaEncodingProperties` for TrueHD in /
        float out. It also declares the matching `windows.appService`
        extension, whose background task calls
        `RegisterMediaExtensionForAppService`.
      - Record whether it installs, whether `CodecQuery` lists it with
        `IsTrusted = false`, and which of the four players load it. Also
        record whether the audio stays in step: decoding happens in another
        process.
5. **Go/no-go**, recorded in the findings file:
   - **WMP: go** if WMP loads the stub for TrueHD or DTS from MKV. If it does
     not, stop. Record what was observed and which path WMP took
     (DirectShow, or another MF decoder). Revise this plan, for example to
     add a DirectShow filter, before Phase 1.
   - **FluentFin / WinRT player:**
     - If it loads the registered stub, add FluentFin to the Goal and to the
       Phase 2 and Phase 4 desktop checks.
     - If only the app-service package works, write a separate plan for
       packaging the decoder that way (D9 stays the WMP route). This one
       stays as it is.
     - If neither works, record it here under F11 and keep FluentFin's Flyleaf
       and VLC engines as the answer for that app (no objects).

Done when:

- `docs/media-foundation-findings.md` answers every question above,
  including the player matrix (the WinRT row is filled in);
- the codec table in this file is corrected to the observed GUIDs;
- F11 and F12 are marked verified or refuted;
- the WMP go/no-go is "go".

### Phase 1: crate skeleton, COM plumbing, registration

1. Create `mf-decoder/` per "Files", with `portable/codecs.rs`, `portable/layout.rs`,
   `portable/clock.rs` and `portable/config.rs` complete and unit-tested, and
   `session.rs` stubbed (silence).
2. `com/`: the class factory, the `IMFTransform` per the contract (with the
   stub session), the four DLL exports, and the registration.
   - `DllRegisterServer` writes `HKLM\SOFTWARE\Classes\CLSID\{CLSID}`
     (default value: the friendly name) and `...\InprocServer32` (default
     value: the DLL's absolute path, `ThreadingModel=Both`).
   - It then calls `MFTRegister(CLSID, MFT_CATEGORY_AUDIO_DECODER, name, 0,
     inputs, outputs, None)`. The inputs are the default rows of the codec
     table, plus the opt-in rows when `mf.yaml` next to the DLL has them
     enabled. The output is `MFAudioFormat_Float`.
   - `DllUnregisterServer` undoes both, and is idempotent.
   - Exports are `#[unsafe(no_mangle)] pub extern "system" fn`. A `.def`
     file is not needed.
3. Workspace and isolation edits (see "Files").
4. CI:
   - `windows-arm64.yml` builds `-p harletty-mf-decoder` and runs its tests.
   - A new job `windows-x64` in the same workflow, on `windows-latest`, does
     the same for `x86_64-pc-windows-msvc`.
   - Both upload `harletty_mf.dll` next to the bridges.
   - `ci.yml` (Linux) runs `cargo test --locked -p harletty-mf-decoder`,
     which covers the portable core.
5. A Windows CI test, `tests/registration.rs`, `cfg(windows)`, run only when
   `HARLETTY_MF_REGISTRY_TEST=1` is set, which CI sets because hosted runners
   are administrators. It calls `DllRegisterServer`, finds the CLSID through
   `MFTEnumEx(MFT_CATEGORY_AUDIO_DECODER, MFT_ENUM_FLAG_SYNCMFT, TrueHD in,
   Float out)`, calls `DllUnregisterServer`, and checks that it is gone.

Done when:

- Linux and both Windows jobs are green;
- `check-crate-isolation.sh` passes with the new check;
- the artifacts hold `harletty_mf.dll` for both architectures;
- **(Windows desktop)** `regsvr32` succeeds and WMP loads the DLL for a
  TrueHD MKV, playing silence.

### Phase 2: decoding and rendering

1. `portable/session.rs`:
   - `Session::new(codec, rate, mask, paths)` writes or reuses the layout
     YAML, resolves the two configs (D5), and calls `Engine::from_paths(Some
     (config), Some(layout), &[dolby_dll, dts_dll], Some(codec), rate)`.
   - It then checks that `engine.channel_layout()` equals the mask's labels
     in order. If not, it fails with a message naming both, rather than
     playing channels in the wrong places.
   - `push(&[u8], Option<pts_us>) -> ()` queues the `RenderedAudio` blocks as
     `(pts_100ns, frames, Vec<f32>)` per the timestamp rules.
   - `drain()` and `reset()` complete the API.
   - The engine's decode thread stays `off`: `DecodeThreadMode` default; see
     Risks before enabling it.
2. Logging (`com/logger.rs`): a `log::Log` writing to
   `%LOCALAPPDATA%\harletty\mf\harletty-mf.log` at `mf.yaml`'s `log_level`
   (default `warn`). It is installed with `log::set_logger` once, ignoring an
   error if the process already has one. The bridges' messages reach it
   through the host log sink the engine installs.
3. Wire the session into `com/transform.rs`: `SetInputType` builds it,
   `ProcessInput` pushes, and so on per the contract.
4. Fixtures: `scripts/make-mf-fixtures.sh` uses ffmpeg (`-c copy`, no
   re-encode) to wrap:
   - `harletty/tests/fixtures/joc_atmos_1s.eac3` into `eac3_joc.mkv`;
   - `harletty/tests/fixtures/dts_core_tone_10f.dts` into `dts_core.mkv`;
   - a few-kB TrueHD 5.1 tone (`ffmpeg -f lavfi -i sine=... -ac 6 -c:a
     truehd -strict -2`) into `truehd_51.mkv`.

   Commit the outputs (a few kB each) under `mf-decoder/tests/fixtures/`.
   Add them to `.gitattributes` as `binary`, and print their SHA-256 in the
   script's comment. Atmos TrueHD and DTS:X stay in the local corpus (env
   variables, as `bridge/tests/family_bridges.rs` does).
5. `tests/core_session.rs` (portable): decode each fixture's payloads
   through `Session` with the real bridge cdylibs.
   - The bridges are dev-dependencies, which makes cargo build their
     cdylibs. Locate them as Omniphony's tests locate `reference_bridge`
     (`orender_ffi/src/live_handle_tests.rs`, `find_reference_bridge`).
   - Assert the channel count; the total frames against the stream's
     duration (±1 frame block); non-silence; and monotonic, gap-free
     timestamps.
   - The MKV payloads come from a tiny test-only Matroska block reader, or
     from feeding the raw fixture files directly, which is the same byte
     stream. Prefer the raw files and keep the MKV path for the Windows test.
6. `tests/source_reader.rs` (`cfg(windows)`, CI): `MFStartup`, then
   `MFTRegisterLocal(our IClassFactory, MFT_CATEGORY_AUDIO_DECODER, …)`, then
   `MFCreateSourceReaderFromURL(fixture.mkv)` and `SetCurrentMediaType(first
   audio stream, partial Float type)`. Then `ReadSample` until end of stream.
   - Assert: the output type's mask and rate; sample count and timestamps as
     in 5; and that the samples equal a `Session` fed the same compressed
     samples (read with a second reader, kept at the native type), bit for
     bit.
   - A local MFT is enumerated before registered ones (`MFTEnumEx`,
     `MFT_ENUM_FLAG_LOCALMFT` / `SORTANDFILTER`; *verify* the Source Reader
     honours it, and if not, fall back to the registered round trip of
     Phase 1). This exercises MF's own decoder resolution, without
     administrator rights and without touching the machine.
7. Measure `Session::new` time (VBAP table build) on the CI runner and log it.
   If it exceeds 500 ms, look at the renderer's table options (the VBAP table
   mode hints in `bridge_api`, `renderer/src/spatial_vbap`) before Phase 3.
   An open topology blocks while it runs.

Done when:

- both Windows jobs and Linux pass `core_session` and `source_reader`;
- **(Windows desktop)** WMP plays `truehd_51.mkv`, `dts_core.mkv`, an Atmos
  TrueHD movie and a DTS:X movie, with sound from every speaker of the PC's
  layout and correct front/back/side placement (Windows' speaker test as
  reference);
- seeking works and keeps lip-sync; pause/resume and stop/play work;
- WMP's progress bar matches the audio.

### Phase 3: robustness and options

1. **Fuzz-ish hardening:** truncated and garbage input (bit-flipped fixture
   blocks) must never panic out or hang. A `cfg(windows)` test drives the
   transform directly with corrupted samples.
2. **Panic path test:** a test-only codec that panics (behind
   `#[cfg(test)]`) shows `E_UNEXPECTED` comes back, the next sample decodes,
   and the process lives.
3. **Device change:** the mask is read when the session is built. When the
   default endpoint changes mid-file, the next file uses the new one.
   Document it. Rebuilding mid-stream through `MF_E_TRANSFORM_STREAM_CHANGE`
   is optional, and only if Phase 0 showed WMP renegotiates cleanly.
4. **`mf.yaml` options:** `speaker_mask` (hex), `codecs` (`[truehd, dts,
   eac3, ac3]`), `log_level`, `log_file`. Plus the engine's own options
   through `config.yaml`: binaural, DRC (`render.drc` or the engine's
   equivalent), presentation. Test that each is honoured (portable tests).
5. **Memory and handles:** play and seek a 2-hour file in a loop for an hour
   (Windows desktop, with Task Manager or `perfmon`). Working set and handle
   count must stay flat.

Done when the tests pass and the soak test shows no growth.

### Phase 4: packaging, release and documentation

1. `install/install.ps1` (requires administrator, `-WithDolbyDigital`
   optional, `-Uninstall` alias):
   - copy `harletty_mf.dll`, `harletty_dolby_bridge.dll`,
     `harletty_dts_bridge.dll`, `mf-config.yaml`, `mf.yaml` and the licence
     texts to `%ProgramFiles%\harletty\mf\`;
   - enable the opt-in codecs in `mf.yaml` when asked;
   - `regsvr32 /s` the DLL; check `MFTEnumEx` finds it;
   - print what was done and how to undo it.

   `uninstall.ps1` reverses it, and leaves `%LOCALAPPDATA%\harletty\mf\`
   alone unless `-Purge` is given.
2. `release.yml`: on Windows x64 and ARM64, build the MFT with the bridges
   (static CRT, as the bridges are). Stage the licences: extend
   `scripts/stage-licences.sh` for the new binary, which also carries
   Omniphony's GPL-3.0 engine, `windows` (MIT/Apache-2.0) and Lua/`mlua`
   (MIT); check each actual dependency with `cargo tree`. Then zip
   `harletty-mf-<version>-windows-<arch>.zip` with the install scripts.
   - The archive holds its own copies of the two bridges: the MFT needs them
     in its folder (D3), and the bridge archive stays as it is.
   - The release notes name the Omniphony commit, as for the bridges.
3. `docs/media-foundation.md`: what it is for (WMP Legacy, MF desktop apps);
   what it is not for (the new Media Player app, F2/F3, said plainly); and
   install, uninstall, configure, speaker mask, binaural, logs, and
   troubleshooting (`tasklist /m harletty_mf.dll`, the log file). `README.md`
   gets a short section linking it.
4. **(Windows desktop)** Install from the release zip on a clean machine (or
   VM), play the Phase 2 set, uninstall, and check that WMP no longer loads
   the DLL and that the registry keys are gone.

Done when a release dry run (`release.yml`, manual dispatch) produces both
zips and the clean-machine check passes.

### Phase 5: optional follow-ups (each its own plan when started)

- **One-file DLL:** an upstream `Engine::from_libs(config, layout, BridgeLibs,
  codec, rate)` in Omniphony, factoring the part of `from_paths` after
  `load_bridges` (F10). The MFT can then link `harletty-dolby-bridge` /
  `harletty-dts-bridge` as rlibs and pass `library()`. Needs an Omniphony
  pull request and a pin bump, and changes the isolation check.
- **Spatial objects to Windows Sonic / Dolby Atmos for headphones:** output
  `MFAudioFormat_Float_SpatialObjects` (F6). Research first: which metadata
  format ids the in-box spatial renderers accept, and whether a third-party
  decoder can use one.
- **IAMF and elementary streams:** a byte-stream handler / media source of
  our own for `.mka`/`.mkv` with IAMF (see Omniphony
  `docs/iamf-matroska-mapping.md`) and for `.thd`/`.dts`/`.eac3`. This is
  large: it is a demuxer.
- **The new Media Player app:** only through a Store media extension. The
  in-process kind needs Microsoft to grant `inProcessMediaExtension` (F3),
  which is out of our control. The app-service kind (F12) is open to us if
  Phase 0 step 4.5 shows it works, and would get a plan of its own.

## Verification matrix

| What | Where | Phase |
|---|---|---|
| registered stub seen by `MFTEnumEx` / `CodecQuery` in a desktop process | Windows CI (admin) | 0 |
| which players load the stub: WMP, WinRT `MediaPlayer`, FluentFin, Media Player app | Windows desktop | 0 |
| codec table, layout generation, clock, config | Linux + Windows CI, unit tests | 1 |
| registration round trip through `MFTEnumEx` | Windows CI (admin) | 1 |
| decode + render of the fixtures through `Session` | Linux + Windows CI | 2 |
| MF's own decoder resolution, bit-exact vs `Session` | Windows CI (`MFTRegisterLocal`) | 2 |
| crate isolation (no decoder linked into the MFT) | Linux CI | 1 |
| corrupted input, panic containment | Windows CI | 3 |
| WMP plays TrueHD/Atmos, DTS/DTS:X MKV; seek, sync | Windows desktop | 1–2 |
| soak (memory, handles) | Windows desktop | 3 |
| install / uninstall from the release zip | Windows desktop (clean VM) | 4 |

## Risks and open questions

- **WMP may not take the MF path, or may not load our MFT** (F1 unverified).
  Phase 0's go/no-go exists for this.
- **The WinRT player may only take packaged codecs** (F11). Then FluentFin's
  "Windows Media Player" engine cannot use a registered DLL. The fallback is
  the app-service package (F12), which no public example proves works.
  Phase 0 step 4 settles it before Phase 1.
- **Microsoft's Dolby decoder may always win for E-AC-3/AC-3** (F5). That is
  acceptable: D6 makes those codecs opt-in, and objects in E-AC-3 are the
  only loss.
- **Engine start-up time** blocks topology resolution (Phase 2, step 7).
- **Global state in `orender_engine` inside a long-lived host process.**
  `overlay`, the live-log logger, OSC, `migrate_legacy_windows_config` and
  the live sidecar (F9). The MFT calls none of the OSC or logging entry
  points. It must check `Engine::from_paths` for any other process-wide side
  effect at the pinned ref, and note what it found in the Phase 2 pull
  request.
- **Decode thread.** The engine's `decode_thread` moves a packet's audio to a
  later call. The MFT's queue and the timestamp rules handle that (output is
  keyed on `input_pts_us`), but drain must then return the held packets.
  Start with it off. Turn it on only with a test of drain at end of stream
  and of flush.
- **Sample-rate mismatch** between the container and the stream (see Format
  changes).
- **Licensing.** The DLL is GPL-3.0-or-later and loads into a proprietary
  player at the user's choice, as the bridges load into hosts. The release
  ships the licence texts (Phase 4). Nothing here changes the decoders'
  licences (`LICENSES/`).
- **Omniphony API drift.** `orender_engine` is not a stable API. A pin bump
  that changes `Engine::from_paths` or `RenderedAudio` breaks only this
  crate, and CI shows it at the bump.

## References

- Omniphony, at the `.omniphony-ref` pin:
  - `omniphony-renderer/BRIDGE_API.md`, `ABI.md` and
    `orender_ffi/include/orender.h`;
  - `orender_ffi/src/lib.rs`, the host pattern to mirror;
  - `orender_engine/src/engine.rs` and `bridge_loader.rs`;
  - `renderer/src/config.rs`, `layouts/` and `BINAURAL.md`.
- This repository: `bridge-common/src/family.rs` and `plugin.rs`;
  `bridge-family-dolby/src/dolby.rs` and `bridge-family-dts/src/family.rs`
  (`INPUT_CODECS`); `scripts/check-crate-isolation.sh`;
  `.github/workflows/windows-arm64.yml` and `release.yml`.
- Microsoft: MKV support
  <https://learn.microsoft.com/en-us/windows/win32/medfound/mkv-support>;
  `IMFPluginControl::SetPreferredClsid`
  <https://learn.microsoft.com/en-us/windows/win32/api/mfobjects/nf-mfobjects-imfplugincontrol-setpreferredclsid>;
  `MF_MT_SPATIAL_AUDIO_OBJECT_METADATA_FORMAT_ID`
  <https://learn.microsoft.com/en-us/windows/win32/medfound/mf-mt-spatial-audio-object-metadata-format-id>;
  Q&A 2028133 (Media Player app ignores a registered MFT)
  <https://learn.microsoft.com/en-us/answers/questions/2028133/register-audio-codec-imftransform-to-be-seen-in-me>;
  Q&A 5769519 (`inProcessMediaExtension` is restricted)
  <https://learn.microsoft.com/en-us/answers/questions/5769519/how-to-develop-custom-decoder-media-foundation-tra>;
  `_MFT_ENUM_FLAG`
  <https://learn.microsoft.com/en-us/windows/win32/api/mfapi/ne-mfapi-_mft_enum_flag>;
  `MediaExtensionManager`
  <https://learn.microsoft.com/en-us/uwp/api/windows.media.mediaextensionmanager>
  and `RegisterMediaExtensionForAppService`
  <https://learn.microsoft.com/en-us/uwp/api/windows.media.mediaextensionmanager.registermediaextensionforappservice>;
  `uap4:MediaCodec`
  <https://learn.microsoft.com/en-us/uwp/schemas/appxpackage/uapmanifestschema/element-uap4-mediacodec>;
  `CodecInfo.IsTrusted`
  <https://learn.microsoft.com/en-us/uwp/api/windows.media.core.codecinfo.istrusted>.
- FluentFin at `d44c2e5`: `FluentFin/Package.appxmanifest`,
  `FluentFin/WmpDeviceProfileManager.cs`,
  `FluentFin/MediaPlayers/WindowsMediaPlayerController.cs`,
  `FluentFin.Core/Contracts/Services/IMediaPlayerController.cs`
  (<https://github.com/insomniachi/FluentFin>).
