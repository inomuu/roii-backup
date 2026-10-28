# Card system (DESIGN SPEC — draft)

Status: **DESIGN — agreed direction, not built.** Captures the 2026-07-02 discussion.
The card MECHANIC already exists in code (`ItemKind::Card`, `CardEffect`, gear sockets,
`slot_card`, `combat::card_bonuses`); this spec RE-SHAPES it (slot-typed + a card panel
separate from the gear item) and defines the CONTENT. Pairs with [[drop-system]] (cards
are Axis-A signature drops, rate/cap-governed) and the rarity system (below / next).

## Confirmed decisions
1. **Effects are flexible PER CARD** — a card fills whatever effect fields it needs
   (flat stats, RO-signature conditionals, or a special). "Cards do various things."
2. **Slot-typed (RO-style)** — every card belongs to a gear-slot CLASS and only slots
   into that class's card slot.
3. **Cards equip to the CHARACTER, not the item** — a per-character card PANEL by class
   (Weapon 4 / Shield 1 / others 2). Cards persist across gear swaps; the equipped gear's
   socket count only gates how many are ACTIVE (`active = min(equipped, sockets)`). The
   OLD "card baked into a gear item socket" path is removed. (Full rules below.)

## Card classes (slot-typed)
Six classes, mirroring the gear slots (Headgear optional / later):
`Weapon · Armor · Shield · Garment · Shoes · Accessory`
- Each card def carries `card_slot: <class>`. It only occupies that class's slot.
- Thematic effect-by-class (RO convention, not enforced by code — guidance for authoring):
  - **Weapon** → offense: %dmg vs race/size/element, ATK, crit, auto-cast on hit.
  - **Armor** → survivability: HP, HP%, status resist, element resist.
  - **Shield** → mitigation: %dmg reduction vs race/element, DEF.
  - **Garment** → evasion / element resist (FLEE, resist).
  - **Shoes** → HP/SP, move/heal.
  - **Accessory** → raw stats (STR/AGI/…), SP, special utility.

## Card PANEL — equip cards to the CHARACTER, activate by gear sockets (CONFIRMED)
The card no longer goes INTO a gear item. Each character has a CARD PANEL with a fixed
number of slots per class; cards equip THERE and persist across gear swaps.

**Panel size (how many cards you can EQUIP per class):**
| Class | Panel slots |
|---|---|
| Weapon | **4** |
| Shield | **1** |
| Armor · Garment · Shoes · Accessory | **2** each |
| Headgear | TBD |

**Activation rule:** for each class,
`active = min(cards equipped in the panel, socket_count() of the equipped gear)`.
- You may EQUIP up to the panel max even with low-socket gear, but only the first
  `socket_count` of them are ACTIVE (apply their effect). The rest sit inactive (grey).
- Cards live on the CHARACTER → swapping the gear item KEEPS the equipped cards; only the
  active count re-evaluates against the new item's sockets.
- Example: 2 cards in the ARMOR panel + an armor with **1 socket** → **1 active**. Swap to
  a 2-socket armor → **both active**. Swap to a 0-socket armor → 0 active, cards still
  equipped (re-activate when you wear a socketed armor again).
- Effect: pre-load a build (weapon panel up to 4), and gear upgrades (more sockets)
  progressively "turn on" more of your cards — RO's "more slots = more power" WITHOUT the
  bind-and-destroy tax.

## Migration — REMOVE the old card-in-item system FIRST (build step 1)
The current code compounds a card into a specific equipped item's sockets. That path is
REPLACED, not extended. To remove:
- `Item.cards` (the baked-in socket contents on gear) — drop it.
- `slot_card` (event that pushes a card into `equipped[slot].cards`) — replace with an
  equip-to-panel event (by class).
- `combat::card_bonuses` reading `equipped[].cards` — reread from the character card panel,
  summing only the ACTIVE cards (min-with-socket_count per class).
- KEEP `Item.card_slots` / `socket_count()` on gear — it now gates ACTIVATION (how many
  panel cards light up), not storage.
- New per-CHARACTER state (in `CharacterData`, alongside `equipped`): the card panel,
  e.g. `card_panel: BTreeMap<CardClass, Vec<Item>>` (per-character, NOT party-shared).
- VERSION bump (serialized shape change on `CharacterData` + `Item`); goldens re-bless if
  any golden equips a card (check — most don't).

## Effect model (extend `CardEffect`)
Existing (keep): `str/agi/vit/int/dex/luk`, `atk/def/matk/hp`, `hit/flee/crit_bp`,
`hp_pct/atk_pct`.
Add the RO-signature CONDITIONALS (the build-defining layer — the game already has
race/size/element on monsters):
- `dmg_vs_race{race, pct}` — +X% damage to a monster RACE (e.g. Plant +30%).
- `dmg_vs_size{size, pct}` — +X% vs SIZE (Small/Medium/Large).
- `dmg_vs_element{element, pct}` — +X% vs an ELEMENT.
- `resist_element{element, pct}` — take −X% from an element.
- (later, optional) `def_vs_race`, status resist %, HP/SP leech %, auto-cast{skill, chance}.
Each card populates only what it does. Multiple conditionals can stack per card for a
Legendary.
- **DEPENDENCY:** conditional %dmg needs the target's RACE/SIZE on the combat `Entity`.
  Today only `element` is on the Entity; `race`/`size` are descriptive in `MonsterDef`
  only → they must be SURFACED onto the spawned entity before weapon-conditional cards
  can compute. (Element cards work today.)

## Market LIQUIDITY — equip is reversible; sinks are elsewhere (2026-07-02)
Unlike classic RO (compounding BINDS/destroys a card into one gear item), our cards equip
to the character PANEL and `UnequipCard` returns them to the stash intact — cards stay
`marketable`. So a player who re-specs, quits, or upgrades can PULL cards back out and
SELL them → cards CIRCULATE → a livelier secondary market (the user's point: "ถอดขายได้
ตลาดคึกกว่า"). Liquidity (equip/unequip freely) is DECOUPLED from the destructive SINKS:
- **FUSE** (6 copies → 1 higher-★) destroys 5 — the big supply sink.
- **CONSUME** (card → permanent stat) destroys it — the smaller sink.
- **UNEQUIP** destroys nothing — pure liquidity.
So supply drains through fuse/consume + the ~900-card max demand, while unequip keeps the
market fluid. Open: are FUSED (★>0) / FOIL cards TRADEABLE (a high-end market), or does
fusing BIND them (protect the low-end market / force everyone to fuse their own)? Leaning
tradeable — a ★5 Foil is a marquee trade item — but revisit with the market design.

## Consume vs slot (dual-use) — keep, as a card SINK
- Keep the existing `consume_effect`: consuming a card grants a SMALLER PERMANENT bonus
  (Redling consume = atk+1) — a dupe sink + F2P path.
- Slot (panel) = the big, gear-gated bonus; Consume = a small forever bonus. Both stay.
- **Consume permanent is PER-CHARACTER (CONFIRMED)** — the bonus lives on the character
  that consumed it (in `CharacterData`, like stats), not a shared account codex. Each
  character builds its own consumed-card history.

## Duplicates ALLOWED in a panel (RO3-style, 2026-07-02 revision)
REVISION: cards CAN stack — you may equip the same card multiple times in a class panel
(RO3 does this: 2× Rabbit Card in a 4-slot two-handed weapon → 2× the effect). Our code
already allows it (`equip_card` only checks the class's `card_panel_capacity`, not
uniqueness). So a Weapon panel can hold up to 4 cards, dupes included, each contributing
its effect (subject to the active = min(equipped, sockets) gate).

## Card leveling & rarity promotion — Honkai-style star fuse (BUILT 0.0.216)
> Status: LIVE as `InputEvent::FeedCard` (0.0.216) + the album feeder-picker UI.
> Rules as shipped: pick ANY sibling stack (any star, foil included) → confirm
> (cannot be undone) → star gain = 1 + feeder's star (copies conserved), cap 5★;
> a foil feeder converts the target to foil — legal on a 5★ plain target as pure
> conversion. Divergence from the old sketch below: star-scale is +20%/★ on the
> authored base (0.0.211), not the 50%+10%×★ curve; there is no separate-SKU
> foil drop — foil is a 5% roll on any card drop.
With dupes now stackable, fusing is an OPTIONAL way to COMPRESS copies: instead of
slotting 2 copies (2 sockets), fuse them into ONE higher-★ card (1 socket, more per-card
power) — trading socket count for density, like a Honkai signature weapon (0★→5★). Both
paths coexist: stack dupes for raw power if you have sockets to spare, or fuse to save
sockets + climb stars.

**Two axes on a card instance: STAR level (PRIMARY) + variant.**
The STAR is the main progression number a player reads ("Pinkling ★3"); the variant
(Epic/Foil) is secondary flavor.
- **Star (level) — the primary axis:** feed a duplicate card → +1 star (0★ base → 5★
  max; exact range tunable). Each star scales the card's effect. (Dupes may ALSO just be
  slotted side-by-side — fusing is the compress-for-★ alternative.)
- **MAX cost = 6 copies + 1 shiny = 7 cards (2026-07-02).** 0★→5★ eats the base + 5 fused
  duplicates = **6 Epic copies**; then **+1 Foil** promotes it to `Foil ★5` (the apex). So
  a single fully-maxed card sinks ~7 cards — the main driver of the demand-side scarcity
  ([[drop-system]]): even a generous drop rate stays scarce because maxing is this hungry.
- **Variant — Epic (normal) vs Foil (special):**
  - Every card has an **EPIC** (normal) version and a **FOIL** version. **The Foil drops
    at ~1/20 the Epic's rate** and carries an EXTRA effect on top of the Epic effect.
    ("Foil" = the TCG shiny — nicer than "rare", and its own gold/foil UI treatment.)
  - **Cross-feed promotes:** feeding a FOIL card into an EPIC card PROMOTES it to Foil
    (adds the extra effect) AND grants the +1 star of a normal feed.
    - e.g. `Pinkling Epic ★1` + feed a Foil → `Pinkling FOIL ★2`.
  - **At max star, a Foil feed only promotes the variant:** `Pinkling Epic ★5` + Foil
    → `Pinkling Foil ★5` (already max stars; the feed just upgrades to Foil + adds the
    extra effect, no star overflow).
- **Power curve is FLAT — 0★ ≈ 50% of 5★ (F2P-friendly, anti-P2W, 2026-07-02):** the
  authored effect is the 5★ (full) value; star-scale runs `50% + 10%×★` → **0★ = 50%,
  5★ = 100%**. So a free player running 0★ cards sits at ~half power and STAYS COMPETITIVE;
  maxing is a ~2× top-up, NOT a 10× cliff. The 7-card max cost buys economy scarcity (a
  big sink) WITHOUT pay-to-win — whales get 2×, not dominance. Aligns with the P2W-decouple
  economy stance. Foil's EXTRA effect is likewise a modest add-on, not a power gate.
- **Data shape (reserve now, effect layer reads from day one — cheap to carry, costly to
  retrofit):** a card Item carries `star: u8` and a `foil: bool` variant. Effect
  = full_effect × (50% + 10%×star) + (foil ? extra-effect : none).
- Status: **DEFERRED build, but SPEC now** so the data + effect layer are shaped for it.
- The variant maps to the global `Rarity` for colour/routing: **Epic → Epic tier, Foil →
  Legendary** (see [[rarity-system]]); the RATE stays the card rules, not the tier.

## Card variant — Epic vs Foil (STARS are the real progression)
A card's power is read mostly by its STAR (above). The variant is the flavor tier:
- **Epic** — the base effect (a flat stat and/or a conditional). The normal drop.
- **Foil (~1/20 of Epic)** — Epic effect PLUS an extra effect (a second conditional, a
  special, or a bigger number), with a shiny/foil UI treatment. The chase.
- Both level 0★→5★ by fusing dupes; a Foil feed promotes an Epic to Foil.
- Boss / MVP cards can be a stronger baseline (their Foil = the apex, from signature boss
  drops, [[drop-system]] Axis A).
- **Maps to the global `Rarity`:** Epic → Epic tier, Foil → Legendary (colour + rare-bag
  routing come from the enum). See [[rarity-system]]. Rate stays the card rules, not the
  tier. (Cards tag `rarity: Rare` today → migrate to Epic/Legendary.)

## Starter cards — the 3 -ling slimes (DESIGNED 2026-07-03)
All three are **Armor**-slot cards (the basic "slime" cards you slot in your body armor),
each teaching a role. In-game today they apply their **0★ base** (leveling is deferred);
the per-★ curve below is the target once fusing is built.

| Card | Slot | Drops from | 0★ base | 5★ max |
|---|---|---|---|---|
| **Pinkling** | Armor | Pinkling family | +5 ATK, +1 STR | +10 ATK, +3 STR |
| **Greenling** | Armor | Greenling family | +200 HP, +1 VIT | +400 HP, +3 VIT |
| **Yellowling** | Armor | Yellowling family | +10 FLEE, +1 AGI | +20 FLEE, +3 AGI |

**Star curve (design intent for the deferred leveling):**
- **Primary** (ATK / HP / FLEE): linear 0★→5★, DOUBLES (0★ = 50% of 5★, the anti-P2W
  flat curve). Steps: Pinkling ATK 5→6→7→8→9→10; Greenling HP 200→240→…→400 (+40/★);
  Yellowling FLEE 10→12→…→20 (+2/★).
- **Secondary** (STR / VIT / AGI): **+1** at 0★–1★ → **+2 at 2★** → **+3 at 5★**.
- (`card_redling` renamed → `card_pinkling` to match the monster.)

**FOIL (Shiny) = Epic effect PLUS a per-card EXTRA (2026-07-03, BUILT).** A Foil card ADDS
its authored `foil_effect` on top of `card_effect` — think "**+how much more**", NOT a
fixed ×2. Each card's foil extra is its own thing and MAY be an unrelated stat/effect.
- Implementation: `Item`/`ItemDef` gain `foil_effect: Option<CardEffect>` (the delta).
  `combat::card_bonuses` sums `card_effect` + (foil ? `foil_effect`). Data-driven per card.
- The 3 -ling starters happen to add "+the same primary again" (so their foil reads as ×2):
  Pinkling foil_effect +5 ATK (→ +10), Greenling +200 HP (→ +400), Yellowling +10 FLEE (→ +20).
- **Foil DROP: a dropped card rolls 5% (`FOIL_DROP_CHANCE_BP`) to be Foil** — one extra PRNG
  draw taken ONLY on the rare card-drop path (`drops::drop_on_death`). (Star/fuse leveling
  still deferred; foil DROP + effect are live.)

## Later card tiers (illustrative — not built)
| Card | Class | Effect |
|---|---|---|
| (Plant Killer) | Weapon | +30% dmg vs Plant |
| (Fireproof) | Armor | −30% dmg from Fire |
| (Boss card) | Accessory | +3 all stats + auto-cast on hit |
Layer conditionals (%dmg / resist vs race/element) as new cards drop past the -ling starters.

## Card UI — layout spec (from the RO3 reference, 2026-07-02)
A dedicated card MODAL (like RO3's 插卡 screen), three regions:
- **LEFT — class rail:** a vertical strip of the 6 gear-class icons (Weapon/Shield/Armor/
  Garment/Shoes/Accessory). Each shows the equipped-gear's socket pips (how many cards of
  that class can be ACTIVE). Click to select which panel to edit. Selected = highlighted.
- **CENTER — the selected class's PANEL slots:** big card slots laid out for that class —
  Weapon 4, Shield 1, others 2. A slot is ACTIVE (bright) if its index < the equipped
  gear's socket count, else LOCKED/greyed (card sits there but inactive). Equipped cards
  show art + ★ + a small "equipped" tag. Empty active slots invite a card. The class's
  gear icon sits in the centre emblem (RO3 shows the weapon there).
- **RIGHT — card detail + collection:**
  - Detail of the focused card: name, ★★★, effect list (ATK +22, +0.23% dmg, …), and an
    Equip / Unequip (取下) button.
  - The player's OWNED cards of the selected class (from the stash) as a grid to pick from;
    click a stash card → equips into the first free panel slot (dupes allowed). "已裝備"
    (equipped) marker on ones already in the panel.
- **Wiring:** click stash card → `sim_driver.equip_card(stash_idx)`; click panel card's
  Unequip → `sim_driver.unequip_card(panel_idx)`. Read the panel from the entity's
  `card_panel` (bridge must surface it) + socket counts from equipped gear.
- **Bridge TODO:** surface `card_panel` (array of card views w/ class/★/effect) + each
  equipped gear's `socket_count` per class on the member view, so the UI can render active
  vs locked without recomputing.
- NOTE: build needs VISUAL iteration with the game open — deferred while the machine is hot.

## Open decisions (before build)
- Headgear class now or later.
- Special-effect set in scope for the first pass (auto-cast? leech? status resist?).
- Per-class slot counts (how many weapon-card slots can a character have max).
- Surface `race`/`size` onto the combat Entity (dependency for weapon conditionals).

## Star upgrade (5★) — design pinned 2026-07-03 (NOT built)

จุดยึดโค้ดที่มีแล้ว: `Item.star` field + `same_card_stack` (def_id+foil+star) → การ์ดคนละดาว
แยก stack ใน album เองโดยธรรมชาติ; UI มี `_stars()` วาดอยู่แล้ว.

1. **Fuse:** การ์ดชนิด+variant เดียวกัน N ใบ → +1 ดาว; ดาวที่ N กิน N ใบ
   (1/2/3/4/5 → สะสม 15 ใบถึง 5★). สร้าง bulk demand ให้ใบธรรมดาในตลาด.
2. **สเกลผล (integer ล้วน):** สายเลขแบน `value×(2+star)/2` (×3.5 @5★) /
   สาย % (crit_bp, hp_pct, atk_pct) `value×(10+3·star)/10` (×2.5 @5★ — ชันน้อยกว่า
   เพราะคูณ curve อื่นอยู่แล้ว).
3. **Drawback (ค่าลบ) ไม่สเกล** — ดาวสูง = คุ้มขึ้น (Burst Chom 5★ = +42/−3).
4. **Foil แยกสาย fuse** ตาม stack ที่แยกอยู่แล้ว; foil_effect สเกลกติกาเดียวกัน.
5. **Consume (1/2/3, cap 3) คงเดิม** → คู่ตัดสินใจ dupe sink: กินทิ้ง vs เก็บ fuse.
6. Implement ต้องมี: `FuseCards` InputEvent, `card_bonuses` คูณ star factor, VERSION bump.
7. NOTE "strictly better": การ์ด band สูงแรงกว่าเดิมตรง ๆ ยอมรับได้ในชั้น flat-stat
   (user 2026-07-03) — ตัวแก้ระยะยาวคือชั้น CONDITIONAL (dmg_vs_race/…, สเปคด้านบน):
   การ์ดสูง ๆ ควรเป็น conditional/situational, flat-stat เป็น commodity tier;
   5★ ของการ์ดเก่าคาบเกี่ยว 0★ ของการ์ดใหม่ = การลงทุน fuse ไม่ตายทันที.

## Chum card set — ชุดแรกที่ author จริง (band 3-4, ลง data แล้ว 0.0.129)

| card id | monster | slot | 0★ | 5★ | drop |
|---|---|---|---|---|---|
| card_chomb | chomb (+chomber/chomboom rate-up) | Weapon | ATK+4, CRIT+2% | ATK+14, CRIT+5% | 1/20k · elite 1/10k · boss 1/47 |
| card_toxic_chom | toxic_chom | Garment | FLEE+10 | FLEE+35 | 1/20k |
| card_burst_chom | burst_chom | Accessory1 | ATK+12, DEF−3 | ATK+42, DEF−3 | 1/20k |
| card_rechum | rechum | Shoes | MaxHP+3%, INT+1 | MaxHP+7%, INT+3 | 1/20k |

Convention ที่ตั้งจากชุดนี้: มอนดรอปการ์ดตัวเอง; elite/boss ของชนิดเดียวกันดรอปใบเดิม
ที่ rate สูงขึ้น (ไม่แตกใบใหม่ — สภาพคล่องตลาดต่อใบดีกว่า). หน่วย %: hp_pct/crit_bp =
basis points (300 = 3%).

**Data-hygiene TODO (พบ 2026-07-03):** การ์ด -ling ยัง placeholder ไขว้ตัว —
pinkling-family→card_greenling, yellowling→card_pinkling, ไม่มีตัวไหนดรอปการ์ดตัวเอง
และ greenling/big_pinkling/king_yellowling ไม่ดรอปการ์ดเลย → remap เป็น self-drop +
king_yellowling = rate-up ตาม convention ใหม่. อนึ่ง `rechum` สะกดหลุด pattern ตระกูล
(ฐาน "chom") — ถ้าไม่ตั้งใจเล่นคำ ควร rename → `rechom` (ยังไม่ตัดสิน).

## Socket source — DECIDED 2026-07-03

**Socket ฝังใน item def** (RO แท้): "Sword [1]" = คนละ item id กับ "Sword" —
ดรอปแยก/แรร์กว่า/ราคาตลาดแยก. ของทุกชิ้น fungible 100% = เข้ากฎ clean-items
ของตลาด (บทเรียน TBH). **การ "เจาะรู" ในอนาคต = def-swap**: บริการแปลง
sword_s3 → sword_s3 [1] (เปลี่ยน def id, ไม่ mutate instance) — Gil/material sink
ที่ไม่สร้าง per-instance state. งานถัดไป: ออกแบบ variant ติด socket ของ gear
แต่ละ tier + drop weights (ยังไม่ทำ — nothing built).

## Socket-count matrix (user 2026-07-04)

| ประเภท | รู | หมายเหตุ |
|---|---|---|
| อาวุธ 1 มือ | 0-3 | |
| อาวุธ 2 มือ | 0-4 | = panel cap Weapon พอดี |
| **มือรอง (WeaponLeft)** | **0-1 (กฎการนับ)** | def เดียวกับมือหลัก — ตอนคำนวณ card_sockets อาวุธในมือรอง contribute `min(card_slots, 1)`; กฎ activate เมื่อ [2]+ เกิด |
| โล่ | 0-1 (rec — panel cap Shield = 1; ถ้าอยาก 0-2 ต้องขยับ cap) | pending |
| อื่น ๆ (เกราะ/ผ้าคลุม/รองเท้า/แหวน/จี้) | 0-2 | = panel cap |

Archetype ล็อคกันเอง: 2H = 4 ชิ้นเดียว / dual = 3+1 สองชิ้น / 1H+โล่ = 3 + การ์ดโล่.

**เรทต่อจำนวนรู (แหล่งขยับขึ้นชั้นแทนหั่น weight — floor w=1 บนตารางเล็ก):**
[1] = มอนทั่วไป 1/8 ของ base (ลงแล้ว 0.0.130) · [2] = ตาราง boss/elite
(~1/40-80/kill) · **[3] และ [4] (2H) = ชั้นเดียวกัน** (gated dungeon + boss
b9+; [4] แรร์กว่า [3] เล็กน้อย) — วัดด้วยต้นทุน complete archetype (user
2026-07-04): 2H [4] ≈ 1H [3]+off [1] สองชิ้น → ต้องถึงพร้อมกัน ไม่ใช่คนละยุค.
เส้นทางที่สอง: เจาะรู def-swap service (deterministic, แพง — Gil/orb sink).
Rollout: launch [1]+[2] → dungeon era [3]+[4].
