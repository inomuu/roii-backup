//! PATHS OF SOUL — a party-wide meta passive WEB (Path-of-Exile style), spent
//! with the SOUL currency (earned per kill, +boss bonus; previously a dead
//! currency with no sink).
//!
//! Design (locked with the user):
//!   - A 2D WEB of nodes joined by edges. You start from the free `origin` and
//!     unlock OUTWARD: a node can be allocated only if it's the origin or is
//!     adjacent (connected) to a node you already own. Allocate-once (no levels).
//!   - PER-SAVE: lives in `GameState.soul_paths` and is wiped by a new game.
//!   - Bonuses are SHARED by the whole party — every character benefits.
//!
//! Three bonus families radiate from the origin:
//!   - Power   — `AtkPct` / `DefPct` / `CritBp` / `HpPct`, applied as always-on
//!     party AURAS (folded by `combat::derived`).
//!   - Farming — `ExpPct` / `GilPct` / `SoulPct`, scaling kill rewards.
//!   - QoL     — `StashCap` (flat extra stash slots).
//!
//! Hardcoded here for now. When `soul_paths` is empty the whole system is a
//! no-op, so existing saves + goldens are byte-identical.

use crate::skills::BuffKind;
use std::collections::BTreeMap;

/// What a node grants. `*Pct` are percentages; `StashCap` is flat slots; `None`
/// is a connector / start node with no stat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathEffect {
    None,
    AtkPct,
    DefPct,
    /// Flat hard-DEF add (→ `BuffKind::HardDefAdd`). `magnitude` in DEF points.
    /// The DEF spine uses this, not `DefPct`: a % of the small gear-DEF base is
    /// nearly worthless, while flat hard DEF gives a real diminishing %-cut.
    DefAdd,
    HpPct,
    /// Flat max-HP add (→ `BuffKind::MaxHpAdd`). `magnitude` in HP points. Cheap
    /// early "Lifeblood" nodes — a guaranteed HP chunk when %HP is still tiny.
    HpAdd,
    /// Flat ATK add (→ `BuffKind::AtkAdd`). `magnitude` in ATK points. Cheap early
    /// "Edge" nodes — weapon-independent flat damage for the early game.
    AtkAdd,
    ExpPct,
    GilPct,
    SoulPct,
    StashCap,
    /// Keynode QoL: when the gear stash is FULL, a drop that would be lost is
    /// auto-vendored instead, recovering `magnitude`% of its `sell_value` as Gil.
    /// Read directly in `drops::bank_to_inventory`, not stamped as a buff.
    AutoSellOverflow,
    /// +CRIT CHANCE (→ `BuffKind::CritBpAdd`). `magnitude` in basis points. The Magic
    /// arm's stat since 0.0.337: `MatkPct` fed the same bucket as `AtkPct`, so the arm
    /// was the Power arm under another name.
    CritBp,
    /// +DODGE CHANCE (→ `BuffKind::DodgeBpAdd`). `magnitude` in basis points. The
    /// Precision arm's stat since 0.0.337, where HIT and FLEE used to sit. Dodge stacks
    /// inverse-multiplicatively, so deep nodes diminish on their own.
    DodgeBp,
}

/// One node in the web. Allocate-once: owned (level 1) or not.
pub struct PathNode {
    pub id: &'static str,
    pub label: &'static str,
    pub effect: PathEffect,
    /// Flat bonus granted when owned (percent, or slots for `StashCap`).
    pub magnitude: i32,
    /// Soul cost to allocate.
    pub cost: u64,
    /// Web position (grid units; the UI scales + centres). +x right, +y down.
    pub x: i32,
    pub y: i32,
    /// True for the free start node (allocatable with no owned neighbour).
    pub root: bool,
    /// Connected node ids (edges). Define BOTH directions so adjacency +
    /// edge-drawing are symmetric.
    pub neighbors: &'static [&'static str],
}

/// Reserved buff `source_id` for the Power auras — distinct from any entity id,
/// so re-stamping refreshes (never stacks) via `apply_buff`.
pub const PATH_AURA_SOURCE: u32 = 9_000_001;

use PathEffect::*;

const fn n(
    id: &'static str,
    label: &'static str,
    effect: PathEffect,
    magnitude: i32,
    cost: u64,
    x: i32,
    y: i32,
    neighbors: &'static [&'static str],
) -> PathNode {
    PathNode { id, label, effect, magnitude, cost, x, y, root: false, neighbors }
}

/// The web. `origin` (free) sits at the centre; FIVE arms radiate out — Power
/// (up: ATK + DEF), HP (down: max-HP + flat HP), Farming (left: EXP + Gil/Soul),
/// Crit (right: crit chance + Stash/inventory), Precision (up-right: dodge). Each
/// spine node grants +3% (flat-stat / stash nodes are flat). 0.0.337: the percent
/// families are written in BASIS POINTS, which is what the buff readers have taken
/// since the aura channel was built — they were authored as whole percent, so every
/// %-node was delivering a hundredth of its documented value. Cost
/// ramps STEEPLY with depth (500 → 200k) — several cheap 2_000 nodes before each
/// spike, so abundant Soul always chases the next node (the idle treadmill);
/// the tips are multi-day goals. Monsters get re-scaled to match this power.
pub const SOUL_PATHS: &[PathNode] = &[
    PathNode { id: "origin", label: "Awakening", effect: None, magnitude: 0, cost: 0, x: 0, y: 0, root: true, neighbors: &["pw1", "hp1", "fm1", "ql1", "pc1", "nw1", "ne1", "sw1", "se1"] },
    // Spines grant +3% per node; raw-stat / stash / hit / flee nodes are flat.
    // --- Power arm (UP): ATK spine + DEF branch + STR node ---
    n("pw1", "Might", AtkPct, 300, 500, 0, -1, &["origin", "pw2", "pe1"]),
    n("pw2", "Might", AtkPct, 300, 2000, 0, -2, &["pw1", "pw3", "pwd1"]),
    n("pw3", "Might", AtkPct, 300, 2000, 0, -3, &["pw2", "pw4"]),
    n("pw4", "Might", AtkPct, 300, 2000, 0, -4, &["pw3", "pw5", "pwv1"]),
    n("pw5", "Wrath", AtkPct, 300, 8000, 0, -5, &["pw4", "pw6"]),
    n("pw6", "Wrath", AtkPct, 300, 8000, 0, -6, &["pw5", "pw7"]),
    n("pw7", "Fury", AtkPct, 300, 40000, 0, -7, &["pw6", "pw8"]),
    n("pw8", "Bloodlust", AtkPct, 300, 200000, 0, -8, &["pw7"]),
    // DEF spine = FLAT hard DEF (+20/node) — a % of the small gear-DEF base is
    // worthless, flat gives a real diminishing %-cut. ~6 flat (+120 ≈ -17% dmg
    // taken, mirroring the +24% ATK spine) + an Aegis %-cap node for geared
    // tanks. cheap core (pwd1-3) = +60 flat ≈ -10% dmg.
    n("pwd1", "Guard", DefAdd, 20, 2000, 1, -2, &["pw2", "pwd2"]),
    n("pwd2", "Guard", DefAdd, 20, 2000, 1, -3, &["pwd1", "pwd3"]),
    n("pwd3", "Plate", DefAdd, 20, 2000, 1, -4, &["pwd2", "pwd4"]),
    n("pwd4", "Plate", DefAdd, 20, 8000, 1, -5, &["pwd3", "pwd5"]),
    n("pwd5", "Bulwark", DefAdd, 20, 8000, 1, -6, &["pwd4", "pwd6"]),
    n("pwd6", "Bulwark", DefAdd, 20, 40000, 1, -7, &["pwd5", "pwd7"]),
    n("pwd7", "Aegis", DefPct, 800, 200000, 1, -8, &["pwd6"]),
    n("pwv1", "Brawn", AtkAdd, 5, 2000, -1, -4, &["pw4", "pwv2"]),
    n("pwv2", "Titan", AtkAdd, 10, 40000, -1, -5, &["pwv1"]),
    // --- HP arm (DOWN): max-HP spine + MDEF(INT) branch + VIT node ---
    n("hp1", "Vigor", HpPct, 300, 500, 0, 1, &["origin", "hp2"]),
    n("hp2", "Vigor", HpPct, 300, 2000, 0, 2, &["hp1", "hp3", "hm1", "he1"]),
    n("hp3", "Stamina", HpPct, 300, 2000, 0, 3, &["hp2", "hp4"]),
    n("hp4", "Stamina", HpPct, 300, 2000, 0, 4, &["hp3", "hp5", "hpv1"]),
    n("hp5", "Fortitude", HpPct, 300, 8000, 0, 5, &["hp4", "hp6"]),
    n("hp6", "Bastion", HpPct, 300, 40000, 0, 6, &["hp5", "hp7"]),
    n("hp7", "Immortal", HpPct, 300, 100000, 0, 7, &["hp6"]),
    n("hm1", "Resolve", HpAdd, 50, 2000, 1, 2, &["hp2", "hm2"]),
    n("hm2", "Resolve", HpAdd, 50, 2000, 2, 2, &["hm1", "hm3"]),
    n("hm3", "Wisdom", HpAdd, 50, 8000, 3, 2, &["hm2", "hm4"]),
    n("hm4", "Enlightened", HpAdd, 100, 40000, 4, 3, &["hm3"]),
    n("hpv1", "Toughness", DefAdd, 20, 2000, -1, 4, &["hp4"]),
    // --- Farming arm (LEFT): EXP spine + Gil/Soul branch ---
    n("fm1", "Insight", ExpPct, 3, 500, -1, 0, &["origin", "fm2"]),
    n("fm2", "Insight", ExpPct, 3, 2000, -2, 0, &["fm1", "fm3", "fg1", "ke1", "fe1"]),
    n("fm3", "Scholar", ExpPct, 3, 2000, -3, 0, &["fm2", "fm4"]),
    n("fm4", "Scholar", ExpPct, 3, 8000, -4, 0, &["fm3", "fm5"]),
    n("fm5", "Enlightenment", ExpPct, 3, 40000, -5, 0, &["fm4", "fm6"]),
    n("fm6", "Omniscience", ExpPct, 3, 100000, -6, 0, &["fm5"]),
    n("fg1", "Greed", GilPct, 3, 2000, -2, 1, &["fm2", "fg2"]),
    n("fg2", "Fortune", GilPct, 3, 8000, -2, 2, &["fg1", "fg3"]),
    n("fg3", "Avarice", SoulPct, 3, 40000, -2, 3, &["fg2"]),
    // Reclaimer (top-left keynode): when the gear stash is full, auto-vendor the
    // overflow drop for 50% of its value instead of losing it. Branches UP off
    // the farming arm into the empty top-left quadrant.
    n("ke1", "Reclaimer", AutoSellOverflow, 50, 8000, -2, -1, &["fm2"]),
    // --- Magic arm (RIGHT): MATK spine + Stash branch (inventory) ---
    n("ql1", "Focus", CritBp, 150, 500, 1, 0, &["origin", "ql2"]),
    n("ql2", "Focus", CritBp, 150, 2000, 2, 0, &["ql1", "ql3", "qs1", "qe1"]),
    n("ql3", "Arcana", CritBp, 150, 2000, 3, 0, &["ql2", "ql4"]),
    n("ql4", "Arcana", CritBp, 150, 8000, 4, 0, &["ql3", "ql5"]),
    n("ql5", "Ascendance", CritBp, 150, 40000, 5, 0, &["ql4", "ql6"]),
    n("ql6", "Transcendence", CritBp, 150, 100000, 6, 0, &["ql5"]),
    n("qs1", "Satchel", StashCap, 1, 2000, 2, 1, &["ql2", "qs2"]),
    n("qs2", "Pack", StashCap, 1, 2000, 3, 1, &["qs1", "qs3"]),
    n("qs3", "Hoard", StashCap, 1, 2000, 4, 1, &["qs2", "qs4"]),
    n("qs4", "Vault", StashCap, 1, 8000, 4, 2, &["qs3", "qs5"]),
    n("qs5", "Trove", StashCap, 1, 8000, 5, 2, &["qs4", "qs6"]),
    n("qs6", "Treasury", StashCap, 1, 40000, 6, 2, &["qs5", "qs7"]),
    n("qs7", "Warehouse", StashCap, 2, 100000, 7, 2, &["qs6"]),
    // --- Precision arm (UP-RIGHT): dodge chance ---
    n("pc1", "Eagle Eye", DodgeBp, 150, 500, 2, -1, &["origin", "pc2"]),
    n("pc2", "Hawk Eye", DodgeBp, 150, 2000, 3, -2, &["pc1", "pc3", "pcd1", "ce1"]),
    n("pc3", "Evasion", DodgeBp, 150, 2000, 4, -3, &["pc2", "pc4"]),
    n("pc4", "Phantom", DodgeBp, 150, 8000, 5, -4, &["pc3", "pc5"]),
    n("pc5", "Reflex", DodgeBp, 100, 40000, 6, -5, &["pc4", "pc6"]),
    n("pc6", "Windwalker", DodgeBp, 200, 100000, 7, -6, &["pc5"]),
    n("pcd1", "Marksman", DodgeBp, 100, 2000, 4, -2, &["pc2", "pcd2"]),
    n("pcd2", "Sniper", DodgeBp, 200, 40000, 5, -2, &["pcd1"]),

    // ===== EXPANSION: four DIAGONAL arms off the origin =====================
    // Each starts with a cheap ~400 early node and ramps to a 120k late tip, so
    // every route now has both an early and a deep-late pick. Effects reuse the
    // existing palette (no new mechanics) — positions sit in the free diagonals
    // between the five cardinal arms.
    // --- NW (up-left): Onslaught — raw ATK + STR ---
    n("nw1", "Rally", AtkAdd, 5, 400, -1, -1, &["origin", "nw2"]),
    n("nw2", "Onslaught", AtkPct, 300, 2000, -2, -2, &["nw1", "nw3"]),
    n("nw3", "Onslaught", AtkPct, 300, 8000, -3, -3, &["nw2", "nw4"]),
    n("nw4", "Warlord", AtkAdd, 10, 40000, -4, -4, &["nw3", "nw5"]),
    n("nw5", "Conqueror", AtkPct, 300, 120000, -5, -5, &["nw4"]),
    // --- NE (up-right, between Power & Precision): Tempest — MATK + DEX ---
    n("ne1", "Spark", DodgeBp, 100, 400, 1, -1, &["origin", "ne2"]),
    n("ne2", "Tempest", CritBp, 150, 2000, 2, -2, &["ne1", "ne3"]),
    n("ne3", "Tempest", CritBp, 150, 8000, 3, -3, &["ne2", "ne4"]),
    n("ne4", "Sharpshooter", DodgeBp, 200, 40000, 4, -4, &["ne3", "ne5"]),
    n("ne5", "Cataclysm", CritBp, 150, 120000, 5, -5, &["ne4"]),
    // --- SW (down-left): Endurance — HP + hard DEF + VIT ---
    n("sw1", "Hearth", DefAdd, 20, 400, -1, 1, &["origin", "sw2"]),
    n("sw2", "Endurance", HpPct, 300, 2000, -1, 2, &["sw1", "sw3"]),
    n("sw3", "Aegis", DefAdd, 20, 8000, -1, 3, &["sw2", "sw4"]),
    n("sw4", "Endurance", HpPct, 300, 40000, -2, 4, &["sw3", "sw5"]),
    n("sw5", "Colossus", DefAdd, 20, 120000, -3, 5, &["sw4"]),
    // --- SE (down-right): Prosperity — economy (Gil/Exp/Soul/Stash) ---
    n("se1", "Thrift", GilPct, 2, 400, 1, 1, &["origin", "se2"]),
    n("se2", "Prosperity", ExpPct, 3, 2000, 2, 3, &["se1", "se3"]),
    n("se3", "Hoarder", StashCap, 1, 8000, 3, 4, &["se2", "se4"]),
    n("se4", "Windfall", SoulPct, 3, 40000, 4, 5, &["se3", "se5"]),
    n("se5", "Opulence", GilPct, 3, 120000, 5, 6, &["se4"]),

    // ===== EARLY-GAME nodes (all 400 Soul) — a cheap starter branch off each ======
    // cardinal arm so a fresh save (few Souls) always has affordable picks. Includes
    // the flat "Edge" (+5 ATK) and "Lifeblood" (+50 HP) nodes — guaranteed early
    // power while the %-spines are still tiny.
    n("pe1", "Edge", AtkAdd, 5, 400, -1, -2, &["pw1", "pe2"]),
    n("pe2", "Edge", AtkAdd, 5, 400, -1, -3, &["pe1", "pe3"]),
    n("pe3", "Quickness", DodgeBp, 100, 400, -2, -4, &["pe2", "pe4"]),
    n("he1", "Lifeblood", HpAdd, 50, 400, 1, 3, &["hp2", "he2"]),
    n("he2", "Lifeblood", HpAdd, 50, 400, 2, 4, &["he1", "he3"]),
    n("he3", "Hardy", DefAdd, 20, 400, 1, 4, &["he2", "he4"]),
    n("fe1", "Study", ExpPct, 2, 400, -3, -1, &["fm2", "fe2"]),
    n("fe2", "Thrift", GilPct, 2, 400, -4, -1, &["fe1", "fe3"]),
    n("fe3", "Essence", SoulPct, 2, 400, -5, -1, &["fe2", "fe4"]),
    n("qe1", "Cantrip", CritBp, 100, 400, 3, -1, &["ql2", "qe2"]),
    n("qe2", "Musing", CritBp, 100, 400, 4, -1, &["qe1", "qe3"]),
    n("qe3", "Trinket", StashCap, 1, 400, 5, -1, &["qe2", "qe4"]),
    n("ce1", "Keen", DodgeBp, 100, 400, 2, -3, &["pc2", "ce2"]),
    n("ce2", "Nimble", DodgeBp, 100, 400, 2, -4, &["ce1", "ce3"]),
    n("ce3", "Steady", DodgeBp, 100, 400, 3, -4, &["ce2", "ce4"]),

    // A second early precision tier (all 1_000 Soul) — a touch more HIT/FLEE for a
    // fresh save once the 400 cluster is picked, before the 2_000 Precision spine.
    n("ce4", "Focus", DodgeBp, 150, 1000, 3, -5, &["ce3", "ce5"]),
    n("ce5", "Nimbus", DodgeBp, 150, 1000, 2, -5, &["ce4", "ce6"]),
    n("ce6", "Truesight", DodgeBp, 150, 1000, 2, -6, &["ce5", "ce7"]),
    // (2,-7), NOT (1,-6): that cell is `pwd5` on the Plate column, and the two drew exactly on
    // top of each other — one node invisible, one unclickable, and the surviving square wearing
    // both nodes' edges so it read as a node with two prerequisites (user 2026-08-12). Now the
    // ce5 -> ce6 -> ce7 tail runs straight up its own column.
    n("ce7", "Afterimage", DodgeBp, 150, 1000, 2, -7, &["ce6"]),
    // ...and one 1_000-Soul node on each other early arm (ATK / HP / EXP / MATK).
    n("pe4", "Edge", AtkAdd, 6, 1000, -2, -5, &["pe3"]),
    n("he4", "Lifeblood", HpAdd, 75, 1000, 1, 5, &["he3"]),
    n("fe4", "Insight", ExpPct, 3, 1000, -6, -1, &["fe3"]),
    n("qe4", "Runes", CritBp, 150, 1000, 6, -1, &["qe3"]),
];

/// Look up a node by id.
pub fn path_node(id: &str) -> Option<&'static PathNode> {
    SOUL_PATHS.iter().find(|nd| nd.id == id)
}

/// True if `owned` contains the node id (allocate-once → presence = owned).
pub fn is_owned(owned: &BTreeMap<String, u8>, id: &str) -> bool {
    owned.get(id).copied().unwrap_or(0) > 0
}

/// Every node sits on its own cell.
///
/// Position is display-only, which is exactly why this went unnoticed: nothing in the sim reads
/// `x`/`y`, so a collision breaks no rule and no golden — it only makes one node invisible and
/// the other unclickable, because the click test resolves a point to the first node covering it.
/// `pwd5` and `ce7` shared (1, -6) until 2026-08-12.
#[cfg(test)]
#[test]
fn every_node_has_its_own_cell() {
    let mut seen: std::collections::BTreeMap<(i32, i32), &str> = Default::default();
    for n in SOUL_PATHS {
        if let Some(other) = seen.insert((n.x, n.y), n.id) {
            panic!("{} and {} both sit at ({}, {})", other, n.id, n.x, n.y);
        }
    }
}

/// Can `node` be allocated next? It must be unowned and either the root or
/// adjacent to an already-owned node.
pub fn is_allocatable(owned: &BTreeMap<String, u8>, node: &PathNode) -> bool {
    if is_owned(owned, node.id) {
        return false;
    }
    node.root || node.neighbors.iter().any(|nb| is_owned(owned, nb))
}

/// Total bonus for `effect` summed across every OWNED node. 0 when nothing is
/// owned (so an empty tree is a no-op).
pub fn path_bonus(owned: &BTreeMap<String, u8>, effect: PathEffect) -> i32 {
    let mut total = 0i32;
    for nd in SOUL_PATHS {
        if nd.effect == effect && is_owned(owned, nd.id) {
            total = total.saturating_add(nd.magnitude);
        }
    }
    total
}

/// The always-on party-aura `BuffKind` an effect maps to. Returns `None` for the
/// farming (`ExpPct`/`GilPct`/`SoulPct`) + `StashCap` effects, which are read
/// directly (kill rewards / stash size), not stamped as buffs.
pub fn power_buff_kind(effect: PathEffect) -> Option<BuffKind> {
    match effect {
        PathEffect::AtkPct => Some(BuffKind::AtkPct),
        PathEffect::DefPct => Some(BuffKind::DefPct),
        PathEffect::DefAdd => Some(BuffKind::HardDefAdd),
        PathEffect::HpPct => Some(BuffKind::HpPct),
        PathEffect::HpAdd => Some(BuffKind::MaxHpAdd),
        PathEffect::AtkAdd => Some(BuffKind::AtkAdd),
        PathEffect::CritBp => Some(BuffKind::CritBpAdd),
        PathEffect::DodgeBp => Some(BuffKind::DodgeBpAdd),
        _ => Option::None,
    }
}

/// All effects stamped as always-on party auras (everything `power_buff_kind`
/// maps). The upkeep aura pass iterates this.
pub const AURA_EFFECTS: [PathEffect; 8] = [
    PathEffect::AtkPct,
    PathEffect::DefPct,
    PathEffect::DefAdd,
    PathEffect::HpPct,
    PathEffect::HpAdd,
    PathEffect::AtkAdd,
    PathEffect::CritBp,
    PathEffect::DodgeBp,
];

/// Scale a kill reward by a farming `*Pct` bonus: `reward * (100 + bonus) / 100`.
pub fn apply_reward_bonus(reward: u64, owned: &BTreeMap<String, u8>, effect: PathEffect) -> u64 {
    let bonus = path_bonus(owned, effect).max(0) as u64;
    reward.saturating_mul(100 + bonus) / 100
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{apply_input, InputEvent};
    use crate::state::GameState;

    fn alloc(s: &mut GameState, id: &str) {
        apply_input(s, InputEvent::LevelSoulPath { skill_id: id.to_string() });
    }

    #[test]
    fn unlocks_outward_from_origin() {
        let mut s = GameState::new(1, 8, 8);
        s.set_soul_for_test(100_000);
        // A node deep in an arm is NOT allocatable before its path is built.
        apply_input(&mut s, InputEvent::LevelSoulPath { skill_id: "pw3".into() });
        assert!(!is_owned(&s.soul_paths, "pw3"), "can't skip to a far node");
        // Origin is free + always allocatable.
        alloc(&mut s, "origin");
        assert!(is_owned(&s.soul_paths, "origin"));
        // Travel up the Power arm one node at a time.
        alloc(&mut s, "pw1");
        alloc(&mut s, "pw2");
        alloc(&mut s, "pw3");
        assert!(is_owned(&s.soul_paths, "pw3"), "reached via the path");
    }

    /// 0.0.247: the two aura channels must agree about DEAD members. `apply_party_auras`
    /// always stamped corpses; `apply_soul_path_auras` skipped them, so a downed member kept
    /// one aura and lost the other. Both stamp everyone now.
    #[test]
    fn path_auras_stamp_dead_members_like_party_auras() {
        use crate::grid::{Cell, Entity, Team};
        let mut s = GameState::new(1, 8, 8);
        s.set_soul_for_test(100_000);
        alloc(&mut s, "origin");
        alloc(&mut s, "pw1"); // a Power node → an AtkPct/AtkAdd aura exists
        assert!(!s.soul_paths.is_empty());

        // A DOWNED party member.
        let mut member = Entity::idle(1, Cell::new(0, 0)).with_team(Team::Player);
        member.alive = false;
        s.grid.entities.push(member);
        let idx = 0;

        crate::tick::upkeep::apply_soul_path_auras(&mut s);
        let on_corpse = s.grid.entities[idx]
            .buffs
            .iter()
            .filter(|b| b.source_id == PATH_AURA_SOURCE)
            .count();
        assert!(on_corpse > 0, "a dead member still carries the path aura");

        // Still idempotent: re-running must refresh, never stack.
        crate::tick::upkeep::apply_soul_path_auras(&mut s);
        let again = s.grid.entities[idx]
            .buffs
            .iter()
            .filter(|b| b.source_id == PATH_AURA_SOURCE)
            .count();
        assert_eq!(again, on_corpse, "re-stamp refreshes, never stacks");
    }

    #[test]
    fn cost_deducts_and_bonus_sums() {
        let mut s = GameState::new(1, 8, 8);
        s.set_soul_for_test(3_000);
        alloc(&mut s, "origin"); // free
        assert_eq!(s.soul(), 3_000);
        alloc(&mut s, "pw1"); // 500
        alloc(&mut s, "pw2"); // 2000
        assert_eq!(s.soul(), 500);
        // pw1 + pw2 → +3% each, in basis points → 600.
        assert_eq!(path_bonus(&s.soul_paths, PathEffect::AtkPct), 600);
        // Can't afford pw3 (2000) on a 500 balance → no-op.
        alloc(&mut s, "pw3");
        assert!(!is_owned(&s.soul_paths, "pw3"));
    }

    #[test]
    fn empty_is_noop() {
        let s = GameState::new(1, 8, 8);
        assert_eq!(path_bonus(&s.soul_paths, PathEffect::AtkPct), 0);
        assert!(s.soul_paths.is_empty());
    }
}




