# Potential Board — functional prototype

Date: 2026-09-10. Scope: idle project, `master`.

The user chose to implement functionality before graphics and to **use existing
Skill Points for now; material costs come later**. This replaces the earlier
proposal to start with Growth Stones and Gil. The
[visual direction study](potential-board-visual-direction.md) remains a layout
reference, not a record of implemented economy or release-ready UI.

## Current rules

- Each stat node, major trace and individual skill rank costs **one Skill Point**.
  The sim checks the complete quote before spending; invalid requests do nothing.
- The prototype uses `ceil(Job Level / 5)`, clamped to tiers 1–10:
  Job 1–5 is tier 1, 6–10 is tier 2, and 46–50 is tier 10.
- A skill upgrade is capped by its actual skill definition and Job tier.
  Major traces A/B/C require Job tiers **2 / 4 / 6**.
- Commander Level is **not a gate in this version**. The generic gate query has
  a Commander tier input, but the action handler deliberately leaves it open.
  This is not a client-authoritative substitute for the future account gate.
- Unlocking a new node requires one connected unlocked neighbour. An already
  learned skill may continue ranking without repurchasing its route.
- “Upgrade toward major” buys the affordable prefix of a deterministic shortest
  route. It stops before the first unaffordable or unavailable node and never
  spends on an incomplete destination's path.
- Reset returns only the Skill Points recorded as paid to this board and restores
  the original skill ranks recorded before the board changed them.

The fixed graph contains **17 purchasable positions plus a free root**. The full
edge transcription and its source ambiguities are documented in the visual study.
Basic is the disconnected position with the source's two-major condition, but it
is not purchasable until its upgrade formula is authored.

## Supported character identity

User decision, 2026-09-10: the class system is cancelled and the starting pair
is **Seraphine + Rapis**. `unit_def` is still the legacy internal field name.
Historical Swordsman saves do not define the new starter roster. The temporary
Swordsman board mapping was removed, including from the staged native plugin.

The initial slice supports the actual fixed-kit **`unit_def = "seraphine"`** and
**`unit_def = "rapis"`**. These map to Sentinel and Striker respectively.
The Doctrine type also names Destroyer, Catalyst and Support for future authored
boards; their presence in the type does not make every existing class supported.

An equipped Exemplar with the same name is not the same identity. In particular,
the existing `exemplar_seraphine` overlay on a legacy Swordsman still supplies the
older Holy kit. It must not silently acquire the new Physical Seraphine board by
matching only the overlay's ID. Unsupported characters have no purchasable board;
they do not inherit placeholder skills or passives.

The source specimen's stat allocation is shared by this prototype. Final
per-Doctrine stat distributions remain future tuning work; the current sample is
not evidence that a complete five-Doctrine balance pass has happened.

## Existing progression and reset safety

Legacy skill points, learned skills and skill trees were still present in the
actual sim before this change. They were not deleted despite wording in the older
handoff. The board credits existing learned ranks as graph entry points and quotes
the next real rank, so a character with a level-5 skill is not charged to learn
level 1 again or downgraded by an upgrade.

`PotentialState` records:

- `ranks`: purchased node/skill ranks;
- `spent_points`: only the points actually spent through the board;
- `base_skill_levels`: the original rank before the board first changes a skill.

Empty fields are omitted from serialization and have defaults when loading older
saves. The state is part of both a live Entity and parked `CharacterData`, with
capture/restore keeping it across bench/deploy and character changes.
Selected learned active skills now park with the character too, so a purchased
active still casts after redeployment; deliberately unslotted skills stay that
way. Existing overlay regrant behavior is preserved. Iron Edge's cached bonus is
initialized immediately when a character is restored.

This slice does not bulk-migrate historical skill-point spending. Uninvested
characters retain the legacy `LevelSkill` behavior so existing saves continue
working. Once a character has board investment, upgrades of its mapped board
skills through that legacy input route into the board's quote and ledger. This
prevents a later reset from erasing separately paid ranks or bypassing the new
rank cap on an invested board.

Reset leaves unrelated skills and Exemplar/Regalia grants intact. It refuses while
the character is casting or if refunding would overflow the point balance.
Repeated reset cannot mint points: an empty paid ledger has nothing to return.
Skills first learned through the board retain a dormant rank-zero record on
reset. This preserves cooldown/charge history when repurchasing; the dormant
skill is unslotted and cannot be manually slotted until learned again.

## Incomplete content is visible, but cannot be charged

| Character | Node | Reason it is unavailable |
|---|---|---|
| Both | Basic (`k4`) | No authored rank-scaling formula; the design also retains older free-Basic wording. |
| Seraphine | C — Second Wall | Gauge refund amount and trigger limit are undecided. |
| Rapis | A — Cinder Rush | Stack duration and reset rules are undecided. |
| Rapis | C — Kindling | Flat gauge gain versus a gain multiplier needs clarification. |

The available authored major effects are Iron Edge (ATK from 25% of current DEF),
Iron Wall (party damage received −10%) and Cinderhide (Fire damage +10% while Flame
Shield remains intact). The latter must recognize Flame Shield specifically,
rather than treating another skill's absorption shield as its condition.

Existing **Bulwark** and **Ember Cascade** stay in their kits. Neither is silently
renamed Basic or removed to make the graph's four skill positions fit.
Fresh characters do not receive these two old passives automatically, and the
prototype board has no purchase position for them. Their acquisition is a
remaining kit-design/UI gap; this slice is not complete fixed-kit progression.
An already-active shield from an old save has no skill provenance. It remains
valid for absorption, but Cinderhide starts recognizing Flame Shield after its
next cast.

## Integration boundary

The Rust `potential` module owns the node definitions, explicit edges, stable
route planning, supported content and tier gates. Its action layer owns the
point quote, mutations and reset ledger. Combat consumes purchased modifiers and
major effects. The host exposes those authoritative results for the Unity UI;
Unity must not reproduce purchasing rules or compute combat bonuses.

The reusable graphic layer can replace the prototype presentation later without
changing node IDs, connections, saved ranks or the input contract. The paid-item
economy and trusted Commander entitlement must be designed and integrated before
they replace the point-only rules; neither is faked in the current save state.

Unity opens the functional node graph from **Skills / K**, using the existing party
member tabs. Select a node to see its real rank, cost and refusal reason, then
upgrade one rank or buy toward an available major. Reset requires a second click
and shows the refundable point total. The graph uses the authoritative node
coordinates and links from the host. Its current shapes and glyphs are a neutral
prototype skin that can be replaced without changing gameplay or save data.

## Validation status

Completed on 2026-09-10:

- `cargo test -p sim-core --lib`: **525 passed, 1 pre-existing ignored diagnostic**.
  The first integrated run caught the new `potential:` query in the ID-shape
  guard. Its protocol prefix now has an explicit allowlist reason, matching the
  existing `stats:` query. The complete core unit suite passed after that fix.
- The sim-core integration suite: **39 passed**, including golden scenarios,
  anchor roundtrips and actual old saves on this machine. **No golden re-bless**.
- Full sim-host **15 passed**, sim-ffi **4 passed**, verify-server **242 passed**.
  These are local tests; they do not constitute a live database/deployment check.
- All **28 new Potential tests** passed. Deliberately broken graph gates, combat
  hooks, purchasing/refund/parking/recording rules and the host query prefix made
  their guards fail; original production sources were restored and verified green.
- `cargo clippy -p sim-core -p sim-host -p sim-ffi --lib` and
  `cargo clippy -p verify-server --all-targets`: passed with warnings. The broader
  all-targets invocation also linted old sim-core tests and failed at 30 existing
  print/panic sites; each offending source line was confirmed present in HEAD.
  Those unrelated test-lint failures were not suppressed.
- Localization guard: **1,867 keys**, no missing keys; canonical and Unity CSV
  copies match. `git diff --check` passed.
- Unity C# was compiled with the installed Unity Roslyn compiler using the idle
  project's assembly references, writing output only under `/tmp`: **zero errors**,
  three existing ArenaHud/ArenaView warnings. No Editor operation was used.
- `tools/unity_dylib.sh`: built and signed the **release 0.0.344-alpha** plugin
  into the idle project's `Assets/Plugins/macOS/libsim_ffi.dylib` (19 C exports).
  The C ABI test exercises the starter **Seraphine**: 18 displayed nodes, point
  debit, save/load, refund, ticks and verifier replay.
- The retained Unity graph was checked in the idle Editor with 18 nodes and 18
  edges; its button rectangles have no overlaps.

The idle backend must also run sim version **0.0.344-alpha** before an online play
check; no server was restarted or deployed during this work.

### Two-character Unity check (2026-09-10)

`python3 tools/check_potential_pair.py` loads the staged native plugin and writes
an isolated preview to `/tmp/potential-pair-preview.json`. Its 50-point, Job-50
pair is a test fixture, not a new-game grant. It checks both major routes, exact
point debits, save/load, refunds and isolation between members. A separate
combat fixture starts Seraphine one XP short of Job 2; an actual kill grants the
level and increases Skill Points from 1 to 2.

`PotentialBoardProbe.Run()` (ROI > Probes > Potential Board) requires that
isolated pair in Play Mode, the board open, and no SessionBoot/save writer.
Verified **36 node previews**, both purchase routes, save/load, two-click reset
and member isolation. The negative control removed the Max button callback in
the preview: the probe failed on the missing debit, then passed again after
restoring the callback. The original dev save was not written.

Full core unit check: **525 passed, 1 ignored**. Localization guard: **1,867 keys**.
Fresh creation now uses the fixed Seraphine + Rapis pair (sim 0.0.345-alpha),
with one Skill Point each and a party-name-only Unity form. The shared constructor
spawns both before zone population; replay and server genesis verification accept
the pair. Legacy saves still resume without being reset. The C ABI starter test
checks identity, points, purchases, save/load and replay; the server anchor test
checks the matching seed and rejects a different seed. A duplicate-Seraphine
negative control failed as expected. After a full idle Editor restart, the
0.0.345 native plugin also passed the real Begin button in an isolated creation
preview: both starter IDs at level 1, one Skill Point each, no console errors.
This preview bypassed network login and wrote no real save. Server clippy passed
with existing warnings. Deployed 2026-09-10 at 865ada6d6e6c: server and
restarted Unity both run 0.0.345-alpha; catalog 0.0.101-alpha matches. Linux
unit/golden tests and the Unity-aware deployment sync check passed.

Outstanding: migrate legacy recruit/advancement flows; author incomplete node
effects; add a global earned-point notification. Board balances already refresh
from the sim while open.


## Free fixed kit (0.0.346-alpha)

Seraphine and Rapis start with all four authored abilities at level 1, free:
Skill 1, Skill 2, Ultimate and Passive. Active abilities are slotted on first grant;
Ultimate still needs its existing Limit Break gauge. The unsupported Basic scaling
node remains gated. Skill nodes k1–k3 therefore display rank 1 immediately, and
upgrades spend Skill Points from rank 2 onward. Stat/Major nodes remain paid.

The shared constructor and trusted-save migration use `potential::base_kit`.
Existing active/benched characters gain missing skills without losing progression,
learned ranks or combat clocks. An already learned but manually disabled skill stays
disabled. An old board purchase from rank zero credits its first paid point once;
raising the recorded baseline prevents repeated refunds. Reset retains rank 1.
The server migrates its verified checkpoint through the same path as Unity, so
no account reset is needed. Migration equality and replay are covered at the C ABI;
a 600-tick combat test verifies both Iron Chains and Fireball actually fired.
