# Server-roll drop table

Reference table of every drop that resolves through the two-stage decoy pipeline
(docs/server-roll-drops-design.md). Snapshot of **game_data 0.0.41** (sim 0.0.207,
2026-07-16) — regenerate from `data/game_data.json` (`server_bp` rows) when rates change;
this file records the *intent* behind the numbers, the data file is the runtime truth.

## How a rate reads

`final = candidate% × server 0.1%` — the client rolls the (worthless, predictable)
candidate at `server_bp`; the server resolves each candidate at a keyed-nonce 0.1%
(`DROP_HIT_DEN` 1000) inside save-verify. So `candidate bp = 10^7 / final_den`.
Delivery is ONLINE: pending candidates expire after 8 h (cap 500), so offline farming
still pays out if the player reconnects within the TTL.

## Policy (settled 2026-07-16)

- **Everything tradeable under 1/100 (1%) rolls on the server**: all cards + all
  sub-1% materials (star stones, rune stones, ore_valkirion).
- **featherfern stays client** — exactly 1% by spec (craft material, not marketable).
- **Sub-1% GEAR stays client until stage 3** (full-item server roll): moving gear on
  the template pipeline would leave substats rolling client-side at grant-apply,
  which is timing-steerable. Stage 3 lands before the gear market opens.
- Final rates were re-rounded to clean fractions when they moved (user-confirmed);
  the old weight-derived rates are noted in the 0.0.207 changelog entry in
  `crates/sim-core/src/lib.rs`.

## Rates

| Item | Kind | Final rate | Client candidate | Sources |
|---|---|---|---|---|
| Burst Chom Card (`card_burst_chom`) | Card | 1/200,000 | 0.5% | burst_chom |
| Chomb Card (`card_chomb`) | Card | 1/200,000 | 0.5% | chomber, chomboom |
| Chomb Card (`card_chomb`) | Card | 1/100,000 | 1% | chomb |
| Greenling Card (`card_greenling`) | Card | 1/100,000 | 1% | baby_greenling |
| Pinkling Card (`card_pinkling`) | Card | 1/100,000 | 1% | baby_pinkling, teenager_pinkling, pinkling |
| Rechum Card (`card_rechum`) | Card | 1/100,000 | 1% | rechum |
| Toxic Chom Card (`card_toxic_chom`) | Card | 1/200,000 | 0.5% | toxic_chom |
| Yellowling Card (`card_yellowling`) | Card | 1/100,000 | 1% | yellowling |
| Valkirion (`ore_valkirion`) | Material | 1/4,000 | 25% | 42 common monsters (bands 1+) |
| Valkirion (`ore_valkirion`) | Material | 1/2,000 | 50% | burst_chom, rechum |
| Rune Stones ×5 (`rune_stone_*`) | Material | 1/20,000 | 5% | 19 elite/boss monsters (big_pinkling, chomboom, king_yellowling, boss_b5+, …) |
| Rune Stones ×5 (`rune_stone_*`) | Material | 1/50,000 | 2% | teenager_pinkling (low-tier taste row) |
| Red Star Stone (`star_stone_red`) | Material | 1/50,000 | 2% | 20 elite/boss monsters |
| Blue Star Stone (`star_stone_blue`) | Material | 1/20,000 | 5% | 43 monsters |
| Yellow Star Stone (`star_stone_yellow`) | Material | 1/10,000 | 10% | 20 elite/boss monsters |
| Chaos Star Stone (`star_stone_chaos`) | Material | 1/10,000 | 10% | 43 monsters |
| Green Star Stone (`star_stone_green`) | Material | 1/10,000 | 10% | 43 monsters |
| Shiny Star Stone (`star_stone_shiny`) | Material | 1/5,000 | 20% | 20 elite/boss monsters |
| Purple Star Stone (`star_stone_purple`) | Material | 1/4,000 | 25% | 20 elite/boss monsters |
| Death Star Stone (`star_stone_death`) | Material | 1/2,500 | 40% | 20 elite/boss monsters |

Chom cards keep their launch spec (1/200,000); the `card_chomb` 1/100,000 row on the
`chomb` monster itself is the legacy band-1 rate carried over — both feed the same
bestiary/card entry.

## Expected yield at measured pace

At the measured farm pace (~1,285 kills/hour): one 1/100,000 card ≈ every 3.2 days of
full-time play; a 1/2,500 death stone ≈ every 2 hours. Tune by editing `server_bp`
(client candidate) only — the server rate is global and should stay at 0.1%
(`DROP_HIT_DEN` is a LOCAL TEST override, never set in production).
