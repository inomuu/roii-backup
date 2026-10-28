# Commander Level + Outpost — the account-meta progression spine

**Status: DESIGN LOCKED 2026-08-28, NOT built.** The numbers here are the design intent; the
absolute CXP values are **server-tunable** and can be recalibrated after launch without touching
the sim (see § Architecture — none of this is deterministic sim state). Decisions taken from the
user are marked (user).

Related: `docs/commander-skills-design.md` (the skills this UNLOCKS), `TODO.md` § COMMANDER LEVEL
(the older sketch this supersedes), memory `no-offline-version`, `monetary-policy`,
`systems-built-but-unreachable`.

---

## What this is

**Commander level** is an ACCOUNT-level meta progression: a single number that rises over months
and **UNLOCKS features, never empowers stats** (user). The player is the Commander; this is how
"who you are across runs" grows, distinct from any character's level.

**Character level ⊥ Commander level** (user, 2026-08-28). They are unrelated axes: Commander level
does NOT gate the character level cap, and character levels do NOT feed Commander XP. This is a
**deliberate divergence from NIKKE / Blue Archive**, where the account level sets the character
cap — coupling them makes the meta level an empowerment gate, which violates the unlock-not-empower
rule. Character cap is content-cadence-gated on its own (`launch-level-cap-cadence`).

The **Outpost** is the idle faucet that feeds it: a passive Commander-XP generator that accrues
while the client is closed, claimed on return (the NIKKE Outpost pattern — the one gacha analogue
that is genuinely idle-native).

---

## Decisions locked (user, 2026-08-28)

- **Max level 50 at launch, +10 per major update**, no hard cap — mirrors the character-cap cadence
  (70→80→90) so the meta ladder always has unclaimed rungs for veterans.
- **Pure XP** — a level is reached by accumulating CXP alone; NO content-clear gates in the ladder.
  (An idle game can technically out-grind content, but the income is time-throttled — see below — so
  "out-grinding" is bounded by real days regardless.)
- **smite unlocks at Commander Lv 2** — the first Commander skill, ~day 1–2 in. See § Architecture
  for how a server-side level reaches a sim-side unlock.
- **UNLOCKS never empowers** — no stat, no drop rate, nothing the sim computes. The moment a level
  granted power it would have to move into the sim + `AccountState`; keeping it unlock-only is what
  lets it live entirely server-side (§ Architecture).

---

## Income model (user, 2026-08-28)

The unit is **CXP** (Commander XP). Three sources, all time-throttled — no stamina (an idle game is
already throttled by real time), no combat-volume XP (kills scale with party power, so an XP bar fed
by them would just mirror character level).

| source | gives | kind |
|---|---|---|
| **Daily quests** | up to **500 CXP/day** (cap) | clock-gate, resets daily |
| **Outpost** | **500 CXP / 24 h** rate, **72 h cap = 1,500 CXP** max bank | idle accrual (see below) |
| **Story missions** | **100 CXP each × ~50 missions = ~5,000 CXP total** (one-time) | onboarding front-load |

**Steady-state ≈ 1,000 CXP/day** (500 daily + 500 Outpost, once story is spent).

**Outpost cap = 72 h (not 24 h)** (user): a player can be away up to 3 days and lose nothing —
weekend-friendly, softer than a daily-must-claim hook, fitting the cozy/casual identity
(`roi-game-identity`) and mobile plausibility (`mobile-is-a-plausible-platform`). The cap is a
ceiling on ABSENCE, not a rate change: a daily-active player still averages 500/day from it, so it
does not alter pacing.

**One-time source = STORY missions**, not generic milestones. A handbook/milestone feeder
(first-kill / first-clear, off the sim's `Measures`) is an OPTIONAL later addition, not in the
locked model.

**Reconciliation flag:** `TODO.md` § COMMANDER LEVEL said Commander XP "shares the battle-pass meter
(online-time + dailies)". This model gives Commander level its OWN sources (Outpost is new). Whether
the daily-quest CXP is the same meter as battle-pass XP or a parallel one is **not settled here** —
resolve when the battle pass and this are built together.

---

## The curve

Derived to hit the user's pacing: **fast to 50, then a hard step into a year-long post-cap tail.**

```
L1–50:   CXP_to_next(L) = round(700    × 1.03^(L-1))
L51+:    CXP_to_next(L) = round(30,000 × 1.035^(L-51))
```

The **50→51 boundary is a deliberate ~10× step** — the ONE intentional cliff, marking the
launch-tier / endgame-tail seam (not a mid-ladder Genshin-style cliff, which players dislike;
those are avoided by the smooth ×1.03 inside 1–50).

| L→L+1 | CXP to next | ≈ days @1,000/day |
|---|---|---|
| 1→2 | 700 | <1 (smite unlocks ~day 1) |
| 10→11 | ~1,100 | ~1 |
| 25→26 | ~1,420 | ~1.4 |
| 49→50 | ~2,890 | ~2.9 |
| **50→51** | **30,000** | **~30 (STEP)** |
| 55→56 | ~34,400 | ~34 |
| 59→60 | ~40,900 | ~41 |
| **cumulative → L50** | **~75,000** | **~75 days = 2.5 months** |
| **cumulative 51→60** | **~352,000** | **~352 days ≈ 1 year** |

**Pacing check (matches the user's target):**
- **Diligent** (daily + Outpost + story ≈ 1,000/day): **L50 in ~2.5 months**, 51–60 in **~1 year**.
- **Casual** (Outpost only ≈ 500/day): L50 in **~5 months** — a healthy 2.5×→5× diligent/casual spread.
- **Story (~5,000)** carries the first ~5–7 levels in week one, then the 1,000/day drip takes over.

**Headroom for future sources (user, 2026-08-28).** The per-level requirements are set **slightly
higher than the current ~1,000/day income demands** — deliberately budgeting for XP sources not yet
built (events, handbook/first-time milestones, seasonal, etc.). Sizing the curve to CURRENT income
exactly would mean every new source later leveled players too fast and forced a re-inflation of the
whole ladder; a modest buffer now lets future sources slot in and CONVERGE toward the target pacing
instead of overshooting it. Consequence: at launch (only daily + Outpost + story live) the climb runs
a touch slower than the raw table below; it reaches the 2.5-month target as more sources come online.
The representative numbers below are therefore a **floor / starting point**, to be nudged up.

**Implementation:** precompute a table rather than evaluating `pow` at runtime. Commander level is
server-side account meta, so the sim's no-float rule does not strictly apply — but a fixed integer
table is the safer artifact and makes the curve reviewable. The absolute base (700 / 30,000) scales
with whatever final daily-income number the Outpost + daily tuning settles on; the SHAPE and the
days-per-level pacing are the locked part.

---

## Outpost mechanics

**Full real-time accrual** (user), **Commander-XP-only for now** (other rewards deferred):
`accrued = rate × min(now − last_claim_at, CAP)`, rate = 500/24h, CAP = 72h (→ 1,500 max).

**Why "Full accrual" does NOT re-open the offline-earning question.** The offline-earning
contradiction (`docs/craft-and-offline-design.md` vs `docs/backend-arch-reconcile.md`, still unsettled
— `no-offline-version`) is about earning **sim/economy resources** (Gil, character EXP, loot, craft)
while closed, which would require simulating the offline window. **Commander XP is not sim state** —
it lives on the server, outside `GameState` — so accruing it offline is pure server arithmetic on a
timestamp: no offline sim, no client fast-forward, forge-proof (the server owns the clock and the
best-band input from verified saves). The carve-out holds ONLY while the Outpost pays Commander XP
(and, later, bound account items). **The moment it mints Gil / gear / character EXP / tradeable
materials, the offline-earning decision AND the economy-faucet policy (`monetary-policy`: "does it
raise Gil/hour?") re-trigger.** Draw that line in code.

Outpost tiers (raising the idle faucet) are a natural **per-level unlock** — a virtuous accrual loop
like NIKKE's.

---

## Architecture & security — how a server level reaches a sim unlock

Commander level lives **entirely on the server, outside `GameState`**: it grants no stat and changes
nothing the sim computes, so it never enters a replay, never moves a golden, and **cannot be forged
by construction** (it is not an `InputEvent` — the client has no way to author it).

The sim cannot read Commander level. So a Commander skill's ownership is a **per-save reflection** of
the server entitlement:

- `GameState.commander_skills_owned: BTreeSet<String>` (BUILT, sim 0.0.307) — the sim checks this on
  every cast; `view_state` filters the skill bar to the same set. Empty = locked.
- When an account **crosses the unlock level** (smite at Lv 2), the server does NOT auto-grant — it
  marks a **PENDING level reward** (user, 2026-08-28: **accept-to-unlock, not autogrant**). The player
  **ACCEPTS** on the rewards screen, which is a **direct, single-action claim — NOT routed through the
  mailbox** (user: a mailbox hop reads as claiming twice / clunky). A `/commander/claim` request has
  the server validate (levels reached, not already claimed) and, for each newly-claimed level, inject
  the reward payload — the placeholder Gil and, at a skill level, the one-time server-authored
  **`GrantCommanderSkill { skill_id, nonce }`** (`InputAuthority::Server`, keyed blake3 MAC over
  `(secret, account_id, skill_id)`, the `GrantDrop` pattern) — into the save's input log. The sim
  inserts the grant; from then on it reads only the stored set — level is irrelevant to the sim.
- **Exactly-once is a durable server high-water-mark**, `commander_reward_claimed_through: level`,
  reconciled on replay — NOT a mail record. This keeps ClaimMail's exactly-once + can't-forge
  guarantee (the server issues the reward, the client only asks) WITHOUT the mailbox inbox: one Accept
  = one validated claim, no second hop. Unclaimed = the skill stays locked and the reward waits on the
  screen; a player cannot claim a level the server has not recorded as reached.
- **The grant is a PERMANENT latch** keyed to a **PER-ACCOUNT** entitlement (you cannot un-reach a
  level; Commander level is XP-accumulating and unlock-only, so it never drops). Per-account means the
  server re-issues the grant into a new save after a wipe / new party — a server implementation detail
  that does not change the sim's read contract (**B2: re-assert from entitlement each session** is
  preferred, matching how server-roll grants are already injected).

**A player cannot self-unlock online**, three ways over:
1. The gate is in the authoritative sim — a hacked client that draws/casts smite still gets a no-op.
2. The **debug** stand-in (`SetCommanderSkillOwned`, `InputAuthority::Debug`, used offline to test the
   gate) is auto-rejected in any production log (`verify-server main.rs` finds any Debug-authority
   event and rejects the whole bundle). Stripping the event but keeping the state fails too: the
   replay recomputes a different state hash (`commander_skills_owned` is hashed) → mismatch → reject.
3. The real `GrantCommanderSkill` carries a server MAC the client cannot forge, and the compiler
   forbids shipping it unchecked (`every_server_authored_event_has_a_verdict` fails the build).

This is the `make-the-compiler-the-gate` pattern, same as Exemplar ownership and server-roll drops.

---

## Build sequencing (design-first, not built)

1. **Server Commander-level container** — a CXP accumulator + the precomputed level table + a level
   readout. Fed by daily + Outpost + story. Needs NO quest system (Outpost feeds it directly), so it
   is NOT blocked on the quest row `TODO.md` lists as its old upstream.
2. **Outpost** — server accrual (rate/cap/timestamp) + a Godot `views/outpost_view.gd` (presentation,
   reads from `server_client`, a claim button). Pays Commander XP + bound only.
3. **Real unlock** — at Lv 2 the server issues `GrantCommanderSkill { commander_smite }`, replacing
   the debug toggle. Add its verify-server verdict (`grant_commander_valid` MAC recompute + a
   `server_event_is_checked` arm — the compiler will demand it).

Zero sim-core work beyond the already-built owned-set: this whole feature is **server + Godot
presentation**.

---

## NOT decided (next design passes)

- **Reward-per-level table — CONFIRMED it will exist (user, 2026-08-28), the TABLE is deferred.**
  Gacha-style: **every level-up hands a reward, continuously** (the NIKKE/Genshin hook — a standing
  reason to keep climbing, especially through the year-long post-50 tail). Candidates: **crystal**
  (premium currency) on level-up, front-loaded and tapering; roster/party slots; Outpost tiers
  (raising the idle faucet — a virtuous loop); stash tabs; feature toggles (market access, craft-queue
  depth); cosmetic frames/titles. **NEVER a stat line** (unlock-not-empower). ⚠️ Crystal-per-level is a
  FREE-PREMIUM faucet — it must be calibrated against the gacha economy, not sprinkled
  (`monetary-policy`, `docs/gacha-economy-design.md`, `calibrate-monetization-to-actual-scale`): it is
  a deliberate generosity lever whose total across all levels is a real monetization number.
  **Delivery = CLAIM-based, DIRECT** (user, 2026-08-28): every level-up drops a reward into a "rewards
  waiting" screen; the player ACCEPTS to collect in ONE action — a `/commander/claim` that is NOT
  routed through the mailbox (that hop reads as a double claim). Exactly-once via a durable
  `commander_reward_claimed_through` high-water-mark (§ Architecture); accepting a skill level is also
  what unlocks the skill. **Delivery is Crystal = BOUND crystal** (user, 2026-08-28 — crystal, NOT Gil:
  crystal credit the `crystal_wallet` DIRECTLY in the claim tx, satisfying the no-mailbox rule with no sim
  event, and crystal are the real intended reward type; Gil has no direct server-grant path — it enters
  the sim only through ClaimMail, i.e. the mailbox). **The reward is a PER-LEVEL TABLE, held as SHARED
  STATIC CONFIG** (user, 2026-08-30): `data/commander_rewards.json` — one row per level (1..MAX) with
  its own `crystal` + optional `skill` — is loaded ONCE at boot by BOTH the server (into
  `AppState.commander_rewards`, the authoritative CREDIT) and the client (a **byte-identical copy** under
  `godot/data/`, the DISPLAY track). Being static config, it is **never a DB table and never crosses the
  wire per state fetch**: `/commander/state` sends only the account's dynamic position (`level` reached +
  `reward_claimed_through`), and the client renders each level's Crystal/skill from its local copy. The
  client copy under `godot/data/` is a gitignored build artifact (`cp` of the source, exactly like
  `godot/data/game_data.json`); `tools/check_online_sync.sh` byte-compares source↔copy so a forgotten
  copy is caught before an online playtest, and `commander::parse_rewards` rejects at boot any file whose
  level coverage is incomplete or whose `skill` column disagrees with `skill_unlocked_at` (the
  signed-grant authority). The
  claim sums `commander::crystal_between` over the newly-reached levels. Editing rewards = edit the file
  (both copies, `cp`) + **redeploy** (static, not a live DB edit — deliberate: rewards are set once, not
  frequently, and this removes a per-open query + a wire payload). The track covers **levels 1..MAX** —
  **level 1 is a FREE welcome** (user, 2026-08-30), claimable at 0 CXP the moment an account exists, so
  the reward high-water starts at 0 (not 1) and the first claim credits level 1. ⚠️ PLACEHOLDER SHAPE —
  300/level, 1000 at each 10th (milestone): enough to exercise
  "rewards differ per level", NOT calibrated. Crystal-per-level is a FREE-PREMIUM faucet whose TOTAL
  across the ladder is a real monetization number and MUST NOT ship as-is; the flow exists to be
  exercised, and the real per-level table (Crystal + later items/cosmetics) is
  calibrated to the gacha economy before this feature is real.
- **Inventory-independence rule (user, 2026-08-28).** Every Commander reward must be a type a FULL
  stash can neither block nor silently lose — **currencies** (Gil / crystal / soul, which are counters,
  no slot), **account-unlocks** (a skill → `commander_skills_owned`; roster/party slots; Outpost
  tiers; feature toggles), and **cosmetics** (account entitlements). **NEVER a bag item.** The reward
  candidates above are already all of this kind (the skill lands in the owned-set, not the bag; Gil is
  a counter), so the claim can never fail on a full inventory. If a physical stash item ever becomes
  unavoidable, it MUST use overflow-safe delivery (held until there is room, like ClaimMail's
  full-inventory guard), never a silent drop — but the rule is to not go there.
- **Battle-pass meter reconciliation** (see § Income model).
- **Handbook/milestone** as an optional extra one-time source.
- **Absolute CXP calibration** — final numbers once Outpost + daily tuning is measured (`sim-cli`
  does not cover server meta; this is a server/spreadsheet tuning pass).

## OUTPOST REMOVED — 2026-09-30

User: "เพราะว่าไอเทม ใน outpost ถูกตัดออกไปหมดเลยน่ะ เอาออกก็ได้". The Outpost only accrued CXP
(500/24 h, 72 h cap); its item rewards were already cut. Removed: `/commander/outpost/claim`, the
accrual functions, `outpost_preview` / `outpost_cap` on `/commander/state`, and the
`outpost_claimed_at_ms` column. Its 500 CXP/day moved to the daily ladder (200 per rung below).
Everything above that describes the Outpost is history.

## Progress CXP — BUILT 2026-09-30

The curve assumed ~1,000 CXP/day. Two server-side sources pay it,
credited server-side (verify-server `add_commander_cxp`):

- **200 CXP per daily-ladder rung claimed** (`CXP_PER_DAILY_RUNG`), paid by `/daily/claim` with the
  rung's crystal (user 2026-09-30) — five rungs, 1,000 a day (it absorbed the Outpost's 500),
  reachable without finishing the board.
- **50 CXP per stage reached for the first time** (`CXP_PER_NEW_STAGE`), from the sim's
  `band_reached` against the `commander_stage` watermark, on the save that reached it (paid once; an
  older save pays nothing).

Expected: ~2,500 CXP on day one (the stage burst + one day's rungs), Commander 10 around day 3-4,
then ~1,000-1,200/day.
The Growth packs sit on this curve at Commander 10/15/20/25/30 (user 2026-09-30). The tutorial's
CXP is planned with the tutorial. ⚠️ Faster levels also pay the placeholder level-up crystal sooner
(`level_reward_crystal`) — calibrate it before launch.
