# Quest System — architecture (sim-agnostic, server-brained)

**Status:** วางแผน (ยังไม่ลงมือ).
**Companion:** `rare-slot-purchase-design.md` + `craft-and-offline-design.md` (ตระกูลเดียวกัน — server-time-split),
`anti-cheat-and-server-model.md` (client-sim + verify), `competition-integrity.md` (window-diff จาก counter สะสม),
`server-plan-phases.md`.

## 0. หลักร่วม (server-time-split) — ใช้กับทุกฟีเจอร์อิงเวลา

sim เป็น **integer-only + wall-clock-free** (กฎเหล็ก). ฟีเจอร์ที่อิงเวลาจริง/เงิน (daily quest, 24h rare-slot,
weekend event, craft catch-up) แบ่งเส้นเดียวกัน:

- **เวลา + เงิน + entitlement = server-authoritative** — sim ไม่มี clock
- **การวัดผล = sim counter สะสม (deterministic)** — server อ่านค่าที่ verify แล้ว **แล้วตัด window เอง** (ไม่ re-simulate)
- **การเปลี่ยน state จากเวลา/เงิน = server-authored InputEvent** ลง log → apply ลง save → replay ยัง consistent
- **client = อ่าน projection มาวาด** (read-only), ไม่คำนวณ outcome

เควสคือ instance รูปธรรมของหลักนี้.

## 1. สามชั้น

| ชั้น | อยู่ที่ | ถือ |
|---|---|---|
| **Catalog** (static) | game_data / server config | นิยามเควส (ไม่มี state) |
| **Measurement** | **sim** (deterministic) | counter สะสมตลอดชีพ |
| **Quest state** | **server** (account-scoped) | active set / progress / reset / claim / streak |

**sim ไม่รู้จักเควสเลย** — เป็นแค่เครื่องยนต์ที่นับเลข. server = สมองเควส. client = โชว์ progress.

## 2. Catalog (static data)

```
quest_id, type, counter_key | event_key, target,
reward{gil|item|...}, cadence(daily|weekly|once), prereq[]
```
แก้/เพิ่มเควสได้อิสระฝั่ง server/data — **ไม่แตะ sim, ไม่ bump VERSION** (นี่คือเหตุผลหลักที่แยกออกมา).

## 3. Measurement = sim counters (deterministic)

```
GameState.counters: BTreeMap<CounterKey, u64>   // monotonic, serialized, ไม่ reset ใน sim
```
ตัวอย่าง key: `kills_of_race[Undead]`, `boss_kills[id]`, `dungeon_clears[id]`, `gil_earned_total`,
`items_refined`, `cards_collected`. objective เกือบทั้งหมดยุบเป็น counter ได้; reach-level = อ่าน stat ตรงๆ.

**discipline (เหมือน GearMod enum):** counter key = **append-only + monotonic + serialized**. เพิ่ม counter =
bump VERSION + re-bless goldens. counter **ห้าม reset ใน sim** — server ทำ windowing ด้วย baseline diff.

## 4. Quest state = server (account-scoped)

```
active_quests[]: { quest_id, accepted_at, baseline_snapshot, progress, status }
reset_at, streak, claimed[]
```
`progress = counter_now − baseline_snapshot` (window-diff — วิธีเดียวกับ competition cumulative-excess).

## 5. Flow

```
1. daily reset (server clock) → server roll เควสของวันให้ทุก account
                              → snapshot sim counters ปัจจุบัน = baseline
2. player เล่น → sim counter เดินหน้า deterministic
3. sync/save-push → server อ่าน counter (VERIFIED) → progress = now − baseline → เช็ค ≥ target
4. สำเร็จ + เคลม → server ยิง authored grant event → reward เข้า save ทาง mail → client เห็น
```
reward = **server-authored grant** (ทาง mail/grant เดิม) — ไม่ให้ client เคลมเอง.

## 6. objective 2 แบบ

| แบบ | ตัวอย่าง | กลไก |
|---|---|---|
| **counter** | ฆ่า N, หาเงิน M, refine K ครั้ง | server อ่าน delta — sim ไม่ต้องรู้จักเควส |
| **event/state** | ถึง Lv30, เคลียร์บอส Y | sim counter[boss]++ หรือ log discrete event → server match |

## 7. Client UX (สำคัญ)

client (GDScript) **อ่าน sim counter local + รายการเควส/target จาก server → วาด progress bar เอง**.
ไม่ round-trip server ทุกครั้งที่ฆ่า. แต่ **completion + reward = server ตัดสินเท่านั้น** → ไม่ผิดกฎ
"GDScript ห้ามคำนวณ gameplay outcome" (นี่คือ render projection ล้วน).

## 8. Anti-cheat (ฟรี)

progress วัดจาก **verified counter** → ปั๊ม kills → save replay ไม่ผ่าน → เควสไม่นับ.
reward = mint value → gate หลัง verify (sample / on-claim). ต่อยอด anti-cheat เดิม ไม่เพิ่มระบบใหม่.

## 9. ทำไมไม่เอาเควสเข้า sim

ถ้า quest progress อยู่ใน GameState + `tick()` เช็ค objective:
- ผูกเนื้อหาเควสเข้า deterministic core → แก้เควส 1 อัน = **bump VERSION + re-bless goldens ทุกครั้ง**
- reset รายวันทำใน sim ไม่ได้ (wall-clock)
- เควสเยอะ = golden เปราะ

แยกออก → **sim นิ่ง (แค่ counter), เควสเป็น content ฝั่ง server เพิ่ม/แก้/A-B ได้อิสระ**.

## 10. Cost ที่ scale

ถูก: server อ่าน counter + window-diff = DB op เบา **1 ครั้ง/วัน/คน** (ไม่ใช่ per-tick). ไม่มีการ simulate ฝั่ง server.
reward grant = append event. โหลดตาม account ไม่ใช่ per-action.

## 11. Open decisions

1. counter-key registry รอบแรกมีอะไรบ้าง (เริ่มจาก kills/gil/dungeon/refine/boss)
2. daily reset เวลาไหน (UTC vs โซนผู้เล่น) + streak นับ/รีเซ็ตยังไง
3. เคลม manual (กดรับ) หรือ auto-grant ตอนสำเร็จ
4. baseline snapshot เก็บที่ server ล้วน หรือ echo กลับ client เพื่อโชว์ progress ตั้งแต่ 0
5. weekly/event quest ใช้ pipeline เดียวกัน (แค่ cadence ต่าง) — ยืนยัน

## 12. ทำเมื่อไหร่

หลังมี server จริง (ตาม server-plan / production-server-plan). ฝั่ง **sim counters ทำล่วงหน้าได้** (deterministic,
ไม่ต้องรอ server) — วาง counter registry + increment ตอน kill/drop/refine ไว้ก่อน; ส่วน quest brain รอ server.
