# Session 2026-06-14 — BETA-4 Archer + Arrow Projectile

**Status:** ✅ ปิด BETA-4 (ranged attacks)
**sim-core version:** 0.0.10-alpha
**Test count:** 55 unit + 6 goldens (alpha2-6 + beta4) — ผ่านครบ

---

## สรุป 1 บรรทัด

เพิ่ม archer class ที่ยิงลูกธนู — sim ตัดสินจริง (damage lock ตอนยิง, ตัดสินผลตอนถึง), Godot แสดงลูกธนูบินจริง

---

## งานที่ทำ

### 1. `resolve_arrived_arrows` ใน `crates/sim-core/src/tick.rs`

ฟังก์ชันใหม่ที่รันท้ายทุก tick:

```rust
fn resolve_arrived_arrows(state: &mut GameState) {
    let mut fired: Vec<Arrow> = Vec::new();
    state.arrows.retain(|a| {
        if a.arrive_at_tick <= state.tick {
            fired.push(a.clone());
            false
        } else { true }
    });
    for arrow in fired {
        let Some(tidx) = state.grid.idx_of_id(arrow.target_id) else { continue; };
        if !state.grid.entities[tidx].alive { continue; }  // whiff
        let t = &mut state.grid.entities[tidx];
        t.hp = t.hp.saturating_sub(arrow.damage);
        if t.hp <= 0 {
            t.alive = false;
            drops::drop_on_death(state, tidx);
            handle_monster_death(state, tidx);
        }
    }
}
```

**Design rules ที่ pin:**
- Damage **lock ตอนยิง** ไม่ใช่ตอนถึง — attacker เปลี่ยน gear/class ระหว่างทาง = ไม่กระทบ
- Target tracking ด้วย **id** ไม่ใช่ position — homing damage (เป้าวิ่งหนีก็โดน)
- ถ้าเป้าตายก่อนลูกถึง → **whiff** (ลูกบินจบเปล่า ๆ ลบทิ้ง)
- Resolve **หลัง** entity tick ของแต่ละ tick → เป้าได้ขยับก่อน แล้วลูกค่อยลง
- Order preserve ด้วย `retain` + temp Vec → renderer interpolate ไม่กระตุก

### 2. Render ลูกธนูใน `godot/main.gd`

- โหลด `res://assets/arrow.png` ตอน `_ready`
- ฟังก์ชันใหม่ `_draw_arrows(t)` — iterate `state.arrows`, lerp `spawn_pos→target_pos` ด้วย `(now_tick - spawn_tick + t) / total_ticks`
- Rotate ด้วย `atan2((dst - src))` (รูปต้นฉบับชี้ขวา = 0 rad)
- Fallback: เส้นสีทองถ้า texture โหลดไม่สำเร็จ

### 3. Archer facing bug fix

อาการ: archer ยืนยิงเป้าทางซ้ายแต่ตัวหันขวา
เหตุ: facing update จาก horizontal motion อย่างเดียว — ranged class ที่ไม่ขยับเลย = ค้าง facing เดิม
แก้: เพิ่ม priority "ถ้าไม่ขยับและ engaged → หันตาม target.x"

### 4. Asset

- เพิ่ม `godot/assets/arrow.png` (หัวลูกธนูชี้ขวา)
- รวม sprite sheets ปัจจุบัน: `player_sheet.png` (swordsman), `archer_sheet.png`, `arrow.png`

---

## การทดสอบ

### Unit tests ใหม่ใน `tick.rs`

5 cases ใน `arrow_tests` module:
- `arrow_in_flight_does_not_apply_damage_early`
- `arrow_applies_damage_on_arrival_tick`
- `arrow_whiffs_if_target_already_dead`
- `arrow_that_kills_triggers_drop_and_kill_counter`
- `arrows_resolve_in_fifo_order`

### Integration test

`tests/golden_beta4.rs` — pin scenario: archer (Fire, range 5, travel 3) vs stationary slime (Earth) → arrow kill, drop fires, hash pinned

```rust
const BETA4_EXPECTED_HASH: &str =
    "f98c0d8a1fb1ccf0307a1f8ffca646e40bd59fdff1c474c2826cbc64a943350a";
```

### Golden hash bumps (GDD §7.2 — ขออนุญาตแล้ว)

BETA-4 เพิ่ม `proj_travel_ticks: u32` ลง Entity → JSON shape เปลี่ยน → hash ของ alpha2-6 ทุกตัวขยับ
**ไม่ใช่ behavior change** (default 0 = melee, ทุก scenario เก่าใช้ค่านี้)

| Test | เก่า | ใหม่ |
|---|---|---|
| alpha2 | `2cc320...` | `3e5960...` |
| alpha3 | `169a6b...` | `9de95d...` |
| alpha4 | `161510...` | `bf408b...` |
| alpha5 | `8448c2...` | `dcc4c8...` |
| alpha6 | `146857...` | `c2bedc...` |

---

## ไฟล์ที่แก้

```
crates/sim-core/src/tick.rs            +90 บรรทัด (resolve + 5 tests)
crates/sim-core/src/data.rs            +4  บรรทัด (test fixture proj_travel_ticks=0)
crates/sim-core/src/spawn.rs           +1  บรรทัด (test fixture)
crates/sim-core/tests/golden_alpha2..6 hash + comment ทุกไฟล์
crates/sim-core/tests/golden_beta4.rs  ไฟล์ใหม่
godot/main.gd                          +60 บรรทัด (texture load, draw, facing fix)
godot/assets/arrow.png                 asset ใหม่
godot/bin/godot_ext.dll                rebuild
```

---

## Design decisions ที่ขีดเส้น

### Travel time = ค่าคงที่ต่อคลาส (ไม่ขึ้นกับระยะ)

```json
"archer_t1": { "atk_range": 5, "proj_travel_ticks": 3 }
```

ระยะ 1 cell vs 5 cell = ใช้เวลา 3 ticks เท่ากัน — เพราะ:
- Integer-only (GDD §4.2 rule)
- Balance ง่ายกว่า ("คลาส archer = ช้าตัดสิน 300ms ตาย ๆ")
- ไม่ต้องคิด physics velocity

### Visual endpoint ≠ damage target

- Damage: homing by `target_id` (ลูกธนู always hit ถ้าเป้ายังมีชีวิต)
- Visual: lerp ไปยัง `target_pos` ที่ snapshot ตอนยิง

ผลที่ตามมา: ศัตรูวิ่งหนีหลายเซลล์ → ลูกธนูบินไปลงพื้นว่าง, damage popup เด้งห่างออกไป
ในทาง gameplay ปัจจุบัน monster max speed = 500 milli = 0.5 cell/tick, travel = 3 ticks → ห่างสุด 1.5 cells (ตาเห็นยาก)

อนาคต ถ้าอยากให้ visual ตาม target จริง ๆ → renderer ใช้ `curr_positions[target_id]` แทน `target_pos`

---

## ก้าวต่อไป (ยังไม่ทำ)

หัวข้อ BETA ใน `taskbar-arpg-alpha-first-order.md` ที่ค้าง:
- **Build depth: stat allocation** (deep core)
- **Build depth: sockets + cards** (deep core)
- **sim-cli build comparison** (พิสูจน์ ≥2 viable builds)
- Polish: hit spark, element trail, sound

ดู Fun-check 2 (decision gate 3) — build depth พอลึกไหมที่ทำให้เล่นยาว

---

## Tasks closed ใน session นี้

1. ✅ Implement `resolve_arrived_arrows` in sim-core
2. ✅ Render arrows in Godot
3. ✅ Verify arrow damage timing visually (รวม facing fix)
4. ✅ Bump golden hashes alpha2-6 for BETA-4 schema change
5. ✅ Add unit tests for `resolve_arrived_arrows`
6. ✅ Add `golden_beta4` integration test
