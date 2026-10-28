# Exclusive-unit Ticket — design (2026-09-22, user's "experimental business model")

Source of truth for the feature. Nothing here is built yet; every engine piece it needs exists
(server-rolled crowd-scaled drops, the Sealed Cache open path, the gacha pity table, the mailbox
grant). Decisions marked **(user)** are set; **(default)** are my proposals still open.

## 1. What it is (user, 2026-09-21/22)

- A **ticket** that drops in play and is **tradeable on the Steam Market**. It is never sold in
  any shop and never given by any banner.
- Opening one pays **one exclusive character** at a low rate (~1%), otherwise **material / Gil /
  soul** — never nothing, never another character.
- The character exists **only on this path**: no banner, no coupon, no shop, ever. A duplicate
  is a Resonance star, so a whale wants seven copies.
- **Drop pace ~0.3-0.5 tickets per player per day** over an **event of 2-3 months**.
- Revenue for us is only the Steam Market fee. The point is not revenue: it is the one item in
  the game whose whale money lands on free players, so a slow / new player has something worth
  listing from day one (`docs/steam-market-study-tbh-wog.md` §4 — farm-everywhere items floor at
  $0.03; this one cannot, because its demand is a character).

## 2. The numbers at the user's pace

10,000 players × 0.4 / day × 75 days ≈ **300K tickets** in one event.

| who | tickets over the event | what it means |
|---|---|---|
| one player, opens everything | ~30 | 26% chance of ONE copy at 1% — most players never see her by farming alone |
| S0 buyer | ~100 (with pity) | buys ~70 tickets on top of his own 30 |
| S6 whale | ~700 | 7 copies ≈ 2 copies from luck + 5 from pity → the market's deep demand |
| the server | ~3,000 copies at 1% | ≈ 400 S6 whales' worth, if every ticket is opened |

Price anchor: a whale valuing S6 at $300 pays ~$0.43 per ticket; at $150, ~$0.21. A free player's
0.4 / day is then **$0.08-0.17 / day** — the "something back" the market study asked for, without
being a wage.

## 3. Rules that keep it from turning into TBH's bot economy

1. **Supply is a server-wide daily quota, not a per-kill rate.** The ticket is a rare drop under
   the existing crowd-scaled server roll (`docs/economy-invariants.md`, memory
   `economy-antibot-and-quota`: rate ∝ 1 / farmers), sized so the expected total is
   `0.4 × expected real players` per day, with a hard daily cap on top (`lootbox_quota`-style
   row, per event day). Ten thousand bots do not mint ten thousand extra tickets; they dilute
   everyone, botters included, which is the accepted model.
2. **Tradeable until opened, everything inside is bound.** The character, the soul, the Gil, the
   material — all bound on open (`bound_items` / `crystal_wallet.bound` pattern). Otherwise the
   consolation pool is a Gil-laundering route between accounts.
3. **Opening yourself must be worth less than selling.** EV(open) < market price at all times, or
   nobody lists and the whale has nothing to buy. The consolation table is tuned to roughly a
   quarter of the expected ticket price.
4. **Bound on the account that opens; the ticket has no expiry inside the event.** After the
   event the ticket stays tradeable but pays only consolation (the character is gone) — say so
   in the item text so nobody is surprised; better: stop the drop two weeks before the end so the
   market clears.

## 4. Open table (default — tune with the real character's value)

| result | odds | note |
|---|---|---|
| the exclusive character | **1.0%** | Resonance star on a duplicate |
| soul, a mid lump | 9% | ≈ 1 h of farming |
| material T4-T5 (bound, relic/crest grade) | 30% | disenchant fodder |
| Gil ≈ 1 h of farming at the opener's stage | 60% | floor |

**Pity (default): 100 tickets per account guarantees a copy** — the whale can price his S6, so
the ticket has a stable ceiling. Counter is server-side like every banner's
(`gacha_pity`), keyed on the event id.

**Where it drops (default): every stage, boss ×3**, under the crowd-scaled roll — the new
player has a listing on day one, which is the whole reason for the item. Alternative kept in
mind: stage 100+ only (mid-game shelf, memory `mid-level-shelf-is-the-market`).

## 5. Build list (when the character exists)

- catalog: `ticket_<event>` item (`marketable: true`, `emission_group: "event_ticket"`,
  quota_class new), the exclusive `exemplar_<unit>` (Mythical or Legendary, NOT in any
  `*_POOL`), the unit's kit like any other.
- sim: `InputEvent::OpenTicket { inv_idx }` — consume only, count in `Measures` (the Sealed Cache
  shape); the payout is server-side on the next verified save.
- server: `ticket_opens` table + per-account pity + per-day server quota; payout via mailbox
  grant / bound wallet; system-chat line on a character win (the "แคปไปอวด" moment).
- client: an Open action on the bag tile, the "after next save" callout reused from Sealed Cache.

## 6. Not decided

- The character (doctrine / element / rarity) — it should be a role the roster lacks so demand
  is real: today that is **Lightning** (no unit) or a second Support.
- Whether an event can rerun (a rerun with a NEW ticket keeps the old one worthless — say so up
  front, or never rerun).
