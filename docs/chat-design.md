# Chat System — design

**Status:** slice 1 (moderation core) BUILT + tested 2026-07-12 (`crates/verify-server/src/chat_mod.rs`).
WS transport + client UI = pending slices.
**Companion:** `steam-integration.md` (identity = steamid + persona), `localization-plan.md` (lang channels
map to loc T1), `anti-cheat-and-server-model.md` (chat is OUT-of-band from the sim).

## 0. หลัก — chat ≠ sim state

chat = ephemeral communication → **แยกจาก deterministic GameState** (ไม่ผ่าน replay-verify). เป็น
server-authoritative service แยก เหมือน market/mail. transport = **WebSocket** (real-time push; ต่างจาก
save-push HTTP). ⚠️ WS = persistent connection → ขัดกับ scale-to-zero → เมื่อ chat live server ต้อง
min_machines_running ≥ 1 (prod เป็นอยู่แล้ว). identity = account (steamid) + display = persona name.

## 1. Moderation — rule-based, NO AI (slice 1, DONE)

`chat_mod::moderate(text) -> ModVerdict { Allow | Censor(String) | Reject(&str) }`. Pure, unit-tested,
zero per-message cost, no external service (→ no Steam AI Q4 disclosure).
- **links blocked 100%** — schemes (`://`), `www.`, "dot"/space evasion (`site . com`, `example dot com`),
  known TLDs; decimals/abbreviations (`3.5k`, `e.g.`) NOT false-blocked.
- **profanity censor** — leetspeak-normalized (`sh1t`/`$hit` → masked), returns original with masked spans.
- **length cap** `MAX_LEN=300`.
- **rate-limit** = STATEFUL → WS layer (token bucket per connection), not the pure module.
- **player report/block/mute** → auto-action at threshold (WS/server state).
- anti-bot for free: Steam $5-account gate + rate-limit → chat spam-bots barely viable.
- AI toxicity API = OPTIONAL later (only if rule-based can't keep up) → then discloses Steam AI Q4.

## 2. Channels (routing at the WS layer)

| channel | key | scope |
|---|---|---|
| **Language** | `lang:<code>` (en/th/zh…) | split by language — each language its own global; default from Steam language. Solves the language barrier; maps to loc T1 |
| **Zone** | `zone:<band/zone id>` | everyone whose party is currently in that zone/band → chat with peers at your progression (auto join/leave on zone change) |
| **Trade** | `trade:<lang>` | listings/wanted, split by language, separate from general chat |
| **Party** | `party:<id>` | your party |
| **Whisper** | `dm:<account>` | 1-to-1 |

Routing: server tracks per-connection `{ account, lang, current_zone }`; client reports `lang` on connect +
`zone` on change. A message names its target channel → server fans out to matching subscribers. Moderation
runs on EVERY message regardless of channel.

## 3. Slices

- [x] **1. Moderation core** — `chat_mod.rs` + tests (links 100% blocked, profanity, length).
- [ ] **2. WS transport + routing** — axum WebSocket endpoint (auth via session token); per-connection state
  (account/lang/zone); channel fan-out; rate-limit (token bucket) + report/block/mute; run every message
  through `moderate()`.
- [ ] **3. Client UI** — Control-based chat panel (channel tabs: Lang/Zone/Trade/Party/Whisper), input box,
  send/receive over WS, block/report buttons, persona-name display.
- [ ] **4. Polish** — recent-history buffer (server), unread badges, /commands, timestamps.

## 4. Decisions / open

- chat is **launch-optional** — the market trades async (no chat needed). Auto-mod makes it feasible for a
  small team, but it can ship post-launch. If shipped, update the Steam "In-game chat" interactive element
  + moderation-guardrail disclosure.
- lang default: from Steam language (`getCurrentGameLanguage`) — player can switch channels.
- zone channel granularity: per BAND (broad, more people) vs per-zone (narrow) — start per-band.
- profanity list: starter in-code; move to config + community-sourced list later.
