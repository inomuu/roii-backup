# Training (ฝึกฝน) — แผน implement (คู่ขนานกับ Paths of Soul)

> ศึกษาต้นแบบใน `docs/wog-training-study.md` · user 2026-09-17: "วางแผน implement training เอาไว้คู่ขนานกับ soul"
> สถานะ: **สร้างแล้วฝั่ง sim (0.0.406 / catalog 0.0.158, 2026-09-17)** — `crates/sim-core/src/training.rs`, `InputEvent::Train`, host query `training_rules`, view key `training`, sim-cli `auto_train` + CSV `train_lv,gil_spent`, loc `TRAIN_*`/`UI_TRAIN_*` · Unity window ยังไม่ทำ · ตัวเลขเป็นค่าเริ่มสำหรับวัด
> **conservative (user)**: "ตัวละคร 5 ตัว ของเค้ามีตัวเดียว" — % ทุกตัวคูณ 5 คน ⇒ ค่าต่อเลเวล = ครึ่งของ WoG (+1% ไม่ใช่ +2%), Precision +0.5% ×15, Attune/Ward +1% ×30

## 1. ที่ยืนในระบบ — สองชั้น meta ระดับปาร์ตี้ คนละสกุลเงิน

| | Paths of Soul (มีแล้ว, `soul_paths.rs`) | Training (ใหม่) |
|---|---|---|
| สกุลเงิน | Soul (ได้ต่อ kill, รอดจาก wipe) | **Gil** (sink อย่างเดียว — ไม่มี Gil market แล้ว) |
| รูปทรง | web 2D เดินออกจาก origin, allocate-once | **บันไดต่อช่อง** level 0→max, ซื้อทีละ 1 |
| ขอบเขต | ทั้งปาร์ตี้ | ทั้งปาร์ตี้ (`GameState` ไม่ใช่ `CharacterData` — สลับตัว/gacha ไม่รีเซ็ต) |
| การตัดสินใจ | "ไปทางไหน" (โครงสร้าง) | "ช่องไหนคุ้มที่สุดตอนนี้" (marginal value) |
| ราคา | คงที่ต่อ node | geometric ที่ exponent ลดลง (WoG) |
| RNG | ไม่มี | ไม่มี |
| ใช้กับ stat ยังไง | aura ต่อ tick (`apply_soul_path_auras`, BuffKind) | `StatMod` ต่อ tick (ดู §4) |

สองชั้นไม่รวมกัน: Soul ตอบ "ทิศทาง build", Gil ตอบ "ทิ้งเงินได้ไม่มีก้น" ทั้งคู่ต้องอยู่ต่อจาก Apex (stat point จาก level, ฟรี, จำกัด) ซึ่งเป็น `StatMod` ต่อตัวละครอยู่แล้ว

## 2. ช่องรอบแรก (12) → `StatMod` ที่มีอยู่

| ช่อง | StatMod | ต่อ lv | max | เต็ม | หมายเหตุ |
|---|---|---|---|---|---|
| Might | `AtkBp` | 100 | 50 | +50% | ช่องหลัก ×1 (ครึ่ง WoG — 5 คน) |
| Vitality | `MaxHpBp` | 100 | 50 | +50% | ×1 |
| Fortitude | **`DefBp` (ใหม่ 0.0.406)** | 100 | 50 | +50% | ไม่มี % DEF ใน StatMod (path ใช้ `DefAdd` flat เพราะ % ของ gear DEF ตอนนั้นน้อย) — ตอนนี้ DEF โตตาม tier แล้ว ทำ `DefBp` + consumer ใน `derived` จุดเดียว |
| Precision | `CritBp` | 50 | 15 | +7.5% | ราคา ×2; ต่ำกว่าช่องอื่นเพราะ crit ไม่มี diminishing |
| Lethality | `CritDmgBp` | 100 | 50 | +50% | |
| Giant Slayer | `DmgVsBossBp` | 100 | 50 | +50% | |
| Sprint | `MoveSpeedBp` | 100 | 10 | +10% | ราคา ×3; มีค่าจาก `march_grace_ticks` |
| Acceleration | `CooldownBp` | 50 | 20 | −10% | ×5 (WoG 10,000 เริ่ม = แพงสุด) |
| Regeneration | `HpRegenFlat` | 10 (=1 แสดง) | 50 | 50/5 วิ | gem Fortitude เริ่ม 15 — ช่องนี้เล็กกว่า gem 1 เม็ดต่อเลเวล |
| Shatter | `BreakDmgBp` | 100 | 50 | +50% | คู่กับ break สูตรใหม่ (0.0.405) ที่ tail ต้องพึ่ง stat นี้ |
| Satchel | `stash_cap` +1 | 1 | 50 | +50 ช่อง (ราคา ×5) | ไม่ใช่ StatMod — เขียนตรง `GameState.stash_cap` แบบ path `StashCap`; **ชนกับ `rare-slot-purchase` SKU** — ต้องเคาะ (§8) |
| Swiftness | atk speed | — | 25 | +25% | **ไม่มี StatMod** — รอบ 2 (ต้องเพิ่ม `AtkSpeedBp` + consumer ใน `atk_acc`) |

### 2.1 ช่องธาตุ (12) — user 2026-09-17: "ใส่ 6 ช่อง Elemental ATK + resist อีก 6 เป็น 12"

6 ธาตุตามชาร์ตผู้เล่น (`element-weakness-system.md` §3): Physical(Neutral) / Fire / Water / Thunder / Holy / Shadow — ไม่ทำ Earth/Wind/Poison (ตัดฝั่งผู้เล่นแล้ว 0.0.356-357)

| ช่อง ×6 | StatMod | ต่อ lv | max | เต็ม | หมายเหตุ |
|---|---|---|---|---|---|
| <ธาตุ> Attunement | `ElementDmgBp(ธาตุ)` | 100 | 30 | +30% | bucket เดียวกับ Arcana gem (cap 50% ของ gem) — training ต่ำกว่า cap เพื่อไม่ให้ทุกคน 100%; ราคา ×1 |
| <ธาตุ> Ward | `ResistElement(ธาตุ)` | 100 | 30 | +30% | มอนตีด้วย `element` ของมัน (`resist_element_bp`, cap 75%); ค่ากระจุกตาม region ที่ติด ⇒ ราคา **×0.5** |

ทำไมช่องธาตุเข้ากับทิศทาง: การลงทุนต่อ**ทีมที่พา** (Rapis+Elfie ลง Fire+Thunder) ⇒ Gil กระจายตามการจัดปาร์ตี้ ไม่ใช่ทุกคนซื้อชุดเดียวกัน — แกนที่ WoG ไม่มี · Ward เป็น "ซื้อเพื่อผ่านกำแพงนี้" ที่หมดค่าเมื่อย้าย region — ถูกกว่าครึ่งจึงพอ

รวมรอบแรก = 11 + 12 = **23 ช่อง** (Swiftness รอ `AtkSpeedBp`)

ไม่ทำ: Gil% / EXP% (เป็น faucet — `monetary-policy`: "raises Gil/hour? then it's a faucet"; EXP อยู่ที่ path) · PvP (ยังไม่มี PvP) · atk speed (รอบ 2)

**cap กับ gem**: `gems::for_each_gem_mod` clamp *ผลรวมของ gem* ด้วย `cap_of` ก่อนส่งออก — training บวก**ต่างหาก**ไม่โดน cap นั้น ⇒ crit สูงสุด = 15% (gem) + 15% (training) + gear/Apex/path; ไม่ต้องแตะ gem

### 2.2 Breadth gate (0.0.410, user: "ไม่ให้ sink อันเดียว")

ช่องที่ยืนอยู่บนเลเวลที่หาร `gate_step` (5) ลงตัว จะขึ้นต่อไม่ได้จนกว่า**ผลรวม level ทุกช่อง** ≥ `gate_total_per_step` (50) × (เลเวล ÷ 5): Lv5→6 ต้องรวม 50, Lv10→11 รวม 100, Lv15→16 รวม 150 … ⇒ ทุ่มช่องเดียวได้แค่ 5 แล้วต้องกระจาย (23 ช่อง: รวม 50 ≈ เฉลี่ย 2/ช่อง) · host ส่ง `gate_total_needed` / `gate_open` ต่อช่อง + `total_levels` ให้ UI แสดง "ต้องการรวม N (ตอนนี้ M)"

## 3. Data (catalog) — `GameData.training: TrainingRules` (`#[serde(default)]` → catalog เก่าโหลดได้)

```json
"training": {
  "cost_curve_bp": [10000, 40000, 76000, 136000, 231000, ...],   // 50 ค่า = WoG Strength ÷ 1000 (Lv1 = 1.0000)
  "slots": [
    { "id": "might",      "stat": "atk_bp",   "per_level": 200, "max_level": 50, "cost_mult_bp": 10000, "base_gil": 50 },
    { "id": "vitality",   "stat": "max_hp_bp", ... },
    ...
  ]
}
```

- `cost(slot, L) = base_gil × cost_mult_bp/10000 × cost_curve_bp[L-1]/10000` (L = เลเวลที่กำลังซื้อ 1..max) — integer ทั้งหมด, ตารางเดียวทั้งระบบ = "เข้าใจง่าย"
- `stat` เป็น key string → `SubstatKey`/`StatMod` ผ่าน `make_for(key, element, value)` ที่มีอยู่ (`substats.rs`; `element_dmg` มีแล้วจาก gem) — เพิ่ม key `def_bp` เมื่อทำ `DefBp` และ `resist_element` (StatMod มี, SubstatKey ยังไม่มี); ช่องธาตุมีฟิลด์ `element` ใน slot def
- `base_gil` 50 ให้ตารางเริ่ม: Lv10 = 6.8k · Lv25 = 242k · Lv50 = 12M ต่อช่องหลัก; อ้างอิง sim seed 1 (0.0.405): Gil สะสม h96 ≈ 78k โดยไม่มีที่ใช้ ⇒ วันที่ 4 ซื้อช่องหลัก 3 ช่องได้ ~Lv12-15 — ตั้งให้วัด ไม่ใช่ตัดสิน (§7)

## 4. State + fold

- `GameState.training: BTreeMap<String, u8>` (slot id → level) `#[serde(default)]` — ว่าง = no-op ⇒ goldens byte-identical
- `GilSpend::Training` · `Measures.training_levels: Counter` (quest layer อ่านได้) · `SoulSpend` ไม่แตะ
- **fold**: เพิ่ม `Entity.training_mods: Vec<StatMod>` (`#[serde(default)]`) เขียนทับทุก tick ใน upkeep ถัดจาก `apply_soul_path_auras` (บรรทัดเดียวกับ `persistent-entity-stats-must-heal`: ต่อ tick ไม่ใช่ตอนซื้อ — เซฟเก่า/recruit ใหม่ได้ค่าถูกทันที) แล้ว `stat_mods::for_each_mod` อ่านต่อจาก `bound_card_mods` ⇒ ทุก consumer (ATK/HP/crit/CDR/boss/break/regen) ได้ฟรี ไม่มี consumer ใหม่ยกเว้น `DefBp`
  - ทำไมไม่ใช้ aura แบบ path: BuffKind มีแค่ 6-7 ชนิด (`AtkPct/HpPct/CritBpAdd/…`) ไม่มี boss/break/regen/CDR — ต้องเพิ่ม BuffKind ทีละตัวพร้อม consumer; StatMod มีครบแล้ว
  - ≤12 mods ต่อคน ต่อ tick — ถูก (`for_each_mod` วนอยู่แล้ว)
- **`stash_cap`**: ช่อง Satchel ไม่ผ่าน entity — `stash_capacity()` บวก `training["satchel"]` แบบเดียวกับ `path_bonus(StashCap)` (state.rs:1287)

## 5. Input

`InputEvent::Train { slot_id: String }` → `events::handlers::train`:
1. slot มีใน catalog · level < max · Gil ≥ cost → `spend_gil(cost, GilSpend::Training)` · level += 1 · `Measures.training_levels += 1`
2. อย่างอื่น = silent no-op (replay ต้องรอด catalog เก่า/ใหม่)
3. ไม่มี draw PRNG — ไม่แตะ draw order
4. ไม่มี "ลด/รีเซ็ต" รอบแรก (WoG ก็ไม่มี) — ถ้าจะมี respec ค่อยเป็น Soul sink

## 6. Host / view (sim-host → Unity)

- `view_state.training`: `{ "<slot>": level }` — เบา ทุก tick
- query `training_rules`: slots (id, stat key, per_level, max, ค่ารวมตอนนี้, **cost ถัดไป**, cost ถึง max) + `cost_curve` — คำนวณใน host จาก catalog ไม่ให้ C# คิดเอง (sim authoritative)
- loc: `TRAIN_<slot>` ชื่อ + `TRAIN_<slot>_D` ใน `strings.csv` ทั้งสองสำเนา (`tools/check_loc_keys.py`)
- หน้าต่าง Unity: grid ช่อง × (ชื่อ / ค่ารวม / ปุ่ม "+1 · N Gil") — session Unity ทำ; ปลดล็อกแสดงตั้งแต่เริ่ม (WoG ปลด Lv3 — ของเราเปิดเลย ผู้เล่นควรเห็นว่า Gil มีที่ไปตั้งแต่ชั่วโมงแรก)

## 7. sim-cli + การวัด

- `build.rs::auto_train(state, data)`: ทุก sample ซื้อช่องที่ **cost ÷ per_level ต่ำสุด** ในกลุ่มหลัก (Might/Vitality/Fortitude/Lethality/Giant Slayer/Shatter) + Attunement ของธาตุที่ปาร์ตี้ตี (Neutral/Fire/Holy/Shadow) + Ward ของธาตุที่ region ปัจจุบันตี ตราบที่ Gil ≥ cost + recruit reserve — ไม่ optimise เกินผู้เล่นทั่วไป
- CSV: คอลัมน์ `train_lv` (ผลรวม level) + `gil_spent_training`
- **baseline ก่อนมี training** (seed 1, catalog 0.0.157, 2026-09-17): Gil สะสมไม่มีที่ใช้ — วัน 1 2.7k · วัน 2 29k · วัน 4 87k · วัน 7 176k · วัน 14 **465k** (≈ 40-45k/วัน คงที่ตั้งแต่วัน 3 เพราะยืนที่ด่าน ~100-110) · ด่าน: วัน 2 94 → วัน 7 105 → วัน 14 111 · ATK/คน 1,431 → 1,919 · wipe 1,267/14 วัน ⇒ ที่ `base_gil` 50 ช่องหลัก 3 ช่อง Lv15 (3×26k) ซื้อได้วัน 3, Lv25 (3×242k) ~วัน 17 — พอดีกับ "สัปดาห์ 2-3"
- เกณฑ์: วิ่ง 1 seed 14 วัน — ช่องหลักควรถึง ~Lv20-25 ในสัปดาห์ 2 (ราคา = Gil สะสม ~1 วันในช่วงนั้น), **ห้าม**ทะลุกำแพง ~105 ด้วยตัวเอง (ตัดสินแล้ว: กำแพงคือที่ตลาดมีค่า — training ควรเลื่อนได้ 5-10 ด่านไม่ใช่ 40) ⇒ ถ้าเลื่อนเกิน ลด `per_level` ไม่ใช่ขึ้นราคา
- โบนัส: sink นี้ทำให้ `gil` ในกราฟ**ไม่พุ่งขึ้นตลอด** — เป็นสัญญาณที่ `monetary-policy` ต้องการ

## 8. ต้องเคาะก่อนลงมือ

1. Satchel อยู่ใน training (Gil) หรือเป็น SKU ($ ถาวร +3 ช่อง, `rare-slot-purchase-design`) — ทำทั้งสองได้ (SKU = ก้อนใหญ่ครั้งเดียว, training = ทีละช่องแพงขึ้น) แต่ต้องตั้งใจ
2. `base_gil` 50 + curve WoG÷1000 — วัดแล้วค่อยเลื่อน
3. Precision max 15 (crit) — หรือให้ 50 แล้วปล่อยให้ crit stack (ไม่มี diminishing → 100% crit ได้ที่ gem 15 + train 50 + gear) — ข้อเสนอ: 15
4. Fortitude เป็น `DefBp` ใหม่ หรือใช้ `DefFlat` per level ตาม ladder (ง่ายกว่าแต่ต้อง scale ตาม tier)

## 9. ลำดับงาน (sim ก่อน, UI ตาม)

1. `TrainingRules` + catalog block (ว่างก่อน) + `GameState.training` + `GilSpend::Training` + `Measures` — **VERSION bump, goldens ต้องไม่ขยับ**
2. `InputEvent::Train` + handler + tests (ซื้อสำเร็จ / Gil ไม่พอ no-op / max no-op / slot ไม่มี no-op / replay เท่ากัน)
3. `Entity.training_mods` + upkeep refresh + `for_each_mod` fold + test ว่า Might Lv10 = +20% ATK จริงใน `derived`
4. `DefBp` + consumer + `substats` key
5. Satchel → `stash_capacity()`
6. host: view + `training_rules` query · loc keys · `check_loc_keys.py`
7. sim-cli `auto_train` + CSV + วัด 14 วัน → จูน `base_gil`/`per_level` → catalog bump
8. Unity window (session อื่น) · deploy
