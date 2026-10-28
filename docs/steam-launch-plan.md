# Steam Page + 3-Month Launch Plan

**Status:** วางแผน 2026-07-03 (ปรับได้) — บริบท: TBH churn เริ่มแล้ว, window เปิดอยู่,
clone flood คาดใน 3-6 เดือน. เป้า: page เร็วสุด, launch ~ต.ค. 2026.
**Companion to:** `server-plan-phases.md` (server = post-launch), `depa-*` (เช็คเงื่อนไขทุน)

---

## Part A — เปิด Steam page (เริ่มได้วันนี้)

### A1. Steamworks onboarding — เริ่มก่อนเพราะ lead time นอกการควบคุม

- [ ] สมัคร Steamworks partner + จ่าย Steam Direct fee **$100/app** (ได้คืนเป็น credit
      เมื่อรายได้ถึง $1,000)
- [ ] **ตัดสินใจ entity: บุคคล vs บริษัท** — กระทบภาษี (W-8BEN, สนธิสัญญาไทย-US)
      และอาจกระทบเงื่อนไข depa → เช็คก่อนเซ็น
- [ ] Identity + tax interview + bank — ปกติไม่กี่วัน แต่เผื่อ 2 สัปดาห์
- [ ] เช็คชื่อ "ROI: Reign of Idlers" ไม่ชนใน Steam / จอง social handle

### A2. Assets (ขนาดปัจจุบัน หลังกฎ Aug 2024: artwork + ชื่อเกม + subtitle เท่านั้น)

| Asset | ขนาด | หมายเหตุ |
|---|---|---|
| Main capsule | 1232×706 | ตัวขายหลัก — ควรจ้าง (~1-2 สัปดาห์ turnaround, $150-500) |
| Header capsule | 920×430 | ตัดจาก main |
| Small capsule | 462×174 | ชื่อเกมต้องอ่านออกในขนาดจิ๋ว |
| Vertical capsule | 748×896 | |
| Library: capsule/header/hero/logo | 600×900 / 920×430 / 3840×1240 / PNG | |
| Screenshots ≥5 | 1920×1080 | เกมจริง — ต้องมีช็อต **companion strip บน taskbar** (ภาพที่ขายตัวเอง) |
| Trailer 30-60s | 1080p | เกม idle ขายด้วย loop เดียว: ปาร์ตี้ตีมอน→ของดรอป→strip บน taskbar |

- [ ] จ้าง capsule art (คิวศิลปิน = lead time ยาวสุดรอง onboarding — สั่งสัปดาห์นี้)
- [ ] ถ่าย screenshots + GIF จาก build จริง (companion mode คือ hero shot)

### A3. Copy + metadata

- [ ] Short description ≤300 chars (EN) — pitch: "RO-style idle ARPG on your taskbar:
      party builds, cards, risky refine — it farms while you work"
- [ ] Long description แบบ GIF-นำ (คนไม่อ่าน wall of text)
- [ ] Tags 20 อัน — **5 แรกน้ำหนักสูงสุด**: Idle, RPG, Auto Battler, Loot,
      Free to Play — **เทียบ profile กับ TBH ให้ใกล้** (More Like This = ท่อรับ
      TBH churner). "Incremental/Clicker" ถอยไปกลาง list: เกมเราคือ idle RPG
      (โตแบบขั้นบันได RO + soul-path prestige) ไม่ใช่ exponential incremental —
      อย่าเคลมนำ เดี๋ยวโดนรีวิว mismatch
- [ ] ภาษา: ตาม **`localization-plan.md`** (ตัดสิน 2026-07-07 — หลายภาษาแต่แรก):
      page copy localize ทุกภาษา T1 (EN/TH/zh-CN/zh-TW) ตั้งแต่ page live;
      pt-BR/ru/ja/ko ตามเมื่อ review ผ่าน
- [ ] Content questionnaire — **ห้ามโฆษณา real-money trading บนหน้า page**
      (Valve gate marketability หลัง release อยู่แล้ว + เลี่ยง gambling flag;
      market เป็น update ทีหลังตาม server-plan)

### A4. Submit + เปิดตัว page

- [ ] Submit review — ใช้ **3-5 วันทำการ** (เผื่อ 7 วัน มีแก้)
- [ ] กฎ: page ต้อง live **≥14 วัน**ก่อนปล่อยเกม (เราจะ live เป็นเดือนอยู่แล้ว)
- [ ] วัน page live = burst รอบแรก: r/incremental_games, ชุมชนรอบ TBH
      (discussions/Discord/subreddit — โทนเบา ๆ "แนวเดียวกันแต่มี party build แบบ RO"),
      กลุ่มเกมไทย — wishlist velocity สัปดาห์แรกดันเข้า Popular Upcoming

**Timeline Part A: page live ได้ภายใน ~3 สัปดาห์จากวันนี้** (คอขวด = onboarding + capsule art)

---

## Part B — Timeline: เร็วที่สุด, เพดานแข็ง 6 เดือน (user ตัดสิน 2026-07-03)

แผน 3 เดือนข้างล่างคือ TRACK เป้าหมาย; ถ้าหลุดมี buffer ถึง ~ม.ค. 2027 (เพดาน 6 เดือน)
— เดือนที่เกินใช้กับ: Next Fest, zh-CN loc, playtest รอบเพิ่ม, S1 groundwork.
รูปแบบปล่อย: **เกมจริง F2P + ป้าย Early Access** (ไม่ใช่ Steam Demo — F2P ไม่มี demo;
วงลอง = Steam Playtest เดือน 2), เนื้อหา = demo-goal scope (Lv70 + 1 advancement).

**เงื่อนไขเดียวที่ทำให้เร็วได้: launch แบบ LOCAL ก่อน, server/online-required มากับ
market update หลัง launch.** ทำ 3 เดือนพร้อม server day-1 ไม่ได้แบบมีคุณภาพ
(S2 เดี่ยว ๆ = 2-3 เดือน และเปิด online วันแรกใส่คลื่นผู้เล่น = ซ้ำรอย TBH 8 มิ.ย.
โดยไม่มีทีม). การ phase-in online-required ทีหลังทำได้เพราะ save deterministic:
grandfather save เดิมครั้งเดียว → hash-pin จากนั้น.

### เดือน 1 (ก.ค.) — content จบ + page live

- [ ] Part A ทั้งหมด (ขนานกับงานเกม)
- [ ] Band 10-11 + หน้า advancement (P2) + balance pass รายคลาส (scope demo เดิม)
- [ ] **Boss walls re-check** (0.0.128 boss แข็งขึ้นจริง) + difficulty re-tune
      ที่ค้างจาก RO-HP port — band pacing ทั้งเส้นต้องนั่งใหม่ก่อนคนจริงเข้า
- [ ] **แหล่ง card socket** ขั้นต่ำ (ตอนนี้ไม่มี gear มี socket เลย → การ์ด inactive
      ทั้งระบบ) — เกมจะ ship โดยการ์ดไม่ทำงานไม่ได้
- [ ] Album UI แบบ (a) minimal + S0 hygiene 2 จุด (ถูก, กันหนี้)
- [ ] **i18n infra** (~2-4 วัน ตาม `localization-plan.md`): strings.csv + loc_extract +
      `Loc` helper + bundle ฟอนต์ fallback CJK/ไทยเป็น theme font (UI ใช้ Godot
      default อยู่ — ครั้งเดียวจบ); กฎ no-literal มีผลทันที

### เดือน 2 (ส.ค.) — first hour + playtest

- [ ] **Onboarding/starter gear/tutorial** — ความเสี่ยงฝั่งเกมอันดับ 1:
      F2P ตัดสินกันที่ชั่วโมงแรก และกำแพง b1→b2 มีอยู่จริง (จดไว้ใน memory แล้ว)
- [ ] Performance ของ idle footprint: CPU/GPU/แบตต่ำ, tray/strip เสถียร, ห้าม crash
      (CCU flywheel ตายเพราะ crash)
- [ ] **Steam Playtest** (ฟรี ไม่ต้องรีวิว) — recruit จากชุมชน TBH-churner ตรง ๆ
      → รอบ feedback 2-3 สัปดาห์
- [ ] **String freeze ก่อน playtest** → แปล T1 (EN/TH/zh-CN/zh-TW) batch เดียว +
      ทดสอบฟอนต์/overflow บน build จริง (ตาม `localization-plan.md`), trailer ตัดจริง

### เดือน 3 (ก.ย.) — แก้จาก playtest + launch prep

- [ ] แก้ top issues จาก playtest (กันเวลาไว้ครึ่งเดือน — ของจริงโผล่ตรงนี้เสมอ)
- [ ] Loc QA: native review zh, แก้ truncation, สลับภาษาทุกจอ; T2 (pt-BR/ru/ja/ko)
      แปลรอ review — ship เมื่อผ่านเท่านั้น
- [ ] Streamer/press outreach (คลิป companion mode สั้น ๆ คือ pitch), press kit
- [ ] ตัดสิน: launch เอง ~ต้น ต.ค. บน wishlist burst **หรือ** เกาะ Next Fest ต.ค.
      (demo ใน fest → release ปลายเดือน = เลื่อนเป็น ~3.5-4 เดือน แลก visibility ฟรี
      ก้อนใหญ่ — ตัดสินตอนเห็นยอด wishlist ปลายเดือน 2)

### Cut list (ของที่ไม่ทำใน 3 เดือนนี้ — จดไว้ ไม่หาย)

Server ทั้ง S1-S4 (ตาม `server-plan-phases.md`), Steam Market/trade, star/fuse,
drop-system 2-axis, Duskmourn, PvPvE, 2nd-branch classes, multi-party premium

### ความเสี่ยงที่ต้องยอมรับ

1. **Launch local = โกงได้ระหว่างรอ server** — รับได้เพราะยังไม่มีของกระทบคนอื่น
   (ไม่มี trade) และ S0 hygiene ทำให้ save ยัง verify ย้อนหลังได้ตอน server มา
2. **เงื่อนไข depa** — DEFERRED (user 2026-07-03): ยังไม่ได้ทุน — ค่อยพิจารณา
   ถ้า/เมื่อได้ทุนจริง (ถ้าได้ทุนแล้วจะ launch ในช่วงทุน ค่อยคุยเงื่อนไขตอนนั้น);
   ไม่ใช่บล็อคเกอร์ของไทม์ไลน์ตอนนี้
3. Solo + live F2P = treadmill เริ่มวัน launch — เดือนแรกหลังปล่อยคือ support/hotfix
   เต็มเวลา วางแผนพลังงานไว้ด้วย
