# Sprite Brief — Haunted House Dungeon (5 monsters + 1 boss)

For the pixel artist. Game: *Reign of Idlers* — an idle ARPG inspired by Ragnarok
Online. Gothic haunted-mansion / full-moon theme. All monsters here are the
"humanoid" bipedal type and share ONE sheet format below.

## Technical spec (must match the engine)

- **Sheet layout: 10 columns × 4 rows.** One animation per row, in this order:
  - **Row 0 — Idle** (loop)
  - **Row 1 — Walk** (loop)
  - **Row 2 — Attack** (one-shot; the blow should LAND around frame 5–6)
  - **Row 3 — Death** (one-shot)
- Up to 10 frames per row. Fewer is fine — leave the trailing cells **empty/transparent**
  (tell us the real frame count per row so we don't flicker on blanks).
- **Frame size is flexible** (engine derives it from texture ÷ cols/rows), but keep
  every frame the SAME size and the character centered. Suggested ~**64×64 or 80×80**
  per frame (boss can be larger, e.g. 96×96).
- **PNG, 32-bit, transparent background.** No padding offsets — content aligned in-cell.
- **Pixel art: hard edges, NO anti-aliasing / soft edges** (engine upscales with
  NEAREST — soft edges turn to mush).
- **Face RIGHT only** — the engine mirrors horizontally for left. Keep designs
  flip-friendly (no text / hard asymmetry that breaks when mirrored).
- Match the existing RO-ish palette + outline weight.
- Deliver one PNG sheet per monster, filename = monster id (e.g. `werewolf.png`),
  dropped in `assets/monsters/`.

## Element color cue (for readability)

Each monster has an element — give a subtle palette/aura hint so players read it:
- **Shadow** = dark purple / black-violet
- **Water** = pale blue / icy
- **Wind** = teal / pale green, wispy
- **Poison** = sickly green

---

## The roster

### 1. Werewolf — Shadow — *bruiser*
Lean, hunched, ferocious wolfman; full-moon hunter. Fast and aggressive — reads as
"glass-cannon rusher," not a tank. Claws prominent (its attack causes bleeding).
- Idle: heavy breathing, twitchy, ready to pounce.
- Attack: a fast lunging claw swipe.
- Death: collapse / revert. (Optional: a brief "enrage" look is handled by a red
  tint in-engine at low HP — no extra frames needed.)

### 2. Marionette — Shadow — *melee DPS*
Creepy puppet on strings (RO Marionette vibe) — wooden joints, painted face, strings
trailing up off-frame. Floaty, jerky movement. Fragile, hits fast/repeatedly.
- Idle: dangling, slight sway on the strings.
- Attack: quick repeated jabs / a string-whip (it hits in a rapid flurry).
- Death: strings cut → puppet crumples.

### 3. Maid (Alice) — Water — *support / debuff*
RO **Alice**-style French maid (apron, bonnet, feather duster) — but ghostly/eerie.
A "polite" support enemy: heals other monsters and dusts your armor away. Should
read as a PRIORITY target (slightly fancier / distinct silhouette).
- Idle: prim curtsy / dusting motion.
- Attack: a feather-duster sweep or a small "service" gesture (cast pose).
- Death: faints elegantly.

### 4. White & Black Spirit — Wind — *critters (a pair)*
Two small ghostly sprites (RO Whisper feeling) — same shape, opposite shading:
- **White Spirit:** soft/glowing, gentle — the guardian (it shields the black one).
- **Black Spirit:** dark, sharp-eyed — the aggressive one (its hit blinds).
Small, floaty, evasive. These are the dungeon's chaff (appear in numbers).
- Deliver as **two recolored sheets** (white & black) of the same body. Small frame
  size is fine (e.g. 48×48).
- Idle: bob in the air. Attack: a quick darting nip. Death: pop / dissipate.

### 5. Witch — Poison — *ranged caster*
Classic hag/witch — pointed hat, hunched, cauldron or potion in hand. Stays back
and lobs poison. Sickly-green accents.
- Idle: stirring a cauldron / muttering.
- Attack: a throwing/casting motion (lobs a poison flask — the projectile + puddle
  are engine VFX, you just need the throw pose).
- Death: melts into her own brew.

---

## BOSS — Dullahaness — Shadow — *headless knight (female)*
A female Dullahan: tall armored headless knight who **carries her own head** (under
one arm or in hand). Imposing, regal, terrifying — the manor's master. Larger frame
(~96×96). Dark armor with violet ghost-fire accents.
- **Idle:** standing menace, cape/ghost-fire flicker; the head's eyes glow.
- **Walk:** slow, heavy, commanding.
- **Attack:** TWO ideas if you have room — (a) a heavy sword cleave (melee), and
  (b) a "throw the head" wind-up (ranged). If only one row, do the **sword cleave**;
  we'll reuse it. (A scream/AoE phase is an engine VFX over the idle — no frames needed.)
- **Death:** armor falls apart, ghost-fire gutters out, head rolls.
- Nice-to-have: a separate small **head** sprite (1–2 frames) we can animate flying
  for the "throw head" attack — optional.

---

## Deliverables checklist
- [ ] `werewolf.png` (10×4)
- [ ] `marionette.png` (10×4)
- [ ] `maid_alice.png` (10×4)
- [ ] `spirit_white.png` + `spirit_black.png` (10×4, small)
- [ ] `witch.png` (10×4)
- [ ] `dullahaness.png` (10×4, larger) — optional `dullahaness_head.png`
- For each: confirm the **real frame count per row** (so empties don't flicker)
  and the **attack "impact" frame** (where the blow lands).
