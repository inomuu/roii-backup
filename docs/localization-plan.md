# Localization Plan — หลายภาษาตั้งแต่แรก

**Status:** วางแผน 2026-07-07 (ปรับได้) — ตัดสินใจ: ทำ i18n infrastructure ตั้งแต่ก่อน launch
ทุกภาษา ไม่ retrofit ทีหลัง. เหตุผลเชิงกลยุทธ์: เป้า mass ระดับแสนต้องกินตลาด
zh-CN/zh-TW/pt-BR/ru ซึ่งเป็น audience ก้อนใหญ่สุดของแนว idle + RO nostalgia.
**Companion to:** `steam-launch-plan.md` (timeline อ้างแผนนี้)

---

## ปริมาณงานจริง (สำรวจ 2026-07-07)

| แหล่ง | ที่อยู่ | จำนวน |
|---|---|---|
| ชื่อ item/monster/zone | `data/game_data.json` field `name` | ~251 |
| ชื่อ + blurb สกิล | `crates/sim-core/src/skills/catalog/*.rs` (74 entries) | ~148 |
| UI strings | `godot/views/*.gd` + `godot/*.gd` (literal กระจาย) | ~300-480 |
| Steam page copy | short/long description ต่อภาษา | เล็ก |

รวม **~700-900 key ต่อภาษา** (≈5-8k คำ) — เล็กพอที่ต้นทุนจริงคือ *โครงสร้าง + วินัย*
ไม่ใช่ค่าแปล. แปลด้วย Claude เป็น batch แล้ว native-review เฉพาะภาษา tier หลัก.

## หลักการสถาปัตยกรรม (ตัดสินแล้ว — สอดคล้อง sim/render split)

1. **sim-core ไม่รู้จักภาษา** — ห้ามมี locale ใน `GameState`/`GameData`. สตริงที่ sim ส่ง
   ข้าม FFI (name/blurb ภาษาอังกฤษ คู่กับ id เสมอ — โครงปัจจุบันเป็นแบบนี้อยู่แล้ว)
   กลายเป็น **fallback** ไม่ใช่ตัวแสดงผล. ไม่แตะ determinism, ไม่ bump `VERSION`.
2. **การแปลทั้งหมด resolve ฝั่ง Godot** ผ่านระบบ i18n ในตัว (CSV → `.translation`,
   `tr()` / `TranslationServer.set_locale`). Key มาจาก id ที่ sim ส่งมาคู่กันอยู่แล้ว.
3. **Missing key → แสดง EN fallback จาก sim** — ไม่มีวันจอว่าง/ขึ้น key ดิบ
   → migrate ได้ทีละจอ, ภาษาใหม่เพิ่มได้โดยไม่บล็อคของเดิม.

### Key convention

```
ITEM_<id>            ชื่อไอเทม            (จาก game_data.json)
MON_<id>             ชื่อมอนสเตอร์         (จาก game_data.json)
ZONE_<id>            ชื่อโซน              (จาก game_data.json)
SKILL_<id>           ชื่อสกิล             (จาก Rust catalog)
SKILL_<id>_B         blurb สกิล           (จาก Rust catalog)
UI_<screen>_<slug>   ข้อความ UI           (เขียนตรงใน strings.csv)
```

ข้อความ UI ที่ประกอบค่าตัวเลข → ใช้ placeholder (`"ATK +{v}"`) ห้าม concat แยกท่อน
(ลำดับคำต่างกันต่อภาษา). ตัวเลขเป็น integer ล้วนอยู่แล้ว ไม่มีปัญหา format.

### คอลัมน์ `flags` — ไม่ต้องแปลทุกอย่าง (ตัดสิน 2026-07-08)

CSV มีคอลัมน์ `flags` ถัดจาก key: **`fixed` = คง EN ทุกภาษา** (extractor ไม่นับ
เป็นค้างแปล, batch แปลข้าม, Loc โชว์ EN ผ่าน fallback). ค่า `=` ใน cell รายภาษา =
"จงใจใช้ EN เฉพาะภาษานี้" (ต่างจาก cell ว่าง = ยังไม่แปล).

Default ตาม namespace (แก้รายคีย์ได้ที่ cell เดียว):

| หมวด | default | เหตุผล |
|---|---|---|
| `ITEM_*` / `MON_*` / `SKILL_*` (ชื่อ) | `fixed` | ศัพท์เทรด/guide ข้ามภาษา + Steam Market ต้องชื่อเดียวทั่วโลก (ธรรมเนียม RO: Poring คือ Poring) |
| `SKILL_*_B` (blurb), `ZONE_*`, `UI_*` | แปล | ข้อความที่ต้องเข้าใจจริง = จุดที่การแปลให้คุณค่า |
| glossary ถาวร (ติด `fixed` ตอนคีย์เกิด): STR/AGI/VIT/INT/DEX/LUK, HP/SP, ATK/DEF/MATK/ASPD, Gil, Soul, ชื่อเกม | `fixed` | ศัพท์สากลของแนว RO — แปลแล้วอ่านยากขึ้น |

ผลปัจจุบัน: พื้นที่แปลจริง **74 จาก 383 key** (จะโตตาม UI_/ZONE_ ที่ทยอยเข้า)

### ชิ้นส่วน

> **สถานะ 2026-07-08: infra ทั้งชุดนี้ BUILT + ทดสอบบน boot จริงแล้ว** —
> strings.csv (383 key จาก extract จริง), loc_extract.py, sim-cli
> `--dump-strings`, `Loc` helper, ฟอนต์ Noto (Latin+Thai+CJK SC) เป็น
> `gui/theme/custom_font` แล้ว. จุดที่ต่างจากแผนเดิม: โหลด CSV **ตอน runtime**
> ผ่าน `Loc.install()` (เรียกจาก main._ready) แทน importer ของ Godot —
> ตัดขั้น import ที่มองไม่เห็น + ทำงานเหมือนกันทุก environment.
> ⚠️ ค้าง: เพิ่ม `i18n/*.csv` ใน export include filter ตอนตั้ง export preset,
> ฟอนต์ zh-TW/ja/ko แท้ (ตอนนี้ SC ตัวเดียว อ่านออกแต่ glyph style จีนแผ่นดินใหญ่),
> subset CJK ลดขนาด (16MB), และ migrate view เดิมมาเรียก `Loc.*` ทีละจอ

- `godot/i18n/strings.csv` — source of truth. คอลัมน์: `key,en,th,zh_CN,zh_TW,pt_BR,ru,ja,ko,es,de,fr,pl,tr,vi,id,it,uk,...`
  (EN คือต้นฉบับ; เพิ่มคอลัมน์ = เพิ่มภาษา — โครงเดียวรับได้ถึง 20 ภาษา).
  Godot import เป็น `.translation` อัตโนมัติ.
- `tools/loc_extract.py` — sync key: ดึง `name` จาก `game_data.json` + ชื่อ/blurb สกิล
  → เติม key ใหม่ลง CSV (ค่า EN), รายงาน key ที่ค้างแปล. สำหรับสกิล ให้ sim-cli เพิ่ม
  flag `--dump-strings` (พิมพ์ `id,name,blurb` จาก REGISTRY) เพื่อให้ Rust catalog
  เป็น source เดียว ไม่ parse Rust ด้วย regex.
- `godot/loc.gd` — `class_name Loc`, static functions เท่านั้น (ไม่ใช่ autoload —
  ไม่ผิดกฎ no-globals): `Loc.item(id, fallback)`, `Loc.skill(id, fallback)`, ฯลฯ
  = `tr(key)` + ถ้าไม่มีคำแปลคืน fallback ที่ sim ส่งมา.
- สลับภาษาใน `options_view.gd` (`TranslationServer.set_locale`) + จำค่าใน config
  ฝั่ง client (การแสดงผลล้วน ๆ — ไม่ใช่ InputEvent).

### ฟอนต์ — งานเล็ก (เช็คแล้ว 2026-07-07: UI ไม่ได้ใช้ฟอนต์พิกเซล)

UI ทั้งเกมใช้ฟอนต์ default ของ Godot (ไม่มี theme font ตั้งไว้);
`PixelOperator-Bold.ttf` โหลดเฉพาะ `arena.gd` = เลขดาเมจ (ตัวเลขล้วน ไม่กระทบ loc).

- [ ] ฟอนต์ default ของ Godot ครอบ Latin/Greek/Cyrillic แต่**ไม่มี CJK/ไทย** —
      Godot 4.1+ fallback ไปฟอนต์ระบบ (OS) อัตโนมัติ ซึ่ง "ใช้ได้" แต่หน้าตาต่างกัน
      ต่อเครื่อง และเครื่อง Linux/Steam Deck อาจไม่มีฟอนต์ CJK → **bundle fallback
      เอง** (Noto Sans Thai + Noto Sans CJK SC/TC/JP/KR) ตั้งเป็น default theme font
      ให้เหมือนกันทุกเครื่อง — งานตั้ง theme ครั้งเดียว
- [ ] ขนาดไฟล์ CJK (~MB ระดับสิบ) — พิจารณา subset ตาม strings.csv ถ้าซีเรียส
- [ ] Layout: จอง width เผื่อ ~30% (ru/de ยาว, zh สั้น) — ผ่าน playtest สลับภาษาจริง

## ภาษา (เป้า 10-20 ภาษา — user ตัดสิน 2026-07-07)

| Tier | ภาษา | ระดับ | เหตุผล |
|---|---|---|---|
| T1 — launch, แปล+review | EN, TH, **zh-CN**, **zh-TW** | คุณภาพเต็ม | zh = audience idle ใหญ่สุด (TBH/Banana); zh-TW แปลงจาก zh-CN + review สั้น, RO nostalgia ไต้หวันแรง |
| T2 — launch ถ้าทัน / fast-follow | **pt-BR**, ru, ja, ko | MT + native review | RO บราซิลใหญ่มาก; ru = idle audience ใหญ่; ja/ko = RO nostalgia แต่เซนซิทีฟกับ MT ห่วยสุด → review ก่อน ship |
| T3 — batch MT + spot-check, ทยอย ship | es-ES, es-419, de, fr, pl, tr, vi, id, it, uk | MT (Claude) + community review | ครบ top-20 ตลาด Steam; vi/id = SEA ที่ RO เคยแรง; string สั้น ๆ MT คุณภาพพอ ship ได้ |
| T4 — ตาม demand | cs, hu, nl, ... | ดูสัญญาณ wishlist/รีวิว | เพิ่มคอลัมน์ CSV = จบ |

รวม T1-T3 = **18 ภาษา** (อยู่ในเป้า 10-20). กติกา: **infrastructure รองรับทุกภาษา
ตั้งแต่วันแรก**; T1/T2 ต้องผ่าน native review ก่อน ship (ja/de/ko เซนซิทีฟสุด),
T3 ship จาก MT ที่ spot-check แล้ว + เปิดช่องรับแก้จากชุมชน — ภาษาห่วยบนหน้า store
แย่กว่าไม่มี แต่ UI string สั้น ๆ ของเกม idle คือเคสที่ MT รอดง่ายสุด.

Community review loop (ต้นทุนต่ำแบบ solo): CSV อยู่ในเกม/repo → ปุ่ม "report
translation" ใน options หรือช่องใน Discord/Steam discussion ต่อภาษา + เครดิตชื่อ
ผู้ช่วยแปลในเกม — playtest เดือน 2 คือแหล่ง recruit native reviewer ชั้นดี.

Steam page: localize page copy ทุกภาษาที่ ship ตั้งแต่ page live (Valve เสิร์ฟหน้า
ตามภูมิภาคอัตโนมัติ → เก็บ wishlist ต่างชาติทันที). Steam locale mapping:
schinese/tchinese/brazilian/koreana/latam.

## วินัยที่มีผลทันที (ก่อน infra เสร็จด้วยซ้ำ)

1. **ห้ามเพิ่ม user-facing literal ใหม่ใน `.gd`** — ใช้ key + `Loc.*` เสมอ
   (ของเก่า migrate ทีละจอ; ของใหม่ห้ามเพิ่มหนี้). เพิ่มกฎลง `CLAUDE.md` แล้ว.
2. content ใหม่ (item/skill/zone) → รัน `loc_extract` ก่อน commit ให้ key ตามทัน.
3. ห้ามฝัง text ใน texture/sprite (ตอนนี้ยังไม่มี — คงไว้).
4. Proper nouns ไม่แปล: ชื่อเกม, Gil, Soul, ชื่อเฉพาะ (ทับศัพท์ต่อภาษาใน glossary
   แถวบนสุดของ CSV เพื่อให้แปล batch คงเส้นคงวา).

## Timeline (ผูกกับ steam-launch-plan.md)

- **เดือน 1 (ก.ค.):** infra ทั้งชุด (~2-4 วัน: CSV + extract + `Loc` + font fallback)
  + กฎ no-literal มีผลทันที; migrate จอที่แตะอยู่แล้วไปพลาง
- **เดือน 2 (ส.ค.):** **string freeze ก่อน playtest** → แปล T1 batch เดียว;
  ทดสอบฟอนต์/overflow บน build จริง; localized page copy ขึ้นทุกภาษา T1
- **เดือน 3 (ก.ย.):** native review zh (จ้าง/ชุมชน), แก้ truncation จาก playtest,
  QA สลับภาษาทุกจอ; T2 แปลไว้รอ review — ship เมื่อผ่าน
- **Market update (second launch):** zh push คือหัวหอก — ภาษาไปรอก่อนแล้ว

## ไม่ localize

`debug_view.gd`, log/console, CSV ของ sim-cli, ชื่อ field ใน save/JSON ทุกชนิด
