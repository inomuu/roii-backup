# TBH — substat (inherent line) ของแต่ละไอเทม: ข้อมูลจริงจาก datamine

2026-09-15 · แหล่ง: `taskbarhero.wiki` datamine (Ver 1.2.x) — ดาวน์โหลดไว้ที่
`docs/references/tbh-datamine/` (JSON ดิบ 22 ตาราง + `gear_lines.csv` 5,760 แถว อ่านง่าย)
ต่อจาก `taskbar-hero-item-study.md` §1.6 ซึ่งเห็นแค่ตารางดาบจากไกด์ — ฉบับนี้เห็น**ทุกชิ้น**

## 1. ID scheme ของ TBH (คำตอบข้อ "id TBH ทั้งหมด")

item id = 6 หลัก **`TT G III`**

| ส่วน | ความหมาย | ค่า |
|---|---|---|
| `TT` | ชนิด | 30 Sword · 31 Bow · 32 Staff · 33 Scepter · 34 Crossbow · 35 Axe · 40 Shield · 41 Arrow · 42 Orb · 43 Tome · 44 Bolt · 45 Hatchet · 50 Helmet · 51 Armor · 52 Gloves · 53 Boots · 60 Amulet · 61 Earring · 62 Ring · 63 Bracer |
| `G` | grade | 0 Common … 9 Cosmic |
| `III` | ลำดับใน (ชนิด×grade) | 001-192 — Common ไล่ตาม level (001=Lv1, 002=Lv5 …); grade อื่นเป็นคู่ **011/012, 021/022 …** = ตัวแปร A/B ของ level เดียวกัน |

ตัวอย่าง Long Sword Lv1: 300001 (Common) · 301011/301012 (Uncommon A/B) · 302011/302012 (Rare A/B) · 303011/303012 · 304011/304012
ของกล่อง/วัสดุอยู่ช่วง 9xxxxx (Normal Monster Box 1 = 910011) · รวมทุกอย่าง 5,944 id · gear 5,760

**เทียบกับของเรา**: `ItemDef.num` (append-only) × 16 + grade step = itemdefid — โครงเดียวกัน (ชนิด/ชิ้น
แล้วค่อย grade) แต่ TBH ใส่ grade ไว้**กลาง** id และมี "ตัวแปร" อีกหลัก — ดู §5

## 2. โครงบรรทัด stat ของ 1 ชิ้น

`gear.json`: `BaseStat1/2` (main stat ตามชนิด — ดาบ = ATK + ASPD, เกราะ = Armor, โล่ = Block, Orb = CDR,
Tome = MaxHp) + **`InherentStat1..3`** แต่ละบรรทัด = `(STATTYPE, MODTYPE FLAT|ADDITIVE|MULTIPLICATIVE, Value)`
+ `UniqueModKey` (36 ชิ้นพิเศษ) — **ค่าคงที่ต่อ id ไม่มีการสุ่มตอนดรอป**

### จำนวนบรรทัดตาม grade (จาก `grades.json: InherentSlotAmount`) — ตรงกับ `LINES_BY_GRADE` ของเราเป๊ะ

| grade | Common | Uncommon | Rare | Legendary | Immortal | Arcana | Beyond | Celestial | Divine | Cosmic |
|---|---|---|---|---|---|---|---|---|---|---|
| inherent | **0** | 1 | 1 | 2 | 2 | 3 | 3 | 3 | 3 | 3 |
| ช่อง cube (ตกแต่ง/แกะสลัก/จารึก — ผู้เล่นทำเอง, สุ่มจาก `stat_mods` tier 1-10) | 0 | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 6 | 6 |

## 3. การค้นพบหลัก: **ทุก (ชิ้น × grade) เหนือ Common มี 2 ตัวแปร A/B** ที่บรรทัดต่างกัน

| | |
|---|---|
| Common | 1 id ต่อ (ชนิด, level) — 320 ชิ้น, 0 บรรทัด |
| Uncommon…Cosmic | **2 id** ต่อ (ชนิด, level, grade) — 2,720 คู่ — บรรทัดของ A กับ B เป็นคนละชุด |

นี่คือที่มาของชื่อใน Steam Market "Long Sword (Immortal) **A**" — ความหลากหลายมีแค่ 2 แบบ ถูกกำหนดตายตัว
และแต่ละแบบเป็นสินค้าของตัวเอง ⇒ **fungible ยังอยู่ แต่ผู้เล่นมี "เลือกได้นิดหน่อย"** ระหว่าง build

ตัวอย่าง Legendary Sword (2 บรรทัด):

| level | ชื่อ | A | B |
|---|---|---|---|
| 1 | Long Sword | ATK +3 · ASPD ×8.2% | ATK +21.5% · ATK +3 |
| 20 | Great Sword | ASPD ×9.0% · ATK +26.9% | ATK +7 · CritDmg +34.6% |
| 50 | Fate Sword | ATK +54.0% · Crit +1.3% | ATK +48 · CDR +6.3% |
| 80 | Dimensional Sword | ATK +80.5% · CritDmg +70.3% | ASPD ×17.1% · CDR +7.4% |

(ค่าในไฟล์เป็น ×10 ของ % ที่แสดง: `ADDITIVE 200` = +20.0%)

## 4. pool ของบรรทัดต่อชนิด — แคบ ผูกกับบทบาทของช่อง (นับจากทุกชิ้น)

| ชนิด | stat ที่ปรากฏ (มาก→น้อย) |
|---|---|
| Sword | ATK% · ATK flat · ASPD% · CritDmg · CDR · Crit · HP/hit |
| Bow | ATK flat · CritDmg · Crit · ASPD% · ATK% · CDR · HP/hit |
| Staff | ATK flat · CDR · CritDmg · ATK% · Crit · HP/hit · CastSpeed% |
| Shield | Armor · CDR · MaxHp · Block · DmgReduction · AoE% · Regen · AllRes |
| Helmet | MaxHp · Armor · Armor% · Regen · SkillDuration% · SkillRange% · Crit · CDR |
| Armor | MaxHp · MaxHp% · Regen · Armor% · AllRes · DmgReduction |
| Gloves | MaxHp · ASPD% · ATK flat · Armor · CritDmg · Regen |
| Boots | MoveSpeed · Armor · MaxHp · Regen · Armor% |
| Amulet | ASPD% · Crit% · CDR · AllRes · CastSpeed% · (หายาก) BaseAttackCount · +AllSkillLv · Projectile · Multistrike |
| Earring | CritDmg · HpLeech · SkillDuration% · ATK flat · MoveSpeed% · EXP% · AoE% |
| Ring | CDR · ASPD% · CritDmg · HP/kill · ATK flat · CastSpeed% · HP/hit · Crit% |
| Bracer | DmgReduction · Block · Regen · DmgAbsorb · AoE% · Dodge · SkillHeal% · MoveSpeed% · Armor% |

รวมทั้งเกม **28 stat type** บน inherent (ไม่ใช่ 7 ตามที่ study เก่าคาด — 7 คือของ*ดาบ*) แต่ **ต่อชนิดเห็นแค่ 6-9** และ
weapon ทุกชนิดใช้ชุดเดียวกัน (ATK/ATK%/ASPD/Crit/CritDmg/CDR/HP-per-hit) ต่างกันแค่น้ำหนัก

## 5. ค่าโตยังไง

- โต**ทั้ง level และ grade** แบบคูณ: ATK% ดาบ Uncommon Lv1 = 20% → Lv90 = 75% · Immortal Lv90 = 107% · Cosmic Lv90 = **200% (cap)**
- มี **hard cap ต่อ stat**: ATK% 200 · CritDmg 150 · ASPD 35 · Crit 5 · MoveSpeed 25 · Armor 10,000 — grade บนที่ level สูงชนเพดานหมด (Divine/Cosmic Lv90 เท่ากัน)
- ค่าเดียวกันที่ (grade, level) เดียวกันต่างกันตาม*ชนิด* (ATK% บนดาบ ≠ บนขวาน) ⇒ มีตาราง range ต่อ (ชนิด × stat)
- **แกน %** โตช้ากว่าแกนดิบมาก (ASPD 8% → 35% = ×4.4 ทั้งบันได; Armor flat ×2,000) — ตรงกับที่เราแยก `pct_bp` / `raw_bp`

## 6. สิ่งที่เราควรเอา / ไม่เอา

| TBH | เรา (0.0.383) | ทำอะไร |
|---|---|---|
| บรรทัดผูกกับ id ไม่สุ่ม | `authored_lines` (def × grade, seed จาก hash) | ✅ มีแล้ว — แต่ควรเป็น**ตาราง** ไม่ใช่ hash |
| 0/1/1/2/2/3… ตาม grade | `LINES_BY_GRADE` เท่ากัน | ✅ |
| pool ต่อชนิด 6-9 จาก 28 | `gear_roll::allowed` 22 family ยุค RO (DmgVsRace/Size, ResistStatus, IgnoreDef…) | ❌ **ต้องล้าง** — หลายตัวไม่มีความหมายในระบบ stat ใหม่ |
| ค่าโตตาม level × grade + cap | ค่า = range ต่อ tier × `pct_bp` ของ grade, ไม่มี cap | ปรับ: เพิ่ม cap ต่อ stat |
| **ตัวแปร A/B** ต่อ (ชิ้น×grade) | ไม่มี — 1 ชุดต่อ (def×grade) | **ตัดสินใจ**: เพิ่ม A/B หรือไม่ — ต้นทุนคือ `num`/itemdef ×2 และชื่อสินค้าใน market ×2 |
| ช่อง cube สุ่ม (ผู้เล่นทำ, bound) | ตัดออกไปแล้ว (Star Stone/enchant 0.0.381) | — รอดีไซน์ใหม่ |

### stat ของเราที่ยังมีชีวิตใน combat (อ่านจาก `stat_mods.rs`) และคู่ TBH

| ของเรา | TBH | หมายเหตุ |
|---|---|---|
| `AtkFlat` / `AtkBp` | ATK flat / ATK% | ✅ ใช้ได้ทันที |
| `CritBp` / `CritDmgBp` | Crit / CritDmg | ✅ |
| `graded_atk_speed_bp` (main ของอาวุธ) | ASPD% | ตอนนี้เป็น main stat ไม่ใช่บรรทัด — TBH ก็ให้ ASPD ทั้งเป็น base และ inherent |
| `CooldownBp` | CDR | ✅ |
| `MaxHp` / `MaxHpBp` | MaxHp / MaxHp% | ✅ |
| `DefFlat` | Armor flat | ✅ (Armor% เรายังไม่มี bucket) |
| `MoveSpeedBp` / `DodgeBp` | MoveSpeed / Dodge | ✅ |
| `ResistElement` (ต่อธาตุ) | AllElementalResistance | ใกล้เคียง — เราไม่มี "ทุกธาตุ" ในบรรทัดเดียว |
| `ChargeRateBp`, `BreakBuildupBp`, `BreakDmgBp` | — | ของเราเอง (Limit Break / Break) — เก็บได้ |
| `CastBp`, `HealBp` | CastSpeed% / SkillHeal% | ✅ |
| `DmgVsRace/Size`, `DrVsRace/Size`, `DrVsRanged`, `ResistStatusBp`, `IgnoreDefBp`, `DmgVsElement` | — | **ยุค RO** — ไม่มีคู่ใน TBH และผูกกับ race/size/status ที่ระบบใหม่ไม่ได้ออกแบบ → ตัดจาก pool |
| — | HP per hit / HP per kill / Regen / Block / DmgReduction / DmgAbsorb / AoE / SkillDuration | เราไม่มี bucket — ถ้าอยากได้ต้องเพิ่มเป็นระบบใหม่ ไม่ใช่แค่บรรทัด |

## 7. ข้อเสนอ — **ทำแล้ว 0.0.384** (ข้อ 1-3; ข้อ 4 A/B ยังไม่ทำ)

`loot_rules.substats` ใน `data/game_data.json` · resolver `crates/sim-core/src/substats.rs` · ตารางที่ generate `docs/references/roi-substat-table.csv` (`cargo run -p sim-cli -- --substat-table --out …`) · guard `catalog_hygiene::the_substat_table_covers_every_slot_and_every_key_is_real`


1. **pool ใหม่ 10 stat** จากแถว ✅ ข้างบน: ATK flat · ATK% · Crit · CritDmg · CDR · MaxHp · MaxHp% · DEF flat · MoveSpeed · Dodge (+ ChargeRate/Break ถ้าจะให้เป็นเอกลักษณ์เรา) — ตัด RO 6 ตัวออกจาก `allowed`
2. **ตารางใน JSON** `loot_rules.substats`: pool ต่อช่อง + range ต่อ (stat × tier) + cap ต่อ stat + จำนวนบรรทัดต่อ grade — โค้ดเหลือแค่เลือก/คำนวณ
3. **generate `docs/references/roi-substat-table.csv`** (ทุก def × grade → บรรทัด) จาก `sim-cli` ให้ดู/diff ได้เหมือน `gear_lines.csv` ของ TBH
4. A/B: **ยังไม่ทำ** — ถ้าจะทำ ต้องตัดสินก่อน Steam mint เพราะเปลี่ยน `num` ×2

แหล่ง: [taskbarhero.wiki database](https://taskbarhero.wiki/database) (datamine) · [TBH Index](https://tbhindex.com/wiki/gear) · [tbhwiki.net gear](https://tbhwiki.net/en/gear/) · [taskbarhero.help](https://taskbarhero.help/database/)

## 8. Cap ของ TBH — สองชั้น (2026-09-15, ตอบ "stat พวกนี้ cap เท่าไหร่")

**ชั้นที่ 1 — cap ต่อบรรทัด (authoring)** อ่านจาก plateau ในดาต้า: ค่าสูงสุดที่บรรทัดหนึ่งพูดได้ และมีหลาย id นั่งอยู่ที่ค่านั้นพอดี (ค่าในไฟล์ ×10 ของ %)

| stat | cap ต่อบรรทัด | id ที่ชนเพดาน | ช่วงที่ grade launch ของเรา (Beyond Lv30-50) |
|---|---|---|---|
| Critical Chance (flat) | **5.0%** | 26 | 1.2-2.1% |
| Critical Chance (% ของฐาน, Amulet) | 60% | 7 | 21-31% |
| Critical Damage | **+150%** | 45 | +15-72% |
| Cooldown Reduction | **15%** | 11 | 3-7.5% |
| Dodge Chance (Bracer เท่านั้น, หายาก) | **15%** | 2 | 7-9% |
| Block Chance | 30% | 2 | 6-9% |
| Attack Speed (×) | 35% | 19 | 12.5-17.5% |
| Movement Speed | 25% | 12 | 6.7-10.8% |
| ATK % | **200%** | 35 | 27-84% |
| Armor flat / Armor % | 10,000 / 500% | 11 / 8 | — |
| MaxHp flat / % | 800 / 50% | 7 / 7 | — |

**ชั้นที่ 2 — cap รวมของตัวละคร** (จากไกด์ Steam + wiki mechanics; ไม่อยู่ในดาต้า เป็นโค้ด):

| stat | cap รวม |
|---|---|
| Cooldown Reduction · Dodge · Block · Resist แต่ละธาตุ | **75%** (default; มี cube mod `MaxDodgeChance` / `MaxBlockChance` / `MaxXResistance` +5-5.5% ต่อบรรทัด ไว้**ยกเพดาน**) |
| Critical Chance · Attack Speed · MaxHp | **ไม่มี cap** ("stack it as high as you can", crit ทะลุ 100% ได้) |
| Critical Damage | ไม่มี hard cap, มี soft DR หลัง +300% |
| สูตรรวม | `(base + ΣFlat) × (1 + ΣAdditive) × Π(1 + Multiplicative)` |

**สิ่งที่ต่างจากเรา** — TBH ปล่อยตัวเลขใหญ่ได้เพราะ (1) เพดานรวม 75% เป็นกำแพงจริง (2) crit/ASPD ไม่ cap แต่เป็นระบบ ARPG ที่ตั้งใจให้ stack ถึง 100%+ (3) ช่อง cube ที่ผู้เล่นทำ (0-6 ช่อง) เป็นตัวเติมส่วนใหญ่ ไม่ใช่ inherent

เทียบกับที่เราตั้ง (0.0.384 / catalog 0.0.134):

| | TBH ต่อบรรทัด @ Beyond Lv30-50 | เรา ต่อบรรทัด @ Beyond s3-s5 | TBH cap รวม | เรา cap รวม |
|---|---|---|---|---|
| Crit | 1.2-2.1% | ~1.7-3% (cap 4%) | ไม่มี | ไม่มี (ชุดเต็ม Beyond = 14.6%) |
| Dodge | 7-9% (Bracer) | ~3-5% (cap 8%) | 75% | fold inverse-mult (ชุดเต็ม 14%) |
| CDR | 3-7.5% | ~3-5% (cap 8%) | 75% | **30%** (`CAP_COOLDOWN_BP`) |
| Crit DMG | +15-72% | ~7-11% (cap 15%) | soft 300% | ไม่มี (ชุดเต็ม 35%) |
| Move | 6.7-10.8% | ~7-10% (cap 15%) | ? | 30% |

⇒ ต่อบรรทัดเราอยู่ในย่านเดียวกับ TBH ทุกตัว ยกเว้น **Crit DMG ที่เราให้น้อยกว่ามาก** (TBH ให้ถึง +150% ต่อบรรทัด แต่ของเขาไม่มีอย่างอื่นให้ crit dmg มาก) — เก็บไว้ต่ำก่อนตามงบ §2.6 · และเรา**ไม่มี cap รวม**ของ crit/critdmg ซึ่ง TBH ก็ไม่มี ต่างกันแค่ของเขามี 75% กั้น dodge/CDR ส่วนเราใช้ fold + 30%

## 9. ค้าง: BLOCK (user 2026-09-15 "ค่อยทำ ก่อน")

เรา**ยังไม่มี block** ในสิมเลย — ไม่มี stat, ไม่มี roll, ไม่มี bucket · doc นี้และ `gear-fusion…` §2.5 เอ่ยชื่อไว้เฉยๆ
ตอนนี้โล่ = DEF 0.60 ของงบเกราะ + lines จาก pool `def / hp / dodge / hp_pct`

ถ้าทำ = ระบบใหม่ ไม่ใช่บรรทัด: `StatMod::BlockBp` → roll ตอนโดนตี (**draw เพิ่ม → goldens ขยับ**) → ผล (ลด x% / กันหมด)
→ cap รวม (TBH 75%) → main stat ของโล่เป็น block แทน DEF ตาม TBH (`SHIELD: BaseStat1 = BlockChance`)
→ เพิ่ม `block` เข้า `SubstatKey` + pool ของ Shield/Bracer → UI · ต้องตัดสินก่อน: โล่ = block (TBH) หรือ = DEF ก้อนใหญ่ (ตอนนี้)
