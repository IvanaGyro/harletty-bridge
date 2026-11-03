# Install the decoder from its archive on this machine, check it, uninstall it,
# check it is gone. Needs an administrator and a 64-bit PowerShell; used by CI.
#   pwsh scripts/test-mf-install.ps1 -Zip harletty-mf-<version>-windows-<arch>.zip
param([Parameter(Mandatory)][string]$Zip)
$ErrorActionPreference = 'Stop'

$clsid = '{AC884ED4-9379-4E5A-BAF6-3365A4F08E38}'
$classes = 'HKLM:\SOFTWARE\Classes'
$installDir = Join-Path $env:ProgramFiles 'harletty\mf'
$work = Join-Path ([IO.Path]::GetTempPath()) "harletty-mf-install-$PID"
Expand-Archive -Path $Zip -DestinationPath $work
Write-Host "Archive holds: $((Get-ChildItem $work -Recurse -File | ForEach-Object { $_.Name }) -join ', ')"

# Media Foundation's own answer, not a guess at where it keeps its registry keys.
. (Join-Path $work 'common.ps1')

function Check([bool]$ok, [string]$what) {
    if (-not $ok) { throw "FAILED: $what" }
    Write-Host "ok: $what"
}

# -WithDolbyDigital is the longer path (it edits mf.yaml before registering).
& (Join-Path $work 'install.ps1') -WithDolbyDigital
Check (Test-Path (Join-Path $installDir 'harletty_mf.dll')) 'the DLL is in the install folder'
foreach ($name in 'mf.yaml', 'mf-config.yaml', 'uninstall.ps1', 'common.ps1', 'licences\LICENSE.GPL-3.0') {
    Check (Test-Path (Join-Path $installDir $name)) "$name is installed"
}
Check ((Get-Content (Join-Path $installDir 'mf.yaml')) -contains 'codecs: [truehd, dts, eac3, ac3]') '-WithDolbyDigital wrote the codec list'
Check (Test-Path "$classes\CLSID\$clsid\InprocServer32") 'the COM class is registered'
Check (Test-MftListed) 'Media Foundation lists the decoder'

# A second run keeps the user's edits to the option files and still works.
Add-Content (Join-Path $installDir 'mf.yaml') '# user edit'
& (Join-Path $work 'install.ps1')
Check ((Get-Content (Join-Path $installDir 'mf.yaml')) -contains '# user edit') 'installing again keeps the options file'
Check (Test-MftListed) 'Media Foundation lists the decoder after installing again'

# A folder that was somebody's: a codec subset (the check must follow the
# enabled codecs), a block-style list that -WithDolbyDigital must replace
# whole, and a file of someone else's that uninstalling must leave alone.
$custom = Join-Path ([IO.Path]::GetTempPath()) "harletty-mf-custom-$PID"
New-Item -ItemType Directory -Force $custom | Out-Null
Set-Content (Join-Path $custom 'keep.txt') 'not ours'
Set-Content (Join-Path $custom 'mf.yaml') @('codecs: [dts]', 'log_level: debug')
& (Join-Path $work 'install.ps1') -InstallDir $custom
Check (Test-MftListed) 'a DTS-only codec list installs and is listed'
Set-Content (Join-Path $custom 'mf.yaml') @('speaker_mask: 0x3F', 'codecs:', '  - truehd', '  - dts', 'log_level: debug')
& (Join-Path $work 'install.ps1') -InstallDir $custom -WithDolbyDigital
$yaml = Get-Content (Join-Path $custom 'mf.yaml')
Check (($yaml -match '^codecs:').Count -eq 1) 'one codecs line after -WithDolbyDigital'
Check (-not ($yaml -match '^\s+- ')) 'the block list entries are gone'
Check (($yaml -contains 'speaker_mask: 0x3F') -and ($yaml -contains 'log_level: debug')) 'the other keys are kept'
& (Join-Path $custom 'uninstall.ps1') -InstallDir $custom
Check (-not (Test-MftListed)) 'uninstalling the custom folder unregisters the decoder'
Check (Test-Path (Join-Path $custom 'keep.txt')) "a file that was not ours is still there"
Check (-not (Test-Path (Join-Path $custom 'harletty_mf.dll'))) 'our DLL is gone from the custom folder'
Remove-Item -Recurse -Force $custom

# Ask for the per-user folder too, so -Purge has something to delete.
$user = Join-Path $env:LOCALAPPDATA 'harletty\mf'
New-Item -ItemType Directory -Force $user | Out-Null
Set-Content (Join-Path $user 'harletty-mf.log') 'x'
& (Join-Path $installDir 'uninstall.ps1')
Check (-not (Test-Path $installDir)) 'the install folder is gone'
Check (-not (Test-Path (Split-Path $installDir))) 'the harletty folder in Program Files is gone too'
Check (-not (Test-Path "$classes\CLSID\$clsid")) 'the COM class is gone'
Check (-not (Test-MftListed)) 'Media Foundation no longer lists the decoder'
Check (Test-Path (Join-Path $user 'harletty-mf.log')) 'the per-user folder is kept without -Purge'

# Again, and the same through install.ps1 -Uninstall -Purge.
& (Join-Path $work 'install.ps1')
& (Join-Path $work 'install.ps1') -Uninstall -Purge
Check (-not (Test-Path $installDir)) 'install.ps1 -Uninstall: the install folder is gone'
Check (-not (Test-MftListed)) 'install.ps1 -Uninstall: Media Foundation no longer lists the decoder'
Check (-not (Test-Path $user)) '-Purge deleted the per-user folder'
Check (-not (Test-Path (Split-Path $user))) '-Purge deleted the empty harletty folder under LocalAppData'

Remove-Item -Recurse -Force $work
Write-Host 'install / uninstall round trip passed'
