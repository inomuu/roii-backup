//! FUZZ PROPERTY GUARD against serialization holes in combat state (the class of bug behind the
//! live account's intermittent hash-mismatch). THE PROPERTY: a serialize round-trip of ANY state is
//! transparent to the forward sim —
//!     advance(deserialize(serialize(s)), n)  ==  advance(s, n)
//! which holds iff EVERY field `advance()` reads survives serialization (nothing is `#[serde(skip)]`
//! that combat depends on). Instead of one hand-picked case, sweep many seeds × zones, reach a
//! random mid-combat tick, then SPRINKLE random values across the transient combat fields on every
//! entity (so player-skill fields a plain run might not set — shields / bloodthirst / kb_shot_hit /
//! … — are covered too), round-trip, and assert the forward outcome is identical. A single dropped
//! combat field makes some seed diverge; the deterministic sim makes the failing seed reproducible.

use sim_core::data;
use sim_core::grid::{AbsorbShield, Cell, Element, Entity, Team};
use sim_core::hash::state_hash;
use sim_core::replay::advance;
use sim_core::rng::Pcg32;
use sim_core::state::GameState;

fn game_data() -> data::GameData {
    data::GameData::from_json_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/game_data.json"
    )))
    .expect("game_data.json must parse")
}

#[test]
fn fuzz_anchor_roundtrip_is_transparent() {
    let d = game_data();
    let zones = ["zone_b1_1", "zone_b2_1", "zone_b3_1", "zone_b4_1"];
    for seed in 0u64..120 {
        let zone = zones[(seed as usize) % zones.len()];
        let Ok(mut state) = data::new_game_in_zone(seed, &d, zone) else {
            continue;
        };
        // A tanky killer sustains combat so transient fields get set naturally too.
        let mut killer = Entity::idle(90_100, Cell::new(2, 2))
            .with_team(Team::Player)
            .with_walk(Cell::new(2, 2), 200)
            .with_combat(5_000_000, 4_000, 400, 300, Element::Light);
        killer.atk_range = 3;
        state.grid.spawn(killer);

        let mut r = Pcg32::new(seed.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(1));
        advance(&mut state, &d, 120 + r.gen_range_u32(0, 240) as u64);

        // Sprinkle random values across EVERY transient combat field on EVERY entity — guarantees the
        // fields are non-default (a plain run wouldn't set them all) so a dropped one shows up.
        let tick = state.tick;
        for e in state.grid.entities.iter_mut() {
            e.act_locked_until = tick + r.gen_range_u32(0, 60) as u64;
            e.bloodthirst_stacks = r.gen_range_u32(0, 6) as u8;
            e.down_ticks = r.gen_range_u32(0, 30);
            e.kb_shot_hit = r.gen_range_u32(0, 2) == 1;
            e.snare_used = r.gen_range_u32(0, 2) == 1;
            e.field_hit_count = r.gen_range_u32(0, 4) as u8;
            e.hiding_used = r.gen_range_u32(0, 2) == 1;
            if r.gen_range_u32(0, 3) == 0 {
                e.shields.push(AbsorbShield {
                    source_skill_id: String::new(),
                    pool: r.gen_range_u32(50, 2000) as i32,
                    burst_dmg: r.gen_range_u32(0, 200) as i32,
                    burst_knock: r.gen_range_u32(0, 2) as i32,
                    src_id: e.id,
                    hits_left: 0,
                    persist: false,
                });
            }
        }

        let span = 80 + r.gen_range_u32(0, 140) as u64;
        let mut live = state.clone();
        advance(&mut live, &d, span);

        let json = serde_json::to_string(&state).expect("serialize");
        let mut replay: GameState = serde_json::from_str(&json).expect("deserialize");
        advance(&mut replay, &d, span);

        assert_eq!(
            state_hash(&live),
            state_hash(&replay),
            "seed {seed} zone {zone}: a serialize round-trip diverged over {span} ticks — a combat \
             field advance() reads is not surviving serialization (a #[serde(skip)] replay hole)"
        );
    }
}

/// The sprinkle above can only cover fields someone remembered to add to it, and that is
/// exactly how it missed the next hole: it sets ENTITY fields and never touches
/// `state.projectiles`, so four `Projectile` transients the sim reads at impact
/// (`is_basic` / `burn_tier` / `focus_mult_bp` / `pin_slow_pct`) stayed `#[serde(skip)]`
/// and rejected live saves until 0.0.261. A list of dangerous fields is open-ended; you
/// cannot enumerate what nobody has written yet.
///
/// So this inverts it. Round-trip a state and require it to come back EQUAL, after
/// forgiving one explicit list — the skips that are legitimately allowed to vanish. Adding
/// a `#[serde(skip)]` now fails here until its name is added below, which is the moment to
/// answer the only question that matters: does `tick()` read it? Renderer-only, yes; if the
/// sim reads it, `skip_serializing_if` instead, or the anchor loses state the client kept.
fn take_allowed_skips(want: &mut GameState, got: &GameState) {
    // Display-only, cleared at the top of every tick — the renderer reads them, the sim never does.
    want.recent_hits = got.recent_hits.clone();
    want.knockbacks = got.knockbacks.clone();
    want.poison_bursts = got.poison_bursts.clone();
    want.field_hits = got.field_hits.clone();
    want.meteor_impacts = got.meteor_impacts.clone();
    want.trap_fires = got.trap_fires.clone();
    want.skill_fx = got.skill_fx.clone();
    // Replay infrastructure: the log and the checkpoint chain are a SEPARATE save, and
    // hashing them here would let checkpointing perturb the hashes it records.
    want.record_inputs = got.record_inputs;
    want.input_log = got.input_log.clone();
    want.snapshot_interval = got.snapshot_interval;
    want.snapshots = got.snapshots.clone();
    // Diagnostic meters — never an input to a later tick.
    want.dps = got.dps.clone();
    // Per-projectile render hints: which way the bolt flies in, what colour to tint it.
    // (`display_hits` and `crit` now SURVIVE — they gate an impact's HitEvent, audit 2026-10-02.)
    for (w, g) in want.projectiles.iter_mut().zip(got.projectiles.iter()) {
        w.sky_fall = g.sky_fall;
        w.element = g.element;
    }
}

#[test]
fn a_round_trip_loses_only_allowlisted_fields() {
    let d = game_data();
    // A ranged unit so there are projectiles to inspect at all.
    let mut state = data::new_game_as(7, &d, "zone_b2_1", "lapis").expect("zone loads");

    let mut spun = 0;
    while state.projectiles.is_empty() && spun < 3_000 {
        advance(&mut state, &d, 1);
        spun += 1;
    }
    assert!(!state.projectiles.is_empty(), "no projectile in flight after {spun} ticks");

    let mut r = Pcg32::new(0xC0FFEE);
    let tick = state.tick;
    for e in state.grid.entities.iter_mut() {
        e.act_locked_until = tick + r.gen_range_u32(0, 60) as u64;
        e.bloodthirst_stacks = r.gen_range_u32(0, 6) as u8;
        e.down_ticks = r.gen_range_u32(0, 30);
        e.kb_shot_hit = true;
        e.snare_used = true;
        e.field_hit_count = r.gen_range_u32(1, 4) as u8;
        e.hiding_used = true;
    }
    for p in state.projectiles.iter_mut() {
        p.is_basic = true;
        p.burn_tier = 3;
        p.focus_mult_bp = 30_000;
        p.pin_slow_pct = 25;
        p.sky_fall = true;
        p.display_hits = 2;
        p.crit = true;
    }

    let got: GameState =
        serde_json::from_str(&serde_json::to_string(&state).expect("serialize")).expect("deserialize");
    let mut want = state.clone();
    take_allowed_skips(&mut want, &got);

    assert_eq!(
        want, got,
        "a field vanished across serialization that is not on the allowlist in \
         take_allowed_skips — if tick() reads it, use skip_serializing_if; if it is \
         renderer-only, add it there and say so"
    );
}
