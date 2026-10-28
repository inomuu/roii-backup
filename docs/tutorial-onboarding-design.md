# Tutorial + onboarding — design (2026-09-30)

Decisions from the 2026-09-29/30 design thread. BUILT pieces are marked; the rest is the plan.

## BUILT server-side (2026-10-01)

`crates/verify-server/src/tutorial.rs` + `GET /tutorial/state`, `POST /tutorial/claim {id}`. 20
quests in 4 chapters; a chapter opens once the one before is fully claimed; each quest is checked
against the last verified save (new sim counters: items / Regalia equipped, Regalia enhanced /
salvaged, Potential ranks) and server tables (Starter pulls, daily claims, Commander claims), and
pays once: Standard tickets (20 in all) and CXP (~2,850) directly, Gil and items by bound mail,
quest 1 a Rare-grade tier-1 weapon of the lead's family. Chapter 1 is the forced tutorial's steps;
the client drives those screens.

## Shape: a short forced tutorial, then quests

- **Forced tutorial (~first 40 min)** — arrows + locked UI, only for steps without which the game
  stalls or the first session reads wrong. Client-side (Codex).
- **Quests** — chapters of self-paced tasks with rewards, each checked server-side against the
  verified save (the daily-commission machinery). Every quest has a "Go" button and a one-time
  `DialogueBox` hint.

## Forced tutorial

1. Watch the auto-fight; defeat room 1-1's boss.
2. Quest 1 pays a **Rare-grade s1 weapon of the lead's family** (bound, mail, once) → equip it.
3. Stage 1 cleared → party slot 2 opens → **Lumi** arrives by mail (BUILT: sim opens the slot at
   stage 2, server mails Lumi). The client claims the mail automatically, shows "Lumi joined", and
   walks the player to the Party screen to deploy her themselves (she arrives benched).
4. Stage 6 → slot 3 opens with 10 Standard tickets (BUILT) → **first Starter-banner pull (x10)**.
5. Deploy the new character into slot 3.
6. **Equip a Memory Fragment (Regalia)** on a character — straight after the first pull (user
   2026-09-30). A first x10 almost always holds a 3★ (R is 87% a pull).

## Quest chapters (self-paced)

Rewards across all chapters: **20 Standard tickets** (with slots 3-5's 30 = the Starter banner's 50
pulls; a Starter x10 costs 10 since 2026-09-30) and **~3,000 Commander CXP** (brings Growth tier 1
at Commander 10 to about day 2-3). Per-quest split to be set with the list.

- **Ch.2 Build the team (5-40 min):** equip dropped gear / Auto-equip; fuse spare gear (Forge);
  claim the first daily-commission rung (crystal + CXP); unlock a Paths of Soul node (user
  2026-10-01 — Soul reaches the first node's 500 at ~20-25 min, so nothing is handed out).
  Also (2026-10-02): **open Companion mode** (client-reported claim, token reward: EXP Elixir ×1 +
  100 CXP). Ch.4 (2026-10-02): the **disenchant** quest also pays one T3 material per Crest
  category, then **raise a Crest once**; the Crest page names each material's source on hover.
  Also (2026-10-01): **load a provision** (the "equip gear" quest pays EXP Elixir ×3 to load) and
  **clear the first Story mission**.
- **Taught separately, not by a forced step:** Story, Companion mode, Provision, Crest. Everything
  else is taught by red dots (level-up points, new items, bag full, …).
- **Ch.3 Memory Fragments (after the first pull):** salvage a spare 3★ into Memory Dust; enhance a
  Memory Fragment with Dust (Dust drops are scarce early — 2-4 in the first hour — so salvage is
  the early source).
- **Ch.4 Long-term systems (1-4 h):** socket a gem; Training one level; one Potential node; claim a
  Commander level; disenchant one item.
- **Stage select** — offered on the first wipe (retreat to farm), not at a fixed step.
- **Ch.5 Full party:** slots 4/5 at stage 18/24 (BUILT) → finish the Starter 50 → Standard opens.

## What the first hour actually drops (sim-cli, Suzu + this flow, 6 seeds)

- Gear 16-30 pieces (mostly 16-20), nearly all Common, 1-2 Uncommon; some are **s2 (Lv10)** while
  the party is Lv5-6 — unwearable early.
- Memory Dust 2-4 (1k tier); gems 0-3 (half the seeds none); **no Sigils**, no consumables.
- ⇒ "socket a gem" and "Potential node" cannot rely on drops (user 2026-09-30: hand them out on
  reaching a stage, then teach):
  - **Reach stage 4** (~15 min) → 3 gems → quest "socket a gem" (the spares cover the daily
    gem-fusion commission).
  - **Reach stage 10** (~1 h) → Sigil I ×6 of the lead's element + 10,000 Gil → quest "unlock the
    first Potential node". The first node costs Sigil I ×6 + 10,000 Gil since the early-Gil cut; by
    hour 6 the sim holds only 2-3 of the lead's Sigil I, so this quest is what lets a new player
    try Potential on day one.

## Tuning that depends on this flow

- Stage HP opens at 2,500 (smooth to stage 24): Suzu clears stage 1 alone by ~6 min without the
  quest weapon; 15 min ≈ stage 4; 2-3 wipes in the first hour (1-2 with the weapon). Re-run
  `sim-cli --lead suzu --gacha-flow N` (± `--no-quest-weapon`) after any change to the opening.

## EXP Elixir (2026-10-01) — the Provision rack's first item

User: Provision moves to an EXP item. **EXP Elixir** (`exp_elixir`): +20% kill EXP for 1 h, a
Party provision (the rack's no-duplicate rule keeps one running at a time; extra copies in the
slot refresh it when it lapses). The party's highest holder counts, never a sum. Bought with
crystal: **120 each, 10 per account per UTC day** (`GET /shop`, `POST /shop/buy`), mailed bound.
Other sources (quest ×3 aside) are not decided.
