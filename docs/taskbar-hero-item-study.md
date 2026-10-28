# TBH: Task Bar Hero — ศึกษาระบบไอเทม (v2, 2026-09-14)

> **สถานะ: เอกสารศึกษา ไม่ใช่ดีไซน์** — user สั่ง "ลอกศึกษาระบบ TBH ให้ถ่องแท้ ก่อนดีไซน์"
> (2026-09-14) รอบ v1 (13 ก.ย.) ตัวเลข synthesis สองแหล่งขัดกันและสรุปผิด รอบนี้แก้แล้ว §3
>
> **ความน่าเชื่อถือ** — เว็บ TBH ส่วนใหญ่เป็น SEO farm ที่ generate เนื้อหาซ้ำกัน ตัวเลขในนี้ติดป้าย:
> **[H]** = ยืนยันตรงกัน ≥2 แหล่งอิสระ หรือมาจากหน้า patch note · **[M]** = แหล่งเดียวที่มีโครงข้อมูลจริง
> (ตาราง ไม่ใช่ prose) · **[L]** = อ้างลอย/ขัดกัน/มาจากฟอรั่ม
> หลายแหล่งตอบ 403 (mobalytics, taskbarhero.wiki, exitlag, Steam guide ติด rate limit)

---

## 0. เกมคืออะไร (กรอบที่ต้องจำไว้ตอนอ่านตัวเลข)

**F2P** บน Steam, ฮีโร่ 3 ตัวสู้เองในแถบ taskbar, เปิดค้างไว้ทั้งวัน **[H]**
รายได้ = **DLC**: Supporter Pack, **คลาสขาย 2 ตัว (Hunter, Slayer)**, soundtrack,
**Stash Pack**, **Tradeship Pack**, Full Package bundle **[H]**
→ เขาขาย **ช่องเก็บของ + ช่องส่งของขึ้นตลาด + ตัวละคร** ไม่ได้ขาย power ตรง ๆ
(ของเราวางไว้ใกล้กันแล้ว: rare-slot purchase / Exemplar / Regalia)

**หัวใจของ loop คือ Steam Community Market** — ของ tradable ทุกชิ้นแปลงเป็นเงิน Steam ได้จริง
นั่นคือเหตุผลที่ระบบไอเทมทั้งหมดถูกออกแบบรอบคำถาม "ชิ้นนี้ขายได้ไหม" **[H]**

---

## 1. โครงไอเทม

| เรื่อง | TBH |
|---|---|
| ขนาด catalog | 5,760 gear (20 ชนิด: อาวุธ 6 / off-hand 6 / เกราะ 4 / accessory 4) × 396 level progression **[M]** |
| category | **Gear / Accessory / Material** — ผสมข้าม category ไม่ได้เลย **[H]** |
| grade | **10 ขั้น** Common → Uncommon → Rare → Legendary → Immortal → Arcana → Beyond → Celestial → Divine → Cosmic, แต่ละขั้นมี **สีประจำ** **[H]** |
| identity ของชิ้น | **ชนิด × grade × item level** — Long Sword มีทุก grade ทุกช่วงเลเวล ⇒ ชิ้น "ดิบ" เป็นของ **fungible** โดยธรรมชาติ **[H]** |
| stat บนชิ้น | base stat ไม่กี่บรรทัด อ่านจบทันที (Long Sword Common Lv1 = `Attack Damage +1`, `Attack Speed 1.00/s`) — **ไม่มี roll ตอนดรอป** ความแรงมาจาก grade × level **[H]** |

### "กี่ชั้น" — มีสองแกน อย่าสับสน (ตรวจซ้ำ 2026-09-14)

| แกน | จำนวนขั้น | มาจากไหน | ความเชื่อถือ |
|---|---|---|---|
| **grade / rarity** (บันไดที่ผู้เล่นเห็นเป็นสี) | **10** Common→Cosmic | **fusion ขยับแกนนี้** + chest/craft/offering | **[H]** — taskbarhero.org (ตาราง gold/XP/ช่อง) + tbhindex (ไล่ชื่อครบ 10) ตรงกัน |
| **item-level tier** (พลังตามคอนเทนต์) | **T1…T7** (บางแหล่งบอกถึง T9) — T1 ≈ Lv1-10, T3 ≈ Lv20-40, T5 ≈ Lv40-50, **T7 ≈ Lv65-80** | **level ของวัตถุดิบ** ไม่ใช่ fusion | **[M]** — probonk calculator (สร้างจากข้อมูลเกม) + tbh.city ("T1-T9") ขัดกันที่ขั้นบนสุด |

- ⚠️ tbhindex มีคอลัมน์ชื่อ "Tier 1-10" ด้วย แต่มัน **แค่ index ของ grade** (1:1 กับชื่อ) ไม่ใช่แกน level
- สัญญาณที่มีค่า: **calculator ของ probonk เปิดให้เลือก grade ต้นทางแค่ 5 ขั้น** (Common→Immortal)
  ⇒ พื้นที่เล่นจริงกว้างราว **5-6 ขั้น** ส่วนขั้น 6-10 เป็นของตลาด/ถ้วยรางวัล **[M]**
  ⇒ บทเรียน: **author 10 แต่เล่นจริง 5-7** — จำนวนขั้นไม่ใช่ปัญหา แหล่งที่มาต่อขั้นคือปัญหา

### 1.6 ตัวเลขจริงจากตารางดาบ (user แปะจาก mobalytics 2026-09-14) **[H — ข้อมูลจากไกด์ที่ไล่ทุกชิ้น]**

ATK ฐานของ "ดาบ" ทุก grade × ทุก level (ชื่อชิ้นไล่ตาม level: Long Sword Lv1 … Dimensional Sword Lv80)

| grade | Lv1 | 5 | 10 | 15 | 20 | 30 | 40 | 50 | 65 | 80 |
|---|---|---|---|---|---|---|---|---|---|---|
| Common | 1 | 2 | 3 | 5 | 9 | 20 | 44 | 94 | 156 | 239 |
| Uncommon | 2 | 3 | 4 | 7 | 14 | 30 | 61 | 119 | 190 | 296 |
| Rare | 3 | 5 | 8 | 12 | 20 | 44 | 85 | 152 | 225 | 360 |
| Legendary | 5 | 8 | 12 | 18 | 30 | 64 | 118 | 193 | 267 | 438 |
| Immortal | 8 | 12 | 18 | 28 | 44 | 93 | 163 | 245 | 369 | 594 |
| Arcana | — | — | — | 42 | 66 | 138 | 239 | 356 | 529 | 840 |
| Beyond | — | — | — | — | — | 179 | 314 | 472 | 710 | 1,143 |
| Celestial | — | — | — | — | — | — | — | — | 959 | 1,544 |
| Divine | — | — | — | — | — | — | — | — | 1,271 | 2,045 |
| Cosmic | — | — | — | — | — | — | — | — | 1,652 | 2,659 |

**กฎที่ถอดออกมาได้ — นี่คือส่วนที่มีค่าที่สุดของเอกสารทั้งฉบับ**

1. **grade ขยับพลังแค่ ×1.2-1.6 ต่อขั้น** (สูง ๆ ราว **×1.33**) ⇒ ทั้งบันได 10 ขั้นรวมกัน =
   **×11 เท่านั้น** ที่ level เดียวกัน (Lv80: 239 → 2,659)
2. **level ขยับพลังมากกว่ามาก**: Common Lv1→Lv80 = **×239**; ต้นเกม ~**×2.2 ต่อ 10 เลเวล**
   แล้วแบนลงเหลือ ~×1.5 ต่อ 15 เลเวลตอนปลาย
3. ⇒ **1 grade ≈ 5-10 item level** — grade เป็น *ของแต้ม* ไม่ใช่ *ทางข้ามคอนเทนต์*:
   **Common Lv80 (239) ชนะ Immortal Lv20 (44) ขาดลอย** เงินซื้อ grade ได้ แต่ซื้อ level ไม่ได้
4. **grade สูงไม่มีอยู่ที่ level ต่ำ**: Arcana เริ่มที่ Lv15, Beyond ที่ Lv30, **Celestial/Divine/Cosmic
   มีแต่ที่ Lv65+** ⇒ บันไดคลี่ตามคอนเทนต์ ไม่ใช่คลี่ตามเวลาที่ฟาร์ม
   (นับชิ้นยืนยัน: Common 20 · Uncommon-Immortal 38/grade · Arcana 32 · Beyond 28 ·
   Celestial 24 · Divine 20 · Cosmic 16 — **รวมดาบ 292 ชิ้น, 20 ขั้น level**)
5. **พลัง ×11 แต่ราคา ×35,000** (alchemy gold Common 10 → Cosmic 355,607 = ×3/ขั้น)
   ⇒ เศรษฐกิจตั้งราคาที่ **ความหายาก ไม่ใช่ความแรง** — เป็นการแยกที่ตั้งใจและฉลาด
6. **จำนวนบรรทัด stat เพิ่มตาม grade**: Common 2 (ATK + ASPD%) · Uncommon/Rare 3 · Legendary/Immortal 4 ·
   Arcana+ 5 — และบรรทัดพวกนี้ **author ต่อ (ชิ้น × grade) ไม่ได้สุ่มต่อ instance**
   (Uncommon Long Sword ได้ `Attack Damage +20%` เหมือนกันทุกใบ) ⇒ **นี่คือสิ่งที่ทำให้มัน fungible จริง**
7. **pool ของ stat แคบมาก ~7 ชนิด**: ATK flat, ATK%, ASPD%, Crit DMG%, Crit Chance%, CDR%, HP per hit
   (ของเรามี 24-26 mod — กว้างกว่า 3 เท่า)
8. ASPD บนอาวุธเป็น % เล็กและโตช้า: Common 1-3% → Cosmic 6.3% (ตรงกับ `atk_speed_bp` ที่เราเพิ่งทำ)

> ⚠️ ในไกด์มีบรรทัดที่ดูเป็นความคลาดของการถอดข้อมูล (เช่น Rare Rapier เขียน `Attack Damage +22.3%`
> แต่ Uncommon Rapier เขียน `Attack Damage +2` ตรงตำแหน่งเดียวกัน) — ตัวเลข ATK ฐานในตารางข้างบน
> สอดคล้องกันทั้งกริด ใช้อ้างอิงได้; ส่วน "บรรทัดที่เท่าไร" อย่ายึดเป็นเป๊ะ

### ตาราง grade (ค่าที่ทุกอย่างอิงจากมัน) **[M — probonk + taskbarhero.org ตรงกัน]**

| grade | Alchemy gold | Cube XP | ช่อง Deco/Engr/Insc |
|---|---|---|---|
| Common | 10 | 2 | 0/0/0 |
| Uncommon | 30 | 6 | 1/0/0 |
| Rare | 90 | 18 | 1/1/0 |
| Legendary | 270 | 54 | 2/2/0 |
| Immortal | 810 | 162 | 2/2/1 |
| Arcana | 2,592 | 518 | 3/2/1 |
| Beyond | 8,294 | 1,658 | 3/2/2 |
| Celestial | 29,029 | 5,803 | 3/2/2 |
| Divine | 101,602 | 20,311 | 3/2/2 |
| Cosmic | 355,607 | 71,089 | 3/2/2 |

ทั้ง gold และ XP **×3 ต่อขั้น** (ปัดเป็นเลขสวย) — ตัวเลขตัวเดียวคุมทั้งราคาย่อยและ XP
"Divine/Cosmic แทบเป็นไปไม่ได้แบบ legit" **[M]**

---

## 2. Cube — ศูนย์กลางเดียวของทุกอย่าง (จุดที่ผมประเมินต่ำไปในรอบแรก)

Cube ไม่ใช่ "เมนูคราฟ" มันคือ **ตัวละครที่สองของผู้เล่น**: มีเลเวลของตัวเอง เลเวลมันปลดล็อกกลไก
และ **XP ของมันมาจากการย่อยของทิ้ง** ⇒ ของทุกชิ้นมีปลายทางเสมอ **[H]**

| หน้าที่ | ปลดล็อกที่ Cube Lv | ค่าใช้ทำ | ทำอะไร |
|---|---|---|---|
| **Synthesis** | 1 (default) | 0 → 10,000g ตาม tier | 9 ชิ้น grade เดียวกัน → 1 ชิ้น grade สูงขึ้น |
| **Alchemy** | 1 (default) | 10g | ย่อย gear → gold + **Cube XP** |
| **Crafting** | 5 | 100g | สร้าง gear จาก material (มี rarity odds) |
| **Decoration** | 8 | 300g | ใส่ crystal = stat ที่ **เลือกได้** |
| **Extraction** | 10 | 1,000g | **ถอด** modifier ออกจากชิ้น (เสีย gold, crystal หายถาวร) |
| **Engraving** | 15 | 1,000g | material → affix **สุ่ม 2 stat** |
| **Offering** | 20 | 3,000g | จ่ายเหรียญ → รางวัลสุ่ม (gacha sink) |
| **Inscription** | 25 | 10,000g | scroll → utility stat |

**[M — probonk/games.gg ตรงกัน; v1.00.22 ลด Offering requirement 20→10 [H ข้อนี้จาก patch note]]**

- **Cube max Lv100 = 234,760,055 XP สะสม** (Lv50 = 8.8M, Lv75 = 62M) **[M]**
- XP ต่อชิ้นตามตาราง grade ข้างบน → ไต่ Cube ต้องย่อยของ **grade สูง** ไม่ใช่จำนวนมาก
  (Cosmic 1 ชิ้น = Common 35,544 ชิ้น)
- **Cube Lv10 = ประตูเข้าตลาด** (ดู §5) ⇒ เลเวล Cube เป็นทั้ง progression และ anti-bot **[H]**

---

## 3. Synthesis — ตัวเลขจริง (แก้จาก v1)

**กติกา**: **9 ชิ้น grade เดียวกัน + category เดียวกัน** → 1 ชิ้น grade สูงขึ้น **[H]**

สองข้อที่ v1 ไม่มีและสำคัญมาก:
1. **ผลลัพธ์เป็นของ "สุ่มในกลุ่มนั้น"** ไม่ใช่ชิ้นที่คุณเลือก — 9 × Common → *1 Uncommon แบบสุ่ม*
   ใน category เดียวกัน **[M — probonk calculator: "9 x COMMON→1 x UNCOMMON"]**
2. **item level ของผลลัพธ์มาจาก level เฉลี่ยของวัตถุดิบ** และออกมาเป็น **ช่วง** ที่อาจ **ต่ำกว่า**
   ของที่ใส่ไป: material T1 Lv0+ → ผล Lv1-1 · T1 Lv15+ → ผล **Lv10-20** · T7 Lv75+ → ผล **Lv65-80**
   **[M]** (นี่คือที่มาของคำเตือน "อัพ Synthesis level ให้ถึง 10 ก่อน Act 2 Hard ไม่งั้น output level ตก")

### อัตราสำเร็จ (ชุดที่สอดคล้องกันเอง — เชื่อชุดนี้) **[M — tbhwiki.net เป็นแหล่งเดียวที่ให้ตารางครบและตัวเลขไม่ขัดกันเอง]**

| ใส่ 9 × | ผลปกติ | great success (+2 ขั้น) |
|---|---|---|
| Common | **95%** → Uncommon | 4.76% → Rare |
| Uncommon | **96%** → Rare | 3.85% → Legendary |
| Rare | **98%** → Legendary | 2.44% → Immortal |
| Legendary | **99%** → Immortal | 0.99% → Arcana |
| **Immortal** | **50%** → Arcana / **50% ได้ Immortal คืน** | 0.25% → Beyond |
| Arcana | **33%** → Beyond / 67% คืน | 0.17% → Celestial |
| Beyond | **23%** → Celestial / 77% คืน | 0.08% → Divine |
| Celestial | **17%** → Divine / 83% คืน | 0.02% → Cosmic |
| Divine | **9.09%** → Cosmic / 91% คืน | — |

**อ่านตารางนี้ให้ออก**: ต่ำกว่า Immortal แทบไม่พลาด — มันเป็น **เครื่องบีบอัดของ ไม่ใช่การพนัน**
กำแพงจริงอยู่ที่ **Immortal ขึ้นไป** และ "fail" ไม่ใช่เสียของ 9 ชิ้น แต่ **คืนมา 1 ชิ้น grade เดิม**
⇒ ราคาคาดหวังของ Arcana 1 ชิ้น = **18 Immortal** (9 ÷ 50%), Beyond 1 ชิ้น ≈ 27 Arcana ≈ 486 Immortal

> ตัวเลขชุด "Common→Rare 60% / Uncommon→Legendary 40% / Rare→Legendary 20% / Legendary→Immortal 5%"
> ที่ v1 ยกมาจาก taskbarhero.org **น่าจะเป็นคนละความหมาย** (ดูเหมือนโอกาสไต่ข้ามหลายขั้นแบบต่อเนื่อง
> หรือเป็นข้อมูลผิด) — ไม่สอดคล้องกับ "ต่ำกว่า Immortal ขึ้นเสมอ" ที่ทุกแหล่งพูดตรงกัน **[L]**

---

## 4. ชั้นทำเพิ่ม — Decoration / Engraving / Inscription

| ชั้น | grade ที่มีช่อง | ใส่อะไร | ลักษณะ |
|---|---|---|---|
| **Decoration** (crystal) | Rare+ | crystal (30 ชนิด 8 tier) | **stat ที่เลือกได้** pool ต่างกันตามชนิดของ (อาวุธ: ATK/ASPD/crit/element% · เกราะ: armor/resist/HP/dodge · accessory: AoE/leech/regen/move) **[H]** |
| **Engraving** | Immortal+ | monster material | **affix สุ่ม 2 stat** RNG สูง **[H]** |
| **Inscription** | Arcana+ | scroll | utility stat สุ่ม **[M]** |
| **Extraction** | — | จ่าย gold | ถอด modifier ออก (Tier 1 = 100/200/400 → Tier 10 = 50k/100k/200k); **crystal หายถาวร** ต้องถอดของออกจากตัวก่อน **[M]** |

**คำแนะนำที่ชุมชนพูดตรงกัน** (สะท้อนดีไซน์ที่ตั้งใจ):
**ใส่ Engraving (สุ่ม) ก่อน แล้วค่อยใส่ Decoration (เลือกได้)** — เพราะจะได้รู้ก่อนว่าได้อะไรมา
แล้วใช้ชั้นที่ควบคุมได้ปิดช่องว่าง **[H]** · และ **อย่าใส่อะไรเลยก่อน Lv20** ของเปลี่ยนเร็วเกิน **[H]**

> ชั้นพวกนี้ = **เหตุผลที่ชิ้นหนึ่งเลิกเป็นของ fungible** และเป็นตัวแบ่ง "ของในตลาด" กับ "ของของฉัน"

---

## 5. ตลาด — และข้อจำกัดที่เกิดจากวิกฤต (ส่วนที่มีค่าที่สุดของเอกสารนี้)

**เส้นทาง**: ของในเกม → **Trade Ship** → Steam inventory → Steam Community Market **[H]**

ข้อจำกัดหลังวิกฤต (มิ.ย. 2026) **[H — ยืนยันจาก patch note + สรุปหลายแหล่ง]**:
- **ขายได้เฉพาะ Legendary ขึ้นไป** — Common / Uncommon / Rare **ถูกถอดออกจากตลาดทั้งหมด**
- **ของที่เลเวลเป็นหมุดหมายขายไม่ได้**: Lv. 25, 35, 45, 55, 60, 70, 75, 85, 90
  (= ล็อก "ของช่วงที่คนซื้อเพื่อข้ามด่าน" ไว้ไม่ให้ซื้อขาม)
- **"Type B" variant ขายไม่ได้** (ตัวแปรของชิ้นเดียวกันที่ถูกตีตรา)
- **เข้าตลาดไม่ได้เลยจนกว่า Cube Lv10** — กันบัญชีใหม่/บอทถล่มตลาดโดยตรง
- **Trade Ship เหลือ 4 ช่อง คูลดาวน์ 8 ชม./ช่อง** (ก่อนหน้านี้มากกว่านั้น) ⇒ ปั๊มขายถูกจำกัดด้วย throughput
- ของใหม่/บัญชีใหม่ **resale lock 7 วัน**
- **ของที่ใส่ Decoration/Engraving/Inscription แล้ว list ไม่ได้** — ต้องถอดก่อน (และ crystal หาย)
  ⇒ **"ของที่ทำเพิ่มแล้ว" ผูกกับผู้เล่นโดยปริยาย** (โมเดลเดียวกับ base-fix ขายได้ / instance bound ของเรา) **[M]**
- ราคาจริงเริ่มมีความหมายที่ **Immortal+**; Legendary ≈ $0.15; "Arcana = main market tier" **[L]**
- **material กลายเป็นกระดูกสันหลังของเศรษฐกิจ** เพราะ **เลี่ยงกฎ Legendary-only ได้** และผู้เล่นปลายเกม
  เผามันทิ้งตลอด **[M]** — บทเรียนตรง: ปิดทางของ gear แล้วอุปสงค์จะไหลไปหาสิ่งที่ยัง fungible

### ไทม์ไลน์วิกฤต (พ.ค.–มิ.ย. 2026) **[H — จากหน้า patch note]**

| วันที่ | เกิดอะไร |
|---|---|
| 30 พ.ค. | คนล้น **30,000 → พีค 140,000+ CCU**; **Steam item server แตก**, เปิด chest/ใช้ Cube ล้มเหลว; ปิด Cube/Alchemy สำหรับผู้เล่นใหม่ |
| 31 พ.ค. | **หั่นอัตราดรอป chest จาก "หลายใบต่อนาที" เหลือ "~1 ใบทุก 5-10 นาที"**; gear หยุดดรอปที่บางช่วงเลเวล; ชุมชนเรียกร้องให้ rollback |
| 31 พ.ค. | **แบนถาวร 411 บัญชี** ฐาน "หมุนเวียนของ tier สูงที่ผลิตจากการโกง"; รัดกุมตลาด + เพิ่ม pop-up เตือน |
| 1 มิ.ย. | **รีเซ็ตไอเทมครั้งใหญ่**, **ลบกลุ่มอุปกรณ์หลายกลุ่ม**, ย้ายสิทธิ์ Trade Ship ไป Cube Lv10, เปิดตลาดใหม่ |
| 8 มิ.ย. | **ปิดการลงขายบน Steam Market** — "บอทสร้างทราฟฟิกมากผิดปกติ" (ยังยกเลิกรายการ/ซื้อได้) |
| 15 มิ.ย. | v1.00.13 แก้ chest spam ตอน reconnect, คราฟ gear เก่า, **การคำนวณ gold ของ Alchemy** |

**อ่านให้ออก**: เกมนี้ **ไม่ได้เจ๊งเพราะ gameplay** — มันเจ๊งเพราะ **ของที่โกงมาเข้าตลาดได้** และ
**server ของ Steam ไม่ใช่ของเขา** ทางแก้ทั้งหมดของเขาเป็น **reactive**: หั่นดรอป, ปิดตลาด, รีเซ็ตของ,
แบนเป็นชุด → รีวิว Mixed ทั้งที่ CCU สูงมาก (บันทึกไว้ใน memory `competitive-landscape-idle-steam`)

**นี่คือจุดที่เราต่างโดยโครงสร้าง**: replay-verify ของเราปฏิเสธ save ที่ไม่ reproduce → ของที่โกงมา
**ไม่มีทางเกิด** ไม่ต้องมาแบนย้อนหลังหรือรีเซ็ตของทั้งเซิร์ฟ (memory `economy-antibot-and-quota`,
`input-authority-security`)

---

## 6. Loot flow & inventory

- **loot มาเป็น chest/box** ไม่ใช่ดรอปตรง: Normal Monster Box / Stage Boss Box **[H]**
- **อัตราได้ chest**: ต้นเกม 16% → 8%, ~3% ตอน Act 1-8 แล้วลดต่อ **[M]**;
  drop rate ต่อ grade: Legendary 0.18-0.45%, Immortal 0.04-0.16% **[M]**
- **เปิด chest มี cooldown** (common 300 วิ / blue & boss 600 วิ), rune "auto-open" ซื้อได้;
  **offline ได้ 0 chest** → gear มาจากการเปิดเกมค้างเท่านั้น **[M]** (ตรงกับ pillar idle-open ของเรา)
- **Rune Tree = ศูนย์กลางการโตของทั้งเกม**: ช่องฮีโร่, auto-chest, **ช่องกระเป๋า**, offline reward,
  ช่องสกิล แตกจากที่นี่ทั้งหมด **[H]** · **stash ขยายได้สูงสุด 4 หน้า** ผ่าน rune **[M]**
- **กระเป๋าเต็มเป็นปัญหาจริงของเกม** — บทความ/ไกด์แนะนำ **script ภายนอก** (ขาย White/Green อัตโนมัติ
  เมื่อเหลือ <5 ช่อง, auto-alchemy background) ⇒ ตัวเกมมี auto-feature ไม่พอ ผู้เล่นไปพึ่ง macro **[H]**
  **และเขาขาย Stash Pack เป็น DLC** — ความเจ็บนี้คือสินค้า
- pet/rune ให้ chest drop% + EXP% (Bat Pet: +10% common chest, +15% EXP) **[M]**

---

## 7. ชม / บ่น

**ชม**: "ฮีโร่สู้เอง เหลือบดูเป็นครั้งคราว ตัวเลขขึ้น" · grade = สี อ่านออกทันที ·
synthesis ทำให้ของทุกชิ้นมีที่ใช้ · ขายของได้เป็นเงิน Steam จริง **[H]**

**บ่น**: กระเป๋าเต็มตอน AFK · ของหายใน Cube / ผลไปโผล่ mailbox ต้องกด refresh ·
ตลาดถูกปิดบ่อย · ต้นเกมงงถ้าไม่รู้เรื่อง Rune Tree · content แค่ 3 act · **ของโกงในตลาด** **[H]**

---

## 8. บทเรียน 6 ข้อที่เอาไปใช้ได้จริง (ยังไม่ใช่ดีไซน์ของเรา)

1. **ของดิบต้อง fungible โดยโครงสร้าง** — "ชนิด × grade × level, ไม่มี roll" คือสิ่งที่ทำให้ทุกอย่าง
   หลังจากนั้นเป็นไปได้: กองรวมกัน, ผสม, ขาย, ตีราคา ระบบ roll-at-drop ของเราอยู่ตรงข้ามกับข้อนี้
2. **ตัวเลขตัวเดียว (grade) คุมทั้ง gold, XP, จำนวนช่อง** — ผู้เล่นเรียนรู้ครั้งเดียวใช้ได้ทั้งเกม
3. **"เครื่องมือ" ควรมีเลเวลของตัวเอง และกินของทิ้งเป็นอาหาร** (Cube) — แก้ปัญหาของไร้ค่า + ให้ progression
   ที่ไม่ผูกกับเลเวลตัวละคร + เป็นด่าน anti-bot ที่แนบมาฟรี (ตลาดเปิดที่ Cube Lv10)
4. **การบีบอัดของ (9→1) ต้องไม่ใช่การพนันในช่วงต้น** — 95-99% จนถึงกำแพงเดียวที่ตั้งใจ
   และ "fail" ที่คืนของ 1 ชิ้นทำให้ไม่มีใครเสียหมด
5. **ของที่ "ทำเพิ่ม" แล้วต้องออกจากตลาด** — เขาบังคับด้วยการห้าม list, เราบังคับด้วย bound ถาวร
   (แข็งกว่าและไม่มีช่อง extraction ให้ล้าง)
6. **ตลาดต้องมีคันบังคับหลายตัวตั้งแต่วันแรก** ไม่ใช่ไปหาตอนไฟไหม้: gate ด้วย progression (Cube Lv10),
   จำกัด throughput (4 ช่อง/8 ชม.), resale lock, ห้ามขาย grade ต่ำ, ห้ามขายของช่วงเลเวลหมุดหมาย
   — และ **อุปสงค์จะไหลไปหาสิ่งที่ยัง fungible** (material) เตรียมรับไว้ก่อน

---

## 9. คำถามที่การศึกษานี้เปิดขึ้น (ต้องตอบก่อนดีไซน์)

1. **ของดิบของเราจะเลิก roll ★/line ตอนดรอปไหม** — ถ้าไม่เลิก ข้อ 1 ใช้ไม่ได้ทั้งหมดและ
   "ผสม" จะกลายเป็นการทำลาย substat ที่สุ่มมา; ถ้าเลิก จะกระทบ `gear_roll`, server-roll drops,
   enchant/star stone, และตลาดที่ออกแบบให้ค้นหา substat (memory `economy-market-and-trade`)
2. **ผสม 6 ชิ้น → ขยับอะไร** tier (`_s3`→`_s4`) หรือ rarity — และ **ผลลัพธ์สุ่มในกลุ่ม** แบบ TBH
   หรือเลือกชิ้นได้ (ถ้าสุ่ม ของที่ไม่มีใครใส่จะกลายเป็นวัตถุดิบทันที)
3. **item level ของผลลัพธ์** มาจากค่าเฉลี่ยวัตถุดิบแบบเขา หรือ fix ตาม tier
4. **fail เริ่มที่ขั้นไหน** และคืน 1 ชิ้นแบบเขาไหม
5. **เรามี "Cube" ไหม** — คือรวม Enchant/Refine/Craft/ผสม/ย่อย เข้าเครื่องเดียวที่มีเลเวลของตัวเอง
   และกินของทิ้งเป็น XP หรือคงหลายหน้าจอแยกกันอย่างตอนนี้
6. **ถ้าทำ Cube แล้ว 3 ชั้นของเรา (★/refine/card) ยุบเป็นกี่ชั้น** และชั้นไหนเป็น "เลือกได้"
   ชั้นไหน "สุ่ม" (TBH แยกหน้าที่ชัด: Decoration เลือกได้ / Engraving สุ่ม)
7. **คันบังคับตลาด Gil ของเรา** จะยืมข้อ 6 มากี่ตัว และ gate ด้วยอะไร (เรายังไม่มี Cube level)

---

## แหล่ง (v2 เพิ่ม)

- https://taskbarhero.org/en/items/ (ตาราง grade: gold/XP/ช่อง)
- https://taskbarhero.org/en/updates/ (patch note + ไทม์ไลน์วิกฤต)
- https://tbhwiki.net/en/synthesis/ (ตารางอัตราสำเร็จครบ 9 ขั้น)
- https://probonk.com/tbh-task-bar-hero/cube (8 หน้าที่ + Cube level/XP/max)
- https://probonk.com/tbh-task-bar-hero/synthesis (output = สุ่ม + item level จากค่าเฉลี่ย)
- https://taskbarhero.blog/en/crystal-guide/ · https://games.gg/tbh-task-bar-hero/ (Cube unlock levels, ลำดับ Engraving→Decoration)
- https://taskbarhero.app/blog/synthesis-troubleshooting · /updates
- https://store.steampowered.com/app/3678970/ + bundle/73382 + app/4427400 (DLC: stash pack, tradeship, คลาสขาย)
- v1 (13 ก.ย.): tbhindex.com/wiki/gear · probonk.com/…/decoration · lagofast/commonsensegamer/annazplays ·
  steamcommunity discussions 569289424252470123 (synthesis rates), 571539955565767716 (lost all 9)

**403 / อ่านไม่ได้ในรอบนี้**: mobalytics.gg (ทุกหน้า), taskbarhero.wiki, exitlag.com,
tbh.city (calculator ไม่มีข้อมูล populate), Steam Community guide 3734611647 (rate limit)
