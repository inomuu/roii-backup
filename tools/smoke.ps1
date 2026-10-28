# smoke.ps1 - end-to-end "is it broken?" check.
#
# Steps:
#   1. cargo test --workspace         (sim-core unit + golden tests)
#   2. cargo build -p godot-ext       (bridge DLL)
#   3. copy target/debug/godot_ext.dll into godot/bin/
#   4. godot --headless --path godot --quit-after N
#      (catches GDScript parse errors, scene wiring, runtime errors
#       from at least the first N frames of sim ticks)
#
# Exits non-zero on any failure so this can wire into CI / pre-commit.
#
# Usage:
#   .\tools\smoke.ps1
#   .\tools\smoke.ps1 -SkipGodot
#   .\tools\smoke.ps1 -GodotPath "C:\path\to\godot.exe"
#   .\tools\smoke.ps1 -RunSeconds 10

[CmdletBinding()]
param(
    [switch]$SkipGodot,
    [string]$GodotPath = "",
    [int]$RunSeconds = 5
)

$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

function Show-Section($name) {
    Write-Host ""
    Write-Host ("==== " + $name + " ====") -ForegroundColor Cyan
}

function Show-Pass($name, $elapsed) {
    Write-Host ("[PASS] {0,-32} {1,6:N2}s" -f $name, $elapsed) -ForegroundColor Green
}

function Show-Fail($name, $elapsed) {
    Write-Host ("[FAIL] {0,-32} {1,6:N2}s" -f $name, $elapsed) -ForegroundColor Red
}

function Invoke-Step($name, [ScriptBlock]$step) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        & $step
        $sw.Stop()
        Show-Pass $name $sw.Elapsed.TotalSeconds
        return $true
    }
    catch {
        $sw.Stop()
        Show-Fail $name $sw.Elapsed.TotalSeconds
        Write-Host $_ -ForegroundColor Red
        return $false
    }
}

function Find-Godot {
    if ($GodotPath -and (Test-Path $GodotPath)) { return $GodotPath }
    $candidates = @(
        "E:\godot\Godot_v4.6.3-stable_win64_console.exe",
        "E:\godot\Godot_v4.6.3-stable_win64.exe",
        "$env:USERPROFILE\Downloads\godot-bin\Godot_v4.5-stable_win64.exe"
    )
    foreach ($c in $candidates) {
        if (Test-Path $c) { return $c }
    }
    $cmd = Get-Command godot -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    return $null
}

function Invoke-GodotSmoke($exe, $root, $seconds) {
    $proj = Join-Path $root "godot"
    $frames = [int]($seconds * 60)
    return Invoke-Step "godot smoke run" {
        $out = & $exe --headless --path $proj --quit-after $frames 2>&1 | Out-String
        $hits = ($out -split "`n") | Select-String "BETA|ALPHA|ERROR|Parse Error"
        foreach ($line in $hits) {
            $color = "DarkGray"
            if ("$line" -match "ERROR|Parse Error") { $color = "Red" }
            Write-Host "       $line" -ForegroundColor $color
        }
        if ($out -match "SCRIPT ERROR|Parse Error") {
            throw "Godot reported script errors (see above)"
        }
        if ($LASTEXITCODE -ne 0) {
            Write-Host $out
            throw "godot exit $LASTEXITCODE"
        }
    }
}

# --- Locate cargo ---------------------------------------------------

$cargoExe = "$env:USERPROFILE\.cargo\bin\cargo.exe"
if (-not (Test-Path $cargoExe)) {
    $cargoCmd = Get-Command cargo -ErrorAction SilentlyContinue
    if ($cargoCmd) { $cargoExe = $cargoCmd.Source }
}
if (-not (Test-Path $cargoExe)) {
    Write-Host "cargo not found - install rustup first." -ForegroundColor Red
    exit 2
}

$allPass = $true

# --- 1. cargo test --------------------------------------------------

Show-Section "1. cargo test --workspace"
$ok = Invoke-Step "cargo test" {
    # No 2>&1: PS 5.1 wraps native stderr as ErrorRecord and breaks
    # try/catch. cargo's test results land on stdout anyway; stderr
    # carries only the noisy "Compiling/Finished" progress that we
    # let stream straight to the terminal.
    $out = & $cargoExe test --workspace | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "cargo test exit $LASTEXITCODE"
    }
    ($out -split "`n") | Select-String "test result:" | ForEach-Object {
        Write-Host "       $_" -ForegroundColor DarkGray
    }
}
if (-not $ok) { $allPass = $false }

# --- 2. cargo build -------------------------------------------------

Show-Section "2. cargo build -p godot-ext"
$ok = Invoke-Step "cargo build" {
    & $cargoExe build -p godot-ext | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build exit $LASTEXITCODE"
    }
}
if (-not $ok) { $allPass = $false }

# --- 3. copy DLL ----------------------------------------------------

Show-Section "3. copy DLL"
$ok = Invoke-Step "copy godot_ext.dll" {
    $src = Join-Path $repoRoot "target\debug\godot_ext.dll"
    $dst = Join-Path $repoRoot "godot\bin\godot_ext.dll"
    if (-not (Test-Path $src)) { throw "no source DLL at $src" }
    Copy-Item $src $dst -Force
    $mb = [math]::Round((Get-Item $dst).Length / 1048576, 2)
    Write-Host "       godot\bin\godot_ext.dll  $mb MiB" -ForegroundColor DarkGray
}
if (-not $ok) { $allPass = $false }

# --- 4. godot --headless --------------------------------------------

Show-Section "4. godot --headless"
if ($SkipGodot) {
    Write-Host "       SKIPPED (-SkipGodot)" -ForegroundColor DarkGray
} else {
    $godotExe = Find-Godot
    if (-not $godotExe) {
        Write-Host "no Godot binary found. Pass -GodotPath or use -SkipGodot." -ForegroundColor Yellow
    } else {
        Write-Host ("       binary: " + $godotExe) -ForegroundColor DarkGray
        $ok = Invoke-GodotSmoke $godotExe $repoRoot $RunSeconds
        if (-not $ok) { $allPass = $false }
    }
}

Write-Host ""
if ($allPass) {
    Write-Host "==== smoke: PASS ====" -ForegroundColor Green
    exit 0
} else {
    Write-Host "==== smoke: FAIL ====" -ForegroundColor Red
    exit 1
}
