# Floating Forge — Unity implementation and handoff

2026-09-16 · worktree `/Users/lunalieneray/idle_mmorpg-gem-ui` · branch `codex/gem-ui-mvp`.
Based on **`1e71fcad`**, sim **0.0.397**, catalog **0.0.146**. Earlier 0.0.392 notes and the initial plan are historical. Godot is retired; this is Unity work only.

## User intent / current implementation

Character & Inventory is the primary window. Bottom navigation changes one secondary window to its right: **Fusion Forge** or **Socket Gem**. The same controllers and drafts work in Battle and Companion. Combat never pauses for these windows or Result.

- **Battle:** Inventory entry or **I**. Primary is 500×560; secondary 320×560 with a 10 px gap. Drag the primary title to move the group.
- **Companion:** **W** from Battle, then **Character & Inventory** below the camera (or **I**). It expands the existing borderless, always-on-top standalone window ABOVE the gameplay row. It does not create another native OS window.
- Primary X closes the management group; secondary X closes only the secondary. **Esc** closes confirmation → Result → secondary → primary. **W** / the camera toolbar X returns to Battle, restoring the previous window size/topmost state.
- **1 / 2 / 3** changes camera count (horizontal beneath the UI when management is open); **-/=** zooms; right-click over the camera cycles its subject. Cameras still render at 30 Hz. Simulation remains owned by `SimDriver`.
- **Game size 1× / 2× / 3× / 4×** lives below the camera, separate from camera zoom and the 1/2/3 pane-count shortcuts. Default is **2×**; `PlayerPrefs["roi.companion.gameScale"]` remembers the choice. Nominal camera sizes are 320×180 / 640×360 / 960×540 / 1280×720. The camera RenderTexture follows the displayed size and aspect, at one render pixel per displayed pixel; no extra 2× supersampling. UI text remains at the selected UI scale.
- **UI size 1× / 1.5× / 2×** is a separate control row visible while management is open. Default is **1.5×**; `PlayerPrefs["roi.companion.uiScale"]` stores index 0/1/2. It scales the primary, secondary, confirmation, Result and their text together. Battle retains its existing scale.
- Size changes apply immediately. The macOS Companion window uses zero-duration `animationResizeTime:`; its original behavior is restored on exit. This leaves the window class intact (AppKit KVO depends on that identity), and forwards other windows to their original timing.
- Native layout fits the selected monitor work area (excluding taskbar/Dock). If the requested size or multiple panes exceed available space, the actual camera scale is capped while the chosen multiplier stays selected. Management uses its chosen UI scale, capped only when the monitor cannot fit it together with gameplay. Closing it releases more space for gameplay. One closed camera at 1× is 336×264 including two control rows; 2× is 656×444 when space permits. With UI open, total size depends on the monitor and pane count; the UI pair is always above the gameplay row with a 10 px gap. Closing UI restores the vertical camera stack at the chosen size.
- **Transparent desktop gaps:** Companion clears to RGBA zero and the standalone native window composites alpha. Panel and gameplay backgrounds remain opaque. macOS restores window/layer opacity, background and shadow on returning to Battle. `preserveFramebufferAlpha: 1` in Player Settings is required; do not reset it as build noise. Unity recreates its native layer asynchronously during resize, so `WindowMode.SettleAppearance` reapplies transparency briefly after a resize. Independent native windows, advanced docking and explicit click-through for transparent gaps are not implemented.
- Ten gear slots: main gear in the top three rows; **bottom-left Ring + Necklace, bottom-right Earring + Bracer**. Shoes moved to the third row on the right. Character selection, actual stats, four-column scrolling bag, selected-item details and Equip. Slot symbols are provisional category icons, not unique item art.
- **Tools** beside the bag opens the previous full inventory for sell/discard/deposit. From Companion, that action explicitly returns to the normal game window first.
- **Full Stats** sits below the compact character summary. It opens a scrollable 320×560 panel on the **left** of Character & Inventory, while the paper doll and bag keep their existing layout. The panel reads `stats:<entity_id>` directly and shows every current combat field (including equipment contributions) plus level/XP, Core and Apex progression values. `Back` or the panel X closes it; changing to Forge/Socket closes it first. Companion sizing reserves the extra width only while it is open.

## Gameplay contract — do not infer outcomes in C#

### Equipment

`fuse_rules` and `fuse_preview:<six indices>` supply counts, ceiling, Blacksmith level, live `fail_bp`, output tier/grade, Gil and server binding. Six distinct bag items of one grade; bound/unique/equipped/ceiling pieces are excluded. Auto-fill also skips gemmed gear and prepares ONE batch. Manual gemmed selection warns that socketed gems are destroyed even if fusion fails.

- **Auto Fusion toggle:** Unity orchestrates repeated low-grade batches only. When enabled it finds six eligible Common or Fine gear, sends one normal verified `FuseGear` action, waits for the refreshed inventory, then repeats. The sim/server remains authoritative for eligibility, consumption, failure, Gil and the resulting item; the client never predicts or batches those outcomes. The loop is owned by the persistent inventory host, so closing the Forge only hides its controls; it continues until disabled or the session changes. It never submits a batch that requires a server roll.

**Master changed during this task:** fail chance follows input grade and the crafter level. A consumed failure returns one piece of the INPUT grade and FULL Gil. A returned item is therefore NOT evidence of success. Preview uses `10000 - fail_bp` for display only; it does not calculate balance or roll anything.

When `needs_server_roll`, `ServerClient.FuseRoll(key, attempt)` must succeed with a nonzero nonce. Timeout is 20 seconds; no local-roll fallback. The callback rechecks session, frozen item JSON, draft and binding. `ApplyVerifiedInputs` accepts both consumed success AND consumed failure; never filter only for success.

### Gems / sockets

- `gem_rules` supplies merge count, ceiling, set weights, crafter level and `fail_bp_now_by_grade`. It is refreshed after transactions, so leveling changes the displayed odds.
- Gem fusion reads ledger `materials`; repeats are permitted up to the held quantity. Six same-grade gems, mixed sets allowed. No Gil charge or reward. Failure returns one gem of the input grade; success gives the next grade. Set is random in both cases.
- Socket page reads `gem_plan:inv:<idx>` / `gem_plan:eq:<entity>,<worn_idx>`. Gear selection can come from the bag or paper doll. The plan supplies held gems, applicability and stat ranges; UI does not recompute them.
- `SocketGem` target tags are LOWERCASE: `{"inventory":{"idx":…}}` or `{"equipped":{"entity_id":…,"equipped_slot":…}}`.
- Insertion/replacement is confirmed explicitly; cancel consumes nothing. Replacement destroys the previous gem. There is no unsocket API/button.
- Item-view `socket_count` = card slots; `gem_slots` = gem slots. Ignore legacy item-view `card_slots` (always zero); the separate `gem_plan.card_slots` is valid.
- Additive `gem_line_groups: [{lines:[…]}]` wraps the existing nested `gem_lines` for Unity JsonUtility; old field remains intact.

### Result

`Session.query_json("forge_receipt")` is an ephemeral adapter receipt captured around one input. Fields: `kind` (equipment/gems), `status` (success/failed/rejected), `item` (actual awarded/refunded item), `gil` (actual delta), `attempt` (consumed operation count).

Status comes from sim attempt/failure counters. Gem reward detection subtracts the actual input quantities first: a refund of the same ID can reduce a stack from six to one and must still appear. Capture happens synchronously before another combat tick, so random drops cannot be confused with the fusion result. Tick preserves the receipt; the next input clears/replaces it. It is not saved, hashed or part of gameplay state.

Result covers only the management group: gold rays/diamond outlines, success or failure title, ivory reward band containing the REAL item and Gil. Dismiss by click/Esc or after 4.5 seconds. Both outcomes show their returned item; failures never receive a success title. Result resets alpha and glow scale before activation, fades in over 0.28s and out over the final 0.32s. The glow grows from its center and settles within 0.35s, keeping thin outlines stable during the hold.

## Code map / extension points

Paths below are under `unity/ROI` unless specified.

| File | Owns |
|---|---|
| `Assets/Scripts/UI/Floating/FloatingInventoryHost.cs` | Persistent canvas/controller outside HudBar, primary bag/paper doll, selection, confirmation, context and layout |
| `…/ForgeModel.cs` | Read models, frozen selection/draft, live rules, server request, verified input and receipt event |
| `…/ForgePanel.cs` | Equipment/Gem tabs, six inputs, odds, crafter level, Auto-fill and Combine |
| `…/GemSocketPanel.cs` | Socket plan, held gem list and confirmed mutation |
| `…/ForgeResult.cs`, `…/ForgeUi.cs` | Reusable result, cells, retained UI primitives and simple geometry |
| `Assets/Scripts/UI/CompanionWidget.cs`, `CompanionWidget.Layout.cs` | Camera toolbar, saved game size, display fit, render resolution, hidden chrome and W while HudBar is inactive |
| `Assets/Scripts/Platform/WindowMode.cs` | Native resize/restore; never manipulate the Editor's own window |
| `Assets/Scripts/UI/HudBar.cs` | Battle entry and old inventory Tools fallback |
| `crates/sim-host/src/forge_receipt.rs`, `session.rs` | Read-only outcome adapter; no balance/RNG/schema changes |

Keep new secondary pages under the host, with one active at a time. Model coroutines live on the always-active controller, not on a closable panel. Refresh rows in place, not by destroying hovered buttons. An inventory-index shift invalidates a frozen selection rather than silently substituting another item. Localization uses `Loc`/`GameText`.

Standalone fixes found by actual launch:
- A culling-mask-zero clear camera removes stale framebuffer/UI trails when the arena camera is disabled.
- Companion hides ModeTabs along with combat HUD; preserves/restores their prior active states.
- `ArenaSparks` uses shader references in `Assets/Resources/Effects/RuntimeSpark*.mat`. These materials must be committed: dynamic Shader.Find alone was stripped from the player, causing repeated null-shader exceptions. Build helper can recreate them.

## Open, rebuild, verify

Unity **6000.0.83f1**, project `/Users/lunalieneray/idle_mmorpg-gem-ui/unity/ROI`.
Do not switch branch or open the master/other-session project by accident.

After changing `native/macos/roi_window.m`, run `tools/build_mac_plugin.sh` at the repo root and restart the player/Editor. The bundle is locally built and ignored by Git. The macOS implementation changes AppKit window opacity and Core Animation layer opacity; the Windows path uses [DwmExtendFrameIntoClientArea](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmextendframeintoclientarea) and still requires Windows-native validation. Unity needs [preserved framebuffer alpha](https://docs.unity3d.com/6000.0/Documentation/ScriptReference/PlayerSettings-preserveFramebufferAlpha.html).

Native bridge must be rebuilt after host edits and Editor restarted (no dylib hot reload):
```bash
cd /Users/lunalieneray/idle_mmorpg-gem-ui
tools/unity_dylib.sh
```
The current local bridge was rebuilt in **release** at 0.0.397, ad-hoc signed and staged. Use release for interactive play: the unoptimized debug bridge caused large JSON/read-model stalls in this scene. If using a custom Cargo target directory, the helper currently assumes `target/`; stage the output explicitly via unlink + copy + codesign, not overwrite-in-place while loaded.

**Editor preview:** exit Play, menu **ROI → UI → Floating Forge Preview**. It creates fresh temporary fixtures under `/private/tmp/roi-floating-forge`; no player save or online account is used. It leaves the preview open.

**Standalone:** exit Play, menu **ROI → UI → Build Floating Forge Standalone Probe**, then:
```bash
/private/tmp/roi_floating_forge/ROI.app/Contents/MacOS/ROI \
  -roiforgesmoke -screen-fullscreen 0 -screen-width 1600 -screen-height 1000 \
  -logFile /private/tmp/roi-forge-native.log
```
For manual play, replace `-roiforgesmoke` with `-roiforgepreview`. It opens Companion with Inventory + Forge, test gear, gems and currency, then leaves controls to the player without automatic actions or exit. It uses a fresh isolated temporary save.

`FloatingForgeSmoke` is DEVELOPMENT_BUILD/Editor only. The flag installs an isolated save directory before boot and skips Steam/server. It temporarily resets both companion-size preferences, exercises all choices, then restores the original preferences on normal process exit. It uses actual UI raycasts, scrolls a target into view when needed, queues keyboard input and exits with status 0/1. Output/screenshots: `/private/tmp/roi-floating-forge-native/`. Wait for the COMPLETE line AND zero exit code; partial PASS lines do not establish success. The normal shipped build does not include this harness or its save override.

Fresh worktrees need local owned dependencies excluded from Git: DOTween DLLs, full Live2D SDK, `Idyllic Fantasy Nature`, `libsim_ffi.dylib` and `roi_window.bundle`. Missing nature shaders create magenta flower cards. This worktree has them staged; they are not new vendored content in this patch.

## Verification and what remains

- **macOS standalone: 142 checks PASS, exit 0** (including resize stress), including native size/topmost, all gameplay/UI scale buttons with measured dimensions and saved choices, transparent framebuffer gaps (alpha 0) with opaque gameplay (alpha 1), whole group bounds, UI raycast hits, real gear/gem fusion, Result while combat ticks advance, socket confirmation, three panes, compact restore, I/Esc/W and return to Battle. No runtime errors/exceptions during the probe. Evidence: `docs/evidence/floating-forge-native/` (screenshots + verification.txt). Original side-layout logs: `/private/tmp/roi-forge-native-v397c.log` and `/private/tmp/roi-forge-native-final.log`. Latest combined layout/scale/transparency/Result/stress run: `/private/tmp/roi-companion-perf-stress.log` (build: `/private/tmp/roi-companion-perf-fixed-build.log`, 0 errors, release sim dylib). Earlier above-gameplay run: `/private/tmp/roi-forge-above-final.log`.
- **sim-host: all 20 unit tests PASS** on 0.0.397. Receipt regression tests cover parsed no-op, successful gear/gems, actual failed gear refund/full Gil and same-ID gem refunds with a negative net stack delta. Both changed failure guards were deliberately broken, failed, restored and passed (logs `/private/tmp/roi-receipt-sabotage-status.log`, `…-net_refund.log`). No sim-core behavior was changed by this UI branch.
- Localization guard: 1888 keys PASS; `git diff --check` PASS. Earlier Editor fixture had 19 passing checks on 0.0.392; use the latest standalone evidence for 0.0.397.
- The native resize guard was sabotaged by omitting `Screen.SetResolution` in `WindowMode.Resize`. The standalone probe correctly exited 1 at “primary native size 324x220” instead of accepting a clipped window; production code was restored. Log: `/private/tmp/roi-forge-native-sabotage.log`.

Older `docs/previews/floating-forge/` images are local untracked Editor captures. Verified native evidence is versioned under `docs/evidence/floating-forge-native/`.

Layout correction requested after the first MVP: the management pair now sits ABOVE gameplay, with a 10 px gap. The user then requested independently selectable gameplay 1×–4× and UI 1×/1.5×/2×, with transparent desktop gaps. Native probe geometry checks require all UI bounds to fit and stay above the camera/toolbar. The new check was first run against the previous side layout and correctly failed (`/private/tmp/roi-forge-above-negative.log`).

The new controls were also checked against deliberately broken production code:
- Game-size button forced to 1×: measured 2× growth failed (`/private/tmp/roi-scale-negative.log`). That early aborted run subsequently hit a Unity/Mono AsyncOperation finalizer crash (exit 139); the assertion itself fired correctly. The restored full run exited 0.
- Opaque camera clear: framebuffer alpha test failed at gap alpha 1 (`/private/tmp/roi-alpha-negative2.log`, exit 1), then passed at alpha 0 after restoration. The first attempt also exposed native layer opacity resetting after resize; the final implementation reapplies it during the transition.
- UI size forced to 1×: measured 1.5× panel growth failed (`/private/tmp/roi-ui-scale-negative.log`, exit 1), then passed after restoration.

The subsequent Result flash fix is covered by a regression that failed against the original animation (`/private/tmp/roi-result-negative.log`): the result previously began its first visible frame at full opacity. The native resize check measured 0.350s before and 0.000s in Companion, then confirmed original class/timing restoration; omitting the override failed the check. Local harness: `/private/tmp/roi-instant-resize-check.m`.

Size choices now swap immediately. macOS `animationResizeTime:` returns zero only for the Companion window; the original method/timing and animation behavior are restored when leaving. Do not change the window’s runtime class (`isa`): AppKit KVO titlebar teardown fails if it is swapped.

The 87-check run initially passed its UI assertions but crashed during shutdown in Unity/Mono `AsyncOperation.Finalize` (`/private/tmp/roi-floating-polish-final.log`, exit 139). The window code previously discarded `Screen.MoveMainWindowTo` operations on every resize. It now moves synchronously through AppKit, captures/restores the native top-left in desktop points, and places the window using its display’s visible frame. The revised player completed 87 checks and exited 0 twice consecutively (`/private/tmp/roi-floating-sync-native.log`, `/private/tmp/roi-floating-sync-repeat.log`); neither run reported a runtime exception or native crash. A separate native movement fixture verified movement, exact position restoration and display containment; deliberately omitting movement made it fail (exit 1). Do not count a COMPLETE line alone as a successful run.

Next priorities:
1. Authenticated high-grade `/fuse/roll` success/failure/timeout in a real test account; native smoke deliberately does not contact the server.
2. Windows native resize/topmost, multiple monitors, Retina/fullscreen transitions and low-resolution layouts. macOS windowed results do not establish these; Windows transparency/DWM is implemented but not run on Windows here.
3. Visual pass: denser/smaller typography and long Thai labels, correct per-slot/category artwork and missing catalog display names, transparent-gap mouse routing and visual polish. Keep the user's accepted Home/Exemplar style intact.
4. Additional bottom navigation/Storage only when its scope is chosen. No batch fusion automation, extra phone shell, or gameplay changes are included.

`godot/i18n/strings.csv` is a byte-identical localization DATA mirror only, required by the existing guard. No Godot development resumes.

## Performance follow-up (2026-09-16)

After interactive use reported frame drops, slow buttons and a crash, the preview process was still alive; the latest available crash report belonged to an earlier run. The new report has not yet been reproduced as a process crash. Do not claim that every possible crash has been fixed.

The local CPU sample (`/private/tmp/roi-interactive-slow.sample.txt`) found substantial scene-wide object searches and native view serialization. Changes:
- `UiConfirm.BlocksNavigation` tracks enabled dialogs instead of scanning/sorting all scene objects on every poll. Same-frame cancellation protection remains.
- Forge selection, mode, Auto-fill, Clear and removal use their existing `Model.Changed` refresh once, instead of refreshing the whole UI again in the button callback. Hidden Forge content is not rebound.
- Companion render textures match the pane's physical pixels. This cuts their pixel count to one quarter of the previous 2×-per-axis allocation while keeping the window and UI sizes unchanged.
- The macOS opacity snapshot uses weak layer keys. Removed Metal layers can be released while Companion stays open. A native lifetime fixture failed with the former strong references and passed with weak references after the Core Animation transaction settled.
- Rebuild the local sim dylib with `tools/unity_dylib.sh` (**release**). A new worktree that stages the old debug dylib will reproduce much of the slowdown despite having the C# fixes. No sim gameplay/schema change was made.

Measured using `-roiforgepreview -roiperf`, same 1262×1328 window, 132 TMP labels / 378 graphics, one gameplay pane, UI 1.5×, vSync off. Both comparison runs had no compiler running:

| Measurement (600 sampled frames) | Before | After |
|---|---:|---:|
| Average app frame | 12.34 ms | 3.84 ms |
| Frames over 25 ms | 95 | 2 |
| Frames over 33 ms | 67 | 2 |
| Worst frame | 122 ms | 97 ms |

These are uncapped application frames; the Companion cameras still update at 30 Hz. This is a short, machine-local comparison, and occasional spikes remain. Logs: `/private/tmp/roi-companion-perf-before-idle.log`, `/private/tmp/roi-companion-perf-after.log`. The first baseline taken during compilation (8 FPS) is invalid for comparison and is explicitly excluded.

`-roiforgesmoke -roiforgestress` adds 24 pairs of size changes, then a 60-second measurement with three panes, before the normal return-to-Battle checks. It reports frame times, graphics memory and per-button handler times. The render-size regression was run with the old 2× allocation and correctly exited 1 at the pixel-budget check (`/private/tmp/roi-render-budget-negative.log`), then the production fix was restored.

Stress result: **142 checks passed, process exit 0**, no runtime errors/native crash. During the 60-second three-pane hold: 10,867 application frames, average 5.52 ms, p95 18.54 ms, maximum 278.47 ms. Graphics allocation before/after the resize cycle was 463,403,105 / 463,335,209 bytes, with three pane textures totaling 4,078,872 bytes. This demonstrates no retained growth across this cycle; it is not a guarantee against every leak or long-session crash. The remaining isolated spike is a follow-up, not something to hide in the average.

Handler measurements include a 199.73 ms cold first Inventory open; subsequent slowest measured handlers were Combine 62.66 ms, Auto-fill 42.72 ms, socket confirmation 41.87 ms and item selection 12.10 ms. Resize callbacks were generally around 1 ms. Next performance work should profile those cold/preflight paths and rare spikes before changing gameplay or reducing the UI refresh rate further.

The native layer regression source is `unity/ROI/native/macos/tests/layer_lifetime.m`. From the repository root, compile with `clang -fobjc-arc -framework AppKit -framework Foundation -framework QuartzCore unity/ROI/native/macos/tests/layer_lifetime.m unity/ROI/native/macos/roi_window.m -o /tmp/roi-layer-check` and run `/tmp/roi-layer-check`. It measures object release after removal/transaction completion, not the map implementation.

## Automatic respawn follow-up (2026-09-16)

The interactive Forge fixture replaces `SimDriver.Session` after normal boot. `DeathBanner` retained the disposed boot handle, so it logged a restart every three seconds while the actual party stayed dead. It now binds the driver, resets its countdown when the live session changes, uses unscaled time like the simulation, and sends `RestartInstance` through the driver. The send is deferred until the next frame on the driver's coroutine host: a hidden Companion/Menu banner must still restart, and listeners must finish consuming the dead view before receiving the revived one. A pending request is cancelled if the session changes or the party revives. Success is logged only after the published state reports a living party. Dungeon summary ownership is unchanged.

Focused native regression: run the development standalone with `-roirespawnsmoke` (isolated temporary saves, no server). It keeps Inventory and Forge open, verifies that one member dying does not restart the team, then wipes the whole team twice. It checks actual native entity life state, a three-second real-time countdown (also at `Time.timeScale = 0`), the same session/UI staying open, and continued simulation ticks.

- Before the fix: exit 1, still dead at 4.53 seconds despite the old success log (`/private/tmp/roi-respawn-negative.log`).
- After the fix: **19 checks passed, process exit 0**, whole-party revivals at 3.11 and 3.10 seconds, no runtime errors (`/private/tmp/roi-respawn-fixed.log`). Build succeeded with zero errors; localization guard passed 1,888 keys. Evidence: `docs/evidence/floating-forge-native/respawn-verification.txt`.
- This focused run validates normal farming wipes in native macOS Companion; it does not add coverage for dungeon result UI or authenticated play.

## Companion X-button freeze follow-up (2026-09-16)

A manual long Companion session froze on returning to Battle. Sampling the still-live player showed its main thread creating/reparenting UI objects in LateUpdate, rather than waiting inside the native window plugin (`/private/tmp/roi-exit-companion-hang.sample.txt`). `ArenaView` kept pushing bars every frame and combat popups every hit while `ArenaHud` was inactive. Its LateUpdate could neither clear the per-frame bars nor age effects, so returning to Battle attempted to allocate UI for the entire hidden interval.

`ArenaHud` now drops all transient presentation feeds while inactive/disabled, clears pending bars, numbers, banners, multi-hit groups, skill labels, loot labels and hover text on disable, and hides pooled graphics. `ArenaView` also skips per-frame bar/hover work when the HUD is hidden. Authoritative damage, loot and sim ticks are unchanged; this discards only invisible visual notifications. Keep this lifecycle rule for future HUD feed methods. No native window-plugin change was needed.

Focused regression: `-roiexitsmoke`, with the same isolated temporary-save setup. It waits five seconds with real combat running, inspects the hidden queues before reenabling them, injects 1,000 calls to each presentation feed, then presses the actual toolbar X through the UI raycaster. Three cycles verify native appearance, Battle camera/HUD restoration, continuing ticks, fresh visible damage/multihit reception, cleanup and keyboard reentry. The fail-before-draw check prevents a regressed test from allocating the huge backlog and hanging the runner itself.

- Original code: failed with **5,081 retained bars** after the initial hidden interval, process exit 1 (`/private/tmp/roi-exit-negative.log`).
- Fixed code: **93 checks passed, process exit 0** (`/private/tmp/roi-exit-fixed.log`). All hidden queues stayed at zero. First returned frames after X were 0.127s, 0.044s and 0.054s; pooled HUD graphics remained 16–21, rather than growing with the hidden interval. No runtime errors were observed.
- Build succeeded with zero errors and localization guard passed 1,888 keys. Evidence: `docs/evidence/floating-forge-native/exit-verification.txt`. This targeted macOS result does not establish Windows behavior or eliminate unrelated long-session performance risks.

## Item drag/drop and complete hover (2026-09-17)

The same floating canvas now owns item interactions in Companion and Battle. No dependency on hidden HudBar or the global ItemTooltip. Equipment moves through ExemplarEquipmentActions and verified native inputs; fusion and socket rules remain in sim-host.

Controls:
- Hover bag, worn equipment, selected Forge materials, held Gems or fitted Gem slots for details. There is a 0.18-second hover delay; leaving the source hides the tooltip without changing the clicked selection.
- Equipment details include grade/tier, requirement, weapon type, eligible characters, owner, graded ATK/MATK/DEF/attack speed, every authored/rolled bonus and its lock marker, card capacity, empty Gem rows, each fitted Gem and every actual rolled line, Steam Market/binding/unique status and lore. Card/overlay lines already projected in ItemView are retained too. Held Gems show native min/max roll lines and their equipment group.
- Tooltip polish (user 2026-09-17): **show grade only**, never the separate base `item_level` / `_sN` tier. The small gear badge is `T{grade_step + 1}`, matching `GradeLabel`; a Fine shield reads Tier 2 / T2. Forge preview also omits base-tier odds. No Quantity or Sell value row in hover (held stack counters remain on cells).
- Sockets use one `[ ] Can insert a Gem` row per actual unfilled `gem_slots`; fitted Gems retain their names and complete native rolled lines. Zero-capacity items show no empty socket block. `socket_count` rows say **[ ] Can activate a Card effect / [ ] สามารถแสดงผลการ์ดได้** (user correction), since Cards are managed in their separate character UI rather than inserted into equipment. These rows describe supported Card effects, not empty card inventory slots. Never read the obsolete `card_slots` field from item view.
- Trade copy is a single **Can sell on Steam Market / Cannot sell on Steam Market** line from projected `marketable && market_clean`. No in-game market wording or vendor price. Socket insertion warnings also name Steam Market; this is presentation, not a listing action or change to economy rules.
- Visuals retain the WuWa/Exemplar palette and typography; TBH is only the reference for information grouping. The header uses an item/slot symbol, clear item name, grade and slot, with thin separators and colored effect lines. Long descriptions wrap at readable size and stay inside the window. Keep the pointer over the source item and use the mouse wheel to read the rest; short tooltips pass the wheel to the bag scroll. Tooltip and drag ghost do not intercept raycasts. UI scaling applies to them with the host canvas.
- Drag bag equipment to the paper doll; valid native destinations have green outlines, all other visible destinations red. Drag worn equipment onto the bag viewport to unequip. Dropping elsewhere or onto red targets consumes/moves nothing.
- Drag equipment/Gems into the next empty Forge input. The compact six-slot list fills in order; selected gear cannot be inserted twice, while Gem stacks may supply several units within their quantity. Combine remains an explicit action.
- In Socket Gem, drag a held Gem onto a green socket. Both insertion and replacement retain the existing confirmation; releasing the drag alone does not spend a Gem. Wrong-group Gems show red.
- Escape/Back first cancels the drag. Closing UI, changing character/page/context/session, focus loss, source removal or a stale item/loadout also cancels it. Legacy items without instance IDs additionally freeze the inventory fingerprint, so ambiguous reindexing never substitutes another copy. Loot changing such a legacy bag during a drag may require retrying.

Implementation seams:
- `FloatingItemInteraction.cs`: host-scoped pointer/drag lifecycle, ghost, non-blocking tooltip, pooled outlines; no global object searches.
- `FloatingInventoryHost.Items.cs`: frozen item/session/character bindings and native equipment actions.
- `FloatingItemDescription.cs` and `FloatingItemTooltip.cs`: host-projected descriptions, wrapping, screen containment and scroll.
- `ForgePanel.cs` / `GemSocketPanel.cs`: secondary drop targets share click-path eligibility and actions.
- `ExemplarEquipmentActions.PreviewDropSlot`: asks native EquipItem/UnequipItem on a disposable copy once at drag start. It does not infer mastery from visual slot labels or mutate the live sim. Final drop still revalidates current data through the existing verified action.

Descriptions are cached between the existing 0.4-second view refreshes. Do not query native per rendered frame, recreate hovered buttons, move the controller under HudBar, or implement stat rolls in C#. The compact clicked-item summary and timed Result remain separate from hover; this change does not replace the game's other full-screen item views.

Layout reference inspected: [Task Bar Hero equipment-comparison screenshot](https://note.com/ranats_mimi/n/n4c4680c6b00b) (Iron Gloves comparison, May 2026). Keep ROI/WuWa surfaces and type; do not adopt its pixel font, heavy gold frames or red trim.

Focused standalone verification uses `-roidragsmoke` in the same isolated development build described above. It checks real pointer raycasts through the ghost, green/red native equip routing, no-op invalid drops, actual equip/unequip, stale inventory, Forge inputs, confirmed Gem insertion, wrong Gem groups, native rolled text, long scrollable content/window bounds, context cancellation, Battle reuse and continuing combat. Latest result: **65 checks passed, process exit 0**, no runtime errors, on native macOS sim 0.0.397/catalog 0.0.146 (`/private/tmp/roi-tooltip-polish-native.log`). Build succeeded with zero errors (`/private/tmp/roi-tooltip-polish-build.log`); localization guard passed 1,918 keys and `git diff --check` passed. The test was deliberately broken twice: rejecting valid native drop slots failed at the green/red eligibility assertion (`/private/tmp/roi-drag-negative-slot.log`, exit 1); omitting actual Gem roll lines failed the socketed-item hover assertion (`/private/tmp/roi-hover-negative-gem.log`, exit 1). Both sabotages were restored before the final passing build. Versioned evidence: `docs/evidence/floating-forge-native/drag-verification.txt`, `13-drag-eligible.png`, `14-full-tooltip.png`, `15-socketed-hover.png`, `16-empty-socket-hover.png`. The latest two captures were visually checked for grade-only labels, an empty Gem row versus complete fitted rolls, and the single Steam Market status. Windows-native pointer/resize behavior and authenticated high-grade fusion still require their existing platform/account checks.

## Item grade backgrounds (2026-09-17)

User requested TBH-style rarity background recognition, keeping the WuWa visual theme. The [TBH inventory/tooltip screenshot](https://note.com/ranats_mimi/n/n4c4680c6b00b) shows colored icon tiles within a dark information panel; ROI now uses that treatment with its own grade palette, not TBH's rarity names or economy.

- `ItemGradeSurface` is a reusable retained `BaseMeshEffect` on the existing Image quad: dark top to stronger grade color below. No generated textures, extra canvases, shadow geometry, Update loop or per-frame allocations. `Bind` only dirties vertices when the colors change.
- `ForgeCell` applies it to bag gear/Gems, worn equipment, Forge materials, Result rewards and the drag ghost. Tooltip icon frames share the same surface and accent. Tooltip body stays neutral slate so stat colors remain readable.
- Gear uses the existing `GameText.GradeColor(grade_step)` through `ItemSlot.ItemColor`. Gems explicitly use `GradeColor`, since their material rarity field is not the grade ladder. Item names are lifted versions of the same tier color. Empty paper-doll slot labels stay neutral so color communicates grade only; slot identity comes from the icon and label. In the tooltip, Card slot rows use a restrained gold and Gem slot rows use teal, following the reference's distinct socket families without competing with the tier colors. Other kinds keep the existing rarity/foil mapping. No changes to drop rates, native grades, item data or balance.
- Palette order is now **grey / green / blue / violet / pink / gold / orange / red / white-pink / space red-violet**, for Common through Eternal. Tier 2 starts green so the first upgrade is clearly distinct from Tier 1; Divine uses a bright white-pink gradient plus fine white-pink sparkles, while Eternal uses a deep violet-red space gradient with gold/lavender sparkles. Empty slots stay dark.
- Selection retains the grade hue with a modest brightness lift and the existing gold marker/count. Grade borders remain visible; the independent green/red drop outlines still show valid/invalid destinations. A dark caption strip keeps white names readable. Read-only Result/ghost cells use the same fill as clickable cells rather than Button's grey disabled tint.

Verification: build succeeded with zero errors (`/private/tmp/roi-grade-surface-build.log`); native hover/drag regression **65 checks passed, exit 0**, no runtime errors (`/private/tmp/roi-grade-surface-native.log`). Visual inspection covered all ten gear/Gem grades, selected Epic, Common versus Fine, an empty-slot tooltip and native drag outlines. `FloatingForgeSmoke.GradePreview.cs` adds only a visual specimen sheet after the existing regression, using production ForgeCells; it neither grants high-grade items nor asserts game rules. Evidence: `docs/evidence/floating-forge-native/17-grade-surfaces.png`, plus refreshed `13-drag-eligible.png` through `16-empty-socket-hover.png`. Localization and whitespace guards passed.

## Full character stats (2026-09-17)

The primary character column keeps the compact Lv/HP/ATK/MATK/DEF summary for quick scanning. `UI_FLOAT_FULL_STATS` opens `FullStatsPanel` to the left. It is a presentation-only projection of the existing native `stats:<entity_id>` query; it does not allocate points, edit equipment or derive effective values in C#.

- Combat rows: Max HP, effective ATK, ATK from gear, MATK, effective DEF, DEF from gear, Dodge, Crit, Crit Damage, Attack Speed and Mitigation.
- Progression rows: Level/cap, XP, Level Points, Core Points, Apex Level, Apex Points and the current Apex allocation vector when present.
- The list uses the shared `UiScroll` column, so it remains usable at the compact 560 px height. `Back` closes the sheet before changing side panels. The standalone `-roifullstatssmoke` probe checks the raycast, left panel width, both sections and close path; latest result is 7 checks passed with screenshot `/private/tmp/roi-floating-forge-native/18-full-stats.png`.

## Latest handoff delta (2026-09-17)

The master-session agent should merge the commits from `/Users/lunalieneray/idle_mmorpg-gem-ui` branch `codex/gem-ui-mvp`; this worktree did **not** merge them into master:

- `88a097c5` — adds the client-side Auto Fusion toggle and localization keys.
- `195faf76` — keeps Auto Fusion running on the persistent inventory host after Forge closes, and continues while the user views another Forge tab. It stops only when disabled or the sim session changes.
- `990ac0c4` — renders resolved damage numbers in Companion panes through a per-pane camera-projected overlay, including normal, multi-hit and field damage.

Auto Fusion remains presentation orchestration only: it selects six eligible Common/Fine items of one grade, sends one verified `FuseGear` input at a time, waits for inventory refresh, and never predicts the result or submits a batch that requires a server roll. The sim/server remains authoritative.

Companion still intentionally hides the full `ArenaHud`; the new overlay covers damage numbers only. Break banners, heals, Miss/Guard text, skill labels, loot pills and HP/cast bars have not been mirrored into the small panes yet. Treat those as optional follow-up work unless full combat-HUD parity is required.

Verification for this delta: Unity build `0 errors`; `python3 tools/check_loc_keys.py` passed; `-roiautofusionsmoke` passed 10 checks; `-roiexitsmoke` passed 94 checks including `Companion renders damage numbers in the pane overlay`, with no runtime errors. The pre-existing untracked `docs/previews/` directory was deliberately excluded from all commits.

## Paste to the next agent

> Continue Unity Floating Forge in `/Users/lunalieneray/idle_mmorpg-gem-ui`, branch `codex/gem-ui-mvp`. Read `CLAUDE.md`, this document and `docs/gem-system-design.md §8`. Prioritize the same functional UI in Companion and Battle; do not move the model under hidden HudBar or pause SimDriver. Current base is sim 0.0.397/catalog 0.0.146: failed gear/gem fusion returns one input-grade item, gear returns full Gil, and the failure counters—not item presence—decide Result status. Follow the verification section before claiming native support. Work only in this isolated worktree. Godot is retired.
