# ⚔️ Skill Rune — **RETIRED 0.0.369** (เก็บไว้เป็นคลังไอเดีย)

> **ระบบนี้ถูกถอนออกจากเกมทั้งหมดเมื่อ 2026-09-14** (user: "rune เป็นระบบ class ที่ยกเลิกไปแล้ว
> ต้องเอาออก" → "งั้นลบเลย") — โค้ดอยู่ใน git ถ้าอยากดู แต่**ทรีปัจจุบันไม่มี rune แล้ว**
>
> **ทำไม**: rune ผูกกับสกิลของ class ที่ยกเลิกไปแล้ว (`twin_strafe` / `arrow_shower` / `bolt`
> ของ archer/mage เก่า) ⇒ ไม่มียูนิตที่เล่นได้ตัวไหนมีสกิลที่ rune augment ได้เลย
> ⇒ **โค้ดตาย 100%**: 12 RuneLine · 13 บล็อกใน combat path · 111 จุดในเทส · ผู้ใช้ 0 คน
> (ตรงกับ memory `systems-built-but-unreachable` — สร้างจบแล้วต่อกับพาหนะที่ไม่เคยออก)
>
> **สิ่งที่ยังมีค่าคือไอเดีย ไม่ใช่โค้ด** — 12 เอฟเฟกต์ข้างล่างใช้เป็นคลังของ **เจม / set effect**
> ที่กำลังออกแบบได้เลย (`gear-fusion-and-base-fix-design.md`)

## คลังไอเดีย: 12 เอฟเฟกต์เดิม → เอาไปใช้ที่ไหนได้

| เอฟเฟกต์เดิม | ทำอะไร | บ้านใหม่ที่เหมาะ |
|---|---|---|
| Chain / Scattershot / Radiance / Arc | กระโดดใส่เป้าถัดไป ดาเมจลดหลั่นตาม tier | **เจมอาวุธ** |
| Cleave | สาดออก 4 ทิศรอบเป้า | เจมอาวุธ |
| Overload | เพิ่มจำนวนลูก bolt ต่อการร่าย | เจมอาวุธ (TBH มี "Projectile Count" บน Chaos Diamond) |
| Fusillade | ×1.6-3.2 ดาเมจ แสดงเป็น N ตัวเลข | เจมอาวุธ |
| Barrage / Flurry | ชาร์จข้ามคูลดาวน์ / ตีถี่แต่เบาลง | **set effect** (ต้องครบชุดจึงได้) |
| Execute | ฆ่าเป้าที่ไม่ใช่บอสซึ่ง HP ต่ำกว่าเกณฑ์ทันที | set effect |
| Vulnerable / BloodMist | ตีตราเป้าให้รับดาเมจเพิ่ม / ลด HIT รอบตัว | เจม accessory |
| Cinders / Combustion | เผาต่อเนื่องเป็นสัดส่วนของหมัด | เจมอาวุธ (กลไก burn ยังอยู่ในสิม มีสกิลใช้จริง) |
| Concussion | สตันเป้าที่ไม่ใช่บอส | set effect |
| Renewal / Benediction | HoT ให้เป้าที่ฮีล / ฮีลจากดาเมจที่ทำได้ | เจม accessory สายซัพ |
| Retribution | ฮีลแล้วฟาดศัตรูด้วยดาเมจ Holy | เจม accessory |
| Focus / Pinning | รวมดาเมจเป็นก้อนเดียว / สโลว์เป้าที่โดน | เจมอาวุธ |

**บทเรียนที่ควรจำ**: ระบบนี้ถูก build เต็มรูปแบบ (ดาเมจ, เทส, UI tooltip, rune stone 5 ชนิดในร้าน)
ก่อนที่จะมีตัวละครที่ใช้มันได้ ⇒ ต้นทุนดูแลเต็ม กำไรศูนย์ **เทสสีเขียวไม่ได้แปลว่ามีคนใช้**

---

# (เนื้อหาเดิมด้านล่าง เก็บไว้อ้างอิง — ไม่ใช่สถานะของเกมแล้ว)

# ⚔️ Skill Rune — DESIGN (แกนที่ 7: augment สกิลเดิม)

> **draft 2026-07-05 · ยังไม่ build** — ผู้เล่นออกแบบเอง. **ต่างจาก Skill Orb**
> (`gear-systems-design.md §5`): Skill Orb = **เพิ่มสกิลใหม่** minor ลงอาวุธ;
> Skill Rune = **เอาสกิลโจมตีที่มีอยู่ มา augment ให้เปลี่ยนพฤติกรรม/เก่งขึ้น**
> (แบบ support crystal ของ PoE / rune-word). สร้าง build identity: สกิลเดิมเล่นคนละแบบ.
>
> **ชื่อในเกม = "Skill Rune"** (ไม่ใช้ "Skill Orb" เพราะ "Orb/Stone" ชนกับตระกูล
> Star Stone ที่ reroll gear enchant แล้ว).
>
> **Companion:** `gear-systems-design.md §5` (Skill Orb — แกนคนละตัว),
> `orb-system-design.md` (Star Stone — pattern หินที่ Rune Stone ลอก),
> `gear-roll-rules.md` (roll-at-drop canon ที่ rune ใช้)

---

## 1. กลไกหลัก

- **Rune ผูกกับ SKILL เดียว + augment เส้นเดียว** (เช่น Twin Strafe — Scattershot)
- **weapon-gated**: augment ทำงานเฉพาะตอนถืออาวุธที่ rune ฝังอยู่ ("potential
  ของอาวุธ") — ถอดอาวุธ = augment หาย
- **tier ขับด้วยตีบวกอาวุธ (user 2026-07-06):** effective tier =
  `max(drop level, refine tier)` — **+7 → lv2, +10 → lv3**
  (`RUNE_REFINE_THRESHOLDS`, ต่อเส้นแก้ได้ทีหลัง "แล้วแต่ Rune"). drop lv1/2/3
  = shortcut (lv3 แรร์ = แตะ max ทันที), refine = ทางไต่ที่เชื่อถือได้ → **refine
  มีความหมายจริง** (แก้ปัญหา [[gear-atk-minor-vs-stats]]). magnitude/slot_cost
  สเกลตาม effective tier
- **cost = active loadout slot** (`ACTIVE_SLOT_CAP = 10`, มีในสิมแล้ว) — augment
  ยิ่ง tier สูงยิ่งกินช่อง → balance lever: rune-max ทุกสกิลไม่ได้ (ตรง pillar
  "เลือก build" [[party-comp-is-the-build]])

## 1.5 Naming convention (user 2026-07-05: style B — ชื่อบอกผล)

`[สกิล] Rune — [เส้น]` โดยชื่อเส้น**บอกพฤติกรรม** (on-brand legibility — เห็นชื่อ
รู้ว่าได้อะไร; ไม่ใช้ Norse rune ทึบ ๆ). สกิลใหม่มา = ตั้งชื่อเส้นตามผลก็จบ.

## 2. Augment lines — ตัวอย่าง Twin Strafe (slice แรก)

Twin Strafe base ตอนนี้ = single-target (`display_hits:2` เป็น visual double-tap
ยิงตัวเดียวโชว์ 2 เลข). สอง augment:

| เส้น | ผล | tier 1/2/3 | reuse ในสิม |
|---|---|---|---|
| **Scattershot** | ยิงหลายเป้าในระยะ | 2 / 3 / 4 เป้า | AoE victim-collection |
| **Barrage** | ได้ charge — ยิงรัวจนหมด **แล้ว cd ค่อยเริ่ม** → refill เต็ม | 2 / 3 / 4 charge | `charges_left` (field มี, ต้องเขียน cooldown-refill path ใหม่ — เดิม Soul Crush refill ตอนเปลี่ยนโซน) |

**Quiver charge model (user 2026-07-05):** มี charge > 0 → **บายพาส cooldown,
ยิงบน ASPD swing timer** (spend 1/ยิง); charge = 0 → cooldown เริ่มนับ; หมด cd →
refill กลับเต็ม tier. = burst-แล้ว-recharge.

**ASPD-gating (ยืนยันจากโค้ด 2026-07-05):** สกิลออกบน attack timer อยู่แล้ว —
`attack_step` สะสม `atk_acc += effective_atk_speed_milli`/tick, ทุก 1000 = 1 swing,
แต่ละ swing `pick_active_skill` เลือกสกิล. ปกติ Twin Strafe ติด cooldown 50t →
cooldown คุม, ASPD ไม่โผล่. **Quiver ปลด cooldown gate ชั่วคราว → ASPD กลายเป็น
ตัวคุมความเร็ว burst โดยตรง** → build AGI/ASPD + Quiver = synergy จริง (เร็ว = charge
หมดไว = วนไว). implement = ให้ charge>0 ข้ามการเช็ค cooldown ตอน pick, แล้วปล่อย
swing timer ทำงาน — ไม่ต้องสร้าง path ยิงใหม่.

pool เต็ม (สกิลอื่น × เส้นอื่น) = ขยายทีหลัง authored ทีละอัน — framework ต้อง
extensible (เพิ่มเส้นใหม่ = เพิ่ม enum variant + hook จุด resolve).

## 3. แหล่งได้มา — roll-at-drop (canon)

- **rune ดรอปมากับอาวุธโดยตรง** (เหมือน enchant/star roll-at-drop — ไม่ใช่เสก
  ทีหลัง) โอกาส**น้อยมาก**, level 1/2/(3 แรร์สุด ๆ)
- draw เป็น **state-keyed conditional** (replay-safe เหมือน star/foil ใน
  `gear_roll` — draw เฉพาะตอนดรอปอาวุธที่ eligible)
- เรทจริงตั้งจาก loot-per-day audit ทีหลัง (volume rule)
- **Rune Stone drop (0.0.143):** `rune_stone_archer` ดรอปจาก boss ~1/13,500
  (≈0.1/วัน, tier C — user: แรร์กว่าอาวุธติด rune เพราะ stone = การันตี+พกพาได้);
  shop 50k = price ceiling/ทางลัด

## 4. Rune Stones — ตระกูลหินขนาน Star Stone (ทำหลัง base ทำงาน)

| หิน | ผล | ลอกจาก |
|---|---|---|
| ~~อัพเลเวล rune~~ | **ตัดออก (0.0.142)** — tier มาจาก refine แล้ว (+7/+10) ไม่ต้องมีหินอัพ | — |
| **ใส่ rune** ✅ 0.0.141 | ฝัง rune lv1 ลงอาวุธที่ยังไม่มี (สุ่มเส้น, class-gated) | — |
| **reroll rune** | สุ่มเส้น augment ใหม่ | Chaos Star Stone |

reuse โครง `OrbEffect` / `UseOrb` / fee-per-use / feature ทั้งหมดจาก
`orb-system-design.md` — เพิ่ม variant ใหม่ ไม่สร้างระบบซ้ำ.

## 5. Implementation sketch

- **Item field**: `skill_rune: Option<SkillRune>` (serde skip-none, per-instance
  earned — preserve across migrate เหมือน star/foil/bonuses)
  - `SkillRune { skill_id: String, line: RuneLine, level: u8 }`
  - `enum RuneLine { Scattershot, Barrage, … }` (APPEND-ONLY — serialized)
- **Weapon-gate resolver**: `rune_on_weapon(entity, skill_id) -> Option<&SkillRune>`
  — อ่านเฉพาะ Weapon/WeaponLeft slot ที่ถืออยู่
- **Cast hooks** (จุด resolve Twin Strafe):
  - Scattershot → target count = base(1) หรือ rune.level+1 (2/3/4) เข้า victim-collection
  - Barrage → charges>0 บายพาส cooldown ยิงบน swing timer; cd เริ่มเมื่อ charges→0; refill เมื่อ cd→0
- **Slot cost**: augmented skill กิน loadout slot เพิ่มตาม tier (ที่ `skill_slotted`/
  cap เช็ค) — สกิลเดียวแต่กินหลายช่องเมื่อ rune แรง
- **roll-at-drop**: ใน `drop_on_death` หลัง enchant roll — 1 draw state-keyed
- **Migrate**: preserve `skill_rune` per-instance (ไม่ re-roll ตอนโหลด — no PRNG)
- **UI/tooltip**: โชว์ augment บนอาวุธ + สกิลที่โดน + tier + slot cost

## 6. เรื่องที่ยังไม่ตัดสิน (default ที่จะใช้ถ้าไม่แย้ง)

1. **slot cost = per-tier หรือ flat +1?** — default: **per-tier** (2★→+1, 3★→+2,
   4★→+3) ให้ balance lever มีน้ำหนัก; ปรับได้
2. rune หลายอันบนอาวุธเดียว (2 มือ = 2 rune?) หรือ 1 rune/อาวุธ — default: **1/อาวุธ**
3. augment ซ้อน enchant `bonuses` ได้ไหมบนอาวุธเดียว — default: **ได้** (คนละ field)
4. เรทดรอป lv1/2/3 ตัวเลขจริง — audit ทีหลัง

## 7. Build order (vertical slice ก่อน)

1. ~~**สิม slice**: SkillRune field + weapon-gate + Scattershot + Barrage + slot cost~~
   ✅ **BUILT 0.0.139** (golden byte-identical; tests ครบ)
2. ~~**roll-at-drop**: rune ติดอาวุธ lv1/2/3~~ ✅ **BUILT 0.0.140** (Bow ~1%,
   lvl 82/15/3; draw state-keyed → golden ไม่ขยับ; เรท draft จูนด้วย audit)
3. **Rune Stones**: ✅ **ใส่ (InsertRune) BUILT 0.0.141** (class-gated) / อัพ =
   ตัดทิ้ง (refine แทน 0.0.142) / **reroll = ยังไม่ทำ**
3b. **tier ↔ refine** ✅ **BUILT 0.0.142** (+7→lv2, +10→lv3, effective=max(drop,refine))
4. **UI + tooltip** — ยังไม่ทำ (ผู้เล่นยังไม่เห็น rune บนอาวุธ)
5. ขยาย pool: สกิลอื่น × เส้นอื่น authored — ยังไม่ทำ
