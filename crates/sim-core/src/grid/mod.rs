//! Grid world + entity definition (GDD §3.2).
//!
//! Symmetric entity rule (§4.2.2): one struct for both party members
//! and monsters. The only difference between them is field VALUES
//! (`team`, stats), never struct SHAPE.
//!
//! ALPHA-3 expansion: every Entity now has combat stats. ALPHA-2 used
//! a smaller struct; behavior is preserved (an Entity with `team =
//! Player` and no living enemies in the arena still walks toward its
//! `target` field exactly as before). The state_hash differs from
//! ALPHA-2 because the JSON shape grew — this is a tracked semantic
//! version bump, not a determinism break.

use serde::{Deserialize, Serialize};

mod board;
mod entity;

pub use board::*;
pub use entity::*;

#[cfg(test)]
mod tests;

/// Integer cell coordinate on the grid.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Cell {
    pub x: i32,
    pub y: i32,
}

impl Cell {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Chebyshev ("king moves") distance.
    pub fn chebyshev(self, other: Cell) -> i32 {
        let dx = (self.x - other.x).abs();
        let dy = (self.y - other.y).abs();
        if dx > dy { dx } else { dy }
    }
}

/// A blast footprint, in HALF-CELL coordinates.
///
/// WHY NOT JUST A RADIUS. Every AoE in the sim used to be `pos.chebyshev(center) <= radius` — a
/// box of side `2r+1` centred on a CELL. Every shape that can produce is ODD-sided, because a
/// 2×2 has no centre cell to be centred on. Commander smite is authored as a 2×2, so the shape
/// language had to grow one before the skill could exist at all.
///
/// The trick is to double everything. A cell `(x, y)` sits at `(2x, 2y)`, which makes the GAP
/// between two cells an addressable point — and an even-sided box is exactly a box centred on a
/// gap. One comparison then covers both parities with no special case in the test itself:
///
/// ```text
/// |2·p.x − c2.x| ≤ e2   and   |2·p.y − c2.y| ≤ e2
/// ```
///
/// Integer-only, no floats, and every pre-existing radius maps onto it EXACTLY rather than
/// approximately — see `radius()`, which is why introducing this moved no golden.
///
/// ANCHORING (user, 2026-08-21). An odd box centres on the cursor cell and expands the way every
/// other AoE in the game always has; an even box has no centre cell, so it takes the up-left one
/// of the middle pair. Both fall out of a constant `+1` on the doubled centre, independent of
/// size — `2×2` is `c … c+1` (cursor top-left) and `4×4` is `c−1 … c+2` (cursor centre, biased
/// up-left). A rule that collapses instead of accumulating per-size cases is the one that is
/// right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blast {
    /// Doubled centre — `2·cell`, or `2·cell + 1` when the footprint is even-sided.
    c2: Cell,
    /// Doubled half-extent, i.e. `side − 1`.
    e2: i32,
}

impl Blast {
    /// A footprint of side `n` anchored at `center`. `n <= 0` is an empty blast that contains
    /// nothing, which is what a caller passing an unset radius should get.
    pub const fn sided(center: Cell, n: i32) -> Self {
        if n <= 0 {
            return Self { c2: Cell::new(0, 0), e2: -1 };
        }
        // The parity branch IS the design decision, and it is the only branch there is.
        let off = if n % 2 == 0 { 1 } else { 0 };
        Self { c2: Cell::new(center.x * 2 + off, center.y * 2 + off), e2: n - 1 }
    }

    /// The LEGACY spelling: `chebyshev(center) <= r`, i.e. an odd box of side `2r+1`.
    ///
    /// Provably identical, not merely similar — `|p−c| ≤ r` and `|2p−2c| ≤ 2r` are the same
    /// inequality — which is what let every existing call site move over without re-pinning a
    /// hash. Keep using this for anything authored as a radius; `sided` is for shapes.
    pub const fn radius(center: Cell, r: i32) -> Self {
        Self { c2: Cell::new(center.x * 2, center.y * 2), e2: r * 2 }
    }

    /// Is `p` inside the footprint?
    pub fn contains(self, p: Cell) -> bool {
        if self.e2 < 0 {
            return false;
        }
        (p.x * 2 - self.c2.x).abs() <= self.e2 && (p.y * 2 - self.c2.y).abs() <= self.e2
    }
}

/// Whose side is this entity on? Players hit Monsters and vice versa,
/// never their own side. Phase 0 has exactly two teams.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum Team {
    Player,
    #[default]
    Monster,
}

/// Attack element (GDD §3.2 / Pillar 4 acquisition variety).
///
/// Fire > Earth > Water > Fire is the rock-paper-scissors triangle.
/// `Neutral` sits OUTSIDE the triangle: it neither beats nor loses to
/// anything — every matchup involving Neutral is 100% (see
/// `combat::element_mult_bp`). It's the default for a plain physical
/// swing (a sword with no elemental endow), mirroring RO where weapons
/// are Neutral until enchanted.
///
/// RO-inspired element set, simplified to a LOOSE (RO-/Pokemon-referenceable)
/// matchup: advantage 150%, disadvantage 50%, else 100% — no per-element levels.
/// The web lives in `combat::element_mult_bp` (its `ADVANTAGE` table), built to
/// be GUESSABLE: the RO nature cycle Fire>Earth>Wind>Water>Fire, the universally
/// known Thunder>Water / Earth>Thunder (Thunder is distinct from Wind), Fire/Wind
/// clearing Poison, and a Holy<->Shadow light/dark axis (mutual 150%). Neutral
/// has no matchups. (Undead/Ghost can be added later.) Variant order is
/// APPEND-ONLY so `skills::element_to_mag` encodings don't shift.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum Element {
    /// Physical — "Neutral" until 0.0.461 (user: "ไม่มีธาตุ Neutral เรียกว่า Physical เป็นธาตุจริง");
    /// the alias reads every save and catalog written before.
    #[default]
    #[serde(alias = "Neutral")]
    Physical,
    Fire,
    Earth,
    /// Ice — "Water" until 0.0.475 (user: "เรียกว่า Ice ดีกว่า"); the alias reads every save and
    /// catalog written before.
    #[serde(alias = "Water")]
    Ice,
    Wind,
    /// Light — "Holy" until 0.0.458 (user: "ให้ใช้คำว่า Dark / Light ให้หมด"); the alias reads
    /// every save and catalog written before.
    #[serde(alias = "Holy")]
    Light,
    /// Dark — "Shadow" until 0.0.458; same alias contract.
    #[serde(alias = "Shadow")]
    Dark,
    Poison,
    Thunder,
    /// Ghost-kind (0.0.210, gothic pack): Holy shreds it (150%), and ITS attacks
    /// shred Neutral (150% — the reverse being "plain weapons pass through
    /// spirits at 50%", the RO Ghost-armor feel). Not in the gear-roll element
    /// pool — player anti-ghost tech stays Holy.
    Spirit,
}

/// RO monster RACE — drives the "damage vs race" gear-mod axis (Hydra-card
/// lineage). Players are always `DemiHuman`. `Unknown` = legacy/unset data;
/// matches nothing. Variant order APPEND-ONLY (serialized).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum Race {
    #[default]
    Unknown,
    Formless,
    Undead,
    Brute,
    Plant,
    Insect,
    Fish,
    Demon,
    DemiHuman,
    Angel,
    Dragon,
}

/// RO monster SIZE — the "damage vs size" gear-mod axis. Players are `Medium`.
/// Variant order APPEND-ONLY (serialized).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum Size {
    Small,
    #[default]
    Medium,
    Large,
}

/// How a strike resolves against a target (BETA-5 Phase 3).
///
///   `Physical` — `amount` is ATK, mitigated by the target's DEF, can
///                crit (LUK ignores DEF) and miss (flee).
///   `Magic`    — `amount` is MATK, mitigated by MDEF, IGNORES physical
///                DEF, never crits and never misses (pre-renewal RO).
///
/// Damage type is a property of the ATTACK, not the attacker — a mage's
/// staff bonk is Physical, its fireball is Magic. (Phase 3 ships one
/// attack per entity; the multi-attack roster + AI is a later phase.)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum DamageType {
    #[default]
    Physical,
    Magic,
    /// PURE / true damage — ignores DEF, element triangle, and ward entirely
    /// (`apply_damage` returns the raw amount). Used by the player click-attack;
    /// no entity ever carries this as its `atk_type`.
    True,
}

/// Fixed neighbor expansion order used by BFS. Exported so tests can
/// reason about it; changing this changes every save's path & hash.
///
/// Order: STRAIGHT moves first, diagonals last. Cells reachable via
/// straight steps enter the BFS queue ahead of cells reachable only
/// via diagonals, so when multiple shortest paths exist the straight
/// one wins. Without this, the visible "preferred" path biased toward
/// diagonal zig-zags even when a straight line was equally short.
pub const BFS_NEIGHBOR_ORDER: [(i32, i32); 8] = [
    (0, -1),                          // N
    (-1, 0),           (1, 0),        // W, E
    (0, 1),                           // S
    (-1, -1), (1, -1),                // NW, NE
    (-1, 1),  (1, 1),                 // SW, SE
];


#[cfg(test)]
mod blast_tests {
    use super::*;

    /// Every cell of the footprint, scanned over a window wide enough to catch overspill.
    fn cells(b: Blast, around: Cell) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        for y in (around.y - 4)..=(around.y + 4) {
            for x in (around.x - 4)..=(around.x + 4) {
                if b.contains(Cell::new(x, y)) {
                    out.push((x, y));
                }
            }
        }
        out
    }

    /// THE MIGRATION'S WHOLE SAFETY ARGUMENT: the doubled form is not an approximation of
    /// `chebyshev(c) <= r`, it is the same inequality. Checked exhaustively over a window rather
    /// than argued, because every AoE call site in the sim was rewritten on the strength of it.
    #[test]
    fn radius_is_exactly_the_chebyshev_test_it_replaced() {
        for r in 0..4 {
            for cy in -2..3 {
                for cx in -2..3 {
                    let c = Cell::new(cx, cy);
                    let b = Blast::radius(c, r);
                    for y in -8..9 {
                        for x in -8..9 {
                            let p = Cell::new(x, y);
                            assert_eq!(b.contains(p), p.chebyshev(c) <= r,
                                "r={r} c=({cx},{cy}) p=({x},{y})");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn an_odd_box_centres_on_the_cursor() {
        let c = Cell::new(5, 5);
        assert_eq!(cells(Blast::sided(c, 1), c), vec![(5, 5)]);
        // 3×3 spans c-1 ..= c+1 on both axes.
        let three = cells(Blast::sided(c, 3), c);
        assert_eq!(three.len(), 9);
        assert!(three.contains(&(4, 4)) && three.contains(&(6, 6)) && three.contains(&(5, 5)));
        assert!(!three.contains(&(7, 5)));
        // and it agrees with the radius spelling
        assert_eq!(cells(Blast::sided(c, 3), c), cells(Blast::radius(c, 1), c));
        assert_eq!(cells(Blast::sided(c, 5), c), cells(Blast::radius(c, 2), c));
    }

    /// 2×2 puts the cursor at the TOP-LEFT (user, 2026-08-21).
    #[test]
    fn a_two_by_two_hangs_down_and_right_of_the_cursor() {
        let c = Cell::new(5, 5);
        assert_eq!(cells(Blast::sided(c, 2), c), vec![(5, 5), (6, 5), (5, 6), (6, 6)]);
    }

    /// 4×4 CENTRES on the cursor, biased up-left — c-1 ..= c+2, NOT c ..= c+3. The corner-anchored
    /// reading gives the right answer for 2×2 and the wrong one here, which is why this is pinned.
    #[test]
    fn a_four_by_four_centres_biased_up_left() {
        let c = Cell::new(5, 5);
        let f = cells(Blast::sided(c, 4), c);
        assert_eq!(f.len(), 16);
        assert!(f.contains(&(4, 4)), "must reach one cell up-left of the cursor");
        assert!(f.contains(&(7, 7)), "and two down-right");
        assert!(!f.contains(&(3, 5)) && !f.contains(&(8, 5)));
        // 6×6 keeps the same bias: c-2 ..= c+3.
        let six = cells(Blast::sided(c, 6), Cell::new(6, 6));
        assert!(six.contains(&(3, 3)) && six.contains(&(8, 8)));
    }

    /// A caller that passes an unset size must hit nothing rather than hitting one cell — the
    /// `aoe_radius == 0` sites mean "single target, no splash", and a stray centre hit there
    /// would be a silent extra victim.
    #[test]
    fn a_zero_or_negative_side_contains_nothing() {
        let c = Cell::new(3, 3);
        assert!(cells(Blast::sided(c, 0), c).is_empty());
        assert!(cells(Blast::sided(c, -2), c).is_empty());
        // `radius(c, 0)` is the OTHER convention — one cell — and must stay that way.
        assert_eq!(cells(Blast::radius(c, 0), c), vec![(3, 3)]);
    }
}
