# Taskbar Idle ARPG — Skills + Base/Job Level System (Design)

**Status:** Design — not yet implemented. Targets BETA-5 Phase 3 continuation (sim ≥ 0.0.14-alpha).
**Companion to:** `taskbar-arpg-gdd-phase0.md` (source of truth). This doc only *expands* the build-making deep core (GDD pillar 4) with RO-style jobs + skills; it doesn't override anything.
**Decisions locked (2026-06-15):** RO-style **Base Level + Job Level**; skill selection is a **deterministic priority list** (no AI randomness — GDD §4.2 rule "skill usage follows fixed, published rules").

---

## 0. หลักการ / how this fits

- **ป้อน deep core ไม่ใช่สร้าง core ใหม่.** สกิล + job เป็นส่วนหนึ่งของ build-making (pillar 4): build = stat × **skill** × element × gear × card × refine. ไม่ใช่ grind แยก — skill point มาจากการเล่น loop เดิม (ฆ่ามอน) ไม่มี mechanic ใหม่มากิน attention
- **Deterministic ทั้งหมด** (pillar 2 + §4.2). XP, level-up, skill selection, SP, cooldown — integer ล้วน, กฎตายตัว, replay ตรง. PRNG โผล่เฉพาะ proc ที่ประกาศ odds (crit/drop/refine) — สกิลที่มี proc ในอนาคตต้อง declared odds
- **สร้างบน `Damage` struct ที่มีแล้ว.** สกิล = action ที่ผลิต `Damage` (อาจหลายก้อน/AoE) + เงื่อนไข (SP/cooldown/range). ไม่รื้อ combat ใหม่

---

## 1. Dual progression — Base Level + Job Level (RO)

RO มีสองแถบ EXP จากการฆ่ามอนตัวเดียวกัน:

| | มาจาก | ให้ | Cap (เสนอ) |
|---|---|---|---|
| **Base Level** | Base EXP | **status points** (STR/AGI/…) + max HP/SP scaling | 99 |
| **Job Level** | Job EXP | **skill points** (เรียน/อัปสกิล) | ต่อ class (เริ่ม 50) |

**สถานะปัจจุบัน:** `Stats` มี `level` + `xp` + `points_unspent` แล้ว = Base Level system. ต้องเพิ่ม **Job** ขนาน:

```rust
// stats.rs — เพิ่มลง Stats (หรือแยกเป็น struct Progression ก็ได้)
pub job_level: u32,        // เริ่ม 1
pub job_xp: u64,           // XP สะสมเข้า job bar ปัจจุบัน
pub skill_points: u32,     // earned-but-unspent (1 ต่อ job level up)
```

**XP source:** monster ให้ทั้งสอง. เพิ่มลง `MonsterDef`/`MonsterTemplate`:
```rust
pub xp_reward: u32,        // base (มีแล้ว)
pub job_xp_reward: u32,    // ใหม่ — มัก ~50-70% ของ base ใน RO
```
> ทางเลือกเลี่ยง field ใหม่: derive `job_xp = ⌊base_xp × ratio⌋` ด้วย const ratio. ง่ายกว่าแต่ปรับ per-monster ไม่ได้ — **เลือก field แยกเพื่อ designer-tunable** (สอดคล้องตัวเลือกเดิมของโปรเจค)

**Curve:** แยกจาก base. เริ่ม linear แบบเดียวกับ base (`JOB_XP_BASE × job_level`) แล้วจูนทีหลัง. ตอน job cap → job_xp ไม่สะสม (overflow ทิ้ง หรือเก็บเป็น 0)

**Level-up loops:** `try_level_up` (base, มีแล้ว) + `try_job_level_up` (ใหม่, ให้ 1 skill point ต่อเลเวล). ฆ่า 1 ตัว → award ทั้งสอง → loop ทั้งสอง

---

## 2. Skill data model

```rust
// data.rs (catalog) — immutable definitions
pub struct SkillDef {
    pub id: String,
    pub unit_def: String,        // สกิลผูกกับ class ไหน
    pub max_level: u8,           // อัปได้ถึง (เช่น 5 / 10)
    pub req_job_level: u32,      // ปลดล็อกที่ job level เท่าไร
    pub req_skills: Vec<(String, u8)>,  // prereq: skill id ต้อง level ≥ n (skill tree) — เริ่มว่างได้
    // --- action profile (ต่อ skill level เป็น Vec ตาม level 1..max) ---
    pub sp_cost: Vec<u32>,       // ต่อ level
    pub cooldown_ticks: Vec<u32>,
    pub power_bp: Vec<u32>,      // % ของ ATK/MATK (10000 = 100%) ต่อ level
    pub range: i32,
    pub is_ranged: bool,
    pub proj_travel_ticks: u32,
    pub element: Element,        // หรือ inherit จากผู้ร่าย (None = ใช้ของตัวละคร)
    pub damage_type: DamageType,
    pub effect: SkillEffect,
}

pub enum SkillEffect {
    Damage,                          // single target
    AreaDamage { radius: i32 },      // ทุกศัตรูใน Chebyshev radius รอบเป้า
    // later: Heal, Buff{stat,amount,dur}, Dot{dmg,dur}, ...
}
```

**Learned skills (per-character, mutable):** เก็บใน `CharacterData` (per-class roster ที่มีแล้ว) เพราะสกิลผูก class:
```rust
// state.rs — CharacterData
pub skills: Vec<LearnedSkill>,   // หรือ BTreeMap<String, u8> (id → level) เพื่อ deterministic
pub struct LearnedSkill { pub id: String, pub level: u8 }
```

**เรียน/อัปสกิล:** `InputEvent::LevelSkill { entity_id, skill_id }` — ใช้ 1 skill_point ถ้า (job level ≥ req, prereq ครบ, ยังไม่ถึง max). idempotent no-op ถ้าไม่ผ่าน (replay-safe ตาม §4.4)

---

## 3. SP resource + regen

```rust
// Entity — combat section
pub sp: i32,        // current
pub max_sp: i32,    // base + INT (Stats::max_sp มีแล้ว = int*6)
```
- regen ทุก tick: `sp = min(max_sp, sp + SP_REGEN_PER_TICK)` (integer, อาจ accumulator แบบ move/atk)
- spend เมื่อร่ายสกิล; ไม่พอ → ข้ามสกิลนั้นใน selection
- respawn/zone change → refill sp เต็ม (เหมือน hp)

---

## 4. Deterministic skill selection (= "AI" ที่ defer ไว้)

แต่ละ swing (ใน `tick::attack_step` ตอน atk_acc ครบ 1000) เลือก action:

```
ordered = skills เรียงตาม PRIORITY (ลำดับใน learned list / ตั้งโดย user ทีหลัง)
for skill in ordered:
    if skill.cooldown_remaining == 0
       && sp >= skill.sp_cost[lvl]
       && target ใน skill.range
       && effect-condition ผ่าน (เช่น AoE ต้องมีศัตรู ≥ 1 ใน radius):
        ใช้สกิลนี้ → spend SP, set cooldown, ผลิต Damage ผ่าน build_damage (power_bp ปรับ amount)
        return
fallback → basic attack เดิม (ไม่กิน SP/cooldown)
```

- **Priority list อัตโนมัติ** (ตัวเลือกที่เคาะ): ไล่ตามลำดับ, ตัวแรกที่ใช้ได้ชนะ. ไม่มี random
- cooldown: เก็บ `Vec<u32>` ขนานกับ skills, ลด 1 ทุก tick (ถึง 0)
- power_bp ปรับ Damage: `amount = effective_atk_or_matk × power_bp / 10000` แล้วเข้า apply_damage ปกติ (def/mdef/element/crit เหมือนเดิม — crit เฉพาะ physical)
- AoE: produce Damage หลายก้อน ใส่ทุกเป้าใน radius (ผ่าน apply_damage ทีละตัว)

> selection เป็นกฎ published → ใส่ golden ได้ตรง (เลเวลสกิล + SP + cooldown state อยู่ใน save → state_hash ครอบ)

---

## 5. Determinism / golden impact

- field ใหม่บน Entity (`sp`, `max_sp`) + Stats (`job_level`, `job_xp`, `skill_points`) + CharacterData (`skills`) + MonsterTemplate (`job_xp_reward`) = **shape change → bump goldens** (ขออนุญาตเหมือน Phase 3). พฤติกรรมเดิมไม่เปลี่ยนถ้า default = 0/ว่าง
- SP regen ทำให้ทุก entity เปลี่ยน state ทุก tick → golden ที่รันหลาย tick จะมี sp ต่างจากเดิม (คาดไว้, re-pin)
- ไม่มี PRNG draw ใหม่จากแกนนี้ (selection deterministic; damage ใช้ stream เดิม). สกิลที่มี proc (stun%) ในอนาคต = declared-odds draw

---

## 6. แผน implement (vertical slices — ทำทีละเส้นให้ทะลุ)

1. **Job level + skill points** — เพิ่ม job_level/job_xp/skill_points + job_xp_reward + try_job_level_up. UI: job bar ข้าง base bar. (ยังไม่มีสกิลให้ใช้ point — แค่สะสม). golden ครอบ dual level-up
2. **SP resource** — sp/max_sp + regen ทุก tick + refill on respawn. UI: SP bar
3. **Skill spine (1 สกิล end-to-end)** — SkillDef + learned skills + LevelSkill event + selection priority + 1 สกิล (เช่น mage **Fire Bolt**: 250% MATK, sp cost, cooldown). golden ครอบ spend SP + cooldown + skill damage
4. **AoE effect** — SkillEffect::AreaDamage + สกิล AoE (เช่น **Fire Wall/Storm**) — ทดสอบ multi-target Damage
5. **Skill tree gating** — req_job_level + req_skills (prereq)
6. **UI setup** — หน้าจัดสกิล: เรียน/อัป (กด +), จัด priority order, SP/cooldown indicator บน arena
7. (ทีหลัง) effect อื่น: Heal / Buff / Dot / status

**เริ่มที่ slice 1** (job level) เพราะเป็นรากของ skill point — ต้องมีก่อนถึงจะเรียนสกิลได้

---

## 7. Open questions

- **Job cap + curve ต่อ class** — เริ่ม 50 + linear เหมือนกันก่อน แล้วค่อย per-class
- **Skill point ต่อ job level** — RO = 1/level. คงไว้ 1 ก่อน
- **Priority order ตั้งโดยใคร** — ตอนนี้ = ลำดับที่เรียน (default). user setup เป็น UI ทีหลัง (§6.6)
- **Element ของสกิล** — inherit จากผู้ร่าย หรือ fix ต่อสกิล (เช่น Fire Bolt = Fire เสมอ)? เสนอ fix ต่อสกิล, override ผู้ร่ายได้ทีหลังด้วย endow
- **Basic attack = skill[0] ไหม** — ตอนนี้แยก (fallback). รวมเป็น skill เดียวกันทำให้ model สวยขึ้นแต่รื้อเยอะ — ไว้พิจารณาตอน multi-attack โต
- **Multi-hit / cast time** — RO มี cast time + hit count. เลื่อนไป เพราะ idle ไม่เน้น cast bar
