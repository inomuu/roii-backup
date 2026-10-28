# 🎲 Gear Roll Rules — กฎการ roll enchant (AS IMPLEMENTED)

> **v0.0.136-alpha · 2026-07-05** — กฎ ROLL-AT-DROP ตามที่ implement จริงใน
> `crates/sim-core/src/gear_roll.rs` (spec ต้นทาง: `gear-systems-design.md` §2)
>
> หลักใหญ่ (user): **"ให้ roll stat ลงไปเลย"** — ของติดดาวดรอปมา**พร้อมค่าเต็มทุกช่อง
> ทันที** ผู้เล่นใหม่ไม่ต้อง craft แค่เก็บของแรงขึ้น; หิน/orb = เครื่องมือ **REROLL**
> สำหรับผู้เล่นที่ invest แล้ว (ยังไม่ build — รอคุย)

**สารบัญ:** 1. เมื่อไหร่ roll · 2. ลำดับ draw (สัญญา PRNG) · 3. Family pool + slot gate ·
4. Tier window · 5. ตารางค่า · 6. กฎห้ามซ้ำ · 7. Param (element/race/size/status) ·
8. Determinism / migration · 9. จุดต่อ UI

---

## 1. เมื่อไหร่ roll

- **ตอนดรอปเท่านั้น** — ใน `drop_on_death` หลัง star roll + rarity promotion:
  `item.bonuses = gear_roll::roll_bonuses(&mut state.rng, &item)`
- roll เฉพาะของที่ `star_eligible` (Weapon/Armor kind + ไม่ติดแท็ก `unique`)
  และ **star > 0** — ของ 0★ ไม่กิน draw สักครั้ง (stream เดิมไม่ขยับ)
- จำนวนบรรทัด = จำนวนดาว (1★ = 1 บรรทัด … 4★ = 4 บรรทัด; 5★ = บ่มทีหลัง)
- ร้านค้า/ของ authored ไม่ผ่านเส้นนี้ — เฉพาะ loot จริงจากมอน

## 2. ลำดับ draw ต่อ 1 ช่อง (สัญญา PRNG — APPEND-ONLY)

ต่อบรรทัด enchant หนึ่งบรรทัด draw ตามลำดับตายตัว:

| # | draw | หมายเหตุ |
|---|---|---|
| 1 | **family** | index บน list "family ที่ slot นี้ roll ได้ − ที่ใช้ไปแล้ว" (ไม่มี rejection loop — จำนวน draw คงที่) |
| 2 | **tier** | ถ่วงน้ำหนัก 60:30:10 ตัดตาม window ของ def (§4) |
| 3 | **param** | เฉพาะ family ที่มีพารามิเตอร์ (element/race/size/status) |
| 4 | **value** | uniform ในช่วงของ tier; **ช่วงค่าเดี่ยว (min==max) ไม่กิน draw** |

ลำดับ family ใน pool / ตารางน้ำหนัก / ตารางช่วงค่า = ส่วนหนึ่งของ replay stream —
**ห้าม reorder, เพิ่มได้ท้าย list เท่านั้น** (เหมือนกฎ enum ใน save)

## 3. Family pool (29) + slot gate

**Pool อิสระ — roll ได้ทุก slot (19):**
STR/AGI/VIT/INT/DEX/LUK · MaxHP แบน · MaxHP% · ATK% · MATK% · ATK แบน ·
MATK แบน · HIT · FLEE · CRIT · Dmg-vs-element · Dmg-vs-race · Dmg-vs-size ·
Resist-element

**Slot-gated (10) — บุคลิก paper doll:**

| family | roll ได้บน | เหตุผล |
|---|---|---|
| Ignore DEF % / Ignore MDEF % | อาวุธ (รวมมือรอง) | อาวุธ=เจาะ; สาย zero-out ต้องคุมแหล่ง (cap 50%) |
| **Heal +%** | **อาวุธ Staff/Mace เท่านั้น** | user 2026-07-05: "แค่ staff + mace ที่ acolyte ใช้ได้" |
| Cast time −% | หัว (Top/Mid/Low) | Kathryne lineage; cap 50% |
| Cooldown −% (CDR) | accessory | cap 30% |
| DR vs race / DR vs size | เสื้อ (Armor) + โล่ | Thara Frog lineage; cap 50% |
| Resist status % | เสื้อ + ผ้าคลุม | cap 75% ร่วมกับการ์ด/path |
| DR vs ranged | ผ้าคลุม | Horn lineage; cap 50% |
| Move speed +% | รองเท้า | cap 30% |

slot อ่านจาก `item.slot()` (explicit `equip_slot` ของ def → fallback ตาม kind);
weapon_type อ่านจาก def → มือรอง (WeaponLeft) นับเป็นอาวุธ

## 4. Tier window — ผูกป้าย `_sN` ของ def (ilvl ที่มองเห็น)

- `def_tier`: parse segment `s<N>` จาก def_id — `sword_s3` / `sword_s3_1s` → 3;
  ไม่มีป้าย (starter/legacy) → 1. **ไม่มี ilvl ซ่อนต่อ instance** — 0★ ทุกใบ
  ของ def เดียวกันเหมือนกัน 100% (fungible)
- window เลื่อนตาม tier ของ def: **s1-4 → T1 เท่านั้น · s5-8 → T1-T2 ·
  s9+ → T1-T3** (band ใหม่ในอนาคต = เพิ่มชั้นบน ไล่ตาม rule 2 ของ design)
- น้ำหนักเลือก tier ใน window: **ต่ำ:กลาง:สูง = 60:30:10** (ตัดเหลือเท่าที่มีแล้ว
  normalize ในตัว — window เดียว = 100%, สอง = 2:1)

## 5. ตารางช่วงค่า (native unit; % = basis points)

| family | T1 | T2 | T3 |
|---|---|---|---|
| สเตต (STR…LUK) | +1 | +2–3 | +4–5 |
| MaxHP แบน | (8–12)**×band** | (16–25)×band | (25–35)×band |
| MaxHP % | 1–2% | 3–4% | 4–6% |
| ATK % / MATK % | 2–3% | 4–6% | 6–9% |
| ATK / MATK แบน | +2–4 | +5–9 | +10–16 |
| HIT / FLEE | +2–4 | +5–8 | +8–12 |
| CRIT | 1% | 2% | 3% |
| Dmg vs element/race/size · Resist element · Resist status · DR ranged | 5–8% | 10–15% | 15–20% |
| DR vs race/size | 3–5% | 6–10% | 10–13% |
| Heal +% | 3–5% | 7–10% | 10–13% |
| Cast −% | 3–5% | 7–10% | 11–15% |
| CDR | 2–3% | 4–6% | 7–10% |
| Ignore DEF/MDEF | 3–5% | 6–10% | 12–15% |
| Move speed | 3–5% | 6–9% | 10–14% |

- **MaxHP แบนเป็น family เดียวที่คูณ band** (`def_tier`) — HP pool โตตามยุค
- ช่วงไม่ทับกันระหว่าง tier → เห็นค่า = รู้ tier (design rule: legibility)

## 5.5 Star base-stat floor — ดาวละ +10% ของ base ชิ้นนั้น

- **ทุกดาวบวก +10% ของ base stat ของชิ้นเอง** (ATK/MATK/DEF — ปัดขึ้น):
  1★ +10% … 5★ +50% (user 2026-07-05: "ดาวเยอะ + base stat ไปด้วยเลย
  ไม่งั้นดาวเยอะ stat กาก off ไม่ตรง ได้แล้วซึม" — floor การันตีว่าดาวเยอะ
  ดีกว่า 0★ เสมอ ไม่ว่า roll จะ off แค่ไหน)
- **derive สดจาก `star` ตอนอ่าน** (`Item::star_atk_bonus/…` — ไม่ bake ลง
  instance → ไม่มีปัญหา migration, ไม่กิน PRNG, ของเก่าก่อน 0.0.136 ที่มีดาว
  ได้ floor นี้ย้อนหลังอัตโนมัติแม้บรรทัด enchant จะว่าง)
- base 0 ไม่ประดิษฐ์ stat ใหม่ (อาวุธไม่มี def → ดาวไม่ให้ def)
- จุด fold: `effective_atk` / `effective_matk` / `effective_hard_def` +
  หน้า status (equip split) + tooltip (`star_atk`/`star_matk`/`star_def`)

## 6. กฎห้ามซ้ำ — ระดับ FAMILY

- **1 family ต่อใบ**: `DmgVsRace` ×2 ไม่ได้ (ต่อให้คนละเผ่า) แต่
  **race + size + element คู่กันได้** (คนละ family) — ตอนคิดดาเมจ:
  **ในแกนเดียวกันบวก** (Fire สองชิ้น = รวมเป็นก้อนเดียว) แล้ว
  **ข้ามแกนคูณกัน**: +20% element × +20% race × +20% size = **×1.728**
  ไม่ใช่ ×1.6 (user 2026-07-05: "เราจะ * เพราะเป็นกึ่ง incremental เกม" —
  `dmg_vs_bp` ใน gear_mods.rs มีเทสต์ pin 7280 bp; จัดครบสามแกนใส่เป้า
  เดียว = payoff ของ build ที่ตั้งใจ)
- **ชั้นนับตามแกนของโบนัส ไม่ใช่ชนิดไอเทม** (user 2026-07-05): +vs Medium
  จากการ์ด + enchant + สกิล = แกน size เดียวกัน → **บวกเป็นก้อนเดียว**
  `(1+รวม%)` แล้วค่อยคูณข้ามแกน (การ์ด 10% + enchant 15% + สกิล 5% =
  ×1.30 ไม่ใช่ 1.10×1.15×1.05). ตอนนี้แกน dmg-vs มีแต่ enchant — **ตอน
  การ์ด/สกิล conditional มาทีหลัง ต้องบวกเข้า bucket เดิมใน `dmg_vs_bp`
  ห้ามเปิดชั้นคูณใหม่**
- **ATK% / MATK% จาก gear ก็คูณแยกชั้น** (user 2026-07-05 "atk% ก็น่าจะคูณ"):
  บรรทัด gear บวกกันเป็นก้อนของตัวเอง แล้วก้อนนั้น**คูณ**ทับ bucket % จาก
  บัฟ/การ์ด/Bloodthirst — สูตร: `base × (1+บัฟ+การ์ด) × (1+gear ATK%)`
  (effective_atk / effective_matk ใน derived.rs). กติกากลาง semi-incremental:
  **แหล่งเดียวกันบวก, คนละชั้นคูณ** — ครอบครัวอื่น (HP%, HIT ฯลฯ) ยังชั้นเดียว
- implement ด้วย eligible-list ตัด family ที่ใช้แล้วก่อน draw
  (test เทียบ `std::mem::discriminant`)
- ถ้า slot มี family เหลือน้อยกว่าจำนวนดาว (เป็นไปไม่ได้ในทางปฏิบัติ —
  ทุก slot มี ≥20 family) จะหยุดที่เท่าที่มี

## 7. Param rolls

| family | param จาก | จำนวน |
|---|---|---|
| Dmg/Resist element | Fire Earth Water Wind Holy Shadow Poison Thunder (**ไม่มี Neutral**) | 8 |
| Dmg/DR race | 10 เผ่า RO (ไม่มี Unknown) | 10 |
| Dmg/DR size | Small / Medium / Large | 3 |
| Resist status | Freeze / Poison / Blind / Stun | 4 |

param = 1 draw uniform หลัง tier ก่อน value

## 8. Determinism / migration

- draw ทั้งหมด key ด้วย (ตาราง static + def ของไอเทม + สถานะ PRNG) —
  replay ดรอปเดิม = บรรทัดเดิมเป๊ะ
- **ของติดดาวที่ดรอปก่อน 0.0.136 = ว่างถาวร** — migration ห้ามแตะ PRNG
  (กฎเหล็ก no-retro-roll เดียวกับดาว 0.0.132); tooltip ติดป้าย
  "unenchanted - dropped before the enchant update"
- effect ทุก family ทำงานจริงแล้วตั้งแต่ 0.0.134 (`combat/gear_mods.rs` —
  additive bucket + cap ที่จุดใช้)

## 9. จุดต่อ UI

- `bonuses` ไปกับ item view JSON อัตโนมัติ (serde ทั้งก้อน)
- format บรรทัด: `ItemDesc.mod_line()` ใน `godot/views/item_desc.gd`
  (serde shape: `{"Str": 2}` / `{"DmgVsElement": ["Fire", 500]}` /
  `{"ResistStatusBp": [3, 1500]}`)

---

**งานต่อจากนี้ (ยังไม่ build):** หินเขย่า (reroll ทุกช่อง — commodity ตลาด Gil) ·
บทบาทใหม่ของหินเสก (ไม่มีช่องว่างแล้ว — candidate: reroll เจาะจง 1 ช่อง) ·
วัสดุอัพดาว 4★→5★ · เปิดเผย weight ใน tooltip (transparent-odds) ·
loot-per-day audit ผ่าน sim-cli ก่อน launch
