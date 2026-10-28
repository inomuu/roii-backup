# Gem — ชั้น "ของของฉัน" บนของ fungible (design v2, 2026-09-16)

สถานะ: **สร้างแล้วใน sim 0.0.396 / catalog 0.0.146** (`crates/sim-core/src/gems.rs`) — เหลือ UI ฝั่ง Unity (§8) · v3: gem ใช้ grade เดียวกับ gear · ไม่มีคำว่า mana · ช่อง gem + ช่องการ์ดเรียงตาม grade (v4) · แทนที่ Star Stone/enchant ที่ตัดไป 0.0.381 · ศึกษา: `gem-study-tbh-wog.md`
· คำตอบ user (WoG): ช่องตาม grade · วางทับแล้วอันเก่าหาย · ไม่ bound (แต่ไม่ใช่กฎ Steam — TBH บังคับ "ถอดให้หมดก่อนขาย") · Mana jewel มีได้

## 1. ทำไม
- โมเดลไอเทม: ของดิบ fungible (ชื่อ = สินค้า) → **ผู้เล่นทำเพิ่ม → ของฉัน** — ครึ่งหลังหายไปตั้งแต่ตัด ★/enchant
- ชั้นเพิ่ม ไม่แตะ pacing ⇒ ทำก่อนรื้อบันได (`stage-ladder-renovation.md`)
- gem เป็น**ของสิ้นเปลือง** (วางทับหาย · ผสม 6:1) ⇒ อุปสงค์ถาวรในตลาด material

## 2. กติกา

| เรื่อง | กติกา | ที่มา |
|---|---|---|
| **ช่องต่อชิ้น** | **เรียงตาม grade ทั้ง gem และการ์ด** (user 2026-09-16 "เข้าใจง่ายกว่า"): Tier 1 ไม่มี · Tier 2 gem 1 · Tier 3 gem 1 การ์ด 1 · Tier 4-5 gem 2 การ์ด 1 · Tier 6-7 gem 2 การ์ด 2 · (Tier 8-10 gem 3 การ์ด 2) — ทุกช่องสวมใส่ 10 ช่อง · ของ `_1s`/`_2s` ตัดทิ้ง (175 def) ช่องการ์ดไม่อยู่ที่ def อีกแล้ว | `gems::GEM_SLOTS_BY_GRADE` / `CARD_SLOTS_BY_GRADE` |
| **ช่องไหนใส่ชุดไหน** | gem อาวุธ → Weapon + Shield (offhand) · gem เกราะ → Helmet / Armor / Gloves / Shoes · gem ประดับ → Necklace / Ring / Earring / Bracer · gem สากล → ทุกช่อง (10 ช่อง — **Pants ตัดออก 0.0.389**: Bracer ตั้งใจมาแทนช่องเกราะ ไม่ใช่เพิ่ม) | user "gem weapon ใช้ได้ทั้ง weapon + offhand / armor 4 ช่องหลัก / accessory 4 ช่อง" |
| **gem = ชุด stat** | 1 gem = **2 stat พร้อมกัน** ตามหมวด (§3) | WoG Strife = ATK% + CritDmg% |
| **grade ของ gem** | **grade เดียวกับ gear 10 ขั้น** (Common…Eternal, ชื่อ/สีเดียวกัน) · ค่าโต **×1.3/ขั้น** (raw ladder ของ gear) · เพดาน launch = Legendary เหมือน gear | user "gem มี grade ตาม gear" |
| **ค่าสุ่ม** | ตอน**ใส่** สุ่มค่าแต่ละ stat ใน window ±5% ของค่ากลาง — 1 draw ต่อ stat จาก `state.rng` (client-roll; ส่วนต่างเล็กจนไม่คุ้มโกง) | WoG |
| **ที่มา** | (1) ดรอปจากมอน grade ตามตาราง `grade_odds` เดียวกับ gear (ledger material) · (2) **ผสม 6 grade เดียวกัน → 1 grade ถัดไป ชนิดสุ่มตาม weight** (2 draw: ล้มเหลว → ชนิด) · **ล้มเหลวได้** `fail_bp_by_grade` ตาม grade ตั้งต้น 0 / 0 / 10 / 20 / 30 / 40 / 50…% (TBH ระดับสูงสำเร็จ ~50% — user 2026-09-16) ล้มเหลว = ได้คืน 1 เม็ด grade เดิม เสีย 5 · **Gem Crafter level** (user "+1% ตามเลเวล"): ทุก 10 fusion ที่รัน = +1 level (cap 30) แต่ละ level ลดโอกาสล้มเหลว 1% — คิดจาก `Measures.gem_fusions` ไม่มี state ใหม่ · **กฎเดียวกับ gear** (0.0.396, user "ควรทำเหมือนกันสองระบบ"): ตาราง fail + บันได crafter อยู่ที่ `loot_rules.fusion` บล็อกเดียว gear ใช้กับ `Measures.fusions` (Blacksmith) gem ใช้กับ `gem_fusions` · (3) ไม่ขายในร้าน | user "รวม 6→1 เหมือน gear" |
| **ใส่** | `SocketGem { target, slot, gem_def_id }` — ใส่ช่องที่เลือก · **ช่องมีของอยู่ = ทับ อันเก่าหาย** · ไม่มีค่าใส่ | user "อันเก่าหาย" |
| **ถอด** | ไม่มีปุ่มถอด (ทับอย่างเดียว) — ถ้าอยาก Gil sink ทีหลังเพิ่ม `UnsocketGem` จ่าย Gil gem หาย แบบ TBH | WoG |
| **bound / ตลาด** | ชิ้นที่มี gem **ขาย market ไม่ได้** จนกว่าจะ**ว่างทุกช่อง** — ไม่ bound ถาวร (`marketable` เช็ค `gems.is_empty()`) · vendor ได้เสมอ | user "ไม่ bound" + TBH "strip all sockets before selling" |
| **ผสม (fusion) ชิ้นที่มี gem** | ได้ — gem หายไปกับชิ้น (เตือนใน UI) | ไม่มีข้อมูล TBH · เลือกทางที่ไม่ล็อกของ |
| **เทรด gem** | ได้ใน **Steam market** (`steam_tradeable: true`, `gil_tradeable: false` — **ไม่มี Gil market แล้ว** user 2026-09-16) — gem ไม่ถูกแก้ต่อชิ้น จึงเป็น product สะอาดเสมอ · ราคา WoG ชี้ว่า Mana 6★ แพงสุด | user "gem trade ใน steam market ได้ด้วย" / "gil trade ไม่มีแล้ว" |
| **cap** | **cap ของ gem ต่อ stat ต่อตัว** รวมทุกชิ้น (`gems::cap_of`, โค้ด): Crit / Dodge / CDR / Move 15% · CritDmg / ATK% / HP% / Break DMG 50% · Charge / Break build-up 100% (กำแพงสูง คุมที่ต้นทาง base 30) · flat ไม่ cap — รวมค่าก่อนแล้วค่อยตัด (`for_each_gem_mod`) จึงได้ 1 บรรทัดต่อ stat เข้า `for_each_mod` · ตารางเช็ค §3.1 | §3 |

## 3. ชุด gem (ค่ากลางที่ Common → Legendary(step 6) = ×1.3⁶ ≈ ×4.8, หน่วย bp)

| หมวด | ชื่อ | stat 1 | stat 2 | บุคลิก |
|---|---|---|---|---|
| Weapon (Weapon, Shield) | **Strife** | ATK% 150 → 725 | CritDmg 225 → 1,087 | ตีแรง |
| Weapon | **Destruction** | ATK flat 5 → 25 | Crit 45 → 218 | คริ |
| Weapon | **Ruin** | BreakDmg 225 → 1,087 | IgnoreDEF 150 → 725 | ทะลุ |
| Armor (Helmet, Armor, Gloves, Shoes) | **Life** | HP% 150 → 725 | DEF flat 5 → 25 | อึด |
| Armor | **Fortitude** | HP flat 45 → 218 | HP regen flat 15 → 73 ต่อ 5 วิ | ฟื้นเอง |
| Armor | **Wind** | MoveSpeed 150 → 725 | Dodge 50 → 242 | หลบ/ไว |
| Accessory (Necklace, Ring, Earring, Bracer) | **Valor** | CDR 60 → 290 | Boss DMG 150 → 725 | ล่าบอส |
| Accessory | **Charge** | ChargeRate 150 → 725 | BreakBuildup 150 → 725 | เกจ |
| Accessory | **Arcana of Twilight** | Holy DMG 150 → 725 | Shadow DMG 150 → 725 | Lucielle · Noire |
| Accessory | **Arcana of Fervor** | Fire DMG 150 → 725 | Physical DMG 150 → 725 | Lapis · Seraphine |
| Accessory | **Arcana of Storm** | Water DMG 150 → 725 | Thunder DMG 150 → 725 | Elfie · (Skadi) |

**11 ชุด (8 + Arcana 3 variant) ไม่มี (stat, ธาตุ) ซ้ำกันเลย** (user 2026-09-16) — Arcana แต่ละ variant = 2 บรรทัด `ElementDmgBp` คู่ธาตุ (ทำไมเป็นคู่ไม่ใช่สุ่ม 1/6: ธาตุอยู่ที่ตัวละคร build อยู่ที่ปาร์ตี้ 5 คนกระจาย 6 ธาตุ เม็ด "ผิดธาตุ" ของคนหนึ่งคือเม็ดถูกของอีกคน ไม่มีเม็ดทิ้ง) · แต่ละ variant ดรอป 1/3 ของชุดอื่น (weight 1 : 3) · Heal% ตัดออก (niche) · `GemStat.element` ใช้กับ key `element_dmg` เท่านั้น · fold รวมต่อธาตุ cap 50% ต่อธาตุ — guard `catalog_hygiene` บังคับ · ชุดสากล (Universal) ตัดออก · key ที่เพิ่มเพื่อระบบนี้ (มีผู้อ่านใน combat ทุกตัว): `heal` · `ignore_def` · `element_dmg` (= `ElementDmgBp` รายธาตุ) · `boss_dmg` (`DmgVsBossBp` ใหม่ 0.0.391) · `hp_regen` (`HpRegenFlat` ใหม่ 0.0.391 — flat ต่อ 5 วิ ใน `tick/upkeep`; kit ใหม่ไม่มี passive regen จึงเป็นการฟื้นนอกฮีลทางเดียว) · `cast_speed` ตัด (kit ใหม่ไม่มี cast time; tempo = Haste/CDR)

รอ implement (user 2026-09-16): **restore on hit** (TBH) · ฟื้นระหว่างด่าน % (เสียบตอนทำ stage ladder ใหม่: heal 20% + stat) · โอกาสดรอป gear/gem (faucet — ต้องคิดกับ crowd-scaling ก่อน) · +Gold/+XP (รอ Gil ต่อ kill / pacing)

ค่าจริงที่ใช้: base × `Grade::raw_bp()` (×1.3/ขั้น ⇒ step 6 Legendary ≈ ×4.83 ไม่ใช่ ×7.5 ที่ตารางเดิมเดา — ตารางบนคือค่าเป้า ค่าที่ sim ตอบอยู่ใน query `gem_rules.sets[].stats[].mid_by_grade`) · ค่า flat ปัดขึ้นเหมือน ATK ของ gear

**Abundance (Gil/XP gain%) ตัดออกจากชุดแรก**: มอนไม่จ่าย Gil ต่อตัว (`gil_reward` = 0 ทุกตัว — Gil มาจากขาย junk) บรรทัด Gil gain% จึงไม่มีที่ทำงาน และ XP gain% ต้องรอ pacing จบ — ใส่คืนได้ด้วย key ใหม่ใน `SubstatKey` + set ใน JSON เมื่อมี faucet ให้มันคูณจริง

ยอดรวมชุดเต็ม (11 ชิ้น × 3 ช่อง Legendary × gem Legendary ทุกเม็ดเป็น Valor/Destruction): Crit สูงสุด ≈ +1.45% × 18 = **26%** — เกินเป้า 20% ⇒ **cap รวมของ gem ต่อ stat** (`gems::cap_of`: crit 1,500 · dodge 1,500 · cdr 1,500 · move 1,500 · critdmg 5,000 · atk_pct 5,000 · hp_pct 5,000) — เข้าใจง่าย "gem ให้ crit ได้สูงสุด 15%"

### 3.1 เช็ค cap รวม — ชุดเต็ม 11 ชิ้น × 2 ช่อง ที่เพดาน launch (step 6 = "Legendary" ในเกม, 22 ช่อง, base ×1.5) gem grade เดียวกันทุกเม็ด — ยอดเท่าตารางนี้

| stat | ทางที่ได้สูงสุด | รวมก่อน cap | cap gem | + substat ชุดเต็ม (s7 step 6) | cap ที่จุดใช้ (โค้ดเดิม) |
|---|---|---|---|---|---|
| Crit | Destruction 6 + Valor 12 × 1.45% | 26.1% | **15%** | 14.6% ⇒ ~30% | ไม่มี |
| CritDmg | Strife/Ruin 6 + Wildness 12 × 7.25% | 130% | **50%** | 35% ⇒ 85% | ไม่มี (×1.5 base) |
| ATK% | Strife 6 × 4.83% | 29% | 50% (ไม่ชน) | 28% ⇒ 57% | ไม่มี |
| HP% | Life 15 + Wildness 12 × 4.83% | 130% | **50%** | 42% ⇒ 92% | ไม่มี |
| Dodge | Wind 8 × 2.42% (ชุดเดียว) | 19% (additive) | **15%** | fold 14% ⇒ ~27% | fold ไม่มี cap |
| Move | Wind 15 × 4.83% | 72% | **15%** | 24% ⇒ 39% → **30%** | `MoveSpeedBp` cap 30% |
| CDR | Valor 12 × 1.94% | 23% | **15%** | 18% ⇒ 33% → **30%** | `CooldownBp` cap 30% — **แต่ไม่มีใครอ่าน** (§7) |
| Charge / Break build-up | Charge 33 × 1.45% (universal ทุกช่อง, base ลดเหลือ 30) | **48%** | **100%** (กำแพงสูง ให้ถึงยาก — user) | — | ไม่มี |
| Break DMG | Ruin 6 × 7.25% | 43% | 50% | — | ไม่มี |
| ATK flat | Vigor 33 × 15 | 495 | ไม่ cap | อาวุธ s7 step 6 = 7,361 ⇒ เศษ | — |
| DEF flat | Fortitude 15 × 20 + Steel 12 × 15 | 480 | ไม่ cap | เกราะ s7 step 6 = 5,271 ⇒ เศษ | — |
| HP flat | Vigor/Fortitude 33 × 145 | 4,785 | ไม่ cap | — ต้องเทียบ HP Lv70 (ยังไม่วัด) | — |

สรุป: ค่า flat ทุกตัวไม่ต้อง cap · ค่า % ชน cap ของ gem ก่อนทุกตัวยกเว้น ATK% / Break DMG / Charge+Break build-up (กำแพง 100% ถึงได้เฉพาะ Ancient+ 44 ช่องแบบ min-max) · ยอด gem + substat รวมสูงสุด ~30% crit / 85% critdmg / 92% HP% — ตัวเลขนี้คือ "ผู้เล่นที่ใส่ Legendary ครบ 33 เม็ดแบบ min-max" ไม่ใช่ median

### 3.2 โอกาสดรอปเทียบ gear (catalog 0.0.135)

| | gear | gem |
|---|---|---|
| ต่อ kill ในสนาม | 10 ช่อง × 12 bp = **1.20%** | **0.20%** (≈ 17% ของ gear — user "10-20%") |
| ต่อ boss | 11 × 24 = 2.64% | 0.40% |
| ต่อชุด/ช่อง | 0.12% ต่อช่อง (แบ่งตาม def ในช่อง) | ชุดหมวด 0.021% (weight 3 / 29) · ชุดสากล 0.007% (weight 1) |
| grade | `grade_odds` ของมอน | ตารางเดียวกัน (1 draw เมื่อออกเป็น gem) |
| 48h seed 42 (1,514 kill/h) | 966 ชิ้น (20.1/h) | ≈ 180 เม็ด (3.7/h ≈ 19% ของ gear เพราะ boss จ่ายเท่าตัว) — ใส่ 51, fuse 25 ครั้ง, ใส่แล้ว Common 13 · Uncommon 5 · Rare 2 |

(รอบแรก 60/120 bp ได้ 457 เม็ด = 45% ของ gear — ลดเป็น 20/40 ตาม user 2026-09-16)

### 3.3 ช่องต่อ grade (ทุกช่องสวมใส่)

| Tier (step) | gem | การ์ด |
|---|---|---|
| 1 Common (0) | 0 | 0 |
| 2 Fine (1) | 1 | 0 |
| 3 Rare (2) | 1 | 1 |
| 4-5 Elite/Epic (3-4) | 2 | 1 |
| 6-7 Royal/Legendary (5-6) | 2 | 2 |
| 8-10 Ancient+ (7-9) | 3 | 2 |

ชุดเต็ม 10 ชิ้นที่เพดาน launch = **20 ช่อง gem** (เดิมออกแบบ 33; ตาราง §3.1 คิดที่ 11 ชิ้น — เกราะเหลือ 4 ⇒ Life/Fortitude/Wind สูงสุด 8 เม็ด ไม่ใช่ 10, ยอดต่ำลงเล็กน้อย) — base ของทุกชุด ×1.5 ให้ยอดรวมชุดเต็มเท่าตารางเช็ค §3.1

## 4. โครงข้อมูล (ตามที่สร้าง 0.0.386)

```
Item.gems: Vec<SocketedGem>            // { def_id, lines: Vec<StatMod> }  บรรทัดที่สุ่มแล้ว (serde default, skip empty)
Item.gem_set: String                   // บน def ของ gem เท่านั้น — ให้ drop path re-key grade ได้โดยไม่มี catalog
ItemDef.gem: Some(GemSpec { set, grade })   // gem แต่ละ (set × grade) เป็น def ของตัวเอง `gem_<set>_g<step>`
                                       // 11 ชุด × 10 = 110 def, num 420-529, kind Material, ledger, gil+steam tradeable
loot_rules.gems: {
  sets: [ { id: "strife", group: "weapon", weight: 3, stats: [ {key: "atk_pct", base: 100}, {key: "crit_dmg", base: 150} ] }, … ],
  roll_window_bp: 500,     // ±5%
  merge: 6,
  field_bp: 20, boss_bp: 40    // โอกาสต่อ kill ที่ได้ gem สักเม็ด (≈15% ของ gear) แบ่งตาม weight ของชุด
}
โค้ด (ไม่ใช่ JSON — เป็นรูปบันไดเหมือน GRADE_RAW_BP): gems::GEM_SLOTS_BY_GRADE = [0,1,1,2,2,2,2,3,3,3] · CARD_SLOTS_BY_GRADE = [0,0,1,1,1,2,2,2,2,2] · gems::cap_of(key)
```
- `Item::gem_slots()` = `GEM_SLOTS_BY_GRADE[grade]` · `Item::socket_count()` (การ์ด) = `CARD_SLOTS_BY_GRADE[grade]` · ไม่ใช่ gear = 0 · `Item.card_slots` เป็น field เก่า (migrate เป็น 0)
- บรรทัดของ gem เข้า `for_each_mod` ผ่าน `gems::for_each_gem_mod(entity)` — รวมต่อ stat ทุกชิ้น → clamp `cap_of` → 1 บรรทัดต่อ stat
- `Item::market_clean()` = `gems.is_empty() && !bound` — กติกาที่ item market (ยังไม่สร้าง) ต้องเช็คก่อนลง listing
- ดรอป: ตาราง drop ของทุกมอนได้แถว gem 1 แถวต่อชุดที่ def Common (`build_drop_table` กลุ่มเดียวกับ per-slot gear) · ตอนดรอปสุ่ม grade ด้วย `grade_odds` ของมอนตัวนั้น (1 draw เฉพาะเมื่อของที่ออกเป็น gem) แล้ว re-key def → เข้า ledger
- migrate: `trim_to_sockets` ตัด gem เกินช่องเมื่อตาราง/การ์ดเปลี่ยน · `Item.bonuses` ไม่ใช้ (legacy ว่าง)

## 5. Events (สร้างแล้ว, Client authority ทั้งคู่)
`SocketGem { target: GemTarget::Inventory{idx} | Equipped{entity_id, equipped_slot}, socket: u8, gem: String }` — ใส่ช่องถัดไปที่ว่างหรือทับช่องที่มีของ (`socket ≤ gems.len()`), 1 draw ต่อ stat, ปฏิเสธ = ไม่ draw (`gems::refusal` บอกเหตุผล) · `FuseGems { gems: Vec<String> }` — 6 def grade เดียวกัน (ชุดใดก็ได้ ซ้ำได้) → 1 draw เลือกชุดตาม weight → credit `gem_<set>_g<step+1>`; เพดาน `FUSE_GRADE_CEILING` เดียวกับ gear · `Measures.gems_socketed` / `gem_fusions`

## 6. Host / client / วัด (สร้างแล้ว)
- query `gem_rules` → `gem_slots_by_grade`, `card_slots_by_grade`, `fail_bp_by_grade`, `fail_bp_now_by_grade` (หลังหัก crafter), `crafter_level` / `crafter_level_cap` / `crafter_fusions_per_level` / `crafter_bp_per_level` / `fusions_run`, `roll_window_bp`, `merge`, `ceiling_step`, `sets[] { id, group, weight, defs[10], stats[] { key, mid_by_grade[10], cap } }`
- query `gem_plan:inv:<idx>` / `gem_plan:eq:<entity_id>,<worn_idx>` → `gem_slots, card_slots, next_socket, socketed[] { def_id, name, grade_step, lines[] }, held[] { def_id, count, set, grade_step, name, fits, refusal, lines[] { key, lo, hi } }`
- item view: `gem_slots`, `gem_lines: string[][]`, `market_clean`; `gems` serialize ตรงเป็น tagged enum · `materials` แสดง gem เป็นแถวมี `grade`/`grade_step` (สี/Tier label ใช้ของ gear ได้เลย)
- `sim-cli --gear-report`: auto `auto_fuse_gems` + `auto_socket_gems` (greedy: grade สูงสุดที่ใส่ได้ ชุดตรงหมวดก่อน universal, ไม่ทับ) · ส่วน "GEMS" = ถือ/ใส่ตาม grade + บรรทัดรวมต่อคน
- guard `catalog_hygiene::the_gem_table_covers_every_set_at_every_grade` · unit test 4 ตัวใน `gems.rs` · host test `gem_rules_and_gem_plan_answer_from_the_sims_table`

ผล 48h seed 42 (catalog 0.0.137, 22 ช่อง): socket 91 เม็ด fuse 9 ครั้ง, ใส่แล้ว Common 37 · Uncommon 9 · Rare 1 · เหลือช่องว่าง 1 — รอบก่อน (0.0.135): socket 55 เม็ด, fuse 79 ครั้ง, ใส่แล้ว Common 6 · Uncommon 10 · Rare 6 · Legendary 2 · Immortal 1 — บรรทัดรวมต่อคน HP +61 flat/+1.7% · DEF +11 · CritDmg +3.3% · ATK% +0.8% (ใกล้เคียง substat ของชุดเดียวกันที่ 48h) · ยังไม่จูน

## 7. ที่ยังไม่ตัดสิน
- ✅ **CDR ต่อสายครบทุก cooldown แล้ว (0.0.387)** — เดิม `stat_mods::scale_cooldown` ใช้เฉพาะ cast path มาตรฐาน (heal + generic) ส่วน Dupla Medicina / Rivus / Subvenio / Taunt / Flame Shield arm ด้วย `cooldown_ticks` ดิบ; ตอนนี้ทุกจุดผ่าน `scale_cooldown` (user เลือก ก) · stat อื่นทุกตัวใน gem/substat มีผู้อ่านจริง (ตรวจ 2026-09-16: move → `tick/movement`, dodge → `combat/targeting`+`mod`, crit dmg / break ×2 → `combat/damage`, charge → `skills/effects`, flat ATK/DEF/HP → `combat/derived`)
- `field_bp` 20 / `boss_bp` 40 (≈15% ของ gear) และ weight universal 1:3 — วัดแล้ว 1 รอบ ยังจูนต่อได้ · จูนแล้ววัดด้วย `--gear-report` (ส่วน GEMS)
- Abundance (Gil/XP gain%) — รอ faucet ต่อ kill / pacing (§3)
- `UnsocketGem` จ่าย Gil (Gil sink แบบ TBH) — ยังไม่ทำ
- ตาราง gem ต่อ set × grade เป็น CSV (`--gem-table`) เหมือน substat — ยังไม่ทำ

## 8. ฝาก session Unity (floating UI) — สถานะ sim 0.0.392 / catalog 0.0.143 (2026-09-16)

**สิ่งที่เปลี่ยนไปจาก UI ที่มีอยู่ (ต้องแก้ก่อน)**
- ช่องสวมใส่เหลือ **10** (Pants ตัดออก 0.0.389) — PaperDoll / ExemplarEquipmentPage / CardsWindow ถอดแถว Pants แล้ว (commit facb34eb) `UI_EXEMPLAR_SLOT_AURA` ไม่ถูกใช้แล้ว
- ช่องการ์ด**ตาม grade** ไม่ใช่ตาม def (0.0.388): `socket_count` ใน item view ยังเป็นตัวเลขเดิม ใช้ต่อได้ · ของ `_1s`/`_2s` ไม่มีแล้ว ชื่อ "[1]"/"[2]" ถ้ามีในโค้ด UI ให้ตัด
- `card_slots` ใน item view = 0 เสมอ (field เก่า) อย่าอ่าน ใช้ `socket_count`

**ข้อมูลจาก sim**
- `gem_rules` → `gem_slots_by_grade[10]`, `card_slots_by_grade[10]`, `roll_window_bp`, `merge` (6), `ceiling_step`, `sets[] { id, group (weapon|armor|accessory), weight, defs[10], stats[] { key, element?, mid_by_grade[10], cap } }`
- `gem_plan:inv:<idx>` / `gem_plan:eq:<entity_id>,<worn_idx>` → `gem_slots, card_slots, next_socket, socketed[] { def_id, name, grade_step, lines[] }, held[] { def_id, count, set, grade_step, name, fits, refusal, lines[] { key, lo, hi } }`
- item view (ทุกชิ้น): `gem_slots`, `gem_lines: string[][]`, `market_clean`, `gems` (raw) · `materials` มีแถว gem พร้อม `grade_step` / `gem_set`
- events: `SocketGem { target: {inventory:{idx}} | {equipped:{entity_id, equipped_slot}}, socket, gem }` · `FuseGems { gems: [6 def ids grade เดียวกัน] }`

**หน้าที่ต้องทำ**
- หน้าใส่ gem: เลือกชิ้น (bag / paper doll) → `gem_plan:…` → ช่อง gem (`gem_slots`) กับช่องการ์ด (`card_slots`) แยกกัน · gem ที่ใส่แล้ว (`socketed[].lines`) · gem ที่ถือ (`held[]` — `fits=false` แสดง `refusal` เทา) → `SocketGem` (เตือน "ทับแล้ว gem เดิมหาย" เมื่อ `socket < next_socket`)
- หน้า fuse gem: `materials` ที่ `gem_set` ≠ "" จัดกลุ่มตาม `grade_step` → เลือก 6 → `FuseGems` · ผลลัพธ์สุ่มชุดตาม weight (Arcana หายาก 1/3) บอกผู้เล่นก่อน
- Fusion Forge ของ gear: ชิ้นที่ `gems` ไม่ว่าง เตือน "gem จะหายไปกับชิ้น"
- tooltip gear: `gem_lines` ต่อจาก `authored_lines` + ช่องว่าง `gem_slots − gems.len()`
- ชื่อ gem: loc `ITEM_gem_<set>_g<step>` มีครบ 110 แถวแล้ว ("Strife Gem", "Arcana of Twilight" …) นำหน้าด้วย `GameText.GradeLabel(grade_step)` เหมือน gear · สี = grade
- ยังไม่มี: `UnsocketGem` (ถอด) — ไม่ต้องทำปุ่ม
