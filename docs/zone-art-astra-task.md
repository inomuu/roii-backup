# Task: dress the region-1 grassland zone

You are working in an isolated git worktree on branch `feature/zone-art-astra`. Another agent is
working on `feature/gacha-pivot` in a different worktree of the same repository — **you will never
see their files and they will never see yours, as long as you stay on this branch.**

## 1. Read first

- `CLAUDE.md` — the project's rules. `AGENTS.md` points at it. It applies to you.
- `docs/scene-brief-external-tool.md` — the creative brief: mood, style, camera, framing,
  cinematography, and what the scene is missing. **This is the spec.**

## 2. What to produce

A dressed region-1 grassland zone that matches the brief, delivered as changes to
`godot/proto/arena3d/zone_dressing_r1.tscn` (prop placement) and, if the grade needs to move,
`godot/views/look_settings.gd`.

If the scene needs props we do not own, add the PNGs under `godot/assets/props/` — transparent,
pivot at the BASE of the image, lit to match the scene's key direction.

## 3. Files you may change

- `godot/proto/arena3d/zone_dressing_r1.tscn` — the dressing scene. This is the main deliverable.
- `godot/assets/props/*.png` — new props only. Do not repaint existing ones.
- `godot/assets/bg/bg_r1.png` — only if the ground itself is the problem.
- `godot/views/look_settings.gd` — grade values ONLY (the `static var` block). One grade for the
  whole scene; never correct an object's colour to make it fit.
- `docs/` — write down what you did and why.

## 4. Files you must NOT touch

- `godot/proto/arena3d/arena3d.gd`, `arena3d_scene.gd`, `arena3d_draw.gd`, `arena3d_fx.gd` and
  anything else in the renderer's runtime path. It carries years of invisible tuning (attack
  anticipation synced to attack speed, hit-frame contact, lunge surge, flinch queue, death
  sequencing) and `godot/` has **zero tests**, so breaking it is silent. If you believe the
  renderer must change, say so in your report instead of changing it.
- `crates/**` — the simulation. Nothing about dressing a zone requires it.
- `unity/**` — a separate port, not your problem.
- Anything under `.claude/`.

## 5. How to see your work

The renderer is not the way to look at a map. Use the dev preview, which renders the real
dressing scene under the game's real lighting and camera and writes screenshots:

```
/Applications/Godot.app/Contents/MacOS/Godot --headless --path godot proto/arena3d/dressing_preview.tscn
```

It writes `~/Desktop/ROI/dress_r1_room{1,2,3}.png` and `dress_r1_over.png`. Judge the scene from
those images, not from coordinates and not from the .tscn source.

A NEW `.gd` file with a `class_name` needs an import pass before anything can load it:

```
/Applications/Godot.app/Contents/MacOS/Godot --headless --path godot --import
```

## 6. Before you say you are done

- `tools/smoke.sh` passes. Both the 2D and the HD-2D scenes must still parse and boot headless —
  the 2D renderer is frozen but must not rot.
- `python3 tools/check_loc_keys.py` passes. No new user-facing string literals in `.gd`.
- The preview screenshots exist and show what you claim.
- No prop blocks the lanes characters walk (party enters LEFT, monsters from the RIGHT).
- New textures are downsized before committing.

## 7. Committing

- **Never `git add -A` or `git commit -a`.** A Godot import pass generates `.uid` files beside
  every script; sweeping them in has polluted commits here twice. Add the files you actually
  changed, by name.
- Stay on `feature/zone-art-astra`. Do not merge, rebase, or switch branches.

## 8. Your report

Say what you changed, what the grade moved to (if it moved), which props you added, and — most
important — whether your composition answers the middle-distance gap described in §5 of the
brief. That gap is the reason this task exists.
