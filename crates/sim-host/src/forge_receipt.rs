//! Ephemeral UI receipt: captured around ONE input, never saved or included in sim hashes.
//! Counters distinguish a parsed no-op from a consumed attempt. No extra random draws.
use serde_json::{json, Value};
use sim_core::{data::GameData, events::InputEvent, state::GameState};

pub struct Before {
    kind: &'static str,
    count: u64,
    gil: u64,
    failures: u64,
    consumed: Vec<String>,
    inventory_len: usize,
    materials: Vec<(String, u32)>,
}
impl Before {
    pub fn capture(st: &GameState, data: &GameData, event: &InputEvent) -> Option<Self> {
        let (kind, count) = match event {
            InputEvent::FuseGear { .. } => ("equipment", st.measures.fusions.get()),
            InputEvent::FuseGems { .. } => ("gems", st.measures.gem_fusions.get()),
            _ => return None,
        };
        Some(Self {
            kind,
            count,
            gil: st.gil(),
            inventory_len: st.inventory.len(),
            failures: if kind == "equipment" {
                st.measures.fusions_failed.get()
            } else {
                st.measures.gem_fusions_failed.get()
            },
            consumed: match event {
                InputEvent::FuseGems { gems } => gems.clone(),
                _ => Vec::new(),
            },
            // Gems are bag stacks (0.0.448): the before-picture is every held gem, by def.
            materials: if kind == "gems" {
                sim_core::gems::held(st, data).map(|(id, n)| (id.to_owned(), n)).collect()
            } else {
                Vec::new()
            },
        })
    }
    pub fn finish(self, st: &GameState, data: &GameData) -> Value {
        let count = if self.kind == "equipment" {
            st.measures.fusions.get()
        } else {
            st.measures.gem_fusions.get()
        };
        if count != self.count.saturating_add(1) {
            return json!({"kind":self.kind,"status":"rejected","gil":0});
        }
        let item = if self.kind == "equipment" {
            // Both outcomes append one piece: success upgrades, failure returns the input grade.
            // The sim's failure counter distinguishes them. This runs synchronously before any combat tick / loot.
            if st.inventory.len() + sim_core::fusion::FUSE_INPUTS == self.inventory_len + 1 {
                st.inventory.last().map(crate::item_view::item_to_view)
            } else {
                None
            }
        } else {
            sim_core::gems::held(st, data).find_map(|(id, n)| {
                let before = self
                    .materials
                    .iter()
                    .find(|(key, _)| key == id)
                    .map(|(_, n)| *n)
                    .unwrap_or(0);
                let spent = self
                    .consumed
                    .iter()
                    .filter(|key| key.as_str() == id)
                    .count() as u32;
                if n <= before.saturating_sub(spent) {
                    return None;
                }
                data.item(id).map(|def| {
                    crate::item_view::item_to_view(&sim_core::data::instantiate_item(def))
                })
            })
        };
        let failures = if self.kind == "equipment" {
            st.measures.fusions_failed.get()
        } else {
            st.measures.gem_fusions_failed.get()
        };
        json!({"kind":self.kind,"status":if failures > self.failures {"failed"} else {"success"},
            "gil":st.gil().saturating_sub(self.gil),"item":item,"attempt":count})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use sim_core::{data, items::Grade};
    use crate::session::give_gems;
    const DATA: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/game_data.json"
    ));
    #[test]
    fn receipts_report_consumption_not_parsing_and_survive_ticks() {
        let mut s = Session::new(7, DATA, "zone_b1_1").unwrap();
        s.state.inventory.clear();
        for _ in 0..6 {
            s.state
                .inventory
                .push(data::instantiate_item(s.data.item("sword_s1").unwrap()));
        }
        let before = s.state.gil();
        assert!(s.apply_input_json(r#"{"FuseGear":{"picks":[0,1,2,3,4,5]}}"#));
        let receipt: Value = serde_json::from_str(&s.query_json("forge_receipt")).unwrap();
        assert_eq!(s.state.inventory.len(), 1);
        assert_eq!(receipt["status"], "success");
        assert_eq!(
            receipt["item"],
            crate::item_view::item_to_view(&s.state.inventory[0])
        );
        assert_eq!(receipt["gil"], s.state.gil() - before);
        s.tick(1);
        assert_eq!(
            serde_json::from_str::<Value>(&s.query_json("forge_receipt")).unwrap(),
            receipt
        );
        assert!(s.apply_input_json(r#"{"FuseGear":{"picks":[0,0,0,0,0,0]}}"#));
        assert_eq!(s.state.inventory.len(), 1);
        assert_eq!(
            serde_json::from_str::<Value>(&s.query_json("forge_receipt")).unwrap()["status"],
            "rejected"
        );
        let id = sim_core::gems::def_id("strife", Grade::Common);
        give_gems(&mut s.state, &s.data, &id, 6);
        let gems = vec![id.clone(); 6];
        assert!(s.apply_input_json(&json!({"FuseGems":{"gems":gems}}).to_string()));
        let receipt: Value = serde_json::from_str(&s.query_json("forge_receipt")).unwrap();
        let output = receipt["item"]["def_id"].as_str().unwrap();
        assert_eq!(receipt["status"], "success");
        assert_eq!(receipt["gil"], 0);
        assert_eq!(sim_core::gems::held_count(&s.state, &id), 0);
        assert_eq!(sim_core::gems::held_count(&s.state, output), 1);
        assert!(!s.apply_input_json("invalid"));
        assert_eq!(s.query_json("forge_receipt"), "{}");
    }
    #[test]
    fn failed_receipts_include_refunds_and_full_gil_without_claiming_success() {
        let mut s = Session::new(7, DATA, "zone_b1_1").unwrap();
        std::sync::Arc::make_mut(&mut s.data).loot_rules.fusion.fail_bp_by_grade = vec![10_000];
        s.state.inventory.clear();
        for _ in 0..6 {
            s.state
                .inventory
                .push(data::instantiate_item(s.data.item("sword_s1").unwrap()));
        }
        let q: Value = serde_json::from_str(&s.query_json("fuse_preview:0,1,2,3,4,5")).unwrap();
        let gil = s.state.gil();
        assert_eq!(q["fail_bp"], 10_000);
        s.apply_input_json(r#"{"FuseGear":{"picks":[0,1,2,3,4,5]}}"#);
        let receipt: Value = serde_json::from_str(&s.query_json("forge_receipt")).unwrap();
        assert_eq!(s.state.inventory.len(), 1);
        assert_eq!(receipt["status"], "failed");
        assert_eq!(
            receipt["item"],
            crate::item_view::item_to_view(&s.state.inventory[0])
        );
        assert_eq!(receipt["item"]["grade_step"], 0);
        assert_eq!(receipt["gil"], s.state.gil() - gil);
        assert_eq!(receipt["gil"], q["payout_gil"]);
        // A refunded gem can be the SAME input ID: its net quantity falls from 6 to 1.
        // Looking only for a positive ledger delta would incorrectly report no reward.
        std::sync::Arc::make_mut(&mut s.data).loot_rules.gems.sets.retain(|set| set.id == "strife");
        let id = sim_core::gems::def_id("strife", Grade::Common);
        give_gems(&mut s.state, &s.data, &id, 6);
        s.apply_input_json(&json!({"FuseGems":{"gems":vec![id.clone();6]}}).to_string());
        let receipt: Value = serde_json::from_str(&s.query_json("forge_receipt")).unwrap();
        assert_eq!(receipt["status"], "failed");
        assert_eq!(receipt["item"]["def_id"], id);
        assert_eq!(receipt["item"]["grade_step"], 0);
        assert_eq!(receipt["gil"], 0);
        assert_eq!(sim_core::gems::held_count(&s.state, &id), 1);
    }
}
