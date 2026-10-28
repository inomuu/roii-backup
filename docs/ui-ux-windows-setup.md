# Windows UI update — 2026-10-10

## Open the game

From PowerShell in this repository:

```powershell
& .\tools\play_windows.ps1
```

This opens the real zone scene at the normal title screen in Unity Play mode. Close this project's existing Unity window before launching again. This is an Editor launch, not a packaged Windows executable. Login and server verification remain required for normal online play.

## Changes

- Retained the navy/gold theme, raised caption contrast, and improved sign-in spacing and persistent field labels.
- Added visible keyboard focus, Tab/Shift-Tab navigation, Enter activation, validation focus, and busy-state input locking.
- Removed creation-screen logo overlap and made the initial connection status visible.
- Added localized labels and a Noto Sans Thai TMP fallback using the font already present in the source archive. The existing translation table is incomplete, so some Thai-screen text still falls back to English.
- Used the local UI/UX Pro Max skill guidance; no remote UI/UX API was connected.

## Local dependencies and compatibility

The source archive targeted Unity 6000.0.83f1. This working copy now uses the installed Unity **6000.6.3f1**, with its migrated project settings and package lock. Narrow compatibility changes cover Cubism animation IDs and three existing editor diagnostics/render-pass APIs. The original downloaded ZIP remains unchanged; original UI files are backed up in `D:\roii\ui-before`.

Restored DOTween 1.3.030 DLLs and the complete official Live2D Cubism SDK for Unity 5-r.5. Built the Windows x64 `sim_ffi.dll` from this archive's Rust sources; it reports simulation version `0.0.492-alpha`. The archive has no Git history, so its build stamp is unknown. Online compatibility and save verification have not been tested.

The launcher checks required DLLs and syncs the canonical translation CSV. Its optional `-RebuildBridge` switch requires Cargo on PATH and all Unity processes closed. The local toolchain used for this setup is under `D:\roii\dependencies\cargo` and `D:\roii\dependencies\rustup`.

Official dependency references: [DOTween](https://dotween.demigiant.com/download.php), [Live2D Cubism](https://www.live2d.com/en/sdk/download/unity/), [Noto Thai](https://notofonts.github.io/thai/). Licenses accompany the imported assets.

## Validation and remaining limitations

- Unity batch compile/import exited with code 0.
- Windows native FFI loaded and returned the expected version.
- Localization key guard passed (2171 keys).
- `TitleUxProbe.Run` passed all 11 behavioral checks. Screenshots inspected at 1280×720 and 1024×768; missing Thai glyphs were fixed and the probe rerun.
- The real scene reached `BOOT: door=(title screen)` and rendered the title screen.

Screenshots: `D:\roii\idle_mmorpg\ui-validation`. Compile log: `D:\roii\unity-ui-compile-6.log`. UI probe log: `D:\roii\title-ui-probe.log`. The launcher writes `unity-windows.log` in this repository.

The supplied source lacks environment prefab assets referenced by the scene, including Idyllic Fantasy Nature foliage/rocks, and some LeafTrigger scripts. Unity therefore reports missing prefab/script warnings. Restore the original asset package with its original GUIDs to repair the full scene; placeholders have not been substituted. A Cubism sample motion import also reported a null reference during initial SDK import, and Unity's search index reported an editor startup exception. Neither prevented UI tests or title-screen rendering; the full scene, Live2D motion playback, online login, and packaged-player build are not validated.

To rerun presentation checks with this Unity project open and Play mode stopped, put `exec:TitleUxProbe.Run` in `unity/ROI/.roi_cmd`. It exercises UI only, creates no account, and leaves a Thai sign-in preview visible. Stop Play mode before using `exec:WindowsPlay.Run` to return to the actual game.
