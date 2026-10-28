# TODO — idle_mmorpg

รายการงาน active (เรียงตามที่ตกลงกัน). รายละเอียดออกแบบลึกอยู่ใน
`taskbar-arpg-*.md`; ของที่ทำเสร็จย้ายลง "Done log" ท้ายไฟล์.

## Next up

### 🌐 Online hardening + Social/MMO layer (added 2026-07-08)
> Online save/resume works end-to-end (server-roll refine + Neon persistence + anchor +
> seed-string + version-sync all landed + proven in-game; account 7 saved at tick 1600).
> **Robustness layer DONE + DEPLOYED (0.0.183, 2026-07-09) — current architecture =
> `online-robustness.md`.** What remains for a real online game:
- ✅ **Save-reject handling** (DONE) — `_push_online` categorizes: superseded→modal /
  version-skew→modal / divergence→auto-revert+notify / network→banner; doomed session
  PAUSED. ✅ **Version check at LOGIN** (not just save-time). ✅ **Single active session**
  (new login supersedes the old token). ✅ **Taint-on-divergence** (integrity-rejected
  save → account tainted → `/refine/roll` refused until it recovers). ✅ **Orphan-roll
  prune** (`created_at` + 1h TTL + startup sweep + prune-on-issue).
- ⚠️ **In-game visual/functional test of all the above still PENDING** (USER runs it —
  never launch on a real save; parse-verified via `--import` only).
- **Server-roll refine — #2 later:** promotion-path reuse `verify_refine_binding`
  (register/ship) — **deferred until the market is live** (no promotion path exists yet +
  the save-verify already makes the stored refine trustworthy; build it WITH the market).
  Prune `consumed` on anchor-advance; fold the bind-check into the single `verified_state`
  replay.
- **Gate the debug menu in production** (verifier already rejects Debug events, so it's
  defense-in-depth / hide-from-players, not a security hole).
- **Real-time PUSH CHANNEL (WebSocket/SSE)** — the foundation for the whole social layer.
  One build unlocks all of:
  - **Global chat** (relay + moderation)
  - **Friend system** (relationships in Neon + presence "who's online" + whisper)
  - **+10 global announce** (server knows every roll → a trustworthy broadcast; a
    client-roll couldn't do it — see server-roll-refine memory)
- **Economy — next valuable RNG (before market):** server-roll rare/tradeable DROPS +
  card drops; `instance_id` at DROP time (currently lazy-at-refine, only refinable gear).
- Pre-flight: `tools/check_online_sync.sh` (tree vs client-dylib vs deployed-server sim
  version) — run before an online test to avoid the version-skew that silently rejects saves.

### 🔒 Server-roll VALUABLE RNG before the tradeable market goes live
> **Gate:** must ship BEFORE refined/rare gear has LIVE tradeable value (Gil market
> live / real-money bridge). Offline/alpha = client-roll is fine (manip only hits
> your own PvE). See input-authority-security memory + s3-economy-design.md §6.4.
- **REFINE (ตีบวก) — HIGH priority.** A client shadow-sim can predict the PCG32
  draw and refine ONLY on a success roll → bypasses the published odds (100% success,
  never breaks) → free +10 tradeable gear, defeats the break-scarcity gate, floods
  the market. replay-verify does NOT catch it (legal inputs). FIX: the SERVER rolls
  against the same `refine::odds_for` table with its own RNG → grants the outcome as
  a Server-authority event (published "no secret gacha" odds stay honest).
  - ✅ **Phase 1 sim foundation (0.0.175):** `StartRefine{,Inventory}` carry
    `server_roll: u64` — non-zero rolls the outcome from the nonce (zero PRNG draws,
    unsteerable, replay-safe), zero = offline client-roll. `authority()` Server/Client.
    Serde-skip-at-0 → goldens byte-identical. See s3 §6.4.1.
  - ⏳ **Phase 2 (verify-server):** `instance_id` on Item (schema add) → server nonce
    = keyed-hash(secret, account, item_instance_id, refine_level) BOUND to that exact
    item+level (⚠️ closes the target-selection re-steer: a free-floating revealed roll
    lets the client route good rolls to hard refines / burn bad rolls on 100%-safe +0s).
    Endpoints: `POST /refine/roll` + verify-time recompute+bind-check. See s3 §6.4.1.
- Same treatment for rare/tradeable DROPS + card drops + lootboxes (move `roll_drop`
  for valuable items to server-authored). Common/low-value stays client-roll (EV too
  low to tool).

### ⭐ Gameplay loop: 3-room instances + boss-gate + auto-advance/farm
> **Status (0.0.38):** ✅ core loop shipped — 3-room instance (grassland→forest→
> dungeon), boss-gate (existing), **escrow** (loot held/banked/lost), **farm
> toggle**, **wipe→restart room 1** (keeps EXP/soul), **soul** (+1/kill, +10/boss).
> **Remaining:** per-monster `soul_reward` curve (now flat); a 2nd instance in
> data (so advance≠farm); end-of-instance reward + scaling; RO-themed dungeons;
> Rare Loot chest (below); retry/revive upsell on wipe.
**Core loop:** 1 instance = **3 ห้อง (rooms)**. แต่ละห้อง: grind monster **30-50 ตัว**
→ เจอ **บอส 1 ตัว** → ผ่านไปห้องถัดไป. **ห้อง 3 = บอสใหญ่** → **จบ 1 instance**.
จบแล้ว → **ไป instance ถัดไปอัตโนมัติ**; ยกเว้น player เปิด **farm mode** จะ **วน
instance เดิม** (ฟาร์มห้องเดิมซ้ำ).

**สถานะ engine:** scaffold มีแล้ว (ALPHA-8b): `boss_threshold` + `kill_counter`
+ `next_zone_id` + `zone_cleared` + `try_spawn_boss`. **ขาด:**
- ตอนนี้มีแค่ 2 zones (grassland 30 / dungeon 50) **วนกลับกัน** + `boss_template
  = None` → **บอสไม่ spawn จริง**.

**ต้องทำ (ร่าง):**
1. **บอส spawn จริง** — ใส่ `boss_template` ให้ทุกห้อง (ตัวอัด hp/atk จาก mob ห้อง
   นั้น); ห้อง 3 = บอสใหญ่ (is_boss, ตัว 1.5×, hp/atk สูง). [data + อาจมี boss sprite]
2. **3-room linear chain** — เพิ่ม room ที่ 3, ปรับ `next_zone_id` เป็น linear ไม่วน.
3. **Instance structure** — group 3 rooms = 1 instance; room3.next → room1 ของ
   instance ถัดไป (advance) **หรือ** room1 ของ instance เดิม (farm).
4. **Farm toggle** — flag ใน GameState (`farm_current_instance: bool`) + ปุ่ม UI;
   ตอน `request_zone_change_if_cleared` เลือก next ตาม flag.
5. **Fail = wipe → restart room 1** — ปาร์ตี้ตายหมด → เริ่มห้อง 1 ของ instance
   เดิมใหม่ (เสีย progress ภายใน instance). **แต่ยังได้ EXP (base+job) + ได้ soul**
   ที่สะสมระหว่างทาง — เสียแค่ **loot escrow**. (fail ไม่ลงโทษหนัก: progress XP +
   soul ยังเดินหน้า)
6. **Loot ตอนจบ instance** — มอนระหว่างทาง **ไม่ดรอปเข้า stash ทันที**; loot ไป
   อยู่ใน **escrow ของ instance** แล้วเข้า stash จริง **ตอนเคลียร์ instance สำเร็จ
   (บอสห้อง 3 ตาย)**. fail/wipe = **เสีย escrow ทั้งก้อน** (เริ่มห้อง 1 ใหม่). ↔ ต้อง
   เปลี่ยน `drop_on_death` จาก stash ตรงๆ เป็น escrow + grant-on-clear.
   (หมายเหตุ: EXP/job XP/soul ยัง grant ต่อ kill ตามปกติ — ต่างจาก loot)
7. (later) reward ก้อนจบ instance, instance ยากขึ้นเรื่อยๆ (scaling).
8. **(later) dungeon theme แบบ Ragnarok** — ตอนนี้ใช้ placeholder rooms
   (grassland → forest → dungeon) เป็น prototype; ค่อยทำ theme/มอน/บอส ตาม RO.

### 🧩 Skill axis-separation — Stage 3 (foundation DONE, migration DEFERRED)
> รายละเอียด + audit ครบ = memory `pending-axis-stage3.md`. 3 แกนแยกแล้ว:
> damage_type (0.0.189) + cast_element (เดิม) + delivery (0.0.190); Holy Light/
> Aspd Dampening flip เป็น Magic + แก้ 2 บั๊ก Benediction/Radiance แล้ว (0.0.192).
- **ค้าง (รอ cross-class item เป็นตัวขับ):** ย้าย ~13 per-hit effect เข้า helper
  เรียกทั้ง `resolve_instant` + `resolve_projectile` — ตอนนี้ผูก path เดียว
  จะหายเงียบถ้าคลาสอื่นใช้:
  - melee-only: Cleave, teleport (Shadow Strike/Dark Ambush), self-AoE (Magnum
    Break/Shield Blast + DEF-scale + Cinders/Concussion rune), rune Vulnerable/
    BloodMist/Flurry/Execute, taunt_on_hit, Autocounter, dual-wield
  - ranged-only: Knock Back Shot, rune Fusillade/Combustion/Focus/Pinning,
    Soul Crush (ranged+is_magic), elemental-arrow autoswap
  - แบบอย่างทำถูก: `apply_aspd_dampening` / `apply_on_hit_debuff` (both paths)
- **ทำไมชะลอ:** verify cross-class จริงไม่ได้จนกว่ามีไอเทม (ตอนนี้ได้แค่ goldens
  ไม่พัง); ควรทำ WITH ไอเทม cross-class เป็น driver
- **coupling รองที่เหลือ:** is_magic (variance/autoswap) ยัง entity-derived

### Soul — currency ใหม่ (ยังไม่มีระบบ)
**TODO:** เพิ่ม currency `soul` (คล้าย gil แต่คนละตัว) บน GameState. ได้จากการ
kill / จบ instance / wipe (ยังได้ soul). ใช้ทำอะไร = ยังไม่เคาะ (prestige/upgrade?).
ต้อง: field `soul: u64` บน GameState (+VERSION bump), `soul_reward` บน
MonsterTemplate/Entity (เหมือน gil_reward), award ใน `handle_monster_death`, UI.

### Stash limit + Rare Loot chest
- **Stash limit** — เริ่มที่ 20 ช่อง; loot ไม่ดรอปเมื่อ stash เต็ม (stack เข้า slot
  เดิมยังได้). ✅ implemented: `drops::STASH_CAPACITY` + cap ใน `drop_on_death`
  (PRNG draw ยังเกิดปกติ → goldens ไม่กระทบ). future: ทำให้ขยายได้ (upgrade).
- **TODO — Rare Loot chest** — ช่องเก็บ rare loot **แยกอีก 5 ช่อง** (ไม่นับรวม
  stash 20). rare drop เข้า chest, ผู้เล่นกด **claim** ถึงย้ายเข้า inventory
  (กัน rare หายเพราะ stash เต็ม). ต้องมี: chest storage แยกจาก stash, drop-
  routing (rare → chest ถ้ายังไม่เต็ม 5), claim event (chest→stash), UI chest.

### 0. ⭐ ระบบ Skill Tree (เลือกเรียน/อัพเอง — ไม่ติดตัว)  **[gates ทุกสกิลใหม่]**
สกิลต้องเป็น **tree** ที่ผู้เล่นเลือกเรียน/อัพด้วย skill point ไม่ใช่ติดตัว
ตั้งแต่ Lv1. ตอนนี้ให้มีแค่ **Bash + HP Recovery** ให้อัพ; สกิลอื่นอยู่ใน
tree แต่ยัง lock (รอ requirement).
**Scope (ร่าง):**
- per-class skill-tree definition (node = skill id + requirement: job level
  / prerequisite skill + max level) แยกจาก "สกิลที่เรียนแล้ว"
- skill level 0 = ยังไม่เรียน (ไม่มี effect); learn = 0→1 ด้วย skill point
- UI tree (ไม่ใช่แค่แถวสกิลใน CHARACTER) + เช็ค requirement
- migrate: ปัจจุบัน `class.skills` = สกิลติดตัว → เปลี่ยนเป็น tree + known-list
**สกิล swordsman ที่ออกแบบ/implement แล้ว (รอเสียบ tree):**
- Sword Mastery / 2H Sword Mastery — passive +4 ATK/Lv, gate weapon type ✅
- Taunt — free autocast, 5×5 aggro, cd 10→4s, Lv7 ✅
- Endure — soft DEF ceil(0.5·Lv)×ศัตรูติดตัว, cap 100, Lv10 ✅
- **Ignite Break** — *ออกแบบแล้ว ยังไม่ implement*: active, AoE ธาตุไฟ 3×3
  รอบตัว + knockback 2 ช่อง + เปลี่ยนธาตุโจมตีเป็นไฟ 5 วิ. (ต้องเคาะ
  power/SP/cooldown/maxLv; กลไกใหม่: self-AoE, knockback, element-buff)

### 1. เพิ่มสกิลทุก class ให้ครบ — ขั้นที่ 1
ทำให้สกิล tier-1 ของทั้ง 5 class ทำงานจริง (ตอนนี้บางตัวยังเป็น stub).
2 สกิล/class ตาม `game_data.json` + `skills-catalog.md`:
- Swordsman — Bash (active), HP Recovery (passive)
- Archer — Twin Strafe (active), Keen Eye (passive)
- Mage — Fireball (active), Mana Flow (passive)
- Thief — Shadow Strike (active), Double Attack (passive)
- Acolyte — Heal (active), Blessing (passive)

**Scope:** effect จริง + SP cost + cooldown + auto-cast priority; ผูกกับ
`skills.rs` / `tick.rs`. ปักหมุด golden ใหม่ตามที่ behavior เปลี่ยน.

### 2. เพิ่มระบบ equipment ทุกช่อง
ตอนนี้มีแค่ weapon + armor. ขยายเป็นช่องครบแบบ RO.
**Scope (ร่าง):** slot enum (weapon, shield/off-hand, head, armor,
garment, shoes, accessory×2), bonus stacking ต่อช่อง, UI ช่อง equip,
validation (กันใส่ผิดช่อง). ต่อยอดจาก `aspd_base`/`atk_bonus` ที่มีอยู่.

### 3. ทำ encounter ให้เหมาะสม
ปรับ encounter/spawn ให้สมดุล — มอนสเตอร์ต่อ zone, ความหนาแน่น,
ระดับความยาก, จังหวะ boss-gate ให้เหมาะกับพลังผู้เล่นในแต่ละช่วง.
**Scope (ร่าง):** ทบทวน spawn pool + weight + จำนวนพร้อมกัน, ระดับ
stat มอนสเตอร์เทียบ player progression, pacing ของ zone → dungeon
(จาก golden alpha6 ที่ player ตายเร็วใน grassland). ผูกกับ
`zone_spawn` / `data.rs`.

### 4. ทำ loot table
ออกแบบ loot table จริงต่อมอนสเตอร์/zone (ตอนนี้ slime ดรอป scrap/
sword_short แบบ weighted ง่ายๆ). **Scope (ร่าง):** drop pool ต่อ
monster, rarity tier, drop rate, การผูกกับ emission_group/quota_class
ที่มีอยู่, ของหายากจาก boss. ต่อยอดจาก `drops.rs` + `DropEntry`.

### 5. ทำระบบ use item
ระบบใช้ไอเทม (consumable) — กินจาก stash/inventory แล้วเกิด effect.
**Scope (ร่าง):** ItemKind::Consumable, effect (HP/SP potion, buff),
InputEvent::UseItem, ลด quantity ใน stash, auto-use rule (กินยาเมื่อ
HP ต่ำ?), UI ปุ่มใช้. ผูกกับ `events.rs` + `items.rs`.

### 6. Adjust UI
ปรับ/จัดระเบียบ UI โดยรวม. **Scope (รอเคาะรายละเอียด):** layout,
หน้าต่าง (Character/Inventory/Skills), action bar, readability,
party frames. — *ระบุจุดที่อยากปรับเพิ่มเมื่อพร้อม.*

---

## Deferred (จาก ASPD 0.0.33 — เพิ่มเมื่อระบบรองรับ)
- Shield ASPD penalty (มี column ในตาราง iRO แล้ว — ดู `aspd-table-iro.md`)
- ASPD potions (Concentration/Awakening/Berserk) + ASPD skills
  (เช่น Two-Handed Quicken) + %-ASPD equipment
- 2-D job×weapon ASPD (ตอนนี้ item เก็บ modifier เดียวต่อชิ้น)

## Parking lot (idea — ยังไม่ผูกเวลา)
- Rival farmers — AI snapshot แข่ง farming (memory: idea-rival-farmers)
- Soul currency — bound 2nd currency อัปเกรด attribute (idea-soul-currency)
- Party single-player — 5-char auto-farming party (plan-party-singleplayer)

---

## Done log
- 2026-06-16 — ASPD 0.0.33: iRO Renewal FJK formula + real table values,
  switch_class fix, stat-panel readout, attack recoil anim, sim-cli fix.
