# sim-ffi vs. SimBridge — the FFI gap for a Unity client

Audit date 2026-09-05. Scope: what a Unity host can drive **today** through `crates/sim-ffi`
(plain C ABI) versus what the Godot client drives through `crates/godot-ext` (gdext).
This is a surface audit only — it is not a port plan (see `docs/unity-port-plan.md`) and no
source file was modified.

Headline: **104 `#[func]` methods on `SimBridge`; 4 have an exact C-ABI equivalent, 1 is
partially covered, 99 are missing.**

---

## 1. `SimBridge` surface (`crates/godot-ext/src/`)

All parameters are Godot Variant types (`i64`, `GString`, `bool`); JSON is the only compound
type. Every method is total — no panics, bad input returns `false` / `-1` / `"{}"` / `"[]"` /
last-good state.

### 1a. Lifecycle — `lib.rs` (11)

| # | Line | Signature | Purpose | sim-core call |
|---|---|---|---|---|
|1|95|`init_game(seed:i64, game_data_json:GString, zone_id:GString) -> bool`|New game from catalog + start zone; enables input recording + snapshot chain|`data::GameData::from_json_str`, `data::new_game_in_zone`|
|2|125|`tick(n:i64)`|Advance n ticks (1 tick = 100 ms) through the shared driver step|`replay::advance`|
|3|145|`view_state() -> GString`|THE read interface: trimmed + enriched state JSON, `"{}"` when uninitialised|`view_state::build_view_state`|
|4|162|`state_hash() -> GString`|Canonical hex state hash, `"no_state"` when empty|`hash::state_hash`|
|5|172|`sim_version() -> GString`|`sim_core::VERSION` — the online version gate|const|
|6|182|`sim_git_stamp() -> GString`|`SIMGIT:<sha>` build identity (read by `check_online_sync.sh`)|const|
|7|194|`save_json() -> GString`|Raw `GameState` JSON for save-to-disk|`serde_json::to_string(&state)`|
|8|209|`export_log() -> GString`|Replayable `SaveBundle` (start checkpoint + input log + claimed hash); stashes a pending re-anchor|`replay::SaveBundle::capture`|
|9|232|`commit_anchor()`|Apply the pending re-anchor after the server confirms the save|`replay::reanchor`|
|10|253|`replay_span() -> i64`|Ticks the next bundle would replay (`-1` = no game)|state field arithmetic|
|11|266|`load_json(save_json, game_data_json) -> bool`|Load a save, migrate items to the current catalog, restart recording|`serde_json::from_str::<GameState>`, `data::migrate_save_items`|

### 1b. World / party / class — `bridge_world.rs` (37)

| Line | Signature | Purpose | sim-core call |
|---|---|---|---|
|22|`create_character(seed:i64, zone_id, unit_def, name) -> bool`|Rebuild as a solo lead of a class with a party name|`data::new_game_as`|
|54|`classes_json() -> GString`|Selectable classes + base stats for the creation chart|catalog read|
|80|`current_class() -> GString`|Lead's class id (sprite selection)|state read|
|90|`party_name() -> GString`|Party identity string|state read|
|108|`set_player_class(unit_def) -> bool`|Swap lead class|`events::apply_input_with` (ChangeClass)|
|130|`respawn_in_current_zone() -> bool`|Reset zone, refill HP, keep escrow|`InputEvent::RespawnInZone`|
|144|`restart_instance() -> bool`|Wipe → restart instance at room 1|`InputEvent::RestartInstance`|
|158|`leave_instance() -> bool`|Leave a cleared dungeon|`InputEvent::LeaveInstance`|
|171|`current_zone() -> GString`|Authoritative current zone id|state read|
|186|`select_band(band:i64) -> bool`|Warp to a band's room 1|`InputEvent::SelectBand`|
|208|`recruit_member(unit_def) -> bool`|Hire a member for Gil|`apply_input_with` + `data::party_size`|
|227|`recruit_cost() -> i64`|Next recruit price (`-1` = party full)|`data::recruit_cost`|
|240|`player_class_ids_json() -> GString`|Recruitable class ids|catalog read|
|256|`advance_options_json(entity_id) -> GString`|2-1 options for a grid entity|catalog read|
|273|`advance_options_for_class_json(unit_def) -> GString`|Same, keyed by class (benched roster)|catalog read|
|289|`advance_class(entity_id, target_class_id) -> bool`|Job advancement|`InputEvent::AdvanceClass`|
|315|`buy_character(unit_def) -> i64`|Buy a benched character; returns new id or `-1`|`apply_input_with`|
|338|`buy_character_cost() -> i64`|Next purchase price|`data::buy_character_cost`|
|348|`deploy_character(char_id) -> bool`|Bench → party|`apply_input_with`|
|366|`bench_character(char_id) -> bool`|Party → bench|`apply_input_with`|
|383|`roster_json() -> GString`|Benched characters list|state read|
|405–572|**16 debug methods**: `debug_fake_clear(failed)`, `debug_blocked()`, `set_debug_allowed(bool)`, `debug_set_hp_pct(id,pct)`, `debug_kill(id)`, `debug_inflict_statuses(id)`, `debug_set_god(on)`, `debug_god()`, `debug_set_level(id,lv)`, `debug_grant_apex(id,n)`, `debug_add_levels(id,n)`, `debug_add_skill_points(id,n)`, `debug_max_job_and_skills(id)`, `debug_advance(id,class)`, `debug_give_gear(id)`, `debug_give_currency(gil,soul)`|Dev cheats. Five of them mutate `GameState` **outside** `apply_input`, so they are gated by `debug_allowed` (false while online) — an ungated one corrupts replay verification|`data::debug_*`, direct field writes|

### 1c. Items / economy — `bridge_items.rs` (35)

| Line | Signature | Purpose | sim-core call |
|---|---|---|---|
|19|`sell_all_junk() -> i64`|Sell banked junk, returns Gil gained|`InputEvent::SellAllJunk`|
|33|`max_listing_price() -> i64`|Market per-unit price cap|`mailbox_grant::MAX_LISTING_PRICE`|
|38|`pending_junk_gil() -> i64`|Gil the banked junk would fetch|`events::pending_junk_gil`|
|44|`add_consumable(entity_id, stash_idx)`|Slot a consumable (cap 3)|`InputEvent::AddConsumable`|
|58|`remove_consumable(entity_id, slot)`|Unslot back to stash|`InputEvent::RemoveConsumable`|
|74|`shop_items_json()`|Buyable catalog|`data::instantiate_item`|
|104|`item_preview_json(def_id)`|One def as a full item view|`data::instantiate_item` + `item_to_view`|
|117|`all_cards_json()`|Full card dex|catalog scan|
|133|`bind_info_json(entity_id, album_idx)`|Card-bind confirm data|state read|
|163|`buy_item(def_id, qty) -> bool`|Buy for Gil|`InputEvent::BuyItem`|
|182|`grant_loot(def_id) -> bool`|Grant loot into stash/rare bag|`InputEvent::GrantLoot`|
|197|`grant_loot_foil(def_id) -> bool`|Debug foil grant|`apply_input_with`|
|214|`sell_stash_item(stash_idx) -> i64`|Sell one item|`InputEvent::SellInventoryItem`|
|226|`discard_item(inv_idx) -> bool`|Destroy a stack (only exit for bound items)|`InputEvent::DiscardItem`|
|236|`expand_inventory(inv_idx) -> bool`|Use an Expander for +1 slot|`InputEvent::ExpandInventory`|
|247/262/276/307/356/370/439|`inventory_json`, `owned_exemplars_json`, `owned_regalia_json`, `stash_json`, `rare_bag_json`, `card_album_json`, `escrow_json`|On-demand list reads, deliberately kept OUT of `view_state` (payload size)|state reads + `item_to_view`|
|291|`debug_grant_regalia(id) -> bool`|Dev regalia grant|`items::regalia_def`|
|320/329/338/347|`deposit_to_stash`, `withdraw_from_stash`, `deposit_card_to_album`, `withdraw_card_from_album`|Bag ↔ warehouse ↔ album moves|matching `InputEvent`s|
|402|`card_special_vals_json(def_id, star)`|Card special display values|`combat::card_specials::display_values`|
|413|`upgrade_card(album_idx, copies)`|Star up|`InputEvent::UpgradeCard`|
|426|`feed_card(target_idx, feeder_idx)`|Star/foil transfer|`InputEvent::FeedCard`|
|461|`refine_odds_json(item_level, level)`|Published refine odds|`refine::odds_for`|
|478|`refine_odds_enriched_json(item_level, level)`|Enriched-ore odds|`refine::odds_for_enriched`|
|494|`recipes_json()`|Craft catalog + learned flags|`craft::output_is_rolled_gear`|
|555|`start_refine_inventory(inv_idx)`|Refine un-equipped gear|`InputEvent::StartRefine`|
|579|`orb_check_json(orb_idx, target_json)`|Star-stone applicability preview|`orbs::orb_applies` / `orb_gil_fee`|

### 1d. Stats / skills / input — `bridge_stats.rs` (21)

| Line | Signature | Purpose | sim-core call |
|---|---|---|---|
|20|`toggle_active_skill(entity_id, skill_id)`|Enable/disable a slotted active|`InputEvent::ToggleActiveSkill`|
|36|`dps_json()`|DPS meter snapshot|`skills::skill_display`|
|77|`lifetime_stats_json()`|Lifetime measures + records|state read|
|120|`reset_dps()`|Reset the meter|`InputEvent::ResetDps`|
|131|`skill_info_json(skill_id)`|One out-of-tree skill card|`skills::skill_def`|
|213|`skill_tree_json(unit_def)`|Class tree nodes|`skills::unit_kit_tree`|
|325|`reserved_sp(_entity_id) -> i64`|Always 0 (legacy shim)|—|
|331|`sp_reserve_capacity(entity_id) -> i64`|Slot capacity|`combat::sp_reserve_capacity`|
|340|`sp_reserve_used(entity_id) -> i64`|Slots requested|`skills::loadout_slot_cost`|
|354|`xp_to_next(level) -> i64`|Base XP curve|`tick::xp_to_next`|
|361|`job_xp_to_next(job_level) -> i64`|Job XP curve|`tick::job_xp_to_next`|
|374|`effective_stats_json(entity_id)`|All derived combat stats|`combat::effective_*`, `refine::refine_*_bonus`|
|458|`allocate_apex(entity_id, bucket, count) -> bool`|Spend Apex points|`InputEvent::AllocateApex`|
|473|`reset_apex(entity_id) -> bool`|Refund Apex|`InputEvent::ResetApex`|
|484|`apex_buckets()`|Static bucket metadata|`sim_core::stats`|
|504|`stat_cost(value) -> i64`|Gil cost to raise a stat (`-1` at cap)|`stats::Stats::cost_to_raise`|
|514|`loadout_json(entity_id)`|Active loadout + cooldowns|`skills::skill_def`|
|542|`buffs_json(entity_id)`|Buffs aggregated by kind|`skills::StackPolicy`|
|606|`soul_paths_json()`|Paths of Soul web|`soul_paths::is_owned` / `is_allocatable`|
|**662**|**`apply_input_json(event_json) -> bool`**|**The one generic write channel** — parses `sim_core::events::InputEvent` JSON and dispatches|`events::apply_input_with`|
|679|`bestiary_json()`|Bestiary + milestones|`data::bestiary_bands::BandScale::build`|

### 1e. Non-`#[func]` behaviour a host must honour

- **Totality.** Every method is a no-op / `false` / `-1` / `"{}"` on missing state or bad input.
  `view_state()` returns `"{}"`, never `""` (`lib.rs:145` — an empty string made a real malformed
  payload indistinguishable from the pre-load case).
- **Version gate.** `sim_driver_core.gd:406-411` compares the server's `sim_version` against
  `bridge.sim_version()` **at login** and blocks the session on mismatch. `sim_git_stamp()` is the
  build-identity twin used by `tools/check_online_sync.sh`.
- **State JSON shape.** `view_state()` is **not** `GameState`: `view_state.rs` starts from
  `serde_json::to_value(&state)`, then **trims** (`zone_spawn` to 5 fields — 45 KB of a 74 KB
  payload, and it also stopped shipping drop tables/weights to the client; `stash`/`inventory`/
  `rare_bag`/`card_album`/`escrow` to `*_count`) and **injects** derived keys (`eff_max_hp`,
  `mon_name`, `sprite`, `recent_hits`, `heal_fx`, `skill_fx`, `knockbacks`, `field_hits`,
  `poison_bursts`, `meteor_impacts`, `trap_fires`, caps, commander charges). GDScript reads these
  keys via `.get(key, default)` in ~1,439 places, so a lost key renders empty rather than erroring;
  `contract_keys_are_stable` in `view_state.rs` pins the key set.
- **Input shape.** `apply_input_json` takes the default serde **externally-tagged** enum form:
  `{"EquipItem":{"entity_id":0,"inventory_idx":1}}`. `InputEvent` has 65 variants
  (`crates/sim-core/src/events/mod.rs:91`). Several fields are `skip_serializing_if` at their
  zero/false default (`server_roll`, `enriched`, seals/coupons) so old logs stay byte-identical.
- **Replay contract.** `apply_input_with` records the event into `state.input_log` *before*
  dispatch (`events/mod.rs:775`). Anything that mutates state outside this path breaks
  `export_log()` verification — which is exactly why `debug_allowed` exists.

---

## 2. `crates/sim-ffi` today

`Cargo.toml`: `crate-type = ["cdylib", "staticlib"]`, deps `sim-core` + `serde_json`, no build
script, **no cbindgen config and no generated header** — the C surface is hand-maintained and
undocumented outside the rustdoc in `src/lib.rs` (165 lines total).

| Line | Exported symbol | Signature | Notes |
|---|---|---|---|
|44|`roi_new`|`Sim* roi_new(uint64_t seed, const char* game_data_json, const char* zone_id)`|NULL on null arg, non-UTF-8, unparseable catalog, or unknown zone. Boxes `Sim { state, data }`. Does **not** set `record_inputs` / `snapshot_interval` (unlike `init_game`)|
|66|`roi_tick`|`void roi_tick(Sim*, uint64_t n)`|No-op on NULL. Calls `tick::tick` **directly**, not `replay::advance` — the pending-zone-change driver step is missing|
|77|`roi_state_json`|`char* roi_state_json(Sim*)`|**Raw `GameState`** serde, i.e. equivalent to `save_json()`, not `view_state()`. NULL on NULL handle or serialisation error. Caller frees|
|87|`roi_sim_version`|`char* roi_sim_version(void)`|`sim_core::VERSION`. Caller frees. The only non-`unsafe` export|
|96|`roi_string_free`|`void roi_string_free(char*)`|`CString::from_raw` + drop. NULL-safe, single-free only|
|107|`roi_free`|`void roi_free(Sim*)`|`Box::from_raw` + drop. NULL-safe, single-free only|

**Ownership.** Every returned `char*` is a Rust `CString::into_raw` allocation and **must** be
returned to `roi_string_free` — never `free()`, never `Marshal.FreeHGlobal`. Input `const char*`
are borrowed for the duration of the call only. The `Sim*` is an opaque owning handle;
`roi_free` exactly once.

**Errors.** Out-of-band only: NULL pointer, or nothing at all (`roi_tick` silently no-ops).
There is no error code, no error string, no "why did that fail" channel. `to_c()` also returns
NULL if the JSON ever contained an interior NUL.

**Threads.** `Sim` is neither `Send` nor `Sync`-annotated and there is no internal locking; the
contract is one thread per handle, no re-entrancy (calling back into `roi_*` from inside a
callback is undefined — there are no callbacks today, so this is only a forward-looking rule).
Multiple independent `Sim` handles on different threads are fine (sim-core has no globals).

**Panic safety.** No `catch_unwind` anywhere in the crate. The stated guarantee is *totality*
(no path can panic) plus `panic = "abort"` in `[profile.release]` (`Cargo.toml:26`), so a release
build aborts instead of unwinding across `extern "C"`. **A dev/debug build unwinds** and is
therefore UB across the boundary — Unity must ship the release dylib.
Note also `overflow-checks = true` in release: an integer overflow in sim-core is a panic, which
under `panic = "abort"` takes the whole host process down.

The crate's two tests (`the_c_surface_drives_a_real_sim`, `null_and_garbage_are_refused_not_panics`)
cover exactly the six functions above.

---

## 3. Build result (macOS, this machine)

```
cargo build -p sim-ffi --release      # 16.86s (cold sim-core rebuild), exit 0
```

Two warnings, both pre-existing and harmless:
`sim-core/src/tick/upkeep.rs:791` constant `SOURCE_BAND` never used; `sim-ffi/src/lib.rs:18`
field `Sim.data` never read (the catalog is held for future data-dependent calls — exactly the
calls that are missing, §4).

Artifacts: `target/release/libsim_ffi.dylib` (1,087,568 B) and `libsim_ffi.a` (7,083,200 B).
`file` → `Mach-O 64-bit dynamically linked shared library **arm64**` (single-arch, not universal).

```
$ nm -gU target/release/libsim_ffi.dylib | grep -i roi
_roi_free  _roi_new  _roi_sim_version  _roi_state_json  _roi_string_free  _roi_tick
```

Six exports, matching the source exactly — no accidental leakage of Rust symbols.

Windows x64 (NOT run here). `tools/export_windows.sh` cross-builds **`godot-ext`** only; the
equivalent for this crate is:

```
cargo xwin build -p sim-ffi --target x86_64-pc-windows-msvc --release
# prereqs per export_windows.sh: rustup target add x86_64-pc-windows-msvc;
# cargo install cargo-xwin; brew install llvm lld  (llvm-lib + lld-link, both keg-only)
```

---

## 4. Gap table

| Group | SimBridge funcs | In sim-ffi | Missing |
|---|---|---|---|
| Lifecycle (`lib.rs`) | 11 | 4 (`init_game`→`roi_new`, `tick`→`roi_tick`, `sim_version`→`roi_sim_version`, `save_json`→`roi_state_json`) + `view_state` PARTIAL | 6 |
| World/party (`bridge_world.rs`) | 37 | 0 | 37 |
| Items (`bridge_items.rs`) | 35 | 0 | 35 |
| Stats/skills/input (`bridge_stats.rs`) | 21 | 0 | 21 |
| **Total** | **104** | **4 exact + 1 partial** | **99** |

`view_state` is "partial" because `roi_state_json` returns the *raw* `GameState`: it is missing
every injected derived key (`eff_max_hp`, `sprite`, `mon_name`, the per-tick FX arrays) and ships
the untrimmed `zone_spawn` + full stash/inventory (the 74 KB-per-frame payload the Godot path
deliberately trims to ~29 KB). A Unity renderer would either re-derive gameplay values in C#
(a rule violation) or be blind to combat FX.

### Proposed additions — five functions cover ~95 of the 99

The 99 are not 99 distinct C entry points. Sorted by mechanism:

| # | Proposed C signature | Covers | Rust work |
|---|---|---|---|
| A | `char* roi_view_json(Sim*)` | `view_state` (properly) | Move `godot-ext/src/view_state.rs` + `item_view.rs` into a renderer-agnostic crate (or a shared module) and call it. ~40 lines of new FFI + a file move; the risk is keeping ONE copy so the two bridges cannot drift. **3–5 h** |
| B | `int32_t roi_apply_input(Sim*, const char* event_json)` (1 = applied, 0 = parse error / no state) | ~30 evented mutators + `apply_input_json` | Mirror `bridge_stats.rs:662`. **~20 lines, 0.5 h** |
| C | `char* roi_query(Sim*, const char* query_json)` — one dispatcher over the read-only getters, e.g. `{"q":"skill_tree","unit_def":"mage_t1"}`, returns `"{}"`/`"[]"` on an unknown query | ~40 JSON getters (`inventory_json`, `skill_tree_json`, `effective_stats_json`, `bestiary_json`, `soul_paths_json`, …) + the ~8 scalar getters (returned as JSON numbers) | The bodies already exist but are written against `GString`; each needs porting to `serde_json::Value`. Same drift risk as A — the honest fix is to move these bodies out of `godot-ext` and have both bridges call them. **~600 lines moved, 12–20 h** |
| D | `char* roi_save_json(Sim*)`, `int32_t roi_load_json(Sim*, const char* save, const char* game_data)`, `char* roi_state_hash(Sim*)`, `char* roi_sim_git_stamp(void)`, `char* roi_export_log(Sim*)`, `void roi_commit_anchor(Sim*)`, `int64_t roi_replay_span(Sim*)` | the 6 remaining lifecycle funcs + the online save path | Direct mirrors of `lib.rs`; `roi_new` must additionally set `record_inputs` + `snapshot_interval` and keep a `session_start` clone, and `roi_tick` must call `replay::advance`, or exported bundles will not verify. **~120 lines, 3–4 h** |
| E | `int32_t roi_debug(Sim*, const char* cmd_json)` + `void roi_set_debug_allowed(Sim*, int32_t)` | the 16 debug funcs | Only worth it for a dev build; keep the `debug_allowed` gate or online saves stop verifying. **~80 lines, 2 h** |

All five keep the existing guarantees: total functions, no unwinding (`panic = "abort"` in
release), JSON in / JSON out, caller frees with `roi_string_free`. If a debug/unwinding profile
is ever shipped, wrap each body in `std::panic::catch_unwind` and return the null/zero sentinel
— under `panic = "abort"` `catch_unwind` is inert, so it is documentation, not protection.

Two additions worth making at the same time, both Unity-driven:
`size_t roi_state_json_into(Sim*, char* buf, size_t cap)` (write into a caller-owned buffer to
avoid a malloc + managed-string alloc 10×/s), and a non-zero error code channel
(`int32_t roi_last_error(Sim*)`), because today "it did nothing" and "it succeeded" are
indistinguishable for `roi_tick`.

---

## 5. C-ABI pitfalls specific to Unity (Mono / IL2CPP)

1. **Strings are UTF-8, C# strings are UTF-16.** Use `Marshal.PtrToStringUTF8` (not
   `PtrToStringAnsi`, which is code-page-dependent on Windows and will corrupt any non-ASCII item
   name). For inputs, marshal explicitly as `byte[]` with a trailing NUL, or set
   `[MarshalAs(UnmanagedType.LPUTF8Str)]`. Default `string` marshalling on Windows is ANSI.
2. **Never let the marshaller own a returned pointer.** Declare returns as `IntPtr`, copy with
   `PtrToStringUTF8`, then call `roi_string_free`. Declaring the return as `string` makes the
   runtime try to `CoTaskMemFree`/`free` a Rust allocation — heap corruption, and it will not
   crash at the call site.
3. **Calling convention.** `extern "C"` on Windows x64 is a single convention, but be explicit:
   `CallingConvention.Cdecl`. IL2CPP is stricter than Mono about mismatched blittable types —
   use `ulong`/`long`/`IntPtr`, never `bool` (marshals as 4-byte Win32 `BOOL` by default; the
   proposed functions return `int32_t` for this reason).
4. **Pointer lifetime.** `Sim*` outlives the managed side; `SafeHandle` or a `sealed class`
   with a finalizer is safer than a bare `IntPtr` field, because a domain reload will drop the
   C# object without freeing the Rust box (see 6).
5. **Re-entrancy / threading.** One `Sim` handle, one thread. Do not call `roi_tick` from a Unity
   Job or `Task` while the main thread reads state. There is no lock inside.
6. **Domain reload is the Unity analogue of the "never hot-reload the dylib" rule.** In the Godot
   client, reloading the gdext dylib double-inits and SIGABRTs, so `reloadable = false` and a new
   dylib means a full relaunch. Unity's failure mode is different but related: the **Editor never
   unloads a `DllImport`ed native plugin** for the process lifetime, so (a) rebuilding
   `libsim_ffi.dylib` has no effect until the Editor restarts — the same "new dylib ⇒ relaunch"
   discipline applies; and (b) a C# domain reload (entering play mode, recompiling a script)
   wipes managed statics while the **native** `Sim` box survives — every cached `IntPtr` becomes a
   leaked, unreachable sim. Free every handle in `OnDisable`/`OnDomainUnload`, and re-create
   rather than re-attach. There is no double-init hazard here (no `ExtensionLibrary` entry point,
   just plain functions), which is one thing the C ABI genuinely makes easier than gdext.
7. **Aborts kill the Editor.** With `panic = "abort"` + `overflow-checks = true`, a sim bug takes
   the Unity Editor down with no managed stack trace. Ship the release dylib (a dev build
   unwinding across `extern "C"` is UB), and log `roi_sim_version` at startup so a version-skew
   crash is identifiable.
8. **Platform builds.** macOS Unity wants a universal bundle:
   `cargo build -p sim-ffi --release --target aarch64-apple-darwin` +
   `--target x86_64-apple-darwin`, then `lipo -create -output libsim_ffi.dylib …`; today's
   artifact is arm64-only. Windows x64 via `cargo xwin` (§3) produces `sim_ffi.dll` — note the
   name has **no `lib` prefix**, so `[DllImport("sim_ffi")]` (no prefix, no extension) is the only
   spelling that resolves on both platforms.
9. **Payload size.** `roi_state_json` at 10 Hz means ~74 KB of JSON serialised, marshalled and
   GC-allocated per tick, before Unity has parsed it. The Godot path already trims to ~29 KB in
   `build_view_state`; add A and the buffer variant before benchmarking anything.

---

## 6. `SimFfi.cs` — P/Invoke for the surface that exists today

```csharp
using System;
using System.Runtime.InteropServices;

/// Raw bindings to crates/sim-ffi (libsim_ffi.dylib / sim_ffi.dll).
/// EXISTING exports only — see docs/sim-ffi-unity-gap.md §4 for what is missing.
internal static class SimFfi
{
    // No "lib" prefix, no extension: resolves to libsim_ffi.dylib on macOS, sim_ffi.dll on Windows.
    private const string DLL = "sim_ffi";

    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr roi_new(
        ulong seed,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string gameDataJson,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string zoneId);

    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    internal static extern void roi_tick(IntPtr sim, ulong n);

    /// Raw GameState JSON (== SimBridge.save_json, NOT view_state). Caller frees.
    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr roi_state_json(IntPtr sim);

    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    internal static extern IntPtr roi_sim_version();

    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    internal static extern void roi_string_free(IntPtr s);

    [DllImport(DLL, CallingConvention = CallingConvention.Cdecl)]
    internal static extern void roi_free(IntPtr sim);

    /// Copy a Rust-owned UTF-8 string and hand the allocation back. Never free it any other way.
    internal static string TakeString(IntPtr p)
    {
        if (p == IntPtr.Zero) return null;          // null IS the error channel today
        try { return Marshal.PtrToStringUTF8(p); }
        finally { roi_string_free(p); }
    }
}

/// Owning wrapper: the native Sim outlives a C# domain reload, so it must be freed explicitly.
internal sealed class Sim : IDisposable
{
    private IntPtr _h;
    public bool Valid => _h != IntPtr.Zero;

    public Sim(ulong seed, string gameDataJson, string zoneId)
        => _h = SimFfi.roi_new(seed, gameDataJson, zoneId);   // IntPtr.Zero on bad catalog/zone

    public void Tick(ulong n) { if (Valid) SimFfi.roi_tick(_h, n); }
    public string StateJson() => Valid ? SimFfi.TakeString(SimFfi.roi_state_json(_h)) : null;
    public static string Version() => SimFfi.TakeString(SimFfi.roi_sim_version());

    public void Dispose()
    {
        if (_h == IntPtr.Zero) return;
        SimFfi.roi_free(_h);        // exactly once
        _h = IntPtr.Zero;
        GC.SuppressFinalize(this);
    }
    ~Sim() => Dispose();
}

// ---------------------------------------------------------------------------
// NOT YET EXPORTED — do not uncomment until crates/sim-ffi grows them (§4):
//   [DllImport(DLL, ...)] static extern IntPtr roi_view_json(IntPtr sim);                     // A
//   [DllImport(DLL, ...)] static extern int    roi_apply_input(IntPtr sim, string eventJson); // B
//   [DllImport(DLL, ...)] static extern IntPtr roi_query(IntPtr sim, string queryJson);       // C
//   [DllImport(DLL, ...)] static extern IntPtr roi_save_json(IntPtr sim);                     // D
//   [DllImport(DLL, ...)] static extern int    roi_load_json(IntPtr sim, string save, string gd);
//   [DllImport(DLL, ...)] static extern IntPtr roi_state_hash(IntPtr sim);
//   [DllImport(DLL, ...)] static extern IntPtr roi_sim_git_stamp();
//   [DllImport(DLL, ...)] static extern IntPtr roi_export_log(IntPtr sim);
//   [DllImport(DLL, ...)] static extern void   roi_commit_anchor(IntPtr sim);
//   [DllImport(DLL, ...)] static extern long   roi_replay_span(IntPtr sim);
//   [DllImport(DLL, ...)] static extern int    roi_debug(IntPtr sim, string cmdJson);         // E
// ---------------------------------------------------------------------------
```

## UPDATE 2026-09-05 — implemented (commit 42a6d18)

- New crate `crates/sim-host` holds `view_state` + `item_view` + the session lifecycle; `godot-ext`
  and `sim-ffi` both delegate to it (drift is now structurally impossible for the lifecycle).
- `sim-ffi` exports 16 symbols: `roi_new`, `roi_new_as`, `roi_load_json`, `roi_free`, `roi_tick`
  (via `replay::advance`, recording armed), `roi_apply_input` (serde `InputEvent` JSON),
  `roi_view_json` (the real view), `roi_save_json` (+ `roi_state_json` alias), `roi_state_hash`,
  `roi_export_log`, `roi_commit_anchor`, `roi_replay_span`, `roi_sim_version`, `roi_sim_git_stamp`,
  `roi_string_free`. Gap rows A, B, D and the lifecycle set are closed; **C (`roi_query` read-only
  getters: inventory/skill tree/effective stats/…) and E (debug) remain open** — port those bodies
  into sim-host when the Unity views need them.
- Verified inside Unity 6000.0.83f1 (`unity/ROI`, `Assets/Editor/SimFfiProbe.cs`, batchmode):
  600 ticks change the hash, input applies, view 33 KB vs save 190 KB, bundle 76 KB, replay span
  600→0 after commit, load round-trips the hash. C# binding: `Assets/Scripts/Sim/SimFfi.cs`
  (IntPtr returns + `roi_string_free`), `SimSession.cs`, `SimDriver.cs` (10 Hz accumulator).
