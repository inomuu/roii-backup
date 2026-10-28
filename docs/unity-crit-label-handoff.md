# Unity crit label — handoff 2026-09-24

From the sim/gameplay session to the Unity client (Codex). Sim side ships in 0.0.472.

## What changed in the sim

Every hit can crit now — magic and skills included (before 0.0.472 only physical hits could).
The view marks a crit blow:

- `recent_hits[].kind == "crit"` — takes the place of `"normal"` on that blow's event. A
  Toughness break is still its own `"break"` event, as before.
- A crit **single** projectile now records a `recent_hits` event (`hits: 1`, `kind: "crit"`);
  before, a single shot recorded nothing and its number came from the hp delta alone.
- `field_hits[].crit: bool` — damage-field ticks and shield bursts.

So: the number for a target this tick is a crit when that target has a `"crit"` event from the
attacker in `recent_hits` (or a `field_hits` entry with `crit: true`). `FeedMultiHits` counts a
`"crit"` event like a `"normal"` one — nothing to exclude there.

## The look (user, 2026-09-24)

A small **"Cri"** label at the **top-left corner of the damage number**. Text light yellow with
a gradient out to orange. Label text goes through a loc key, not a literal.
