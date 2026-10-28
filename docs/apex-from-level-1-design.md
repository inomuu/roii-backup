# Apex from level 1 — the new stat-point system (design)

Design draft, 2026-09-11. The prerequisite (step 2) in `docs/exemplar-leveling-reconciliation.md`:
the stat rewrite removed the six attributes, so the old level-up allocation is gone and the current
Apex cannot be reused as-is. This is the replacement — a single point system that runs from level 1.

## Where the numbers come from now (so a "point" means something)

The stat rewrite (master) removed STR/INT/VIT/DEX/AGI/LUK entirely. `Stats` is now `{ level, xp,
apex… }` — no attributes. A unit's power comes from **the doctrine profile (base stats), gear,
level-derived formulas, and the Potential Board** (`stats.rs` module note: "not poured into
attributes"). So there is no attribute pool for a level-up to feed. The system needs a NEW thing for
a level-up to grant.

The current **Apex** is already that thing, minus the "post-cap only" restriction: 9 buckets of
FLAT bonuses — Might/Focus/Vigor/Guard/Precision/Evasion/Edge/Deadeye/Avarice (+ATK/+MATK/+HP/+hard
DEF/+HIT/+FLEE/+critDMG/+crit/+soul) — added on top of the derived pipeline. Flat and
attribute-independent is EXACTLY why it survived the attribute deletion. Bringing it forward is the
natural move the user named ("Apex เลื่อนมาตั้งแต่เลเวลอัพ").

## The proposal

**One allocation currency — Apex Points — earned from level 1, spent into FIVE buckets, one point
at a time.** The mechanism is the current `apex_unspent` / `allocate_apex(bucket, count)`
/ per-bucket cap / reset, unchanged in shape; what changes is WHEN points are granted and the
numbers. Level 1 → cap and post-cap become one continuous point track instead of two systems.

This keeps three properties the rest of the plan needs:
- **Attribute-free**, so it fits the new stats.
- **Reversible** (`allocate` has a `reset`), which is what feature/leveling's XP-refund needs to
  un-spend a unit's points when its levels are reclaimed.
- **Unified**, so there is not a level-1 stat system AND a separate post-cap one to balance apart.

## The buckets — TWO TIERS (settled, user 2026-09-11)

Surveying what combat actually consumes (not the stale Apex struct comments) and splitting by WHEN
a stat is earned:

### Tier 1 — CORE, earned level 1 → cap (the level-up allocation)

**ATK · HP · DEF · Crit · Crit DMG · Dodge** — the unconditional stats every build wants. A new
player only ever meets these six, so early allocation has no trap. All six are live in combat:
`effective_atk`, `effective_max_hp`, `effective_hard_def`, `effective_crit_chance_bp`,
`effective_crit_dmg_bp` (folds gear/card on top of the base ×1.5), `dodge_bp` (the accuracy/evasion
axis that replaced the cancelled HIT/FLEE). MATK is not here — `effective_matk` returns
`effective_atk` verbatim (0.0.330), so ATK covers physical and magic alike. Soft DEF (level/2) and
ASPD/cast/range are structural, not allocatable.

### Tier 2 — SUBSTATS, earned over-level / post-cap (the original Apex accrual)

**DEF pen · Break DMG · Break Buildup · Element DMG · Cooldown · Cast speed · Charge rate · Heal**
— the build-flavour stats, unlocked at max level and bought with the existing post-cap Apex points
(`apex_level` accrued from post-cap XP). This is what post-cap grinding PAYS (D5): not more of the
core, but access to deeper, more specialised allocation. All are live combat stats; most are also
gear-signature (slot-gated in `gear_roll`), which fits them arriving as the ADVANCED tier. This
REPLACES the old post-cap Apex's 9 same-as-core buckets.

Excluded from both tiers — the CONDITIONAL stats (`DmgVs Element/Race/Size`, `Resist Element`,
`Dr Vs Race/Size/Ranged`, `Resist Status`, `MoveSpeed`): they only pay against a specific target or
situation, and an idle party cannot pick its fights, so a point there is a trap, not a choice. They
stay gear-only, where a player equips them deliberately for a matchup.

Both tiers use the same allocate/cap/reset machinery, one point at a time, cap 20 per bucket, and
both must reset cleanly for the XP-refund (D6).

## Decisions to settle (options + a lean recommendation)

**D1 — Which buckets? — SETTLED: five (ATK/HP/DEF/Crit/Crit DMG), grounded in the current stats
(user 2026-09-11).** Reuse the existing Apex machinery, but only the buckets that map to a stat
combat still reads: MATK folded into ATK (0.0.330), HIT/FLEE replaced by `dodge_bp`, Soul removed as
a shared currency. See the buckets section above. (Corrects an earlier "same 9" that trusted stale
struct comments over the live damage path.)

**D2 — Apex points separate from Potential-Board points? — SETTLED: separate.** The two-tier
Apex (core + substat) is the "how strong" axis; the Potential Board is the "what build" axis with
its own point currency heading for an item faucet. Refund (option b) leaves the board alone, which
only holds if the pools are distinct. Three point pools in total, each with its own source: Tier-1
(level-up), Tier-2 (post-cap Apex), Board (its own / items).

**D3 — Grant curve — SETTLED: flat 1 point per level (`POINTS_PER_LEVEL`).** 69 points at the
launch cap of 70 against 120 to max all six core buckets, so the allocation stays a choice. The
per-point magnitudes were then TUNED (0.0.348) under one rule — a bucket at its cap of 20 ≈ one
top-tier gear line — against `--dump-stats 70` and the gear roll table. Final table in
`stats.rs::APEX_PER_PT` with the reasoning per row. **ATK and HP are FLAT by decision** (user
2026-09-11: a percentage is harder to tune); the measured cost is that base HP grows 40x level 1 →
cap while ATK grows 4.4x, so a flat HP point skews early-strong — HP is sized for the middle and is
the bucket to watch. Measured allocation on/off in sim-cli: +4.7% kills mid-game, more XP in every
early seed, no change at the cap where kills are spawn-gated.

**D4 — Per-bucket cap — SETTLED: 20 (user 2026-09-11), down from 50.** With the current per-point
values a fully-committed stat gives: ATK/DEF +200, HP +4000, Crit DMG +20%, Crit +2%. Five buckets ×
20 = 100 points to max everything, so the level-1→cap grant curve (D3) must total WELL under 100 by
the cap or allocation is not a choice. The per-point VALUES were
tuned for the old cap-50 post-cap top-up; validate them against cap-20-over-the-whole-game in
sim-cli — if a flat bonus reads as noise against late base+gear, the value moves, not the cap.

**D5 — Post-cap reward — SETTLED: SUBSTAT allocation (user 2026-09-11, "เอาแค่ substat ก่อน").**
Max-level grinding unlocks and pays the Tier-2 substat buckets (above) via the existing post-cap
Apex accrual — that is the "something else" the grind gives. The Engram idea (post-cap XP → reserve
→ other units) is set aside FOR NOW, not killed; it can be added alongside later. So post-cap has a
concrete reward and the merge no longer waits on this being open.

**D6 — Reset is a first-class, ALWAYS-available player action (user 2026-09-11: "มีปุ่ม reset ได้
ตลอด").** A free respec at any time: reset returns every spent point to its tier's unspent pool, both
tiers, no cost, no cooldown. This is stronger than the refund's internal need — it is a UI button the
player presses whenever. Two consequences for the build: (1) allocation must be pure and reversible
by construction (spent-per-bucket tracked, so reset = move all back), which the existing
`allocate`/`spent_on`/`reset` Apex machinery already provides; (2) it composes with the XP-refund —
reclaiming a unit's XP drops its level and un-grants exactly the points that level gave, and a manual
reset is the same operation the player can trigger directly. Deterministic per-level grant keeps both
exact.

## What this unblocks

With D1–D6 settled and built: master has a coherent progression again (level-up → Apex Points →
buckets; Board for build; post-cap → chosen reward), and feature/leveling's Engram/refund/resonance
can re-integrate onto a foundation that finally matches its granted-on-level-up assumption. Then the
merge deferred in the reconciliation doc becomes tractable.

## NOT in this design (named so scope stays honest)

Gacha + pity (acquisition), the item-based Potential Board faucet, and the actual balance numbers
(D3/D4 curves) — those are separate work after the shape here is chosen.

## Implementation architecture (how Apex feeds the stats)

Apex plugs into the ONE aggregation seam the game already has. `combat/stat_mods.rs::for_each_mod`
iterates every StatMod source on an entity (gear, exemplar, regalia, cards); every stat reader
(`dodge_bp`, `ignore_def_bp`, `break_dmg_bp`, and the summed `atk`/`hp`/`def`/`crit`) folds those.
So each Apex bucket maps to a **StatMod variant** (ATK→AtkFlat, HP→MaxHp, DEF→DefFlat, Crit→CritBp,
Crit DMG→CritDmgBp, Dodge→DodgeBp, DEF pen→IgnoreDefBp, Break→BreakDmgBp/BreakBuildupBp, Element
DMG→ElementDmgBp, Cooldown→CooldownBp, Cast→CastBp, Charge→ChargeRateBp, Heal→HealBp), a unit's
allocation emits `bucket_count × per_point` as those StatMods, and `for_each_mod` yields them
alongside gear. One integration point covers all ~14 buckets, and an unallocated unit emits nothing
— so every non-Apex golden stays byte-identical. The old `ApexBonus` flat struct and its six direct
`apex()` reads in `derived.rs` retire into this path.

Build stages (each compiles): (1) stats.rs — bucket→StatMod table, tier tag, per-point, cap 20, two
pools (Tier-1 level pool + Tier-2 apex_level), `apex_mods()` emitter, allocate/reset for both;
(2) for_each_mod yields `apex_mods()` and derived.rs drops the six apex() reads; (3) xp.rs grants
Tier-1 points on level-up; (4) events: allocate/reset (reset always) ; (5) goldens re-bless +
sim-cli tune the grant curve (D3).
