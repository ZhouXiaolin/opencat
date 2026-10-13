# Render with a memory gate + peak-memory measurement.
#
# Wraps target/release/opencat.exe, polls the process every PollMs, records the
# peak private bytes, and kills the process the moment it crosses GateGB
# (suspected leak). Prints KEY=VALUE lines the caller can parse:
#   WALL_SECS=..., PEAK_MB=..., EXIT=<code|GATE_KILLED>
# followed by the child's "Render profile:" section (stderr) when present.
#
# Usage:
#   powershell -NoProfile -File tools/render-memgate.ps1 `
#     -Exe target/release/opencat.exe `
#     -Jsx examples/k3-promo/k3-promo.jsx -Out out/foo.mp4 `
#     [-GateGB 5.0] [-PollMs 500] [-StdoutLog out/foo.stdout.log] [-StderrLog out/foo.stderr.log]
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Jsx,
    [Parameter(Mandatory = $true)][string]$Out,
    [double]$GateGB = 5.0,
    [int]$PollMs = 500,
    [string]$StdoutLog = "",
    [string]$StderrLog = "",
    [string]$CurveLog = ""
)

$ErrorActionPreference = "Stop"
$gateBytes = [double]$GateGB * 1073741824.0

if ($StdoutLog -eq "") { $StdoutLog = [System.IO.Path]::GetTempFileName() }
if ($StderrLog -eq "") { $StderrLog = [System.IO.Path]::GetTempFileName() }

$sw = [System.Diagnostics.Stopwatch]::StartNew()
$proc = Start-Process -FilePath (Resolve-Path $Exe).Path `
    -ArgumentList $Jsx, $Out `
    -PassThru -NoNewWindow `
    -RedirectStandardOutput $StdoutLog -RedirectStandardError $StderrLog

$peakBytes = [double]0
$gateHit = $false
while (-not $proc.HasExited) {
    Start-Sleep -Milliseconds $PollMs
    if ($proc.HasExited) { break }
    try {
        $proc.Refresh()
        $cur = $proc.PrivateMemorySize64
        if ($cur -gt $peakBytes) { $peakBytes = $cur }
        if ($CurveLog -ne "") {
            Add-Content -Path $CurveLog -Value ("{0:F1},{1:F1}" -f $sw.Elapsed.TotalSeconds, ($cur / 1048576.0))
        }
        if ($cur -gt $gateBytes) {
            $gateHit = $true
            Write-Host ("MEMGATE: private bytes {0:N0} exceeded gate {1:N0} - killing process (suspected leak)" -f $cur, $gateBytes)
            Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
            break
        }
    } catch {
        break  # process died between the HasExited check and the read
    }
}
$proc.WaitForExit()
$sw.Stop()

$exitLabel = "$($proc.ExitCode)"
if ($gateHit) { $exitLabel = "GATE_KILLED" }

$peakMB = [math]::Round($peakBytes / 1048576.0, 1)
"WALL_SECS=" + [math]::Round($sw.Elapsed.TotalSeconds, 1)
"PEAK_MB=" + $peakMB
"EXIT=" + $exitLabel

# Surface the child's "Render profile:" summary (it prints to stderr).
if (Test-Path $StderrLog) {
    $err = Get-Content $StderrLog -Raw
    $idx = $err.IndexOf("Render profile:")
    if ($idx -ge 0) {
        ""
        $err.Substring($idx)
    }
}
