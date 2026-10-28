//! STEAM → GAME IMPORT (user 2026-10-04): an item a player holds in their Steam inventory — bought
//! on the Market, or sent there earlier — comes back into the game. The mirror of `steam_inv`.
//!
//! Free, no waiting period, and the imported item stays TRADEABLE (it may be sent to Steam again).
//! The flow never lets an item exist twice:
//! 1. The server reads the player's Steam inventory itself (`IInventoryService/GetInventory`) —
//!    the client's word is never taken for what it holds.
//! 2. An import claims the Steam item id in `steam_imports` (`consuming`) BEFORE asking Steam to
//!    consume it, so a second request for the same id cannot run beside it.
//! 3. `IInventoryService/ConsumeItem` removes it from Steam. Only then is the game item mailed,
//!    in the same transaction that marks the row `done`.
//! 4. A consume whose answer was lost leaves the row `consuming`; the next inventory read settles
//!    it: the id GONE from Steam means it was consumed (deliver), still there means it was not
//!    (release the claim, the player may import again).
//!
//! ⚠️ The two Steam calls follow the Steamworks Web API docs and have not been run against live
//! Steam; the parsers below fail closed (`Unknown`) on any shape they do not recognise.

/// One item in a player's Steam inventory, as the server read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamItem {
    pub itemid: String,
    pub itemdefid: i64,
    pub quantity: u32,
}

/// Items one import request may name.
pub const IMPORT_PER_REQUEST: usize = 50;

/// Read a GetInventory body: `response.item_json` is a JSON STRING holding the item array (the
/// same shape AddItem answers with). `Err` on anything else — never guess an inventory.
pub fn parse_inventory(body: &serde_json::Value) -> Result<Vec<SteamItem>, String> {
    let raw = body.pointer("/response/item_json").and_then(|v| v.as_str())
        .ok_or_else(|| "no item_json".to_string())?;
    let items: Vec<serde_json::Value> = serde_json::from_str(raw).map_err(|e| format!("item_json: {e}"))?;
    Ok(items.iter().filter_map(item_of).collect())
}

fn item_of(v: &serde_json::Value) -> Option<SteamItem> {
    // Steam sends ids and counts as strings or numbers depending on the call; accept both.
    let as_string = |k: &str| v.get(k).and_then(|x| x.as_str().map(str::to_string).or_else(|| x.as_u64().map(|n| n.to_string())));
    let itemid = as_string("itemid")?;
    let itemdefid = as_string("itemdefid")?.parse::<i64>().ok()?;
    let quantity = as_string("quantity").and_then(|q| q.parse::<u32>().ok()).unwrap_or(1);
    (quantity > 0).then_some(SteamItem { itemid, itemdefid, quantity })
}

/// What one ConsumeItem call came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsumeResult {
    /// Steam consumed it.
    Consumed,
    /// Steam answered no (not owned, already gone) — nothing changed.
    Refused(String),
    /// No answer we can trust: settled by the next inventory read.
    Unknown(String),
}

/// Read a ConsumeItem body: success echoes the consumed item in `response.item_json`.
pub fn parse_consume(body: &serde_json::Value, itemid: &str) -> ConsumeResult {
    let Some(raw) = body.pointer("/response/item_json").and_then(|v| v.as_str()) else {
        return ConsumeResult::Unknown(format!("no item_json: {body}"));
    };
    match serde_json::from_str::<Vec<serde_json::Value>>(raw) {
        Ok(items) if items.iter().filter_map(item_of_any).any(|id| id == itemid) => ConsumeResult::Consumed,
        Ok(items) if items.is_empty() => ConsumeResult::Refused("nothing consumed".into()),
        Ok(_) => ConsumeResult::Unknown(format!("consumed something else: {raw}")),
        Err(e) => ConsumeResult::Unknown(format!("item_json: {e}")),
    }
}

/// The item id of a consume echo, whatever its remaining quantity (0 once consumed).
fn item_of_any(v: &serde_json::Value) -> Option<String> {
    v.get("itemid").and_then(|x| x.as_str().map(str::to_string).or_else(|| x.as_u64().map(|n| n.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_inventory_reads_ids_defs_and_counts_and_fails_closed() {
        let body = serde_json::json!({"response": {"item_json":
            r#"[{"itemid":"111","itemdefid":"1001","quantity":"1"},{"itemid":222,"itemdefid":1002,"quantity":3},{"itemid":"333","itemdefid":"1003","quantity":"0"}]"#}});
        assert_eq!(parse_inventory(&body).unwrap(), vec![
            SteamItem { itemid: "111".into(), itemdefid: 1001, quantity: 1 },
            SteamItem { itemid: "222".into(), itemdefid: 1002, quantity: 3 },
        ], "a zero-quantity row is gone");
        assert!(parse_inventory(&serde_json::json!({"response": {}})).is_err(), "never guess an inventory");
    }

    #[test]
    fn a_consume_is_trusted_only_when_steam_echoes_the_item() {
        let ok = serde_json::json!({"response": {"item_json": r#"[{"itemid":"111","quantity":"0"}]"#}});
        assert_eq!(parse_consume(&ok, "111"), ConsumeResult::Consumed);
        let none = serde_json::json!({"response": {"item_json": "[]"}});
        assert!(matches!(parse_consume(&none, "111"), ConsumeResult::Refused(_)));
        let other = serde_json::json!({"response": {"item_json": r#"[{"itemid":"999"}]"#}});
        assert!(matches!(parse_consume(&other, "111"), ConsumeResult::Unknown(_)));
        assert!(matches!(parse_consume(&serde_json::json!({}), "111"), ConsumeResult::Unknown(_)));
    }
}
