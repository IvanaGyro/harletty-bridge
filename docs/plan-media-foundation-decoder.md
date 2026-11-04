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
- **Release scope (a rule, for every phase).** This fork's final release
  contains everything the upstream repository (`harletty/harletty-bridge`)
  releases today, plus the MFT archives:
  - the bridge archives (`harletty-bridge-<version>-<os>-<arch>.zip`, the
    Omniphony plugins) and the CLI archives
    (`harletty-cli-<version>-<os>-<arch>.zip`), each **as upstream produces
    it**;
  - the MFT archives (`harletty-mf-<version>-windows-<arch>.zip`), a separate
    set (D10).

  Nothing in this plan may change what the bridge and CLI jobs of
  `release.yml` build or package, or what their binaries output.
  - **The MFT build compiles only what it uses.** What goes into which build
    is split with **Cargo features**, at compile time, never with a run-time
    configuration key.
    - The upstream builds keep their current feature sets. Each feature that
      a shared crate (`bridge-common`, the `bridge-family-*` crates, `dca`,
      `harletty`) gains is in its `default` set.
    - So `plugin-*`, `bridge` and `harletty` compile exactly the code they
      compile today.
    - The MFT depends on those crates with `default-features = false` and
      names only the features it needs. Nothing ships dormant in it. The
      first such features are the IEC 61937 paths (Phase 5).
      - D11's "Build contents" lists the wider split that removing the
        engine would have needed.
  - **Features add code and never change behaviour.** No feature makes a
    crate behave differently from what it does without it; it only adds a
    path. The reason is Cargo's feature unification:
    - the features of one package are the union over everything built in the
      same invocation;
    - so a feature that *changed* behaviour, a "bed only" switch for
      example, would leak from the MFT into a plugin built alongside it.
    - With additive features, unification can only add code to the MFT, in
      builds that do not ship.
    - `release.yml` builds the MFT in its own `cargo build -p
      harletty-mf-decoder`, apart from the bridges and the CLI, and must keep
      doing so.
    - A capability only the MFT needs is therefore what the crate does when
      the MFT leaves the feature for the extra path off. D11's DTS lossless
      bed, with no DTS:X unfolding, was an example.
  - **The check:** the pull request that adds such a feature shows the bridge
    and CLI output unchanged:
    - the existing tests pass unmodified, with no expected value, hash or
      golden file edited: `bridge/tests/family_bridges.rs`,
      `harletty/tests/golden.rs` and the family crates' own unit tests;
    - a `tools/host-baseline` run over the local corpus, bridge DLLs built
      before against after the change, is bit for bit equal. Its result goes
      in the pull request description;
    - `cargo tree -e features` for the bridge and CLI packages, before and
      after, lists the same crates. Every new feature is shown enabled,
      through `default`;
    - `scripts/check-crate-isolation.sh` asserts what the MFT leaves out.

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

That is the design Phases 1–4 built, and it stays.
- D11 evaluated an MFT without `orender_engine` that plays each stream's own
  channel presentation. Phase 6's gate was run once, on headphones
  (`docs/media-foundation-findings.md`).
- The owner then decided to **keep the engine** (D2), so D11 is rejected and
  kept only as the record of what was evaluated.
- One result of that evaluation stays: stereo endpoints get the 7.1 output,
  not forced binaural (Phase 6 step 0).

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
  - The one exception is Phase 5's Cargo features for the IEC 61937 paths of
    `bridge-common` and the two families. They are in each crate's `default`
    set, so the plugins and the CLI compile and behave as they do, under the
    release-scope rule above.

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
  mask) has fixed speaker channels only.
  - The objects must therefore become speaker channels inside the MFT. Today
    `orender_engine` renders them.
  - The other way is to play a channel presentation the stream already
    carries (F14, D11).
  - The spatial-objects output type (`MFAudioFormat_Float_SpatialObjects`,
    with `MF_MT_SPATIAL_AUDIO_OBJECT_METADATA_FORMAT_ID`) exists. Its metadata
    format, however, is one the downstream spatial renderer must recognise,
    and it is barely documented
    (<https://learn.microsoft.com/en-us/windows/win32/medfound/mf-mt-spatial-audio-object-metadata-format-id>).
    It is Phase 5 research, not the base design.
  - **What the documentation says, checked against D11.**
    - The metadata GUID is "decoder-defined" and the blob is "opaque to the
      Media Foundation pipeline".
    - A metadata stream's format id is "the identifier of the metadata
      format for the currently active spatial rendering engine"
      (<https://learn.microsoft.com/en-us/windows/win32/api/spatialaudiometadata/ns-spatialaudiometadata-spatialaudioobjectrenderstreamformetadataactivationparams>).
    - No public format id or command set is documented.
    - Microsoft's own Dolby decoder documents float, PCM and IEC 61937
      outputs only
      (<https://learn.microsoft.com/en-us/windows/win32/medfound/dolby-audio-decoder>).
    - So no documented route exists for a third-party MFT to hand objects to
      Windows. Removing the engine cannot rest on one.
  - **The decoder side, as far as the SDK shows it** (verified in the SDK
    bindings and the API reference):
    - The SDK defines two decoder attributes: `MFT_AUDIO_DECODER_AUDIO_ENDPOINT_ID`
      and `MFT_AUDIO_DECODER_SPATIAL_METADATA_CLIENT`. The latter holds an
      `ISpatialAudioMetadataClient`.
    - That client is activated on the endpoint for one format, and "will not
      successfully activate" when the current render endpoint does not
      support that format
      (<https://learn.microsoft.com/en-us/windows/win32/api/spatialaudiometadata/nn-spatialaudiometadata-ispatialaudiometadataclient>).
    - Metadata command IDs and value sizes are "defined by the metadata
      format". Microsoft defines one standard command, `SPATIAL_AUDIO_POSITION`
      (ID 200, three floats, metres)
      (<https://learn.microsoft.com/en-us/windows/win32/coreaudio/spatial-audio-xxx-constants>).
    - The SDK has no public metadata-format GUID, and no factory for
      `IMFSpatialAudioSample` (a decoder would implement it itself).
    - **Third-party code, not verified:** a public research project
      (`barralutz/spatial-audio-lab`) drives DTS's own DTS:X decoder MFT (from
      DTS Sound Unbound) with this output type. The decoder uses a DTS-private
      format, and the project plays the objects through its own
      `ISpatialAudioClient`. Vendors use the mechanism with their own formats
      for their own renderers.
  - **The documented object route belongs to whoever owns the output**
    (Spatial Sound, "Render spatial sound using spatial audio objects",
    `ISpatialAudioObject::SetPosition`; verified):
    - An app activates `ISpatialAudioClient` on the endpoint. It renders a
      static bed of up to 8.1.4.4 and dynamic objects, positioned with
      `SetPosition(x, y, z)`: metres from the listener, x right, y up, z
      behind.
    - No metadata format is needed on that path. The spatial format chosen
      by the user (Windows Sonic for Headphones, Dolby Atmos, DTS
      Headphone:X) "is abstracted from the above implementation".
    - On current Windows, 128 dynamic objects are available for Windows Sonic
      for Headphones and Dolby Atmos (Headphones).
    - `GetMaxDynamicObjectCount` returns 0 when spatial sound is off or
      unavailable.
    - Audio from non-spatial clients "will also be mixed" with it, not
      spatialised.
    - Media Foundation gets one sentence: "Microsoft Spatial Sound support is
      also integrated into Media Foundation; apps that use media foundation
      can successfully play Dolby Atmos content with no additional
      implementation." No decoder output type, renderer path or format is
      named.
    - *Inferred:* a decoder inside WMP or the WinRT `MediaPlayer` does not own
      the output (the SAR does). So it cannot use this route.
    - Opening its own `ISpatialAudioClient` stream would bypass the player's
      clock, pause, rate and volume, which reach the SAR and not the
      decoder. That is not viable. The route fits a player that renders its
      own audio (Phase 5).
  - **A height bed has no documented PCM route either.** A 7.1.4 mask can be
    *described* with the `SPEAKER_TOP_*` bits (`portable/layout.rs` does), but:
    - nothing documented says the SAR, which takes "PCM or IEEE
      floating-point"
      (<https://learn.microsoft.com/en-us/windows/win32/medfound/streaming-audio-renderer>),
      plays such a type as heights;
    - Windows' spatial renderers take a channel bed up to 8.1.4.4 only as
      static objects of `ISpatialAudioClient`
      (<https://learn.microsoft.com/en-us/windows/win32/coreaudio/spatial-sound>),
      and a decoder MFT owns none.
    - *Verify* in Phase 6, gate measurement 2.
    - **Measured (owner, Windows ARM64, headphones):** the WinRT
      `MediaPlayer` accepts and plays an 8-channel (`0x63F`) and a 12-channel
      (`0x2D63F`) float type on a stereo endpoint, and Windows downmixes them.
      Whether `SPEAKER_TOP_*` channels come out of height speakers is still
      not measured: there were no speakers.
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
- **F10. `from_paths` needs bridge files, but the engine does not.**
  `Engine::from_paths` loads the bridges it is given through `abi_stable`,
  from files found by absolute path. Below it, at the pinned ref, everything
  is public: `BridgeLibs::new(Vec<BridgeLibRef>)` and `LoadedBridge::open`
  (`orender_engine/src/bridge_loader.rs`) take in-process root modules,
  `Engine::new(LoadedBridge, SpatialRenderer, rate)` takes the result, and
  the steps `from_paths` runs between the two (`configure_presentation`,
  `renderer_build::build_spatial_renderer`,
  `seed_runtime_state_from_render_config`, `prepare_speaker_stage`, the
  evaluation-grid settle) are `pub` too. Each family's root module comes
  from `bridge_common::plugin::root_module::<P>()` for a `Plugin` impl. So
  the decoders can be linked into the MFT with **no upstream change** (D10).
  - The plugin crates (`harletty-dolby-bridge`, `harletty-dts-bridge`) each
    carry an `#[export_root_module]`, whose loader symbol has a fixed name
    (*verify*). Linking both into one cdylib would then clash, so the MFT
    links the family crates (`bridge-family-dolby`, `bridge-family-dts`) and
    `bridge-common` instead, with `Plugin` impls of its own, and exports no
    root module.

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
- **F14. What the decoders emit, without the engine.** Measured on Linux by
  driving `PluginBridge` directly, at the `.omniphony-ref` pin.
  - **TrueHD Atmos** (FFmpeg FATE `truehd/atmos.thd`):
    - presentations 0, 1 and 2 are 2, 6 and 8 fixed channels (`L R`, then
      `L R C LFE Ls Rs`, then that plus `Lb Rb`);
    - presentation 3, the `best` default, is `LFE` plus 11 `Object`
      channels, which hold 99.7 % of the energy.
  - **The other Atmos samples:**
    - `fba_atmos_obj.mlp` (in the `truehd` fork's test assets) is `LFE` plus
      15 objects;
    - `fba_atmos_cbi.mlp` is a channel-based 9.1.6 with no objects (`… Tfl
      Tfr Tsl Tsr Tbl Tbr Lw Rw`).
  - **TrueHD 5.1** (`ffvii`): presentations 2 and 3 fall back to the 6-channel
    one (`truehd` `PresentationMap::effective_presentations`).
  - **E-AC-3 JOC** (`joc_atmos_1s.eac3`): `LFE` plus 15 objects, with 100 % of
    the energy in the objects.
  - **DTS:X** (read in `bridge-family-dts/src/dts_pipeline.rs`, no sample at
    hand):
    - each height feed and object is subtracted from the 7.1 bed at its
      stated fold, then emitted on its own channel;
    - Auro-3D carriers are always unfolded to height labels;
    - the family has no switch to return the bed with them still in.
  - **So dropping objects is not an option.** On Atmos it leaves the LFE. On
    DTS:X it leaves a bed with its heights and objects taken out.
  - **TrueHD's `presentation` key** (already a Dolby-family key) selects the
    encoder's own lossless 2.0, 5.1 or 7.1 rendering of the mix.
- **F15. What the engine adds for the MFT, and what it costs.** Measured on
  Linux x86-64 at the pin. Windows is not measured.
  - **On a bed whose labels the layout has, it is an identity.** TrueHD 5.1
    rendered to 7.1 is bit-identical to the bridge's output after a fade-in
    of the first 4800 frames.
  - **What it really does:**
    - object panning (VBAP);
    - virtual placement of labels the layout lacks;
    - the HRTF binaural stage (`renderer/src/binaural`: SAF KEMAR HRIRs, ITD,
      reflections, reverb). The MFT forces it for every output of two
      speakers or fewer (`portable/layout.rs`, `plan`).
  - **What it does not do by default:** DRC (the engine applies the bridge's
    `drc_gain` ramp) and dialnorm are off by default (`drc_mode: Off`,
    `use_loudness: false`).
  - **Start-up:** `Session::open` takes 15–36 ms at stereo, 5.1, 7.1 and
    7.1.4.
  - **CPU:** rendering adds about 0.8 % of a core for 7.1 and about 1.5 % for
    binaural.
  - **Size:** a cdylib of the two families is 3.2 MB (thin LTO, stripped);
    with the engine it is 10.0 MB, and 61 more crates (`mlua` with Lua,
    `rosc`, `socket2`, `rayon`, `realfft`, `regex`, …).
  - **Threads:** a process-wide rayon pool of N−1 threads
    (`renderer/src/background_pool.rs`) and a layout-follower thread per
    renderer, both inside the player.
  - **Quality** on `atmos.thd`, rendered to 7.1:
    - the engine and a 225-line dual-balance Cartesian panner gave nearly
      the same output;
    - each was as close to Dolby's own 7.1 presentation as the other.
    - The clip lasts 0.11 s, so this suggests and does not show.
- **F16. Windows' headphone virtualisation of a channel bed is reported, not
  documented.**
  - Third-party guides and bug reports (for example
    <https://github.com/music-assistant/desktop-app/issues/146>) say choosing
    Windows Sonic for Headphones turns on "7.1 virtual surround". Shared-mode
    apps are then given a 7.1 mix format, which Windows virtualises.
  - Microsoft's Spatial Sound page documents only the `ISpatialAudioClient`
    route (F6).
  - If the report holds, the MFT, which reads the mix format
    (`com/endpoint.rs`), already gets 7.1 there. The engine's binaural stage
    then never runs.
  - *Verify*: Phase 6, gate measurement 1.
  - **Not confirmed on the one machine measured** (owner, Windows ARM64,
    two-channel headphones, WinRT `MediaPlayer`; details in
    `docs/media-foundation-findings.md`):
    - the mix format stayed stereo at every Spatial audio setting ("Off",
      "Windows Sonic for Headphones", "Dolby Atmos for Headphones");
    - with the output pinned to 8 or 12 channels, the three settings sounded
      alike, and front, side and back could not be told apart.
    - *Inferred:* Windows only downmixes such a stream to stereo here. Do not
      rely on Windows to virtualise the MFT's output.
  - **The owner's hypothesis:** the settings made no difference because they
    act only when spatial information (objects) is handed to the system.
    - This agrees with the documentation (F6): the spatial formats render
      what comes through `ISpatialAudioClient`, and other clients' audio is
      mixed in.
    - The "7.1 virtual surround" reports concern apps given a 7.1 mix format,
      which this endpoint never offered.
    - It is consistent with the measurements, but not tested on its own:
      testing it needs a stream that does hand objects to the system, which
      is the check Phase 5 describes.

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
  - **Reviewed, and kept.** On beds the engine is an identity (F15).
    Elsewhere its work is object panning and binaural.
    - D11 proposed playing the streams' own presentations instead. Phase 6's
      first gate run found the binaural stage no reason to keep the engine:
      for the owner it lost to a plain downmix.
    - The deciding blind preference test (3′, presentation 2 against the
      engine's presentation 3) was not run.
    - The owner judged that two-channel Pixel Buds A-Series, through
      Windows' stereo downmix, cannot show which is better, even at matched
      levels, and decided to keep the engine.
    - D2 stands as written. D11 is rejected.
  - With spatial sound off, Windows gives no object route at all (F6), so
    any object output Phase 5 may add would sit beside the engine, not
    replace it.
- **D3. Until Phase 3, the decoders are the bridge DLLs, loaded from the
  MFT's own folder.** This is a stage on the way to D10, not what ships:
  Phases 1 and 2 are built and tested this way, and Phase 3 step 1 links the
  decoders in. The MFT finds its folder with
  `GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
  GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, <a fn of ours>)` and
  `GetModuleFileNameW`. It passes the absolute paths of
  `harletty_dolby_bridge.dll` and `harletty_dts_bridge.dll` there to
  `Engine::from_paths`. It does not rely on the DLL search order, which in
  WMP's process starts at WMP's folder. This needs no upstream change. Until
  Phase 3 the MFT crate links no decoder, which `check-crate-isolation.sh`
  enforces (Phase 1).
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
  - **An endpoint of one or two speakers still gets the full stream, as a
    full layout: 7.1 (`0x63F`) by default, 7.1.4 (`0x2D63F`) on request**
    (Phase 6 step 0). The decoder never reduces the stream to stereo itself;
    the engine renders the whole stream, bed and objects, to that layout, and
    Windows downmixes it for the endpoint.
    - **The owner's decision:** a stereo or mono endpoint should still receive
      the full stream: TrueHD + Atmos as 7.1 + objects, E-AC-3 JOC as 5.1 +
      objects, with an option for 7.1.4 when Atmos is available.
    - **What Media Foundation can carry** decides the form:
      - A float PCM output is fixed speaker channels only, and no documented
        route takes objects from a decoder (F6). Objects as objects stay
        Phase 5 research.
      - "7.1 + objects" would also play every object twice: the 7.1 (and
        5.1) presentation is the encoder's render of the whole mix, objects
        included (F14). The stream's full content is its bed plus its
        objects, which the engine renders.
      - So the full stream reaches the system as the engine's render to the
        largest layout chosen. That is the owner's "full stream" in the one
        form the decoder can deliver.
    - **The layout is the same for every codec.** E-AC-3 JOC's 5.1 core is
      not its limit: its objects are rendered to 7.1 or 7.1.4 like TrueHD's.
      A 5.1 bed lands on the 7.1 layout unchanged (F15).
    - **7.1.4 is an option, `stereo_endpoint_mask` in `mf.yaml`,** not chosen
      per stream:
      - The output type is fixed when the input type is set, before a packet
        is decoded. Whether a stream carries objects is only known after
        that, and changing the type mid-stream is untested (Phase 3 step 4).
      - Applying the option to every stream should cost nothing on streams
        without objects. Their channels should reach the 7.1.4 layout
        unchanged, with the four top channels silent. That is expected from
        F15, which showed the identity for a 7.1 layout; step 0 tests it for
        7.1.4.
      - In the gate run, presentation 2 at 7.1 and at 7.1.4 could not be told
        apart (2 of 6, blind).
      - On Atmos the two layouts do sound different after Windows' downmix
        (6 of 6, blind). Which is better is not measured, hence an option
        and 7.1 as the default.
      - Any mask of three or more speakers the layout table covers is
        accepted there. Others are refused with a log line, and 7.1 is used.
    - The 7.1 output is the one the owner heard as "much better" than the
      forced binaural one. It was measured with the WinRT `MediaPlayer`
      only; WMP Legacy has to accept 8- and 12-channel types on a stereo
      endpoint too (Phase 6 step 0).
    - `speaker_mask` still overrides both. An endpoint of three or more
      speakers keeps its own mask: its speakers are real, and a larger
      layout would only be downmixed back to them.
- **D5. A config of its own, in two places.** The defaults are
  `mf-config.yaml`, next to the DLL, written by the installer. A per-user
  override is `%LOCALAPPDATA%\harletty\mf\config.yaml`, if present, and wins.
  - The engine config is an Omniphony config file (same schema), with
    `render.osc: false` and no `bridge_path(s)`, since the MFT supplies the
    decoders itself (D3, then D10).
  - The MFT's own options live in a sibling file, `mf.yaml`, so the engine
    schema stays untouched: `speaker_mask`, `stereo_endpoint_mask` (from
    Phase 6 step 0, D4), `codecs`, `log_level`,
    `log_file`. It is looked up the same way: next to the DLL, overridden by
    `%LOCALAPPDATA%\harletty\mf\mf.yaml`.
  - Binaural for headphones is the engine's own option
    (`render.binaural.output_mode: binaural`,
    `../Omniphony/omniphony-renderer/BINAURAL.md`) and works with a stereo
    mask.
    - From Phase 6 step 0 it is only used when that file asks for it, not
      forced on stereo endpoints (D4).
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
- **D10. What ships is one DLL with the decoders linked in, released as a set
  of its own, apart from the Omniphony bridges.** (Decided after Phase 1;
  until then the plan shipped copies of the bridge DLLs in the MFT's folder
  and left a one-file DLL to Phase 5.)
  - **What the MFT does not need from Omniphony:** the plugin interface,
    meaning bridge files, finding them at run time, and the `abi_stable`
    load and layout check. Those exist so that a host and plugins built
    apart can meet. The MFT and its decoders are built together, from one
    commit, for one host, so that boundary only costs: two more files to
    find inside `wmplayer.exe`, an install that breaks if one is missing or
    from another build, and a load-time check that can only fail.
  - **What it keeps:** Omniphony's renderer, `orender_engine`, which renders
    the objects (D2), and the `bridge_api` types, since the engine takes its
    decoders through the `FormatBridge` trait, in process as well as from a
    file. The MFT does not export a root module, so it is not an Omniphony
    plugin, and Omniphony cannot load it.
  - **How:** link `bridge-family-dolby`, `bridge-family-dts` and
    `bridge-common`, and build the session with `Engine::new` over
    `BridgeLibs::new([dolby, dts])` (F10). That means the MFT assembles the
    engine itself, mirroring what `from_paths` does after `load_bridges`, at
    the pinned ref.
    - That copy is what the MFT should own anyway. It loads its config with
      `Config::load_or_default_with_status`, which leaves the live-handoff
      sidecar alone (F9), and logs the status to the MFT's log (that
      function's own parse warning goes to stderr, which a player discards). It skips the overlay prefs, the OSC config-path
      bookkeeping and the bridge-path recording, all of which are Studio's
      concerns and process-wide state in a player (Risks).
    - It can drift from upstream. A test feeds every fixture to both the
      in-process session and `from_paths` over the bridge cdylibs, and
      checks the output is identical. A pin bump that adds a setup step to
      `from_paths` fails there, not on a user's machine.
    - An upstream `Engine::from_libs` would remove the copy. It is a
      clean-up (Phase 5), not a blocker.
  - **Two release sets, not one.** The bridge archive stays what it is,
    Omniphony plugins for every platform. The MFT archive holds
    `harletty_mf.dll` and no bridge DLL. Both are built in the same
    `release.yml` run, from the same commit, with the same version. One set
    serving both is rejected:
    - **Their compatibility differs.** A bridge loads only in an Omniphony
      built against the same `bridge_api` minor (`.omniphony-ref`). The MFT
      carries its own engine, so it has no such constraint. If WMP used
      the user's Omniphony bridges, updating either one could break the
      other.
    - **They install differently.** Omniphony finds bridges in a folder the
      user picks, with no administrator. The MFT is registered machine-wide
      by an administrator (D9) and loaded by every Media Foundation process.
      A shared folder would make an Omniphony upgrade a change to what every
      player on the machine loads.
    - **Their audiences differ.** Most WMP users have no Omniphony, and the
      MFT is Windows only, x64 and ARM64. The bridges ship for Linux, macOS
      and Windows.
    - **Sharing saves nothing that matters.** The duplicated decoder code is
      a few MB on disk and one more link step in CI.
  - **The licence is GPL either way.** Even without the engine, the families
    link `bridge_api` (GPL-3.0-or-later), and so does `bridge-common`.
    - A non-GPL MFT would have to drop the families as well and drive the
      decoder crates directly, as the Apache-2.0 CLI does. That is not
      planned.
- **D11. Direction: decode the stream's own presentation, render (almost)
  nothing, give headphones the 7.1 output.** ***Rejected.*** The owner kept
  the engine after Phase 6's first gate run (D2). What follows is the record
  of what was evaluated, not work to do.
  - What survives elsewhere in the plan:
    - stereo endpoints get the 7.1 output (D4, Phase 6 step 0);
    - the release-scope rule.
  - The feature split under "Build contents" would only be needed without
    the engine. Its IEC 61937 part applies with the engine too (Phase 5).
  - **Why:**
    - On the speaker layouts Windows gives a PCM endpoint (≤ 7.1 in
      practice, *inferred*; F6), every Atmos object ends up folded into
      ≤ 7.1.
    - TrueHD already carries the encoder's own lossless fold
      (presentations 0–2, F14).
    - On beds the engine is an identity (F15).
    - What the engine costs inside every Media Foundation process (size,
      crates, threads, a large pin to track) then buys object panning that
      the stream already carries rendered, plus the binaural stage.
    - For the one listener so far, the binaural stage lost to a plain
      downmix of the 7.1 output (F16, "Gate results").
  - **Per codec, with the output mask from D4** (stereo endpoints get
    `0x63F`):
    - **TrueHD:** `presentation` = 1 for masks of 3–6 speakers, 2 for more.
      No objects are decoded.
      - The family built without `atmos` never decodes presentation 3. When
        the presentation asked for is missing, it takes the highest of 0–2
        the stream has.
      - The `truehd` crate's own fallback for a missing presentation can
        pick the object presentation (`max_independent_presentation`).
        Without `atmos` the family checks the presentation map, so a
        5.1-core Atmos stream gives its 6 channels whatever that fallback
        does.
      - That makes gate measurement 4 a test of this code (Phase 6 step 4),
        not an assumption.
      - Presentation 0 (the stream's own stereo) is used only if check 6
        prefers it to Windows' downmix of presentation 2.
    - **DTS, DTS-HD, DTS:X, Auro-3D over DTS-HD:** the family built without
      its DTS:X and Auro features ("Build contents") gives the lossless
      backward-compatible bed. The encoder folded the heights and objects
      into it.
    - **E-AC-3, AC-3:** not registered, and not built in (D6).
  - **Routing:** each frame label goes to its mask bit (`portable/layout.rs`
    keeps that table). Two kinds of label do not map:
    - `Ls/Rs` on a 5.1 mask with back bits go to the back speakers;
    - a label with no bit (`Lw/Rw`, `Lsd/Rsd`, `Cb`, `Lsc/Rsc` on smaller
      masks, heights on masks without them) is panned from its nominal
      position across its nearest speakers, power preserving.

    That is a fixed table worked out once per label set, not a renderer.
    Stereo and mono endpoints get the 7.1 mask (D4), so the table never mixes
    to fewer than eight channels.
  - **What stays from the engine path, ported to the session:**
    - DRC: the bridge's `drc_gain` and ramp, applied as
      `orender_engine::render::fill_pcm_f32_drc` does, about 30 lines. It is
      off unless `mf.yaml` sets `drc`.
    - Timestamps: the first block of a push carries that push's time. The
      latency is 0. `Block`, `OutputClock` and the transform do not change.
  - **What is lost:**
    - HRTF binaural inside the MFT.
      - Windows does not virtualise in its place on the one machine measured
        (F16). Headphones get Windows' plain downmix of the 7.1 output.
      - The owner preferred that downmix, unblinded, to the binaural stage.
    - Height speakers get nothing. Gate measurement 2 says whether they
      could at all; it is still open, for want of speakers.
    - Object panning, which only matters if a layout bigger than the
      stream's channel presentation can reach the speakers.
    - Engine parity with mpv-omniphony.
  - **Not chosen:**
    - *Objects dropped, bed kept:* it plays the LFE alone (F14).
    - *Objects to Windows* (`MFAudioFormat_Float_SpatialObjects`): no
      documented route (F6). That stays Phase 5 research.
    - *An object renderer of our own:* a dual-balance Cartesian panner of a
      few hundred lines came out about as close to Dolby's fold as the engine
      on one short clip (F15). It is the fallback only if the gate shows
      height speakers reachable, or a DTS:X fold the bed cannot give, or if
      test 3′ prefers object rendering. It would be its own Cargo feature of
      the MFT, off unless that holds.
  - **Build contents** (Phase 6 settles the exact feature names). The MFT
    depends on the shared crates with `default-features = false` and enables:
    - in `bridge-common`, no feature;
    - in `bridge-family-dolby`, `truehd`;
    - in `bridge-family-dts` and `dca`, nothing.

    The shared crates gain these features, all in `default` (release-scope
    rule):
    - `bridge-common/iec61937`: the IEC 61937 transport branch of
      `PluginBridge::push_packet`, with its burst-type routing. The MFT only
      pushes `RInputTransport::Raw`.
    - `bridge-family-dolby/truehd` and `bridge-family-dolby/eac3`: each
      codec's decode path and its crate (`truehd`, `eac3`), and the raw-codec
      sniffing between them.
      - `bridge-family-dolby/iec61937` gets the S/PDIF and MAT paths
        (`eac3_spdif.rs`, `mat.rs`).
      - `bridge-family-dolby/atmos` gets TrueHD presentation 3 and its
        objects: the OAMD-to-event path (`metadata.rs`) and the spatial
        labels.
      - Without it, `presentation` takes 0–2 and `best` means 2. The MFT
        never asks for 3, so that path would otherwise sit in the DLL unused.
    - `bridge-family-dts/dtsx`: the DTS:X extension path (`build_x_frame`,
      the fold plan, estimator and renderer, `dts-fold.yaml` with
      `serde_yaml_ng`).
      - `bridge-family-dts/auro` gets the Auro-3D unfold and the `auro`
        crate.
      - `bridge-family-dts/iec61937` gets `dts_spdif.rs`.
      - Without `dtsx` and `auro`, a frame is `bed_only_frame`'s, as for a
        stream with no extension today.
    - `dca/dtsx`: the extension-waveform decode in `dca` (`HdFrame::x_samples`,
      `dcadec/xll.rs`, `spatial.rs`). Phase 6 step 1 first checks how far it
      separates from the XLL bed decode; without it, `dca` decodes the core
      and the XLL bed only.

    **Left out of the MFT DLL** under D11, asserted by
    `check-crate-isolation.sh` (Phase 6 step 3):
    - `orender_engine` and `renderer`, and what only they bring: `live_log`,
      `runtime_control`, `script_backend`, `mlua`, `rosc`, `socket2`,
      `rayon`, `realfft`, `regex`, `serde_json`, `env_logger`, `omniphony_geometry`,
      `omniphony-osc-contract`;
    - `eac3` and `auro`. `truehd` keeps its own dependencies (`oamd`
      included): the MFT does not take features out of the decoder crates
      the CLI shares, other than `dca/dtsx`.
    - everything the check already excludes (IAMF, the plugin crates, the
      CLI's crates).
    - Code paths within linked crates that are compiled out as above:
      IEC 61937, E-AC-3, TrueHD objects, DTS:X, Auro-3D.
    - `bridge_common::plugin::root_module` and the probes are generic or
      unreferenced and do not reach the DLL. A symbol check on the built DLL
      confirms it (Phase 6 step 3).
  - **Gate:** Phase 6. Each measurement says which result keeps the engine
    (D2 as written). D11 replaces D2 only if none of them does.
    - What the first run settled and what it left open is in "Gate results".

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
                                     ├─ bridge-family-dolby, linked in  (TrueHD, E-AC-3)
                                     ├─ bridge-family-dts, linked in    (DTS family)
                                     └─ VBAP / binaural render to the generated layout
                 (until Phase 3, the two families are harletty_dolby_bridge.dll
                  and harletty_dts_bridge.dll next to harletty_mf.dll: D3)
                         │
                         ▼  float32 PCM IMFSample, WAVEFORMATEXTENSIBLE mask = endpoint's
 └─ Streaming Audio Renderer (SAR) ─ WASAPI shared mode ─ speakers
```

From Phase 6 step 0, a stereo or mono endpoint gets the full layout: 7.1
(`0x63F`), or 7.1.4 (`0x2D63F`) when `stereo_endpoint_mask` asks. Windows
downmixes it (D4).

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

- Unit test: for every mask the table covers, the YAML parses as an
  Omniphony layout (use the `renderer` crate's layout loader).
- Unit test: `label_for_name` of each generated name gives the table's label,
  in order.
- The YAML is written to `%LOCALAPPDATA%\harletty\mf\layout-<mask>.yaml`,
  only when absent or different, because `from_paths` takes a path. A mask
  with no speaker left falls back to stereo.
- From Phase 6 step 0, `plan` gives a mask of two speakers or fewer the
  stereo-endpoint layout (`stereo_endpoint_mask`, 7.1 by default; D4), not
  the forced binaural plan.
  - `plan` takes that mask as an argument, so it stays portable and
    testable.
  - Binaural runs only when `mf-config.yaml` asks for it, and then as
    today.

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
  Phase 3 narrows it to what D10 allows: the Dolby and DTS families, and
  nothing of IAMF or of the CLI.
- `.github/workflows/windows-arm64.yml` and a new x64 job: Phases 1, 2 and 4.
- `.github/workflows/release.yml`: Phase 4.
- `README.md`: Phase 4, one short section linking `docs/media-foundation.md`.

Phase 6 step 0:

- `mf-decoder/src/portable/layout.rs`: `plan(mask, stereo_endpoint_mask)`.
  Stereo and mono masks get the stereo-endpoint layout.
- `mf-decoder/src/portable/config.rs`: `stereo_endpoint_mask` (a number or
  `0x…` text, like `speaker_mask`), overlaid key by key, and its check.
- `mf-decoder/src/portable/session.rs`: passes it to `plan`; the presentation
  log line.
- `mf-decoder/src/com/transform.rs`: reads it from the config.
- `mf-decoder/install/mf.yaml`: the key, commented out, with its two useful
  values.
- `mf-decoder/tests/session.rs` and the unit tests (Phase 6 step 0).
- `docs/media-foundation.md`, "Speakers".

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

### Phase 3: one DLL, robustness and options

1. **Link the decoders in (D10).** First, so that every later step, the
   soak test included, runs on the binary that ships.
   - `mf-decoder` depends on `bridge-family-dolby`, `bridge-family-dts` and
     `bridge-common`, and defines a `Plugin` impl per family. It never
     depends on the plugin crates, whose root-module exports would clash
     (F10), and exports no root module itself.
   - `portable/session.rs` builds the engine with `Engine::new` over
     `BridgeLibs::new([dolby, dts])`, following `from_paths` at the pinned
     ref minus the parts D10 drops. A comment names the commit it follows.
     The folder lookup and the bridge paths of D3 go.
   - The `from_paths` session stays, test-only, as the reference: a portable
     test feeds every fixture to both and asserts identical output, with the
     bridge cdylibs kept as dev-dependencies for it.
   - `check-crate-isolation.sh`: the MFT may now link the Dolby and DTS
     families. It must still not depend on `bridge-family-iamf`,
     `iamf-dec`, `iamf-obu`, `iamf-codecs`, `clap`, `indicatif` or
     `env_logger`, nor on `harletty-dolby-bridge` / `harletty-dts-bridge`.
   - **(Windows desktop)** With only `harletty_mf.dll` in its folder, WMP
     plays the Phase 2 set, and `tasklist /m` shows no bridge DLL in
     `wmplayer.exe`.
2. **Fuzz-ish hardening:** truncated and garbage input (bit-flipped fixture
   blocks) must never panic out or hang. A `cfg(windows)` test drives the
   transform directly with corrupted samples.
3. **Panic path test:** a test-only codec that panics (behind
   `#[cfg(test)]`) shows `E_UNEXPECTED` comes back, the next sample decodes,
   and the process lives.
4. **Device change:** the mask is read when the session is built. When the
   default endpoint changes mid-file, the next file uses the new one.
   Document it. Rebuilding mid-stream through `MF_E_TRANSFORM_STREAM_CHANGE`
   is optional, and only if Phase 0 showed WMP renegotiates cleanly.
5. **`mf.yaml` options:** `speaker_mask` (hex), `codecs` (`[truehd, dts,
   eac3, ac3]`), `log_level`, `log_file`. Plus the engine's own options
   through `config.yaml`: binaural, DRC (`render.drc` or the engine's
   equivalent), presentation. Test that each is honoured (portable tests).
6. **Memory and handles:** play and seek a 2-hour file in a loop for an hour
   (Windows desktop, with Task Manager or `perfmon`). Working set and handle
   count must stay flat.

Done when the tests pass, the shipped DLL loads no bridge DLL, and the soak
test shows no growth.

### Phase 4: packaging, release and documentation

1. `install/install.ps1` (requires administrator, `-WithDolbyDigital`
   optional, `-Uninstall` alias):
   - copy `harletty_mf.dll`, `mf-config.yaml`, `mf.yaml` and the licence
     texts to `%ProgramFiles%\harletty\mf\`. No bridge DLL is installed
     (D10);
   - enable the opt-in codecs in `mf.yaml` when asked;
   - `regsvr32 /s` the DLL; check `MFTEnumEx` finds it;
   - print what was done and how to undo it.

   `uninstall.ps1` reverses it, and leaves `%LOCALAPPDATA%\harletty\mf\`
   alone unless `-Purge` is given.
2. `release.yml`: on Windows x64 and ARM64, in the same run as the bridges,
   build the MFT (static CRT, as the bridges are). Stage the licences:
   extend `scripts/stage-licences.sh` for the new binary. It carries what
   the Dolby and DTS bridges carry (their decoders are linked in), plus
   Omniphony's GPL-3.0 engine, `windows` (MIT/Apache-2.0) and Lua/`mlua`
   (MIT); check each actual dependency with `cargo tree`. Then zip
   `harletty-mf-<version>-windows-<arch>.zip` with the install scripts.
   - Two sets per release (D10): the bridge archives, unchanged, for
     Omniphony; and the MFT archives, which hold no bridge DLL.
   - The release notes name the Omniphony commit, as for the bridges. They
     say which archive is for which use: Omniphony users take the bridges,
     Windows Media Player users take the MFT, and neither needs the other.
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

- **Upstream `Engine::from_libs`:** a constructor in Omniphony taking
  `(config, layout, BridgeLibs, codec, rate)`, factored out of the part of
  `from_paths` after `load_bridges` (F10). The MFT's own copy of that
  assembly (D10) then shrinks to a call. It needs an Omniphony pull request
  and a pin bump. The one-file DLL does not wait for it: Phase 3 already
  ships one.
  - The engine stays (D2, Phase 6), so this item is live.
- **Spatial objects to Windows Sonic / Dolby Atmos for headphones:** output
  `MFAudioFormat_Float_SpatialObjects` (F6). Research first: which metadata
  format ids the in-box spatial renderers accept, and whether a third-party
  decoder can use one.
  - What the documentation gives is in F6: no public format id, and a
    metadata client only for a format the endpoint supports. The research
    starts from what Windows' own pipeline does, not from the documentation.
  - **First step, a probe on a machine with the spatial formats:**
    - list the decoders' `MFAudioFormat_Float_SpatialObjects` output types
      and their metadata-format GUIDs under each Spatial audio setting.
      Microsoft's Dolby Digital Plus decoder on an E-AC-3 JOC track is the
      key case;
    - ask `ISpatialAudioClient::IsSpatialAudioStreamAvailable` for each
      GUID;
    - play the E-AC-3 track through Microsoft's decoder and log the output
      type it negotiates.
    - The unmerged branch `feat/probe-loopback-measure` has probe options to
      start from (`--audio-track`, `--force-dll`, `--measure`).
  - **The listening half of that check was skipped on the owner's machine.**
    After matching levels, the owner could not tell WMP, mpv and VLC apart on
    the E-AC-3 track with Pixel Buds A-Series (mpv and VLC hand no objects
    to the system), so listening there cannot show anything. That is one
    listener on one pair of earbuds. The probe's logged types do not depend
    on hearing.
  - It is the one route by which objects, and heights, could reach Windows
    from an MFT. It would be an MFT Cargo feature beside the engine, used
    only when the endpoint accepts the format.
    - The engine stays the output when spatial sound is off: Windows then
      offers no objects.
    - A Dolby- or DTS-private format is not proposed: undocumented, and
      licensed.
- **Object output from a player that owns its audio output** (F6): the
  documented route.
  - A player that renders its own audio (not WMP or the WinRT
    `MediaPlayer`) can open `ISpatialAudioClient`. It would send the decoded
    bed as static objects and each object as a dynamic object, mapping our
    room coordinates to `SetPosition`.
  - It falls back to the engine when `GetMaxDynamicObjectCount` is 0.
  - That is a different product from this decoder, and needs its own plan.
- **Leave unused code out of the engine-based DLL** (the release-scope rule):
  - **In this repository:** the IEC 61937 paths of `bridge-common` and the
    two families (the MFT only pushes `RInputTransport::Raw`). They go under
    Cargo features in `default`, which the MFT turns off, with the
    release-scope check.
  - **In Omniphony:** OSC and scripting (`rosc`, `socket2`, `mlua`) are
    linked but never used by the MFT. Leaving them out needs optional
    features upstream and a pin bump.
- **IAMF and elementary streams:** a byte-stream handler / media source of
  our own for `.mka`/`.mkv` with IAMF (see Omniphony
  `docs/iamf-matroska-mapping.md`) and for `.thd`/`.dts`/`.eac3`. This is
  large: it is a demuxer.
- **The new Media Player app:** only through a Store media extension. The
  in-process kind needs Microsoft to grant `inProcessMediaExtension` (F3),
  which is out of our control. The app-service kind (F12) is open to us if
  Phase 0 step 4.5 shows it works, and would get a plan of its own.

### Phase 6: the renderer question (D11), answered: the engine stays

Phase 6 asked whether the MFT could do without `orender_engine` (D11). Its
gate was run once (below), and the owner decided to keep the engine (D2). What
remains to do is step 0, which the gate run justified on its own, and its WMP
check. The gate and its results stay here as the record.

**Gate.** Four **(Windows desktop)** measurements, made with the MFT as it
ships today (Phase 4) and recorded in `docs/media-foundation-findings.md`.
Each says what result keeps the engine, that is D2 as written, with Phase 6
stopping here. If one does, the result goes into F16 or F6 and D11 is marked
rejected. Only if none does do the steps below start.

1. **Headphones** (F16). On a headphone endpoint, read the shared-mode mix
   format's mask (the MFT's log prints it) with Windows Sonic for Headphones
   off, then on.
   - With it on, play a TrueHD file in WMP and in the WinRT `MediaPlayer`,
     with `speaker_mask: 0x63F` in `mf.yaml` if the mix format is not 7.1.
   - Record whether the sound is virtualised: front, side and back speaker
     test tones are heard apart.
   - **Keeps the engine:** Windows does not virtualise a 7.1 stream from a
     Media Foundation player. Without the engine, headphones would then get
     plain stereo, and the engine's binaural stage is worth what it costs.
2. **Heights.** On every endpoint at hand, Dolby Atmos for home theater or
   headphones and any 7.1.4-capable virtual device included, set
   `speaker_mask` to 7.1.4 (`0x2D63F`).
   - Record whether WMP and the WinRT `MediaPlayer` accept the 12-channel
     float type.
   - Record whether a test signal on `SPEAKER_TOP_*` comes out from above,
     or is folded or dropped.
   - **Keeps object rendering:** heights come out as heights on an endpoint a
     user can have. Then the presentation-only design would throw away
     speakers that work.
   - Object rendering is then either the engine, or the small panner as an
     MFT feature (D11, "Not chosen"). Measurement 3 decides which.
3. **Listening and metrics.** A full-length TrueHD Atmos title and a DTS:X
   title, on a 7.1 and a 5.1 system:
   - today's engine render, against presentation 2 or 1 (TrueHD) and the
     lossless bed (DTS:X). Until Phase 6 step 2 exists, take the bed from
     FFmpeg's DCA decoder, which decodes DTS-HD MA and, as far as known,
     ignores the DTS:X extension (*verify* it on the stream used);
   - blind A/B, plus per-channel level and correlation, as F15 measured them.
   - **Keeps the engine:** the engine's render is preferred, or the bed is
     found to lack content the engine places (a DTS:X stream whose objects
     the bed does not fold).
4. **A 5.1-core Atmos TrueHD stream** (no 8-channel presentation): asking for
   presentation 2 must give the 6-channel presentation, not the object one.
   - This is a check of D11 itself, and not an engine keeper.
   - If it fails, the Dolby family needs a feature exposing the presentation
     map, and the MFT selects from it.

**Gate results (first run).** The owner ran these on a Windows ARM64 laptop
with two-channel headphones and the WinRT `MediaPlayer`. There were no
loudspeakers, no WMP and no DTS:X. The reports are in
`docs/media-foundation-findings.md` ("Phase 6 gate"). They are one listener's
judgements, and only the identification test was blind.

1. **Headphones: answered.** The mix format stayed stereo at all three Spatial
   audio settings ("Off", "Windows Sonic for Headphones", "Dolby Atmos for
   Headphones"). Pinned 8- and 12-channel outputs sounded alike at all three.
   - *Inferred:* Windows downmixes and does not virtualise (F16).
   - Read literally, the keep-condition above holds. Its reason does not: it
     assumed the binaural stage beats a plain downmix. The owner heard the
     forced binaural output as worse than Windows' plain downmix of the 7.1
     output, run after run. That comparison was unblinded.
   - **Decision:** binaural does not keep the engine. Stereo endpoints get
     the 7.1 output now (step 0) and under D11.
   - This needs no further measurement to proceed. A blind repeat, or a
     second listener, would make it firmer and is welcome.
2. **Heights: answered in part.** The WinRT `MediaPlayer` accepts and plays
   a 12-channel type on a stereo endpoint (F6).
   - Height channels on height speakers: **not measured.** There were none.
   - **On headphones the owner hears height differences.**
     - The blind test bears out that something differs: with presentation 3,
       the 7.1.4 and 7.1 renders were told apart 6 times in 6.
     - What is unknown is how that difference reaches the ears.
     - The headphones are Google Pixel Buds A-Series. The owner says they
       support spatial audio, but Google's help page says Spatial Audio "is
       not compatible with Pixel Buds A-Series"
       (<https://support.google.com/googlepixelbuds/answer/12921041>).
     - The Windows Spatial audio setting made no audible difference.
     - *Inferred:* the buds play the two channels Windows downmixes to, with
       no processing of their own. The 12-channel render then differs from
       the 7.1 one only in how the engine's top-layer signal folds into that
       stereo. The owner may hear that as height, but no elevation cue is
       added.
   - **Decision:**
     - This is not evidence that heights reach height speakers, so for
       loudspeakers measurement 2 stays open. With the engine kept, a user
       with a 7.1.4 PCM endpoint can already pin `speaker_mask` to it.
     - It was a reason to test the 12-channel render on headphones as a
       preference. 3′ was to include it, and was not run.
3. **Listening: answered in part.**
   - **Blind:**
     - presentation 2 and the engine's render of presentation 3 were told
       apart in 12 of 12 trials;
     - with presentation 3, the 7.1 and 7.1.4 masks were told apart in 6 of
       6, so the engine renders them audibly differently even after Windows'
       downmix;
     - with presentation 2 the masks were not told apart (2 of 6, chance).
       That is consistent with F15: its 8 channels reach either layout
       unchanged.
   - **Unblinded:** the owner leaned to presentation 2. The engine's 7.1
     render of presentation 3 was "so loud", closer and toward the top
     centre, and earlier judged worse than mpv's 7.1, which the owner recalls
     plays the non-Atmos fallback (unchecked).
   - **No blind preference has been measured,** and the level difference may
     have told the conditions apart on its own. 3′ was to measure it, and was
     not run (Decision, below).
   - DTS:X and loudspeakers were not tested.
4. **5.1-core Atmos: not tested, no sample.** Moot with the engine kept: the
   engine decodes and renders presentation 3, so the presentation fallback
   that D11 depended on is not used.

**Decision (owner).** Keep the engine. The owner judged that this hardware
cannot decide the remaining question: two-channel Pixel Buds A-Series,
through Windows' stereo downmix, cannot show whether presentation 2 or the
engine's presentation 3 is better, even at matched levels. D2 stands, and
D11 is rejected.

**Not run, and why** (recorded in `docs/media-foundation-findings.md`):

- **3′, a blind, level-matched preference test** between presentation 2 and
  the engine's presentation 3 at 7.1 and 7.1.4. It was not run, for the
  reason above. A probe and script for it exist on the unmerged branch
  `feat/probe-loopback-measure`, should the question come back on other
  hardware.
- **The E-AC-3 listening check**: through Microsoft's decoder, does the
  Spatial audio setting change anything? It was skipped.
  - After matching levels, the owner could not tell WMP, mpv and VLC apart
    on these earbuds. By the owner's account, mpv and VLC hand no objects to
    the system.
  - So listening on this hardware cannot show anything. That is one listener
    on Pixel Buds A-Series.
  - The logged-type half of that check moves to Phase 5.
- **Checks that only D11 needed** (DTS:X bed against render, the stream's
  own stereo presentation, the 5.1-core Atmos fallback) are dropped with D11.
- **Loudspeakers and height speakers** were never at hand. They stay open
  for whoever has them: the Phase 2 set on a 5.1 or 7.1 system, and
  `speaker_mask` pinned to 7.1.4 on a 7.1.4 PCM endpoint.

**Steps:**

0. **Stereo endpoints get the full stream as a full layout** (D4), with the
   engine:
   - `layout::plan(mask, stereo_endpoint_mask)`: an endpoint of two speakers
     or fewer gets `stereo_endpoint_mask` for output and render, not forced
     binaural. The default is `0x63F` (7.1); `0x2D63F` (7.1.4) is the other
     useful value.
     - The binaural stage runs only when `mf-config.yaml` sets
       `render.binaural.output_mode: binaural`, and then as today.
     - `speaker_mask`, when set, still wins over both.
   - `mf.yaml` gains `stereo_endpoint_mask`, parsed like `speaker_mask`.
     - A mask with fewer than three speakers the layout table covers is
       refused with a log line, and 7.1 is used.
     - It applies only to endpoints of one or two speakers.
   - The session logs the decoded presentation (`Engine::source_label`,
     `bed_labels`) and the output mask at the start and whenever they change.
     The identification test had no such line.
   - **Tests** (portable, Linux and Windows CI):
     - `plan` for masks `0x3` and `0x4`, with no option (gives `0x63F`), with
       `0x2D63F` (gives `0x2D63F`), and with an invalid value (gives
       `0x63F`);
     - `plan` for a 5.1 endpoint ignores the option;
     - config parsing of the key (number, `0x…` text, nonsense) and the user
       folder overriding it;
     - a session at mask `0x3` returns 8 channels by default, and 12 with
       the option;
     - **streams without objects at 7.1.4** (the DTS core fixture, and the
       TrueHD 5.1 sample where CI has it): the four top channels are exactly
       zero, and the other eight equal the 7.1 session's output, sample for
       sample;
     - an object stream at 7.1.4 (`atmos.thd` where CI has it; the E-AC-3
       JOC fixture): the top channels carry signal.
   - `docs/media-foundation.md`, "Speakers", says:
     - stereo devices get the whole stream rendered to 7.1, and Windows
       downmixes it;
     - `stereo_endpoint_mask: 0x2D63F` renders to 7.1.4 instead;
     - objects are never passed to Windows as objects (F6);
     - binaural is an option.
   - `install/mf.yaml`: the key, commented out, with both values explained.
   - **(Windows desktop)** Three checks:
     - WMP Legacy accepts the 8-channel type, and with the option the
       12-channel type, on a stereo endpoint, and plays them.
     - The WinRT `MediaPlayer` result is repeated with the new DLL, for both
       values.
     - If WMP refuses either type, the transform also offers a stereo type,
       second (index 1): the engine's binaural render.

Done when:

- step 0 is merged with its tests;
- the WMP and WinRT `MediaPlayer` desktop checks pass, or the stereo
  fallback type is in and WMP plays with it.

## Verification matrix

| What | Where | Phase |
|---|---|---|
| registered stub seen by `MFTEnumEx` / `CodecQuery` in a desktop process | Windows CI (admin) | 0 |
| which players load the stub: WMP, WinRT `MediaPlayer`, FluentFin, Media Player app | Windows desktop | 0 |
| codec table, layout generation, clock, config | Linux + Windows CI, unit tests | 1 |
| registration round trip through `MFTEnumEx` | Windows CI (admin) | 1 |
| decode + render of the fixtures through `Session` | Linux + Windows CI | 2 |
| MF's own decoder resolution, bit-exact vs `Session` | Windows CI (`MFTRegisterLocal`) | 2 |
| crate isolation (no decoder linked into the MFT; from Phase 3, only the Dolby and DTS families) | Linux CI | 1, 3 |
| in-process session identical to `from_paths` over the bridge DLLs | Linux + Windows CI | 3 |
| corrupted input, panic containment | Windows CI | 3 |
| WMP plays TrueHD/Atmos, DTS/DTS:X MKV; seek, sync | Windows desktop | 1–2 |
| soak (memory, handles) | Windows desktop | 3 |
| install / uninstall from the release zip | Windows desktop (clean VM) | 4 |
| D11 gate, first run (headphones, WinRT player): recorded, engine kept | Windows desktop | 6 |
| stereo endpoints get 7.1, or 7.1.4 with `stereo_endpoint_mask`; streams without objects leave the tops silent; binaural only on request; WMP accepts 8 and 12 channels; presentation logged | Linux + Windows CI, Windows desktop | 6 (step 0) |
| a shared-crate feature (Phase 5, IEC 61937) leaves bridge and CLI output and feature resolution unchanged (release scope) | Linux CI + local corpus (`tools/host-baseline`) | 5 |
| presentation choice, routing table, DRC ramp, timestamps without the engine | Linux + Windows CI | 6 |

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
  Measured 15–36 ms on Linux x86-64 (F15); Windows ARM64 is not measured.
- **Global state in `orender_engine` inside a long-lived host process.**
  `overlay`, the live-log logger, OSC, `migrate_legacy_windows_config` and
  the live sidecar (F9). The MFT calls none of the OSC or logging entry
  points. It must check `Engine::from_paths` for any other process-wide side
  effect at the pinned ref, and note what it found in the Phase 2 pull
  request. From Phase 3 the MFT assembles the engine itself (D10) and leaves
  out the overlay prefs and the sidecar, so what remains is in `Engine::new`
  and what it calls.
  - That includes threads that outlive the file being played: a process-wide
    rayon pool and a layout-follower thread per renderer (F15). They stay
    with the engine.
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
  - Without the engine it would be no different: the families link
    `bridge_api`, which is GPL-3.0-or-later (D10).
- **Omniphony API drift.** `orender_engine` is not a stable API. A pin bump
  that changes `Engine::from_paths` or `RenderedAudio` breaks only this
  crate, and CI shows it at the bump. The MFT's copy of the engine assembly
  (D10) can also fall behind silently, with a step `from_paths` gained. The
  Phase 3 test comparing it with `from_paths` turns that into a CI failure
  at the bump.
  - At the pin, `orender_engine` and `renderer` had 754 commits since May
    2026, against 24 for `bridge_api`.
  - The engine stays (D2), so this drift is tracked at every pin bump.
- **Binaural off by default (Phase 6 step 0).** Windows did not virtualise
  on the one machine measured (F16). The owner preferred Windows' plain
  downmix of the 7.1 output to the binaural stage (unblinded). So stereo
  endpoints get the full layout (7.1, or 7.1.4 on request), and binaural
  only on request.
  - The preference is one listener's on one pair of earbuds. The option
    stays, so a user who prefers binaural keeps it.
- **Windows' downmix of the 7.1 (or 7.1.4) output** is now what headphone
  and laptop-speaker users hear by default.
  - Its matrix is Windows', not ours, and how it folds the four top channels
    is not known.
  - It has been heard only through the WinRT `MediaPlayer` on one machine.
    Step 0 checks WMP.
  - Which layout sounds better on Atmos is not measured. That is why 7.1.4
    is an option, not the default.
- **Cargo feature unification.** The features of one package are the union
  over everything built in the same `cargo` invocation.
  - That is why the release-scope rule's features only add code:
    unification can then only put more into the MFT, never change a plugin.
  - It is also why `release.yml` keeps the MFT in a `cargo build` of its own.
  - A workspace-wide build or test (`--workspace`, or the MFT's
    dev-dependencies) may compile the MFT with more than it ships. The
    crate-isolation check runs on the release build's feature set for that
    reason.
- **The upstream release changing by accident.** Any pull request that
  touches a shared crate carries the release-scope check. A missing check is
  a reason to reject the pull request.

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
