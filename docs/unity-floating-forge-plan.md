# Floating Fusion Forge — Equipment / Gems

> Historical design exploration. The implemented MVP behavior, latest fusion failure/crafter rules, native-window evidence and agent handoff are in [unity-floating-forge-implementation.md](unity-floating-forge-implementation.md). Do not implement the superseded API gaps or layout tables below without checking that document.

สถานะ: Unity MVP implement แล้ว; ดู `unity-floating-forge-implementation.md` สำหรับผลตรวจและข้อจำกัด
อัปเดตตามผู้ใช้: ใช้ทั้ง Companion/Battle; ตัวละคร+กระเป๋าคือหน้าหลัก และแถบล่างสลับหน้ารองทางขวา
รายละเอียดด้านล่างเก็บเป็นแผนตั้งต้น; สถานะ implementation/ขนาด/layer ที่ใช้งานจริงให้ยึดเอกสาร implementation ด้านบน
วันที่: 2026-09-16 · ฐานที่ตรวจ: `70d428d1`, sim 0.0.397 / catalog 0.0.143
Worktree: `/Users/lunalieneray/idle_mmorpg-gem-ui` · branch `codex/gem-ui-mvp`

ผู้ใช้ยืนยันเลิก Godot ถาวร: งานนี้เป็น Unity เท่านั้น ไม่มีงาน port/รักษา feature parity กับ Godot
เอกสาร handoff เก่าบางส่วนยังเขียน 11/12 ช่อง, sockets ตาม def, refine/enchant — ให้ยึด
`gem-system-design.md` §8 และโค้ดที่ commit ข้างต้นแทนเมื่อขัดกัน

## 1. Reference และสิ่งที่หยิบมาใช้

- **WoG ภาพที่ผู้ใช้แนบล่าสุด**: Character/inventory ซ้าย, Equipment Forge ขวา, ช่องวัตถุดิบรอบผลลัพธ์,
  ปุ่มหลักอยู่ด้านล่าง, เหลือพื้นที่เห็น combat ใต้หน้าต่าง ใช้เป็นเรฟสัดส่วนและตำแหน่ง
  ภาพต้นฉบับ: `/var/folders/91/1s5x6fqx1gqbkxwhd2khx9jh0000gn/T/codex-clipboard-4692b060-fe0c-45fd-a2e8-b5b32a4ad79b.png`
- **TBH Cube**: inventory กับเครื่องผสมอยู่ข้างกัน, Auto Fill, แท็บแยกงานและคำอธิบายก่อนผสม
  [ภาพหน้าจอในคู่มือ Cube](https://mobalytics.gg/gamebase/guides/task-bar-hero-cube-synthesis)
  และ [ภาพ inventory/Cube](https://storage.ghost.io/c/b9/32/b9320fa4-9d41-450c-82d4-f517eadd2af7/content/images/size/w1200/2026/05/image-28.webp)
  แหล่งนี้ใช้เพื่อดู UI เท่านั้น กติกา TBH ไม่ใช่กติกา ROI
- [TBH official](https://store.steampowered.com/app/3678970/TBH_Task_Bar_Hero/) /
  [WoG official](https://store.steampowered.com/app/4891320/): บริบท desktop companion
- ROI ใช้ตัวหนังสือ Noto/TMP คม, พื้น slate, ivory text, champagne accent จาก UI เดิม
  หน้าต่างเล็กเป็น screen-space ตรง; Home perspective คงเป็นดีไซน์ของ Home
- ตามคำขอล่าสุดของผู้ใช้: Result เป็นแถบ Item Acquired ข้ามกลุ่มหน้าต่าง Character/Inventory + Forge พร้อมแสงและไอเทม/Gil ที่ได้รับจริง กดปิดหรือปิดอัตโนมัติ โดย combat ยังเดินต่อ

## 2. Layout ที่เสนอสำหรับ MVP

**โครงที่ผู้ใช้ยืนยัน:** หน้าต่างหลักรวมตัวละคร/paper doll กับกระเป๋าอยู่ด้วยกันเสมอ
แถบเมนูด้านล่างของหน้าต่างหลักเป็นตัวเลือกหน้ารอง ซึ่งเปิดชิดทางขวาได้ครั้งละหนึ่งหน้า
Forge เป็นหนึ่งในหน้ารอง ไม่ใช่หน้าหลักที่เรียก inventory picker ขึ้นมาประกอบ
รายชื่อปุ่มอื่นยังเป็นข้อเสนอ; ใช้ Forge / Socket Gem / Storage ในผังเพื่อทดลอง navigation

ขนาดตั้งต้นใน canvas logical pixels (ต้องลองที่ DPI/ความละเอียดจริงก่อนล็อก):

| ส่วน | ขนาดโดยประมาณ | หน้าที่ |
|---|---|---|
| Character + Inventory | 500 × 560 | ตัวละคร/paper doll ซ้าย, bag grid 4 คอลัมน์ขวา, แถบเมนูล่าง |
| หน้ารองทางขวา | 320 × 560 | พื้นที่สลับ Forge / หน้ารองอื่น; Forge มี Equipment/Gems ภายใน |
| ช่องว่างระหว่างหน้าต่าง | 12 | ขอบบนตรงกันและขยับเป็นกลุ่ม |
| Item slot | 56–64, hit target อย่างน้อย 44 | เห็นรูปและ grade ชัด; ข้อมูลเต็มใน detail/tooltip |

คู่หน้าต่างกินพื้นที่ประมาณ 772 × 520; หน้าหลักเปิดเดี่ยวได้เมื่อไม่มีหน้ารอง
Companion วางเหนือพื้นที่ต่อสู้; Battle วางลอยบนฉากและหลบ HUD โดยใช้ UI/state/controller เดียวกัน
เมื่อพื้นที่ขวาไม่พอ ให้ขยับทั้งกลุ่มเข้ามาก่อน; พื้นที่แคบมากใช้หน้ารองเป็นแผ่นซ้อนพร้อม Back
โดยเก็บหน้าหลักและแถบล่างไว้ ไม่เปลี่ยน Forge ให้กลายเป็นหน้าหลัก ไม่ย่อ text จนอ่านไม่ได้

```
Character + Inventory                หน้ารอง: Fusion Forge   ×
[Seraphine / Rapis]                   [Equipment] [Gems]
paper doll       กระเป๋า              [1]         [2]
 + ตัวละคร      [ ][ ][ ][ ]          [3]  [ ? ]  [4]
 10 slots       [ ][ ][ ][ ]          [5]         [6]
                [ ][ ][ ][ ]          preview / warning
รายละเอียดสิ่งที่เลือก                [Auto Fill] [Clear]
[Forge] [Socket Gem] [Storage]        [     Fuse 6/6     ]
           Companion หรือ Battle — combat เดินต่อ
```

- ทั้ง Companion toolbar และ Battle HUD เปิด **Character + Inventory** ชุดเดียวกัน
- กดเมนูด้านล่าง → เปลี่ยนหน้ารองทางขวา; ไม่สร้างหน้าต่างหลายอันซ้อนทับสะสม
- กดปุ่มที่ active ซ้ำหรือ X ของหน้ารอง → ปิดเฉพาะหน้ารอง; หน้าหลักยังคงอยู่
- ตัวละครที่ inspect, bag scroll/filter, selected item และ draft แต่ละหน้ารองอยู่ใน state แยกกัน
  สลับหน้ารอง/Companion/Battle แล้วกลับมาต้องไม่ล้างของที่เลือก; revalidate เมื่อข้อมูลจริงเปลี่ยน
- **Equipment / Gems เป็นแท็บภายใน Forge**; เปลี่ยนเฉพาะรายการกระเป๋าที่เกี่ยวข้องและเนื้อหา Forge
  ส่วนตัวละคร/paper doll และแถบเมนูล่างยังคงอยู่
- เลือกของตามหน้ารองที่ active: ไม่มีหน้ารอง = ดูรายละเอียด, Forge = เพิ่มวัตถุดิบ,
  Socket Gem = เลือกอุปกรณ์เป้าหมาย; ในโหมด Fuse คลิกช่องวัตถุดิบเพื่อนำออก
- ลาก title bar ของหน้าหลักย้ายทั้งกลุ่ม; หน้ารองยึดตำแหน่งด้านขวา ไม่ลากแยกอิสระใน MVP
  จำตำแหน่ง/UI scale แยกตามบริบท Companion/Battle และ clamp เมื่อเปลี่ยนจอ
- เปิดหน้าต่างไม่ pause sim และไม่สลับกลับหน้าเมนูเต็มจอ
- Windows layer 10, confirmation/result popup 20, tooltip 30, HUD 40 ตามโครงเดิม
- Tooltip ไม่รับ raycast; confirmation รับ click และไม่ปล่อยต่อไปยัง item ที่อยู่ด้านหลัง
- ปิดด้วย X/Esc ตามลำดับ popup → หน้ารอง → หน้าหลัก; ปิดหน้าหลักจะซ่อนหน้ารองด้วย
  คลิกพื้นว่างทั่วไปไม่ปิดหน้าหลัก/หน้ารอง
  confirmation มี blocker เฉพาะบริเวณคู่หน้าต่าง ไม่ยึดทั้ง desktop
- Floating นี้คือ panel ใน Unity overlay; ไม่ถือว่าได้ native OS click-through/always-on-top
  มาอัตโนมัติ ต้องตรวจ integration ของ companion host ต่างหากเมื่อทดสอบ Windows

## 3. Equipment flow

1. เลือก grade → แสดงของใน bag; ชิ้นแรกกำหนด grade ให้ชุดที่กำลังเลือก
2. เลือกของต่างชิ้นครบ 6; ชนิด/slot/ระดับอุปกรณ์ปนกันได้ แต่ grade ต้องเท่ากัน
3. กลาง Forge เป็นผลลัพธ์ `?` พร้อมชื่อ grade ถัดไป ไม่แสดงชนิดอุปกรณ์ที่ยังไม่ได้สุ่ม
4. Preview ต้องบอก: ระดับอุปกรณ์ที่เป็นไปได้ + โอกาส, โอกาสสำเร็จ,
   Gil ที่ได้เมื่อสำเร็จ/ล้มเหลว, ของทั้ง 6 ถูกใช้; การผสมไม่เสีย Gil
5. ถ้ามี gem ฝังอยู่ ให้ป้ายเตือนพร้อมจำนวน gem และชื่ออุปกรณ์; กด Fuse แล้วต้องยืนยันว่า gem หาย
6. ชุดปกติผสมหนึ่งครั้งต่อคลิก; ชุดที่เสี่ยงล้มเหลวต้องมี confirmation ที่บอกผลเสียชัด
7. Pending → กันกดซ้ำ/เปลี่ยนชุด; success/failure แสดงของ/Gil จริงใน Forge พร้อมเลือกชุดถัดไป
   animation 0.35–0.6 วินาที ข้ามได้; ไม่ทำให้การยืนยันเกมขึ้นกับ animation

**Auto Fill = เติมให้ครบหนึ่งชุด ไม่ใช่ auto-fuse ต่อเนื่อง**

- เริ่มจาก grade/filter ปัจจุบันและเก็บของที่ผู้ใช้เลือกเองแล้ว
- ไม่เลือกของที่สวมอยู่, unique, bound, หรือที่มี gem; ไม่หยิบจาก stash อัตโนมัติ
- ไม่สร้างระบบ lock item ถาวรใหม่ใน MVP; หากมี favorite/lock อยู่จริงค่อยผูกเข้า policy
- รายการ sort คงที่, ไม่ shuffle ทุกครั้งมี loot; แสดงจำนวนที่เลือกอยู่ในกระเป๋าด้วย
- ถ้าของสะอาดไม่ครบ แสดงจำนวนที่ขาด ไม่เลือก gemmed gear มาเติมเอง
- ถ้าเลือกชิ้นที่ใส่ gem เองอยู่แล้ว Auto Fill เก็บชิ้นนั้นไว้พร้อม warning

## 4. Gem Fusion flow

- อ่านจาก material ledger (`gem_set` ไม่ว่าง) ไม่ใช่สมมติว่า gem ทุกเม็ดเป็น bag instance
- ช่องกระเป๋าเป็น stack เช่น Strife ×12; กดแต่ละครั้งเพิ่มหนึ่งเม็ดใน 6 ช่องและแสดง selected/held
- เลือก 6 เม็ด grade เดียวกัน, ผสมชนิดกันได้, ชนิดเดียวกันซ้ำได้ตามจำนวนที่ถือ
- ผลที่ถูกกติกา: gem grade+1 จำนวน 1 เม็ดแน่นอน; **สุ่มชุด** ไม่ inherit ชุดที่ใส่เยอะที่สุด
- ไม่มีค่า Gil หรือ Gil reward จาก `FuseGems` ปัจจุบัน; ไม่แสดงแถบ Gil จาก Equipment ค้างมา
- ใส่คำว่า "สุ่มชนิด" ติดกับผลลัพธ์; รายละเอียดโอกาสเปิดดูได้จาก `gem_rules.sets[].weight`
  catalog ปัจจุบัน: 8 ชุด weight 3 + 3 ชุด Arcana weight 1, รวม 27;
  Arcana **แต่ละชุด** มี weight 1/3 ของชุดปกติ ไม่ได้หมายถึงโอกาส Arcana รวม 1/3
- UI format สัดส่วนจากข้อมูลที่ sim ส่ง; ไม่เลือกผลหรือ roll เอง
- จำกัดเพดานจาก `ceiling_step`; grade ที่ผสมต่อไม่ได้ยังตรวจของได้พร้อมข้อความอธิบาย
- Auto Fill หนึ่งชุดภายใน grade เท่านั้น, ไม่เลือกชุด Arcana หายากให้อัตโนมัติเป็นค่าเริ่มต้น
  (policy เสนอ; ผู้ใช้เลือกเองได้เสมอ) ไม่ตั้ง budget/batch automation ใน MVP

## 5. Socket Gem / Tooltip — งานต่อจาก Fuse

- เมนูล่าง Socket Gem หรือ Bag/paper doll → item detail → **Socket Gem** เปิดหน้ารองใส่ gem พร้อม target
  ใช้พื้นที่ขวาเดียวกับ Forge; ไม่เปลี่ยนตัวละคร+กระเป๋าหรือเปิดหน้ารองซ้อนเพิ่ม
- header = อุปกรณ์ + เจ้าของ (ถ้ามี); ช่อง gem แยกจากช่องการ์ดอย่างชัดเจน
- ช่องที่เลือกได้คือช่องที่มีของอยู่หรือช่องว่างถัดไป; ช่องถัดจากนั้น disabled พร้อมเหตุผล
- รายการ gem ใช้ `held[].fits/refusal`; ไม่ filter gem ต่าง grade ออกเอง เพราะ sim ยอมให้ใส่ได้
- preview คือบรรทัดเก่าที่ roll จริง เทียบกับช่วง min/max ของ gem ใหม่ ไม่อ้างว่าเป็นค่าแน่นอน
- ทับช่องเดิมต้องยืนยันชื่อ gem เก่า/ใหม่และแจ้งว่าอันเก่าหาย; ไม่มี Unsocket
- ใส่ช่องว่างให้แสดงผลเรื่อง market ก่อนยืนยัน: ชิ้นที่มี gem ขาย market ไม่ได้
  ไม่ใช้คำว่า bound ถาวรและไม่เสนอว่าถอดคืนได้
- tooltip: authored_lines → socketed gem_lines → ช่อง gem ว่าง → สถานะ market
- `market_clean` เป็นเงื่อนไขหนึ่งของตลาด ไม่ใช่คำอนุญาตว่า item ทุกชิ้นที่ clean ขายได้แน่นอน

## 6. สิ่งที่ตรวจพบจาก implementation จริง

| จุด | สถานะที่ 70d428d1 | งานก่อนส่ง UI ใช้จริง |
|---|---|---|
| UiWindow | ยัง full-bleed; HudBar ปิดหน้าต่างอื่นเมื่อเปิดอันใหม่ | เพิ่ม CharacterInventoryHost + หน้ารองขวาหนึ่งช่อง ใช้ใน Companion/Battle; ไม่เปลี่ยนทุกหน้าจอ |
| Fusion UI | พบ query/event/ServerClient.FuseRoll; ยังไม่มี dedicated FusionForge class | สร้างหน้าใหม่และแชร์ visual slots/selection กับ Gem |
| ItemView C# | ยังไม่มี gem_slots, gem_lines, gems, market_clean | เพิ่ม read model/adapter + decode test; JsonUtility ไม่รองรับ nested arrays ตรงๆ |
| fuse_rules / fuse_preview | มี inputs/ceiling, ระดับ output, payout, server binding | **ยังขาด success chance และ failure payout**; ต้องเพิ่มจาก sim ไม่เขียนสูตรซ้ำใน C# |
| Gear result | query ไม่ส่ง FuseOutcome | ต้องรับ receipt ที่ชัดหรือจับ before/after ใน command เดียวก่อน tick; ห้ามเดาของใหม่จาก grade สูงสุด |
| gem_plan | รองรับ target bag/equipped, held fits/refusal, ช่วง stat | อ่านตรงจาก query และ map refusal เป็น loc; ตรวจ selected socket order แยก |
| Gear slots | 10, Pants ถูกถอดแล้ว | ใช้ mapping ปัจจุบัน; ไม่คืนแถว Pants จากเอกสารเก่า |
| Card slots | item.socket_count vs legacy item.card_slots=0 | ใช้ socket_count บน item; gem_plan.card_slots เป็นค่าที่คำนวณถูกแล้วและใช้ได้ |

เสนอ additive host fields สำหรับ gear preview: `outcomes[]` แต่ละแถวมีระดับอุปกรณ์,
น้ำหนักโอกาสได้ระดับนั้น, โอกาสสำเร็จ, Gil success/failure — สร้างจาก helper เดียวกับ fusion จริง
รวมถึง structured result/attempt receipt ที่ผูกกับคำสั่งที่เพิ่งทำ ทั้งสองเป็นงาน contract ไม่เปลี่ยน balance/RNG
ถ้ายังไม่มีข้อมูลนี้ ห้ามเปิด flow ระดับเสี่ยงด้วยข้อความรับประกันสำเร็จ

## 7. State และ concurrency

- State: Selecting → Reviewing (เมื่อมี loss warning) → Pending → Result/Error
- Host state แยกเป็น inspected character / bag / active side page / per-page drafts / presentation context
  routing จากแถบล่างเปลี่ยนเฉพาะ active side page; draft ไม่ผูก lifetime กับ GameObject ของหน้ารอง
  Pending operation ต้องอยู่ได้แม้เปลี่ยนหน้ารองหรือสลับ Companion/Battle; ห้ามส่ง mutation ชนกัน
- Combat/loot ยังเดิน: จำ selected item ด้วย snapshot/raw identity + inventory position;
  slot index อย่างเดียวไม่พอเพราะ sell/equip/fuse ทำให้เลื่อน
- ตรวจ selection/quantity/session ซ้ำก่อน preview, ก่อนขอ server roll, และก่อน submit
  ถ้ามีการเปลี่ยนของระหว่างรอ ให้ยกเลิก/ขอเลือกใหม่ ไม่เลือกชิ้นอื่นแทนเงียบๆ
- Grade ที่ `needs_server_roll`: ใช้ `ServerClient.FuseRoll(key, attempt, ...)` ตาม binding;
  nonce ต้องเก็บแบบไม่สูญเสีย precision; error ไม่ fallback เป็น 0; timeout ไม่ขอ reroll ด้วย attempt ใหม่
- ตรวจ attempt เดิม/receipt ก่อน retry เพื่อไม่ consume ซ้ำ; ปิดหน้าต่างไม่ยกเลิกคำสั่งที่ส่งแล้ว
- animation เป็น presentation; update state จาก sim แม้ผู้ใช้ปิดหน้าต่างก่อนแสดงผล
- refresh แบบ diff/pooled controls เมื่อ bag/materials เปลี่ยน; ไม่สร้าง grid ใหม่ทุก tick
- ใช้หน้าต่าง/list shared แต่แยก Gear/Gem model + submit handler ไม่รวมกติกาคนละแบบไว้ใน widget เดียว

## 8. ลำดับทำและเกณฑ์จบ

1. Character+Inventory shell พร้อมแถบล่างและช่องหน้ารองขวา ใช้ทั้ง Companion/Battle;
   ทดลองสลับหน้ารอง/ปิดหน้ารอง โดยตัวละคร กระเป๋า scroll และ draft คงอยู่
2. Read models, preview/result contract, selection stale guards และ loc
3. Gem Fusion หนึ่งชุดครบวงจร (ledger, repeated ids, ceiling, actual outcome)
4. Equipment Fusion ครบวงจร (server pending/refusal, success/failure, gem-loss warning)
5. Socket Gem + tooltip/market state + entry จาก bag/paper doll
6. จูนภาพ: icon, border/grade, shadow, spacing, effect ที่กรอบผลลัพธ์ หลัง flow ผ่าน

Acceptance: 6→1 จริง, duplicate index gear ถูกกันแต่ repeated gem id ใช้ได้ตามจำนวน,
ต่าง grade/ไม่ครบ/ถึงเพดานกดไม่ได้, gemmed item ไม่ Auto Fill, manual warning ครบ,
ไม่แสดงผลสำเร็จเมื่อ command เป็น no-op, ไม่ consume ซ้ำขณะ timeout,
combat เดินต่อทั้ง Companion/Battle, กดแถบล่างสลับเฉพาะหน้ารองขวา,
ปิดหน้ารองไม่ปิดตัวละคร+กระเป๋า, สลับโหมด/หน้ารองแล้ว draft คงอยู่และไม่ส่งซ้ำ,
scroll/selection ไม่เด้งเมื่อ loot เข้า, TH/EN อ่านได้ที่ 1366×768/1920×1080 และ DPI 100/150%

ทดสอบ data adapter จาก payload จริง, sim-host query contract เมื่อเพิ่ม field, Unity compile,
แล้ว play test flow แบบ local fixture ที่แยกจากเซฟผู้ใช้ + online server-roll กรณีที่ต้องใช้จริง
ตรวจ localization ตาม guard ที่ repository ใช้; ถ้ายังอ้าง Godot legacy ให้ระบุว่าเป็น data mirror
compatibility ของ tooling ไม่ใช่งานพัฒนา Godot ต่อ และไม่รื้อ pipeline ใน UI patch โดยไม่จำเป็น

เรื่องยังเป็นข้อเสนอ: ขนาดหน้าต่าง, layout ช่องผสม, default Auto Fill ที่เว้น Arcana และจังหวะ animation
ยังไม่อนุมัติ automation, batch fuse, native OS window ใหม่ หรือ phone-shaped chrome
