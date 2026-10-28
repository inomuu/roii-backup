# Anti-cheat & Server Model — Decision Rationale

**Status:** Decision for Phase 0–2. Phase 3+ is open.
**Companion to:** `taskbar-arpg-gdd-phase0.md` §4.4, §9.

> ⚠️ **PARTLY SUPERSEDED — this doc argues from "pillar 1 (offline-first)", which no longer
> holds.** The shipped game is ONLINE-REQUIRED and there is no offline build (user, 2026-07-03
> and again 2026-08-02); the Godot login picker's offline mode is a dev path only. Every
> conclusion below that is justified BY offline-first needs re-deriving before it is cited —
> in particular the "Offline play" row in the comparison table, which scores an option on a
> capability the product does not ship. What still holds independently: sim stays client-side,
> instanced and deterministic, and replay-verify is the anti-cheat spine.
> Current direction: `docs/server-plan-phases.md`.

This document records *why* sim-core is bit-deterministic + event-sourced,
and *why* the server model is "replay-on-demand" rather than "mirror sim
per instance". Both questions come up regularly during design — pin them
here so the next time someone asks "why don't we just have the server
run the sim?" the answer doesn't get re-derived from scratch.

---

## The question that keeps coming back

> "ถ้าผู้เล่นแต่ละคนเป็น single-player instance (ไม่ใช่ shared world),
> ทำไมเราไม่ให้ server รัน sim parallel ของแต่ละ instance ด้วย? เพื่อ
> anti-cheat แข็งสุด — server เห็นทุก tick"

Tempting because: anti-cheat ที่แข็งที่สุดคือ server เป็น ground truth
ตลอด, client เป็นแค่ display ไม่มีทาง forge ผลลัพธ์ได้

แต่ — มันราคาแพงกว่าที่ดูเหมือน, และมันขัด pillar 1 (offline-first)
โดยที่ "ของแถมที่ได้" (real-time anti-cheat) ไม่จำเป็นสำหรับ economy game
ตามที่จะแสดงด้านล่าง

---

## Three approaches considered

### A. Server-side mirror sim per instance (always-on)

```
Client ส่ง events → Server รัน sim parallel ตลอดเวลา
                  → Server ส่ง state กลับ client เพื่อแสดง
Trust = server เป็น ground truth, client ไม่มี authority
```

ใช้โดย: Diablo 3, Path of Exile, TFT (match-scope), classic MMORPGs

### B. Server batched sim (periodic catch-up)

```
Client รัน sim เอง, ส่ง event log + state hash ทุก N นาที
Server รัน sim parallel ทุก N นาที (batch), เทียบ hash
Hash mismatch = cheat suspected
```

ใช้โดย: ไม่มีตัวอย่างชัด — middle ground

### C. Replay-on-demand (GDD §9, the chosen path)

```
Client มี authority สำหรับ private progress (offline-capable)
Client บันทึก event log + snapshots + hash chain
Server idle ส่วนใหญ่
Trigger: เมื่อผู้เล่น action ที่ affect คนอื่น —
         list market / claim leaderboard / guild war prize
  → Client upload (seed + events + snapshots + final_hash)
  → Server replay sim-core ตัวเดียวกัน
  → Hash match? → allow / mismatch? → reject + flag
```

ใช้โดย: Hearthstone deck verification, TFT replay system, anti-cheat
systems ที่ verify ผลลัพธ์ไม่ใช่ทุก tick

---

## Cost comparison @ 10k MAU (~1-2k peak concurrent)

Numbers are ballpark order-of-magnitude estimates for AWS-like cloud
pricing, late 2025. They are not production-validated benchmarks.

### Cloud infra

| Mode  | CPU (sim)  | Network (state sync) | Storage (event log) | Always-on infra | Total /mo  |
|-------|------------|----------------------|---------------------|-----------------|------------|
| A     | $30-100    | $100-300 (with diff) | $25                 | $200-500        | $300-1000  |
| B     | $5-30      | $20-100              | $50                 | $100-200        | $100-300   |
| C     | $5-30      | $5-30                | $50-100             | $50-100         | $60-200    |

**CPU ของ sim เราเบามาก** (integer-only, ~1-5μs/tick) — A ทำได้แม้ราคา
ไม่บ้าระห่ำเท่าที่กลัวตอนแรก ราคาขึ้นจริงคือ network + always-on infra.

### Engineering cost (solo dev — the real killer)

| Mode  | Implementation effort | What you have to build                              |
|-------|----------------------|-----------------------------------------------------|
| A     | 6-12 เดือน           | Netcode, prediction, reconciliation, reconnect,     |
|       |                      | horizontal scale, crash recovery, anti-DDoS         |
| B     | 2-3 เดือน            | Event upload, batch verify, mismatch handling       |
| C     | ~1 เดือน             | Save/load + snapshot chain, server endpoint that    |
|       |                      | runs sim-core (Rust binary or WASM)                 |

**Engineering = ตัวที่กิน solo dev มากกว่า cloud bill หลายเท่า** — ปี
ของชีวิตที่หายไปกับ netcode คือต้นทุนที่ไม่กลับมา

### Player UX

| Aspect                          | A             | B                          | C                |
|---------------------------------|---------------|----------------------------|------------------|
| Offline play                    | ❌ ทำไม่ได้   | △ ทำได้แต่ delay sync      | ✓ first-class    |
| Action latency                  | RTT 50-100ms  | 0 (client decides)         | 0                |
| Server downtime = ผู้เล่นเล่นได้ไหม | ❌ ไม่ได้    | ✓ เล่นได้                   | ✓ เล่นได้         |
| Anti-cheat detection time       | ทันที (1 tick) | 5-30 นาที delay           | ตอน trigger      |

---

## Insight: anti-cheat ที่ "ดีพอ" คือ anti-cheat ที่ focused

A เห็นทุก tick — แต่ส่วนใหญ่ของ tick **ไม่มีอะไรต้อง verify**:

| Action                    | กระทบใคร            | ต้อง verify ไหม |
|---------------------------|--------------------|--------------|
| ฟาร์ม slime ในห้องตัวเอง   | ตัวเอง             | ไม่ — victimless |
| ตีบวก +10 sword           | ตัวเอง             | ไม่ — until trade |
| List sword on market      | ผู้ซื้อ + economy    | ใช่           |
| Claim weekly leaderboard  | ผู้เล่นอื่น          | ใช่           |
| Submit guild war build    | ทั้ง guild + คู่ต่อสู้| ใช่           |

A verify everything; C verify the things that matter. Result: เกือบ
เหมือนกันจาก player-protection มุมมอง, แต่ราคาห่างกัน 5-10×.

**Insurance analogy:** ระบบบัญชีบริษัทไม่ audit ทุก transaction ทันที —
audit ตอน withdraw + spot-check + audit หลัง quarter. Cost of "audit
everything continuously" = cost of "ทำธุรกิจ" = ไม่ทำ.

---

## Why Phase 0 work already supports C cleanly

ทุก architecture rule ที่อยู่ใน GDD §4.1 / §4.2 / §4.4 มีอยู่เพื่อ C:

| Phase 0 rule                              | Enables C how                                          |
|-------------------------------------------|--------------------------------------------------------|
| sim-core in Rust, no Godot, no I/O        | Server can link the same crate, run identical sim      |
| No floats, integer-only math              | Cross-OS bit-deterministic results (Win/Mac/Linux)     |
| Single PCG32 PRNG, seeded at new-game     | Same seed + same events = same outcome, period         |
| No HashMap iteration in gameplay logic    | Iteration order is deterministic across runs/OSes      |
| All input via `InputEvent` enum           | Event log = compact upload payload                     |
| Tick is pure w.r.t. state                 | `replay(seed, events) → final_state` is a pure function|
| Snapshots + hash chain (GDD §4.4)         | Server resumes from snapshot, doesn't run from tick 0  |
| Golden tests pin hashes across OSes (M1)  | Continuous proof that "server replay = client run"     |

**ของพวกนี้ไม่ใช่ over-engineering** — มัน is the foundation of C.
ถ้าจะเลือก A หลัง ๆ ของพวกนี้ก็ยังต้องการ (server ของ A ก็ต้องการ
deterministic sim เพื่อ verify suspicious sessions ทีหลัง). ดังนั้น
**Phase 0 work เปิดทาง C โดยไม่ปิดทาง A.**

---

## What Phase 2 actually needs (still to build, not in Phase 0)

1. **`export_log()` ใน gdext bridge** — currently absent; Phase 0 §4.1
   reserved it but ALPHA-1..7 ไม่ได้ wire up. Phase 2 จะต้อง.
2. **Save format with snapshot+events chain** — design ใน GDD §4.4
   แต่ Phase 0 alpha-first.md ข้ามไป GAMMA. ทำใน GAMMA หรือ Phase 2 ก็ได้.
3. **Server endpoint that runs sim-core** — รัน sim-core เป็น Rust
   binary หรือ compile ไป WASM แล้วโหลดใน Node/Cloudflare Worker.
4. **Hash verification + cheat-flag pipeline** — เมื่อ verify fail
   ทำอะไรต่อ (ban, soft-reject, retry, manual review).

ทั้งหมดนี้ **ไม่ใช่ Phase 0 scope**.

---

## When A becomes worth it (Phase 3+ open question)

A กลายเป็นถูกเลือกเมื่อ:

1. **Real-time PvP** — เห็นกัน sync ใน arena. ไม่มีทาง verify ทีหลัง
   เพราะ "ทีหลัง" ผู้เล่นทั้งสองคนเสีย time spent already
2. **Player count + revenue คุ้ม engineering** — 50k+ MAU paying คุ้ม
   จ้าง backend team
3. **มีทีม backend dev** — ไม่ใช่ solo อีก
4. **Shared world** — ถ้าเปลี่ยน design เป็น shared world (ขัด GDD
   pillar 1 อย่างชัดเจน) ก็ไม่มีทาง verify นอกจาก A

GDD §9 ระบุชัด **Phase 0-2 ไม่ทำของพวกนี้** (no shared-world realtime,
no realtime PvP).

---

## Decision

**Phase 0-2: C (replay-on-demand).**

- รักษา offline-first (pillar 1)
- solo-maintainable
- cost predictable ($60-200/เดือน @ 10k MAU)
- anti-cheat ครอบคลุม "ของกระทบคนอื่น" 100%
- ไม่ปิดทาง A — ถ้าเกมโตจน A คุ้ม, switch ได้เพราะ sim-core deterministic
  รองรับทั้งคู่อยู่แล้ว

**Phase 3+: revisit** เมื่อมี data จริง (MAU, ARPU, attack rate, player
expectation around real-time features).

---

## Detection signals beyond replay-verify (all feed `cheat_flags` → MANUAL review)

Replay-verify + the fast-forward cap + nonce-binding catch illegitimate RESULTS. These are
supplementary NON-invasive signals that raise the bar / catch what results-checks can't (bots,
alts, RNG-steering of not-yet-server-rolled RNG). None auto-bans — they feed the same
`cheat_flags` audit for a human. See `competition-integrity.md`, `online-robustness.md`.

**Do NOT chase client-integrity detection.** A modded client is undetectable in general (the
user owns the machine — any check is patchable/spoofable). Cheap checks (build/version hash at
login, file checksum) catch only LAZY modders (spoofable) and are the ceiling of "client
detection" — worthwhile only as low-effort casual deterrence, never a gate. Kernel anti-cheat
(EAC/BattlEye-style) is invasive, bypassable, and drives legit players away (the TBH-EULA
complaints) — it REVERSES our "we verify results, not surveil clients" advantage. Don't.

**Steam signals (when Steam auth replaces `/auth/dev`) — anti-BOT/ALT, not anti-mod:**
- **Account risk score** via Steamworks (account age, VAC-ban history `GetPlayerBans`, game
  ownership, Steam level): a fresh throwaway account + no games + anomalous behavior = a weight
  on the flag, not a ban. Useful vs bot farms / alt-evasion (a re-login-cycling cheater on
  throwaway accounts). Some fields gated by the user's privacy settings.
- **$5 + authenticator requirement** for market (Valve-enforced, `steam-market-requirements.md`)
  = raises the cost of disposable cheater/bot accounts — free anti-bot friction.
- **VAC** (opt-in per game): free, but SIGNATURE-based → catches KNOWN cheats, not a custom
  sim-speed/logic mod (no signature) → LOW value for our model (we already catch the result).
  Optional bonus (a VAC ban on the public profile has social deterrence); NOT our defense.
- **Steam auth (encrypted app ticket)** authenticates the ACCOUNT (anti-spoof), NOT client
  integrity. There is no reliable Steam "is this binary unmodified" API (VAC is the best-effort
  signature approach).

**Result-side statistical/behavioral signals (from data we already trust — no client trust):**
- **RNG-anomaly:** observed refine-success / drop-luck vs the published odds → flag outliers.
  Catches RNG-steering (shadow-sim peek) EVEN for value-RNG not yet server-rolled, statistically.
- **Behavioral:** 24/7 no-break sessions, superhumanly regular input timing (bot) — from save +
  session timestamps.

**Stance:** the primary defense stays result-side (replay/rate-cap/nonce). Steam account signals
are a cheap anti-bot ADD when Steam auth lands; VAC + build-hash are optional casual deterrence.
None replaces the result-side model, and none auto-bans.

---

## References

- `taskbar-arpg-gdd-phase0.md` §4.1 (language split — why Rust + integer)
- `taskbar-arpg-gdd-phase0.md` §4.2 (determinism rules)
- `taskbar-arpg-gdd-phase0.md` §4.4 (event sourcing + saves)
- `taskbar-arpg-gdd-phase0.md` §9 (three-tier authority model, async PvP)
- `taskbar-arpg-alpha-first-order.md` §🔒 ("ห้ามเลื่อน" list — sim Rust
  integer-only + golden test = pre-requisite for replay-on-demand)
