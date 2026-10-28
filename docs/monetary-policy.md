# Monetary policy — keeping Gil inflation low in an idle economy

DESIGN / PRINCIPLES (guides balance, not a build spec). An idle game mints Gil continuously
(every kill drops `gil_reward`, 24/7, auto-farmed), so inflation is STRUCTURAL — it must be
actively managed, not assumed away. This doc is the framework the whole economy is tuned
against. Companions: `economy-invariants.md`, `s3-economy-design.md`, `refine-ore-economy` +
`endgame-economy-vision` memories, `crowd-scaled-rare-drops.md` (the rare-supply cousin).

## The core equation

```
long-run inflation ≈ (Gil minted per hour) − (Gil burned per hour)
```

The idle faucet never stops and scales with AGGREGATE PLAYTIME (× active accounts). So the
master rule:

> **Sinks must scale with the same drivers as the faucet** (playtime, wealth, progression).
> Flat or one-time sinks always lose to a perpetual faucet.

## What we already have going for us (credit the design)

**No offline earnings + anti-fast-forward** (10 t/s real time, `competition-integrity.md`) caps
each account's faucet to WALL-CLOCK time — you can't mint Gil faster than real time. This closes
the worst inflation vector (time-compression). Inflation then scales with `accounts × hours`,
not exploitable time. We start from a much better place than most idle games; the job is to hold
the ratio, not to fight a runaway exploit.

## What counts as a REAL sink — the zero-Gil-ROI test

A sink removes Gil PERMANENTLY (Gil → NPC/void, never to another player). The one-line test for
any Gil-spending feature:

> **"Does this raise the player's Gil/hour?"** — if YES, it is a faucet amplifier, not a sink.

Real sinks buy things that do NOT mint Gil: refine, cosmetics, progression unlocks, rare-loot
gambles, respecs. Everything else is suspect.

## Trap 1 — a "farm faster" buff is NOT a sink

A buff that costs Gil but speeds up Gil farming is a faucet in disguise. Math: buff costs `C`,
lets you mint `E` extra Gil while active. Its effect on the money supply is `C − E`. If players
rationally buy it, `E ≥ C` (it pays for itself) → the buff is net **inflationary**.

Example: buff costs 100, but the faster farming mints 500 extra → the money supply grows by 400
each time it's used. Calling it a sink is backwards.

**Fix:** price efficiency buffs in MATERIALS (not Gil), or have them accelerate NON-Gil rewards
(XP, rare-drop chance, materials). Then the Gil (if any) buys loot/progress, not more Gil.

## Trap 2 — crafted consumables are a CONVEYOR, not a sink

Three ways a consumable (potion / arrow / food) can reach a player, and how much Gil each
actually removes:

| Channel | Gil removed |
|---|---|
| Buy from NPC shop | 100% of price (Gil → NPC → gone) — strong DIRECT sink |
| Craft from player-bought mats | only the ~5% **market fee**; the rest MOVES buyer→seller |
| Craft from self-farmed mats | 0% — a TIME sink, Gil-neutral |

So crafted + player-traded consumables barely sink Gil — they **circulate** it (combat-farmer →
crafter → gatherer), building the demand web (`the RS-depth model`) and redistributing Gil DOWN
to newer/gatherer players. The actual removal happens at the END of that conveyor, when crafters
spend their accumulated Gil on the real sinks (refine / cosmetics / fee).

**Consequence for design:** do NOT make consumables your primary Gil sink — those two goals
(max burn vs healthy circulation) pull opposite ways. Let consumables be the ECONOMY ENGINE;
put the real sinks at the end of the chain.

**Don't underestimate fee-on-volume:** an idle party burns potions/ammo 24/7, so consumable
trade VOLUME is enormous. 5% of a huge volume is a large ABSOLUTE sink that auto-scales with
activity. The market fee (`fees_burned`, already 5%) is a bigger stabilizer than it looks.

## Trap 3 — the combat-neutral lever (the structural fix)

Make the primary loop (auto-combat) approximately **Gil-neutral**: the potions/ammo/upkeep the
party auto-consumes cost ≈ the Gil combat mints. Then idling alone doesn't accumulate Gil; Gil
accrues only from efficient/deliberate play and is spent on aspirational goals. This is the RS
model (fighting ≈ pays for its own supplies) and it doubles as the craft-demand engine.

Note this is about consumables being a COST OF FARMING (a sink at the individual level, offset
against minted Gil) — distinct from a farm-speed BUFF (Trap 1), which is a multiplier ON income.
The test again: cost of playing (sink) vs multiplier on income (pump).

## Where the real sinks live

1. **Refine / enchant — EXPONENTIAL Gil cost at the high end.** Currently `1000 × lvl` (linear);
   the endgame (+8..+10) should cost STEEPLY more so the rich must burn to progress. This taxes
   Gil at its source (the players minting the most). Pairs with refine downgrade
   (`refine-ore-economy`), which destroys value on failure.
2. **Cosmetics / prestige** — pure sinks, zero Gil ROI, scale with wealth (`character-skins-idea`).
3. **Market fee burn** — auto-stabilizer, scales with trade volume (have it, 5%).
4. **Progression** — respecs, tier/advancement unlocks (one-time but large, gate wealth).
5. **NPC shop as a high-priced FALLBACK, not a channel.** Price NPC consumables ABOVE the normal
   player-crafted market price → it acts as (a) an anti-gouging price CEILING and (b) a 100% sink
   that auto-kicks-in exactly when the player market is undersupplied / Gil is abundant. An
   elegant auto-stabilizer that self-activates when inflation is worst.

## Durability / repair — decided: selective auto-upkeep only

Universal durability (gear that DEGRADES and BREAKS) is annoying and ANTI-IDLE (a player returns
after hours to find gear broke early and farmed nothing). Do NOT ship it. If a repair sink is
wanted, only the good version: **selective, auto, non-breaking upkeep on END-GAME/luxury gear**
(RS Barrows/augment model) — a progressive top-tier sink that never interrupts play, auto-pays,
and at worst gently drops efficiency when unpaid. Prefer **consumption sinks first** (potions/
ammo/food are burned "naturally" and resented far less than gear you own decaying).

## Target: mild, predictable inflation — NOT zero

Aim for LOW and STEADY inflation, not 0. Zero/negative → hoarding (deflation), and it lets bot/
whale stockpiles hold value forever. Mild inflation keeps Gil circulating and gently erodes idle
stockpiles so new players catch up. The goal is preventing RUNAWAY, not stopping the printer.

## Instrumentation — monetary policy needs data

Track, via sim-cli (headless) + server telemetry: **aggregate Gil supply, faucet rate (Gil
minted/hr), sink throughput (per sink type).** `CommodityLedger::total_gil()` already exists as a
probe. When the faucet/sink ratio drifts, add sinks (new refine tiers, new luxury, new content
costs) — inflation control is ACTIVE management across updates, not a one-time constant.

## Design checklist — for ANY new Gil-touching feature

1. Does it raise the player's Gil/hour? → it's a FAUCET; treat it as one (Trap 1).
2. Does the Gil go to the void (NPC) or to another player? → only void is a sink; player-to-
   player only sinks the fee (Trap 2).
3. Does the cost scale with wealth/progression, or is it flat? → flat loses long-term.
4. Is it a cost of playing (sink) or a multiplier on income (pump)? (Trap 3)
5. Does it interrupt the idle loop? → if yes, reconsider (durability lesson).

## Status / open decisions

PRINCIPLES doc, not a build. Concrete follow-ups: (1) steepen high-end refine Gil cost from
linear → exponential; (2) build consumption sinks (potion auto-drink, ammo) as the combat-
neutral lever + craft engine; (3) price an NPC consumable fallback ABOVE player-market as the
ceiling/auto-stabilizer; (4) instrument faucet/sink telemetry in sim-cli. No sim-core schema
change implied by the framework itself.
