# Road to 1st Demo

> **REFINE IS RETIRED (0.0.370, 2026-09-15).** Every row below that mentions refine — the
> refine shop, the refine nonce, refine dailies, ore laundering — describes a system that no
> longer exists. The +N ladder was deleted with the move to the GRADE/tier gear model
> (`docs/gear-fusion-and-base-fix-design.md`); the server-roll protocol it drove lives on as
> the FUSION roll (0.0.377, design §7.8: `/fuse/roll`, `verify_fuse_bindings`, `reconcile_fuse`)
> and `docs/refine-server-roll-2b.md` stays as the reference for it. Rows are left in place
> rather than rewritten: they still record why each decision was made.


> **STATE OF PLAY 2026-09-19 (sim 0.0.444 / catalog 0.0.187; server deployed at 0.0.438 / 0.0.184).**
> Since the 09-18 block: Nyx (5th unit), **Crests** (Relic renamed; T3 = XP/Gil/Soul/Drop economy
> lines), **MATK removed**, **class system retired** (remnants listed in memory), **monsters have no
> element** (weakness lists only), **Resonance S1-S6** as mechanics (`unit_rank.rs`), **SSR Regalia
> role-keyed passives** (doctrine-gated), ult gauge ×10 + no charge from damage taken, monster
> **enrage** (+20% ATK / 15 s after 30 s), stalemate breaker for trash rooms and full-strength
> parties, damage-blocked telemetry, `sim-cli --bench / --dummy / --stars / --regalia`.
> Aveline is a Striker; Rapis is a real Exemplar pull. Kit numbers: see `lib.rs` 0.0.425-0.0.444.
>
> (the 09-18 block below is kept for the rows it explains)
>
> **STATE OF PLAY 2026-09-18 (reconciled against the tree at sim 0.0.419 / catalog 0.0.174).**
> Read this block before any row below — a great deal of the file describes a game that has
> since moved. What is TRUE today:
>
> | area | today |
> |---|---|
> | Progression | **240-stage ladder** (12/region, launch cap 168), no mob level; monster HP/ATK/XP
>   tuned on the REAL party (Seraphine + pulled Exemplars) — every number measured before
>   2026-09-17 came from the old class harness and is void (memory `measure-with-the-real-party`) |
> | Party | **Seraphine alone at genesis**; every other character is a GACHA pull; Gil buys the
>   four extra **party slots** (5k/5k/8k/12k) and each slot pays a 10-pull ticket server-side.
>   `RecruitMember`/`BuyCharacter` are tombstones. Only Seraphine/Rapis have fixed kits — the
>   other Exemplars ride class kits + overlay until their own are authored |
> | Economy | **Gil ×100** at the source (0.0.412). **No vendoring** (0.0.411) and **no Gil market**
>   — income is fusion's payout (100% of fed sell_value); sinks are **Training** (23 ladders,
>   breadth gate) and party slots. **Craft materials left the drop tables** (catalog 0.0.163)
>   until craft is redesigned; recipes are unreachable. `Regalia`/`Exemplar` 10-Gil test prices
>   are still in the catalog |
> | Gear sinks | **Fusion** (six → one, fails by grade) and **Crests/Disenchant** (T3+ gear, Rare+
>   gems → 20 bound materials → 20 crests, 20 stats; 0.0.416-0.0.418) — the player chooses which
>   eats a piece |
> | Gems | 11 sets, weapon/armor/accessory groups, Twilight/Fervor/Storm element pairs; socket
>   count by grade; Steam-tradeable |
> | Break | `(200 + 14 × Σ party level) × (toughness+20)/40`, HSR's shape (0.0.405/0.0.408) |
> | Daily/Weekly | boards are `kills / rooms / damage / fuse_3 / card_feed_1 / train_1 / spend_gil /
>   soul_node_1` (sell_10 and craft_1 retired with their systems) |
> | Story | **DEFERRED** (user 2026-09-17: "story คิดว่าจะเลื่อนไปก่อน") — the prologue sandbox still
>   uses the retired `BuyCharacter`; tier-2 "write the story" below is not current work |
> | Not doing | PvP proper (PvPvE boss race first, memory), mount, competitive events, offline |
>
> **Open threads (2026-09-19), not a plan — the user decides what comes next:** Unity branch
> `codex/ui-crests` follows `docs/ui-handoff-2026-09-19.md`; weakness themes per region are a
> PROPOSAL only (`element-weakness-system.md` §9, user: "ค่อยดูอีกที"); class remnants listed in
> memory `class-system-retired`; local master is ahead of the deployed server (0.0.438).

**Goal:** playable to **Base Lv70** + **one job advancement (2nd-tier class)**,
presentable (art + audio), then release the 1st demo.
**Pace target:** Lv70 in ~8–14 days (curve EXP_BASE=250 → ~8 days now; steepen
later if needed). Advance at ~Lv40.

> 🔴 **P0 DIFFICULTY RE-TUNE (deferred 2026-06-25):** the RO HP-curve port (0.0.69)
> raised player HP to RO scale → players are now FAR too tanky vs the old-scale
> monsters. Re-tune monster ATK/HP + boss_thresholds + band pacing to the new HP
> curve (verify with `cargo run -p sim-cli`). The old Lv29@24h tuning is STALE.
> See memory [difficulty-retune-after-ro-hp].
>
> ✅ **GATE CLEARED 2026-08-03.** The condition was "do this AFTER the 2-1 classes are
> added (avoid tuning twice)" — `game_data.json` now carries 10 `player_classes`
> (5 t1 + 5 t2: elementalist / assassin / swordmaster / bishop / ranger), so the thing
> it was waiting for shipped. Nothing blocks it now; it is the only P0 in this file and
> it has been sitting unblocked. Re-tune against a party that can actually advance.

Priority tags: **P0** = must-have / gameplay-complete · **P0.5** = launch-readiness
systems (M1.5 — what an ONLINE game needs, not what a demo needs) · **P1** = demo
polish/UX · **P2** = presentation · **P3** = release. Do P0 first; P2 (art/audio) can
run in parallel (outsourced) since it doesn't block the others.

---

## 🎯 PRIORITY ORDER (2026-08-13, updated 2026-08-16)

The two tables below are ordered by **"does this block opening"**, which is not the same as
"what should be done next". This list is. The sort key is **what gets more expensive the longer
it waits** — everything here still has to ship.

### 1 — Costs more every day it waits

| | units | why it is first |
|---|---|---|
| ~~**Finish Market → rows (path A)**~~ | ✅ 0 | **DONE 2026-08-16** (`0f83108` → `ff2f2c2`, design in `docs/market-rows-migration.md`). commodity + item market are rows; the `ledger`/`item_ledger` blob keys are deleted on load and `market_blobs()` is gone, so "rows and blob coexisting on a money path" cannot occur. Per-trade cost went from the whole market twice (11.7 MB at 50k orders) to a constant 2–3 rows, pinned by `market_write_cost_per_trade_is_constant`. No data migration was needed — the live market held only test data. |
| **Un-price Regalia / Exemplar from 10 Gil** | ~0.1 | 26 catalog items still carry the 2026-08-03 test price. The shop lists whatever has `buy_price > 0`, so the $8.99/$4.99 goods are on sale for 10 Gil. **Deliberately KEPT for now (user, 2026-08-16) — it is still the test fixture.** It only bites a build that ships, so the place to gate it is `tools/export_windows.sh`, not the catalog. |

**Nothing is left in tier 1.** That is worth saying out loud rather than deleting the heading: the
next thing to work on is in tier 2, which is not code.

**Market hardening is CLOSED (2026-08-18).** Two adversarial audit rounds after the rows migration
found and fixed two live money-loss bugs — `put_save` reading the poison flag outside the lock, and
`custody_is_accountable` being structurally blind to every `full` plan — plus three ways the
lock/poison guard could be bypassed with the whole suite green. verify-server is at 175 tests.
Six items were assessed and DEFERRED ON PURPOSE; none blocks opening, and the reasoning for each
is in `docs/market-rows-migration.md` §16 so the list is not read as debt. Two of them are
unreachable or self-healing rather than unfinished. The one worth picking up later is an
HTTP-level test harness (build the `Router`, drive `put_save` with `oneshot`) — deferred not for
lack of value but because tier 3 is all endpoints, so building it there makes it shared
infrastructure instead of market-only.

### 2 — Not code, and nothing in code can start it

| | why it outranks the code |
|---|---|
| **Write the story** | Stage + dialogue is the largest line in the estimate and its MACHINE IS BUILT. The only thing between it and done is prose nobody has written. It blocks Tutorial after it. Every week unwritten is a week added to the open date that no amount of engineering removes. |
| **Commission the remaining art** | 6 class sheets + VFX, on someone else's queue. |

These two are the whole of the "5–8 weeks of code → 10–14 weeks to open" gap. They can run from
today alongside everything below.

### 3 — Blocks opening, and is code

**RECONCILED AGAINST THE TREE 2026-08-21.** This table had drifted badly — three of its eight rows
were finished and one said "do not start" about a decision the user had already made. That is not
a bookkeeping problem: two sessions share this worktree, and a stale priority list is how the same
row gets built twice or left waiting on a blocker that is gone. Every claim below was checked
against a file, a line, or a commit, and the method is worth repeating before picking anything up.

| | units | note |
|---|---|---|
| ~~**Schedule + claim machine**~~ | ✅ 0 | **BUILT.** `verify-server/src/schedule.rs` is 822 lines — `TrackDef` / `Window` / `Grant` / `owed()` / `reached_mask()` / `retired_tracks()` / `per_account_expiring()` — and `/claim/state` + `/claim/collect` are live in `main.rs`. The hardcoded `vec![]` in `daily.rs` this row was written about is gone. |
| **Quest / objective** | 2–3 | **Still not built** — nothing in either crate matches `quest` on a word boundary (the greps that look like hits are all `request`). The measurement half is real: `Measures` at `sim-core/src/state.rs:702`, already on `GameState`. This is now the top of the list, and it is also what Commander level is waiting on. |
| **Packs** | 1 | **UNBLOCKED.** No `pack_id` anywhere yet, but the schedule machine it was queued behind is done, so the dependency in the old note no longer applies. Cheapest revenue in the file. |
| **Bound items** | ~0.5 | **HALF BUILT, and no longer blocked.** The `bound_items` table exists (`main.rs:410`) with credit (`:3615`), locked spend (`:4454`, `:4500`) and read (`:4794`) paths, `schedule.rs` has the `Grant` variant that credits it, and the gacha coupon spends it today. The five questions this row said to wait on were answered by the user on 2026-08-18 — `docs/bound-items-design.md` now has a "ที่ตัดสินแล้ว" section and files all five under ANSWERED, including the EXP-ticket one that was supposed to overturn a third of the design. What is left is the other ticket kinds and their UI, not the architecture. |
| **Commander level** | ~1 | Account-level progression. One of its two upstream rows (schedule) is now done; the other (quest) is not, so it stays downstream of quest and no earlier. Nothing in either crate matches `commander_level` or `account_level`. |
| ~~**Level-cap / band gate**~~ | ✅ 0 | **DONE 2026-08-18 (`7eef37e`).** Half of it already existed — `try_level_up` has stopped at `LEVEL_CAP` and drained XP into Apex since 0.0.227. The gate did not: `SelectBand` is `InputAuthority::Client` and its only check was `band >= 1`, so a level-1 party could type 20 and stand in the endgame, and replay-verify could not see it because the sim accepted the input. Rule (user): any band up to REACHED + 1. No migration — the gate reads the CURRENT ZONE as a second source of truth, so a save written before the field existed is not locked out of where it is parked. |
| ~~**Telemetry, client half**~~ | ✅ 0 | **DONE 2026-08-21 (`549f099`).** `POST /telemetry` + `net/telemetry.gd`: names batch client-side, the endpoint folds them into the same in-memory accumulator `put_save` uses and writes nothing, so the added DB write rate is zero. A separate endpoint rather than a field on the save, because `SaveBundle` lives in sim-core. Safety lives in `metrics::client` — an allow-list (a client that names its own counters controls the flush size), a server-added `client.` prefix (so a forged name cannot reach `grant.drop`), and unknown names counted rather than dropped. `session.end` is a HEARTBEAT instead: an async push cannot complete at teardown, which `sim_driver.gd` already documents for the save. `tools/check_telemetry_keys.py` guards both directions and is in smoke.sh. **Three names are marked `// UNWIRED`** — `screen.market` / `screen.craft` (plain `Control`s, no `UIWindow` hook) and `store.abandon` (checkout-flow logic). **Needs a deploy** before any of it records. |
| ~~**Money-path audit follow-ups**~~ | ✅ 0 | **ALL FIVE FIXED**, contrary to what this row said. `docs/money-path-audit-2026-08.md` carries a commit for each: F4 refund revocation `1371399` (and `main.rs:3113` documents the reader it added), F2 gem debit `1749ea5`, F3 double-charge guard `1749ea5` + `d53dbac`, F5 and F6 `fbd195c`. Read the audit's own limits section before treating the AUDIT as complete — nothing it examined was refuted, which means the sampling rate bound the result, not the defect rate. |

**What is actually left in tier 3: quest, packs, the rest of bound items, commander level.** verify-server is at 209 tests and sim-core at 0.0.305 — both well past the numbers
the rest of this file quotes, so treat any other count in here as a lower bound until checked.

### 4 — Launch readiness, order is flexible

- **Recruit → gacha only** (P1, planned 2026-09-17, NOT built): `RecruitMember`/`BuyCharacter` become no-ops, the gacha grant path (`grant_exemplar_unit`) is the only way in; sim-cli needs a party-arrival schedule instead of `auto_recruit`. Plan + the five decisions: `docs/recruit-retirement-plan.md`.
- **Training (ฝึกฝน) — Gil sink, WoG-shaped** (P1, BUILT 0.0.406-0.0.410 sim side; Unity window pending): 12 party-wide stat ladders bought with Gil, one cost table, no RNG; folds as `StatMod` per tick next to Apex/gems. Gil has NO sink after recruits and no market — this is the sink. Plan + open questions: `docs/training-system-plan.md`; source data: `docs/wog-training-study.md`.

Tutorial (2–3, now the cheap version — the dialogue machine it was waiting on is built) ·
M2 polish (~4–6.5, 2 of 10 already done) · macOS build + distribution (2) · Content authoring
(1–2) · Dispatch (1–2) · harden the release dylib (0.5–1) · the two save switches (~0.5) ·
operator mail (~0.5).

### Not on this list on purpose

**Online security (6 items)** — the section header says "lock these **rules**"; they are the
architecture, already built, not work. **Competitive events (3 items)** — the header says "NOT
building yet". **P0 difficulty re-tune** — deferred by the user 2026-08-13. **Mount / ride** —
CUT 2026-08-13.

---

## 🛒 STEAM MARKET — PARKED (user 2026-09-26: "เกมยังไม่เป็นรูปร่างขนาดนั้น")

Built: Send to Steam (game → Steam mint, 0.0.478), itemdef 1001 `farm_box`, Farm Banner box →
ticket → `/farm/roll` (0.0.479), boxes never stack (0.0.480), save reply `drops` (server).
Design: `docs/farm-banner-design.md`, `docs/steam-mint-quota-design.md`.

Open before a box can be SOLD on the Community Market (checked on the partner site 2026-09-26):

- **Steam → game redeem** — not built. Without it a box bought on the market cannot be opened in
  the game, so nobody buys one. Server consumes the Steam item, mails the box.
- **Mint status endpoint** — the client cannot yet see whether a sent item reached Steam.
- **Valve admits the app to the Steam Economy** — Economy page: "limited beta… please let us
  know". A support request; unknown whether a Coming Soon app qualifies (ask in it).
- `steam/itemdefs.json` 1001: `tradable` / `marketable` → true, re-upload.
- Economy → Item Visibility: Private → Public.
- Decide Steam Trade Protection (7-day lock, reversible trades).
- `commodity` in itemdefs.json is not a documented schema field; identical boxes want commodity
  listing — "Force all items to commodities" (Economy page) is the known switch, untested.
- The real Zolfia event: window (2–3 months) and `box_ppm` / `daily_cap` against the
  0.5 box / player / day target (defaults give a 24 h idler ~1/day).
- Unity (Codex): Farm Banner page + ticket pulls, `drops` auto-claim, Send to Steam button —
  `docs/unity-farm-banner-handoff.md`, `docs/unity-send-to-steam-handoff.md`.

---

## 🎖️ COMMANDER LEVEL — designed 2026-08-17, NOT built

> **The curve, income model, Outpost, and unlock architecture are now spec'd in
> `docs/commander-level-and-outpost.md` (LOCKED 2026-08-28).** That doc supersedes the sketch below
> on the specifics (max 50 / +10, pure-XP curve, daily 500 + Outpost 500/24h·72h-cap + story 5k,
> smite@Lv2, server-side carve-out that dodges offline-earning). The block below stays for the
> upstream-dependency reasoning; note the doc's finding that Outpost-fed CXP needs NO quest system,
> so this row is NOT blocked on quests the way it was written.

An ACCOUNT-level progression, distinct from character levels and from Paths of Soul.

> **Briefly amended and then RESTORED, 2026-08-21.** A Commander skill's multiplier was going to
> scale with Commander level, which is empowerment; the user moved it to the PARTY'S AVERAGE LEVEL
> instead, so the rule below holds unqualified again and `AccountState` stays unblocked. The
> reasoning is kept in `docs/commander-skills-design.md` because the cheaper form was not obvious.

**It UNLOCKS, it never empowers (user, 2026-08-17).** Features, items, rewards, Commander
skills. It grants no stat, touches no drop rate, and changes nothing the sim computes. That
one rule is what keeps it cheap: it can live entirely on the server, outside `GameState`,
so it never enters a replay, never moves a golden, and **cannot be forged by construction
rather than by checking** — it is not an `InputEvent`, so the client has no way to author
it. The moment it grants power it must move into the sim AND into `AccountState`, which is
the (planned, unbuilt) `GameState → AccountState + Vec<PartySession>` split.

**Why it is not just more Soul.** Paths of Soul is already a party-wide meta tree spent with
`soul` — but it is PER-SAVE and wiped by a new game. The line between them: **Soul is this
run's build; Commander level is who you are across runs.** Without that distinction this is
just more soul nodes.

**XP comes from LOGIN + QUESTS, not from combat (user).** Kills scale with party power, so
an XP bar fed by them is a second display of character level. The battle pass already plans
`XP = online-time + dailies`; this shares that meter rather than adding one.

### It is downstream of two rows already in tier 3 — one of which is now DONE

    Schedule + claim ──┬──> daily login ──┐        ✅ BUILT (schedule.rs, /claim/*)
                       │                  ├──> XP ──> Commander level ──> rewards ──┐
    Quest / objective ─┴──> quests done ──┘                                          │
                       ▲  ❌ still the blocker                                       │
                       └──────────── back through the SAME claim machine ────────────┘

Only the left leg is missing now (reconciled 2026-08-21). That does NOT make Commander level
cheaper to start first: the reward table still routes through quests, so building it ahead of
them means stubbing one and writing the table twice — the same argument as before, with one
of the two stubs removed.

Rewards per level are a FOURTH consumer of the schedule+claim machine, not a new one — the
same argument that row already makes for itself ("doing this properly makes the next row
free"). `Measures` in the sim already counts what the quests would read (kills, rooms
cleared, orbs used, cards fed).

### What is actually left once those exist

- an XP accumulator + level curve, server-side
- one more reward table in the claim machine
- the OWNED-skill list fed into `player_skill_available` as its **second source** — that
  function's own doc comment already reserves this: *"a future Regent skill tree grants
  skills the player OWNS everywhere — that becomes a second source checked right here, so no
  call site has to learn about it."* This is also how `regent_hand` stops being a temporary
  override and becomes real. It must ADD a source, never replace the zone one: Deathknell's
  3 s wind-up is designed around the boss room granting the answer to it.
- items and rewards must arrive through the **mailbox** (`ClaimMail`), not a new path —
  anything entering the inventory is inside the replay, and that route already has the
  full-bag guard and consume-on-verified-save.
- a level readout in the UI

---

## ⏱️ Effort estimate — STRUCTURE only (2026-07-31)

Systems only: no content authoring, no art, no story writing, no balance passes, no QA loop.
Estimates are for building the machine, not for filling it.

**Unit = a session like 2026-07-30's dungeon kit** (design + code + tests + measurement +
commit — four engine hooks, two bug fixes, boss skills, a clear screen, client drag + telegraph).
**Velocity is measured, not assumed:** the 28 days to 2026-07-31 had 24 working days and 64
VERSION bumps (~2.3 shipped slices/day, two sessions in parallel). Bumps are smaller than a
"unit"; observed throughput is roughly **7–9 units/week**.

### Must-have before opening

| system | units | note |
|---|---|---|
| Telemetry (M1.5) | 0.5–1 | server half BUILT 2026-08-03 (metrics.rs + metrics_daily + /dev/balance); left = client-emitted intent events |
| Quest / objective (M1.5) | 2–3 | catalog as data + server state + daily reset + UI; `Measures` already counts most of it |
| Schedule + claim (M1.6) | 2–3 | one machine → daily login + event windows + pack availability |
| ~~Micro-transaction + store (M1.6)~~ | ✅ 0 | DONE — `store_checkout` → Steam `InitTxn`, `store_finalize` → `FinalizeTxn` ("the client saying they approved is not evidence"), sandbox by default, `store.rs:27` VERIFIED 2026-08-02 against the Steam sandbox with a THB wallet. Client: `store_view.gd` + a Gems button in `hud_bar.gd`, wired in `main.tscn` — reachable, not just built. This was the item flagged "the easiest to underestimate"; it is the one that turned out already done. Verified 2026-08-13 |
| Save-write throughput (top of file) | ~0.5 | ✅ the URGENT half is DONE — `MAX_REPLAY_SPAN` is 50_000 in main.rs, not the 500_000 the section below still describes. Left = the two storage switches (gzip, `SAVE_INTERVAL` 10→60 s), both "safe to do late" and the second a product call. Verified 2026-08-13 |
| Level-cap / band gate (M1.5) | 0.5–1 | must exist before shipping the bands it gates |
| ~~gacha: fail-closed secret + server nonce~~ | ✅ 0 | DONE — `main.rs:814` refuses to boot with a DB and no `SERVER_ROLL_SECRET`; `gacha_seed(secret, account_id, idem_key, server_nonce)` draws the nonce INSIDE the claiming transaction, which is what closes the enumerate-an-idem_key hole. Verified 2026-08-13 |
| ~~gacha: rate verification query + test~~ | ✅ 0 | DONE — 5 tests in `gacha.rs` that MEASURE rather than assert a boolean (`character_banner_lands_on_its_locked_average` runs pulls and checks 46–54/featured; `the_rate_is_flat_forever`). Verified 2026-08-13 |
| security: ~~refine nonce~~ + harden dylib | 0.5–1 | refine nonce DONE (C06) — `RefineRollReq` carries `enriched` + `seal` and `refine_nonce` hashes both under `refine-roll:v2`, so peek-and-select is closed. Left = harden the release dylib. serde-skip audit DONE 0.0.261, guarded by a test not a memory. Verified 2026-08-13 |
| **subtotal** | **~6–10** | **was 15–22. A 2026-08-13 code sweep found ~8–12 units already built (see the ✅ rows) — over half the list. Re-verify before planning off any number here.** |

### Should have, can follow

| system | units | note |
|---|---|---|
| Dispatch (M1.6) | 1–2 | 🔴 no trace in server or client. server wall-clock timers + delivery through the existing mailbox |
| Tutorial / onboarding (M1.5) | 2–3 | 🔴 not built — but the dialogue machine it was waiting to be cheaper than IS built, so this is now the cheap version |
| Stage + dialogue (M1.6) | ~1–2 | 🟢 the MACHINE is built — `dialogue_box.gd` 451, `dialogue_script.gd` 273, `story_view.gd` 636, dungeon menu/clear, and 4 headless selftests that pass. **What is left is not code: the story is unwritten.** Verified 2026-08-13 |
| Packs (M1.6) | 1 | 🔴 `store.rs CATALOG` holds 6 gem SKUs only (gems_300…gems_17850), no packs. Needs the schedule machine first |
| Market layer → rows (path A) | ~2–3 | 🟡 3 of 4 parts DONE 2026-07-31 (the (B) decide/apply split, mail is rows, `consumed_ops` is rows). LEFT: commodity + item market, still `market_kv (k TEXT, v TEXT)`. ⚠️ That is precisely the half-done state `gacha-arch-review.md` says to avoid — "rows and blob coexisting on a money path" — and the money path is now live. Verified 2026-08-13 |
| Content authoring + validation (M1.5) | 1–2 | 🔴 no tool exists. worth it when the plan is many tiers / bands 15-20 |
| Quick UI (4 screens) + operator mail | ~0.5 | 🟢 all four screens BUILT — `refine_view` 667, `album_view` 539, `bestiary_view` 324, `stash_view` 211. LEFT: operator mail only (`mail_send` exists but is dev-gated; there is no operator tooling). Verified 2026-08-13 |
| M2 polish (10 items) | ~4–6.5 | 🟡 at least 2 of 10 already built — DPS meter (`dps_view.gd` 192) and desktop companion mode (`companion_view.gd` 430, its own line says "built"). Settings has no view. mostly ~0.5 each |
| macOS build + distribution (M4) | 2 | 🔴 `export_presets.cfg` has ONE preset, "Windows Desktop" |
| **subtotal** | **~14–22** | was 23–35. Mount / ride CUT, and a 2026-08-13 sweep found Stage+dialogue and the 4 Quick-UI screens already built. |

**Total ~20–32 units ≈ 3–5 weeks of code** (was 38–57 ≈ 5–8 weeks; halved by the 2026-08-13
verification sweep, not by cutting scope — only Mount / ride was cut).

> **Read the ✅/🟢/🟡/🔴 marks, not the original numbers.** The sweep checked that the CODE
> exists and is reachable; it did not audit that every path is correct. Re-verify anything you
> are about to plan a date on.

### Critical path (cannot be parallelised)

```
micro-transaction ──→ packs
schedule+claim    ──→ packs · daily login · event windows
dialogue          ──→ tutorial · affection · campaign   (and blocked on written story)
```
Everything else is independent: telemetry, quest, dispatch, save throughput and the security
items do not depend on each other.

### What the code estimate does NOT include

Story text (nothing written yet — memory `world-lore-canon`), art (6 class sheets outstanding +
VFX), Steam review/sandbox waiting time, the P0 difficulty re-tune at the top of this file, and
QA. Those are what turn **5–8 weeks of code** into a realistic **10–14 weeks to open** — and
almost none of that gap is engineering, which is the argument for starting the writing and the
art commissions in parallel now rather than after the code lands.

### Lowest-confidence estimates

1. **Micro-transaction** — third party in the path; refund/edge cases expand it. Worth pulling
   EARLIER than its priority suggests, so surprises arrive while there is still slack.
2. **Stage + dialogue** — 4 vs 6+ depends entirely on scope: a text box, or portraits, voice and
   choices. Decide the scope before estimating again.
3. **Save-write throughput** — depends on whether the fix touches the replay path.

---

## 📖 STORY MODE — locked 2026-08-07

Design settled with the user across one session. The dialogue pipeline is BUILT (`ca849b4`);
everything below it is not.

### The Prologue — a Trial with a pre-made team

The first thing a new account sees, **before character creation**. The protagonist and two named
characters go somewhere dangerous and **lose**; an Exemplar sacrifices itself. Play resumes three
hundred years later with the player at Lv1.

Why it exists, measured: at tick 0 a new account is Lv1, 43 HP, no gear, no Gil, one member — the
worst possible showcase — and a sim-cli hour shows **five wipes between minute 5 and 15**, because
the sim auto-advances on `next_zone_id` and carries a Lv5 party into band 4 before the player has
any reason to touch `SelectBand`. The Trial inverts the order: see what the game becomes, then
earn it. Dying at minute 7 then reads as "I am not there yet".

- **Three named characters**, the story's main cast. They are Exemplars — an Exemplar is already
  "a named character overlaying a class", so no new entity type is needed.
- **Not short.** A two-minute Trial makes the sacrifice weightless and the later "get her back"
  hook worthless (user: it would read as a cash-grab gacha). Length is what buys the attachment.
- Therefore it is a CHAIN — read, fight, read — and **the dialogue file is the spine**, invoking
  battles via cues, not the other way round.
- The scripted **defeat is authored**, not emergent: a pre-made party at power would simply win.
- Numbered missions use the player's real party. **The roster is a per-mission property**
  (`@ roster preset <id>` / `@ roster party`), because the user wants pre-made casts available for
  later scenes too — so preset is a parameter, not an exception. The Prologue sits outside the
  mission numbering, which is now only a naming preference, not an architectural one.

**OPEN, and it must be settled before the launch banner line-up:** is the sacrificed Exemplar ever
obtainable? Recoverable later is the strongest gacha hook available to this game; never obtainable
is the strongest signal that the story has weight. Reversing it afterwards is expensive.

### Missions

Chapter → mission → scene. **A mission is a file**, a scene is a `~ block` inside it.

> ⚠️ **Not a user decision — an engineering rationale, corrected 2026-08-10.** The paragraph that
> used to sit here said "mission id comes from the filename; there is no separate manifest, because
> a manifest that must agree with a directory is the failure mode that cost this project three
> separate bugs this week". It was written by an assistant while summarising the session and filed
> under "settled with the user", which the user never said — the other bullets in this section cite
> them explicitly and that one did not. It was then quoted back at them as their own locked design.
> Everything below is what the code actually does now, and why.

**Mission id is `% id` in the file's own header, and it is REQUIRED.** The no-manifest instinct was
sound and survives: there is still no second list to keep in step with the directory. But deriving
the id from the FILENAME was not sound — the id keys `story_claims`, so it is a key in a payments
table, and a filename is an editing convenience. Tying them made `git mv` an economic act: renaming
`prologue` to `c1_prologue` orphaned the old claim row and paid the reward again. `% id` lives in
the mission's own file, so it is written down instead of inferred, and the file can be renamed
freely. Enforced by `every_story_mission_declares_a_unique_id`.

**The server holds its own list** (`STORY_MISSIONS`) and pays for nothing else. That IS a second
copy of the directory — allowed only because `the_server_pays_for_exactly_the_missions_that_exist`
binds them and fails in both directions. Without it, `mission_id` was any string ≤64 chars and every
distinct one minted a reward: an unbounded faucet, no story mode required.

- **Unlock is sequential and does not check power** (user 2026-08-07). A weak party may always
  attempt the next mission and simply lose; losing is what says "go farm", not a lock icon. This
  matches the already-locked "no scaling — an overpowered party simply wins".
- Resume at **mission** granularity, not scene: it is one value, and `story_claims` doubles as the
  cleared record — CLEARED == PAID, which is why story mode is online-only (they can only diverge
  offline, where the reward call never reaches the server). Read back via `GET /story/progress`.
- Presentation lives in the file; the cleared flag, the reward and any choice consequence live
  server-side, so none of it touches the replay contract.
- `1-1` also means a band sub-zone. The caution applies to text a PLAYER reads, not to ids — a
  mission id is never displayed — so the files are `prologue` / `1-1` / `1-2` (user 2026-08-10).
  Display ORDER comes from the `% requires` chain, never from the filenames: digits sort before
  letters, so `prologue` would land last, and `1-10` between `1-1` and `1-2`.
- Locked missions are **greyed out, not hidden** (user 2026-08-10, reversing the earlier call). A
  disabled row says the mission exists and is reachable; a hidden one cannot be told apart from a
  mission nobody has written. Gating is CLIENT-side and advisory — `STORY_MISSIONS` already caps
  each mission at one payout per account, so playing out of order changes the order paid, not the
  total, and enforcing sequence server-side would buy nothing.

### Player abilities — NOT BUILT, touches sim-core

The complaint this answers: the party fights itself and there is nothing to press early on. Note
the distinction — more class skills would be more to WATCH; this is more to DO. (Base classes have
5–8 skills each; the gap is agency, not variety.)

**PHASE IT: Commander abilities first, character ultimates second** (user 2026-08-07 — the
ultimates are scope creep otherwise). The Commander side is one ability set with one gauge and no
per-class balance work; the ultimates are 10 classes plus 10 Exemplars. The Commander side solves
the stated problem on its own, so it can ship without the rest.

Everything here is **charge, not cooldown**: a cooldown makes you watch a clock, a charge bar makes
you watch the fight, which is the actual complaint. Integer accumulation, so determinism holds.

#### Phase 1 — Commander (do this one)

- A **simple AoE**, and it **scales with the player** rather than fading. Consequence: it is a
  permanent part of the DPS budget, so **C25's difficulty re-tune must count it in**, or the
  tuning is done against numbers no real player produces.
- Gauge is **pure time**: `charge_bp += 10000 / 450` per tick → full every **45 s**, identically in
  every situation. Kills were considered and rejected: a boss fight is one enemy, so a
  kill-fed gauge goes dead in the most engaging minute of the game.
- Abilities are **unlocked by story missions** — this is what gives story mode a mechanical payoff
  beyond a one-off reward.

#### Phase 2 — character ultimates (designed, deferred)

Gauge is per character, **10,000 bp = full**, persists between fights, no decay:

```
auto-attack   charge_bp += 60
skill used    charge_bp += 6 * cooldown_ticks
floor         charge_bp += 5  per tick
CAP           no more than 40 bp per tick, from all sources
```

Skills pay per COOLDOWN, so every skill is worth the same per second and spamming a cheap one does
not beat casting a big one. The floor exists because a tank nothing is hitting, or a healer with
nobody hurt, would otherwise never get a turn — it alone fills in ~3 min 20 s.

Expected: ~59 s casual (1 atk/s + one 5 s skill), ~34 s for a built character, 25 s at the cap.
An earlier draft paid `20 * cooldown_ticks`, which made one 5 s skill out-earn all auto-attacking
and filled the gauge in 27 s — the cooldown normalisation is what keeps that from happening.

**ASPD deliberately DOES speed the gauge up** (user 2026-08-07: try it, tune later). It is a
double-dip — DPS and ultimate frequency — so builds will pull that way. The **cap is the single
number to move** when tuning; the formula does not need to change.

The five tier-1 ultimates, kept deliberately simple and legible over clever (user 2026-08-07 —
"you can see what happened"), and all reusing existing FX so none of them waits on art:

| class | ultimate | numbers |
|---|---|---|
| Mage | three Fireballs back to back | 20,000 bp MATK each, radius 1, 10 ticks apart — 60,000 total |
| Archer | ten rapid shots | 6,000 bp ATK each, single target, one per 3 ticks — 60,000 total |
| Thief | bleed everything in 5×5 | radius 2 around self, 1,000 bp/s for 8 s = 8,000 per target |
| Acolyte | field-wide DoT | every enemy, 800 bp every 0.5 s for 10 s = 16,000 per target, 20 pops |
| Swordsman | self buff | +10,000 bp ATK and DEF for 150 ticks |

Archer and Mage land exactly on 60,000; Thief and Acolyte exceed it when enemies are many, which is
the reward for an area ultimate. Swordsman at +5,000 bp was worth roughly half the others
(+50% on ~24,000 bp/10 s for 15 s ≈ 18,000), hence +10,000. It is also the only one with nothing
on screen when pressed — a shockwave on activation, from existing FX, would fix that.

**Exemplar ultimates are one formula, not ten designs:** the class ultimate plus ONE axis (wider,
longer, or one extra effect) and a faster charge. "Clearly stronger" then becomes a single tunable
number instead of ten unrelated balance problems, and Exemplar #11 needs no new thinking.

#### Cost and exposure

- New `InputEvent` variants + saved charge state = `sim_core::VERSION` bump + re-bless every
  golden. ~3–4 units for the full thing, less for Phase 1 alone. The only work here that touches
  the sim.
- **Security, stated honestly:** cooldowns enforced in sim-core make spam a silent no-op, but a bot
  that triggers at the optimal moment every time out-farms a human by the gap between optimal and
  average timing. Small, real, and the same exposure every auto-battler carries.

### Built vs not

| | |
|---|---|
| BUILT `ca849b4` | `DialogueBox` (2 instances, branching, choices, cues), `DialogueScript` parser, `story/prologue.dialogue` placeholder, `godot/tests/dialogue_selftest.gd` |
| NOT BUILT | `StoryDriver` (2nd SimBridge, preset roster, authored end condition), a cue host, VN presentation + portraits, the `.dialogue` → `strings.csv` extractor, first-launch trigger + skip, server mission/reward tables, all player abilities |

Cue vocabulary is declared in `dialogue_script.gd` (`KNOWN_CUES`) and an unknown one is a parse
error: `roster`, `battle`, `scene`, `cutscene`, `remove_member`.

---

## 🛡️ DEF / MDEF — what the code actually does vs RO (audited 2026-08-07)

Audited against the rAthena source and the iRO Wiki, both saved under `docs/reference/`
(`ro_prere_monsters.csv` + `ro_renewal_monsters.csv`, `tools/fetch_ro_mobdb.py --renewal`).
DECISION: **post-renewal is the reference** (user 2026-08-07).

### Verified CORRECT — do not touch

| | ours | RO renewal |
|---|---|---|
| hard DEF curve | `x (4000+d)/(4000+10d)` | identical; wiki's own checks land exactly — DEF 275 → 36.7% ("about ⅓"), DEF 500 → 50.0% ("half") |
| player soft DEF | `VIT/2 + AGI/5 + Lv/2` | identical, term for term |
| mobbed penalty | −5%/attacker past 2 | matches |
| order | hard %, then soft flat | matches |
| MATK | `Lv/4 + INT + INT/2 + DEX/5 + LUK/3` | renewal (the squared pre-re term is long gone) |

### DONE in 0.0.273 — all four gaps below are FIXED

Kept as the record of what was wrong and why, because the reasoning is the part worth having.
Goldens: 3 of 15 re-blessed (alpha6 / diag_band4 load real monsters, now with soft DEF; beta6's
hand-built slime was drawing 20 soft DEF out of AGI 100 through the PLAYER formula, which has an
AGI term the monster one does not). ⚠️ NOT yet validated for balance — the 7-day sim-cli run
against the captured baseline (Lv27 @ 24h, Lv59 @ 168h) is the acceptance test, not the hashes.

### The gaps

1. **NO hard MDEF at all.** Magic is `amount − mdef`, one flat layer; the code comment has said
   "hard/equipment MDEF is future" since the day magic shipped. RO applies the SAME curve shape with
   a different constant: `x (1000+m)/(1000+10m)` — verified in `battle.cpp:6105` and against the
   wiki's checks (MDEF 60 → ⅓, 125 → half). **K=1000 is MDEF's constant, not a correction to our
   4000** — 4000 is DEF's and ours is right.
2. **`soft_mdef = INT/5` matches nothing.** Traced to `b08d414` (0.0.14, the day magic was added):
   invented on the spot, and the comment says as much. Renewal is
   `INT + VIT/5 + DEX/5 + Lv/4` — for a Lv70 INT60 player that is ~93 against our 12.
3. **Monsters have no soft DEF or soft MDEF.** `effective_mdef` short-circuits on `mob_mdef` and
   `effective_soft_def` reads a stat block monsters do not fill. Renewal: soft DEF `(VIT + Lv)/2`,
   soft MDEF `(INT + Lv)/4`.
4. **`mob_mdef` is a PRE-renewal median** (0.0.95, per 5-level band from rAthena pre-re).

### Two things measurement settled

- **`Defense` has no formula in EITHER edition.** Level correlates 0.22 pre-re / 0.51 renewal, and
  the spread is 0–127 at every level: it is monster IDENTITY, authored per mob. So the level table
  gives a BASE and each monster's `def_mult_bp` shapes it, exactly like hp/atk. VIT correlates
  better (0.60) but that is designers giving tanky mobs both, NOT a formula — the source computes
  `def2` from VIT, and `def` is read straight from the DB.
- **The wiki and rAthena disagree on monster soft MDEF** (`Lv/4 + VIT/10 + INT/5` vs `(INT+Lv)/4`)
  — and it does not matter yet: our monsters carry VIT/INT fields that are 0 (INT on all 42, VIT on
  36), so both reduce to `Lv/4`. It matters the day monsters get real stats.

### Rough tune agreed (user 2026-08-07) — the MDEF half is DONE 0.0.275, the DEF half is not

`def = level` (1→100) and `mdef = round(level × 0.4)` (1→40). That lands on RO's own monster range
(measured: renewal mob DEF median ~40–98, MDEF ~15–43) and gives 18% physical reduction at level
100 — modest by design, because in RO it is PLAYERS who get tanky through DEF (275 for a third),
not monsters.

⚠️ Once hard MDEF exists, `mdef 1–40` on the K=1000 curve reduces MORE than `def 1–100` does on
K=4000 (28.6% vs 18.0% at cap). That is RO-correct but the opposite of what the two ranges suggest
at a glance, so re-check the mdef range when that layer lands.

**That re-check happened and the ×0.4 was wrong — it is `def/4`, 0.0.275.** The warning above
understated the problem twice over. First, the live catalog was not on ×0.4 at all, it was on
**×0.9**, so magic was already being mitigated about three times harder than physical at every
level (Lv96: 20.8% vs 6.9%) and the two Duskmourn bosses were 54% / 42.6% magic walls. Second,
the right ratio is not a taste call: `(1000 + d/4)/(1000 + 10·d/4)` is algebraically
`(4000 + d)/(4000 + 10·d)`, so **a quarter is the exact point where the steeper curve and the
smaller number cancel** — and RO's own renewal mob DB sits there, median MDEF/DEF 0.19–0.28 across
the levels we ship. Whatever `def` becomes in step 3, `mdef = round(def/4)` stays correct, which is
why it is expressed against DEF and not against level.

Guarded by `no_monster_shrugs_off_magic_much_harder_than_it_shrugs_off_steel` (damage.rs), which
runs the real catalog through the real `apply_damage`. 7-day sim: both the mixed and the pure-mage
party reach ~2 sub-bands deeper at the same level, and the mage-vs-mixed gap is unchanged (53 vs
57), so the accepted class spread did not flip.

### Order of work — do NOT merge these

1. ~~`level_base` — move hp/atk/xp off the zone~~ — **DONE 0.0.274**. A monster's strength is now
   `level_base[1..100] × its own multiplier`; a zone carries a `level`, not a scale. Derived from
   the shipped catalog (`tools/derive_monster_levels.py`), so it reproduces the existing balance
   rather than re-tuning it: **the 7-day sim-cli run lands on the SAME level (56 / job 46) as the
   0.0.273 baseline**, kills within 1.5% and wipes within 5%. The only real movers are the 5 gothic
   mobs (their authored numbers never sat on a band curve) and `boss_b9` in Duskmourn room 1.
   Also removed: `scale_from_base` (12 placeholder monsters with it), and `ZoneDef`'s three
   `*_scale_bp` fields, which nothing read any more — an editable field that does nothing is worse
   than no field. Did not touch def/mdef.
2. ~~Hard MDEF + monster soft DEF/MDEF~~ — **DONE 0.0.273**, ahead of the rough tune, because
   changing the mechanism first means the tune is done against the shape it will keep.
3. ~~The def/mdef VALUE tune~~ — **DONE**: MDEF at 0.0.275 (`mdef = round(def/4)`), DEF at
   0.0.277 (`def = the zone's level`, identity via `def_mult_bp`). Armour now sits on RO's number
   — 13.4% physical reduction at the launch cap against their measured 14.7% — where it used to be
   3.7%. Cost: ONE base level over a 7-day sim, with the response curve mapped at x0.5/x1/x2/x3/x5
   first so that "small" could be told apart from "unmeasurable" (x5 costs five levels). A
   pure-swordsman party is unchanged.
   ⚠️ It cut Glacial Storm's freeze rate against a level-98 mob by 26% — DEF → MDEF → the freeze
   roll, three hops from the edit, caught only because 0.0.276 had just pinned that rate.
   **0.0.278 then moved MDEF off DEF and onto the LEVEL** (user call). For an ordinary monster the
   two are the same number, so this touches only the two `def_mult_bp` bosses — deliberately:
   heavy plate no longer buys magic armour, so dullahan is 31.9% against steel and 8.2% against a
   spell, and an armoured boss becomes something a party composition can answer. Note this goes
   AGAINST RO's own data (mob MDEF correlates with DEF at r 0.495 and with level at 0.044 inside
   levels 1-100, n=1334) — a design choice made with the measurement in hand, not a correction.
4. Per-monster VIT/INT, so soft values differ between a golem and a slime instead of coming from
   level alone (`ro_*_monsters.csv` has the columns; our monsters' fields exist and are 0).
   **STILL OPEN**, and now the only one left in this list.

### 🌍 THE WORLD IS 100 BANDS, ONE PER LEVEL (0.0.280)

60 bands x 3 zones became 100 x 3 = 300 zones, `band N == level N`, and the 40 levels no zone had
ever stood on are inhabited. No new monsters and no code change: `level_base` already had a row for
every level, the 20 spawn pools each covered a 5-level stretch before and still do, and every zone
id is built with `format!("zone_b{band}_1")`. `boss_threshold` scaled by 3/5 in the same pass so the
world is not 67% longer — total kills to clear everything went 4077 -> 4075.

**LEFT UNFIXED ON THE USER'S CALL (2026-08-08): the gothic wall.** Wipes cluster hard at the first
band of a pool group — `zone_b46_1` runs 18.2 wipes/hour against ~5 elsewhere. The step at every
pool change is IDENTICAL in the 60- and 100-band worlds, so the band split did not cause it. From
level 56 up the curve is smooth (+6% a group); below it lurches: **-25% at 36, +62% at 41, +44% at
46, -30% at 51**. Cause is placement, not tuning: `werewolf` (`hp_mult_bp` 15845) and `witch`
(16156) are 2.5x their gothic neighbours (maid 7480, mini_ghost 5590), and werewolf stands at 31,
is absent at 36, and returns at 41 and 46. The fix would be to re-pool 36-50 so the heavy monster
is present across its whole stretch — no monster numbers change.

Deferred because Exemplar/Regalia power is expected to absorb it. ⚠️ Worth re-reading before
launch: an overlay raises the floor everywhere, so it makes 36-40 easier too. It moves the wall up
the level range rather than flattening it.

### Opened by the audit, not yet acted on (2026-08-08)

- **Freeze resist is a pre-renewal/community hybrid.** Read from rAthena source, not memory:
  `status_get_sc_def` computes renewal freeze as `sc_def = mdef*100 − levelAdv`, applied
  `rate -= rate*sc_def/10000`, on `status->mdef` — the HARD half only — with **no LUK term** (LUK
  is the `#ifndef RENEWAL` branch) and a flat **+3 s** duration rather than an MDEF-scaled cut.
  Ours is `chance × (97 − MDEF − LUK/3)/100` on hard+soft with an MDEF-scaled duration. Moving to
  renewal proper would make players freezable at the full rate, because —
- **No player can ever have hard MDEF.** `mob_mdef` is written in exactly three places, one of
  which is a monster template and two of which are tests. No item, card, refine, star, path or buff
  grants it, and `items.rs` has no mdef field at all. So the `(1000+m)/(1000+10m)` layer added in
  0.0.273 is the identity function for every player character, forever — renewal magic is built on
  the monster side only.
- **`game_data_version` is still `0.0.68-alpha`** after two catalog changes. The server gates on
  that hand-typed string (`verify-server/src/main.rs:3210`) and bakes the catalog into its image at
  Docker build, so a client with new monster stats and a deployed server with old ones both claim
  0.0.68, pass the gate, and diverge at replay — which reads as a cheating player, not a skew.
- **The bestiary prints raw catalog `hp`/`atk`**, vestigial since 0.0.274: 12 of the 66 placed
  monsters will show "HP 1 ATK 1" (`bridge_stats.rs:688`).
- **`docs/combat-formulas.md` and the GDD** still document magic as one flat subtraction and soft
  MDEF as `INT/5`. CLAUDE.md names the first as the live-formula record.

Merged, a balance shift cannot be attributed to any one of them.

---

## ✅ CODE AUDIT — what is actually built (2026-08-06)

Every remaining code item was checked against the SOURCE, not against this file. Five were
already done or nearly so, and the effort estimate above was written without opening them.

**Bigger than the notes said:**

- **Micro-transaction / store — BUILT.** `store::CATALOG`, routes `/store` `/store/checkout`
  `/store/finalize` `/store/refund`, real `steam_mtx` calls for `InitTxn` (main.rs:1239) and
  `FinalizeTxn` (1303), tables `store_orders` (idem_key UNIQUE, price_cents, first_buy) and
  `entitlements`, and a 255-line `store_view.gd` wired through `server_client.gd`. What is left
  is a sandbox run and the refund cases — 1-2 units, not 3-5. **The reason it was pulled forward
  in the schedule is gone with it.**
- **Gacha — BUILT.** `gacha.rs` is 725 lines (banner tables, pity, wallet, mileage, exchange)
  with `/gacha/roll` `/gacha/exchange` `/gacha/state` live. Only the server nonce and the rate
  query remain.
- **Roll secret is already fail-closed** — main.rs:712 gates the dev literal behind `db.is_none()`
  and is FATAL when a database is configured with no env secret.
- **The four "quick UI" screens all exist** and are instanced in `main.tscn`. That line is a
  polish pass, not four builds.
- **`TEST_DUNGEONS` is already gone**, and `Cargo.toml` already sets `lto = "thin"` +
  `panic = "abort"`, leaving only `strip`.
- **Card drop rows are seeded** — all 19 card defs appear in a drop table. What remains is
  authoring more cards, which is content.

**Smaller than it looked:** `level_req` on meals is a DATA edit — the mechanism already refuses a
consumable below its requirement (`handlers.rs:667`); no meal simply carries the field.

**Confirmed unbuilt, no trace in either crate or the client:** quest layer, band gate,
schedule + claim, packs, dispatch, stage + dialogue, tutorial, operator mail, content validation,
macOS export preset (`export_presets.cfg` has only "Windows Desktop").

Remaining code: **37.8-60.8 units** (was 43.3-68.7). The lesson for the next estimate is that this
file lags the repo by weeks — **read the source before sizing anything in it.**

---

## 📅 SCHEDULE — dated (2026-08-06)

The estimate above says 5–8 weeks of CODE and 10–14 weeks TO OPEN. This turns that into dates and
names what has to run in parallel, because **almost none of the 5-week gap between those two
numbers is engineering** — it is story, art and Steam review, and all three can start today.

From 2026-08-06, 10–14 weeks lands the launch window at **15 Oct – 12 Nov 2026**. Everything
below is planned against the LATE end (5 Nov) so the Steam gates have slack; hitting the early
end just means waiting on Valve with the build already done.

### Hard dates that are not ours to move

| date | gate | source |
|---|---|---|
| ~mid-Sept | depa ฿200K result | pitched 15 Jul, ~2 months |
| launch − 2 wk | store page must be 'Coming Soon' | Steam App Admin |
| launch − 3 wk | submit store page (review 2–3 biz days + buffer) | Steam |
| launch − 1 wk | submit build (review 3–5 biz days) | Steam |

The ≥21-days-since-first-app-credit gate cleared on 2 Aug; it is no longer a constraint.

### Four tracks, run at the same time

| | CODE (me) | ART (you / commission) | STORY (you) | STEAM |
|---|---|---|---|---|
| **W1–2** 6–16 Aug | must-have block A: quest catalog, level-cap gate, gacha nonce + rate query | region backgrounds r4–r8 | **start writing** — zone lore 11-20, then dialogue beats | — |
| **W3–4** 17–30 Aug | **micro-transaction + store** (pulled early: only item with a third party) | r9–r13 | tutorial script | open sandbox test txn |
| **W5–6** 31 Aug–13 Sep | schedule + claim, packs, telemetry client half | r14–r17, class sheets | campaign beats | — |
| **W7–8** 14–27 Sep | save-write throughput, dispatch | VFX pass | dialogue text final | draft store page copy |
| **W9–10** 28 Sep–11 Oct | stage + dialogue system (unblocked by W7-8 writing) | polish pass | — | capsule art + trailer |
| **W11–12** 12–25 Oct | tutorial, M2 polish, difficulty re-tune | — | — | **submit store page ~17 Oct**, live 'Coming Soon' by 22 Oct |
| **W13–14** 26 Oct–8 Nov | QA loop, balance pass, macOS build | — | — | **submit build ~29 Oct** → launch ~5 Nov |

### Team + capacity (2026-08-06)

| who | role | done | left | rate |
|---|---|---|---|---|
| คุณอาร์ท | Character Design / Art | — | 5 | 2 / month |
| คุณกะหู้ | Character Design / Art Director | 1 | 5 | 2 / month |
| พี่กัม | Pixel Art | monster 10, boss 2, hero 5 | hero 10, boss 1, monster 10 | **unknown — the one number missing** |
| คุณ Kiky | Story — intro + rough world design | — | delivers next week | — |
| คุณมู | Logo (done, tweaks) + UI | logo | UI — schedule below | — |
| — | Pixel-art backgrounds | — | 17 regions | **recruiting** |
| — | BGM | — | — | **recruiting** |

### Launch scope — 5 classes (user 2026-08-06)

Launch ships the **5 base classes, which already have pixel art**. Every remaining hero becomes a
**class-2 skin released after launch**, riding the monthly +10 cap cadence.

That removes the collision recorded a day earlier. The character-design chain — 10 designs at a
combined 4/month, last one landing 21 Oct — is no longer on the launch path at all. คุณอาร์ท and
คุณกะหู้ are now feeding the monthly content train, not the release.

**Backgrounds are what is actually launch-critical, and the count is smaller than assumed.** Lv70
is about legacy band 14, and `region = ceil(band/3)` (the bg README's own example: `zone_b7`
displays as "3-1"), so launch content is **regions 1-14** — not all 20. Three exist, so **11 to
go**, and regions 15-20 belong to the three monthly expansions.

พี่กัม is fast and no longer waiting on designs, so his queue becomes: the 11 non-hero pixel
pieces, then those 11 backgrounds. That closes one of the two recruiting slots without hiring
anyone.

**BGM is AI-generated** (user 2026-08-06), so it needs no hire — the work there is selection and
loop points, not composition.

**SFX is a HIRE** (user 2026-08-06), and the idle format is why. Measured pace is **1,078 kills
per hour** sustained over a 168-hour run, and the game is designed to be left running — so a
single hit sound repeats on the order of tens of thousands of times a day, per player. Almost
no other genre tests repetition that hard, and a sound that is merely acceptable on the tenth
play is unbearable on the ten-thousandth.

That should shape the brief: ask for **variation sets and a gentle high end**, not for impact
or loudness. The failure mode here is fatigue, not weakness.

### คุณมู — proposed UI schedule

| window | deliverable | why then |
|---|---|---|
| Aug 6–30 | logo tweaks final + core screen pass (inventory, character, shop, craft) | these four are what a first-time player touches; they exist and need polish, not invention |
| Aug 31–Sep 27 | onboarding / tutorial screens + the quest panel | follows the systems being built W1–6; designing them earlier would be against a moving target |
| Sep 28–Oct 18 | **Steam capsule art + store page assets** | hard gate: the store page is submitted ~17 Oct |
| Oct 19–Nov 1 | polish pass, icon set, whatever QA surfaces | buffer that absorbs slippage from every other track |

### Recruiting is on the critical path

Backgrounds (17) and BGM have **nobody on them** and hiring takes weeks before any output. Every
week without a hire is a week added to the end, and unlike code neither can be compressed by
working harder. The background spec is already written (`godot/assets/bg/README.md`) and the
palette pipeline normalises tone automatically, so a contractor needs no engine knowledge — that
is the cheapest brief to hand out and it should go out first.

### The three things that decide whether this holds

1. **Story starts THIS week or the date slips.** Nothing is written (`world-lore-canon`), and it
   blocks the single largest system — stage + dialogue at 4–6 units — which in turn blocks
   tutorial and campaign. It is the only dependency in the plan that engineering cannot unblock
   by working harder.
2. **Micro-transaction is scheduled at W3–4, ahead of its priority.** It is the one item with a
   third party in the path; surprises there need to arrive while there is still slack, not in
   W12.
3. **depa lands mid-Sept, i.e. W7.** A yes funds the artist commission for the remaining
   backgrounds and class sheets; a no means the art track stays in-house and W9–12 gets tighter.
   Plan the art track so it degrades rather than stalls.

### What is deliberately NOT in this schedule

Bands 15–20 content, the market path-A rows, mount/ride, character skins, and the competitive
events. All are post-launch by the cadence in `launch-level-cap-cadence` (launch at Lv70, +10 a
month). Pulling any of them forward moves the launch date, not the workload.

---

## 📈 Save-write throughput — the scaling wall (measured 2026-07-31)

Not a problem now; the switch to flip before growth. CPU is cheap and predictable, the DB write
rate is what explodes.

**Measured**, from a real 155 KB save (band 28, party of 3):

| | |
|---|---|
| deserialize a 155 KB bundle | 0.51 ms |
| replay span 100 (a normal 10 s push) | 1.7 ms |
| replay span 500,000 (`MAX_REPLAY_SPAN`) | **8.4 s** |
| per save, total | ~2.2 ms CPU |

The server upserts the WHOLE `state_json` into `saves` every 10 s per active player, so write
throughput scales with save size × concurrency. Saves grow too: `grid` is 67% of the file, ~6.3 KB
per character, and the endgame vision is a 10-character party at Lv99 with a full card album —
projected ~295 KB, roughly 2× today.

At 295 KB and CCU = 5% of MAU:

| MAU | CCU | CPU | DB writes | with the two switches below |
|---|---|---|---|---|
| 10,000 | 500 | 0.2 core | 14 MB/s | 0.3 MB/s |
| 100,000 | 5,000 | 2.1 core | **144 MB/s** | **3.0 MB/s** |
| 1,000,000 | 50,000 | 21 core | 1,441 MB/s | 30 MB/s |

**The two switches, ~48× together, both storage-layer only — they do not touch replay, the hash,
or the save format the sim sees, so they are safe to do late:**
- **gzip `state_json`** (~8×; JSON compresses very well)
- **stretch `SAVE_INTERVAL` 10 s → 60 s** (6×; costs the player up to a minute of progress on a
  crash — a product call, not a technical one)

Past ~1M MAU neither is enough: move the state to object storage and keep only hash + tick in
Postgres, or persist the anchor + input log instead of the full state.

✅ **DONE (verified 2026-08-13): `MAX_REPLAY_SPAN` is `50_000` in main.rs.** The paragraph below is
kept because it is the reasoning, not the status — it read:

> ⚠️ Separate and more urgent than cost: `MAX_REPLAY_SPAN = 500_000` lets ONE request occupy a
> core for 8.4 s.** On the current shared-cpu-1x that stalls every other player. It only triggers
when a client fails to sync for hours (the anchor advances on every confirmed save), but the
bound is far higher than it needs to be — 50,000 would still cover 83 minutes of unsaved play and
caps a request at 0.84 s.

---

## 🎲 Gothic-dungeon LOOT TABLES — the Duskmourn mobs drop nothing (user 2026-07-31)
Found while checking why the dungeon clear screen reported "Items banked: 0". It is not a UI
bug — the counter is correct and the dungeon genuinely has no client-side loot.

Every Duskmourn monster's drop table is a single `nothing` row plus, on some, a
**`weight: 0` card entry with `server_bp: 50`**:

```
maid / maid_horn / werewolf / banshee / witch / maid_warden / maid_horn_warden
    nothing        weight=100
    card_*         weight=0   server_bp=50     ← client can NEVER roll this
brutus_lord (room 2 boss), dullahan (room 4 boss)
    nothing        weight=100                  ← nothing at all, not even a card
```

`weight: 0` means the client roll can never select it; the only payout is a 0.5% server-rolled
CARD candidate, resolved at save-verify and therefore not "items banked". Compare `boss_b9`,
an ORDINARY band boss, with **49 real drop rows** — killing a normal band boss out-loots
clearing the whole dungeon.

Decide, then do one of:
- **Give the gothic mobs real tables** (band 10 appropriate), so a run pays out; or
- **Keep it card-only by design** and change the summary line to count PENDING CARD candidates
  from the run ("2 card rolls pending") instead of a permanent "Items banked: 0", which reads
  as broken.

Related: `docs/server-roll-drops-design.md`, memory [resume-gothic-cards],
[duskmourn-manor-dungeon]. Deferred by the user 2026-07-31; the run-end screen itself is done
(0.0.251).

---

## 🔒 Bound-item craft/refine laundering — REVERSED, tradeable output (user 2026-07-24)
- Earlier (2026-07-23) the craft/refine OUTPUT was bound when any bound material/ore fed it
  (audit H1/H2 anti-laundering). **Reversed 2026-07-24**: the player can't pick which stack
  (bound vs unbound) a craft/refine consumes — bound-first is forced — so binding their
  tradeable result is a surprise worse than the tiny laundering. Now: bound-first consume is
  KEPT (clears clutter, deterministic) but the OUTPUT stays tradeable (`craft.rs` grants
  `grant_loot`; `refine.rs` dropped the `if bound_ore>0 { .bound=true }` in both refine paths).
  In-game market only → laundering value negligible; **gate by price/quota if abused**.
- `grant_loot_bound` (data/build/items.rs) is now UNUSED by craft/refine but kept as a general
  helper for future bound drops.

## 📮 Game-to-player mail (user 2026-07-22)
- [ ] **Operator/system mail to players** — today mail only comes out of TRADE flows
      (AH sale proceeds / listing returns); there is no way for the GAME to mail a
      player directly (event rewards, compensation, patch gifts, welcome mail).
      Needs: an admin/ops path on the verify-server to author mail (single player /
      broadcast, with attached Gil/items), delivered through the existing inbox +
      verified ClaimMail path — client UI already handles it.

## 🗒️ Quick UI TODO (user 2026-07-06 — build/polish these screens)

**ALL FOUR EXIST (reconciled 2026-08-21).** Every screen on this list was built and the list was
never ticked. Sizes are given because "the file exists" is a weaker claim than "the screen is
done" — these are not stubs, but whether each is FINISHED is a judgement to make by opening it,
not by reading here.
- [x] **Refine shop (ร้านตีบวก)** — `views/refine_view.gd`, 667 lines
- [x] **Bestiary (beastiary)** — `views/monster_catalog.gd`, 320 lines
- [x] **Stash** — `views/stash_view.gd`, 211 lines
- [x] **Card Album** — `views/album_view.gd`, 539 lines. See `docs/handoff-card-album.md`; the
      open item there is a visual pass on the enchant flow, not the screen.

## 🎭 LATE — Character skins (user 2026-07-07)
- [ ] Named-character SKINS (e.g. "Rin Sword Girl") equippable on ANY instance of a
      base-class family (Rin → any swordsman). **COSMETIC ONLY** — render layer +
      account-cosmetic record, MUST NOT enter GameState (no combat/hash impact).
      Power-neutral → clean monetization + natural tradeable-cosmetic via the S3
      mint pipeline. See memory `character-skins-idea`. Not scoped.

## ✅ Done
- **Multi-element system (9 elements + loose RO/Pokemon web)** — `grid::Element`
  now Neutral/Fire/Earth/Water/Wind/Holy/Shadow/Poison/Thunder; `combat::
  element_mult_bp` is an `ADVANTAGE` pair-table (150/50/100, no per-element
  levels): Fire>Earth>Wind>Water>Fire · Thunder>Water · Earth>Thunder ·
  Fire>Poison (Poison's only weakness, no offence) · Holy<->Shadow mutual.
  172 unit + all goldens green, NO re-bless. 2-1 skills now use REAL elements
  (Vermilion=Wind, Sancta=Holy, Poison Cloud=Poison, etc. — no placeholders).
  TODO (P1, deferred): player-facing ELEMENT CHART UI in Godot (a reference
  panel — the web is guessable but not pure-memorable) + show each monster's
  element + per-hit weak/strong feedback. DPS-meter overkill cut -> effective
  damage (recorded value clamped to target HP at all 4 sites).
- Mage cast time (RO VCT, DEX·2+INT) + cast bar
- Auto-retreat farming (5 wipes → farm prev band 20× → climb back)
- 3hr-loot items REAL with mechanics (Max Potion / Field Ration / Rejuvenating /
  Phoenix Tail / Monster Lure / Accelerator + Aegirilium + 7 uniques)
- Item tooltips (all items) + shop 2-col + hover tooltips
- Monster HP/atk/xp rescaled (RO-shaped ramp) + Lv50@3day pacing
- Windows tester build (cross-compiled DLL) + sprite-in-export fix
- Card SYSTEM exists: sockets, consume-for-stat, 3 slime cards
- **Session 2026-07 (gameplay/system):** Roster⇄Party (built) + lead removal +
  party-naming · gear s5-s7 + b10-14 maps 32×16 · Pet Master 4×→20×level ·
  Subvenio decoupled from Heal cd · auto-retreat now sim-cli-ONLY (game restarts
  in place on wipe) · Ranger traps bias to the enemy side + Land Mine uncapped ·
  DPS meter now attributes field/trap/pet/flame-shield/poison-explosion damage ·
  Freeze CENTRALIZED in `Entity::take_hit` — now shatters on ANY direct hit incl.
  ranged/magic + ground fields (only DoT/poison leaves it) · b1 mob MDEF 15→0
  (Lv1 mage basic no longer floored to 1) · tooltip-lingers-on-close fix.

---

## M1 — Gameplay-complete to Lv70  (P0, the bulk)
The game should be fully playable/balanced to Lv70 with the class advancement.

- [x] **Job advancement + 2nd-tier classes (first branch)** ✅ DEMO SCOPE COMPLETE
      — 5 t1 + 5 first-branch t2 + advancement pipeline/UI all built. (2nd branch
      per t1 = POST-DEMO, out of the 6-month scope.)
  - [x] **Advancement pipeline BUILT** (commit 9f108d9) — `PlayerClassDef.
        advances_to` + `build::advance_class()` + bridge `advance_class` /
        `advance_options_json`. Gate = JOB MAXED (job_level == JOB_LEVEL_CAP 50),
        NOT base Lv40 (user override: same cap every tier; class-3 advances when
        2-1 job maxes). Keeps base level/stats/skills/gear, resets job to 1.
        First class proven: thief_t1 -> assassin_t2 (+ Knife Mastery skill).
        UI BUILT (commit 7ddbb94): skill tree panel (K) shows a green
        "ADVANCE -> <Class>" button per option when the viewed member's job is
        maxed; click advances + saves + refreshes. Draws one button per
        advances_to entry (branch-ready). TODO: more 2-1 classes wired the same
        way + a 2nd branch per t1.
        FUTURE (P2 polish, user 2026-06-23): replace the header button with a
        GRAND dedicated advancement screen — class art/silhouette, the branch
        options side-by-side with their new-skill previews, a confirm step, and
        a celebratory effect on advance. The current in-panel button is the
        minimal "works now" version.
  - [~] Branching classes — **FIRST branch of every t1 BUILT (5/5):** swordsman→
        **Swordmaster** · mage→**Elementalist** · thief→**Assassin (7/7)** ·
        acolyte→**Bishop (8/8)** · archer→**Ranger (8/8)**. Each has catalog skills +
        t2 class def + registered skill tree + advance wiring.
        **SECOND branch per t1** (Templar/Swordmage/Trickster/Monk/Dancer) =
        **POST-DEMO (out of 6-month scope)** — not started, future work.
        Per-class balance pass still pending (sim-cli).
  - [ ] New mechanics if used: DEF-ignore (Blitz Beat), party shield, "next hit 2x"
  - SWORDMASTER (swordsman 2-1, sword/shield) ✅ BUILT — 7 new skills: Cleave
    (orthogonal splash passive), Double Edge (2-hit active, triggers Cleave),
    Sword Quicken (aspd reserve), Autocounter (counter reserve, +1%/lvl w/ shield),
    Sword & Shield Mastery (unlock 2H+shield, -50%→0% + baseline -5 gone @max),
    Shield Blast (5x5 AoE, %ATK+%hardDEF), Shield Boomerang (furthest r7, taunt 5s).
    Global: shield baseline -5 aspd. New mechanics: orthogonal AoE, taunt,
    furthest-target, DEF-scaling skill dmg, equip-rule unlock.
  - ELEMENTALIST (mage 2-1, AoE nuker) ✅ BUILT — 9 new skills. AoE: Glacial Storm
    (7x7 persistent 4.5s/10hits 120-570%MATK water, random knock, freeze@3rd),
    Meteor Rain (8x8 area, 5x5/meteor, 2-7 meteors x1-5hits, 125%MATK fire,
    sky-fall, no stun), Vermilion Field (7x7, 500-1400% total /20hits, Wind→Earth
    placeholder, blind), Earth Drive (5x5, lvl-hits x125%MATK), Earth Piercer
    (earth bolt, lvl-hits x100%MATK, ground-up). Util: Flame Shield (absorb
    INT*lvl*2, explode 100%MATK fire 3x3 + knock1/2/3, cd30s), Quicksand (7x7
    auto-cast slow DEX/AGI 5-25% 10s, cd30s), Incantation Mastery (variable-cast
    -2%/lvl), Soul Crush (passive: normal atk -> 200% MATK, 5-25 charges/level,
    refill on zone change). FROZEN status: immobilize 7s, DEF-50%, FLEE 0, armor
    -> Water, BREAKS on any damage. New mechanics: persistent ground-field,
    freeze/blind status, FIXED+VARIABLE cast (fixed = unreducible floor),
    absorb-shield+explode, auto-cast trigger, DEX/AGI debuff, random knockback.
    See [[element-system-mockup]] — design uses INTENDED elements.
  - ASSASSIN (thief 2-1, dual-wield burst) — **BUILT so far: Knife Mastery,
    Dual Wield Mastery (full dual-wield: off-hand slot + 2-hit basic + −10 ASPD),
    Bloodthirst (kill-stack +0.5%/lvl, cap 50, reset on new instance run /
    death; streak persists across rooms 1→2→3), Dark Ambush (active cd 10s:
    blink onto NEWEST non-boss spawn, range 9, 150%→600% ATK), Sonic Blow
    (active cd 10s: single 300%→1200% packet shown as 8 hits + STUN status —
    immobilize 3s, targetable, no break on dmg, boss-immune), Poison Cloud
    (active Lv5 cd60s: 5x5 FIXED persistent ground field [first GroundField
    engine], applies classic-Poison each interval; classic-Poison = soft DEF
    -25% / 1.5%MaxHP+2 per s floor 25% can't-kill / SP-regen off / boss-immune).
    , Poison Explosion (passive Lv10 cd30s: killing a POISON-afflicted enemy
    [classic Poison or Deadly Poison, NOT Bleed] detonates a 5x5 physical
    Poison blast, 500%->1400% ATK split among enemies; cd gates chains).**
    ✅ **ASSASSIN COMPLETE 7/7.** NOTE: thief Venom Edge -> **Open Wound**
    (re-themed to BLEED so poison stays the Assassin's theme; Bleed does NOT
    feed Poison Explosion). + built support systems: after-cast/motion delay,
    STUN status, classic-Poison status, GroundField engine.
    DESIGN below:
    Dark Ambush (active, teleport to a non-boss spawn on spawn → instant hit,
    150-600% ATK +50%/lvl, physical single-hit, cd 10s, max Lv10),
    Bloodthirst (passive: on kill / last-shot +0.5%/lvl damage, stacks, cap 50,
    resets on band-change OR death, applies to ALL damage, max Lv10 → +5%/kill),
    Dual Wield Mastery (passive: unlock 2 daggers; off-hand dmg 30+lvl*7%
    [Lv10=100%]; PERMANENT -10 aspd [starting tune value, mastery does NOT reduce
    it — inherent per RO]; 1 BASIC attack = 2 hits, each hand uses its OWN weapon
    damage + rolls Crit + Double-Attack INDEPENDENTLY; SKILL damage uses
    MAIN-HAND [right] weapon ONLY — off-hand never feeds skills, no double-dip),
    Sonic Blow (active, 8 hits continuous single-target, 300-1200% ATK TOTAL [÷8
    per hit, Lv10=150%x8], stun 12-30% chance x ~3s, cd 10s, max Lv10,
    main-hand ATK, NO katar restriction — usable with any/dagger),
    Knife Mastery (passive Lv10: +3 ATK/lvl [Lv10=+30], DAGGER-only — add a
    `WeaponType::Dagger` arm to `passive_atk_bonus` in skills/effects.rs:114,
    `atk_flat_per_level: 3`; flat ATK so it feeds basic AND skill, no main-hand
    conflict),
    Poison Cloud (active Lv5, cd 60s, no SP — cd-gated: self-centred 5x5 PERSISTENT ground
    field [like Quicksand/Glacial Storm], duration 10/15/20/25/30s [+5s/lvl],
    applies classic POISON status to every enemy inside each interval — the AoE
    the assassin lacked). STUN status (NEW): immobilize ~3s, does NOT change
    element, does NOT break on damage (unlike freeze). New mechanics:
    teleport-to-spawn-strike, kill-stack damage buff, dual-wield 2-hit basic
    (skill=main-hand only), stun status, classic-poison status,
    Poison Explosion (passive Lv10, cd 30s: on killing a POISON-afflicted enemy
    [EITHER classic Poison OR Deadly Poison triggers it], explode at the corpse,
    5x5, physical, element Poison; 500%+100%/lvl ATK [500-1400%] as a TOTAL pool
    DIVIDED equally among enemies in the blast [1 target=full, 5=÷5]). Combos
    with Poison Cloud: AoE-poison a pack -> kill one -> chain explosion.
  - TWO POISON SYSTEMS (locked) — keep SEPARATE:
    * **Deadly Poison (EDP)** = the EXISTING `BuffKind::PoisonDot` (ATK-scaled
      TOTAL/duration, raw HP, CAN kill, no side-effects, hits bosses). Used by
      Venom Edge (thief-1). UNCHANGED.
    * **Poison (classic, NEW `BuffKind::Poison`)** — used by Poison Cloud:
      soft/status DEF -25%; lose 1.5%*MaxHP + 2 per second ONLY while HP > 25%
      (floors at 25%, CANNOT kill, does NOT route death path); SP regen disabled;
      BOSSES IMMUNE (never applies). Engine: new buff kind + DEF arm + MaxHP-%
      DoT w/ 25% floor in upkeep + boss-immune guard on apply + SP-regen gate.
    ASSASSIN = 7 skills, COMPLETE: Dark Ambush, Bloodthirst, Dual Wield Mastery,
    Sonic Blow, Knife Mastery, Poison Cloud, Poison Explosion.
  - BISHOP (acolyte 2-1, support backbone + battle-priest/holy-DPS options)
    DESIGNED. Acolyte-1 base: Heal, Blessing,
    Agility Up, Guardian Aura, Holy Light. New skills so far:
    * Subvenio (active/auto, Lv5) — when an ALLY (NOT self) drops < 40% HP:
      teleport to them + cast HEAL on them. Heal amount = the priest's own Heal
      skill value (renewal formula at their Heal level — not a new %). Firing
      ALSO consumes Heal's cooldown (shared, prevents Subvenio + instant manual
      Heal double-burst). Own cd 120/105/90/75/60s (Lv1-5, -15/lvl).
    * Resurrection (auto, Lv10) — when a party member dies & a charge is
      available: auto-revive (idle = no manual target, so revives the FIRST ally
      to die each band) at 10%*lvl MaxHP (Lv10 = 100%). 1x per band (reset on
      band-change, like Hiding/Bloodthirst). Brings a member back mid-instance
      without a full wipe/room-restart.
    * Gloria (active party buff, Lv5) — +6 LUK/lvl → +6..+30 (Lv1-5).
      Reservation buff (cast_buff_party + sp_reserve ~18, cd ~280), same pattern
      as Blessing/Agility Up; engine already buffs LUK (buffs.rs:241). LUK in our
      stats = crit (LUK-1)*0.30% + ATK/MATK/HIT (LUK/3) + FLEE (LUK/5), so +30 =
      ~+8.7% crit + ~+10 ATK/MATK/HIT + ~+6 FLEE party-wide (great for crit
      builds / Assassin). Becomes timed-cast later ([[future-cast-based-party-buffs]]).
    * Praesidium Divinum (= Kyrie Eleison rename) (active, target ally, Lv10,
      cd 20s, no SP — cd-gated) — damage-ABSORB shield with DUAL cap: pool = the CASTER's
      (priest's) MaxHP * (10+2*lvl)% [12-30%] AND 5+floor(lvl/2) hits [5-10],
      whichever empties first ends it. A blow exceeding remaining pool passes the
      overflow through (counts 1 hit, reduced) then ends. Auto-target = lowest-HP%
      ally. PERSISTS across room/zone changes (special — most buffs clear on
      transition; flag at impl). Magic SIMPLIFIED (absorbs phys+magic equally;
      dropped RO's magic-bypasses-but-still-counts quirk). Re-cast replaces.
      Shares a NEW absorb-shield engine feature w/ Elementalist Flame Shield (pool of
      absorbed damage), plus a hit-count cap on top.
    * Purificatio (cleanse) (active, single-target auto, Lv5, cd 5s, no SP) —
      removes ALL currently-unlocked debuffs from one ally at once (auto-target
      the ally with the worst/highest-priority ailment). Level gates WHICH it can
      clear: Lv1 Stun / Lv2 +Freeze / Lv3 +Blind / Lv4 +Poison(classic) /
      Lv5 +Slow. NEVER removes Deadly Poison/EDP (heavy poison, cleanse-immune by
      design). Forward-looking: matters once monsters/bosses inflict these on the
      party (mostly player->enemy today).
    New mechanics: auto-teleport-heal-on-threshold, mid-fight revive,
    absorb-shield (pool + hit-count cap), buff-persist-across-rooms, cleanse
    (level-gated debuff removal, EDP-immune).
    * Sancta Tempestas ("Holy Storm") (active AoE field, Lv10, cast ~1s
      fixed+variable, cd ~15s, no SP — cd-gated) — opens the priest's HOLY-DPS path.
      5x5 persistent field lasting 10s; fires a WAVE at t=0/3/6/9s (max 4 waves,
      4 only if enemies present from the start). Each wave = 10 hits x (10%*lvl
      MATK) per hit -> Lv10 = 100% MATK/hit x10 = 1000% MATK/wave. DamageType
      Magic. Element = HOLY (intended; mock has no Holy -> Neutral placeholder,
      [[element-system-mockup]]). Auto-target the densest enemy cluster, field
      stationary. NOTE: this opens a light DPS path, a deliberate deviation from
      the acolyte "support-only" vision ([[class-balance-sim-findings]]).
    * Dupla Medicina ("double medicine") (active/auto, Lv10, cd 12s, reserve 0)
      — smart 2-target heal. Auto-fires when >=1 ally is below 75% HP; heals the
      2 LOWEST-HP% allies (map-wide, no range limit; INCLUDES the priest). Heal
      per target = the renewal Heal formula evaluated at DUPLA'S OWN level * 60%
      (Lv1 = 60% of Heal-Lv1, scaling like Heal every level up to Lv10). Fills
      the priest's AoE-heal gap (efficient triage rather than a full party heal).
    * Mace Mastery (passive, Lv10) — +5 ATK/lvl [Lv10 = +50], MACE-only (add a
      `WeaponType::Mace` arm to `passive_atk_bonus` in skills/effects.rs, like
      Sword/Knife Mastery; `atk_flat_per_level: 5`). Flat ATK -> feeds basic AND
      skill. Opens a battle-priest (mace-melee) path. +5 > Sword +4 / Knife +3
      (maces hit harder, slower aspd, per RO).
    BISHOP = 8 skills, COMPLETE: Subvenio, Resurrection, Gloria, Praesidium
    Divinum, Purificatio, Sancta Tempestas, Dupla Medicina, Mace Mastery.
  - RANGER (archer 2-1, pet + trap + kiting zone-control) DESIGNED.
    Archer-1 base: Twin Strafe, Keen Eye, Arrow Shower, Sharp Focus, Hawk Sight.
    8 new skills:
    * Knock Back Shot (passive, Lv5) — basic attacks have 5%/lvl chance (5-25%)
      to push the target back 2 cells (away from the archer, reuse
      knockback_entity). ONCE PER TARGET — each enemy can be knocked by this
      skill only once in its lifetime (prevents perma-kite/chain-push; one
      breathing-room shove, not a permanent lock). Engine: needs a per-target
      "already-KB'd-by-this" marker (flag/buff). Kiting tool for the ranged archer.
    * Arrow Crafter (passive, Lv5) — on the archer's LAST-HIT kill of a
      NON-Neutral enemy (Fire/Earth/Water), 10%/lvl chance (10-50%) to craft
      1 arrow (FIXED) of the enemy's element into the quiver/ammo. Neutral
      enemies never proc. 1 PRNG draw (chance only), conditional on the killer
      having the skill -> goldens safe (like Steal). Passive ammo farm feeding
      the elemental-arrows system ([[elemental-arrows-design]]) — the trickle
      alternative to buying 10k-round quivers.
    * Pet Master (passive, Lv10) — summons an ABSTRACT pet auto-attacker
      (engine approach A: no HP, can't be targeted, can't die — "try this first").
      Pet auto-attacks the nearest enemy WITHIN THE ARCHER'S attack range,
      ~1 hit/sec (tunable). Pet damage uses the PET'S OWN self-contained ATK
      stat = 4*lvl (Lv10 = 40 ATK), physical/neutral, minus target DEF — NOT
      derived from the archer's stats. Foundation for layered pet skills
      (Blitz Beat / Falcon Assault) that add to pet ATK later. Godot renders a
      cosmetic companion sprite; mechanics stay deterministic/abstract.
    * Pet Assault (active, Lv10, cd 5s) — "Bash but for the pet". The PET
      auto-casts a heavy single-target strike on its target (Bash-style):
      150% + 50%/lvl (Lv1 150% -> Lv10 600%) of the PET's ATK stat,
      physical/neutral, cd 5s. Stronger than Bash (max 380%) to offset the pet's
      thin base ATK (4*lvl). Requires Pet Master (no pet -> unusable). First
      layered pet skill on the Pet Master foundation.
    * Snare Strike (passive, Lv10, prereq Pet Master) — the PET's attacks have
      2.5%/lvl chance (2.5-25%) to ROOT the target for 2s. ROOT = NEW status:
      movement locked but the target CAN still attack (unlike Stun which blocks
      all actions; unlike Freeze). ONCE PER TARGET (per-target marker, like
      Knock Back Shot — prevents perma-root). Synergy w/ Knock Back Shot: shove
      the melee enemy away, then pin it so it can't chase.
    * TRAP SYSTEM (NEW engine feature) — ground-object list on GameState
      (`Vec<Trap>{cell, kind, level, owner_id}`, serialize -> VERSION bump), NOT
      entities. Placement (idle = auto): the trap skill is an active on a place-cd;
      each placement drops 1 trap on a RANDOM (PCG32 -> deterministic) cell on the
      BORDER ring of a 5x5 around the archer (the 16 perimeter cells at range 2 —
      a defensive perimeter that intercepts approaching enemies). PRNG draw is
      conditional on the Hunter having the trap skill -> goldens safe. If the
      rolled cell is blocked, scan clockwise to the next free cell (no re-roll, to
      keep the stream stable). TRIGGER: an enemy enters the 3x3 around the trap
      (trap cell +-1). LIFETIME: traps PERSIST until room/zone change (no timer
      expiry; cleared on transition). Caps are PER-SKILL (3 each; learning both =
      both kinds active independently). Trigger check each tick iterates
      traps x enemies in index order (deterministic); damage routes the normal
      path; status applies a debuff. Godot draws ground sprites + activation FX.
    * Land Mine (active, Lv5, place cd 8s, cap 3) — on trigger, immediately hits
      the single triggering enemy for 250% + 50%/lvl ATK (Lv5 = 500%), single-target.
    * Claymore Trap (active, Lv5, place cd 10s, cap 3) — on trigger, a 5-tick fuse
      (0.5s) then explodes 3x3 for 150% + 30%/lvl ATK (Lv5 = 300%) to all enemies
      in the area. The delay lets more enemies wander in (bigger AoE) at the risk
      they walk back out.
    * Freezing Trap (active, Lv5, place cd 12s, cap 3) — pure CC, NO damage. On
      trigger, FREEZES all enemies in 3x3 for lvl+2 seconds (Lv1 3s -> Lv5 7s).
      Reuses the Elementalist Freeze status (immobilize, DEF-50%, armor->Water, BREAKS
      on any damage taken). No damage on purpose — dealing damage would break its
      own freeze (the Glacial Storm problem); let pet/archer/other traps do the
      hitting (shatters the freeze, but the target was already locked).
    * FREEZE-in-auto-game RISK (applies to ALL freeze: Freezing Trap + Elementalist
      Glacial Storm) — in an idle/auto game the party auto-attacks constantly, so
      a "breaks on any damage" freeze gets shattered immediately by your OWN
      attacks, gutting its CC value. NEEDS PLAYTEST. Mitigation levers to keep in
      mind: (1) AI DEPRIORITIZES frozen enemies (auto-target hits non-frozen
      first; freeze only breaks when no other target — preserves the lock, is
      RO-authentic); (2) SHATTER = bonus damage on break (turn the problem into a
      feature: freeze->shatter combo, so an auto-attack that breaks it is a damage
      setup, not waste; reinforces the party-comp synergy pillar). 1+2 together
      make freeze valuable as BOTH control and damage-amp in an auto context.
    New mechanics: per-target one-time knockback marker, on-kill ammo crafting,
    abstract pet auto-attacker (own ATK stat, ranged to archer's range),
    pet-cast skills (layered on Pet Master), ROOT status (move-lock, can attack),
    TRAP system (ground objects, 5x5-border PRNG placement, 3x3 trigger,
    persist-until-room-change, per-skill caps, fused AoE).
    RANGER = 8 skills, COMPLETE: Knock Back Shot, Arrow Crafter, Pet Master,
    Pet Assault, Snare Strike, Land Mine, Claymore Trap, Freezing Trap.
  - ENGINE SYSTEMS for 2-1 (BUILD SYSTEM-FIRST, not class-by-class — ~38 new
    skills collapse into ~9 shared systems; building a system unlocks many skills):
    1. **persistent ground-field** — Glacial/Vermilion/Meteor/Quicksand/Earth Drive,
       Poison Cloud, Sancta Tempestas (and traps are adjacent). 1 system ~10 skills.
       BUILD FIRST.
    2. **status framework** — stun / freeze / blind / ROOT / classic-poison, with
       per-status effects (immobilize, can-still-attack?, DEF mods, breaks-on-damage).
       BUILD FIRST (cross-class).
    3. **absorb-shield** — Flame Shield + Praesidium Divinum (pool of absorbed dmg;
       Praesidium adds a hit-count cap + caster-MaxHP pool + persist-across-rooms).
    4. **on-kill hook** — Bloodthirst / Poison Explosion / Steal / Arrow Crafter
       (+ Bishop nothing) — one "entity died, last-hitter = X" event, conditional
       PRNG draws only when the killer has the relevant skill (goldens safe).
    5. **teleport** — Dark Ambush / Subvenio / Shadow Strike (partly exists).
    6. **weapon-gated mastery** — Knife/Mace/Sword (EXISTS: passive_atk_bonus).
    7. **abstract pet + trap** — Hunter-only, heaviest, build LAST (lowest reuse).
       Pet = no-HP auto-attacker w/ own ATK stat; trap = ground-object list.
    8. **charge system (Pokemon-PP style)** — skill has `max_charges` (may scale by
       level, e.g. Soul Crush 5-25/lvl); each cast spends 1; refills on a condition
       (zone/band change, etc.). "Once per band" (Resurrection, Hiding) = the
       special case max_charges 1 + refill-on-band -> UNIFY into this. Store
       `charges_left` in SkillState (serialize -> VERSION bump). Deterministic.
    9. **once-per-target tracking** — Knock Back Shot, Snare Strike (and trap-ish
       markers). Recommend a per-ENTITY BITMASK ("which once-per-target effects
       already hit me"); set the bit on apply, check before applying, clears on
       despawn. Cleaner than marker-buffs (no expiry worry) or owner-side sets
       (no dead-target cleanup). Deterministic, serialize-able.
    Slice order: systems 1+2 first (unlock ~half the kit), then 3/4/5/8/9, pet+trap
    (7) last. Per-class balance (Assassin burst, pet falloff, Bishop holy-DPS) =
    the sim-cli balance pass. NOTE: do NOT "trim" the Elementalist's multiple AoE
    nukes — Glacial(Water)/Meteor(Fire)/Earth Drive(Earth)/Vermilion(Wind) are
    INTENTIONAL multi-element coverage, not redundancy. They only LOOK redundant
    now because several map to Earth/Neutral placeholders ([[element-system-mockup]]);
    they differentiate once the real multi-element system ships.
  - ~~**Mount / ride (Peco-Peco-style bird)**~~ — **CUT 2026-08-13 (user).** Was
        FUTURE/ART-GATED: a full mounted sprite set per class. Recorded rather than
        deleted so it is not re-proposed as a fresh idea.
- [x] **Thief-1 skill expansion (4 → 7, parity w/ swordsman/mage)** ✅ COMPLETE 7/7
      (Shadow Strike, Double Attack, Open Wound, Nimble Mastery + Steal, Hiding,
      Sand Strike). Built in `skills/catalog/thief.rs`:
  - [x] **Steal** BUILT (commit 776b0f7) — REWORKED to LOOT-steal (kill-Gil is 0
    in data): on the thief's last-hit kill, 10%/lvl chance for an EXTRA roll on
    the victim's drop table (reuses drop_on_death, obeys the NOTHING sentinel =
    can miss). Job-9 thief node + carried into Assassin. Built the reusable
    ON-KILL HOOK (handle_monster_death gains `killer: Option<usize>`, threaded
    from direct-attack sites) — Bloodthirst/Poison Explosion/Arrow Crafter reuse
    it. Conditional PRNG -> goldens safe.
  - [x] **Hiding** BUILT (commit c80dae1) — passive Lv6, job-9 thief node (+
    carried into Assassin). HP<30% -> auto-vanish 10s, UNTARGETABLE
    (find_enemy_idx skips) + no actions, HoT heals (10+10*lvl)% maxHP [Lv1 20% ..
    Lv6 70%], 1x/band (Entity.hiding_used serde-skip, reset on band-advance).
    Godot fades the sprite to alpha 0.35. Threshold trigger (no PRNG) -> goldens
    safe. **thief-1 COMPLETE 7/7** (parity w/ swordsman/mage).
  - [x] **Sand Strike** BUILT (commit 28b05c3) — active, job-7 thief node (+
    carried into Assassin), 200% ATK FIXED, 7%/lvl -> 7-70% BLIND, Earth, cd 5s.
  - [x] **BLIND status** BUILT — `BuffKind::Blind` marker, -25% HIT & -25% FLEE,
    5s. Reusable by Elementalist Vermilion + Ranger. Added `DebuffSpec.
    chance_per_level` (level-scaled proc). MONSTER STATUS RESISTANCE deferred:
    monsters carry no real stats (all 1) so RO stat-based resist can't
    differentiate them -> plan is a scalar `status_resist_bp` on MonsterDef
    (bosses tune high) when boss CC lands. STILL TODO for thief-1 parity (-> 7):
    Steal (on-kill hook) + Hiding (charge + untargetable).
- [x] **Bands → Lv70 content** — ✅ extended b10-14 (Lv46-70) from RO pre-re
      medians + boss HP×5/ATK×2; maps shrunk to 32×16 (match early rooms); gear
      s5-s7 seeded for the new bands. Pacing verified: Lv70 ≈ 13d @24/7 (on target).
- [x] **Loot-grant wiring** — 3hr-loop claim grants REAL items (sim grant_loot →
      stash / rare bag by rarity). SSR Draw gacha (5%→10% ramp by band clears) →
      SSR pool (card/socket/unique) vs normal pool; RATES disclosure page.
      TODO: tune loot VALUES (some items don't feel worth the slot yet); move the
      WHICH-item roll server-side for the real online economy (currently client RNG).
- [ ] **Card variety + drops** — more cards per band/monster + seed real drop
      rates (currently ~0). Card effects per archetype. drop model drafted (see
      **docs/drop-system.md**): common = per-kill uncapped, value tiered by band
      (over-level self-balances); cards = ~1/20k/kill + DAILY CAP 2/24h total,
      server-rolled; dungeons/events bypass cap but gate access. Kill rate baseline:
      ~900/hr full party (~14.5k/day @16h).
- [~] **Balance pass (sim-cli)** — [x] Lv70 XP pace (v0.0.94) + [x] gil-gate FIXED
      (sim-cli now recruits a full party) + [x] sim-cli models CLASS ADVANCEMENT
      (auto_advance) + pet-wipe deadlock fixed → full-party pacing now real:
      **Lv70 ≈ 13d @24/7 (on target)**, progresses to ~b12, no deadlock.
      TODO: per-2nd-class DPS balance (DPS meter now attributes field/trap/pet), and
      smooth the noisy median-port DEF/MDEF curve (b1 MDEF already fixed 15→0).

## Roster ⇄ Party system  (CORE BUILT 2026-07)
- [~] **Roster (unique-id, dup classes) ⇄ Party (cap 5); bench = no XP.** ✅ CORE
      DONE: `GameState.roster` + `buy_character` (Gil, progressive) / `deploy` /
      `bench` + class_characters→roster migration; bridge + Godot Party UI (key Y).
      Removed the fixed LEAD (party symmetric; `PLAYER_ID`→`FIRST_PLAYER_ID`).
      Party-naming onboarding ("Found Your Party" + `party_name`). Recruit offers
      TIER-1 only (t2 via advancement). TODO (future): real-money buy path, premium
      2-party (needs the Account/Session split — see memory
      [[multi-party-account-session-split]]), lead-swap. Spec: docs/roster-party-system.md.

> **Architecture plans for everything in M1.5 / M1.6:** `docs/unbuilt-systems-architecture.md`
> — shared foundations first (the grant path, the claimable machine, measurement, catalog-as-data),
> then a per-system shape. Read it before starting any of them: four of these systems are the
> same three primitives wearing different names.

## M1.5 — Launch-readiness systems  (P0.5, proposed 2026-07-31)

These are the systems a game that OPENS ONLINE needs, as opposed to the ones a demo needs.
M1-M4 were written from "what must the demo contain"; nothing below appears in them. The one
marked ⛔ should land **before** the next content push, because it produces data that can never
be recovered afterwards.

**Not on this list, by decision:** offline progression. The game is ONLINE 100% — the sim
computes on the player's own PC, but nothing accrues while disconnected (`main.rs` ~176 states
this as the intended behaviour, not a gap). Settled 2026-07-31, superseding the offline half of
docs/craft-and-offline-design.md. Consequence worth keeping: every pacing figure in the tree was
measured under connected play and stays valid — there is no pending change that would invalidate
it.

- [x] ⛔ **Telemetry** — **COMPLETE 2026-08-21 (`549f099`); substrate built 2026-08-03.** The
      client half is `POST /telemetry` + `net/telemetry.gd`, guarded by
      `tools/check_telemetry_keys.py`. Needs a DEPLOY before it records. Original note: `verify-server/src/metrics.rs` (closed
      core: no sqlx, no clock) accumulates counters + `PlayerDay` + per-zone `FirstSeen`, drains
      into `metrics_daily` in Postgres on save, and `/dev/balance` is the read path so the tables
      cannot silently accumulate unread. Save outcomes, the three grant kinds and every rejection
      reason are already counted.
      **Left: the half the server cannot derive.** Everything above is inferred from saves, which
      record state, not intent — so a player who opens the dungeon menu and backs out, quits at a
      wall, or wipes in a specific room is still invisible. Those need CLIENT-emitted events
      (session start/end, dungeon menu opened vs entered, wipe + where, `regent_hand` used) on
      the same endpoint. Same reason as before: that week happens once and is unrecoverable.
      NOT a substitute for watching a real player: it says what, never why.
      **Why before content:** it answers the questions that decide WHICH content is worth
      building — where new players stop, whether the b1→b2 wall (memory
      [[early-game-onboarding-gear]]) is real and how big, how long a Duskmourn run takes for
      a human vs the 105 s sim-cli figure, whether anyone uses a mechanic we just built.
      **Why it can't wait:** saves record the CURRENT state, not the path taken — "reached
      band 5, retreated to 2, quit" is unrecoverable after the fact, and the week that matters
      most (launch) happens once. Key by `account_id` so a deletion request is one DELETE.
      NOT a substitute for watching a real player: it says what, never why.
- [ ] **Quest / objective layer** — not built (design only, memory [[quest-system-design]]:
      Catalog / Measurement / Quest-state). The measurement substrate partly exists already —
      `state::Measures` counts kills, rooms cleared, refines, orbs, card feeds. What is
      missing is the catalog and the per-account state. Turns content into a reason to play
      it; can land alongside content rather than strictly before.
- [ ] **Tutorial / onboarding** — no system, only the phrase in a comment. The known early
      wall is b1→b2 (ATK 8 → 24) and the agreed fix is tutorial + high-stat starter gear, not
      nerfing monsters. This is the point where new players are lost, so it is worth more than
      the 15th zone.
- [x] **Level-cap / band gate cadence** — **DONE 2026-08-18 (`7eef37e`).** `SelectBand` was
      `InputAuthority::Client` and checked only `band >= 1`, so a level-1 party could type 20 and
      stand in the endgame — and replay-verify could not see it, because the sim accepted the
      input. Rule (user): any band up to REACHED + 1.
- [ ] **Content authoring + validation** — adding a dungeon tier today means hand-editing a
      25k-line `game_data.json`. Tuning the 2026-07-31 boss conversion needed a script that
      nudged `*_mult_bp` by 1 until integer division landed on the authored value, and the
      "spawn_pool with target_population 0" mistake was caught by a unit test rather than at
      author time. Worth building when the plan is many tiers / bands 15-20, not before:
      a target-value → multiplier solver, validation at build time, and a tier template.

Adjacent and already tracked, not duplicated here: **save-write throughput** (top of this
file) is a CCU ceiling rather than a content ceiling, and the **gacha purchase path** +
`docs/gacha-arch-review.md` findings are the money-path equivalent of this section.

## M1.6 — Live-ops & monetization systems  (P0.5, proposed 2026-07-31)

The systems a gacha game runs on after launch, listed by the MACHINE they need rather than by
the feature name — four of the requested features turned out to be two machines, and building
them separately means writing "claim once / reset daily / deliver to the player" four times on
paths that hand out value.

- [x] **Schedule + claim machine** — **BUILT (reconciled 2026-08-21).**
      `verify-server/src/schedule.rs`, 822 lines: `TrackDef` / `Window` / `Grant` / `owed()` /
      `reached_mask()` / `retired_tracks()` / `per_account_expiring()`, with `/claim/state` and
      `/claim/collect` routed. It IS server-side data, as §10.2 required. The design below is kept
      because it is the reasoning, not a plan. Original text:
      one server-authored table of "reward or offer, with a
      window and a condition, claimable once". Covers **daily login (7-day newbie ladder + a
      30-day/monthly calendar)**, the **time window of an event**, and later the availability of
      **packs**. They differ only in the condition and the window:
      | | condition | window |
      |---|---|---|
      | 7-day login | Nth day since the account was created | per-account |
      | monthly | Nth day of the calendar month | calendar |
      | event | a mission counter reached | event period |
      | pack | paid for | limited time / limited count |
      Foundations already here: `bp.rs` has seasons/windows, the mailbox delivers exactly-once,
      `gacha_exchanges` is the idem-key + claim pattern to copy. The gacha reference doc already
      requires this to be **server-side DATA, not hardcoded handlers** (§10.2 — "free today, a
      rewrite after the first hardcoded purchase handler ships").
- [ ] **Dispatch (send characters to gather resources)** — the use for the roster members who
      are not in the 5-slot party; right now they idle. Send N characters for a real-time
      duration, claim the yield through the existing mailbox.
      **Shape that fits this architecture:** a SERVER-side wall-clock timer, not sim ticks. The
      game is online 100% (no offline sim), but a dispatch started at 20:00 and collected at
      midnight is just two timestamps — no offline progression is implied, and it matches the
      standing rule that the money economy is real-time only.
      **Guard:** the yield must NOT be Gil or directly sellable goods, or it becomes a faucet
      that runs without playing. Craft materials / upgrade stock keep it a reason to log in.
- [x] **Text box over gameplay** (~0.5) — **BUILT: `views/dialogue_box.gd` (451 lines) +
      `views/dialogue_script.gd` (273).** Original text:
      a Control on a CanvasLayer with a message queue:
      speaker + line + advance, and the sim KEEPS RUNNING under it. Used by tutorial hints, quest
      prompts, event notices and a character speaking mid-fight. Touches no save and no replay.
      **This is what unblocks onboarding and tutorial without waiting on the story**, because a
      first-run hint is not narrative.
- [~] **Story mode** (~3-3.5, SCOPE LOCKED 2026-08-06) — **THE MACHINE IS BUILT, THE PROSE IS
      NOT** (reconciled 2026-08-21): `views/story_view.gd` (636) + `dialogue_box.gd` (451) +
      `dialogue_script.gd` (273) + `story_reward_view.gd` (204), with `/story/claim` and
      `/story/progress` on the server. This row is now tier 2 work — writing — not code, which is
      what the priority list has said since 2026-08-13. Original text:
      reading AND stages, opened as its own
      tab while the farm keeps running. Nothing about it interrupts the player, which is the
      whole point.
      - **Runs on a SNAPSHOT of the real party**, not a preset cast. The story is about the
        player's own party — and Exemplars are PURCHASED characters, so a preset cast would keep
        the thing a player paid for out of the story entirely. It also contradicts the
        party-comp-is-the-build pillar.
      - **No XP, no drops, nothing flows back.** Dying in a story stage does not touch the real
        party; levelling in one is discarded with the snapshot.
      - **An overpowered party simply wins** (user 2026-08-06). No level scaling: the player came
        to read, and scaling away their progress would delete the feeling of having got stronger.
      - **The only durable output is "mission N cleared" + a fixed, once-only reward.** Which is
        why this is cheap: with no XP and no drops, the story sim is **entirely outside the replay
        contract**. The server needs to verify nothing about the fight — only that this mission
        has not been claimed before, the same shape as first-clear rewards.
      - **`StoryDriver`** is a second node shaped like `SimDriver`: its own `SimBridge`, its own
        tick loop, its own `state_updated`. It can never write a save or call the server. The
        story screen listens to it instead of `SimDriver`, and the two sims run side by side
        without knowing about each other. `SimBridge` holds only `state: Option<GameState>` with
        no statics, so a second instance is safe; tick cost is ~7 µs, so two are free.
      - **Amend the project rule.** "There is exactly ONE SimBridge" becomes "exactly one
        AUTHORITATIVE SimBridge; the story sandbox owns a second that can never write to the
        save." The intent — no view ticks the real sim — is unchanged.
      - `Arena` currently binds to `SimDriver`; it needs to take a driver, or the story screen
        needs its own smaller renderer. Counted in the estimate.
- [ ] **Stage chain + first-clear rewards** (~1) — the room/clear-condition half already exists:
      Duskmourn is that shape, on `instance_start_zone_id` / `is_instance_end` / `EnterInstance`.
      Missing is a first-clear flag (`record_band_clear` counts clears but cannot tell the first
      from the rest) and the once-only grant that hangs off it. Story mode and event stages both
      consume this.
      ⚠️ **There IS a `first_clear` table in `main.rs` and it is NOT this** (2026-08-21). It is
      telemetry's `FirstSeen` — `(account_id, zone, level, tick, party)`, written by the metrics
      flush and read by `/dev/balance` to answer "what level do people arrive at band 7 with".
      Nothing grants off it and nothing may: it is aggregate analytics a client can influence,
      not a reward gate. The name is the trap.
      `/story/claim` exists and is once-only, but pays a flat `MISSION_GIL = 1_000` to every
      mission — the per-mission reward table is the data half still missing.
- [ ] **Dialogue scenes — visual-novel presentation** (~1-1.5) — portraits + background + speaker,
      inside the story tab. **Still open, and checked**: `dialogue_box.gd` has no `portrait`
      anywhere in it, so what exists is the text box, not the presentation. **Affection / bond scenes ride the same player**, so it is built once
      for both. Portraits are an ART cost, and คุณอาร์ท / คุณกะหู้ are off the launch path now, so
      the capacity exists. CHOICES are deliberately excluded for launch: a choice is a player
      input, which makes it an `InputEvent` inside the replay contract — the most expensive part
      of the codebase to get wrong.
      **Non-code precondition:** the script. `docs/world-lore.md` is canon but the scenes are not
      written; คุณ Kiky delivers the intro + rough world design first.
- [~] **Micro-transaction path + real-money store** — **"nothing exists" is FALSE as of
      2026-08-21.** `verify-server/src/store.rs` is the money-in half, `/store/checkout` +
      `/store/finalize` + `/store/refund` are routed, `entitlements` and `bp_entitlements` carry
      `revoked_at`, and `views/store_view.gd` (256 lines) is the gem shop with `views/gacha_view.gd`
      as its twin. The money-path audit's five findings against exactly this code are all fixed.
      What is genuinely open is the STEAM side — InitTxn/FinalizeTxn against the real Steam
      backend and the refund webhook — plus F4's rule that no SKU selling a CAPABILITY ships until
      refund revocation is proven, which it now is. Re-read `docs/gacha-arch-review.md` against the
      tree before trusting any other item in it.
- [ ] **Packs / bundles** — products on top of the store (starter, growth, limited). Built from
      the schedule machine above; do not give them their own claim logic. **The schedule machine
      is now DONE**, so this is unblocked (reconciled 2026-08-21).

Suggested order: schedule+claim → dispatch → stage+dialogue → store → packs. The first two are
small and pay retention immediately; the third is the largest and is gated on written story;
the last two are the money path and should follow the review findings.

## M2 — Polish & UX  (P1)
- [~] **Desktop companion mode** — BUILT (key W; 16:9 always-on-top camera widget,
      auto-rotate party 10s; `views/companion_view.gd`, 430 lines). The box is open for the
      FOLLOW-UPS below, not the feature. TODO: on-screen toggle BUTTON (not just hotkey);
      **"director" smart camera** (Dota2-style auto-cam): switch focus on
      situation — boss spawn / rare monster / HP crashing / big kill, scored with
      a min dwell time, 10s rotation as fallback.
- [ ] **Companion-mode calmer hit FX** — the multi-hit pummel flinch + SOLID WHITE
      damage flash (enemy_animator `MONSTER_HURT_DUR` / `HUMANOID_HURT_FLASH` /
      `PUMMEL_PULSE`) reads great in the full game but is too eye-catching /
      distracting in the small always-on-top companion widget. When companion mode
      is active, swap the white blow-out for a SUBTLE/dim blink (lower modulate,
      softer pulse) so the always-on-screen view stays calm.
- [ ] **Death-anim / hit-popup sync** (arena.gd) — the HP bar already lags to
      sync with damage popups (`hp_pending` / `entity_displayed_hp`), but the
      DEATH animation still fires the instant the sim flags `alive=false`
      (arena.gd:256/628), so a one-shot kill snaps to death before the killing
      number + attack anim finish. Fix: gate the death anim on the LAGGED
      `displayed_hp` reaching 0 (reuse `hp_pending`), not the raw sim flag — so
      "hit → number pops → THEN death" reads cleanly. Matters more now with the
      Assassin dual-wield 2-hit basic (two staggered popups per swing). Renderer-
      only; verify in-game (smoke.sh + watch). Pairs with the off-hand staggered
      popup work.
- [ ] **Multi-hit popup timing vs ASPD** — the per-number stagger is a fixed
      0.20s (arena.gd MULTIHIT_STAGGER); a fast attacker swings again before its
      numbers finish, so they pile up. Scale the stagger to the attacker's swing
      interval (e.g. `min(0.20, swing/total_numbers)`) so the whole multi-hit
      display fits inside one attack cycle. attacker `atk_speed_milli` is already
      in view_state per entity; key off recent_hits' `attacker_id`.
- [ ] **Skill tooltips** for 2nd-class (+ existing) skills
- [ ] **Onboarding** — first-time clarity (it's idle, but what do I do?)
- [ ] **Optimize first impression** — tester feedback: first ~1h feels rough
      (BALANCE issue, not design): smooth band-1 survivability / pace so the
      opening hooks before the player settles into idle. Tune later.
- [~] **Click-to-attack (early-game)** — PvE single-player: click a monster for a
      bonus hit (online-safe: strong early, trivial late). PROTOTYPE DONE (commit
      7f5a4ae): flat 10 PURE dmg, recorded sim event, 1/tick rate-limit, 2-cell
      snap. TODO: scale by BASE atk (not flat); late-game Soul Path auto-click.
- [ ] **DPS meter (per class / per skill)** — track + display damage-per-second
      contribution by class and by skill (party breakdown). Aids the player AND
      the balance pass (spot under/over-performing classes/skills). Source the
      numbers from the sim (per-hit damage already flows through the damage path);
      surface as a panel/overlay. Keep deterministic — measure, don't compute in GDScript.
- [ ] **Save migration** robustness — new classes/bands/items don't break saves
- [x] **Language** — **EN-only for the demo** (locked). Keep all drawn text ASCII;
      Thai/Unicode (needs a real font) deferred post-demo.
- [x] **Settings** — volume is wired (`views/options_view.gd`: Master / SFX / Music at the
      AudioServer BUS level, persisted). Resolution/window is still not there and was always the
      "maybe" half.

## M3 — Art & Audio  (P2, parallelizable)
- [ ] Art: characters / monsters / band backgrounds / UI skin
      — **candidate: Duelyst Animated Sprites (CC0, 697 units, 100px frames)** —
        previewed in-game, acceptable (bigger than 1 cell, fine); attack anims
        need ~3x speed for real-time. Format = Godot SpriteFrames (AnimatedSprite2D).
        Decision: adopt (style upgrade, free) vs custom (distinct identity).
      — **artist brief: `docs/SPRITE_SPEC.md`** (clean static head + hat overlay
        on the 100×100 grid; archer bow re-posed off the head; hat mock-ups)
- ~~**Hat cosmetics** — clean-head sheets + hat slot on the character + overlay
      draw in player_animator~~ — **CANCELLED (user, 2026-08-06).** It was never only a
      slot: it needs a CLEAN-HEAD redraw of every class sheet, and the five in-house
      sheets that just landed (swordsman / mage / acolyte / thief / archer) are all
      drawn with hair and headwear as part of the character. Cutting hats back in means
      re-commissioning the set, not adding a layer.
      ⚠️ `docs/store-starter-and-qol-skus.md` §1 lists "One cosmetic (hat)" in the
      proposed $1.99 starter pack and §2 prices cosmetics at $1.99-4.99. That pack now
      has nothing cosmetic to give — pick a different sweetener before it ships, or the
      SKU is short one line of its contents.
- [ ] **ASPD-scaled attack anim + recoil timing** (do with the real sprite set) —
      currently attack CADENCE scales with ASPD (atk_acc ramp) but the clip
      PLAYBACK speed is FIXED (slime: MONSTER_ANTICIPATE_TIME/HIT_HOLD/LUNGE_RECOVER
      constants; humanoid: HUMANOID_ANIM fps). At high ASPD the fixed clip can't fit
      the swing interval → recoil/windup overlaps or holds the end frame (graceful
      but not intended). Fix when real art lands: derive fps/clip-duration from the
      swing interval (e.g. effective_fps = frames/(swing_interval − windup)) so the
      windup→impact→recoil scales with ASPD and always finishes before the next
      anticipate. Also: Ranger pet currently renders as a placeholder slime sprite
      (enemy.draw_monster) → swap to real pet/companion art.
- [ ] Audio: SFX (attack/hit/skill/UI) + music
- [x] Volume controls wired into settings — `options_view.gd`, three buses, persisted
- [x] **"Wow" VFX direction — HD-2D is ADOPTED (user, 2026-08-16).** This entry used to
      park it on two guesses, both made before anyone had built one. The prototype
      (`godot/proto/arena3d/`) answered them:
      - "TOO heavy" — it runs on the EXISTING `gl_compatibility` renderer. No renderer
        switch, so the mobile plan is untouched, and companion mode held 4 panes on an M3.
      - "friction vs renderer-read-only" — none. The prototype computes no gameplay; the
        2026-08-16 audit confirmed nothing outside SimDriver touches the bridge.

      **The reason it is worth it (user): this is a GACHA game, and a gacha game sells on
      presentation.** Everything has to look pretty; the 2D-juice middle ground this entry
      proposed does not buy that, it buys a louder version of the same picture.

      **What is still NOT measured, and should not be claimed:** sustained GPU and BATTERY
      on an always-on widget over hours — the one thing the parking was actually worried
      about. The numbers taken on 2026-08-13 were invalid (`ioreg` reports whole-machine
      GPU). Full-screen was ~30 fps against ~90 windowed on a Retina panel — fill-rate,
      never resolved, and the HiDPI lever is still untouched. Nothing has run on a phone.

      Remaining work is tracked as its own section, not here. Zone art exists for **3 of
      20 regions**, which is what decides whether the look actually lands in play.

      Seraphine VFX folder map, accepted previews, integration boundaries, and capture
      commands: [VFX handoff](docs/vfx/README.md).

## ⚠️ TEMPORARY: every Regalia / Exemplar is priced at 10 Gil (user 2026-08-03, for testing)

All 26 overlay carriers carry `buy_price: 10` in `data/game_data.json` so they show up in the Gil
shop and can be equipped, enhanced and resonated by hand. **This is a test fixture, not pricing.**

Before anything ships, decide per rarity what is Gil-buyable at all — the intent from the design
pass is that **R is the free/starter tier** (shop + the post-region-1 grant) while **SR and SSR are
gacha-only**, i.e. `buy_price: 0`. The two heroes that already existed were repriced from 50000
down to 10 and need that number back or a decision that replaces it.

The PROVISIONS were repriced the same way on 2026-08-04 so the screen could be exercised
through the normal buy flow instead of a debug grant: `loot_field_ration` 150 -> 10, and
`aspd_pot_1/2/3` 80/200/450 -> 10/20/40. They were already purchasable in the Potions tab; only
the price was in the way. Food is a legitimate permanent Gil sink, so these want a real number
rather than a removal — and once a craft recipe exists, a price that undercuts crafting would
make the recipe pointless.

Also unresolved: `exemplar_seraphine` / `exemplar_lucielle` are `marketable: true` +
`quota_class: "tradeable"`, which contradicts Exemplars being account-bound. The 24 new defs were
authored `marketable: false` + `untradeable`; the two old ones were left alone rather than
silently changed.

## 🔨 Server-rolled craft — DONE 0.0.254 (2026-08-02)

Crafted gear rolls **1★ or better** (72 / 24 / 3.96 / 0.04) and the roll is the SERVER's,
because 4★ is Legendary. A gear recipe accrues a `CandidateKind::Craft` candidate that
`GrantCraft` resolves from a keyed nonce; potions/ores still grant instantly. Rules and the
reasoning behind the separate nonce domain: `docs/server-roll-drops-design.md`.

Goldens did not move — a `Drop` candidate serializes exactly as before the `kind` field
existed. NOT yet exercised online: needs a deploy (server is on 0.0.253) and a play test.

### Also still open from the same conversation

- **Blacksmith vs apothecary** — a `station` field on `RecipeDef` + tabs in `craft_view`. Data +
  UI only, no VERSION bump (recipes live in `GameData`, outside `GameState`).
- **Dark-element consumables** the user asked for — attack buff, resistance to dark monsters, a
  melee element coating. NONE are expressible: `BuffKind` has no element-aware variant and
  `LoadAmmo` is archer-only. Needs its own sim slice.
- **Recipe names are not localised** — `craft_view` renders `r.name` raw, so all 17 recipes show
  untranslated. Pre-existing, affects every recipe, worth its own change.

## 🎴 Card rate + the live knob 0.0.256 removed (user 2026-08-03: keep 1/200,000, defer scaling)

**Rate DECIDED: leave cards at 1/200,000.** Nothing to build — that is already the live value
and 0.0.256 preserves it exactly (`stage2_preserves_every_authored_final_rate`). Recorded so
nobody "fixes" it later without reading the numbers below.

What 1/200,000 actually means, measured:

| | |
|---|---|
| solo, farming one monster (~1,470 kills/h) | **136 hours** per card |
| RO's real card rate (0.01%) | 20x more generous |
| global supply at 100 CCU | 0.7 cards/h — thin |
| global supply at 500 CCU | 3.7 cards/h — a market works |

So cards are deliberately a MARKET item, not something a player farms for themselves — which is
how RO works too. The number is right for a mature market; it is not right for launch day, when
there is neither a market nor the CCU, and 19 card defs with 18 specials, stars and foil sit
unreachable. That is the gap crowd-scaling was meant to close.

### The live rate knob — RESTORED 2026-08-03

**DONE 2026-08-03** — `RARE_RATE_MULT_BP` (verify-server env, basis points, default 10000 = x1,
clamped to [x0.01, x50]). The sim keeps the deterministic BASE rate; the server scales it at
issuance. Deploying it unset changes nothing.

Replay-safe for free: grants are server-authored and `drop_grants_valid` only checks the nonce
is ours, so the multiplier never has to agree with anything the client replayed.

**The scaling POLICY is still not built** — nothing counts active farmers or moves the dial.
Today it is a manual knob: set it, restart, done. Launch generous (x5-x10) while CCU is low,
tighten toward x1 as the market fills.

## 🏁 Competitive events — what each one costs in integrity (user 2026-07-31, NOT building yet)

Three planned event shapes, each with a different metric — and the metric is what decides how
much anti-cheat work the mode needs. Layers 1 and 2 are already live
(`docs/competition-integrity.md`); this is only about what is left PER MODE.

| mode | metric | what it still needs |
|---|---|---|
| **level race** | real time | event-window excess counter + rank cap — the real work |
| **speed clear** | ticks inside the instance | a durable best-time; NO anti-cheat work at all |
| **PvP / co-op** | server-produced result | verified per-team aggregation, a different layer |

- [ ] **Level race** — the only mode that needs the event-window variant. Its metric IS real
      time, so a fast-forward buys rank directly. Needs a per-`(account, event)` excess counter
      **starting at 0 for everyone — no `EXCESS_FLOOR`** (the −3 000 floor is right for a
      lifetime counter, but inside a one-week window it just hands long-time players 5 min of
      free cheating), plus ranking on progress per CONNECTED real-second with the counted rate
      capped at 1.0×. The cap matters more than the detection: it makes cheating worthless
      without having to catch it.
- [ ] **Speed clear — the cheapest way into competitive.** Ranking by ticks-in-instance is
      **immune to fast-forward by construction**: running the sim at 10× compresses real time
      but the tick count is identical, so there is nothing to detect. The measure already exists
      and is already trustworthy — `ClearSummary.ticks` (`sim-core/src/state.rs`, "Ticks from
      entering room 1 to the clear") lives in `GameState`, so it is inside the state hash and
      covered by replay-verify. The other threats are covered too: forged gear by
      replay + trusted anchor, save-scumming a lucky run by the `pins` chain (a diverging
      segment is rejected as a rollback/rewrite).
      **The one gap:** `last_clear` is a transient `Option` that clears when the summary screen
      is dismissed, so the server only sees a run if a 10 s autosave happens to land while it is
      up. A leaderboard needs a DURABLE per-instance best time in `GameState` → touches sim-core
      + a `VERSION` bump. That is the whole job.
- [ ] **PvP / co-op (see [[pvpve-boss-race]])** — "result from the server" has a specific
      meaning in this architecture: the server does not run a sim, it runs REPLAY, so the result
      must be DERIVED from verified state, never reported by the client. For a shared boss
      (4 teams × 5 on one target) each client simulates its own world, so contribution has to be
      a verified per-team number the server aggregates — which bounds resolution to the **10 s
      save cadence** (the boss can die up to ~10 s later than it "really" did). Decide whether
      that is acceptable before designing the mode around instant feedback.

## Online security / anti-cheat architecture (lock these rules for the online phase)
The defence is NOT code secrecy (Godot .pck unpacks trivially; gdsdecomp reads
GDScript back to source) — it's SERVER AUTHORITY. A fully-leaked client still can't
cheat the economy. Rules:
- [x] **Logic in Rust only** — gameplay logic stays in `sim-core` (compiled native
      cdylib, far harder to reverse than GDScript). GDScript = presentation only,
      NEVER gameplay outcomes. (This is already the architecture — keep it.)
- [x] **Server re-simulation** — save = `(seed + initial state + Vec<InputEvent>)`;
      server replays the same inputs and compares. Edited client state/outcomes
      won't match → caught. Spot-check sampling + ALWAYS-validate high-stakes
      (trades, rare drops, refine).
- [x] **Server-authoritative RNG for economy-critical rolls** — rare/tradeable loot,
      gacha, refine odds roll on the SERVER (client can't predict/seed-scum/save-scum).
      Cards 0.0.205-207, refine deployed, `/gacha/roll` and `/refine/roll` are the paths.
      CRITICAL determinism rule: a server roll must be **a separate RNG channel
      injected as a recorded INPUT at a fixed tick point** — it must NEVER draw from
      the main PCG32 stream (one shared PRNG → any added/removed/reordered draw
      shifts EVERY downstream outcome = butterfly). Read it as data, don't draw it.
      Latency: pause resolution of THAT event until the value arrives, stamp at its
      logical tick (don't freeze the whole sim).
- [x] **P3 MUST-FIX — bind refine nonce to `enriched` + `seal`** — DONE (C06; verified
      2026-08-13: `RefineRollReq` carries both fields and `refine_nonce` hashes them under
      `refine-roll:v2`, so the client must COMMIT them at roll-request time). Was: (audit 2026-07-24, sim
      0.0.233). `StartRefine`/`StartRefineInventory` carry `enriched: bool` + `seal:
      RefineSeal` that CHANGE the outcome (enriched odds / Aegis keeps level), but the
      server-roll `refine_nonce` + `verify_refine_binding` (verify-server main.rs ~1237)
      ignore them. → peek-and-select reopens: client fetches nonce, sees `r=N%10000`,
      THEN picks enriched/seal to convert a fail→success or dodge a level-loss;
      undetectable (nonce valid, hash matches, consume-all satisfied). FIX = add both
      fields to `RefineRollReq` + the `refine_nonce` hash input + pass through
      `verify_refine_binding`, so the client must COMMIT enriched/seal at roll-request
      time (mismatch → BadNonce). Harmless on dev (no money enforcement); gates real
      tradeable refines. (Reforge bound-laundering — the audit's other HIGH — already
      fixed in 0.0.233.)
- [x] **Sensitive DATA server-side** — **DONE (verified 2026-08-21).** `data/game_data.json`
      contains no `rare`, no `drop_chance` and no `chance_bp` at all; the rates live on the server
      with `RARE_RATE_MULT_BP` as its live knob. Nothing is left to move.
- [~] **Harden the release dylib** — `lto = "thin"` and `panic = "abort"` are set in
      `Cargo.toml`; **`strip` is NOT** (checked 2026-08-21). That one line is the whole remainder.
      Raises the reverse bar; not a substitute for server authority.
- [x] **Serialize outcome-affecting serde-skip fields before online** — done twice, and the
      second time is why it is now checked instead of remembered. 0.0.196 serialized ~9 `Entity`
      combat fields; **0.0.261 found the same hole reopened** on four `Projectile` transients the
      sim reads at impact (`is_basic` / `burn_tier` / `focus_mult_bp` / `pin_slow_pct`), each
      added later with a comment repeating the same false reasoning — "a reload mid-flight just
      drops it for that one shot", true for a player reloading, false for replay-verify. It was
      rejecting real saves with an EMPTY input log, which points at nothing.
      Now structural: `a_round_trip_loses_only_allowlisted_fields` requires a round-tripped state
      to come back EQUAL, forgiving one named allowlist, so a NEW `serde(skip)` fails until
      someone answers whether `tick()` reads it. The old fuzz could not do this — it sprinkles a
      hand-written list of ENTITY fields and never touches `projectiles`.
      Residual accepted risks: private/cracked offline servers + botting (universal; botting
      needs behavioural/rate detection).

## M4 — Release  (P3)
- [x] **REMOVE the debug shortcut in the Dungeon menu** — DONE (verified 2026-08-13: no
      `TEST_DUNGEONS`, `Throne Room` or `duskmourn_4` left anywhere in `godot/`). Was: `TEST_DUNGEONS` in
      `godot/views/dungeon_menu_view.gd` (the `[TEST] Throne Room` card) warps straight to
      `zone_dungeon_duskmourn_4` so the `regent_hand` pick-up can be tried without clearing
      rooms 1-3. It carries RAW English labels on purpose (kept out of `strings.csv`), so it
      would ship as untranslated text. Delete the const + its loop in `_refresh()`; the
      localised `DUNGEONS` entry stays. (Added 0.0.244 — the skill is granted per-zone by
      `ZoneDef.player_skills`, so nothing else needs touching.)
- [ ] **macOS build** — `godot/export_presets.cfg` holds exactly one preset, "Windows Desktop"
      (checked 2026-08-21). Needs a macOS preset + ad-hoc sign.
- [ ] **QA loop** — tester feedback (started); crash/error handling
- [ ] **Distribution** — itch.io (simple) or Steam page (per Market vision)

---

## Open decisions to lock
1. ~~Band count to Lv70~~ — **RESOLVED: extended to b14 (Lv70), 3 rooms/band.**
2. ~~2nd-class scope~~ — **LOCKED: first branch only (5 t2) for the demo; 2nd branch is post-demo.**
3. **2nd-class skill names** — original, or keep RO-style (Knight/Wizard…)?
4. ~~Language — EN-only for the demo~~ — **SUPERSEDED 2026-08-03.** The i18n layer is built
   and populated: `godot/i18n/strings.csv` carries **1,096 keys × 18 locale columns**, read at
   runtime by `godot/loc.gd` (imported `keep`, so the CSV is the single source — no generated
   `.translation` resources). Target tier 1 is EN / TH / zh-CN / zh-TW (memory
   [[mass-100k-strategy]]). What is left is TRANSLATING and finishing the `views/*.gd`
   migration to `Loc.*`, not deciding whether to localise.
5. ~~Distribution target — itch.io vs Steam~~ — **RESOLVED: Steam.** App ID **4958230** is
   registered (memory [[steam-app-id]]); the launch thesis depends on Steam CCU / Most Played
   discovery (memory [[idle-ccu-flywheel]]). macOS + Windows exports are tracked in M4.

## References (memory)
- demo-goal-and-2nd-classes · loot-items-and-autoretreat · endgame-economy-vision
- steam-market-banana-vision · rare-loot-bag-design · refine-ore-economy
