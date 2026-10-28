use super::{MajorEffect, MajorSlot, MajorSpec, NodeDef, NodeKind, PotentialStat, SkillSlot};

const fn stat(id: &'static str, kind: PotentialStat, bp: i32, tier: u8) -> NodeDef {
    NodeDef {
        id,
        kind: NodeKind::Stat {
            stat: kind,
            magnitude_bp: bp,
            tier,
        },
    }
}
const fn skill(id: &'static str, slot: SkillSlot) -> NodeDef {
    NodeDef {
        id,
        kind: NodeKind::Skill { slot },
    }
}
const fn major(id: &'static str, slot: MajorSlot, tier: u8) -> NodeDef {
    NodeDef {
        id,
        kind: NodeKind::Major { slot, tier },
    }
}

/// Source specimen from skill-system-redesign §5.3. These generic stat amounts
/// are not a final per-Doctrine rebalance. Stable IDs/order are used by routing.
pub const NODE_DEFS: [NodeDef; 18] = [
    NodeDef {
        id: "root",
        kind: NodeKind::Root,
    },
    // Cost tier (`NODE_TIER_COST`) follows the ring: T1 next to the origin, T4 the far end of the
    // ultimate branch, Majors above the stats (user 2026-09-13: "ต้องมี node ยากด้วย").
    stat("s1", PotentialStat::AtkBp, 400, 1),
    stat("s2", PotentialStat::AtkBp, 300, 1),
    stat("s3", PotentialStat::AtkBp, 300, 2),
    stat("s4", PotentialStat::CritBp, 200, 2),
    stat("s5", PotentialStat::HpBp, 400, 1),
    stat("s6", PotentialStat::DefBp, 400, 1),
    stat("s7", PotentialStat::HpBp, 400, 2),
    stat("s8", PotentialStat::ElementDmgBp, 400, 4),
    stat("s9", PotentialStat::ChargeRateBp, 600, 3),
    stat("s10", PotentialStat::ElementDmgBp, 600, 3),
    skill("k1", SkillSlot::Skill1),
    skill("k2", SkillSlot::Skill2),
    skill("k3", SkillSlot::Ultimate),
    skill("k4", SkillSlot::Basic),
    major("A", MajorSlot::A, 5),
    major("B", MajorSlot::B, 5),
    major("C", MajorSlot::C, 6),
];

/// s4 is interpreted inline between k1 and A in the source ASCII. C has no
/// second entry; k4 has no edge and uses its special two-major gate instead.
pub const EDGES: [(&str, &str); 18] = [
    ("root", "s1"),
    ("root", "s5"),
    ("s1", "s2"),
    ("s2", "k1"),
    ("k1", "s4"),
    ("s4", "A"),
    ("k1", "s3"),
    ("s3", "B"),
    ("B", "k2"),
    ("k2", "s1"),
    ("B", "s7"),
    ("s7", "s6"),
    ("s6", "s5"),
    ("k1", "s10"),
    ("s10", "k3"),
    ("k3", "s9"),
    ("s9", "s8"),
    ("s8", "C"),
];

pub fn skill_id(unit_def: &str, slot: SkillSlot) -> Option<&'static str> {
    match (unit_def, slot) {
        ("seraphine", SkillSlot::Skill1) => Some("iron_chains"),
        ("seraphine", SkillSlot::Skill2) => Some("steel_break"),
        ("seraphine", SkillSlot::Ultimate) => Some("aegis"),
        ("lapis", SkillSlot::Skill1) => Some("fireball"),
        ("lapis", SkillSlot::Skill2) => Some("flame_shield"),
        ("lapis", SkillSlot::Ultimate) => Some("fire_cat_meteor"),
        // Lucielle (0.0.421): the heal and the plague are her two actives, the blessing her Ultimate.
        ("lucielle", SkillSlot::Skill1) => Some("lux_medica"),
        ("lucielle", SkillSlot::Skill2) => Some("lumen_contagio"),
        ("lucielle", SkillSlot::Ultimate) => Some("benedictio_lucis"),
        ("aveline", SkillSlot::Skill1) => Some("nightfall_volley"),
        ("aveline", SkillSlot::Skill2) => Some("shadowbind"),
        ("aveline", SkillSlot::Ultimate) => Some("eclipse_rain"),
        ("raplus", SkillSlot::Skill1) => Some("veil_slash"),
        ("raplus", SkillSlot::Skill2) => Some("rend"),
        ("raplus", SkillSlot::Ultimate) => Some("umbral_requiem"),
        // Basic rank scaling is not authored. Existing Bulwark/Ember Cascade
        // passives are NOT Basic and must remain in the current kit untouched.
        _ => None,
    }
}

/// Real authored text remains inspectable even when a numeric/trigger rule is
/// incomplete. `effect: None` is an explicit non-purchasable content gate.
pub fn major_spec(unit_def: &str, slot: MajorSlot) -> Option<MajorSpec> {
    Some(match (unit_def, slot) {
        ("seraphine", MajorSlot::A) => MajorSpec {
            id: "iron_edge", name: "Iron Edge",
            description: "ATK increases by 25% of current DEF, recalculated every second.",
            effect: Some(MajorEffect::AtkFromDefBp(2500)),
        },
        ("seraphine", MajorSlot::B) => MajorSpec {
            id: "iron_wall", name: "Iron Wall",
            description: "The whole party receives 10% less damage.",
            effect: Some(MajorEffect::PartyDamageReductionBp(1000)),
        },
        ("seraphine", MajorSlot::C) => MajorSpec {
            id: "second_wall", name: "Second Wall",
            description: "Broken Aegis shields refund Seraphine's Limit Break gauge. Refund amount and trigger limit await design.",
            effect: None,
        },
        ("lapis", MajorSlot::A) => MajorSpec {
            id: "cinder_rush", name: "Cinder Rush",
            description: "Kills grant Fireball +20% damage, stacking up to three times. Stack duration/reset rules await design.",
            effect: None,
        },
        ("lapis", MajorSlot::B) => MajorSpec {
            id: "cinderhide", name: "Cinderhide",
            description: "While Flame Shield remains intact, Fire damage increases by 10%.",
            effect: Some(MajorEffect::FireDamageWhileShieldedBp(1000)),
        },
        ("lapis", MajorSlot::C) => MajorSpec {
            id: "kindling", name: "Kindling",
            description: "Enemy Toughness breaks grant +20% Limit Break gauge. Flat versus multiplicative gain awaits clarification.",
            effect: None,
        },
        _ => return None,
    })
}
