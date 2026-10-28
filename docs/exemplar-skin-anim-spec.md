# Exemplar Skin — animation spec (artist reference)

**Status:** render framework BUILT 2026-07-12 (`godot/views/player_animator.gd` SKINS + `arena.gd`).
Placeholder skin works; real skins slot into `SKINS`. Companion: [[character-skins-idea]].

## What is standard vs per-skin

A skin swaps a party member's class sprite for an Exemplar sheet. Sheets from different artists WON'T
share a grid, so **the layout is NOT fixed** — each skin declares its OWN rows/frames/geometry. What IS
standard is the set of animation **NAMES** + their **semantics**. A skin MUST provide all of them.

## Required animations (the contract)

| name | what it is | notes |
|---|---|---|
| `idle` | standing loop | loops |
| `walk` | moving loop | loops; fps auto-scales to move speed |
| `attack` | basic melee/ranged swing | **MUST carry `impact`** (see below) |
| `death` | death → hold last frame | `hold_last: true` |

## The one hard rule — `attack.impact`

Damage is 100% sim-decided; the animation is SYNCED to it. Each skin's `attack` clip must say WHICH
frame is the CONTACT (when the hit lands), plus optional lunge frames:
- `impact` — the frame the DAMAGE visually lands on (synced to the sim's damage tick).
- `lunge_from` — first strike frame (lunge starts here; default = impact).
- `strike_end` — last strike frame; `impact+1..strike_end` = follow-through after the hit.

Get `impact` wrong and the hit number trails or leads the pose. Everything else (row, frame count, fps)
is free per skin.

## Per-skin entry shape (`SKINS` in player_animator.gd)

```gdscript
"skin_id": {
    "sheet": "res://assets/<sheet>.png",
    "anim": {
        "idle":   {"row": R, "frames": N, "fps": F, "hold_last": false},
        "walk":   {"row": R, "frames": N, "fps": F, "hold_last": false},
        "attack": {"row": R, "frames": N, "fps": F, "hold_last": false,
                   "impact": I, "lunge_from": L, "strike_end": E},
        "death":  {"row": R, "frames": N, "fps": F, "hold_last": true},
    },
    "geom": {"fw": PX, "fh": PX, "foot": 0.0-1.0, "lift": 0.0, "faces_left": bool},
},
```

`geom`: `fw`/`fh` = source frame px; `foot` = fraction of frame height where the feet sit (anchors onto
the tile); `lift` = extra upward nudge in cells; `faces_left` = true if the art faces LEFT by default.

## Wiring (already built)

- `player_animator.skin_of(e)` = the entity's `skin_id` (or the `TEST_SKIN_ALL` override).
- If skinned, `draw_player` uses that skin's `anim`/`geom` instead of the class layout; `arena` passes
  the skin's `sheet` texture. Class SKILL_ATTACK clips (e.g. Bash) don't apply to skins yet — a skin
  plays its standard `attack` for every swing (add per-skin skill clips later if wanted).
- Which member wears which skin = the sim's per-character `skin_id` (server-authored entitlement, later,
  per [[character-skins-idea]]). Render side only for now.

## To test the placeholder
Set `TEST_SKIN_ALL := "placeholder"` in player_animator.gd → every party member renders as the
placeholder skin (proves the swap + standard-named animation sync). Set back to `""` to disable.
