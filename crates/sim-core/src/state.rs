//! `GameState` — the single root of all sim state.
//!
//! `tick()` and `apply_input()` are the only functions that may mutate
//! it. Everything inside is `serde`-serializable so we can both hash
//! and snapshot the whole tree.

// DEBT (1 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::grid::{Cell, Element, Grid, Team};
use crate::items::{DropEntry, Item};
use crate::rng::Pcg32;
use crate::stats::Stats;

/// One APPLIED L1-market operation, recorded in `GameState.market_ops` so the verify-server can
/// read the trusted "what really happened" from the verified state (see the field's doc). `id` is
/// the per-op dedup key (a client-issued nonce). A no-op market input records nothing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MarketOp {
    /// RETIRED (0.0.490, Gil market removed): `Deposit`, `Buy`, `DepositItem` and `BuyItem` can no
    /// longer be created (their InputEvents are tombstones). They stay so old saves deserialize;
    /// the server marks any it still sees as consumed (so the client prunes it) and settles none.
    ///
    /// Was: listed `qty` of `def_id` at `unit_price` each.
    Deposit { id: u64, def_id: String, qty: u32, unit_price: u64 },
    /// Committed a market-buy: `qty * max_price` Gil left GameState into market hold.
    Buy { id: u64, def_id: String, qty: u32, max_price: u64 },
    // (id accessor below in the impl.)
    /// Claimed mail `mail_id` — the APPLIED payload (`gil`/`def_id`/`qty`) is recorded so the
    /// server can verify it against the real mail record. A forged ClaimMail (real mail id, lied
    /// payload) PASSES save-verify (the server replays the forged event faithfully → hash matches),
    /// so the reconcile MUST compare this recorded payload to the server's mail → mismatch = forgery.
    Claim {
        id: u64,
        mail_id: u64,
        #[serde(default, skip_serializing_if = "Gil::is_zero")]
        gil: Gil,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        def_id: String,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        qty: u32,
        /// A full ITEM instance claimed (item-market purchase/return) — recorded so the server can
        /// verify the applied payload against the real mail. Skip-if-none keeps commodity claims
        /// byte-identical.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item: Option<Item>,
    },
    /// ITEM MARKET (per-instance) — listed a specific EQUIPMENT instance at `price` (the item left
    /// the inventory; the server creates the per-item listing on the verified save). Carries the
    /// FULL item — replay proved it authentic (it was in the seller's verified inventory) — so the
    /// server escrows + later delivers the exact instance.
    DepositItem { id: u64, item: Item, price: u64 },
    /// ITEM MARKET — committed a per-item buy: `max_price` Gil left GameState as a hold on
    /// `listing_id`. The server transfers the item + refunds the unspent hold (max_price − actual),
    /// or FULL-refunds if the listing was already taken (race). Symmetric to `Buy`.
    BuyItem { id: u64, listing_id: u64, max_price: Gil },
}

impl MarketOp {
    /// The op's dedup id (the server's consumed-op key, a client-issued nonce).
    pub fn id(&self) -> u64 {
        match self {
            MarketOp::Deposit { id, .. }
            | MarketOp::Buy { id, .. }
            | MarketOp::Claim { id, .. }
            | MarketOp::DepositItem { id, .. }
            | MarketOp::BuyItem { id, .. } => *id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameState {
    /// Game-time tick counter (1 tick = 100 ms, GDD §4.2 rule 5).
    pub tick: u64,
    /// The single PRNG (GDD §4.2 rule 3).
    pub rng: Pcg32,
    /// World grid + entities.
    pub grid: Grid,
    /// Monotonic counter for new monster spawns. Player ids = 1..1000,
    /// monsters start at 1000 and grow. Survives zone changes so an old
    /// monster id never collides with a new spawn.
    pub next_monster_id: u32,
    /// `None` until a zone is loaded. Holds the pre-resolved spawn pool
    /// + population target so continuous spawn in `tick.rs` runs without
    /// re-reading the GameData catalog every tick.
    pub zone_spawn: Option<ZoneSpawnState>,
    /// When `Some`, the sim is asking external orchestration (gdext
    /// bridge / sim-cli) to load the named zone next. Set by the tick
    /// loop after a zone's boss dies. The reader is responsible for
    /// calling `data::change_zone` and clearing this back to `None`.
    /// Keeping this Out-of-band lets sim-core stay GameData-free.
    pub pending_zone_change: Option<String>,
    /// LANE MODE (`lane.rs`): `None` everywhere but a headless runner that opts in. Skipped when
    /// `None`, so every save and golden is byte-identical with or without this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<crate::lane::LaneState>,
    /// Where `LeaveInstance` returns the party: the zone they stood in when they
    /// entered the dungeon. Owned by the sim so the client asks to leave rather than
    /// naming a destination — a forged input can only send you where you came from.
    /// Empty = nothing to return to. Skip-if-empty keeps existing saves byte-identical.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub return_zone_id: String,
    /// Live totals for the run in progress (see `RunTally`).
    #[serde(default, skip_serializing_if = "is_default_run_tally")]
    pub run_tally: RunTally,
    /// The finished run's summary; `Some` exactly while a clear screen is owed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_clear: Option<ClearSummary>,
    /// `regent_hand` CHARGES, held by the PLAYER rather than by each character (user
    /// 2026-07-30) — so the hand can be used in a burst when it matters and then has
    /// to recover, and so the UI has ONE readable number instead of five.
    ///
    /// Stored as charges SPENT (0 = full, `PICKUP_MAX_CHARGES` = empty) precisely so
    /// every default is zero: an old save and every golden serialize unchanged and
    /// load as a full pool.
    #[serde(default, skip_serializing_if = "is_zero_u32_state")]
    pub pickup_spent: u32,
    /// Tick the next spent charge returns; 0 = nothing recharging (pool full).
    #[serde(default, skip_serializing_if = "is_zero_u64_state")]
    pub pickup_recharge_at: u64,
    /// Projectiles in flight (ranged attacks). Damage applies at
    /// `arrive_at_tick`; before that the projectile is presentation-only
    /// (the renderer interpolates from spawn to target).
    /// `alias = "arrows"` so saves written before the 0.0.20 rename still
    /// load (the whole save would otherwise fail and reset the party).
    #[serde(default, alias = "arrows")]
    pub projectiles: Vec<Projectile>,

    /// Active persistent ground fields (Poison Cloud, …). Skip-if-empty so a
    /// field-less state (every golden / non-Assassin game) is byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ground_fields: Vec<GroundField>,

    /// PENDING server-roll rare candidates (server-roll-drops-design.md): kills of
    /// monsters whose drop table carries `server_bp` entries accrue these; the server
    /// resolves each with a keyed nonce (~0.1%) at the next save-verify and grants a
    /// HIT via `GrantDrop` (duds are never sent — they expire via TTL). Skip-if-empty
    /// keeps candidate-less states byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rare_candidates: Vec<RareCandidate>,
    /// Monotonic candidate counter — the server nonce's binding key (never reused,
    /// so a replayed/duplicated GrantDrop can't double-grant).
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub rare_candidate_seq: u64,

    /// Armed Ranger TRAPS (Land Mine / Claymore / Freezing). Dormant ground
    /// objects (NOT entities) that fire ONCE when an enemy enters their 3×3, then
    /// are consumed. Persist until a zone change clears them. Skip-if-empty so a
    /// trap-less state (every golden / non-Ranger game) is byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub traps: Vec<Trap>,

    /// SCHEDULED bomb-monster explosions: a dying ChomBomb queues its blast here to fire
    /// `fire_tick` ticks later, so the sim damage + knockback land IN SYNC with the
    /// explosion animation (not the instant of death). Serialized (survives save/replay);
    /// skip-if-empty so states with no pending blast stay byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_explosions: Vec<PendingExplosion>,

    /// Damage applications THIS tick, for the renderer's multi-hit popups.
    /// Display-only + cleared at the start of every tick → never serialized,
    /// never affects the state hash.
    #[serde(skip)]
    pub recent_hits: Vec<HitEvent>,

    /// Entity ids KNOCKED BACK this tick, so the renderer plays a bounce arc instead of
    /// a linear glide. Display-only + cleared each tick → never serialized.
    #[serde(skip)]
    pub knockbacks: Vec<u32>,

    /// Entity ids PULLED this tick. A pull and a shove use the same walk but are
    /// not the same event: the renderer draws them differently, and anything that
    /// ever triggers "when knocked back" must not fire when someone is hauled IN.
    /// Display-only + cleared each tick → never serialized.
    #[serde(skip)]
    pub pulls: Vec<u32>,

    /// Poison Explosions that detonated THIS tick, for the renderer's burst FX.
    /// Display-only + cleared at the start of every tick → never serialized.
    #[serde(skip)]
    pub poison_bursts: Vec<BurstFx>,

    /// Per-tick DAMAGE-field hits (Vermilion / Glacial), for the renderer's running
    /// YELLOW total per victim (a REAL multi-hit over time, vs the one-packet
    /// MultiHitPopup). Display-only + cleared each tick → never serialized.
    #[serde(skip)]
    pub field_hits: Vec<HitEvent>,

    /// Meteor strikes THIS tick (Meteor Rain), for the renderer to flash each
    /// 5×5 impact where a meteor actually landed (including misses). Display-only
    /// + cleared each tick → never serialized.
    #[serde(skip)]
    pub meteor_impacts: Vec<BurstFx>,

    /// Traps that FIRED THIS tick (Land Mine / Claymore / Freezing), for the
    /// renderer to play a snap/blast anim at the cell — even a trap consumed the
    /// same tick. Display-only + cleared each tick → never serialized.
    #[serde(skip)]
    pub trap_fires: Vec<TrapFire>,

    /// Skill activations THIS tick (targeted melee/ranged casts), for the
    /// renderer's per-skill VFX (e.g. Bash's impact burst vs a basic slash).
    /// Display-only + cleared each tick → never serialized.
    #[serde(skip)]
    pub skill_fx: Vec<SkillFx>,

    /// Ultimates that resolved THIS tick (`caster_id` + the ult's id; `target_id` = caster),
    /// pushed from `unit_rank::after_ult`, the one funnel every ult cast path passes through.
    /// The renderer's cut-in trigger and sim-cli's ult count read this instead of guessing
    /// from the gauge. Display-only + cleared each tick → never serialized.
    #[serde(skip)]
    pub ult_fx: Vec<SkillFx>,

    /// Heals that LANDED this tick, for the renderer's green number and sparkle.
    /// Display-only + cleared each tick → never serialized.
    ///
    /// The client used to INFER heals from `hp_now - hp_was` (`arena.gd`), which cannot tell a
    /// heal from regen, cannot name the healer, merges two heals in one tick into one number, and
    /// — the real defect — shows NOTHING when a heal and a hit of similar size land on the same
    /// tick, because the net HP never moves. An explicit event is the only fix for that last one.
    #[serde(skip)]
    pub heal_fx: Vec<HealEvent>,

    /// Items BANKED this tick, for the renderer's loot banner. Display-only + cleared at
    /// the start of every tick → never serialized, never affects the state hash, and NO
    /// logic may read it: a `serde(skip)` field that something depends on is how replay
    /// diverged once already (0.0.196).
    ///
    /// The whole `Item` rather than a name: item names are DERIVED on the client from the
    /// def plus the rolled stats, so a name minted here would be a second naming path that
    /// could disagree with every other screen.
    #[serde(skip)]
    pub loot_fx: Vec<Item>,

    // --- BETA-5 Phase 1.5: per-class roster ---
    /// Active class id. Was bridge-only state in 0.0.11; moving it
    /// here means resume-from-save shows the correct sprite without
    /// extra recovery heuristics. `None` between `GameState::new`
    /// and the first `new_game_in_zone` (zone load sets it from
    /// `ZoneDef.unit`).
    #[serde(alias = "current_class_id")]
    pub current_unit_def: Option<String>,
    /// Zone the party is currently in (e.g. "zone_b3_1"). Set by `populate_zone`
    /// — EVERY zone load funnels through it (new-game + `change_zone`), so this is
    /// always authoritative. Replaces the bridge's grid-dimension recovery
    /// heuristic on load (two zones can share a width×height → wrong guess). `None`
    /// between `GameState::new` and the first zone load. The UI derives the current
    /// BAND from this. `#[serde(default)]` keeps pre-0.0.162 saves loadable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_zone_id: Option<String>,

    /// The highest main-chain band the party has actually STOOD IN — the ceiling `SelectBand`
    /// may not jump past.
    ///
    /// `SelectBand` is `InputAuthority::Client` and its only check was `band >= 1`, so a level-1
    /// party could type 20 and walk into the endgame. Replay-verify could not see the problem:
    /// the sim accepted the input, so the server replayed it and agreed. This is the same shape
    /// as every other input-authority hole — the question is never "did the client lie about the
    /// result" but "was the client allowed to author this at all".
    ///
    /// RULE (user, 2026-08-18): you may select any band up to REACHED + 1. Going back is always
    /// allowed; skipping ahead is not. The +1 is what lets the natural `next_zone_id` advance and
    /// a manual re-entry behave the same, so the gate never blocks the ordinary path forward.
    ///
    /// The ceiling is a floor made of two sources, and the CURRENT ZONE is the authoritative one:
    /// a party standing in band 40 has self-evidently reached 40, whatever this field says. That
    /// is what makes the gate safe on saves written before it existed — they carry no value here,
    /// default to 0, and are still not locked out of the band they are standing in. No migration
    /// pass, no load hook.
    #[serde(default, skip_serializing_if = "is_zero_u32_state")]
    pub band_reached: u32,
    /// Highest band whose instance-end boss was killed (`record_band_clear`). The travel gate's
    /// frontier is `band_cleared + 1` (audit 2026-10-02): `reached + 1` let a log of SelectBand
    /// 2, 3, … 240 walk to any band in one tick, and `band_reached` now pays party slots,
    /// Commander CXP and offline EXP.
    #[serde(default, skip_serializing_if = "is_zero_u32_state")]
    pub band_cleared: u32,

    // --- XP REFUND (0.0.349, docs/exemplar-unit-and-xp-refund-design.md §5.1, §6) ---
    /// Lifetime high-water marks of the highest BASE level / Apex level any unit has reached
    /// through COMBAT. Raised only in `try_level_up`, NEVER by a refund or an apply: the apply
    /// path caps a unit's poured progression at these, so pooling every unit's XP into one can
    /// never push it past what the account has actually proven. Account-wide. Old saves default
    /// to 0, so readers take `max(mark, current max across units)` rather than the raw field.
    #[serde(default, skip_serializing_if = "is_zero_u32_state")]
    pub level_reached: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32_state")]
    pub apex_reached: u32,
    /// The Engram reserve — refunded XP as bound, denominated items: per-tier COUNTS indexed by
    /// `ENGRAM_TIERS` (1k / 10k / 100k / 1M XP). OFF-BAG by construction: not in `stash` or
    /// `inventory`, so no sell / list / vendor path can address it and no bag cap applies. Only
    /// `RefundUnitXp` credits it (no minting), only the apply events debit it.
    #[serde(default, skip_serializing_if = "engrams_empty")]
    pub owned_engrams: [u32; ENGRAM_TIERS.len()],
    /// RESONANCE CREDITS per Exemplar bare-id (0.0.350, docs §3). A duplicate grant of an Exemplar
    /// the account already owns as a UNIT does not make a second unit — it banks +1 here, and the
    /// player spends one with `FeedExemplarUnit` to raise that unit's stars. A counter, not an
    /// item: no bag cap, no sell / list path can see it — bound by construction.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pending_resonance: BTreeMap<String, u32>,

    // --- S1: input-log recording (event-sourced save / replay-verify, GDD §4.4) ---
    /// When true, `apply_input_with` appends every applied event to `input_log`
    /// (with the tick it fired at). Off by default so headless tools (sim-cli) and
    /// the golden fixtures don't accumulate a log they never use; the live bridge
    /// turns it on at new-game/load. `#[serde(skip)]` — the log is a SEPARATE save
    /// artifact (export_log), never part of the state snapshot or its hash.
    #[serde(skip)]
    pub record_inputs: bool,
    /// Recorded (tick, event) inputs since the session's start snapshot — the
    /// replayable half of the save = (seed + initial state + Vec<InputEvent>).
    /// `#[serde(skip)]`: excluded from the state snapshot + `state_hash` (so
    /// recording never shifts a golden), exported on its own via the bridge.
    #[serde(skip)]
    pub input_log: Vec<crate::events::RecordedInput>,
    /// S1c: interval (in ticks) between hash-chain checkpoints. 0 = off (default;
    /// headless tools + goldens take none). `replay::advance` appends a `Snapshot`
    /// every `snapshot_interval` ticks. `#[serde(skip)]`.
    #[serde(skip)]
    pub snapshot_interval: u64,
    /// S1c: the tamper-evident checkpoint chain (GDD §4.4) accumulated this session.
    /// Each links its `state_hash` to the previous `chain_hash`; a verifier
    /// regenerates the chain by replaying and compares. `#[serde(skip)]` — a
    /// separate save artifact, never in the state snapshot or its hash.
    #[serde(skip)]
    pub snapshots: Vec<crate::replay::Snapshot>,

    /// Owned-but-BENCHED characters, keyed by UNIQUE char id (so duplicate
    /// classes are allowed — own three swordsmen). Deployed party members live
    /// as grid entities (`Team::Player`); everyone else parks here as a full
    /// frozen loadout (no tick, no XP) until `deploy`ed. `buy_character` adds an
    /// entry; `deploy` moves one into the grid; `bench` moves a grid member back.
    /// Empty by default → skip-if-empty keeps old saves + goldens byte-identical. BTreeMap for deterministic JSON order (GDD §4.2 rule 4).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub roster: BTreeMap<u32, RosterChar>,

    // --- BETA-5 Phase 3 (party): shared resources ---
    /// Party-shared main bag ("Inventory"). Drops land here; equip pulls from
    /// here to any member's `equipped`. Renamed from `stash` (0.0.168) to free
    /// that name for the deposit warehouse (the real `stash` field below).
    /// NO `alias = "stash"`: it COLLIDED with the `stash` warehouse field —
    /// serde saw both keys and errored `duplicate field inventory`, so any save
    /// with a non-empty warehouse became UNLOADABLE (0.0.169–0.0.193). The alias
    /// only ever served pre-0.0.168 saves (bag stored under "stash"); those are
    /// long gone (every save since writes "inventory"), so dropping it is safe.
    /// Cap = `INVENTORY_CAPACITY` (via `inventory_cap()`).
    pub inventory: Vec<Item>,
    /// Party-shared currency (Gil). Earned from kills (monster
    /// `gil_reward`); spent on refine / future shop + skill attributes.
    ///
    /// PRIVATE ON PURPOSE — read with [`GameState::gil`], move with `earn/spend/receive/send/
    /// hold/release_gil`. Every movement has to name its counterparty so the economy can be
    /// counted; a `pub` field made that a convention, and a convention is not a gate (the same
    /// lesson `InputAuthority::Server` taught, where a label everyone respected turned out to
    /// enforce nothing). Here the compiler enforces it.
    gil: u64,
    /// Sub-Gil remainder (thousandths) carried by `earn_gil` so the GilPct
    /// path bonus on tiny sales accrues over time instead of rounding to 0 each
    /// time — deterministic, no per-item over-reward. Skip-if-0 keeps saves +
    /// goldens byte-identical (only nonzero once a GilPct node earns a fraction).
    ///
    /// Private for the same reason as `gil`: it is half of the balance.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    gil_millis: u64,
    /// Tick of the last player CLICK-ATTACK. Rate-limits clicks to ONE per tick
    /// (100ms) — a deterministic, replay-safe cap so an auto-clicker can't spam
    /// faster than the sim ticks. Defaults 0 (skipped when 0 → goldens unchanged).
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub last_click_tick: u64,
    /// COMMANDER CHARGES — the shared pool every Commander skill spends from, on top of its own
    /// cooldown (user, 2026-08-21). Cooldowns alone leave four independent timers, where the
    /// correct play is always "press whatever is ready"; one pool makes the skills compete, and
    /// gives the whole family a single balance dial instead of four.
    ///
    /// Regenerates deterministically from the tick counter — see `COMMANDER_CHARGE_TICKS`. No
    /// wall clock, no float, and it is replayed state the verifier recomputes, so a client can no
    /// more forge a charge than a level.
    ///
    /// Skip-if-default keeps every existing save and golden byte-identical: a party that never
    /// casts one never writes the field. `commander_charges` is `MAX` at a fresh start, which is
    /// why its skip test is against the FULL value rather than zero.
    #[serde(default = "commander_charges_full", skip_serializing_if = "is_full_charges")]
    pub commander_charges: u32,
    /// Tick the pool last granted a charge on. Paired with the above; 0 on a fresh state.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub commander_charge_tick: u64,
    /// Ticks remaining on each Commander skill, keyed by skill id. `BTreeMap` because iteration
    /// order is part of the hash. An id with no entry is ready.
    ///
    /// A cooldown, never an index — `no_index_into_a_container_is_stored_in_serialized_state`
    /// covers why that distinction matters here.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub commander_cd: BTreeMap<String, u32>,
    /// Commander skills the PLAYER OWNS — the unlock set. Empty at a fresh start: smite is the
    /// FIRST UNLOCK, not a starter (`docs/commander-skills-design.md`). A Commander skill is
    /// castable and shown in the bar only when its id is here. Populated by a GRANT — today a
    /// DEBUG toggle (`SetCommanderSkillOwned`) for testing the gate; the real trigger (Commander
    /// level, server-side, → a server-authored grant) lands with the backend and only swaps HOW
    /// this set fills, not the read sites. `BTreeSet` because iteration order is part of the hash;
    /// skip-if-empty keeps every existing save and golden byte-identical (a party that never
    /// unlocks one never writes the field), exactly like `commander_cd` above.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub commander_skills_owned: BTreeSet<String>,
    /// L1 MARKET bridge (docs/l1-commodity-market-scope.md Slice 2b): an append-only log of the
    /// market operations that ACTUALLY APPLIED in the sim (a deposit/buy/claim is only recorded
    /// here when its handler mutated state — a no-op records nothing). The verify-server reads this
    /// from the verified state as the trusted "what really happened" (closing the apply-detection
    /// hole: the server can't tell an applied event from a silently-no-op'd one by the final hash
    /// alone). Each op carries a monotonic `id` (from `next_market_op_id`) → the server dedups by
    /// id like a refine nonce (idempotent across a retried/growing-log save). Skip-if-empty keeps
    /// pre-Slice-2b saves + goldens byte-identical. Append-only for now; prune-after-confirm is a
    /// later slice (mirrors the refine consumed-prune TODO).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub market_ops: Vec<MarketOp>,
    /// Monotonic id source for `market_ops` (never reset, so ids stay unique across the save's
    /// life → the server's consumed-id set can't collide). Skip-if-0 keeps old saves identical.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub next_market_op_id: u64,
    /// Live DPS-meter accumulator (player damage by class + by skill). DISPLAY
    /// ONLY — `#[serde(skip)]` so it never enters saves or golden hashes; it's
    /// derived from the same deterministic damage events and reset on demand.
    #[serde(skip)]
    pub dps: DpsTracker,
    /// LOOT METER (0.0.455, diagnostic): gear pieces banked per tier badge, index = tier. Same
    /// contract as `dps` — `#[serde(skip)]`, never in a save or a golden, read by sim-cli to
    /// check the drop curve against what a party actually receives (including a party that
    /// farms under-level on purpose).
    #[serde(skip)]
    pub gear_dropped_by_tier: [u64; 8],
    /// Instance loot ESCROW — drops during a run land here, NOT in `stash`.
    /// Banked to the stash only when the instance is cleared (its final room's
    /// boss dies); cleared on a party wipe. This is the "held loot" the player
    /// risks losing — shown separately from the stash in the UI.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub escrow: Vec<Item>,
    /// When true, clearing the instance's final room loops back to its room 1
    /// (farm the same content) instead of advancing to the next instance.
    /// Player-toggled via `InputEvent::SetFarmMode`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub farm_current_instance: bool,

    /// DEBUG god mode: when true, `tick::apply_debug_god` keeps every player
    /// topped up (HP/SP full, revived, cooldowns 0) and stamps a huge ATK/MATK
    /// buff so the party is invincible and one-shots everything. Skip-if-false →
    /// normal saves + goldens are byte-identical; a debug save with it ON
    /// persists the flag.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub debug_god: bool,
    /// Party-shared SOUL currency. Earned per kill (boss bonus); unlike loot
    /// it's KEPT through a party wipe. Uses TBD (prestige / upgrades). Skip-if-0
    /// keeps old saves + goldens byte-identical. (Prototype: a flat per-kill
    /// award; a per-monster `soul_reward` curve is future tuning.)
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    soul: u64,
    /// GIL SCALE GENERATION (0.0.412): every Gil number in the game is ×100 what it was, so a
    /// save written before that carries its balance in the old unit. 0 = pre-rescale (the
    /// field is absent), `GIL_SCALE_CURRENT` = scaled. `migrate_gil_scale` (load path) lifts an
    /// old balance and its ledger tallies by the same factor exactly once.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub gil_scale: u8,
    /// PARTY SLOTS BOUGHT (0.0.413) — the deploy cap, `party_slots()`. 0 = never bought: the cap
    /// is then whatever is standing in the field (a starter pair keeps both), so an old save
    /// loses nothing. Grows one at a time through `BuyPartySlot`, to `MAX_PARTY`.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub party_slots: u8,
    /// BESTIARY (0.0.184): lifetime kills per BESTIARY GROUP — same-model
    /// monsters share one entry/pool (`MonsterDef.bestiary_group`, empty =
    /// the def stands alone under its own id). Grows only on the first kill
    /// (skip-if-empty keeps old saves/goldens stable until then).
    /// Account-shared when the multi-party split comes.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub monster_kills: BTreeMap<String, u32>,
    /// Claimed bestiary milestone rewards per def — bit i set = milestone i of
    /// `BESTIARY_MILESTONES` paid out (idempotent across replays by design:
    /// the bit is part of the state, so a milestone pays exactly once).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bestiary_claimed: BTreeMap<String, u8>,
    /// MEASUREMENT COUNTERS (0.0.227) — account-lifetime monotonic tallies of
    /// player actions, read by the ONLINE layer (quest/battle-pass measurement:
    /// the server diffs them across verified saves). The sim only counts — it
    /// attaches no meaning and never reads them back. Bumped ONLY on the success
    /// path of each action (a silent no-op input never counts). Skip-if-empty
    /// keeps pre-0.0.227 saves byte-identical until the first counted action.
    #[serde(default, skip_serializing_if = "Measures::is_empty")]
    pub measures: Measures,
    /// Items sent to Steam (0.0.478), by catalog id: every accepted `SendToSteam` took one from
    /// the bag and counted it here. The server mints each count past its watermark as a Steam
    /// Inventory item; the game copy is already gone, so nothing exists twice.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub steam_sent: std::collections::BTreeMap<String, u64>,
    /// Farm Banner boxes opened (0.0.479), by box id: every accepted `OpenFarmBox`. The server
    /// credits one bound ticket per open past its watermark, for the event that box belongs to.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub farm_opened: std::collections::BTreeMap<String, u64>,
    /// Personal bests (maxima). Skip-if-empty, so a save that never set one is byte-identical.
    #[serde(default, skip_serializing_if = "Records::is_empty")]
    pub records: Records,
    /// PATHS OF SOUL — the party-wide meta skill tree (node id → level), spent
    /// with `soul`. Per-save (wiped by a new game). Skip-if-empty keeps old
    /// saves + goldens byte-identical.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub soul_paths: BTreeMap<String, u8>,
    /// TRAINING (0.0.406) — slot id → level bought with Gil (`training.rs`). Party-shared,
    /// per-save. Skip-if-empty keeps old saves + goldens byte-identical.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub training: BTreeMap<String, u8>,
    /// The levels RESOLVED against the catalog (`training::resolve`) — what `training::apply`
    /// stamps on the party each tick, held here because `tick` has no catalog.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub training_mods: Vec<crate::items::StatMod>,
    /// CRESTS (0.0.416, `crest.rs`) — crest id → level, lifted with `crest_mats`. Party-shared.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(alias = "relics")]
    pub crests: BTreeMap<String, u8>,
    /// CREST MATERIALS — `<category>_t<tier>` → count. A COUNTER, never a bag item: it takes no
    /// slot and has no market path (user: "ไม่เก็บในช่อง + แลกเปลี่ยนไม่ได้ ติดตัว").
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[serde(alias = "relic_mats")]
    pub crest_mats: BTreeMap<String, u32>,
    /// The crest levels resolved against the catalog (`crest::resolve`), `crest::apply`'s source.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub crest_mods: Vec<crate::items::StatMod>,
    /// STALEMATE CLOCK (0.0.416, re-keyed 0.0.418): ticks a boss has stood with at least one
    /// member down and its HP never falling below the lowest it has been. `tick::break_stalemate`
    /// fells the rest of the party at `STALEMATE_TICKS` — a lone Seraphine against a
    /// self-healing maid boss stood five days in a 14-day run (measured), neither able to end
    /// it. A fight that is WINNING keeps pushing the floor down and never trips it (a kill count
    /// could not tell the two apart: a boss room has one thing to kill). 0 when the floor moves
    /// or nobody is down.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub stalemate_ticks: u32,
    /// The lowest boss HP the clock has seen this fight (0 = no fight watched).
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub stalemate_floor: i32,
    /// TRASH-ROOM stalemate (0.0.429): with no boss up, the zone kill counter the clock last saw,
    /// +1 (0 = not watching). A member down and no kill for `STALEMATE_TICKS` is the same dead
    /// fight as the boss case — a healed Seraphine who cannot kill and cannot die stood 2½ days.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub stalemate_kills: u32,
    /// Craft recipes LEARNED (recipe id), bought one-time with soul + Gil via
    /// `LearnRecipe`. Party-shared knowledge; like `soul` it survives a party
    /// wipe (nothing in the wipe path touches it). Skip-if-empty keeps old
    /// saves + goldens byte-identical.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub recipes_learned: BTreeSet<String>,
    /// Player-chosen name for the lead character (set at character creation).
    /// Skip-if-empty keeps old saves + goldens byte-identical.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub character_name: String,
    /// Player-chosen name for the PARTY — the persistent identity now that there's
    /// no fixed lead (the party is the "you", members come and go). Set at
    /// creation. Skip-if-empty keeps old saves + goldens byte-identical.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub party_name: String,

    /// RETIRED (0.0.380, user: "rare bag เอาออกจากเกมก่อน ต้องคิดใหม่"). Nothing routes here any
    /// more; a save that still holds pieces has them moved to the inventory on load
    /// (`migrate_save_items`). The field, its cap and `rare_threshold` stay so old saves and old
    /// input logs (`MoveRareToInventory`, `SetRareThreshold`) still read; the screen is gone.
    /// Skip-if-empty keeps old saves + goldens byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rare_bag: Vec<Item>,
    /// Rare-bag capacity in slots (base `RARE_BAG_BASE_CAP`, upgradeable). 0 in
    /// the struct means "unset" → `rare_bag_cap()` returns the base; skip-if-0
    /// keeps old saves byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub rare_bag_cap: u32,
    /// Minimum rarity routed to the rare bag (player-set). Default `Rare`: Rare +
    /// Legendary go to the bag, Uncommon stays in the normal stash. Skip-if it's
    /// the default so old saves + goldens stay byte-identical.
    #[serde(default = "default_rare_threshold", skip_serializing_if = "is_default_rare_threshold")]
    pub rare_threshold: crate::items::Rarity,

    /// CARD ALBUM — a SEPARATE dex-style store for cards, parallel to the gear stash
    /// (stash = non-card loot / album = cards). One stack per card type; cards drop /
    /// are bought straight into here, and the card panel equips from here. Keeping cards
    /// out of the 20-slot stash means a growing collection never crowds out gear.
    /// Skip-if-empty keeps old saves + goldens byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub card_album: Vec<Item>,
    /// VESTIGIAL (0.0.217): the card album is now UNBOUNDED — `card_album_cap()`
    /// ignores this field and returns `usize::MAX`. Kept for save compat (old saves
    /// carrying a non-zero value still deserialize; skip-if-0 keeps them byte-identical).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub card_album_cap: u32,

    /// OWNED EXEMPLARS (0.0.220) — the dedicated, UNCAPPED home for Exemplar carrier
    /// items, separate from `inventory` so a precious Exemplar can never overflow-drop.
    /// Purchases bank here; `EquipExemplar`/`UnequipExemplar` and the Resonance feed all
    /// move carriers between here and the equipped member. The Exemplar screen lists this
    /// (available copies) plus what's equipped on party members. `migrate_save_items`
    /// sweeps any Exemplar an older save left in the inventory/stash into here on load.
    /// Skip-if-empty keeps old saves + goldens byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owned_exemplars: Vec<Item>,

    /// The player's REGALIA collection (weapon-overlay carriers) — the twin of
    /// `owned_exemplars`. Account-bound, never in `inventory`/`stash`/market. Skip-if-empty
    /// keeps old saves + goldens byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owned_regalia: Vec<Item>,

    /// POTENTIAL MATERIALS — Sigil / Essence / Boss Core / Track, `def_id` → count. A LEDGER, not
    /// bag items (0.0.354, user: "sigil ไม่ต้องรวมใน inventory เพราะจะรก"): no slot, no cap, never
    /// bound; party-shared like Gil. Mail delivers into it by def id exactly as into a bag stack,
    /// and it is the shape a Steam Inventory itemdef mint wants — fungible id × qty.
    /// Private with verbs so every credit and debit is a named site. Skip-if-empty keeps
    /// goldens that never roll a material byte-identical.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    materials: BTreeMap<String, u32>,

    /// PARTY PROVISION RACK — up to `PARTY_PROVISION_SLOTS` consumables that buff EVERY living
    /// member at once, on top of each member's own slots (docs/party-buff-slots-design.md).
    ///
    /// Party-shared, so it sits here beside `stash`/`gil`/`soul` rather than on an entity: the
    /// rack belongs to the party, and a member leaving must not take a slot with them.
    ///
    /// NO DUPLICATE ITEM across the rack (user) — that is what makes three slots a choice instead
    /// of "fit the best one three times". Enforced in `add_party_provision`, not just in the UI.
    /// Skip-if-empty keeps old saves + goldens byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub party_provisions: Vec<crate::items::PartyProvision>,

    /// STASH — the player-managed DEPOSIT WAREHOUSE (RO Kafra-style), separate from
    /// `inventory` (the working bag drops land in). The player moves items here to
    /// bank them long-term (DepositToStash / WithdrawFromStash); nothing auto-lands
    /// here. Large, upgradeable. Skip-if-empty keeps old saves + goldens byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stash: Vec<Item>,
    /// Stash warehouse capacity in slots (base `STASH_CAPACITY`, upgradeable). 0 =
    /// unset → `stash_cap()` returns the base; skip-if-0 keeps old saves byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub stash_cap: u32,
    /// PERMANENT extra BAG (inventory) slots from Inventory Expander items, capped at
    /// `MAX_INVENTORY_CAP_BONUS`. Added on top of the base + soul-path StashCap in
    /// `inventory_cap()`. Skip-if-0 keeps old saves byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub inventory_cap_bonus: u32,
    /// Stash slots from the Training Satchel (0.0.406), refreshed by `training::apply` every
    /// tick from the levels — held here because `inventory_cap` has no catalog in hand.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub training_stash_slots: u32,
    /// CARD drop-rate MULTIPLIER in basis points (10000 = ×1, 20000 = ×2). Scales
    /// the weight of Card entries at roll time — for balance TESTING (sim-cli
    /// `--card-rate-mult`) and live drop-rate EVENTS (set via the recorded
    /// `SetCardRateBonus` input, so replays stay bit-identical). 0 = unset = ×1;
    /// skip-if-0 keeps old saves byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub card_rate_bonus_bp: u32,


    /// AUTO-RETREAT FARMING — idle parties that hit a wall grind easier content
    /// instead of stalling forever. Consecutive party wipes on the current band
    /// (reset to 0 the moment a band is cleared). At `WIPE_RETREAT_LIMIT` the
    /// party retreats one band and farms it (`handle_party_wipe`). Skip-if-0
    /// keeps old saves + goldens byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub consecutive_wipes: u32,
    /// THE FALL-BACK (0.0.403, WoG's rule, user 2026-09-17: "ตายที่ N+1 กลับไปฟาร์ม N อัตโนมัติ 5 ครั้ง
    /// แล้วค่อยไปที่ N+1"): a wipe on stage N+1 sends the party to stage N in farm mode for
    /// `WIPE_FALLBACK_FARM_CLEARS` clears, then back to N+1. `retreat_farm_remaining` counts the
    /// clears down; `retreat_return_band` is the stage that walled the party (0 = not falling
    /// back). (The 0.0.398 sim dropped the older 5-wipe retreat; this is the one-wipe version.)
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub retreat_farm_remaining: u32,
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub retreat_return_band: u8,
}

/// How many ZONES make up one REGION. The world is 20 regions of 5 zones, one zone per level,
/// and each zone holds 3 sub-zone rooms — 20 x 5 x 3 = the 300 authored zone entries.
///
/// The grouping was never written down: it existed only as the fact that the boss changes every
/// fifth entry, which it does for all 20 groups with no exception. `regions_line_up_with_the_boss_
/// blocks` pins that, so authoring a 21st boss block or a 6-zone region fails instead of silently
/// making a region mean something else.
pub const ZONES_PER_REGION: u8 = 12;

/// The ladder (0.0.398, `docs/stage-ladder-renovation.md` §4): 240 STAGES of three rooms, twenty
/// regions of twelve. The player counts stages; nothing on screen says a level.
pub const STAGES: u16 = 240;
/// The stage the launch ladder ends on — Lv70 in level terms, the top of the gear tiers the
/// launch catalog authors.
pub const LAUNCH_STAGE: u16 = 168;
/// REGION (1..=20) for a STAGE. In-game the player reads "region"; `band` survives in older
/// code and in the serialized `band_clears` / `SelectBand` keys, where it means the stage.
pub fn region_of_stage(stage: u8) -> u8 {
    stage.saturating_sub(1) / ZONES_PER_REGION + 1
}

/// Which of the region's twelve stages this is (1..=12).
pub fn stage_in_region(stage: u8) -> u8 {
    stage.saturating_sub(1) % ZONES_PER_REGION + 1
}

/// The STAGE a (region, index) pair addresses — the inverse of the two above.
pub fn stage_of_region_index(region: u8, index: u8) -> u8 {
    region.saturating_sub(1).saturating_mul(ZONES_PER_REGION) + index.max(1)
}

/// Parse the band number from a zone id like "zone_b5_1" → Some(5). Used to
/// attribute an instance-end clear to its band. None if the id isn't shaped
/// `zone_b<N>_*`.
pub fn band_of_zone(zone_id: &str) -> Option<u8> {
    let rest = zone_id.strip_prefix("zone_b")?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// A DUNGEON RUN in progress: what the party has earned since it entered room 1.
/// Reset on every instance start, read once at the clear. Kept as running totals
/// rather than start-snapshots because XP is spread across members and resets on
/// level-up — a snapshot could not be diffed.
/// How many of a run's drops the end-of-run screen keeps. A screenful; the rest are already in
/// the stash, which is where the player actually looks for them.
pub const RUN_LOOT_SHOWN: usize = 24;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunTally {
    pub started_tick: u64,
    pub start_kills: u64,
    pub xp: u64,
    pub gil: u64,
    /// Items the run banked, for the end-of-run screen to LIST — not just count. Collected at
    /// the DROP; the old summary read the loot escrow, which has been vestigial since
    /// direct-grant (0.0.47) and so reported nothing forever.
    ///
    /// CAPPED at `RUN_LOOT_SHOWN`: this rides in the save and the state hash, and a long run can
    /// bank hundreds of items. A summary screen only ever shows a screenful, so keeping more
    /// would grow every save for nothing.
    ///
    /// ⚠️ `serde(default)` IS THE POINT, not decoration. A plain new field makes every EXISTING
    /// save fail to deserialize — the player's game is replaced by a fresh one and only the
    /// `.UNREADABLE-*` rescue copy saves them. `#[derive(Default)]` does NOT cover this; serde
    /// needs the attribute per field. Every added field in this file carries it.
    /// RENAMED from `items`, which briefly shipped as a u32 count. A save written by that build
    /// carries `items: 5`, and a number cannot deserialize into a Vec — reusing the name would
    /// have broken those saves exactly the way the missing serde(default) did an hour earlier.
    /// serde ignores unknown keys (nothing here uses deny_unknown_fields), so the stale numeric
    /// `items` is simply dropped and `loot` starts empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loot: Vec<crate::items::Item>,
}

/// The DUNGEON CLEAR summary: what the finished run produced. Built the moment the
/// instance's final room is cleared — before the escrow banks, so the loot list is
/// still there to show — and cleared when the party leaves. Its presence IS the
/// signal to the client that a clear screen is due; nothing else needs a flag.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClearSummary {
    /// Instance the run cleared (room 1's zone id — the run's identity).
    pub instance_id: String,
    /// Ticks from entering room 1 to the clear.
    pub ticks: u64,
    pub kills: u64,
    pub xp: u64,
    pub gil: u64,
    /// What the run banked, up to `RUN_LOOT_SHOWN`. The items themselves so the screen can name
    /// and colour them — a bare count reads as "nothing happened" even when it did.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loot: Vec<crate::items::Item>,
    /// The run ENDED BADLY — a party wipe inside a dungeon rather than a clear. Same screen,
    /// opposite verdict; the client reads this to say FAIL instead of CLEAR.
    #[serde(default, skip_serializing_if = "is_false")]
    pub failed: bool,
}

/// A tally that can only ever go UP.
///
/// `Measures` has always said "fields must only ever increase" in a comment, held up by fifty-four
/// hand-written call sites all remembering to use `saturating_add`. That is not an invariant, it
/// is a habit — and a comment claiming an invariant is worse than none, because it gets believed.
///
/// The type has no setter and no subtraction. Writing a decrement is not a mistake anyone can
/// make here; it does not compile. That replaces the habit, a comment, and a source-scanning test
/// that had to grant itself an exemption from another guard to exist.
///
/// It matters because two systems SUBTRACT these. The online layer diffs snapshots for accrual,
/// and a Daily Commission reads `now - start`. A counter that fell would underflow the first and
/// silently read zero progress in the second — a player's day broken with nothing in any log.
///
/// `Deref` gives reads for free; there is deliberately NO `DerefMut`, because `*m.kills = 0`
/// would hand back everything this type exists to prevent.
///
/// `#[serde(transparent)]` keeps the wire shape a bare integer, so saves and golden hashes are
/// byte-identical to the `u64` this replaced.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Counter(u64);

impl Counter {
    pub fn get(self) -> u64 {
        self.0
    }

    /// The only way the number moves. Saturating, matching what every call site did by hand.
    pub fn add(&mut self, n: u64) {
        self.0 = self.0.saturating_add(n);
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    /// The ONE production path besides `add`: the load-time Gil rescale (0.0.412) lifts every
    /// Gil tally by the same factor as the balance, so the ledger identity survives the unit
    /// change. Nothing else may call it — a tally that is scaled twice lies.
    pub(crate) fn scale_for_migration(&mut self, k: u64) {
        self.0 = self.0.saturating_mul(k);
    }

    /// TESTS ONLY — set an arbitrary tally. Production has `add` and nothing else.
    ///
    /// Not `#[cfg(test)]`: other crates' tests need it and would not see it. Not `From<u64>`
    /// either — an impl that reads `m.kills = 5.into()` would hand the gate straight back, and
    /// quietly. The clumsy name IS the guard: it cannot appear in production without being
    /// obvious in the diff.
    #[doc(hidden)]
    pub fn set_for_test(&mut self, n: u64) {
        self.0 = n;
    }

    /// TESTS ONLY — a starting tally for a struct literal. Same reasoning as `set_for_test`.
    #[doc(hidden)]
    pub const fn seeded_for_test(n: u64) -> Self {
        Self(n)
    }
}

impl std::ops::Deref for Counter {
    type Target = u64;
    fn deref(&self) -> &u64 {
        &self.0
    }
}

impl std::fmt::Display for Counter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Account-lifetime action tallies (see `GameState::measures`). Flat additive
/// u64s — the online layer diffs snapshots, so fields must only ever increase.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Measures {
    /// Monsters killed (any monster, boss included).
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub kills: Counter,
    /// Rooms/zones cleared (boss down → zone_cleared).
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub rooms_cleared: Counter,
    /// Star Stones (orbs) successfully applied.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub orbs_used: Counter,
    /// Card feeds that transferred stars/foil.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub cards_fed: Counter,
    /// Exemplar feeds that granted Resonance.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub exemplars_fed: Counter,
    /// Successful craft actions (per action, not per output unit).
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub items_crafted: Counter,
    /// Market listings placed (commodity or per-instance gear).
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub items_listed: Counter,
    /// Items sold to an NPC vendor, counted per ITEM STACK rather than per transaction —
    /// "sell all junk" clears a bagful in one press and a player who did that has sold a bagful.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub items_sold: Counter,
    /// Paths of Soul nodes allocated. Counts the ACT of spending, so it rises once per node and
    /// never falls — a respec, if one is ever added, must not walk it backwards or the daily
    /// reading its delta would go negative.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub soul_nodes_allocated: Counter,

    /// Bosses killed. A subset of `kills`, not an addition to it.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub boss_kills: Counter,
    /// BANDS cleared — a band is three rooms (X-1, X-2, X-3), so this is `rooms_cleared`'s
    /// coarser sibling and roughly a third of it.
    ///
    /// Not derivable from `band_clears`, which looks like the same number and is not: that map is
    /// a claim BACK-LOG and `InputEvent::ResetBandClears` empties it every time the player banks
    /// the rewards. Same trap as `consecutive_wipes` (a streak) versus `wipes` (the history) —
    /// a counter that something else resets cannot also be the lifetime figure.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub bands_cleared: Counter,
    /// Character level-ups. Counts every member's every level, not the party's best.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub levels_gained: Counter,
    /// Damage the PARTY dealt, lifetime. Booked in `record_damage`, so the scope is exactly what
    /// the DPS meter counts: player-team attackers only, post-mitigation, clamped to the target's
    /// remaining HP (overkill is not damage dealt). Monster damage is not in here.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub damage_dealt: Counter,
    /// Party wipes. Counts the same event as `consecutive_wipes` but never resets, so a retreat
    /// or a restart does not erase the history.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub wipes: Counter,
    /// Damage the party RECEIVED. Read off the same hit vectors as `damage_dealt`, filtered by
    /// TARGET rather than attacker — which is why it exists at all: `record_damage` is the DPS
    /// meter and only ever sees player attackers, so incoming damage is invisible to it.
    ///
    /// Raw, like `biggest_hit`: a killing blow counts what was thrown, not the sliver of HP that
    /// was left. The overstatement is bounded to one hit per death.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub damage_taken: Counter,
    /// Fusions RUN (success and failure both; a rejected no-op is not one). Doubles as the
    /// monotonic attempt number the server binds a fusion nonce to (`fusion::binding`): every
    /// attempt is a fresh roll, and a roll issued for attempt N cannot be spent at N+1.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub fusions: Counter,
    /// Gems SOCKETED (0.0.386) — every accepted `SocketGem`, overwrites included.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub gems_socketed: Counter,
    /// Gem FUSIONS run — every accepted `FuseGems`.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub gem_fusions: Counter,
    /// Of those, the ones that FAILED (one gem of the input grade came back).
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub gem_fusions_failed: Counter,
    /// Training levels bought (0.0.406) — every accepted `Train`.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub training_levels: Counter,
    /// Party slots bought (0.0.413) — every accepted `BuyPartySlot`. The server reads it to hand
    /// out the slot's 10-pull ticket (the characters come from gacha, the slot from Gil).
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub party_slots_bought: Counter,
    /// Sealed Caches OPENED (0.0.449) — every accepted `OpenLootbox`. The server pays what is
    /// inside per opened box beyond its watermark (`lootbox_opens`), the way `party_slots_bought`
    /// pays slot tickets: the sim never rolls the contents.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub lootboxes_opened: Counter,
    /// Farm Banner boxes OPENED (0.0.477) — retired 0.0.479 for the per-box `GameState.farm_opened`;
    /// nothing writes it now, and it stays only so a 0.0.477 save still reads.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub farm_boxes_opened: Counter,
    /// Items DISENCHANTED into crest materials (0.0.416) — every accepted `Disenchant`.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub items_disenchanted: Counter,
    /// Crest levels bought (0.0.416) — every accepted `UpgradeCrest`.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    #[serde(alias = "relic_levels")]
    pub crest_levels: Counter,
    /// Of the gear fusions, the ones that FAILED (one piece of the input grade came back).
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub fusions_failed: Counter,
    /// Party member deaths. A wipe is several of these plus a `wipes`.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub deaths: Counter,
    /// Skill activations by the party — `skill_fx` entries, the same list that tells the renderer
    /// which cast VFX to play. Basic attacks are not skills and are not counted.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub skills_cast: Counter,
    /// HP the party restored — EFFECTIVE restore, so a heal rolled onto a full bar adds nothing.
    /// Every source counts (skill, regen, HoT, potion, lifesteal) because they all put HP back;
    /// a monster healing another monster does not, since the healer is not party.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub healing_done: Counter,
    /// Gear pieces EQUIPPED (0.0.490) — every accepted `EquipItem`; the tutorial reads it.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub items_equipped: Counter,
    /// Memory Fragments (Regalia) EQUIPPED on a member — every accepted `EquipRegalia`.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub regalia_equipped: Counter,
    /// Memory Fragment ENHANCEMENTS — every accepted Dust feed.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub regalia_enhanced: Counter,
    /// Memory Fragments SALVAGED into Dust.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub regalia_salvaged: Counter,
    /// POTENTIAL ranks bought (any node, any member) — not `soul_nodes_allocated`, which is the
    /// older Paths of Soul web.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub potential_ranks: Counter,
    /// Party provisions LOADED into the rack — every accepted `AddPartyProvision`.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub party_provisions_loaded: Counter,

    // ---- Gil ledger. Six movements, six tallies; see `GameState::earn_gil` and friends. ----
    //
    // These are what make the balance auditable rather than merely present:
    //
    //     gil + (gil_held - gil_released) == gil_earned + gil_received - gil_spent - gil_sent
    //
    // checked by `GameState::gil_ledger_balances`. Every one is monotonic, as the struct's
    // contract demands — escrow coming back is `gil_released` going UP, never `gil_held` going
    // down, because the online layer diffs snapshots and a field that can fall makes a delta
    // meaningless (and a negative one underflows the BP accrual).
    /// Gil the WORLD paid the party, INCLUDING the GilPct bonus actually credited — the figure
    /// that landed in the balance, not the pre-bonus base, so the equation above is exact.
    /// Summed across accounts this is the money supply minted.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub gil_earned: Counter,
    /// Gil the party paid the WORLD. Summed across accounts, the money supply burned.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub gil_spent: Counter,
    /// Gil received FROM another player. Supply-neutral: never counts as earnings.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub gil_received: Counter,
    /// Gil sent TO another player. Supply-neutral.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub gil_sent: Counter,
    /// Gil ever placed in escrow (lifetime, not the outstanding amount).
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub gil_held: Counter,
    /// Escrow ever returned — cancellations and change. Outstanding = `gil_held - gil_released`.
    #[serde(default, skip_serializing_if = "Counter::is_zero")]
    pub gil_released: Counter,
}

impl Measures {
    pub fn is_empty(&self) -> bool {
        *self == Measures::default()
    }
}

/// Personal BESTS — values that take a `max`, never a sum.
///
/// A separate type from `Measures` on purpose, and the reason is mechanical rather than tidy: the
/// online layer awards battle-pass XP and daily-quest progress by DIFFING two `Measures`
/// snapshots. Diffing a maximum produces a number that means nothing ("your best hit grew by
/// 4,000" is not an achievement), and a `CounterId` pointing at one would quietly pay out for it.
/// Keeping bests in their own type makes registering one as a countable impossible instead of
/// merely discouraged — the same reason Gil moves through verbs rather than a documented rule.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Records {
    /// Largest single damage INSTANCE the party has landed — the raw number the renderer drew,
    /// not the overkill-clamped one the DPS meter is given. See `tick::note_damage_records`.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub biggest_hit: u64,
}

impl Records {
    pub fn is_empty(&self) -> bool {
        *self == Records::default()
    }
}

/// Pool ceiling. Four skills, so three banked casts is "you may open with two and still answer
/// once", which is the shape the design wants — enough to act, not enough to fire everything.
pub const COMMANDER_CHARGE_MAX: u32 = 3;
/// Ticks per regenerated charge. 600 = 60 s at 10 ticks/s, so a full pool is three minutes of
/// standing still — the master dial for the whole Commander family.
pub const COMMANDER_CHARGE_TICKS: u64 = 600;
/// AUTO-CAST NEVER SPENDS BELOW THIS. Three of the six abilities default to auto, and without a
/// reserve a player reaching for smite at the moment that matters would find it unaffordable
/// because their own bulwark spent the charge while they were not looking — the idle convenience
/// taking the active play away. Auto is cautious with the last of the Commander's power.
pub const COMMANDER_AUTO_RESERVE: u32 = 1;

fn commander_charges_full() -> u32 {
    COMMANDER_CHARGE_MAX
}

fn is_full_charges(n: &u32) -> bool {
    *n == COMMANDER_CHARGE_MAX
}

fn is_zero_u64(n: &u64) -> bool {
    *n == 0
}

fn is_zero_u32(n: &u32) -> bool {
    *n == 0
}

fn is_zero_i32(n: &i32) -> bool {
    *n == 0
}

fn is_zero_u8(n: &u8) -> bool {
    *n == 0
}

/// Consecutive party wipes on a band before the idle party RETREATS to farm an
/// easier band (instead of restarting the wall forever).
/// BESTIARY milestone rewards per group: `(kills_needed, gil, soul)` — paid
/// once each, inside the kill-accounting tick (deterministic, no input event;
/// the claimed bit in `bestiary_claimed` makes it exactly-once across replays).
/// Numbers are provisional (จดไว้ปรับได้).
/// See `GameState::gil_scale`.
pub const GIL_SCALE_CURRENT: u8 = 1;
pub const GIL_RESCALE_FACTOR: u64 = 100;

pub const BESTIARY_MILESTONES: [(u32, u64, u64); 4] = [
    (1_000, 50_000, 0),
    (10_000, 300_000, 0),
    (20_000, 0, 500),
    (50_000, 2_000_000, 5_000),
];


/// Two minutes (10 ticks/s) of a boss standing, a member down and its HP floor not moving: the
/// fight is lost.
pub const STALEMATE_TICKS: u32 = 1_200;
/// The same clock with NOBODY down (0.0.443): five minutes without a kill (trash) or a new boss
/// floor is a fight the party cannot end even at full strength.
pub const STALEMATE_TICKS_HEALTHY: u32 = 3_000;

/// Stage clears the party farms on the stage below after a wipe, before it is sent back up —
/// the CEILING; `wipe_fallback_clears` ramps to it.
pub const WIPE_FALLBACK_FARM_CLEARS: u32 = 5;

/// How many clears of stage N−1 a wipe on stage N buys (0.0.414): one in the first stages,
/// one more every six, `WIPE_FALLBACK_FARM_CLEARS` from stage 24. A flat five sent a first-session
/// player who died on stage 2 back to farm stage 1 for fifteen minutes (measured) — the one
/// stretch of the game where a wall must be a beat, not a wait.
pub fn wipe_fallback_clears(stage: u16) -> u32 {
    (1 + u32::from(stage) / 6).min(WIPE_FALLBACK_FARM_CLEARS)
}

/// Base rare-bag size before any upgrade (user spec: 3 slots, upgradeable).
pub const RARE_BAG_BASE_CAP: u32 = 3;

/// Cap on permanent bag expansion from Inventory Expander items (`inventory_cap_bonus`),
/// so the bag grid can't grow without bound.
pub const MAX_INVENTORY_CAP_BONUS: u32 = 50;


// Default raised Rare → Mythical (0.0.137): star-promoted rarity (2★ = Rare)
// plus Rare-base [2] defs pushed ~15-23 drops/day at the old threshold into a
// 3-slot bag (full in ~4h, audit 2026-07-05). Mythical = 3★+ ≈ 3/day — the
// "daily jackpot" bag the 3 slots were sized for. Players can still lower it.
fn default_rare_threshold() -> crate::items::Rarity {
    crate::items::Rarity::Mythical
}

fn is_default_rare_threshold(r: &crate::items::Rarity) -> bool {
    *r == crate::items::Rarity::Mythical
}

/// Per-character data that gets parked when its class is inactive.
/// Grows in later betas (cards, sockets, build name, …) — keep new
/// fields here rather than scattering parallel maps across GameState.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CharacterData {
    pub stats: Stats,
    #[serde(default, skip_serializing_if = "crate::potential::PotentialState::is_empty")]
    pub potential: crate::potential::PotentialState,
    pub equipped: Vec<Item>,
    /// Skills LEARNED from this class's tree (level ≥ 1). Parked per class
    /// so a reclass preserves each class's own build. Default keeps old
    /// saves loadable.
    #[serde(default)]
    pub skills: Vec<crate::skills::SkillState>,
    /// Selected learned actives, including an intentionally empty loadout.
    /// Captured after overlay grants are cleared, preserving their existing
    /// regrant behavior rather than making a bench auto-enable them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_skills: Vec<String>,
    /// CARD PANEL parked for this character (cards equipped to the character, by
    /// class). Default keeps old saves loadable; skip-if-empty keeps them clean.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub card_panel: Vec<Item>,
    /// Equipped Exemplar (skin + stat grant + skill). None = none. Skip-if-none keeps old saves +
    /// goldens byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exemplar: Option<crate::items::Exemplar>,
    /// Equipped Regalia (signature-weapon overlay: stat grant + skill + signature anim). None =
    /// none. Skip-if-none keeps old saves + goldens byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regalia: Option<crate::items::Regalia>,
    /// The emergency consumable bar, the reserved Meal/Tonic slots, and any loaded elemental
    /// rounds. Parked with the character, because they belong to the character.
    ///
    /// These were MISSING, and the struct's own doc says why that matters: it is "the ONE place
    /// that knows every per-character field", and a short list here destroys items rather than
    /// parking them. `bench` captures and then retains the entity out of the grid, so whatever
    /// `capture` does not take is gone — a benched character lost its potions, its meals and its
    /// quivers. Skip-if-empty keeps old saves + goldens byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub consumables: Vec<crate::items::ConsumableSlot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provisions: Vec<crate::items::ConsumableSlot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ammo: Vec<crate::items::AmmoStack>,
}

impl CharacterData {
    /// Brand-new character: default stats, no equipped items, no skills.
    pub fn new() -> Self {
        Self {
            stats: Stats::new(),
            potential: crate::potential::PotentialState::default(),
            equipped: Vec::new(),
            skills: Vec::new(),
            active_skills: Vec::new(),
            card_panel: Vec::new(),
            exemplar: None,
            regalia: None,
            consumables: Vec::new(),
            provisions: Vec::new(),
            ammo: Vec::new(),
        }
    }
}

impl CharacterData {
    /// Capture a live entity's per-character loadout, RESETTING the entity's fields —
    /// the ONE place that knows every per-character field, used by bench / park /
    /// recruit (adding a per-character field = update `capture` + `restore_onto` and
    /// every transfer site is covered; hand-copied field lists have already dropped a
    /// field once). Strips Exemplar-GRANTED skills first so they never persist as
    /// learned skills.
    pub fn capture(e: &mut crate::grid::Entity) -> Self {
        e.clear_exemplar_skill();
        e.clear_regalia_skill();
        Self {
            stats: std::mem::replace(&mut e.stats, crate::stats::Stats::new()),
            potential: std::mem::take(&mut e.potential),
            equipped: std::mem::take(&mut e.equipped),
            skills: std::mem::take(&mut e.skills),
            active_skills: std::mem::take(&mut e.active_skills),
            card_panel: std::mem::take(&mut e.card_panel),
            exemplar: e.exemplar.take(),
            regalia: e.regalia.take(),
            consumables: std::mem::take(&mut e.consumables),
            provisions: std::mem::take(&mut e.provisions),
            ammo: std::mem::take(&mut e.ammo),
        }
    }

    /// Overlay this captured loadout onto a freshly-built entity (deploy / class
    /// restore) and re-materialize the Exemplar's granted skills. Mirror of `capture`.
    pub fn restore_onto(self, e: &mut crate::grid::Entity) {
        e.stats = self.stats;
        e.potential = self.potential;
        e.equipped = self.equipped;
        e.skills = self.skills;
        e.active_skills = self.active_skills;
        e.card_panel = self.card_panel;
        e.exemplar = self.exemplar;
        e.regalia = self.regalia;
        e.consumables = self.consumables;
        e.provisions = self.provisions;
        e.ammo = self.ammo;
        e.apply_exemplar_skill();
        e.apply_regalia_skill();
        // A rebuilt entity has no Iron Edge snapshot. Initialize it before its
        // first attack, without waiting for the next periodic second boundary.
        crate::potential::effects::refresh(std::slice::from_mut(e), 0, true);
    }
}

impl Default for CharacterData {
    fn default() -> Self {
        Self::new()
    }
}

/// A benched (owned, not deployed) character: its class + full loadout. Stored
/// in `GameState.roster` keyed by the character's unique id. `deploy` rebuilds a
/// grid entity from `unit_def`'s class def and overlays `data`; `bench` captures
/// a grid member's live state back into this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RosterChar {
    #[serde(alias = "class_id")]
    pub unit_def: String,
    pub data: CharacterData,
}

/// Live DPS-meter accumulator. Totals player damage by class and by skill since
/// `start_tick`; DPS = total / ((tick − start_tick)/TICKS_PER_SEC). Not persisted.
#[derive(Debug, Clone, Default)]
pub struct DpsTracker {
    /// Tick the current window began (set on first damage / on reset).
    pub start_tick: u64,
    /// Total damage dealt by each player unit_def.
    pub by_class: std::collections::BTreeMap<String, u64>,
    /// Total damage dealt by each skill ("Basic Attack" for non-skill swings).
    pub by_skill: std::collections::BTreeMap<String, u64>,
    /// Damage TAKEN by each player unit_def (0.0.422, tuning telemetry — user: "วัด damage done
    /// healing done damage receive แต่ละตัว").
    pub taken_by_class: std::collections::BTreeMap<String, u64>,
    /// HP soaked by shields, credited to the unit whose skill PLACED the shield (Aegis on the
    /// whole party is Seraphine's block), keyed by unit def (0.0.438).
    pub blocked_by_class: std::collections::BTreeMap<String, u64>,
    /// Damage the unit's own DEF / %-reductions / flat guard took off incoming packets
    /// (0.0.456, user: "จะดูยังไง damage mitigation") — the same window, display-only.
    pub mitigated_by_class: std::collections::BTreeMap<String, u64>,
    /// Healing DONE by each player unit_def (the healer's), same window.
    pub healed_by_class: std::collections::BTreeMap<String, u64>,
    /// Deaths per player unit_def, and ticks each stood ALIVE — the uptime a damage share has
    /// to be read against (user: "หรือเพราะตัวอื่นตายก่อน?").
    pub deaths_by_class: std::collections::BTreeMap<String, u64>,
    pub alive_ticks_by_class: std::collections::BTreeMap<String, u64>,
    /// BASIC-attack damage per player unit_def (user: "basic attack ต้องแยกตัวละคร") — `by_skill`
    /// pools every unit's basics under one key.
    pub basic_by_class: std::collections::BTreeMap<String, u64>,
    /// Damage by (unit def, skill id) — "Basic Attack" for a swing — the per-member breakdown
    /// (0.0.439, user: "แจกแจง dmg ของทุกสกิล + basic").
    pub by_class_skill: std::collections::BTreeMap<(String, String), u64>,
}

// Display-only: never participate in GameState equality (so a round-trip through
// serialize — which drops `dps` — still compares equal). Always-equal.
impl PartialEq for DpsTracker {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for DpsTracker {}

impl GameState {
    /// Record one player damage event for the DPS meter. No-op for non-player
    /// attackers or non-positive amounts. `skill_id = None` ⇒ a basic attack.
    pub(crate) fn record_damage(&mut self, attacker_idx: usize, skill_id: Option<&str>, amount: i32) {
        if amount <= 0 {
            return;
        }
        let e = &self.grid.entities[attacker_idx];
        if e.team != crate::grid::Team::Player {
            return;
        }
        let class = e.unit_def.clone();
        let skill = skill_id.unwrap_or("Basic Attack").to_string();
        if self.dps.start_tick == 0 {
            self.dps.start_tick = self.tick;
        }
        let amt = amount as u64;
        if skill_id.is_none() {
            *self.dps.basic_by_class.entry(class.clone()).or_insert(0) += amt;
        }
        *self.dps.by_class_skill.entry((class.clone(), skill.clone())).or_insert(0) += amt;
        *self.dps.by_class.entry(class).or_insert(0) += amt;
        *self.dps.by_skill.entry(skill).or_insert(0) += amt;
        // Lifetime total rides the same call. The DPS maps are windowed and `reset_dps` clears
        // them; this one never resets, which is the whole difference between a meter and a stat.
        self.measures.damage_dealt.add(amt);
    }

    /// Clear the DPS meter and restart its window at the current tick.
    pub(crate) fn reset_dps(&mut self) {
        self.dps.by_class.clear();
        self.dps.by_skill.clear();
        self.dps.start_tick = self.tick;
    }

    pub fn new(seed: u64, grid_w: i32, grid_h: i32) -> Self {
        Self {
            tick: 0,
            last_click_tick: 0,
            // A new Commander starts with a full pool, not an empty one: an empty pool would make
            // the first three minutes of a new game the only stretch where none of this exists.
            commander_charges: COMMANDER_CHARGE_MAX,
            commander_charge_tick: 0,
            commander_cd: BTreeMap::new(),
            commander_skills_owned: BTreeSet::new(),
            dps: DpsTracker::default(),
            gear_dropped_by_tier: [0; 8],
            rng: Pcg32::new(seed),
            grid: Grid::new(grid_w, grid_h),
            next_monster_id: 1000,
            zone_spawn: None,
            pending_zone_change: None,
            lane: None,
            return_zone_id: String::new(),
            run_tally: RunTally::default(),
            last_clear: None,
            pickup_spent: 0,
            pickup_recharge_at: 0,
            measures: Measures::default(),
            steam_sent: std::collections::BTreeMap::new(),
            farm_opened: std::collections::BTreeMap::new(),
            records: Records::default(),
            projectiles: Vec::new(),
            ground_fields: Vec::new(),
            rare_candidates: Vec::new(),
            rare_candidate_seq: 0,
            traps: Vec::new(),
            pending_explosions: Vec::new(),
            recent_hits: Vec::new(),
            knockbacks: Vec::new(),
            pulls: Vec::new(),
            poison_bursts: Vec::new(),
            field_hits: Vec::new(),
            meteor_impacts: Vec::new(),
            trap_fires: Vec::new(),
            skill_fx: Vec::new(),
            ult_fx: Vec::new(),
            heal_fx: Vec::new(),
            loot_fx: Vec::new(),
            current_unit_def: None,
            current_zone_id: None,
            band_reached: 0,
            band_cleared: 0,
            level_reached: 0,
            apex_reached: 0,
            owned_engrams: [0; ENGRAM_TIERS.len()],
            pending_resonance: BTreeMap::new(),
            record_inputs: false,
            input_log: Vec::new(),
            snapshot_interval: 0,
            snapshots: Vec::new(),
            roster: BTreeMap::new(),
            inventory: Vec::new(),
            gil: 0,
            gil_millis: 0,
            escrow: Vec::new(),
            farm_current_instance: false,
            debug_god: false,
            soul: 0,
            gil_scale: GIL_SCALE_CURRENT,
            party_slots: 0,
            market_ops: Vec::new(),
            next_market_op_id: 0,
            monster_kills: BTreeMap::new(),
            bestiary_claimed: BTreeMap::new(),
            soul_paths: BTreeMap::new(),
            training: BTreeMap::new(),
            training_mods: Vec::new(),
            crests: BTreeMap::new(),
            crest_mats: BTreeMap::new(),
            crest_mods: Vec::new(),
            stalemate_ticks: 0,
            stalemate_floor: 0,
            stalemate_kills: 0,
            recipes_learned: BTreeSet::new(),
            character_name: String::new(),
            party_name: String::new(),
            rare_bag: Vec::new(),
            rare_bag_cap: 0,
            card_album: Vec::new(),
            owned_exemplars: Vec::new(),
            owned_regalia: Vec::new(),
            materials: BTreeMap::new(),
            party_provisions: Vec::new(),
            card_album_cap: 0,
            stash: Vec::new(),
            stash_cap: 0,
            inventory_cap_bonus: 0,
            training_stash_slots: 0,
            card_rate_bonus_bp: 0,
            rare_threshold: default_rare_threshold(),
            consecutive_wipes: 0,
            retreat_farm_remaining: 0,
            retreat_return_band: 0,
        }
    }

    /// Record one instance-end (room-3 boss) clear — one full band, three rooms.
    ///
    /// `band` is unused now that the per-band BACK-LOG is gone. It stays in the signature because
    /// the caller has the number and a future per-band breakdown would want it; dropping and
    /// re-threading it later is churn for nothing.
    pub fn record_band_clear(&mut self, band: u8) {
        self.measures.bands_cleared.add(1);
        self.band_cleared = self.band_cleared.max(u32::from(band));
    }

    /// The highest band a party may TRAVEL to by its own input (SelectBand / EnterInstance): any
    /// band it has already reached — floored by the band it stands in (proof of reach for a save
    /// older than `band_reached`) — or the one after the highest band it has CLEARED.
    pub fn band_travel_max(&self) -> u32 {
        let standing = self.current_zone_id.as_deref().and_then(band_of_zone).unwrap_or(0) as u32;
        self.band_reached.max(standing).max(1).max(self.band_cleared + 1)
    }

    /// Total stash capacity in SLOTS — the UNIFIED cap counting EVERY entry (gear,
    /// material, junk). One stash `Vec` entry = one slot; stacking more of an item
    /// you already hold doesn't consume a slot. Base `drops::INVENTORY_CAPACITY`, raised
    /// by the PATHS OF SOUL "Hoard" (StashCap) upgrade.
    pub fn inventory_cap(&self) -> usize {
        crate::drops::INVENTORY_CAPACITY
            + crate::soul_paths::path_bonus(
                &self.soul_paths,
                crate::soul_paths::PathEffect::StashCap,
            )
            .max(0) as usize
            + self.inventory_cap_bonus.min(MAX_INVENTORY_CAP_BONUS) as usize
            + self.training_stash_slots as usize
    }

    /// Effective rare-bag capacity (base when unset/0). Upgrades raise the stored
    /// `rare_bag_cap` above the base.
    pub fn rare_bag_cap(&self) -> usize {
        self.rare_bag_cap.max(RARE_BAG_BASE_CAP) as usize
    }

    /// Effective stash WAREHOUSE capacity (base when unset/0). Upgrades raise the
    /// stored `stash_cap` above `drops::STASH_CAPACITY`.
    pub fn stash_cap(&self) -> usize {
        self.stash_cap.max(crate::drops::STASH_CAPACITY as u32) as usize
    }

    /// The card album is UNBOUNDED (0.0.217, user: "album ไม่จำกัดเสมอ player ไม่ต้อง
    /// ขยาย") — every distinct card identity keeps its own row, no upgrade needed. The
    /// count is still naturally bounded by the catalog (defs × stars × foil). Returning
    /// `usize::MAX` makes every `len() < cap` / `len() >= cap` guard a no-limit push, so
    /// drops always bank and the feed refund path (an album-full safety net) stays inert.
    pub fn card_album_cap(&self) -> usize {
        usize::MAX
    }

    /// Record a heal that landed, for the renderer and for `healing_done`.
    ///
    /// A no-op at zero, which is the common case: a target already at full HP absorbs the roll
    /// and gains nothing, and a green "+0" floating off a full bar is noise, not information.
    pub(crate) fn note_heal(&mut self, target_id: u32, healer_id: u32, amount: i32, kind: HealKind) {
        if amount <= 0 {
            return;
        }
        self.heal_fx.push(HealEvent { target_id, healer_id, amount, kind });
    }

    /// The party's Gil. Read-only accessor; the field is private so that every MOVEMENT has to
    /// name its counterparty (see the field's doc).
    /// Soul on hand. The field is private for the same reason `gil` is: a `pub u64` means every
    /// site moves it by hand, so no source or sink is ever complete and a leak has nowhere to
    /// show up. Soul survives a party wipe, so a mistake here is permanent.
    pub fn soul(&self) -> u64 {
        self.soul
    }

    /// Soul entering the party from the WORLD — a kill, a bestiary milestone. Naming the source
    /// is the point: when soul needs counters the way Gil got them, every faucet is already
    /// enumerated here instead of being found by grepping for `+=`.
    pub fn earn_soul(&mut self, n: u64, _src: SoulEarn) {
        self.soul = self.soul.saturating_add(n);
    }

    /// Spend soul on a sink. ATOMIC: checks affordability and deducts together, so a caller
    /// cannot check one amount and deduct another, and an unaffordable spend changes nothing.
    /// Returns whether it went through.
    #[must_use]
    pub fn spend_soul(&mut self, n: u64, _sink: SoulSpend) -> bool {
        if self.soul < n {
            return false;
        }
        self.soul -= n;
        true
    }

    #[doc(hidden)]
    pub fn set_soul_for_test(&mut self, n: u64) {
        self.soul = n;
    }

    pub fn gil(&self) -> u64 {
        self.gil
    }

    /// Gil the WORLD pays the party — a monster's purse, an NPC sale, a quest reward. The money
    /// supply GROWS here: every `GilEarn` variant is a faucet.
    ///
    /// Only faucets earn the Paths-of-Soul GilPct bonus, and that is enforced by SHAPE rather than
    /// by a match arm: `receive_gil` contains no bonus code at all, so "pay the % on money that
    /// came from another player" is not something this API can express. It would be a printer —
    /// two accounts trading one item back and forth would mint the bonus every lap.
    ///
    /// `base` is added in full (no loss); the bonus (`base · pct%`) accumulates in thousandths and
    /// flushes to whole Gil as it crosses 1000 — so even a 2-Gil sale eventually pays its full %
    /// once enough tiny sales add up, instead of rounding to 0 each time. Integer math, no PRNG.
    pub fn earn_gil(&mut self, base: u64, _src: GilEarn) {
        // Gil crest (0.0.433): +% on every source. 0 without it.
        let base = base.saturating_mul((10_000 + crate::crest::gil_gain_bp(self).max(0)) as u64) / 10_000;
        self.gil = self.gil.saturating_add(base);
        self.measures.gil_earned.add(base);
        let pct = crate::soul_paths::path_bonus(
            &self.soul_paths,
            crate::soul_paths::PathEffect::GilPct,
        )
        .max(0) as u64;
        if pct == 0 {
            return;
        }
        // bonus = base·pct/100 Gil; tracked ×1000 (millis) = base·pct·10.
        self.gil_millis = self
            .gil_millis
            .saturating_add(base.saturating_mul(pct).saturating_mul(10));
        let whole = self.gil_millis / 1000;
        if whole > 0 {
            self.gil = self.gil.saturating_add(whole);
            self.measures.gil_earned.add(whole);
            self.gil_millis %= 1000;
        }
    }

    /// Gil the party pays the WORLD — a refine fee, a craft cost, a recruit. The supply SHRINKS,
    /// so the `GilSpend` variants are the game's sink inventory: `monetary-policy` needs that list
    /// to stay complete, and now it cannot be added to anywhere else.
    ///
    /// The caller must already have checked the balance. This keeps the plain `-=` that all eleven
    /// call sites used, so the arithmetic — including how it behaves on underflow — is exactly what
    /// it was before the funnel existed. Hardening that is a separate decision from moving it.
    pub fn spend_gil(&mut self, n: u64, _sink: GilSpend) {
        self.gil -= n;
        self.measures.gil_spent.add(n);
    }

    /// Gil ANOTHER PLAYER pays the party. The supply is UNCHANGED — the money already existed in
    /// someone else's pocket. No GilPct bonus, deliberately and structurally (see `earn_gil`).
    pub fn receive_gil(&mut self, n: u64, _from: GilPeer) {
        self.gil = self.gil.saturating_add(n);
        self.measures.gil_received.add(n);
    }

    /// Gil the party pays ANOTHER PLAYER. Supply unchanged. Caller checks the balance.
    pub fn send_gil(&mut self, n: u64, _to: GilPeer) {
        self.gil -= n;
        self.measures.gil_sent.add(n);
    }

    /// Gil moved into ESCROW — a listing's hold, a buy order's `max_price`. NOT a spend: it comes
    /// back when the order is cancelled, and comes back as CHANGE when the order fills below the
    /// price held. Booking escrow as spending would show a player who listed and cancelled a
    /// hundred times as having spent a fortune they still have.
    pub fn hold_gil(&mut self, n: u64, _h: GilHold) {
        self.gil -= n;
        self.measures.gil_held.add(n);
    }

    /// Escrow coming back — a cancelled order, or the change from one that filled cheap.
    pub fn release_gil(&mut self, n: u64, _h: GilHold) {
        self.gil = self.gil.saturating_add(n);
        self.measures.gil_released.add(n);
    }

    // ---- Potential materials ledger ----

    /// Units of `def_id` held in the materials ledger (0 for anything else).
    pub fn material_count(&self, def_id: &str) -> u32 {
        self.materials.get(def_id).copied().unwrap_or(0)
    }

    /// Every (def id, count) the ledger holds, for views and the market listing.
    pub fn materials(&self) -> impl Iterator<Item = (&str, u32)> {
        self.materials.iter().map(|(k, v)| (k.as_str(), *v))
    }

    /// Add `n` units of `def_id`. `from` names the faucet — a drop, a mail delivery, a load-time
    /// migration — so a new credit site has to say what it is.
    pub fn credit_material(&mut self, def_id: &str, n: u32, _from: MaterialSource) {
        if n == 0 {
            return;
        }
        let e = self.materials.entry(def_id.to_string()).or_insert(0);
        *e = e.saturating_add(n);
    }

    /// Remove `n` units of `def_id`; `false` (and nothing removed) when the ledger holds fewer.
    pub fn debit_material(&mut self, def_id: &str, n: u32) -> bool {
        let Some(e) = self.materials.get_mut(def_id) else {
            return n == 0;
        };
        if *e < n {
            return false;
        }
        *e -= n;
        if *e == 0 {
            self.materials.remove(def_id);
        }
        true
    }

    /// Set the balance outright. TESTS ONLY, and available to the integration suite as well as the
    /// unit tests, which is why it is not `#[cfg(test)]` — `tests/suite/` is a separate crate and
    /// would not see it. Production code cannot reach the field at all, which is the point.
    #[doc(hidden)]
    pub fn set_gil_for_test(&mut self, n: u64) {
        // Book the jump so `gil_ledger_balances` stays true. Done by hand rather than by calling
        // earn_gil, because that would apply the GilPct bonus and land somewhere other than `n`.
        if n >= self.gil {
            self.measures.gil_earned.add(n - self.gil);
        } else {
            self.measures.gil_spent.add(self.gil - n);
        }
        self.gil = n;
    }

    /// The party's DEPLOY CAP: the slots owned, at least one, at most `MAX_PARTY`. The field is
    /// the source — `claim_fielded_slots` folds what stands in the field into it at genesis and on
    /// load, so a pre-0.0.413 save's five members are five OWNED slots, and benching one never
    /// takes the slot away (0.0.418: reading the fielded count live did exactly that).
    pub fn party_slots(&self) -> usize {
        usize::from(self.party_slots).max(1).min(crate::data::MAX_PARTY)
    }

    /// Own every slot the field already occupies. Genesis and the load path; idempotent.
    pub fn claim_fielded_slots(&mut self) {
        let fielded = self
            .grid
            .entities
            .iter()
            .filter(|e| matches!(e.team, crate::grid::Team::Player) && !e.is_pet && e.owner_id == 0)
            .count()
            .min(crate::data::MAX_PARTY) as u8;
        self.party_slots = self.party_slots.max(fielded);
    }

    /// LOAD-TIME: lift a pre-0.0.412 save's Gil and its ledger tallies ×100, once. The tallies
    /// move with the balance so `gil_ledger_balances` reads the same truth in the new unit.
    pub fn migrate_gil_scale(&mut self) {
        if self.gil_scale >= GIL_SCALE_CURRENT {
            return;
        }
        self.gil = self.gil.saturating_mul(GIL_RESCALE_FACTOR);
        let m = &mut self.measures;
        for c in [&mut m.gil_earned, &mut m.gil_spent, &mut m.gil_received, &mut m.gil_sent, &mut m.gil_held, &mut m.gil_released] {
            c.scale_for_migration(GIL_RESCALE_FACTOR);
        }
        self.gil_scale = GIL_SCALE_CURRENT;
    }

    /// Does the Gil balance equal what the six movement tallies say it should?
    ///
    /// ```text
    /// gil + outstanding escrow == earned + received - spent - sent
    /// ```
    ///
    /// True by construction today, which is exactly why it is worth asserting: it stops being
    /// true the moment a new path moves Gil without booking it, and that is the failure this is
    /// here to catch. Scope is the SIM's view — Gil the verify-server grants outside a replay
    /// never passes through here and is not in this equation.
    pub fn gil_ledger_balances(&self) -> bool {
        let m = &self.measures;
        let outstanding = m.gil_held.get().saturating_sub(m.gil_released.get());
        let credits = m.gil_earned.get() as u128 + m.gil_received.get() as u128;
        let debits = m.gil_spent.get() as u128 + m.gil_sent.get() as u128;
        self.gil as u128 + outstanding as u128 + debits == credits
    }
}

/// Who paid, when the WORLD pays the party. Every variant GROWS the money supply — adding one is
/// adding a faucet, and `docs/economy-invariants.md` asks whether the sinks grew to match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GilEarn {
    /// A monster's `gil_reward`.
    MonsterKill,
    /// Sold to an NPC vendor.
    NpcSale,
    /// A quest / bestiary / story payout applied inside the sim.
    Quest,
    /// Value recovered when an item is broken down.
    Salvage,
    /// Laplus' Pickpocket / Grand Heist: a share of the touched enemy's own `gil_reward`,
    /// paid at once (0.0.464).
    Pickpocket,
    /// A DEBUG grant (`SimBridge::debug_give_currency`), already gated by `debug_blocked()`.
    ///
    /// It gets a variant rather than a back door so that cheated Gil is COUNTED like any other
    /// faucet. Money that appears without passing through the ledger is money the balance check
    /// cannot explain, and "the debug tool did it" is exactly the answer an audit needs to be able
    /// to reach — silently is how it would otherwise look identical to a leak.
    Debug,
}

/// An amount of Gil.
///
/// A newtype for the same reason `Counter` is one: the API is what enforces the rule. There is
/// deliberately NO `impl Sub` — money that goes short must say what it wants (`checked_sub` to
/// refuse, `saturating_sub` to floor), because a bare `-` on a u64 is an underflow and the
/// release profile sets `overflow-checks = true`, which makes that a panic and `panic = "abort"`
/// makes it the process. `Account.gil` was exactly that shape until today.
///
/// It also separates money from IDs, which are `u64` too: `claim_mail(mail_id, gil)` and
/// `buy_item_hold(listing_id, max_price)` each take an id and an amount side by side, and
/// swapping them compiles today.
///
/// `#[serde(transparent)]` keeps the wire shape a bare integer — old saves read back unchanged.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Gil(u64);

impl Gil {
    pub const ZERO: Gil = Gil(0);
    pub const fn new(n: u64) -> Self {
        Self(n)
    }
    pub const fn get(self) -> u64 {
        self.0
    }
    pub const fn is_zero(&self) -> bool {
        self.0 == 0
    }
    #[must_use]
    pub fn saturating_add(self, o: Gil) -> Gil {
        Gil(self.0.saturating_add(o.0))
    }
    /// Refuses to go short. Use this wherever "not enough" is a real outcome.
    #[must_use]
    pub fn checked_sub(self, o: Gil) -> Option<Gil> {
        self.0.checked_sub(o.0).map(Gil)
    }
    /// Floors at zero. Only for places where a short result is genuinely meant to be zero.
    #[must_use]
    pub fn saturating_sub(self, o: Gil) -> Gil {
        Gil(self.0.saturating_sub(o.0))
    }
    /// Scale by basis points — the fee/bonus shape used throughout the economy.
    #[must_use]
    pub fn scale_bp(self, bp: u64) -> Gil {
        Gil(self.0.saturating_mul(bp) / 10_000)
    }
}

/// Where soul came FROM. One variant per faucet; add a variant rather than an untagged call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoulEarn {
    /// The per-kill award.
    MonsterKill,
    /// A bestiary milestone claim.
    Bestiary,
    /// Debug/dev grant — never a shipping path.
    Debug,
}

/// Where soul WENT. Same contract as `GilSpend`: this list is the sink inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoulSpend {
    /// Learning a craft recipe.
    CraftLearn,
    /// Allocating a Paths of Soul node.
    SoulPath,
}

/// Where it went, when the party pays the WORLD. Every variant SHRINKS the supply. This list IS
/// the sink inventory; there is no other way to remove Gil from the economy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GilSpend {
    /// Paying to learn a craft recipe.
    CraftLearn,
    /// A craft action's Gil component.
    Craft,
    /// Bought from an NPC shop.
    Shop,
    /// Recruiting a member straight into the field party.
    RecruitField,
    /// Recruiting a member into the roster.
    RecruitRoster,
    /// The Star Stone application fee.
    OrbFee,
    /// A Potential rank's Gil cost (v2 ladder) — the sink that climbs to seven figures.
    Potential,
    /// A Training level (0.0.406) — the sink with no end.
    Training,
    /// A party slot (0.0.413) — cheap, four of them, then never again.
    PartySlot,
}

/// Where a materials-ledger credit came from (see `GameState::credit_material`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialSource {
    /// A monster drop (sim-rolled) or a server-rolled drop delivered through the drop path.
    Drop,
    /// A mail payload — a market delivery or an operator grant.
    Mail,
    /// Load-time migration of a pre-ledger save that held the material as a bag stack.
    Migration,
    /// The other side of a Sigil merge / split — the same material in another denomination.
    Conversion,
    /// A salvaged Regalia (0.0.489) — the card is consumed for its Memory Dust.
    Salvage,
}

/// The other side of a PLAYER-to-PLAYER move. The supply is unchanged in both directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GilPeer {
    /// Proceeds of a market sale.
    MarketSale,
    /// Paying for a market purchase.
    MarketBuy,
    /// A mail payload's Gil, kind UNKNOWN to the sim.
    ///
    /// `claim_mail` is handed a bare `gil: u64` with no `MailKind`, so the sim cannot tell sale
    /// proceeds from a refund from an operator gift. Booking all three as peer money is what the
    /// code did before this funnel existed (a raw add, no bonus), so it is what preserves
    /// behaviour — but it will misattribute refunds and gifts once these are counted. Plumbing
    /// the kind through `claim_mail` is the fix, and it belongs with the counters, not here.
    MailPayload,
}

/// What an escrow hold is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GilHold {
    /// A listing's fee hold.
    Listing,
    /// A buy order's `max_price`, of which the unspent part returns as change.
    BuyOrder,
}

/// In-flight projectile (ranged attack). Damage is computed at fire
/// time (attacker's effective atk + element multiplier vs target's
/// effective def + element at fire time) and stored here, so even if
/// the attacker's stats change during flight the projectile still carries
/// the value it was launched with. If the target dies before arrival
/// the projectile whiffs (no damage applied; the projectile is removed).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Projectile {
    pub target_id: u32,
    /// Cell the attacker was on when this projectile was fired. Used only
    /// by the renderer for spawn→target interpolation.
    pub spawn_pos: Cell,
    /// Cell the target was on when this projectile was fired. Damage is
    /// applied to `target_id` regardless of where it has moved by
    /// arrival; this is the visual flight endpoint, not the homing
    /// point.
    pub target_pos: Cell,
    pub spawn_tick: u64,
    pub arrive_at_tick: u64,
    pub damage: i32,
    /// True for a magic bolt (renderer draws an animated fireball instead
    /// of the projectile sprite). Set from the firer's damage type.
    pub is_magic: bool,
    /// AoE explosion radius in cells (Chebyshev). 0 = single-target
    /// (the default — hits only `target_id`). 1 = a 3×3 blast centred on
    /// the impact, hitting every alive `victim_team` entity in range.
    #[serde(default)]
    pub aoe_radius: i32,
    /// Which team the blast damages. Stored at fire time so an AoE still
    /// detonates correctly even if the primary target dies mid-flight.
    #[serde(default)]
    pub victim_team: Team,
    /// Renderer hint: a multi-bolt SKILL (fire/lightning/frost/earth bolt) RAINS
    /// from the sky onto the target; a BASIC magic attack flies from the staff.
    /// Not serialized (transient flight cosmetic) → golden hashes stay identical.
    #[serde(skip)]
    pub sky_fall: bool,
    /// Renderer hint: which SKILL fired this, "" for a basic attack.
    ///
    /// The renderer had no way to tell a Lightning Bolt from a Fire Bolt — every magic
    /// projectile drew the same fireball sheet, because `is_magic` was the only thing it could
    /// ask. Carrying the id lets the client pick both the flight art and the per-bolt impact
    /// through the map it already keeps (`SKILL_HIT_FX`), rather than the renderer growing its
    /// own idea of elements.
    ///
    /// SERIALIZED since 0.0.445: it started as a flight cosmetic (`#[serde(skip)]`), but the
    /// Resonance ladder reads it at landing (`unit_rank::on_aoe_hits` — Rapis S1 refunds Fireball
    /// off the skill id), so an anchor that dropped it replayed differently from the client
    /// that kept it. Skip-if-empty keeps basic-attack projectiles byte-identical.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub skill_id: String,
    /// Entity id of the firer — used at impact to attribute on-kill effects
    /// (Arrow Crafter / Steal …) to a RANGED killer and to drive Knock Back Shot
    /// (push the victim away from the archer). 0 = unknown. Not serialized
    /// (transient flight data, like `sky_fall`); a reload mid-flight just drops the
    /// attribution for that one shot → golden hashes stay identical.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub attacker_id: u32,
    /// True when this shot is a BASIC attack (no skill cast) — card special
    /// procs/reflect fire only for basics, matching the melee rule. Absent when
    /// false, so a state with no basic-attack shot in flight stays byte-identical.
    /// It is `skip_serializing_if`, NOT `serde(skip)`: the sim READS it at impact, so a
    /// value that vanishes from the anchor makes the server's replay diverge from the
    /// client with an EMPTY input log — an unprovable save, not a dropped cosmetic.
    #[serde(default, skip_serializing_if = "is_false")]
    pub is_basic: bool,
    /// VISUAL multi-hit count carried to impact (Twin Strafe = 2): the single
    /// projectile pushes ONE HitEvent with `hits` = this on arrival, so the renderer
    /// splits the one damage instance into N numbers (like Double Attack). 0/1 = a
    /// plain single number. SERIALIZED (audit 2026-10-02): it gates the impact's HitEvent, which
    /// crit riders, damage records and Winter Chill read — a transient lost on a mid-flight save
    /// made the server's replay diverge from the client.
    #[serde(default, skip_serializing_if = "is_zero_u32_state")]
    pub display_hits: u32,
    /// The shot crit (0.0.472). Its HitEvent is a Crit, which `crit_riders` (gauge + Spark) and
    /// `biggest_hit` read — so it must survive a save mid-flight (audit 2026-10-02).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub crit: bool,
    /// BURN tier (0 = none): each enemy the blast hits BURNS for
    /// `tier ×` this projectile's `damage` over 8 s. Absent when 0, like `attacker_id`.
    /// It is `skip_serializing_if`, NOT `serde(skip)`: the sim READS it at impact, so a
    /// value that vanishes from the anchor makes the server's replay diverge from the
    /// client with an EMPTY input log — an unprovable save, not a dropped cosmetic.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub burn_tier: u8,
    /// TOTAL-damage multiplier in bp (0 = none).
    /// At impact the blast's damage becomes `damage × this / 10000` SPLIT evenly
    /// among the victims (a lone target eats the whole multiplier). Absent when 0.
    /// It is `skip_serializing_if`, NOT `serde(skip)`: the sim READS it at impact, so a
    /// value that vanishes from the anchor makes the server's replay diverge from the
    /// client with an EMPTY input log — an unprovable save, not a dropped cosmetic.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub focus_mult_bp: u32,
    /// SLOW percent applied to each victim on
    /// impact (0 = none). Absent when 0.
    /// It is `skip_serializing_if`, NOT `serde(skip)`: the sim READS it at impact, so a
    /// value that vanishes from the anchor makes the server's replay diverge from the
    /// client with an EMPTY input log — an unprovable save, not a dropped cosmetic.
    #[serde(default, skip_serializing_if = "is_zero_i32")]
    pub pin_slow_pct: i32,
    // The three fields above (burn_tier / focus_mult_bp / pin_slow_pct) were authored ONLY by
    // the mage/archer runes, retired in 0.0.369. They are always zero today and kept because
    // the anchor's shape is the contract (replay.rs) and the impact branches still work — a
    // future skill can set them without touching the serialized shape again.
    /// Render tint hint: the bolt's damage element, so the renderer can colour it
    /// (a Holy bolt like Sagitta Lucis draws gold, not the orange fireball). The sim
    /// never reads this — damage is locked at fire time (`damage`) — so it's transient
    /// (serde-skip, like `sky_fall`); a reload mid-flight just drops the tint for that
    /// one shot, hashes stay identical. SERIALIZED since the audit of 2026-10-02: the impact now
    /// runs the full landing (`combat::land_blow`), whose Toughness chip reads the element.
    #[serde(default, skip_serializing_if = "is_physical_element")]
    pub element: crate::grid::Element,
}

/// A persistent GROUND FIELD placed at a cell (Assassin Poison Cloud — and
/// reusable for future Elementalist fields: Quicksand / Glacial Storm / Meteor).
/// Fixed at its `center`; affects `victim_team` entities within `radius`
/// (Chebyshev) once per regen interval; ages out over `ticks_left`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroundField {
    pub center: Cell,
    pub radius: i32,
    /// The team this field affects (the opposite of the caster's team).
    pub victim_team: Team,
    /// Ticks remaining before the field dissipates (decremented each tick).
    pub ticks_left: u32,
    pub kind: GroundFieldKind,
    /// Entity id of the caster — credits kills (drops/XP) + DPS for a damaging
    /// field back to the Elementalist. `#[serde(default)]` so older saves (which
    /// only had Poison Cloud, where it's unused) still load. 0 = no/unknown caster.
    #[serde(default)]
    pub source_id: u32,
    /// AURA mode (Sanctus Terra): the field FOLLOWS this entity — `tick_ground_fields`
    /// re-centres it on the entity's cell each tick while it lives (freezes in place if
    /// the carrier dies/despawns). 0 (default) = a normal fixed field; skip-if-0 keeps
    /// every existing field + goldens byte-identical.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub follow_id: u32,
    /// Skill id that created this field, for DPS-meter attribution (else its
    /// per-interval damage is logged as "Basic Attack"). `#[serde(default)]` +
    /// skip-if-empty so older saves + field-less goldens stay byte-identical.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub skill_id: String,
    /// Victim ids this field has ALREADY hit, for a `once_per_target` field (Lord
    /// of Vermilion: each unit takes its 20-hit instance ONCE, but a unit entering
    /// the AoE later gets its own). Empty + unused for normal fields. `#[serde(
    /// default)]` so older saves load.
    #[serde(default)]
    pub hit_ids: Vec<u32>,
}

/// What a pending candidate IS. The three sources differ in every rule that matters —
/// whether they always pay, whether they expire, which star table applies, where the item
/// banks — so the kind is stored rather than inferred.
///
/// - `Drop` — a LOTTERY ticket. The server pays ~`hit_ppm` of them and the rest expire by
///   TTL. This is stage 2 of the two-stage decoy pipeline.
/// - `Craft` — PAID FOR with materials and Gil. Always pays. Rolls the craft star table
///   (1★ floor) and banks like a purchase.
/// - `Gear` — a piece of equipment that HAS ALREADY DROPPED. Always pays; the only open
///   question is its star count and bonus lines, which is exactly what a client must not
///   decide (4★ is Legendary). Rolls the drop star table, `boss` selecting the generous
///   one, and banks as loot so rare-bag routing still applies.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum CandidateKind {
    #[default]
    Drop,
    Craft,
    Gear {
        boss: bool,
    },
}

impl CandidateKind {
    /// Serde skip predicate — a `Drop` candidate serializes exactly as it did before
    /// the field existed, so every pre-0.0.254 save and golden stays byte-identical.
    pub(crate) fn is_drop(&self) -> bool {
        matches!(self, CandidateKind::Drop)
    }

    // Deliberately NO `always_pays()` / `is_not_drop()` helper. A predicate like that reads
    // fine and is wrong at exactly one call site: a grant handler must match its own kind
    // EXACTLY, or a nonce issued for one kind resolves another's ticket. That bug existed for
    // the length of one test run; do not reintroduce the shape that caused it.
}

/// A PENDING server-roll rare candidate — see `GameState::rare_candidates`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RareCandidate {
    pub seq: u64,
    /// The item this candidate is FOR (a card def id, or a recipe's output) — fixed
    /// at accrual, so the server grant can't be steered onto a different item.
    pub def_id: String,
    /// Accrual tick (drives the TTL expiry in `tick`). A `Craft` candidate is EXEMPT
    /// from that expiry — see `expire_rare_candidates`.
    pub tick: u64,
    /// Which grant event resolves this candidate. Defaults to `Drop` so an old save
    /// loads unchanged, and skips serialization in that case so the hash doesn't move.
    #[serde(default, skip_serializing_if = "CandidateKind::is_drop")]
    pub kind: CandidateKind,
    /// STAGE-2 hit rate in parts per million — the odds the SERVER pays this ticket out.
    ///
    /// Carried here because the server resolves a candidate knowing only the candidate: it
    /// has no idea which monster died, and the rate depends on that monster's mix of
    /// server-roll entries (`drops::stage2_hit_ppm`). Client-supplied in name only — the
    /// server recomputes the whole state by replay, so a forged value fails the hash.
    ///
    /// Defaults to 1,000 ppm (0.1%), the flat rate every candidate used before 0.0.256, so
    /// tickets already pending in an old save resolve on the odds they were accrued under.
    #[serde(default = "default_hit_ppm")]
    pub hit_ppm: u32,
}

fn default_hit_ppm() -> u32 {
    1_000
}

/// One damage application this tick, for the RENDERER's multi-hit popups (RO
/// "N hits + a running yellow total"). Each resolve CALL emits ONE event: a
/// normal hit has `hits = 1`; Double Attack sets `hits = 2`; Sonic Blow sets
/// `hits = 8`. The off-hand swing emits its OWN event, so dual-wield shows two
/// separate popups. `amount` is the TOTAL dealt (the renderer splits it across
/// `hits` numbers). Display-only — `#[serde(skip)]`, cleared every tick.
/// What a hit event represents, for the renderer's floating text. `Normal` = a
/// damage popup (number from the hp-delta / multi-hit split). `Dodge` = the attack
/// MISSED (Flee beat Hit) → show "Dodged". `Guard` = a shield fully absorbed it
/// → show "Guarded". Display-only (HitEvent is serde-skip).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HitKind {
    #[default]
    Normal,
    Dodge,
    Guard,
    /// The target's Toughness gauge broke on this hit. Display + measurement
    /// only — the anomaly the break will eventually apply is not built yet.
    Break,
    /// The blow crit (0.0.472): the renderer labels its number. Takes the place of `Normal` on
    /// the swing's own event; a break is still its separate event.
    Crit,
}

/// Free function so a call site holding `&mut Entity` borrowed out of `state.grid` can still
/// record — the borrow ends before this runs, which a method on `GameState` could not promise.
pub(crate) fn push_heal(
    state: &mut GameState,
    target_id: u32,
    healer_id: u32,
    amount: i32,
    kind: HealKind,
) {
    state.note_heal(target_id, healer_id, amount, kind);
}

/// Where a heal came from. The renderer picks its FX from this (a cast sparkle is not a potion
/// glow), and `healing_done` counts only what the party actually put out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HealKind {
    /// A skill heal — Heal / Dupla / Subvenio / Benediction / card procs.
    #[default]
    Skill,
    /// Passive HP regeneration.
    Regen,
    /// A heal-over-time buff tick.
    Hot,
    /// A consumable.
    Potion,
    /// Life drain off damage dealt.
    Lifesteal,
}

/// A heal that landed THIS tick.
///
/// `amount` is the EFFECTIVE restore — what the HP bar actually gained after the max-HP clamp,
/// not what was rolled. A full-HP target reading "+500" while nothing moved is the same class of
/// lie as filing an overkill crit at the victim's leftover HP.
///
/// `healer_id` equals `target_id` for regen, HoT and lifesteal, which are self-inflicted by
/// nature; it is the caster for a skill heal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HealEvent {
    pub target_id: u32,
    pub healer_id: u32,
    pub amount: i32,
    pub kind: HealKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitEvent {
    pub target_id: u32,
    pub attacker_id: u32,
    pub amount: i32,
    pub hits: u32,
    pub kind: HitKind,
}

/// A targeted SKILL activation THIS tick (melee/ranged cast), for the renderer's
/// per-skill VFX — e.g. Bash plays an impact burst instead of the basic weapon
/// slash. `skill_id` lets the renderer pick the effect; the generic weapon slash
/// suppresses itself for the same (caster, target) so the two don't stack.
/// Display-only — `#[serde(skip)]`, cleared every tick like `recent_hits`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillFx {
    pub caster_id: u32,
    pub target_id: u32,
    pub skill_id: String,
}

/// A radial burst FX that fired THIS tick, for the renderer's shockwave: `(x, y)`
/// = the cell it centred on, `radius` = blast Chebyshev radius (cells), `element`
/// = the colour the renderer tints the ring (Poison-green, Holy-gold, …). Shared
/// by poison explosions, Lumen Contagio, and (geometry only) meteor impacts.
/// Display-only — `#[serde(skip)]`, cleared every tick like recent_hits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BurstFx {
    pub x: i32,
    pub y: i32,
    pub radius: i32,
    pub element: crate::grid::Element,
}

/// A SCHEDULED bomb-monster explosion (see `GameState.pending_explosions`). Captured at
/// the bomb's death from its on-death blast skill (`self_destruct` / `toxic_burst`); on
/// `fire_tick` it deals `dmg` + a `push`-cell knockback to both teams within `radius`
/// around `(x, y)` — plus `poison_ticks` of classic Poison to survivors when > 0 — so the
/// blast lands in sync with the explosion animation rather than the instant of death.
/// `src_id` credits the damage popup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingExplosion {
    pub x: i32,
    pub y: i32,
    pub dmg: i32,
    pub push: i32,
    pub radius: i32,
    /// Classic-Poison duration applied to living survivors (0 = no poison).
    pub poison_ticks: u32,
    /// > 0 flips the blast to an ALLY-MONSTER HEAL of this many HP (no damage /
    /// knockback / poison) — Rechum's Mending Burst. 0 = a normal damaging blast.
    pub heal: i32,
    pub src_id: u32,
    pub fire_tick: u64,
    /// > 0 = a Lumen Contagio explosion (Lucielle): `dmg` is a pre-scaled MATK amount
    /// dealt as a MAGIC HOLY 5×5 blast to ENEMIES, and every un-marked enemy caught is
    /// re-marked with `LumenVuln` at this level (contagion). 0 = a normal flat bomb.
    #[serde(default, skip_serializing_if = "crate::items::is_zero_u8")]
    pub lumen_level: u8,
}

/// A dormant Ranger TRAP sitting on a grid cell. Placed on the 5×5 perimeter
/// around the archer (one PRNG draw, gated on owning a trap skill → goldens
/// byte-identical), it arms until an enemy enters its 3×3, then fires ONCE and is
/// removed. Persists until a zone change. NOT an entity (no HP / turn / blocking).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Trap {
    pub cell: Cell,
    pub kind: TrapKind,
    pub level: u8,
    pub owner_id: u32,
    /// Claymore only: ticks left on the fuse after it triggered (5 → 0 = boom).
    /// 0 for an un-triggered trap and for the instant kinds (Land Mine / Freezing).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub fuse: u32,
    /// Claymore only: armed → triggered (now counting the fuse down). The instant
    /// kinds fire and are removed in one tick, so they never set this.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub triggered: bool,
    /// Enemy ids currently inside the 3×3 as of the last check — for EDGE-triggering.
    /// A trap fires only when a NEW enemy id enters (one not in this set), so a trap
    /// placed UNDER an enemy that's already standing there won't go off; it waits for
    /// someone to step in. Seeded at placement with whoever's already inside.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seen_ids: Vec<u32>,
}

/// A trap that FIRED this tick, for the renderer's snap/blast FX: the cell it sat
/// on + which kind. Display-only — `#[serde(skip)]`, cleared every tick like
/// `recent_hits` (so a Land Mine that vanishes the same tick still gets an anim).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrapFire {
    pub x: i32,
    pub y: i32,
    pub kind: TrapKind,
}

/// Which Ranger trap this is. Append-only.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TrapKind {
    /// Land Mine — instant single-target `250% + 50%/lvl` ATK to the triggerer.
    LandMine,
    /// Claymore Trap — a 5-tick fuse, then a 3×3 blast of `150% + 30%/lvl` ATK.
    Claymore,
    /// Freezing Trap — no damage; FREEZES the whole 3×3 for `lvl+2` seconds.
    Freezing,
}

/// What a `GroundField` does on each interval. Append-only (more land with the
/// Elementalist: Quicksand slow, Meteor damage, …).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GroundFieldKind {
    /// Applies classic `Poison` (for `poison_ticks`) to every victim inside.
    PoisonCloud { poison_ticks: u32 },
    /// Elementalist damaging field (Glacial Storm / Vermilion Field / Quicksand):
    /// each interval, deals `amount` (a SNAPSHOT of the caster's MATK·power at cast,
    /// element-adjusted) to every victim inside, and rolls `status` at `status_bp`
    /// (Freeze / Blind / AgiSlowAdd). `amount` 0 = pure debuff field (Quicksand).
    Damage {
        amount: i32,
        element: crate::grid::Element,
        status: Option<crate::skills::BuffKind>,
        status_bp: u32,
        status_dur: u32,
        status_mag: i32,
        /// Ticks between hits (Vermilion = 2 → a hit every 0.2 s). The field fires
        /// when `ticks_left % hit_interval == 0`. >= 1.
        hit_interval: u32,
        /// GUARANTEED status every Nth hit (Glacial = 3). 0 = random via status_bp.
        status_at: u32,
        /// Cells to shove each (non-frozen) victim in a RANDOM direction per hit
        /// (Glacial Storm = 1 — Storm Gust scatter). 0 = no knockback.
        knockback: i32,
        /// METEOR mode (Meteor Rain). 0 = a normal whole-field hit (Vermilion /
        /// Glacial hit everyone within `radius`). > 0 = each interval one METEOR
        /// strikes a RANDOM cell within `radius` (the 7×7 target area) and damages
        /// only those within `impact_radius` of THAT cell (5×5) — so the random
        /// drop can MISS. `radius` is the target area; this is the per-meteor blast.
        impact_radius: i32,
        /// VISUAL hits per strike — how many numbers the renderer splits `amount`
        /// into (`amount` already bundles them, DEF/MDEF once). Meteor Lv10 = 5,
        /// Lord of Vermilion = 20. 0/1 = a single number.
        strike_hits: u32,
        /// BLACK HOLE (0.0.451, Abyssal Embrace): each hit interval every victim inside
        /// `radius + pull` is hauled `pull` cells toward the centre and ROOTED for the interval,
        /// so they arrive and stay; with `fuse` the burst lands when the hole closes. 0 = a
        /// normal field. Skip-if-0 keeps every existing field byte-identical.
        #[serde(default, skip_serializing_if = "is_zero_i32")]
        pull: i32,
        /// FUSE (a planted bomb — Dulahaness's Pyre Mark): the field deals NOTHING
        /// while it lives and detonates ONCE on its final tick. A normal field is the
        /// opposite — it burns from the moment it lands. The fuse is what lets a mark
        /// be a WARNING the party can answer (walk clear, or lift the marked member
        /// off the board) instead of damage that has already happened. Skip-if-false
        /// so every existing field serializes byte-identically.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        fuse: bool,
        /// ONCE-PER-TARGET (Lord of Vermilion): each victim takes its instance only
        /// once for the field's life (tracked in `GroundField.hit_ids`), but a NEW
        /// victim entering the AoE still gets hit. false = a normal repeating field.
        once_per_target: bool,
    },
}

fn is_zero_u64_state(v: &u64) -> bool {
    *v == 0
}

/// Engram denominations in XP, smallest first. Fixed and few on purpose: a refund breaks a unit's
/// XP into these greedily (largest first) and the sub-1,000 remainder is a tiny, acknowledged sink.
pub const ENGRAM_TIERS: [u64; 4] = [1_000, 10_000, 100_000, 1_000_000];

fn engrams_empty(e: &[u32; ENGRAM_TIERS.len()]) -> bool {
    e.iter().all(|&n| n == 0)
}

fn is_zero_u32_state(v: &u32) -> bool {
    *v == 0
}

fn is_default_run_tally(t: &RunTally) -> bool {
    *t == RunTally::default()
}

fn is_physical_element(e: &crate::grid::Element) -> bool {
    *e == crate::grid::Element::Physical
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn default_deploy_grace() -> u16 { crate::tick::DEPLOY_GRACE_TICKS_MARCH }

/// Per-zone spawn schedule, lives inside GameState while a zone is loaded.
/// Cleared when a new zone loads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ZoneSpawnState {
    /// Weighted pool of monster templates the zone draws from.
    pub pool: Vec<MonsterSpawnEntry>,
    /// How many alive monsters the zone tries to maintain.
    pub target_population: u32,
    /// Sim ticks between spawn attempts (when below target).
    pub spawn_interval_ticks: u32,
    /// The sim tick this zone opened. The origin for the OPENING WINDOW, during which monsters
    /// enter from the right only (see `spawn::OPENING_WINDOW_TICKS`) so the party — which spawns
    /// on the left — meets them in the middle instead of being surrounded from tick 0.
    ///
    /// Serialized: a save taken inside the window must resume inside it, or reloading would move
    /// where the next monster appears and the replay would diverge.
    #[serde(default)]
    pub opened_at_tick: u64,
    /// How long combat is held after this zone opened, in ticks. It varies BY WHAT THE OPEN HAS TO
    /// COVER, not by the zone: a room-to-room advance is a 52 m walk and needs the long window
    /// (`tick::DEPLOY_GRACE_TICKS_MARCH`), while a first room, a band swap or a retry only has an
    /// entrance from just off the left edge (`DEPLOY_GRACE_TICKS_OPEN`).
    ///
    /// Serialized for the same reason as `opened_at_tick`: a save taken inside the window must
    /// resume inside the SAME window, or the replay diverges. Old saves default to the long one,
    /// which is what they were recorded under.
    #[serde(default = "default_deploy_grace")]
    pub deploy_grace_ticks: u16,
    /// The next absolute sim tick at which a spawn attempt is allowed.
    /// Set to `0` at zone load so the first attempt fires immediately
    /// (the initial population fill).
    pub next_spawn_at_tick: u64,
    /// Regular (non-boss) monster kills accumulated in this zone.
    pub kill_counter: u32,
    /// Once `kill_counter >= boss_threshold`, the boss spawns and
    /// continuous mob spawn keeps going alongside (user choice — the
    /// boss is a "rush" moment, not a 1v1 arena).
    pub boss_threshold: u32,
    /// True once the boss has appeared in the arena. Latched — the
    /// boss can only be spawned once per zone load.
    pub boss_spawned: bool,
    /// Pre-resolved boss template; cloned at spawn time. `None` means
    /// the zone has no boss (e.g. tutorial zone or test fixture).
    pub boss_template: Option<MonsterTemplate>,
    /// When true, the boss is a RANDOM eligible pool monster wearing `boss_template`'s stats
    /// (gothic bands 5-10). Skip-if-false so every existing (fixed-boss) zone serializes
    /// byte-identically. See `try_spawn_boss`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub boss_from_pool: bool,
    /// Copied from `ZoneDef`: the TOTAL number of monsters this zone will ever trickle in, or 0
    /// for the endless farm behaviour every band zone has. A story stage is a designed encounter —
    /// ten pinklings and a boss, not a tap you turn off by hitting a kill quota — and there was no
    /// way to say that. Skip-if-zero so every existing zone serializes byte-identically.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub max_spawns: u32,
    /// True the tick the boss dies. Drives `pending_zone_change`.
    pub zone_cleared: bool,
    /// The tick the zone was cleared (boss died). The zone change is HELD for
    /// `ZONE_CLEAR_DELAY_TICKS` after this so the boss's death animation can
    /// finish on the client before the room swaps. `None` until cleared; skipped
    /// when absent so uncleared zones stay byte-identical in the state hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleared_at_tick: Option<u64>,
    /// Latches once this instance-end clear has been counted into `band_clears`,
    /// so the per-tick clear check can't tally the same run twice. Reset with the
    /// fresh `ZoneSpawnState` on the next zone load.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub band_clear_counted: bool,
    /// Where to advance to when this zone is cleared. `None` = no
    /// next zone (terminal zone — sim just loops the cleared arena).
    pub next_zone_id: Option<String>,
    /// Copied from `ZoneDef`: this is the instance's FINAL room (big boss) —
    /// clearing it banks the escrow + triggers advance-or-farm.
    #[serde(default)]
    pub is_instance_end: bool,
    /// Copied from `ZoneDef`: room 1 of this instance (farm-loop + wipe-restart).
    #[serde(default)]
    pub instance_start_zone_id: String,
    /// Copied from `ZoneDef`: PLAYER skills this room enables (`regent_hand`).
    /// Skip-if-empty so every ordinary zone (all goldens) serializes unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub player_skills: Vec<String>,
}

/// One row of the spawn pool: a pre-resolved monster template + its
/// relative weight. Pre-resolving the template means the sim never has
/// to look up `GameData` again during gameplay — only the catalog
/// load path touches it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonsterSpawnEntry {
    pub template: MonsterTemplate,
    pub weight: u32,
}

/// Spawn-ready snapshot of a monster definition. Carries everything
/// needed to instantiate an `Entity` with no further data lookups.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonsterTemplate {
    /// `MonsterDef.id` — kept so logs / debug can name the monster.
    pub def_id: String,
    /// Bestiary group key (same-model family, `MonsterDef.bestiary_group`).
    /// Empty = the def tallies under its own id. Skip-if-empty: pre-0.0.184
    /// saves load unchanged.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bestiary_group: String,
    pub hp: i32,
    pub atk: i32,
    pub def: i32,
    /// Physical ATK damage RANGE (RO atk1/atk2); 0/0 = flat `atk`.
    #[serde(default)]
    pub atk_min: i32,
    #[serde(default)]
    pub atk_max: i32,
    /// Skill ids (active + passive) this monster carries — instantiated at level 1
    /// on spawn (see `spawn::build_and_spawn_monster`). Drives on-death blasts
    /// (`self_destruct` / `toxic_burst`) and any future monster ability. Empty by
    /// default → skipped so non-skilled templates serialize byte-identically.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    pub element: Element,
    /// Elements that chip this monster's gauge (`MonsterDef.weaknesses`). Empty =
    /// the element matrix decides, which is what a pre-0.0.326 catalog gets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weaknesses: Vec<Element>,
    /// Elements this monster resists (80%). Opt-in; empty for a plain monster.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resists: Vec<Element>,
    /// TOUGHNESS gauge size, copied from the def. 0 = no gauge, and skipped so a
    /// template without one serializes exactly as it did before the gauge existed.
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub toughness_max: u16,
    pub speed_milli: u32,
    pub atk_speed_milli: u32,
    pub atk_range: i32,
    pub drops: Vec<DropEntry>,
    /// GEAR GRADE odds for THIS monster's band, resolved from `loot_rules.gear.grade_odds` at
    /// build time and baked in — `tick()` has no catalog in hand, so anything the drop path needs
    /// has to travel with the monster (the same reason `drops` is a copy, not a lookup).
    /// Empty = the code default (`drops::GRADE_ODDS_BP`); skip-if-empty keeps old saves and every
    /// golden byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gear_grade_odds: Vec<u32>,
    pub proj_travel_ticks: u32,
    pub xp_reward: u32,
    /// Gil (currency) awarded to the party on death.
    pub gil_reward: u32,
    /// Minions to spawn around this monster when it appears as a boss (0 = none).
    pub boss_minions: u32,
    /// RO race/size (gear-mod conditional axes). Serde-default so templates in
    /// old saves load unchanged; skip-if-default so raceless templates (and
    /// every existing golden) stay byte-identical.
    #[serde(default, skip_serializing_if = "is_unknown_race")]
    pub race: crate::grid::Race,
    #[serde(default, skip_serializing_if = "is_medium_size")]
    pub size: crate::grid::Size,
    /// The STAGE this monster was built on (0.0.399; was its level) — the sigil tier and the
    /// grade-odds fallback read it. 0 = unplaced. Skip-if-0 keeps hand-built templates clean.
    #[serde(default, alias = "level", skip_serializing_if = "is_zero_u16")]
    pub stage: u16,
}

fn is_zero_u16(v: &u16) -> bool {
    *v == 0
}

#[cfg(test)]
mod bestiary_milestone_tests {
    use super::BESTIARY_MILESTONES;

    /// `bestiary_claimed` stores one BIT per milestone in a `u8`, and the claim handler does
    /// `1u8 << idx` after bounds-checking `idx` against `BESTIARY_MILESTONES.len()`. Those are two
    /// numbers in two files with nothing binding them: the table can grow, the `u8` cannot.
    ///
    /// The 9th milestone is where it breaks, and it breaks in the worst available way. `1u8 << 8`
    /// PANICS in a debug build and is masked in release — so the two builds would disagree about
    /// what a save contains, which is a determinism divergence, and `apply_input` is required
    /// never to panic at all (a replay has to survive version skew, GDD §4.4). Neither symptom
    /// points at the milestone table.
    ///
    /// Four today, so there is room for four more and this test costs nothing until the day it
    /// matters. Widening `bestiary_claimed` past `u8` is a serialized-shape change: bump
    /// `sim_core::VERSION`.
    #[test]
    fn the_milestone_table_still_fits_the_claimed_bitmask() {
        let bits = u8::BITS as usize;
        assert!(
            BESTIARY_MILESTONES.len() <= bits,
            "BESTIARY_MILESTONES has {} entries but `bestiary_claimed` is a u8 ({bits} bits). \
             `1u8 << idx` in ClaimBestiaryReward would panic in debug and wrap in release — the \
             two builds would disagree. Widen the field (VERSION bump) or drop a milestone.",
            BESTIARY_MILESTONES.len(),
        );
        // The thresholds must also ASCEND, or a later milestone is reachable before an earlier one
        // and the bits stop meaning "how far you have come".
        for w in BESTIARY_MILESTONES.windows(2) {
            assert!(w[0].0 < w[1].0, "milestone kill thresholds must ascend: {:?}", BESTIARY_MILESTONES);
        }
    }
}

#[cfg(test)]
mod earn_gil_tests {
    use super::{GameState, GilEarn};

    #[test]
    fn no_gilpct_node_adds_only_base() {
        let mut s = GameState::new(1, 8, 8);
        s.earn_gil(7, GilEarn::NpcSale);
        assert_eq!(s.gil, 7);
        assert_eq!(s.gil_millis, 0, "no bonus tracked without a GilPct node");
    }

    #[test]
    fn tiny_sales_accrue_their_percent_via_millis() {
        let mut s = GameState::new(1, 8, 8);
        s.soul_paths.insert("fg1".to_string(), 1); // Greed = GilPct 3%
        // Four 10-Gil sales: base 40, bonus 40·3% = 1.2 Gil → 1 whole + 0.2 carried.
        for _ in 0..4 {
            s.earn_gil(10, GilEarn::NpcSale);
        }
        assert_eq!(s.gil, 41, "40 base + 1 whole bonus Gil");
        assert_eq!(s.gil_millis, 200, "0.2 Gil carried as 200 millis");
    }

    #[test]
    fn cheap_item_eventually_pays_the_bonus() {
        let mut s = GameState::new(1, 8, 8);
        s.soul_paths.insert("fg1".to_string(), 1); // 3%
        // A 2-Gil item alone rounds to 0 bonus, but 100 of them accrue 6 Gil.
        for _ in 0..100 {
            s.earn_gil(2, GilEarn::NpcSale);
        }
        assert_eq!(s.gil, 206, "200 base + 6 bonus (2·100·3% = 6)");
    }
}


fn is_unknown_race(r: &crate::grid::Race) -> bool {
    matches!(r, crate::grid::Race::Unknown)
}
fn is_medium_size(z: &crate::grid::Size) -> bool {
    matches!(z, crate::grid::Size::Medium)
}
