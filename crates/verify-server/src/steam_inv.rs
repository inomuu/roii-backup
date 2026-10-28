//! STEAM INVENTORY MINTING — an in-game item becomes a Steam item (`docs/steam-mint-quota-design.md`,
//! D1 cancelled 2026-09-24: every account may mint).
//!
//! The flow never lets an item exist twice:
//! 1. The player sends it: `InputEvent::SendToSteam` takes it out of the bag and counts it in
//!    `GameState.steam_sent[def_id]` — only for a catalog def with a `steam_itemdef`.
//! 2. A VERIFIED save carries the count. `accrue_after_save` queues every count past this
//!    account's watermark as a `steam_mints` row (`plan`), one row per item, keyed
//!    (account, def, seq) — a save seen twice queues nothing new. Provenance is the replay: an
//!    item in a verified bag got there by a path the server checked (a mailed box, a claim).
//! 3. A worker mints pending rows through `IInventoryService/AddItem`. It runs only when
//!    `STEAM_INVENTORY_LIVE=1` and a publisher key are set; otherwise rows wait, nothing is lost.
//!
//! AddItem is not idempotent on Steam's side, so the worker never retries blind: a row is marked
//! `inflight` before the call; Steam's own "no" puts it back to `pending`, a success records the
//! item id as `done`, and anything that leaves the outcome unknown (timeout, dropped connection,
//! unparseable body) parks it as `uncertain` for a human to reconcile against the player's Steam
//! inventory rather than risk a second grant.

use std::collections::BTreeMap;

/// Most rows one save may queue — a guard on a runaway count, not a pacing lever.
pub const QUEUE_PER_SAVE: u64 = 50;

/// The rows to queue: for each def, sequence numbers `minted+1 ..= sent` (capped per save).
/// `queued` is the highest seq already queued per def. Pure.
pub fn plan(sent: &BTreeMap<String, u64>, queued: &BTreeMap<String, u64>) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    for (def, &n) in sent {
        let have = queued.get(def).copied().unwrap_or(0);
        let upto = n.min(have + QUEUE_PER_SAVE);
        for seq in (have + 1)..=upto {
            out.push((def.clone(), seq));
        }
    }
    out
}

/// What one AddItem call came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MintResult {
    /// Granted: Steam's item id.
    Granted(String),
    /// Steam answered and said no — safe to try again later.
    Refused(String),
    /// Nothing reached Steam (no connection) — retried without counting toward the refusal cap.
    NotSent(String),
    /// No answer we can trust — the item may or may not exist. Never retried automatically.
    Unknown(String),
}

/// Read an AddItem response body. Steam returns the granted items as a JSON STRING inside
/// `response.item_json`; the first item's `itemid` is the grant.
pub fn parse_add_item(body: &serde_json::Value) -> MintResult {
    let Some(raw) = body.pointer("/response/item_json").and_then(|v| v.as_str()) else {
        return MintResult::Unknown(format!("no item_json: {body}"));
    };
    let Ok(items) = serde_json::from_str::<serde_json::Value>(raw) else {
        return MintResult::Unknown(format!("item_json does not parse: {raw}"));
    };
    match items.get(0).and_then(|i| i.get("itemid")) {
        Some(serde_json::Value::String(id)) => MintResult::Granted(id.clone()),
        Some(serde_json::Value::Number(id)) => MintResult::Granted(id.to_string()),
        _ if items.as_array().is_some_and(|a| a.is_empty()) => MintResult::Refused("no item granted".into()),
        _ => MintResult::Unknown(format!("no itemid in {raw}")),
    }
}

/// Whether the worker may run: the operator switched it on AND a key exists.
pub fn live(flag: Option<&str>, key_set: bool) -> bool {
    key_set && flag.map(str::trim) == Some("1")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(pairs: &[(&str, u64)]) -> BTreeMap<String, u64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn plan_queues_each_count_past_the_watermark_once() {
        assert_eq!(plan(&m(&[("farm_box", 3)]), &m(&[])), vec![
            ("farm_box".into(), 1), ("farm_box".into(), 2), ("farm_box".into(), 3),
        ]);
        assert!(plan(&m(&[("farm_box", 3)]), &m(&[("farm_box", 3)])).is_empty(), "seen twice: nothing");
        assert!(plan(&m(&[("farm_box", 2)]), &m(&[("farm_box", 3)])).is_empty(), "an older save: nothing");
        assert_eq!(plan(&m(&[("farm_box", 500)]), &m(&[])).len() as u64, QUEUE_PER_SAVE, "capped per save");
    }

    #[test]
    fn a_response_is_a_grant_a_refusal_or_unknown() {
        let ok = serde_json::json!({"response": {"item_json": "[{\"itemid\":\"12345\",\"itemdefid\":\"1001\"}]"}});
        assert_eq!(parse_add_item(&ok), MintResult::Granted("12345".into()));
        let none = serde_json::json!({"response": {"item_json": "[]"}});
        assert!(matches!(parse_add_item(&none), MintResult::Refused(_)));
        let junk = serde_json::json!({"response": {}});
        assert!(matches!(parse_add_item(&junk), MintResult::Unknown(_)), "never guess a grant");
    }

    #[test]
    fn the_worker_runs_only_when_switched_on_with_a_key() {
        assert!(live(Some("1"), true));
        assert!(!live(Some("1"), false));
        assert!(!live(None, true));
        assert!(!live(Some("0"), true));
    }
}
