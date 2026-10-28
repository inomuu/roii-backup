# Unity — ส่งต่องาน Floating UI (โหมด Companion)

> **Latest implementation (2026-09-16):** read [Floating Forge implementation + handoff](unity-floating-forge-implementation.md) first. It supersedes the older worktree, slot counts, fusion rules and Companion layout below. Current base: sim 0.0.397 / catalog 0.0.146; ten slots; failed fusion returns one input-grade item; gear failure pays full Gil. Godot development is retired.

อัปเดต 2026-09-15 (คืน) · master ล่าสุด (sim 0.0.385 · catalog 0.0.134) — `git log` ดูก่อน
เอกสารนี้ต่อจาก [unity-ui-luna-handoff.md](unity-ui-luna-handoff.md) (Home / Exemplar / Equipment)
— อ่านฉบับนั้นก่อนเรื่อง style ที่รับแล้วและวงจร preview ใน Unity; ฉบับนี้บอกว่า **โมเดลเกม
เปลี่ยนไปอย่างไรตั้งแต่งานนั้น** และหน้าไหนหายไปแล้ว จะได้ไม่เขียน UI ให้ระบบที่ไม่มี

## 1. เริ่มที่ไหน

| | |
|---|---|
| worktree | `/Users/lunalieneray/idle_mmorpg-unity-graphics` |
| branch | `codex/unity-graphics-ui` — ff ตาม master ตลอด (**ทั้งหมด merge แล้ว**) |
| Unity project | `…/idle_mmorpg-unity-graphics/unity/ROI` (6000.0.83f1) |
| untracked ที่ตั้งใจไม่ commit | `docs/previews/` (67 MB png), `docs/design-checkpoints/` (zip) — ปล่อยไว้ |

งาน Home/Exemplar/Equipment ของ codex ที่เคยค้าง **อยู่ใน master แล้ว** (`2c36612b` → merge
`14e0d313` → ปรับให้ตรงโมเดล `4c19c3cf`) — ไม่ต้องกลัวเปิดจาก HEAD แล้วงานหาย

`/Users/lunalieneray/idle_mmorpg` เป็น worktree ของอีก session (sim/economy) — อย่าสลับ branch
ที่นั่น · commit ในฝั่งนี้แล้ว master จะ ff ตามทีหลัง

compile ตรวจได้โดยไม่ต้องเปิด Editor (ใช้ตอนปิด Editor เท่านั้น — มี lockfile):

```bash
/Applications/Unity/Hub/Editor/6000.0.83f1/Unity.app/Contents/MacOS/Unity \
  -batchmode -nographics -quit -projectPath "$PWD/unity/ROI" -logFile /tmp/u.log; grep "error CS" /tmp/u.log
git checkout -- "unity/ROI/Assets/Resources/Fonts" "unity/ROI/Assets/TextMesh Pro"   # batch run เขียน atlas ทิ้ง
python3 tools/check_loc_keys.py
```

## 2. โมเดลเกมวันนี้ — สิ่งที่ UI ต้องพูดให้ตรง

### 2.1 ของสวมใส่มี **แกนเดียว = Grade** (0.0.372-0.0.380)

| แกน | อยู่ไหม | ใช้ทำอะไร |
|---|---|---|
| **grade** (`grade_step` 0-9) | ✅ | พลัง ×1.30/ขั้น · ชื่อ · สี · fusion 6→1 |
| rarity | ❌ บน gear (Common เสมอ) · ✅ บน Card / Exemplar / Regalia / Material | สีของพวกนั้นเท่านั้น |
| ★ star | ❌ บน gear · ✅ บน Card (= level การ์ด) | — |
| refine +N | ❌ ลบทั้งระบบ 0.0.370 | — |
| enchant / Star Stone | ❌ ลบทั้งระบบ 0.0.381 | — |
| bonus line ที่ roll | ❌ (ของดิบไม่ roll — D1) | — |
| **authored_lines** | ✅ | บรรทัดของ (def × grade) เหมือนกันทุกชิ้น — string[] |
| card socket | ✅ `socket_count` คงที่ตาม def (`_1s` / `_2s`) | badge |

ชื่อ grade (ตัดสินแล้ว 2026-09-15) — อ่านจาก `GRADE_<step>` ใน strings.csv ผ่าน `GameText`:

| step | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 |
|---|---|---|---|---|---|---|---|---|---|---|
| EN | Common | Fine | Rare | Elite | Epic | Royal | **Legendary** | Ancient | Divine | Eternal |
| ไทย | ธรรมดา | ดี | หายาก | ชั้นสูง | มหากาพย์ | หลวง | ตำนาน | โบราณ | เทพ | นิรันดร์ |
| สี | เทา | ขาว | เขียว | ฟ้า | ม่วง | ชมพู | ทอง | ส้ม | แดง | ดำ/รุ้ง |

เพดาน launch = step 6 (Legendary) · **ในเกมเขียน "Tier 3 - Rare"** (`GameText.GradeLabel`,
key `UI_GRADE_TIER`) ทุกที่ที่อธิบาย grade; ชื่อไอเทมใช้คำนำหน้าเฉยๆ ("Royal Adamant Saber" —
`GameText.ItemName`) · Common ไม่มีคำนำหน้า

API ที่มีให้แล้ว — **ใช้พวกนี้ อย่าเขียน switch สีเอง**:

```csharp
ItemSlot.ItemColor(ItemView it)   // gear → สีตาม grade, อย่างอื่น → สีตาม rarity, foil → ทอง
GameText.GradeName(step)          // "Rare"
GameText.GradeLabel(step)         // "Tier 3 - Rare"
GameText.GradeColor(step)
GameText.ItemName(it)             // คำนำหน้า grade + affix + ชื่อ
GameText.UnitName(unit_def)       // (ไม่ใช่ ClassName แล้ว)
ItemSlot.StatLine(it)             // "+140 ATK" จาก graded_*
```

### 2.2 `ItemView` (contract ของ `item_to_view`) — field ที่ **หายไป** และที่ต้องใช้แทน

| เคยใช้ | ตอนนี้ |
|---|---|
| `refine_level`, `refine_atk/matk/def` | ไม่มี |
| `star_atk`, `star_matk`, `star_def` | ไม่มี — ใช้ **`graded_atk` / `graded_matk` / `graded_def` / `graded_atk_speed_bp`** ตรงๆ |
| `atk_bonus` (ค่า authored) | ยังส่งมา แต่**ห้ามโชว์** — เป็นค่าก่อนคูณ grade |
| `bonus_lines` | เหลือแค่ของเก่า (ว่างหลัง migrate) — โชว์ **`authored_lines`** แทน/ก่อน |
| `rarity` บน gear | "Common" เสมอ — ห้ามใช้เลือกสี |
| `star` บน gear | 0 เสมอ (pip ★ ให้เฉพาะ `kind == "Card"`) |
| `class_id` (entity) | **`unit_def`** |

### 2.3 ช่องสวมใส่ 11 ช่อง (0.0.383)

```
Helmet  Gloves  Pants  Armor  Weapon  Shield  Earring  Shoes  Necklace  Ring  Bracer
```
- `WeaponLeft` **ออกแล้ว** และ dual wield ออกแล้ว — ช่อง Shield ถือแค่โล่ (off-hand แยกยังไม่ออกแบบ)
- Bracer **มีไอเทมแล้ว** (`bracer_s1..s7`) ดรอปเท่าช่องอื่น · paper-doll ขวามี 6 แถว
- Necklace/Ring: sim ยังยอมให้แหวนใส่ช่องสร้อยได้ (เศษ RO เก่า) — UI ไม่ต้องทำอะไรกับมัน
- Necklace / Ring / Earring / Bracer **ไม่มี main stat** (ATK/DEF = 0) — ค่าของมันคือ `authored_lines` ล้วน อย่าโชว์ "+0 ATK"
- `authored_lines` ตอนนี้มาจากตาราง `loot_rules.substats` (0.0.384): pool 10 stat, ต่อช่อง — ดู `docs/references/roi-substat-table.csv` ว่าชิ้นไหนได้บรรทัดอะไร
- ชื่อเก่า `HeadTop/HeadMid/HeadLow/Garment/Accessory1/2` ห้ามใช้ (ดู `PaperDoll.cs` Left/Right table)

### 2.4 หน้าที่**หายไป**แล้ว (0.0.380) — อย่าลิงก์ไปหา

| หน้า | สถานะ | แทนด้วย |
|---|---|---|
| RareStrip (rare bag ใต้ Inventory) | ลบ · ระบบ rare bag ถอดออก "รอคิดใหม่" | ไม่มี — ของทุกชิ้นลง Inventory |
| EnchantWindow (Star Stone) | ลบ | ไม่มี |
| ApexWindow (แต้ม stat เก่า) | ลบ | `ExemplarApexPage` ในหน้า Exemplar |
| BlacksmithWindow (refine) | ลบตั้งแต่ 0.0.370 | ไม่มี |

query ฝั่ง host ที่ยังตอบแต่ตอบค่าว่าง: `rare_bag` → `[]`, `orbs` → `[]`, `orb_plan:*`/`orb_fits:*`
→ `{}`, `refine_plan:*` → null, `advances` → `[]` — ถ้าเจอ UI เก่าเรียกพวกนี้ = ลบได้

### 2.5 ที่ทำได้เลย / ที่ยังรอ

- **Fusion Forge** (ผสม 6 ชิ้น grade เดียวกัน → 1 ชิ้น grade+1, ชนิดสุ่ม, tier เฉลี่ย) — **bridge พร้อมแล้ว
  (0.0.385)** ทำ UI ได้เลย · shell ของ Equipment picker เอามาใช้ต่อได้:
  1. `sim.QueryJson("fuse_rules")` → `{inputs:6, ceiling_step, server_roll_from_step}`
  2. เลือก 6 index ในกระเป๋า → `sim.QueryJson("fuse_preview:i,i,i,i,i,i")` →
     `{tier, tier_up_bp, grade_step, grade, payout_gil, needs_server_roll, key, attempt}` หรือ `{}` ถ้าผสมไม่ได้
     — โชว์ก่อนกด: "Tier N (100−up%) หรือ N+1 (up%) · grade ถัดไป · ชนิดสุ่ม · ได้ Gil payout"
  3. ถ้า `needs_server_roll` → `ServerClient.FuseRoll(key, attempt, (ok, nonce, why) => …)` → ถ้า ok ส่ง
     `{"FuseGear":{"picks":[…],"server_roll":<nonce string as number>}}` · ถ้าไม่ ok **ไม่กด** บอกเหตุผล
     (`why`) — ห้าม fallback เป็น 0 verifier จะ reject save
  4. ถ้าไม่ต้อง server → `{"FuseGear":{"picks":[…]}}` ตรงๆ
  · ผลลัพธ์ดูจาก inventory ที่เปลี่ยน (`FuseOutcome` ไม่ได้ส่งกลับ) — ของใหม่คือชิ้นที่ grade_step สูงกว่าเดิม
- Character sheet แบบ floating — เก็บ `CharacterWindow.cs` ไว้เป็นต้นแบบ (user: "จะ rework ให้เป็น
  floating UI แบบ TBH/WoG") · ค่าทุกตัวมาจาก query `stats:<entity_id>` เท่านั้น renderer ไม่คำนวณเอง

## 3. Companion mode — ที่มีอยู่แล้ว

`unity/ROI/Assets/Scripts/UI/CompanionWidget.cs` — กล้องเล็กล็อกตัวละครทีละคน (W เปิด/ปิด · 1/2/3
จำนวน pane · -/= zoom · คลิกขวาสลับคน) · **กล้องแยกต่อ pane** และ render 30 Hz ตั้งใจ (รันในมุมจอ
หลายชั่วโมง) — อย่าเปลี่ยนเป็น render ทุกเฟรม · ดีไซน์เดิม: [desktop-companion-mode] memory =
16:9 widget always-on-top, INT zoom · UI ที่ลอยทับ companion ต้องเคารพ canvas layer:
windows 10 / popups 20 / tooltips 30 / HudBar 40 (`ui-canvas-layer-zorder`)

กฎ UI ที่ยังใช้: Control nodes ไม่ใช่ `_draw` · **ไม่มี drag-and-drop** (tap-select แล้ว tap-place —
มือถือมีแผน) · Noto Sans ผ่าน `UiTheme` · ไม่มี string literal ใหม่ — ทุกข้อความผ่าน `Loc.Get`
(key ต้องอยู่ใน `strings.csv` **ทั้ง 2 copy** `unity/ROI/Assets/StreamingAssets/` และ `godot/i18n/`
ให้ byte-identical — `check_loc_keys.py` เช็ค) · Godot เลิกทำแล้ว แต่ไฟล์ `.gd` ต้อง parse ได้

## 4. ก่อนประกาศเสร็จ

```
git diff --check
python3 tools/check_loc_keys.py
cargo test -p sim-host            # ถ้าแตะ crates/sim-host (query contract test อ่าน .cs)
Unity batch compile = 0 error CS  (ข้อ 1)
```
`cargo test --workspace` ค้างบนเครื่องนี้ (godot-ext/sim-ffi) — รันแยก package

## Prompt สำหรับส่งต่อ

> ทำ floating UI โหมด companion ต่อใน `/Users/lunalieneray/idle_mmorpg-unity-graphics`
> (branch `codex/unity-graphics-ui`, ชี้ master แล้ว) อ่าน `CLAUDE.md`,
> `docs/unity-ui-luna-handoff.md` แล้ว `docs/unity-floating-ui-handoff.md` ก่อน — ของสวมใส่มีแกนเดียว
> คือ grade (สี/ชื่อผ่าน `ItemSlot.ItemColor` / `GameText.GradeLabel`), ไม่มี ★/refine/enchant/
> rare bag แล้ว, 12 ช่อง, entity ใช้ `unit_def`. ยึด style Home/Exemplar ที่รับแล้ว ไม่รื้อ
> Fusion Forge ทำได้แล้ว — contract อยู่ §2.5 (fuse_rules / fuse_preview / ServerClient.FuseRoll)

> อัปเดต 2026-09-16 (sim 0.0.395): `fuse_rules` มี `blacksmith_level` / `crafter_level_cap` / `crafter_fusions_per_level` / `fusions_run` / `fail_bp_by_grade` / `fail_bp_now_by_grade[10]` และ `fuse_preview:` มี `fail_bp` (ตาม grade ตั้งต้น) — หน้า Fusion Forge ควรแสดง "Blacksmith Lv N (x/10)" และโอกาสล้มเหลว · ล้มเหลว = ได้ 1 ชิ้น grade เดิมคืน (`FuseOutcome::Failed.def_id`) เหมือน gem · ระบบ gem ทั้งหมด (socket / fuse / crafter level) ดู `docs/gem-system-design.md` §8

> อัปเดต 2026-09-16 (sim 0.0.398, **stage ladder**): band → **stage** 240 ด่าน (`zone_b<N>_<r>` N ถึง 240) region ละ 12 (`arena_tables.field.zones_per_region` 12) · loc ใหม่ `UI_STAGE_RANGE` / `UI_STAGE_CLEARED`, `UI_AREA_BAND` = "Stage {0}" · view key `loot_gap_mult_bp` หายไป (ไม่มี penalty แล้ว) · `zone_spawn.deploy_grace_ticks` ตอนนี้ = เวลาเดินจริงของปาร์ตี้ + 3 tick — ArenaView ก้าวให้ถึงก่อน `MarchSettleSeconds` (0.3 s) แล้ว (แก้ใน ArenaView.cs แล้ว) · ห้อง 2-3 heal 20% ไม่ full · DungeonWindow ยังมี "zone_b10_1 / Lv 46–50" ของ Duskmourn ที่ล้าสมัย (ควรชี้ `zone_dungeon_duskmourn_1`)
