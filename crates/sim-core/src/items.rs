//! Items — definitions, instances, and drop-table rows.
//!
//! Phase 0 keeps items self-contained: each `Item` instance carries its
//! own baked stats. There is no separate "ItemDef catalog" yet — when
//! ALPHA-6 introduces `game_data.json` we'll refactor to (def_id →
//! catalog lookup) + (instance carries only its per-instance state + def_id).
//!
//! Phase 2 economy fields are reserved here (GDD §3.3): `marketable`,
//! `emission_group`, `quota_class`. Phase 0 stores them but does NOT
//! enforce them. Don't strip these out "to simplify" — they're cheap
//! to carry now, expensive to retrofit.

// DEBT (7 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

use serde::{Deserialize, Serialize};

/// serde skip predicate so a 0 `sell_value` is omitted (byte-identity).
fn default_weapon_level() -> u8 { 1 }
fn is_one_u8(n: &u8) -> bool { *n == 1 }
pub(crate) fn is_zero_u8(n: &u8) -> bool { *n == 0 }
fn is_zero_u32(n: &u32) -> bool {
    *n == 0
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ItemKind {
    Weapon,
    Armor,
    /// Crafting material (RESERVED for a future crafting system — NOT
    /// auto-sold). Distinct from `Junk` so players never fear selling a
    /// crafting mat. The `nothing` no-drop sentinel is also this kind.
    Material,
    /// Sellable trash — the Gil source. Drops from monsters, stacks, and is
    /// sold in bulk via `SellAllJunk`. Named "junk" (not "material") so the
    /// player sells freely without hoarding it "for crafting".
    Junk,
    /// Usable item (potion/scroll). Stacks, slotted into a character's 3
    /// consumable slots, and AUTO-USED when its `ConsumableSpec.trigger` fires.
    Consumable,
    /// A CARD (RO-style): a rare monster drop that slots into a socket on
    /// "slotted" gear for a permanent bonus (`card_effect`). Stacks in the stash
    /// until slotted; tradeable (the way-A economy centerpiece).
    Card,
    /// A collectible EXEMPLAR (skin + stat + signature-skill overlay). NOT stackable —
    /// each copy is its own slot and carries its Resonance stars in `resonance_stars`
    /// (equip restores them; merging a copy onto the same equipped Exemplar raises the
    /// rank). The item's `def_id` maps to a code catalog entry via `exemplar_def`.
    Exemplar,
    /// A collectible REGALIA (signature-weapon overlay) — the `Exemplar` twin. Same carrier
    /// semantics (own slot, Resonance stars in `resonance_stars`, `def_id` → `regalia_def`);
    /// lives in `owned_regalia`, account-bound.
    Regalia,
}

/// Maximum Exemplar star rank. 0★ = the base (first copy owned); each duplicate
/// copy merged raises it by one, up to 5★ (six ranks total).
pub const EXEMPLAR_MAX_STARS: u8 = 6;

/// One skill an Exemplar grants — an id (active OR passive; the loadout auto-slots actives) plus the
/// level it's taught at. When the SAME id appears at a higher rank, that entry RE-LEVELS the earlier
/// grant instead of adding a duplicate — so "unlock a new skill" and "level up an existing one" are
/// the same mechanic (list it again with a higher level).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GrantedSkill {
    pub id: String,
    /// Taught level (floored at 1 when materialized so a granted skill is never inert).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub level: u8,
}

/// One STAR RANK's unlock (DEF-side) — the gacha-dupe / Genshin-constellation / HSR-eidolon /
/// ZZZ-mindscape model: reaching `(i+1)★` adds this rank's mods + skills. Ranks are ADDITIVE —
/// being at N★ applies ranks `0..N` cumulatively. An ARBITRARY, PER-EXEMPLAR mix of perks (stat
/// mods and/or ANY NUMBER of skills). Lives ONLY in the def catalog (`exemplar_def`/`regalia_def`) —
/// NEVER serialized into a save (the stored overlay keeps just id+stars and recomputes from the def),
/// so no stat-mod list rides in the hash.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExemplarStar {
    /// Extra stat mods added by THIS rank (additive on top of lower ranks + the base mods).
    pub mods: Vec<StatMod>,
    /// Skills unlocked (or re-leveled) at this rank — as many as you like, active or passive.
    pub skills: Vec<GrantedSkill>,
}

/// An equipped Exemplar (collectible-character overlay) as STORED on an entity/character: a display
/// skin + a star rank, and nothing else. The STAT grant + skills + per-rank Resonance ladder are NOT stored — they
/// are recomputed on demand from `exemplar_def(id)` (the def is the SINGLE SOURCE OF TRUTH; the
/// save/hash carries only this lean identity, so no derived stat data is persisted → no serde-skip
/// combat-field replay hole). Ownership is a server-authored entitlement (character-skins-idea; the
/// store SKU is real-money — price/mint/merge live in the entitlement layer, NEVER in sim-core), but
/// the POWER (via `mods()`) enters sim state → affects combat/hash/replay.
///
/// Star ranks (0..=`EXEMPLAR_MAX_STARS`): a duplicate copy merged raises `stars` by one. Skip-if-
/// default keeps a member WITHOUT an Exemplar byte-identical.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Exemplar {
    pub id: String,
    /// Display name (render convenience; resolved from the def on equip).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// PlayerAnimator SKINS key for the render-side sprite swap.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub skin_id: String,
    /// Current star rank (0..=`EXEMPLAR_MAX_STARS`). Raised by merging duplicate copies.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub stars: u8,
}

/// The AUTHORED Exemplar content (from `exemplar_def`) — the single source of the stat grant, base
/// skills, and per-rank Resonance ladder. NEVER serialized; the stored `Exemplar` recomputes from it by id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExemplarDef {
    /// Bare id ("seraphine") — the key the stored lean `Exemplar` recomputes from.
    pub id: String,
    pub name: String,
    pub skin_id: String,
    /// Base stat mods at 0★.
    pub base_mods: Vec<StatMod>,
    /// Base skills granted from 0★ (active or passive; any number). Higher ranks can re-level these.
    pub base_skills: Vec<GrantedSkill>,
    /// Per-rank Resonance ladder, index `i` = the perk at `(i+1)★`. Ranks `0..stars` apply cumulatively.
    pub star_ranks: Vec<ExemplarStar>,
}

impl ExemplarDef {
    /// The star ranks ACTIVE at `stars` (`0..stars`, clamped to what's defined).
    fn active_ranks(&self, stars: u8) -> &[ExemplarStar] {
        let n = (stars as usize).min(self.star_ranks.len());
        &self.star_ranks[..n]
    }

    /// Total stat mods at `stars`: base ++ every active rank's mods. The aggregators SUM every mod,
    /// so folding is plain concatenation (no per-field add like the old `CardEffect`).
    pub fn mods_at(&self, stars: u8) -> Vec<StatMod> {
        let mut out = self.base_mods.clone();
        for r in self.active_ranks(stars) {
            out.extend_from_slice(&r.mods);
        }
        out
    }

    /// Every granted skill at `stars` as `(id, level)`: base first, then rank 1★→N★ (a repeated id
    /// RE-LEVELS in place). Deterministic insertion order. Feeds `Entity::apply_exemplar_skill`.
    pub fn granted_skills_at(&self, stars: u8) -> Vec<(String, u8)> {
        let mut out: Vec<(String, u8)> = Vec::new();
        let mut apply = |g: &GrantedSkill| {
            if g.id.is_empty() {
                return;
            }
            match out.iter_mut().find(|(id, _)| *id == g.id) {
                Some((_, lv)) => *lv = g.level, // re-level in place
                None => out.push((g.id.clone(), g.level)),
            }
        };
        for g in &self.base_skills {
            apply(g);
        }
        for r in self.active_ranks(stars) {
            for g in &r.skills {
                apply(g);
            }
        }
        out
    }
}

/// The catalog ids that exist. Kept beside the builders — a new Exemplar needs its id here or it
/// will never resolve.


/// BUILT ONCE. The builders below allocate ~20 times per call (three `to_string`s, the base-mod
/// and skill Vecs, and five star ranks each with two more Vecs) to hand back a handful of
/// numbers — and `for_each_mod` has ~24 separate readers, every one of which used to re-run that
/// for every entity it looked at. Measured 2026-07-31: one re-resolving overlay source cost
/// +2.5 µs/tick on a 5-member party; the mods themselves cost nothing (50 extra StatMods on the
/// plain-Vec path measured within noise of zero).
///
/// A `OnceLock` and not a `static` literal because the defs contain `String`/`Vec` fields that
/// `GrantedSkill`'s serde derive needs — making them `&'static str`/`&'static [T]` like
/// `skill_def`'s registry would change a SERIALIZED type. Initialised once and never mutated, so
/// it stays deterministic: every reader sees byte-identical data and lookup is a linear scan, not
/// a hash iteration.
static EXEMPLAR_DEFS: std::sync::OnceLock<Vec<(&'static str, ExemplarDef)>> =
    std::sync::OnceLock::new();
static REGALIA_DEFS: std::sync::OnceLock<Vec<(&'static str, RegaliaDef)>> =
    std::sync::OnceLock::new();

fn exemplar_table() -> &'static [(&'static str, ExemplarDef)] {
    EXEMPLAR_DEFS.get_or_init(|| {
        let mut out: Vec<(&'static str, ExemplarDef)> = Vec::new();
        for line in EXEMPLAR_LINES {
            out.push((exemplar_item_def_id_static(line.id), build_hero_def(line)));
        }
        out
    })
}

fn regalia_table() -> &'static [(&'static str, RegaliaDef)] {
    REGALIA_DEFS.get_or_init(|| {
        let mut out: Vec<(&'static str, RegaliaDef)> = Vec::new();
        for line in REGALIA_LINES {
            for (rarity, id) in line.ids.iter().enumerate().filter(|(_, id)| !id.is_empty()) {
                out.push((regalia_item_def_id_static(id), build_line_def(line, rarity, id)));
            }
        }
        out.push((REGALIA_FODDER_DEF_ID, build_fodder_def()));
        out.push((REGALIA_FODDER_SR_DEF_ID, build_fodder_sr_def()));
        out
    })
}

/// Every Regalia carrier def_id that exists, in table order. The banner, the shop and the
/// catalog-consistency test all need the list; none of them should re-derive it.
pub fn all_regalia_def_ids() -> Vec<&'static str> {
    regalia_table().iter().map(|(k, _)| *k).collect()
}

/// Twin of `all_regalia_def_ids`.
pub fn all_exemplar_def_ids() -> Vec<&'static str> {
    exemplar_table().iter().map(|(k, _)| *k).collect()
}

/// Resolve by ITEM def id ("exemplar_seraphine").
pub fn exemplar_def(item_def_id: &str) -> Option<&'static ExemplarDef> {
    exemplar_table().iter().find(|(k, _)| *k == item_def_id).map(|(_, d)| d)
}

/// Resolve by BARE id ("seraphine") — what the lean `Exemplar` stores. Separate from the item-id
/// lookup so the hot path does not `format!` a temporary String on every call.
/// Unit ids renamed in 0.0.492 to match their display names (Lapis and Raplus since 0.0.467):
/// `(old, new)`. Old saves are moved by `data::migrate_save_items`; an old input log naming the
/// old id is read through `current_unit_id`.
pub const RENAMED_UNIT_IDS: [(&str, &str); 2] = [("rapis", "lapis"), ("nyx", "raplus")];

/// The current id for a unit id an older save or input log may carry; any other id unchanged.
pub fn current_unit_id(id: &str) -> &str {
    RENAMED_UNIT_IDS.iter().find(|(old, _)| *old == id).map_or(id, |(_, new)| new)
}

pub fn exemplar_def_by_id(bare_id: &str) -> Option<&'static ExemplarDef> {
    exemplar_table().iter().find(|(_, d)| d.id == bare_id).map(|(_, d)| d)
}

pub fn regalia_def(item_def_id: &str) -> Option<&'static RegaliaDef> {
    regalia_table().iter().find(|(k, _)| *k == item_def_id).map(|(_, d)| d)
}

pub fn regalia_def_by_id(bare_id: &str) -> Option<&'static RegaliaDef> {
    regalia_table().iter().find(|(_, d)| d.id == bare_id).map(|(_, d)| d)
}

impl Exemplar {
    /// Total stat mods at the current star rank — recomputed from the def (unknown id → empty).
    pub fn mods(&self) -> Vec<StatMod> {
        exemplar_def_by_id(&self.id)
            .map(|d| d.mods_at(self.stars))
            .unwrap_or_default()
    }

    /// Every skill this Exemplar grants at the current star rank — recomputed from the def.
    pub fn granted_skills(&self) -> Vec<(String, u8)> {
        exemplar_def_by_id(&self.id)
            .map(|d| d.granted_skills_at(self.stars))
            .unwrap_or_default()
    }
}

/// The ITEM def_id that carries Exemplar `ex_id` in the inventory (the reverse of
/// `exemplar_def`) — used when unequipping / swapping to rebuild the carrier item.
pub fn exemplar_item_def_id(ex_id: &str) -> String {
    format!("exemplar_{ex_id}")
}

/// EXEMPLAR CATALOG — resolve an Exemplar-kind item's `def_id` to its full definition at
/// 0★ (same "tune in code, move to data later" stance as the skill catalog). The caller
/// overlays the carried star rank from the item. `None` for an unknown id (replay-safe:
/// the equip event just no-ops).
/// ONE hero per row: the row is a UNIT (0.0.338 — it was a class family's `[SR, SSR]` pair before
/// that) and its rarity is a field. `weapon` is the ONE weapon family the character wields
/// (0.0.368, user: "Seraphine เอาดาบอย่างเดียว / rapis เอาแค่คทา / acolyte ถือ staff"); it lives
/// here because a character is the axis the roster is organised on, and the class stat blocks
/// that used to carry it are gone (0.0.445).
struct ExemplarLine {
    weapon: WeaponType,
    /// Index into `EXEMPLAR_LADDER`: 0 = SR, 1 = SSR.
    rarity: usize,
    /// Bare id; the carrier item def_id is `exemplar_<id>`, the unit def (when one ships) is the
    /// bare id itself.
    id: &'static str,
    name: &'static str,
    skin_id: &'static str,
}

/// Each line USED to carry a third axis of its own — the RO attribute that suited the class
/// (thief AGI, archer DEX, casters INT). The attributes are gone (0.0.317), so the third axis is
/// now CRIT DMG for everyone: a stat every character converts, and one the gear pool also rolls,
/// so an Exemplar reads as a strong version of a familiar line rather than its own currency.
///
/// A line whose bare id has no unit def in the catalog (rowena, sable, linnea, pyrra, celestine,
/// noelle, astier) is a PLANNED character: its banner entry and item def exist, but a pull cannot
/// stand it up (`grant_exemplar_unit` returns None) until its kit and stat block ship.
const EXEMPLAR_LINES: &[ExemplarLine] = &[
    ExemplarLine { weapon: WeaponType::Sword1H, rarity: 0, id: "rowena", name: "Rowena", skin_id: "swordsman" },
    ExemplarLine { weapon: WeaponType::Sword1H, rarity: 1, id: "seraphine", name: "Seraphine", skin_id: "placeholder" },
    ExemplarLine { weapon: WeaponType::Dagger, rarity: 0, id: "sable", name: "Sable", skin_id: "thief" },
    // Raplus (0.0.464 display name; id `nyx` until 0.0.492, which moved it to `raplus` —
    // `migrate_unit_ids` renames it in old saves).
    ExemplarLine { weapon: WeaponType::Dagger, rarity: 1, id: "raplus", name: "Raplus", skin_id: "thief" },
    ExemplarLine { weapon: WeaponType::Bow, rarity: 0, id: "linnea", name: "Linnea", skin_id: "archer" },
    // Aveline is the first SR with a real kit (0.0.452, user 2026-09-21): SR base stats, her
    // skills at ~75% of the SSR numbers, a +45% Resonance ladder — S6 ≈ 95% of an SSR Striker's S0.
    ExemplarLine { weapon: WeaponType::Bow, rarity: 0, id: "aveline", name: "Aveline", skin_id: "archer" },
    ExemplarLine { weapon: WeaponType::Staff, rarity: 0, id: "pyrra", name: "Pyrra", skin_id: "mage" },
    ExemplarLine { weapon: WeaponType::Staff, rarity: 1, id: "celestine", name: "Celestine", skin_id: "mage" },
    // Rapis (0.0.441): the Fire unit is a PULL like every other character (user 2026-09-17:
    // "ตัวละครเข้าทาง gacha อย่างเดียว").
    ExemplarLine { weapon: WeaponType::Staff, rarity: 1, id: "lapis", name: "Lapis", skin_id: "mage" },
    // Nemu (0.0.451): the Dark staff Striker — succubus tone; art key mage until she has a sheet.
    ExemplarLine { weapon: WeaponType::Staff, rarity: 1, id: "nemu", name: "Nemu", skin_id: "mage" },
    // Lumi (0.0.456): the SR Ice Sentinel — sword and shield, the timid one; art key swordsman.
    ExemplarLine { weapon: WeaponType::Sword1H, rarity: 0, id: "lumi", name: "Lumi", skin_id: "swordsman" },
    // Airi (0.0.462): the SR Thunder Striker — bow and quiver, Lumi's loud partner; art key archer.
    ExemplarLine { weapon: WeaponType::Bow, rarity: 0, id: "airi", name: "Airi", skin_id: "archer" },
    // Charon (0.0.463): the SSR Dark Destroyer — mace and trinket, the ferryman; art key swordsman.
    ExemplarLine { weapon: WeaponType::Mace, rarity: 1, id: "charon", name: "Charon", skin_id: "swordsman" },
    // Suzu + Mirei (0.0.465): the SR pair built for Laplus' team — she breaks, she charges.
    ExemplarLine { weapon: WeaponType::Sword1H, rarity: 0, id: "suzu", name: "Suzu", skin_id: "swordsman" },
    ExemplarLine { weapon: WeaponType::Staff, rarity: 0, id: "mirei", name: "Mirei", skin_id: "mage" },
    ExemplarLine { weapon: WeaponType::Staff, rarity: 0, id: "noelle", name: "Noelle", skin_id: "acolyte" },
    // Lucielle carries a staff too (user: "acolyte ถือ staff") — her long cross-staff.
    ExemplarLine { weapon: WeaponType::Staff, rarity: 0, id: "lucielle", name: "Lucielle", skin_id: "acolyte" },
    // Support x Water (docs/exemplar-roster.md §10). The second Support slot: Lucielle is
    // Support x Holy and nothing else, so a party wanting a healer had to bring Holy with it,
    // which costs an element slot now that break is gated on each monster's weakness list.
    ExemplarLine { weapon: WeaponType::Staff, rarity: 1, id: "astier", name: "Astier", skin_id: "acolyte" },
    // Sylvie (0.0.473): the SR Thunder Support — staff, a pure buffer (crit + attack speed).
    ExemplarLine { weapon: WeaponType::Staff, rarity: 0, id: "sylvie", name: "Sylvie", skin_id: "mage" },
    // Zolfia (0.0.476): the SSR Ice Destroyer — the Dulahaness won to the party; sword and shield.
    ExemplarLine { weapon: WeaponType::Sword1H, rarity: 1, id: "zolfia", name: "Zolfia", skin_id: "swordsman" },
];

/// SR / SSR budget. No HP line — that axis belongs to the Regalia, so the two overlays add up to
/// one character rather than to two copies of the same thing.
///
/// Stars are the ONLY growth axis an Exemplar has: a Regalia also takes Enhancement levels from
/// fodder, while an Exemplar moves only on duplicates, which is the expensive half of the gacha.
/// So a player holding one copy holds a fixed number forever. That asymmetry is real and worth
/// closing later with an Exemplar level axis; the budget here is base + stars and would have to
/// be RE-SPLIT if levels land, not topped up.
struct ExemplarTier {
    /// CRIT DMG in basis points (100 = +1%) — the third axis, see `EXEMPLAR_LINES`.
    crit_dmg_bp: i32,
    flat: i32,
    /// bp: 500 = +5%. Fixed per rarity — same rule as the Regalia ladder, stars never move it.
    pct_bp: i32,
    per_star_crit_dmg_bp: i32,
    per_star_flat: i32,
}

/// At 5 stars: SR = +10% crit DMG / +50 flat / +5%; SSR = +16% crit DMG / +80 flat / +8%.
///
/// Deliberately close to a maxed Regalia rather than above it, even though a character copy costs
/// roughly three times a weapon copy on the planned banners. As pure stat sticks these two SHOULD
/// look similar — the character's premium has to be paid for by its SKILLS and skin, and if it is
/// ever paid for by raw numbers instead, the weapon banner becomes the only rational pull.
const EXEMPLAR_LADDER: [ExemplarTier; 2] = [
    // SR
    ExemplarTier { crit_dmg_bp: 500, flat: 25, pct_bp: 500, per_star_crit_dmg_bp: 100, per_star_flat: 5 },
    // SSR
    ExemplarTier { crit_dmg_bp: 800, flat: 40, pct_bp: 800, per_star_crit_dmg_bp: 160, per_star_flat: 8 },
];

/// An Exemplar's rarity rung: 0 = SR, 1 = SSR; `None` for an unknown id. The Combat Score reads
/// it; nothing in combat does (the ladder is already folded into the def's mods).
pub fn exemplar_rarity_index(bare_id: &str) -> Option<usize> {
    EXEMPLAR_LINES.iter().find(|l| l.id == bare_id).map(|l| l.rarity)
}

/// A Regalia's rarity rung: 0 = R, 1 = SR, 2 = SSR; `None` for an unknown id.
pub fn regalia_rarity_index(bare_id: &str) -> Option<usize> {
    if bare_id.is_empty() {
        return None;
    }
    REGALIA_LINES.iter().find_map(|l| l.ids.iter().position(|id| *id == bare_id))
}

/// Every bare Regalia id the catalog knows, in line order (the server's pools are bound to it).
pub fn regalia_all_ids() -> Vec<&'static str> {
    REGALIA_LINES.iter().flat_map(|l| l.ids.iter().copied()).filter(|id| !id.is_empty()).collect()
}

/// The `'static` spelling of a bare Regalia id the catalog knows, `None` for a stranger.
pub fn regalia_static_id(bare_id: &str) -> Option<&'static str> {
    if bare_id.is_empty() {
        return None;
    }
    REGALIA_LINES.iter().flat_map(|l| l.ids.iter()).find(|id| **id == bare_id).copied()
}

fn exemplar_item_def_id_static(bare_id: &str) -> &'static str {
    Box::leak(format!("exemplar_{bare_id}").into_boxed_str())
}

fn build_hero_def(line: &ExemplarLine) -> ExemplarDef {
    let t = &EXEMPLAR_LADDER[line.rarity];
    let id = line.id;
    // ONE offense pool since 0.0.330 — MATK is ATK — so every line feeds the same pair.
    let (flat, pct): (fn(i32) -> StatMod, fn(i32) -> StatMod) = (StatMod::AtkFlat, StatMod::AtkBp);
    let (base_skills, star_skills) = authored_skills(id);
    ExemplarDef {
        id: id.to_string(),
        name: line.name.to_string(),
        skin_id: line.skin_id.to_string(),
        base_mods: vec![StatMod::CritDmgBp(t.crit_dmg_bp), flat(t.flat), pct(t.pct_bp)],
        base_skills,
        star_ranks: (0..EXEMPLAR_MAX_STARS as usize)
            .map(|i| ExemplarStar {
                mods: vec![StatMod::CritDmgBp(t.per_star_crit_dmg_bp), flat(t.per_star_flat)],
                skills: star_skills.get(i).cloned().unwrap_or_default(),
            })
            .collect(),
    }
}

/// The two heroes that shipped BEFORE the ladder keep their hand-authored kits. Everything else is
/// a stat stick for now — skills are the next slice, and they are what will justify a character
/// costing more than a weapon.
fn authored_skills(id: &str) -> (Vec<GrantedSkill>, Vec<Vec<GrantedSkill>>) {
    let sk = |id: &str, level: u8| GrantedSkill { id: id.to_string(), level };
    // RESONANCE S1..S6 (0.0.441, Genshin's constellations): every rank is a mechanic in
    // `unit_rank::ranks_of` — the S3 / S5 level ranks included, read at power time so they sit
    // ABOVE the board's cap instead of vanishing under it. The overlay only lifts the kit to Lv5
    // at S0; the ladder is six deep so a sixth copy is a rank, not a refund.
    let kit = |s1: &str, s2: &str, ult: &str| -> (Vec<GrantedSkill>, Vec<Vec<GrantedSkill>>) {
        (vec![sk(s1, 5), sk(s2, 5), sk(ult, 5)], vec![vec![]; 6])
    };
    match id {
        "seraphine" => kit("iron_chains", "steel_break", "aegis"),
        "lapis" => kit("fireball", "ember_cascade", "fire_cat_meteor"),
        "lucielle" => kit("lux_medica", "lumen_contagio", "benedictio_lucis"),
        "aveline" => kit("nightfall_volley", "shadowbind", "eclipse_rain"),
        "raplus" => kit("veil_slash", "rend", "umbral_requiem"),
        "nemu" => kit("velvet_bog", "heartstring", "abyssal_embrace"),
        "lumi" => kit("frost_taunt", "ice_mend", "frost_aura"),
        "airi" => kit("chain_bolt", "overload", "storm_volley"),
        "charon" => kit("tolling_blow", "blackwater_mark", "passage_of_no_return"),
        "suzu" => kit("sunder", "bell_strike", "break_fever"),
        "mirei" => kit("storm_field", "quicken", "thunderfall"),
        "sylvie" => kit("surge", "arc_focus", "bolt_from_the_blue"),
        "zolfia" => kit("coldflame_split", "remorseless_winter", "winters_verdict"),
        // Astier (no unit yet): all three actives re-level +1 per star; Unda Vitae is flat.
        "astier" => (
            vec![sk("unda_vitae", 1), sk("rivus", 5), sk("tidebless", 5), sk("fons_vitae", 5)],
            (6..=10)
                .map(|lv| vec![sk("rivus", lv), sk("tidebless", lv), sk("fons_vitae", lv)])
                .collect(),
        ),
        _ => (vec![], vec![]),
    }
}


// ==================== REGALIA (signature-weapon OVERLAY) ====================
// A Regalia is the WEAPON analog of an Exemplar: a bound, account-only overlay in its OWN slot
// (a member can wear BOTH an Exemplar and a Regalia). It reuses the Exemplar machinery verbatim —
// a `CardEffect` stat grant + `GrantedSkill`s + a per-rank `ExemplarStar` Resonance ladder — with
// `signature_anim` (an attack-clip key) in place of the Exemplar's `skin_id`. Power enters sim
// state → affects combat/hash/replay; ownership/gacha live in the entitlement layer, never here.
// P0: overlay mechanic + placeholder stats from the CURRENT `CardEffect` vocabulary (atk_pct /
// crit_bp / flat). The designed headline stats (CritDmg / MatkPct / element-dmg% / heal) need a
// `CardEffect` extension — a later phase. See docs/regalia-overlay-design.md.

/// Regalia Resonance runs S1..S5 (0.0.486, user 2026-09-27: "ให้เริ่มที่ S1 … Max ที่ S5"): a fresh
/// card is S1 = stars 0, and four duplicates reach S5 = stars 4. The passive's S5 is twice its S1.
pub const REGALIA_MAX_STARS: u8 = 4;
/// Cap on the Regalia LEVEL (0.0.400 — ZZZ's W-Engine Lv60; was an Enhancement of five). The lump
/// and the advanced stat both scale with it (`regalia_lump_at` / `regalia_advanced_at`); fodder
/// feeds it (`regalia_fodder_levels`). ⚠️ The fodder rates are provisional until the gacha
/// economy design prices them (`docs/gacha-economy-design.md`).
pub const REGALIA_MAX_LEVEL: u8 = 70;
/// End of the base curve (0.0.490, user 2026-09-29: "ตันเลเวล 70 เหมือนตัวละคร … 60→70 steep มากๆ",
/// "ลองสัก 20m"). Lv0-60 climbs the ZZZ-shaped curves below; Lv61-70 is the late sink, walking in a
/// straight line from the Lv60 value to the AUTHORED Lv70 value (`REGALIA_LV70`), and the XP it
/// costs is `REGALIA_OVERCAP_XP`.
pub const REGALIA_BASE_CAP: u8 = 60;

/// The Lv70 value of every tier stat, AUTHORED (user 2026-09-29: "พลังควร predetermine ไม่ใช่ +% จะ
/// บาลานซ์ยาก"), keyed by the Lv1 value it belongs to in `REGALIA_LADDER`: the lump (R / SR / SSR),
/// then the big and small advanced stats. ~+15-20% over Lv60, rounded. A test binds every tier
/// value to a row here.
const REGALIA_LV70: [(i32, i32); 12] = [
    (100, 1_800), (140, 2_500), (200, 3_600),   // lump: ATK
    (360, 1_050), (620, 1_800), (880, 2_600),   // advanced, big (crit dmg / HP% / element%): bp
    (150, 430), (260, 750), (380, 1_100),       // advanced, small (crit / ATK%): bp
    (75, 215), (130, 375), (190, 550),          // advanced, CDR (half the small size): bp
];

/// The authored Lv70 value for a tier stat whose Lv1 value is `lv1`; `None` off the table.
pub fn regalia_lv70(lv1: i32) -> Option<i32> {
    REGALIA_LV70.iter().find(|(a, _)| *a == lv1).map(|(_, b)| *b)
}

/// Lv61-70: the straight line from `v60` to the authored Lv70 value. Off the table (a test value,
/// never a shipped tier) the stat holds its Lv60 value rather than inventing a growth.
fn regalia_past_60(lv1: i32, v60: i64, level: u8) -> i64 {
    let past = i64::from(level.min(REGALIA_MAX_LEVEL).saturating_sub(REGALIA_BASE_CAP));
    let span = i64::from(REGALIA_MAX_LEVEL - REGALIA_BASE_CAP);
    match regalia_lv70(lv1) {
        Some(v70) if past > 0 => v60 + (i64::from(v70) - v60) * past / span,
        _ => v60,
    }
}
/// The lump's growth per Regalia level, bp compounded: ×15 from Lv1 to Lv60, ZZZ's own ratio
/// (Steel Cushion: 46 → 684 base ATK).
pub const REGALIA_LUMP_PER_LEVEL_BP: i64 = 470;
/// The advanced stat's growth per level, bp of its Lv1 value, linear: ×2.5 at Lv60 (ZZZ: 9.6% →
/// 24% crit rate on an S-rank).
pub const REGALIA_ADVANCED_PER_LEVEL_BP: i64 = 254;

/// The lump at Regalia `level` (0 and 1 are both the floor).
pub fn regalia_lump_at(lump_base: i32, level: u8) -> i32 {
    // Compounded in thousandths so 59 floors do not eat a tenth of the top (100 → 1,298 instead
    // of 1,500 when each step floors a whole number).
    let mut v = i64::from(lump_base) * 1_000;
    for _ in 1..level.clamp(1, REGALIA_BASE_CAP) {
        v = v * (10_000 + REGALIA_LUMP_PER_LEVEL_BP) / 10_000;
    }
    let v60 = (v + 500) / 1_000;
    regalia_past_60(lump_base, v60, level).min(i64::from(i32::MAX)) as i32
}

/// The advanced stat's value at Regalia `level`.
pub fn regalia_advanced_at(base_bp: i32, level: u8) -> i32 {
    let steps = i64::from(level.clamp(1, REGALIA_BASE_CAP) - 1);
    let v60 = i64::from(base_bp) * (10_000 + REGALIA_ADVANCED_PER_LEVEL_BP * steps) / 10_000;
    regalia_past_60(base_bp, v60, level).min(i64::from(i32::MAX)) as i32
}

/// A Regalia (signature-weapon overlay) as STORED on an entity/character — the lean identity only:
/// id + Resonance stars + Enhancement level. The STAT grant + skills + per-rank ladder are NOT stored;
/// they are recomputed on demand from `regalia_def(id)` (twin of the lean `Exemplar`; def = single
/// source, so no stat-mod list rides in the save/hash → no replay hole).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Regalia {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// PlayerAnimator signature attack-clip key (render-side; the flair hook, P1).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature_anim: String,
    /// Current Resonance rank (0..=`REGALIA_MAX_STARS`). Raised by merging duplicate copies.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub stars: u8,
    /// Current Enhancement LEVEL (0..=`REGALIA_MAX_LEVEL`). Raised by feeding R fodder; scales the
    /// main/flat mods by `per_level`. The SECOND axis (Level = stats; Resonance = passive/ranks).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub level: u8,
}

/// The AUTHORED Regalia content (from `regalia_def`) — NEVER serialized; the stored `Regalia`
/// recomputes from it by id. `per_level_mods` scales with the Enhancement level.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegaliaDef {
    /// Bare id ("aurora_edge") — the key the stored lean `Regalia` recomputes from.
    pub id: String,
    pub name: String,
    pub signature_anim: String,
    /// The ATK LUMP at Lv1 (0.0.400, ZZZ's W-Engine base ATK) — a FIXED stat of the piece that
    /// grows with the Regalia's OWN level (user 2026-09-16: "ก้อนไม่ได้ scale ตามผู้สวม … scale
    /// ตาม level ของ regalia"). 0 = none (fodder).
    pub lump_base: i32,
    /// The ADVANCED stat at Lv1 — one fixed line per piece (crit / crit dmg / ATK% / CDR / HP% by
    /// line), the W-Engine's second number; grows with the level too. `None` = fodder.
    pub advanced: Option<StatMod>,
    pub base_skills: Vec<GrantedSkill>,
    /// PASSIVE per Resonance rank — EMPTY on every line today (user 2026-09-16: "passive ว่างไว้
    /// ก่อน"); the ladder stays so a rank can be authored later without a shape change.
    pub star_ranks: Vec<ExemplarStar>,
}

impl RegaliaDef {
    fn active_ranks(&self, stars: u8) -> &[ExemplarStar] {
        let n = (stars as usize).min(self.star_ranks.len());
        &self.star_ranks[..n]
    }

    /// The lines at Regalia `level` and Resonance `stars`: the lump and the advanced stat at
    /// that level, plus whatever the (empty) rank ladder adds.
    pub fn mods_at(&self, level: u8, stars: u8) -> Vec<StatMod> {
        let mut out = Vec::new();
        if self.lump_base > 0 {
            out.push(StatMod::AtkFlat(regalia_lump_at(self.lump_base, level)));
        }
        if let Some(adv) = self.advanced {
            out.push(adv.with_value(regalia_advanced_at(adv.value(), level)));
        }
        for r in self.active_ranks(stars) {
            out.extend_from_slice(&r.mods);
        }
        out.extend(crate::regalia_passive::flat_mods(&self.id, stars));
        out
    }

    /// Every granted skill at `stars` as `(id, level)` — base then rank 1★→N★, repeated id re-levels.
    pub fn granted_skills_at(&self, stars: u8) -> Vec<(String, u8)> {
        let mut out: Vec<(String, u8)> = Vec::new();
        let mut apply = |g: &GrantedSkill| {
            if g.id.is_empty() {
                return;
            }
            match out.iter_mut().find(|(id, _)| *id == g.id) {
                Some((_, lv)) => *lv = g.level,
                None => out.push((g.id.clone(), g.level)),
            }
        };
        for g in &self.base_skills {
            apply(g);
        }
        for r in self.active_ranks(stars) {
            for g in &r.skills {
                apply(g);
            }
        }
        out
    }
}

impl Regalia {
    /// Total stat mods at the current Level + Resonance — recomputed from the def (unknown → empty).
    pub fn mods(&self) -> Vec<StatMod> {
        regalia_def_by_id(&self.id)
            .map(|d| d.mods_at(self.level, self.stars))
            .unwrap_or_default()
    }

    /// Every granted skill at the current Resonance — recomputed from the def.
    pub fn granted_skills(&self) -> Vec<(String, u8)> {
        regalia_def_by_id(&self.id)
            .map(|d| d.granted_skills_at(self.stars))
            .unwrap_or_default()
    }
}

/// The ITEM def_id that carries Regalia `id` in the owned collection (reverse of `regalia_def`).
pub fn regalia_item_def_id(id: &str) -> String {
    format!("regalia_{id}")
}

/// REGALIA CATALOG — resolve a Regalia-carrier item's `def_id` to its authored def. `None` for an
/// unknown id (replay-safe: the equip event no-ops). STAGE 2 keeps the P0 PLACEHOLDER stats (now in
/// `StatMod` vocabulary); the real design headline stats (CritDmg / MatkPct / Holy-DMG-dealt / heal)
/// + layered passives land in stage 3. See docs/regalia-overlay-design.md.
/// One class family's signature weapon, in three rarities (R, SR, SSR).
///
/// A TABLE and not a `match` arm per weapon: there were 3 hand-written arms and five classes
/// times three rarities is 15, at which point a balance pass means editing fifteen places and
/// a new class means remembering five. Adding a class is one row here; re-balancing an entire
/// rarity is one row in `REGALIA_LADDER`.
struct RegaliaLine {
    /// The DOCTRINE the line is built for (0.0.436 — the class system is retired). Anyone
    /// equips anything and takes the stats; only the PASSIVE asks for the doctrine.
    doctrine: crate::potential::Doctrine,
    /// Bare ids in rarity order; the carrier item def_id is `regalia_<id>`. An EMPTY string is
    /// a rung that does not exist (0.0.462, user: "สร้างเฉพาะของตัวละครก่อนทีละอัน … SR ก็สร้าง Regalia
    /// SR เฉพาะตัวนี้พอ") — a unit's line is built one piece at a time, at the unit's own rarity.
    ids: [&'static str; 3],
    names: [&'static str; 3],
    /// The line's ADVANCED stat (the same on all three tiers; the tier sets its size).
    advanced: crate::substats::SubstatKey,
}

/// Every weapon is named for itself. An earlier pass gave each family a shared noun with an
/// escalating qualifier (Squire's Edge / Vigil Edge / Aurora Edge) and it read as one weapon's
/// evolution chain rather than three artifacts you could want separately (user: "ชื่อใช้คำซ้ำเยอะไป
/// เหมือนวิวัฒนาการ"). Rarity escalates through MEANING instead — domestic, then weather and
/// wildlife, then the sky and the Court — so nothing has to repeat a word to place itself.
/// `aurora_edge` and `litany_of_dawn` shipped before the ladder and keep their names as the SSR
/// of their own lines.
const REGALIA_LINES: &[RegaliaLine] = &[
    RegaliaLine {
        doctrine: crate::potential::Doctrine::Sentinel,
        ids: ["hearthguard", "", "aurora_edge"],
        names: ["Hearthguard", "", "Aurora Edge"],
        advanced: crate::substats::SubstatKey::CritDmg,
    },
    RegaliaLine {
        doctrine: crate::potential::Doctrine::Catalyst,
        ids: ["cutpurse", "", "silent_verdict"],
        names: ["Cutpurse", "", "Silent Verdict"],
        advanced: crate::substats::SubstatKey::Crit,
    },
    RegaliaLine {
        doctrine: crate::potential::Doctrine::Striker, // Aveline is a Striker (0.0.440); no Destroyer unit yet
        ids: ["kestrel", "", "meridian"],
        names: ["Kestrel", "", "Meridian"],
        advanced: crate::substats::SubstatKey::AtkPct,
    },
    // Nemu's line (0.0.451): a Striker piece keyed on APPLYING debuffs — any Striker that
    // lays statuses (Rapis burns, Aveline marks, Nemu everything) climbs it.
    // Lumi's line (0.0.456): a Sentinel piece keyed on CONTROL — a taunt held guards the party,
    // every status the wearer lays hardens it. Seraphine's chains and taunt walk into both.
    RegaliaLine {
        doctrine: crate::potential::Doctrine::Sentinel,
        ids: ["", "hoarfrost_crown", ""],
        names: ["", "Hoarfrost Crown", ""],
        advanced: crate::substats::SubstatKey::HpPct,
    },
    RegaliaLine {
        doctrine: crate::potential::Doctrine::Striker,
        ids: ["rouge_thread", "", "lycoris_lullaby"],
        // SSR renamed for its art (user 2026-09-27, 0.0.487): Nemu asleep in a field of red spider
        // lilies. Was `crimson_mirror` / Crimson Mirror (`DEF_ID_REMAP` carries an old save).
        names: ["Rouge Thread", "", "Lycoris Lullaby"],
        advanced: crate::substats::SubstatKey::CritDmg,
    },
    // Astier's line (0.0.468): the second Support piece — SSR rung only, keyed on HEALING.
    RegaliaLine {
        doctrine: crate::potential::Doctrine::Support,
        ids: ["", "", "tidecaller"],
        names: ["", "", "Tidecaller"],
        advanced: crate::substats::SubstatKey::HpPct,
    },
    // Zolfia's line (0.0.476): the third Destroyer piece — SSR rung only, keyed on BREAKING.
    RegaliaLine {
        doctrine: crate::potential::Doctrine::Destroyer,
        ids: ["", "", "winters_oath"],
        names: ["", "", "Winter's Oath"],
        advanced: crate::substats::SubstatKey::AtkPct,
    },
    // Sylvie's line (0.0.473): the third Support piece — SR rung only, keyed on CRIT.
    RegaliaLine {
        doctrine: crate::potential::Doctrine::Support,
        ids: ["", "static_charm", ""],
        names: ["", "Static Charm", ""],
        advanced: crate::substats::SubstatKey::Crit,
    },
    // Suzu's line (0.0.465): the second Destroyer piece — SR rung only, keyed on BREAKING.
    RegaliaLine {
        doctrine: crate::potential::Doctrine::Destroyer,
        ids: ["", "emberbell", ""],
        names: ["", "Emberbell", ""],
        advanced: crate::substats::SubstatKey::AtkPct,
    },
    // Mirei's line (0.0.465): a Catalyst piece keyed on APPLYING statuses — SR rung only.
    RegaliaLine {
        doctrine: crate::potential::Doctrine::Catalyst,
        ids: ["", "stormglass_pin", ""],
        names: ["", "Stormglass Pin", ""],
        advanced: crate::substats::SubstatKey::AtkPct,
    },
    // Charon's line (0.0.463): the FIRST Destroyer piece — SSR rung only, keyed on BREAKING.
    RegaliaLine {
        doctrine: crate::potential::Doctrine::Destroyer,
        ids: ["", "", "ferrymans_lantern"],
        names: ["", "", "Ferryman's Lantern"],
        advanced: crate::substats::SubstatKey::Crit,
    },
    // Airi's line (0.0.462): a Striker piece keyed on FINISHING — a target under half takes
    // more, which her chain and storm (lowest-HP targeting) walk into every fight.
    RegaliaLine {
        doctrine: crate::potential::Doctrine::Striker,
        ids: ["", "thunderhead", ""],
        names: ["", "Thunderhead", ""],
        advanced: crate::substats::SubstatKey::AtkPct,
    },
    RegaliaLine {
        doctrine: crate::potential::Doctrine::Striker,
        ids: ["emberwick", "", "empyrean"],
        names: ["Emberwick", "", "Empyrean"],
        advanced: crate::substats::SubstatKey::Cdr,
    },
    RegaliaLine {
        doctrine: crate::potential::Doctrine::Support,
        ids: ["vesper", "", "litany_of_dawn"],
        names: ["Vesper", "", "Litany of Dawn"],
        advanced: crate::substats::SubstatKey::HpPct,
    },
    // THE 3★ SET (0.0.485, docs/regalia-three-star-backend-handoff-2026-09-27.md): ten R cards,
    // two per Doctrine — the six rows above keep their R rung, these four are R-only.
    RegaliaLine {
        doctrine: crate::potential::Doctrine::Sentinel,
        ids: ["last_rivet", "", ""],
        names: ["Last Rivet", "", ""],
        advanced: crate::substats::SubstatKey::HpPct,
    },
    RegaliaLine {
        doctrine: crate::potential::Doctrine::Support,
        ids: ["wayfarers_bell", "", ""],
        names: ["Wayfarer's Bell", "", ""],
        advanced: crate::substats::SubstatKey::HpPct,
    },
    RegaliaLine {
        doctrine: crate::potential::Doctrine::Destroyer,
        ids: ["faultline", "", ""],
        names: ["Faultline", "", ""],
        advanced: crate::substats::SubstatKey::AtkPct,
    },
    RegaliaLine {
        doctrine: crate::potential::Doctrine::Destroyer,
        ids: ["striking_stone", "", ""],
        names: ["Striking Stone", "", ""],
        advanced: crate::substats::SubstatKey::Crit,
    },
];

/// The ten 3★ (R) cards (0.0.485). A gacha R pull pays one of these; salvaged, each is one
/// Memory Dust (`regalia_salvage_yield`).
pub const REGALIA_THREE_STAR: [&str; 10] = [
    "hearthguard", "last_rivet", "kestrel", "emberwick", "cutpurse",
    "rouge_thread", "vesper", "wayfarers_bell", "faultline", "striking_stone",
];

struct RegaliaTier {
    /// The ATK lump at Lv1 (×15 by Lv60).
    lump: i32,
    /// The advanced stat's Lv1 size in bp for the LINE's stat: damage-shaped stats (crit dmg,
    /// HP%) use `adv_big`, rate-shaped ones (crit, ATK%, CDR) `adv_small` (×2.5 by Lv60).
    adv_big: i32,
    adv_small: i32,
}

/// R / SR / SSR (0.0.400, W-Engine shape; ZZZ at Lv60: B 290-330 / A 430-480 / S 580-680 base ATK,
/// advanced 6-10% / 12-18% / 18-24%). Sized against OUR gear: an SSR at Lv60 is 3,000 ATK beside
/// a Beyond s7 weapon's 7,361 + a Lv70 base of 770 — about 27% of the character; an R at Lv60 is
/// 1,500. Rarity is 1 : 1.4 : 2 on the lump and 0.4 : 0.7 : 1 on the advanced stat — the tier is
/// the piece; no level turns an R into an SSR.
const REGALIA_LADDER: [RegaliaTier; 3] = [
    RegaliaTier { lump: 100, adv_big: 360, adv_small: 150 },   // R
    RegaliaTier { lump: 140, adv_big: 620, adv_small: 260 },   // SR
    RegaliaTier { lump: 200, adv_big: 880, adv_small: 380 },   // SSR
];

/// `"regalia_<id>"` as a `&'static str`. The table is built once into a `OnceLock`, so leaking a
/// small String per line is a one-time cost that buys `&'static` keys the lookups already expect.
fn regalia_item_def_id_static(bare_id: &str) -> &'static str {
    Box::leak(regalia_item_def_id(bare_id).into_boxed_str())
}

fn build_line_def(line: &RegaliaLine, rarity: usize, id: &'static str) -> RegaliaDef {
    use crate::substats::SubstatKey as K;
    let t = &REGALIA_LADDER[rarity.min(2)];
    // CDR lines carry HALF the small size (0.0.490, user 2026-09-29: every CDR source halved — the
    // sources summed to ~80% against the 30% clamp).
    let size = match line.advanced {
        K::CritDmg | K::HpPct => t.adv_big,
        K::Cdr => t.adv_small / 2,
        _ => t.adv_small,
    };
    // The SIGNATURE piece (0.0.435) carries its unit's DOCTRINE stat instead of the line's —
    // ZZZ's per-role advanced stat: Sentinel DEF%, Striker element DMG, Support HP%, Destroyer
    // crit, Catalyst ATK%. The line's own stat stays on the R/SR rungs.
    let advanced = match id {
        "aurora_edge" => StatMod::DefBp(t.adv_big),
        "empyrean" => StatMod::ElementDmgBp(crate::grid::Element::Fire, t.adv_big),
        "litany_of_dawn" => StatMod::MaxHpBp(t.adv_big),
        "meridian" => StatMod::CritBp(t.adv_small),
        "silent_verdict" => StatMod::AtkBp(t.adv_small),
        "lycoris_lullaby" => StatMod::ElementDmgBp(crate::grid::Element::Dark, t.adv_big),
        "hoarfrost_crown" => StatMod::DefBp(t.adv_big),
        "thunderhead" => StatMod::ElementDmgBp(crate::grid::Element::Thunder, t.adv_big),
        "ferrymans_lantern" => StatMod::CritBp(t.adv_small),
        "emberbell" => StatMod::ElementDmgBp(crate::grid::Element::Fire, t.adv_big),
        "stormglass_pin" => StatMod::AtkBp(t.adv_small),
        "tidecaller" => StatMod::MaxHpBp(t.adv_big),
        "static_charm" => StatMod::CritBp(t.adv_small),
        "winters_oath" => StatMod::ElementDmgBp(crate::grid::Element::Ice, t.adv_big),
        // 3★ set (0.0.485): the R rungs whose Secondary Mod is not their line's. Only these ids
        // — the SR / SSR rungs of the same rows keep the line's stat.
        "hearthguard" => StatMod::DefBp(t.adv_big),
        "rouge_thread" => StatMod::AtkBp(t.adv_small),
        _ => line.advanced.make(size),
    };
    RegaliaDef {
        id: id.to_string(),
        name: line.names[rarity].to_string(),
        signature_anim: "placeholder".to_string(),
        lump_base: t.lump,
        advanced: Some(advanced),
        base_skills: vec![],
        // Passive left empty (user 2026-09-16); five ranks so a dupe still counts as Resonance.
        star_ranks: (1..=5).map(|_| ExemplarStar { mods: vec![], skills: vec![] }).collect(),
    }
}

/// R-rarity FODDER — fed to Enhance a real Regalia. Owns no stats; it is the "EXP food", NOT the
/// R rung of the ladder (an R the player cannot equip would read as a bug).
fn build_fodder_def() -> RegaliaDef {
    RegaliaDef { id: "fodder".to_string(), name: "Memory Dust".to_string(), ..RegaliaDef::default() }
}

/// The SR tier's carrier. A separate def only so its NAME matches its catalog entry —
/// `every_overlay_has_an_item_def_with_a_matching_name` requires it, and two feed tiers a player
/// cannot tell apart would defeat the point of having two.
fn build_fodder_sr_def() -> RegaliaDef {
    RegaliaDef { id: "sr_fodder".to_string(), name: "Radiant Memory Dust".to_string(), ..RegaliaDef::default() }
}

/// The carrier def_id used for R-rarity fodder (fed to Enhance a real Regalia).
pub const REGALIA_FODDER_DEF_ID: &str = "regalia_fodder";

/// SR-tier fodder. PLACEHOLDER content (user, 2026-08-10): the SR tier had no items at all —
/// `sr_pool` was empty, so an SR pull paid the same R fodder as everything else and the guaranteed
/// SR every ten pulls delivered a colour rather than a reward.
pub const REGALIA_FODDER_SR_DEF_ID: &str = "regalia_sr_fodder";

/// The third Dust tier (0.0.490, user 2026-09-29: "ทำ dust 100,000 xp เพิ่ม"): Memory Crystal,
/// 100,000 XP — deep-stage bosses and SSR salvage pay it, so late drops are few and large.
pub const REGALIA_FODDER_CRYSTAL_DEF_ID: &str = "regalia_crystal_fodder";

/// Is this def id Memory Dust (either tier)? Nonzero = Dust. Kept as the one "is it Dust" question
/// the catalog test binds to; what a Dust is WORTH is `regalia_dust_xp` (0.0.490).
pub fn regalia_fodder_levels(def_id: &str) -> u8 {
    match def_id {
        REGALIA_FODDER_DEF_ID | REGALIA_FODDER_SR_DEF_ID | REGALIA_FODDER_CRYSTAL_DEF_ID => 1,
        _ => 0,
    }
}

/// Enhance XP one Dust gives (0.0.490, user 2026-09-28: "เพิ่ม xp มากขึ้น + ให้ dust drop ง่ายขึ้น",
/// numbers after HSR's ~1M-EXP light cone). 0 = not Dust.
pub fn regalia_dust_xp(def_id: &str) -> u32 {
    match def_id {
        REGALIA_FODDER_DEF_ID => 1_000,
        REGALIA_FODDER_SR_DEF_ID => 10_000,
        REGALIA_FODDER_CRYSTAL_DEF_ID => 100_000,
        _ => 0,
    }
}

/// Enhance XP from level 0 to `REGALIA_BASE_CAP` (60), by rarity rung (3★ / SR / SSR). PROVISIONAL
/// (user 2026-09-29: cheaper early, a longer late bar — SSR 2M, the last level ≈ one Crystal).
pub const REGALIA_TOTAL_XP: [u32; 3] = [1_200_000, 1_600_000, 2_000_000];

/// Cumulative Enhance XP to REACH `level` from 0 for rarity rung `rung`. Each step L→L+1 is
/// proportional to `(L + 5)²`: Lv1-20 is ~5% of the total and Lv40-60 ~67%, and the last step is
/// ~160× the first. One cumulative formula, so rounding never drifts. `level` clamps to the cap.
pub fn regalia_xp_to_level(rung: usize, level: u8) -> u32 {
    let total = u64::from(REGALIA_TOTAL_XP[rung.min(2)]);
    // Σ_{k=0}^{l-1} (k+5)² = S(l+4) − S(4), with S(n) = n(n+1)(2n+1)/6.
    let sq = |n: u64| n * (n + 1) * (2 * n + 1) / 6;
    let weight = |l: u64| sq(l + 4) - sq(4);
    let base_cap = u64::from(REGALIA_BASE_CAP);
    let l = u64::from(level.min(REGALIA_MAX_LEVEL));
    let base = total * weight(l.min(base_cap)) / weight(base_cap);
    // Lv61-70: the late sink, each step ×1.4 the one before (`REGALIA_OVERCAP_STEP_W`).
    let past = (l.saturating_sub(base_cap) as usize).min(REGALIA_OVERCAP_STEP_W.len());
    let w_all: u64 = REGALIA_OVERCAP_STEP_W.iter().sum();
    let w_done: u64 = REGALIA_OVERCAP_STEP_W[..past].iter().sum();
    let over = u64::from(REGALIA_OVERCAP_XP[rung.min(2)]) * w_done / w_all;
    u32::try_from(base + over).unwrap_or(u32::MAX)
}

/// Enhance XP from Lv60 to Lv70, by rung: 3★ 12M · SR 16M · SSR 20M. PROVISIONAL.
pub const REGALIA_OVERCAP_XP: [u32; 3] = [12_000_000, 16_000_000, 20_000_000];
/// Relative cost of each step Lv60→61 … Lv69→70: ×1.4 a step (Lv69→70 ≈ 20× Lv60→61).
const REGALIA_OVERCAP_STEP_W: [u64; 10] = [1_000, 1_400, 1_960, 2_744, 3_842, 5_378, 7_530, 10_541, 14_758, 20_661];

/// `(level, xp into that level)` for `total` XP invested at rung `rung` — the inverse of
/// `regalia_xp_to_level`. At the cap the remainder is 0.
pub fn regalia_level_of_xp(rung: usize, total: u32) -> (u8, u32) {
    let mut level = 0u8;
    while level < REGALIA_MAX_LEVEL && regalia_xp_to_level(rung, level + 1) <= total {
        level += 1;
    }
    let rest = if level >= REGALIA_MAX_LEVEL { 0 } else { total - regalia_xp_to_level(rung, level) };
    (level, rest)
}

/// Rarity rung of a Regalia carrier def id (`regalia_<bare>`), `None` for Dust / strangers.
pub fn regalia_rung_of(def_id: &str) -> Option<usize> {
    def_id.strip_prefix("regalia_").and_then(regalia_rarity_index)
}

/// Salvage value of ONE copy, in XP, by rung (0.0.490): 3★ 50,000 (5 Radiant), SR 150,000
/// (15 Radiant), SSR 600,000 (6 Crystals, only a spare past S5). PROVISIONAL.
pub const REGALIA_SALVAGE_XP: [u32; 3] = [50_000, 150_000, 600_000];

/// An amount of Dust across the three tiers, in XP order.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DustPay {
    pub dust: u32,
    pub radiant: u32,
    pub crystal: u32,
}

impl DustPay {
    /// `(def id, count)` per tier, skipping none — the credit and the host view both walk this.
    pub fn tiers(self) -> [(&'static str, u32); 3] {
        [
            (REGALIA_FODDER_DEF_ID, self.dust),
            (REGALIA_FODDER_SR_DEF_ID, self.radiant),
            (REGALIA_FODDER_CRYSTAL_DEF_ID, self.crystal),
        ]
    }
}

/// `total_xp` as Dust: Crystals first, then Radiant, the remainder as Dust rounded UP (never
/// short-changed).
pub fn regalia_xp_as_dust(total_xp: u32) -> DustPay {
    let c = regalia_dust_xp(REGALIA_FODDER_CRYSTAL_DEF_ID);
    let r = regalia_dust_xp(REGALIA_FODDER_SR_DEF_ID);
    let d = regalia_dust_xp(REGALIA_FODDER_DEF_ID);
    DustPay { crystal: total_xp / c, radiant: (total_xp % c) / r, dust: (total_xp % r).div_ceil(d) }
}

/// What SALVAGING a Regalia pays (0.0.489; XP-valued 0.0.490). Every copy the item carries (1 +
/// its Resonance stars) pays its rung's `REGALIA_SALVAGE_XP`, plus every Enhance XP invested in
/// it, paid out largest tier first. `None` for Dust itself and unknown ids.
pub fn regalia_salvage_yield(def_id: &str, stars: u8, enhance_level: u8, enhance_xp: u32) -> Option<DustPay> {
    if regalia_fodder_levels(def_id) > 0 {
        return None;
    }
    let rung = regalia_rung_of(def_id)?;
    let copies = u32::from(stars.min(REGALIA_MAX_STARS)) + 1;
    let base = REGALIA_SALVAGE_XP[rung.min(2)].saturating_mul(copies);
    let invested = regalia_xp_to_level(rung, enhance_level).saturating_add(enhance_xp);
    Some(regalia_xp_as_dust(base.saturating_add(invested)))
}

/// The six stat-only SR carriers RETIRED in 0.0.489 (user 2026-09-28: "SR รุ่นเก่าลบออกเลย") —
/// they had no passive, so a 3★ card out-did them. A save holding one is paid its salvage value on
/// load (`data::build::items::migrate_retired_regalia`).
pub const RETIRED_SR_REGALIA: [&str; 6] = ["sunbreaker", "nightjar", "farsight", "velvet_mask", "stormglass", "choirlight"];

/// What salvaging `owned[idx]` would pay, or `None` when it may not be salvaged: a worn or
/// locked copy, Dust itself, an unknown id, and an SSR unless ANOTHER copy of the same card is
/// already at max Resonance (a spare dupe belongs in Resonance first). The one rule the salvage
/// handler and the host's `salvage` preview both read.
pub fn regalia_salvage_of(owned: &[Item], idx: usize) -> Option<DustPay> {
    let it = owned.get(idx)?;
    if !matches!(it.kind, ItemKind::Regalia) || it.equipped_by != 0 || it.locked {
        return None;
    }
    let paid = regalia_salvage_yield(&it.def_id, it.resonance_stars, it.enhance_level, it.enhance_xp)?;
    let rung = it.def_id.strip_prefix("regalia_").and_then(regalia_rarity_index)?;
    if rung >= 2
        && !owned
            .iter()
            .enumerate()
            .any(|(i, c)| i != idx && c.def_id == it.def_id && c.resonance_stars >= REGALIA_MAX_STARS)
    {
        return None;
    }
    Some(paid)
}

/// Does "salvage all" take `owned[idx]`? Only an UNTOUCHED 3★ card — Enhance Lv0, Resonance S1,
/// not worn, not locked (user 2026-09-28). Everything else is one deliberate `SalvageRegalia`.
pub fn regalia_bulk_salvageable(owned: &[Item], idx: usize) -> bool {
    owned.get(idx).is_some_and(|it| {
        it.enhance_level == 0
            && it.resonance_stars == 0
            && it.def_id.strip_prefix("regalia_").and_then(regalia_rarity_index) == Some(0)
            && regalia_salvage_of(owned, idx).is_some()
    })
}

/// Is this def id a UNIQUE by its NAME? Used by ONE caller: the test that compares this convention
/// against `ItemDef.unique`, which is the flag everything else reads.
///
/// It is not a rule input. It briefly was — `reforge_matches` called it, and wrapping a
/// `starts_with` in a well-named function made that look considered when `Item` had carried the
/// flag the whole time. The convention is still worth keeping true, so
/// `the_unique_flag_and_the_unique_prefix_agree` in `data/mod.rs` binds it to the data, so a
/// unique authored without the prefix (or a prefix without the flag) is a red test rather than a
/// silent change to what can reforge what.
pub fn is_unique_def_id(def_id: &str) -> bool {
    def_id.starts_with("unique_")
}

/// Is this def id Regalia FODDER — the feed for the Level axis?
///
/// Also a convention (`*fodder`), and until now it lived as three independent `ends_with("fodder")`
/// checks in the client plus one hard-coded id in the sim, which disagreed about what counts. A
/// second fodder tier is exactly the change that would have found that the hard way.
pub fn is_regalia_fodder(def_id: &str) -> bool {
    regalia_fodder_levels(def_id) > 0
}

/// Class family a Regalia may equip onto (empty = ANY class). P0: unrestricted — a Regalia is
/// meant to be broadly usable (best-on-role, not fixed); role/weapon-type gating is a later tune.
pub fn regalia_allowed_classes(_id: &str) -> &'static [&'static str] {
    &[]
}

/// The DOCTRINE a Regalia line is built for (0.0.436). Never a gate on equipping — anyone wears
/// anything and takes its lump and advanced stat; the PASSIVE works only on a wearer of this
/// doctrine (user: "ใส่ได้ทุกตัว แต่สกิลจะทำงานเฉพาะตรงสาย Doctrine / stat ได้หมดถึงจะไม่ตรงสาย").
pub fn regalia_doctrine(id: &str) -> Option<crate::potential::Doctrine> {
    if id.is_empty() {
        return None;
    }
    // The 3★ set (0.0.485) moves Rouge Thread's R rung to Catalyst — two cards per Doctrine —
    // while Velvet Mask / Lycoris Lullaby stay the Striker line Nemu's piece belongs to.
    if id == "rouge_thread" {
        return Some(crate::potential::Doctrine::Catalyst);
    }
    REGALIA_LINES.iter().find(|l| l.ids.contains(&id)).map(|l| l.doctrine)
}

/// The R / SR / SSR ids of the FIRST line built for `doctrine` (a doctrine may own several).
pub fn regalia_line_for_doctrine(doctrine: crate::potential::Doctrine) -> Option<[&'static str; 3]> {
    REGALIA_LINES.iter().find(|l| l.doctrine == doctrine).map(|l| l.ids)
}

/// The SSR piece designed around `unit_def`, with its R / SR rungs — the line a unit's own
/// banner features. None for a unit without one (its doctrine's first line is the fallback).
pub fn regalia_line_for_unit(unit_def: &str) -> Option<[&'static str; 3]> {
    let sig = match unit_def {
        "seraphine" => "aurora_edge",
        "lapis" => "empyrean",
        "lucielle" => "litany_of_dawn",
        "aveline" => "meridian",
        "raplus" => "silent_verdict",
        "nemu" => "lycoris_lullaby",
        "lumi" => "hoarfrost_crown",
        "airi" => "thunderhead",
        "charon" => "ferrymans_lantern",
        "suzu" => "emberbell",
        "mirei" => "stormglass_pin",
        "astier" => "tidecaller",
        "sylvie" => "static_charm",
        "zolfia" => "winters_oath",
        _ => return crate::potential::doctrine_for_unit(unit_def).and_then(regalia_line_for_doctrine),
    };
    REGALIA_LINES.iter().find(|l| l.ids.contains(&sig)).map(|l| l.ids)
}


/// Card star cap (0-5★) and the DUPLICATE copies consumed per upgrade step
/// (star N → N+1 costs `CARD_STAR_DUPES[N]` extra copies of the same stack).
/// Full 0→5★ = 1+1+2+3+5 = 12 dupes + the promoted copy itself.
pub const CARD_STAR_MAX: u8 = 5;
pub const CARD_STAR_DUPES: [u32; 5] = [1, 1, 2, 3, 5];

/// When a slotted consumable auto-fires (RO auto-pot style). Self-contained so
/// the auto-use tick can evaluate it without the skill layer.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConsumeTrigger {
    /// Use when the holder's HP drops below this percent of max (e.g. 50).
    HpBelowPct(u8),
    /// Use when the spec's own buff is NOT currently active — keeps a self-buff
    /// (ASPD/Berserk pot) refreshed. Pairs with `ConsumeEffect::Buff`.
    BuffMissing(crate::skills::BuffKind),
    /// Never auto-fires from the consumable bar — applied only by an explicit
    /// player action (e.g. loading an elemental quiver via `LoadAmmo`). The
    /// upkeep skips it entirely.
    Manual,
    /// Fires when a PARTY MEMBER is down — pairs with `ReviveAlly` (Phoenix
    /// Tail). Handled in `phoenix_revive_upkeep`, not the self-use loop.
    AllyDown,
}

/// What a consumable does when it fires.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConsumeEffect {
    /// Restore a FLAT amount of HP (RO red/white/blue potion tiers).
    HealHp(i32),
    /// Apply a timed self-buff (ASPD/Power potion). `magnitude` follows the
    /// `BuffKind`'s own scale (e.g. AspdPct is basis points: 1000 = +10%).
    Buff { kind: crate::skills::BuffKind, magnitude: i32, ticks: u32 },
    /// Apply TWO timed self-buffs at once — a Meal is a stat line AND a ward, and one item that
    /// hands out both is what a player expects from food. A separate variant rather than turning
    /// `Buff` into a list, so every existing consumable serializes exactly as before and no
    /// golden moves. A fixed pair rather than a `Vec` because `ConsumeEffect` is `Copy` and the
    /// auto-use loop matches it by value; widen it deliberately if a third effect is ever real.
    Buffs([ConsumeBuff; 2]),
    /// Load an elemental QUIVER into the holder's ammo pool: `shots` rounds of
    /// `element`. Ranged physical attacks then auto-swap to whichever loaded
    /// element best beats the current target (see `combat::pick_ammo_idx`),
    /// spending one round per advantaged shot. NOT auto-triggered — loaded
    /// manually (LoadAmmo input), so it carries no `ConsumeTrigger`/cooldown.
    LoadAmmo { element: crate::grid::Element, shots: u32 },
    /// Revive the most-recently-fielded DOWN party member (Phoenix Tail),
    /// restoring `hp_pct`% of their max HP. Pairs with `ConsumeTrigger::AllyDown`
    /// and is resolved in `phoenix_revive_upkeep` (it touches another entity, not
    /// the holder), so the self-use loop treats it as a no-op.
    ReviveAlly { hp_pct: u8 },
}

/// One element's stack of loaded arrows in an archer's ammo pool. The pool is a
/// small `Vec` (one entry per distinct element); empty for everyone who never
/// loads a quiver, so it serde-skips and keeps melee/mage goldens byte-identical.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct AmmoStack {
    pub element: crate::grid::Element,
    /// Remaining rounds. Decremented one per advantaged shot; the stack is
    /// dropped from the pool when it hits 0.
    pub shots: u32,
}

/// Which RESERVED slot a consumable belongs in. `None` = an ordinary emergency item that lives
/// in the auto-use bar (a healing phial, a quiver).
///
/// Reserved rather than sharing the bar, because food competing with a healing potion for the
/// same three slots is not an interesting choice — a player picks "do not die" every time, and
/// the buff line would simply never be used.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProvisionKind {
    Meal,
    Tonic,
    /// A PARTY provision — it goes in one of the three party-wide slots and buffs every living
    /// member, layered ON TOP of the per-character ones rather than replacing them
    /// (docs/party-buff-slots-design.md).
    ///
    /// Unlike Meal/Tonic this does NOT name one reserved slot: the three party slots are
    /// interchangeable and any Party item fits any of them. The variant is membership — "this may
    /// enter the party rack" — not a position.
    Party,
}

/// One entry of a multi-buff consumable. Mirrors the fields of `ConsumeEffect::Buff`.
/// `Copy` so `ConsumeEffect` can stay `Copy` — the auto-use loop matches it by value in a hot
/// path and every call site assumes that.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsumeBuff {
    pub kind: crate::skills::BuffKind,
    pub magnitude: i32,
    pub ticks: u32,
}

/// The auto-use behaviour baked into a consumable item.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsumableSpec {
    pub trigger: ConsumeTrigger,
    pub effect: ConsumeEffect,
    /// Reserved-slot membership. Absent for every existing consumable, so their items serialize
    /// byte-identically and no golden moves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provision: Option<ProvisionKind>,
    /// Minimum ticks between auto-uses (anti-spam; e.g. an HP pot can't fire
    /// every tick while still hurt).
    pub cooldown: u32,
}

/// One of a character's 3 consumable slots: the slotted item (its `quantity`
/// is the remaining charges) plus the next tick it may auto-fire.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsumableSlot {
    pub item: Item,
    #[serde(default, skip_serializing_if = "is_zero_u64_cs")]
    pub ready_tick: u64,
    /// STABLE slot identity, 1-based. The buff source is derived from it, never from the position
    /// in the Vec — `consumables`/`provisions` SHRINK (a depleted stack is swept every tick, and
    /// the player removes slots by hand), and a positional source slides the survivor onto the
    /// departed slot's id: its `BuffMissing` then finds nothing there, re-fires, burns a charge and
    /// runs two copies of one buff.
    ///
    /// 0 = a slot written before this field existed. `consumable_upkeep` assigns it on the next
    /// pass, so the repair needs no migration step of its own. Skip-if-zero keeps old saves
    /// byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u8_cs")]
    pub key: u8,
}

fn is_zero_u8_cs(n: &u8) -> bool {
    *n == 0
}

fn is_zero_u64_cs(n: &u64) -> bool {
    *n == 0
}

/// Reserved buff `source_id` for consumable self-buffs (distinct from entity
/// ids + the path-aura source, so re-use refreshes rather than stacks).
pub const CONSUMABLE_BUFF_SOURCE: u32 = 9_100_001;

/// One PARTY rack entry. Its own type rather than a `ConsumableSlot`, for the `key`.
///
/// `key` is the slot's STABLE identity, chosen at insert as the lowest free value and held until
/// the item leaves. The buff source is derived from it, never from the position in the Vec —
/// removing an entry shifts everything after it, and a source that shifted with it would mean a
/// still-running buff (they run to their full duration, see `remove_party_provision`) suddenly
/// belonging to a different item: the neighbour would stop refreshing while the departed item's
/// buff blocked it, then double up when it lapsed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartyProvision {
    pub key: u8,
    pub item: Item,
    #[serde(default, skip_serializing_if = "is_zero_u64_cs")]
    pub ready_tick: u64,
}

/// How many PARTY provision slots the rack holds.
pub const PARTY_PROVISION_SLOTS: usize = 3;

/// Reserved buff `source_id` base for the party rack — slot `i` uses `BASE + i`.
///
/// One per SLOT, not one for the rack, and that is load-bearing rather than tidy: three different
/// items each granting +ATK is a legal loadout (the rule bans a repeated ITEM, not a repeated buff
/// kind), and `apply_buff` replaces on `(kind, source_id)`. Share the id and that loadout collapses
/// to one buff — three items paid for, one applied, nothing said.
pub const PARTY_PROVISION_SOURCE: u32 = 9_200_001;

/// Weapon class — drives weapon-type-gated effects (e.g. Sword Mastery
/// only boosts 1H swords + daggers). `None` = no weapon / non-weapon item
/// (armor, materials) / bare hand. Default so old saves + non-weapons
/// deserialize cleanly.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum WeaponType {
    #[default]
    None,
    Sword1H,
    Sword2H,
    Dagger,
    Bow,
    Staff,
    Mace,
}

/// WHAT SITS IN THE OFF-HAND (0.0.446, user 2026-09-19: "จะเพิ่ม tome quiver hidden knife เป็น
/// offhand ใหม่"). The off-hand is the `Shield` slot; until now a shield was the only thing it
/// held. Each weapon family has ONE off-hand kind — `offhand_for_weapon` — and the gate reads the
/// character's family the way weapons do (`is_preferred_by`), so a Tome is Rapis' and
/// Lucielle's, a Quiver is Aveline's, a Hidden Knife is Nyx's, a Shield is Seraphine's. `None` is
/// every item that is not an off-hand piece (the field is absent on their defs).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum OffhandType {
    #[default]
    None,
    Shield,
    Tome,
    Quiver,
    HiddenKnife,
    /// Mace & trinket (0.0.463, user: "Mace offhand เป็น trinket ก็ได้") — Charon's.
    Trinket,
}

impl OffhandType {
    pub fn is_none(&self) -> bool {
        matches!(self, OffhandType::None)
    }

    /// Stable string key — matches the serde variant name.
    pub fn key(self) -> &'static str {
        match self {
            OffhandType::None => "None",
            OffhandType::Shield => "Shield",
            OffhandType::Tome => "Tome",
            OffhandType::Quiver => "Quiver",
            OffhandType::HiddenKnife => "HiddenKnife",
            OffhandType::Trinket => "Trinket",
        }
    }
}

/// The off-hand kind a weapon family carries. Sword & shield, staff & tome, bow & quiver,
/// dagger & hidden knife, mace & trinket (0.0.463); a two-hander has no off-hand.
pub fn offhand_for_weapon(wt: WeaponType) -> OffhandType {
    match wt {
        WeaponType::Sword1H => OffhandType::Shield,
        WeaponType::Staff => OffhandType::Tome,
        WeaponType::Bow => OffhandType::Quiver,
        WeaponType::Dagger => OffhandType::HiddenKnife,
        WeaponType::Mace => OffhandType::Trinket,
        WeaponType::Sword2H | WeaponType::None => OffhandType::None,
    }
}

/// The off-hand kinds a character may equip — one per weapon family it wields.
pub fn offhand_types_for_character(exemplar_id: Option<&str>, unit_def: &str) -> Vec<OffhandType> {
    weapon_types_for_character(exemplar_id, unit_def)
        .iter()
        .map(|w| offhand_for_weapon(*w))
        .filter(|o| !o.is_none())
        .collect()
}

impl WeaponType {
    /// Stable string key — matches the serde variant name, for tables and
    /// views keyed by weapon type without allocating.
    pub fn key(self) -> &'static str {
        match self {
            WeaponType::None => "None",
            WeaponType::Sword1H => "Sword1H",
            WeaponType::Sword2H => "Sword2H",
            WeaponType::Dagger => "Dagger",
            WeaponType::Bow => "Bow",
            WeaponType::Staff => "Staff",
            WeaponType::Mace => "Mace",
        }
    }
}

/// Paper-doll equipment slot — one item per slot. Drives equip de-dup: equipping a piece swaps
/// out whatever shares its slot. `ItemKind` is kept for COMBAT semantics (Weapon→ATK, everything
/// else→DEF); the slot is purely the wear position. The Assassin's second dagger wears the
/// `Shield` slot (0.0.383 — the off-hand is one cell, shield OR blade): `Shield` is stamped onto
/// `Item.equip_slot` at equip time when a dagger routes off-hand (see
/// `events::handlers::equip_item`) and cleared on unequip.
///
/// RENAMED 0.0.375 (design §2.5). The old names described RO's layout and then lied about it:
/// `Gloves` displayed as "Face" (a mask) and `Pants` as "Aura", so three "head" slots covered
/// two things that are not on the head — and `Earring` (a cloak) sat on the defensive side while
/// carrying no defence worth the name. The new set says what the player sees:
///
/// | was | is | why |
/// |---|---|---|
/// | `Helmet` | `Helmet` | same thing, honest name |
/// | `Gloves` | `Gloves` | was a mask; hands are a slot every ARPG has and nobody has to learn |
/// | `Pants` | — | was an aura; RETIRED 0.0.389 (Bracer took its place in the count) |
/// | `Earring` | `Earring` | moves to utility — a cloak that gives no defence is jewellery |
/// | `Necklace` | `Necklace` | a name instead of an index |
/// | `Ring` | `Ring` | " |
/// | — | `Bracer` | NEW, the 4th utility slot (design §2.5: defence 5 · utility 4 · hands 2+1) |
///
/// Every renamed variant keeps a `serde(alias)` for its old spelling, so saves written before the
/// rename still load — the names are a display decision and must not cost anyone their gear.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EquipSlot {
    #[serde(alias = "HeadTop")]
    Helmet,
    #[serde(alias = "HeadMid")]
    Gloves,
    /// RETIRED (0.0.389, user: "เอา pant ออกได้" — Bracer was meant to REPLACE an armour slot,
    /// not join it). Deserializes so an old save's legs still parse; `migrate_save_items` drops
    /// the piece. Never produced, never in a pool, fits no gem.
    #[serde(alias = "HeadLow")]
    Pants,
    Armor,
    Weapon,
    Shield,
    #[serde(alias = "Garment")]
    Earring,
    Shoes,
    #[serde(alias = "Accessory1")]
    Necklace,
    #[serde(alias = "Accessory2")]
    Ring,
    /// RETIRED (0.0.383): the off-hand IS the `Shield` slot now — a dagger routed off-hand is
    /// stamped `Shield`. The variant stays so an old save's stamp deserializes; `migrate_item`
    /// rewrites it to `Shield` on load. Never produced, never in a pool.
    WeaponLeft,
    /// The 4th utility slot — no main stat, lines only, like Necklace / Ring / Earring.
    Bracer,
}

/// The card PANEL classes (RO-style, slot-typed). A Card belongs to exactly one;
/// it equips into that class's panel on the CHARACTER (not into a gear item). How
/// many panel cards of a class are ACTIVE is gated by the equipped gear's sockets.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CardClass {
    Weapon,
    Shield,
    Armor,
    #[serde(alias = "Garment")]
    Earring,
    Shoes,
    /// Covers the DEFENSIVE slots that used to be the three "head" slots — Helmet and Gloves
    /// (Pants retired 0.0.389). Kept as ONE panel rather than split into three when the slots were renamed
    /// (0.0.375): cards are being redesigned alongside gems, and splitting a panel that is about
    /// to change shape would buy a migration and nothing else.
    Head,
    #[serde(alias = "Accessory1")]
    Necklace,
    #[serde(alias = "Accessory2")]
    Ring,
}

/// All panel classes, in a fixed order (deterministic iteration for `card_bonuses`).
pub const CARD_CLASSES: [CardClass; 8] = [
    CardClass::Weapon,
    CardClass::Shield,
    CardClass::Armor,
    CardClass::Earring,
    CardClass::Shoes,
    CardClass::Head,
    CardClass::Necklace,
    CardClass::Ring,
];

/// How many cards a character may EQUIP in a class's panel (independent of gear).
/// Weapon 4 / Shield 1 / everything else 2 (the two accessories are SEPARATE panels,
/// Necklace + Ring, 2 each). Activation is separately gated by the equipped
/// gear's socket count (`card_bonuses`).
pub fn card_panel_capacity(class: CardClass) -> u8 {
    match class {
        CardClass::Weapon => 4,
        CardClass::Shield => 1,
        _ => 2,
    }
}

/// The panel class an equipped gear slot feeds (its sockets gate that class's cards).
/// The 3 head slots feed the single Head panel; the two accessory slots feed SEPARATE
/// panels (Necklace / Ring), each gated by its own accessory's sockets. WeaponLeft
/// (off-hand) feeds nothing yet. `None` = this slot doesn't gate any card class.
pub fn card_class_of_slot(slot: Option<EquipSlot>) -> Option<CardClass> {
    match slot {
        Some(EquipSlot::Weapon) => Some(CardClass::Weapon),
        Some(EquipSlot::Shield) => Some(CardClass::Shield),
        Some(EquipSlot::Armor) => Some(CardClass::Armor),
        Some(EquipSlot::Earring) => Some(CardClass::Earring),
        Some(EquipSlot::Shoes) => Some(CardClass::Shoes),
        Some(EquipSlot::Helmet) | Some(EquipSlot::Gloves) => Some(CardClass::Head),
        Some(EquipSlot::Necklace) => Some(CardClass::Necklace),
        Some(EquipSlot::Ring) => Some(CardClass::Ring),
        _ => None,
    }
}

/// True for the two accessory panel classes. An ACCESSORY card is generic — it may
/// equip into EITHER accessory panel (RO-style), so equip checks this instead of an
/// exact class match, and stamps the chosen panel onto the card.
pub fn is_accessory_class(c: CardClass) -> bool {
    matches!(c, CardClass::Necklace | CardClass::Ring)
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_neutral_element(e: &crate::grid::Element) -> bool {
    matches!(e, crate::grid::Element::Physical)
}

/// Tolerate an orb effect this build no longer knows (a RETIRED variant in an old
/// save) by loading it as `None`. Orbs are def-driven and consumable, so an unknown
/// one is inert — far better than refusing to load the save. Everything else still
/// errors, so a genuine shape break is still caught.
fn orb_lenient<'de, D>(d: D) -> Result<Option<OrbEffect>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<serde_json::Value>::deserialize(d)?;
    Ok(raw.and_then(|v| serde_json::from_value(v).ok()))
}

/// RETIRED with the Star Stone system (0.0.381). Where a stone was aimed — kept only so an old
/// input log's `UseOrb` still parses.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OrbTarget {
    Stash { idx: u32 },
    Equipped { entity_id: u32, slot: u32 },
}

/// RETIRED (0.0.381): the Star Stone system is gone — no stone def, no `orbs` module, no fee, no
/// apply. The enum stays so an OLD save's `Item.orb` and an old `UseOrb` in an input log still
/// deserialize (through `orb_lenient`, which also swallows variants this build never had).
/// Nothing reads it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OrbEffect {
    /// Chaos Star Stone: reroll EVERY enchant line (the locked one survives).
    RerollAll,
    /// Colored Star Stone: raise star by one — usable only when the item is at
    /// exactly `from` stars (Green 0, Blue 1, Purple 2, Yellow 3, Red 4). The
    /// new slot's line rolls immediately (roll-at-drop canon).
    StarUp { from: u8 },
    /// Death Star Stone: lock ONE RANDOM line permanently (reusable — each use
    /// re-draws which line is locked; one lock per item at a time).
    LockRandom,
    /// Shiny Star Stone: reroll only the NUMBERS of every line, within each
    /// line's current tier (families/params keep; the locked line is skipped).
    RerollValues,
}

/// Combustion peak per-second burn as a fraction of the hit (basis points): 20% ×
/// tier. Single source shared by the burn resolve (`apply_combustion_burn`) and the
/// tooltip's `effect_value`.
pub const fn combustion_peak_bp(tier: u8) -> u32 {
    2_000 * tier as u32
}


/// A per-instance copy of a `GearOverride` (catalog) for THIS item's grade — the graded stats
/// the designer pinned for this (def × grade) instead of the formula. Stamped wherever a grade is
/// assigned (drop, fusion, migration) from the same catalog row, so every copy carries the same
/// numbers and replay stays identical. `None` = the formula.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StatOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atk: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub def: Option<i32>,
}

/// GEAR GRADE — the power axis of the fixed-stat gear model (0.0.372,
/// `docs/gear-fusion-and-base-fix-design.md` §3.5). Ten steps, ascending. A piece's stats are
/// (type × tier) as authored, MULTIPLIED by its grade; six pieces of one grade fuse into one of
/// the next, which is why the ladder and the fusion ratio have to be read together.
///
/// SEPARATE from `Rarity`, deliberately. Rarity is the display/rare-bag axis and carries the
/// enchant ★ ladder; grade is what the market prices and what fusion moves. Collapsing them
/// would make "how good is it" and "what colour is its name" the same question, and they stop
/// agreeing the moment an enchanted Common exists.
///
/// Serialized BY NAME, and the `Ord` derive keys off declaration order (Common is the floor).
/// APPEND-ONLY past `Cosmic`: a reorder silently re-grades every saved item.
///
/// These identifiers are SAVE KEYS only — no player ever reads them. The names on screen are the
/// loc rows `GRADE_0`..`GRADE_9` (2026-09-29, user-chosen ladder: Common · Uncommon · Rare · Epic ·
/// Legendary · Astral · Stellar · Solar · Nova · Cosmos), so renaming a grade is a loc edit, never
/// a variant rename (that would orphan every saved item).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub enum Grade {
    #[default]
    Common,
    Uncommon,
    Rare,
    Legendary,
    Immortal,
    Arcana,
    Beyond,
    Celestial,
    Divine,
    Cosmic,
}

/// Every grade, ascending — the one place the ladder's LENGTH is written down.
pub const GRADES: [Grade; 10] = [
    Grade::Common, Grade::Uncommon, Grade::Rare, Grade::Legendary, Grade::Immortal,
    Grade::Arcana, Grade::Beyond, Grade::Celestial, Grade::Divine, Grade::Cosmic,
];

/// RAW-stat multiplier per grade in basis points: ×1.30 per step, ×10.60 across the ladder.
/// Measured against Task Bar Hero's real sword table (raw ×10.8-10.9 end to end) — see
/// `docs/taskbar-hero-item-study.md` §1.6. Written as a TABLE, not `pow`: the sim has no floats,
/// and a table is the only form that is exactly reproducible on every platform.
pub const GRADE_RAW_BP: [u32; 10] =
    [10_000, 13_000, 16_900, 21_970, 28_561, 37_129, 48_268, 62_749, 81_573, 106_045];

/// PERCENT-stat multiplier per grade in basis points: ×1.15 per step, ×3.52 across the ladder.
/// Deliberately FLATTER than the raw axis. A percent stat has a real ceiling — block caps at
/// 100%, attack speed hits the tick — so paying it ×10.6 would run the ladder off the end of its
/// own scale by grade 7 and make the last three steps worthless. TBH's own percent lines move
/// ×2.2-4.0 across the same ladder, which is the evidence this is not a guess.
pub const GRADE_PCT_BP: [u32; 10] =
    [10_000, 11_500, 13_225, 15_209, 17_490, 20_114, 23_131, 26_600, 30_590, 35_179];

impl Grade {
    /// Position on the ladder, 0 = Common. The index into both multiplier tables.
    pub fn step(self) -> usize {
        GRADES.iter().position(|g| *g == self).unwrap_or(0)
    }
    /// The next grade up, or `None` at the top — the fusion output, and the only way up.
    pub fn next(self) -> Option<Grade> {
        GRADES.get(self.step() + 1).copied()
    }
    /// Raw-stat multiplier (bp). See `GRADE_RAW_BP`.
    pub fn raw_bp(self) -> u32 {
        GRADE_RAW_BP[self.step()]
    }
    /// Percent-stat multiplier (bp). See `GRADE_PCT_BP`.
    pub fn pct_bp(self) -> u32 {
        GRADE_PCT_BP[self.step()]
    }
    /// Scale a RAW authored stat to this grade. Rounds UP so a grade step never reads as "+0" on
    /// a small number — the same rule `star_base_bonus` uses, for the same reason.
    pub fn scale_raw(self, base: i32) -> i32 {
        if base == 0 {
            return 0;
        }
        let m = self.raw_bp() as i64;
        let v = (base as i64 * m + if base > 0 { 9_999 } else { -9_999 }) / 10_000;
        v.clamp(i32::MIN as i64, i32::MAX as i64) as i32
    }
    /// Scale a PERCENT authored stat (basis points) to this grade. See `GRADE_PCT_BP`.
    pub fn scale_pct(self, base_bp: i32) -> i32 {
        if base_bp == 0 {
            return 0;
        }
        let m = self.pct_bp() as i64;
        let v = (base_bp as i64 * m + if base_bp > 0 { 9_999 } else { -9_999 }) / 10_000;
        v.clamp(i32::MIN as i64, i32::MAX as i64) as i32
    }
}

fn is_common_grade(g: &Grade) -> bool {
    matches!(g, Grade::Common)
}


/// Rarity tiers, ordered ascending (`Common < Uncommon < Rare < Mythical <
/// Legendary < Immortal`) — Dota 2's ladder, per the user (Immortal before
/// Arcana; Arcane stays in reserve as a future tier ABOVE this). The `Ord`
/// derive keys off this declaration order — used by the rare-loot router to
/// test `item.rarity >= rare_threshold`.
/// Serialized BY NAME (serde), so inserting `Mythical`/`Immortal` (0.0.135)
/// did not disturb old saves. The ladder mirrors gear stars (user 2026-07-05):
/// 1★ Uncommon · 2★ Rare · 3★ Mythical · 4★ Legendary · 5★ Immortal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Rarity {
    #[default]
    Common,
    Uncommon,
    Rare,
    Mythical,
    Legendary,
    Immortal,
}

/// One rolled ENCHANT line on a piece of gear (gear-systems-design §2 — the
/// full 24-mod pool, effect layer). Values are NATIVE units: flat stats/HP/
/// HIT/FLEE are plain ints; every `*_bp` value is basis points (10000 = 100%).
/// The ROLLING systems (orbs) come later — this is the carrier + effect layer.
/// Variant order APPEND-ONLY (serialized in saves).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StatMod {
    // --- HP / offense %s (additive buckets, uncapped by design) ---
    MaxHp(i32),
    MaxHpBp(i32),
    #[serde(alias = "MatkBp")]
    AtkBp(i32),
    CritBp(i32),
    // --- attacker-side conditionals (Hydra lineage; additive, free) ---
    DmgVsElement(crate::grid::Element, i32),
    DmgVsRace(crate::grid::Race, i32),
    DmgVsSize(crate::grid::Size, i32),
    // --- "can zero a mechanic" family: capped at the consumption site ---
    ResistElement(crate::grid::Element, i32), // cap 7500 per element
    #[serde(alias = "IgnoreMdefBp")]
    IgnoreDefBp(i32),                     // cap 5000 (weapon-pool)
    CooldownBp(i32),                          // cap 3000 (accessory-pool)
    CastBp(i32),                              // cap 5000 (head-pool)
    MoveSpeedBp(i32),                         // cap 3000 (shoes)
    DrVsRace(crate::grid::Race, i32),         // cap 5000 (shield/armor)
    DrVsSize(crate::grid::Size, i32),         // cap 5000
    DrVsRanged(i32),                          // cap 5000 (garment)
    ResistStatusBp(u8, i32),                  // status code (see status_code), cap 7500
    // --- support ---
    HealBp(i32),
    // --- flat offense (appended 0.0.134b) ---
    #[serde(alias = "MatkFlat")]
    AtkFlat(i32),
    // --- crit damage (appended 0.0.224 — renewal-crit rework; weapon+accessory
    // pool, additive into the ×150%-base crit multiplier, uncapped) ---
    CritDmgBp(i32),
    // --- flat hard DEF (appended 0.0.243 — the ONE vocabulary gap `CardEffect`
    // had over `StatMod`; joins gear DEF BEFORE the DefPct multiplier, exactly
    // where the old card `def` field folded). NOT in `gear_roll::FAMILIES`: gear
    // DEF comes from `def_bonus` + refine + stars, so this family is authored-only
    // (cards / overlays) and the enchant draw set is unchanged. ---
    DefFlat(i32),
    // --- the two stats the gacha stat set names that nothing could express
    // (appended 0.0.320; `skill-system-redesign.md` §4.5.1). Both are AUTHORED-ONLY
    // (cards / overlays / the Potential board) — not in `gear_roll::FAMILIES`, so
    // the enchant draw set is unchanged. ---
    /// +% damage dealt with attacks of THIS element — the attacker's own element,
    /// not the defender's (`DmgVsElement` is the defender-keyed one). Folds into the
    /// same element-keyed bucket the Broomhilda card special already uses.
    ElementDmgBp(crate::grid::Element, i32),
    /// +% Limit Break gauge earned from every source. Scales the gain, never the
    /// ceiling — the ceiling is the Ultimate's `lb_max`.
    ChargeRateBp(i32),
    /// +% BREAK DAMAGE — the payoff for emptying a Toughness gauge. HSR's Break
    /// Effect: it scales the break's own number, which is built from the attacker's
    /// LEVEL and the gauge that broke, never from ATK.
    BreakDmgBp(i32),
    /// +% BREAK BUILD-UP — how fast a hit chips the gauge. ZZZ's Anomaly Mastery,
    /// the other half of the pair: build-up decides how OFTEN a break happens,
    /// `BreakDmgBp` decides how hard it lands. Neither does the other's job.
    BreakBuildupBp(i32),
    /// DODGE CHANCE — the defender's chance to take nothing at all from a direct
    /// hit. Replaces the HIT/FLEE pair (0.0.333): an opposed pair asks the player to
    /// read two hidden numbers against each other, and in an idle game they see
    /// neither, so a 30% miss rate at Lv70 was a tax nobody could see or answer.
    ///
    /// Sources stack INVERSE MULTIPLICATIVELY, `1 - (1-p1)(1-p2)...`, which is
    /// Diablo 4's rule and the reason it needs no cap: the fifth line is always worth
    /// less than the first, and affixes alone never reach 100%.
    DodgeBp(i32),
    /// +% damage against a BOSS (`Entity::is_boss`) — the other half of the Valor gem (0.0.391,
    /// WoG's boss-damage line). Folded in `combat::damage` as its own multiply, after the
    /// element bucket.
    DmgVsBossBp(i32),
    /// FLAT HP restored every regen window (`tick::upkeep`, 5 s), on top of whatever a passive
    /// pays — the live kits have no regen passive, so this line is the only out-of-heal
    /// recovery a unit has (user 2026-09-16: "HP regen ทำเลย แต่เป็น flat"). The Fortitude gem.
    HpRegenFlat(i32),
    /// +% HARD DEF (0.0.406) — the Training Fortitude slot. Authored-only (not in the enchant
    /// draw set); folds into the `DefPct` bucket in `derived::effective_hard_def`.
    DefBp(i32),
    /// PARTY ECONOMY lines (0.0.433, the T3 crests): +% XP per kill, +% Gil earned from any
    /// source, +% soul per kill, and +% chance the kill table pays (the `nothing` share shrinks).
    /// Read off `GameState.crest_mods`, never off a wearer — they are party-wide by construction.
    XpGainBp(i32),
    GilGainBp(i32),
    SoulGainBp(i32),
    DropRateBp(i32),
    /// Flat damage taken −N, subtracted LAST in `apply_damage` (after DEF, element, every %
    /// bucket), floored at 1 (0.0.447, the Training Guard slot). Worth a third of a stage-1
    /// swing and nothing against a stage-100 one — an early-game line by construction.
    DmgReduceFlat(i32),
    /// THE SINK for every line whose stat no longer exists — the six attributes (0.0.317)
    /// and the HIT/FLEE pair (0.0.333). It reads as nothing, rolls as nothing and renders as
    /// nothing; it exists so a save written before those cuts still DESERIALIZES, and so it
    /// keeps the item's line COUNT, which stars own. One inert variant instead of eight dead
    /// stat names in the enum, the roll table and two label tables.
    ///
    /// The twins that folded into a live stat are NOT here: `MatkFlat`/`MatkBp`/`IgnoreMdefBp`
    /// are serde ALIASES of `AtkFlat`/`AtkBp`/`IgnoreDefBp`, so an old line keeps its value.
    #[serde(alias = "Str", alias = "Agi", alias = "Vit", alias = "Int", alias = "Dex",
            alias = "Luk", alias = "Hit", alias = "Flee")]
    Retired(i32),
}

impl StatMod {
    /// This line's NUMBER (every variant's last field).
    pub fn value(&self) -> i32 {
        match *self {
            StatMod::MaxHp(v) | StatMod::MaxHpBp(v)
            | StatMod::AtkBp(v) | StatMod::AtkFlat(v)
            | StatMod::DefFlat(v)
            | StatMod::CritBp(v) | StatMod::Retired(v) | StatMod::CritDmgBp(v) | StatMod::HealBp(v)
            | StatMod::IgnoreDefBp(v) | StatMod::CastBp(v)
            | StatMod::CooldownBp(v) | StatMod::DrVsRanged(v) | StatMod::MoveSpeedBp(v)
            | StatMod::ChargeRateBp(v) | StatMod::BreakDmgBp(v)
            | StatMod::BreakBuildupBp(v) | StatMod::DodgeBp(v)
            | StatMod::DmgVsBossBp(v) | StatMod::HpRegenFlat(v) | StatMod::DefBp(v)
            | StatMod::XpGainBp(v) | StatMod::GilGainBp(v) | StatMod::SoulGainBp(v)
            | StatMod::DropRateBp(v) | StatMod::DmgReduceFlat(v) => v,
            StatMod::DmgVsElement(_, v) | StatMod::ResistElement(_, v)
            | StatMod::ElementDmgBp(_, v) => v,
            StatMod::DmgVsRace(_, v) | StatMod::DrVsRace(_, v) => v,
            StatMod::DmgVsSize(_, v) | StatMod::DrVsSize(_, v) => v,
            StatMod::ResistStatusBp(_, v) => v,
        }
    }

    /// The same line carrying a different number (variant + element/race/size/status kept).
    pub fn with_value(&self, v: i32) -> StatMod {
        match *self {
            StatMod::MaxHp(_) => StatMod::MaxHp(v),
            StatMod::MaxHpBp(_) => StatMod::MaxHpBp(v),
            StatMod::AtkBp(_) => StatMod::AtkBp(v),
            StatMod::AtkFlat(_) => StatMod::AtkFlat(v),
            StatMod::DefFlat(_) => StatMod::DefFlat(v),
            StatMod::CritBp(_) => StatMod::CritBp(v),
            StatMod::CritDmgBp(_) => StatMod::CritDmgBp(v),
            StatMod::HealBp(_) => StatMod::HealBp(v),
            StatMod::IgnoreDefBp(_) => StatMod::IgnoreDefBp(v),
            StatMod::CastBp(_) => StatMod::CastBp(v),
            StatMod::CooldownBp(_) => StatMod::CooldownBp(v),
            StatMod::DrVsRanged(_) => StatMod::DrVsRanged(v),
            StatMod::MoveSpeedBp(_) => StatMod::MoveSpeedBp(v),
            StatMod::DmgVsElement(x, _) => StatMod::DmgVsElement(x, v),
            StatMod::ResistElement(x, _) => StatMod::ResistElement(x, v),
            StatMod::DmgVsRace(x, _) => StatMod::DmgVsRace(x, v),
            StatMod::DrVsRace(x, _) => StatMod::DrVsRace(x, v),
            StatMod::DmgVsSize(x, _) => StatMod::DmgVsSize(x, v),
            StatMod::DrVsSize(x, _) => StatMod::DrVsSize(x, v),
            StatMod::ResistStatusBp(c, _) => StatMod::ResistStatusBp(c, v),
            StatMod::ElementDmgBp(x, _) => StatMod::ElementDmgBp(x, v),
            StatMod::ChargeRateBp(_) => StatMod::ChargeRateBp(v),
            StatMod::BreakDmgBp(_) => StatMod::BreakDmgBp(v),
            StatMod::BreakBuildupBp(_) => StatMod::BreakBuildupBp(v),
            StatMod::DodgeBp(_) => StatMod::DodgeBp(v),
            StatMod::DmgVsBossBp(_) => StatMod::DmgVsBossBp(v),
            StatMod::HpRegenFlat(_) => StatMod::HpRegenFlat(v),
            StatMod::DefBp(_) => StatMod::DefBp(v),
            StatMod::XpGainBp(_) => StatMod::XpGainBp(v),
            StatMod::GilGainBp(_) => StatMod::GilGainBp(v),
            StatMod::SoulGainBp(_) => StatMod::SoulGainBp(v),
            StatMod::DropRateBp(_) => StatMod::DropRateBp(v),
            StatMod::DmgReduceFlat(_) => StatMod::DmgReduceFlat(v),
            StatMod::Retired(_) => StatMod::Retired(v),
        }
    }

    /// True when two lines are the SAME display family — same variant AND same parameter, so
    /// `DmgVsElement(Fire)` and `DmgVsElement(Water)` stay apart while two Fire lines merge.
    pub fn same_family(&self, other: &StatMod) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
            && self.with_value(0) == other.with_value(0)
    }

    /// SUM same-family lines, keeping first-appearance order. The star ladder deliberately
    /// emits one entry per source (base ++ per-star step ++ each rank bump), which the stat
    /// pipeline just adds up — but a TOOLTIP must not print "+5 STR +1 STR +1 STR +1 STR".
    /// Display code calls this; the combat path never needs it.
    pub fn merged(mods: &[StatMod]) -> Vec<StatMod> {
        let mut out: Vec<StatMod> = Vec::new();
        for m in mods {
            match out.iter_mut().find(|x| x.same_family(m)) {
                Some(slot) => *slot = slot.with_value(slot.value().saturating_add(m.value())),
                None => out.push(*m),
            }
        }
        out
    }
}

/// One IRREGULAR star bump on a card: at `star` (and every rank above it) these mods join
/// the card's total. Additive with the lower ranks — the `ExemplarStar` model, addressed by
/// an explicit rank number so a card can skip stars (STR at 1★/3★/5★ only).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CardStarRank {
    pub star: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mods: Vec<StatMod>,
}

/// An item instance — what an entity holds in inventory/equipped, or
/// what's sitting on the ground waiting to be picked up.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Item {
    /// Stable string identity, e.g. "sword_short", "potion_minor_hp".
    /// Phase 2 economy uses this as the catalog key.
    pub def_id: String,
    pub kind: ItemKind,
    /// Human-facing display name (e.g. "Phoenix Tear"). Empty → the UI falls
    /// back to `def_id`. Skipped in serde when empty so name-less gear saves +
    /// goldens stay byte-identical.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Flat bonus added to wearer's effective atk when equipped. A staff's number lives here too
    /// (0.0.437 — MATK is gone; the catalog folded every `matk_bonus` into `atk_bonus`).
    pub atk_bonus: i32,
    /// ATTACK-SPEED line in basis points (0.0.367 — was `aspd_base`, RO ASPD
    /// points). 500 = "+5% ATK SPD", read straight as written: the bonus is additive
    /// and multiplies the wearer's base rate, so this line is worth the same
    /// percentage on every unit (which is what lets a market price it). 0 on
    /// ordinary gear; the unique weapons carry the only non-zero values today.
    #[serde(default)]
    pub atk_speed_bp: i32,
    /// Weapon class (1H sword, dagger, …) for weapon-type-gated effects.
    /// `None` for armor / non-weapons. Default keeps old saves valid.
    #[serde(default)]
    pub weapon_type: WeaponType,
    /// Off-hand kind (shield / tome / quiver / hidden knife) — the off-hand gate reads it, and
    /// `has_shield` asks for `Shield` rather than "in the Shield slot and not a weapon".
    /// Skip-if-None keeps every other item's save bytes identical; `migrate_item` stamps it from
    /// the def, so a shield saved before 0.0.446 becomes a `Shield` on load.
    #[serde(default, skip_serializing_if = "OffhandType::is_none")]
    pub offhand_type: OffhandType,
    /// Weapon LEVEL (RO-style tier, 1-4) — the RO weapon class, still read by the card
    /// socket rule (`weapon_max_card_slots`). 1 for unset/non-weapons. Skip-if-1 keeps saves +
    /// goldens byte-identical.
    #[serde(default = "default_weapon_level", skip_serializing_if = "is_one_u8")]
    pub weapon_level: u8,
    /// Same for defense.
    pub def_bonus: i32,
    /// RESONANCE stars carried by an EXEMPLAR item (0..=`EXEMPLAR_MAX_STARS`) — its own
    /// field, and NOT `stars` (gear bonus-line
    /// stars): all three are separate systems. Preserved across unequip/trade; consumed
    /// into `Exemplar.stars` on equip/merge. Skip-if-0 keeps every non-Exemplar item +
    /// goldens byte-identical. Always 0 for non-Exemplar kinds.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub resonance_stars: u8,
    /// REGALIA Enhancement LEVEL (0..`REGALIA_MAX_LEVEL`) — the SECOND Regalia axis (fed by R
    /// fodder), separate from `resonance_stars` (dupes). Scales the
    /// Regalia's main/flat stat (`per_level`). Preserved across unequip; consumed into
    /// `Regalia.level` on sync. Skip-if-0 keeps every non-Regalia item + goldens byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub enhance_level: u8,
    /// Enhance XP banked INTO `enhance_level` toward the next level (0.0.490; 0 at the cap).
    /// Skip-if-0 keeps saves byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub enhance_xp: u32,
    /// EXEMPLAR carriers live PERMANENTLY in `owned_exemplars` (0.0.221) — this marks
    /// which party member currently has this one equipped (0 = available). Equip/unequip
    /// just flip this; the carrier is never consumed, so Resonance (FeedExemplar) can raise
    /// ANY copy like a card. Skip-if-0 keeps non-Exemplars + goldens byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub equipped_by: u32,
    pub rarity: Rarity,
    /// Number of identical units this stack carries. Phase 0:
    /// stacking only applies to `Material` kind (scrap etc.); weapons
    /// and armor stay 1-per-row because each instance refines
    /// independently. Drop merge logic lives in `drops::drop_on_death`.
    pub quantity: u32,

    /// Gil paid PER UNIT when sold (materials/junk only). 0 for gear/weapons.
    /// Skipped in serde when 0 so non-material saves + goldens stay
    /// byte-identical; materials carry a real value.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub sell_value: u32,

    /// Auto-use behaviour when this is a `Consumable` (None for gear/junk).
    /// Skipped in serde when absent → non-consumable saves stay byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumable: Option<ConsumableSpec>,

    // --- Phase 2 economy reserved fields (GDD §3.3) ---
    /// Whether this item can ever be listed on the Phase 2 Steam
    /// Market. Phase 0 doesn't act on this, but every drop now
    /// populates it correctly so we don't have a data migration later.
    pub marketable: bool,
    /// Which emission cohort this item belongs to (used by Phase 2
    /// economy throttling). Empty string in Phase 0 = "uncategorized".
    pub emission_group: String,
    /// Which quota class for trade-restriction policy.
    pub quota_class: String,

    /// Per-instance identity for the tradeable economy + server-roll refine binding.
    /// `None` until the item first needs one — assigned on the FIRST refine from
    /// `hash::instance_id(rng_fingerprint, tick, def_id)` (deterministic in replay, so a
    /// server can recompute it and BIND a refine roll to THIS exact instance; §6.4.1),
    /// then stable for every later refine. Skip-if-none keeps un-refined gear + goldens
    /// byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,

    /// Paper-doll wear position. `None` for materials and for hand-built
    /// test/legacy gear that predates the multi-slot system — those fall back
    /// to a slot derived from `kind` (Weapon→Weapon, anything else→Armor) so
    /// old single-weapon/single-armor saves keep working. Real game_data gear
    /// sets this explicitly (a shield and a body armor are both `ItemKind::Armor`
    /// but occupy DIFFERENT slots).
    #[serde(default)]
    pub equip_slot: Option<EquipSlot>,

    /// RETIRED (0.0.388): card sockets are the GRADE's now (`socket_count`). Kept so an old save
    /// still deserializes; migration zeroes it. Skip-if-0 keeps goldens byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub card_slots: u8,
    /// Which card PANEL class this Card belongs to (Card-kind only) — it equips into
    /// that class's panel and is gated by the matching gear's sockets. `None` for
    /// gear/weapons/consumables. Skip-if-none keeps saves clean.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_slot: Option<CardClass>,
    /// The mods this card grants while ACTIVE in the panel at 0★ — Card-kind only (the
    /// Epic/normal effect). Same `StatMod` vocabulary as gear/overlays (0.0.243).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub card_mods: Vec<StatMod>,
    /// Mods added ONCE PER STAR (× `star`) — the "ไต่ทุกดาว" ladder for a card's headline
    /// number (ATK/HP/FLEE…). AUTHORED constants, never a % of the base: the old
    /// `×(1 + 0.2·star)` fold FLOORED away every step of a small line (a +1 STR card was
    /// bit-identical from 0★ to 4★). A drawback line simply gets no step.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub card_star_step: Vec<StatMod>,
    /// IRREGULAR star bumps — mods that join only at specific ranks (user 2026-07-29: the
    /// small flat stats step at 1★/3★/5★ while the headline number climbs every star).
    /// Cumulative: every rank with `star <= item.star` applies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub card_star_ranks: Vec<CardStarRank>,
    /// The EXTRA mods a FOIL (shiny) copy grants ON TOP of `card_mods` — authored PER CARD
    /// (think "+how much more", not a fixed ×2). Empty ⇒ a foil grants nothing extra.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub foil_mods: Vec<StatMod>,
    /// Per-star ladder for the FOIL extra (twin of `card_star_step`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub foil_star_step: Vec<StatMod>,
    /// The small PERMANENT mods granted when this card is BOUND (instead of equipped).
    /// Much smaller than `card_mods`, and NOT star-scaled — binding is a flat floor.
    /// Card-kind only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bind_mods: Vec<StatMod>,
    /// Card ELEMENT (Card-kind only) — drives the element-coloured card FRAME in the UI
    /// (a Water card = blue frame, Fire = orange, …). Neutral default → skipped so
    /// non-card / neutral gear stays byte-identical.
    #[serde(default, skip_serializing_if = "is_neutral_element")]
    pub element: crate::grid::Element,
    /// Card STAR level (0-based; fuse duplicates to raise it). RESERVED for the
    /// deferred leveling system — carried now so the effect layer reads it later.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub star: u8,
    /// Card FOIL variant (the ~1/20 special: extra effect + shiny UI). RESERVED for
    /// the deferred variant/leveling system. Skip-if-false keeps saves clean.
    #[serde(default, skip_serializing_if = "is_false")]
    pub foil: bool,
    /// ACCOUNT-BOUND (0.0.227): use-only — refuses every liquidation path (Gil-market
    /// listing, vendor sale; the Steam promote endpoint checks it server-side too).
    /// Stamped by the server on granted rewards (battle-pass free track etc.); nothing
    /// in the sim ever sets it. Per-instance state, preserved across migration.
    #[serde(default, skip_serializing_if = "is_false")]
    pub bound: bool,
    /// Rolled ENCHANT lines (gear only; fills the star slots). Empty until the
    /// enchant/orb systems roll into them — skip-if-empty keeps every existing
    /// save and golden byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bonuses: Vec<StatMod>,
    /// UNIQUE gear (fixed-identity items, the `unique_*` defs): stars never roll
    /// at drop — its power is authored, not enchanted. A SEPARATE tag from
    /// `Rarity` (user 2026-07-05: "Legendary is not unique") — Legendary is a
    /// reachable drop tier (4★), unique-ness is its own axis. Def-driven, so
    /// migration re-stamps it from the catalog for free.
    #[serde(default, skip_serializing_if = "is_false")]
    pub unique: bool,
    /// Has a Steam identity (`ItemDef.steam_itemdef`, 0.0.480): one copy in the game is one item
    /// on Steam, so it never stacks (user: "ไม่ซ้อน stack เพื่อความเข้าใจง่าย"). Def-driven —
    /// migration and every grant re-stamp it from the catalog.
    #[serde(default, skip_serializing_if = "is_false")]
    pub steam: bool,
    /// GEAR GRADE (0.0.372) — the power axis. Per-instance and EARNED: a drop is rolled into a
    /// grade, fusion moves it up, and nothing else may change it. Skip-if-Common keeps every
    /// existing save and golden byte-identical, and makes an un-graded old item read as the
    /// ladder's floor rather than as a missing value.
    #[serde(default, skip_serializing_if = "is_common_grade")]
    pub grade: Grade,
    /// The designer's exception to the grade formula for this (def × grade), if any — see
    /// `StatOverride` / catalog `gear_overrides`. Skip-if-none keeps every existing save and
    /// golden byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stat_override: Option<StatOverride>,
    /// STAR STONE effect — set only on orb defs (`star_stone_*`). Def-driven.
    /// Deserialized LENIENTLY (`orb_lenient`): a retired variant loads as `None`
    /// instead of failing the whole save — `InsertRune` died with the rune system
    /// in 0.0.369 and real saves still carry rune stones.
    #[serde(default, deserialize_with = "orb_lenient", skip_serializing_if = "Option::is_none")]
    pub orb: Option<OrbEffect>,
    /// The product's SUBSTAT lines (0.0.384): a pure function of (def × grade) through the
    /// catalog's `loot_rules.substats`, stamped by `assign_grade` and refreshed on load — so a
    /// re-tuned table reaches every copy without a migration, and no copy can disagree. Never
    /// rolled; never edited per instance.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<StatMod>,
    /// Kept in the materials ledger, not the bag — see `ItemDef::ledger`. Def-driven; the bank
    /// path credits `GameState.materials` and consumes the item.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ledger: bool,
    /// Index of the enchant line LOCKED by a Death Star Stone (survives every
    /// reroll). Per-instance EARNED state — preserved across migration like
    /// star/foil. None = nothing locked (the default for all existing saves).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked_slot: Option<u8>,
    /// Player LOCK on an owned Regalia (0.0.489): Salvage and Resonance feed refuse a locked copy.
    /// Per-instance, preserved across migration. Skip-if-false keeps saves byte-identical.
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
    /// GEMS socketed into this piece (0.0.386) — the OWNER's layer on a fungible product, one
    /// entry per filled socket, in socket order. Each carries the lines it rolled when it went
    /// in, so the fight (which holds no catalog) reads them straight. A piece with any gem is
    /// no longer a clean product (`market_clean`). Skip-if-empty keeps every save and golden
    /// byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gems: Vec<SocketedGem>,
    /// The gem SET this item is, for a gem def (`gems::GemRules::sets`); empty for everything
    /// else. Stamped from the def so the drop path can re-key a rolled grade without a catalog.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub gem_set: String,
}

/// One gem sitting in a socket: WHICH gem (its def, whose id carries set + grade) and the lines
/// it rolled on the way in (`gems::roll_lines`). The lines are per-instance on purpose — the
/// ±window roll is the "mine" in "my gear" — and are what `combat::stat_mods` folds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SocketedGem {
    pub def_id: String,
    pub lines: Vec<StatMod>,
}

/// WHICH WEAPON TYPES A CHARACTER MAY WIELD (0.0.368, user 2026-09-14: "Seraphine เอาดาบอย่างเดียว
/// / rapis เอาแค่คทา / acolyte ถือ staff"). ONE family per character — a strict pairing, not the
/// permissive RO matrix this replaces. Read from the character's `EXEMPLAR_LINES` row, so a new
/// unit declares its family where it declares its name. NOT keyed on Doctrine — a Doctrine says
/// how a unit FIGHTS and nothing about what it holds: lapis is a Striker with a staff, and the
/// next Striker could carry daggers.
///
/// An EMPTY list means unrestricted, which covers hand-built test entities and any id this table
/// has never heard of. That silence is right for fixtures and dangerous for a real character, so
/// `weapon_gate_tests` fails the build if a creatable unit ever falls through it.
///
/// It is a CODE table rather than a catalog field on purpose: `apply_input_with` takes
/// `data: Option<&GameData>`, so an equip rule that read the catalog would apply for the client
/// (which passes `Some`) and not for a headless tool (which passes `None`) — the replay would
/// diverge on who was holding what. A static table is readable from everywhere, always.
pub fn weapon_types_for_unit(unit_def: &str) -> &'static [WeaponType] {
    match EXEMPLAR_LINES.iter().find(|l| l.id == unit_def) {
        Some(l) => match l.weapon {
            WeaponType::Sword1H => &[WeaponType::Sword1H],
            WeaponType::Staff => &[WeaponType::Staff],
            WeaponType::Dagger => &[WeaponType::Dagger],
            WeaponType::Bow => &[WeaponType::Bow],
            WeaponType::Sword2H => &[WeaponType::Sword2H],
            WeaponType::Mace => &[WeaponType::Mace],
            WeaponType::None => &[],
        },
        None => &[],
    }
}

/// THE CHARACTER, not the stat block, decides the family (0.0.368). An Exemplar IS the character;
/// the worn Exemplar's row answers first, the unit def's row when no Exemplar is worn (a bare
/// roster unit) or the Exemplar id is unknown. Since 0.0.445 both resolve through the same table.
/// Share of its own ATK/DEF a weapon or off-hand keeps on a character outside its family
/// (user 2026-09-29: -20% of the piece, not of the character's output). Substat lines are whole.
pub const OFF_FAMILY_GEAR_BP: i64 = 8_000;

fn off_family_scaled(v: i32, preferred: bool) -> i32 {
    if preferred { v } else { (i64::from(v) * OFF_FAMILY_GEAR_BP / 10_000) as i32 }
}

pub fn weapon_types_for_character(exemplar_id: Option<&str>, unit_def: &str) -> &'static [WeaponType] {
    if let Some(fam) = exemplar_id.map(weapon_types_for_unit).filter(|f| !f.is_empty()) {
        return fam;
    }
    weapon_types_for_unit(unit_def)
}

/// Every id the gate can answer for — every Exemplar line, shipped or planned.
pub fn gated_ids() -> impl Iterator<Item = &'static str> {
    EXEMPLAR_LINES.iter().map(|l| l.id)
}

/// The character ids that may wield `wt` — the inverse lookup, for tooltips.
pub fn units_that_can_wield(wt: WeaponType) -> Vec<&'static str> {
    if matches!(wt, WeaponType::None) {
        return Vec::new();
    }
    let mut out: Vec<&'static str> = gated_ids()
        .filter(|u| weapon_types_for_unit(u).contains(&wt))
        .collect();
    out.sort_unstable();
    out
}

/// The character ids whose weapon family carries off-hand kind `o` — the inverse of
/// `offhand_types_for_character`, for tooltips.
pub fn units_that_can_carry(o: OffhandType) -> Vec<&'static str> {
    if o.is_none() {
        return Vec::new();
    }
    let mut out: Vec<&'static str> = gated_ids()
        .filter(|u| weapon_types_for_unit(u).iter().any(|w| offhand_for_weapon(*w) == o))
        .collect();
    out.sort_unstable();
    out
}

/// Reserved drop-table sentinel for a "no drop this kill" outcome.
/// A drop row can carry this `item_id` with a weight: `roll_drop` still
/// takes its single PRNG draw (contract preserved), but `drop_on_death`
/// recognizes the sentinel and banks NOTHING — it just widens the table's
/// total weight so real gear becomes appropriately rare (e.g. weapon 2 /
/// armor 3 / nothing 95 → ~5% gear per kill). Defined as a real item in
/// `game_data` so load-time validation passes; it never reaches the stash.
pub const NOTHING_DROP_ID: &str = "nothing";

impl Item {
    /// Convenience constructor for tests + hardcoded drop tables.
    /// Quantity defaults to 1; stack merging happens at drop time.
    pub fn new(def_id: &str, kind: ItemKind, atk_bonus: i32, def_bonus: i32, rarity: Rarity) -> Self {
        Self {
            def_id: def_id.to_string(),
            kind,
            name: String::new(),
            atk_bonus,
            atk_speed_bp: 0,
            weapon_type: WeaponType::None,
            offhand_type: OffhandType::None,
            weapon_level: 1,
            def_bonus,
            resonance_stars: 0,
            enhance_level: 0,
            enhance_xp: 0,
            equipped_by: 0,
            rarity,
            quantity: 1,
            sell_value: 0,
            consumable: None,
            marketable: true,
            emission_group: String::new(),
            quota_class: String::new(),
            instance_id: None,
            equip_slot: None,
            card_slots: 0,
            card_slot: None,
            card_mods: Vec::new(),
            card_star_step: Vec::new(),
            card_star_ranks: Vec::new(),
            foil_mods: Vec::new(),
            foil_star_step: Vec::new(),
            bind_mods: Vec::new(),
            element: crate::grid::Element::Physical,
            star: 0,
            foil: false,
            bound: false,
            bonuses: Vec::new(),
            unique: false,
            steam: false,
            grade: Grade::Common,
            stat_override: None,
            orb: None,
            lines: Vec::new(),
            ledger: false,
            locked_slot: None,
            locked: false,
            gems: Vec::new(),
            gem_set: String::new(),
        }
    }

    /// Card sockets on this piece — the GRADE's (`gems::CARD_SLOTS_BY_GRADE`, 0.0.388), for any
    /// gear; nothing else has any. The def's `card_slots` and the weapon-level cap are retired.
    pub fn socket_count(&self) -> u8 {
        if !matches!(self.kind, ItemKind::Weapon | ItemKind::Armor) {
            return 0;
        }
        crate::gems::card_slots_at(self.grade)
    }

    /// True if the character described by (`exemplar_id`, `unit_def`) may equip this item
    /// (0.0.368). Only WEAPONS are gated, by `weapon_types_for_character`: armour, accessories
    /// and headgear fit everyone, and a weapon whose type is `None` (bare-hand / hand-built) is
    /// unrestricted so old gear still equips. The EXEMPLAR wins where there is one — it is the
    /// character; the unit is the stat block under it.
    /// Every weapon and off-hand fits every character (user 2026-09-29): one outside the
    /// character's family is worn at `OFF_FAMILY_GEAR_BP` of its own ATK/DEF (`worn_atk`).
    pub fn can_be_equipped_by(&self, _exemplar_id: Option<&str>, _unit_def: &str) -> bool {
        true
    }

    /// Whether this piece is in the character's weapon family — the weapon types the unit (or its
    /// worn Exemplar) wields, and the off-hand kind that family carries. Armour, type-less
    /// weapons and unknown characters (empty family, e.g. fixtures) always count as preferred.
    pub fn is_preferred_by(&self, exemplar_id: Option<&str>, unit_def: &str) -> bool {
        if !self.offhand_type.is_none() {
            let allowed = weapon_types_for_character(exemplar_id, unit_def);
            return allowed.is_empty()
                || allowed.iter().any(|w| offhand_for_weapon(*w) == self.offhand_type);
        }
        if !matches!(self.kind, ItemKind::Weapon) || matches!(self.weapon_type, WeaponType::None) {
            return true;
        }
        let allowed = weapon_types_for_character(exemplar_id, unit_def);
        allowed.is_empty() || allowed.contains(&self.weapon_type)
    }

    /// True if this item kind merges into an existing stack of the same
    /// `def_id` instead of pushing a new inventory slot.
    pub fn is_stackable(&self) -> bool {
        !self.steam
            && matches!(
                self.kind,
                ItemKind::Material | ItemKind::Junk | ItemKind::Consumable | ItemKind::Card
            )
    }

    /// ITEM LEVEL (0.0.136, user: "S1 = item level : 1") — the def's `_sN`
    /// tier badge surfaced as a first-class number (gear only; everything
    /// else is 0 = no level concept). Same parse the enchant tier window
    /// uses, so tooltip and roll rules can never disagree.
    pub fn item_level(&self) -> u32 {
        if matches!(self.kind, ItemKind::Weapon | ItemKind::Armor) {
            crate::gear_roll::def_tier(&self.def_id)
        } else {
            0
        }
    }

    /// Minimum character LEVEL to equip: `(item_level − 1) × 10` — s1 = Lv1
    /// (starter gear must never gate), s2 = Lv10 … s7 = Lv60. Sits ~15-20
    /// levels under where each tier naturally drops (s2@b4≈Lv25, s3@b6≈Lv40,
    /// s4@b10≈Lv65), so a self-farmer never hits the wall — the gate only
    /// tempers market twinking. 0 for non-gear.
    pub fn level_req(&self) -> u32 {
        match self.item_level() {
            0 | 1 => 1,
            n => (n - 1) * 10,
        }
    }

    /// Every `StatMod` this CARD grants at its current star (and foil), resolved from the
    /// authored ladders: base ++ `card_star_step × star` ++ every `card_star_ranks` entry at
    /// or below the star, then the same for the FOIL extra. Pure integer, no scaling fold —
    /// what is authored is what applies. Empty for non-cards, so it is safe to call on any
    /// item. Feeds `stat_mods::for_each_mod` (the ONE modifier pipeline).
    pub fn active_card_mods(&self) -> Vec<StatMod> {
        let mut out = self.card_mods_at_star();
        if self.foil {
            out.extend(self.foil_mods_at_star());
        }
        out
    }

    /// The EPIC (non-foil) half at this card's star: base ++ `card_star_step × star` ++ every
    /// `card_star_ranks` entry at or below the star.
    pub fn card_mods_at_star(&self) -> Vec<StatMod> {
        let star = self.star.min(CARD_STAR_MAX);
        let mut out = self.card_mods.clone();
        for _ in 0..star {
            out.extend_from_slice(&self.card_star_step);
        }
        for r in &self.card_star_ranks {
            if star >= r.star {
                out.extend_from_slice(&r.mods);
            }
        }
        out
    }

    /// The FOIL EXTRA half at this card's star (what a shiny copy adds ON TOP). Independent of
    /// the `foil` flag so a tooltip can show "what a foil would add".
    pub fn foil_mods_at_star(&self) -> Vec<StatMod> {
        let star = self.star.min(CARD_STAR_MAX);
        let mut out = self.foil_mods.clone();
        for _ in 0..star {
            out.extend_from_slice(&self.foil_star_step);
        }
        out
    }

    /// THE ONE WAY a piece receives its grade. Sets the grade and the designer's override for
    /// (def × grade) together, from whatever override rows the caller has in hand — the catalog's
    /// (fusion, migration) or the baked template's (the drop bank, which holds no catalog).
    ///
    /// Three sites assign grades and all three go through here, so a fourth one cannot forget the
    /// override and mint a copy that disagrees with every other copy of the same product — which
    /// would be both a replay divergence and, on a market, two prices for one name.
    pub fn assign_grade(&mut self, grade: Grade, overrides: &[crate::data::GearOverride], lines_by_grade: &[Vec<StatMod>]) {
        self.grade = grade;
        self.stat_override = overrides
            .iter()
            .find(|o| o.def_id == self.def_id && o.grade == grade)
            .map(|o| StatOverride { atk: o.atk, def: o.def });
        // The product's SUBSTAT lines at this grade (0.0.384) — stamped here, with the grade,
        // from the same pre-resolved table every other path uses, so every copy agrees.
        self.lines = lines_by_grade.get(grade.step()).cloned().unwrap_or_default();
    }

    /// This piece's ATK as the fight sees it: the AUTHORED base scaled to its grade. Derived at
    /// read time from `grade` (never baked into the instance) for the same reason
    /// `star_base_bonus` is — an instance that stored its scaled numbers would need a migration
    /// every time the ladder is tuned, and two copies of one grade could disagree.
    pub fn graded_atk(&self) -> i32 {
        self.stat_override.and_then(|o| o.atk).unwrap_or_else(|| self.grade.scale_raw(self.atk_bonus))
    }
    /// `graded_atk` as the wearer gets it: `OFF_FAMILY_GEAR_BP` of it outside their family.
    pub fn worn_atk(&self, exemplar_id: Option<&str>, unit_def: &str) -> i32 {
        off_family_scaled(self.graded_atk(), self.is_preferred_by(exemplar_id, unit_def))
    }
    /// `graded_def` as the wearer gets it. See `worn_atk`.
    pub fn worn_def(&self, exemplar_id: Option<&str>, unit_def: &str) -> i32 {
        off_family_scaled(self.graded_def(), self.is_preferred_by(exemplar_id, unit_def))
    }
    /// This piece's DEF at its grade. See `graded_atk`.
    pub fn graded_def(&self) -> i32 {
        self.stat_override.and_then(|o| o.def).unwrap_or_else(|| self.grade.scale_raw(self.def_bonus))
    }
    /// This piece's attack-speed line at its grade — the PERCENT ladder (×1.15/step), not the raw
    /// one: a rate bonus that climbed ×10.6 would hit the tick ceiling long before Cosmic.
    pub fn graded_atk_speed_bp(&self) -> i32 {
        self.grade.scale_pct(self.atk_speed_bp as i32)
    }

    /// The PRODUCT this piece is, for any system that treats two items as interchangeable —
    /// a market listing, a commodity stack, a "you already own one" check.
    ///
    /// `def_id` alone is NOT the product. Two `sword_s7` of different grades differ by up to ×10.6
    /// in power; pooled under one name they become a lemon market, where the cheapest listing is
    /// always the weakest copy and no buyer can tell. The grade is therefore part of the identity,
    /// exactly as it is part of the displayed NAME ("Beyond Adamant Saber").
    ///
    /// Enchanted / carded instances are NOT products at all — the design binds them permanently
    /// and they never reach a market, which is what keeps this key honest: everything it can name
    /// really is interchangeable.
    pub fn product_key(&self) -> String {
        // A gem's def id already names its grade (`gem_strife_g3`); the ledger and the commodity
        // market key on that id, so the suffix would only split one stock into two names.
        if matches!(self.grade, Grade::Common) || !self.gem_set.is_empty() {
            self.def_id.clone()
        } else {
            format!("{}#{:?}", self.def_id, self.grade)
        }
    }

    /// GEM sockets this piece has — the grade's count (`gems::gem_slots_at`), 0 for anything
    /// that is not gear. Which gems fit is the set's business (`GemGroup::fits`).
    pub fn gem_slots(&self) -> u8 {
        if !matches!(self.kind, ItemKind::Weapon | ItemKind::Armor) {
            return 0;
        }
        crate::gems::gem_slots_at(self.grade)
    }

    /// True when this piece is still the plain PRODUCT its name says — no gem in any socket and
    /// not account-bound — and so may be listed on an item market. A gemmed piece is the owner's
    /// (TBH's "strip every socket before selling"); it is not bound, just not clean until emptied.
    pub fn market_clean(&self) -> bool {
        self.gems.is_empty() && !self.bound
    }

    /// Paper-doll slot this piece occupies. Uses the explicit `equip_slot` when
    /// present; otherwise derives one from `kind` so legacy/test gear (which
    /// only ever had one Weapon + one Armor) still de-dups correctly.
    pub fn slot(&self) -> EquipSlot {
        match self.equip_slot {
            Some(s) => s,
            None => match self.kind {
                ItemKind::Weapon => EquipSlot::Weapon,
                _ => EquipSlot::Armor,
            },
        }
    }
}

/// One row of a weighted drop table.
///
/// Weights are integer relative weights (NOT basis points). Total
/// weight = sum of all `weight` fields in the table. The roll picks
/// one entry with probability `entry.weight / total`.
///
/// Why not basis points? Drop tables grow CSV rows over time and tend
/// to want relative tuning ("rare drop is 1/10 as common as uncommon")
/// rather than absolute %. Letting designers write integers like
/// (1000, 100, 10, 1) is more ergonomic than computing percentages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DropEntry {
    /// This def's `gear_overrides`, every grade — baked so the bank path (which holds no
    /// catalog) can stamp the right one once the grade is rolled. Empty for almost every def.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<crate::data::GearOverride>,
    /// The def's SUBSTAT lines at every grade (index = `Grade::step()`), baked for the same
    /// reason `overrides` is: the bank path holds no catalog. Empty for non-gear rows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines_by_grade: Vec<Vec<StatMod>>,
    pub template: Item,
    pub weight: u32,
    /// SERVER-ROLL candidate rate (basis points PER KILL) — the two-stage rare pipeline
    /// (server-roll-drops-design.md). An entry with `server_bp > 0` keeps `weight` 0
    /// (never client-rolled); each kill instead takes one gated draw at this rate and,
    /// on a hit, accrues a PENDING `RareCandidate` that the server resolves with a keyed
    /// nonce (~0.1%) at the next save-verify. Skip-if-0 keeps every normal entry (and
    /// all existing saves/goldens) byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub server_bp: u32,
}

impl DropEntry {
    pub fn new(template: Item, weight: u32) -> Self {
        Self { template, weight, server_bp: 0, overrides: Vec::new(), lines_by_grade: Vec::new() }
    }
}

#[cfg(test)]
mod equip_tests {

    use super::*;

    /// The ladder's SHAPE, pinned end to end. Two multipliers that must NOT be one: raw ×10.6 is
    /// what makes a Cosmic weapon worth six Divines, and percent ×3.5 is what stops a percent
    /// line from running off its own ceiling (block 100%, attack speed vs the tick) three grades
    /// early. If a future tune moves either, it moves DELIBERATELY, through this test.
    #[test]
    fn the_grade_ladder_scales_raw_and_percent_on_different_curves() {
        assert_eq!(GRADES.len(), 10, "ten grades — the fusion pacing is computed against this");
        assert_eq!(Grade::Common.step(), 0, "Common is the floor");
        assert_eq!(Grade::Cosmic.step(), 9);
        assert_eq!(Grade::Common.raw_bp(), 10_000, "the floor multiplies by 1.00, never 0");
        assert_eq!(Grade::Common.pct_bp(), 10_000);
        assert_eq!(Grade::Cosmic.raw_bp(), 106_045, "raw ×10.60 across the ladder (TBH: ×10.8-10.9)");
        assert_eq!(Grade::Cosmic.pct_bp(), 35_179, "percent ×3.52 (TBH: ×2.2-4.0) — flatter ON PURPOSE");
        assert!(Grade::Cosmic.pct_bp() * 2 < Grade::Cosmic.raw_bp(),
            "the percent axis must stay far below the raw one, or percent lines break their caps");
        // Monotonic and strictly rising: a grade that does not beat the one below is a grade
        // nobody would fuse for, and six-into-one is the only way up.
        for w in GRADES.windows(2) {
            assert!(w[1].raw_bp() > w[0].raw_bp(), "{:?} must beat {:?} on raw", w[1], w[0]);
            assert!(w[1].pct_bp() > w[0].pct_bp(), "{:?} must beat {:?} on percent", w[1], w[0]);
        }
        assert_eq!(Grade::Cosmic.next(), None, "the top of the ladder has no next grade");
        assert_eq!(Grade::Common.next(), Some(Grade::Uncommon));
    }

    /// A grade step must never read as "+0" on a real stat, and must never invent one on a zero.
    /// Rounding UP is what guarantees the first half of that (the same rule the ★ floor uses).
    #[test]
    fn grade_scaling_rounds_up_but_never_invents_a_stat() {
        assert_eq!(Grade::Common.scale_raw(140), 140, "the floor is identity, exactly");
        assert_eq!(Grade::Uncommon.scale_raw(1), 2, "1 × 1.30 rounds UP to 2, not down to 1");
        assert_eq!(Grade::Cosmic.scale_raw(0), 0, "a stat the piece does not have stays absent");
        assert_eq!(Grade::Cosmic.scale_pct(0), 0);
        assert_eq!(Grade::Uncommon.scale_raw(-10), -13, "a NEGATIVE line scales too, away from zero");
        // The authored s7 weapon budget (1,525 ATK) at the launch ceiling, as the design's table
        // reads it: Beyond is ×4.83, which is the number the drop-odds pacing was computed on.
        assert_eq!(Grade::Beyond.scale_raw(1_525), 7_361);
    }

    /// GRADE AND RARITY ARE DIFFERENT AXES. The moment they are treated as one, an enchanted
    /// Common and a plain Legendary become indistinguishable to half the code.
    #[test]
    fn a_graded_item_scales_its_own_authored_stats_only() {
        let mut it = Item::new("sword", ItemKind::Weapon, 100, 40, Rarity::Common);
        it.atk_speed_bp = 1_000;
        assert_eq!((it.graded_atk(), it.graded_def(), it.graded_atk_speed_bp()), (100, 40, 1_000),
            "a Common piece is its authored self");
        it.grade = Grade::Immortal;
        assert_eq!(it.graded_atk(), 286, "×2.86 at Immortal");
        assert_eq!(it.graded_def(), 115);
        assert_eq!(it.graded_atk_speed_bp(), 1_749, "the speed line rides the PERCENT ladder (×1.75)");
        assert_eq!(it.rarity, Rarity::Common, "grade moved; rarity is a separate axis and did not");
    }

    #[test]
    fn item_level_and_level_req() {
        let s3 = Item::new("sword_s3", ItemKind::Weapon, 24, 0, Rarity::Common);
        assert_eq!(s3.item_level(), 3);
        assert_eq!(s3.level_req(), 20);
        let s1 = Item::new("sword_s1", ItemKind::Weapon, 8, 0, Rarity::Common);
        assert_eq!(s1.level_req(), 1);
        let starter = Item::new("starting_sword", ItemKind::Weapon, 5, 0, Rarity::Common);
        assert_eq!(starter.item_level(), 1, "badge-less legacy gear = iLv 1");
        assert_eq!(starter.level_req(), 1);
        let s7v = Item::new("armor_s7_2s", ItemKind::Armor, 0, 30, Rarity::Rare);
        assert_eq!(s7v.item_level(), 7, "socket variants share the base badge");
        assert_eq!(s7v.level_req(), 60);
        let ore = Item::new("ore_valkirion", ItemKind::Material, 0, 0, Rarity::Common);
        assert_eq!(ore.item_level(), 0, "non-gear has no level concept");
        assert_eq!(ore.level_req(), 1);
    }

    /// Any weapon or off-hand fits anyone (user 2026-09-29); outside the family the piece keeps
    /// 80% of its own ATK and DEF, and a preferred piece keeps all of it.
    #[test]
    fn off_family_gear_fits_but_keeps_eighty_percent_of_its_own_stats() {
        let mut staff = Item::new("w", ItemKind::Weapon, 100, 0, Rarity::Common);
        staff.weapon_type = WeaponType::Staff;
        assert!(staff.can_be_equipped_by(None, "seraphine"), "a staff fits Seraphine now");
        assert!(!staff.is_preferred_by(None, "seraphine"));
        assert_eq!(staff.worn_atk(None, "seraphine"), staff.graded_atk() * 8 / 10);
        assert_eq!(staff.worn_atk(None, "lapis"), staff.graded_atk(), "lapis wields staves: no cut");
        let mut shield = Item::new("s", ItemKind::Armor, 0, 50, Rarity::Common);
        shield.offhand_type = OffhandType::Shield;
        assert!(shield.can_be_equipped_by(None, "lapis"));
        assert_eq!(shield.worn_def(None, "lapis"), 40, "an off-family shield keeps 80% of its DEF");
        assert_eq!(shield.worn_def(None, "seraphine"), 50);
    }

    #[test]
    fn a_unit_prefers_only_its_own_weapon_family() {
        // Seraphine takes a 1H sword and nothing else; lapis takes a staff (user 2026-09-14).
        let w = |wt: WeaponType| {
            let mut i = Item::new("w", ItemKind::Weapon, 10, 0, Rarity::Common);
            i.weapon_type = wt;
            i
        };
        assert!(w(WeaponType::Sword1H).is_preferred_by(None, "seraphine"));
        for wt in [WeaponType::Sword2H, WeaponType::Staff, WeaponType::Bow, WeaponType::Dagger, WeaponType::Mace] {
            assert!(!w(wt).is_preferred_by(None, "seraphine"), "seraphine must refuse {wt:?}");
            if wt != WeaponType::Staff {
                assert!(!w(wt).is_preferred_by(None, "lapis"), "lapis must refuse {wt:?}");
            }
        }
        assert!(w(WeaponType::Staff).is_preferred_by(None, "lapis"));
        // Armour is never gated, and a type-less weapon (hand-built/legacy) stays wearable.
        assert!(Item::new("a", ItemKind::Armor, 0, 20, Rarity::Common).is_preferred_by(None, "lapis"));
        assert!(w(WeaponType::None).is_preferred_by(None, "lapis"));
        // An id the table never heard of is unrestricted — that is what test fixtures rely on.
        assert!(w(WeaponType::Bow).is_preferred_by(None, "not_a_unit"));
        // THE EXEMPLAR WINS: a worn Exemplar's family answers before the unit def's.
        assert!(w(WeaponType::Staff).is_preferred_by(Some("lucielle"), "seraphine"));
        assert!(!w(WeaponType::Sword1H).is_preferred_by(Some("lucielle"), "seraphine"));
        assert!(w(WeaponType::Sword1H).is_preferred_by(Some("rowena"), "lapis"));
        assert!(!w(WeaponType::Staff).is_preferred_by(Some("rowena"), "lapis"));
        // An unknown Exemplar id falls back to the unit's own family rather than opening up.
        assert!(!w(WeaponType::Bow).is_preferred_by(Some("nobody"), "lapis"));
        // The inverse lookup agrees with the forward one.
        assert_eq!(units_that_can_wield(WeaponType::Staff).contains(&"lapis"), true);
        assert!(!units_that_can_wield(WeaponType::Sword1H).contains(&"lapis"));
        assert!(units_that_can_wield(WeaponType::None).is_empty());
    }

    /// The ladder is a TABLE now, so the thing worth pinning is the table's shape rather than any
    /// one weapon: every class family exists at every rarity, and a higher rarity is actually
    /// better. Sized from the real numbers rather than asserted as a boolean — a swapped row or a
    /// typo'd digit that makes SR beat SSR passes any "the def resolves" check.
    /// Every tier stat has an AUTHORED Lv70 value above its Lv60 one, and Lv61-69 walk the straight
    /// line between them (0.0.490).
    #[test]
    fn every_tier_stat_has_an_authored_lv70() {
        for t in REGALIA_LADDER.iter() {
            for (lv1, at) in [
                (t.lump, regalia_lump_at as fn(i32, u8) -> i32),
                (t.adv_big, regalia_advanced_at),
                (t.adv_small, regalia_advanced_at),
                (t.adv_small / 2, regalia_advanced_at),
            ] {
                let v70 = regalia_lv70(lv1).unwrap_or_else(|| panic!("tier value {lv1} has no Lv70 row"));
                let v60 = at(lv1, REGALIA_BASE_CAP);
                assert_eq!(at(lv1, REGALIA_MAX_LEVEL), v70, "{lv1}: Lv70 is the authored value");
                assert!(v70 > v60, "{lv1}: Lv70 {v70} must beat Lv60 {v60}");
                assert_eq!(at(lv1, 65), v60 + (v70 - v60) / 2, "{lv1}: Lv65 is halfway");
            }
        }
    }

    #[test]
    fn every_unit_has_a_regalia_at_every_rarity_and_the_ladder_climbs() {
        for u in ["seraphine", "lapis", "lucielle", "aveline", "raplus", "nemu", "lumi", "airi", "charon", "suzu", "mirei", "astier", "sylvie", "zolfia"] {
            assert!(
                super::regalia_line_for_unit(u).is_some(),
                "{u} has no Regalia line — a unit with nothing to equip is a hole in the banner"
            );
        }

        let mut ids = std::collections::BTreeSet::new();
        let mut names = std::collections::BTreeSet::new();
        for line in super::REGALIA_LINES {
            // Fully invested: base + REGALIA_MAX_LEVEL levels + all five stars.
            let mut prev = [0i64; 4];
            for (rarity, id) in line.ids.iter().enumerate().filter(|(_, id)| !id.is_empty()) {
                assert!(ids.insert(*id), "duplicate Regalia id {id}");
                assert!(names.insert(line.names[rarity]), "duplicate name {}", line.names[rarity]);
                let def = regalia_def(&regalia_item_def_id(id))
                    .unwrap_or_else(|| panic!("{id} does not resolve"));

                // PER AXIS: the lump and the advanced stat, at Lv1 and at Lv60 — a rarity must
                // beat the one under it on both, at both ends.
                let lump = |lv: u8| i64::from(regalia_lump_at(def.lump_base, lv));
                let adv = |lv: u8| def.advanced.map(|m| i64::from(regalia_advanced_at(m.value(), lv))).unwrap_or(0);
                let total = [lump(1), lump(REGALIA_MAX_LEVEL), adv(1), adv(REGALIA_MAX_LEVEL)];
                for (i, (now, before)) in total.iter().zip(prev.iter()).enumerate() {
                    assert!(
                        now > before,
                        "{id}: axis {i} scores {now}, not more than the tier below ({before}) \
                         — a rarity must beat the one under it on EVERY axis, not on average"
                    );
                }
                assert!(lump(REGALIA_MAX_LEVEL) >= lump(1) * 14, "{id}: the lump is ×15 by Lv60");
                assert!(adv(REGALIA_MAX_LEVEL) >= adv(1) * 2, "{id}: the advanced stat is ×2.5 by Lv60");
                assert!(def.star_ranks.iter().all(|r| r.mods.is_empty() && r.skills.is_empty()), "{id}: the passive is empty for now");
                prev = total;
            }
        }
        assert_eq!(ids.len(), 24, "10 3★ + 5 SR + 9 SSR — the six stat-only SR rungs retired in 0.0.489");
    }

    /// Same contract for the hero side, checked the same way — per axis, because a blended score
    /// hid a broken rarity row the first time it was tried on the Regalia table.
    #[test]
    fn every_weapon_family_has_an_exemplar_at_every_rarity_and_the_ladder_climbs() {
        for f in [WeaponType::Sword1H, WeaponType::Dagger, WeaponType::Bow, WeaponType::Staff] {
            assert!(
                super::EXEMPLAR_LINES.iter().any(|l| l.weapon == f),
                "{f:?} has no Exemplar — a weapon family with no hero cannot be sold one"
            );
        }

        let mut ids = std::collections::BTreeSet::new();
        let mut names = std::collections::BTreeSet::new();
        // Per-axis totals by rarity: every SSR must out-score every SR on EVERY axis, checked
        // across the whole table rather than within a family — 0.0.338 made the row a UNIT, so
        // a family is no longer the thing a rarity climbs inside.
        let mut by_rarity: [Vec<[i64; 3]>; 2] = [Vec::new(), Vec::new()];
        for line in super::EXEMPLAR_LINES {
            let id = line.id;
            assert!(ids.insert(id), "duplicate Exemplar id {id}");
            assert!(names.insert(line.name), "duplicate name {}", line.name);
            let def = exemplar_def(&format!("exemplar_{id}"))
                .unwrap_or_else(|| panic!("{id} does not resolve"));

            let axis = |mods: &[StatMod], want: fn(&StatMod) -> Option<i64>| -> i64 {
                mods.iter().filter_map(want).sum()
            };
            let stat = |m: &StatMod| match m {
                StatMod::CritDmgBp(v) => Some(*v as i64),
                _ => None,
            };
            let flat = |m: &StatMod| match m {
                StatMod::AtkFlat(v) => Some(*v as i64),
                _ => None,
            };
            let pct = |m: &StatMod| match m {
                StatMod::AtkBp(v) => Some(*v as i64),
                _ => None,
            };
            // Stars are an Exemplar's ONLY growth axis — it has no Enhancement level.
            let invested = |want: fn(&StatMod) -> Option<i64>| {
                axis(&def.base_mods, want)
                    + def.star_ranks.iter().map(|r| axis(&r.mods, want)).sum::<i64>()
            };
            by_rarity[line.rarity].push([invested(stat), invested(flat), invested(pct)]);

            assert!(
                !def.star_ranks.iter().flat_map(|r| r.mods.iter()).any(|m| pct(m).is_some()),
                "{id}: a percentage axis scales with stars"
            );
            // No HP: that axis is the Regalia's, so the two overlays add up to one character
            // instead of two copies of the same one.
            assert!(
                !def.base_mods
                    .iter()
                    .any(|m| matches!(m, StatMod::MaxHp(_) | StatMod::MaxHpBp(_))),
                "{id}: heroes must not carry the Regalia's HP axis"
            );
        }
        for i in 0..3 {
            let sr = by_rarity[0].iter().map(|t| t[i]).max().expect("SR rows exist");
            let ssr = by_rarity[1].iter().map(|t| t[i]).min().expect("SSR rows exist");
            assert!(ssr > sr, "axis {i}: the weakest SSR ({ssr}) must beat the strongest SR ({sr})");
        }
        assert_eq!(ids.len(), super::EXEMPLAR_LINES.len(), "one def per row, no id reused");
    }
}

#[cfg(test)]
mod weapon_gate_tests {
    use super::*;

    /// A playable unit MUST declare its weapon family. `weapon_types_for_unit` returns an empty
    /// list for anything it does not know, and empty means UNRESTRICTED (test fixtures rely on
    /// that) — so a new unit added to the catalog without a line in the table would silently be
    /// able to hold everything, which is the one failure this gate cannot afford. The table is
    /// code and the roster is catalog, so only a test can tie them together.
    #[test]
    fn every_creatable_unit_declares_its_weapons() {
        let d = crate::data::GameData::from_json_str(include_str!("../../../data/game_data.json"))
            .expect("game_data.json must parse");
        let naked: Vec<&str> = d
            .creatable_units()
            .iter()
            .filter(|u| weapon_types_for_unit(&u.id).is_empty())
            .map(|u| u.id.as_str())
            .collect();
        assert!(
            naked.is_empty(),
            "these units would be able to wield ANY weapon — add them to `weapon_types_for_unit`: {naked:?}",
        );
        // And every unit the table names must exist in the catalog, or the list is stale.
        // Every EXEMPLAR must resolve to a family too — it is the character the gate answers for.
        let naked_ex: Vec<&str> = EXEMPLAR_LINES
            .iter()
            .filter(|l| weapon_types_for_character(Some(l.id), "").is_empty())
            .map(|l| l.id)
            .collect();
        assert!(
            naked_ex.is_empty(),
            "these Exemplars resolve to no weapon family: {naked_ex:?}",
        );
    }

    /// The off-hand follows the weapon family (0.0.446): a shield is Seraphine's, a tome is a
    /// staff-wielder's, a quiver Aveline's, a hidden knife Nyx's — and every catalog off-hand
    /// piece declares its kind, or the gate would wave it through to everyone.
    #[test]
    fn offhand_pieces_follow_the_weapon_family() {
        let d = crate::data::GameData::from_json_str(include_str!("../../../data/game_data.json"))
            .expect("game_data.json must parse");
        let piece = |id: &str| crate::data::instantiate_item(d.item(id).expect(id));
        let ok = |id: &str, unit: &str| piece(id).is_preferred_by(None, unit);
        assert!(ok("shield_s1", "seraphine") && !ok("shield_s1", "lapis") && !ok("shield_s1", "raplus"));
        assert!(ok("tome_s3", "lapis") && ok("tome_s3", "lucielle") && !ok("tome_s3", "seraphine"));
        assert!(ok("quiver_s5", "aveline") && !ok("quiver_s5", "raplus"));
        assert!(ok("hidden_knife_s7", "raplus") && !ok("hidden_knife_s7", "aveline"));
        // The worn Exemplar decides, as for weapons.
        assert!(piece("tome_s1").is_preferred_by(Some("lucielle"), "seraphine"));
        // Every piece in the off-hand slot names its kind.
        let unstamped: Vec<&str> = d
            .items
            .iter()
            .filter(|i| matches!(i.equip_slot, Some(EquipSlot::Shield)) && i.offhand_type.is_none())
            .map(|i| i.id.as_str())
            .collect();
        assert!(unstamped.is_empty(), "off-hand pieces with no offhand_type: {unstamped:?}");
        assert_eq!(units_that_can_carry(OffhandType::Tome), vec!["celestine", "lucielle", "mirei", "nemu", "noelle", "pyrra", "lapis", "astier", "sylvie"].into_iter().collect::<std::collections::BTreeSet<_>>().into_iter().collect::<Vec<_>>());
    }

    /// A unit's STARTING weapon must be one it may wield — otherwise a fresh character spawns
    /// holding something the equip screen would refuse to give back.
    #[test]
    fn a_units_starting_weapon_is_inside_its_own_family() {
        let d = crate::data::GameData::from_json_str(include_str!("../../../data/game_data.json"))
            .expect("game_data.json must parse");
        for u in d.creatable_units() {
            let Some(w) = d.item(&u.starting_weapon) else { continue };
            assert!(
                crate::data::instantiate_item(w).is_preferred_by(None, &u.id),
                "{} starts with {} ({:?}), which its family does not include",
                u.id, w.id, w.weapon_type,
            );
        }
    }
}
