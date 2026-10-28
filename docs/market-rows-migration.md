# ย้ายตลาดจาก blob เป็นแถว — ดีไซน์

ร่าง 2026-08-14 · ปิดงานที่ `docs/gacha-arch-review.md` เรียกว่า path (A)
ตัวเลขทั้งหมดในเอกสารนี้**วัดจากโค้ดจริง** ไม่ใช่ประมาณ (`market_write_cost_per_trade`, release build)

---

## 0. ปัญหา วัดแล้ว

`persist_market` แปลง `CommodityLedger` และ `ItemLedger` ทั้งก้อนเป็น JSON แล้วเขียนทับสองแถวใน
`market_kv (k TEXT, v TEXT)` ทุกครั้งที่มีการซื้อขาย ต้นทุนต่อ **หนึ่ง** คำสั่งจึงเท่ากับขนาด
ตลาดทั้งใบ คูณสอง:

| open orders | accounts | ไบต์ต่อ 1 คำสั่ง | serialize |
|---|---|---|---|
| 100 | 50 | 22 KB | 190 µs |
| 1,000 | 500 | 226 KB | 603 µs |
| 10,000 | 5,000 | 2.3 MB | 4.96 ms |
| 50,000 | 25,000 | 11.7 MB | 25.5 ms |

ที่ ~2,500 คำสั่ง (≈ 50,000 ผู้เล่น × 5% ลงขาย = สเกลจุดคุ้มทุนใน `docs/budget-plan.md`) การลงขาย
ของ **ชิ้นเดียว** เขียนราว 565 KB — สามเท่าครึ่งของไฟล์เซฟผู้เล่นทั้งไฟล์ (155 KB ที่วัดไว้ใน
หัวข้อ save-write throughput ของ `TODO.md`)

`market_persist_lock` บังคับให้ทุกธุรกรรมต่อคิวเดียว เพดานจาก serialize อย่างเดียวคือ ~1,660/วินาที
ที่ 1,000 คำสั่ง และเหลือ ~39/วินาที ที่ 50,000

หลังย้าย ต้นทุนต่อธุรกรรมในโปรดักชันคือ 1 แถว `sell_orders` (ลงขาย) หรือ 1–3 แถวที่ถูกจับคู่ (ซื้อ)
ประมาณ **50–200 ไบต์ คงที่ ไม่โตตามขนาดตลาด** — ที่สเกลเปิดตัวคือดีขึ้นราว **3,000–11,000 เท่า**

**ทำตอนนี้เพราะตลาดยังว่าง** การย้ายข้อมูลจึงแทบไม่มีต้นทุน ถ้ารอจนมีคำสั่งจริงหลักหมื่นกับเงินจริง
วิ่งอยู่ มันจะกลายเป็นงานที่ต้องวางแผนหยุดให้บริการ

---

## 1. ขอบเขต — สิ่งที่ **ไม่** แตะ

ตั้งใจจำกัดให้แคบ เพราะนี่คือเส้นทางเงิน:

- **`reconcile_market` ยังเป็น pure + synchronous** ไม่แปลงเป็น async ไม่ให้รู้จัก DB
  → เทสต์เดิมทั้งหมดใช้ได้ต่อโดยไม่แก้สักบรรทัด
- **`market_persist_lock` ยังอยู่** การถอดล็อกคือการตัดสินใจแยก และต้องวัดก่อนว่ามันเป็นคอขวดจริง
  (ตอนนี้ยังไม่มีใครวัด) เอกสาร gacha-arch-review เรียกทางนั้นว่า *"a rewrite of a pure,
  well-tested function that pulls the whole market layer with it"*
- **โครงสร้างในหน่วยความจำยังเป็น `BTreeMap`** ทุกอย่างที่อ่านตลาดยังอ่านจากหน่วยความจำเหมือนเดิม
- **การรับประกัน atomic กับ mailbox และกับเซฟผู้เล่น ไม่เปลี่ยน**

สิ่งที่ได้: ตัด write amplification ทิ้ง (ต้นทุนที่วัดได้จริงข้างบน)
สิ่งที่ไม่ได้: เพดานจาก mutex

> ✅ `gacha-arch-review.md` ระบุว่าสภาพที่แย่ที่สุดคือ *"rows and blob coexisting on a money path"*
> — หลังตัดการย้ายข้อมูลออก (§5) สภาพนั้น **เกิดขึ้นไม่ได้เชิงโครงสร้าง** ไม่ใช่แค่ห้ามไว้:
> ขั้นสุดท้ายสลับอ่าน/เขียน ลบคีย์ blob และลบ `market_blobs()` ในคอมมิตเดียว

---

## 2. Schema

```sql
CREATE TABLE market_accounts (
    account_id BIGINT PRIMARY KEY,
    gil        BIGINT NOT NULL
);
CREATE TABLE market_holdings (
    account_id BIGINT NOT NULL,
    def_id     TEXT   NOT NULL,
    qty        BIGINT NOT NULL,
    PRIMARY KEY (account_id, def_id)
);
CREATE TABLE sell_orders (
    id           BIGINT PRIMARY KEY,
    seller       BIGINT NOT NULL,
    def_id       TEXT   NOT NULL,
    qty          BIGINT NOT NULL,
    unit_price   BIGINT NOT NULL,
    listed_at_ms BIGINT NOT NULL
);
CREATE INDEX sell_orders_book ON sell_orders (def_id, unit_price, id);
CREATE TABLE item_listings (
    id           BIGINT PRIMARY KEY,
    seller       BIGINT NOT NULL,
    item_json    JSONB  NOT NULL,   -- JSONB ไม่ใช่ TEXT — ดูเหตุผลใต้ตาราง
    price        BIGINT NOT NULL,
    listed_at_ms BIGINT NOT NULL
);
-- ยังไม่ใส่ GIN index บน item_json — ดู §9 memo
CREATE TABLE market_meta (k TEXT PRIMARY KEY, v BIGINT NOT NULL);
-- keys: next_order_id · next_item_id · fees_burned_commodity · fees_burned_item
```

**`item_json` เก็บทั้งก้อน โดยตั้งใจ** `ItemListing.item` คือ `sim_core::items::Item` — มี substat,
socket, refine level, per-instance fields ที่แตกกางเป็นคอลัมน์ไม่ได้โดยไม่ผูกตารางเข้ากับ schema ของ
sim-core ซึ่งเปลี่ยนทุกครั้งที่ `sim_core::VERSION` ขยับ **แต่มันเป็น blob ต่อ*แถว* ไม่ใช่ต่อ*ตลาด***
ซึ่งคือทั้งหมดของประเด็นนี้

**ทำไม `JSONB` ไม่ใช่ `TEXT` — กฎที่ใช้ตัดสิน: "เลื่อนแล้วแพงไหม" ไม่ใช่ "ตอนนี้แพงไหม"**

วัดบน Postgres เครื่องนี้ 15,000 แถว ไอเทมรูปร่างจริง (substat 3 เส้น + socket 2 ช่อง):

| | ต่อแถว | เทียบ TEXT |
|---|---|---|
| `TEXT` | 1.1 µs | — |
| `JSONB` ไม่มี index | 4.1 µs | 3.7× |
| `JSONB` + GIN | 13.8 µs | 12.3× |

การเปลี่ยนคอลัมน์เป็น JSONB คิดเป็น **+3 µs ต่อแถว** — เทียบกับ round-trip ไป Neon (1–5 ms) คือ
0.06–0.3% มองไม่เห็น ส่วน GIN คือครึ่งที่แพง (+9.7 µs)

แต่ตัวเลขไม่ใช่เหตุผล **ต้นทุนการเลื่อนต่างหาก**:

| | เปลี่ยนทีหลังยังไง | ราคา |
|---|---|---|
| `TEXT` → `JSONB` | `ALTER TABLE … TYPE jsonb USING …` | **เขียนตารางใหม่ทั้งใบ + ACCESS EXCLUSIVE lock** ฟรีตอนตารางว่าง เป็นการวางแผนหยุดให้บริการตอนโต |
| เพิ่ม GIN | `CREATE INDEX CONCURRENTLY` | ไม่เขียนตารางใหม่ ไม่บล็อก ถูกเท่ากันทุกขนาด |

ชนิดคอลัมน์จึงทำตอนนี้ (ฟรี เพราะตารางว่าง) และ index รอได้ — **หลักเดียวกับที่ทำให้ migration
ทั้งงานนี้ควรทำวันนี้แทนที่จะรอ**

`sell_orders_book` เรียงตาม `(def_id, unit_price, id)` ตรงกับกฎการจับคู่ (`candidates.sort_unstable()`
บน tuple `(unit_price, id)`) — ถูกสุดก่อน เสมอกันตัดสินด้วย `id` = เวลามาก่อน

**ยังไม่มีใคร query มันวันนี้** การจับคู่เกิดใน `BTreeMap` ในหน่วยความจำ (`commodity_book` อ่าน
`st.commodity.held().book()` ไม่ใช่ SQL) เก็บไว้เพราะวัดแล้วราคา ~3 µs ต่อแถว (2× ของการเขียน
ที่ไม่มี index แต่ 0.1–0.9% ของเวลาทั้งธุรกรรม) และเผื่อวันที่ย้ายไป DB-only — ไม่ใช่เพราะมันจำเป็นตอนนี้

---

## 3. เส้นทางโปรดักชันไม่แตะ `Account` เลย — ตรวจแล้ว 2026-08-14

ข้อนี้เปลี่ยนดีไซน์มากที่สุด และเกือบพลาดไป

`Account { gil, holdings: BTreeMap<String,u64> }` **ไม่ถูกแตะโดยเส้นทางจริงสักทางเดียว**:

| การกระทำ | เส้นทางโปรดักชัน | แตะ `Account`? |
|---|---|---|
| ลงขาย | `add_order` — *"the units already LEFT the seller's GameState, so no holdings pool"* | ❌ |
| ซื้อ | `settle_buy` → `MailIntent` (value ข้ามบัญชีทางเมลเท่านั้น = settlement invariant) | ❌ |
| ยกเลิก | `cancel` → คืนหน่วยเป็นเมล | ❌ |
| หมดอายุ | `expire` → คืนหน่วยเป็นเมล | ❌ |

ผู้ที่แตะมีสามราย — `place_sell` · `market_buy` · `dev_seed` — ทั้งหมดคือ **isolated core (Slice 1)**
และ `/commodity/sell` อยู่หลัง `require_dev` แล้ว

คอมเมนต์ที่ `cancel` ระบุว่านี่เป็นผลจากการตรวจสอบครั้งก่อน ไม่ใช่ความบังเอิญ:

> crediting a server-side holdings pool **stranded the units** — they had no path back to the player

**ผลต่อดีไซน์:** คำถามเรื่อง granularity ของ holdings ที่เคยเป็นเรื่องใหญ่ **หายไปทั้งข้อ** ในโปรดักชัน
ไม่มี holdings ให้เขียน แถวคำสั่งขายเองคือ custody เพดาน 116 def_id (หรือ ~310 เมื่อ Card/Material โต)
ไม่เกี่ยวกับเส้นทางร้อนเลย

`market_accounts` / `market_holdings` ยังสร้างไว้ เพื่อให้ dev/test รอดการรีสตาร์ตและ invariant harness
ทำงานเหมือนเดิม — แต่ในโปรดักชันมันว่าง ต้นทุนเป็นศูนย์โดยธรรมชาติ

> ⚠️ **นี่คือสมมติฐานที่ต้องมีเทสต์ยัน** ถ้าวันหนึ่งมีคนต่อ `place_sell` เข้ากับเส้นทางจริง
> สมมติฐานพังเงียบและไบต์ต่อธุรกรรมกลับไปโตตามแคตตาล็อก — เคสเดียวกับที่ CLAUDE.md เตือนเรื่อง
> cap กับ load ที่อยู่คนละไฟล์ เทสต์: รันลำดับ ops แบบโปรดักชันแล้วยืนยันว่า `accounts` ยังว่าง

## 3b. รู้ได้ยังไงว่าแถวไหนต้องเขียน

ให้ ledger จำเอง ไม่ต้องแก้ที่จุดเรียกทั้ง 12 จุด

```rust
#[derive(Debug, Clone, Default)]
enum Dirty {
    #[default]
    All,                    // ← ค่าเริ่มต้น และค่าหลังโหลด
    Some(BTreeSet<u64>),    // id ที่ถูกแตะตั้งแต่ persist ครั้งล่าสุด
}
```

- `CommodityLedger` ถือ `dirty_orders` + `dirty_accounts` · `ItemLedger` ถือ `dirty_listings`
- ทุกเมธอด `&mut self` มาร์ก id ที่ตัวเองแตะ · `persist` เดินเฉพาะที่เปื้อนแล้วเคลียร์
- **"มีใน dirty แต่ไม่มีใน map" = ลบ** การยกเลิก/หมดอายุ/ซื้อจนหมด ไม่ต้องมีช่องทางแยก
- `dirty_accounts` มาร์ก**ราย account** ไม่ใช่ราย `(account, def_id)` — ตัดคลาสบั๊ก "ลบไม่ครบ" ทิ้ง
  และในโปรดักชันมันไม่เคยถูกใช้อยู่แล้ว (ดู 3 ข้างบน)

**`Dirty::All` คือค่าเริ่มต้น และนั่นคือกลไกความปลอดภัย ไม่ใช่ความขี้เกียจ**
ทุกทางที่ทำให้ dirty set เชื่อไม่ได้ — โหลดใหม่ รีสตาร์ต ย้อนสถานะ — ตกมาที่ `All` = เขียนทั้งหมด
หนึ่งรอบ **เสียเวลา ไม่เสียข้อมูล** ทิศทางที่ผิดพลาดได้มีทางเดียวและมันไม่อันตราย

> บทเรียนที่ยืมมา: `#[serde(skip)]` เคยทำ replay พังสองรอบใน sim-core (0.0.196, 0.0.261) เพราะฟิลด์
> ที่ "แค่ชั่วคราว" กลับมีผลต่อผลลัพธ์ ที่นี่ dirty set ชั่วคราวจริง และการตกไป `All` ทำให้การหายของมัน
> ไม่มีผล — **เงื่อนไขนั้นคือสิ่งที่ทำให้ skip ตรงนี้ปลอดภัย ไม่ใช่คำว่า "ชั่วคราว"**

## 4. ผู้เขียนสองทาง — ทั้งคู่ต้องย้ายพร้อมกัน

นี่คือจุดที่พลาดง่ายที่สุด `market_kv` ถูกเขียนจาก **สอง** ที่:

| | ที่ไหน | ใช้ตอน |
|---|---|---|
| 1 | `persist_market` → `market_blobs()` | ops ที่แตะตลาดอย่างเดียว (cancel, expire) |
| 2 | `persist_save_and_market` → `w.blobs` | **เส้นทางเงิน** — commit เซฟผู้เล่น + ตลาด ในทรานแซกชันเดียว |

ทางที่ 2 สำคัญกว่า คอมเมนต์ในโค้ดระบุสัญญาไว้ว่า *"Either everything is durable or nothing is"* —
ถ้าตลาด commit แต่การหักเงินของผู้จ่ายหาย (หรือกลับกัน) คือเงินหายหรือเงินงอก

ทั้งสองทางเปลี่ยนจาก "เขียน 2 blob" เป็น "เขียนแถวที่เปื้อน" **ในทรานแซกชันเดิม** ไม่เพิ่ม
ไม่แยกทรานแซกชัน การรับประกันจึงไม่เปลี่ยนเลย

---

## 5. ไม่มีการย้ายข้อมูล — ทิ้งแล้วเริ่มใหม่ (user ยืนยัน 2026-08-14)

ตลาดบนเซิร์ฟเวอร์จริงมีแต่ข้อมูลทดสอบ **ทิ้งได้ทั้งหมด** ดังนั้น:

```
ตอนบูต: DELETE FROM market_kv WHERE k IN ('ledger','item_ledger')
         แล้วโหลดจากตารางแถว (ว่างในรอบแรก)
```

ร่างแรกของเอกสารนี้ออกแบบทางย้าย (อ่าน blob → เขียนเป็นแถว → ลบคีย์) ไว้ครบ **และมันถูกตัดทิ้ง
เพราะไม่จำเป็น ไม่ใช่เพราะยาก** — โค้ดย้ายข้อมูลบนเส้นทางเงินคือโค้ดที่ต้องเขียนเทสต์ว่าย้ายครบ
ต้องรับมือกับ blob เก่าที่ deserialize ไม่ผ่านหลัง schema เปลี่ยน และต้องมีทางถอย **โค้ดที่ไม่ได้เขียน
คือโค้ดที่พังไม่ได้**

ผลพลอยได้ที่สำคัญกว่า: **ไม่มีช่วงเวลาใดเลยที่ rows กับ blob อยู่ด้วยกัน** ซึ่งเป็นสภาพที่
`gacha-arch-review.md` บอกให้เลี่ยง เดิมมันเลี่ยงไม่ได้ ต้องอาศัยวินัย "ห้ามปล่อยข้ามคืน" ตอนนี้
โครงสร้างของงานทำให้เกิดไม่ได้

> ⚠️ ข้อนี้ผูกกับข้อเท็จจริงที่ว่าตลาดจริงยังว่าง ถ้าวันหลังมีคนเล่นจริงลงขายก่อนงานนี้เสร็จ
> ต้องกลับมาเขียนทางย้าย — เช็ค `SELECT count(*) FROM market_kv WHERE k IN ('ledger','item_ledger')`
> และดูว่า blob ข้างในว่างจริงไหม ก่อนลบ

## 6. เมื่อเขียนพลาด

กลไกเดิมยังใช้ได้ ไม่ต้องออกแบบใหม่:

```
persist ล้มเหลว → load_market() ใหม่จาก DB
                → ถ้าโหลดไม่ได้อีก → market_poisoned = true → บล็อกการ settle
```

เพิ่มข้อเดียว: **`load_market` ต้องเซ็ต `Dirty::All` เสมอ** เพราะหลังย้อนสถานะ ไม่มีใครรู้ว่าอะไร
ถูกเขียนไปแล้วบ้าง

---

## 7. ลำดับ commit — ✅ เสร็จทั้งหมด 2026-08-16

| ขั้น | commit |
|---|---|
| 1. schema + ตัววัด | `0f83108` |
| 2. dirty tracking ใน ledger | `3ce3071` |
| 3. สลับมาที่แถว + ลบ blob | `7f29187` |

### แล้วมีอีก 5 commit ที่ตามมาจากการตรวจสอบ — ซึ่งคือเนื้อหาที่มีค่าที่สุดของงานนี้

| | |
|---|---|
| `1b7a532` `44c65f2` | **race ที่ทำ row หายและ row ซ้ำ** — สองรอบ เพราะรอบแรกแก้ครึ่งเดียว |
| `28258d2` | **mutate + persist เป็นหน่วยเดียวใต้ lock เดียว** (`MarketGuard`) |
| `c655fcb` | fail-open สามจุดที่รายงานสำเร็จโดยไม่ได้ทำอะไร |
| `bfbafd3` | **เส้นทาง load fail closed** |
| `a3ab4c5` | `load_market` ต้องการ lock เพราะมัน **เขียนทับ** ไม่ใช่อ่าน |
| `71553e1` | สี่ข้อจาก audit รวม A+B |

**บทเรียนที่ควรอยู่ในเอกสารมากกว่าลำดับขั้น:** การย้ายเองใช้ 3 commit ส่วนอีก 6 commit คือการตาม
เก็บสิ่งที่การย้ายทำเกิดขึ้น — และ **ทุกอันถูกพบโดย audit หลังจากผู้เขียน self-review แล้วสรุปว่าสะอาด**
สามรอบติดต่อกัน ไม่มีรอบไหนที่ self-review จับได้เอง

ถ้ามีใครทำงานลักษณะนี้อีก: การ audit โดยคนอื่นไม่ใช่ขั้นตอนเสริมท้ายงาน มันคือส่วนหนึ่งของงาน

## 8. พิสูจน์ยังไง — ✅ สร้างแล้วทั้งหมด

| | สถานะ |
|---|---|
| **Conservation ข้าม persist→reload** | ✅ `pg_conservation_survives_persist_and_reload` — ops แบบโปรดักชัน persist ทุก op reload ทุก 5 · sabotage แดง 4 แบบทั้งทิศเสกและทิศทำลาย |
| Round-trip | ✅ `pg_market_rows_round_trip_through_postgres` |
| ไบต์ต่อธุรกรรมคงที่ | ✅ `market_write_cost_per_trade_is_constant` — 2–3 แถว เท่ากันที่ 100 และ 50,000 คำสั่ง |
| mark ครบทุก mutation | ✅ `every_mutation_marks_the_row_it_touched` |
| ลบต้องทำงาน | ✅ ครอบใน round-trip (เฟส incremental) |
| load ปฏิเสธตลาดที่เชื่อไม่ได้ | ✅ `pg_load_market_fails_closed_on_a_market_it_cannot_trust` |
| mutate อยู่ใต้ lock | ✅ compiler (`MarketGuard`) + `every_ledger_mutation_is_under_the_market_lock` |

**`fees_burned` ตรวจทานด้วย `SUM(fee)` — สูตรในเอกสารเดิมผิด** ต้องเป็น
`SUM(fee) FROM trade_ledger == fees_burned_commodity + fees_burned_item` ไม่ใช่เทียบกับตัวใดตัวหนึ่ง
เพราะ `trade_ledger` ปนทั้ง `kind='commodity'` และ `kind='item'` ส่วน `fees_burned` เป็นสองตัวนับ
เทสต์ที่มีอยู่ผ่านได้เพราะ fixture ไม่มี item fill — **ยังไม่มีใครแก้**

## 9. ตัดสินแล้ว (user, 2026-08-14)

1. **`fees_burned` → `market_meta`** — เป็นตัวแปรเดียว เก็บที่เดียวง่ายสุด **บวก** เทสต์ยืนยัน
   `SELECT SUM(fee) FROM trade_ledger` == `fees_burned` ตัวนับที่แก้ได้ drift เงียบได้ log ที่ต่อท้าย
   อย่างเดียว drift ไม่ได้ — สองแหล่งจึงเป็น guard ไม่ใช่ความซ้ำซ้อน ตรงกับ I7 (*"กัน I1 พังแบบซ่อน"*)
2. **`next_order_id` เก็บ ไม่คำนวณ** — `MAX(id)+1` มีรูตรงที่ถ้าคำสั่งถูกลบหมด ตัวนับวนกลับไปออก id
   ที่ `trade_ledger` เคยอ้าง การตรวจสอบย้อนหลังจะชี้ผิดตัว **ตอนโหลดต้องยืนยัน
   `next_order_id > MAX(sell_orders.id)` ไม่งั้นปฏิเสธที่จะบูต** — id ซ้ำบนเส้นทางเงินคือหายนะ
   ที่เงียบกว่าการบูตไม่ขึ้นมาก
3. **`accounts`/`holdings` เก็บ** — dev/test-only แล้ว (§3) แต่ราคาศูนย์ในโปรดักชัน และทำให้
   invariant harness เหมือนกันทั้งสองโหมด
4. **`item_json` เป็น `JSONB`** — ดู §2 (เลื่อนแล้วแพง)
5. **`sell_orders_book` ใส่ไว้เลย** — 3 µs/แถว เผื่อระบบค้นฝั่งผู้เล่นที่เร็วขึ้นในอนาคต

---

## 📌 MEMO — พิจารณาเพิ่ม GIN index บน `item_listings.item_json`

**ยังไม่ทำวันนี้ โดยตั้งใจ** เขียนไว้เพราะเป็นของที่จะถูกลืม ไม่ใช่ของที่ตัดทิ้ง

**มันจะให้อะไร** — ค้น equipment ด้วย **substat** ด้วย SQL ได้ ซึ่งบันทึกไว้ว่าเป็น *centerpiece*
ของตลาดไอเทม (`docs/item-trade-design.md`) ตอนนี้การค้นเกิดใน `BTreeMap` ในหน่วยความจำ ซึ่งใช้ได้
ตราบใดที่ ledger ในหน่วยความจำยังเป็นตัวจริง

**ทำเมื่อไหร่ — เงื่อนไขข้อใดข้อหนึ่ง:**
- ระบบค้น substat ฝั่งผู้เล่นจะลงจริง และต้องการให้ฐานข้อมูลค้นแทนหน่วยความจำ
- ย้ายไป DB-only (ถอด `BTreeMap` ออก) — ตอนนั้นจำเป็นทันที ไม่ใช่ทางเลือก
- จำนวน listing โตจนการไล่ค้นในหน่วยความจำเริ่มกินเวลาเห็นได้

**ราคาที่วัดไว้แล้ว:** +9.7 µs ต่อแถวที่เขียน (13.8 vs 4.1 µs) และกินพื้นที่ราวเท่าตัวตาราง

**คำสั่ง — ไม่ต้อง migrate ข้อมูล ไม่บล็อกการเขียน:**

```sql
CREATE INDEX CONCURRENTLY item_listings_json ON item_listings USING GIN (item_json);
```

ต้องเป็น `JSONB` ถึงจะทำได้ — ซึ่งเป็นเหตุผลที่ §2 เลือก JSONB ตั้งแต่วันนี้ทั้งที่ยังไม่มีใครใช้
`CONCURRENTLY` ทำให้เพิ่มตอนระบบมีคนเล่นอยู่ได้ จึงเลื่อนได้จริง ไม่ใช่เลื่อนแล้วติดหนี้

---

## 10. ที่ยังค้าง (อัปเดต 2026-08-16 หลัง audit สามรอบ)

**สถานะ:** รายการที่เคยอยู่ตรงนี้ปิดหมดแล้ว ดู §13 (แผน E/F/G) และ §15 (audit รอบ 6) ของที่ยัง
เหลือจริงอยู่ใน §15 ท้ายไฟล์ พร้อมเหตุผลว่าทำไมเลื่อน — **อย่าอ่านลิสต์นั้นเป็นหนี้ค้าง** สองในนั้น
เข้าไม่ถึงและหายเองตามลำดับ

### กอง C — ปิดแล้ว (แก้รอบสอง 2026-08-16 หลัง audit; รอบแรกยังไม่ปิดจริง)

อาการเดิม: `restore_plan` คืน dirty mark แต่ `MailPlan` เป็น borrow ของ local ใน handler และตายไป
พร้อม response → request ถัดไปหยิบ mark กำพร้าไป `DELETE FROM sell_orders WHERE id = 7` โดยไม่มี
Return mail = หน่วยหายถาวร (ออกจาก GameState ผู้ขายตอนลงขาย และ mail คือทางกลับทางเดียว)

ไม่ได้ปิดด้วยการจัดการ mail แต่ปิดด้วยการทำให้ **mark ที่คืนมาไม่มีทางถูกสังเกตเห็น** ทุกทางออก:

1. `persist_market_or_revert` — reload สำเร็จ → **แทน ledger ทั้งก้อน** mark หายไปด้วย
2. `persist_market_or_revert` — reload ยืนยันไม่ได้ → `market_poisoned` → ผู้เขียนถูกกั้น
   (ข้อนี้เพิ่งจริงหลัง `71553e1`/`ef1b667` ก่อนหน้านั้น `commodity_sell`/`cancel`/`item_market_cancel`
   ไม่เช็ค poison เลย — คือทางที่ audit เดินเจอ)

   **⚠️ ข้อนี้เขียนไว้ครั้งแรกว่า "ผู้เขียนทั้งสี่ถูกกั้น" — ผิด มีห้า** `put_save` เป็นคนที่ห้า
   ไม่เคยเรียก `market_unusable` เลย และเป็นตัวเดียวที่ **ดูดออเดอร์ทิ้งได้** (`settle_buy`)
   มันอ่าน flag ที่ `:4659` **นอก lock** แล้ว `await` lock ที่ `:4709` — ช่วงรอกว้างเท่า persist ที่
   ล้มบวก `load_market` หกคิวรี่ที่ตั้ง poison พอดี audit reproduce ได้: herb 500 ชิ้นหาย แถวหาย
   ไม่มี Return mail และ `load_market` ปฏิเสธได้จาก **invariant refusal** ที่ `bfbafd3` ตั้งใจใส่
   (meta ไม่ครบ / holdings กำพร้า / listing deserialize ไม่ผ่าน) ไม่ต้องรอ DB ล่ม และปฏิเสธซ้ำได้
   เพราะ deterministic

   **แก้แล้ว** — `usable_market_lock(lock, poisoned)` คืน guard เฉพาะเมื่อ**ถือ lock อยู่แล้ว**และยัง
   ไม่ poison เป็นฟังก์ชันอิสระเหนือสองฟิลด์ที่ต้องใช้ เพื่อให้ **interleaving ทดสอบได้** และตัวมันเอง
   คือทางแก้: caller ที่ผ่านทางนี้ไม่มีที่ให้วางการอ่านก่อน `await` อีก · `put_save` ให้ **guard เป็น
   ตัวถือคำตอบ** แทน bool คู่ขนาน (`market_recon` gate ที่ guard ไม่ใช่ `was_tainted || poisoned`)
   fusion แบบเดียวกับ `MarketSave` ขึ้นไปอีกชั้น
3. `persist_save_and_market` (save path) — เดิม**ยังรั่ว** ดูด้านล่าง

**รูที่ 3 เจอตอนตรวจสอบว่ากอง C ปิดจริงไหม** ไม่ได้อยู่ในรายงาน audit เดิม: ฝั่ง save
`restore_plan` ทำงานเมื่อ `write_market` จริง แต่ผู้เรียก revert เมื่อ `market_changed &&
mkt_guard.is_some()` สองค่านี้ผูกกันด้วย `debug_assert!` เท่านั้น ซึ่ง **ถูกคอมไพล์ทิ้งใน release**
คือบิลด์เดียวที่เรื่องนี้สำคัญ ถ้าหลุด: mark ถูกคืน ไม่ reload ไม่ poison = อาการเดิมเป๊ะ

แก้โดยหลอมสองค่าเป็นค่าเดียว `MarketSave::{Write(&MarketGuard), Skip}` — "save นี้เขียน market"
กับ "หลักฐานว่าถือ lock" เป็นเรื่องเดียวกัน สถานะ "เขียนโดยไม่ถือ lock" จึง **เขียนออกมาไม่ได้**
และ write กับ revert อ้างอิงค่าเดียวกัน ไม่มีทางครอบคลุมคนละชุด ([[make-the-compiler-the-gate]])
มุม `(changed, ไม่มี guard)` ที่เข้าไม่ถึงอยู่แล้ว → **reject** (ทางเดียวที่ไม่เสียอะไร: op ยังไม่ถูก
consume แบบ durable ผู้เล่น retry แล้ว replay idempotent) ไม่ใช่ข้ามการเขียนเงียบ ๆ

เทสต์: `pg_a_failed_persist_leaves_the_ledger_agreeing_or_poisoned` ขับ `persist_market_or_revert`
ตัวจริง ทั้งสองทางออก (ซ่อนตาราง `mailbox` = persist ล้มแต่ reload ยืนยันได้ / ปิด pool = ล้มทั้งคู่)
ก่อวินาศกรรมแล้วแดงทั้งสองอัน

หมายเหตุที่ควรรู้: ตัด `orders_del` ออกจาก `restore_plan` แล้ว **เทสต์ทั้ง 144 ตัวยังเขียว** — ไม่ใช่
ช่องโหว่ แต่เป็นผลที่ตามมาโดยตรงจากข้างบน: mark ที่คืนมาสังเกตไม่ได้ตามนิยาม `restore_plan` จึงเป็น
defense-in-depth ล้วน ๆ ตอนนี้ ถ้าวันหน้ามี path ที่คืน mark แล้วไม่ reload/poison มันจะกลับมาสำคัญ
ทันที — และเทสต์ด้านบนคือสิ่งที่จะจับ path นั้น

### แยกออกมาเพื่อให้เทสต์ได้: `MarketParts`

`persist_market_or_revert` เดิมรับ `&AppState` (30+ ฟิลด์ construction site เดียวใน `main`) จึง
ทดสอบกับฐานข้อมูลจริงไม่ได้เลย เทสต์ที่จำลองลำดับขั้นเองพิสูจน์ได้แค่ตัวเทสต์ ไม่ใช่โค้ดที่ deploy
`MarketParts` คือ borrow struct ของสี่ชิ้นที่ path นี้แตะจริง (`db`/`commodity`/`item`/`poisoned`)
โปรดักชันส่ง `st.market_parts()` — โค้ดเส้นทางเดียวกันทั้งสองฝั่ง

**ตัดสินแล้วว่าไม่ทำตอนนี้:**

- ถอด `market_persist_lock` — หลังย้ายแล้ว critical section หดลงมาก เพดานขึ้นเองโดยไม่ต้องถอด
- GIN index บน `item_listings.item_json` — ดู MEMO ข้างบน
- index บน `market_holdings.def_id` — ยังไม่มี query ไหนใช้

---

## 11. แผนกอง E / F / G (วางไว้ 2026-08-16, ยังไม่ลงมือ)

อ่านโค้ดจริงทั้งหมดก่อนวาง — สองข้อด้านล่างต่างจากที่ §10 บันทึกไว้ตอนแรก

### สิ่งที่พบตอนอ่าน

**G2 ร้ายแรงกว่าที่บันทึก — เป็นข้อที่แย่ที่สุดในลิสต์**

`row_plan()` ตั้ง `p.full = true` ถ้า **ตัวใดตัวหนึ่ง**ใน `dirty_orders`/`dirty_accounts` เป็น `All`
แล้ว `write_market_rows` เห็น `cm.full` ก็ลบ **ทั้งสามตาราง** แต่ใส่กลับเฉพาะที่อยู่ในแผน

```
dirty_orders = All , dirty_accounts = Ids({7})
  → full = true
  → DELETE FROM market_accounts     ← gil + holdings ของผู้เล่นทุกคนหาย
  → accounts_up = [7] เท่านั้น       ← กลับมาคนเดียว
```

เข้าไม่ถึงวันนี้เพราะทั้งสามทางเปลี่ยนสถานะ (`Default`, `restore_plan`, `take_row_plan`) ตั้งสอง
ฟิลด์พร้อมกันเสมอ — **โดยบังเอิญ ไม่ใช่โดยออกแบบ** เพิ่ม transition ที่แตะฟิลด์เดียวเมื่อไหร่ก็ล้างตลาด

**E2 เป็น dev จริง** — `/commodity/sell` อยู่ใน `dev_routes` หลัง `require_dev` ต้องมี DEV_KEY หลุดก่อน

**save path ถูกจำกัดราคาโดยอ้อม** — `MarketOp::Deposit` ต้องผ่าน `deposit_commodity` ใน sim-core
และ op ต้องรอด replay-verify แต่ **server ไม่บังคับเอง** ยืมขอบเขตจาก sim มาทั้งดุ้น

### ข้อ 0 — เช็ค delete↔mail ตอนเขียน (ตัวใหม่ ไม่ได้อยู่ใน §10)

เหตุผลที่ต้องมี: บั๊กทุกตัวในรอบนี้วิ่งเข้าหา failure mode เดียว — **แถวที่ถือของหายโดยไม่มี mail**
และไม่มีอะไรในโปรดักชันเฝ้าอยู่เลย (I1 conservation เป็น fuzz ในหน่วยความจำเท่านั้น ที่
`commodity.rs::fuzz_conservation_and_atomicity` · `trade_ledger` จับได้แค่ I7 · `/health` เป็น
liveness · `/dev/balance` เป็น telemetry บาลานซ์เกม) บั๊กจึงต้องหาด้วยการ*อ่าน* สี่รอบ

**Σgil ตัดทิ้ง — เล็งผิดตัวเลข** `settle_buy` (production path ผ่าน `reconcile_market`) ไม่แตะ
`Account::gil` เลย: เงินผู้ซื้อถูกหักใน GameState ตอน verify และเงินผู้ขายกลับเป็น mail คนเขียน
ledger gil มีแค่ `dev_seed` กับ `market_buy` ซึ่งเป็นทาง Slice-1 stand-in ที่โปรดักชันข้ามไป
`SUM(gil) FROM market_accounts` จึงบวกตารางที่แทบว่าง — **จับ G2/กอง C/E1 ไม่ได้สักตัว** = decoration

ของที่มีวงปิดจริงคือ **ของในอารักขา**: units/items ออกจาก GameState ตอนลงขาย → อยู่เป็นแถว
`sell_orders.qty` / `item_listings` → กลับด้วย mail เท่านั้น

**จุดที่ทำ**: `persist_market_locked` — `MarketRowPlan` กับ `MailPlan` อยู่ในทรานแซกชันเดียวกัน
ตรงนั้นพอดี ทำใน `commodity.rs` ไม่ได้ เพราะ `cancel`/`cancel_listing` **ไม่สร้าง mail เอง** มัน
return `(def_id, qty)` / `Item` ให้ handler สร้างต่อ (คือช่องว่างที่กอง C พูดถึง)

**เช็คสองข้อ** (~15 บรรทัด ทำงานใน release ไม่ใช่ `debug_assert!` ล้ม → reject → client retry):

1. `accounts_del.is_empty()` — **ไม่มีโค้ดไหนลบบัญชีเลย** (`accounts.remove` ไม่มีในไฟล์) แถวใน
   `accounts_del` จึงแปลว่ามี mark ของบัญชีที่ไม่มีตัวตน = ผิดเสมอ
2. `orders_del.len() + listings_del.len() <= mail.send.len()` — เงื่อนไข**จำเป็น** ไม่ใช่เพียงพอ
   แต่จับได้ทั้งสามบั๊กที่เจอจริง ไล่ครบ 7 จุดที่ลบแถวแล้ว ทุกจุดมี mail คู่กัน:

   | จุด | ลบอะไร | mail |
   |---|---|---|
   | `cancel` :564 | order | Return (handler สร้าง) |
   | `settle_buy` :630 | order ที่เต็ม | SaleProceeds + Purchase |
   | `expire` :683 | order หมดอายุ | Return |
   | `market_buy` :727 | order (Slice-1 stand-in) | SaleProceeds |
   | `cancel_listing` :1006/:1053 | listing | Return (handler สร้าง) |
   | `settle_item_buy` :1024 | listing | Purchase + SaleProceeds |

   fill บางส่วนไม่ลบ (ไป `orders_up`) จึงไม่นับ · ทางที่ mail มากกว่า delete มีปกติ (buy หลาย order
   = 3 delete + 4 mail) ทางที่ delete มากกว่า mail ไม่มี

**ทางที่แข็งกว่าถ้าอยากได้ทีหลัง**: ใส่ `source: Option<u64>` ลง `MailIntent` แล้วจับคู่ id↔mail
แบบตรงตัว จะปิดช่องว่าง "เพียงพอ" ด้วย และแก้ข้อที่ `economy-invariants.md` บ่นไว้ (บรรทัด 135:
mail row ไม่มีรายบรรทัดให้ตรวจ) — แต่แพงกว่าและยังไม่จำเป็นตอนนี้

**สิ่งที่เช็คนี้ไม่จับ**: ledger กับ DB ที่ต่างกันด้วยเหตุอื่น มันตรวจว่า*แผนถูก* ไม่ได้ตรวจว่า
*ฐานข้อมูลตรง* — ตรงตามที่ตั้งใจ

### ลำดับ

| # | ข้อ | ทำอะไร | ขนาด |
|---|---|---|---|
| 0 | เช็ค delete↔mail | ข้างบน | เล็ก |
| 1 | **G2** | ยุบ `dirty_orders`/`dirty_accounts` เป็นโครงเดียว: `all: bool` + id set สองชุด — `full` เป็นคุณสมบัติของ *ledger* อยู่แล้ว (ธงเดียวคุม 3 ตาราง) ให้ต้นทางเป็น ledger-level ด้วย สถานะ "ไม่ตรงกัน" จึงเขียนออกมาไม่ได้ | กลาง |
| 2 | **E1** | `settle_item_buy` ย้าย `dirty_listings.mark(listing_id)` เข้าใน `if let Some(l) = taken` — id มาจาก client ("redundant" จึงเป็นสิ่งที่ผู้โจมตีคุม) | เล็ก |
| 3 | **F2** | `every_ledger_mutation_is_under_the_market_lock` เป็น decoration: ตัดฟังก์ชันที่ `fn` คอลัมน์ 0 ทำให้ `mod pg_integration` ทั้งก้อนเป็น body เดียว · `body.contains("MarketGuard")` พอใจได้จากคอมเมนต์ · sentinel `checked >= 3` พอใจตัวเองจากเทสต์ **แก้: ให้ mutator รับ `&MarketGuard`** แล้วลบเทสต์สแกนทิ้ง | ใหญ่ |
| 4 | **E2** | `place_sell` เช็ค `MAX_LISTING_PRICE` เอง ไม่ยืมจาก sim ([[input-authority-security]]) | เล็ก |
| 5 | **F1** | สูตร I7 ต้องเป็น `fees_burned_commodity + fees_burned_item` | เล็ก, doc |
| 6 | **G1** | `has_expired` → `values().next()` (BTreeMap, id เรียงตามเวลาสร้าง) | เล็ก |

**เหตุผลของลำดับ**: ข้อ 0 ขึ้นก่อนเพราะทำให้ข้อที่เหลือ*ตรวจสอบได้*แทนที่จะต้องเชื่อ · G2 ต่อ
เพราะผลลัพธ์คือล้าง gil ทุกบัญชี และแก้ถูกกว่าตอนนี้มากกว่าตอนมีคนแตะโครง dirty แล้ว — เข้าไม่ถึง
วันนี้ไม่ใช่เหตุผลให้ปล่อย เพราะสิ่งที่กันอยู่คือความบังเอิญ · F2 อยู่กลางเพราะมันคือ guard ที่ควรจับ
E/G ตั้งแต่แรก ถ้าแก้แบบ compiler gate จะกลืน E-class ไปด้วย แต่เป็นงานใหญ่สุด ถ้าอยากได้เร็ว
สลับ 4 ขึ้นก่อนได้

### ทำไมรอบนี้เจอเยอะ (บันทึกไว้เพื่อไม่ให้ประเมินผิดซ้ำ)

บั๊กเกือบทุกตัวรูปเดียวกัน: **สองอย่างที่ต้องตรงกัน ถูกค้ำด้วยธรรมเนียม ไม่ใช่โครงสร้าง** —
`dirty_orders`↔`dirty_accounts` (ธรรมเนียม) · `write_market`↔`mkt_guard` (`debug_assert!` ที่
หายไปใน release) · mark↔mail ("อยู่ใน handler เดียวกัน") · poison↔lock ("ทุกคนจำได้ว่าต้องเช็ค")
· `full`↔3 ตาราง (คอมเมนต์) อันที่ทำถูกมีอันเดียว: `MAX_LISTING_PRICE`↔ชนิดคอลัมน์ ค้ำด้วยเทสต์ที่
assert *ความสัมพันธ์* ไม่ใช่ตัวเลข (`mailbox_grant.rs:406`) — และเป็นอันเดียวที่ไม่เคยพัง

blob **ถูกต้องด้วยกำลังดิบ**: "เขียนทั้งโลกทุกครั้ง" มีบั๊ก partial-write / stale-mark /
full-vs-incremental ไม่ได้ เพราะไม่มี state ให้ไม่ตรง incremental ซื้อ O(1) มาด้วยการจ่ายเป็น
**state ที่ไม่ตรงกันได้** ตอนวางแผนเราชั่ง "write amplification เทียบกับแรงที่ต้อง migrate" แต่ไม่ได้
ชั่ง **"สร้าง invariant ใหม่กี่ข้อ"** — §7 (migrate 3 commit ตามแก้ 6) คือการประเมินผิดข้อนี้เป็นตัวเลข
ยังเป็นการตัดสินใจที่ถูก (blob มี HIGH-1 ของตัวเอง และ 11.7 MB/trade คือกำแพง) แต่ครั้งหน้าต้อง
ชั่งข้อนี้ด้วย

แยกให้ชัด: **migration สร้างจริง** = ทุกอย่างที่เกี่ยวกับ dirty-tracking (G2, E1, `clear_written`,
กอง C) · **มีอยู่ก่อนแล้ว** = `place_sell` ไม่เคยเช็คราคา, เทสต์ lock เป็น decoration, `AppState`
ทดสอบไม่ได้, poison ไม่มี metric — migration ไม่ได้สร้าง แต่ทำให้เห็น เพราะบังคับให้มีคนอ่าน
money path ละเอียดเป็นครั้งแรก

---

## 12. Audit รอบ 5 (2026-08-16/17) — 4 confirmed, ปิดครบ

รอบนี้ต่างจากสี่รอบก่อน: หนึ่งข้อเป็นบั๊กเงินหายจริงที่ reproduce ได้ อีกสามข้อเป็น**เทสต์ที่ไม่ได้
คุมอะไร** ซึ่ง sabotage แล้ว 144 เขียวทุกตัว

### F1 — `put_save` คือผู้เขียนคนที่ห้า อ่าน poison นอก lock (`e0e7b05`)

`ef1b667` ปิด sample-แล้ว-lock ให้ HTTP writer สี่ตัว แล้วเหลือรูปเดียวกันไว้ที่ `put_save` ซึ่งเป็น
endpoint ที่ busiest ที่สุด และเป็นตัวเดียวที่ **ดูดออเดอร์ทิ้ง** (`settle_buy`) มันอ่าน flag ที่ `:4659`
แล้ว `await` lock ที่ `:4709` — ช่วงรอกว้างเท่า persist ที่ล้มบวก `load_market` หกคิวรี่ที่ตั้ง poison

reproduce ได้: herb 500 ชิ้นหาย แถวหาย ไม่มี Return mail · และ `load_market` ปฏิเสธได้จาก
**invariant refusal** ที่ `bfbafd3` ตั้งใจใส่ ไม่ต้องรอ DB ล่ม และปฏิเสธซ้ำได้เพราะ deterministic

§10 เขียนไว้ว่า "ผู้เขียนทั้งสี่ถูกกั้น" — **ผิด มีห้า** แก้ที่ §10 แล้ว

### F2 — `pg_a_poisoned_market_writes_nothing` ตายทางสถิต (`bb3c431`)

`allowed` มาจาก literal `true` สองบรรทัดบน, `assert!(!allowed)` พอใจตัวเอง, `if allowed {...}`
เข้าไม่ได้ ไม่เรียก `market_unusable` เลย · ปิด `market_unusable` ให้คืน `false` ตลอด = 144 เขียว
คือลบงาน `71553e1`+`ef1b667`+`e0e7b05` ได้โดยไม่มีอะไรแดง · ย้ายเช็คไปใต้ mutation ก็เขียว และ
`ef1b667` ได้**ลบ** `assert!(check < mutate)` ที่เคยจับได้ทิ้งไปแล้ว = coverage จากอ่อนเป็นศูนย์

**ไม่ได้แก้ด้วยเทสต์ที่ดีกว่า แต่ด้วยการทำให้พื้นที่ผิดเล็กลง** — `market_unusable` ถูกลบ ผู้เขียน
ทั้งห้าขอ guard ผ่าน `usable_market_lock` ทางเดียว "ลืมเช็ค" / "เช็คหลัง mutate" / "เช็คก่อน lock"
จึงเขียนออกมาไม่ได้ · `market_lock` → `market_lock_ignoring_poison` มีผู้เรียกเดียวคือ
`recover_poisoned_market` (แยกออกจาก `put_save` เพื่อให้กฎ "writer ห้ามถือ raw lock" ไม่มีข้อยกเว้น
ให้จำ) · enumeration test มีครึ่งที่**ไม่ต้องมีลิสต์**: นับ raw acquisition ในฝั่งโปรดักชัน แดงทันที
ที่มีตัวใหม่ ไม่ว่าจะอยู่ตรงไหน

พบระหว่างทาง: `every_ledger_mutation_is_under_the_market_lock` รับสตริง `MarketGuard` ที่ไหนก็ได้
ในตัวฟังก์ชัน คอมเมนต์ `// see MarketGuard` จึงพอใจมันได้ — นั่นคือเหตุผลที่ `commodity_cancel`
รอดตอน rename ส่วน `commodity_sell` ไม่รอด แก้ให้ดูการเรียกจริงหรือ signature แล้ว

### F3 — `market_parts()` ไม่มี coverage (`bb3c431`)

ตั้ง `db: None` = 144 เขียว ในโปรดักชันกู้ไม่ได้: persist อ่าน `None` เป็น "in-memory ไม่ต้องเขียน"
แล้ว**รายงานสำเร็จ** → `/commodity/sell` ตอบ `ok:true` ให้ออเดอร์ที่อยู่แค่ในหน่วยความจำ
`/commodity/cancel` ตอบ "cancelled" โดยไม่เคยเขียน Return mail · `db` เป็นฟิลด์เดียวที่ fixture
มือทำมองไม่เห็น (อีกสามตัวคนละ type คอมไพล์ไม่ผ่านถ้าสลับ) → `test_state()` สร้าง `AppState` จริง

### F4 — revert ของ `put_save` ไม่มี coverage (`bb3c431`)

ลบทั้งบล็อก = 144 เขียว ทั้งที่โค้ดเส้นเดียวกันฝั่ง handler มีเทสต์คุม แก้โดย**ไม่เทสต์สองรอบ**:
มี `revert_market_or_poison` ตัวเดียว และ **restore mark กับ revert เป็นการกระทำเดียวกัน** จึงอยู่ใน
ฟังก์ชันที่เป็นคนเอา mark ออกมา · `persist_market_locked` ถูกยุบเข้า `persist_market_or_revert`
ด้วยเหตุผลเดียวกัน (มันคืน mark ที่ 4 branch แล้วปล่อย revert ให้ caller = รอยแยกที่ audit เดินผ่าน)

`every_function_that_takes_the_marks_restores_and_reverts` **แจกแจงจาก source** ไม่ใช่ลิสต์ที่พิมพ์
— คือความล้มเหลวที่ซ่อนผู้เขียนคนที่ห้าไว้ ลบครึ่งไหนของ path ไหนก็แดง และ mark-taker ตัวที่สาม
ที่เพิ่มตรงไหนก็แดง

### บทเรียนของรอบนี้

**sabotage ที่ไม่ sabotage อันตรายกว่าเทสต์ที่ตาย** ระหว่างแก้ F1 ผม*แทรก* early-return เข้าไปโดยยัง
คงเช็คที่ถูกไว้ข้างล่าง ฟังก์ชันจึงยังทำงานถูก แล้วผมเกือบสรุปว่าเทสต์ที่ดีอยู่แล้วเป็น decoration และ
เขียนทับ · เห็นเพราะใส่ probe วัดจริง กฎ: sabotage ต้อง**แทนที่**พฤติกรรม ไม่ใช่เพิ่มทางเข้า และถ้า
sabotage ไม่แดง ให้สงสัย sabotage ก่อนสงสัยเทสต์

เทสต์แรกของ F1 ก็เป็น decoration (ครั้งที่ห้าในรอบนี้): สร้าง future ของ B แล้ว `await` หลัง poison แต่
future ไม่ทำงานจนกว่าจะถูก poll การอ่านแบบ "ก่อน" จึงเกิดหลัง store อยู่ดี · `join!` ที่ poll ตามลำดับ
คือสิ่งที่ทำให้ interleaving เป็นของจริง

### เหลือจากแผน §11

ข้อ 0 (เช็ค delete↔mail) · G2 · E1 · **F2 ยังเหลือ compiler gate** (ให้ mutator รับ `&MarketGuard`)
— ครึ่งที่แก้ไปคือเลิกรับคอมเมนต์ ยังไม่ใช่การกั้นด้วยคอมไพเลอร์ · E2 · F1 · G1

---

## 13. สถานะแผน §11 (2026-08-17)

| ข้อ | สถานะ | commit |
|---|---|---|
| 0 · เช็ค delete↔mail | **เสร็จ** `custody_is_accountable` ทั้งสอง write path | `57734b9` |
| 1 · G2 ธง full | **เสร็จ** `DirtyRows` — ธง all เดียวเหนือ id space สองชุด | `1153d96` |
| 2 · E1 มาร์กก่อน validate | **เสร็จ** มาร์กเฉพาะ arm ที่ลบจริง | `1fc22cb` |
| 3 · F2 compiler gate | **เสร็จ** `MarketLedger` — `&mut` แลกด้วย `&MarketGuard` เท่านั้น | ด้านล่าง |
| 4 · E2 เพดานราคา | **เสร็จ** `place_sell` บังคับเอง | `60eb4a8` |
| 5 · F1 สูตร I7 | **เสร็จ** และไม่ใช่แค่เอกสาร ดูด้านล่าง | `cd9e867` |
| 6 · G1 `has_expired` | **เสร็จ** O(log n) | `60eb4a8` |

**F1 ใหญ่กว่าที่คิด** — `pg_trade_ledger_commits_with_the_settlement` **เข้ารหัสสูตรผิดไว้เหมือนกัน**
(`SUM(fee) == led.fees_burned`) และผ่านมาตลอดเพราะ fixture มีแต่ commodity trade ไม่มีการขายไอเทม
เลย สองตัวจึงเท่ากันโดยบังเอิญ · แก้: fixture มีการขายไอเทมด้วย และเทียบกับ
`led.fees_burned + items.fees_burned` · sabotage กลับไปสูตรเดิม → แดง (55 ≠ 5)

รูปแบบเดิมอีกครั้ง: **เอกสารที่ผิดกับเทสต์ที่ผิดยืนยันกันเอง** และ fixture ที่แคบเกินทำให้ทั้งคู่ดูถูก

**E2 — ทำไม `add_order` ไม่ต้องเช็ค** (บันทึกไว้แทนที่จะพึ่งเงียบ ๆ): `deposit_commodity` ใน sim-core
ปฏิเสธราคาเกินเพดานอยู่แล้ว และ `policy_check` (`main.rs:4302`) ปฏิเสธ bundle ที่ `sim_version`
ไม่ตรงกับ server ดังนั้น op ที่มาถึงใน save ที่ผ่านแล้ว ถูกเขียนด้วยค่าคงที่ตัวเดียวกันนี้
`place_sell` ไม่มี sim อยู่ข้างหน้า — มาจาก HTTP body ตรง ๆ — จึงต้องบังคับเอง

---

## 14. F2 — compiler gate (2026-08-17)

ไม่ได้ทำด้วยการแก้ signature ของ mutator ทุกตัว (แพง และ `commodity.rs` เรียกในเทสต์เยอะมาก) แต่กั้นที่
**ทางเข้าถึง ledger**:

```rust
pub struct MarketLedger<T>(Mutex<T>);        // ฟิลด์ private ในโมดูลของตัวเอง
impl<T> MarketLedger<T> {
    pub fn read(&self) -> impl Deref<Target = T> + '_;              // DerefMut ถูกซ่อน
    pub fn write(&self, _g: &MarketGuard<'_>) -> MutexGuard<'_, T>; // ต้องมี guard
}
```

**เหตุผลเดิมที่บอกว่าทำไม่ได้ ไม่ผูกอีกต่อไป** — คอมเมนต์ของ
`every_ledger_mutation_is_under_the_market_lock` เขียนไว้เองว่า "coupling `CommodityLedger` เข้ากับ
lock ของ server จะแย่กว่าเทสต์นี้" ถูกต้อง แต่ข้อสรุปตั้งอยู่บนสมมติฐานว่าต้องผูกที่ `CommodityLedger`
ซึ่งไม่จำเป็น: `commodity.rs` ยัง lock-agnostic ทั้งไฟล์ ไม่รู้จัก axum/tokio/`market_persist_lock`
สิ่งที่ถูกผูกคือ **การเข้าถึง** — server เป็นเจ้าของประตู และประตูขอ guard

**ทำไมต้องเป็นโมดูลแยก** — ฟิลด์ private ที่ crate root ยังมองเห็นได้จากทุกโมดูลลูก `self.0.held()`
จะคอมไพล์ผ่านที่ไหนก็ได้ และ "อย่าแตะ `.0`" ก็เป็นธรรมเนียมอีกอัน ซึ่งคือหมวดที่งานทั้งชุดนี้พยายามเลิกพึ่ง

**พิสูจน์แล้ว** (คอมไพล์ไม่ผ่านทั้งคู่):

| ทางลัด | ผล |
|---|---|
| `st.commodity.read().place_sell(...)` | `E0596: cannot borrow ... as mutable` |
| `st.commodity.0.held().place_sell(...)` | `E0616: field 0 of struct MarketLedger is private` |

`every_ledger_mutation_is_under_the_market_lock` **ถูกลบ** — สแกนนั้น disarm ได้สามทางที่เกิดจริงทั้งหมด:
สตริงตรงกับที่อยู่ใน**คอมเมนต์** (`// see MarketGuard` นับ), ย้าย mutation ไป helper แล้วหลักฐานที่มันหาหายไป,
และมันพูดอะไรกับ mutator/handler ที่ไม่มีใครใส่ลิสต์ไม่ได้เลย ตอนนี้เขียนออกมาไม่ได้ทั้งสามแบบ

**⚠️ ย่อหน้าถัดไปเขียนผิด แก้ที่ §15** — ครึ่งที่อ้างว่า "ไม่ต้องมีลิสต์ ยังคุมส่วนอันตรายที่สุด"
ใช้ไม่ได้: มันนับ `market_lock_ignoring_poison(` แต่ตัวสร้างจริงคือ `MarketGuard::take(` audit
เพิ่ม writer คนที่หกที่เรียกตัวหลังตรง ๆ แล้วเทสต์เขียวทั้งชุด

**ยังเป็น source scan อยู่หนึ่งอย่าง** — `every_market_writer_acquires_through_the_poison_gate`
คอมไพเลอร์พิสูจน์ว่า "ถือ lock" แต่ไม่ได้พิสูจน์ว่า "เช็ค poison แล้ว" เพราะนั่นเป็นคำถามเรื่อง**ที่มา**ของ
guard (`usable_market_lock` เทียบกับ `market_lock_ignoring_poison`) ไม่ใช่เรื่องชนิดของมัน
`recover_poisoned_market` **ต้อง** เขียน ledger ขณะ poison อยู่ (มันคือทางเดียวที่จะเลิก poison) จึงแยก
สองชนิดไม่ได้โดยไม่เพิ่มชนิดที่สาม — ประเมินว่าไม่คุ้มตอนนี้ ครึ่งที่ไม่ต้องมีลิสต์ (นับ raw acquisition
ในฝั่งโปรดักชัน) ยังคุมส่วนที่อันตรายที่สุดอยู่

---

## ⚠️ §15-§17 ไปอยู่ใน commit ที่ชื่อไม่ตรงเรื่อง

งานทั้งหมดที่บรรยายใน §15-§17 — `MarketLock`, custody ที่เห็นแผน `full`, `from_rows` ลง clean,
poison มองเห็นได้, เทสต์ฝั่ง item, `send_item`, HTTP harness — **landed ใน `95d2b7e`** ซึ่งข้อความ
commit ของมันคือ *"feat(server): schedule + claim machine, with the 7-day login ladder as its first
track"* คนละเรื่องกันเลย

เกิดจากสอง session ทำงานบน worktree เดียวกัน แล้วอีกฝั่ง `git add` กว้างไปตอนที่งานนี้ยังไม่ commit
ไม่มีอะไรหาย (ตรวจครบทุกชิ้น เทสต์ 178 ผ่าน) และไม่ได้ rewrite history เพราะ tree ใช้ร่วมกันอยู่

**ถ้ากำลังตามหาว่าอะไรมาเมื่อไหร่:** `git log -S MarketLock` จะชี้มาที่ `95d2b7e` ให้อ่าน §15-§17
แทนข้อความ commit ส่วนงานก่อนหน้านั้น (`e0e7b05` → `0e0ba93`) ข้อความ commit ตรงเรื่องตามปกติ

---

## 15. Audit รอบ 6 (ultracode workflow, 2026-08-17/18) — 5 confirmed, ปิด 4

13 agent สามเฟส 37 นาที เทียบกับ agent เดียว 3 ชั่วโมงที่ไม่ได้ผลอะไรเลย โครงที่ทำให้ต่างกันคือ
**แยกอ่านออกจากพิสูจน์**: finder 6 ตัวอ่านขนานกัน read-only แล้ว verifier รัน sabotage ใน worktree
ของตัวเอง + ฐานข้อมูลของตัวเอง ผลรายตัวรอดแม้ตัวอื่นล่ม

### F1 — บั๊กจริง: custody มองไม่เห็นแผน `full` ทั้งหมด

`row_plan()` ที่ `Dirty::All` **return ก่อน** โดยไม่เติม `orders_del`/`del` เพราะ writer ลบด้วยการ
truncate สามตารางแล้ว insert กลับ — **แถวหายโดยถูกละเว้น ไม่ใช่โดยมีชื่อ** แต่ `custody_is_accountable`
อ่านแต่ del list `gone` จึงเป็น 0 เสมอ: เช็คไม่ให้ข้อมูลอะไรเลยบนการเขียนที่ล้างทั้งตลาด

หน้าต่างไม่หายาก — ledger อยู่ที่ `Dirty::All` หลัง **`load_market` สำเร็จทุกครั้งบน revert path**
คือทางที่รันตอนมีอะไรผิดพลาดไปแล้ว reproduce ได้: reload → cancel ไม่มี mail → persist คืน true →
`sell_orders = 0`, `mailbox = 0` ของ 500 ชิ้นหายถาวร

**แก้สองชั้น เพราะแต่ละชั้นกลบอีกชั้น** (sabotage แยกกันแล้วแดงคนละตัว):
1. `from_rows` ลง **clean** — เหตุผลเดิม "ยังไม่มีอะไรพิสูจน์ว่าเขียนแล้ว" สับสนหมวด แถวเพิ่งถูก
   *อ่าน* มาจากฐานข้อมูลเดี๋ยวนี้ และราคาไม่ใช่ "เขียนซ้ำหนึ่งครั้ง" แต่คือทุกการเขียนแรกหลัง revert
2. `write_market_rows` นับแถวที่จะหายจากการ truncate (`SELECT id` เทียบแผน) ส่งให้ custody เช็ค
   **ในทรานแซกชันเดียวกัน** — ผลต่างระหว่างตารางกับแผนมองเห็นได้จากในนั้นที่เดียว เช็คเดิมอยู่ข้างนอก

### F2-F4 — สาม bypass ของตัวกั้น lock/poison ปิดด้วยการย้ายเดียว

audit ทำสำเร็จทั้งสามโดยเทสต์เขียวครบ 151: mint guard จาก mutex อื่น · ส่ง `AtomicBool` ปลอมเข้า gate ·
writer คนที่หกผ่าน `MarketGuard::take` **ครึ่งที่ §14 อ้างว่า "ไม่ต้องมีลิสต์" ใช้ไม่ได้จริง**

แก้: `MarketLock` รวม lock กับ flag เป็นวัตถุเดียว และ **`MarketGuard` ไม่มีตัวสร้างนอกโมดูล** ทางเดียว
ที่ได้ guard คือขอจาก `MarketLock` ซึ่งตอบจาก flag ที่ตัวเองถือ — ไม่มี argument ให้โกหก ไม่มี primitive
ชั้นล่างให้เอื้อมข้าม สองในสามคอมไพล์ไม่ผ่านแล้ว

**รูที่เหลือและปิดด้วย type ไม่ได้:** สร้าง `MarketLock` ตัวที่สองแล้วใช้ guard ของมันคุม ledger จริง
ledger ไม่รู้ว่า lock ไหนคุมมัน — คุมด้วยการนับ constructor (`= 1`) เท่านั้น เขียนไว้ตรง ๆ ดีกว่าอ้างว่าปิดหมด

### F5 — refuted: `send_item` กลืน serialize failure

audit ยืนยันว่า **เข้าไม่ถึง** (`Item` เป็น derive ล้วนทั้งกราฟ ไม่มี float/flatten/custom Serialize)
แก้เป็นบรรทัดเดียวเพื่อลบความไม่สมมาตร — row writer ปฏิเสธทั้งการเขียน mail writer กลืนเงียบ ๆ
**ไม่มีเทสต์ และเทสต์ไม่ได้**: สร้าง `Item` ที่ serialize ไม่ผ่านมาไม่ได้เลย

### เพิ่ม: poison มองเห็นได้

`revert_market_or_poison` ไม่เคย log และ `/health` คืน `"ok"` ตายตัว ตอนนี้ log ตอนติด และ `/health`
คืน `"ok market=POISONED"` — **ยัง 200 เสมอโดยตั้งใจ** `fly.toml` ชี้ health check มาที่นี่ และ Fly
restart instance ที่เช็คไม่ผ่าน ซึ่งจะแลกสภาพที่หายเองได้ (save สะอาดครั้งถัดไปเคลียร์) กับการ restart
ที่ทิ้ง ledger ในหน่วยความจำไปด้วย

---

## 16. ค้างไว้โดยตั้งใจ — อย่าอ่านเป็นหนี้ค้าง

ประเมินแล้ว 2026-08-18 ว่า **ไม่มีอันไหนบล็อกการเปิด** เกณฑ์: ปล่อยขึ้นโปรดักชันแล้วมีอะไรพังสำหรับ
ผู้เล่น หรือมีเหตุการณ์ไหนกู้/วินิจฉัยไม่ได้ไหม

| | สถานะจริง | ทำเมื่อไหร่ |
|---|---|---|
| ~~**HTTP harness**~~ | **เสร็จ 2026-08-18** ดู §17 | — |
| **assertion "pending DELETE survived the reload"** | audit บอกว่า falsify ไม่ได้ ซึ่งถูก — แต่ §10 บันทึกไว้เองแล้วว่า mark ที่คืนมาสังเกตไม่ได้โดยนิยาม มันจึงเฝ้าสิ่งที่ปลอดภัยอยู่แล้ว | เขียนคอมเมนต์ว่าทำไมมันจริงเสมอ ไม่ใช่พยายามทำให้แดงได้ |
| **future cancellation** | ไล่แล้ว **หายเองตอน restart**: mark หาย → หน่วยความจำล้ำหน้า DB → ไม่มีมูลค่าหาย ออเดอร์ยังอยู่ใน DB ไม่มี mail ถูก commit ซื้อซ้ำไม่ได้เพราะไม่อยู่ในหน่วยความจำ · ผู้เล่นเห็นว่า "กดยกเลิกแล้วไม่ยกเลิก" | **1-2 วัน และเป็นการเปลี่ยนดีไซน์** ดูกับดักด้านล่าง |

### ⚠️ กับดักของ future cancellation — ทางลัดที่ใช้ไม่ได้

ทางที่ฟังดูง่ายคือ "spawn ตัว persist ออกไป client จะได้ drop มันไม่ได้" **ใช้ไม่ได้**:
`MarketGuard` ยืม lock อยู่ จึงข้าม spawn boundary ไม่ได้ และถ้าย้ายไปถือ lock ข้างในงานที่ spawn
แทน ก็แปลว่า handler mutate ledger ไปก่อนแล้วค่อยล็อก — คือ **mutate-then-lock** ซึ่งเป็นบั๊กที่
`28258d2` แก้ไปแล้ว (revert ที่วิ่งพร้อมกันจะ reload ทับการเปลี่ยนแปลงที่ apply แล้วแต่ยังไม่เขียน)

เวอร์ชันที่ถูกคือ handler แค่ validate แล้วปล่อย *คำสั่ง* ออกไป มี owner task เดียวที่ apply + persist
ใต้ lock — actor/queue ซึ่งเป็นราคาที่แพงเกินสำหรับสิ่งที่หายเองและไม่มีมูลค่าหาย **ถ้าจะทำ อย่าเริ่ม
ด้วย spawn**
| **`MarketLock` ตัวที่สอง** | คุมด้วยการนับ constructor เท่านั้น type ปิดไม่ได้ | ถ้าวันหน้าย้าย ledger เข้าไปอยู่ใน `MarketLock` จะปิดได้ — ใหญ่เกินคุ้มตอนนี้ |
| **pg test skip เงียบเมื่อไม่มี Postgres** | บนเครื่องที่ไม่มี DB ฟีเจอร์ custody ทั้งอันเหลือแค่ source scan ค้ำ `smoke.sh` รายงาน PARTIAL อยู่แล้ว | ตอนตั้ง CI |
| **harness ของ workflow** | worktree ของ verifier ถูก checkout ที่ `896e3ac` (เก่าจนยังไม่มี `commodity.rs`) ทุกตัวจับได้เอง แต่รันแบบไม่มีคนดูจะได้รายงานมั่นใจเกี่ยวกับโค้ดที่ไม่มีอยู่ | ก่อน workflow รอบหน้า |

---

## 17. HTTP harness (2026-08-18) — เดิน router จริงแบบที่ client เดิน

**ทำเลยแทนที่จะเลื่อน เพราะเหตุผลที่ผมใช้เลื่อนมันผิด** — ผมอ้างว่าของแพง (bundle ที่ผ่าน replay)
จะถูกใช้ร่วมกับ tier 3 แต่เช็คแล้ว `daily_claim`/`mail_claim` รับแค่ auth + payload เล็ก ไม่ใช้
`SaveBundle` เลย มีแต่ `put_save` ที่ใช้ **การเลื่อนจึงไม่ทำให้ถูกลง** เสียแค่บริบทที่มีอยู่ตอนนั้น

### ชิ้นส่วน

- `build_router(state, dev_routes)` — แยกจาก `main` ไม่เปลี่ยนเส้นทางใด ๆ นั่นคือประเด็น: เทสต์ที่
  สร้าง router เองพิสูจน์เรื่อง router ของเทสต์ อันนี้คือตัวที่เซิร์ฟเวอร์เสิร์ฟจริง
- `[dev-dependencies] tower + http-body-util` — `oneshot` รันใน process ไม่มี socket ไม่มี port
- `seed_session()` — seed สำคัญ: `start` ของ bundle จะ trusted ก็ต่อเมื่อเป็น genesis ที่เซิร์ฟเวอร์
  **สร้างซ้ำได้เอง**จาก seed ของบัญชีนั้น (`genesis_candidates`)
- `verifiable_bundle()` — **`SaveBundle::capture` ใช้ไม่ได้**: มันอ่าน log กลับจาก `live.input_log`
  แต่ `replay()` ปิด `record_inputs` สภาพที่ replay แล้วจึงมี log ว่าง ประกอบเองให้ mirror
  `verified_state` ทุกจุด (base เดียวกัน `snapshot_interval` เดียวกัน replay เดียวกัน) hash จึงตรง
  **โดยโครงสร้าง ไม่ใช่โดยบังเอิญ**

### เทสต์

`pg_an_honest_save_is_accepted_through_the_real_router` — **harness พิสูจน์ตัวเองก่อน** ถ้าไม่มีอันนี้
เทสต์ปฏิเสธทุกตัวจะผ่านบน harness ที่ปฏิเสธทุกอย่าง ซึ่งเป็นรูปแบบ "เขียวเพราะไม่มีอะไรรัน" ที่ไฟล์นี้
โดนมาแล้วหลายครั้ง

`pg_a_save_that_sweeps_the_market_commits_rows_and_mail_together` — ออเดอร์ที่เลย TTL ทำให้ save
**ธรรมดาที่ไม่มี market op เลย** กลายเป็น market write (`touches_market` → sweep → `market_changed`)
เดินครบ reconcile → `persist_save_and_market` → `commit_save_and_market` → custody →
`write_market_rows` ยืนยัน **การจับคู่**: แถวหาย **และ** Return mail ต้องมี

sabotage: ไม่เขียน row → แดง · ไม่ commit mail → แดง (พาเทสต์เก่าแดงด้วย)

### สิ่งที่ harness นี้ยัง**ไม่**จับ

ถอด `custody_is_accountable` ออกจาก settlement path แล้ว**ยังเขียว** เพราะ custody กันแผนที่*ไม่ซื่อสัตย์*
ซึ่ง client ที่ซื่อสัตย์เขียนออกมาไม่ได้ — มันกันบั๊กภายใน ไม่ได้กัน input ส่วนนั้นยังคุมด้วย unit test
ตรง ๆ กับ `every_writer_of_rows_checks_custody` เขียนไว้ให้ชัดดีกว่าปล่อยให้เข้าใจว่า harness คุมทุกอย่าง

**ของแพงจ่ายครั้งเดียวแล้ว** เทสต์ระดับ HTTP ตัวถัดไปราคาไม่กี่บรรทัด

---

## 18. ทดสอบกับเซิร์ฟเวอร์จริง (2026-08-18) — T2 และ T4 ผ่าน

deploy `f1ee5cf` แล้วเล่นจริง `check_online_sync.sh` ยืนยันก่อนเล่นว่า tree/dylib/server อยู่ที่
commit เดียวกัน — จำเป็น เพราะ dylib ในเครื่องตอนนั้นเก่ากว่าเซิร์ฟเวอร์ 30+ commit ขณะที่
`sim_version` เป็น `0.0.294-alpha` เท่ากันทั้งคู่ `policy_check` เทียบแค่สตริงจึงปล่อยผ่าน แล้ว
replay ค่อยแตกทีหลังแบบเงียบ ๆ นั่นคือเหตุผลทั้งหมดที่สคริปต์นั้นมีอยู่

**T2 · cancel ครบวง — ผ่าน** ลงขาย → ยกเลิก → หายจากกระดานและ "ของฉัน" → Return mail เข้ากล่อง →
claim → ของกลับ นี่คือ path เดียวที่ของหายถาวรได้: หน่วยออกจาก GameState ตอนลงขาย และ mail คือทาง
กลับทางเดียว

**T4 · รอด restart — ผ่านทั้งสองครึ่ง** boot log หลัง `flyctl machine restart`:

```
loaded market: 2 orders
verify-server (sim-core 0.0.294-alpha) listening on http://0.0.0.0:8080
```

ไม่มี `REFUSING`, ไม่ exit(1), ไม่ panic — ออเดอร์อ่านกลับจาก Postgres ครบ

**ครึ่งที่สำคัญกว่า: ลงขายเพิ่มหลัง restart แล้วของเดิมไม่หาย** นี่คือสิ่งที่ §15 F1 แก้ ก่อนหน้านั้น
ledger กลับมาในสภาพ `Dirty::All` ทำให้การเขียนถัดไปเป็นแผน `full` ที่ truncate สามตารางแล้วใส่กลับ
เฉพาะที่ตัวเองถือ — การเขียนแบบเดียวที่ `custody_is_accountable` มองไม่เห็น ถ้ายังพัง ของเดิมสอง
รายการจะหายเหลือแต่อันใหม่ **มันไม่หาย**

log ตลอดช่วงเล่น: `POST /save -> 200` 13 ครั้ง ไม่มี 4xx/5xx และไม่มี `CUSTODY` / `POISONED` /
`DURABILITY` สักบรรทัด

**T1 · trade เต็มวง — ผ่าน** สองบัญชี ซื้อจริง เงินข้ามมือ · การซื้อไม่มี endpoint ของตัวเอง มัน
ขี่ไปกับ `POST /save` ตามที่ออกแบบ (ไม่มี `/commodity/buy` ตรง ๆ เพื่อไม่ให้ settle ข้ามบัญชีได้โดย
ไม่ผ่านด่าน verified-save)

**T3 · item market — ผ่าน** รวมถึง `POST /item-market/cancel` ซึ่งเป็น custody ที่มีมูลค่าสูงสุด:
commodity เป็นกองที่ทดแทนกันได้ ส่วน listing ของไอเทมคือชิ้นเดียวที่ไม่มีตัวแทน

**สรุปทั้งรอบ: 99 คำขอ ทั้งหมด 200** ไม่มี 4xx/5xx และไม่มี `CUSTODY` / `POISONED` / `DURABILITY`
/ `REFUSING` สักบรรทัด · ปริมาณ: `GET /mail` 10, `GET /commodity` 10, `GET /item-market` 6,
`/item-market/mine` 3, `/item-market/cancel` 1

### ยังไม่ได้ทดสอบในเกม และไม่ต้อง

- **หมดอายุ** (order 3 วัน / listing 7 วัน) — คุมด้วย
  `pg_a_save_that_sweeps_the_market_commits_rows_and_mail_together` ซึ่งเดินเส้นเดียวกันทั้งเส้น
- **503 poison** — ต้องให้ DB ล้มถึงจะเกิด ไม่ควรไปยั่วบน Neon · คุมด้วย
  `a_market_poisoned_while_a_writer_waits_gets_no_guard` + `/health` ที่รายงานสถานะแล้ว
