//! COMMANDER LEVEL — the account-meta CXP curve, as PURE functions.
//!
//! Source of truth for the design: `docs/commander-level-and-outpost.md`. Like `daily`/`schedule`,
//! nothing here knows what a database, a request, or a clock is — it is `(config, position) -> answer`
//! so the whole curve is testable without Postgres. The endpoints in `main.rs` are the only place that
//! touches the DB; they call these.
//!
//! Commander level UNLOCKS, never empowers — it lives entirely server-side, outside the sim save, so
//! nothing here feeds `GameState`, moves a golden, or needs cross-platform determinism. The integer
//! table is computed once at boot (mirroring `schedule`'s boot-built config).

use serde::Deserialize;

/// STARTING curve — TUNABLE. `cost(L->L+1) = BASE * RATIO^(L-1)`, integer basis-point compounding
/// (×1.03), calibrated in the doc to ~2.5 months to L50 at ~1000 CXP/day. The user sets these a
/// touch HIGHER than current income demands, as HEADROOM for XP sources not yet built (events,
/// milestones) — adjust the two constants, not the shape. See the doc's § The curve.
const LEVEL_BASE_CXP: i64 = 700;
/// Per-level growth in basis points (10000 = ×1.0). 10300 = ×1.03.
const LEVEL_RATIO_BP: i64 = 10_300;

/// PROGRESS CXP (user 2026-09-30, Growth packs at Commander 10/15/20/25/30): the curve was
/// calibrated to ~1,000 CXP/day with headroom for sources not yet built. Each daily-ladder rung claimed pays CXP alongside its crystal (`/daily/claim`; five rungs =
/// 1,000 a day, all five without finishing the whole board — it absorbed the Outpost's 500 when the
/// Outpost was removed), and each stage reached for the first time pays on the save that reached it —
/// both server-side, never taken from the client's word.
pub const CXP_PER_DAILY_RUNG: i64 = 200;
pub const CXP_PER_NEW_STAGE: i64 = 50;

/// Launch cap (user: 50 at launch, +10 per major update, no permanent cap). To raise the cap, bump
/// this and — past 50 — switch the compounding to the post-50 tier the doc specifies
/// (`30_000 * 1.035^(L-51)`, a deliberate ~10× step at 50→51 that seams launch to the endgame tail).
/// That tier is intentionally NOT built until the +10 update ships and 51+ is reachable.
pub const MAX_LEVEL: u32 = 50;


// The per-level reward is DATA in a SHARED config file (user, 2026-08-30): `data/commander_rewards.json`
// is loaded once at boot by BOTH the server (for the authoritative Crystal CREDIT) and the client (a
// byte-identical copy under `godot/data/`, for the DISPLAY track). Static config, so it is not a DB
// table and never crosses the wire per state fetch — /commander/state sends only the account's dynamic
// position (level + reward_claimed_through) and the client renders the track from its local copy. The
// client copy under godot/data/ is a gitignored build artifact (`cp` of the source, like
// game_data.json); `tools/check_online_sync.sh` byte-compares the two so an edit that never reached the
// copy is caught before an online playtest, and `parse_rewards` rejects at boot any file whose skill
// column disagrees with `skill_unlocked_at` (the signed grant authority). Crystal = BOUND crystal, credited
// in the claim tx (the NIKKE level-up hook — no
// mailbox, no sim event). ⚠️ PLACEHOLDER faucet — see docs/commander-level-and-outpost.md § reward.

/// One level's reward row, mirrored on the client. `skill` is the sim skill id the level unlocks (for
/// the ★ chip); the authoritative grant still rides `skill_unlocked_at` + a signed nonce, and
/// `parse_rewards` asserts the two agree so display never diverges from what is actually granted.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct RewardRow {
    pub level: u32,
    pub crystal: i64,
    #[serde(default)]
    pub skill: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RewardFile {
    max_level: u32,
    rewards: Vec<RewardRow>,
}

/// Parse the shared reward config. Enforces that it covers exactly levels `2..=MAX_LEVEL` in order,
/// that `max_level` matches the code's cap, and that each `skill` matches `skill_unlocked_at` — so a
/// mis-edited file is a boot panic, not a silent display/grant mismatch. Returns the rows on success.
pub fn parse_rewards(json: &str) -> Result<Vec<RewardRow>, String> {
    let f: RewardFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if f.max_level != MAX_LEVEL {
        return Err(format!("reward file max_level {} != MAX_LEVEL {MAX_LEVEL}", f.max_level));
    }
    let got: Vec<u32> = f.rewards.iter().map(|r| r.level).collect();
    // Level 1 has a reward too (user 2026-08-30: a FREE welcome, claimable at 0 CXP), so the track
    // covers 1..=MAX_LEVEL.
    let expected: Vec<u32> = (1..=MAX_LEVEL).collect();
    if got != expected {
        return Err(format!("reward levels must be exactly 1..={MAX_LEVEL} in order, got {got:?}"));
    }
    for r in &f.rewards {
        // Crystal is credited as BOUND premium currency: a NEGATIVE value would be a silent debit and
        // an absurd one (a fat-fingered extra zero) mints a windfall. Bound it so a mis-edit is a boot
        // panic, not a live economy bug. The cap is generous vs the placeholder shape (300/1000).
        if !(0..=1_000_000).contains(&r.crystal) {
            return Err(format!(
                "reward level {} crystal {} out of range [0, 1_000_000] — a negative value is a debit, an absurd one a typo",
                r.level, r.crystal
            ));
        }
        if r.skill.as_deref() != skill_unlocked_at(r.level) {
            return Err(format!(
                "reward level {} skill {:?} disagrees with skill_unlocked_at {:?}",
                r.level, r.skill, skill_unlocked_at(r.level)
            ));
        }
    }
    Ok(f.rewards)
}

/// Total Crystal for claiming the reward track over `(from_level, to_level]` — the authoritative sum the
/// claim credits, read from the loaded config so it matches exactly what the client rendered.
pub fn crystal_between(rewards: &[RewardRow], from_level: u32, to_level: u32) -> i64 {
    rewards.iter().filter(|r| r.level > from_level && r.level <= to_level).map(|r| r.crystal).sum()
}

/// The skill a given level UNLOCKS on claim, if any. Only smite@Lv2 for now (doc: smite is the first
/// unlock). Returns the sim skill id the claim should issue a `GrantCommanderSkill` for.
pub fn skill_unlocked_at(level: u32) -> Option<&'static str> {
    match level {
        2 => Some("commander_smite"),
        _ => None,
    }
}

/// Every skill an account at `level` should OWN — one per unlock level at or below `level`. Used to
/// RE-OFFER skill grants idempotently: the crystal reward is exactly-once (a high-water), but a skill
/// unlock rides an echoed `GrantCommanderSkill` that only lands once a save is verified, so if that
/// echo is lost the grant must be re-issued rather than stranded behind the crystal high-water. Insert is
/// idempotent both in the sim and the verifier, so re-offering a skill already owned is harmless.
pub fn skills_unlocked_up_to(level: u32) -> Vec<&'static str> {
    (2..=level).filter_map(skill_unlocked_at).collect()
}

/// Cumulative CXP required to REACH each level. `table[L]` = total CXP for level `L`; `table[1] = 0`
/// (a fresh account is level 1 at 0 CXP). Length `MAX_LEVEL + 1`; index 0 is unused padding.
pub fn build_level_table() -> Vec<i64> {
    // Built by push, never indexed, so it stays inside verify-server's `indexing_slicing` deny.
    // Index 0 is unused padding; index 1 (level 1) is 0 CXP; index L is the cumulative to reach L.
    let mut table = Vec::with_capacity((MAX_LEVEL as usize) + 1);
    table.push(0); // [0] padding
    table.push(0); // [1] level 1 = 0 CXP
    let mut cost = LEVEL_BASE_CXP; // cost of the L -> L+1 step, starting at L1
    let mut cumulative = 0i64;
    for _level in 1..MAX_LEVEL {
        cumulative += cost;
        table.push(cumulative); // [L+1]
        cost = cost * LEVEL_RATIO_BP / 10_000; // grow for the next step
    }
    table
}

/// Level for a given total CXP, clamped to `MAX_LEVEL`. Highest `L` with `table[L] <= cxp`.
pub fn level_for_cxp(table: &[i64], cxp: i64) -> u32 {
    let mut level = 1u32;
    for l in 1..=MAX_LEVEL {
        match table.get(l as usize) {
            Some(&threshold) if threshold <= cxp => level = l,
            _ => break, // beyond the table or past the threshold — this level is not reached
        }
    }
    level
}

/// Progress readout for the UI: `(level, cxp_into_level, cxp_to_next)`. At the cap, `cxp_to_next` is
/// 0 (nothing more to earn until the cap raises).
pub fn progress(table: &[i64], cxp: i64) -> (u32, i64, i64) {
    let level = level_for_cxp(table, cxp);
    let cur = table.get(level as usize).copied().unwrap_or(0);
    let into = cxp - cur;
    let to_next = if level >= MAX_LEVEL {
        0
    } else {
        table.get((level + 1) as usize).copied().unwrap_or(cur) - cur
    };
    (level, into, to_next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_monotonic_and_starts_at_zero() {
        let t = build_level_table();
        assert_eq!(t.len(), (MAX_LEVEL as usize) + 1);
        assert_eq!(t[1], 0, "a fresh account is level 1 at 0 CXP");
        for l in 2..=MAX_LEVEL as usize {
            assert!(t[l] > t[l - 1], "level {l} threshold must exceed {}", l - 1);
        }
    }

    #[test]
    fn level_for_cxp_maps_and_clamps() {
        let t = build_level_table();
        assert_eq!(level_for_cxp(&t, 0), 1);
        assert_eq!(level_for_cxp(&t, t[2] - 1), 1, "just below the L2 threshold");
        assert_eq!(level_for_cxp(&t, t[2]), 2, "exactly the L2 threshold");
        assert_eq!(level_for_cxp(&t, i64::MAX), MAX_LEVEL, "clamps at the cap");
    }

    #[test]
    fn smite_unlocks_at_level_2_and_nowhere_else() {
        assert_eq!(skill_unlocked_at(2), Some("commander_smite"));
        assert_eq!(skill_unlocked_at(1), None);
        assert_eq!(skill_unlocked_at(3), None);
    }

    #[test]
    fn pacing_reaches_l50_near_the_design_target() {
        // Doc: ~75,000 CXP cumulative to L50 (~2.5 months at ~1000 CXP/day). Integer truncation moves
        // it a little; pin the ballpark so a mistuned constant (or an accidental exponent) is caught.
        let t = build_level_table();
        let to_50 = t[MAX_LEVEL as usize];
        assert!(
            (70_000..=82_000).contains(&to_50),
            "cumulative to L50 = {to_50}, expected ~75k (doc curve)",
        );
        // L2 arrives fast (smite ~day 1): its threshold is the base cost.
        assert_eq!(t[2], LEVEL_BASE_CXP);
    }

    #[test]
    fn skills_unlocked_up_to_lists_every_owned_skill() {
        assert!(skills_unlocked_up_to(1).is_empty(), "level 1 owns nothing");
        assert_eq!(skills_unlocked_up_to(2), vec!["commander_smite"]);
        assert_eq!(skills_unlocked_up_to(30), vec!["commander_smite"], "still just smite past L2");
    }

    // Only the SOURCE file (tracked in git) is read here. The client copy lives under `godot/data/`,
    // which is gitignored (a build artifact, like game_data.json) so it is NOT present in a fresh
    // checkout — the source↔copy drift guard therefore lives in `tools/check_online_sync.sh`, where both
    // files are guaranteed to exist, NOT in a cargo test.
    const SERVER_REWARDS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/commander_rewards.json");

    #[test]
    fn shipped_reward_file_parses_and_sums_correctly() {
        let json = std::fs::read_to_string(SERVER_REWARDS).expect("read shipped reward file");
        let rows = parse_rewards(&json).expect("shipped reward file must parse");
        assert_eq!(rows.len(), MAX_LEVEL as usize, "one row per level 1..=MAX_LEVEL");
        assert_eq!(crystal_between(&rows, 0, 1), 300, "a fresh account claims the FREE level-1 reward");
        assert_eq!(crystal_between(&rows, 1, 2), 300, "claiming L1->L2 credits the L2 row");
        assert_eq!(crystal_between(&rows, 2, 2), 0, "an already-claimed level credits nothing");
        assert!(crystal_between(&rows, 0, MAX_LEVEL) > 0, "the whole track has a positive total");
    }

    #[test]
    fn parse_rewards_rejects_incomplete_coverage() {
        let bad = r#"{"max_level":50,"rewards":[{"level":2,"crystal":300,"skill":"commander_smite"}]}"#;
        assert!(parse_rewards(bad).is_err(), "a file missing levels must be rejected");
    }

    #[test]
    fn parse_rewards_rejects_out_of_range_crystal() {
        // A fully-covered file, correct skills, but one level carries a negative Crystal (a debit) — must
        // be rejected at boot, not silently credited later.
        let mut s = String::from("{\"max_level\":50,\"rewards\":[");
        for lv in 1..=MAX_LEVEL {
            if lv > 1 {
                s.push(',');
            }
            let crystal = if lv == 5 { -300 } else { 300 };
            if lv == 2 {
                s.push_str(&format!("{{\"level\":{lv},\"crystal\":{crystal},\"skill\":\"commander_smite\"}}"));
            } else {
                s.push_str(&format!("{{\"level\":{lv},\"crystal\":{crystal}}}"));
            }
        }
        s.push_str("]}");
        assert!(parse_rewards(&s).is_err(), "a negative Crystal reward must be rejected");
    }

    #[test]
    fn parse_rewards_rejects_skill_disagreement() {
        // A fully-covered file, but smite sits at the wrong level — must be rejected so the ★ display can
        // never diverge from the signed grant (skill_unlocked_at is the authority).
        let mut s = String::from("{\"max_level\":50,\"rewards\":[");
        for lv in 1..=MAX_LEVEL {
            if lv > 1 {
                s.push(',');
            }
            if lv == 3 {
                s.push_str(&format!("{{\"level\":{lv},\"crystal\":300,\"skill\":\"commander_smite\"}}"));
            } else {
                s.push_str(&format!("{{\"level\":{lv},\"crystal\":300}}"));
            }
        }
        s.push_str("]}");
        assert!(parse_rewards(&s).is_err(), "smite at the wrong level must be rejected");
    }
}

/// The `band_reached` a stored save's JSON carries (0 when absent or unreadable) — the Commander
/// stage watermark an existing account starts from, so a deploy pays no back-dated stage CXP.
pub fn seed_stage_of(state_json: &str) -> i32 {
    serde_json::from_str::<serde_json::Value>(state_json)
        .ok()
        .and_then(|v| v.get("band_reached").and_then(|b| b.as_i64()))
        .map_or(0, |b| i32::try_from(b.max(0)).unwrap_or(i32::MAX))
}

#[cfg(test)]
mod seed_tests {
    #[test]
    fn the_seed_reads_band_reached_and_defaults_to_zero() {
        assert_eq!(super::seed_stage_of(r#"{"tick":5,"band_reached":31}"#), 31);
        assert_eq!(super::seed_stage_of(r#"{"tick":5}"#), 0, "an untouched field serializes absent");
        assert_eq!(super::seed_stage_of("not json"), 0);
    }
}
