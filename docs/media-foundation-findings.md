# Media Foundation decoder: Phase 0 findings

Results of the questions in `docs/plan-media-foundation-decoder.md`, Phase 0.
Each entry says where it was measured. Entries marked **desktop** still need a
person at a Windows PC and are open.

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

## Open (desktop)

- Which of WMP Legacy, the WinRT `MediaPlayer`, FluentFin (direct play) and
  the Media Player app load the stub, and which MFT they choose (matrix in the
  plan, Phase 0 step 4.4).
- The app-service route (F12), only if the WinRT rows fail.
- Subtype GUIDs the MKV source really sets, and which `wmplayer.exe` the Start
  menu runs (`tools/mf-probe`, not yet written).
