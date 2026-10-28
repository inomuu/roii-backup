# Unity port parity — how the port is CHECKED

The Unity arena was ported from the Godot HD-2D renderer by hand. A hand port drops things
silently: the first `arena_tables.json` carried 1 class of 10 and 0 of the 36 bespoke monster art
entries, and nothing anywhere reported it. Screenshots do not catch this either — a side-by-side
can say "looks close", it cannot say "the boss is 1.6× where it should be 2.5×".

So parity is enforced two ways, and neither is "read the diff carefully".

## 1. Tables are GENERATED, never typed

```bash
godot --headless --path godot --script tools/export_arena_tables.gd
```

Reads `views/render_data.gd`, `views/player_animator.gd`, `views/monster_catalog.gd` at runtime and
writes `unity/ROI/Assets/StreamingAssets/art/arena_tables.json` + copies every sheet the tables
reference into the same folder. `cols`/`rows` come from the PNG's real size, not from a count typed
by hand.

Re-run it after ANY render-table edit on the Godot side; `git diff` on the JSON is the drift
report. A row that exists in Godot and not in the JSON is now impossible.

## 2. The renderers are compared as NUMBERS

Each engine can dump the per-actor render state at chosen sim ticks, from the same seed + zone +
class. Everything is in CELL space: the two engines deliberately disagree on axis sign and camera
yaw, so world coordinates would report that as a difference on every row and bury the real ones.

```bash
T=40,60,80,100,120,140,160,180,200,220,240,260,280,300,320,340,360,380,400,420,440

# Godot (windowed — the capture path needs a real display)
ARENA_DUMP_TICKS=$T ARENA_DUMP_OUT=/tmp/godot_arena.csv \
  godot --path godot proto/arena3d/main3d.tscn

# Unity (batchmode; ONE Editor at a time on this machine)
ARENA_DUMP_TICKS=$T ARENA_DUMP_OUT=/tmp/unity_arena.csv \
  /Applications/Unity/Hub/Editor/6000.0.83f1/Unity.app/Contents/MacOS/Unity \
  -batchmode -projectPath unity/ROI -executeMethod ArenaDump.Run -logFile /tmp/unity_dump.log

python3 tools/compare_arena.py /tmp/godot_arena.csv /tmp/unity_arena.csv
```

`tools/compare_arena.py` normalises what is a naming difference (Godot's `row_N` clips, `damaged`
vs `hurt`) and skips the frame index of FREE-RUNNING clips (idle/move phase is off each engine's
own clock and carries no information). Position tolerance is 0.12 cells — one glide frame.

Implementation: `godot/proto/arena3d/arena_dumper.gd` (spawned by `sim_driver_core.gd` when
`ARENA_DUMP_TICKS` is set, same env gate as the screenshot pass) and the `DumpIfDue` block in
`unity/ROI/Assets/Scripts/Arena/ArenaView.cs`, driven by `Assets/Editor/ArenaDump.cs`.

**Measure the measurement first.** The first run reported the player facing inverted at every
tick; that was the dumper reading `flip_h` as facing, which is backwards for a sheet whose art
already faces left. A defect list from an instrument you have not checked is a list of your own
bugs.

## 3. Every symbol is ACCOUNTED FOR

```bash
tools/port_ledger.py            # verify — exit 1 on a Godot symbol missing from the ledger
tools/port_ledger.py --update   # add new symbols as TODO, keep the statuses already set
```

`docs/unity-port-ledger.csv` holds one row per const / func / top-level var / signal in the HD-2D
render path — the 6 files the Unity arena is a port OF (`views/enemy_animator.gd` and
`views/base_monster.gd` are the FROZEN 2D renderer's equivalents and are deliberately out of
scope). Every row carries a status, and the check fails on a symbol with no row AND on a row still
untriaged, so new work cannot arrive without a decision attached.

| status | meaning | count |
|---|---|---|
| `ported` | has a Unity counterpart; the note says where | 126 |
| `deferred` | deliberately not ported; the note says WHY | 129 |
| `missing` | a real gap — it should exist and does not | 101 |
| `n/a` | Godot engine plumbing with nothing to port | 51 |

`deferred` and `missing` are kept apart on purpose: deferred is a decision someone can defend,
missing is work nobody has done. Collapsing them loses the only distinction that matters when
asking whether the port is finished.

The 101 missing rows are 6 clusters, not 101 problems — environment art (38), lighting and
atmosphere (20), the room scroll (20), contact shadows (10), hit juice: squash + overbright flash
(9), and 4 one-offs (spawn fade, per-def death-flinch switch, element sheet fallback, no
dead-target check on the swing).

`--update` seeds `ported?` when the name survives into the C# sources after squashing case and
underscores. That is a hint to speed up triage, never a verdict, and it counts as untriaged until
someone confirms it.

Deliberately NOT wired into `tools/smoke.sh`: the Unity port is not a committed direction yet, and
a Godot-side const should not fail the Godot gate on Unity bookkeeping. Run it in the port loop.

## Defect log

First sweep (seed 42, ticks 40…440): **29 mismatches**. After the fixes below: **11**, of which 5
are downstream of one deferred feature and 4 are believed to be sampling artefacts.

### Fixed

| # | What the diff said | Cause | Fix |
|---|---|---|---|
| 1 | `px_size godot=0.078 unity=0.04992` on a boss | Unity hardcoded `IsBoss ? 1.6f`; the generic slime's `boss_scale` is 2.5 | `boss_scale` exported per-def and read from the table |
| 2 | `death frame godot=3 unity=5`, `clip hurt vs death` | the killing-blow HURT flinch before the death clip (`_death_after_hurt`) was not ported | play the hurt clip, then death |
| 3 | `attack frame godot=1 unity=0` (recurring) | the table was built from `ArenaRenderData.MONSTER_ANIM` (2D) whose contact frame is 3; the 3D renderer reads `MonsterCatalog.SLIME_DEF`, `hit_frame` 4 | `monster` is now exported from `SLIME_DEF` |
| 4 | `cell_x` off 0.29–0.40 at contact | the lunge eased out from frame 0; Godot HOLDS through the wind-up then snaps in over `LUNGE_SURGE` (cubic out) to the contact and eases back (quad in). Magnitude is authored in sprite-local px, not cells | ported the tween shape; `LUNGE_PX`/`LUNGE_SURGE` exported |
| 5 | player `clip idle vs attack` (ticks 100, 320) | no `swing_over` exception: the finished attack clip was held an extra `ANIM_SWITCH_DELAY` after the shot landed | ported the exception; the wind-up's VISIBILITY now runs off the real accumulator (`swing_start_at` uses `ttw_real`) with the `attack_until` window, while the frame number keeps using the smoothed one |

### Open

| # | Rows | Status |
|---|---|---|
| 6 | tick 340/360/380/420/440 (player `cell_x` 19.54 vs 14.0, clip, facing) | **deferred** — the room SCROLL (continuous walk-through rooms, `_scroll_x`) does not exist in the Unity arena at all. Everything in that window is downstream of it, so these rows stay red until the feature is ported; do not chase them individually. |
| 7 | tick 120/280/440 monster `idle` vs `move`, tick 260 facing | **suspected sampling**, not a defect: `step_duration` is identical in both engines, so these are glide-window boundaries sampled at each engine's own frame time. Confirm by dumping at a tick where a glide starts, before spending a fix on it. |
| 8 | tick 200 `cell_x` delta 0.154 | residual lunge timing, down from 0.398. Just over the 0.12 tolerance — recheck after 6 is settled. |
