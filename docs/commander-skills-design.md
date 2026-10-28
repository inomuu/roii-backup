# Commander skills — 5 abilities the player owns, not the party

**Status: DESIGN, not built. Drafted 2026-08-21.** Decisions taken from the user in this round are
marked (user). Everything else is a proposal with its reasoning, and the open questions are at the
bottom in one block.

Related: `TODO.md` § COMMANDER LEVEL (the unlock gate, downstream of quest), memory
`commander-active-skills`, `docs/combat-formulas.md`, `docs/skills-catalog.md`.

---

## What this is

The player is the **Commander** — not a character on the board. These are the abilities they use
*themselves*, from any zone, on top of whatever the party is doing. One already ships:
`regent_hand` lifts a member off the board and works everywhere (decided 2026-08-18, and no golden
moved when it did).

Three decisions from the user frame the rest:

- **Damage is in scope** (user). At least one of these hits.
- **`commander_smite` is the FIRST UNLOCK, not a starter** (user, 2026-08-28 — this REVERSES the
  earlier "all five owned from the start"). Ownership is **per-account** (user): the server holds
  it and re-issues it into every save, so it survives a wipe and a party swap. The *trigger* —
  what actually unlocks it — is **still undecided** (Commander level, story/zone clear, and a soul
  node are the candidates; Commander level is downstream of quest, which is unbuilt). Until the
  real trigger and the ownership spine land, smite stays always-castable **as a debug stand-in
  only** (user: "smite grant ไว้ เพราะ debug เฉยๆ") — reachability during development, NOT a
  design that all five are free. See § Ownership; the spine is deferred to ship WITH its first
  real trigger so nothing is built wired to nothing (`systems-built-but-unreachable`).
- **Useful but never mandatory** (user: "ต้องมีประโยชน์แต่ไม่บังคับ"). Press them and it goes
  better; ignore them and nothing is lost. That is also the pillar constraint — this game is idle,
  and a Commander skill you must babysit turns the overworld into an action game.
- **Some of them auto-cast** (user). This is what makes the previous point real rather than
  aspirational: see below.

---

## The engine already does all of this — reuse, don't invent

Checked before designing, per the house rule:

| what is needed | what already exists |
|---|---|
| auto-cast on a timer | `SkillKind::Active` — *"auto-cast in place of a basic attack, gated by cooldown + range only"*. `sp_cost` is 0 everywhere; cooldown is the single power dial. |
| damage that scales | `power_bp` — *"damage as a basis-point multiple of effective ATK/MATK at level 1"* (`bash` is `18_000` = 180%, `+2_000`/level). |
| availability anywhere | `events/handlers.rs::player_skill_available`, whose doc comment already reserves the "second source" slot for exactly this. |
| a shield / a cleanse / a mark | the buff and field machinery skills already use; nothing new. |
| delivering an unlock | the mailbox (`ClaimMail`) and the schedule+claim machine, both built. |

**The one genuinely new thing is ownership**, and it is a security question, not a gameplay one —
see § Ownership below.

### How the damage scales, given the Commander has no stats

**It does not read a stat at all.** An earlier draft cast smite THROUGH a party member so it could
borrow that member's ATK via `power_bp`; the flat `5 × party average level` model (see § 2 below)
replaced that, and with it the only reason a channeller existed. Nothing about the skill reads a
caster now — not ATK, not element, not crit — so it does not have one.

That removes a whole class of question rather than answering it: no "who channels it", no tie-break
rule, no behaviour change when that member dies mid-cast. `click_strike` already sets the
precedent, killing monsters with `kill_monster(state, idx, None)` — an unattributed kill routes
through the normal drop path perfectly well.

---

## The Commander's BASIC ATTACK is already built, and it is ability zero

**(user, 2026-08-21: click-attack exists, make it the first one.)**

`InputEvent::ClickAttack { target_id }` → `click_strike(state, target_id, 10)` — a flat 10 PURE
hit, rate-limited to **one click per tick** by `state.last_click_tick` (deterministic and
replay-safe, so the input-authority question is already answered), and a click-kill routes through
the same drop and reward path as any other kill.

It belongs at the front of this list, but as the Commander's **basic attack**, not as a sixth
skill — and that framing resolves the one thing that looked like a contradiction.

### Why it must NOT scale, when the other five do

Its own doc says the flat number is the point: *"strong vs low early-game HP and trivial vs
late-game HP … so it never accelerates late-game farming."* That reads like it argues with the
party-average-level scaling above. It does not — it is the same rule applied to a different role:

- a **basic attack** is the thing you can do constantly, so it must stay small, or an idle game
  becomes one where you have to click to keep up;
- a **skill** is on a cooldown, so it can afford to be big.

Every unit in this game already works this way. Giving the Commander the same shape — one small
constant action plus five big occasional ones — is not a compromise between two designs, it is the
structure the rest of the game already uses.

**So the flat 10 stays flat, deliberately, and this document is now where that is written down.**
Anyone later tempted to make click damage scale with party level is undoing the idle pillar, not
tuning a number.

It is also the natural **first thing a player is taught** — it needs no unlock, no ownership grant
and no cooldown to explain, and Tutorial is still an open row in `TODO.md`. "You can act" is a
cheaper first lesson than any of the five.

---

## The five

| # | id | what it does | auto? | cooldown | scaling |
|---|---|---|---|---|---|
| 0 | `click_attack` | flat 10 pure damage on click (**already built**) | no | 1 tick | **none, on purpose** |
| 1 | `regent_hand` | lift a member off the board (**already built**) | no | existing | — |
| 2 | `commander_smite` | AoE hit on a placed cell (cursor = top-left) | **yes** | 600 ticks (60 s) | **flat `5 × party avg level`, PURE, 2×2** — ~4-5% of a boss from Lv30 to the cap; soul path raises power and grows it to 3×3 |
| 3 | `commander_bulwark` | shield one member for a flat % of their MaxHP | **yes** | 450 ticks (45 s) | 25% of the target's MaxHP, decays after 10 s |
| 4 | `commander_mark` | mark one enemy; the party focuses it | **yes** | 300 ticks (30 s) | no numbers — pure targeting |
| 5 | `commander_recall` | pull the whole party back to the spawn edge | **no** | 900 ticks (90 s) | — |

### 2 · Smite — the damage one

**A FLAT VALUE FROM PARTY AVERAGE LEVEL, not a % of ATK** (user, 2026-08-21):

    damage = 5 × party_avg_level          # PURE, AoE

The user's reason for moving off `power_bp`: a % of ATK is *"เก่งไปเลย ไม่ก็อ่อนไปเลย"* — a Mage
party and a Swordsman party have wildly different ATK, so the same multiplier is a different skill
depending on the build. A flat value keyed to level is the same skill for everyone.

**Measured against the real catalog before agreeing to it**, because a flat number against scaling
HP is how `click_attack` was designed to FADE — the failure mode was worth checking rather than
assuming:

| party avg lv | boss HP (catalog) | 5 × lv | share of a boss |
|---|---|---|---|
| 10 | 120 | 50 | 42% |
| 20 | 966 | 100 | 10.4% |
| 30 | 3,175 | 150 | 4.7% |
| 40 | 4,125 | 200 | 4.8% |
| 50 | 5,455 | 250 | 4.6% |
| 60 | 7,215 | 300 | 4.2% |
| 70 | 10,390 | 350 | 3.4% |

It holds at **4-5% of a boss from Lv30 to the cap** — a straight line happens to run parallel to
the boss-HP curve across the range that matters, which is why this works where `click_attack`'s
flat 10 does not. Strong early is intended, and by Lv10 a fresh account has no path nodes and a
weak party, so a 42% chunk on a 60 s cooldown is help rather than a trivializer.

**5×, not 10×, because the soul path is the upgrade** (user). The base is deliberately modest: a
player who never invests keeps a minor nice-to-have, a player who spends soul on the Commander arm
turns it into a real tool. That is "useful but never mandatory" made structural instead of
promised — the power is opt-in and paid for with an existing sink, rather than handed out.

**PURE damage.** The whole reason for leaving `power_bp` was predictability, and a normal physical
hit hands that straight back: DEF would make the same skill worth a different amount against a
heavy-armour boss than a soft one, reintroducing through the back door exactly the variance the
flat value removes. `click_attack` already takes `DamageType::True` for the same reason.

**AoE, so PACK SIZE is the real power dial**, not the number: 250 against a pack of five is 1,250
delivered.

**2×2 at base, 3×3 as a soul-path upgrade** (user, 2026-08-21).

⚠ **2×2 IS NOT EXPRESSIBLE TODAY.** `aoe_radius` is a Chebyshev radius measured from a CELL, so
every shape the engine can make is odd-sided. This is therefore not a tuning choice — **the
half-cell work in the appendix is a prerequisite for smite, not groundwork for later.** It has to
land, with the goldens unmoved, before this skill can be authored at all.

What the two sizes mean in practice, on the 20×10 board every zone uses with a target population
of 5-7: a **2×2 (4 cells)** realistically catches **1-2** of an approaching pack (~375 delivered
at Lv50), a **3×3 (9 cells)** catches **2-3** (~625). The radius node is therefore worth roughly
**1.7×**, on top of whatever the power nodes add.

A 4-cell base is a better fit for "5×, and the path is the upgrade" than the 3×3 first drafted —
it keeps the un-invested version genuinely small, which is the whole point of the modest base.

It also gives the Commander path arm two DIFFERENT node types, power and radius, instead of one
node repeated at rising cost. An arm where every node says the same thing is a slider, not a
tree.

⚠ **The soul-path bonus is no longer free.** While this was a % of ATK, the whole Power arm raised
it with no code (see § Soul path bonuses). A flat value ignores ATK, so that implicit bonus is
GONE and `PathEffect::CommanderPowerPct` stops being optional — it is now the only way a path can
touch this. That is a fair trade and arguably the better one: the bonus becomes visible and
countable instead of arriving invisibly through a stat.

### 3 · Bulwark — the one that makes idle play better

Auto-cast, and the auto rule is what makes it worth having: **fires on the member below 40% HP,
lowest first**. A player watching can pre-shield the tank before a boss swing; a player who is not
watching still gets the save. Same skill, two ways to be right, and the idle one is not a worse
version.

% of MaxHP rather than a flat number, so it never needs a band table.

### 4 · Mark — the Commander-flavoured one

No damage, no defence: it decides *what the party hits*. That is the ability a commander should
have, and it is the one that pays off most in a party-comp game — focusing the healer in a mixed
pack is a real decision that costs nothing to ignore.

Auto rule: **mark the highest-threat enemy** — proposed as highest ATK, ties by spawn order.

### 5 · Recall — deliberately manual only

A retreat nobody asked for is a disaster, and there is no auto condition that is right often
enough. It is also the natural pair to the auto-retreat farming already in
`loot-items-and-autoretreat`.

---

## Cooldowns AND a shared charge pool

**(user, 2026-08-21: "จะทำเป็นแต้มชาร์จรวม แล้วใช้ร่วมกัน ควบคู่กับระบบ CD".)** Two limiters, not one:
each skill keeps its own cooldown, and every cast also spends from ONE pool the Commander owns.

### What the pool buys that cooldowns cannot

Cooldowns are independent — four skills on four timers means the correct play is always "press
whatever is ready", which is not a decision. A shared pool makes them **compete**: spend on smite
now or hold the charge for a bulwark that is not needed yet. That is the difference between four
buttons and a resource.

It also gives the whole family ONE master dial. Retuning four cooldowns is four balance arguments;
retuning the regen rate is one, and it moves everything together.

### Reuse note: per-skill charges exist, a shared pool does not

`SkillDef.max_charges` → `SkillState.charges_left` is already in the sim (Soul Crush, Hiding, the
elementalist's), but it is **per skill and per band** — seeded at creation, refilled on a zone
change. That is a different thing: a budget attached to one skill, not a currency shared across
several. The pool is new state.

Shape, kept as small as the rule allows:

    GameState { commander_charges: u32, commander_charge_tick: u64 }

Deterministic regen from the tick counter, no wall clock, no float — one charge per N ticks up to a
cap. It is replayed state the verifier recomputes, so a client cannot forge a charge any more than
it can forge a level.

### ⚠ Auto-cast must not starve the manual player

The sharp edge, and it is worth deciding before it is discovered: three of the six default to AUTO.
If auto draws from the same pool without restraint, a player who reaches for smite at the moment
that matters finds it unaffordable **because their own bulwark spent the charge while they were not
looking** — the idle convenience actively taking the active play away.

Options, in the order I would try them:

1. **Auto only fires above a reserve** (e.g. it will spend down to 1 charge, never the last). Cheap,
   one comparison, and it reads correctly: the Commander's automation is cautious with the last of
   its power.
2. Separate pools for auto and manual. Honest but doubles the state and the UI.
3. Auto costs less than manual. Rewards not playing, which is backwards.

(1) unless something argues against it.

### Open numbers

Cap, regen interval, and per-skill cost. `commander_smite` at 2 charges against `commander_mark`
at 1 would make the cheap utility genuinely spammable and the damage a considered spend — which is
probably the shape wanted, but it is the sort of thing `sim-cli` should measure rather than a
document assert.

**A third soul-path node type falls out of this**, and it may be the best one on the arm: raising
the CAP or the regen rate improves every Commander skill at once, which is what an arm dedicated to
the Commander should feel like — better at commanding, not just a bigger number on one spell.

---

## Soul path bonuses — half of this already works

**(user, 2026-08-21: Commander skills get a bonus from soul paths.)**

### For SMITE there is no implicit half — that changed with the damage model

While smite was `power_bp × ATK` the whole Power arm raised it for free, because path nodes are
stamped as buffs under `PATH_AURA_SOURCE` and fold through `buffed_stats` into exactly that
number. **The flat `5 × party average level` model ended that**: it reads no stat, so no path node
can reach it except one written for the purpose. `PathEffect::CommanderPowerPct` is therefore not
optional for smite — it is the only lever there is.

Arguably the better trade. The bonus becomes visible and countable instead of arriving invisibly
through a stat, and nobody has to account for a hidden multiplier when tuning the node.

**`commander_bulwark` still gets its half for free**, and this is worth keeping straight: the
shield is a % of the target's MaxHP, so `HpPct`, `HpAdd` and `VitAdd` already feed it with no code
written. So the two skills genuinely differ — one is path-fed already, the other is not — which is
exactly the kind of thing that produces a double-counted node if it is not written down before the
arm is designed.

### The explicit half is a new `PathEffect` and a new arm

If the Commander deserves its own nodes — which is what "a bonus from soul paths" most likely
means — the shape is:

    PathEffect::CommanderPowerPct     // magnitude = % added to Commander skill power

`soul_paths.rs` is a const web of `PathNode`s and the STATE is only which node ids are owned, so
this is a catalog addition rather than a schema change. Allocation is already an `InputEvent`,
already self-cost-gated by `soul`, so nothing new has to be trusted.

⚠ **A Commander arm must NOT include cooldown reduction.** Every other rule in this document is
about keeping these "occasionally" rather than "on rotation"; a path node that shortens the
cooldown is the one bonus that converts the design into the thing it was built to avoid. Power,
duration and shield size are all safe. Cooldown is not.

⚠ **Adding nodes may move goldens** — check with `cargo test -p sim-core --tests` before blessing,
and bless only if the drift is the new nodes and nothing else.

**PvE only, and that is already decided.** `pvp-excludes-soul-paths` makes path bonuses PvE-only,
so a path-boosted Commander skill inherits the exclusion rather than needing a new rule. It is the
same reason § What this must NOT do already keeps these out of PvP.

---

## Ownership — the only genuinely new plumbing

The sim cannot ask the server anything: it is deterministic and offline by construction. So
"which Commander skills do I own" has to be **state in `GameState`**, which means it enters
through an `InputEvent`, which means it is forgeable unless something checks it.

That path is already built and already guarded:

- a new `GrantCommanderSkill { skill }` variant tagged `InputAuthority::Server`. It carries NO
  power number — the multiplier comes from party level, which the verifier already recomputes;
- its doc is blunt that the tag is a **classification, not a guarantee** — *"reading it as one
  cost us a live mint hole"* — so each variant needs its own verdict;
- `verify-server`'s `server_event_is_checked` enumerates the verdict per variant and **defaults to
  reject**, and `every_server_authored_event_has_a_verdict` **fails the build** when the list
  grows.

So the compiler forces the security question to be answered before this can ship. That is the
`make-the-compiler-the-gate` pattern, and it is why this is safe to start before Commander level
exists.

**Today smite is always-castable as a DEBUG stand-in** (user, 2026-08-28), not because it is free —
the design is that it is the FIRST thing unlocked (§ Three decisions), **per-account**. The
ownership spine (`commander_skills_owned: BTreeSet` + a server-authored `GrantCommanderSkill` +
the verify verdict) is DEFERRED to land WITH its first real trigger, not now against a stand-in
author: the plumbing does not depend on which trigger wins, but building it early buys a system
wired to nothing until the trigger exists. Commander level is the intended gate and is downstream
of quest (unbuilt), so there is no near-term consumer. When it lands it is a data + server-issue
change on top of this spine, not a rewrite; `player_skill_available` gains its second source there
and must ADD, never replace, the zone source.

`player_skill_available` gains its second source here. It must **ADD**, never replace: the zone
source stays, because Deathknell's 3 s wind-up is designed around the boss room granting the
answer to it.

---

## Auto-cast, and the toggle that is an input

Auto is not free of the replay contract. If the player can switch a Commander skill's auto on and
off, that switch **changes what the sim computes**, so it is an `InputEvent` like any other —
`SetCommanderAuto { skill, on }`, `InputAuthority::Client`, self-gated and therefore safe to
accept as-is (a forged one only turns the player's own auto on or off).

Defaults: **auto ON for smite, bulwark and mark.** An idle game whose new abilities default to
manual is an idle game where most players never see them.

---

## What this must NOT do

- **Not a faucet.** None of the five produces Gil, drops or XP. `monetary-policy`'s test — "does
  it raise Gil/hour?" — must answer no, or it is an economy change wearing a skill's clothes.
- **Not power sold for money.** These are owned by playing, and Commander level "unlocks, never
  empowers".
- **Not in PvP as-is.** PvP already excludes path bonuses (`pvp-excludes-soul-paths`); a
  Commander skill is the same shape of asymmetry and should be excluded by the same rule until
  someone decides otherwise.
- **Not in story mode.** Story runs on a *snapshot* party and nothing flows back; letting a
  Commander ability in raises "does its cooldown persist" for no gain.

---

## Open questions

1. **Does smite need a floor at low level?** The one place the flat model is uncomfortable is the
   bottom: 50 damage against a Lv10 boss's 120 HP is 42% in one press. A fresh account has no path
   nodes and a weak party, so this is arguably the early-game aid working as intended — but if it
   reads as trivializing, `5 × (lv − 9)` starts it from zero instead. No cap is needed at the TOP;
   the curve falls to 3.4% on its own.
2. **Cooldowns.** 60 / 45 / 30 / 90 s are drafted to feel "occasionally", not "on rotation". These
   are the numbers to argue about, and `sim-cli` can measure the difference before anyone plays.
3. **Is mark's threat rule right?** Highest ATK is the simple read; "lowest HP" (finish it) and
   "the healer" are both defensible and would feel different.
4. **How many are actually new?** Two of the abilities already exist — `click_attack` and
   `regent_hand` — so the list above is **six abilities, four of them new**. If "five" was meant
   as the total, one of smite / bulwark / mark / recall comes out; if it meant five NEW, one more
   goes in. Recall is the weakest of the four and the first thing I would cut.
5. **Does smite need a target cap?** Less pressing now that the sizes are 2×2 and 3×3 — nine cells
   cannot hold many. Probably drop the question unless the 3×3 measures higher than the 2-3 est.
6. **How much does the Commander path arm add in total?** It is now the ONLY upgrade path for
   smite, so this number decides whether investing is worth it. A doubling across the arm
   (~4-5% -> ~9-10% of a boss) is the obvious starting point.
7. **How many Commander path nodes, and on which arm?** A new spine off `origin`, or nodes grafted
   onto the existing Power arm? A new arm is more legible; grafting is cheaper and keeps the web
   from growing a sixth direction.
8. **Do the non-damage four also scale with party level?** Bulwark's 25% of MaxHP and mark's
   duration could. Bulwark arguably should NOT: it is already a % of MaxHP, which scales on its
   own. Cooldowns should definitely not — shortening them turns "occasionally" into "on
   rotation", which is the hands-on drift the design is built to avoid.

---

## Appendix — how a 2×2 AoE would work (investigation, 2026-08-21)

The user expects to want an even-sided blast eventually. It is not expressible today; this is what
it would take, written down now so the answer is not re-derived later.

### Why it is impossible right now

Every AoE in the sim is `pos.chebyshev(center) <= radius` — a box of side `2r+1` centred on a
CELL. Every shape that produces is odd-sided. There is no radius that yields 2×2, because a 2×2 has
no centre cell.

### The fix is half-cell coordinates, not a new shape enum

Work in DOUBLED integers, where a cell `(x, y)` sits at `(2x, 2y)` and the gap between two cells is
an addressable point. Then one predicate covers both parities:

    |2·pos.x − c2.x| ≤ e2   and   |2·pos.y − c2.y| ≤ e2

    odd side n    →   c2 = 2·center,              e2 = n − 1     (3×3: c2 = 2c,       e2 = 2)
    even side n   →   c2 = 2·center + 1,          e2 = n − 1     (2×2: c2 = 2c + 1,   e2 = 1)

Integer-only, no floats, no new geometry — the same comparison with everything scaled by two. Every
existing skill maps onto it unchanged, which is what makes this safe to add.

### Which 2×2 — SETTLED: the cursor cell is the TOP-LEFT

**(user, 2026-08-21: "จุดที่เม้าชี้ คือมุมซ้ายบนของ targeting".)** The block grows down and right
from the cell under the pointer, so the offset is always `(+1, +1)`:

    c2 = 2·center + (1, 1)     e2 = 1        // 2×2, anchored at the cursor cell

Verified against the renderer rather than assumed: `_cell_to_world` maps cell `+y` to world `+Z`,
and with the arena camera at `CAM_PITCH −42` looking down the `+Z` axis, screen-down IS `+y`. So
"top-left" means the smallest `x` and `y` of the four cells, and `(+1, +1)` is the correct offset
in both renderers.

This replaces an earlier proposal to aim the block *toward the caster*. The cursor anchor is
better for a reason that outranks flavour: **the player can see where it will land before they
commit.** A rule derived from the caster's position is invisible at the moment of aiming, so the
same click means different cells depending on where the party happens to be standing. Every
selection rectangle in every tool works the cursor way, and players already know it.

### Parity decides the anchor (user, 2026-08-21)

    e2 = n − 1
    c2 = 2·center            if n is ODD    — cursor is the centre, expands as it always has
    c2 = 2·center + (1, 1)   if n is EVEN   — cursor is the up-left cell of the centre pair

    2×2   c2 = 2c + (1,1)   e2 = 1    cells c   … c+1    cursor = top-left
    3×3   c2 = 2c           e2 = 2    cells c−1 … c+1    cursor = centre
    4×4   c2 = 2c + (1,1)   e2 = 3    cells c−1 … c+2    cursor = centre, biased up-left
    5×5   c2 = 2c           e2 = 4    cells c−2 … c+2    cursor = centre

**The user's two statements turned out to be one rule.** "2×2 anchors at the cursor's top-left"
and "4×4 centres on the cursor, biased up-left" look like different requirements, and a first
draft encoded the first as a corner offset of `(n−1)` — which gives the right answer for 2×2 and
the WRONG one for 4×4 (`c … c+3`, cursor at the corner, not near the middle).

Written as "centre it, and when there is no centre cell take the up-left one", both fall out of a
CONSTANT `+1`, independent of size. That is also exactly the general form the appendix reached
before any anchor was chosen — the sign was the only thing left to pin. A rule that collapses
rather than accumulating cases is usually the one that is actually right.

Every one of the fourteen existing AoE skills centres on its target, so this keeps Commander smite
reading like the rest of the game rather than teaching a private convention — and under this rule
the upgrade from 2×2 to 3×3 barely moves the box at all, because both are already centred on the
pointer as closely as their parity allows. The earlier worry that an upgrade would re-teach the
control mostly evaporates: the box gains a row and a column up-left, and that is all.

The formula needs the parity branch and nothing else — no per-size cases, no lookup table.

⚠ **The renderer's targeting preview and the sim's shape must use this same rule.** They are
separate code in separate languages, and a preview that disagrees with the hit test is worse than
no preview — the player aims at what they were shown and hits something else.

### The real cost is that the shape rule is copy-pasted, and the copies DIFFER

There is already a shared helper, `victims_in_radius` (`combat_step/mod.rs`), and **only two of the
five sites use it**:

| site | what it is | uses helper | excludes the centre cell | radius floor |
|---|---|---|---|---|
| `tick/mod.rs:384` | projectile AoE | no | **no** — the blast hits its own target | — |
| `combat_step/cast.rs:120` | AoE heal splash | no | — | — |
| `combat_step/xp.rs:473` | on-death blast | yes | yes | — |
| `combat_step/mod.rs:437` | debuff splash | no | — | **`.max(1)`** |
| `combat_step/mod.rs:786` | self-AoE cast | yes | yes | — |

Those differences are REAL, not drift: a projectile blast must hit the cell it landed on, while an
on-death blast must not re-hit the corpse. So this is not a merge — extract only the **shape test**
(`in_blast(pos, center, shape) -> bool`) and leave "is the centre included" and "is there a floor"
at the call sites where they are decisions.

Doing it the other way round — folding the policy into one helper because it looks tidier — is how
a projectile silently stops damaging its own target.

### Cost and risk

Small and mechanical, but touching five sites in the combat step means **goldens are the check, not
the code review**. The extraction alone must move NOTHING (`cargo test -p sim-core --tests`, all 39
green with no re-pin); only a skill actually using an even shape may move a hash, and none does
until one is authored.

**REQUIRED BEFORE SMITE CAN SHIP** (user, 2026-08-21: smite is 2×2 at base). This started as a
"may be needed later" investigation and is now on the critical path for the first Commander skill,
which is why the five-site survey below matters more than it looked.

### A cursor anchor makes the skill CURSOR-PLACED

The event becomes `{ x, y }` rather than a target id. `PlaceMember { entity_id, x, y }` already has
that shape, and `InputAuthority::Client` still fits — a forged cell only spends the player's own
cooldown.

**Auto-cast is NOT a complication here** (user, 2026-08-21, correcting an earlier draft of this
section that treated it as one). Every `SkillKind::Active` in the game already auto-selects its
target, and fourteen skills already auto-select one and blast a radius around it. Auto-placing a
2×2 is that same picker with a corner offset instead of a centre — manual and auto differ only in
where the cell comes from, and share the whole resolution. The scan itself is nothing the combat
step is not doing every tick already.

What DOES stay true from that draft is the cross-language part: the renderer's preview and the
sim's hit test are separate code in separate languages and must agree, because a player aims at
what they were shown.
