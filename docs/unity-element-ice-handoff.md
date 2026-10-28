# Unity: Water is Ice — handoff 2026-09-24

From the sim/gameplay session to the Unity client (Codex). Sim 0.0.475 / catalog 0.0.210.

The element formerly named **Water** is **Ice** everywhere in the sim (user: "เรียกว่า Ice ดีกว่า").
Every element string the view JSON sends (`element`, `projectiles[].element`, weakness lists,
card elements) now reads `"Ice"` where it read `"Water"`. Saves written before still load (serde
alias) — the rename is only what the client receives.

Client lookups keyed on the old string go silently to their default:

- `Assets/Scripts/Arena/ArenaProjectiles.cs` — the bolt tint table has `["Water"]`.
- `Assets/Scripts/UI/CardArt.cs` — the card-frame index has `["Water"] = 3`.

Add `"Ice"` beside (or in place of) `"Water"` in both. The TH display name is น้ำแข็ง.
