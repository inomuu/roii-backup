# Regalia — sim / gameplay design (the weapon OVERLAY)

> **0.0.400 (2026-09-16) — SHAPE CHANGED to a ZZZ W-Engine, user decision.** A Regalia = a fixed
> **ATK lump** + one **advanced stat**, both on the Regalia's OWN level (cap 60; lump ×15 and advanced
> ×2.5 from Lv1 → Lv60 — ZZZ's ratios: Steel Cushion 46 → 684 base ATK, 9.6% → 24% crit rate; S/A/B at
> Lv60 ≈ 580-680 / 430-480 / 290-330 base ATK and 18-24% / 12-18% / 6-10% advanced). Tiers R / SR / SSR
> = lump 100 / 140 / 200 at Lv1 (1,500 / 2,100 / 3,000 at Lv60, ≈ 27% of a launch-cap character
> beside a Beyond s7 weapon); advanced by line: sword crit dmg · thief crit · archer ATK% · mage CDR ·
> acolyte HP% (SSR 8.8% → 22%). **Passive (the Resonance web) is EMPTY for now** — the sections
> below describing main stat + passive triggers are the design intent for later, not what ships.
> Fodder: R shard +5 levels, SR shard +15 (⚠️ provisional — price them in `gacha-economy-design.md`).
> Code: `items.rs` `REGALIA_LADDER`, `regalia_lump_at`, `regalia_advanced_at`, `RegaliaDef.{lump_base, advanced}`.

**Status:** design v1, 2026-07-28. This is the SIM / GAMEPLAY layer of Regalia — the overlay
mechanic, stats, signature animation, and build plan. **The ECONOMY (currency, pity, pass, banner
cost, rarity mix, roster) lives in `docs/gacha-economy-design.md` — that doc is authoritative for
money; do NOT redefine it here.** Regalia is that doc's **"Weapon banner"**.
**DESIGN ONLY — gacha build deferred (user); P0/P1 below stand alone without gacha.**

Terminology (from gacha-economy-design.md — never conflate): **Rarity = SR / SSR** (two tiers) ·
**Resonance = R0–R5** (the dupe/"star" track). "R0–R5" is Resonance, NOT a rarity.

## 1. What a Regalia IS

A **signature-weapon OVERLAY** — the weapon analog of an Exemplar. Its own slot on a party member;
it does NOT go in the weapon slot and does NOT replace the crafted/refined weapon (that stays
equipped underneath). **BOUND, account-only, never in `inventory`/`stash`/Gil market** — lives in
its own server-entitlement collection (`owned_regalia`, the twin of `owned_exemplars`), exactly
like an Exemplar. Power comes from **Resonance only, never ore-refine** (refine stays a
dropped/crafted-gear sink — see gacha-economy-design.md#weapon-banner).

Equipped, a Regalia grants: a **signature attack animation** + a **stat package** (below) +
optionally a **signature skill** (a `GrantedSkill`, same as an Exemplar).

## 2. Stat framework — signature-weapon pattern (main stat + layered passive)

Modeled on real gacha signature weapons (Genshin weapons / HSR Light Cones / ZZZ W-Engines — see
the passive study below). Reuses the `BuffKind` / `CardEffect` vocabulary (same as cards/Exemplars);
triggers come from `trigger-vocabulary.md` (all are AUTOMATIC — no manual input in an idle battler).

A Regalia = **MAIN STAT + PASSIVE**, where the passive has three parts:
1. **MAIN STAT** — one headline %-stat (the always-on substat). Flat weapon ATK barely moves
   combat (`gear-atk-minor-vs-stats`), so use %: `CritDmg` / `MatkPct` / `AtkPct` / `HpPct` / …
2. **FLAT baseline** — a small always-on bonus on a **DIFFERENT axis** than the main stat (cover two
   stats, never double the same one).
3. **CONDITIONAL / STACK** — a stacking or conditional mechanic on a **generic auto-trigger**
   (on-hit / on-cast / on-kill / debuff-death / element), often with a payoff. This is the
   "signature".

**Design rules (real-pattern-grounded):**
- **best-on, not fixed** — triggers are GENERIC events (Mistsplitter fires on "Normal Attack deals
  Elemental DMG", not a named skill). A Regalia is BEST on its paired Exemplar but usable by anyone
  of that role/element; off-role lines simply idle (no crash).
- **same-name passives don't stack** (universal gacha rule).
- **Resonance R0→R5** scales the numbers (per-stack %, cap, condition) — the Superimposition analog.

Scale (placeholder — calibrate via sim-cli vs Exemplar grants + monster ATK). **Calibration rule:**
an ALWAYS-ON total-damage multiplier (`AtkPct`/`MatkPct`) is far stronger than it looks — keep it
MODEST (~+20–25% SSR main). A CONDITIONAL stat (`CritDmg`, applies only on a crit; element-dmg-%,
only vs that element) can run higher (~+40%). R0 flat ~+12% on a second axis; per-Resonance-★ scales
the conditional's key number.

### Rarity tiers + two progression axes (R / SR / SSR)

Three rarities, and a Regalia grows on **two independent axes** — the standard gacha-weapon model
(Genshin weapon Level + Refinement; HSR Light Cone Level + Superimposition):

| Axis | Fed by | Scales |
|---|---|---|
| **Enhancement Level** (Lv0 → max) | consuming **R Regalia as FODDER** (+ a Gil cost) | the **MAIN STAT + FLAT** up to full effectiveness |
| **Resonance** (R0 → R5) | **dupes** (a duplicate of the same Regalia) | the **PASSIVE** (per-stack %, cap, unlocks) |

"Full effectiveness" = **max Level** (fed enough R fodder); a **stronger passive** = **Resonance**
(dupes). Flow: pull an SSR → level it with R fodder (raises stats) → dupe it for Resonance (raises
the passive).

| Tier | Role | Passive | Main stat @ Lv0 → max |
|---|---|---|---|
| **R** | **FODDER** — fed to Level up SR/SSR (if equipped: a weak stat-stick) | none | +6–8% flat |
| **SR** | accessible real Regalia — Level + Resonance | one simple conditional | +10% → **+20%** |
| **SSR** | flagship signature — Level + Resonance | layered signature (§ above) | +12% → **+24%** |

**Why R-as-fodder:** an R pull is never worthless (it is EXP food), and feeding it is a
material/Gil sink. Reconciles with `gacha-economy-design.md` weapon banner ("power from Resonance
stars") by ADDING the Level axis — note it is R-fodder EXP, NOT the craft/refine ORE system (which
stays a dropped/crafted-gear sink, per that doc).

**RARITY-GUARANTEE RULE (hard, but keep the gap SMALL):** a **fully-invested SR (max Level) must be
≤ a fresh SSR (R0, 0 stars)** — pulling ONE SSR (even 0 dupes) always edges out a maxed SR — **but
only just.** A maxed SR should be COMPETITIVE (~85–90% of SSR-R0), so f2p with a maxed SR isn't left
far behind; the SSR's real lead comes from **Resonance** (dupes), not from its R0 baseline. Current
numbers: SR-max (main +20% + one simple conditional) sits just under SSR-R0 (main +24% + flat +12% +
the layered signature at base) — a small edge on stats, SSR pulling ahead mainly via its passive
and then widening the gap through R1→R5. If an SR ever meets or beats SSR-R0, cut the SR (never buff
the SSR past this line).

### The first two Regalia (paired with the built Exemplars — Seraphine + Lucielle)

**⚔️ Aurora Edge** — Seraphine (Holy physical melee). Archetype: **follow-up combo DPS**.
- MAIN: `CritDmg` +40%
- FLAT: +12% Holy DMG dealt *(different axis from CritDmg)*
- STACK: **on-hit** → "Cadence" stack (cap 3, refresh); each stack **+Crit Rate (`crit_bp`)** — crit
  ramps as she sustains hits, so the CritDmg main pays off (crit chance is scarce elsewhere, so the
  weapon supplies its own crit PAIR).
- PAYOFF: at 3 stacks → a **follow-up Holy slash** (AoE ~150% ATK, Holy) → consume stacks. The
  follow-up is a tagged attack → other on-hit/crit effects can chain off it (`trigger-vocabulary.md`
  ➕ follow-up primitive).
- Resonance: Holy DMG flat, +Crit Rate/stack, follow-up %.

**✨ Litany of Dawn** — Lucielle (Holy magic support). Archetype: **Holy-Bolt caster + team support**.
- MAIN: `MatkPct` **+24%** *(an always-on total-magic multiplier is strong — keep these modest;
  contrast CritDmg, which only applies on a crit. Placeholder → calibrate via sim-cli.)*
- FLAT: **+Healing done (`heal_bp`)** *(support axis, ≠ MATK; Radiant Saint theme)*
- SIGNATURE (buff a CLASS skill, not a named one → generic): **Holy Light (`holy_light`) +40% dmg +
  fires a 2nd bolt at a nearby enemy** (reuses its Radiance-chain rune FX). `holy_light` is the
  acolyte-line Holy Bolt → any acolyte running it benefits; best on Lucielle (Benedictio + Sagitta
  Holy loop).
- TEAM: **on-cast** (any skill) → team gains "Devotion" stack (cap 4, 10 s), each **+6% Holy DMG
  dealt team-wide** (support amp, Robin/Lucy pattern).
- Resonance: heal done, Holy Light dmg, Devotion/stack + cap.

**Impl notes (P0):** `crit_bp` is already grantable via `CardEffect` (→ `effective_crit_chance_bp`).
`heal_bp` exists (`gear_mods::heal_bp`, scales heal output) but is currently GEAR-only — fold the
Regalia/overlay `heal_bp` into the effective heal sum. The **follow-up attack** is a new ➕ primitive
(reuses the attack pipeline; see `trigger-vocabulary.md`). Buffing a specific skill (`holy_light`)
needs a per-skill damage/extra-bolt hook.

Guardrail (`p2w-defensible-framing` + gacha-economy-design.md): a Regalia makes a character farm
BETTER, it must not trivialize farmed/refined gear or skip the farm hook. SSR R5 is strong but the
piece is BOUND, PvE stays F2P-beatable, PvP power-matched.

## 3. Signature attack animation (the flair hook)

The reason a Regalia feels special. Reuse the animator's per-skin/anim-slot override path (the
same one Exemplar skins use to swap the attack clip): a Regalia declares a signature attack clip
that OVERRIDES the class's basic `attack` (and/or a `signature_1` ANIM_SLOT), scoped to the
weapon/attack — the character keeps its class body/skin (unless ALSO wearing an Exemplar).

## 4. Architecture — reuse the Exemplar overlay verbatim

REUSE (near-copy of the built Exemplar system, 0.0.199):
- `Entity.exemplar: Option<Exemplar>` → add `Entity.regalia: Option<Regalia>`.
- `state.owned_exemplars: Vec<Item>` → `owned_regalia: Vec<Item>`.
- `EquipExemplar/UnequipExemplar` + merge=Resonance → `EquipRegalia/UnequipRegalia`.
- `Exemplar` struct (stat grant + `GrantedSkill` + `star_ranks`) → `Regalia` (same shape, minus
  the character SKIN, plus a `signature_anim` id for the attack clip).
- Stat/skill apply on equip (`apply_exemplar_skill`, derived-stat overlay), Resonance `star_ranks`.
- Godot Exemplar collection/equip + Resonance UI → the Regalia collection/equip view.

The GACHA PULL itself (server-authoritative, keyed-nonce grant, currency, pity) is NOT here — it is
the economy doc's job, and it reuses the server-roll grant pattern (cards/drops).

## 5. Build plan

- **P0 — Regalia overlay in sim** (mirror Exemplar): `Regalia` struct, `owned_regalia`,
  `EquipRegalia/UnequipRegalia`, Resonance merge, stat/skill apply. Grant via DEBUG only. Stands
  alone — no gacha, no server needed.
- **P1 — Signature attack animation** (Godot): the flair hook. Also standalone.
- **P2+ — Gacha (server + UI)** — see `gacha-economy-design.md`; DEFERRED per user.
