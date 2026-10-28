# Hand-off — Card Album + Card Panel (2026-07-03)

สรุปสถานะเพื่อเปิด session ใหม่ต่อ. **โค้ดทั้งหมดยังไม่ commit** (last commit = `f49be5e`).
`cargo test --workspace` + goldens เขียวทั้งหมด, build ทั้ง workspace ผ่าน, dylib ถูก copy แล้ว.

**อัปเดต (session รีวิว codebase, วันเดียวกัน):** แก้บั๊กจาก review 3 ตัว → VERSION `0.0.128-alpha`:
1. **Boss spawn ได้ template เต็ม** — `try_spawn_boss` route ผ่าน `build_and_spawn_monster`
   (เดิม copy มือ ทำ mdef/hit/flee/atk-variance/skills หาย → boss อ่อนกว่า data ทุกตัว).
   Boss แข็งขึ้นจริง → **band pacing ต้อง re-check boss walls**. Re-bless golden `alpha6` (ตัวเดียวที่ถึง boss).
2. **Foil = คนละ stack** — `same_card_stack` (def_id+foil+star) gate ทั้ง `bank_card_to_album`
   และ merge-guard ใน `unequip_card` (เดิม merge by def_id → foil bit หาย/ติดฟรี).
3. **Consume card ย้ายมาหน้า Cards [D]** — right-click การ์ดใน OWNED = eat (+hint บรรทัดล่างตอน hover);
   ตัด flow เก่าใน sidebar (pick-card/slot/USE tag + `slot_card*` shims) ที่ตายเพราะการ์ดไม่เข้า stash แล้ว.
   Test ใหม่ 3 ตัว: boss-template, foil-album-stack, unequip-foil-guard.

---

## 1. Model ที่ตกลงกันไว้ (สำคัญ — อย่าสับสนชื่อ)

3 storage แยกกันชัดเจน:

- **Inventory** = ตัวที่มีอยู่เดิม ในโค้ดคือ `GameState.stash` (20 ช่อง). กระเป๋าหลักที่ loot เข้ามา, ใช้/สวม/ขายจากตรงนี้. **ยังคงชื่อ `stash` ในโค้ดไว้ก่อน** (rename → `inventory` เป็นงานรอบหน้า).
- **Stash** = ระบบ **ใหม่ (ยังไม่ทำ)** = คลังฝากแบบ **หลาย Tab**. Album จะเป็น 1 tab ในนี้. → งานอนาคต.
- **Card Album** = container การ์ดแยก (dex-style) — **ทำเสร็จรอบนี้** (sim + ท่อ UI).

Scope ที่ผู้ใช้เลือก: "ทำ Card Album tab ก่อน" — ตัวคลัง Stash หลายแท็บยังไม่ทำ.

## 2. ทำอะไรไปแล้วรอบนี้ (Card Album — sim เสร็จ)

**sim-core:**
- `state.rs`: `GameState.card_album: Vec<Item>` + `card_album_cap: u32` (skip-if empty/0),
  const `CARD_ALBUM_BASE_CAP = 30` (type-slots, upgrade ได้), method `card_album_cap()`.
- `drops.rs`: `bank_to_stash_inner` ต้นฟังก์ชัน redirect `ItemKind::Card` → `bank_card_to_album`
  (merge by def_id / push ถ้ามีที่ / `overflow_recover` ที่แยกออกมา share กับ stash).
  → ทุกทางเข้า (drop / shop buy / loot / unequip) การ์ดไป album หมด.
- `events/handlers.rs`: `equip_card` / `consume_card` อ่านจาก `card_album`; `unequip_card` คืน album
  และ **no-op ถ้า album เต็ม + เป็นชนิดใหม่** (การ์ดใน panel ไม่มีวันหาย — แก้บั๊ก "unequip ตอน inventory เต็ม → การ์ดหาย").
- `events/mod.rs`: `EquipCard`/`ConsumeCard` field `stash_idx` → `album_idx` (มี `#[serde(alias="stash_idx")]`).
- `data/build/items.rs` (`migrate_save_items`): migrate การ์ด + **sweep การ์ดที่ค้างใน stash → album** ตอน load (save เดิม).
- `lib.rs`: VERSION → `0.0.127-alpha` + changelog. **Goldens ไม่ต้อง re-bless** (ไม่มีการ์ด bank ระหว่าง golden scenario).

**bridge (godot-ext):**
- `lib.rs` view_state: strip `card_album` → count + inject `card_album_cap`.
- `bridge_items.rs`: getter `card_album_json()`.

**Godot:**
- `sim_driver.gd`: `func card_album()`; event key `album_idx`.
- `card_panel_view.gd`: คอลัมน์ OWNED อ่านจาก `card_album()` (`_album_cache`/`_refresh_album`/`album_rects`).
  + รอบก่อนหน้า: panel เป็นการ์ดใหญ่ 2 ฝั่ง + effect box "การ์ดทำอะไร" ข้างล่าง + idle animation reusable ทุก monster
  (`_mon_idle` ใช้ catalog เดียวกับ arena) + เปลี่ยน `[locked]` → `[inactive]`.

## 3. งานถัดไป (ที่ค้าง — จุดที่หยุด)

**หน้า Album UI ยังไม่มี** — ตอนนี้ album โผล่แค่ในคอลัมน์ OWNED ของหน้า Cards ([D]) และ filter เฉพาะ class ที่เลือก.
ยังไม่มี: หน้า dex รวมทุก class, ตัวนับ used/cap (เช่น 3/30), แท็บ Album ของตัวเอง.
ผู้ใช้ยังไม่ตัดสินว่าจะเอาแบบไหน — 3 ตัวเลือกที่เสนอไว้:
  (a) แท็บใน Cards (ปุ่มสลับ Panel ↔ Album, โชว์ทุก class + used/cap)
  (b) หน้า/ปุ่มแยก (กด A, dex เต็มจอ, equip ชัดเจน)
  (c) ไม่ต้องทำ (OWNED พอแล้ว)
มี `card_album_cap` ส่งมาใน view_state แล้ว พร้อมใช้โชว์ used/cap.

**ค้างอื่น ๆ (deferred):**
- Rename `stash` → `inventory` + สร้างระบบ **Stash หลายแท็บ** (Album เป็น 1 tab) — งานใหญ่รอบหน้า.
- ไม่มี gear ชิ้นไหนมี **card socket** เลย (`grep sockets game_data.json` = 0) → การ์ดทุกใบ inactive ตลอด.
  ต้องมีแหล่ง socket (เช่น gear "[1]" หรือ refine เจาะรู) การ์ดถึงจะ active จริง — ยังไม่ออกแบบ.
- Card star/fuse leveling, Epic/Foil variant leveling — ยังไม่ทำ (field `star`/`foil` มีแล้ว).
- Drop-system 2-axis (docs/drop-system.md), rarity Epic tier — ยังไม่ implement.

## 4. Verify / Build

```bash
cargo test -p sim-core --lib          # 240 unit
cargo test -p sim-core --tests        # + goldens (เขียวหมด, ไม่ต้อง re-bless)
cargo build -p godot-ext && cp target/debug/libgodot_ext.dylib godot/bin/libgodot_ext.dylib
/Applications/Godot.app/Contents/MacOS/Godot --headless --path godot --import   # เช็ค parse
```

Save migration: การ์ดใน save เดิมย้ายเข้า album อัตโนมัติตอน load (idempotent).

## 5. Memory guidance ที่ใช้อยู่
สื่อสารภาษาไทย; verify observation ก่อน diagnosis; ไม่ใช้คำ "ล็อค/final" (alpha);
verify ใน Rust core (test/golden/sim-cli) ไม่ต้องเปิดเกม; commit เฉพาะตอนผู้ใช้สั่ง.
