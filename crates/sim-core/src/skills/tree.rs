//! Unit KITS — which skills a unit carries. Every node is open from the start;
//! the Potential board levels them (`potential`).
//!
//! Separation of concerns:
//!   - `catalog.rs` — what a skill DOES (numbers, kind, effect fields).
//!   - `tree.rs`    — WHO carries it (unit id → kit).
//!
//! Hardcoded per unit (same "tune in code, move to data later" stance as the
//! catalog).


/// One node in a unit's kit.
pub struct SkillNode {
    pub skill_id: &'static str,
    /// Job Level required before this node can be learned. 1 = available
    /// from character creation.
    /// Optional prerequisite — another skill must be at ≥ this level first
    /// (RO-style: Two-Handed Sword Mastery needs Sword Mastery Lv1).
    pub prereq: Option<(&'static str, u8)>,
}

const fn node(skill_id: &'static str) -> SkillNode {
    SkillNode { skill_id, prereq: None }
}

// --- unit kits ------------------------------------------------

/// Seraphine — the whole kit is hers from the start. The new design has no skill
/// tree at all (`docs/skill-system-redesign.md` §5): a character carries a fixed
/// Basic / Skill×2 / Ultimate / Passive and levels it on a board. Until that board
/// exists she is expressed as a tree with every node open at job level 1, which is
/// the closest the current engine gets to "fixed kit".
const SERAPHINE: &[SkillNode] = &[
    node("iron_chains"),
    node("steel_break"),
    node("aegis"),
    node("bulwark"),
];

/// Rapis — fixed kit, every node open from job level 1 (see SERAPHINE above for
/// why a fixed kit is expressed as a fully-open tree for now).
const LAPIS: &[SkillNode] = &[
    node("fireball"),
    node("flame_shield"),
    node("fire_cat_meteor"),
    node("ember_cascade"),
];

/// Lucielle — fixed kit (0.0.421, redesigned with the user): the Holy beam basic, an ATK-scaled
/// heal, the light-plague seeder as her damage, the blessing as her ULTIMATE (fixed numbers),
/// and Communio — every cast also heals the Holy and Sentinel allies.
const LUCIELLE: &[SkillNode] = &[
    node("sagitta_lucis"),
    node("lux_medica"),
    node("lumen_contagio"),
    node("benedictio_lucis"),
    node("communio"),
];

/// Aveline — fixed kit (0.0.423-0.0.424): marking Shadow arrows, the three-target volley, the
/// bleeding bind, Eclipse Rain as her ultimate.
const AVELINE: &[SkillNode] = &[
    node("umbra_fletch"),
    node("nightfall_volley"),
    node("shadowbind"),
    node("eclipse_rain"),
];

/// Nyx — fixed kit (0.0.426): Shadow blades that exploit statuses, the Darkness slash, the rend,
/// and Umbral Requiem across the whole board.
const RAPLUS: &[SkillNode] = &[
    node("umbral_exploit"),
    node("veil_slash"),
    node("rend"),
    node("umbral_requiem"),
];

/// The fixed kit of a unit (empty for unknown ids / monsters).
/// Nemu — fixed kit (0.0.451): the poison basic, the bog, the tether, the black hole.
/// Astier — fixed kit (0.0.468): the tide's bolt, the stream, the blessing, the spring.
const ASTIER: &[SkillNode] = &[
    node("unda_vitae"),
    node("rivus"),
    node("tidebless"),
    node("fons_vitae"),
];

/// Suzu — fixed kit (0.0.465): the plain edge, the sunder, the bell, the fever.
const SUZU: &[SkillNode] = &[
    node("bright_edge"),
    node("sunder"),
    node("bell_strike"),
    node("break_fever"),
];

/// Mirei — fixed kit (0.0.465): the conductor, the field, the quickening, the fall.
const MIREI: &[SkillNode] = &[
    node("conductor"),
    node("storm_field"),
    node("quicken"),
    node("thunderfall"),
];

/// Charon — fixed kit (0.0.463): the toll, the blow, the mark, the passage.
const CHARON: &[SkillNode] = &[
    node("ferrymans_toll"),
    node("tolling_blow"),
    node("blackwater_mark"),
    node("passage_of_no_return"),
];

/// Airi — fixed kit (0.0.462): the static basic, the chain, the overload, the storm.
const AIRI: &[SkillNode] = &[
    node("static_shot"),
    node("chain_bolt"),
    node("overload"),
    node("storm_volley"),
];

/// Lumi — fixed kit (0.0.456): the chill basic, the refuge, the flurry, the whiteout.
const LUMI: &[SkillNode] = &[
    node("chill_touch"),
    node("frost_taunt"),
    node("ice_mend"),
    node("frost_aura"),
];

const NEMU: &[SkillNode] = &[
    node("sweet_poison"),
    node("velvet_bog"),
    node("heartstring"),
    node("abyssal_embrace"),
];

/// Sylvie — fixed kit (0.0.473): the tap, the surge, the focus, the bolt.
const SYLVIE: &[SkillNode] = &[
    node("spark_tap"),
    node("surge"),
    node("arc_focus"),
    node("bolt_from_the_blue"),
];

/// Zolfia — fixed kit (0.0.476): the knight, the split, the winter, the verdict.
const ZOLFIA: &[SkillNode] = &[
    node("coldflame_knight"),
    node("coldflame_split"),
    node("remorseless_winter"),
    node("winters_verdict"),
];

pub fn unit_kit_tree(unit_def: &str) -> &'static [SkillNode] {
    match unit_def {
        "seraphine" => SERAPHINE,
        "lapis" => LAPIS,
        "lucielle" => LUCIELLE,
        "aveline" => AVELINE,
        "raplus" => RAPLUS,
        "nemu" => NEMU,
        "lumi" => LUMI,
        "airi" => AIRI,
        "charon" => CHARON,
        "suzu" => SUZU,
        "mirei" => MIREI,
        "astier" => ASTIER,
        "sylvie" => SYLVIE,
        "zolfia" => ZOLFIA,
        _ => &[],
    }
}

/// The kit node for `skill_id` within `unit_def`, or `None` if that skill
/// is not in the unit's kit.
pub fn tree_node(unit_def: &str, skill_id: &str) -> Option<&'static SkillNode> {
    unit_kit_tree(unit_def).iter().find(|n| n.skill_id == skill_id)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_unit_has_empty_kit() {
        assert!(unit_kit_tree("slime").is_empty());
        assert!(tree_node("slime", "iron_chains").is_none());
    }
}
