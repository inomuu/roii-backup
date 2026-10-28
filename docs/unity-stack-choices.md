# Unity Stack Choices — ROI: Reign of Idlers

Scope: **the Unity side only** — which packages and subsystems, for **Unity 6000.0.x LTS + URP +
C#**. Deliberately out of scope (other docs own them, no duplication here):
`docs/unity-port-plan.md` (phasing) and `docs/sim-ffi-unity-gap.md` (the `crates/sim-ffi` C-ABI).

Sized against the client that exists today: ~27k lines of GDScript over ~110 view scripts
(`godot/views/`), an HD-2D 3D arena (`godot/proto/arena3d/`), online-required networking
(`godot/net/`), i18n from `godot/i18n/strings.csv` (1735 rows).

---

## TL;DR (ไทย)

- **UI: UI Toolkit เป็นหลัก + uGUI เฉพาะ world-space** — panel เยอะ + list ยาว (inventory หลายร้อยชิ้น)
  คือจุดที่ retained mode + `ListView` virtualization ชนะ uGUI canvas rebuild ขาด
- **HD-2D: URP **3D Renderer** (Forward+) ไม่ใช่ 2D Renderer** — เราต้องการ shadow/DoF/fog จริง
  แบบที่ `arena3d` ทำอยู่; sprite = quad billboard ใน vertex shader ไม่ใช่ SpriteRenderer + script
- **sprite number ทุกตัวอยู่ใน ScriptableObject catalog** เหมือน `monster_catalog.gd` วันนี้
- **VFX: Shuriken** (asset pack เป็น Built-in RP → ใช้ Render Pipeline Converter + URP particle
  shader); VFX Graph กินของพวกนี้ไม่ได้เลย
- **Net: `UnityWebRequest` + async/await, NativeWebSocket, Newtonsoft** (`com.unity.nuget.*`)
- **Steam: Steamworks.NET** — flow เดียวกับ `steam_auth.gd` (`GetAuthTicketForWebApi`) และ
  **ต้อง boot ได้เมื่อไม่มี Steam**
- **Localization: custom CSV loader อ่าน `strings.csv` ตรง ๆ** ไม่ใช่ Unity Localization package —
  คอลัมน์ `flags` + วินัย key-only (คีย์หายให้โชว์ KEY) แพ็กเกจแทนไม่ได้
- **Live2D: Cubism SDK** ฟรีแต่มีเพดานรายได้ ⚠️ **verify ตัวเลขก่อนตัดสินใจ**
- **Idle-open: sim tick แยกจาก frame rate** (accumulator บน `Time.unscaledDeltaTime`) + throttle
  `targetFrameRate` ตอนไม่ focus (ต้องปิด vSync ด้วย ไม่งั้นไม่มีผล)
- **Desktop companion: Unity เปิดหน้าต่างที่สองไม่ได้** → resize/mode-switch หน้าต่างเดียว
- **Hygiene: asmdef ตั้งแต่วันแรก** (compiler บังคับ boundary) + ปิด domain reload + วินัย `.meta`

---

## 1. UI — UI Toolkit primary, uGUI for world-space

**Recommendation: UI Toolkit (UITK) for every screen-space panel; uGUI only for world-space
overlays and drag/drop.** Alternatives: all-uGUI (safe, every Asset Store kit works, but you
hand-roll list virtualization); all-UITK (cleanest, but UITK has no world-space canvas in 6000.0).

Why UITK for panels:

- The screens are list-heavy — `stash_view.gd`, `item_market_view.gd`, `bestiary_view.gd`,
  `album_view.gd` render hundreds of rows. UITK's `ListView`/`MultiColumnListView` **virtualize by
  default**; the uGUI answer is a third-party recycling scroll rect.
- uGUI's cost model is *canvas rebuild* — one dirty `Text` re-batches the canvas, so you split
  canvases by update frequency forever. UITK keeps a retained tree and regenerates only what
  changed. For an app open for hours this is the difference that matters.
- UXML/USS separate layout from behaviour, matching the existing rule (`CLAUDE.md`: one scene one
  responsibility, split past ~400 lines).
- Rich text uses the TextCore/TMP tag subset, so `item_tooltip.gd`'s markup ports as-is.

Where uGUI stays: floating damage numbers (`number_fx.gd`, `multi_hit_popup.gd`), boss bars over
monsters, nameplates — all world-space. And drag/drop inventory, where uGUI's
`IBeginDragHandler`/`IDropHandler` is less code than a UITK `PointerManipulator` (build the
manipulator once and reuse it if you go UITK there anyway).

**Structure — "one panel one prefab, events not singletons"** (the current signal rule,
`CLAUDE.md`: "Cross-scene events are signals, never globals"):

```
UI/Panels/Stash/   StashPanel.uxml   StashPanel.uss   StashPanel.cs
```

Each panel = a `VisualTreeAsset` + a plain C# controller taking the root `VisualElement` and a
read-only state snapshot. Cross-panel talk goes through **C# events on injected interfaces**
(`stashPanel.EquipRequested += driver.RequestEquip;`), never a static `UIManager.Instance`. Only
the Unity `SimDriver` touches the native sim, exactly as `godot/sim_driver_core.gd` is the only
thing that touches `SimBridge`.

Gotchas:

- A full-screen transparent `UIDocument` swallows pointer events unless passthrough containers set
  `picking-mode: ignore` — this bites the click-attack idea.
- `UIDocument.sortingOrder` and `Canvas.sortingOrder` are **separate systems**. Reproduce the
  decided layer bands (windows 10 / popups 20 / tooltips 30 / HudBar 40 / world 0) as constants in
  one file and pick one system per band; do not interleave.
- Controller/touch later still needs `EventSystem` + `InputSystemUIInputModule` even in a
  UITK-only project. Wire it day one — retrofitting focus order across 110 panels is expensive.
- `ListView` wants **fixed item height**; `Dynamic` height virtualization is measurably slower.
  The rows already are fixed height.

---

## 2. HD-2D in URP

**Recommendation: URP **3D Renderer** (Forward+), perspective camera tilted down; sprites are
textured **quads** billboarded in the vertex shader.**

**Why 3D Renderer, not 2D Renderer.** The look is not 2D with faked depth.
`docs/hd2d-look-pipeline.md` documents a shadow-casting directional key (`SHADOW_OPACITY 0.93`), a
point fill (`FILL_ENERGY 3.0`, `FILL_HEIGHT 9.0`), **depth of field** with bands measured in view
distance (`DOF_FAR_DISTANCE 26`, `DOF_NEAR_DISTANCE 9`), glow (`GLOW_THRESHOLD 0.78`), a
frustum-fitted vignette quad, cloud shadows, and lit 3D grass. The URP 2D Renderer has no
shadow-casting directional light, no depth prepass, no DoF, and no 3D lighting — the whole grade
would be re-invented and would not land in the same place. The Godot project already runs
`forward_plus` (verified 2026-08-23), so the tuned numbers transfer as numbers.

Forward+ specifically: the scene runs several point lights (fill, plus VFX muzzle lights the LeLu
pack drives to `light_energy` 12.0 — `docs/vfx-notes.md` §3). Forward+ removes Forward's
8-lights-per-object limit.

Camera: perspective, ~25-35° pitch, narrow FOV (the Octopath long-lens flattening). Port the Godot
pitch/FOV/distance directly rather than re-tuning by eye.

Sprites:

- **Quad + vertex-shader billboard**, not `SpriteRenderer` + a `LateUpdate` `LookAt`. A managed
  per-object callback per frame for every actor, monster, prop and grass clump is pure waste; use
  `Graphics.RenderMeshInstanced` + `MaterialPropertyBlock` for crowds (`ambient_critter.gd`,
  `bird_flock.gd`, grass).
- **Full vs Y-billboard is already answered** — `hd2d-look-pipeline.md` §5: full billboard +
  directional shadow makes the mass *float*; Y-billboard sits but flattens under a pitched camera.
  Shipped answer: **full billboard, `cast_shadow` off, plus a shade-pool blob quad** on the ground,
  offset and stretched along the light direction. Reproduce it; do not re-litigate.
- **Contact shadows** for characters = the same blob quad. URP's screen-space contact shadows are
  wrong here — they shadow the billboard's silhouette in mid-air.
- **Filtering**: `Filter Mode: Point`, compression off (DXT kills pixel art), no mip maps, max size
  above native. Do **not** use the Pixel Perfect Camera — it belongs to the 2D Renderer and its
  integer snapping fights a perspective camera. HD-2D wants crisp texels inside a 3D scene, not a
  snapped pixel grid. Anti-alias the scene (SMAA), not the sprites.

**Catalog owns all sprite numbers.** `godot/views/monster_catalog.gd` holds every per-monster fact
— sheet, `fw`/`fh` or `cols`/`rows`, per-clip `{row, count, fps}`, `hit_frame`, `boss_scale`,
`faces_right`, `death_fade`, art aliases, and a `SLIME_DEF` fallback for unknown def_ids. Its
header states the rule: catalog = "what", animator = "how". Unity shape: a
`MonsterRenderCatalog : ScriptableObject` of `MonsterRenderEntry` (sheet ref, grid, clips, flags,
`aliasOf`), a generic fallback entry, and an animator containing **zero literals**. The
alias/fallback path is load-bearing — the doc records boss-room handmaids rendering as pinklings
when an alias was missed. Same discipline for sprite scale (ask the catalog's cell function, not a
lone constant).

**Sorting/transparency with many alpha sprites** — the pitfall that will cost the most time:

- Alpha-blended geometry does not write depth, so sprites sort **per object by camera distance**
  and pop when two centres cross. Fix in this order: (1) **alpha-clip** the pixel-art sprites
  (`clip()` / `AlphaToMask`) — it writes depth and sorts per pixel, and pixel art is hard-edged so
  it costs nearly nothing; (2) reserve true alpha blending for VFX and soft edges; (3)
  `renderQueue`/`sortingPriority` overrides for the handful that must always be in front.
- **DoF and every depth effect need sprites in the depth buffer.** Transparent-queue sprites are
  invisible to depth and the tuned DoF bands will not behave. This is the strongest argument for
  alpha-clip on actors.

---

## 3. VFX — Shuriken, not VFX Graph

**Recommendation: Shuriken (built-in Particle System) as the default. VFX Graph only for a
specific, measured, GPU-bound set piece.** Alternatives: VFX Graph everywhere (wrong); a
shader-only bespoke layer (too much work solo).

Why: **the packs are Shuriken.** The Hovl "Magic effects" pack ships Built-in RP Particle Systems
with `Particles/Standard Unlit` materials; VFX Graph cannot consume them at all, so adopting it
means re-authoring everything you paid not to author. Idle-game particle counts are small anyway —
the real cost here is overdraw/fill rate, which VFX Graph does not fix. And this project's VFX
workflow is script-driven recolouring from tables, which Shuriken + `MaterialPropertyBlock`
supports directly.

**Built-in RP pack → URP, the least painful path:**

1. **Window ▸ Rendering ▸ Render Pipeline Converter ▸ Built-in to URP**, *Material Upgrade*.
   Remaps `Particles/Standard Unlit` → `Universal Render Pipeline/Particles/Unlit`.
2. **The converter misses hand-written shaders** (distortion, dissolve, trails). Check the pack
   folder for an official URP variant *before* hand-porting — Hovl ships URP versions of most
   packs. Manual port is mostly `GrabPass` → `_CameraOpaqueTexture` plus fog/matrix macro renames.
3. Distortion/refraction needs **Opaque Texture ON** in the URP Asset, and it renders after
   opaques — it will not distort a transparent sprite drawn under it.
4. Soft particles need **Depth Texture ON**. Both cost a pass; enable deliberately and measure —
   this app stays open for hours.

**Porting `BY_SKILL` / `PAINTS`.** `godot/proto/arena3d/fx/pack_vfx.gd` owns four tables:
`BY_SKILL` (skill id → look; elements arrive post-override, so `sagitta_lucis` is Holy regardless
of caster element), `PAINTS` (all colour, `muzzle`/`hit` slot swaps, `motes`, `curve`), `TRIPLETS`
(the pack's own proj/hit/muzzle pairing, read-only), and scale constants. Unity:

- `SkillVfxCatalog : ScriptableObject` — entries keyed by **skill id, not element**.
- `PaintDef : ScriptableObject` — one asset per paint, shared by many skills (the current `tint`
  shorthand).
- `VfxTripletCatalog` — generated from the pack by an editor script, never hand-edited.
- Port `tools/check_vfx_tables.py` as an **edit-mode test** validating every id resolves and every
  prefab exists. Every failure mode in these tables is *silent on screen*; a test is the only guard.

Gotchas that carry over from `docs/vfx-notes.md` in a different costume:

- **Shared resources.** `renderer.material` clones (leaks one material per instance);
  `renderer.sharedMaterial` mutates the asset and dirties it on disk in the editor. Per-instance
  colour = `MaterialPropertyBlock`. Particle module structs are already per-instance.
- **Animation overwrites script-set values** — same trap as the Godot `AnimationPlayer` one. Check
  for an `Animator` clip or a particle curve driving a `Light`'s intensity before concluding your
  multiplier "did nothing".
- **Colour lives in ≥5 places**: material colour, `main.startColor`, `ColorOverLifetime`, the trail
  module's gradient, and painted textures. Enumerate them all in code once; fixing what you can see
  never converges.

---

## 4. Networking

**HTTP: `UnityWebRequest` + `async`/`await`** (Unity 6's `Awaitable`, or a small extension on
`UnityWebRequestAsyncOperation`). Alternatives: raw `HttpClient` (bypasses Unity's platform
transports — a mobile/WebGL liability); a third-party wrapper (unnecessary). This is a direct
translation of `godot/net/server_client.gd`, already `await`-based and spinning a fresh request per
call so calls cannot collide.

Port notes:

- Keep **one class owns all server IO** — `server_client.gd`'s header: "the ONLY thing in the Godot
  client that talks to verify-server". One `ServerClient` service, injected; panels never touch it.
- Reproduce the graceful-parse behaviour. `server_client.gd` avoids the parse path that logs an
  engine error because the deployed server answers an unknown POST with **plain text**, and a
  client running ahead of a deploy filled the log with red lines every flush — which is how a real
  error gets missed. Check content type / try-parse; never let a non-JSON body throw.
- `UnityWebRequest` leaks if abandoned: `using`, plus a `CancellationToken` tied to teardown.

**WebSocket: NativeWebSocket** (endel). Alternatives: `System.Net.WebSockets.ClientWebSocket` — the
"correct" BCL answer, fine on desktop IL2CPP, but **not implemented on WebGL** and historically
prone to AOT-stripping surprises on iOS; `websocket-sharp` — unmaintained, own TLS stack (a
security liability), predates .NET Standard 2.1. NativeWebSocket is a thin façade over
`ClientWebSocket` on desktop/mobile and a JS bridge on WebGL, so the call site survives a later
mobile or browser build. **Gotcha:** it requires pumping `DispatchMessageQueue()` from `Update` on
non-WebGL. If the game throttles to 10 fps unfocused (§8), **messages arrive at that cadence** —
pump from the fixed-cadence loop, or accept the chat latency knowingly.

**JSON: Newtonsoft via `com.unity.nuget.newtonsoft-json`.** Alternatives: `System.Text.Json` (not
Unity-packaged, drags a NuGet chain, its source generator does not run in Unity's compilation);
Utf8Json (unmaintained) / MemoryPack (binary — it does not read the server's JSON at all).
Newtonsoft is first-party-packaged, IL2CPP-safe if you avoid dynamic codegen, and needs no server
change.

**The real concern is per-tick state JSON.** `godot/sim_driver_core.gd`: `const TICK_INTERVAL :=
0.1` — 10 ticks/s. Naive `DeserializeObject<T>` at 10 Hz is ~36,000 allocations/hour and will show
as GC hitches over an idle session. In order: (1) **do not deserialize the whole view state every
tick** — split the payload so the per-tick part is small and panel-sized data (inventory, market)
is pulled on demand (a sim-side ask; the port plan owns it); (2) `JsonTextReader` over a pooled
buffer instead of `DeserializeObject<T>(string)`; (3) `PopulateObject` into reused targets;
(4) if still hot after measuring, escalate to a **binary** view struct, not a faster JSON library.

**TLS on Windows.** Unity's Mono historically failed cert validation on some Windows configs. In
Unity 6 `UnityWebRequest` uses the platform stack and Let's Encrypt chains validate — but **verify
against the real `renrei-idle-server.fly.dev` cert on a clean Windows box** during the first
cross-build (`tools/export_windows.sh` already does Windows). Never ship a `certificateHandler`
that returns `true` unconditionally; that is disabling TLS, not fixing it.

---

## 5. Steam — Steamworks.NET

**Recommendation: Steamworks.NET.** Alternatives: Facepunch.Steamworks (nicer async API, thinner
coverage); hand-written P/Invoke (no). Steamworks.NET is a near-1:1 binding like `godotsteam`, so
`godot/net/steam_auth.gd` translates almost line for line.

Install via UPM git URL (`rlabrecque/Steamworks.NET`) or the `.unitypackage`. A `steam_appid.txt`
containing **4958230** must sit beside the editor working directory and beside the built exe.

| `godot/net/steam_auth.gd` | Unity |
|---|---|
| `Steam.steamInitEx(APP_ID, true)` | `SteamAPI.Init()` |
| `Steam.getSteamID()` / `getPersonaName()` | `SteamUser.GetSteamID()` / `SteamFriends.GetPersonaName()` |
| `Steam.getAuthTicketForWebApi("")` | `SteamUser.GetAuthTicketForWebApi(null)` |
| `get_ticket_for_web_api` signal | `Callback<GetTicketForWebApiResponse_t>.Create(...)` |
| `buf.slice(0,size).hex_encode()` | `BitConverter.ToString(t,0,size).Replace("-","")` |

Details the Godot file already encodes and Unity must repeat:

- **`GetAuthTicketForWebApi`, not `GetAuthSessionTicket`** — the file's header states the
  SDK-to-SDK ticket fails Web-API `AuthenticateUserTicket` validation.
- The identity string (`""`/`null`) **must match** what the server passes to
  `AuthenticateUserTicket`.
- The **5-second timeout** racing the callback, so a stuck Steam client cannot hang login forever.
- `embed_callbacks=true` has no equivalent: Steamworks.NET needs `SteamAPI.RunCallbacks()` **every
  frame** from one `MonoBehaviour`. Forget it and the ticket callback never fires — login just
  times out, silently. Use the shipped `SteamManager.cs`.

**Running without Steam is a requirement, not a nicety.** The header: "if Steam isn't running (dev
without Steam, headless/CI) init returns false and the caller falls back to the dev login — the
game still boots". `SteamAPI.Init()` returning false must be a handled branch, and the dev path
(`POST /auth/dev` with the `X-Dev-Key` header, per `server_client.gd`) must stay reachable. Leave
`SteamAPI.RestartAppIfNecessary` **off** in dev builds or the editor relaunches through Steam.

---

## 6. Localization — custom CSV loader, not the Unity package

**Recommendation: a loader that reads `godot/i18n/strings.csv` in its current shape.** Alternative:
`com.unity.localization` (String Tables, Smart Strings) — a good package, wrong fit here.

The real header is
`keys,flags,en,th,zh_CN,zh_TW,pt_BR,ru,ja,ko,es,de,fr,pl,tr,vi,id,it,uk`, 1735 rows. Two
project-specific semantics the package cannot express:

- **`flags`** — `fixed` = keep EN in every language (`docs/localization-plan.md`:
  `ITEM_*`/`MON_*`/`SKILL_*` names are `fixed` because trade/guide terms must be one name
  worldwide — Poring is Poring). `=` in a per-language cell = "deliberately EN here", distinct from
  an empty cell = "not translated yet".
- **Key-only discipline** — `Loc.ui(key)` has no EN fallback: the CSV is the single source and a
  missing key renders **the raw KEY on screen**, which is what makes it visible. Guarded by
  `python3 tools/check_loc_keys.py` (exit 1 on a literal key not in the CSV).

Adopting the package means converting to String Table assets (a build step that can silently drop
`flags`), losing `check_loc_keys.py`, and gaining Smart Strings the project does not need (integers
and `{v}` placeholders). A ~120-line reader keeps the source of truth, the guard, and the
translation workflow.

Shape: `LocDatabase` parses the CSV at boot (RFC-4180 quoting; Thai + Cyrillic ⇒ UTF-8, no BOM
assumptions) into `Dictionary<string,string[]>`; `static class Loc` mirrors `godot/loc.gd`'s
surface (`Ui`, `Item`, `Mon`, `Skill`, `ItemLore`). Then:

- **`Ui(key)` returns the key when missing.** Do not add a "nice" fallback — the loudness is the
  design.
- **Resolve on display, never cache at construction** — a cached resolved string leaves stale
  labels after a TH↔EN swap. Bind a re-localize pass to a `LocaleChanged` event walking live
  `UIDocument`s.
- Item names read the `en` column in every locale (`godot/loc.gd`: "ITEM NAMES ARE NOT LOCALISED,
  user 2026-08-16") — one function, keep it one function.
- **Fonts**: Thai + zh/ja/ko need TMP **dynamic** font assets with a fallback chain (Latin → Thai →
  CJK). A static CJK atlas is tens of MB. World-space text under zoom needs SDF — TMP is natively.

---

## 7. Live2D — Cubism SDK for Unity

**Recommendation: Cubism SDK for Unity 5.x, for cut-ins only.** `docs/cutin-authoring.md` describes
the cut-in system as already built in Godot, presentation-only (manager + spine + per-Exemplar
scene, rendered once into an offscreen 1600×900 viewport and blitted). The acolyte art is a mockup
to become Live2D. So Live2D is additive: a Cubism model replaces the static `Art` texture inside
the per-Exemplar scene; nothing else changes.

**License — ⚠️ verify before any budget commitment.** The shape of Live2D's terms (they change):
the Cubism SDK Release License is free for individuals and "small-scale" businesses defined by an
annual-revenue threshold (order of ¥10-20M, roughly USD 70-140K); above it a paid **Publication
License** with an annual fee is required, quoted case by case; the Cubism **Editor** is licensed
separately from the SDK, with feature limits on the free tier. Against `docs/budget-plan.md`
(ceiling ฿300K, break-even ~1,000 payers ≈ 50K players), the project starts under the threshold and
crosses it exactly in the success case — **treat the Publication License as a line item that
activates on success**, and confirm the current numbers on Live2D's site before art spend commits
to Live2D over animated sprite sheets. Do not repeat the figures above as fact.

Integration effort is low-to-moderate: a `.unitypackage`; models import as a prefab with
`CubismModel` + `CubismRenderController`; `.motion3.json` → `AnimationClip` driven by a normal
`Animator`, so the "AnimationPlayer clip directs the framing" workflow maps over.

URP gotchas:

- The SDK ships **Built-in RP shaders by default**. Install Live2D's URP shader package or the
  model renders magenta.
- Live2D is many transparent quads with a **hand-authored draw order** (`CubismRenderer` sets
  `sortingOrder` per drawable). URP's transparent sorting must not reorder it: own layer, rendered
  by the offscreen cut-in camera only, no depth write.
- `CubismMaskController` uses a render texture — a second RT on top of the cut-in RT. Budget for it
  on a MacBook Air; it is why the cut-in already renders once offscreen instead of per-pane.

---

## 8. Idle-open behaviour

Sitting open for hours is a **feature** (idle-open → CCU → Steam discovery), so resource behaviour
is a design constraint, not hygiene.

**Sim cadence independent of frame rate.** The rule exists: `godot/sim_driver_core.gd` —
`TICK_INTERVAL := 0.1` (100 ms/tick, GDD §4.2 rule 5) with a `tick_acc` accumulator.

```csharp
_acc += Time.unscaledDeltaTime;              // NOT deltaTime — timeScale must never touch the sim
int steps = 0;
while (_acc >= TickInterval && steps < MaxCatchupSteps) { Sim.Tick(); _acc -= TickInterval; steps++; }
if (steps == MaxCatchupSteps) _acc = 0f;     // drop the backlog rather than spiral
```

- **Unscaled**, so a paused cut-in or a `timeScale = 0` modal does not freeze the sim.
- **Cap catch-up.** A machine that slept for an hour returns a huge delta; unbounded catch-up is a
  multi-second freeze on resume. `sim_driver_core.gd:136` records the same hazard — a full
  `_process` stop lets `tick_acc` and delta pile up and fire a burst, so **pause by not
  accumulating**, never by disabling the component.
- Port `SPAN_STALL_TICKS := 3000` (~5 min unsaved freezes the sim until a save lands) with the tick
  loop — it is an anti-cheat boundary, not a nicety.

**Background throttling:**

```csharp
Application.runInBackground = true;          // required — the sim must keep ticking
QualitySettings.vSyncCount = 0;              // targetFrameRate is IGNORED while vSync is on
void OnApplicationFocus(bool f) => Application.targetFrameRate = f ? 60 : 10;
```

The vSync line is the single most common reason background throttling "does nothing".
`OnApplicationFocus` is the desktop hook (`OnApplicationPause` is mobile) and it will **not** fire
for a merely-occluded window. At 10 fps also gate heavy per-frame work — the
`ArenaRenderData.FX_ENABLED` concept already exists (every heavy animated VFX checks it); make the
Unity equivalent consider focus too.

**Expectations to measure, not promise:** unfocused **< 3% of one core** with **no allocation
growth over an hour** (run the Memory Profiler for 30 idle minutes before launch — the per-tick
JSON in §4 is the thing that breaks this). RAM **400-800 MB** for a URP build with pixel atlases
and a few Live2D models; cut-in art dominates, which is why `docs/cutin-authoring.md` already
mandates 3000px BC7 (~4× less VRAM). Never allocate a render texture per frame.

---

## 9. Desktop companion window

The design: a tiny always-on-top 16:9 camera widget toggled by a key, right-click cycles party
member, INT zoom (`godot/views/companion_view.gd`).

**What Unity cannot do:** a player build has **one OS window**. There is no supported multi-window
API — `Display`/multi-display drives multiple *monitors* from one app, not a second floating
window, and `EditorWindow` does not ship. The Godot second-`Window` approach has no equivalent.

**Recommendation: single-window mode switch** — effectively what `companion_view.gd` already does.
On toggle: `Screen.SetResolution(w, h, FullScreenMode.Windowed)`, reposition
(`Screen.MoveMainWindowTo`, Unity 6 — **verify** on macOS), render the companion camera full-window,
restore and persist size/position on toggle back.

**Always-on-top is not exposed by Unity** and needs native code per platform: Windows —
`SetWindowPos(hwnd, HWND_TOPMOST, …)` P/Invoke on `user32.dll` with `GetActiveWindow()` (plus
`WS_EX_LAYERED`/`WS_EX_TRANSPARENT` if click-through is ever wanted); macOS — `NSWindow.level =
.floating` from a small Objective-C plugin in `Plugins/`, no managed path. Both are small but they
are **native code you now maintain**, with notarization implications on macOS — and macOS is the
dev machine, so that is the one that will actually get written.

**Pragmatic alternative worth costing:** a **separate lightweight companion app** (native, or
Avalonia/Tauri) talking to the verify server directly, showing party status. It dodges every Unity
window limit, can be genuinely tiny in RAM (the point of the feature), and works while the game is
closed — but it cannot show the live rendered arena, which is the emotional core.
**Ship the mode switch first;** keep the separate app as the fallback if the mode switch proves too
disruptive.

---

## 10. Project hygiene for a solo dev + AI

- **Assembly definitions from day one.** Without them every edit recompiles everything, and on a
  MacBook Air domain reload is the dominant iteration cost. Split: `ROI.Sim` (FFI wrapper, no UI
  deps) · `ROI.Net` · `ROI.Render` · `ROI.UI` · `ROI.Editor` (`includePlatforms: Editor`) ·
  `ROI.Tests.EditMode` / `.PlayMode`. The compiler then **enforces** "views never touch the sim
  bridge" — `ROI.UI` simply does not reference `ROI.Sim`. That is a gate, not a convention.
- **Enter Play Mode: domain reload OFF, scene reload OFF** (*Project Settings ▸ Editor*). Play-mode
  entry drops from ~10 s to under 1 s. Price: `static` fields persist between play sessions — every
  static must reset in a
  `[RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.SubsystemRegistration)]`. This *will*
  cause one confusing bug; still worth it.
- **Batchmode / CLI** (the team already uses `Unity -batchmode -executeMethod`):
  - Build: `Unity -quit -batchmode -nographics -projectPath . -executeMethod ROI.Editor.Build.Win64 -logFile -`
  - Edit-mode tests: `Unity -runTests -batchmode -testPlatform EditMode -testResults r.xml -logFile -`
  - **Screenshots need a display.** `-nographics` captures black — the exact lesson
    `docs/hd2d-look-pipeline.md` §0 records for Godot ("NOT `--headless`, which has no rendering
    driver and captures black"). Open a window for ~2 s, `ScreenCapture.CaptureScreenshot`, quit.
    Port `_look_probe.gd`'s design wholesale: `--set KNOB=v`, `--tod`, and especially
    `--compare=KNOB=v1,v2,v3` (several values from ONE frozen frame, because separate runs never
    share a layout). The doc calls that probe "the whole reason the pass was fast".
  - Preview PNGs go to `~/Desktop/ROI/`, then `open`.
  - **The Editor holds a project lock** — batchmode fails while it is open. Close it, or keep a
    second cloned project directory for CLI runs.
- **Git**: ignore `Library/ Temp/ Obj/ Build/ Logs/ UserSettings/ *.csproj *.sln` (start from
  Unity's official gitignore). Confirm *Force Text* serialization and *Visible Meta Files*
  (defaults in Unity 6). **Git LFS** for `.psd`, large `.png`, `.fbx`, audio — art will dominate.
- **`.meta` discipline — the #1 silent breakage with AI-assisted edits.** A script created or moved
  by a shell command without its `.meta` gets a **new GUID**, and every prefab/scene reference to it
  becomes `Missing (Mono Script)` with no edit-time error. Move/rename through the Editor, or move
  the `.meta` alongside; always commit the `.meta`; never commit a `.meta` for a deleted file.
- **Editor weight on a MacBook Air.** Disable auto-refresh (*Preferences ▸ Asset Pipeline*; refresh
  manually), the Package Manager auto-update check, and unused built-in modules in
  `Packages/manifest.json` (`terrain`, `vehicles`, `cloth`, `wind`, `unityanalytics`). Iterate with
  a fast texture-compression default and re-import at quality only for builds. Keep the project on
  the internal SSD.

---

## 11. Packages / versions

For **Unity 6000.0.x LTS**. Treat exact patch numbers as a starting point and let the Package
Manager resolve.

| Package | Identifier | Version | Confidence |
|---|---|---|---|
| Universal RP | `com.unity.render-pipelines.universal` | 17.0.x | high — URP 17 is the Unity 6 line |
| Shader Graph | `com.unity.shadergraph` | 17.0.x | high (pulled in by URP) |
| Input System | `com.unity.inputsystem` | 1.11.x | **verify** patch |
| uGUI + TextMeshPro | `com.unity.ugui` | 2.0.0 | high — TMP folded into ugui in Unity 6 |
| UI Toolkit | built in (`com.unity.modules.uielements`) | — | high — nothing to add |
| Newtonsoft JSON | `com.unity.nuget.newtonsoft-json` | 3.2.1 | high |
| Test Framework | `com.unity.test-framework` | 1.4.x | **verify** |
| Addressables (optional) | `com.unity.addressables` | 2.x | **verify** — only if content streams; likely unneeded solo |
| Cinemachine (optional) | `com.unity.cinemachine` | 3.1.x | **verify** — CM3 is a rewrite, different API from 2.x |
| Burst (optional) | `com.unity.burst` | 1.8.x | **verify** — probably unnecessary; the sim is native Rust |
| Localization | `com.unity.localization` | 1.5.x | **not recommended** — see §6 |
| Steamworks.NET | git URL `rlabrecque/Steamworks.NET` | 2024.8.0 (Steam SDK 1.60) | **verify** package + bundled SDK |
| NativeWebSocket | git URL `endel/NativeWebSocket` | 1.1.x | **verify** |
| Live2D Cubism SDK | `.unitypackage` from live2d.com | 5.x + the **URP shader package** | **verify** SDK version, URP shaders, and §7's license thresholds |

Also **verify** before relying on it:

- Whether the specific Hovl pack ships an official URP variant folder — it changes §3 from
  "hand-port shaders" to "swap a folder".
- Steamworks.NET's UPM branch URL and whether it still ships `SteamManager.cs`.
- `Screen.MoveMainWindowTo` behaviour on macOS in 6000.0 (§9).
- URP DoF with alpha-clipped sprites on Apple Silicon. The Godot side hit three *silent* Metal
  shader traps (`docs/hd2d-look-pipeline.md` §10) and there is no reason Unity's Metal backend is
  free of that class of surprise.
