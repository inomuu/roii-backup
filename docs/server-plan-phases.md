# Server Plan — Phases (S0–S4)

**Status:** วางแผน (ยังไม่ลงมือ) — ตัดสินใจหลักครบเมื่อ 2026-07-03, ปรับได้.
**Companion to:** `anti-cheat-and-server-model.md` (โมเดล replay-on-demand),
`anti-cheat-economy-strategy-v2.md` (checklist ฝั่ง economy),
`taskbar-arpg-gdd-phase0.md` §4.4 / §9.

เอกสารนี้ร้อยการตัดสินใจทั้งหมดเป็นแผนเดียว: **จะไปถึงระบบ server กลาง
ยังไงโดยไม่เขียนทิ้ง** — ไม่ใช่การเลือก architecture ใหม่ (เลือกแล้ว: ทาง C,
replay-on-demand + ปรับตาม online-required)

---

## 1. Decisions log (2026-07-03)

| เรื่อง | ตัดสินใจ |
|---|---|
| ทิศเกม | **กึ่ง MMO, ONLINE-REQUIRED** (login ตลอด) — pillar "offline-first" ใน GDD เป็น scope ของ prototype Phase 0 เท่านั้น |
| 3-hour loot loop | **เอาออก → PURE DROP** — ของ tradeable มาจาก drop table ใน sim (`roll_drop` deterministic) ไม่ใช่ wall-clock claim |
| จุด verify | **สองจังหวะ**: pin/label ต่อเนื่องระหว่างเล่น + re-verify ตอน export (ขึ้นเรือ) |
| Promotion gate | **เรือ/escrow แบบ diegetic** (ตาม TBH) — ของต้อง "ขนออก" ก่อนโผล่ Steam inventory; ระยะเดินทาง ≥ verify latency + Steam trade-hold |
| ค่า infra | ผู้ใช้รับ bump ~50% ได้ — **ตัวแปรจริงคือเดือน engineering + ops/on-call** ไม่ใช่บิล cloud; optimize เพื่อ "แตะน้อยสุดหลัง launch" |
| Demo Lv70 | ยังเป็น local/offline ตามเดิม — งาน server เริ่มหลัง demo พิสูจน์ความสนุก |

## 2. หลักที่ยังยืน (ไม่เปลี่ยนเพราะ online-required)

- **Sim รันฝั่ง client เสมอ** — instanced, deterministic, integer-only.
  ไม่มี shared-world realtime combat (GDD §9). เหตุผลที่ไม่ทำ server-run sim
  (โมเดล A) คือ 6-12 เดือน engineering + netcode ops สำหรับ solo dev —
  ไม่เกี่ยวกับ offline. **อย่าไถลไปทาง A.**
- **Anti-cheat = replay-verify** — `replay(seed, events) = final_state`
  server รัน sim-core ตัวเดียวกัน (Rust binary/WASM) เทียบ hash.
  Golden tests คือหลักฐานต่อเนื่องว่า replay ข้ามเครื่องตรงกัน.
- **Frame ที่ถูก** (จาก brief): บอทที่เล่นตามกติกา = ลูกค้า; กติกา economy
  (cap/throttle) ใช้เท่ากันทุกบัญชี; สิ่งเดียวที่เป็น "โกง" คือ forged state.

## 3. Online model (รูปแบบ engineering)

"Online-required" = **authenticated + sync เป็นจังหวะ + grace window**
ไม่ใช่ per-tick server dependency:

1. Login (Steam auth ticket) → server แจก session + **seed** (server-seeded RNG
   → seed-grinding ตาย)
2. Client รัน sim ปกติ, stream `InputEvent` log + state hash เป็นระยะ
   (เกม idle input เบามาก — ไม่กี่ byte/นาที) → server **pin hash ต่อเนื่อง**
   (ถูกมาก ไม่ต้อง replay) → rollback/dupe ตาย เพราะ save ตัวจริงอยู่ server
3. เน็ตสะดุด → client sim ต่อได้ (grace window หลายนาที) แล้วส่ง segment
   ย้อนหลังตอน reconnect — determinism ให้ฟรี
4. **Graceful degradation**: server ล่ม → เกมเล่นต่อ local, ฟีเจอร์
   social/market ปิดชั่วคราว, reconcile ทีหลัง — เปลี่ยน incident ตี 3 จาก
   "เกมดับ" เป็น "เทรดหยุด แก้พรุ่งนี้ได้" (บทเรียน TBH 8 มิ.ย.)
5. Offline earnings ยังมีได้: fast-forward แบบ zero-input ที่ server
   attest เวลาจริงแล้ว verify ตอน login (code path เดียวกับ catch-up เดิม)
6. **Managed services ทุกอย่างที่ทำได้** (Postgres/queue/auth) — จ่ายเงิน
   ซื้อ ops ที่ไม่ต้องทำเอง

Server replay จริงเฉพาะ: segment ที่มี tradeable-class drop (ออก label) /
สุ่มตรวจ / น่าสงสัย / จุด export — บัญชีที่ฟาร์มของ bound ล้วนแทบไม่กิน CPU.

## 4. Item lifecycle (tradeable class)

```
ดรอป in-sim (server-seeded)                → PROVISIONAL (ใช้ในเกมได้เต็มที่)
  ↓ sync รอบถัดไป (~นาที)
server replay segment → เห็น drop event    → REGISTERED (ได้ id ใน ledger)
  ↓ ผู้เล่นขนขึ้นเรือ (LoadCargo event)
verify เฉพาะช่วง checkpoint→export         → ESCROWED (บนเรือ; ยัง revoke ได้)
  ↓ ครบ window (verify + Steam hold ~7d)
mint ผ่าน Steamworks IInventoryService     → MINTED (Steam inventory)
```

- **Deterministic instance id**: hash(seed, drop tick, PRNG draw index) —
  replay ฝั่ง server ได้ id เดียวกับ client → ledger dedupe ได้.
  จองเป็น optional field (serde-skip) ตอน seed rare drops.
- **Clean items only** (บทเรียน TBH): mint เฉพาะ base itemdef ที่ fungible —
  gear ที่ refine/ใส่การ์ด/มี affix ต้อง strip ก่อน export หรืออยู่เป็น bound.
  การ์ด (ของหลักที่อยากให้เทรด) เป็น clean by nature อยู่แล้ว.
- **Rate throttle ที่ทางออก**: ความจุ + ตารางเรือ (เทียบ TBH: 4 slots ×
  8h lock ≈ 12 ชิ้น/วัน) — in-game drop ไม่ต้อง cap; "cap lives on the
  promotion gate only". ห้ามขาย "เรือเร็วขึ้น" เป็นเงินจริง (P2W ของ exit rate).

## 5. Phases

### S0 — อุดรอยรั่ว replay model (ทำได้เลย, งาน sim ปกติ)

ทุก mutation ต้องเป็น `InputEvent` — ยิ่ง online-required ยิ่งบังคับ.
Audit (2026-07-07) เจอ 11 replay holes ใน bridge (mutate GameState ตรง ไม่ผ่าน
event). ไล่อุดทีละรู เป็น commit แยก:

- [x] `set_player_class` → `InputEvent::ChangeClass` **DONE 0.0.161** — ย้าย 75
      บรรทัดจาก bridge เข้า `data::change_class` (rebuild บน `build_player_entity` →
      per-level HP/SP factor ครบ, แก้ latent bug); `apply_input` แตกเป็น data-free
      wrapper + `apply_input_with(state, Option<&GameData>, event)` (enum เดียว log
      เดียว) เพื่อรองรับ event ที่ต้องใช้ catalog.
- [x] `current_zone_id` เข้า `GameState` **DONE 0.0.162** — stamped by
      `populate_zone`; ลบ grid-dimension heuristic (เหลือเป็น pre-0.0.162 save
      migration); zone changes (respawn/select_band/restart) → events RespawnInZone
      / SelectBand / RestartInstance. tick-driven advance คงใน driver (deterministic).
- [x] **7 รูสุดท้าย DONE 0.0.163** — RecruitMember / AdvanceClass / BuyCharacter /
      DeployCharacter / BenchCharacter (party) + BuyItem / GrantLoot (items). Handler
      เรียก `data::*` เดิม (logic ไม่ย้าย → goldens byte-identical, ไม่ bless); bridge
      route ผ่าน `apply_input_with` + กู้ค่า return จาก state delta (party size / Gil /
      unit_def / roster key ใหม่). **ครบ 11/11 — ทุก GameState mutation เป็น InputEvent
      แล้ว** → event log (S1) จะ capture ได้เต็ม.
- [ ] ถอด 3h claim flow ใน `steam_market_view.gd` (+ timer `user://`) —
      ตายไปกับ pure-drop decision อยู่แล้ว; `grant_loot()` (ตอนนี้ = GrantLoot event,
      TRANSITIONAL) → flag เป็น debug เมื่อถอด claim UI
- [ ] วินัยต่อเนื่อง: click-attack (แผนอนาคต) = recorded event;
      field ใหม่ทุกตัว classify account-shared vs session-local

### S1 — Save = event log จริง (งาน sim, ยังไม่มี server)

- [x] **S1a DONE 0.0.164** — recording: `apply_input_with` เก็บ `RecordedInput
      {tick, event}` ลง `GameState.input_log` (gated `record_inputs`, serde-skip →
      goldens ไม่ขยับ). `replay::advance()` = driver step เดียว (bridge+replay ใช้ร่วม);
      `replay(initial, data, log, final_tick)` + test พิสูจน์ session เล่นจริง replay
      bit-for-bit. bridge เปิด recording ที่ new-game/create/load.
- [x] **S1b DONE 0.0.165** — `export_log()` ใน bridge + `replay::SaveBundle`
      {sim_version, game_data_version, start, log, final_tick, final_hash} +
      `verify(data)` (replay log ทับ start เช็ค hash = Model C anti-cheat check,
      total ไม่ panic). bridge เก็บ `session_start` checkpoint. test: JSON round-trip
      + verify + tamper-reject. → `sim_core::VERSION` + `game_data.version` บันทึกใน
      bundle แล้ว (ข้อ 3 เดิม DONE).
- [x] **S1c (chain) DONE 0.0.166** — hash-chained snapshot chain: `replay::advance`
      ต่อ `Snapshot{tick, state_hash, chain_hash}` ทุก `SNAPSHOT_INTERVAL_TICKS`
      (6000=10นาที); `chain_hash = hash::chain_link(prev, tick, state_hash)` →
      tamper-evident. `SaveBundle.verify()` regenerate chain + เช็ค final + ทั้ง chain
      → จับ forged MID-session state ที่ endpoint ยังตรงได้. test: forged end / mid /
      dropped checkpoint reject หมด.
- [ ] **S1c เหลือ** — (1) GDScript wiring: เขียน bundle ลง `user://` + verify ตอน load
      (ตอนนี้ save ยังใช้ full snapshot `save_json`; input_log serde-skip → ยังไม่รอด
      save/load ข้ามรอบ). (2) full-STATE per snapshot → server resume จาก snapshot
      ล่าสุดไม่ต้อง replay จาก tick 0 (ตอนนี้ snapshot เก็บแค่ hash = integrity;
      resume ต้องเก็บ state เต็ม) — server-side, ทำตอน S2.

### S2 — Server ตัวแรก (งานก้อนใหญ่: ~2-3 เดือน)

> **BUILD PLAN ละเอียด: `docs/s2-server-build-plan.md`** — stack (Rust+axum+Postgres),
> vertical slices (verify-server → auth → cloud-save → ingest → verify-triggers →
> degradation), data model, milestones M1-M4, open decisions. อ่านตัวนั้นก่อนลงมือ S2.

- [ ] Auth (Steam ticket) + account + session service + server-seeded RNG
- [ ] Ingest: รับ event stream + hash pinning (monotonic — reject segment
      ที่ diverge หลัง hash ที่ pin แล้ว)
- [ ] Verifier: sim-core server-side, replay ตาม trigger ใน §3
- [ ] Cloud save authoritative + offline-earnings attestation
- [ ] Degradation policy ตาม §3 ข้อ 4 (ออกแบบก่อนเขียน ไม่ใช่แปะทีหลัง)

### S3 — Economy layer (ตาม checklist v2 + ของใหม่)

> **DESIGN ละเอียด: `docs/s3-economy-design.md`** — item lifecycle (5 stages,
> register≠mint), deterministic instance-id, ledger + invariants, trade =
> server-granted event, Gil-market-ก่อน→real-money, mint pipeline, build slices
> S3.0-S3.5. อ่านตัวนั้นก่อนลงมือ S3.

> **⚠️ ก่อนแตะ market ใด ๆ: `docs/economy-invariants.md` คือ safety spine** —
> list "เงินห้าม X" I1-I10 + Steam boundary contract + harness = ตัวตรวจแทน
> human review (user 2026-07-05: "AI เขียนโดยฉันไม่ตรวจละเอียด"). harness เขียว
> = ใบเบิกทาง. Timeline: online closed beta (Gil จริง, ปิด real-money) → public
> launch (สองตลาด flag พร้อม, Gil soak สั้น, flip เมื่อ signal เขียว).

- [ ] Ledger เดี่ยว transactional; `export_escrow` state ใน sim
      (`InputEvent::LoadCargo`) + skin การขนส่ง (เรือ/คาราวาน/Kafra — ตัดสินทีหลัง)
- [ ] Mint pipeline §4 + Steamworks integration — **server เราเป็น buffer/
      rate-limiter หน้า Steam API เสมอ** (TBH ตายเพราะ traffic อัด Steam ตรง)
- [ ] **เปิดแคบก่อน**: tradeable ชุดเล็ก → ขยาย (TBH เปิดกว้างแล้วโดนบังคับหด
      — แพงกว่ามาก); in-game Gil market ก่อน real-money (backend เดียวกัน
      สองหน้าร้าน — พิสูจน์ transfer-tech โดยไม่มีความเสี่ยงเงินจริง/กฎหมาย)
- [ ] Merchant License (staked) + Turing test ที่ cash-out + smooth-rate
      ของ tradeable drop class — ตาม checklist v2

### S4 — Social features

- [ ] PvPvE boss race: แต่ละทีมรัน sim ตัวเอง → submit contribution →
      server replay-verify — ไม่ต้องมี realtime netcode
- [ ] Leaderboards, towns/market presence ("คนอื่นอยู่ด้วย" มาจากชั้นนี้
      ไม่ใช่ shared sim)

## 6. TBH intel (2026-06/07 — คู่แข่งตรง, ใช้โมเดลใกล้เรา)

- Trade Ship: ปลดล็อค Cube Lv10; 4 slots, ล็อค 8h/slot แม้ขายแล้ว;
  ship → Steam inventory → ผู้เล่น list เองบน Steam + ยืนยันมือถือ
- ของต้อง clean (no decorations/engravings) ถึง list ได้
- **8 มิ.ย. 2026**: ปิด market + เมนูเรือทั้งอัน ("excessive traffic and
  abnormal requests affecting Steam-side systems") — pivot จาก relay server
  ไป dedicated server กลางไฟไหม้
- **25 มิ.ย. 2026 (v1.00.20)**: เปิดใหม่แบบแคบลง — top 3 grades เทรดไม่ได้
  (ยกเว้น Soulstones), Legendary+ only, แบน level-milestone gear + Type B
- บทเรียน: (1) infra เบา (relay) ไม่รอด → S2 ต้องเป็น buffer จริงจัง;
  (2) เปิดแคบก่อนถูกกว่าโดนบังคับหด; (3) top power tier ควรเป็น bound,
  เทรดของกลาง ๆ

## 7. Open decisions (ยังไม่ตัดสิน)

| เรื่อง | ตัวเลือก | แนวโน้ม/rec |
|---|---|---|
| Replay ข้าม VERSION | multi-verifier vs checkpoint-cut ตอน upgrade | checkpoint-cut (ง่ายกว่ามากสำหรับ solo) |
| Mismatch policy | ban / soft-reject / flag+review | เริ่ม flag+review (false positive จาก bug ตัวเองช่วงแรกเยอะแน่) |
| Skin การขนส่ง | เรือ/คาราวาน/Kafra/นกส่งของ | เนื้อเรื่อง RO-flavored, ตัดสินตอน S3 |
| Tradeable drop classes + rate | การ์ด? uniques? Aegirilium? อัตราเท่าไหร่ | ผูกกับงาน seed rare drops (ยังไม่มีของใน table เลย) |
| Offline-earnings cap | กี่ชั่วโมง, ลดหลั่นยังไง | ตัดสินตอน S2 (server attest เวลาแล้ว ปรับง่าย) |
| Grace window ยาวเท่าไหร่ | นาที → ชั่วโมง | สั้นพอกันโกง ยาวพอ wifi สะดุด — ตัดสินตอน S2 |

## 8. Doc revision TODOs

- [ ] `anti-cheat-and-server-model.md` + GDD §9 ยังเถียงจากมุม offline-first —
      ต้อง revision pass ตอนเริ่ม S2 (โมเดล C ยังถูก แต่เหตุผล/ตาราง UX ต้องอัปเดต
      เป็น online-required + grace window)
- [ ] GDD pillar list — ระบุ scope ให้ชัดว่า pillar 1 เป็นของ Phase 0

## 9. TAB MODEL (user 2026-07-04) — รูปธรรมของ "กึ่ง MMO"

Client เดียว หลาย tab:
- **Tab เมือง** — shared social space แบบ MMO จริง: เดินได้ เห็นผู้เล่นอื่น
  ดูแผงลอย/ตลาด. **อยู่นอก sim ทั้งก้อน** (ไม่มี combat/PRNG/determinism —
  presence เป็น cosmetic, sync หลวม ๆ ได้) = โมเดล PoE-towns/Warframe-relay.
  เส้นแดง GDD §9 ยังอยู่ครบ: **ห้ามมี combat ใน shared space** — นี่คือสิ่งที่
  ทำให้เมืองราคาถูก (presence service + channel แบ่งคนล้น, งานเล็กกว่า verifier).
- **Tab party ×N** — PartySession = sim deterministic เดิม ต่อ tab;
  ตรงกับ refactor AccountState + Vec<PartySession> (own-rng ต่อ session);
  slot เพิ่ม = ขายขาด ~300฿ (pricing ใน gear-systems-design §8).
- Flow ซื้อของในเมือง: market transaction บน ledger → **server-granted
  InputEvent** เข้า party session — ท่อ verify เดิม ไม่มีทางลัด.
- Synergy: แผงลอยเปิดเฉพาะตอน client online (กติกา RO แท้) → การค้าขาย =
  เหตุผลเปิดเกมค้าง = CCU flywheel; เมือง = ที่อยู่ UI ของ boss race /
  leaderboard / social ทั้งหมดใน S4.
