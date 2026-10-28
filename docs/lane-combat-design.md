# Lane combat (Trickcal-style): decisions so far

Status: a TRIAL. `crates/lane-sim` runs it beside the live combat. Nothing here is wired into
`GameState`, saves or the client, and the live 20×10 combat is unchanged (user 2026-10-04:
"ทำอันใหม่ อันเก่าก็เซฟเก็บไว้ อย่าพึ่งเปลี่ยนอันเดิม").
Visual prototype: the "Lane Formation Lab" HTML artifact (scratchpad `lane-formation.html`).

## Settled (user, 2026-10-04)

| Topic | Decision |
|---|---|
| Field | 20 wide × 3 lanes (a lane per depth row, not the old 10 rows) |
| Formation | 3 columns × 3 lanes; back / mid / front at x = 4 / 6 / 8, gap columns between |
| Party per cell | 1 unit per cell, by default |
| Monsters per cell | Stack up to N per cell, so a front line can be swarmed. `stack_cap` defaults to 2 |
| Reach | Chebyshev distance: attacks cross lanes and diagonals |
| Rear ambush / burrowers | Supported: rear spawns enter from x = 0, burrowers surface beside the rearmost unit. They land in the gap cells; the formation needs no extra cells |
| Stats | The live sim's, built the way `sim-cli --bench` builds them, with the live damage formula, mobbed DEF included (`lane-sim/src/bench.rs`) |
| Skills, AoE | AoEs keep their live shapes for now (radius / 3×3 / 5×5 / field), even though a 3-lane field makes them cover every lane |
| Iron Chains | Pulls from ANY position: its two targets land next to Seraphine wherever she stands. The gap rule and the stacking cap do not stop the pull |
| Abyssal Embrace (Nemu) | Pulls into a FIXED point: the middle lane (lane 1), the first cell on the enemy side of the front column. It is the same cell whether or not a unit stands in the middle-lane front slot, and the field is centred there, not on the densest cluster. The pull ignores the stacking cap |
| Frost Taunt (Lumi) | A taunted monster crosses the gap rule to reach Lumi wherever she stands |

## Stage flow (user, 2026-10-04: "ให้แบบเกมเดิม มี 3 โซนย่อย มีบอสโซนย่อย")

A stage keeps the live structure: 3 rooms, each with its live spawn floor, trickle, cap of 20,
kill quota and room boss with minions; the 20% room-hop heal; the Move-Speed march between rooms.
Lanes change only positions: the live spawner's monsters enter at the right edge of their lane,
and the party stands on its formation. (An earlier trial of 7–10 waves + one boss was dropped: a
third of the live kills, so EXP and drops fell behind.)

Positioning (user, 2026-10-04):
- The clash is MID-FIELD. The party holds its formation, and steps forward only when a monster hits
  it from beyond every member's reach.
- Only a room's FIRST group meets the party half-way: the party walks in from the left edge onto
  its slots while the room's opening monsters walk in from the right edge. After that the party
  holds, and every later spawn up to the quota and the boss walks in to it (user:
  "เดินเข้าหากันแค่ชุดแรกของ wave").
- Monsters that trickle in afterwards ENTER AT THE EDGE AND WALK IN ("เดินเข้ามาดูดีกว่า"). This was
  chosen over spawning next to the clash line after a side-by-side replay.
- Walking speed: **50% of live** (`lane::LANE_SPEED_BP`, user 2026-10-05). In a 7-day read it
  kept the live pacing (stage 109 vs 108 on day 7, kills −2%) and gave fewer wipes (329 vs 407).

Groups (user, 2026-10-05: "spawn เป็นกลุ่มใหญ่ แล้วปะทะทีเดียว -> เดินต่อนิดๆ" and "ให้ปาตี้เดินหมุนฉากตลอด"):
- In lane mode the live one-at-a-time trickle is off.
- Each room comes as `groups_per_room` (3) big groups of ~10, a third of the room's quota, released
  at once from the lane edges. A 2–3 s walk separates groups, then the live room boss follows.
- `lane.marching` is true while nobody in the party has a monster in range. The client scrolls the
  scene and plays the walk, so the party always reads as moving.
- WALK AND CRASH (user, 2026-10-05):
  - Nobody waits. While marching, the party's own walk also closes the distance, so the two sides
    meet mid-field.
  - Nobody attacks or casts until the crash (a monster within one cell of a member), shooters
    included.
  - Then they fight until the group is down, and walk on.
- VISUAL FIRST (user): the numbers are not tuned. In a 7-day read the groups are easier than the
  live trickle: stage 134–145 on day 7 vs 108, with ~25% fewer kills; 4–5 groups per room did not
  close the gap. Balance comes after the look.

Rule audit (user, 2026-10-05). The user asked "มีอะไรที่ทำไปโดยไม่ได้สั่งอีกบ้าง":
- **Removed:**
  - The formation stepping forward when out-ranged (hold-fire makes it moot).
  - Overtaking: no multi-cell pass-through and no side-steps around a full cell. Stacking is
    allowed, so a walker queues.
- **Confirmed:**
  - 4 groups per room (a quarter of the quota each).
  - The gap rule.
  - The room boss waits for the last group.
  - A walker goes STRAIGHT along its lane while a target is reachable that way (its own or the
    adjacent lane). It steps diagonally only when none is.
- **Crash** = a monster in a cell ADJACENT to a member (Chebyshev 1, diagonals included).
- **Release layout:** a group lands ONE MONSTER PER CELL as a 3×3 block on the three rightmost
  columns, front column first. Only a group of more than 9 stacks: monster k goes to cell k % 9.
  With 4 groups per room (user) a group is a quarter of the quota, at most 9, so it lands one per
  cell.
- **Enemy gaps (2026-10-06):**
  - The enemy block lands on x 15 / 17 / 19 (`lane::ENEMY_COLS`), one gap column between ranks,
    mirroring the party.
  - It keeps a free column between monsters in a lane until the crash; after the crash it may close
    up.
  - Gap rule, how far apart and when the gap holds: the assistant proposed these; the user did not
    object.
- **Vertical spacing: VISUAL ONLY for now** (user chose "แบบ V"). The renderer draws the lanes further
  apart so rows overlap less. The sim keeps 3 adjacent lanes (cross-lane melee, AoE and pulls
  unchanged). Real gap rows in the sim (lanes at y 0/2/4) were offered and deferred.
- **Ambush support (the unit itself comes later):**
  - When the march resumes, a member off its slot teleports home; `lane.returned` lists them for the
    client's teleport-back.
  - `lane::ambush_cell` finds the landing cell behind an enemy for a future ambush/teleport skill.
- **Stacking (BUILT 2026-10-06):**
  - Before the crash walkers never share a cell; the one exception is a release of more than 9.
  - After the crash any cell takes any number, so the ones behind walk through a stopped shooter and
    close in together.
  - User decisions on the critique:
    - AoE will be retuned later (likely a cap on targets hit).
    - The blob at the clash line is a KNOWN ISSUE, to be eased by spawning mixed melee/ranged
      groups.
    - Piles beside back-row units after a flank or a pull are accepted.
- **Still the assistant's own defaults, not confirmed by the user:**
  - `stack_cap` 2 (now only for placing boss minions at the lane edge)
  - a 2–3 s walk between groups
  - the party walk-in starting 4 cells left of its slots
  - closing pace = the slowest member's
  - the auto-formation (Sentinel/melee front, Support mid, ranged back) for members the player has
    not placed. The formation screen's sim side is built (`lane::set_slot`, debug `lane_slot`; a
    swap; it persists as `lane.chosen`). Dev-only until lane mode is made real.

Built IN `sim-core` as an opt-in mode (`sim_core::lane`, off unless `GameState::lane` is set), so
every skill, heal, break, Limit Break, Regalia and monster skill is the live code's.
- Headless: `sim-cli --lane [cap]`
- Dev slots in the client: debug `{"lane_mode":{"cap":2}}` / `{"lane_off":{}}`
- Ladder against live snapshots: `lane-sim --ladder DIR [--reach | --walkin | --trace STAGE OUT]`

Measured (F2P seed 42, 7 days, live party):
- Same day-7 stage as the live game (108).
- Faster early: stage 60 vs 37 at half a day.
- About 10% more kills, Lv57 vs Lv55.
- About 18% more wipes.
- Walk-in from the lane edge averages 0.4–2.5 s, shorter than the live 1.8–3.5 s.

## Found by the trial (`lane-sim`)

- A party melee unit that chases drags the whole formation out of its lanes, so the party holds
  its slots.
- A shooter that out-ranges the party shoots a slot-holding party forever. Fix: the formation
  advances one cell as a block while no party unit has a target in range.
- Mode A splits a full front evenly (2 per tank at cap 2). The swarm concentrates beside an empty
  lane: up to 5–6 monsters on one tank, with mobbed DEF on top.
- Melee units in rows 2–3 deal nothing against frontal waves, but guard well against rear spawns
  and burrowers. The user's idea: they may step up and share a front cell, then go home when the
  formation moves on (`--party-stack 2`). It works, but showed two problems:
  - The stacked unit takes no hits, because ties go to the lower id (the front unit).
  - Melee in the back row beats ranged in the back row.
  - Proposed fixes, not decided yet: monsters spread their target choice across a shared cell,
    and stacking is allowed only in the gap cells.

## Open

- Target mode A / B / C. Recommended: C (A for normal monsters; divers ignore the gap rule).
- Party size: 6 or 9.
- Stacking fixes (see above).
- AoE retune, deferred: shapes stay as live for now.
- A stalemate breaker in lane-sim, like `tick::break_stalemate` in the live sim.
