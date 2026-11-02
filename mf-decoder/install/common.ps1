# Shared by install.ps1 and uninstall.ps1 (dot-sourced; not run on its own).

$script:Clsid = '{AC884ED4-9379-4E5A-BAF6-3365A4F08E38}'
$script:DllName = 'harletty_mf.dll'
$script:DefaultDir = Join-Path $env:ProgramFiles 'harletty\mf'

function Assert-Administrator {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not (New-Object Security.Principal.WindowsPrincipal $id).IsInRole(
            [Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Run this from an elevated (administrator) PowerShell.'
    }
    # A 32-bit PowerShell would register through the 32-bit regsvr32 and
    # registry view, which no 64-bit player reads.
    if (-not [Environment]::Is64BitProcess) {
        throw 'Run this from a 64-bit PowerShell (not "Windows PowerShell (x86)").'
    }
}

function Get-Regsvr32 { Join-Path $env:SystemRoot 'System32\regsvr32.exe' }

# 'x64' or 'arm64', from the DLL's PE header.
function Get-DllArchitecture([string]$Path) {
    $bytes = [IO.File]::ReadAllBytes($Path)
    $peOffset = [BitConverter]::ToInt32($bytes, 0x3C)
    switch ([BitConverter]::ToUInt16($bytes, $peOffset + 4)) {
        0x8664 { 'x64' }
        0xAA64 { 'arm64' }
        default { 'other' }
    }
}

function Get-OsArchitecture {
    switch ([Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()) {
        'X64' { 'x64' }
        'Arm64' { 'arm64' }
        default { 'other' }
    }
}

# regsvr32 /s, with its exit code turned into a message.
function Invoke-Regsvr32([string]$Dll, [switch]$Unregister) {
    # An array even for one flag: a lone string would be glued to the path.
    $arguments = @('/s')
    if ($Unregister) { $arguments += '/u' }
    $arguments += "`"$Dll`""
    $p = Start-Process -FilePath (Get-Regsvr32) -ArgumentList $arguments -Wait -PassThru
    if ($p.ExitCode -ne 0) {
        $verb = if ($Unregister) { 'unregister' } else { 'register' }
        throw "regsvr32 could not $verb $Dll (exit code $($p.ExitCode))."
    }
}

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public static class HarlettyMft {
    [ComImport, Guid("2CD2D921-C447-44A7-A13C-4ADABFC247E3"),
     InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IMFAttributes {
        void GetItem(); void GetItemType(); void CompareItem(); void Compare();
        void GetUINT32(); void GetUINT64(); void GetDouble();
        [PreserveSig] int GetGUID(ref Guid key, out Guid value);
    }

    [StructLayout(LayoutKind.Sequential)]
    struct TypeInfo { public Guid Major; public Guid Sub; }

    [DllImport("mfplat.dll")] static extern int MFStartup(uint version, uint flags);
    [DllImport("mfplat.dll")] static extern int MFShutdown();
    [DllImport("mfplat.dll")]
    static extern int MFTEnumEx(Guid category, uint flags, ref TypeInfo input,
        IntPtr output, out IntPtr activates, out uint count);

    // The CLSIDs of the audio decoders MFTEnumEx lists for `subtype`.
    public static string[] Decoders(Guid subtype) {
        var found = new List<string>();
        int hr = MFStartup(0x00020070, 0);
        if (hr != 0) Marshal.ThrowExceptionForHR(hr);
        try {
            var category = new Guid("9ea73fb4-ef7a-4559-8d5d-719d8f0426c7"); // audio decoder
            var input = new TypeInfo {
                Major = new Guid("73647561-0000-0010-8000-00AA00389B71"),  // audio
                Sub = subtype
            };
            IntPtr array; uint count;
            hr = MFTEnumEx(category, 0x1F, ref input, IntPtr.Zero, out array, out count);
            if (hr != 0) Marshal.ThrowExceptionForHR(hr);
            for (int i = 0; i < count; i++) {
                IntPtr unk = Marshal.ReadIntPtr(array, i * IntPtr.Size);
                if (unk == IntPtr.Zero) continue;
                var attributes = (IMFAttributes)Marshal.GetObjectForIUnknown(unk);
                var key = new Guid("6821C42B-65A4-4E82-99BC-9A88205ECD0C"); // MFT_TRANSFORM_CLSID_Attribute
                Guid clsid;
                if (attributes.GetGUID(ref key, out clsid) == 0) found.Add(clsid.ToString("B").ToUpper());
                Marshal.ReleaseComObject(attributes);
                Marshal.Release(unk);
            }
            Marshal.FreeCoTaskMem(array);
        } finally {
            MFShutdown();
        }
        return found.ToArray();
    }
}
'@

# TrueHD, as the decoder registers it.
$script:TrueHdSubtype = [Guid]'EB27CEC4-163E-4CA3-8B74-8E25F91B517E'

# Whether Media Foundation lists this decoder for TrueHD.
function Test-MftListed {
    [HarlettyMft]::Decoders($script:TrueHdSubtype) -contains $script:Clsid
}
