# Crests — disenchant sink (0.0.416-0.0.417, catalog 0.0.174)

> user 2026-09-17: "ฝากทำระบบนี้ โดยแยกของที่ disenchant อาวุธ / ชุด / เครื่องประดับ / เพชร → ได้ unique material ตาม tier ไม่เก็บในช่อง + แลกเปลี่ยนไม่ได้ ติดตัว → เอาไว้อัพ stat ตามชนิดไอเทม" · "อาวุธ T3-T7 ก็จะได้ mat 5 ชนิด × 4 = 20 unique material" · "20 unique material ต้องอัพเดท 20 stats ที่ต่างกัน" · "แย่งกัน (กับ fusion) ไม่เป็นไร เป็น player decision"
> ต้นแบบ: WoG Blacksmith dismantle T3+ → crest material ตาม tier/ประเภท (ตัวเลข WoG ไม่เผยแพร่)


> **0.0.433 (2026-09-18) — the table changed.** Every category's **T3 crest is an economy line** (XP / Gil / soul / drop rate, user's call), the element gem crests are gone (a T7 Shadow crest against a T3 Fire crest was not a fair board), and the four combat stats the user flagged as too generous were halved. Live table:
>
> | หมวด | T3 | T4 | T5 | T6 | T7 |
> |---|---|---|---|---|---|
> | อาวุธ | **Scholar** XP +1%/lv | Edge ATK +0.5% | Fang Crit DMG +1% | Piercer Ignore DEF +0.5% | Slayer Boss DMG +1% |
> | ชุด | **Merchant** Gil +1%/lv | Heart HP +0.5% | Plate DEF +0.5% | Shadowstep Dodge +0.3% | Bulwark DR vs ranged +0.5% |
> | เครื่องประดับ | **Spirit** Soul +1%/lv | Eye Crit +0.1% | Tempo CDR +0.1% | Surge Charge +0.5% | Fracture Break build-up +0.5% |
> | เพชร | **Fortune** Drop +0.5%/lv | Shatter Break DMG +1% | Mending HP Regen +0.2% | Stride Move +0.3% | Grace Heal +1% |
>
> All Lv30, 1 material/level. Economy lines are party-wide (`crest::xp_gain_bp` …).

## 1. ที่สร้าง (`crates/sim-core/src/crest.rs`)

| ขั้น | กติกา |
|---|---|
| Disenchant (`InputEvent::Disenchant{inv_idx}`) | ทำลายชิ้นในกระเป๋า → เครดิต `crest_mats["<category>_t<tier>"]` · **ไม่ใช่ไอเทม** (counter ใน `GameState`, ไม่กินช่อง, ไม่มีทาง market/mail) · unique ปฏิเสธ · bound รับ (เป็นการใช้ ไม่ใช่ขาย) |
| category | weapon = Weapon + Shield · armor = Helmet/Armor/Gloves/Shoes · accessory = Necklace/Ring/Earring/Bracer · gem = `gem_<set>_g<n>` |
| tier | gear = `item_level` (s1-s7) · gem = grade step + 1 · **ต่ำกว่า T3 ไม่ให้อะไร** (fodder ของ fusion) · สูงกว่า T7 = T7 ⇒ 4 × 5 = **20 material** |
| yield | **shards** (10 = 1 material, ไม่มี RNG — user "ได้ 10% เป็นตัวเลข"): gear `yield_by_grade[grade]` = 1/2/3/5/8/12/16/20/25/30 shard (Common 10 ชิ้น = 1 material, Rare ~3 ชิ้น) · gem 2 shard · × stack |
| Crest (`UpgradeCrest{crest_id}`) | 20 crest = 1 ต่อ material: level L ใช้ `mats_per_level × L` material (= ×10 shard) ของตัวเอง (L=1..30 รวม 465 material = 4,650 shard ≈ Common 46,500 ชิ้น / Immortal ~580 ชิ้น) · max 30 |
| stat | resolve → `crest_mods` → `Entity.crest_mods` ทุก tick → `for_each_mod` (แบบเดียวกับ training) |

## 2. 20 crest / 20 stat (per level × 30)

| category | T3 | T4 | T5 | T6 | T7 |
|---|---|---|---|---|---|
| weapon | ATK +0.5% | Crit DMG +1% | Ignore DEF +0.5% | Break DMG +1% | Boss DMG +1% |
| armor | HP +0.5% | DEF +0.5% | HP regen +2 | Dodge +0.3% | DR vs ranged +0.5% |
| accessory | Crit +0.2% | CDR +0.25% | Move +0.3% | Charge rate +1% | Break build-up +1% |
| gem | Fire DMG +1% | Water +1% | Thunder +1% | Holy +1% | Shadow +1% |

เต็ม 30: ATK +15%, Crit DMG +30%, Crit +6%, CDR +7.5% (cap 30), Dodge +9%, ธาตุ +30% (บวกกับ Attunement 30 + gem) — conservative ตามกติกา 5 คน

## 3. host / UI

- query `crest_rules` → `materials` (purse เป็น shard), `shards_per_material`, `crests[]` {id, category, tier, stat, element, level, max_level, material, held / held_materials, next_cost / next_cost_materials (shard / material), text, text_per_level}
- query `disenchant_preview:<inv_idx>` → {material, category, tier, yield} หรือ `null` (ทำไม่ได้)
- view: `crests`, `crest_mats` (serde อัตโนมัติ)
- loc: `CREST_<id>` 20 ชื่อ · `MAT_weapon/armor/accessory/gem` "… Essence T{0}" · `UI_CRESTS`, `UI_DISENCHANT`, `UI_DISENCHANT_YIELD`, `UI_CREST_UPGRADE`, `UI_CREST_MAXED`
- daily/weekly ยังไม่มี commission disenchant (counter `items_disenchanted`/`crest_levels` มีแล้ว)

## 4. ที่ยังไม่ทำ / เคาะ

- sim-cli จำลองแล้ว (0.0.420): `auto_disenchant_surplus` (T3+ ส่วนเกินก่อนทิ้ง) + `auto_crests` ทุกนาที · CSV `crest_lv,disenchanted`
- ตัวเลข yield / mats_per_level / max_level เป็นค่าเริ่ม ยังไม่วัด
- Unity: หน้า Crest (20 ช่อง จัดตาม category) + ปุ่ม Disenchant ในกระเป๋า (ข้าง Discard)
