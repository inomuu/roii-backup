# UI handoff — sim 0.0.405 → 0.0.414 (2026-09-17)

ทุกอย่างอยู่บน master (`8c20643e`) · server อยู่ 0.0.412 / catalog 0.0.164 (จะ deploy 0.0.414 หลังเคาะเรื่อง Rapis) · Editor ต้อง restart เพื่อโหลด `libsim_ffi.dylib` ใหม่

## 1. Training (ฝึกฝน) — หน้าต่างใหม่ ⚠️ ใหญ่สุด / สำคัญสุด (Gil sink เดียวของเกมตอนนี้)

- query `training_rules` →
  ```json
  { "cost_curve_bp": [...50], "levels_bought": n, "total_levels": n, "gate_step": 5, "gate_total_per_step": 50, "stash_slots": n,
    "slots": [ { "id":"might", "stat":"atk_pct", "element":null, "per_level":100, "max_level":50, "base_gil":5000, "cost_mult_bp":10000,
                 "level":3, "value_now":300, "next_cost":38000 | null, "gate_total_needed": 50 | null, "gate_open": true,
                 "text":"+3.00% ATK", "text_per_level":"+1.00% ATK" } ... 23 ช่อง ] }
  ```
- event `{"Train":{"slot_id":"might"}}` — no-op ถ้า Gil ไม่พอ / cap / gate ปิด (client ไม่ต้องเช็คเอง แต่ควรแสดงเหตุผล)
- view: `training` = `{slot_id: level}` (มีเฉพาะที่ซื้อแล้ว)
- **breadth gate**: ช่องที่อยู่ Lv5/10/15… ขึ้นต่อไม่ได้จน `total_levels ≥ gate_total_needed` → แสดง "ต้องการรวม 50 (ตอนนี้ 37)" บนช่องนั้น
- loc: `TRAIN_<slot_id>` ชื่อ 23 ช่อง · `UI_TRAINING`, `UI_TRAIN_LEVEL` "Lv {0}/{1}", `UI_TRAIN_BUY` "Train · {0} Gil", `UI_TRAIN_MAXED`, `UI_TRAIN_PER_LEVEL`
- จัดกลุ่ม: หลัก 11 (might…satchel) / Attunement 6 ธาตุ / Ward 6 ธาตุ (`element` = Neutral=Physical, Fire, Water, Thunder, Holy, Shadow)
- design: `docs/training-system-plan.md`

## 2. Party slot — `PartyWindow` ทำแล้ว (ปุ่ม "Open a slot — N Gil")

- `party_meta` เพิ่ม `party_slots`, `slot_cost` (-1 = ครบ) · event `"BuyPartySlot"` · `RecruitMember`/`BuyCharacter` เป็น no-op แล้ว (ตัวละครมาจาก gacha เท่านั้น)
- **ยังไม่มี**: ตั๋ว 10-pull ตอนซื้อ slot (งาน server อ่าน `Measures.party_slots_bought`) — UI ควรเผื่อ callout "ได้ตั๋ว Summon ×10" หลังซื้อ

## 3. ขาย NPC ออก — `ItemGridWindow` ทำแล้ว (เหลือ Discard/Deposit)

- `SellInventoryItem`/`SellAllJunk` no-op · `pending_junk_gil` = 0 · Gil มาจาก **fusion 100%** ของ sell_value ชิ้นที่ใส่ → หน้า Forge ควรโชว์ payout ให้เด่น (`fuse_preview:` มี `payout_gil`)
- ฝั่ง gem-ui: `FloatingForgeSmoke.Drag.cs` ยังอ้าง Sell — ลบได้

## 4. Gil ×100 — ตัวเลขทุกที่ใหญ่ขึ้น 100 เท่า (5,000 / 38,000 / 1,000,000)

- ไม่ต้องหาร — sim ส่งค่าจริง; ควรใช้ `N0` format ทุกที่ที่โชว์ Gil · เซฟเก่า migrate เองตอนโหลด

## 5. Break — สูตรใหม่ `(200 + 14×Σlevel ปาร์ตี้) × (toughness+20)/40`

- ไม่มี query ใหม่; ตัวเลข break ใหญ่ขึ้นมาก (≈ 20-30% HP มอน) — damage number ของ `HitKind::Break` ควรเด่นกว่าปกติ (companion pane ที่เพิ่งทำมี damage numbers แล้ว)

## 6. Ladder / first session (ไม่มี UI ใหม่ แต่ควรรู้)

- ห้องละ 20→30 kill (ด่าน 1→12), มอน HP ×2, ดาเมจมอน ramp, XP ½, ถอยฟาร์มหลัง wipe 1→5 clear ตามด่าน — `zone_spawn.boss_threshold` ส่งอยู่แล้ว
- ข้อเสนอ **unlock cadence** ใน `docs/first-session-design.md` §2: UI ซ่อน/โชว์ระบบตามเงื่อนไข (Fusion เมื่อมี 6 ชิ้น → Training ด่าน 5 → Gem ด่าน 8 → Sigil ด่าน 13) + callout ครั้งแรก — sim ไม่ซ่อนอะไร ใช้ `band_reached` / นับชิ้นในกระเป๋าฝั่ง client

## 6.5 Crests + Disenchant (0.0.416-0.0.417) — หน้าใหม่ + ปุ่มในกระเป๋า

- กระเป๋า: ปุ่ม **Disenchant** ข้าง Discard — query `disenchant_preview:<inv_idx>` ก่อนเพื่อโชว์ "→ Weapon Essence T3 × 3" (null = ทำไม่ได้ → ปุ่มเทา) · event `{"Disenchant":{"inv_idx":n}}`
- หน้า **Crests**: query `crest_rules` → 20 ช่องจัดตาม category × T3-T7 (ชื่อ `CREST_<id>`, material `MAT_<category>` format T{0}), แต่ละช่อง level/max, ค่ารวม (`text`), material ที่ถือ vs `next_cost`, ปุ่ม `{"UpgradeCrest":{"crest_id":"crest_w3"}}`
- view: `crests` `{id: level}`, `crest_mats` `{"weapon_t3": n}`
- design: `docs/crest-system-design.md`

## 6.6 Daily/Weekly labels เปลี่ยน (server 2026-09-18)

- `sell_10` → **`fuse_3`**, `craft_1` → **`train_1`**, `spend_100_gil` → **`spend_gil`** (10,000) — `PeriodWindow.TaskName` อัปเดตแล้ว, loc `UI_DAILY_Q_*` เพิ่มแล้ว

## 7. ค้างจากรอบก่อน (ยังอยู่)

- Regalia แสดง "Lv x/60" · bestiary ป้าย `stage_min/stage_max` แทน "Lv" · DungeonWindow Duskmourn entry เก่า · weakness ≥2 ต่อมอน (bestiary แสดงได้เลย)
