//! DIAGNOSTIC (cross-platform probe): NO golden reaches BAND 4, so a determinism divergence in the
//! band-4 combat path (yellowling / toxic_chom passives + on-death blasts + the king_yellowling
//! boss) would slip past the x86 golden guard AND reject a live save (client debug-arm64 vs server
//! release-x86). This drives real band-4 combat: a tanky custom killer (raw stats persist, like
//! diag_acolyte) clears band-4 spawns for a long run — monsters spawn, use their skills, DIE
//! (on-death blast), and the boss triggers at the kill threshold. Run in BOTH profiles + compare;
//! the Docker build runs it on x86 (release) so a divergence fails the deploy instead of the live
//! char's saves.
//!   cargo test -p sim-core --test diag_band4 -- --nocapture
//!   cargo test -p sim-core --test diag_band4 --release -- --nocapture

use sim_core::data;
use sim_core::grid::{Cell, Element, Entity, Team};
use sim_core::hash::state_hash;
use sim_core::replay::advance;

fn game_data() -> data::GameData {
    data::GameData::from_json_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/game_data.json"
    )))
    .expect("game_data.json must parse")
}

#[test]
fn diag_band4_hash() {
    let d = game_data();
    // Band-4 zone: sets zone_spawn to the yellowling/toxic_chom pool (+ king_yellowling boss) and
    // spawns the default party.
    let mut state = data::new_game_in_zone(42, &d, "zone_b4_1").unwrap();

    // A tanky custom KILLER on the player team — raw combat stats (not a class-char) persist across
    // ticks, so it survives the whole run and one-shots band-4 spawns → many deaths (on-death
    // blasts) + enough kills to trigger the boss. Placed on the player spawn.
    let mut killer = Entity::idle(90_001, Cell::new(2, 7))
        .with_team(Team::Player)
        .with_walk(Cell::new(2, 7), 200)
        .with_combat(/*hp*/ 5_000_000, /*atk*/ 5_000, /*def*/ 500, /*atk_speed_milli*/ 300, Element::Light);
    killer.atk_range = 3;
    state.grid.spawn(killer);

    // Long run: band-4 monsters spawn (interval 16, population 4), get cleared, the boss triggers at
    // the kill threshold (19) and fights too. Exercises the full band-4 combat path deterministically.
    advance(&mut state, &d, 1500);

    let h = state_hash(&state);
    let monsters = state.grid.entities.iter().filter(|e| matches!(e.team, Team::Monster)).count();
    let killer_alive = state.grid.entities.iter().any(|e| e.id == 90_001 && e.alive);
    eprintln!("DIAG_BAND4 state_hash = {h}");
    eprintln!("  entities={} monsters_on_board={} killer_alive={}", state.grid.entities.len(), monsters, killer_alive);
    assert!(killer_alive, "the tanky killer must survive so band-4 combat actually runs the whole time");
    assert!(state.grid.entities.len() > 1, "band-4 monsters must have spawned + been fought");

    // PINNED on arm64 (dev). The Docker build runs this on x86 (release) — if the band-4 combat path
    // is cross-platform-divergent, THIS assert fails on x86 and the deploy aborts. If it holds on
    // x86, band-4 combat is cross-platform-clean → a live band-4 save reject was NOT this path.
    // Re-pinned 0.0.212 (gothic cards, game_data 0.0.46): zombie + demon_tree
    // (band 5 — this scenario spills past the band-4 boss) gained card
    // `server_bp` rows = one extra gated candidate draw per kill.
    // (0.0.47 data: every card row unified at 1/200k — candidate-gate thresholds
    // moved, so the same draws accept/reject differently.)
    // Re-pinned 0.0.227: GameState.measures (kills/rooms counters) now hash in
    // every combat scenario — intentional new-field drift, zero logic change.
    // Re-pinned 0.0.245 (bound cards join the same pipeline): `bind_effect` → `bind_mods`
    // reshapes the very same card prototypes described below. Numbers unchanged — every
    // authored bind was FLAT and maps 1:1 onto the flat StatMod family it already folded into.
    // Re-pinned 0.0.243 (card stat unify): a monster's `DropEntry.template` is a FULL `Item`,
    // and those templates live in `ZoneSpawnState` — so every card row in a drop table puts a
    // card prototype straight into the hash, whether or not one ever drops. Cards swapped
    // `card_effect` for `card_mods`/`card_star_step`/`card_star_ranks` and their values were
    // re-authored, so the prototypes serialize differently. Verified by bisect: stripping
    // `card_effect` from the defs alone (no code change) moves this hash the same way.
    const ARM64_HASH: &str = "7fa75ffde645c47b9e07c7f15185f5757be11bb35f069f1435ee8a6b136a0de8";
    assert_eq!(h, ARM64_HASH, "band-4 state hash differs from the arm64-pinned value (cross-platform?)");
}
