# Unity: skill timing — one action timeline (handoff 2026-09-26)

From the sim/gameplay session to the Unity client (Codex). Sim 0.0.481.

## What was wrong

User: a skill fired the moment its cooldown ended and the attack animation was cut mid-swing.
In the sim, heals, party buffs, shields, ultimates, fields, summons and traps were cast on ANY
tick — including while `atk_acc` was climbing and `ArenaView` was drawing the basic wind-up.
During a skill's motion lock `atk_acc` also kept charging, so a basic wind-up was drawn over the
skill and the basic fired the tick the lock ended.

## What the sim does now (0.0.481)

- While a member is engaged (`atk_acc > 0`), EVERY cast waits for the tick the current swing
  would land and takes that swing's place. A wind-up therefore always ends in exactly one thing:
  the basic strike, or a skill. Off-target (walking, no enemy) casts still fire at once.
- The motion lock freezes `atk_acc`. The next wind-up starts after the skill's motion.

## What the client should do

1. **Wind-up into the right animation.** `pending_skill` on a party member now also names the
   out-of-cycle cast the wind-up leads into (a heal, buff, ultimate, …), not only a swing's own
   skill. While `pending_skill` is set, play that skill's cast/wind-up (its `anim_slot`) instead of
   the basic attack wind-up, then its resolve when `skill_fx` / `ult_fx` arrives. The cut-in
   trigger stays on `ult_fx`.
2. **Hold the skill pose through the lock.** New entity key `motion_lock` = ticks left in the
   skill's motion lock (absent when 0). While present, do not start a basic wind-up (the
   `windupDue` path in `ArenaView.cs` ~3679) — keep the skill animation / its follow-through.
   `atk_acc` is frozen then, so the wind-up math resumes correctly when the key disappears.
3. `ArenaSound.cs` already plays the skill sound on the rising edge of `pending_skill`; it now
   also covers these casts — check nothing double-plays with the resolve sound.

Verify on a party with Seraphine (Aegis), Lapis (meteor ult) and a support (Rivus / Sylvie buffs):
no basic wind-up should be visibly interrupted by a skill, and no wind-up should draw over a skill.

## 0.0.483 — `pending_skill` is stable through a wind-up

The kind of a swing is decided when its wind-up BEGINS. `pending_skill` is set on the wind-up's
first tick or not at all — it no longer appears halfway through a basic (which is what swapped
Seraphine's sword row for the open-hand row mid-swing). The remaining exception is a situational
cast whose condition turns true mid-wind-up (a heal for an ally just hurt); if the client latches
the row at wind-up start it can ignore those, and the cast then plays from the strike.
