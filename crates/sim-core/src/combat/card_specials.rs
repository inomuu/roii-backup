//! CARD SPECIAL effects (0.0.212) — per-card mechanics beyond flat stats,
//! designed card-by-card with the user (gothic pack, 2026-07-19).
//!
//! Catalog lives HERE in code (like `items::exemplar_def`): a card def carries
//! only its id — magnitudes derive from (def_id, star), so data stays dumb and
//! replay-safe. Every read walks ACTIVE socketed cards via
//! `derived::for_each_active_card` (the same activation rule as stat bonuses),
//! so a benched/inactive card grants nothing.
//!
//! 0.0.243: the specials whose effect the `StatMod` vocabulary ALREADY expresses moved
//! OUT of this module into authored `card_mods` in game_data — DMG-vs-element
//! (Twinling/Pinkling → `DmgVsElement`) and CRIT DAMAGE (Gloomtooth/Scarlette/Chomb →
//! `CritDmgBp`). What stays here is only what NO family can express: a bonus keyed on the
//! ATTACK's own element, damage reduction on BASIC hits only, a bonus on ONE skill id, and
//! a flat add derived from the holder's STR.
//!
//! All folds return 0 for card-less entities → every pre-card scenario and
//! golden stays byte-identical. NO PRNG in this module — proc ROLLS happen at
//! the combat sites (state-keyed conditional draws, gated on the holder
//! actually carrying the card).

use super::derived::for_each_active_card;
use crate::grid::{Element, Entity};

/// Star-stepped magnitude: `base + step × star` (star capped at 5).
#[inline]
fn stepped(base: i32, step: i32, star: u8) -> i32 {
    base + step * star.min(crate::items::CARD_STAR_MAX) as i32
}

/// The holder's star for card `def_id` among ACTIVE sockets — None when the
/// card isn't socketed/active. Highest star wins if duplicates are socketed.
pub fn active_card_star(e: &Entity, def_id: &str) -> Option<u8> {
    let mut best: Option<u8> = None;
    for_each_active_card(e, &mut |c| {
        if c.def_id == def_id {
            best = Some(best.map_or(c.star, |b| b.max(c.star)));
        }
    });
    best
}

/// Attacker-side: bonus bp when the ATTACK's element is `atk_el`.
/// Broomhilda +10→20% and Burst Chom +10→20% for Fire-element attacks
/// (Weapon vs Necklace slots — a stacking fire-build pair).
pub fn dmg_with_atk_element_bp(e: &Entity, atk_el: Element) -> i32 {
    let mut bp = 0;
    if atk_el == Element::Fire {
        if let Some(s) = active_card_star(e, "card_broomhilda") {
            bp += stepped(1_000, 200, s);
        }
        if let Some(s) = active_card_star(e, "card_burst_chom") {
            bp += stepped(1_000, 200, s);
        }
    }
    bp
}

/// Defender-side: bp shaved off incoming BASIC hits (power-100% swings and
/// basic projectiles; skills pass through untouched).
/// Greenling — −2.5→−5% (kept small: basic hits come from every source).
pub fn basic_dmg_taken_reduction_bp(e: &Entity) -> i32 {
    match active_card_star(e, "card_greenling") {
        Some(s) => stepped(250, 50, s),
        None => 0,
    }
}

/// Healer-side: bonus bp on the HEAL skill's output (the skill id "heal"
/// ONLY — Dupla/Subvenio and other heal sources are untouched).
/// Rechum — +25→50%.
pub fn heal_skill_bonus_bp(e: &Entity) -> i32 {
    match active_card_star(e, "card_rechum") {
        Some(s) => stepped(2_500, 500, s),
        None => 0,
    }
}

/// Brutus — Bash gains +5→10% ATK of extra power. It scaled off the holder's
/// STR until 0.0.316; STR feeds nothing now, and ATK is the number a character's
/// design states, so the card rides that instead. Returns basis points of ATK to
/// ADD to Bash's power (0 without the card).
pub fn bash_atk_bonus_bp(e: &Entity) -> u32 {
    match active_card_star(e, "card_brutus") {
        Some(s) => stepped(500, 100, s) as u32,
        None => 0,
    }
}

/// DISPLAY values for a card's special at `star` — the numbers the album UI
/// substitutes into the localized description ({0}, {1}). Sourced HERE so the
/// text can never drift from the mechanics. Empty = no special (e.g. Zombie).
pub fn display_values(def_id: &str, star: u8) -> Vec<String> {
    let s = star.min(crate::items::CARD_STAR_MAX);
    match def_id {
        "card_marionette" | "card_broomhilda" | "card_burst_chom" => {
            vec![format!("{}", 10 + 2 * s as i32)]
        }
        "card_banshee" => vec![format!("{}", 5 + s as i32), format!("{}", 50 + 10 * s as i32)],
        "card_dustelle" | "card_crimsonne" | "card_gnarloak" | "card_yellowling"
        | "card_toxic_chom" => {
            vec![format!("{}", 5 + s as i32)]
        }
        "card_ashelle" | "card_greenling" => {
            let bp = if def_id == "card_greenling" { 250 + 50 * s as i32 } else { 200 + 40 * s as i32 };
            vec![if bp % 100 == 0 { format!("{}", bp / 100) } else { format!("{}.{}", bp / 100, (bp % 100) / 10) }]
        }
        "card_rechum" => vec![format!("{}", 25 + 5 * s as i32)],
        "card_brutus" => vec![format!("{}", 5 + s as i32)],
        _ => Vec::new(),
    }
}
