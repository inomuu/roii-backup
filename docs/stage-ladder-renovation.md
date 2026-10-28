# บันไดด่าน (band / stage) — วิเคราะห์เพื่อรื้อใหม่ให้ไม่งง

2026-09-16 · สถานะ: **สร้างแล้ว sim 0.0.399 / catalog 0.0.148 (§5) — วัด 48h แล้ว (§5.1) กำลังจูน** · โจทย์จาก user: "band ยากไป · ไม่ต้องมี lv กำกับ band · ซอยย่อยกว่านี้
ก็ย่อยไป (WoG ผ่าน 40 band/วัน) · เราเคยซอยเป็น 5 band แล้วครั้งนึง อาจต้อง renovate ใหม่ไม่ให้งง"

## 1. ทำไมถึงงง — บันไดนี้ถูกรื้อมาแล้ว 3 รอบ และแต่ละรอบทิ้งคำศัพท์ไว้

| รุ่น | โครง | ที่เหลืออยู่วันนี้ |
|---|---|---|
| **G1** (Phase 0) | 20 band × 5 level/band · ชื่อแผนที่ 20 ชื่อ | `ZONE_NAME_b1..b20`, `ZonesPerRegion = 5` ใน WorldMap, "Lv 1–5" บนแผนที่, memory `launch-level-cap-cadence` ("Lv70 ≈ band 14") |
| **G2** 0.0.226 | ซอย ×3 → 60 sub-band "7-2" × 3 ห้อง (180 zone) · `hp_scale_bp` ต่อ sub-band · ชื่อทางการ **Zone → Area ("7-2") → Stage (ห้อง)** | loc `UI_AREA` / `UI_STAGE`, `UI_AREA_BAND = "Band {0}"`, ตัวเลข "7-2" ใน overlay |
| **G3** 0.0.274 | **มอนสเตอร์ = level** (`level_base[1..100]` × multiplier ต่อตัว) · band = level · **100 band × 3 ห้อง = 300 zone** · `hp_scale_bp` ลบ | `zone_b<N>_<r>` N=1..100 = mob level N, ห้อง 1-3 ยากเท่ากัน (ต่างแค่ boss_threshold 12/13/14) |
| 0.0.310 | 3 ห้อง = แผนที่ยาวเดินต่อเนื่อง fade ตอนจบ band | renderer/art ผูกกับ "band = 3 ห้อง" |

⇒ วันนี้มี **4 หน่วยซ้อนกัน**: region 20 (art/ชื่อ) · band 100 (=level) · area "7-2" (ตกค้าง) · room/stage 3 · และป้าย "Lv" โผล่ทั้งบนแผนที่ (คำนวณจาก region×5 = ผิดแล้ว) และใน AreaSelector

## 2. ข้อมูลจริง (sim-cli seed 42, catalog 0.0.134)

### 48 ชม. — เวลาที่ติดอยู่แต่ละ band

| band | ชม. | lv ตอนออก | wipe สะสม |
|---|---|---|---|
| b1-6 | <1 | 8 | 5 |
| b7 | 2.5 | 12 | 42 |
| b10 | 3.7 | 16 | 90 |
| b12 | 6.3 | 19 | 151 |
| b14 | 8.3 | 22 | 231 |
| **b15/16** | **~33** (วนถอย-ฟาร์ม-กลับ) | 29 | **454** |

- **wipe 454 ครั้งใน 2 วัน (~10/ชม.)** — การไปข้างหน้าถูกกั้นด้วยการตาย ไม่ใช่ด้วยจำนวน kill (ห้องหนึ่ง = ~13 kill + boss; ที่ 1,500 kill/ชม. ถ้าไม่ตายจะผ่านได้ 100 ห้อง/ชม.)
- `WIPE_RETREAT_LIMIT` 5 → ถอย 1 band ฟาร์ม 20 ห้อง → กลับ → ตายอีก 5 → วนใหม่ · ตัวเลข "rooms 3955" ในรายงานคือลูปนี้
- 240 ชม. (seed 42) รายวัน — band สูงสุด / lv / wipe สะสม:

| วัน | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|---|---|---|---|---|---|---|---|---|---|---|
| band | 16 | 16 | 19 | 25 | 27 | 29 | 32 | 40 | 41 | 41 |
| lv | 23 | 29 | 34 | 38 | 42 | 46 | 51 | 54 | 57 | 60 |
| wipe สะสม | 283 | 454 | 743 | 1,067 | 1,355 | 1,673 | 2,031 | 2,343 | 2,563 | 2,711 |

  **~3 band/วัน · ตาย ~270 ครั้ง/วัน** · ตัวละครนำ band 15-20 level ทุกช่วง เพราะ XP มาจากการวนฟาร์ม ไม่ใช่จากการทะลุ · วันที่ 1-2 ติดที่ b16 ทั้ง 2 วัน

### ต้นตอที่ 1 — ตาราง `level_base` มีหน้าผาที่ตกทอดจาก G1

ตารางถูก *derive* จาก catalog 20-band เก่า (`band→level = floor(band×5/3)`) จึงพกรอยต่อ band เก่ามาด้วย:

| level → | ×HP | ×ATK |
|---|---|---|
| 5→6 | 1.29 | 1.25 |
| **10→11** | **1.41** | **1.43** |
| **20→21** | **1.34** | **1.25** |
| 30→31 | 1.05 | 1.03 |
| 42→43 | 1.02 | 1.02 |
| 70→71 | 1.08 | 1.05 |

ต้นเกมกระโดดทีละ 25-40% ทุก 5 level แล้วแบนเหลือ 2%/level กลางเกม — ไม่มีใครตั้งใจให้เป็นแบบนี้ มันคือ artefact ของการ derive

### ต้นตอที่ 2 — ความยากโตเร็วกว่าตัวละคร และหวังให้ gear อุด

| lv | ATK ตัวละคร (authored) | mob HP | mob ATK | swing ต่อตัว (ไม่นับ gear) |
|---|---|---|---|---|
| 1 | 310 | 390 | 50 | 1 |
| 20 | 435 | 7,990 | 600 | 18 |
| 42 | 579 | 21,280 | 1,250 | 37 |
| 70 | 763 | 58,530 | 3,610 | 77 |

ตัวละคร ×2.5 · mob HP **×150** · ช่องว่างต้องมาจาก gear (s1 140 → s7 1,525 × grade ×10.6) — เป็นดีไซน์ที่ตั้งใจ (P2) แต่หมายความว่า **จังหวะดรอป/ผสมคือตัวกำหนดว่าผ่าน band ได้เร็วแค่ไหน** ไม่ใช่ level

### ต้นตอที่ 3 — รางวัลไม่โตตาม band แต่ถูก*หัก*เมื่อถอย

`slot_bp` 12/24 เท่ากันทุก band · grade tail โตช้า (0.35% → 0.5% Legendary) · แต่มี **loot gap penalty** (ตัวละครสูงกว่า mob >15 lv → ดรอป/Gil/ticket −20% แล้ว −5%/lv จนเหลือ 10%) ⇒ ผู้เล่นที่ถูกกำแพงกันไว้ **โดนลงโทษซ้ำ** ตอนถอยไปฟาร์ม

### ต้นตอที่ 4 — ป้าย "Lv" บน band สร้างความคาดหวังผิด

band 42 = mob Lv42 แต่คนที่ไปถึงคือ Lv63 → ผู้เล่นอ่านว่า "ฉันสูงกว่าด่านนี้ 20 level ทำไมยังตาย" — เพราะเลขที่โชว์คือ level มอน ไม่ใช่ความยากจริง (ซึ่งอยู่ที่ gear)

## 3. WoG เป็นเป้า

- ด่านเล็ก ผ่าน **~40 ด่าน/วัน** ต้นเกม · ไม่มีป้าย level · ความยากไต่ทีละน้อย
- แรงจูงใจไปข้างหน้า = **ดรอปดีขึ้นตามด่าน** ไม่ใช่ด่านต่ำโดนหัก
- ฟาร์ม = อยู่ที่ด่านสูงสุดที่ผ่านได้ (ไม่มี "ถอย" เป็นกลไก)

## 4. ตัดสินแล้ว (user 2026-09-16) — **ซอยเป็น 200-300 ด่าน + ห้องต่อกันแบบ heal 20%**

> "คิดว่าเราจะซอยย่อยขึ้นเป็น 200-300 ด่าน เปลี่ยนระบบห้องเล็กน้อยให้ heal 20% แทน full heal ออกแบบไว้ละไปทำ gem ก่อน"

หน่วยวันนี้: **1 ด่าน = 3 ห้อง (100 ด่าน)** — ผู้เล่นนับ "ด่าน" ไม่ใช่ห้อง · เป้าใหม่ 200-300 ด่านถึง content cap

### โครงที่ออกแบบไว้ (ยังไม่ทำ)

| เรื่อง | วันนี้ | ใหม่ |
|---|---|---|
| ด่าน | 100 (= mob level) × 3 ห้อง, ห้องยากเท่ากัน | **~250 ด่าน** (2.5 ด่านต่อ level เดิม) × 3 ห้อง — ห้องภายในด่านไต่เล็กน้อย |
| ความยาก/ด่าน | ×1.06 ต่อ band + หน้าผา ×1.4 ตกทอด | **×~1.025 ต่อด่านคงที่** (`level_base` regen เป็นเส้นเรียบ, index = ด่าน ไม่ใช่ level) — ยอดที่ด่านสุดท้ายของ launch = ค่าปัจจุบันที่ Lv70 |
| ระหว่างห้อง | **full heal** ทุกครั้งที่เปลี่ยนห้อง/ด่าน (`change_zone` → `hp = effective_max_hp`) | **heal 20% ของ max** เมื่อเดินเข้าห้องถัดไป**ในด่านเดียวกัน** · เข้าด่านใหม่ (ห้อง 1) = full heal เหมือนเดิม ⇒ ด่านหนึ่งคือ "การวิ่ง 3 ห้อง" ที่ต้องรอดทั้งชุด — ยากขึ้นแบบมีความหมาย ไม่ต้องพึ่งหน้าผา stat |
| ตาย | ถอยอัตโนมัติหลัง 5 wipe ฟาร์ม 20 ห้อง | **ไม่ถอย** — เริ่มห้อง 1 ของด่านเดิม · ผู้เล่นเลือกฟาร์มด่านก่อนหน้าเอง (farm toggle มีแล้ว) · sim-cli จำลอง: ตาย 3 ครั้งติด → ฟาร์มด่านก่อนหน้า |
| รางวัล | เท่ากันทุกด่าน + gap penalty | **`slot_bp` และ grade tail เป็นตารางตามด่าน** · **ไม่มี gap penalty** |
| ป้าย | "Lv 42" / "Band 42" / "7-2" | **"Stage 42" / "ด่าน 42"** ไม่มี level · ชื่อแผนที่ 20 region คงเดิม (region = ~12 ด่าน) |
| mob level | = ด่าน | ยังมีเป็นค่าภายในสำหรับ level_req/loot tier — map ด่าน→level = `ceil(ด่าน × 70/250)` ให้ tier ของของยังไต่ตามเดิม |

### เหตุผลของ heal 20%
ตอนนี้ 3 ห้อง = 3 fight แยกกันเพราะเลือดเต็มทุกห้อง ⇒ ความยากอยู่ที่ตัวเลข mob ล้วน · พอเลือดต่อเนื่อง ด่านกลายเป็น "ทนได้ครบ 3 ห้องไหม" — ให้ HP/regen/sustain มีค่า (ตรงกับ pool substat ที่มี HP/HP%/Dodge/DEF) และให้ตัวคูณต่อด่านเตี้ยลงได้โดยไม่รู้สึกง่าย · 20% เป็นค่าเริ่ม (จูนใน const เดียว)

### ที่ต้องแตะเมื่อทำ
- zone catalog: regen 250 ด่าน × 3 = 750 zone (script — `zone_b<N>_<r>` N ถึง 250; art/story ผูกที่ region ยังใช้ได้ถ้า region = ด่านช่วง) · `level_base` index เปลี่ยนเป็นด่าน
- `change_zone`: heal 20% เมื่อ `!is_instance_start`, full เมื่อ start · `handle_party_wipe`: ตัด retreat loop
- `drops.rs`: ตัด `loot_gap_mult_bp` · `GearRule.slot_bp` → ตาราง `max_stage` เหมือน `grade_odds`
- `band_of_zone` / `party_meta` / Unity WorldMap (`ZonesPerRegion`), AreaSelector, loc `UI_AREA_BAND`
- goldens ที่โหลด zone จริง (alpha6, diag_band4) ขยับ
- วัด 240h: ด่าน/วัน (เป้า ~30 ต้น → 10 ปลาย, ถึง cap วันที่ 8-14), wipe/วัน < 30, lv ตัวละคร − level ด่าน ±5

### ลำดับ
**Gem ก่อน** (ชั้นเพิ่ม ไม่แตะ pacing) → แล้วค่อยรื้อบันไดตามตารางนี้

## 5. ที่สร้างจริง (0.0.398, 2026-09-16)

| เรื่อง | ค่าที่ใช้ |
|---|---|
| จำนวนด่าน | **240** × 3 ห้อง = 720 zone (`zone_b1_1` … `zone_b240_3`) · 20 region × **12** ด่าน (`ZONES_PER_REGION` 12, Godot/Unity/arena_tables ตาม) · เพดาน launch = ด่าน **168** (= Lv70) |
| ความยาก | `stage_base[240]` แทน `level_base` (JSON key เก่าเป็น alias): geometric ระหว่างจุดยึด ด่าน 1 (= Lv1 เดิม) → 168 (= Lv70 เดิม) → 240 (= Lv100 เดิม) — HP ×1.0305/ด่าน ถึง 168 แล้ว ×1.0155 · ATK ×1.0259 / ×1.0117 · XP ต่อ kill ตามเดียวกัน ⇒ **กลางเกมง่ายกว่าเดิมมาก** (ด่าน 100 = Lv42 เดิม: HP 7,607 vs 21,280) เพราะตารางเดิมชันต้นแบนปลาย — ต้องวัดแล้วอาจเพิ่มจุดยึดกลาง |
| mob level | **ไม่มีแล้ว (0.0.399, user "ตัด mob level ออกเลย")**: `stage_base` มีคอลัมน์ `def` (= level×10 เดิม เป็นเส้นเรียบ) และ `toughness` · gear tier `bands.max_stage` (44/60/120/180/204/228) · grade odds `max_stage` (34/68/101/135/168) · sigil tier 60/120 · soft DEF = ด่าน × 25/12 · `MonsterTemplate.stage` / `Entity.mob_stage` (alias ของชื่อเก่าให้เซฟเก่าโหลดได้) · dungeon/story ระบุ `stage` เอง (Duskmourn 96-111) · bestiary ส่ง `stage_min/max` |
| ระหว่างห้อง | ห้อง 1 full heal · ห้อง 2-3 **+20% ของ max** (`ROOM_HOP_HEAL_BP`) คนที่ตายในห้องก่อนลุกที่ 20% |
| เดินข้ามห้อง | grace = ระยะ 52 cell ÷ (5 cell/s × (1 + move% ต่ำสุดในปาร์ตี้)) + **3 tick** (`march_grace_ticks`) · เข้าด่านใหม่ 15 tick (เดิน 1.2 s + 0.3 s) · renderer อ่าน `deploy_grace_ticks` แล้วก้าวให้ถึงก่อน 0.3 s (`ArenaView.MarchSettleSeconds`) — user "delay <0.5 วิ" · **regen ทุก 1 วิ ระหว่างเดิน** (`HP_REGEN_MARCH_INTERVAL_TICKS`) |
| ตาย | **0.0.403**: wipe ที่ด่าน N (ไม่ได้กดฟาร์มเอง) → ถอยฟาร์ม N−1 จนครบ 5 clear (`WIPE_FALLBACK_FARM_CLEARS`) → กลับ N อัตโนมัติ (แบบ WoG) |
| รางวัล | ไม่มี gap penalty (`loot_gap_mult_bp` ลบ, view key หาย) · `slot_bp_by_stage`: ≤24 12/24 · ≤72 14/28 · ≤120 16/32 · ≤168 18/36 · ≤240 20/40 |
| ป้าย | Unity: WorldMap "Stage a–b" (`UI_STAGE_RANGE`), AreaSelector "Stage N" (`UI_AREA_BAND`), overlay "Stage N cleared" (`UI_STAGE_CLEARED`) · ชื่อ region 20 ชื่อเดิม |
| เซฟเก่า | zone id เลขเดิม = ด่านเลขเดิม (ง่ายลง) ไม่ migrate — ยังไม่มีผู้เล่นจริง และ server ปฏิเสธเซฟต่างเวอร์ชันอยู่แล้ว |

### 5.1 วัด 48h seed 42 (`sim-cli --ticks 1728000`, ด่านสูงสุด / Lv / wipe สะสม)

| ชม. | band เดิม (0.0.397) | ด่านใหม่ ATK เต็ม | ATK −30% (0.0.398) | + XP −30% ไม่มี mob level (0.0.399) |
|---|---|---|---|---|
| 6 | — | 42 · 15 · 78 | 60 · 15 · 47 | 37 · 13 · 47 |
| 24 | b16 · 23 · 283 | 90 · 27 · 436 | 97 · 28 · 406 | 86 · 25 · 433 |
| 48 | b16 · 29 · 454 | 103 · 35 · 1,025 | 109 · 36 · 913 | 102 · 32 · 948 |

อ่าน: ไปไกลกว่าเดิม 3× แต่ตายถี่ 2× และชนกำแพงที่ด่าน ~100 (= Lv42 เดิม) เพราะ level ตัวละครตามด่านไม่ทัน (ด่าน 109 ≈ Lv46 เดิม ตัวละคร Lv36) — ATK −30% แก้ได้น้อย (+6 ด่าน) · ตารางกลางเกมเบากว่าเดิม 3× (geometric ระหว่างจุดยึด) เป็นสาเหตุที่ทะลุเร็ว · XP −30% ไม่เปลี่ยนภาพ (กำแพงเดิมที่ ~100, Lv ต่ำลง 3) ⇒ กำแพงคือ "พลังปาร์ตี้โตเป็นเส้นตรงต่อวัน แต่ mob โต geometric ต่อด่าน" — ความชันต้องตามอัตราโตของปาร์ตี้ ไม่ใช่ค่าคงที่ (§6.5)

ที่เคาะแทน (§6): 240 ด่าน · ตัด gap penalty ทั้งหมด · heal = +20% ของ max ไม่เกิน max · region = 12 ด่าน art strip เดิม (ผู้เล่นอยู่ใน region นานขึ้น ~2.4×)

### 5.2 รอบ 0.0.400-0.0.404 / catalog 0.0.149-0.0.156 (2026-09-17) — กำแพงที่ ~100-110 เป็นเชิงโครงสร้าง

ที่เปลี่ยนระหว่างรอบ: Regalia W-Engine (0.0.400) · break fix + display ÷10 (0.0.401) · ตาราง `stage_base` สร้างจาก **พลังที่วัดได้ต่อด่าน** (`tools/build_stage_ladder.py run.csv`: K = จำนวน hit ต่อ mob 1→5@30→8@cap, J = 12%→16% ของ HP สมาชิกต่อ hit ÷ ประชากรห้อง, XP 3.3%→1% ของ HP) · gear/gem drop odds ∝ K (ด่าน under-level จ่ายต่อ kill น้อยลง ต่อชั่วโมงเท่ากัน — ไม่มี penalty) · ถอยอัตโนมัติแบบ WoG ใน sim (0.0.403: wipe ที่ N → ฟาร์ม N−1 ครบ 5 clear → กลับ N, `WIPE_FALLBACK_FARM_CLEARS`) · next-tier preview ในไตรมาสสุดท้ายของ band (0.0.404) · sim-cli ไม่จำลองถอยเองแล้ว

**วัด 5 seed × 96h** (catalog 0.0.156, `--seed 1..5 --ticks 3456000 --sample 36000`) — seed เดียวหลอกได้ (รอบก่อน seed 42 ได้ 159 กับ 105 ต่างกันแค่ตารางขยับนิดเดียว):

| ชม. | ด่าน (5 seed) | median | ATK/สมาชิก |
|---|---|---|---|
| 24 | 38 40 41 43 43 | 41 | ~1,450 |
| 48 | 94 97 97 98 98 | 97 | — |
| 72 | 97 102 103 104 105 | 103 | — |
| 96 | 102 104 104 106 109 | 104 | 1,743-2,206 · wipe 411-431 · Lv45 |

ทดลองที่ **ไม่ช่วย** (วัดแล้วถอนออก): boss_threshold โตตามด่าน (ช้าลงทุกช่วง) · heal ระหว่างห้อง 50%/100% (ไป 100 เร็วขึ้นแต่ wipe เท่าเดิม — wipe มาจาก auto-advance ไม่ใช่ heal) · preview ทั้งครึ่งหลังของ band (0.0.405 ถอน: h96 median 105 vs 104) · ถอยอัตโนมัติลด wipe ลงครึ่ง (875 → 430) แต่ไม่ขยับด่าน

**สาเหตุ**: ATK ต่อสมาชิกอิ่มตัวที่ ~2,000 ราวด่าน 100 แล้วโตช้ามาก เพราะ level ให้ ATK แค่ `atk_per_level_milli` 6554 (Lv1→70 = +450) ขณะที่ gear ให้ ×10 ต่อ tier — พลังทั้งหมดคือ gear ที่ tier ถูกล็อกด้วยด่าน (band 61-120 = tier 3) ⇒ ยืนอยู่ในกำแพง 48 ชั่วโมงโดยที่การเก็บ level ไม่ช่วยอะไร **พบทีหลัง (catalog 0.0.157)**: break ทำงานเฉพาะธาตุที่เป้าอ่อน และบอส region 9 จาก 20 ตัว (b6/b8/b9/b13/b14/b15/b17/b18/b20) มี weakness แค่ Thunder หรือ Water ซึ่งปาร์ตี้ sim-cli (Neutral/Fire/Holy/Shadow) ตีไม่ได้ — กำแพง 97-105 คือ region b9 พอดี · แก้ชั่วคราว: มอนทุกตัวมี ≥2 weakness (ต่อท้าย list ตัวแรก = Sigil ไม่แตะ) ⇒ 5 seed: h24 40→46, h96 104→105 (กำแพงเดิม), break/96h +40% · **ไม่ใช่ดีไซน์สุดท้าย** — user: "ไม่ได้อยากให้บวกแค่พื้นฐาน ไม่งั้นมันจะไม่ได้ build party แต่ค่อยจัดการ" (memory `boss-weakness-party-build`)

**ตัดสินแล้ว (user 2026-09-17): รับสภาพนี้** — "เลทเกมจะช้าลงเป็นปกติ กระตุ้นให้ผู้เล่นหาซื้อไอเทมมาเสริมพลังตัวเอง" ⇒ ไม่เพิ่มน้ำหนัก level→ATK, ตาราง 0.0.156 คือของจริง; กำแพงปลาย band คือจุดที่ตลาด (Steam market / gem / Regalia) มีค่า อย่าเสนอปรับ level→ATK อีก

### 5.3 สู้ยาวขึ้น (0.0.407-0.0.409 / catalog 0.0.159-0.0.161, 2026-09-17)

user: "อยากให้การต่อสู้ยาวขึ้น ลดเดเมจทั้งสองฝั่ง 30 kill ต่อห้อง เอา penalty ตอนโดนรุมออก" / "WoG มอนจะไม่ attack แรงมาก แต่จะค่อยๆ ตีให้เลือดลดลง"

| รอบ | ที่เปลี่ยน | seed 1: ด่านวัน 1 / 4 / 7 | Lv วัน 7 |
|---|---|---|---|
| 0.0.157 base | — | 42 / 99 / 105 (วัน 14: 111) | 53 |
| + training 0.0.158 | 23 ช่อง | 83 / 108 / 113 (วัน 14: 132) | 55 |
| 0.0.407 | penalty รุมออก · ATK มอน = **6% แบน** ของ HP โมเดล ÷ pop (เดิม 12→20% จาก HP ที่วัด) · 30 kill/ห้อง | 108 / 169 / 183 | 51 |
| 0.0.408 | + HP มอน ×2 (K 2→10→16→20) + break ×2 | 103 / 169 / 182 | 57 |
| (ทดลอง) J 8% แบน | | 77 / 161 / 169 | 59 |
| (ทดลอง) J ramp 6→12→15 | | 82 / 140 / 162 | 61 |
| **0.0.409** | J ramp 6→12→15 + **XP ×0.5** | **37 / 109 / 114** | 48 |

บทเรียน: K (HP มอน) กำหนด "สู้นานแค่ไหน" แต่**ไม่**กำหนดว่าไปได้ไกลแค่ไหน — การขึ้นด่านถูกจำกัดด้วยการตายเท่านั้น (ห้อง 30 kill × 3 วิ = 100 ด่าน/วันถ้าไม่ตาย) ⇒ J ต้อง ramp; และ level = HP ที่ใช้ทน J ⇒ XP คือคันเร่งตัวจริง · ATK มอนคิดจาก HP **โมเดล** ไม่ใช่ HP ที่วัด (ปาร์ตี้ยืนด่านเดิมเก็บ level ทำให้ตารางพุ่ง ×2 ที่ 100→120) · กำแพง 97-120 ยังเป็น content (สกิล gothic) ไม่ใช่ตาราง — user: "แล้วค่อยคิดวิธี tune เพิ่ม"

### 5.4 harness ผิดมาทั้งวัน → จูนใหม่บนปาร์ตี้จริง (0.0.415-0.0.417 / catalog 0.0.165-0.0.174, 2026-09-17/18)

**ค้นพบ**: ทุกการวัดใน §5.1-5.3 ใช้ sim-cli ที่เริ่มด้วย `zone.unit` = swordsman_t1 + recruit class เก่า (bash/heal เป้าเดียว) — stat พื้นเท่า Seraphine แต่**ไม่มี AoE kit** พอเปลี่ยนเป็น genesis จริง (Seraphine: Iron Chains 7×7 + Steel Break + Aegis) + Exemplar จาก gacha (rapis/lucielle/nyx/aveline) ห้อง 20-30 ตัวละลาย: ถึงด่าน 240 ในวัน 3 บนตารางที่ class party ติด 110 · user: "ห้ามรันด้วยของเดิมแล้ว" ⇒ sim-cli field Exemplar ผ่าน `grant_exemplar_unit` เท่านั้น (memory `measure-with-the-real-party`)

| catalog | ที่เปลี่ยน | วัน 1 / 4 / 7 (ด่าน) | Lv วัน 7 |
|---|---|---|---|
| 0.0.165 (party slots) | ปาร์ตี้จริง ครั้งแรก | 132 / 240 / — | 41 (วัน 3) |
| ทดลอง HP ×3 | | 93 / 168 / 240 | 59 (วัน 6) |
| 0.0.168 | HP ×3 + ดาเมจมอน ×2 + XP ÷2 | 80 / 108 / 110 | 54 |
| 0.0.169 | + XP ÷2 อีก + auto_train ตาม gate | 72 / 109 / 109 | 44 |
| 0.0.170 | K ชันต้น 6→45@30 | 48 / 87 / 109 (วัน 14: 111, Lv56) | 45 |
| 0.0.171 | **K = ตี*ทั้งปาร์ตี้* จาก roster ที่วัด** (builder อ่านคอลัมน์ `party`) | 36 / 57 / 85 | 46 |
| 0.0.172 | + gothic (ด่าน 49-120) ATK/HP −20% | 36 / 84 / 85 (วัน 14: 86 — **deadlock** วัน 9) | 48 |

- **บทเรียน**: (1) ความยากช่วงต้นต้องตาม "จำนวนคน" ที่เพิ่ม ไม่ใช่ ATK ต่อคน — builder ใช้ Σ ATK ปาร์ตี้แล้ว (2) กำแพง 84-86 = region 8 (banshee wail −30% ASPD ทีม + maid heal ฝูง 15%/10 วิ) ไม่ใช่ตัวเลข — ยังไม่แก้ (เสนอ dirge 15→5%, wail −30→−15%) (3) **deadlock**: Seraphine คนเดียว vs บอส maid (heal ตัวเอง) ยืน 5 วัน → 0.0.416 `break_stalemate`: บอสอยู่ + มีคนตาย + ไม่มี kill 60 วิ ⇒ ที่เหลือล้ม (เข้า wipe path)
- ค้าง: วัดใหม่บน 0.0.172 + stalemate + crest; region 8 support skills

## 6. ที่เคาะไปแล้วตอนทำ (เดิม: ที่ยังต้องเคาะ)

1. จำนวนด่านที่แน่นอน (200 / 250 / 300) และกี่ด่านถึง launch cap
2. ตัด gap penalty ทั้งหมด — ยอมรับว่าบอทฟาร์มด่านต่ำได้ของด่านต่ำเต็มอัตรา (crowd-scale ยังกันอยู่)
3. heal 20% นับจาก max หรือ "เติมจนถึง 20% ถ้าต่ำกว่า" — ค่าเริ่ม: +20% ของ max, ไม่เกิน max
4. region 20 ชื่อ = ด่านละ ~12 ⇒ art strip ต่อ region ยังพอไหม

## 7. ประวัติที่ต้องล้างออกจากเอกสาร/memory เมื่อทำ

memory `band-split-x3` (G2, ล้าสมัยตั้งแต่ G3) · `launch-level-cap-cadence` "Lv70 ≈ band 14" (G1) · `band-pacing-confirmed-ok` (+15%/band ยุค 20 band) · `WorldMapWindow.ZonesPerRegion=5` + "Lv x–y" · `UI_AREA_BAND` · `overlay picker "7-2"` ใน Godot (frozen ไม่แตะ)


## 8. DPS bench (2026-09-19) — the standard party, one sim hour, ~4 s wall-clock

```
target/release/sim-cli --seed 1 --recruit rapis,lucielle,aveline,nyx \
    --bench 70:6:legendary:100 --regalia sig:60:0 --out /dev/null
```
`--bench level:tier:grade:stage` stands the whole party up at tick 0 (level held, best gear of
tier ≤ T at that grade with its substat lines, all five fielded), holds the stage (room 3 wraps to
room 1) and runs 36,000 ticks. Read `MEMBERS` (dmg/alive-h, ults, blocked) and the `BENCH` line
(party dmg/h, kills/h, bosses, wipes). Change ONE thing between runs. Reference numbers at 0.0.444
/ catalog 0.0.187 (`--stars N` = Resonance rank, `--regalia sig|r:level:stars`):

| preset | party dmg/h | kills/h | bosses | wipes |
|---|---|---|---|---|
| 70 / T6 / Legendary / stage 100 / no Regalia | 161M | 818 | 20 | 0 |
| … / R Regalia Lv60 | 207M | 1,097 | 28 | 0 |
| … / SSR Regalia Lv60 S0 | 288M | 1,500 | 39 | 0 |
| … / SSR Lv60 **S6** | 417M | 2,112 | 56 | 0 |
| … / SSR Lv60 S0 / stage 168 | 373M | 1,100 | 28 | 0 |
| … / SSR Lv60 S6 / stage 168 | 605M | 1,688 | 43 | 0 |
| 50 / T4 / Rare / stage 60 / SSR Lv30 | 104M | 1,378 | 37 | 0 |

Per member at 70/T6/Legendary/100/SSR S0: rapis 30% · aveline 23% · nyx 21% · seraphine 13% ·
lucielle 12% (stage 168: rapis 29 · aveline 25 · nyx 24 · seraphine 12 · lucielle 8).
SSR over R is +39% party damage; S6 over S0 is +45% at stage 100 and +62% at stage 168 (the
Resonance ladders, §8.1 — the earlier "+4%" was the Regalia passive's per-star step, since replaced).

### 8.1 Resonance ladders (S1-S6) — effects + "+X% DMG" add-ons (0.0.441-0.0.442, as shipped)

User rules: a rank adds an EFFECT; numbers only as +3 skill levels (S3 / S5) or as a "+X% DMG"
ADD-ON bucket on one skill (additive across ranks, applied once, apart from the skill's power —
never a multiplier on the skill, never a follow-up strike, which would need animation).

| | S1 | S2 | S3 | S4 | S5 | S6 |
|---|---|---|---|---|---|---|
| Seraphine | Iron Chains 2→4 targets, caught take +10% dmg 6 s, Chains +30% DMG | DEF +2%/hit while taunting (×10); Steel Break on ≥3 → cd −2 s | +3 | Aegis also +20% ATK to the party 30 s | +3 | Aegis burst ×2, room taunt 3 s, break refunds 100 gauge, Steel Break +50% DMG |
| Rapis | Fireball on ≥3 → cd −2 s, Fireball +25% DMG | a kill charges 30 gauge | +3 | kills detonate 3×3 50% ATK | +3 | Meteor leaves +20% dmg taken 10 s, Meteor +50% DMG |
| Lucielle | Lux Medica splashes 50%, Contagio +30% DMG | Communio reaches everyone at 15% | +3 | healed ally DEF +10% 8 s | +3 | Benedictio +20% ATK more and heals 30% max HP |
| Aveline | Mark 3→4, lasts ×2, Volley +30% DMG | Shadowbind bleeds 2× faster; an arrow on a capped mark charges 8 gauge | +3 | Volley +2 targets | +3 | Mark 10%/stack, Eclipse Rain +100% DMG |
| Nyx | Darkness ×1.5 duration, spreads to the 3×3, Veil Slash +40% DMG | Rend stacks ×3 | +3 | Exploit +15%/status | +3 | Requiem resets Veil Slash + Rend, +80% DMG |

Party bench 70/T6/Legendary/stage 168/SSR Lv60, dmg/h per star (M) — 0.0.442:

| | S0 | S1 | S2 | S3 | S4 | S5 | S6 | S6 gain |
|---|---|---|---|---|---|---|---|---|
| seraphine | 57 | 73 | 72 | 82 | 79 | 80 | 94 | +65% |
| rapis | 106 | 116 | 129 | 150 | 149 | 154 | 173 | +63% |
| aveline | 93 | 103 | 110 | 114 | 141 | 143 | 153 | +65% |
| nyx | 91 | 102 | 103 | 108 | 130 | 135 | 146 | +60% |
| lucielle | 32 | 35 | 36 | 37 | 38 | 38 | 38 | support |
| party | 379 | 429 | 450 | 490 | 537 | 551 | 605 | +59% |

Source of truth: `crates/sim-core/src/unit_rank.rs` (`ranks_of`); the UI reads `overlay_ranks`.

## 9. F2P vs whale, 7 days × 3 seeds (0.0.443, 2026-09-19)

Real party (Seraphine + rapis/lucielle/aveline/nyx pulled in order), same ladder. F2P = R Regalia
Lv1, S0. Whale = SSR Regalia Lv60 + S6 from the hour each unit is fielded (the ceiling, not a
realistic first week — Regalia levels and dupes take time).

| | d1 | d2 | d3 | d4 | d5 | d7 | wipes/7d |
|---|---|---|---|---|---|---|---|
| F2P s1/s2/s3 | 37/36/37 | 60/61/60 | 71/73/73 | 85/91/85 | 92/96/94 | 96/95/93 (Lv53) | 382/344/383 |
| Whale s1/s2/s3 | 175/174/173 | 220/217/222 | **240** ×3 | 240 | 240 | 240 (Lv51-52) | 132/161/166 |

- F2P: three seeds agree within ±3 stages every day; the wall is 91-100 (370 wipes across the
  three runs), the mid-game bumps are 31-50 and 61-80.
- Whale: the whole 240-stage ladder in **3 days**, then four days parked on 240 (uptime 35-38%
  for Nyx / Lucielle / Seraphine = dying to the last stage's enrage). Stage 175 on DAY ONE at Lv38
  — the launch cap (168) must be enforced by the stage gate, not by difficulty, or a paying
  player empties the content in a weekend. The gate exists for the client (`SelectBand` ≤
  reached + 1); sim-cli auto-advances past it.
- Shares: F2P Rapis 31 / Aveline 25 / Nyx 20 / Seraphine 12 / Lucielle 10; whale Aveline 25 /
  Nyx 25 / Rapis 25 / Seraphine 18 / Lucielle 5.

## 10. Loot curve + Gil per kill (0.0.455 / catalog 0.0.197, 2026-09-21)

User's rule after 0.0.454 thinned the whole table: **"โอกาสดรอปเกียร์ควรเท่าเดิมทุกเลเวล แต่โอกาสโรลไป tier
สูงจะเพิ่มขึ้นทีละนิดๆ … T1-T2 ควรจะตกเป็นหลักเสมอ"**. Two tables in `loot_rules.gear`:

- `slot_bp_by_stage` — how often a kill drops a gear piece at all (0.0.450 values, ≈ 2.4-3.2 % of
  kills, flat across the ladder; boss ×2).
- `tier_mix_by_stage` (`GearRule::mix_at`) — how that piece splits across tiers (bp per tier, by
  stage band). The bands only widen the tail; T1-T2 keep ≥ 75 % everywhere.

Built tables, ppm of a FIELD kill (boss = ×2), read from `tier_mix_print::print_ppm`:

| stage | T1 | T2 | T3 | T4 | T5 | T6 | T7 | gear/kill |
|---|---|---|---|---|---|---|---|---|
| 30 | 19191 | 4793 | – | – | – | – | – | 2.4 % |
| 60 | 17468 | 8018 | 1290 | – | – | – | – | 2.7 % |
| 100 | 15337 | 9178 | 2676 | 524 | – | – | – | 2.8 % |
| 120 | 13979 | 9232 | 3356 | 1248 | 122 | – | – | 2.8 % |
| 150 | 13998 | 8539 | 3359 | 1399 | 699 | 2 | – | 2.8 % |
| 180 | 14498 | 8843 | 3479 | 1449 | 724 | 2 | – | 2.9 % |
| 210 | 15496 | 9142 | 3718 | 1858 | 774 | 5 | – | 3.1 % |
| 232-240 | 15993 | 9276 | 3837 | 1918 | 958 | 9 | 3 | 3.2 % |

Per ~15,000 field kills a day (five-unit party, 24 h): T3 ~50 · T4 ~20-29 · T5 ~11-14 · **T6 ≈ 1/week**
(232+) · **T7 ≈ 1/month** (232+). Fusion raises grade, never tier (`fusion::fuse` averages the inputs' tier) — a T6 or T7 only ever DROPS.
Under-level farming (stage 60 at Lv70) yields ~40 T1-T2 an hour and nothing higher — more kills,
never more tier. The `LOOT gear/h by tier` line on `sim-cli --bench` reads it live.

**Gil per kill** (`StageBase.gil`): `10 × (stage/30)^1.2`, min 3, × the monster's `xp_mult_bp` (boss
≈ ×3) — stage 60 23 · 96 40 · 120 53 · 180 86 · 240 121 a field kill. Fusion salvage — the only Gil
faucet before this — returns 20 % for T3+ inputs (`FUSE_GIL_RETURN_BP`, was 100 %) and 50 % for
T1-T2 (`FUSE_GIL_RETURN_EARLY_BP`): a flat 20 % took day one from Lv25 / stage 46 to Lv18 / stage 21,
because a stage 1-30 kill pays 3-12 Gil and salvage is the whole early purse. A base of 20 read 1.0-1.37 M
Gil/day on the 7-day (0.0.450: 0.7-0.8 M); 10 lands on the old line (see the 0.0.455 changelog).
