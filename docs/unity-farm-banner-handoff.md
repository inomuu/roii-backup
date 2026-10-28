# Unity: Farm Banner — handoff (updated 2026-09-25, ticket flow)

From the sim/gameplay session to the Unity client (Codex). Sim 0.0.480 / catalog 0.0.214.
Design: `docs/farm-banner-design.md` (section "Built — box → ticket → banner").

## What the player sees

- While a Farm Banner event runs, being online earns **Farm Banner Boxes** (`farm_box`), mailed
  (reason `"farm banner"`) and claimed into the bag. The box is TRADEABLE and can be sent to Steam
  (`docs/unity-send-to-steam-handoff.md`).
- **Opening a box gives a ticket** — no reward yet. The ticket is bound and never expires.
- The **gacha page** shows a Farm Banner for each event the player holds tickets for (hidden at
  zero tickets, even while the event runs). ×1 / ×10 pulls spend tickets: the featured character
  (Zolfia) at 1%, guaranteed at the 150th pull without her; otherwise Gil or a bound Sigil stack.
  Everything arrives by mail. Wins are not announced (removed 2026-10-03).

## What the client does

0. **Boxes drop straight into the bag** (user 2026-09-26: a drop, not a mail to collect) — the
   same pattern as `SummonWindow.Collect()` for gacha pulls. The save reply (`/save`) now carries
   `drops`: the mail rows of the boxes that save earned, shaped like `/gacha/roll`'s `mails`.
   After an accepted save, feed each row to `MailWindow.ClaimFrom(sim, id, row)` in order and show
   a drop toast ("Farm Banner Box") — no mailbox step. Rules:
   - Each row arrives in ONE reply only. Claim it at most once; never re-claim a row from a later
     reply or from the mailbox list (the server treats a second claim of a mail as forgery and
     rejects the save).
   - A full bag makes the claim a no-op (`ClaimFrom` returns false); the box stays in the mailbox,
     which is the safety net. Say so: "Bag full — box waiting in the mailbox".
   - Boxes minted before this change (3 on the dev account) are ordinary mailbox rows: claimed by
     hand once.
   Every item with a Steam identity is a SINGLE in the bag since 0.0.480 (`steam: true` on the
   item, `quantity` always 1): one box = one slot = one Steam item. Do not draw a stack count.
1. **Bag → Open** on any item whose catalog def has `farm_box: true` sends
   `{"OpenFarmBox": {"inv_idx": N}}` (the Sealed Cache shape). The ticket appears on the server
   after the next accepted save — refresh `/gacha/state` after it.
2. **`GET /gacha/state`** now carries `farm`: a list, one entry per banner to draw:

   ```json
   {"event_id": "zolfia-1", "box_def": "farm_box", "featured": "exemplar_zolfia",
    "tickets": "12", "since": 37, "pity": 150, "featured_ppm": 10000,
    "farming": true, "ends_ms": "1798761600000"}
   ```

   `tickets` and `ends_ms` are strings (u64). `since` = pulls since the last featured win, so the
   pity bar is `since / pity`. `farming` = boxes still drop (show the end date when true; when
   false, the banner still pulls while tickets last). Odds to disclose: `featured_ppm` / 1e6.
3. **`POST /farm/roll`** `{"box_def": "farm_box", "count": 1|10, "idem_key": "<fresh uuid>"}`:
   - 200 `{"ok": true, "reason": "rolled"|"idempotent", "pulls": [{"kind": "featured"|"gil"|"sigil",
     "def_id": "...", "count": N}], "tickets": "11", "since": 38}` — the result screen reads
     `pulls`; the rewards are in the mailbox.
   - 409 `"not enough tickets"` (with `tickets`), 404 unknown banner, 400 bad count. Retry a
     network failure with the SAME `idem_key`; it never pays twice.
   - The ×10 button is disabled below 10 tickets (no crystal top-up on this banner).

Loc: `ITEM_LORE_farm_box`; item names come from the catalog. The banner title/labels are new UI
strings — add keys to `godot/i18n/strings.csv` (the loc source) and run `tools/unity_data.sh`.
