# Reconciling feature/leveling (Exemplar-unit + Engram) with master (Potential Board)

Analysis, 2026-09-11. Answers "leveling ยังไม่อยู่หรอ / master ทำงานยังไง" and scopes the merge
BEFORE any rebase. Settled design context (user, 2026-09-11): **Exemplar-as-unit is official; the
`class` is retired as a unit identity; a loose class-like grouping called `doctrine` replaces it.**

## ✅ DONE (2026-09-11 evening): re-integrated onto master, sim 0.0.349–0.0.350

The chain below played out in order the same day: new stats (done earlier) → Apex from level 1
(0.0.347-348) → XP refund stages 1-4 (0.0.349: drain/ceiling/Engrams, events, hosts, both
clients) → Exemplar-as-unit stage 5 (0.0.350: grant + Resonance credit + player feed, delivery,
carrier flow retired with a load migration, both clients). Nothing was merged from the branch as
commits — every piece was re-implemented against the current foundation, which is what this doc
recommended. `feature/leveling` is now history only. The live design doc is
`docs/exemplar-unit-and-xp-refund-design.md` (with its master-adaptation banner).

## ⚠️ VERDICT (as of 2026-09-11 morning, superseded above): KEEP the branch, DEFER the merge

A trial merge + a field check overturned the optimistic read below. master did not merely
auto-spend stat points — it **DELETED the `points_unspent` field entirely** (git grep finds no
definition anywhere on master). feature/leveling is built ON that field: 22 refs in `stats.rs`, 5
in `party.rs`, 3 in `xp.rs`, plus events/tests. A straight merge does not compile.

And the ground is still moving: the stat-point system master removed **is to be rebuilt, but is not
yet planned** (user, 2026-09-11). So the foundation feature/leveling assumes is gone AND its
replacement is undesigned.

**Decision:** keep feature/leveling as a branch (pushed) + its design doc, and do NOT merge now.
Merging would either reintroduce the just-deleted stat-point code or force adapting to a target
that has not been designed — either way, work redone later. Re-integrate AFTER the new stat-point
system is planned, so the "what does XP buy" accounting is reconciled once against the final model.
What survives regardless of the stat model — and is the real value to preserve — is the ontology
(already on master), the Engram XP-refund mechanism, resonance/stars, the high-water caps, the
Godot unit-management screen, and `docs/exemplar-unit-and-xp-refund-design.md`.

The analysis below is kept for the parts that still hold (ontology converged, roster shared, the
potential/ module orthogonal) — but its "one careful hunk / medium risk" conclusion is SUPERSEDED
by this banner: the stat-point deletion makes it a re-implementation against an undesigned
foundation, not a merge.

## TL;DR

master already HAS a complete, working leveling system — XP → level-up → Potential Board → Apex —
built by the other session while feature/leveling sat unmerged. feature/leveling is NOT "the
leveling"; it is a SEPARATE, complementary axis: how a gacha UNIT levels, banks resonance, and
refunds XP. The two belong together (the user is right), and the good news from the code is that
they were built on the SAME ontology and the SAME roster, so the merge is a bounded reconciliation
— not a redesign. There is exactly ONE real design decision, at the XP↔points boundary.

## What master's leveling is, today (0.0.346)

| layer | where | how it works |
|---|---|---|
| XP → level-up | `tick/combat_step/xp.rs`, `93ded54f` | a level-up grants its points and AUTO-SPENDS them by a fixed per-doctrine `stats::growth_order`. The player no longer allocates. |
| Potential Board | `crates/sim-core/src/potential/` + Unity screen | a per-unit skill-point constellation (`e.potential`), gated by `doctrine_for_unit(unit_def)`; already Exemplar-aware (`from_exemplar` skills). |
| Apex | `stats.rs` (`apex_unspent`/`allocate_apex`) | post-cap progression, 9 buckets. |
| + | | Toughness scales with level; fixed starter kits at rank one. |

So the game has full leveling WITHOUT feature/leveling. Nothing is broken.

## What feature/leveling adds (20 commits, unmerged, sim+FFI+Godot foundation done)

An **Exemplar-as-unit** layer, built "PURELY ADDITIVE" (0.0.315 — a party with no Exemplar stays
byte-identical, all goldens unmoved). It adds:

- **Engram / XP-refund** (`RefundUnitXp`, `ApplyEngram`, `owned_engrams` — an off-bag bound
  XP-item reserve, tiers [1k,10k,100k,1M]). Reclaim a unit's XP, pour it into another.
- **Resonance** (`pending_resonance`, `FeedExemplarUnit { unit_id }`) — a duplicate Exemplar grant
  banks +1 resonance instead of making a second unit; feeding raises `stars`.
- **Account high-water marks** (`level_reached`/`apex_reached`) — the refund/apply path caps a
  unit's poured progression at the highest any unit reached through COMBAT, so refund can never
  push a unit past earned progress.
- Godot Phase 3b: the Exemplar screen rebuilt as unit management.
- SoT: `docs/exemplar-unit-and-xp-refund-design.md`.

## The reconciliation, file by file

**The two big risks are already GONE:**

- **Ontology CONVERGED.** master keys the board by `unit_def` values that are unit names
  (`"seraphine"`, `"rapis"`) grouped by `Doctrine` (Sentinel/Striker). That IS exemplar-as-unit +
  loose-class-as-doctrine — the same model feature/leveling built to. Neither has to be talked out
  of its identity model.
- **Roster is SHARED, not colliding.** Both declare the identical `pub roster: BTreeMap<u32,
  RosterChar>` with `RosterChar { unit_def, data, … }` from a common ancestor. feature/leveling's
  units live in the roster master already has.

**The conflict surface** (both branches edited these; kind matters):

| file | master did | feature/leveling did | kind |
|---|---|---|---|
| `tick/combat_step/xp.rs` | auto-spend via `growth_order` (`93ded54f`) | +229: XP-refund path | **design** — see below |
| `state.rs` | +potential, roster tweaks | +engram/resonance/high-water fields | additive, mechanical |
| `grid/entity.rs` | potential fields | unit fields | additive, mechanical |
| `events/mod.rs`,`handlers.rs` | board actions | refund/feed events; −146 in handlers | textual, mechanical |
| `items.rs` | board/items | Engram items | textual, mechanical |
| `potential/**` | the whole module | **untouched** (0 refs) | none — orthogonal |

## The one real design decision: XP-refund × master's model

feature/leveling was built before the Potential Board and before auto-spend. Its refund reclaims a
unit's XP and levels. On today's master, two things changed under it:

1. **Stat points auto-spend by `growth_order`.** This HELPS: the old manual-allocation model made
   "which points do we un-spend on refund" ambiguous; a deterministic growth order reverses
   cleanly. The refund logic just needs re-pointing at `growth_order` instead of the old grant path.
2. **The Potential Board is new, per-unit, skill-point-based.** Refunding a unit's XP now raises a
   question feature/leveling never had to answer: **does a refund reset the unit's Potential
   Board?**

   **SETTLED (user, 2026-09-11): option (b) — a refund does NOT touch the Potential Board.** XP
   refund moves XP and stat levels only; the board persists. The board is meant to be advanced by a
   SEPARATE ITEM system ("ใช้ไอเทมอัพแทน"), which is NOT built yet — a future feature, out of scope
   for this merge. This is the cleanest reconciliation available: the refund path reverses stat
   levels through `growth_order` and never reads or writes `e.potential`, so no board-reset logic is
   written and the `potential/` module stays exactly the orthogonal, untouched surface it already is.

   (The rejected options, recorded so the decision is not re-litigated: (a) refund strips the board
   back to base — makes refund a full "recycle this unit"; (c) board points refund into a separate
   pool. Both were dropped in favour of the item-based board upgrade.)

Everything else (resonance, engram reserve, high-water caps, the roster) is additive and merges
without a design question.

## Godot screens — two views of one unit, not a conflict

feature/leveling's "Exemplar screen as unit management" (roster, level, resonance, refund) and
master's Potential Board screen are complementary: one is "which units I own and how they grow,"
the other is "this unit's stat/skill constellation." Keep both; the unit-management screen links
into the board.

## The build order — a dependency CHAIN, not a checklist (user 2026-09-11)

The merge is not the next action; it sits at the END of a chain, because each link changed the
ground under the one before it.

1. **New stat model** — IN PROGRESS on master. The six attributes were removed; stats are now
   level-derived / authored / flat. This is the deletion that stranded `points_unspent`.
2. **New Apex system, rebuilt on the new stats** — REQUIRED NEXT, and it cannot reuse the current
   one: today's Apex buckets pay flat bonuses onto the OLD attribute-derived pipeline, and "stat
   เปลี่ยนใช้แบบเดิมไม่ได้" (user) — the old Apex does not fit the new stats. The direction is to
   build it as the level-1 point system above (Apex-style allocation from level 1, not only
   post-cap), on the new stat foundation.
3. **THEN re-integrate feature/leveling** — its Engram/refund/resonance land on a foundation that
   now matches its granted-on-level-up assumption. Resolve `xp.rs` against the new grant model
   (not the deleted `points_unspent`), refund reversing levels only, never `e.potential` (option
   b). Wire the post-cap → Engram reward here if chosen, and the Godot unit-management screen beside
   the Potential Board screen.
4. **Separate, still unbuilt:** the item-based Potential Board upgrade the refund decision leans
   on. (CORRECTION 2026-09-11: gacha IS built — `verify-server/src/gacha.rs`, four endpoints, Unity
   `SummonWindow` + Godot `gacha_view`; the locked design has NO pity ladder by choice, using
   flat 4% SSR + a Mileage exchange instead. feature/leveling's docs predate it.)

So "should we merge feature/leveling" resolves to "not until steps 1–2 exist." The branch waits on
a NEW Apex built on the NEW stats; only then does its own model fit again.

## The coming stat-point system (direction, user 2026-09-11) — and why it eases the merge

master removed `points_unspent` as a step in a stat rewrite whose REPLACEMENT is now sketched:
**bring the Apex allocation model forward to start at LEVEL 1, not post-cap.** A level-up grants
points, and the player spends them one at a time (the existing `allocate_apex(bucket, count)` model
— unspent pool minus allocated, per-bucket caps, replay-safe silent no-op) from level 1 onward,
instead of only after the cap.

This matters for the deferred merge: it means the successor to `points_unspent` will be a
GRANTED-ON-LEVEL-UP, MANUALLY-SPENT pool — conceptually what feature/leveling already assumes.
feature/leveling's flat RO-style pool (`stat_points_for_level`, 1225 points 1→99) is not identical
to the Apex-bucket model, so its granting side will be swapped for bucket allocation; but its
XP-REFUND mechanism is indifferent to which — it reclaims XP and reverses levels, and the point
grant reverses with the level whichever shape it takes. So when the new stat system lands, the
re-integration is more tractable than against today's points-deleted master, which is a further
reason to KEEP the branch rather than discard it: it is waiting for a foundation that is moving
back toward its own assumptions, not away from them.

**Post-cap stays a grind incentive, but its REWARD is open (user 2026-09-11).** With Apex-style
points now earned from level 1, hitting the cap can no longer just grant "more of the same." The
system is kept — a maxed unit should still have a reason to keep fighting — but what it PAYS at the
cap is the open question ("อาจจะได้อย่างอื่นแทน?").

One fit worth flagging because it closes the loop with feature/leveling: **post-cap XP converts to
the Engram reserve** (`owned_engrams`, the bound XP-items feature/leveling already defines), which
pours into OTHER units. Then grinding a maxed unit is never wasted — it levels the bench. That
turns the very branch this doc is about into the answer to the post-cap-reward question, and is a
reason the two systems want to ship together. Other candidates: post-cap feeds Potential Board
points (deepen the build), or the meta `soul` currency. Undecided — listed, not chosen.

STILL A DIRECTION, NOT A SPEC — the buckets, the per-level grant amount, the post-cap reward, and
how a level-1 Apex pool interacts with the Potential Board (also point-based) are unplanned. Those
are the design the merge waits on.

## Risk

Medium, and lower than the raw file-conflict count implies: converged ontology + shared roster +
additive-by-design mean the merge is mostly append plus one careful xp.rs reconciliation gated on
one design decision. Not a rewrite.
