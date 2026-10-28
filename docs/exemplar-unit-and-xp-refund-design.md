> **RE-INTEGRATED ONTO MASTER — 2026-09-11 (sim 0.0.349–0.0.350).** This doc was written on
> `feature/leveling` against sim 0.0.313-0.0.318, before the stat rewrite and the Potential Board.
> The mechanics below now live on master with these ADAPTATIONS, which win where the text differs:
>
> - **Refund touches the BASE stream only** — level, bar, both Apex tiers' points and allocation.
>   Job level / job XP are NOT reset: job is the Potential Board's cap and decision (b)
>   (`docs/exemplar-leveling-reconciliation.md`) keeps a refund out of the board. `ResetSkills`,
>   `full_progression_reset` (§5.4-5.5) and the job stream of `ApplyEngram` are therefore not
>   ported; `base_progression_reset` is the master primitive. Engrams feed base XP only.
> - **Stat points are 0.0.347's two-tier Apex** (`level_points` + `apex_level`), not
>   `points_unspent`; `drain_base_xp` grants `POINTS_PER_LEVEL` per level and the refund resets
>   both tiers (a free respec exists anyway).
> - **A unit's class**: the Exemplar's own fixed-kit `player_class` when one exists (seraphine),
>   else `exemplar_allowed_classes[0]` (rowena → swordsman_t1).
> - **The carrier flow is RETIRED on master too** (§2 "1b-sim"): Equip/Unequip/FeedExemplar are
>   tombstones, `owned_exemplars` migrates to units/credits in `migrate_save_items`.
> - **Stars rise only by the player's hand** (`FeedExemplarUnit`); a dupe never auto-feeds.
> - Gacha + Mileage exist on the server (`verify-server/src/gacha.rs`); delivery lands through
>   `bank_collectible` → `grant_exemplar_unit`. The "gacha still to build" notes below are stale.
> - Godot: the bench (party_view) carries Refund / Fill; the Exemplar screen (U) lists units and
>   spends Resonance. Unity: PartyWindow + ExemplarUnitsWindow mirror them.

# Exemplar-as-Unit + XP Refund — sim/gameplay design

**Status:** DRAFT v3, 2026-09-04 (branch `feature/leveling`). Phase 0a shipped then revised —
`level_req` is STUBBED to 1 (no gate, plumbing kept), not removed (§5.7); **Phase 0 + 1a COMPLETE**
(sim 0.0.316, all green): level-up refactor, three high-water marks, `cum_*`, `ResetSkills`,
`full_progression_reset`, Exemplar-as-Unit grant + `pending_resonance`, and **1b-sim** (carrier flow
deleted, grant rerouted). Next: **1b-ui** (gacha-style team management — collection/formation/detail),
and **Phase 2 COMPLETE** (XP refund: `owned_engrams`, `RefundUnitXp`, `ApplyEngram`,
`ApplyEngramsToMax` + `engram_fill_preview`, sim 0.0.318). **The entire SIM side of the feature is
done** — Exemplar-as-unit, carrier removed, XP refund manual + auto + preview, all tested. What
remains is Godot: **1b-ui** (gacha team-management screens) + **Phase 3** (FFI read views for
`owned_engrams`/`engram_fill_preview` + deploy). v3 locks the
user's storage decision: **denominated Engram items** (not a pool), one item feeds base+job, in a
dedicated off-bag collection, overflow at the ceiling DISCARDED (not banked). Migration dropped
(pre-launch — saves are wiped). Remaining open item: the Engram name (§10-1).

**Scope of this session:** two coupled changes, shipped as ONE release (one `VERSION` bump, one
golden re-bless, one coordinated verify-server deploy):

1. **Exemplar == Unit** — every Exemplar becomes its own first-class roster unit with its own
   level/XP/stats/skills, instead of an equippable overlay on a class character.
2. **XP refund** — a fresh Exemplar-unit starts at Lv1 and cannot catch the party, so the player can
   **drain an old unit's progression into a bound XP reserve**, then **apply that reserve to another
   unit** — capped so no unit can ever be pushed above the level the account has legitimately
   reached through normal play.

Everything else (combat, skills, soul, gacha economy) stays as-is. Out of scope: the gacha economy
(`docs/gacha-economy-design.md`) and the Regalia weapon overlay (`docs/regalia-overlay-design.md`).

---

## 1. What exists today (verified against source, sim 0.0.313)

**Unit storage — a roster already exists, with ONE id space.**
- `GameState.roster: BTreeMap<u32, RosterChar>` (`state.rs:308`), keyed by **unique u32 char id**;
  **duplicate classes already allowed** ("own three swordsmen", `state.rs:299-308`).
  `RosterChar { unit_def, data: CharacterData }` (`state.rs:1060`).
- **Roster id == entity id.** `deploy` removes the roster entry and builds the grid entity with the
  SAME `char_id` (`party.rs:263-286`); `bench` finds `e.id == char_id` and re-inserts it
  (`party.rs:298-324`). A unit is either on the grid OR in the roster, never both, one id.
- `CharacterData` (`state.rs:956-991`): `stats`, `equipped`, `skills`, `card_panel`,
  `exemplar: Option<Exemplar>` (`:972`), `regalia`, consumables/provisions/ammo.
  `capture`/`restore_onto` (`:1017-1047`) move every per-character field between Entity and store.
- Legacy `class_characters` (`state.rs:297`) — single-lead `change_class` only. Not our path.

**Exemplar today = an equippable overlay carrier, never its own character.**
- `ItemKind::Exemplar` (`items.rs:52`); lean `Exemplar { id, name, skin_id, stars }` (`:100-111`)
  sits on `CharacterData.exemplar`; grants recomputed from `exemplar_def` (`:230`).
- Carrier collection `owned_exemplars: Vec<Item>` (`state.rs:506`) with `equipped_by`; synced onto
  the entity by `sync_member_exemplar` (`handlers.rs:1113`). Flow: `EquipExemplar` (`mod.rs:415`),
  `UnequipExemplar` (`:420`), `FeedExemplar { target_idx, feeder_idx }` (`:428`, dupe → ★).
- Granted skills materialise via `Entity::apply_exemplar_skill` / `clear_exemplar_skill`
  (`entity.rs:631/654`). **They cannot be touched by skill points:** `level_skill` gates on
  `tree_node(&e.unit_def, skill_id)` (`handlers.rs:290`) — only CLASS-TREE nodes are point-spendable.
  So "player-spent" vs "granted" is decidable by `tree_node(...).is_some()`; no flag needed.

**Stats** (`stats.rs:68-115`). Allocated stats baked into `str_..luk: u8`, **no per-level ledger** —
refundable = recomputed vs `base_stats` (`Stats::reset`, `:438-455`, refunds into `points_unspent`).
`points_unspent` (`:78`), `level` (`:80`), `xp` (`:84`), `job_level` (`:89`), `job_xp` (`:91`),
`skill_points` (`:94`), `apex_level` (`:110`), `apex: [u32; APEX_BUCKETS]` (`:114`).
New units: `Stats::new()` = Lv1 / `points_unspent 0`; `build_player_entity` sets
`skill_points = STARTING_SKILL_POINTS` (`party.rs:43`).

**Level-up is PURE on `stats` + one counter.** `try_level_up(state, idx)` (`xp.rs:636-674`) touches
only `stats.{level, xp, points_unspent, apex_level}` and `measures.levels_gained.add(1)` (`:672`);
no HP/derived recompute. `try_job_level_up` (`:713-729`) likewise (`job_level, job_xp,
skill_points`). Both take a GRID index — benched roster units are not reachable through them today.
**At the cap both DISCARD overflow:** apex cap → `xp = 0` (`:657-659`), job cap → `job_xp = 0`
(`:716-718`).

**Curves.** `XP_TABLE: [u64;99]` (`xp.rs:540`), `LEVEL_CAP=70` (`:608`); at cap the same `xp` bar
drains into Apex via `APEX_XP_TABLE` (`:684`), `APEX_LEVEL_CAP=50` — **base levels + Apex are one
continuous XP stream**. Job: `job_xp_to_next = 90j²+13j³` (`:705`), `JOB_LEVEL_CAP=50`.

**Respec today:** `ResetStats` (stats only, level untouched), `ResetApex` (buckets only). **No skill
unlearn. Level/XP never reset by any event.** Lifetime-counter discipline: `state.rs:766` — a respec
must never walk a lifetime counter backwards.

**High-water marks.** `band_reached` (`state.rs:257`, `.max()` in `zone.rs:141`) is the pattern.
**No max-level mark exists** — `levels_gained` counts events, `party_max_level` (`drops.rs:84`) is
current, not lifetime.

**Equip level gate (`level_req`) — STUBBED to 1 (user, 2026-09-04; see §5.7).** `Item::level_req()`
(`items.rs:1859`) is DERIVED from `item_level` (`(n−1)×10`, s2=Lv10 … s7=Lv60), never serialized.
Its one gameplay use is the equip refusal at `handlers.rs:946`, checked at equip time only, never
re-validated. Its own comment states the intent: it "only tempers market twinking" — it is not a
balance lever (`gear-atk-minor-vs-stats`). Other touch points: FFI `item_view.rs:86`, tooltip
`item_slot.gd:116-118` ("Requires Lv." — a raw literal, not through `Loc`), unit tests
`items.rs:2009` and `events/tests.rs:233`. No golden references it and sim-cli never checks it.

**No catch-up exists.** Recruit/buy = hard Lv1 (`party.rs:143-174`); the only gate is the lead-level
slot requirement.

**Authority.** `InputAuthority { Client, Server, Debug }`, `authority()` catch-all `_ => Client`
(`mod.rs:749`); verify-server mirror fails the build on an unverdicted `Server` variant.

---

## 2. Model — Exemplar == Unit

An Exemplar is a `RosterChar` whose `CharacterData.exemplar` is `Some`. Per
`exemplar-overlay-is-a-unit`: class = base stat block, so Exemplar-on-class already IS a unit; we
give it its own roster entry and its own progression instead of overlaying a shared class character.

- **unit ≡ Exemplar (bijection), confirmed (user, 2026-09-04):** every playable unit IS an Exemplar
  (SR minimum rarity — SR/SSR per `docs/gacha-economy-design.md`, `regalia-gacha-design`). No plain
  "class character" ships as a unit. Class survives as the ARCHETYPE (skill tree + role); a unit's
  class = its Exemplar line's base (t1) class (`exemplar_allowed_classes[0]`). Skills will be reworked
  later toward per-unit kits; for now the class-tree + Exemplar-grant system stands (user).
- `Exemplar { id, name, skin_id, stars }` unchanged; grants derive from `exemplar_def`,
  `apply_exemplar_skill` on deploy. Duplicate classes are legal in `roster` (two Exemplar-units of the
  same class coexist). Exemplar-units are BOUND (only ever in `roster`/grid).
- **Starting roster = 2 Exemplars from STORY MODE** (user) — the free base-class starters are removed.
  The 2 are TBD; **Seraphine + Lucielle are the placeholders** (the two with full kits + art). This is
  the SHIPPED new-game flow — deferred (needs story mode); NOT changed in Phase 1a.
- **Grant → unit [Phase 1a DONE `f325753`]:** `grant_exemplar_unit(state, data, bare_id)` — first copy
  → fresh Lv1 `RosterChar` (class `exemplar_allowed_classes[0]`, `exemplar: Some`, `Stats::new()`,
  `STARTING_SKILL_POINTS`); a dupe → `pending_resonance` (§3). `DebugGrantExemplarUnit { exemplar_id }`
  (Debug) is the dev/story grant stand-in; the server `mailbox_grant.rs` will route to the same fn (1b).
- **Acquisition model (user, 2026-09-05): GACHA ONLY.** A pull (RNG) and the PITY/spark exchange
  (accumulated gacha shards → a chosen unit) BOTH grant an Exemplar-unit via `grant_exemplar_unit`
  (a dupe → `pending_resonance`) — ONE acquisition concept, and the foundation already supports it.
  `buy_character`/`recruit_member` (Gil, plain-class) become **Debug-only**, demoted WHEN gacha ships —
  NOT before (until then recruit is the interim player path; demoting early would leave no acquisition
  route but `DebugGrantExemplarUnit`). The gacha + pity + shard-currency system itself is a SEPARATE
  deferred build (`docs/gacha-economy-design.md`, server-side); this branch only provides the grant
  primitive it calls.
- **Not a messy dual entity (arch review 2026-09-05):** `exemplar: Option` on the unit is a single
  owned field whose grants derive from the `exemplar_def` catalog (one source) — layering (class base
  + exemplar overlay), not two entities cross-referencing. Keep `Option` (engine/test substrate needs
  plain-class units); enforce "every unit is an Exemplar" at the grant/new-game level, NOT by making
  the type mandatory (that would break goldens/sim-cli for no real de-duplication).
- **Test substrate kept:** `build_player_entity` (plain class) stays for goldens/sim-cli/tests — only
  the shipped NEW-GAME DEFAULT roster changes (later), not the class-build mechanism.
- **Carrier flow removal [1b-sim DONE `4e540c8`]:** `owned_exemplars`, `sync_member_exemplar`,
  `EquipExemplar`/`UnequipExemplar`/`EquipExemplarTest`, carrier `FeedExemplar` + handlers — deleted;
  grant rerouted to `grant_exemplar_unit`; FFI `owned_exemplars_json`→`[]`. (`equipped_by` STAYS —
  Regalia shares it.) Godot exemplar-view rework = **1b-ui** (gacha-style team management, §9).

## 3. Resonance / dupes

**[Phase 1a DONE `f325753`].** A duplicate grant does NOT create a second unit — it credits
`GameState.pending_resonance: BTreeMap<String /*exemplar bare-id*/, u32>` (skip-if-empty). The NEW
event `FeedExemplarUnit { unit_id }` (Client) consumes one and raises `unit.exemplar.stars` by 1
(≤ `EXEMPLAR_MAX_STARS`), incrementing `exemplars_fed`, on a benched OR deployed unit. A counter, not
an item: no bag cap, no sell/list path can see it, bound by construction (`make-the-compiler-the-gate`).
(`FeedExemplarUnit` is a NEW event added alongside the carrier `FeedExemplar`, which Phase 1b removes.)

## 4. No migration

The game has not launched; existing saves are wiped, not migrated. Consequences:
- No `owned_exemplars` conversion, no old-save fallback for the new high-water marks (they simply
  start at the party's current max on a new game, i.e. 1).
- Removed `InputEvent` variants are deleted, not tombstoned.
- `VERSION` still bumps (shape change) and goldens re-bless (§8) — that is about determinism, not
  save compatibility.

---

## 5. Primitives (Phase 0)

Refund is only correct if lowering a unit's level repairs everything derived from it. There is no
ledger (§1), so each primitive recomputes consistency itself.

**5.1 High-water marks [DONE `ce6a072`]** on `GameState`, mirroring `band_reached`:
`level_reached: u32`, `apex_reached: u32`, `job_level_reached: u32`. Raised by `.max(new)` **only in
the combat level-up path** (`try_level_up`/`try_job_level_up`), never receding, never written by
refund/apply. `#[serde(default, skip_serializing_if = is_zero_u32_state)]`.

**⚠️ Two guards the Phase-2 apply path MUST honour** (audit 2026-09-04 — latent, not bugs yet
because nothing reads the marks): (a) **debug mutators bypass the marks** — `debug_add_levels` /
`debug_max_job_and_skills` set `level`/`job_level` directly without raising them, so a debug unit can
sit ABOVE its mark; (b) **old/parked saves default the marks to 0** even for a high-level unit. So the
ceiling MUST be read as **`max(stored_mark, current max level across ALL units)`**, never the raw
field — this both back-fills old saves and keeps debug units from being capped below themselves.

**5.2 Cumulative-XP helpers [DONE `d854ab3`]** (pure, `u64`, integer-exact, round-trip-tested):
- `cum_base(stats) = Σ XP_TABLE[1..level] + Σ apex_xp_cost[0..apex_level] + xp` — one stream.
- `cum_job(stats) = Σ job_xp_to_next[1..job_level] + job_xp`.
- **Invertible ONLY for a reconciled state** (`bar < requirement`; combat reconciles every kill). The
  apply path must never call `cum_*` on a hand-built/unreconciled state (audit note).

**5.3 Level-up refactor — one loop, two callers. [DONE `ee4b19a`]**
The body of `try_level_up` / `try_job_level_up` is extracted into pure free fns in `xp.rs` (where the
curve constants live), taking `&mut Stats`:
```
drain_base_xp(stats: &mut Stats, level_cap: u32, apex_cap: u32)
drain_job_xp (stats: &mut Stats, job_cap: u32)
```
- Drains the EXISTING `stats.xp` bar into whole levels while `level < level_cap` (granting
  `stat_points_for_level` per level), then, once `level >= LEVEL_CAP`, into Apex while
  `apex_level < apex_cap`. A partial remainder that cannot afford the next level stays in the bar —
  a legit partial bar, and only ever strictly BELOW the cap.
- **PURE on `stats`:** no `Measures`, no high-water marks, and it NEVER discards — leftover stays in
  the bar. Each caller decides the leftover's fate.
- **Both callers DISCARD at their cap (`xp`/`job_xp` = 0)** — the ceiling bar must be empty. A parked
  99% bar would become a free level the next time the cap rises (+10/month) = XP smuggled above the
  proven ceiling (user, 2026-09-04). Discard is a small XP SINK, not a conservation hole: the
  invariants that matter are no-mint and never-exceed-the-ceiling (§7).
  - **Combat** (`try_level_up`, grid): caps = `LEVEL_CAP` / `APEX_LEVEL_CAP`; zeroes `xp` at the apex
    cap; adds the base-level delta to `levels_gained`; raises the three marks (Phase 0b).
    Byte-identical to pre-refactor (verified: 39 tests, golden drift 0).
  - **Apply** (§6, roster): caps = `level_reached` / `apex_reached` (base) and `job_level_reached`
    (job); zeroes the bar once the unit sits at its ceiling; touches NO `Measures`, NO mark.
Same curve, two paths → they cannot drift.

**5.4 `ResetSkills { entity_id }` [DONE `d854ab3`]** — a NEW Client event.
`Entity::reset_learned_skills` removes every PLAYER-LEARNED skill (`!from_exemplar && !from_regalia`
— a complete definition, audit-confirmed: a built unit starts with no skills and the only point-free
grants are not stored in `entity.skills`) and **refunds Σ their levels into `skill_points`** (each
level cost one point) — NOT a reset to STARTING; that is the full-reset's job (§5.5). Exemplar/Regalia
grants are kept. Cleans the loadout (`active_skills`) and drops any in-flight `casting` (mirror of
`clear_exemplar_skill`). Doubles as a standalone skill respec.

**5.5 Full progression reset [DONE `d854ab3`]** — `build::full_progression_reset(&mut CharacterData)`,
a helper used by refund (not an event): `level=1, xp=0, apex_level=0, apex=[0;…], job_level=1,
job_xp=0`; `Stats::reset` (stats → `base_stats`) **then `points_unspent = 0`** (reset refunds into the
pool, but Lv1 owes nothing); `skills.clear()` (a benched unit holds only learned skills — grants were
stripped at `capture`); `skill_points = STARTING_SKILL_POINTS`. Benched-only (§5.6).

**5.6 Roster-only.** All refund/apply primitives operate on `roster[id].data` (benched units).
`try_level_up`'s body is pure on `stats` (§1), so this needs no grid; derived stats recompute on
deploy through the existing `build_player_entity` + `restore_onto`. Resetting a LIVE entity
mid-tick (HP above new max, buffs keyed to old stats) is exactly what this avoids.

**5.7 Stub `level_req` to 1 — do NOT remove it** (user, 2026-09-04, revising the v2 "remove it"
call). `Item::level_req()` returns **1 for every item**: no effective equip gate (every character is
≥ Lv1), but the plumbing (fn, the `handlers.rs` check, the FFI key) STAYS, so re-enabling a real gate
is a one-line formula change. Shape-neutral (derived, never serialized) → rides 0.0.314, **golden
drift ZERO** (a gate at req=1 blocks no equip; if a golden shifts, some entity was sub-Lv1 —
investigate, don't bless, `dont-weaken-the-test`).

*Why not remove it (the v2 plan):* removing the gate outright left **only top-tier gear with value**
— with no level gate, demand collapses onto the best base and mid-tier drops lose their level-matched
buyers (user). *Why not restore the real `(item_level−1)×10` gate either:* the gear economy needs a
proper rethink (is mid-tier demand load-bearing? what else gives it value?) — a separate econ pass,
not forced by this work. Stub-to-1 is the honest middle: no regression, no premature econ commitment.

*Why the gate isn't needed for catch-up anyway:* a new unit is levelled by Engrams FIRST, then it
equips gear at its (now high) level — the gate never blocks the intended flow. "Equip top gear while
still Lv1" was an over-solution.

**No auto-unequip on refund** (user): a drained unit KEEPS its gear/cards. Forcing an unequip would
dump gear into a possibly-full stash with nowhere to go. Nothing is moved, so nothing is lost, and
with the gate stubbed a Lv1 wearing high-iLv gear is legal anyway.

**[REC]** Ceiling scope is **account-wide** (a new unit catches up to the best any unit ever reached).

---

## 6. XP refund — drain → Engram items → apply

Chosen shape (user, 2026-09-04): the refund breaks a unit's progression into **denominated Engram
items** (working name, §10-1) the player holds in a dedicated collection and applies later.

**Item. [DONE `1d75688`]** Four fixed tiers — **1,000 / 10,000 / 100,000 / 1,000,000 XP** (`ENGRAM_TIERS`,
index 0..3). Implemented as `GameState.owned_engrams: [u32; 4]` — per-tier COUNTS, **no `ItemKind`**
(a fixed-denomination consumable needs no per-instance identity), **NOT in the flat `stash`/`inventory`** (user: "เก็บเป็นไอเทมส่วนตัว ไม่รวมในกระเป๋า") — so no sell/list/vendor path
can address it and no bag cap applies (bound by construction; `inventory-tabs-are-visual-filters`).
**One Engram feeds BOTH streams:** on use its face value is granted to the base XP bar AND the job
XP bar (user: one value covers both, mirroring how a monster kill awards base+job together). The
future server EXP-ticket can grant Engrams through the same path.

**Event `RefundUnitXp { unit_id }`** — authority **Client** (mints nothing; replay-exact).
1. Unit must be in `roster` (benched) — no-op otherwise.
2. `total = cum_base(stats)` (base levels + Apex). The refund quantity is derived from BASE level XP
   ONLY — job is not separately counted (user); job is reset away and re-earned via the Engrams.
3. Break `total` greedily into Engram tiers, largest-first; credit `owned_engrams`. The sub-1,000
   remainder is dropped — a tiny sink (§10-A).
4. Full progression reset (§5.5) — base AND job. Gear/cards stay on the unit (no gate, §5.7). The
   unit remains in the roster at Lv1, playable.

**Event `ApplyEngram { unit_id, tier, count }`** — authority **Client**.
1. Unit in `roster`; `count` clamped to `owned_engrams[tier]`.
2. `v = TIER_VALUE[tier] * count`. Base: `stats.xp += v; drain_base_xp(stats, level_reached,
   apex_reached)`; if the unit is at its base ceiling, `xp = 0` (discard overflow). Job:
   `stats.job_xp += v; drain_job_xp(stats, job_level_reached)`; if at the job ceiling, `job_xp = 0`.
3. Decrement `owned_engrams[tier]` by `count`. Touches no `Measures`, no high-water mark (§7).

Overflow past a ceiling is DISCARDED, not refunded (§5.3, user-B). Because the job curve is short,
a large Engram maxes job (to `job_level_reached`) and discards the rest — intended (a caught-up unit
should have its skills).

**Event `ApplyEngramsToMax { unit_id }`** — the "fill to max" convenience (user, 2026-09-04).
Authority **Client**. The sim greedily pours `owned_engrams` into the unit until the unit hits its
base ceiling (`level_reached`) or the bag is empty. Tier selection minimises waste: spend the
largest tier that does NOT overshoot the remaining base headroom first, then smaller tiers; only when
nothing but oversized tiers remain does it spend one and eat the discard (the user allows reaching
the ceiling even at a cost). Same per-Engram mechanics as `ApplyEngram` (both streams, discard at
ceiling). Deterministic → replay-safe.

**Confirm + warning are CLIENT UX, but the waste figure is SIM-computed** (GDScript must not compute
gameplay outcomes). Add a pure read `engram_fill_preview(state, unit_id) -> { engrams_by_tier,
resulting_level, xp_wasted }` (FFI, Phase 3) the UI queries BEFORE acting: it shows the result,
**warns when `xp_wasted` is large** (threshold is client display tuning — `dont-overspecify`), and
requires an explicit **confirm** before firing `ApplyEngramsToMax`. The event re-derives the same
result, so the preview and the commit cannot disagree.

`ApplyEngram` (manual, pick tier+count) also stays — a player topping off precisely wants it, and its
own UI can warn on a single over-tier apply. Build both in Phase 2 (events) + Phase 3 (preview/UI).

**Cost.** Free in v1 (§10-6); a `gil_cost` hook is kept for later tuning as a sink.

---

## 7. Invariants (the spine)

- **No minting.** Engrams are created ONLY by `RefundUnitXp` draining a unit's real, provable XP;
  nothing else credits `owned_engrams`. XP is otherwise conserved except two acknowledged SINKS: the
  sub-1,000 refund remainder (§10-A) and overflow discarded at a ceiling (§5.3, user-B). Neither can
  create XP; both only ever LOSE a little.
- **The ceiling is inviolable.** `level ≤ level_reached`, `apex_level ≤ apex_reached`,
  `job_level ≤ job_level_reached` after any apply. Only the combat path raises the marks. Pooling
  many units' Engrams into one cannot fabricate a unit beyond the account's proven peak.
- **The ceiling level's bar is empty.** `ApplyEngram` lands a unit at exactly the ceiling with a
  ZERO bar (`xp`/`job_xp`/apex bar = 0); overflow is DISCARDED, never a parked partial bar — which
  would become a free level the moment the cap rises (§5.3). A partial bar is legal only strictly
  BELOW the ceiling. *Acceptance:* apply a huge Engram to a Lv1 unit with `level_reached=N` → unit
  ends at Lv N with `xp=0`.
- **Refund/apply never touch `Measures` or any lifetime counter.** Without this, drain A→B, drain
  B→A repeats for free (XP is conserved) and farms `levels_gained` — every daily/quest keyed on it
  is exploitable. Generalise: *any* future conserved transfer must be invisible to `Measures`.
- **Bound by construction.** `owned_engrams`, `pending_resonance`, and Exemplar-units live in
  dedicated fields no trade/sell/list path can address (`make-the-compiler-the-gate`).
- **Lifetime counters never walk backward** (`state.rs:766`).
- **Total & replay-safe:** roster lookup, silent no-op on a missing id / benched-check / equipped
  guard / catalog skew; never panic; all `u64`; **no PRNG draws** (refund is not a random outcome).

---

## 8. Cross-cutting impact

- **`sim_core::VERSION`** 0.0.313 → 0.0.314-alpha (`GameState` shape: three marks, `owned_engrams`,
  `pending_resonance`, removed `owned_exemplars`; new `ItemKind::Engram`). Changelog entry in `lib.rs`.
- **Goldens move once, in Phase 0:** `level_reached`/`job_level_reached` serialise non-zero in every
  golden that levels up (all of them). `apex_reached` stays 0 (no golden reaches cap) → skipped.
  Confirm the drift is exactly those fields, then `tools/bless_goldens.sh`. Phase 1's deletions must
  NOT move them (no golden touches an Exemplar — verify by grep). Phase 2 events are unused in
  goldens → no drift.
- **Combat byte-identical:** the §5.3 refactor must leave `try_level_up`'s observable behaviour
  unchanged (same consumption order, same cap-discard). A moved golden in Phase 0 beyond the new
  fields = the refactor is wrong, not "expected drift" (`dont-weaken-the-test`).
- **Deploy in lockstep** — sim-version skew silently rejects online saves. `tools/deploy.sh` +
  `tools/check_online_sync.sh` with the client build.
- **verify-server:** the two new `Client` events replay like any input; `mailbox_grant` grants a
  unit / a `pending_resonance` credit instead of a carrier. No new `Server` verdict needed.
- **FFI / Godot:** roster view surfaces Exemplar-units; an `owned_engrams` view; refund/apply/feed
  through `apply_input_json`; loc keys for the Engram tiers and the two flows. UI must confirm before
  `RefundUnitXp` (destructive), enforce the bench precondition, and warn on Engram overflow (§6).

---

## 9. Build order

| Phase | Work | Goldens |
|---|---|---|
| **0a** ✅ | ~~remove~~ **stub** `level_req` to 1 (§5.7) — plumbing kept, no gate `aaeba64` | ZERO |
| **0b** ✅ | high-water marks · `cum_*` · level-up refactor · `ResetSkills` · full-reset helper `ee4b19a`/`ce6a072`/`d854ab3` | moved (marks) — blessed |
| **1a** ✅ | grant-as-unit + `DebugGrantExemplarUnit` · `pending_resonance` + `FeedExemplarUnit` (additive, carrier untouched) `f325753` | ZERO |
| **1b-sim** ✅ | delete carrier flow in sim + FFI (`owned_exemplars`/Equip/Unequip/carrier `FeedExemplar`/handlers) · reroute grant→`grant_exemplar_unit` · `owned_exemplars_json`→`[]` `4e540c8` | ZERO (drift 0) |
| **1b-ui** | **gacha-style team management** (user): collection (roster) + formation (deploy/bench) + character-detail (level/resonance via `FeedExemplarUnit`) — rework `exemplar_view`/`character_view`/`sim_driver`, drop carrier equip UI. Sim already supports it (needs UI design/mockup). Shipped new-game roster = 2 story Exemplars (needs story mode) | — |
| **2-core** ✅ | `owned_engrams: [u32;4]` (off-bag counts, NO ItemKind) · `RefundUnitXp` (bench-only) · `ApplyEngram` (both streams, ceiling=`mark.max(current)`, discard overflow, no Measures) `1d75688` | ZERO |
| **2-auto** ✅ | `ApplyEngramsToMax` (greedy waste-min fill) · `engram_fill_preview`→{plan, resulting_level, resulting_apex, xp_wasted} · `xp::cum_at` `e9cd714` | ZERO |
| **3** | FFI views + `engram_fill_preview` · Godot roster/engram/refund/apply UI (fill-to-max confirm + waste warning) · loc · deploy client + server together | — |

Green `cargo test -p sim-core --tests` (+ `cargo test -p verify-server`) before each next phase.
Ship 0→3 as one release.

---

## 10. Open decisions (confirm before Phase 0)

1. **Storage.** RESOLVED (user): denominated **Engram** items (1k/10k/100k/1M), one item feeds both
   base+job, in a dedicated `owned_engrams` collection outside the bag (§6). Name **Engram** is a
   working pick (steampunk / semi-robot / FFXIV feel — "extract a veteran's engrams, imprint a new
   unit"); alts: Aether Cell/Core, Anima, Memory Core. **Confirm the final name.**
2. ~~Equipped guard / remove `level_req`~~ **RESOLVED (user): stub `level_req` to 1** (no gate now,
   plumbing kept), and **no auto-unequip** on refund (§5.7). Gear economy (mid-tier value) is a
   deferred, separate design pass — not blocking this work.
3. **Apex in the drain.** **[REC]** included — base+Apex are one stream, so a capped unit can fully
   reset; fill is capped at `apex_reached`. *Alt:* v1 refuses refund FROM a unit at `LEVEL_CAP`.
4. **Ceiling scope.** **[REC]** account-wide. *Alt:* per-class.
5. **Pending dupes.** **[REC]** `pending_resonance` counter + manual `FeedExemplar`. *Alt:* auto-★.
6. **Apply cost.** **[REC]** free in v1, hook kept.
7. **Roster-only for apply.** **[REC]** yes in v1 (one code path). Applying to a DEPLOYED unit is
   also safe (levelling live is what combat does) — extend later if the UX wants it.

---

## 11. Audit trail + known-minor items

Two independent adversarial audits (both read-only, both green): **Phase 0** (`8207315..d854ab3`) and
**Phase 1a/1b/2** (`d854ab3..HEAD`, 2026-09-05). Verdict both times: conservation (no mint), ceiling
inviolable, discard-at-ceiling correct, refund/apply touch no `Measures`/marks, determinism intact
(no floats/PRNG/HashMap), Regalia intact. Fixes applied from the audits: the phantom `xp_wasted` on an
at-ceiling unit (`engram_fill_preview` guards `granted==0 → 0`).

**Known-minor, deferred (not defects for this feature):**
- `try_grant_item` returns `false` for an `ItemKind::Exemplar` (no `data` to build a unit) → an
  Exemplar mailed as a FULL ITEM instance would soft-lock that mail. **Unreachable by design** —
  Exemplars are granted via the `def_id` path (`try_grant`) / gacha, never as a full-item instance.
- `next_char_id` (pre-existing, unchanged) excludes pets when taking the max id — a pet holding the
  highest id could collide with a new roster unit. Pre-dates this work; flagged for a future sweep.

---

**Related memory:** `exemplar-overlay-is-a-unit`, `make-the-compiler-the-gate`,
`inventory-tabs-are-visual-filters`, `monetization-positioning`, `dont-weaken-the-test`,
`guard-threshold-off-the-cliff`, `competition-integrity`, `input-authority-security`.
