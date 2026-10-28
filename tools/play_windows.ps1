param(
    [string]$UnityEditor = '',
    [switch]$RebuildBridge
)
$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$projectPath = Join-Path $repoRoot 'unity\ROI'
$requiredVersion = ((Get-Content (Join-Path $projectPath 'ProjectSettings\ProjectVersion.txt') |
    Select-String '^m_EditorVersion:').Line -split ':', 2)[1].Trim()
if (-not $UnityEditor) {
    $UnityEditor = Join-Path $env:ProgramFiles "Unity\Hub\Editor\$requiredVersion\Editor\Unity.exe"
}
if (-not (Test-Path -LiteralPath $UnityEditor)) {
    throw "Install Unity $requiredVersion or pass -UnityEditor with its Unity.exe path."
}
$plugin = Join-Path $projectPath 'Assets\Plugins\x86_64\sim_ffi.dll'
if ($RebuildBridge) {
    # Native plugins stay loaded for the entire Editor session. Rebuild with this project closed.
    if (Get-Process Unity -ErrorAction SilentlyContinue) {
        throw 'Close Unity before rebuilding the native simulation plugin.'
    }
    Push-Location $repoRoot
    try {
        & cargo build -p sim-ffi --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'Simulation build failed.' }
        New-Item -ItemType Directory -Path (Split-Path $plugin) -Force | Out-Null
        Copy-Item -LiteralPath (Join-Path $repoRoot 'target\release\sim_ffi.dll') -Destination $plugin
    } finally { Pop-Location }
}
$requiredFiles = @(
    $plugin,
    (Join-Path $projectPath 'Assets\Plugins\Demigiant\DOTween\DOTween.dll'),
    (Join-Path $projectPath 'Assets\Live2D\Cubism\Plugins\Windows\x86_64\Live2DCubismCore.dll')
)
foreach ($file in $requiredFiles) {
    if (-not (Test-Path -LiteralPath $file)) { throw "Missing dependency: $file" }
}
Copy-Item -LiteralPath (Join-Path $repoRoot 'godot\i18n\strings.csv') -Destination (Join-Path $projectPath 'Assets\StreamingAssets\strings.csv')
$log = Join-Path $repoRoot 'unity-windows.log'
Start-Process -FilePath $UnityEditor -ArgumentList @(
    '-projectPath', ('"' + $projectPath + '"'),
    '-executeMethod', 'WindowsPlay.Run',
    '-logFile', ('"' + $log + '"')
)
Write-Output "Opening ROI in Unity. Log: $log"
