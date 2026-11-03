<#
.SYNOPSIS
  Phase 6, test 3': a blind, level-matched preference test between the stream's
  own 7.1 mix and the engine's render of the Atmos stream
  (docs/plan-media-foundation-decoder.md, "Still to measure").

.DESCRIPTION
  Conditions, all through the installed decoder and the default render endpoint:
    R  presentation 2 at 7.1    (the reference)
    A  presentation 3 at 7.1
    B  presentation 3 at 7.1.4
  1. Calibration: each condition's passage is played with `--measure`, which
     records a WASAPI loopback of what reaches the headphones. A and B get a
     `render.master_gain` that puts their level within -ToleranceDb of R's.
  2. Two blind pair tests, R against A and R against B, -PerPassage trials per
     passage each. The two are played in random order and you say which you
     would rather listen to. Nothing is revealed until the end.

  Needs: the decoder installed, a PowerShell with write access to
  %LOCALAPPDATA%\harletty\mf, a probe built with --measure, and nothing else
  playing sound. Your own mf.yaml / mf-config.yaml in that folder are put back
  at the end.

.EXAMPLE
  .\blind-test-3prime.ps1 -Probe C:\tools\mf-player-probe.exe -File 'C:\Users\me\Downloads\Atmos.mkv'
#>
param(
    [Parameter(Mandatory)][string]$Probe,
    [Parameter(Mandatory)][string]$File,
    [int[]]$Passages = @(315, 555),
    [int]$Seconds = 10,
    [int]$PerPassage = 6,
    [double]$ToleranceDb = 0.5
)
$ErrorActionPreference = 'Stop'

$dir = Join-Path $env:LOCALAPPDATA 'harletty\mf'
$yaml = Join-Path $dir 'mf.yaml'
$cfg = Join-Path $dir 'mf-config.yaml'
$log = Join-Path $dir 'harletty-mf.log'
New-Item -ItemType Directory -Force $dir | Out-Null

$conditions = @{
    R = @{ Pres = 2; Mask = '0x63F';   Channels = 8 }
    A = @{ Pres = 3; Mask = '0x63F';   Channels = 8 }
    B = @{ Pres = 3; Mask = '0x2D63F'; Channels = 12 }
}

function Set-Condition([string]$name, [double]$gain) {
    $c = $conditions[$name]
    Set-Content $yaml @('log_level: debug', "speaker_mask: $($c.Mask)")
    Set-Content $cfg @('render:', '  osc: false', "  presentation: $($c.Pres)",
        ('  master_gain: ' + $gain.ToString('0.0', [Globalization.CultureInfo]::InvariantCulture)))
}

function Invoke-Probe([int]$passage, [switch]$Measure) {
    Remove-Item $log -ErrorAction SilentlyContinue
    $extra = if ($Measure) { @('--measure') } else { @() }
    $out = & $Probe $File --start $passage --for $Seconds @extra 2>&1
    Start-Sleep -Milliseconds 500
    return ($out | Out-String)
}

function Get-Level([string]$name, [double]$gain, [int]$passage) {
    Set-Condition $name $gain
    $out = Invoke-Probe $passage -Measure
    if ($out -match 'loopback: total (-?\d+(\.\d+)?) dBFS') {
        return [double]::Parse($Matches[1], [Globalization.CultureInfo]::InvariantCulture)
    }
    throw "No loopback level in the probe's output:`n$out"
}

function Get-Played {
    $text = if (Test-Path $log) { Get-Content $log } else { @() }
    $outLine = $text | Where-Object { $_ -match 'output: (\d+) channels' } | Select-Object -Last 1
    $presLine = $text | Where-Object { $_ -match 'presentation' } | Select-Object -Last 1
    [pscustomobject]@{ Channels = if ($outLine -match 'output: (\d+) channels') { [int]$Matches[1] } else { 0 }; Presentation = $presLine }
}

function Get-PValue([int]$k, [int]$n) {   # one-sided, p = 0.5
    $sum = 0.0
    for ($i = $k; $i -le $n; $i++) {
        $c = 1.0; for ($j = 1; $j -le $i; $j++) { $c = $c * ($n - $i + $j) / $j }
        $sum += $c
    }
    return $sum / [math]::Pow(2, $n)
}

# Keep what was there.
$backups = @{}
foreach ($f in $yaml, $cfg) { if (Test-Path $f) { $backups[$f] = Get-Content $f -Raw } }
$oldLog = $env:HARLETTY_LOG
$env:HARLETTY_LOG = 'debug'
try {
    # --- 1. calibration ---------------------------------------------------
    Write-Host 'Calibrating levels. Close every other program that makes sound.'
    Read-Host 'Put the headphones on at your usual volume. Press Enter'
    $gain = @{}
    $calibration = @()
    foreach ($p in $Passages) {
        $ref = Get-Level 'R' 0 $p
        foreach ($name in 'A', 'B') {
            $g = 0.0
            $level = $null
            for ($try = 1; $try -le 4; $try++) {
                $level = Get-Level $name $g $p
                $diff = $ref - $level
                if ([math]::Abs($diff) -le $ToleranceDb) { break }
                $g = [math]::Round($g + $diff, 1)
            }
            if ([math]::Abs($ref - $level) -gt $ToleranceDb) {
                throw "Could not match $name to R at $p s within $ToleranceDb dB (R $ref, $name $level). Is another program playing?"
            }
            $gain["$name/$p"] = $g
            $calibration += [pscustomobject]@{ Passage = $p; Condition = $name; GainDb = $g; LevelDb = $level; ReferenceDb = $ref }
        }
        $gain["R/$p"] = 0.0
    }
    $calibration | Format-Table -AutoSize | Out-String | Write-Host

    # --- 2. the blind trials ----------------------------------------------
    $specs = foreach ($pair in 'A', 'B') { foreach ($p in $Passages) { 1..$PerPassage | ForEach-Object { [pscustomobject]@{ Pair = $pair; Passage = $p } } } }
    $specs = $specs | Sort-Object { Get-Random }
    $results = @()
    $n = 0
    Read-Host 'Calibration done. The test starts now: for each trial you hear two versions of the same passage. Press Enter'
    foreach ($spec in $specs) {
        $n++
        $first, $second = if ((Get-Random -Maximum 2) -eq 0) { 'R', $spec.Pair } else { $spec.Pair, 'R' }
        $answer = 'r'
        while ($answer -eq 'r') {
            Read-Host "Trial $n of $($specs.Count). Press Enter to hear the first version"
            Set-Condition $first $gain["$first/$($spec.Passage)"]
            Invoke-Probe $spec.Passage | Out-Null
            $playedFirst = Get-Played
            Start-Sleep -Milliseconds 500
            Set-Condition $second $gain["$second/$($spec.Passage)"]
            Invoke-Probe $spec.Passage | Out-Null
            $playedSecond = Get-Played
            $answer = Read-Host 'Which would you rather listen to?  1 = first   2 = second   r = replay both'
        }
        $chosen = if ($answer -eq '1') { $first } else { $second }
        $okChannels = ($playedFirst.Channels -eq $conditions[$first].Channels) -and ($playedSecond.Channels -eq $conditions[$second].Channels)
        $results += [pscustomobject]@{
            Trial = $n; Pair = "R vs $($spec.Pair)"; Passage = $spec.Passage; First = $first; Second = $second
            Preferred = $chosen; ChannelsAsExpected = $okChannels
            FirstLog = $playedFirst.Presentation; SecondLog = $playedSecond.Presentation
        }
    }
}
finally {
    foreach ($f in $yaml, $cfg) {
        if ($backups.ContainsKey($f)) { Set-Content $f $backups[$f] -NoNewline } else { Remove-Item $f -ErrorAction SilentlyContinue }
    }
    $env:HARLETTY_LOG = $oldLog
}

# --- 3. results ---------------------------------------------------------
$results | Format-Table Trial, Pair, Passage, First, Second, Preferred, ChannelsAsExpected -AutoSize
foreach ($pair in 'A', 'B') {
    $trials = @($results | Where-Object { $_.Pair -eq "R vs $pair" })
    $wins = @($trials | Where-Object { $_.Preferred -eq $pair }).Count
    '{0}: preferred over R in {1} of {2} trials (one-sided p = {3:N3})' -f $pair, $wins, $trials.Count, (Get-PValue $wins $trials.Count)
}
'Trials whose decoder log did not show the expected channel count: {0}' -f @($results | Where-Object { -not $_.ChannelsAsExpected }).Count
'The plan keeps object rendering if A or B is preferred in 10 or more of 12 trials.'
