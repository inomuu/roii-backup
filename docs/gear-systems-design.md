# Gear Systems v2 — Stars / Refine / Skill Orb / Uniques / Orbs / Market

**Status:** DESIGN ตกผลึก 2026-07-03 (v2 — โครงดาว 0-5 ของ user แทน Magic/Rare
ของ v1 ทั้งแท่ง). ยังไม่ build — ทุกอย่างหลัง launch ตาม [steam-launch-plan] cut list.
**Companion:** `gear-roll-rules.md` (**กฎ roll ตามที่ implement จริง 0.0.136 — อ่านอันนั้น
ก่อนแตะ `gear_roll.rs`**), `card-system.md` (การ์ด/socket/ดาวการ์ด),
`server-plan-phases.md` (S3), `drop-system.md` (2-axis), GDD §3.2.1 (legibility)

---

## 1. กายวิภาคไอเทม (THE spec)

```
ไอเทม 1 ชิ้น
├─ Base stats (ตาม def + tier; socket การ์ดฝังใน def — "Sword [1]" = คนละ id)
├─ ดาว 0-5 = จำนวนบรรทัด BONUS **+ base stat floor ดาวละ +10%** (0.0.136 —
│    ดาวเยอะดีกว่า 0★ เสมอแม้ roll ออก off-stat: "ได้แล้วซึม" ไม่มี)
│    ├─ ดรอปมา 0-4★ (4★ โคตรแรร์) → อัพต่อถึง 5★ ด้วยวัสดุ
│    └─ **bonus ROLL ตอนดรอปทันที เต็มทุกช่อง** (0.0.136 — user: "ให้ roll
│       stat ลงไปเลย" ของดรอปมาพร้อมใช้; orb §3 = เครื่องมือ REROLL ทีหลัง
│       สำหรับผู้เล่นที่ invest แล้ว — ผู้เล่นใหม่ไม่ craft แค่เก็บของแรงขึ้น)
├─ ตีบวก +N → base stat เพิ่ม + ปลด GEAR PASSIVE ตาม milestone (+ เลเวลสกิลไอเทม)
├─ Card sockets (ระบบที่มีอยู่ — ดู card-system.md)
└─ Skill slot (1) ← Skill Orb สุ่มสกิล MINOR จาก pool ของคลาส
```

- **ดาว 0-5 แทน rarity ladder ของ PoE ทั้งหมด** (ไม่มี Normal/Magic/Rare,
  ไม่มี Transmute/Aug/Regal) — สเกลเดียว อ่านออกทันที, ภาษากาชามาตรฐาน.
  ใช้คำว่า "ดาว" ซ้ำกับดาวการ์ดได้ (user: เกมกาชาใช้ดาวหมด ไม่งง).
- ได้ทั้ง "ล่าของ" (ดรอป 3★ = ตื่นเต้น) และ "บ่มของ" (วัสดุอัพ 4-5★ = sink
  + ของเก่าไม่ตัน) — เส้น 4-5★ มีทางเดียวคือบ่ม.
- **Star roll ต่อ gear drop (0.0.135, ตาราง bp):** มอนทั่วไป
  75/18/6/0.99/**0.01**% — boss 40/30/20/9.9/**0.1**% (boss = สามแกนรางวัล:
  การ์ด rate-up + [2] socket + ดาวสวย). **4★ ดรอปได้แล้วแบบโคตรหายาก**
  (user 2026-07-05 "ยากมากๆ"): trash 1/10,000 ชิ้น gear, boss 1/1,000 —
  5★ ยังบ่มอย่างเดียว. คณิต chase สองชั้น: 3★ อะไรก็ได้ ~4-5/วัน idle
  (แจ็คพอตรายวัน) / 3★ ของ*ชิ้นเป้าหมาย* ~1/10+วัน (ความเจาะจงทำหน้าที่แทน
  เรทโหด → ตลาดมีชีวิต: ขายข้ามสาย). Unique ไม่ roll ดาว — **`unique` เป็น
  แท็กแยก ไม่ใช่ rarity Legendary** (user: "Legendary is not unique").
  Draw เฉพาะ gear (state-keyed แบบ foil — replay-safe). เลขปรับตาม
  trial-and-error.
- **Rarity ตามดาว (0.0.135):** ladder 6 ขั้นแบบ Dota 2 — Common < Uncommon <
  Rare < **Mythical** < Legendary < **Immortal** (ตามลำดับ Dota: Immortal
  มาก่อน Arcana → **Arcane เก็บไว้เป็นขั้นสูงกว่าในอนาคต**). Gear ที่ดรอป
  rarity = `max(def, ดาว)`: 1★ Uncommon · 2★ Rare (**เข้า rare bag
  อัตโนมัติ** — faucet จริงตัวแรกของถุง) · 3★ Mythical · 4★ Legendary ·
  5★ Immortal; def `[1]`/`[2]` = Uncommon/Rare ตั้งแต่ใน data.
- **⚠️ VOLUME RULE (user 2026-07-04):** ผู้เล่น idle ฆ่า 10k-50k+ ตัว/วัน
  (วัดจริงจาก sim: ~2,200/ชม.) × multi-party อีก — **เรทของแรร์ทุกตัวห้าม pin
  จากความรู้สึกต่อ roll**: ตั้ง "เป้าผลลัพธ์/วัน" ในสเปค (3★ ~1-3/วัน,
  ชิ้นเป้าหมาย ~1/สัปดาห์+) แล้วให้ sim-cli วัด volume จริงต่อ band เพื่อแก้
  สมการย้อนกลับ. เพิ่ม "loot-per-day audit" เข้า Update Kit ทุกครั้งที่เพิ่ม
  band. Boss ระวังสุด (kills/วัน ~60 → [2]+3★ ที่ตั้งไว้ใจดีเกิน — จูนตอน
  audit). ของ tradeable = per-account daily soft-cap ตาม economy brief
  (ที่สเกลนี้ cap ต่อบัญชีคือตัวคุมจริง ไม่ใช่เรทต่อ roll).
  **LEVEL-GAP PENALTY + HARD-MODE DUNGEONS (user 2026-07-05 — คู่กันเสมอ):**
  หั่นดรอปตาม gap (user: **15 lv → −20%, 25 lv → −50%**; เส้นตรง ~3%/lv,
  เริ่มที่ gap 15, cap หั่นสูงสุด −90% ~gap 38+; ต่ำกว่า 15 = เต็ม 100%;
  **โชว์ % บนจอ** — transparent-odds) — auto-retreat (ถอย
  1-2 band) อยู่ใต้เส้นเสมอ. ทางที่ตั้งใจให้เวลสูงฟาร์มของ band ต่ำ =
  **เวอร์ชันยากของดันเจี้ยน/โซนต่ำ** (RO Illusion Dungeon แท้ ๆ): มอนตระกูลเดิม
  + loot table เดิม (การ์ดใบเดิม!) + สเตตสเกลตามยุค → ดรอปเต็ม.
  Update-Kit material ชั้นดี: content เก่า + สเตตใหม่ = beat ราคาถูก.
  Implement: หลัง launch (คู่กับ dungeon แรก).

## 2. Bonus pool (ต่อช่องดาว) — **แบบ RO Random Options ตัด SP** (DECIDED 2026-07-03)

หลัก: **enchant pool = ตัวเลขล้วน** (RO-style) — "กลไก" อยู่แกนอื่น: multi-hit/
Double Attack = อาณาเขตการ์ด (Sidewinder), extra projectile/rule-change = unique,
active proc = Skill Orb → แต่ละแกนไม่ทับกัน.

หน่วย % = basis points. **ค่า ROLL เป็นช่วงใน tier** (DECIDED 2026-07-04 —
user เลือก range แทน fixed): ช่วงไม่ทับกันระหว่าง tier (เห็นค่า = รู้ tier),
step เต็มหน่วย (% ทีละ 1, flat ทีละ 1), roll ค่า = 1 PRNG draw ต่อ mod →
ได้ "perfect roll" chase + Divine-like orb มีที่ยืน. สเตตเลขจิ๋วบาง tier
เป็นค่าเดี่ยวโดยธรรมชาติ (+1 ไม่มีช่วงให้ roll).

**Roll rules (DECIDED 2026-07-04):**
1. **เลข tier นับขึ้น: มาก = แรง** (T1 อ่อนสุด — user: กลับด้านจาก PoE เพื่อ
   เปิดเพดาน) — **T1 = ทุก band / T2 = b5+ / T3 = b9+ / อนาคต T4 @b13+,
   T5 @b17+, …** ไล่ตาม band ใหม่ได้ไม่จำกัด (เพิ่มชั้น ไม่ขยายเลขในชั้น).
   **Tier window ผูกกับ TIER ของ base def (s-number ↔ band) ไม่ใช่ instance**
   (2026-07-04) — ไม่มี ilvl ซ่อนต่อชิ้นแบบ PoE: gear def เราแยกตาม band อยู่แล้ว
   (s1↔b1-2, s5↔b5+, …) → ของ 0★ ทุกใบเหมือนกัน 100% = fungible จริง
   (PoE ของขาวไม่ fungible เพราะ ilvl ซ่อน/quality/socket สุ่ม — ไม่เอา);
   ป้าย band บนของ = tier badge ของ def. **เปิดเผย weight ตรง ๆ ใน tooltip**
   (transparent-odds brand — PoE2 ซ่อนจนชุมชนต้อง reverse-engineer, ไม่เอา).
2. **น้ำหนัก roll**: tier ที่เข้าถึงได้ถ่วง ต่ำ:กลาง:สูง = 60:30:10 —
   band เปิด "สิทธิ์" ไม่ใช่การันตี (ใบ 5★ T3 ครบทุกช่อง ≈ 1/แสน = ยอด chase).
   **หน้าต่างเลื่อน (แนะนำ):** pool กว้าง 3 ชั้นเสมอ, ชั้นล่างเกษียณตาม band —
   b1-4: T1 / b5-8: T1-T2 / b9-12: T1-T3 / b13-16: T2-T4 / b17+: T3-T5 —
   ของ band สูงไม่ roll ค่ากาก + ทุก bracket ใหม่ยกทั้งเพดานและพื้น.
3. **ไม่มี prefix/suffix + ห้าม mod id ซ้ำในใบเดียว** — 5 ช่องอิสระ.
4. **สเกลสองคลาส**: ทุก mod ค่าตายตัว (สเตต=สกุลไร้สเกลของ RO; HIT/FLEE
   ตัดสินที่ผลต่าง; % สเกลเอง) ยกเว้น **MaxHP แบน = สูตร ×band** ตัวเดียว
   (HP pool โต ×100). แบนเด่นต้นเกม % เด่นปลายเกม = จังหวะสลับตามเลเวล.
5. **TIER ใหม่ (T4+) โตขั้นบวก ไม่ใช่ขั้นคูณ** (pin 2026-07-04): T1→T3 ดูเหมือน
   ×2/ขั้น แต่หลัง T3 สลับเป็น +คงที่ (เช่น ATK% +4/ขั้น: T4 12-16, T5 16-20 —
   แบบบันได life ของ PoE). เบรกกันระเบิดมีครบสามชั้นอยู่แล้ว: additive bucket
   ใน sim (`buff_sum` รวมตระกูลแล้ว `apply_pct` ครั้งเดียว), ห้าม mod ซ้ำ/ใบ
   (เพดาน era = slots × top tier: ~×2.0 BiS ตอนนี้), หน้าต่างเลื่อน.
   **กติกา semi-incremental (user 2026-07-05 "เราจะ * เพราะเป็นกึ่ง incremental
   เกม" + "atk% ก็น่าจะคูณ"): แหล่งเดียวกันบวก, คนละชั้นคูณ** —
   (1) dmg-vs สามแกน (element/race/size) ข้ามแกน MULTIPLY (+20%³ = ×1.728;
   ในแกนเดียวกันบวก) · (2) gear ATK%/MATK% เป็นชั้นคูณแยกจาก bucket
   บัฟ+การ์ด: `base × (1+บัฟ+การ์ด) × (1+gear%)`. จัดของครบหลายชั้นใส่เป้า
   เดียว = build payoff ตั้งใจ (watch-point ตอนเลข tier สูง: ชั้นคูณโตเร็ว). มอน era ใหม่โตคูณ / gear โตบวก → สัดส่วนพลังจาก gear
   หดเบา ๆ ทุก era เอง = scale ไม่จำกัดโดยไม่ re-balance ของเก่า.

| Mod | T1 (ทุก band) | T2 (b5+) | T3 (b9+) |
|---|---|---|---|
| +STR / +AGI / +VIT / +INT / +DEX / +LUK | +1 | +2–3 | +4–5 |
| MaxHP แบน | (8–12)×band | (16–25)×band | (25–35)×band |
| MaxHP % | 1–2% | 3–4% | 4–6% |
| ATK % / MATK % | 2–3% | 4–6% | 6–9% |
| ATK / MATK แบน (เพิ่ม 0.0.134) | +2–4 | +5–9 | +10–16 |
| Dmg vs **element** / **race** / **size** | 5–8% | 10–15% | 15–20% |
| Resist [element] | 5–8% | 10–15% | 15–20% |
| +HIT | +2–4 | +5–8 | +8–12 |
| +FLEE | +2–4 | +5–8 | +8–12 |
| +CRIT (เพดานเตี้ย) | 1% | 2% | 3% |
| Cast time −% (มี FCT floor) | 3–5% | 7–10% | 11–15% |
| Heal effectiveness % (**staff/mace เท่านั้น** — อาวุธสาย acolyte, user 2026-07-05) | 3–5% | 7–10% | 10–13% |
| Cooldown −% (CDR) | 2–3% | 4–6% | 7–10% |
| Ignore DEF % | 3–5% | 6–10% | 12–15% |
| Ignore MDEF % | 3–5% | 6–10% | 12–15% |

(T3 ลด ~20% ทั้งแถว + ปลด CDR/Ignore เข้า pool — user 2026-07-04.)

**กฎ stack ข้ามชิ้น (user 2026-07-04 — แบ่งตาม "ปลายทางของ mod"):**
- **สายเพิ่มเลข = อิสระทุกช่อง** (ATK%/MATK%, Dmg-vs, HP, HIT, สเตต) —
  additive bucket เจือจางตัวเอง ไม่มีสภาวะพัง. FLEE อิสระด้วย
  (มี floor ในระบบแล้ว: accuracy ขั้นต่ำ 5% + โดนรุม flee ละลาย).
- **สาย "ลดจนเป็น 0 ได้" = คุมแหล่ง + cap ประกาศ**:
  Ignore DEF/MDEF → **อาวุธเท่านั้น** + cap รวม ≤50% ·
  CDR → **accessory เท่านั้น** + cap รวม ≤30% (สเกล 1/(1−x)) ·
  **Cast −% → หัวเท่านั้น (HeadTop/Mid/Low) + cap รวม ≤50%** (user 2026-07-04:
  FCT floor ไม่ครอบทุกสเปล + อาวุธ mod แน่นแล้ว; RO แท้ — Kathryne = การ์ดหัว;
  3 ช่องหัว × T3 ≈ 45% ชนใกล้ cap เองพอดี) ·
  Resist → อิสระแต่ **cap 75%/ธาตุ** (แบบ RO/PoE max res).
- **Signature mods สายรับ (user 2026-07-04 — สองตระกูลแชร์สองชิ้น เลขเบาลง):**
  **−%dmg จาก [race]/[size]** = เสื้อ + โล่ (3-5/6-10/10-13 ต่อชิ้น, รวม ~26%,
  cap 50% — Thara Frog) ·
  **Resist [status]%** = เสื้อ + ผ้าคลุม (5-8/10-15/15-20 ต่อชิ้น — บันไดเดียว
  กับ resist ธาตุ; enchant สองชิ้น ~40% เหลือ budget ใต้ cap 75% ให้การ์ด/
  unique/path — immune 100% = อาณาเขต unique/การ์ดแบบ Marc) ·
  **รองเท้า** = Move speed +% (3-5/6-9/10-14, cap 30% — melee uptime จริง) ·
  **ผ้าคลุม** = −%dmg จาก ranged (5-8/10-15/15-20, cap 50% — Horn card).
  → บุคลิก paper doll: อาวุธ=เจาะ, หัว=เร็วเวท, accessory=CDR, เสื้อ=ฮับสายรับ
  (ลดเผ่า+กันสถานะ), โล่=ลดเผ่า, ผ้าคลุม=กันสถานะ+กันไกล+หลบ, รองเท้า=เร็ว.
- **ประดับ = 2 ช่อง GENERIC แบบ RO (user 2026-07-04):** ประดับทุกชิ้น
  (แหวน/จี้/อื่น) ใส่ช่องไหนก็ได้ — แหวนสองวง/จี้สองเส้นได้. ปัจจุบัน implement
  ผิด (ring ล็อค Acc1, pendant ล็อค Acc2) → **งานแก้**: def เป็น accessory
  generic + ตอนใส่ stamp ช่องว่าง (1→2), ถอดแล้วล้าง stamp — pattern เดียวกับ
  dual-wield WeaponLeft ที่มี test อยู่แล้ว; การ์ด accessory route 1/2 รองรับ
  อยู่แล้ว. CDR cap 30% รวมสองช่องคงเดิม.
- CRIT อิสระแต่ tier จิ๋วโดยเจตนา (crit ของเรา ignore DEF ทั้งหมด = zero-out
  แฝง — เพดานจาก gear ~30%; watch-point ตอน meta จริง).

- Stat bonuses เข้า Layer-2 ผ่านท่อเดียวกับการ์ด (buffed_stats) — ไม่แตะ
  stat allocation ของผู้เล่น.
- dmg vs race/size ต้องการ "surface race/size onto Entity" (จดใน card-system
  doc แล้ว — dependency ร่วมกับ weapon-card conditionals).
- **ตัดออก:** SP ทุกชนิด (MaxSP / SP cost — SP model ยังรอ %-rework, ไม่ผูกของ
  ถาวรกับระบบที่ไม่นิ่ง; user 2026-07-03) · Double Attack / extra projectile
  (ย้ายไปแกนการ์ด/unique) · Party-mods (เก็บเป็น candidate ของ unique แทน).
- **ห้ามตลอดชีพ:** ATK SPEED% (authored ผ่าน `AspdPct` / `atk_speed_bp` เท่านั้น) — ตัวเดียว.
  (CDR + Ignore DEF/MDEF ปลดเข้า pool แล้วแบบ % เล็ก 2026-07-04; crit คุมด้วย
  เพดานเตี้ยใน pool อยู่แล้ว.)
- Tier gate ตาม band ของแหล่งดรอป (= ilvl ของเรา, โชว์บนของ).

## 3. Orb set (เครื่องมือบนช่องดาว) — ไล่หยาบ→ละเอียดตามอายุระบบ

> **⚠️ superseded 2026-07-05 → `orb-system-design.md` คือแหล่งจริงของระบบหิน**
> (ตารางล่างเก็บไว้เป็น trail; บทบาทหินเสกเปลี่ยนแล้วเพราะ roll-at-drop)

| จังหวะ | Orb (ชื่อ RO fiction ค่อยตั้ง) | ผล |
|---|---|---|
| Update 1 | **หินเสก** | ~~roll ลงช่องว่าง~~ **บทบาทเปลี่ยน (0.0.136 ดรอป roll เต็มแล้ว)** — candidate: reroll เจาะจง 1 ช่อง (รอคุยตอน build orb) |
| Update 1 | **หินเขย่า** | reroll bonus ทุกช่อง (PoE1-Chaos) — commodity หลักตลาด Gil |
| Update 1 | **วัสดุอัพดาว** | 3★→4★→5★ (แพงขึ้นชัน) |
| Update 3 | deterministic (essence-like) | การันตีประเภท bonus ที่เลือก |
| Update 4 | swap-1 (PoE2-Chaos) + omen-like | ถอด 1 ใส่ 1 / ดัดผล craft ครั้งถัดไป |
| Update 4+ | **หินขัด (Divine-like)** | reroll ค่าทุกช่องภายใน tier เดิม — ตัวจบ craft / perfect-roll chase |
| ปลายทาง | fracture-like | ล็อค bonus 1 ช่องถาวร |

กติกาเพดาน determinism (บทเรียน PoE2 0.5): เครื่องมือเล็งได้กี่ช่องต่อใบ
ต้องมีลิมิต (เช่น การันตี/ล็อคได้ 1 ช่อง) — กันคันโยกกลืน RNG chase หมด.

## 4. ตีบวก (refine) — แกนเดิม + สองหน้าที่ใหม่

- Base stat ต่อ +N (เดิม) + **GEAR PASSIVE ปลดตาม milestone** (+5/+7/+9 —
  authored ต่อสาย gear, deterministic) — แก้ [gear-atk-minor-vs-stats]:
  ตีบวกสูงมีความหมายเกินเลข atk.
- **เลเวลสกิลจาก Skill Orb โตตามตีบวก** (Lv1 ติดตัว → Lv2 @+6 → Lv3 @+9,
  ตัวเลขปรับได้) — เปลี่ยน "สุ่มเลเวลสกิล" ของ ROO เป็น "เลเวลต้อง earn";
  refine/skill/ดาว ล็อคเข้าหากันเป็นระบบเดียว.
- Fail = downgrade + ore burned (ตาม [refine-ore-economy] — tension เดิมยังอยู่).

## 5. Skill Orb — สกิลบนไอเทมชั้น MINOR (DECIDED: คนละเวอร์ชันกับ unique)

- ผู้เล่นกดใช้เอง (agency-gamble ✓) สุ่ม**มิติเดียว**: ได้สกิลไหน จาก pool
  authored ต่อคลาส (เริ่ม 5-8 สกิล) — เลเวล**ไม่สุ่ม** (โตตาม refine, §4).
- **เกณฑ์พลัง: สกิล orb ทำให้ build ดีขึ้น ~5-10% / unique ทำให้ build "มีอยู่"** —
  ถ้าสกิล orb ตัวไหนแรงจนคน build รอบมัน = ย้ายไปเป็น unique.
- เอียง pool ไปทาง "เห็นผลบนจอ" (proc มี animation — ความสนุก idle คือมองปาร์ตี้)
  เช่น auto-cast bolt เล็ก, on-kill heal ระยิบ, on-hit spark.
- โครงนี้ = RO แท้ (gear auto-cast ธรรมดา vs god item) — ผู้เล่นรู้จักอยู่แล้ว.

## 6. การ์ด — แกนแยก ไม่แตะ (card-system.md): socket ฝังใน def, ดาวการ์ด = fuse,
foil, consume — ครบแล้ว. Rune ของ PoE2 = เวอร์ชัน simplified ของแกนนี้ → ไม่ลอก.

## 7. Uniques — "กุญแจของ build" (ชั้น STRONG, ตัวขายหลักตลาดเงินจริง)

ยกจาก v1: fixed 100% = fungible = listing เดียว/ใบ. เปลี่ยนกติกา ไม่แข่งตัวเลข.
ตัวอย่างทิศ: ไม้เท้า "Heal ทำดาเมจใส่ Shadow/Undead" · โล่ "ดาเมจ 20% หัก SP แทน HP" ·
Accessory "Weapon panel active เกิน socket +1" · ธนู "Double Strafe ยิงเป้าที่สอง" ·
เกราะ "เปลี่ยน element ผู้สวมเป็น Earth". โค้ด bespoke ต่อใบ — งบแรก 1-2 ใบ/คลาส.
Legendary 7 ชิ้นเดิมต้อง re-author เป็น build-enabler. แหล่ง: boss + gated dungeon
ใต้ mint-rate cap. Unique มีดาว/ตีบวกได้ไหม → open decision (เอียง: ตีบวกได้
(RO แท้), ดาว = ตายตัวตาม def).

## 8. Orbs + ตลาด (DECIDED)

- **Orb + วัสดุที่ใช้แล้วผลสุ่ม (รวมแร่ตีบวก): เทรดในเกมเท่านั้น (Gil) —
  ไม่ขึ้น Steam Market** (เกณฑ์ "ใช้แล้วสุ่มไหม" — บรรทัดถัดไป) →
  ไม่มี "เงินจริงโดยตรง→ผลสุ่ม" = gambling-law เหลือระดับพื้นหลัง.
  Orb = commodity หลักของตลาด Gil (เงินตลาดคือ Gil — orb เป็นสินค้า ไม่ใช่เงิน).
- **Steam Market (เงินจริง): fixed เท่านั้น** — เกณฑ์ตัดสิน (2026-07-04):
  **"ใช้แล้วสุ่มไหม"** — ของที่ใช้/ใส่แบบ deterministic ขึ้น Steam ได้
  (uniques / การ์ด / socket-variant / วัสดุ deterministic เช่นวัสดุอัพดาว /
  **gear บริสุทธิ์ = 0★ +0** — ของติดดาว roll ค่ามาแล้วตั้งแต่ดรอป =
  per-instance → ตลาด Gil); ของที่ใช้แล้วผลสุ่ม = **Gil เท่านั้น**
  (orb ทุกชนิด + **แร่ตีบวก** — เงินจริง→ของสุ่ม = โครง CS:GO key, เลี่ยง).
  ตีบวกแล้ว/roll แล้ว = per-instance → ตลาด Gil (กัน listing แตก def×ดาว×refine).
- **สะพานสองตลาด (by design):** ของ dual-listed ทุกชิ้นคือสะพาน — เกิดอัตรา
  แลกเปลี่ยน Gil↔เงินจริงโดยปริยาย; **การ์ด = bond ประจำเกมโดยธรรมชาติ**
  (แบบ OSRS Bond — sanctioned RMT ฆ่า gold-seller). Friction ไม่สมมาตรตั้งใจ:
  ขาเข้า (เงิน→Gil) ทันที / ขาออก (Gil→เงิน) ผ่านเรือ+verify+ค่าต๋ง+wallet-only.
- **"ถุงเงิน" (player mint Gil→ขาย Steam) = REJECTED 2026-07-04** — precedent
  D3 RMAH gold: Gil ไม่มี emission cap + bypass เรือทั้งระบบ + race to the
  bottom. เวอร์ชันกลับทิศแบบ PLEX (dev ขาย, เงินเข้าทางเดียว) = OPEN OPTION
  ของ S3 stack — และควรเป็น **"ไอเทมเวลา premium ที่เทรดเป็น Gil ได้"** แทน
  ถุง Gil เปล่า (บทเรียน EVE: PLEX ยืนได้เพราะ utility จริงค้ำราคา).
  **PRICING SPLIT (2026-07-04):** หลัก = **ความสะดวก ขายขาด / การเข้าถึง
  เศรษฐกิจ มีอายุ** → ปาร์ตี้ 2 = **ขายขาด ~300฿** (คู่แข่งคือ alt account ฟรี
  → sub ไม่รอด; slot 3,4 ขายขาดขั้นบันไดต่อได้) / PLEX anchor ที่เอียงไว้ =
  Merchant License (**ยังไม่ final** — user 2026-07-04; ถ้าไม่ทำ License,
  monetization ยืนได้ด้วยขาอื่น). (เกร็ด EVE: อย่าให้ mechanic เกมทำลาย
  ไอเทมแทนเงินจริงได้ — PLEX vault lesson.)
- **MARKET-LOCK (user 2026-07-04):** ไอเทมมีค่า (fixed class ที่เทรดได้) **โอน
  ผ่านตลาดเท่านั้น — ไม่มี direct trade/gift** → (1) จับ fee เต็ม volume:
  ฝั่ง Steam = income เงินจริง ~10%/ดีล, ฝั่ง Gil = sink คุมเงินเฟ้อ (คนละ
  บทบาท อย่าปน); (2) anti-fraud: ทุก flow อยู่บน ledger, ไม่มีช่องฟอกนอกตลาด;
  (3) precedent: GW2 Trading Post (no direct trade, 15%, 12 ปี — ต้องเป็นกติกา
  ตั้งแต่วันแรก อย่าเพิ่มทีหลัง). Honesty: fee = ขาเสริมโตตาม economy
  (Bongo-cat lesson: market % ≠ business model) — เสาหลักยัง cosmetics/pass
  + ขายขาด slot.
  Mapping มิติของขาว: socket=ฝัง def · link=ไม่ใช้ · quality=ตีบวก ·
  ilvl=tier ของ def · implicit roll=ไม่มี (จงใจทั้งหมด — fungibility ก่อน).
- **ตลาด Gil = สองพื้นผิว (user 2026-07-04):** (1) **order book** สำหรับของ
  fungible (orb/วัสดุ/gear บริสุทธิ์/การ์ด) + (2) **ร้านแผงลอยแบบ RO (ตั้งร้าน)**
  สำหรับของ **non-fungible** (roll/ตีบวก/ติดสกิลแล้ว) — ผู้ขายตั้งราคาเป็นชิ้น.
  แผงลอย: idle-native (ตัวละครนั่งขายระหว่าง AFK — โชว์บน companion strip =
  ป้อน CCU flywheel), เป็น presence layer ของกึ่ง MMO (GDD §9 — เมืองเต็มไปด้วย
  ร้านผู้เล่น = ภาพ Prontera), นับเป็น "ตลาด" ใต้ market-lock (ledger+ค่าต๋ง),
  และ**มาก่อน search-by-mod** (list+เดินดู สร้างถูกกว่า engine ค้น mod —
  search ข้ามร้านค่อยเป็น QoL ทีหลัง, ลำดับเดียวกับ RO จริง). ถ้าทำ Merchant
  License → License = ใบอนุญาตเปิดร้าน (anchor จับต้องได้; ยังไม่ final).
- ใช้ orb = `InputEvent`, roll จาก PRNG sim → เข้า replay-verify ฟรี.
- Way-A ramp-gate: roll ลงบน base ที่ต้อง farm ตาม band เอง.
- **Gil = สกุลตลาด ไม่ bound** (user 2026-07-03 — zeny model: โอนผ่านการซื้อขาย).
  ผลตาม: (1) orb จะ**ไม่**กลายเป็นหน่วยเงินแบบ Chaos — PoE เงิน-item เกิดเพราะ
  เขาไม่มีเงินเทรดได้; เรามี Gil เป็นเงินอยู่แล้ว → orb เป็นแค่ commodity;
  (2) แรงกดดัน RMT/gold-seller ลงที่ Gil ตรง ๆ (ปัญหาคลาสสิก zeny) — จัดการ
  ด้วยเฟรมเดิมของ brief: forged Gil = โกง (hash-chain จับ), ฟาร์มตามกติกา =
  ลูกค้า; (3) ราคา NPC (refine/enchant/respec) กระทบเศรษฐกิจรวม — ยอมรับ ปกติ
  ของ MMO. สกุล bound จริงของเรา = **Soul** (meta, ผูกบัญชี) ถ้าต้องการคันโยก
  จูนอิสระค่อยใช้ฝั่งนั้น.
- **Gil sinks (ยังใช้ได้หมด — sink เผาเงินจากระบบไม่ว่าเงินใคร):** ค่าต๋งตลาด
  ต่อ transaction (สเกลตาม volume เอง — ยิ่งสำคัญเมื่อ Gil คือเงินตลาดตัวจริง) ·
  gamble NPC (แหล่ง base + sink, OCA-flavor) · ค่า enchant/เสก · reset stats
  ควรเก็บ Gil (ตอนนี้ฟรี — RO แท้เก็บ) · recruit (ใหญ่สุด, มีแล้ว).
- **Base item explicit-band:** โชว์ band บนของตรง ๆ ("Sword (b9)") — band คือ
  ilvl ของเรา (gate bonus tier) แต่ไม่ซ่อนแบบ PoE (ผู้เล่นใหม่โดน ilvl หลอกทั้งเกม).

## 9. วินัยกันบวม (คงเดิม + อัพเดตบัญชีแกน)

บัญชีแกนหลัง v2 (การยุบ rarity เข้าดาวประหยัดไปหนึ่ง): (1) base/tier+socket-def
(2) ดาว+bonus+orb (3) ตีบวก (+passive+skill level — ส่วนขยายแกนเดิม)
(4) การ์ด(+foil+ดาวการ์ด) (5) Skill Orb (6) unique — **เหลือ headroom 1 แกน
จากเพดาน 7**. กติกาเดิม: RNG ชั้นเดียวต่อ acquisition (agency-gamble ไม่นับ),
สกิล = authored pool เท่านั้น, ระบบใหม่ต้องตอบคำถามผู้เล่นข้อใหม่.

## 10. Rollout (ทยอยปล่อย — หนึ่งระบบ/หนึ่ง update = retention beat)

| จังหวะ | ปล่อย | ของใหม่ให้ฟาร์ม |
|---|---|---|
| Launch | refine + การ์ด + socket variants | การ์ด, socket gear |
| Update 1 | ดาว+bonus (หินเสก/เขย่า) + วัสดุอัพดาว | orb, วัสดุดาว, ล่า 3★ |
| Update 2 | Uniques wave 1 + gated dungeon + refine passives | unique hunt, ore |
| Update 3 | ตลาด Gil order book (fixed) + deterministic orbs + **Skill Orb** | skill hunt |
| Update 4 | **ร้านแผงลอยแบบ RO** (ของ non-fungible เทรดได้ครั้งแรก) + swap-1/omen | ขายของ roll เทพ = อาชีพใหม่ |
| Update 5+ | search ข้ามร้าน (QoL) + หินขัด + star/fuse การ์ด | perfect-roll chase, dupes มีค่า |

ในเกม: ปลดแกนตาม band (เจอ NPC เสกครั้งแรก ~band 6 ฯลฯ) — เผยความลึกทีละชั้น.

## Open decisions

- ~~ช่วงค่า vs ตายตัว~~ RESOLVED 2026-07-04: roll เป็นช่วงทุก mod (§2)
- คานสำรองกัน glass-cannon meta: ถ้า roll รุกล้วน 5 ช่องครอบเกม → เพิ่ม
  **category cap** (mod สายรุก ≤3/ใบ) = หน้าที่กัน-stack ของ prefix/suffix
  โดยไม่ต้อง import ไวยากรณ์ P/S ทั้งระบบ (จดไว้ ยังไม่ทำ)
- Unique มีดาวไหม (เอียง: ไม่ — ดาวตายตัวตาม def; ตีบวกได้)
- อัตรา/ต้นทุนอัพ 4★→5★ + แหล่งวัสดุดาว (ผูก drop-system 2-axis)
- Enchant NPC กับ refine NPC คนเดียวกันไหม (fiction/UI)
- Skill pool รายคลาสชุดแรก (5-8 ต่อคลาส) — author ตอน Update 3
- rolled tooltip UI (งานแพงชิ้นเดียว)

## 11. ปรัชญา: ความลึกเป็นรางวัล ไม่ใช่ภาษี (2026-07-03 — ตัวคูณ ×5 ตัวละคร)

Tension: party 5 (→10) ตัว × ระบบลึก = management ภาษี 40 ช่อง / แต่ผู้เล่นสาย
craft build แปลก ๆ คือคนที่เราอยาก reward. ทางออก:

1. **Baseline ต้องรอด** — เนื้อหาหลักจูนให้ของ 0★ ไม่ enchant ผ่านได้;
   ความลึกซื้อความเร็ว+เพดาน (band สูง/dungeon/boss race) ไม่ใช่สิทธิ์เล่นต่อ.
   (สอดคล้อง: ความยากจูนผ่าน monster stats, gear = economy/expression)
2. **ห้าม maintenance loop บน gear** — ไม่มี durability/daily ต่อชิ้น/ของเสื่อม.
   ทุกแกน = "ลงทุนเมื่ออยาก" ไม่ใช่ "ดูแลไม่งั้นแย่ลง".
3. **ต้นทุน craft ชันพอให้โฟกัสชิ้นเรือธง** (1-2 ชิ้น/ตัว) — เศรษฐกิจกันภาษี
   40 ช่องเอง.
4. **Party คือ build** — ความลึก combination อยู่ชั้นจัดทีม ต่อตัวไม่ต้องลึกเท่า
   PoE; ของส่งต่อระหว่าง 5-10 ตัว = internal economy ที่ทำให้ craft ครั้งเดียว
   คุ้มหลายรอบ.
5. **คนลึกเลี้ยงคนตื้นผ่านตลาด Gil** — crafter ขาย mid-roll ให้ casual;
   รางวัลของสายลึก = กำไร+ศักดิ์ศรี (dynamics แบบ PoE crafting community).

## 12. Rebalancing gear (nerf/buff) — propagation + THE nerf lever (2026-07-09)

Nerfs are unavoidable (MTG / PoE / Diablo / RO all do them) — minimise, but plan for them.
How a def change propagates is a DESIGN LEVER, not a fixed cost.

**`migrate_item` (runs on load/resume) rebuilds every held copy from the current def, but
KEEPS per-instance earned state.** So a def change splits into two very different behaviours:

| Change the **base** (atk/def/matk/aspd, weapon_level) | Change the **roll RANGE** (affix / enchant / star bonus pool) |
|---|---|
| Hits **EVERY copy** (equipped/inventory/stash/rare_bag/…), existing + future, on next load | Hits **FUTURE DROPS ONLY** — existing copies keep their rolled affixes (per-instance, frozen at drop) |

**THE nerf lever — pick your blast radius:**
- **Nerf the base** = universal + instant, but it DESTROYS the power of gear already in players'
  hands (and the market value of tradeables → angry holders). Reserve for emergencies.
- **Nerf the roll range** = only future drops get weaker; existing copies are GRANDFATHERED for
  free (they keep their rolls). This is the SOFT nerf — trims future supply without touching
  held wealth. Prefer this for tradeable/unique power creep.
- Buffs mirror this: a base buff is a windfall to all holders + speculation; a roll-range buff
  only lifts future drops.

**Rebalancing a TRADEABLE item is an ECONOMIC act, not just a balance tweak** — a base nerf
crashes the market value of every held copy. Guardrails: prefer roll-range (grandfather) or
buff-only; decouple tradeable VALUE from power so a power nerf doesn't gut value (value rides
beauty/scarcity — see character-skins-idea). NB: a base atk buff/nerf barely moves late-game
combat anyway (gear-atk-minor-vs-stats) — real power levers are stats/skills, not base atk.

**Resume is SAFE across any rebalance** (2026-07-09 fix): the server migrate-on-resume re-trusts
the migrated hash, so a returning player never gets a bogus "not anchored". See
`online-robustness.md`. **Deploy discipline for a content change:** bump `game_data_version` +
redeploy server AND rebuild client TOGETHER (a client/server catalog mismatch causes silent
hash issues) + re-bless any golden the item appears in.
