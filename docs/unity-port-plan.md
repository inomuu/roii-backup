# Unity port plan — evaluating a move of the ROI client from Godot 4.6 to Unity 6 (URP, C#)

Written 2026-09-05. **This is an assessment + plan, not a decision.** The sim (`crates/sim-core`)
and the backend (`crates/verify-server`) are untouched by any of it — only the renderer moves.
Everything below is measured against the tree as it exists today; every claim points at a real path.

---

## TL;DR (ไทย)

- **ที่จะย้ายจริง ๆ คือ renderer เท่านั้น** — gameplay อยู่ใน Rust (`crates/sim-core`) และ backend
  (`crates/verify-server`) ไม่ต้องแตะเลย นี่คือเหตุผลเดียวที่ port นี้เป็นไปได้ในเวลาที่มี
- **ขนาดงานจริง ~45,000 บรรทัด GDScript** (views 27.0K + proto/arena3d 11.3K + root 3.3K + net/tests/tools)
  ไม่ใช่ทุกบรรทัดที่ต้องย้าย: 2D renderer (~5.0K, FROZEN) **ไม่ต้อง port**
- **ตัวบล็อกอันดับ 1 คือ FFI**: `crates/sim-ffi` วันนี้ export แค่ **6 ฟังก์ชัน** ขณะที่ `SimBridge`
  (Godot) มี **~106 `#[func]`** → ต้องขยาย sim-ffi ก่อนทุกอย่าง (งาน Rust ล้วน ทำได้ทันทีโดยไม่ต้องรอ Unity)
- **`roi_state_json` ไม่เท่ากับ `view_state()`** — มันคืน `GameState` ดิบ (ไม่ trim stash/pool, ไม่มี
  derived fields) ส่วน Godot ใช้ตัว trim แล้วใน `crates/godot-ext/src/view_state.rs` ต้องย้าย module นี้
  ลงไปให้ทั้งสอง bridge ใช้ร่วมกัน ไม่งั้น payload ต่อ tick พองจาก ~29KB เป็น ~74KB
- **Hovl VFX pack เป็นของ Unity แต่เดิม** — เรามี `tools/unity_vfx_to_godot.py` ที่แปลง Unity prefab
  มาเป็น Godot scene 76 ไฟล์ ถ้าไป Unity คือ **ใช้ prefab ต้นฉบับตรง ๆ ไม่ต้องแปลง** (กำไรชัดเจน)
  แต่ LeLu pack ที่ `godot/VFX/` (36 .tres + 21 .tscn) เป็น Godot-native — ต้องหาของแทนหรือทำใหม่
- **ของที่ต้อง re-implement แบบ "ห้ามพลาด" ไม่ใช่ UI** แต่คือ `sim_driver_core.gd` / `sim_driver.gd`:
  tick pacing 0.1s, `SPAN_STALL_TICKS = 3000`, `SAVE_INTERVAL = 10s`, anchor/replay-span, auto-revert,
  save slots — พลาดตรงนี้ = save ถูก server ปฏิเสธ
- **UI แนะนำ uGUI ไม่ใช่ UI Toolkit** สำหรับเกม gacha/idle ที่มีหลายสิบ panel: uGUI มี asset/ตัวอย่าง
  มากกว่า, world-space canvas ใช้ได้เลย, และ UI Toolkit runtime ยังตามหลังเรื่อง effect/animation
- **เฟสงานประมาณการ (1 คน + Claude)**: P0 spike 1-2 สัปดาห์, P1 arena HD-2D 3-4 สัปดาห์, P2 views หลัก
  4-6 สัปดาห์, P3 online 2-3 สัปดาห์, P4 VFX/cut-in/castle 3-4 สัปดาห์, P5 export 1-2 สัปดาห์
  → **รวม ~14-21 สัปดาห์** ทับกับ budget/schedule ใน `docs/budget-plan.md` โดยตรง
- **ความเสี่ยงใหญ่สุดคือตาราง ไม่ใช่เทคนิค** — ตาม memory "budget-and-runway" schedule เสี่ยงกว่าเงิน
  การ port กินเวลาที่ตั้งใจไว้สำหรับ art/content
- **Live2D Cubism SDK** สำหรับ cut-in: Unity มี SDK ทางการ (ดีกว่า Godot ที่ไม่มี) แต่ licence
  ต้องเช็ก — ฟรีเมื่อรายได้ต่ำกว่าเกณฑ์ ต้องยืนยันก่อนวางแผนพึ่งพา
- **อย่าลบโปรเจกต์ Godot** — เก็บเป็น reference ที่รันได้ (ตัวเลข tuning หลายปีอยู่ในนั้น) และ `docs/*.md`
  ยังเป็น source of truth เหมือนเดิม ไม่ต้องเขียนใหม่

---

# PART A — Review of the Godot client

## A.1 Inventory by responsibility

Line counts are `.gd` only unless stated (`wc -l`, addons excluded). `godot/addons/` (145 MB,
2,125 files) and `godot/assets/` (59 MB, 392 PNG + 409 `.import`) are content, not code.

| Group | Path | GDScript lines | Class |
|---|---|---:|---|
| Orchestrator | `godot/main.gd` (235), `godot/main.tscn`, `godot/proto/arena3d/main3d.gd` (14) + `main3d.tscn` (43 nodes) | 249 | (a) presentation |
| Sim facade | `godot/sim_driver_core.gd` (725), `godot/sim_driver_debug.gd`, `godot/sim_driver.gd` (1497) — one object via script inheritance | ~2,400 | **(b) sim-facing glue** |
| Save layer | `godot/save_store.gd` (SLOT_COUNT 5, `user://` paths) | ~130 | (b) |
| Catalogue / loc | `godot/catalogue.gd`, `godot/loc.gd`, `godot/i18n/strings.csv` (1,735 rows × 19 locale columns) | ~400 | (a) |
| Story | `godot/story_driver.gd` (349), `godot/story/*.dialogue` (3) | 349 | (a)/(b) |
| **Views (all)** | `godot/views/` — 103 `.gd`, only **2 `.tscn`** | **26,965** | mostly (a) |
| ↳ 2D renderer (**FROZEN**) | `views/arena.gd` 943, `arena_draw.gd` 862, `arena_fx.gd` 636, `player_animator.gd` 1003, `enemy_animator.gd` 342, `base_monster.gd` 470, `player_node.gd` 171, `number_fx.gd` 357, `field_fx.gd` 200 | 4,984 | (a) — **do not port** |
| ↳ Shared render tables | `views/render_data.gd` (569) `ArenaRenderData` — sheet paths, frame tables, SFX/BGM banks, `sprite_cells()`, day colours | 569 | (a), used by BOTH renderers |
| ↳ Screens / panels | the other ~90 files: `character_view`, `refine_view` 667, `story_view` 636, `debug_view` 632, `album_view`, `item_market_view`, `gacha_view`, `skill_tree_view`, `world_map_view`, `hud_bar` 596, … | ~21,400 | (a) + (b) reads of `state` |
| ↳ Cut-in | `views/cutin_view.gd` (155) + `views/cutin/` (`cutin_content.gd`, 5 `.gdshader`, `cutin_acolyte.tscn` 354 lines) | ~500 | (a) |
| **HD-2D arena (the direction)** | `godot/proto/arena3d/` — 4-file inheritance chain `arena3d_scene.gd` 1389 → `arena3d_draw.gd` 318 → `arena3d_fx.gd` 1399 → `arena3d.gd` 873; plus `player_node_3d`, `base_monster_3d` 602, `field_fx_3d`, `blast_fx_3d`, `flat_fx_3d`, `grass_field`, `cliff_box`, `prop_3d`, `castle_*`, 8 `.gdshader` | **11,287** | (a) |
| ↳ VFX tables | `proto/arena3d/fx/pack_vfx.gd` (1250: `TRIPLETS`/`BY_ELEMENT`/`BY_SKILL`/`PAINTS`/`PROJ_SCALE`…), `fx/pack_preview.gd`, `fx/fx_scene.gd` | 2,001 | (a) |
| ↳ Imported VFX assets | `godot/proto/arena3d/fx/hovl/` — **76 `.tscn`** + 2 `.gdshader`, generated by `tools/unity_vfx_to_godot.py` from a **Unity** pack | (assets) | (d) |
| ↳ LeLu pack | `godot/VFX/` — 17 MB, 176 files, Godot-native `.tres`/`.tscn`, licence `godot/VFX/LICENCE-LeLu.txt` | (assets) | (d) |
| Baked scenes | `proto/arena3d/castle_scene.tscn` 4595, `castle_scene_fire.tscn` 4662, `zone_dressing_r1/r2/r90.tscn` | (data) | (a), machine-baked |
| Net | `godot/net/server_client.gd` (282, ~45 endpoints), `steam_auth.gd` (Steam App ID 4958230, `getAuthTicketForWebApi`), `telemetry.gd` | 441 | **(c) online** |
| Chat WS | `views/chat_view.gd` — the only `WebSocketPeer` in the client; gated off (`SimDriver.CHAT_ENABLED = false`) | 178 | (c) |
| Self-tests | `godot/tests/` — 8 headless selftests (`save_write_guards`, `story_*`, `day_cycle`, `mode_layer_ownership`, `dialogue`, `player_node_geometry`) | 1,443 | **(e) dev/test** |
| Godot-side art tools | `godot/tools/` — `bake_castle_scene.gd`, `bake_lurking_eyes.gd`, `bake_pixel_fire.gd`, `bake_voxel_fire.gd`, `extract_hovl_meshes.gd`, `shoot_hovl.gd`, `verify_export.gd` | 454 | **(d) pipeline** |
| Repo-side art tools | `tools/gen_grassland.py`, `gen_castle_floor.py`, `gen_castle_tex.py`, `apply_palette.py`, `build_bg.py`, `bake_cliff_shadows.py`, `unity_vfx_to_godot.py`, `check_vfx_tables.py`, `check_loc_keys.py` | (Python) | (d) — **engine-agnostic, keep as-is** |
| Prototypes | `godot/proto/hd2d_probe.gd` (709), `proto/arena3d/_look_probe.gd`, `fake_sim.gd`, `dressing_preview.tscn`, `pack_preview.tscn` | ~1,500 | (e) |

Renderer settings: `godot/project.godot` — 1600×900, `renderer/rendering_method.mobile="forward_plus"`
(the HD-2D memory note is correct: this actually runs forward+, not Compatibility).
Export presets today cover **Windows Desktop x86_64 only** (`godot/export_presets.cfg`).

**Structural fact that dominates the port cost:** `godot/views/` has 103 scripts but only **2
`.tscn`**. Every panel is built imperatively in GDScript (`Control` nodes constructed in `_ready`),
so there is almost no scene data to convert — but also nothing that a converter could carry over.
In Unity the natural equivalent is prefabs, which means the layout work is genuinely re-authored,
not translated.

## A.2 The FFI contract the client depends on

### `crates/godot-ext` — `SimBridge`, ~106 `#[func]` methods

`lib.rs` still documents "API surface stays at ≤ 6 functions". It has not held: the real counts are
`bridge_world.rs` 38, `bridge_items.rs` 35, `bridge_stats.rs` 21, `lib.rs` 12.

**Lifecycle (`crates/godot-ext/src/lib.rs`)** — the six that matter plus the save/replay set:

| Method | Args | Returns |
|---|---|---|
| `init_game` | `seed: i64, game_data_json: GString, zone_id: GString` | `bool` |
| `tick` | `n: i64` | — |
| `view_state` | — | `GString` (JSON) |
| `state_hash` | — | `GString` (hex) |
| `sim_version` / `sim_git_stamp` | — | `GString` |
| `save_json` | — | `GString` |
| `load_json` | `save_json, game_data_json` | `bool` |
| `export_log` | — | `GString` (SaveBundle: start checkpoint + input log) |
| `commit_anchor` | — | — (advance the start checkpoint after a **confirmed** push) |
| `replay_span` | — | `i64` (ticks since the anchor — feeds `SPAN_STALL_TICKS`) |
| `apply_input_json` (in `bridge_stats.rs`) | `event_json: GString` | `bool` |

**World (`bridge_world.rs`, 38):** `create_character`, `classes_json`, `current_class`,
`party_name`, `set_player_class`, `respawn_in_current_zone`, `restart_instance`, `leave_instance`,
`current_zone`, `select_band`, `recruit_member`, `recruit_cost`, `player_class_ids_json`,
`advance_options_json`, `advance_options_for_class_json`, `advance_class`, `buy_character`,
`buy_character_cost`, `deploy_character`, `bench_character`, `roster_json`, + 17 `debug_*`
(`debug_fake_clear`, `debug_blocked`, `set_debug_allowed`, `debug_set_hp_pct`, `debug_kill`,
`debug_inflict_statuses`, `debug_set_god`, `debug_god`, `debug_set_level`, `debug_grant_apex`,
`debug_add_levels`, `debug_add_skill_points`, `debug_max_job_and_skills`, `debug_advance`,
`debug_give_gear`, `debug_give_currency`).

**Stats/skills (`bridge_stats.rs`, 21):** `toggle_active_skill`, `dps_json`, `lifetime_stats_json`,
`reset_dps`, `skill_info_json`, `skill_tree_json`, `reserved_sp`, `sp_reserve_capacity`,
`sp_reserve_used`, `xp_to_next`, `job_xp_to_next`, `effective_stats_json`, `allocate_apex`,
`reset_apex`, `apex_buckets`, `stat_cost`, `loadout_json`, `buffs_json`, `soul_paths_json`,
`apply_input_json`, `bestiary_json`.

**Items/economy (`bridge_items.rs`, 35):** `sell_all_junk`, `max_listing_price`,
`pending_junk_gil`, `add_consumable`, `remove_consumable`, `shop_items_json`, `item_preview_json`,
`all_cards_json`, `bind_info_json`, `buy_item`, `grant_loot`, `grant_loot_foil`, `sell_stash_item`,
`discard_item`, `expand_inventory`, `inventory_json`, `owned_exemplars_json`, `owned_regalia_json`,
`debug_grant_regalia`, `stash_json`, `deposit_to_stash`, `withdraw_from_stash`,
`deposit_card_to_album`, `withdraw_card_from_album`, `rare_bag_json`, `card_album_json`,
`card_special_vals_json`, `upgrade_card`, `feed_card`, `escrow_json`, `refine_odds_json`,
`refine_odds_enriched_json`, `recipes_json`, `start_refine_inventory`, `orb_check_json`.

### `crates/sim-ffi/src/lib.rs` — 6 exported C functions

```c
Sim*   roi_new(uint64_t seed, const char* game_data_json, const char* zone_id);
void   roi_tick(Sim* sim, uint64_t n);
char*  roi_state_json(Sim* sim);     // caller frees
char*  roi_sim_version(void);        // caller frees
void   roi_string_free(char* p);
void   roi_free(Sim* sim);
```

`crate-type = ["cdylib", "staticlib"]`, deps `sim-core` + `serde_json` only. Every function is
total (null/garbage → null), and there is a real round-trip test
(`the_c_surface_drives_a_real_sim`) plus a null/garbage test.

### The gap — this is the port's first blocker

| Missing from `sim-ffi` | Why it blocks |
|---|---|
| `apply_input_json` | **No mutation is possible at all.** Without it the C host can only watch. |
| `view_state` | `roi_state_json` returns `serde_json::to_string(&GameState)` — the **raw** state. `crates/godot-ext/src/view_state.rs` trims `zone_spawn` (45 KB of a 74 KB payload — the pool carries a full `MonsterTemplate` with drop tables per entry) and the stash, and **injects** derived fields (`eff_max_hp`, `mon_name`, `sprite`, per-tick FX arrays). A Unity client reading `roi_state_json` at 10 Hz would parse ~2.5× the bytes and still lack the fields the UI draws. |
| `save_json` / `load_json` / `export_log` / `commit_anchor` / `replay_span` / `state_hash` | The entire **online save + verify** path. `export_log` is the SaveBundle the server replays; `commit_anchor` must fire only after a confirmed push; `replay_span` drives the stall freeze. Without these the Unity client can never save online — i.e. can never ship (`CLAUDE.md`: there is no offline version). |
| the ~90 query/mutation helpers | Every screen. `inventory_json`, `skill_tree_json`, `effective_stats_json`, `card_album_json`, `recipes_json`, `refine_odds_*`, `roster_json`, … |

**Mitigation that costs almost nothing:** `view_state.rs` and `item_view.rs` currently live in
`godot-ext`, but neither touches Godot types except at the `GString` edge. Move them into a shared
crate (or into `sim-core` behind a `view` feature) so **both** bridges emit the identical JSON —
which is exactly the "the two bridges cannot drift into different contracts" promise the sim-ffi
header already makes but cannot currently keep. Also add a golden test that
`roi_view_state_json` == `SimBridge::view_state` for the same state.

## A.3 Godot-side logic that is NOT presentation

This must be re-implemented faithfully; getting it subtly wrong shows up as *the server rejecting
saves*, not as a visual bug.

| Concern | Path | What must be preserved |
|---|---|---|
| Tick pacing | `godot/sim_driver.gd` `_process` | `TICK_INTERVAL = 0.1` accumulator, `bridge.tick(1)` in a `while` loop (never a batched `tick(n)` in the windowed path), `_debug_speed` stays 1.0 in real builds, `t` = fractional interpolation passed to views |
| Headless fast-forward | same | `HEADLESS_BATCH = 500`, `HEADLESS_TICKS` env, relative quit tick |
| Unsaved-span freeze | `sim_driver_core.gd` `SPAN_STALL_TICKS = 3000`, `_update_sync_stall()` | Must stay **below both** the server's `MAX_REPLAY_SPAN` (50,000) and `RESTART_GRACE_MS` (~3,600 ticks). The sim freezes but the save loop keeps running — a confirmed push is the only exit |
| Save cadence | `SAVE_INTERVAL = 10.0`, `_write_save_file()`, `_notification(NOTIFICATION_WM_CLOSE_REQUEST)` | Offline writes synchronously on quit; **online deliberately does not** (async HTTP can't finish during teardown) |
| Anchor/replay chain | `_push_online_inner()` → `export_log()` → push → **on success** `commit_anchor()` | Committing on anything but a confirmed 200 breaks the replay chain permanently |
| Session invalidation | `_session_invalid`, `superseded`, `_auto_revert()`, `_enter_session_invalid()` | Ticks pause on a doomed session so unsaveable progress is not accumulated |
| Save slots | `godot/save_store.gd` — `SLOT_COUNT = 5`, `user://active_slot.cfg`, `user://savegame.json` legacy migration, `_preserve_unreadable_save()` | Path/rename hazards are a known way to lose player saves |
| Debug-cheat gating | `SimBridge::debug_allowed` (false while online) | Five bridge methods bypass `apply_input`; online that corrupts the replay chain silently |
| Zone transition timing | `sim_driver.gd` `_update_zone_transition()` | Veil only on `is_instance_end`, edge-tracker seeding on the first state, `set_mode_active` guard |
| Death/respawn | `_check_player_alive_state()`, `PLAYER_RESPAWN_DELAY = 3.0` | Presentation-triggered but frozen while stalled |
| Single instance | `main.gd` `INSTANCE_PORT = 47821` loopback lock | Two clients share one save slot |
| Online client | `net/server_client.gd` (~45 endpoints), `net/steam_auth.gd` | Ticket flavour (`getAuthTicketForWebApi`, **not** `getAuthSessionTicket`), 5 s timeout race |

## A.4 Godot idioms to watch when translating

- **`signal` → C# `event` / `Action`.** `state_updated(state, t, just_ticked, ticks_fired)` is
  connected by ~40 views; keep the same 4-arg shape so the port is mechanical.
- **Script inheritance chains are a Godot-ism.** `SimDriverCore → SimDriverDebug → SimDriver` and
  the 4-file `arena3d_*` chain exist to keep one object under the 400-line rule. In C# use
  `partial class` — same single object, no `extends`-by-path traps, no "base can't call derived".
- **No autoloads by design** (`CLAUDE.md` rule 3). Do not "fix" this with Unity singletons; the
  equivalent is a `SimDriver` MonoBehaviour that children find via serialized reference.
- **Imperative `Control` construction** — no `.tscn` to import for 90 panels. See A.1.
- **`Dictionary`/`Variant` reads everywhere:** ~2,400 `.get(` calls against untyped JSON. A typed
  C# model is a real improvement but is also where silent-empty-panel bugs move to.
- **`@tool` scripts** (`views/render_data.gd`) run in the editor; the Unity equivalent is
  `[ExecuteAlways]` or, better, a `ScriptableObject`.
- **`resource_local_to_scene` / shared `.tres`** — `docs/vfx-notes.md` warns pack resources are
  SHARED and animation overwrites node values. The Unity mirror is material instancing
  (`renderer.material` vs `.sharedMaterial`) and it has the identical failure mode.
- **`res://` vs `user://`** → `Application.streamingAssetsPath` / `Application.persistentDataPath`.
- **Typed arrays** (`Array[Color]`) and `preload` by path (not `class_name`) have no Unity analogue
  and simply disappear.

---

# PART B — Unity port plan

## B.1 Project setup

Unity **6000.0.83f1** (`/Applications/Unity/Hub/Editor/6000.0.83f1`), **URP 3D**, Linear color
space, Forward+ (matches `project.godot`'s actual `forward_plus`).

```
Assets/
  Plugins/              # sim_ffi.dylib (macOS arm64), sim_ffi.dll (win-x64)
  Scripts/
    Sim/                # SimBridge P/Invoke, SimDriver, save store   -> asmdef ROI.Sim
    Views/              # panels, one prefab + one script each        -> asmdef ROI.Views
    Arena/              # HD-2D renderer                              -> asmdef ROI.Arena
    Net/                # ServerClient, SteamAuth, Telemetry          -> asmdef ROI.Net
    Localization/       # CSV loader + Loc.cs                         -> asmdef ROI.Loc
  Prefabs/{Views,Arena,VFX}/
  Art/                  # mirrors godot/assets/
  StreamingAssets/game_data.json   # the copy godot/data/game_data.json is today
  Resources/i18n/strings.csv
```

Assembly definitions matter here: `ROI.Views` must be **allowed to reference `ROI.Sim` but never
the reverse**, which turns CLAUDE.md rule 2 from a convention into a compile error — the
`make-the-compiler-the-gate` principle already used for `Gil` in sim-core.

**Coding rules (the Unity restatement of the Godot rules):**

1. One prefab, one responsibility; split any script past ~400 lines (use `partial class`, not
   inheritance chains).
2. **Sim is authoritative.** Exactly one `SimBridge` instance, owned by `SimDriver`. `internal`
   the P/Invoke class inside `ROI.Sim` so no other assembly can call it.
3. **C# never computes gameplay outcomes.** Reads come from the `StateUpdated` event; mutations go
   through `SimDriver` methods.
4. Cross-scene events are C# events, never static singletons and never `GameObject.Find`.
5. i18n: no new user-facing literals in C#; keys go through `Loc.Ui(key)` against
   `i18n/strings.csv`. Keep `tools/check_loc_keys.py` running (it parses source text — teach it
   `.cs` alongside `.gd`).

## B.2 sim-ffi binding

**Build:**

```bash
# macOS arm64
cargo build -p sim-ffi --release --target aarch64-apple-darwin
#   -> target/aarch64-apple-darwin/release/libsim_ffi.dylib
# Windows x64 (cross from macOS, the toolchain tools/export_windows.sh already uses)
cargo xwin build -p sim-ffi --release --target x86_64-pc-windows-msvc
#   -> target/x86_64-pc-windows-msvc/release/sim_ffi.dll
```

Place as `Assets/Plugins/macOS/sim_ffi.bundle` (rename the dylib) and
`Assets/Plugins/x86_64/sim_ffi.dll`; set the platform/CPU in the plugin importer.
`DllImport("sim_ffi")` then resolves on both. Add a `dylib`-staging step to a Unity equivalent of
`tools/play.sh` — and keep the **rm + cp + codesign** staging from the
`shared-worktree-playtest-hazard` note: `cp` onto a loaded dylib is a SIGKILL at load with an empty
log, and Unity will hit it exactly the same way (worse: the **Editor keeps native plugins loaded
for the whole session**, so a rebuilt dylib needs an Editor restart unless the binding is moved
behind an explicit `dlopen`/`dlclose` wrapper).

**P/Invoke for today's surface:**

```csharp
internal static class SimFfi {
    const string LIB = "sim_ffi";
    [DllImport(LIB)] internal static extern IntPtr roi_new(ulong seed,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string gameDataJson,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string zoneId);
    [DllImport(LIB)] internal static extern void   roi_tick(IntPtr sim, ulong n);
    [DllImport(LIB)] internal static extern IntPtr roi_state_json(IntPtr sim);
    [DllImport(LIB)] internal static extern IntPtr roi_sim_version();
    [DllImport(LIB)] internal static extern void   roi_string_free(IntPtr p);
    [DllImport(LIB)] internal static extern void   roi_free(IntPtr sim);

    internal static string TakeString(IntPtr p) {            // never leak: Rust owns the buffer
        if (p == IntPtr.Zero) return "";
        try { return Marshal.PtrToStringUTF8(p); } finally { roi_string_free(p); }
    }
}
```

Every added function follows the same shape: `*const c_char` in →
`[MarshalAs(UnmanagedType.LPUTF8Str)] string`; `*mut c_char` out → `IntPtr` + `TakeString`;
`i64`/`u64` → `long`/`ulong`; `bool` → `[return: MarshalAs(UnmanagedType.I1)] bool`.
**Never marshal a returned string as `string` directly** — the default marshaller would call
`CoTaskMemFree` on a Rust `CString` and corrupt the heap.

**Functions sim-ffi must gain before P0 can close** (mirroring §A.2, all `*const c_char` in /
`*mut c_char` out): `roi_apply_input_json`, `roi_view_state_json`, `roi_state_hash`,
`roi_save_json`, `roi_load_json`, `roi_export_log`, `roi_commit_anchor`, `roi_replay_span`,
`roi_git_stamp`. The ~90 query helpers can follow behind a single generic entry point —
`roi_query(sim, verb_json) -> *mut c_char` — which avoids 90 P/Invoke declarations and 90
`#[no_mangle]` blocks, at the cost of a string dispatch per call (negligible; these are UI-rate,
not tick-rate).

**Threading:** main thread only. `GameState` is `!Sync`, the handle is a raw pointer, and the
sim is deterministic precisely because there is one caller. Do not "optimise" ticks onto a job.

**JSON:** `System.Text.Json` is not available in the default Unity profile; use
**Newtonsoft Json.NET** (`com.unity.nuget.newtonsoft-json`, first-party UPM) for the low-rate
queries, and for the 10 Hz `view_state` use a **typed** DTO with `JsonUtility` or a hand-written
`Utf8JsonReader`-style parser over the returned bytes. Measure before optimising: the Godot side
parses ~29 KB at 10 Hz today and it is not the bottleneck.

**The `SimDriver` loop** (direct translation of `sim_driver.gd::_process`):

```csharp
void Update() {
    if (_state == null || _creating || _sessionInvalid) return;
    bool stalled = UpdateSyncStall();               // replay_span >= SPAN_STALL_TICKS
    int fired = 0;
    if (!stalled && !_cutinPaused) {
        _tickAcc += Time.deltaTime;
        while (_tickAcc >= TickInterval) { SimFfi.roi_tick(_sim, 1); _tickAcc -= TickInterval; fired++; }
    }
    if (fired > 0) RefreshState();
    if (!stalled) CheckPlayerAliveState();
    _saveAcc += Time.deltaTime;
    if (_saveAcc >= SaveInterval) { _saveAcc = 0f; WriteSaveFile(); }
    StateUpdated?.Invoke(_state, Mathf.Clamp01(_tickAcc / TickInterval), fired > 0, fired);
}
```

`Update`, not `FixedUpdate`: the accumulator already decouples sim time from frame time, and
`FixedUpdate` adds a second clock that can drift against it.

## B.3 Godot → Unity mapping

**UI framework recommendation: uGUI (Canvas), not UI Toolkit.** Reasons specific to this project:
(1) ~90 panels, many with per-item icons, tooltips and drag targets — uGUI's `Button`/`Image`
model maps 1:1 onto what `views/item_slot.gd`, `views/ui_window.gd`, `views/popup_shell.gd`
already are; (2) the gacha/cut-in presentation wants shader-driven effects on UI (the five
`views/cutin/*.gdshader`) and uGUI takes a normal material, UI Toolkit does not; (3) world-space
UI — the HD-2D arena projects labels along **camera up** (`docs/hd2d-look-pipeline.md`) — is a
world-space Canvas in uGUI and awkward in UI Toolkit; (4) the asset/tutorial ecosystem for gacha
UI is overwhelmingly uGUI. Keep UI Toolkit for editor tooling (the `debug_view` "World look" panel
is a natural first UI Toolkit window).

| Godot | Unity |
|---|---|
| `main.tscn` / `main3d.tscn` (43 sibling view nodes) | One `Main` scene; each view an **inactive prefab instance**; `ModeTabs` toggles `SetActive` — same ownership map `main.gd` holds today |
| `sim_driver*.gd` | `SimDriver` MonoBehaviour (`partial` across Core/Debug/Facade files), `StateUpdated` C# event |
| `save_store.gd`, `user://` | `Application.persistentDataPath` + the same 5-slot layout and legacy-migration guard |
| `views/*.gd` panels | prefab + `MonoBehaviour` per panel, subscribing to `StateUpdated` |
| `views/ui_window.gd`, `popup_shell.gd` | one `UiWindow` prefab, panels as children — keeps the `ui-canvas-layer-zorder` memory (windows 10 / popups 20 / tooltips 30 / HudBar 40) as `Canvas.sortingOrder` |
| `views/render_data.gd` (`ArenaRenderData`) | **`ScriptableObject`** assets (`ArenaRenderData.asset`) — sheet paths become sprite references, `sprite_cells()` a static helper. Keep the *numbers* verbatim (`PLAYER_DISPLAY_CELLS = 5.2`, `CELLS_PER_ART_PX = 0.052`, `MONSTER_FRAME_PX = 32`) |
| `proto/arena3d/arena3d_*.gd` (code-built camera/floor/lights) | An `Arena` prefab authored in the editor (Unity's advantage: it is visible without a probe) + `ArenaRenderer` scripts; billboards = quads with a billboard shader or `SpriteRenderer` + per-frame `LookAt(camera.transform.rotation)`; **bottom pivot** and full billboard per the HD-2D memory note |
| Camera / grade | URP Camera + **Volume**: Bloom, Depth of Field, Color Adjustments, Vignette, Tonemapping. `views/look_settings.gd` statics → a `LookSettings` ScriptableObject with `[Range]` sliders (live tuning in the Inspector replaces the `_look_probe.gd` + `--compare` workflow — this is the single biggest ergonomic win of the port) |
| Contact shadows, canopy rim | URP has a **Screen Space Shadows** renderer feature; the foreground canopy is a sorted transparent quad layer as today |
| `*.gdshader` (8 in arena3d + 5 cut-in + `zone_bg`) | Shader Graph or HLSL. **Re-check the `godot-shader-macos-gotchas` memory**: descending `smoothstep`, `source_color` → linear, depth fog past the end — Unity/Metal has its own variants of all three; re-measure, don't port thresholds blind |
| `fx/pack_vfx.gd` `BY_SKILL`/`PAINTS`/`TRIPLETS`/`PROJ_SCALE` | One `VfxTable` **ScriptableObject** + prefab references; keep `tools/check_vfx_tables.py` (it can validate the exported asset JSON) — every failure mode there is silent on screen |
| `fx/hovl/` (76 generated `.tscn`) | **Use the original Unity Hovl prefabs.** `tools/unity_vfx_to_godot.py` becomes dead weight; the pack's Shuriken systems, materials and shaders work natively — this is the clearest single win |
| `godot/VFX/` LeLu pack (Godot-native `.tres`/`.tscn`) | **No conversion path.** Must be replaced (a Unity projectile pack) or re-authored in VFX Graph. Budget for it; `pack_vfx.gd`'s per-skill table survives, its assets do not |
| `i18n/strings.csv` (1,735 × 19 locales) | A **custom CSV loader** (recommended) — one `Loc.Ui(key)` static over a `Dictionary<string,string>` built at boot. Unity Localization is heavier (tables, addressables, per-locale assets) and buys nothing here since the CSV is already the single source and `check_loc_keys.py` already guards it |
| `net/server_client.gd` | `UnityWebRequest` + async/await; keep the ~45 methods 1:1 |
| `views/chat_view.gd` `WebSocketPeer` | **NativeWebSocket** (endel/NativeWebSocket) — works on desktop and WebGL; chat is gated off today (`CHAT_ENABLED = false`) so this is not on the critical path |
| `net/steam_auth.gd` + `addons/godotsteam` | **Steamworks.NET** (or Facepunch.Steamworks). Same call: `GetAuthTicketForWebApi`, App ID 4958230, graceful `false` when Steam is absent |
| `views/cutin*` + `cutin_acolyte.tscn` | **Live2D Cubism SDK for Unity** (official; Godot has none — this is a real reason the port helps). Offscreen 1600×900 `SubViewport` → a `RenderTexture` + a second Camera; blit to a full-screen `RawImage` |
| `castle_scene.tscn` (4,595 lines, machine-baked) | Re-run the generator against Unity: `tools/gen_castle_floor.py` / `gen_castle_tex.py` are Python and engine-agnostic; only `godot/tools/bake_castle_scene.gd` needs a C# editor-script twin writing a prefab |
| `godot/tests/*_selftest.gd` | Unity **EditMode/PlayMode tests** (`UnityEngine.TestTools`) — and this is an upgrade: the Godot ones are ad-hoc scripts run by `tools/smoke.sh` |
| `tools/play.sh`, `smoke.sh` | Keep the shell wrappers; swap the Godot invocation for `Unity -batchmode -runTests` / a built player |

## B.4 Phases

Effort = one dev + Claude, calendar weeks, assuming the sim work happens in parallel with art.

| Phase | Scope | Effort | "Done" means |
|---|---|---|---|
| **P0 spike** | Extend `sim-ffi` (apply_input, view_state, save/load/export_log/commit_anchor/replay_span, state_hash, generic `roi_query`); move `view_state.rs` to a shared crate; Unity project + asmdefs; `SimDriver` loop; one debug panel printing party HP/level | **1–2 wk** | A Unity build ticks a real save at 10 Hz, shows live numbers, and `state_hash` after N ticks **matches** the Godot client from the same seed |
| **P1 arena** | HD-2D scene: camera + URP Volume grade, billboard sprites, floor/backdrop, monster + player animators (copy the tables, not the code — `copy-then-adapt-not-rewrite`), popups, contact shadows, canopy | **3–4 wk** | A fight is watchable side-by-side with `tools/play.sh` and reads as the same game; frame time on the MacBook Air is measured |
| **P2 core views** | HudBar, party, inventory/stash, character/stats, skills + skill tree, options, mode tabs, tooltips, item slots, i18n loader | **4–6 wk** | Every screen needed to play from login to Lv30 exists; `check_loc_keys.py` green on `.cs` |
| **P3 online** | ServerClient (45 endpoints), Steamworks.NET login, push/pull/anchor, sync banner, session-invalid + auto-revert, save slots | **2–3 wk** | A Unity client logs in against `renrei-idle-server.fly.dev`, plays an hour, and every push is **accepted** (server-side replay verifies) |
| **P4 VFX / cut-in / castle** | Hovl prefabs wired to the `VfxTable` SO; LeLu replacement decided; Live2D cut-in; castle prefab bake; gacha/store/market/album/bestiary/world-map screens | **3–4 wk** | Cut-in plays, per-skill VFX table complete, castle zone renders |
| **P5 export** | macOS + Windows player, Steam build upload, mobile (Android) smoke on URP | **1–2 wk** | Both desktop builds ship; one Android build boots and ticks |

**Total ≈ 14–21 weeks.** Critical path: **P0 → P3**. Not P1 — the arena can lag, but nothing ships
until a Unity save is accepted by the verify server, and that depends entirely on the sim-ffi
surface being complete and `commit_anchor` being called at exactly the right moment.

### Risks

| Risk | Severity | Mitigation |
|---|---|---|
| **Schedule** — 14–21 wk of port is 14–21 wk not spent on art/content, and `docs/budget-plan.md` + the `budget-and-runway` memory both say schedule is the bigger risk than money | **Highest** | Timebox P0 to 2 weeks. If `state_hash` parity is not reached by then, the port's core premise is unproven — stop and reassess rather than continue |
| **Determinism across builds** — a Unity client that produces a state the server cannot replay is unshippable | High | The sim is Rust and integer-only, so the risk is *not* in the math: it is in **input ordering and anchor timing**. Add a cross-engine golden: same seed + same input log, assert Godot `state_hash` == Unity `state_hash`. This test is cheap and must exist before P1 |
| **FFI/JSON at 10 Hz** — 2,414 untyped `.get()` reads today; a naive Newtonsoft parse of 29 KB × 10/s allocates hard and will show as GC spikes | Medium | Typed DTO for `view_state`; keep the `zone_spawn`/stash trims (i.e. **must** ship `roi_view_state_json`, not `roi_state_json`); profile in P0, not P4 |
| **Unity Editor weight on a MacBook Air** — the Editor plus a native plugin is far heavier than `tools/play.sh`, which skips the editor entirely; native plugins stay loaded for the Editor session, so every Rust change may mean an Editor restart | Medium | Keep a headless/player-build fast loop (`-batchmode` + a built player) as the primary iteration path, mirroring `play.sh`; consider a `dlopen` wrapper so the dylib can be reloaded |
| **LeLu pack is non-portable** (36 `.tres` + 21 `.tscn`, Godot-only, and `godot/VFX/LICENCE-LeLu.txt` forbids redistribution anyway) | Medium | Decide the replacement *before* P4; the `pack_vfx.gd` decision tables survive the swap, only assets change |
| **Live2D licensing** — the Cubism SDK is free only below a revenue/company threshold | Medium | Confirm the current terms before P4 commits the cut-in system to it; the fallback is the sprite/shader cut-in that already works (`views/cutin/cutin_template.tscn`) |
| **WebSocket on a Steam build** — chat is the only WS user and is currently disabled | Low | NativeWebSocket on desktop is fine; revisit when `CHAT_ENABLED` flips |
| **Silent-empty-panel class of bug moves, doesn't vanish** — `view_state.rs` warns a renamed key blanks a panel with no error | Low–Medium | Typed DTOs turn most of these into compile errors; keep `contract_keys_are_stable` in `view_state.rs` as the pin |

## B.5 What to keep from Godot

- **`docs/*.md` stays the source of truth, unchanged.** `docs/taskbar-arpg-gdd-phase0.md`,
  `docs/combat-formulas.md`, `docs/skills-catalog.md`, `docs/economy-invariants.md`,
  `docs/vfx-notes.md`, `docs/cutin-authoring.md`, `docs/localization-plan.md` are engine-agnostic.
  Only `docs/hd2d-look-pipeline.md` needs a Unity appendix (the numbers stay; the probe workflow is
  replaced by live Inspector tuning).
- **The HD-2D findings.** Catalog owns all sprite numbers; full billboard + bottom pivot; measure
  frame padding, not `foot_drop`; project UI along camera up; omni shadows are cubemaps. Every one
  of those is a Unity problem too.
- **Keep the whole `godot/` project, runnable.** It is the reference implementation for years of
  invisible tuning (attack anticipation synced to `atk_acc`, `hit_frame` contact, lunge surge,
  flinch queue, death sequencing — `views/player_animator.gd`, 1,003 lines) and the only way to A/B
  a Unity result against the intended one. Do not delete it, and expect to run both during P1–P2.
- **`tools/` Python stays** — `gen_grassland.py`, `apply_palette.py`, `build_bg.py`,
  `check_loc_keys.py`, `check_vfx_tables.py`, `check_online_sync.sh`, `deploy.sh`. None of it knows
  what an engine is. `tools/unity_vfx_to_godot.py` becomes obsolete (keep it in history).
- **`crates/godot-ext` stays too**, at least through P3: it is the reference the Unity binding is
  diffed against, and it is what keeps the Godot fallback alive if the port stalls.

## Reconciliation notes (2026-09-05, after review)

- Verified against source: `crates/sim-ffi/src/lib.rs:68` calls `sim_core::tick::tick`, while
  `crates/godot-ext/src/lib.rs:133` uses `replay::advance` with `record_inputs = true` — so today a
  Unity session cannot produce a server-acceptable bundle. This is P0's first task (see
  `docs/sim-ffi-unity-gap.md`). `godot/views`: 104 `.gd`, 4 `.tscn`, ~26.9K lines (counts here are
  slightly off; magnitude holds).
- **UI decision: uGUI** (this document) over UI Toolkit (`docs/unity-stack-choices.md` §1). Reason:
  a solo dev working through Claude gets the most reliable results where documentation, Asset
  Store UI kits, DOTween integration and examples are densest — that is uGUI. Mitigate its
  canvas-rebuild cost by one Canvas per panel (disabled when hidden) and pooled list rows; revisit
  UI Toolkit only if inventory lists prove too slow.
- Everything else in `docs/unity-stack-choices.md` stands (URP 3D Renderer, alpha-clip actor
  sprites, Shuriken for the Hovl pack, NativeWebSocket, Newtonsoft, Steamworks.NET, custom CSV
  localisation, Cubism SDK, single-window companion).
