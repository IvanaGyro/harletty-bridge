<#
.SYNOPSIS
  Removes the harletty Media Foundation decoder.

.DESCRIPTION
  Unregisters the decoder, then deletes the files install.ps1 put in the
  install folder. Run it from an elevated, 64-bit PowerShell. The per-user
  folder %LOCALAPPDATA%\harletty\mf (log, options) stays unless -Purge is given.

.PARAMETER InstallDir
  Where it was installed. Default: %ProgramFiles%\harletty\mf.

.PARAMETER Purge
  Also delete %LOCALAPPDATA%\harletty\mf of the user running this.
#>
[CmdletBinding()]
param(
    [string]$InstallDir,
    [switch]$Purge
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'common.ps1')
if (-not $InstallDir) { $InstallDir = $script:DefaultDir }

Assert-Administrator
$target = Join-Path $InstallDir $script:DllName

if (Test-Path $target) {
    Invoke-Regsvr32 $target -Unregister
} else {
    # The file is gone but the registration may not be: remove the keys.
    Write-Host "$target is not there; removing the registration only."
    $mf = 'HKLM:\SOFTWARE\Microsoft\Windows Media Foundation\Transforms'
    foreach ($key in "HKLM:\SOFTWARE\Classes\CLSID\$($script:Clsid)", "$mf\$($script:Clsid)",
            "$mf\Categories\9ea73fb4-ef7a-4559-8d5d-719d8f0426c7\$($script:Clsid)") {
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $key
    }
}

if (Test-MftListed) {
    throw 'The decoder is still listed by Media Foundation after unregistering.'
}

if (Test-Path $InstallDir) {
    try {
        Remove-Item -Recurse -Force $InstallDir
    } catch {
        throw "Unregistered, but could not delete $InstallDir. Close every program that plays media and delete it. ($_)"
    }
}
$parent = Split-Path $InstallDir
if ((Test-Path $parent) -and -not (Get-ChildItem $parent -Force)) { Remove-Item $parent }

if ($Purge) {
    $user = Join-Path $env:LOCALAPPDATA 'harletty\mf'
    if (Test-Path $user) { Remove-Item -Recurse -Force $user; Write-Host "Deleted $user" }
}

Write-Host 'The decoder is unregistered and its files are deleted.'
if (-not $Purge) { Write-Host "Kept: $env:LOCALAPPDATA\harletty\mf (log, options). Use -Purge to delete it." }
Write-Host 'Programs that were playing must be restarted to stop using it.'
