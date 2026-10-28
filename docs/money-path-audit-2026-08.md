# Audit เส้นเงินจริง (store / gacha / crystal) — 2026-08-19

15 agent สามเฟส · **8 confirmed, 0 refuted, 0 inconclusive** · แก้ไปแล้ว 3 ค้าง 5

> audit เขียนข้อจำกัดของตัวเองไว้ และควรอ่านก่อนเชื่อผลว่าครอบคลุม:
> *"ไม่มีอะไรถูก refute และไม่มีอะไร inconclusive — ทุก candidate ที่ตรวจยืนยันหมด นั่นไม่ใช่
> เรื่องน่าอุ่นใจ มันแปลว่าตัวจำกัดคืออัตราการสุ่มตรวจ ไม่ใช่อัตราการมีบั๊ก"*

## แก้แล้ว

| | commit |
|---|---|
| **F1** `gacha_rolls`/`gacha_exchanges` idem_key เป็น PK ระดับ global แต่ตัวค้นหาเป็นราย account → บัญชีอื่นโรลไม่ได้ตลอดกาล · `store_checkout` replay ไม่มี account clause เลย คืน order_id/transid ของคนอื่น | `638cbbe` |
| first-buy bonus อ่านนอกทรานแซกชัน + guard `<> 'granted'` ปล่อย refunded ผ่าน | `eca8a08` |
| conservation check ไม่มี runner — boot + รายชั่วโมง + `/health` | `8deb70c` |

## ค้าง — ไม่เหลือแล้ว

| | commit |
|---|---|
| **F4** refund revocation ไม่มีใครอ่าน → `owns_sku()` เป็นคำถามเดียว filter อยู่ในตัวคำถาม + source scan + pg test ที่ให้สิทธิ์/คืนเงิน/ต้องไม่ถือ | `1371399` |
| **F2** การหักเจมเป็นการเขียนค่าสัมบูรณ์จากค่าที่อ่านมา → delta พร้อมพื้น `>= $2` จ่ายไม่ไหว = แมตช์ 0 แถว = การจั่วล้ม ไม่ใช่การจั่วฟรี | `1749ea5` |
| **F3** guard กันชาร์จซ้ำไม่มีใครแตะ → transaction จริงสองตัวแข่งกัน + source pin ว่าจอง key ก่อนหักเงิน | `1749ea5` `d53dbac` |
| คูปองไม่มี ledger เลย (เจอตอนตรวจ F2/F3 ซ้ำ) → `bound_ledger` + conservation test + source guard สองครึ่ง | `d53dbac` `42bb8be` |
| **F5** `!out.is_empty()` บน `&'static str` → resolve กับแคตตาล็อกจริง และ SSR ต้องเป็น Legendary | `fbd195c` |
| **F6** `EXCHANGE_POOL` SSR-only ค้ำด้วยคอมเมนต์ → ทุกตัวต้องมีจริงและเป็น Legendary | `fbd195c` |

**ทุกข้อ falsify แล้ว** — ใส่บั๊กกลับ ดูว่าแดง แล้วคืนค่า สาม sabotage แรกเขียวด้วยเหตุผลผิด
(compile ไม่ผ่าน / pattern ไม่ตรง) และถูกไล่หาสาเหตุแทนที่จะนับเป็นผ่าน

## ไม่ได้ตรวจ (ชน cap) — ยังไม่มีใครดู ไม่ใช่ว่าสะอาด

1. **fixture ของ conservation check อาจไม่เคยมีเคสที่ `LEFT JOIN` มีไว้เพื่อมัน** — ทั้งสองบัญชีใน
   เทสต์มีแถว ledger หมด บัญชีที่มี wallet แต่ **ไม่มี ledger เลย** (ซึ่งคือสิ่งที่
   `/dev/gacha-crystal` ผลิตเมื่อ ledger insert ที่ถูกทิ้งผลล้มเหลว) อาจมองไม่เห็นถ้าเปลี่ยนเป็น
   INNER JOIN · **นี่คือรูป I7 เป๊ะ ๆ: สูตรกับ fixture เห็นด้วยกันเองโดยไม่มีใครทดสอบข้ออ้าง**
2. `store_checkout` replay ที่ไม่ scope — read-confirmed แล้วแก้ไปที่ `638cbbe` แต่ไม่เคยถูกรัน

## จุดที่ audit ชี้ว่าน่าจะยังมีบั๊กซ่อนอยู่

**`store_refund` อ่านจำนวนที่จะกลับรายการ *นอก* ทรานแซกชันที่กลับรายการ** — `granted_tradeable` /
`granted_bound` อ่านจาก pool ที่ `:2169` ส่วน `db.begin()` อยู่ที่ `:2178` ถ้า `store_finalize`
commit ในช่วงนั้น refund จะกลับรายการเป็น 0 ทั้งที่ยังตั้ง status เป็น refunded และเพิกถอน
entitlement — **รูปเดียวกับ first-buy ที่เพิ่งแก้ไป และผมมองข้ามตอนแก้ตัวนั้น**
