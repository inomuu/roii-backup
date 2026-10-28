# VFX Handoff

Start here for character VFX work. The Unity client is presentation-only;
do not copy editor fixture motion or hit scheduling into gameplay authority.

## Static Carrier Rule

The mandatory VFX mesh rule lives in `CLAUDE.md` under Client rules (2026-09-25).
Keep carrier meshes fixed at their world-space spawn placement. Build motion from
UV/shader flow and independent head/tail timing, not moving or rotating the mesh.
Existing rotating-orbit and moving-aura studies are known noncompliant legacy
work awaiting explicit migration, not approved patterns for new VFX. Current Bell Strike work
must demonstrate both fixed transforms and changing rendered coverage.

## Folder Map

```text
unity/ROI/Assets/
  Scripts/Arena/Vfx/
    Common/                  Shared buff geometry, hit flash, sword impact
    Seraphine/               Basic slash and circular mesh/UV/flipbook studies
      IronChains/            Linked chain renderer and caster aura
    Suzu/                    Fire sweep presentation (Sunder)
  Scripts/Cutin/             Cut-in and Aegis sequence/trigger
  Scripts/Arena/ArenaView.Aegis.cs  Arena integration stays with its partial class
  Scripts/Arena/ArenaView.Seraphine.cs  Live Steel Break / Iron Chains presentation
  Editor/Vfx/
    HovlBuffProfiles.cs       Shared buff profile authoring
    Seraphine/               Art capture and comparison tools
      Chains/                Capture entry points, temporary fixture, checks
    Suzu/                    Sunder/Bell Strike capture, temporary fixture, checks
  Resources/Effects/         Runtime-loaded effect shaders/materials/textures
  Resources/BuffEffects/     Reusable buff profiles and prefabs
  Resources/Cutin/           Cut-in shaders, portrait assets, aura flow texture
  Shaders/SwordEdge.shader   Basic sword-edge shader
docs/vfx/
  seraphine-*/               Per-effect handoffs, retained media and studies
  suzu-sunder/               Fire sweep preview and handoff
  suzu-bell-strike/          Downward slash and Hovl explosion preview
  *-study/                   Video reference analysis and technique comparisons
```

Existing scripts moved with their original `.meta` files; namespaces and public
capture method names are unchanged. `Resources.Load` paths remain unchanged.
Do not move resource assets without updating callers and preserving GUIDs.

## Current Work

| Effect | Handoff | Integration state |
| --- | --- | --- |
| Basic sword edge | [Basic Slash](seraphine-basic-slash/README.md) | Called by ArenaView's Seraphine basic animation |
| Ultimate cut-in | [Cut-in](seraphine-ultimate-cutin/README.md) | Aegis presentation path; see Scripts/Cutin |
| Aegis field/buff | [Aegis](seraphine-aegis-field/README.md) | ArenaView.Aegis integration; shared buff pieces |
| Circular slash | [Circular Slash](seraphine-circular-slash/README.md) | Live Steel Break; fixed world carrier and approved UV sweep |
| Hit impact / stars | [Impact](seraphine-impact/README.md), [Stars](seraphine-radial-stars/README.md) | Live skill impacts; soft red flash scoped to Steel Break, global flash unchanged |
| Iron Chains | [Chains](seraphine-iron-chains/README.md) | Live resolved casts, sim hit IDs, reach/pull/settle poses and caster aura |
| Suzu Sunder | [Sunder](suzu-sunder/README.md) | Two-target editor fixture with placeholder swordsman art; no sim-event integration |
| Suzu Bell Strike | [Bell Strike](suzu-bell-strike/README.md) | Single-target downward slash and existing Hovl explosion; editor-only |

Latest chain preview: [two targets](seraphine-iron-chains/iron-chains-two-targets.mp4),
[slow playback](seraphine-iron-chains/iron-chains-two-targets-slow.mp4).
It uses the approved reach/retract poses, thick black outlined links, fast yank,
and one blue-white caster aura. Pixelation was discussed but is not implemented.

## Iron Chains Ownership

- `IronChainsEffect.cs`: chain geometry, tip extension, opacity, outline, lifetime.
- `IronChainsAura.cs`: owns the caster curtain/ribbon/ground resources and their pulse/fade.
- `SeraphineChainsPreview.cs`: stable menu/batch entry points and viewport capture I/O.
- `SeraphineChainsFixture.cs`: temporary authored scene, source sprites, target list,
  synthetic pull, pose selection, cleanup. Both targets use the same sampling code.
- `SeraphineChainsChecks.cs`: rendered-pixel, endpoint, pose, fade and cancellation
  checks, including deliberate negative controls. This is editor-only code.

To change chain appearance, start with the runtime files and
`Resources/Effects/IronChainMetal.shader` (historical filename, now a toon shader).
To change the demonstration pose or target placement, change the fixture, not the sim.
The optional caster aura belongs only to the first chain, not every target.

## Seraphine Live Integration (2026-09-26)

`ArenaView.Seraphine.cs` owns per-cast effects, clocks, sprite poses, hit-flash scope
and cleanup. No editor asset or preview fixture is needed at runtime. Basic sword
edge and Aegis retain their existing integration. Aegis hides in-flight skill VFX
while its presentation hold freezes the arena, then restores them.

Timing follows `docs/unity-skill-timing-handoff.md` (sim 0.0.481): pending Aegis
and Iron Chains use Seraphine's open-hand row 3; Steel Break uses the first two
attack preparation frames. Prediction changes the sprite only, never fires VFX.
`motion_lock` cancels basic strike latches and holds the skill pose until the sim
releases it. Resolved slash/chain sprite poses take priority over the basic state
machine, but their visual tails cannot hold the sprite into the next action.
Aegis still triggers its cut-in only from `ult_fx`. Its cast pose is a reused
open-hand clip, not a bespoke Aegis animation. Other units without authored skill
mapping hold their last pose during motion lock; their wind-up mapping is separate work.

The current sim's instant self-AoE branch does **not** populate `skill_fx`.
Steel Break and Iron Chains therefore read `skills[].last_fired == view.tick`,
deduplicate `(caster, skill)` within the tick, and use that caster's `recent_hits`
IDs for recipients. No client radius, nearest-target selection, damage or cooldown
calculation. Initial/session/room baseline snapshots do not replay old casts.
This grouping assumes this kit's one instant offensive cast per tick; `recent_hits`
does not identify individual skills. Per-skill hit provenance and low-frame-rate
tick-batch event retention remain bridge-level limitations, not solved here.

The chain uses the existing linked-geometry renderer (not a slash carrier). Only
the visual glide is re-timed for catch/yank; its destination is the sim's cell.
Slash and aura roots are detached world-space anchors. Live impact transforms are
placed once, then fade by shader opacity; previous editor studies stay unchanged.

`SeraphineRuntimeProbe.Run` feeds real FFI snapshots through `ArenaView.FeedView`
in isolated preview scenes. Checks: two-target chains, Aegis event recipients,
same-tick dedup, UV head travel and unchanged carrier vertices/transforms, scoped
flash, expiration, reset/retry/baseline, and sabotaged event/target/pixel checks.
The timing fixture fills only an isolated session's Aegis gauge, then lets the
native sim produce pending/resolve/lock. It measures the actual rendered sprite
row, holds beyond the normal swing duration, and checks basic recovery on unlock.
Removing pending_skill or motion_lock deliberately makes the pose guard fail.
Landscape/portrait renders go to `/tmp/roi-seraphine-runtime`.
This is not an authenticated online playthrough; test the final combat framing
and pacing in the actual party. Restart Unity if its loaded native sim version
differs from the staged `Assets/Plugins/macOS/libsim_ffi.dylib`.

## Run And Verify

Use Unity's `ROI > Art` menus. Batch entry points remain:

- `SeraphineChainsPreview.Capture`: one target, `/tmp/roi-seraphine-chains`.
- `SeraphineChainsPreview.CaptureTwoTargets`: two targets, `/tmp/roi-seraphine-chains-two-targets`.
- `SeraphineRuntimeProbe.Run`: real-event runtime integration checks and viewport stills.
- `SuzuSunderPreview.Capture`: two-target fire sweep, `/tmp/roi-suzu-sunder`.
- `MaykerFireSlashPreview.CaptureSuzuSkills`: latest vendor-derived fire orbit
  with caster aura, then Bell Strike; see [orbit](suzu-sunder/free-slash-pack/README.md)
  and [Bell Strike](suzu-bell-strike/README.md).
- `SuzuSlashBaseTexturePreview.Capture`: isolated white wedge mask on the lens
  mesh, one-shot UV pass, `/tmp/roi-suzu-base-texture`; [handoff](suzu-sunder/base-texture.md).
- `SuzuSlashAttackPreview.Capture`: five white-base attack variants,
  `/tmp/roi-suzu-slash-attacks`; [comparison and handoff](suzu-sunder/attack-variants/README.md).
- Other effects retain their `Seraphine*Preview` entry points; see their handoffs.

Do not edit C# while a Unity capture is running. Captures use an additive temporary
scene and must not save the authored scene. Verify both landscape and portrait.
For structural changes, capture a baseline first and compare rendered frames as well
as numeric checks. Numeric guards include sabotage so a disabled layer cannot pass.

Retain the current accepted preview, a useful slow version, and necessary source
references. Raw PNG sequences live in `/tmp`, not under Assets or docs. Remove them
after verification; the capture commands regenerate them. Superseded chain clips
were removed with the user's approval. Older non-chain studies remain reference work,
not assets to ship. Do not delete runtime textures based solely on similar filenames.

## Reference Studies

- [Anatomy of a Slash](anatomy-slash-study/README.md)
- [Octopath Divine Skills](octopath-divine-study/README.md)
- [Sword of Convallaria](soc-vfx-study/README.md)
- [UV motion comparison](seraphine-uv-study/README.md)
- [Generated flipbook comparison](seraphine-flipbook-study/README.md)
