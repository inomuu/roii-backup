// Stat dump — read-only measurement of the CURRENT stat model.
//
// Emits, per unit and per base level, the effective combat stats a character
// carries with NO gear. Growth is the sim's own now — a level-up spends its
// points on the unit's growth order — so this measures the game, not a model
// of a player. Feeds the move to the new
// flat stat model (`docs/skill-system-redesign.md` §4.5.1.2): the new base
// values and the shared level curve are FITTED to these numbers so the swap
// lands on the balance the game already has, instead of on guessed values.
//
// Changes nothing about how the sim behaves — it only reads.

use sim_core::combat;
use sim_core::data::{self, GameData};
use sim_core::grid::Team;
use sim_core::state::GameState;

/// Units to measure — every one that ships.
const UNITS: &[&str] = &["seraphine", "lapis", "lucielle", "aveline", "raplus", "nemu", "lumi", "airi", "charon", "suzu", "mirei", "astier", "sylvie", "zolfia"];

pub(crate) fn dump(data: &GameData, zone: &str, seed: u64, max_level: u32) -> Result<(), String> {
    println!(
        "unit,level,max_hp,atk,hard_def,soft_def,dodge_bp,\
aspd_milli,crit_bp,crit_dmg_bp"
    );
    for unit_def in UNITS {
        let mut state = data::new_game_as(seed, data, zone, unit_def)?;
        let Some(id) = lead_id(&state) else {
            return Err("no player entity".to_string());
        };
        for level in 1..=max_level {
            let cur = entity_level(&state, id);
            if level > cur {
                data::debug_add_levels(&mut state, id, level - cur);
            }
            let Some(e) = state.grid.entities.iter().find(|e| e.id == id) else {
                return Err("lead vanished".to_string());
            };
            println!(
                "{unit_def},{level},{},{},{},{},{},{},{},{}",
                combat::effective_max_hp(e),
                combat::effective_atk(e),
                combat::effective_hard_def(e),
                combat::effective_soft_def(e),
                sim_core::combat::stat_mods::dodge_bp(e),
                combat::effective_atk_speed_milli(e),
                combat::effective_crit_chance_bp(e),
                combat::effective_crit_dmg_bp(e),
            );
        }
    }
    Ok(())
}

fn lead_id(state: &GameState) -> Option<u32> {
    state
        .grid
        .entities
        .iter()
        .find(|e| matches!(e.team, Team::Player) && !e.is_pet)
        .map(|e| e.id)
}

fn entity_level(state: &GameState, id: u32) -> u32 {
    state
        .grid
        .entities
        .iter()
        .find(|e| e.id == id)
        .map_or(1, |e| e.stats.level)
}
