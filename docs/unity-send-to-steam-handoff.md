# Unity: Send to Steam — handoff 2026-09-25

From the sim/gameplay session to the Unity client (Codex). Sim 0.0.478 / catalog 0.0.213.
Design: `docs/steam-mint-quota-design.md` (every account may mint — the pass gate was cancelled).

## What the player does

An item that can live on Steam (today only the Farm Banner Box, `farm_box`) gets a **Send to
Steam** action in the bag. Sending it removes ONE from the game immediately; the server then
mints it into the player's Steam inventory, where it can be traded / sold on the Steam market
(once the market is switched on). It does not come back into the game.

## What the client does

- Show the action only for an item whose catalog def carries `steam_itemdef` and that is not
  `bound`. Confirm first — it is one-way.
- Send `{"SendToSteam": {"inv_idx": N}}` through the normal input path. The sim removes one and
  counts it (`steam_sent` in the state); the server queues the mint on the next accepted save.
- The mint itself is asynchronous (a server worker, once a minute) and only runs when the server
  has `STEAM_INVENTORY_LIVE=1`. Until then the item is gone from the bag and waiting server-side.
  A "sent to Steam — arriving shortly" note is the honest copy; there is no status endpoint yet.
