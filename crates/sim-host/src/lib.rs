//! sim-host — the renderer-agnostic half of every bridge (see Cargo.toml).
//!
//! - `view_state` / `item_view`: the ONE read contract the client draws from (moved verbatim from
//!   godot-ext; its key-set pin test lives with it).
//! - `session`: the lifecycle both bridges run — kept as free functions over `GameState` +
//!   `GameData` so `SimBridge` (which keeps its own fields) and `sim_ffi::Sim` (which wraps the
//!   `Session` struct below) execute the SAME code for start / load / tick / export / re-anchor.
#![forbid(unsafe_code)]

pub mod combat_skills;
pub mod item_view;
pub mod forge_receipt;
pub mod potential_view;
pub mod session;
pub mod view_state;

/// THE DISPLAY SCALE (0.0.401, user 2026-09-16: "หาร ÷10 ที่ UI"). The sim runs on a ×10 stat scale
/// so percentages survive integer maths (commit 003132eb); the player never needs to see it. Every
/// AMOUNT this host hands a client — HP, ATK, DEF, damage and heal numbers, flat stat lines — is
/// divided by this at the boundary, once, here. Percentages, levels, Gil, XP and counts are not
/// amounts and do not move. The sim and the save are untouched.
pub const DISPLAY_SCALE: i64 = 10;

/// An amount on the display scale, rounded half away from zero (so a 5-HP regen tick still shows
/// as 1, and a 4 as 0).
pub fn disp(v: i64) -> i64 {
    if v >= 0 { (v + DISPLAY_SCALE / 2) / DISPLAY_SCALE } else { -((-v + DISPLAY_SCALE / 2) / DISPLAY_SCALE) }
}

/// PLAYER-SIDE HP is shown on the SIM scale (user 2026-09-18: "ฝั่งผู้เล่นแสดง max hp เป็นค่าปกติ ×10 +
/// damage taken ×10 … ฝั่งศัตรูยังแสดงแบบเดิม"): a party member's HP, max HP, and every hit or heal
/// that lands ON a party member pass through unscaled, so the numbers a player reads off their own
/// bar are ten times a monster's. Monsters, ATK, DEF and gear lines stay on `disp`.
pub fn disp_for(player_side: bool, v: i64) -> i64 {
    if player_side { v } else { disp(v) }
}
