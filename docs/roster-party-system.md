# Roster ⇄ Party system + character monetization (DESIGN SPEC — draft)

Status: **DRAFT — pre-alpha prototype, expect heavy change.** Not built; not final.
A structural feature for a later phase. This just captures the 2026-06-29 discussion
so the idea isn't lost — every number / decision here is provisional.

## Core model
- **Roster** — every character you OWN. **Unlimited** count. Unique character id
  (NOT class-keyed) so **duplicate classes are allowed** (own 3 Swordsmen).
- **Party** — the ACTIVE deployed team. **Cap 5.** You pick ≤5 roster characters to
  field; the rest sit on the bench.
- **Bench characters gain NO XP.** Only the active party levels. Swapping a fresh
  character in means re-leveling it → party composition is a real, weighty choice
  (RO-style), not a free swap.

## Buying characters (two paths, neither caps the COUNT)
Each new character = a "slot" you buy. You may keep buying forever; only the price
climbs.
- **Gil (F2P path):** progressive — escalates hard, late slots **10M+ Gil**. This is
  the deep end-game Gil SINK that keeps Gil valuable for the whole game.
- **Real money (shortcut):** **$0.99 → $2.99 → $4.99 → $9.99, then $9.99 per slot**
  forever. Per-slot price caps at $9.99 but the COUNT is uncapped → total spend is
  uncapped (collector/whale-friendly; mildly predatory by intent).
- At purchase you **choose the class** of the new character.
- Draft cost table (tune in sim-cli once built):

  | slot # | Gil (progressive) | Real money |
  |--------|-------------------|-----------|
  | 1 (2nd char) | 1,000 | $0.99 |
  | 2 | 10,000 | $2.99 |
  | 3 | 50,000 | $4.99 |
  | 4 | 200,000 | $9.99 |
  | 5 | 1,000,000 | $9.99 |
  | 6 | 5,000,000 | $9.99 |
  | 7+ | 10,000,000++ | $9.99 |

## Premium — 2 concurrent parties (FUTURE)
- Premium unlocks running **2 parties at once, in separate sessions** (parallel
  progression). Since bench earns no XP, this doubles how fast you develop the
  roster: free = level 5 at a time, premium = 10 at a time.
- **Sells SPEED of roster development, not instant power** → soft / indirect P2W,
  consistent with the F2P business model.
- Architecture: 2 parties = 2 independent GameState sessions/instances — fits the
  deterministic-replay model. Build during the online phase. Premium pricing tiers
  TBD (the $0.99–$9.99 ladder may apply here too / or a separate SKU).

## Why this is regulation-safe (still)
Selling transparently-priced character slots / premium ≠ loot boxes or gambling.
Aggressive on revenue (uncapped slot count) WITHOUT touching the gambling line that
worries Thai regulators / depa. Bench-no-XP keeps it soft-power (you pay for breadth
+ parallel leveling, never a maxed character).

## Codebase impact (the big refactor)
- `GameState.class_characters` is keyed by CLASS id (one char per class, park/restore
  on class-switch). → Replace with a **roster: Vec/Map of CharacterData by UNIQUE id**
  to allow duplicate classes. (Serialize → VERSION bump.)
- `data::recruit_member` (adds straight into the party) → split into:
  - `buy_character(class, pay_with)` → adds a CharacterData to the roster (Gil or RM).
  - `deploy(char_id)` / `bench(char_id)` → move between roster and the ≤5 party.
- Party cap stays 5 (no change). Bench characters: store full CharacterData, frozen
  (no tick, no XP) while benched.
- **Save migration:** old saves use class-keyed `class_characters` → migrate each into
  a roster entry on load.

## UI flow (ties into the "easier UI" goal)
- **Roster screen** — grid of owned characters; buy (Gil / real money) with the
  class picker + the progressive price shown; see level/class per character.
- **Party screen** — pick ≤5 to deploy; drag bench ↔ party; benched = greyed,
  "no XP while benched" hint.

## Open / to-confirm
- Exact Gil + real-money cost curves (sim-cli tune once built).
- Whether the level-gate is dropped entirely (price/Gil paces it) — leaning YES.
- Premium SKU vs the per-slot ladder.
