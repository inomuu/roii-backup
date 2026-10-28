# 📦 Item System — AS IMPLEMENTED

> **v0.0.138-alpha · 2026-07-05** — เอกสารนี้บรรยาย**เฉพาะสิ่งที่มีอยู่จริงในโค้ด/data ตอนนี้** ไม่ใช่แผน
> ของที่ design แล้วแต่ยังไม่สร้าง (ดาว / enchant / orb / ตลาด / เรือ) → [§10](#10-ยังไม่-implement-design-เสร็จ--อ่านที่-spec)
>
> อ้างอิงโค้ด: `crates/sim-core/src/{items,drops,refine,state}.rs`, `combat/derived.rs`, `data/game_data.json`

**สารบัญ**

1. [โครงสร้าง Item](#1-โครงสร้าง-item-ฟิลด์ที่มีผลจริง) · 2. [Gear ในเกม](#2-gear-ในเกม-data) · 3. [Socket](#3-socket-รูการ์ด--ฝังใน-def-เสมอ) · 4. [ตีบวก](#4-ตีบวก-refine) · 5. [การ์ด](#5-การ์ด-ระบบครบวงจรแล้ว) · 6. [Drop → Storage](#6-drop--storage-เส้นทางของทุกชิ้น) · 7. [Consumables](#7-consumables) · 8. [ราคา/การขาย](#8-ราคาการขาย) · 9. [ผลต่อ combat](#9-ผลของไอเทมต่อ-combat-ที่-implement) · 10. [ยังไม่ implement](#10-ยังไม่-implement-design-เสร็จ--อ่านที่-spec)

---

## 1. โครงสร้าง Item (ฟิลด์ที่มีผลจริง)

ไอเทมหนึ่งชิ้น (`Item`) มี:
- `def_id` + `kind`: **Weapon / Armor / Material / Card / Consumable**
- `atk_bonus / matk_bonus / def_bonus` — สเตตฐานจาก def
- `rarity`: **Common / Uncommon / Rare / Mythical / Legendary / Immortal**
  (6 ขั้น, ladder แบบ Dota 2 — 0.0.135) — rarity ของ gear **ตามดาว**:
  `max(rarity ของ def, rarity จากดาว)` โดย 1★ Uncommon · 2★ Rare (**เข้า
  rare bag อัตโนมัติที่ threshold ปกติ**) · 3★ Mythical · 4★ Legendary ·
  5★ Immortal (Arcane เก็บไว้เป็นขั้นสูงกว่าในอนาคต ตามลำดับ Dota); def variant `[1]` = Uncommon, `[2]` = Rare ตั้งแต่ใน data
- **Item Level (0.0.136)** — ป้าย `_sN` ของ def = iLv (โชว์ใน tooltip
  "Item Lv. N"); **level requirement ใส่ของ = (iLv−1)×10**: s1=Lv1 (starter
  ไม่โดน gate), s2=Lv10 … s7=Lv60 — ต่ำกว่าเลเวลที่ tier นั้นดรอปจริง
  ~15-20 lv (s2 ดรอป b4≈Lv25, s3 b6≈Lv40, s4 b10≈Lv65) → คนฟาร์มเอง
  ไม่เคยติด, gate ไว้คุม twink ผ่านตลาด; ใส่ไม่ถึงเลเวล = silent no-op
- `unique` (0.0.135) — **แท็กแยกจาก rarity** ("Legendary is not unique"):
  ของ authored ตายตัว (`unique_*` 7 ชิ้น) ไม่ roll ดาวตอนดรอป; ส่วน gear
  rarity Legendary ธรรมดา (4★ jackpot) roll ได้ปกติ
- `quantity` — ของ stackable (Material/Card/Consumable); gear ไม่ stack
- `refine` — ระดับตีบวก (ดู §4)
- `equip_slot` — 11 ช่อง: HeadTop/Mid/Low, Armor, Weapon, **WeaponLeft**
  (มือรองของ dual wield), Shield, Garment, Shoes, Accessory1, Accessory2
- `weapon_type`: Sword1H / Sword2H / Dagger / Bow / Staff / Mace
  (ไม่ gate การใส่แล้ว 0.0.366 — ทุก unit ใส่ได้ทุกชนิด; weapon_type เหลือไว้
  ให้ effect/VFX ที่อ่านชนิดอาวุธ)
- `atk_speed_bp` — ATK SPEED เป็น bp (500 = +5%), เดิมชื่อ `aspd_base` ที่เป็น ASPD
  point ติดลบ; 0 บนของธรรมดา, ตอนนี้มีแต่ unique 5 ชิ้นที่ไม่เป็น 0
- `card_slots` — จำนวนรูการ์ด (**ฝังใน def** — ดู §3)
- `star` — สองความหมาย: บนการ์ด = ระดับ fuse (ระบบยังไม่มา) / **บน gear =
  จำนวนช่อง bonus, ROLL ตอนดรอปแล้ว (0.0.132, ตาราง bp 0.0.135)**:
  มอนทั่วไป 75/18/6/0.99/**0.01**% · boss 40/30/20/9.9/**0.1**%
  (0-4★; **4★ = chase drop โคตรหายาก** — trash 1/10,000 ชิ้น gear, boss
  1/1,000; 5★ = บ่มอย่างเดียว ไม่ดรอป; ของ `unique` ไม่ roll — ตายตัวตาม
  def; ของที่ดรอปก่อน 0.0.132 = 0★ ถาวร ไม่ retro-roll เพราะ migration
  ห้ามแตะ PRNG)
- ฟิลด์การ์ด: `card_slot` (class), `card_effect`, `consume_effect`,
  `foil_effect`, `foil`
- `bonuses: Vec<GearMod>` (0.0.134/0.0.136) — บรรทัด enchant: **ROLL ตอน
  ดรอปเต็มทุกช่องดาวแล้ว** (`gear_roll.rs` — pool 29 family ตาม §2 ของ
  design: gate ตาม slot อาวุธ=เจาะ/Heal%=staff+mace เท่านั้น/หัว=cast/accessory=CDR/เสื้อ+โล่=DR/
  ผ้าคลุม=กันไกล+สถานะ/รองเท้า=MS, tier window ตามป้าย `_sN` ของ def
  s1-4=T1 · s5-8=+T2 · s9+=+T3 ถ่วง 60:30:10, ห้าม family ซ้ำในใบ,
  MaxHP ×band — **กฎ roll ฉบับเต็ม: `gear-roll-rules.md`**) + effect layer
  ทำงานจริงทุก hook (ดู §9); tooltip โชว์
  บรรทัดแล้ว. ของติดดาวที่ดรอปก่อน 0.0.136 = ว่างถาวร (migration ห้าม
  แตะ PRNG); **Star Stones ครบชุด 0.0.138**: Chaos (เขย่าทั้งใบ, บรรทัดล็อค
  รอด) · หินสี Green→Red (อัพดาว 0★→5★ + roll บรรทัดใหม่ทันที) · Death
  (ทอยล็อคสุ่ม 1 บรรทัด ใช้ซ้ำย้ายได้ — `locked_slot` per-instance) · Shiny
  (ขัดเลขใน tier เดิม) — ใช้ผ่าน `UseOrb` (Gil fee ต่อครั้ง: 500/5000×ดาว/
  2000/1500 ×iLv), UI: คลิกหินใน inventory → คลิกของ; เรทดรอป: trash
  Chaos~3 Green~4 Blue~1.5 ต่อวัน / boss Purple .4 Yellow .14 Red .03
  Death .5 Shiny .3 ต่อวัน
- เศรษฐกิจ: `sell_value` (ขาย NPC), `buy_price` (มี = ซื้อจากร้านได้),
  ธง `marketable / emission_group / quota_class` — **ติดไว้เฉย ๆ ยังไม่มีระบบ
  ตลาดใช้มัน**

## 2. Gear ในเกม (data)

**15 สาย × 7 tier (s1-s7):**

| กลุ่ม | สาย | slot |
|---|---|---|
| อาวุธ | sword, dagger, greatsword, bow, rod, mace | Weapon (dagger ใส่มือรองได้เมื่อ dual wield) |
| ชุด | armor, shield, garment, shoes | ตาม slot |
| หัว | helm, mask, charm | HeadTop / Mid / Low |
| accessory | ring, pendant | **generic (0.0.133)**: ใส่ช่องไหนก็ได้ — แหวน 2 วง/จี้ 2 เส้นได้; ใส่แล้ว stamp ช่องบน instance (ล้างแบบ dual-wield) |

- tier สูง = สเตตสูง (อาวุธ ~+8 atk/tier — จงใจให้ gear เป็น economy ไม่ใช่
  แกน power หลัก); แหล่ง: ดรอปตามตาราง band + บางส่วนขายในร้าน NPC
- **Socket variants (0.0.130):** "<base>_1s" = [1 รู] 84 ตัว (12 สาย — ไม่มีหัว)
  ดรอปข้างทุกจุดที่ base ดรอปที่ ~**1/8 ของ base** / "<base>_2s" = [2 รู] 77 ตัว
  (ไม่มี shield [2] — panel โล่รับการ์ดได้ 1) ดรอปจาก**ตาราง boss เท่านั้น**
  รวม ~**1/220-340 ต่อการฆ่า boss** (retune 0.0.133 — boss ต้นเกมมาทุก ~1-2
  นาที ต้องคิดราคาตามความถี่จริง). variant = สเตตเท่า base เป๊ะ, sell ×1.5/×2
- **Uniques 7 ชิ้น** (unique_blade/greatblade/dagger/longbow/rod/mace/plate):
  Legendary, มี socket ติดตัว — **ยังไม่อยู่ใน drop table ไหนเลย** ซื้อได้จาก
  ร้าน (ช่องทางทดสอบ); การ re-author เป็น build-enabler = งานอนาคต
- demo items (demo_socket_*, demo_str_card) ยังอยู่ใน data — ของทดสอบ

## 3. Socket (รูการ์ด) — ฝังใน def เสมอ

- รูเป็นคุณสมบัติของ **item def** ("Sword s3 [1]" คือคนละไอเทมกับ "Sword s3")
  → ของสะอาดทุกใบเหมือนกัน 100% ไม่มี state ซ่อนต่อชิ้น
- เพดานตามชนิด (สเปค 2026-07-04): 1H 0-3 / 2H 0-4 / มือรอง **นับ**ได้สูงสุด 1
  (`min(card_slots,1)` — กฎการนับ ยังไม่ implement เพราะทุก variant เป็น [1])
  / โล่ 0-1 / อื่น 0-2 — **ตอนนี้ในเกมมีจริงแค่ [1] และ [2]**; [3]+[4] มากับ
  ยุค dungeon (ดู card-system.md)
- อาวุธมีกฎ RO: `weapon_max_card_slots(weapon_level)` — weapon Lv1 รับได้ถึง 4 รู

## 4. ตีบวก (Refine)

- กด NPC ตี: สำเร็จ = +1 (สูงสุด **+10**), ล้มเหลว = **ดาวน์เกรด -1**
  (ของไม่แตก — ตัดสินใจไว้แล้ว) — ตาราง odds แบบ RO (bp รวม 10000, มี test pin)
- ต้นทุนต่อครั้ง: **Gil 1000 × ระดับถัดไป** + แร่ — **Primocon** (อาวุธ,
  ดรอปจริง) / **Aegirilium** (เกราะ, ตอนนี้ขายร้านอย่างเดียว — ยังไม่ดรอป)
- ผล: อาวุธ +atk ต่อขั้น / เกราะ +def ต่อขั้น (`refine_def_bonus`)
- ยังไม่มี: milestone passive / เลเวลสกิลไอเทม (อยู่ใน design v2)

## 5. การ์ด (ระบบครบวงจรแล้ว)

**การ์ดที่ author แล้ว 7 ใบ:** Pinkling (Armor: atk5+str1) / Greenling (Armor:
hp200+vit1) / Yellowling (Armor: flee10+agi1) / Chomb (Weapon: atk4+crit2%) /
Toxic Chom (Garment: flee10) / Burst Chom (Accessory1: atk12/def−3 — drawback
ใบแรก) / Rechum (Shoes: hp_pct 300bp = +3% MaxHP ใบแรก)

**วงจรชีวิต:**
1. **ดรอป ~1/100k** จากมอนประจำ (0.0.131 — ≈ 0.01% ของ RO คลาสสิคในหน่วยเวลาจริง;
   convention: มอนดรอปการ์ดตัวเอง; elite ~1/61k, boss ~1/200 = ใบเดียวกัน rate-up)
   + **ตัวคูณ event/test**: `card_rate_bonus_bp` ใน state, ตั้งผ่าน recorded input
   `SetCardRateBonus` (sim-cli: `--card-rate-mult N`) — หลาย layer คูณกันได้ใน
   อนาคต (item-buff/path) — การ์ด -ling ดรอปตัวเองครบแล้ว (remap 0.0.136:
   pinkling family→card_pinkling, yellowling→card_yellowling)
2. **Foil roll**: การ์ดที่ดรอปมีโอกาส **5%** (500bp) เป็น foil → ได้
   `foil_effect` เพิ่ม; foil แยก stack จากใบธรรมดาเสมอ (`same_card_stack`
   เทียบ def_id+foil+star)
3. **เข้า Album** อัตโนมัติทุกทาง (drop/ซื้อ/ถอด) — **30 type-slot**
   (`CARD_ALBUM_BASE_CAP`, ฟิลด์ upgrade ได้), เต็มแล้วชนิดใหม่เข้า overflow
   policy; ถอดการ์ดจาก panel ตอน album เต็ม = no-op (การ์ดไม่มีวันหาย)
4. **Equip เข้า card panel ของตัวละคร** (ไม่ใช่เข้าไอเทม): panel ต่อ class —
   Weapon 4 / Shield 1 / อื่น 2; การ์ดอยู่กับตัวละครข้ามการเปลี่ยน gear
5. **Activation**: `active = min(จำนวนการ์ดใน panel class, รูรวมของ gear
   ที่ใส่ใน class นั้น)` — เกิน = นั่ง inactive (สีเทา) รอ gear รูเยอะขึ้น
6. **Consume ("eat")**: กินการ์ดจาก album เป็นสเตตถาวร — ครั้งที่ N กิน N ใบ
   (1/2/3), **cap 3 ครั้ง/ชนิด/ตัวละคร** — UI: right-click ในหน้า Cards [D]
- `star` มีฟิลด์แล้วแต่**ยังไม่มีระบบ fuse** (design พร้อมใน card-system.md)

## 6. Drop → Storage (เส้นทางของทุกชิ้น)

```
kill → roll_drop (ตาราง weighted ต่อมอน, มี "nothing" sentinel ~75-95%)
  ├─ Card → foil roll 5% → CARD ALBUM (30 ช่อง)
  ├─ Rare+ non-stackable + ≥ threshold ผู้เล่นตั้ง → RARE BAG (3 ช่อง, upgrade ได้)
  └─ อื่นทั้งหมด → STASH ("Inventory", 20 ช่อง + โบนัส path Hoard)
        stackable merge ไม่กินช่องใหม่ · เต็ม → overflow: Reclaimer path
        auto-sell % หรือหายทิ้ง
```
- drop เข้า stash ตรง (direct-grant) — **ตายยกทีมของก็ไม่หาย** (escrow เลิกใช้แล้ว
  เหลือ vestigial)
- ⚠️ ธง rare bag: ตอนนี้ของ Rare+ non-stackable ที่ดรอปจริงแทบไม่มี
  (uniques ยังไม่ seed) — ระบบพร้อมแต่ faucet ยังไม่เปิด

## 7. Consumables

- **Potion bar 3 ช่องต่อตัวละคร** — auto-use ตามเงื่อนไข (HP threshold ฯลฯ);
  hp/aspd potions ซื้อร้าน
- **Quiver ธาตุ** (Fire/Water/Earth/...) — โหลดเป็น ammo pool ของ archer,
  **autoswap ต่อเป้า เผาลูกเฉพาะตอนได้เปรียบธาตุ**; 10k นัด/500 Gil, ร้านอย่างเดียว
- **Loot items** (Max Potion เต็มหลอด, Field Ration +AllStat, Rejuvenating HoT,
  Phoenix Tail ชุบเพื่อน, Monster Lure ดูดสปอว์น, Accelerator ×2 trickle) —
  กลไกทำงานจริงครบ แต่**ยังขายร้านอย่างเดียว ไม่อยู่ใน drop table**
  (ค้างจากการตัด 3h-loop → ต้อง seed เป็น pure drop — งานค้าง)

## 8. ราคา/การขาย

- ขายชิ้นให้ NPC = `sell_value` / **Sell Junk** ปุ่มเดียวกวาด junk ทั้ง stash
- ซื้อร้าน: ของที่มี `buy_price` — เข้าตรง stash เสมอ (ไม่ผ่าน rare bag)
- ไม่มี player-to-player ใด ๆ ทั้งสิ้นในเวอร์ชันนี้

## 9. ผลของไอเทมต่อ combat (ที่ implement)

- atk/matk/def จาก gear + refine → สูตร renewal สองชั้น (hard DEF จาก gear =
  ลด% diminishing / soft DEF จาก VIT+AGI+Level = ลบแบน)
- `atk_speed_bp` ของทุกชิ้น + buff `AspdPct` / `AspdPotionPct` / `CardAspdStack`
  **บวกกันเป็น % ก้อนเดียว** แล้วคูณ base rate ของ unit ครั้งเดียว (0.0.367 —
  แบบ LoL/Dota, ไม่ทบต้น); slow เป็นตัวคูณท้ายสุด
- การ์ด active → `card_bonuses` รวมเข้า effective stats (บวกใน bucket แล้วคูณ
  ครั้งเดียว — additive, ไม่ทบต้น)
- Dual wield (Assassin): มือรอง = dagger, **−18% ATK SPEED** / 2H+shield ต้องมี
  mastery / โล่ **−10% ATK SPEED** (Sword&Shield Mastery ลบคืน 1 pt/level)

**Gear-mod effect layer (0.0.134 — hook ครบ 26 mods, `combat::gear_mods`):**

| ตระกูล | จุด hook | Cap |
|---|---|---|
| +STR/AGI/VIT/INT/DEX/LUK | `buffed_stats` (จุดเดียวกับการ์ด — cascade ครบ) | — |
| MaxHP แบน/% · ATK แบน/% · MATK แบน/% · HIT · FLEE · CRIT | derived (bucket เดียวกับ buff/การ์ด) | — |
| Dmg vs ธาตุ/race/size | คูณก่อน mitigation ทุกเส้น (melee/off-hand/ranged) | อิสระ |
| Ignore DEF / MDEF | stamp ลง packet ตอน build — เจาะ hard+soft / mdef | 50% |
| Resist ธาตุ (ฝั่งรับ) | หลัง element mult ใน `apply_damage` | 75% |
| DR vs race/size/ranged (ฝั่งรับ) | `apply_damage` | 50% รวม |
| CDR | คูลดาวน์เส้น cast มาตรฐาน | 30% |
| Cast −% | ทั้งก้อน windup, floor 1 tick | 50% |
| Move speed | bucket `MoveSpeedPct` เดิม | 30% |
| Heal +% | สูตร Renewal Heal ฝั่งผู้ร่าย | — |
| Status resist (Freeze/Poison/Blind/Stun) | **ตัด duration** ที่ `apply_buff` จุดกลาง — deterministic ไม่มี roll เพิ่ม | 75% |

- Entity มี **race/size** แล้ว (0.0.134): มอนจาก template←def strings
  (`race`/`size` ใน game_data — ตอนนี้มี Formless/Plant); **player = DemiHuman/
  Medium แบบ implicit** ผ่าน `gear_mods::race_of()` (ไม่ stamp ลง state —
  hash save เก่าไม่ขยับ)
- AoE/blast/field ที่สร้าง packet มือ (trap/explosion/DoT) = ค่า default
  (ไม่มี penetration, race Unknown) — ยังไม่ผูก gear ผู้วาง

## 10. ยังไม่ implement (design เสร็จ — อ่านที่ spec)

| ระบบ | spec |
|---|---|
| ~~ดาว (roll ตอนดรอป)~~ ✅ 0.0.132 / ~~bonus effect layer~~ ✅ 0.0.134 — เหลือ: **orb/ระบบ roll ค่า + วัสดุบ่มดาว 4-5★** | `gear-systems-design.md` §1-3 |
| ตีบวก milestone passive + เลเวลสกิลไอเทม + Skill Orb | ๆ §4-5 |
| Uniques เป็น build-keys (re-author 7 ชิ้น) | ๆ §7 |
| ตลาดทุกชั้น (Steam/Gil order book/แผงลอย) + เรือ + market-lock | ๆ §8 + `server-plan-phases.md` |
| [3]/[4] sockets + เจาะรู def-swap | `card-system.md` |
| star/fuse การ์ด, Epic rarity, drop-system 2-axis | `card-system.md`, `drop-system.md` |

**งานค้างระยะสั้นของระบบไอเทม:** ระบบ roll (หินเสก/เขย่า — คุยรอบหน้า) ·
seed loot items + Aegirilium + uniques เป็น pure drop · แก้การ์ด -ling ไขว้ตัว ·
rename `rechum`→`rechom`? · Album UI (a) · UI โชว์ bonus lines ใน tooltip
(effect layer มีแล้วแต่ tooltip ยังไม่พิมพ์บรรทัด mod)
