//! Per-character Potential Board definitions and pure unlock/path queries.
//!
//! Current prototype spends existing skill points (user, 2026-09-10). Items are
//! deferred. Its caller checks/debits points atomically and supplies tier caps.
//! Existing learned skills are retained as a reset baseline, never erased.

use serde::{Deserialize, Serialize};
use crate::grid::Element;
use std::collections::{BTreeMap, VecDeque};

mod catalog;
pub mod base_kit;
pub mod actions;
pub mod effects;
pub use catalog::*;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Doctrine {
    Sentinel,
    Striker,
    Destroyer,
    Catalyst,
    Support,
}

impl Doctrine {
    pub const ALL: [Self; 5] = [
        Self::Sentinel,
        Self::Striker,
        Self::Destroyer,
        Self::Catalyst,
        Self::Support,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Sentinel => "sentinel",
            Self::Striker => "striker",
            Self::Destroyer => "destroyer",
            Self::Catalyst => "catalyst",
            Self::Support => "support",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.id() == id)
    }

    /// Display form of the id, for a lock message.
    pub fn name(self) -> &'static str {
        match self {
            Self::Sentinel => "Sentinel",
            Self::Striker => "Striker",
            Self::Destroyer => "Destroyer",
            Self::Catalyst => "Catalyst",
            Self::Support => "Support",
        }
    }
}

/// Only explicitly authored fixed-kit classes are supported. An old class or
/// Exemplar overlay must not silently inherit another character's board.
pub fn doctrine_for_unit(unit_def: &str) -> Option<Doctrine> {
    match unit_def {
        "seraphine" => Some(Doctrine::Sentinel),
        "lapis" => Some(Doctrine::Striker),
        "lucielle" => Some(Doctrine::Support),
        "aveline" => Some(Doctrine::Striker),
        "raplus" => Some(Doctrine::Catalyst),
        "nemu" => Some(Doctrine::Striker),
        "lumi" => Some(Doctrine::Sentinel),
        "airi" => Some(Doctrine::Striker),
        "charon" => Some(Doctrine::Destroyer),
        "suzu" => Some(Doctrine::Destroyer),
        "mirei" => Some(Doctrine::Catalyst),
        "astier" => Some(Doctrine::Support),
        "sylvie" => Some(Doctrine::Support),
        "zolfia" => Some(Doctrine::Destroyer),
        _ => None,
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PotentialState {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ranks: BTreeMap<String, u8>,
    /// Only points actually paid to THIS board are refundable.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub spent_points: u32,
    /// Original rank before this board first changes a skill. Zero means the
    /// skill was not learned; reset must preserve unrelated/overlay grants.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub base_skill_levels: BTreeMap<String, u8>,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

impl PotentialState {
    pub fn is_empty(&self) -> bool {
        self.ranks.is_empty() && self.spent_points == 0 && self.base_skill_levels.is_empty()
    }

    pub fn rank(&self, id: &str) -> u8 {
        let Some(def) = node(id) else {
            return 0;
        };
        if matches!(def.kind, NodeKind::Root) {
            return 1;
        }
        self.ranks.get(id).copied().unwrap_or(0).min(def.max_rank())
    }
}

/// SKILL NODES RANK 1 → 20 (docs/potential-upgrade-design.md v2, user 2026-09-12). Rank 1 is the
/// free kit. No level gate — the cost ladder is the only pacing. Soft cap 12 is reachable by
/// farming alone; 13-20 is the long-haul sink.
pub const SKILL_RANK_CAP: u8 = 20;
pub const SOFT_CAP: u8 = 12;

/// Past a skill's own `max_level`, each rank is an OVERRANK: +2% on the skill's numbers via
/// `SkillDef::power_at` — one hook for every skill, and never enough to put the baseline out of
/// scale (rank 20 of a level-10 skill = +20%).
pub const OVERRANK_BP_PER_RANK: u32 = 200;

/// The three material families (catalog ids). Sigil is ELEMENT-LOCKED (user 2026-09-12: "ตาม
/// element ไม่ใช่ doctrine") — a unit spends the Sigil of its own element, index = `ELEMENTS`
/// order — in three grades; Boss Core and Track are the rare capstones. All three are tradeable —
/// a Sigil of another element is a market good. (Essence, a generic second basic family, was cut
/// in 0.0.360 — user: "ตัด Essence ออกก่อน ใช้แค่ Sigil".)
///
/// `ELEMENTS` is the PLAYER CHART (docs/element-weakness-system.md §3): the six elements a unit
/// can be — Thunder took Wind's seat in 0.0.357 (user: "สลับลม เป็นสายฟ้าดีกว่า"). Earth / Poison /
/// Wind / Spirit stay in the `Element` enum as monster attack elements only (user 2026-09-13:
/// "คงต้องเอาออก") — no Sigil exists for them, and a monster whose first weakness is one of them
/// drops nothing from this draw.
pub const SIGIL_GRADES: usize = 3;
pub const CHART: usize = 6;
pub const ELEMENTS: [Element; CHART] = [
    Element::Physical, Element::Fire, Element::Ice, Element::Thunder, Element::Light, Element::Dark,
];
/// Is `e` an element a unit can be — one a Sigil exists for?
pub fn in_chart(e: Element) -> bool {
    ELEMENTS.contains(&e)
}
pub fn element_id(e: Element) -> &'static str {
    match e {
        Element::Physical => "neutral", Element::Fire => "fire", Element::Earth => "earth",
        Element::Ice => "water", Element::Wind => "wind", Element::Light => "holy",
        Element::Dark => "shadow", Element::Poison => "poison", Element::Thunder => "thunder",
        Element::Spirit => "spirit",
    }
}
pub fn element_from_id(id: &str) -> Option<Element> {
    ELEMENTS.into_iter().find(|e| element_id(*e) == id)
}
pub fn element_index(e: Element) -> usize {
    ELEMENTS.iter().position(|x| *x == e).unwrap_or(0)
}
/// The Sigil of `grade` (0-based). ELEMENT-LESS since 2026-09-30 (user: "เอาธาตุออก ให้ sigil ไม่ต้อง
/// มีธาตุ"): every unit spends the same three Sigils.
pub fn sigil_def(grade: usize) -> String {
    format!("sigil_{}", grade + 1)
}
/// Is `id` one of the Sigil defs? By construction, not by the id's spelling.
pub fn is_sigil_def(id: &str) -> bool {
    (0..SIGIL_GRADES).any(|g| sigil_def(g) == id)
}
/// The retired element Sigils (`sigil_<element>_<n>`, one per chart element × grade) and the
/// grade each one folds into. Built from the chart, so a load migration can find them without
/// matching ids by shape.
pub fn legacy_element_sigils() -> Vec<(String, usize)> {
    ELEMENTS
        .iter()
        .flat_map(|e| (0..SIGIL_GRADES).map(move |g| (format!("sigil_{}_{}", element_id(*e), g + 1), g)))
        .collect()
}
pub const BOSS_CORE_DEF: &str = "boss_core";
pub const TRACK_DEF: &str = "track";

/// What one rank costs. `sigil` is `(grade, count)`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Cost {
    pub sigil: (usize, u32),
    pub core: u32,
    pub track: u32,
    pub gil: u64,
}

/// What a rank-1 node of `tier` costs (user 2026-09-13: "stat เต็มไวไป ต้องมี node ยากด้วย"). Tier
/// is AUTHORED on the node (`catalog.rs`), roughly its distance from the origin: the inner ring
/// is grade I, the outer ring grade III with Core and Track, the Majors above that. Index = tier.
pub const NODE_TIER_COST: [Cost; 7] = [
    Cost { sigil: (0, 6), core: 0, track: 0, gil: 10_000 },      // 0: unused (Root / Skill)
    Cost { sigil: (0, 6), core: 0, track: 0, gil: 10_000 },      // T1  inner stat (100k → 10k, 2026-09-30)
    Cost { sigil: (1, 6), core: 0, track: 0, gil: 500_000 },     // T2
    Cost { sigil: (2, 6), core: 1, track: 0, gil: 2_000_000 },    // T3
    Cost { sigil: (2, 12), core: 2, track: 1, gil: 6_000_000 },   // T4  outer stat
    Cost { sigil: (2, 8), core: 2, track: 0, gil: 4_000_000 },    // M1  Major A / B
    Cost { sigil: (2, 16), core: 4, track: 1, gil: 12_000_000 },  // M2  Major C
];

/// The ladder for a SKILL node reaching `next_rank` (2..=20) — the table in the design doc.
/// Genshin's shape: the basic material by grade, Boss Core from rank 7, Track from rank 11, Gil
/// that climbs to seven figures. Stat and Major nodes are rank-1 nodes priced by their tier.
pub fn node_cost(kind: NodeKind, next_rank: u8) -> Cost {
    match kind {
        NodeKind::Skill { .. } => {}
        NodeKind::Stat { tier, .. } | NodeKind::Major { tier, .. } => {
            return NODE_TIER_COST.get(tier as usize).or(NODE_TIER_COST.last()).copied().unwrap_or_default();
        }
        NodeKind::Root => return NODE_TIER_COST.first().copied().unwrap_or_default(),
    }
    // (sigil grade, sigil n, core, track, gil) for ranks 2..=20 — Gil ×100 since the 0.0.412 rescale.
    // EARLY GIL CUT (user 2026-09-30): ranks 2-10 were priced at 6-8 hours of a new player's whole
    // income for the first rank; they now climb from 10k at ≤ ×2.5 a rank and meet the old curve at
    // rank 11 (`skill_rank_gil_never_jumps_more_than_two_and_a_half`). Materials are unchanged.
    const LADDER: [(usize, u32, u32, u32, u64); 19] = [
        (0, 4, 0, 0, 10_000),       // 2
        (0, 6, 0, 0, 25_000),       // 3
        (0, 8, 0, 0, 60_000),       // 4
        (1, 6, 0, 0, 150_000),      // 5
        (1, 8, 0, 0, 300_000),      // 6
        (1, 10, 1, 0, 600_000),     // 7
        (1, 12, 1, 0, 1_200_000),   // 8
        (2, 8, 2, 0, 2_400_000),    // 9
        (2, 10, 2, 0, 5_000_000),   // 10
        (2, 12, 3, 1, 12_000_000),    // 11
        (2, 16, 3, 1, 16_000_000),    // 12  soft cap
        (2, 20, 4, 1, 20_000_000),   // 13
        (2, 24, 5, 1, 26_000_000),   // 14
        (2, 28, 6, 2, 33_000_000),   // 15
        (2, 32, 7, 2, 42_000_000),   // 16
        (2, 36, 8, 2, 52_000_000),   // 17
        (2, 40, 9, 3, 65_000_000),   // 18
        (2, 48, 10, 3, 80_000_000),  // 19
        (2, 60, 12, 4, 100_000_000), // 20 hard cap
    ];
    let i = (next_rank.clamp(2, SKILL_RANK_CAP) - 2) as usize;
    let (sg, sn, core, track, gil) = LADDER.get(i).or(LADDER.last()).copied().unwrap_or_default();
    Cost { sigil: (sg, sn), core, track, gil }
}

/// One row of the Potential LOOT TABLE, keyed by the dying monster's LEVEL (user 2026-09-12:
/// "loot table ตามเลเวลมอนสเตอร์"): the HIGHEST grade a kill in this level range can yield. A zone
/// pays every grade up to its own (0.0.362, user: option ข) — grade I flows at the same rate
/// everywhere, so the stat nodes a capped unit still buys never send it back down the map, while
/// II and III stay tied to the high bands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LootTier {
    /// Inclusive top of the STAGE range (0.0.399; was a level); the last row is open-ended.
    pub max_stage: u16,
    /// Highest grade index this range pays (0 = I, 1 = II, 2 = III).
    pub grade: usize,
}

pub const LOOT_TIERS: [LootTier; 3] = [
    LootTier { max_stage: 60, grade: 0 },
    LootTier { max_stage: 120, grade: 1 },
    LootTier { max_stage: u16::MAX, grade: 2 },
];

/// Per-grade odds of ONE kill, in basis points, field and boss. Tuned 0.0.363 against a 21-day
/// sim-cli run: at 20 / 8 / 3 the world minted 292% of one unit's whole board across all elements
/// in three weeks (user: "300% น่าจะเยอะไป"), and that pooled figure is the real supply — Sigils
/// are sim-rolled (no crowd scale) and every element trades, and the Lv20 element areas will make
/// own-element ≈ pooled without a market. User's fix: "ลด drop rate 50% + เพิ่ม requirement ×2" —
/// half the odds here (grade III rounded up from 1.5 so a grade-III zone still pays it at all)
/// and every Sigil count in the ladder and the node tiers doubled, ~4× slower in all, so pooled ≈
/// 75% at three weeks (target: soft cap on one unit in 3-4 weeks). Boss ≈ 7.5× the field row. In
/// a zone that pays several grades the draw checks the highest first, so the odds are what is
/// written.
pub const GRADE_BP: [u32; SIGIL_GRADES] = [10, 4, 2];
pub const BOSS_GRADE_BP: [u32; SIGIL_GRADES] = [75, 30, 12];

/// Sigil conversion, both ways at one rate (user 2026-09-13: "ควรให้ผสมหรือแตกออกได้ด้วย"):
/// `SIGIL_MERGE` of a grade make one of the next, and one splits back into `SIGIL_MERGE`. Lossless,
/// so the grades are denominations of one material — the drop table, not the exchange, is what
/// makes a higher band worth standing in.
pub const SIGIL_MERGE: u32 = 3;

/// The loot row for a monster of `level` (0 = an unset def → the first row).
pub fn loot_tier(stage: u16) -> &'static LootTier {
    LOOT_TIERS
        .iter()
        .find(|t| stage <= t.max_stage)
        .or(LOOT_TIERS.last())
        .unwrap_or(&LootTier { max_stage: u16::MAX, grade: 0 })
}

/// Everything a purchase can spend, snapshotted from the bag and the purse so the batch preview
/// and the commit run one ledger. Sigil is per grade (element-less since 2026-09-30).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Purse {
    pub sigil: [u32; SIGIL_GRADES],
    pub core: u32,
    pub track: u32,
    pub gil: u64,
}

impl Purse {
    pub fn can_pay(&self, c: &Cost) -> Result<(), String> {
        let roman = ["I", "II", "III"];
        if self.sigil_held(c.sigil.0) < c.sigil.1 {
            let grade = roman.get(c.sigil.0).copied().unwrap_or("?");
            return Err(format!("Needs {} Sigil {}.", c.sigil.1, grade));
        }
        if self.core < c.core {
            return Err(format!("Needs {} Boss Core.", c.core));
        }
        if self.track < c.track {
            return Err(format!("Needs {} Track.", c.track));
        }
        if self.gil < c.gil {
            return Err(format!("Needs {} Gil.", c.gil));
        }
        Ok(())
    }
    /// Sigil held of one grade; an out-of-range grade holds nothing.
    fn sigil_held(&self, grade: usize) -> u32 {
        self.sigil.get(grade).copied().unwrap_or(0)
    }
    pub fn pay(&mut self, c: &Cost) {
        if let Some(held) = self.sigil.get_mut(c.sigil.0) {
            *held -= c.sigil.1;
        }
        self.core -= c.core;
        self.track -= c.track;
        self.gil -= c.gil;
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillSlot {
    Skill1,
    Skill2,
    Ultimate,
    Basic,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MajorSlot {
    A,
    B,
    C,
}

impl MajorSlot {
    pub fn node_id(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
        }
    }
    pub fn tier(self) -> u8 {
        match self {
            Self::A => 2,
            Self::B => 4,
            Self::C => 6,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PotentialStat {
    AtkBp,
    HpBp,
    DefBp,
    CritBp,
    ElementDmgBp,
    ChargeRateBp,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeKind {
    Root,
    Stat {
        stat: PotentialStat,
        magnitude_bp: i32,
        /// Cost tier (`NODE_TIER_COST` index) — authored, so a specific node can be the hard one.
        tier: u8,
    },
    Skill {
        slot: SkillSlot,
    },
    Major {
        slot: MajorSlot,
        tier: u8,
    },
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct NodeDef {
    pub id: &'static str,
    pub kind: NodeKind,
}

impl NodeDef {
    pub fn max_rank(&self) -> u8 {
        if matches!(self.kind, NodeKind::Skill { .. }) {
            SKILL_RANK_CAP
        } else {
            1
        }
    }
    /// The Commander tier a node asks for. Only a Major carries one; a skill rank is paced by its
    /// cost ladder alone (v2), so it asks for nothing here.
    pub fn required_tier(&self, _next_rank: u8) -> u8 {
        match self.kind {
            NodeKind::Major { slot, .. } => slot.tier(),
            _ => 1,
        }
    }
}

/// Only effects with an authored, unambiguous numeric meaning are represented.
/// The integration must consume these effects before enabling their purchase.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "bp", rename_all = "snake_case")]
pub enum MajorEffect {
    AtkFromDefBp(u16),
    PartyDamageReductionBp(u16),
    FireDamageWhileShieldedBp(u16),
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct MajorSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub effect: Option<MajorEffect>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct TierCaps {
    pub commander: u8,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum LockReason {
    UnsupportedCharacter,
    UnknownNode,
    RootAlreadyUnlocked,
    AlreadyMax,
    NotAuthored,
    CommanderTier { required: u8, actual: u8 },
    NeedsNeighbor,
    NeedsTwoMajors,
    NotMajor,
    NoPath,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct Upgrade {
    pub node_id: &'static str,
    pub next_rank: u8,
    pub required_tier: u8,
}

pub fn node(id: &str) -> Option<&'static NodeDef> {
    NODE_DEFS.iter().find(|n| n.id == id)
}

/// Both directions are inferred from a single edge entry, so there cannot be a
/// one-way prerequisite caused by forgetting to duplicate an adjacency list.
pub fn neighbors(id: &str) -> impl Iterator<Item = &'static str> + '_ {
    EDGES.iter().filter_map(move |&(a, b)| {
        if a == id {
            Some(b)
        } else if b == id {
            Some(a)
        } else {
            None
        }
    })
}

pub fn is_owned(state: &PotentialState, id: &str) -> bool {
    state.rank(id) > 0
}

pub fn major_count(state: &PotentialState) -> u8 {
    [MajorSlot::A, MajorSlot::B, MajorSlot::C]
        .into_iter()
        .filter(|slot| is_owned(state, slot.node_id()))
        .count() as u8
}

fn upgrade_requirements(
    unit_def: &str,
    state: &PotentialState,
    id: &str,
    caps: TierCaps,
) -> Result<Upgrade, LockReason> {
    doctrine_for_unit(unit_def).ok_or(LockReason::UnsupportedCharacter)?;
    let def = node(id).ok_or(LockReason::UnknownNode)?;
    if matches!(def.kind, NodeKind::Root) {
        return Err(LockReason::RootAlreadyUnlocked);
    }
    let rank = state.rank(id);
    if rank >= def.max_rank() {
        return Err(LockReason::AlreadyMax);
    }
    let next_rank = rank.saturating_add(1);
    let required_tier = def.required_tier(next_rank);
    if caps.commander < required_tier {
        return Err(LockReason::CommanderTier {
            required: required_tier,
            actual: caps.commander,
        });
    }
    match def.kind {
        NodeKind::Skill { slot } => {
            skill_id(unit_def, slot).ok_or(LockReason::NotAuthored)?;
        }
        NodeKind::Major { slot, .. } => {
            major_spec(unit_def, slot)
                .and_then(|m| m.effect)
                .ok_or(LockReason::NotAuthored)?;
        }
        _ => {}
    }
    Ok(Upgrade {
        node_id: def.id,
        next_rank,
        required_tier,
    })
}

/// Reachability/rank/content gates only; materials and Gil are checked by the
/// atomic purchase handler, using the same authoritative quote the UI displays.
pub fn eligibility(
    unit_def: &str,
    state: &PotentialState,
    id: &str,
    caps: TierCaps,
) -> Result<Upgrade, LockReason> {
    let upgrade = upgrade_requirements(unit_def, state, id, caps)?;
    if id == "k4" {
        if major_count(state) < 2 {
            return Err(LockReason::NeedsTwoMajors);
        }
    } else if !is_owned(state, id) && !neighbors(id).any(|n| is_owned(state, n)) {
        return Err(LockReason::NeedsNeighbor);
    }
    Ok(upgrade)
}

/// Fewest new purchases from any owned node to a selected major. Ties follow
/// the committed node/edge order, never a hash-map order. Tier/content-blocked
/// nodes are excluded. The route does not promise affordability.
pub fn path_toward_major(
    unit_def: &str,
    state: &PotentialState,
    target: &str,
    caps: TierCaps,
) -> Result<Vec<&'static str>, LockReason> {
    doctrine_for_unit(unit_def).ok_or(LockReason::UnsupportedCharacter)?;
    let target_def = node(target).ok_or(LockReason::UnknownNode)?;
    if !matches!(target_def.kind, NodeKind::Major { .. }) {
        return Err(LockReason::NotMajor);
    }
    if is_owned(state, target) {
        return Ok(Vec::new());
    }
    upgrade_requirements(unit_def, state, target, caps)?;
    let mut queue = VecDeque::new();
    let mut parent: BTreeMap<&'static str, Option<&'static str>> = BTreeMap::new();
    for def in &NODE_DEFS {
        if is_owned(state, def.id) {
            parent.insert(def.id, None);
            queue.push_back(def.id);
        }
    }
    while let Some(current) = queue.pop_front() {
        for next in neighbors(current) {
            if parent.contains_key(next) {
                continue;
            }
            if upgrade_requirements(unit_def, state, next, caps).is_err() {
                continue;
            }
            parent.insert(next, Some(current));
            if next == target {
                let mut route = Vec::new();
                let mut cursor = next;
                while !is_owned(state, cursor) {
                    route.push(cursor);
                    cursor = parent
                        .get(cursor)
                        .copied()
                        .flatten()
                        .ok_or(LockReason::NoPath)?;
                }
                route.reverse();
                return Ok(route);
            }
            queue.push_back(next);
        }
    }
    Err(LockReason::NoPath)
}

pub fn next_toward_major(
    unit_def: &str,
    state: &PotentialState,
    target: &str,
    caps: TierCaps,
) -> Result<Option<&'static str>, LockReason> {
    Ok(path_toward_major(unit_def, state, target, caps)?
        .first()
        .copied())
}
