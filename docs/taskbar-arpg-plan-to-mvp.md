# Taskbar Idle ARPG — Step-by-Step Plan to MVP

**Companion to:** `taskbar-arpg-gdd-phase0.md` (the GDD = *what & rules*; this plan = *order & how to execute*).
**For:** solo dev working in short sessions, paired with Claude Code.
**Goal of MVP:** a playable, fun, offline vertical-slice idle ARPG with real build depth — usable both as a personal proof and as a recruiting artifact.

---

## 0. วิธีใช้แผนนี้

- แผนนี้แตก milestone M0–M5 ของ GDD ออกเป็น task ย่อยที่ลงมือได้จริง + ลำดับ + ตัวอย่างวิธีสั่ง Claude Code
- ทำ **เรียงตาม milestone เสมอ** ห้ามข้าม แต่ภายใน track ขนานกันได้ (ดู §3)
- ไม่ทวน acceptance criteria ของ GDD ซ้ำ — เปิด GDD คู่กันเวลาเช็ก "done"

### โมเดลการทำงานกับ Claude Code

| บทบาท | ใคร |
|---|---|
| Architect / ผู้ตัดสินใจ design | คุณ (เป็นคนเดียวที่ตัดสิน scope + balance intent) |
| Implementer | Claude Code (เขียนโค้ดตาม GDD + task) |
| Reviewer ของ money-zone code | คุณ (drops/refine/rng/events/hash ต้องอ่านจนอธิบายได้ก่อน merge) |
| Playtester | คุณ (สนุกไหม / น่ามองไหม — ตัดสินด้วยความรู้สึกตัวเอง) |

**กฎเหล็กเวลาเปิด session กับ Claude Code:** เริ่มทุกครั้งด้วย "อ่าน `taskbar-arpg-gdd-phase0.md` ก่อน แล้วทำ [task]" — ให้มันถือ GDD เป็นแหล่งความจริงเสมอ ไม่ใช่ความจำจาก session ก่อน

---

## 1. จังหวะ & กฎทอง

- **Task เล็กพอจบใน 1 session สั้น** — แตกงานให้ commit ได้บ่อย ดีกว่า task ใหญ่ที่ค้างข้ามสัปดาห์
- **ทุก milestone จบด้วยของที่ verify ได้** — เห็นภาพ/รันเทสต์/เล่นได้ ไม่ใช่ "เขียนเสร็จแต่ยังไม่เห็นผล"
- **Vertical slice ก่อนเสมอ** — ให้เห็นลูป farm→loot→build ที่เล่นจริงเร็วที่สุด แล้วค่อยขยาย ไม่ใช่สร้างทุกระบบให้สมบูรณ์ทีละอันแยกกัน
- **เจอ idea ใหม่ระหว่างทาง → เข้า parking lot (§7) ไม่ implement ทันที**

---

## 2. Pre-flight (ก่อนแตะโค้ดบรรทัดแรก)

### 2.1 เคาะ decision ที่ block งานได้ (จาก GDD §8)

ตัดสินตอนนี้เท่าที่จำเป็น ที่เหลือปล่อยเป็น default ได้:

- [ ] **MVP party size** → แนะนำ **เริ่ม 1 ตัว** ใน M0–M3, ปลดช่องที่ 2 ใน M4 เพื่อทดสอบ synergy (สมดุลระหว่างพิสูจน์ core fantasy กับไม่ให้ M4 บวมเกิน)
- [ ] **Tick rate** → รับ default 100 ms ไปก่อน (เปลี่ยนทีหลังได้ ตราบใดที่ fix ต่อ save)
- [ ] **Player class model** → แนะนำ "weapon เป็นตัวกำหนด style" เพื่อให้ build อิสระตั้งแต่ต้น (เลี่ยง class lock)
- [ ] **Master palette** → เลือกได้ตอน M0 squint test (ยังไม่ต้องเคาะตอนนี้)

### 2.2 ติดตั้ง toolchain

- [ ] Rust (stable) + `cargo`
- [ ] Godot 4.x
- [ ] gdext (godot-rust binding) — ลองตัวอย่าง hello-world ให้คอมไพล์ผ่านก่อน เพื่อยืนยันว่า build chain ทำงาน (นี่คือ tech risk เงียบ ๆ ที่ควรเจอตั้งแต่วันแรก)
- [ ] Python 3 + Pillow (สำหรับ data pipeline + asset normalize)

### 2.3 Repo bootstrap

- [ ] สร้าง repo `taskbar-arpg`, วาง GDD ไว้ที่ root
- [ ] วางโครง workspace ตาม GDD §4.3 (ยังว่างได้ แค่มีโฟลเดอร์)
- [ ] commit แรก = GDD + โครงเปล่า

> 💡 Pre-flight ทั้งหมดนี้สั่ง Claude Code ช่วยได้ ยกเว้นการเคาะ decision (§2.1) ซึ่งเป็นงานคุณคนเดียว

---

## 3. ภาพรวม: Two-Track Plan

M0 (Godot) กับ M1 (Rust) **ไม่พึ่งกัน ทำขนานได้** — เบื่อ track ไหนสลับไปอีก track ได้ เหมาะกับการทำตามช่วงเวลา/อารมณ์ ทั้งสอง track มาบรรจบที่ **M3** (combat vertical slice) ซึ่งเป็นจุดที่ "เกมเริ่มเป็นเกม"

```
GODOT TRACK:  M0 (strip spike) → M0.5 (window shell + toggle) ─┐
                                                               ├─→ M3 ─→ M4 ─→ M5
RUST TRACK:   M1 (sim-core+CI) → M2 (data pipeline) ───────────┘
```

**ลำดับแนะนำถ้าทำทีละอย่าง:** M0 → M1 → M0.5 → M2 → M3 → M4 → M5
(เอา M0 ก่อนเพื่อ de-risk strip + ล็อก cell size เร็ว, แล้วลุย Rust track ต่อเนื่องยาว ๆ เพราะเป็นหัวใจ)

---

## 4. Milestone breakdown

### M0 — Strip de-risk spike + readability lock  *(Godot, ~2–3 วัน)*

**เป้า:** พิสูจน์ว่า strip ทำได้จริง + ล็อกความละเอียด art/grid ทั้งเกม ก่อนลงแรงที่อื่น

ลำดับทำ:
1. หน้าต่าง borderless / always-on-top / no-focus docked เหนือ taskbar (โค้ดตั้งต้นมีใน GDD)
2. วาง placeholder sprite ขยับไปมา + เปิด Low Processor Mode, cap FPS 30
3. รันคู่งานจริง วัด CPU/RAM + ทดสอบ focus/sleep/wake/multi-monitor
4. **Squint test:** เอา sprite ผู้สมัคร (จาก asset pack ฟรีที่โหลดมา) ย่อมาที่ strip height ถ่ายรูป ดูว่าหรี่ตามองแล้วแยกมอน 3–4 ตัวออกไหม
5. จดผล → ล็อก cell size + strip row height + normalize resolution

**ตัวอย่างสั่ง Claude Code:**
> "อ่าน GDD §3.1 + M0. ทำ Godot scene ที่เปิดหน้าต่าง strip ตาม spec (borderless/always-on-top/no-focus, docked เหนือ taskbar) มี sprite placeholder เดินไปมาบน grid 16×3 เปิด Low Processor Mode + cap FPS 30"

**Done = ?** ติ๊กครบ M0 ใน GDD + มีตัวเลข cell size ที่ล็อกแล้ว
**คุณทำเอง:** squint test, วัด CPU, ตัดสินว่า strip ผ่านไหม

> 🚦 **Decision gate 1:** strip เวิร์กไหม? ถ้าเจอปัญหาหนักแก้ไม่ได้ → ตัดเป็น windowed-only, จดเหตุผล, เดินต่อ (ไม่ใช่ blocker อีกต่อไปตาม pillar 3)

---

### M0.5 — Window shell + mode toggle  *(Godot)*

**เป้า:** window mode (พื้นที่ dev หลัก) + สลับ strip/window ได้

ลำดับทำ:
1. หน้าต่างปกติ resizable วาด placeholder grid arena
2. ปุ่ม toggle สลับ window ↔ strip (scene เดียว เปลี่ยน viewport/layout)
3. gdext โหลดใน Godot ได้ — เรียก `state_hash()` จาก GDScript แล้ว print (พิสูจน์ bridge ทำงาน)

**Done = ?** สลับโหมดได้ + GDScript คุยกับ Rust ได้

---

### M1 — sim-core skeleton + CI  *(Rust, หัวใจ — ทำช้า ๆ อ่านเข้าใจ)*

**เป้า:** วางรากฐาน deterministic + ตาข่าย golden test ตั้งแต่ยังเล็ก

ลำดับทำ:
1. Cargo workspace + crate `sim-core` เปล่า + `#![forbid(unsafe_code)]`
2. `rng.rs` — PCG32 + reference test vectors (จุดเรียน Rust จุดแรก: struct + integer ops)
3. `state.rs` + `tick.rs` — GameState ขยับ counter, serde
4. `hash.rs` — canonical hash ของ state
5. golden test ตัวแรก: seed → run N ticks → assert hash
6. CI: clippy `-D warnings`, rustfmt, รันเทสต์ทั้ง **windows-latest + ubuntu-latest** เทียบ hash

**ตัวอย่างสั่ง Claude Code:**
> "อ่าน GDD §4.1, §4.2, §7. ทำ crate sim-core: เริ่มจาก PCG32 ใน rng.rs พร้อม reference test vectors และคอมเมนต์อธิบายสำหรับคนเพิ่งเริ่ม Rust อธิบายให้ผมฟังว่าทำไมออกแบบ struct แบบนี้ก่อน merge"

**Done = ?** ติ๊กครบ M1 + CI เขียว 2 OS
**คุณทำเอง:** อ่าน rng.rs/hash.rs จนอธิบายได้ทุกบรรทัด (money-zone)

> ⚠️ M1 คือจุดที่ลงทุนเวลา "เขียนไปอ่านไป" จริง — อย่ารีบ ให้ Claude Code อธิบายทุก PR

---

### M2 — Data pipeline  *(Rust + Python)*

**เป้า:** content อยู่ใน CSV ไม่อยู่ในโค้ด ตั้งแต่ตัวแรก

ลำดับทำ:
1. ออกแบบ CSV schema (items/monsters/zones/drop_tables/refine_table/constants) — ใส่ scale ในชื่อคอลัมน์ (`crit_bp`, `speed_milli`) ตาม §4.2.1
2. `build_data.py` — validate (ID ซ้ำ, dangling ref, drop weight, refine rows) → emit `game_data.json` แบบ versioned
3. sim-core โหลด game_data — bad data ต้องพังตอน build ไม่ใช่ runtime
4. กรอก content ขั้นต่ำ: 10 items, 5 monsters, 2 zones

**Done = ?** แก้ตัวเลขใน CSV แล้วเกมเห็นค่าใหม่โดยไม่แตะโค้ด

---

### M3 — Combat vertical slice  *(จุดบรรจบ — "เกมเริ่มเป็นเกม")*

**เป้า:** เห็นลูป farm→loot ที่เล่นจริงบน strip + sim-cli วัด balance ได้

ลำดับทำ:
1. `grid.rs` — occupancy, BFS movement + tie-break ที่ document ไว้ (money-zone, อ่านให้เข้าใจ)
2. `combat.rs` — ATK−DEF + element triangle, integer ล้วน
3. `drops.rs` — roll จาก drop table ผ่าน PRNG (money-zone)
4. Godot render sim state บน strip grid + interpolate + hit juice
5. `sim-cli` — รัน 30 วันใน <10s พ่น CSV progression
6. golden tests: pathing / combat / drops / 1-day replay

**ตัวอย่างสั่ง Claude Code:**
> "อ่าน GDD §3.2, §3.2.1, §7. ทำ grid.rs (BFS + tie-break ตาม spec) แล้ว combat.rs (element triangle, integer) เขียน golden test คู่กันทุกอัน อธิบาย tie-break logic ให้ผมก่อน merge"

**Done = ?** ติ๊กครบ M3
**คุณทำเอง:** เล่นจริง + ดู progression CSV จาก sim-cli

> 🚦 **Decision gate 2 (สำคัญสุด):** ดู vertical slice แล้ว **สนุก/น่ามองไหม?** นี่คือ first "is this fun" check — ถ้าจืด หยุดปรับ core combat feel ก่อนไปต่อ อย่าหนีไปสร้าง feature เพิ่มเพื่อกลบความจืด

---

### M4 — Builds, refine + wall  *(deep core ตัวจริง)*

**เป้า:** พิสูจน์ว่า build depth สนุกจริง ด้วยตัวเลข ไม่ใช่ความหวัง

ลำดับทำ:
1. `refine.rs` — state machine ผ่าน input events, published odds (money-zone)
2. failure outcomes (nothing/downgrade/break) จาก data
3. stat allocation (free respec) เป็น input events
4. sockets + 10–15 cards drop จาก data + socketing เป็น input events
5. ปลดช่องตัวที่ 2 (ถ้าเลือก party 2 ตาม §2.1)
6. `sim-cli` build comparison report — พิสูจน์ ≥2 viable builds ในโซน 2
7. จูน wall โซน 2 ด้วย sim-cli ให้ refine + ปรับ build คือคำตอบ
8. offline catch-up (capped)

**Done = ?** ติ๊กครบ M4 + report ยืนยัน 2+ builds
**คุณทำเอง:** อ่าน refine.rs จนเข้าใจ, เล่นจริงดูว่าตัดสินใจ build สนุกไหม

> 🚦 **Decision gate 3:** build depth จริงไหม? ถ้า sim-cli บอกมี build เดียวกินรวด → จูน data จนเกิดทางเลือกจริงก่อนไป M5

---

### M5 — Playtest build  *(ออกสู่คนจริง)*

**เป้า:** เอาเกมไปวางหน้าคนจริง เก็บ retention number

ลำดับทำ:
1. save/load (snapshot + replay) ทนต่อ game_data version bump
2. ผสม normalized asset set (§6) — รัน normalize script กับ asset ที่ซื้อ/ฟรี
3. local analytics counter (sessions, days retained — ในเครื่อง ไม่ส่งออก)
4. export Windows → อัป itch.io (private/unlisted)
5. หาคนเล่น 10–30 คน (เริ่มจาก network VN/Discord ที่มี)

**Done = ?** มีบิลด์ที่คนนอกเล่นได้ + เริ่มมีตัวเลข retention

> 🚦 **Decision gate 4 (go/no-go):** retention (คนเปิดซ้ำกี่วันติด) ดีพอไหม?
> - ดี → เข้า Phase 1 + เริ่มหานักวาด (contract ไม่ใช่ rev-share)
> - ไม่ดี → กลับไปแก้ core loop ด้วย data ที่เพิ่งได้ ก่อนลงทุนเพิ่ม

---

## 5. Definition of MVP (เมื่อไหร่เรียกว่าเสร็จ)

MVP = ผ่าน M5 โดยมีครบทั้งหมดนี้:
- เล่นได้จริง offline บน strip + window mode
- ลูป farm → loot → build (stats/cards/refine) → wall → push ทำงานครบ
- มี build ที่ viable ต่างกัน ≥2 สาย (พิสูจน์ด้วย sim-cli)
- มีคนนอกเล่นแล้ว + มีตัวเลข retention เบื้องต้น
- architecture ครบ (deterministic, event log, golden test 2 OS) — พร้อมต่อ Phase 1–2 โดยไม่ rewrite

---

## 6. กับดักที่ต้องระวัง (จาก session ที่ผ่านมา)

- **Scope dreaming** — guild war / world boss / live broadcast / 16v16 ทั้งหมดอยู่ใน GDD §9 = **do not build** จนกว่าจะผ่าน MVP + มีผู้เล่นจริง ความฝันพวกนี้สนุกที่จะคิด อันตรายที่จะทำตอนนี้
- **GDScript แอบคำนวณ gameplay** — วันที่เผลอทำคือวันที่ client/server จะ drift ทีหลัง (§4.1 เป็นกำแพง)
- **"ซ่อมเทสต์" ด้วยการแก้ expected hash** — ถ้า golden hash เปลี่ยน ต้องเข้าใจว่าทำไม ไม่ใช่อัปเดตให้ผ่าน (§7.2)
- **สร้าง feature กลบความจืด** — ถ้า gate 2 บอก combat ไม่สนุก การเพิ่มระบบไม่ช่วย แก้ feel ก่อน

---

## 7. Parking Lot (ที่พักของไอเดียใหม่)

ไอเดียจะโผล่ระหว่างทางแน่นอน — วิธีจัดการที่ไม่ทำลาย momentum:

1. ไอเดียใหม่ → จดท้ายไฟล์นี้ใต้ "Parking Lot" ทันที **ไม่ implement**
2. ถ้ามันเกี่ยวกับอนาคต (war/economy/social) → ย้ายไป GDD §9 ตอนว่าง
3. ถ้ามันอยากแก้ scope ปัจจุบัน → รอ decision gate ถัดไป ค่อยพิจารณาพร้อม data
4. ถามตัวเองด้วย litmus test ของ pillar 4: "มันทำให้ build/refine ลึกขึ้นไหม" ถ้าไม่ → ค้างไว้

*(เขียนไอเดียใหม่ต่อจากบรรทัดนี้)*

---

## 8. Timeline (ประเมินแบบ relative — ลำดับสำคัญกว่าความเร็ว)

ทำในช่วงพักสั้น + บางวันข้าม velocity ไม่สม่ำเสมอ ตัวเลขนี้เป็น "effort blocks" ไม่ใช่ปฏิทิน:

| Milestone | Effort คร่าว ๆ | หมายเหตุ |
|---|---|---|
| Pre-flight | สั้น | setup + เคาะ decision |
| M0 + M0.5 | ~สัปดาห์ | Godot, สนุก, เห็นผลเร็ว |
| M1 | ~1–2 สัปดาห์ | ช้าได้ — เรียน Rust ไปด้วย |
| M2 | ~สัปดาห์ | งานตรงไปตรงมา |
| M3 | ~2 สัปดาห์ | จุดบรรจบ งานหนักสุดก้อนแรก |
| M4 | ~2–3 สัปดาห์ | deep core หนักสุด (โดยเฉพาะถ้าปลด party 2) |
| M5 | ~สัปดาห์ | + เวลารอ playtest feedback |

รวมคร่าว ๆ ใกล้เคียงกรอบ 6 เดือนใน GDD ถ้า velocity ไม่สม่ำเสมอ — เทียบ IdleOn ที่ EA 4 ปีครึ่ง ถือว่าเร็วมากแล้ว ไม่ต้องเร่ง

---

## Parking Lot
*(ไอเดียที่โผล่ระหว่างทาง — จดไว้ ไม่ทำตอนนี้)*

-
