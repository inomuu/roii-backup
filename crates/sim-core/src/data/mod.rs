//! `GameData` — the read-only content catalog (GDD §4.5).
//!
//! What goes here:
//!   - Item definitions (stats, rarity, economy hints)
//!   - Monster definitions (stats, drop tables-by-id)
//!   - Zone definitions (arena size + monster spawn list)
//!   - Refine table (will move from refine.rs in a later milestone)
//!
//! What does NOT go here:
//!   - Anything in `GameState` (per-save mutable state). GameData is
//!     immutable for the life of one save.
//!   - I/O. sim-core has no `std::fs` — callers (sim-cli, godot-ext)
//!     read `data/game_data.json` from disk and hand the bytes to
//!     `GameData::from_json_str()`. (GDD §4.1.)
//!
//! Phase 0 design choices:
//!   - Validation runs at load time. Bad data panics-with-message —
//!     "fail loudly at build/load time, never at runtime" (GDD §7.1).
//!   - Lookups are linear scans over Vec. ALPHA-6 has ~5 items and
//!     ~5 monsters; a HashMap (forbidden by §4.2 rule 4) would gain
//!     us nothing and risk nondeterministic iteration.
//!   - Refine table stays in `refine.rs` for now. Will move in a
//!     later milestone when the rest of data-from-CSV lands.

pub mod bestiary_bands;
pub(crate) mod build;

/// The GRADE-odds row a monster of `level` drops on, resolved from the catalog — the same
/// function the zone builder bakes into every monster template. Public so a BALANCE TOOL can
/// state the pacing a table implies without re-deriving the mapping (and drifting from it).
pub mod build_public {
    /// See `build::items::grade_odds_for`.
    pub fn items_grade_odds_for(data: &super::GameData, stage: u16) -> Vec<u32> {
        super::build::items::grade_odds_for(data, stage)
    }
}
pub use build_public::items_grade_odds_for;
mod defs;

pub use build::*;
pub use defs::*;

#[cfg(test)]
mod tests {
    use super::*;
    use super::defs::StageBase;
    use crate::grid::{Cell, DamageType, Element, Entity, Team};
    use crate::items::{ItemKind, Rarity, WeaponType};

    fn tiny_data() -> GameData {
        GameData {
            version: "test".to_string(),
            // Left EMPTY on purpose: `level_stats` answers with a level-1-shaped row of 1s for a
            // hand-built GameData, so a fixture that cares about drops or items does not have to
            // carry a hundred-row balance table to compile.
            stage_base: Vec::new(),
            loot_rules: Default::default(),
            training: Default::default(),
            party: Default::default(),
            crests: Default::default(),
            features: Default::default(),
            gear_overrides: Vec::new(),
            items: vec![
                ItemDef {
                    num: 0,
                    id: "scrap".to_string(),
                    name: String::new(),
                    kind: ItemKind::Material,
                    atk_bonus: 0,
                    atk_speed_bp: 0,
                    weapon_type: WeaponType::None,
                    offhand_type: crate::items::OffhandType::None,
                    weapon_level: 1,
                    def_bonus: 0,
                    rarity: Rarity::Common,
                    sell_value: 0,
                    buy_price: 0,
                    consumable: None,
                    marketable: false,
                    steam_itemdef: None,
                    farm_box: false,
                    emission_group: "junk".to_string(),
                    quota_class: "unlimited".to_string(),
                    equip_slot: None,
                    card_slots: 0,
                    card_slot: None,
                    card_mods: Vec::new(),
                    card_star_step: Vec::new(),
                    card_star_ranks: Vec::new(),
                    foil_mods: Vec::new(),
                    foil_star_step: Vec::new(),
                    element: crate::grid::Element::Physical,
                    bind_mods: Vec::new(),
                    foil: false,
                    unique: false,
                    orb: None,
                    ledger: false,
                    crest_material: None,
                    xp_envelope: false,
                    gil_tradeable: false,
                    steam_tradeable: false,
                    gem: None,
                },
                ItemDef {
                    num: 0,
                    id: "test_sword".to_string(),
                    name: String::new(),
                    kind: ItemKind::Weapon,
                    atk_bonus: 10,
                    atk_speed_bp: 0,
                    weapon_type: WeaponType::Sword1H,
                    offhand_type: crate::items::OffhandType::None,
                    weapon_level: 2,
                    def_bonus: 0,
                    rarity: Rarity::Common,
                    sell_value: 0,
                    buy_price: 0,
                    consumable: None,
                    marketable: true,
                    steam_itemdef: None,
                    farm_box: false,
                    emission_group: "weapon_t1".to_string(),
                    quota_class: "tradeable".to_string(),
                    equip_slot: Some(crate::items::EquipSlot::Weapon),
                    card_slots: 0,
                    card_slot: None,
                    card_mods: Vec::new(),
                    card_star_step: Vec::new(),
                    card_star_ranks: Vec::new(),
                    foil_mods: Vec::new(),
                    foil_star_step: Vec::new(),
                    element: crate::grid::Element::Physical,
                    bind_mods: Vec::new(),
                    foil: false,
                    unique: false,
                    orb: None,
                    ledger: false,
                    crest_material: None,
                    xp_envelope: false,
                    gil_tradeable: false,
                    steam_tradeable: false,
                    gem: None,
                },
            ],
            monsters: vec![MonsterDef {
                resists: Vec::new(),
                weaknesses: Vec::new(),
                id: "slime".to_string(),
                bestiary_group: String::new(),
                hp: 30,
                atk: 8,
                def: 2,
                atk_min: 0,
                atk_max: 0,
                skills: Vec::new(),
                toughness_max: 100,
                boss_toughness_max: 0,
                speed_milli: 0,
                atk_speed_milli: 200,
                drops: vec![
                    DropRow { item_id: "scrap".to_string(), weight: 90, server_bp: 0 },
                    DropRow { item_id: "test_sword".to_string(), weight: 10, server_bp: 0 },
                ],
                atk_range: 1,
                proj_travel_ticks: 0,
                xp_reward: 10,
                gil_reward: 5,
                boss_minions: 0,
                name: String::new(),
                stage: 0,
                race: String::new(),
                size: String::new(),
                sprite: String::new(),
                hp_mult_bp: 0,
                atk_mult_bp: 0,
                def_mult_bp: 0,
                xp_mult_bp: 0,
            }],
            zones: vec![ZoneDef {
                id: "z1".to_string(),
                stage: 0,
                width: 16,
                height: 8,
                player_spawn: Cell::new(0, 0),
                unit: "test_class".to_string(),
                party: vec![],
                target_population: 2,
                spawn_interval_ticks: 10,
                spawn_pool: vec![SpawnPoolEntry {
                    monster_id: "slime".to_string(),
                    weight: 1,
                }],
                lane_pool: vec![],
                lane_group_size: None,
                boss_threshold: 5,
                boss_monster_id: String::new(), // no boss in fixture by default
                boss_from_pool: false,
                max_spawns: 0,
                player_skills: Vec::new(),
                base_monster_id: String::new(),
                next_zone_id: String::new(),
                is_instance_end: false,
                instance_start_zone_id: String::new(),
            }],
            units: vec![UnitDef {
                atk_base_milli: 0,
                atk_per_level_milli: 0,
                id: "test_class".to_string(),
                hp: 100,
                atk: 20,
                def: 5,
                element: Element::Fire,
                speed_milli: 1000,
                atk_speed_milli: 500,
                atk_range: 1,
                proj_travel_ticks: 0,
                atk_type: DamageType::Physical,
                is_ranged: false,
                skills: vec![],
                hp_per_level: 0,
                hp_factor: 0,
                hp_increase: 0,
                starting_weapon: String::new(),
            }],
            recipes: vec![],
        }
    }

    #[test]
    fn validates_tiny_data() {
        tiny_data().validate().unwrap();
    }

    #[test]
    fn rejects_duplicate_item_id() {
        let mut d = tiny_data();
        d.items.push(d.items[0].clone());
        let err = d.validate().unwrap_err();
        assert!(err.contains("duplicate item id"));
    }

    #[test]
    fn rejects_dangling_drop_reference() {
        let mut d = tiny_data();
        d.monsters[0].drops.push(DropRow {
            item_id: "ghost_item".to_string(),
            weight: 50,
            server_bp: 0,
        });
        let err = d.validate().unwrap_err();
        assert!(err.contains("unknown item"));
    }

    #[test]
    fn rejects_spawn_pool_with_zero_weight() {
        let mut d = tiny_data();
        d.zones[0].spawn_pool[0].weight = 0;
        let err = d.validate().unwrap_err();
        assert!(err.contains("total weight is 0"), "got: {err}");
    }

    #[test]
    fn rejects_spawn_pool_unknown_monster() {
        let mut d = tiny_data();
        d.zones[0].spawn_pool.push(SpawnPoolEntry {
            monster_id: "ghost_monster".to_string(),
            weight: 50,
        });
        let err = d.validate().unwrap_err();
        assert!(err.contains("unknown monster"), "got: {err}");
    }

    #[test]
    fn json_round_trip() {
        let original = tiny_data();
        let s = serde_json::to_string_pretty(&original).unwrap();
        let parsed = GameData::from_json_str(&s).unwrap();
        assert_eq!(original, parsed);
    }

    /// EVERY counter only ever goes UP, over a real run.
    ///
    /// `Measures` states this in a comment — "fields must only ever increase" — and until now
    /// nothing enforced it. Thirty-two hand-written call sites all happen to use `saturating_add`;
    /// the day one of them does not, the comment keeps saying otherwise.
    ///
    /// It matters beyond tidiness. Two systems subtract these: the online layer diffs snapshots
    /// for accrual, and a Daily Commission reads `now - start`. A counter that falls makes the
    /// first underflow and the second silently read zero progress — the player's day looks broken
    /// with nothing in the log.
    ///
    /// Field-by-field through JSON rather than a hand-written list, so a counter added next month
    /// is covered the day it lands instead of the day someone remembers to add it here.
    #[test]
    fn no_counter_ever_decreases_over_a_real_run() {
        let data = tiny_data();
        let mut state = new_game_in_zone(21, &data, "z1").unwrap();
        let snap = |m: &crate::state::Measures| -> serde_json::Map<String, serde_json::Value> {
            match serde_json::to_value(m).expect("Measures serializes") {
                serde_json::Value::Object(o) => o,
                _ => panic!("Measures should serialize to an object"),
            }
        };
        let mut prev = snap(&state.measures);
        let mut moved = 0usize;
        for t in 0..3_000 {
            crate::tick::tick(&mut state, 1);
            let now = snap(&state.measures);
            for (k, v) in &prev {
                let before = v.as_u64().unwrap_or(0);
                // A key can VANISH: skip_serializing_if drops zeros, so absence means 0 — which
                // is itself a decrease if it was ever non-zero.
                let after = now.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
                assert!(after >= before, "tick {t}: `{k}` fell {before} -> {after}");
                if after > before {
                    moved += 1;
                }
            }
            prev = now;
        }
        assert!(moved > 0, "no counter moved in 3000 ticks — this test proved nothing");
    }

    /// The Gil ledger closes over a REAL run, not a state the test built.
    ///
    /// Written this way on purpose: an assertion over hand-placed numbers proves the arithmetic in
    /// `gil_ledger_balances` and nothing about the game. Ticking a real zone means every credit
    /// goes through the combat path that actually pays players, so the day someone moves Gil
    /// without booking it, this is what notices.
    ///
    /// The `gil_earned > 0` check is not decoration — without it the whole test passes on a
    /// scenario where no Gil ever moved, which is the shape of a guard that guards nothing.
    #[test]
    fn the_gil_ledger_closes_over_a_real_run() {
        let data = tiny_data();
        let mut state = new_game_in_zone(42, &data, "z1").unwrap();
        assert!(state.gil_ledger_balances(), "a fresh game is already out of balance");
        for t in 0..3_000 {
            crate::tick::tick(&mut state, 1);
            assert!(
                state.gil_ledger_balances(),
                "tick {t}: gil {} + held {} - released {} != earned {} + received {} - spent {} - sent {}",
                state.gil(), state.measures.gil_held, state.measures.gil_released,
                state.measures.gil_earned, state.measures.gil_received,
                state.measures.gil_spent, state.measures.gil_sent,
            );
        }
        assert!(
            state.measures.gil_earned.get() > 0,
            "no Gil moved in 3000 ticks — the balance check above proved nothing",
        );
    }

    /// NO index into a container may be STORED in serialized state.
    ///
    /// `Casting.slot` was the only field that ever did, and 0.0.294 is what it cost:
    /// `clear_exemplar_skill` shrinks `skills` with `retain` while `casting` keeps pointing past
    /// the end, so the next tick indexed out of bounds. In verify-server — which links this crate
    /// and REPLAYS uploaded saves under `panic = "abort"` — that is not a failed request, it is
    /// the process, reached with one ordinary Client-authority event. The quiet variant is worse:
    /// a slot still IN range after the shift names a DIFFERENT skill and fires it silently.
    ///
    /// A stored index is safe only if EVERY path that removes from its container clears or
    /// re-maps it. Nothing can check that, and `entities`/`skills`/`card_album` all shrink. So the
    /// rule is the simpler one: store an ID and resolve it — `Casting.target_id` and
    /// `active_skills` already do, and neither can go stale.
    ///
    /// Walks the serialized JSON rather than a hand-written field list, so a field added next
    /// month is covered the day it lands. Over a REAL run plus a forced in-progress cast, because
    /// `skip_serializing_if` hides anything still at its default — an empty state would sail
    /// through this while proving nothing, which is why the walker has to find `slot` to pass.
    #[test]
    fn no_index_into_a_container_is_stored_in_serialized_state() {
        // Every index-SHAPED integer field the serialized state may contain, with the verdict
        // reached about it. Adding a line here is a decision, not a formality: either the field
        // is not a position, or someone owns clearing it wherever its container shrinks.
        const REVIEWED: &[(&str, &str)] = &[
            ("card_slots", "a COUNT of sockets on a piece of gear, not a position"),
            ("party_slots", "a COUNT of party slots owned (the deploy cap), not a position"),
            (
                "slot",
                "Casting.slot — the one true stored index. Every path that shrinks `skills` now \
                 drops the cast (0.0.294), and both readers take it with `.get()`. A SECOND field \
                 named slot must be checked against both of those before being listed here.",
            ),
        ];
        // The LAST snake_case segment, matched exactly: `inv_idx` and `card_slots` are caught,
        // and so is a bare `idx` — which a suffix test misses, since "idx" does not end with
        // "_idx". Written as suffixes this needs four extra equality arms to cover the bare forms.
        //
        // It also happens not to trip `no_new_id_matching_by_string_shape`, and that is NOT the
        // reason it is written this way: this matches a JSON KEY, never a game-data id, so it was
        // never that ratchet's target. If a future version does need suffix tests, the honest move
        // is an ALLOWED entry stating that — not a rewrite to slip past the scanner.
        fn is_index_shaped(key: &str) -> bool {
            let last = key.rsplit('_').next().unwrap_or(key);
            matches!(last, "idx" | "index" | "slot" | "slots")
        }
        fn walk(v: &serde_json::Value, path: &str, found: &mut Vec<(String, String)>) {
            match v {
                serde_json::Value::Object(o) => {
                    for (k, val) in o {
                        let here = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
                        // Only INTEGERS: `equip_slot` is an EquipSlot enum and serializes as a
                        // string, so it names a paper-doll position, not a Vec offset.
                        if val.is_u64() && is_index_shaped(k) {
                            found.push((k.clone(), here.clone()));
                        }
                        walk(val, &here, found);
                    }
                }
                serde_json::Value::Array(a) => {
                    for (i, val) in a.iter().enumerate() {
                        walk(val, &format!("{path}[{i}]"), found);
                    }
                }
                _ => {}
            }
        }

        let data = tiny_data();
        let mut state = new_game_in_zone(7, &data, "z1").unwrap();
        for _ in 0..1_500 {
            crate::tick::tick(&mut state, 1);
        }
        // Force a channel so `Casting` is present: the walker must be shown to FIND the one index
        // we know about, or an empty result would read as "clean" when it means "saw nothing".
        if let Some(e) = state.grid.entities.iter_mut().next() {
            e.casting = Some(crate::grid::Casting {
                ticks_left: 3,
                ticks_total: 10,
                slot: crate::skills::SkillSlot::new(0),
                target_id: 0,
                ground_cell: None,
            });
        }

        let json = serde_json::to_value(&state).expect("GameState serializes");
        let mut found = Vec::new();
        walk(&json, "", &mut found);

        for (key, path) in &found {
            assert!(
                REVIEWED.iter().any(|(k, _)| k == key),
                "`{path}` is a stored index into a container and nothing has decided who clears \
                 it when that container shrinks. Store an ID and resolve it (see Casting.target_id), \
                 or add it to REVIEWED with the paths that keep it valid. 0.0.294 is what the \
                 alternative costs.",
            );
        }
        assert!(
            found.iter().any(|(k, _)| k == "slot"),
            "the walker found no `slot` in a state that definitely has one — it is not looking \
             where the fields are, so its silence about everything else means nothing",
        );
    }

    /// A best hit is the RAW number, not the overkill-clamped one.
    ///
    /// This is the whole reason `note_damage_records` reads `recent_hits` instead of hooking
    /// `record_damage`, which is deliberately fed `dealt.min(pre_hp)`. Rewire it to the DPS meter
    /// and this test fails, which is the point: the hardest hit a player ever lands is precisely
    /// the one that overkills, so the clamped figure would report the victim's leftover HP while
    /// the player watched a far bigger number come off the screen.
    #[test]
    fn the_best_hit_is_the_number_the_player_saw_not_the_overkill_clamp() {
        use crate::state::{HitEvent, HitKind};
        let data = tiny_data();
        let mut s = new_game_in_zone(3, &data, "z1").unwrap();
        let hero = s.grid.entities.iter()
            .find(|e| matches!(e.team, crate::grid::Team::Player)).unwrap().id;
        let victim = s.grid.entities.iter()
            .find(|e| matches!(e.team, crate::grid::Team::Monster)).unwrap().id;
        // A killing blow far bigger than what is left to kill.
        let vidx = s.grid.idx_of_id(victim).unwrap();
        s.grid.entities[vidx].hp = 1;
        s.recent_hits.push(HitEvent {
            target_id: victim, attacker_id: hero,
            amount: 500_000, hits: 1, kind: HitKind::Normal,
        });
        // Called directly, not via `tick`: the tick CLEARS `recent_hits` at its top and
        // rebuilds it during the body, so an event staged beforehand would never be seen. In
        // production the pushes happen mid-tick, which the real-run test above covers.
        crate::tick::note_damage_records(&mut s);
        assert_eq!(
            s.records.biggest_hit, 500_000,
            "best hit was clamped to the victim's HP instead of the damage actually dealt",
        );
    }

    /// Incoming damage is booked to the party, outgoing is not — and never both.
    #[test]
    fn damage_taken_counts_hits_on_the_party_and_nothing_else() {
        use crate::state::{HitEvent, HitKind};
        let data = tiny_data();
        let mut s = new_game_in_zone(5, &data, "z1").unwrap();
        let hero = s.grid.entities.iter()
            .find(|e| matches!(e.team, crate::grid::Team::Player)).unwrap().id;
        let mob = s.grid.entities.iter()
            .find(|e| matches!(e.team, crate::grid::Team::Monster)).unwrap().id;
        let ev = |att, tgt, amt| HitEvent {
            target_id: tgt, attacker_id: att, amount: amt, hits: 1, kind: HitKind::Normal,
        };
        s.recent_hits.push(ev(mob, hero, 700));    // incoming        -> taken
        s.recent_hits.push(ev(hero, mob, 5_000));  // outgoing        -> not taken
        crate::tick::note_damage_records(&mut s);
        assert_eq!(s.measures.damage_taken.get(), 700, "outgoing damage leaked into damage_taken");
        assert_eq!(s.records.biggest_hit, 5_000, "the outgoing hit should still set the best");

        // FRIENDLY SPLASH: party -> party. The HP was really lost, so it is damage taken; it is
        // also the caster's hit, so it may set the best. Different counters, both legitimate.
        // Aiming an outgoing hit at a MONSTER (above) can never exercise this, which is how the
        // first version of this rule passed its own sabotage while dropping the damage.
        s.recent_hits.clear();
        s.loot_fx.clear();
        s.recent_hits.push(ev(hero, hero, 300));
        crate::tick::note_damage_records(&mut s);
        assert_eq!(s.measures.damage_taken.get(), 1_000, "friendly splash was not counted as taken");
    }

    /// A heal EVENT carries what the bar actually gained, and a monster's heal is not ours.
    ///
    /// The effective-restore rule is the point: an over-heal onto a full bar must add nothing, or
    /// the stat and the green number the renderer draws would tell the player different stories.
    #[test]
    fn healing_done_counts_effective_restore_by_the_party_only() {
        use crate::state::HealKind;
        let data = tiny_data();
        let mut s = new_game_in_zone(9, &data, "z1").unwrap();
        let hero = s.grid.entities.iter()
            .find(|e| matches!(e.team, crate::grid::Team::Player)).unwrap().id;
        let mob = s.grid.entities.iter()
            .find(|e| matches!(e.team, crate::grid::Team::Monster)).unwrap().id;

        s.note_heal(hero, hero, 250, HealKind::Skill);
        s.note_heal(hero, hero, 0, HealKind::Regen);   // full bar: rolled, restored nothing
        s.note_heal(mob, mob, 9_999, HealKind::Skill); // a monster mending itself
        crate::tick::note_damage_records(&mut s);
        assert_eq!(s.measures.healing_done.get(), 250);
        assert_eq!(s.heal_fx.len(), 2, "a zero-restore heal should not become an event at all");
    }

    /// Only the party's own casts count.
    #[test]
    fn skills_cast_counts_party_casts_only() {
        use crate::state::SkillFx;
        let data = tiny_data();
        let mut s = new_game_in_zone(6, &data, "z1").unwrap();
        let hero = s.grid.entities.iter()
            .find(|e| matches!(e.team, crate::grid::Team::Player)).unwrap().id;
        let mob = s.grid.entities.iter()
            .find(|e| matches!(e.team, crate::grid::Team::Monster)).unwrap().id;
        let fx = |c, t| SkillFx { caster_id: c, target_id: t, skill_id: "bash".to_string() };
        s.skill_fx.push(fx(hero, mob));
        s.skill_fx.push(fx(hero, mob));
        s.skill_fx.push(fx(mob, hero));   // a monster casting is not the party casting
        crate::tick::note_damage_records(&mut s);
        assert_eq!(s.measures.skills_cast.get(), 2);
    }

    /// A death is a TRANSITION: standing at the top of the tick, not standing at the end.
    #[test]
    fn deaths_count_the_transition_not_the_state() {
        let data = tiny_data();
        let mut s = new_game_in_zone(8, &data, "z1").unwrap();
        let hero = s.grid.entities.iter()
            .find(|e| matches!(e.team, crate::grid::Team::Player)).unwrap().id;
        let hidx = s.grid.idx_of_id(hero).unwrap();

        // Standing at the top, standing at the end -> nothing happened.
        crate::tick::note_deaths(&mut s, &[hero]);
        assert_eq!(s.measures.deaths.get(), 0, "a living member was counted as a death");

        // Standing at the top, down at the end -> one death.
        s.grid.entities[hidx].alive = false;
        crate::tick::note_deaths(&mut s, &[hero]);
        assert_eq!(s.measures.deaths.get(), 1);

        // Already down at the top -> NOT counted again. This is the whole reason the snapshot
        // exists; counting "is dead" instead of "just died" would tick up every frame of a corpse.
        crate::tick::note_deaths(&mut s, &[]);
        assert_eq!(s.measures.deaths.get(), 1, "a corpse was re-counted");
    }

    /// A monster hitting the party is not the party's personal best.
    #[test]
    fn a_monsters_swing_is_not_the_partys_best_hit() {
        use crate::state::{HitEvent, HitKind};
        let data = tiny_data();
        let mut s = new_game_in_zone(4, &data, "z1").unwrap();
        let hero = s.grid.entities.iter()
            .find(|e| matches!(e.team, crate::grid::Team::Player)).unwrap().id;
        let mob = s.grid.entities.iter()
            .find(|e| matches!(e.team, crate::grid::Team::Monster)).unwrap().id;
        s.recent_hits.push(HitEvent {
            target_id: hero, attacker_id: mob,     // monster -> player
            amount: 999_999, hits: 1, kind: HitKind::Normal,
        });
        crate::tick::note_damage_records(&mut s);
        assert!(
            s.records.biggest_hit < 999_999,
            "a monster's swing was recorded as the party's best hit",
        );
    }

    /// `damage_dealt` and the DPS meter are fed by the SAME call, so they must agree.
    ///
    /// Binding the two is the point. Asserting `damage_dealt > 0` alone would pass on a counter
    /// that double-books or drops overkill differently from the meter; tying it to a number the
    /// game already computes means the two fail together, which is the only time either failing
    /// tells you anything. Valid only while nothing calls `reset_dps` — the meter is windowed and
    /// the stat is not, which is exactly the difference under test.
    #[test]
    fn damage_dealt_agrees_with_the_dps_meter_it_rides_on() {
        let data = tiny_data();
        let mut state = new_game_in_zone(11, &data, "z1").unwrap();
        for _ in 0..3_000 {
            crate::tick::tick(&mut state, 1);
        }
        let meter: u64 = state.dps.by_class.values().sum();
        assert!(meter > 0, "no damage in 3000 ticks — this test proved nothing");
        assert_eq!(
            state.measures.damage_dealt.get(), meter,
            "lifetime damage_dealt drifted from the DPS meter that feeds it",
        );
        // by_skill is the same total sliced a different way; it must agree too.
        let by_skill: u64 = state.dps.by_skill.values().sum();
        assert_eq!(by_skill, meter, "the two DPS slices disagree");
        // And the best-hit scan is actually WIRED INTO the tick, not merely correct in isolation.
        assert!(state.records.biggest_hit > 0, "note_damage_records never ran during a real tick");
    }

    /// Every movement verb books itself. One of each, then the equation.
    #[test]
    fn every_gil_verb_books_itself() {
        use crate::state::{GilEarn, GilHold, GilPeer, GilSpend};
        let data = tiny_data();
        let mut s = new_game_in_zone(7, &data, "z1").unwrap();
        s.set_gil_for_test(10_000);

        s.earn_gil(500, GilEarn::MonsterKill);
        s.receive_gil(300, GilPeer::MarketSale);
        s.spend_gil(200, GilSpend::Craft);
        s.send_gil(100, GilPeer::MarketBuy);
        s.hold_gil(400, GilHold::BuyOrder);
        s.release_gil(150, GilHold::BuyOrder);   // change back from a cheap fill

        assert_eq!(s.gil(), 10_000 + 500 + 300 - 200 - 100 - 400 + 150);
        assert!(s.gil_ledger_balances());

        // Escrow is NOT spending. The 400 held (250 of it still outstanding) must not show up as
        // Gil this player spent -- that conflation is the bug the separate verbs exist to prevent.
        assert_eq!(s.measures.gil_spent.get(), 200, "escrow leaked into spending");
        assert_eq!(s.measures.gil_held.get() - s.measures.gil_released.get(), 250, "outstanding escrow wrong");
    }

    #[test]
    fn new_game_in_zone_spawns_player_and_monsters() {
        let data = tiny_data();
        let state = new_game_in_zone(42, &data, "z1").unwrap();

        // Exactly one Player at zone.player_spawn with class stats.
        let players: Vec<&Entity> = state.grid.entities.iter()
            .filter(|e| matches!(e.team, Team::Player)).collect();
        assert_eq!(players.len(), 1);
        assert_eq!(players[0].id, FIRST_PLAYER_ID);
        assert_eq!(players[0].pos, Cell::new(0, 0));
        // Spawns full. VIT is gone (0.0.86), so there is no longer a 1% on top:
        // effective_max_hp = 100 + 0·lvl = 100.
        assert_eq!(players[0].hp, 100);
        assert_eq!(players[0].atk, 20);

        // Initial population = zone.target_population.
        let monsters: Vec<&Entity> = state.grid.entities.iter()
            .filter(|e| matches!(e.team, Team::Monster)).collect();
        assert_eq!(monsters.len(), 2);
        for m in &monsters {
            assert_eq!(m.hp, 30);
            assert_eq!(m.drops.len(), 2);
        }
        // ZoneSpawnState is wired up.
        assert!(state.zone_spawn.is_some());
    }

    #[test]
    fn a_monsters_stats_are_its_levels_base_times_its_own_multipliers() {
        let mut data = tiny_data();
        // The level table IS the difficulty curve now. A zone names a level; a monster names how
        // much of that level it is. Strength stopped depending on where a creature stands.
        data.stage_base = vec![StageBase { stage: 7, hp: 1000, atk: 100, xp: 50, gil: 0, def: 70, toughness: 25 }];
        data.zones[0].stage = 7;
        let mut role = data.monsters[0].clone();
        role.id = "role_mob".to_string();
        role.hp = 1; // IGNORED — the level's base is what a monster is built from
        role.atk = 1;
        role.def = 10;
        role.hp_mult_bp = 20_000; // a 2x HP tank
        role.atk_mult_bp = 8_000; // hitting for 0.8x
        data.monsters.push(role);
        data.zones[0].boss_monster_id = String::new();
        data.zones[0].spawn_pool = vec![SpawnPoolEntry {
            monster_id: "role_mob".to_string(),
            weight: 1,
        }];

        let state = new_game_in_zone(42, &data, "z1").unwrap();
        let tmpl = &state.zone_spawn.as_ref().unwrap().pool[0].template;
        assert_eq!(tmpl.hp, 2000, "hp = level 7's base 1000 x 2.0");
        assert_eq!(tmpl.atk, 80, "atk = level 7's base 100 x 0.8");
        assert_eq!(tmpl.xp_reward, 50, "xp = the level's base (no multiplier named)");
        // DEF IS on the curve as of 0.0.277, and the authored 10 above is ignored exactly as the
        // authored hp/atk are. The earlier reading — that RO mob DEF has no relationship to level
        // (correlation 0.51 renewal, 0-127 at every level) so armour must stay monster identity —
        // was right about the SPREAD and wrong about the conclusion: our own hand-authored column
        // had a 0.92 correlation with level and a slope of 0.284, i.e. someone was already tracking
        // level by hand, just far too shallowly to matter (1.1% to 6.9% mitigation across the whole
        // game). Identity now lives in `def_mult_bp`, the same place HP's and ATK's does.
        assert_eq!(tmpl.def, 70,
            "def = the stage table's (this monster names no def_mult_bp)");
        assert_eq!(tmpl.stage, 7, "the zone's stage reaches the spawned monster");

        // And the identity knob works: 3x plate triples the armour the level handed out.
        data.monsters.last_mut().unwrap().def_mult_bp = 30_000;
        let state = new_game_in_zone(42, &data, "z1").unwrap();
        let tmpl = &state.zone_spawn.as_ref().unwrap().pool[0].template;
        assert_eq!(tmpl.def, 210, "def_mult_bp 3x -> def 21, x10 stat scale");
    }

    /// Softening a band and paying less for it were ONE edit until 0.0.262: XP borrowed
    /// `hp_scale_bp`, so cutting a wall's HP by 20% silently cut its XP by 20% too. This
    /// pins both halves of the split — that XP still follows HP when the zone says nothing
    /// (every zone authored before the field, i.e. all of them), and that it stops following
    /// the moment the zone says otherwise.
    #[test]
    fn xp_is_its_own_column_so_a_level_can_pay_without_getting_harder() {
        // The old shape had XP riding `hp_scale_bp` unless a zone opted out, which made "this band
        // is too hard" and "this band pays too little" the same edit. In the table they are
        // separate columns, so that cannot recur — this checks the property, not a knob.
        let build = |hp: i32, xp: u32| {
            let mut data = tiny_data();
            data.zones[0].boss_monster_id = String::new();
            data.zones[0].stage = 3;
            data.stage_base = vec![StageBase { stage: 3, hp, atk: 10, xp, gil: 0, def: 30, toughness: 21 }];
            let state = new_game_in_zone(42, &data, "z1").unwrap();
            let t = &state.zone_spawn.as_ref().unwrap().pool[0].template;
            (t.hp, t.xp_reward)
        };
        let (hp0, xp0) = build(1000, 100);
        assert!(hp0 > 0 && xp0 > 0, "the fixture must produce both");

        // Pay more at the same difficulty.
        let (hp1, xp1) = build(1000, 400);
        assert_eq!(hp1, hp0, "HP must not move when only the reward does");
        assert_eq!(xp1, xp0 * 4);

        // Harder at the same pay.
        let (hp2, xp2) = build(4000, 100);
        assert_eq!(hp2, hp0 * 4);
        assert_eq!(xp2, xp0, "the reward must not move when only the difficulty does");
    }

    #[test]
    fn buy_item_spends_gil_and_banks_to_stash() {
        let mut data = tiny_data();
        data.items[1].buy_price = 50; // test_sword
        let mut state = new_game_in_zone(42, &data, "z1").unwrap();
        let stash0 = state.inventory.len();
        // Not enough Gil → err, untouched.
        state.set_gil_for_test(30);
        assert!(buy_item(&mut state, &data, "test_sword", 1).is_err());
        assert_eq!(state.gil(), 30);
        // Buy 2 for 100 Gil → banked, Gil spent.
        state.set_gil_for_test(120);
        assert!(buy_item(&mut state, &data, "test_sword", 2).is_ok());
        assert_eq!(state.gil(), 20);
        assert_eq!(state.inventory.len(), stash0 + 2, "two swords banked");
        // Item with no buy_price is not for sale.
        assert!(buy_item(&mut state, &data, "scrap", 1).is_err());
    }

    #[test]
    fn buy_into_a_full_stash_refunds_nothing_and_loses_nothing() {
        // Bug fix: a full inventory used to take the Gil and drop the item.
        let mut data = tiny_data();
        data.items[1].buy_price = 50; // test_sword (non-stackable gear)
        let mut state = new_game_in_zone(42, &data, "z1").unwrap();
        // Fill the stash to its cap with junk.
        while state.inventory.len() < state.inventory_cap() {
            state.inventory.push(crate::data::instantiate_item(&data.items[0])); // scrap
        }
        state.set_gil_for_test(1000);
        let gil0 = state.gil();
        let n0 = state.inventory.len();
        assert!(buy_item(&mut state, &data, "test_sword", 1).is_err(), "full stash rejects the buy");
        assert_eq!(state.gil(), gil0, "no Gil spent on a rejected buy");
        assert_eq!(state.inventory.len(), n0, "no slot change");
    }

    #[test]
    fn bought_rare_item_goes_to_stash_not_the_rare_bag() {
        let mut data = tiny_data();
        data.items[1].buy_price = 50; // test_sword
        data.items[1].rarity = Rarity::Rare; // at/above the default rare threshold
        let mut state = new_game_in_zone(42, &data, "z1").unwrap();
        let stash0 = state.inventory.len();
        state.set_gil_for_test(100);
        assert!(buy_item(&mut state, &data, "test_sword", 1).is_ok());
        assert_eq!(state.inventory.len(), stash0 + 1, "purchase banked to the stash");
        assert!(state.rare_bag.is_empty(), "a PURCHASE never routes to the rare bag");
    }

    #[test]
    fn migrate_save_items_refreshes_def_fields_keeps_instance_state_and_qty() {
        use crate::items::Item;
        let data = tiny_data(); // test_sword def: atk 10, weapon_level 2
        let mut state = new_game_in_zone(42, &data, "z1").unwrap();
        // A stale sword as if from an OLD def (atk 99, weapon_level 4), upgraded.
        let mut stale = Item::new("test_sword", crate::items::ItemKind::Weapon, 99, 0, Rarity::Common);
        stale.weapon_level = 4;
        stale.quantity = 1;
        state.inventory.push(stale);
        // A stackable material whose quantity must survive migration.
        let mut ore = Item::new("scrap", crate::items::ItemKind::Material, 0, 0, Rarity::Common);
        ore.quantity = 40;
        state.inventory.push(ore);
        // A weapon carrying an off-hand stamp from before 0.0.383 (dual wield is gone): the
        // stamp is cleared on load so the piece re-derives its natural main-hand slot.
        let mut offhand = Item::new("test_sword", crate::items::ItemKind::Weapon, 5, 0, Rarity::Common);
        offhand.equip_slot = Some(crate::items::EquipSlot::WeaponLeft);
        state.inventory.push(offhand);

        migrate_save_items(&mut state, &data);

        let sword = state.inventory.iter().find(|i| i.def_id == "test_sword" && i.equip_slot.is_none()).unwrap();
        assert_eq!(sword.atk_bonus, 10, "def atk applied (99 -> 10)");
        assert_eq!(sword.weapon_level, 2, "def weapon_level applied");
        assert!(!state.inventory.iter().any(|i| i.equip_slot == Some(crate::items::EquipSlot::WeaponLeft)
            || (i.weapon_type != crate::items::WeaponType::None && i.equip_slot == Some(crate::items::EquipSlot::Shield))),
            "an off-hand stamp on a weapon is cleared on load (0.0.383)");
        let mat = state.inventory.iter().find(|i| i.def_id == "scrap").unwrap();
        assert_eq!(mat.quantity, 40, "per-instance quantity preserved");
    }

    #[test]
    fn migrate_save_items_preserves_every_per_instance_field() {
        // REGRESSION (0.0.222): migrate_item used to rebuild the item from its def and copy
        // earned state back from a hand-maintained list — so any per-instance field missing
        // from that list was silently wiped on EVERY load. It cost us the Exemplar marks
        // (equipped_by/resonance_stars: loading unequipped your Exemplar and reset Resonance,
        // and unequip then no-op'd because nothing carried the mark) and the market
        // instance_id. Assert the WHOLE earned set survives.
        use crate::items::Item;
        let data = tiny_data();
        let mut state = new_game_in_zone(42, &data, "z1").unwrap();
        let mut it = Item::new("test_sword", crate::items::ItemKind::Weapon, 99, 0, Rarity::Common);
        it.resonance_stars = 3;
        it.equipped_by = 2;
        it.instance_id = Some("abc123".to_string());
        it.star = 2;
        it.foil = true;
        it.bound = true;
        it.locked_slot = Some(1);
        state.inventory.push(it);

        migrate_save_items(&mut state, &data);

        let m = state.inventory.iter().find(|i| i.instance_id.is_some()).expect("item survived");
        assert_eq!(m.resonance_stars, 3, "resonance_stars (Exemplar rank)");
        assert_eq!(m.equipped_by, 2, "equipped_by (Exemplar equip mark)");
        assert_eq!(m.instance_id.as_deref(), Some("abc123"), "instance_id (market identity)");
        assert!(m.foil, "foil");
        assert!(m.bound, "bound (use-only mark, 0.0.227)");
        // GEAR loses its ★ and its lock on load since 0.0.382 (fixed-stat gear has neither) —
        // that is a migration, not a wipe: the card below keeps its star, which is its level.
        assert_eq!(m.star, 0, "gear star cleared on purpose");
        assert_eq!(m.locked_slot, None, "gear lock cleared on purpose");
        // ...while the DEF fields still refresh.
        assert_eq!(m.atk_bonus, 10, "def atk still applied (99 -> 10)");

        let mut card = Item::new("test_card", crate::items::ItemKind::Card, 0, 0, Rarity::Mythical);
        card.star = 3;
        card.instance_id = Some("card1".to_string());
        state.inventory.push(card);
        migrate_save_items(&mut state, &data);
        let c = state.inventory.iter().find(|i| i.instance_id.as_deref() == Some("card1")).expect("card survived");
        assert_eq!(c.star, 3, "a CARD's star is its level and survives");
    }

    /// 0.0.448: a save that still holds gems in the materials ledger gets them as bag stacks on
    /// load — every one of them, past the cap if it must — and a second load moves nothing.
    #[test]
    fn ledger_gems_migrate_into_the_bag_and_only_once() {
        let data = GameData::from_json_str(include_str!("../../../../data/game_data.json")).expect("parse");
        let mut s = new_game_in_zone(1, &data, "zone_b1_1").expect("zone");
        s.credit_material("gem_strife_g2", 7, crate::state::MaterialSource::Drop);
        s.credit_material("gem_life_g0", 2, crate::state::MaterialSource::Drop);
        let bag_before = s.inventory.len();
        migrate_save_items(&mut s, &data);
        assert_eq!(s.material_count("gem_strife_g2"), 0);
        assert_eq!(s.material_count("gem_life_g0"), 0);
        assert_eq!(crate::gems::held_count(&s, "gem_strife_g2"), 7);
        assert_eq!(crate::gems::held_count(&s, "gem_life_g0"), 2);
        assert_eq!(s.inventory.len(), bag_before + 2);
        let snap = s.clone();
        migrate_save_items(&mut s, &data);
        assert_eq!(s.inventory, snap.inventory, "idempotent");
    }

    #[test]
    fn migrate_save_items_is_idempotent() {
        // GUARDS the online resume-across-a-catalog-change fix (verify-server get_save
        // "migrate-on-resume"): the server migrates the stored save ONCE (→ a hash it then
        // trusts) and the client `load_json`s it + migrates AGAIN — which MUST reproduce the
        // same hash, or the resume still fails to anchor ("not anchored"). A non-idempotent
        // migrate would silently break every returning player after a balance patch. Uses the
        // REAL catalog + `new_game` (which EQUIPS the starting weapon — the item class whose
        // stat drift originally triggered this bug).
        // Crate-relative (CARGO_MANIFEST_DIR = crates/sim-core), so the path doesn't depend
        // on how deep THIS file sits — unlike a fragile "../../.." count.
        let data = crate::data::GameData::from_json_str(
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json"))
        ).unwrap();
        let mut state = new_game_in_zone(42, &data, "zone_b1_1").unwrap();
        crate::replay::advance(&mut state, &data, 50); // accumulate some kills/loot/state
        migrate_save_items(&mut state, &data);
        let h1 = crate::hash::state_hash(&state);
        migrate_save_items(&mut state, &data);
        let h2 = crate::hash::state_hash(&state);
        assert_eq!(h1, h2, "migrate must be idempotent — the migrate-on-resume anchor fix depends on it");
    }

    /// Slots open by PROGRESS (0.0.490): reaching stages 2 / 6 / 18 / 24. Each
    /// opening counts once in `party_slots_bought` (the server pays off it), a save that already
    /// holds more slots keeps them, and the retired input buys nothing.
    #[test]
    fn party_slots_open_at_stages_2_6_18_24() {
        let data = tiny_data();
        let mut state = new_game_in_zone(42, &data, "z1").unwrap();
        let open = |s: &mut crate::state::GameState| { open_earned_slots(s); s.party_slots() };
        assert_eq!((open(&mut state), next_slot_requirement(&state)), (1, Some(2)));
        state.measures.boss_kills.add(1);
        assert_eq!(open(&mut state), 1, "room 1-1's boss is not stage 1's");
        state.band_reached = 2;
        assert_eq!((open(&mut state), next_slot_requirement(&state)), (2, Some(6)));
        state.band_reached = 5;
        assert_eq!(open(&mut state), 2, "stage 5 is not stage 6");
        state.band_reached = 18;
        assert_eq!((open(&mut state), next_slot_requirement(&state)), (4, Some(24)), "6 and 18 together");
        state.band_reached = 24;
        assert_eq!((open(&mut state), next_slot_requirement(&state)), (5, None));
        assert_eq!(state.measures.party_slots_bought.get(), 4, "one count per slot opened");
        assert_eq!(open(&mut state), 5, "idempotent");
        assert_eq!(state.measures.party_slots_bought.get(), 4);

        // A save that bought slots with Gil keeps them; progress only ever raises.
        let mut old = new_game_in_zone(42, &data, "z1").unwrap();
        old.party_slots = 4;
        open_earned_slots(&mut old);
        assert_eq!((old.party_slots(), old.measures.party_slots_bought.get()), (4, 0));

        // The retired input parses and buys nothing, Gil or not.
        old.earn_gil(1_000_000, crate::state::GilEarn::Debug);
        crate::events::apply_input_with(&mut old, Some(&data), crate::events::InputEvent::BuyPartySlot);
        assert_eq!(old.party_slots(), 4);
    }

    #[test]
    fn party_slots_are_bought_with_gil_cheaply_and_cap_the_field() {
        let mut data = tiny_data();
        data.party.slot_gil = vec![5_000, 8_000, 12_000, 15_000];
        let mut state = new_game_in_zone(42, &data, "z1").unwrap();
        assert_eq!(party_size(&state), 1);
        assert_eq!(state.party_slots(), 1, "no slot bought: the cap is what stands in the field");
        assert_eq!(party_slot_cost(&state, &data), Some(5_000));

        // The field is capped by the slots, not by MAX_PARTY.
        assert!(model_add_member(&mut state, &data, "test_class").is_err());

        // Can't afford → refused, untouched.
        state.set_gil_for_test(10);
        assert!(buy_party_slot(&mut state, &data).is_err());
        assert_eq!(state.party_slots(), 1);
        assert_eq!(state.gil(), 10);

        // Buy all four, the catalog's prices in order.
        state.set_gil_for_test(100_000);
        for (n, cost) in [(2usize, 5_000u64), (3, 8_000), (4, 12_000), (5, 15_000)] {
            assert_eq!(party_slot_cost(&state, &data), Some(cost));
            assert_eq!(buy_party_slot(&mut state, &data).unwrap(), n);
            assert_eq!(state.party_slots(), n);
        }
        assert_eq!(state.gil(), 100_000 - 40_000);
        assert_eq!(state.measures.party_slots_bought.get(), 4);
        assert_eq!(party_slot_cost(&state, &data), None, "MAX_PARTY: nothing left to buy");
        assert!(buy_party_slot(&mut state, &data).is_err());

        // The model path fields members for nothing, up to the slots.
        assert!(model_add_member(&mut state, &data, "ghost_class").is_err(), "unknown class");
        for size in 1..MAX_PARTY {
            model_add_member(&mut state, &data, "test_class").unwrap();
            assert_eq!(party_size(&state), size + 1);
        }
        assert!(model_add_member(&mut state, &data, "test_class").is_err(), "full");
        assert_eq!(state.gil(), 100_000 - 40_000, "the model path spends nothing");
        let mut ids: Vec<u32> = state.grid.entities.iter()
            .filter(|e| matches!(e.team, Team::Player))
            .map(|e| e.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), MAX_PARTY);
    }

    #[test]
    fn a_fielded_party_owns_its_slots_on_load_and_keeps_them_when_benching() {
        // A pre-0.0.413 save fields five with `party_slots` 0: the load path claims the five as
        // owned, and benching one keeps the cap at five (0.0.418 — reading the field live
        // lost a slot per bench).
        let data = tiny_data();
        let mut state = new_game_in_zone(42, &data, "z1").unwrap();
        state.party_slots = 5;
        for _ in 1..MAX_PARTY {
            model_add_member(&mut state, &data, "test_class").unwrap();
        }
        state.party_slots = 0;
        assert_eq!(state.party_slots(), 1, "unclaimed: the field is not the cap");
        migrate_save_items(&mut state, &data);
        assert_eq!(state.party_slots(), MAX_PARTY, "the load path claims what stands");
        let benched = state.grid.entities.iter().filter(|e| matches!(e.team, Team::Player)).map(|e| e.id).max().unwrap();
        bench(&mut state, benched).unwrap();
        assert_eq!(state.party_slots(), MAX_PARTY, "benching does not sell the slot");
        deploy(&mut state, &data, benched).unwrap();
        assert_eq!(party_size(&state), MAX_PARTY);
    }

    #[test]
    fn roster_buy_deploy_bench_roundtrip() {
        let data = tiny_data();
        let mut state = new_game_in_zone(42, &data, "z1").unwrap();
        assert_eq!(party_size(&state), 1); // the lead
        assert_eq!(owned_count(&state), 1);

        // Bench two characters (the model path — a pull lands here too) → roster grows, party
        // unchanged.
        state.party_slots = 5;
        let a = model_bench_member(&mut state, &data, "test_class").unwrap();
        let b = model_bench_member(&mut state, &data, "test_class").unwrap();
        assert_ne!(a, b, "unique ids");
        assert_eq!(state.roster.len(), 2);
        assert_eq!(party_size(&state), 1, "benching does not deploy");
        assert_eq!(owned_count(&state), 3);
        assert_eq!(state.gil(), 0, "the model path spends nothing");

        // Unknown class → no-op.
        assert!(model_bench_member(&mut state, &data, "ghost").is_err());
        assert_eq!(state.roster.len(), 2);

        // Tag char `a`'s benched stats so we can prove deploy restores them.
        state.roster.get_mut(&a).unwrap().data.stats.level = 27;

        // Deploy `a` → moves to the grid with its saved stats; roster shrinks.
        deploy(&mut state, &data, a).unwrap();
        assert_eq!(party_size(&state), 2);
        assert_eq!(state.roster.len(), 1);
        let ea = state.grid.idx_of_id(a).unwrap();
        assert_eq!(state.grid.entities[ea].stats.level, 27, "loadout restored");

        // Bench `a` back → roster regrows, party shrinks, live state captured.
        state.grid.entities[ea].stats.level = 28; // change while deployed
        bench(&mut state, a).unwrap();
        assert_eq!(party_size(&state), 1);
        assert_eq!(state.roster.len(), 2);
        assert_eq!(state.roster[&a].data.stats.level, 28, "live state captured");

        // No protected lead: with another member deployed, even id 1 (the
        // first-spawned char) can be benched. Only rule: >=1 must remain.
        deploy(&mut state, &data, a).unwrap(); // party = {id1, a}
        assert!(bench(&mut state, FIRST_PLAYER_ID).is_ok(), "lead is benchable when another remains");
        assert_eq!(party_size(&state), 1); // {a}
        assert!(bench(&mut state, a).is_err(), "cannot bench the last member");
        assert!(bench(&mut state, 999).is_err()); // not deployed
        // State now: deployed {a}, benched {FIRST_PLAYER_ID, b}.

        // Deploy respects the slots (5 here = MAX_PARTY): fill to 5, then refuse.
        let c = model_bench_member(&mut state, &data, "test_class").unwrap();
        let d = model_bench_member(&mut state, &data, "test_class").unwrap();
        let extra = model_bench_member(&mut state, &data, "test_class").unwrap();
        deploy(&mut state, &data, FIRST_PLAYER_ID).unwrap(); // {a, id1} = 2
        deploy(&mut state, &data, b).unwrap(); // + b = 3
        deploy(&mut state, &data, c).unwrap(); // + c = 4
        deploy(&mut state, &data, d).unwrap(); // + d = 5 = MAX_PARTY
        assert_eq!(party_size(&state), MAX_PARTY);
        assert!(deploy(&mut state, &data, extra).is_err(), "party full");
    }

    #[test]
    fn rejects_unknown_player_class_in_zone() {
        let mut d = tiny_data();
        d.zones[0].unit = "ghost_class".to_string();
        let err = d.validate().unwrap_err();
        assert!(err.contains("unknown unit"));
    }

    #[test]
    fn rejects_player_spawn_out_of_bounds() {
        let mut d = tiny_data();
        d.zones[0].player_spawn = Cell::new(99, 99);
        let err = d.validate().unwrap_err();
        assert!(err.contains("player_spawn out of bounds"));
    }

    /// Real `data/game_data.json` must parse and validate. This test
    /// reads the file at build time via `include_str!`; if the path
    /// shifts, the test fails loudly so we notice.
    #[test]
    fn real_game_data_json_parses_and_validates() {
        let s = include_str!("../../../../data/game_data.json");
        let data = GameData::from_json_str(s).expect("game_data.json must parse + validate");
        assert!(!data.items.is_empty());
        assert!(!data.monsters.is_empty());
        assert!(!data.zones.is_empty());
    }

    /// 0.0.129 CHUM CARD SET: the 4 authored cards carry the intended shape and
    /// each family monster drops ITS OWN card (self-drop convention; elite/boss
    /// = the same card at rate-up, never a separate card). Guards against the
    /// placeholder-era cross-drops (chomb used to drop card_yellowling).
    #[test]
    fn chum_card_set_wired_to_its_monsters() {
        use crate::items::{CardClass, StatMod};
        let s = include_str!("../../../../data/game_data.json");
        let data = GameData::from_json_str(s).unwrap();

        let card = |id: &str| data.items.iter().find(|i| i.id == id).expect(id);
        // Shape: slot class + the effect axes each card is built around.
        let chomb = card("card_chomb");
        assert_eq!(chomb.card_slot, Some(CardClass::Weapon));
        let has = |mods: &[crate::items::StatMod], f: fn(&crate::items::StatMod) -> bool| {
            mods.iter().any(f)
        };
        assert!(
            has(&chomb.card_mods, |m| matches!(m, StatMod::AtkFlat(v) if *v > 0))
                && has(&chomb.card_mods, |m| matches!(m, StatMod::CritBp(v) if *v > 0)),
            "chomb = atk + crit burst"
        );
        let toxic = card("card_toxic_chom");
        assert_eq!(toxic.card_slot, Some(CardClass::Earring));
        assert!(has(&toxic.card_mods, |m| matches!(m, StatMod::DodgeBp(v) if *v > 0)));
        let burst = card("card_burst_chom");
        assert_eq!(burst.card_slot, Some(CardClass::Necklace));
        assert!(
            has(&burst.card_mods, |m| matches!(m, StatMod::AtkFlat(v) if *v > 0))
                && has(&burst.card_mods, |m| matches!(m, StatMod::DefFlat(v) if *v < 0)),
            "burst chom = glass cannon (drawback)"
        );
        assert!(
            !burst.card_star_step.iter().any(|m| matches!(m, StatMod::DefFlat(_))),
            "the DRAWBACK line never grows with stars"
        );
        let rechum = card("card_rechum");
        assert_eq!(rechum.card_slot, Some(CardClass::Shoes));
        assert!(
            has(&rechum.card_mods, |m| matches!(m, StatMod::MaxHpBp(v) if *v > 0)),
            "max-HP % in bp"
        );

        // Drop wiring: each monster's table carries its own family card.
        let drops_card = |mon: &str, cid: &str| -> bool {
            data.monsters
                .iter()
                .find(|m| m.id == mon)
                .expect(mon)
                .drops
                .iter()
                .any(|d| d.item_id == cid)
        };
        for mon in ["chomb", "chomber", "chomboom"] {
            assert!(drops_card(mon, "card_chomb"), "{mon} drops card_chomb");
        }
        assert!(!drops_card("chomb", "card_yellowling"), "placeholder cross-drop removed");
        assert!(drops_card("toxic_chom", "card_toxic_chom"));
        assert!(drops_card("burst_chom", "card_burst_chom"));
        assert!(drops_card("rechum", "card_rechum"));
    }
}

#[cfg(test)]
mod catalog_version_tests {

    /// NO CLIFFS ON THE LADDER (user 2026-09-30). Stage HP was sized from the measuring run's
    /// summed party ATK, so the run's party growing from one member to five put a ×6 HP step
    /// between stages 1 and 2 and ×1.86 at 9→10. A monster stat that climbs more than 20% in one
    /// stage is a wall a player hits by walking one room forward; `tools/build_stage_ladder.py`
    /// (`smooth_opening`) keeps the opening under it, and this keeps the whole ladder under it.
    #[test]
    fn no_stage_raises_monster_hp_atk_or_xp_by_more_than_a_fifth() {
        let data = super::GameData::from_json_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json"
        ))).expect("catalog");
        let rows = &data.stage_base;
        assert!(rows.len() > 1, "no ladder to check");
        for w in rows.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            for (what, x, y) in [("hp", a.hp as i64, b.hp as i64), ("atk", a.atk as i64, b.atk as i64), ("xp", a.xp as i64, b.xp as i64)] {
                assert!(y * 10 <= x * 12,
                    "stage {} -> {}: {what} {x} -> {y} climbs more than 20% in one stage", a.stage, b.stage);
            }
        }
    }

    /// A REMAP whose target does not exist turns a recognisable stale item into an unrecognisable
    /// one — strictly worse than leaving it alone. This caught a real one: `ore_primocon ->
    /// ore_valkirion` outlived the ore it pointed at by one release (refine took both in 0.0.370),
    /// and nothing else would have noticed until a player with an old save loaded it.
    #[test]
    fn the_remap_table_points_at_real_defs() {
        let data = super::GameData::from_json_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json"
        ))).expect("catalog");
        for (old, new) in crate::data::build::items::DEF_ID_REMAP {
            assert!(!old.is_empty() && !new.is_empty());
            // Prefix entries: at least one real def must start with the NEW prefix, or the rename
            // is pointing at a family that no longer exists.
            assert!(data.items.iter().any(|d| d.id.starts_with(new)),
                "remap {old} -> {new}: no def in the catalog starts with {new}");
            assert!(!data.items.iter().any(|d| d.id.starts_with(old)),
                "remap {old} -> {new}: the catalog still has defs named {old}, so the rename is \
                 half-done and the remap would rewrite a LIVE id");
        }
    }

    /// THE CATALOG'S CONTENT AND ITS VERSION STRING ARE ONE DECISION.
    ///
    /// `verify-server` gates every uploaded save on `bundle.game_data_version != data.version`
    /// (main.rs:3210) and bakes its copy of the catalog into the image at build time
    /// (Dockerfile:43). When the two versions DIFFER the player gets a clean 409 and knows to
    /// update. The dangerous case is the opposite one: content that differs while the version
    /// string does not. The gate waves it through, the replay then diverges on a monster's HP, and
    /// the mismatch surfaces as a hash that does not reproduce — which reads as a cheating player,
    /// not as a deploy that was never done.
    ///
    /// That is not hypothetical. `version` sat at 0.0.68-alpha from 2026-08-05 through six catalog
    /// changes — capped zones, `level_base`, the mdef re-point, `def` on the level, 100 bands,
    /// marionette — because nothing anywhere required the two to move together. This does.
    ///
    /// WHEN THIS FAILS: you changed `data/game_data.json`. Bump its `version`, put the new digest
    /// below, and remember the server needs a REDEPLOY (`tools/deploy.sh`) before a client on the
    /// new catalog can save. Re-pinning the digest alone is the one thing to not do — the version
    /// line is directly above it for that reason.
    #[test]
    fn the_catalog_cannot_change_without_its_version_changing() {
        const VERSION: &str = "0.0.224-alpha";
        const CONTENT_DIGEST: &str = "0c1bf8e7e6ba9c7e1341de2e57daf9ac22a8de582a20f942853c029fe434bb94";

        let raw = include_str!("../../../../data/game_data.json");
        let mut v: serde_json::Value = serde_json::from_str(raw).expect("game_data.json parses");
        let got_version = v
            .get("version")
            .and_then(|s| s.as_str())
            .unwrap_or_default()
            .to_string();

        // The digest covers everything EXCEPT the version field, so it answers "did the content
        // move?" rather than "did anything in the file move?". serde_json's default map is a
        // BTreeMap, so re-serialising sorts the keys and the digest does not depend on how the
        // file happens to be formatted.
        v.as_object_mut().expect("top level is an object").remove("version");
        let digest = blake3::hash(&serde_json::to_vec(&v).expect("re-serialises")).to_hex();

        assert_eq!(
            got_version, VERSION,
            "game_data.json says version {got_version} but this test pins {VERSION} — whichever \
             is stale, the server gates on this string"
        );
        assert_eq!(
            digest.as_str(), CONTENT_DIGEST,
            "the catalog's content changed. Bump `version` in data/game_data.json (it is \
             {got_version}), set VERSION and CONTENT_DIGEST above to match, and redeploy the \
             server — its copy is baked in at image build."
        );
    }

    /// Every `@ battle <zone>` a story mission names must be a zone that EXISTS.
    ///
    /// `prologue.dialogue` pointed at `zone prologue_gate`, which was never authored. The client
    /// warned and carried on, so the script ran to `=> END`, the summary screen came up, and the
    /// mission paid its Gil for a fight that never happened — a one-word typo was a free reward.
    /// The client now fails the mission instead, but the failure still needs a player to hit it.
    /// This catches it at `cargo test`, where a typo belongs.
    ///
    /// Two files in two languages that must agree, so nothing but a test binds them — the same
    /// shape as the catalog digest above. Reads at RUNTIME rather than `include_str!` so a
    /// packaged build without the Godot tree still compiles; a MISSING directory skips, a present
    /// one with a bad zone fails.
    #[test]
    fn every_story_battle_names_a_zone_that_exists() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../godot/story");
        let Ok(entries) = std::fs::read_dir(dir) else {
            eprintln!("skip: {dir} not present (packaged build?)");
            return;
        };
        let data = super::defs::GameData::from_json_str(include_str!("../../../../data/game_data.json"))
            .expect("game_data.json must parse");

        let mut checked = 0usize;
        let mut missing: Vec<String> = Vec::new();
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|s| s.to_str()) != Some("dialogue") {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(&path) else { continue };
            let file = path.file_name().and_then(|s| s.to_str()).unwrap_or("?").to_string();
            for line in src.lines() {
                let t = line.trim();
                // `@ battle <zone_id> [ticks]`. A leading `#` is a comment, including one that
                // quotes a cue — so the marker has to be the first thing on the line.
                let Some(rest) = t.strip_prefix("@ battle ") else { continue };
                let Some(zone) = rest.split_whitespace().next() else { continue };
                checked += 1;
                if data.zones.iter().all(|z| z.id != zone) {
                    missing.push(format!("{file}: @ battle {zone}"));
                }
            }
        }

        assert!(
            checked > 0,
            "no `@ battle` cue found in {dir} — either the missions lost their fights or this \
             test stopped matching the cue syntax, and a guard that checks nothing passes always"
        );
        assert!(
            missing.is_empty(),
            "story missions name {} zone(s) that do not exist in data/game_data.json:\n  {}\n\
             Author the zone or fix the id — a mission whose fight cannot start is a mission that \
             pays for nothing.",
            missing.len(),
            missing.join("\n  "),
        );
    }

    /// The smite PREVIEW must anchor its box the way the sim anchors the hit.
    ///
    /// `grid::Blast::sided` decides which cells are HIT; `blast_fx_3d.gd::sync_smite_aim` decides
    /// which are SHOWN, in a different language with no compiler between them. A preview that
    /// disagrees is worse than none: the player aims at what they were shown and hits elsewhere,
    /// and nothing errors — it just misses, occasionally, in a way that reads as bad luck.
    ///
    /// Two drifts are realistic and both are checked. Hardcoding the SIZE instead of reading
    /// the `side` on the aimed skill's row in `commander_skills`, which would silently keep a 2×2
    /// preview after the soul-path node grew the blast to 3×3. And losing the PARITY offset, which
    /// is the whole
    /// even/odd rule — without it a 2×2 renders centred on the cursor while the sim hits down-right
    /// of it, so every cast lands one cell off in both axes.
    ///
    /// Reads the .gd at RUNTIME like the guards below: a moved Godot file must not fail the sim's
    /// BUILD. Missing skips; present but wrong fails.
    #[test]
    fn the_smite_preview_anchors_the_way_the_sim_does() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../godot/proto/arena3d/blast_fx_3d.gd"
        );
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("skip: {path} not present (packaged build?)");
            return;
        };
        // The OPEN PAREN matters: without it `sync_smite_aim_OLD` satisfies the prefix and a
        // renamed-away preview reads as present. Caught by falsifying this guard, not by review.
        let Some(body) = src.split("func sync_smite_aim(").nth(1) else {
            panic!("blast_fx_3d.gd no longer defines sync_smite_aim — the preview moved or went")
        };
        let body = body.split("\nfunc ").next().unwrap_or(body);
        assert!(
            body.contains("side % 2 == 0"),
            "sync_smite_aim lost its PARITY branch. The sim offsets an even-sided box by half a \n             cell (Blast::sided) and centres an odd one; without the same split here, a {}×{} \n             preview draws centred while the blast lands down-right of the cursor.",
            crate::events::COMMANDER_SMITE_SIDE,
            crate::events::COMMANDER_SMITE_SIDE,
        );
        assert!(
            body.contains("0.5") && body.contains("0.0"),
            "sync_smite_aim no longer offsets by HALF a cell — that half is the whole even-sided \n
             rule, and `grid::Blast` exists because of it",
        );
        // The SIZE must come from the sim, never be written here.
        let caller_path =
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../godot/proto/arena3d/arena3d.gd");
        if let Ok(caller) = std::fs::read_to_string(caller_path) {
            // The CALL, not the file. A first version searched for the bare name and passed on
            // the COMMENT that mentions it — a guard a comment can satisfy is not a guard.
            //
            // The read MOVED when the per-skill keys became the `commander_skills` catalog: the
            // side now comes off the aimed skill's entry instead of a `commander_smite_side` key.
            // Same requirement, new accessor — what is being guarded is that the number is READ,
            // not that it is read under one particular name.
            assert!(
                caller.contains(r#"state.get("commander_skills""#),
                "arena3d.gd no longer reads the `commander_skills` catalog off the view state — \n
                 without it there is nowhere for the footprint side to come from but a literal",
            );
            assert!(
                caller.contains(r#".get("side""#),
                "arena3d.gd no longer READS `side` off the aimed skill's entry — a hardcoded \n
                 side keeps a 2×2 preview after the soul-path node grows the blast to 3×3",
            );
        }
    }

    /// The client's `ZONES_PER_REGION` must equal the sim's.
    ///
    /// They disagreed for four months and nobody noticed: the client kept 3 from the 0.0.226 split
    /// while the world became 20x5 in 0.0.280, so `region_of(100)` answered 34 and every zone past
    /// 60 rendered under a region name that does not exist. Two constants in two languages, no
    /// compiler between them.
    ///
    /// Reads the .gd at RUNTIME, like `the_client_stalls_before_the_server_refuses_its_bundle` —
    /// the sim must not fail to BUILD because a Godot file moved. Missing file skips; present but
    /// unparseable fails, because the constant vanishing is exactly the drift worth catching.
    #[test]
    fn the_client_and_the_sim_agree_on_region_size() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../godot/views/overlay_view.gd");
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("skip: {path} not present (packaged build?)");
            return;
        };
        let line = src
            .lines()
            .map(str::trim)
            .find(|l| l.starts_with("const ZONES_PER_REGION"))
            .unwrap_or_else(|| panic!("overlay_view.gd no longer declares ZONES_PER_REGION"));
        let got: u8 = line
            .rsplit(':')
            .next()
            .and_then(|v| v.trim().trim_start_matches('=').trim().parse().ok())
            .unwrap_or_else(|| panic!("cannot read a number from {line:?}"));
        assert_eq!(
            got,
            crate::state::ZONES_PER_REGION,
            "the client groups zones {got} to a region, the sim {} — the client would name the \
             wrong region for every zone past the first disagreement",
            crate::state::ZONES_PER_REGION,
        );
    }

    /// A REGION is 5 zones, and the content agrees: the boss changes exactly at every region
    /// boundary, 20 times, with no exception.
    ///
    /// That alignment is the ONLY thing that makes "region" mean anything — it was never authored
    /// as a grouping, it is a pattern the data happens to hold. `region_of_level` now presents it
    /// to the player as the travel unit, so if a 6-zone region or a 21st boss block is ever
    /// authored, the word quietly starts describing something else: a player told "region 3" would
    /// be shown a span that does not share a boss.
    ///
    /// Checks BOTH directions — the boss must not change INSIDE a region either, or the five zones
    /// are not one place.
    #[test]
    fn regions_line_up_with_the_boss_blocks() {
        let data = super::defs::GameData::from_json_str(include_str!("../../../../data/game_data.json"))
            .expect("game_data.json must parse");
        let mut boss_at: std::collections::BTreeMap<u8, &str> = std::collections::BTreeMap::new();
        for z in &data.zones {
            if z.instance_start_zone_id == z.id && z.stage >= 1 && !z.boss_monster_id.is_empty() {
                boss_at.entry(z.stage as u8).or_insert(&z.boss_monster_id);
            }
        }
        assert_eq!(boss_at.len(), usize::from(crate::state::STAGES), "a boss per stage");

        let mut wrong: Vec<String> = Vec::new();
        for (&stage, boss) in &boss_at {
            let region = crate::state::region_of_stage(stage);
            if let Some(first) = boss_at.get(&crate::state::stage_of_region_index(region, 1)) {
                if first != boss {
                    wrong.push(format!("stage {stage} (region {region}) has {boss}, region starts with {first}"));
                }
            }
            if crate::state::stage_in_region(stage) == 1 && stage > 1 {
                if boss_at.get(&(stage - 1)) == Some(boss) {
                    wrong.push(format!("region {region} starts at stage {stage} but reuses the previous region's boss {boss}"));
                }
            }
        }
        assert!(
            wrong.is_empty(),
            "regions and boss blocks disagree — 'region' would name a span the player cannot \
             recognise:\n  {}",
            wrong.join("\n  "),
        );
        let regions: std::collections::BTreeSet<u8> =
            boss_at.keys().map(|&l| crate::state::region_of_stage(l)).collect();
        assert_eq!(regions.len(), 20, "expected 20 regions, found {}", regions.len());
        for z in &data.zones {
            assert!(z.stage >= 1 && z.stage <= crate::state::STAGES, "{}: every zone names its stage", z.id);
            if z.id.starts_with("zone_b") {
                assert_eq!(Some(z.stage as u8), crate::state::band_of_zone(&z.id), "{}: a ladder zone IS its stage", z.id);
            }
        }
    }

    /// The sim's fodder rule and the client's must agree over the REAL catalog.
    ///
    /// The sim asks `regalia_fodder_levels` (an exact id list, because it also needs to know how
    /// many levels the feed is worth). The client asks `ends_with("fodder")`, because a view has no
    /// business knowing the tier table. Two different questions that must give the same yes/no, in
    /// two languages, with nothing but this between them.
    ///
    /// They disagree the moment someone authors `regalia_fodder_epic` and forgets the sim list: the
    /// client greys it out of the collection as feed, the sim refuses to consume it, and the item
    /// exists in neither place a player can use.
    #[test]
    fn the_fodder_rule_matches_between_sim_and_client() {
        let data = super::defs::GameData::from_json_str(include_str!("../../../../data/game_data.json"))
            .expect("game_data.json must parse");
        let mut disagree: Vec<String> = Vec::new();
        let mut fodder = 0usize;
        for i in &data.items {
            let sim = crate::items::regalia_fodder_levels(&i.id) > 0;
            let client = i.id.ends_with("fodder");   // mirror of ItemNaming.is_regalia_fodder
            if sim {
                fodder += 1;
            }
            if sim != client {
                disagree.push(format!("{} — sim:{sim} client:{client}", i.id));
            }
        }
        assert!(
            disagree.is_empty(),
            "the sim and the client disagree about what is fodder:\n  {}\n Update \
             `items::regalia_fodder_levels` (which also sets what the tier is WORTH) and \
             `ItemNaming.is_regalia_fodder` together.",
            disagree.join("\n  "),
        );
        assert!(fodder >= 2, "expected at least the R and SR tiers, found {fodder}");
    }

    /// A RATCHET: no NEW `starts_with` / `ends_with` / `begins_with` on an id.
    ///
    /// Matching an id by shape instead of by identity is how a rename becomes a silent rule change.
    /// It cost a real one here — `refine.rs` decided from `starts_with("unique_")` whether a
    /// damaged piece needed the same unique to repair it, with nothing keeping the prefix and the
    /// `unique` flag in step. Naming the conventions fixed the sites that existed; this stops the
    /// next one being written.
    ///
    /// Not a ban — some of these are correct. It is an ALLOWLIST: a new site fails the build until
    /// someone writes down why it is not an id check. That is the whole mechanism; the reasoning is
    /// the point, not the count.
    ///
    /// Skips comments, tests, addons, and literals that are plainly file paths or punctuation.
    #[test]
    fn no_new_id_matching_by_string_shape() {
        // (file suffix, literal, why this one is allowed)
        const ALLOWED: &[(&str, &str, &str)] = &[
            ("sim-core/src/items.rs", "unique_", "used ONLY by the_unique_flag_and_the_unique_prefix_agree, which exists to compare the prefix against the flag"),
            ("sim-core/src/data/mod.rs", "fodder", "the_fodder_rule_matches_between_sim_and_client MIRRORS the client rule on purpose, to compare the two"),
            ("views/item_naming.gd", "fodder", "client mirror of items::is_regalia_fodder"),
            ("views/overlay_view.gd", "boss_", "parses a band number out of a zone id — degrades to a wrong label"),
            ("views/overlay_view.gd", "mob_", "same"),
            ("views/overlay_view.gd", "b", "same"),
            ("views/world_map_view.gd", "b", "same"),
            ("views/card_panel_view.gd", "Accessory", "reads a SLOT name, not an id"),
            ("sim-host/src/session.rs", "stats:", "splits a QUERY KEY on its parameter separator — `stats:7` is this host's wire format for \"the sheet for entity 7\", not a game-data id. The suffix is parsed as a u32 and a miss answers {}"),
            ("sim-host/src/session.rs", "combat_skills:", "same parameterized query protocol: the suffix is an entity u32 selecting its live skill list; parsed and a miss answers []"),
            ("sim-host/src/session.rs", "engram_preview:", "same parameterized query protocol: the suffix is a roster unit u32; parsed and a miss answers null"),
            ("sim-host/src/session.rs", "potential:", "same parameterized query protocol: an entity u32 selects its board; this does not infer any class, node or item type from an id's spelling"),
            ("sim-host/src/session.rs", "card_slots:", "same wire format, same parse, same empty answer on a miss"),
            ("sim-host/src/session.rs", "refine_plan:", "same wire format; the suffix addresses a bag index or an (entity, slot) pair, both parsed as integers"),
            ("sim-host/src/session.rs", "orb_plan:", "same wire format again; the suffix is a bag index for the STONE, parsed as usize, and a miss answers {}"),
            ("sim-host/src/session.rs", "bind_info:", "same wire format once more; the suffix is an ALBUM index, parsed as usize, and a miss answers null"),
            ("sim-host/src/session.rs", "gem_plan:", "same wire format: the suffix is a bag index (`inv:`) or an entity id + worn index (`eq:`) parsed as u32, handed to `gems::refusal`, and anything else answers {}"),
            ("sim-host/src/session.rs", "disenchant_preview:", "same wire format: the suffix is a bag index parsed as usize, handed to `crest::material_of`, and anything else answers null"),
            ("sim-host/src/session.rs", "fuse_preview:", "same wire format: the suffix is six BAG indices parsed as u32, handed to `fusion::preview`, and anything else answers {}"),
            ("sim-host/src/session.rs", "orb_fits:", "the other direction of that key — its suffix is a TARGET id (`inv:<i>` or `<entity>:<slot>`) matched against ids this same function built, never against game data"),
            ("sim-host/src/session.rs", "inv:", "the location half of that same wire format — bag vs worn, which is the sim's own split between StartRefineInventory and StartRefine"),
            ("views/dialogue_script.gd", "#", "comment marker in the dialogue format"),
            ("views/soul_web.gd", "%", "a unit suffix on a formatted effect string"),
            ("views/store_view.gd", "Sandbox", "Steam's own environment name, not our id"),
            ("views/chat_view.gd", "advance:", "a chat command prefix, which IS a string protocol"),
            ("views/options_view.gd", "th", "a locale code"),
            ("views/arena_draw.gd", "b", "parses a band number out of a zone id"),
            ("views/debug_view.gd", "advance:", "a debug command prefix — a string protocol"),
            ("views/card_panel_view.gd", "\u{1}", "an in-band SOH marker on a formatted line, not an id"),
            ("verify-server/src/main.rs", "Sandbox", "Steam's own environment name"),
            ("verify-server/src/main.rs", "const SPAN_STALL_TICKS", "reads the .gd source in the cross-language guard"),
            ("sim-core/src/data/mod.rs", "#", "this guard's OWN comment-skipping code"),
            ("sim-core/src/data/mod.rs", "const ZONES_PER_REGION", "reads the .gd source in the cross-language region guard"),
            ("sim-core/src/data/mod.rs", "contains", "this guard inspecting its own method list"),
            ("sim-core/src/data/mod.rs", "id", "this guard testing whether a contains() receiver is an id"),
            ("fx/pack_vfx.gd", ":light_energy", "an AnimationPlayer TRACK PATH, whose format is `NodePath:property` — this matches Godot's own path grammar to find every light-energy track in a pack scene, and there is no id anywhere near it"),
            ("fx/pack_vfx.gd", "depth_test", "a shader `render_mode` TOKEN. Godot's render_mode vocabulary is a fixed family (`depth_test_disabled`, `depth_draw_never`, ...) and it REJECTS a redefinition, so the existing members have to be stripped by prefix before the replacements are appended"),
            ("fx/pack_vfx.gd", "depth_draw", "same render_mode family as above"),
            ("fx/_pack_probe.gd", ":light_energy", "the dev probe reading the SAME AnimationPlayer track paths back, to verify the light scaler did what it claimed"),
            ("tools/export_arena_tables.gd", "EditorGround", "a NODE NAME in the editing scene, not an id: the exporter walks a .tscn and skips the placeholder ground the artist works over, which exists only in the editor and is named by hand"),
        ];

        let mut offenders: Vec<String> = Vec::new();
        let mut roots: Vec<std::path::PathBuf> = Vec::new();
        roots.push(std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../")));
        roots.push(std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../godot")));

        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            let Ok(rd) = std::fs::read_dir(dir) else { return };
            for e in rd.flatten() {
                let p = e.path();
                let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if p.is_dir() {
                    // never our code: build output, the editor cache, third-party addons
                    if matches!(name, "target" | ".godot" | "addons" | ".git" | "tests") {
                        continue;
                    }
                    walk(&p, out);
                } else if matches!(p.extension().and_then(|s| s.to_str()), Some("rs") | Some("gd"))
                    && !name.contains("test")
                {
                    out.push(p);
                }
            }
        }
        let mut files = Vec::new();
        for r in &roots {
            walk(r, &mut files);
        }
        assert!(files.len() > 50, "walked only {} files — the guard is looking in the wrong place", files.len());

        for f in &files {
            let Ok(src) = std::fs::read_to_string(f) else { continue };
            let path = f.to_string_lossy().replace('\\', "/");
            // STOP AT THE TEST MODULE. The walker above already exempts test code — it skips any
            // file whose NAME contains "test" — but that is file granularity, and verify-server
            // keeps `mod tests` / `mod pg_integration` INLINE in main.rs. So a test that scans
            // Rust source for function boundaries (`starts_with("async fn ")`) was read as an
            // attempt to identify a catalog item by the shape of its id.
            //
            // Same exemption, applied consistently — not a widening. What the guard defends is
            // the SHIPPED catalog: game logic must not decide what an item IS from the letters in
            // its id. A test asserting on source text cannot reach the catalog. `verify-server`'s
            // own custody scanner already truncates at `mod pg_integration` for this exact reason,
            // so the idiom is the codebase's, not this guard's invention.
            //
            // Production code AFTER a test module would be skipped. Nothing in this tree does
            // that, and the convention here is tests last.
            let src = &src[..src.find("#[cfg(test)]").unwrap_or(src.len())];
            for (n, line) in src.lines().enumerate() {
                let t = line.trim_start();
                if t.starts_with("//") || t.starts_with("#") {
                    continue; // a comment quoting the pattern is not a use of it
                }
                // `contains` belongs here too: the ore/junk filter had a `contains("stone")`
                // sitting between two prefix checks and the first version of this guard walked
                // straight past it.
                for m in ["starts_with(\"", "ends_with(\"", "begins_with(\"", "contains(\""] {
                    let mut from = 0;
                    while let Some(i) = line[from..].find(m) {
                        let lit_start = from + i + m.len();
                        let Some(end) = line[lit_start..].find('"') else { break };
                        let lit = &line[lit_start..lit_start + end];
                        let recv_end = from + i;
                        from = lit_start + end;
                        // `contains` is overwhelmingly used on MESSAGES (an assertion checking an
                        // error string), so scanning it blind buries the signal and the guard gets
                        // switched off. Only flag it when the RECEIVER is an id — which is what
                        // `row.item_id.contains("stone")` was.
                        if m.starts_with("contains") {
                            let recv: String = line[..recv_end.saturating_sub(1)]
                                .chars()
                                .rev()
                                .take_while(|c| c.is_alphanumeric() || *c == '_')
                                .collect();
                            let recv: String = recv.chars().rev().collect();
                            if !recv.to_ascii_lowercase().ends_with("id") {
                                continue;
                            }
                        }
                        // plainly not an id: a file path, an extension, a URL
                        if lit.starts_with('.') || lit.contains('/') || lit.contains("://") {
                            continue;
                        }
                        if ALLOWED.iter().any(|(fs, l, _)| path.ends_with(fs) && *l == lit) {
                            continue;
                        }
                        offenders.push(format!("{}:{} — {m}{lit}\")", path.rsplit("idle_mmorpg/").next().unwrap_or(&path), n + 1));
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "new id-matching by string shape:\n  {}\n\nMatch on the id or the TYPE, or call a named \
             predicate (`items::is_unique_def_id`, `items::is_regalia_fodder`). If this one is not \
             an id check, add it to ALLOWED above WITH the reason — the reason is the point.",
            offenders.join("\n  "),
        );
    }

    /// The `unique` FLAG and the `unique_` PREFIX must agree, in both directions.
    ///
    /// `reforge_matches` decides from the id whether a damaged piece needs the SAME unique to
    /// repair it or merely a comparable one. It reads the id because it runs on an `Item` instance,
    /// which does not carry the flag and cannot reach the catalog — so the convention IS the rule.
    ///
    /// Both directions are a live gameplay change and neither errors:
    /// - flagged but not prefixed → a unique repairable with any ordinary piece of the same slot
    ///   and tier, which is the whole scarcity of a unique gone.
    /// - prefixed but not flagged → an ordinary item that can only be repaired by another copy of
    ///   itself, which for a non-unique is usually impossible.
    ///
    /// 7 and 7 today. Nothing but this keeps them that way.
    #[test]
    fn the_unique_flag_and_the_unique_prefix_agree() {
        let data = super::defs::GameData::from_json_str(include_str!("../../../../data/game_data.json"))
            .expect("game_data.json must parse");
        let mut flagged_not_prefixed: Vec<&str> = Vec::new();
        let mut prefixed_not_flagged: Vec<&str> = Vec::new();
        for i in &data.items {
            match (i.unique, crate::items::is_unique_def_id(&i.id)) {
                (true, false) => flagged_not_prefixed.push(&i.id),
                (false, true) => prefixed_not_flagged.push(&i.id),
                _ => {}
            }
        }
        assert!(
            flagged_not_prefixed.is_empty() && prefixed_not_flagged.is_empty(),
            "`unique` and the `unique_` prefix disagree — reforge reads the PREFIX, so this \
             silently changes what can repair what.\n  flagged, not prefixed: {flagged_not_prefixed:?}\
             \n  prefixed, not flagged: {prefixed_not_flagged:?}",
        );
        // 0.0.490: the seven unique pieces left the game (user: "พวกนี้ไม่ใช้แล้วเอาออกจากเกม"), so
        // the catalog holds none today and this guard only proves nothing carries the prefix
        // without the flag. It bites again the day a unique is authored.
    }

    /// Every unit has a `CLASS_NAME_<id>` row, in EN and TH (the key kept its historical prefix
    /// when the class system went; both clients read it).
    ///
    /// The name a player reads used to be DERIVED from the id — strip `_tN`, capitalise — which
    /// meant the id was the display string, so Thai players read "Swordsman" and the only way to
    /// change what was on screen was to change the id, which keys the roster. Splitting them is
    /// what lets the name be free and the id be frozen.
    ///
    /// The lookup is key-only with no fallback (the project rule — a missing key renders the
    /// key), so a unit added without its row shows `CLASS_NAME_x` on a button. Loud, but only if
    /// someone runs the game and looks; this makes it a red test instead.
    #[test]
    fn every_unit_has_a_display_name_in_both_languages() {
        let csv_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../godot/i18n/strings.csv");
        let Ok(csv) = std::fs::read_to_string(csv_path) else {
            eprintln!("skip: {csv_path} not present (packaged build?)");
            return;
        };
        let data = super::defs::GameData::from_json_str(include_str!("../../../../data/game_data.json"))
            .expect("game_data.json must parse");
        assert!(!data.units.is_empty(), "no units — nothing checked");

        let mut bad: Vec<String> = Vec::new();
        for c in &data.units {
            let key = format!("CLASS_NAME_{}", c.id);
            // columns: keys,flags,en,th,...
            let Some(row) = csv.lines().find(|l| l.starts_with(&format!("{key},"))) else {
                bad.push(format!("{key} — no row at all"));
                continue;
            };
            let cols: Vec<&str> = row.split(',').collect();
            let en = cols.get(2).copied().unwrap_or("").trim();
            let th = cols.get(3).copied().unwrap_or("").trim();
            if en.is_empty() || th.is_empty() {
                bad.push(format!("{key} — en={en:?} th={th:?}"));
            }
        }
        assert!(
            bad.is_empty(),
            "units with no display name:\n  {}\n Add the row to godot/i18n/strings.csv — \
             the lookup has no fallback, so the player sees the raw key on a button.",
            bad.join("\n  "),
        );
    }

    /// Read every mission's `(id, requires)` off disk. `None` when the tree is absent.
    #[cfg(test)]
    fn story_chain() -> Option<Vec<(String, String, String)>> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../godot/story");
        let entries = std::fs::read_dir(dir).ok()?;
        let mut out = Vec::new();
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|s| s.to_str()) != Some("dialogue") {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(&path) else { continue };
            let file = path.file_name().and_then(|s| s.to_str()).unwrap_or("?").to_string();
            let head = |k: &str| -> String {
                let pat = format!("% {k} ");
                src.lines()
                    .map(str::trim)
                    .find_map(|l| l.strip_prefix(&pat).map(|v| v.trim().to_string()))
                    .unwrap_or_default()
            };
            out.push((head("id"), head("requires"), file));
        }
        Some(out)
    }

    /// Every `% requires` names a mission that EXISTS, and the chain has no cycle.
    ///
    /// Neither failure errors. A locked mission renders as a disabled row, which is exactly what a
    /// legitimately-not-yet-reached mission looks like — so a broken edge is indistinguishable
    /// from ordinary progress, and stays that way forever:
    ///
    /// - `requires` naming an id nothing declares (a typo, or a file renamed without updating the
    ///   mission that follows it) locks that mission FOREVER, because the id can never be cleared.
    ///   Its tooltip names a mission that does not exist, which is the only visible trace.
    /// - A cycle locks every mission in it forever, for the same reason.
    ///
    /// Renaming is exactly what this file family invites — `% id` exists so a file CAN be renamed
    /// — and the edge lives in a different file from the id it names. Nothing else binds them.
    #[test]
    fn every_story_requires_names_a_real_mission() {
        let Some(chain) = story_chain() else {
            eprintln!("skip: godot/story not present (packaged build?)");
            return;
        };
        assert!(!chain.is_empty(), "no .dialogue files found — this guard checked nothing");

        let ids: Vec<&str> = chain.iter().map(|(id, _, _)| id.as_str()).collect();
        let mut dangling: Vec<String> = Vec::new();
        for (id, needs, file) in &chain {
            if !needs.is_empty() && !ids.contains(&needs.as_str()) {
                dangling.push(format!("{file}: '{id}' requires '{needs}', which no mission declares"));
            }
        }
        assert!(
            dangling.is_empty(),
            "broken unlock chain:\n  {}\n A mission whose prerequisite does not exist can never \
             be unlocked, and the only symptom is that it is missing from the list.",
            dangling.join("\n  "),
        );

        // Walk from every mission to a root. A chain of N missions cannot take more than N steps;
        // more than that means it came back on itself.
        for (id, _, file) in &chain {
            let mut at = id.as_str();
            for _ in 0..=chain.len() {
                let needs = chain.iter().find(|(i, _, _)| i == at).map(|(_, n, _)| n.as_str()).unwrap_or("");
                if needs.is_empty() {
                    break;
                }
                at = needs;
            }
            let still_going = chain
                .iter()
                .find(|(i, _, _)| i == at)
                .map(|(_, n, _)| !n.is_empty())
                .unwrap_or(false);
            assert!(
                !still_going,
                "{file}: the unlock chain from '{id}' never reaches a root — a cycle hides every \
                 mission in it, permanently and silently",
            );
        }

        // At least one root, or nothing is ever playable on a fresh account.
        assert!(
            chain.iter().any(|(_, needs, _)| needs.is_empty()),
            "no story mission is available from the start — every one declares a `% requires`, so \
             a new account sees an empty list forever",
        );
    }

    /// Every story mission declares an `% id`, and no two declare the same one.
    ///
    /// The id keys `story_claims (account_id, mission_id)` — the row that makes a first-clear
    /// reward once-only. Two failure modes, one guard, because they are the same decision:
    ///
    /// MISSING: there is no filename fallback (there was, briefly, and it was the bug in quieter
    /// form — a default that always produces a plausible key and is wrong exactly when the file is
    /// renamed, which is the rename the story-mode plan calls for). A mission with no id has no
    /// reward key, and the client refuses to play it.
    ///
    /// DUPLICATE: two missions on one id share one row, so whichever is played first pays and the
    /// other reports "already claimed" forever — a symptom indistinguishable from the honest case.
    /// The filesystem used to enforce uniqueness for free; that is the guarantee an explicit id
    /// trades away in exchange for surviving a rename, so it has to be bought back here.
    #[test]
    fn every_story_mission_declares_a_unique_id() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../godot/story");
        let Ok(entries) = std::fs::read_dir(dir) else {
            eprintln!("skip: {dir} not present (packaged build?)");
            return;
        };

        let mut ids: Vec<(String, String)> = Vec::new();   // (declared id, file it came from)
        let mut undeclared: Vec<String> = Vec::new();
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|s| s.to_str()) != Some("dialogue") {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(&path) else { continue };
            let file = path.file_name().and_then(|s| s.to_str()).unwrap_or("?").to_string();
            match src
                .lines()
                .map(str::trim)
                .find_map(|l| l.strip_prefix("% id ").map(|v| v.trim().to_string()))
                .filter(|v| !v.is_empty())
            {
                Some(id) => ids.push((id, file)),
                None => undeclared.push(file),
            }
        }

        assert!(
            !ids.is_empty() || !undeclared.is_empty(),
            "no .dialogue files found in {dir} — this guard checked nothing"
        );
        assert!(
            undeclared.is_empty(),
            "story mission(s) declare no `% id`: {}\nAn id is the key its reward is paid against; \
             it cannot be inferred from the filename without making a rename pay twice.",
            undeclared.join(", "),
        );
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(
                    ids[i].0, ids[j].0,
                    "'{}' and '{}' both declare mission id '{}' — they would share one \
                     story_claims row, so only the first one played would ever pay.",
                    ids[i].1, ids[j].1, ids[i].0,
                );
            }
        }
    }
}

/// One element NAME from the catalog, or `None` when it names nothing. Used by the
/// weakness lists, where an unrecognised name must vanish rather than default —
/// see `build::zone::parse_elements`.
pub fn parse_element_opt(name: &str) -> Option<crate::grid::Element> {
    use crate::grid::Element as E;
    Some(match name {
        "Neutral" | "Physical" => E::Physical,
        "Fire" => E::Fire,
        "Earth" => E::Earth,
        "Ice" | "Water" => E::Ice,
        "Wind" => E::Wind,
        "Light" | "Holy" => E::Light,
        "Dark" | "Shadow" => E::Dark,
        "Poison" => E::Poison,
        "Thunder" => E::Thunder,
        "Spirit" => E::Spirit,
        _ => return None,
    })
}
