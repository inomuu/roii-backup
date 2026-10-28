# 💰 Economy Invariants + Steam Boundary — THE safety spine of S3

> **draft 2026-07-05 · ยังไม่ build** — ชั้น money/trade คือ**จุดเดียวในเกมที่
> careless mode ห้ามใช้**. sim เรา vibecode ได้เพราะมี determinism + golden +
> replay-verify คอยจับ (`sim-render-split-enables-vibecoding`); ชั้นเศรษฐกิจต้องมี
> **safety net เทียบเท่า = invariant harness** ก่อนแตะเงินจริง.
>
> **หลักการกำกับ doc นี้ (user 2026-07-05):** "AI อาจจะเขียนโดยฉันไม่ได้ตรวจ
> ละเอียด" → คำตอบไม่ใช่ "ตรวจให้หมด" (จะไม่ตรวจอยู่ดี) แต่คือ **test คือตัวตรวจ
> แทน** — คุณตรวจแค่ **list invariant ~10 ข้อ** (ประโยค "เงินห้าม X") ไม่ใช่
> 5,000 บรรทัด. ตรวจ 10 กฎทำได้จริง; ตรวจทุกบรรทัดทำไม่ได้.
>
> **Companion:** `server-plan-phases.md` (S3), `economy-antibot-brief`(memory),
> `steam-market-banana-vision`(memory), `semi-mmo-direction`(memory — online-required)

---

## 0. Timeline ที่ doc นี้รับใช้ (ตกผลึก 2026-07-05)

1. **Online closed beta** — server + **Gil market จริง, ปิด real-money** — invariant
   harness วิ่งเก็บ mismatch ภายใต้ผู้เล่น concurrent จริงเป็นสัปดาห์ = ที่เดียวที่
   transfer-tech ถูกซ้อมก่อนเงินจริงแตะ. (local beta ซ้อม gameplay ได้ แต่**ไม่แตะ
   เศรษฐกิจ** — ต้องเป็น online beta ถึงจะ de-risk ชั้น money.)
2. **Public launch** — ทั้งสองตลาด **build+ship day 1**, real-money หลัง **feature
   flag ปิด** → Gil soak ~2-3 วัน → **flip เมื่อ signal เขียว ไม่ใช่ครบวัน**.
   (beta ดูดความเสี่ยงไปแล้ว → soak launch เหลือเป็น final smoke check.)

**หมุดสร้าง:** invariant harness ต้องเสร็จ **ก่อน closed beta** (ไม่ใช่ก่อน launch) —
เพราะ beta คือที่มันเริ่มทำงานจริง.

---

## 1. Steam Boundary Contract — โค้ดเราจบตรงไหน / Valve เริ่มตรงไหน

แยกให้ชัดเพราะมันบอกว่า **"test ได้แค่ไหน"**: exploit จริงอยู่ฝั่งเรา (test ได้);
การเคลื่อนเงินจริงเป็นของ Valve (ไม่ต้องเราซ้อม).

```
[ ในเกม (sim + ledger เรา) ]          |  [ Steam / Valve — hardened ]
drop → escrow → verify → MINT ─────►  |  Steam Inventory item (granted)
   (โค้ดเรา — จุด dup exploit)         |     └─► Community Market listing
   ต้อง idempotent + conserved         |         ├─ wallet funds
   ═══ BOUNDARY ═══════════════════►  |         ├─ ค่าธรรมเนียม 30%
                                       |         └─ escrow เงิน + payout
```

- **ฝั่งเรา (ต้อง get right + testable ใน Inventory Service dev/sandbox):**
  mint-on-verify, grant เข้า inventory, idempotency, conservation, no-dup.
  **exploit ปั๊มของ/mint ซ้ำ อยู่ตรงนี้ทั้งหมด.**
- **ฝั่ง Valve (ไม่ต้องเราซ้อม — production-hardened เป็นสิบปี):** การขายจริง,
  wallet, fee, escrow เงิน. โค้ดเราจบที่ "ใส่ item ที่ verify แล้วเข้า inventory".
- **สรุป:** ส่วน "เงินจริง" ที่น่ากลัวส่วนใหญ่ไม่ใช่ความรับผิดชอบเรา — โค้ดเรา
  รับผิดชอบแค่ถึง boundary, และ boundary นั้น test ได้.

---

## 2. THE INVARIANTS — "เงิน/ของ ห้าม X" (นี่คือ list ที่ user ตรวจ)

แต่ละข้อ = machine-checkable, มี test บังคับ, fail = **หยุดเทรด (fail-safe)**.
ถ้าเพิ่ม feature เศรษฐกิจใหม่ ต้องไม่ทำข้อใดข้อหนึ่งพัง.

| # | Invariant | นิยามตรวจได้ |
|---|---|---|
| **I1** | **Conservation** — เงินโผล่เองไม่ได้ | Σ minted == Σ ในทุก balance + ทุก listing + ทุก escrow เสมอ (ทุก tick/ทุก txn) |
| **I2** | **No negative** | ทุก balance ≥ 0, ทุก quantity ≥ 0 — ตลอดเวลา ไม่ใช่แค่ปลาย txn |
| **I3** | **Atomic trade** | trade สำเร็จ**ทั้งคู่**หรือ rollback**ทั้งคู่** — ไม่มีสถานะครึ่ง ๆ (ให้ของแล้วไม่ได้เงิน / ได้เงินแล้วไม่เสียของ) |
| **I4** | **Idempotent mint** | verify item เดิม N ครั้ง → mint **ครั้งเดียว**. replay event เดิม → ไม่ปั๊มซ้ำ (มี dedup key ต่อ mint) |
| **I5** | **No double-spend** | item/เงิน ก้อนเดียว list/โอน/ใช้ ได้ครั้งเดียว — lock ก่อน list, ปล่อยเมื่อ cancel |
| **I6** | **Mint ต้องมี provenance** | ทุก mint สืบกลับไป drop event ที่ replay-verify ผ่าน — mint ที่ไม่มีต้นทาง = reject |
| **I7** | **Sink บันทึกครบ** | เงิน/ของที่ถูกเผา (fee, orb ใช้, fusion ที่ fail) หายจาก Σ อย่างถูกบัญชี — ไม่ใช่หายเงียบ (กัน I1 พังแบบซ่อน) |
| **I8** | **Reconciliation = fail-safe** | job ตรวจ ledger เรา vs Steam inventory เป็นระยะ → **mismatch = หยุดเทรดอัตโนมัติ** ไม่ใช่ปล่อยผ่าน (fail-closed ไม่ fail-open) |
| **I9** | **Rate/quota บังคับที่ ledger** | daily soft-cap ต่อบัญชี + smooth-rate ของ tradeable-drop class บังคับตรงจุด mint ไม่ใช่แค่ UI (กันบอท/multi-account ตาม anti-bot brief) |
| **I10** | **Real-money หลัง flag** | promote Gil→Steam ทำได้เฉพาะเมื่อ flag เปิด + signal เขียว — ไม่มี path อื่นที่ item เข้า Steam inventory ได้ |
| **I11** | **Bucket separation (crystal)** | crystal **ไม่แปลงข้ามถัง**ทั้งสองทิศ: `paid` (ซื้อด้วยเงิน) กับ `bound` (faucet/pass/โบนัส) อนุรักษ์**แยกกันคนละถัง** — Σ ต่อถังต้องตรงเสมอ. pull หัก bound ก่อนแล้วค่อยไหลไป paid ได้ แต่ห้าม "แปลง" |

**หมายเหตุ I11 (gacha, 2026-07-30):** กาชา**สืบทอด** I1-I7 ทั้งชุดเมื่อ crystal wallet เป็น ledger
จริง (`crystal_wallet` + `crystal_ledger`, per-row ไม่ใช่ JSON snapshot) — ข้อที่ใหม่จริงมีข้อเดียวคือ
กฎสองถัง. **2026-10-01: crystal ไม่มีตลาดแล้ว — column เปลี่ยนชื่อ `tradeable` → `paid`** (`d_paid`,
`granted_paid`). เหตุผลที่ยังต้องแยกถัง: refund ดึงคืนได้เฉพาะที่จ่ายเงินจริง, บัญชีรายได้นับเฉพาะ paid,
และ Monthly ให้ paid 300 แยกจาก bound รายวัน — ถ้า bound รั่วไป paid ได้ = refund/รายได้ผิด. ทดสอบด้วย fuzz แบบเดียวกับ I1
(`gacha::tests::fuzz_bucket_separation_and_conservation` — ตรวจ Σ ทั้งสองถังหลัง **ทุก** operation
ไม่ใช่แค่ปลายทาง).

**I12 — Summon Shards เป็นสกุลที่สาม แยกขาดจาก crystal (2026-07-31):** shard **ซื้อด้วยเงินไม่ได้**
มีทางเข้าทางเดียวคือ 1 ใบต่อ 1 pull และมีทางออกทางเดียวคือแลกที่ `MILEAGE_COST`. ห้ามแปลงไป-กลับ
กับ crystal ทั้งสองถัง และห้ามรวมยอดแสดงผลเป็นก้อนเดียวกับ crystal. เก็บใน `gacha_mileage` ของตัวเอง —
เจตนา**ไม่**เอาไปเป็นคอลัมน์ใน `crystal_wallet` เพราะจะทำให้ I11 (Σ สองถัง) ตรวจยากขึ้นโดยไม่จำเป็น.

เหตุผลที่ต้องเป็น invariant: ถ้า shard แปลงเป็น crystal ได้ = ก๊อกน้ำที่ไม่ต้องจ่ายเงินไหลเข้าถัง
paid = crystal ที่ไม่ได้จ่ายเงินกลายเป็น "จ่ายแล้ว" แบบเดียวกับที่ I11 กัน. ถ้า crystal แปลงเป็น shard ได้ = ซื้อ pity ตรง ๆ
ซึ่งทำลายเหตุผลที่ shard มีอยู่ (มันคือรางวัลของการ**หมุน** ไม่ใช่ของการ**จ่าย**).

⚠️ shard **ไม่ใช่ pity** — มันได้ทุก pull ไม่ว่าผลออกอะไร ⇒ ในเชิงเศรษฐกิจมันคือก๊อกน้ำคู่ขนาน
ที่**บวก**เข้ากับอัตราสุ่ม (1/50 + 1/200 = 1/40) ไม่ใช่เพดานที่จ่ายเฉพาะคนซวย. อ่านผิดข้อนี้
ทำให้ประเมินเพดานรายได้สูงเกินจริง 45%. ตรึงด้วย
`gacha::tests::mileage_pays_lucky_players_too_so_the_sources_add`.

**I13 — `InputAuthority::Server` ไม่ใช่ประตู เป็นแค่ป้าย (2026-08-10):** ไคลเอนต์เขียน input log
ของตัวเองและคำนวณ hash ด้วยการ replay log นั้น ⇒ **มันเขียน event ที่ติดป้าย Server ได้ทุกตัว**
แล้ว bundle ยังผ่าน verify. สิ่งที่กันจริงคือ**การตรวจรายตัว**ของ verify-server: nonce ที่คำนวณซ้ำ
(`Grant*`), reconcile กับเรคอร์ดจริง (`ClaimMail`), หรือ**ปฏิเสธทิ้ง**เมื่อไม่มีทั้งสอง
อย่าง (`SetCardRateBonus`).

⇒ **variant ใหม่ที่ authority เป็น Server = ไม่มีใครตรวจ จนกว่าจะมีคนเขียนตัวตรวจ.** ตรึงด้วย
`server_event_is_checked` (ค่าตั้งต้น = reject) + `every_server_authored_event_has_a_verdict`
ซึ่งจะแดงทันทีที่ `authority()` มี variant เพิ่ม. ข้อนี้เกิดจากรูจริงสองรูที่พบ 2026-08-10:
`SetCardRateBonus` ปั๊ม card rate ได้ ×10 และ `ClaimMail` เคลมเมลใบเดียวซ้ำได้ N ครั้งใน bundle เดียว.

**I14 — เซิร์ฟเวอร์ต้องมีเครื่องเดียว จนกว่า lock จะย้ายลง Postgres (2026-08-10):** ledger ตลาด
เป็นโครงสร้างในหน่วยความจำที่ล็อกด้วย `market_persist_lock` ซึ่งเป็น **process-local**. เครื่องที่สอง
= ล็อกคนละตัว + สำเนาคนละชุด ⇒ settlement สองรายการทับกันได้โดยไม่มีใครเห็นกัน. `fly.toml` เขียน
ข้อจำกัดนี้ไว้แล้ว — ถ้าจะ scale out ต้องย้าย state ลง Postgres (advisory lock หรือ
`SELECT ... FOR UPDATE`) **ก่อน** ไม่ใช่หลัง.

**หมายเหตุ I1 คือแม่:** ถ้า conservation ถือ + sink บันทึกครบ (I7) bug ปั๊มเงิน
ส่วนใหญ่จะทำ Σ ไม่ match แล้ว harness จับได้ทันที.

---

## 3. Test harness — ตัวตรวจแทนการรีวิว

- **Property/fuzz**: ยิง sequence สุ่มของ (mint / trade / list / cancel / burn)
  พัน–หมื่นครั้ง → ยืนยัน I1-I7 ไม่แตกทุกสถานะกลาง (ไม่ใช่แค่ปลาย). แบบเดียวกับ
  golden แต่สำหรับ ledger.
- **Deterministic replay**: ledger เป็น event-sourced (เหมือน sim) → mint/trade
  ทุกตัวเป็น event, replay ได้ → reconciliation = replay แล้วเทียบ.
- **Concurrency/atomicity**: ยิง trade คู่ขนานชนกัน (ขายของชิ้นเดียวสองคนพร้อมกัน)
  → ต้องมีคนเดียวชนะ, I3+I5 ถือ.
- **Idempotency**: ยิง mint event เดิมซ้ำ (จำลอง retry/replay) → I4 ถือ.
- **Load (ใน closed beta)**: harness วิ่งต่อเนื่องบน server จริงภายใต้ผู้เล่น
  concurrent — จับ slow-burn ที่ fuzz ในเครื่องไม่เจอ.

### `trade_ledger` — ตัวบันทึกที่หายไป (สร้าง 2026-08-01)

`crystal_ledger` บันทึกทุกการเคลื่อนไหวของเจมเป็นเดลต้ามีเครื่องหมาย + เหตุผล + อ้างอิง มาตั้งแต่ต้น
แต่**ตลาด Gil ไม่มีอะไรเทียบเท่าเลย** — `fees_burned` เป็นยอดรวมไม่มีรายบรรทัด, แถวเมลบอกแค่
"SaleProceeds 95 gil" โดยไม่รู้ว่าขายอะไรให้ใคร, และ `consumed_market_ops` เก็บแต่ id เปล่า ๆ

⇒ คำถามที่เอกสารนี้มีไว้ถาม ตอบไม่ได้สักข้อ: ไอเทมนี้เคยขายราคาเท่าไหร่ · สองบัญชีนี้เทรดกันเองรึเปล่า ·
Gil ก้อนนี้มาจากไหน

```sql
trade_ledger(id, at, kind, def_id, qty, seller, buyer, unit_price, payment, fee)
```

เขียนใน**ทรานแซกชันเดียวกับการ settle** — audit log ที่หายไปได้ไม่ใช่ audit log

เก็บเฉพาะ**การขายจริง** ยกเลิก/หมดอายุไม่มีคู่สัญญาและไม่มีการจ่ายเงิน เมลบันทึกไว้แล้วด้วย
`reason` ของตัวเอง การกันออกคือสิ่งที่ทำให้ `SUM(fee)` มีความหมายเทียบกับ `fees_burned` ได้ (I7)

**I7 ตรวจจาก SQL ได้แล้ว** — `SELECT sum(fee)::bigint FROM trade_ledger` เทียบกับ
**`fees_burned_commodity + fees_burned_item`** (`pg_trade_ledger_commits_with_the_settlement` ยืนยัน)
เดิมเป็น invariant ที่พิสูจน์ได้เฉพาะใน fuzz ในเครื่อง ตอนนี้พิสูจน์กับข้อมูลโปรดักชันจริงได้

⚠️ บรรทัดนี้เคยเขียนว่าเทียบกับ "`fees_burned` ของ ledger" **ซึ่งผิด** — `trade_ledger` ปนทั้ง
`kind='commodity'` และ `kind='item'` ส่วน `fees_burned` เป็นตัวนับแยกกัน ledger ละตัว เทียบกับตัวเดียว
คือ I7 ของตลาดที่ไม่เคยมีใครขายไอเทม เทสต์ก็เข้ารหัสสูตรผิดนี้ไว้เหมือนกัน และผ่านมาตลอดเพราะ
fixture มีแต่ commodity trade — แก้แล้ว 2026-08-17 (fixture มีการขายไอเทมด้วย สูตรเดิมจึงแดง)

**Gate:** flip real-money flag ได้เมื่อ — (a) I1-I10 เขียวใน fuzz, (b)
reconciliation clean ตลอด soak, (c) ไม่มี mismatch ค้างภายใต้ load จริง. **ตัวเลข
วัน = ขั้นต่ำ ไม่ใช่ trigger.**

---

## 4. Unknowns ที่ต้อง verify (ไม่ใช่โค้ด — เช็คก่อนล็อคแผน beta)

⚠️ ความรู้ผม cutoff ม.ค. 2026 — สองข้อล่างต้อง verify กับ **Steamworks docs/partner
จริง** ไม่ใช่เชื่อ doc นี้:

| # | Unknown | ทำไม gate ได้ | action |
|---|---|---|---|
| U1 | **Steam Inventory Service sandbox** ปัจจุบันทดสอบ grant/consume/trade ได้แค่ไหน | ถ้า sandbox จำกัด → harness ต้อง mock boundary + soak จริงตอน beta หนักขึ้น | อ่าน Steamworks Inventory Service docs ล่าสุด |
| U2 | **Marketability approval timeline** — Valve ต้องเปิด Community Market ให้ item เรา (review/relationship — TBH ผ่านมาแล้ว) | เป็น process ไม่ใช่โค้ด → **gate launch ได้** ถ้าสมัครช้า | ยิงถาม/สมัคร Steamworks แต่เนิ่น ๆ |
| U3 | Steam **wallet/payout/tax** เงื่อนไขต่อภูมิภาค (depa/entity — ดู idle-ccu-flywheel) | กระทบ cash-out flow + Merchant License | ผูกกับ entity decision |

---

## 5. Open (ยังไม่ตัดสิน)

- Ledger tech: event-sourced เดี่ยว transactional (แนะ) vs อื่น — ตัดสินตอนเริ่ม S3
- Merchant License staked + Turing test ที่ cash-out (ตาม checklist v2) — ผูก I9
- Grace window / offline-earnings attest (ตาม server-plan-phases §3) — ผูก I6
  (mint ต้องรอ event ที่ server attest แล้ว)

**Next:** เมื่อเริ่ม S3 → implement ledger + I1-I10 harness **ก่อน** เปิด market
ใด ๆ ใน closed beta. harness เขียวคือใบเบิกทาง ไม่ใช่ human review.
