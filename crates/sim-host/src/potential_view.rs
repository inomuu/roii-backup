//! On-demand, renderer-neutral Potential Board projection.
//!
//! Quotes and batch previews share the sim's action rules. This module only
//! formats authored metadata and results; it never spends or predicts currency.
use serde_json::{json, Value};
use sim_core::grid::{Entity, Team};
use sim_core::potential::{self, actions, NodeDef, NodeKind, PotentialStat};
use sim_core::state::GameState;

pub fn for_entity(state: &GameState, entity_id: u32) -> Value {
    let Some(entity) = state.grid.entities.iter().find(|e| e.id == entity_id) else {
        return json!({});
    };
    let doctrine = potential::doctrine_for_unit(&entity.unit_def);
    let supported = entity.team == Team::Player && !entity.is_pet && doctrine.is_some();
    let reason = if entity.team != Team::Player || entity.is_pet {
        "Only party characters can use Potential."
    } else if doctrine.is_none() {
        "This character's fixed kit is not authored for Potential yet. Its existing skills are unchanged."
    } else {
        ""
    };
    let board = actions::effective_board(entity);
    let purse = actions::purse(state);
    let reset_reason = actions::reset_reason(entity);
    let nodes = if supported {
        potential::NODE_DEFS
            .iter()
            .map(|def| node_view(entity, &board, &purse, def))
            .collect()
    } else {
        Vec::<Value>::new()
    };
    json!({
        "entity_id": entity.id,
        "unit_def": entity.unit_def,
        "supported": supported,
        "reason": reason,
        "doctrine": doctrine.map(|d| d.id()).unwrap_or(""),
        "element": entity.element,
        // v2: no level gate. `cap` is the skill-rank ceiling; `soft_cap` where the ladder turns
        // steep. The purse is every family the ladder can ask for (Sigil has no element since
        // 2026-09-30).
        "cap": potential::SKILL_RANK_CAP,
        "soft_cap": potential::SOFT_CAP,
        "level": entity.stats.level,
        "purse": purse_view(&purse),
        "nodes": nodes,
        "can_reset": supported && reset_reason.is_none(),
        "reset_reason": if supported { reset_reason.unwrap_or_default() } else { reason.to_string() },
    })
}

/// `{sigil:[I,II,III], core, track, gil, element}` — the generic Sigil by grade. `element` stays
/// in the shape as "" (Sigils have none since 2026-09-30) so an older client reads a string.
fn purse_view(p: &potential::Purse) -> Value {
    json!({
        "element": "",
        "sigil": p.sigil,
        "core": p.core,
        "track": p.track,
        "gil": p.gil,
    })
}

/// One rank's recipe: `{sigil_grade, sigil, essence_grade, essence, core, track, gil}`.
fn cost_view(c: potential::Cost) -> Value {
    json!({
        "sigil_grade": c.sigil.0,
        "sigil": c.sigil.1,
        "core": c.core,
        "track": c.track,
        "gil": c.gil,
    })
}

fn node_view(entity: &Entity, board: &potential::PotentialState, purse: &potential::Purse, def: &NodeDef) -> Value {
    let rank = board.rank(def.id);
    let quote = actions::quote(entity, purse, def.id);
    let plan = actions::max_plan(entity, purse, def.id);
    // What the next rank costs, whether or not it can be afforded — `(grade, count)`, so the
    // screen can print "3× Potential Shard II" beside a locked node too.
    let cost = actions::cost_of(entity, def.id).ok();
    let max_steps = plan.as_ref().map(|steps| steps.len()).unwrap_or(0);
    let links: Vec<&str> = potential::neighbors(def.id).collect();
    // This is topology only, deliberately distinct from the full quote (which
    // also checks authored content, caps, points and overlay ownership).
    let reachable = if matches!(def.kind, NodeKind::Root) || rank > 0 {
        true
    } else if def.id == "k4" {
        potential::major_count(board) >= 2
    } else {
        potential::neighbors(def.id).any(|id| board.rank(id) > 0)
    };
    let (kind, name, description, current, next, max_rank) = text_for(entity, def, rank);
    let (x, y) = position(def.id);
    json!({
        "id": def.id,
        "name": name,
        "kind": kind,
        "rank": rank,
        "max_rank": max_rank,
        "required_tier": def.required_tier(rank.saturating_add(1).min(max_rank)),
        "reachable": reachable,
        "can_upgrade": quote.is_ok(),
        "reason": quote.err().unwrap_or_default(),
        "cost": cost.map(cost_view),
        "description": description,
        "current_effect": current,
        "next_effect": next,

        "can_max": max_steps > 0,
        "max_reason": plan.as_ref().err().cloned().unwrap_or_default(),
        "max_steps": max_steps,
        "max_node_ids": plan.unwrap_or_default(),
        "links": links,
        "x": x,
        "y": y,
    })
}

// Skill descriptions come from the same authored registry as combat. Rank
// previews intentionally say rank, not an invented damage percentage: some
// skills scale several different effects and only their sim hooks know those.
fn text_for(
    entity: &Entity,
    def: &NodeDef,
    rank: u8,
) -> (&'static str, String, String, String, String, u8) {
    match def.kind {
        NodeKind::Root => (
            "root",
            "Origin".into(),
            "Start of this character's Potential Board.".into(),
            "Unlocked.".into(),
            String::new(),
            1,
        ),
        NodeKind::Stat { stat, magnitude_bp, .. } => {
            let label = stat_name(stat);
            let bonus = format!("+{} {label}", percent(magnitude_bp));
            let current = if rank > 0 {
                bonus.clone()
            } else {
                "No bonus from this node.".into()
            };
            let next = if rank == 0 {
                bonus.clone()
            } else {
                String::new()
            };
            (
                "stat",
                bonus.clone(),
                format!("Grants {bonus} while this node is unlocked."),
                current,
                next,
                1,
            )
        }
        NodeKind::Skill { slot } => {
            let Some(id) = potential::skill_id(&entity.unit_def, slot) else {
                return (
                    "skill", "Basic Attack".into(),
                    "Basic attack rank scaling has not been authored. No points can be spent here yet.".into(),
                    "Existing basic attacks remain available.".into(), String::new(), def.max_rank(),
                );
            };
            let (name, blurb) = sim_core::skills::skill_display(id);
            let detail = sim_core::skills::skill_detail(id);
            let max = sim_core::skills::skill_def(id)
                .map(|skill| skill.max_level.min(def.max_rank()))
                .unwrap_or(def.max_rank());
            (
                "skill",
                if name.is_empty() {
                    id.into()
                } else {
                    name.into()
                },
                if detail.is_empty() {
                    blurb.into()
                } else {
                    detail.into()
                },
                if rank == 0 {
                    "Not learned.".into()
                } else {
                    format!("Skill rank {rank}.")
                },
                if rank < max {
                    format!("Skill rank {}.", rank.saturating_add(1))
                } else {
                    String::new()
                },
                max,
            )
        }
        NodeKind::Major { slot, .. } => {
            if let Some(spec) = potential::major_spec(&entity.unit_def, slot) {
                (
                    "major",
                    spec.name.into(),
                    spec.description.into(),
                    if rank > 0 {
                        spec.description.into()
                    } else {
                        "Not unlocked.".into()
                    },
                    if rank == 0 && spec.effect.is_some() {
                        spec.description.into()
                    } else {
                        String::new()
                    },
                    1,
                )
            } else {
                (
                    "major",
                    format!("Major {}", slot.node_id()),
                    "This major trace has not been authored.".into(),
                    "Not unlocked.".into(),
                    String::new(),
                    1,
                )
            }
        }
    }
}

fn stat_name(stat: PotentialStat) -> &'static str {
    match stat {
        PotentialStat::AtkBp => "ATK",
        PotentialStat::HpBp => "Max HP",
        PotentialStat::DefBp => "DEF",
        PotentialStat::CritBp => "CRIT Rate",
        PotentialStat::ElementDmgBp => "Element DMG",
        PotentialStat::ChargeRateBp => "Charge Rate",
    }
}

fn percent(bp: i32) -> String {
    let magnitude = bp.unsigned_abs();
    let sign = if bp < 0 { "-" } else { "" };
    if magnitude % 100 == 0 {
        format!("{sign}{}%", magnitude / 100)
    } else {
        format!("{sign}{}.{:02}%", magnitude / 100, magnitude % 100)
    }
}

// Presentation coordinates only. The functional list does not depend on them;
// retained graph renderers may use this planar specimen without inventing edges.
fn position(id: &str) -> (u8, u8) {
    match id {
        "root" => (52, 66),
        "s1" => (52, 54),
        "s2" => (52, 43),
        "s3" => (25, 32),
        "s4" => (52, 20),
        "s5" => (42, 78),
        "s6" => (26, 78),
        "s7" => (14, 66),
        "s8" => (78, 75),
        "s9" => (86, 61),
        "s10" => (76, 34),
        "k1" => (52, 32),
        "k2" => (32, 54),
        "k3" => (86, 47),
        "k4" => (32, 92),
        "A" => (52, 8),
        "B" => (14, 53),
        "C" => (66, 87),
        _ => (50, 50),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{apply_input_json, query_json};
    use sim_core::grid::Cell;
    use sim_core::potential::sigil_def;
    use std::collections::BTreeSet;

    const MEMBER: u32 = 11;

    /// A member whose materials ledger holds `sigil` grade-I Sigil plus `gil`
    /// Gil — the two things a rank-one purchase can ask for.
    fn specimen(unit_def: &str, sigil: u32, gil: u64) -> GameState {
        let mut state = GameState::new(42, 12, 12);
        let mut entity = Entity::idle(MEMBER, Cell::new(0, 0)).with_team(Team::Player);
        entity.unit_def = unit_def.into();
        state.grid.entities.push(entity);
        push(&mut state, sigil_def(0), sigil);
        state.set_gil_for_test(gil);
        state
    }

    fn push(state: &mut GameState, id: String, n: u32) {
        state.credit_material(&id, n, sim_core::state::MaterialSource::Drop);
    }

    /// The Sigil by grade, straight from the sim's ledger.
    fn sig(state: &GameState) -> [u32; 3] {
        sim_core::potential::actions::purse(state).sigil
    }

    fn member(state: &GameState) -> &Entity {
        state.grid.entity_of_id(MEMBER).expect("fixture member")
    }

    fn board(state: &GameState) -> Value {
        serde_json::from_str(&query_json(state, None, &format!("potential:{MEMBER}")))
            .expect("the Unity query must return parseable JSON")
    }

    fn row<'a>(view: &'a Value, id: &str) -> &'a Value {
        view.get("nodes")
            .and_then(Value::as_array)
            .expect("the query must expose node rows")
            .iter()
            .find(|node| node.get("id").and_then(Value::as_str) == Some(id))
            .expect("requested node must be visible")
    }

    fn send(state: &mut GameState, event: Value) {
        assert!(
            apply_input_json(state, None, &event.to_string()),
            "Unity event must parse"
        );
    }

    #[test]
    fn dispatched_board_tracks_purchase_and_live_affordability() {
        // Exactly one T1 purchase's worth of Sigil (6 × grade I) lets two adjacent choices
        // compete. Reading the board must not spend it or credit purchased stats for free.
        let mut state = specimen("seraphine", 6, 1_000_000);
        let unchanged = state.clone();
        let before = board(&state);
        assert_eq!(state, unchanged, "opening a board is read-only");
        assert_eq!(before["supported"], true);
        assert_eq!(before["cap"], 20);
        assert_eq!(before["soft_cap"], 12);
        let nodes = before["nodes"].as_array().expect("board nodes");
        let ids: BTreeSet<_> = nodes
            .iter()
            .map(|node| node["id"].as_str().expect("node id"))
            .collect();
        assert_eq!(nodes.len(), 18, "17 investments plus one origin");
        assert_eq!(ids.len(), 18, "each investment must have its own identity");
        assert_eq!(row(&before, "root")["rank"], 1);
        assert_eq!(row(&before, "s1")["can_upgrade"], true);
        assert_eq!(row(&before, "s5")["can_upgrade"], true);
        let cost = &row(&before, "s1")["cost"];
        assert_eq!(cost["sigil_grade"], 0);
        assert_eq!(cost["sigil"], 6);
        assert_eq!(cost["gil"], 10_000);
        assert_eq!(before["purse"]["sigil"][0], 6);
        assert_eq!(before["purse"]["gil"], 1_000_000);
        assert_eq!(before["purse"]["element"], "", "Sigils have no element");

        send(
            &mut state,
            json!({"UpgradePotential": {"entity_id": MEMBER, "node_id": "s1"}}),
        );
        let after = board(&state);
        assert_eq!(after["purse"]["sigil"][0], 0);
        assert_eq!(after["purse"]["gil"], 990_000);
        assert_eq!(sig(&state)[0], 0, "the displayed debit must have happened");
        assert_eq!(state.gil(), 990_000);
        assert_eq!(row(&after, "s1")["rank"], 1);
        assert_eq!(row(&after, "s1")["can_upgrade"], false);
        assert_eq!(row(&after, "s5")["reachable"], true);
        assert_eq!(
            row(&after, "s5")["can_upgrade"],
            false,
            "the other branch is now unaffordable"
        );
        assert_eq!(row(&after, "s5")["reason"], "Needs 6 Sigil I.");
        assert_eq!(after["can_reset"], false, "no reset, ever (0.0.351)");
    }

    #[test]
    fn existing_skill_rank_credit_survives_paid_upgrade_and_reset_is_a_tombstone() {
        // Rank 3 → 4 is ladder row 4: eight grade-I Sigil and 400k Gil — stocked one Sigil short.
        let mut state = specimen("seraphine", 7, 10_000_000);
        let entity = state.grid.entity_of_id_mut(MEMBER).expect("member");
        entity
            .skills
            .push(sim_core::skills::SkillState::new("iron_chains", 3));
        entity
            .skills
            .push(sim_core::skills::SkillState::new("bulwark", 4));
        let before = board(&state);
        assert_eq!(before["supported"], true);
        assert_eq!(
            row(&before, "k1")["rank"],
            3,
            "opening the page must credit the actual existing skill"
        );
        assert_eq!(before["purse"]["sigil"][0], 7);
        let cost = &row(&before, "k1")["cost"];
        assert_eq!(cost["sigil_grade"], 0);
        assert_eq!(cost["sigil"], 8);
        assert_eq!(cost["core"], 0);
        assert_eq!(cost["gil"], 60_000);
        assert_eq!(row(&before, "k1")["can_upgrade"], false);
        assert_eq!(row(&before, "k1")["reason"], "Needs 8 Sigil I.");

        push(&mut state, sigil_def(0), 2);
        send(
            &mut state,
            json!({"UpgradePotential": {"entity_id": MEMBER, "node_id": "k1"}}),
        );
        let paid = board(&state);
        assert_eq!(row(&paid, "k1")["rank"], 4);
        assert_eq!(member(&state).skill_level("iron_chains"), 4);
        assert_eq!(paid["purse"]["sigil"][0], 1, "eight of nine Sigil are spent");
        assert_eq!(paid["purse"]["gil"], 9_940_000);

        // NO RESET: the tombstone parses and the rank stays.
        send(&mut state, json!({"ResetPotential": {"entity_id": MEMBER}}));
        let reset = board(&state);
        assert_eq!(row(&reset, "k1")["rank"], 4);
        assert_eq!(member(&state).skill_level("iron_chains"), 4);
        assert_eq!(member(&state).skill_level("bulwark"), 4);
        assert_eq!(reset["can_reset"], false);
    }

    #[test]
    fn max_preview_cost_matches_applied_route_and_a_reset_is_a_tombstone() {
        // Free rank-one skills are entry points; charge only the remaining route. Every step on
        // it is a rank-one stat/major node priced by its tier (s4 = T2, A / B = M1), so the
        // purse is stocked across grades and Core; the preview's per-node recipes must sum to
        // exactly what the commit debits.
        for (destination, expected_steps) in [("A", 2u64), ("B", 1)] {
            let mut state = specimen("seraphine", 18, 10_000_000);
            sim_core::potential::base_kit::ensure_entity(state.grid.entity_of_id_mut(MEMBER).unwrap());
            push(&mut state, sigil_def(1), 6);
            push(&mut state, sigil_def(2), 16);
            push(&mut state, sim_core::potential::BOSS_CORE_DEF.to_string(), 4);
            let before = board(&state);
            let preview = row(&before, destination);
            assert_eq!(preview["can_max"], true);
            assert_eq!(preview["max_steps"], expected_steps);
            let route: Vec<String> = preview["max_node_ids"]
                .as_array()
                .expect("route")
                .iter()
                .map(|id| id.as_str().expect("node id").to_string())
                .collect();
            assert_eq!(route.len() as u64, expected_steps);
            assert!(
                !route.iter().any(|id| id == "root"),
                "the free origin must not be charged"
            );
            send(
                &mut state,
                json!({"MaxPotential": {"entity_id": MEMBER, "target_id": destination}}),
            );
            let after = board(&state);
            // Sum the route's rank-1 recipes (a not-yet-reachable step shows no `cost` in the
            // view, so the sim's own table is the reference) and compare with the measured purse
            // debit, per family.
            let mut want = [0u64; 3];
            let (mut core, mut gil) = (0u64, 0u64);
            for id in &route {
                let def = sim_core::potential::node(id).expect("route node");
                let c = sim_core::potential::node_cost(def.kind, 1);
                want[c.sigil.0] += u64::from(c.sigil.1);
                core += u64::from(c.core);
                gil += c.gil;
            }
            for g in 0..3 {
                let had = before["purse"]["sigil"][g].as_u64().expect("before");
                let has = after["purse"]["sigil"][g].as_u64().expect("after");
                assert_eq!(had - has, want[g], "grade {g}: the preview must equal the measured debit");
            }
            assert_eq!(before["purse"]["core"].as_u64().unwrap() - after["purse"]["core"].as_u64().unwrap(), core);
            assert_eq!(after["purse"]["gil"].as_u64(), Some(10_000_000 - gil));
            assert!(gil > expected_steps * 100_000, "tiered nodes cost more than the old flat 100k");
            let remaining = after["purse"]["sigil"][2].as_u64().expect("sigil balance");
            assert_eq!(u64::from(member(&state).potential.spent_points), expected_steps);
            for id in &route {
                assert_eq!(row(&before, id)["rank"], 0);
                assert_eq!(
                    row(&after, id)["rank"],
                    1,
                    "every advertised step must have been applied"
                );
            }
            assert_eq!(
                row(&after, destination)["rank"],
                1
            );
            // NO RESET: the tombstone leaves every bought step in place.
            send(&mut state, json!({"ResetPotential": {"entity_id": MEMBER}}));
            let reset = board(&state);
            assert_eq!(reset["purse"]["sigil"][2].as_u64(), Some(remaining));
            for id in &route {
                assert_eq!(row(&reset, id)["rank"], 1);
            }
        }
    }

    #[test]
    fn unsupported_legacy_character_is_explicit_and_queries_never_write() {
        // A unit the board has never heard of (a fixture id) — no kit, no board.
        let mut state = specimen("test_class", 7, 0);
        state
            .grid
            .entity_of_id_mut(MEMBER)
            .expect("member")
            .skills
            .push(sim_core::skills::SkillState::new("bash", 4));
        let unchanged = state.clone();
        let view = board(&state);
        assert_eq!(view["supported"], false);
        assert!(!view["reason"]
            .as_str()
            .expect("unsupported reason")
            .is_empty());
        assert!(view["nodes"].as_array().expect("node rows").is_empty());
        assert_eq!(view["purse"]["sigil"][0], 7);
        assert_eq!(view["can_reset"], false);
        for key in ["potential:999999", "potential:invalid"] {
            let response: Value =
                serde_json::from_str(&query_json(&state, None, key)).expect("JSON");
            assert_eq!(
                response,
                json!({}),
                "an invalid subject must not show another character"
            );
        }
        assert_eq!(state, unchanged);
        send(
            &mut state,
            json!({"UpgradePotential": {"entity_id": MEMBER, "node_id": "s1"}}),
        );
        send(
            &mut state,
            json!({"MaxPotential": {"entity_id": MEMBER, "target_id": "A"}}),
        );
        send(&mut state, json!({"ResetPotential": {"entity_id": MEMBER}}));
        assert_eq!(
            state, unchanged,
            "unsupported inputs must preserve the old character exactly"
        );
    }
}
