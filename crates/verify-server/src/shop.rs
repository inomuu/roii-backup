//! CRYSTAL SHOP — catalog items bought with crystal, a daily cap per account (2026-10-01).
//!
//! Pure: the shelf and the cap arithmetic. `GET /shop` and `POST /shop/buy` in `main.rs` read the
//! day's count, debit the wallet (bound first) and mail the items BOUND.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShopRow {
    pub def_id: &'static str,
    /// Crystal per unit.
    pub crystal: u64,
    /// Units per account per UTC day.
    pub daily_limit: u32,
}

/// The shelf. EXP Elixir: 120 crystal, 10 a day (user 2026-10-01).
pub const SHELF: &[ShopRow] = &[
    ShopRow { def_id: "exp_elixir", crystal: 120, daily_limit: 10 },
];

pub fn row(def_id: &str) -> Option<ShopRow> {
    SHELF.iter().copied().find(|r| r.def_id == def_id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopError {
    BadQty,
    DailyLimit,
}

/// Crystal for `qty` more units when `bought_today` are already taken. All-or-nothing: a quantity
/// that would cross the cap is refused whole, never trimmed.
pub fn price(r: ShopRow, qty: u32, bought_today: u32) -> Result<u64, ShopError> {
    if qty == 0 || qty > r.daily_limit {
        return Err(ShopError::BadQty);
    }
    if bought_today.saturating_add(qty) > r.daily_limit {
        return Err(ShopError::DailyLimit);
    }
    Ok(r.crystal * u64::from(qty))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cap_refuses_whole_and_never_trims() {
        let r = row("exp_elixir").unwrap();
        assert_eq!(price(r, 1, 0), Ok(120));
        assert_eq!(price(r, 10, 0), Ok(1_200));
        assert_eq!(price(r, 3, 8), Err(ShopError::DailyLimit));
        assert_eq!(price(r, 2, 8), Ok(240));
        assert_eq!(price(r, 1, 10), Err(ShopError::DailyLimit));
        assert_eq!(price(r, 0, 0), Err(ShopError::BadQty));
        assert_eq!(price(r, 11, 0), Err(ShopError::BadQty));
    }

    /// Every shelf item is a real, untradeable catalog def — crystal must not buy Steam goods.
    #[test]
    fn every_shelf_item_is_in_the_catalog_and_not_a_steam_item() {
        let data = sim_core::data::GameData::from_json_str(include_str!("../../../data/game_data.json")).unwrap();
        for r in SHELF {
            let d = data.item(r.def_id);
            assert!(d.is_some(), "{} missing from the catalog", r.def_id);
            assert!(d.is_some_and(|d| d.steam_itemdef.is_none()), "{} is a Steam item", r.def_id);
        }
    }
}
