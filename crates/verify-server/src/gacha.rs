//! Gacha — the pure core (docs/gacha-economy-design.md). NO HTTP, NO Postgres.
//!
//! Like `bp.rs` / `mailbox.rs`, everything here is plain data + math so the
//! banner tables and the pity ladder can be tested without a database. The handlers, the
//! wallet/pity tables and the delivery transaction live in `main.rs`.
//!
//! WHY THE ROLL IS HERE AND NOT IN THE SIM: a pull is money. Drawing it from the sim's PCG32
//! would let a client save-scum a bad result, and adding a draw to that stream shifts every
//! downstream outcome in the game (the butterfly rule in CLAUDE.md). The server rolls, the
//! server applies the outcome, and the sim only ever sees the delivered item (mail →
//! `ClaimMail`, which is `InputAuthority::Server` and exactly-once).
//!
//! LIVE RATES (0.0.489 — this is what `/gacha/state` discloses; the history below is kept):
//!   * **3% SSR** (`SSR_BP`) on every banner, then **10% SR / 87% R** (`SR_BP` of the rest), and a
//!     guaranteed SR-or-better after 9 pulls without one (SSR 3% / SR 97% on that pull).
//!   * NO SSR pity on Standard / Limited / Weapon. The **Newcomer** (Starter) banner alone is
//!     CERTAIN by pull 50 (`NEWCOMER_PULLS`) counted from the last SSR, stays open for all 50, and
//!     pays every SR as an SR CHARACTER the account does not own (`starter_payout`). Standard opens
//!     once the 50 are spent, and after `STANDARD_SELECT_AT` Standard pulls pays one chosen SSR.
//!   * Mileage runs in two currencies (`MileageKind`): Limited/Weapon pulls buy any exchange-pool
//!     entry, Starter/Standard pulls buy Memory Fragments only (2026-09-30).
//!   * Limited: 50/50, no carry. Weapon (Memory Fragment): every SSR is the featured card.
//!
//! Locked design (2026-07-31 — NIKKE-shape, replacing the 2026-07-30 soft-pity ladder; the 4% is
//! superseded by the live rates above):
//!   * **4% SSR flat on both banners. NO pity of any kind** — no soft ramp, no hard ceiling, no
//!     counter that resets. The rate a player sees is the rate on every single pull, forever.
//!   * Character: 50/50 on an SSR and **no carry-guarantee** — losing does NOT arm the next one.
//!     ⇒ featured = 2% flat ⇒ ~50 pulls per featured copy from the rolls alone.
//!   * Weapon: no 50/50 — every SSR is the featured one ⇒ ~25 pulls per copy.
//!   * **MILEAGE is a second, parallel currency — NOT a pity mechanic.** Every pull mints 1
//!     ticket regardless of what that pull produced, and 200 tickets buy any `EXCHANGE_POOL`
//!     unit of the player's choosing. A player who wins the unit on pull 30 keeps accruing
//!     toward the next one; nothing is refunded, reset, or forfeited.
//!   * x10 still guarantees at least one SR.
//!   * "Resonance" ranks are S0–S5 (renamed from R0–R5, which collided with rarity R).
//!
//! GETTING THE MILEAGE MODEL WRONG IS EASY AND EXPENSIVE. Read as a pity ceiling ("you get the
//! unit if you reach 200 without it") it only ever pays the unlucky, and the banner prices out
//! at ~49 pulls/copy. Read correctly — a flat 1-per-pull faucet that pays EVERY player — the two
//! sources add: 1/50 + 1/200 = **1/40 pulls per character copy**, 1/25 + 1/200 = 1/22 per weapon
//! copy. That is a 45% cut in the full-clear ceiling (660 pulls → 360), not a rounding error.
//!
//! WHY THIS SHAPE. Three things players actually praise about it, in the order they feel them:
//!   1. **Gold shows up.** A flat 4% is ~1.4 SSR a month for a F2P on our faucet, against 0.64
//!      under the old 1% ladder. Most of the perceived generosity is this number, not the price.
//!   2. **No pull is ever wasted.** Mileage accrues on EVERY pull, on EVERY banner, forever, and
//!      it buys a unit you CHOOSE — including one no longer featured. Spending into a banner you
//!      only half-want is still progress toward the one you do.
//!   3. **Nothing resets.** There is no per-banner counter to abandon at a rotation, so there is
//!      no reason to hoard, and no sunk-cost feeling when a banner ends.
//!
//! WHAT IT COSTS US, and the price that answers it. On our own crystal economy (6-copy S5 ladder,
//! TWO banner axes where NIKKE has one) the full 6+6 clear fell from 660 pulls to 360 — a 45%
//! cut to the ceiling at the old $0.65/pull, and a F2P went from 2.3 to 1.1 months per copy.
//! The price was reset to **$1.85/pull, half of NIKKE's real in-game rate** (฿3,000 → 7,000 crystal
//! ≈ $3.67/pull, verified against a live store price — NOT the cheaper third-party top-up
//! resellers, which an earlier revision of this comment wrongly quoted at $1.94–4.00). With the
//! existing first-purchase 2× bonuses the on-ramp lands at ~$35 for a first character copy
//! (cheaper than the old ladder's $53) and the ceiling at ~$438 (vs $429). Numbers, packs and
//! rationale live in `docs/gacha-economy-design.md`; nothing here depends on them.
//!
//! Everything the old ladder needed is still here as DATA, not code: `soft`/`hard`/`ramp_bp` of
//! 0 mean "disabled", and `carry_guarantee` is a bool. Reverting is an edit to the two `Rates`
//! constants below — no logic changes. It needs no `allow(dead_code)`: `ssr_chance_bp` and the
//! `Rates` fields are all still on the live call path, they just evaluate to a constant.
//!
//! Rates are basis points (10000 = 100%) and every draw is integer — the house style, and it
//! keeps these tests exact rather than float-fuzzy.

use sim_core::rng::Pcg32;

/// The two banners. Pity is tracked per TYPE (not per banner id), so a rotation never resets
/// progress — see `PityState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerType {
    Newcomer,
    Standard,
    Limited,
    Weapon,
}

/// What a single pull produced. `R` doubles as the fodder tier (an R pull is never worthless:
/// it feeds a Regalia's Enhancement Level).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rarity {
    R,
    Sr,
    Ssr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pull {
    pub rarity: Rarity,
    /// True only for an SSR that is the banner's featured unit. Always true for a weapon-banner
    /// SSR (no 50/50); on the character banner it is the 50/50 outcome, forced true when the
    /// previous SSR lost.
    pub featured: bool,
}

/// One banner's ladder. `soft`/`hard` are 1-based pull counts within the pity window; **0 on
/// either disables that half of the ladder**, which is how the live NIKKE-shape tables express
/// "no pity" without deleting the machinery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rates {
    pub base_bp: u32,
    /// Pull count after which `ramp_bp` starts compounding. 0 = no soft pity.
    pub soft: u32,
    /// Pull count at which an SSR is certain. 0 = no hard pity (mileage is the only ceiling).
    pub hard: u32,
    pub ramp_bp: u32,
    /// SR rate on a non-SSR pull (the rest is R).
    pub sr_bp: u32,
    /// False = every SSR is the featured unit (weapon banner).
    pub fifty_fifty: bool,
    /// True = losing the 50/50 arms the NEXT SSR to be featured. **False under the NIKKE shape**:
    /// mileage is the backstop instead, so a loss is just a loss. Only meaningful with
    /// `fifty_fifty`.
    pub carry_guarantee: bool,
}

/// THREE CHARACTER BANNERS + the Regalia banner (user 2026-09-21), all at **3% SSR**, then
/// `SR_BP` SR and the rest R (0.0.489 — was 97% SR with no R tier):
///   * **Newcomer** (Starter) — the standard pool, an SSR is CERTAIN by pull 50, open for all 50
///     pulls, then closed. One per account. Full price: one ticket per pull (the x10 discount was
///     removed 2026-09-30, user).
///   * **Standard** — the same pool, no pity of any kind; every SSR is a uniform pick from it.
///   * **Limited** — a pick-up unit: 50/50 against the standard pool, no carry (mileage backs it).
///   * **Weapon** — every SSR is the featured Regalia, as before.
pub const SSR_BP: u32 = 300;

/// The SR roll, taken only when the SSR roll missed (0.0.489, user 2026-09-28: the R tier is back —
/// the ten 3★ cards exist to be pulled). 1,031 of the 97% = **10% SR overall, 87% R** (user:
/// "จั่วปกติ SR น่าจะ 10%"), before the "SR in every 10" floor lifts it.
pub const SR_BP: u32 = 1_031;

pub const NEWCOMER: Rates = Rates {
    base_bp: SSR_BP,
    soft: 0,
    hard: NEWCOMER_PULLS,
    ramp_bp: 0,
    sr_bp: SR_BP,
    fifty_fifty: false,
    carry_guarantee: false,
};

pub const STANDARD: Rates = Rates {
    base_bp: SSR_BP,
    soft: 0,
    hard: 0,
    ramp_bp: 0,
    sr_bp: SR_BP,
    fifty_fifty: false,
    carry_guarantee: false,
};

pub const LIMITED: Rates = Rates {
    base_bp: SSR_BP,
    soft: 0,
    hard: 0,
    ramp_bp: 0,
    sr_bp: SR_BP,
    fifty_fifty: true,
    carry_guarantee: false,
};

/// Kept for the tests and any reader that still says "character": the LIMITED shape is what the
/// old character banner was (50/50, no carry).
pub const CHARACTER: Rates = LIMITED;

/// Same 3%, but every SSR is the featured weapon — the banner that never cheats you.
pub const WEAPON: Rates = Rates {
    base_bp: SSR_BP,
    soft: 0,
    hard: 0,
    ramp_bp: 0,
    sr_bp: SR_BP,
    fifty_fifty: false,
    carry_guarantee: false,
};

/// The newcomer banner's whole life: at most this many pulls, the last one certain.
pub const NEWCOMER_PULLS: u32 = 50;

pub fn rates_for(t: BannerType) -> Rates {
    match t {
        BannerType::Newcomer => NEWCOMER,
        BannerType::Standard => STANDARD,
        BannerType::Limited => LIMITED,
        BannerType::Weapon => WEAPON,
    }
}

/// The `gacha_pity.banner_type` key a banner type persists under. Newcomer and Standard share
/// nothing with each other; Limited keeps the historical "character" key so nobody's parked
/// counter moves.
pub fn pity_key(t: BannerType) -> &'static str {
    match t {
        BannerType::Newcomer => "newcomer",
        BannerType::Standard => "standard",
        BannerType::Limited => "character",
        BannerType::Weapon => "weapon",
    }
}

/// Every shipped SSR Exemplar — the standard pool the Newcomer and Standard banners draw from
/// uniformly and the Limited banner falls back to on a lost 50/50.
pub const STANDARD_POOL: &[&str] = &[
    "exemplar_seraphine",
    "exemplar_lapis",
    "exemplar_raplus",
    "exemplar_nemu",
    "exemplar_charon",
    "exemplar_astier",
];

/// The SR UNITS a character banner's SR pull can pay.
pub const SR_UNITS: &[&str] = &[
    "exemplar_aveline", "exemplar_lumi", "exemplar_airi", "exemplar_suzu", "exemplar_mirei",
    "exemplar_lucielle", "exemplar_sylvie",
];

/// The five SR Memory Fragments (0.0.489) — the sim's SR rung (a test binds the two). They
/// replace the Radiant Dust the SR tier used to pay: a spare card salvages into it.
pub const SR_CARDS: &[&str] = &[
    "regalia_hoarfrost_crown", "regalia_static_charm", "regalia_emberbell", "regalia_stormglass_pin",
    "regalia_thunderhead",
];

/// The nine SSR Memory Fragments — the sim's SSR rung (a test binds the two).
pub const SSR_CARDS: &[&str] = &[
    "regalia_aurora_edge", "regalia_silent_verdict", "regalia_meridian", "regalia_lycoris_lullaby",
    "regalia_tidecaller", "regalia_winters_oath", "regalia_ferrymans_lantern", "regalia_empyrean",
    "regalia_litany_of_dawn",
];

/// One live banner: which unit is featured and what the consolation pools are.
///
/// Exemplar carriers have real catalog defs; REGALIA carriers are code-only by design
/// (`items::regalia_def`, built with `Item::new` — see `debug_grant_regalia`, whose comment
/// already says "the REAL grant is the server gacha pull"). The caller resolves a def_id to an
/// `Item` accordingly.
#[derive(Debug, Clone, Copy)]
pub struct BannerCfg {
    pub id: &'static str,
    pub kind: BannerType,
    /// The SSR this banner is advertising.
    pub featured: &'static str,
    /// Off-banner SSRs — only reachable on a lost 50/50, so always empty for the weapon banner.
    pub ssr_off: &'static [&'static str],
    /// SR units an SR pull can pay.
    pub sr_pool: &'static [&'static str],
    /// SR Memory Fragments an SR pull can pay. With both lists filled an SR pull is a 50/50
    /// between them (then uniform inside); with one empty the other takes every SR.
    pub sr_cards: &'static [&'static str],
    /// The floor: one of the ten 3★ cards, equal odds (0.0.485, user 2026-09-27: "fodder เอาออกจาก
    /// เกม การ์ด 3* คือใช้แทน fodder ด้วย"). Never worthless — a card is also Enhance feed.
    pub r_pool: &'static [&'static str],
}

/// What an empty pool pays instead of nothing — the first 3★ card.
pub const R_FALLBACK: &str = "regalia_hearthguard";

/// The ten 3★ cards as carrier item ids — `sim_core::items::REGALIA_THREE_STAR` with the
/// `regalia_` prefix (a test binds the two).
pub const R_POOL: &[&str] = &[
    "regalia_hearthguard", "regalia_last_rivet", "regalia_kestrel", "regalia_emberwick", "regalia_cutpurse",
    "regalia_rouge_thread", "regalia_vesper", "regalia_wayfarers_bell", "regalia_faultline", "regalia_striking_stone",
];

pub const REGALIA_FODDER: &str = "regalia_fodder";

/// The launch banners. Content, not logic — extend this list, not the roll. A pool banner's
/// `featured` is unused for the SSR pick (`ssr_off` IS the pool) but must still resolve.
pub const BANNERS: &[BannerCfg] = &[
    BannerCfg {
        id: "newcomer",
        kind: BannerType::Newcomer,
        featured: "exemplar_seraphine",
        ssr_off: STANDARD_POOL,
        sr_pool: SR_UNITS,
        sr_cards: SR_CARDS,
        r_pool: R_POOL,
    },
    BannerCfg {
        id: "standard",
        kind: BannerType::Standard,
        featured: "exemplar_seraphine",
        ssr_off: STANDARD_POOL,
        sr_pool: SR_UNITS,
        sr_cards: SR_CARDS,
        r_pool: R_POOL,
    },
    // The limited pick-up. The unit here rotates; the lost-50/50 pool is the standard one.
    BannerCfg {
        id: "limited_nemu",
        kind: BannerType::Limited,
        featured: "exemplar_nemu",
        ssr_off: &["exemplar_seraphine", "exemplar_lapis", "exemplar_raplus", "exemplar_charon"],
        sr_pool: SR_UNITS,
        sr_cards: SR_CARDS,
        r_pool: R_POOL,
    },
    BannerCfg {
        id: "weapon_aurora_edge",
        kind: BannerType::Weapon,
        featured: "regalia_aurora_edge",
        ssr_off: &[],
        sr_pool: &[],
        sr_cards: SR_CARDS,
        r_pool: R_POOL,
    },
];

pub fn banner(id: &str) -> Option<&'static BannerCfg> {
    BANNERS.iter().find(|b| b.id == id)
}

/// The def_id a pull pays out. A lost 50/50 picks uniformly from `ssr_off`; if that pool is
/// empty (misconfigured banner) it falls back to the featured unit rather than paying nothing —
/// a player must never be charged for an empty result.
pub fn payout_def(b: &BannerCfg, p: &Pull, rng: &mut Pcg32) -> &'static str {
    match p.rarity {
        Rarity::Ssr => {
            // A POOL banner (Newcomer / Standard) has no featured unit: every SSR is a uniform
            // pick from `ssr_off`, and `roll_one` marks its SSRs featured only because there is
            // no 50/50 to lose — so the pool decides here, not the flag.
            let pool_banner = matches!(b.kind, BannerType::Newcomer | BannerType::Standard);
            if (p.featured && !pool_banner) || b.ssr_off.is_empty() {
                b.featured
            } else {
                // The draw is taken unconditionally on this branch (PRNG order is load-bearing);
                // `get` then makes "never pay nothing" the code path rather than a comment.
                let i = rng.gen_range_u32(0, b.ssr_off.len() as u32) as usize;
                b.ssr_off.get(i).copied().unwrap_or(b.featured)
            }
        }
        Rarity::Sr if !b.sr_pool.is_empty() || !b.sr_cards.is_empty() => {
            // Both filled: the 50/50 draw is taken, then the pick. One filled: no split draw.
            let pool = match (b.sr_pool.is_empty(), b.sr_cards.is_empty()) {
                (false, false) => if rng.roll_bp(5_000) { b.sr_cards } else { b.sr_pool },
                (true, _) => b.sr_cards,
                (false, true) => b.sr_pool,
            };
            let i = rng.gen_range_u32(0, pool.len() as u32) as usize;
            pool.get(i).copied().unwrap_or(R_FALLBACK)
        }
        _ => {
            let i = rng.gen_range_u32(0, b.r_pool.len().max(1) as u32) as usize;
            b.r_pool.get(i).copied().unwrap_or(R_FALLBACK)
        }
    }
}

/// Guaranteed at least one SR (or better) within every this-many pulls. This is the ONLY
/// pity-shaped mechanic left in the system, and it sits on the SR tier, not SSR.
pub const SR_GUARANTEE_WINDOW: u32 = 10;

/// The size of the multi-pull button. Equal to `SR_GUARANTEE_WINDOW` today, but they are
/// DIFFERENT concepts and must not be conflated — `cost_of` used to validate batch sizes against
/// the guarantee window, so retuning the SR floor to 20 would silently have made the x10 button
/// reject every purchase.
pub const BATCH_MULTI: u32 = 10;

/// Per (account, banner TYPE). Persisted as one row; carries across banner rotations, so a
/// player parked at 60 pity keeps it when the banner turns over.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PityState {
    /// Pulls since the last SSR.
    pub count: u32,
    /// Pulls since the last SR-or-better — drives the x10 floor.
    pub pulls_since_sr: u32,
    /// The previous SSR lost its 50/50, so the next one is forced featured. Character only.
    pub lost_5050: bool,
}

/// SSR chance for the NEXT pull given how many have passed since the last one. With the live
/// tables (`soft`/`hard`/`ramp_bp` all 0) this is a constant `base_bp` — the ladder below only
/// engages if someone re-enables it.
pub fn ssr_chance_bp(r: &Rates, count: u32) -> u32 {
    let n = count + 1; // this pull is the (count+1)-th since the last SSR
    if r.hard > 0 && n >= r.hard {
        return 10_000;
    }
    let ramp = if r.soft > 0 && n > r.soft { r.ramp_bp * (n - r.soft) } else { 0 };
    (r.base_bp + ramp).min(10_000)
}

/// Roll ONE pull, advancing `pity` in place. `force_sr` is the x10 floor (see `roll_batch`).
pub fn roll_one(r: &Rates, pity: &mut PityState, rng: &mut Pcg32, force_sr: bool) -> Pull {
    if rng.roll_bp(ssr_chance_bp(r, pity.count)) {
        // No 50/50 on the weapon banner; on the character banner a loss only forces the next win
        // if the carry-guarantee is enabled (it is NOT under the NIKKE shape — mileage covers it).
        let featured =
            !r.fifty_fifty || (r.carry_guarantee && pity.lost_5050) || rng.roll_bp(5_000);
        pity.count = 0;
        pity.pulls_since_sr = 0;
        pity.lost_5050 = if r.fifty_fifty && r.carry_guarantee { !featured } else { false };
        return Pull { rarity: Rarity::Ssr, featured };
    }
    pity.count += 1;
    if force_sr || rng.roll_bp(r.sr_bp) {
        pity.pulls_since_sr = 0;
        return Pull { rarity: Rarity::Sr, featured: false };
    }
    pity.pulls_since_sr += 1;
    Pull { rarity: Rarity::R, featured: false }
}

/// Roll `n` pulls as one batch. The LAST pull of a window is forced to SR-or-better when nothing
/// in the window (counting pulls carried in from before it) has hit yet — the "guaranteed SR in
/// every 10" every comparable game ships.
pub fn roll_batch(r: &Rates, pity: &mut PityState, rng: &mut Pcg32, n: u32) -> Vec<Pull> {
    (0..n)
        .map(|_| {
            let force = pity.pulls_since_sr + 1 >= SR_GUARANTEE_WINDOW;
            roll_one(r, pity, rng, force)
        })
        .collect()
}

/// The two crystal buckets. They are NEVER summed into one balance and never convert into each
/// other — see invariant I11 in docs/economy-invariants.md. `paid` is bought with money (what a
/// refund claws back and revenue counts); `bound` is faucet / pass / bonus. Neither is tradeable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Wallet {
    pub paid: u64,
    pub bound: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GachaError {
    InsufficientCrystal,
    BadPullCount,
    InsufficientMileage,
    NotExchangeable,
}

pub const TOKEN_PER_PULL: u64 = 300;

/// What a spend took from each bucket — the ledger writes exactly this.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Spend {
    pub from_bound: u64,
    pub from_paid: u64,
}

impl Wallet {
    pub fn total(&self) -> u64 {
        self.paid.saturating_add(self.bound)
    }

    /// Debit `cost`, spending the BOUND bucket first — the same "bound first" rule
    /// `materials::consume_n_stackable_bound_first` already applies to materials, and it keeps the
    /// crystal that carry market value in the player's hands for as long as possible.
    /// All-or-nothing: an underfunded wallet is left untouched.
    pub fn spend(&mut self, cost: u64) -> Result<Spend, GachaError> {
        if self.total() < cost {
            return Err(GachaError::InsufficientCrystal);
        }
        let from_bound = self.bound.min(cost);
        let from_paid = cost - from_bound;
        self.bound -= from_bound;
        self.paid -= from_paid;
        Ok(Spend { from_bound, from_paid })
    }
}

/// Cost of a pull batch. Only 1 and 10 are offered.
pub fn cost_of(n: u32) -> Result<u64, GachaError> {
    if n != 1 && n != BATCH_MULTI {
        return Err(GachaError::BadPullCount);
    }
    Ok(TOKEN_PER_PULL * n as u64)
}

/// COUPONS ARE THE PULL CURRENCY (user 2026-08-19). Crystal buy coupons; coupons buy pulls.
///
/// One coupon = one pull, and a coupon costs exactly what a pull used to — `TOKEN_PER_PULL`. That
/// equality is the whole safety of inserting a layer here: any other number would be a silent
/// price change wearing a refactor's clothes, and nothing in the UI would say so.
/// `a_coupon_costs_exactly_what_a_pull_used_to` is what holds it.
pub const COUPON_PRICE_GEMS: u64 = TOKEN_PER_PULL;

/// The coupon movements one pull makes: `(bought, spent)`.
///
/// Both are REAL movements and both belong in the ledger, which is the point — an earlier version
/// logged `+bought` and `-min(need, have)`, and those do not sum to what the balance actually did.
/// Holding 2 and pulling 10: the balance goes 2 -> 0, a change of -2, while the ledger claimed
/// +8 and -2 for a net of +6. Eight coupons appeared in the audit trail that no counter ever held.
///
/// `bought - spent` must equal the change in the stored balance, and that is what
/// `the_coupon_ledger_nets_to_the_balance_change` asserts for every case.
pub fn coupon_movements(need: u64, have: u64) -> (u64, u64) {
    (need.saturating_sub(have), need)
}

/// TWO TICKETS, no universal one (user 2026-09-30): a LIMITED ticket spends on the Limited and
/// Weapon banners only; a STANDARD ticket on the Starter and Standard banners only
/// (`ticket_kind`). The pre-split universal `gacha_coupon` is deleted outright by `migrate`
/// (user 2026-09-30, pre-launch).
pub const LIMITED_COUPON_KIND: &str = "gacha_coupon_limited";
pub const STANDARD_COUPON_KIND: &str = "gacha_coupon_std";

/// The ticket a banner spends (and buys with crystal on a shortfall).
pub fn ticket_kind(t: BannerType) -> &'static str {
    match t {
        BannerType::Newcomer | BannerType::Standard => STANDARD_COUPON_KIND,
        BannerType::Limited | BannerType::Weapon => LIMITED_COUPON_KIND,
    }
}

/// Either coupon kind — one pull each, priced at `COUPON_PRICE_GEMS`.
pub fn is_coupon_kind(kind: &str) -> bool {
    kind == LIMITED_COUPON_KIND || kind == STANDARD_COUPON_KIND
}

/// What a pull of `n` costs when the account already holds `have` coupons: the coupons it will
/// spend, and the crystal needed to top up the shortfall first.
///
/// Returning both is what lets the client show "you have 2, buy 8 for 2400?" before anything is
/// charged, and lets the server do the buy and the roll in ONE transaction afterwards.
pub fn pull_price(n: u32, have: u64) -> Result<(u64, u64), GachaError> {
    pull_price_on(BannerType::Standard, n, have)
}

/// `pull_price` for a banner: one ticket per pull on every banner (the Starter's x10 discount was
/// removed 2026-09-30). `t` stays so a banner can be priced differently again without a new API.
pub fn pull_price_on(_t: BannerType, n: u32, have: u64) -> Result<(u64, u64), GachaError> {
    if n != 1 && n != BATCH_MULTI {
        return Err(GachaError::BadPullCount);
    }
    let need = n as u64;
    let short = need.saturating_sub(have);
    Ok((need, short * COUPON_PRICE_GEMS))
}

/// How many pulls the Newcomer banner still allows an account that has made `made`: 0 once
/// `closed` (at `NEWCOMER_PULLS`; a row closed by an early SSR before 2026-09-30 stays closed).
/// A x10 past the end is refused rather than trimmed — the client asks for what is left.
pub fn newcomer_remaining(made: u32, closed: bool) -> u32 {
    if closed { 0 } else { NEWCOMER_PULLS.saturating_sub(made) }
}

/// STARTER BANNER SR RULE (user 2026-09-30): an SR pull pays an SR CHARACTER the account does not
/// own yet, uniform among those; once every SR character is owned it falls back to the ordinary
/// SR payout (a unit or a Memory Fragment). Everything else pays as `payout_def`. With the
/// SR-in-every-ten guarantee this is a new character in every ten pulls while new ones remain.
/// `owned` holds unit ids (`suzu`), not exemplar def ids.
pub fn starter_payout(
    b: &BannerCfg,
    p: &Pull,
    rng: &mut Pcg32,
    owned: &std::collections::BTreeSet<String>,
) -> &'static str {
    if p.rarity == Rarity::Sr {
        let fresh: Vec<&'static str> = b
            .sr_pool
            .iter()
            .copied()
            .filter(|d| d.strip_prefix("exemplar_").is_some_and(|u| !owned.contains(u)))
            .collect();
        if !fresh.is_empty() {
            let i = rng.gen_range_u32(0, fresh.len() as u32) as usize;
            return fresh.get(i).copied().unwrap_or(R_FALLBACK);
        }
    }
    payout_def(b, p, rng)
}

/// Standard pulls after which the account may pick ONE SSR character from `STANDARD_POOL`, once
/// (user 2026-09-30).
pub const STANDARD_SELECT_AT: u32 = 100;

pub fn is_standard_selectable(def_id: &str) -> bool {
    STANDARD_POOL.contains(&def_id)
}

/// Which mileage a banner's pulls mint (user 2026-09-30: "ตู้ limit แลกตัวอะไรก็ได้").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MileageKind {
    /// Limited and Weapon pulls: buys any `EXCHANGE_POOL` entry, characters included.
    Limited,
    /// Starter and Standard pulls: buys Memory Fragments only, so standard pulls are never a path
    /// to a limited unit.
    Standard,
}

pub fn mileage_kind(t: BannerType) -> MileageKind {
    match t {
        BannerType::Newcomer | BannerType::Standard => MileageKind::Standard,
        BannerType::Limited | BannerType::Weapon => MileageKind::Limited,
    }
}

/// The table a mileage kind is held in. `gacha_mileage` predates the split, so every balance
/// earned before it stays spendable on characters.
pub fn mileage_table(k: MileageKind) -> &'static str {
    match k {
        MileageKind::Limited => "gacha_mileage",
        MileageKind::Standard => "gacha_mileage_std",
    }
}

/// What Standard mileage buys: the Memory Fragments of `EXCHANGE_POOL`, named rather than matched
/// by id shape (a test binds this to the pool).
pub const EXCHANGE_POOL_STD: &[&str] = &[
    "regalia_aurora_edge",
    "regalia_silent_verdict",
    "regalia_meridian",
    "regalia_lycoris_lullaby",
    "regalia_tidecaller",
    "regalia_winters_oath",
    "regalia_ferrymans_lantern",
    "regalia_empyrean",
    "regalia_litany_of_dawn",
];

pub fn is_exchangeable_with(k: MileageKind, def_id: &str) -> bool {
    match k {
        MileageKind::Limited => is_exchangeable(def_id),
        MileageKind::Standard => EXCHANGE_POOL_STD.contains(&def_id),
    }
}

// ---------------------------------------------------------------------------------------------
// MILEAGE — a second currency running in parallel with the rolls. NOT pity.
//
// One ticket per pull, on every banner, into ONE shared pool, minted regardless of what the pull
// produced. It never expires and is never scoped to a banner. `MILEAGE_COST` tickets buy any
// `EXCHANGE_POOL` entry — including a unit that is not currently featured.
//
// It pays EVERY player, not just unlucky ones, which is what makes it a faucet rather than a
// ceiling: a 200-pull player has banked a free unit of their choice on top of whatever the rolls
// already gave them. The economics of the whole banner follow from this — see the module header.
//
// This is deliberately MORE portable than the game it is modelled on, which splits its tickets
// into two non-fungible tracks. Ours is one pool because "แลกอะไรก็ได้" is the whole point: a
// pull spent on a banner you only half-want is still real progress toward the unit you do want.
// ---------------------------------------------------------------------------------------------

/// Tickets minted per pull.
pub const MILEAGE_PER_PULL: u64 = 1;

/// Tickets required for one exchange.
pub const MILEAGE_COST: u64 = 200;

/// Everything mileage can buy. NOT limited to the live banners — an off-rotation unit is exactly
/// what the pool is for. Extend this as SSR content lands.
/// ⚠️ SSR-ONLY BY CONTRACT. `gacha_view.gd` labels every exchange result as SSR because it has
/// no rarity to read back — adding a non-SSR entry here would make that label lie. If a lower
/// tier ever belongs in the pool, send the rarity in the `/gacha/exchange` response first.
pub const EXCHANGE_POOL: &[&str] = &[
    "exemplar_seraphine",
    "exemplar_lapis",
    "exemplar_raplus",
    "exemplar_nemu",
    "exemplar_charon",
    "exemplar_astier",
    "regalia_aurora_edge",
    "regalia_silent_verdict",
    "regalia_meridian",
    "regalia_lycoris_lullaby",
    "regalia_tidecaller",
    "regalia_winters_oath",
    "regalia_ferrymans_lantern",
    "regalia_empyrean",
    "regalia_litany_of_dawn",
];

pub fn is_exchangeable(def_id: &str) -> bool {
    EXCHANGE_POOL.contains(&def_id)
}

/// Tickets held, and what an exchange does to them. Separate from `Wallet` on purpose: mileage
/// is not a crystal, cannot be bought, and must never be summed with or converted into one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mileage {
    pub tickets: u64,
}

impl Mileage {
    /// Accrue for a batch of `n` pulls.
    pub fn earn(&mut self, n: u32) {
        self.tickets = self.tickets.saturating_add(MILEAGE_PER_PULL * n as u64);
    }

    /// All-or-nothing: an unaffordable or ineligible exchange leaves the balance untouched.
    /// Limited mileage — see `exchange_in`.
    pub fn exchange(&mut self, def_id: &str) -> Result<u64, GachaError> {
        self.exchange_in(MileageKind::Limited, def_id)
    }

    /// `exchange` from the given currency: Standard mileage refuses anything but a Memory Fragment.
    pub fn exchange_in(&mut self, k: MileageKind, def_id: &str) -> Result<u64, GachaError> {
        if !is_exchangeable_with(k, def_id) {
            return Err(GachaError::NotExchangeable);
        }
        if self.tickets < MILEAGE_COST {
            return Err(GachaError::InsufficientMileage);
        }
        self.tickets -= MILEAGE_COST;
        Ok(MILEAGE_COST)
    }
}

#[cfg(test)]
mod coupon_tests {
    use super::*;

    /// THE LEDGER NETS TO THE BALANCE CHANGE, for every reachable case.
    ///
    /// `bound_items` is debited by `min(need, have)` — the part that came out of the stored
    /// balance — so the two ledger rows must sum to exactly that. They did not: the spend row
    /// recorded `min(need, have)` while the buy row recorded the shortfall, and holding 2 while
    /// pulling 10 wrote +8 and -2 against a balance that fell by 2.
    ///
    /// Pure arithmetic on purpose. The pg test that was supposed to cover this drives COPIES of
    /// the statements and never calls `gacha_roll`, so it agreed with itself and could not see
    /// the disagreement. This is the claim, stated where it can be checked.
    #[test]
    fn the_coupon_ledger_nets_to_the_balance_change() {
        for need in [1u64, 10] {
            for have in [0u64, 1, 2, 5, 9, 10, 11, 99] {
                let (bought, spent) = coupon_movements(need, have);
                let net = bought as i64 - spent as i64;
                // What the handler's UPDATE actually takes off the stored balance.
                let debited = need.min(have) as i64;
                assert_eq!(
                    net, -debited,
                    "need={need} have={have}: ledger nets {net} against a balance change of \
                     {}, so {} coupons exist in the audit trail that no counter ever held",
                    -debited,
                    (net + debited).abs()
                );
                // And the spend is the whole pull, not just the part that was already owned.
                assert_eq!(spent, need, "a pull of {need} must record {need} coupons spent");
                assert_eq!(bought, need.saturating_sub(have), "the buy is only the shortfall");
            }
        }
    }

    /// A COUPON COSTS EXACTLY WHAT A PULL USED TO. Inserting a currency between crystal and pulls is
    /// only safe while that holds — any other number is a silent price change wearing a
    /// refactor's clothes, and nothing on screen would say so.
    #[test]
    fn a_coupon_costs_exactly_what_a_pull_used_to() {
        assert_eq!(COUPON_PRICE_GEMS, TOKEN_PER_PULL);
        assert_eq!(pull_price(1, 0).map(|(_, g)| g), Ok(cost_of(1).unwrap()));
        assert_eq!(pull_price(BATCH_MULTI, 0).map(|(_, g)| g), Ok(cost_of(BATCH_MULTI).unwrap()));
    }

    /// The quote the confirm dialog is built from: hold 2, press x10, buy 8.
    #[test]
    fn a_short_balance_quotes_only_the_shortfall() {
        let (need, crystal) = pull_price(BATCH_MULTI, 2).expect("valid count");
        assert_eq!(need, 10);
        assert_eq!(crystal, 8 * COUPON_PRICE_GEMS, "the player was charged for coupons they hold");
        // Enough coupons: no crystal at all.
        assert_eq!(pull_price(BATCH_MULTI, 10), Ok((10, 0)));
        assert_eq!(pull_price(BATCH_MULTI, 99), Ok((10, 0)), "a surplus must not be spent");
        assert_eq!(pull_price(1, 0), Ok((1, COUPON_PRICE_GEMS)));
    }

    /// Only 1 and 10 are pulls, whatever the balance — the count is validated before the price.
    #[test]
    fn only_a_real_pull_count_gets_a_price() {
        assert!(pull_price(0, 100).is_err());
        assert!(pull_price(3, 100).is_err());
        assert!(pull_price(11, 100).is_err());
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    fn owned(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    /// An SR on the Starter banner is always a character the account lacks, while one remains.
    #[test]
    fn starter_sr_is_a_new_character_until_none_are_left() {
        let b = banner("newcomer").unwrap();
        let sr = Pull { rarity: Rarity::Sr, featured: false };
        let mut have = owned(&["suzu", "lumi"]);
        let mut rng = Pcg32::new(7);
        for _ in 0..SR_UNITS.len() {
            let before = SR_UNITS.iter().filter(|d| !have.contains(d.trim_start_matches("exemplar_"))).count();
            if before == 0 {
                break;
            }
            let d = starter_payout(b, &sr, &mut rng, &have);
            let u = d.strip_prefix("exemplar_").expect("an SR character while new ones remain");
            assert!(have.insert(u.to_string()), "{u} was already owned");
        }
        // Everything owned: the ordinary SR payout (a unit or an SR Memory Fragment).
        for _ in 0..50 {
            let d = starter_payout(b, &sr, &mut rng, &have);
            assert!(SR_UNITS.contains(&d) || SR_CARDS.contains(&d), "{d}");
        }
        // SSR and R are untouched by the rule.
        let ssr = starter_payout(b, &Pull { rarity: Rarity::Ssr, featured: false }, &mut rng, &have);
        assert!(STANDARD_POOL.contains(&ssr), "SSR on the Starter banner is a character: {ssr}");
    }

    /// Starter/Standard mileage buys Memory Fragments only; Limited mileage buys anything.
    #[test]
    fn standard_mileage_buys_fragments_and_limited_mileage_buys_characters_too() {
        assert_eq!(mileage_kind(BannerType::Newcomer), MileageKind::Standard);
        assert_eq!(mileage_kind(BannerType::Standard), MileageKind::Standard);
        assert_eq!(mileage_kind(BannerType::Limited), MileageKind::Limited);
        let mut m = Mileage { tickets: MILEAGE_COST * 3 };
        assert_eq!(m.exchange_in(MileageKind::Standard, "exemplar_nemu"), Err(GachaError::NotExchangeable));
        assert_eq!(m.exchange_in(MileageKind::Standard, "regalia_aurora_edge"), Ok(MILEAGE_COST));
        assert_eq!(m.exchange_in(MileageKind::Limited, "exemplar_nemu"), Ok(MILEAGE_COST));
        // The Standard list is exactly the pool's Memory Fragments: in the pool, and every
        // non-character entry of the pool is on it.
        assert!(EXCHANGE_POOL_STD.iter().all(|d| EXCHANGE_POOL.contains(d)), "a Standard entry left the pool");
        for d in EXCHANGE_POOL {
            let is_unit = STANDARD_POOL.contains(d) || BANNERS.iter().any(|b| b.featured == *d && b.kind == BannerType::Limited);
            assert_eq!(EXCHANGE_POOL_STD.contains(d), !is_unit, "{d}: Standard mileage buys fragments, never units");
        }
        assert!(is_standard_selectable("exemplar_seraphine") && !is_standard_selectable("exemplar_aveline"), "SSR characters only");
    }

    use super::*;

    /// Average pulls per FEATURED copy, measured the way the design review computed it. Featured
    /// costs ~1.5 SSRs on the character banner (50/50 + guaranteed next) and exactly 1 on the
    /// weapon banner.
    fn avg_pulls_to_featured(r: &Rates, seed: u64, copies: u32) -> f64 {
        let mut rng = Pcg32::new(seed);
        let mut pity = PityState::default();
        let (mut pulls, mut got) = (0u64, 0u32);
        while got < copies {
            pulls += 1;
            let p = roll_one(r, &mut pity, &mut rng, false);
            if p.rarity == Rarity::Ssr && p.featured {
                got += 1;
            }
        }
        pulls as f64 / copies as f64
    }

    #[test]
    fn character_banner_lands_on_its_locked_average() {
        // 3% SSR x 50/50 with no carry-guarantee = 1.5% featured flat = ~67 pulls/copy.
        let avg = avg_pulls_to_featured(&CHARACTER, 42, 20_000);
        assert!((61.0..73.0).contains(&avg), "locked at ~67 pulls/featured, got {avg:.1}");
    }

    #[test]
    fn weapon_banner_lands_on_its_locked_average() {
        // 3% SSR, all featured = ~33 pulls/copy.
        let avg = avg_pulls_to_featured(&WEAPON, 42, 20_000);
        assert!((30.0..36.0).contains(&avg), "locked at ~33 pulls/featured, got {avg:.1}");
    }

    /// The headline number the shape was chosen for: how often ANY gold appears.
    #[test]
    fn ssr_shows_up_about_every_thirty_three_pulls() {
        let mut rng = Pcg32::new(17);
        let mut pity = PityState::default();
        let n = 200_000;
        let ssr = (0..n).filter(|_| roll_one(&CHARACTER, &mut pity, &mut rng, false).rarity == Rarity::Ssr).count();
        let per = n as f64 / ssr as f64;
        assert!((30.0..36.0).contains(&per), "3% flat = ~33 pulls/SSR, got {per:.1}");
    }

    /// The weapon banner's whole selling point: it never cheats you.
    #[test]
    fn weapon_ssr_is_always_featured_and_never_sets_the_flag() {
        let mut rng = Pcg32::new(7);
        let mut pity = PityState::default();
        for _ in 0..20_000 {
            let p = roll_one(&WEAPON, &mut pity, &mut rng, false);
            if p.rarity == Rarity::Ssr {
                assert!(p.featured, "weapon banner has no 50/50");
            }
            assert!(!pity.lost_5050, "the flag is character-only");
        }
    }

    /// The defining property of the shape: the odds NEVER move. Not after 10 pulls, not after
    /// 500. If this test starts failing, someone re-introduced a pity ladder.
    #[test]
    fn the_rate_is_flat_forever() {
        for r in [&CHARACTER, &WEAPON] {
            for count in [0, 1, 50, 89, 200, 1_000, 100_000] {
                assert_eq!(ssr_chance_bp(r, count), r.base_bp, "the rate moved at count={count}");
            }
        }
        // ...and no run length is impossible, which is the cost of having no ceiling. A 2%-flat
        // featured rate WILL sometimes run past 200 pulls; mileage is what covers that, not luck.
        let mut rng = Pcg32::new(1);
        let mut pity = PityState::default();
        let mut longest = 0u32;
        let mut run = 0u32;
        for _ in 0..200_000 {
            run += 1;
            if roll_one(&CHARACTER, &mut pity, &mut rng, false).rarity == Rarity::Ssr {
                longest = longest.max(run);
                run = 0;
            }
        }
        // 90 was the old hard-pity ceiling. Blowing past it is the proof there is no ceiling now.
        assert!(longest > 90, "with no hard pity a drought must exceed the old ceiling, got {longest}");
    }

    /// The soft-pity machinery is retained but disabled. If it is ever switched back on it must
    /// still behave — this guards the revert path the module header promises.
    #[test]
    fn the_disabled_pity_ladder_still_works_when_re_enabled() {
        let laddered = Rates { base_bp: 100, soft: 74, hard: 90, ramp_bp: 600, ..CHARACTER };
        assert_eq!(ssr_chance_bp(&laddered, 0), 100);
        assert_eq!(ssr_chance_bp(&laddered, laddered.soft - 1), 100);
        assert_eq!(ssr_chance_bp(&laddered, laddered.soft), 100 + 600);
        assert_eq!(ssr_chance_bp(&laddered, laddered.hard - 1), 10_000);
        for seed in 0..200u64 {
            let mut rng = Pcg32::new(seed);
            let mut pity = PityState::default();
            let mut n = 0u32;
            loop {
                n += 1;
                let hit = roll_one(&laddered, &mut pity, &mut rng, false).rarity == Rarity::Ssr;
                assert!(n <= laddered.hard, "seed {seed}: passed hard pity without an SSR");
                if hit {
                    break;
                }
            }
        }
    }

    /// A plain 50/50: losing it buys you NOTHING toward the next one. That is the trade mileage
    /// pays for, and asserting it stops the carry-guarantee creeping back in unnoticed.
    #[test]
    fn the_5050_is_plain_and_a_loss_arms_nothing() {
        let mut rng = Pcg32::new(3);
        let mut pity = PityState::default();
        let (mut won, mut lost) = (0u32, 0u32);
        for _ in 0..200_000 {
            let p = roll_one(&CHARACTER, &mut pity, &mut rng, false);
            if p.rarity == Rarity::Ssr {
                if p.featured { won += 1 } else { lost += 1 }
            }
            assert!(!pity.lost_5050, "the carry-guarantee is off — nothing may ever arm it");
        }
        let share = won as f64 / (won + lost) as f64;
        assert!((0.47..0.53).contains(&share), "a true 50/50, got {share:.3} featured");
    }

    /// ...but re-enabling it restores the guarantee, for the same revert-path reason as above.
    #[test]
    fn the_carry_guarantee_works_when_re_enabled() {
        let guaranteed = Rates { carry_guarantee: true, ..CHARACTER };
        let mut rng = Pcg32::new(3);
        let mut pity = PityState::default();
        let mut saw_loss = false;
        for _ in 0..50_000 {
            let was_owed = pity.lost_5050;
            let p = roll_one(&guaranteed, &mut pity, &mut rng, false);
            if p.rarity == Rarity::Ssr {
                if was_owed {
                    assert!(p.featured, "an owed SSR must be featured");
                    assert!(!pity.lost_5050, "the debt is cleared once paid");
                }
                if !p.featured {
                    saw_loss = true;
                    assert!(pity.lost_5050, "a loss must arm the guarantee");
                }
            }
        }
        assert!(saw_loss, "the 50/50 does lose sometimes");
    }

    #[test]
    fn every_ten_pull_contains_at_least_one_sr() {
        let mut rng = Pcg32::new(11);
        let mut pity = PityState::default();
        for batch in 0..2_000 {
            let out = roll_batch(&CHARACTER, &mut pity, &mut rng, 10);
            assert_eq!(out.len(), 10);
            assert!(
                out.iter().any(|p| p.rarity != Rarity::R),
                "batch {batch} was all R — the x10 floor failed"
            );
        }
    }

    #[test]
    fn spend_drains_the_bound_bucket_first() {
        // Enough bound alone: the paid crystal are untouched.
        let mut w = Wallet { paid: 1_000, bound: 500 };
        assert_eq!(w.spend(300), Ok(Spend { from_bound: 300, from_paid: 0 }));
        assert_eq!(w, Wallet { paid: 1_000, bound: 200 });

        // Not enough bound: it empties first, the remainder comes from paid.
        assert_eq!(w.spend(300), Ok(Spend { from_bound: 200, from_paid: 100 }));
        assert_eq!(w, Wallet { paid: 900, bound: 0 });
    }

    #[test]
    fn an_underfunded_wallet_is_left_untouched() {
        let mut w = Wallet { paid: 100, bound: 100 };
        assert_eq!(w.spend(300), Err(GachaError::InsufficientCrystal));
        assert_eq!(w, Wallet { paid: 100, bound: 100 }, "all-or-nothing");
    }

    /// I11: buckets never convert. Whatever a spend takes must come out of the two buckets in
    /// exactly the amounts reported — no rounding, no cross-over, no minting.
    #[test]
    fn fuzz_bucket_separation_and_conservation() {
        let mut rng = Pcg32::new(99);
        for seed in 0..50u64 {
            let mut w = Wallet {
                paid: rng.gen_range_u32(0, 5_000) as u64,
                bound: rng.gen_range_u32(0, 5_000) as u64,
            };
            let (mut spent_t, mut spent_b) = (0u64, 0u64);
            let (t0, b0) = (w.paid, w.bound);
            let mut r = Pcg32::new(seed);
            for _ in 0..500 {
                let n = if r.roll_bp(5_000) { 1 } else { 10 };
                let cost = cost_of(n).unwrap();
                let before = w;
                match w.spend(cost) {
                    Ok(s) => {
                        assert_eq!(s.from_bound + s.from_paid, cost, "debit must equal cost");
                        spent_b += s.from_bound;
                        spent_t += s.from_paid;
                    }
                    Err(_) => assert_eq!(w, before, "a refused spend mutates nothing"),
                }
                assert_eq!(w.bound + spent_b, b0, "I11: bound bucket conserved on its own");
                assert_eq!(w.paid + spent_t, t0, "I11: paid bucket conserved on its own");
            }
        }
    }

    #[test]
    fn payout_never_pays_nothing_and_respects_the_5050() {
        let mut rng = Pcg32::new(5);
        let ch = banner("limited_nemu").unwrap();
        let wp = banner("weapon_aurora_edge").unwrap();

        let won = Pull { rarity: Rarity::Ssr, featured: true };
        let lost = Pull { rarity: Rarity::Ssr, featured: false };
        assert_eq!(payout_def(ch, &won, &mut rng), "exemplar_nemu");
        let off = payout_def(ch, &lost, &mut rng);
        assert!(ch.ssr_off.contains(&off) && off != "exemplar_nemu", "a loss pays off-banner: {off}");
        // A POOL banner ignores the featured flag: every SSR is a pick from the pool.
        let sp = banner("standard").unwrap();
        let picks: std::collections::BTreeSet<&str> = (0..200).map(|_| payout_def(sp, &won, &mut rng)).collect();
        assert!(picks.len() >= 4 && picks.iter().all(|p| STANDARD_POOL.contains(p)), "{picks:?}");
        // The weapon banner has no off-pool — even a (impossible) non-featured SSR must still
        // pay the featured unit rather than nothing.
        assert_eq!(payout_def(wp, &lost, &mut rng), "regalia_aurora_edge");

        // The two lower tiers must pay DIFFERENT things, or the SR guarantee is a colour. This
        // asserted they were the same until 2026-08-10, which was the bug: `sr_pool` was empty so
        // `payout_def` fell through to the R pool and every purple frame paid an R shard.
        // SR on a character banner: a 50/50 between the SR units and the SR cards (0.0.489).
        let sr = Pull { rarity: Rarity::Sr, featured: false };
        let srs: Vec<&str> = (0..1000).map(|_| payout_def(ch, &sr, &mut rng)).collect();
        assert!(srs.iter().all(|p| SR_UNITS.contains(p) || SR_CARDS.contains(p)), "SR pays only the SR tier");
        let units = srs.iter().filter(|p| SR_UNITS.contains(p)).count();
        assert!((400..=600).contains(&units), "units vs cards is a 50/50: {units}/1000");
        // The Memory Fragment banner's SR is always a card.
        assert!((0..200).all(|_| SR_CARDS.contains(&payout_def(wp, &sr, &mut rng))), "weapon SR = SR cards");
        let r = payout_def(ch, &Pull { rarity: Rarity::R, featured: false }, &mut rng);
        assert!(R_POOL.contains(&r), "R pays a 3★ card");
        assert!(!SR_UNITS.contains(&r) && !SR_CARDS.contains(&r), "an SR that pays exactly what an R pays is a label, not a reward");
    }

    /// The card lists are the sim's rungs: every SR / SSR Memory Fragment the sim knows is in its
    /// list, and nothing else — a new card cannot ship unpullable.
    #[test]
    fn card_pools_match_the_sim_rungs() {
        let rung = |id: &str| id.strip_prefix("regalia_").and_then(sim_core::items::regalia_rarity_index);
        for (list, want) in [(SR_CARDS, 1usize), (SSR_CARDS, 2)] {
            assert!(list.iter().all(|id| rung(id) == Some(want) && sim_core::items::regalia_def(id).is_some()), "{list:?}");
            let set: std::collections::BTreeSet<_> = list.iter().collect();
            assert_eq!(set.len(), list.len(), "no duplicates");
        }
        let all = sim_core::items::regalia_all_ids();
        for (want, list) in [(1usize, SR_CARDS), (2, SSR_CARDS)] {
            let n = all.iter().filter(|id| sim_core::items::regalia_rarity_index(id) == Some(want)).count();
            assert_eq!(n, list.len(), "rung {want}: the sim has {n} cards, the pool {}", list.len());
        }
        assert!(SSR_CARDS.iter().all(|id| EXCHANGE_POOL.contains(id)), "every SSR card is in the mileage pool");
    }

    /// A normal pull is 3% SSR / 10% SR / 87% R before the x10 floor, and the disclosed SR share
    /// reads exactly 10%.
    #[test]
    fn the_r_tier_is_back() {
        let mut rng = sim_core::rng::Pcg32::new(5);
        let mut pity = PityState::default();
        let (mut ssr, mut sr, mut r) = (0u32, 0u32, 0u32);
        for _ in 0..100_000 {
            match roll_one(&STANDARD, &mut pity, &mut rng, false).rarity {
                Rarity::Ssr => ssr += 1,
                Rarity::Sr => sr += 1,
                Rarity::R => r += 1,
            }
        }
        assert!((2_600..=3_400).contains(&ssr), "{ssr}");
        assert!((9_400..=10_600).contains(&sr), "{sr}");
        assert!((86_000..=88_000).contains(&r), "{r}");
        assert_eq!(u64::from(10_000 - STANDARD.base_bp) * u64::from(STANDARD.sr_bp) / 10_000, 1_000);
    }

    /// 0.0.485: the R tier is the ten 3★ cards, equal odds, and no fodder — bound to the sim's list.
    #[test]
    fn an_r_pull_pays_one_of_the_ten_three_star_cards_evenly() {
        let want: Vec<String> = sim_core::items::REGALIA_THREE_STAR.iter().map(|id| format!("regalia_{id}")).collect();
        assert_eq!(R_POOL.iter().map(|s| s.to_string()).collect::<Vec<_>>(), want);
        let b = &BANNERS[0];
        let mut rng = sim_core::rng::Pcg32::new(11);
        let mut seen = std::collections::BTreeMap::<&str, u32>::new();
        for _ in 0..10_000 {
            *seen.entry(payout_def(b, &Pull { rarity: Rarity::R, featured: false }, &mut rng)).or_default() += 1;
        }
        assert_eq!(seen.len(), 10, "all ten, nothing else: {seen:?}");
        assert!(!seen.contains_key(REGALIA_FODDER));
        assert!(seen.values().all(|n| (850..1_150).contains(n)), "equal odds: {seen:?}");
    }

    #[test]
    fn every_banner_resolves_to_a_payout_for_every_rarity() {
        // AGAINST THE CATALOG, not against emptiness.
        //
        // This used to assert `!out.is_empty()` on a `&'static str` — which is true of every
        // string literal anyone would ever write here, so pointing a banner at an id that exists
        // nowhere in the repo left it green. What a pull owes the player is a def the game can
        // actually mint; that is the property, and it needs the real catalog to state.
        let data = crate::GameData::from_json_str(include_str!(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json")
        ))
        .expect("game_data.json must parse");
        // ENUMERATE THE POOLS, do not sample them. `payout_def` takes one draw and returns ONE
        // element, so calling it a handful of times leaves most of `ssr_off` / `sr_pool` /
        // `r_pool` untouched — a bad def sitting in the middle of a pool would simply never be
        // picked, and the test would pass while the pull that eventually hits it cannot be minted.
        let mut checked = 0;
        for b in BANNERS {
            for id in [b.featured]
                .iter()
                .chain(b.ssr_off.iter())
                .chain(b.sr_pool.iter())
            {
                assert!(data.item(id).is_some(),
                    "banner {} has `{id}` in a payout pool and it is not in the catalog", b.id);
                checked += 1;
            }
        }
        assert!(checked >= 4, "the pools were not enumerated — only {checked} defs seen");

        // And the resolver itself still answers for every rarity, on the real catalog.
        let mut rng = Pcg32::new(6);
        for b in BANNERS {
            for r in [Rarity::Ssr, Rarity::Sr, Rarity::R] {
                for featured in [true, false] {
                    let out = payout_def(b, &Pull { rarity: r, featured }, &mut rng);
                    assert!(!out.is_empty(), "banner {} paid nothing", b.id);
                    let def = data.item(out).unwrap_or_else(|| panic!(
                        "banner {} pays `{out}` for {r:?}, which is not in the catalog — the pull \
                         is charged and `payout_def`'s caller has nothing to mint",
                        b.id
                    ));
                    // An SSR must actually BE one. A banner quietly pointed at a lesser def would
                    // charge SSR odds and pay something else.
                    if matches!(r, Rarity::Ssr) {
                        assert_eq!(
                            format!("{:?}", def.rarity), "Legendary",
                            "banner {} pays `{out}` on an SSR pull, and it is not Legendary",
                            b.id
                        );
                    }
                }
            }
        }
    }

    /// `EXCHANGE_POOL` is SSR-ONLY, and it says so in a comment. This is the part that holds it.
    ///
    /// 200 mileage tickets buy any entry, so a lesser unit slipping in would be the cheapest SSR
    /// in the game by an order of magnitude — and the only thing standing between that and
    /// shipping was prose.
    #[test]
    fn the_exchange_pool_is_ssr_only_in_fact_and_not_only_in_the_comment() {
        let data = crate::GameData::from_json_str(include_str!(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json")
        ))
        .expect("game_data.json must parse");
        assert!(!EXCHANGE_POOL.is_empty(), "an empty pool would satisfy every check below");
        for id in EXCHANGE_POOL {
            let def = data.item(id).unwrap_or_else(|| panic!(
                "`{id}` is exchangeable for {MILEAGE_COST} tickets and is not in the catalog"
            ));
            assert_eq!(
                format!("{:?}", def.rarity), "Legendary",
                "`{id}` is in EXCHANGE_POOL at {MILEAGE_COST} tickets and is NOT SSR — it would be \
                 the cheapest top-tier unit in the game, and only a comment said otherwise"
            );
        }
        // Every SSR a banner can pay should be obtainable this way too, or mileage stops being the
        // parallel currency the module header describes and becomes a partial one.
        for b in BANNERS {
            assert!(is_exchangeable(b.featured),
                "banner {}'s featured unit `{}` cannot be bought with mileage", b.id, b.featured);
        }
    }

    #[test]
    fn only_one_and_ten_are_sellable_batch_sizes() {
        assert_eq!(cost_of(1), Ok(300));
        assert_eq!(cost_of(10), Ok(3_000));
        for bad in [0, 2, 9, 11, 100] {
            assert_eq!(cost_of(bad), Err(GachaError::BadPullCount), "n={bad}");
        }
    }

    #[test]
    fn mileage_accrues_one_per_pull_whatever_the_pull_produced() {
        let mut m = Mileage::default();
        m.earn(1);
        m.earn(10);
        assert_eq!(m.tickets, 11);
    }

    #[test]
    fn an_exchange_costs_exactly_the_threshold_and_is_all_or_nothing() {
        let mut m = Mileage { tickets: MILEAGE_COST - 1 };
        assert_eq!(m.exchange("exemplar_seraphine"), Err(GachaError::InsufficientMileage));
        assert_eq!(m.tickets, MILEAGE_COST - 1, "a refused exchange spends nothing");

        m.earn(1);
        assert_eq!(m.exchange("exemplar_seraphine"), Ok(MILEAGE_COST));
        assert_eq!(m.tickets, 0);
    }

    /// "แลกอะไรก็ได้" means the POOL, not the live banners — an off-rotation unit is the whole
    /// point. Anything outside it is refused without touching the balance.
    #[test]
    fn the_exchange_pool_spans_off_banner_units_and_refuses_everything_else() {
        // regalia_litany_of_dawn is on no banner, and must still be buyable.
        assert!(is_exchangeable("regalia_litany_of_dawn"));
        for b in BANNERS {
            assert!(is_exchangeable(b.featured), "banner {} featured unit must be buyable", b.id);
        }
        let mut m = Mileage { tickets: 10_000 };
        for bad in [REGALIA_FODDER, "", "exemplar_nonexistent"] {
            assert_eq!(m.exchange(bad), Err(GachaError::NotExchangeable), "def={bad}");
        }
        assert_eq!(m.tickets, 10_000, "refusals spend nothing");
    }

    /// THE ECONOMIC PROPERTY, and the one that was originally modelled wrong: mileage is a
    /// parallel faucet, not a pity ceiling. It pays the LUCKY too, so the two sources ADD.
    /// Character copies per pull = 1/50 (rolls) + 1/200 (mileage) = 1/40.
    #[test]
    fn mileage_pays_lucky_players_too_so_the_sources_add() {
        let mut rng = Pcg32::new(23);
        let mut pity = PityState::default();
        let mut m = Mileage::default();
        let n = 400_000u32;
        let mut copies = 0u32;
        for _ in 0..n {
            if roll_one(&CHARACTER, &mut pity, &mut rng, false) == (Pull { rarity: Rarity::Ssr, featured: true }) {
                copies += 1;
            }
            m.earn(1);
            // Spend the moment it is affordable — the player always has something to want.
            if m.exchange("exemplar_seraphine").is_ok() {
                copies += 1;
            }
        }
        let per_copy = n as f64 / copies as f64;
        // 1/67 + 1/200 = 1/50: the mileage track is a quarter of the copies.
        assert!((46.0..54.0).contains(&per_copy), "expected ~50 pulls/copy, got {per_copy:.1}");

        // And the guard against re-introducing the ceiling reading: rolls ALONE are ~67.
        let mut rng2 = Pcg32::new(23);
        let mut pity2 = PityState::default();
        let rolls_only = (0..n)
            .filter(|_| {
                roll_one(&CHARACTER, &mut pity2, &mut rng2, false)
                    == (Pull { rarity: Rarity::Ssr, featured: true })
            })
            .count();
        let per_roll_copy = n as f64 / rolls_only as f64;
        assert!(
            (61.0..73.0).contains(&per_roll_copy),
            "rolls alone must be ~67 pulls/copy, got {per_roll_copy:.1}"
        );
    }
    /// The Newcomer (Starter) banner: an SSR is certain by pull 50, a pull costs one ticket like
    /// every banner (no x10 discount), and `newcomer_remaining` closes it at 50.
    #[test]
    fn newcomer_is_certain_by_fifty_full_price_and_closes() {
        // Worst case: no SSR ever rolls until the ceiling forces one.
        let mut rng = Pcg32::new(99);
        let mut pity = PityState::default();
        let mut first = None;
        for i in 1..=NEWCOMER_PULLS {
            if roll_one(&NEWCOMER, &mut pity, &mut rng, false).rarity == Rarity::Ssr {
                first = Some(i);
                break;
            }
        }
        assert!(first.is_some_and(|i| i <= NEWCOMER_PULLS), "an SSR within 50, got {first:?}");
        assert_eq!(ssr_chance_bp(&NEWCOMER, NEWCOMER_PULLS - 1), 10_000, "the fiftieth is certain");
        assert_eq!(ssr_chance_bp(&NEWCOMER, 0), SSR_BP);
        assert_eq!(pull_price_on(BannerType::Newcomer, 10, 0), Ok((10, 10 * COUPON_PRICE_GEMS)), "no x10 discount");
        assert_eq!(pull_price_on(BannerType::Newcomer, 1, 0), Ok((1, COUPON_PRICE_GEMS)));
        assert_eq!(pull_price_on(BannerType::Standard, 10, 0), Ok((10, 10 * COUPON_PRICE_GEMS)));
        assert_eq!(newcomer_remaining(0, false), 50);
        assert_eq!(newcomer_remaining(40, false), 10);
        assert_eq!(newcomer_remaining(50, false), 0);
        assert_eq!(newcomer_remaining(12, true), 0, "a closed row stays closed");
    }

}
