//! Telemetry — the aggregate record of what actually happens to real players.
//!
//! WHY THIS EXISTS. On 2026-08-03 it emerged that the entire server-roll drop pipeline had been
//! dead for three months: every rare stopped dropping about fifteen minutes into any session,
//! silently, and nothing anywhere noticed. It was found because the user happened to ask a
//! question. After launch nobody is going to ask. A single counter — grants issued per day —
//! would have shown a flat zero from the day it broke.
//!
//! So the first thing this must do is make a dead system visible, and only then answer the
//! commercial questions.
//!
//! A CLOSED CORE, like `mailbox.rs` and `store.rs`: no sqlx, no axum, no clock. The day is a
//! parameter, never read here, so a test can drive a week of traffic in a loop. `main.rs` owns
//! the tables and the flush.
//!
//! WRITE COST IS THE WHOLE DESIGN CONSTRAINT. `put_save` runs every 10 s per active player, and
//! `TODO.md`'s measured scaling wall is the DB write rate, not CPU — 144 MB/s at 100 k MAU. So
//! nothing here writes per save. Everything accumulates in memory and `drain()` hands over a
//! batch on a timer, which makes the added write rate a function of the flush interval and the
//! number of distinct keys, NOT of concurrency.
//!
//! The cost of that: a restart drops whatever has not flushed. Acceptable — these are aggregate
//! counters for spotting trends and dead systems, not an audit trail. Anything that must be
//! exact (money, mints) belongs in the ledgers, which are transactional.
//!
//! NO PII. Counters are keyed by metric name; the per-player table holds the account id that is
//! already the primary key everywhere else, and nothing more.

use std::collections::BTreeMap;

/// What one player looked like the last time they were seen on a given day. Upserted, so a
/// player who saves 8,640 times in a day produces ONE row — which is what makes DAU, the level
/// distribution and retention all fall out of the same table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerDay {
    pub level: u32,
    pub tick: u64,
    pub gil: u64,
    /// Pending drop tickets. The number that would have exposed the 0.0.255 cap bug: it sat
    /// pegged at 500 for every established account and nothing reported it.
    pub pending_drops: u32,
    /// Where they were. The difficulty signal a level alone cannot give.
    pub zone: String,
}

/// The first time an account was ever seen in a zone: the level it ARRIVED at, and who was in
/// the party. Written once and never updated — a retreat back to an old band re-visits a zone
/// already recorded, so the row keeps the level from the first visit.
///
/// This is the difficulty curve, and it is the one thing a daily snapshot cannot give: clear
/// three bands in one day and the snapshot only ever shows the third, losing the arrival level
/// for the other two permanently.
///
/// It records ARRIVAL, not a clear. In a linear band progression arriving at band N+1 means
/// band N was cleared, and "what level do people arrive at band 7 with" is the number you
/// actually tune against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstSeen {
    pub level: u32,
    pub tick: u64,
    /// Party class ids, sorted and comma-joined, so comps GROUP BY cleanly.
    pub party: String,
}

/// In-memory accumulator. Everything is additive or last-write-wins, so a lost flush costs
/// resolution, never correctness of the surviving numbers.
#[derive(Debug, Default)]
pub struct Metrics {
    counters: BTreeMap<(i32, String), i64>,
    players: BTreeMap<(i32, u64), PlayerDay>,
    first_seen: BTreeMap<(u64, String), FirstSeen>,
}

/// One batch to persist. `BTreeMap` throughout, so the batch order is deterministic and a test
/// can assert on it.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Batch {
    pub counters: Vec<(i32, String, i64)>,
    pub players: Vec<(i32, u64, PlayerDay)>,
    pub first_seen: Vec<(u64, String, FirstSeen)>,
}

impl Batch {
    pub fn is_empty(&self) -> bool {
        self.counters.is_empty() && self.players.is_empty() && self.first_seen.is_empty()
    }
}

impl Metrics {
    /// Add to a counter. `day` is a caller-supplied day number (days since epoch) — this module
    /// never reads a clock, so tests can drive any calendar they like.
    pub fn bump(&mut self, day: i32, key: &str, by: i64) {
        if by == 0 {
            return;
        }
        *self.counters.entry((day, key.to_string())).or_insert(0) += by;
    }

    pub fn incr(&mut self, day: i32, key: &str) {
        self.bump(day, key, 1);
    }

    /// Record where a player stands. Last write wins for the day: a save every 10 s must not
    /// turn into 8,640 rows, and for "what level are people quitting at" the last sighting is
    /// the interesting one anyway.
    pub fn observe_player(&mut self, day: i32, account_id: u64, p: PlayerDay) {
        self.players.insert((day, account_id), p);
    }

    /// Note the account's presence in a zone. `or_insert` in memory and `ON CONFLICT DO
    /// NOTHING` in the table, so the FIRST sighting wins at both layers and calling this on
    /// every save costs nothing after the first.
    ///
    /// Deliberately needs no previous state: the primary key enforces "first" on its own, so
    /// there is no diff to compute, no per-account memory to keep warm, and nothing that breaks
    /// across a restart.
    pub fn observe_zone(&mut self, account_id: u64, zone: &str, f: FirstSeen) {
        if zone.is_empty() {
            return;
        }
        self.first_seen.entry((account_id, zone.to_string())).or_insert(f);
    }

    /// Hand over everything accumulated and reset. Called on a timer by `main.rs`.
    pub fn drain(&mut self) -> Batch {
        Batch {
            counters: std::mem::take(&mut self.counters)
                .into_iter()
                .map(|((d, k), v)| (d, k, v))
                .collect(),
            players: std::mem::take(&mut self.players)
                .into_iter()
                .map(|((d, a), p)| (d, a, p))
                .collect(),
            first_seen: std::mem::take(&mut self.first_seen)
                .into_iter()
                .map(|((a, z), f)| (a, z, f))
                .collect(),
        }
    }

    /// Distinct (day, key) and (day, account) pairs held right now — the thing that decides how
    /// big a flush is, and therefore the write cost.
    pub fn pending(&self) -> (usize, usize) {
        (self.counters.len(), self.players.len())
    }
}

/// Metric names. Constants rather than string literals at the call sites, because a typo'd
/// counter is a metric that silently reads zero forever — which is the exact failure this
/// module exists to catch, and it would be embarrassing to reproduce it here.
pub mod keys {
    /// Saves that passed replay-verify.
    pub const SAVE_OK: &str = "save.accepted";
    /// Saves rejected, by reason — `save.rejected.<reason>`.
    pub const SAVE_REJECTED_PREFIX: &str = "save.rejected.";

    /// The key `flag_cheat` records a rejection under. A function, not a `format!` at the call
    /// site, so a test can exercise the same code the server runs — building the key inside the
    /// test instead would assert that `BTreeMap` works.
    pub fn rejected_key(reason: &str) -> String {
        format!("{SAVE_REJECTED_PREFIX}{reason}")
    }
    /// Server-roll grants actually issued, by kind. **A flat zero here means a dead pipeline.**
    pub const GRANT_DROP: &str = "grant.drop";
    pub const GRANT_CRAFT: &str = "grant.craft";
    pub const GRANT_GEAR: &str = "grant.gear";
    // No prune counter: the CLIENT applies the prune, so this side cannot count it. The signal
    // is `PlayerDay::pending_drops` trending UP across days, which is strictly better — it
    // measures the outcome (tickets piling up) rather than the mechanism.
}

/// CLIENT-EMITTED events — the half of telemetry the server cannot observe for itself.
///
/// Everything above is read off state the server REPLAYED, so none of it depends on trusting the
/// client. This module is the opposite: the client is the only witness. A session that never
/// saved, a screen opened and abandoned, the frame rate on real hardware — the save path sees
/// none of it, and those are the questions that decide whether HD-2D survives and whether the
/// store converts.
///
/// Because the client is the witness, it is also the attacker. Three rules follow, and all three
/// are enforced HERE rather than at the endpoint, so a second caller cannot reintroduce them:
///
/// 1. **An allow-list, not a free-form key.** `Metrics::pending()` says the distinct key count is
///    what decides how big a flush is, and therefore the write rate — the one thing this whole
///    module is built to bound. A client that may name its own counters controls that number. Ten
///    thousand keys per request would be a write amplifier addressed by the attacker.
/// 2. **The `client.` prefix is added here, never sent.** Otherwise a forged event lands on
///    `grant.drop` and the alarm that exists to catch a dead pipeline reads healthy while it is
///    dead — the exact failure this file was written for, handed to anyone with a token.
/// 3. **An unrecognised name is COUNTED, not dropped.** A silently ignored event is indistinguishable
///    from a feature nobody uses, which is how a renamed constant reads zero forever.
///
/// None of this makes the numbers trustworthy per-account — a client can still lie about its own
/// events. It makes them bounded and aggregate, which is what a trend is for. Nothing here may
/// ever gate a reward.
pub mod client {
    /// The most events one request may fold. A client batches on a timer, so an honest batch is a
    /// handful; this is the ceiling that keeps one request from being a whole flush.
    pub const MAX_EVENTS: usize = 64;

    /// Every name the client may send. Adding a row here is the whole cost of a new metric.
    ///
    /// Kept flat and short on purpose: a name that has to be parsed (`screen.<id>`) is a
    /// free-form key wearing a prefix, and would put the client back in charge of the key count.
    pub const ALLOWED: &[&str] = &[
        // Sessions. The save path cannot see a session that never saved — a crash on the loading
        // screen, or a player who opened the game and left.
        //
        // A HEARTBEAT rather than a `session.end`, and that is forced rather than chosen: the
        // client cannot send anything at teardown. `sim_driver.gd` documents why for the SAVE and
        // it applies here unchanged — the HTTPRequest node is out of the tree by then, so the
        // call returns ERR_UNCONFIGURED. Session length is therefore heartbeats x flush interval,
        // which is also the only form that survives a crash rather than a clean quit.
        "session.start",
        "session.heartbeat",
        // Which screens are actually used. The answer decides what gets UI polish before launch.
        "screen.shop",
        "screen.gacha",
        // Wired by the UNITY client, not the Godot one. There, `market_view` and `craft_view` are
        // plain `Control`s rather than `UIWindow` subclasses, so they have no `telemetry_key` hook
        // to hang this on; the Unity port reports every screen from one table in `HudBar`, so both
        // arrived with it. A zero on either of these now means nobody opened them.
        "screen.market",
        "screen.craft",
        "screen.enchant",
        "screen.options",
        // The commercial question: reached the store, did not check out.
        "store.view",
        // Needs the pair to mean anything — "opened and did NOT check out" is the metric, and
        // that is flow logic in the checkout path, not a line in `_init`.
        "store.abandon",   // UNWIRED
        // HD-2D IS ON TRIAL (user, 2026-08-21: "รอดูท่าทางอีกที ว่า 3D ไปรอดไหม"). That decision
        // needs frame rates from real hardware, not from this machine. Buckets rather than a
        // number, because a number is a free-form key.
        "fps.under30",
        "fps.30to50",
        "fps.over50",
        // Effects turned off is a perf complaint the player made without filing one.
        "fx.off",
        "fx.on",
    ];

    /// The counter an unrecognised name lands on. One key, so a bad client cannot mint many.
    pub const UNKNOWN: &str = "client.unknown";

    pub fn is_allowed(name: &str) -> bool {
        ALLOWED.contains(&name)
    }

    /// The counter key for an allowed name. Private prefix, applied here.
    pub fn key(name: &str) -> String {
        format!("client.{name}")
    }
}

impl Metrics {
    /// Fold a client's batch of event names into the counters.
    ///
    /// Returns `(accepted, rejected)` so the endpoint can answer the caller and so a test can
    /// assert the split. Over-long batches are TRUNCATED rather than refused: dropping the whole
    /// request would lose the honest prefix too, and a client that oversends is far more likely
    /// to be a bug in our own flush than an attack.
    pub fn observe_client(&mut self, day: i32, names: &[String]) -> (usize, usize) {
        let (mut ok, mut bad) = (0usize, 0usize);
        for n in names.iter().take(client::MAX_EVENTS) {
            if client::is_allowed(n) {
                self.incr(day, &client::key(n));
                ok += 1;
            } else {
                self.incr(day, client::UNKNOWN);
                bad += 1;
            }
        }
        (ok, bad)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(level: u32) -> PlayerDay {
        PlayerDay { level, tick: 1, gil: 2, pending_drops: 3, zone: "zone_b1_1".into() }
    }

    #[test]
    fn counters_add_and_drain_once() {
        let mut m = Metrics::default();
        m.incr(1, keys::SAVE_OK);
        m.incr(1, keys::SAVE_OK);
        m.bump(1, keys::GRANT_GEAR, 5);
        m.bump(1, "ignored", 0); // a zero delta must not create a key
        m.incr(2, keys::SAVE_OK); // a different day is a different bucket

        let b = m.drain();
        assert_eq!(
            b.counters,
            vec![
                (1, keys::GRANT_GEAR.to_string(), 5),
                (1, keys::SAVE_OK.to_string(), 2),
                (2, keys::SAVE_OK.to_string(), 1),
            ],
        );
        assert!(m.drain().is_empty(), "draining twice must not re-emit");
    }

    /// The property the whole design rests on: write cost tracks distinct keys, not traffic.
    /// A save every 10 s per player must not become a row every 10 s per player.
    #[test]
    fn a_days_worth_of_saves_collapses_to_one_row_per_player() {
        let mut m = Metrics::default();
        for i in 0..8_640u32 {
            // one account saving all day, plus a counter each time
            m.incr(7, keys::SAVE_OK);
            m.observe_player(7, 42, p(i / 100));
        }
        m.observe_player(7, 43, p(9)); // a second player
        assert_eq!(m.pending(), (1, 2), "one counter key, two player rows");

        let b = m.drain();
        assert_eq!(b.counters, vec![(7, keys::SAVE_OK.to_string(), 8_640)]);
        // The KEYS, not just the count. Asserting `len() == 2` alone passed a sabotage that
        // mangled the day in the key — two rows, both wrong, and the test could not tell.
        let keyed: Vec<(i32, u64, u32)> =
            b.players.iter().map(|(d, a, p)| (*d, *a, p.level)).collect();
        assert_eq!(
            keyed,
            vec![(7, 42, 86), (7, 43, 9)],
            "one row per (day, account); last sighting wins, so 42 is at the level it stopped on",
        );
    }

    /// Days are separate buckets, which is what makes retention answerable at all: the same
    /// account on two days is two rows, and "did they come back" is a join.
    #[test]
    fn a_player_across_two_days_is_two_rows() {
        let mut m = Metrics::default();
        m.observe_player(1, 42, p(10));
        m.observe_player(2, 42, p(12));
        let b = m.drain();
        assert_eq!(b.players.len(), 2);
        assert_eq!((b.players[0].0, b.players[1].0), (1, 2));
    }

    /// FIRST wins and later sightings are free. Calling this on every save (every 10 s) must
    /// not overwrite the arrival level with wherever they happen to be now — that number is the
    /// whole point of the table.
    #[test]
    fn a_zone_records_the_first_arrival_and_ignores_every_later_one() {
        let mut m = Metrics::default();
        let f = |level: u32, party: &str| FirstSeen { level, tick: 100, party: party.into() };
        m.observe_zone(42, "zone_b7_1", f(63, "lapis,seraphine"));
        for lvl in 64..90 {
            m.observe_zone(42, "zone_b7_1", f(lvl, "changed_later"));
        }
        // A RETREAT to an old band must not rewrite that band's arrival either.
        m.observe_zone(42, "zone_b3_1", f(20, "seraphine"));
        m.observe_zone(42, "zone_b3_1", f(88, "retreated_to_farm"));
        m.observe_zone(42, "", f(1, "x")); // no zone = nothing to record

        let b = m.drain();
        let rows: Vec<(u64, &str, u32, &str)> = b.first_seen.iter()
            .map(|(a, z, f)| (*a, z.as_str(), f.level, f.party.as_str())).collect();
        assert_eq!(
            rows,
            vec![
                (42, "zone_b3_1", 20, "seraphine"),
                (42, "zone_b7_1", 63, "lapis,seraphine"),
            ],
            "arrival level and the party AT ARRIVAL, not the latest state",
        );
    }

    /// A rejected save is bucketed by REASON. Rejections that only ever appear as a total are
    /// useless — "50 rejects today" could be one broken client or a version skew hitting
    /// everybody, and those need different responses.
    #[test]
    fn rejections_are_keyed_by_reason() {
        let mut m = Metrics::default();
        m.incr(1, &keys::rejected_key("version-skew"));
        m.incr(1, &keys::rejected_key("version-skew"));
        m.incr(1, &keys::rejected_key("hash-mismatch"));
        let b = m.drain();
        assert_eq!(
            b.counters,
            vec![
                (1, "save.rejected.hash-mismatch".to_string(), 1),
                (1, "save.rejected.version-skew".to_string(), 2),
            ],
        );
    }
}

#[cfg(test)]
mod client_tests {
    use super::*;

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_string()).collect()
    }

    fn count(b: &Batch, key: &str) -> i64 {
        b.counters.iter().find(|(_, k, _)| k == key).map(|(_, _, v)| *v).unwrap_or(0)
    }

    #[test]
    fn allowed_event_lands_under_the_client_prefix() {
        let mut m = Metrics::default();
        let (ok, bad) = m.observe_client(1, &names(&["screen.shop", "screen.shop"]));
        assert_eq!((ok, bad), (2, 0));
        let b = m.drain();
        assert_eq!(count(&b, "client.screen.shop"), 2);
        // The name the client sent must NOT exist as a key on its own.
        assert_eq!(count(&b, "screen.shop"), 0);
    }

    /// The rule that matters most: a token holder must not be able to write a SERVER-observed
    /// counter. `grant.drop` at a flat zero is the alarm for a dead drop pipeline; if a client
    /// could bump it, the alarm reads healthy while the pipeline is dead.
    #[test]
    fn a_forged_server_counter_cannot_be_written() {
        let mut m = Metrics::default();
        let (ok, bad) = m.observe_client(1, &names(&[keys::GRANT_DROP, keys::SAVE_OK]));
        assert_eq!((ok, bad), (0, 2));
        let b = m.drain();
        assert_eq!(count(&b, keys::GRANT_DROP), 0);
        assert_eq!(count(&b, keys::SAVE_OK), 0);
        assert_eq!(count(&b, client::UNKNOWN), 2);
    }

    /// An unknown name is counted, and every unknown shares ONE key. Both halves matter: the
    /// count is what makes a renamed constant visible instead of silent, and the single key is
    /// what stops a client from minting keys — the number `pending()` says decides write cost.
    #[test]
    fn unknown_names_share_a_single_key() {
        let mut m = Metrics::default();
        m.observe_client(1, &names(&["a", "b", "c", "d"]));
        let b = m.drain();
        assert_eq!(b.counters.len(), 1, "one key for all unknowns, got {:?}", b.counters);
        assert_eq!(count(&b, client::UNKNOWN), 4);
    }

    /// Truncated, not refused — dropping the whole batch would lose the honest prefix, and an
    /// oversend is far more likely to be our own flush misbehaving than an attack.
    #[test]
    fn an_oversized_batch_is_truncated_and_keeps_its_prefix() {
        let mut m = Metrics::default();
        let over = vec!["screen.shop".to_string(); client::MAX_EVENTS + 25];
        let (ok, bad) = m.observe_client(1, &over);
        assert_eq!((ok, bad), (client::MAX_EVENTS, 0));
        assert_eq!(count(&m.drain(), "client.screen.shop"), client::MAX_EVENTS as i64);
    }

    /// A duplicate in the allow-list is a metric that reads as two and is one. Cheap to assert,
    /// and the list is edited by hand every time a metric is added.
    #[test]
    fn the_allow_list_has_no_duplicates_and_no_prefix() {
        let mut seen = std::collections::BTreeSet::new();
        for n in client::ALLOWED {
            assert!(seen.insert(*n), "duplicate in ALLOWED: {n}");
            assert!(!n.starts_with("client."),
                "{n}: the prefix is added by `client::key`, sending it doubles it");
            assert!(!n.is_empty(), "an empty name would collide with nothing and count as allowed");
        }
    }

    /// The counters are per DAY, so a batch on day 2 must not merge into day 1's row.
    #[test]
    fn days_do_not_merge() {
        let mut m = Metrics::default();
        m.observe_client(1, &names(&["fx.off"]));
        m.observe_client(2, &names(&["fx.off"]));
        let b = m.drain();
        assert_eq!(b.counters.len(), 2);
        assert!(b.counters.iter().all(|(_, _, v)| *v == 1));
    }
}
