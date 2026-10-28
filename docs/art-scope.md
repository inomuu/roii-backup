# Art Scope — Pixel Art Asset Brief (สำหรับ Studio)

เอกสารสรุปงาน pixel art ทั้งหมดของเกม (idle ARPG, แนว Ragnarok Online) สำหรับใช้
ปรึกษา/ขอใบเสนอราคา studio. สเปคเทคนิคอ้างอิงจาก `docs/SPRITE_SPEC.md` (pipeline จริง).

> **จุดประหยัดงานสำคัญ:** sprite ทุกตัว **วาดหันขวาทางเดียว** — engine flip ซ้ายให้เอง
> ⇒ **ไม่ต้องวาดหลายทิศทาง** (ถ้า studio quote แบบ "ต่อ direction" ให้คิดทิศเดียว).

---

## 0. สเปคเทคนิค (บอก studio ให้ตรงทุกข้อ)

| หัวข้อ | ค่า |
|---|---|
| Gameplay sprite frame 
| Floor tile | 32×32 px (1 tile = 1 grid cell), ส่งเป็น TileSet ไม่ใช่ map สำเร็จ |
| Facing | วาดหันข้างเท่านั้น 
| สไตล์ | Pixel-art ขอบคม 
| Background | โปร่งใส (transparent) |


**กติกาตัวละคร (สำคัญต่อระบบหมวก cosmetic):**
- หัวอยู่ **ตำแหน่งเดิมทุกเฟรม** (bob ได้นิดเดียว) และ **ตำแหน่งหัวเดียวกันทุก class**
- **หัวโล่ง ไม่มี headgear baked** (ผมได้ แต่เว้นบนหัวให้หมวกวางทับได้)
- **Archer:** ธนูต้องไม่บังโซนหัว/หมวก
- Boss: frame ใหญ่กว่า 100×100 ได้ (ระบุขนาดต่อตัว เช่น 150×150 / 200×200)

---

## 1. Player Characters — 15 ตัว
5 × class-1 (Swordsman, Mage, Archer, Thief, Acolyte) + 10 × class-2 (สาขาละ 2 ต่อ class-1)
Class 2-1 (Swordmaster, Elementalist, Ranger, Assassin, Bishop)
ท่าต่อตัว ประมาณ 7 animations : idle · basic attack · use skill · walk · die · special attack ×2 (อาจจะ+/-)

Swordman Animation
- Basic Attack
- Bash
- Ignite Break (fire base AoE)
- Use skill (taunt)

Mage
- Basic Attack
- Cast 
- ไม่มี special attack แต่มี particle effect ของสกิลแทน 
- fire bolt, frost bolt, lightning bolt, fireball, 


Archer (ranged physical — เหมือน Mage: pose ยิงร่วม, ต่างที่ particle)
- Basic Attack (ยิงธนู)
- Use skill (ยิง) — Twin Strafe (ยิงหนัก) / Arrow Shower (AoE volley) ใช้ pose ยิงร่วม, ต่างที่ particle
- Buff cast (Sharp Focus) — pose เพ่ง/ชูธนู
- passive ไม่ต้องมี anim: Keen Eye, Hawk Sight

Thief (melee — ส่วนใหญ่ pose แทงร่วม + particle)
- Basic Attack (แทงมีด) — หมายเหตุ: assassin (2-1) ตี 2 มือ = เล่น pose แทงเดิมซ้ำ (ไม่ต้องวาดเพิ่ม)
- Use skill (แทง/ฟัน) — Venom Edge (พิษ) / Sand Strike (ดิน+blind) ใช้ pose แทงร่วม, ต่างที่ particle
- Shadow Strike — teleport + backstab (particle teleport)
- Hiding — pose หมอบ/หาย (vanish) ← ต้องมี pose เฉพาะ
- passive ไม่ต้องมี anim: Double Attack, Nimble Mastery, Steal

Acolyte (melee mace + caster)
- Basic Attack (ฟาดกระบอง mace)
- Cast — Heal / Holy Light / Blessing / Agility Up ใช้ pose ร่ายร่วม, ต่างที่ particle (heal/holy/buff aura)
- passive/aura ไม่ต้องมี anim: Guardian Aura

> **หลักการ animation (สำคัญต่อ quote):**
> - **Spell / caster** (Mage, Acolyte) → **cast animation เดียว ครอบทุก spell**; ความต่างทั้งหมด
>   อยู่ที่ **particle effect** (§8). เพิ่มสกิลเวทใหม่ภายหลัง = **เพิ่มแค่ particle ไม่ต้องวาด character เพิ่ม**.
> - **Ranged** (Archer) → คล้ายกัน: pose ยิงเดียว + particle (volley/AoE ต่างที่ effect).
> - **Melee** (Swordsman, Thief, Acolyte-basic) → วาด swing แยกเฉพาะสกิลที่ **ท่าทางต่างจริง**
>   (เช่น Bash, Ignite Break); สกิลแทงที่ท่าเหมือนกันใช้ pose ร่วม + particle.
> - **Passive / aura** → ไม่ต้องมี character animation เลย.

---

## 2. Monster Sprites — 20 ตัว (ไม่รวม recolor)
**ท่าต่อตัว (4):** idle · walk · attack · die

| | ต่อตัว | ×20 |
|---|---|---|
| Animations | 4 | **80** |
| Frames (~idle4/walk6/atk6/die6) | ~22 | **~440** |

> **Recolor = palette swap (ถูกมาก)** ใช้ขยาย roster: monster base 20 ตัว → variant สีหลายเท่า
> ตามแต่ละ band. ควรคุย studio ว่า recolor คิดราคาแยก/เหมา.

---

## 3. Boss Monsters — 3 ตัว
**ท่าต่อตัว (5):** idle · walk · attack · die · **special attack ×1** · (frame ใหญ่)

| | ต่อตัว | ×3 |
|---|---|---|
| Animations | 5 | **15** |
| Frames (~30) | ~30 | **~90** |

---

## 4. Terrain — 5 Biome
Tile **20×20 px**, **4–8 variant/biome** (กันลายซ้ำ) + edge/transition optional.
ธีมให้เลือก 5 จาก: Grassland · Forest · Cavern · Ruins · Volcano · Abyss.

| | จำนวน |
|---|---|
| Floor tiles | ~6 × 5 biome = **~30 tiles** (+ edge/transition optional) |

> ⚠️ **ต้องเลือกว่าเอา 5 biome ไหน** (spec มี 6 ธีม — ตัด 1 ออก).

---

## 5. UI Pixel Art
แตกตามหน้าจอจริงในเกม (Arena, Sidebar, Stats, Skill tree, Shop, Paths of Soul,
Character creation, Modals, HUD).

### 5.1 Frames / Panels
- HUD strip บน (title + zone bar)
- Sidebar (class selector + equipment paper-doll + inventory grid)
- Stats panel (6 stats + XP)
- Skill-tree panel (node + เส้นเชื่อม)
- Shop panel (2 คอลัมน์)
- Paths-of-Soul panel (passive web: node + edge)
- Character-creation screen (+ radar/hexagon chart)
- Advancement screen (จอ advance class — planned, "grand")
- Modal/dialog frame (refine confirm ฯลฯ)
- Tooltip frame (item/skill hover)
- DPS meter panel
- Companion widget frame (16:9 camera)

### 5.2 Bars
HP · SP · XP (base) · Job XP · Cast bar · Monster HP bar (เล็ก) · Boss HP bar (ใหญ่) · Zone-progress bar

### 5.3 Buttons
ปุ่ม generic (normal/hover/pressed/disabled) · close (X) · tab · นำทาง (←/→) · +/− allocate stat · reset · advance

### 5.4 Icons — Stat / Element / Status / Currency / Rarity
- **Stat ×6:** STR · AGI · VIT · INT · DEX · LUK
- **Element ×9:** Fire · Earth · Water · Wind · Holy · Shadow · Poison · Thunder · Neutral
- **Status/Buff ×~12:** poison · deadly poison · stun · blind · hiding · blessing · agility up · arcane ward · bloodthirst · frozen · slow · (+เผื่อ)
- **Currency ×2–3:** Gil · Soul (+ Steam-market token ถ้าทำ)
- **Rarity frame ×4:** Common · Uncommon · Rare · Legendary (กรอบ/สีขอบ)
- **Equip-slot placeholder ×11:** Head×3 · Armor · Weapon · Shield · Garment · Shoes · Accessory×2 · **Weapon-Left (off-hand)**

### 5.5 Misc
inventory/equip slot frame · paths-of-soul node (allocated/locked) + edge · radar chart (creation) · drop/loot popup · refine success/fail banner · cursor · (มี pixel font สำหรับ damage number แล้ว)

---

## 6. Item Icons
ไอคอนในช่อง inventory/shop/tooltip (น่าจะ 32×32 หรือ 48×48 — เคาะกับ studio)

- **Equipment** — weapon 6 type (dagger · 1H sword · 2H sword · bow · rod · mace) + armor 9 ชิ้น
  (head×3 · body · shield · garment · shoes · accessory×2) **× 3 gear set (s1/s2/s3)** ≈ **~45 ไอคอน**
- **Consumables ×~13** — Max Potion · Field Ration · Rejuvenating · Phoenix Tail · Monster Lure ·
  Accelerator · (+ uniques ~7)
- **Materials / Refine ore ×~4** — Primocon ore · Aegirilium · (+)
- **Cards ×N** — slime cards (3) + การ์ดที่จะเพิ่มภายหลัง

| | ประมาณ |
|---|---|
| Item icons | **~65–80** |

---

## 7. Skill Icons
1 ไอคอนต่อ skill (ใช้ในแถบ skill / skill tree / tooltip)

| | จำนวน |
|---|---|
| ปัจจุบันในเกม | **~35** |
| ครบ roster 15 class (เป้าหมาย) | **~80–120** |

> แนะนำเริ่มจาก ~35 ของจริงตอนนี้ก่อน แล้วเติมตาม class ที่ทยอย build.

---

## 8. Effect Particles / VFX
sprite-sheet เอฟเฟกต์ (เล่นครั้งเดียวแล้วหาย) — ผูกกับ element + skill mechanic ของเกม

- **Hit / impact:** hit spark · **crit burst** · block/miss puff
- **Element ×9 (projectile + impact):** fire · water · earth · wind · holy · shadow · poison · thunder · neutral
  (มีลูกธนู/บอลธาตุ + เอฟเฟกต์ตอนกระทบ)
- **Skill VFX:** AoE blast (3×3/5×5) · **ground field** (poison cloud / quicksand / meteor / glacial storm) ·
  teleport blink (Dark Ambush / Shadow Strike) · multi-hit slash (Sonic Blow) · knockback dust
- **Status onset:** stun (ดาวหมุน) · blind · poison bubble · frozen shatter · blessing/buff aura
- **Support / world:** heal sparkle · level-up burst · death poof · loot drop sparkle · soul/gil pickup

| | ประมาณ |
|---|---|
| Effect particles | **~25–35 เอฟเฟกต์** |

---

## 9. Showcase Full-Art — 15 ตัว
ภาพใหญ่ high-detail (big pixel art) ของทุก class 15 ตัว — สำหรับ profile / หน้าโชว์ /
splash. **ไม่เกี่ยว gameplay.** ขยับน้อย (idle เบา ๆ เช่นหายใจ 2–4 เฟรม optional).

| | จำนวน |
|---|---|
| Showcase illustration | **15 ภาพ** (+ idle loop เบา ๆ optional) |

---

## 📊 สรุปยอดรวม

| หมวด | จำนวนชิ้น | ~animations | ~frames |
|---|---|---|---|
| Player characters | 15 | 105 | ~660 |
| Monsters | 20 | 80 | ~440 |
| Bosses | 3 | 15 | ~90 |
| **รวม character/monster** | **38** | **200** | **~1,190** |
| Floor tiles | 5 biome | — | ~30 tiles |
| UI (frames/bars/buttons/icons/misc) | — | — | itemized §5 |
| Item icons | ~65–80 | — | — |
| Skill icons | ~35 (full ~100) | — | — |
| Effect particles | ~25–35 | — | — |
| Showcase full-art | 15 | — | 15 ภาพใหญ่ |

\*frame เป็นค่าประมาณ (idle4 / walk6 / attack6 / skill6 / die6 / special8) — **studio เคาะจำนวนจริง**

---

## ❓ ต้องเคลียร์ก่อนสรุปราคา
1. **เลือก 5 biome** จาก 6 ธีม (ตัดอันไหน)
2. **Recolor** monster คิดราคาแยกหรือเหมา (ขยาย roster ราคาถูก)
3. **ขนาดไอคอน** item/skill (32×32 vs 48×48) + **ขนาด boss frame** ต่อตัว
4. **เฟสงาน** — ทำชุดแรกแค่ของที่อยู่ในเกมตอนนี้ (5 class-1 + monster band ต้น + skill ~35) ก่อน
   แล้วค่อยเติม class-2 / skill ที่เหลือ ตามที่ dev ทยอย build
