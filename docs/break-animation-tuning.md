# BREAK presentation tuning — 2026-09-09

Revised on master after comparing Octopath reference frames 007, 014, 025 and 033.
The golden arc/burst belongs to the incoming attack, not BREAK. No sim, damage,
stun/recovery, or existing boss camera/pause timing was changed.

## Current presentation

- Dedicated Crimson Text Semibold Italic title: dark navy face, icy outline, larger
  initial B. After the entry settles at 0.25 s, the word drifts right in one
  direction (18 canvas pixels at boss size, scaled with ordinary BREAK). SmoothStep
  eases this drift through the 1.5 s lifetime without reversing. This replaces the
  previous ±10 pixel horizontal sway after user review. The exit now follows
  the observed white/soft silhouette in reference frames 023–027: face turns white
  at 0.90–1.08 s; aligned crisp/soft glyphs crossfade at 1.00–1.22 s; final opacity
  falls at 1.18–1.50 s (0.32 s rather than the previous 0.60 s fade).
  The soft layer uses SDF edge softness 0.55 and opacity rising from 0.32 to 0.85.
  Both layers keep identical baseline/scale and fixed letter spacing. No letter
  spread, rotation or exit vertical rise. These timings are art choices; reference
  FPS and exact durations are unknown. The gold attack effect is excluded.
- BREAK materials are fixed, shared per font; each glyph carries its own tint and
  opacity, so overlapping announcements can whiten/soften at independent ages.
  Above-word damage remains semantic-colored and follows the final fade. Ordinary
  damage numerals retain their existing presentation. Crimson Text SDF and OFL
  licence are bundled under Resources/Fonts with an editor baker for reproduction.
- 12 irregular glass mesh fragments, reduced from 34: only two larger pieces at
  size 0.18–0.28, with the remaining ten at 0.05–0.13. Three silhouettes retain
  translucent blue/lavender facets and bright edges. Lifetimes 1.05–1.5 s,
  gain 2.2, gravity 0.3, speed 1.
- New pooled ArenaBreakFlash uses a procedural additive shader, a white core and
  a larger blue/lavender halo. Core stays bright through about 0.7 s, fades by
  1.15 s; halo ends at 1.3 s. It follows the sprite during visual interpolation,
  overlays its interior, and renders behind glass and title. The shader lives in
  Resources/Shaders so it is included in builds. No generated bitmap required.
- Boss key is a real cool spotlight with hard shadows, intensity 45, range 9,
  outer/inner angles 82/48 degrees. It is placed on the camera side just above
  the silhouette: centre + front*max(2.4, height*1.4) + right*0.7 +
  up*max(0.5, height*0.5). Lower placement lengthens the rear cast shadow.
- At full focus, sun, fill, CanopyBounce and Reference_SunBreak lights fall to
  18% of their starting energy; flat ambient falls to 8%. The separate actor key
  contribution makes the camera-side light reach sprites without changing ordinary
  actor exposure. All borrowed lights and shader globals are restored at the end
  or when focus is interrupted/disabled.
- PC pipeline additional-light shadows enabled. Active painted terrain uses stock
  URP Terrain/Lit and receives the actual sprite silhouette shadow. ActorSprite's
  original ShadowCaster vertex/bias implementation was retained.
- Ordinary BREAK uses smaller title/flash and glass, with no boss camera/dim/key.
  Camera envelope remains 0.34 s punch, 0.55 s hold, 0.75 s release; existing sim
  pause remains 0.95 s. Title, flash and shards now use unscaled presentation time
  alongside focus, so slow editor frames do not leave the title stuck in its hold.

## Verification and review

The portable review set is v4 (white/soft exit and fade, with timing CSVs), plus
art-review-break/2026-09-09/damage-font/damage-typography.png. The original Octopath
captures and earlier v1–v3 iterations described below remain local archives in the
master checkout; they are intentionally excluded from this gameplay-asset commit.

The white/soft exit compiled successfully and was reviewed in live boss-style
DebugBreak captures on ordinary living enemies: v4/white-soft.png at actual age
1.1554 s and v4/fade.png at 1.3461 s. The former shows a filled white soft word;
the latter shows the soft silhouette fading without a dark face or sharp outline.
The key light and global actor key restored after both captures. Adjacent CSVs
record actual ages. One earlier attempt had no living target and emitted a review
helper error; another was obscured by the death overlay. Neither was used as final
visual evidence. No scene or gameplay changes; Unity remains in Play Mode.

Before the white/soft exit revision, the one-way drift revision compiled without errors. A temporary HUD exercised the
actual DrawBanners path with a fixed world anchor at ages 0.25, 0.6, 0.9, 1.2 and
1.45 s. Glyph X increased monotonically in both ordinary and boss sizes (9.85 and
17.92 px total sampled movement); opacity held at 1 through 0.9 s and fell to
0.0197 at 1.45 s. The temporary HUD was removed; no simulation state was changed.
Existing captures below predate this one-way motion change.

The preceding v3 revision was compiled and reviewed live: the running shard count is
12, and the title remains intact during its fade. v3/hold.png was captured at
0.570 s; v3/fade.png at 1.290 s, with measured ages in the adjacent CSV files.
The final single-frame review completed with no console errors, key reset to zero,
and no remaining flash particles. Unity remains in Play Mode as it was when this
revision was requested. No scene settings changed in this revision.

The checks below also retain context from the preceding v2 pass. Its captures
are for comparison and do not show the current title motion or fragment count.

Explicit AssetDatabase.Refresh was required after external file edits; current
compiled types and shader support were checked in the live master Unity instance.
Unity compilation and final Play Mode console have no errors. Both boss and ordinary
DebugBreak paths were exercised on living ordinary monsters, not a real boss event.
The debug path uses the actual actor ID/anchor and a display-only damage value 999;
it does not apply damage or alter toughness in the simulation.

After the beat: key disabled, environment light energies restored, actor key global
zero, flash particle count zero. Normal capture confirms focus age stays -1.
Source whitespace checks passed. Unity was returned to Edit Mode; the authored
Zone_r1_Painterly_3Rooms scene contains only the saved focus settings, no runtime
capture actors or particles.

Screenshots: art-review-break/2026-09-09/v2 show the previous 34-fragment,
letter-scatter version. The parent folder preserves the first pixel-font pass.
v2/break-review-0.png is that version's hold at an actual age of 0.642 s. CSV files
record actual ages: GPU readback can stall the Editor and skip requested marks,
so filename numbers are not frame-accurate timestamps. A previous warm capture
showed the old whitening/separation near 1.1 s; the retained boss run skipped that
exit frame because of readback stalls. That exit was superseded by the whole-word
one-way drift and fade described above. Review current motion live before judging final
pacing. Mobile shadow parity, standalone build and GPU cost have not been tested
in this pass.

## Reuse and controls

ArenaView.PlayBreak is shared by combat and the editor preview. ArenaBreakFlash
owns pooled emitter pairs/materials; ArenaShards owns glass meshes/materials.
Both clean up their resources. B triggers the full beat, N the ordinary version,
and V is the existing camera easing selector. In Play Mode, execute
BreakReview.Run() or BreakReview.RunNormal(); they capture completed frames to
/tmp, compress PNGs after the sequence, and write timing CSVs. The review helper
restores the driver's prior pause flag and refuses overlapping capture runs.
RunStill(age) captures one chosen age, avoiding earlier readbacks skipping the
exit, and checks light restoration after LateUpdate. This was used for v3 and v4.

Local reference archive: docs/references/octopath-break-2026-09-09.
Only BREAK/damage presentation and its dependencies are included in the commit.
