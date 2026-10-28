# Cut-in authoring guide

A **cut-in** is the dramatic full-screen character splash that plays when an ultimate / skill
fires (gacha-style). It is **presentation only** — it renders and pauses the sim for the moment,
but never decides anything about combat. Per-**Exemplar** (the paid skin unit): owning an Exemplar
buys its cut-in, so the art + direction are keyed to the Exemplar, not the base class.

HD-2D only. The frozen 2D renderer does not get cut-ins.

## Architecture — three layers

| Layer | File | Scope | Owns |
|---|---|---|---|
| **Manager** | `godot/views/cutin_view.gd` | shared | setting-gate, sim pause, offscreen render + blit, skip, companion mirror, watchdog, path routing |
| **Spine** | `godot/views/cutin/cutin_content.gd` | shared | `play()` → runs the clip, emits `finished`; resolves `skill_name_key` |
| **Content** | `godot/views/cutin/cutin_<id>.tscn` | per-Exemplar | camera direction, effects, art — everything unique to this Exemplar |

The manager loads `res://views/cutin/cutin_<id>.tscn` by id, so a new Exemplar cut-in is picked up
just by existing at that path — no code change in the manager.

### Rendering
The cut-in renders **once** into an offscreen 1600×900 (16:9) `SubViewport`. Main mode blits that
texture to the whole window; companion mode draws the **same** texture into every 16:9 pane (so
each pane shows the full splash, not one image straddling the strip). Author everything as if the
frame is 1600×900.

## Shared vs per-Exemplar

- **Shared (reference, never copied):** `cutin_view.gd`, `cutin_content.gd`, the shaders
  (`cutin_bloom` / `cutin_godrays` / `cutin_vignette` / `speed_lines` `.gdshader`), `star.png`.
- **Per-Exemplar (embedded in the `.tscn`):** the `AnimationPlayer` clip, particle configs, the
  `ShaderMaterial`s (their param values), the twinkle `Curve` / `Gradient`, the art texture.

## Making a new cut-in

1. **Copy the template:** `cp godot/views/cutin/cutin_template.tscn godot/views/cutin/cutin_<id>.tscn`
   then delete the ` uid="uid://…"` from the first `[gd_scene …]` line so Godot assigns a fresh one.
2. **Set the art:** assign the Art node's `texture` to the Exemplar splash. Import it as
   **3000px BC7** (`compress/mode=2`, `high_quality=true`) — ~4× less VRAM, no visible loss at this
   zoom (see the acolyte import).
3. **Skill name (optional):** if the design shows the ultimate name, add a `UI_CUTIN_<ID>_SKILL`
   key to `godot/i18n/strings.csv`, set it as the root's `skill_name_key`, and add a `SkillName`
   Label + framing tracks (acolyte kept the loc key; the on-screen label was cut — copy it back
   from git history if wanted).
4. **Direct the camera** in the `AnimationPlayer` "cutin" clip (the template already has these
   tracks): `Art:offset_left/top/right/bottom` = the 3-beat framing (legs → chest → whole figure),
   `Art:modulate` = fade in/out (its last keyframe drives when `finished` fires),
   `Art:scale` = the slow continued drift. Scrub the timeline and drag the Art node to frame each
   beat by hand.
5. **Opt in to effects** (the template has NONE — see below).
6. **Test:** press `J` (dev trigger, works in any mode incl. companion) or the debug panel's
   "Play cut-in" button.

## Effects are opt-in — an effect reference library

The template ships **empty** — Art + camera + fade only. Effects are a **shared palette you pick
from**, not something a new cut-in inherits. Two shared pieces are reusable *directly* (a new cut-in
`ext_resource`s them, exactly like acolyte does):

**Shared shaders** — `godot/views/cutin/`:

| Shader | What it does | Tunable via material |
|---|---|---|
| `cutin_bloom.gdshader` | soft glow — brightens skin/whites/gold (screen-mip bright-pass, additive) | `strength`, `threshold`, `lod`, `tint` |
| `cutin_godrays.gdshader` | volumetric light shafts (procedural, additive) — adapted from godotshaders.com | `angle`, `spread`, `ray1/2_density`, `color` |
| `cutin_vignette.gdshader` | radial dark edge | `strength`, `inner`, `shade` |
| `speed_lines.gdshader` | radial motion streaks | `strength`, `density`, `speed` — *available; acolyte dropped it* |
| `cutin_bg.gdshader` | themed backdrop, rotating rays | `deep`, `glow`, `ray_count` — *for a future parallax/cutout art; unused so far* |

**Shared texture** — `godot/assets/cutin/star.png` (procedural 4-point sparkle, used by every star
emitter).

**`cutin_acolyte.tscn` is the reference implementation.** Each effect below is configured there —
its node, its `ShaderMaterial` params, and its animation tracks. To add one, copy that node (+ its
`sub_resource` material + its tracks) into your scene; the shader/texture it points at is shared, so
only the *config* is copied. Retune the params to your Exemplar.

| Effect | Node(s) | Uses shader/tex | Sync | What to copy |
|---|---|---|---|---|
| **Vignette** | `Vignette` ColorRect + `Mat_vig` | `cutin_vignette` | static | node + material |
| **Star field (edge)** | `Stars` CPUParticles2D + `Curve_star` + `Grad_star` | `star.png` | self-playing | node + curve/gradient |
| **Bloom** | `Bloom` ColorRect + `Mat_bloom` | `cutin_bloom` | **timeline** — `strength` eases down over the "full" beat | node + material + track |
| **God rays** | `GodRays` ColorRect + `Mat_gr` | `cutin_godrays` | **timeline** — `angle` sweep + `color` flare on the burst | node + material + tracks |
| **Star burst** | `StarBurst` CPUParticles2D | `star.png` + `Curve_star`/`Grad_star` | **timeline** — `emitting` fires at the reveal beat | node + emitting track |
| **Star shimmer** | `StarShimmer` CPUParticles2D | `star.png` + `Curve_star` | timeline — emits from reveal, lingers | node + emit/modulate tracks |
| **Speed lines** | *(not in acolyte)* — add a ColorRect + `speed_lines.gdshader` material | `speed_lines` | self-playing | build from the shader |

Node **z-order** in acolyte (bottom → top): `Art`, `GodRays`, `Vignette`, `Stars`, `StarBurst`,
`StarShimmer`, `Bloom` — Bloom is last so it reads the composited frame.

> When a timeline effect's material (bloom/godrays) is **animated**, set `resource_local_to_scene =
> true` on that `ShaderMaterial` so the animated params never bleed across concurrent cut-ins.

Turning this palette into drag-in effect **scenes** (`fx/*.tscn`) is deferred until the 2nd/3rd
cut-in exists — the shared interface for the timeline-synced ones is only clear once there are real
examples to compare. Until then the shaders/texture are the shared library and acolyte is the
worked example to copy a configured effect from.

## Trigger

- **Dev:** `J` fires a cut-in from any mode (the debug panel is hidden in companion mode).
- **Debug panel:** "Play cut-in" button (main mode).
- **Real (future):** a skill-cast hook will call `CutinView.play_cutin(exemplar_id)`. Gate it to a
  **rare/manual** trigger (Commander active skill, boss, milestone) — never every auto-cast, or an
  idle game plays it every few seconds (spammy + the sim pause stutters + it stops feeling special).

## Companion mode

Automatic — nothing to author. The manager renders once and companion draws the shared texture
into every pane. Panes are 16:9 (720×405), an exact fit for the 1600×900 render.

## Setting

`CUTIN_MODE` (Options → Skill cut-in): 0 = always, 1 = off, 2 = first-time only this session.
The per-session "already shown" flag is separate and not persisted.

## Sim pause / anti-cheat

`play_cutin` calls `sim_driver.pause_sim_for_cutin(true/false)`. It freezes **only** the tick loop
— state still emits, saves still run, and there is **no catch-up on resume**. The server reads a
paused client as honest (it ticks slower than wall-clock, never faster), so no server change is
needed. Never let a cut-in wedge the sim: the manager's `MAX_CUTIN_SEC` watchdog always unfreezes.

## Layers

CutinView is CanvasLayer **129** — above HUD/debug (~50) and the companion pane layer (128), below
the fatal session-ended modal (130), which must show even mid-cut-in.
