<#
.SYNOPSIS
  Installs the harletty Media Foundation decoder (TrueHD, DTS, DTS-HD, DTS:X).

.DESCRIPTION
  Copies harletty_mf.dll, the option files and the licence texts to
  %ProgramFiles%\harletty\mf, and registers the decoder with Windows
  (regsvr32), for every user of the machine. Run it from an elevated,
  64-bit PowerShell, from the folder of the archive that matches your
  processor (x64 or ARM64).

  Registers one decoder per machine: installing the x64 archive over the ARM64
  one (or the reverse) replaces it. Edits to mf.yaml and mf-config.yaml in the
  install folder are kept when this is run again.

.PARAMETER WithDolbyDigital
  Also register E-AC-3 and AC-3 (off by default: Windows has its own decoder
  for them, and it may be chosen first).

.PARAMETER InstallDir
  Where to install. Default: %ProgramFiles%\harletty\mf.

.PARAMETER Uninstall
  Same as running uninstall.ps1.

.PARAMETER Purge
  With -Uninstall: also delete the per-user folder %LOCALAPPDATA%\harletty\mf
  (the log, and options kept there).
#>
[CmdletBinding()]
param(
    [switch]$WithDolbyDigital,
    [string]$InstallDir,
    [switch]$Uninstall,
    [switch]$Purge
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'common.ps1')
if (-not $InstallDir) { $InstallDir = $script:DefaultDir }

if ($Uninstall) {
    & (Join-Path $PSScriptRoot 'uninstall.ps1') -InstallDir $InstallDir -Purge:$Purge
    return
}

Assert-Administrator
$source = Join-Path $PSScriptRoot $script:DllName
if (-not (Test-Path $source)) { throw "$($script:DllName) is not next to install.ps1." }

$dllArch = Get-DllArchitecture $source
$osArch = Get-OsArchitecture
if ($dllArch -ne $osArch) {
    throw "This archive is for $dllArch processors and this machine is $osArch. Use the $osArch archive."
}

$target = Join-Path $InstallDir $script:DllName
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

# An earlier install: take it out of the registry first, so the copy below
# cannot leave a registration pointing at a half-written file.
if (Test-Path $target) {
    Write-Host 'Replacing the installed decoder.'
    Invoke-Regsvr32 $target -Unregister
}

try {
    Copy-Item -Force $source $target
} catch {
    throw "Could not write $target. Close every program that plays media (Windows Media Player, your video player) and run this again. ($_)"
}
foreach ($name in 'common.ps1', 'uninstall.ps1') {
    Copy-Item -Force (Join-Path $PSScriptRoot $name) (Join-Path $InstallDir $name)
}
# The option files are the user's once installed: an existing copy stays.
foreach ($name in 'mf.yaml', 'mf-config.yaml') {
    $dest = Join-Path $InstallDir $name
    if (-not (Test-Path $dest)) { Copy-Item (Join-Path $PSScriptRoot $name) $dest }
}
$licences = Join-Path $InstallDir 'licences'
New-Item -ItemType Directory -Force -Path $licences | Out-Null
Get-ChildItem $PSScriptRoot -File |
    Where-Object { $_.Name -match '^(LICENSE|LICENSES|NOTICE)' } |
    Copy-Item -Destination $licences -Force

if ($WithDolbyDigital) {
    $options = Join-Path $InstallDir 'mf.yaml'
    $list = 'codecs: [truehd, dts, eac3, ac3]'
    # Replace the whole value of `codecs:`: its first line, and the indented
    # lines of a block list or a wrapped flow list under it.
    $out = @()
    $found = $false
    $skipping = $false
    foreach ($line in @(Get-Content $options)) {
        if ($line -match '^codecs:') {
            $out += $list
            $found = $true
            $skipping = $true
        } elseif ($skipping -and $line -match '^\s+\S' -and $line -notmatch '^\s*#') {
            continue
        } else {
            $skipping = $false
            $out += $line
        }
    }
    if (-not $found) { $out += '', $list }
    Set-Content -Path $options -Value $out -Encoding UTF8
    Write-Host 'E-AC-3 and AC-3 are switched on in mf.yaml.'
}

# The codec list in mf.yaml is read here, when the decoder registers itself.
Invoke-Regsvr32 $target

if (-not (Test-MftListed)) {
    throw 'The decoder is registered, but Media Foundation does not list it for any codec. Check codecs: in mf.yaml.'
}

Write-Host ''
Write-Host "Installed: $target"
Write-Host 'Media Foundation lists it as a decoder.'
Write-Host "Options:   $(Join-Path $InstallDir 'mf.yaml') and mf-config.yaml (same folder)"
Write-Host "Log:       $env:LOCALAPPDATA\harletty\mf\harletty-mf.log (per user)"
Write-Host 'Programs that are already playing must be restarted to see the decoder.'
Write-Host ''
Write-Host 'To undo:'
$uninstall = "& '$(Join-Path $InstallDir 'uninstall.ps1')'"
if ($InstallDir -ne $script:DefaultDir) { $uninstall += " -InstallDir '$InstallDir'" }
Write-Host "  $uninstall"
