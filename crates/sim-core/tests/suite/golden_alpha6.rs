//! Golden test for ALPHA-6 — data-driven new-game end to end.
//!
//! Re-pinned in 0.0.7-alpha. Scenario now runs through boss-gate
//! progression: player clears grassland (30 kills + slime_king),
//! auto-advances to dungeon, and (without UI to equip drops) usually
//! falls to the dungeon mob density. The test PRINTS its observations
//! and pins the final hash — we deliberately do NOT require player to
//! survive, because that's a UI-driven concern not a sim one.

use sim_core::data::{self, GameData, FIRST_PLAYER_ID};
use sim_core::grid::Team;
use sim_core::hash::state_hash;
use sim_core::state::GameState;
use sim_core::tick::tick;

// Re-bumped in BETA-4 (sim-core 0.0.10-alpha): Entity, MonsterDef and
// UnitDef gained `proj_travel_ticks` (+ `atk_range` on Monster).
// Defaults are 0 / 1 (= melee with 1-cell range) so this scenario's
// behaviour matches ALPHA-6 exactly — JSON shape only.
//
// Re-bumped in BETA-5 (sim-core 0.0.11-alpha): Stats schema + XP
// rewards on monsters. Both change behavior:
//   - STAT_MIN baseline shifts damage by ±1.
//   - Monster kills now award XP → player levels up → mid-run stat
//     allocation pool grows (though no auto-spend, so combat math
//     stays similar). The hash captures the new "with XP earned"
//     end state of the scenario.
// Re-bumped in 0.0.12 (per-class roster). This scenario DOES run
// new_game_in_zone, so `current_unit_def` is now `Some("test_class")`.
// Behavior change in the SCHEMA (extra field) but not in any tick math.
// Re-bumped in 0.0.14 (team-aware stacking — Monsters stack on each
// other; Players still block). Only this scenario drifts because it
// has multiple monsters that previously queued behind each other on
// the shortest path; now they overlap and reach the player faster.
// Re-bumped in 0.0.14 (surround targeting). Multiple monsters in
// this scenario spread around the player instead of all aiming at
// the same tile — both `target` fields and downstream tick paths
// shift accordingly.
// 0.0.14: +Entity.atk_type + is_ranged (Damage/magic refactor).
// Shape-only field add (defaults Physical/false) — behavior identical.
// 0.0.15: +Stats job_level/job_xp/skill_points + Entity/MonsterTemplate
// job_xp_reward (RO Job Level). Shape add; gear-less/job_xp=0 scenarios
// behave identically.
// 0.0.16: +Entity sp + skills (Bash active, HP Recovery passive).
// Shape add; skill-less/SP-less scenarios behave identically (alpha6
// now auto-casts Bash + HP-regens, so its run legitimately changes).
// 0.0.33: iRO Renewal ASPD model (FJK formula + real ASPD-table job
// bases & weapon modifiers). This is the one golden scenario that builds
// its player from class data (new_game_in_zone), so it takes the new
// ASPD path: the swordsman's starting_sword now yields JB 149 (156−7) →
// ASPD ~150, a small swing-timing shift vs the prior model. A LEGITIMATE
// behavior change — the player still fights through grassland (11 kills
// before falling at tick 207); per this test's contract survival is not
// required, only the hash is pinned.
// 0.0.33b: swordsman gains Sword Mastery + 2H Sword Mastery (passive +4
// ATK/level each). Legitimate buff — the lead swordsman now hits harder
// (20 kills before falling at tick 282, up from 11 @ 207). Re-pinned.
// 0.0.34: +Entity {endure_soft_def, taunted_by, taunt_ticks} + Item.weapon_type
// (Endure/Taunt + weapon-mastery scaffolding). Every scenario here is
// skill-less / weapon_type None, so behavior is identical — pure JSON shape.
// 0.0.35: skill tree — skills are no longer innate. The class-built
// swordsman now starts with an EMPTY skill list (must learn bash/etc. from
// the tree by spending points), so it fights without Bash/HP-Recovery:
// 7 kills, falls at tick 160 (was 11 @ 207). Legitimate behavior change.
// 0.0.41: drop-rate tuning — `scrap` removed; tables are now weapon/armor +
// a weighted `nothing` no-drop band (~5% gear/kill). This run's escrow rolls
// shift accordingly, so the end-state hash drifts. Re-blessed.
// 0.0.42: +Item.equip_slot + game_data now drops the full RO 10-slot gear set
// (weapon/armor/shield/garment/shoes/3 headgear/2 accessory per band, each
// ~0.5%% + nothing 190 = ~5%% gear/kill). Drop tables + item shape changed →
// escrow + hash drift. Re-blessed.
// 0.0.43: job curve now QUADRATIC (JOB_EXP_BASE=320 * job_level^2) so job
// level tracks ~0.7x base instead of racing ahead. Kills here earn job XP →
// job_level (in the hash) shifts. Re-blessed.
// 0.0.45: weapon system — game_data now has 6 weapon types per band (dagger/
// sword/greatsword/bow/rod/mace) + the full 9-piece armor set that finally
// landed; drop tables + Item.equip_classes shape changed → escrow + hash drift.
// Re-blessed.
// 0.0.46: gear coarsened from 6 per-band tiers to 3 ten-level SETS (s1/s2/s3
// for Lv1-10/11-20/21-30); drop tables remap band->set. Content drift. Re-blessed.
// 0.0.47: direct-grant — drops bank to the stash, not the escrow. This run's
// loot now lands in state.inventory (was state.escrow) → state hash shifts. Re-blessed.
// 0.0.50: Soul income now scales with band (mob = band, boss = band*3) — this
// band-1 run kills the band-1 boss, so its soul total shifts. Re-blessed.
// 0.0.51: gear drop rate cut ~90% (nothing/junk weights x10) + dropped-gear
// sell_value x3. roll_drop's gen_range_u32(0, total_weight) now spans a larger
// range, so the drop PRNG stream + this run's loot diverge (stash drops to 1).
// Behavior intact (player alive, 28 kills); content drift. Re-blessed.
// 0.0.52: mobs now drop the tier-1 refine ore (weight 1, ~0.025%/kill). The mob
// drop tables grew an entry → roll_drop's range widens again → drop stream
// diverges. Behavior intact; content drift. Re-blessed.
// 0.0.54: that ore renamed ore_prima → ore_valkirion — the def_id string in the
// mob tables / any banked drop changed, shifting the hash. Pure rename. Re-blessed.
// 0.0.55: Primocon rarity Common → Uncommon. The rarity enum in the mob drop
// tables serializes differently → hash shifts. Content tweak. Re-blessed.
// 0.0.56: starter slime cards (Redling/Greenling/Yellowling) — Greenling drops
// from the band-1 Earth slime, so mob_b1_a's drop table grew an entry → the drop
// stream shifts. Content add; behavior intact. Re-blessed.
// 0.0.57: card drop rate cut to ~0.001% (non-card weights on the 3 card slimes
// scaled x25 so the weight-1 card is ~1/100,000). gen_range range widens →
// mob_b1_a's drop stream shifts; gear/junk rates unchanged. Re-blessed.
// 0.0.58: card rate further cut to ~0.0001% (1/~1,000,000) — non-card weights
// scaled another x5 (x125 total) so field + Steam-loop COMBINED ≈ 2-3 days/card.
// Drop stream widens again; gear/junk rates unchanged. Re-blessed.
// 0.0.243: CARD STAT UNIFY — cards grant `StatMod` lines (`card_mods` + the authored star
// ladder) instead of `CardEffect`. This run never sockets a card, but band-1 slimes carry card
// rows in their drop tables and a `DropEntry.template` is a FULL `Item` living in
// `ZoneSpawnState` — so the card prototypes hash here directly. Shape + re-authored values →
// re-blessed. Bisect confirmed: removing `card_effect` from the defs alone moves it identically.
// 0.0.245: BOUND cards join the same pipeline — `bind_effect` → `bind_mods` reshapes the same
// drop-table card prototypes. Values unchanged (every authored bind was flat and maps 1:1).
const ALPHA6_EXPECTED_HASH: &str =
    "d2602ac94bd4f435d855a5e640997be3af660d68337c48320266a434e51e34d4";

const GAME_DATA: &str = include_str!("../../../../data/game_data.json");

#[test]
fn alpha6_grassland_run() {
    let data = GameData::from_json_str(GAME_DATA).unwrap();
    let mut state: GameState = data::new_game_in_zone(42, &data, "zone_b1_1").unwrap();

    let player_idx = state.grid.idx_of_id(FIRST_PLAYER_ID).expect("player must exist");
    assert!(state.grid.entities[player_idx].alive);

    // Track headline events so we can compare against the Godot
    // window run (which sees the same physics — they must agree).
    let mut grassland_boss_spawn_tick: Option<u64> = None;
    let mut grassland_clear_tick: Option<u64> = None;
    let mut dungeon_enter_tick: Option<u64> = None;
    let mut player_death_tick: Option<u64> = None;
    let mut current_zone = "zone_b1_1".to_string();

    for _ in 0..600 {
        tick(&mut state, 1);
        // Event captures — must come BEFORE any zone change so the
        // state we read reflects the just-finished tick on the current zone.
        if grassland_boss_spawn_tick.is_none()
            && current_zone == "zone_b1_1"
        {
            if let Some(z) = state.zone_spawn.as_ref() {
                if z.boss_spawned {
                    grassland_boss_spawn_tick = Some(state.tick);
                }
            }
        }
        if grassland_clear_tick.is_none()
            && current_zone == "zone_b1_1"
        {
            if let Some(z) = state.zone_spawn.as_ref() {
                if z.zone_cleared {
                    grassland_clear_tick = Some(state.tick);
                }
            }
        }
        if player_death_tick.is_none()
            && !state.grid.entities[state.grid.idx_of_id(FIRST_PLAYER_ID).unwrap()].alive
        {
            player_death_tick = Some(state.tick);
        }
        if let Some(next_id) = state.pending_zone_change.clone() {
            data::change_zone(&mut state, &data, &next_id).unwrap();
            current_zone = next_id;
            if dungeon_enter_tick.is_none() && current_zone == "zone_dungeon" {
                dungeon_enter_tick = Some(state.tick);
            }
        }
    }

    // Gather stats.
    let pidx = state.grid.idx_of_id(FIRST_PLAYER_ID).expect("player still in vec");
    let player_alive = state.grid.entities[pidx].alive;
    let player_hp = state.grid.entities[pidx].hp;

    let dead_mons = state.grid.entities.iter()
        .filter(|e| matches!(e.team, Team::Monster) && !e.alive).count();
    let alive_mons = state.grid.entities.iter()
        .filter(|e| matches!(e.team, Team::Monster) && e.alive).count();
    let kill_counter = state.zone_spawn.as_ref().map(|z| z.kill_counter).unwrap_or(0);
    let boss_spawned = state.zone_spawn.as_ref().map(|z| z.boss_spawned).unwrap_or(false);
    let zone_cleared = state.zone_spawn.as_ref().map(|z| z.zone_cleared).unwrap_or(false);

    eprintln!(
        "ALPHA-6 after 600 ticks:\n\
         \tplayer_alive={player_alive}  hp={player_hp}/200  player_death_tick={:?}\n\
         \tdead_mons={dead_mons}  alive_mons={alive_mons}\n\
         \tcurrent_zone={current_zone}  kill_counter={kill_counter}  boss_spawned={boss_spawned}  zone_cleared={zone_cleared}\n\
         \tstash={}\n\
         \tgrassland_boss_spawn_tick={:?}\n\
         \tgrassland_clear_tick={:?}\n\
         \tdungeon_enter_tick={:?}",
        player_death_tick,
        state.inventory.len(),
        grassland_boss_spawn_tick,
        grassland_clear_tick,
        dungeon_enter_tick,
    );

    let h = state_hash(&state);
    eprintln!("ALPHA-6 state_hash = {h}");
    assert_eq!(
        h, ALPHA6_EXPECTED_HASH,
        "ALPHA-6 golden hash drifted — see GDD §7.2"
    );
}
