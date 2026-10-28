# Octopath Divine Skills: Battlefield Follow-through

## Source and Scope

Local user-supplied reference:
`/Users/lunalieneray/Downloads/Divine Skills - Octopath Traveler II - OhDittoh (720p, h264).mp4`

1280x720, 60 fps, 132.254 seconds. Visual analysis of extracted frames only;
audio was not evaluated. Overview sampled every 2 seconds; five representative
sequences sampled every 0.25 seconds. Timing descriptions are approximate at that
sampling resolution, not frame-exact animation measurements.

The inspected main sequences show battlefield casting, not a full-screen character
illustration cut-in like our prototype. The inset gameplay windows appear to be
the video's edited comparisons; do not reproduce them as a game transition.
Our cut-in-to-field transition below is a proposed adaptation, not an observed
Octopath cut-in implementation. The actual engine techniques cannot be established
from rendered footage alone.

## Reference Findings

| Sequence | Source time | Visible direction | Useful for ROI |
| --- | --- | --- | --- |
| Bifelgan's Bounty | 00:02.75-00:06.75 | Gold symbol forms over the caster; large overhead arc; directed impact and numbers; coins and light linger | Separate charge, release, target contact and residue |
| Sealticge's Seduction | 00:17.75-00:20.00 | Pink musical bands spread around the caster; another localized pool marks the recipient; brightness recedes | Buff transfer should identify its recipient, not just decorate the caster |
| Brand's Blade | 00:30.25-00:34.25 | Warm charge, brief calmer anticipation, one large slash and hit number, red-lit terrain, then smoke/fade | One legible peak instead of constant maximum brightness; offensive reference only |
| Aelfric's Blessing | 00:55.75-01:00.50 | Bright gold caster bloom, then a separate recipient circle/vertical volume around 00:58.25 onward | Closest staging reference for a defensive/buff ultimate |
| Dohter's Charity | 01:11.00-01:15.75 | Cyan caster flare contracts to a bright point; light rises; a recipient column and ground ring appear around 01:13.50; sparse residue | Closest color and shape reference for Seraphine's field VFX |

![Dohter: caster emphasis, raised light, recipient column, residue](frames/dohter-key-01.jpg)

![Aelfric: caster pose, bloom, recipient effect, restored field](frames/aelfric-key-01.jpg)

The common visual structure is **caster -> release -> recipient -> recovery**.
Large effects stay registered to a body or the ground. Nearby terrain visibly
brightens with the effect while peripheral areas are darker. This gives luminous
volume to small pixel sprites. It is not merely an additive sprite over an
unchanged battlefield. Focus/framing changes and background softness keep the
active subject readable. Do not copy the several-second turn-based pacing intact
into an idle game.

## User Direction: Move Into Focus, Then Return

ROI does not divide actors into fixed left/right teams. Stage the involved actors
temporarily for the skill, then slide them back to their original positions.
This is **presentation movement only**, never movement input to the simulation.

Recommended sequence for the existing 1.8-second Seraphine cut-in:

| Proposed elapsed time | Beat |
| --- | --- |
| 0.00-1.65 s | Existing cut-in; hold sim and suspend ordinary combat presentation |
| 1.65-1.95 s | As the art exits, reveal the real battlefield and slide caster/recipients into a compact focus area |
| 1.95-2.15 s | Short grounded anticipation at Seraphine: cyan floor light and shield sigil; no second long charge |
| 2.15-2.60 s | One outward water-light wave; all confirmed recipients get a brief vertical wash and a shield outline at the same presentation cue |
| 2.60-2.90 s | Slide participants back; restore camera, facing, lighting and ordinary pose ownership |
| After 2.90 s | Release the sequence's sim hold; residual particles can fade while combat resumes |

These are tuning starting points, not timings measured from Octopath. The source
illustration's painted water ring does not become a world-space effect: reuse the
generated mask and color language, not the full portrait overlay.

### Layout

- Select the caster and actual recipients; deduplicate a self-targeted caster.
- Preserve relative screen order and approximate formation, not a fixed heroes-left/enemies-right arrangement.
- Use projected sprite bounds to fit participants into a safe central region.
  Move only as much as necessary and keep feet on the staging ground plane.
- For Aegis, focus the party around Seraphine while leaving unrelated enemies in
  place and visually subdued. Do not collect enemies into the shield group.
- Add spacing where projected sprites overlap. Account for large actors and
  narrow viewports; reduce displacement or use in-place VFX if staging cannot fit.
- Move actor, shadow, attached particles and effect anchors together. Hide or
  move health bars coherently; do not leave them at the old coordinates.
- Freeze unrelated combat attacks/projectiles/number lifetimes during the
  cinematic beat, so they cannot finish invisibly behind the cut-in.

### Return

Capture the rendered pose and position when presentation ownership begins,
including any part-completed walk or attack. Return to that pose before ordinary
animation continues, and preserve its remaining animation time. Restoring just
the transform while old `Time.time` deadlines elapse causes a visible snap.

For normal completion, return offsets exactly to zero before releasing the hold.
For skip/disable, restore immediately and release only this sequence's ownership.
For scene/session change or a destroyed actor, discard stale handles; never
recreate old units or snap a new room's actors to the previous room's positions.

## Implementation Plan, Not Implemented

One presentation coordinator should own:
`capture -> cut-in -> focus-in -> field effect -> focus-out -> restore -> release`.
It holds the existing `SimDriver.HoldPresentation` owner across the entire
sequence, rather than allowing the cut-in's destruction to resume combat early.

Current code implications:

- `Cutin/UltCutinTrigger.cs` reads `ult_fx` and remembers which ultimate has shown
  its portrait this session. Keep that frequency policy **separate** from field
  VFX: every resolved ultimate still gets battlefield feedback, even when the
  portrait is skipped. Use a short in-place field version for later casts unless
  a different focus policy is explicitly chosen.
- `Arena/ArenaView.cs::OnView` starts the cut-in, then continues feeding sound,
  projectiles, HUD and actor updates in the same call. Sim pause does not stop
  those already-dispatched visuals. Buffer the relevant presentation event and
  dispatch its field cue when the focus sequence is ready.
- `ArenaView.Place` assigns `Root.transform.localPosition` every render frame
  from `CurrentGlidePos` plus the attack lunge. An independent transform tween
  will be overwritten. Add a cinematic pose/offset override at this placement
  boundary, or an equivalent presentation parent with coherent anchor handling;
  do not edit `CellX`, `CellY`, `GlideTo` or sim movement to stage a shot.
- `ArenaView.LateUpdate` drives animation with `Time.time`/`Time.deltaTime` and
  already owns camera/lighting for Break focus. Add explicit presentation clock
  and focus ownership; do not start a competing camera/light writer. Retain the
  exact captured animation phase and resume without advancing through the hold.
- Cut-in completion needs to be distinguishable from skip/cancel/disable. Chain
  field VFX on normal completion; cancellation restores safely rather than
  unexpectedly starting an attack while leaving the arena.

The sim has already applied Aegis when `ult_fx` arrives:
`try_cast_aegis` places shields before `after_ult` emits the event. Do not apply
the skill a second time from the VFX timeline, and do not change damage timing.
`ult_fx` carries caster and skill, not an explicit recipient list. The view begins
from serialized state and includes nonempty entity `shields`; Aegis recipients
can be read from matching `source_skill_id`, `src_id` and a positive `pool` in the
event snapshot. Verify that view contract with a targeted test when implementing
the DTO. Do not infer shield amounts from C# or from an expanding visual radius.

For other ultimates, use authoritative target/hit events; extend the host view
when a required recipient identity is absent. Do not associate every hit in a
tick with the cinematic skill. A resolved-event bridge cannot generally recover
a pre-hit pose of an already-dead target without a presentation snapshot/buffer.

### Field Asset Layers

| Layer | Proposed Unity representation |
| --- | --- |
| Ground ring and light pool | Ground-plane mesh/decal with soft additive falloff, centered on the staged caster/recipient |
| One expanding water sheet | Low-poly ring mesh using `SeraphineAuraFlow-v1.png`, one-shot radial expansion and dissolve |
| Vertical recipient wash | Short cylinder/ribbon mesh with gradient and the same intensity texture; render back/front portions with correct depth |
| Shield outline | Camera-facing shield silhouette or curved shield mesh, sharp cyan-white core plus soft halo |
| Residue | Sparse particle motes, not another full-screen loop |
| Local illumination | Short-lived effect light/material response integrated with the arena focus lighting owner |

Big opaque-looking shield barriers should not persist after the cinematic unless
driven by live shield state. A faint live shield marker may follow the actor home;
remove it when the sim says the shield is gone. On-cast energy release is not the
damage burst that Aegis separately produces when a shield breaks.

## Verification Before Shipping

- Real Unity capture through the whole chain, not the existing still-backdrop preview.
- Confirm focus moves and returns every participant, shadow and attachment without
  crossing/overlap issues at 16:9, ultrawide and portrait aspect ratios.
- Native sim ticks/cells remain unchanged during the hold; VFX never sends gameplay input.
- No hidden hit numbers, duplicate shield application, duplicate event playback or
  stale one-frame target anchors at the cut-in/field transition.
- Existing manual pause and overlapping presentation holders survive cleanup.
- Skip, missing assets, missing caster, multiple ultimates, a target death, scene
  switch and session replacement cannot strand a hold or an actor offset.
- Later Aegis casts still display field VFX without replaying the once-per-session portrait.

Only reference frames and analysis were added in this pass. Runtime implementation
is unchanged. `contact_sheet.py` reproduces timestamped boards from extracted frames.
