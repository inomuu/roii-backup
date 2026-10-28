# UI handoff — 2026-09-19 (master @ sim 0.0.438 / catalog 0.0.184, after merging codex/ui-training)

เขียนสำหรับ session UI (Codex) ก่อนแยก branch ใหม่ · ของเดิม: `docs/ui-handoff-2026-09-17.md`,
`docs/unity-ui-training-merge-handoff.md` (บัคเปิด + follow-up ของ master อยู่ท้ายไฟล์นั้น)

**ก่อนเริ่ม**: `git merge master` เข้า branch ใหม่ · rebuild plugin `tools/unity_dylib.sh` + restart Editor
(`roi_clone` อยู่ในแล้ว) · `tools/unity_data.sh` · คำต้องห้าม: **MATK** และ **class** ไม่มีในเกมแล้ว
(มีแค่ ATK และ Doctrine: sentinel / striker / destroyer / catalyst / support) · "SSR Regalia" ไม่ใช่ "5★"

## 1. Party slot — ปุ่มปลดล็อกในช่องปาร์ตี้ (ใหม่, user 2026-09-19) ⚠️ ทำก่อน

ช่องปาร์ตี้ที่ยังล็อก (ในแถบสมาชิก / หน้า Character ที่มี 5 ช่อง) ต้องเป็น **ปุ่มปลดล็อกในตัวช่อง** ไม่ใช่ปุ่มใน PartyWindow เท่านั้น:
- ราคากำกับ: `party_meta.slot_cost` (Gil ของช่องถัดไป, -1 = ครบแล้ว) · เทาถ้า `gil < slot_cost`
- **รางวัลแสดง**: `party_meta.slot_reward_tickets` (10) → "ได้ตั๋ว Summon ×10" บนช่อง และ callout หลังซื้อ
  (ตั๋วจ่ายโดย server ตอน save ถัดไปผ่าน verify — ไม่ใช่ทันที; ข้อความควรบอกว่า "จะได้รับหลังบันทึก")
- event `"BuyPartySlot"` · view `party_slots` (ที่ซื้อแล้ว) / `party_max` (5)
- ราคา 5,000 / 5,000 / 8,000 / 12,000 (catalog `party.slot_gil`) — อ่านจาก query อย่า hardcode

## 2. Crests + Disenchant — ยังไม่มี UI (ระบบเสร็จตั้งแต่ 0.0.416, rename Relic→Crest 0.0.431)

- กระเป๋า: ปุ่ม **Disenchant** ข้าง Discard — query `disenchant_preview:<inv_idx>` → "→ Weapon Essence T3 × 3"
  (null = ทำไม่ได้ → เทา) · event `{"Disenchant":{"inv_idx":n}}` · gear T3-T7 และเพชร Rare+ เท่านั้น
- หน้า **Crests**: query `crest_rules` → 20 ช่อง (weapon/armor/accessory/gem × T3-T7), ชื่อ `CREST_<id>`,
  material `MAT_<category>` (format `T{0}`), level/max 30, ค่ารวม (`text` จาก host), material ที่ถือ vs
  `next_cost`, ปุ่ม `{"UpgradeCrest":{"crest_id":"crest_w3"}}` · view `crests` `{id: level}`, `crest_mats`
  `{"weapon_t3": n}` · loc `UI_CRESTS`, `UI_DISENCHANT*`, `UI_CREST_UPGRADE/MAXED`
- **T3 ของทุกหมวดเป็น economy stat**: Scholar XP / Merchant Gil / Spirit Soul / Fortune Drop rate —
  label `+X% XP Gain` ฯลฯ มาจาก host แล้ว · design `docs/crest-system-design.md` (ตารางล่าสุดอยู่หัวไฟล์)

## 3. Regalia

- **passive ของ SSR** (0.0.435): item view ส่ง `overlay_passive` = `[{key, args:[now, atS5, extra]}]` แล้ว
  (ทำ 2026-09-19) — UI format ด้วย `Loc.Fmt(key, args…)`; key = `REGALIA_PASSIVE_*` (11 key ใน strings.csv)
  · แสดงบรรทัด `UI_REGALIA_PASSIVE_DOCTRINE` ({0} = `Loc.Get("UI_DOCTRINE_"+overlay_doctrine)`) ใต้ passive
  · R/SR ส่ง array ว่าง
- ทุกคนใส่ได้ทุกชิ้น (stat ได้หมด) — อย่าทำ gate ฝั่ง client
- แสดง "Lv x/60" และดาว 0-5 (Resonance จากตัวซ้ำ) แยกกัน · `overlay_classes` ว่างเสมอแล้ว

## 3.5 Resonance S1-S6 (ใหม่ 2026-09-19)

- ดาว Exemplar สูงสุด **6** แล้ว (`EXEMPLAR_MAX_STARS`); ทุกดาวเป็น effect ไม่ใช่แค่ +1 lv
- item view ของ `exemplar_*` ส่ง `overlay_ranks` = 6 list ของ `{key, args, skill}` — UI: `Loc.Fmt(key, args[0], args[1], GameText.SkillName(skill))` ต่อบรรทัด (key `RANK_*` 27 key ใน strings.csv; `{2}` = ชื่อสกิล)
- หน้าตัวละคร/Exemplar: แสดง S1-S6 ไล่ลง ดาวที่ถึงแล้วสว่าง ที่ยังไม่ถึงเทา (เหมือน Genshin)

## 4. Combat / HUD

- ult gauge max = **1,000** (`eff_lb_max`) — ยืนยันว่า gauge ของ HUD ใหม่ไม่ hardcode 100
- HP ฝั่งผู้เล่นเต็มสเกล / มอน ÷10 (host ทำแล้ว) — ยืนยัน boss bar / damage number ไม่คูณซ้ำใน UI
- สถานะใหม่ต้องมี icon: **Darkness** (`Darkness`, debuff ของ Nyx: ตีพลาด %), **Umbral Mark** stack,
  **Enrage** ของมอน (`AtkPct` source = ตัวเอง, โตทุก 15 วิ หลัง 30 วิ) — view `buffs` มีอยู่แล้ว
- Nyx: `CLASS_NAME_nyx` มี, sprite ยังใช้ thief (รอ art)

## 5. ค้างจากรอบก่อน

- bestiary ป้าย `stage_min/stage_max` แทน "Lv" · weakness ≥2 ต่อมอน · DungeonWindow Duskmourn entry เก่า
- warning `enableWordWrapping` → `textWrappingMode` (CompanionWidget.Damage.cs, HomeMenuLayout.cs)
- ไอคอนอุปกรณ์ยัง placeholder · Companion standalone ต้องเทส build จริง (ดู merge-handoff)
- ยืนยันว่า fix "ช่องแดงทุกช่อง" (`FreshSnapshot` ใน FloatingInventoryHost.Items.cs) หายจริงตอน auto-fusion รัว

## Sim-side ที่ master ทำให้แล้ว
- `overlay_passive` (loc key + numbers) สำหรับ SSR Regalia · `party_meta.slot_reward_tickets`
