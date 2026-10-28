# Steam Community Market — Requirements + ข้อปฏิบัติ

**Status:** research 2026-07-07, ทวนกับเว็บ 2026-07-12 (ปรับได้เมื่อ Valve เปลี่ยนกติกา)
**Companion to:** `server-plan-phases.md` (market มากับ S2), `economy-invariants.md`,
memory: mass-100k-strategy (market update = second launch)

---

## ฝั่ง developer — เปิดใช้

ทำได้ทันทีที่มี appid (ไม่ต้องรอ release):

1. Steam Economy settings → **Enable Inventory Service**
2. อัปโหลด **ItemDefs** (JSON): ชื่อ, icon (จำเป็นสำหรับ market listing), คุณสมบัติ,
   flag ต่อไอเทม `tradable` / `marketable` — itemdefid ต้อง **< 1,000,000** และห้าม
   reuse ความหมายเดิม (id คือ identity ถาวรของไอเทมในระบบ Valve)
3. Item visibility = **Private** ระหว่าง dev (เห็นเฉพาะ partner group) → Public เมื่อพร้อม
4. Grant ของจาก server ผ่าน Web API `IInventoryService` + **publisher key**
   (= จุด mint-on-verify ของเรา; key เป็น secret ฝั่ง server ห้ามลง client/repo)

## เงื่อนไขก่อน Market ใช้ได้จริง

- เกมต้อง **released** (listing ผูกกับ community hub ของเกมที่ขายจริง)
  → เทสการซื้อขายจริงก่อน release ไม่ได้; เทสได้แค่ grant/consume/inventory
- ตั้ง **game-specific fee** (ส่วนของเราต่อธุรกรรม; มาตรฐานวงการ 10%)
  + Steam fee 5% → ผู้ขายโดนหักรวม ~15%
- ช่วงราคา **$0.01–$100** ต่อชิ้น (เพดานสูงกว่านี้ Valve เปิดเฉพาะบางเกม)
  → ออกแบบ tier ให้ของหมุนเวียนส่วนใหญ่อยู่ในช่วงนี้
- **เงินขายเข้า Steam Wallet เท่านั้น — ผู้เล่นถอนเป็นเงินสด/ธนาคารไม่ได้** (ยืนยัน web
  2026-07-12). "ขายได้เงินจริง" ในทางปฏิบัติ = ได้ Wallet credit หมุนใน Steam; มีแต่
  **developer** ที่ได้ payout จริงจาก Valve → game fee 10% คือรายได้จริงของเราต่อธุรกรรม

## กติกาที่ Valve บังคับ

- **Market resale hold: ของที่ซื้อจาก Market ห้ามขาย/เทรดซ้ำภายใน ~1 สัปดาห์** (web
  2026-07-12; แก้จากที่เคยจด "3 วัน"). แยกจาก **trade cooldown** ที่ dev ตั้งเองผ่าน
  `TradabilityRestrictions` (CS ใช้ 7 วัน). ⚠️ ตัวเลขเป๊ะให้ยืนยันที่ schema doc ก่อนใช้จริง
  (Steam Support หน้า render ด้วย JS ดึง verbatim ไม่ได้)
- **restriction ถูก copy ตอน transfer + stack ได้ "วันที่ล่าสุด (เข้มสุด)"** — ของ
  tradable/marketable ที่รวม stack → cooldown = อันใหม่สุด. **กระทบ design:** ของเพิ่ง mint
  รวมเข้า stack เดิม → ลาก cooldown ทั้ง stack → marketable ควรเป็น per-instance ไม่ stack
- **item = "license rights" ไม่ใช่ ownership; Valve ไม่รับรอง transfer นอก Steam** →
  RMT นอก Steam = โมฆะ/ผิดกติกา (ตรงกับ design เรา: เงินจริงผ่าน Valve rail เท่านั้น)
- **developer ต้องเป็นเจ้าของ IP ที่อัปโหลด** → art ที่จ้าง commission **ต้องมีหนังสือโอนสิทธิ์
  ก่อนอัป itemdef/icon** (ไม่งั้นผิดสัญญา — ค่าเริ่มต้น: ผู้ทำถือสิทธิ์ร่วมถ้าไม่มีสัญญาโอน)
- **ห้ามโฆษณา RMT/market บน store page** ตอน submit (จดใน steam-launch-plan A3;
  เลี่ยง gambling flag) — ระบบทำงานเงียบ ๆ จนถึงวันประกาศ market update
- Content questionnaire ตอบตามจริง — จุดแข็งเรา: **ดรอปจาก gameplay ล้วน
  ไม่มี paid loot box / กุญแจสุ่ม** → เส้นผิดกฎหมายคือ "จ่ายเงิน + สุ่ม + เนื้อห mint
  cash-out ได้" (NL/BE) → โมเดล gameplay-drop = คูเมืองกันเรื่องนี้; **ห้ามเผลอทำกล่องสุ่ม
  ซื้อเงินที่ออกของ marketable**
- **Platform MFN:** ห้ามขายเกมถูกกว่า / ให้ content มากกว่าบน platform อื่น (tangential)
- อย่า promise มูลค่าไอเทม; เก็บสิทธิ์ปรับ economy ใน EULA

## ฝั่งผู้เล่น (= กำแพง anti-bot ชั้นแรกที่ Valve ให้ฟรี)

ผู้เล่นจะซื้อ/ขายบน Market ได้ต่อเมื่อ:

- บัญชีไม่ limited: **เคยเติมเงิน ≥$5**
- มีการซื้อสำเร็จในช่วง **7 วัน – 1 ปี** ที่ผ่านมา
- ไม่มี **Steam Guard Mobile Authenticator** (เปิด ≥7 วัน) → ของที่ลิสต์โดน
  **hold 15 วัน**
- ล็อกอินเครื่องใหม่ → market restriction **7 วัน**

→ ทุก bot account มีต้นทุน $5 + มือถือ/authenticator + เวลาบ่ม ก่อนถึง
verify pipeline ของเราด้วยซ้ำ — คิด economics ของ bot จากฐานนี้

## ลำดับ integrate (สอดคล้องแผนที่ตัดสินแล้ว)

1. **ก่อน launch**: auth + inventory integration เสร็จ, ทุก itemdef
   `tradable:false, marketable:false`, visibility Private → Public ตอน release
   (ของเข้า inventory ผู้เล่นได้ แต่เทรดไม่ได้ = ไม่มีความเสี่ยง economy)
2. **Launch (ต.ค.)**: เกมออก, mint-on-verify เก็บชั่วโมงบิน, สะสม trust ใน S2
3. **Market update (second launch)**: S2 verify + anti-bot พร้อม → พลิก flag
   เป็น tradable/marketable + ตั้ง game fee → ประกาศเป็นอีเวนต์ใหญ่ พร้อม zh push

## Sources (เช็ค 2026-07-07, ทวน web 2026-07-12)

- partner.steamgames.com/doc/features/inventory (+ /schema, /economy/TradabilityRestrictions)
- help.steampowered.com Community Market FAQ (61F0-72B7-9A18-C70B)
- help.steampowered.com Trading and Market Restrictions (451E-96B3-D194-50FC)
- tandfonline.com 10.1080/14459795.2024.2390827 (illegal loot boxes on Steam — legal line)
- Steamworks Distribution Agreement (item = license, no transfer outside Steam; IP ownership; MFN)
