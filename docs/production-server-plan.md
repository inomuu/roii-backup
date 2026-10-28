# Production Server Plan — test → prod split

**Status:** วางแผน (ยังไม่ลงมือ). เขียนไว้ตอนมี test server `renrei-idle-server` (Fly, PoC) รันอยู่.
**Companion to:** `deploy-verify-server.md` (วิธี deploy test PoC), `server-plan-phases.md` (architecture S0–S4),
`online-robustness.md`, `steam-launch-plan.md`, `economy-invariants.md` (economy safety + horizontal-scale gate, §8).

เอกสารนี้ตอบคำถามเดียว: **มี test server แล้ว จะยืน production server อีกตัวยังไงโดยไม่ปนกัน.**

---

## 1. หลักการ

โค้ด/binary **ตัวเดียวกัน** (commit เดียวกัน). ต่างกันแค่ **app + database + config + secrets** เป็นคนละชุด.
prod ไม่ใช่ "โปรเจกต์ใหม่" — มันคือ **environment ที่สอง** ของโค้ดชุดเดิม.

**กฎเหล็ก:**
- test กับ prod **ห้ามใช้ DB ร่วมกัน** — test เขียนมั่ว/ลบล้างได้ ต้องไม่แตะข้อมูลผู้เล่นจริง
- **dev/cheat routes ต้องปิดใน prod** (DEV_KEY, `/flags`, trigger-replay ฯลฯ)
- deploy เข้า **test ก่อนเสมอ** → smoke ผ่าน → ดัน **commit เดียวกัน** ขึ้น prod (promote ไม่ใช่ build ใหม่)

test server = ทำหน้าที่ **staging** ไปในตัว.

---

## 2. สิ่งที่แยกเป็นคนละชุด (test ↔ prod)

| ด้าน | test (`renrei-idle-server`, ตอนนี้) | prod (`renrei-idle-prod`, จะทำ) |
|---|---|---|
| **Fly app** | `renrei-idle-server` | app ใหม่ (`fly launch --name renrei-idle-prod`) |
| **config file** | `fly.toml` | `fly.prod.toml` (deploy ด้วย `-c`) |
| **machines** | `min_machines_running = 0` (cold start 1–2s ยอมได้) | `= 1`+ (ไม่หลับ); เพิ่ม region ถ้าคนกระจาย |
| **VM** | shared-cpu-1x / 256mb | ใหญ่ขึ้นตามโหลด + วางแผน autoscale |
| **database** | Neon project (test) | Neon project **แยก** + เปิด backup/PITR |
| **DEV_KEY / dev_routes** | เปิด | **ปิด** (ดู §4) |
| **Steam auth** | PoC / mock | verify Steam ticket จริง |
| **secrets** | `fly secrets` บน test app | ชุดแยกบน prod app |
| **logs/alert** | อ่าน `fly logs` มือ | monitoring + alert (ดู §6) |

---

## 3. ขั้นตอนยืน prod (one-time)

```bash
# 1) app ใหม่ + config แยก (อย่า deploy ทันที)
fly launch --copy-config --no-deploy --name renrei-idle-prod --region sin
#    → คัดลอก fly.toml เป็น fly.prod.toml, แก้ app=, min_machines_running=1, vm ใหญ่ขึ้น

# 2) DB prod แยก (Neon project ใหม่) → ได้ connection string
#    เปิด PITR / branching ของ Neon ไว้ (rollback ข้อมูลได้)

# 3) secrets ชุด prod (คนละ app — ไม่ inherit จาก test)
fly secrets set DATABASE_URL='<prod neon url>' -a renrei-idle-prod
fly secrets set PROD=1 -a renrei-idle-prod          # gate ปิด dev routes (§4)
#    ⚠️ อย่า set DEV_KEY บน prod

# 4) deploy commit เดียวกับที่ผ่าน test แล้ว
fly deploy -c fly.prod.toml -a renrei-idle-prod
```

---

## 4. ปิด dev/cheat routes ใน prod (สำคัญสุด) — ✅ BUILT 2026-09-29

`main.rs::gated_routes` แยก route ที่มี gate เป็นสองกลุ่ม:

| กลุ่ม | routes | gate | บน `PROD=1` |
|---|---|---|---|
| **test fixtures** | `/auth/dev`, `/dev/gacha-crystal`, `/dev/steam-mint-test`, `/mail/claim`, `/demo/bundle`, `/verify`, `/ingest`, `/submit` | `X-Dev-Key` = `DEV_KEY` | **ไม่ mount เลย (404)** และ `DEV_KEY` ถูกเพิกเฉย |
| **ops** | `/store/refund`, `/dev/mail-send` (ชดเชย), `/dev/bp-season`, `/flags`, `/dev/storage`, `/dev/balance` | `X-Admin-Key` = `ADMIN_KEY` (นอก prod รับ `X-Dev-Key` ด้วย) | ใช้ได้ด้วย `ADMIN_KEY` เท่านั้น |

- ไม่ตั้ง `ADMIN_KEY` = ops routes ตอบ 503 (fail closed)
- เทียบ key ด้วย blake3 digest (ไม่รั่วเวลาตามจำนวน byte ที่เดาถูก)
- **ขั้นตอนเปิดบน prod:** `fly secrets set ADMIN_KEY=<สุ่มใหม่> PROD=1` แล้ว `fly secrets unset DEV_KEY` — เก็บ
  `ADMIN_KEY` ไว้ที่ password manager ไม่ใช่ไฟล์ใน repo. หลังเปิด `PROD=1` ประตู dev login (`ROI_BOOT=online`)
  ยิง prod ไม่ได้อีก — ใช้ Steam login หรือ local server แทน
- **ลืมไม่ได้:** `STEAM_MTX_LIVE=1` (เงินจริง) คู่กับไม่มี `PROD=1` หรือยังมี `DEV_KEY` → server **ไม่ยอมบูต**
  (`live_money_misconfig`). `/version` รายงาน `prod` / `money_live` และ `check_online_sync.sh` แสดงทุกครั้งที่ deploy
- **ops tools (ADMIN_KEY):** `/admin/ban` `{account_id, hours?, reason}` (ไม่ใส่ hours = ถาวร, จบ session ทันที,
  login ถูกปฏิเสธ) · `/admin/unban` · `/admin/bans` · `/admin/store/grant` `{order_id, force?}` (แจก order ที่จ่ายแล้ว;
  `pending` ต้อง `force` หลังเช็กใน Steam partner ว่าตัดเงินจริง). Steam login ปฏิเสธ `publisherbanned`; VAC แค่ log
- **จ่ายแล้วไม่ได้ของ:** order ที่ Steam ตัดเงินแล้วแต่ grant ล้มค้างที่ `charged` → client retry / sweep ทุก 5 นาที /
  admin grant — ทุกทางใช้ `grant_order` ตัวเดียว จ่ายครั้งเดียวเสมอ
- ยังไม่ได้ทำ: จำกัด ops routes ให้เรียกได้จาก private network (`fly proxy`) เท่านั้น
- บทเรียนเดิม: route ของผู้เล่นต้อง auth-gated ไม่ใช่ key-gated (commodity `/cancel` เคยพลาด)

---

## 5. Data safety (prod เท่านั้น)

- **backup/PITR** เปิดบน Neon prod — restore ย้อนเวลาได้เมื่อ migration พัง / ข้อมูลเสีย
- **migration** รันแบบระวัง (backward-compatible ก่อน, ค่อย drop) — version-gate ที่มีอยู่ช่วย reject
  save จาก sim_version ที่ไม่เข้ากันได้แล้ว
- save + trusted anchor + mail claims + consumed_ops เขียนใน tx เดียว (ตลาด Gil ถูกลบแล้ว 0.0.490)
- **MED-2 (market_ops unbounded growth)** ต้องปิดก่อน/ระหว่าง scale prod จริง (consumed-op ack)
- **ห้ามหมุน `REFINE_SECRET` บน prod** (2026-09-29): nonce ของ drop / craft / gear / commander / fusion
  คำนวณจาก secret นี้ ทุกบัญชีที่ถือ grant ค้างใน log จะโดน `forged-*-grant` flag พร้อมกันทันที
  ถ้าจำเป็นต้องหมุน ต้องมีขั้นตอน drain grant ที่ค้างก่อน (ยังไม่ได้เขียน)
- **flag ผิดพลาด:** `/flags` แสดง `detail` (sim/catalog version + `X-Client-Platform`) ของ `hash-mismatch`
  / `not-anchored` — flag ที่กระจุกที่ build หรือ platform เดียว = บั๊กของเรา ไม่ใช่คนโกง. รับ mail
  ของตัวเองซ้ำ (inbox ค้าง) = ปฏิเสธ save แต่ไม่ flag

---

## 6. Observability + rollback

- **health check** มีแล้ว (`GET /health` ทุก 15s ใน fly.toml) — prod เพิ่ม alert เมื่อ fail
- **metrics/logs**: Fly metrics + `fly logs`; ต่อ Grafana/external ถ้าต้องการ dashboard
- **rollback**: `fly releases -a renrei-idle-prod` → `fly deploy --image <digest เก่า>` (pin image เดิม)
- **version probe**: `GET /version` คืน `sim_version` — ใช้ยืนยันว่า rollout ถึงจริง (เช็คทุก deploy)

---

## 7. ลำดับที่แนะนำ (เริ่มเมื่อไหร่)

prod จริง **ยังไม่ต้องรีบ** — ทำหลัง demo/เกมพิสูจน์ความสนุก (ตาม server-plan-phases §1).
ลำดับก่อนเปิด prod ให้ผู้เล่นจริง:

1. ปิด dev_routes ด้วย `PROD` gate (§4) — ทำได้เลย ถูกและกันพลาด
2. Steam auth จริง (แทน PoC)
3. Neon prod + backup/PITR + `fly.prod.toml`
4. MED-2 (prune market_ops)
5. monitoring/alert + rollback drill (ซ้อม 1 รอบ)

จนกว่าจะถึงตรงนั้น: **test server ตัวเดียวพอ** — ใช้เป็นทั้ง dev + staging + demo-online.

---

## 8. Scaling & capacity

**หลักการ: vertical ก่อน, horizontal ติด gate.** โตด้วยการขยายเครื่อง (แกน 1) ให้สุดก่อน
แล้วค่อยแยกหลายเครื่อง (แกน 2) ซึ่งมี correctness gate ขวางอยู่.

**ทำไมเราสเกลได้ดี:** เป็น client-sim + server-verify ไม่ใช่ realtime authoritative — งานต่อ request
จึง**เบาและมีเพดาน**. client idle 1 คน save แค่ ~ทุก 10s (~0.1 req/s), แต่ละ request คือ Postgres
write เล็กๆ + replay (ปกติ ~sub-ms, worst case cap ที่ `MAX_REPLAY_SPAN` = 50k ticks ≈ 0.18s).
เทียบคร่าวๆ: 10k CCU ≈ 1k req/s, 30k CCU ≈ 3k req/s — **I/O-bound ไม่ใช่ CPU-bound**.
คอขวดจะมาตามลำดับ: (1) RAM/CPU ของ VM เดียว → (2) Postgres write throughput + pool → (3) economy lock (แกน 2).
**ตัวเลขข้างบนเป็น intuition — ต้องวัด req/s จริงก่อนเชื่อ.**

### แกน 1 — Vertical (ขยายเครื่อง): ง่าย, ทำเมื่อโหลดขึ้น

- เปลี่ยน `fly.prod.toml`: shared-cpu-1x/256mb → dedicated-cpu + หลาย GB. ไม่ต้องแก้สถาปัตยกรรม.
- ⚠️ **`MAX_INFLIGHT` (crates/verify-server/src/main.rs) ต้องขึ้นตาม RAM.** มันคือ OOM guard
  (load-shed 503): worst-case memory ≈ `MAX_INFLIGHT × 2MB body`. ที่ 256mb VM ตั้งไว้ **64** (≈128MB).
  ขึ้น VM แล้วลืมขยับ cap = shed เร็วเกินจำเป็น; ขยับ cap โดยไม่ดู RAM = เสี่ยง OOM (= outage, panic=abort).
- VM เดียวที่ใหญ่พอ**น่าจะรับหลายหมื่น CCU ได้** เพราะ request เบา — แกนนี้ซื้อเวลาได้เยอะ ควรใช้จนสุดก่อน.

### แกน 2 — Horizontal (หลายเครื่อง): ต้องปลดล็อก economy ก่อน

**Gate อยู่ที่ `economy-invariants.md` (§ market state) + `fly.toml` ("ONE MACHINE ONLY"):**
market ledger เป็นโครงสร้างใน memory ล็อกแบบ **process-local**. เครื่องที่สองได้สำเนาของตัวเอง →
settlement สองอันชนกันโดยไม่เห็นล็อกของกัน = **double-mint**. ฉะนั้นก่อนแยกเครื่องต้อง **ย้าย
market state ลง Postgres (advisory lock / `SELECT...FOR UPDATE`) ก่อน ไม่ใช่หลัง** — และปิด MED-2
(§5) ด้วย. รายละเอียด/เหตุผลอยู่ใน `economy-invariants.md`, อย่าเขียนซ้ำที่นี่.

> **อัปเดต 0.0.490 — market ledger ถูกลบแล้ว** (Gil market ยกเลิก 2026-09-16): ไม่มี order book / item listings / market lock ใน memory อีก, ตาราง market ถูก DROP; `/save` settle แค่ mail claim (rows ใน `mailbox` + `consumed_market_ops`, transaction เดียว). เหตุผล double-mint ข้างบนจึงหมดไป **แต่ `fly.toml` ยังคง ONE MACHINE** — session pinning / tick anchor / fuse rolls ยังเป็น state ใน memory ของ process; ต้องประเมินส่วนนั้นก่อนแยกเครื่อง.

**สำคัญ:** ที่ผูกกับ "เครื่องเดียว" มีแค่ **economy/market**. ส่วน verify/save/anchor เป็น stateless
แยกเครื่องได้ง่ายกว่า → อนาคตน่าจะเป็น fleet ของ verify/save (สเกลตามใจ) หน้า economy authority
ตัวเดียว (หรือ Postgres-backed lock).

### DoS / load-shed layer (มีในโค้ดแล้ว)

3 ชั้นใน `main.rs` (application-level; volumetric L3/L4 เป็นงาน CDN/Cloudflare — จงใจ defer, ดู
`anti-cheat-and-server-model.md`): **`rate_limit`** per-IP 120 req/60s → **`concurrency_limit`**
global shed 503 ที่ `MAX_INFLIGHT` → **`request_timeout`** 30s. onion order: rate → concurrency →
timeout → handler. Chat WS แยกต่างหาก (socket ยาว middleware เห็นแค่ handshake): **`MAX_CHAT_CONNS`**
(total) + **`MAX_CHAT_CONNS_PER_ACCOUNT`** (กันคนเดียวยึดทุก slot / คูณ per-connection anti-spam).

### ลำดับ

vertical จน VM เดียวไม่ไหวจริง (วัดจาก metric) **ก่อน** ค่อยลงทุนแกน 2 — เพราะแกน 2 คือการรื้อ
economy ซึ่งเป็น safety spine, เสี่ยง double-mint ถ้าทำพลาด. อย่าทำก่อนมี trigger ชัด.
