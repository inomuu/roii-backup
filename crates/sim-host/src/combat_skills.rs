//! Read-only combat HUD: Godot's loadout_json plus learned passive skills.
use sim_core::{state::GameState, skills::{self, SkillKind}};

pub fn for_entity(state: &GameState, id: u32) -> serde_json::Value {
    let Some(i) = state.grid.idx_of_id(id) else { return serde_json::json!([]) };
    let e = &state.grid.entities[i];
    let ids = e.active_skills.iter().chain(e.skills.iter().filter(|sk| {
        sk.level > 0 && skills::skill_def(&sk.id).is_some_and(|d| d.kind == SkillKind::Passive)
    }).map(|sk| &sk.id));
    let rows: Vec<_> = ids.filter_map(|id| {
        let sk = e.skills.iter().find(|sk| &sk.id == id && sk.level > 0)?;
        let def = skills::skill_def(id)?;
        let (name, blurb) = skills::skill_display(id);
        Some(serde_json::json!({
            "id": id, "name": name, "blurb": blurb,
            "detail": skills::skill_detail(id), "level": sk.level,
            "passive": def.kind == SkillKind::Passive,
            "cd": sk.cooldown, "cd_max": def.cooldown_ticks,
        }))
    }).collect();
    serde_json::json!(rows)
}

#[cfg(test)]
mod tests {
    use crate::session::Session;
    #[test]
    fn hud_tracks_cooldowns_passives_and_member_isolation() {
        let mut session = Session::new_as(42, include_str!("../../../data/game_data.json"),
            "zone_b1_1", "seraphine", "HUD").unwrap();
        let id = session.state.grid.entities.iter().find(|e| e.unit_def == "seraphine").unwrap().id;
        let before = session.query_json(&format!("combat_skills:{id}"));
        let rows: serde_json::Value = serde_json::from_str(&before).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 4);
        assert!(rows.as_array().unwrap().iter().any(|r| r["id"] == "bulwark" && r["passive"] == true));
        assert!(!rows.as_array().unwrap().iter().any(|r| r["id"] == "fireball"));
        let i = session.state.grid.idx_of_id(id).unwrap();
        let sk = session.state.grid.entities[i].skills.iter_mut().find(|s| s.id == "iron_chains").unwrap();
        sk.cooldown = 17;
        let rows = super::for_entity(&session.state, id);
        let chain = rows.as_array().unwrap().iter().find(|r| r["id"] == "iron_chains").unwrap();
        assert_eq!(chain["cd"], 17);
        assert!(chain["cd_max"].as_u64().unwrap() > 0);
        assert!(!chain["blurb"].as_str().unwrap().is_empty());
        assert_eq!(session.query_json("combat_skills:999999"), "[]");
        assert_eq!(session.query_json("combat_skills:bad"), "[]");
        session.state.grid.entities[i].active_skills.retain(|s| s != "iron_chains");
        let rows = super::for_entity(&session.state, id);
        assert!(!rows.as_array().unwrap().iter().any(|r| r["id"] == "iron_chains"));
        assert!(rows.as_array().unwrap().iter().any(|r| r["id"] == "bulwark"));
    }
}
