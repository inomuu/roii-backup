//! Transient buffs & debuffs — timed stat / combat modifiers stamped onto
//! an `Entity` by skill casts (self-buffs, party buffs, on-hit debuffs).
//!
//! Idle-adapted design: there is no manual buff timing. Active skills carry
//! `cast_buff` specs (applied on cast, to self or the whole party) and an
//! `on_hit_debuff` (rolled once on a hit). Buffs age one tick per
//! `tick::decay_buffs` and self-expire — there's no manual dispel.
//!
//! Determinism (GDD §4.2): all math is integer, no PRNG. Stat-add buffs are
//! folded through the SINGLE `buffed_stats` fold point so STR/AGI/VIT/INT/DEX
//! deltas flow into every derived stat (ATK, MATK, DEF, HIT, FLEE, ASPD) with
//! one technique. Percentage/flat combat buffs (`AtkPct`, `HitAdd`, …) fold on
//! top in `combat::derived`.

use serde::{Deserialize, Serialize};

use crate::grid::{Element, Entity};

/// What a buff modifies. Fieldless + `Copy` so a `Buff` stays a small POD and
/// serializes as a bare enum tag (compact JSON, stable hash). The `AtkElement`
/// variant carries its element in the buff's `magnitude` (see
/// `element_to_mag` / `mag_to_element`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BuffKind {
    // --- percentage / flat combat modifiers ---
    #[serde(alias = "MatkPct")]
    AtkPct,
    DefPct,
    /// FLAT hard-DEF add (folded in `combat::effective_hard_def` BEFORE the
    /// `DefPct` multiplier, so gear DEF + this both scale with `DefPct`). Gives a
    /// real diminishing %-reduction even on a small base, unlike `DefPct` alone —
    /// used by the Soul-path DEF spine. `magnitude` in flat DEF points.
    HardDefAdd,
    /// % boost to MAX HP (folded in `combat::effective_max_hp`). Used by the
    /// global "Fortitude" aura. `magnitude` in whole percent.
    HpPct,
    /// FLAT max-HP add (folded in `combat::effective_max_hp`, after the VIT mult like
    /// a flat HP card). Soul-path "Lifeblood" nodes. `magnitude` in HP points.
    MaxHpAdd,
    /// FLAT ATK add (folded in `combat::effective_atk` before the %ATK). Soul-path
    /// "Edge" nodes — flat weapon-independent damage. `magnitude` in ATK points.
    #[serde(alias = "MatkAdd")]
    AtkAdd,
    AspdPct,
    /// ASPD % from a POTION (aspd_pot_*) — same math as `AspdPct` (both fold
    /// into the FJK `(1 − pot% − skill%)` term, summed, capped by the 207 max)
    /// but a SEPARATE kind so the potion's `BuffMissing` auto-drink trigger
    /// doesn't see Sword Quicken: RO-style, a skill quicken and a potion STACK;
    /// two potions don't (the second won't drink until the first expires).
    AspdPotionPct,
    /// % boost to movement speed (RO Increase AGI / Agility Up). Folded into the
    /// move accumulator in `tick::move_step`. `magnitude` in whole percent.
    MoveSpeedPct,
    /// Enemy debuff: a percentage REDUCTION of the victim's hard DEF.
    DefDownPct,
    /// Flat % reduction of ALL incoming damage, applied last in `apply_damage`
    /// (after DEF + element). Arcane Ward — a "mana shield" kept up while it fits
    /// the loadout's SP-reserve slots (3); no per-hit SP cost. `magnitude` in bp.
    DmgReducePct,
    /// Damage-over-time: `magnitude` HP per regen interval (`tick::decay_buffs`).
    PoisonDot,
    /// Heal-over-time: `magnitude` HP RESTORED per regen interval
    /// (`tick::decay_buffs`), capped at max HP. Rejuvenating Potion. Mirror of
    /// `PoisonDot` with the sign flipped.
    HealHot,
    /// Self weapon-element override (Magnum Break Fire). `magnitude` encodes the
    /// `Element` — read via `buff_atk_element`.
    AtkElement,
    /// MARKER buff (no stat effect) — Monster Lure: while a player carries it,
    /// monsters spawn only within `magnitude` cells of THAT player. Read by
    /// `spawn::spawn_one_monster`, never folded into `buffed_stats`.
    SpawnLure,
    /// MARKER buff (no stat effect) — Monster Accelerator: while any player
    /// carries it, the continuous-spawn trickle fires twice as fast. Read by
    /// `spawn::try_continuous_spawn`, never folded into `buffed_stats`.
    SpawnHaste,
    /// Timed POISON resistance in basis points, from a Meal. Read by
    /// `combat::stat_mods::resist_status_bp` alongside the gear/card lines — buffs were not a
    /// source before this, so food could not ward anything.
    ResistPoisonBp,
    /// Kill EXP bonus in basis points (2026-10-01, the EXP Elixir party provision). Read by the
    /// kill-XP split as the party's HIGHEST holder, never summed; not a stat.
    XpPct,
    /// BLIND status — while present, the afflicted entity's effective HIT and
    /// FLEE are cut by 25% (folded in `combat::effective_hit_points` /
    /// `effective_flee_points`). A marker: any positive `magnitude` = blinded;
    /// the 25% is fixed, not magnitude-scaled. Applied by Sand Strike (and later
    /// Vermilion Field / etc.). Append-only variant — keeps Buff serialization
    /// of existing kinds stable.
    Blind,
    /// HIDING status (thief panic) — while present the entity is UNTARGETABLE
    /// (`combat::find_enemy_idx` skips it) and takes NO action (the tick loop
    /// skips its turn). A marker (magnitude unused); the heal-while-hidden is a
    /// separate `HealHot` buff applied alongside. Append-only.
    Hiding,
    /// STUN status (Assassin Sonic Blow) — while present the entity takes NO
    /// action (immobilized: no move, no attack — the tick loop skips its turn).
    /// UNLIKE Hiding it is still TARGETABLE and still takes damage; UNLIKE a
    /// freeze it does NOT change element and does NOT break on damage (it just
    /// decays). A marker (magnitude unused). Bosses are immune (never applied).
    /// Append-only.
    Stun,
    /// CLASSIC POISON status (Assassin Poison Cloud) — distinct from the
    /// ATK-scaled `PoisonDot` (Deadly Poison). While present: soft/status DEF is
    /// cut 25% (`effective_soft_def`); the victim loses `1.5%·MaxHP + 2` HP per
    /// second BUT only while HP > 25% MaxHP — it floors at 25% and CANNOT kill
    /// (never routes the death path); SP regen is disabled. Bosses are immune
    /// (never applied). A marker (magnitude unused; the DoT is MaxHP-derived).
    /// Append-only.
    Poison,
    /// BLEED DoT (thief Open Wound) — mechanically identical to `PoisonDot`
    /// (ATK-scaled `magnitude` HP per regen interval; CAN kill; hits bosses), but
    /// a SEPARATE kind so poison stays the Assassin's theme — Bleed does NOT feed
    /// Poison Explosion. Aged + applied alongside `PoisonDot` in `decay_buffs`.
    /// Append-only.
    Bleed,
    /// FROZEN status (Elementalist Glacial Storm) — like `Stun` it immobilizes
    /// (no move/attack; the tick loop skips the turn), still targetable; bosses
    /// are immune. Additionally (RO): DEF halved (`effective_hard_def`), FLEE 0
    /// (`effective_flee_points`), armor turns Water (`apply_damage`), and the next
    /// damage BREAKS it (`resolve_attack`/field skips frozen victims). Append-only.
    Freeze,
    /// QUICKSAND slow — a `magnitude`% REDUCTION of AGI **and** DEX (folded in
    /// `buffed_stats`, so it cuts ASPD / FLEE / HIT / cast) AND of move speed
    /// (folded in `tick::movement` like a negative `MoveSpeedPct`). Stored as a
    /// POSITIVE percent and subtracted everywhere. 0 ⇒ unchanged. Append-only.
    AgiDexSlowPct,
    /// ROOT status (Ranger Snare Strike) — movement LOCKED but the entity CAN still
    /// attack (unlike `Stun`/`Freeze`, which block all actions). The tick loop skips
    /// only its move step. A marker (magnitude unused); bosses immune (never
    /// applied). Decays in `decay_buffs`; does NOT break on damage. Append-only.
    Root,
    /// BURN DoT (Yellowling's thorn burn) — like `Bleed`: `magnitude` HP
    /// per regen interval, CAN kill, hits bosses. A SEPARATE fire-themed kind so a
    /// burn never collides with thief Bleed / assassin PoisonDot and can carry its
    /// own visual. Does NOT feed Poison Explosion. Applied in `upkeep` alongside
    /// the other damage DoTs. Append-only.
    Burn,
    /// VULNERABLE (thief Open Wound's Vulnerable rune) — the target takes
    /// `magnitude` bp MORE damage from ALL sources (the mirror of `DmgReducePct`);
    /// applied at the end of `apply_damage`. Hits bosses. Append-only.
    DmgTakenPct,
    /// ASPD-SLOW (acolyte Aspd Dampening) — reduces the victim's ATTACK SPEED by
    /// `magnitude` basis-points-of-percent (1150 = 11.5%). A DIRECT ASPD cut,
    /// unlike the AGI-based slows: it bites monsters (flat-ASPD) too, applied as
    /// a final multiplier in `effective_atk_speed_milli`. Append-only.
    AspdSlowPct,
    /// HOLY VULNERABILITY (Sanctus Terra) — the game's FIRST element-specific vulnerability:
    /// the target takes `magnitude` bp MORE damage from HOLY-element sources ONLY (folded in
    /// `apply_damage` via `holy_vuln_bp`, gated on the incoming element being Holy). Named
    /// after the skill that applies it; a future holy-vuln from a DIFFERENT skill gets its OWN
    /// kind (kept separate) and is summed into `holy_vuln_bp`. Append-only.
    SanctusTerra,
    /// Flat MATK add (Lucielle Benedictio party buff). Folded in `effective_atk`
    /// exactly like `AtkAdd` folds in `effective_atk`. Beneficial → PerSource. Append-only.

    /// +% HOLY damage DEALT (Lucielle Benedictio) — ATTACKER-side; multiplies the
    /// outgoing `amount` in `build_damage_as` ONLY when the packet element is Holy.
    /// Beneficial → PerSource. Append-only.
    HolyDmgDealtPct,
    /// HOLY VULNERABILITY (Lucielle Lumen Contagio) — a SECOND holy-vuln kind, summed
    /// into `holy_vuln_bp` on top of `SanctusTerra` (per-kind Strongest, cross-kind ADD).
    /// Debuff. Append-only.
    LumenVuln,
    /// UMBRAL MARK (0.0.424, Aveline's Umbra Fletch): +% SHADOW damage taken, applied on every
    /// hit she lands, one instance per source whose magnitude climbs by a step per hit to a
    /// cap (5% × 3). Read by `shadow_vuln_bp`.
    UmbralMark,
    /// HEARTSTRING (0.0.451, Nemu): a Bleed-shaped DoT whose every tick also HEALS its source
    /// for `LEECH_BP` of the damage. Ticks beside `Bleed` in `decay_buffs`; the heal routes to
    /// `source_id`. Per source. Append-only.
    LeechBleed,
    /// SWEET POISON (0.0.451): the move slow that rides Nemu's mark, per source, magnitude =
    /// whole percent (stacks × step). Read where `AgiDexSlowPct` is. Append-only.
    MarkSlow,
    /// SWEET POISON: the ASPD slow that rides the mark, per source, magnitude in bp. Read
    /// where `AspdSlowPct` is. Append-only.
    MarkAspdSlow,
    /// FROST AURA (0.0.456, Lumi's ultimate) — a marker on a PARTY member: while it lasts, any
    /// enemy that lands a hit on the carrier is stamped with the aura's SOURCE's mark (Chill)
    /// as if she had hit it (`combat::apply_hit` → `stamp_mark`). `source_id` = the caster;
    /// magnitude unused. Not a debuff. Append-only.
    FrostAura,
    /// FROST MARK (0.0.459) — the Water twin of `UmbralMark`: `magnitude` bp of extra WATER
    /// damage taken, stacked by `combat::stamp_mark` for a marker whose passive attacks with
    /// Water (Lumi's Chill). Append-only.
    FrostMark,
    /// STATIC MARK (0.0.462) — the Thunder twin: `magnitude` bp of extra THUNDER damage taken,
    /// stacked by `combat::stamp_mark` for a marker whose passive attacks with Thunder (Airi's
    /// Static). Append-only.
    StaticMark,
    /// BLACKWATER MARK (0.0.463, Charon) — a seal on ONE enemy: hits from its SOURCE chip the
    /// target's Toughness for `magnitude` bp MORE when the target is weak to the element, and
    /// at HALF when it is not (a bar no one else could touch). Debuff. Append-only.
    BlackwaterMark,
    /// DAZZLE (0.0.464, Laplus' Pickpocket) — `magnitude` bp of the victim's blows whiff, the
    /// same inverse fold as Darkness (both read in `combat::miss_chance_bp`); a separate kind
    /// so the icon and the stack are hers. Debuff. Append-only.
    Dazzle,
    /// SUNDER (0.0.465, Suzu) — the target's DEF is cut by `magnitude` percent. Its own kind
    /// beside `DefDownPct` so it stacks from a different source, carries its own icon, and
    /// counts as one more status for an Exploit. Debuff. Append-only.
    Sunder,
    /// BREAK FEVER (0.0.465, Suzu's ultimate) — a SELF buff: while it lasts the owner's basic
    /// chips Toughness at `magnitude` percent and ignores the weakness gate. Append-only.
    BreakFever,
    /// STATIC SPARK (0.0.465, Mirei) — `magnitude` = what ONE spark pays (bp of the layer's ATK
    /// at the time it was laid, resolved into flat damage); the buff's `ticks` is its life and
    /// the STACK COUNT rides `source_id`-keyed instances. Struck → one spark fires; the layer's
    /// own hits fire it without spending a stack. Debuff. Append-only.
    StaticSpark,
    /// +% damage DEALT of EVERY element (0.0.468, Astier's Tidebless) — attacker-side, folded
    /// once in `build_damage_as` beside the Holy bump. Beneficial → PerSource. Append-only.
    DmgDealtPct,
    /// +% damage DEALT of ONE element: the element rides `source_id`'s twin, `magnitude` is
    /// `(element_to_mag << 16) | bp` (0.0.468, Astier's rolled element). Append-only.
    ElementDmgDealtPct,
    /// +% Limit Break gauge gained from every source while it lasts (0.0.468, Fons Vitae).
    GaugeRatePct,
    /// Darkness (0.0.426): the holder WHIFFS `magnitude` basis points of its blows — a
    /// magnitude-driven Blind. Strongest mark wins; halved on a boss at apply time.
    Darkness,
    /// Ashelle CARD stack (0.0.212): +ASPD bp granted when the holder is healed
    /// by a SKILL — up to 3 concurrent instances, each on its own 5 s timer.
    /// Folded with `AspdPct` in `effective_atk_speed_milli`. Append-only.
    CardAspdStack,
    /// +CRIT CHANCE in basis points, party or self (Gloria; the Soul-path Precision
    /// arm's twin). Folded in `effective_crit_chance_bp` alongside the gear lines.
    /// Append-only.
    CritBpAdd,
    /// +DODGE CHANCE in basis points. Folded in `stat_mods::dodge_bp` as ONE MORE
    /// SOURCE, not an additive bucket — the same inverse-multiplicative rule gear and
    /// the Nimble Mastery passive already use, so a buff cannot push anyone to 100%.
    /// Append-only.
    DodgeBpAdd,
    /// +CRIT DAMAGE in bp, added onto the ×1.5 base (0.0.473, Sylvie's Arc Focus).
    CritDmgBpAdd,
    /// MARKER (0.0.473, Sylvie S1): each crit the carrier lands charges its own Limit Break gauge
    /// by `magnitude`. Read in `tick::crit_riders`.
    CritGauge,
    /// MARKER (0.0.473, Sylvie S4): each crit the carrier lands throws a Thunder spark of
    /// `magnitude` bp of its ATK at the same target. Read in `tick::crit_riders`.
    CritSpark,
    /// MARKER on an enemy (0.0.476, Zolfia's passive): she has struck it. A break while it
    /// carries this turns into `Shattered`. `source_id` = the Zolfia who laid it.
    WinterChill,
    /// DEBUFF (0.0.476): `magnitude` bp more ICE and FIRE damage taken. Laid by a break on a
    /// Winter-Chilled target; `source_id` = the chill's Zolfia.
    Shattered,
    /// MARKER on Zolfia (0.0.476): Remorseless Winter is up — her aura ticks in
    /// `tick::winter_aura` while it lasts.
    WinterAura,
    /// +ATK% from a Wayfarer's Bell ultimate (0.0.485, 3★ Regalia): one instance per Bell
    /// (`source_id` = its wearer), each with its own expiry, and only the STRONGEST counts —
    /// two Bells never add (`StackPolicy::Strongest`). Folded next to `AtkPct`.
    BellAtkPct,
    /// THE SINK for every buff whose stat no longer exists — the six attribute adds and
    /// their percent twins (0.0.317), HIT/FLEE (0.0.333) and the AGI-derived slow. It is
    /// applied by nothing and read by nothing; it exists so a save written before those
    /// cuts still DESERIALIZES. One inert variant instead of twelve dead stat names.
    #[serde(alias = "StrAdd", alias = "AgiAdd", alias = "VitAdd", alias = "IntAdd",
            alias = "DexAdd", alias = "LukAdd", alias = "AllStatAdd", alias = "AgiPct",
            alias = "DexPct", alias = "HitAdd", alias = "FleeAdd", alias = "AgiSlowAdd",
            alias = "HitDownPct")]
    RetiredBuff,
}

/// How multiple live instances of the SAME `BuffKind` on one target COMBINE at read
/// time. Entries are always STORED per `(kind, source_id)` (apply_buff refreshes within
/// a source) — the policy only collapses the READ, so when the strongest mark expires
/// the next-strongest takes over naturally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackPolicy {
    /// Magnitudes SUM across sources — beneficial buffs, and every DoT (each caster's
    /// wound bleeds on its own).
    PerSource,
    /// Only the STRONGEST single instance applies (debuff rule, 2026-07-16: no debuff
    /// stacks except DoTs) — two casters landing the same mark never multiply it.
    Strongest,
}

impl BuffKind {
    /// The read-side stacking rule for this kind: DEBUFFS take the strongest single
    /// mark, EXCEPT DoTs (which tick per source); everything else sums per source.
    /// Existence-check markers (Stun/Freeze/…) are unaffected either way.
    pub fn stack_policy(self) -> StackPolicy {
        if self.is_debuff() && !self.is_dot() || matches!(self, BuffKind::BellAtkPct) {
            StackPolicy::Strongest
        } else {
            StackPolicy::PerSource
        }
    }

    /// Damage-over-time kinds — exempt from the no-stack debuff rule (each source's
    /// DoT ticks independently in `decay_buffs`).
    fn is_dot(self) -> bool {
        matches!(self, BuffKind::PoisonDot | BuffKind::Bleed | BuffKind::LeechBleed | BuffKind::Burn | BuffKind::Poison)
    }

    /// Is this a harmful status / debuff (vs a beneficial buff or neutral marker)?
    /// Used to strip ailments off a fallen ally on death (`tick::resurrection_upkeep`)
    /// so a revived member doesn't come back still crippled. `Hiding` is excluded —
    /// it's a self-protective state, not an enemy-inflicted debuff.
    pub fn is_debuff(self) -> bool {
        matches!(
            self,
            BuffKind::Stun
                | BuffKind::Freeze
                | BuffKind::Blind
                | BuffKind::Poison
                | BuffKind::Bleed
                | BuffKind::Burn
                | BuffKind::DmgTakenPct
                | BuffKind::PoisonDot
                | BuffKind::AgiDexSlowPct
                | BuffKind::DefDownPct
                | BuffKind::Root
                | BuffKind::AspdSlowPct
                | BuffKind::SanctusTerra
                | BuffKind::LumenVuln
                | BuffKind::UmbralMark
                | BuffKind::LeechBleed
                | BuffKind::MarkSlow
                | BuffKind::MarkAspdSlow
                | BuffKind::FrostMark
                | BuffKind::StaticMark
                | BuffKind::BlackwaterMark
                | BuffKind::Dazzle
                | BuffKind::Sunder
                | BuffKind::StaticSpark
                | BuffKind::Darkness
        )
    }
}

/// Combined magnitude of `kind` on `e` under its `StackPolicy` — THE read helper for
/// every magnitude debuff (use `buff_sum` directly only for beneficial buffs, which
/// deliberately sum). 0 when none.
pub fn effective_bp(e: &Entity, kind: BuffKind) -> i32 {
    match kind.stack_policy() {
        StackPolicy::PerSource => buff_sum(e, kind),
        StackPolicy::Strongest => e
            .buffs
            .iter()
            .filter(|b| b.kind == kind)
            .map(|b| b.magnitude)
            .max()
            .unwrap_or(0),
    }
}

/// Total HOLY-element vulnerability (basis points) on `e`. Read by `apply_damage` and
/// applied ONLY to Holy-element damage. 0 for an unmarked target. Strongest-mark policy
/// (never sums); a future holy-vuln from a DIFFERENT skill gets its own kind and ADDS.
pub fn holy_vuln_bp(e: &Entity) -> i32 {
    // Different holy-vuln kinds ADD (each takes its own Strongest mark first).
    effective_bp(e, BuffKind::SanctusTerra)
        .max(0)
        .saturating_add(effective_bp(e, BuffKind::LumenVuln).max(0))
}

/// Number of DISTINCT debuff kinds on `e` (Umbral Exploit counts statuses, not instances).
pub fn distinct_debuff_kinds(e: &Entity) -> usize {
    let mut kinds: Vec<BuffKind> = Vec::new();
    for b in e.buffs.iter().filter(|b| b.kind.is_debuff()) {
        if !kinds.contains(&b.kind) {
            kinds.push(b.kind);
        }
    }
    kinds.len()
}

/// +% Shadow damage taken (Umbral Mark stacks). 0 unmarked.
pub fn shadow_vuln_bp(e: &Entity) -> i32 {
    effective_bp(e, BuffKind::UmbralMark).max(0)
}

/// Extra ICE and FIRE damage taken from Shattered on `e` (0.0.476): the strongest instance.
pub fn shattered_bp(e: &crate::grid::Entity) -> i32 {
    effective_bp(e, BuffKind::Shattered).max(0)
}

/// Extra WATER damage taken from every Frost Mark on `e` (0.0.459).
pub fn water_vuln_bp(e: &Entity) -> i32 {
    effective_bp(e, BuffKind::FrostMark).max(0)
}

/// Extra THUNDER damage taken from every Static Mark on `e` (0.0.462).
pub fn thunder_vuln_bp(e: &Entity) -> i32 {
    effective_bp(e, BuffKind::StaticMark).max(0)
}

/// Base `source_id` of an `ElementDmgDealtPct` buff; the element's tag is added to it so each
/// element keeps its own slot (0.0.469).
pub const ELEMENT_BUFF_SRC: u32 = u32::MAX - 20;

/// `ElementDmgDealtPct` packs `(element, bp)` into one magnitude: element in the high half.
pub const fn pack_element_bp(e: Element, bp: i32) -> i32 {
    (element_to_mag(e) << 16) | (bp & 0xFFFF)
}

/// The +% damage `attacker` deals with a packet of `el`: the flat team bump plus the rolled
/// element's own (0.0.468, Astier).
pub fn dmg_dealt_bp(attacker: &Entity, el: Element) -> i64 {
    let flat = buff_sum(attacker, BuffKind::DmgDealtPct).max(0) as i64;
    let per_element: i64 = attacker
        .buffs
        .iter()
        .filter(|b| matches!(b.kind, BuffKind::ElementDmgDealtPct))
        .filter(|b| mag_to_element(b.magnitude >> 16) == el)
        .map(|b| i64::from(b.magnitude & 0xFFFF))
        .sum();
    flat + per_element
}

/// Encode an `Element` into an `AtkElement` buff's `magnitude`.
pub const fn element_to_mag(e: Element) -> i32 {
    match e {
        Element::Physical => 0,
        Element::Fire => 1,
        Element::Earth => 2,
        Element::Ice => 3,
        Element::Wind => 4,
        Element::Light => 5,
        Element::Dark => 6,
        Element::Poison => 7,
        Element::Thunder => 8,
        Element::Spirit => 9,
    }
}

/// Decode an `AtkElement` buff's `magnitude` back into an `Element`.
pub const fn mag_to_element(m: i32) -> Element {
    match m {
        1 => Element::Fire,
        2 => Element::Earth,
        3 => Element::Ice,
        4 => Element::Wind,
        5 => Element::Light,
        6 => Element::Dark,
        7 => Element::Poison,
        8 => Element::Thunder,
        9 => Element::Spirit,
        _ => Element::Physical,
    }
}

/// One live buff on an entity. Part of the canonical serialization (lives on
/// `Entity.buffs`), so adding fields needs a `VERSION` bump.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Buff {
    pub kind: BuffKind,
    pub magnitude: i32,
    /// Ticks remaining; aged each tick by `tick::decay_buffs`, dropped at 0.
    pub ticks: u32,
    /// Id of the entity that applied it. `apply_buff` refreshes (not stacks) a
    /// buff with the same `(kind, source_id)`, so a re-cast renews duration
    /// rather than piling up.
    pub source_id: u32,
}

/// A buff a skill applies on cast (to self or party). `per_level` lets the
/// magnitude scale with the caster's skill level. Const-constructible so it
/// can sit in a `SkillDef` const.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffSpec {
    pub kind: BuffKind,
    pub magnitude: i32,
    pub per_level: i32,
    pub duration: u32,
}

impl BuffSpec {
    /// Materialize this spec into a live `Buff` for a caster at `level`.
    pub fn at_level(&self, level: u8, source_id: u32) -> Buff {
        let lv = level.saturating_sub(1) as i32;
        Buff {
            kind: self.kind,
            magnitude: self.magnitude + self.per_level * lv,
            ticks: self.duration,
            source_id,
        }
    }
}

/// A debuff a skill applies to its target on hit. The effective chance is
/// `chance_bp + chance_per_level * skill_level`; when that total is 0 it ALWAYS
/// applies WITHOUT a PRNG draw (preserves draw-order for skill-less streams),
/// otherwise one roll decides. `chance_per_level` lets a skill scale its proc
/// chance with level (Sand Strike: blind 7%/level) while its damage stays flat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebuffSpec {
    pub kind: BuffKind,
    pub magnitude: i32,
    pub per_level: i32,
    pub duration: u32,
    pub chance_bp: u32,
    /// Signed so a status can SCALE DOWN with level (RO Storm Gust: freeze 60%
    /// at Lv1 → 15% at Lv10). Effective chance = `(chance_bp + chance_per_level
    /// × level).max(0)`. Positive for the usual ramp-up (Sonic Blow stun).
    pub chance_per_level: i32,
}

impl DebuffSpec {
    pub fn at_level(&self, level: u8, source_id: u32) -> Buff {
        let lv = level.saturating_sub(1) as i32;
        Buff {
            kind: self.kind,
            magnitude: self.magnitude + self.per_level * lv,
            ticks: self.duration,
            source_id,
        }
    }
}

/// Sum the magnitudes of every buff of `kind` on `e`. 0 when none (so a
/// buff-less entity reads exactly as it did before buffs existed).
pub fn buff_sum(e: &Entity, kind: BuffKind) -> i32 {
    e.buffs
        .iter()
        .filter(|b| b.kind == kind)
        .map(|b| b.magnitude)
        .fold(0i32, |a, m| a.saturating_add(m))
}

/// The self weapon-element override (Magnum Break), if any. Last-applied wins.
pub fn buff_atk_element(e: &Entity) -> Option<Element> {
    e.buffs
        .iter()
        .rev()
        .find(|b| b.kind == BuffKind::AtkElement)
        .map(|b| mag_to_element(b.magnitude))
}

/// Apply (or refresh) a buff on `e`. A buff with the same `(kind, source_id)`
/// is replaced in place (renew duration / magnitude) so re-casting doesn't
/// stack; otherwise it's pushed.
pub fn apply_buff(e: &mut Entity, mut buff: Buff) {
    // Gear-mod STATUS RESIST (0.0.134, cap 75%): cut the DURATION of an incoming
    // negative status — deterministic (no block roll, zero extra PRNG draws;
    // deliberate deviation from RO's chance roll). Central point: every source
    // (skill debuffs, monster blasts) applies statuses through here.
    let code = match buff.kind {
        BuffKind::Freeze => Some(crate::combat::stat_mods::STATUS_FREEZE),
        BuffKind::Poison => Some(crate::combat::stat_mods::STATUS_POISON),
        BuffKind::Blind => Some(crate::combat::stat_mods::STATUS_BLIND),
        BuffKind::Stun => Some(crate::combat::stat_mods::STATUS_STUN),
        _ => None,
    };
    if let Some(c) = code {
        let bp = crate::combat::stat_mods::resist_status_bp(e, c);
        if bp != 0 {
            // NEGATIVE = the status sticks LONGER on this target (drawback line).
            buff.ticks = (((buff.ticks as i64) * (10_000 - bp as i64)) / 10_000).max(1) as u32;
        }
    }
    if let Some(slot) = e
        .buffs
        .iter_mut()
        .find(|b| b.kind == buff.kind && b.source_id == buff.source_id)
    {
        *slot = buff;
    } else {
        e.buffs.push(buff);
    }
}

/// The SINGLE fold point for stat-add buffs: clone `e.stats` and apply the
/// `*Add` buff sums (AGI also takes the negative `AgiSlowAdd`). Every derived
/// stat in `combat::derived` reads through this, so one change routes Blessing
/// / Increase AGI / Angelus / Frost-Bolt-slow into ATK, MATK, DEF, HIT, FLEE
/// AND the ASPD formula's `floor(DEX/5)` non-linearity correctly.
///
/// The stat-buff family is RETIRED. StrAdd, AgiAdd, VitAdd, IntAdd, DexAdd,
/// LukAdd, AllStatAdd, AgiPct, DexPct and AgiSlowAdd wrote to the six attributes,
/// and the attributes are gone (0.0.317) — so is `buffed_stats`, the fold point
/// that collected them; callers read `entity.stats` directly.
///
/// The VARIANTS stay in `BuffKind`: it is serialized and its order is append-only,
/// so removing one would break every save. The skills that grant them (Blessing,
/// Agility Up, Sharp Focus, Keen Eye, the food buffs) are being rewritten to grant
/// the stats that exist now.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{Cell, Entity};

    fn unit() -> Entity {
        let e = Entity::idle(1, Cell::new(0, 0));
        e
    }

    #[test]
    fn empty_buffs_are_identity() {
        let e = unit();
        assert_eq!(buff_sum(&e, BuffKind::AtkPct), 0);
        assert!(buff_atk_element(&e).is_none());
    }


    /// 0.0.136: %-of-base rounds AWAY FROM ZERO — a non-zero percent always
    /// moves the stat by at least 1, so Sharp Focus is VISIBLE on small bases
    /// (the user's archer: AGI 16 × 6% used to truncate to +0) and a small
    /// Quicksand slow can't round away to nothing either.

    #[test]
    fn the_retired_buff_sink_moves_nothing() {
        // Twelve kinds wrote to stats that no longer exist (the six attributes, their
        // percent twins, HIT/FLEE, the AGI-derived slow). They deserialize into ONE inert
        // variant now; carrying it must change nothing a fight can see.
        let mut e = unit();
        let before = (crate::combat::effective_atk(&e), crate::combat::effective_max_hp(&e),
                      crate::combat::effective_atk_speed_milli(&e),
                      crate::combat::effective_crit_chance_bp(&e));
        apply_buff(&mut e, Buff { kind: BuffKind::RetiredBuff, magnitude: 50, ticks: 100, source_id: 1 });
        let after = (crate::combat::effective_atk(&e), crate::combat::effective_max_hp(&e),
                     crate::combat::effective_atk_speed_milli(&e),
                     crate::combat::effective_crit_chance_bp(&e));
        assert_eq!(after, before, "the retired sink must not move a single number");
    }

    #[test]
    fn the_two_new_buff_stats_reach_their_readers() {
        let mut e = unit();
        let crit = crate::combat::effective_crit_chance_bp(&e);
        apply_buff(&mut e, Buff { kind: BuffKind::CritBpAdd, magnitude: 500, ticks: 100, source_id: 1 });
        assert_eq!(crate::combat::effective_crit_chance_bp(&e), crit + 500, "Gloria's crit");
        // Dodge folds as a SOURCE: two 50% sources are 75%, never 100%.
        apply_buff(&mut e, Buff { kind: BuffKind::DodgeBpAdd, magnitude: 5_000, ticks: 100, source_id: 1 });
        assert_eq!(crate::combat::stat_mods::dodge_bp(&e), 5_000);
        apply_buff(&mut e, Buff { kind: BuffKind::DodgeBpAdd, magnitude: 5_000, ticks: 100, source_id: 2 });
        assert_eq!(crate::combat::stat_mods::dodge_bp(&e), 7_500, "inverse-multiplicative");
    }

    #[test]
    fn apply_buff_refreshes_same_source_kind() {
        let mut e = unit();
        apply_buff(&mut e, Buff { kind: BuffKind::AtkPct, magnitude: 100, ticks: 5, source_id: 9 });
        apply_buff(&mut e, Buff { kind: BuffKind::AtkPct, magnitude: 100, ticks: 50, source_id: 9 });
        assert_eq!(e.buffs.len(), 1, "same (kind,source) refreshes, not stacks");
        assert_eq!(e.buffs[0].ticks, 50);
        // Different source stacks.
        apply_buff(&mut e, Buff { kind: BuffKind::AtkPct, magnitude: 100, ticks: 5, source_id: 8 });
        assert_eq!(e.buffs.len(), 2);
        assert_eq!(buff_sum(&e, BuffKind::AtkPct), 200);
    }

    #[test]
    fn holy_vuln_takes_the_strongest_mark_never_the_sum() {
        // Two Sanctus Terras (different casters) mark the same target: the field DAMAGE
        // of both lands, but the same-named vuln reads as the STRONGEST single mark.
        let mut e = unit();
        apply_buff(&mut e, Buff { kind: BuffKind::SanctusTerra, magnitude: 2_000, ticks: 45, source_id: 1 });
        apply_buff(&mut e, Buff { kind: BuffKind::SanctusTerra, magnitude: 3_000, ticks: 45, source_id: 2 });
        assert_eq!(e.buffs.len(), 2, "per-source entries coexist");
        assert_eq!(holy_vuln_bp(&e), 3_000, "max, not 5,000");
    }


    #[test]
    fn element_override_roundtrips() {
        let mut e = unit();
        apply_buff(&mut e, BuffSpec {
            kind: BuffKind::AtkElement,
            magnitude: element_to_mag(Element::Fire),
            per_level: 0,
            duration: 30,
        }.at_level(1, 1));
        assert_eq!(buff_atk_element(&e), Some(Element::Fire));
    }
}
