#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
// The determinism bans, from clippy.toml beside this crate's Cargo.toml. DENY, not warn: a
// HashMap or a clock read does not degrade the sim, it makes replay-verify wrong, and the
// symptom (a rejected save with an empty input log) points nowhere near the cause.
#![deny(clippy::disallowed_types)]
#![deny(clippy::disallowed_methods)]
// NEVER PANIC. GDD §4.4 / CLAUDE.md: `apply_input` on bad input is a silent no-op, because a
// replay has to survive game_data version skew — a panic there does not lose one action, it makes
// the whole save unreplayable. Zero violations today, so this costs nothing and stops the first.
#![deny(clippy::panic)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
// sim-core is a pure library: no I/O, and that includes writing to a terminal. Printing from here
// also means the renderer and the verify-server inherit output nobody asked for.
#![deny(clippy::print_stdout)]
#![deny(clippy::print_stderr)]
#![deny(clippy::dbg_macro)]
// An index that cannot be proven in range is a panic. In the client that is a crash; in
// verify-server — which links this crate and REPLAYS uploaded saves under `panic = "abort"`
// — it is the whole process, so an ordinary Client-authority event becomes a remote DoS
// (0.0.294 was exactly that). 25 files carry a documented `#![allow]` for the 1044 sites that
// predate this; the other 38 are covered NOW, and every new file is covered by default.
#![deny(clippy::indexing_slicing)]
// Tests index freely — a panic there is a failed assertion, which is the point. The same goes
// for `panic!` in a match arm and the `eprintln!` a diagnostic test prints on failure.
#![cfg_attr(test, allow(clippy::indexing_slicing, clippy::panic, clippy::print_stderr, clippy::print_stdout))]
// The doc lints are OFF. This crate documents heavily and 872 of clippy's 948 findings were
// "doc list item overindented" — noise that buries the ~76 real ones and would train anyone
// reading the output to ignore all of it.
#![allow(clippy::doc_overindented_list_items, clippy::doc_lazy_continuation)]

//! sim-core — deterministic, integer-only game simulation.
//!
//! Rules (see docs/taskbar-arpg-gdd-phase0.md §4.1, §4.2):
//! - No Godot, no I/O, no threads, no wall-clock, no floats anywhere.
//! - Single PCG32 PRNG owned by `GameState`, seeded at new-game.
//! - All math is i64/u64; percentages are basis points (10000 = 100%).
//! - `tick()` and `apply_input()` are pure with respect to state.

pub mod combat;
pub mod craft;
pub mod mailbox_grant;
pub mod data;
pub mod drops;
pub mod events;
pub mod fusion;
pub mod gear_roll;
pub mod gems;
pub mod substats;
pub mod grid;
pub mod hash;
pub mod items;
pub mod lane;
pub mod materials;
pub mod potential;
pub mod replay;
pub mod rng;
pub mod skills;
pub mod soul_paths;
pub mod spawn;
pub mod state;
pub mod stats;
pub mod tick;
pub mod training;
pub mod crest;
pub mod regalia_passive;
pub mod unit_rank;
pub mod combat_score;

/// Bump whenever the canonical serialization of `GameState` changes
/// shape. Saves stamped with an older version must either replay
/// cleanly or be refused at load time (GDD §4.4).
///
/// 0.0.0-alpha  — ALPHA-1/2 minimal entity (position + movement only).
/// 0.0.1-alpha  — ALPHA-3 added combat fields to Entity.
/// 0.0.2-alpha  — ALPHA-4 added equipped/drops on Entity + ground on
///                GameState.
/// 0.0.3-alpha  — ALPHA-5 added refine. NO state shape change (uses
///                existing `refine_level` field on Item). Golden hashes
///                from 0.0.2 are preserved because effective_atk's new
///                refine term is 0 at refine_level=0 (the initial
///                value for every item in those scenarios).
/// 0.0.4-alpha  — ALPHA-6 added the `data` module (game data catalog:
///                items / monsters / zones). NO state shape change —
///                GameData is loaded once, lives outside GameState,
///                and is passed by reference where needed.
/// 0.0.5-alpha  — ALPHA-7a added `UnitDef` + zone.player_spawn
///                + zone.unit to GameData so the player is
///                spawned by `new_game_in_zone` rather than by the
///                caller. NO GameState shape change, but GameData
///                shape and golden_alpha6 hash both bumped.
/// 0.0.6-alpha  — ALPHA-8a continuous spawn + larger map. ZoneDef
///                replaced fixed `spawns: Vec<SpawnRow>` with
///                `spawn_pool` + `target_population` + `spawn_interval_ticks`.
///                GameState gained `next_monster_id` + `zone_spawn`.
/// 0.0.7-alpha  — ALPHA-8b boss-gate progression. Entity gained
///                `is_boss`. ZoneSpawnState gained kill counter +
///                boss schedule + zone_cleared + next_zone_id.
///                GameState gained `pending_zone_change` so the sim
///                can request a zone load without touching GameData.
/// 0.0.8-alpha  — ALPHA-8c auto-loot. Removed `GameState.ground` +
///                `DroppedItem` struct. Added `Entity.inventory`.
///                Drops route straight to the player's inventory on
///                monster death. InputEvent::EquipItem now references
///                `inventory_idx` rather than the now-gone ground_idx.
/// 0.0.9-alpha  — ALPHA-7d.1 stackable Material items. Item gained
///                `quantity: u32` (default 1). drop_on_death merges
///                stackable drops into the player's existing slot
///                instead of pushing a new row, so 30 scrap kills
///                show "scrap x30" rather than 30 inventory rows.
/// 0.0.10-alpha — BETA-4 ranged attacks. UnitDef + MonsterDef
///                gained `atk_range` + `proj_travel_ticks`. Entity
///                gained `proj_travel_ticks`. GameState gained
///                `arrows: Vec<Arrow>` for in-flight projectiles
///                whose damage applies on arrival (GDD §3.2).
/// 0.0.11-alpha — BETA-5 Phase 1 stats foundation. Entity gained
///                `stats: Stats` (6 RO base stats + points_unspent +
///                level + xp). No effect on combat math yet (Task #9
///                wires that in); pure schema bump for now.
/// 0.0.12-alpha — BETA-5 Phase 1.5 per-class roster. GameState gained
///                `current_unit_def` (was bridge-only) and `class_stats`
///                (BTreeMap parking lot). `set_player_class` now
///                stashes the active entity's `stats` under the old
///                class id and restores `class_stats[new_id]`
///                (or `Stats::new()` if first time on that class).
///                Inventory and equipped items are NOT class-scoped
///                — they follow the player across swaps.
/// 0.0.13-alpha — BETA-5 Phase 1.5 extended: `class_stats` widened
///                to `class_characters: BTreeMap<String, CharacterData>`
///                where `CharacterData { stats, equipped }` so equipped
///                gear is now per-class too. Inventory remains shared
///                (it's a stash). Future cards/sockets fields land
///                inside `CharacterData` — single source of "what's
///                per-character".
/// 0.0.14-alpha — BETA-5 Phase 3: hit/flee + magic. Entity gained
///                `atk_type: DamageType {Physical|Magic}` and
///                `is_ranged: bool`; `Element` gained a `Neutral`
///                variant (outside the Fire/Earth/Water triangle, always
///                100%). Combat resolves through a `Damage` output
///                packet (compute → apply): magic uses MATK(INT) and
///                ignores physical DEF, never crits/misses. All goldens
///                re-pinned for the two new Entity fields (shape-only;
///                every prior scenario defaults Physical/melee so
///                behavior is identical).
/// 0.0.15-alpha — BETA-5 Phase 3 (skill foundation): RO Job Level.
///                Stats gained `job_level` / `job_xp` / `skill_points`;
///                Entity + MonsterTemplate gained `job_xp_reward`. Kills
///                award job XP alongside base XP; `try_job_level_up`
///                grants 1 skill point per job level (cap 50). No skills
///                consume points yet — that's the next slice. All goldens
///                re-pinned for the new fields (shape add; scenarios with
///                job_xp_reward = 0 behave identically).
/// 0.0.16-alpha — BETA-5 Phase 3 skill spine. Entity gained `sp` +
///                `skills` (Vec<SkillState>); a hardcoded catalog
///                (`skills.rs`) with Bash (active: SP cost + cooldown +
///                power) and HP Recovery (passive: per-tick HP regen).
///                Classes grant skills; an active auto-casts over the
///                basic swing by priority when SP/cooldown/range allow
///                (deterministic). SP regen + passive HP regen each tick.
///                Goldens re-pinned (shape add; the grassland swordsman
///                now Bashes + regens, so alpha6's run changes).
/// 0.0.17-alpha — party spine. ZoneDef gains `party` (roster of class
///                ids); new_game spawns N members (ids PLAYER_ID..), each
///                its own build. Entity gains `unit_def` (per-member
///                sprite + labels). Kills award full base/job XP to every
///                LIVING member (per-character; no split yet). Game-over
///                fires only on a full party WIPE, not one member dying.
///                Goldens re-pinned (shape add + grassland now a 3-member
///                party, so alpha6 changes a lot).
/// 0.0.18-alpha — Arrow gains `is_magic` (renderer draws an animated
///                fireball for mage bolts vs the arrow sprite). Goldens
///                with an in-flight projectile at pin time re-pinned.
/// 0.0.19-alpha — Loot moves to a party-SHARED stash (`GameState.inventory`)
///                and a party currency (`GameState.gil`); per-entity
///                `inventory` removed. Drops/equip/unequip now flow
///                through the stash. Monster kills award `gil_reward`
///                (once, party-shared). `InputEvent::LevelSkill` lets a
///                member spend a skill point to raise a skill's level.
///                Entity shape changed (inventory→gone, gil_reward added)
///                so every golden re-pinned.
/// 0.0.20-alpha — Mage skills (Fireball AoE + Mana Flow) + first AoE
///                support. SkillDef gains `aoe_radius` + `sp_regen_flat`;
///                Damage gains `is_aoe`. The `Arrow` type/`arrows` field
///                renamed to `Projectile`/`projectiles` (it carries
///                arrows AND fireballs now), and Projectile gains
///                `aoe_radius` + `victim_team` so a blast hits every
///                enemy in its 3×3. Archer skills also added this cycle
///                (Twin Strafe + Keen Eye). The serialized field rename
///                shifts every golden hash → all re-pinned.
/// 0.0.21-alpha — Thief class + two new combat mechanics: Shadow Strike
///                (teleport-BEHIND the lowest-HP enemy in range, a
///                guaranteed backstab burst) and Double Attack (passive:
///                basic swings can strike twice). SkillDef gains
///                `teleport`, `target_lowest_hp`, `double_atk_chance_bp`.
///                Both are conditional (only fire for entities that own
///                the skill) so NO golden re-pin was needed. distance-
///                scaled projectile flight landed the same cycle.
/// 0.0.22-alpha — Acolyte class + support mechanics: Heal (active —
///                restores the most-injured ally in range for MATK% HP,
///                cast in `try_cast_support`, does NOT use the Damage
///                struct) and Blessing (passive party +ATK aura, stamped
///                onto the new `Entity.aura_atk` each tick by
///                `apply_party_auras`, folded into `effective_atk`).
///                SkillDef gains `heals_ally` + `party_atk_per_level`.
///                The new `Entity.aura_atk` field shifts every golden hash
///                → all re-pinned (behaviour unchanged: it's 0 for all
///                non-blessed entities).
/// 0.0.23-alpha — Stat→ATK formulas copied to RO pre-renewal StatusATK:
///                melee `STR+⌊STR/10⌋²+⌊DEX/5⌋+⌊LUK/5⌋`, ranged the DEX
///                mirror; VIT soft DEF now `⌊VIT/2⌋`. Deterministic (RO's
///                per-hit variance dropped). At STAT_MIN all extra terms
///                are 0 so every golden is byte-identical — no re-pin.
/// 0.0.24-alpha — Switched stat formulas pre-renewal → RO **Renewal**
///                (linear + BaseLevel-fed): melee/ranged StatusATK
///                `⌊Lvl/4⌋+STR+⌊DEX/5⌋+⌊LUK/3⌋` (+ DEX mirror), StatusMATK
///                `⌊Lvl/4⌋+INT+⌊INT/2⌋+…`, soft DEF `⌊VIT/2+AGI/5+Lvl/2⌋`.
///                Chosen for long-term balance (esp. PvP). `matk_from_int`
///                → `status_matk`. Re-pinned alpha6 (leveling now feeds
///                ATK), beta6 (AGI soft-DEF), beta7 (linear MATK; mage INT
///                retuned to keep the one-shot). New docs/combat-formulas.md.
/// 0.0.25-alpha — Renewal two-layer DEF: `effective_def` split into
///                `effective_hard_def` (base+equip+refine, %-reduction
///                `amount×(K+def)/(K+10·def)`, K=4000, caps 90%, no cliff)
///                and `effective_soft_def` (VIT/AGI/Lvl flat, subtracted
///                after). Monsters share the path (symmetric). Re-pinned
///                alpha3/alpha4/alpha6/beta4 (DEF>0 targets take a % now).
/// 0.0.26-alpha — Status points per level now follow the RO **Renewal**
///                curve `⌊L/5⌋+3` (<100) / `⌊L/10⌋+13` (100+) instead of a
///                flat 5 — `stat_points_for_level`. beta5 re-pinned (a
///                1→3 level run now grants 6 points, was 10).
/// 0.0.27-alpha — Per-class HP/SP (RO Renewal job growth). Entity +
///                UnitDef gain `hp_per_level`/`sp_base`/
///                `sp_per_level`; effective_max_hp = (base + growth·Lvl)
///                ×(1+VIT/100), effective_max_sp = (sp_base + growth·Lvl)
///                ×(1+INT/100). VIT/INT are now % multipliers (was flat
///                ×5 / +INT·6, both removed). Casters get far more SP than
///                warriors. New Entity fields shifted every golden hash →
///                all re-pinned (behaviour: HP ≈ base×1.01 at lvl1/VIT1).
/// 0.0.28-alpha — ATK comes from the WEAPON, not a flat class `atk` (RO).
///                All class `atk` → 0; each class spawns a `starting_weapon`
///                (UnitDef.starting_weapon, equipped in
///                build_player_entity). Items gain `matk_bonus`
///                (staff/mace) — effective_atk folds equip matk_bonus;
///                physical atk_bonus still never touches MATK. Starting
///                weapons carry the old class atk values so balance is
///                ~unchanged. Re-pinned alpha4/5/6/beta4 (Item.matk_bonus
///                field + the lead now spawns holding a weapon).
/// 0.0.30-alpha — Per-class level-1 base stats. Stats gains `base_stats`
///                [STR,AGI,VIT,INT,DEX,LUK]; UnitDef gains
///                `base_*`; `set_class_base` snaps spawn stats to them.
///                `reset` now returns to the class base (refunds only
///                points spent above it) instead of STAT_MIN. game_data:
///                swordsman STR8/VIT6, mage INT9, etc. New Stats field →
///                all goldens re-pinned.
/// 0.0.31-alpha — RO ASPD model. Items gain `aspd_base` (per RO per-job
///                weapon swing times: dagger 150, sword/bow/staff 130,
///                mace 135). For players, effective_atk_speed_milli now
///                = 10000/(200 − ASPD) where ASPD = weapon base +
///                (AGI×5+DEX)/4, capped 180 (≈0.2s). Bare-hand / no
///                weapon → flat atk_speed_milli (keeps hand-built golden
///                scenarios identical); monsters unchanged. Old AGI×5 milli
///                bonus removed. Re-pinned item/player goldens (Item field
///                + alpha6 weapon-ASPD timing).
/// 0.0.36-alpha — Phase A buff/debuff engine. NEW `skills::buffs` module
///                (`BuffKind`/`Buff`/`BuffSpec`/`DebuffSpec` + the single
///                `buffed_stats` stat-fold point + `apply_buff`/`buff_sum`/
///                `buff_atk_element`). Entity gained `buffs: Vec<Buff>`
///                (`skip_serializing_if = "Vec::is_empty"` ⇒ shape-only:
///                every buff-less monster/test entity stays byte-identical).
///                `combat::derived` folds stat-adds (via `buffed_stats`) +
///                `*Pct`/`HitAdd`/`FleeAdd` buffs into every effective stat;
///                `build_damage` honours a self `AtkElement` override (Magnum
///                Break). `tick::decay_buffs` ages buffs + ticks `PoisonDot`;
///                `apply_party_auras` extended to stamp aura cast-buffs
///                (Angelus) alongside the legacy ATK aura. `SkillDef` gained
///                `self_aoe_radius`/`knockback`/`cast_buff`/`cast_buff_party`/
///                `on_hit_debuff`/`cast_element`; `combat_step` resolves
///                caster-centred AoE + knockback, self/party cast-buffs
///                (`try_cast_buffs`), on-hit debuffs, and element-aware
///                auto-cast. (`VERSION` also catches up from a stale 0.0.32 to
///                the 0.0.33-0.0.35 commits.) No golden re-pin expected — new
///                draws are gated behind new skills; buff-less entities are
///                untouched.
/// 0.0.37-alpha — Phase B curated job kits (~4-5 skills/class) riding the
///                Phase A engine. Swordsman Ignite Break (Fire point-blank AoE
///                + knockback); Mage Fire/Lightning/Frost Bolt (element-auto
///                3-bolt, leveled separately) + Arcane Ward; Archer Arrow
///                Shower + Sharp Focus + Hawk Sight; Thief Venom Edge +
///                Nimble Mastery; Acolyte Agility Up (active party buff) +
///                Guardian Aura (passive aura) + Holy Light. New passive
///                fields `hit_flat_per_level` / `flee_flat_per_level`.
///                RO-trademark skill names renamed to originals (display +
///                internal id + Rust const aligned; `mana_flow` → `mana_spring`
///                etc.). No GameState shape change (SkillDef fields aren't
///                serialized) — every golden stays byte-identical.
/// 0.0.38-alpha — Instance gameplay loop. `ZoneDef`/`ZoneSpawnState` gain
///                `is_instance_end` + `instance_start_zone_id`; game_data
///                restructures into a 3-room instance (grassland→forest→
///                dungeon, room 3 = is_instance_end big boss). `GameState`
///                gains `escrow: Vec<Item>` (loot held per-run, banked on
///                instance clear, lost on wipe), `farm_current_instance: bool`
///                (loop vs advance), and `soul: u64` (party currency, +1/kill /
///                +10/boss, kept through a wipe) — the bools/scalars are
///                skip-if-default; escrow is skip-if-empty. `drop_on_death` now
///                routes to escrow; `grant_escrow_to_stash`/`clear_escrow`;
///                `InputEvent::SetFarmMode`. Bridge: `restart_instance` (wipe →
///                room 1 + clear escrow) + `escrow_json`/`escrow_count`.
///                Re-pinned the kill/drop goldens (loot→escrow + soul/kill;
///                gameplay otherwise identical, no new PRNG draws).
/// 0.0.39-alpha — Character creation + onboarding. `GameState` gains
///                `character_name: String` (skip-if-empty). New `new_game_as`
///                (solo lead of a chosen class) + bridge `create_character` /
///                `classes_json` drive a vintage-RO creation screen (pick 1 of
///                5 classes, fixed per-class stats summing to 30 / each 1-9,
///                radar chart, name). Acolyte reworked to a MELEE physical mace
///                (range 1, atk_type Physical, mace gains ATK). Heal no longer
///                auto-casts as an ATTACK (`pick_active_skill` skips
///                `heals_ally`); heal triggers below 70% HP. Grassland eased
///                for a solo Lv1 (pop 8→3, slower spawns, boss 30→20). Class
///                base stats re-pinned to sum 30. golden_alpha6 re-blessed.
/// 0.0.40-alpha — Idle progression pass (target ~24h → Lv30). `xp_to_next` is
///                now QUADRATIC (`EXP_BASE·level²`, EXP_BASE=185) so per-level
///                cost outpaces the per-band EXP reward → later bands take
///                longer. Party EXP is now SHARED by size (1→100/2→75/3→60/
///                4→55/5→50%; solo unchanged → goldens stable). game_data
///                restructured into **6 level-bands × 3-room instances = 18
///                zones** (start `zone_b1_1`), smooth-scaled monsters per band
///                + **tiered gear** (weapon_b/armor_b N) so higher zones drop
///                better gear, + monster `gil_reward` (recruit economy). sim-cli
///                gained a wipe-restart (keep XP) + auto stat/gear/recruit
///                modeling + wipes/level CSV cols to measure the 24h pace
///                headlessly. golden_alpha6 (start zone) + golden_beta5 (curve)
///                re-blessed.
/// 0.0.41-alpha — Drop-rate tuning for idle check-in cadence. Removed the
///                `scrap` junk material entirely; every monster drop table is
///                now `weapon_bN 2 / armor_bN 3 / nothing 95` → ~5% keepable
///                gear per kill ≈ **~20 gear/hour** (≈ one 20-slot stash fill,
///                so the player clears the stash about hourly). The `nothing`
///                sentinel is a real (inert, non-marketable) item so load-time
///                validation passes; `drop_on_death` recognizes it and banks
///                no loot — `roll_drop` still takes its single PRNG draw, so
///                the draw-order contract is intact. golden_alpha6 re-blessed
///                (drop tables feed its escrow → state hash).
/// 0.0.42-alpha — RO 10-slot paper-doll. New `EquipSlot` enum (Helmet/Mid/Low,
///                Armor, Weapon, Shield, Earring, Shoes, Necklace/2) + an
///                `Item.equip_slot: Option<EquipSlot>`; equip de-dup now keys on
///                the SLOT, not `ItemKind`, so a shield + body armor (both
///                `Armor` kind) coexist. `None` falls back to a kind-derived
///                slot → legacy/test gear is byte-compatible in behavior (only
///                the serialized hash shifts). game_data gains the full gear set
///                per band (weapon/armor/shield/garment/shoes/3 headgear/2
///                accessory ×6 = 60 defs); drop tables now roll all 10 band
///                slots (~0.5% each) + `nothing` 190 = ~5% gear/kill. sim-cli
///                auto-equips best-per-slot. golden_alpha4/5/6 + beta4 re-blessed.
/// 0.0.43-alpha — Job curve QUADRATIC. `job_xp_to_next` was linear (60·job)
///                while base XP is quadratic, so job level raced to ~3× base.
///                Now `JOB_EXP_BASE(320)·job_level²` — same growth order as base,
///                so job tracks a ~constant ~0.7× of base: Lv30→Job~22,
///                Lv60-70→Job~50 (cap). Measured over a 24h sim-cli run. Kills
///                earn job XP, so golden_alpha6 + golden_beta8 re-blessed (beta8
///                also rescaled its job_xp_reward 200→1650 to keep its two-job-
///                level-in-one-kill intent). 2nd-job (past the cap) is deferred.
/// 0.0.44-alpha — Passive HP regen (HP Recovery skill) moved off the 1-second
///                tick onto a 5-second cadence (`HP_REGEN_INTERVAL_TICKS=50`),
///                with the per-fire amount scaled ×5 so the average HP/s is
///                unchanged — the HP bar now heals in a few bigger, less jittery
///                bounces. SP regen / poison / buff aging stay on the 1s
///                interval. golden_beta9 re-blessed (ticks 30→60 to cross a
///                regen boundary).
/// 0.0.45-alpha — RO weapon system. Weapons are now class-restricted: a new
///                `default_weapon_classes(weapon_type)` matrix (Dagger = all
///                five jobs; 1H Sword = swordsman+thief; 2H Sword = swordsman;
///                Bow = archer; Rod/Staff = mage+acolyte; Mace = acolyte) gates
///                `equip_item`, with a per-item `Item.equip_classes` OVERRIDE
///                reserved for special cross-class gear. game_data gains 6
///                weapon types per band (dagger/sword/greatsword/bow/rod/mace —
///                rods carry MATK for mages) and the full 9-piece armor set that
///                never actually landed in 0.0.42; drop tables roll the lot
///                (daggers common, the rest rarer). sim-cli auto-equip respects
///                the restriction. golden_alpha4/5/6 + beta4 re-blessed.
/// 0.0.46-alpha — Gear tiers coarsened. Was 6 per-band gear tiers (one per
///                5-level band); now 3 ten-level SETS — s1 (Lv1-10 = bands 1-2),
///                s2 (Lv11-20 = bands 3-4), s3 (Lv21-30 = bands 5-6). Each
///                band's drop table maps to its set (`ceil(band/2)`); set stats
///                step ×2/×4/×6. 45 gear defs (15 pieces × 3 sets). Zones stay
///                6 bands. golden_alpha6 re-blessed (drop content).
/// 0.0.47-alpha — DIRECT-GRANT loot. Drops now bank STRAIGHT to the party stash
///                instead of the instance escrow: gear is KEPT on a party wipe.
///                Losing hard-won loot because you couldn't clear an instance is
///                too punishing for an idle game — keeping it means a failed run
///                still yields an upgrade, so the player gets stronger and
///                eventually clears. The escrow (+ `grant_escrow_to_stash` /
///                `clear_escrow`) is now vestigial (kept for legacy saves); the
///                `INVENTORY_CAPACITY` cap is the only loot sink. Also fixed the
///                stale `effective_atk` doc — rods DO scale a mage's damage via
///                `matk_bonus`. golden_alpha6 + beta4 re-blessed (loot lands in
///                stash, shifting the state hash).
/// 0.0.48-alpha — PATHS OF SOUL (sim-core). A party-wide meta skill tree spent
///                with the previously-dead SOUL currency: a branching, prereq-
///                gated tree (`soul_paths.rs`, 9 nodes) whose bonuses are SHARED
///                by the whole party. Power nodes (ATK/DEF/MATK/HP %) apply as
///                always-on party auras (new `BuffKind::HpPct` folds into
///                `effective_max_hp`); Farming nodes (EXP/Gil/Soul %) scale kill
///                rewards; QoL `Hoard` adds stash slots. Per-save (in
///                `GameState.soul_paths`, wiped by a new game), spent via the new
///                `InputEvent::LevelSoulPath`. Empty tree = total no-op, so old
///                saves + all goldens are byte-identical (0 drift). UI next.
/// 0.0.49-alpha — Paths of Soul reworked into a Path-of-Exile-style passive WEB:
///                a graph of allocate-once nodes joined by edges, unlocked
///                OUTWARD from a free `origin` (a node is allocatable only when
///                adjacent to an owned one). `PathNode` carries x/y + neighbors +
///                flat magnitude + flat cost; `is_allocatable` (adjacency)
///                replaces the level-prereq gate. Still a total no-op when empty
///                → goldens 12/12 byte-identical.
/// 0.0.50-alpha — Soul economy pass. Soul income now SCALES with the current
///                band (mob = band, boss = band*3; band parsed from the
///                instance start id) so the running total keeps climbing — the
///                idle-incremental feel. Paths of Soul web expanded to ~29 nodes
///                across three deep arms with STEEP per-node cost (500 → 1.5M),
///                each node a small flat +1% (or +1 stash slot) so a big web
///                doesn't inflate stats — cost is the treadmill, not magnitude.
///                golden_alpha6 re-blessed (band-1 boss soul shifted).
/// 0.0.51-alpha — Signature skills fire reliably. Fireball + Ignite Break were
///                starved out: their high SP cost lost to the cheaper spammable
///                skills that drained SP first, so they almost never fired. They
///                are now NO-mana with a long 10 s (100-tick) cooldown — a
///                periodic signature whose cost is the cooldown, not SP (SP stays
///                for the cheaper skills). Auto-cast priority changed to LONGEST-
///                ready-cooldown first (then element score, then list order) so a
///                signature preempts the spammables the instant it's up. No
///                golden drift (scenarios have ≤1 ready active).
/// 0.0.52-alpha — Mana RESERVATION for buffs (PoE-style). Buff skills (Blessing,
///                Arcane Ward, Sharp Focus, Agility Up) no longer cast on a
///                cooldown (where they were SP-starved + flickered); they're now
///                persistent AURAS that RESERVE a flat chunk of SP. New
///                `SkillDef.sp_reserve`: while a reservation aura is active it
///                lowers usable SP (`max - Σ reserved`, allocated in list order
///                until the budget runs out) and applies its buff to the party
///                every tick for free. Active skills draw from the unreserved
///                pool — stacking auras genuinely trades SP for power. Shadow
///                Strike also joined the no-mana signature set (0.0.51b). No
///                golden drift (reservation-free entities are byte-identical).
/// 0.0.53-alpha — Spam skills equalised to a flat SP 14 (Bash 12→14, Arrow
///                Shower 16→14, Venom Edge 12→14, Holy Light 16→14; bolts +
///                Twin Strafe already 14). No cheaper skill can dominate by
///                winning the SP race, and each one's small cooldown rotates
///                them — cast one, it goes on cooldown, a sibling fires next.
///                golden_beta9 re-blessed (Bash SP/timing).
/// 0.0.54-alpha — CLEAN skill model: three tidy types. ACTIVE skills are now
///                COOLDOWN-ONLY (every `sp_cost` is 0; the SP gates + spends are
///                gone from the cast paths), so cooldown is the single power dial
///                — short = spammy, long = signature (no more no-mana special
///                case, no SP-equalising hack). AURA skills are the only thing
///                that touch SP, via reservation. PASSIVES are always-on. SP thus
///                has ONE meaning: the aura budget. Spam cooldowns lengthened
///                (bolts 1.6s, melee 2s, Arrow Shower 2.4s) now that SP no longer
///                throttles them. golden_beta9 re-blessed.
/// 0.0.55-alpha — Fair auto-cast ROTATION. New `SkillState.last_fired` adds a
///                final tie-breaker to skill selection: among otherwise-equal
///                ready skills (same cooldown + element score) the one that fired
///                LONGEST ago goes next — a "ready-time queue" so equal skills
///                rotate evenly instead of the list-first one always winning. The
///                higher tiers are unchanged: longest cooldown first (signatures
///                preempt), then element score (bolts still prefer the winning
///                element). For same-cooldown skills last_fired == ready order,
///                so it matches a true FIFO queue where it matters. golden_beta9
///                re-blessed (skill now records its fire tick).
/// 0.0.56-alpha — New mage passive ELEMENTAL MASTERY: attacks that BEAT the
///                target's element deal +2.5% damage per level (max +25% at 10).
///                Carried on the `Damage` packet (new `mastery_bp`, set by
///                `build_damage` from the passive) and applied by `apply_damage`
///                ONLY on an element advantage (mult > 100%). In the mage tree at
///                job level 5. No golden drift (no scenario has the passive →
///                mastery_bp 0 → byte-identical).
/// 0.0.57-alpha — Assassin DUAL-WIELD. New `EquipSlot::WeaponLeft` off-hand +
///                `DUAL_WIELD_MASTERY` passive (Assassin 2-1): a member who knows
///                it can equip a SECOND dagger (routed to the off-hand by
///                `equip_item`); a BASIC swing then strikes with BOTH hands in the
///                same tick — each hand rolls Crit/Hit/variance/Double-Attack
///                INDEPENDENTLY (`resolve_offhand_attack`), the off-hand scaled by
///                `30% + 7%/level` (`dual_wield_offhand_factor_bp`, Lv10 = 100%).
///                `effective_atk` split into per-hand (`effective_atk_for_hand`)
///                so each blade counts only its own weapon; `effective_offhand_atk`
///                added. Dual-wield carries a flat −10 ASPD. SKILLS use the main
///                hand only (no double-dip). Also fixes a standing bug: the weapon
///                matrix had no `assassin_t2`, so an advanced thief could equip
///                NOTHING — added to Dagger + Sword1H. All new paths gated by
///                `is_dual_wielding`, so non-dual streams are byte-identical →
///                NO golden re-bless (only the EquipSlot variant bumps the shape).
/// 0.0.58-alpha — Assassin BLOODTHIRST passive: each LAST-HIT kill adds a damage
///                stack (`+0.5%/level`, capped at `BLOODTHIRST_MAX_STACKS = 50`),
///                applied to ALL of the holder's damage via the `effective_atk`
///                %-fold (`combat::bloodthirst_bonus_bp`). The streak RESETS at the
///                start of a new instance run (`is_instance_start` — band advance,
///                retreat, OR a farm-mode loop back to room 1) AND on death (both
///                revive sites — zone re-entry + Phoenix Tail — clear it); a
///                mid-instance room hop keeps it. New transient `Entity.bloodthirst_stacks`
///                (`#[serde(skip)]`, like `hiding_used`) so there is NO serialized
///                shape change and goldens stay byte-identical. Increment rides the
///                existing on-kill hook in `handle_monster_death` (gated by owning
///                the skill → no PRNG / draw-order change). Same cycle: the reset
///                trigger for BOTH per-run states is unified to `is_instance_start`
///                (entering an instance's room 1). Thief HIDING's charge is now
///                per-SESSION (was per-band): a farm-mode loop back to room 1 now
///                refreshes it, matching "3 rooms = 1 session". Transient/gated →
///                goldens byte-identical.
/// 0.0.59-alpha — Assassin DARK AMBUSH active (cd 10s): blinks onto the NEWEST
///                non-boss enemy in a long reach (range 9) and bursts it for
///                150%+50%/lvl ATK (Lv10 600%), physical single hit, main-hand.
///                New `SkillDef.target_newest_nonboss` + `find_newest_nonboss_enemy_in_range`
///                (highest spawn id) feed a new `pick_active_skill` target branch;
///                reuses the Shadow Strike teleport-behind path. SkillDef fields
///                aren't serialized and the cast is gated to skill owners → no
///                shape change, no golden drift.
/// 0.0.60-alpha — Assassin SONIC BLOW active (cd 10s) + new STUN status. The sim
///                resolves ONE point-blank packet `300%+100%/lvl` ATK (Lv10 1200%,
///                main-hand); the "8 hits" is a renderer flourish (display only).
///                On a connecting cast, `on_hit_debuff` rolls ONCE for a
///                `12%+2%/lvl` (Lv1 12% .. Lv10 30%) STUN for 3 s. New
///                `BuffKind::Stun` (append-only marker): the tick loop skips a
///                stunned entity's turn (no move/attack), but it stays TARGETABLE
///                and does NOT break on damage (decays normally) — unlike Hiding/
///                freeze. BOSSES are stun-immune (`apply_on_hit_debuff` guard;
///                the roll still fires to keep the stream stable). Gated → no
///                golden drift. (Renderer: show 8 staggered numbers like the mage
///                multi-bolt — deferred to the UI pass.)
/// 0.0.61-alpha — MOTION DELAY (RO animation/motion lock). New
///                `SkillDef.motion_delay_ticks`: when a skill resolves the caster
///                is LOCKED for that many ticks (`Entity.act_locked_until`,
///                `#[serde(skip)]`) — `tick_one_entity` skips its turn (no move /
///                attack / next cast) so the skill animation plays out before
///                basics resume. This is the lock that blocks basic attacks; RO's
///                After-Cast Delay only gates the NEXT skill, so motion delay is
///                the accurate name. Set in `attack_step` (then breaks the swing
///                loop) and `fire_pending_cast`; distinct from the pre-cast
///                `cast_time` windup. The attack-speed accumulator STILL charges
///                during the lock (capped at one swing), so the delay OVERLAPS the
///                basic-attack interval (RO: motion subsumes amotion) rather than
///                stacking on top. Sonic Blow gets 8t (~0.8s flurry); else 0 =
///                no lock. Transient + default 0 → goldens byte-identical.
/// 0.0.62-alpha — Assassin POISON CLOUD + classic-POISON status + first GROUND-
///                FIELD engine; thief Venom Edge re-themed to BLEED (Open Wound).
///                New `GameState.ground_fields: Vec<GroundField>` (skip-if-empty)
///                + `GroundFieldKind` — a fixed-position persistent field; Poison
///                Cloud (active Lv5, cd 60s, no SP, `SkillDef.ground_field_radius`
///                2 = 5×5) places one on the caster's cell that applies classic
///                `Poison` to enemies inside each interval (`tick_ground_fields`);
///                cast proactively when off-cd + an enemy is in range
///                (`try_cast_ground_field`). New `BuffKind::Poison` (classic):
///                soft DEF −25%, loses 1.5%·MaxHP+2/s but only while HP > 25%
///                (floors there, CAN'T kill), SP regen off, bosses immune. New
///                `BuffKind::Bleed` = the old ATK-scaled PoisonDot mechanic under
///                a separate name so poison stays the Assassin's theme — Open
///                Wound (was Venom Edge) now bleeds; Bleed will NOT feed Poison
///                Explosion. All gated / skip-if-empty / default-0 → goldens
///                byte-identical. (Poison Explosion = next step.)
/// 0.0.63-alpha — Assassin POISON EXPLOSION passive (Lv10, cd 30s) — completes the
///                Assassin kit (7/7). On the holder's last-hit kill of a POISON-
///                afflicted enemy (classic `Poison` OR Deadly `PoisonDot`, NOT
///                Bleed), the corpse detonates: a 5×5 physical Poison-element blast
///                whose TOTAL power (500%+100%/lvl ATK, Lv10 1400%) is split
///                equally among living enemies caught. Rides the on-kill hook
///                (`maybe_poison_explosion`); the 30 s cooldown is latched before
///                the blast so a kill it scores can't chain another. Gated to the
///                passive's owner → no PRNG/draw change, goldens byte-identical.
/// 0.0.64-alpha — DEBUG tooling (dev only). New `GameState.debug_god` (skip-if-
///                false): `tick::apply_debug_god` tops every player up each tick
///                (full HP/SP, revived, cooldowns cleared) + stamps a huge
///                ATK/MATK buff → invincible + one-shotting. New sim debug helpers
///                (`data::debug_max_job_and_skills` / `debug_advance` /
///                `debug_give_gear`) + bridge methods (`debug_set_god`,
///                `debug_set_level`, `debug_max_job_and_skills`, `debug_advance`,
///                `debug_give_gear`, `debug_give_currency`) for the in-game debug
///                menu. All gated / skip-if-false → goldens byte-identical.
///                (Multi-save-slots + the debug-menu UI are Godot-side, next.)
/// 0.0.65-alpha — Multi-hit display CHANNEL (renderer feature). New transient
///                `GameState.recent_hits: Vec<HitEvent>` (`#[serde(skip)]`, cleared
///                each tick): every melee resolve CALL emits one `HitEvent`
///                {target, attacker, amount, hits} via a threaded sink —
///                `resolve_attack(_scaled)` + `resolve_offhand_attack`. `hits` =
///                max(instances, `SkillDef.display_hits`): a plain hit = 1, Double
///                Attack = 2, Sonic Blow = 8; the off-hand emits its OWN event so
///                dual-wield shows two separate popups. Exposed in `view_state` for
///                the arena's MultiHitPopup (white numbers + yellow total). Damage
///                math unchanged + serde-skip → goldens byte-identical.
/// 0.0.66-alpha — Swordmaster (swordsman 2-1) STARTED. New class `swordmaster_t2`
///                (swordsman `advances_to`), KEEPS the swordsman tree + adds the
///                2-1 kit incrementally. First skill: Cleave (passive) — a basic
///                hit splashes `25%+5%/level` of its damage to enemies orthogonally
///                adjacent to the target (`skills::cleave_splash_bp` +
///                `apply_cleave_splash`). Deterministic (fraction of the dealt blow,
///                no DEF/crit re-roll, no PRNG) + gated on knowing Cleave so goldens
///                stay byte-identical (no re-bless). Also a transient
///                `GameState.poison_bursts` (`#[serde(skip)]`) surfacing each Poison
///                Explosion for the renderer's burst FX.
/// 0.0.67-alpha — Swordmaster kit COMPLETE (6 more skills). Sword Quicken (self
///                AspdPct reservation buff), Double Edge (2-hit active that also
///                Cleaves — Cleave now triggers on targeted skill strikes too),
///                Shield Blast (5×5 self-AoE scaling on ATK + 50% HARD DEF via new
///                `SkillDef.def_scale_bp`), Shield Boomerang (new `target_furthest`
///                selection at range 7 + single-target taunt via `taunt_on_hit_ticks`),
///                Autocounter (struck-defender riposte, gated by the passive + a
///                shield → PRNG only for owners), Sword & Shield Mastery (+2 hard
///                DEF/level +1% ASPD/level while a shield is equipped). New SkillDef
///                fields (def_scale_bp / requires_shield / target_furthest /
///                taunt_on_hit_ticks) default to 0/false + every new path is gated on
///                a Swordmaster passive/shield → goldens byte-identical (no re-bless).
/// 0.0.68-alpha — GLOBAL shield ASPD penalty. Equipping a shield (`EquipSlot::
///                Shield`) now costs a flat −5 ASPD for EVERY class (5000 milli in
///                `effective_atk_speed_milli`, like dual-wield's −10). Sword & Shield
///                Mastery shaves 0.5 ASPD/level off it (gone at Lv10) instead of the
///                placeholder +1%/level it granted in 0.0.67. No golden equips a
///                shield in its scenario → goldens byte-identical (no re-bless).
/// 0.0.69-alpha — RO HP CURVE ported (raw magnitudes). MaxHP base now follows the
///                real RO formula (rAthena `job_basepoints` factor mode): per level
///                it gains `(hp_increase + hp_factor·level)/100`, a CONVEX curve
///                (per-level HP rises with level) instead of the old linear
///                `hp_per_level`. New class fields `hp_factor`/`hp_increase` (entity +
///                UnitDef, serde-default 0 → legacy linear when unset);
///                `combat::ro_base_hp` + reference to rAthena/iROwiki in the doc. All
///                8 built classes set to their REAL RO per-class constants (Swordman
///                504/70 .. Knight 524/150, Wizard 503/55, etc.), L1 base = 40 per RO.
///                HP magnitudes jump to RO scale (Knight ~9.5k@99 incl VIT) — combat
///                difficulty / band pacing to be RE-TUNED next (user-approved). SP
///                left on the legacy linear model (a %-of-MaxSP rework is pending).
///                ALL 11 goldens re-blessed (player HP changed → intended drift).
/// 0.0.70-alpha — Elementalist (mage 2-1) damaging ground-fields + Freeze. New
///                `GroundFieldKind::Damage {amount, element, status, ...}` +
///                `GroundField.source_id` (kill/DPS credit; serde-default) +
///                `BuffKind::Freeze` (Stun-like timed immobilize, boss-immune).
///                `tick_damage_field` hits everyone in range each interval
///                (snapshot MATK·power via apply_damage) + rolls the field status;
///                `try_cast_ground_field` builds a Damage field when the skill has
///                power/debuff, else the legacy Poison Cloud. Skills: Glacial Storm
///                (Water + Freeze), Vermilion Field (Wind + Blind), Quicksand (slow,
///                no damage). All gated on a field existing (Elementalist-only) so
///                goldens byte-identical (no re-bless). Earlier this version line also
///                added Earth Piercer/Earth Drive/Incantation Mastery.
/// 0.0.71-alpha — Elementalist field polish: cast bar + per-field hit timing.
///                Ground-field skills with a cast time now CHANNEL (show a cast
///                bar; `fire_pending_cast` places the field when the windup ends via
///                `cast::place_ground_field`) instead of placing instantly. Damage
///                fields fire on a PER-FIELD `hit_interval` (new SkillDef
///                `field_hits`/`field_duration_ticks`; Damage variant carries
///                `hit_interval`) — Vermilion Field = 20 hits over 4 s, Glacial Storm
///                10/4.5 s, per-hit MATK% so totals match the design. Gated on a
///                field existing → goldens byte-identical (no re-bless).
/// 0.0.72-alpha — Elementalist fields centre on the ENEMY (cast range) not the
///                caster; Glacial Storm = RO Storm Gust: guaranteed freeze on the
///                3rd hit (`SkillDef.status_at` + per-victim `Entity.field_hit_count` +
///                `Damage.status_at`), a FROZEN victim takes no field damage, any
///                real attack breaks Freeze (resolve_attack/offhand) so the field
///                resumes, and random-direction knockback per hit
///                (`Damage.knockback`). Gated → goldens byte-identical (no re-bless).
/// 0.0.73-alpha — Elementalist pass: align built skills to the GDD design (TODO.md)
///                + add Meteor Rain. (1) Glacial Storm freeze = RO Storm Gust proper:
///                BaseChance table 60%@Lv1→15%@Lv10 EVERY hit (DebuffSpec.chance_per_level
///                now i32 so a status can scale DOWN), the `status_at`-th hit BOOSTS the
///                base to 150% (no longer an absolute guarantee — was), then FREEZE
///                applies the RO resist: success ×(97−MDEF−LUK/3)/100, duration ×(1−MDEF/100).
///                (2) FROZEN now full RO: DEF halved (effective_hard_def), FLEE 0
///                (effective_flee_points), armor→Water (apply_damage), breaks on damage.
///                (3) New `SkillDef.field_hits_per_level` → Earth Drive = "level hits ×125%"
///                (5×5 burst field) and Meteor Rain (NEW: 8×8 Fire barrage, 125%/meteor,
///                2→11 meteors, sky-fall, NO stun). (4) Quicksand flat-AGI → `BuffKind::
///                AgiDexSlowPct` (5–25% cut to AGI+DEX+move) so it actually slows; cd 12s→30s.
///                All Elementalist-gated → goldens byte-identical (no re-bless), but VERSION
///                bumps for the new BuffKind variant + SkillDef field (serialized shape).
/// 0.0.74-alpha — Meteor Rain reworked to the real RO WZ_METEOR. It is no longer a
///                whole-field nuke: a new METEOR field mode (`GroundFieldKind::Damage`
///                gains `impact_radius` + `strike_hits`; `SkillDef.impact_radius`)
///                drops meteors ONE PER interval onto a RANDOM cell in the 7×7 target
///                area, each blasting only a 5×5 (`impact_radius` 2) around where it
///                lands — so a drop can MISS. Per level the cast schedules meteor
///                COUNT `2 + lvl/2` (2→7) and HITS-per-meteor `(lvl+1)/2` (1→5). Each
///                meteor is ONE damage instance — VISUAL multi-hit: MDEF subtracted
///                once on `per_hit × hits` (125% ATK Fire each), shown as `hits`
///                numbers + a yellow total, with ONE `3%/level` STUN roll per meteor
///                (so 7 meteors = 7 instances). Random strike cell = 2 PRNG draws, gated
///                on a meteor field (Elementalist-only) → 11 goldens byte-identical
///                (no re-bless). VERSION bumps for the new Damage fields + SkillDef
///                field (serialized shape). Godot: Fire-field meteor-impact highlight.
/// 0.0.75-alpha — Vermilion Field reworked to RO Lord of Vermilion + a damage-number
///                standard. NEW once-per-target field mode (`SkillDef.once_per_target`,
///                `GroundFieldKind::Damage.once_per_target`, per-field
///                `GroundField.hit_ids`): a unit caught in the lingering 7×7 AoE takes
///                ONE 20-hit VISUAL instance (MDEF once on 20 × per-hit) + one Blind
///                roll, then is skipped — but a unit ENTERING later (or a fresh cast /
///                another caster's field, since hit_ids is per-field) gets its own.
///                `tick_ground_fields` now iterates fields by index to read/write
///                hit_ids; `tick_damage_field` returns the ids it hit. Meteor also
///                finalised as VISUAL (1 meteor = 1 instance, DEF once, shown as N).
///                Damage-number STANDARD documented in NumberFx: white=enemy /
///                red=ally / yellow=total, 1 hit=1 number, N-hit action=N+total, death
///                never truncates, status once/instance, VISUAL vs REAL (ask per skill).
///                All Elementalist-gated → 11 goldens byte-identical (no re-bless).
///                VERSION bumps for the new serialized fields.
/// 0.0.76-alpha — Magic-damage pass + Soul Crush (Elementalist 8/9; Flame Shield left).
///                (1) MATK is now
///                a RANGE per RO Renewal: `combat::apply_matk_variance` rolls the
///                WEAPON-MATK variance `±0.1 × weapon_level × matk_bonus` (StatusMATK
///                — our `status_matk`, an exact Renewal match — stays fixed); one draw
///                in the magic branch of `resolve_ranged_swing`. (2) The mage BASIC is
///                nerfed to 50% ATK (was 100%). (3) Soul Crush — CHARGE skill (engine
///                #8): new `SkillState.charges_left` (serde skip-if-0); while charged,
///                the mage basic becomes a 200% ATK NEUTRAL hit spending one charge;
///                max charges `5 × level` (max Lv5 ⇒ 5..25), refilled on every zone
///                change (`build::zone`). beta7 (the only mage golden) RE-BLESSED for
///                the 50% basic + MATK variance (intentional); the rest byte-identical.
/// 0.0.77-alpha — Flame Shield → ELEMENTALIST 9/9 COMPLETE. New absorb-shield engine
///                (shared with Bishop Praesidium): Entity gains serde-SKIP `absorb_pool`
///                / `absorb_burst_dmg` / `absorb_burst_knock` / `absorb_burst_pending`.
///                Flame Shield (self active, Lv10, cd 30s, `try_cast_flame_shield` on
///                cooldown) banks `INT × level × 2` absorb; `combat::absorb_soak` soaks
///                incoming phys+magic in `resolve_attack[_scaled]` (overflow passes
///                through, empties the pool); when emptied `tick::resolve_absorb_bursts`
///                detonates a 3×3 100% ATK Fire blast + a 1/2/3 (by level) knockback,
///                credited to the caster, flashed via the meteor-impact FX channel. All
///                fields serde-skip + gated on a live shield → goldens byte-identical
///                (no re-bless); no serialized-shape change (bump is for the feature).
///                Shields STACK as a per-entity FIFO `Vec<AbsorbShield>`: damage drains
///                the OLDEST first + cascades; an emptied shield explodes; a re-cast
///                from the SAME source replaces its own (RO "re-cast replaces"), other
///                sources (a Bishop's Praesidium) stack separately.
/// 0.0.78-alpha — Sword & Shield Mastery now does its GDD "unlock 2H+shield": a
///                two-handed sword + a shield is FORBIDDEN to equip (blocked in
///                `events::equip_item` — silent no-op) UNLESS the member knows the
///                mastery; once unlocked it carries a damage penalty of −50% at Lv1
///                easing to 0% at Lv10 (`5000×(10−level)/9` off `effective_atk`, only
///                when a 2H weapon AND a shield are worn). Gated on the combo → no
///                golden equips it → byte-identical (no re-bless); no serialized
///                shape change. (S&S Mastery's earlier −5-ASPD relief + +2 DEF/lvl
///                kept.) Unit tests: equip-block + penalty curve.
/// 0.0.79-alpha — Autocounter is now an SP-RESERVE stance (GDD "counter reserve")
///                instead of a free passive: `sp_reserve 10`; the riposte in the
///                melee resolve now also requires `skill_reserved("autocounter")`
///                (the reserve fits the SP budget), so it competes with Sword
///                Quicken for SP and is OFF when starved. Swordmaster-only/gated →
///                12 goldens byte-identical (no re-bless); catalog-only, no shape
///                change. (Double Edge stays a VISUAL 2-hit per the GDD — confirmed.)
/// 0.0.80-alpha — SP-RESERVE reworked from an SP-point budget to a SLOT budget. Every
///                character has `sp_reserve_capacity = 10 + INT/10` reserve SLOTS
///                (INT = base + permanent card/gear, NOT timed buffs like Blessing —
///                so capacity is stable but unequipping INT gear lowers it, dropping
///                the loadout's over-budget tail);
///                each SLOTTED ACTIVE costs ≥1 slot (`SkillDef.sp_reserve` →
///                `sp_reserve_slots`; default active = 1, big party buffs = 2, Arcane
///                Ward = 3), passives 0. `skills::loadout_fits` (replaces
///                `skill_reserved`/`reserved_sp`) gates EVERY active's firing on
///                fitting the budget in loadout order — overflow = inert. Reserves NO
///                LONGER touch the SP pool (removed the usable-SP clamp in upkeep);
///                SP regens to full max. Arcane Ward = 3 slots, no per-hit SP drain
///                (drain_ward_sp/WARD_SP_COST_BP deleted). Autocounter is now an Active
///                stance (1 slot). All gated/non-serialized → 12 goldens byte-identical
///                (no re-bless). Bridge `reserved_sp` → 0 (UI SP bar no longer shaded).
/// 0.0.81-alpha — Flame Shield cooldown now starts when the shield BREAKS, not at
///                cast: `try_cast_flame_shield` sets no cd + won't re-bank while a
///                self shield is up; `resolve_absorb_bursts` starts the 30 s cd on
///                the burst. Real downtime after the blast before re-shield. Gated →
///                goldens byte-identical. (Plus UI: per-skill slot_cost in the tree,
///                sp_reserve_capacity/used bridge methods, proportional reserve bars
///                on the party SP bar + skill page.)
/// 0.0.82-alpha — BISHOP (acolyte 2-1) started + Heal/Blessing cast motion. New
///                `bishop_t2` class (acolyte advances_to; HP factor 75 / increase 513
///                = rAthena Priest, confirmed by its `HpFactor` field; SP on our
///                linear scale, base stats kept from class 1 per the advance rule).
///                catalog/bishop.rs: Gloria (party +6 LUK/level via new
///                `BuffKind::LukAdd`, 2 slots) + Mace Mastery (+5 ATK/level, MACE arm
///                in passive_atk_bonus). Carries the acolyte-1 tree. Heal + Blessing
///                gain a 0.5 s casting-animation motion lock (`motion_delay_ticks 5`,
///                applied in try_cast_support/try_cast_buffs) — a motion lock, not a
///                cast bar. Gated → 12 goldens byte-identical (no re-bless). VERSION
///                bumps for the LukAdd variant + bishop_t2 data. (6 Bishop skills
///                remain: Sancta Tempestas, Subvenio, Resurrection, Praesidium,
///                Purificatio, Dupla Medicina.)
/// 0.0.83-alpha — Bishop heal pair (4/8). Dupla Medicina (`try_cast_dupla`): when an
///                ally is < 75% HP, heals the 2 lowest-HP% allies map-wide (incl. the
///                priest) for 60% of the Renewal Heal formula. Subvenio
///                (`try_cast_subvenio`): an ally < 40% HP → blink to a RANDOM free
///                cell in their 5×5 (`movement::teleport_adjacent`) + Heal them for
///                the priest's own Heal value, sharing Heal's cd; own cd 120→60 s by
///                level. Extracted `renewal_heal_amount` (one heal-formula source).
///                Gated → 12 goldens byte-identical (no re-bless).
/// 0.0.84-alpha — Bishop Resurrection (5/8): auto-revive (`tick::resurrection_upkeep`,
///                mirrors phoenix) — a living holder with the skill + a CHARGE revives
///                the first-fallen (lowest-id) downed ally at 10%/level MaxHP (Lv10 =
///                100%). One charge per band via `SkillState.charges_left`, refilled
///                on a band start in `build::zone` (like Hiding). Passive/auto (no
///                loadout slot). Gated → 12 goldens byte-identical (no re-bless).
/// 0.0.85-alpha — Bishop Sancta Tempestas (6/8): the Bishop's HOLY AoE nuke. A new
///                NORMAL-field VISUAL multi-hit knob (`SkillDef.field_strike_hits`,
///                threaded into `place_ground_field`'s plain-field branch): a 5×5
///                Holy field lasts 10 s and fires a WAVE every ~2.5 s (`field_hits`
///                4 waves), each wave ONE Magic damage instance of `10%×level` MATK
///                drawn as 10 numbers (`field_strike_hits` 10) — Lv10 = 1000% ATK
///                per wave. Element Holy (shreds Shadow/Undead). RO cast: Fixed
///                1 s + Variable 4 s (cast_time 50 → effective_cast_time's 20/80
///                split) + Cast Delay 1 s (motion lock); cd 15 s. Gated/non-serialized → 12 goldens
///                byte-identical (no re-bless); VERSION bumps for the new SkillDef
///                field. (2 Bishop skills remain: Praesidium, Purificatio.)
/// 0.0.86-alpha — Resurrection tuned: (1) now an ACTIVE that reserves 1 loadout
///                slot (was a free Passive) — `resurrection_upkeep` gates the
///                auto-revive on `loadout_fits` like the other auto-cast actives,
///                so it competes for SP-reserve budget. (2) A 5 s GRACE DELAY
///                (`RESURRECTION_DELAY_TICKS` = 50) before it fires: a new transient
///                serde-skip `Entity.down_ticks` ages each downed party member
///                each tick (reset on revive), and the rez only targets allies
///                down ≥ the delay — no more instant pop-up the moment a member
///                falls. Gated on the skill + serde-skip counter → 12 goldens
///                byte-identical (no re-bless); no serialized-shape change.
/// 0.0.87-alpha — Bishop Praesidium Divinum (7/8) — ally absorb-shield (RO Kyrie
///                Eleison). Extends the absorb engine: `AbsorbShield` gains
///                `hits_left` (a hit-count cap — the shield also ends after N
///                absorbed BLOWS, pool OR hits whichever first; `absorb_soak`
///                decrements + ends it) and `persist` (RESERVED, set but not yet
///                read — for a future survives-a-new-run effect). Shields now
///                follow the buff/Bloodthirst lifetime in `build::zone`: they CARRY
///                across a mid-instance room hop and clear only on a NEW RUN (band
///                advance / retreat / farm-loop) or death — Praesidium included
///                (it behaves like every other shield today).
///                `try_cast_praesidium` (action loop 0.63, cd 20 s, no SP,
///                1 reserve slot, 0.5 s cast motion) shields the LOWEST-HP% ally
///                that does NOT already carry a PD shield with pool = caster
///                MaxHP × (10+2·lvl)% + (5+lvl/2) hits, no burst (burst_dmg 0 →
///                `resolve_absorb_bursts` skips it). Targeting the UNSHIELDED weakest
///                SPREADS protection (one ally per cd, several PD up at once),
///                topping each back up as it breaks. Bridge surfaces it as a
///                synthetic "Praesidium" buff (hit-capped shield, any src; hover
///                shows the remaining pool). AbsorbShield is serde-skip → 12 goldens
///                byte-identical (no re-bless). (1 Bishop skill remains: Purificatio.)
/// 0.0.88-alpha — Bishop Purificatio (8/8) — BISHOP COMPLETE. A cleanse
///                (`try_cast_purificatio`, action loop 0.55, cd 5 s, no SP, 1 slot,
///                0.5 s motion): auto-targets the ally with the WORST clearable
///                ailment (priority Stun > Freeze > Poison > Blind > Slow; tie-break
///                lowest HP% then id) and strips ALL that ally's level-unlocked
///                debuffs at once. `purificatio_rank` gates WHICH clear: Lv1 Stun /
///                Lv2 +Freeze / Lv3 +Blind / Lv4 +Poison(classic) / Lv5 +Slow
///                (AgiSlowAdd + AgiDexSlowPct). NEVER clears `PoisonDot` (Deadly
///                Poison / EDP) or `Bleed` — cleanse-immune by design. Mostly
///                forward-looking (statuses run player→enemy today) so it usually
///                holds. Catalog/cast-only + gated → 12 goldens byte-identical (no
///                re-bless). Bishop's 8 skills: Gloria, Mace Mastery, Dupla
///                Medicina, Subvenio, Resurrection, Sancta Tempestas, Praesidium
///                Divinum, Purificatio (+ the carried acolyte-1 tree). Also: a
///                DOWN party member now has its DEBUFFS stripped each tick
///                (`BuffKind::is_debuff` — Stun/Freeze/Blind/Poison/Bleed/PoisonDot/
///                slows/DefDown; beneficial buffs kept) so it isn't revived still
///                crippled. Player-team + down-only + players carry no debuffs in
///                normal play → 12 goldens byte-identical (no re-bless). Plus a
///                `debug_inflict_statuses` dev button to test Purificatio.
/// 0.0.89-alpha — RANGER (archer 2-1) STARTED — 1/8. New `ranger_t2` class
///                (archer advances_to; HP factor 85 = rAthena Hunter; carries the
///                archer-1 tree). First skill Arrow Crafter (`catalog/ranger.rs`,
///                marker passive): on a LAST-HIT kill of a NON-Neutral enemy,
///                10%/level chance to craft 1 arrow of the victim's element into
///                the killer's ammo pool (passive feed for the elemental-arrows
///                system). One PRNG draw in `handle_monster_death`, gated on owning
///                the skill AND a non-Neutral victim (like Steal) → 12 goldens
///                byte-identical (no re-bless). Class data is external (game_data)
///                so no GameState shape change. (7 Ranger skills remain: Knock Back
///                Shot, Pet Master, Pet Assault, Snare Strike + ROOT status, Land
///                Mine, Claymore Trap, Freezing Trap + the trap engine.)
/// 0.0.90-alpha — Ranger 2/8 (Knock Back Shot) + ranged-killer attribution + a
///                knockback-direction fix. (1) `Projectile` gains a serde-skip
///                `attacker_id` (set at fire) so `resolve_arrived_projectiles` can
///                pass the firer to `kill_monster` — RANGED kills now run on-kill
///                hooks (which makes the archer's Arrow Crafter actually fire; was
///                dead before since projectiles passed killer `None`). Skill-gated +
///                no golden ranged class owns an on-kill skill → byte-identical.
///                (2) Knock Back Shot (`catalog/ranger.rs`, marker passive;
///                `maybe_knock_back_shot`): a SURVIVING single-target shot has
///                5%/level chance to shove the victim 2 cells, ONCE per enemy
///                lifetime (`Entity.kb_shot_hit`, serde-skip). (3) `knockback_entity`
///                direction reworked from per-axis sign (8-way) to DOMINANT-axis
///                with a narrowed ~30°–60° diagonal band (tan30° fixed-point) — so a
///                multi-cell shove travels straight, not snapped to a diagonal. All
///                serde-skip/gated → 12 goldens byte-identical (no re-bless).
/// 0.0.91-alpha — Ranger 3/8 (Pet Master) — ENTITY-BASED abstract pet (HP-ready for
///                a future tank pet). `Entity` gains serialized skip-if-default
///                `is_pet` / `owner_id` / `untargetable` (non-pet entities serialize
///                identically → goldens byte-identical). `find_enemy_idx` skips
///                `untargetable`; party-wipe / XP-share / boss-spawn-anchor exclude
///                `is_pet`. `tick::combat_step::pet_step` (called after the action
///                loop; pets skipped in it) spawns ONE pet per Pet Master owner
///                (id from `next_monster_id`, untargetable + immortal), chases the
///                nearest enemy, and bites ~1/sec for `4 × level` ATK (its own stat),
///                crediting the owner on kill. Godot party-UI (overlay frames /
///                sidebar / member picker) excludes pets. All gated on the skill →
///                no golden spawns a pet → 12 goldens byte-identical (no re-bless);
///                VERSION bumps for the new Entity fields. (Pet Assault + Snare
///                Strike + ROOT status next; pet art = cosmetic TODO.)
/// 0.0.92-alpha — Ranger 5/8 (Pet Assault + Snare Strike) — two pet ACTIVES, each
///                costing 1 SP reserve slot (reserve-gated like every active).
///                Pet Assault (cd 100 = 10 s, requires Pet Master): when slotted +
///                off-cd, the pet's bite becomes a heavy strike of `150% + 50%/level`
///                of pet ATK (Lv1 150% .. Lv10 600%), logic in `pet_step::pet_act`.
///                Snare Strike (requires Pet Master): a surviving bite has
///                `2.5% × level` chance (logic `pet_step::maybe_snare`) to apply the
///                NEW `BuffKind::Root` for 2 s — movement locked, attacks still allowed
///                (gated in `tick`'s `move_step`) — but only ONCE per enemy lifetime
///                (new serde-skip `Entity.snare_used`); bosses immune. `Root` appended
///                to `BuffKind` (after `LukAdd`) + `is_debuff()`. All paths gated on
///                the owner owning + slotting the skill, and the snare draw is taken
///                only for an un-snared non-boss target → no golden runs them (no pets)
///                → 12 goldens byte-identical (no re-bless); VERSION bumps for the new
///                `BuffKind::Root` variant + `Entity.snare_used`. (Ranger 6–8 = the
///                Trap system: Land Mine / Claymore / Freezing Trap, next.)
/// 0.0.93-alpha — Ranger 8/8 COMPLETE (TRAP system) — a NEW engine feature:
///                `GameState.traps: Vec<Trap>` (skip-if-empty), dormant ground
///                objects (NOT entities) `Trap { cell, kind, level, owner_id, fuse,
///                triggered }` + `TrapKind { LandMine, Claymore, Freezing }`.
///                Placement (`tick::combat_step::trap::try_place_trap`, a new
///                proactive cast in `tick_one_entity` after the ground-field cast):
///                a slotted trap active off its PLACE cooldown, under the per-kind
///                cap (3), with a threat present, drops one trap UNIFORMLY among the
///                perimeter cells of its 5×5 Chebyshev-2 ring (a free cell = in-bounds
///                + no other trap; an entity standing there does NOT block — traps lie
///                on the GROUND, so the ring reaches the side enemies approach from) —
///                a SINGLE PRNG draw (`gen_range_u32(0, free.len())`), no bias, no
///                re-roll. Triggering (`tick_traps`, once/tick after `pet_step`) is
///                EDGE-based via `Trap.seen_ids`: a trap fires only when a NEW enemy
///                id ENTERS its 3×3 (seeded at placement with whoever's already there,
///                so a mine dropped under a sitting foe waits for a newcomer). On fire
///                ONCE — Land Mine `250%+50%/lvl` single-target,
///                Claymore arms a 5-tick fuse then a 3×3 `150%+30%/lvl` blast,
///                Freezing guarantees `BuffKind::Freeze` for `lvl+2` s on the 3×3
///                (no damage, no resist roll, bosses immune). Damage routes the
///                normal `apply_damage`/`kill_monster` path (owner credited).
///                `change_zone` clears `traps`. Three Active defs (`land_mine` /
///                `claymore_trap` / `freezing_trap`, 1 reserve slot, place cd 8/10/12
///                s, max Lv5) + tree nodes (job 11/13/15, no prereq) + registry.
///                Everything gated on owning + slotting a trap skill (no golden party
///                does) and the placement draw only fires on a successful place →
///                12 goldens byte-identical (no re-bless); VERSION bumps for the new
///                `traps` field + `Trap`/`TrapKind` shape. RANGER kit COMPLETE.
/// 0.0.94-alpha — XP pacing re-tune (sim-cli verified). The level curve goes from
///                pure quadratic (`250·L²`) to BLENDED quad+cubic
///                `70·L² + 10·L³` (base) and `90·j² + 13·j³` (job) — steeper, so
///                Lv1-20 are CHEAPER (≈2× faster early) but the crossover at ~Lv20
///                makes everything past it progressively SLOWER (Lv70 ≈ 2× slower).
///                Party XP share hardened to `[100,70,55,45,40]` (was
///                `[100,75,60,55,50]`) so a full party (the late game) levels
///                slower while early SOLO play (share=100%) is untouched. Net (sim-
///                cli, full party, 24/7): Lv70 ≈ 10.3M ticks ≈ 12 days (in the
///                8-14d demo target); no base level cap (overflows past 99 by design,
///                Lv99 ≈ 28M ticks). Re-blessed alpha6/beta5/beta8 (curve shifts the
///                hash + the beta5/beta8 XP-carry value asserts updated to the new
///                thresholds). No shape change → only these XP-leveling goldens move.
/// 0.0.95-alpha — Monsters carry real RO Hit/Flee/MDEF. NEW flat overrides
///                `Entity.{mob_hit, mob_flee, mob_mdef}` (skip-if-zero) + matching
///                `MonsterDef`/`MonsterTemplate` fields (`hit`/`flee`/`mdef`, serde
///                default 0). `effective_hit_points`/`effective_flee_points`/
///                `effective_mdef` use the override when >0, else derive from the
///                stat block as before — so players & hand-built golden entities
///                (override 0) are byte-identical. Previously mobs had stat-1 blocks
///                → ~0 Hit/Flee/MDEF (players never missed, magic ignored MDEF); now
///                mobs dodge, land hits, and resist magic per RO. Band mob stats
///                (HP/ATK/DEF + new MDEF/Hit/Flee) re-set to the **real pre-renewal
///                median** per 5-level band (sourced from the rAthena pre-re mob_db,
///                see docs/reference/). Re-blessed alpha6 (loads game_data → combat
///                outcomes shift); beta5/beta8 use hand-built mobs → unaffected.
///                VERSION bumps for the new Entity/MonsterDef/MonsterTemplate fields.
/// 0.0.96-alpha — FIX: 2-1 classes can wield their weapon type. `default_weapon_classes`
///                listed only the `*_t1` jobs for Bow/Staff/Mace, so a player who
///                advanced archer→ranger / mage→elementalist / acolyte→bishop could
///                no longer equip a bow/staff/mace at all (advancement KEEPS the worn
///                weapon, so it wasn't visible until trying to upgrade). Added
///                `ranger_t2` (Bow), `elementalist_t2`+`bishop_t2` (Staff), `bishop_t2`
///                (Mace). Catalog logic only — no GameState shape change; goldens use
///                `*_t1` classes (no advancement) so all 11 stay byte-identical (NO
///                re-bless). Also two sim-cli-only measurement fixes (no sim effect):
///                model class advancement at job cap (`build::auto_advance`), and
///                EXCLUDE the ranger pet from the wipe check (a party whose every real
///                member died but whose untargetable pet survived never registered a
///                wipe → the headless run deadlocked at the b8 boss). With both, the
///                modeled party now advances to 2-1 and pushes through b9→b10 (Lv70 ≈
///                13.7 days 24/7) instead of stalling at Lv59.
/// 0.0.97-alpha — Roster ⇄ Party (Phase 1, sim-core model). NEW
///                `GameState.roster: BTreeMap<u32, RosterChar>` (skip-if-empty)
///                = owned-but-BENCHED characters keyed by unique id → duplicate
///                classes allowed. `build::{owned_count, buy_character, deploy,
///                bench, buy_character_cost}`: buy adds a benched char (Gil,
///                progressive cost), deploy rebuilds its grid entity from the
///                class def + overlays its saved loadout, bench captures a live
///                member back (despawning it + its pet). LEAD (PLAYER_ID) stays
///                fixed/always-deployed this phase. MIGRATION: the old class-keyed
///                `class_characters` parking lot folds into `roster` on load
///                (each parked alt-class → a benched char); the field is kept for
///                old-save deserialize, emptied after. Shape change → VERSION
///                bump; roster empty by default so all 11 goldens + existing saves
///                stay byte-identical (NO re-bless). Bridge + Godot party UI =
///                Phase 2/3.
/// 0.0.98-alpha — Roster Phase 1b: removed the fixed LEAD. The party is now
///                SYMMETRIC — `bench` accepts ANY deployed member (only rule:
///                ≥1 must remain), and the few spots that needed "the lead"
///                (recruit-gate `lead_level`, the deploy spawn anchor) derive it
///                from `first_deployed` = the lowest-id deployed player instead of
///                the hardcoded lead constant. RENAMED the constant `PLAYER_ID` →
///                `FIRST_PLAYER_ID` (it's now purely the player id-allocation BASE
///                — the id the first char spawns with + the empty-grid fallback —
///                not a "lead"). Logic-only (no shape change) → all 11 goldens
///                byte-identical (goldens never bench; the constant's VALUE is
///                still 1, only its name changed).
///                Godot: Party UI drops the "(lead)" tag + lets any member bench;
///                `sim_driver.viewed_member()` falls back to the first deployed
///                member if the viewed one was benched.
/// 0.0.99-alpha — Party identity: NEW `GameState.party_name` (skip-if-empty) —
///                the persistent "you" now that there's no fixed lead. Set at
///                creation: the creation screen ("Found Your Party") names the
///                PARTY + picks its first member; recruit/buy the rest from the
///                Party screen. Bridge `create_character` stamps `party_name`
///                (+ `character_name` for the save-slot label); new `party_name()`
///                getter (falls back to character_name for older saves). Godot:
///                creation relabeled to party-naming; Party UI header shows the
///                name. Shape change → VERSION bump; field empty by default so all
///                11 goldens + existing saves byte-identical (NO re-bless).
/// 0.0.100-alpha — Ranger trap tweaks (gameplay): (1) trap placement now BIASES
///                 toward the enemy side — pick uniformly among perimeter cells in
///                 the nearest enemy's half-plane (offset · threat-dir > 0), falling
///                 back to the full ring if the front is blocked; a uniform pick
///                 wasted ~half the traps behind the archer. (2) Land Mine is now
///                 UNCAPPED (was 3) — still naturally bounded by the 16-cell ring;
///                 Claymore/Freezing keep the cap of 3. Behaviour-only (no shape
///                 change); trap code is skill-gated + keeps its single PRNG draw,
///                 so all 11 goldens stay byte-identical (NO re-bless).
/// 0.0.101-alpha — DPS-meter attribution for the field / trap skills (they were
///                 logged as "Basic Attack" or, for traps, not at all). NEW
///                 `GroundField.skill_id` (serde default, skip-if-empty) → every
///                 persistent field (Glacial/Meteor/Vermilion/Quicksand/Sancta)
///                 credits its per-interval damage to the casting skill; Flame
///                 Shield's absorb burst now credits "flame_shield"; TRAP damage
///                 (Land Mine / Claymore) now calls `record_damage` at all with the
///                 trap id + clamped overkill. Caster-gated → all 11 goldens
///                 byte-identical (no field/trap in goldens; skill_id empty there).
///                 VERSION bump for the new GroundField field.
/// 0.0.102-alpha — Auto-retreat is now SIM-CLI ONLY. `handle_party_wipe` gains an
///                 `auto_retreat: bool`: the real game (bridge `restart_instance`)
///                 passes `false` → a wipe just restarts the instance in place;
///                 sim-cli passes `true` → it still models the drop-a-band-and-farm
///                 optimal pacing. Logic-only (no shape change; the retreat state
///                 fields just never trigger in-game); goldens don't exercise wipes
///                 → byte-identical. The manual farm toggle (game) is unaffected.
/// 0.0.103-alpha — Pet Master damage buffed: pet ATK 4×level → 20×level (Lv10
///                 40 → 200). Balance-only, skill-gated (no golden spawns a pet)
///                 → all 11 goldens byte-identical. Tooltip/doc updated.
/// 0.0.104-alpha — Subvenio is self-contained: firing it no longer parks the Heal
///                 skill (removed the shared-cooldown bump). It runs on its OWN
///                 per-level cooldown only; the heal amount still scales with the
///                 priest's Heal level. Balance/logic-only, skill-gated → all 11
///                 goldens byte-identical.
/// 0.0.105-alpha — FIX: Freeze now shatters on ANY direct hit, not just basic/
///                 off-hand attacks. Added the freeze-break (retain ≠ Freeze on
///                 dealt > 0) to pet bites, trap hits (Land Mine/Claymore) and
///                 Poison Explosion — previously those damaged a frozen enemy
///                 without clearing the freeze. Damaging GROUND FIELDS still SKIP
///                 frozen by design (they don't reach a victim to break it).
///                 Skill-gated (no golden has Freeze) → all 11 goldens byte-identical.
/// 0.0.106-alpha — Freeze fix, part 2: RANGED/MAGIC hits now shatter Freeze too.
///                 The 0.0.105 pass missed the projectile arrival path
///                 (`resolve_arrived_projectiles`) — every mage bolt / arrow /
///                 sky-fall spell applied damage there without clearing Freeze.
///                 Added the break (retain ≠ Freeze on damage > 0). Goldens have no
///                 Freeze → byte-identical.
/// 0.0.107-alpha — Freeze-break CENTRALIZED + ground fields now break it too.
///                 NEW `Entity::take_hit(dealt)` = the ONE place a direct hit
///                 lands (hp − dmg, shatter Freeze, flag death); every damage site
///                 routes through it — melee/skill (resolve_attack/offhand), pet
///                 bite, traps, Poison Explosion, ranged/magic projectiles, ground
///                 fields, and Flame Shield's burst — replacing the duplicated
///                 inline freeze-breaks from 0.0.105/106. Glacial Storm & co. no
///                 longer SKIP frozen victims (removed that guard): the rule is now
///                 simply "any direct damage shatters Freeze; only DoT/poison
///                 leaves it". Behaviour-preserving for everything that isn't
///                 frozen → all 11 goldens byte-identical (no Freeze in goldens).
/// 0.0.108-alpha — Data: b1 mob MDEF 15 → 0 (RO first-zone mobs ~0). The
///                 median-port MDEF made b1 an outlier (15, higher than b2-b5)
///                 that floored a Lv1 mage's basic (50% ATK ≈ 14) to 1 damage.
///                 Mage basic is intentionally 50% ATK — the fix is the bad MDEF,
///                 not the basic. Re-blessed alpha6 (loads game_data → b1 combat
///                 shifts); beta goldens (hand-built mobs) unaffected.
/// 0.0.109-alpha — Data: seed CARD drop rate ≈ 1/20,000 (beta). The 3 cards
///                 (greenling/yellowling/redling on b1-b3 a-mobs) were weight 1 vs
///                 a ~1.014M table total = 1/1,014,251 (effectively never). Set
///                 weight 51 → ~1/19,888 so a solo beta tester actually sees cards.
///                 Launch plan: rarer 1/50k-100k for the trade economy (see
///                 docs/drop-system.md). Re-blessed alpha6 (loads game_data → b1
///                 drop draw shifts); beta goldens (hand-built) unaffected.
/// 0.0.110-alpha — Combat feedback: "Dodged" / "Guarded" floating text. NEW
///                 `HitKind {Normal, Dodge, Guard}` on `HitEvent` (display-only —
///                 HitEvent is serde-skip, so NO serialized-shape change / golden
///                 drift). A physical MISS (Flee beat Hit — main + off-hand) pushes
///                 a Dodge event; a hit a shield FULLY absorbs pushes a Guard event.
///                 Bridge surfaces `kind` in the recent_hits JSON; the renderer
///                 (number_fx) shows "Dodged"/"Guarded" text at the target (these
///                 leave no hp-delta so no number would otherwise appear). No PRNG
///                 draws added → 235 unit + 11 golden byte-identical.
/// 0.0.111-alpha — RENEWAL accuracy so DEX/AGI finally MATTER. Two parts:
///                 (1) DATA: monster mob_hit/mob_flee bumped to renewal scale
///                 (+175 hit / +100 flee — they were the pre-re Lv+DEX / Lv+AGI
///                 port; monster HP/ATK/DEF stay pre-re on purpose for the big
///                 party). (2) FORMULA: `HIT_RATE_BASE_PCT` 80 → 0, so
///                 `Hit% = HIT − FLEE` (the renewal 100/95 base is already inside
///                 the 175/100 HIT/FLEE bases). At equal stats hit% = 75% and each
///                 DEX/AGI point of lead = ±1% BOTH ways (matches the RO renewal
///                 wiki: player→mob `100+Hit−(200+Lv+AGI)`, mob→player dodge
///                 `95+Flee−(170+Lv+DEX)`). Consequence: a naked entity now MISSES
///                 ~25% at parity (was a guaranteed 100% cap), so combat has real
///                 hit/dodge — DEX raises accuracy, AGI raises dodge. Re-blessed ALL
///                 goldens (every STAT_MIN swing shifts 155%→cap to 75%→rolls);
///                 outcome tests give their attacker enough DEX to cap. sim-cli
///                 pacing must be re-measured (players now miss/dodge).
/// 0.0.112-alpha — VFX plumbing (display-only, NO golden change). GameState gained
///                 `skill_fx: Vec<SkillFx>` (#[serde(skip)], cleared each tick like
///                 recent_hits) — pushed at the melee-skill resolve site with the
///                 caster/target ids + skill_id, so the renderer plays a per-SKILL
///                 effect (e.g. Bash's impact burst) instead of the generic weapon
///                 slash. Bridge also surfaces per-entity `weapon_type` (main-hand)
///                 so the basic-attack slash is keyed by WEAPON, not class (future:
///                 per-weapon effects). Also `combat::main_hand_weapon_type` (pub)
///                 and `tick::preview_pending_skill` (pub, PURE read of
///                 pick_active_skill) — the latter lets the renderer show the right
///                 attack animation + skill-name banner during the WIND-UP (skill_fx
///                 only arrives at resolve). All display-only reads; sim serde shape
///                 unchanged (skip) → hash untouched. GRID: every zone's width/height
///                 in game_data.json scaled x0.75 (bigger sprites) — re-blessed
///                 golden_alpha6 (grassland loads a real zone; the only golden that
///                 does). VERSION bumped to log the new view fields + the zone resize.
/// 0.0.113-alpha — SOUL PATHS expansion (~+27 nodes, const data only): four new
///                 DIAGONAL arms off the origin (Onslaught ATK/STR · Tempest MATK/DEX
///                 · Endurance HP/DEF/VIT · Prosperity economy), each early ~400 →
///                 late 120k, + every cardinal spine deepened one more late node
///                 (40k-100k). Reuses the existing PathEffect palette (no new
///                 mechanics). soul_paths is a BTreeMap keyed by node id and no
///                 golden/sim-cli allocates paths, so hashes are untouched (no
///                 re-bless); the bridge iterates SOUL_PATHS so the UI shows them
///                 automatically. NOTE: doubles the reachable power ceiling — monster
///                 scaling will want a re-tune (see difficulty-retune memory). ALSO:
///                 +15 cheap 400-Soul EARLY nodes (a starter branch off each cardinal
///                 arm) so a fresh save isn't wall-gated; among them two flat
///                 "Lifeblood" (+50 max HP) + two flat "Edge" (+5 ATK) nodes needing
///                 two NEW `BuffKind::MaxHpAdd`/`AtkAdd` (folded in effective_max_hp/
///                 effective_atk as +0 no-ops unless a path grants them → goldens
///                 still byte-identical) + `PathEffect::HpAdd`/`AtkAdd`.
/// 0.0.114-alpha — MONSTER DB, phase 1. `MonsterDef` gained an RO-style descriptive
///                 block (name, level, sp, race, size, STR/AGI/VIT/INT/DEX/LUK,
///                 view_range) + a `sprite` key — all #[serde(default)]; combat
///                 still uses the DIRECT hp/atk/def/hit/flee (stats are identity for
///                 now). `sprite` DECOUPLES the look from the combat `element` (the
///                 renderer keyed slime colour off element before). First real monster:
///                 mob_b1_a = "Pinkling" (element Water, sprite slime_pink, race Plant).
///                 Bridge surfaces per-entity `sprite` + `mon_name` for the render
///                 override + hover tooltip. Re-blessed golden_alpha6 (Pinkling's
///                 element Earth→Water shifts the grassland combat).
/// 0.0.115-alpha — Twin Strafe now reads as a VISUAL 2-hit (like Double Attack).
///                 `Projectile` gained a transient `display_hits` (#[serde(skip)],
///                 like `attacker_id`) carried to impact; `resolve_arrived_projectiles`
///                 pushes ONE display-only HitEvent with `hits = display_hits` so the
///                 renderer splits the single arrow into N numbers (damage still lands
///                 once). TWIN_STRAFE.display_hits = 2. Serde-skip + recent_hits is
///                 display-only → hashes byte-identical (no re-bless).
/// 0.0.116-alpha — EARLY-GAME BAND RE-ANCHOR. Renumbered the whole band ladder to
///                 smooth the ramp: inserted a NEW easy starter band 1 (Baby
///                 Pinkling / Baby Greenling / Elder Pinkling, ~½ the old b1 stats)
///                 + 4 stat-interpolated "veteran" bands at slots 7/9/11/13. Old
///                 bands shifted: old b1→b2 (Pinkling), …, old b12→b17, old b14→b19
///                 (14 old → 19 total). All done in game_data.json (data, not
///                 GameState shape) — zone/monster ids renumbered + chains rewired
///                 deterministically. render_data DEFID_TO_SHEET humanoids moved
///                 b7/8/9 → b10/12/14. Re-blessed golden_alpha6 (zone_b1_1 is now
///                 the Baby-Pinkling starter). VERSION bumped: saves embed zone/
///                 monster ids, so old saves no longer replay against this catalog.
///                 (Briefly tried a 20th "b10.5" interp band to soften the old-b7→b8
///                 seam — reverted: subdividing didn't lower the wall, it's a
///                 stat-vs-level grind the party clears at Lv54 regardless of band
///                 granularity, and the extra boss only ADDED wipes. Kept as a wall.)
/// 0.0.117-alpha — Monster ATK damage RANGE (RO atk1/atk2). `MonsterDef` +
///                 `MonsterTemplate` + `Entity` gained `atk_min`/`atk_max`
///                 (Entity: `mob_atk_min`/`mob_atk_max`, skip-if-zero like
///                 `mob_hit`). A mob's physical hit rolls ATK uniformly in
///                 `[min,max]` and scales the packet by `rolled/atk`
///                 (`apply_weapon_variance` Team::Monster branch), one draw; no
///                 range ⇒ flat atk, no draw. game_data seeded at ±10% of atk for
///                 every monster. Players keep weapon-level roll-DOWN. Also filled
///                 Baby Pinkling/Greenling hit/flee (0 → 91/51). Re-blessed ALL
///                 goldens (every scenario has monster hits). Also +4 early Soul
///                 Path nodes (ce4-ce7, 1_000 Soul each: +HIT/+FLEE) between the
///                 400 cluster and the 2_000 Precision spine, plus one 1_000-Soul
///                 node on each other early arm (pe4 +ATK, he4 +HP, fe4 +EXP,
///                 qe4 +MATK) — additive, goldens buy no paths so no further drift.
/// 0.0.118-alpha — BOMB monsters (ChomBombs, band 3). MonsterDef/MonsterTemplate/Entity
///                 gain `death_explode_dmg` + `death_explode_push` (Entity: skip-if-zero
///                 like `mob_atk_min`). On death, `handle_monster_death` → `death_explode`:
///                 flat damage + knockback (away from the corpse, clamped) to EVERY alive
///                 entity — BOTH teams — in the 3×3; a monster killed by the blast chains
///                 its own. Deterministic (no PRNG). Only ChomBombs carry it (5 dmg / 1
///                 push) → no golden has the field (skip-if-zero) → byte-identical, no
///                 re-bless.
/// 0.0.119-alpha — Bomb explosion is now SCHEDULED, not instant: a dying ChomBomb queues
///                 a `PendingExplosion` (new GameState Vec, skip-if-empty → goldens
///                 byte-identical) that fires EXPLODE_DELAY (2 = 0.2 s) ticks later via
///                 `fire_pending_explosions` (a new tick step after resolve_absorb_bursts),
///                 so the sim damage + knockback land IN SYNC with the explosion anim
///                 instead of the instant of death. `knockback_entity` split into a
///                 cell-based `knockback_from` (+ the entity wrapper) so a scheduled blast
///                 with no live source can still shove. Chain = a monster killed by the
///                 blast schedules its own. Also folded skill lookups into
///                 Entity::skill_level / ready_skill + shared victims_in_radius across the
///                 AoE blasts (all byte-identical).
/// 0.0.120-alpha — Knockback now surfaces a display-only shove event: `knockback_from`
///                 pushes the shoved entity id to `GameState.knockbacks` (a new
///                 `#[serde(skip)]` Vec cleared each tick, like `recent_hits` → NOT hashed,
///                 goldens byte-identical). The bridge relays it so the renderer plays a
///                 parabolic bounce arc for a shoved sprite instead of a flat 1-cell slide.
/// 0.0.121-alpha — MONSTER PASSIVE SKILLS + skill-based on-death blasts. Monsters now carry
///                 catalog skills (`MonsterDef.skills` → `MonsterTemplate.skills` →
///                 `Entity.skills` at spawn, instantiated at level 1; a monster may hold
///                 several). The hard-coded `death_explode_dmg`/`death_explode_push` scalars
///                 (MonsterDef/MonsterTemplate/Entity) are REMOVED — the on-death blast is
///                 now a Passive skill: CONVENTION `kind==Passive && self_aoe_radius>0` marks
///                 an on-death blast, and `schedule_explosion` reads its params
///                 (`power_bp`=FLAT dmg, `knockback`=push, `self_aoe_radius`=radius,
///                 `on_hit_debuff` Poison=toxic cloud). Two new catalog passives:
///                 `self_destruct` (ChomBomb, band 3) and `toxic_burst` (Toxic Chom, band 4:
///                 same blast + classic Poison to survivors, bosses immune). `PendingExplosion`
///                 gains `radius` + `poison_ticks`. Band 4 named: Yellowling (slime_gold/Fire),
///                 Toxic Chom (toxic_burst), King Yellowling boss. `apply_or_refresh_poison`
///                 is now pub(crate) (shared by Poison Cloud + toxic blasts). New behavior is
///                 gated on carrying the skill → synthetic goldens byte-identical; alpha6
///                 RE-BLESSED (band-1 MonsterTemplate pool no longer serializes the dropped
///                 death_explode scalars — a serialized-shape change, not a logic change).
/// 0.0.122-alpha — Band-4 room variety: `zone_b4_2` gains Burst Chom (`burst_blast` passive:
///                 a big 5×5 flat-20 blast + knockback, red recolor sheet). Also a DATA
///                 convention shift — monster ids for the NAMED monsters (bands 1-4) are now
///                 DESCRIPTIVE (`mob_b1_a`→`baby_pinkling`, `boss_b4`→`king_yellowling`, …)
///                 instead of `mob_bN_x`, so spawn pools read clearly and species duplicate
///                 (`burst_chom_2`…) cleanly. Zone ids stay `zone_bN` (band/soul/difficulty
///                 still parse from them); the boss-name UI now derives its band from the
///                 current zone, not the boss id. def_id is a serialized value → alpha6
///                 RE-BLESSED (band-1 slimes' def_id strings changed; no logic/shape change).
///                 Bands 5-19 keep `mob_bN_x` until they're named.
/// 0.0.123-alpha — Rechum (band 4, room 3): a SUPPORT bomb-plant. New passive `mending_burst`
///                 (`heals_ally: true`) — its on-death blast HEALS every ally monster in a
///                 5×5 for a flat 50 HP instead of dealing damage. `PendingExplosion` gains a
///                 `heal` field; when > 0 `resolve_explosion` routes to `resolve_heal_blast`
///                 (heals the non-Player team in radius, no damage/knockback/poison/chain).
///                 zone_b4_2 already had Burst Chom; zone_b4_3 reworked (Yellowling dropped)
///                 to toxic_chom/burst_chom/rechum — a nastier pre-boss room. New monster
///                 skill path is gated → goldens byte-identical (no re-bless).
/// 0.0.124-alpha — CARD SYSTEM REWORK (RO-flavored, card-equips-to-CHARACTER). The old
///                 "card compounded into a gear item's socket" model is REMOVED: `Item.cards`
///                 gone; `SlotCard`/`SlotCardInStash` events gone. New model: cards equip to a
///                 per-character CARD PANEL by CLASS (new `CardClass`: Weapon/Shield/Armor/
///                 Earring/Shoes/Accessory) via `EquipCard`/`UnequipCard`. Panel capacity per
///                 class (`card_panel_capacity`: Weapon 4 / Shield 1 / others 2); how many are
///                 ACTIVE = min(cards equipped, summed sockets of the equipped gear feeding
///                 that class) — so gear upgrades "turn on" more cards and a gear swap keeps
///                 the cards (just re-gates activation). `card_bonuses` rewritten to sum the
///                 active panel cards (+ bound permanent). New per-character `card_panel`
///                 on `Entity` + `CharacterData` (parked/restored like `equipped`). Item gains
///                 `card_slot: Option<CardClass>` + RESERVED `star: u8` / `foil: bool` for the
///                 deferred Honkai-style star-fuse + Epic/Foil variant leveling. ALL new fields
///                 are serde-skip (empty/none/zero/false) → card-less states byte-identical.
///                 The CODE change re-blesses nothing; but the paired DATA change (tagging
///                 the existing cards with a `card_slot` class — card_redling drops in band
///                 1) shifts the band-1 drop-pool template serialization → alpha6 RE-BLESSED
///                 (a card-def field, not logic). Godot card-panel UI = follow-up.
///                 See docs/card-system.md, rarity-system.md.
/// 0.0.125-alpha — Card panel UI (Godot, RO3-style: class rail / panel slots / collection).
///                 CardClass grows to 8: `Head` (the 3 head slots feed it) + the two
///                 accessories are SEPARATE panels `Necklace` / `Ring` (each gated by
///                 its own accessory's sockets, cap 2). Bridge surfaces `card_panel` +
///                 per-class `card_sockets`. New variants are unused by data → existing
///                 states/goldens unchanged (no re-bless).
/// 0.0.126-alpha — The 3 -ling starter cards DESIGNED: Pinkling/Greenling/Yellowling, all
///                 Armor-slot (card_redling→card_pinkling), 0★ base +5 ATK/+200 HP/+10 FLEE
///                 (+1 stat). FOIL cards: a dropped Card rolls 5% (`FOIL_DROP_CHANCE_BP`) to
///                 be Foil — ONE extra PRNG draw taken ONLY on the (rare) card-drop path
///                 (contract note in drops.rs). A Foil card ADDS its authored `foil_effect`
///                 (new per-card Item/ItemDef field) ON TOP of `card_effect` — think "+how
///                 much more", any fields (not a fixed ×2); card_bonuses sums both. Card
///                 effect/foil data changed the band-1 drop-pool templates → alpha6
///                 RE-BLESSED (data, not logic). See docs/card-system.md.
/// 0.0.127-alpha — CARD ALBUM: cards now live in a SEPARATE dex-style store
///                 (`GameState.card_album` + upgradeable `card_album_cap`, base
///                 `CARD_ALBUM_BASE_CAP=30` type-slots), parallel to the gear stash
///                 (stash = non-card loot / album = cards) — a growing collection never
///                 crowds the 20-slot stash. `bank_to_inventory_inner` redirects ANY
///                 `ItemKind::Card` to `bank_card_to_album` (one stack per type; merge by
///                 def_id / push if room / shared `overflow_recover`), so every banking
///                 path (drop / shop buy / loot / unequip) funnels cards to the album.
///                 `equip_card`/`bind_card` now index the album (EquipCard/BindCard
///                 `stash_idx`→`album_idx`, `#[serde(alias="stash_idx")]` for old input);
///                 `unequip_card` returns to the album and no-ops (keeps the card equipped)
///                 only if the album is full with no stack to merge — a panel card can
///                 never be lost (fixes "unequip with full stash deletes the card").
///                 `migrate_save_items` sweeps any card a pre-album save left in the stash
///                 into the album. Bridge: view_state strips `card_album` to a count +
///                 injects `card_album_cap`; new `card_album_json()` getter. New fields are
///                 serde-skip (empty Vec / 0) → card-less states byte-identical; card
///                 routing changed but no card banks during any golden scenario → goldens
///                 byte-identical (NO re-bless).
/// 0.0.128-alpha — TWO fixes from the codebase review:
///                 (1) BOSS SPAWN honours the FULL template: `try_spawn_boss` used to
///                 hand-build the entity and silently dropped `mdef`/`hit`/`flee`, the
///                 `atk_min..atk_max` variance band and the catalog `skills` — every boss
///                 fought with 0 mdef, formula-fallback hit/flee, FLAT atk and no skills
///                 (chomboom's zone boss never self-destructed). Now routed through
///                 `spawn::build_and_spawn_monster` (same constructor as regular spawns +
///                 the minion ring) with `is_boss` flagged after. Bosses are HARDER than
///                 before — band pacing was tuned against the weaker boss, re-check
///                 boss walls. Goldens: only alpha6 runs through a boss gate (atk
///                 variance also adds per-swing draws there) — re-blessed alpha6 only.
///                 (2) FOIL CARDS keep their own album stack: album merge was by def_id
///                 only, so a foil drop folded onto the plain stack (foil bit erased) or —
///                 if the first-banked copy was foil — later plain dups inherited foil on
///                 equip (equip clones the ROW). New `same_card_stack` (def_id + foil +
///                 star) gates `bank_card_to_album` AND the `unequip_card` merge guard;
///                 each variant is its own row/type-slot. `star` is in the key now so fuse
///                 leveling won't hit the same wall. No PRNG/shape change; goldens have no
///                 foil banking → unaffected by (2).
/// 0.0.129-alpha — CHUM CARD SET (data + docs only, no code): 4 cards mapped to the 4
///                 distinct chom KINDS — card_chomb (Weapon: atk+crit), card_toxic_chom
///                 (Earring: flee), card_burst_chom (Accessory: atk+12/def−3 glass cannon,
///                 first drawback card), card_rechum (Shoes: hp_pct 300bp + int — first
///                 %HP card, rides the RO HP curve). Elite/boss (chomber/chomboom) drop
///                 the SAME card_chomb at rate-up (1/10k, 1/47) instead of own cards —
///                 boss = card-farm source convention. chomb's placeholder card_yellowling
///                 drop swapped to card_chomb; the 5 non-chomb family tables scaled ×5
///                 (ratios preserved) so a weight-1 card entry lands ≈1/20k. Star-upgrade
///                 (5★) DESIGN pinned in docs/card-system.md (fuse costs, ×3.5 flat /
///                 ×2.5 pct scaling, foil fuses separately, drawbacks don't scale) — NOT
///                 implemented. Balance-only data change; goldens stay in bands 1-2 →
///                 byte-identical (NO re-bless).
/// 0.0.130-alpha — SOCKETED GEAR VARIANTS (data only): first non-demo/non-unique card
///                 socket source — "[1]" defs for 12 gear lines × 7 tiers = 84 items +
///                 "[2]" defs for 11 lines (no shield_2s — Shield panel caps at 1) = 77,
///                 each a faithful stat-clone of its base (sell ×1.5 / ×2). Sockets live
///                 in the DEF (hole-punch = future def-swap; docs/card-system.md) so
///                 clean gear stays market-fungible. Seeding: [1] beside every base-gear
///                 drop at weight/8 (687 entries) — trash everywhere; [2] on BOSS tables
///                 only (209 entries, combined ≈1/28 per boss kill). Socket matrix:
///                 1H 0-3 / 2H 0-4 / off-hand counts min(slots,1) / others 0-2 / shield
///                 0-1; [3]+[4] = same dungeon-era shelf (archetype-completion parity).
///                 Every card class can finally go ACTIVE. Drop tables changed → goldens
///                 rolling gear drift; re-blessed after verifying only drop-identity
///                 moved (draw order untouched).
/// 0.0.131-alpha — CARD RATE ×5 RARER + EVENT/TEST MULTIPLIER: trash card drops
///                 retuned 1/20k → ~1/100k (elite ~1/61k, boss chomboom ~1/200) —
///                 volume rule: at ~2.2k kills/h idle, 1/100k ≈ RO's classic 0.01%
///                 in wall-clock feel (~45h/card). New `GameState.card_rate_bonus_bp`
///                 (serde-skip-0) scales Card entry weights at roll time
///                 (`apply_card_rate_bonus` — cloned table, single-draw contract
///                 untouched); set via the RECORDED `InputEvent::SetCardRateBonus`
///                 so tests + live drop-rate events replay bit-identically. sim-cli
///                 `--card-rate-mult N`. Multiplier layers compose multiplicatively
///                 (`combined_card_rate_bp` — event today; item-buff/path layers slot
///                 in later, all in recorded state). Goldens: b1 card weights changed
///                 → alpha6 re-blessed.
/// 0.0.132-alpha — GEAR STAR ROLL at drop: equipable gear (Weapon/Armor kinds,
///                 Legendary/uniques excluded) rolls `Item.star` 0-3 on drop —
///                 trash 75/18/6/1%, boss 40/30/20/10% (drafts; loot audit
///                 recalibrates). Stars = future bonus slots (gear-systems-design
///                 §1; 4-5★ upgrade-only, orb/bonus systems come in Update 1 —
///                 stars are visible-but-dormant potential until then). ONE extra
///                 state-keyed conditional PRNG draw (same contract as the foil
///                 roll) → goldens with gear drops drift; re-blessed after verify.
///                 `star` field already existed (cards) → save shape unchanged.
/// 0.0.133-alpha — GENERIC ACCESSORY SLOTS (RO-style, user 2026-07-05): the two
///                 accessory slots are interchangeable — two rings, two pendants,
///                 or one of each. Def Necklace/Ring = "home" only; equip
///                 prefers the item's own stamp, else the other slot if free, else
///                 swaps its home occupant. Placed slot stamped on the instance
///                 (dual-wield WeaponLeft pattern; migration already preserves
///                 equip_slot). Also boss [2]-variant rates retuned 1/28 → ~1/220-
///                 340 per boss kill (early-game bosses spawn ~1/min — the per-kill
///                 rate must price in real boss FREQUENCY per band; loot-audit
///                 lesson). No shape change; goldens: alpha6 re-blessed (boss
///                 tables).
/// 0.0.134-alpha — GEAR-MOD EFFECT LAYER, all 24 mods live (orbs/rolling = next):
///                 `Item.bonuses: Vec<StatMod>` (serde-skip-empty) + one additive
///                 bucket per family in `combat::stat_mods`. Wired: stats→
///                 buffed_stats; HP flat/%, ATK%/MATK%, HIT/FLEE/CRIT → derived;
///                 dmg-vs element/race/size (attacker, pre-mitigation) + Ignore
///                 DEF/MDEF (packet-stamped, cuts both def layers / mdef) +
///                 defender Resist-element (≤75%) + DR vs race/size/ranged (≤50%)
///                 → damage pipeline; CDR ≤30% (standard cast path), Cast −% ≤50%
///                 (whole windup, floor 1), MoveSpeed ≤30% (existing pct bucket),
///                 Heal +%, Status resist = DURATION cut ≤75% (deterministic — no
///                 block roll, zero extra draws) central in apply_buff. Entities
///                 gained race/size (players DemiHuman/Medium via with_team;
///                 monsters from template ← def strings). All fields serde-skip-
///                 default; no bonuses exist in data yet → goldens byte-identical.
/// 0.0.135-alpha — RARITY FOLLOWS STARS + MYTHICAL/IMMORTAL TIERS + ULTRA-RARE 4★:
///                 `Rarity::Mythical` (between Rare and Legendary) and
///                 `Immortal` (top — Dota 2 ladder, Immortal before Arcana;
///                 Arcane reserved for a future higher tier; serde by-name →
///                 old saves unaffected).
///                 Dropped gear's display/routing rarity promotes to
///                 `max(def, star_rarity)` — user's ladder: 1★ Uncommon, 2★
///                 Rare (→ auto-routes to the rare bag; its first live faucet),
///                 3★ Mythical, 4★ Legendary, 5★ Immortal. Star tables went
///                 percent → bp with a 4★ tail ("ยากมากๆ"): trash 1/10,000,
///                 boss 1/1,000; 5★ stays upgrade-only. Socket-variant DEFS get
///                 base rarity in data: `*_1s` Uncommon (84), `*_2s` Rare (77).
///                 Migration recomputes the promotion from the preserved star
///                 (pure map, zero draws) so old starred items recolor on load.
///                 NEW `unique` TAG on ItemDef/Item ("Legendary is not unique"):
///                 unique-ness is its own axis — `star_eligible` keys off the
///                 tag now, so a plain Legendary-rarity drop rolls stars like
///                 any gear; the 7 `unique_*` defs are tagged in data.
///                 Same single star draw per eligible drop, but the wider table
///                 shifts outcomes → only alpha6 drifted (verified, re-blessed).
/// 0.0.136-alpha — ROLL-AT-DROP ENCHANTS (user: "ให้ roll stat ลงไปเลย" — drops
///                 arrive fully enchanted; players only engage with crafting
///                 after investing, so orbs become REROLL tools later): new
///                 `gear_roll` module rolls ALL of a drop's star slots at drop
///                 time from the §2 pool — 29 families, slot-gated by paper
///                 doll (Ignore→weapon, Heal%→staff/mace weapons only ("แค่
///                 staff + mace ที่ acolyte ใช้ได้"), Cast→head, CDR→accessory,
///                 DR-race/size→armor+shield, status-resist→armor+garment,
///                 ranged-DR→garment, MS→shoes), tier window from the `_sN` badge
///                 (s1-4 T1 / s5-8 +T2 / s9+ +T3, weights 60:30:10), no dup
///                 family per item, MaxHp ×band. Draws (family/tier/param/
///                 value, fixed order) taken ONLY on starred drops — state-
///                 keyed, replay-safe. Effects already live (0.0.134). Pre-
///                 0.0.136 starred items stay unenchanted (migration never
///                 draws). Dmg-vs stacking REWORK in the same pass: within an
///                 axis lines still ADD, but element x race x size MULTIPLY
///                 across axes (+20% each = x1.728 — user: "เราจะ * เพราะเป็น
///                 กึ่ง incremental เกม"), and gear ATK%/MATK% became their own
///                 MULTIPLICATIVE layer over the buff+card % bucket ("atk% ก็
///                 น่าจะคูณ") — the semi-incremental rule: sources add within a
///                 layer, layers multiply. %-of-base stat buffs (Sharp Focus /
///                 Quicksand) now round AWAY FROM ZERO — truncation made the
///                 buff invisible on small bases (AGI 16 × 6% → +0; user:
///                 "ใช้แล้วไม่ + stat ให้เห็น"); non-zero percent always moves
///                 the stat ≥1. STAR BASE-STAT FLOOR (user: "ดาวเยอะ + base
///                 stat ไปด้วยเลย ไม่งั้น... ได้แล้วซึม"): each star adds
///                 +10% of the piece's OWN base ATK/MATK/DEF (ceil, derived at
///                 read time from `star` — never baked, no migration/PRNG
///                 concerns) → a high-star drop beats its 0★ copy even when
///                 the rolled lines miss the build. Folds in effective_atk/
///                 matk/hard_def + the bridge equip split + tooltips
///                 (star_atk/star_matk/star_def view keys). ITEM LEVEL + EQUIP
///                 REQ (user: "S1 = item level : 1"): the `_sN` badge surfaces
///                 as `Item::item_level()`; equip gates at (iLv−1)×10 (s1=Lv1
///                 so starters never gate, s2=Lv10 … s7=Lv60 — ~15-20 lv under
///                 each tier's natural drop band, so self-farmers never hit it;
///                 it only tempers market twinking). Under-level equip = silent
///                 no-op; tooltip shows "Item Lv." + "Requires Lv." DATA:
///                 -ling card drops remapped to SELF (pinkling family →
///                 card_pinkling, yellowling → card_yellowling — was crossed
///                 placeholder wiring; card_yellowling previously unreachable).
///                 alpha6 re-blessed (drop templates live inside spawned
///                 entities → hash moves); all other goldens byte-identical.
/// 0.0.137-alpha — AUDIT-DRIVEN RETUNE (loot-per-day audit 2026-07-05: 33.6k
///                 kills + 1,358 boss/day measured via a 24h sim-cli run):
///                 (1) chomboom card_chomb weight 97 → 10 (1/195 → ~1/1,900
///                 per boss; was flooding ~7 cards/day — card scarcity is the
///                 market centerpiece); (2) rare-bag DEFAULT threshold Rare →
///                 Mythical (2★/[2] at the old default poured 15-23 drops/day
///                 into a 3-slot bag — full in ~4h; Mythical = 3★+ ≈ 3/day,
///                 the daily-jackpot bag the slots were sized for; saves with
///                 an explicit non-default threshold keep theirs). Goldens
///                 re-blessed where drifted after verifying the cause.
/// 0.0.138-alpha — STAR STONES (orb system Patch-1 set, orb-system-design.md):
///                 8 stones live — Green→Red (StarUp 0★→5★, new line rolls on
///                 the spot + rarity/floor follow), Chaos (RerollAll — the
///                 Death-locked line survives verbatim and reserves its
///                 family), Death (LockRandom — reusable, re-draws which line
///                 is locked, one lock per item), Shiny (RerollValues — number-
///                 only reroll inside each line's inferred tier; locked line
///                 skipped). New `OrbEffect` on ItemDef/Item + `locked_slot`
///                 (per-instance, migration-preserved) + `InputEvent::UseOrb
///                 { orb_idx, target: Stash|Equipped }` — refine's contract:
///                 validate (zero draws) → Gil fee + consume one stone →
///                 draws in fixed order. Fees: Chaos 500×iLv · StarUp 5000×
///                 targetStar×iLv · Death 2000×iLv · Shiny 1500×iLv. DATA:
///                 8 defs + drop seeding at the approved per-day targets
///                 (audit-calibrated vs PoE2 ratios): trash Chaos ~3/Green 4/
///                 Blue 1.5 per day; boss Purple .4/Yellow .14/Red .03/Death
///                 .5/Shiny .3 per day (tables rescaled where totals were too
///                 small — probabilities of existing entries unchanged).
///                 `roll_bonuses` refactored through `roll_bonuses_keep(None)`
///                 — draw-for-draw identical (goldens proved it pre-seed);
///                 alpha6 re-blessed for the drop-table additions only.
/// 0.0.139-alpha — SKILL RUNE foundation + BARRAGE line (skill-rune-design.md):
///                 augments an EXISTING skill of the wielder while a runed WEAPON
///                 is held (distinct from Skill Orb, which ADDS a skill). New
///                 `Item.skill_rune: Option<SkillRune{skill_id, line, level}>`
///                 (serde skip-none, migration-preserved) + `RuneLine` enum +
///                 `Entity::weapon_rune()` resolver (reads Weapon/WeaponLeft).
///                 BARRAGE: a runed skill fires on the swing timer bypassing its
///                 cooldown while charges remain (reuses `charges_left`); charges
///                 hit 0 → cooldown starts; idle refill (`refill_barrage_charges`,
///                 pre-attack-loop) tops it back to tier+1 (2/3/4) → ASPD now
///                 drives burst speed. No runes exist anywhere yet → every path
///                 gated on `weapon_rune`, goldens byte-identical (verified).
///                 SCATTERSHOT line + roll-at-drop + Rune Stones = next.
/// 0.0.140-alpha — SKILL RUNE roll-at-drop (skill-rune-design §3): a rune-
///                 eligible WEAPON (a type with authored lines — only Bow →
///                 Twin Strafe now) has a ~1% chance to drop with a rune
///                 (`gear_roll::roll_skill_rune`, after the enchant roll).
///                 Draw order present→level→line, taken ONLY for such weapons
///                 (state-keyed, replay-safe) so other drops are byte-identical;
///                 levels 82/15/3 (lvl 3 very rare). No golden drops a bow →
///                 goldens byte-identical (verified). Rate is a draft — tune via
///                 the loot-per-day audit. Rune Stones + UI = next.
/// 0.0.141-alpha — CLASS RUNE STONE (insert): `OrbEffect::InsertRune { class }`
///                 inserts a random level-1 skill rune into a rune-less weapon
///                 the class wields (Archer Rune Stone → Bow → Twin Strafe).
///                 OrbEffect lost Copy (String field) → fee/applies take
///                 `&OrbEffect`, use_orb clones the effect (Star Stone tests
///                 unchanged). Class-gated via `default_weapon_classes`;
///                 gear_roll::insert_rune picks a random line (fee 3000×iLv).
///                 DATA: `rune_stone_archer` def (shop 50k). Goldens byte-
///                 identical (no runes/stones in any scenario, verified).
/// 0.0.142-alpha — RUNE TIER ↔ WEAPON REFINE (user 2026-07-06): a rune's
///                 EFFECTIVE tier = max(drop level, refine tier) via
///                 RUNE_REFINE_THRESHOLDS (+7 → lvl 2, +10 → lvl 3) — refine
///                 becomes the reliable path to a higher tier (drop lvl is a
///                 shortcut), giving refine real purpose. magnitude()/slot_cost()
///                 now take refine_level; `Entity::weapon_rune_at` returns the
///                 rune + the weapon's +N. The "upgrade Rune Stone" is dropped
///                 (refine replaces it). Existing tests unchanged (lvl-2 test
///                 runes clamp the same); goldens byte-identical (verified).
/// 0.0.143-alpha — RUNE STONE drop rate (user 2026-07-06: rarer than a weapon
///                 that drops already-runed): `rune_stone_archer` seeded into
///                 every boss table at ~1/13,500 per boss (≈0.1/day, tier C).
///                 DATA only. alpha6 golden re-blessed (its b2 boss table gained
///                 the entry → drop stream shifts); rest byte-identical.
/// 0.0.144-alpha — RUNE → LEGENDARY rarity floor (user 2026-07-06: "ทำเป็น
///                 legendary"): a runed weapon (rolled-at-drop OR insert-stoned)
///                 floors `rarity` at Legendary — it reads as the build-defining
///                 jackpot it is + routes to the rare bag, instead of showing
///                 its low star rarity. Stars stay a separate axis (a 0★ runed
///                 weapon still takes StarUp; the floor holds via max). Goldens
///                 byte-identical (no runes in any scenario).
/// 0.0.145-alpha — BAND CASCADE (data): a new GENTLER band 7 inserted (mob hp
///                 1352 / atk 125, bridging b6 1058/100 → old-b7 1727/157 which
///                 the b6→b7 spike jumped); every band b7-19 shifted up one to
///                 b8-20 (zones + monsters + all refs). Bands now 1-20. Golden
///                 scenarios (b1-2) untouched → byte-identical (verified).
/// 0.0.146-alpha — MONSTER max-HP fix: `effective_max_hp` now returns a
///                 MONSTER's authored `max_hp` verbatim instead of applying the
///                 PLAYER VIT/level/curve scaling — the STAT_MIN=1 default VIT
///                 inflated it ~1%, so a boss HP bar never read full at spawn
///                 and its "max" didn't match the DB number. Pets are
///                 Team::Player → unaffected. No golden's combat depended on the
///                 1% → byte-identical (verified).
/// 0.0.147-alpha — STALE-MONSTER HP clamp on load: a monster serialized before
///                 a stat re-tune (e.g. the band cascade) kept its old hp, which
///                 can exceed the new max → "hp > max" on the boss bar. Migration
///                 now clamps every MONSTER's hp to its current effective max on
///                 load (players were already clamped). Paired with a client
///                 clamp so the boss bar never displays hp > max. Byte-identical
///                 (no monsters in golden saves change).
/// 0.0.148-alpha — SWORDSMAN skill runes (skill-rune-design §5 pool expansion):
///                 `bash` gains two lines — Barrage (reuses the generic charge
///                 path, skill-agnostic) and Cleave (a new RuneLine variant that
///                 splashes 50/75/100% of the hit to the target's 4 orthogonal
///                 neighbors, reusing `apply_cleave_splash`). Sword1H AND Sword2H
///                 are now rune-eligible (the swordsman wields both). New item
///                 `rune_stone_swordsman` (class-gated insert, mirrors the archer
///                 stone) added to the shop + all 20 boss drop tables. alpha6
///                 golden re-blessed: swords now take the ~1% rune draw at drop +
///                 the swordsman stone widened the boss table weights — both
///                 INTENTIONAL (verified the drift is only these two).
/// 0.0.149-alpha — ACOLYTE skill runes: `holy_light` gains Radiance (line A —
///                 the multi-target fan, shares the Scattershot resolve) and
///                 Benediction (line B — on cast, heal the most-injured ally
///                 within a fixed BENEDICTION_HEAL_RANGE=6 for 50/75/100% of the
///                 damage dealt). Two new RuneLine variants. Staff AND Mace are
///                 now rune-eligible (both cast holy_light; Staff will also carry
///                 mage `fireball` lines once authored). New `rune_stone_acolyte`
///                 in the shop + all 20 boss drop tables. alpha6 golden re-blessed
///                 (Staff/Mace take the rune draw + the stone widened boss weights
///                 — both INTENTIONAL).
/// 0.0.150-alpha — MAGE skill runes + BURN status + rune class-filter foundation:
///                 `fireball` gains Fusillade (line A — ×1.6/2.4/3.2 damage shown
///                 as N DISPLAY hits, one real lump) and Combustion (line B — hit
///                 enemies BURN). New `BuffKind::Burn`: a LINEAR-DECAY DoT, peak =
///                 20%×tier of the hit/s, value = peak×ticks/BURN_DURATION (100t =
///                 10 s); re-applying STACKS onto the current decayed value + resets
///                 the duration (user: 100 → 50 after 5 s → +100 = 150). Projectile
///                 gains a serde-skip `burn_tier` (transient, golden-safe). Staff is
///                 now SHARED (fireball + holy_light lines); `insert_rune` gained a
///                 CLASS filter (via `tree_node`) so a Class Rune Stone on a shared
///                 weapon inserts only its own class's rune — with `has_insertable_
///                 rune` gating `orb_applies`. New `rune_stone_mage` in the shop +
///                 all 20 boss tables. alpha6 re-blessed (Staff line count 2→4 + the
///                 stone widened boss weights — both INTENTIONAL).
/// 0.0.151-alpha — THIEF skill runes complete the first-job rune set (5/5 classes):
///                 `shadow_strike` gains Flurry (line A — the Barrage charge burst
///                 shared skill-agnostically, but each strike hits for a flat 70%
///                 via FLURRY_POWER_BP) and Execute (line B — a non-boss left below
///                 30/40/50% (tier) HP after the strike is killed outright; bosses
///                 spared via is_boss). Dagger (thief-only) is now rune-eligible.
///                 New `rune_stone_thief` in the shop + all 20 boss tables. alpha6
///                 re-blessed (Dagger takes the rune draw + the stone widened boss
///                 weights — both INTENTIONAL). All six WeaponTypes now carry rune
///                 lines; only the typeless weapon has none.
/// 0.0.152-alpha — BALANCE: Arrow Shower cooldown 24t (2.4s) → 50t (5s) — a 160%
///                 3×3 AoE firing every 2.4s out-DPS'd + outpaced Twin Strafe
///                 (5s single-target); now a deliberate periodic AoE, not a spam.
///                 Doc (was a stale 16t) synced. Goldens byte-identical.
/// 0.0.153-alpha — Rune Stones may now apply to UNIQUE weapons (a unique can
///                 CARRY a rune even though it never rolls one at drop). Moved the
///                 `item.unique → false` gate in `orb_applies` to AFTER the
///                 InsertRune arm, so star/enchant stones still can't touch a
///                 unique's fixed enchants but a Class Rune Stone can rune a
///                 rune-less unique (the shop's unique weapons were unrunable). The
///                 UI greys via this same fn (bridge orb_check) → fix is end-to-end.
///                 Goldens byte-identical (no golden orbs a unique).
/// 0.0.154-alpha — VARIABLE CAST LEVEL (sim core): the mage bolts (fire/lightning/
///                 frost/earth_piercer) now cast at a PLAYER-CHOSEN level via the new
///                 `SkillState.cast_level` (serde skip-if-0 → old saves/goldens byte-
///                 identical; 0 = cast at the learned level). A bolt's cast time =
///                 5 ticks × cast_level (0.5 s/level) and hits = cast_level — RO's
///                 speed-for-hits trade (NO SP: damage skills are free, the game
///                 reserves slots not SP). Replaces the old fixed 15t bolt cast. New
///                 `InputEvent::SetCastLevel {entity_id, skill_id, level}` (clamped
///                 to learned; bridge deserializes it generically). Goldens byte-
///                 identical (a Lv3 bolt is 5×3=15t = the old fixed value, and no
///                 golden casts a bolt off Lv3). UI level-selector still to come.
/// 0.0.155-alpha — RUNE BATCH 2 (swordsman): `ignite_break` gains Cinders (line A —
///                 the blast BURNS survivors 20/40/60%, reusing apply_combustion_burn)
///                 and Concussion (line B — the knockback also STUNS non-boss victims
///                 2/2.5/3 s via `concussion_stun_ticks`, reusing BuffKind::Stun with
///                 an is_boss guard). Two new RuneLine variants. Sword1H/Sword2H now
///                 carry bash AND ignite_break lines (the swordsman stone inserts
///                 either via the class filter — no data change). alpha6 re-blessed
///                 (sword rune line count 2→4 shifts the drop-roll line pick).
/// 0.0.156-alpha — RUNE BATCH 2 (archer): `arrow_shower` gains Focus (line A — the
///                 blast's TOTAL damage ×2.25/3.00/3.75 but SPLIT evenly among the
///                 victims, so a lone target eats it all) and Pinning (line B —
///                 hit enemies SLOWED 20/30/40% for 3 s, reusing AgiDexSlowPct). Two
///                 new RuneLine variants + two serde-skip Projectile fields
///                 (focus_mult_bp / pin_slow_pct, resolved at impact). Bow now
///                 carries twin_strafe AND arrow_shower lines. alpha6 re-blessed
///                 (Bow rune line count 2→4).
/// 0.0.157-alpha — RUNE BATCH 2 (mage) + SKILL-GROUP foundation + chain rework:
///                 the three bolts (fire/lightning/frost) share ONE "bolt" group
///                 rune — Arc (line A, a CHAIN) and Overload (line B, +1/2/3 bolts/
///                 cast). Foundation: `rune_skill_key` maps a bolt cast → the "bolt"
///                 rune at resolve; `class_can_rune_skill` lets the class filter
///                 treat "bolt" as learnable if the class learns any bolt member;
///                 `overload_bonus_hits` adds the extra bolts at cast. Renamed
///                 `RuneLine::is_fan` → `is_chain` (user: "fan reads as a cone").
///                 The chain (Scattershot/Radiance/Arc) now JUMPS nearest-first —
///                 each jump lands 1 tick later; damage-decay is WIRED but OFF for
///                 now (100%, tunable). Staff carries fireball + holy_light +
///                 bolt lines. alpha6 re-blessed (Staff line count 4→6). No golden
///                 has a chain rune → the delay/decay is byte-identical.
/// 0.0.158-alpha — RUNE BATCH 2 (thief): `open_wound` gains Vulnerable (line A —
///                 marks the target to take +10/15/20% damage from ALL sources) and
///                 Blood Mist (line B — target + enemies within 1/2/3 cells lose 20%
///                 HIT for 5 s). Two new BuffKind: DmgTakenPct (mirror of the ward,
///                 applied at the end of apply_damage) and HitDownPct (folded into
///                 effective_hit_points). Both gated on the buff → goldens byte-
///                 identical. Two new RuneLine variants; Dagger carries shadow_strike
///                 AND open_wound lines.
/// 0.0.159-alpha — RUNE BATCH 2 COMPLETE (acolyte, 5/5 classes): `heal` gains
///                 Retribution (line A — the heal also SMITES the nearest enemy for
///                 100/150/200% of the heal as HOLY magic, strong vs shadow) and
///                 Renewal (line B — the healed ally gains a HealHot of 10/20/30%
///                 of the heal per second for 5 s). Hooked in try_cast_support after
///                 the heal (gated on the "heal" rune → goldens byte-identical).
///                 Staff + Mace carry heal lines. Every first-job class now has TWO
///                 runed skills (10 batch-2 lines authored: Cinders/Concussion,
///                 Focus/Pinning, Arc/Overload, Vulnerable/BloodMist, Retribution/
///                 Renewal).
/// 0.0.160-alpha — LEVEL_CAP=70 + fixed XP TABLE. (1) Base character level now caps
///                 at LEVEL_CAP=70 (user: "level cap 70, band ไม่ cap") — `try_level_up`
///                 parks base XP at 0 once level ≥ cap, mirroring JOB_LEVEL_CAP. Bands
///                 are NOT gated: a capped player can still push high bands. Expansion
///                 = raise the cap alone (80→90→99). (2) `xp_to_next` is now a FIXED
///                 per-level lookup `XP_TABLE[level]` (user: "ตารางเลเวลใช้ fixed value
///                 แทนสูตร") instead of the 70·L²+10·L³ formula — each level is an
///                 independent tuning knob. Table SEEDED from that exact formula so the
///                 curve is byte-identical; ≥99 falls back to the formula. No golden
///                 reaches Lv70 → all 11 goldens byte-identical (verified). sim-cli
///                 14-day run now tops out at Lv70 (was Lv86).
/// 0.0.161-alpha — S0 replay-hole #1: class change is now an EVENT. `set_player_class`
///                 (bridge) was a DIRECT GameState mutation — untracked, so replay
///                 diverged. Now: (1) new `InputEvent::ChangeClass { unit_def }`;
///                 (2) the 75-line swap logic moved out of the bridge into
///                 `data::change_class`, rebuilt on `build_player_entity` so a swapped
///                 class gets CORRECT per-level HP/SP factors (the bridge hand-copy
///                 omitted them — latent bug) and a fresh class its starting weapon;
///                 (3) `apply_input` split into a data-free wrapper + `apply_input_with(
///                 state, Option<&GameData>, event)` so data-dependent events resolve
///                 catalog defs — ONE enum, ONE log (replay model intact). Data-free
///                 events + all 69 legacy `apply_input` call sites unchanged →
///                 all 11 goldens byte-identical (verified). First of the S0
///                 replay-hole sweep (docs/server-plan-phases.md); zone/recruit/buy/
///                 advance/deploy/bench/grant_loot still direct — next passes.
/// 0.0.162-alpha — S0 replay-hole #2: ZONE. (1) `current_zone_id: Option<String>`
///                 now lives ON GameState, stamped by `populate_zone` (the single
///                 chokepoint every zone load funnels through: new-game + change_zone).
///                 Kills the bridge's grid-dimension recovery heuristic (two zones
///                 can share a size → wrong guess); the bridge field is removed and
///                 `current_zone()` reads the authoritative state field. load_json
///                 keeps the heuristic ONLY as a pre-0.0.162 save migration (field
///                 absent → recover once). (2) Player-initiated zone changes are now
///                 events: `RespawnInZone`, `SelectBand { band }`, `RestartInstance`
///                 (bridge respawn/select_band/restart_instance route through
///                 apply_input_with). The TICK-driven advance (pending_zone_change →
///                 change_zone) stays in the driver — deterministic, not free input.
///                 SCHEMA CHANGE: `current_zone_id` is serialized (skip-if-none).
///                 alpha6 re-blessed (its grassland run loads a real zone → field set);
///                 the other 10 goldens hand-build via GameState::new (no populate_zone
///                 → field stays None → skip → byte-identical). 303 unit tests.
/// 0.0.163-alpha — S0 replay-holes COMPLETE: the last 7 bridge direct-mutations are
///                 now events. RecruitMember / AdvanceClass / BuyCharacter /
///                 DeployCharacter / BenchCharacter (party) + BuyItem / GrantLoot
///                 (items). Each handler just calls the SAME `data::*` fn the bridge
///                 used to call directly — logic does NOT move, so all 11 goldens stay
///                 byte-identical (no re-bless). The bridge methods route through
///                 apply_input_with and recover their UI return value from a state
///                 delta (party grew/shrank, Gil spent, unit_def changed, new roster
///                 key) since events don't return values. GrantLoot is TRANSITIONAL
///                 (3h-claim remnant, → debug once the claim UI is removed). With #1
///                 (ChangeClass) + #2 (zone), ALL 11 audited replay holes are closed:
///                 every GameState mutation is now an InputEvent, so the future event
///                 log (S1) captures the full save = (seed + state + events). 304 unit.
/// 0.0.164-alpha — S1a: EVENT-LOG RECORDING + replay verify (GDD §4.4). Now that
///                 every mutation is an InputEvent (S0), `apply_input_with` records
///                 each `RecordedInput { tick, event }` into `GameState.input_log`
///                 when `record_inputs` is on. BOTH new fields are `#[serde(skip)]` —
///                 the log is a SEPARATE save artifact, never in the state snapshot
///                 or its hash, so goldens stay byte-identical (306 unit, 11 goldens).
///                 New `replay` module: `advance(state, data, n)` is the ONE tick-
///                 driver step (tick + pending_zone_change) shared by the live bridge
///                 `tick()` AND `replay(initial, data, log, final_tick)`, so a re-run
///                 can't diverge. Test `replay_reproduces_a_recorded_session` plays a
///                 session (ticks + SelectBand + SetFarmMode + ChangeClass) and proves
///                 the log replays to an identical state_hash — the end-to-end payoff
///                 of the S0 hole-closing. Recording defaults OFF (sim-cli/goldens
///                 stay lean); the bridge turns it on at new-game/create/load. Still
///                 TODO (S1b): export_log() + start-snapshot capture + snapshot/hash-
///                 chain container (GDD §4.4 save = {header, seed, gd_version, log[],
///                 snapshots[]}).
/// 0.0.165-alpha — S1b: SaveBundle (self-verifying replay save) + bridge export_log().
///                 `replay::SaveBundle { sim_version, game_data_version, start, log,
///                 final_tick, final_hash }` — `capture` builds it from a session's
///                 start checkpoint + live end state; `verify(data)` replays the log
///                 over `start` and checks the end hash (the server's Model C anti-
///                 cheat check, total/never-panics). Bridge gains a `session_start`
///                 checkpoint (cloned at new-game/create/load) + `export_log()`
///                 emitting the bundle JSON. Test proves JSON round-trip + verify +
///                 tamper-rejection. Serde-skip fields keep goldens byte-identical
///                 (307 unit, 11 goldens). Single start snapshot for now; a periodic
///                 mid-session snapshot CHAIN (server resume w/o full replay) is S1c.
///                 GDScript persist/verify wiring (write bundle to user://) pending.
/// 0.0.166-alpha — S1c: hash-chained SNAPSHOT chain (GDD §4.4). `replay::advance`
///                 appends a `Snapshot { tick, state_hash, chain_hash }` every
///                 `snapshot_interval` ticks (bridge sets SNAPSHOT_INTERVAL_TICKS =
///                 6000 = 10 min; 0 = off for sim-cli/goldens). `chain_hash =
///                 hash::chain_link(prev_chain, tick, state_hash)` binds each link to
///                 the last, making the checkpoint sequence tamper-evident. SaveBundle
///                 carries the chain; `verify` now regenerates it by replaying and
///                 checks BOTH the final hash AND the whole chain — catching a forged
///                 MID-session state that leaves the endpoints intact. New fields are
///                 `#[serde(skip)]` (snapshot_interval, snapshots) → hashing the state
///                 never sees the growing chain → goldens byte-identical (307 unit).
///                 Test: chain verifies after JSON round-trip; forged end / forged mid
///                 checkpoint / dropped checkpoint all rejected. Remaining: GDScript
///                 wiring (persist bundle to user://, verify on load) + full-state-per-
///                 snapshot RESUME (skip replay from tick 0) — server-side, later.
/// 0.0.167-alpha — SECURITY: input-authority taxonomy + cap the card-rate hole.
///                 Red-teaming the replay-verify model surfaced that it proves
///                 "state follows from inputs" but NOT that inputs are legitimate —
///                 so safety reduces to "is every InputEvent safe to author?".
///                 `SetCardRateBonus { bp }` was UNCAPPED (a forged bp=u32::MAX →
///                 cards drop every kill → mints tradeable wealth via a VALID log).
///                 Fix: (1) new `InputEvent::authority() -> InputAuthority
///                 {Client, Server, Debug}` — SetCardRateBonus = Server (a global
///                 drop-event multiplier only a server grant may set; a verifier
///                 replays its own value + rejects a client-forged one), GrantLoot =
///                 Debug (3h-claim remnant, must not appear in a production log),
///                 all else Client (self-cost-gated → forging is a no-op/paid).
///                 (2) handler now clamps bp to `drops::MAX_CARD_RATE_BONUS_BP` (×10)
///                 as defense-in-depth. sim-cli (trusted balance tool) sets the field
///                 DIRECTLY, bypassing the cap. `combined_card_rate_bp` documented as
///                 multi-source: server-event multiplier + future cost-gated Bubble-
///                 Gum-style buffs + soul-path. 309 unit (cap/authority tests); 11
///                 goldens byte-identical (no golden exceeds the cap).
/// 0.0.168-alpha — RENAME `GameState.stash` → `inventory` (the party main bag) to
///                 free the name "stash" for the incoming DEPOSIT WAREHOUSE. Pure
///                 mechanical rename: field + helpers (`STASH_CAPACITY` →
///                 `INVENTORY_CAPACITY`, `stash_cap` → `inventory_cap`,
///                 `bank_to_stash*` → `bank_to_inventory*`). Field carries
///                 `#[serde(alias = "stash")]` so old local saves still load. Event
///                 WIRE FORMAT preserved untouched (`SellStashItem`, `stash_idx`,
///                 `MoveRareToStash`) — replay/SaveBundle logs stay valid. Behavior
///                 identical; only the serialized field KEY changed, so all 11 goldens
///                 drifted by hash and were re-blessed (310 unit unaffected). NOTE:
///                 the key change invalidates pre-0.0.168 server anchor/bundle hashes
///                 — fine in alpha (no real players), MUST NOT rename post-launch.
/// 0.0.169-alpha — STASH WAREHOUSE (deposit store) + card-flow rework. New
///                 `GameState.stash` (the "Stash" — a large upgradeable deposit
///                 warehouse, base `STASH_CAPACITY = 100`) parallel to `inventory` /
///                 `rare_bag` / `card_album`; skip-if-empty so goldens stay
///                 byte-identical. Player-driven transfers (all Client-authority,
///                 replay-covered): DepositToStash / WithdrawFromStash (item ⇄
///                 warehouse) + DepositCardToAlbum / WithdrawCardFromAlbum (card ⇄
///                 album). A deposit/withdraw into a FULL destination restores the
///                 item (never destroyed); cards stack by `same_card_stack` so a Foil
///                 never folds onto a plain copy. CARD FLOW REWORK: dropped cards now
///                 land in the INVENTORY (was auto-routed to the album) → the player
///                 deposits them by hand; the album stays the EQUIP source (unequip
///                 returns there via bank_card_to_album). Removed the load-time
///                 inventory→album card sweep. Also renamed the stale event names left
///                 by the stash→inventory field rename — SellStashItem →
///                 SellInventoryItem, MoveRareToStash → MoveRareToInventory, their
///                 `stash_idx` → `inv_idx` — each with `#[serde(alias)]` so old replay
///                 logs stay valid (event names aren't in the state hash → no golden
///                 drift). 314 unit (4 new deposit/withdraw/foil tests); 11 goldens
///                 byte-identical.
/// 0.0.170-alpha — RENAME weapon refine ore `ore_primocon` ("Primocon") → `ore_valkirion`
///                 ("Valkirion", Norse theme pairing Aegirilium). id + display + all
///                 drop-table refs in game_data; `REFINE_ORE_PRIMOCON` →
///                 `REFINE_ORE_VALKIRION`. migrate_item remaps the old id so any save
///                 stack still resolves. goldens alpha5/alpha6 re-blessed (the ore def_id
///                 is serialized in the scenario state). 314 unit.
/// 0.0.171-alpha — REFINE REWORK (money-zone). Odds curve now chosen by the item
///                 iLv, not weapon type (iLv 1-2 easiest LV1 … 7+ hardest LV4 =
///                 armor curve) — refine difficulty scales with gear power, unified
///                 for weapons + armor. FAILURE at +5 and up can now BREAK: −1 AND a
///                 new per-instance `Item.broken` flag (works + stays tradeable but
///                 never refinable; no repair yet) — the +5+ fail mass splits evenly
///                 downgrade/break. Ore cost = iLv per attempt (was flat 1); Gil
///                 unchanged. Refine BONUS = level × max(iLv flat tier [+3/+4/+5/+7],
///                 3% of the base stat). New `StartRefineInventory` + attempt_refine_
///                 inventory so the Refine screen can target un-equipped gear too.
///                 `broken` is skip-if-false (no golden drift from the field);
///                 migrate preserves it. golden alpha5 rescripted (swap a bricked
///                 sword) + re-blessed. 314 unit + 11 goldens.
/// 0.0.172-alpha — refine Gil fee + tunable-consts consolidation. Fee is now
///                 min(1000, 100×weapon_level) BASE × (current_level+1) — the cap is
///                 on the per-weapon base only (Lv2 base 200 → +2→+3 = 600, +8→+9 =
///                 1800). Gathered ALL refine balance knobs (curves, break level,
///                 gil, bonus %/tiers, ore) into one TUNABLE block atop refine.rs.
///                 golden alpha5 re-blessed (fee shift). 314 unit + 11 goldens.
/// 0.0.173-alpha — BIND is PARTY-WIDE. Consuming (binding) a card now grants its
///                 permanent bonus to EVERY party member (was the one member), with a
///                 shared escalating cost/cap (max over members). No schema change —
///                 uses the existing per-entity bound_card_bonus, applied to all;
///                 no golden touches BindCard. Album Bind is now a confirm POPUP
///                 (gain / cards used / party-wide / cannot-undo). 314 unit.
/// 0.0.174-alpha — RENAME the CARD system consume→bind (kept SEPARATE from the
///                 consumable-item ConsumeTrigger/ConsumeEffect, which stay
///                 "consume"). `ConsumeCard`→`BindCard`, `consumed_card_bonus/
///                 counts`→`bound_card_*`, ItemDef/Item `consume_effect`→
///                 `bind_effect` (+ game_data key). Serde ALIASES on every renamed
///                 serialized name (`ConsumeCard` / `consumed_card_*` /
///                 `consume_effect`) so old saves + replay logs still load. Field
///                 names shifted the serialized shape but all skip-if-default →
///                 goldens byte-identical. golden alpha6 re-blessed to catch up the
///                 earlier Mythical-card-rarity data change (not this rename).
///                 314 unit + 11 goldens.
/// 0.0.175-alpha — SERVER-ROLL refine foundation (sim side, docs §6.4). `StartRefine`
///                 / `StartRefineInventory` gain `server_roll: u64` — non-zero = a
///                 server-issued grind-proof roll nonce the outcome is derived from
///                 (zero `state.rng` draws), so a client can't shadow-sim the shared
///                 stream onto a success; zero = offline/alpha client-roll (one draw,
///                 unchanged). `authority()` → Server when server_roll != 0, else
///                 Client. `refine::attempt_refine{,_inventory}` take the nonce; the
///                 roll still reads `odds_for` (published-odds contract intact). Field
///                 is serde-skipped at 0 → existing logs + goldens byte-identical (NO
///                 re-bless). Server nonce issuance + verify-time recompute = next
///                 (verify-server). 316 unit (+2 refine) + 11 goldens.
/// 0.0.176-alpha — refine INSTANCE ID (server-roll Phase 2a, §6.4.1). `Item` gains
///                 `instance_id: Option<String>`, assigned lazily on an item's FIRST
///                 refine from `hash::instance_id(rng_fingerprint, tick, def_id)` (the
///                 non-advancing fingerprint → no PRNG draw) and stable across later
///                 refines. This is the stable identity a server binds a refine roll to
///                 (closes target-selection). Skip-if-none → only refined gear carries
///                 it; goldens byte-identical EXCEPT alpha5 (refines a sword → it now
///                 gets an id) — re-blessed. 317 unit (+1) + 11 goldens.
/// 0.0.177-alpha — refine ATTEMPT COUNTER (server-roll Phase 2b, §6.4.1). `Item` gains
///                 `refine_attempts: u32`, +1 per accepted refine — the MONOTONIC
///                 component of the server-roll nonce so every attempt is a fresh roll
///                 (no deterministic re-roll of a recurred level; keeps the gamble live).
///                 Skip-if-0 → only-refined gear carries it; goldens byte-identical
///                 EXCEPT alpha5 (refines) — re-blessed. 317 unit + 11 goldens. (Server
///                 `/refine/roll` binds this via the attempt count; verify-server 2b.)
/// 0.0.178-alpha — ENDURE REWORK (user design 2026-07-08): the flat "soft DEF per
///                 adjacent enemy" decayed to nothing vs banded ATK growth AND was
///                 itself taxed by the mobbed-DEF penalty. Now: (1) +1 VIT per level
///                 UNCONDITIONAL — folded in `buffed_stats` (soft DEF etc.) and
///                 `effective_max_hp`, useful un-mobbed; (2) resists 5%·level of the
///                 mobbed-DEF penalty (Lv10 halves it; at 22+ attackers keeps 50% DEF
///                 where others keep 0) via `mobbed_def_resisted` in `apply_damage`.
///                 SCHEMA: removed `Entity.endure_soft_def` + its `apply_endure`
///                 upkeep (old saves' stray field ignored by serde). ALSO: HP
///                 RECOVERY retune (user spec) — `level×(10 HP + 0.2% MaxHP)` per
///                 5-SECOND window; the old build paid those magnitudes per SECOND
///                 (~10x RO) and this game has no out-of-combat state to gate it,
///                 so a tank free-healed through band mob DPS. Goldens shift
///                 wherever a party fields Endure / HP Recovery — re-blessed.
/// 0.0.179-alpha — PER-CLASS WEAPON ASPD (iRO §4, docs/aspd-table-iro.md; user
///                 caught thief dagger == sword speed). `UnitDef.aspd_mods`
///                 (class × weapon-type table: Thief Dagger −8 / Sword −10,
///                 Assassin Dagger −2, …) stamped as `Entity.aspd_mods` at
///                 spawn + load (self-heal pattern). Item `aspd_base` is now a
///                 DELTA (uniques keep their +1/+2 edge; ordinary weapons drop the
///                 field). The old model put iRO PER-CLASS numbers on GLOBAL items
///                 — starting_dagger carried the thief's −8 next to sword_short's
///                 swordsman −7, so the thief's dagger was SLOWER than a sword.
///                 SCHEMA: `Entity.aspd_mods` (skip-if-empty). Goldens
///                 shift (every player's ASPD) — re-blessed.
/// 0.0.180-alpha — ARMOR refine odds = the HARDEST (weapon iLv 7+ / CURVE_LV4) curve at
///                 EVERY level, regardless of the armor's own iLv (armor is meant to be
///                 hard to refine). New `refine::refine_curve_ilv(item)` (weapon → own iLv;
///                 else → 7) drives the odds in `odds_for_item`, the verifier's
///                 `verify_refine_binding` risky-check, and the item view. Only the ODDS
///                 curve — bonus + ore cost still use the real item_level. Weapons
///                 unchanged → goldens byte-identical (alpha5 refines a weapon). No schema.
/// 0.0.181-alpha — ONLINE character-creation anchor fix. The party NAME is now a recorded
///                 `SetPartyName` event instead of being baked into the start snapshot, so a
///                 freshly-created online character's `start` is the server-reproducible
///                 `new_game_as(seed, class)` (name-free). The verifier's genesis anchor
///                 (`genesis_candidates`) now accepts the bare `new_game_in_zone` OR
///                 `new_game_as` for any CREATABLE class — fixing "start snapshot not
///                 anchored" that blocked EVERY online save. Additive event → goldens
///                 byte-identical.
/// 0.0.182-alpha — ASPD CURVE: linear tail above a 180 knee, cap 193 → 207 (user
///                 design: kill the hyperbolic blowup at the top and open design space
///                 for future "+ASPD" gear — on a line every +1 ASPD is worth the same
///                 +1/6 swing-milli, so the affix is printable arithmetic). Below the
///                 knee the iRO FJK curve is untouched; both pieces meet at exactly
///                 250 milli (180), and the 207 cap = exactly 7.00 swings/s. AGI/DEX
///                 clamp at 99 tops natural ASPD out ~179, so every existing build
///                 (all buffs stacked) is byte-identical — goldens expected unchanged.
///                 Bridge display inversion mirrors the same piecewise. No schema.
/// 0.0.183-alpha — ASPD BUFFS FOLD INTO THE NUMBER, RO-style (user: "quicken
///                 +ASPD แต่ไม่เกิน max / RO ปกติ quicken+potion ทับกันได้"). Sword
///                 Quicken (`AspdPct`) + speed potions (new kind `AspdPotionPct`)
///                 sum into the FJK `(1 − pot% − skill%)` gap-shrink BEFORE the 207
///                 cap — they stack with each other, are bounded by the cap, and
///                 ride the 0.0.182 linear tail (safe now that the hyperbola is
///                 gone; the old post-curve multiply existed to dodge it). Potions
///                 auto-drink on `BuffMissing(AspdPotionPct)` so a running Quicken
///                 no longer suppresses the sip (they're different kinds), while
///                 two potions still don't stack. game_data aspd_pot_* retagged.
///                 Goldens: shift only where a Quicken/potion was active — bless
///                 per drift.
/// 0.0.184-alpha — BESTIARY (user design): per-GROUP lifetime kill tally
///                 (`monster_kills`; same-model families share one pool via
///                 `MonsterDef.bestiary_group` — Pinkling line, Greenling line) +
///                 milestone rewards at 1k/10k/20k/50k kills (BESTIARY_MILESTONES:
///                 Gil/Gil/Soul/big pack), paid inside handle_monster_death with a
///                 claimed-bit map (`bestiary_claimed`) so replays pay exactly once.
///                 SCHEMA: 2 GameState maps (skip-if-empty) + MonsterTemplate.
///                 bestiary_group (skip-if-empty). Goldens shift on first kill —
///                 re-blessed.
/// 0.0.186-alpha — CRAFTING (potions). New catalog `GameData.recipes` (Vec<CraftRecipeDef>,
///                 serde-default so old catalogs parse) + `InputEvent::CraftItem{recipe_id}`
///                 (recorded, replay-safe; Client authority — SELF-COST: spends the player's
///                 own materials + Gil, bounded by supply; unknown recipe / insufficient
///                 inputs are silent no-ops). Deterministic (no PRNG draw). `crate::craft`.
///                 Seed: `red_herb` + `glass_flask` materials + recipe `brew_red_phial`
///                 (1 red_herb + 1 glass_flask + 5 Gil → hp_pot_1 ×10). Goldens unaffected
///                 (recipes not exercised; catalog field not in GameState hash).
/// 0.0.185-alpha — BESTIARY rewards are CLAIMED, not auto-paid (user). New
///                 InputEvent::ClaimBestiaryReward{group, idx} — recorded +
///                 replay-safe; Client authority is SAFE (it can only realise
///                 value the deterministic kill tally already earned; invalid /
///                 premature / repeat claims are silent no-ops). Kill accounting
///                 keeps only the tally. Goldens: auto-pay removal is inert below
///                 1k kills — expected byte-identical.
/// 0.0.187-alpha — ACOLYTE gains HOLY BANE (user: class had only 5 skills). Passive
///                 +3%/level (max +30%) damage vs SHADOW-element foes (dark / undead /
///                 demon) — `passive_holy_bane_bp` on the Damage packet, applied in
///                 `apply_damage` only when the DEFENDER is Shadow (0 bp = no change →
///                 goldens byte-identical). Tree node at job 9. (Divine Protection was
///                 prototyped alongside but CUT — it overlapped Guardian Aura's party DEF.)
/// 0.0.188-alpha — ACOLYTE gains ASPD DAMPENING (7th skill): a Holy 3×3 burst on
///                 the nearest enemy (light MATK) that SLOWS every foe in the blast's
///                 attack speed by (10 + 1.5·level)% for 5 s. New `BuffKind::AspdSlowPct`
///                 — a DIRECT attack-speed cut applied as a final multiplier in
///                 `effective_atk_speed_milli`, so it bites MONSTERS (flat ASPD, which
///                 the AGI-based slows never touched) and enemy players in PvP. The 3×3
///                 slow rides a shared helper called from BOTH the instant and ranged
///                 resolution paths (skill effect decoupled from the caster's anim/class
///                 — user foresight for future cross-class skill items). Gated on the
///                 skill id → 0 for everyone else, goldens byte-identical.
/// 0.0.189-alpha — AXIS SEPARATION stage 1 (user: "magic ไม่ควรคิดว่าเป็น range หรือ
///                 melee"). DAMAGE TYPE is now a SKILL property, decoupled from the
///                 caster: `SkillDef.damage_type: Option<DamageType>` (None = inherit
///                 the caster's atk_type). Threaded through the whole resolve path —
///                 `build_damage_as(kind)` / `build_damage_scaled(..,kind)` /
///                 `resolve_attack_scaled(..,kind)` — so the melee, ranged, and
///                 self-AoE paths all honour a skill's own type. Every skill defaults
///                 to None → kind = caster atk_type → goldens BYTE-IDENTICAL. Lets a
///                 future cross-class item grant e.g. Holy Light and have it stay
///                 Magic on a physical class. (Delivery axis instant/projectile +
///                 flipping specific skills to Magic = later stages.)
/// 0.0.190-alpha — AXIS SEPARATION stage 2: the DELIVERY axis. `SkillDef.delivery:
///                 Delivery { Inherit, Instant, Projectile }` — a skill can FORCE how
///                 it reaches the target regardless of the caster (a weapon strike stays
///                 instant on a projectile class; a bolt stays a flying projectile on an
///                 instant class). `effective_delivery_travel` folds it into the flight
///                 time before dispatch. RENAMED `resolve_melee_swing`→`resolve_instant`,
///                 `resolve_ranged_swing`→`resolve_projectile` (the names were the
///                 delivery axis, never a melee-weapon claim). All skills default Inherit
///                 → goldens BYTE-IDENTICAL. The 3 orthogonal axes for a damage skill are
///                 now explicit: damage_type (0.0.189), cast_element (existing), delivery.
/// 0.0.191-alpha — L1 MARKET bridge (Slice 2b): `GameState.market_ops: Vec<MarketOp>` +
///                 `next_market_op_id: u64` — an append-only log of the market operations that
///                 ACTUALLY APPLIED in the sim (Deposit/Buy/Claim). The verify-server reads this
///                 verified log as the trusted "what really happened", closing the apply-detection
///                 hole (it can't tell an applied event from a silently-no-op'd one by the final
///                 hash alone); each op's `id` is the server's dedup key (a client-issued nonce,
///                 refine-pattern). Handlers append ONLY on apply (a no-op records nothing). Both
///                 fields `skip_serializing_if` empty/0 → goldens BYTE-IDENTICAL (no scenario
///                 emits a market event). Append-only for now; prune-after-confirm is a later slice.
/// 0.0.192-alpha — HOLY LIGHT declared via the axes → fixes 2 live bugs. Now
///                 `damage_type: Magic` + `delivery: Projectile` (a flying magic bolt): it
///                 scales MATK (matches the "130% ATK" design, ending the acolyte-is-Physical
///                 discrepancy) AND resolves on the projectile path where its skill runes live —
///                 so BENEDICTION (heal) and RADIANCE (chain) finally fire for a real acolyte
///                 (reachable via rune_stone_acolyte). Tests dropped their proj_travel fakes.
///                 Aspd Dampening also → Magic. Goldens byte-identical.
/// 0.0.193-alpha — L1 MARKET Slice 4a: `DepositCommodity` + `MarketOp::Deposit` now carry
///                 `unit_price` — a deposit LISTS directly (the server creates the sell order on the
///                 verified save) instead of crediting a server holdings pool the units could strand
///                 in. Closes the deposit-strand + no-prod-listing-path gaps; listing rides the
///                 verified-save reconcile (no separate endpoint). market_ops skip-if-empty →
///                 goldens BYTE-IDENTICAL (no scenario emits a market event).
/// 0.0.194-alpha — SAVE-LOAD BUGFIX: dropped `#[serde(alias = "stash")]` from `inventory`. It
///                 collided with the real `stash` warehouse field (added 0.0.169) — serde saw
///                 both the "inventory" and "stash" keys and errored `duplicate field inventory`,
///                 so ANY GameState with a non-empty warehouse was UNLOADABLE (client `load_json`,
///                 the server's bundle deserialize, and its stored-save reads all failed). A latent
///                 brick since the stash shipped (0.0.169–0.0.193): deposit to the warehouse → save
///                 → can never load again. The alias only ever served pre-0.0.168 saves (bag under
///                 "stash"), long extinct. Serialize shape + state_hash UNCHANGED → goldens
///                 BYTE-IDENTICAL. New probe `tests/diag_inventory_replay.rs` pins the roundtrip
///                 (with-stash save must survive) AND that server replay (Some(data)) reproduces a
///                 live bridge session issuing Sell/Deposit/Withdraw via apply_input (None).
/// 0.0.195-alpha — ITEM MARKET Slice 1 (sim side, docs/item-trade-design.md): the per-instance
///                 EQUIPMENT trade. `MarketOp::DepositItem{item,price}` + `MarketOp::BuyItem
///                 {listing_id,max_price}` + `InputEvent::DepositItem{inv_idx,price}` / `BuyItemListing
///                 {listing_id,max_price}` → `mailbox_grant::deposit_item` (the item LEAVES the
///                 inventory, replay proves it authentic) / `buy_item_hold` (holds max_price Gil).
///                 Both Client-authority, self-cost, no-op on bad input. Cancel + settlement are
///                 server-side (reuse L1). market_ops skip-if-empty → goldens BYTE-IDENTICAL (no
///                 scenario emits an item op).
/// 0.0.196-alpha — SAVE-VERIFY REPLAY-HOLE FIX (found via a live account's intermittent
///                 hash-mismatch, tick 239k-383k). Transient COMBAT fields on `Entity`
///                 (`shields` + `pending_bursts` [absorb], `act_locked_until` [motion lock],
///                 `bloodthirst_stacks`, `down_ticks`, `kb_shot_hit`, `snare_used`,
///                 `field_hit_count`, `hiding_used`) and `Projectile.attacker_id` (Knock Back
///                 Shot / Steal / Arrow Crafter attribution) were `#[serde(skip)]`. The client
///                 kept them LIVE, but an autosave anchor captured mid-combat serialized WITHOUT
///                 them → the server's replay from that anchor diverged → INTERMITTENT
///                 hash-mismatch whenever one was non-default at the save tick (build/zone-
///                 specific: band-4 bomb+knockback combat sets them often). The "transient →
///                 drop on reload → golden-identical" assumption held for goldens (which never
///                 serialize mid-combat) but broke save-verify. Now serialized (skip-if-default →
///                 states without them stay byte-identical; AbsorbShield gained Serialize/
///                 Deserialize). ALL goldens BYTE-IDENTICAL; only the band-4 probe hash moved
///                 (re-pinned) since band-4 is the sole scenario that sets these at its hashed
///                 state — exactly the coverage gap that let this through. New guard
///                 `tests/anchor_roundtrip.rs` (a mid-combat serialize round-trip must reproduce
///                 the forward outcome; confirmed to FAIL without the fix).
/// 0.0.197-alpha — ITEM MARKET Slice 1 (sim delivery): `ClaimMail` + `MarketOp::Claim` gain an
///                 optional `item: Option<Item>` so an equipment PURCHASE/RETURN delivers the exact
///                 instance (stats/refine/stars), not a rebuilt def_id×qty stackable. New
///                 `data::try_grant_item` banks a full instance (all-or-nothing full-inventory guard,
///                 like `try_grant`). skip-if-none → commodity claims BYTE-IDENTICAL; goldens
///                 unmoved. Completes the sim half of the per-instance item trade (server ledger +
///                 routes next).
/// 0.0.198-alpha — ITEM MARKET audit fixes. LOW-2 (sim): `deposit_item` now REFUSES to list a
///                 stackable (equipment market is per-instance; stackables belong on the commodity
///                 order book) — no client emits a stackable DepositItem yet, so goldens
///                 BYTE-IDENTICAL. (Server-side, same batch: MED-1 cancel-path durability —
///                 persist-or-revert on the commodity/item cancel + sell handlers so a DB write
///                 failure can't leave in-memory diverged from Postgres → no false forged-claim
///                 flag after a restart; LOW-1 saturating fee multiply.) MED-2 (market_ops unbounded
///                 growth) DEFERRED — scaling-only, pre-existing, needs a server→client consumed-op
///                 ack to prune safely.
/// 0.0.199-alpha — EXEMPLAR power (skin + stat + skill + star ranks). New `items::Exemplar` carried
///                 per-character (`CharacterData.exemplar: Option`) and on the live `Entity`,
///                 transferred across deploy/bench/park. An Exemplar has a display `skin_id`, a base
///                 stat `effect`, a base skill set `skills: Vec<GrantedSkill{id,level}>`, and a `stars`
///                 (0..=5) rank with a per-rank `star_ranks: Vec<ExemplarStar>` unlock web — the
///                 gacha-dupe / constellation / eidolon model: each merged duplicate activates the next
///                 rank, which confers an ARBITRARY per-Exemplar mix of extra stats AND/OR any number
///                 of skills (a repeated id RE-LEVELS a lower rank's grant, so "unlock" and "level up"
///                 are one mechanic; active or passive, actives auto-slotted). Ranks `0..stars` apply
///                 cumulatively. `card_bonuses`
///                 folds `exemplar.total_effect()` (base + active ranks) into the bound-card stat pool.
///                 Granted skills are MATERIALIZED into the live loadout at deploy (`Entity::
///                 apply_exemplar_skill`, each `SkillState.from_exemplar = true`, actives FRONT-slotted
///                 so a full bar can never push the signature past the SP-reserve budget = inert)
///                 and STRIPPED at bench/park (`clear_exemplar_skill`) so they never persist as learned
///                 skills; a grant colliding with a learned class skill is left untouched. Debug
///                 `EquipExemplarTest{}` (authority Debug) equips a 3★ test Exemplar (placeholder skin,
///                 +stats, Bash Lv5→10, HP-Recovery passive at 2★) on the first member. Every new field
///                 serde-skips its default (`stars`/`from_exemplar`/`star_ranks`/… ), so a member
///                 WITHOUT an Exemplar stays BYTE-IDENTICAL — all goldens unmoved (336 unit + goldens
///                 pass). Client: `skin_id` read from the nested entity exemplar; sidebar Exemplar slot
///                 click sends the test event. Real store SKU ($8.99) + copy-merge = server entitlement
///                 layer, NOT sim (sim only carries `stars`).
/// 0.0.200-alpha — FIRST real Exemplar + SANCTUS TERRA (new skill) + ELEMENT-specific vulnerability
///                 (a first for the engine). `sanctus_terra` (skills/catalog/exemplar.rs): a 5×5 HOLY
///                 ground field, 15 s, pulses every 3 s (5 waves), 10% ATK/level (Lv5 50% … Lv10 100%),
///                 MAGIC damage (MDEF-mitigated) but scaling off ATK via new `SkillDef.field_power_from_atk`
///                 (default false → every existing field keeps MATK, goldens byte-identical); costs 2
///                 loadout slots; cooldown 30 s > 15 s duration so it never self-overlaps. Reuses the
///                 generic ground-field machinery + `on_hit_debuff` (guaranteed `chance_bp 10000`) to mark
///                 every enemy inside with the new `BuffKind::SanctusTerra` — the engine's FIRST
///                 element-specific vuln: `holy_vuln_bp` amplifies ONLY Holy-element damage (any source)
///                 by +20%…+30% (2%/level), applied in `apply_damage` after the generic `DmgTakenPct`
///                 vuln. Named after the skill; future holy-vuln skills get their own kind and sum into
///                 `holy_vuln_bp` (kept separate per skill). Debug `EquipExemplarTest{}` now equips the
///                 real SERAPHINE LUMINARY KNIGHT at 5★ (Sanctus Terra Lv10). New BuffKind variant +
///                 SkillDef field = serialized-shape change → VERSION bump; but no golden casts the skill
///                 or takes Holy damage while marked, so ALL goldens remain BYTE-IDENTICAL.
/// 0.0.201-alpha — EXEMPLAR AS AN ITEM (acquisition loop). New `ItemKind::Exemplar` (not
///                 stackable; Resonance stars carried on the item in `refine_level`) + shop item
///                 `exemplar_seraphine` (game_data 0.0.35, buy_price 50000 — a Gil stand-in until
///                 the real-money server entitlement). Code catalog `items::exemplar_def(item_def_id)`
///                 (same tune-in-code stance as skills) + `exemplar_item_def_id` for the carrier
///                 round-trip. New CLIENT events `EquipExemplar { entity_id, inv_idx }` /
///                 `UnequipExemplar { entity_id }` (handlers::equip_exemplar/unequip_exemplar):
///                 fresh equip at the item's carried stars; SAME id = MERGE (+1 + carried, cap 5;
///                 no-op at cap so a copy is never wasted); DIFFERENT id = swap (old carrier back
///                 to the inventory — always fits, the consumed copy freed its slot); unequip
///                 room-guarded (never destroys). The old `EquipExemplarTest` debug grant is a
///                 TOMBSTONE no-op (variant kept so recorded event logs still deserialize).
///                 Granted actives are NOT auto-slotted — the player enables them in the loadout
///                 (`ToggleActiveSkill`) like any learned active (a Resonance merge re-levels in
///                 place and PRESERVES the slotting). New `SkillDef.field_center_self` (default
///                 false): Sanctus Terra now consecrates the ground UNDER THE CASTER (self-centred
///                 AoE, range 2 trigger) instead of dropping on the target. Goldens: no golden
///                 touches an Exemplar item → ALL BYTE-IDENTICAL. Client: inventory click on an
///                 Exemplar-kind item equips it (viewed member); the sidebar Exemplar slot click
///                 UNEQUIPS; the skill page shows granted skills as "Exemplar Skill" cards
///                 (bridge `skill_info_json`) that slot via the normal loadout bar. Server
///                 redeploy required (sim + game_data version gate).
/// 0.0.202-alpha — DEBUFF STACK POLICY (explicit, per-kind). New `StackPolicy` on `BuffKind`
///                 (`stack_policy()`): every DEBUFF is now NON-STACKING — the target reads only
///                 the STRONGEST single mark of that kind across casters — EXCEPT DoTs
///                 (PoisonDot/Bleed/Burn/Poison), which keep per-source stacking (each caster's
///                 wound ticks its own). Entries are still STORED per (kind, source_id); the
///                 policy collapses the READ (new `skills::effective_bp`), so when the strongest
///                 mark expires the next takes over. Read sites swapped: DmgTakenPct, DefDownPct,
///                 AspdSlowPct, HitDownPct, AgiSlowAdd, AgiDexSlowPct (derived/damage/movement),
///                 SanctusTerra (`holy_vuln_bp`). Beneficial buffs keep `buff_sum` (unchanged).
///                 Sanctus Terra tuning (user-set): cooldown 30 s → 20 s. Behavior-only change
///                 (no schema); goldens re-checked — multi-source same-debuff scenarios collapse
///                 to the strongest mark (see re-bless note below if any moved).
/// 0.0.203-alpha — Item.resonance_stars: Exemplar Resonance stars move OFF `refine_level` onto a
///                 dedicated skip-if-0 field (refine +N / gear bonus-line `stars` / Resonance are
///                 THREE separate systems — user call). Equip reads `max(resonance_stars,
///                 refine_level)` so 0.0.201 carrier items keep their stars (legacy fallback);
///                 unequip writes only `resonance_stars`. REFACTOR: `CharacterData::capture` /
///                 `restore_onto` replace the 5 hand-copied per-character transfer sites (recruit /
///                 deploy / bench / park / class-restore) — capture strips Exemplar-granted skills
///                 itself, restore re-materializes them. Behavior-identical; goldens byte-identical.
/// 0.0.204-alpha — SANCTUS TERRA becomes a moving AURA + damage doubled. New
///                 `GroundField.follow_id` (skip-if-0) + `SkillDef.field_follows_caster`:
///                 `tick_ground_fields` re-centres a follow-field on its carrier's cell each tick
///                 while it lives (freezes in place if the carrier dies) — the knight casting then
///                 walking away no longer strands the field. Damage 4%→8% ATK per level per 1s
///                 pulse (Lv5 40% … Lv10 80%) — the static-field number felt light once mobile.
///                 Also: renderer plays the IDLE loop during a skill's motion-lock instead of a
///                 frozen mid-swing frame (act_locked_until read client-side). Goldens
///                 byte-identical (no golden casts a follow-field). ALSO in this version:
///                 UNIVERSAL ANIM SLOTS (`SkillDef.anim_slot`, e.g. "special_1" — sheets map the
///                 slot to their own row; bash + sanctus_terra declare it; field casts surface a
///                 display-only `skill_fx` so the animator plays the clip); FIAT LUX — Seraphine's
///                 2nd skill (passive): 20% on a landed basic MELEE hit → ONE magic HOLY packet of
///                 20% ATK × level at the struck target, drawn as a TWIN flash (2 visual hits); granted Lv5
///                 at 0★, re-leveled Lv10 at 2★ (roll gated on knowing the skill → goldens
///                 byte-identical); bridge `skill_info_json` ships the FULL detail-panel payload so
///                 Exemplar-skill cards inspect like tree nodes.
/// 0.0.205-alpha — SERVER-ROLL DROPS stage 1+2 (docs/server-roll-drops-design.md; user: "key
///                 feature"): tradeable CARDS move off the client-predictable weight pool onto the
///                 two-stage decoy pipeline. `DropEntry.server_bp` (skip-if-0; from the drop row's
///                 `server_bp`) marks a server-rolled entry: each kill takes ONE gated draw at that
///                 rate (chom cards = 50bp = 0.5%; draws only for monsters carrying such entries) and
///                 a hit accrues a PENDING `GameState.rare_candidates` entry (skip-if-empty; monotonic
///                 `rare_candidate_seq` = the nonce binding key; CAP 500; TTL 8h expiry in tick()).
///                 The server resolves candidates at save-verify with `keyed_hash(secret, account,
///                 seq)` at ~0.1% (final ≈ 1/200,000) and returns HITS as `GrantDrop { candidate_seq,
///                 server_roll }` (authority SERVER; duds are never sent — zero-byte convention).
///                 Apply = remove candidate + bank the card; FOIL derives from the nonce's high bits
///                 (server-authored, no client draw); seq never reuses → duplicated grants can't
///                 double-bank. Goldens: only chom-killing scenarios shift (card weight left the
///                 pool + the gated candidate draw) — re-blessed after verifying the drift is exactly
///                 those two causes.
/// 0.0.206-alpha — RECIPE LEARNING (docs/craft-and-offline-design.md "soul unlocks what you can
///                 make"): crafting now requires the recipe to be LEARNED first. New event
///                 `LearnRecipe { recipe_id }` (authority Client — self-cost) spends the recipe's
///                 one-time `learn_soul` + `learn_gil` (new CraftRecipeDef fields, default 0) and
///                 inserts into new `GameState.recipes_learned: BTreeSet<String>` (skip-if-empty;
///                 party-shared knowledge, survives a wipe like `soul`). `craft_item` no-ops on an
///                 unlearned recipe. game_data 0.0.40: Red Phial learn = 500 soul + 100 Gil; Speed
///                 Potion I learn = 10,000 soul + 1,000 Gil, output_qty 5→1 (user re-spec). Old
///                 saves: prior CraftItem ticks are baked into anchors, so nothing retro-breaks;
///                 going forward everyone (incl. old saves) must learn Red Phial. Goldens:
///                 byte-identical (no scenario crafts; skip-if-empty field) — no re-bless.
/// 0.0.207-alpha — SERVER-ROLL DROPS coverage: every sub-1% CARD + MATERIAL drop row moves onto
///                 the two-stage decoy pipeline (game_data 0.0.41, 380 rows, NO sim/server code
///                 change — grant_drop was already kind-generic, foil stays Card-gated). Legacy
///                 band-1..3 cards = 1/100k; rune stones 1/20k (low-tier row 1/50k);
///                 star stones red 1/50k / blue 1/20k / yellow+chaos+green 1/10k / shiny 1/5k /
///                 purple 1/4k / death 1/2.5k; ore_valkirion 1/4k (top mons 1/2k) — clean-rate
///                 re-round confirmed by user; candidate bp = 10^7/den against the global 0.1%
///                 server rate. featherfern stays client (user-specced exactly 1%); sub-1% GEAR
///                 stays client until stage-3 full-item rolls (substats would be steerable).
///                 These drops are now ONLINE-delivered (candidates expire offline after 8h).
///                 Goldens: most scenarios drift (extra gated candidate draws per kill on many
///                 monsters + rows leaving the weight pool) — re-blessed after confirming both
///                 causes.
/// 0.0.208-alpha — LOOT LEVEL-GAP PENALTY: over-levelled parties farm down-band monsters at
///                 reduced loot. `loot_gap_mult_bp` (drops.rs): gap = party MAX base level −
///                 `mob_level`; grace 15 levels (user re-spec from 10); gap 16 opens with a
///                 20% cut (user re-spec), then −5%/level, floor 10% at gap ≥ 30. Scales
///                 (1) the weighted item roll via `roll_drop_scaled`
///                 (draw range stretched ×10000/mult — ONE draw as before, bit-identical at
///                 full loot), (2) the Gil kill reward (xp.rs), (3) server-roll candidate
///                 rates (leaving them unscaled would hand over-levelled bots the full rare
///                 rate). New fields: `MonsterTemplate.level` + `Entity.mob_level` (both
///                 skip-if-0; from `MonsterDef.level`; 0 = exempt). game_data 0.0.42 fills
///                 the 49 level-less placeholder mobs by band formula (regular 5N−4, boss
///                 5N−3; zombie pinned 21 per spec — anchors fit the named mobs). EXP is NOT
///                 scaled (out of scope by user decision). Goldens: unaffected while the
///                 scenario party stays within grace of its zone's mobs — else re-bless.
/// 0.0.209-alpha — LUCIELLE RADIANT SAINT Exemplar (Acolyte, Holy). New BuffKinds MatkAdd /
///                 HolyDmgDealtPct (attacker-side, build_damage_as) / LumenVuln (2nd holy-vuln
///                 kind, summed in holy_vuln_bp). Reusable SkillDef basic_* override (Sagitta
///                 Lucis: basic → ranged(3) Holy magic @50% ATK, via skills::basic_override at
///                 effective_atk_range + build_damage). Benedictio Lucis (party +ATK/+MATK/+Holy%
///                 3s cast). Lumen Contagio: cast marks the nearest un-marked enemy (LumenVuln);
///                 a marked death schedules a MAGIC HOLY 5×5 blast (PendingExplosion.lumen_level,
///                 skip-if-0) that re-marks the unmarked (contagion) and cascades ACROSS ticks via
///                 handle_monster_death→maybe_schedule_lumen (EXPLODE_DELAY breaks same-tick
///                 recursion). exemplar_def("exemplar_lucielle"). All default-off / unreachable
///                 until equipped → goldens byte-identical; diag_acolyte unchanged.
///                 ALSO in 0.0.209: rare-bag claim now respects the inventory SLOT cap (user bug
///                 report — the old deliberate bypass let a full inventory overflow). Stackable
///                 merge into an existing stack still always fits; otherwise full inventory =
///                 no-op, the rare stays in the bag. Goldens byte-identical (no scenario claims
///                 at full inventory).
/// 0.0.210-alpha — SPIRIT element (gothic pack): 10th element, RO Ghost-styled — Holy → Spirit
///                 150% (ghost-busting) and Spirit → Neutral 150% (reverse = plain Neutral hits
///                 pass through spirits at 50%); everything else 100 (Shadow⇄Spirit neutral).
///                 Deliberately NOT in the gear-roll element pool (draw-order + player anti-ghost
///                 tech stays Holy). AtkElement codec extends (Spirit=9). game_data 0.0.45:
///                 Twinling + Banshee become Spirit (neither spawns yet). Goldens: byte-identical
///                 (no Spirit entity exists in any scenario) — no re-bless.
/// 0.0.211-alpha — CARD STARS (0-5★, user-designed): `UpgradeCard { album_idx }` (authority
///                 Client, self-cost) promotes ONE copy of an album stack a star, consuming
///                 `CARD_STAR_DUPES[star]` = 1/1/2/3/5 duplicates from the SAME stack (def+foil+
///                 star identity — a Foil never feeds a plain upgrade); all-or-nothing with an
///                 album-cap refund. `CardEffect::star_scaled` folds +20%/star (floored, integer)
///                 into `card_bonuses`; star 0 is bit-identical and no scenario holds a starred
///                 card → goldens byte-identical, no re-bless. Foil bonus stays a separate
///                 UNSCALED layer. Special effects per card land next (0.0.212).
/// 0.0.212-alpha — CARD SPECIAL procs/triggers (gothic set, user-designed): fixed-order
///                 basic-hit proc block (each roll GATED on holding the card — card-less streams
///                 byte-identical): Dustelle 5→10% self-Heal Lv1 (routes apply_skill_heal — CAN
///                 feed Ashelle, the maid combo); Banshee 3% casts SPIRIT STRIKE Lv5→10 (+50→100%);
///                 Crimsonne 5% drains 5→10% of the hit; Marionette 10→20% Bleed (100% ATK / 5 s,
///                 per-source refresh). Gnarloak reflects 5→10% of landed basic hits
///                 (deterministic, can kill). NEW: `apply_skill_heal` = the ONE skill-heal apply
///                 (Heal/Dupla/Subvenio/procs routed) firing Ashelle's on-healed +2→4% ASPD stack
///                 (BuffKind::CardAspdStack, cap 3 × 5 s, folds with AspdPct); SPIRIT STRIKE skill
///                 def (mage catalog, ⌈lv/2⌉ Spirit bolts × 100% ATK, REAL multi-hit, one shared
///                 resolve for proc + future tree). Goldens: byte-identical (no card exists yet).
/// 0.0.213-alpha — card procs + Gnarloak reflect fire on RANGED BASIC hits too (user: "ต้องแก้"):
///                 new transient `Projectile.is_basic` (serde-skip like attacker_id; set from
///                 cast_opt at launch — skill shots stay excluded) gates the same fixed-order
///                 proc block at projectile arrival. Card-gated → goldens byte-identical.
/// 0.0.214-alpha — card star upgrade goes GACHA-STYLE (user re-spec): flat 1 duplicate = +1
///                 star (was the 1/1/2/3/5 curve); `UpgradeCard` gains `copies` (serde default
///                 1 — old logs parse) and feeds N copies into ONE survivor in a single action,
///                 clamped to spares + the 5★ cap. Album-cap refund unchanged. Goldens
///                 byte-identical (no scenario upgrades).
/// 0.0.215-alpha — card upgrades feed from the BASE 0★ stack (fix: a starred card could
///                 never find same-star duplicates, so >1★ upgrades were impossible). Feeder =
///                 the def's 0★ stack (same foil); self-feed keeps one survivor; cap 5★
///                 confirmed. Goldens byte-identical (no scenario upgrades).
/// 0.0.216-alpha — card feed PICKER re-spec (user): new `FeedCard { target_idx, feeder_idx }`
///                 (authority Client) feeds ONE chosen copy — same def, ANY star/foil. Star
///                 gain = 1 + feeder's star (copies conserved), cap 5★; a FOIL feeder converts
///                 the target to foil (legal on a 5★ plain target as pure conversion; user:
///                 "ไม่ได้ดาวเพิ่ม แค่เปลี่ยนสถานะเป็น foil"). `UpgradeCard` kept for replay
///                 compat. Goldens byte-identical (no scenario feeds).
/// 0.0.217-alpha — card-feed audit follow-ups: (1) `feed_card` star math is now SATURATING
///                 (was raw u8 +/- → a version-skewed/tampered star > cap would underflow-PANIC
///                 with overflow-checks on, breaking the never-panic replay invariant); guard
///                 test feed_card_survives_out_of_range_stars. (2) card album is now UNBOUNDED
///                 (user: "album ไม่จำกัดเสมอ player ไม่ต้องขยาย") — `card_album_cap()` returns
///                 usize::MAX; `card_album_cap` field + `CARD_ALBUM_BASE_CAP` vestigial; bridge
///                 stops emitting card_album_cap. No scenario fills the album → goldens
///                 byte-identical. Also drops the now-unreachable album-full refund from
///                 feed_card / upgrade_card (bank_into can't fail on an unbounded album).
/// 0.0.218-alpha — FOIL now STAR-SCALES (user reversal of the flat-foil rule: "อยากให้ scale
///                 20% ตามดาวด้วย"): `card_bonuses` runs `foil_effect.star_scaled(star)` like the
///                 Epic effect, so 0★ = authored value and each star lifts base + foil together.
///                 Card panel display scales the foil line to match. 0★ unchanged → goldens
///                 byte-identical (no golden equips a starred foil).
/// 0.0.219-alpha — foil review (data): Chomb + Gloomtooth crit normalized to 2% (200bp) on BOTH
///                 base and foil (user "ปรับเป็น 2% ทั้ง basic/foil ทั้ง chom กับ gloomtooth") —
///                 Chomb foil 300→200, Gloomtooth base 300→200 + foil 300→200. Also Scarlette foil
///                 crit 100→200 (+2%, peer parity). Stored per-instance so existing copies
///                 grandfather (future drops only); VERSION bump keeps the online version-gate from
///                 letting a new-data client sync a stale-data server. No golden drops these gothic
///                 cards → byte-identical.
/// 0.0.220-alpha — EXEMPLARS move out of the inventory into a dedicated, UNCAPPED
///                 `owned_exemplars` collection (user: "Exemplar อยู่ที่หน้านี้ ไม่ใส่
///                 inventory แล้ว...เป็นไอเทมสำคัญจะได้ไม่หาย"). Purchases bank here;
///                 EquipExemplar/UnequipExemplar + the Resonance feed move carriers to/from
///                 the equipped member; the Exemplar screen lists the collection (equipped ones
///                 labelled, not removed) and equip picks from the available list. `EquipExemplar`
///                 field `inv_idx`→`owned_idx` (serde alias keeps old logs parseable, now
///                 indexing the owned collection). No migration — at alpha a tester just re-buys;
///                 any Exemplar a pre-0.0.220 save left in the bag is simply inert. New field is
///                 skip-if-empty → old saves + goldens byte-identical.
/// 0.0.221-alpha — Exemplars go PERMANENT-in-collection (like cards): `Item.equipped_by`
///                 marks which member wears a carrier; `EquipExemplar` now MARKS (no consume,
///                 no merge) + syncs the combat overlay, `UnequipExemplar` just unmarks. New
///                 `FeedExemplar { target_idx, feeder_idx }` (authority Client) is Resonance —
///                 mirrors `FeedCard` minus foil: feeds an UNEQUIPPED duplicate into any target
///                 (equipped or not), rank += 1 + feeder stars (saturating, cap 5), re-syncing
///                 the wearer in place so the loadout slot survives. Seraphine class-gate fixed
///                 (swordsman family, was unrestricted). New field skip-if-0 → goldens
///                 byte-identical.
/// 0.0.222-alpha — LOAD-TIME DATA LOSS fixed: `migrate_item` rebuilt each item from its def
///                 and copied earned state back from a hand-maintained list, so every
///                 per-instance field missing from that list was wiped on EVERY load. Lost:
///                 `resonance_stars` + `equipped_by` (0.0.221 Exemplar marks — loading
///                 unequipped your Exemplar, reset its Resonance to 0, and left Unequip a
///                 no-op since nothing carried the mark: user "กด u จะไม่เจอ exemplar ที่
///                 equipped อยู่ + กดถอดไม่ได้"), `refine_attempts` (the monotonic component
///                 of the server-roll refine NONCE — a reload rewound it, so attempts
///                 replayed spent nonces) and `instance_id` (market identity stamped at
///                 first refine). Now ASSIGNS def fields onto the live item instead, so
///                 adding a per-instance field can never again require editing migrate_item.
///                 Guarded by `migrate_save_items_preserves_every_per_instance_field`.
///                 No serialized-shape change; all 11 goldens byte-identical (none carry
///                 these fields). VERSION bumped because the server REPLAYS saves through
///                 the same migrate — a client that keeps the marks and a server that drops
///                 them would diverge on hash.
/// 0.0.223-alpha — LEGACY CARD SPECIALS (user-designed, 2026-07-21): the 7 stat-only
///                 starter cards gain mechanics like the gothic set. Pinkling +10→20% dmg
///                 vs Fire defenders (dmg_vs_element channel); Burst Chom +10→20% on
///                 Fire-element attacks (stacks with Broomhilda); Chomb +10→20% crit
///                 damage (crit_dmg channel; Weapon slot — competes with Scarlette);
///                 Greenling −2.5→−5% damage taken from BASIC hits (new defender-side
///                 fold in resolve_attack_scaled gated power_bp==10_000, mirrored at the
///                 off-hand + basic-projectile sites); Yellowling: defender ignites a
///                 basic-hit attacker 5→10% with a tier-1 Combustion burn (try_thorn_burn,
///                 rolled after Gnarloak reflect at both sites); Toxic Chom: attacker
///                 basic hits 5→10% classic Poison 10 s (after Marionette in
///                 try_card_procs; bosses immune, roll still taken); Rechum +25→50% on
///                 the HEAL skill's output only (id-gated in try_cast_support). All new
///                 PRNG draws are state-keyed on the card being socketed → all goldens
///                 byte-identical (no re-bless). VERSION bumped: saves carrying these
///                 cards replay differently across the boundary (version-gate handles it).
/// 0.0.224-alpha — RENEWAL-CRIT rework + CRIT DAMAGE becomes a real stat (user
///                 2026-07-21: "1.5x + ปรับ cri เป็นแบบ post renewal ที่เพิ่มเดเมจเฉยๆ
///                 ไม่ ignore def"). (1) A physical crit no longer ignores DEF — it takes
///                 the normal two-layer mitigation like any hit. (2) New
///                 `effective_crit_dmg_bp` = 15_000 base (×1.5) + card crit-dmg channel
///                 (Gloomtooth/Scarlette/Chomb — MOVED out of the additive card bucket
///                 into this dedicated multiply layer) + new gear enchant line. Applied
///                 at all three crit sites: resolve_attack_scaled, offhand (which never
///                 had a crit-dmg fold), the ranged/projectile packet build. (3) New
///                 `StatMod::CritDmgBp` (enum tail, append-only) rollable on weapon +
///                 accessory, tiers 5-8%/10-15%/15-20% via the def-tier window, uncapped;
///                 Family::CritDmgBp appended (family-draw set grows → enchant rolls on
///                 future starred weapon/accessory drops shift). Goldens: byte-identical,
///                 no re-bless — golden parties never allocate LUK (crit chance
///                 (LUK−1)×30bp = 0 → zero crit draws) and no golden run drops a starred
///                 weapon/accessory. Real saves DO shift → version-gate handles skew.
/// 0.0.225-alpha — RANGED dmg-vs GAP closed (user: "projectile fold dmg-vs ด้วยเลย ทำงาน
///                 แบบ melee เลย"): the projectile packet build now folds gear dmg-vs
///                 (element × race × size, via dmg_vs_bp) and the card dmg-vs bucket
///                 (Twinling/Pinkling + Broomhilda/Burst Chom) exactly like
///                 resolve_attack_scaled — Hydra-lineage gear and those cards now work
///                 on bows/casts. Folds sit AFTER the shot's element overrides (bolt
///                 element / ammo autoswap / Soul Crush) so the atk-element channel keys
///                 on the FINAL element; resolved vs the primary target (splash inherits,
///                 like cleave). No new draws; mod-less/card-less streams byte-identical
///                 → goldens unchanged. VERSION for the replay boundary on modded saves.
/// 0.0.226-alpha — BAND SPLIT ×3 (user: band-clear ping ต้องถี่ขึ้น — "band 1-1/1-2/1-3
///                 ใช้มอนเดิม เพิ่มเลือดเพิ่มเดเมจ"): the 20-band ladder becomes 60
///                 sub-bands (data regen; each legacy band → 3 sub-bands × the same 3
///                 rooms, thresholds kept — user chose the ×3-kill longer walk). New
///                 ZoneDef `hp_scale_bp`/`atk_scale_bp` (default 10000 = byte-identical)
///                 fold into MonsterTemplate at build: HP/ATK/atk_min/atk_max scale,
///                 XP+JobXP ride the HP bp, Gil deliberately flat, def/hit/flee/level
///                 untouched (level-gap loot penalty keys on monster LEVEL — unchanged
///                 semantics). Sub-band scales interpolate geometrically to the next
///                 legacy band (measured pool-weighted ratios; b1-b4 cliffs ×2-3 get
///                 real intermediate steps, b7+ ≈ +4-6%/step). Soul now scales by
///                 REGION ((band+2)/3) so the tripled band numbers don't triple the
///                 faucet. Goldens: data change shifts every zone-advancing scenario →
///                 re-blessed (intentional content restructure).
/// - 0.0.227-alpha: MEASUREMENT COUNTERS + BOUND ITEMS (battle-pass P0 substrate,
///                 docs/battle-pass-implementation-plan.md). `GameState.measures`
///                 (kills / rooms_cleared / refine_attempts / orbs_used / cards_fed /
///                 exemplars_fed / items_crafted / items_listed): account-lifetime
///                 monotonic u64s bumped ONLY on each action's success path — the sim
///                 attaches no meaning; the ONLINE layer (quest/BP) diffs them across
///                 verified saves. `Item.bound` (use-only, server-stamped on granted
///                 rewards): refuses vendor sale (sell_value preview = 0), commodity
///                 listing (unbound-only count/consume), and per-instance listing;
///                 bound stacks NEVER merge with unbound (all four merge sites).
///                 Preserved by migrate_item (per-instance set + test). Goldens:
///                 measures.kills/rooms count in every combat scenario → all combat
///                 goldens re-blessed (intentional new-field drift; zero logic change).
/// - 0.0.228-alpha: APEX (post-cap progression; user: capped players ต้องมีอะไรทำ +
///                 "ตัวเลขไม่หยุด") — player-facing name APEX (code = apex). At LEVEL_CAP
///                 `try_level_up` no longer parks XP at 0 — the same bar drains into apex
///                 levels via `APEX_XP_TABLE` (a fixed 50-entry per-point table like the
///                 level `XP_TABLE`, seeded from a linear 4M+0.85M×n ramp; SCALING so early
///                 points are quick and later a chase), each = 1 Apex Point. Capped at
///                 `APEX_LEVEL_CAP`=50 (51+ future), calibrated so a 24/7 idler reaches ~50
///                 around the 90-day patch window; at the apex cap XP parks at 0. Kept
///                 through a level-cap raise (leveling resumes; apex retained).
///                 Points spend (InputEvent `AllocateApex`, authority Client; `ResetApex`
///                 refunds) into 9 low-impact FLAT buckets on `Stats` (`apex: [u32; 9]`,
///                 append-only), EACH capped at `APEX_BUCKET_CAP` = 50 pts: Might+1ATK /
///                 Focus+1MATK / Vigor+20HP / Guard+1DEF / Precision+2HIT / Evasion+2FLEE
///                 / Edge+1%CritDMG / Deadeye+0.05%CritChance / Avarice+0.25%Soul-gain.
///                 Folds join the existing flat channels in `derived` (+ soul reward in
///                 xp.rs uses party-max Avarice). Per-character, PvE-only (gate out of PvP
///                 later like soul paths). New `Stats` fields skip-if-zero → every golden
///                 (party never reaches LEVEL_CAP) byte-identical; no re-bless. VERSION for
///                 the save/replay boundary once a real save carries apex.
/// - 0.0.229-alpha: DISCARD ITEM (user: อาจจะให้ขายได้ราคา 0 ถ้า player รู้สึกว่ามันรก) —
///                 new InputEvent `DiscardItem { inv_idx }` (authority Client): removes the
///                 whole stack for ZERO Gil. It is the ONLY liquidation path a `bound` item
///                 accepts — a pure destroy mints no value and reaches no market, so it can't
///                 launder (BP6 intact). Lets a player clear unwanted bound pass clutter. UI
///                 surfaces it only on bound items, behind a confirm. GameState shape
///                 unchanged (new event variant only) → every golden byte-identical, no
///                 re-bless; VERSION bumped so the client/server sync gate rejects a peer that
///                 doesn't know the new variant (replay would diverge).
/// - 0.0.230-alpha: BOUND-FIRST CRAFT/REFINE + BOUND OUTPUT (user: การตีบวก กับคราฟ priority
///                 bound item ไหม → กิน bound ก่อน + ผลผูกมัด). Craft ingredient and refine ore
///                 consumption now spends BOUND stacks before unbound (clears a player's pass
///                 clutter before their sellable stock), via `consume_n_stackable_bound_first`
///                 (returns bound units taken). When any bound input fed the operation the
///                 OUTPUT binds — a crafted item via `grant_loot_bound`, a refined piece by
///                 setting `bound` on the gear. This closes the audit H1/H2 laundering channel
///                 (a free bound material's value could otherwise flow into a tradeable
///                 craft/refined item). Retired the plain `consume_n_stackable` (footgun: not
///                 bound-aware). No golden moves — goldens hold no bound stacks, so the
///                 bound-pass consumes nothing and the unbound pass matches the old order
///                 exactly; bound_taken=0 ⇒ no output binding. VERSION bumped because the same
///                 -dirty git sha can't distinguish this behavior from 0.0.229 at the sync gate.
/// - 0.0.231-alpha: CAST RETARGET-ON-TARGET-LOSS (user: อยากปรับการ cast โดยรวม → ถ้า target
///                 หายไปให้ retarget ก่อนจะ whip). A channel whose locked enemy dies/despawns
///                 mid-windup no longer DROPS the cast (wasting the whole windup) — it
///                 re-resolves a target for the SAME skill via its own targeting rule
///                 (`resolve_skill_target`, extracted from `pick_active_skill` so both share
///                 one source; skill-selection path stays byte-identical) and KEEPS windup
///                 progress (swap target_id, continue the countdown, fire this cadence). Only
///                 abandons when NO valid target remains. Buffs unaffected (their target is the
///                 living caster). Deterministic (reuses the same targeting helpers, no PRNG).
///                 Combat behavior changes only when a cast's target dies mid-windup; no golden
///                 auto-play hits that timing edge → every golden byte-identical, no re-bless
///                 (new unit tests cast_retargets_when_its_target_dies_keeping_windup_progress
///                 + cast_abandons_when_no_enemy_remains cover it). VERSION bumped for the
///                 save/replay boundary (a peer without this logic diverges on that edge).
/// - 0.0.232-alpha: REFINE BREAK SHARE 1/3 + CAP 20% (user: เป็นโอกาสแตก 1/3 แทน; broken chance
///                 max 20%). On a failed refine +5↑ the BREAK (damaged) share of the failure mass
///                 drops from 1/2 → 1/3, then capped at `MAX_BREAK_BP` = 2000 bp (20%); the rest is
///                 a plain downgrade. Gentler than the old even split (RO's fail is deterministic
///                 by ore, never a split — this is our house rule) but still a real risk the Seal
///                 items (Aegis/Unbroken) protect against. Also UI (refine_view) now shows ONLY the
///                 success % — the downgrade/break split is hidden (sim still rolls all three).
///                 Refine-outcome mapping changed → any golden that fails a +5↑ refine re-blessed
///                 (see bless list); intentional.
/// - 0.0.233-alpha: REFINE ENRICHED ORE + PROTECTION SEALS (refine-protection feature, sim core).
///                 `StartRefine`/`StartRefineInventory` gain `enriched: bool` + `seal: RefineSeal`
///                 (both serde default+skip → old logs/goldens replay byte-identical). `enriched`
///                 uses the Enriched ore (`enriched_valkirion`/`_aegirilium`) on `odds_for_enriched`
///                 = success min(100, s+max(s/2,10)) with the SAME 1/3-break-cap-20% fail split
///                 (still breaks). `RefineSeal::{Aegis,Unbroken}` is consumed per attempt if held
///                 (bound-first; a requested-but-unheld seal is ignored, unprotected): Aegis maps any
///                 fail → new `RefineOutcome::NoChange` (keep level); Unbroken maps Break → Downgrade
///                 (−1, no `broken`). `attempt_refine[_inventory]` stay as thin no-seal/unenriched
///                 wrappers over `_with`, so every existing caller + golden is byte-identical (no
///                 re-bless). New item defs enriched_valkirion/_aegirilium/aegis_seal/unbroken_seal;
///                 UI_REFINE_BROKEN loc "BROKEN"→"Damaged". VERSION bumped: a peer without the new
///                 event fields would silently drop enriched/seal on replay and diverge. UI slot +
///                 Reforge event still TODO.
/// - 0.0.234-alpha: MONSTER ACTIVE-CAST SKILLS + BLEED PHYSICAL MITIGATION (user-designed
///                 Duskmourn/fauna bestiary). `build_and_spawn_monster` copies a monster's
///                 `Active` skills into `active_skills`, so the shared (not team-gated)
///                 `pick_active_skill` / `try_cast_*` pipeline casts them — monsters now cast
///                 DURING combat, not only on death. New `SkillDef.spawn_use_chance_bp` gates
///                 whether a spawned mob carries an active (per-monster variety); the first cast
///                 is desynced from the entity id (a hash — NO PRNG draw). Four skills, all reusing
///                 existing FX: `poison_attack` (150% Neutral bite + Poison, chance_bp 0 = no
///                 draw), `umbral_roots` (3×3 Shadow ground_field: ticks + slow), `blood_slash`
///                 (80% Shadow hit + Bleed), `bloodfrenzy` (Gloomtooth PASSIVE — each landed hit
///                 ramps its own ASPD +5%, cap +100%, via a new gated on-hit proc `try_bloodfrenzy`).
///                 BLEED now mitigates like a physical hit — hard DEF (armor %) then soft DEF
///                 (VIT/AGI/Level, flat) — applied ONCE to the lifetime TOTAL (per-tick would over-
///                 subtract the flat soft DEF); affects thief Open Wound too. Goldens byte-identical
///                 (these mobs aren't in golden scenarios; golden bleed hits 0-DEF dummies) → no
///                 re-bless; 5 new integration tests in tests/monster_active_cast.rs. VERSION
///                 bumped: a peer without monster active-cast diverges on replay once a mob casts.
/// - 0.0.235-alpha: CRAFT/REFINE OUTPUT STAYS TRADEABLE (user 2026-07-24 — reverses the audit
///                 H1/H2 output-binding). Bound-first consumption is KEPT (clears clutter,
///                 deterministic) but a bound material/ore no longer binds the result: refine
///                 dropped `if bound_ore>0 { item.bound=true }` in both `attempt_refine_with` +
///                 `attempt_refine_inventory_with`; craft grants `grant_loot` (was `grant_loot_bound`).
///                 Rationale: the player can't pick which stack (bound vs unbound) feeds the op —
///                 bound-first is forced — so surprise-binding their tradeable gear/craft is worse
///                 than the negligible in-game-market laundering (gate by price if abused). Two
///                 tests re-asserted (…_leaves_gear_tradeable / …_output_is_tradeable). Goldens
///                 byte-identical (no golden feeds a bound input). VERSION bumped: a peer on the old
///                 logic binds an item the new logic leaves tradeable → replay hash diverges.
/// - 0.0.236-alpha: REFINE DISCOUNT COUPONS (user 2026-07-24 — early-game Gil relief, granted via
///                 the battle pass: Coupon I ×5 early / Coupon II ×5 mid). A new `RefineCoupon`
///                 (None/Minor/Major) on `StartRefine`/`StartRefineInventory` (serde default-skip,
///                 so existing saves replay unchanged). If held, it is spent this attempt and knocks
///                 a FIXED Gil amount off the cost — Minor 500, Major 1000 — free when cost ≤ value
///                 (`charged = gil_cost.saturating_sub(value)`). EVERYTHING else is a normal refine:
///                 same ore, odds, break risk; the coupon is a bound pass reward. Consumed bound-first
///                 like ore/seal. Two new item defs `refine_ticket_minor/major`. 3 new tests; goldens
///                 byte-identical (no golden uses a coupon). VERSION bumped: a peer without the field
///                 diverges once a coupon discounts an attempt.
/// - 0.0.237-alpha: INVENTORY EXPANDER (user 2026-07-25 — make the pass's "+1 bag slot" reward real).
///                 New `ExpandInventory { inv_idx }` (Client) uses one `inventory_expander` item for a
///                 PERMANENT +1 bag slot via a new `GameState.inventory_cap_bonus` field (serde
///                 skip-if-0; capped at `MAX_INVENTORY_CAP_BONUS = 50`), which `inventory_cap()` now
///                 adds on top of the base + soul-path StashCap. No-op (nothing consumed) on a wrong
///                 item or at the cap. One bound item def `inventory_expander`, granted on the pass
///                 free track. 1 new test; goldens byte-identical (no golden expands). VERSION bumped:
///                 a peer without the handler skips the event → different bag cap → drop-banking diverges.
/// - 0.0.238-alpha: MAGE cast animation (new 60px mage sheet — art integration). The mage's bolt
///                 nukes (fireball/fire_bolt/spirit_strike/lightning_bolt/frost_bolt) gained
///                 `anim_slot: "cast_finish"` + `motion_delay_ticks: 5` so they play the sheet's
///                 casting→cast_finish clips like the acolyte's casts: the 0.5 s act-lock is what
///                 keeps the cast_finish one-shot from being cancelled by the resuming auto-attack
///                 (atk_acc). `fire_pending_cast` now also pushes a display-only `skill_fx` for the
///                 resolved bolt (the animator's only cast signal for a swingless offensive cast).
///                 anim_slot/skill_fx are renderer-only (no hash impact); motion_delay IS a real
///                 act-lock → band-4 diag hash re-pinned (a mage bolt-casts there). VERSION bumped:
///                 a peer without the act-lock diverges on cast timing. ALSO (same mage pass): the
///                 four bolts' `cooldown_ticks` 16 → 50 (~5 s after FIRE, user pacing — the mage
///                 auto-attacks between casts so bolts read as special, not spammed). Balance drift →
///                 band-4 diag re-pinned again; no golden_* moved.
/// - 0.0.239-alpha: REGALIA overlay P0 (signature-weapon overlay — the Exemplar twin, gacha
///                 deferred). New `Regalia` struct (mirrors `Exemplar`: `CardEffect` stat grant +
///                 `GrantedSkill`s + `ExemplarStar` Resonance web, with `signature_anim` in place of
///                 `skin_id`) + `owned_regalia: Vec<Item>` + `Entity.regalia`/`CharacterData.regalia`
///                 + `SkillState.from_regalia` + `ItemKind::Regalia`. Events `EquipRegalia` /
///                 `UnequipRegalia` / `FeedRegalia` (Resonance) mirror the Exemplar equip/feed
///                 handlers; the stat package folds into `card_bonuses` beside the Exemplar; a member
///                 may wear BOTH. P0 stats are placeholder from the current `CardEffect` vocab
///                 (atk_pct/crit_bp/flat) — designed headline stats (CritDmg/MatkPct/element/heal)
///                 need a `CardEffect` extension later; the layered passives are a later phase. All
///                 new fields serde-skip default → goldens byte-identical (nothing owns a Regalia).
///                 VERSION bumped: the serialized GameState shape gained fields/an ItemKind/events.
///                 PLUS the Regalia-ONLY ENHANCEMENT LEVEL axis (Exemplar has none, user 2026-07-28):
///                 `Regalia.level` + `per_level` (stat × level) + `Item.enhance_level` (per-instance,
///                 preserved by migrate_item's assign-only shape) + `EnhanceRegalia { target, feeder }`
///                 consuming an R-rarity `regalia_fodder` carrier (+1 level, cap `REGALIA_MAX_LEVEL`
///                 = 5). Two independent axes: Level (fodder → main/flat) + Resonance (dupes → ranks).
/// - 0.0.240-alpha: RANDOM boss from the pool (gothic bands 5-10). New `ZoneDef.boss_from_pool`
///                 (GameData) + `ZoneSpawnState.boss_from_pool` (GameState, serde skip-if-false so
///                 every fixed-boss zone — all goldens — serializes byte-identically). When set,
///                 `try_spawn_boss` weighted-picks a pool monster (any but mini_ghost/bat) via ONE
///                 PRNG draw and makes it WEAR the fixed boss's stats + rewards (look/skills/element
///                 from the mob, difficulty from boss_bN). VERSION bumped: GameState shape gained a
///                 field + boss spawn draws one more time in bands 5-10 (no golden reaches them).
/// - 0.0.241-alpha: DUNGEON — standalone instance off the main band chain (Duskmourn Manor, user
///                 2026-07-28). New `EnterInstance { start_zone_id }` event (Client, twin of
///                 SelectBand but addressed by arbitrary zone id) warps to a `zone_dungeon_*` room 1,
///                 reachable only from the new Dungeon menu. GameData-only content: 4 rooms
///                 (`zone_dungeon_duskmourn_1` gothic-roster wave + pool doorkeeper (boss_b9 stats,
///                 user 2026-07-29 — an ordinary-monster room BEFORE the mini-boss) / `_2` mini-boss
///                 Brutus + 8 gothic minions / `_3` mob wave + pool boss / `_4` Dulahaness end-boss,
///                 is_instance_end), plus one absolute-stat mini-boss def `brutus_lord` (werewolf
///                 skin, boss_minions 8). Clearing `_4` banks the escrow and parks the party in the
///                 cleared room (no `next_zone_id` — the player leaves via the menu). No GameState shape
///                 change; goldens byte-identical (no golden reaches the dungeon). VERSION bumped: a
///                 peer without the EnterInstance handler can't deserialize a log carrying it (online
///                 version-gate), so the client dylib + verify-server must match.
/// - 0.0.242-alpha: OVERLAY STAT UNIFY — Exemplar/Regalia stop granting `CardEffect` and grant
///                 `Vec<StatMod>` through the SAME modifier pipeline as gear (`combat::stat_mods`,
///                 the module the 0.0.239 rename anticipated). Two halves: (1) VOCABULARY — the
///                 overlays now speak the full 24-family gear vocab (AtkBp/CritBp/CritDmgBp/
///                 dmg-vs/…) instead of the narrow flat-stat `CardEffect`, so the designed
///                 headline stats no longer need a `CardEffect` extension. `card_bonuses` keeps
///                 ONLY the card paths (bound + slotted); `stat_mods::for_each_mod` walks
///                 gear → exemplar → regalia (all consumers SUM, so order is inert).
///                 (2) DEF/STORE SPLIT — the authored content moved to NON-serialized
///                 `ExemplarDef` / `RegaliaDef` (base mods + skills + star web); what an entity
///                 STORES is a lean identity (`Exemplar` = id/name/skin/stars, `Regalia` = +
///                 signature_anim/level) and the grant is recomputed on demand via `mods()` /
///                 `granted_skills()`. So no derived stat list rides in the save or the hash
///                 (the serde-skip replay hole of 0.0.196 can't recur here), and re-tuning an
///                 Exemplar patches every existing save. `heal_exemplar_grants` no longer
///                 refreshes a stored grant — it only re-applies the granted SKILLS idempotently.
///                 Old saves deserialize fine (the dropped fields are simply ignored) but their
///                 shape changed → VERSION bumped. Goldens byte-identical (no golden equips an
///                 overlay). ALSO: `GameData::validate` now allows `spawn_pool` +
///                 `target_population == 0` when the zone's boss has `boss_minions > 0` (the
///                 escort draws from the zone pool) — required by dungeon room 1, which is a
///                 mini-boss + 8 minions and no ambient wave.
/// - 0.0.243-alpha: CARD STAT UNIFY + AUTHORED STAR LADDER — the second half of the 0.0.242
///                 unify: SLOTTED cards stop granting `CardEffect` and grant `Vec<StatMod>`
///                 through `stat_mods::for_each_mod` beside gear/overlays. `card_bonuses` is
///                 now ONLY `bound_card_bonus` (binding leaves no id to recompute from, so
///                 that blob stays — see S4 below). Three parts:
///                 (1) `StatMod::DefFlat` appended (enum tail) — the ONE vocabulary gap
///                 `CardEffect` had. NOT in `gear_roll::FAMILIES`, so the enchant draw set is
///                 unchanged and `family_of` returns `Option` (an authored-only line sits
///                 outside the no-dup ladder and a reroll passes it through with ZERO draws).
///                 (2) `Item`/`ItemDef` swap `card_effect`/`foil_effect` for `card_mods` +
///                 `card_star_step` + `card_star_ranks` + `foil_mods` + `foil_star_step`;
///                 `Item::active_card_mods()` resolves base ++ step×star ++ the ranks at or
///                 below the star (++ the foil half when foil).
///                 (3) STAR SCALING IS NOW AUTHORED CONSTANTS, not `×(1 + 0.2·star)` floored.
///                 The old multiply killed 56 star-steps in real data — a +1 STR card was
///                 bit-identical from 0★ to 4★ — and it grew DRAWBACK lines (burst chom's
///                 −3 DEF became −6). Two ladder shapes, both authored (user 2026-07-29):
///                 `card_star_step` climbs EVERY star (the headline ATK/HP/FLEE number) while
///                 `card_star_ranks` bumps only at the ranks it names (the small flat stats
///                 step at 1★/3★/5★, e.g. Pinkling STR +1/+2/+3 cumulative). This matches
///                 `card_specials::stepped(base, step, star)`, which was already constant —
///                 the two card paths now agree. All 19 cards re-authored in game_data.
///                 The bridge ships RESOLVED lines (`card_lines` / `card_foil_lines`) so
///                 `item_slot.gd` / `card_panel_view.gd` stop re-deriving the star formula in
///                 GDScript (the sim owns it; the UI only prints).
///                 VERSION bumped: `Item`'s serialized shape changed (card defs are refreshed
///                 from the catalog by `migrate_item`, so old saves heal on load). GOLDENS:
///                 `golden_alpha6` + `diag_band4` RE-BLESSED — not because either sockets a card
///                 (neither does) but because `DropEntry.template` is a FULL `Item` and those
///                 templates live in `ZoneSpawnState`, so every card row in a monster's drop
///                 table hashes a card prototype directly. Bisected to be sure: stripping
///                 `card_effect` from the defs with ZERO code change moves both hashes the same
///                 way. All other goldens byte-identical.
///                 ALSO S3 (same slice): the card SPECIALS whose effect a `StatMod` family
///                 already expresses moved OUT of `combat::card_specials` into authored data —
///                 DMG-vs-element (Twinling/Pinkling → `DmgVsElement`) and CRIT DAMAGE
///                 (Gloomtooth/Scarlette/Chomb → `CritDmgBp`), with their `display_values` +
///                 `CARD_SPECIAL_*` loc rows dropped so the tooltip cannot print the same line
///                 twice. Crit-dmg was already additive → identical numbers; dmg-vs-element now
///                 joins the element AXIS beside gear (additive within an axis, axes multiply)
///                 instead of being its own multiply layer — same result alone, slightly
///                 different only when a card and a gear line target the SAME element.
///                 What STAYS bespoke, because no family expresses it: a bonus keyed on the
///                 ATTACK's own element (Broomhilda/Burst Chom), damage reduction on BASIC hits
///                 only (Greenling), a bonus on ONE skill id (Rechum — `HealBp` is all heal
///                 output, so folding it would be a balance change), and a flat add derived from
///                 the holder's STR (Brutus).
///                 STILL ON `CardEffect` (S4, later): `bound_card_bonus` + `bind_effect`. NOTE
///                 the def/store split that made the overlays lean is NOT reachable for bound
///                 cards — binding leaves no id on the entity, so the honest S4 is a VOCABULARY
///                 swap (`bound_card_mods: Vec<StatMod>`) plus a load-path conversion of the old
///                 blob, not a recompute-from-def.
/// - 0.0.244-alpha: PLAYER SKILLS + the Duskmourn end-boss AoE (user 2026-07-29). Two halves:
///                 (1) `ZoneDef.player_skills` / `ZoneSpawnState.player_skills` — a list of
///                 abilities belonging to the PLAYER rather than a character, granted by CONTENT
///                 (a boss room), so the idle overworld stays hands-off. A list, not a flag per
///                 skill, because more are planned (incl. a future Regent skill tree, whose
///                 OWNED skills become a second source inside `player_skill_available`).
///                 First entry `regent_hand`: `PickUpMember` lifts a member OFF THE BOARD (no
///                 action, untargetable, no AoE reaches them — `is_held` guards targeting, the
///                 ground-field victim scans and projectile landing) and `PlaceMember` drops
///                 them within `PICKUP_RANGE` 6. The sim owns every rule: range, a free landing
///                 cell, a 3 s hold cap that drops in place, a 0.5 s landing lock, a 10 s
///                 per-character cooldown — so a forged input can only do what a real drag could,
///                 and the cost of dodging is that character's damage uptime.
///                 (2) `deathknell` — Dulahaness's 3 s TELEGRAPHED Shadow barrage (5 bells over
///                 a 7×7, each a 5×5 blast) built on the existing meteor ground-field engine; the
///                 wind-up IS the pick-up window. Meteor count/hits now honour an explicit
///                 `field_hits`/`field_strike_hits` (a monster is always skill level 1, so the
///                 Meteor Rain level curve couldn't shape a boss barrage); Meteor Rain leaves
///                 both 0 ⇒ its curve, and every golden, is unchanged.
///                 Also 4 ROOMS in Duskmourn (user 2026-07-29): a gothic-roster wave room with a
///                 pool doorkeeper now precedes the Brutus arena — see the 0.0.241 entry.
///                 VERSION bumped: GameState gained `Entity.held_until` / `pickup_ready_tick`
///                 (+ zone `player_skills`), all serde-skipped at their defaults, so the 11
///                 goldens are byte-identical; a peer without the two new event variants can't
///                 deserialize a log carrying them (online version-gate).
/// - 0.0.245-alpha: `CardEffect` IS GONE — the stat-vocabulary unify finishes. BOUND cards were
///                 the last holdout; they now grant `Vec<StatMod>` like gear, the Exemplar /
///                 Regalia overlays and slotted cards, so `combat::stat_mods::for_each_mod` is
///                 the ONE place a modifier can enter the sim and the `CardEffect` struct, its
///                 `add`/`is_zero`/`star_scaled` impl, `combat::card_bonuses`, and the parallel
///                 `skills::buffs::card_stat_bonuses` sum are all deleted.
///                 `Entity.bound_card_bonus: CardEffect` → `bound_card_mods: Vec<StatMod>`, and
///                 `ItemDef`/`Item.bind_effect` → `bind_mods`. The stored list is DERIVED, never
///                 authored: `bind_card` appends one copy of the def's `bind_mods` per bind, and
///                 `migrate_save_items` REBUILDS it from `bound_card_counts` × the def's current
///                 `bind_mods` on every load. So (a) re-tuning a bind in the catalog patches
///                 every existing save, (b) a pre-0.0.245 save — whose `bound_card_bonus` blob no
///                 longer deserializes — is restored in full from the counts it still carries,
///                 and (c) the verify server runs that same pass before replay, so both sides
///                 rebuild identically. Idempotent, `BTreeMap` key order → deterministic.
///                 NUMBERS ARE UNCHANGED by construction: every authored `bind_effect` in
///                 game_data used FLAT fields only (atk/def/flee/hp/luk/matk/str_), and each maps
///                 onto the flat `StatMod` family that already folded at the same stage — there
///                 was no `atk_pct`/`hp_pct` bind to move across the %-layer boundary.
///                 Bridge/UI: `bind_info_json` and the item view ship RESOLVED strings
///                 (`bind_lines`), and the last GDScript stat formatters (`CARD_STATS`,
///                 `card_stat_line`, `ItemDesc.card_effect_str`) are deleted — no client-side
///                 copy of a sim formula is left on the card path.
///                 GOLDENS: `golden_alpha6` + `diag_band4` re-blessed AGAIN, same mechanism as
///                 0.0.243 — a monster's `DropEntry.template` is a full `Item` living in
///                 `ZoneSpawnState`, so renaming the card prototypes' bind field reshapes what
///                 those templates serialize. All other goldens byte-identical.
/// - 0.0.246-alpha: NEGATIVE StatMod lines are REAL DRAWBACKS (audit fix, user 2026-07-30).
///                 The "can zero a mechanic" families clamped `0..=CAP`, so an authored
///                 negative — heavy armor that slows you, a cursed card that deepens an
///                 elemental weakness — was silently discarded; and every consumption site
///                 additionally guarded on `> 0`, so even an unclamped negative would have
///                 done nothing. Now: symmetric `-CAP..=CAP` on all seven capped families
///                 (resist-element / DR / ignore-DEF / ignore-MDEF / cooldown / cast /
///                 move-speed) and the consumers test `!= 0`. Semantics: negative resist =
///                 weakness (×>1 damage taken), negative DR = more damage, negative CDR/cast
///                 = LONGER, negative ignore-DEF = the target's armor counts for more.
///                 ONE LATENT BUG fixed while doing it: `effective_cast_time` did
///                 `stat_mods::cast_bp(e) as u64`, which would have turned a −1000 into
///                 ~1.8e19 and destroyed the subtraction — that path is signed i64 now.
///                 ALSO `stat_adds` was the only aggregator using plain `+=` while the rest
///                 use `saturating_add`; with this project's `overflow-checks = true` in BOTH
///                 dev and release, an extreme sum would PANIC in a shipped build instead of
///                 saturating. Now consistent.
///                 No shape change, no authored negative exists yet ⇒ all goldens
///                 byte-identical. VERSION bumped because BEHAVIOUR changed: a peer on
///                 0.0.245 would mitigate a negative line differently and diverge on replay.
/// - 0.0.247-alpha: AURA housekeeping (user 2026-07-30). (1) The two always-on aura channels
///                 disagreed about DEAD members: `apply_party_auras` stamped corpses,
///                 `apply_soul_path_auras` skipped them — so a downed member kept its party
///                 aura and silently lost its path aura. Both stamp every player now (the
///                 clear+re-stamp is still idempotent, so nothing can double up).
///                 (2) NEW `Entity.aura_matk` + `SkillDef.party_matk_per_level` +
///                 `skills::party_matk_bonus` — the caster-side twin of the flat-ATK aura,
///                 folded into `effective_atk`'s flat pool exactly where `aura_atk` sits in
///                 `effective_atk`. The channel is wired END TO END but NO skill grants it
///                 yet, so it is 0 for every build today; a test pins BOTH halves (it folds
///                 when set, it is inert without a skill) so the plumbing can't rot before
///                 its skill lands. `aura_matk` is skip-if-0 ⇒ saves and goldens
///                 byte-identical.
///                 (3) `refine::refine_matk_bonus` + `SkillDef.matk_flat_per_level` +
///                 `skills::passive_matk_bonus` — refine and passive/mastery now reach MATK,
///                 the last two channels ATK had and MATK did not. Refine MATK is GATED on
///                 `matk_bonus > 0` so a refined SWORD cannot sprout magic attack off the flat
///                 iLv tier; the passive reuses the SAME weapon gate as its ATK twin, so a
///                 future Staff Mastery is one arm in `passive_atk_applies`. Only the refine
///                 half is live today — both `*_per_level` knobs are 0 on every skill and are
///                 pinned inert by tests so the plumbing cannot rot before its skills land.
///                 VERSION bumped: GameState's shape gained `aura_matk`, the corpse rule
///                 changes replay for a party owning soul paths, and REFINED MAGIC WEAPONS NOW
///                 HIT HARDER (a real balance change — expect golden drift wherever a
///                 refined staff/mace is in play).
/// - 0.0.248-alpha: COLLECTIBLE ROUTING — Exemplar/Regalia carriers never enter the bag, on
///                 EVERY acquisition path. Groundwork for gacha (docs/gacha-economy-design.md):
///                 a pull is delivered as MAIL, and `ClaimMail` → `try_grant_item` banked
///                 everything into `state.inventory`, so a paid pull would have landed in the
///                 bag instead of `owned_exemplars` — the collection the equip UI reads and the
///                 one `state.rs` already declares must "never [be] in inventory/stash/market" —
///                 and worse, would have FAILED the room check when the bag was full (money in,
///                 nothing out).
///                 The rule itself is not new: the SHOP path has enforced it since Exemplars
///                 shipped. It is now one helper, `data::build::items::bank_collectible`, called
///                 by both paths, so the two cannot drift (the same "one resolver, N call sites"
///                 lesson as the 0.0.243 card-tooltip bugs). Regalia gains it for the first time
///                 — `ItemKind::Regalia` previously appeared in NO acquisition path at all
///                 (equip/feed/enhance only, plus the debug grant), because gacha is its
///                 intended first source.
///                 Collections are UNCAPPED, so a collectible grant can no longer fail; each
///                 copy stays its own carrier (dupes are what Resonance consumes, and a 2nd copy
///                 equips on another member). No GameState shape change; VERSION bumped because
///                 `ClaimMail`/purchase REPLAY behaviour changes for these two kinds. Goldens
///                 byte-identical — no existing log or golden mails or buys a collectible.
/// - 0.0.249-alpha: DUNGEON KIT — a boss that can be answered, and a dungeon that can be
///                 scaled (user 2026-07-30, driven by play tests). Engine additions, all
///                 generic rather than boss-specific:
///                 · FUSED ground field (`GroundFieldKind::Damage.fuse` + `SkillDef.field_fuse`)
///                   — inert while it lives, ONE detonation on its last tick. What makes a
///                   marked cell a WARNING the party can answer instead of damage already
///                   taken; the client draws any fused field as a floor telegraph, so a future
///                   fused skill needs no client work.
///                 · `cast_target_random` — a field skill brands a RANDOM foe (one PRNG draw,
///                   taken only when the cast fires). A nearest-target field can only ever
///                   threaten the frontline, never the healer behind it.
///                 · `cast_hp_above_pct` — the mirror of `cast_hp_below_pct`, so a calm and a
///                   wounded version of a skill HAND OVER instead of both being live.
///                 · `max_charges` — a per-fight use budget seeded into `SkillState` at
///                   creation (a monster is rebuilt each spawn, so the budget is per fight).
///                 · `xp_mult_bp` / `job_xp_mult_bp` / `level_add` — a `scale_from_base` monster
///                   scaled its COMBAT stats only, so a scaled boss paid a trash mob's XP and
///                   stood at the trash mob's level.
///                 BUGS FIXED: `cast_hp_below_pct` was implemented inline on the buff-cast path
///                 ONLY — ground-field, summon and trap skills ignored it entirely (a 50%-HP
///                 skill fired from full health, reading in game as "the AoE spawns too often
///                 and sometimes doubles up"). Now one `hp_gate_open` every path consults.
///                 CONTENT (game_data): Dulahaness gains Pyre Mark (a 3 s fused 3×3 Fire mark on
///                 a random member, 8 s cadence, 4 s once wounded) and a ONE-TIME wounded summon;
///                 the boss rooms' handmaids are their own defs carrying a 4 s taunt, so the
///                 party must fight INTO the ring instead of killing the boss through it. Both
///                 named bosses converted to `scale_from_base`, so a Lv60/70/80/90 copy of the
///                 dungeon is pure data — pinned by a test asserting this tier's 12 stats are
///                 unchanged (the atk min-max spread now follows the band base's shape; the mean
///                 is identical).
///                 REGENT HAND: available on every map now (an OWNED player skill, the second
///                 source `player_skill_available` was built for) and its cooldown moved from
///                 per-character to a PLAYER-held charge pool (2 charges, one back per 10 s) —
///                 stored as charges SPENT so every default is zero and old saves load full.
///                 DUNGEON CLEAR: `return_zone_id` (recorded on entry, so `LeaveInstance` carries
///                 no destination and a forged one can only send you back where you came from),
///                 `RunTally` + `ClearSummary` (captured BEFORE the escrow banks, or the loot is
///                 indistinguishable from everything else), and a client clear screen.
///                 VERSION bumped: GameState gained the fuse flag, the charge pool, the return
///                 zone, the run tally and the clear summary, plus two event variants.
///                 GOLDENS: `golden_alpha6`, `golden_beta5`, `diag_band4` re-blessed — `run_tally`
///                 starts counting at every instance room 1, so it is no longer a default value
///                 and enters the hash. Deliberately NOT serde-skipped: it feeds `ClearSummary`,
///                 which IS state, so skipping it would diverge on replay (the 0.0.196 hole).
/// - 0.0.250-alpha: DUNGEON CLEAR FIX — `LeaveInstance` now dismisses `last_clear`
///                 UNCONDITIONALLY; the zone change stays best-effort.
///                 The dismissal used to be tied to a successful `change_zone`, so a leave with
///                 no `return_zone_id` on record was a silent no-op. That is not merely a failed
///                 action: the client hides the clear screen and re-reads state next frame, sees
///                 the summary still there and re-shows it — an UNCLOSABLE clear screen with the
///                 party stuck in the dungeon. Found by play-test, not by a test.
///                 No return point can be recorded when entering deeper into a chain you are
///                 already inside (the only candidate is another dungeon room), so the sim
///                 genuinely cannot know the destination; the world map is the way out. What it
///                 must never do is refuse to close the screen.
///                 dungeon_clear_view.gd also stops clearing its own `_shown_for` guard on leave.
///                 Regression test: `leaving_still_works_when_no_return_point_was_recorded`.
///                 VERSION bumped: `apply_input` behaviour changed, so a replay of the same log
///                 diverges. GameState SHAPE is unchanged.
///                 SAME VERSION, second play-test fix: the clear screen also fired on ORDINARY
///                 band bosses. `is_instance_end` is set on every `zone_bN_3` zone, where it
///                 has always meant "bank the escrow, count the band clear"; the dungeon kit
///                 reused the flag for the screen. A band's third zone has a `next_zone_id` and
///                 auto-advances — the run does not end, so no screen. `ClearSummary` is now
///                 written only when NOTHING will move the party on (no next zone, not farm-
///                 looping), which is exactly a dungeon's final room: the party is left standing
///                 there and the screen is how they leave.
///                 Folded into 0.0.250 rather than bumped again — it was never deployed.
///                 GOLDEN: `diag_band4` re-blessed. It runs through `zone_b4_3`, which used to
///                 write a ClearSummary into the state it hashes and now correctly does not.
///                 Regression test: `an_ordinary_band_boss_does_not_raise_the_dungeon_clear_screen`.
/// - 0.0.251-alpha: DUNGEON RUN END — one screen, two verdicts, and a real loot count.
///                 A party wipe INSIDE A DUNGEON now ends the run the way a clear does: it raises
///                 `ClearSummary` with the new `failed` flag, and the client shows the same panel
///                 saying PARTY WIPED instead of DUNGEON CLEAR. Ordinary band wipes are untouched
///                 — that is the respawn-and-retry grind, not a run, and a modal on every death
///                 would be miserable. "Inside a dungeon" is derived, not a new data field: a
///                 dungeon chain's start zone is not a band zone (`band_of_zone` -> None), so
///                 ZoneDef is unchanged and no zone data moved.
///                 `ClearSummary.items` was `Vec<Item>` cloned from the LOOT ESCROW — vestigial
///                 since direct-grant in 0.0.47, so it has reported 0 items for ~200 versions. It
///                 is now a u32 counted at the drop via the new `RunTally.items`.
///                 `LeaveInstance` falls back to `zone_b10_1` when no return point was recorded,
///                 so a missing one can never strand the party in a room with no exit.
///                 `RestartInstance` (the Retry button) no longer bumps `consecutive_wipes` — a
///                 deliberate re-run is not a defeat, and counting it skewed sim-cli's
///                 auto-retreat model — and it dismisses the verdict.
///                 VERSION bumped: GameState SHAPE changed (RunTally gained `items`;
///                 ClearSummary's `items` changed type and it gained `failed`).
///                 GOLDENS: alpha4, alpha6, beta4, beta5, diag_band4 re-blessed. `RunTally.items`
///                 counts every drop, so like `run_tally` itself in 0.0.249 it stops being a
///                 default value and enters the hash on any scenario that loots.
///                 ⚠️ SAVE-COMPAT: the three new fields SHIPPED WITHOUT `#[serde(default)]` for
///                 about an hour and a real save failed to load because of it — a player's game
///                 was replaced by a fresh one, recovered only from the `.UNREADABLE-*` rescue
///                 copy. `#[derive(Default)]` on the struct does NOT make serde tolerate a
///                 missing key; the attribute is needed per field, which is why ~90 fields in
///                 state.rs carry it. Fixed, and pinned by
///                 `a_save_written_before_the_newest_fields_still_loads`, which strips the new
///                 fields from a real serialized save and requires it to load.
/// - 0.0.252-alpha: END-OF-RUN SCREEN LISTS THE ITEMS, not a count. `RunTally`/`ClearSummary`
///                 carry `loot: Vec<Item>` (capped at `RUN_LOOT_SHOWN` = 24, since it rides in
///                 the save and the state hash) collected at the drop. A bare number reads as
///                 "nothing happened" even when it did.
///                 ⚠️ RENAMED `items` -> `loot` ON PURPOSE. `items` shipped for ~30 minutes in
///                 0.0.251 as a `u32`; a save written by that build carries `items: 5`, and a
///                 number cannot deserialize into a Vec. Reusing the name would have broken
///                 those saves the same way the missing `serde(default)` did an hour earlier.
///                 Nothing in the crate uses `deny_unknown_fields`, so the stale numeric key is
///                 ignored and `loot` starts empty. Pinned in
///                 `a_save_written_before_the_newest_fields_still_loads`, which now also feeds a
///                 save carrying the short-lived numeric `items`.
///                 DEV: `debug_fake_clear(failed)` raises a fabricated summary (console `clearui`
///                 / `failui`) — the gothic mobs have no client-side loot tables (TODO.md), so a
///                 real run always shows an empty list, indistinguishable from a broken UI.
///                 GOLDENS: all looting scenarios re-blessed — `loot` holds Items now, which
///                 serialize nothing like the count they replaced.
/// 0.0.253-alpha — MARKET-OP PRUNE. `market_ops` was append-only and never trimmed, so a client
///                 re-presented its whole trading history on every save — the Vec rides the state
///                 hash, every save blob and every replay, and it only grew. Live data: one
///                 account at 150 ops in a 352 KB save.
///                 New `InputEvent::PruneMarketOps { ids }`, `InputAuthority::Server`: the save
///                 response reports which ops the server actually settled and the client applies
///                 the prune as a RECORDED event after `commit_anchor`, exactly like `GrantDrop`.
///                 Recorded, not silent, because `market_ops` is hashed — a local trim would
///                 desync from the anchor the server trusts.
///                 Server authority is load-bearing: a save can be ACCEPTED with nothing settled
///                 (tainted session, poisoned market), so "my push succeeded" does NOT mean "my
///                 ops were taken", and pruning on that assumption would delete a listing the
///                 server never made.
///                 Prerequisite done first (0.0.252 server-side): `trade_ledger` now holds the
///                 real record of what was traded, so this Vec is free to be the work queue it
///                 always was rather than the only trace that a trade happened.
///                 GOLDENS: none move — no scenario records a market op, and GameState's shape is
///                 unchanged (a new InputEvent variant is log-only).
/// 0.0.254-alpha — SERVER-ROLLED CRAFT. The 12 Duskmourn weapon recipes shipped outputting a flat
///                 0★ Common — `craft_item` → `grant_loot` → `instantiate_item` copies the def and
///                 rolls nothing — so a crafted weapon was strictly worse than any dropped one and
///                 the recipes were a floor nobody would use. Crafted gear now rolls 1★ or better
///                 (user 2026-08-02, "ได้ 1* ขึ้นไป ไล่อัตราส่วน แต่ต้อง roll ผ่าน server").
///                 `STAR_WEIGHTS_CRAFT` = the drop table with 0★ removed and the rest rescaled by
///                 their own total: 72 / 24 / 3.96 / 0.04. That puts Legendary at 4× the trash-drop
///                 rate, which is exactly why the roll cannot be the client's.
///                 `RareCandidate.kind` (`CandidateKind::{Drop, Craft}`) — the ONE shape change.
///                 Skip-if-`Drop` on serialization, so every existing save and golden is
///                 byte-identical. A GEAR recipe now accrues a `Craft` candidate instead of
///                 granting; potions, ores and materials still land instantly (nothing to roll).
///                 New `InputEvent::GrantCraft { candidate_seq, server_roll }`,
///                 `InputAuthority::Server`, mirroring `GrantDrop`: star + `roll_bonuses` both come
///                 out of `Pcg32::new(server_roll)`, so the whole outcome is fixed by the server's
///                 nonce, replays identically everywhere, and takes NO draw from the state PRNG.
///                 A SEPARATE event and a separate nonce domain (`craft-roll:v1`), not a flag on
///                 `GrantDrop`, because the two have opposite rules — a craft has no rate to pass.
///                 Sharing one event would let a client replay the craft nonce it was legitimately
///                 issued for seq N against a forged history where seq N is a card candidate,
///                 buying a 1-in-200,000 card with a 30-material craft. Both handlers therefore
///                 also guard on `kind`, so a cross-kind grant is a no-op.
///                 Two ways to lose a paid-for item, both closed: `craft_item` refuses a craft that
///                 would exceed `RARE_CANDIDATE_CAP` rather than eating the inputs (`drop_on_death`
///                 may skip a candidate at the cap — free for a dud lottery ticket, ruinous here),
///                 and `Craft` candidates are EXEMPT from the 8 h TTL that prunes duds, so crafting
///                 and then staying offline overnight no longer deletes the weapon.
///                 Banks like a SHOP purchase, not like loot: the rare bag protects random rare
///                 drops from the common-gear flood, and a piece bought with 30 materials is not
///                 something the player needs protecting from. No skill-rune roll either — a rune
///                 is a drop chase, and a repeatable recipe would turn it into a farm.
///                 `pending_crafts` in `view_state` + a craft-screen line: the item arrives a save
///                 cycle later, and an unexplained empty bag reads as a failed craft.
///                 GOLDENS: none move — no scenario crafts, and a `Drop` candidate serializes
///                 exactly as before the field existed.
/// 0.0.255-alpha — CRAFT AND DROPS GET SEPARATE CANDIDATE BUDGETS. 0.0.254's craft guard counted
///                 against `RARE_CANDIDATE_CAP`, the DROP lottery's 500. That was wrong in a way
///                 that only shows on a real account: drop candidates are duds by design and sit
///                 there until the 8 h TTL, so an established farmer is near the cap most of the
///                 time — and because `craft_item` REFUSES rather than eating the inputs, the
///                 symptom was a Craft button that did nothing whatsoever, silently, on exactly
///                 the accounts the recipes exist for. Found by play test, not by a test: every
///                 unit test started from an empty candidate list.
///                 `CRAFT_CANDIDATE_CAP = 64` is now its own budget (`candidate_counts` splits
///                 them), and `drop_on_death` likewise counts only DROP candidates against its
///                 own cap instead of the whole list. State stays bounded at 564.
///                 `pending_drops` joins `pending_crafts` in `view_state`, and `craft_item` logs
///                 the before/after counts client-side: a refused craft and a working one look
///                 identical on screen, and that must never again be invisible.
///                 Bumped despite no shape change: the accept/refuse boundary moved, so a 0.0.254
///                 client against a 0.0.255 server would disagree on replay. The version gate is
///                 what stops that.
///                 GOLDENS: none move — no scenario crafts, and the new count is identical to
///                 `len()` on a list that holds only Drop candidates.
/// 0.0.256-alpha — THE TWO-STAGE DROP PIPELINE, PUT BACK THE RIGHT WAY ROUND.
///                 `docs/server-roll-drops-design.md` §Design specifies stage 1 as "a low-rate
///                 `rare_candidate` flag PER KILL (e.g. 1%)" — one global rate, with per-item
///                 rarity living in stage 2 on the server, and `candidate_rate` named as the
///                 knob that sets server-roll LOAD.
///                 0.0.207 inverted that. It needed clean final rates and got them by putting
///                 the per-item rate in STAGE 1 as `bp = 10^7/den` — a form that grows as the
///                 item gets MORE common (1/2,500 needs 40%) — rolled per drop-table ENTRY, so
///                 a 12-entry monster accrued 1.3 candidates per kill. Measured across the
///                 catalog: 0.62/kill against the 0.01/kill the design budgets. 62x.
///                 Nothing caught it because the FINAL RATES were all correct (the commit even
///                 states the formula, and the user confirmed the rates it produced) and no
///                 test has ever measured candidate accrual. The 500 cap, sized when only
///                 cards were on the pipeline, was overrun after ~800 kills — and past the cap
///                 `drop_on_death` just stopped accruing, so EVERY server-roll item, cards
///                 included, silently stopped dropping ~15 minutes into any session.
///                 Now: ONE gated draw per kill at `CANDIDATE_RATE_BP` (100 = 1%, the doc's
///                 default), then a second draw picking WHICH item, weighted by the entry's
///                 authored `server_bp`. Those numbers keep their meaning as relative
///                 frequency within a monster — no rebalance, nothing in game_data.json moves.
///                 Stage 2 moves onto the ticket: `RareCandidate.hit_ppm` (parts per million)
///                 replaces the server's flat `drop_hit_den`. It has to live there because the
///                 server resolves a candidate knowing only the candidate — not which monster
///                 died — and the rate depends on that monster's mix. `stage2_hit_ppm(W) =
///                 10*W` is derived to reproduce every authored final rate EXACTLY (old
///                 `bp/10^7` = new `1% x bp/W x 10W/10^6`), as an exact integer, no rounding.
///                 Old saves default `hit_ppm` to 1,000 (the previous flat 0.1%), so tickets
///                 already pending resolve on the odds they were accrued under.
///                 Both stage-1 draws are now taken whether or not the candidate can be
///                 stored, so the PRNG stream no longer depends on how full the list is; the
///                 old per-entry loop `break`-ed at the cap and silently re-phased every
///                 downstream roll.
///                 `drop_grants_valid` drops its `% den == 0` test — the log carries a seq,
///                 not the ticket, so this side can no longer know the odds that applied.
///                 Nothing is lost: the nonce is `blake3(secret, account, seq)`, only issued
///                 for hits, so presenting a valid one already proves the grant was issued.
///                 TESTS, the ones 0.0.207 should have had: one measures candidate accrual
///                 against the real catalog and fails if any table can push stage 1 off its
///                 global rate; one checks the cap against a MEASURED 8 h offline window
///                 (`KILLS_PER_8H = 11,759`, sim-cli to band 10) and demands 2x headroom. At
///                 1% that is ~118 candidates against a 500 cap; the old pipeline wanted 7,290.
///                 KNOWN, not fixed here: `card_rate_bonus_bp` (double-card events) still only
///                 scales the client weight table, where every card now sits at weight 0 — so
///                 it currently does nothing. It should scale the stage-1 gate instead.
///                 GOLDENS: `alpha6` + `diag_band4` re-blessed — the same two 0.0.207 moved,
///                 and for the same reason (gated candidate draws shifting the stream).
/// 0.0.257-alpha — STAGE 3: the SERVER decides what a dropped piece of gear is WORTH.
///                 Star, bonus lines and skill rune were the client's rolls, and between them
///                 they are the entire value of an item: 4★ promotes it to Legendary, the
///                 lines are its stats, a rune floors it at Legendary on its own. No Legendary
///                 BASE item drops anywhere in the catalog — so a client-rolled star was the
///                 only way one could be minted, and a modified client mints as many as it
///                 likes. `docs/server-roll-drops-design.md` has always called this stage 3
///                 and gated it on "must ship BEFORE rare gear has LIVE tradeable value".
///                 The client still decides WHICH item dropped (a weighted table, low value on
///                 its own); the server decides what it is worth. `CandidateKind::Gear { boss }`
///                 carries the piece — `boss` because the generous star table is the boss's
///                 third reward axis and the server has no idea what killed what — and
///                 `GrantGear` values it from `Pcg32::new(server_roll)` in the same order the
///                 client used: star, bonuses, rune. Banks as LOOT, so rare-bag routing still
///                 applies (unlike a craft, which banks as a purchase).
///                 Third nonce domain (`gear-roll:v1`); all three now go through ONE
///                 `keyed_nonce` rather than a pasted copy each.
///                 A REAL BUG this shipped with for about a minute: `grant_craft` matched
///                 `!kind.is_drop()`, so the moment a third kind existed a craft nonce could
///                 resolve a GEAR ticket — exactly the cross-kind swap the domains exist to
///                 stop. `each_grant_resolves_only_its_own_kind` (a 3x3 matrix) caught it
///                 immediately; the handlers now match their kind EXACTLY and the tempting
///                 `always_pays()` helper is deleted with a note saying why.
///                 Cost: a piece appears one save cycle after the kill instead of instantly.
///                 The design doc accepts this ("rares materialize ~10-20 s after the kill —
///                 fine for idle"). The run summary now tallies at GRANT time, since tallying
///                 at drop time would list an item the player had not been given yet.
///                 `GEAR_CANDIDATE_CAP = 128`, sized against the window that actually exists:
///                 `SPAN_STALL_TICKS` freezes the client's sim after ~5 min unsynced, which is
///                 ~122 kills, and gear is 1-in-130 of those. `gear_tickets_fit_the_unsynced_
///                 window` asserts it even under the absurd assumption that every kill drops
///                 gear. At the cap the piece is DROPPED rather than banked un-starred:
///                 banking it would hand a client a way to mint gear the server never saw.
///                 GOLDENS: `diag_band4` re-blessed. `alpha6` did NOT move and that is
///                 consistent — it runs 600 ticks, a handful of kills, and gear is 1-in-130.
/// 0.0.258-alpha — LOAD-vs-CAP TESTS, and the cap they immediately caught.
///                 Three bugs got through a green suite on 2026-08-02/03 for one reason: the
///                 tests asserted the code does what it does, in a state the test built. A
///                 craft cap bug survived twelve passing tests because every one started from
///                 an empty candidate list while a real account sits at the cap; the inverted
///                 stage 1 ran three months because nothing MEASURED accrual. Two rules now in
///                 CLAUDE.md: a cap and its load are one decision and belong in one test, and
///                 anything numeric gets measured from the real catalog rather than asserted
///                 as a boolean. Plus: sabotage every new test before believing it.
///                 New pairs — `the_bag_fills_at_the_pace_the_loop_is_designed_around`
///                 (INVENTORY_CAPACITY vs the catalog's real gear rate: ~106 min, intent ~1 h),
///                 `the_rare_bag_cannot_fill_fast_enough_to_lose_a_jackpot` (RARE_BAG_BASE_CAP
///                 vs the 3★+ tail, reading the ACTUAL default threshold rather than a second
///                 copy of it), and `every_candidate_cap_together_stays_inside_the_save_budget`
///                 (all three caps x a really-serialized candidate, in bytes).
///                 THAT LAST ONE FAILED ON ITS FIRST RUN, which is the point of the exercise:
///                 500 + 64 + 128 tickets came to 70 KB against a ~310 KB save. `RARE_CANDIDATE_
///                 CAP` was sized for the load the INVERTED pipeline produced (0.62/kill); at
///                 the corrected 1% an 8 h session accrues ~118, so 500 was carrying 4x more
///                 dud tickets than the design ever asks for — in every save, forever, since
///                 duds only leave by TTL. Now 320: 2.7x headroom, worst case ~52 KB.
///                 Behaviour change, hence the bump — a 0.0.257 client and a 0.0.258 server
///                 disagree about accrual for any state holding 320-500 candidates, which the
///                 live account is passing through right now as its backlog drains.
///                 Also deletes `CARD_ALBUM_BASE_CAP`, vestigial since 0.0.217 and referenced
///                 by nothing — a dead cap is noise in exactly the audit this adds.
///                 GOLDENS: none move; no scenario comes near any of these caps.
/// 0.0.259-alpha — CARD EVENTS: the multiplier now works, and would have been a MINTING HOLE.
///                 `card_rate_bonus_bp` scaled the client WEIGHT table. Every card has sat at
///                 `weight: 0` since 0.0.207 moved them to server-roll, so the multiplier did
///                 nothing — except the old code was `(weight * bp / 10_000).max(1)`, and
///                 `0.max(1)` is 1. The first card event ever run would have given every card
///                 a client weight of 1, dropping cards straight from the client table at
///                 ~1/117,000 — bypassing the server roll entirely and 1.7x more often than
///                 the authored 1/200,000. Latent only because no event has run.
///                 The bonus now scales the entry's `server_bp` inside `accrue_candidate`
///                 (`card_boosted_bp`), and the weight-table function is deleted rather than
///                 left as a trap. Exact, because the final rate works out to
///                 `1% x (bp_i/W) x (10W/1e6)` = `bp_i x 1e-7` — **W cancels**, so raising one
///                 item's bp raises only that item and a double-card weekend cannot quietly
///                 halve orb drops. No `.max()` floor: an entry with no bp is not on the
///                 server-roll path, and inventing a rate for it is the original bug.
///                 Behaviour is IDENTICAL while no event is active (`card_boosted_bp` returns
///                 `server_bp` at 0 or 10000), so goldens do not move — but a 0.0.258 client
///                 against a 0.0.259 server would diverge DURING an event, hence the bump.
///                 The tests took three attempts and that is the story worth keeping. v1 tested
///                 `card_boosted_bp` directly and sailed through every sabotage of the pipeline
///                 that calls it — a helper can be correct and unreached. v2 drove
///                 `accrue_candidate` and `drop_on_death` for real, and caught the weight-table
///                 leak, but its x2 case could not see a dropped running total (a 3% shift,
///                 inside the noise). v3 adds an x50 case where the card is half the table, and
///                 DERIVES the expected count from `CANDIDATE_RATE_BP` instead of arithmetic I
///                 did by hand — which had already produced two wrong assertions in a row
///                 (ppb off by 1000x, then per-kill confused with per-ticket).
///                 GOLDENS: none move — no scenario sets `card_rate_bonus_bp`.
/// 0.0.260-alpha — PRUNE ADJUDICATED TICKETS, and the cap becomes a real backstop.
///                 The server judges EVERY pending candidate on EVERY save and reports only
///                 the hits, so the client cannot tell a dud from an unjudged ticket and holds
///                 all of them for the full TTL. That TTL is measured in TICKS, which only
///                 advance while the game is open — so a player who closes the game drains
///                 NOTHING. The live account sat at its cap overnight and would have needed
///                 ~5.4 more hours of play to clear it (user: "ไม่น่าจะ ตั้งแต่เมื่อคืน").
///                 The information was always there; the server simply never sent it.
///                 `SaveResp.resolved_seq` = the highest DROP seq this save adjudicated, and
///                 `InputEvent::PruneCandidates { up_to_seq }` (`InputAuthority::Server`) drops
///                 the rest. Same shape as `PruneMarketOps`, and recorded for the same reason:
///                 `rare_candidates` rides the state hash, so a silent local trim desyncs.
///                 DROP tickets only — craft and gear always pay and are removed by their own
///                 grants; pruning one destroys an item already earned.
///                 ORDER IS LOAD-BEARING and now pinned by a test: the same response carries
///                 the winners, and the client applies grants FIRST. Prune-then-grant deletes
///                 the winning ticket before its grant can find it. That test asserts both
///                 orders so a future reorder explains itself instead of just losing loot.
///                 Steady state goes from ~118 tickets in every save to ~1.
///                 `RARE_CANDIDATE_CAP` 320 -> 1000 (user). Not a bigger budget — a bound moved
///                 out of reach. 320 assumed the measured 11,759 kills/8 h, which came from a
///                 THREE-member party and is a floor: an endgame party at 3x that rate has a
///                 steady state of ~353 and would have started hitting 320. 1,000 needs 100,000
///                 kills inside one TTL window, ~68 h of play. The rule this encodes: a bound
///                 should be unreachable or loud, never quietly reachable — 0.0.255 was the
///                 alternative.
///                 The byte-budget test was mismeasuring and blocked the change: it summed all
///                 three caps as if they could fill at once, ignoring that the TTL alone holds
///                 the drop list near steady state whatever the cap is. Split in two — what a
///                 save actually carries (tight, 24 KB) and the both-drains-broken blast radius
///                 (loose, 192 KB). A test that fails a correct change is mismeasuring, not
///                 strict.
///                 GOLDENS: none move — no scenario prunes or comes near a cap.
/// 0.0.261-alpha — THE REPLAY HOLE: four projectile transients the sim READS at impact.
///                 `Projectile.is_basic` / `burn_tier` / `focus_mult_bp` / `pin_slow_pct` were
///                 `#[serde(skip)]`, each with a comment arguing it was harmless because "a
///                 reload mid-flight just drops it for that one shot". That is true for a
///                 player reloading a save and FALSE for replay-verify: the anchor is
///                 serialized while a shot is airborne, the client keeps the value in memory
///                 and applies the burn/slow/multiplier/card-proc at impact, the server
///                 deserializes zeroes and does not. The states part with an EMPTY input log —
///                 the save is rejected and there is nothing to point at, because the cause was
///                 never an event. Same class as the 0.0.196 combat-field hole; the reasoning
///                 that reopened it is written verbatim above each field.
///                 Now `skip_serializing_if` (the shape `attacker_id` beside them already
///                 used): absent at the default, carried when set.
///                 Found from a real rejected bundle — `log: []`, 100 ticks, hash differs, which
///                 leaves only an unrecorded mutation or an anchor that loses state.
///                 `a_deserialized_anchor_continues_identically` covers the general contract,
///                 not the four fields: freeze mid-flight, continue from the WIRE form, hashes
///                 must not part. `replay_reproduces_a_recorded_session` could never have caught
///                 this — it anchors with `clone()`, which copies exactly the fields the wire
///                 drops. Sabotage-checked (re-skip `burn_tier` -> red).
///                 GOLDENS: `beta7` re-blessed, ALONE and for the right reason — it hashes with
///                 two basic bolts still in flight, so the state now carries `is_basic: true`
///                 that it silently lost before. Every behavioural assert in it still passes.
/// 0.0.262-alpha — XP STOPS BORROWING THE HP SCALER.
///                 `ZoneDef.xp_scale_bp: Option<u32>` — `None` follows `hp_scale_bp`, which is
///                 what every authored zone does today, so nothing changed anywhere.
///                 It exists because the two were ONE number and that made the obvious balance
///                 move inexpressible: softening a wall by cutting monster HP also cut the XP for
///                 killing it, so "too hard" and "pays too little" were the same edit. The
///                 coupling was never decided — XP borrowed HP's scaler when sub-bands landed in
///                 0.0.226 (user: "ไม่ค่อย make sense ที่เราจะ tune xp กับ hp แยกกันไม่ได้").
///                 `xp_scaling_follows_hp_until_a_zone_separates_them` pins BOTH halves: still
///                 follows when the zone is silent, stops the moment it is authored.
///                 GOLDENS: none move — no zone authors the field.
/// 0.0.263-alpha — REGALIA: 5 class families x 3 rarities, as a TABLE and as stat sticks.
///                 Was 2 hand-written weapons in a `match`; five families times three rarities is
///                 15, at which point a balance pass means editing fifteen places and a new class
///                 means remembering five. `REGALIA_LINES` (family -> axis + intended classes +
///                 ids + names) x `REGALIA_LADDER` (the per-rarity budget) generates all of them.
///                 Adding a class is one row; re-balancing a rarity is one row.
///                 FOUR AXES, because flat and percent carry opposite ends of the level curve.
///                 Measured 2026-08-03: a lead has ~110 HP at the Lv7 grant point, 570 at Lv20,
///                 2,001 at Lv40, 6,314 at Lv70 — HP grows ~57x while effective ATK grows ~3x. A
///                 flat HP that matters early is noise at cap; a percentage that matters at cap is
///                 nothing early (user: "ต้นเกมผู้เล่นยังเลเวลน้อย เดเมจน้อย +% ไม่เห็นผล").
///                 Levels and stars scale the FLAT axes ONLY — the percentages are the rarity's
///                 signature, so investment can never turn a fed R into an SSR.
///                 R is deliberately generous (20 flat / 100 HP / +3% / +5%): it is the free tier
///                 and its job is to carry a new party through the early bands.
///                 Mage and acolyte lines scale MATK; swordsman/thief/archer scale ATK.
///                 NOT gated by class. `regalia_allowed_classes` stays unrestricted as it was
///                 documented to be, and the new `regalia_intended_classes` is for PICKING a line
///                 (a grant, a banner, a shop row), never for refusing an equip — a mage's sigil
///                 on a swordsman is already a bad idea because it scales the wrong pool.
///                 `aurora_edge` / `litany_of_dawn` keep their names as the SSR of their own
///                 lines; both lost their bespoke CritBp/Int lines to the uniform ladder.
///                 NAMES are standalone. The first pass gave each family a shared noun with an
///                 escalating qualifier (Squire's / Vigil / Aurora Edge) and it read as one
///                 weapon evolving rather than three artifacts worth wanting separately; rarity
///                 now escalates through meaning, not through a repeated word.
///                 `every_class_has_a_regalia_at_every_rarity_and_the_ladder_climbs` checks PER
///                 AXIS. Its first version summed all four with weights and passed with SSR's flat
///                 cut to a fifth of SR's, because the percentages alone carried the total — a
///                 blended score can always hide a broken row inside a healthy one. Sabotage-
///                 checked on the flat and HP axes separately.
///                 GOLDENS: none move — no scenario equips a Regalia.
/// 0.0.264-alpha — EXEMPLAR ladder: 5 class families x SR/SSR, same table treatment as Regalia.
///                 `EXEMPLAR_LINES` x `EXEMPLAR_LADDER`; the two heroes that shipped first keep
///                 their hand-authored kits through `authored_skills`, everything else is a stat
///                 stick until the skill slice lands.
///                 Stat per family: swordsman STR, archer DEX, mage/acolyte INT — and thief AGI,
///                 because AGI drives ASPD (`√(AGI·9.999 + DEX·0.19212)`) so it converts to damage
///                 through swing rate rather than StatusATK, which keeps the dagger family from
///                 being a second sword family.
///                 NO HP axis: that belongs to the Regalia, so hero + weapon add up to one
///                 character instead of two copies of the same one.
///                 Budget sits close to a maxed Regalia rather than above it even though a
///                 character copy costs ~3x a weapon copy on the planned banners. As pure stat
///                 sticks they SHOULD look alike — the character premium has to be paid for by
///                 skills and skin, and if raw numbers ever pay for it instead, the weapon banner
///                 becomes the only rational pull.
///                 Seraphine went from +5 STR/+10 ATK to the SSR budget (+12/+40/+8%), which is a
///                 real buff to an existing hero; both her tests were updated, not weakened.
///                 KNOWN GAP (user 2026-08-03, deferred): an Exemplar's only growth axis is
///                 STARS, i.e. duplicates, while a Regalia also takes Enhancement levels from
///                 fodder. A f2p player holding one copy holds a fixed number forever. Closing it
///                 means an Exemplar level axis and RE-SPLITTING this budget, not adding to it.
///                 GOLDENS: none move — no scenario equips an overlay.
/// 0.0.265-alpha — PROVISIONS groundwork: a Meal that wards, and a downed member that earns.
///                 `ConsumeEffect::Buffs([ConsumeBuff; 2])` — food is a stat line AND a ward, and
///                 one item handing out both is what a player expects. A separate variant, not a
///                 `Buff`-turned-list, so every existing consumable serializes unchanged; a fixed
///                 pair, not a `Vec`, because `ConsumeEffect` is `Copy` and the auto-use loop
///                 matches it by value.
///                 `BuffKind::ResistPoisonBp` + `resist_status_bp` now sums BUFFS as well as
///                 gear/card/overlay mods. It could not before, so no consumable could ward
///                 anything — the status-resist chain existed end to end and had exactly one
///                 unreachable half.
///                 `loot_field_ration` becomes MEAL I ("Trail Meal"): +3 all stats AND 50% poison
///                 ward, 1 h -> 2 h. Repointed rather than duplicated — it was already +3 all
///                 stats for an hour, so a second item would have been indistinguishable. The
///                 def_id is untouched: saves hold it. Region 4 is why the ward starts at tier I —
///                 `toxic_chom` is in all three of its sub-band pools and `toxic_burst` applies
///                 classic Poison (DEF -25%, 1.5% MaxHP/s, SP regen off) on hit, so the poison
///                 arrives before any second tier would.
///                 Ward caps at "brief", not immune: `buffs.rs` floors the shortened duration at
///                 1 tick, so 100% resistance still lets the status land for an instant.
///                 XP: a DOWNED party member now earns its share, and the share DENOMINATOR
///                 counts the whole party rather than the survivors. Skipping the downed punished
///                 exactly the member already behind — die, miss the band's XP, fall further
///                 behind — and shrinking the divisor quietly paid the survivors MORE for the same
///                 kill, so fixing one half alone would have been a buff to dying (user: "ไม่ให้
///                 ตัวละครเลเวลห่างกัน"). `alive` gates who FIGHTS, not who is on the payroll.
///                 GOLDENS: `diag_band4` re-blessed ALONE — it is the only scenario where a party
///                 member goes down, which is exactly the case that changed. Its behavioural
///                 assertions all still pass.
/// 0.0.266-alpha — PROVISIONS: two RESERVED slots, held apart from the emergency bar.
///                 `Entity.provisions` (serde-skipped when empty, so old saves and goldens are
///                 byte-identical) holds at most one Meal and one Tonic, chosen by a new
///                 `ConsumableSpec.provision: Option<ProvisionKind>` that every existing
///                 consumable leaves absent.
///                 RESERVED and not three-more-bar-slots on purpose: food competing with a
///                 healing phial for the same slots is not an interesting choice — a player
///                 takes "do not die" every time and the buff line would never be used.
///                 Only an item that DECLARES a kind may enter, and a second of the same kind
///                 REPLACES rather than being refused; a slot that can hold exactly one thing
///                 should not make swapping it a two-step chore.
///                 `AddProvision` / `RemoveProvision` mirror the consumable pair. The auto-use
///                 upkeep runs the SAME body over both lists (the vec is moved out for the pass
///                 so `&mut e` stays available), bar first then reserved — a fixed order, so what
///                 fires never depends on layout.
///                 `loot_field_ration` -> Meal, `aspd_pot_1..3` -> Tonic.
///                 `AddProvision` SPLITS the stack (`qty`) rather than moving it whole. Slots are
///                 per-character but stackables merge into ONE inventory stack, so a whole-stack
///                 move let the first member take all 100 Meals and left the other four unable to
///                 eat — the per-character design broke at the second party member (user asked
///                 "inventory เหลือ 90?", which is what exposed it). Topping up the same food
///                 merges; a different food of the same kind swaps and returns the old stack.
///                 The emergency bar still moves whole stacks: its event has recorded history, and
///                 adding a field would replay old logs as "move zero".
///                 GOLDENS: none move — no scenario slots a provision, and the new field is
///                 absent on every existing item.
/// 0.0.267-alpha — COOKING: four food materials, a Kitchen bench, and the Meal line.
///                 `CraftStation::Kitchen` joins blacksmith/apothecary. Separate from the
///                 apothecary because a Meal and a Tonic sit in DIFFERENT reserved slots and are
///                 farmed from different materials — one bench would put two lines that never
///                 compete on the same screen.
///                 Materials drop from families that already exist rather than from a new race
///                 rule: `slime_jelly` (greenling/pinkling, regions 1-2), `chom_sprout` (chom,
///                 3-4), `bitter_root` (demon_tree, 5), `beast_meat` (werewolf/red_hood, 6-7).
///                 Each tier therefore unlocks where its ingredient lives — Meal II arrives in
///                 region 3-4, which is exactly where `toxic_chom` poisons the party, so the ward
///                 lands on time without a rule saying so.
///                 Drop rate 1 in 60, expressed against each monster's OWN table total because
///                 the tables differ in size and a fixed weight would mean a different rate per
///                 family.
///                 THE RECIPE IS THE CONSTRAINT, NOT THE DROP. Measured: a five-member party
///                 kills ~2,450/h (from the 11,759-per-8h figure), so 1/60 yields ~41 materials
///                 an hour against ~2.5 meals eaten — 27x more supply than demand. One meal per
///                 craft at 8 materials puts consumption near 20/h, which leaves headroom without
///                 making food free. Retune THAT number, not the drop.
///                 Meal I +3 all / 50% ward · Hearty Stew +8 / 70% · Commander's Feast +15 / 90%
///                 (wider gaps, user). No separate HP line on the Feast: +15 to ALL stats already
///                 includes VIT, and spending the buff pair's second slot on HP would have cost
///                 the poison ward, which is the reason the line exists.
///                 Speed Potion II/III finally have recipes; they stay at the apothecary because
///                 a tonic is brewed in a flask, not cooked.
///                 GOLDENS: `alpha6` and `diag_band4` re-blessed — adding a drop row changes both
///                 the spawn template and the roll, which is exactly the documented reason an
///                 ItemDef edit moves a hash. Every behavioural assert in both still passes.
/// 0.0.268-alpha — COOKING part 2: nine dishes on THREE role lines, not one ladder.
///                 The three meals were the same dish three times — `AllStatAdd` +
///                 `ResistPoisonBp`, only the magnitude moving. Now every band offers ATK / MATK
///                 / defensive, so which dish a member eats is a build decision (user: a menu by
///                 role, not a ladder by level).
///                 `ConsumeEffect::Buffs` is a fixed `[ConsumeBuff; 2]`, so a dish gets the
///                 all-stat line plus exactly ONE specialisation. The all-stat line stays on all
///                 nine — it is what makes a meal a meal, and it keeps the dishes comparable.
///                 Stats +3 / +5 / +7 for Lv1-30 / 31-50 / 51-70 (user; bigger numbers are held
///                 for the cap raises). Role lines 5% / 8% / 12% as BASIS POINTS — `apply_pct`
///                 divides by 10_000, and the `HpPct` doc comment claiming whole percent is
///                 wrong.
///                 THE WARD WAS LYING. `CAP_RESIST_STATUS_BP` is 7500, so the Feast's authored
///                 90% was clamped and the top meal warded 5 points better than the middle one,
///                 not 20. Poison now sits on Trail Meal alone at 30%, under the cap and with
///                 room left for a future dish.
///                 FIVE NEW MATERIALS, placed BY BAND rather than by family: `dew_cap` (b1-6),
///                 `ember_root` (b7-12), `wild_honey` + `cave_garlic` (b16-30), `night_truffle`
///                 (b22-30). By family would have meant a pool where only some monsters carry the
///                 drop, diluting it by the ones that do not. Weight is carved OUT of each
///                 monster's `nothing` filler, never appended — appending raises the table total
///                 and silently thins every gear rate the forge recipes are balanced against.
///                 `lycan_fang` / `grave_wax` were deliberately NOT reused: twelve forge recipes
///                 consume them, so feeding the party would have accelerated gear crafting.
///                 MEASURED, and the first attempt failed it: seven of nine dishes were
///                 uncraftable because their two materials never shared a zone, or accrued too
///                 slowly.
///                 THE TARGET IS DELIBERATELY BELOW FULL UPTIME. Five members on a 2h buff eat
///                 2.5 meals/h; one farming spot yields 1.0-1.23, so a party feeds about two of
///                 five and buys the rest (user: keeping everyone fed must NOT be self-service,
///                 or nobody trades). Tuned on the RECIPE QUANTITIES, not the drop rates —
///                 materials should keep pouring out, because they are what players in different
///                 bands swap; it is the cooked dish that has to be scarce.
///                 The per-dish figure understates it. A mixed party (2 melee / 2 caster / 1
///                 front-liner) needs ATK, MATK and a defensive dish AT ONCE, and those live in
///                 different bands: the best zone in the game covers 48% of that demand, and the
///                 early zones cover 0% because b1-6 has no ATK dish and b7-12 no MATK dish.
///                 Halving the quantities was tried and reverted — it pushed b25 to exactly 100%
///                 coverage, i.e. one zone where a party never needs the market.
///                 That pair — 2.5/h eaten vs 1.0-1.23/h farmed — is the whole balance. Change
///                 the buff duration, the party size, or a quantity, and re-derive the others.
///                 GOLDENS: `alpha6` and `diag_band4` re-blessed — new drop rows change both the
///                 spawn template and the roll, the documented reason an ItemDef edit moves a
///                 hash.
/// 0.0.269-alpha — SHADOW STRIKE: the blink and the strike are ONE TICK APART.
///                 They resolved in the same instant, so the thief arrived and hit
///                 simultaneously and no animation could show the arrival — the sprite was at
///                 the destination on the same frame the damage popped (user: "0-3 teleport,
///                 4 appear, 5 hit").
///                 Spent as `cast_time_ticks: 1` rather than a new mechanism: `begin_channel`
///                 already means "act now, resolve later", so the blink moves there and
///                 `resolve_instant` skips it when `base_cast_time > 0`. An instant teleport
///                 skill (Dark Ambush) still blinks at resolve and is unchanged.
///                 The wind-up is FLOORED at 1 tick for a teleport. `effective_cast_time` is
///                 DEX-scaled and would round a 1-tick base to nothing, so a high-DEX thief
///                 would silently lose the gap while a low-DEX one kept it — a blink's tick is
///                 structural, not a cast bar to shorten.
///                 ⚠️ BALANCE, not just presentation: in that tick the target can die to
///                 someone else or walk out from under the strike, and this skill deliberately
///                 picks the LOWEST-HP enemy — the one most likely to do exactly that. Shadow
///                 Strike now whiffs sometimes. Accepted by the user for the animation.
///                 GOLDENS: none moved — all 36 pass unblessed, because no golden scenario has
///                 a thief that casts it. That is a COVERAGE gap, not a clean bill: the sim's
///                 signature thief skill can change behaviour without a single hash noticing.
///                 VERSION is bumped even though the serialized SHAPE is unchanged: an input
///                 log recorded before this replays differently, which is exactly the skew the
///                 version gate exists to refuse.
/// 0.0.270-alpha — SHADOW STRIKE: 0.0.269 REVERTED. The blink is instant again.
///                 0.0.269 split the blink and the strike across a tick so the vanish had
///                 somewhere to play. That was the wrong layer, twice over: it turned the
///                 thief's signature INSTANT engage into a cast (user: "shadow strike
///                 มันไม่มี cast time นะ"), and it widened the whiff window on a skill that
///                 deliberately targets the enemy most likely to die first.
///                 It was also unnecessary. `preview_pending_skill` already tells the renderer
///                 which skill the NEXT swing will cast, precisely so the wind-up can animate
///                 before the resolve tick — its own doc says `skill_fx` "only arrives on the
///                 resolve tick, too late to animate the swing that cast it". The wind-up
///                 therefore plays at the OLD cell for free, and the animator already HOLDS the
///                 `impact` frame until the sim's strike tick.
///                 So the fix is one number in the client: `impact` = the frame the thief is
///                 fully gone. The clip dissolves, PAUSES on empty for as long as the swing has
///                 left to run, then the strike cuts the position and releases the arrival
///                 frames. The pause absorbs the slack, so it reads right at any attack speed —
///                 which the 0.0.269 fixed 3-tick gap could not do.
///                 `cast_time_ticks` is removed from Shadow Strike, Dark Ambush and Puppet
///                 Lunge; `anim_slot` on all three STAYS (renderer-only, and correct).
///                 GOLDENS: unmoved, same as 0.0.269 — no golden casts any of the three.
///                 VERSION bumped because 0.0.269 SHIPPED: reverting behaviour is itself a
///                 behaviour change against what the server is running, and needs a deploy.
/// 0.0.271-alpha — IDLE WANDER (monsters): nothing to fight is no longer nothing to do.
///                 With no enemy the target-selection `None` branch only cleared `target_id` and
///                 left the `target` CELL wherever it was, so a monster stood frozen on the spot
///                 (user).
///                 `wander_step`: walk 2-5 cells, stand 3-5 s, repeat. The PAUSE is the point —
///                 re-aiming on arrival makes it walk continuously, which reads as patrolling
///                 rather than idling. Needs one field (`Entity.wander_until`, serde-skipped at
///                 0, so every existing save and golden serializes unchanged) because "currently
///                 pausing" cannot be recomputed from position alone.
///                 NO PRNG DRAW — direction and timings are a hash of `(entity id, seed)`. A draw
///                 would advance the one shared stream and change every downstream roll (drops,
///                 refine, cards) to pay for an idle animation. Two odd multipliers and a rotate,
///                 because a plain `id % n` marches adjacent ids in formation. Legs are seeded off
///                 `wander_until` rather than the tick, or every monster waking on the same tick
///                 would pick the same heading.
///                 PLAYERS RE-FORM instead of strolling — the other half of the report. They
///                 pile onto one cell because they all chase the SAME nearest enemy, converge on
///                 it, and stay where the last one died; the answer is formation, not wandering
///                 (a member that drifts is out of position when the next pack spawns).
///                 Anchored on the LEAD's cell, not the zone's spawn point: `tick()` takes no
///                 `GameData` by design, so `zone.player_spawn` is unreachable from there. That
///                 choice also makes it safe by construction — the lead's own offset is (0,0),
///                 so a party of ONE never moves, which is precisely the walk-to-fixed-target
///                 contract `golden_alpha2` pins. A first attempt that nudged every idle player
///                 broke that test; the fix was a better anchor, not a re-pin.
///                 Reuses `party_cell`, the same formation a zone places the party with, so idle
///                 spacing and arrival spacing cannot drift apart.
///                 GOLDENS: all 36 pass UNMOVED — no golden has an idle monster, since they meet
///                 the party on spawn. Worth stating plainly: that is a coverage gap, not
///                 evidence the change is inert. Idle positions decide who is in range when the
///                 next pack arrives, so engagement timing does move in real play.
/// 0.0.272-alpha — CAPPED ZONES: a zone can now say how many monsters it will ever produce.
///                 Story stages need a DESIGNED ENCOUNTER — ten pinklings and a boss, kill them
///                 and move on — and there was no way to express that (user 2026-08-07). Every
///                 band zone trickles forever to hold `target_population`, so the only end
///                 condition available was a kill quota, which is a tap you turn off rather than
///                 a fixed fight: with threshold 10 and a floor of 3, thirteen monsters appear.
///                 `ZoneDef.max_spawns` / `ZoneSpawnState.max_spawns`, 0 = endless. The gate is
///                 derived from `kill_counter + alive` rather than a new counter, so there is no
///                 second number that can drift out of step with reality: what a zone has ever
///                 produced is exactly what is dead plus what is standing.
///                 SERIALIZATION UNCHANGED for every existing zone — skip-if-zero, the same trick
///                 `boss_from_pool` uses. ALL 36 GOLDENS PASS UNMOVED, which is the point: this is
///                 opt-in data, not a change to the farm loop.
///                 The new test COUNTS what a capped zone produces (exactly 10) and asserts an
///                 UNcapped one keeps going past 100, because a wrong derivation would still stop
///                 eventually, just at the wrong number. Deleting the gate makes it read 4002.
/// 0.0.273-alpha — DEF/MDEF to RENEWAL. Audited the whole pair against the rAthena source and
///                 the iRO Wiki (both DBs now under `docs/reference/`); five things were already
///                 right — the hard DEF curve reproduces the wiki's own worked examples exactly
///                 (DEF 275 → 36.7% for "about a third", 500 → 50.0% for "half") — and four were
///                 not. All four fixed here (user: use post-renewal).
///                 (1) MAGIC IS TWO-LAYER NOW. It was `damage − mdef`, a single flat subtraction,
///                 so a monster's MDEF was crushing against an early nuke and irrelevant against a
///                 late one: a constant does not scale with the game. Renewal applies the same
///                 shape as DEF with its own constant, `× (1000+m)/(1000+10m)`, then subtracts
///                 soft. K=1000 is MAGIC's constant — DEF's 4000 was never wrong.
///                 Four times steeper, so a point of MDEF is worth far more than a point of DEF.
///                 (2) PLAYER soft MDEF `INT/5` → `INT + VIT/5 + DEX/5 + Lv/4`. The old one traced
///                 to 0.0.14, the day magic shipped, and matched neither edition (pre-re is
///                 `INT + VIT/2`); MATK had already moved to renewal, so this was the last thing
///                 in `stats.rs` still on the day-one guess. A Lv70 INT60 player goes 12 → 93.
///                 (3) MONSTERS GET SOFT DEF AND SOFT MDEF, which they had NEITHER of: both paths
///                 short-circuited before a stat block monsters never fill. Renewal gives them
///                 their OWN formulas — `(VIT + Lv)/2` and `(INT + Lv)/4` — which the wiki states
///                 outright and rAthena implements. Their level is `mob_level`, not `stats.level`.
///                 (4) Keyed on TEAM, not on `mob_level > 0`. A monster with an unset level
///                 (`teenager_pinkling` is one) would otherwise fall through to the PLAYER formula.
///                 The unit tests caught that: two fixtures are `Team::Monster` with no level.
///                 GOLDENS: 3 of 15 re-blessed, and each for an understood reason. `alpha6` and
///                 `diag_band4` load real monsters, which now HAVE soft DEF. `beta6`'s slime is
///                 hand-built with AGI 100 and was drawing 20 soft DEF out of that AGI — through
///                 the player formula, which has an AGI term the monster one does not. A slime
///                 built to be dodgy was quietly armoured too. The other 12 hold, including the
///                 magic golden `beta7`, whose target has MDEF 0 so neither new layer engages.
///                 NOT MEASURED YET: monster soft DEF is `Lv/2` at VIT 0, i.e. 25 flat off every
///                 physical hit at level 50, and that is the largest balance shift in the set. The
///                 7-day sim-cli comparison is the acceptance test, not these hashes.
/// 0.0.274-alpha — MONSTER LEVEL, NOT MONSTER-PER-ZONE. A monster's strength used to be the
///                 numbers typed on its own def, optionally multiplied by a scale its ZONE
///                 carried. Two consequences, both bad: the same monster was a different creature
///                 in two zones, and there was no way to ask "how strong is a level 40 monster?"
///                 because nothing in the catalog answered at a level. Story mode needs exactly
///                 that question answered — it picks a monster and a level with no zone involved.
///                 The model is now `level_base[1..100] × the monster's own multiplier`:
///                 `GameData.level_base` is a 100-row table (Lv1 hp 39 / atk 5, Lv100 hp 17697 /
///                 atk 835), each monster carries `hp_mult_bp` / `atk_mult_bp`, and a zone carries
///                 a `level` instead of a scale. `base_scaled_stats` reads the row for the zone's
///                 level; `atk_min`/`atk_max` keep their authored SPREAD by riding the same ratio.
///                 DERIVED, NOT INVENTED — `tools/derive_monster_levels.py` read the shipped
///                 catalog back out: band→level is `floor(band × 5/3)`, each level's base is the
///                 geometric mean of that band's pool, and each multiplier is that monster's
///                 median share of it. So the table reproduces what was already balanced rather
///                 than re-tuning it: 185 of 1312 (stat, zone) placements move more than 2%, and
///                 they collapse to 9 distinct (stat, monster) pairs — the 5 gothic mobs, whose
///                 authored numbers never sat on any band curve, plus `boss_b9` in Duskmourn room
///                 1, a room whose HP:ATK ratio matches no single table row. Everything else moves
///                 by quantisation only.
///                 Also gone: `scale_from_base` (12 placeholder monsters deleted with it) and
///                 every zone `hp_scale_bp` / `atk_scale_bp`. An empty `level_base` falls back to
///                 the monster's authored stats so a hand-built test fixture still works.
///                 GOLDENS: 2 of 15 re-blessed, both for the same one-unit reason. `alpha6` and
///                 `diag_band4` are the only two that load a real zone, and a zone's monster
///                 TEMPLATES live inside `ZoneSpawnState` — so `teenager_pinkling` hp 55 → 54 and
///                 `big_pinkling` hp 120 → 119 / atk 16 → 15 move the state hash even before a
///                 blow is struck. The other 13 build their monsters by hand and are untouched.
///                 A test that read a boss's strength off `d.monster(id).hp` was corrected to read
///                 the BUILT template — the catalog number is no longer what walks onto the field.
/// 0.0.275-alpha — MDEF RE-POINTED TO `round(def/4)`. 0.0.273 gave magic RO's two-layer shape but
///                 left the DATA where the flat era had put it, and the two disagreed badly. Every
///                 monster shipped with `mdef ≈ 0.9 × def`, which reads as "roughly as armoured
///                 against spells as against steel" and is nothing of the kind: the curves run
///                 different constants, 4000 for DEF and 1000 for MDEF, so the same number in the
///                 two columns is worth four times more in the MDEF one. Measured on the shipped
///                 catalog, magic was mitigated about THREE TIMES harder than physical at every
///                 level — Lv96 mobs shed 20.8% of a nuke and 6.9% of a swing — and the two
///                 Duskmourn bosses were outright magic walls at 54% (dullahan, mdef 150) and
///                 42.6% (brutus_lord, mdef 90). Those two numbers were harmless when magic was
///                 `damage − mdef`; the curve turned them into percentages overnight.
///                 THE RATIO IS DERIVED, NOT CHOSEN. `(1000 + d/4)/(1000 + 10·d/4)` is
///                 algebraically `(4000 + d)/(4000 + 10·d)`, so a quarter is exactly where the
///                 steeper curve and the smaller number cancel. RO's own renewal mob DB sits
///                 there: median MDEF/DEF is 0.19–0.28 across the levels we ship (n=2440, DB under
///                 `docs/reference/`). 61 monsters moved; worst departure from parity is +0.43pp,
///                 pure integer rounding, exact wherever DEF divides by four.
///                 Five monsters had `def 0, mdef 10` — plants and slimes with no armour but 8.3%
///                 spell resistance, an authoring default nobody chose. They go to 0.
///                 NEW GUARD, `no_monster_shrugs_off_magic_much_harder_than_it_shrugs_off_steel`:
///                 fires equal Physical and Magic packets at every catalog monster through the
///                 REAL `apply_damage` (no re-derived formula) at `mob_level` 0 so only the hard
///                 layer speaks, and fails if magic is mitigated more than 1.0pp harder. The bound
///                 was placed by sweeping every candidate rule, not picked: def/4 tops out at
///                 +0.43pp, def/3 at +6.04, def/2 at +15.21, the old ~0.9× at +29.94. It is
///                 one-sided on purpose — def/5 passes, because a monster that shrugs off LESS
///                 magic than steel is the squishy-caster archetype, not a bug. Sabotaged in three
///                 directions before being believed.
///                 GOLDENS: the same 2 of 15, and for a smaller reason than last time. `alpha6`
///                 and `diag_band4` are the only scenarios that load a real zone, and their
///                 pinklings had `mdef 0` — magic passed through untouched. They now carry mdef 1
///                 and 3, matching the 1.1% and 2.2% their DEF already gave. The rule pulled them
///                 UP. `beta7`, the magic golden, holds: its target has MDEF 0 by construction.
/// 0.0.276-alpha — EXTRAPOLATION FIX + two guards, all three from an audit of the 0.0.273/275
///                 fallout rather than from a failing test — nothing was failing.
///                 (1) `level_stats` past the authored table applied the last decade's growth ratio
///                 once per LEVEL instead of once per DECADE; `span` was computed and then thrown
///                 away with `let _ = span;` while the comment above it said `^(over/span)`. It
///                 compounds: level 110 came back with 678,728 HP where the curve wants 25,484, 26x
///                 over and worsening every level. Never fired in play only because the shipped
///                 table stops at 100 and the deepest zone is level 100 — but a story mission names
///                 any level it likes, and the cap raise cadence is 80/90/99. Now walks whole spans
///                 and interpolates the remainder linearly: integer-only, monotone, exact on every
///                 span boundary.
///                 (2) `past_the_table_a_decade_grows_by_one_decade_not_by_ten` MEASURES the
///                 extrapolated slope against the table's own rather than asserting it returned
///                 something — the shape of assertion the 26x bug lived under. Its first version
///                 SURVIVED its own sabotage (dropping the remainder left the decade boundaries
///                 correct and only flattened the levels between, which nothing sampled); it now
///                 samples mid-decade and requires strict monotonicity.
///                 (3) `mdef_and_glacial_storms_freeze_rate_cannot_drift_apart` binds two things
///                 that were already coupled and undocumented. The freeze roll reads
///                 `effective_mdef` as PERCENTAGE POINTS — `chance x (97 - MDEF - LUK/3)/100` —
///                 and that number is hard + soft, two halves with different jobs, both of which
///                 moved this week. Between 0.0.273 and 0.0.275 a level-98 mob's resist term went
///                 54 to 32, i.e. Glacial Storm became half again as likely to freeze it, with no
///                 freeze test in the repo to notice. The rate is now pinned at five levels.
///                 NOT FIXED, and it is a design call rather than a defect: rAthena's
///                 `status_get_sc_def` (read from source, not memory) computes renewal freeze as
///                 `sc_def = mdef*100 - levelAdv` applied `rate -= rate*sc_def/10000`, using
///                 `status->mdef` — the HARD half only — with no LUK term (LUK is the pre-renewal
///                 branch) and a flat +3 s duration instead of an MDEF-scaled cut. Ours is a
///                 pre-renewal/community hybrid. Moving it to renewal proper would make players
///                 freezable at the full rate, since nothing in this game grants a player hard
///                 MDEF at all.
///                 GOLDENS: none. The extrapolation path is unreachable below level 101 and no
///                 zone goes there, which is exactly why it survived.
/// 0.0.277-alpha — ARMOUR IS THE ZONE'S LEVEL. `def` joins hp/atk/xp on the level table and stops
///                 being a number typed on the monster; `mdef` is derived from it at a quarter and
///                 is never authored at all. Identity moves to `def_mult_bp`, the same place HP's
///                 and ATK's already lives.
///                 The old column had drifted into a curve so shallow it did nothing: DEF 5 at
///                 level 1 rising to 33 at level 96, i.e. 1.1% to 6.9% off a physical hit across
///                 the WHOLE game, against RO renewal's own mobs at 14.7% for the same band. Worth
///                 noting it was not random — our authored DEF correlated 0.92 with level at a
///                 slope of 0.284, so someone was already tracking level by hand, just 3.5x too
///                 shallowly. `def = level` puts us on RO's number (13.4% at the launch cap of 70
///                 vs their measured 14.7%).
///                 MEASURED BEFORE SHIPPING, with the response curve mapped so "small" could be
///                 told apart from "unmeasurable" — 7-day sim-cli, mixed party, final base level:
///                 def x0.5 -> 56, def = lv -> 56, x2 -> 54, x3 -> 54, x5 -> 52, against 57 today.
///                 So the real change costs ONE level in a week and the harness would have shown
///                 five. A pure-swordsman party ends at 56 either way. Wipes go DOWN (1220 ->
///                 1112), which is what a slower party fighting relatively easier content looks
///                 like.
///                 The two Duskmourn bosses keep their authored armour through `def_mult_bp`
///                 (dullahan 5.5x, brutus_lord 4.0x): theirs was tuned at RO-MVP scale and is the
///                 point of them, and `def = level` alone would have dropped dullahan from 31.9%
///                 to 10.2% as a side effect of a change about ordinary mobs. Three now-dead
///                 columns went with it — `mdef_mult_bp`, `hit_mult_bp`, `flee_mult_bp`, authored
///                 and read by nothing.
///                 THREE HOPS: this is a change about PHYSICAL mitigation, and because MDEF rides
///                 DEF and the freeze roll reads MDEF, it cut Glacial Storm's freeze rate against
///                 a level-98 mob by 26%. Caught by the guard added in 0.0.276 one commit earlier,
///                 which is the entire reason that guard exists. Accepted and re-pinned.
///                 GOLDENS: 1 of 15. `alpha6` runs zone_b1_1 where DEF falls 5 -> 1 and damage
///                 numbers actually move. `diag_band4` does NOT move, and the reason is worth
///                 recording because it looks like a stale build for an hour: that scenario runs
///                 long enough to walk from zone_b4_1 (level 6) into a level-10 zone sharing the
///                 same pool, and pinkling's authored `def 10 / mdef 3` happens to equal that
///                 zone's level and its quarter exactly. Verified against a full `cargo clean`.
/// 0.0.278-alpha — MDEF RIDES THE LEVEL, NOT THE MONSTER'S DEF (user call). One line, and it
///                 changes exactly two monsters in the catalog — which is the whole point of it.
///                 Since 0.0.277 an ordinary monster wears its zone's level as armour, so `def/4`
///                 and `level/4` are the same number for all but the monsters carrying a
///                 `def_mult_bp`: today the two Duskmourn bosses. Under the old rule their 4-5.5x
///                 plate bought them the same multiple of MAGIC armour — dullahan shed 31.9% of a
///                 nuke as well as 31.9% of a sword, and nothing a party could bring answered it.
///                 They now sit at 8.2% against a spell and 25.7% / 31.9% against steel: an
///                 armoured boss has a shape you can build against, which is the point of building
///                 a party.
///                 The parity property survives where it matters — an ordinary monster still
///                 mitigates magic and steel identically, because a quarter is where MDEF's
///                 K=1000 curve and DEF's K=4000 cancel, and its def IS its level.
///                 AGAINST THE DATA, DELIBERATELY. RO's renewal DB correlates mob MDEF with DEF,
///                 not with level, inside the levels we ship: r = 0.495 vs 0.044 across 1334
///                 monsters, and partial correlation says level adds NOTHING once DEF is known
///                 (-0.023) while DEF still explains MDEF knowing level (0.494). The whole-DB
///                 figure that looks even (0.496 vs 0.484) is carried by levels 101-200 where
///                 everything scales together. So this is a design departure with its eyes open,
///                 not a correction — recorded here because the next person to measure it will
///                 find RO disagreeing.
///                 THE GUARD HAD TO BE REWRITTEN, and this is the more important half of the
///                 commit: `no_monster_shrugs_off_magic_much_harder_than_it_shrugs_off_steel` was
///                 reading `data.monsters[i].def/.mdef`, columns that 0.0.277 turned into leftovers
///                 the game never reads. It would have kept passing while measuring numbers that
///                 do not ship. It now walks every zone, builds it, and measures the TEMPLATES —
///                 100+ of them, with a floor assert so it cannot quietly go blank. Sabotage
///                 (mdef = the full level, 4x): red at baby_pinkling. Worst real deviation is
///                 -23.75pp at dullahan, i.e. magic mitigated LESS than steel, which the one-sided
///                 bound allows on purpose — that is the squishy-to-magic archetype, not a bug.
///                 GOLDENS: none. Only the two `def_mult_bp` bosses move and neither is in a
///                 golden scenario.
/// 0.0.279-alpha — REVERTED IN THE SAME SESSION. It made a monster carry its OWN level, so that
///                 `MonsterDef.level` beat the zone's. That is not what decoupling meant and it
///                 broke the world: three bands share one spawn pool precisely so they can be the
///                 same creature at three difficulties, and pinning the level to the def collapsed
///                 each group into three copies of one fight, separated only by boss thresholds.
///                 The 7-day sim showed it as kills +12% and wipes -7% — the upper band of every
///                 group had gone soft — and it took the user pointing at bands 1-3 to see it.
///                 Left behind deliberately: `the_same_monster_is_a_different_strength_in_a_
///                 different_band`, which fails on exactly this mistake, and the finding that
///                 drove it — between 0.0.273 and 0.0.279 a monster's SOFT defences read
///                 `MonsterDef.level` while its HP, ATK and HARD defences read the zone's, and on
///                 64 of the 66 placed monsters those two disagreed by up to 14 levels. One level
///                 per encounter now, so they cannot.
/// 0.0.280-alpha — ONE BAND PER LEVEL. 60 bands become 100, band N is level N, and the 40 levels
///                 that no zone had ever stood on (2, 4, 7, 9, 12, ...) are inhabited. The zone
///                 names the level; the monster turns it into stats through its own multipliers.
///                 NO NEW MONSTERS AND NO CODE CHANGE. `level_base` already held a row for every
///                 level 1-100, so a band at level 2 is the same creatures at `level_base[2]`. The
///                 20 spawn pools each covered a 5-level stretch before and still do — what
///                 changes is that the stretch is 5 bands instead of 3. Every zone id is built
///                 with `format!("zone_b{band}_1")` and `retreat_return_band` is a `u8`, so 100
///                 fits everywhere; `sim_driver.gd:max_band()` reads the count out of the catalog
///                 rather than hard-coding it, so the band picker grew on its own.
///                 180 zones -> 300. `boss_threshold` is scaled by 3/5 in the same pass, because 5
///                 bands standing where 3 did would otherwise make the world 67% longer: total
///                 kills to clear everything is 4077 -> 4075.
///                 7-day sim: the end level is unchanged (mixed 56, physical 56 -> 55) but a party
///                 gets much DEEPER into the world for it (band 27 -> 55, band 22 -> 40), which is
///                 the finer ladder doing its job. Wipes rise ~46% (1132 -> 1649).
///                 THOSE WIPES ARE NOT NEW, and the measurement says so: they cluster at the first
///                 band of a pool group, and the size of the step at every pool change is
///                 IDENTICAL in the 60-band and 100-band worlds. The curve is smooth from level 56
///                 up (+6% a group) and lurches below it: -25% at 36, +62% at 41, +44% at 46, -30%
///                 at 51. `werewolf` (hp_mult_bp 15845) and `witch` (16156) are 2.5x their gothic
///                 neighbours and werewolf appears at 31, vanishes at 36 and returns at 41 — a
///                 placement problem, not a tuning one. Left alone on the user's call (2026-08-08):
///                 Exemplar/Regalia power is expected to absorb it. Recorded in TODO.
/// 0.0.281-alpha — MARIONETTE ENTERS THE WORLD (bands 41-50). It had been in the catalog since
///                 the Duskmourn kit as `hp 1 / atk 1`, no multipliers, no drop table, placed
///                 nowhere — a card source and a skill with no creature under them.
///                 hp x0.8 / atk x1.1 of its level's base (user), xp and job_xp at 7100 to sit
///                 between maid_horn 7238 and banshee 6938. Weight 20, about 9% of spawns. Full
///                 s3 drop table copied from maid_horn with its own junk — a NEW item,
///                 `junk_tangled_string`, named and loremastered in both EN and TH.
///                 It needs no `level`: the zone names the encounter's level, so one marionette
///                 def covers all ten bands at ten different strengths (335 -> 407 on its lunge
///                 across 41-50). That is the 0.0.280 model paying for itself the first time
///                 content used it.
///                 `puppet_lunge` arms itself: `spawn.rs:156` reads `spawn_use_chance_bp`, and 0
///                 means "a signature always carries it", so every marionette teleports to the
///                 LOWEST-HP party member from five cells and hits for 250% Shadow every 6 s.
///                 MEASURED, AND THE FIRST TWO READINGS WERE WRONG. A single 7-day run said the
///                 party fell from Lv56 to Lv44, and I reported that as a wall. Three seeds later:
///                 57 / 57 at the same settings, and atk x0.9 ALSO produced a Lv49 on a different
///                 seed. So damage is not the variable; the median is Lv57 at every value tested,
///                 which is what the world does with no marionette in it at all. The x1.1 here is
///                 caution on the user's call, not a fix — the data does not object to x1.2.
///                 What the bad runs share is sim-cli's auto-retreat (run.rs:94): five wipes on a
///                 band sends the party back one band for twenty clears before it may return, and
///                 a run that bounces twice loses days to the loop. That model is deliberate and
///                 correct for a balance run — it is how an optimal player paces past a wall — so
///                 those outcomes are signal about VARIANCE in 41-50, not noise to discard.
///                 GOLDENS: none. No golden scenario reaches band 41, and the new ItemDef only
///                 appears in marionette's own table — verified by a control run with the item and
///                 the def present but the placements removed, which reproduced the no-marionette
///                 numbers to the kill.
/// 0.0.295-alpha — BAND GATE. `SelectBand` is `InputAuthority::Client` and its ONLY check was
///                 `band >= 1`, so a level-1 party could type 20 and stand in the endgame.
///                 Replay-verify could never catch it: the sim accepted the input, so the server
///                 replayed it and agreed. The question an input-authority hole asks is not "did
///                 the client lie about the result" but "was it allowed to author this at all",
///                 and the answer has to be given in the sim or nowhere.
///                 RULE (user): any band up to REACHED + 1. Back is always allowed; skipping is
///                 not. The +1 keeps the natural `next_zone_id` advance and a manual re-entry on
///                 the same code path — two paths that must agree about progression is how they
///                 come to disagree.
///                 New `GameState.band_reached: u32`, raised in `change_zone` because every
///                 transition funnels through there (advance, retreat, farm-loop, SelectBand), so
///                 no second site has to remember. Monotonic: retreating to band 1 does not give
///                 back what was earned.
///                 NO MIGRATION, deliberately. The gate takes the CURRENT ZONE as a second source
///                 of truth — a party standing in band 40 has self-evidently reached 40 — so a
///                 save written before the field existed defaults to 0 and is still not locked out
///                 of where it is parked. `a_save_from_before_the_gate_keeps_the_band_it_is_standing_in`
///                 pins that, and sabotaging the current-zone half turns it red.
///                 GOLDENS: TWO MOVED — `golden_alpha6::alpha6_grassland_run` and
///                 `diag_band4::diag_band4_hash`. Both start in a band, so `band_reached` is now
///                 non-zero and serializes (skip-if-0 only protects a save that never entered a
///                 band). Measured, not assumed: with the `change_zone` line disabled all 39
///                 goldens go green again, so the drift is exactly this field entering the state
///                 and nothing behavioural. Re-pinned.
/// 0.0.294-alpha — A WINDUP OUTLIVED THE SKILL LIST IT INDEXES. Remote abort, and a live crash.
///                 `Casting` stores a SLOT INDEX into `skills`, and `clear_exemplar_skill`
///                 (`grid/entity.rs`) shrank that list with `retain` while never touching
///                 `casting`. Unequip an Exemplar mid-channel and the next tick indexes past the
///                 end: `fire_pending_cast` panics. Reproduced — three granted skills at 0-2, a
///                 class skill learned after them at 3, one `UnequipExemplar`:
///                 "index out of bounds: the len is 1 but the index is 3".
///                 It is worse than a client crash. verify-server links this crate and REPLAYS
///                 uploaded saves, and release sets `panic = "abort"`, so the index takes the
///                 whole server process down — a remote DoS authored with one ordinary
///                 Client-authority event. The quiet variant is worse still: a slot that stays IN
///                 range after the shift now names a DIFFERENT skill, which fires silently.
///                 Fixed at both ends, deliberately. ROOT: `clear_exemplar_skill` drops the cast,
///                 matching every other path that invalidates one — death (`tick/mod.rs`),
///                 warp/revive (`build/zone.rs`), being lifted (`handlers.rs`), and RO itself.
///                 USE: `fire_pending_cast` + `resolve_buff_cast` + `place_ground_field` read the
///                 slot with `.get()` and fizzle. The root fix alone is not enough — `Casting` is
///                 serialized, so a save written before today already carries the stale index and
///                 never passes through `clear_exemplar_skill` on load.
///                 Found by auditing 558 index sites across the 9 files that touch client-supplied
///                 indices; this was the only one reachable. Two tests, each sabotaged against its
///                 own fix: removing the root fix reddens one, removing `.get()` reddens the other.
///                 GOLDENS: unmoved, all 38. No scenario unequips an Exemplar mid-cast, so the bump
///                 is for replay agreement — client and server must not disagree about a save that
///                 contains this sequence.
/// 0.0.293-alpha — `soul_nodes_allocated`: Paths of Soul spends, for a daily commission.
///                 One site (`allocate_soul_path`), counting the ACT of spending. Rises once per
///                 node and never falls — a respec, if one is ever added, must not walk it back,
///                 because a daily reads its DAY DELTA and a fall would read as negative
///                 progress. Same reason maxima live in `Records` and are not addressable as
///                 daily counters at all.
///                 GOLDENS: unmoved. No scenario allocates a node, so the field stays
///                 skip-if-zero; the bump is for the serialized shape.
/// 0.0.292-alpha — `items_sold`: what the vendor took.
///                 `Measures.items_sold`, counted at both NPC sell paths — `sell_all_junk` and
///                 `sell_inventory_item`. Per ITEM STACK, not per transaction: "sell all junk"
///                 clears a bagful in one press, and a player who did that has sold a bagful, not
///                 sold once.
///                 Added for a daily objective ("ขายของให้ NPC", user 2026-08-12) that had no
///                 counter to read. `GilEarn::NpcSale` already names the same event for the Gil
///                 ledger, but that records VALUE and a quest wants COUNT — and bulk selling
///                 makes one call for many items, so the ledger's call site cannot supply it.
///                 GOLDENS: unmoved. No scenario sells to a vendor, so the field stays
///                 skip-if-zero and every hash is byte-identical. The VERSION bump is for the
///                 serialized SHAPE, which changed regardless.
/// 0.0.291-alpha — THE BAND-CLEAR BACK-LOG IS GONE (user 2026-08-12: the system is retired).
///                 `GameState.band_clears` removed. It was WRITE-ONLY in production — written by
///                 `record_band_clear`, read by nothing but tests — and the client wrapper that
///                 fired its reset, `sim_driver.reset_band_clears()`, had no callers either. A
///                 whole chain carrying weight in every save and answering no question.
///                 `record_band_clear` keeps only the `bands_cleared` tally added in 0.0.290,
///                 which is the number anyone actually wanted from it.
///                 `InputEvent::ResetBandClears` STAYS, as a permanent no-op. Not caution about
///                 deleting code: a save is `(seed + state + Vec<InputEvent>)`, so the variant is
///                 part of the REPLAY WIRE FORMAT. Remove it and any log still holding one fails
///                 to deserialize — losing the whole save rather than skipping one dead
///                 instruction. Nothing ever fired it, so no log should contain one, but "should"
///                 is not something a replay format may lean on. Cost: one unreachable match arm.
///                 Its test now pins BOTH halves — that it still parses, and that it moves
///                 nothing. The second half matters because re-pointing a reset event at
///                 `bands_cleared` is the obvious thing to do once its old target is gone, and
///                 that would let a claim wipe the player's lifetime history.
///                 Removing a serialized field changes the hash of any state that carried it, so
///                 there is a new suite test proving a save that still holds the retired key
///                 loads: nothing here uses `deny_unknown_fields`, and now something checks that.
///                 GOLDENS: `diag_band4` re-blessed — the only scenario whose map was ever
///                 non-empty.
/// 0.0.290-alpha — BANDS CLEARED, WHICH `band_clears` COULD NEVER HAVE TOLD YOU.
///                 `Measures.bands_cleared` — a band is three rooms (X-1/X-2/X-3), so it is
///                 `rooms_cleared`'s coarser sibling at roughly a third of it. Booked in
///                 `record_band_clear`, the one site, already latched by `band_clear_counted` so
///                 a per-tick check cannot tally the same clear twice.
///                 It is NOT derivable from `band_clears`, which looks like exactly this number
///                 and is not: that map is a claim BACK-LOG, and `InputEvent::ResetBandClears`
///                 empties it every time the player banks the rewards. Precisely the trap
///                 `consecutive_wipes` (a streak, zeroed by a retreat) versus `wipes` (the
///                 history) already set — a counter something else resets cannot also be the
///                 lifetime figure.
///                 Tested as a PAIR in one test, since the two are incremented by the same call
///                 and the only thing separating them is that a reset touches one: after
///                 `ResetBandClears` the back-log is empty and the tally still reads 3. Testing
///                 either alone would look fine the day someone merges them. Sabotage — making
///                 the reset wipe the tally too — fails it.
///                 GOLDENS: `diag_band4` re-blessed — it is the one scenario that runs a band
///                 far enough to clear its third room. The other 36 never reach one.
/// 0.0.289-alpha — HEALS ARE AN EVENT NOW, NOT A GUESS FROM THE HP BAR.
///                 New `HealEvent` + `GameState.heal_fx` (display-only, `serde(skip)`, cleared
///                 each tick like `recent_hits`), emitted at all SEVEN sites that restore HP:
///                 `apply_skill_heal`, Benediction, Mending Burst, passive regen, HoT, potions,
///                 and Crimsonne lifesteal. `Measures.healing_done` then falls out of the same
///                 per-tick scan as the rest.
///                 THE CLIENT WAS INFERRING HEALS from `hp_now - hp_was` (`arena.gd`), and that
///                 inference had a real defect, not just a missing attribution: the heal pop was
///                 the `else` of the damage branch, so a tick that healed 500 and took 500 showed
///                 NOTHING — net HP never moved, the damage branch was skipped, and the jump was
///                 zero. It also could not name the healer and merged two heals into one number.
///                 The pop now reads `heal_fx` and sits outside the damage if/else.
///                 `amount` is the EFFECTIVE restore, after the max-HP clamp. A heal rolled onto
///                 a full bar produces no event at all, because a green "+500" over a bar that
///                 did not move is the same class of lie as filing an overkill crit at the
///                 victim's leftover HP. `HealKind` (Skill/Regen/Hot/Potion/Lifesteal) rides
///                 along so the renderer can tell a cast sparkle from a potion glow.
///                 NOTED, NOT FIXED: `apply_benediction_heal` restores HP directly instead of
///                 routing through `apply_skill_heal`, whose own doc says "EVERY skill-heal
///                 resolve path must route through here so the trigger can't be missed by a new
///                 heal source". The trigger in question is the Ashelle card's on-heal ASPD
///                 stack, which therefore does not fire for Benediction. Routing it would change
///                 combat, so it is a balance decision and not part of an event-plumbing change.
///                 GOLDENS: `beta9` alone — the one scenario that heals. `heal_fx` is `serde(skip)`
///                 so it costs no hash; only `healing_done` moves anything.
/// 0.0.288-alpha — DAMAGE TAKEN, DEATHS, CASTS — ALL OFF THE SAME PASS.
///                 `Measures` gains `damage_taken`, `deaths`, `skills_cast`. No new funnel: the
///                 per-tick scan added in 0.0.287 already walks `recent_hits` / `field_hits`, and
///                 those carry a TARGET as well as an attacker, so incoming damage was one line.
///                 `record_damage` could never have supplied it — it is the DPS meter and only
///                 ever sees player attackers.
///                 `skills_cast` reads `skill_fx`, the list that tells the renderer which cast VFX
///                 to play, so a skill that plays an effect and a skill that counts are the same
///                 set by construction. Basic attacks are not skills and are not in it.
///                 `deaths` is a TRANSITION, so it needs something that saw both sides: the tick
///                 collects the standing party at its top as a tick-LOCAL Vec (not serialized
///                 state) and diffs at the end. Counting "is dead" instead would tick up on every
///                 frame of a corpse. Party entities are never pruned, so a missing id is a death
///                 and not a despawn.
///                 A SABOTAGE CAUGHT A REAL BUG HERE. The first rule made attacker-side and
///                 target-side EXCLUSIVE (`else if`), which drops friendly-splash damage — HP the
///                 party genuinely lost. It survived its own sabotage because the test only aimed
///                 outgoing hits at monsters, where the target check excludes them anyway, so the
///                 branch under test was never entered. The sides are independent now: they feed
///                 different counters and there was never a double-book to prevent. The test
///                 fires a party->party hit, and both the missing-booking and the old exclusive
///                 rule now fail it.
///                 GOLDENS: re-blessed where the party takes damage, dies, or casts.
/// 0.0.287-alpha — DAMAGE, WIPES, AND THE BEST HIT YOU EVER LANDED.
///                 `Measures` gains `damage_dealt` and `wipes`; a NEW `GameState.records`
///                 (`Records`) gains `biggest_hit`.
///                 `damage_dealt` rides `record_damage`, the call that already feeds the DPS
///                 meter from fourteen sites — so its scope is exactly the meter's: player-team
///                 attackers, post-mitigation, overkill clamped. The difference between the two
///                 is that `reset_dps` clears the meter and never touches this.
///                 `wipes` sits beside `consecutive_wipes`, which is a STREAK and gets zeroed by
///                 a retreat; the stat wants the history.
///                 RECORDS IS A SEPARATE TYPE, and the reason is mechanical. The online layer
///                 awards BP XP and daily progress by DIFFING two `Measures` snapshots; diffing a
///                 maximum yields a number that means nothing, and a `CounterId` aimed at one
///                 would quietly pay out for it. Splitting the type makes registering a best as a
///                 countable impossible rather than discouraged.
///                 `biggest_hit` is read off `recent_hits` / `field_hits` once per tick, NOT off
///                 `record_damage` — which is deliberately given `dealt.min(pre_hp)`. That clamp
///                 is right for DPS and wrong for a best, because the hardest hit anyone lands is
///                 precisely the one that overkills: a 500k crit on a 12k monster would be filed
///                 as 12k while the player watched 500000 leave the screen. `HitEvent.amount` is
///                 the number the renderer draws, so stat and popup can never disagree. Scanning
///                 the two vectors also means no call site can be missed — every hit the player
///                 sees is in one of them by construction. `HitEvent.hits` is `display_hits` (one
///                 instance drawn as N numbers), so `amount` is the whole instance, which is what
///                 a biggest-hit figure means. Party attackers only; both vectors carry monster
///                 swings and those are not anyone's personal best.
///                 Sabotaged four ways — clamping the best, dropping the party filter, unhooking
///                 the scan from the tick, and halving the damage tally — each caught.
///                 GOLDENS: re-blessed where damage lands; `Records` is skip-if-empty so a
///                 scenario that never hits stays byte-identical.
/// 0.0.286-alpha — THE GIL LEDGER BALANCES, AND THE COUNTERS SAY WHY.
///                 `Measures` gains nine tallies: `boss_kills`, `levels_gained`,
///                 `refine_successes`, and the six-part Gil ledger `gil_earned` / `gil_spent` /
///                 `gil_received` / `gil_sent` / `gil_held` / `gil_released`. They exist to feed
///                 a player STAT screen and the daily-quest deltas (the server already diffs
///                 `Measures` snapshots for BP accrual, so one set of counters serves both).
///                 The Gil six are more than stats. With every movement booked, the balance is
///                 checkable rather than merely present:
///                     gil + (gil_held - gil_released) == gil_earned + gil_received
///                                                        - gil_spent - gil_sent
///                 `GameState::gil_ledger_balances`. True by construction TODAY, which is the
///                 reason to assert it — it stops being true the first time a new path moves Gil
///                 without booking it, and that is the leak this is here to catch. Scope is the
///                 SIM's view; Gil the verify-server grants outside a replay is not in it.
///                 `gil_earned` counts the GilPct bonus flush as well as the base, because the
///                 equation is about what LANDED in the balance, not what was nominally awarded.
///                 ESCROW IS NOT SPENDING. A listing hold and a buy order's `max_price` come back
///                 — on cancel, or as CHANGE when the order fills cheap — so they book to
///                 `gil_held`/`gil_released`, never `gil_spent`. Conflating them would show a
///                 player who listed and cancelled a hundred times as having spent a fortune they
///                 still hold. Both stay MONOTONIC (escrow returning raises `gil_released`; it
///                 never lowers `gil_held`), because the snapshot diff underflows on a field that
///                 can fall.
///                 `set_gil_for_test` books its own jump, so the balance check is meaningful in
///                 tests rather than trivially defeated by them.
///                 TESTED over a REAL 3000-tick run, not a hand-built state, and the run asserts
///                 `gil_earned > 0` so it cannot pass on a scenario where no money moved.
///                 Sabotaged three ways — dropping the earn tally, dropping the spend tally, and
///                 booking escrow as spending — each caught by a different assertion.
///                 GOLDENS: `alpha5`, `alpha6`, `beta5`, `diag_band4` re-blessed. Exactly the four
///                 scenarios that move Gil, gain a level, or refine; the other 33 never touch the
///                 new fields, which stay skip-if-zero and byte-identical.
/// 0.0.285-alpha — THE BATTLEFIELD IS 20x10, THE PARTY OPENS ON THE LEFT.
///                 Board 24x12 -> 20x10 on all 306 zones (catalog 0.0.71 -> 0.0.72), the party
///                 spawns at (2,5) instead of centre, and for the first 10 SECONDS of a zone
///                 monsters may only enter from the RIGHT half (`spawn::OPENING_WINDOW_TICKS`).
///                 The fight now opens as an approach that meets in the middle; uniform placement
///                 put roughly a third of the first wave BEHIND the party.
///                 The window covers the initial wave as well, because
///                 `spawn_initial_population` goes through the same `pick_random_cell` — one rule
///                 rather than two that must agree. Only x's RANGE narrows; the draw COUNT per
///                 retry is unchanged.
///                 New `ZoneSpawnState.opened_at_tick`, serialized: a save taken inside the window
///                 must resume inside it or the next monster appears somewhere else and the replay
///                 diverges.
///                 MEASURED, 6 seeds x 6000 ticks, mean kills: 211.2 (24x12) -> 215.7 (20x10) ->
///                 217.5 (+ left spawn / right opening). One wipe in six seeds in all three, and
///                 every seed still reaches level 5 — the shape changed more than the throughput.
///                 GOLDENS: `alpha6` + `diag_band4` re-blessed. They are the only two scenarios
///                 that populate a real catalog zone; the rest hand-build their entities and never
///                 touch the spawn picker.
/// 0.0.284-alpha — THE SR GACHA TIER PAYS SOMETHING.
///                 `sr_pool` was empty on both banners, so `payout_def` fell through to `r_item`
///                 and an SR paid the same shard an R paid — the guaranteed SR every ten pulls
///                 delivered a purple frame, which is what `SR_GUARANTEE_WINDOW`'s own comment
///                 warned it would. New `regalia_sr_fodder` (catalog 0.0.70 -> 0.0.71), placeholder
///                 content, worth TWO Enhance levels against the cap of five.
///                 THE SIM CHANGE: the fodder rule now answers HOW MANY LEVELS rather than
///                 yes/no. `items::regalia_fodder_levels` is the single source,
///                 `is_regalia_fodder` delegates, and `enhance_regalia` adds what it returns,
///                 clamped — `saturating_add(2)` from level 4 would otherwise store a 6 that only
///                 shows up as an overlay reading a level its table has no row for.
///                 VERSION BUMP because a replay of the same log now produces a different
///                 `enhance_level`. `policy_check`'s version equality is the only thing that makes
///                 client and server move together; without it an un-updated client's save
///                 diverges and reads as a cheating player.
///                 GOLDENS: none move — no scenario enhances a Regalia.
///                 Landed with a pattern-matching cleanup that made adding a tier touch ONE table:
///                 `starts_with`/`ends_with` on ids is now named (`is_unique_def_id`,
///                 `regalia_fodder_levels`) and ratcheted by `no_new_id_matching_by_string_shape`.
/// 0.0.283-alpha — A BESTIARY CLAIM THAT FAILS NO LONGER LEAVES A KEY BEHIND.
///                 `ClaimBestiaryReward` did `bestiary_claimed.entry(group).or_insert(0)` BEFORE
///                 checking the kill threshold, so a claim that was refused still wrote its key.
///                 `group` is an arbitrary client string and `MAX_LOG_LEN` is 50,000, so one save
///                 could plant 50,000 junk keys and `bestiary_claimed` accumulates across saves
///                 with no bound — not a mint, but a map the client grows at will, carried in
///                 every future bundle and hashed into every future `state_hash`.
///                 Now READ first, insert only on the success path. That makes the kills check the
///                 allowlist: `monster_kills` only holds groups the sim recorded on a real kill, so
///                 an invented string reads 0 and never reaches the insert. Found while auditing
///                 whether the bestiary shared /story/claim's unbounded-key hole (it did not — the
///                 payout was always gated on sim-produced state, which is the whole difference).
///                 Also `the_milestone_table_still_fits_the_claimed_bitmask`: `bestiary_claimed` is
///                 a `u8`, one bit per milestone, and the handler shifts by an index bounds-checked
///                 against the TABLE. Four entries today; the 9th makes `1u8 << 8` panic in debug
///                 and wrap in release, so the two builds would disagree about a save — and
///                 `apply_input` may never panic at all.
///                 VERSION BUMP IS THE POINT: the serialized SHAPE is unchanged, but what a replay
///                 PRODUCES is not, and `policy_check`'s version equality is the only thing that
///                 forces client and server to move together. Without it an un-updated client's
///                 save diverges and reads as a cheating player instead of a stale build.
///                 GOLDENS: none move — no scenario claims a bestiary reward, so nothing reaches
///                 the changed line. Existing saves are unaffected: `bundle.start` is a trusted
///                 ANCHOR, not genesis, so junk already baked in stays and is never re-derived.
/// 0.0.282-alpha — THREE THINGS THE CATALOG REFACTOR LEFT BEHIND.
///                 (1) `game_data_version` 0.0.68 -> 0.0.69, and a test that will not let it go
///                 stale again. `verify-server` gates every upload on that string (main.rs:3210)
///                 and bakes its own copy of the catalog into the image at build (Dockerfile:43).
///                 When the two DIFFER a player gets a clean 409. The dangerous case is content
///                 that differs while the string does not: the gate waves it through, the replay
///                 diverges on a monster's HP, and the mismatch reads as a cheating player rather
///                 than a deploy nobody did. The string had sat at 0.0.68 across six catalog
///                 changes. `the_catalog_cannot_change_without_its_version_changing` now hashes the
///                 content (version field excluded) and pins it next to the version, so editing one
///                 without the other is red either way. `tools/check_online_sync.sh` checked only
///                 `sim_version` and now checks all three surfaces — sim, the server's catalog, and
///                 the CLIENT's copy under `godot/data/`, which no server-side check can see.
///                 (2) BESTIARY SHOWS BANDS, NOT NUMBERS. `bridge_stats.rs` was printing the def's
///                 raw `hp`/`atk`, which 0.0.274 turned into leftovers — 12 of the 66 placed
///                 monsters displayed "HP 1 ATK 1". There is no single true number to print any
///                 more: a banshee is 1535 HP in one band and 2043 in another. What does not move
///                 is the MULTIPLIER, so `data::bestiary_bands` classifies it — in sim-core,
///                 integer-only, with the client owning only the words. Five bands on the user's
///                 0.5/0.8/1.2/1.5 thresholds, applied to the ratio against the monster's OWN KIND:
///                 measured on one shared scale, 29 of 45 ordinary monsters read "low" and all 22
///                 bosses read "very high", because a multiplier is a share of its band's geometric
///                 mean and that mean includes the boss. The entry also reports the LEVEL RANGE the
///                 creature is met across, which is the question the old `Lv` line was pretending
///                 to answer.
///                 (3) UMBRAL_ROOTS 25% -> 80% ATK a tick. The field is ten separate packets and
///                 the magic path subtracts soft MDEF from each, so the flat term is charged ten
///                 times: at 25% a tick was 15-21 against a level-21-25 caster's 30-42 and every
///                 one floored to 1, for 10 damage over five seconds. SCOPE, because the first
///                 draft of this overstated it — this is DoT versus INT, not a broken magic system.
///                 A physical class at that band carries 10-19 soft MDEF and was still taking 5-11
///                 a tick, and single-packet monster magic lands 80-92% on the three classes that
///                 never raise INT. Magic mitigation is doing what RO intends.
///                 The test that let this through asserted `player.hp < 100_000` against a STAT_MIN
///                 dummy — a boolean that `.max(1)` keeps true forever. Its replacement measures,
///                 against a caster and a victim built at the band this monster stands in, and
///                 fails if the field lands under a third of what it swings for (25% lands 6%,
///                 80% lands 44%). Two earlier versions of that test passed while measuring the
///                 wrong thing: one stood the pair two cells apart and counted the caster's ORDINARY
///                 attacks, the other hand-picked INT 40 where a real level-23 mage has 23.
///                 GOLDENS: none. No scenario reaches the levels demon_tree stands in, and the
///                 bestiary and version work is display and tooling.
/// 0.0.296-alpha — PARTY PROVISION RACK. `GameState` gains `party_provisions` — up to three
///                 consumables that buff EVERY living member, layered ON TOP of each member's own
///                 slots rather than replacing them (docs/party-buff-slots-design.md).
///                 `ProvisionKind::Party` marks membership (not a position — the three slots are
///                 interchangeable), `AddPartyProvision`/`RemovePartyProvision` move items, and
///                 `party_provision_upkeep` fires the rack after the per-character pass.
///                 ONE BUFF SOURCE PER SLOT, `PARTY_PROVISION_SOURCE + i`. Three different items
///                 each granting +ATK is a legal loadout — the rule bans a repeated ITEM, not a
///                 repeated buff kind — and `apply_buff` replaces on `(kind, source_id)`, so a
///                 shared id would collapse that loadout to one buff: three items paid for, one
///                 applied, nothing said.
///                 The same collision already existed in the PER-CHARACTER layer, where all five
///                 slots shared `CONSUMABLE_BUFF_SOURCE`, so a Meal and a Tonic both granting
///                 +ATK applied as one buff and the loser's stack then never drained again —
///                 `BuffMissing` saw the winner's buff and never re-fired. Both are now per-slot,
///                 and `BuffMissing` asks whether ITS OWN buff has lapsed rather than whether the
///                 kind is absent. Without that second change the rack would have switched every
///                 member's personal food off, which is the "cut the craft sink by 80%" outcome
///                 `provisions_view.gd` warns about, arriving through a door nobody was watching.
///                 REMOVING AN ITEM DOES NOT CANCEL ITS BUFF (user): a provision applies for a
///                 duration and that duration is paid for — pull the item fifteen minutes into a
///                 thirty-minute buff and the buff still ends at thirty. Which is why a rack entry
///                 carries a STABLE `key` instead of being identified by its position: removal
///                 shifts every later entry down, and a positional source would slide the
///                 neighbour onto the departed item's still-running buff, so the neighbour would
///                 see "already buffed" and never refresh again — an item sitting in the rack with
///                 charges left, silently doing nothing. Its test only catches that once the
///                 SURVIVOR's own buff lapses while the orphan runs on; the first version asserted
///                 buff COUNTS after removal and passed on the broken code.
///                 Tests use ONE buff kind repeated, deliberately: three DIFFERENT kinds pass on
///                 the broken code, which is why nothing caught the existing collision. Both were
///                 falsified by removing the mechanism and confirming they name the failure.
///                 GOLDENS: none. Skip-if-empty keeps old saves byte-identical, and no scenario
///                 stocks two consumables of one kind.
/// 0.0.297-alpha — PROVISION AUDIT: three defects the party-rack work introduced or exposed.
///                 (1) PER-CHARACTER slot buff sources were derived from the Vec POSITION, and
///                 both `consumables` and `provisions` SHRINK — a depleted stack is swept every
///                 tick and the player removes slots by hand. The survivor slid onto the departed
///                 slot's id, its `BuffMissing` found nothing there, and it re-fired: a charge
///                 burned and TWO copies of one buff summing in `buff_sum`. Reachable with no
///                 player input at all. `ConsumableSlot` now carries the same stable `key` the
///                 rack already had; a legacy slot reads 0 and is assigned on the next upkeep, so
///                 the repair needs no migration step.
///                 (2) A rack `key` was handed out again while its buff was still running — and
///                 un-racking leaves the buff live BY DESIGN, so the key was never free. The new
///                 item's `BuffMissing` saw the orphan on its own source and never fired, never
///                 drained: an item in the rack, charges left, doing nothing. Keys are now free
///                 only when absent from the rack AND not stamped on anyone.
///                 (3) `remove_party_provision` DESTROYED the item when the bag was full —
///                 `try_grant_item` banks nothing and returns false, and the return was discarded
///                 after the entry had already been removed. It now grants first and unracks only
///                 on success.
///                 All three found by an audit of the provision system, all three falsified by
///                 restoring the defect and watching the new test name it.
///                 GOLDENS: none.
/// 0.0.298-alpha — PROVISION AUDIT, the rest. Seven more, none of them new code:
///                 • A PROVISION COULD SIT IN THE EMERGENCY BAR. The reserved slots exist so one
///                   Meal and one Tonic are the limit; `provisions_view.gd` filtered its own
///                   picker and said so in a comment, while `character_view_bag.gd` routed any
///                   Consumable straight to `add_consumable`. Enforced by one of the two screens
///                   that reach it, which is the same as not enforced — four meals summed to +26
///                   to every stat. The sim rejects it now.
///                 • `add_consumable`/`add_provision` accepted ANY entity id. Naming a monster or
///                   a pet debited the bag and pushed the item onto something the upkeep skips and
///                   the pruner deletes. Not reachable through the UI, which is not the bar.
///                 • A `ProvisionKind::Party` item could take a per-character reserved slot, where
///                   `provisions_view.gd` (Meal and Tonic columns only) could never show it again.
///                 • POISON RESIST NEVER WORKED. `apply_or_refresh_poison` pushed the buff
///                   directly, and the status-resist duration cut lives inside `apply_buff` and
///                   nowhere else — so a Field Ration's "Poison wears off 30% faster" was false
///                   against both real sources (Poison Cloud, toxic_burst). Refresh-to-longest is
///                   preserved by hand, since `apply_buff` replaces outright.
///                 • THREE RAW `inventory.push`es on the unslot paths skipped both the stack merge
///                   and the cap: unslotting after looting more left TWO stacks of one consumable,
///                   and a full bag went to 61/60, where the next drop falls through to
///                   `overflow_recover` and is auto-sold or lost. All three bank now, and only
///                   free the slot once banking succeeds. The swap-out branch also restores the
///                   already-debited portion when it bails, or the fix would have destroyed it.
///                 • `GOD_BUFF_SOURCE` WAS `u32::MAX`, the same value as `AURA_SOURCE`, and
///                   `apply_party_auras` strips that id every tick before anyone swings — god mode
///                   delivered invincibility and no cooldowns but never its damage buff. Debug
///                   only; no golden ever saw it.
///                 • A charge could be spent on an effect that does nothing (a `BuffMissing`
///                   trigger paired with a non-buff effect drains forever, since the trigger stays
///                   true). Nothing applied, nothing spent.
///                 Also `bridge_stats::buffs_json` aggregated with `max` where the sim reads every
///                 kind with `buff_sum`, so the readout disagreed with the damage being dealt.
///                 GOLDENS: none — no scenario slots a provision, poisons a resist-carrier, or
///                 unslots into a full bag.
/// 0.0.299-alpha — PROVISION RE-AUDIT. The second pass checked the FIRST pass's fixes and found
///                 five of them incomplete or wrong, plus four it had missed:
///                 • The per-character key allocator never got the live-buff check the RACK got in
///                   the same commit. Removing a slot leaves its buff running, so the key was free
///                   while the buff was not — SWAPPING a Tonic or Meal handed the replacement the
///                   departed item's source, where its `BuffMissing` saw "already buffed": the
///                   upgrade never fired and never drained, for 30 minutes or, with a meal, two
///                   hours. Reachable with two ordinary clicks.
///                 • The new key was 1-BASED while the old source was 0-based, so every save with
///                   a live provision would have had its buff orphaned once on update — re-fire,
///                   charge burned, two copies summing: the exact defect the fix was for. `key - 1`
///                   makes the transition a no-op.
///                 • `try_grant_item` could return TRUE after destroying the item. It predicted a
///                   merge from `def_id` alone while the real rule also requires `bound` to match,
///                   so the cap check was skipped, the merge failed, and the item fell to
///                   `overflow_recover`. Live on the battle-pass path, which mints BOUND
///                   stackables: claim at cap beside an unbound stack of that def and the reward
///                   was destroyed while the server consumed the mail. Now one `can_bank` that
///                   mirrors the real rule, asked before acting.
///                 • `add_provision`'s "restoring always succeeds" was not true over cap. It asks
///                   `can_bank` BEFORE debiting now, so there is nothing to restore.
///                 • `buffs_json` summed every kind, but the sim sends non-DoT DEBUFFS through
///                   `StackPolicy::Strongest` — two 30% slows are 30%, and the readout said 60%.
///                   It branches on the kind's own policy now; `max` had been right for those.
///                 • `character_view_bag.gd` still routed provisions to `add_consumable`, which the
///                   sim now refuses — a button that did nothing and said nothing.
///                 • `load_ammo` never got the slot-holder guard its neighbours did.
///                 • `migrate_save_items` walked `consumables` but not `provisions`, so a slotted
///                   meal froze at whatever spec was serialized while the same stack in the bag was
///                   corrected on load.
///                 • The per-character "nothing spent" guard was narrower than the rack's: a
///                   `BuffMissing` trigger paired with a non-buff effect drains forever, because
///                   the trigger never stops being true.
///                 GOLDENS: none.
/// 0.0.300-alpha — 0.0.299 SHIPPED A REMOTE PROCESS-ABORT. The live-buff clause I added to the
///                 per-character key allocator removed the loop's only bound: orphaned buffs are
///                 unbounded (removing a slot leaves its buff running BY DESIGN, and `e.buffs`
///                 only shrinks on expiry), so `k` climbed past every one of them. `k` is a `u8`,
///                 `overflow-checks = true` in BOTH profiles, and release is `panic = "abort"` —
///                 so ~255 slot/unslot cycles with a long-duration buff kill the process.
///                 verify-server links this crate and REPLAYS CLIENT-AUTHORED LOGS, so a crafted
///                 log aborts the server. The rack version this was modelled on searches a fixed
///                 range and refuses when it finds nothing; only half of that shape was copied.
///                 Now bounded to the 16-wide band, preferring a key that is free AND carries no
///                 live buff, falling back to merely free — which always exists, since at most
///                 three slots contend for sixteen keys. The worst case is the inheritance the
///                 preference avoids, not a panic. `SOURCE_BAND` also names the bound the old
///                 comment only asserted: a key past 16 would land bar-on-provision.
///                 Second: the "nothing applied, nothing spent" guard `continue`d AFTER the effect
///                 ran, which INVERTED it. `HealHp`'s arm really does mutate HP, so a
///                 `BuffMissing` + `HealHp` pairing healed for free every tick and never depleted
///                 — the cooldown write-back was skipped too. It decides before the effect now.
///                 Its test could not have caught that: it ran at FULL HP, where `HealHp` is a
///                 no-op whichever way the guard falls, so it passed against the broken build for
///                 the wrong reason. It starts wounded now and asserts HP did not move.
///                 GOLDENS: none.
/// 0.0.301-alpha — ULTRACODE AUDIT (7 auditors, adversarial verification). Two sim defects:
///                 • `try_grant` still carried the def_id-only merge predicate that `can_bank`
///                   was written to close in 0.0.299 — I fixed `try_grant_item` and not its
///                   sibling 38 lines away, which serves the SAME caller (`claim_mail` splits
///                   full items from def_id payloads between the two). On a bound mismatch it
///                   skipped the cap check, the real merge refused, the item fell through to
///                   `overflow_recover`, and it returned true while the server consumed the mail.
///                   It also never routed collectibles, so an Exemplar delivered by def_id would
///                   have been pushed into the bag. Both now go through the same gate.
///                 • `add_provision`'s TOP-UP compared `def_id` alone, so adding an UNBOUND stack
///                   onto a BOUND slot relabelled it, and unslotting returned the whole merged
///                   quantity at the slot's flag. `aspd_pot_1` is reachable both ways on shipped
///                   data (battle-pass mints it bound, `brew_speed_potion_1` crafts it unbound).
///                 GOLDENS: none.
/// 0.0.302-alpha — JOB ADVANCEMENT ATE THE CHARACTER'S THINGS. `advance_class` replaces the whole
///                 entity with a fresh build of the target class and hand-copied only stats,
///                 skills and equipped gear — so the slotted potions, the reserved Meal/Tonic
///                 provisions, the CARD PANEL and any loaded elemental ammo were all destroyed, on
///                 the one action the demo is built around, silently. Cards are the costliest of
///                 those and there is no way to get one back. Measured, not inferred: a probe
///                 advanced a real swordsman and read 0 kept in all four containers.
///                 Combat TRANSIENTS (buffs, shields, casting, cooldowns) are still dropped, which
///                 is correct — the class changed. Anything a PLAYER put there now survives.
///                 GOLDENS: none — no scenario advances a job.
/// 0.0.303-alpha — THE PARTY RACK COULD GO DEAD, AND ATE THE UNITS ON THE WAY. Two defects, both
///                 in code this branch added, both found by the audit's adversarial pass:
///                 • `add_party_provision` DEBITED THE BAG BEFORE the key search could decline, so
///                   every refusal destroyed the units it had just taken. That is the same
///                   ordering bug `add_provision` was fixed for two versions earlier; the sibling
///                   twenty lines up kept it.
///                 • The rack's key space is exactly as wide as the rack (3) while un-racking
///                   deliberately leaves the buff running — so with a long-duration provision a
///                   single removal could make EVERY later add fail until that buff lapsed, a
///                   screen that silently refuses everything. It now prefers a key that is free
///                   and quiet but FALLS BACK to merely free: inheriting an orphan costs one slot
///                   one refresh cycle, refusing costs the player the feature. Same
///                   preference-then-fallback shape the per-character allocator was bounded with.
///                 GOLDENS: none.
/// 0.0.304-alpha — THE REST OF THE AUDIT'S SIM FINDINGS.
///                 • BENCHING destroyed the same things job advancement did. `CharacterData` is,
///                   by its own doc, "the ONE place that knows every per-character field" — and it
///                   listed six, missing `consumables`, `provisions` and `ammo`. `bench` captures
///                   and then retains the entity out of the grid, so what capture does not take is
///                   gone. Parked and restored now, which fixes class-switch parking too.
///                 • `migrate_save_items` never walked `state.roster` — the LIVE parking lot;
///                   `class_characters` is the legacy field the 0.0.97 fold empties into it. A
///                   benched character's gear kept pre-patch numbers and `deploy` put it straight
///                   back on the board. Also added: equipped CARD PANELS (combat reads the panel
///                   instance's own `card_mods` and never the catalog, so a socketed card froze
///                   forever while the same card in the album was corrected), `party_provisions`,
///                   and `owned_regalia` — the declared twin of `owned_exemplars`, which was
///                   migrated while it was not.
///                   ⚠️ Panel cards go through `migrate_panel_card`, which preserves `card_slot`:
///                   that field is per-instance ROUTING for an equipped card (`equip_card` stamps
///                   the destination), so refreshing it from the def would silently move the card
///                   between accessory slots — the hazard `equip_slot` is already excluded for.
///                 • `SpawnLure`'s magnitude is a spawn RADIUS in cells and was read with
///                   `buff_sum`, which ADDS: two lures widened the ring instead of keeping the
///                   tighter one, so stacking the item pushed spawns FURTHER away. Takes the
///                   minimum now — a radius is not a bonus.
///                 • The per-character key search is bounded by the container's CAPACITY rather
///                   than the 16-wide source band. Re-adding one item mints a fresh key each time
///                   (the departed slot's buff still holds the old one), so a 16-key search was a
///                   16x self-stack for the price of the charges. Three slots, three keys, three
///                   simultaneous copies at most — and not the current LENGTH either, or a
///                   one-slot container would hand every swap the departed item's live source.
///                 GOLDENS: none.
/// 0.0.305-alpha — `Projectile.skill_id`, a renderer hint. The client could ask only `is_magic`,
///                 so every magic bolt drew the same fireball sheet — Lightning Bolt flew as fire
///                 and flashed a zap on landing, both at once. Carrying the firing skill lets the
///                 client pick flight art and per-bolt impact through the map it already keeps,
///                 rather than the renderer growing its own idea of elements.
///                 `#[serde(skip)]` like `sky_fall` beside it: transient flight cosmetic, no
///                 save-shape change. GOLDENS: none — verified, not assumed.
/// 0.0.306-alpha — COMMANDER SMITE, the first new Commander skill (`docs/commander-skills-design.md`).
///                 A placed 2×2 blast for `5 × party average level` PURE damage, gated by a
///                 per-skill cooldown AND a shared charge pool the whole Commander family will
///                 spend from. Flat rather than `power_bp × ATK` because a % multiplier is a
///                 different skill on a Mage party than on a Swordsman one; measured against the
///                 catalog it holds 4-5% of a boss from Lv30 to the cap. PURE for the same reason
///                 the number is flat — DEF would put the build variance straight back.
///                 The average counts DEAD members (a death must not weaken the answer to the
///                 fight that killed them) and EXCLUDES pets, which `drops::party_max_level` does
///                 not — harmless for a max, a real bug for an average.
///                 STATE: `commander_charges` / `commander_charge_tick` / `commander_cd`, all
///                 skip-if-default. The regen clock is stamped on the SPEND, not held at `now`
///                 while full: the obvious version writes the stamp every tick, which makes the
///                 field non-default in every save and MOVED TWELVE GOLDENS before it was caught.
///                 A test now pins that a party which never casts serializes none of the three.
///                 GOLDENS: none — 39/39 unchanged, verified not assumed.
/// 0.0.307-alpha — COMMANDER SKILL OWNERSHIP — smite is now a first UNLOCK, not a starter
///                 (`docs/commander-skills-design.md`, user 2026-08-28). New state
///                 `commander_skills_owned: BTreeSet<String>` (skip-if-empty). `commander_smite`
///                 no-ops unless its id is in the set, and `view_state` filters the bar catalog to
///                 owned skills — one owned set read by both, so the bar can never draw a skill the
///                 handler would refuse. Populated by a DEBUG event `SetCommanderSkillOwned`
///                 (`InputAuthority::Debug` — a play-test stand-in for the real trigger, Commander
///                 level → a server-authored grant, which is unbuilt and only swaps HOW the set
///                 fills). GOLDENS: none expected — the set is empty in every golden (none unlock a
///                 Commander skill), so skip-if-empty omits it and the bytes are identical; verify.
/// 0.0.308-alpha — COMMANDER SKILL — the PRODUCTION grant. `InputEvent::GrantCommanderSkill
///                 { skill_id, nonce }` (`InputAuthority::Server`) inserts into
///                 `commander_skills_owned` — the server issues it when the account claims the level
///                 reward that unlocks the skill (accept-to-unlock; docs/commander-level-and-outpost).
///                 The sim is total: it just inserts (idempotent) and never reads the nonce; the
///                 nonce is a keyed blake3 MAC over `(secret, account_id, skill_id)` that
///                 verify-server recomputes (`commander_grants_valid`), so a forged unlock rejects the
///                 save. The DEBUG twin `SetCommanderSkillOwned` (0.0.307) stays the offline test
///                 path. GOLDENS: none — no golden authors a grant; verified not assumed.
/// 0.0.309-alpha — DEPLOYMENT GRACE. For the first `tick::DEPLOY_GRACE_TICKS` (20 = 2 s) of a
///                 freshly-opened zone NOBODY acts — the whole combat+movement half of the tick
///                 (`resolve_actions`: entity loop, pets, traps, arrived projectiles, absorb
///                 bursts, explosions) is skipped, on both teams. The map opens with the party
///                 walking into position (the HD-2D renderer strides it in from the left; monsters
///                 wait) before the fight begins, instead of swings landing at tick 0 while the
///                 sprites are still sliding in. Spawns + upkeep still run, so the field populates
///                 during the pause. Anchored on `zone_spawn.opened_at_tick`, the opening window's
///                 origin — so it is per zone entry and replay-stable (opened_at_tick is
///                 serialized). GOLDENS: EXPECTED to move — every scenario that clears a zone now
///                 has 20 idle ticks per entry; re-pinned deliberately.
/// 0.0.310-alpha — SWEEP-CLEAR + BOSS RIGHT-CENTRE, the continuous-world loop. (1) A zone now
///                 clears when the field is SWEPT — the boss has appeared (spawner halted) AND no
///                 monster is left alive (`tick::mark_zone_cleared_if_swept`) — not the instant the
///                 boss dies; leftover mobs must be finished too. `rooms_cleared` moves to the
///                 sweep, `boss_kills` stays on the boss death. (2) The boss enters at the field's
///                 RIGHT-CENTRE (`spawn::pick_random_cell_for_boss`, now deterministic — nearest
///                 free cell to `(w-2, mid)`, no PRNG draw) so the left-spawning party marches
///                 rightward to it and walks on rightward to the next area. GOLDENS: the two
///                 boss-reaching runs (alpha6, band4) move on both counts; re-pinned deliberately.
/// 0.0.311-alpha — DEPLOY GRACE 20 -> 36 ticks (2.0 -> 3.6 s). The renderer's room-to-room camera
///                 pan + party march take ~3.4 s; at a 2 s grace the fight resumed while they were
///                 still travelling (user: sim acted before the party arrived). Sizing the grace to
///                 the travel keeps the pan/walk at their tuned speed and still lands everyone before
///                 combat — rushing the visuals instead read as "ไวแปลกๆ". GOLDENS: any run that
///                 opens a zone and acts within the first 36 ticks shifts; re-pinned deliberately.
/// 0.0.312-alpha — DEPLOY GRACE 36 -> 31 ticks (3.6 -> 3.1 s). 3.6 s left a dead beat after the
///                 party had arrived (user); trimmed 0.5 s. The march still lands inside it; the
///                 camera's 3.4 s pan finishes settling right as combat resumes. GOLDENS: the
///                 zone-open runs (alpha6, band4) shift again; re-pinned deliberately.
/// 0.0.313-alpha — DEPLOY GRACE 31 -> 20 ticks (3.1 -> 2.0 s). The room-transition pause read as
///   too long in play (user); back to the original 2.0 s. The renderer's camera pan + walk-in are
///   re-synced to land inside the shorter window by SHORTENING the travel, not speeding the walk
///   (CAM_TRAVEL_SEC 3.4 -> 2.0, WALK_IN_CELLS 5 -> 3 in arena3d_scene.gd), which is why the grace
///   was raised in 0.0.311 in the first place — the pan needed the time, so the pan is now shorter.
/// 0.0.314-alpha — GACHA PIVOT, the stat-line cuts. Four lines that ran from the six attributes
///   into combat are gone. (1) CAST TIME is a fixed per-skill value; DEX no longer shortens it —
///   Incantation Mastery and cast-speed sources still apply, clamped to +-50%. (2) ASPD is a fixed
///   per-character number from the catalog; the AGI/DEX terms are zero, so level alone never moves
///   it and only equipment or another source can. (3) MONSTER MDEF is removed — `effective_hard_mdef`
///   returns 0 and monster soft MDEF is 0; magic is mitigated by the same DEF as physical (0.0.313's
///   single curve), and Freeze resist reads a constant instead of MDEF. (4) MONSTER MATK is removed;
///   a monster has one attack number. (5) The attribute-granting BUFFS (StrAdd/AgiAdd/VitAdd/IntAdd/
///   DexAdd/LukAdd/AllStatAdd/AgiPct/DexPct/AgiSlowAdd) no longer fold into `buffed_stats`; the
///   BuffKind variants stay because BuffKind is serialized append-only. TESTS: golden_beta7's subject
///   ("magic ignores DEF", "INT feeds MATK") is the removed mechanic — rewritten to assert the bolt
///   lands and is mitigated, not re-pinned around a rule that no longer exists. GOLDENS: beta7 only;
///   the sim-cli curve is unchanged (seed 42, 6000 ticks: 203 kills / 15 bosses / Lv5, identical).
/// 0.0.315-alpha — SP IS GONE. The pool, its regen, its costs and its ceiling are all removed;
///   the second bar under HP is the LIMIT BREAK gauge (0.0.84) from here on. `Entity.sp`,
///   `sp_base`, `sp_per_level`, `combat::effective_max_sp`, `SkillDef.sp_cost`,
///   `SkillDef.sp_regen_flat` + `skills::passive_sp_regen`, `tick::SP_REGEN_PER_SEC`, the
///   catalog's `MonsterDef.sp` / `UnitDef.sp_base` / `sp_per_level` (game_data
///   0.0.88-alpha) and the bridge's `reserved_sp` are deleted. `sp_cost` was already 0
///   everywhere but three Swordmaster leftovers, so nothing but those three ever paid.
///   The reserve BUDGET survives the pool under its own name: `sp_reserve_slots` →
///   `loadout_slots`, `sp_reserve_capacity` → `loadout_capacity`, and the capacity is now the
///   FIXED `LOADOUT_CAPACITY = 10` instead of `10 + INT/10` — INT feeds nothing any more
///   (0.0.314). Mana Spring keeps its def but grants nothing (its effect was SP regen; the
///   design gives it +Charge Rate in the kit rewrite). Arcane Ward stays as-is: it costs slots,
///   not SP. FFI: `eff_max_sp` → `eff_lb_max` in view_state, `max_sp` → `lb_max` in
///   effective_stats, `sp_reserve_used` → `loadout_used`; the Godot party frame's second bar
///   and the skill panel follow. GOLDENS: every pin moves — `sp` leaving the entity moves the
///   serialized shape — re-pinned deliberately. The sim-cli curve is unchanged (seed 42, 6000
///   ticks: 203 kills / 15 bosses / Lv5), which is the measurement that says SP gated nothing.
/// 0.0.316-alpha — THE LAST FOUR ATTRIBUTE READERS. Nothing in combat reads STR/AGI/VIT/INT/DEX/
///   LUK any more; the fields survive this commit but no longer feed anything. (1) An entity with
///   no AUTHORED attack curve — a monster, a pet, a fixture — attacks with the flat `atk` from its
///   catalog entry and nothing else (user); `Stats::melee_atk_bonus`/`ranged_atk_bonus` (RO Renewal
///   StatusATK) are deleted. Monster attributes were never x10-scaled, so this also drops an
///   unscaled term out of a scaled formula. (2) BRUTUS card: Bash's rider was `STR x 50->100%`,
///   now `+5->10% ATK` — `bash_str_bonus` -> `bash_atk_bonus_bp`, returning basis points that join
///   `power_bp` directly instead of a flat number divided back out by ATK at the call site.
///   (3) FLAME SHIELD's absorb pool was `INT x level x 2`, now `effective_def x level / 2` (Lv10 =
///   5x the caster's DEF). (4) HEAL drops the INT term: `(BaseLevel + INT)/5` -> `BaseLevel/5`,
///   x skill_level x 3 + ATK. GOLDENS: 10 pins move (monster ATK is a hair lower). sim-cli, seed
///   42 / 6000 ticks: 203 kills / 15 bosses / Lv5 unchanged; XP 1045 -> 1034, breaks 10 -> 12.
/// 0.0.317-alpha — THE SIX ATTRIBUTES ARE GONE. `Stats` loses `str_`/`agi`/`vit`/`int_`/`dex`/
///   `luk`, `base_stats` and `points_unspent`, and with them `set_class_base`, `cost_to_raise`,
///   `spent_on`, `get`/`allocate`/`reset`, `status_matk`, `growth_order`/`grow_into`,
///   `STAT_CAP`/`STAT_MIN`, `xp::stat_points_for_level` and `skills::buffed_stats` (a fold point
///   with nothing left to fold — callers read `entity.stats`). A character's power is AUTHORED:
///   the per-character ATK curve, the class HP curve, gear, and the Potential board. What still
///   grows with LEVEL — soft DEF, HIT, FLEE — stays; crit is a flat base plus gear.
///   REPLAY: `InputEvent::AllocateStatPoint` and `ResetStats` are KEPT as no-ops (with `StatKind`,
///   kept purely as the shape they name) — `InputEvent` is serialized and every save replays its
///   whole log. `BuffKind`'s StrAdd/AgiAdd/… and `StatMod::Str`/`Agi`/… are likewise kept and
///   inert; the gear rolls and Exemplar axes that grant them are repointed when the substat table
///   is rewritten, and until then those lines are DEAD ROLLS — known, deliberate, tracked.
///   CATALOG (game_data 0.0.89-alpha): the six columns come off all 67 monsters and every class.
///   FFI: `eff_str`..`eff_luk`, `points_unspent`, `stat_cost`, `classes_json`'s stat block; Godot
///   loses the six-row stat block with its + buttons and RESET, and the creation screen's stat
///   radar + table. GOLDENS: all 13 move (the serialized shape changed); the sim-cli curve does
///   not — seed 42 / 6000 ticks: 203 kills / 15 bosses / Lv5 / XP 1034, identical to 0.0.316.
/// 0.0.318-alpha — NO DEAD ROLLS. The six attribute lines stop being GRANTED anywhere, closing
///   the hole 0.0.317 left open. (1) GEAR: `Family::Str`..`Luk` leave the roll pool (30 -> 24
///   families), so those rolls go to the families that are left — element damage, element resist,
///   ATK/HP/CRIT and the slot-restricted lines. A pre-0.0.318 item can still CARRY an attribute
///   line: `family_of` returns None for it, so it passes through a reroll untouched (dropping it
///   would change the item's line count, which stars own). (2) EXEMPLARS: the third axis was the
///   RO attribute that suited the class (thief AGI, archer DEX, casters INT); it is CRIT DMG for
///   every line now, SR +5% base / +1%/star, SSR +8% / +1.6%/star — a stat the gear pool also
///   rolls, so a hero reads as a strong familiar line rather than its own currency. (3) CARDS
///   (game_data 0.0.91-alpha): the attribute lines are CUT, not converted, and what they occupied
///   is designed fresh — see 0.0.319. The client loses the market's "Base Stats" substat filter.
///   GOLDENS: the two runs that generate drops (alpha6, band4) move — a droppable ItemDef's
///   template lives in ZoneSpawnState. The curve does not: seed 42 / 6000 ticks, 203 kills / 15
///   bosses / Lv5 / XP 1034.
/// 0.0.319-alpha — CARD LINES, DESIGNED RATHER THAN CONVERTED (game_data 0.0.91-alpha). The eight
///   cards that carried an attribute lose that line outright; carrying the old numbers across into
///   whatever stat the attribute used to pay would have been the retired design wearing a new name
///   (user). Two holes opened and are filled deliberately:
///   (1) `card_star_ranks` — the irregular bump at ★1/★3/★5 — was attributes on six cards, leaving
///   the whole mechanic with no author. It is now a SECOND stat that appears only at high stars,
///   chosen to widen each card's role rather than deepen the line it already leads with:
///   pinkling +3% Fire RES · greenling +5 DEF · yellowling +3 HIT · rechum +2% Heal ·
///   twinling +1% CRIT · dustelle +2% Move Speed · brutus +1.5% CDR, each ×3 ranks.
///   (2) Brutus kept only its coded Bash rider, and Scarlette lost its bound bonus: Brutus is one
///   flat-ATK line (+5, bound +1, the shape Pinkling already uses) and Scarlette's bound bonus
///   follows its own stat (+2% crit DMG). Values are in the units the other card lines already use
///   — the card/gear scale against the x10 stat scale is a separate pass, not this one.
/// 0.0.320-alpha — THE TWO MISSING GACHA STATS. `StatMod` gains `ElementDmgBp(Element, i32)` and
///   `ChargeRateBp(i32)` — the two entries in the stat set (`skill-system-redesign.md` §4.5.1)
///   that nothing in the sim could express. The Potential board needs both (four of its ten stat
///   nodes name them), which is why they land first. ELEMENT DMG% keys on the ATTACKER's own
///   element, the mirror of `DmgVsElement` which keys on the defender's, and folds into the same
///   element-keyed multiply the Broomhilda card special already used. CHARGE RATE scales the
///   Limit Break gain at `charge_limit_break` — the one point every source passes through, so a
///   future charge source cannot forget it — and never touches the ceiling, which stays the
///   Ultimate's `lb_max`. Both are AUTHORED-ONLY: `family_of` returns None, so gear does not roll
///   them and the enchant draw set is unchanged. Haste and Elemental RES% needed nothing new —
///   they are `CooldownBp` and `ResistElement`, already built. Nothing grants either mod yet, so
///   every golden is byte-identical and the curve does not move.
/// 0.0.321-alpha — BREAK DAMAGE, and the two stats that own it. Emptying a Toughness gauge dealt
///   NOTHING until now — it refilled the bar and emitted a `Break` hit event with amount 0. It
///   deals its own packet now, built HSR's way: `BREAK_DMG_PER_LEVEL(40) x attacker level x
///   (toughness_max + 20)/40`, scaled by the new `StatMod::BreakDmgBp` — never by ATK, so a break
///   build and a damage build are two different builds instead of one stat paying for both. It is
///   a real packet: DEF, the element triangle and shields all apply.
///   `StatMod::BreakBuildupBp` is the other half (user's names: Break Build-up / Break Damage) —
///   it scales the CHIP, so `chip_toughness` now takes the attacker's build-up. ZZZ splits its
///   anomaly stat the same way (Mastery = how often, Proficiency = how hard) and for the same
///   reason: one stat doing both makes every break build identical. Both are authored-only; gear
///   rolls neither, so the enchant draw set is unchanged.
///   ⚠️ MEASURED: in the sim-cli run all 12 breaks land on an ALREADY-DEAD monster — a trash mob
///   dies in about three hits and a 60-point gauge at 20 per chip also empties on the third. At
///   the current sizing break damage is a corpse event for trash and only matters to bosses. A
///   30-point gauge makes it live: breaks 12 -> 169, and the party ends the run at 769 HP instead
///   of 383 (kills 203 and bosses 15 either way — the spawn rate caps those, not damage). The
///   gauge is data, not code; this records the number, it does not change it.
/// 0.0.322-alpha — MONSTER HP x2 (game_data 0.0.92-alpha), which is what makes BREAK real. The
///   0.0.321 measurement said every break landed on an already-dead monster: a trash mob died in
///   about three hits and a 60-point gauge chipped 20 at a time also emptied on the third. Two
///   ways out — shrink the gauge, or make the fight longer. The fight is longer (user).
///   `level_base[].hp` doubles across all 100 rows, so every monster and every boss doubles from
///   the one table they all read. MEASURED, seed 42 / 6000 ticks: breaks 12 -> 114 and break
///   damage goes from 2.4% of the party's output to 14.7% — the gauge now empties while the target
///   is alive. The cost is pace: 203 kills -> 160, Lv5 -> Lv4, and the run ends in band 4 instead
///   of band 6; wipes fall 3 -> 1, because a party that clears more slowly also walks into
///   over-level content more slowly. At 12000 ticks it reaches band 5-3 with 292 kills, 21 bosses,
///   7 wipes. `BREAK_DMG_PER_LEVEL` stays 40: across all three configurations where breaks land on
///   a living target it puts break at 13-15% of damage, and 80 puts it at 25%, which is a second
///   damage source rather than a payoff. GOLDENS: alpha6 and band4 (the two runs that fight);
///   `dungeon_bosses_scale_...` re-pins the two dungeon bosses' HP, which is the doubling arriving
///   where it should.
/// 0.0.323-alpha — A BOSS CARRIES ITS OWN TOUGHNESS GAUGE, 200 (`docs/element-weakness-system.md`
///   §6.2). Bosses read `MonsterDef.toughness_max` like anything else, so they carried the field
///   gauge — 60, three landed hits — and a boss fight broke every few seconds, which is not an
///   event. The SAME monster now has two gauges: 60 standing in a field, `BOSS_TOUGHNESS_MAX`
///   (200) as a zone's boss, about ten hits or five on the element it is weak to. Two fields
///   rather than a multiplier because `MonsterDef.boss_toughness_max` (0 = use the shared
///   constant) lets one boss be tuned without moving every other — a boss with an add phase and a
///   boss that stands alone do not want the same bar. `toughness_max == 0` still means no gauge at
///   all, boss or not: that is a property of the creature. MEASURED, seed 42 / 6000 ticks: breaks
///   114 -> 106 (the eight that vanish are a boss re-breaking), kills 160 -> 158, boss kills 11
///   either way — the pace is unchanged because the run spends its time on trash, not bosses.
///   GOLDENS: alpha6 and band4, the two runs that reach a boss.
/// 0.0.324-alpha — ONE BREAK FOR EVERYONE, and BREAK TRIGGERS for the difference (user
///   2026-09-07). An idle game cannot ask "who do I break, with what, now" — the player never
///   picks a target — so the break stops being where characters differ and becomes a fixed event
///   they all produce and all read.
///   THE EVENT: damage + `BuffKind::Stun` + `DmgTakenPct` for `BREAK_STUN_TICKS` (30t, boss 50t),
///   the vulnerability at `BREAK_VULN_BP` (+20%) — the half of a break the REST of the party
///   collects, which is the half an idle game can build around (ZZZ's stun window on HSR's frame).
///   Both statuses are applied directly, not through `cast_buff`, because that path makes bosses
///   immune to Stun: a rule for skills, and a boss that cannot be broken has no use for the long
///   gauge 0.0.323 gave it.
///   THE DAMAGE now reads the TARGET's level, not the breaker's (`40 x target level x gauge`), so
///   a support landing the last chip is worth exactly what a striker landing it is worth.
///   THE GAUGE empties to 0 and refills ACROSS the stun (`tick::refill_toughness`), arriving full
///   as the target gets its turn back — the bar on screen IS the timer. A hit inside the window
///   chips nothing and cannot re-break. `Entity.break_ticks` counts the window; "gauge below max"
///   would have healed a merely-chipped bar for free.
///   THE TRIGGER is vocabulary: `SkillDef.break_trigger` replaces the hardcoded `"ember_cascade"`
///   in the tick loop and the cast path, so a character that answers breaks is CONTENT now.
///   Renamed with it: `ember_queue`/`ember_cd`/`EMBER_*` -> `break_queue`/`break_cd`/`BREAK_*`
///   (the generalization alone left the state hash byte-identical, which is how we know it was a
///   rename and not a change). MEASURED, seed 42 / 6000 ticks: breaks 106 -> 112, kills 158 -> 153,
///   boss kills 11 and wipes 1 unchanged — the stun costs the monsters actions while the
///   non-snapping gauge lets breaks come slightly sooner, and the two nearly cancel.
/// 0.0.325-alpha — THE TOUGHNESS GAUGE SCALES WITH LEVEL (game_data 0.0.93-alpha). A flat
///   60-point bar is wrong at both ends of the game, and measurement is what showed it: for a
///   level-matched geared party it breaks 0.23 times per kill at Lv10 and 6.9 times at Lv70 —
///   unreachable early, and a monster stun-locked from spawn late, with the 0.0.324 vulnerability
///   window permanently on. The invariant is BREAKS PER KILL, not the size of the bar:
///   `hits_to_break = gauge / TOUGHNESS_CHIP_BASE` has to track hits-to-kill, which measured 1.5
///   at Lv10 rising past 14 at Lv70. So `defs::toughness_for_level(l) = 30 + 2l/3 + l^2/30`, and
///   `MonsterDef.toughness_max` becomes a PERCENT of that (every monster 100, `BOSS_TOUGHNESS_PCT`
///   300, 0 still means no gauge at all). MEASURED: breaks/kill 0.65-0.90 for geared parties at
///   Lv20/Lv50 and 1.70 on the natural Lv1 run, against 0.23-6.9 before. The base term was fitted
///   the same way — 14 / 30 / 45 all measured against both a geared party and the natural run,
///   because a party on its own drops needs more hits per kill than the `--kit` reference and
///   would over-break on a bar fitted only to geared play. Seed 42 / 6000 ticks: kills 153 -> 164,
///   bosses 11 -> 12, wipes 1 -> 0, breaks 112 -> 279. GOLDENS: alpha6 and diag_band4.
/// 0.0.326-alpha — WEAKNESS ONLY: an attack chips the Toughness gauge ONLY if the target is weak
///   to its element (game_data 0.0.94-alpha). HSR's rule, and the thing that makes the element
///   chart decide anything — before this every hit chipped 20 and a matching one 40, so element
///   was a 2x on a bar that filled anyway. `MonsterDef.weaknesses` lists 1-3 elements (mean 1.99
///   across all 67, `docs/element-weakness-system.md` §2), carried to `MonsterTemplate` and
///   `Entity`; `Entity::is_weak_to` reads the list, falling back to the element MATRIX when it is
///   empty so pre-0.0.326 data still breaks the way it used to.
///   THE OBJECTION THAT WAS WRONG: that an idle party with no matching element loses the whole
///   system without noticing. The player picks the band they farm (user), so an unmatched party is
///   a reason to move or to bring someone else — which is the decision an idle game HAS, where
///   "who do I hit now" is the one it does not.
///   THE LISTS ARE AUTHORED, NOT RANDOM: each of the six launch elements is a weakness on 20-24
///   monsters, so no element is a dead collection, and the matrix's own pairings are picked first.
///   Half the monsters below Lv5 answer to Physical — every party starts Physical-only, so none
///   would break anything at all; but making them ALL answer to it would teach a new player that
///   elements do not matter. MEASURED breaks/kill, before -> after: natural Lv1 1.70 -> 0.99,
///   geared Lv20 0.65 -> 0.42, Lv50 0.90 -> 0.66, Lv70 1.23 -> 0.19 — inside the 0-1 the design
///   asked for, and now set by how many of the party match rather than by the size of the bar.
///   Seed 42 / 6000 ticks: kills 164 -> 169, bosses 12, wipes 0, breaks 279 -> 167.
/// 0.0.327-alpha — MONSTER HP BACK TO 1x, AND THE GAUGE CURVE RE-FITTED (game_data 0.0.95-alpha).
///   Both were answers to a problem 0.0.326 solved a different way. The x2 HP of 0.0.322 existed
///   to make a monster survive long enough to be broken; the level-scaled bar of 0.0.325 existed to
///   stop a strong party breaking it every three hits. Once only a WEAKNESS chips the bar, the
///   party's element coverage sets the rate and neither lever is doing that job any more — so the
///   HP doubling is REVERTED (`level_base[].hp` halved back, the two dungeon bosses with it) and
///   the curve is re-fitted from `30 + 2l/3 + l^2/30` to `20 + l/2 + l^2/120`, which is much
///   flatter: the bar reaches 95 at Lv70 rather than 240.
///   The old shape measured breaks/kill 0.18 at Lv50 and 0.02 at Lv70 — the mechanic switching off
///   exactly where it should be loudest, because it was still sized for "every hit chips". Four
///   shapes measured (natural / geared Lv20 / Lv50 / Lv70): the old one 0.61/0.14/0.18/0.02,
///   `25 + l/2 + l^2/60` 0.61/0.17/0.52/0.10, the chosen one 0.61/0.43/0.99/0.19, and a plain
///   `20 + l` 0.61/0.21/0.99/0.19. The natural run is identical in all four: below Lv5 the bar is
///   under one chip whatever the shape. Lv70's 0.19 is the auto-party's element COVERAGE against
///   band-70 monsters, not the curve, and tuning the curve to chase it would be tuning away the
///   variable this rule exists to hand the player.
///   Seed 42 / 6000 ticks: 207 kills / 15 bosses / 2 wipes / Lv5 / 126 breaks — the pacing the
///   game had before the HP change (203/15/3), with break now landing on something alive.
/// 0.0.328-alpha — A SHORTER STUN, AND A GAUGE THAT GROWS EVERY TIME IT BREAKS (user). Break stun
///   3 s -> 2 s for a normal monster (a boss keeps 5 s), and a monster's `toughness_max` is
///   multiplied by 3/2 on every break of that body. Bosses are EXEMPT: a boss fight is one long
///   body whose whole rhythm is repeated breaks, so escalating there would delete the mechanic from
///   the only fight built around it — its 300% bar already prices the repetition in.
///   WHY: the stress measurement (every monster weak to every element, i.e. perfect coverage) had a
///   level-70 party breaking a monster 2.87 times per kill — a monster that never acts. Escalation
///   puts the ceiling in the STRUCTURE rather than hoping the numbers stay low: a fourth break
///   costs 3.4x the first, so no amount of Break Build-up can chain stuns indefinitely.
///   MEASURED, real coverage / perfect coverage: natural 0.61/1.91, geared Lv20 0.34/1.27,
///   Lv50 0.86/1.25, Lv70 0.13/2.36 — against 0.43/1.40, 0.99/1.47, 0.19/2.87 before. The runs
///   that barely move are the ones where a monster dies before a second break, which is escalation
///   correctly doing nothing. Break DAMAGE reads the gauge, so the harder second break also PAYS
///   more — the escalation is a price with a matching reward, not a penalty.
///   Seed 42 / 6000 ticks: 207 kills / 15 bosses / 2 wipes / 126 breaks, unchanged.
/// 0.0.329-alpha — THE DAMAGE MULTIPLIER MOVES TO THE WEAKNESS LIST TOO, closing the split where
///   0.0.326 left the game reading elements two ways at once: BREAK asked the list while DAMAGE
///   still asked the 150/50 matrix, so a monster could take 1.5x from an element that could not
///   chip its bar. `combat::element_mult_vs(target, el)` is the one rule now — 120% on the target's
///   weakness list, 80% on its resist list, 100% otherwise (`docs/element-weakness-system.md` §2).
///   `MonsterDef.resists` is new and OPT-IN, for elites and bosses; every monster ships empty, so
///   in practice this is 120/100 and bringing the wrong element costs the bonus, never a penalty.
///   The matrix survives as the fallback for a target with no lists at all — old saves, summons,
///   players — so nothing that predates the lists changes. FROZEN deliberately stays on the matrix:
///   the RO rule is that ice is what the blow meets, so the creature's own lists do not apply.
///   Skill auto-pick scores on the same rule (the mage's bolts).
///   The gap between right and wrong element narrows from 3x to 1.5x — the reason to collect
///   elements is meant to be the BREAK gate and Disorder, not the multiplier. MEASURED: seed 42 /
///   6000 ticks, kills 207 -> 201, bosses 15, breaks 126 -> 136. Losing the 150% bonus and losing
///   the 50% punishment very nearly cancel, which is what §2 predicted and why the monster table
///   needs no re-tune. GOLDENS: alpha6 and diag_band4.
/// 0.0.330-alpha — MATK IS ATK, ALL THE WAY DOWN, and the BREAK PAIR becomes rollable.
///   0.0.83 merged the two BASE numbers but left every gear-side MATK channel standing, so a
///   staff's lines helped only magic skills and a blade's only physical ones — the dead-roll the
///   merge existed to kill (`skill-system-redesign.md` §4.5.1: half of what drops is worthless to
///   its owner, which is what makes a market dull). Now `effective_atk` IS `effective_atk`, and
///   every MATK source folds into it: `matk_bonus`, refine MATK, star MATK, `passive_matk_bonus`,
///   `aura_matk`, Apex Focus, `MatkAdd`/`MatkPct`, and the `MatkFlat`/`MatkBp` gear lines (which
///   sum beside their ATK twins in `stat_mods::atk_flat`/`atk_bp`, so no existing item loses
///   value). Per ITEM the pairs take the MAX, not the sum: refine hands a weapon both terms (its
///   ATK curve floors at the iLv tier even on a staff), and summing would pay a magic weapon
///   twice. `IgnoreMdefBp` folds into `ignore_def_bp` for the same reason — there has been one DEF
///   to penetrate since 0.0.85. GEAR POOL: `MatkBp`, `MatkFlat` and `IgnoreMdefBp` leave
///   `FAMILIES`; `BreakBuildupBp` and `BreakDmgBp` JOIN it (weapon + accessory, the "what my hits
///   are worth" slots), so the stat that controls break frequency finally has a source — 26 -> 23
///   -> 25 families. Build-up rolls NARROW (2-12% vs 5-20%) and is capped at +50% in
///   `stat_mods::break_buildup_bp`, because break is a RATE and a rate has a ceiling of meaning:
///   the stress case already measured 2.36 breaks/kill with no build-up at all. `BreakDmgBp` is
///   uncapped — magnitude can grow where frequency cannot. Retired lines return `None` from
///   `family_of`, so an old item keeps them through a reroll instead of losing a line.
///   CLIENT: the stat sheet drops its MATK and MDEF columns; a MATK line reads "+n ATK".
///   Seed 42 / 6000 ticks unchanged (201 kills / 15 bosses / 136 breaks): the reference party
///   wields no magic weapon and its gear rolls no substats, so nothing in that run touches either
///   half of this.
/// 0.0.331-alpha — NO CAP ON BREAK BUILD-UP after all (user 2026-09-08): "don't add a ceiling
///   yet, or I will not remember which stats have one". `stat_mods` holds exactly seven capped
///   stats (ignore-DEF, cooldown, cast, move speed, element resist, status resist, damage
///   reduction) and that list is only useful while it is short enough to hold in your head — a
///   stat capped in a comment nobody re-reads behaves differently from how it looks. The reasoning
///   for capping it is left in place beside the others, so if play shows a build stacking its way
///   back to stun-lock the ceiling goes there rather than being invented at a call site.
/// 0.0.332-alpha — THE RETIRED-STAT SWEEP: three things that were cancelled but still computed.
///   (1) MDEF. `effective_hard_mdef` (always 0), `effective_soft_mdef` (`level/4 x scale` for a
///   player) and `effective_mdef` survived 0.0.85's merge into one DEF, read by NO formula — but
///   still SHOWN, through `effective_stats`, `stats:<id>` and the stat sheet. A number on the sheet
///   that changes nothing is worse than a missing one: a player builds toward it. All three go,
///   with `Stats::soft_mdef`, the `mdef` key in both FFI surfaces, and the `matk`/`hard_mdef`
///   columns of `--dump-stats` (MATK has been the same number as ATK since 0.0.330).
///   (2) `stat_mods::stat_adds` — the six-attribute bucket. Its last real reader went with
///   `buffed_stats` in 0.0.317; it was summing gear STR/AGI/… into an array nobody looked at.
///   (3) The freeze roll's `97 - MDEF - LUK/3` had already collapsed to a literal 97 with a dead
///   `mdef` local beside it; it is now `FREEZE_RESIST_BASE`, named, with the binding test rewritten
///   to assert the one thing still true — nothing about a TARGET changes its freeze chance.
///   NOT swept, and deliberately: the retired stat BUFFS are still granted by Soul Paths, Blessing,
///   Improve Concentration, the food buffs and the two slows. They fold to nothing (0.0.314), so a
///   player sees an icon that does nothing and pays for path nodes that do nothing — the same dead
///   -roll shape, but repointing them belongs with the kit rewrite that will rewrite those skills
///   anyway. Recorded here rather than half-fixed.
///   Client: an item's MATK line reads as ATK, Ignore MDEF as Ignore DEF, and the market's
///   MATK%/MATK/Ignore-MDEF search leaves drop. No golden moves; seed 42 is untouched.
/// 0.0.333-alpha — HIT/FLEE ARE GONE; DODGE CHANCE REPLACES THEM (game_data 0.0.96-alpha).
///   The accuracy pair was still live and still expensive: MEASURED, a level-matched geared party
///   missed 28.9% of its swings at Lv20, 31.6% at Lv50 and 36.6% at Lv70 — a third of the damage,
///   invisible on screen and unanswerable by any build, because both sides had been level-only
///   functions since 0.0.317 took the attributes out. `docs/skill-system-redesign.md` §4.5.1.1
///   predicted exactly this and kept the pair only because removing it was work; the work is here.
///   NEW: `StatMod::DodgeBp` — the defender's chance to take nothing. Sources stack INVERSE
///   MULTIPLICATIVELY (`1 - (1-p1)(1-p2)...`), Diablo 4's rule, which is where the diminishing
///   return comes from and why it needs no cap: two 40%/30% lines are 58%, not 70%, and affixes
///   alone never reach 100%. `combat::roll_dodge` takes ONE draw and only when something can
///   actually avoid the blow, so an ordinary fight's PRNG stream carries no trace of the question.
///   BLIND survives the cut as the one thing accuracy did worth keeping — a blinded attacker
///   whiffs 25% (`BLIND_MISS_BP`), folded into the same roll by the same formula.
///   REMOVED: `accuracy_bp`, `roll_hit`, `mobbed_flee`, `effective_hit_points`,
///   `effective_flee_points`, `Stats::hit_points`/`flee_points`, `HIT_BASE`/`FLEE_BASE`,
///   `Entity.mob_hit`/`mob_flee`, `MonsterDef.hit`/`flee`/`hit_mult_bp`/`flee_mult_bp` (126 data
///   columns), and the Hit/Flee gear families. `SkillDef.hit_flat_per_level`/`flee_flat_per_level`
///   become `dodge_bp_per_level`: Nimble Mastery is +1% dodge per level, and Hawk Sight keeps the
///   attack-range half that was always the reason to take it. Dodge rolls on garment/shoes/shield.
///   MEASURED, seed 42 / 6000 ticks: 201 kills -> 215, bosses 15 -> 16, wipes 3 -> 1. At a matched
///   band the gain runs +16% at Lv20 to +37% at Lv70, which is the miss tax coming back.
///   GOLDENS: nearly all of them — a draw per swing disappeared, so every stream shifted. Two
///   tests were rewritten rather than re-pinned: BETA-6 compared damage against a stale
///   per-hit constant (16, from a retune long past) and now runs a control fight without dodge
///   and asserts the dodger takes less; ALPHA-4 pinned "3 items dropped", which was pinning one
///   draw sequence, and now asserts the stash has something to equip.
/// 0.0.334-alpha — THE MDEF PIPE, END TO END (game_data 0.0.97-alpha). 0.0.332 removed the MDEF
///   a player could SEE; the number itself was still being authored, parsed, scaled, spawned and
///   carried on every monster in the game, read by nothing since 0.0.85. Gone now: the 64 `mdef`
///   columns in `game_data.json`, `MonsterDef.mdef`/`mdef_mult_bp`, `MonsterTemplate.mdef`,
///   `Entity.mob_mdef`, the quarter-of-level term in `base_scaled_stats`, and `Damage
///   .ignore_mdef_bp` (a straight copy of `ignore_def_bp` since 0.0.330, never read). One armour
///   number, authored once. `StatMod::IgnoreMdefBp` STAYS — it is a serialized shape, it is
///   append-only, and it already sums into `ignore_def_bp`, so old gear keeps its line.
///   `BuffKind::HitDownPct` is retired the same way: nothing reads HIT any more, so Open Wound's
///   Blood Mist rune lands -10% `AtkPct` over the same 5s window instead — the mist cuts what its
///   victims swing with rather than what they aim with, and stays a real choice against the
///   Vulnerable rune (which raises damage TAKEN, not lowers damage DEALT).
///   Three tests were rewritten rather than re-pinned: the dungeon-boss tier pin and the two zone
///   pins each asserted a `mdef` column that no longer exists.
///   MEASURED, seed 42 / 6000 ticks: 215 kills / 16 bosses / 1 wipe / 129 breaks — identical to
///   0.0.333. GOLDENS: ALPHA-6 and DIAG-BAND4 move, and only because the catalog VERSION STRING
///   is part of the hashed state; no monster's numbers changed.
/// 0.0.335-alpha — `StatMod::IgnoreMdefBp` DELETED (user 2026-09-08: "nothing uses it").
///   0.0.334 kept the variant on the append-only rule. It was not inert, though — it summed into
///   `ignore_def_bp`, so it was a SECOND NAME for a live stat, which is the shape that makes a
///   pool hard to read: it could not roll, could not be named, and showed as "Ignore DEF" beside
///   the real line. Gone from the enum, `value_of`/`with_value` in both `items.rs` and
///   `gear_roll.rs`, the weapon-slot assertion, `sim-host`'s item view, the two Godot label
///   tables and the `STAT_IGNORE_MDEF` string.
///   COST, stated plainly: `StatMod` is an externally-tagged serde enum, so a save carrying an
///   `IgnoreMdefBp` line no longer DESERIALIZES — it is a hard error, not a missing field. Only
///   pre-0.0.330 drops can carry one (the family stopped rolling then) and this branch has
///   already changed the sim past those saves. `MatkBp`/`MatkFlat` are the same shape and are
///   still here; `Hit`/`Flee`/`Str`..`Luk` are genuinely inert, read by nothing.
///   No behaviour change: seed 42 / 6000 ticks unchanged, no golden moves.
/// 0.0.336-alpha — THE REST OF THE RETIRED `StatMod` FAMILIES (game_data 0.0.98-alpha).
///   `Str`/`Agi`/`Vit`/`Int`/`Dex`/`Luk` (inert since 0.0.317), `Hit`/`Flee` (inert since
///   0.0.333) and `MatkBp`/`MatkFlat` (a second name for `AtkBp`/`AtkFlat` since 0.0.330) all
///   leave the enum — ten variants, the roll table, `value_of`/`with_value` in two files, the
///   sim-host tooltip vocabulary and both Godot label tables.
///   OLD SAVES STILL LOAD, which is the whole design of this cut and what `save_shape` enforces:
///   * `MatkFlat`/`MatkBp`/`IgnoreMdefBp` become serde ALIASES of `AtkFlat`/`AtkBp`/`IgnoreDefBp`
///     — the same fold that was happening at the consumption site, so no item loses value.
///   * `StatMod::Retired(i32)` is the sink for the eight that have no successor, with an alias
///     each. It reads, rolls and renders as nothing, and keeps the item's line COUNT (stars own
///     that). One inert variant instead of eight dead stat names spread over five tables.
///   REGALIA/EXEMPLAR: `RegaliaAxis` is deleted with them. Phys and Magic picked between
///   `AtkFlat`/`AtkBp` and `MatkFlat`/`MatkBp`, which have been the same two stats since 0.0.330
///   — an axis with one side. Every line now feeds the one offense pool; the ten `axis:` rows go.
///   CATALOG: eight card lines are repointed onto stats that exist rather than left blank —
///   `Flee` -> `DodgeBp` (10 -> 300, 15 -> 450, step 2/3 -> 60/90, bind 1 -> 30: dodge is the
///   system that replaced flee wholesale in 0.0.333), `MatkFlat` -> `AtkFlat` at the same number,
///   and Ashelle's `AtkFlat 5 + MatkFlat 5` merges to one `AtkFlat 10`. Yellowling's three
///   `Hit 3` star ranks had no successor, so they become `DrVsRanged 150`: it is an Armor-slot
///   evasion card and star ranks are its SECOND stat, the shape Twinling and Dustelle already use.
///   Godot: the character sheet's HIT/FLEE rows become one Dodge row (the FFI has carried
///   `dodge_bp` since 0.0.333 and nothing was reading it), and eight now-unreachable `AFFIX_*`
///   strings leave `strings.csv`.
///   MEASURED, seed 42 / 6000 ticks: 215 kills / 16 bosses / 1 wipe / 129 breaks — unchanged.
///   No golden moves.
/// 0.0.337-alpha — THE BUFFS AND FIELDS NOTHING READ (game_data 0.0.99-alpha). 0.0.336 emptied
///   the gear vocabulary; this empties the rest of the sim. A sweep for "written but never read"
///   found three classes, all cut:
///   (1) TWELVE `BuffKind`s that were still being GRANTED and read by nobody — `StrAdd`/`AgiAdd`/
///   `VitAdd`/`IntAdd`/`DexAdd`/`LukAdd`/`AllStatAdd`/`AgiPct`/`DexPct` (the attributes, 0.0.317),
///   `HitAdd`/`FleeAdd` (0.0.333) and `AgiSlowAdd` (an AGI delta). A player was paying for path
///   nodes and watching buff icons that moved no number. They collapse into one inert
///   `RetiredBuff` with an alias each, so old saves still deserialize.
///   Every skill that granted one is REPOINTED at the outcome its attribute used to buy, not at
///   a 1:1 replacement: Blessing = +1% party ATK/level; Agility Up = +3% ASPD +1%/level (the RO
///   curve) and keeps its +25% move speed; Guardian Aura's VIT half = +2% max HP +1%/level;
///   Gloria = +2% crit/level; Sharp Focus = +3% ASPD and +3% ATK, +1% each per level; Frost Bolt
///   slows with `AgiDexSlowPct` (the slow that still exists); Scarlette's Quick Step = +15% dodge.
///   TWO NEW BUFF STATS carry that: `CritBpAdd` (read in `effective_crit_chance_bp`) and
///   `DodgeBpAdd` (folded in `stat_mods::dodge_bp` PER BUFF, so a stack of dodge marks diminishes
///   like every other dodge source and never reaches 100%).
///   (2) SOUL PATHS: two arms were dead or duplicated. `MatkPct` fed the same bucket as `AtkPct`,
///   so the Magic arm WAS the Power arm — it is the CRIT arm now (`PathEffect::CritBp`). The
///   Precision arm was HIT/FLEE/AGI/DEX — it is the DODGE arm now (`PathEffect::DodgeBp`). The
///   remaining attribute nodes land on live neighbours: STR -> flat ATK, INT -> flat HP, VIT ->
///   flat DEF. Seven `PathEffect`s go.
///   A REAL BUG fell out of it: the percent nodes were authored as whole percent ("+3%") but the
///   aura channel stamps them into readers that take BASIS POINTS, so every %-node in the tree
///   had been delivering a hundredth of its documented value. The table is written in bp now —
///   `AtkPct` 3 -> 300 — which is the first time the Power/HP arms do what they say.
///   (3) DEAD FIELDS: `MonsterDef.view_range`/`level_add`/`scale_from_base` (declared, defaulted,
///   read by nothing — 16 catalog columns), `SkillDef.matk_flat_per_level`/`party_matk_per_level`
///   with their `Entity.aura_matk` channel and both `*_matk_bonus` functions (plumbing waiting
///   since 0.0.247 for a skill that never came, and a duplicate of the ATK twin since 0.0.330),
///   and the unused `SOURCE_BAND` constant.
///   MEASURED, seed 42 / 6000 ticks: 215 kills / 16 bosses / 1 wipe / 129 breaks — unchanged, and
///   no golden moves: the reference party carries none of the repointed skills and no path nodes.
///   Every repointed skill is a real buff now, so a party that DOES run an acolyte is stronger
///   than it was; that is the point, and it wants a balance pass once the kits are rewritten.
/// 0.0.338-alpha — ASTIER, TIDEWARDEN: Support x Water (game_data 0.0.101-alpha, roster §10).
///   Lucielle is Support x Holy and nothing else, so a party that wanted a healer had to bring
///   Holy with it — and break is gated on each monster's WEAKNESS LIST now, so an off-element
///   healer quietly costs the team an element slot. Astier is the second Support slot, which is
///   the element x Doctrine collection axis `docs/skill-system-redesign.md` §4.4.2 asks for.
///   HER KIT IS DELIBERATELY PLAIN (user 2026-09-08: "เอาง่ายๆพอตัวเริ่มต้น") — she is a unit a
///   player meets early, not one they have to learn:
///   * `Unda Vitae` (passive) — her basic is a ranged Water magic bolt at 50% ATK. Not flavour:
///     an acolyte-block Support has almost no ATK, so without it she walks into melee to poke.
///   * `Rivus` (cd 8 s) — a heal that BOUNCES: the ally worst off BY PERCENT, then the next two
///     at 70% and 50%. Percent and not missing HP, because missing HP always points at the
///     biggest health bar in the party. NO "is anybody hurt enough" gate, unlike
///     `dupla_medicina`: a heal that waits for a threshold spends its cooldown waiting for the
///     party to get hurt, and an idle party has nobody watching for that moment. The decay is
///     what keeps the overheal small — a gate would have been the wrong tool.
///   * `Vortex Undae` (cd 10 s) — a 5×5 Water magic burst, 72%+18%/level MATK. A Support that
///     can put nothing into a Toughness gauge is a slot the party cannot afford.
///   * `Fons Vitae` (ULTIMATE, gauge 100, no cooldown) — every living ally is restored by
///     20%+2%/level of ITS OWN max HP, so the one number reads the same on the 200-HP caster and
///     the 2000-HP tank. HELD while the party is whole: a gauge poured into full health is the
///     gauge wasted, and nobody is watching to hold it by hand.
///   PRIORITY: the ultimate is tried BEFORE Rivus (0.53 vs 0.54). The other way round, a cheap
///   heal that covers the damage means the gauge fills and is never spent.
///   STRUCTURE: `EXEMPLAR_LINES` was one row per CLASS FAMILY carrying a fixed `[SR, SSR]` pair,
///   which made a family the unit of content — an SR could not ship without an SSR placeholder
///   beside it. A row is a UNIT now with `rarity` as a field. `classes` stays, because a class is
///   still the base STAT BLOCK a hero stands on, not a role the player picks. The ladder test
///   moved with it: the weakest SSR must beat the strongest SR on every axis, checked across the
///   whole table instead of within a family.
///   MEASURED, seed 42 / 6000 ticks: 215 kills / 16 bosses / 1 wipe / 129 breaks — unchanged; the
///   reference party does not carry her. EVERY number in the kit is shaped, NOT measured: they
///   want a level-matched run against a Lucielle party before they are trusted.
/// 0.0.339-alpha — THE DEPLOYMENT GRACE IS 6 s, NOT 2 (user 2026-09-08: "เพิ่มความเร็ว กับ เพิ่ม
///   grace ให้ดูไม่แปลกเกินไป"). The grace exists so a party that has just been deployed is not
///   swung at while it is still walking in, and 2 s was sized for a band whose three rooms sat one
///   field width apart — that march took about a second and landed comfortably inside it.
///
///   Region 1's rooms are authored terrain 52 m apart. The renderer caps the march to the grace
///   (combat resuming mid-stride was rejected on 2026-09-03), so at 2 s the party crossed 52 m in
///   two seconds: 26 cells per second, a slide rather than a walk. The two honest ways out are a
///   longer window or a faster crossing, and this is both — 6 s of grace holding a 5.2 s march at
///   `ArenaView.MarchSpeedCps` = 10, with a settle beat left over. Walking it at the normal 3
///   cells/s would have been 17 s of nothing happening, which is worse in an idle game than a jog.
///
///   MOVED ONE GOLDEN: `diag_band4`. Expected and blessed — four extra seconds of nobody acting on
///   every zone entry changes the tick every subsequent roll is drawn on. Nothing else re-opens a
///   zone inside its window.
/// 0.0.340-alpha — THE GRACE IS 11 s, following the march down to a walking pace. 0.0.339 set it
///   to 6 s to hold a 5.2 s crossing at 10 cells/s; watching it, 10 was plainly too fast (user
///   2026-09-09: "pan ไวไป เดินไวไป"), so the march is 5 and the longest room takes 10.4 s. The
///   window has to hold the walk or the renderer clamps it back into a slide.
///
///   THE COST IS REAL AND WORTH NAMING: 11 s of nobody acting on every room entry, three times a
///   band. If that reads as dead air in play, the lever is the ROOM SPACING — 52 m for a 20-cell
///   field is what makes every one of these numbers awkward — not the grace.
///
///   Moved `diag_band4` again, same reason as 0.0.339 and blessed the same way.
/// 0.0.341-alpha — A BREAK NO LONGER RUNS STRAIGHT INTO THE NEXT ONE. `break_ticks` was the STUN
///   and the gauge refilled across it, so from the SECOND tick of a break the bar held a small
///   non-zero number — 5 of 100 — that any weakness hit (chip 40) emptied again. `chip_toughness`
///   guarded on `toughness == 0`, which covers the break tick and nothing after it, so a target
///   re-broke while it was still lying down (user 2026-09-09: "มีปัญหา break ติดกันสองครั้งได้
///   ทั้งๆที่ยังไม่ recovery").
///
///   The stun and the recovery are now two clocks. The stun is unchanged and still drives the Stun
///   and Vulnerability buffs — 20 ticks, 50 for a boss. `break_ticks` becomes the RECOVERY:
///   `BREAK_RECOVERY_TICKS` = 50 (5 s), never shorter than the stun, over which the gauge refills
///   and during which nothing chips it. The target is up and fighting again with its guard still
///   coming back, which is the window the yellow bar under its HP now shows.
///
///   MEASURED, seed 42 / 6000 ticks: breaks 111 -> 75, and kills 126, boss kills 10, wipes 5, level
///   4, XP 1409 and Gil 52 are all UNCHANGED — the 36 that vanish were re-breaks landing on a
///   target the volley was already killing. `golden_alpha6` moved and was blessed; it is the one
///   scenario where the removed breaks were doing work.
/// 0.0.342-alpha — A BREAK IS FIVE HITS, NOT THREE. `TOUGHNESS_CHIP_ADVANTAGE` 40 -> 20 against
///   the 100-point gauge every monster in the catalog carries (user 2026-09-09: "ตีทีเดียว break
///   ไม่ค่อยสมเหตุผล"). At three, one multi-hit skill did it in a single swing — every instance
///   chips separately — so the break was not something a party built toward.
///
///   THE GAUGE WAS NOT THE LEVER, which is worth recording because the obvious read is that band 1
///   is soft: all 67 monster entries carry `toughness_max: 100`, band 1 included, so raising it
///   "to 80" would have made the break EASIER. The chip is the only number that was wrong.
///
///   Skill multipliers untouched, so a break weapon stays one: Rapis (200%) takes 40 a hit, Steel
///   Break (300%) takes 60 — still the fastest route, no longer a one-shot.
///
///   MEASURED, seed 42 / 6000 ticks: breaks 75 -> 58 with kills 126, boss kills 10 and wipes 5 all
///   unchanged; the party ends slightly FURTHER along (b2_1 -> b2_3, XP 1409 -> 1422, Gil 52 -> 56).
/// 0.0.343-alpha — THE DEPLOY GRACE IS SIZED BY WHAT THE OPEN HAS TO COVER. It was one number,
///   110 ticks, chosen in 0.0.340 to hold the 52 m walk-through march at a walking pace. Every
///   other kind of open inherited it, and a RETRY is the worst case: the party is put down where
///   it stands, the renderer walks it in from just off the left edge in about a second, and then
///   nobody moves for ten more (user 2026-09-09: "ตายเริ่มใหม่ ... ยืนรอเกือบสิบวิ").
///
///   `ZoneSpawnState::deploy_grace_ticks` records which window the zone opened with.
///   `DEPLOY_GRACE_TICKS_MARCH` (110) is for a hop WITHIN an instance — the march. Everything else
///   gets `DEPLOY_GRACE_TICKS_OPEN` (20, which is Godot's original GRACE_SEC): a first room, a band
///   swap behind the fade, a farm loop and a retry all have only an entrance to cover.
///
///   The split is `is_instance_start`, already computed in `change_zone` for the per-session
///   resets. It lands exactly where the renderer's own split does — march within the scroll,
///   walk in from off the left edge otherwise — with no zone-id parsing of its own. Serialized
///   alongside `opened_at_tick` for the same reason: a save inside the window must resume inside
///   the SAME window. Old saves default to the long one, which is what they were recorded under.
///
///   MEASURED, seed 42 / 6000 ticks: kills 126 -> 139, boss kills 10 -> 11, XP 1422 -> 1656, wipes
///   5 unchanged. The party is not stronger — it gets back the seconds it spent standing still.
/// 0.0.344-alpha — POTENTIAL BOARD, skill-point prototype (user 2026-09-10).
///   Seraphine/Rapis fixed kits have a 17-node graph plus free origin. Quotes,
///   affordable routes, point debits and full paid-point refunds run in sim-core.
///   Existing learned ranks are credited; reset restores their baseline and keeps
///   cooldown history. Unauthored Basic/major effects cannot take points. Empty
///   board fields skip serialization; paid state parks with CharacterData. Skill
///   cap uses Job tiers (five levels each); Commander/material pricing is deferred.
///   Authored stat and major effects reach real combat. Iron Edge snapshots DEF
///   each second; Iron Wall uses normal tick aura lifetime; Cinderhide requires an
///   intact own Flame Shield. Shields now preserve source spell identity.
///   Legacy LevelSkill joins the ledger after board opt-in, avoiding reset loss.
///   Validation/golden changes are recorded in docs/potential-board-implementation.md.
/// 0.0.345: fixed Seraphine + Rapis new-party constructor, mirrored by verifier genesis.
/// Legacy save constructors are retained; existing accounts are not reset.
/// 0.0.346: fixed-kit characters receive authored rank-1 skills free, active skills slotted.
/// Shared load migration repairs existing/parked characters without resetting progression;
/// old board purchases refund only the formerly paid first rank, exactly once.
/// 0.0.347-alpha — THE STAT POINTS COME BACK, AS TWO TIERS OF APEX (design:
///   docs/apex-from-level-1-design.md). The attribute deletion left a level-up granting nothing;
///   this is the replacement, and it reuses Apex rather than inventing a second system, because
///   Apex's flat buckets are exactly what survived the attribute pipeline's removal.
///   TIER 1 (indices 0..APEX_CORE) — ATK / HP / DEF / Crit / Crit DMG / Dodge, the unconditional
///   stats every build wants, bought with `level_points` (new field, granted on level-up).
///   TIER 2 (APEX_CORE..) — DEF pen / Break DMG / Break Buildup / Cooldown / Cast / Charge / Heal,
///   build-flavour substats bought with the existing post-cap `apex_level`: that is what grinding
///   past the cap now pays, since points themselves arrive from level 1. Conditional stats
///   (DmgVs*/DrVs*/resists/MoveSpeed) are deliberately absent — an idle party cannot pick its
///   fights, so a point there is a trap; they stay gear-only.
///   MECHANISM: a bucket maps to a StatMod and allocation is emitted through `apex_mods()`, which
///   `stat_mods::for_each_mod` now yields alongside gear — one seam instead of six hand-written
///   `apex()` reads in derived.rs, which are gone. An unallocated unit emits nothing, so every
///   golden is byte-identical (39/39 unmoved). `ApexBonus` retires with them.
///   `APEX_BUCKET_CAP` 50 -> 20 (a maxed stat is +200 ATK/DEF, +4000 HP, +20% crit DMG, +2% crit);
///   per-point values move into the `APEX_PER_PT` table and are PLACEHOLDERS pending a sim-cli
///   pass. `reset_apex()` is a free respec of both tiers, available at any time (user).
///   MATK/HIT/FLEE/Soul buckets are retired: `effective_atk` has returned `effective_atk` since
///   0.0.330, accuracy became `dodge_bp`, and soul is party-shared, so a per-unit bucket read wrong.
///   SAVE SHAPE: `Stats` gains `level_points` (skip-if-zero) and `apex` widens 9 -> 13. A fixed-width
///   array would reject an old 9-wide save OUTRIGHT ("invalid length 9"), losing the whole file, so
///   `de_apex` reads any width: a legacy width REFUNDS the allocation (zeroed) and keeps
///   `apex_level`, because the old indices meant different stats (MATK/HIT/FLEE/Soul are retired),
///   the survivors changed tier, and the old ceiling was 50 against today's 20 — a remap would be
///   guessing how to spend someone's points. The refund costs nothing now that reset is free.
///   THE LEVEL-UP GRANTS ONE TIER-1 POINT (`POINTS_PER_LEVEL` = 1). Flat, because the per-bucket
///   cap paces the build and a flat grant reverses exactly — which is what the free respec and the
///   XP refund both need. At LEVEL_CAP 70 that is 69 points against the 120 it would take to max
///   all six core buckets, so the allocation is a choice rather than a formality. Goldens
///   diag_band4 / golden_alpha6 / golden_beta5 move for this and ONLY this: with the grant set to
///   zero all 39 pass unchanged, which is how the drift was confirmed to be the new field alone.
///   WIRED: both client screens show the two tiers (the core half is live from level 1; only the
///   substat half waits for the cap), `ApexProbe` asserts the new rule instead of the old
///   "locked below the cap", `debug_add_levels` grants points as a real level does, and sim-cli
///   spends them on a priority-fill build so a headless run stops measuring a character weaker
///   than any real one. LEFT: validating `APEX_PER_PT` against cap-20 in sim-cli.
/// 0.0.348-alpha — THE STAT-POINT MAGNITUDES ARE TUNED (docs/apex-from-level-1-design.md D3/D4).
///   0.0.347 shipped `APEX_PER_PT` as placeholders. Measured now against `--dump-stats 70` (base
///   stats per level) and the gear roll table, under ONE RULE: a bucket taken to its cap of 20 is
///   worth about one top-tier gear line — gear carries many lines and a player carries one
///   allocation, so a bucket that beat a fistful of lines would make gear pointless. Crit 10 -> 15
///   (= one T3 CritBp line), Dodge 20 -> 60 (= one T3 line), Break Buildup 100 -> 50 (its roll
///   window is narrow by design and a two-line bucket would hand the +50% ceiling over for
///   nothing), Cooldown 50 -> 40 (two T3 accessories are 2000 against the 3000 clamp; at 50/pt a
///   maxed bucket sat exactly ON the cap and any third source made it dead). ATK 10 and HP 100 stay
///   FLAT — a percentage was tried and measured, and rejected by the user as harder to tune; the
///   known cost is that base HP grows 40x from level 1 to the cap while ATK grows 4.4x, so a flat HP
///   point is worth far more early than late. HP is sized for the middle (11% of capped HP) and is
///   the bucket to watch. DEF 10 stays: it subtracts from a ~4500 hit at the cap, ~4% off at a full
///   bucket, and is deliberately the early-strong pick.
///   MEASURED in sim-cli, allocation on vs off, same seed: mid-game (Lv45 kitted, band 14) +4.7%
///   kills; early (three seeds from level 1) more XP in every seed; at the cap with a full kit no
///   change, because kills there are spawn-gated, not damage-gated. No golden moves: none allocates.
/// 0.0.349-alpha — XP REFUND, STAGE 1: THE FOUNDATION (docs/exemplar-unit-and-xp-refund-design.md,
///   re-integrated from feature/leveling onto the 0.0.347 stat model; plan in
///   docs/exemplar-leveling-reconciliation.md). Nothing a player can trigger yet.
///   `drain_base_xp(stats, level_cap, apex_cap)` — the body of `try_level_up`, extracted PURE on
///   `Stats` so combat and the coming apply path share ONE curve and cannot drift. Byte-identical:
///   with the new marks stubbed out all 39 goldens pass unchanged. `cum_base(stats)` — the
///   cumulative XP a reconciled unit represents (levels + Apex points + bar), what a refund hands
///   back. On GameState: `level_reached` / `apex_reached`, account-wide high-water marks raised ONLY
///   by combat, never by refund/apply, so pooling XP can never build a unit past the account's
///   proven peak (old saves default 0 → readers take max(mark, current max)); and
///   `owned_engrams: [u32; 4]`, the bound off-bag Engram reserve (1k/10k/100k/1M), credited only by a
///   refund. Goldens beta5 / beta8 / alpha6 / diag_band4 move: the four where a unit levels, so
///   `level_reached` leaves 0.
///   ASSUMPTION carried into stage 2 (differs from the branch doc, which predates the Potential
///   Board): refund touches the BASE stream only — job level is the board's cap and decision (b)
///   says refund leaves the board alone. Job stays untouched both ways.
///   STAGE 2 — THE EVENTS, all Client authority (they move only a unit's own provable XP):
///   `RefundUnitXp { unit_id }` drains a BENCHED unit's cumulative base XP into Engrams (greedy,
///   largest tier first; the sub-1k remainder is the one sink) and returns it to Lv1 — level, bar,
///   both Apex tiers' points and their allocation; job, skills, board, gear untouched.
///   `ApplyEngram { unit_id, tier, count }` pours stock into a benched unit through
///   `apply_base_xp_capped`, capped at the account's ceiling (`max(marks, current max across
///   units)`) with the ceiling bar DISCARDED — a parked bar would be a free level when the cap
///   rises. `ApplyEngramsToMax { unit_id }` fills to the ceiling with the waste-minimising plan;
///   `data::engram_fill_preview` computes that plan, the resulting level and the XP wasted WITHOUT
///   mutating, so a screen can warn and confirm, and the commit re-derives the same plan. Nine
///   tests pin the spine: no minting, the ceiling holds, the ceiling bar is empty, a stale mark
///   never caps a unit below one that stands higher, Measures never move, and a refund→fill round
///   trip is an exact ledger (unit + stock + the two sinks = what went in). No golden moves.
/// 0.0.350-alpha — EXEMPLAR == UNIT, the carrier flow retired (docs §2-§3; user 2026-09-11: option
///   (ก), and stars rise ONLY by the player's hand). An Exemplar is a roster UNIT, not an item worn
///   by a class character. `data::grant_exemplar_unit` is the one acquisition primitive: the first
///   copy the account receives becomes a benched Lv1 unit — its class the Exemplar's own fixed-kit
///   `unit` when one exists (seraphine), else the first class of its line — carrying
///   `exemplar: Some` at 0★ so the star ranks layer on through the existing overlay path; every
///   later copy banks `pending_resonance[bare_id]` (new GameState field, skip-if-empty), a bound
///   counter no bag or market can see. `FeedExemplarUnit { unit_id }` (Client) spends one credit
///   for +1★ on the bench or in the field; a dupe never auto-feeds, a credit past the cap stays
///   banked. `DebugGrantExemplarUnit` is the dev/story stand-in. DELIVERY: `bank_collectible` takes
///   the catalog and routes an Exemplar envelope (mail, def-id grant, the shop's debug row) into
///   the grant; the envelope is never kept. `try_grant_bag_item` is the catalog-less bag path and
///   refuses a collectible. RETIRED: `equip_exemplar` / `unequip_exemplar` / `feed_exemplar` /
///   `sync_member_exemplar`; `EquipExemplar` / `FeedExemplar` / `UnequipExemplar` are tombstones
///   that parse and do nothing. MIGRATION in `migrate_save_items`: each legacy `owned_exemplars`
///   carrier becomes what the unit model says it is — a WORN carrier's wearer already IS the unit
///   and takes the carrier's stars; an unworn one is granted (unit or credit) and its fed stars
///   become credits; the list empties, so the pass is idempotent. `owned_exemplars` stays as a
///   legacy field for deserialization and is always empty after load. Regalia is unchanged — still
///   a carrier with its own slot. No golden moves (none holds an Exemplar).
/// 0.0.351-alpha — POTENTIAL IS BOUGHT WITH SHARDS, GATED BY BASE LEVEL, AND NEVER RESET
///   (docs/potential-upgrade-design.md; user 2026-09-11). The Job-tier gate and the skill-point
///   prototype are gone from the board. `potential::level_tier(level)` is the one cap: every node
///   may reach rank 5 from level 1, and past 50 the cap is `level / 10` (Lv70 → 7, Lv100 → 10), so
///   the +10-per-update level cap unlocks a rank per update by itself. `TierCaps.level` replaces
///   `.job`, `LockReason::LevelTier` replaces `JobTier`. Cost: rank n = n Potential Shards of the
///   grade for its band (1-3 → I, 4-6 → II, 7-10 → III) — three new bound Materials
///   (`potential_shard_1..3`, catalog 0.0.102). Purchase, the batch preview (`MaxPotential`) and
///   the host view run one `Purse` snapshotted from the bag, and a commit debits the bag by
///   exactly what the purse spent, so what a screen quotes is what is charged. `ResetPotential`
///   is a tombstone; `reset_reason` says so. Job level still exists in the sim and still mints
///   skill points nothing consumes — its removal is the next, separate step. No golden moves.
///   DROPS (catalog 0.0.103): every monster with a drop table gains one shard row — grade by
///   its level (≤25 → I, 26-50 → II, 51+ → III), weighted to 3% of a kill, 25% of a boss.
///   PLACEHOLDER rates, tuned in play. golden_alpha6 and diag_band4 move: the two scenarios that
///   roll the catalog's real tables, so the added row shifts the draw.
/// 0.0.352-alpha — CLASS AND JOB ARE GONE (user 2026-09-12: "class job ทิ้งหมดแล้ว"). A unit is its
///   fixed kit and its Potential; there is no Job level, no Job XP, no skill points, no class tier
///   and nothing to advance into. `Stats` loses `job_level` / `job_xp` / `skill_points`; monsters
///   lose `job_xp_reward` / `job_xp_mult_bp` and the level table its `job_xp` column (catalog
///   0.0.104); `UnitDef.advances_to` is gone and `creatable_units` is every stat block.
///   The job XP stream, `try_job_level_up`, `job_xp_to_next`, `JOB_LEVEL_CAP`, `advance_class`
///   and the Job-tier skill-tree gate are deleted; `AdvanceClass` and `LevelSkill`-on-a-tree-skill
///   are tombstones (a skill node still routes `LevelSkill` to the Potential board). The base kit
///   no longer refunds points it cannot hand out. Debug: `debug_max_skills` replaces
///   `debug_max_job_and_skills`; `debug_advance` / `debug_add_skill_points` are gone. `unit`
///   defs STAY as the stat block a unit is built from (seraphine / rapis are entries; an Exemplar
///   line's first class is its base) — they are no longer an identity or a progression.
///   Every golden that serialises a player's Stats moves (the three fields were never
///   skip-if-zero); golden_beta8 — the Job-level scenario — is removed with the mechanic.
///   Hosts: the roster row and the `advances` query lose Job (the latter answers []). Godot: the
///   Skill Tree (K) and Advancement (V) screens, the character panel's Job bar and the wrappers are
///   removed; Unity: AdvanceWindow and its bar entry. sim-cli: `auto_potential` spends looted
///   shards where `auto_learn_skills` spent Job points; `auto_advance` and `--no-advance` are gone.
///   NAMING (same version, catalog 0.0.105): the template is called what it is. Catalog `units`
///   (was `player_classes`), `UnitDef` (was `PlayerClassDef`), entity/roster/event field
///   `unit_def` (was `class_id`), `GameState.current_unit_def`, `data.unit(id)` /
///   `creatable_units()`, `doctrine_for_unit`, `unit_kit_tree`, `ZoneDef.unit`, host query
///   `"units"`. `unit_id` (u32) stays the roster/entity id; `unit_def` (String) is the template.
///   Serde aliases keep old saves and catalogs readable, so the machine's real saves still load.
///   Twelve goldens re-pin on the key rename alone (lib tests unmoved).
/// 0.0.353-alpha — POTENTIAL v2: the upgrade ladder. Skill nodes rank to `SKILL_RANK_CAP` = 20
///   (`SOFT_CAP` = 12 is where the ladder turns steep); a rank past the skill's own `max_level`
///   is an OVERRANK worth `OVERRANK_BP_PER_RANK` (+2%) on `power_at`, the one hook every skill
///   shares. Four material families replace the Potential Shard (catalog 0.0.106/0.0.107):
///   SIGIL in three grades, LOCKED TO THE UNIT'S ELEMENT (user 2026-09-12: "ตาม element ไม่ใช่
///   doctrine"; `sigil_<element>_<grade>`, ten elements incl. Spirit); ESSENCE in three grades;
///   BOSS CORE (server-rolled, bosses, 20%) and TRACK (server-rolled, band ≥ 10 / Lv46+ bosses,
///   2%). `node_cost(kind, next_rank)` is the 19-row ladder (rank 2 = Sigil I ×2 + Essence I ×2
///   + 1k Gil … rank 12 = III ×8 + III ×8 + 3 Core + 1 Track + 160k … rank 20 = III ×30 + III ×30
///   + 12 Core + 4 Track + 1M Gil); stat and major nodes cost Sigil I ×3 + 1k Gil. Gil is spent
///   through `GilSpend::Potential`. `Purse` is sigil[element][grade] + essence + core + track +
///   gil; `can_pay`/`pay` take the unit's element. `LevelTier` and the level gate are gone;
///   `required_tier` survives on Majors only. Everything is tradeable (marketable, quota
///   tradeable, emission group `potential`). Drops: every monster rolls one Sigil row per
///   element (family 2% of a kill, 15% of a boss) and an Essence row (4% / 30%); the five
///   small drop tables were scaled ×10 so the family lands on its share instead of the
///   weight-1 floor. Hosts: the board query answers `cap`, `soft_cap`, `purse {element, sigil[3],
///   essence[3], core, track, gil}` and per-node `cost {sigil_grade, sigil, essence_grade,
///   essence, core, track, gil}`; `shards`/`shard_defs`/`skill_point_cost`/`cost_grade`/
///   `cost_count` are gone. Unity PotentialWindow reads the purse and the recipe. golden_alpha6
///   and diag_band4 move on the catalog alone (verified: the HEAD catalog reproduces the HEAD
///   pins); no code-driven golden moves. Daily/soft caps on acquisition are deferred by the user
///   ("ค่อยคิดระบบ cap ทำระบบอัพเกรดก่อน").
/// 0.0.354-alpha — POTENTIAL MATERIALS ARE A LEDGER, NOT BAG ITEMS (user 2026-09-12: "sigil
///   ไม่ต้องรวมใน inventory เพราะจะรก แต่ต้องทำให้แลกเปลี่ยนได้"). `GameState.materials` is a private
///   `def_id → count` map with `credit_material` / `debit_material` / `material_count` verbs and a
///   `MaterialSource` on every credit; no slot, no cap, never bound, party-shared like Gil. The
///   35 defs (Sigil ×30, Essence ×3, Boss Core, Track) are `ItemKind::PotentialMaterial` (catalog
///   0.0.108) — stackable for the market's purposes, but `bank_to_inventory_inner` credits the
///   ledger and consumes the item, `can_bank` always says yes (a mail delivery is never blocked
///   by a full bag), and `count_stackable_unbound` / `consume_n_stackable_unbound` read the
///   ledger first so `DepositCommodity` lists Sigils exactly like a bag stack. `Purse` reads the
///   ledger. Load migration moves a pre-ledger save's bag/rare-bag stacks of these defs into the
///   ledger. Hosts: query `materials` answers item-view rows (quantity = the count); the Unity
///   trade window reads `inventory` + `materials` for held stock. Godot's market views accept the
///   kind on the buy side only — Godot has no Potential surface, so listing from the ledger there
///   is not built. The shape is deliberately what a Steam Inventory itemdef mint wants (fungible
///   id × qty); nothing is built toward that yet. golden_alpha6 / diag_band4 move (their Sigil
///   drops now serialize as ledger counts instead of bag stacks); no other golden rolls one.
/// 0.0.355-alpha — POTENTIAL LOOT TABLE BY MONSTER LEVEL (user 2026-09-12: "ควรทำ loot table
///   ตามเลเวลมอนสเตอร์"). The 11 Sigil/Essence rows on every monster are gone (catalog 0.0.109
///   restores the pre-0.0.107 tables; the Core/Track server rows on bosses stay). In their place
///   `potential::LOOT_TIERS` — three rows keyed by the dying monster's level (≤25 → I, 26-50 →
///   II, 51+ → III) with absolute per-kill odds, field and boss — and `roll_potential_material`
///   in `drop_on_death`: ONE extra PRNG draw per kill, after the weighted item roll and before the
///   conditional foil draw, on every kill regardless of outcome. The Sigil is the monster's FIRST
///   WEAKNESS (bring the unit that hits it hardest, take home its material); a monster with no
///   weakness list pays Essence instead. Absolute odds never dilute the weighted table (the
///   0.0.107 rows shifted every other share — `recipe_supply` caught it). Not scaled by the
///   level-gap penalty: grade already follows the monster's level, and a capped unit needs
///   grade I for every stat node. Rates are still the placeholder pass (2% / 4%, boss 15% /
///   30%). The six goldens/diags that kill monsters re-pin (alpha3, alpha4, alpha6, beta4, beta5,
///   diag_band4): one more draw per kill moves their streams. Data pass to give
///   each element a primary-weakness monster in every level tier is the next step.
/// 0.0.356-alpha — THE PLAYER CHART IS SIX ELEMENTS (user 2026-09-13: "คงต้องเอาออก").
///   Earth / Poison / Thunder / Spirit came in with the 2026-06 matchup matrix and the gothic
///   pack; since the weakness-list pivot they are MONSTER attack elements only (Earth: 20
///   monsters, Spirit 2, Poison 1, Thunder none, ever). `potential::ELEMENTS` is now the chart —
///   Neutral / Fire / Water / Wind / Holy / Shadow (`CHART` = 6, `in_chart`); `Purse.sigil` is
///   6-wide; a monster whose first weakness is outside the chart pays Essence instead of a Sigil.
///   Catalog 0.0.110 drops the twelve dead Sigil defs and `quiver_earth` (an Earth attack key in
///   the player's hands, which docs/element-weakness-system.md §4 already retires). The gear-roll
///   element pool loses Thunder — no monster has ever carried it, so the "+dmg vs Thunder" and
///   "resist Thunder" lines were dead on every roll; Earth and Poison stay in that pool because
///   they are monster-facing. The `Element` enum is untouched (append-only, serialized in saves
///   and data); legacy class skills that cast Earth / Thunder / Spirit are untouched (legacy
///   units, not in the chart). No golden moves (none of the removed defs is in a drop table, and
///   no golden rolls a Thunder line).
/// 0.0.357-alpha — THUNDER TAKES WIND'S SEAT IN THE CHART (user 2026-09-13: "สลับลม เป็นสายฟ้า
///   ดีกว่า"), and the gear-roll element pool is the chart and nothing else ("gear roll เฉพาะ
///   ธาตุหลัก" — a "+dmg vs Earth" line is a niche nobody builds around). `potential::ELEMENTS` =
///   Neutral / Fire / Water / Thunder / Holy / Shadow; catalog 0.0.111 renames `sigil_wind_*` to
///   `sigil_thunder_*` and retags the 24 monsters weak to Wind as weak to Thunder (the lists are
///   baked into spawned entities, so golden_alpha6 / diag_band4 re-pin; no unit or live skill
///   was Wind, so no damage changes). `gear_roll::ELEMENTS` = Fire / Water / Thunder / Holy /
///   Shadow. Wind joins Earth / Poison / Spirit as enum-only. The legacy Elementalist Wind cast
///   is untouched (legacy unit). SAME VERSION, follow-up: the gear pool is the WHOLE chart,
///   Physical included (user: "ฟิสิก =/= ไร้ธาตุ") — `gear_roll::ELEMENTS` = `potential::ELEMENTS`;
///   "Honed" / "of Warding" were already in strings.csv.
/// 0.0.358-alpha — WEAKNESS ORDER DATA PASS (catalog 0.0.112): the first hand-tune of the 67
///   weakness lists, now that the first entry is the Sigil a monster drops. 21 monsters reordered
///   and `baby_pinkling` gains Fire, so that every chart element is the first weakness of a FIELD
///   monster in every grade tier (18 cells) and all six appear in the zones up to level 20 (user
///   2026-09-13: "ควรจะครบ 6 ธาตุภายใน area 20" — the element areas, not yet built, unlock at
///   Lv20 and take over as the main Sigil source). Area 1 now pays Physical + Fire, the two
///   starters. Both rules are ratchets in `tests/suite/potential_coverage.rs`. Combat: only the
///   one added weakness changes a multiplier; order never did. golden_alpha6 / diag_band4 re-pin
///   on the baked lists.
/// 0.0.359-alpha — THE PLAYER SIDE HAS NO WEAKNESS (docs/element-weakness-system.md §5, decided
///   2026-09-07, reaffirmed 2026-09-12). `element_mult_vs` answers 100% for any Player-team target;
///   a frozen player is not a Water target either. Until now a unit had no weakness list and so
///   fell through to the retired 150/50 matrix like any listless monster — Rapis (Fire) took 150%
///   from Water and 50% from Earth, i.e. half damage through areas 1-3 and half again as much in
///   area 4. Defence against an element is now `resist_element_bp` (gear / card / meal RES%) and
///   the element's debuff, nothing else. Monsters are untouched. diag_acolyte re-pins (its
///   player is elemental); the goldens' parties are all Physical, which the matrix never touched.
/// 0.0.360-alpha — ESSENCE IS CUT (user 2026-09-13: "ตัด Essence ออกก่อน ใช้แค่ Sigil"). The basic
///   material is the element Sigil alone; `Cost`, `Purse`, the ladder and `LootTier` lose their
///   Essence columns (Sigil counts unchanged), the three defs leave the catalog (0.0.113), and a
///   kill whose monster has no in-chart first weakness now yields nothing from the Potential draw
///   (the draw is still taken, so the stream is unchanged). Hosts drop `purse.essence` and
///   `cost.essence*`; the Unity window and two loc rows follow. golden_alpha4 / alpha6 /
///   diag_band4 re-pin — their kills used to credit Essence.
/// 0.0.361-alpha — SIGIL RATES CUT ~10× (user 2026-09-13: "ได้เยอะไปมาก ต้องลด"). `LOOT_TIERS`:
///   grade I 20 bp (boss 150), II 8 bp (60), III 3 bp (25) — derived from the ladder's consumption
///   against ~30k kills / ~2k boss kills a day (grade III ≈ 9/day ≈ rank 12 in ~10 days). A 24 h
///   run now pays ~6 Physical I and ~18 Fire I instead of ~46 / ~198, and Gil stops being the
///   binding constraint (20k left, was 696). golden_alpha4 / alpha6 / diag_band4 re-pin.
/// 0.0.362-alpha — TIERED NODES, EVERY-GRADE DROPS, SIGIL CONVERSION (user 2026-09-13; target:
///   soft cap 12 on one unit in 3-4 weeks solo). Stat and Major nodes carry an authored `tier`
///   priced by `NODE_TIER_COST` (T1 I ×3 + 1k … T4 III ×6 + Core 2 + Track 1 + 60k; Majors M1 /
///   M2 above) — "stat เต็มไวไป ต้องมี node ยากด้วย". A zone's kill pays every grade up to its own
///   (`GRADE_BP` per grade, highest checked first in the same single draw) after a 7-day run
///   showed both starters stuck on day 1 with 0 grade I and a pile of unusable II. `MergeSigil` /
///   `SplitSigil` (Client, self-cost, no PRNG) convert 3:1 both ways, lossless (`SIGIL_MERGE`);
///   `MaterialSource::Conversion`. Catalog 0.0.114: the Boss Core / Track server rows are 20 / 3
///   bp (were 2000 / 200 placeholders). Hosts unchanged in shape; Unity gains four conversion
///   buttons; sim-cli converts toward affordability and prints the Core / Track expected value.
///   No golden moves (grade-I zones draw exactly as before).
/// 0.0.363-alpha — HALF THE SIGIL ODDS, DOUBLE EVERY SIGIL REQUIREMENT (user 2026-09-13: "ลด drop
///   rate 50% + เพิ่ม requirement ×2" after a 21-day sim-cli run at 20 / 8 / 3 bp minted 292% of
///   one unit's whole board pooled across elements — own-element Rapis 68%, Seraphine 29%, a
///   zone-choice gap the Lv20 element areas will close). `GRADE_BP` = 10 / 4 / 2 (III rounded up
///   from 1.5), `BOSS_GRADE_BP` = 75 / 30 / 12; the ladder's Sigil column and every node tier's
///   Sigil count ×2 (rank 2 = I ×4 … rank 12 = III ×16 … rank 20 = III ×60; T1 I ×6 … M2 III
///   ×16). Core / Track / Gil untouched — their expected value (55 / 8 of 60 / 10 in 21 days)
///   was on target. ~4× slower in all: pooled ≈ 75% at three weeks, i.e. the 3-4 week target.
///   No golden moved (none of their kills drew inside the halved range).
/// 0.0.364-alpha — LOOT TABLE CLEAN-UP, PASS 1 (user 2026-09-13: "จัดการตามที่บอกข้างบน").
///   (1) `ItemDef::ledger` / `Item::ledger` replaces `ItemKind::PotentialMaterial`: kind says
///   what a thing IS, ledger says where it is KEPT. Sigil / Boss Core / Track go back to kind
///   `Material`; the STAR STONES join the ledger (user: "star stone ก็ดรอปเข้าแบบ sigil เลยจะได้
///   ไม่รก") — drops, mail and load migration route on the flag. (2) `GameData::loot_rules`
///   (catalog 0.0.115): the Star Stone rows (by field / boss) and the Boss Core / Track rows are
///   GENERATED into the drop table at spawn by `build_drop_table` instead of copied onto 67
///   monsters — 328 rows leave `game_data.json` (2,449 → 2,121). Every field monster now carries
///   the field stones (the tier-II gothic monsters had none — an omission, not a rule); the
///   class Rune Stones no longer drop anywhere (retired classes; still shop-bought and usable).
///   (3) `UseOrb { orb: <def id> }` replaces the bag index; `orbs::orb_effect_of` is the sim's
///   stone table, pinned to the catalog by `star_stone_effects_match_the_catalog`. Hosts:
///   `orbs` / `orb_plan:<id>` / `orb_fits` answer by def id; the Godot bridge gains
///   `materials_json` and `orb_check_json(orb, …)`; both Enchant screens list stones from the
///   ledger. `potential::is_sigil_def` is the typed predicate sim-cli tallies with. The real-save
///   shape test lets a ledger def leave the bag into the ledger. golden_alpha6 / diag_band4
///   re-pin (the rule rows sit at the end of every table).
/// 0.0.365-alpha — LOOT TABLE CLEAN-UP, PASS 2 (user 2026-09-13: "ok ได้"). (1) `loot_rules.gear`
///   (catalog 0.0.116): the gear pool is a RULE — a monster's gear tier from its level (`bands`,
///   an unset def level takes the zone's), every non-unique Weapon / Armor def of that tier at
///   absolute odds (field 2 bp plain / 2 bp 1-slot; boss 3 / 3 / 3 incl. 2-slot; daggers ×2 —
///   the odds the 60 of 62 authored tables agreed on), converted to weights taken out of the
///   table's `nothing` so every authored row keeps its exact odds (a hundred-weight table is
///   scaled up first). 1,846 copied gear rows leave `game_data.json` (2,121 → 275). Behaviour
///   changes, all on purpose: burst_chom / rechum (2-4 pieces) and the gothic field monsters
///   (mini_ghost, ghost_rabbit, red_hood, red_hood_crimson, bat — none at all) now carry their
///   band's full pool; the near-zero 1-slot odds on the million-weight tables are normalised to
///   2 bp. `exclude` on the rule is the authored opt-out. (2) ORE and the ore shards are LEDGER
///   materials (user: "ore เข้า ledger ได้ แต่ต้องแลกเปลี่ยนได้ด้วย" — the ore defs were already
///   marketable): `count_stackable` / `consume_n_stackable_bound_first` read the ledger after
///   bound stacks and before unbound ones, so refine and craft see one stock; the host refine
///   plan and the Godot refine view count it. (3) Ratchet: every non-unique gear def is
///   reachable from some spawned table. The bag-pace and rare-bag tests read spawned tables.
///   golden_alpha6 / diag_band4 re-pin.
/// 0.0.366-alpha — ANY UNIT EQUIPS ANY WEAPON, and weapon familiarity is gone
///   (catalog 0.0.117). With classes retired, a weapon whitelist was the class system under a
///   new name — and a fungible market wants every buyer to be every player. (1) The equip gate
///   is deleted: `Item::can_be_equipped_by`, `Item`/`ItemDef.equip_classes` and the
///   `EquipItem` class check all go; `items::default_weapon_classes` survives ONLY as the
///   eligibility table for the five retired-class rune stones (`orbs::orb_applies`) and dies
///   with them. Six dead class-locked defs leave the catalog (arrow_1..3, quiver_fire/water,
///   demo_socket_bow — unreferenced by any table or view; the `LoadAmmo` machinery stays).
///   (2) `aspd_mods` (the iRO per-weapon-type familiarity table) is retired from `UnitDef` and
///   `Entity`, with its −15 unlisted-type fallback: ASPD = the unit's own `base_aspd` + item
///   `aspd_base` + buffs, which is what `skill-system-redesign.md` §4.5.1 always said. To keep
///   the change from INFLATING swing rates, each unit's `base_aspd` absorbs the mod of its
///   STARTING weapon (seraphine 160→153, rapis 151→146, …): every unit swings exactly as it did
///   with its own weapon, another type is no longer slower, and nothing anywhere is faster than
///   its old best (mage_t1/rapis lose the 151 a dagger used to give — the only downward case).
///   SCHEMA: `Entity.aspd_mods` and `Item.equip_classes` removed. Goldens re-pin.
/// 0.0.367-alpha — ATTACK SPEED is attacks/sec, the LoL/Dota way (catalog 0.0.119). The RO
///   ASPD-point model is deleted: `rate = base_rate × (1 + Σatk_speed_bp) × (1 − slow)`, clamped to
///   [0.20, 10.00] atk/s, ONE path for players and monsters (they already carried a flat rate
///   and the old code returned early for them — which is why `AspdPct` pack-buffs on MONSTERS
///   were silently INERT: Ashelle's +30% and the dancer's +30% never applied to anything.
///   They work now). `UnitDef.base_aspd` and the 146-207 scale are gone; a unit's
///   `atk_speed_milli` IS its rate (milli-swings per tick, `/100` = atk/s), converted through
///   the old curve so every unit swings exactly as before: seraphine 1.06, rapis 0.92,
///   monsters ~0.70, and the 0.0.366 guard test now pins those. `Item::aspd_base` (ASPD points)
///   becomes `atk_speed_bp` (bp, 500 = +5%) — uniques carry +2%/+5%. The bonus is ADDITIVE across every
///   source (item lines, `AspdPct`, `AspdPotionPct`, `CardAspdStack`) and then multiplies the
///   base once, so "+30%" is +30% of the rate on every unit — the old fold
///   (`200 − (200−ASPD)·(1−pct)`) turned +30% into +43% and made one affix worth different
///   amounts on different units, which a fungible market cannot price. Shield −5 ASPD →
///   **−10%** and dual-wield −10 ASPD → **−18%** (the percentages those flats worked out to);
///   `AspdSlowPct` stays a FINAL multiplier so a slow bites a sped-up target hardest. Bounds are
///   written down: the bonus caps at +200% (`ATK_SPEED_BONUS_CAP_BP` — the balance dial; the word
///   "Haste" is left free for COOLDOWN reduction, which is what the design reserves it for), the rate clamp is
///   just the tick's physical ceiling (1000 milli = one swing per tick), and a base rate of 0
///   still means "no attack authored" (it must NOT rise to the floor — that broke 30 skill
///   tests before the early return went back in). The stats payload renames `aspd` (0-207) to
///   `atk_speed_x100`; Godot and Unity now show "1.06 /s". Deleted: `ASPD_CENTI_CAP`,
///   `ASPD_KNEE_CENTI`, `ASPD_CENTI_FLOOR`, the hyperbola + knee, and the ASPD-inversion both
///   hosts kept for display. SCHEMA: `Entity.base_aspd` out, `Item.aspd_base` → `atk_speed_bp`.
///   Goldens re-pin. `aspd-table-iro.md` stays as the reference the base numbers came from.
/// 0.0.368-alpha — WEAPONS ARE GATED PER UNIT again (user 2026-09-14: "Seraphine เอาดาบอย่างเดียว
///   / rapis เอาแค่คทา / acolyte ถือ staff"). ONE STRICT FAMILY PER CHARACTER, keyed by the five
///   archetypes: sword line [Sword1H], mage AND acolyte lines [Staff], thief [Dagger], archer
///   [Bow] — so seraphine holds a 1H sword and rapis a staff, and the permissive RO matrix
///   (swordsman could hold dagger/mace/2H) is gone. **THE EXEMPLAR DECIDES, not the stat block**
///   (user: "ค่านี้มันอยู่ใน exemplar ไหม" — it was not, and that was the hole): an Exemplar IS
///   the character, so `weapon_types_for_character` resolves the worn Exemplar through its own
///   `EXEMPLAR_LINES.classes` first and falls back to the unit only when none is worn. That
///   revives `classes`, which had been dead data since classes were retired — `exemplar_archetype`
///   is its first reader. A Lucielle on a swordsman block keeps her staff. NOT keyed on Doctrine:
///   a Doctrine says how a unit FIGHTS, not what it holds (rapis is a Striker with a staff).
///   An id the table never heard of is unrestricted, so hand-built test entities still equip.
///   It stays a CODE table because
///   `apply_input_with` takes `data: Option<&GameData>` — an equip rule that read the catalog
///   would decide differently for a caller that passes None, and the replay would diverge.
///   Enforced in `equip_item`, `build::party` auto-gear and sim-cli's auto-equip/auto-sell (all
///   four read the worn Exemplar); the rune-stone eligibility table is now this same one, so the
///   five retired-class stones narrow with it — they are slated to leave the catalog anyway.
///   DROPS ARE NOT SCREENED (user: "ทำดรอบรวมๆไปก่อนก็ได้ ไม่ต้องสกรีนดรอบออก"): the gear pool
///   stays the whole tier, so a parked family (Bow / Dagger / Mace / Sword2H — 88 defs no unit
///   wields today) still drops as sellable gear. Filtering the pool by the gate is a one-line
///   change in `build_drop_table` if that starts to feel like junk. Two new guards tie the code
///   table to the catalog roster:
///   a creatable unit with no declared family fails the build, as does a starting weapon outside
///   its own family. ASPD does NOT come back with it — familiarity stays retired (0.0.367), so
///   the gate decides what you may hold, never how fast you swing it. No golden moved: every
///   equip a golden performs was already family-legal and the drop pool is unchanged.
/// 0.0.369-alpha — THE DELETION PASS: skill RUNES leave the game, and raw gear stops being
///   ROLLED (catalog 0.0.120). Two removals in one patch because both delete PRNG draws, so the
///   goldens re-pin once. Spec: `docs/gear-fusion-and-base-fix-design.md` (D1); the rune
///   post-mortem + its 12 effects as an idea bank: `docs/skill-rune-design.md`.
///
///   (1) RUNES, ENTIRELY. They augmented skills of the RETIRED classes (`twin_strafe`,
///   `arrow_shower`, `bolt`) — no creatable unit has any of them, so the whole system was
///   unreachable: 12 `RuneLine`s, 13 blocks in the combat path, 111 test references, zero users.
///   Gone: `SkillRune`, `RuneLine`, `Item.skill_rune`, `RUNE_REFINE_THRESHOLDS`,
///   `FLURRY_POWER_BP`, `Entity::weapon_rune{,_at}`, `gear_roll::{rune_lines_for,
///   roll_skill_rune,insert_rune,has_insertable_rune}`, `OrbEffect::InsertRune`, the five
///   `rune_stone_*` defs, the rune tooltip keys in `item_view`, and the rune display in
///   `item_desc.gd` / `enchant_view.gd`. Behaviour deleted with them: Overload extra bolts,
///   Barrage/Flurry charge-vs-cooldown, Cleave splash, Cinders/Concussion on ignite_break,
///   Execute, Vulnerable/BloodMist, Fusillade, Focus/Pinning, Renewal/Retribution on heal,
///   Benediction, Scattershot/Radiance/Arc chains. `apply_combustion_burn` STAYS — a real skill
///   still calls it. `combat_step/mod.rs` −317 lines; `tick/tests/skills.rs` 2,002 → 1,576.
///
///   (2) D1 — GEAR IS FIXED-STAT. Nothing is rolled on a gear drop OR a craft, by the client or
///   the server: a piece is (type × tier) as authored, and depth moves to what the PLAYER adds
///   later (gem / card / fusion). The stage-3 server-roll detour existed only to stop a modified
///   client minting value out of ★/lines/runes; with none of those rolled there is nothing to
///   protect, so `accrue_gear_candidate`, `roll_gear_star{,_public}`, `star_eligible` and
///   `roll_craft_star`/`STAR_WEIGHTS_CRAFT` are gone and the piece banks at the moment of death.
///   `GrantGear`/`GrantCraft` remain (the nonce plumbing still serves cards/materials, and a
///   save mid-flight may hold a pending candidate) but now just bank the authored item.
///   `star_rarity` stays — enchant, migration and the craft display still use it, and P1 will
///   feed it from GRADE.
///
///   (3) STAR STONES PARKED, not deleted (user: "star stone ก็พักไว้ก่อน"): `orbs.rs` and the 8
///   defs stay with their tests green, but `loot_rules.orbs` is emptied and `buy_price` is 0 —
///   they neither drop nor sell. Re-arm by restoring the rule rows. Same treatment as the Gil AH.
///
///   SCHEMA: `Item.skill_rune` removed. Goldens re-pin. `Item.orb` now deserializes through
///   `orb_lenient` — a retired variant in an old save (a real save on this machine holds a
///   `rune_stone` whose orb serialised `InsertRune`) loads as `None` instead of failing the whole
///   file. Orbs are def-driven consumables, so an unknown one is inert; every other malformed
///   shape still errors, so `save_shape` keeps its teeth.
/// 0.0.370-alpha — REFINE IS GONE (catalog 0.0.121). The +N gear ladder retires with the tier
///   system that replaces it: once a piece's power is its GRADE, nobody wants to sink ore into a
///   piece they are about to fuse away (user 2026-09-15: "ตีบวกก็คงต้องลบด้วย เพราะระบบ tier
///   คนก็ไม่อยากตีบวกจนกว่าจะได้ของดีสุด"). Spec: `docs/gear-fusion-and-base-fix-design.md`.
///
///   SIM: `refine.rs` deleted whole (1,190 lines) — odds tables, seals, coupons, the Enriched
///   ore, `attempt_refine{,_inventory}`, `reforge_item`, the ATK/MATK/DEF refine terms. The
///   stackable-materials helpers it happened to host (`count_stackable`,
///   `consume_n_stackable_bound_first` and friends) MOVED to the new `materials.rs`: refine was
///   only their first caller, crafting and the market still need them.
///
///   SCHEMA: `Item.refine_level`, `Item.refine_attempts` and `Item.broken` are gone (serde
///   ignores them in an old save, so a save still loads — the gear just stops carrying its +N);
///   `InputEvent::{StartRefine, StartRefineInventory, ReforgeItem}` are gone (no real save on
///   this machine holds one); `Measures::{refine_attempts, refine_successes}` and
///   `GilSpend::{Refine, Reforge}` are gone. Every golden re-pins.
///
///   CATALOG: the 10 refine defs leave (`ore_valkirion`/`ore_aegirilium` + their shards, both
///   Enriched ores, both seals, both Refine Coupons) and with them 42 monster drop rows — each
///   row's weight is added to that monster's `nothing` so no OTHER drop's rate moves. The two
///   ore-forging recipes go; the 12 blacksmith GEAR recipes stay and move to the Craft screen's
///   new third tab, because deleting the workbench would otherwise strand them.
///
///   SERVER: the whole server-roll refine path (§6.4 2b) — `/refine/roll`, the nonce, the
///   outstanding/consumed reconcile, the bind-check — is deleted; the `refine_outstanding` /
///   `refine_consumed` TABLES are left in place (dropping them in the boot path would delete
///   data every time an old binary deploys). The `refine_1` daily/weekly commission is replaced
///   by `card_feed_1` on a FRESH bit (4 is retired, never recycled), and the battle-pass /
///   schedule rewards that paid Refine Coupons or ore now pay consumables.
///
/// 0.0.372-alpha — GRADE: the power axis of the fixed-stat gear model (design §3.5). P1 of the
///   equipment rework, and the piece every later phase hangs off — gem values, fusion output and
///   the market's prices are all stated as functions of grade, so nothing else could go first.
///
///   `Grade` is ten steps (Common … Cosmic), ordinal, serialized by name, and SEPARATE from
///   `Rarity`: rarity is the display/rare-bag axis and carries the enchant ★ ladder, grade is
///   what fusion moves and what the market prices. `Item.grade` is per-instance EARNED state,
///   skip-if-Common so every existing save and golden stays byte-identical.
///
///   TWO multiplier tables, not one: raw stats (ATK/MATK/DEF) ×1.30 per step (×10.60 end to end)
///   and PERCENT stats ×1.15 (×3.52). A percent line has a real ceiling — block caps at 100%,
///   attack speed hits the tick — so paying it the raw curve would run it off its own scale by
///   grade 7. Both numbers are measured against TBH's real sword table (raw ×10.8-10.9, percent
///   ×2.2-4.0), and they are const TABLES because the sim has no floats and `pow` is not
///   reproducible across platforms.
///
///   DERIVED AT READ TIME (`graded_atk` / `graded_def` / `graded_atk_speed_bp`), never baked into
///   the instance — the same rule `star_base_bonus` follows, and for the same reason: a stored
///   scaled number needs a migration every time the ladder is tuned. ★ now scales off the GRADED
///   base, so an enchant on a high-grade piece is worth what the design's per-layer caps say it
///   is. Every host that showed `atk_bonus` now shows the graded number, so the item tooltip and
///   the character sheet cannot disagree.
///
///   ONE new PRNG draw: `roll_gear_grade`, taken only when a real piece of gear banks (the
///   conditional-draw shape the foil roll already uses). Odds are the design's per-band table,
///   mapped from the monster's level through the LAUNCH ladder (20 design bands across Lv1-70)
///   rather than the catalog's 100 zone bands — reading one as the other falls off the end of the
///   table. Common absorbs each row's rounding so every row sums to exactly 1.0. `golden_alpha4`
///   re-pins (it is the scenario that banks gear).
///
/// 0.0.373-alpha — THE GRADE ODDS MOVE INTO THE CATALOG, and sim-cli learns to MEASURE them
///   (`--gear-report`). Answering "how do we fine-tune this?" rather than adding a mechanic:
///   the odds are the numbers most likely to be swept, and they were in Rust, where every trial
///   cost a rebuild and a sweep was impossible.
///
///   `loot_rules.gear.grade_odds` (catalog 0.0.122) is now the source; `drops::GRADE_ODDS_BP`
///   stays as the fallback a hand-built test state uses, with a test asserting a REAL monster
///   carries the authored rows — a fallback that quietly takes over would mean edits to
///   game_data.json do nothing and nothing says so. Resolved at build time and baked into
///   `MonsterTemplate.gear_grade_odds` / `Entity.gear_grade_odds`, because `tick()` holds no
///   catalog; skip-if-empty keeps old saves loading. Goldens re-pin (templates carry a new field).
///
///   `sim-cli --gear-report` runs the farm loop and prints per-grade drop counts, CE/hour
///   (Common-equivalents: fusion is 6:1, so a grade is worth 6^step) and how long a party's 55
///   slots take to fill — then the same pacing computed ANALYTICALLY from the authored odds ×
///   the measured drop rate, because the tail (Immortal is one drop in 400) cannot show up in a
///   six-hour sample and a reader would conclude the ladder is unreachable.
///
///   First measurement at Lv70 (zone_b70_1, 6 h): 1,471 kills/h, 8.8 gear/h, sampled shares
///   75.5/20.8/3.8% against the authored 76.4/18.0/4.3 — and 18.1 CE per drop, which is the
///   number the design computed by hand. The 55-slot pacing lands ~20% faster than the doc's
///   table because the doc assumed 7 gear/h.
///
/// 0.0.374-alpha — FUSION (design P3) + the economy it replaces. Six pieces of one grade become
///   one of the next: the only road up the ladder that is not a drop, and the thing the drop
///   odds were always written against.
///
///   `fusion::fuse` / `InputEvent::FuseGear`: six of ONE grade, any type, any tier. The output's
///   tier is the inputs' average rounded by a coin whose bias IS the fraction, which keeps two
///   properties the design proved across all 117,649 combinations — `ceil(avg) <= max(input)`, so
///   tier cannot be laundered upward, and an expectation equal to the true average, where
///   floor-only rounding would tax every fusion 11.5%. THREE draws in a fixed order, taken even
///   when the outcome is already decided, because the stream must not depend on the result.
///
///   FUSING IS FREE AND PAYS (user 2026-09-15: "ผสมแล้วได้ gil ดังนั้นผสม cost 0 gil ก็ได้"). A fee
///   would make every combine a decision about MONEY in a system meant to be about GEAR, and an
///   early player is broke exactly when the fodder piles up. It hands back half what the six
///   would have vendored for; a failure pays half of that.
///
///   JUNK LEAVES THE GAME (user, same conversation): 33 defs and 62 drop rows, 21.8% of the whole
///   table's weight, moved into `nothing`. Junk existed to be the baseline Gil faucet, and fusion
///   is that faucet now — measured, not assumed: the 48-hour run ends at 13,939 Gil where the
///   same run with a fee and no junk ended at 0 and could not fuse once.
///
///   BAG 20 -> 50 SLOTS (user). Six of a grade have to be held AT ONCE, and a party feeding
///   several grades needs several piles; at 20 the bag filled with parts and incoming drops were
///   lost. Also measured: the pacing run reported "0 fusions" until the bag grew.
///
///   DROPS TWICE AS OFTEN, WORTH HALF (catalog 0.0.125, user: "เพิ่ม drop rate ขึ้น 2 เท่า ให้ดรอบรัวๆ").
///   The gear rule's bp doubled AND the grade tail halved, because those are two different axes:
///   a drop every 2.5 minutes instead of 5 is FLOW, and the CE per hour that reaches the economy
///   is VALUE. Doubling flow alone put 16 Legendaries in a new player's first two days against a
///   target of 5-10; halving the tail alongside it lands on 8. Bands 1-12 keep the +50% tail from
///   0.0.124, which is what lifted a new player's first two days from 4 Legendaries to 7.
///
///   Both goldens that bank gear re-pin. `sim-cli --gear-report` grew the party side of the
///   model: every member equips best-in-slot on GRADED stats (not just the lead, not the authored
///   numbers), surplus is fused rather than vendored, and the report prints what the party ends
///   up WEARING — the only number in it that is about power rather than supply.
///
/// 0.0.375-alpha — SLOT NAMES SAY WHAT THEY ARE (design §2.5, catalog 0.0.126). A rename, and
///   the reason it is worth a version: the old names described RO's layout and then lied about
///   it. `HeadMid` was displayed as "Face" and held a MASK; `HeadLow` was displayed as "Aura" and
///   held a CHARM — three "head" slots, two of which were not on the head. `Garment` was a cloak
///   filed under defence while carrying almost none.
///
///   `EquipSlot`: HeadTop→Helmet · HeadMid→Gloves · HeadLow→Pants · Garment→Earring (and it moves
///   to the utility side) · Accessory1→Necklace · Accessory2→Ring · +Bracer (the 4th utility
///   slot, no items yet — P2 authors them). `CardClass` follows for the three it shares, and
///   keeps `Head` as one panel covering Helmet/Gloves/Pants until cards are redesigned with gems.
///
///   COMPATIBILITY, both halves: every renamed variant carries a `serde(alias)` for its old
///   spelling, and `DEF_ID_REMAP` rewrites the four renamed def families on load
///   (`pendant_s3` → `necklace_s3`). The catalog's `pendant`/`ring` were also SWAPPED onto the
///   slots their names imply — they were on Accessory2/Accessory1 respectively, which nothing
///   depended on and everything read wrong.
///
///   Found while doing it: the remap table still carried `ore_primocon → ore_valkirion`, whose
///   TARGET was deleted with refine in 0.0.370. A remap pointing at a def nobody authored is
///   worse than no remap — it turns a recognisable stale item into an unrecognisable one. Gone,
///   and `the_remap_table_points_at_real_defs` now pins both directions (the target must exist,
///   and the source must NOT, or the table would rewrite a live id).
///
///   `save_shape`'s per-instance guard stopped comparing def_id: migration is ALLOWED to remap an
///   id, and a guard that pins it fails on a rename that worked. It compares the earned state
///   (stars / Resonance / stack / bound) positionally instead, which is what it was defending.
///
/// 0.0.376-alpha — P2 lands, with the two systems that make it work and the two guards that make
///   it tunable (catalog 0.0.127).
///
///   GEAR BUDGET (design §2.6): weapon ATK s1 140 → s7 1,525 (was 240 → 720, ×2.25 across the
///   ladder; now ×10.9, the same span as the grade ladder so neither axis buries the other) and
///   the character curve drops to compensate — total ATK per level is UNCHANGED, so no monster is
///   re-tuned. DEF is re-split across the six defensive slots by the design's ratios (armor 1.00 ·
///   shield 0.60 · pants 0.45 · shoes 0.35 · helmet 0.30 · gloves 0.25) at the same per-tier
///   total. Necklace / Ring / Earring lose their main stat entirely (§2.5 rule 1): their value is
///   their lines now. First measured attempt was ~9% weaker because it computed the split from
///   (char + weapon) and forgot the 560 ATK the two accessories used to carry; with them counted
///   the doc's own numbers fall out exactly (2,202 total at s7), and the 48-hour run lands within
///   3.4% of the old curve with all 50 slots worn.
///
///   AUTHORED SUB-STAT LINES (`gear_roll::authored_lines`): every piece carries lines derived from
///   (def × grade) — 0/1/1/2/2/3… by grade, one more on a utility piece — through the SAME
///   family/slot/value machinery a roll uses, driven by a PRNG seeded from the def id and grade.
///   Deterministic, never stored, never drawn from the game's stream, and identical on every copy
///   of a product, which is the property TBH's fungible market rests on and the one a per-instance
///   roll would have destroyed. Values ride the percent ladder.
///
///   PRODUCT IDENTITY: `Item::product_key()` = `def#Grade`. `def_id` alone would pool a Common and
///   a Beyond under one name (×10.6 apart) — the lemon market D1 exists to prevent — and the
///   client now names them apart ("Beyond Adamant Saber").
///
///   `ItemDef.num`: a permanent, append-only integer per item, assigned now in catalog order so the
///   Steam itemdefid (`num × 16 + grade`) has something stable to derive from before anything is
///   minted. `gear_overrides`: per-(def × grade) exceptions to the formula, stamped onto the item
///   wherever a grade is assigned (drop via the baked template, fusion, migration) so replay
///   agrees; two tests keep every row a real, sane, explained exception.
///   `catalog_hygiene` also pins the shipped copies to the source — the manual `cp` after every
///   catalog edit was the one step with no guard.
///
/// 0.0.377-alpha — fusion closes its peek-and-select hole (design §7.8): a SERVER ROLL from
///   Legendary up.
///
///   The client holds the seed, so a modified client could read the next three draws and press
///   FUSE only when they said "weapon, tier up, success" — every fusion of the desired type, and
///   the market demand that random output exists to create is gone. Replay cannot see it: nothing
///   in the log breaks a rule. The fix is the one refine used before it was retired (0.0.370):
///   `FuseGear.server_roll` (serde default 0 — old logs unchanged). Non-zero reads all three
///   outcomes out of the nonce (mixed radix over a 63-bit value: coin, success, type) and takes
///   ZERO `state.rng` draws; zero is the client roll and still draws three. The draw count follows
///   the EVENT, which is in the log, so replay is exact either way.
///
///   HYBRID by grade: `fusion::FUSE_SERVER_ROLL_FROM = Legendary` — a Rare→Legendary combine or
///   better must carry a roll; below it the client keeps rolling (the prize is not worth the cheat
///   and the server should not be consulted about every Common combine). The sim accepts any roll
///   for any grade; the VERIFIER refuses a zero from that line up and recomputes the nonce against
///   `fusion::binding` — the six sorted `product_key`s plus `measures.fusions`, the new attempt
///   counter (moves on failure too). Products rather than instances: the output depends on nothing
///   else about the inputs, so a nonce for one set of six is correctly valid for another set of the
///   same products, and pick order is irrelevant. Shape change: the counter and the event field.
///
/// 0.0.378-alpha — gear drops are authored PER SLOT, not per def (catalog 0.0.128).
///
///   `loot_rules.gear` paid every def of the tier the same absolute odds, which made the drop mix
///   a function of how many defs a slot had: six weapon families at three variants each put ~60%
///   of every drop into one of a member's eleven slots, and adding a family would have shifted
///   every other slot's share with nobody deciding it. Now each paper-doll slot pays
///   `field_slot_bp` / `boss_slot_bp` per kill, split across its defs by relative weight
///   (`slotted_mult` / `two_slot_mult` / `dagger_mult`, all 1-2 today); the five per-def fields
///   are gone. Set at 12 / 24 bp per slot so the tier-4 total per kill is what it was (116 / 246
///   bp); tier 1 loses the extra odds its starting_* / demo_* defs were adding to the weapon slot.
///   Equal first, then tune (user 2026-09-15) — the tuning is now one number per slot. Goldens
///   that bank gear moved; the baked tables changed.
///
/// 0.0.379-alpha — the gear pool deals only BADGED defs; the two `demo_socket_*` test defs are
///   gone; starting weapons sit below the first drop (catalog 0.0.129).
///
///   `def_tier` reads an id with no `_sN` as tier 1, and the drop / fusion pools were filtering on
///   it — so the five `starting_*` weapons and `demo_socket_sword` / `demo_socket_armor` (card-
///   system test defs from 0.0.1xx, referenced by nothing) were being dealt at tier 1, and the
///   demo pieces out-hit every real s1 (240 / 200 with a free socket against 140 / 156). The pools
///   now share `gear_roll::in_gear_pool`, which requires the badge (`def_tier_badge`); the
///   fallback stays for enchant / scoring, where a starting weapon still needs a band.
///   `every_gear_def_is_reachable_from_some_spawned_table` now allows exactly the classes'
///   starting weapons to be undealt and nothing else.
///
///   The demo defs are deleted; a save holding one migrates to the real 1-slot s1 piece
///   (`DEF_ID_REMAP`). The same sweep found four Phase-0 relics the fallback was also dealing —
///   `sword_short` / `sword_long` / `leather_armor` / `chain_armor` (250 ATK, 700 DEF at tier 1)
///   — deleted and remapped to the s1 piece of their slot. `num`s 2-5, 125, 133 are retired,
///   never reused.
///
///   `starting_*` ATK 160 / 160 / 140 / 140 / staff 120 → 100 / 100 / 80 / 100 (+70 MATK) / 100
///   MATK — 0.7 × the family's s1, so the first drop is an upgrade rather than a downgrade (the
///   old numbers predate P2 and had ended up ABOVE s1). Goldens moved: the baked tables lost rows
///   and the starter's hits changed.
///
/// 0.0.380-alpha — gear has ONE axis: rarity leaves Weapon / Armor (catalog 0.0.130).
///
///   Since grade arrived (0.0.372) a piece of gear carried two ladders that shared three words:
///   `rarity` (a Common/Uncommon/Rare that meant "0/1/2 card sockets", bumped by stars) and
///   `grade` (the power axis). The client coloured by the first and named by the second, so a
///   grey "Legendary" was a Common-socket piece at grade 7 and nobody could read it. User
///   2026-09-15: "rarity ต้องเอาออก + ช่องการ์ดต้อง fixed ตามไอเทม".
///
///   - The 278 Weapon / Armor defs drop their `rarity` field (serde default → Common); a loaded
///     save's gear refreshes to Common through `migrate_item`, which no longer re-derives a
///     star-promoted rarity (`star_rarity` is gone; a star-up no longer touches `rarity`).
///   - Card sockets stay what they were: the def's own fixed `card_slots`. Never rolled.
///   - THE RARE BAG IS RETIRED (user: "rare bag เอาออกจากเกมก่อน ต้องคิดใหม่"). Its routing rule
///     was written in rarity, and with rarity gone from gear there was nothing left to route by;
///     rather than re-key it on grade it leaves until it is redesigned. Nothing routes there now;
///     `migrate_save_items` moves whatever an old save still holds into the inventory (past the
///     cap if it must — the bag held three). `rare_bag`, `rare_bag_cap`, `rare_threshold` and the
///     two events stay so old saves and logs read; the client's rare strip is gone, and with it
///     the Enchant (Star Stone) and Apex screens the user named obsolete in the same pass.
///   Rarity itself stays for Cards / Exemplars / Regalia / materials, which have no grade.
///   Shape change: the gear defs' baked rows (two goldens moved).
///
/// 0.0.381-alpha — the STAR STONE system leaves the game (catalog 0.0.131). User 2026-09-15:
///   "ตัดระบบ Star Stone ออกทั้งหมดเลย".
///
///   With the fixed-stat gear model (D1: no ★, no rolled line on a drop) the stones were the only
///   thing still writing stars and rolled lines onto gear, through a bench the client had just
///   deleted. Gone: `orbs` (fee, eligibility, apply, its four draw contracts), the eight
///   `star_stone_*` defs, the host's `orbs` / `orb_plan:` / `orb_fits:` answers (they return the
///   empty shape now) and the bridge preview. Kept so old saves and logs read: `OrbEffect` and
///   `OrbTarget` as deserialize-only types (`Item.orb` stays lenient), `UseOrb` as a no-op like
///   `AllocateStatPoint`, and `measures.orbs_used` (a Daily counter row names it). A stone an old
///   save still holds in the ledger is simply an unknown def now — inert, not an error.
///   `Item.star` / `bonuses` and `gear_roll` stay: the server-rolled craft and `GrantGear` paths
///   still write them, and `authored_lines` reads the same machinery. No golden moved (the stones
///   had no drop row).
///
/// 0.0.382-alpha — ★ leaves gear entirely; crafting gear grants on the spot. User 2026-09-15:
///   "ตัด server-roll craft/GrantGear ด้วยเลย ให้ ★ หายหมด".
///
///   After 0.0.369 (D1) and 0.0.381 nothing was WRITING a star or a rolled line onto gear any
///   more, but the plumbing that once did was still standing: the +10%/★ base-stat floor
///   (`star_*_bonus`, read by `derived`, the item view, the bridge, the sim-cli scorer), the
///   `gear_roll` roll functions, and the server round-trip a gear craft took — a `Craft` ticket
///   the server answered with `GrantCraft` banking the unrolled piece, i.e. latency for nothing.
///   Gone, all of it: `CraftItem` grants gear like a potion, now; `output_is_rolled_gear` answers
///   `false` (the craft screens' `server_rolled` key keeps its shape); `GrantCraft` / `GrantGear`
///   stay only to drain a ticket an older save may still carry. `migrate_item` clears `star`,
///   `bonuses` and `locked_slot` on every Weapon / Armor it loads — a piece an old save enchanted
///   comes back as the authored piece at its grade. A CARD's star is its level and is untouched.
///   `Item.star` / `bonuses` remain as fields for cards and for the save shape; `gear_roll` keeps
///   only what `authored_lines` needs. The item view sends `authored_lines` as text now, through
///   the same formatter as `bonus_lines`. No golden moved.
///
/// 0.0.383-alpha — the off-hand is the Shield slot; Bracer drops (catalog 0.0.132). User
///   2026-09-15: "weapon left เอาออก → offhand ทุกกรณี / Bracer ต้องทำให้ดรอบ".
///
///   `EquipSlot::WeaponLeft` is retired, and so is DUAL WIELD with it (user: "ไม่มี skill
///   dualwield แล้ว จะมี offhand แยกแทน (ยังไม่คิด)"): `dual_wield_mastery` leaves the Assassin
///   kit and the catalog, the off-hand second strike, its ATK term and the −18% speed penalty
///   are gone from combat, and a dagger never routes off-hand — the Shield slot holds a shield
///   and nothing else until the separate off-hand is designed. `has_shield` and the speed sum
///   ask "Shield slot AND not a weapon" so a stale stamp can never count as a shield;
///   `migrate_item` clears any off-hand stamp (`WeaponLeft`, or `Shield` on a weapon) and the
///   per-entity de-dup evicts the now-colliding second dagger to the bag. The variant stays so
///   the stamp deserializes; the line pools drop it; the client draws one Off-Hand cell.
///
///   Bracer, the fourth utility slot, has items now: `bracer_s1..s7` × plain / [1] / [2], nums
///   399-419, no main stat (lines only, like Necklace / Ring / Earring; its pool is the accessory
///   pool). It joins the per-slot drop rule at the same weight as every other slot, so the
///   guard now expects eleven wearable slots in every table. Two goldens moved (the baked tables
///   gained rows).
///
/// 0.0.384-alpha — SUBSTATS are a catalog table, on a ten-stat pool (catalog 0.0.133).
///   User 2026-09-15: "ลุยตาม §7 เลย ทำ pool 10 ตัว + ตารางใน JSON" — after the TBH datamine
///   (`docs/tbh-substat-datamine.md`) showed what the lines actually are.
///
///   The lines a piece carries at its grade were a formula in `gear_roll` over the 22 RO-era
///   enchant families — damage vs race / size, status resist, ignore-DEF and the rest — most of
///   which the post-pivot combat no longer reads. Now `loot_rules.substats` says everything:
///   lines per grade (0/1/1/2/2/3…, TBH's), the utility extra, each stat's seven tier windows
///   and cap, and which stats each slot may carry (Weapon: ATK% · ATK · Crit DMG · Crit · CDR;
///   Armor: HP · HP% · DEF; Bracer: Dodge · DEF · HP · HP%; …). `substats::lines_for` is the
///   only resolver: FNV-1a over (def × grade) into a local PRNG, no game draw, values on the
///   percent ladder and under the cap. Ten stats in the pool — the ones combat reads — plus
///   Break / Charge reachable from the catalog. The generated table is
///   `docs/references/roi-substat-table.csv` (`sim-cli --substat-table`), the thing to diff.
///
///   The lines are STAMPED now (`Item.lines`, by `assign_grade`, like the override): the drop
///   bank reads them off the baked row (`DropEntry.lines_by_grade`), fusion and migration off the
///   catalog. Combat, the view and the sim-cli scorer read the field — no per-tick recompute, and
///   an old save refreshes on load so a re-tuned table reaches every copy. `gear_roll` keeps only
///   the tier badge. Shape change: `Item.lines`, `DropEntry.lines_by_grade`; two goldens moved
///   (baked tables).
///
/// 0.0.384-alpha (catalog 0.0.134) — the sensitive stats are budgeted as a SET, not a line.
///   User: "ระวังค่าที่ sensitive เช่น dodge cri cdr". The first windows let a full eleven-slot
///   best-in-slot set at the launch cap (Beyond s7) reach Crit +44%, CDR 80% and a nominal
///   Dodge 100%. The audit is now done against the STACKED total across every slot (the
///   `--substat-table` output, best def per slot): at Beyond s7 the set tops out at Crit +14.6%,
///   Dodge 14% (folded), CDR +18%, Crit DMG +35%, Move +24%, ATK% +28%, HP% +42% — and Cosmic
///   at Crit 16 / Dodge 22 / CDR 24 / Crit DMG 59. Done by narrowing the windows, tightening the
///   per-line caps (crit 4%, dodge 8%, CDR 8%, crit dmg 15%, move 15%), and putting each
///   sensitive stat in three or four slots rather than five: CDR lives on Helmet / Necklace /
///   Ring, Dodge on Shield / Shoes / Bracer, Crit on Weapon / Necklace / Earring / Ring.
///
/// 0.0.385-alpha — the Fusion Forge's bridge: `fuse_rules` and `fuse_preview:` host queries and
///   `ServerClient.FuseRoll`, so the client can build the screen (no sim shape change).
/// 0.0.386-alpha — GEMS (`docs/gem-system-design.md`, `gems.rs`): the owner's layer on a fungible
///   product. A piece has `gems::SOCKETS_BY_GRADE` sockets minus its card slots (`Item::gem_slots`);
///   a gem is a two-stat SET fitting a slot group (11 sets: Strife / Destruction / Ruin on weapons,
///   Life / Fortitude / Wind on armour, Valor / Wildness / Steel on accessories, Charge / Vigor
///   anywhere), graded on the gear ladder (×1.3/step raw), kept in the materials LEDGER as
///   `gem_<set>_g<step>` (110 defs, nums 420-529). `SocketGem` rolls each stat ±5% (one draw per
///   stat) into `Item.gems: Vec<SocketedGem>`, overwriting a filled socket; `FuseGems` turns six of
///   one grade into one of the next (one draw for the set); both Client authority, both no-ops
///   without a draw on refusal. Gems DROP through the gear rule's table as one Common row per set
///   (`loot_rules.gems.field_bp/boss_bp`), graded by the monster's odds with one conditional draw
///   and re-keyed. Worn gem lines fold in `stat_mods::for_each_mod` summed per stat and clamped by
///   `gems::cap_of` (crit/dodge/CDR/move 15%, crit dmg/ATK%/HP% 50%). `Item::market_clean` names
///   the rule a future item market gates on. `Measures.gems_socketed` / `gem_fusions`. Catalog
///   0.0.135. GOLDENS: alpha6 + diag_band4 moved (gem rows in every drop table).
/// 0.0.387-alpha — CDR reaches every cooldown: `stat_mods::scale_cooldown` now also arms Dupla
///   Medicina, Rivus, Subvenio, Taunt and Flame Shield's break-cooldown (they took the raw
///   `cooldown_ticks`). Gems: drop odds 20/40 bp (≈15% of the gear rate, user), Charge set base
///   30 with a 100% wall on charge rate / break build-up (Break DMG 50%), Steam-only trade. Catalog
///   0.0.136. GOLDENS: alpha6 + diag_band4 moved (gem drop weights changed every table).
/// 0.0.388-alpha — SOCKETS ARE THE GRADE'S (user: "เรียงตามเกรด … เข้าใจง่ายกว่า"): gem sockets
///   `gems::GEM_SLOTS_BY_GRADE` [0,1,1,2,2,2,2,3,3,3] on every wearable slot, card sockets
///   `CARD_SLOTS_BY_GRADE` [0,0,1,1,1,2,2,2,2,2] (`Item::socket_count` reads the grade; the def's
///   `card_slots` and the RO weapon-level cap are retired). The 175 slotted def variants
///   (`_1s` / `_2s`) are deleted — a save holding one keeps the base def at its grade
///   (`migrate_item` suffix rule); `GearRule.slotted_mult` / `two_slot_mult` gone. Gem sets fit by
///   slot family (weapon → Weapon + Shield, armour → the five armour slots, accessory → the four,
///   universal → all); bases ×1.5 so a full set at the launch cap (22 sockets) totals what the
///   33-socket design did. Catalog 0.0.137. GOLDENS: alpha6 + diag_band4 (drop tables shrank).
/// 0.0.389-alpha — the PANTS slot is retired (user: Bracer was meant to replace an armour slot,
///   "เอา pant ออกได้"): ten wearable slots — Weapon Shield · Helmet Armor Gloves Shoes · Necklace
///   Ring Earring Bracer. `EquipSlot::Pants` stays deserialize-only; `migrate_save_items` drops a
///   Pants piece from bag / stash / paper doll; the seven `pants_*` defs and the `charm_` remap
///   are gone; `card_class_of_slot(Pants)` = None; armour gems fit four slots. sim-cli's
///   auto-equip list had never included Bracer (10 entries ending in Pants) — Bracer takes the
///   seat, so the gear-report now wears all ten. Catalog 0.0.138. GOLDENS: alpha6 + diag_band4
///   (seven rows left every drop table).
/// 0.0.390-alpha — `StatMod::ElementDmgAnyBp`: +% damage with any non-Neutral element, folded in
///   `stat_mods::element_dmg_bp` (Valor gem = CDR + Element DMG; cast speed retired — the live
///   kits have no cast time and skill tempo is Haste). Catalog 0.0.141. GOLDENS: none moved.
/// 0.0.391-alpha — two lines with consumers: `DmgVsBossBp` (own multiply in `combat::damage`
///   when the target `is_boss`) and `HpRegenFlat` (flat HP every 5 s regen window in
///   `tick::upkeep`, beside the passive regen). Gems: Valor = CDR + Boss DMG, Fortitude = HP +
///   HP regen, new accessory set Arcana = Element DMG + Heal (nums 530-539) — nine sets, eighteen
///   distinct stats. Catalog 0.0.142. GOLDENS: alpha6 + diag_band4 (one gem row per table).
/// 0.0.392-alpha — Arcana is three gems, each a PAIR of elements as two `ElementDmgBp` lines
///   (Twilight = Holy + Shadow, Fervor = Fire + Physical, Storm = Water + Thunder; user: a party
///   spans the six launch elements, so no pair is a wasted drop, and 1/3 beats 1/6). Each weighs
///   a third of another set (weight 1 vs 3). Heal% leaves the gem table (niche). `ElementDmgAnyBp`
///   (0.0.390, never shipped) is deleted; `GemStat.element` parameterises the `element_dmg` key;
///   the fold sums element lines per element. Nums 540-569. Catalog 0.0.143. GOLDENS: alpha6 +
///   diag_band4 (gem rows).
/// 0.0.393-alpha — gem fusion can FAIL (`loot_rules.gems.fail_bp_by_grade`, by input grade: 0 / 0
///   / 10 / 20 / 30 / 40 / 50…%; TBH reads ~50% success at the top): one gem of the input grade
///   comes back, five are gone. Two draws per fusion, always (fail check, then set).
///   `Measures.gem_fusions_failed`. Catalog 0.0.144. GOLDENS: none moved (no golden fuses gems).
/// 0.0.394-alpha — GEM CRAFTER level (user: "+1% ตามเลเวล blacksmith/gem crafter"): derived from
///   `Measures.gem_fusions` (10 fusions per level, cap 30), each level takes 1% off the fail odds
///   (`gems::fail_bp`); no new state. `gem_rules` answers the party's level and its live odds.
///   Catalog 0.0.145. GOLDENS: none moved.
/// 0.0.395-alpha — BLACKSMITH level for gear fusion, the gem crafter's twin: `Measures.fusions`
///   / 10 per level, cap 30, +1% success per level on every tier (`fusion::success_bp_for`).
///   `fuse_rules` / `fuse_preview:` answer the level and live odds. GOLDENS: none moved (no
///   golden reaches tier 6 fusions).
/// 0.0.396-alpha — ONE fusion rule for gear and gems (user: "ควรทำเหมือนกันสองระบบ"):
///   `loot_rules.fusion { fail_bp_by_grade, crafter_* }`. Gear fusion now fails by INPUT GRADE
///   (not output tier — `FUSE_SUCCESS_BP` deleted) and a failure hands the drawn type back at the
///   input grade (five lost, half Gil) instead of eating all six; `FuseOutcome::Failed` carries
///   the def. Blacksmith / Gem Crafter constants moved from code / the gem block into that one
///   block. `Measures.fusions_failed`. Catalog 0.0.146. GOLDENS: none moved (no golden reaches a
///   fallible fusion).
/// 0.0.397-alpha — a failed gear fusion pays the FULL Gil (user: "ให้ gil เต็มไปเลยถึงจะล้มเหลว");
///   `FUSE_FAIL_RETURN_BP` deleted. No shape change.
/// 0.0.398-alpha — THE STAGE LADDER (`docs/stage-ladder-renovation.md` §4, user 2026-09-16):
///   240 stages × 3 rooms (20 regions × 12) replace 100 bands. Difficulty is a smooth per-STAGE
///   table `stage_base` (geometric, anchored stage 1 / 168 = Lv70 / 240 = Lv100; `level_base` and
///   its inherited cliffs are gone, the JSON key is an alias); a monster's LEVEL is derived,
///   `level_of_stage` = ceil(stage × 100/240), and still keys DEF / gear tier / grade odds.
///   Rooms 2-3 heal `ROOM_HOP_HEAL_BP` (20%) of max instead of full (room 1 full) — a stage is one
///   run to last through. The march grace is the WALK: `march_grace_ticks(slowest move%)` =
///   52 cells at 5 c/s ÷ (1 + move%) + 3 ticks, the open grace 15 ticks; regen ticks every second
///   while marching. No auto-retreat (`handle_party_wipe` restarts room 1; sim-cli models its
///   own), no loot gap penalty (`loot_gap_mult_bp` deleted, view key gone); gear slot odds grow
///   by stage (`GearRule::slot_bp_by_stage`, 12/24 → 20/40). `ZONES_PER_REGION` 12; region helpers
///   take a stage. Catalog 0.0.147. GOLDENS: alpha6 + diag_band4 (real zones).
/// 0.0.399-alpha — NO MONSTER LEVEL (user: "ตัด mob level ออกเลย"): the stage is the one difficulty
///   number. `stage_base` gains `def` (was level×10) and `toughness` (was `toughness_for_level`);
///   `ZoneDef.level` is gone (dungeon/story zones author `stage`), `MonsterDef.level` →
///   `stage`, `MonsterTemplate.level` → `stage`, `Entity.mob_level` → `mob_stage` (aliases keep
///   old saves loading); soft DEF = stage × 25/12 (the old level/2 × 10, exact); gear tier
///   `bands.max_stage`, grade odds `max_stage` (code fallback by stage), sigil `loot_tier` by
///   stage (60/120). Bestiary view keys `stage_min` / `stage_max`. XP per kill ×0.7 (user:
///   "xp ช้าลงนิดนึงสัก 30%"). Catalog 0.0.148. GOLDENS: alpha6 + diag_band4.
/// 0.0.400-alpha — REGALIA IS A W-ENGINE (user 2026-09-16): a fixed ATK LUMP + one ADVANCED stat
///   per piece, both growing with the Regalia's OWN level (cap 60; lump ×15 and advanced ×2.5 by
///   Lv60 — ZZZ's ratios), three tiers R / SR / SSR (lump 100 / 140 / 200 at Lv1), passive empty.
///   Lines: sword crit dmg · thief crit · archer ATK% · mage CDR · acolyte HP%. `base_mods` /
///   `per_level_mods` are gone from `RegaliaDef`; `REGALIA_MAX_LEVEL` 5 → 60; fodder R +5 / SR +15
///   (provisional). No shape change (level was already u8). GOLDENS: none moved (no golden wears
///   a Regalia).
/// 0.0.401-alpha — BREAK DAMAGE reads the target's level off its STAGE (`mob_stage × 100/240`):
///   a monster's stat-block `level` is never filled, so since the level moved off the stat block
///   every break was `40 × 1 × gauge` — a scratch. Now `40 × level × gauge` as designed (a fifth
///   of a matched monster's HP). Host: `DISPLAY_SCALE` ÷10 on every amount a client prints (HP,
///   ATK, DEF, damage / heal numbers, flat lines); the sim is untouched. GOLDENS: alpha6 +
///   diag_band4 (breaks now deal real damage).
/// 0.0.402-alpha — gem drop odds BY STAGE (`GemRules::field_bp_by_stage`, twin of the gear
///   rows): drop odds per kill follow hits-to-kill, so an under-level farm pays the same per hour
///   in count and less in tier / grade / XP (user: "ฟาร์มด่าน under lv คุ้มน้อยกว่า on level"). Catalog
///   0.0.151 (ladder rebuilt by `tools/build_stage_ladder.py`: K 1 → 3 by stage 30 → 6 at the
///   cap, J 6 → 10%). GOLDENS: alpha6 + diag_band4.
/// 0.0.403-alpha — THE FALL-BACK (WoG's rule, user): a wipe on stage N+1 sends the party to
///   stage N in farm mode for `WIPE_FALLBACK_FARM_CLEARS` (5) clears, then back to N+1
///   (`handle_party_wipe` + the countdown in `tick`); stage 1 and a player-set farm just restart.
///   The two `retreat_*` fields are live again (same serde). sim-cli's own retreat model is gone.
///   GOLDENS: alpha6 + diag_band4 if they wipe.
/// 0.0.404-alpha — the NEXT gear tier previews in the last quarter of each band
///   (`GearRule::tier_at`; `build_drop_table` pools both tiers at equal weight there): every
///   measured ladder run stalled 12-48 h at a band's end (stages ~37 and ~110) wearing the band's
///   tier against the next tier's numbers. Catalog 0.0.155 (K 3 at stage 30 → 5 at the cap,
///   kills per room ×1 → ×2.5). GOLDENS: alpha6 + diag_band4 (drop tables).
/// 0.0.405-alpha — BREAK DAMAGE = `(100 + 7 × Σ party level) × (toughness + 20)/40` before
///   DEF (HSR's shape, the level being the PARTY's), replacing `40 × target level × gauge`, a
///   constant authored before the x10 stat scale that left a break below a basic swing on
///   every stage (40 vs ~300 at stage 1, 1,680 vs ~2,000 at stage 100). Levels and recruits
///   now grow the break; `BreakDmgBp` carries the tail past the cap. GOLDENS: alpha6 +
///   diag_band4.
///
/// 0.0.406-alpha — TRAINING (ฝึกฝน): the party-wide Gil ladders (`training.rs`,
///   `docs/training-system-plan.md`). `GameData.training` (one cost curve + a slot per stat),
///   `GameState.training` (slot → level), `InputEvent::Train`, `GilSpend::Training`,
///   `Measures.training_levels`, `Entity.training_mods` rewritten per tick and folded by
///   `stat_mods::for_each_mod`; `StatMod::DefBp` (new, the Fortitude slot) joins the DefPct
///   bucket; the Satchel slot reaches `inventory_cap` through `training_stash_slots`. Every
///   field is serde-default/skip and the catalog block ships EMPTY in this version, so goldens
///   are byte-identical.
///
/// 0.0.407-alpha — LONGER FIGHTS, pass 1 (user 2026-09-17: "ลดเดเมจมอนสเตอร์ + เพิ่มจำนวนก่อน"
///   / "WoG มอนจะไม่ attack แรงมาก แต่จะค่อยๆ ตีให้เลือดลดลง"): the mobbed-DEF penalty is retired
///   (`mobbed_def` is the identity); catalog 0.0.159 rebuilds `stage_base` with a FLAT J of 6%
///   of a member's (model) HP per room-hit — half the old floor, no ramp, so the crowd chips
///   rather than bursts and difficulty rises through K and the room's kill count — and sets
///   every ladder room to 30 kills (+1 per room) before its boss. GOLDENS: alpha6 + diag_band4.
///
/// 0.0.408-alpha — LONGER FIGHTS, pass 2 (user: "ทำ HP ×2"): catalog 0.0.160 rebuilds
///   `stage_base` with K ×2 (2 → 10 → 16 → 20 hits per monster) — 0.0.407's halved crowd
///   damage alone reached the launch cap on day 4. XP, Gil and drop odds ride K, so the hour
///   pays the same; the break constants double with it (`BREAK_DMG_FLOOR` 200, per level 14) so
///   a break keeps its share. GOLDENS: alpha6 + diag_band4.
///
/// 0.0.409-alpha — LONGER FIGHTS, pass 3 (catalog 0.0.161): the crowd's damage RAMPS again,
///   6% of a member's HP per room-hit at stage 1 → 12% at the cap → 15% at 240 (flat 6% had
///   no wall: cap on day 4; 8% flat only slowed the start), and XP per kill is HALVED (user:
///   "xp ×0.5 กำลังดี" — Lv61 on day 7 was the HP that out-sustained every ramp). Seven days,
///   seed 1: stage 37 / 95 / 101 / 109 / 114 on days 1-4 and 7, Lv48. GOLDENS: alpha6 +
///   diag_band4.
///
/// 0.0.410-alpha — TRAINING BREADTH GATE (catalog 0.0.162; user: "5→6 ต้องให้มีทุกค่ารวมกัน 50 /
///   10→11 รวมกัน 100 ... ไม่ให้ sink อันเดียว"): a slot standing on a multiple of `gate_step` (5)
///   cannot go up until every slot's level sums to `gate_total_per_step` (50) × that multiple.
///   `training::gate_open` / `gate_total_needed` / `total_levels`; `training_rules` reports the
///   gate per slot. Goldens byte-identical (nothing buys training in a golden).
///
/// (catalog 0.0.163 — no sim change) CRAFT MATERIALS LEAVE THE DROP TABLES: the 15 herb /
///   ingredient / flask rows (79 rows across 67 monsters) are gone until the craft system is
///   redesigned (user: "เอา item material ออกไปก่อน จนกว่าจะคิดระบบ craft ใหม่"). Recipes stay in
///   the catalog, uncraftable; `recipe_supply`'s two supply guards are `#[ignore]`d with that
///   reason. Sigils, cores, gems and gear are rule-generated and untouched. GOLDENS: alpha6 +
///   diag_band4 (drop tables).
///
/// 0.0.411-alpha — VENDORING RETIRED (user: "เอาระบบขายออกได้" — Gil comes from fusion, there is
///   no junk and no Gil market): `SellAllJunk` / `SellInventoryItem` stay in the enum for old
///   replay logs and are silent no-ops; `pending_junk_gil` is 0; `FUSE_GIL_RETURN_BP` rises to
///   100% (the payout is the income now). sim-cli fuses its surplus and discards the rest
///   instead of selling. GOLDENS: see the bless line.
///
/// 0.0.412-alpha — GIL ×100 AT THE SOURCE (user: "เพิ่ม ×100 ทั้งหมด + gil income ×100 ... ควรทำ
///   ที่ DB ตั้งต้นเลย"): every Gil number is a hundred times what it was — catalog 0.0.164
///   (`sell_value`, `buy_price`, recipe `gil_cost`/`learn_gil`, `gil_reward`, training
///   `base_gil` 50 → 5,000) and the code constants (Potential ladder, recruit, bestiary
///   milestones, the server's MISSION_GIL, `default_gil_reward`). Pace is untouched; the
///   numbers read like money. `GameState.gil_scale` (new games = 1) + `migrate_gil_scale` on
///   the load path lift an old save's balance and its six ledger tallies ×100 once.
///   GOLDENS: every scenario (the new field is on every new game).
///
/// 0.0.413-alpha — PARTY SLOTS FOR GIL, CHARACTERS FROM GACHA (user: "ใช้ Gil ซื้อ party slot
///   ได้แบบไม่ต้องแพงมาก max 5 (ครบได้ใน 1-2 hr) แต่ตัวละคร pull ได้เรื่อยๆ"): `GameState.party_slots`
///   (the deploy cap — never below what is fielded, so a starter pair and an old save keep
///   theirs), `InputEvent::BuyPartySlot` priced by catalog 0.0.165 `party.slot_gil`
///   (5k/8k/12k/15k — the first hour's fusion income), `GilSpend::PartySlot`,
///   `Measures.party_slots_bought` (the server's cue for the slot's 10-pull ticket).
///   `RecruitMember` / `BuyCharacter` are tombstones; `recruit_member` / `buy_character` become
///   the free MODEL paths `model_add_member` / `model_bench_member` (sim-cli, no event). Goldens
///   byte-identical (nothing buys a slot in a golden; `party_slots` 0 is skipped).
///
/// 0.0.414-alpha — FIRST SESSION (docs/first-session-design.md; user: "ด่าน 1 ควรจะสัก 20 คิล
///   แล้วสเกลขึ้นเลย"): catalog 0.0.167 — rooms ask 20 kills on stage 1 rising to 30 by stage 12
///   (was 30 flat), the crowd's damage starts at 3% of a member's HP per room-hit and reaches 6%
///   at stage 12 (a lone Lv3 lead was dying in stage 2's third room), and a wipe's fall-back
///   farms ONE clear of the stage below in the first stages, one more every six, five from
///   stage 24 (`wipe_fallback_clears`; a flat five sent a new player back to stage 1 for fifteen
///   minutes). GOLDENS: see the bless line.
///
/// 0.0.415-alpha — THE STARTER IS SERAPHINE ALONE (user: "ทีมเริ่มด้วย seraphine"): `new_starter_party`
///   no longer fields Rapis — her AoE kit ran the ladder twice as fast as the party it was
///   tuned on, and every other character is a pull. The genesis candidates move with it
///   (client and verifier share the function). GOLDENS: unchanged (they build their own parties).
///
/// 0.0.416-alpha — RELICS, the disenchant sink (`relic.rs`, `docs/relic-system-design.md`;
///   user: "แยกของที่ disenchant อาวุธ / ชุด / เครื่องประดับ / เพชร → unique material ตาม tier ไม่เก็บใน
///   ช่อง แลกเปลี่ยนไม่ได้ → อัพ stat ตามชนิดไอเทม", "T3-T7 ... 5 ชนิด × 4 = 20 unique material"):
///   `InputEvent::Disenchant` destroys tier-3+ gear or a Rare+ gem and credits
///   `GameState.relic_mats[<category>_t<tier>]` (a counter, no bag slot, no market);
///   `UpgradeRelic` spends `mats_per_level × L` of tier `2 + ceil(L/levels_per_tier)` to lift a
///   relic; the levels resolve to `relic_mods` and fold beside Training's. Catalog 0.0.173
///   authors eight relics (two per category). `Measures.items_disenchanted` / `relic_levels`.
///   Goldens byte-identical (nothing disenchants in a golden).
///
/// 0.0.417-alpha — TWENTY RELICS, TWENTY STATS (catalog 0.0.174; user: "20 unique material ต้อง
///   อัพเดท 20 stats ที่ต่างกัน"): a relic is one (category × tier) material's ladder — weapon
///   T3-T7 ATK% / crit DMG / ignore DEF / break DMG / boss DMG, armour HP% / DEF% / regen / dodge
///   / ranged DR, accessory crit / CDR / move / charge / build-up, gem Fire / Water / Thunder /
///   Holy / Shadow damage. `TrainStat` grows the nine names the vocabulary already priced.
///
/// 0.0.418-alpha — AUDIT FIXES (a review of the 0.0.398-0.0.417 work): (1) `party_slots` is
///   OWNED, not read off the field — `claim_fielded_slots` at genesis and on load folds what
///   stands into the field, so benching a member no longer sells the slot (a pre-0.0.413 save
///   that benched two of five could not redeploy them); (2) the stalemate clock keys on the
///   BOSS'S HP FLOOR, not on kills — a boss room has one thing to kill, so every fight longer
///   than a minute after one death was being lost; now only a floor that holds two minutes
///   (`STALEMATE_TICKS` 1,200) fells the party, and a healing boss cannot lift it; (3) relic
///   material credit saturates. GOLDENS: see the bless line (`stalemate_kills` left the state).
///
/// 0.0.419-alpha — REGION 8 SUPPORT CASTS eased (user: "ลดฮีลเหลือ 10% -aspd เหลือ 15%"): the
///   maid's Mending Dirge heals 10% of max HP (was 15%), the banshee's Wail slows 15% ASPD (was
///   30%). The real party stood on stages 84-86 for ten of fourteen days against the pair — a
///   heal-and-slow wall, not a numbers wall. GOLDENS: see the bless line.
///
/// 0.0.420-alpha — RELIC SHARDS (catalog 0.0.175; user: "ได้ 10% เป็นตัวเลขเลยไม่ RNG"): the purse
///   counts SHARDS, `shards_per_material` (10) make one material, so a Common piece is a tenth of
///   a material and a Rare three tenths — deterministic, no draw; every relic price is in shards.
///   `relic_rules` reports both. sim-cli models disenchant + relic lifting (`auto_disenchant_surplus`,
///   `auto_relics`, CSV `relic_lv,disenchanted`).
///
/// 0.0.421-alpha — LUCIELLE IS A UNIT (catalog 0.0.176; user: "ต้องทำ skill Lucielle Acolyte" and
///   the redesign that followed): a pull fields `unit_def` "lucielle" with her kit active — she
///   had deployed as an acolyte carrying three overlay skills nothing activated, and never cast.
///   Kit: Sagitta Lucis (Holy beam basic), LUX MEDICA (Skill 1 — heals the most wounded ally for
///   `power_at(level)` of her ATK, `heal_scales_atk`), Lumen Contagio (Skill 2 — her damage),
///   BENEDICTIO LUCIS as her ULTIMATE (Limit Break-gated, FIXED +30 ATK / +30% Holy to the
///   party; `try_cast_buffs` honours `lb_max` and `resolve_buff_cast` spends the gauge), and
///   COMMUNIO (passive — every cast also heals allies of her element or the Sentinel doctrine
///   for 25% of her ATK, `communio_after_cast` at every cast site). Doctrine Support; board
///   k1/k2/ult authored. An Exemplar line may now LIFT a kit skill it names (star ranks). Every
///   skill scales from ATK except the ultimate (user). Goldens unchanged (no golden fields her).
///
/// 0.0.422-alpha — PER-MEMBER TELEMETRY (display-only, never hashed): `DpsTracker` gains
///   `taken_by_class` / `healed_by_class` beside `by_class`, so a tuning run can say what each
///   unit dealt, healed and took (user: "ต้องวัด damage done / healing done / damage receive
///   แต่ละตัวตอน tune"). sim-cli prints `MEMBERS` at the end of a run and models disenchant as
///   "the odd T3+ pieces fusion cannot use", every minute. Goldens unchanged.
///
/// 0.0.423-alpha — AVELINE STARFLETCHER (Archer · Shadow · Destroyer; catalog 0.0.177; user:
///   "ตัวต่อไปเป็น Archer ธาตุมืด"): a unit with her own kit — Umbra Fletch (Shadow arrows),
///   NIGHTFALL VOLLEY (Skill 1: one arrow on each of the three lowest-HP enemies in range 6, 70%
///   ATK +5%/lv — `volley_targets` and `try_cast_volley`, a new multi-target strike path),
///   SHADOWBIND (Skill 2: 60% ATK + a 120% ATK +10%/lv bleed over 5 s), ECLIPSE RAIN (ultimate:
///   a 5×5 field on the DENSEST knot within 8 — `field_place_densest` / `densest_cell` — 300%
///   ATK, fixed, Limit Break-gated: ground fields now honour `lb_max` and spend the gauge), and
///   TWILIGHT PACT (passive: every cast gives Shadow / Striker allies +10% crit 5 s — Communio's
///   damage twin, same after-cast hook). Doctrine Destroyer; board k1/k2/ult. Goldens unchanged.
///
/// 0.0.424-alpha — AVELINE, second pass (catalog 0.0.178; user: "Passive 1 อันพอ โจมตีศัตรูแล้ว
///   เพิ่ม 5% dark attack debuff max 3 stack / อัลติ scale ได้ + priority boss"): Twilight Pact is
///   gone; Umbra Fletch now stacks an UMBRAL MARK on every hit she lands (`BuffKind::UmbralMark`,
///   +5% Shadow damage taken per hit, three deep, 8 s — `mark_*` passive fields, applied in
///   `resolve_attack_scaled`, read by `shadow_vuln_bp` in `apply_damage` for Shadow packets);
///   Eclipse Rain scales (+20% per level, star ranks re-level it) and lands on a BOSS in range
///   before the densest knot (`densest_cell`). Goldens unchanged.
///
/// 0.0.425-alpha — STEEL BREAK halved (80% ATK +8%/lv, 8 s; was 160% +16%, 5 s): per-member
///   telemetry put Seraphine at 46-52% of the party's damage with uptime equal to everyone
///   else's. `DpsTracker.basic_by_class` splits basic-attack damage per unit. GOLDENS: see the
///   bless line.
///
/// 0.0.426-alpha — NYX (Thief · Shadow · Catalyst), the fourth Exemplar unit (catalog 0.0.180,
///   `unit_def nyx`, thief stat block, dagger). Kit: Umbral Exploit (passive — Shadow basics,
///   `debuff_amp_bp` +10% damage per DISTINCT debuff kind on the target, read in
///   `apply_damage` via `Damage.debuff_amp_bp`), Veil Slash (120% ATK +10%/lv, lays
///   `BuffKind::Darkness` — a magnitude-driven whiff, 20%, HALVED on a boss at apply time,
///   stacked into `avoid_bp` like Blind), Rend (100% ATK +10%/lv + Bleed 150% +15%/lv over
///   5 s), Umbral Requiem ultimate (`zone_wide` — `try_cast_zone_wide` hits EVERY enemy on the
///   board for 200% ATK +15%/lv and applies both `on_hit_debuff` and the new
///   `on_hit_debuff_2`; lb-gated, gauge spent there). `serde` adds a BuffKind variant only
///   (append). Goldens unchanged (no golden fields Nyx; Darkness/amp are 0 for everyone else).
///
/// 0.0.427-alpha — IRON CHAINS caps at TWO targets (`SkillDef.self_aoe_max_targets`, the nearest
///   by Chebyshev then spawn order, applied in `resolve_self_aoe`; user: "Iron chain target upto 2
///   พอ"). Damage, pull and taunt all follow the cap. EVERY KIT DAMAGE SKILL HALVED (user: "ลดเดเมจ
///   ทุกสกิลอีก 50%" — a Sentinel should deal 50-70% of a DPS): Iron Chains 50%+5%/lv, Steel
///   Break 40%+4%/lv, Fireball 100%+10%/lv (the mage's too), Ember Cascade 20%+5%/lv, Fire Cat
///   Meteor 100%+10%/lv, Flame Shield burst 50% ATK, Lumen Contagio 30%, Nightfall Volley
///   35%+2.5%/lv, Shadowbind 30% + bleed 60%+5%/lv, Eclipse Rain 150%+10%/lv, Veil Slash
///   60%+5%/lv, Rend 50%+5%/lv + bleed 75%+7.5%/lv, Umbral Requiem 100%+7.5%/lv + the same
///   bleed, Aegis burst 100% ATK (was 200%). Basic attacks, heals and Darkness untouched. Absorb
///   bursts are now credited to the skill that placed the shield (`source_skill_id`) — Aegis
///   bursts were reported as `flame_shield`, and the Flame Shield cooldown no longer starts when
///   an Aegis shield breaks. Goldens unchanged.
///
/// 0.0.428-alpha — RAPIS BUFF (user: "บัฟราพิสด้วย" — she was 12% of the party after the halving):
///   Fireball 150% ATK +15%/lv every 6 s (was 100%+10% / 10 s), Ember Cascade back to 40%
///   +10%/lv, Fire Cat Meteor 150% +15%/lv (was 100%+10%). AEGIS burst 40% ATK (100% was still
///   22% of the party's damage — a Sentinel should sit at 50-70% of a DPS). Goldens unchanged.
///
/// 0.0.429-alpha — TRASH-ROOM STALEMATE (`GameState.stalemate_kills`, serde default): with no boss
///   up, a member down and the zone kill counter unchanged for `STALEMATE_TICKS` fells the party,
///   the same way the boss-floor rule does. The 0.0.428 run stood 2½ days on stage 97 at 20/30
///   kills — three DPS dead, a Lucielle-healed Seraphine who could neither kill nor die. Goldens
///   unchanged.
///
/// 0.0.430-alpha — MONSTER ENRAGE (`Entity.enrage_ticks`, serde default): a monster alive on the
///   board for 30 s gains +20% ATK every 15 s after that, uncapped (`upkeep::tick_enrage`, one
///   self-sourced `AtkPct` buff grown in place). User: "ให้มอนได้บัฟ DMG+20% ทุก 15 วิ ที่มันมีชีวิตรอด
///   หลัง 30 วิ … stack ขึ้นเรื่อยๆ" — a long fight gets harder instead of standing still. GOLDENS:
///   alpha3/4/6, beta4/5/6/7/9, diag_acolyte, diag_band4 (every fight past 30 s now hits harder).
///
/// 0.0.431-alpha — RELIC → CREST (user: "ใช้คำว่า Relic อาจจะซ้ำ … rename ได้"): `crest.rs`,
///   `GameState.crests` / `crest_mats` / `crest_mods`, `UpgradeCrest { crest_id }`, catalog key
///   `crests` with slot ids `crest_*` (catalog 0.0.181), loc `CREST_*` / `UI_CREST*`, host
///   `crest_rules`. serde aliases keep a 0.0.416-0.0.430 save loading (`relics`, `relic_mats`,
///   `relic_levels`, `UpgradeRelic`/`relic_id`); crest LEVELS keyed `relic_*` in such a save are
///   re-keyed on load. No such save has ever been uploaded (the server predates relics).
///
/// 0.0.432-alpha — LADDER 11-80 (catalog 0.0.182): K reaches 14 by stage 80, J 22% at 50 / 24% at
///   80 and steps back to the old ramp at 81 (user: "ช่วงต้นเกมง่ายไป แทบไม่มีติดหรือ wipe เลย ช่วงด่าน
///   11-80"). Sim unchanged.
/// 0.0.433-alpha — ECONOMY CRESTS (catalog 0.0.183): every category's T3 crest is an economy line
///   (user: "xp gain / gil gain / soul gain / drop rate มาใส่เลเวลแรกของทุกอัน … สลับ drop rate มาที่
///   เพชร"): Scholar +1% XP/lv, Merchant +1% Gil/lv, Spirit +1% soul/lv, Fortune +0.5% drop/lv.
///   New `StatMod::{XpGainBp, GilGainBp, SoulGainBp, DropRateBp}` + `TrainStat` twins, read
///   party-wide off `crest_mods` (`crest::xp_gain_bp` and friends): XP and soul in
///   `handle_monster_death`, Gil in `earn_gil` (every source), drop rate in `drop_on_death`
///   (real entries' weights grow, `nothing` keeps its weight — one draw as before). The element
///   gem crests are gone (a T7 Shadow crest against a T3 Fire one was not a fair board): the
///   gem row is Fortune / Shatter (break dmg) / Mending (regen) / Stride (move) / Grace (heal).
///   Crit 0.2 → 0.1%/lv, CDR 0.25 → 0.1%/lv, charge rate and break build-up 1 → 0.5%/lv (user:
///   "stat สำคัญค่อนข้างเยอะ"). Goldens unchanged (no golden owns a crest).
///
/// 0.0.434-alpha — ULTIMATE GAUGE ×10 (`lb_max` 100 → 1,000 on Aegis, Fire Cat Meteor, Benedictio
///   Lucis, Fons Vitae, Eclipse Rain, Umbral Requiem; every gain unchanged) — user: "ulti charge
///   ไวเกินไป น่าจะต้องช้าลงประมาณ 10 เท่า". A gauge that filled in ~30 s now takes ~5 min alone.
///   The TAKEN charge scales with the blow: `lb_on_hit_taken` is paid in full for a hit of 10%
///   max HP (`LB_HIT_TAKEN_FULL_BP`) and proportionally for lighter ones (user: Seraphine
///   "โดนเดเมจเยอะจนชาจรัวๆ … แต่โดนเบามาก"). Goldens unchanged (no golden owns an ultimate).
///
/// 0.0.435-alpha — REGALIA PASSIVES (`regalia_passive.rs`): the five signature pieces carry a
///   ROLE-keyed passive (ZZZ / HSR shape, user: "ไม่อิงสกิลเป๊ะๆ") — Aurora Edge: taunt held →
///   party −8% damage taken, shield broke on her → +20% ATK 10 s; Empyrean: +3% ATK per enemy on
///   the board (max 5), a break → +30% ATK 8 s; Litany of Dawn: heal +12%, the healed ally +10%
///   ATK 8 s; Meridian: debuffed target +20%, from range ≥4 +10%; Silent Verdict: each status
///   applied → +4% taken (×3), target under half +15%. All `base + per_star × stars`. Hooks:
///   `tick` (sustained stamps, 2-tick buffs), `on_shield_break`, `on_break`, `on_heal`,
///   `on_status_applied`, `hit_bonus_bp` folded in `resolve_attack_scaled`. The signature's
///   advanced stat is its DOCTRINE's (DEF% / Fire DMG / HP% / crit / ATK%). sim-cli `--regalia
///   sig|r[:level[:stars]]` fields them; MEMBERS gains an `ults` column. Goldens unchanged.
///
/// 0.0.436-alpha — REGALIA BY DOCTRINE, not class (user: "ระบบ class ยกเลิกไปแล้ว … ใส่ได้ทุกตัว แต่
///   สกิลจะทำงานเฉพาะตรงสาย Doctrine / stat ได้หมดถึงจะไม่ตรงสาย"): `RegaliaLine.doctrine` replaces
///   `classes`; `regalia_doctrine` / `regalia_line_for_doctrine` replace `regalia_intended_classes`
///   (host: `overlay_doctrine`, `overlay_classes` now empty). A passive fires only when the
///   wearer's `doctrine_for_unit` is the line's; lump and advanced stat apply to anyone. The rest
///   of the class remnants are listed in memory `class-system-retired` for the sweep.
///
/// 0.0.437-alpha — MATK REMOVED (user: "MATK เลิกใช้แล้ว … ล้าง MATK ก่อน"): `effective_matk`,
///   `Item.matk_bonus` / `graded_matk`, `ItemDef.matk_bonus`, `StatOverride.matk`,
///   `BuffKind::{MatkPct, MatkAdd}` (serde aliases onto `AtkPct` / `AtkAdd`) are gone; the
///   catalog folded every `matk_bonus` into `atk_bonus` (max of the two — catalog 0.0.184) and
///   the weapon variance reads the weapon's ATK. Host / bridge drop the `matk` stat; Unity drops
///   the row. GOLDENS: alpha4/6, beta4/6, diag_band4 + catalog copies (staff / mace variance now
///   reads ATK; the mage's Fireball ×4).
///   ULTIMATES: no gauge from damage taken (`lb_on_hit_taken` 0 everywhere — user: Seraphine
///   "ulti ยังเยอะไป"); every damage ultimate ×5 (Meteor 750% +75%/lv, Eclipse Rain 750%
///   +50%/lv, Umbral Requiem 500% +37.5%/lv, Aegis burst 200% ATK); every damage SKILL ×3 (user:
///   "basic attack อัตราส่วนเยอะเกิน … สกิล *3") and Rapis ×4 ("บัฟ Rapis เพิ่มอีก": Fireball 600%
///   +60%/lv, Ember Cascade 160% +40%/lv, Flame Shield burst 150%).
///
/// 0.0.438-alpha — DAMAGE BLOCKED telemetry (user: "ต้องมี stat damage blocked ของ seraphine"):
///   `Entity.soaked_fx` (display-only, serde-skip like `recent_hits`) logs every shield soak with
///   the shield's source; the tick drains it into `DpsTracker.blocked_by_class` keyed by the unit
///   whose skill PLACED the shield, so Aegis on the whole party counts as Seraphine's block.
///   sim-cli MEMBERS gains a `blocked` column. Aegis charges 2 per basic (was 4; user: Seraphine
///   still fired the most ultimates). No state or hash change.
///
/// 0.0.439-alpha — SSR REGALIA PASSIVES UP (user: "ปรับ skill regalia ให้แรงขึ้น"): every base ×1.25-1.7
///   and every per-star step doubled so S5 = 2× S0 (taunt guard 10→20%, shield-break ATK 30→60%,
///   ATK/enemy 5→10%, break ATK 40→80%, heal 20→40% and healed-ally ATK 20→40%, debuffed +30→60%,
///   range +15→30%, status vuln 6→12%/stack, low-HP +25→50%). SERAPHINE NERF ("nerf dmg seraphine"):
///   her BASIC swings at 70% ATK (`Bulwark.basic_power_bp` — `basic_override` now also fires on a
///   passive that only sets element/power; ATK itself is untouched so Aegis and her skills keep
///   their numbers), Iron Chains 100% +10%/lv (was 150%), Steel Break 90% +9%/lv (was 120%). Bench 70/T6/Legendary/100/SSR: Seraphine 18→16% of the
///   party, S5 over S0 +4→+10%. Goldens unchanged.
///
/// 0.0.440-alpha — AVELINE IS A STRIKER (user: "Archer เป็น Striker"): `doctrine_for_unit`, the
///   archer Regalia line is a Striker line (two Striker lines now; `regalia_line_for_unit` names
///   each unit's own SSR — sim-cli `--regalia` uses it). Eclipse Rain ×3 (2,250% ATK +150%/lv);
///   Nightfall Volley −20% (84% +6%/lv) and her basic −20% (Umbra Fletch `basic_power_bp` 80%).
///   Bench: Eclipse Rain 6% → 14% of her damage. Goldens unchanged.
///
/// 0.0.441-alpha — RESONANCE RANKS S1..S6 (`unit_rank.rs`, user: "S0-S6 เหมือน Genshin"):
///   `EXEMPLAR_MAX_STARS` 5 → 6; every star is a fixed effect per unit — S1 opener nudge, S2
///   mechanic, S3 the two skills +3 levels, S4 mechanic, S5 the ultimate +3, S6 signature. The
///   level ranks read at power time (`unit_rank::power`, over the board's cap — the overlay's
///   old "+1 level per star" re-levels are gone, it lifts the kit to Lv5 at S0 only). Hooks:
///   power sites, `resolve_self_aoe` cap, `try_cast_volley`, `apply_on_hit_debuff` shape,
///   Umbral Mark cap, Umbral Exploit, hit-taken DEF stack, shield-break gauge refund, Aegis
///   signature (burst ×2 + room taunt), Fireball crowd cooldown refund, kill explosion,
///   heal splash + healed-ally DEF, Communio-all, Benedictio signature. sim-cli `--stars N`.
///   RAPIS IS A PULL (catalog 0.0.186): an `ExemplarLine` + `exemplar_rapis` carrier — nothing
///   could grant her before, and sim-cli fielded her without an overlay (no base mods, no
///   stars): every measurement to date under-read her. sim-cli `--dummy` (monsters with a huge
///   HP pool and 0 ATK) reads RAW solo output; the webs were sized on it to S6 = +60-100% over S0
///   (user): Seraphine +71%, Rapis +79%, Aveline +62%, Nyx +75%; Lucielle +3% (her ranks are the
///   party's). Goldens unchanged.
///
/// 0.0.442-alpha — RESONANCE sized to "S6 = +60-100% over S0" on the party bench (the S0 kits
///   stay where they are — user: "อัพ S1-S6 ไม่ใช่ S0"): Aveline S2 gauge 8/capped arrow, S4 Volley
///   +2 targets, S6 Umbral Mark 5→10% per stack + Eclipse +100% DMG (the cooldown reset did
///   nothing — 18 ults an hour); Nyx S2 Rend ×3, S4 Exploit +15%, S6 Requiem +80% DMG; Seraphine
///   S4 Aegis +20% ATK to the party, S6 Steel Break +50% DMG. Bench 70/T6/Legendary/168/SSR,
///   S0 → S6: Seraphine 57 → 94M (+65%), Rapis 106 → 173 (+63%), Aveline 93 → 153 (+65%), Nyx
///   91 → 146 (+60%), party 379 → 605M (+59%); S1-S3 ≈ half of it. Goldens unchanged.
///
/// 0.0.443-alpha — STALEMATE WITH NOBODY DOWN (`STALEMATE_TICKS_HEALTHY` 3,000): the breaker's
///   clock now runs for a full-strength party too, on a five-minute window — an S6 party on
///   stage 240 stood an hour killing nothing and losing no one. With nobody down the trash signal
///   is the room's total-HP floor (a slow party still denting the room keeps it moving), and the
///   clock only runs against a live enemy. GOLDENS: alpha6, beta6/7/9, diag_acolyte, diag_band4 —
///   the watched floor is a serialized field, so the hash moves; nothing is felled in any of them.
///
/// 0.0.444-alpha — MONSTERS HAVE NO ELEMENT (user: "มอนไม่มีธาตุ มอนมีแค่ weakness"; a character has
///   an ATTACK element only): `MonsterDef.element` removed (catalog 0.0.187 strips it from all 67),
///   spawned monsters are `Neutral`; `element_mult_vs` has no matrix fallback (no list = 100%);
///   Holy Bane pays vs a target WEAK to Holy; `DmgVsElement` lines (Twinling / Pinkling) pay vs
///   a target weak to that element (`dmg_vs_bp` takes the weakness list); a frozen monster counts
///   as weak to Ice. GOLDENS: all ten scenario goldens (the entity's `element` field is
///   serialized and is now Neutral on every monster) — no fight in any of them changed.
///
/// 0.0.445-alpha — THE CLASS SYSTEM IS GONE (user 2026-09-19: "เอาคลาสออกหมด … ล้าง db เก่าหมดได้เลย"):
///   a character is a UNIT and nothing else. Catalog 0.0.188 drops the ten class stat blocks
///   (`swordsman_t1` … `ranger_t2`); every zone's `unit` is `seraphine`. The ten class skill
///   files (72 defs) and their trees are deleted — Fireball and Flame Shield move to `rapis.rs`;
///   `unit_kit_tree` knows the five units only. `ExemplarLine.classes` → `weapon` (the family
///   is declared on the character; `weapon_types_for_unit` reads the line, `exemplar_archetype`
///   / `exemplar_allowed_classes` / `GATED_IDS` gone, `gated_ids()` iterates the lines);
///   `grant_exemplar_unit` needs a unit def and banks a credit for a planned line instead of
///   standing up a class. `GameState.class_characters` removed with its 0.0.97 fold;
///   `InputEvent::ChangeClass` is a tombstone beside `AdvanceClass`; `data::change_class` gone.
///   The Bloodthirst kill hook went with its skill. Engine hooks keyed on the dead skill ids
///   (traps, pets, spirit strike, purificatio, …) are left in place and inert — a later sweep.
///   Old saves are NOT migrated (the server DB is wiped for this). Loc: the ten `CLASS_NAME_*`
///   class rows dropped; the five unit rows keep the key. Found on the way: `Projectile.skill_id`
///   was `#[serde(skip)]` while `unit_rank::on_aoe_hits` reads it at landing (Rapis S1) — a
///   replay hole since 0.0.441; now serialized (skip-if-empty). GOLDENS: beta9 re-authored on
///   the Seraphine kit (Bash / HP Recovery are gone); the rest move on the catalog / shape.
///
/// 0.0.446-alpha — OFF-HAND KINDS (user 2026-09-19: "จะเพิ่ม tome quiver hidden knife เป็น offhand
///   ใหม่"): `items::OffhandType` {Shield, Tome, Quiver, HiddenKnife} on `ItemDef` / `Item`
///   (skip-if-None, `migrate_item` stamps it from the def); one kind per weapon family
///   (`offhand_for_weapon`: Sword1H→Shield, Staff→Tome, Bow→Quiver, Dagger→HiddenKnife) and the
///   equip gate reads the character's family like the weapon gate does — a shield is Seraphine's
///   now, not everyone's. Catalog 0.0.189: `tome_s1-7`, `quiver_s1-7`, `hidden_knife_s1-7`
///   (nums 571-591, Armor kind in the Shield slot, ATK = half the family's weapon at the tier,
///   shield's shop prices), the seven shields stamped `Shield`; same gear pool, equal weight
///   (user: "อัตราดรอบเท่าๆกัน"). `has_shield` asks for the kind. Host: `offhand_family` on a
///   member, `offhand_type` + `wieldable` on an off-hand item. GOLDENS: whatever equips a
///   shield off a Sword1H unit, and every drop table (21 more defs in the pool).
///
/// 0.0.447-alpha — TRAINING FLAT LINES (user 2026-09-20: "Training น่าจะเพิ่ม flat stat หน่อย
///   ช่วงต้นเกมมันไม่ค่อยได้ใช้"): catalog 0.0.190 puts three slots at the head of Training,
///   priced like Might — Brawn +20 ATK/lv ×30, Stamina +50 HP (5 shown)/lv ×30, Guard damage
///   taken −20 (2 shown)/lv ×20. Guard is a NEW line: `StatMod::DmgReduceFlat` /
///   `TrainStat::DmgReduceFlat`, subtracted LAST in `apply_damage` (after DEF, element, every %
///   bucket), floored at 1 — a third of a stage-1 swing, a rounding error at stage 100. Flat
///   beats % early and % takes over past Lv30-40, which is the point. sim-cli's auto-trainer
///   wants the three. GOLDENS: none (no golden trains).
///
/// 0.0.448-alpha — GEMS ARE BAG ITEMS (user 2026-09-20: "gem ต้องเข้ากระเป๋า"): the 110 gem
///   defs lose `ledger` (catalog 0.0.191) and drop / bank / stack in the inventory like any
///   material. `gems::held_count` / `held` / `take` are the three places that know where a gem
///   lives; `SocketGem` / `FuseGems` read the bag (a bag target is re-aimed when the stack that
///   empties sat before it — that index shift was a latent bug the moment gems had indices);
///   fusion output banks through the loot path. Host `gem_plan.held` reads the bag; the
///   `materials` query no longer lists gems. Load migration `migrate_gems_to_bag` moves a
///   0.0.386-0.0.447 save's ledger gems into stacks (past the cap if it must, never lost).
///   This is what makes Disenchant reach gems: `InputEvent::Disenchant { inv_idx }` now finds
///   them where it always looked — the ledger routing was the bug (0.0.416 wrote `material_of`
///   for gems and an entry point that could not see them). GOLDENS: alpha6 + diag_band4 if a
///   gem dropped in them (bag instead of ledger).
///
/// 0.0.449-alpha — SEALED CACHE, the farming lottery (user 2026-09-21: a very rare box, mostly
///   10 crystal inside, Summon ×1 / ×10 / ×100 as the jackpots with a SERVER-WIDE monthly quota
///   per tier, untradeable, no disclosed odds since nothing about it is for sale): item
///   `lootbox` (catalog 0.0.192, num 592) in the server-rolled `orbs` lane — field 200 bp
///   (0.002%/kill), boss 2,000 bp — bound on arrival (`drops::LOOTBOX_ID`).
///   `InputEvent::OpenLootbox { inv_idx }` takes one and adds to `Measures.lootboxes_opened`;
///   that count is ALL the sim knows. The server (`verify-server/src/lootbox.rs`) rolls each
///   open past the account watermark from a keyed nonce, pays bound crystal / coupons with
///   ledger rows, counts coupon wins against `lootbox_quota (month, tier)` — a win past the
///   quota pays 50 crystal — and announces ×10 / ×100 with a serial. GOLDENS: alpha6 +
///   diag_band4 (an orb row in every field table).
///
/// 0.0.450-alpha — COMBAT SCORE (user 2026-09-21: "ระบบ combat score แบบเกม gacha"):
///   `combat_score::{member_score, member_own_score, party_score}`, display only, integer,
///   nothing in `tick()` reads it. It measures INVESTMENT, not damage (user: "ไม่ว่าจะเป็น sup
///   หรือ tank ควร score ใกล้ๆ dps"): member = [unit(rarity, level) + Σ gear(tier, grade,
///   enhance, lines, gems) + Regalia(rarity, level, stars) + share of Training + Crests] ×
///   stars — the star multiplier on the whole (user: "S1-S6 เป็น multiplier"), SSR S6 = ×2,
///   SR S0 = 65% of SSR S0 and SR S6 ≈ 95% of it (memory `sr-vs-ssr-rule`). Role never
///   enters: five SSRs at the same build read within 2%. Reference bench party ≈ 10K.
///   `items::{exemplar_rarity_index, regalia_rarity_index}` added for it. Host:
///   `party_meta.combat_score`, `stats:<id>.combat_score`; sim-cli `--bench` prints `CP`.
///   NO state change, no golden moves.
///
/// 0.0.451-alpha — NEMU, unit six (user 2026-09-21): SSR · Striker · Dark · staff range 6,
///   succubus tone. Kit (`skills/catalog/nemu.rs`): Sweet Poison (basic override 90% Dark r6;
///   every landed hit stacks the mark: +3% Dark taken, −3% move, −5% ASPD per stack, ×10, 6 s —
///   `mark_slow_pct` / `mark_aspd_slow_bp` on the mark machinery, `BuffKind::{MarkSlow,
///   MarkAspdSlow}` read where the existing slows are), Velvet Bog (3×3 densest-knot field 6 s,
///   slow 50%, 25% ATK +2.5%/lv every 0.5 s), Heartstring (`BuffKind::LeechBleed`: 360% +36%/lv
///   over 6 s, each tick heals the source `tick::LEECH_BP` = 10%), Abyssal Embrace (Enigma's
///   Black Hole: a FUSED 3×3 field 3 s with `field_pull` 1 — hauls every enemy within reach one
///   cell per interval toward the centre and Roots it — then 1,200% +120%/lv Dark on the 5×5 when
///   it closes; `GroundFieldKind::Damage.pull`). Engine: `combat::stamp_mark` extracted and now
///   called from FIELD ticks too (a field tick is a landed hit), field ticks apply `NukeVuln`;
///   DoT ticks (Bleed / Poison / Burn / Heartstring) are CREDITED to the DPS meter per source —
///   they never were, so Rend and Shadowbind's bleed read as nothing. Web S1-S6 in
///   `unit_rank.rs`; SSR Regalia line Rouge Thread / Velvet Mask / Crimson Mirror (Striker,
///   advanced Dark DMG%), passive `Trigger::DebuffAppliedAtk` (+5% ATK per status applied, ×6,
///   S5 +10%) — role-keyed, any Striker. Catalog 0.0.193: unit `nemu`, item `exemplar_nemu`
///   (num 593); gacha STANDARD_POOL / EXCHANGE_POOL / the limited banner feature her. Bench
///   70/T6/168 SSR S0 in Aveline's slot: Nemu 115M (24%) beside Rapis 118M (25%), S6 147M. GOLDENS:
///   alpha6 + diag_band4 (a new unit def / item in every table), DoT credit moves none (meter
///   is serde-skip).
///
/// 0.0.452-alpha — AVELINE IS THE FIRST SR (user 2026-09-21: "Aveline เป็น SR"; rule: an SR's S6
///   ≈ 95% of the same-line SSR's S0, SR S0 ≈ 65%, SR web +40-50%): `ExemplarLine` rarity 0
///   (SR base stats), the carrier item Mythical (catalog 0.0.194), Umbra Fletch 55%, Nightfall
///   Volley 58% +4.2%/lv, Shadowbind 67.5% + bleed 135% +11.25%/lv, Eclipse Rain 1,600%
///   +110%/lv; her web keeps its six effects with the add-ons sized to +49%. Bench 70/T6/168
///   against Rapis S0 129M: Aveline S0 84M (65%), S6 126M (97%). Gacha: she leaves the SSR
///   pools and joins `SR_POOL` (a quarter of SR pulls, beside the fodder) — the 97% tier now
///   pays a real unit. GOLDENS: none (her kit is not in any golden; the item rarity
///   is not in a drop table).
///
/// 0.0.453-alpha — THE TOP TIERS COME OFF THE ANVIL (user 2026-09-21: "ดรอปตรงโอกาสน้อยก็ได้",
///   after the Steam Market survey `docs/steam-market-study-tbh-wog.md`): `GearRule
///   .tier_weight_bp` (catalog 0.0.195: T1-T5 100%, T6 25%, T7 10%) scales a def's ABSOLUTE
///   direct-drop odds by its tier badge, and the T5 band now runs to stage 228 (T6 the band
///   past it, T7 only as its preview). A T6 piece is a rare floor drop and a common fusion of
///   six T5s — which keeps T5 (the tier the population sits on) worth selling to whales instead
///   of flooring at $0.03. GOLDENS: alpha6 + diag_band4 if any golden stage sits past 180
///   (none do) — expect none to move.
///
/// 0.0.454-alpha — THE GEAR CURVE THINS BY TIER (user 2026-09-21: "T6 ดรอปสัปดาห์ละครั้ง T7
///   เดือนละครั้ง" after seeing every tier T1-T5 drop ~400 pieces a day flat): catalog 0.0.196
///   `slot_bp_by_stage` steps down by band — T1-T2 unchanged (15-26 bp), T3 ×0.24 (6-7 bp),
///   T4 ×0.06 (2 bp), T5+ ×0.015 (1 bp / boss 2) — and `tier_weight_bp` T6 1.2%, T7 0.4% on
///   top. Per ~15,000 field kills a day for a five-unit party: T3 ~100 · T4 ~25 · T5 ~7 ·
///   T6 ~1/week · T7 ~1/month direct (fusion raises GRADE, never tier — a T6 only ever drops). The
///   table's scale-up (`k`) now sizes itself in micro-bp WITH the tier factor and is capped so
///   u32 weights cannot saturate. GOLDENS: alpha6 + diag_band4 (stage 1-10 slot odds are
///   unchanged and the rescale keeps every ratio — none moved).
///
/// 0.0.455-alpha — GEAR ODDS ARE FLAT, ONLY THE TIER MIX CLIMBS; MONSTERS DROP GIL (user
///   2026-09-21, correcting 0.0.454: "โอกาสดรอปเกียร์ควรเท่าเดิมทุกเลเวล แต่โอกาสโรลไป tier สูงจะเพิ่มขึ้น
///   ทีละนิดๆ … T1-T2 ควรจะตกเป็นหลักเสมอ"). Catalog 0.0.197:
///   - `slot_bp_by_stage` back to the 0.0.450 values (gear/kill ≈ 2.4-3.2% at every stage);
///     `tier_weight_bp` is gone. A new `tier_mix_by_stage` table (`GearRule::mix_at`) splits
///     that gear roll across tiers by stage — T1-T2 ≥ 75% everywhere, the top tiers a sliver
///     that grows: T5 enters at stage 120 (122 ppm), T6 at 150 (2 ppm → 9 ppm at 232), T7 at
///     232 (3 ppm). Per ~15,000 field kills a day: T3 ~50 · T4 ~20 · T5 ~11 · T6 ~1/week ·
///     T7 ~1/month at the top of the ladder; farming under-level yields only T1-T2, never more
///     gear. Boss tables are the same mix at ×2.
///   - `StageBase.gil` — every kill pays Gil: `10 × (stage/30)^1.2` (min 3) × the monster's
///     `xp_mult_bp`, so a boss pays ×3 and stage 180 pays ~86 a field kill (a base of 20
///     read 1.3M Gil/day on the 7-day, 1.7× the 0.0.450 line; 10 lands on it). Fusion salvage was
///     the ONLY Gil faucet and would have died with the thinner tables — it now returns
///     `FUSE_GIL_RETURN_BP` 20% (was 100%) of the consumed pieces' value.
///   - Fusion Gil return is tiered: T1-T2 pieces hand back `FUSE_GIL_RETURN_EARLY_BP` 50%,
///     T3+ `FUSE_GIL_RETURN_BP` 20% (was 100% flat). A flat 20% took day one from Lv25 / stage
///     46 to Lv18 / stage 21 — kills at stage 1-30 pay 3-12 Gil, so salvage IS the early purse.
///   - `GameState.gear_dropped_by_tier` (serde-skip, `#[serde(default)]`): a loot meter counted
///     where the bank routes a gear piece; sim-cli's bench prints it as `LOOT gear/h by tier`.
///   GOLDENS: alpha6 + diag_band4 (every drop table's weights changed) — re-blessed.
///
/// 0.0.456-alpha — LUMI, THE SR ICE SENTINEL (user 2026-09-21: "Sentinel ธาตุน้ำ … SR ถือเกราะโล่
///   เป็นสาวน้อยขี้ประหม่า … ธรรมดาหน่อย ค่อยไปเสริมให้ S1-S6"). Catalog 0.0.198: unit `lumi`
///   (Sentinel · Water · sword+shield, art key swordsman), `exemplar_lumi` (Mythical, SR), and
///   a second Sentinel Regalia line Snowdrop Charm / Frostbell / Hoarfrost Crown (594-600).
///   - Kit (`skills/catalog/lumi.rs`): Chill Touch — basic 55% Ice, every hit stacks Chill (+2%
///     Ice taken, −4% move, −4% ASPD, ×5) on the mark engine; Frost Taunt — 5×5 80% +8%/lv +
///     taunt 5 s; Ice Mend — heals HERSELF 10% +1%/lv max HP (new `SkillDef.heal_self_only` +
///     `heal_maxhp_per_level_bp` on the support path); Frost Aura (ult) — party DEF +30% +2%/lv
///     30 s and `BuffKind::FrostAura`: an enemy that hits an aura carrier is stamped with the
///     caster's Chill (`combat::apply_hit`).
///   - Hoarfrost Crown (Sentinel-gated): DEF +16% (+3.2%/star, S5 ×2) and `Trigger::UltHealParty` —
///     the wearer's ultimate heals every member 10% (+1%/star) of its own max HP, via
///     `regalia_passive::on_ult_cast`, now called from `unit_rank::after_ult` on EVERY ult path
///     (the pure-buff path and Aegis gained an `after_ult` call).
///   - Ladder (SR, +45%): S1 Chill ×8 + Frost Taunt +10%; S2 Taunt on ≥4 → cd −1 s; S3 +3 lv;
///     S4 Taunt leaves +5% taken 6 s; S5 ult +3 lv; S6 Chill +0.3%/stack, Taunt +5%. Dummy bench
///     70/T6/168 vs Seraphine S0 131M: Lumi S0 83M (64%), S6 121M (92%).
///   - Gacha: `SR_POOL` gains `exemplar_lumi` (a fifth of SR pulls beside Aveline and fodder).
///   - Telemetry: `Entity.mitigated_fx` (serde-skip) → `DpsTracker.mitigated_by_class` — what a
///     member's DEF / reductions / aura took off incoming packets (main hit path + field ticks);
///     sim-cli's MEMBERS table prints `mitigated` and `mit%` = mitigated / (mitigated + taken +
///     blocked). Stage 200: Seraphine 77%, Lumi 82% (Frost Aura + Hoarfrost Crown).
///   GOLDENS: none moved (nothing in a golden wears an aura, casts an ult through a crown, or
///   drops the new items).
///
/// 0.0.457-alpha — DEF% BUFFS LANDED AT 1/100 (found while reading Lumi's mitigation): `DefPct`
///   is summed as BASIS POINTS in `effective_hard_def` (beside gear DEF%), but Iron Chains
///   (+20% → 0.2%), Seraphine's S2 stack (+2%/hit), Lucielle's S4 (healed ally +10%) and Frost
///   Aura authored it in whole percent. Iron Chains now 2_000 +300/lv; the two ranks multiply
///   their `pct` by 100 at the stamp. Stage 200 Seraphine: mit% 77 → 78, taken −27%.
///   Lumi (user: "อาจจะเก่งเกิน … Chill max stack 3, ulti +def 20%"): Chill ×3 (was 5), Frost
///   Aura DEF +20% +2%/lv (was 30). Her ladder re-sized on the PARTY bench (stage 200, the
///   standard four beside her — the dummy read overstated her refund): S1 Chill ×5 + Taunt
///   +25%, S2 Taunt on ≥3 → cd −2 s, S4 +5% taken + Taunt +20%, S6 Chill +0.5%/stack + Taunt
///   +30%: S0 42M (68% of Seraphine's 62M) → S6 61M (98%). GOLDENS: none (no golden casts
///   Iron Chains).
///
/// 0.0.458-alpha — HOLY / SHADOW ARE LIGHT / DARK (user 2026-09-21: "สับสนระหว่าง Dark กับ Shadow
///   ให้ใช้คำว่า Dark / Light ให้หมด"): `Element::Light` (alias `Holy`) and `Element::Dark` (alias
///   `Shadow`) — every save and catalog written before still reads; `parse_element_opt` takes
///   both spellings; the catalog (0.0.199) and the view JSON now say Light / Dark. Loc: the
///   Training attune / ward rows and the affix suffix read Light / Dark; Unity's card-art and
///   bolt-tint tables accept both spellings. Internal names (`UmbralMark`, `holy_vuln_bp`,
///   `shadow_bane_bp`) are untouched. GOLDENS: alpha6 + diag_band4 (the element name is in the
///   serialized drop templates) — re-blessed.
///
/// 0.0.459-alpha — WEEK AUDIT FIXES (user 2026-09-22: "ฝาก audit code ของ 1 wk ที่ผ่านมา"):
///   - Lumi's Chill vuln was an UMBRAL MARK — `stamp_mark` had one kind, so "+2% Ice taken" was
///     +2% DARK taken (it fed Nyx / Aveline / Nemu, never her). `BuffKind::FrostMark` is the
///     Water twin; the mark's kind follows the marker's passive attack element, and
///     `apply_damage` amplifies Water by `water_vuln_bp`. Chill keeps its slows either way.
///   - Frost Aura stamped the mark on FRIENDLY splash too (a party AoE catching an aura carrier
///     chilled the caster): only a hit from the other team counts now.
///   Reviewed and unchanged: the Sealed Cache payout (server-rolled candidate → watermark →
///   bound ledger rows, WRITERS-guarded), the four banners (pity keys, newcomer close, quote
///   ceiling), fusion payout (deterministic from state, no roll), the two new serde-skip fields
///   (display-only, never read by logic), no new PRNG draws in a week, no new server unwraps
///   outside tests. GOLDENS: two NEW — `golden_roster_a` (the launch five at S3) and
///   `golden_roster_b` (Nemu + Lumi at S6), the real party stood up the bench's way against a
///   stage-168 room for five sim minutes (`tests/suite/roster_common.rs`); B moves on the
///   pre-fix Frost Mark code, which is the coverage every golden before it lacked.
///
/// 0.0.460-alpha — SSR LADDERS +100-150%, THE TANK EARNS HER SLOT, GEAR VALUE CLIMBS BY TIER
///   (user 2026-09-22). Catalog 0.0.200.
///   - Resonance (user: "เอาสัก 100-150%" after the HSR read, E6 there ≈ +150-300%): the four SSR
///     damage ladders re-sized on the party bench 70/T6/168 — Seraphine 52 → 110M (+112%), Rapis
///     128 → 256M (+100%), Nyx 124 → 245M (+98%), Nemu 122 → 266M (+117%); the party +82% at S6.
///     Bigger `+X% DMG` add-ons at S1/S4/S6, plus Nyx's Exploit +50%/status (was 15%) and Nemu's
///     Sweet Poison +4% Dark taken per stack (was 1%) — half of both units' damage is the basic,
///     which an add-on cannot reach. SR ladders untouched (Aveline S6 = 98% of Rapis S0).
///   - Aegis (composition sweep: `heal+4dps` beat `tank+heal+3dps` at EVERY stage and the tank
///     blocked a third of what the healer healed back): pool 30% +4%/lv of her max HP per
///     member (was 15% +2%), recast when ANY member stands bare (was: only when her own shield
///     had gone), the gauge fills from blows she TAKES (`lb_on_hit_taken` 80 — the passive's
///     comment always said so, the field said 0). Blocked/h 0.5M → 1.1-1.7M; at stage 168 and
///     200 `tank+4dps` ≈ `heal+4dps` ≈ `5dps` (1,351 / 1,343 / 1,362 kills/h at 168) and the
///     healer still wins at 220, which is the intended shape. Halving the healer was tried and
///     rejected: it moved no ranking, only wipes.
///   - `sell_value` climbs by tier (T1 ×1.0 … T5 ×2.0, T7 ×3.4 — T7 weapon 11,400 → 39,800),
///     fusion T3+ return 20% → 15% (`FUSE_GIL_RETURN_BP`), kill Gil `9 × (stage/30)^1.2` (was
///     10): 7-day Gil/day 780-920K on days 3-7 (0.0.450: 770-810K).
///   GOLDENS: alpha6 + diag_band4 (sell_value is in the drop templates), roster_a + roster_b
///   (ladders + Aegis) — re-blessed.
///
/// 0.0.461-alpha — FIRST NAMES ONLY; PHYSICAL IS THE ELEMENT (user 2026-09-22). Catalog 0.0.201.
///   - Every Exemplar is its first name — `Seraphine`, `Rapis`, `Lucielle`, `Aveline`, `Nyx`,
///     `Nemu`, `Lumi` and the placeholder lines (user: "เอานามสกุลออก มีแค่ชื่อต้นก็พอ"): `EXEMPLAR_LINES`
///     and the carrier items agree (the overlay catalog test holds them together). The epithets
///     live on in the lore rows only.
///   - `Element::Physical` (serde alias `Neutral`; user: "ไม่มีธาตุ Neutral เรียกว่า Physical เป็นธาตุจริง
///     / neutral = ไม่มีธาตุ"): the catalog and the view JSON say Physical; `parse_element_opt`
///     takes both spellings; Unity's card-art map accepts both. Nothing about the matrix moved.
///   GOLDENS: alpha6, diag_band4, roster_a, roster_b (names and the element are serialized) —
///   re-blessed.
///
/// 0.0.462-alpha — AIRI, THE SR THUNDER STRIKER (user 2026-09-22: "นักธนูธาตุสายฟ้า เป็นคู่หูกับ Lumi
///   SR เหมือนกัน … cocky overconfident … Striker … Airi"). Catalog 0.0.202: unit `airi` (Striker ·
///   Thunder · bow+quiver, art key archer), `exemplar_airi` (Mythical, SR), and ONE Regalia —
///   Thunderhead, the SR rung of a Striker line whose other rungs do not exist yet (user: "สร้าง
///   เฉพาะของตัวละครก่อนทีละอัน … SR ก็สร้าง Regalia SR เฉพาะตัวนี้พอ"; `RegaliaLine.ids` takes "" for a
///   missing rung). Lumi's line is cut the same way: Hoarfrost Crown is now her SR piece and
///   Snowdrop Charm / Frostbell are gone (catalog 594-596 → 596 Mythical; 601, 604).
///   - Kit (`skills/catalog/airi.rs`): Static Shot — basic 70% Thunder r6, every hit stacks
///     Static (+2% Thunder taken ×3, 8 s) and +20% on EVERY hit she lands on a taunted target
///     (`SkillDef.taunted_target_bp`, read at the hit-bonus fold beside the Regalia passives;
///     user: the pair with Lumi — she holds them, Airi hits what is held; the shield bonus of one
///     commit is gone); Chain Bolt — 90% +9%/lv on the three lowest in
///     range 6 (no Toughness bonus — Striker, not Destroyer, user); Overload — self ASPD +30% +2%/lv 6 s; Storm Volley (ult) — 350%
///     +35%/lv on the five lowest in range 8.
///   - Engine: `BuffKind::StaticMark` — the mark engine now picks its vuln kind by the marker's
///     attack element (Dark / Water / Thunder); `try_cast_volley` honours `lb_max` (a volley
///     ultimate waits on the gauge and spends it — it used to fire every tick).
///   - Thunderhead (Striker-gated): ATK +12% (+2.4%/star), hits on a target under half HP +20%
///     (+4%/star).
///   - THE STAR RULE, pinned (user: "regalia 5 ดาว ต้องความสามารถ 2 เท่าของ 0 ดาว"): every passive
///     number doubles by S5 — `per_star × 5 == base` for every flat mod and trigger, a test now
///     holds it. Two were wrong: Thunderhead's ATK (120 → 240/star) and Hoarfrost Crown's ult
///     heal (100 → 200/star; DEF was already 320).
///   - Lumi's Chill conducts (user: "Lumi debuff + dmg สายฟ้าด้วย"): a Frost Mark amplifies Thunder
///     damage as well as Water — the Lumi + Airi pair.
///   - Ladder (SR, +46%): S1 +1 target + Chain +20%; S2 a kill charges 25 gauge; S3 +3 lv; S4
///     Storm +20% + Chain +15%; S5 ult +3; S6 Static ×5 + Storm +20% + Chain +10%. Party bench
///     168 beside Rapis/Lucielle/Seraphine/Nyx (Aegis shields on): S0 80M (63% of Rapis S0), S6 117M (92%); beside LUMI (the pair): S0 83M (65%), S6 121M (94%).
///   - Gacha: `SR_POOL` gains `exemplar_airi`.
///   GOLDENS: roster_b (Lumi's Crown is the SR rung now) — re-blessed.
///
/// 0.0.463-alpha — CHARON, THE FIRST DESTROYER (user 2026-09-22; design `docs/character-design-
///   charon.md`). Catalog 0.0.203: unit `charon` (SSR · Destroyer · Dark · MACE + TRINKET, art key
///   swordsman), `exemplar_charon` (Legendary, 613), one Regalia — Ferryman's Lantern (SSR rung,
///   614; ATK +12% +2.4%/★, a break → ATK +30% +6%/★ 8 s), `starting_mace` (605), and the mace
///   family's off-hand `OffhandType::Trinket` — `trinket_s1-7` (606-612, ATK = half the mace of
///   the tier, in the gear pool like every off-hand; user: "Mace offhand เป็น trinket ก็ได้").
///   - Kit (`skills/catalog/charon.rs`): Ferryman's Toll — basic 85% Dark, the oar chips Toughness
///     at ×2 (`SkillDef.basic_toughness_mult`), every hit on a BROKEN target +30%
///     (`broken_target_bp`); Tolling Blow — 3×3 100% +10%/lv Dark, Toughness ×3; Blackwater Mark —
///     boss-first single target (`target_boss_first`) 120% +12%/lv + `BuffKind::BlackwaterMark`
///     10 s: HER hits chip a target that is NOT weak to Dark at half, and a weak one at ×1.5
///     (user's numbers: basic 40 → 20 / 60); Passage of No Return (ult) — 5×5 400% +40%/lv Dark,
///     Toughness ×5, gauge fills from breaks (`lb_on_break` 60).
///   - Engine: a SELF-AoE ultimate rides the strike path on a full gauge (`ActiveCast.ult` —
///     the picker used to skip every ult; nothing else had a path for a point-blank one) and
///     spends the gauge on resolve.
///   - Ladder (SSR +93%): S1 Blow +80%; S2 a kill charges 30; S3 +3 lv; S4 Blow +100%, Mark
///     +120%; S5 ult +3; S6 Passage leaves +20% taken, Passage +550%, Blow +200%. Party bench
///     168 beside Rapis/Lucielle/Seraphine/Nyx (S0 overlays): S0 143M, S6 276M — and the TEAM
///     reads +16% (435 → 505M/h vs Aveline in the seat: a break is a window for everyone,
///     Rapis 133 → 166M). Stage 184 (nothing weak to Dark): 91M, −37% on herself; the seal is
///     what still lets her break there. That is the Destroyer's shape.
///   - Gacha: `STANDARD_POOL` gains `exemplar_charon`.
///   GOLDENS: alpha6, diag_band4, roster_a, roster_b — the trinkets join every drop table —
///   re-blessed.
///
/// 0.0.464-alpha — LAPLUS (user 2026-09-22: "Nyx … ตัวสายขโมย เร่ง gil … เปลี่ยนชื่อเป็น Laplus"). The
///   id stays `nyx` (art keys, `limited_nyx`, saves, goldens); the name is Laplus. Catalog 0.0.204.
///   - Sticky Fingers (passive, id `umbral_exploit`): the Exploit as before, and a monster SHE
///     last-hits pays +20% Gil (`SkillDef.kill_gil_bonus_bp`; 50% read ≈ +18% party Gil, 20%
///     lands the kit at ≈ +9%, the user's "5-10%").
///   - Pickpocket (id `veil_slash`): NO damage — `BuffKind::Dazzle` (a 20% whiff like Darkness,
///     its own kind, boss half) 8 s, and 10% of the target's `gil_reward` paid at once
///     (`pickpocket_gil_bp`, `GilEarn::Pickpocket`).
///   - Grand Heist (id `umbral_requiem`): the room-wide strike now dazzles instead of veiling,
///     rends, and picks every pocket for 10%.
///   - Ladder: the +DMG that sat on Veil Slash moved to Rend (S1 +80%, S6 +100%); the S1
///     dazzle still lasts ×1.5 and spreads.
///   Bench 168: 121M (was ~124M); Gil ≈ +9% for a party that fields her.
///   GOLDENS: roster_a + roster_b (she is in both) — re-blessed.
///
/// 0.0.465-alpha — SUZU + MIREI, LAPLUS' CREW (user 2026-09-22). Catalog 0.0.205: two SR units
///   with one Regalia each at their own rarity (615-618).
///   - **Suzu** (Destroyer · Fire · sword+shield, art key swordsman): Bright Edge — a plain 110%
///     Fire basic; Sunder — 3×3 150% +15%/lv, Toughness ×2, `BuffKind::Sunder` −20% DEF 8 s (its
///     own kind beside `DefDownPct`, so it stacks and counts as one more status for an Exploit);
///     Bell Strike — 190% +19%/lv, Toughness ×2, and its cooldown CLEARS whenever anything on the
///     field breaks (`SkillDef.reset_on_break`, read in `collect_break_triggers`); Break Fever
///     (ult) — 15 s of ASPD +40%, her basic chipping at double, and NO weakness gate
///     (`BuffKind::BreakFever`). Emberbell: ATK +12% (+2.4%/★), a break → ATK +30% (+6%/★).
///   - **Mirei** (Catalyst · Thunder · staff+tome, art key mage): Conductor — a 90% Thunder bolt
///     at range 6 that aims at whoever carries her spark, fires one and does NOT spend it
///     (`spark_keeper`); Storm Field — a 5×5 blast that lays FIVE `BuffKind::StaticSpark`, each
///     worth 150% of her ATK; Quicken — self ASPD +30% +2%/lv; Thunderfall (ult) — the room for
///     500% +37.5%/lv and every spark detonates (`spark_detonate`). A spark fires on ANY landed
///     hit (`combat::fire_static_spark`, also on the projectile path) and is spent by everyone
///     but its layer; damage is booked to HER (`Entity.spark_fx` → the DPS meter, "Static Spark").
///     `Rank::SparkStep` grows the payout. Stormglass Pin: ATK +12% (+2.4%/★), each status she
///     lays → ATK +5% ×4 (+1%/★).
///   - Party bench 168 (Laplus + Suzu + Mirei + Seraphine + Lucielle, S0 → S6): Suzu 86 → 132M,
///     Mirei 84 → 121M (user: "Mirei เดเมจน้อยไปหน่อย" — Storm Field 100 → 140%, a spark 130 →
///     150% ATK, the ladder's SparkStep grown); ≈ 65-70% → 100% of Laplus' S0, the SR band.
///   - Gacha: `SR_POOL` gains `exemplar_suzu` and `exemplar_mirei`.
///   GOLDENS: none moved (no golden fields either of them, and no drop table changed).
///
/// 0.0.466-alpha — SUZU AND MIREI'S LADDERS ARE MECHANICS AGAIN (user 2026-09-22: "+dmg ตรงๆ
///   มันสิ้นคิดไปหน่อย ควรจะสร้างผลพิเศษเพิ่มมากกว่า"; the rule `unit_rank.rs` opens with, which
///   0.0.465 drifted from). One `DmgAdd` left between them.
///   - Suzu: S1 Sunder on ≥3 → cd −2 s · S2 the swept take +15% damage from EVERYONE 6 s
///     (`SelfAoeVuln`) + a kill charges 25 · S3 +3 lv · S4 **Break Fever lasts 5 s longer**
///     (`Rank::BuffDuration`) · S5 +3 lv · S6 **every break extends a live Fever by 2 s**
///     (`Rank::FeverOnBreak`) + Sunder +30%.
///   - Mirei: S1 Storm Field lays **3 more sparks** (`Rank::SparkStacks`) · S2 a spark on a
///     target under 50% HP is **not spent** when someone else fires it (`SparkKeepBelowPct`) ·
///     S3 +3 lv · S4 spark +40% ATK · S5 +3 lv · S6 **Quicken buffs the WHOLE PARTY**
///     (`Rank::BuffParty`, user) + a kill charges 25.
///   - sim-cli `--stars-for unit:N` (repeatable): raise ONE unit's Resonance so a ladder is read
///     as the PARTY's delta — the only way to measure a rank that buffs someone else (user:
///     "แต่จะวัดยังไงว่า dps มาจากไหน"). Party dmg/h at stage 168, one unit starred: Suzu
///     339 → 394M (+16%), Mirei 339 → 396M (+17%), and Mirei's S6 shows up on SUZU's line
///     (87 → 98M) rather than her own — which is the point.
///   GOLDENS: none moved.
///
/// 0.0.467-alpha — RAPIS IS LAPIS, LAPLUS IS RAPLUS (user 2026-09-22). Display names only:
///   the ids `rapis` and `nyx` — and with them every save, banner, art key and golden — do not
///   move. Catalog 0.0.206 + `CLASS_NAME_*` (TH ลาพิส / ราปลัส) + the starter-pair line.
///
/// 0.0.468-alpha — ASTIER, THE SECOND SUPPORT (user 2026-09-23; she was a planned line since the
///   class era — now an SSR Water Support, Lapis' partner). Catalog 0.0.207: unit `astier`,
///   `exemplar_astier` promoted Mythical → Legendary, one Regalia — Tidecaller (619).
///   - Buff design follows the Hoyo supports the user asked me to read (Robin / Ruan Mei /
///     Kazuha): a FLAT team-wide DMG% that multiplies with everything, ONE element rolled from
///     what the party carries, and an ultimate on a DIFFERENT layer so nothing collapses into
///     one number.
///   - Unda Vitae (passive): 50% Water bolt r3, a quarter of its damage heals the most wounded
///     ally (`heal_on_hit_bp`, booked in the damage path and spent by the tick), and every ally
///     she heals carries **+4% damage ×3 stacks** 8 s (`BuffKind::DmgDealtPct`).
///   - Rivus: unchanged (3 bounces, 100/70/50%).
///   - Tidebless (new, replaces Vortex Undae): no damage — the party deals **+20% +2%/lv** for
///     12 s and ONE rolled element **+10%** on top (`BuffKind::ElementDmgDealtPct`, element
///     packed in the magnitude's high half). cd 18 s, so CDR is worth building (user).
///   - Fons Vitae: no longer an instant heal — a party **HoT** of 5% +0.5%/lv of each ally's OWN
///     max HP a second for 8 s, and everyone's gauge charges **+100% faster**
///     (`BuffKind::GaugeRatePct` folded into `charge_limit_break`'s existing rate). Its bespoke
///     caster is retired; the generic buff path owns it.
///   - Tidecaller: HP +12% (+2.4%/★) and `Trigger::HealedAllyDr` — an ally she heals takes 15%
///     (+3%/★) less damage for 8 s.
///   - Ladder: S1 rolled element +10% · S2 an overheal leaves 50% of the excess as a shield ·
///     S3 +3 lv · S4 rolled element +15% · S5 +3 lv · S6 Rivus splashes + Tidebless +6 s.
///   - Party bench 168 (Lapis lead, Suzu / Seraphine / Laplus beside her), her seat vs
///     Lucielle's: **416 vs 374M/h (+11%)** — Lapis 108 vs 90M, Suzu 112 vs 96M — while she
///     deals 32M herself (7%). Stage 220: 493 vs 438M/h and one wipe instead of two, on less
///     healing (1.54M vs 2.16M) because a DMG buff ends fights sooner. S6 alone: 479M/h.
///   GOLDENS: none moved (no golden fields her, and no drop table changed).
///
/// 0.0.469-alpha — THE SUPPORTS EARN THEIR SEAT, AND EVERY ULTIMATE SLOWS DOWN (user 2026-09-23).
///   Catalog 0.0.208.
///   - **Lucielle is an SR** (user: "ปรับ Lucielle เป็น SR ได้ไหม"): `ExemplarLine` rarity 0, the
///     carrier Mythical, out of `STANDARD_POOL` / `EXCHANGE_POOL` / the Limited off-pool and into
///     `SR_POOL` — the party's first healer is now four times easier to pull. Her ladder was the
///     flattest in the game (+3% measured) and is rewritten as mechanics: S1 the plague spreads
///     3×3 + her heals splash, S2 Communio reaches everyone, S4 the blessing is stronger + a
///     healed ally gains DEF, S6 the signature blessing also carries a DAMAGE layer.
///   - **Two measurement findings drove the rest.** Measured against a plain SR DPS in the same
///     seat, her seat read −7% for the party: not the buff's size but its UPTIME — 13 ultimates
///     an hour × 30 s covered 11% of the fight. Benedictio now runs **120 s** (43% uptime) at the
///     same 13 casts. And `AtkPct` in THIS engine is a clean multiplier on total ATK, so it is
///     worth as much as a damage layer — the Hoyo lesson about diluted ATK% does not port.
///   - **Ultimate frequency is capped ~20/h** (user: "39 ครั้งต่อชม เยอะไป … มีคัทอินเดี๋ยวคนรำคาญ"),
///     holding damage by paying in POWER or DURATION: Suzu 91 → 14 (`lb_on_break` 80 → 5, gauge
///     2,000, Break Fever 15 → 100 s), Laplus/Raplus 26 → 16 (gauge per cast halved, Grand Heist
///     500 → 850%), Lapis 23 → 21 (Meteor 750 → 1,400%), Astier 7 → 16 (gauge 900, +4/s).
///     Party dmg/h unchanged (411M before, 411M after).
///   - Astier's Tidebless 20 → 25% and its rolled element 10 → 15%; her S1 rolls a SECOND element
///     and S2/S4 raise the ult's charge rate and the blessing (`Rank::BuffPower`), S6 puts the
///     healed-ally buff on EVERY heal of hers, five stacks deep (`Rank::HealBuffEverywhere`).
///   - TWO BUGS the per-star measurement found, both the same shape — a buff sharing its
///     `source_id` with another of the same kind, which `apply_buff` REPLACES: her passive's
///     +4% was overwriting her own Tidebless (S6 measured 2.5% WORSE than S5), and a second
///     rolled element was erasing the first (S1 was worth nothing). Each now has its own source
///     (`HEAL_BUFF_SRC`, `ELEMENT_BUFF_SRC + element`).
///   - Measured, stage 168, the fifth seat beside a fixed four (control = Aveline): Charon +14%,
///     Nemu +10%, Mirei +0.5%, Astier ±0 (and fewer wipes), Airi −1%, Suzu −4%, Lumi −18%;
///     in the other cast Lapis +2%, Raplus −7%. Raplus pays for hers in GIL: a 3-day economy run
///     reads **+14.5% Gil** for the party that fields her (2.32M vs 2.03M) on +3% kills.
///   GOLDENS: every scenario that fields Lucielle or an ultimate's gauge — re-blessed.
///
/// 0.0.470-alpha — LAPIS' METEOR SLOWS TOO (user 2026-09-23: "Lapis Ulti เยอะไปไหมนะ").
///   She was the roster's most frequent cut-in at 19 casts/h against 12-16 for everyone else.
///   `fire_cat_meteor` gauge 1,000 → 1,300 and power 750 → 975% (+30%) to pay it back:
///   measured 19 → **15 casts/h**, her damage 140.9M → 139.4M and party 424.5M → 419.8M
///   (−1.1%, inside bench noise).
///   GOLDENS: golden_roster_a moves (it fields Lapis); roster_b holds.
///
/// 0.0.471-alpha — ASTIER'S LADDER REACHES THE DPS (user 2026-09-24: S1/S2 read ~+2% each, S6
///   +0.9%). Every new line rides Tidebless, so cooldown reduction keeps its value (user: a
///   longer blessing would make CDR worthless — duration grows 1 s, once).
///   - S1: + Tidebless lasts 1 s longer (13 s of an 18 s cycle).
///   - S2: + Fons Vitae resets Tidebless (`CooldownResetAfterUlt`) + while Tidebless lasts the
///     party's Limit Break gauge fills 50% faster (`Rank::CastGaugeRate`, new — a
///     `GaugeRatePct` rider on its own source so it stacks with Fons Vitae's).
///   - S6: + the rolled element counts for EVERY ally (`Rank::ElementRollAll`, new — the roll
///     lands as a plain DMG% on the same per-element slot). Her heal-buff S6 only reached whoever
///     got hurt, i.e. the tank: S5 → S6 was +0.9% for the party, Seraphine +3.6M and the DPS ±0.
///   - Measured, stage 168, Astier in the support seat, party dmg/h (`--stars 0 --stars-for`):
///     S0 458.5M · S1 478.2 (+4.3%) · S2 505.6 (+5.7%) · S3 512.5 · S4 547.2 · S5 554.1 ·
///     S6 595.6M (+7.5%); the ladder +13.4% → **+29.9%**. Party ults ~16 → ~23/h from S2.
///   - strings.csv: `RANK_cast_gauge_rate`, `RANK_element_roll_all`; 5 rows whose EN text held
///     an unquoted comma (the TH text had shifted into zh_CN) quoted, 3 trailing commas cut.
///   GOLDENS: none fields Astier — none moved.
///
/// 0.0.472-alpha — MAGIC AND SKILLS CRIT (user 2026-09-24: "เวทย์ต้องคริได้ด้วย เพราะเปลี่ยนระบบแล้ว
///   + สกิลด้วย"). The no-crit rule for magic was the MATK era's; with one ATK it only meant a
///   crit support could not reach half the roster.
///   - `resolve_attack_scaled` and the ranged path roll a crit for MAGIC (one draw, off the
///     attacker's crit chance); magic is still never dodged. True damage still never crits.
///   - Skill hits that bypass them crit off their SOURCE (`combat::roll_source_crit`): damage-
///     field ticks, shield bursts (Aegis), Lumen blasts. DoT ticks do not crit.
///   - `HitKind::Crit` (view `recent_hits[].kind == "crit"`, `field_hits[].crit`) replaces
///     `Normal` on a crit blow's event; a crit single projectile now records its event too.
///     `Projectile.crit` is serde-skip (display only).
///   - Measured, stage 168, S0: party 430.1M → 447.9M (+4.1%; Lapis +4.2%, Lucielle +12.7%).
///   GOLDENS: alpha6, beta7, roster_a, roster_b — every one fields a magic dealer, whose hits
///   now draw a crit roll.
///
/// 0.0.473-alpha — SYLVIE, THE SR THUNDER SUPPORT (user 2026-09-24). Easygoing to a fault,
///   forgetful, always smiling. A pure buffer (user: no heal), staff. Catalog 0.0.209.
///   - Spark Tap: 60% ATK Thunder bolt, range 4. Surge: party CRIT +15% and ASPD +10% (+1%/lv
///     each) for 10 s every 16 s. Arc Focus: the top-ATK DAMAGE DEALER (Striker / Destroyer /
///     Catalyst first — a healer out-ATKs a DPS on paper) gains CRIT DMG +50% and ATK +15% for
///     8 s every 14 s. Bolt from the Blue (ult): party CRIT +25% for 15 s and every ally's skill
///     cooldowns −3 s.
///   - Ladder: S1 each crit under Surge charges the critter's gauge +5 · S2 Arc Focus reaches a
///     second dealer · S3 +3 lv · S4 each crit under Surge throws a 30% ATK Thunder spark · S5
///     ult +3 lv · S6 Arc Focus reaches the whole party.
///   - Engine: `BuffKind::CritDmgBpAdd` / `CritGauge` / `CritSpark`; `SkillDef::
///     buff_top_atk_allies` / `party_cooldown_cut_ticks`; `Rank::BuffTargets` / `CritGaugeRider`
///     / `CritSparkRider`; `tick::crit_riders` reads every `HitKind::Crit` of the tick.
///   - Regalia: Static Charm (SR, Support line) — CRIT +8% (+1.6%/star), no trigger. SR_POOL.
///   - Measured, stage 168, Sylvie in a DPS seat beside Lucielle (control = Aveline, 447.9M):
///     S0 442.9M (99%) · S1 +4.6% · S2 +6.4% · S3/S5 in noise · S4 +4.5% · S6 543.8M (+22.8%).
///     A crit-gauge S1 at +20 per crit read +19.5% (every ult doubled) — cut to +5.
///   GOLDENS: none fields Sylvie; the new paths are inert without her.
///
/// 0.0.474-alpha — SYLVIE'S THUNDER SYNERGY (user 2026-09-24: "ควรจะทำให้มี synergy กับตัวอื่น";
///   "อาจจะไม่ใช่ 2 เท่า แต่เป็น 50%"). Surge gives Thunder allies half as much attack speed again
///   (+10% → +15% at Lv1; `SkillDef::aspd_kin_element` / `aspd_kin_mult_bp`). Mirei, Airi and she
///   herself are Thunder. Measured, stage 168, S0: a Thunder party (Mirei · Lucielle · Airi ·
///   Sylvie) 408.9M → 419.0M (+2.5%); the standard party with her unchanged (442.9 → 442.0M).
///   GOLDENS: none fields Sylvie; none moved.
///
/// 0.0.475-alpha — WATER IS ICE (user 2026-09-24: "เรียกว่า Ice ดีกว่า แก้ให้ตรงให้หมด"; there is
///   no element cycle any more, so the name carries no matchup). `Element::Water` → `Element::Ice`
///   with `#[serde(alias = "Water")]` so every older save and catalog still reads; the catalog
///   parser takes both names. Catalog 0.0.210: every "Water" element → "Ice". strings.csv: the
///   Training attune/ward rows and three legacy skill blurbs (which also still argued the retired
///   cycle — "strong vs Fire") read Ice. docs/unity-element-ice-handoff.md: two client lookups.
///   GOLDENS: alpha6, roster_a, roster_b, diag_band4 re-pinned — each serializes an Ice element
///   (a unit or a monster's weakness list); play is unchanged, only the string in the hash moved.
///
/// 0.0.476-alpha — ZOLFIA, THE SSR ICE DESTROYER (user 2026-09-24): the Dulahaness of Duskmourn,
///   won to the party — a headless knight in a cold blue flame, sword and shield (two-handed
///   swords are on their way out). Catalog 0.0.211. Not in any pool: she is to come from the boss.
///   - Coldflame Knight (passive): her hits lay Winter Chill (8 s); a chilled enemy that BREAKS —
///     from anyone — is Shattered: +20% Ice and Fire taken for the stun + 5 s (`tick::winter_pass`,
///     reading the tick's hit stream). Breaks stay weakness-gated (user: a Destroyer that chips
///     anything would make building for the enemy worthless — tried and reverted).
///   - Coldflame Split 350% Ice at ×3 Toughness · Remorseless Winter: an 8 s aura (5×5) — ATK
///     speed −20% and a 45% ATK Ice DoT a second (`tick::winter_aura`) · Winter's Verdict (ult):
///     500% Ice on the whole room at ×2 Toughness. `try_cast_room_nuke` now serves any
///     `SkillDef::room_nuke` ult (Lapis' meteor sets it).
///   - Ladder: S1 every enemy Shattered this RUN adds +10% to Coldflame Split, no cap (user
///     picked it over +20% capped at 10 — the two measured alike); `Entity::shatter_stacks`,
///     serialized, cleared on room 1 of an instance (band advance, retreat, farm loop) · S2 the
///     split sweeps 3×3 · S3 +3 lv · S4 a dying Shattered enemy hands it to the nearest · S5 ult
///     +3 lv · S6 a Shattered enemy inside the aura stays Shattered until it dies.
///   - Regalia: Winter's Oath (SSR, Destroyer line) — ATK +12%, break → ATK, advanced Ice DMG.
///   - Measured (Lapis · Lucielle · Nyx + her; Charon in the same seat): stage 142 (Ice-weak
///     mobs) 416.7M vs Charon 399.1M; stage 168 (none weak to Ice) 455.0M vs 524.3M — she wins
///     where the enemy is weak to Ice. Ladder at 142: party +11.2%, hers +54%; at 168 S6: party
///     +19.5%, hers +77%.
///   GOLDENS: none fields Zolfia.
///
/// 0.0.477-alpha — FARM BANNER, THE SIM HALF (docs/farm-banner-design.md). Catalog 0.0.212: item
///   `farm_box` (tradeable). `InputEvent::OpenFarmBox { inv_idx }` takes one from the bag and adds
///   to `Measures.farm_boxes_opened` (serde default, skipped at zero) — all the sim does; the
///   server mails the box for verified online time and pays each open past its watermark
///   (`verify-server/src/farm.rs`: 1 roll per hour at 4%, 2/day; Zolfia 1% with pity 150).
///   GOLDENS: none moved (the new counter is skipped at zero).
///
/// 0.0.478-alpha — SEND TO STEAM, THE SIM HALF (docs/steam-mint-quota-design.md; D1 cancelled —
///   every account may mint). `ItemDef.steam_itemdef` (catalog 0.0.213: `farm_box` = 1001) names an
///   item's permanent Steam identity; only such an item, unbound, can be sent.
///   `InputEvent::SendToSteam { inv_idx }` takes one from the bag and counts it in
///   `GameState.steam_sent[def]` (serde default, skipped when empty) — the game copy is gone before
///   the server mints (`verify-server/src/steam_inv.rs`), so nothing exists twice.
///   GOLDENS: none moved (the map is skipped when empty).
///
/// 0.0.479-alpha — FARM BANNER TICKETS (user: a box opens into a ticket, pulled on the gacha
///   screen; the box trades, the ticket does not). Catalog 0.0.214: `ItemDef.farm_box` marks a
///   box (one def per event; `farm_box` has it). `OpenFarmBox` now accepts any such def (needs the
///   catalog) and counts it in `GameState.farm_opened[def]` (serde default, skipped when empty);
///   `Measures.farm_boxes_opened` is no longer written and stays only so 0.0.477 saves read. The
///   server credits one bound ticket per open and rolls on `/farm/roll`.
///   GOLDENS: none moved (the map is skipped when empty).
///
/// 0.0.480-alpha — STEAM ITEMS NEVER STACK (user: most games of this kind keep them apart, and
///   one copy in the game should read as one item on Steam). `Item.steam` (serde default, skipped
///   when false) is stamped from `ItemDef.steam_itemdef` by `instantiate_item`, `migrate_item` and
///   every mail grant; `is_stackable` is false for it, so every merge path keeps it single. A
///   mailed ×N lands as N singles or not at all; a saved stack splits on load
///   (`split_steam_stacks`). `DepositItem` refuses one — it trades on Steam only.
///   GOLDENS: none moved (no golden holds a Steam item).
///
/// 0.0.481-alpha — ONE ACTION TIMELINE (user 2026-09-26: a skill fired the tick its cooldown
///   ended and cut the attack animation mid-swing).
///   - SWING-SLOT GATE: the casts outside `attack_step` (heals, party buffs, shields, ultimates,
///     fields, summons, traps — now `tick::out_of_cycle_cast`) wait while a swing is winding up
///     (`atk_acc > 0`) and fire on the tick it would land, taking its place (`atk_acc -= 1000`).
///     Off-target (walking, no enemy) they fire at once, as before. Ember Cascade's queued
///     Fireball is a reaction and stays ungated.
///   - The motion lock no longer charges `atk_acc`: the next swing winds up after the skill's
///     motion, not under it.
///   - `preview_pending_skill` names the out-of-cycle cast a party member's wind-up leads into
///     (a dry run on a clone), ahead of the swing's own skill.
///   - Bench (70:6:legendary:168, sig:60:0): party dmg/h S0 447.9M → 417.0M (−6.9%), S6 749.7M →
///     724.5M (−3.4%); Aveline −14% at both — her basics fell 28%: Volley (6 s) and Shadowbind
///     (10 s) each take a swing now. A shorter motion lock (5 → 3) recovered nothing at S0, and a
///     stronger Volley mostly overkills, so her ARROW takes it: Umbra Fletch `basic_power_bp`
///     5,500 → 8,000. Aveline S0 76.7M → 88.3M (was 89.0M), S6 112.9M → 124.8M (was 130.9M).
///   GOLDENS: beta9, alpha6, roster_a, roster_b, diag_band4 moved (cast timing, Aveline's arrow).
///
/// 0.0.482-alpha — two follow-ups from the first play of 0.0.481 (user 2026-09-26).
///   - "Still an animation cancel sometimes": the swing-slot gate read "engaged" as `atk_acc > 0`,
///     but an ASPD that divides 1000 leaves it at exactly 0 after every swing, and `move_step`
///     zeroes it on the step into range — both let a cast fire the next tick and cut the strike's
///     follow-through. `tick::swing_engaged` = a charge OR an enemy in reach; the gate and
///     `preview_pending_skill` both use it.
///   - Iron Chains takes the two FARTHEST in its ring (user: "ท่าดึง priority ตัวไกล"): a capped
///     caster-centred blast with `pull > 0` picks far-first and hauls near-first, so the nearer
///     one clears the lane.
///   - Bench (70:6:legendary:168, sig:60:0): party S0 424.6M → 427.1M, S6 726.2M → 733.3M.
///   GOLDENS: alpha6, roster_a, roster_b moved.
///
/// 0.0.483-alpha — A WIND-UP IS DECIDED WHEN IT BEGINS (user 2026-09-27: "the basic attack is
///   still cut sometimes"). A skill coming off cooldown halfway through a basic's wind-up claimed
///   that swing, so `pending_skill` appeared mid-wind-up and the renderer swapped the pose
///   (Seraphine: sword row → open-hand row). `Entity.windup_cast` (serde default, skipped when
///   false) is set by `tick::open_windup` on the wind-up's first tick — true only if a skill is
///   ready then (cooldown 0, gauge full) — and gates the swing-slot casts, `attack_step`'s skill
///   pick and `preview_pending_skill`. A skill that comes ready mid-wind-up takes the next swing.
///   - Measured (sim-cli, pending_skill appearing mid-wind-up per hour, S0): 857 → 125
///     (Seraphine 360 → 38, Aveline 218 → 7, Nyx 214 → 3). The rest is mostly Lucielle's heal
///     answering an ally hurt mid-wind-up — a reaction, left alone.
///   - Party dmg/h S0 427.1M → 424.3M, S6 733.3M → 725.0M.
///   - Wind-ups abandoned because the target died (1–2% of swings) are unchanged — nothing is
///     left to hit.
///   GOLDENS: alpha6, roster_a, roster_b, diag_band4 moved.
///
/// 0.0.484-alpha — TRAINING GUARD −20 → −5 PER LEVEL (catalog 0.0.215; user 2026-09-27). Its
///   line read "−2 Damage Taken" while it took 20 off every hit (sim-host showed it on `disp`,
///   but hits on a member read unscaled since 2026-09-18), so a 9-damage blow landed as 1. The
///   host now shows player-side HP lines (flat HP, HP regen, Guard) unscaled, and the user set
///   Guard to −5/lv (−100 at its cap of 20). GOLDENS: none (no golden trains).
///
/// 0.0.485-alpha — THE 3★ REGALIA SET (docs/regalia-three-star-backend-handoff-2026-09-27.md from
///   the UI worktree; user 2026-09-27). Ten R cards, two per Doctrine; catalog 0.0.216.
///   - Four new R-only lines: Last Rivet (Sentinel, HP%), Wayfarer's Bell (Support, HP%),
///     Faultline (Destroyer, ATK%), Striking Stone (Destroyer, CRIT) — carriers `regalia_<id>`.
///   - Rouge Thread's R rung is CATALYST (`regalia_doctrine` override); Velvet Mask / Crimson
///     Mirror stay Striker. Its R Secondary Mod is ATK% 150, Hearthguard's R one DEF% 360 — the
///     SR / SSR rungs of both rows keep their line's stat.
///   - Primary Mod 100 → 1,503 raw (Lv1 → Lv60); Secondary Mod 150 → 374 / 360 → 899 bp.
///   - Passives (base / per star, doctrine-gated triggers; flat lines stay ungated by decision):
///     Hearthguard self-only −5% dmg while taunting (`TauntGuardSelf`), Last Rivet DEF +12%
///     strictly below half HP (`LowHpDef`), Kestrel +6% from 4 cells, Emberwick ATK +3% per enemy
///     (max 3), Cutpurse +6% vs a debuffed target, Rouge Thread ATK +8% 6 s on applying a status
///     (one stack, refresh), Vesper heal +8% (flat), Wayfarer's Bell party ATK +6% 6 s on the
///     ultimate (`BellAtkPct`, `StackPolicy::Strongest` — Bells never add, each keeps its
///     expiry), Faultline ATK +12% 6 s on the wearer's break, Striking Stone Toughness damage
///     +8% (folded into the chip's build-up; HP damage untouched).
///   - FODDER LEAVES THE GACHA (user: "fodder เอาออกจากเกม การ์ด 3* คือใช้แทน fodder ด้วย"): an R pull
///     pays one of the ten cards (server `gacha::R_POOL`), and a 3★ card is Enhance feed worth
///     the R fodder's 5 levels (`regalia_feed_levels`) — never one a member wears, and still a
///     valid target itself. Existing fodder keeps working.
///   - Host: `primary_mod` / `secondary_mod` (+ `_max`) at the carrier's own level, and
///     `passive_active` on `owned_regalia`.
///   GOLDENS: none moved.
///
/// 0.0.486-alpha — REGALIA RESONANCE S1..S5, PASSIVES STRONGER (user 2026-09-27: "ให้เริ่มที่ S1 …
///   Max ที่ S5 ความสามารถพิเศษควรแรงขึ้น … เพิ่ม baseline ของ S1 ทำให้ *2 ที่ S5").
///   - `REGALIA_MAX_STARS` 6 → 4 (its own constant, no longer the Exemplar's): stars 0..4 read
///     S1..S5. A carrier with more stars is read as S5 (clamped at sync and in `describe`).
///   - Every passive: S1 = 2 × the old S0, S5 = 2 × S1 (`per_star × 4 == base`) — EXCEPT the
///     defensive lines (user: "สาย def อาจจะไม่ต้องเพิ่ม"): party / self damage reduction, DEF%, HP%
///     keep the old S0 as S1.
///   - `RangedDmg` pays any RANGED attacker at any distance (user: "โจมตีระยะไกลก็พอ ธนูยิงติดตัว
///     ก็นับ") — `min_range` is gone (Kestrel, Meridian).
///   - Bench (70:6:legendary:168, Lv60 matched doctrines): 3★ S1 341.5M → S5 354.2M; SSR S1
///     460.0M → S5 515.1M (was 422.5M at the old S0).
///   - Host: `resonance_s` / `resonance_s_max` on every Regalia view.
///   GOLDENS: roster_a, roster_b moved (the real party wears Regalia).
///
/// 0.0.487-alpha — NEMU'S SSR REGALIA RENAMED FOR ITS ART (user 2026-09-27: Nemu asleep in a field of
///   red spider lilies): Crimson Mirror → LYCORIS LULLABY, id `crimson_mirror` → `lycoris_lullaby`
///   everywhere ("เปลี่ยน id ด้วยให้หมดเลย … เกมยังไม่มีคนเล่น"). Catalog 0.0.217; a save holding the old
///   carrier is remapped (`DEF_ID_REMAP`); new lore line. Nothing else about the piece moved.
///   GOLDENS: roster_b moved (Nemu wears it; the id is in the hashed state).
///   Same day, catalog 0.0.218 — REGALIA READS "MEMORY FRAGMENT" to the player (user: the cards are
///   moments of the originals; `docs/world-lore.md` §8). Display text only: ten UI loc lines, and
///   the fodder's names Regalia Shard → Memory Dust / Radiant Memory Dust so Fragment and Shard do
///   not read as one thing. Code, ids and keys keep `regalia`.
///
/// 0.0.488-alpha — SSR PASSIVES: TWO EFFECTS, TWO ZONES (user 2026-09-28, after comparing HSR / ZZZ /
///   Genshin: a standard-banner 5★ carries two effects whose multipliers stack; three is for a
///   future limited piece). Every condition is a ROLE event, never one unit's skill.
///   Numbers below are S1 (S5 = 2x S1).
///   - Empyrean: ATK +8%/enemy (max 5) + after a break the WEARER deals DMG +30% for 8 s
///     (`OnBreakDmg`, a buff on the breaker — a window on the broken target alone was too narrow).
///   - Meridian: ranged DMG +15% + vs a debuffed target CRIT RATE +13% and CRIT DMG +30%
///     (`DebuffedTargetCritRate` / `...CritDmg`, folded where the crit is rolled / multiplied). A
///     crit-damage line alone paid only its rate share.
///   - Lycoris Lullaby: + DMG +40% vs a target with >=2 different statuses (`MultiDebuffTargetDmg`).
///   - Tidecaller: + a healed ally deals DMG +20% for 8 s (`HealedAllyDmg`).
///   - Ferryman's Lantern: ATK +14% + after a break CRIT RATE +18% and CRIT DMG +36% for 8 s — no
///     longer a copy of Winter's Oath's ATK line.
///   - FIX: the projectile path (`resolve_projectile`) skipped every regalia hit bonus, the
///     taunted/broken target bonus, boss DMG and the new crit folds since 0.0.435 — ranged wearers
///     were paid nothing by their card. Now folded the same as melee.
///   - SCALED to the growth frame (wearer DMG S1 -> S5: SSR +10-20%, SR +8-15%, 3-star +5-8%),
///     measured card by card on `--bench 70:6:legendary:168 --regalia <id>:60:<S>`: Empyrean x0.75,
///     Ferryman x0.6, Meridian x0.5, Winter's Oath x0.75, Emberbell x0.55, Stormglass x0.6. Result:
///     Empyrean +17.7%, Ferryman +19.6%, Meridian +19.4%, Winter's Oath +19.2%, Emberbell +15.7%,
///     Stormglass +8.4%; 3-star cards +5-9%. Support/tank cards (Aurora Edge, Litany, Tidecaller,
///     Hoarfrost) need a non-DMG metric — not scaled.
///   GOLDENS: roster_a (Empyrean, Meridian) and roster_b (Lycoris) moved.
///
/// 0.0.489-alpha — ENHANCE TAKES MEMORY DUST ONLY; A CARD IS SALVAGED FIRST (user 2026-09-28).
///   - Memory Dust / Radiant Memory Dust (`regalia_fodder` / `regalia_sr_fodder`) are LEDGER counts
///     (catalog 0.0.219 `ledger: true`), not `owned_regalia` carriers: a mailed / bought / pulled
///     Dust lands in `GameState.materials` (`bank_collectible`), and a save's Dust carriers move
///     there on load (`migrate_materials_to_ledger`). Untradeable as before.
///   - `EnhanceRegaliaDust { target_idx, dust_id, count }` spends up to `count` Dust (5 levels a
///     Dust, 15 a Radiant), never past the Lv60 cap and never more than the ledger holds. The old
///     `EnhanceRegalia { target_idx, feeder_idx }` is a tombstone no-op — a card is no longer feed.
///   - `SalvageRegalia { owned_idx }` / `SalvageRegaliaAll {}` (bulk takes only UNTOUCHED 3★ cards —
///     Lv0, S1, unworn, unlocked, `regalia_bulk_salvageable`; user 2026-09-28): each copy (1 + Resonance stars) pays 3★ 1 Dust, SR 1 Radiant, SSR 4 Radiant
///     (PROVISIONAL), and every fed level comes back (Radiant per 15, then Dust per 5). Refuses a worn
///     or locked copy, and an SSR unless another copy of it is already S5 (`regalia_salvage_of`).
///   - `LockRegalia { owned_idx, locked }` — new `Item.locked` (skip-if-false); FeedRegalia refuses a
///     locked feeder too.
///   - Host: each `owned_regalia` row carries `locked` and `salvage` ({dust ids → n}, null when
///     refused); the Dust counts are rows of `materials`.
///   - SIX STAT-ONLY SR CARDS RETIRED (user: "SR รุ่นเก่าลบออกเลย"): Sunbreaker, Nightjar, Farsight,
///     Velvet Mask, Stormglass, Choirlight had no passive, so a 3★ card out-did them. Gone from
///     `REGALIA_LINES`, the catalog and the loc; a save holding one is paid its salvage value on
///     load (`migrate_retired_regalia`: 1 Radiant per copy + fed levels) and a wearer loses the
///     overlay. 24 cards remain: 10 3★, 5 SR, 9 SSR.
///   - Server (same release): R tier back — SSR 3% / SR 10% / R 87%, the x10 floor 3% / 97%; SR =
///     50/50 SR unit or SR card (the Memory Fragment banner: card only); no Dust in any pool; all
///     nine SSR cards in the mileage pool.
///   - GIL SHOP HOLE CLOSED: every unit (20) and Memory Fragment (24) still carried the 0.0.201
///     "Gil stand-in" `buy_price` 1,000, so `BuyItem` minted a unit or card for Gil on prod.
///     Catalog prices → 0 and `buy_item` refuses Exemplar / non-ledger Regalia by KIND, so a price
///     cannot reopen it. Memory Dust stays a 1,000-Gil shop material.
///   GOLDENS: none moved.
///
/// 0.0.490-alpha — MEMORY FRAGMENT ENHANCE IS XP; MEMORY DUST DROPS BY STAGE (user 2026-09-28/29:
///   "เพิ่ม xp มากขึ้น + ให้ dust drop ง่ายขึ้น", then "% แบบนี้ทำให้ abuse ฟาร์มโซนแรก", "ดรอปเยอะไปก็ดูเฟ้อ —
///   dust 100,000 xp เพิ่ม + ขยายหลอดช่วงเลท + ลดช่วงต้น", "อยากให้เยอะแต่ไม่เฟ้อ").
///   - XP to Lv60 (`REGALIA_TOTAL_XP`): 3★ 1.2M · SR 1.6M · SSR 2M. Step L→L+1 ∝ (L + 5)²
///     (`regalia_xp_to_level`): Lv1-20 ≈ 5% of the bar, Lv40-60 ≈ 67%, the last SSR step ≈ 92k.
///   - CAP Lv70 like a character (user: "ตันเลเวล 70 เหมือนตัวละคร … 60→70 steep มากๆ", "ลองสัก 20m"):
///     `REGALIA_MAX_LEVEL` 70, `REGALIA_BASE_CAP` 60. Lv60→70 costs `REGALIA_OVERCAP_XP` 3★ 12M · SR
///     16M · SSR 20M more, each step ×1.4 — the late-game Dust sink (measured endgame: stage 200 with
///     T7 Beyond gear ≈ 920k XP a day at 8 h). Lv70 stats are AUTHORED, not a percentage ("พลังควร
///     predetermine ไม่ใช่ +%"): `REGALIA_LV70` per tier stat (SSR lump 3,600 · adv 26.0% / 11.0%;
///     SR 2,500 · 18.0% / 7.5%; 3★ 1,800 · 10.5% / 4.3%), Lv61-69 on the straight line from Lv60.
///     New `Item.enhance_xp` (skip-if-0); an old save keeps its level.
///   - Three Dust tiers (`regalia_dust_xp`): Memory Dust 1,000 · Radiant 10,000 · NEW Memory
///     Crystal (`regalia_crystal_fodder`, catalog 0.0.220) 100,000. `EnhanceRegaliaDust` adds XP and
///     spends only what the cap needs.
///   - Salvage is XP-valued (`REGALIA_SALVAGE_XP`): 3★ 50k · SR 150k · SSR spare 600k a copy + all
///     invested XP, paid largest tier first (`DustPay` / `regalia_xp_as_dust`).
///   - DROPS (`drops::roll_memory_dust`, ONE new draw on every kill after the Sigil draw, odds by
///     monster STAGE — `memory_dust_odds`): ordinary kills Dust (30+s)·10/100k + Radiant past 80;
///     bosses Radiant (50+s)·10/100k + Crystal past 120. Measured 8 h bench (real five): stage 24
///     65 Dust ≈ 65k XP/day; 60 ≈ 77k; 110 ≈ 108k; 168 158 Dust + 8 Radiant ≈ 238k/day (SSR Lv60
///     in ~8 days of drops alone). The first zone is always the worst XP/hour.
///   - EVERY CDR SOURCE HALVED (user 2026-09-29: "ลองลดอย่างละครึ่งไหม" — the sources summed to ~80%
///     against the 30% clamp): gear substat tiers + line cap 800→400, Valor gem base 60→30 and the gem
///     CDR cap 1,500→750, Training Acceleration 50→25/lv, Apex 40→20/pt, Crest C5 10→5/lv, Card Brutus
///     150→75 a rank, Memory Fragment CDR lines half the small size (SSR Lv70 5.5%). Theoretical max
///     now ~40%. Catalog 0.0.220. GOLDENS alpha6, diag_band4, roster_a/b moved.
///   - CARDS HELD BACK from the first release (user: "card จะไม่ ship ใน patch แรก … ใส่ช่องการ์ดไว้ใน
///     ไอเทมก่อน"): catalog `features.cards: false` (`data::Features`, absent = on). No card row enters
///     a drop table, `sells_for_gil` (the one shop rule, also used by the host's shop list) refuses
///     cards, host query `features` → `{"cards": false}`. Card sockets and the 19 card defs stay.
///     GOLDENS alpha6, diag_band4 moved (card rows left the drop tables).
///   - MEMORY DUST IS NOT SOLD (user: "memory dust ไม่ได้ซื้อได้นะ"): its 1,000 Gil was the 6c8ab845
///     "temporary test price", like the units' and cards'. `buy_price` 0 and `sells_for_gil` refuses
///     every Regalia kind, Dust included.
///   - GIL MARKET CLOSED AT THE SOURCE (cancelled 2026-09-16): `DepositCommodity`, `BuyCommodity`,
///     `DepositItem`, `BuyItemListing` are tombstones, so no trade op reaches `market_ops` and the
///     server's reconcile only ever settles mail claims (the server replays with this sim, so a
///     modified client cannot author one).
///   - GIL MARKET REMOVED: `mailbox_grant`'s trade functions (`deposit_commodity`,
///     `buy_commodity_hold`, `deposit_item`, `buy_item_hold`) are deleted; the four InputEvents and
///     `MarketOp::Deposit/Buy/DepositItem/BuyItem` stay (retired) so old saves still deserialize.
///     verify-server drops its order book / listing ledgers, endpoints and tables; `/save` settles
///     mail claims only and marks any leftover trade op consumed without settling it. No schema or
///     hash change.
///   - ITEMS TAKEN OUT (user: "พวกนี้ไม่ใช้แล้วเอาออกจากเกม"): ASPD potions I-III, the field ration,
///     the inventory expander and the seven unique pieces, with the four recipes that made them and
///     their loc rows. A save still holding one loses it on load (`data::RETIRED_ITEM_IDS`,
///     `remove_retired_items`). The dev battle-pass track pays hp_pot_2 in their place.
///     The `loot_*` consumables followed (Max Potion, Rejuvenating Potion, Phoenix Tail, Monster
///     Lure, Monster Accelerator; user 2026-09-29): defs, loc rows, and on load like the rest. Their
///     buff/revive mechanics stay in the engine, inert.
///   - ANY WEAPON, ANY OFF-HAND (user 2026-09-29): the weapon-family gate on equipping is gone —
///     `Item::can_be_equipped_by` is true for every piece. A weapon or off-hand outside the
///     wearer's family (`is_preferred_by`) keeps `OFF_FAMILY_GEAR_BP` (80%) of its OWN ATK and DEF
///     (`worn_atk` / `worn_def`, read by `effective_atk`, `effective_hard_def`, the weapon spread and
///     the host's stats query); substat lines are untouched. Debug/bench gear still picks in-family.
///   - SMOOTH OPENING (user 2026-09-30: "ไม่ต้อง jump มากถึงแม้ว่า party จะเพิ่ม"): stage HP had a
///     ×6 step at 1→2 and ×1.86 at 9→10 (sized from a measuring run whose party jumped from one
///     member to five). Stages 1-23 now climb geometrically — HP 2,500 → stage 24's 52,534, ATK
///     13 → 70, XP keeping each stage's XP-per-HP — via `tools/build_stage_ladder.py
///     --smooth-opening` (`smooth_opening`, also applied on every full rebuild). Fitted with sim-cli
///     `--gacha-flow`: 15 min ≈ stage 4, 2-3 wipes in the first hour with Suzu alone through
///     stage 1 (first fitted at 3,200, which walled the solo stage-1 boss once slot 2 moved to
///     stage 2 — lowered to 2,500 the same day). Guard:
///     `no_stage_raises_monster_hp_atk_or_xp_by_more_than_a_fifth`. Catalog 0.0.220 digest
///     updated. GOLDENS: alpha6 and diag_band4 moved (they fight on the reshaped stages).
///   - PARTY SLOTS OPEN BY PROGRESS (user 2026-09-30): slots 2/3/4/5 on reaching stage 2 / 6 / 18
///     / 24 (`band_reached`; slot 2 after stage 1's boss), opened each tick by
///     `data::open_earned_slots` and counted in `measures.party_slots_bought` (the server pays slot
///     2 as Lumi, slots 3-5 as ten Starter pulls). `InputEvent::BuyPartySlot` is a tombstone; a
///     save that bought slots with Gil keeps them. Host `party_meta.slot_next` (the stage that
///     opens the next slot, -1 = all open); `slot_cost` is always -1. GOLDENS: diag_band4 moved again.
///   - The catalog's `gacha_coupon` item is gone (a bag item nothing paid or spent since coupons
///     became a server counter); the server's two ticket kinds replaced the counter of that name.
///     `RETIRED_ITEM_IDS` drops any copy on load.
///   - POTENTIAL GIL, EARLY RANKS (user 2026-09-30): the ladder was priced when Gil income was
///     higher (vendoring, fuller fusion salvage) and ×100'd with income in 0.0.412; income fell
///     after, prices did not. The first node (Root / T1 stat / skill rank 2) is 10,000 (was
///     100,000), and skill ranks 3-10 climb ≤ ×2.5 a rank (25k, 60k, 150k, 300k, 600k, 1.2M,
///     2.4M, 5M) to rejoin the old ladder at rank 11. Sigil / Core / Track unchanged — Sigil I of
///     the unit's element stays the early bottleneck (2-3 by hour 6), covered by a tutorial quest.
///     Guard: `skill_rank_gil_never_jumps_more_than_two_and_a_half`. Goldens unchanged.
///   - TUTORIAL COUNTERS (2026-10-01): `Measures` gains `items_equipped`, `regalia_equipped`,
///     `regalia_enhanced`, `regalia_salvaged` and `potential_ranks` (skip-if-zero, so an old save
///     serializes unchanged), read by verify-server's tutorial quests. `soul_nodes_allocated` is
///     the Paths of Soul web, not Potential, plus `party_provisions_loaded`. GOLDENS: alpha4,
///     roster_a, roster_b moved (they equip gear / Regalia).
///   - EXP ELIXIR (user 2026-10-01): `exp_elixir` (num 633), a Party provision giving `BuffKind::XpPct`
///     2,000 bp for 36,000 ticks (1 h). Kill XP takes the party's HIGHEST XpPct after the XP crest,
///     never a sum. Sold for crystal by verify-server's `/shop` (120, 10 a day).
///   - CRAFT HELD BACK, MEALS CUT (user 2026-10-02): catalog `features.craft: false` makes
///     `LearnRecipe` / `CraftItem` no-ops and keeps craft-only materials off the shop (recipes stay
///     authored); the eight Meals, their eight recipes and ten Meal-only ingredients leave the
///     catalog and `RETIRED_ITEM_IDS` (`ProvisionKind::Meal` stays in the engine, inert).
///   - CREST MATERIAL ENVELOPES: `ItemDef::crest_material` (`crest_mat_<category>_t3`, nums
///     634-637) — `bank_collectible` credits `crest_mats` with `shards_per_material` per unit, so a
///     server mail can hand out Crest material (the tutorial's disenchant quest).
///   - OFFLINE EXP (user 2026-10-02): `tick::offline_xp_per_member` = band base XP × the FIVE-member
///     share (40%, whatever the party size — user) × 528 an hour (80% of the ~660 sim-cli measured online, 3 seeds × 24 h), 10 min - 10 h;
///     `ItemDef::xp_envelope` (`offline_exp`, num 638) pays it to every fielded member on arrival.
///     verify-server mails one when a save lands ≥10 min after the previous one.
///   - AUDIT FIXES (2026-10-02): `GameState.band_cleared` (highest band whose boss died) and
///     `band_travel_max()` = max(reached, standing, cleared + 1) gate SelectBand AND EnterInstance —
///     `reached + 1` chained to any band in one tick, and EnterInstance took any `zone_b<N>_1`,
///     while `band_reached` now pays slots, CXP and offline EXP. EquipRegalia on a card another
///     member wears moves it and clears the old wearer's overlay. GOLDENS: diag_band4 and
///     roster_b moved (the new field serializes once a band is cleared; verified hash-identical
///     with it skipped). `tick::preview_pending_skills` previews every member's wind-up from ONE
///     probe clone (a clone per member per view was ~1 ms each on a 1 MB state); a test pins it to
///     the single form tick by tick.
///   - AUDIT FIXES, second pass (2026-10-02, range 2026-09-11..25): `Projectile.display_hits` /
///     `crit` are SERIALIZED (they gate an impact's HitEvent — crit riders, `biggest_hit`, Winter
///     Chill — so a save mid-flight diverged on replay); `ToggleActiveSkill` is party-only (it could
///     switch a boss's actives off); `SocketGem` re-aims by `take`'s exact stack order (a bound
///     single ahead of the piece sent the gem to the next item); `buy_item` checks room with
///     `can_bank` (a bound stack does not take a purchase); `StatusVuln` debuffs on its own source
///     `REGALIA_VULN_SRC` (it shared the break / nuke vuln slot); a benched wearer's Regalia overlay
///     follows feed / enhance; `EquipItem` takes Weapon / Armor only. GOLDENS: beta7 + roster_a
///     (projectile fields serialize — hash-identical with them skipped), roster_a + roster_b
///     (StatusVuln no longer stacks onto the break vulnerability — intended).
///   - WHOLE-PROJECT AUDIT (2026-10-03). COMBAT (balance moves — bench lv70/T6/stage 168 party
///     dmg/h 464M → 538M, ults/h Lucielle 12 → 25, Rapis 14 → 20, Aveline 11 → 15):
///     `combat::land_blow` is the landing half of every blow, and a PROJECTILE impact now runs it
///     (shields soak ranged hits, ranged hits chip Toughness / break, marks stack, sparks fire);
///     `Projectile.element` is serialized for the chip; a landed ranged BASIC charges
///     `lb_on_basic`; a killing blow no longer also breaks. Buff sources split: Regalia heal
///     triggers on allies `REGALIA_ALLY_SRC`, `HEAL_BUFF_SRC` off `RANK_SELF_SRC`'s value, Lucielle
///     S6 `RANK_SIGNATURE_SRC` (no 0% stamp), Seraphine S4 Aegis `RANK_AEGIS_SRC`, an ultimate's
///     party buff of a kind another of the caster's skills casts on `id | ULT_SRC_BIT`, the break
///     vulnerability `BREAK_VULN_SRC`. A stunned / frozen caster's channel holds; a boss-halved
///     Darkness spreads at full strength to non-boss neighbours; a dead attacker stops swinging.
///     EXPLOITS: LeaveInstance falls back to the party's reached band (not band 10); EnterInstance
///     only at a chain's first room; RespawnInZone refused on a cleared zone; summoned familiars
///     (`Entity::is_summon`) pay nothing; server grants bank past the bag cap (waiting let their candidates expire);
///     EXP Elixir reads the strongest single XpPct buff (rack cycling summed them); one ClaimMail
///     per mail id; Equip/EquipCard party-only; party name ≤ 24 chars; one click per tick incl.
///     tick 0 (`last_click_tick` holds tick + 1); retired items leave escrow; a run-ending clear is
///     handled once. `SaveBundle.sim_build` (`SIM_BUILD`). GOLDENS: alpha6, beta7, roster_a, roster_b.
///   - ELEMENT-LESS SIGIL (user 2026-09-30: "เอาธาตุออก ให้ sigil ไม่ต้องมีธาตุ"): the eighteen
///     `sigil_<element>_<n>` defs become three — `sigil_1` / `sigil_2` / `sigil_3` (catalog nums
///     630-632). `potential::sigil_def(grade)`, `Purse.sigil` is per grade, `can_pay` / `pay`
///     take no element, and every unit spends the same Sigil. The drop keeps its draw and its
///     gate (a first weakness in the chart), so the total dropped is unchanged — only the id is
///     generic. `MergeSigil` / `SplitSigil` still parse `element` and ignore it. Load folds every
///     old element Sigil (ledger or bag) into the generic one of its grade
///     (`potential::legacy_element_sigils`, `migrate_element_sigils`). Host purse `element` is "".
///     The Farm Banner's Sigil pays by tier only.
///   - Host: `owned_regalia` rows carry `enhance_xp`, `enhance_xp_next`, `enhance_xp_to_max`;
///     `salvage` lists all three tiers. sim-cli `--bench` prints a DUST line.
///   GOLDENS: every kill golden moved (the new draw) — alpha3/4/6, beta4/5, diag_band4, roster_a/b.
///
/// 0.0.491-alpha — THE AUDIT FIXES (2026-10-02/03), listed as the "AUDIT FIXES" and
/// "WHOLE-PROJECT AUDIT" bullets in the 0.0.490 entry (written while 0.0.490 was unreleased).
/// Bumped because the serialized shape moved (`Entity::is_summon`, `Projectile::element`;
/// `band_cleared` and `Projectile::{crit, display_hits}` already shipped in 0.0.490) and combat outcomes moved (`combat::land_blow` on
/// projectile impact, buff source splits), so a 0.0.490 client must not replay against this sim.
/// Also: `SIM_BUILD` (a digest of sim-core's source) for the save gate; the op digest is built
/// from claim values, not serialized bytes. No golden moved for the bump itself; the re-audit's
/// combat fixes (a dead firer's projectile no longer lands, a summon kill runs only the corpse
/// effects, a channel holds through a stun) moved `golden_roster_a` / `golden_roster_b`, re-pinned.
/// Lucielle's `lb_on_basic` 5 → 2: her ranged basic now charges the gauge, which took her from 12
/// to 25 ults an hour; now ~17 (Aveline stays 2 at ~15; Rapis's rise is not basic-driven).
/// Bench (`--bench 70:6:legendary:168 --regalia sig:60:0`): S0 538M → 514M dmg/h (fewer buff ults).
/// Server-side: Sealed Cache / Farm Banner win announcements REMOVED (user 2026-10-03).
/// Re-audit 2: a projectile whose firer died in flight lands through the victim's shields;
/// `SaveBundle::verified_state_via` (a re-send from an older base that passes through the latest
/// save is a continuation); `SIM_BUILD` hashes code only (non-test `.rs`, comments stripped).
/// Server: finalize claims the order `charging` before Steam; a refund-voided mail settles for
/// 30 min, then is refused unflagged; later monthly windows shift on a refund; `granted_at`
/// backfill reads the purchase ledger; post-commit state moves before any await; the one-time
/// trusted prune spares accounts active in the last hour; stored op digests cleared once;
/// personas persisted. No golden moved.
/// Re-audit 3: `verified_state_via` checks around inputs recorded AT the earlier tick; byte raw
/// strings and `'\''` in the `SIM_BUILD` lexer. Server: an ambiguous FinalizeTxn failure stays
/// `charging` (202) for the QueryTxn sweep, which runs on its own loop; admin grant of `charging`
/// needs force; claims already in the verified base are exempt from the withdrawn-mail window; a
/// continuation keeps its old base trusted; fusion consumes, the offline gap (`offline_owed`
/// table, zeroed by the paying accrual) and refilled op digests commit with the save.
/// Re-audit 4 (server/tools only): offline gap measured from the latest save to the resuming
/// login; tainted saves held to the withdrawn-mail window, forgiveness only for claims stored in
/// time; fuse rolls answered only once durable; premium re-checked under lock in /bp/claim;
/// lifetime-excess load fails boot; test DB guard reads ?host=; lootbox quota rows locked in one
/// order; release_check / check_online_sync compare SIM_BUILD; tools/rollback_to_66d8c875.sql.
/// Re-audit 5 (server/tools only): the login's absence is recorded durably at GET /save
/// (`offline_handshake`) and paid by the first save continuing from it; stored claims are forgiven
/// the withdrawn window unconditionally; a capped SKU's cap is counted and the order claimed under
/// one per-(account, SKU) lock; a Steam reply without an explicit `result` is ambiguous; logins,
/// bans and evictions share an advisory lock and one memory swap; chat sockets close with their
/// token; every per-account money transaction locks the account row first; a consumed fusion
/// nonce is never re-opened; rollback script restores `commander.outpost_claimed_at_ms`.
/// Re-audit 6: `SetPartyName` is cut to 24 characters BEFORE it is recorded (replay-identical).
/// Server: chat frames capped at 4 KiB and the trimmed line broadcast; bundles with a checkpoint
/// spacing below `SNAPSHOT_INTERVAL_TICKS` refused before replay, and replays / the fusion
/// bind-check run in `spawn_blocking`; the login's absence is keyed on the save it measured
/// (tick + saved_at); boot does not restore a banned account's session; login answers 503 on a
/// database failure (403 only for a ban); the test DB guard decodes `%xx` keys and checks PGHOST.
/// Re-audit 7 (server only): the offline gap is capped at wall time since the save minus what the
/// bundle played, and the row and memory share one stored-at time; replays hold a server-wide
/// permit (cores) for their whole run and a server-side replay failure answers 503 unflagged;
/// a per-account replay budget (3 full spans / 10 min, rejected saves charged); request bodies are
/// read before the in-flight permit, large ones under 32 permits; cheat_flags rows deduped per
/// (account, reason) per hour; crystal debt answers 409; continuation bases persisted to `trusted`.
/// Re-audit 8 (server/tools/FFI; no sim change): offline EXP is paid AT THE LOGIN (GET /save,
/// `pay_offline_at_login`, `offline_paid.through_ms`) — the measure-at-login / pay-at-save design
/// and its `offline_handshake` / `offline_owed` tables are gone; GET /save waits for a running save
/// (429). Chat removed (route, sockets, moderation, personas). Weekly turns over Monday 04:00 TH;
/// the shop cap at 04:00 TH. Refund reconcile walks only money-taking orders, least recently checked
/// first. Body reads hold no shared permit (per-IP cap of 4; large-body permit taken after the
/// read). Replay budget refunded when the server failed the save; a fast-forward against a
/// boot-synthesized anchor answers "reload" unflagged. sim-ffi exports `roi_sim_build()`.
/// Sim: a DUNGEON (`zone_dungeon_*`) is entered only once its first room's stage is within
/// `band_travel_max()` (user 2026-10-03: Duskmourn, stage 111, was open to a new account). Story
/// chains are gated by the previous chapter on the server / story screen, not here. No golden moved.
/// Re-audit 9 (server only): an idempotent re-send keeps its replay charge (refunding it allowed an
/// unlimited replay loop); a boot-synthesized tick anchor is honoured for `BOOT_ANCHOR_TTL_MS`,
/// replaced by the first save accepted against it, and dropped at login; request bodies reserve
/// their declared size from a 64 MiB `BODY_BUDGET` before reading and must arrive at ≥ 32 KiB/s
/// after 2 s; weekly ids `(day + 4) / 7` (never below the old ids); the login payout reads the
/// save's stored-at from its row.
/// Re-audit 10 (server only): with no live anchor a save is measured from the account's latest
/// stored tick at now (falling back to tick 0 locked out a client that slept through a restart);
/// request bodies are charged by bytes actually buffered (64 KiB units, ≤ 4 MiB per IP) and a GET
/// may not carry one; /dev/bp-season refuses while a season runs and ignores slot_cost in prod.
/// Re-audit 11 (server only): the no-live-anchor fallback (stored tick, now) is installed in
/// `tick_ref` as a counted session anchor — left uncounted, a login that skipped GET /save got 300
/// free ticks per cycle, never flagged and never in `lifetime_excess`.
/// Re-audit 12: that fallback is marked `synthesized`; a layer-1 failure against it answers
/// "reload" unflagged (a client that ran through a long outage reaches it honestly), while the
/// saves it accepts stay counted by layer 2.
/// Sim: `AddPartyProvision` of an item already racked with the same binding REFILLS that slot in
/// place (key, `ready_tick` and its live buff untouched), even when the rack is full; a different
/// binding is refused. Before, any duplicate was a no-op. No golden moved.
/// Server-side in the same round (verify-server, no sim effect): refund reads the order's amounts
/// under its lock; a monthly refund takes back only that card's window; a refund before delivery
/// keeps a capped SKU open; finalize refuses closed orders and never re-finalizes `grant_failed`;
/// a new checkout retires the account's own stale one; Steam-unreachable mints do not count toward
/// the refusal cap; the season is re-read hourly and every XP / claim / badge read asks whether it
/// is still running; chat and announcements show the Steam persona, never `steam:<id>`; refunded
/// pack and premium-BP mail is voided, not deleted; login answers `game_data_version` + `sim_build`.
///
/// 0.0.492-alpha — UNIT IDS FOLLOW THEIR NAMES (user 2026-10-04). `rapis` → `lapis`, `nyx` →
///   `raplus` (display names since 0.0.467), with `exemplar_rapis` / `exemplar_nyx` →
///   `exemplar_lapis` / `exemplar_raplus`. Catalog 0.0.221; `CLASS_NAME_*` / `ITEM_LORE_*` keys
///   renamed; the gacha pools in verify-server follow. `data::migrate_save_items` moves an old save
///   (`items::RENAMED_UNIT_IDS`: field + bench unit defs and overlays, the zone unit, banked
///   Resonance credits — merged onto the new key — and every `exemplar_<id>` item);
///   `DebugGrantExemplarUnit` reads an old id through `items::current_unit_id`. The ids are in the
///   hashed state, so `golden_roster_a` and `golden_roster_b` moved (no behaviour change). Sprite
///   art keys (`thief`, `mage`, `rapis_Idle`) are skin ids, not unit ids, and do not move.
///
/// 0.0.493-alpha — THE STARTER IS SUZU (user 2026-10-12, matching `docs/gacha-economy-design.md`'s
///   onboarding): `new_starter_party` fields `data::STARTER_UNIT` ("suzu") alone instead of
///   Seraphine (0.0.415). Client and verifier share the function, so a new account's genesis moves
///   with it; existing saves are untouched. `sim-cli` without `--lead` now starts with Suzu.
///   LANE MODE (opt-in, dev only; `docs/lane-combat-design.md`): a group spawns melee in front,
///   shooters at the back, and walks in as one block to the cell beside the party's front column
///   (the crash); after it any unit with no target in range walks out to the nearest enemy; a
///   monster shooter stops once in range; moves with no cell change keep `move_acc`. GOLDENS:
///   unchanged (they build their own parties; lane mode is off in all of them).
///
/// 0.0.494-alpha — MONSTER SUPPORT SKILLS GET A USE BUDGET, SAME STRENGTH (user 2026-10-19):
///   Dustelle's `mending_dirge` casts at most TWICE per spawn (was every 10 s), Ashelle's
///   `frenzied_rite` ONCE per spawn (was every 15 s), Banshee's `wail` ONCE per spawn (was every
///   15 s). Their numbers (10% max-HP heal, +30% ASPD, -15% ASPD) are unchanged. Brutus's feral
///   transformation now triggers below 30% HP (was 50%). `max_charges` existed, but the support-heal,
///   buff and active-attack cast paths never read it. All three now gate on `has_charge` and spend
///   on cast: a buff after its chance roll, an attack where its cooldown latches. No other
///   heal, buff or attack skill sets `max_charges`. GOLDENS: unchanged (no maid, banshee or wolf in
///   them).
///
/// 0.0.495-alpha — SLOWER LATE LADDER (user 2026-10-21: a 24 h/day F2P player should reach stage
///   ~200 in ~2 months, not 25-44 days). (1) EXP needed per level ×2 (`XP_CURVE_MULT`, applied in
///   `xp_to_next`): every source halves, offline EXP included since it is paid in level XP; Apex is
///   unchanged. Alone it barely moved stage pace (Lv is not the gate). (2) Catalog 0.0.224:
///   `stage_base` hp and atk after stage 120 grow a flat 1.5% per stage from stage 120's row (the
///   old table grew ~1%); def / toughness / xp / gil unchanged. Measured, `sim-cli --gacha-flow`
///   60 days × 3 seeds: stage on day 60 = 175 / 198 / 213 (old table: 200 on day 25-44); 2.0% was
///   also run and no seed passed 181. The 169-180 wall (end of gear tier 4) is kept as is.
///   GOLDENS: `golden_roster_a`, `golden_roster_b`, `golden_beta5` and `diag_band4` moved (level-ups
///   come later); `golden_beta5` now carries 420 XP past Lv3 (was 860).
///
pub const VERSION: &str = "0.0.495-alpha";

/// Git identity of the SOURCE this build came from (build.rs; `-dirty` when
/// build-affecting paths had uncommitted changes; "unknown" outside git without
/// the SIM_GIT_SHA env). Deploy/sync tooling compares this across client dylib
/// and server so "same VERSION, different code" skew can't ship silently.
/// The "SIMGIT:" prefix makes it extractable from a compiled dylib via `strings`.
pub const GIT_STAMP: &str = concat!("SIMGIT:", env!("SIM_GIT_SHA"));

/// Digest of sim-core's own source (`build.rs::sim_build_hash`): equal on client and server
/// exactly when they replay identically — the save gate compares it, not the git SHA.
pub const SIM_BUILD: &str = env!("SIM_BUILD_HASH");

/// Placeholder retained for the ALPHA-1 bridge smoke test. Real state
/// hashing goes through `sim_core::hash::state_hash(&state)`.
pub fn state_hash_placeholder() -> String {
    "alpha".to_string()
}
