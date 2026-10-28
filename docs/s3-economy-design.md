# S3 — Economy Layer: Item Lifecycle · Ledger · Mint · Gil Market (Design)

**Status:** วางแผน (ยังไม่ลงมือ) — 2026-07-07.
**Prereq:** S2 (auth + cloud-save + replay-verify) — **backbone พร้อม**: replay-verify,
`SaveBundle::verified_state`, promotion trigger (Slice 4). S3 **นั่งบน** ชั้น trust นี้.
**Companion (อ่านก่อน):** `docs/server-plan-phases.md` §4 (item lifecycle) · **`docs/
economy-invariants.md` = SAFETY SPINE (I1-I10 "เงินห้าม X") — money = careless zone,
harness เขียวก่อนถึงแตะได้** · `docs/s2-server-build-plan.md`.

> S3 = ที่ "เงินจริง" เข้ามา → เสี่ยงสุด. หลัก: **ทำ Gil market (ปิด real-money) ก่อน**
> เพื่อซ้อม transfer-tech โดยไม่มีความเสี่ยงเงิน/กฎหมาย → flip real-money เมื่อ signal เขียว.

---

## 1. Scope

**S3 คือ:** item มี identity รายชิ้น (dedup ได้) → ledger เจ้าของ → เทรด (Gil ก่อน,
real-money ทีหลัง) → mint เข้า Steam. **S3 ไม่ใช่:** verify (S2 ทำแล้ว), social/PvP (S4).

**หลักความปลอดภัย (ตลอด S3):** ทุกการเปลี่ยนแปลง money-state = **transactional + ผ่าน
`economy-invariants` harness**. test คือ reviewer แทน human (user: "AI เขียนโดยฉันไม่ตรวจ
ละเอียด") → harness เขียว = ใบเบิกทาง.

---

## 2. Item lifecycle (5 stages) — reversibility เป็นแกน

```
ดรอป in-sim (server-seeded RNG)          → PROVISIONAL   (ใช้ในเกมได้เต็ม, ยังไม่ tradeable)
  ↓ replay-verify (promotion trigger, S2 Slice 4)
server เห็น drop event จริง + ให้ id      → REGISTERED    (มี instance-id ใน ledger; ของเรา, revoke ได้)
  ↓ ผู้เล่นขนขึ้นเรือ (InputEvent::LoadCargo)
verify ช่วง checkpoint→export             → ESCROWED      (บนเรือ; ยัง revoke ได้ — หน้าต่างกันโกง)
  ↓ ครบ window (verify OK + Steam trade-hold ~7d)
Steamworks IInventoryService              → MINTED        (Steam inventory; ← จุดเดียวที่ irreversible)
```

**กฎเหล็ก:** ยิ่งไปทางขวา ยิ่งย้อนยาก. REGISTERED/ESCROWED = **revocable** (เจอ fraud/bug
ทีหลังคืนได้); MINTED = ของ Valve แล้ว. → **register เยอะได้ แต่ mint น้อย + หลัง hold** เพราะ
verify ไม่ perfect (bug ใน sim เราเอง / exploit เพิ่งเจอ) ต้องมีช่วง revocable ก่อน commit.

**register ≠ mint:** register = "พิสูจน์แล้ว + จองใน ledger เรา"; mint = "ออกเป็น Steam asset".

### 2.1 DECIDED 2026-07-07 — tradeable = per-instance non-stack; stack = in-game only

สังเกต genre (TBH/Dota/CS): tradeable item **ไม่ stack** — เพราะ atomic ง่ายกว่า (1 item = 1
id = 1 mint = 1 trade → **ไม่มี split/merge** ของ verified stack) + ได้ provenance/มูลค่าต่อชิ้น.
เราเอาโมเดลนี้:
- **stack = IN-GAME UX เท่านั้น** (การ์ด redling ×12 ในอัลบั้ม). **mint = ดึงออกทีละใบ** →
  กลายเป็น per-instance asset (stack −1). → **tradeable ไม่ stack เลย**.
- **fungible/bulk (ore, orb, trash, material) = in-game อย่างเดียว, ไม่ mint/ไม่เข้า ledger**
  (trash → ขายเป็น Gil). → **ไม่มี "fungible tradeable" → ledger 1 ทรง (instance)**, เบา.
- ตรงกับ clean-item rule §7 (mint เฉพาะสิ่งที่ track ต่อชิ้นได้).

### 2.2 DECIDED — in-game numbers = DERIVED, not trusted (anti-cheat หัวใจ)

player แก้ local ได้ (×12→×999, เปลี่ยน STR) — กันไม่ได้ **แต่ไร้ความหมาย**: server ไม่เชื่อเลข
ที่ client อ้าง → **replay จาก (seed + inputs) คำนวณเลขจริงใหม่** → เลขที่ไม่ follow inputs =
hash mismatch = จับได้. ทุกเลข (stack/stat/quantity) = output ของ tick+inputs (นี่คือเหตุผลที่
S0 บังคับทุก mutation เป็น InputEvent). airtight เพราะ sim deterministic integer-only (invariant
sim-core) → replay bit-identical → เปลี่ยน +1 ก็เห็น. mint/trade/leaderboard ผ่าน replay 100% →
เลขปลอมโผล่แค่บนจอ cheater, mint ไม่ได้. (พิสูจน์แล้ว: bundle ปลอม 1 byte → reject.)

---

## 3. Deterministic instance id (dedup key) — งาน sim-core

✅ PRIMITIVE DONE (S3.0): `hash::instance_id(rng_fingerprint, tick, def_id)` = **256-bit hex**
(ไม่ใช่ u64 — money key ต้อง collision-free) · `Pcg32::fingerprint()` = ตำแหน่ง RNG stream
(อ่านไม่ advance).
- **deterministic + replayable** → server replay ได้ id เดียวกับ client → **ledger dedupe**
  (ปั๊มซ้ำ/ดูปไม่ได้ — id ชนกัน = ตัวเดียวกัน). test + curl พิสูจน์แล้ว.
- **derive ตอน register (server-side), ไม่เก็บใน GameState** → zero schema/golden impact.
  server ได้ fingerprint จาก **replay ของ drop เอง** (promotion trigger) — ไม่เชื่อ id จาก client.
- เฉพาะ **distinct/tradeable** (§2.1); fungible ไม่มี instance-id.

---

## 4. Ledger — single transactional source of truth

Postgres, ตารางเดียว, transactional (money-zone). เก็บ item instance + เจ้าของ + stage.
```
item_instances (instance_id PK, def_id, owner_account_id, stage, registered_at, ...)
ledger_entries (id, instance_id, from_account, to_account, kind, gil_amount, at)  -- append-only
escrow         (instance_id, loaded_at, release_after)
mint_log       (instance_id, steam_item_id, minted_at)
```
**Invariants (economy-invariants I1-I10 form):** instance_id ห้ามซ้ำ; เจ้าของ 1 คน ณ เวลาหนึ่ง;
Gil ห้ามสร้างจากอากาศ (conservation); transfer = atomic (หัก+เพิ่ม ใน tx เดียว); mint 1 ครั้ง/
instance. **harness ทดสอบ invariant พวกนี้ = ตัวตรวจ.**

---

## 5. Trade = server-granted InputEvent (ผูกกับ input-authority)

item เปลี่ยนเจ้าของ **ไม่ใช่ client authored** — เป็น **server-authored** (ตาม
[[input-authority-security]]): market transaction บน ledger → server ออก
`InputEvent::GrantItem/RemoveItem` เข้า session ของแต่ละฝั่ง → **ผ่านท่อ verify เดิม ไม่มี
ทางลัด**. client emit เองไม่ได้ (verifier reject ตัวที่ไม่ตรง ledger grant).
→ นี่คือ machinery เดียวกับที่คุยเรื่อง SetCardRateBonus (server ถือค่า, replay ด้วยของจริง).

---

## 6. สองตลาด: Gil ก่อน → real-money ทีหลัง (backend เดียว สองหน้าร้าน)

| | Gil market (ก่อน) | Real-money (ทีหลัง) |
|---|---|---|
| สกุล | Gil (in-game) | เงินจริง ผ่าน Steam |
| ปลาย | โอนใน ledger | mint เข้า Steam inventory |
| ความเสี่ยง | ต่ำ (ไม่มีเงินจริง/กฎหมาย) | สูง (careless zone เต็ม) |
| เมื่อไร | **online closed beta** ซ้อม | flip เมื่อ signal เขียว (ไม่ใช่ครบวัน) |

**เหตุผล:** พิสูจน์ transfer-tech (ledger, grant-event, dedup, escrow) ด้วย Gil ก่อน → โค้ด
เดียวกัน → พอเขียวค่อยเปิดหน้าร้าน real-money. cards = ของเทรดหลัก.

### 6.1 DECIDED 2026-07-07 — tradeability = 2 flag อิสระ (ไม่ใช่ ladder)

`ItemDef.gil_tradeable` + `ItemDef.steam_tradeable` (bool, default false) → **4 สถานะ**:

| gil | steam | สถานะ | ใคร |
|---|---|---|---|
| ✗ | ✗ | **Bound** | ขยะ, soulbound, quest |
| ✓ | ✗ | **Gil-only** | modified gear (refine/การ์ด — clean-rule mint ไม่ได้) |
| ✗ | ✓ | **Steam-only** | **premium/ซื้อเงินจริง** (skin, cosmetic) — real-money asset, ห้ามไหลลง Gil |
| ✓ | ✓ | **both** | ของดรอปสะอาด (การ์ด/base clean) — liquidity สูงสุด |

**ทำไม 2 flag ไม่ใช่ ladder (Steam⊃Gil):** premium item ต้อง Steam-only (กันไหลลง Gil เจือจาง)
→ Steam ไม่ใช่ superset ของ Gil เสมอ → ต้องแยก.

**ทำไม default Bound + opt-in:** เปิดแคบก่อน (§7). tradeability = property ของ **def** (catalog,
นอก GameState) → **zero hash/save impact**; instance ที่ modify (refine/การ์ด) โดน **demote off
Steam ตอนเทรด** (clean-rule) — compute ไม่ store.

**revenue = fee% ไม่ใช่ venue:** Steam trade ได้ค่า fee ทุกครั้ง **ไม่ว่าจะ Gil-ได้ด้วยหรือไม่**
(คนละกลุ่ม: อยากเงินจริง→Steam, f2p→Gil). บีบของดรอปเป็น Steam-only = paywall เกียร์ท็อป →
churn → idle client น้อย → **CCU flywheel อ่อน**. เลย "ของดรอป = both" ปล่อย f2p เทรด Gil ได้.

**Server gate:** `/register` ต้อง `gil_tradeable`; `/ship/mint` ต้อง `steam_tradeable` (+clean-rule).
Bound → เทรดไม่ได้เลย. **SEED แรก 2026-07-07:** การ์ด 8 def = `gil_tradeable` (steam=both ค่อยดูรายตัว).
supersede legacy `marketable`/`quota_class` (จะลบทีหลัง).

### 6.2 DECIDED 2026-07-07 — trade MODEL แกนที่ 2: per-instance vs commodity

2-flag (§6.1) = **venue** ("ตลาดไหน"). แกนอิสระที่ 2 = **transport** ("เทรดยังไง"):

| | per-instance (ทำแล้ว) | **commodity** (ยังไม่ build) |
|---|---|---|
| ของ | การ์ด/gear (distinct, value สูง volume ต่ำ) | ore/material (fungible, value ต่ำ volume สูง) |
| key | `instance_id` 256-bit | ไม่มี — เทรด **จำนวน**, verify ที่ **balance/state-level** |
| กันเฟ้อ | scarcity | **ต้องมี SINK** |

**ตัวตัดสิน instance vs commodity ไม่ใช่ "stack ไหม"** (การ์ดก็ stack) — แต่คือ **volume×value +
provenance ต่อชิ้นสำคัญไหม**. การ์ด = value สูง/foil/provenance สำคัญ → instance. ore = ชิ้นไหน
ก็เหมือนกัน volume สูง → instance-id ต่อก้อน = ไร้สาระ → commodity.

**INTERIM GUARD (built 2026-07-07):** `/register` reject fungible kind (Material/Junk/Consumable)
แม้ flag ผ่าน → กัน material หลุดเข้า instance-ledger จน commodity transport พร้อม.

**แร่ตีบวก = commodity ตัวอย่างหลัก (ore = first commodity):**
- **sink พร้อมในตัว** — refine เผา ore ทุกครั้ง + RO fail-downgrade เผาเพิ่ม → demand ต่อเนื่อง →
  ไม่เฟ้อ (เหมือน Elunium/Oridecon) → **commodity ที่ปลอดภัยสุดให้เปิดตัวแรก** (ไม่ต้องสร้าง sink ใหม่)
- **venue = gil ก่อน → steam ทีหลัง** (Steam market ของ ore = ยกไปเฟสหลัง, แต่ **โครง data model
  express ได้แล้ว** = 2 flag)

**ore บน Steam ทำไมปลอดภัย (self-balancing) — เมื่อถึงเฟสนั้น:**
1. **anchor supply** — เราขาย ore cash-shop (first-party mint) ราคา X = **price ceiling** →
   player Steam-ore ขายได้แค่ ≤X → **bot ROI ถูก pin ต่ำ** = self-defense + เราได้ revenue หลัก
2. **fabricate ไม่ได้** — drop replay-gated → bot ต้องเล่นจริง (farm ได้เท่าคน)
3. **Steam Market = Steam Wallet เท่านั้น ถอนเงินสดไม่ได้** (Valve lock) → ตัด RMT/laundering ที่ราก
4. **exit-cap + Merchant License stake** ก่อนเปิด A
- **ไม่ต้องแยก farmed vs minted ore** (fungible) → control อยู่ที่ exit-gate + anchor-supply ไม่ใช่ item

**Build order (ore commodity):** (1) commodity balance-transport → (2) cash-shop ore (B, revenue +
anchor) → (3) Steam commodity mint (quantity/stackable แบบ TF2 metal, ไม่ใช่ pull-one) → (4) exit-cap
+ license ก่อนเปิด A. **B ก่อน A เสมอ** (B pin ราคาให้ A ปลอดภัย).

### 6.3 DECIDED 2026-07-07 — fungible loot: confirm "ได้มาจริง" ยังไง (2 คำถามแยก)

fungible ไม่มี instance_id → พิสูจน์ที่ **balance ไม่ใช่ per-item**. แยก 2 คำถาม:

**A. "ได้มาจริง?" = ได้ฟรีจาก save-replay ที่มีแล้ว (S2 Slice 2).** ore อยู่ **ใน GameState (ถูก hash)**
→ SaveBundle replay reproduce count เป๊ะจาก (seed+inputs). edit save ore=999999 → ไร้ผล (server ยึด
ค่า replay). drop deterministic → ปั๊มไม่ได้ นอกจากเล่นจริงนานขึ้น (farm rate = เท่าคน). **ไม่ต้อง build
อะไรใหม่.**

**B. "ไม่ dup ตอนเทรด?" = deposit→escrow แบบจำนวน (= commodity transport, ยังไม่ build):**
```
DepositToMarket{def_id, qty}  (recorded InputEvent, self-cost, Client-authority)
  └─ sim: ore -= qty                          ← ออกจากเกม, refine ด้วยไม่ได้แล้ว
  └─ server: replay ยืนยัน delta = qty จริง → credit market-balance += qty (Server-granted)
market ซื้อขาย "จำนวน" → debit/credit atomic (conservation); Withdraw = reverse event
```
market เชื่อแค่ตัวเลขที่ replay ยืนยัน ไม่เชื่อ client claim. attack→defense:

| attack | defense |
|---|---|
| edit save ให้ ore เยอะ | replay ยึด count จริง → deposit เกิน balance = reject |
| dup deposit (credit ซ้ำ) | log ประมวลครั้งเดียว/tick + **idempotent ที่ pinned head** (S2 ingest) |
| fork: deposit แล้ว submit log อื่นที่ยังมี ore | **hash-chain pinning** — 1 account = 1 monotonic chain |
| manipulate drop เอา ore เพิ่ม | common = marginal (ปล่อย); rare mat = server-roll (§6.4) |

→ instance พิสูจน์ **provenance ต่อชิ้น**; fungible พิสูจน์ **total ผ่าน full-state replay + conserve**.

### 6.4 DECIDED 2026-07-07 — valuable RNG = server-roll (obscurity ≠ security / Kerckhoffs)

**ช่องโหว่:** sim = PCG32 ตัวเดียว deterministic → client รู้ state (ใน save) + algorithm → **predict
draw ถัดไปได้** + filler action ชิฟต์ stream → เลือกลำดับ input ที่ **ถูกกฎ 100%** ให้ jackpot ตกบน loot
event (luck-manip; เช่น cast-then-kill). **replay จับไม่ได้** เพราะ inputs ถูกกฎจริง → reproduce exploit
→ hash ตรง → ผ่าน. ไม่ต้องแฮก process จริง — **shadow sim offline** (seed server-given + inputs ตัวเอง
→ rebuild state; sim-core ก็ ship อยู่บนเครื่องเขา) → brute-force combo.

**"reverse ยาก" ไม่ใช่ security** — เป็นความลับที่**เรารั่วเองได้** และรั่วไปแล้วหลาย channel:
`game_data.json` ship plaintext (drop table public), `sim-cli` = shadow sim สำเร็จรูป, open-source =
draw order public. → **อย่านับ obscurity เป็น control.**

**Kerckhoffs — defense ที่ทน:** ออกแบบให้ปลอดภัย **แม้ sim public 100%**:
- **valuable RNG (rare card / rare mat / lootbox / REFINE) = server-roll** — server roll ด้วย rng ตัวเอง
  (state ไม่เคย ship) → grant เป็น Server-authority event → client เดา stream ไม่ได้แม้มี source ครบ.
- **REFINE (ตีบวก) = HIGH-priority server-roll (2026-07-08):** client shadow-sim predict PCG32 →
  ตีเฉพาะ draw ที่ success → bypass published odds = 100% ไม่ break → +10 tradeable ฟรี, ทำลาย break-
  scarcity, ท่วมตลาด. EV ใหญ่+ทำซ้ำง่าย → priority สูงกว่าการ์ด. FIX: server roll ด้วย `refine::odds_for`
  ตารางเดิม + rng server → published-odds "no secret gacha" ยังจริง (แค่ย้ายคนทอย). ต้องทำ**ก่อน**เกียร์
  refined เทรด live. offline/alpha = client-roll พอ.
- **common/low-value = client-roll** — defense = **EV ต่ำเกินคุ้ม tool** ไม่ใช่การซ่อน.
- ห้ามมี tier กลางที่ "ปลอดภัยเพราะ reverse ยาก". ความลับที่หวงจริง = **server rng state + server keys**.
- โบนัส: economy security ไม่พึ่งความลับ sim → **open-source / moddable / แจก sim-cli ได้** โดยไม่อ่อนลง.
- สถานะ: drop roll ตอนนี้ = client-side ทั้งหมด (`roll_drop`) → ยังไม่กระทบ (ไม่มี tradeable drop live);
  ต้องย้าย valuable roll ไป server-authored **ก่อน** เปิด tradeable value จริง.

### 6.4.1 กลไก server-roll refine (2026-07-08)

**Sim foundation (0.0.175, DONE):** `StartRefine{,Inventory}` พก `server_roll: u64`.
- `!= 0` → outcome ทอยจาก nonce (`server_roll % 10_000` อ่านผ่าน `odds_for` เดิม), **ไม่กิน
  `state.rng` draw เลย** → shadow-sim steer stream ไม่ได้ + replay ซ้ำได้ (nonce อยู่ใน input log).
  `authority() = Server`.
- `== 0` → offline/alpha client-roll (1 draw, เดิม). `authority() = Client`.
- skip-if-0 → log/goldens เดิม byte-identical (alpha5 ไม่ re-bless). published-odds ยังจริง (แค่ย้ายคนทอย).

**DECIDED (2026-07-08) = A: server โรล, sim apply.** ไม่เอา B (server roll+apply ถือ live item state).
เหตุผล: online-required **ไม่ได้แปลว่า server จำลองเกม** — client รัน sim เองเพื่อ **scale** (100k CCU =
100k live sim ถ้า server รัน = ตาย) → server ต้องเบา = verifier + roll-arbiter. B ถือ live item state +
sync เต็ม + desync เปราะ + logic ซ้ำ + odds honesty อ่อน → แพงและแตกสถาปัตย์.

**Production = server โรลเสมอ, ไม่มี fallback.** ทุก refine ผ่าน server (online-required). `server_roll=0`
เหลือไว้แค่ **test/tooling** (sim-cli/goldens/headless ที่ไม่มี server) — ไม่ใช่โหมด player. ออฟไลน์ =
ปุ่มตีบวก disabled "ต้องต่อเน็ต".

**กฎความปลอดภัยเดียว = "โรลแล้ว final".** ขอ roll = **commit ทันที ถอนไม่ได้**. ถ้าไม่บังคับข้อนี้ →
**peek-and-select**: client peek roll ทุกชิ้น (compute `odds_for` เองได้) → ตีเฉพาะตัวชนะ, ตัว fail ไม่ตี.
ตัว fail **ไม่มีร่องรอยใน log** → promotion-replay เห็นแต่ refine สำเร็จ nonce ถูกหมด → **ผ่าน** → mint +10
tradeable เงียบๆ. ปิดด้วย: server จำ issued rolls + บังคับ **consume-all**.

**nonce ผูก (account, item_instance_id, refine_level)** → กัน target-selection (revealed roll ใช้กับ item
อื่นไม่ได้). ต้องมี **stable `instance_id` บน Item** (ขยาย `hash::instance_id` ให้ครอบ refinable gear ทุกชิ้น).

**Server เก็บอะไร:** **outstanding issued-rolls ต่อ account** เท่านั้น (`{account, item_instance_id, level,
nonce, seq}`) — เล็ก + **self-clearing** (client sync → consume → ลบ). ไม่ใช่ stateless (จับ peek-skip ไม่ได้),
ไม่ใช่ live item state (นั่น B).

**Reconcile ถูก:** ตอน sync **scan input-log หา refine events แล้ว match กับ issued rolls** (ขาด = ทิ้ง fail
= flag) — **log-scan ไม่ใช่ replay** → รันได้ทุก sync. Full replay + consume-all enforcement เต็มตัว = เฉพาะ
**promotion** (ขอบที่ value ออกเป็นเงิน) → เข้ากับ audit cost model (ไม่ verify ทุก session).

**Phase 2 build order:** (a) `instance_id` บน Item (schema add + VERSION bump; set ตอน drop/craft/instantiate);
(b) verify-server: `server_secret` (env) + `POST /refine/roll` (bound nonce + บันทึก outstanding) + sync-time
log-scan reconcile; (c) promotion-time full enforcement (consume-all + bind-check). adversarial tests: peek,
abandon-retry, drop-fail, reorder, cross-item.

**Deferred (social, ยังไม่มีระบบ):** +10 global announce — server รู้ทุกผลโรล real-time → เป็น broadcast
ที่ **เชื่อถือได้** (client fake-flex ไม่ได้; client-roll ทำไม่ได้เลย). พึ่ง push channel (WebSocket/SSE/poll)
ที่ยังไม่มี → ทำหลัง base.

---

## 7. Mint pipeline (real-money, ระวังสุด)

- **server = buffer/rate-limiter หน้า Steam API เสมอ** (TBH ตายเพราะ traffic อัด Steam ตรง).
- **clean-item rule:** mint เฉพาะ base itemdef ที่ fungible; gear ที่ refine/ใส่การ์ด/affix →
  strip ก่อน หรืออยู่เป็น bound. การ์ด = clean by nature อยู่แล้ว.
- **rate gate = เรือ/escrow** (ความจุ + ตารางเรือ, เทียบ TBH 4 slot × 8h ≈ 12 ชิ้น/วัน).
  in-game drop ไม่ต้อง cap; **"cap lives on the promotion gate only"**. ห้ามขาย "เรือเร็วขึ้น"
  เป็นเงินจริง (P2W ของ exit rate).
- **เปิดแคบก่อน** (TBH: เปิดกว้างแล้วโดนบังคับหด = แพงกว่า): tradeable ชุดเล็ก → ขยาย;
  top power tier = bound, เทรดของกลางๆ.

---

## 8. Anti-fraud: S3 นั่งบน S2 ยังไง

1. **promotion = 100% replay** (S2 Slice 4) → ของออกตลาดพิสูจน์ provenance เสมอ = value เอา
   ออกไม่ได้โดยไม่ตรวจ.
2. **instance-id dedup** (§3) → ปั๊ม/ดูปไม่ได้.
3. **escrow revocable window** (§2) → เจอ fraud/bug ก่อน mint ยังคืนได้.
4. **~10% audit + risk-weight** (S2 Slice 4) → กันโกง progress/leaderboard ที่ไม่ promote.
5. **Merchant License (staked) + Turing test ที่ cash-out** (economy checklist v2) → โทษหนัก
   ทำให้ EV โกงติดลบ (จับ 1−0.9^N ต่อ session + เสีย stake).

---

## 9. Build order (slices — Gil ก่อน, แคบก่อน)

- **S3.0 — instance-id** ✅ DONE (sim-core): `Pcg32::fingerprint` + `hash::instance_id
  (fp, tick, def)` = dedup key; test replayable+unique; zero schema impact. เหลือ: wire
  เข้า drop path จริง + seed rare/tradeable drops (ยังไม่มีของ tradeable ใน table).
- **S3.1 — ledger** 🟡 IN-MEMORY DONE (verify-server): `POST /register` (server derive id,
  dedup — dupe/cross-account = 409) + `GET /ledger`. curl พิสูจน์. เหลือ: ย้าย Postgres
  transactional + invariants harness (economy-invariants) = S3.1b.
- **S3.2 — REGISTERED**: promotion (S2 Slice 4) → เข้า ledger ได้ id (ต่อ Slice4↔register).
- **S3.3 — Gil market** ✅ IN-MEMORY DONE ⭐ (verify-server): `/market/list` + `/market/buy`
  + `/wallet` — buyer จ่าย, **fee 5% burn (Gil sink)**, seller ได้ price−fee, ownership โอน
  ใน ledger, listing consumed. Gil conserved. curl พิสูจน์. เหลือ: Postgres + Gil = verified
  GameState.gil ผ่าน server-granted event.
- **S3.4 — escrow/ship** ✅ IN-MEMORY DONE: `/ship/load` REGISTERED→ESCROWED (ถอดจาก market,
  revocable). guard: trade ESCROWED ไม่ได้.
- **S3.5 — Steamworks mint** ✅ IN-MEMORY DONE (mocked): `/ship/mint` ESCROWED→MINTED +
  mock steam_item_id, mint-once, MINTED เทรด/mint ซ้ำไม่ได้. เหลือ: Steamworks IInventoryService
  จริง (real-money, หลัง flag + signal เขียว) + hold window.

**S3 lifecycle demo ครบ end-to-end (in-memory):** register→ledger dedup→Gil market+fee→escrow
→mint, guards ทุกด่าน. **เหลือ production:** Postgres transactional + economy-invariants harness
+ Steamworks จริง + wire instance-id เข้า drop path ใน sim (seed tradeable drops).
**harness เขียวทุก slice** ก่อนไปต่อ real-money.

---

## 10. Open decisions

| เรื่อง | ตัวเลือก | แนวโน้ม |
|---|---|---|
| tradeable classes | การ์ด / uniques / Aegirilium / อัตรา | เริ่มการ์ด (clean, เป็นของหลัก); ผูกกับงาน seed rare drops |
| skin การขนส่ง | เรือ/คาราวาน/Kafra/นกส่งของ | RO-flavored, ตัดสินตอน S3.4 |
| exit rate (เรือ) | slot × lock เท่าไหร่ | เทียบ TBH ~12/วัน; tune |
| Gil→real flip trigger | signal อะไรถึงเขียว | Gil market เสถียร + invariant ไม่แดง + soak สั้น (ไม่ใช่ครบวัน) |
| P2W ramp-gate | ตาม economy-antibot-brief | way-A + ramp-gate guardrail (decided) |

---

## 11. อย่าทำ (TBH lessons + safety)

- ❌ traffic อัด Steam ตรง → server ต้องเป็น buffer
- ❌ เปิด tradeable กว้างวันแรก → เปิดแคบ ขยายทีหลัง
- ❌ mint ของที่ไม่ clean (affix/refine) → strip หรือ bound
- ❌ แตะ money-state โดยไม่ผ่าน harness/tx → careless zone
- ❌ flip real-money ตามตาราง → flip ตาม signal เขียว
- ❌ ขาย exit-rate (เรือเร็ว) เป็นเงินจริง → P2W ผิดจุด
