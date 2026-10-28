# Unity handoff — gear families, store packs, onboarding (2026-09-30)

What the client has to build for the server/sim work of 2026-09-29/30. Each item says whether the
backend half is BUILT (keys exist today) or PLANNED (decided, not built — do not wire yet).

## 1. Any weapon / off-hand on anyone — BUILT (sim 0.0.490)

User: every weapon and off-hand fits every character; a piece outside the character's family gives
80% of its OWN ATK/DEF (substats untouched). The sim no longer refuses the equip.

- Stop hiding or greying off-family pieces on the equip screen; the equip always succeeds.
- **Yellow frame** on an equipped piece worn off-family.
- **Hover** on it: which weapons (and off-hand kind) this character prefers.
- View keys (`sim-host`):
  - `entities[].equipped[].off_family` — bool.
  - `entities[].preferred_weapons` — e.g. `["Staff"]`; `entities[].preferred_offhands` — e.g. `["Tome"]`.
  - item views keep `wieldable` (unit ids); it now means "who PREFERS this", not "who can equip".
- Loc rows for the tooltip line and weapon/off-hand type names if they are missing.

## 2. Store: packs, monthly card, battle pass premium — BUILT (server, not deployed)

- `GET /store` rows gain `section` (`crystal` | `pack` | `monthly` | `battlepass`), `bound_crystal`,
  `coupons`, `items[{def_id, qty, name}]`, `limit`, `commander_level`, `monthly_days`,
  `monthly_daily_crystal`.
- `GET /store/me` (auth): `skus[{sku, bought, buyable, reason}]`,
  `monthly{active, days_left, claimable_today, daily_crystal}`, `battle_pass_premium`.
- `POST /monthly/claim` (auth): today's 150 bound crystal; 409 = nothing to claim today.
- `/bp/state` gains `premium_track`, `premium`, `claimed_premium`; `/bp/claim` takes `"track": 1`
  for the premium track (403 without the pass).
- Checkout refusals (409 + `reason`): `purchase limit reached`, `commander level too low`,
  `monthly card already banked`, `no active season`, `battle pass already owned`.
- Shelf the user asked for:
  - **Starter** $0.99 = 10 pulls, once.
  - **Growth** packs at Commander 10 / 15 / 20 / 25 / 30 ($1.99 / 4.99 / 9.99 / 19.99 / 29.99).
    Show ALL tiers; a locked tier shows its unlock level, an unlocked tier stays buyable until bought.
  - Commander CXP now comes from each daily-ladder rung claimed (200, with the crystal;
    `/daily/claim` returns `cxp`) and each stage reached for the first time (50, credited on save).
  - **The Outpost is removed**: `/commander/outpost/claim` is gone (404) and `/commander/state` no
    longer sends `outpost_preview` / `outpost_cap`. Remove the Outpost UI from `CommanderWindow.cs`,
    `HudBar.cs` and the call in `ServerClient.cs`.
  - **Monthly card** $2.99: a daily claim button while active.
  - **Battle Pass** $9.99: the premium row on the pass screen.
- Pack items arrive BOUND through the mailbox.

## 3. Gacha banners — BUILT (server, not deployed)

See `docs/gacha-economy-design.md` "Banner flow".

- `/gacha/state`: each banner gains `open` (Standard is false until the Starter 50 are spent) and
  `mileage_kind` (`limited` | `standard`); top level gains `mileage_std`, `exchange_pool_std`
  (Memory Fragments), `standard_made`, `standard_select_at` (100), `standard_selected` (null or a
  def id), `standard_select_pool`.
- `/gacha/roll` on Standard before the Starter is spent: 409 `finish the starter banner first`.
  Its response gains `mileage_kind`.
- `POST /gacha/exchange` takes `"currency": "limited" | "standard"` (default `limited`).
- **Two ticket kinds, no universal one**: Standard tickets (Starter + Standard banners) and Limited
  tickets (Limited + Weapon banners). `/gacha/state.coupons_std` / `coupons` hold each, and every
  banner row carries `ticket_kind` + `tickets` (the balance that banner spends). `SummonWindow.cs`
  shows `st.coupons` for every banner today — it must show the banner's own `tickets`. The 402
  quote buys the banner's own ticket kind. Bound kinds are `gacha_coupon_std` and
  `gacha_coupon_limited`; the old `gacha_coupon` is deleted server-side, so `GameText.cs`'s
  `"gacha_coupon" => "Coupon"` needs names for the two new kinds. Standard tickets come from the newcomer daily track,
  party slots 3-5, the Starter pack and Sealed Caches.
- `POST /gacha/standard-select {def_id}`: 409 `not enough standard pulls` / `already selected`;
  a repeat of the same pick is `idempotent`. The unit arrives as mail.

- **Starter banner** (today's Newcomer): 50 pulls, SSR guaranteed within 50 counted from the last
  SSR. Every SR is a NEW SR character. Standard opens only after the 50.
- **Standard banner**: after 100 pulls, a one-time "choose your SSR" selector.
- **Two mileage currencies**: Limited mileage buys any character; Starter/Standard mileage buys
  Memory Fragments only.

## 4. Onboarding flow

BUILT (sim + server, not deployed):
- **Party slots open by progress**, not Gil: slots 2/3/4/5 on reaching stage 2 / 6 / 18 / 24
  (slot 2 = after stage 1's boss in room 1-3). `BuyPartySlot` is now a no-op — `UI/PartySlotUnlock.cs` must stop sending it
  and drop the buy button.
- `party_meta.slot_next`: the stage that opens the next slot ("reach stage N"), -1 = all open.
  Show it on the locked slot. `slot_cost` is always -1.
- Rewards when the slot opens: slot 2 = **Lumi** (mail), slots 3-5 = 10 Standard tickets each
  (`slot_reward_tickets`).
- Slot 2 is NOT a gacha moment (user 2026-09-30): when the `party slot 2` mail lands, claim it
  automatically, show "Lumi joined", then run the tutorial step that walks the player to the Party
  screen and has THEM deploy her into the empty slot (she arrives benched in the roster).
- **Start with Suzu**: the server already accepts a `suzu` genesis — `TitleScreen` should found
  `"suzu"` instead of `"seraphine"`.

TUTORIAL QUESTS — BUILT (server, not deployed). Plan: `docs/tutorial-onboarding-design.md`.
- `GET /tutorial/state` → `open_chapter` and `quests[{id, chapter, done, claimed, claimable,
  rewards[{kind: tickets|cxp|gil|item|lead_weapon, ...}]}]`. Chapters after `open_chapter` are
  locked — show them greyed or hide them.
- `POST /tutorial/claim {id}` → 200 with the rewards, or 409 `not done yet` / `already claimed`.
  Gil and items (the Rare weapon, gems, Sigils) arrive as mail — claim it for the player.
- Chapter 1 = the forced tutorial (first_boss → equip the weapon, deploy_second, first_pull,
  deploy_third, equip_regalia): the client locks the UI and points at each step, then claims the
  quest when `claimable`. Chapters 2-4 are a self-paced quest panel with a "Go" button per quest
  and one `DialogueBox` hint the first time.
- Quest ids need loc rows (title + hint), e.g. `UI_TUTORIAL_Q_<id>`.

## 5. EXP Elixir + crystal shop — BUILT (sim + server, not deployed)

- New item `exp_elixir` (catalog num 633, loc `ITEM_exp_elixir` / `ITEM_LORE_exp_elixir`): a Party
  provision, +20% kill EXP for 1 h (buff kind `XpPct`). Load it through the existing provision
  rack (`AddPartyProvision`); the rack already refuses a second copy of the same item.
- `GET /shop` → `items[{def_id, name, crystal, daily_limit, bought_today, left_today}]`
  (`bought_today` only with a token).
- `POST /shop/buy {def_id, qty, idem_key}` → `{ok, def_id, qty, crystal, left_today}`; spends bound
  crystal first; items arrive as BOUND mail — claim it for the player. 409 `daily limit` (refused
  whole, never trimmed), 402 `insufficient crystal`, 400 `bad qty`; the same `idem_key` again =
  `idempotent`, no charge.
- User: buyable from BOTH the Provision screen (a "Buy" next to the rack) and the Store, several
  per purchase. 120 crystal each, 10 a day.
- Tutorial quests gain `load_provision` (Ch.2, after `equip_gear`, which now pays Elixir ×3) and
  `first_story` (Ch.2, the first Story mission cleared). Loc rows `UI_TUTORIAL_Q_<id>` for both.

## 6. Crystal `tradeable` → `paid` — BUILT (server, not deployed)

User 2026-10-01: crystal no longer trade, so the paid bucket is named `paid`. Wallet JSON now sends
`paid` AND the old `tradeable` (same value) on `/gacha/state`, `/gacha/roll`, `/store/finalize`
and the admin grant; `/dev/gacha-crystal` accepts either key. Switch `SummonWindow.cs`,
`CrystalWindow.cs`, `HomeWalletBar.cs` and `ServerClient.cs` to `paid`; the `tradeable` key is
removed once the client ships without it. Show the two as "paid" / "free" crystal.

## 7. Battle Pass season content — BUILT (server, not deployed)

Plan: `docs/battle-pass-design.md` (top box). 42-day seasons, 50 slots.
- `/bp/state` track cells gain `reward`: `item` | `standard_ticket` | `limited_ticket` | `crystal` |
  `gil`. For the non-item ones `def_id` is `gacha_coupon_std` / `gacha_coupon_limited` / `crystal`
  / `gil` and `kind` is `Currency` — draw the ticket / crystal / Gil icon and a localized name, not
  a catalog lookup.
- `/bp/claim` → `mails` (items and Gil, claim them for the player) plus `granted[{slot, kind, qty}]`
  for tickets and crystal, which are credited directly (refresh the wallet / ticket counts).
- `/weekly/claim` returns `xp` (90 pass XP per rung; the daily is now 35) — show it like the daily's.

## 8. Red dots — data BUILT (server + sim-host), drawing is yours

User 2026-10-01: Story, Companion, Provision and Crest get their own teaching; everything else is
taught by red dots, including level-up stat points, new items and a full bag.

**Server — `GET /badges`** (auth, read-only; each number is what that claim would pay now):
`daily_rungs`, `weekly_rungs`, `tracks` (schedule cells, `/claim/state`), `bp_free`, `bp_premium`
(0 without the pass), `mail` (unclaimed), `commander_levels` (pending level rewards), `monthly`
(bool, today's card claim), `tutorial` (claimable quests), `standard_select` (bool, the 100-pull
pick is open), `tickets_std`, `tickets_limited` (held — a dot on Summon while > 0).
Poll it on login, after any claim / save response, and when a window opens; no tight loop.

**Sim view — `badges`** (every tick, from the save):
- `stat_points[{id, core, apex}]` — fielded members with unspent core / Apex points.
- `bag_free`, `bag_full` — inventory room against `inventory_cap`.
- `resonance[unit_id]` — units a banked Resonance credit can raise now (`FeedExemplarUnit`).
Needs the new sim-ffi dylib (`tools/unity_dylib.sh`).

**Client-only:**
- **New items:** remember the item ids the player has SEEN locally (per account) and dot the bag tab
  and the item until hovered / opened. This is display state, not sim state.
- Propagate a dot up to its parent button (item → bag tab → HUD bag button), clear it when the
  player opens that screen (or claims), never compute a gameplay number in C#.
- Story: dot the Story button when the next mission is unlocked client-side and not yet cleared
  (`/story/progress`).

## 9. Craft held back, Meals cut, Crest material sources — BUILT (sim + catalog + server)

User 2026-10-02.
- **Craft off:** `QueryJson("features")` now returns `{"cards": false, "craft": false}` — hide the
  craft / recipe screens and their buttons. `LearnRecipe` / `CraftItem` do nothing while it is off.
- **Meals gone:** the eight Meals and their ingredients left the catalog (a save holding one loses
  it on load). Drop Meal UI from the per-member Provision panel (the party rack — EXP Elixir — stays).
  Growth packs 10 / 15 now carry EXP Elixir ×5 / ×10 instead of Commander's Feast.
- **Crest material source:** on the Crest page, hover a material (or the purse) → a popup from
  `MAT_SOURCE_<category>` with the tier as `{0}` (e.g. `MAT_SOURCE_armor` + 5 → "Disenchant a tier
  5 or higher helmet, armor, gloves or shoes.").
- **Crest material envelopes:** `crest_mat_<category>_t3` items arrive by mail (tutorial quest
  `disenchant`) and become Crest material on claim — they never show in the bag.
- **Tutorial quests:** `crest_level` (Ch.4, raise any Crest once) and `companion_mode` (Ch.2): the
  client claims `companion_mode` itself after the player first opens Companion mode — it never
  shows `done` before that. Loc `UI_TUTORIAL_Q_<id>` for both.

## 10. Offline EXP — BUILT (sim + server)

User 2026-10-02: EXP only, 80% of the online rate at the latest band reached, at most 10 hours, no
EXP Elixir; the cap refills every time the player comes back.
- When a save lands ≥10 min after the previous one, the save response's `drops` carries a mail with
  an `offline_exp` item: `quantity` = EXP **per fielded member**, `reason` = `"offline <minutes>m"`
  (minutes capped at 600). Claim it like the other drops (it never enters the bag; every fielded
  member gains the EXP).
- Show a "While you were away" popup with the minutes and EXP, but only for a gap of 30 min or
  more; shorter gaps claim silently. Loc `ITEM_offline_exp` exists; popup strings are yours.

## 11. Audit fixes that touch the client (2026-10-02)

- **Release gate (your file):** `tools/release_check.sh` treats the literal
  `UI_TITLE_LOCAL_SESSION_DEV` in a player build as a Development Build, but `TitleScreen.cs:298`
  builds the `_dev` button in a RUNTIME `if (DevDoors)` branch — the string stays in IL / IL2CPP
  metadata of every build, so the gate fails every release build. Put the whole `_dev` button
  block (creation + handlers) under `#if UNITY_EDITOR || DEVELOPMENT_BUILD`, then run
  `tools/release_check.sh` on a release build to confirm it passes.
- **Stage picker:** `band_select_max` is now `max(reached, standing, cleared + 1)` — a band is
  selectable once the band before it has been CLEARED (boss killed), not merely entered.
  `EnterInstance` on a `zone_b<N>_1` takes the same gate.
- **Regalia:** equipping a card another member wears moves it; the old wearer's overlay clears.
- **Store:** a second checkout of a one-per-account pack (or a second Battle Pass for the same
  season) while one is in flight returns 409 `a purchase of this pack is already in progress`.
- **Offline EXP:** the envelope is now the five-member share for everyone, whatever the party size.
- **Store refunds:** a refunded one-per-account pack is NOT sold again (`purchase limit reached`),
  and a refunded Battle Pass is not sold again for that season. A Battle Pass order whose season
  has ended is refused at `/store/finalize` with 409 `that season has ended` (no charge).
- **No season → no Battle Pass on the shelf:** `GET /store` omits `battle_pass` when no season is
  running or it has ended, and `/store/me` reports it `buyable: false` / `no active season` — draw
  no buy button for it.
- **Refund debt:** a refund now takes back the WHOLE crystal grant, so a bucket can go NEGATIVE
  (`/gacha/state.paid` / `bound` may be "-60"). While it is, every crystal spend answers 409
  `crystal balance is negative` (with `"debt": true` on a gacha roll; was 402 — see §17.2) — show the debt and point to the crystal shop.
- **Tutorial `equip_gear`:** now "a fielded hero wears 3 pieces" (read from the save), not three
  equip actions.
- **VFX (from the audit, yours):** `FeedSeraphineSkills` should also require `FxEnabled`
  (`ArenaView.cs:3094`); `ArenaView.Seraphine.cs:56` fires a cast only when
  `last_fired == view.tick`, so a view that spans several ticks after a hitch drops it — track the
  last tick handled per caster. Iron Chains' per-frame mesh rebuild waits on the user's decision.

## 12. Audit, second pass (2026-10-02) — client work, in priority order

1. **CRITICAL — overlapping saves.** `SimDriver` calls `ExportLog()` every 10 s even while a
   `/save` is in flight; `SessionBoot` drops that bundle, but the export had already replaced the
   pending anchor, so the first push's `CommitAnchor()` anchored on a state the server never saw
   and every later save was refused + flagged. **sim-host now re-sends the SAME bundle while a
   push is unconfirmed** and the server answers a re-send of the latest save as `ok`
   (`reason: "idempotent"`), so this no longer corrupts the anchor. Still: don't export while
   `_pushing`, and give `PutSave` a timeout (`ServerClient.cs:286` has none).
2. **Only the latest save is a base now.** A bundle starting from an EARLIER confirmed save gets
   409 `start is an earlier save, not the latest — reload` (not flagged). Treat it like a
   superseded session: reload from `GET /save`.
3. **Gacha double press** (`SummonWindow.cs:234-243`): a plain pull makes a fresh GUID per press,
   so a double-click on x10 sends two charged rolls. Disable the pull buttons until the callback.
4. **Fusion rolls** — `/fuse/roll` now refuses (409) a second set of pieces for the same attempt,
   a later attempt while an earlier roll is unused, and more than 4 open rolls. On a stale draft
   keep the issued (key, attempt, nonce) and re-offer it; retry after the next save on
   `not settled yet`. (`ForgeModel.cs:204-213` drops it today.)
5. **Release gate:** move the `_dev` title button code under `#if UNITY_EDITOR || DEVELOPMENT_BUILD`
   (section 11).
6. **Companion CPU:** `FloatingInventoryHost` polls `QueryJson("inventory")` + a second `ViewJson()`
   and a full `JObject.Parse` every 0.4 s even when closed — feed it from `StateUpdated`, or skip
   it in companion low power.
7. Lower: dead `graded_matk` reads (ExemplarAutoEquip, FloatingItemDescription, ItemTooltip
   "MATK", ExemplarGearDetails); `SummonWindow` literals ("Short … coupons", "refused: ",
   "Excavate ×N") → Loc keys; `unity_probes.sh` usage still names ApexProbe.

## 13. Whole-project audit (2026-10-03) — client work

Server/sim fixes are in (see the commit). Client items, priority order:
1. **A plain-text error body kills saving for the session.** `ServerClient.cs:294` runs
   `JsonUtility.FromJson<SaveResp>` on any body; the 429 / 503 / 504 load-shed answers are plain
   text, the parse throws, `SessionBoot.Push` dies before `_pushing = false`, and no save is sent
   again. try/catch the parse (null resp) and reset `_pushing` in `finally` (Push + PushAndReport).
   Note the server now also answers 429 `a save is already in progress` (one save per account).
2. **No request timeout** anywhere but /fuse/roll (`ServerClient.Post/Get`): set `req.timeout = 30`.
3. **Companion mode hides every session notice.** `SessionNotice` finds `HudBar` with
   `FindFirstObjectByType` (skips inactive); Companion deactivates HudBar. Also `Superseded()` must
   set `Online = false` and stop the driver (the next Push NREs on a null client).
4. **Mail claimed twice** (MailWindow `_applied` is per window; Summon/Pass collect apply ClaimMail
   too). The sim now ignores a second ClaimMail for the same mail id, but keep one applied-id set.
5. **Claim reports success when nothing landed** (bag full): check the result before marking it.
6. **9 full view parses per publish, 10 Hz, even hidden** (`ArenaView.OnView` fan-out; CombatBar /
   MemberTabs / HomeMenuLayout ask the FFI for a fresh view): parse once, skip inactive handlers,
   reuse the last published JSON.
7. **Trade window** (G key, Home → Store) talks to removed routes — remove it.
8. Save responses can now be 409 `another save landed first — reload` or `sim build skew` — reload.

## 14. Re-audit (2026-10-03) — client work, in priority order

Server and sim fixes from the same pass are in `sim_core` 0.0.491's changelog. What the client owns:

1. **HIGH — the save response's grants are never applied.** Unity reads `ok` / `reason` from
   `PUT /save` and drops the rest. Port `godot/sim_driver_core.gd:495-566` into the save-response
   handler, each applied as a recorded input AFTER the anchor commit so it rides the next save:
   - `grants[]` → `{"GrantDrop": {...}}` per entry (server-rolled rare drops);
   - `craft_grants[]` → `GrantCraft`, `gear_grants[]` → `GrantGear`;
   - `resolved_seq` (> 0) → `{"PruneCandidates": {"up_to_seq": n}}`;
   - `settled_ops[]` (non-empty) → `{"PruneMarketOps": {"ids": [...]}}`.
   Without the last one `market_ops` grows forever and every save re-sends every mail claim.
   Without the first three, a player's server-rolled drops never arrive.
2. **409 with "reload"** in the reason ("another save landed first — reload", "start is an earlier
   save") → stop the push loop, reload the server's state (the login load path), resume. Today the
   client retries the same bundle forever.
3. **Permanent rejections** (`ok:false`; see the status table in §18 — skew is 409, limits 400) → stop
   exporting and show the session notice; do not retry the same bundle.
4. **Login skew check.** `/auth/*` now answers `game_data_version` and `sim_build` beside
   `sim_version`. Compare all three with the local dylib (`roi_sim_version`, `roi_sim_build` — new
   FFI export) / StreamingAssets copy at login and show
   the update prompt instead of starting a session whose every save will be refused.
5. **SummonWindow tickets** — show the per-banner `tickets` the server returns, not one shared count.
6. **Dead calls** — Outpost and Trade endpoints are gone server-side; remove the calls and their UI.
7. **Missing screens** (server built, no UI yet): store checkout + finalize flow, Monthly card claim,
   premium Battle Pass purchase, tutorial quests (`/tutorial/state`, `/tutorial/claim`), Farm
   Banner, Standard select, crystal exchange / shop.
8. **Duskmourn card** — draw the disabled state when the zone is locked (it is clickable now).
9. **`PushNow` semantics** — it should force an immediate push of the CURRENT pending bundle, not
   export a new one while a push is in flight (sim-host re-sends the pending bundle anyway).

Also new from this pass, no client change needed but worth knowing:
- Refunded pack / premium-BP mail is now VOIDED (hidden from `/mail`) rather than deleted; a claim
  already taken into the sim still settles.
- `GET /store` and the BP endpoints treat a season past its end as no season, even before the
  hourly refresh — hide BP UI on `no active season`.
- A new checkout of a SKU cancels this account's own unfinished one of the same SKU; finalizing
  the cancelled order answers 409 "that order is closed".
- Sealed Cache and Farm Banner wins are no longer announced in chat (user 2026-10-03: no name
  broadcasts). Chat still carries players' own messages; a system line (`"from": ""`) is not sent
  by anything now.

## 15. Re-audit 2 (2026-10-03) — client items

1. **Dungeon Enter sends the wrong zone.** `UI/DungeonWindow.cs:20` sends
   `EnterInstance{"zone_b10_1"}` (the band-10 stage). The Godot original sent
   `zone_dungeon_duskmourn_1`. The sim's stage gate refuses the band-10 stage unless band 9 is
   cleared (silently), and when it passes the party lands on the wrong map. Send
   `zone_dungeon_duskmourn_1`.
2. **Store purchase confirmation = mail.** Pack contents stay in the mailbox as the purchase receipt
   (user 2026-10-03). After `/store/finalize` answers `granted`, show "purchase complete — items are
   in the Mailbox"; the mailbox red dot already comes from `/badges`. Finalize may now answer
   409 `payment in progress — retry` (another finalize of the same order is with Steam) — retry
   after a moment, do not open a new checkout. It may also answer 202 `payment is being confirmed`
   (Steam timed out mid-charge; the server settles it within ~15 min): show "processing", then
   refresh `/store/me` and the mailbox later.
3. **`/mail/claim` and save verdicts:** a mail withdrawn by a refund answers 410 / save reason
   "already claimed or withdrawn"; treat like the stale-claim reload.

## 16. Audit round 4 (2026-10-03) — client items

1. **HIGH — no save stall in Unity.** CLAUDE.md promises the client freezes the sim after
   `SPAN_STALL_TICKS` (~5 min) unsaved; `SimDriver.Update` / `SessionBoot.Push` have no such stop.
   During a long outage the sim keeps playing, and past `MAX_REPLAY_SPAN` (50k ticks, ~83 min) every
   later bundle is refused (400) until a relaunch — everything since the last save is lost. Port the
   Godot stall (`sim_driver_core.gd`, `SPAN_STALL_TICKS`): stop ticking while the unsaved span is at
   the limit, show the reconnect notice, resume when a save lands.
2. **Offline EXP is now paid only for the gap between the last save and a LOGIN** (`GET /save`
   handshake), on the first save of that session. Reconnecting without a new handshake earns none —
   do not add a re-handshake on network recovery to "fix" this.
3. **`PushNow`** after a failed push re-sends the old pending bundle (sim-host keeps it until it
   lands), so a market action placed meanwhile is not in it; refetch after the NEXT successful push.
4. **Login status codes (round 5-6):** `/auth/*` now answers **503** when the server could not
   reach its database ("sign-in is unavailable right now — try again") and **403** only for a ban.
   Retry a 503 with backoff; show the ban text on a 403.
5. **Chat:** messages over 4 KiB close the socket (server cap); the server also closes a socket
   whose session was superseded or banned — treat a chat close as "re-check the session".

## 17. Audit round 7 (2026-10-03) — contract mismatches found by diffing Rust against the C#

1. **Resonance page is always empty.** `UI/ExemplarResonancePage.cs:83` queries `owned_exemplars`,
   which the host removed (f4bd5c51); unknown query keys now answer `null`. Read `exemplar_units` /
   `roster` instead.
2. **Gacha in crystal debt loops.** A roll while the wallet is negative now answers **409**
   `{"reason": "crystal balance is negative", "debt": true}` (was 402, which `ServerClient.cs:340`
   parsed as a coupon quote → "Short 0 coupons. Buy them for 0 Crystal?" forever). Show the debt.
   `/shop/buy` answers the same 409.
3. **Save response `drops` is never applied.** `SaveResp` (`Net/ServerClient.cs:686`) has no
   `drops`; it carries the offline-EXP envelope and Farm Banner / Sealed Cache mail rows. Apply each
   like the gacha's mail rows and show "While you were away" for the offline envelope (§10). Add to
   the §14.1 port list.
4. **Dead view reads (harmless):** `loot_gap_mult_bp` (`UI/ArenaOverlay.cs:260,507`) and
   `job_level` (`UI/ExemplarReadModel.cs:70,91`) no longer exist.
5. Already listed but still open: `/bp/claim` `granted[]` ignored (`UI/PassWindow.cs:221`);
   `/store` `section` ignored (`UI/CrystalWindow.cs:118` draws packs as "0 Crystal");
   crystal `tradeable` still read (server sends `paid` too — switch before the alias goes);
   `AuthResp` lacks `game_data_version` / `sim_build`; badges / features never read.
6. Save may now answer **429** "too much replay in a short time" (per-account replay budget) and
   **503** "server busy — retry" (replay could not run) — both are retry-later, never a reload.

## 18. Round 8 (2026-10-03) — what the save and login endpoints answer, in ONE place

This table supersedes the scattered notes in §11-17 where they disagree.

| Endpoint | Status | Body reason (contains) | Client should |
|---|---|---|---|
| POST /save | 200 `ok:true` | stored / idempotent | prune `settled_ops`, apply grants + `drops` |
| POST /save | 200 `ok:false` | replay hash mismatch, forged …, fusion bind-check | permanent: stop pushing this bundle, reload |
| POST /save | 200 `ok:false` | already claimed or withdrawn | reload (a refund withdrew mail the client holds) |
| POST /save | 200 `ok:false` | server persistence error | retry later (same bundle) |
| POST /save | 400 | span / log / snapshot_interval limits | permanent for this bundle: reload |
| POST /save | 408 / 413 (plain text) | request body too slow / too large | 408 retry; 413 permanent |
| POST /save | 409 | sim/catalog/sim build skew | update prompt — a reload does NOT fix skew |
| POST /save | 409 | `reload` (earlier save, another save landed, server restart reset the clock) | GET /save (reload), resume |
| POST /save | 409 | fast-forward / cumulative tick advance | reload |
| POST /save | 429 | save already in progress / too much replay / too many uploads | retry after a delay, same bundle |
| POST /save | 503 / 504 | server busy / timed out | retry after a delay |
| GET /save | 429 | (empty) — a save of this account is running | retry shortly |
| /auth/* | 403 | ban text | show it |
| /auth/* | 503 | sign-in is unavailable right now | retry with backoff |

Bodies of 408/413/429 from the body reader and 503 from the load shed are PLAIN TEXT, not JSON —
guard the parse (§13.1).

Also this round:
- **Chat is removed from the game** (user 2026-10-03). `/chat` no longer exists server-side; remove
  the chat UI and its connection code.
- **Offline EXP is mailed at LOGIN** (`GET /save`), not with the first save: after BeginSession, read
  `/mail` (or badges) for an `offline_exp` envelope to show "While you were away". The save
  response's `drops` no longer carries it (it still carries Farm Banner / Sealed Cache mail).
- **Weekly commissions turn over Monday 04:00 Thailand time**; the shop's daily cap now turns over
  at 04:00 Thailand time like every other daily. Use the server's `week_end` / `day_end` for
  countdowns, never local math.
- New FFI export `roi_sim_build()` (caller frees with `roi_string_free`) for the login skew check.
- **Dungeon progression gate (sim):** `EnterInstance` into `zone_dungeon_*` is now a silent no-op
  until the party's reach (`band_select_max` in the view) is at least the dungeon's first-room stage
  (Duskmourn = 111). Draw the Duskmourn card disabled with the requirement until then (§15.1's
  wrong zone id still needs fixing first). **Story chapters** unlock in order — gate each chapter on
  `/story/progress` having the previous one cleared (server order: see `STORY_MISSIONS`).
- **Round 9:** a request body must arrive at ≥ 32 KiB/s after its first 2 s (else 408), and the
  server sheds uploads with 503 "server busy" when its body memory budget is full — retry later.
  Always send `Content-Length` (a body without it reserves the 2 MiB maximum).

## 19. EVERYTHING STILL OPEN FOR THE CLIENT — one list (2026-10-03, after audit rounds 3-13)

**Supersedes §11-18 where they disagree.** Corrections to earlier sections: offline EXP is mailed
at LOGIN, not with the first save and not in `drops` (§16.2 and §17.3 are stale); chat is REMOVED
(§14's chat note and §16.5 are stale).

**Backend to integrate (master, not pushed):** `4f4fdd87` (audit rounds 3-8, the scoped §13-16
commit you asked for), `ebc26965`, `866c90d6`, `4505711d`, `92912a70`, `23e4da2c`. sim-core is
**0.0.491**; rebuild the dylib (`tools/unity_dylib.sh`) — `SIM_BUILD` now gates saves, and the
dungeon stage gate is native. Your 5cc6eb6d / 77f28bb6 already cover a lot of §11-16 (save safety,
grants/prune, Resonance, 3000-tick stall, dungeon zone id, store finalize, debt display); re-check
each item below against them and tick off what is done.

### P0 — sessions and saves
1. **Status codes:** follow the §18 table exactly. New since your integration: `GET /save` can
   answer **429** (a save of this account is running — retry shortly); `/save` 409 **"session
   clock reset by a server restart — reload"** (unflagged) → GET /save and resume; 429 "too much
   replay in a short time"; 503 "server busy"; plain-text 408/413/429/503 from the body reader.
2. **Login:** `/auth/*` answers **503** for a server-side failure (retry with backoff) and **403**
   only for a ban. `AuthResp` carries `game_data_version` and `sim_build`; compare with
   `roi_sim_version()` / **`roi_sim_build()`** (new FFI export, free with `roi_string_free`) and the
   StreamingAssets catalog — show the update prompt on a mismatch.
3. **Uploads:** always send `Content-Length`; a body must arrive at ≥ 32 KiB/s after its first 2 s.
4. **Offline EXP:** paid by the server at `GET /save` as an `offline_exp` mail. After BeginSession,
   look in `/mail` (or `/badges`) for that envelope, claim it and show "While you were away"
   (≥ 30 min). The save response's `drops` no longer carries it.
5. **Save response `drops`:** Farm Banner and Sealed Cache mail rows — apply them like the gacha's
   mail rows (if your 5cc6eb6d port of `grants` did not already include `drops`).

### P1 — features and contracts
6. **Chat is removed from the game** (user 2026-10-03): `/chat` is gone server-side — remove the
   chat UI, its socket code and any chat strings that are now unused.
7. **Dungeon gate:** `EnterInstance` into `zone_dungeon_*` is a silent no-op until `band_select_max`
   ≥ the dungeon's first-room stage (Duskmourn = 111). Draw the card disabled with the requirement.
8. **Story order:** prologue → 1-1 → 1-2 (`STORY_MISSIONS`). Unlock each chapter only when
   `/story/progress` lists the previous one; the server does not enforce the order.
9. **Resets:** weekly commissions now turn over **Monday 04:00 Thailand time**; the shop's daily
   cap at **04:00 Thailand time**. Use `week_end` / `day_end` from the server for countdowns.
10. **Gacha / shop in crystal debt:** 409 `crystal balance is negative` with `"debt": true` (no
    longer 402 — make sure the 402 coupon-quote path cannot catch it).
11. **Contract leftovers (§17.5):** `/bp/claim` `granted[]` (crystal / tickets) not shown;
    `/store` `section` ignored (packs drawn as "0 Crystal"); crystal `tradeable` still read — the
    server sends `paid`, switch to it; `badges` / `features` never read; dead reads
    `loot_gap_mult_bp`, `job_level`.
12. **Store receipt:** pack contents arrive as mail; after `granted` show "purchase complete —
    items are in the Mailbox". 202 "payment is being confirmed" = processing (you handle this).
    A withdrawn (refunded) mail answers 410 / "already claimed or withdrawn" → reload.

### Waiting on the backend (my side, not yours)
- In-place `AddPartyProvision` refill (top up the same rack slot without resetting its key /
  ready_tick / live buff) — not done yet; tracked.

## 20. Mint coupons (2026-10-03, server BUILT) — client work

The cost of sending an item to Steam (name not final). Server: `crates/verify-server/src/mint_coupon.rs`.
- `GET /mint-coupons` → `{held, pending, held_cap: 10, pending_cap: 5, room, next_at_ms, now_ms}`.
  One accrues every 4 h of real time (offline too) up to 5 uncollected; `next_at_ms` is null while
  the pool is full (clock stopped). `room` = how many a claim would take now.
- `POST /mint-coupons/claim` → `{claimed, held, pending}`. Fills held to 10; the rest STAYS
  uncollected. **Warn before claiming when `room < pending`** ("only N fit; M stay uncollected").
- `/badges.mint_coupons` = what a claim would collect now (red dot on the claim button).
- **Send to Steam costs 1 held coupon per item.** Block the send button when `held == 0` (an item
  sent without a coupon leaves the bag and waits on the server until one is held). Show the held
  count on the send screen. Minting is OFF on the server right now (`STEAM_INVENTORY_LIVE` unset);
  queued items mint once it is switched on.
- Untradeable, free only. A future second use (upgrades) is not designed yet.

## 21. Steam → game import: the mailbox "From Steam" tab (2026-10-04, server BUILT) — client work

Items a player holds in their Steam inventory (bought on the Market, or sent there earlier) come
back into the game. Import is free and immediate, and the item stays tradeable, so it can be sent
to Steam again (at the usual cost of 1 mint coupon). Server code: `crates/verify-server/src/steam_import.rs`
and the `steam_*import*` handlers in `main.rs`.

- **UI:** add a second tab to the mailbox, "From Steam". The existing mail list stays the first tab.
- `GET /steam/inventory` → `{items: [{steam_itemid, itemdefid, def_id, quantity, importing}], mails}`.
  - Draw the items from `def_id` through the catalog, the same way bag items are drawn.
  - `importing: true` means an earlier import is still settling. Show it disabled.
  - `mails` holds imports that just settled: claim them at once (see below).
- `POST /steam/import {steam_itemids: [..]}` (1–50 ids) → `{mails, pending, refused}`.
  - Offer "Import" per item and "Import all".
  - **Auto-claim every mail in `mails`** straight into the bag, the same way gacha results are
    claimed. The player never has to open the regular mail tab for these.
  - `pending`: Steam's answer was lost. Say "importing…". The next `GET /steam/inventory` settles
    these and returns their mails.
  - `refused`: the item is no longer in the player's Steam inventory (sold, moved, or already
    imported). Refresh the list.
- Every import mail has `reason == "steam import"`. If an auto-claim fails (for example, the bag
  is full), the mail stays in the inbox like any other, so nothing is lost.
- **Errors:**
  - 503 `steam inventory is off`: hide the tab or show a disabled state. It is off on the server
    right now, behind the same `STEAM_INVENTORY_LIVE` switch as minting.
  - 400 `not a Steam account`: a dev login.
  - 429 `too soon`: the server allows one Steam read per account every 5 s. Don't poll; refresh
    when the tab opens and after each import.
- i18n: every new label goes through `Loc.*` with keys in `godot/i18n/strings.csv`.

## 22. Unit ids renamed: `rapis` → `lapis`, `nyx` → `raplus` (2026-10-04, sim 0.0.492, catalog 0.0.221)

The ids now match the display names (Lapis and Raplus). This is local only, and NOT deployed yet:
the server and the dylib must move together.

What changes for the client:
- In the view JSON, `unit_def` is now `lapis` / `raplus`. Exemplar ids follow the same rename, and
  so do the carrier items `exemplar_lapis` / `exemplar_raplus`.
- The loc keys are now `CLASS_NAME_lapis`, `CLASS_NAME_raplus` and `ITEM_LORE_exemplar_raplus`.
  `strings.csv` is already updated and copied into StreamingAssets.
- `tools/unity_data.sh` has already copied the new catalog into StreamingAssets. **Rebuild the
  dylib (`tools/unity_dylib.sh`) and restart the Editor.** An old dylib reading the new catalog
  cannot find `rapis` / `nyx`.
- Old saves load onto the new ids automatically (`migrate_save_items`).

C# that compares a unit id has to change. Sprite or art keys that happen to be named `rapis`
(sheet names, `casterArtKey:"rapis"`, `rapis_Idle.asset`) are skin keys and can stay.
- Runtime:
  - `Scripts/Arena/ArenaView.cs:4272`: `if (id == "rapis") id = "mage_t1";` should now test
    `"lapis"`.
  - `Scripts/UI/StoryWindow.cs:38`: `Cast.Of("rapis", 50)` should use `"lapis"`.
- Editor probes that check `unit_def` or start a sim as `rapis` / `nyx`:
  - `PotentialBoardProbe.cs:23`
  - `EquipmentTransferVerification.cs:189`
  - `LapisMeteorRuntimeProbe.cs:25` (`SimSession.NewAs(..., "rapis", ...)`)
  - `LapisFlameShieldPreview.cs:122` (the `unit_def` in its test JSON)
  - `LucielleUltimatePreview.cs:62` (`AddAlly("rapis")`, if that is a unit id)
  - `AvelineBasicPreview.cs:102` (`unit_def = "nyx"`)
- Check `CompanionStripStudy*.cs` (`s.name == "rapis"`): if it filters sprite sheets by name,
  it stays.

## 23. Lane mode: show it in the game (2026-10-04, sim BUILT, dev slots only)

The user wants the Trickcal-style lane combat (`docs/lane-combat-design.md`) visible IN THE GAME.
The sim side is done. `sim_core::lane` is an opt-in mode on the live combat and the live room flow;
it is OFF everywhere unless a session turns it on.

**Turning it on / off (dev slots only):**
- `roi_debug_json(sim, "{\"lane_mode\":{\"cap\":2}}")` turns it on. `cap` = monsters per cell.
- `roi_debug_json(sim, "{\"lane_off\":{}}")` turns it off. It returns to the stage's first room on
  the normal field.
- Like every debug edit, both are refused once online (`set_allow_debug(false)`). The verify server
  replays without lane mode, so it would refuse a lane save anyway.
- Put a dev toggle on the debug panel. Its labels go through `Loc.*`.

**What the view JSON carries while it is on:**
- `grid.width = 20`, `grid.height = 3`: three lanes. y 0 is the back lane (top of the screen) and
  y 2 the front lane (bottom). Entities keep using `pos.x` / `pos.y` as today.
- The new `lane` block:
  - `stack_cap`
  - `slots` and `homes`: `{ "<entity id>": [x, lane] }`. `slots` is the formation as placed; `homes`
    is where each slot stands now, because the formation advances as one block.
  - `advance_cd`
- Up to `stack_cap` monsters can share ONE cell. Offset them inside the cell so a pile reads as a
  swarm, not as one sprite.
- The party stands on a 3×3 formation: back / mid / front at x = 4 / 6 / 8, with gap columns at
  5 and 7. Monsters enter at the right edge of their lane and walk left.
- Everything else is unchanged and should render as today:
  - skills, heals, buffs, ultimates (`ult_fx`, cut-ins), `recent_hits`, `heal_fx`
  - room changes: 3 rooms per stage, room boss + minions, the march between rooms

**Presentation (from the user's Trickcal reference):**
- Side view.
- Large chibi-sized sprites: about a third of the screen tall, overlapping by lane depth and
  sorted by y.
- The whole 20-cell field framed in one shot.
- Party facing right.

A visual reference that replays real sim traces is the "Lane Stage Replay" artifact (placeholder
boxes, not art).

The usual rules hold: views stay read-only, nothing here moves gameplay, and FX and companion mode
switch off as before.

**§23 addendum: cell size, walking pace, contact (user question, 2026-10-04)**
- The sim owns time and distance in CELLS. Monsters walk ~1 cell per 0.7 s, the party ~1 per 0.3 s,
  and range is a cell count. Drawing a cell wider never slows the game. A wider cell only makes a
  sprite cover more pixels per second, so choose a cell width where the walk cycle does not slide.
- Contact is the sim's call. A melee hit happens between ADJACENT cells (diagonals across lanes
  count); a ranged hit happens within its range in cells. `recent_hits` says who hit whom on the
  tick. Stand each sprite at its cell centre, and play the swing on its hit. A small lunge toward
  the target is fine, but the position stays the sim's.
- To make adjacent cells read as "touching", make one cell about one body wide. With
  Trickcal-sized chibis (about 1/3 of the screen tall), that shows about 10–12 of the 20 cells. Frame
  the fight around the contact line with the party on the left. The spawn edge (x 17–19) then sits
  off screen, so monsters walk in from the screen edge.

**§23 addendum 2: display modes (user, 2026-10-04)**

The user wants the lane battle to play in a STRIP ALONG THE BOTTOM OF THE DESKTOP, like Deskrawl:
Idle ARPG (Steam app 4623570). In Deskrawl, combat runs in a strip along the bottom of the desktop
while other apps stay in front.

The two sides walk toward each other as in Trickcal Re:VIVE. In the sim, the party's formation
advances from the left while nobody in it has a target in range, and monsters enter from the right
edge of their lane and walk left.

Keep the existing modes. The result is THREE:

1. **Bottom strip (new)**
   - Full desktop width, a short band at the bottom, always on top, click-through outside the
     battle.
   - The whole 20-cell field fits across the width, about 96 px per cell at 1920.
   - The 3 lanes are compressed in depth to fit the strip height, so sprite size follows the strip
     height, not the 1/3-of-screen chibi size.
2. **Full screen (existing)**
   - The Trickcal framing from §23: large chibis, about 10–12 cells around the contact line, with
     monsters walking in from off screen.
3. **Companion (existing widget)**
   - Unchanged, and it must still meet the companion-mode CPU/GPU rules in CLAUDE.md.
   - Those rules apply to the strip too: it is an always-on idle surface, so bounded render cadence,
     optional FX off, and no gameplay slowed.

All three draw the same sim positions (cells and lanes). Only the camera and sprite scale differ.

**§23 addendum 3: how the fight is staged (user, 2026-10-04)**

These rules are BUILT in sim-core but NOT YET in the deployed sim or the staged dylib (pending
commit + deploy). The client draws what the sim sends, so no gameplay code is needed. Plan the
presentation for them:

- **Clash mid-field.** The party holds its formation (front column x = 8), so the fight meets at
  x = 9–10, the middle of the 20 cells. The formation steps forward only when a shooter hits it from
  beyond everyone's reach.
- **A room's first group meets the party half-way.**
  - On every room open, party members start 4 cells left of their slots (the back row on the left
    edge) and walk onto them.
  - At the same time, the room's opening monsters walk in from the right edge.
  - In the view, a member's `pos` starts left of its `lane.homes` cell and walks to it.
- **Every later spawn walks in from the right edge**, up to the quota and the boss, while the party
  stands. The user compared this with spawning next to the clash line and chose walking in.
- **Walking speed** may be slowed for lanes (`lane.speed_bp`, in the view's `lane` block;
  10000 = live). Trickcal's walkers are slower. The value is not decided yet. Read `pos` changes
  as they come; do not assume a fixed pace.

**§23 addendum 4 (2026-10-05):** lane walking speed is decided at **50% of live**
(`lane.speed_bp = 5000` by default). The addendum-3 rules are now DEPLOYED and in the staged dylib
(see the inbox note for the commit).

**§23 addendum 5: groups and the scrolling march (2026-10-05)**

- In lane mode monsters no longer trickle in one by one. Each room is 3 BIG GROUPS (~10 each),
  released at once from the lane edges (x 12–19, up to `stack_cap` per cell). Each group walks in
  and clashes mid-field. After it falls the party walks on for 2–3 s, then the next group comes.
  After the third group the live room boss and minions appear.
- The view's `lane.marching` is true while nobody in the party has a monster in range. While it is
  true, scroll the scene (parallax) and play the party's walk cycle; ease in and out, since it
  flips on contact. The party's cells do not change while marching: the motion is the scene's.
- Visual reference with the scroll: the "Lane Spawn Compare" artifact, option C.

**§23 addendum 6: walk-and-crash (2026-10-05)**

- **Both sides walk, nobody waits.** While `lane.marching` is true, the party walks: the scene
  scrolls and the walk cycle plays. The approaching group moves toward the party at monster pace +
  party pace, so they meet mid-field (x 9–10).
- **Nobody attacks until the CRASH.** No swings and no casts, shooters and long-range skills
  included, until a monster comes within one cell of a member. `lane.clashed` turns true on that
  tick and `marching` turns false. Fight there until the group is down; then `marching` returns
  and the next group appears.
- Draw the crash as the moment both front lines meet. Hits only start after it.

**§23 addendum 7: sub-cell positions, so the picture never disagrees with the sim (2026-10-05)**

The sim moves units a WHOLE cell at a time. At lane speed, a monster takes about 1.3 s per cell.
- Drawn only at its cell, a unit jerks.
- Drawn by easing toward each new cell after the sim has moved it, the picture trails the sim by up
  to a cell. The sim says "crashed, fighting" while the sprites still stand apart.

Draw where the unit REALLY is, from progress the view already carries:
- **Own walk:** `entity.move_acc` (0–999) is the progress toward the next cell. In lane mode:
  - monsters walk toward smaller x;
  - a party member whose `pos` differs from its `lane.homes` cell walks toward that home (room
    walk-in).
  - Drawn x = `pos.x ∓ move_acc / 1000`.
  - The sim zeroes `move_acc` for anyone fighting (an enemy within its reach, after the crash), so a
    fighter draws exactly on its cell.
- **The party's march:** while `lane.marching`, every surface monster also closes by the party's
  walk. Subtract `lane.march_acc / 1000` from every monster's drawn x.
- **So:** drawn monster x = `pos.x − move_acc/1000 − (marching ? march_acc/1000 : 0)`.
- **Lane changes (diagonal steps)** are not announced ahead. Ease y over about one step when a unit's
  lane changes.
- With one cell about one body wide (addendum 1), adjacent cells read as touching. The drawn crash
  then matches the sim's crash.

If this still looks too coarse in the game, the fallback is a finer lane grid (for example 40 cells,
half-body each, with every range and speed doubled). That is a sim change. Ask before assuming it.

**§23 addendum 8: movement contract after the user's rule audit (2026-10-05)**

- **Walkers move ONE cell per step and never overtake.** A full cell ahead is queued behind: no
  multi-cell pass-through and no side-step around it. This was the 2-cell jump Codex traced (ticks
  44/57).
- **A blocked walker holds `move_acc` at 999** (the edge of its cell) instead of wrapping to 0. The
  addendum-7 formula therefore waits at the edge and never snaps back.
- **Lane changes (one diagonal cell)** happen only when no target is reachable straight along the
  lane, or toward a taunter. Keep easing y for them.
- **The formation never advances.** Party cells change only on the room walk-in and on Iron Chains
  pulls, which move monsters, not the party.
- **Decided but NOT built yet:** unlimited stacking while fighting, no stacking while walking in.
  Today `lane.stack_cap` (2) applies throughout.

**§23 addendum 9: group release layout (2026-10-05).** A group now lands one monster per cell as a
3×3 block on x 17–19 (front column first, lanes from a random one). Only a group of more than 9
stacks a second monster on a cell, in the same order. It then walks in under addendum 8: queued, no
overtaking.

**§23 addendum 10:** 4 groups per room (user). A group is a quarter of the room's quota, at most 9,
so every group lands one monster per cell in the 3×3 block.

**§23 addendum 11: one progress counter, and a clean arrival at the crash (2026-10-05, fixes Codex's
contact rewind 8.083 → 9.000)**

- **`lane.march_acc` is RETIRED (always 0).** The party's march now adds straight into each walker's
  own `move_acc`.
  - A walker has ONE counter, so it never draws more than one cell ahead of its cell.
  - **Drop the march term.** Drawn monster x = `pos.x − move_acc/1000`.
- **Arrival:** a walker whose cell touches a party unit (Chebyshev 1) has `move_acc` = 0, so it
  stands at its cell's centre. It takes that 0 on the very tick it steps in, so the progress runs to
  ~999 and then the cell changes with progress 0: continuous, no rewind, never inside the party's
  column.
- **Queued walkers** (a full cell ahead, not the party) still hold 999 at the cell edge.
- **Party walk-in:** progress is 0 on reaching the slot (no overshoot).
- A sim test now checks every monster's drawn x every tick of a whole room flow: it never moves
  back (except a pull, which moves ≥ 1 cell) and never enters the party's front column.

**§23 addendum 12: enemy gaps, the teleport-back hook, vertical spacing (2026-10-06)**

- **Enemy block with gaps.** A group lands on x 15 / 17 / 19 (gap columns 16 and 18), one per cell.
  Before the crash, monsters in a lane keep at least one free cell between them. After the crash they
  may close up and stack (`stack_cap`).
- **Teleport back.** When the march resumes (`clashed` true → false), any party member off its slot
  is put back on it in one tick. `lane.returned` lists those ids for that tick only. Play a
  teleport-back animation for them instead of easing the move. No unit uses this yet: an
  ambush/teleport unit is planned. Nothing else moves the party off its slot today except the
  room walk-in.
- **Vertical spacing: VISUAL ONLY (user chose this).** Draw the 3 lanes further apart in depth so
  sprites in neighbouring lanes overlap less. The sim keeps 3 adjacent lanes, so every gameplay
  distance stays the same: do not change any range or contact logic for it.

**§23 addendum 13: stacking (2026-10-06).** Before the crash, monsters never share a cell (the only
exception is a release of more than 9, which does not happen at the current group sizes). After the
crash, any number may share any cell: piles of 6–8 on the cells in front of the tanks are expected.
Spread them with bounded offsets. The user accepts the blob as a known issue, to be eased later by
mixed melee/ranged groups.

**§23 addendum 14: FORMATION SCREEN, sim side built (2026-10-06, dev slots)**

- **Read:** `lane.slots` is `{ "<entity id>": [x, lane] }`, with x 4 / 6 / 8 = back / mid / front and
  lane 0..2. Draw a 3×3 grid with the gap columns shown between ranks.
- **Write:** `roi_debug_json(sim, "{\"lane_slot\":{\"entity_id\":ID,\"col\":\"back|mid|front\",\"lane\":L}}")`
  moves a member. A member already on that slot SWAPS into the mover's old slot. It returns 0 for a
  bad id or column.
- **Behaviour:**
  - The choice persists across rooms (`lane.chosen`, which wins over the auto-formation). Members not
    in it, such as a new recruit, get an auto slot.
  - Members WALK to their new slots; there is no teleport.
  - Like every lane command it is dev-only: refused online. A real online formation (a replayable
    input) comes when lane mode becomes the real mode.
- **UI:** drag a portrait onto a slot. Show who stands where now, and show an empty lane as open
  (monsters can flank it). All labels go through `Loc.*`.
