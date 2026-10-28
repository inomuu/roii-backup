//! FARM BANNER — a limited character players can grind for (`docs/farm-banner-design.md`).
//!
//! Two halves, both paid from `accrue_after_save` against VERIFIED saves:
//! - **Accrual.** Online time is the verified sim ticks between accepted saves (a save cannot
//!   outrun wall-clock — `within_tick_rate` — and a closed game does not tick). Every
//!   `roll_ticks` of it is one roll at `box_ppm`; a hit mails a TRADEABLE box (`box_def`), at
//!   most `daily_cap` a day. Only inside the event window.
//! - **Tickets.** The sim only counts opens (`GameState.farm_opened[box_def]`); each open past
//!   the account's watermark credits one BOUND ticket for that box's event (user: the box
//!   trades, the ticket does not). Tickets never expire.
//! - **Pulls.** A ticket is spent on the event's banner (`/farm/roll`, ×1 or ×10): the featured
//!   character at `featured_ppm`, guaranteed on the `pity`-th pull without one (pity per account
//!   and event, apart from the store banners), otherwise a fixed payout: Gil, or a bound Sigil
//!   stack of a rolled tier and an equal-odds element (`TABLE`). No crystal: the store sells it.
//!
//! Events are server DATA, read from `FARM_EVENT` (one JSON object or an array) at boot. An
//! ended event STAYS in the list: its boxes still sell and its tickets still pull; only accrual
//! stops. Unset = no event, so a deploy never starts one by accident.

use serde::Deserialize;

/// One Farm Banner.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Event {
    /// Stable key for this event's progress rows.
    pub id: String,
    /// Window, unix milliseconds: boxes accrue only inside it.
    pub starts_ms: u64,
    pub ends_ms: u64,
    /// The character a pull can pay, as its item def (`exemplar_zolfia`).
    pub featured: String,
    /// The box this event mails — a catalog item with `farm_box` set, one per event (its Steam
    /// identity is per def). Its tickets pull only this event's banner.
    #[serde(default = "default_box_def")]
    pub box_def: String,
    /// Verified ticks of play per roll (36,000 = one hour).
    #[serde(default = "default_roll_ticks")]
    pub roll_ticks: u64,
    /// Chance a roll mails a box, parts per million.
    #[serde(default = "default_box_ppm")]
    pub box_ppm: u64,
    /// Boxes an account can earn in one day.
    #[serde(default = "default_daily_cap")]
    pub daily_cap: u32,
    /// Chance an open pays the featured character, parts per million.
    #[serde(default = "default_featured_ppm")]
    pub featured_ppm: u64,
    /// Opens without the character that guarantee it on the next.
    #[serde(default = "default_pity")]
    pub pity: u32,
}

fn default_box_def() -> String { "farm_box".into() }
fn default_roll_ticks() -> u64 { 36_000 }
fn default_box_ppm() -> u64 { 40_000 }
fn default_daily_cap() -> u32 { 2 }
fn default_featured_ppm() -> u64 { 10_000 }
fn default_pity() -> u32 { 150 }


impl Event {
    /// The events from the `FARM_EVENT` env var: one object or an array of them. `Err` on a
    /// set-but-bad value — malformed, an invalid entry, or two events sharing a `box_def` (a
    /// ticket must name one banner) — which the caller logs and runs NO event rather than a
    /// wrong one. Unset or empty = `Ok(vec![])`.
    pub fn list_from_env_str(raw: Option<&str>) -> Result<Vec<Self>, String> {
        let Some(raw) = raw.map(str::trim).filter(|r| !r.is_empty()) else { return Ok(Vec::new()) };
        let list: Vec<Event> = if raw.starts_with('[') {
            serde_json::from_str(raw).map_err(|e| e.to_string())?
        } else {
            vec![serde_json::from_str(raw).map_err(|e| e.to_string())?]
        };
        let mut defs = std::collections::BTreeSet::new();
        for e in &list {
            if e.roll_ticks == 0 || e.ends_ms <= e.starts_ms || e.featured.is_empty() || e.box_def.is_empty() || e.id.is_empty() {
                return Err(format!("event '{}' is invalid", e.id));
            }
            if !defs.insert(e.box_def.as_str()) {
                return Err(format!("box_def '{}' is on two events", e.box_def));
            }
        }
        Ok(list)
    }

    pub fn live_at(&self, now_ms: u64) -> bool {
        now_ms >= self.starts_ms && now_ms < self.ends_ms
    }
}

/// Where an account stands on an event's accrual, as stored in `farm_progress`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Progress {
    /// The last verified `final_tick` credited (the anchor).
    pub last_tick: u64,
    /// Credited ticks not yet spent on a roll.
    pub banked: u64,
    /// Rolls made so far (the nonce sequence).
    pub rolls: u64,
    /// UTC day number of `boxes_today`.
    pub day: i64,
    pub boxes_today: u32,
}

/// What one save's accrual did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accrued {
    pub progress: Progress,
    /// Boxes to mail now.
    pub boxes: u32,
}

/// Credit the ticks between the anchor and `final_tick`, roll every whole `roll_ticks`, and
/// count hits up to the day's cap. `nonce(seq)` is the server's keyed nonce for roll `seq`.
/// A first sight (anchor 0) or a tick that went BACKWARDS (a new game) only moves the anchor:
/// nothing is credited that the server did not see being played. Pure and total.
pub fn accrue(
    ev: &Event,
    mut p: Progress,
    final_tick: u64,
    day: i64,
    live: bool,
    nonce: impl Fn(u64) -> u64,
) -> Accrued {
    if p.day != day {
        p.day = day;
        p.boxes_today = 0;
    }
    let fresh = p.last_tick == 0 && p.rolls == 0 && p.banked == 0;
    let delta = if fresh || final_tick < p.last_tick { 0 } else { final_tick - p.last_tick };
    p.last_tick = final_tick;
    if !live {
        // Outside the window: keep the anchor current, bank nothing.
        p.banked = 0;
        return Accrued { progress: p, boxes: 0 };
    }
    p.banked = p.banked.saturating_add(delta);
    let mut boxes = 0;
    while p.banked >= ev.roll_ticks {
        p.banked -= ev.roll_ticks;
        p.rolls += 1;
        let hit = nonce(p.rolls) % 1_000_000 < ev.box_ppm;
        if hit && p.boxes_today < ev.daily_cap {
            p.boxes_today += 1;
            boxes += 1;
        }
    }
    Accrued { progress: p, boxes }
}

/// What one pull pays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The featured character.
    Featured,
    /// Gil (user: a box pays Gil OR Sigils, never both).
    Gil(u64),
    /// A BOUND Sigil stack: catalog id and count. Sigils have no element since 2026-09-30.
    Sigil { id: &'static str, count: u32 },
}

/// What an open that is not the character pays, ppm of the other 99% — fixed amounts, no stage
/// scaling (user: "fix cost"), no crystal (it is what the store sells). `(ppm, tier, count)`;
/// tier 0 = Gil.
const TABLE: [(u64, u8, u32); 4] = [
    (500_000, 0, 0),   // Gil
    (350_000, 1, 3),   // Sigil I ×3
    (120_000, 2, 1),   // Sigil II ×1
    (30_000, 3, 1),    // Sigil III ×1
];

/// Sigil ids by tier (the catalog's spelling; element-less since 2026-09-30).
const SIGILS: [&str; 3] = ["sigil_1", "sigil_2", "sigil_3"];

fn sigil(tier: u8) -> &'static str {
    SIGILS.get(usize::from(tier.clamp(1, 3) - 1)).copied().unwrap_or(SIGILS[0])
}

/// The Gil an open pays when it pays Gil.
pub const BUNDLE_GIL: u64 = 25_000;

/// Roll one pull. `since` = pulls since the last character (before this one). The `pity`-th
/// pull without one pays it for certain. Returns the outcome and the new `since`. Pure, total.
pub fn pull(ev: &Event, since: u32, nonce: u64) -> (Outcome, u32) {
    if since.saturating_add(1) >= ev.pity || nonce % 1_000_000 < ev.featured_ppm {
        return (Outcome::Featured, 0);
    }
    // An independent slice of the nonce: the band.
    let band = (nonce / 1_000_000) % 1_000_000;
    let mut acc = 0;
    let mut pick = (0u8, 0u32);
    for (ppm, tier, count) in TABLE {
        acc += ppm;
        if band < acc {
            pick = (tier, count);
            break;
        }
    }
    let out = match pick {
        (0, _) => Outcome::Gil(BUNDLE_GIL),
        (tier, count) => Outcome::Sigil { id: sigil(tier), count },
    };
    (out, since.saturating_add(1))
}

/// The bound-item kind that holds an account's tickets for a box.
pub fn ticket_kind(box_def: &str) -> String {
    format!("farm_ticket:{box_def}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev() -> Event {
        Event {
            id: "zolfia-1".into(),
            starts_ms: 1_000,
            ends_ms: 2_000,
            featured: "exemplar_zolfia".into(),
            box_def: "farm_box".into(),
            roll_ticks: 36_000,
            box_ppm: 40_000,
            daily_cap: 2,
            featured_ppm: 10_000,
            pity: 150,
        }
    }

    #[test]
    fn the_env_value_parses_with_defaults_and_a_bad_one_runs_no_event() {
        let one = Event::list_from_env_str(Some(r#"{"id":"z","starts_ms":1,"ends_ms":9,"featured":"exemplar_zolfia"}"#)).expect("parses");
        let e = &one[0];
        assert_eq!((e.roll_ticks, e.box_ppm, e.daily_cap, e.featured_ppm, e.pity), (36_000, 40_000, 2, 10_000, 150));
        assert_eq!(e.box_def, "farm_box");
        assert_eq!(Event::list_from_env_str(None), Ok(vec![]));
        assert_eq!(Event::list_from_env_str(Some(" ")), Ok(vec![]));
        assert!(Event::list_from_env_str(Some("{not json")).is_err());
        assert!(Event::list_from_env_str(Some(r#"{"id":"z","starts_ms":9,"ends_ms":1,"featured":"x"}"#)).is_err(), "an empty window");
        assert!(e.live_at(1) && e.live_at(8) && !e.live_at(9) && !e.live_at(0));
        let two = Event::list_from_env_str(Some(r#"[{"id":"a","starts_ms":1,"ends_ms":9,"featured":"x"},
            {"id":"b","starts_ms":9,"ends_ms":19,"featured":"y","box_def":"farm_box_2"}]"#)).expect("a list");
        assert_eq!(two.len(), 2);
        assert!(Event::list_from_env_str(Some(r#"[{"id":"a","starts_ms":1,"ends_ms":9,"featured":"x"},
            {"id":"b","starts_ms":9,"ends_ms":19,"featured":"y"}]"#)).is_err(), "one box, two banners");
    }

    #[test]
    fn only_ticks_the_server_saw_played_are_credited() {
        let e = ev();
        let always = |_| 0u64; // every roll hits
        // First sight: the anchor moves, nothing is credited.
        let a = accrue(&e, Progress::default(), 500_000, 1, true, always);
        assert_eq!((a.boxes, a.progress.last_tick, a.progress.banked), (0, 500_000, 0));
        // 1.5 hours later: one roll, half an hour banked.
        let b = accrue(&e, a.progress, 500_000 + 54_000, 1, true, always);
        assert_eq!((b.boxes, b.progress.rolls, b.progress.banked), (1, 1, 18_000));
        // A new game (ticks went backwards): anchor only.
        let c = accrue(&e, b.progress, 100, 1, true, always);
        assert_eq!((c.boxes, c.progress.last_tick, c.progress.banked), (0, 100, 18_000));
        // Outside the window: nothing banks, the anchor still moves.
        let d = accrue(&e, c.progress, 100 + 72_000, 1, false, always);
        assert_eq!((d.boxes, d.progress.banked, d.progress.last_tick), (0, 0, 72_100));
    }

    #[test]
    fn the_daily_cap_holds_and_a_new_day_reopens_it() {
        let e = ev();
        let always = |_| 0u64;
        let p = Progress { last_tick: 1, rolls: 1, ..Default::default() };
        let a = accrue(&e, p, 1 + 10 * 36_000, 7, true, always);
        assert_eq!((a.boxes, a.progress.boxes_today, a.progress.rolls), (2, 2, 11), "ten hits, the cap is two");
        let b = accrue(&e, a.progress, a.progress.last_tick + 36_000, 7, true, always);
        assert_eq!(b.boxes, 0, "still capped today");
        let c = accrue(&e, b.progress, b.progress.last_tick + 36_000, 8, true, always);
        assert_eq!(c.boxes, 1, "a new day");
    }

    #[test]
    fn the_rate_lands_near_one_box_a_day_for_a_24_hour_idler() {
        let e = ev();
        let n = 200_000u64; // rolls
        let hits = (1..=n).filter(|s| blake(*s) % 1_000_000 < e.box_ppm).count() as u64;
        let per_day = hits as f64 / n as f64 * 24.0;
        assert!((0.9..1.02).contains(&per_day), "{per_day} boxes/day at 24 h");
    }

    #[test]
    fn the_character_lands_at_its_rate_and_never_past_pity() {
        let e = ev();
        let mut since = 0u32;
        let mut wins = 0u32;
        let mut worst = 0u32;
        let n = 300_000u64;
        for s in 1..=n {
            let (o, next) = pull(&e, since, blake(s));
            if o == Outcome::Featured {
                wins += 1;
                worst = worst.max(since + 1);
            }
            since = next;
        }
        assert!(worst <= 150, "pity: {worst}");
        let per = n as f64 / f64::from(wins);
        assert!((70.0..86.0).contains(&per), "{per} opens per character (1% with pity 150 ≈ 78)");
    }

    #[test]
    fn a_pull_pays_gil_or_sigils_never_both_in_the_table_shares() {
        let no_char = Event { featured_ppm: 0, pity: u32::MAX, ..ev() };
        assert_eq!(TABLE.iter().map(|t| t.0).sum::<u64>(), 1_000_000);
        let (mut gil, mut t1, mut t2, mut t3) = (0u64, 0u64, 0u64, 0u64);
        let n = 200_000u64;
        for s in 1..=n {
            match pull(&no_char, 0, blake(s)).0 {
                Outcome::Gil(g) => { assert_eq!(g, BUNDLE_GIL); gil += 1; }
                Outcome::Sigil { id, count } => {
                    match id.chars().last() { Some('1') => { assert_eq!(count, 3); t1 += 1 } Some('2') => t2 += 1, _ => t3 += 1 }
                    assert!(SIGILS.contains(&id), "{id}");
                }
                Outcome::Featured => panic!("featured_ppm 0, no pity"),
            }
        }
        let share = |c: u64| c * 1_000 / n; // per mille
        assert!((490..510).contains(&share(gil)) && (340..360).contains(&share(t1))
            && (112..128).contains(&share(t2)) && (26..34).contains(&share(t3)), "{gil} {t1} {t2} {t3}");
        let data = sim_core::data::GameData::from_json_str(include_str!("../../../data/game_data.json")).expect("catalog");
        assert!(SIGILS.iter().all(|id| data.item(id).is_some()), "every Sigil id is a catalog item");
        assert!(!matches!(pull(&ev(), 0, 5_500_000).0, Outcome::Featured), "a nonce outside the 1%");
    }

    fn blake(s: u64) -> u64 {
        let h = blake3::hash(&s.to_le_bytes());
        u64::from_le_bytes(h.as_bytes()[..8].try_into().expect("8 bytes")) & (u64::MAX >> 1)
    }
}
