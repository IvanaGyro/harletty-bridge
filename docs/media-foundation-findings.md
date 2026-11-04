# Media Foundation decoder: Phase 0 findings

Results of the questions in `docs/plan-media-foundation-decoder.md`, Phase 0.
Each entry says where it was measured. Entries marked **desktop** still need a
person at a Windows PC and are open.

The tools behind these numbers (a stub decoder that plays silence, a player
probe, and the CI workflow that ran them) are not part of `dev/wmp`. They are
saved on the branch `archive/mf-phase0-tools` (`harletty_mf_stub.dll`,
`mf-player-probe.exe`, `.github/workflows/windows-mf-phase0.yml`) if a later
question needs them.

## Measured in CI (`windows-mf-phase0.yml`)

Run 41318298761 (commit `7f08a30`), on the hosted `windows-11-arm` (arm64)
and `windows-latest` (x64) runners: Windows client builds with Media
Foundation, administrator rights, no audio device. The probe is a normal
unpackaged desktop process. Both jobs passed. The probe was run before the
stub was registered, and after.

### F11: a registered decoder is visible to the WinRT stack in a desktop process — confirmed (arm64)

After `regsvr32` of the stub, `CodecQuery.FindAllAsync(Audio, Decoder, <subtype>)`,
the call the FluentFin device profile makes, lists it, with `IsTrusted = true`:

| Subtype asked | Before registering | After registering |
|---|---|---|
| TrueHD `{EB27CEC4-…}` | Windows "Dolby TrueHD IEC-61937 converter MFT" | that, and "harletty MF stub decoder (phase 0)" |
| DTS-HD `{A2E58EB7-…}` | Windows "DTS IEC-61937 converter MFT" | that, and the stub |
| DTS `{00000008-…}` (`MFAudioFormat_DTS`) | **no codec** | the stub |
| E-AC-3 `{A7FB87AF-…}`, AC-3 `{E06D802C-…}` | Windows "Dolby AC-3 and E-AC-3 IEC-61937 converter MFT" | unchanged (stub registered for the first three only) |

Consequences:

- A machine-wide `MFTRegister` is enough for a desktop process to *see* the
  decoder through both `MFTEnumEx` and the WinRT `CodecQuery`. No packaging
  and no capability was needed.
- FluentFin enables DTS in its Jellyfin device profile only when `CodecQuery`
  finds an `MFAudioFormat_DTS` decoder (F13). Before registering there is
  none, so it does not ask the server for DTS. With the decoder registered
  it would.
- The x64 job passed too; its probe output is in the run's artifact
  (`harletty-mf-phase0-x64-…`), not yet compared line by line.

**Not answered:** that only shows the decoder is *listed*. Whether
`Windows.Media.Playback.MediaPlayer` *loads* it when playing a file needs
the desktop run (Phase 0 step 4.4). The stub log was empty in CI: nothing
instantiated the DLL, because enumerating does not create the object, and no
file was played.

### New fact (corrects F5): Windows already has decoder-category MFTs for TrueHD and DTS-HD

The plan assumed Windows ships nothing for TrueHD or DTS. It ships
**IEC 61937 converters** registered in the audio-decoder category:

- "Dolby TrueHD IEC-61937 converter MFT" for TrueHD;
- "DTS IEC-61937 converter MFT" for DTS-HD and related subtypes
  (`{00002001-…}`, `{A61AC364-…}`, `{C2FE6F0A-…}`, `{E06D8033-…}`);
- "Dolby AC-3 and E-AC-3 IEC-61937 converter MFT" for AC-3 and E-AC-3.

They wrap a stream for S/PDIF/HDMI passthrough rather than decode it. They
are candidates for the same input types as ours, so which one the topology
loader picks for TrueHD and DTS-HD is no longer obvious. **Desktop** (step
4.4): does WMP or `MediaPlayer` play a TrueHD MKV with only the stub
registered, and which MFT does it instantiate? If it picks Windows' converter,
the plan needs a way to win (a different output type the converter cannot
produce, or a registration the sort order puts first).

## Measured on a desktop (Windows ARM64, the arm64 build from run 41318298761)

`mf-player-probe` (an unpackaged desktop process) with the stub registered and
a TrueHD 7.1 MKV:

- **The WinRT `MediaPlayer` loaded the registered stub for TrueHD.** The stub
  log shows `DllGetClassObject`, `SetInputType subtype={EB27CEC4-…}` (TrueHD),
  and `ProcessOutput`, from the probe's PID, `desktop` (not AppContainer). The
  player reported `MediaOpened`.
- **It chose the stub over Windows' own "Dolby TrueHD IEC-61937 converter
  MFT".** `MFTEnumEx` with `SORTANDFILTER` lists the converter first and the
  stub second, yet the stub was instantiated. Most likely the converter cannot
  produce the float PCM the player asks for; not tested further. This answers
  the F5 worry for this player. WMP and FluentFin are still open.
- `MFTEnumEx` and `CodecQuery` agree. `UNTRUSTED_STOREMFT` only reorders the
  list here.
- E-AC-3/AC-3: Windows' "Dolby Digital Plus Decoder MFT" is listed (its CLSID
  attribute is absent, shown as `?`), next to the IEC 61937 converter. The
  stub was not registered for these in this run.
- **Playback stalled.** The position reached 0.80 s in 10 s. Not yet
  explained: a bug in the stub's flow or timestamps is the first suspect. The
  stub now logs the first three `ProcessInput`/`ProcessOutput` calls, every
  hundredth after, every `ProcessMessage`, and the accepted output type, to
  find where it stops.

## Phase 6 gate, measured on a desktop (owner's listening, one listener)

**Setup:**
- a Windows ARM64 laptop, two-channel headphones;
- the WinRT `MediaPlayer` through `mf-player-probe`;
- the MFT as Phase 4 ships it, with the engine.

The headphone endpoint's Spatial audio setting was tried at each of its three
values: "Off", "Windows Sonic for Headphones" and "Dolby Atmos for
Headphones". What follows is the owner's report and the decoder's log.
Nothing was measured with an instrument, and no loudspeakers were used.

1. **No pinned mask.**
   - In all four runs, at all three Spatial audio settings, the log shows
     `output: 2 channels, mask 0x3` and "rendering binaurally".
   - So the endpoint's shared-mode mix format stayed stereo with Windows
     Sonic for Headphones and Dolby Atmos for Headphones on.
   - The owner judged that binaural sound worse than mpv's. The file had no
     Atmos presentation.
2. **`speaker_mask` pinned to `0x63F` (8 channels) or `0x2D63F` (12).**
   - The player accepted the type and played in every run. The sound was
     "much better", "very similar to mpv".
   - On a non-Atmos file (two masks × the three settings, start of the file),
     the six runs sounded very similar to each other. Front, side and back
     could not be told apart.
   - On a Dolby Atmos TrueHD file (passages at 315 s and 555 s), the Spatial
     audio setting made no audible difference.
3. **Heights.** The owner first reported that with 12 channels the four top
   sounds came from above, then corrected it: where they sit in the plane
   above differs between this decoder and mpv.
   - The owner recalls that mpv cannot decode Atmos and plays the fallback.
     That is not checked.
   - The owner does hear height differences.
   - The headphones are Google Pixel Buds A-Series, which the owner says
     support spatial audio.
   - **Checked against Google's help page**
     (<https://support.google.com/googlepixelbuds/answer/12921041>): Spatial
     Audio "is not compatible with Pixel Buds A-Series". It is a Pixel Buds
     Pro / Pro 2 feature with Pixel phones; the page does not mention
     Windows.
   - **Inferred, not measured:** over Bluetooth from Windows the buds receive
     the two channels Windows has downmixed to, with no spatial processing of
     their own. Whatever differs between the 7.1 and 7.1.4 renders reaches
     the ears through that stereo mix, not as height reproduction.
   - The difference itself is real: the blind test below told the two masks
     apart 6 times in 6 with presentation 3.
4. **Blind identification**, 12 trials, 3 per condition, shuffled:
   - The conditions were TrueHD presentation 2 or 3 (`render.presentation`
     in `mf-config.yaml`) × mask `0x63F` or `0x2D63F`, on a 5 s passage. The
     owner named the condition.
   - The log confirmed the channel count of every trial. It has no line for
     the presentation decoded.
   - **Results:**
     - presentation right in 12 of 12;
     - mask right in 6 of 6 with presentation 3, and in 2 of 6 with
       presentation 2;
     - exact condition right in 8 of 12.
   - **Per trial:**
     - T1, T3 and T11 (p3/7.1.4) were right; T5, T6 and T7 (p3/7.1) were
       right.
     - T2 and T4 (p2/7.1.4) were answered p2/7.1; T8 (p2/7.1.4) was right.
     - T9 and T10 (p2/7.1) were answered p2/7.1.4; T12 (p2/7.1) was right.
5. **Not blind, one passage.**
   - Presentation 3 at the 7.1 mask was "so loud" next to presentation 2,
     and seemed closer and toward the top centre ("the sound is front right
     top").
   - Earlier, the owner judged the engine's 7.1 render of presentation 3
     worse than mpv's 7.1.
   - No blind preference between presentations 2 and 3 has been measured.
6. **Not tested:**
   - a 5.1-core Atmos TrueHD stream (gate measurement 4: no sample);
   - DTS:X;
   - a 5.1 or 7.1 loudspeaker system;
   - WMP Legacy;
   - any other device.

**What follows from it**, as the plan reads it (Phase 6, "Gate results"):
- **Inferred:** Windows downmixes an 8- or 12-channel Media Foundation stream
  to stereo here, at all three Spatial audio settings, and does not
  virtualise it. F16 is not confirmed on this machine.
- **Measured, for this listener:** the engine's binaural stage loses to that
  downmix. That comparison was not blind.
- **Consistent with F15:** presentation 2 sounds the same at either mask
  (2 of 6 is chance), because its 8 channels reach the 7.1.4 layout unchanged
  and the tops stay silent.
- **Measured, blind:** the engine's 7.1.4 render of presentation 3 sounds
  different from its 7.1 render, even downmixed to stereo.
  - Whether the owner prefers it is not measured.

**Afterwards (owner):**
- **Decision: keep the engine.** The blind, level-matched preference test 3′
  was not run. The owner judged that two-channel Pixel Buds A-Series, through
  Windows' stereo downmix, cannot show whether presentation 2 or the engine's
  presentation 3 is better, even at matched levels.
- **The E-AC-3 check was skipped:** play the E-AC-3 JOC track through
  Microsoft's decoder and see whether the Spatial audio setting changes
  anything.
  - After matching levels, the owner could not tell WMP, mpv and VLC apart on
    these earbuds. By the owner's account, mpv and VLC hand no objects to the
    system.
  - So on this hardware listening cannot show anything. One listener, Pixel
    Buds A-Series.
- **The owner's hypothesis:** the Spatial audio settings made no difference
  because they act only when objects are handed to the system.
  - It agrees with Microsoft's Spatial Sound documentation (plan F6, F16).
  - It was not tested on its own.
- Probe additions for these checks (`--measure` with WASAPI loopback,
  `--audio-track`, `--force-dll`, `scripts/blind-test-3prime.ps1`) are on
  the unmerged branch `feat/probe-loopback-measure`.

## Open (desktop)

- Whether WMP Legacy, FluentFin (direct play) and the Media Player app load
  the stub (the WinRT `MediaPlayer` in a desktop process does; matrix in the
  plan, Phase 0 step 4.4).
- Why playback stalls after 0.8 s with the stub.
- The app-service route (F12), only if the WinRT rows fail.
- Subtype GUIDs the MKV source really sets, and which `wmplayer.exe` the Start
  menu runs (`tools/mf-probe`, not yet written).
