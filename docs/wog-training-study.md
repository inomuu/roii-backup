# WoG "Training" (ฝึกฝน) — ศึกษาเพื่อทำ Gil sink ของเรา

> user 2026-09-17: "ลองศึกษาระบบฝึกฝนของ WoG เป็นระบบที่น่าสนใจ เพราะเข้าใจง่าย + Gil sink ชั้นยอด"
> ข้อมูลตารางดึงจาก `titlee2111/war-of-genesis-helper` (index.html, `trainingSkills` — export จาก client จริง 40 รายการ)
> วันที่ 2026-09-17. ยังไม่ตัดสินอะไร — เอกสารนี้คือข้อมูล + ข้อเสนอ

## 1. ระบบเป็นยังไง

หน้าต่างเดียว มีช่อง 40 ช่อง แต่ละช่องคือ stat หนึ่งตัว มีเลเวล กด "ฝึก" จ่าย Gold ได้ +1 เลเวล = stat ขึ้นเป็น**เส้นตรง** (ค่าคงที่ต่อเลเวล) ส่วน**ราคาโตแบบ geometric ที่ค่อยๆ แบนลง** ไม่มีความล้มเหลว ไม่มีวัสดุ ไม่มี RNG — Gold อย่างเดียว ปลดที่ user level 3

ที่ "เข้าใจง่าย": ทุกช่องใช้ตารางราคาเดียวกัน (ต่างกันแค่ตัวคูณ) และ stat โตเป็นเส้นตรงเห็นตัวเลขล่วงหน้าได้ทั้งเส้น

## 2. ตารางจริง

### 2.1 ราคา (Strength = ต้นแบบ, 50 เลเวล)

| Lv | Gold | Lv | Gold | Lv | Gold |
|---|---|---|---|---|---|
| 1 | 1,000 | 13 | 299,000 | 30 | 13,080,000 |
| 2 | 4,000 | 16 | 632,000 | 35 | 31,850,000 |
| 3 | 7,600 | 20 | 1,627,000 | 40 | 69,790,000 |
| 5 | 23,100 | 25 | 4,850,000 | 45 | 137,100,000 |
| 10 | 136,700 | 26 | 5,960,000 | 50 | 241,400,000 |

- อัตราโตต่อเลเวล: ×4 → ×1.6 → ×1.3 (Lv10) → ×1.2 (Lv25) → ×1.11 (Lv50) — **geometric ที่ exponent ลดลงเรื่อยๆ** (ไม่ใช่ exponential คงที่ ไม่ใช่ polynomial)
- สะสม: ถึง Lv25 = 24M · ถึง Lv40 = 449M · ถึง Lv50 = **1.97B** ต่อช่อง
- ช่องอื่นใช้ตารางเดียวกันคูณตัวคูณ: ช่อง 2,000 เริ่ม (×2 ที่ Lv1 แต่ Lv26+ เท่ากัน 5.96M — ตัวคูณมีผลแค่ต้นๆ), Swiftness 25 เลเวล (3,000 → 385M), Sprint 10 เลเวล (3,000 → 184M), Cooldown 20 เลเวล (10,000 → **938M**), Inventory 100 เลเวล (5,000 → 1.09B)

### 2.2 ช่องทั้ง 40 (ค่าต่อเลเวล × เลเวลสูงสุด = ค่าเต็ม)

| กลุ่ม | ช่อง | ต่อเลเวล | Max | เต็ม |
|---|---|---|---|---|
| **หลัก** | Strength / Fortitude / Vitality | +2% ATK / DEF / HP | 50 | +100% |
| | Swiftness | +1% atk speed | 25 | +25% |
| | Sprint | +1% move speed | 10 | +10% |
| | Precision / Focus / Agility / Piercing | +1 crit-hit / +2 accuracy / +2 evade / +2 block-pierce (flat) | 50 | 50 / 100 |
| | Critical Strike | +2% crit dmg | 50 | +100% |
| **PvE/PvP** | Subjugation / Expedition Def | +2.5% PvE ATK / DEF | 50 | +125% |
| | Duel / Combat Def | +2.5% PvP ATK / DEF | 50 | +125% |
| | Giant Slayer | +1.5% boss dmg | 50 | +75% |
| | Amplification / Protection | +1% dmg dealt / −1% dmg taken | 50 | ±50% |
| **ต้านทาน** | Critical Def, Iron Skin, Insulation/Heat/Antidote (ธาตุ), Lightning/Fire/Immunity (สถานะ), Shock/Burn/Poison Def | +1-2 หรือ +1.5% | 50 | 50-100 |
| | Indomitable | +2% stun resist | 10 | +20% |
| | Cooldown Acceleration | −0.5% CDR | 20 | −10% |
| | Regeneration | +1% natural regen | 50 | +50% |
| **QoL** | Inventory Expansion | +1 ช่อง | 100 | +100 |
| | Storage Expansion | +1 หน้า | 4 | 100k → 40M |
| | Vault Expansion | +20M gold cap | 50 | +1B |
| | Hunt Time Reduction | −1 kill ต่อการเรียกบอส | 10 | −10 |
| | Recycling ×5 (gear/acc/wing/rune/pet) | +1% เงินคืนตอนแยกชิ้น | 20 | +20% |

รวมทั้ง 40 ช่อง ≈ **60-70B gold** ให้เต็ม — ไม่มีใครเต็ม; นี่คือ sink ที่ไม่มีก้น

## 3. ทำไมมันเป็น sink ที่ดี (สิ่งที่ต้องคัดลอกให้ถูก ไม่ใช่ตัวเลข)

1. **ไม่มีก้น แต่มีจุดคุ้ม** — ราคาโต geometric แต่ผลโตเส้นตรง ⇒ marginal value ต่อ gold ลดลงเรื่อยๆ ผู้เล่นตัดสินใจเองว่าจะหยุดตรงไหน ทุก stat มี "ราคาต่อ +1%" ที่เปรียบเทียบข้ามช่องได้ (afkmeta ทำ planner ให้เลย) ⇒ ไม่มีวันที่ Gold "ไม่มีอะไรให้ซื้อ"
2. **40 ช่องกระจายกว้าง** — เงินก้อนใหญ่ถูกดูดเข้า Vitality/Strength/Fortitude ก่อน แล้วค่อยลามไปช่องรอง; ช่อง niche (ต้านทานธาตุ, PvP) มีไว้ให้คนที่อยากลง ไม่ใช่ทุกคนต้องลง
3. **ไม่มีล้มเหลว ไม่มี RNG** — Gold เข้า = stat ออก แน่นอน ต่างจาก fusion/refine ที่เป็น sink ผ่านความเสี่ยง; สองแบบอยู่ร่วมกันได้ (refine = ตื่นเต้น, training = ที่ทิ้งเงินแน่นอน)
4. **ตัวคูณ % ไม่ใช่ flat** — ไม่หมดค่าเมื่อ gear โต (+100% ATK ที่ Lv50 มีค่าเสมอ) ⇒ sink ที่ยังอยู่ตอน endgame
5. **ปลดตั้งแต่ Lv3** — ผู้เล่นรู้ตั้งแต่ชั่วโมงแรกว่า gold มีที่ไป
6. **QoL อยู่ในตารางเดียวกัน** — inventory, vault cap, boss-summon-kill-count ก็ซื้อด้วย gold — ผูกความสะดวกเข้ากับ sink เดียวกัน

## 4. ของเรา (ตอนนี้)

- faucet: `MonsterKill`, `NpcSale`, `Salvage`, `Quest` · sink: `RecruitRoster/Field`, `Potential` (Apex/Sigil), `Craft/CraftLearn`, `Shop`, fusion จ่าย Gil (คืนเต็มเมื่อล้มเหลว 0.0.396)
- วัด sim-cli seed 1 (0.0.405): Gil สะสม h24 ≈ 1.7k · h48 ≈ 22k · h96 ≈ 78k (ไม่นับที่ recruit/fusion ใช้ไป) — **หลัง recruit ครบ Gil ไม่มีที่ไป** (`monetary-policy` memory: faucet 24/7 ⇒ sink ต้อง scale ตาม)
- ไม่มี Gil market แล้ว (2026-09-16) ⇒ Gil เป็นสกุลเงิน sink อย่างเดียว — training คือคำตอบตรงตัว
- Apex (`paragon-system`) เป็น stat-point จาก level ไม่ใช่ Gil — ไม่ชนกัน: Apex = จุดฟรีจำกัด, training = ซื้อไม่จำกัด

## 5. ข้อเสนอ "Training" ของ ROI (ยังไม่ตัดสิน)

- **ระดับปาร์ตี้** (account-wide เหมือน WoG) ไม่ใช่ต่อตัวละคร — ตัวละครสลับ/gacha ได้ ต้องไม่รีเซ็ต
- ช่องรอบแรก **~12 ช่อง** ไม่ใช่ 40: ATK% / DEF% / HP% (50) · Crit / CritDmg (50) · Boss DMG (50) · Atk speed (25) · Move (10) · CDR (20) · HP regen (50) · Break DMG% (50) · Bag slots (สอดคล้อง `rare-slot-purchase` SKU idea: ช่องถาวร) — ช่องต้านทานรอจน element resist มี consumer
- **ตารางราคาเดียว** ทั้งระบบ (ต่างกันแค่ตัวคูณ) — คือสิ่งที่ทำให้ "เข้าใจง่าย"; ทำเป็น `loot_rules.training` ใน catalog ให้ tune ได้ไม่แตะ code
- ค่าต่อเลเวลเข้า `StatMod` ที่มีอยู่แล้ว (`AtkPct`, `HpPct`, `DmgVsBossBp`, `HpRegenFlat`, `BreakDmgBp`, …) — fold ที่ `stat_mods` เหมือน Regalia/gem ⇒ ไม่มี consumer ใหม่
- cap รวมกับ gem: gem cap ต่อ stat (crit 15% ฯลฯ) ตั้งไว้แล้ว — training ต้องเป็น**อีกชั้น**ที่ไม่ถูก cap เดียวกัน หรือไม่ก็ให้ training เฉพาะ stat ที่ gem ไม่มี — ต้องเคาะ
- ราคา: ยึดรูปทรง WoG (exponent ลดลง) แต่ scale ให้ Lv1-10 ของ ATK% ซื้อได้ในวันแรก และช่องหลัก Lv25 ≈ Gil สะสมสัปดาห์ที่ 2 — ต้องวัดจาก sim-cli (`gil` คอลัมน์) หลังใส่ `GilSpend::Training` แล้วให้ `build.rs` ซื้อแบบ marginal-value
- `InputEvent::Train { slot }` + `Measures.training_levels` (quest layer อ่านได้) · server: เป็น replay-verify ปกติ ไม่ต้อง server-roll (ไม่มี RNG)

## 6. ที่ต้องเคาะก่อนทำ

1. ปาร์ตี้ vs ต่อตัวละคร (ข้อเสนอ: ปาร์ตี้)
2. ช่องรอบแรกกี่ช่อง / ช่องไหน
3. training ซ้อน cap ของ gem ยังไง
4. Bag slot อยู่ใน training (Gil) หรือเป็น SKU ($) — ชนกับ `rare-slot-purchase-design`
