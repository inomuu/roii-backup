# Landscape mobile title screen — 2026-10-27

The approved composition is recorded in `approved-concept.png`: centered title, server selection and Start Game beneath it, four utility actions at the right, restrained dark fantasy art. Runtime labels and controls are live Unity UI, not baked into the background.

## Implementation

- `TitleScreen.cs` retains the existing account, Steam/development-route, busy-state and starter-party behavior. Start Game and Account explicitly open the account form; tapping scenery no longer opens it.
- `TitleScreen.Landing.cs` owns the landing layout and modal utilities. Settings apply the existing music, SFX and frame-rate preferences. News describes the installed client's UI update. Help copies only the version, configured host and platform. No account credentials are included.
- Server selection contains the configured server only. It does not invent shards, population, ping or availability. The production endpoint and existing authentication requirements remain unchanged.
- `TitleGlyph.cs` renders the line icons using static, antialiased UI geometry.
- Safe-area anchors use `Screen.safeArea`; the background crops to fill without stretching. No decorative animations or new gameplay simulation behavior are introduced.
- All added player-facing prose comes from the English/Thai localization CSV and its StreamingAssets copy.

## Artwork provenance

`Assets/Resources/TitleUI/Backdrop.png` was created with the built-in ImageGen tool from the user-approved concept. The original generated file is `exec-5f8d481c-a946-4ea9-8af7-b34ab3b81afa.png`; the concept is `exec-c6966cd9-eb45-4431-9476-f0b880bd8402.png`.

Generation prompt: Extract the clean background from the approved title screen. Preserve the scene, composition, lighting, dark stone arch, misty forest, distant castle, bridges and lake. Remove all text, logos, lines, icons, server strip, buttons and version number, reconstructing the scenery underneath. Deliver a full-bleed 16:9 background with no UI.

The texture is imported without mipmaps or NPOT rescaling, with clamp wrapping and a 2048px maximum. Original generated artwork is retained outside the project.

## Validation

Editor commands through `.roi_cmd`:

```
exec:TitleLandingProbe.Run
exec:TitleUxProbe.Run
exec:OnlineCompatibilityProbe.Run
```

Stop Play mode between commands. The landing journey exercises real UI raycasts, settings persistence, the configured server picker, modal navigation, support-copy contents and account busy locking. It captures 1280×720, 844×390, 2340×1080 and 1024×768, plus a simulated notched safe area, and measures minimum 44px landing touch targets. The probe restores the user's sound preference and clipboard on exiting Play mode.

The journey's negative control (`TitleLandingProbe.RunSabotage`) disconnects the production server-picker listener. It fails at the expected server-picker assertion; reconnecting the listener passes the full journey. The existing title UX probe separately checks keyboard focus, submission and party validation.

`TitleLandingProbe.OpenGamePreview` opens the real game scene through SessionBoot, without automatic login. It temporarily selects Thai and suppresses the editor's Steam pipe; both are restored on leaving Play mode.

Screenshots and results are under `D:/roii/idle_mmorpg/ui-validation/title-redesign/`. These screenshots predate the version synchronization below.

### Server version synchronization — 2026-10-28

Installed the Windows simulation library built from server commit `eaf05e23efbbc3a011542bf16b50412dd0aad674`, together with its simulation source and catalog. Direct loading of the installed DLL and a fresh public `/version` request confirmed simulation `0.0.495-alpha`, catalog `0.0.224-alpha`, and build `cb786b6d51f601842794d024dc9abb95` match exactly. Compatibility checks remain enabled. The approved Unity UI was preserved; starter text now names Suzu.

Simulation library tests passed: 528 sim-core, 27 sim-host, and 7 sim-ffi (562 total; 2 ignored). Backup and synchronization metadata: `D:/roii/server-sync-backup-20261028/`. Test log: `D:/roii/server-sync-tests-20261028.log`.

Online gameplay remains unverified: the configured development authentication key receives HTTP 403 from `/auth/dev`. Unity visual verification could not run because the editor exited with code 198 (no valid Editor license). Neither authentication nor licensing was bypassed.

### Anime title revision — 2026-10-28
Replaced Resources/TitleUI/Backdrop.png with the approved simplified navy/violet anime character artwork, retaining its importer metadata. Removed the server picker and its focus entry; Start Game still opens the existing sign-in flow and the configured endpoint remains unchanged. The title uses the existing HomeSerif font; settings, news, support and account remain real controls. Updated the journey probe to verify Start Game and absence of the picker; its negative control now disconnects Start Game.

Artwork generated using built-in imagegen from the approved mockup. Final prompt: Remove all text, icons and UI rules, reconstruct the background, and preserve the characters, framing and blue-purple castle mist. Installed asset: unity/ROI/Assets/Resources/TitleUI/Backdrop.png. Backup: D:/roii/title-anime-backup-20261028/.

Validation: production C# assembly compiled successfully with Unity's Roslyn compiler and existing reference configuration (warnings remain). Runtime visual and click verification is pending Unity license activation; the updated journey has not been run.
