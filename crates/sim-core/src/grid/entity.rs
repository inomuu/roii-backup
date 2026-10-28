//! `Entity` — the symmetric simulation entity (players + monsters).

use serde::{Deserialize, Serialize};

use super::{Cell, DamageType, Element, Team};
use crate::items::{DropEntry, Item};
use crate::stats::Stats;

/// One simulation entity.
///
/// Fields are grouped by concern. **Field order is part of the state
/// hash** (canonical JSON serializes in declared order) — don't
/// reshuffle without a version bump.
/// One absorb shield on an entity (Flame Shield, future Praesidium). A pool of
/// soakable damage that, when emptied, EXPLODES for `burst_dmg` Fire in a 3×3 +
/// `burst_knock` knockback, credited to `src_id`. Shields stack as a FIFO list on
/// the entity; damage drains the oldest first. Transient (never serialized).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbsorbShield {
    /// Identifies the spell which created a shield for conditional passives.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_skill_id: String,
    pub pool: i32,
    pub burst_dmg: i32,
    pub burst_knock: i32,
    pub src_id: u32,
    /// Hit-count cap (Bishop Praesidium): the shield also ENDS after it has
    /// absorbed this many separate BLOWS — pool OR hits, whichever empties first.
    /// 0 ⇒ unlimited hits (Flame Shield, capped only by its pool). A LIVE shield
    /// with `hits_left > 0` is a Praesidium (the bridge uses this to tell the two
    /// shield types apart for the buff icon).
    pub hits_left: i32,
    /// RESERVED (set but not yet read): would let a shield survive even a NEW-RUN
    /// transition. Today ALL shields share one lifetime — they carry across a
    /// mid-instance room hop and clear on a new run or death (`build::zone`),
    /// regardless of this flag. Kept for a future "persists across runs" effect.
    pub persist: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entity {
    // --- identity ---
    /// Stable spawn-order id. Used as tie-breaker in targeting (GDD §3.2).
    pub id: u32,
    pub team: Team,
    /// `false` after hp reaches 0. Slot is kept in the Vec so that
    /// indices and target_ids referring to this entity stay valid.
    pub alive: bool,

    // --- spatial ---
    pub pos: Cell,
    /// Walk destination when no enemy is being chased. The tick loop
    /// will OVERWRITE this each tick if an enemy is in sight.
    pub target: Cell,
    /// If `Some`, the id of the enemy currently being engaged. Cleared
    /// each tick by the target selector (re-selected fresh — cheap
    /// for small N, and avoids stale-pointer bugs when the previous
    /// target dies).
    pub target_id: Option<u32>,

    // --- movement (accumulator pattern, GDD §4.2.1) ---
    pub speed_milli: u32,
    pub move_acc: u32,

    // --- combat ---
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    /// Monster-only FLAT overrides for Hit / Flee / hard-MDEF (0 = derive from the
    /// Monster-only ATK damage RANGE (RO atk1/atk2). A mob's physical hit rolls its
    /// ATK uniformly in `[mob_atk_min, mob_atk_max]` (one draw) instead of the flat
    /// `atk`. 0/0 (or max ≤ min) = no range → flat `atk`, no draw. Skip-if-zero so
    /// every non-mob entity serializes byte-identically.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub mob_atk_min: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub mob_atk_max: i32,
    /// Monster-only STAGE (0.0.399; was `mob_level`) — soft DEF, the sigil tier and the
    /// grade-odds fallback read it. 0 = unset. Skip-if-zero so every non-mob entity serializes
    /// byte-identically.
    #[serde(default, alias = "mob_level", skip_serializing_if = "is_zero_u16")]
    pub mob_stage: u16,
    pub element: Element,
    /// RO RACE (gear-mod conditional axis: dmg-vs-race / DR-vs-race). Monsters
    /// copy it from their template; every Player is `DemiHuman` (stamped in
    /// `with_team`). Skip-if-default keeps old saves byte-identical.
    #[serde(default, skip_serializing_if = "is_default_race")]
    pub race: crate::grid::Race,
    /// RO SIZE (dmg-vs-size axis). Monsters from template; players `Medium`.
    #[serde(default, skip_serializing_if = "is_default_size")]
    pub size: crate::grid::Size,
    pub atk_speed_milli: u32,
    pub atk_acc: u32,
    /// Attack range in cells (Chebyshev). 1 = melee. Ranged values
    /// per monster come from data in ALPHA-6.
    pub atk_range: i32,
    /// Physical or magic resolution for this entity's attack
    /// (BETA-5 Phase 3). Magic uses MATK/MDEF and ignores DEF/crit/flee.
    pub atk_type: DamageType,
    /// Whether this entity's attack is "ranged damage" — an intrinsic
    /// property, NOT derived from distance: an archer firing at an
    /// adjacent cell still deals ranged damage. Carried onto the
    /// `Damage` packet so future "blocks ranged" defenses can key off
    /// it. Independent of `proj_travel_ticks` (the projectile-flight
    /// mechanic): a hitscan gun would be `is_ranged` with travel 0.
    pub is_ranged: bool,

    // --- ALPHA-4: equipment + drop table ---
    /// Items currently equipped — flat ATK/DEF bonuses sum into the
    /// effective combat stats (see `combat::effective_atk` / `_def`).
    pub equipped: Vec<Item>,
    /// CARD PANEL: cards equipped to THIS CHARACTER (not into a gear item), each a
    /// full Card with its `card_slot` class. Capacity per class = `card_panel_capacity`;
    /// how many are ACTIVE is gated by the matching gear's sockets (see
    /// `combat::card_bonuses`). Persists across gear swaps. Empty for monsters /
    /// card-less characters → serde-skipped so goldens stay byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub card_panel: Vec<Item>,
    /// Equipped Exemplar (skin + stat grant + granted skill) — the skin_id drives the render, the
    /// effect folds into `card_bonuses`. Skip-if-none keeps un-skinned entities byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exemplar: Option<crate::items::Exemplar>,
    /// Equipped Regalia (signature-weapon overlay: stat grant + granted skill + signature attack
    /// anim). Twin of `exemplar` — folds into `card_bonuses` the same way. A member can wear BOTH.
    /// Skip-if-none keeps un-Regalia'd entities byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regalia: Option<crate::items::Regalia>,
    /// Up to 3 consumable slots (RO auto-pot). Each holds a `Consumable` item
    /// with `quantity` charges; the auto-use upkeep fires it when its trigger
    /// is met. Empty Vec for monsters + fresh characters → serde-skipped so old
    /// saves + goldens stay byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub consumables: Vec<crate::items::ConsumableSlot>,
    /// RESERVED provision slots — at most one Meal and one Tonic, held apart from the emergency
    /// bar above so a long buff never costs a healing slot. Fires through the same auto-use
    /// upkeep. Empty for monsters + fresh characters → serde-skipped, so old saves and goldens
    /// stay byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provisions: Vec<crate::items::ConsumableSlot>,
    /// Weighted drop table rolled exactly once when this entity dies.
    /// Empty Vec = no drops (e.g. player characters).
    pub drops: Vec<DropEntry>,
    /// This monster's GEAR GRADE odds, carried from its template (see `MonsterTemplate`).
    /// Empty = the code default.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gear_grade_odds: Vec<u32>,

    // --- ALPHA-8b: zone boss flag ---
    /// True if this entity is the zone's boss. Bosses count toward
    /// `zone_cleared` instead of `kill_counter`, and render at 1.5×
    /// size on the client.
    pub is_boss: bool,

    // (Carried items moved to the party-shared `GameState.inventory` in the
    //  BETA-5 party slice; equip pulls from there to `equipped`.)

    // --- BETA-4: projectile travel ---
    /// Ticks between firing a swing and damage landing on the target.
    /// 0 = instant (melee). > 0 = ranged: an Arrow is queued at fire
    /// time and resolves on arrival (GDD §3.2).
    pub proj_travel_ticks: u32,

    // --- BETA-5 Phase 1: RO-style base stats ---
    /// Six base stats (STR/AGI/VIT/INT/DEX/LUK) + unspent point pool
    /// + level + XP. Monsters carry this field too so the struct stays
    /// symmetric (GDD §4.2.2), but `points_unspent` and `level` stay
    /// at their `new()` values for them — only Player entities ever
    /// allocate or level up.
    pub stats: Stats,

    /// XP awarded to the player who killed this entity. Players keep
    /// this at 0 (they don't drop XP). Bosses pre-bake a higher value
    /// at spawn time. Read in `handle_monster_death`.
    pub xp_reward: u32,
    /// Job XP awarded on death (feeds the killer's Job Level). Same
    /// "monsters carry it, players keep 0" rule as `xp_reward`.
    /// Gil (currency) awarded to the party on death. 0 for players.
    pub gil_reward: u32,

    // --- BETA-5 Phase 3: skills ---
    /// Known skills (active + passive). Active ones auto-cast by priority
    /// (list order) in place of a basic attack; passives apply each tick.
    /// Granted from the class on spawn / class switch; empty for monsters.
    pub skills: Vec<crate::skills::SkillState>,
    /// Skill-point investments in the per-character Potential Board.
    #[serde(default, skip_serializing_if = "crate::potential::PotentialState::is_empty")]
    pub potential: crate::potential::PotentialState,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub potential_atk_from_def: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub potential_party_dr_bp: i32,
    /// ACTIVE-skill loadout (RO hotbar): the ids the player has slotted (cap
    /// `ACTIVE_SLOT_CAP`). ONLY slotted actives are cast — an unslotted skill
    /// never fires (drag it onto the bar to use it; drop a Fire skill in a Water
    /// zone). EMPTY = no actives auto-cast (just basic attacks). Newly-learned +
    /// class-granted actives auto-slot (up to the cap) so a fresh character
    /// isn't inert. Passives always apply regardless. See `Entity::skill_slotted`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_skills: Vec<String>,
    /// Player class id this entity was spawned as (e.g. "archer_t1"), so
    /// the renderer can pick the right sprite per party member and the UI
    /// can label them. Empty for monsters.
    #[serde(alias = "class_id")]
    pub unit_def: String,
    /// Monster template id (e.g. "mob_b7_a", "boss_b9") this entity spawned
    /// from, so the RENDERER can map it to a dedicated sprite sheet. Empty for
    /// players and any monster without a bespoke sheet (those fall back to the
    /// procedural slime). Skip-if-empty keeps saves + goldens byte-identical.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub def_id: String,
    /// Per-class HP scaling (RO Renewal-style job growth). MaxHP =
    /// (max_hp + hp_per_level·BaseLvl) × (1 + VIT/100). Set from the
    /// class on spawn; 0 for monsters (they use their flat `max_hp`).
    #[serde(default)]
    pub hp_per_level: i32,
    /// RO HP curve (rAthena factor mode): when `hp_factor != 0`, MaxHP uses the
    /// RO formula (convex per-level growth) instead of the linear `hp_per_level`.
    /// See `combat::effective_max_hp`. 0 for monsters / legacy.
    #[serde(default)]
    pub hp_factor: i64,
    #[serde(default)]
    pub hp_increase: i64,
    /// Transient party-aura ATK bonus (e.g. an Acolyte's Blessing),
    /// recomputed each tick by `apply_party_auras` and folded into
    /// `effective_atk`. Not class state — a buff slot. Defaults 0.
    #[serde(default)]
    pub aura_atk: i32,
    /// Forced-aggro lock from an enemy's Taunt: while `taunt_ticks > 0`
    /// this entity targets `taunted_by` (the taunter's id) instead of its
    /// nearest enemy. Refreshed each taunt cast; expires on its own.
    #[serde(default)]
    pub taunted_by: Option<u32>,
    #[serde(default)]
    pub taunt_ticks: u32,
    /// ENRAGE (0.0.430): ticks this MONSTER has been alive on the board. After
    /// `ENRAGE_GRACE_TICKS` it gains `ENRAGE_STEP_BP` ATK every `ENRAGE_STEP_TICKS`,
    /// without limit (user: "มอนได้บัฟ DMG+20% ทุก 15 วิ หลัง 30 วิ … stack ขึ้นเรื่อยๆ").
    /// Players and pets never count.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub enrage_ticks: u32,
    /// DISPLAY-ONLY (0.0.438): `(shield source id, HP the shield soaked)` this tick, drained by
    /// the telemetry into `DpsTracker.blocked_by_class`. Never serialized, never hashed — the
    /// same contract as `GameState.recent_hits`.
    #[serde(skip)]
    pub soaked_fx: Vec<(u32, i32)>,
    /// DISPLAY-ONLY (0.0.456): damage this entity's DEF / reductions / aura took OFF incoming
    /// packets this tick (packet − what `apply_damage` let through), drained by the telemetry
    /// into `DpsTracker.mitigated_by_class`. Never serialized, never hashed.
    #[serde(skip)]
    pub mitigated_fx: i64,
    /// DISPLAY-ONLY (0.0.465): `(spark layer's entity id, damage)` a Static Spark paid this
    /// tick, drained by the telemetry into the layer's DPS. Never serialized, never hashed.
    #[serde(skip)]
    pub spark_fx: Vec<(u32, i32)>,
    /// HP this entity's `heal_on_hit_bp` passive owes the party (0.0.468, Unda Vitae): booked in
    /// the damage path, spent by the tick. Never serialized, never hashed.
    #[serde(skip)]
    pub heal_debt: i32,
    /// How many living enemies this entity currently holds under its own Taunt.
    /// Recomputed every tick in `apply_taunt`, so it is a CACHE of state that
    /// already exists rather than a new source of truth — but it is serialized
    /// anyway, because a value read by a derived stat must survive a save
    /// (a transient combat field that skips serialization is how a replay
    /// diverges). Read by Bulwark in `effective_hard_def`.
    ///
    /// Omitted when zero, which is the state of every entity that holds no
    /// taunt — so a fight where nobody taunts serializes exactly as it did
    /// before this field existed, and the goldens that never taunt stay
    /// byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub taunt_hold: u32,

    /// TOUGHNESS — the gauge an attacker breaks (`docs/element-weakness-system.md`
    /// §6.1). `toughness_max` 0 means this entity has no gauge at all, which is
    /// every player and every monster whose data has not declared one yet.
    /// `toughness` is what is LEFT; it refills on break.
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub toughness: u16,
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub toughness_max: u16,
    /// Elements that chip this entity's gauge (`MonsterDef.weaknesses`). EMPTY means
    /// "ask the element matrix instead" — old saves and every player entity, which
    /// has no gauge to chip anyway.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weaknesses: Vec<crate::grid::Element>,
    /// Elements this entity RESISTS — 80% damage. Opt-in per monster (elites and
    /// bosses); empty everywhere else.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resists: Vec<crate::grid::Element>,

    /// BREAK TRIGGER queue: enemy ids waiting to be answered, one per Toughness
    /// break this entity saw. Ids, never indices — an index kept across ticks points
    /// at the wrong body the moment the entity list shrinks. FIFO, capped at
    /// `BREAK_QUEUE_CAP`; a target that dies before its turn is dropped. Empty for
    /// anyone whose kit has no `SkillDef.break_trigger`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub break_queue: Vec<u32>,
    /// Ticks left before the next queued answer may fire. Counts down only while the
    /// queue has something in it.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub break_cd: u8,
    /// Zolfia's S1 (0.0.476): enemies Shattered this RUN — each adds to Coldflame Split's
    /// damage. Cleared when a run starts (room 1 of an instance: band advance, retreat, or a
    /// farm loop), kept across a room hop. Serialized: it has to survive a save mid-run.
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub shatter_stacks: u16,

    /// Ticks left in a BREAK. Set when the gauge empties, counted down by
    /// `tick::refill_toughness`, which is also what refills the gauge across the
    /// window. A separate counter rather than "is the gauge below max": a gauge
    /// that was merely CHIPPED must stay chipped, or a monster that survives a few
    /// hits heals its bar for free.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub break_ticks: u8,

    /// LIMIT BREAK charge. The gauge's size and how it fills are written on the
    /// character's Ultimate (`SkillDef.lb_max` and friends), so this is only the
    /// running total. Emptied when the Ultimate fires.
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub limit_break: u16,

    /// AUTHORED attack growth (thousandths), copied from the class at spawn. See
    /// `UnitDef::atk_base_milli`. Both zero ⇒ this entity still derives its
    /// attack from the six attributes, which is every monster and every save made
    /// before the curve existed.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub atk_base_milli: i32,
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub atk_per_level_milli: i32,

    /// Thief Hiding's per-session charge: set true when Hiding auto-triggers,
    /// cleared at the start of a new instance run (room 1, see `change_zone`), so
    /// it's usable once per session (3-room run). Not serialized — a reload
    /// mid-run simply refreshes the charge (acceptable) and golden hashes stay
    /// byte-identical.
    #[serde(default, skip_serializing_if = "is_false")]
    pub hiding_used: bool,
    /// IDLE WANDER bookkeeping (monsters), and the "has settled" marker for an idle player.
    /// Tick at which a standing monster starts its next stroll; 0 = not currently pausing.
    /// Serde-skipped at 0, so an existing save/golden with no idle entity serializes unchanged.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub wander_until: u64,

    /// Assassin Bloodthirst stacks: +1 per LAST-HIT kill (capped at
    /// `skills::BLOODTHIRST_MAX_STACKS`), each worth +0.5% damage per skill
    /// level. A kill-streak reward — reset at the START of a new instance run
    /// (band advance / retreat / farm-mode loop to room 1) OR on this entity's
    /// death. Not serialized (like `hiding_used`): a reload drops the streak
    /// (acceptable) and golden hashes stay byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub bloodthirst_stacks: u8,
    /// Ticks this party member has been DOWN (incremented each tick while
    /// `!alive`, reset to 0 on revive/while alive). Bishop Resurrection waits a
    /// grace delay (`RESURRECTION_DELAY_TICKS`) before auto-reviving, so a member
    /// isn't instantly popped back up the moment they fall. Not serialized
    /// (transient, like `hiding_used`): a reload restarts the timer (acceptable)
    /// and golden hashes stay byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub down_ticks: u32,
    /// Ranger Knock Back Shot: set true once this entity has been knocked back by
    /// that skill, so each enemy is shoved only ONCE in its lifetime (no
    /// perma-kite). Not serialized (transient; fresh monsters spawn `false`) →
    /// golden hashes stay byte-identical.
    #[serde(default, skip_serializing_if = "is_false")]
    pub kb_shot_hit: bool,
    /// Ranger Snare Strike: set true once this entity has been ROOTed by that
    /// skill, so each enemy is snared only ONCE in its lifetime (no perma-root).
    /// Not serialized (transient; fresh monsters spawn `false`) → goldens identical.
    #[serde(default, skip_serializing_if = "is_false")]
    pub snare_used: bool,
    /// Per-victim hit counter for a guaranteed-Nth-hit field freeze (Glacial Storm
    /// freezes on the 3rd hit IT takes). Incremented per field hit, reset on freeze.
    /// Not serialized (transient) → golden hashes stay byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub field_hit_count: u8,

    /// Active absorb shields (Flame Shield — and future Bishop Praesidium),
    /// OLDEST-FIRST: incoming damage soaks from `shields[0]` first and cascades to
    /// the next when one empties. They stack independently (separate pools), so a
    /// Flame Shield (self) + a Praesidium (from a Bishop) coexist. Not serialized
    /// (transient, like `bloodthirst_stacks`) → a reload drops them; goldens stay
    /// byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shields: Vec<AbsorbShield>,
    /// Shields that emptied THIS tick, awaiting their explosion in
    /// `tick::resolve_absorb_bursts`. Not serialized (transient).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_bursts: Vec<AbsorbShield>,

    /// Motion delay (RO animation/motion lock): the absolute tick until which
    /// this entity is LOCKED after casting a skill — no move / attack / cast — so
    /// the skill's animation plays out before basics resume. Set from the skill's
    /// `motion_delay_ticks` when it resolves; the action loop skips while
    /// `tick < act_locked_until`. Not serialized (transient combat timing, like
    /// `hiding_used`); 0 = not locked.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub act_locked_until: u64,
    /// THIS WIND-UP MAY RESOLVE AS A SKILL (0.0.483). Decided when the wind-up begins
    /// (`tick::open_windup`): true only if a skill was ready then. A skill that comes ready
    /// mid-wind-up waits for the next swing, so the renderer never sees `pending_skill` appear
    /// halfway through a basic and swap the pose (user: the basic attack was cut). Serialized —
    /// a save taken mid-wind-up must replay the same decision.
    #[serde(default, skip_serializing_if = "is_false")]
    pub windup_cast: bool,

    /// Transient buffs / debuffs (skill cast-buffs, party buffs, on-hit
    /// debuffs, DoTs). Aged each tick by `tick::decay_buffs`; folded into the
    /// effective stats via `skills::buffed_stats` + the `combat::derived`
    /// `*Pct`/`*Add` reads. Skip-if-empty keeps every buff-less entity (every
    /// monster + test fixture) byte-identical in the state hash.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buffs: Vec<crate::skills::Buff>,

    /// Elemental ammo pool (archers): loaded quivers, one stack per element.
    /// Physical ranged attacks auto-swap to the loaded element that best beats
    /// the current target, spending a round (see `combat::pick_ammo_idx`).
    /// Empty for everyone who never loads a quiver → serde-skipped, so every
    /// melee/mage/monster entity stays byte-identical in the state hash.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ammo: Vec<crate::items::AmmoStack>,

    /// PERMANENT mods from BOUND cards (the "bind for a small permanent stat instead of
    /// socketing" path) — folded through `stat_mods::for_each_mod` exactly like gear, an
    /// overlay or a socketed card (0.0.245). DERIVED, never authored: rebuilt from
    /// `bound_card_counts` × each def's `bind_mods` by `bind_card` and again on load by
    /// `migrate_save_items`, so re-tuning a bind in the catalog patches every save and an
    /// old save's dropped `bound_card_bonus` blob costs nothing. Empty for anyone who never
    /// bound a card → serde-skipped, goldens byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bound_card_mods: Vec<crate::items::StatMod>,
    /// TRAINING lines (0.0.406) — the party's bought levels as StatMods, rewritten every tick by
    /// `training::apply`. Empty until something is bought → serde-skipped, goldens byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub training_mods: Vec<crate::items::StatMod>,
    /// CREST lines (0.0.416) — the party's crest levels as StatMods, rewritten every tick by
    /// `crest::apply`. Empty until a crest is lifted → serde-skipped.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub crest_mods: Vec<crate::items::StatMod>,
    /// How many times each card def_id has been BOUND (cap 3 per type). The
    /// Nth bind of a type costs N cards (1, then 2, then 3) — escalating cost +
    /// a hard cap so cards can't be ground into unlimited permanent stats.
    /// Skip-if-empty keeps card-less entities byte-identical.
    #[serde(default, alias = "consumed_card_counts", skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub bound_card_counts: std::collections::BTreeMap<String, u8>,

    /// In-progress spell CAST (mage Variable Cast Time). `Some` only while the
    /// caster is channelling a windup; the entity stands still and casting IS
    /// its action until the windup elapses, then the stored spell resolves at
    /// the stored target. DEX+INT shorten the windup (`combat::effective_cast_time`).
    /// `None` for everyone not mid-cast → serde-skipped, so every existing
    /// entity stays byte-identical in the state hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub casting: Option<Casting>,

    /// HELD by the player's hand (`regent_hand`): the tick this hold auto-drops,
    /// 0 = not held. A held character is OFF THE BOARD — it takes no action, is
    /// not targetable, and no AoE reaches it — which is why the hold is capped
    /// (`PICKUP_HOLD_MAX_TICKS`) and costs the character's whole damage uptime.
    /// 0 for everyone not mid-drag → serde-skipped, goldens byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub held_until: u64,

    /// PET flag — true for a Ranger pet (an abstract auto-attacker bound to an
    /// owner). Excluded from party-wipe / XP-share / lead logic via this flag.
    /// Skip-if-false → every non-pet entity (all goldens) stays byte-identical.
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_pet: bool,
    /// A SUMMONED monster (Broomhilda's bats, `spawn::spawn_familiars`). Its death pays nothing —
    /// no soul, kill counts, room progress, bestiary credit or drop rolls (audit 2026-10-02: the
    /// empty drop table alone did not stop the rest, so an unlimited summoner was a farm).
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_summon: bool,
    /// Owner entity id of a pet (the archer it belongs to); 0 = not a pet.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub owner_id: u32,
    /// Can't be targeted by enemies (`find_enemy_idx` skips it). The Ranger
    /// damage-pet is untargetable + immortal; a FUTURE tank pet sets this false
    /// (+ real HP) to soak hits. Skip-if-false → non-pet entities byte-identical.
    #[serde(default, skip_serializing_if = "is_false")]
    pub untargetable: bool,
}

/// Toughness taken by one damage instance: the baseline every hit carries, and
/// the doubled figure an element the target is weak to carries instead.
///
/// Sized against a 60 gauge — three plain hits, or two when the element matches —
/// which measurement picked over the 100 the design first assumed. See
/// `docs/element-weakness-system.md` §6.1.2 for the numbers and why 100 never broke.
/// A break trigger's pacing: one queued answer every `BREAK_TRIGGER_CD_TICKS`, and
/// at most `BREAK_QUEUE_CAP` waiting. The cooldown is what keeps the chain from
/// resolving inside one tick no matter how many gauges break at once; the cap is
/// what keeps the queue out of a save's tail.
/// A BREAK stuns: 2 s for a normal monster, 5 s for a boss. The gauge is empty for
/// exactly that long and refills across it, so it comes back full as the target
/// gets its turn back — the bar on screen IS the timer, and there is no separate
/// number to explain.
pub const BREAK_STUN_TICKS: u32 = 20;
pub const BREAK_STUN_TICKS_BOSS: u32 = 50;
/// A monster's gauge grows by HALF AGAIN every time it is broken (x3/2 per break).
/// Breaking the same body twice is meant to be a thing a party EARNS, not the
/// default state of a fight: with full element coverage a level-70 party broke a
/// monster 2.87 times per kill, which is a monster that never acts. Escalation puts
/// a ceiling on that in the structure instead of hoping the numbers stay low.
///
/// BOSSES ARE EXEMPT (user): a boss fight is one long body, so the whole rhythm of
/// it is repeated breaks — escalating there would delete the mechanic from the only
/// fight built around it. Their 300% bar already prices the repetition in.
pub const BREAK_GAUGE_ESCALATION_NUM: u32 = 3;
pub const BREAK_GAUGE_ESCALATION_DEN: u32 = 2;
/// Extra damage a broken target takes while it is broken. The break's own packet
/// is paid to whoever landed it; THIS is what the rest of the party gets out of
/// somebody else's break, which is the half a idle game can actually build around.
pub const BREAK_VULN_BP: i32 = 2_000;

/// How long a broken gauge takes to come back, and the window in which it CANNOT
/// break again (user 2026-09-09: "add break recovery อาจจะ 5 วิ / แต่ stun 2 วิ
/// ไม่ให้ break ติดกัน").
///
/// Recovery is deliberately LONGER than the stun. The stun is what the target loses;
/// recovery is what the party has to earn back, and separating them is the only thing
/// that stops a break from chaining. Before this the gauge refilled across the stun
/// alone and was chippable from its second tick — at a 40-point chip against a bar
/// refilling 5 points a tick, one hit eight ticks in emptied it again, so a break
/// re-broke while the target was still lying down.
pub const BREAK_RECOVERY_TICKS: u32 = 50;

pub const BREAK_TRIGGER_CD_TICKS: u8 = 2;
pub const BREAK_QUEUE_CAP: usize = 16;

/// What ONE landed hit takes off the gauge. Only a hit of an element the target is
/// weak to takes anything at all (0.0.326), so there is one number here now:
/// `TOUGHNESS_CHIP_ADVANTAGE`. `TOUGHNESS_CHIP_BASE` is what an off-element hit used
/// to take and is kept as the unit the skill multipliers were authored against.
///
/// 20 AGAINST A 100-POINT GAUGE IS FIVE HITS, and the five is the point (user
/// 2026-09-09: "ตีทีเดียว break ไม่ค่อยสมเหตุผล"). At 40 it was three, and three is
/// one multi-hit skill: every instance chips separately, so a single swing emptied
/// the bar and the break stopped being something the party built toward. The gauge
/// itself is not the lever — every monster in the catalog carries 100, so raising
/// band 1 toward "80+" would have made it EASIER, not harder.
///
/// The skill multipliers ride this number and were left alone, which is what keeps
/// a break weapon a break weapon: Rapis at 200% takes 40 a hit, Steel Break at 300%
/// takes 60 — still the fastest way to a break, no longer a one-shot.
pub const TOUGHNESS_CHIP_BASE: u16 = 20;
pub const TOUGHNESS_CHIP_ADVANTAGE: u16 = 20;

fn is_false(b: &bool) -> bool {
    !*b
}
fn is_zero_u32(v: &u32) -> bool {
    *v == 0
}
fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}
fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}

fn is_zero_i32(v: &i32) -> bool {
    *v == 0
}

fn is_zero_u16(v: &u16) -> bool {
    *v == 0
}

/// A spell windup in progress (RO Variable Cast Time). Carries enough to draw
/// the client cast bar (`ticks_left` / `ticks_total`) and to resolve the SAME
/// spell at the SAME target when the windup ends (`slot` + `target_id`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Casting {
    /// Ticks remaining in the windup. Hits 0 → the spell fires.
    pub ticks_left: u32,
    /// Total windup the cast began with (cast-bar denominator). Frozen at start.
    pub ticks_total: u32,
    /// Skill slot being channelled. A `SkillSlot`, not a bare `usize`, so a provision or
    /// consumable slot cannot be assigned here by mistake — they are all `usize` otherwise.
    pub slot: crate::skills::SkillSlot,
    /// Id of the locked target the spell resolves against on fire.
    pub target_id: u32,
    /// GROUND-FIELD casts only: the cell the field was AIMED AT, snapshotted when
    /// the channel began. The field lands HERE, not wherever the target has walked
    /// to by the time the windup ends — otherwise the danger zone tracks its victim
    /// and no telegraph can be dodged (which is the whole contract of a wind-up).
    /// `None` for every non-field cast → serde-skipped, so a mid-cast entity in an
    /// existing save/golden serializes unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground_cell: Option<Cell>,
}

/// PICK-UP (the Regent's hand — drag a party member out of a telegraphed AoE).
/// How far (Chebyshev) a held member may be dropped from where they were picked
/// up: wide enough to clear a 5×5 blast, too short to rewrite the party's
/// melee/ranged spacing for free.
pub const PICKUP_RANGE: i32 = 6;
/// How many lifts the player may bank. A pool rather than a flat cooldown so the hand
/// can answer two threats back to back and then be empty — the burst is the skill.
pub const PICKUP_MAX_CHARGES: u32 = 2;
/// Ticks to recover ONE spent charge (10 s).
pub const PICKUP_COOLDOWN_TICKS: u64 = 100;
/// Landing recovery after a drop (0.5 s) — reuses `act_locked_until`, the same
/// motion lock a resolved skill applies.
pub const PICKUP_LAND_LOCK_TICKS: u64 = 5;
/// A hold lasts at most 3 s, then the character DROPS in place. Caps the window
/// in which a held member is off the board (and dealing no damage), and means a
/// client that dies mid-drag self-heals instead of parking someone forever.
pub const PICKUP_HOLD_MAX_TICKS: u64 = 30;

/// Max ACTIVE skills a character may slot into its loadout.
pub const ACTIVE_SLOT_CAP: usize = 10;


fn is_default_race(r: &crate::grid::Race) -> bool {
    matches!(r, crate::grid::Race::Unknown)
}
fn is_default_size(z: &crate::grid::Size) -> bool {
    matches!(z, crate::grid::Size::Medium)
}

impl Entity {
    /// True if `skill_id` is slotted in the loadout (so it may be cast). An
    /// unslotted active never fires; an empty loadout casts nothing. Passives
    /// don't go through the cast paths, so this only ever gates actives.
    pub fn skill_slotted(&self, skill_id: &str) -> bool {
        self.active_skills.iter().any(|s| s == skill_id)
    }

    /// Auto-slot a freshly-known ACTIVE skill into the loadout if there's room
    /// (cap `ACTIVE_SLOT_CAP`) and it isn't already slotted — so learning or
    /// being granted a skill makes it fire without a manual drag. No-op past the
    /// cap (the player then chooses which to slot).
    pub fn auto_slot_skill(&mut self, skill_id: &str) {
        if self.active_skills.len() < ACTIVE_SLOT_CAP
            && !self.active_skills.iter().any(|s| s == skill_id)
        {
            self.active_skills.push(skill_id.to_string());
        }
    }

    /// Apply `dealt` damage from a DIRECT HIT: subtract HP, shatter Freeze (RO —
    /// any hit breaks the ice), and flag death at 0. THE single place a hostile
    /// hit lands on an entity, so on-hit side-effects (the freeze break) live in
    /// ONE spot instead of being duplicated at every damage site. Callers still
    /// own the death follow-up (kill_monster / drops) since that needs `GameState`.
    /// NOT used by damage-over-time (continuous — doesn't break Freeze) or ground
    /// fields (which skip frozen targets entirely).
    /// Chip the Toughness gauge for one landed damage instance and say whether it
    /// BROKE. `advantage` is the element test — an attack the target is weak to
    /// takes the gauge down twice as fast, which is where the element system gets
    /// its weight without touching the damage multiplier.
    ///
    /// A break refills the gauge, so breaking again means chewing through the whole
    /// bar a second time. That is what paces Disorder without a separate cooldown.
    /// No gauge (`toughness_max == 0`) ⇒ nothing happens and nothing is reported.
    /// `mult_pct` is the skill's own multiplier on that chip — 0 means the
    /// baseline every hit carries, 300 means triple (Steel Break). A skill built
    /// to break gauges says so on its own definition rather than being special-
    /// cased here.
    ///
    /// `buildup_bp` is the ATTACKER's Break Build-up stat (basis points, 0 = none):
    /// the same chip, scaled by what the character brings. The skill says how much
    /// of the gauge this MOVE takes; build-up says how much this CHARACTER takes.
    /// Does an attack of `el` chip this entity's gauge? The authored weakness LIST
    /// when it has one, the element matrix when it does not.
    pub fn is_weak_to(&self, el: crate::grid::Element) -> bool {
        if self.weaknesses.is_empty() {
            return crate::combat::element_mult_bp(el, self.element) > 10_000;
        }
        self.weaknesses.contains(&el)
    }

    pub fn chip_toughness(&mut self, advantage: bool, mult_pct: u32, buildup_bp: i32) -> bool {
        if self.toughness_max == 0 {
            return false;
        }
        // WEAKNESS ONLY (0.0.326, HSR's rule): an attack the target is not weak to
        // lands for damage and leaves the bar alone. This is what makes the element
        // chart decide anything — and in an idle game it is answered by WHERE the
        // player farms and WHO they bring, which are the two decisions they do make.
        if !advantage {
            return false;
        }
        let base = TOUGHNESS_CHIP_ADVANTAGE;
        let chip = if mult_pct == 0 {
            base as u32
        } else {
            base as u32 * mult_pct / 100
        };
        // Build-up scales the chip. A non-zero chip never rounds away to nothing.
        let chip = ((chip as i64) * (10_000 + buildup_bp.max(-10_000) as i64) / 10_000)
            .clamp(1, u16::MAX as i64) as u16;
        // RECOVERING: the gauge refills across `break_ticks` (`tick::refill_toughness`)
        // and nothing chips it until it is back. The guard is the TIMER, not the
        // gauge's value: a `toughness == 0` test only covers the break tick itself,
        // and from the next one the bar is a small non-zero number that a single hit
        // empties again — which is how a break came to chain into the one after it.
        if self.break_ticks > 0 || self.toughness == 0 {
            return false;
        }
        if self.toughness > chip {
            self.toughness -= chip;
            return false;
        }
        self.toughness = 0;
        true
    }

    /// Attack from the AUTHORED curve, or `None` when this entity has none and the
    /// old attribute-derived path still applies.
    pub fn authored_atk(&self) -> Option<i32> {
        if self.atk_base_milli == 0 && self.atk_per_level_milli == 0 {
            return None;
        }
        let lvl = self.stats.level as i64;
        Some(((self.atk_base_milli as i64 + self.atk_per_level_milli as i64 * lvl) / 1000) as i32)
    }

    pub fn take_hit(&mut self, dealt: i32) {
        self.hp = self.hp.saturating_sub(dealt);
        if dealt > 0 {
            self.buffs.retain(|b| b.kind != crate::skills::BuffKind::Freeze);
        }
        if self.hp <= 0 {
            self.alive = false;
        }
    }

    /// Bare entity at a position. All stats start at "harmless idle":
    /// alive, no movement, no attacks, 1 hp, Fire element. Use the
    /// `with_*` builder methods to configure further. This keeps test
    /// fixtures readable.
    pub fn idle(id: u32, pos: Cell) -> Self {
        Self {
            id,
            team: Team::Player,
            alive: true,
            gear_grade_odds: Vec::new(),
            pos,
            target: pos,
            target_id: None,
            speed_milli: 0,
            move_acc: 0,
            hp: 1,
            max_hp: 1,
            atk: 0,
            def: 0,
            element: Element::Fire,
            race: crate::grid::Race::Unknown,
            size: crate::grid::Size::Medium,
            atk_speed_milli: 0,
            atk_acc: 0,
            mob_atk_min: 0,
            mob_atk_max: 0,
            mob_stage: 0,
            atk_range: 1,
            atk_type: DamageType::Physical,
            is_ranged: false,
            equipped: Vec::new(),
            card_panel: Vec::new(),
            exemplar: None,
            regalia: None,
            consumables: Vec::new(),
            provisions: Vec::new(),
            active_skills: Vec::new(),
            drops: Vec::new(),
            is_boss: false,
            proj_travel_ticks: 0,
            stats: Stats::new(),
            xp_reward: 0,
            gil_reward: 0,
            skills: Vec::new(),
            potential: crate::potential::PotentialState::default(),
            potential_atk_from_def: 0,
            potential_party_dr_bp: 0,
            unit_def: String::new(),
            def_id: String::new(),
            hp_per_level: 0,
            hp_factor: 0,
            hp_increase: 0,
            aura_atk: 0,
            taunted_by: None,
            taunt_ticks: 0,
            enrage_ticks: 0,
            soaked_fx: Vec::new(),
            mitigated_fx: 0,
            spark_fx: Vec::new(),
            heal_debt: 0,
            taunt_hold: 0,
            toughness: 0,
            toughness_max: 0,
            weaknesses: Vec::new(),
            resists: Vec::new(),
            break_queue: Vec::new(),
            break_cd: 0,
            shatter_stacks: 0,
            break_ticks: 0,
            limit_break: 0,
            atk_base_milli: 0,
            atk_per_level_milli: 0,
            hiding_used: false,
            wander_until: 0,
            bloodthirst_stacks: 0,
            down_ticks: 0,
            kb_shot_hit: false,
            snare_used: false,
            is_pet: false,
            is_summon: false,
            owner_id: 0,
            untargetable: false,
            field_hit_count: 0,
            shields: Vec::new(),
            pending_bursts: Vec::new(),
            act_locked_until: 0,
            windup_cast: false,
            buffs: Vec::new(),
            ammo: Vec::new(),
            bound_card_mods: Vec::new(),
            training_mods: Vec::new(),
            crest_mods: Vec::new(),
            bound_card_counts: std::collections::BTreeMap::new(),
            casting: None,
            held_until: 0,
        }
    }

    /// True while the player's hand holds this character (off the board).
    pub fn is_held(&self) -> bool {
        self.held_until != 0
    }

    /// ON THE BOARD — the predicate every VICTIM / TARGET scan should use instead of
    /// a bare `alive`: a held character (`regent_hand`) is alive but lifted out of
    /// play, so nothing may hit, splash, heal or buff them. Kept as one function
    /// because the alternative — remembering `&& !e.is_held()` at each of a dozen
    /// scans — is exactly how the bolt-hits-a-held-member hole appeared. Liveness
    /// bookkeeping (party wipe, XP share, drops) still reads `alive` directly: being
    /// in the player's hand is not being dead.
    pub fn on_board(&self) -> bool {
        self.alive && !self.is_held()
    }

    /// Load `shots` rounds of `element` into the ammo pool, stacking onto the
    /// existing stack for that element (saturating) or adding a new one.
    pub fn load_ammo(&mut self, element: Element, shots: u32) {
        if let Some(st) = self.ammo.iter_mut().find(|s| s.element == element) {
            st.shots = st.shots.saturating_add(shots);
        } else {
            self.ammo.push(crate::items::AmmoStack { element, shots });
        }
    }

    pub fn with_team(mut self, team: Team) -> Self {
        self.team = team;
        self
    }

    /// Level of a learned skill by id, or 0 if not learned. Folds the very common
    /// `skills.iter().find(|s| s.id == id).map(|s| s.level).unwrap_or(0)` lookup.
    pub fn skill_level(&self, id: &str) -> u8 {
        self.skills.iter().find(|s| s.id == id).map(|s| s.level).unwrap_or(0)
    }

    /// Materialize the equipped Exemplar's granted skills (signature + rank-unlocked passives) into
    /// the KNOWN skill list, each flagged `from_exemplar` so `clear_exemplar_skill` can strip them
    /// at bench (they must never persist into learned skills). An active grant is NOT auto-slotted
    /// — like any learned active, the player enables it in the loadout (`ToggleActiveSkill`) before
    /// it fires; passives apply by being known. Idempotent — call on deploy AND after any star-rank
    /// change (a re-level updates the existing entry in place, preserving its loadout slotting).
    /// A grant whose id is already LEARNED from the class tree is left untouched (the character's
    /// own skill wins; we don't shadow or later strip it).
    pub fn apply_exemplar_skill(&mut self) {
        let grants = match &self.exemplar {
            Some(ex) => ex.granted_skills(),
            None => return,
        };
        for (id, level) in grants {
            let level = level.max(1); // unset level ⇒ Lv1, never inert
            match self.skills.iter_mut().find(|s| s.id == id) {
                Some(s) if s.from_exemplar => {
                    s.level = level; // refresh our own injected level (star-rank change)
                    continue;
                }
                // Already learned: a KIT skill the overlay also names (Lucielle since 0.0.421 — her
                // line IS her kit) is LIFTED to the star rank's level, never lowered and never
                // re-flagged, so the board's own ranks and the kit's free rank one survive.
                Some(s) => {
                    s.level = s.level.max(level);
                    continue;
                }
                None => {}
            }
            let mut st = crate::skills::SkillState::new(&id, level);
            st.from_exemplar = true;
            self.skills.push(st);
        }
    }

    /// Remove every Exemplar-granted skill from the live loadout (used at bench, before the
    /// character's learned skills are captured back into the roster).
    pub fn clear_exemplar_skill(&mut self) {
        let ids: Vec<String> =
            self.skills.iter().filter(|s| s.from_exemplar).map(|s| s.id.clone()).collect();
        if ids.is_empty() {
            return;
        }
        self.skills.retain(|s| !s.from_exemplar);
        self.active_skills.retain(|a| !ids.contains(a));
        // A windup holds a SLOT INDEX into `skills`, which just shifted under it: the stale index
        // either points past the end (a panic — and under `panic = "abort"` in verify-server, the
        // whole process) or, worse, at a DIFFERENT skill that then fires silently. Every other
        // path that invalidates a cast drops it too — death (`tick/mod.rs`), warp/revive
        // (`build/zone.rs`), being lifted (`handlers.rs`). RO does the same: any interrupt
        // breaks the channel.
        self.casting = None;
    }

    /// Materialize the equipped Regalia's granted skills — the twin of `apply_exemplar_skill`,
    /// flagged `from_regalia`. (P0 Regalia grant no skills; this keeps the lifecycle correct for
    /// when the layered passives land.)
    pub fn apply_regalia_skill(&mut self) {
        let grants = match &self.regalia {
            Some(r) => r.granted_skills(),
            None => return,
        };
        for (id, level) in grants {
            let level = level.max(1);
            match self.skills.iter_mut().find(|s| s.id == id) {
                Some(s) if s.from_regalia => {
                    s.level = level;
                    continue;
                }
                Some(_) => continue, // learned or Exemplar-granted — don't shadow/strip
                None => {}
            }
            let mut st = crate::skills::SkillState::new(&id, level);
            st.from_regalia = true;
            self.skills.push(st);
        }
    }

    /// Remove every Regalia-granted skill from the live loadout (twin of `clear_exemplar_skill`).
    pub fn clear_regalia_skill(&mut self) {
        let ids: Vec<String> =
            self.skills.iter().filter(|s| s.from_regalia).map(|s| s.id.clone()).collect();
        if ids.is_empty() {
            return;
        }
        self.skills.retain(|s| !s.from_regalia);
        self.active_skills.retain(|a| !ids.contains(a));
    }

    /// Level of a skill that is learned (level > 0) AND off cooldown — `None` otherwise.
    /// The common "ready to fire this passive/proc" check.
    pub fn ready_skill(&self, id: &str) -> Option<u8> {
        self.skills
            .iter()
            .find(|s| s.id == id && s.level > 0 && s.cooldown == 0)
            .map(|s| s.level)
    }

    pub fn with_walk(mut self, target: Cell, speed_milli: u32) -> Self {
        self.target = target;
        self.speed_milli = speed_milli;
        self
    }

    pub fn with_combat(
        mut self,
        hp: i32,
        atk: i32,
        def: i32,
        atk_speed_milli: u32,
        element: Element,
    ) -> Self {
        self.hp = hp;
        self.max_hp = hp;
        self.atk = atk;
        self.def = def;
        self.atk_speed_milli = atk_speed_milli;
        self.element = element;
        self
    }

    pub fn with_drops(mut self, drops: Vec<DropEntry>) -> Self {
        self.drops = drops;
        self
    }

    /// Configure the attack's damage type + ranged flag in one call.
    /// Defaults (Physical, melee) are what `idle()` already sets, so
    /// test fixtures only call this for magic / ranged variants.
    pub fn with_attack(mut self, atk_type: DamageType, is_ranged: bool) -> Self {
        self.atk_type = atk_type;
        self.is_ranged = is_ranged;
        self
    }

    pub fn with_equipped(mut self, items: Vec<Item>) -> Self {
        self.equipped = items;
        self
    }
}
