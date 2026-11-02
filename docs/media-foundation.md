# harletty decoder for Windows Media Foundation

`harletty_mf.dll` is an audio decoder that Windows' media stack (Media
Foundation) can pick for **Dolby TrueHD** (including Atmos), **DTS, DTS-HD and
DTS:X**, and optionally **E-AC-3 and AC-3**. A player that plays through Media
Foundation then plays those tracks like any other: the decoder turns them into
PCM for your speakers, or for headphones when the output is stereo.

It is the decoders of this repository and the rendering engine of
[Omniphony](https://github.com/mgth/Omniphony) in one DLL. It needs no bridge
plugin and no Omniphony install.

## Who it is for, and who it is not

It is for programs that play through Media Foundation **as 64-bit programs**
(x64 or ARM64, matching the DLL). It is built and its installer is tested
on Windows 11; Windows 10 has not been tried:

- apps that use the Windows `MediaPlayer` (`Windows.Media.Playback`), such as
  FluentFin's "Windows Media Player" engine;
- 64-bit Windows Media Player Legacy, if your copy is 64-bit.

What is checked and what is not (at the time of this release):

| Player | State |
|---|---|
| A desktop program using the WinRT `MediaPlayer` (the project's `mf-player-probe`) | the decoder is picked and a TrueHD 7.1 MKV plays; that the sound is right has not been confirmed yet |
| FluentFin | not confirmed here. FluentFin also decides itself whether to ask a Jellyfin server for TrueHD, which is separate from this decoder |
| Windows Media Player Legacy (64-bit) | not confirmed here |

It is **not** for:

- **32-bit programs.** A 32-bit `wmplayer.exe` (the one under
  `C:\Program Files (x86)`; the Start menu may point to it) cannot load a
  64-bit DLL. Nothing here changes that.
- **The new "Media Player" app** from the Microsoft Store (and the Movies & TV
  app). They only load decoders that come in a Store package, which a
  registered DLL is not.
- Programs that bring their own decoders (VLC, mpv, MPC-HC with its filters):
  they do not need it.

If you use [Omniphony](https://github.com/mgth/Omniphony), you want the
`harletty-bridge-*` archives instead; the two are separate and neither needs
the other.

## Install

1. Download `harletty-mf-<version>-windows-x86_64.zip` for an Intel/AMD PC, or
   `harletty-mf-<version>-windows-arm64.zip` for an ARM PC (Snapdragon X and
   the like). *Settings → System → About* shows "System type". The installer
   refuses the other one.
2. Unzip it anywhere, open **PowerShell as administrator** (a 64-bit one; the
   Start menu's "Windows PowerShell" is), go to the folder and run:

   ```powershell
   Set-ExecutionPolicy -Scope Process Bypass
   .\install.ps1
   ```

   The first line only allows scripts in this window. To also have E-AC-3 and
   AC-3 decoded by it (Windows has its own decoder for them, which may still
   be chosen first), add `-WithDolbyDigital`.
3. Close and reopen the programs you play with. The ones already running do
   not see the new decoder.

The installer copies `harletty_mf.dll`, `mf.yaml`, `mf-config.yaml`, the
licence texts and the uninstaller to `C:\Program Files\harletty\mf\`,
registers the decoder for every user of the PC, and checks that Media
Foundation lists it for TrueHD. It prints what it did and how to undo it. Run
it again to upgrade; your edits to the two option files are kept.

One decoder is registered per PC: installing the x64 archive after the ARM64
one (or the reverse) replaces it.

## Uninstall

```powershell
& 'C:\Program Files\harletty\mf\uninstall.ps1'
```

(or `.\install.ps1 -Uninstall` from the unzipped folder). It unregisters the
decoder and deletes the install folder. Close the players first, or the DLL
is still in use and cannot be deleted; the script says so. Your log and
per-user options in `%LOCALAPPDATA%\harletty\mf\` stay unless you add `-Purge`.

## Configure

Two files, in the install folder (changes there apply to every user) or in
`%LOCALAPPDATA%\harletty\mf\` (this user only, and wins):

- **`mf.yaml`**: the decoder's own options: `codecs`, `speaker_mask`,
  `log_level`, `log_file`. Every key is explained in the file. The `codecs`
  list is read when the decoder is registered, so after changing it run
  `install.ps1` again. The rest is read when a player loads the decoder.
- **`mf-config.yaml`**: the rendering engine's settings, in the
  [Omniphony `config.yaml` schema](https://github.com/mgth/Omniphony): dynamic
  range control, which presentation to decode, and the like. The file shipped
  does little more than switch OSC off; everything else is Omniphony's
  default. This is the decoder's own file: Omniphony's shared config is not
  read.

A file that does not parse is ignored with a line in the log, not an error.

## Speakers

The decoder renders to the speakers your **default audio device** reports when
a file starts, so a 5.1 setup gets 5.1 and a 7.1 setup gets 7.1. Notes:

- The layout is fixed for the file being played. If you switch the device in
  the middle of a file, it takes effect on the next file.
- If Windows reports the wrong speakers, set `speaker_mask` in `mf.yaml`
  (`0x3F` is 5.1, `0x63F` is 7.1, in Windows' speaker masks).
- **Stereo (or mono) output is rendered as binaural, for headphones.** This is
  forced when the device has two speakers or fewer, because the speaker
  renderer needs at least three; your `mf-config.yaml` cannot turn it off.

## Logs

The decoder logs to `%LOCALAPPDATA%\harletty\mf\harletty-mf.log` (per user,
appended to). `log_level` in `mf.yaml` goes from `off` to `trace`; the default
is `info`. For a bug report, set `debug`, play the file until it fails, and
attach the log.

## Troubleshooting

**The player does not use it.** Check that Media Foundation knows it. In an
administrator PowerShell:

```powershell
Test-Path 'HKLM:\SOFTWARE\Classes\MediaFoundation\Transforms\{AC884ED4-9379-4E5A-BAF6-3365A4F08E38}'
```

must print `True`. Then check that the player really loaded it, while a TrueHD
or DTS file is playing:

```powershell
tasklist /m harletty_mf.dll
```

If the player's process is not listed, it did not pick the decoder: it is
probably not a program this decoder is for (see above), or it was already
running when you installed.

**Audio is missing or silent.** Look at the log. The common causes: the
player gave the decoder a codec it is not registered for (E-AC-3 without
`-WithDolbyDigital`), or `speaker_mask` is wrong for the device.

**The installer says the DLL could not be written or deleted.** A player still
has it loaded. Close every program that plays media, then run the script again.

**"This archive is for x64 processors and this machine is arm64."** Download
the other archive.

**Windows warns that the script is not signed / will not run.** Use the
`Set-ExecutionPolicy -Scope Process Bypass` line above; it affects only that
window.
