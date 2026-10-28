# Farm Banner — a limited character you can grind for

**Status: BUILT** through the ticket flow (sim 0.0.479, server `farm.rs` + `/farm/roll`). The live
shape is **"Built — box → ticket → banner" at the bottom**, which supersedes the drafts above it.

Captured 2026-09-24 from the user. The first one features **Zolfia** (SSR Ice Destroyer, sim
0.0.476, in no gacha pool). Separate from the paid Limited banner.

## Why

The CCU flywheel (memory `idle-ccu-flywheel`): a tradable drop that comes from being ONLINE gives
players — and their extra accounts — a reason to keep the game open, which lifts concurrent
players on Steam's Most Played and feeds discovery. The Banana model, on purpose. Accounts idling
for boxes are a feature of this plan, not an abuse of it.

## Decided (user)

| | |
|---|---|
| What drops | a **box** — marketable on Steam |
| Where from | **online time**, not kills. The game fights whenever it is open, so online time is play time; there is nothing to gain from farming a low stage |
| Rate | about **0.5 box per player per day** |
| Opening it | crystal, materials, other items — and **Zolfia 1%** |
| Pity | **150 boxes** guarantee Zolfia |
| Window | about **2–3 months** |

## Constraints this design must keep

- **Every opened reward is BOUND.** The box is the only marketable thing. A paid, random box whose
  contents cash out is the line `steam-market-requirements.md` forbids (NL/BE gambling); bound
  contents keep the box on the right side of it.
- **Server-rolled.** A marketable drop is minted by the server (the rare-drop rule) — the client
  sim only reports the online time the server already verifies by replay.
- A per-account **daily cap** (proposed: 1 box/day) so a 24-hour idler does not out-earn a normal
  player many times over while still rewarding being online.

## Open

1. **The 99%**: exact crystal / material / item table and its EV.
2. **Pity**: per account; do boxes bought from the market count (proposed: yes — it counts
   opens); reset on a Zolfia; does it carry to the next Farm Banner or end with this one.
3. **Duplicates**: Resonance, as a gacha copy (proposed).
4. **After the window**: can unopened boxes still be opened / sold; does Zolfia return.
5. **Market timing**: the Steam market is the "second launch" (memory `mass-100k-strategy`). A
   Farm Banner before the market opens has boxes that cannot be sold yet.
6. **Odds disclosure**: a box that can be bought for real money must publish its odds in some
   jurisdictions (KR / CN) — unlike the Sealed Cache, whose table is internal.
7. **Steam policy check before launch**: confirm Valve's current rules on idle-dropped marketable
   items (post-Banana). If idle drops are restricted, the CCU plan fails at its root.
8. **Expected market price**: a farm-everywhere item floors around $0.03 at scale (memory
   `steam-market-what-sells`) — acceptable, since the goal is CCU, not player income.

---

## System design — dropping a box from online time (DRAFT 2026-09-24)

### What "online" means: verified sim ticks

The server already knows how long a player has played, and cannot be lied to about it:

- Every `POST /save` (`put_save`) replays the upload; its `final_tick` is verified.
- `within_tick_rate` rejects a save whose ticks outrun wall-clock (`elapsed_ms/100 + 300`) — a
  fast-forwarded client is flagged, so ticks can never exceed real time online.
- The client freezes its sim after `SPAN_STALL_TICKS` without a save — closing the game or losing
  the network stops the clock. Offline earns nothing, by construction.
- One live session per account (session pinning) — one account cannot farm on two machines.

So **online time = Δ`final_tick` between accepted saves**. The game fights whenever it is open, so
this is also play time. No new client code, no new trust.

### Accrual — a hook in `accrue_after_save`

`accrue_after_save` already runs after every accepted save and hosts the Sealed Cache and the BP
anchor. Farm Banner adds one block there:

1. `delta = final_tick − last_tick` (clamped ≥ 0; a new game resets the anchor), added to the
   account's `credited_ticks` for the active event.
2. Every `ROLL_TICKS` of credited play (proposed: **36,000 = 1 hour**) is one roll, drawn with
   `keyed_nonce("farm-box:v1", secret, account, roll_seq)` — the same server-nonce pattern every
   rare drop uses; the client cannot predict or reroll it.
3. A roll hits at `hit_ppm` (proposed **4%**). The average is set by how long people keep the game
   open: 24 h open ≈ 0.96 box/day, 12 h ≈ 0.48, 4 h ≈ 0.16 — the "~0.5 per player per day" target
   lands on a ~12-hour idler, and longer online always earns more (what the CCU plan needs).
4. **Daily cap** per account (proposed 2, UTC day) — a guard against a pathological streak, not the
   pacing lever; the rate is.
5. Only inside the event window (`starts_at` / `ends_at` in server config).

Table (new): `farm_progress(account_id, event_id, credited_ticks, last_tick, rolls_done,
day_key, boxes_today, pity, won)`.

### The box

- **Rolled and minted by the server**, delivered through the mailbox (`mailbox_store::send_item`,
  `MailKind::Reward`) as a TRADEABLE item — unlike the Sealed Cache, which is bound.
- Opening is a server roll too (the `lootbox_opens` shape: a watermark + a `farm-open:v1` nonce),
  paying BOUND crystal / materials / items via the existing wallets and ledgers — and **Zolfia 1%**.
- **Pity** lives in `farm_progress.pity`: +1 per open, Zolfia at 150, then reset. A box bought on
  the market counts — pity follows the opener, not the farmer.
- Zolfia arrives through the existing collectible path (`bank_collectible`); a duplicate is a
  Resonance copy like a gacha pull.

### The dependency that decides the schedule

**The box is only sellable once Steam Inventory minting exists — and none of it is built.** The
plan (`steam-mint-quota-design.md`) is: in-game item → player PROMOTE → server checks provenance
and quota → `IInventoryService` mint (idempotent) → the in-game copy is burned. Until then the box
is an in-game item with no market (there is no Gil market). Two ways to sequence it:

- **A. Build accrual + box + opening now; the market comes with the Steam Inventory work.** A first
  Farm Banner before the market is a plain grind event (boxes open, cannot be sold) — it still
  measures the CCU effect of online-time drops.
- **B. Hold the Farm Banner until minting ships**, so the first event is the full experiment.

**DECIDED (user 2026-09-24): B** — "เป็นหัวใจของเกม". Steam Inventory minting is built first; the
Farm Banner launches with a sellable box.

### Build order (server, `verify-server`)

1. `farm_progress` table + the accrual block in `accrue_after_save` + the roll (unit-testable pure
   core, like `lootbox.rs`).
2. Box delivery via mail; `farm_open` handler: roll, bound payouts, pity, Zolfia grant.
3. Event config (window, rates, table) as server DATA — memory `mobile-is-a-plausible-platform`:
   SKUs and claimables are server-side data.
4. Steam Inventory mint for the box (the separate, larger project).

Sim side: nothing, apart from the box item def in the catalog.

---

## Built — box → ticket → banner (sim 0.0.479, 2026-09-25)

User 2026-09-25: an opened box becomes a **ticket**, pulled on the gacha screen; the box trades, the
ticket does not. Answers: ×1 and ×10 like the store banners · pity per account, apart from the
store banners · tickets never expire, and a box stays sellable and openable after its event
ends; the banner is **hidden while the account holds no ticket** · boxes **drop only in the event
window**.

| Piece | Where |
|---|---|
| Box | a catalog item with `farm_box: true`, **one def per event** (Steam identity is per def); `farm_box` = event 1, itemdef 1001 |
| Open | `InputEvent::OpenFarmBox { inv_idx }` — the sim removes one and counts `GameState.farm_opened[def]` |
| Ticket | server: each open past `farm_box_opens(account, box_def).paid` credits bound `farm_ticket:<def>` in `bound_items` (+ `bound_ledger`, reason `farm_box`) |
| Pull | `POST /farm/roll {box_def, count: 1\|10, idem_key}` — spends tickets, rolls `farm::pull` (nonce domain `farm-pull:v1:<def>`, seq = the account's pull count), pity in `farm_pity(account, box_def)`, mails the payouts; `idem_key` in `gacha_rolls` (banner `farm:<def>`) |
| Screen | `/gacha/state` → `farm: [{event_id, box_def, featured, tickets, since, pity, featured_ppm, farming, ends_ms}]`, only entries with tickets > 0 |
| Events | `FARM_EVENT` env: one object or an array; ended events stay listed (their tickets pull), a `box_def` belongs to one event |

Payout table unchanged: the featured character at `featured_ppm` (pity `pity`), else 50% Gil 25,000 /
35% Sigil I ×3 / 12% Sigil II / 3% Sigil III, element equal odds, Sigils bound.

**0.0.477 carry-over.** The test event paid on open. `Measures.farm_boxes_opened` is no longer
written; the server adds it to `farm_opened["farm_box"]` and seeds the new watermark and pity from
`farm_opens` (paid, since) on first sight, so no open is paid twice and the pity count carries.
