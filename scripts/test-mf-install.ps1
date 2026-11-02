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
Check (Test-Path "$classes\MediaFoundation\Transforms\$clsid") 'the MFT is registered'

# A second run keeps the user's edits to the option files and still works.
Add-Content (Join-Path $installDir 'mf.yaml') '# user edit'
& (Join-Path $work 'install.ps1')
Check ((Get-Content (Join-Path $installDir 'mf.yaml')) -contains '# user edit') 'installing again keeps the options file'
Check (Test-Path "$classes\MediaFoundation\Transforms\$clsid") 'the MFT is registered after installing again'

# Ask for the per-user folder too, so -Purge has something to delete.
$user = Join-Path $env:LOCALAPPDATA 'harletty\mf'
New-Item -ItemType Directory -Force $user | Out-Null
Set-Content (Join-Path $user 'harletty-mf.log') 'x'
& (Join-Path $installDir 'uninstall.ps1')
Check (-not (Test-Path $installDir)) 'the install folder is gone'
Check (-not (Test-Path "$classes\CLSID\$clsid")) 'the COM class is gone'
Check (-not (Test-Path "$classes\MediaFoundation\Transforms\$clsid")) 'the MFT registration is gone'
Check (Test-Path (Join-Path $user 'harletty-mf.log')) 'the per-user folder is kept without -Purge'

# Again, and the same through install.ps1 -Uninstall -Purge.
& (Join-Path $work 'install.ps1')
& (Join-Path $work 'install.ps1') -Uninstall -Purge
Check (-not (Test-Path $installDir)) 'install.ps1 -Uninstall: the install folder is gone'
Check (-not (Test-Path "$classes\MediaFoundation\Transforms\$clsid")) 'install.ps1 -Uninstall: the MFT registration is gone'
Check (-not (Test-Path $user)) '-Purge deleted the per-user folder'

Remove-Item -Recurse -Force $work
Write-Host 'install / uninstall round trip passed'
