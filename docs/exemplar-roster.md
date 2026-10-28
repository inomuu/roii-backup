# Commission Brief — *ROI: Reign of Idlers*
### งาน: Character Key Art (ภาพตัวละครอนิเมะ 2D)

> **⚠️ ชื่อ 2026-09-07: ระบบ ★ "Resonance" เปลี่ยนชื่อที่แสดงเป็น "Memory Fragment"**
> (โค้ด `resonance_stars` คงเดิม — เป็น field ที่ serialize ลงเซฟ) ดู `skill-system-redesign.md` §4.1
>
> เอกสาร brief สำหรับศิลปิน commission 
> สถานะ: kit spec ครบ 5/5 — Seraphine (Swordman/Luminary Knight) · Lucielle (Acolyte/Radiant Saint) ·
> Skadi (Mage/Winter Witch) · Elfie (Archer/Windwhisper) · Noire (Thief/Ballad of Twilight). เหลือ art + implement

---

## 0. Roster as built — 2026-09-19 (sim 0.0.444) — read this before any kit section below

The five units that ship. A character is a UNIT with a fixed kit, a Doctrine, an ATTACK element
(never a defensive one), an SSR Regalia whose passive is doctrine-gated, and six Resonance ranks.
Every number below is at Lv1 (+per level); "Resonance ladder" (S1-S6) = `unit_rank::ranks_of`; bench = 70 / T6 /
Legendary / stage 168 / SSR Lv60 (`sim-cli --bench`), dmg/h at S0 → S6.

| unit | doctrine | attack element | kit (basic · S1 · S2 · ult) | SSR Regalia | bench S0 → S6 |
|---|---|---|---|---|---|
| **Seraphine** (starter) | Sentinel | Physical | basic 80% · Iron Chains 100% +10%/lv (2 nearest, taunt, DEF+) · Steel Break 90% +9%/lv 3×3 · **Aegis** party shields 30% +4%/lv of her max HP each (0.0.460; gauge fills from blows taken), burst 200% ATK | Aurora Edge — taunt held → party −10% dmg (S5 −20%), own shield breaks → ATK +30% 10 s | 52 → 110M (+112%) |
| **Lapis** (pull) | Striker | Fire | basic 100% · Fireball 600% +60%/lv 3×3, cd 6 s · Ember Cascade 160% +40%/lv on every break · **Fire Cat Meteor** 750% +75%/lv room | Empyrean — +5% ATK per enemy on the field (max 5), a break → +40% ATK 8 s | 128 → 256M (+100%) |
| **Lucielle** (pull, **SR** since 0.0.469) | Support | Light | Sagitta Lucis 50% Holy beam range 3 · Lux Medica heal 50% ATK +5%/lv · Lumen Contagio 90% + Holy-vuln chain · **Benedictio** party ATK +30% +3%/lv **120 s** (13 casts/h ≈ 43% uptime) · Communio: every cast heals Light / Sentinel allies 10% ATK | Litany of Dawn — heal +20%, healed ally +20% ATK 8 s | 42 → 43M (support) |
| **Aveline** (pull, **SR** since 0.0.452) | Striker | Dark | Umbra Fletch basic 55% Dark + Umbral Mark (5%/stack ×3) · Nightfall Volley 58% +4.2%/lv × 3 targets · Shadowbind 67.5% + bleed 135% +11.25%/lv · **Eclipse Rain** 1,600% +110%/lv 5×5 on the boss | Meridian — debuffed target +30%, range ≥ 4 +15% | 84 → 126M (SR: 65% → 98% of Lapis S0) |
| **Raplus** (pull; id `nyx`, 0.0.464) | Catalyst | Dark | Sticky Fingers: +10% per distinct status on the target; a monster she last-hits pays +20% Gil · Pickpocket: no damage, Dazzle (whiff 20%, boss 10%) 8 s + 10% of the target's price at once · Rend 150% +15%/lv + bleed 225% · **Grand Heist** 500% +37.5%/lv on EVERY enemy + Dazzle + Rend + 10% of every price | Silent Verdict — each status applied → +6% taken (×3), target under half +25% | 124 → 245M (+98%) |
| **Astier** (pull, 0.0.468) | Support | Water | Unda Vitae: 50% Water r3, a quarter of the damage heals the worst-off, and every ally she heals carries +4% DMG ×3 8 s · Rivus heal, 3 bounces 100/70/50% · **Tidebless** (cd 18 s): party +20% +2%/lv DMG 12 s + one rolled element +10% · **Fons Vitae**: party HoT 5% +0.5%/lv max HP/s 8 s + everyone's gauge charges ×2 | Tidecaller (SSR piece) — HP +12%, an ally she heals takes −15% damage 8 s | her seat vs Lucielle's at 168: party 416 vs 374M/h (+11%) |
| **Suzu** (pull, **SR**, 0.0.465) | Destroyer | Fire | Bright Edge: plain 110% Fire basic · Sunder 3×3 150% +15%/lv, Toughness ×2, −20% DEF 8 s · Bell Strike 190% +19%/lv, Toughness ×2, cd clears on ANY break · **Break Fever** 15 s: ASPD +40%, basic chips ×2, breaks anything regardless of element | Emberbell (SR piece) — ATK +12%, a break → ATK +30% 8 s | party bench 168: 86 → 132M |
| **Mirei** (pull, **SR**, 0.0.465) | Catalyst | Thunder | Conductor: 90% Thunder r6, aims at her sparked target, fires a spark without spending it · Storm Field 5×5 140% +14%/lv + **5 Static Sparks** (150% ATK each, anyone's hit sets one off and spends it) · Quicken self ASPD +30% · **Thunderfall** room 500% +37.5%/lv + detonates every spark | Stormglass Pin (SR piece) — ATK +12%, each status laid → ATK +5% ×4 | party bench 168: 84 → 121M |
| **Charon** (pull, 0.0.463) | **Destroyer** | Dark | Ferryman's Toll: basic 85% Dark, oar chips Toughness ×2, hits on a Broken target +30% · Tolling Blow 3×3 100% +10%/lv, Toughness ×3 · Blackwater Mark (boss first) 120% +12%/lv + seal 10 s: her hits chip a non-weak target at half, a weak one ×1.5 · **Passage of No Return** 5×5 400% +40%/lv, Toughness ×5, gauge from breaks | Ferryman's Lantern (SSR piece) — ATK +12%, a break → ATK +30% 8 s | party bench 168: 143 → 276M (+93%); TEAM +16% vs Aveline in the seat (breaks open the target to everyone); stage 184 (no Dark weakness) 91M |
| **Airi** (pull, **SR**, 0.0.462) | Striker | Thunder | Static Shot: basic 70% Thunder r6, every hit stacks Static (+2% Thunder taken ×3); every hit on a taunted target +20% · Chain Bolt 90% +9%/lv on the 3 lowest · Overload self ASPD +30% +2%/lv 6 s · **Storm Volley** 350% +35%/lv on the 5 lowest | Thunderhead (SR piece) — ATK +12%, target under half HP +20% | party bench 168 beside Lumi: 83 → 121M (SR: 65% → 94% of Lapis S0); beside Seraphine 80 → 117M |
| **Lumi** (pull, **SR**, 0.0.456) | Sentinel | Ice | Chill Touch: basic 55% Ice; every hit stacks Chill (+2% Ice AND Thunder taken / −4% move / −4% ASPD ×3) · Frost Taunt 5×5 80% +8%/lv + taunt 5 s · Ice Mend self-heal 10% +1%/lv max HP · **Frost Aura** party DEF +20% +2%/lv 30 s, attackers of the party get her Chill | Hoarfrost Crown (SR piece) — DEF +16%, her ult heals the party 10% max HP | party bench stage 200: 42 → 61M (SR: 68% → 98% of Seraphine S0 62M) |
| **Nemu** (pull, 0.0.451) | Striker | Dark | Sweet Poison: basic 90% Dark r6; every hit stacks +3% Dark taken / −3% move / −5% ASPD ×10 · Velvet Bog 3×3 pool 6 s, slow 50%, 25% +2.5%/lv per 0.5 s · Heartstring tether 360% +36%/lv over 6 s, 10% leeched · **Abyssal Embrace** (Enigma's Black Hole) 3×3 hole 3 s pulls 1 cell / 0.5 s + Root, 1,800% +180%/lv 5×5 on close | Lycoris Lullaby — each status applied → ATK +5% (×6, S5 +10%) | 122 → 266M (+117%) |

Resonance ladders (S3 = the two skills +3 levels, S5 = the ultimate +3, the rest mechanics):

| | S1 | S2 | S4 | S6 |
|---|---|---|---|---|
| Seraphine | Chains 2→4 targets, caught +10% dmg taken 6 s, Chains +60% DMG | DEF +2%/hit taken while taunting (×10); Steel Break on ≥3 → cd −2 s | Aegis also +20% ATK to the party 30 s, Steel Break +60% DMG | Aegis burst ×2, room taunt 3 s, each break refunds 100 gauge, Steel Break +100% DMG, Chains +40% DMG |
| Lapis | Fireball on ≥3 → cd −2 s, Fireball +100% DMG | a kill charges 30 gauge | kills detonate 3×3 50% ATK, Fireball +100% DMG | Meteor leaves +20% dmg taken 10 s, Meteor +200% DMG, Ember Cascade +100% DMG |
| Lucielle | Contagio spreads 3×3, heals splash 50% | Communio reaches everyone at 15% | Benedictio +8% ATK more, healed ally DEF +10% | Benedictio also carries +13% DMG and heals 30% max HP |
| Aveline | Mark 3→4, lasts ×2, Volley +30% DMG | Shadowbind bleeds 2× faster; an arrow on a capped mark charges 8 gauge | Volley +2 targets | Mark 10%/stack, Eclipse +100% DMG |
| Raplus | Dazzle ×1.5 duration, spreads to the 3×3, Rend +80% DMG | Rend stacks ×3 | Exploit +50%/status, Rend +80% DMG | Grand Heist resets Pickpocket + Rend, Heist +220% DMG, Rend +100% DMG |
| Astier | rolled element +10% | an overheal leaves 50% as a shield | rolled element +15% | Rivus splashes, Tidebless +6 s |
| Suzu | Sunder on ≥3 → cd −2 s | the swept take +15% dmg 6 s; a kill charges 25 | Break Fever lasts +5 s | every break extends a live Fever +2 s, Sunder +30% |
| Mirei | Storm Field lays 3 more sparks | a spark on a target under 50% HP is not spent | spark +40% ATK | **Quicken buffs the whole party**, a kill charges 25 |
| Charon | Tolling Blow +80% DMG | a kill charges 30 gauge | Tolling +100%, Blackwater +120% | Passage leaves +20% taken 10 s, Passage +550%, Tolling +200% |
| Airi | Chain +1 target, +20% DMG | a kill charges 25 gauge | Storm +20%, Chain +15% | Static ×5, Storm +20%, Chain +10% |
| Lumi | Chill 3→5 stacks, Frost Taunt +25% DMG | Frost Taunt on ≥3 → cd −2 s | Frost Taunt leaves +5% dmg taken 6 s, +20% DMG | Chill +0.5%/stack, Frost Taunt +30% DMG |
| Nemu | Poison 10→15 stacks, Bog +180% DMG | Poison +4% Dark taken per stack, Heartstring +60% DMG | Heartstring ×2 targets, +150% DMG | Embrace leaves +20% taken 10 s, +350% DMG; Bog +220% DMG |

Party shares at S0 on the bench: Lapis 29 / Aveline 25 / Raplus 24 / Seraphine 15 / Lucielle 8.
Gauge: every ultimate needs 1,000; +2/s, +4 per basic (Aegis 2), +10-15 per skill; none from
damage taken. Every attack element but Fire's twin now has a unit (Ice: Lumi 0.0.456, Lightning: Airi 0.0.462). SR rule
(2026-09-21): an SR's S0 ≈ 65% of the same-line SSR, its Resonance ladder +40-50% (S6 ≈ 95% of SSR S0); an SSR's ladder is
+100-150% (0.0.460, user 2026-09-22 after the HSR read — E6 there is +150-300%);
Aveline is the first SR (0.0.452): S0 84M = 65% of Lapis S0, S6 126M = 97%. Bench numbers from 0.0.451 on include
DoT ticks in the meter (Rend / bleeds / Heartstring were never credited before).

## 1. เกมของเราเกี่ยวกับอะไร

**สรุปหนึ่งบรรทัด:** *ROI: Reign of Idlers* คือ **idle action RPG** ได้แรงบันดาลใจจาก
Ragnarok Online + เกมแนว Idle เช่น Task Bar Hero — ผู้เล่นจัดปาร์ตี้ฮีโร่ แล้ว นั่งดูปล่อยพวกเธอออโต้สู้ ฝ่าดันเจียน ขณะที่เราค่อยๆ
ปั้น build / เกียร์ / อัพ skill stat / จัดการทรัพยากรต่างๆ

**ลูปการเล่น:** รวบรวมปาร์ตี้ → ออโต้สู้มอนสเตอร์แบบเรียลไทม์ → ดรอปอาวุธ/เกราะ/การ์ด/แร่ตีบวก →
ยกระดับ stat/สกิล/เกียร์เสก → ดันโซนยากขึ้น หาอาวุธของ loot ดีๆ เพื่ออัพเกรดตัวละคร และ แลกเปลี่ยนกับผู้เล่นอื่นๆ —
ความสนุกคือการจัดปาร์ตี้ แล้วดูว่าแผนการของเรามันออกมาเป็นยังไงบ้าง

ในเกมนี้มีตัวละครแบบธรรมดาที่ผู้เล่นสายฟรี สามารถปลดล็อคได้ทันที โดยจะมี Base ตัวละคร Pixel Art และ Art 2D แบบแนบในตัวอย่าง

แต่เกมนี้จะมีตัวละครที่เรียกว่า Exemplar คือฮีโร่ signature ที่พิเศษต่างกับตัวละครธรรมดา — หน้าตาที่มีบุคลิกของแต่ละคลาส
เป็นตัวละครพิเศษ ของเกมที่มี Bonus Status + skill พิเศษ ต่างๆ 


---

## 2. ทิศทางศิลป์รวม (Art Direction)

ตัวเกมเพล เกมรันบน **โลก pixel-art** แต่งานนี้คือ **ชั้นภาพตัวละครอนิเมะ 2D แยกต่างหาก**
(portrait / key art / gacha reveal) → วาดเป็น **illustration 2D ความละเอียดเต็ม**

**โทน — เซ็กซี่แบบ elegant มีรสนิยม ไม่ 18+ มากเกินไป**
Reference **Blue Archive / Azur Lane / Brown Dust / Nikke** 


**โลก & บรรยากาศ:** fantasy 


**Reference บรรยากาศในเกม:**
- **Guardian Tales** 
- **Blue Archive / Azur Lane** 
- **Brown Dust 2** 
- **Octopath Traveler / Blasphemous** 

**ความสม่ำเสมอของ roster (สำคัญมาก):** นี่คือตัวที่ **1 จาก 5** (ผู้หญิงล้วน จงใจให้หลากหลาย —
นักดาบ, นักบุญ, จอมเวท, นักธนู, โจร) ทั้ง 5 ต้องใช้ **สไตล์ลายเส้น การลงสีแบบเดียวเดียวกัน**




---

### 3.1 ตัวละครที่ 1 — Swordman - **Seraphine(name not final)**

| | |
|---|---|
| **บทบาท** | อัศวินแนวหน้า |
| **ธาตุ** | ศักดิ์สิทธื์ |
| **วัย / หุ่น** | ผู้หญิง วัยรุ่นตอนปลาย หน้าอกใหญ่พอประมาณ |

**บุคลิก / archetype:** มีวินัย — สง่า สงบ ยึดหน้าที่ ปกป้องเงียบๆ เป็นกำแพงที่ปาร์ตี้
ยืนอยู่ข้างหลัง ความอบอุ่นโผล่แค่รอยยิ้มบางๆ นานๆ ครั้ง

**ดีไซน์:**
- **เกราะ:** ชุดเกราะสีเงิน + ฟ้า + เข้ารูป + เปิดส่วนน่าสนใจ 
- **ผม:** บลอนด์ทอง
- **อาวุธ:** ดาบใหญ่ / ดาบ / ดาบโล่ อะไรก็ได้

- **Palette:** เหล็ก/เงิน + น้ำ + บลอนด์ทอง



---

### 3.2 ตัวละครที่ 2 — Acolyte — **Lucielle, Radiant Saint** (provisional; art มีแล้ว: "Acolyte Sample 01")

| | |
|---|---|
| **บทบาท** | นักบุญ support/healer แนวหลัง (ระยะไกล) |
| **ธาตุ (ธีม)** | Holy — *label ช่วย player จัดทีม; combat = Neutral ปกติ (จองไว้ให้ mechanic อนาคต hook)* |
| **วัย / หุ่น** | ผู้หญิง วัยรุ่น หน้าอกใหญ่ (โทน roster เดียวกัน) |

**บุคลิก / archetype:** แม่ชี/priestess สงบ ศรัทธา — เปล่งประกายแสง

**ดีไซน์ (ตาม art ที่วาดแล้ว):**
- **ชุด:** จีวรขาว + ซับในน้ำเงินเข้ม + ขลิบทอง, veil กางเขน, ถุงน่องขาว, ปีกใส
- **ผม:** เงิน/ขาว ยาว
- **อาวุธ:** คทากางเขนยาว มี **เฟือง/กลไก (clockwork)** ฝังในกางเขน
- **Palette:** ฟ้าอ่อน / น้ำเงินสด / น้ำเงินเข้ม / ทอง

---

## 4. Exemplar Kit — **Lucielle, Radiant Saint** (Acolyte · Holy) — FULL SPEC

> **BUILT AS A UNIT 0.0.421 (2026-09-18), REDESIGNED with the user — supersedes §4.1-4.5 below:**
>
> | slot | skill | กติกาที่ใช้จริง |
> |---|---|---|
> | Passive | Sagitta Lucis | basic = ลำแสง Holy ระยะ 3, 50% ATK (คงเดิม) |
> | Skill 1 | **Lux Medica** (ใหม่) | ฮีลเป้าเลือดต่ำสุด ระยะ 6 **cd 8 วิ** = **50% ATK +5%/lv** (Lv5 70%, Lv10 95%) · `heal_scales_atk` |
> | Skill 2 | Lumen Contagio | ดาเมจ 60% ATK Holy + Holy-vuln + ระเบิดลูกโซ่ (คงเดิม) |
> | Ultimate | **Benedictio Lucis** → ult | Limit Break gauge (lb_on_skill 15 / basic 5 / hit 4 / sec 2) · ทั้งทีม **+20% ATK +2%/lv ของ ult** 30 วิ — ไม่อ่าน ATK ของเธอ |
> | Passive 2 | **Communio** (ใหม่) | ทุกครั้งที่ cast สกิลใดก็ตาม → เพื่อนที่ **ธาตุ Holy** หรือ **doctrine Sentinel** ฮีล **10%** ของ ATK เธอ |
>
> ทุกสกิล scale จาก ATK (MATK = ATK ตั้งแต่ 0.0.336) ยกเว้น ult · ดาว: Lux Medica + Contagio Lv5→10 · board Support k1 = Medica, k2 = Contagio, ult = Benedictio · unit def `lucielle` ใน catalog (stat block acolyte, staff, Holy, ranged 3)


> ดีไซน์ 2026-07-17. อัพเกรด **ผ่าน Resonance ★ เท่านั้น** (fixed skill — ไม่ใช่ skill point).
> **ไม่มีระบบ SP** (cooldown = limiter เดียว, `sp_cost=0`). ตัวเลขเป็น bp (10000=100%) / ticks (10t=1s).

### 4.0 Exemplar meta + star→level mapping
- `exemplar_def`: id `lucielle`, name "Lucielle", title "Radiant Saint", **element label = Holy** (data จริง, โชว์เป็น guide จัดทีม; ยังไม่มี mechanic — จองไว้)
- **Base stat grant (0★, `CardEffect`/card_bonuses):** +5 INT, +10 MATK *(mirror Seraphine 0★ = +5 STR/+10 ATK; ⚠️ปรับได้)*
- **Star→level (mirror Seraphine 0★=Lv5 → 5★=Lv10):** สกิลที่สเกล (Benedictio, Lumen Contagio) grant ที่ **Lv5 @0★**, ทุก ★ re-level **+1** → **Lv10 @5★**. *Sagitta Lucis* = flat, อยู่ **Lv1 ทุก ★**
  - `skills` (0★): `[Sagitta Lucis Lv1, Benedictio Lucis Lv5, Lumen Contagio Lv5]`
  - `star_ranks[i]` (i=1..5): re-level `Benedictio` + `Lumen Contagio` เป็น `Lv(5+i)` (list id ซ้ำที่ level สูงขึ้น = re-level ตามระบบ Resonance)

### 4.1 Passive — *Sagitta Lucis* (named Passive SkillDef, **โชว์ในหน้า skill**)
| field | value |
|---|---|
| kind / max_level | Passive / 1 (flat ทุก ★) |
| ผล | basic attack: `is_ranged=true`, `atk_range=3`, ดาเมจ = **50% MATK (5000 bp)**, `damage_type=Magic`, `cast_element=Holy` |
| skill_display | name "Sagitta Lucis" · blurb: "การโจมตีปกติกลายเป็นลำแสงศักดิ์สิทธิ์ระยะไกล — ทำดาเมจเวท 50% MATK ธาตุ Holy" |
| plumbing ใหม่ | **basic-attack profile override** — engine ต้องอ่าน passive นี้แล้วคิด basic เป็น %MATK/Magic/Holy/ranged (ไม่ใช่แค่ stat add) |

### 4.2 Active #1 — *Benedictio Lucis* (party buff, ต้องร่าย)
| field | value |
|---|---|
| kind / max_level | Active / 10 |
| sp_reserve_slots | 2 |
| cooldown_ticks | 280 (28s) |
| cast_time_ticks | **30 (3s base — variable cast)** · ลดด้วย DEX/INT ตามกลไก `effective_cast_time` (เหมือน Sancta Tempestas — DECIDED) |
| motion_delay_ticks | 5 (0.5s act-lock หลังร่าย) |
| range | 0 (party buff, cast เมื่อ due) |
| cast_buff_party | true · duration 300t (30s) → > cd → uptime ~100% |

**Buffs (ค่าที่ level L, สูตร linear):**
| buff | สูตร | Lv5 (0★) | Lv10 (5★) | kind |
|---|---|---|---|---|
| +flat ATK | 5·L | +25 | +50 | `AtkAdd` (มีอยู่) |
| +flat MATK | 5·L | +25 | +50 | **`MatkAdd` (ใหม่)** — *ดู Decision A* |
| +Holy dmg dealt % | 500·L bp | 2500 (25%) | 5000 (50%) | **`HolyDmgDealtPct` (ใหม่)** — attacker-side, apply เฉพาะ `dmg.element==Holy` |

### 4.3 Active #2 — *Lumen Contagio* (spreading light-plague)
| field | value |
|---|---|
| kind / max_level | Active / 10 |
| sp_reserve_slots | 2 |
| cooldown_ticks | 80 (8s) — แค่ seed ลูกโซ่ |
| motion_delay_ticks | 5 (0.5s) · cast_time_ticks 0 |
| range | 6 |
| targeting | **ศัตรูใกล้สุดที่ยังไม่มี `LumenVuln`** (predicate ใหม่); ถ้าทุกตัว mark แล้ว = no-op |
| cast damage | **ไม่มี** (cast = แปะ mark ล้วน) |

**Cast แปะ debuff (chance_bp 10000, guaranteed):**
| | สูตร | Lv5 (0★) | Lv10 (5★) | kind |
|---|---|---|---|---|
| Light-vuln | 300·L bp | 1500 (15%) | 3000 (30%) | **`LumenVuln` (ใหม่)** → sum เข้า `holy_vuln_bp` (ADD ทับ Sanctus Terra) |
| duration | — | 100t (10s) | 100t | *Decision B* |

**On death (ศัตรูที่ติด LumenVuln ตาย) → schedule `PendingExplosion` (delay 2t = 0.2s):**
| | สูตร | Lv5 (0★) | Lv10 (5★) |
|---|---|---|---|
| ระเบิด radius | — | `ground_field_radius=2` (5×5) | 5×5 |
| ดาเมจ | 500·L bp MATK, Magic, Holy | 25% MATK | 50% MATK |
| popup | **1 เลขจริง/ตัว** (field_strike_hits=1) | | |
| contagion | ระเบิดแปะ `LumenVuln` (level เดียวกัน) ให้ศัตรูใน 5×5 ที่ยังไม่มี → **ตายก็ระเบิดต่อ = ลูกโซ่ไม่จำกัด** | | |

- **Determinism:** ระเบิดหน่วง 2t + process spawn-order → ไม่มี loop ในทิคเดียว (แพร่ต้องรอตายจริง). ทุก field ใหม่ skip-if-default/zero → goldens byte-identical
- ระเบิดเป็น Holy → โดน `holy_vuln_bp` ของเป้าเอง (รวม LumenVuln) → synergy

### 4.4 Synergy loop
Contagio ปูพรม **+Holy-vuln** ทั้งแพ็ก → Benedictio ดัน **+Holy dmg** ทีม → ทีม Holy (basic Lucielle, Holy Light,
Seraphine's Sanctus Terra/Fiat Lux) ยิงเข้าเป้าที่กินเพิ่ม → ตาย → ระเบิดแพร่ต่อ = **เครื่องจักร Holy เลี้ยงตัวเอง** → จูงใจ mono-Holy

### 4.5 ⚠️ Open decisions (เคาะก่อน implement)
- **A. Benedictio "+Dmg flat" = ATK อย่างเดียว หรือ ATK+MATK?** — แนะนำ **ทั้งคู่** (ให้ทั้ง physical + caster ได้; ต้องเพิ่ม `MatkAdd` kind). ถ้า ATK อย่างเดียว → caster ในทีมไม่ได้ประโยชน์
- **B. Light-vuln duration** = 100t (10s) พอไหม (ต้องนานพอให้เป้าตายทั้งยังติด mark)
- **C. Lumen Contagio range/target** = range 6, ตัวใกล้สุดที่ยังไม่ mark — โอเคไหม
- **D. Base stat grant** = +5 INT/+10 MATK @0★ (สเกลต่อ ★ ไหม หรือคงที่)
- **E. Sagitta basic 50% MATK** = flat ทุก ★ (ไม่สเกล) — โอเคไหม
- **F. 2 Lucielle ซ้อนกัน:** Benedictio = buff instance เดียว (refresh); LumenVuln = same-kind **MAX** (ไม่ sum). คง policy มาตรฐานไหม
- **G. Cast priority** เมื่อ 2 active due พร้อมกัน: รักษา Benedictio (uptime) ก่อน, Contagio ตาม cd — โอเคไหม

### 4.6 Build checklist (ให้ session โค้ด)
- **BuffKind ใหม่:** `HolyDmgDealtPct` (attacker, gate element==Holy) · `LumenVuln` (→ `holy_vuln_bp`) · `MatkAdd` (ถ้า Decision A = ทั้งคู่)
- **basic-attack profile override** (Sagitta): ranged + `atk_range` + basic = %MATK Magic Holy
- **Lumen Contagio:** `try_cast_*` targeting predicate (nearest un-marked) + on-death Holy explosion (reuse `maybe_poison_explosion`/`PendingExplosion` path) + contagion re-mark
- catalog (`skills/catalog/exemplar.rs`) + `skill_display` (3 ชื่อ+blurb) + `exemplar_def(lucielle)` + star_ranks + goldens re-bless เฉพาะถ้า golden เรียนสกิลนี้
- ทุก SkillDef field/BuffKind ใหม่ = append-only + skip-if-default → **goldens byte-identical**

### 4.7 Master parameter table (implementation-ready — decisions A–G = ตามแนะนำหมด)

| field (SkillDef) | Sagitta Lucis | Benedictio Lucis | Lumen Contagio |
|---|---|---|---|
| id | `sagitta_lucis` | `benedictio_lucis` | `lumen_contagio` |
| kind | Passive | Active | Active |
| max_level | 1 | 10 | 10 |
| sp_reserve_slots | 0 | 2 | 2 |
| **cast_time_ticks** (wind-up bar) | — | **30 (3s, ยืนร่าย)** | 0 |
| **motion_delay_ticks** (act-lock หลังร่าย) | — (ใช้จังหวะ basic) | 5 (0.5s) | 5 (0.5s) |
| cooldown_ticks | — (basic ตาม `atk_speed`) | 280 (28s) | 80 (8s) |
| anim_slot | — (basic anim, ranged) | `special_1` | `special_2` |
| range / atk_range | `atk_range=3` | 0 (party buff) | 6 (หาเป้า) |
| is_ranged | true | — | — (cast ใส่ mark ทันที) |
| proj_travel_ticks | 0 (hitscan beam) | — | — |
| damage_type | Magic | — (ไม่มีดาเมจ) | Magic (ระเบิด) |
| cast_element (ธาตุดาเมจ) | Holy | — (VFX Holy) | Holy (ระเบิด) |
| power (ดาเมจ) | 50% MATK = 5000 bp (flat) | — | ระเบิด 500·L bp MATK (Lv5 25% → Lv10 50%) |
| ground_field_radius | — | — | 2 (5×5, ขอบเขตระเบิด) |
| field_strike_hits (popup/ตัว) | 1 (basic) | — | 1 |
| **explosion delay** | — | — | 2t (0.2s, `PendingExplosion`) |
| cast_buff_party | — | true (duration 300t=30s) | — |
| buffs ที่ให้ | — | AtkAdd 5·L · MatkAdd 5·L · HolyDmgDealtPct 500·L bp | — |
| on-cast debuff | — | — | LumenVuln 300·L bp, dur 100t (10s), chance_bp 10000 |
| on-death trigger | — | — | ถ้าเป้าติด LumenVuln → schedule ระเบิด + contagion re-mark (unmarked ใน 5×5) |
| targeting | basic auto (ใกล้สุดในระยะ) | ทั้งทีม (ally เป็น) เมื่อ buff ใกล้หมด | ศัตรูใกล้สุด **ที่ยังไม่มี LumenVuln** (ทุกตัว mark = no-op) |
| chain | — | — | ไม่จำกัด (แพร่ผ่าน death) |
| scaling (★→level) | flat ทุก ★ (Lv1) | Lv5@0★ → Lv10@5★ | Lv5@0★ → Lv10@5★ |
| act-lock (หยุดขยับ/ตี) | ไม่ (basic ปกติ) | 5t (0.5s หลังร่าย) | 5t (0.5s หลังร่าย) |
| มี cast bar | — | **ใช่ (3s, ลดด้วย DEX/INT)** | ไม่ (cast_time=0, ติดทันที) |
| skill_display name | Sagitta Lucis | Benedictio Lucis | Lumen Contagio |
| blurb (ผู้เล่น) | "การโจมตีปกติกลายเป็นลำแสงศักดิ์สิทธิ์ระยะไกล — 50% MATK ธาตุ Holy" | "อวยพรทั้งทีมด้วยแสง — เพิ่มพลังโจมตี/พลังเวท และดาเมจธาตุ Holy" | "สาปศัตรูให้อ่อนแอต่อแสง; เมื่อมันตายจะระเบิดเป็นแสงศักดิ์สิทธิ์และแพร่คำสาปสู่ศัตรูรอบข้าง" |
| plumbing ใหม่ | basic-attack profile override | `MatkAdd`+`HolyDmgDealtPct` kinds | `LumenVuln` kind + target-predicate + on-death Holy explosion + contagion |

**สูตร linear (value ที่ level L):** `AtkAdd/MatkAdd = 5·L` · `HolyDmgDealtPct = 500·L bp` · `LumenVuln = 300·L bp` · `explosion = 500·L bp MATK`
→ L=5 (0★) / L=10 (5★) ให้ช่วง 25→50, 25%→50%, 15%→30%, 25%→50% MATK ตามลำดับ

**BuffKind ใหม่ (append-only, skip-if-default):**
| kind | ทิศทาง | อ่านที่ | stack |
|---|---|---|---|
| `MatkAdd` | flat MATK (buff) | `buffed_stats` fold (คู่ `AtkAdd`) | refresh/instance เดียว |
| `HolyDmgDealtPct` | attacker %, gate `dmg.element==Holy` | `apply_damage` (step คูณใหม่ หรือ fold mastery-like) | instance เดียว |
| `LumenVuln` | defender holy-vuln | `holy_vuln_bp` (ADD ทับ SanctusTerra; same-kind = MAX) | MAX |

**Star ranks (`exemplar_def(lucielle)`):**
| ★ | skills granted (re-level) | stat |
|---|---|---|
| 0★ | Sagitta Lv1, Benedictio Lv5, Lumen Lv5 | +5 INT, +10 MATK |
| 1★–5★ | Benedictio Lv(5+★), Lumen Lv(5+★) | (คงที่) |

### ชื่อสกิล (Latin)
Passive *Sagitta Lucis* (ลำแสง) · #1 *Benedictio Lucis* (พรแห่งแสง) · #2 *Lumen Contagio* (แสงแพร่กระจาย)

---

### 3.3 ตัวละครที่ 3 — Mage (Fire) — **Lapis** ⚠️ ยังไม่ได้ออกแบบ

> แทนที่ Skadi ตั้งแต่ 2026-09-07 · ธาตุ **Fire** · ยังไม่มี art ยังไม่มี kit

<details><summary>ดีไซน์เดิมของ Skadi (Ice) — เก็บไว้อ้างอิง</summary>

#### Skadi, Winter Witch (ปลดออกจาก roster)

| | |
|---|---|
| **บทบาท** | จอมเวทน้ำแข็ง — DPS/combo controller แนวหลัง (ต่างจาก Lucielle ที่ support) |
| **ธาตุ (ธีม + damage)** | Water/Ice — สกิลทำดาเมจ Water จริง (mage ร่ายธาตุ) |
| **อาวุธ / basic** | staff, ranged Magic range 6 — **basic คง Neutral เดิม** (ไม่แปลง) |

**Character design (art brief — sexy elegant, Nikke/BD2 tone):**
- **Archetype / personality:** Aloof ice queen — composed, regal, distantly seductive; warmth surfaces only in a rare faint smile
- **Age / body:** Grown woman (early 20s), tall, cinched waist, **full bust**, regal noble bearing
- **Outfit:** A **tastefully revealing** winter-sorceress ensemble — white / ice-blue / silver; bare shoulders + a **cleavage window**, a fur-trimmed mantle open at the front, a **high slit showing leg** with long stockings / knee-high boots, sheer frost-panel fabric, crystal & snowflake ornaments
- **Hair:** Long silver-white, flowing
- **Weapon:** Ice-crystal scepter / frost tome
- **Palette:** White / ice-blue / pale cyan / silver / deep navy (accent)
- **Key art / mood:** One hand raised weaving ice, a cold piercing gaze, a swirl of snowflakes orbiting her
- **Reference:** Brown Dust 2 (elegant sorceress), Nikke, Azur Lane ice-themed

> ⚠️ **tone note:** Nikke/BD2 sit in the **Mature** band (spicier than Blue Archive) — at odds with the **Rated Teen** positioning in the store/monetization docs. Keep the spice **below the true BD2/Nikke ceiling**, or revisit the rating (see memory: spice-ceiling-vs-rating). Decide the dial before briefing the artist.

---

## 5. Exemplar Kit — **Skadi, Winter Witch** (Mage · Water) — ⛔ PARKED 2026-09-07

> **Skadi ถูกตัดออกจาก roster** ช่อง Mage เปลี่ยนเป็น **Lapis ธาตุ Fire** ดีไซน์ด้านล่างเก็บไว้เป็นฐาน
> สำหรับตัวละครธาตุ Water ที่จะมาทีหลัง อย่านำไป implement ตามนี้โดยตรง — เขียนบนระบบเก่า SP/reserve

### 5.-1 เนื้อหาเดิม

> **ชื่อสกิล (ชุด Norse Frost):** stack = **Icicle** · ① Passive **Shatter** · ② Active **Frostbind** · ③ Active **Fimbulwinter**

> ดีไซน์ 2026-07-17. อัพเกรดผ่าน ★ เท่านั้น. **ไม่มี SP** (cooldown limiter). bp (10000=100%) / ticks (10t=1s).
> **Freeze verified:** `Entity::take_hit` = จุดเดียวที่ freeze แตก (ตี/field ตรงๆ = แตก; DoT ไม่แตก);
> ศัตรู frozen → เกราะ = Water element (RO). Boss = freeze-immune (มีอยู่แล้ว).

### 5.0 Meta + star→level
- id `TBD`, element label **Water**, basic **Neutral** (mage เดิม, ไม่แปลง)
- Base stat (0★): **+5 INT, +10 MATK** (⚠️ปรับได้)
- star→level: สกิลสเกล grant Lv5@0★ → Lv10@5★ (Shatter passive = flat Lv1)

### 5.1 Passive — *Shatter*
| | |
|---|---|
| trigger | ศัตรู **freeze แตก** (take_hit, **แหล่งไหนก็ได้** — Glacial Storm/เพื่อน/Frostbind) + caster รู้สกิลนี้ |
| ผล | burst **5×5 (radius 2) Water/Magic** ที่ตัวศัตรูที่แตก (จุดกลาง) + รอบๆ, **enemies only** |
| power | `1000·L bp` MATK → Lv5 **50%** / Lv10 **100%** · 1 เลขจริง/ตัว |
| bonus | caster ได้ **Icicle +1** |
| plumbing | hook ที่ freeze-break ใน `take_hit` → queue `PendingShatter` (แบบ PendingExplosion) → resolve ใน tick step, credit เจ้าของ Shatter + fragment |

### 5.2 Active — *Frostbind* (แช่ trash)
| field | value |
|---|---|
| kind / max_level | Active / 10 · sp_reserve 2 |
| cooldown / cast_time / motion | 50t (5s) / 0 (flick, ติดทันที) / 5t |
| range / โพรเจกไทล์ | 6 / ranged bolt (proj_travel เหมือน bolt) |
| targeting | **ศัตรูใกล้สุดที่ยังไม่ frozen, priority non-boss** (reuse `find_nearest_enemy_without_buff(Freeze)` + non-boss pref) |
| power | `2000·L bp` MATK → Lv5 **100%** / Lv10 **200%** · Water/Magic |
| on-hit Freeze | chance `4000 + 800·lvl` → Lv5 **80%** / Lv10 **120%** (base >100% ชนะ resist ที่ max); dur 70t; reuse RO freeze roll (MDEF/LUK cut); boss immune |
| bonus | caster **Icicle +1** |

### 5.3 Active — *Fimbulwinter* (payoff, boss finisher · gated ที่ 5 fragment)
| field | value |
|---|---|
| kind | Active — **ยิงได้เฉพาะตอน `icicles == 5`** (cast-gate ใหม่) |
| on RESOLVE | consume 5 → 0 **เฉพาะตอน resolve** + มีศัตรูเป็นๆในระยะ ≥1; ยิง **ก้อนน้ำแข็ง 5 ก้อน = 5 real instance** |
| ต่อก้อน | burst **3×3 (radius 1) Water/Magic**, `power_bp 12000 + per_level 2000` → Lv5 **200%** / Lv10 **300%** |
| targeting | **priority BOSS** — แต่ละก้อน **re-acquire เป้าเป็นตอน resolve** → boss ถ้าอยู่ในระยะ; ไม่มี boss = **สุ่มศัตรูเป็นในระยะ (ซ้ำได้)** = 5 PRNG draws ลำดับ fixed → ไม่ waste ใส่ศพ |
| range / cd / cast_time | **9 (ไกล)** / ~0 (gate ด้วย stack) / สั้น ~10t (moment ท่าพุ่ง) · motion 5t |
| **REFUND / no-enemy** | cast เริ่มได้เมื่อ stack==5 + มีเป้า; **ถ้าตอน resolve ศัตรูตายเกลี้ยงหมดกระดาน → ไม่ consume (ค้าง 5 ไว้)**. ไม่มี partial-refund — retarget-at-resolve จัดการให้ (มีเป้าเหลือ ≥1 = 5 ก้อนหาเป้าได้หมด) |

### 5.4 Icicle (resource)
- counter ใหม่บน Ice Mage: `icicles: u8`, cap **5**, skip-if-zero → goldens safe
- **+1** จาก Shatter (freeze-break) · **+1** จาก Frostbind
- ครบ 5 → Fimbulwinter ยิงได้ (consume → 0). **reset ตอนเปลี่ยน band/room** (transient)

### 5.5 Loop
Frostbind แช่ trash → เพื่อน/AoE ตี freeze แตก → **Shatter** 5×5 + Fragment → ครบ 5 → **Fimbulwinter**
เท boss → วน. **role split:** freeze/shatter เคลียร์ trash, Barrage ทำลาย boss (ที่ freeze ไม่ติด)

### 5.6 Master table
| field | Shatter | Frostbind | Fimbulwinter |
|---|---|---|---|
| kind | Passive | Active | Active (gate frag==5) |
| max_level | flat (Lv1) | 10 | 10 |
| sp_reserve | 0 | 2 | 1–2 (TBD) |
| cast_time | — | 0 | ~10t (moment) |
| motion | — | 5t | 5t |
| cooldown | — | 50t (5s) | ~0 (gate) |
| range | — (ที่ศัตรูแตก) | 6 | 9 |
| radius | 5×5 (r2) | — (bolt) | 3×3 (r1) ×5 ก้อน |
| power | 1000·L (50→100%) | 2000·L (100→200%) | 12000+2000/lv (200→300%) ต่อก้อน |
| element / type | Water / Magic | Water / Magic | Water / Magic |
| targeting | ศัตรูที่แตก + AoE | nearest unfrozen, non-boss | **boss priority**, else สุ่ม (ซ้ำได้) |
| on-hit | Fragment +1 | Freeze 80→120% + Fragment +1 | consume 5 |
| instances | 1/ตัว | 1 bolt | **5 (real)** |

### 5.7 Build checklist
- **`icicles: u8`** counter บน Entity (cap 5, skip-if-zero, reset on band/room)
- **freeze-break hook** ใน `take_hit` → `PendingShatter` + fragment (credit Shatter owner; แบบ PendingExplosion)
- **Frostbind targeting:** nearest unfrozen non-boss (reuse `find_nearest_enemy_without_buff(Freeze)`)
- **Fimbulwinter:** cast-gate `fragments==5` ใน `pick_active_skill` + consume + 5-shard boss-priority/random (5 PRNG draws, fixed order) 3×3 bursts
- catalog + skill_display (3 ชื่อ) + `exemplar_def` + star_ranks + goldens (ไม่มี golden ครอบ mage-exemplar → byte-identical)

### 5.8 ⚠️ Open / defaults
- art ยังไม่มี
- Frostbind cast_time = 0 (instant flick) — โอเคไหม หรืออยากมี cast bar
- Fimbulwinter sp_reserve slots (1 หรือ 2) + cast_time 10t
- Frostbind freeze duration 70t (7s, base) — ตาม Glacial Storm

---

</details>

---

### 3.4 ตัวละครที่ 4 — Archer — **Elfie, the Windwhisper** (provisional; art ยังไม่มี)

| | |
|---|---|
| **บทบาท** | นักธนูเอลฟ์สายลม — DPS hit-and-run ว่องไว เอาตัวรอดสูง |
| **ธาตุ** | Wind — ลูกธนูลม (physical + element Wind, ได้เปรียบ vs Water) |
| **อาวุธ / basic** | bow, ranged Physical range 3 |
| **companion** | **Sylph** — นางฟ้าลมบินตาม (โชว์ในเรนเดอร์ + กลไก heal ③) |

**Character design (art brief — cute-sexy, Nikke/BD2 genki tone):**
- **Archetype / personality:** Tiny elf girl — cheerful, mischievous, bursting with energy; bright as the wind
- **Age / body:** Petite elf girl with a **small but curvy, shapely figure** (adult petite — **youthful face but must clearly read as grown**, absolutely avoid a loli look), pointed ears, nimble
- **Outfit:** A short, skin-revealing ranger outfit — mint-green / white / cream; **bare midriff**, a very short skirt / hot-pants + **over-the-knee stockings**, fingerless gloves, a light cape with leaf / feather accents, clothing catching the wind
- **Hair:** Short-to-medium, light (blonde / mint-green), twin-tails or tousled, wind-blown
- **Weapon:** A slender elven bow + quiver
- **Companion:** **Sylph** — a tiny glowing wind-fairy fluttering beside her (adorable)
- **Palette:** Mint-green / white / cream / gold / sky-blue
- **Key art / mood:** Mid-leap / drawing the bow, a bright grin, wind tossing her hair and clothes, Sylph at her side
- **Reference:** Nikke (cute units), Brown Dust 2, Blue Archive
- ⚠️ **rating:** cute + sexy must **read clearly as a petite adult** (grown proportions) — critical for Rated Teen

---

## 6. Exemplar Kit — **Elfie, the Windwhisper** (Archer · Wind) — FULL SPEC

> SUPERSEDED — the archer shipped as **Aveline** (Dark, **Striker** since 0.0.440); see §0.

> ดีไซน์ 2026-07-17. อัพเกรดผ่าน ★ เท่านั้น. **ไม่มี SP** (cooldown limiter). bp (10000=100%) / ticks (10t=1s).
> ธีม = **นักรบลม hit-and-run** (dodge / haste / self-heal / gather-lockdown). ทุกสกิล = กลไกใหม่ของเกม.

### 6.0 Meta + star→level
- id `elfie`, class **Archer** (physical ranged bow), element label **Wind**
- **basic:** ranged Physical — **element Wind** (ลูกธนูลม; ได้เปรียบ vs Water) *(⚠️ ถ้าอยากคง Neutral บอกได้)*
- Base stat (0★): **+5 DEX, +10 ATK** (⚠️ปรับได้)
- star→level: สกิลสเกล (Tailwind/Sylph/Cyclone Vortex) Lv5@0★ → Lv10@5★; **Windstep = flat Lv1**

### 6.1 Passive — *Windstep* (dodge + reposition)
| | |
|---|---|
| kind | Passive (internal cooldown) |
| trigger | มีการโจมตี **เล็ง Elfie** (melee/ranged/AoE) ขณะ off cooldown |
| ผล | **hit นั้น MISS** (negate การันตี ไม่ roll) + Elfie **ถอย 3 ช่องหนีจากผู้โจมตี** (clamp bound) |
| cooldown | **7s (70t)** · flat ทุก ★ |
| plumbing | hook ที่ resolve-attack-vs-Elfie → ถ้ารู้ Windstep + off cd → negate + move 3 cells + start cd (**ใหม่**) |

### 6.2 Active — *Tailwind* (ASPD steroid, self)
| field | value |
|---|---|
| kind / max_level | Active self-buff / 10 · sp_reserve 2 |
| cast_time / motion | 0 / 5t |
| duration / cooldown | **8s (80t) / 16s (160t) = 50% uptime** |
| buff | **+ASPD `AspdPct`** (มีอยู่แล้ว): `1200 + 200·lv` bp → Lv5 **20%** / Lv10 **30%** |
| หมายเหตุ | ASPD soft-cap อาจตัดตอนฐานสูง (ปกติ) |

### 6.3 Passive — *Sylph* (fairy reactive heal, self)
| | |
|---|---|
| kind | Passive · companion **Sylph** (render-only fairy) |
| trigger | HP Elfie **ตกผ่าน 50%** (edge) |
| ผล | heal **`2·lv+10`% ของ MaxHP** → Lv5 **20%** / Lv10 **30%** |
| cooldown | **15s (150t)** — ต้องรอ cd แม้ยังต่ำกว่า 50% |
| plumbing | HP-threshold reactive self-heal + cd (**ใหม่**); reuse heal application |

### 6.4 Active — *Cyclone Vortex* (signature · Black Hole style)
| field | value |
|---|---|
| kind | Active signature (ground field, reuse Glacial Storm/Vermilion machinery) |
| placement | **scan 5×5 ที่ครอบศัตรูมากสุดในระยะ 8** → วางตรงนั้น (tie-break: ใกล้ Elfie → cell id ต่ำ, deterministic) — **smart AoE placement = ใหม่** |
| cast | ~1s (10t) แล้ว **ฟิลด์รันเอง 5s (ไม่ channel** → Elfie ยิงต่อได้) · motion 5t · sp_reserve 2 · **cd 18s (180t)** |
| field | **5×5 (radius 2), duration 50t (5s)**, tick ทุก 0.5s = **10 ครั้ง** |
| DoT/tick | Wind AoE **`400·lv+1000` bp ATK** → Lv5 **30%** / Lv10 **50%** ต่อ tick (×10 = 300→500% รวม) · 1 เลขจริง/ตัว/tick |
| **slow pull** | ทุก tick **ดูดศัตรู(non-boss) เข้าจุดกลาง 1 ช่อง** (ค่อยๆรวมกอง แบบ Black Hole) |
| CC | **Root** ศัตรูในฟิลด์ (refresh ทุก tick; ตรึง ขยับเองไม่ได้ ยังตีได้); **boss immune pull+root** แต่กิน DoT |
| ธาตุ/type | Wind / Physical |

### 6.5 Master table
| field | Windstep | Tailwind | Sylph | Cyclone Vortex |
|---|---|---|---|---|
| kind | Passive | Active (self-buff) | Passive | Active (field) |
| max_level | flat (Lv1) | 10 | 10 | 10 |
| trigger/cast | โดนเล็งโจมตี | ร่ายเอง (cd) | HP<50% edge | ร่ายเอง (cd) |
| cooldown | 7s (70t) | 16s (160t) | 15s (150t) | 18s (180t) |
| duration | — | 8s (80t) | — | 5s (50t), 10 ticks |
| effect | miss + ถอย 3 ช่อง | +ASPD 20→30% | heal 20→30% MaxHP | slow-pull + Root + DoT 30→50%/tick |
| range/area | self | self | self | 5×5 @ densest, range 8 |
| element/type | — | — | — | Wind / Physical |
| new mechanic | on-hit negate+reposition | (AspdPct มีแล้ว) | HP-threshold heal | smart-place + pull + Root field |

### 6.6 Build checklist
- **Windstep:** on-incoming-attack hook vs Elfie → negate hit + reposition 3 cells (หนีจากผู้โจมตี, clamp) + internal cd 70t
- **Tailwind:** `AspdPct` cast_buff (มีแล้ว) — dur 80t / cd 160t
- **Sylph:** HP-cross-below-50% reactive heal (2·lv+10 % MaxHP) + cd 150t; companion sprite = render-only
- **Cyclone Vortex:** densest-5×5 placement scan + ground field (dur 50t, 10 hits) + **slow-pull 1 cell/tick toward center** + Root refresh (boss immune) + Wind DoT; reuse field machinery
- basic-element override → Wind (ถ้าเอา); catalog + skill_display (4 ชื่อ) + `exemplar_def(elfie)` + star_ranks
- goldens: ไม่มี golden ครอบ archer-exemplar → byte-identical (skip-if-default)

### 6.7 ⚠️ Open / defaults
- art ยังไม่มี · companion **Sylph** (render)
- basic = **Wind** (ปรับเป็น Neutral ได้)
- Windstep flat ทุก ★ (ไม่สเกล) — หรืออยากให้ cd ลด/ระยะเพิ่มตาม ★
- Cyclone Vortex cd 18s / cast 1s / DoT 30→50%/tick — จูนได้

### ชื่อสกิล
Passive *Windstep* · *Tailwind* · Passive *Sylph* · signature *Cyclone Vortex* — companion **Sylph**

---

### 3.5 ตัวละครที่ 5 — Thief — **Noire, Ballad of Twilight** (provisional; art ยังไม่มี)

| | |
|---|---|
| **บทบาท** | นักลอบเอลฟ์(?)สายมืด-แสง — melee dual-dagger DPS สลับธาตุ + hybrid ATK/MATK |
| **ธาตุ** | **Shadow + Holy (dual)** — มีดคู่ มืด/แสง; Shadow⇄Holy ตบกัน 150% → สลับ = ไม่โดนต้านเต็ม |
| **อาวุธ / basic** | dual dagger, melee (range 1), Physical — **element สลับ Shadow↔Holy ทุก hit** |

**Character design (art brief — sleek sexy assassin, Nikke/BD2 tone):**
- **Archetype / personality:** A mysterious, alluring, dangerous assassin — two selves in one (light / shadow); a voice soft as a ballad
- **Age / body:** Grown woman, tall and lithe, an **athletic curvy figure, full bust** — sleek and sultry
- **Outfit:** An **asymmetric light/shadow** assassin bodysuit — left half **white + gold (light)** / right half **black (shadow)**; a **high-cut leotard/bodysuit** with **deep cleavage**, **long stockings / knee-high boots (mismatched left vs right)**, a thigh garter holding daggers, sheer black panels, twilight-purple accents
- **Hair:** Long, **two-tone** (black + white/silver), or black with a light streak
- **Weapon:** Dual daggers — a light blade (white/glowing gold) + a shadow blade (black/purple)
- **Palette:** Black / white / gold / twilight-purple / crimson (accent)
- **Key art / mood:** Daggers crossed, **face split half-lit half-shadowed** (light/dark down the middle), an alluring, dangerous gaze
- **Reference:** Nikke (sexy units), Brown Dust 2, Azur Lane — hook = the **light/shadow asymmetry**
- ⚠️ **rating:** high-cut + cleavage leans Mature — keep under the Teen ceiling or revisit the rating (see the tone note on Skadi)

---

## 7. Exemplar Kit — **Noire, Ballad of Twilight** (Thief · Light/Dark) — FULL SPEC

> **SUPERSEDED — the Shadow thief shipped as `nyx` (Raplus Umbral Dancer) 0.0.426 (2026-09-18), designed with the user ("ทำ melee DPS ต่อ โจร มืด"): Thief · Shadow · dagger melee · doctrine Catalyst · thief stat block. The dual Light/Dark Noire spec below is parked.**
>
> | slot | skill | กติกาที่ใช้จริง |
> |---|---|---|
> | Passive | **Umbral Exploit** | basic = Shadow · ทุก packet ที่ตี **+10% ต่อสถานะผิดปกติ (นับชนิด) บนเป้า** (`debuff_amp_bp`, นับใน `apply_damage`) |
> | Skill 1 | **Veil Slash** | 60% ATK +5%/lv · cd 8 วิ · ติด **Darkness**: เป้าตีพลาด **20%** (บอส **10%**) 8 วิ (`BuffKind::Darkness`, stack เข้า `avoid_bp` แบบ Blind) |
> | Skill 2 | **Rend** | 50% ATK +5%/lv · cd 10 วิ · **Rended = Bleed** 75% ATK +7.5%/lv ตลอด 5 วิ (ร่ายเฉพาะเป้าที่ยังไม่ bleed) |
> | Ultimate | **Umbral Requiem** | Limit Break gauge (skill 12 / basic 4 / hit 3 / sec 2) · **ศัตรูทุกตัวบนสนาม** 100% ATK +7.5%/lv Shadow + Darkness + Rended (`zone_wide`, `try_cast_zone_wide`) |
>
> (0.0.427: ทุกสกิลดาเมจของทั้ง 5 kit ถูกหาร 2 — user "ลดเดเมจทุกสกิลอีก 50%" — ตัวเลขข้างบนคือหลังหาร) ดาว: Slash + Rend + Requiem Lv5→10 · board Catalyst k1 = Veil Slash, k2 = Rend, ult = Requiem · synergy: Aveline's Umbral Mark + Lapis Burn + her own two statuses = +40% on a fully-dressed target.

> ดีไซน์ 2026-07-17. อัพเกรดผ่าน ★. **ไม่มี SP**. bp / ticks. **5★ = 2× ของ 0★** (เลขที่ระบุ = 5★, 0★ = ครึ่ง).

### 7.0 Meta + star→level
- id `noire`, class **Thief** (melee dagger, Physical), element label **Shadow + Holy (dual)**
- **Base stat (0★): +5 STR, +5 INT** — STR→ATK (ดาบ), INT→MATK (ปลดล็อกส่วน magic ของ Activate II hybrid) *(⚠️ปรับได้)*
- star→level: สกิลสเกล Lv5@0★ → Lv10@5★; passive (①, ②) = flat Lv1

### 7.1 Passive ① — *Duality* (สลับธาตุ)
| | |
|---|---|
| ผล | basic attack **สลับ element Shadow ↔ Holy ทุก hit** (physical dagger, melee) |
| plumbing | per-entity element toggle flip ต่อ basic (**ใหม่**; Ignite Break แค่ set ไม่สลับ) · determinism ปลอดภัย |
| combo | ดาบมืดกิน enemy Holy 150% / ดาบแสงกิน enemy Shadow 150% → ไม่โดนต้านเต็ม + feed holy-vuln (Lucielle/Seraphine) |

### 7.2 Active — *Eclipse Cross* (เปิด vuln 2 ธาตุ)
| field | value (0★ → 5★) |
|---|---|
| kind / sp_reserve / cd | Active / 2 / 8s (80t) · cast 0, motion 5t, range 1 |
| damage | **2 hit** (hit1 Shadow, hit2 Holy), `1000·lv bp` ATK/hit → **50% → 100% ATK/hit** |
| apply vuln | **Light-vuln + Shadow-vuln** (ทั้งคู่) บนเป้า: `200·lv bp` → **+10% → +20%** แต่ละธาตุ, duration 8s (80t) |

### 7.3 Passive ② — *Balance* (เก็บ stack)
| | |
|---|---|
| trigger | basic hit ที่ธาตุ (Light/Shadow) **ชนะทาง** = web advantage **หรือ** เป้ามี vuln ตรงธาตุนั้น |
| ผล | **+1 stack ตามธาตุที่ตี** → `light_stacks` / `shadow_stacks` (cap **10** ต่อราง) |
| plumbing | 2 counter บน Entity (u8, cap 10, skip-if-zero) + เช็ค advantage/vuln ตอน hit (**ใหม่**) |

### 7.4 Activate II — *Eclipse Nova* (dual spender, gated 10)
| field | value (0★ → 5★) |
|---|---|
| gate | ยิงเมื่อ `light_stacks==10` **หรือ** `shadow_stacks==10` (แบบ Skadi Fimbulwinter) → consume 10 รางนั้น |
| area / target | **7×7 (radius 3), priority BOSS** (center boss ถ้ามี, else densest/nearest), range 9 |
| CC | **Blind 5s (50t)** ศัตรูในพื้นที่ |
| **Dark** (10 shadow) | Shadow, **2 instance**: `3000·lv` ATK + `1500·lv` MATK → **300% ATK + 150% MATK** |
| **Light** (10 light) | Holy, **2 instance**: `1500·lv` ATK + `3000·lv` MATK → **150% ATK + 300% MATK** |
| flavor | Dark = สาย ATK (ดาบเก็บเกี่ยว) · Light = สาย MATK (แสงชำระ) — hybrid (ต้องมี INT จาก base grant) |

### 7.5 Loop
Eclipse Cross เปิด **Light+Shadow vuln** → ① สลับตี, ② hit ที่ตรง vuln/advantage เก็บ stack 2 ราง → ครบ 10 → **Eclipse Nova** 7×7 (มืด ATK / แสง MATK) + Blind → วน. **duality self-feeding** 🌗

### 7.6 Build checklist
- **`shadow_vuln`** BuffKind (คู่ `holy_vuln`, apply ใน apply_damage เมื่อ element==Shadow)
- **① element toggle** ต่อ basic (Shadow↔Holy)
- **Eclipse Cross:** 2-hit (Shadow+Holy) + apply holy_vuln + shadow_vuln
- **② stack gen:** 2 counter (light/shadow, cap 10) + gain เมื่อ hit element ชนะทาง (web adv OR matching vuln)
- **Eclipse Nova:** gate stack==10 ใน pick_active_skill + consume 10 + 7×7 boss-priority + Blind 50t + **hybrid 2-instance (ATK + MATK)**, 2 variant (Shadow/Holy)
- catalog + skill_display (4 ชื่อ) + `exemplar_def` + star_ranks + goldens (ไม่มี golden ครอบ thief-exemplar → byte-identical)

### 7.7 ⚠️ Open / defaults
- art ยังไม่มี
- Eclipse Cross vuln duration 8s / cd 8s — จูนได้
- Eclipse Nova: gated-auto (แบบ Skadi) · ถ้าเป้าตายหมดตอน resolve → hold stack (แบบ Fimbulwinter refund)
- MATK ของ thief ต่ำ → base grant +INT ช่วย; อยากเพิ่ม INT ต่อ ★ ไหม

### ชื่อสกิล
① *Duality* · *Eclipse Cross* · ② *Balance* · *Eclipse Nova* (Dark/Light)

---

---

## 8. Exemplar Kit — **Seraphine** (Swordsman · Physical) — FULL SPEC

> ออกแบบ 2026-09-07 · **ยังไม่ build** · เขียนบน **ระบบใหม่** — อ่าน `skill-system-redesign.md`
> (ชุด Basic/Skill×2/Ultimate+Passive · ไม่มี SP · ไม่มี skill point · Potential Board) และ
> `element-weakness-system.md` (weakness list · หลอด Toughness · Disorder) ก่อน
>
> ⚠️ **ตัวนี้แทน Seraphine เดิมที่จองไว้เป็น Holy** — ธาตุเปลี่ยนเป็น **Physical** เพราะ Holy ชนกับ
> Lucielle และภายใต้ชาร์ตสะสม การมี Exemplar สองตัวธาตุเดียวกันทำลาย pitch การเก็บให้ครบ
> art brief §3.1 ต้องแก้: ผมบลอนด์ทอง → **เงิน/ฟ้า** (ตาม key art ที่มีแล้ว), อาวุธ = **ดาบสองมือ**

### 8.0 Meta
- **ชื่อสกิลใช้ภาษาอังกฤษทั้งชุด** (user 2026-09-07 — เฉพาะตัวนี้ ไม่ลามไปตัวอื่น;
  Lucielle คงชุดละตินไว้ตามเดิม): *Iron Chains · Steel Break · Aegis · Bulwark*
- ธาตุ **Physical** · **Doctrine: Sentinel** · อาวุธ **ดาบสองมือ**
- อาวุธ: **ดาบสองมือ** เท่านั้น — ไม่มีโล่ ไม่มีหอก (ตีช้า ตีหนัก ตาม `twohand_sword_mastery`)
- ⚠️ Base stat grant (0★) — ยังไม่เคาะ (มิเรอร์ตัวอื่น: +5 STR / +10 ATK)
- ⚠️ **Memory Fragment (★)** แก้สกิลยังไง — ยังไม่เคาะ (ระบบเลเวลสกิลย้ายไปอยู่ที่ Potential Board แล้ว
  ★ จึงควรเป็นการ *เปลี่ยนพฤติกรรม* แบบ Eidolon ไม่ใช่ +เลเวล)

### 8.1 Basic — ฟันสองมือ
ระยะ 1 · Physical · ตีช้า หนัก · **ทำ Toughness damage ตาม baseline** (ทุก damage instance กินอยู่แล้ว)

### 8.2 Skill-1 — *Iron Chains*
| field | value |
|---|---|
| ผล | **ดึงศัตรูทุกตัวใน 7×7 (r3) รอบตัวเข้ามาติดตัว** + **Taunt** + **DEF ตัวเองขึ้น** |
| taunt | 60t (6s) |
| self buff | `DefPct` **+(20 + 3·L)%** · 100t (10s) |
| cooldown / motion | ⚠️ 120t (12s) / 5t |
| ดาเมจ | **มี** — ลงศัตรูทุกตัวที่ถูกดึง ⚠️ `10000 bp + 1000/lvl` (Lv5 150% / Lv10 200% ATK) · Physical · ทำ Toughness damage ตาม baseline |
| reuse | `knockback` **ค่าติดลบ** (ดึงแทนผลัก) · `taunt` · `DefPct` |

> ⚠️ **ต้องเขียนความต่างจาก `Cyclone Vortex` ของ Elfie ลงสเปกทั้งสองฝั่ง** ไม่งั้นกลายเป็นท่าซ้ำ:
> **Elfie = ดูดไปรวมที่จุดไกล ๆ (จัดตำแหน่งให้ AoE ลง)** · **Seraphine = ลากมาที่ตัวแล้วล็อกไว้ (รับหน้าแทนทีม)**

### 8.3 Skill-2 — *Steel Break*
| field | value |
|---|---|
| ผล | **AoE 3×3 (r1) รอบตัว** Physical |
| power | ⚠️ `16000 bp + 1600/lvl` (Lv5 240% / Lv10 320% ATK) — ตั้งต้นจาก Ignite Break |
| **โบนัส** | **ทำ Toughness damage เพิ่ม** (`toughness_dmg_mult` ⚠️ ×3) ← ท่าหลักที่ทุบหลอดให้ทีม |
| cooldown / motion | ⚠️ 50t (5s) / 5t |
| reuse | `self_aoe_radius` (Ignite Break ใช้ทรงนี้อยู่) + ฟิลด์ใหม่ `toughness_dmg_mult` |

**บทบาทเชิงระบบ:** เธอคือ **เครื่องทุบ Toughness ของปาร์ตี้** — รวบศัตรูมากองด้วย Skill-1 แล้ว
Steel Break ทุบหลอดหลายตัวพร้อมกัน → แตกเป็นแถบ → ทั้งแถบ stun และรับดาเมจเพิ่ม +20% พร้อมกัน
และทุกครั้งที่แตกคือ event ที่ **Break Trigger** ของเพื่อนอ่านได้ (Lapis ยิงตาม)
นี่คือเหตุผลที่เธอทำให้ทั้งทีมแรงขึ้น ไม่ใช่แค่ตัวเองแกร่ง

### 8.4 Ultimate — *Aegis*
| field | value |
|---|---|
| ผล | **ทุกคนในปาร์ตี้ได้โล่ดูดซับ** พร้อมกัน |
| pool ต่อคน | **MaxHP ของ Seraphine × (15 + 2·L)%** (ตามแบบ Praesidium Divinum ที่ใช้ MaxHP ผู้ร่าย) |
| hit cap | `hits_left = 0` — pool ล้วน ไม่จำกัดจำนวนหมัด (แบบ Flame Shield) |
| **แตกแล้วระเบิด** | `burst_dmg` ⚠️ 200% ATK Physical 3×3 ที่ตัวคนนั้น **+ ทำ Toughness damage เพิ่ม** |
| duration | ⚠️ 150t (15s) |
| reuse | **ไม่มีกลไกใหม่เลย** — `Entity.shields` / `AbsorbShield` / `combat::absorb_soak` / `resolve_absorb_bursts` / hit event `Guard` มีครบแล้ว |

**ทำไมต้องมี burst:** อัลติสายตั้งรับในเกม idle มีปัญหาว่า *ผู้เล่นไม่เห็นดาเมจที่ไม่เกิดขึ้น* —
การให้โล่ระเบิดตอนแตกทำให้มันมีผลลัพธ์บนจอ และป้อน Toughness กลับเข้าลูป (Flame Shield ใช้ทรงนี้อยู่แล้ว)

**cut-in:** ใช้ท่าใน key art ได้ตรง ๆ — เกราะยักษ์ที่เธอนั่งอยู่ลุกขึ้นกางป้องทั้งปาร์ตี้
(เป็น **VFX เท่านั้น ไม่ใช่ summon** ไม่มีกลไกเพิ่ม)

### 8.5 Passive — *Bulwark*
**DEF เพิ่มตามจำนวนศัตรูที่ถูก taunt อยู่** ⚠️ `+2·L` ต่อศัตรู 1 ตัว (cap ⚠️) — ยิ่งรวบเยอะยิ่งแกร่ง
ทรงเดียวกับ `Endure` ที่มีอยู่ (นับศัตรูรอบตัว)

### 8.6 Limit Break — ชาร์จจาก **การรับดาเมจ**
ลูปปิดพอดี: **ดึงเข้ามา → โดนรุม → เกจเต็มเร็ว → เปิดโล่ให้ทีม → ดึงรอบใหม่**
⚠️ น้ำหนักตั้งต้น: รับดาเมจ = แหล่งหลัก · ร่ายสกิล = เล็กน้อย · ตีโดน = เล็กน้อย · เติมตามเวลาเป็นพื้น

### 8.7 Potential Board — สามกิ่ง ตาม `skill-system-redesign.md` §5.3

major trace = passive ที่ปลายกิ่ง ปลดที่ Job tier 2 / 4 / 6 · ไม่มีเลเวล · กติกาอยู่ที่ §5.5

| major trace | ผล |
|---|---|
| **A — Iron Edge** | ATK **+25% ของ DEF ปัจจุบัน** คิดใหม่ทุกวินาที (รูปเดียวกับ Gepard A6) |
| **B — Iron Wall** | ทั้งปาร์ตี้รับดาเมจ **-10%** |
| **C — Second Wall** | โล่จาก *Aegis* ที่ **แตก** คืนเกจ Limit Break ให้ Seraphine (รูปเดียวกับ Gepard A4) |

> B เคยเขียนว่า "ศัตรูที่ถูก taunt ทำดาเมจใส่คนอื่นได้น้อยลง" ซึ่งเป็นเงื่อนไขที่ไม่มีผล —
> ศัตรูที่ถูก taunt ตีคนอื่นไม่ได้อยู่แล้ว (user 2026-09-07) เหลือเป็นการลดดาเมจตรง ๆ

**สามคำถามคนละข้อ** A ถามว่าจะแปลง DEF ที่กองไว้เป็นดาเมจไหม · B ถามว่าจะเอาไปลดดาเมจให้ทีมไหม ·
C ถามว่าจะเอาอัลติมาถี่ขึ้นโดยยอมให้โล่แตกไหม — C เป็นข้อเดียวที่ทำให้ *การเสียโล่* เป็นเรื่องดี
ซึ่งกลับทิศกับสัญชาตญาณของสายตั้งรับ

**A กับ B ใช้ DEF ตัวเดียวกันคนละทาง** ซึ่งทำให้ของสวมใส่ชิ้นเดียวกันตอบได้ทั้งสองกิ่ง
ไม่ต้อง build แยก แต่ผลบนสนามต่างกันชัด

⚠️ ค่าตัวเลขของทั้งสามยังไม่เคาะ · C ต้องระวังไม่ให้กลายเป็นลูปที่เปิดโล่เพื่อให้แตกทันที
(เพดานที่ง่ายที่สุดคือคืนเกจได้ครั้งเดียวต่อการร่ายหนึ่งครั้ง ไม่ใช่ต่อคนในปาร์ตี้)

**ย้ายไปเป็น Memory Fragment (★)** — ปิดข้อค้างที่ §8.9 ไปหนึ่งข้อ
ทั้งสองอันเปลี่ยน *รูปร่าง* ของท่า ซึ่งเป็นงานของ ★ ไม่ใช่ trace (`skill-system-redesign.md` §5.5)

| ★ | ผล |
|---|---|
| **Chain Rend** | *Iron Chains* ทำ Toughness damage ด้วย และ *Steel Break* ขยายเป็น 5×5 |
| **Wider Chains** | *Iron Chains* ขยายจาก 7×7 เป็น 9×9 |

### 8.8 Build checklist
- ฟิลด์ใหม่ `SkillDef.toughness_dmg_mult` (โบนัส Toughness damage)
- `knockback` รองรับค่าติดลบ = ดึงเข้า (ตรวจว่าเดินชนกำแพง/ขอบแมพแล้วไม่หลุดกริด)
- Ultimate: แจก `AbsorbShield` ยกปาร์ตี้ (ต่อยอด `try_cast_praesidium` ที่แจกทีละคน)
- Passive นับศัตรูที่ taunt อยู่ (ต่อยอด `apply_taunt` / `apply_endure`)
- catalog + `skill_display` 4 ชื่อ + `exemplar_def` + loc keys
- ⚠️ art brief §3.1 แก้: ผมเงิน/ฟ้า · ดาบสองมือ · ธาตุ Physical

### 8.9 ⚠️ ค้างเคาะ
- ตัวเลขทุกตัวที่ทำเครื่องหมาย ⚠️ (cooldown / power / duration / pool%)
- Memory Fragment (★) ของเธอทำอะไร
- Base stat grant 0★
- ⚠️ **ผลข้างเคียงเชิงบาลานซ์:** ดึง 7×7 + taunt = ศัตรูทั้งห้องกองที่เธอคนเดียว →
  ปาร์ตี้เก่งขึ้นมากกับ AoE แต่ถ้าเธอตาย ทีมเปิดโล่ง · ต้องดูคู่กับ AoE ของศัตรูด้วย

---

## 9. Exemplar Kit — **Lapis** (Mage · Fire) — FULL SPEC

> 0.0.441: Lapis is a gacha PULL like every unit (`ExemplarLine` + `exemplar_rapis`, name "Lapis Emberheart" provisional); 0.0.437: Fireball 600% +60%/lv cd 6 s, Ember 160%, Meteor 750% — see §0.

> ออกแบบ 2026-09-07 · ยังไม่ build · เขียนบนระบบใหม่ อ่าน `skill-system-redesign.md` และ
> `element-weakness-system.md` ก่อน · แทนที่ Skadi ที่ปลดออกจาก roster

### 9.0 Meta
- ธาตุ **Fire** · **Doctrine: Striker** แท็ก **AoE** · อาวุธสายเวท
- ⚠️ บทบาทในปาร์ตี้ บุคลิก และหน้าตา ยังไม่เคาะ ยังไม่มี art
- ⚠️ Base stat grant 0★ และผลของ Memory Fragment ยังไม่เคาะ

### 9.1 Basic
โจมตีเวทระยะไกลตามคลาส Mage ทำ Toughness damage ตามอัตราพื้นฐาน

### 9.2 Skill-1 — Fireball
AoE 3×3 ที่จุดตกกระทบ ธาตุ Fire ใช้กลไกเดิมของ `fireball` ที่มีอยู่แล้ว

| field | value |
|---|---|
| radius | 3×3 target-centred |
| element | Fire |
| power / cooldown / range | ⚠️ ตั้งต้นจากค่าเดิมของ `fireball` |

### 9.3 Skill-2 — Flame Shield
โล่ดูดซับใส่ตัวเอง เมื่อโล่แตกจะระเบิดเป็นดาเมจ Fire รอบตัว ใช้สกิล `flame_shield` เดิมทั้งดุ้น

### 9.4 Passive — Fireball on Toughness Break
เมื่อศัตรูตัวใดก็ตาม **ถูกทุบ Toughness จนแตกจากแหล่งใดก็ได้** Lapis ยิง Fireball ใส่ศัตรูตัวนั้นทันที

| field | value |
|---|---|
| trigger | Toughness break ของศัตรูตัวใดก็ได้ ไม่จำกัดว่าใครเป็นคนทุบ |
| effect | Fireball ที่เป้าตัวนั้น เครดิตดาเมจเป็นของ Lapis |
| power | ⚠️ ต่ำกว่า Skill-1 |
| plumbing | hook ที่จุด Toughness break คิว pending effect แล้ว resolve ใน tick step ทรงเดียวกับ `PendingExplosion` |

**ลูกโซ่เป็นฟีเจอร์ ไม่ใช่บั๊ก** user 2026-09-07 Fireball จาก passive **ทำ Toughness damage ได้ตามปกติ**
และทุบตัวอื่นให้แตกต่อเป็นลูกโซ่ได้ ลูกโซ่จำกัดตัวเองอยู่แล้วเพราะหลอด Toughness ฟื้นกลับหลังแตก
ลูกถัดไปต้องกัดหลอดใหม่ทั้งหลอดถึงจะทุบตัวต่อไปได้ ศัตรูหลอดยาวต่อยาก ฝูง trash หลอดสั้นต่อได้ยาว

**คูลดาวน์และคิว** user 2026-09-07

| field | value |
|---|---|
| cooldown ระหว่างลูก | **2t** |
| ทริกเกอร์ที่มาระหว่างคูลดาวน์ | **เข้าคิวไว้** ไม่ทิ้ง |
| ลำดับ | FIFO |
| เป้าตายก่อนถึงคิว | **ข้าม** แล้วหยิบตัวถัดไปในคิวมายิงในเทิร์นเดียวกัน ยิงได้ไม่เกิน 1 ลูกต่อโอกาส |
| คิวว่าง | คูลดาวน์ไม่เดิน |
| ความยาวคิวสูงสุด | **16** ทริกเกอร์ที่มาตอนคิวเต็มถูกทิ้ง |

คูลดาวน์ 2t ทำหน้าที่เป็นตัวกันสิมวนไม่จบไปในตัว ไม่ต้องมีเพดานความลึกต่อเทิร์นแยกอีก
และการทุบ Toughness หลายตัวพร้อมกันจะกลายเป็นสายไฟกระจายทีละลูกแทนที่จะระเบิดพร้อมกันหมด

⚠️ **คิวต้องเก็บ entity id ไม่ใช่ index** เป็นกับดักที่เคยทำพังมาแล้วในโปรเจกต์นี้ index ที่ถือข้ามเทิร์น
จะชี้ผิดตัวทันทีที่รายชื่อ entity หด

**ผลเชิงระบบ** พลังของเธอขึ้นกับความสามารถของทั้งทีมในการทุบ Toughness ไม่ใช่ดาเมจของตัวเองอย่างเดียว
จับคู่กับ Seraphine ที่ทุบ Toughness ทีละหลายตัวด้วย Steel Break ได้โดยตรง

### 9.5 Ultimate — Fire Cat Meteor
ร่ายเวทมนตร์พร้อม cast bar ระหว่างร่ายเล่น cut-in เมื่อร่ายจบศัตรูทุกตัวในห้องรับดาเมจ Fire
**หนึ่ง instance พร้อมกัน** ไม่ใช่อุกกาบาตตกทีละลูก

| field | value |
|---|---|
| trigger | Limit Break เต็ม |
| targeting | **ศัตรูทุกตัวในห้อง** ไม่อิงรัศมี วนตามลำดับ spawn |
| instance | 1 ต่อศัตรู 1 ตัว |
| cast time | ⚠️ มี cast bar ใช้ `effective_cast_time` |
| resistance damage | ⚠️ โบนัส ทั้งจอ |
| cut-in | ภาพซ้อนบนไคลเอนต์ **ไม่หยุดสิม** ดาเมจลงตามเทิร์นที่ร่ายจบ |

⚠️ `meteor_rain` เป็น skill id ของ Elementalist อยู่แล้วและเป็นคนละกลไก ตัวของ Lapis ต้องใช้ id แยก

### 9.6 Limit Break

หลอดเติมจาก **Toughness break** เป็นหลัก ทุกครั้งที่ศัตรูตัวใดถูกทุบหลอดแตกจากแหล่งใดก็ได้
Lapis ได้เกจ แหล่งเดียวกับที่ทริกเกอร์ passive ของเธอ

| แหล่ง | น้ำหนัก |
|---|---|
| Toughness break ของศัตรูตัวใดก็ได้ | หลัก |
| ดาเมจที่ทำได้ | เล็กน้อย |
| ตามเวลา | พื้นฐาน |

ผลคือ Fire Cat Meteor มาเร็วเมื่อทีมทุบหลอดได้ถี่ และมาช้าลงในฉากบอสตัวเดียวที่หลอดยาว
ต่างจาก Seraphine ที่ชาร์จจากดาเมจที่รับเข้ามา ทั้งคู่จึงไม่แย่งเงื่อนไขกัน

### 9.7 Potential Board — สามกิ่ง ตาม `skill-system-redesign.md` §5.3

major trace = passive ที่ปลายกิ่ง ปลดที่ Job tier 2 / 4 / 6 · ไม่มีเลเวล · กติกาอยู่ที่ §5.5

| major trace | ผล |
|---|---|
| **A — Cinder Rush** | ฆ่าเป้าได้ **Fireball ทำดาเมจ +20%** ซ้อน 3 ชั้น (รูปเดียวกับ Seele A2) |
| **B — Cinderhide** | ระหว่างที่ *Flame Shield* ยังไม่แตก **ดาเมจ Fire +10%** (รูปเดียวกับ Seele A4) |
| **C — Kindling** | ศัตรูถูกทุบ Toughness แตก ได้เกจ Limit Break **+20%** |

**สามคำถามคนละข้อ** A ถามว่าจะเก่งขึ้นตอนเจอฝูงที่ตายง่ายไหม (ฉากบอสตัวเดียว A เป็นศูนย์) ·
B ถามว่าจะรักษาโล่ให้อยู่ตลอดไหม · C ถามว่าจะเอาอัลติมาถี่ขึ้นไหม ซึ่งทำงานทั้งกับฝูงและบอส
เพราะมันเกาะกับ Toughness break ไม่ใช่กับการฆ่า

⚠️ ค่าตัวเลขยังไม่เคาะ

**ย้ายไปเป็น Memory Fragment (★)** — ปิดข้อค้างที่ §9.0 ไปหนึ่งข้อ

| ★ | ผล |
|---|---|
| **Ember Split** | Fireball จาก passive ที่ฆ่าเป้าได้ ยิงต่อทันทีใส่เป้าถัดไปในคิว ไม่รอคูลดาวน์ 2t ⚠️ ต้องคงเพดาน 1 ลูกต่อโอกาสไว้ในทางอื่น ไม่งั้นการฆ่าต่อเนื่องระบายทั้งคิวใน tick เดียว |
| **Quickfire** | คูลดาวน์ระหว่างลูกของ passive 2t → 1t และขยายคิวเป็น 32 |
| **Emberfield** | Fireball ทิ้งพื้นไฟไว้ที่จุดตกกระทบ ใช้กลไก ground field เดิมของ `vermilion_field` |

### 9.7.1 ผลวัดจริง (2026-09-07) — คู่ Seraphine + Lapis ทำงานเกินกว่าการบวกกัน

prototype บนเอนจินปัจจุบัน seed 42 · Lv40 ใส่เกียร์ · 6000 tick · หลอด Toughness 60 · chip 20/40

| ปาร์ตี้ | kills | breaks | ไปถึง |
|---|---|---|---|
| **Seraphine + Lapis** | 569 | **107** | b14-2 |
| Seraphine + mage | 549 | 73 | b13-3 |
| swordsman + Lapis | 554 | 37 | b13-3 |
| swordsman + mage (ฐาน) | 540 | 21 | b13-2 |

- Lapis คนเดียวเพิ่ม break จาก 21 เป็น 37
- Seraphine คนเดียวเพิ่มเป็น 73
- **อยู่ด้วยกันได้ 107 ทั้งที่ถ้าบวกกันตรง ๆ ควรได้ราว 89**

ส่วนเกินคือลูปที่ออกแบบไว้ทำงานจริง Seraphine ทุบหลอดแตกถี่ขึ้น → Lapis ยิงตอบบ่อยขึ้น →
Fireball ของเธอไปทุบตัวถัดไปแตกเพิ่มอีก **คู่นี้แรงกว่าผลรวมของตัวเอง ซึ่งเป็นเหตุผลเชิงกลไก
ที่ทำให้ผู้เล่นอยากได้ทั้งคู่** ไม่ใช่แค่คำโฆษณา

### 9.8 Engine requirements
ของที่ต้องเขียนใหม่
- targeting แบบศัตรูทุกตัวในห้อง
- hook ที่จุด Toughness break สำหรับ passive
- คิวเป้าหมายของ passive บน `CharacterData` เก็บเป็น entity id พร้อมคูลดาวน์ 2t และ serialize ลงเซฟ
- hook ยิงสัญญาณ cut-in ตอนเริ่มร่าย

ของที่มีอยู่แล้ว
- `fireball`
- ground field ของ `vermilion_field` สำหรับกิ่ง B
- `flame_shield` และ absorb engine
- `effective_cast_time`
- pending effect queue ทรง `PendingExplosion`
- ระบบ cut-in ฝั่ง Godot และ Unity

---

## 10. Exemplar Kit — **Astier, Tidewarden** (Support · Water · SR) — FULL SPEC

> ดีไซน์ 2026-09-08. SR ตัวแรกของสาย Support หลังเลิกใช้ class — ตัวนี้เป็น **unit** ไม่ใช่
> overlay บนคลาส ยึด Doctrine **Support** ตาม `docs/skill-system-redesign.md` §4.4
> bp (10000=100%) / ticks (10t=1s) ไม่มี SP คูลดาวน์เป็นตัวจำกัดอย่างเดียว

### 10.0 ทำไมต้องมี ทั้งที่มี Lucielle อยู่แล้ว

สองแกนของการจัดทีมคือ **ธาตุ × Doctrine** (§4.4.2) Lucielle คือ Support × **Holy** ช่องเดียวเท่านั้น
แปลว่าปาร์ตี้ที่ต้องการคนฮีลต้องพา Holy มาด้วยเสมอ ทั้งที่ระบบ break ผูกกับ **weakness list**
ของมอนแล้ว (มอนละ 1–3 ธาตุ) — คนฮีลที่ไม่ตรงธาตุจึงเท่ากับสละช่องธาตุไปหนึ่งช่อง

Astier คือ Support × **Water** ช่องที่สอง ผู้เล่นได้เลือกว่าจะเอาคนฮีลแบบไหนมาเติมธาตุที่ทีมขาด
และเป็นเหตุผลให้เก็บตัวละครสองทางตามที่ §4.4.2 ตั้งไว้

### 10.1 Meta

| | |
|---|---|
| id | `astier` |
| ชื่อ | Astier, Tidewarden (**แอสเทียร์**) |
| rarity | **SR** (2 สกิล ตาม `EXEMPLAR_LADDER[0]`: crit_dmg 500 / flat 25 / pct 500) |
| Doctrine | **Support** (โปรไฟล์ HP กลาง · ATK ต่ำ · DEF กลาง) |
| element | **Water** |
| tag | AoE |
| star→level | สกิลที่สเกล Lv5@0★ → Lv10@5★ · passive flat Lv1 |

### 10.2 ธีม — คนฮีลตัวเริ่มต้น ที่ยิงน้ำได้ด้วย

**เคาะ 2026-09-08** เอาแบบง่ายก่อน เพราะเธอเป็นตัวที่ผู้เล่นเจอตั้งแต่ต้น ไม่ใช่ตัวที่ต้องเรียนรู้

ต่างจาก Lucielle ตรงที่ Lucielle เป็นสายบัฟ+ลูกโซ่ holy ส่วน Astier คือ **คนฮีลตรง ๆ**
ที่มีปุ่มดาเมจ Water ให้ทีมใช้ตอนที่ห้องนั้นแพ้ธาตุน้ำ — Support ที่ทุบหลอด break ไม่ได้เลย
คือ Support ที่ทีมจ่ายค่าช่องแพงเกินไป

### 10.3 Passive — *Unda Vitae*

| | |
|---|---|
| kind | Passive · flat Lv1 ทุก ★ |
| basic override | ranged (range 3) **Water MAGIC** 50% MATK — รูปเดียวกับ `SAGITTA_LUCIS` |
| เหตุผล | ตัวที่ยืนบน stat block ของ acolyte แทบไม่มี ATK ถ้าไม่มีอันนี้เธอจะเดินเข้าไปจิ้มระยะประชิด |

### 10.4 Skill 1 — *Rivus* (ฮีลเด้ง 3 จังหวะ)

| field | value |
|---|---|
| kind / max_level | Active (heal) / 10 · loadout_slots 2 |
| cast / motion / cd | 10t (1s) / 5t / **80t (8s)** |
| เป้า | ally ที่ **% HP ต่ำสุด** แล้วเด้งต่ออีก 2 ตัวที่ต่ำรองลงมา (map-wide) |
| จำนวน | **100% / 70% / 50%** ของก้อนแรก (`RIVUS_BOUNCE_PCT`) |
| ก้อนแรก | สูตร renewal heal ที่ `power_bp: 6000` แบบเดียวกับ `heal` / `dupla_medicina` |
| ทำไม % ไม่ใช่ HP ที่หาย | ถ้าเรียงตาม HP ที่หาย แทงค์จะกินทุกเด้ง ส่วนตัวคาสต์ 200 HP ที่เหลือเสี้ยวเดียวไม่ได้อะไร |
| ไม่มี threshold | ต่างจาก `dupla_medicina` ที่รอให้มีคนต่ำกว่า 75% — ในเกม idle ไม่มีใครนั่งดูจังหวะ ฮีลที่รอ threshold คือฮีลที่นอนรอ cooldown ทิ้ง ตัวหารคือ decay ไม่ใช่ประตู |

### 10.5 Skill 2 — *Vortex Undae* (AoE Water 5×5)

| field | value |
|---|---|
| kind / max_level | Active (AoE) / 10 · loadout_slots 2 |
| cast / motion / cd | 15t (1.5s) / 5t / **100t (10s)** |
| area / range | 5×5 รอบเป้าที่โดน (`aoe_radius: 2`) · range 6 |
| damage | Water **MAGIC** `7200 + 1800·(lv−1)` bp MATK → Lv5 **126%** / Lv10 **216%** |

### 10.6 Ultimate — *Fons Vitae* (ฮีลหมู่)

| field | value |
|---|---|
| kind | Active · **gauge-gated** (`lb_max: 100`) ไม่มี cooldown |
| เติมเกจ | basic 4 · skill 10 · 2 ต่อวินาที (ทรงเดียวกับ Lapis) |
| ผล | ally ที่ยังไม่ตาย **ทุกตัว** ฟื้น `2000 + 200·(lv−1)` bp ของ **MaxHP ตัวเอง** → Lv1 20% / Lv5 28% / Lv10 38% |
| ทำไมเป็น % ของตัวเอง | เลขก้อนเดียวจะอ่านได้เท่ากันทั้งบนตัวคาสต์ 200 HP และแทงค์ 2000 HP |
| กติกาการถือ | ถ้าปาร์ตี้เต็มเลือดหมด **ไม่ร่าย** — เกจที่เทลงตอนเลือดเต็มคือเกจที่ทิ้ง และไม่มีใครนั่งถือให้ |

### 10.6.1 Master table

| field | Unda Vitae | Rivus | Vortex Undae | Fons Vitae |
|---|---|---|---|---|
| kind | Passive | Active (heal) | Active (AoE) | Ultimate |
| max_level | flat Lv1 | 10 | 10 | 10 |
| gate | — | cd 80t | cd 100t | เกจเต็ม + มีคนเจ็บ |
| ผล | basic = Water bolt 50% MATK | ฮีล 3 เด้ง 100/70/50% | 126→216% MATK 5×5 | ทั้งปาร์ตี้ 20→38% MaxHP |
| new mechanic | — | bounce heal | (aoe_radius มีแล้ว) | ulti heal ฝั่ง party |

### 10.6.2 Build checklist

- ✅ `SkillDef` สี่อันใน `skills/catalog/exemplar.rs` + `skill_display` สี่ชื่อ
- ✅ `try_cast_rivus` / `try_cast_fons_vitae` ใน `combat_step/cast.rs` เสียบใน priority chain
  ที่ 0.53 (ulti ก่อน) และ 0.54 (Rivus) — ulti มาก่อนเพราะถ้าฮีลถูก ๆ กลบดาเมจไปหมด เกจจะไม่ถูกใช้เลย
- ✅ `authored_skills("astier")` + star_ranks (+1 level ต่อ ★ ให้ทั้งสาม active)
- ✅ item def `exemplar_astier` ใน game_data
- ✅ goldens: ไม่มี golden ไหนถือเธอ → byte-identical

### 10.7 ⚠️ ค้างเคาะ

- art ยังไม่มี
- **ทุกตัวเลขตั้งจากรูปทรง ยังไม่ได้วัด** — power ของ Rivus (6000 bp), decay 100/70/50,
  ตัวคูณของ Vortex Undae และ 20→38% ของ Fons Vitae ต้องรัน sim-cli เทียบปาร์ตี้ที่มี Lucielle
  กับที่มี Astier ที่ band เดียวกันก่อนล็อก
- ฮีลล้นไปไหน — ตอนนี้ **หายไปเฉย ๆ** (ไม่มี shield จากส่วนเกิน) เพราะยังไม่มีเพดานอื่นในระบบ
  และผู้ใช้สั่งไว้ว่าอย่าเพิ่งใส่เพดานใหม่
- decay ของ Rivus เป็นค่าคงที่ ไม่ขยับตาม ★ — ถ้าอยากให้ ★ สูงเด้งแรงขึ้น ต้องเพิ่มแกนนั้นเอง
- SSR คู่ของสาย Support × Water ยังไม่ออกแบบ
