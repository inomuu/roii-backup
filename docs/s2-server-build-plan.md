# S2 — Server ตัวแรก: Build Plan (ละเอียด)

**Status:** วางแผน (ยังไม่ลงมือ) — 2026-07-07.
**Parent:** `docs/server-plan-phases.md` (S0–S4, decisions log). อ่านตัวนั้นก่อนเพื่อ context.
**Prereq ที่เสร็จแล้ว:** S0 (ทุก mutation = InputEvent) + S1a/b/c (event-log recording +
`replay::SaveBundle::verify()` + hash-chain snapshots). **แกน replay-verify พร้อม + พิสูจน์ใน
เทสต์แล้ว** — S2 คือการ "ห่อ backend" รอบแกนนี้ ไม่ใช่สร้างแกนใหม่.

> เอกสารนี้คือ HOW ของ S2. หลักการ (why) อยู่ที่ server-plan-phases §2-4; S3 (economy) มี
> safety spine แยกที่ `docs/economy-invariants.md` — **S2 ห้ามแตะเงิน/market เลย** (แค่ auth +
> save + verify). Money layer เริ่ม S3.

---

## 0. ข้อสังเกตที่ทำให้ S2 "เล็กกว่าที่คิด"

- **ส่วนยากที่สุดเสร็จแล้ว.** `sim-core` เป็น Rust ไม่มี I/O → compile เป็น binary ฝั่ง server
  ได้ตรงๆ. `SaveBundle::verify(&data) -> bool` (replay log ทับ start snapshot, เช็ค final hash
  + ทั้ง chain) **คือฟังก์ชันที่ server เรียก** ("one binary, two homes", GDD §9).
- **Model C — replay-verify, ไม่ใช่ server-run sim.** server ไม่รัน combat realtime; มันแค่
  re-run โค้ดเดียวกันตอน trust ต้องการ → CPU ถูกมาก. อย่าไถลไปทาง A (server-run sim = 6-12
  เดือน netcode) — เหตุผลเต็มใน server-plan §2.
- **Trust = players re-run identical code.** ความถูกต้องข้าม OS พิสูจน์โดย golden suite
  (Win+Linux hash ตรง, GDD §4.2 rule 7). server (Linux) verify client (Windows) ได้เพราะ
  `sim-core` bit-deterministic — โดยต้อง build จาก **VERSION เดียวกัน** (bundle เก็บ
  `sim_version` + `game_data_version` แล้ว → เช็คก่อน replay).

---

## 1. Tech stack (ตัดสินแล้ว — ตรงกับดีไซน์)

| ชั้น | เลือก | เหตุผล |
|---|---|---|
| ภาษา server | **Rust** | link `sim-core` เป็น crate ตรงๆ → hash ตรงกับ client รับประกัน. ภาษาอื่นต้อง WASM/FFI ยุ่งกว่าและเสี่ยง divergence |
| web framework | **axum + tokio** | มาตรฐาน Rust, async, ecosystem ครบ |
| DB | **Postgres (managed)** | Neon / Supabase — transactional (จำเป็นตอน S3 ledger), managed = ไม่ต้องดูแล |
| blob (bundle/snapshot) | **object storage (S3-compatible)** | bundle ใหญ่กว่า row ปกติ → เก็บใน R2/S3, เก็บแค่ pointer+hash ใน Postgres |
| auth | **Steam** (encrypted app ticket → verify ผ่าน Steam Web API) | เกมลง Steam อยู่แล้ว; ticket = ตัวตนที่ปลอมยาก |
| hosting | **เริ่ม single region, managed compute** (Fly.io / Railway) | solo dev, scale ทีหลัง. ตัวแปรจริงคือ eng-months ไม่ใช่บิล cloud (§3.5) |

**Deploy shape:** binary เดียว (axum + sim-core) + managed Postgres + object storage. ไม่มี
microservices — โมโนลิธเล็กจน deploy ได้ใน 1 คำสั่ง.

---

## 2. Build order — vertical slices (แต่ละอันรันได้/เทสต์ได้จบในตัว)

หลัก: **อย่าสร้าง 5 ชิ้นพร้อมกัน.** ทำทีละ slice บางๆ ทั้งเส้น. ลำดับนี้จัดให้ของที่พิสูจน์
architecture มาก่อน ของที่เป็น "งาน backend ธรรมดา" มาทีหลัง.

### Slice 0 — verify-server ✅ DONE 2026-07-07 (0.0.167 era)
crate `crates/verify-server` (axum + tokio + sim-core) — RUN จริงแล้ว: GET /health,
GET /demo/bundle (สร้าง bundle จริงจาก sim), POST /verify (version check + reject
Debug events + `SaveBundle::verify`). curl พิสูจน์ live: honest→valid; ปลอม
final_hash / mid-checkpoint / GrantLoot(debug) / version → reject หมด (แต่ละด่านของ
S0/S1/security ทำงาน). `GAME_DATA_PATH` + `BIND` env. **เดิม:**
crate ใหม่ `crates/verify-server` (workspace member) link `sim-core`.
```
POST /verify   body = SaveBundle JSON
  1. โหลด game_data.json ครั้งเดียวตอน start (Arc<GameData>)
  2. เช็ค bundle.sim_version == VERSION && game_data_version ตรง (ไม่ตรง → 409 version skew)
  3. bundle.verify(&data)          ← โค้ดที่มีแล้ว
  4. → { valid: bool, final_tick, checkpoints: N }
GET /health → 200
```
**DoD:** รัน local, `curl` ยิง bundle จริงจาก `export_log()` เข้าไป → verify ผ่าน; แก้ 1 byte
ใน log → verify false. (ยืม test ใน `replay.rs` มาเป็น integration test ยิงผ่าน HTTP.)
พิสูจน์: server รัน sim-core headless + verify client bundle ได้จริง. **ที่เหลือคือห่อรอบนี้.**

### Slice 0.5 — client wiring (S1 leftover, จำเป็นก่อนมี server ใช้จริง)
- `sim_driver.gd`: ตอน save เขียน `bridge.export_log()` bundle ลง `user://` (ข้าง save_json เดิม);
  เพิ่ม flow ส่ง bundle ขึ้น server (HTTPPost).
- **full-STATE per snapshot** (S1 leftover #2): เปลี่ยน `Snapshot` ให้เก็บ state เต็ม (option)
  เพื่อให้ server resume จาก snapshot ล่าสุดไม่ต้อง replay จาก tick 0 → verify แบบ incremental.

### Slice 1 — Auth
- ✅ **PLACEHOLDER DONE 2026-07-07** — `POST /auth/dev {name}` → `{token, account_id,
  name, seed}` (server-chosen seed = seed-grinding ตาย); `GET /me` (Bearer) → account
  / 401. In-memory sessions (AtomicU64 + Mutex<HashMap>). curl พิสูจน์: login→token+
  seed, /me ผ่าน/401, 2nd login = account+seed คนละตัว. **Steam = สลับ handler เดียว.**
- `POST /auth/steam` body = Steam encrypted app ticket → verify กับ Steam Web API
  (`AuthenticateUserTicket`) → ออก **session token** (JWT ระยะสั้น) + สร้าง/ดึง account row.
- **server-seeded RNG:** login แจก `seed` (server สุ่ม, ผูกกับ account+session) → new-game ใช้ seed
  นี้ → **seed-grinding ตาย** (client เลือก seed เองไม่ได้). client `init_game(seed=server_seed)`.
- **DoD:** login ด้วย Steam ticket จริง → ได้ token + seed; token หมดอายุ/ปลอม → 401.

### Slice 2 — Cloud save (authoritative)
- ✅ **IN-MEMORY DONE 2026-07-07** — `POST /save` (auth): push bundle → policy_check +
  `SaveBundle::verified_state` → เก็บ **final state ที่ replay ได้** เฉพาะที่ verify ผ่าน
  (bundle ปลอม → reject ไม่เก็บ → authoritative save ไม่มีวันมี state เถื่อน). `GET /save`
  (auth) → StoredSave (state_json ให้ client load_json) / 404. curl พิสูจน์: resume-before
  → 404, push → stored, no-token → 401, tampered → not stored, resume → state ที่ REPLAY
  ได้ (zone/tick ไม่เชื่อ snapshot client). สำคัญ: `verified_state` เพิ่มใน sim-core.
- **เหลือ:** ย้าย store จาก `Mutex<HashMap>` → **Postgres** (row: account → verified
  snapshot + chain head + bundle pointer; blob → object storage).
- Postgres: ตาราง `saves` — ต่อ account เก็บ **latest verified snapshot + chain head + bundle
  pointer**. save ตัวจริงอยู่ server → rollback/dupe ตาย (client แก้ไฟล์ local ไม่มีผล).

### Slice 3 — Ingest + hash pinning
- client stream `RecordedInput` segments + snapshot chain heads เป็นระยะ (idle input เบามาก
  — ไม่กี่ byte/นาที). server **pin chain_hash ต่อเนื่อง (monotonic)** — reject segment ที่
  diverge หลัง hash ที่ pin แล้ว (นี่คือที่ hash-chain S1c ได้ใช้จริง).
- pin = ถูกมาก (ไม่ต้อง replay). replay จริงเฉพาะ trigger ใน Slice 4.
- **DoD:** stream ปกติ → pin ไล่ไปเรื่อยๆ; ยัด segment ปลอม → reject ที่จุด diverge.

### Slice 4 — Verifier triggers (replay เฉพาะที่จำเป็น)
replay จริง (กิน CPU) **เฉพาะ**: (§3) segment ที่มี tradeable-class drop / สุ่มตรวจ (audit %) /
น่าสงสัย (pin diverge, timing ผิด) / จุด export. บัญชีที่ฟาร์มของ bound ล้วน **แทบไม่กิน CPU**.
- queue + worker: หยิบ verify job → `SaveBundle::verify` จาก snapshot ที่ pin ล่าสุด (incremental).
- **DoD:** วัดได้ว่า X% ของ session ถูก replay, ที่เหลือแค่ pin. ต้นทุน CPU = f(tradeable drops).

### Slice 5 — Graceful degradation (ออกแบบก่อนเขียน, §3.4)
- server ล่ม → **เกมเล่นต่อ local** (grace window), ปิดชั่วคราวเฉพาะ social/market, reconcile
  ตอน reconnect (determinism ให้ฟรี). incident ตี 3 = "เทรดหยุด แก้พรุ่งนี้" ไม่ใช่ "เกมดับ".
- net สะดุด → client sim ต่อได้, ส่ง segment ย้อนหลังตอน reconnect.
- **offline-earnings attestation:** server attest wall-clock ตอน login → fast-forward zero-input
  → verify ด้วย code path เดียวกับ catch-up.
- **DoD:** kill server ระหว่างเล่น → เกมไม่ค้าง; เปิด server กลับ → reconcile ตรง.

---

## 3. Data model (Postgres, ร่าง)

```
accounts        (id, steam_id, created_at, ...)
sessions        (id, account_id, session_seed, token_jti, expires_at)
saves           (account_id PK, latest_snapshot_ptr, chain_head_hash, final_tick,
                 sim_version, game_data_version, updated_at)
event_segments  (id, account_id, from_tick, to_tick, chain_head_hash, blob_ptr, pinned_at)
verify_jobs     (id, account_id, segment_id, reason, status, result, verified_at)
-- S3 เพิ่มทีหลัง: ledger, escrow, mint_log (ห้ามแตะใน S2)
```
blob (bundle/snapshot/segment) → object storage; Postgres เก็บ pointer + hash เท่านั้น.

---

## 4. สิ่งที่ S2 **ไม่ทำ** (กัน scope creep)

- ❌ economy / market / mint / เงินจริง → **S3** (มี `economy-invariants.md` เป็น gate;
  online closed beta ใช้ Gil จริงปิด real-money ซ้อมก่อน)
- ❌ social / เมือง shared / PvP / boss race → **S4**
- ❌ server-run realtime sim (โมเดล A) → ห้ามเด็ดขาด (§2)
S2 = **auth + cloud-save + replay-verify backbone เท่านั้น.** จบแค่นี้ = "online-required ที่
เล่นได้จริง ปลอดภัยจาก forged state" — พอสำหรับ online closed beta (ยังไม่มีตลาด).

---

## 5. Milestones (~2-3 เดือน, solo)

- **M1 — verify-server local** (Slice 0 + 0.5): server verify bundle จริงได้ + client เขียน/ส่ง
  bundle. *สัปดาห์แรกๆ — พิสูจน์ architecture.*
- **M2 — auth + cloud-save** (Slice 1-2): login Steam + save อยู่ server + server-seeded RNG.
  = **online closed alpha** (login + save survives + verify manual).
- **M3 — ingest + verify triggers** (Slice 3-4): stream + pin + replay เฉพาะจุด. = anti-cheat
  ทำงานอัตโนมัติ.
- **M4 — degradation + offline + harden** (Slice 5): server ล่มไม่ทำเกมดับ + offline earnings.
  = **online closed beta** พร้อม (ยังไม่มีเงินจริง → S3).

---

## 6. Decisions

### 6.1 HOSTING / STORAGE — DECIDED 2026-07-07

**หลักการ: compute = stateless/ถูก/สลับง่าย (Model C) → ทุ่มความระวังที่ STATE (DB+blob).
Commit 1 platform ตั้งแต่แรก + วินัย PORTABILITY (ไม่ใช่ multi-cloud).**

| ชั้น | เลือก | เหตุผล / วินัย portability |
|---|---|---|
| **compute** | **Fly.io** (region สิงคโปร์ nrt/sin) | Rust binary, always-on (ไม่ cold-start), `fly scale count/regions` = scale ง่าย; stateless → horizontal trivial. **ผูก Fly ต่ำ** เพราะแค่ Dockerfile + env |
| **DB** | **Neon** (managed Postgres, PITR+backup เปิด) | truth ปัจจุบัน + **ledger เงิน (money-zone, backup ห้ามพลาด)**; INDEPENDENT (ไม่ใช้ "Fly Postgres" → ย้ายได้) |
| **blob** | **Cloudflare R2** ผ่าน **S3-compatible API** | **zero egress fee** (game stream save/replay = egress เยอะ → S3 bill บาน); S3-compat → swap R2/B2/S3/MinIO ได้ |
| **auth** | Steamworks Web API (encrypted app ticket) | ปลอมยาก ผูก session; placeholder `/auth/dev` ก่อน |

**storage split:** PG = account/session/ledger/pin-head/**latest verified snapshot**+hash
(เล็ก hot transactional) · R2 = event-log segments/bundles/archived snapshots (append-only,
โตเรื่อยๆ, อ่านนานๆที) — **compress (zstd ~10x) + prune log ก่อน trusted snapshot ล่าสุด**
เพื่อจำกัดการโต. blob → R2, PG เก็บ pointer+hash.

**วินัย 3 ข้อ (ให้ย้าย host ได้ใน weekend เดียว):** (1) Dockerfile + config ผ่าน env (12-factor);
(2) state ใช้ของ independent (Neon/R2 ไม่ใช่ Fly-native); (3) ห้าม proprietary API เข้า logic.
**อย่าทำ:** multi-cloud/k8s/abstraction เผื่อ scale ที่ยังไม่มี = YAGNI, ดูด eng-months.

### 6.2 Open (ตัดสินตอนเริ่ม slice)

| เรื่อง | ตัวเลือก | rec |
|---|---|---|
| Replay ข้าม VERSION | multi-verifier / checkpoint-cut ตอน upgrade | checkpoint-cut (ง่ายสุด solo) — bundle เก็บ sim_version แล้ว |
| Mismatch policy | ban / soft-reject / flag+review | flag+review ช่วงแรก (false-positive จาก bug ตัวเองเยอะแน่) |
| grace window ยาว | นาที → ชม. | สั้นพอกันโกง ยาวพอ wifi สะดุด — ตัดสินตอน Slice 5 |

---

## 7. ต้นทุน (sketch)

managed Postgres (ฟรี-ต่ำ tier ตอนเริ่ม) + compute เล็ก (verify ไม่ realtime) + object storage
(ถูก). **ตัวแปรจริง = eng-months + ops/on-call ไม่ใช่บิล cloud** (§3.5). Model C + verify-เฉพาะ-
tradeable ทำให้ CPU โตตาม "ของที่เทรดได้" ไม่ใช่ "จำนวนผู้เล่นที่ฟาร์ม" → ถูกโดยดีไซน์.
