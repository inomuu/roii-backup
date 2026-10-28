> **RETIRED (0.0.381, 2026-09-15).** The Star Stone system described here was cut in full —
> user: "ตัดระบบ Star Stone ออกทั้งหมดเลย". With the fixed-stat gear model (`gear-fusion-and-base-fix-design.md`
> D1) a drop has no ★ and no rolled line, so nothing was left for a stone to act on. The `orbs`
> module, the eight stone defs and the Enchant screens are gone; `OrbEffect`/`UseOrb` survive only
> as deserialize-only types so old saves and logs read. Kept as a record of the design.

# 🪨 Orb System (หินเสก/หินเขย่า) — DESIGN

> ## ⚠️ PARKED 0.0.369 (2026-09-14)
> Star Stone ทั้ง 8 ชนิด **ถูกพักไว้**: โค้ด `orbs.rs` + def + เทสยังอยู่ครบและเขียว แต่
> `loot_rules.orbs` ว่างและ `buy_price` = 0 ⇒ **ไม่ดรอป ไม่มีขายในร้าน** (user: "star stone
> ก็พักไว้ก่อน") ปลุกคืนด้วยการใส่กฎดรอปกลับ — วิธีเดียวกับที่ทำกับ Gil AH
> **Rune Stone 5 ชนิดถูกลบจริง** (ไม่ใช่พัก) พร้อมระบบ rune ทั้งระบบ — ดู `skill-rune-design.md`


> **Patch-1 set BUILT 0.0.138 (2026-07-05)** — Chaos/สี×5/Death/Shiny ทำงานจริง
> ในเกมแล้ว (`orbs.rs` + UI จิ้มหิน→จิ้มของใน sidebar); เรทดรอปตามตาราง §2
> ที่เคาะแล้ว; essence-like ยังไม่ build
>
> ~~draft 2026-07-05 · ยังไม่ build~~ — ออกแบบบนฐาน ROLL-AT-DROP (0.0.136,
> `gear-roll-rules.md`): ของติดดาวดรอปมาพร้อมค่าเต็มทุกช่องแล้ว → **orb ทุกตัวคือ
> เครื่องมือ "แก้ของที่มี" ไม่ใช่ "เติมของว่าง"** — เนื้อหาสำหรับผู้เล่นที่ invest แล้ว
> (user: "ผู้เล่นที่ไหนจะอยากมานั่ง roll stat เองแต่แรก ต้อง invest กับเกมระดับนึงก่อน")
>
> **Companion:** `gear-roll-rules.md` (กฎ roll ที่ orb ต้อง reuse ทุกข้อ),
> `gear-systems-design.md` §3 (ตารางจังหวะเดิม — doc นี้ทับ), economy brief (cap/วัน)

## 0. หลักการตายตัว

1. **Orb ใช้แล้วผลสุ่ม** 
2. **Orb = commodity หลักของตลาด Gil** — fungible, stack ได้, ราคาตลาดลอยตัว
3. **Reuse กฎ roll เดิม 100%** — pool/slot gate/tier window/น้ำหนัก 60:30:10/
   ห้าม family ซ้ำ ตาม `gear-roll-rules.md` — orb ห้ามมีตารางของตัวเอง
4. **การใช้ = recorded InputEvent + draw ตามลำดับตายตัว** (replay-safe แบบ
   เดียวกับ refine) — invalid = silent no-op
5. **เพดาน determinism**: เครื่องมือเล็งเจาะจงได้สูงสุด **1 ช่อง/ใบ** (บทเรียน
   PoE2 0.5 — กันคันโยกกลืน RNG chase หมด)

## 0.5 ปรัชญา crafting: ปาร์ตี้ 5 ตัว → ฐาน merciful, ปลาย brutal (confirm 2026-07-16)

**บริบทต่างจาก PoE:** PoE = ตัวเดียว โหด RNG ได้ (ทุ่มทุกอย่างลงชิ้นเดียว) —
**ROI = ปาร์ตี้ 5 ตัว** ต้อง gear **×5** ถ้า craft โหดเท่า PoE ความเจ็บคูณ 5 →
ขัดกลุ่ม idle/casual-leaning ของเรา ∴ crafting **merciful กว่าเล็กน้อย** = scaling
ให้ถูกกับ party format **ไม่ใช่ dumbing down**.

**โครง 2 ชั้น (บาลานซ์ทั้งสองฝั่ง):**

| ชั้น | ระบบ | โทน | บทบาท |
|---|---|---|---|
| **ฐาน (accessible)** | **Star Stone** (Chaos reroll + Death lock 1 บรรทัด + สีอัพดาว + Shiny) | **merciful / targeted-ish** | พา 5 ตัวไปถึง "good enough" ไม่ทรมาน |
| **ปลาย (aspirational)** | **Refine (ตีบวก)** — fail = ลดขั้น (RO-style, `refine.rs`) | **brutal / high-stakes** | grind ระยะยาว + Gil/ore sink หนัก สำหรับสายฮาร์ดคอร์ |

→ **merciful ที่ฐาน + โหดที่ปลาย** เสิร์ฟทั้ง casual (พาปาร์ตี้ครบได้) และ
hardcore/PoE-fan (มีอะไรให้ปั่นต่อ) พร้อมกัน โดยไม่พังฝั่งใดฝั่งหนึ่ง.

**เทียบ PoE2 EA:** PoE2 ตัด Orb of Alteration ออก → reroll ของฟ้าตรง ๆ ไม่ได้.
ROI มี **Death (lock 1 เส้น) + Chaos (reroll ที่เหลือ)** = **targeted crafting**
ที่ PoE2 EA ยังไม่มี → ฐาน crafting ของเรา **ให้อภัย/เจาะจงกว่า PoE2** ตอนนี้ —
สอดคล้องเพดาน determinism §0.5 (lock 1 ช่อง/ใบ = mercy ที่มีเพดาน ไม่กลืน chase).

**"merciful" = *เล็กน้อย* เท่านั้น** — ถ้ามากไป: chase หมดแรงดึง (retention),
Gil sink อ่อน (เฟ้อ — ขัด monetary-policy), BiS ได้ง่าย (ไม่มีดีมานด์เทรด Steam
Market). **Lever ปรับระดับ:** ค่า reroll (§3), จำนวนเส้นที่ lock ได้ (ตอนนี้ Death =
สุ่ม 1 เส้น; ถ้าให้เลือก/หลายเส้น = merciful ขึ้น), StarUp fail ไหม (ตอนนี้**ไม่
fail** — merciful อยู่แล้ว). Refine เป็นตัวคุมความโหดฝั่งตรงข้ามเพื่อรักษา chase.

## 1. รายการ orb ตามจังหวะปล่อย

| จังหวะ | ชื่อ (fiction ค่อยตั้ง) | ผล | อ้างอิง |
|---|---|---|---|
| **PATCH 1** | **Chaos Star Stone** (หินเขย่า) | สุ่มบรรทัด **ใหม่ทั้งหมดตามจำนวน ★ ของไอเทม** (family/tier/param/value — reuse `roll_bonuses` ตรง ๆ) | PoE1 Chaos — ตัวหมุนหลักของตลาด/ว่าที่หน่วยเงิน |
| **PATCH 1** | **Star Stones ×5 สี** (Green→Red) | อัพดาวทีละขั้น 0★→5★ — ดู §1.1 | บันได Exalt ของเรา |
| **PATCH 1** | **Death Star Stone** | **ล็อค 1 บรรทัดแบบ "สุ่ม"** (ทั้งบรรทัด: family+tier+ค่า — Chaos/Shiny ไม่โดน) · ของ 1 ชิ้นล็อคได้**ทีละ 1 บรรทัด** · **ใช้ซ้ำได้ = ทอยย้ายล็อค** | ≠ PoE Fracturing (one-shot): determinism แปลงเป็นต้นทุน (5★ ≈ 5 ก้อน/เป้า) → demand ถาวร; ไม่ต้องมีกฎขั้นต่ำดาว |
| **PATCH 1** | **Shiny Star Stone** (หินขัด — ชื่อ user 2026-07-05) | reroll **เฉพาะตัวเลข** ทุกช่อง (family/tier เดิม อยู่ใน range ของ tier; **ช่องที่ Death ล็อคไม่โดน**) | Divine — ตัวจบ perfect-roll chase |
| Update 3 | essence-like | เขย่าทั้งใบแบบ **การันตี family ที่เลือก 1 บรรทัด** (เฉพาะ family ยอดนิยม ~5-8 ชนิด) | PoE essence |
| เลื่อนไม่มีกำหนด | omen / swap-1 / targeted-reroll | ดัดผลครั้งถัดไป / ถอด1ใส่1 / จิ้มช่อง | user 2026-07-05: "complicate เกินไป ค่อยมาอัพเดทหลัง" (หินเสก targeted ก็ตัดแล้ว — บทเรียน Harvest) |

**PATCH 1 = ครบทั้งตระกูล 8 ก้อน (user 2026-07-05): Chaos + สี ×5 + Death + Shiny**
— loop สมบูรณ์ตั้งแต่แพตช์แรก: ฟาร์ม → อัพดาว (สี) → หมุน (Chaos) → ล็อค (Death)
→ ขัดเลข (Shiny) → ขาย; ความละเอียดชั้นถัดไป (essence) ค่อยหยอดทีหลัง

**ทำไม Shiny ตั้งแต่ Patch 1 ไม่ทำ chase จบเร็ว (user 2026-07-05):** perfect
roll ของเราหมดอายุเป็น "ยุค" ตาม Update Kit อยู่แล้ว — ilv ใหม่ (s8/s9…) base
สูงกว่า + tier window เลื่อนปลด T4/T5 ที่ของเก่าไปไม่ถึง + มอนยุคใหม่โตคูณ/
gear โตบวก → BiS เก่าเสื่อมเอง ทุก era ทุกคน craft ใหม่จากฐานใหม่ = ตระกูล
Star Stone มี demand ถาวรข้ามยุค และ BiS มือสองไหลลงเป็นของผู้เล่นใหม่/alt
(ตลาดหมุนเองโดยไม่ต้อง league wipe แบบ PoE)



### 1.1 Star Stones — ตระกูลหินดาว (user 2026-07-05)

ทั้งสาย orb ใช้ชื่อตระกูลเดียว "**Star Stone**": 5 สี = อัพดาว, **Chaos** =
เขย่าบรรทัดใหม่ (ตาราง §1) — ภาษาเดียวกัน ผู้เล่นเก็ตทันทีว่าเป็นของสายเดียวกัน

**REVISED 2026-07-24 (user):** collapse the 5 single-step colored stones into **3 stones**, each of
the low two covering **2 star-steps**; the 1★ and 3★ single-step stones are removed. The old
Green/Blue → the 2★ stone; Purple/Yellow → the 4★ stone; Red (unchanged) is the 5★ endgame stone.

| หิน | อัพ | note | แหล่ง (draft) |
|---|---|---|---|
| **Blue Star Stone** (2★) | 0★→1★ **และ** 1★→2★ | คุม 2 ขั้นล่าง (ยุบ Green+Blue เดิม) | trash ทุก band |
| **Purple Star Stone** (4★) | 2★→3★ **และ** 3★→4★ | คุม 2 ขั้นกลาง (ยุบ Purple+Yellow เดิม) | boss / dungeon |
| **Red Star Stone** (5★, เดิม) | 4★→5★ | endgame — **ของแพงสุดในเกม** (5★ = apex ที่ดรอปไม่ได้) | endgame |

- **NAMING (user 2026-07-24): Blue → Purple → Red** — สีตอนนี้ = **บันได tier (cool→warm)** ไม่ผูกกับ
  rarity ปลายทางแล้ว (หินคุม 2 ขั้น เลยแมป rarity 1:1 ไม่ได้). Blue = ของ 0-1★, Purple = ของ 2-3★,
  Red = ของ 4★. Kept only 3 of the 5 colors: **dropped Green + Yellow** (game_data has all 5 defs;
  merging/removing the 2 = a build task). Chaos / Death / Shiny endgame stones unchanged.
- ใช้ได้กับ gear ทุก tier ที่ดาวตรงขั้น (หินผูกกับ "ขั้นดาว" ไม่ใช่ tier ของ);
  Gil fee สเกลด้วย item_level (§3) — อัพของ s7 แพงกว่า s1 เอง
- อัพแล้ว**บรรทัด enchant ช่องใหม่ roll ทันที** (canon roll-at-drop) +
  base floor +10% ต่อดาวมาเองจากระบบเดิม
- **5 commodity แยกราคาในตลาด Gil**; แจกตาม content tier → ทุกยุคมีหิน
  ให้ฟาร์ม = วัตถุดิบ Update Kit
- **watch-point**: อัพจาก 0★ ได้ทำให้ดรอปดาวธรรมชาติพิเศษลง — คุมด้วย
  ราคา: Green/Blue ไม่แรร์แต่ Gil fee จริงจัง (ให้ฟาร์มดรอปดาวต่ำเองยังคุ้ม
  กว่า), ดาวสูงกลับด้าน — หินคือทางหลักเพราะดรอป 4★ ~1/เดือน

## 2. แหล่งได้มา + เรท (volume rule — ตั้งจากผลลัพธ์/วัน)

- **pure drop จากมอน** (in-sim `roll_drop` ปกติ — ไม่มี 3h loop): Chaos Star
  Stone หลุดจาก trash ทุก band, Star Stones สีสูงหลุดจาก **boss/dungeon**
  (Green/Blue จาก trash — ดูตาราง §1.1)
- **เป้าผลลัพธ์/วัน (idle 24 ชม. at-band, ยึด audit 2026-07-05 = 33.6k kills
  + 1,358 boss/วัน):** Chaos Star Stone **~8-12/วัน** · Star Stones สี:
  Green/Blue หลัก~หน่วย/วัน, Purple/Yellow ~หน่วย/สัปดาห์, Red ~หน่วย/เดือน
  (draft) — ⚠️ boss มาทุก ~1-2 นาที: ราคาต่อ-boss ต้องหารพันสามร้อยต่อวัน
  ห้าม pin จากความรู้สึก (บทเรียน [2] 1/28 flood)
- ~~ไม่ขายในร้าน NPC~~ → **ขายในร้าน NPC แล้ว (user 2026-07-05)** — ราคา NPC
  ทำหน้าที่เป็น price CEILING ของตลาด Gil (ตลาดผู้เล่นต้องถูกกว่าร้านเสมอ):
  Green 1.5k / Blue 4k / Purple 12k / Yellow 30k / Red 100k / Chaos 2.5k /
  Death 12k / Shiny 18k (draft); ขายคืน NPC ราคา junk กันขยะล้น
- **daily soft-cap ต่อบัญชี** สำหรับสถานะ tradeable ตาม economy brief
  (ที่ volume ระดับนี้ cap ต่อบัญชีคือตัวคุมจริง ไม่ใช่เรทต่อ roll)

## 3. Gil sink — ค่าธรรมเนียมการใช้ (คู่กับ orb เสมอ)

ทุกการใช้ orb กิน **Gil fee** เหมือนตีบวก (แพตเทิร์น refine เป๊ะ: ore+Gil):

| orb | Gil fee (draft) |
|---|---|
| Chaos Star Stone | `500 × item_level` |
| หินสีอัพดาว | `5,000 × ดาวปลายทาง × item_level` (draft) |
| Death Star Stone | `2,000 × item_level` (draft) |
| Shiny Star Stone | `1,500 × item_level` (draft) |

→ orb เป็นของหมุนเวียนin ตลาด (player→player) แต่ **Gil ถูกเผาทุกครั้งที่ใช้** =
sink โตตามกิจกรรม craft เอง (แก้โจทย์ "Gil ไม่ bound ต้องมี sink แรง")

## 4. Implementation sketch (sim)

- Item ใหม่ kind `Material` + ธง `orb_effect` ใน def (enum: RerollAll /
  RerollSlot / StarUp / …) — ไม่เพิ่ม ItemKind ใหม่ (save-compat)
- `InputEvent::UseOrb { orb_stash_idx, target: OrbTarget, slot: Option<u8> }`
  — target = equipped(entity_id, equip_slot) | stash(idx); ทุก validation
  พลาด = silent no-op (เลเวล/ดาว 0/ไม่ใช่ gear/unique/Gil ไม่พอ)
- **Draw order ต่อการใช้** (สัญญาใหม่ ต่อท้าย `gear_roll`):
  RerollAll = ลำดับเดียวกับ roll-at-drop เป๊ะ (reuse `roll_bonuses`) ·
  StarUp = star+1 แล้ว roll บรรทัดใหม่ 1 บรรทัด (family draw จาก
  eligible-list ตัด family ที่ช่องอื่นถือ) · Death lock = `locked_slot:
  Option<u8>` บน Item (serde skip-if-none) — ใช้ Death = 1 draw สุ่ม index
  ใหม่; RerollAll ข้ามช่องที่ล็อค + family ที่ล็อคถูกตัดจาก pool ของ
  ช่องอื่น (กฎห้ามซ้ำเดิม)
- **unique ใช้ orb ไม่ได้** (แท็ก 0.0.135) — authored คือ authored
- VERSION bump + golden: ไม่มี orb ใน scenario เก่า → byte-identical

## 5. UI (Update 1 ขั้นต่ำ)

- ใช้จากช่อง inventory: คลิกขวา orb → cursor เปลี่ยน → คลิกของเป้าหมาย
  (แบบการ์ด RO เสียบรู) — โชว์ preview: บรรทัดปัจจุบัน + Gil fee + ปุ่มยืนยัน
- ผล roll ใหม่เด้งเป็น tooltip diff (เก่า→ใหม่) ให้เห็น "หมุนได้อะไร" ชัด ๆ
- **เปิดเผย odds ตรง ๆ** ในหน้า confirm (tier weight 60:30:10, family pool
  ของ slot นั้น) — transparent-odds brand

## 6. เรื่องที่ยังไม่ตัดสิน (รอ user)

1. เรท/วัน ในตาราง §2 (ตัวเลข draft ทั้งหมด — จูนผ่าน sim-cli audit ก่อนปล่อย)
2. Gil fee ตาราง §3 (draft)
3. หินเขย่าใช้กับของคนอื่นในตลาดก่อนขายได้ไหม (craft-to-sell) — โน้มเอียง: ได้,
   ของ roll แล้วก็อยู่ตลาด Gil อยู่แล้ว (per-instance)
4. จังหวะปล่อยจริง: launch พร้อมหินเขย่าเลย หรือเก็บทั้งชุดไว้ market update
   (CCU flywheel แนะนำอย่างหลัง — launch โฟกัส core loop)
