//! Store — SKUs, pricing and what a purchase grants. The closed core of the money-in path.
//!
//! NO HTTP, NO Postgres, NO Steam: the same split `gacha.rs` and `mailbox.rs` use, and it earns
//! its keep here more than anywhere else. Pricing and the first-purchase bonus are the rules a
//! player will check against the storefront, so they have to be testable without a payment
//! provider in the loop.
//!
//! Ownership boundary (`docs/unbuilt-systems-architecture.md` §5): this module turns money into
//! an ENTITLEMENT. It does not know what an entitlement is worth in the game — crystal land in
//! `crystal_wallet`, items go through the mailbox. It only says "this SKU, bought by this account,
//! for the first time or not, yields these crystal in these buckets".
//!
//! Numbers come from `docs/gacha-economy-design.md` §"Crystal packs", which priced them on
//! 2026-07-31 at half NIKKE's real in-game rate.

/// A price in one currency, in that currency's SMALLEST unit — cents, satang — as an integer,
/// which is also the only form Steam's `amount` field accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Price {
    /// ISO 4217, matching what `ISteamMicroTxn/GetUserInfo` reports for the buyer.
    pub currency: &'static str,
    pub minor: u64,
}

/// The currency every SKU must carry, and the fallback for a buyer we have no local price for.
///
/// VERIFIED 2026-08-02 against the Steam sandbox: a THB-wallet account accepted an InitTxn in
/// USD *and* one in THB, both returning a transid. So a USD quote is a working purchase
/// everywhere, not a decline — which is what makes the fallback below genuinely safe rather than
/// merely the least-bad option.
///
/// USD is therefore the default for the whole world. Local prices exist where we choose to set
/// them, because a THB wallet charged in USD is converted at Steam's rate and the player can end
/// up paying more than the local sticker — not because an unpriced country would break.
/// THB is the one that proves the per-currency path works; adding another is a data edit.
pub const BASE_CURRENCY: &str = "USD";

/// Which shelf a SKU sits on. Decides what a purchase does beyond the crystal it grants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    /// The crystal ladder: paid crystal + the once-per-SKU bound bonus.
    Crystal,
    /// A bundle: bound crystal, gacha coupons and bound items, bought up to `limit` times.
    Pack,
    /// Monthly card: paid crystal now, `MONTHLY_DAILY_CRYSTAL` bound crystal claimable per
    /// day for `MONTHLY_DAYS` days. Buying again extends the card.
    Monthly,
    /// The current season's premium Battle Pass track. Once per season.
    BattlePass,
}

/// What must be true of the account before the SKU is offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    Open,
    CommanderLevel(u32),
}

/// One purchasable SKU.
///
/// A catalog entry is DATA, not a handler. The reference doc calls this out — "Content, not
/// logic — extend this list, not the roll" — so adding a pack must never mean adding a code path.
/// Held as a `const` slice today; moving it to a table changes only where `catalog()` reads from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sku {
    pub id: &'static str,
    pub section: Section,
    /// Every currency this pack is sold in. MUST include `BASE_CURRENCY`.
    pub prices: &'static [Price],
    /// Crystal the SKU always grants, into the PAID bucket — money spent is the only paid source
    /// (pass / faucet / bonus crystal are bound).
    pub crystal: u64,
    /// Extra crystal the FIRST purchase of this SKU grants, on top of `crystal`.
    ///
    /// BOUND, deliberately: it was not paid for, so a refund must not count it as paid and the
    /// revenue books must not either. This is exactly why a grant has to name its bucket rather
    /// than just an amount.
    pub first_buy_bonus: u64,
    /// Bound crystal every purchase grants (packs).
    pub bound_crystal: u64,
    /// Standard-banner gacha coupons (`gacha::STANDARD_COUPON_KIND` counter), one pull each.
    pub coupons: u32,
    /// Catalog items, delivered BOUND through the mailbox.
    pub items: &'static [(&'static str, u32)],
    /// Lifetime purchases per account; `None` = unlimited.
    pub limit: Option<u32>,
    pub gate: Gate,
}

const fn crystal_sku(id: &'static str, crystal: u64, prices: &'static [Price]) -> Sku {
    Sku {
        id, section: Section::Crystal, prices, crystal, first_buy_bonus: crystal,
        bound_crystal: 0, coupons: 0, items: &[], limit: None, gate: Gate::Open,
    }
}

const fn usd_thb(usd: u64, thb: u64) -> [Price; 2] {
    [Price { currency: "USD", minor: usd }, Price { currency: "THB", minor: thb }]
}

/// Monthly card: days covered by one purchase, and the bound crystal each of those days pays.
pub const MONTHLY_DAYS: i64 = 30;
pub const MONTHLY_DAILY_CRYSTAL: u64 = 150;
/// A purchase is refused while more than this many days remain, so at most two cards are banked.
pub const MONTHLY_MAX_BANKED_DAYS: i64 = MONTHLY_DAYS;

/// The ladder. **The effective rate must decrease monotonically down this table** — a draft once
/// had the entry SKU at the best rate, which makes buying anything else irrational and collapses
/// the whole ladder. `the_ladder_never_rewards_buying_small` holds the line.
///
/// The $0.99 tier is deliberately absent: at this rate it buys half a pull, and an entry pack you
/// cannot spend is worse than no entry pack. $1.99 is exactly one pull.
/// THB is regional pricing, not a conversion — every gacha game sets local prices by hand rather
/// than tracking an exchange rate, and the rate implied here drifts from ~34.7 at the entry pack
/// to ~32.9 at the top on purpose. What must hold in EVERY currency is the monotonic rate.
pub const CATALOG: &[Sku] = &[
    crystal_sku("crystal_300",     300, &usd_thb(  199,   6_900)),
    crystal_sku("crystal_790",     790, &usd_thb(  499,  16_900)),
    crystal_sku("crystal_1620",  1_620, &usd_thb(  999,  33_900)),
    crystal_sku("crystal_3330",  3_330, &usd_thb(1_999,  66_900)),
    crystal_sku("crystal_8570",  8_570, &usd_thb(4_999, 169_000)),
    crystal_sku("crystal_17850", 17_850, &usd_thb(9_999, 329_000)),
    // STARTER (user 2026-09-29): $0.99, ten pulls, once per account, on sale forever.
    Sku {
        id: "pack_starter", section: Section::Pack, prices: &usd_thb(99, 3_500),
        crystal: 0, first_buy_bonus: 0, bound_crystal: 0, coupons: 10, items: &[],
        limit: Some(1), gate: Gate::Open,
    },
    // GROWTH (user 2026-09-29/30): Commander 10 / 15 / 20 / 25 / 30 — every tier inside the first
    // month on the progress-CXP curve — bound crystal + bound items. A tier stays on the shelf once
    // unlocked, so it can be bought any time after.
    // Prices climb to $49.99 at Lv50, which still pays ~2x the crystal ladder per dollar; Lv10 is the
    // best value and each tier after is worse per dollar (user 2026-09-29):
    // `growth_value_falls_tier_by_tier` holds that line.
    // Contents are placeholders until the user sets them.
    Sku {
        id: "pack_growth_10", section: Section::Pack, prices: &usd_thb(199, 6_900),
        crystal: 0, first_buy_bonus: 0, bound_crystal: 1_500, coupons: 0,
        items: &[("regalia_sr_fodder", 10), ("sigil_1", 20), ("exp_elixir", 5)],
        limit: Some(1), gate: Gate::CommanderLevel(10),
    },
    Sku {
        id: "pack_growth_15", section: Section::Pack, prices: &usd_thb(499, 16_900),
        crystal: 0, first_buy_bonus: 0, bound_crystal: 3_300, coupons: 0,
        items: &[("regalia_sr_fodder", 25), ("sigil_2", 10), ("boss_core", 3), ("exp_elixir", 10)],
        limit: Some(1), gate: Gate::CommanderLevel(15),
    },
    Sku {
        id: "pack_growth_20", section: Section::Pack, prices: &usd_thb(999, 33_900),
        crystal: 0, first_buy_bonus: 0, bound_crystal: 5_800, coupons: 0,
        items: &[("regalia_crystal_fodder", 10), ("regalia_sr_fodder", 20), ("sigil_2", 20), ("boss_core", 5)],
        limit: Some(1), gate: Gate::CommanderLevel(20),
    },
    Sku {
        id: "pack_growth_25", section: Section::Pack, prices: &usd_thb(1_999, 66_900),
        crystal: 0, first_buy_bonus: 0, bound_crystal: 10_000, coupons: 0,
        items: &[("regalia_crystal_fodder", 20), ("sigil_3", 5), ("boss_core", 8), ("track", 2)],
        limit: Some(1), gate: Gate::CommanderLevel(25),
    },
    Sku {
        id: "pack_growth_30", section: Section::Pack, prices: &usd_thb(2_999, 99_900),
        crystal: 0, first_buy_bonus: 0, bound_crystal: 13_500, coupons: 0,
        items: &[("regalia_crystal_fodder", 30), ("sigil_3", 10), ("boss_core", 15), ("track", 5)],
        limit: Some(1), gate: Gate::CommanderLevel(30),
    },
    // MONTHLY (user 2026-09-29): 300 paid crystal now + 150 bound a day for 30 days.
    Sku {
        id: "monthly_card", section: Section::Monthly, prices: &usd_thb(299, 9_900),
        crystal: 300, first_buy_bonus: 0, bound_crystal: 0, coupons: 0, items: &[],
        limit: None, gate: Gate::Open,
    },
    // BATTLE PASS premium track for the season running at checkout.
    Sku {
        id: "battle_pass", section: Section::BattlePass, prices: &usd_thb(999, 33_900),
        crystal: 0, first_buy_bonus: 0, bound_crystal: 0, coupons: 0, items: &[],
        limit: None, gate: Gate::Open,
    },
];

/// The crystal ladder alone — the rows the rate rules apply to.
pub fn ladder() -> impl Iterator<Item = &'static Sku> {
    CATALOG.iter().filter(|s| s.section == Section::Crystal)
}

/// Currencies with NO minor unit, where Steam requires the amount in increments of 100.
///
/// Verified against the sandbox 2026-08-02: `JPY 250` is refused with "currency requires price
/// increments of 100" while `JPY 300` is accepted. Nothing in the price ladder would catch that —
/// converting $1.99 at a plausible rate lands on ¥299, which looks fine, passes every test here,
/// and fails at the till.
///
/// ⚠️ This list is the currencies I could verify, not all of them. It only guards what we
/// actually PRICE, so an unlisted zero-decimal currency is harmless until someone adds a price
/// in it — and whoever does must add it here too. Probe the sandbox rather than trusting a table
/// found online; the rule is per-app, and Steam answers in one request.
pub const ZERO_DECIMAL: &[&str] = &["JPY", "KRW", "VND", "CLP", "ISK"];

pub fn sku(id: &str) -> Option<&'static Sku> {
    CATALOG.iter().find(|s| s.id == id)
}

/// What to charge a buyer whose Steam wallet is in `currency`.
///
/// Falls back to `BASE_CURRENCY` rather than refusing, because a missing local price is our gap,
/// not the player's — and per the sandbox check on `BASE_CURRENCY`, a cross-currency USD quote is
/// ACCEPTED by Steam, so the fallback completes the sale rather than failing it. What it can
/// never do is charge a local number under a foreign label. `unwrap` is safe:
/// `every_sku_is_priced_in_the_base_currency` makes the USD row a compile-time-adjacent promise.
pub fn price_for(s: &Sku, currency: &str) -> Price {
    s.prices
        .iter()
        .find(|p| p.currency == currency)
        .or_else(|| s.prices.iter().find(|p| p.currency == BASE_CURRENCY))
        .copied()
        .expect("every SKU carries a base-currency price")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    UnknownSku,
    /// The same idempotency key already produced an order — replay its result, never charge again.
    DuplicateOrder,
    /// Steam refused (not authorized, already finalized, wrong app…).
    ProviderRejected(String),
}

/// What a purchase puts in the wallet, split by bucket because the buckets never mix (I11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Grant {
    pub paid: u64,
    pub bound: u64,
}

impl Grant {
    pub fn total(self) -> u64 {
        self.paid + self.bound
    }
}

/// Resolve what `sku` grants, given whether this account has bought it before.
///
/// Pure and total: the caller decides `first_buy` by looking for a prior entitlement row, and this
/// decides nothing about money. Splitting it out is what lets the doubling be tested against the
/// published ladder without a Steam sandbox.
pub fn grant_for(s: &Sku, first_buy: bool) -> Grant {
    Grant {
        paid: s.crystal,
        bound: s.bound_crystal + if first_buy { s.first_buy_bonus } else { 0 },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ladder's whole purpose: bulk must be cheaper per crystal. A cheaper small pack makes every
    /// larger SKU irrational and the storefront pointless — this is the check the design doc asks
    /// to re-run whenever the numbers move.
    #[test]
    fn the_ladder_never_rewards_buying_small() {
        // Holds in EVERY currency: a local price list is hand-written, so it is exactly the place
        // the ordering gets broken by a typo that USD would never reveal.
        for cur in ["USD", "THB"] {
            let mut prev = u64::MAX;
            for s in ladder() {
                // Minor units per 1000 crystal — integer, so no float sneaks into a money figure.
                let rate = price_for(s, cur).minor * 1000 / s.crystal;
                assert!(
                    rate < prev,
                    "{cur}: {} at {rate} per 1k crystal is not better than the tier above ({prev})",
                    s.id,
                );
                prev = rate;
            }
        }
    }

    /// `price_for` may only ever fall back to a REAL price. A SKU missing its base currency would
    /// make the fallback panic at the till, so the promise is checked here instead.
    #[test]
    fn every_sku_is_priced_in_the_base_currency() {
        for s in CATALOG {
            assert!(
                s.prices.iter().any(|p| p.currency == BASE_CURRENCY),
                "{} has no {BASE_CURRENCY} price — the whole-world default is missing",
                s.id,
            );
            assert!(s.prices.iter().all(|p| p.minor > 0), "{} has a free price row", s.id);
        }
    }

    /// Steam refuses a zero-decimal price that is not a whole unit, and no pricing check would
    /// notice: converting $1.99 lands on ¥299, which reads as a perfectly ordinary number.
    #[test]
    fn zero_decimal_currencies_are_priced_in_whole_units() {
        for s in CATALOG {
            for p in s.prices {
                if ZERO_DECIMAL.contains(&p.currency) {
                    assert_eq!(
                        p.minor % 100, 0,
                        "{} is priced at {} {} — Steam requires increments of 100 for {}",
                        s.id, p.minor, p.currency, p.currency,
                    );
                }
            }
        }
    }

    /// A wallet we have no local price for pays the base currency — the case that covers every
    /// country nobody has priced yet, which is most of them.
    #[test]
    fn an_unpriced_currency_falls_back_to_usd_not_to_a_wrong_number() {
        let s = sku("crystal_300").unwrap();
        assert_eq!(price_for(s, "THB"), Price { currency: "THB", minor: 6_900 }, "local price wins");
        for unpriced in ["EUR", "JPY", "BRL", "", "usd"] {
            let p = price_for(s, unpriced);
            assert_eq!(p.currency, "USD", "{unpriced} falls back to the base currency");
            assert_eq!(p.minor, 199, "and to the base AMOUNT — never a local number relabelled");
        }
    }

    /// The entry SKU has to buy exactly one pull. Half a pull is unspendable, and the doc rejected
    /// a $0.99 tier for precisely that.
    #[test]
    fn the_entry_pack_buys_exactly_one_pull() {
        let entry = sku("crystal_300").unwrap();
        assert_eq!(entry.crystal, crate::gacha::TOKEN_PER_PULL, "entry pack = one pull, no remainder");
        assert_eq!(price_for(entry, "USD").minor, 199);
    }

    /// The bonus doubles, lands in the BOUND bucket, and happens once.
    #[test]
    fn the_first_purchase_doubles_into_the_bound_bucket_only() {
        let s = sku("crystal_1620").unwrap();

        let first = grant_for(s, true);
        assert_eq!(first.total(), 3_240, "first buy is 2x the sticker amount");
        assert_eq!(first.paid, 1_620, "only what was PAID for is in the paid bucket");
        assert_eq!(first.bound, 1_620, "the incentive half is pull-only — never RMT-able");

        let again = grant_for(s, false);
        assert_eq!(again, Grant { paid: 1_620, bound: 0 }, "the bonus is once per SKU");
    }

    /// Buying every tier once should land on the on-ramp the design doc advertises.
    #[test]
    fn buying_the_whole_ladder_once_matches_the_published_on_ramp() {
        let cents: u64 = ladder().map(|s| price_for(s, "USD").minor).sum();
        let crystal: u64 = ladder().map(|s| grant_for(s, true).total()).sum();
        assert_eq!(cents, 18_694, "$186.94 to buy every tier once");
        assert_eq!(crystal, 64_920, "64,920 crystal");
        let pulls = crystal / crate::gacha::TOKEN_PER_PULL;
        assert_eq!(pulls, 216, "216 pulls");
        // ~$0.86/pull — the doc's headline on-ramp rate, at roughly half sticker.
        assert_eq!(cents / pulls, 86);
    }

    /// Every item a SKU hands over exists in the catalog. A missing def fails the grant, which
    /// leaves a PAID order stuck at `charged` — so a typo here is a support ticket per buyer.
    #[test]
    fn every_pack_item_is_in_the_catalog() {
        let data = sim_core::data::GameData::from_json_str(include_str!("../../../data/game_data.json")).unwrap();
        for s in CATALOG {
            for &(def_id, qty) in s.items {
                let def = data.item(def_id);
                assert!(def.is_some(), "{} grants {def_id}, which the catalog does not have", s.id);
                // Bought value must never reach the Steam Market: no pack line may carry a Steam
                // item identity (the sellable box versions are separate defs).
                assert!(def.is_some_and(|d| d.steam_itemdef.is_none()), "{} grants {def_id}, which mints to Steam", s.id);
                assert!(qty > 0, "{} grants zero {def_id}", s.id);
            }
        }
    }

    /// Only the crystal ladder carries a first-buy bonus and paid crystal is only on the
    /// ladder and the monthly card; packs are bound and bought a bounded number of times.
    #[test]
    fn the_shelves_keep_their_rules() {
        for s in CATALOG {
            match s.section {
                Section::Crystal => assert!(s.coupons == 0 && s.items.is_empty() && s.bound_crystal == 0, "{}", s.id),
                Section::Pack => {
                    assert_eq!((s.crystal, s.first_buy_bonus), (0, 0), "{}: pack crystal is bound", s.id);
                    assert!(s.limit.is_some(), "{}: a pack needs a purchase limit", s.id);
                }
                Section::Monthly | Section::BattlePass => assert_eq!(s.first_buy_bonus, 0, "{}", s.id),
            }
        }
        let starter = sku("pack_starter").unwrap();
        assert_eq!((price_for(starter, "USD").minor, starter.coupons, starter.limit), (99, 10, Some(1)));
        let growth: Vec<u32> = CATALOG.iter().filter_map(|s| match s.gate {
            Gate::CommanderLevel(l) if s.id.starts_with("pack_growth_") => Some(l),
            _ => None,
        }).collect();
        assert_eq!(growth, vec![10, 15, 20, 25, 30], "growth packs at Commander 10/15/20/25/30");
        assert!(growth.iter().all(|l| *l <= crate::commander::MAX_LEVEL), "a tier past the level cap");
        let monthly = sku("monthly_card").unwrap();
        assert_eq!(grant_for(monthly, true), Grant { paid: 300, bound: 0 });
    }

    /// The first growth tier is the hook: each later tier costs more and pays less crystal per
    /// unit of money, in every currency.
    #[test]
    fn growth_value_falls_tier_by_tier() {
        for cur in ["USD", "THB"] {
            let mut prev: Option<(u64, u64)> = None;
            for s in CATALOG.iter().filter(|s| s.id.starts_with("pack_growth_")) {
                let price = price_for(s, cur).minor;
                // Minor units per 1000 crystal: higher = worse value.
                let rate = price * 1000 / s.bound_crystal;
                if let Some((p, r)) = prev {
                    assert!(price > p, "{cur}: {} is not dearer than the tier before", s.id);
                    assert!(rate > r, "{cur}: {} is not worse value than the tier before ({rate} vs {r})", s.id);
                }
                prev = Some((price, rate));
            }
        }
    }

    /// Every growth tier beats the crystal pack at the same price by at least 1.25x, counting that
    /// pack's first-buy bonus — crystal alone, before the items (Lv50 = 22,000, user 2026-09-29).
    #[test]
    fn growth_beats_the_first_buy_ladder_rate_by_a_quarter() {
        for g in CATALOG.iter().filter(|s| s.id.starts_with("pack_growth_")) {
            // Against the ladder's per-dollar rate at the dearest pack not above this price (the
            // ladder has no $29.99 tier), first-buy bonus included.
            let usd = price_for(g, "USD").minor;
            let Some(pack) = ladder().filter(|l| price_for(l, "USD").minor <= usd).last() else {
                panic!("{} at {usd} is cheaper than every crystal pack", g.id);
            };
            let first = grant_for(pack, true).total();
            let pack_usd = price_for(pack, "USD").minor;
            // g.crystal / usd >= 1.25 × first / pack_usd, in integers.
            assert!(g.bound_crystal * pack_usd * 4 >= first * usd * 5,
                "{}: {} crystal at {usd} is under 1.25x the {} first-buy rate", g.id, g.bound_crystal, pack.id);
        }
    }

    #[test]
    fn an_unknown_sku_resolves_to_nothing() {
        assert!(sku("crystal_free_lol").is_none());
        assert!(sku("").is_none());
    }
}
