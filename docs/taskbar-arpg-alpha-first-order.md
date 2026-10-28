# Taskbar Idle ARPG — Alpha-First Execution Order

**Companion to:** `taskbar-arpg-plan-to-mvp.md` (this does NOT replace it — it re-orders the same tasks through one lens: *reach the "is it fun?" signal as fast as possible, on colored boxes, without paying any rewrite debt*).
**Use when:** you want momentum and an early go/no-go, not completeness.

---

## หลักการ

1. **Optimize เพื่อ signal ไม่ใช่ completeness** — เป้าหมายของ ALPHA คือตอบคำถามเดียว: "loop + the hook มี pull ไหม" ให้เร็วที่สุด ทุกอย่างที่ไม่ช่วยตอบคำถามนี้ → เลื่อน
2. **แยก fun-check เป็นสองจังหวะ** — เกม idle มีสองคำถามที่ต่างกัน อย่ารอตอบทั้งคู่พร้อมกัน:
   - **Fun-check 1 (หลัง ALPHA):** loop พื้นฐาน + refine มี "ลุ้น/อยากเล่นต่อ" ไหม
   - **Fun-check 2 (หลัง BETA):** build depth ลึก/มี replayability ไหม (= decision gate 3 เดิม)
3. **เลื่อน "ความครบ" ได้ แต่ห้ามเลื่อน "ความถูกของแกน"** — ดู §ห้ามเลื่อน

---

## ⚠️ อย่า prototype ใน GDScript เพื่อความเร็ว

เป็น temptation ที่แรงมาก ("เขียน loop ใน GDScript ก่อน เร็วกว่า แล้วค่อยย้าย Rust") — **อย่าทำ** ด้วยสองเหตุผล:

1. **มันตอบ fun-check ผิด** — ความสนุกของเกม idle ไม่ได้อยู่ที่ action juice (เกม auto-battle ไม่มี action ให้กด) แต่อยู่ที่ **progression curve + build decision** ซึ่งต้องการ sim จริง + ตัวเลขจริง ถ้า prototype ด้วย juice สวย ๆ แต่ progression ปลอม จะได้คำตอบหลอก ("สวยดี") แล้วตายตอนเล่นจริงวันที่ 3
2. **มันคือ rewrite + ขัด thesis** — sim ต้องเป็น Rust เพื่อ server reuse (§9 one-binary). เขียน GDScript ก่อน = ทิ้งทีหลัง

ทางที่ถูก: sim-core Rust แบบ **minimal** ตั้งแต่ ALPHA — เล็กแต่จริง

---

## 🔒 ห้ามเลื่อน (ข้ามแล้ว = rewrite ภายหลัง)

ทำให้ถูกตั้งแต่บรรทัดแรกของ ALPHA แม้จะ minimal:
- Sim เป็น Rust (sim-core), integer-only, PRNG เดียว
- Sim/render แยกเด็ดขาด — Godot ห้ามคำนวณ gameplay (อ่าน `view_state()` เท่านั้น)
- Grid integer + tick architecture + event log
- gdext bridge (Rust↔Godot) ตั้งแต่ชิ้นแรก
- Golden test แบบ **local** (รันเครื่องตัวเองพอ ยังไม่ต้อง CI 2 OS) — เป็น safety net ตอน refactor

## 🕓 เลื่อนได้ (เก็บใน BETA/GAMMA)

- Strip mode + squint test + cell-size lock (M0 เดิม) — strip คือ *marketing* ไม่ใช่ *gameplay* เลื่อนไปทำตอนใกล้โชว์คน
- CI 2-OS + golden coverage เต็ม
- `build_data.py` validation เต็ม (ALPHA ใช้ JSON เขียนมือก่อนได้)
- Cards + socket + stat allocation + party (deep core — เก็บไว้ BETA ตอบ fun-check 2)
- Offline catch-up, save/load robust, version migration
- Asset pipeline + normalize (GAMMA)

---

## PHASE ALPHA — ไปให้ถึง Fun-check 1 (กล่องสี)

ลำดับ task (แต่ละอันเล็กพอจบใน session สั้น):

1. **gdext hello-world** — Rust↔Godot คุยกัน เรียก `state_hash()` จาก GDScript print ได้ (พิสูจน์ build chain — tech risk เงียบ)
2. **sim-core minimal** — `state` + `tick` + PCG32 + grid + 1 entity ขยับหนึ่งช่องต่อ move-interval. golden test local ตัวแรก
3. **combat minimal** — ATK−DEF + element triangle, integer, ใน sim-core. entity ที่สองเป็นมอนสเตอร์ ตีกันได้ ตายได้
4. **drop minimal** — ดรอปจาก PRNG, equip แล้ว stat เปลี่ยน (ตัวแข็งขึ้นจริง)
5. **refine minimal (the hook)** — ตีบวก +N เสี่ยงแตก, published odds. นี่คือ decision ที่ทำให้ loop มี "ลุ้น" — ขาดอันนี้ fun-check จะตอบแค่ "ฟาร์มเฉย ๆ น่าเบื่อ" ซึ่งรู้อยู่แล้ว
6. **game_data.json เขียนมือ** — ~5 monster, ~5 item, 2 zone (ยังไม่ต้อง build_data.py)
7. **Godot render บน window** — กล่องสีตาม sim state: party (วงกลม), มอน (สี่เหลี่ยมสีตามธาตุ), HP bar, ตัวเลขดาเมจเด้ง, ของดรอป. window mode พอ — **ไม่ต้อง strip**
8. **zone progression** — โซน 2 แข็งกว่า, ต้อง refine/หาของก่อนไปต่อ (กำแพงแรก)
9. **sim-cli minimal** — รัน N วัน พ่น progression curve CSV

### 🚦 FUN-CHECK 1
เล่นจริง 2-3 วัน (คุณนัทคือ target user) + ดู progression curve:
- loop farm→drop→refine→แข็งขึ้น→โซนต่อ มี pull ไหม
- จังหวะ refine (ลุ้นแตก/สำเร็จ) สนุกไหม
- curve น่าเล่นต่อหรือตันเร็ว

**ผ่าน → BETA. ไม่ผ่าน → จูน core loop/refine feel ก่อน อย่าเพิ่งเติม build depth กลบ**

---

## PHASE BETA — Build depth + เก็บหนี้ → Fun-check 2

ตอบคำถามที่สอง: build decision ลึกพอให้เล่นยาวไหม
- **Build depth (deep core):** stat allocation, sockets + 10-15 cards, (party slot 2 ถ้าเอา)
- **sim-cli build comparison** — พิสูจน์ ≥2 viable builds โซน 2
- **เก็บหนี้ technical:** `build_data.py` validation เต็ม, CI 2-OS, golden coverage เต็ม, save/load robust, offline catch-up

### 🚦 FUN-CHECK 2 (= decision gate 3 เดิม)
build สนุก/มีทางเลือกจริงไหม (ไม่ใช่ build เดียวกินรวด) — **ผ่าน → GAMMA**

---

## PHASE GAMMA — โชว์คนได้ (= M5 + strip)

- **Strip mode + squint test + cell-size lock** — ทำตอนนี้เพราะใกล้โชว์/ทำคลิปแล้ว (marketing wedge)
- Asset pipeline + normalize asset pack (ฟรี/AI) แทนกล่องสี
- itch.io playtest build (private) + local analytics
- หาคนเล่น 10-30 คน → retention number → go/no-go Phase 1

---

## สรุปความต่างจาก plan-to-mvp เดิม

| | plan-to-mvp (เดิม) | alpha-first (อันนี้) |
|---|---|---|
| ลำดับเริ่ม | M0 strip ก่อน | gdext + sim ก่อน, strip เลื่อนไป GAMMA |
| fun-check | จุดเดียวหลัง M3 | สองจุด (loop+hook เร็ว, แล้ว build depth) |
| data pipeline | M2 เต็มก่อน combat | JSON มือก่อน, validation ทีหลัง |
| เป้า | ครบทุก milestone | signal เร็วสุดด้วยหนี้ที่จ่ายคืนได้ |

หลักเดียวกันทั้งสองเอกสาร: architecture ห้ามเป็นหนี้, content/polish/strip เป็นหนี้ได้ชั่วคราว
