// SimBridge — class / zone / party methods.
//
// A SECONDARY `#[godot_api]` impl block (gdext 0.2.4 allows one primary +
// any number of `(secondary)` blocks on the same #[derive(GodotClass)] type,
// in any module of the crate). Splitting the methods this way keeps each file
// under 500 lines without merging unrelated `#[func]`s into one giant block.

// `use super::*` glob-imports the crate-root items the gdext `(secondary)`
// macro needs in scope: the `SimBridge` struct, its funcs-collection struct,
// and the module-local `__registration_*` statics the primary impl declares.
#[allow(unused_imports)]
use super::*;
use sim_core::{data, events};

#[godot_api(secondary)]
impl SimBridge {
    /// Character creation: (re)build the game as a SOLO lead of `unit_def`
    /// with the chosen `name`. Stats are the class's fixed base. Requires
    /// `init_game` first (to load game data). Returns false on unknown
    /// class/zone or if data isn't loaded.
    #[func]
    fn create_character(
        &mut self,
        seed: i64,
        zone_id: GString,
        unit_def: GString,
        name: GString,
    ) -> bool {
        let Some(gd) = self.data.as_ref() else { return false; };
        let zid = zone_id.to_string();
        let Ok(mut st) = data::new_game_as(seed as u64, gd, &zid, &unit_def.to_string()) else {
            return false;
        };
        // `name` is the PARTY name (the persistent identity; there's no fixed lead). It is
        // NOT baked into the start snapshot — it's a recorded SetPartyName event (below), so
        // the bundle's `start` stays the server-reproducible genesis (`new_game_as`,
        // name-free) and the online anchor stays valid.
        // `state.current_zone_id` is already set by new_game_as → populate_zone.
        sim_host::session::arm_recording(&mut st); // S1/S1c: input log + snapshot chain.
        self.state = Some(st);
        self.session_start = self.state.clone(); // S1b: name-free genesis anchor.
        events::apply_input(
            self.state.as_mut().unwrap(),
            events::InputEvent::SetPartyName { name: name.to_string() },
        );
        true
    }

    /// JSON array of selectable classes with their base stats, for the
    /// character-creation stat chart: `[{id, str, agi, vit, int, dex, luk}]`.
    /// Order follows game_data. Empty array if data isn't loaded.
    #[func]
    fn classes_json(&self) -> GString {
        let Some(gd) = self.data.as_ref() else { return GString::from("[]"); };
        let arr: Vec<serde_json::Value> = gd
            .creatable_units()
            .iter()
            .map(|c| {
                serde_json::json!({ "id": c.id })
            })
            .collect();
        GString::from(serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string()))
    }

    /// Current lead unit id (e.g. "seraphine"). Empty string if
    /// no game is loaded. GDScript uses this to pick the correct
    /// sprite sheet each frame.
    #[func]
    fn current_class(&self) -> GString {
        match self.state.as_ref().and_then(|s| s.current_unit_def.as_ref()) {
            Some(s) => s.clone().into(),
            None => GString::default(),
        }
    }

    /// The PARTY name (the persistent identity). Falls back to `character_name`
    /// for saves made before the party_name field existed. Empty if no game.
    #[func]
    fn party_name(&self) -> GString {
        match self.state.as_ref() {
            Some(s) if !s.party_name.is_empty() => s.party_name.clone().into(),
            Some(s) => s.character_name.clone().into(),
            None => GString::default(),
        }
    }

    /// Swap the player to a different class. Replaces base stats
    /// (hp / max_hp / atk / def / element / speed / atk_speed) from the
    /// target class definition; inventory and equipped items carry
    /// over so the player keeps their build. Returns false if the
    /// class id is unknown or there's no active game.
    ///
    /// EVENTED (S0 replay-hole fix): routes through `InputEvent::ChangeClass`
    /// so the swap is recorded in the input log and replays bit-identically.
    /// The mutation itself lives in `sim_core::data::change_class`.
    #[func]
    fn set_player_class(&mut self, unit_def: GString) -> bool {
        let id = unit_def.to_string();
        let Some(d) = self.data.as_ref() else { return false; };
        // Reject an unknown class up front so the UI still gets a false
        // (the event handler would otherwise no-op silently).
        if d.unit(&id).is_none() {
            return false;
        }
        let Some(s) = self.state.as_mut() else { return false; };
        events::apply_input_with(
            s,
            self.data.as_ref(),
            events::InputEvent::ChangeClass { unit_def: id },
        );
        true
    }

    /// Player respawn: reset the current zone (clear monsters, refill player HP,
    /// place at zone.player_spawn). Loot escrow is KEPT. EVENTED via
    /// `InputEvent::RespawnInZone` (reads `state.current_zone_id`). Returns false
    /// if there's no active game / current zone.
    #[func]
    fn respawn_in_current_zone(&mut self) -> bool {
        let Some(s) = self.state.as_mut() else { return false; };
        if s.current_zone_id.is_none() {
            return false;
        }
        events::apply_input_with(s, self.data.as_ref(), events::InputEvent::RespawnInZone);
        true
    }

    /// Party wipe → restart the CURRENT INSTANCE at room 1 (clears the loot
    /// escrow; EXP/job/soul survive). EVENTED via `InputEvent::RestartInstance`
    /// (`handle_party_wipe` with auto_retreat=false). `change_zone` restamps
    /// `state.current_zone_id`. Returns false only with no active game.
    #[func]
    fn restart_instance(&mut self) -> bool {
        if self.data.is_none() {
            return false;
        }
        let Some(s) = self.state.as_mut() else { return false; };
        events::apply_input_with(s, self.data.as_ref(), events::InputEvent::RestartInstance);
        true
    }

    /// Leave a CLEARED dungeon: return to the zone the party entered from and dismiss
    /// the clear summary. EVENTED via `InputEvent::LeaveInstance`. The destination is
    /// the SIM's (`state.return_zone_id`) — this call names no zone, so the client
    /// cannot use it to travel anywhere else. Returns false only with no active game.
    #[func]
    fn leave_instance(&mut self) -> bool {
        if self.data.is_none() {
            return false;
        }
        let Some(s) = self.state.as_mut() else { return false; };
        events::apply_input_with(s, self.data.as_ref(), events::InputEvent::LeaveInstance);
        true
    }

    /// The id of the zone the player is currently in (e.g. "zone_b3_1"). Empty
    /// before the game starts. The UI derives the current BAND from this. Reads
    /// the authoritative `state.current_zone_id`.
    #[func]
    fn current_zone(&self) -> GString {
        let id = self
            .state
            .as_ref()
            .and_then(|s| s.current_zone_id.clone())
            .unwrap_or_default();
        GString::from(id)
    }

    /// Warp to the start (room 1) of `band`'s instance — the band picker. Clears
    /// the held loot escrow (leaving the current instance), keeps XP/job/soul.
    /// EVENTED via `InputEvent::SelectBand`. Returns false if the band is out of
    /// range / its zone is unknown (state left untouched — the up-front zone check
    /// avoids clearing escrow for an invalid band).
    #[func]
    fn select_band(&mut self, band: i64) -> bool {
        if band < 1 {
            return false;
        }
        let Some(d) = self.data.as_ref() else { return false; };
        if d.zone(&format!("zone_b{band}_1")).is_none() {
            return false;
        }
        let Some(s) = self.state.as_mut() else { return false; };
        events::apply_input_with(
            s,
            self.data.as_ref(),
            events::InputEvent::SelectBand { band: band as u32 },
        );
        true
    }

    /// Hire a new party member of `unit_def`, paid in Gil. Returns true
    /// on success; false if the party is full, the class is unknown, or
    /// the party can't afford it (all leave state untouched). The new
    /// member spawns adjacent to the lead in the current zone.
    #[func]
    fn recruit_member(&mut self, unit_def: GString) -> bool {
        if self.data.is_none() {
            return false;
        }
        let Some(s) = self.state.as_mut() else { return false; };
        // EVENTED (S0): route through InputEvent; success = the party grew.
        let before = data::party_size(s);
        events::apply_input_with(
            s,
            self.data.as_ref(),
            events::InputEvent::RecruitMember { unit_def: unit_def.to_string() },
        );
        data::party_size(s) > before
    }

    /// Gil price of the NEXT recruit (the member that grows the party by
    /// one). Returns -1 when the party is already full, so the UI can
    /// gray out the recruit button.
    #[func]
    fn recruit_cost(&self) -> i64 {
        // RETIRED (0.0.413): recruiting for Gil is gone — the frozen 2D client keeps its button
        // greyed out. Party SLOTS are what Gil buys now (`party_slot_cost`).
        -1
    }

    /// Gil price of the NEXT party slot (0.0.413), -1 when none is left to buy.
    #[func]
    fn party_slot_cost(&self) -> i64 {
        match (self.state.as_ref(), self.data.as_ref()) {
            (Some(s), Some(d)) => data::party_slot_cost(s, d).map_or(-1, |c| c as i64),
            _ => -1,
        }
    }


    // CLASS ADVANCEMENT — RETIRED (0.0.352). `advance_options_json`, `advance_options_for_class_json`
    // and `advance_class` are gone with the mechanic; the Godot advancement view is removed.

    // --- Roster ⇄ Party (buy / deploy / bench) ---------------------------

    /// Buy a new character of `unit_def`, paid in Gil, added BENCHED to the
    /// roster (NOT auto-deployed). Returns the new char's unique id, or -1 on
    /// failure (unknown class / not enough Gil — state untouched).
    #[func]
    fn buy_character(&mut self, unit_def: GString) -> i64 {
        if self.data.is_none() {
            return -1;
        }
        let Some(s) = self.state.as_mut() else { return -1; };
        // EVENTED (S0): the event discards buy_character's returned id, so recover
        // it as the roster key that appeared. -1 = purchase failed (state untouched).
        let before: std::collections::BTreeSet<u32> = s.roster.keys().copied().collect();
        events::apply_input_with(
            s,
            self.data.as_ref(),
            events::InputEvent::BuyCharacter { unit_def: unit_def.to_string() },
        );
        s.roster
            .keys()
            .copied()
            .find(|k| !before.contains(k))
            .map_or(-1, |k| k as i64)
    }

    /// Gil price of the NEXT character purchase (grows OWNED by one). Always a
    /// value (count is uncapped); the per-char price caps at 10M.
    #[func]
    fn buy_character_cost(&self) -> i64 {
        // RETIRED (0.0.413): characters come from gacha; -1 greys the frozen client's button.
        -1
    }

    /// Deploy a benched character (`char_id`) into the party. Returns false if
    /// the party is full or the id isn't in the roster (state untouched).
    #[func]
    fn deploy_character(&mut self, char_id: i64) -> bool {
        if self.data.is_none() {
            return false;
        }
        let Some(s) = self.state.as_mut() else { return false; };
        // EVENTED (S0): success = the party grew (roster → grid).
        let before = data::party_size(s);
        events::apply_input_with(
            s,
            self.data.as_ref(),
            events::InputEvent::DeployCharacter { char_id: char_id as u32 },
        );
        data::party_size(s) > before
    }

    /// Bench a deployed party member (`char_id`). Returns false if it's the lead,
    /// the last member, or not deployed (state untouched).
    #[func]
    fn bench_character(&mut self, char_id: i64) -> bool {
        let Some(s) = self.state.as_mut() else { return false; };
        // EVENTED (S0): success = the party shrank (grid → roster). bench needs no
        // catalog, but routes through the same entry point for uniformity.
        let before = data::party_size(s);
        events::apply_input_with(
            s,
            self.data.as_ref(),
            events::InputEvent::BenchCharacter { char_id: char_id as u32 },
        );
        data::party_size(s) < before
    }

    /// JSON array of BENCHED characters for the roster UI — `[{id, unit_def,
    /// level, job_level}]`, in id order (BTreeMap = deterministic). Deployed
    /// members come from `view_state` (the grid party); this is the bench.
    #[func]
    fn roster_json(&self) -> GString {
        let Some(s) = self.state.as_ref() else { return GString::default(); };
        let list: Vec<serde_json::Value> = s
            .roster
            .iter()
            .map(|(id, rc)| {
                serde_json::json!({
                    "id": id,
                    "unit_def": rc.unit_def,
                    "level": rc.data.stats.level,
                    "apex_level": rc.data.stats.apex_level,
                    // What a refund would hand back (0.0.349) — the sim's own figure.
                    "refund_xp": sim_core::tick::cum_base(&rc.data.stats),
                })
            })
            .collect();
        serde_json::to_string(&list).unwrap_or_default().into()
    }

    /// DEV: fabricate an end-of-run summary so the clear/fail screen can be SEEN without a
    /// dungeon that drops anything. The gothic mobs have no client-side loot tables yet (see
    /// TODO.md), so a real run always reports an empty list — indistinguishable from a broken UI.
    /// Borrows up to 5 items from the stash purely so the list renders real names.
    #[func]
    fn debug_fake_clear(&mut self, failed: bool) -> bool {
        let Some(s) = self.state.as_mut() else { return false };
        let items: Vec<sim_core::items::Item> = s.inventory.iter().take(5).cloned().collect();
        s.last_clear = Some(sim_core::state::ClearSummary {
            instance_id: "zone_dungeon_duskmourn_1".to_string(),
            ticks: 3_600,
            kills: 42,
            xp: 12_345,
            gil: 6_789,
            loot: items,
            failed,
        });
        true
    }

    /// DEBUG (dev tool, not a recorded event): set `entity_id`'s HP to `pct`% of
    /// its max — e.g. drop a thief below 30% to test Hiding. Returns true on hit.
    #[func]
    /// Refuse a direct-mutation cheat while online, and say so. See `SimBridge::debug_allowed`.
    fn debug_blocked(&self) -> bool {
        if self.debug_allowed {
            return false;
        }
        godot_warn!(
            "[debug] cheat refused — ONLINE. Direct state edits are not recorded in the input \
             log, so the save would stop verifying and every push after it would be rejected."
        );
        true
    }

    #[func]
    /// ONLINE gate for the direct-mutation cheats. Set by SimDriver at login.
    fn set_debug_allowed(&mut self, allowed: bool) {
        self.debug_allowed = allowed;
    }

    #[func]
    fn debug_set_hp_pct(&mut self, entity_id: i64, pct: i64) -> bool {
        if self.debug_blocked() { return false; }
        let Some(s) = self.state.as_mut() else { return false; };
        let Some(i) = s.grid.idx_of_id(entity_id as u32) else { return false; };
        let maxhp = sim_core::combat::effective_max_hp(&s.grid.entities[i]) as i64;
        s.grid.entities[i].hp = (maxhp * pct.clamp(0, 100) / 100).max(1) as i32;
        true
    }

    /// DEBUG (dev tool): instantly DOWN `entity_id` — sets HP to 0 and clears
    /// `alive`, exactly like a killing blow. Use it to test the revive path
    /// (Bishop Resurrection / Phoenix Tail) and full-party WIPE handling, which
    /// `debug_set_hp_pct` can't reach (it floors at 1 HP). Returns true on hit.
    #[func]
    fn debug_kill(&mut self, entity_id: i64) -> bool {
        if self.debug_blocked() { return false; }
        let Some(s) = self.state.as_mut() else { return false; };
        let Some(i) = s.grid.idx_of_id(entity_id as u32) else { return false; };
        s.grid.entities[i].hp = 0;
        s.grid.entities[i].alive = false;
        true
    }

    /// DEBUG (dev tool): inflict the full spread of status ailments on `entity_id`
    /// so the Bishop's Purificatio cleanse can be tested (statuses normally only run
    /// player→enemy). Applies one of each gate tier — Stun / Freeze / Blind /
    /// Poison(classic) / Slow(%) — PLUS the two cleanse-IMMUNE ones (Deadly Poison
    /// = PoisonDot, Bleed) so you can confirm those survive. 30 s each.
    #[func]
    fn debug_inflict_statuses(&mut self, entity_id: i64) -> bool {
        use sim_core::skills::{Buff, BuffKind};
        let Some(s) = self.state.as_mut() else { return false; };
        let Some(i) = s.grid.idx_of_id(entity_id as u32) else { return false; };
        let e = &mut s.grid.entities[i];
        for (kind, mag) in [
            (BuffKind::Stun, 1),
            (BuffKind::Freeze, 1),
            (BuffKind::Blind, 1),
            (BuffKind::Poison, 1),
            (BuffKind::AgiDexSlowPct, 30),
            (BuffKind::PoisonDot, 50), // Deadly Poison — cleanse-immune
            (BuffKind::Bleed, 50),     // cleanse-immune
        ] {
            e.buffs.retain(|b| b.kind != kind); // refresh, don't stack
            e.buffs.push(Buff { kind, magnitude: mag, ticks: 300, source_id: 0 });
        }
        true
    }

    /// DEBUG: toggle GOD MODE — party invincible (full HP/SP, revived, no
    /// cooldowns) and one-shotting. Persists in the save while on.
    #[func]
    fn debug_set_god(&mut self, on: bool) -> bool {
        let Some(s) = self.state.as_mut() else { return false; };
        s.debug_god = on;
        true
    }

    /// DEBUG: is god mode currently on? (for the menu's toggle state)
    #[func]
    fn debug_god(&self) -> bool {
        self.state.as_ref().map_or(false, |s| s.debug_god)
    }

    /// DEBUG: set a member's BASE level (resets XP toward the next).
    #[func]
    fn debug_set_level(&mut self, entity_id: i64, level: i64) -> bool {
        if self.debug_blocked() { return false; }
        let Some(s) = self.state.as_mut() else { return false; };
        let Some(i) = s.grid.idx_of_id(entity_id as u32) else { return false; };
        s.grid.entities[i].stats.level = level.clamp(1, 999) as u32;
        s.grid.entities[i].stats.xp = 0;
        true
    }

    /// DEBUG: grant `n` Apex Points to test the post-cap screen — forces the member
    /// to the level cap and half-fills the bar so the Apex progress + buckets show.
    #[func]
    fn debug_grant_apex(&mut self, entity_id: i64, n: i64) -> bool {
        if self.debug_blocked() { return false; }
        let Some(s) = self.state.as_mut() else { return false; };
        let Some(i) = s.grid.idx_of_id(entity_id as u32) else { return false; };
        let st = &mut s.grid.entities[i].stats;
        st.level = st.level.max(sim_core::tick::LEVEL_CAP);
        st.apex_level =
            (st.apex_level + n.max(0) as u32).min(sim_core::tick::APEX_LEVEL_CAP);
        // Half-fill the bar toward the NEXT point (SCALING cost depends on apex_level).
        st.xp = sim_core::tick::apex_xp_cost(st.apex_level) / 2;
        true
    }

    /// DEBUG: add `n` base levels, granting the proper RO status points per level.
    #[func]
    fn debug_add_levels(&mut self, entity_id: i64, n: i64) -> bool {
        let Some(s) = self.state.as_mut() else { return false; };
        data::debug_add_levels(s, entity_id as u32, n.max(0) as u32)
    }

     /// DEBUG: raise every skill in a member's kit to its max and slot its actives.
    #[func]
    fn debug_max_skills(&mut self, entity_id: i64) -> bool {
        let Some(s) = self.state.as_mut() else { return false; };
        data::debug_max_skills(s, entity_id as u32)
    }

    /// DEBUG: equip the best catalog gear the member's class can wield, per slot.
    #[func]
    fn debug_give_gear(&mut self, entity_id: i64) -> bool {
        let Some(d) = self.data.as_ref() else { return false; };
        let Some(s) = self.state.as_mut() else { return false; };
        data::debug_give_gear(s, d, entity_id as u32)
    }

    /// DEBUG: add party Gil + Soul.
    #[func]
    fn debug_give_currency(&mut self, gil: i64, soul: i64) -> bool {
        if self.debug_blocked() { return false; }
        let Some(s) = self.state.as_mut() else { return false; };
        s.earn_gil(gil.max(0) as u64, sim_core::state::GilEarn::Debug);
        s.earn_soul(soul.max(0) as u64, sim_core::state::SoulEarn::Debug);
        true
    }
}
