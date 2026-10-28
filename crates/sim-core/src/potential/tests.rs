use super::*;
use std::collections::BTreeSet;

const OPEN: TierCaps = TierCaps { commander: 10 };

fn owned(ids: &[&str]) -> PotentialState {
    PotentialState {
        ranks: ids.iter().map(|id| ((*id).to_string(), 1)).collect(),
        ..Default::default()
    }
}

#[test]
fn source_graph_has_seventeen_nodes_and_only_the_authored_connections() {
    assert_eq!(NODE_DEFS.len(), 18, "17 purchasable nodes plus free root");
    assert_eq!(
        NODE_DEFS
            .iter()
            .map(|n| n.id)
            .collect::<BTreeSet<_>>()
            .len(),
        18
    );
    assert_eq!(EDGES.len(), 18);
    let mut links = BTreeSet::new();
    for &(a, b) in &EDGES {
        assert!(node(a).is_some() && node(b).is_some());
        assert_ne!(a, b);
        assert!(
            links.insert(if a < b { (a, b) } else { (b, a) }),
            "no duplicate edge"
        );
        assert!(neighbors(a).any(|n| n == b) && neighbors(b).any(|n| n == a));
    }
    assert_eq!(
        neighbors("s4").collect::<BTreeSet<_>>(),
        BTreeSet::from(["A", "k1"])
    );
    assert_eq!(neighbors("C").collect::<Vec<_>>(), vec!["s8"]);
    assert_eq!(neighbors("k4").count(), 0);
    let mut reached = BTreeSet::from(["root"]);
    let mut queue = VecDeque::from(["root"]);
    while let Some(id) = queue.pop_front() {
        for next in neighbors(id) {
            if reached.insert(next) {
                queue.push_back(next);
            }
        }
    }
    assert_eq!(reached.len(), 17);
    assert!(!reached.contains("k4"));
    let totals = [
        PotentialStat::AtkBp,
        PotentialStat::HpBp,
        PotentialStat::DefBp,
        PotentialStat::CritBp,
        PotentialStat::ElementDmgBp,
        PotentialStat::ChargeRateBp,
    ]
    .map(|wanted| {
        NODE_DEFS
            .iter()
            .filter_map(|n| match n.kind {
                NodeKind::Stat { stat, magnitude_bp, .. } if stat == wanted => Some(magnitude_bp),
                _ => None,
            })
            .sum::<i32>()
    });
    assert_eq!(totals, [1000, 800, 400, 200, 1000, 600]);
}

#[test]
fn neighbour_and_tier_gates_do_not_assume_a_linear_tree() {
    let empty = PotentialState::default();
    assert!(eligibility("seraphine", &empty, "s1", OPEN).is_ok());
    assert!(eligibility("seraphine", &empty, "s5", OPEN).is_ok());
    assert_eq!(
        eligibility("seraphine", &empty, "k1", OPEN),
        Err(LockReason::NeedsNeighbor)
    );
    let shortcut = owned(&["s1"]);
    assert!(
        eligibility("seraphine", &shortcut, "k2", OPEN).is_ok(),
        "side entry from s1 is legal"
    );
    // The Commander tier is the one gate left (Majors only): A is tier 2.
    let a = owned(&["s4"]);
    assert_eq!(
        eligibility("seraphine", &a, "A", TierCaps { commander: 1 }),
        Err(LockReason::CommanderTier { required: 2, actual: 1 })
    );
    assert_eq!(
        eligibility(
            "seraphine",
            &a,
            "A", TierCaps { commander: 1 }
        ),
        Err(LockReason::CommanderTier {
            required: 2,
            actual: 1
        })
    );
    assert_eq!(
        eligibility(
            "seraphine",
            &a,
            "A", TierCaps { commander: 2 }
        ),
        Ok(Upgrade {
            node_id: "A",
            next_rank: 1,
            required_tier: 2
        })
    );
    assert_eq!(
        eligibility("seraphine", &shortcut, "s1", OPEN),
        Err(LockReason::AlreadyMax)
    );
    assert_eq!(
        eligibility("seraphine", &empty, "root", OPEN),
        Err(LockReason::RootAlreadyUnlocked)
    );
    assert_eq!(
        eligibility("seraphine", &empty, "unknown", OPEN),
        Err(LockReason::UnknownNode)
    );
}

/// NO GIL CLIFF on the skill ladder (user 2026-09-30): the early cut climbs at most ×2.5 a rank all
/// the way to the cap, so it rejoins the old prices without a wall.
#[test]
fn skill_rank_gil_never_jumps_more_than_two_and_a_half() {
    let k = NodeKind::Skill { slot: SkillSlot::Skill1 };
    let mut prev = node_cost(NodeKind::Root, 1).gil;
    assert_eq!(prev, 10_000, "the first node costs about an hour or two of a new player's Gil");
    for rank in 2..=SKILL_RANK_CAP {
        let gil = node_cost(k, rank).gil;
        assert!(gil >= prev && gil * 2 <= prev * 5, "rank {rank}: {prev} -> {gil}");
        prev = gil;
    }
}

#[test]
fn tier_boundaries_and_skill_ranks_are_bounded_without_a_debit() {
    // The v2 ladder (docs/potential-upgrade-design.md): no level gate, ranks to 20, Core from 7,
    // Track from 11, Gil to seven figures. Spot-check the shape at its corners.
    let k = NodeKind::Skill { slot: SkillSlot::Skill1 };
    assert_eq!(node_cost(k, 2), Cost { sigil: (0, 4), core: 0, track: 0, gil: 10_000 });
    assert_eq!(node_cost(k, 7).core, 1);
    assert_eq!(node_cost(k, 11).track, 1);
    assert_eq!(node_cost(k, SOFT_CAP), Cost { sigil: (2, 16), core: 3, track: 1, gil: 16_000_000 });
    assert_eq!(node_cost(k, SKILL_RANK_CAP), Cost { sigil: (2, 60), core: 12, track: 4, gil: 100_000_000 });
    // A stat or Major node is a one-rank node priced by its authored tier: the inner ring is
    // grade I, the far end of the ultimate branch grade III with Core and Track.
    assert_eq!(node_cost(NodeKind::Root, 1).sigil, (0, 6));
    let cost_of = |id: &str| node_cost(node(id).expect("node").kind, 1);
    assert_eq!(cost_of("s1"), NODE_TIER_COST[1]);
    assert_eq!(cost_of("s4"), NODE_TIER_COST[2]);
    assert_eq!(cost_of("s10"), NODE_TIER_COST[3]);
    assert_eq!(cost_of("s8"), NODE_TIER_COST[4]);
    assert_eq!(cost_of("A"), NODE_TIER_COST[5]);
    assert_eq!(cost_of("C"), NODE_TIER_COST[6]);
    assert!(cost_of("s8").track > 0 && cost_of("C").core > cost_of("A").core, "the far nodes are the hard ones");
    assert_eq!(
        [MajorSlot::A, MajorSlot::B, MajorSlot::C].map(MajorSlot::tier),
        [2, 4, 6]
    );
    let mut state = owned(&["s1", "s2", "k1"]);
    state.ranks.insert("k1".into(), 2);
    state.spent_points = 4;
    state.base_skill_levels.insert("iron_chains".into(), 1);
    let before = state.clone();
    // A skill rank has NO tier gate (v2): rank 2 → 3 is eligible at any Commander tier.
    assert!(eligibility("seraphine", &state, "k1", TierCaps { commander: 1 }).is_ok());
    assert_eq!(
        eligibility("seraphine", &state, "k1", OPEN).map(|u| u.next_rank),
        Ok(3)
    );
    assert_eq!(
        state, before,
        "query cannot spend points or overwrite reset baseline"
    );
    state.ranks.insert("k1".into(), 255);
    assert_eq!(state.rank("k1"), SKILL_RANK_CAP);
    assert_eq!(
        eligibility("seraphine", &state, "k1", OPEN),
        Err(LockReason::AlreadyMax)
    );
    assert_eq!(state.rank("unknown"), 0);
}

#[test]
fn incomplete_content_is_visible_but_never_available_to_charge() {
    assert_eq!(Doctrine::ALL.len(), 5);
    for doctrine in Doctrine::ALL {
        assert_eq!(Doctrine::from_id(doctrine.id()), Some(doctrine));
    }
    assert_eq!(Doctrine::from_id("unknown"), None);
    assert_eq!(doctrine_for_unit("seraphine"), Some(Doctrine::Sentinel));
    assert_eq!(doctrine_for_unit("lapis"), Some(Doctrine::Striker));
    // Lucielle is a unit since 0.0.421 (Support board); her Ultimate node is the content gate.
    assert_eq!(doctrine_for_unit("lucielle"), Some(Doctrine::Support));
    // Nyx is a unit since 0.0.426 (Catalyst board).
    assert_eq!(doctrine_for_unit("raplus"), Some(Doctrine::Catalyst));
    // Astier is a unit since 0.0.468 (she shares the Support board with Lucielle).
    assert_eq!(doctrine_for_unit("astier"), Some(Doctrine::Support));
    let near_all = owned(&["s4", "k2", "s8", "A", "B"]);
    assert_eq!(
        eligibility("seraphine", &near_all, "C", OPEN),
        Err(LockReason::NotAuthored)
    );
    assert_eq!(
        eligibility("seraphine", &near_all, "k4", OPEN),
        Err(LockReason::NotAuthored)
    );
    assert_eq!(major_count(&near_all), 2);
    let lapis = owned(&["s4", "k2", "s8"]);
    assert_eq!(
        eligibility("lapis", &lapis, "A", OPEN),
        Err(LockReason::NotAuthored)
    );
    assert!(eligibility("lapis", &lapis, "B", OPEN).is_ok());
    assert_eq!(
        eligibility("lapis", &lapis, "C", OPEN),
        Err(LockReason::NotAuthored)
    );
    // Astier is a Support unit since 0.0.468, so the Support board answers for her too; a unit
    // with NO doctrine is what `UnsupportedCharacter` is for.
    assert!(eligibility("astier", &near_all, "s1", OPEN).is_ok());
    assert_eq!(
        eligibility("nobody", &near_all, "s1", OPEN),
        Err(LockReason::UnsupportedCharacter)
    );
    for slot in [MajorSlot::A, MajorSlot::B, MajorSlot::C] {
        assert!(major_spec("seraphine", slot).is_some());
        assert!(major_spec("lapis", slot).is_some());
    }
    assert_eq!(
        major_spec("seraphine", MajorSlot::A).and_then(|m| m.effect),
        Some(MajorEffect::AtkFromDefBp(2500))
    );
    assert_eq!(
        major_spec("seraphine", MajorSlot::B).and_then(|m| m.effect),
        Some(MajorEffect::PartyDamageReductionBp(1000))
    );
    assert_eq!(
        major_spec("lapis", MajorSlot::B).and_then(|m| m.effect),
        Some(MajorEffect::FireDamageWhileShieldedBp(1000))
    );
    assert_eq!(skill_id("seraphine", SkillSlot::Basic), None);
    for class in ["seraphine", "lapis"] {
        for slot in [SkillSlot::Skill1, SkillSlot::Skill2, SkillSlot::Ultimate] {
            let id = skill_id(class, slot).expect("authored skill");
            assert_eq!(
                crate::skills::skill_def(id)
                    .expect("live skill definition")
                    .max_level,
                10
            );
        }
    }
}

#[test]
fn route_is_shortest_from_existing_progress_and_deterministic() {
    let empty = PotentialState::default();
    assert_eq!(
        path_toward_major("seraphine", &empty, "B", OPEN),
        Ok(vec!["s1", "k2", "B"])
    );
    assert_eq!(
        path_toward_major("seraphine", &empty, "A", OPEN),
        Ok(vec!["s1", "s2", "k1", "s4", "A"])
    );
    let lower = owned(&["s5", "s6"]);
    assert_eq!(
        path_toward_major("seraphine", &lower, "B", OPEN),
        Ok(vec!["s7", "B"])
    );
    let tie = owned(&["s1", "s5"]);
    for _ in 0..4 {
        assert_eq!(
            next_toward_major("seraphine", &tie, "B", OPEN),
            Ok(Some("k2"))
        );
    }
    assert_eq!(
        path_toward_major("seraphine", &owned(&["B"]), "B", OPEN),
        Ok(vec![])
    );
    assert_eq!(
        path_toward_major("seraphine", &empty, "B", TierCaps { commander: 3 }),
        Err(LockReason::CommanderTier { required: 4, actual: 3 })
    );
    assert_eq!(
        path_toward_major("seraphine", &empty, "C", OPEN),
        Err(LockReason::NotAuthored)
    );
    assert_eq!(
        path_toward_major("seraphine", &empty, "k1", OPEN),
        Err(LockReason::NotMajor)
    );
}

#[test]
fn repeated_route_steps_reach_the_major_without_buying_an_unrelated_node() {
    let mut state = PotentialState::default();
    let mut purchased = Vec::new();
    for _ in 0..18 {
        let Some(next) = next_toward_major("seraphine", &state, "A", OPEN).expect("known route")
        else {
            break;
        };
        let upgrade = eligibility("seraphine", &state, next, OPEN)
            .expect("first route step must be reachable now");
        state.ranks.insert(next.to_string(), upgrade.next_rank);
        purchased.push(next);
    }
    assert_eq!(purchased, ["s1", "s2", "k1", "s4", "A"]);
    assert!(is_owned(&state, "A"));
    assert!(!is_owned(&state, "B"));
    assert!(!is_owned(&state, "C"));
    assert_eq!(next_toward_major("seraphine", &state, "A", OPEN), Ok(None));
}

#[test]
fn default_and_invested_state_round_trip_without_losing_refund_baseline() {
    let empty: PotentialState = serde_json::from_str("{}").expect("old save default");
    assert!(empty.is_empty());
    assert_eq!(serde_json::to_string(&empty).expect("serialize"), "{}");
    let mut paid = owned(&["s1", "k2"]);
    paid.spent_points = 2;
    paid.base_skill_levels.insert("steel_break".into(), 5);
    assert!(!paid.is_empty());
    let json = serde_json::to_string(&paid).expect("serialize");
    assert_eq!(
        serde_json::from_str::<PotentialState>(&json).expect("deserialize"),
        paid
    );
}
