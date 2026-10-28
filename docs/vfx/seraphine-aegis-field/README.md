# Seraphine: Aegis battlefield prototype

Presentation only. No gameplay balance, entity position, targeting, shield pool or RNG edits.

## Sequence

1. Read a resolved `ult_fx` Aegis event and the recipients' matching `shields` from native view JSON.
2. Hold `SimDriver` through an independent presentation owner; defer that arena view until the sequence ends.
3. First Aegis of a `SimSession`: play the existing 1.8 s portrait cut-in. Keep every character at
   its current battlefield position throughout the portrait and field effect; no regrouping or return slide.
4. Charge at Seraphine's feet, expand one textured cyan water wave, then show curved mesh shields,
   ground rings, filled blue ground glow, rising aura, an orbiting ribbon and the Hovl receive-buff
   particles on recipients. No repeating radial pulse.
5. Fade, restore lighting/UI/transients, release
   this presentation hold, then ingest the deferred authoritative view. Total duration: 3.25 s.
6. Subsequent casts: 1.45 s battlefield effect in place, without portrait or regrouping.

The sim has already resolved the skill when it reports the event. Presentation delays its arena
display; it does not rewind or re-apply the result. Characters are not assigned opposing stage sides.
The effect never writes actor positions, including during cleanup or cancellation.

## Assets And Ownership

- `SeraphineAegisSequence`: manually sampled in-place sequence, world-space meshes, lighting snapshots and restoration.
- `BuffShieldMesh`: one broad-shouldered, pointed shield in front of each recipient, sharing one generated
  mesh per sequence (676 vertices / 1152 triangles). The convex face, back and separate thickness rim
  replace the earlier three-panel enclosure, which read as a barrier rather than a shield. A bright
  border, inset trim and small central crest make the silhouette legible at battle scale. It remains
  upright world-space geometry centered directly in front of the character, with no lateral offset
  or extra yaw, not a camera-facing sprite.
- `BuffEffect` / `BuffEffectProfile`: reusable particle-prefab player, independent of Seraphine,
  cut-ins, targeting and sim pause. Profiles select prefab, color override, scale relative to visible
  actor height, playback speed, warmup and lifetime. `Sample(age)` follows the anchor and supports
  seeking/replaying with fixed visual seeds. The caller owns elapsed presentation time and disposal.
- Optional `GroundGlow`, HDR `GroundColor` and `GroundDiameter` add a soft, filled ground disc
  independently of the vendor prefab. Aegis enables cyan-blue; other profiles can choose their own color.
- Optional `RisingAura`, `AuraTexture` and HDR `AuraColor` enable `BuffAura`: an open, slightly flared
  curtain (585 vertices / 1024 triangles) and one 1.15-turn helical ribbon (485 vertices / 768 triangles).
  A single upward texture reveal and 115-degree partial orbit use normalized buff age, never shader
  wall-clock time. The ribbon rises 0.18 actor heights during the burst. Geometry stays in world-up;
  `ZTest LEqual` against ActorSprite's alpha-clipped depth hides rear portions without a screen overlay.
  Existing generated water texture breaks up the curtain; a soft ribbon body surrounds a sharper core.
  Aegis opts in; other profiles retain their old appearance. All generated meshes/materials are disposed
  with the owning buff. Meshes are currently per-instance, not pooled.
- `Resources/BuffEffects`: AegisReceive (cyan Buff), HealingReceive, ShieldReceive. These reference
  existing imported URP prefabs from [Magic Effects FREE by Hovl Studio](https://assetstore.unity.com/packages/vfx/particles/spells/magic-effects-free-247933).
  The vendor files remain untouched and gitignored. Import this pack and convert its materials to URP
  on a fresh machine; `ROI > VFX > Create Hovl Buff Profiles` creates missing profiles without overwriting edits.
- `SeraphineAegisField.shader`: additive HDR cyan-white outlines and soft textured emission. The shared
  generated `Resources/Cutin/SeraphineAuraFlow-v1.png` provides the water breakup. Radial edge fade hides quad borders.
- `AegisPresentationTrigger`: session reset, duplicate event guard, player/non-pet caster filter,
  recipient selection from shield source, and first-portrait frequency.
- `ArenaView.Aegis`: arena lifecycle integration, frozen actor animation/HUD/transients, clock rebasing,
  pending view, scene/session cancellation. Story-fed views are excluded while the live arena is suspended.
- `SimDriver.HoldPresentation`/`ReleasePresentation`: do not clear another owner's pause or the manual pause.

No scene asset edits or global post-processing/bloom tuning are required. The scene's existing
post-processing remains in use. This prototype has no new SFX, bespoke casting sprite animation,
persistent shield-status indicator, or shield-break effect.
The shared player currently handles a short receive burst, not authoritative long-lived status VFX.
The first implementation resimulates a small particle prefab for seekable previews; it is not a
pooled, benchmarked solution for hundreds of simultaneous effects.

## Preview And Verification

Current preview: `in-place.mp4` and `in-place-portrait.png` keep the original scattered encounter
positions. The unchanged-position guard failed against the previous staging implementation at 0.25 s;
after removing all actor-position writes it passed entry, hold, field and cleanup across all three
viewports, along with native pause/hash/resume and shared-buff cleanup checks.

In Unity play mode, with Seraphine in a live combat arena:

`ROI > Cut-in > Seraphine Aegis > Play battlefield sequence`

For an isolated editor capture:

```sh
/Applications/Unity/Hub/Editor/6000.0.83f1/Unity.app/Contents/MacOS/Unity \
  -batchmode -projectPath /Users/lunalieneray/idle_mmorpg/unity/ROI \
  -executeMethod SeraphineAegisPreview.Capture -quit \
  -logFile /private/tmp/roi-seraphine-aegis.log
```

The capture opens the authored 3D arena additively and never saves it. Five scattered sprites demonstrate
in-place reception; companions include stand-in art, not a claim about the current playable roster.
The fixture is positioned once for each viewport before the sequence begins, then must not move.
Foliage wind is frozen on temporary material copies only, so before/after pixel comparisons are meaningful.

The probe renders 1280x720, 1680x720 and 540x960, checks visible field changes and shader compilation,
isolated aura pixel contribution against black with post-processing disabled, ribbon rotation and backward seeking,
unchanged actor positions at every captured sample, native sim hash stability during a real driver hold, release/resume,
short repeat casts, and cancellation with an existing manual pause. Trigger scenarios use synthetic view
payloads. A separate comparison renders healing, buff and shield through the shared player and checks
emission and visible cleanup. This is not an end-to-end online encounter or a mobile GPU performance test.

Earlier staged aura capture (2026-09-24): `aura-wrap.mp4` is the full sequence; `aura-wrap-field.gif` loops only
the portrait battlefield section. `aura-wrap-portrait.png` shows the upward reveal. With the aura
opacity deliberately zeroed, the isolated pixel test failed with zero changed pixels; restored
opacity produced 15,218 / 15,659 / 23,695 changed pixels for landscape / ultrawide / portrait.
Whole-scene consecutive-frame comparisons were rejected for this test because other effects add
unrelated pixel differences. The isolated check disables post-processing on a temporary free layer
and restores camera state afterward. Full-scene screenshots still check sprite occlusion visually.

Known contract limitation: the current driver publishes once after a batch of ticks; intermediate
`ult_fx` events can be missed. Busy Break/room-transition/cut-in states also decline the presentation;
the first-portrait flag is only consumed after a successful start. No event queue is introduced here.
