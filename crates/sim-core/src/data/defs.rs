//! Content-catalog types — the `GameData` struct, its `*Def` rows, and
//! the validation/lookup `impl`. Split out of data.rs (0.0.34 refactor).

// DEBT (2 sites): this file predates `deny(clippy::indexing_slicing)`. An index that
// cannot be proven in range is a panic, and in verify-server (which links this crate and
// replays saves under `panic = "abort"`) a panic is the whole process. Shrink this, never
// add to it — a NEW file has no allow and is rejected on its first bare index.
#![allow(clippy::indexing_slicing)]

use serde::{Deserialize, Serialize};

/// The standard Toughness gauge now lives in `stage_base.toughness` (0.0.399); the formula it
/// was generated from: `20 + L/2 + L²/120` with L the stage's old level.

/// A boss's gauge as a percent of its level's standard one, unless its def names its
/// own. A boss is one long fight rather than a stream of short ones, so its bar is
/// sized to the fight rather than to the creature.
pub const BOSS_TOUGHNESS_PCT: u16 = 300;
use std::collections::BTreeSet;

use crate::grid::{Cell, DamageType, Element};
use crate::items::{EquipSlot, ItemKind, Rarity, WeaponType};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameData {
    /// Free-form version string. Saves stamp this so a save made
    /// against `1.0` can refuse to load against incompatible `2.0`
    /// (GDD §4.4 migration policy).
    pub version: String,
    pub items: Vec<ItemDef>,
    pub monsters: Vec<MonsterDef>,
    pub zones: Vec<ZoneDef>,
    /// Monster strength by LEVEL — the base a zone's monsters are built from, shaped per monster by
    /// its `*_mult_bp`. Replaces the per-zone `hp_scale_bp` / `scale_from_base` pair, under which
    /// the same creature was a different creature depending on where it stood and the difficulty
    /// curve lived in three places at once (two bands went BACKWARDS unnoticed).
    ///
    /// A TABLE, not a formula: one geometric rate misses this game's midgame by 87%, because the
    /// real curve is steep early and flat late.
    ///
    /// The difficulty ladder BY STAGE (0.0.398): hp / atk / xp of an ordinary monster on each of
    /// the 240 stages, a smooth geometric ramp anchored at stage 1, the launch cap (168 = Lv70)
    /// and the top (240 = Lv100). Replaces the per-LEVEL table, whose cliffs (×1.4 at Lv10→11)
    /// were artefacts of deriving it from the 20-band catalog. A monster's LEVEL is derived from
    /// its stage (`state::level_of_stage`) and still keys DEF, gear tier and grade odds.
    #[serde(default, alias = "level_base")]
    pub stage_base: Vec<StageBase>,
    /// LOOT RULES — drop rows every monster gets by RULE instead of by hand (0.0.364): the Star
    /// Stones by level tier and field/boss, the Boss Core / Track capstones on bosses. Generated
    /// into the drop table at spawn (`build_drop_table`), as `server_bp` rows the server-roll
    /// pipeline sees like any other. One place to tune; nothing copied onto 67 monsters.
    #[serde(default)]
    pub loot_rules: LootRules,
    /// TRAINING (0.0.406) — the party-wide Gil ladders (`training.rs`): one cost curve, a slot
    /// per stat. Empty = the system is absent (no slots to buy), so a catalog without the block
    /// loads and plays as before.
    #[serde(default)]
    pub training: TrainingRules,
    /// PARTY SLOTS (0.0.413): the Gil price of each party slot past the first — `slot_gil[0]`
    /// opens the 2nd, up to `MAX_PARTY`. Cheap by design (user: "ไม่ต้องแพงมาก max 5 ครบได้ใน
    /// 1-2 hr"); the characters themselves come from gacha only.
    #[serde(default)]
    pub party: PartyRules,
    /// CRESTS (0.0.416, `crest.rs`): disenchant gear or a gem → a BOUND material keyed by the
    /// item's category and tier → level a crest of that category. Empty = absent.
    #[serde(default)]
    pub crests: CrestRules,
    /// Per-(def × grade) exceptions to the grade formula. See `GearOverride`.
    #[serde(default)]
    pub gear_overrides: Vec<GearOverride>,
    /// Player class definitions (Phase 0: a single fixed class per
    /// MVP, but the shape supports multiple). GDD §8 open question
    /// "single fixed class vs weapon-defines-style" still applies; the
    /// data shape doesn't lock us in either direction.
    pub units: Vec<UnitDef>,
    /// Crafting recipes (materials + Gil → an output item). `#[serde(default)]` so
    /// pre-crafting catalogs still parse. See `crate::craft`.
    #[serde(default)]
    pub recipes: Vec<CraftRecipeDef>,
    /// Feature switches (0.0.490). Absent = every feature on, so an older catalog plays as before.
    #[serde(default)]
    pub features: Features,
}

/// Whole systems the catalog can hold back from a release without deleting their data.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Features {
    /// CARDS (user 2026-09-29: "card จะไม่ ship ใน patch แรก เพราะ complicate (แต่ใส่ช่องการ์ดไว้ใน
    /// ไอเทมก่อน)"). Off: no card drop row enters a drop table, the shop neither lists nor sells a
    /// card, and the host tells the client to hide the card screens. Pieces keep their card
    /// sockets; the 19 card defs stay authored for the patch that turns this on.
    #[serde(default = "feature_on")]
    pub cards: bool,
    /// CRAFT (user 2026-10-02: "Craft ตัดออกก่อน"). Off: `LearnRecipe` / `CraftItem` are no-ops,
    /// the shop does not sell a material whose only use is a recipe ingredient, and the host tells
    /// the client to hide the craft screens. Recipes stay authored for a later patch.
    #[serde(default = "feature_on")]
    pub craft: bool,
}

fn feature_on() -> bool {
    true
}

impl Default for Features {
    fn default() -> Self {
        Self { cards: true, craft: true }
    }
}

/// One crafting recipe: consume the `ingredients` (stackable materials) + `gil_cost`,
/// produce `output_qty` of `output_def_id`. Deterministic — always succeeds if the player
/// holds the inputs (no PRNG draw; goldens unaffected). Batch output (e.g. ×10) suits an
/// idle game that auto-consumes potions fast.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CraftRecipeDef {
    pub id: String,
    /// Display name; empty → UI falls back to `id`.
    #[serde(default)]
    pub name: String,
    pub output_def_id: String,
    #[serde(default = "default_output_qty")]
    pub output_qty: u32,
    #[serde(default)]
    pub gil_cost: u64,
    /// One-time LEARN price (soul + Gil) paid via `LearnRecipe` before the recipe can be
    /// crafted. Soul is the real gate (knowledge specialization — see
    /// docs/craft-and-offline-design.md); every recipe must be learned, even at cost 0.
    #[serde(default)]
    pub learn_soul: u64,
    #[serde(default)]
    pub learn_gil: u64,
    /// Which bench the recipe belongs to. Grouping only — the sim never reads it, so a
    /// mis-tagged recipe is a UI placement bug, never a gameplay one.
    #[serde(default)]
    pub station: CraftStation,
    pub ingredients: Vec<RecipeIngredient>,
}

/// The bench a recipe is crafted at. Exists so potions are not brewed at the anvil; it
/// partitions the craft screen and nothing else. Untagged data lands on `Blacksmith`,
/// the larger set, so adding a gear recipe without the field still shows up somewhere.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CraftStation {
    #[default]
    Blacksmith,
    Apothecary,
    /// Food. Separate from the apothecary because a Meal and a Tonic occupy DIFFERENT reserved
    /// slots and are farmed from different materials — sharing a bench would put two lines that
    /// never compete on the same screen and hide that they are independent.
    Kitchen,
}

/// A recipe input: `qty` units of a stackable material `def_id`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecipeIngredient {
    pub def_id: String,
    pub qty: u32,
}

fn default_output_qty() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ItemDef {
    pub id: String,
    /// PERMANENT numeric identity (0.0.376). Assigned once, in catalog order, and then
    /// APPEND-ONLY: a number is never reused and never moved, even if its item is deleted.
    ///
    /// It exists for one consumer that does not exist yet — the Steam item-definition registry,
    /// whose `itemdefid` is an integer that must stay stable for the life of every item ever
    /// minted (`itemdefid = num × 16 + grade_step`, design §7.6). Allocating it NOW, while
    /// nothing depends on it, is free; allocating it on the day of the first mint means freezing
    /// whatever order this file happens to be in after another year of edits.
    /// `item_nums_are_unique_and_pinned` guards both halves.
    #[serde(default)]
    pub num: u32,
    pub kind: ItemKind,
    /// Human-facing display name. Empty → UI falls back to `id`. See `Item::name`.
    #[serde(default)]
    pub name: String,
    pub atk_bonus: i32,
    /// Attack-speed line in bp (0 = none). See `Item::atk_speed_bp`.
    #[serde(default)]
    pub atk_speed_bp: i32,
    /// Weapon class for weapon-type-gated effects. Default None.
    #[serde(default)]
    pub weapon_type: WeaponType,
    /// Off-hand kind (0.0.446): Shield / Tome / Quiver / HiddenKnife on the pieces that sit in
    /// the off-hand slot; None (absent) on everything else. See `items::OffhandType`.
    #[serde(default)]
    pub offhand_type: crate::items::OffhandType,
    /// Weapon LEVEL (RO tier 1-4) — scales refine ATK (Lv1 +2 .. Lv4 +7).
    /// Default 1 (non-weapons / unset).
    #[serde(default = "default_weapon_level")]
    pub weapon_level: u8,
    pub def_bonus: i32,
    /// Absent on gear (0.0.380): a Weapon / Armor def has no rarity — its GRADE is the one axis
    /// the player sees, and its card sockets are the def's own fixed count. Cards, Exemplars,
    /// Regalia and materials still carry one.
    #[serde(default)]
    pub rarity: Rarity,
    /// Gil paid per unit when sold. Set on materials/junk (the Gil source);
    /// 0 (default) for gear. See `Item::sell_value`.
    #[serde(default)]
    pub sell_value: u32,
    /// Gil price to BUY this from the shop. 0 (default) = not sold in the shop.
    /// Items with `buy_price > 0` form the shop catalog (the Gil sink + the
    /// supply for consumables).
    #[serde(default)]
    pub buy_price: u32,
    /// Auto-use spec when `kind == Consumable` (None for gear/junk). See
    /// `Item::consumable`.
    #[serde(default)]
    pub consumable: Option<crate::items::ConsumableSpec>,
    /// Phase 2 economy reserved field (GDD §3.3). True = listable on
    /// the Steam Market when that ships.
    pub marketable: bool,
    /// The Steam Inventory itemdef this item mints as (0.0.478) — its permanent identity on Steam.
    /// Only an item that has one can be sent to Steam (`InputEvent::SendToSteam`); None for
    /// everything else, whatever `marketable` says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_itemdef: Option<u32>,
    /// A Farm Banner box (0.0.479): `InputEvent::OpenFarmBox` accepts it and counts it per id in
    /// `GameState.farm_opened`; the server turns each open into a bound ticket for the event that
    /// box belongs to. Every event has its own box def.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub farm_box: bool,
    pub emission_group: String,
    pub quota_class: String,
    /// Paper-doll wear position (RO's 10). Omit for materials and for plain
    /// weapon/body-armor that can rely on the `kind`-derived fallback; set it
    /// for shields/headgear/garment/shoes/accessories that share `ItemKind`
    /// but occupy distinct slots. See `Item::equip_slot`.
    #[serde(default)]
    pub equip_slot: Option<EquipSlot>,
    /// Card sockets on this gear (RO "slotted variant"). 0 = none. Weapons cap
    /// this by weapon level in `Item::socket_count`.
    #[serde(default)]
    pub card_slots: u8,
    /// Which card PANEL class this Card equips into — Card-kind defs only.
    #[serde(default)]
    pub card_slot: Option<crate::items::CardClass>,
    /// The mods this card grants while ACTIVE in the panel at 0★ — Card-kind defs only.
    #[serde(default)]
    pub card_mods: Vec<crate::items::StatMod>,
    /// Mods added once PER STAR (× star) — the headline number's ladder.
    #[serde(default)]
    pub card_star_step: Vec<crate::items::StatMod>,
    /// Irregular star bumps (mods that join only at specific ranks, cumulative).
    #[serde(default)]
    pub card_star_ranks: Vec<crate::items::CardStarRank>,
    /// FOIL-variant EXTRA mods (per card, not a fixed ×2). Empty ⇒ foil adds nothing.
    #[serde(default)]
    pub foil_mods: Vec<crate::items::StatMod>,
    /// Per-star ladder for the FOIL extra.
    #[serde(default)]
    pub foil_star_step: Vec<crate::items::StatMod>,
    /// Card ELEMENT (drives the element-coloured frame). Default Neutral.
    #[serde(default)]
    pub element: crate::grid::Element,
    /// Small PERMANENT mods granted when BOUND — set only on Card-kind defs.
    #[serde(default)]
    pub bind_mods: Vec<crate::items::StatMod>,
    /// FOIL variant (the special ~1/20 card). Default false. Deferred mechanics.
    #[serde(default)]
    pub foil: bool,
    /// UNIQUE gear: authored fixed-identity item — stars never roll at drop.
    /// Its own axis, separate from `rarity` (Legendary is a reachable 4★ drop
    /// tier; unique-ness is not a rarity). See `Item::unique`.
    #[serde(default)]
    pub unique: bool,
    /// STAR STONE effect — set only on orb defs. See `Item::orb`.
    #[serde(default)]
    pub orb: Option<crate::items::OrbEffect>,
    /// Lives in the MATERIALS LEDGER (`GameState.materials`, def id → count), never in the bag:
    /// no slot, no cap, never bound. Sigil / Boss Core / Track and the Star Stones (0.0.364,
    /// user: "star stone ก็ดรอปเข้าแบบ sigil เลยจะได้ไม่รก"). `kind` says what a thing IS; this says
    /// where it is kept. Stamped onto the `Item` at instantiate so the bank path can route it
    /// without a catalog in hand.
    #[serde(default)]
    pub ledger: bool,
    /// A CREST MATERIAL envelope (2026-10-02, the tutorial's Crest quest): the `crest_mats` key it
    /// credits (`weapon_t3` …), one material (`shards_per_material` shards) per unit. Never reaches
    /// the bag — `bank_collectible` turns it into the counter on arrival, so only a server mail
    /// can deliver one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crest_material: Option<String>,
    /// An EXP envelope (2026-10-02, offline progression): on arrival `quantity` EXP goes to every
    /// fielded member (`tick::grant_party_xp`) and nothing lands in the bag. Server mail only.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub xp_envelope: bool,
    /// Tradeability (S3 economy) — TWO INDEPENDENT flags → 4 states:
    /// (F,F)=Bound · (T,F)=Gil-market-only · (F,T)=Steam-only (premium/real-money
    /// asset) · (T,T)=both. Def-level catalog metadata (GameData, NOT GameState) →
    /// zero hash/save impact. The server gates /register on `gil_tradeable` and
    /// /ship mint on `steam_tradeable`; a modified INSTANCE (refine/card/affix) is
    /// demoted off Steam at trade-time (clean-item rule), NOT stored here. Default
    /// Bound — tradeability is opt-in per def. Supersedes the legacy `marketable` /
    /// `quota_class` reserved fields (to be removed in a later cleanup).
    #[serde(default)]
    pub gil_tradeable: bool,
    #[serde(default)]
    pub steam_tradeable: bool,
    /// A GEM def (0.0.386): which set it is and which grade — `gem_<set>_g<step>`, one def per
    /// (set × grade) so the ledger, the commodity market and Steam's registry each see a plain
    /// product. Lives in the ledger (`ledger: true`).
    #[serde(default)]
    pub gem: Option<GemSpec>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GemSpec {
    pub set: String,
    pub grade: crate::items::Grade,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonsterDef {
    pub id: String,
    pub hp: i32,
    pub atk: i32,
    pub def: i32,
    /// RO Hit / Flee / hard-MDEF. Default 0 (old data loads; combat then derives
    /// Physical ATK damage RANGE (RO atk1/atk2). A hit rolls ATK uniformly in
    /// `[atk_min, atk_max]`. 0/0 (or max ≤ min) = flat `atk` (no range, no draw).
    /// Preliminary: seeded at ±10% of `atk`; tune per-monster later.
    #[serde(default)]
    pub atk_min: i32,
    #[serde(default)]
    pub atk_max: i32,
    /// The elements this monster is WEAK to — 1-3 of them, averaging 2
    /// (`docs/element-weakness-system.md` §2). Only an attack of a listed element
    /// chips its Toughness gauge; everything else lands for damage and leaves the bar
    /// untouched, which is HSR's rule and the reason the element chart decides
    /// anything at all.
    ///
    /// A MONSTER HAS NO ELEMENT of its own (0.0.444, user: "มอนไม่มีธาตุ มอนมีแค่ weakness"): this
    /// list and `resists` are all it says about elements; an empty list means every element
    /// lands at 100%.
    #[serde(default)]
    pub weaknesses: Vec<String>,
    /// Elements this monster RESISTS — 80% damage (`docs/element-weakness-system.md`
    /// §2). Opt-in and meant for elites and bosses: a plain monster resists nothing,
    /// so bringing the wrong element costs a player the bonus, never a penalty.
    #[serde(default)]
    pub resists: Vec<String>,
    /// TOUGHNESS gauge, as a PERCENT of the level's standard gauge
    /// (`toughness_for_level`). 0 (the default, so old data loads) = no gauge at all,
    /// this creature can never be broken; 100 = the standard bar for its level.
    ///
    /// A percent rather than a number because the bar has to grow with the content:
    /// a fixed 60 breaks 0.23 times per kill at Lv10 and 6.9 times at Lv70 — too rare
    /// to matter at one end and permanent stun-lock at the other. What stays constant
    /// across the game is roughly ONE break per kill, and that is a function of how
    /// long a monster survives, which is a function of its level.
    #[serde(default)]
    pub toughness_max: u16,
    /// The gauge this monster carries WHEN IT IS THE ZONE'S BOSS, as a percent of the
    /// level's standard gauge. 0 = use the shared `BOSS_TOUGHNESS_PCT`. A boss fight is one long target instead of a stream of
    /// short ones, so the same 60-point bar would break several times a minute and
    /// the break would stop being an event — the bar has to be sized to the fight,
    /// not to the creature (`docs/element-weakness-system.md` §6.2). Per-boss
    /// because a boss with an add phase and a boss that stands alone do not want
    /// the same number.
    #[serde(default)]
    pub boss_toughness_max: u16,
    pub speed_milli: u32,
    pub atk_speed_milli: u32,
    /// Drop table: pairs `(item_id, weight)`. Validation enforces
    /// item_id ∈ items at load time.
    pub drops: Vec<DropRow>,
    /// Chebyshev attack range. Default 1 (melee).
    #[serde(default = "default_atk_range")]
    pub atk_range: i32,
    /// Projectile travel time in ticks (0 = instant).
    #[serde(default)]
    pub proj_travel_ticks: u32,
    /// XP awarded to the killer on this monster's death. Defaulted to
    /// 10 so old game_data.json (pre-BETA-5) still loads with sensible
    /// behaviour; tune per-monster as Phase 1 stabilizes.
    #[serde(default = "default_xp_reward")]
    pub xp_reward: u32,
    /// Job XP on death (feeds the killer's Job Level → skill points).
    /// Defaults to ~60% of the base xp default so pre-Phase-3 data still
    /// loads with a sensible job curve; tune per-monster in game_data.
    /// Gil (currency) dropped to the party on death.
    #[serde(default = "default_gil_reward")]
    pub gil_reward: u32,
    /// How many minions this monster spawns AROUND itself when it appears as a
    /// boss (pulled from the zone's spawn pool, ringing the boss). 0 = none.
    /// Only meaningful for a `boss_monster_id`. Per-boss so each can differ.
    #[serde(default)]
    pub boss_minions: u32,
    /// Skill ids (active + passive) this monster carries — instantiated at level 1
    /// on spawn. Drives on-death blasts (`self_destruct` / `toxic_burst`) and any
    /// future monster ability; a monster may carry SEVERAL. Validated against the
    /// skill catalog at load. Empty by default → pre-existing monsters load unchanged.
    #[serde(default)]
    pub skills: Vec<String>,

    // --- RO-style descriptive stat block (bestiary / lootable info). Combat still
    // uses the DIRECT hp/atk/def/hit/flee above; these are identity + future
    // derivation. ALL #[serde(default)] so the pre-existing monsters load unchanged.
    /// Display name (e.g. "Pinkling"). Empty = fall back to the def_id.
    #[serde(default)]
    pub name: String,
    /// BESTIARY grouping: same-model monsters (Baby/Teen/Big Pinkling) share
    /// one bestiary entry + kill pool under this key. Empty = the def stands
    /// alone under its own id. Convention: the key equals the representative
    /// def's id (e.g. "pinkling"), whose name/stats front the entry.
    #[serde(default)]
    pub bestiary_group: String,
    /// The STAGE this monster fights like when nothing places it (a test, story mode). 0 = unset
    /// (→ the zone's). A placed monster always takes the ZONE's stage.
    #[serde(default, alias = "level")]
    pub stage: u16,
    /// RO race: Formless / Undead / Brute / Plant / Insect / Fish / Demon /
    /// Demihuman / Angel / Dragon. Free-form string for now.
    #[serde(default)]
    pub race: String,
    /// RO size: Small / Medium / Large.
    #[serde(default)]
    pub size: String,
    /// VISUAL sprite key — FREE-FORM, decoupled from `element` (which is combat only).
    /// The renderer maps it to a texture (e.g. "slime_pink"/"slime_green"). Empty =
    /// the renderer falls back to its element/def_id rules (old monsters unchanged).
    #[serde(default)]
    pub sprite: String,

    // --- MULTIPLIER SCALING (Diablo/PoE-style role × band-base) ------------------------------
    /// When true, this monster's COMBAT stats are DERIVED from the zone's `base_monster_id` (the
    /// band's standard mob) times the per-stat multipliers below — instead of its own absolute
    /// hp/atk/def/…. So ONE def scales across every band it appears in (the base is per-band and
    /// already RO-shaped: convex hp/atk, level-linear hit/flee). Identity (element / race / size /
    /// speeds / skills) and drops stay the def's own. Default false = the absolute stats above
    /// (every existing monster loads unchanged).
    /// Per-stat multipliers in basis points (0 → treated as ×1 = 10000) applied to the BASE
    /// monster's stat when `scale_from_base`. The role identity: a tank sets hp/def high, a glass
    /// cannon atk high + hp low, a dodger flee high. `atk_mult` scales atk + atk_min/atk_max.
    #[serde(default)]
    pub hp_mult_bp: u32,
    #[serde(default)]
    pub atk_mult_bp: u32,
    #[serde(default)]
    pub def_mult_bp: u32,
    /// REWARD multipliers for a `scale_from_base` monster (0 bp → ×1, i.e. the band
    /// base's own reward). Without these a scaled BOSS pays what a trash mob pays —
    /// the combat mults carry its difficulty across bands but its XP would collapse to
    /// the base's, which is how a boss ends up worth 1/18th of what it was.
    #[serde(default)]
    pub xp_mult_bp: u32,
}

fn default_weapon_level() -> u8 { 1 }
fn default_atk_range() -> i32 { 1 }
fn default_xp_reward() -> u32 { 10 }
fn default_gil_reward() -> u32 { 500 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DropRow {
    pub item_id: String,
    pub weight: u32,
    /// SERVER-ROLL candidate rate (bp per kill) — see `DropEntry::server_bp`. Default 0.
    #[serde(default)]
    pub server_bp: u32,
}

/// One row of `GameData.level_base`.
/// A DELIBERATE exception to the grade formula for one (def × grade) — the only way a single
/// product can leave the curve without the whole ladder moving with it (0.0.376).
///
/// The formula (`Grade::scale_raw` over the def's base) is the DEFAULT; an override is INTENT.
/// Every override is a sentence the designer wrote down — "this sword at Immortal hits harder
/// than the curve says" — and the whole set is meant to fit on one screen. Two tests keep it
/// honest: an override that equals the formula is a leftover and fails, and one that strays more
/// than `MAX_OVERRIDE_RATIO` from it fails unless marked `deliberate` (a mistyped zero would
/// otherwise ship).
///
/// Baked into every drop template (`DropEntry.overrides`) because the drop path that assigns a
/// grade holds no catalog — the same reason `gear_grade_odds` rides on the monster.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GearOverride {
    pub def_id: String,
    pub grade: crate::items::Grade,
    /// Replaces the GRADED atk / def. `None` = that stat keeps the formula.
    #[serde(default)]
    pub atk: Option<i32>,
    #[serde(default)]
    pub def: Option<i32>,
    /// Waives the "too far from the formula" guard. Say why in `why`.
    #[serde(default)]
    pub deliberate: bool,
    #[serde(default)]
    pub why: String,
}

/// See `GameData::loot_rules`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LootRules {
    /// Star Stone rows, chosen by the monster's level tier and whether it is a zone boss.
    #[serde(default)]
    pub orbs: Vec<OrbRule>,
    /// Boss Core / Track rows on every zone boss.
    #[serde(default)]
    pub capstones: Vec<CapstoneRule>,
    /// The GEAR pool by band — every Weapon / Armor def of the monster's gear tier, at absolute
    /// odds, instead of 27-38 rows copied onto each monster (pass 2, 0.0.365).
    #[serde(default)]
    pub gear: Option<GearRule>,
    /// The SUBSTAT table (0.0.384): how many lines a grade carries, which stats each slot may
    /// carry, each stat's value window per tier and its cap. Resolved by `substats::lines_for`;
    /// authored here so the lines are tuned by editing this block. Empty = gear carries no lines.
    #[serde(default)]
    pub substats: SubstatRules,
    /// The GEM table (0.0.386): the sets (which two stats, at what Common value), the roll window,
    /// the merge count and the drop odds. Sockets per grade and the per-stat caps are code
    /// (`gems::SOCKETS_BY_GRADE`, `gems::cap_of`) — ladder-shaped invariants like the grade
    /// multipliers, not tuning knobs. See `docs/gem-system-design.md`.
    #[serde(default)]
    pub gems: GemRules,
    /// ONE fusion rule for gear and gems (user 2026-09-16: "ควรทำเหมือนกันสองระบบ"): the fail odds
    /// by input grade and the crafter ladder that buys them down. Gear reads it with
    /// `Measures.fusions` (the Blacksmith), gems with `Measures.gem_fusions` (the Gem Crafter).
    #[serde(default)]
    pub fusion: FusionRules,
}

/// See `LootRules::fusion`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FusionRules {
    /// Odds (bp) that a fusion FAILS, indexed by the INPUT grade's step; a row past the end
    /// repeats the last. A failure hands back ONE item of the INPUT grade (type / set drawn as
    /// usual) — five are lost. TBH's synthesis reads ~50% success at the top grades; the low
    /// grades are free so the first combines teach the system without a punishment.
    #[serde(default)]
    pub fail_bp_by_grade: Vec<u32>,
    /// CRAFTER level ("+1% ตามเลเวล blacksmith/gem crafter"): every `crafter_fusions_per_level`
    /// fusions RUN (success or failure) raise the level by one, up to `crafter_level_cap`, and
    /// each level takes `crafter_bp_per_level` off the fail odds. Derived from a `Measures`
    /// counter — no new state, and the server already verifies those.
    #[serde(default)]
    pub crafter_fusions_per_level: u32,
    #[serde(default)]
    pub crafter_level_cap: u32,
    #[serde(default)]
    pub crafter_bp_per_level: u32,
}

/// See `GameData::crests` and `docs/crest-system-design.md`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrestRules {
    /// The lowest tier that disenchants into anything (user: "อาวุธ T3-T7 ... 5 ชนิด × 4 = 20
    /// unique material"): below it a piece is fodder for fusion only.
    #[serde(default)]
    pub min_tier: u8,
    /// The highest material tier; a piece above it yields this tier's material.
    #[serde(default)]
    pub max_tier: u8,

    /// SHARDS a disenchant yields per GRADE step of the piece (index = `Grade::step()`); gems
    /// yield `gem_yield` flat — their grade is already their tier. `shards_per_material` shards
    /// make one material (user: "ได้ 10% เป็นตัวเลขเลยไม่ RNG" — ten Common pieces are one), so the
    /// purse counts shards and every price is `materials × shards_per_material`. No draw.
    #[serde(default)]
    pub yield_by_grade: Vec<u32>,
    #[serde(default)]
    pub gem_yield: u32,
    #[serde(default = "ten_u32")]
    pub shards_per_material: u32,
    #[serde(default)]
    pub slots: Vec<CrestSlot>,
}

/// One crest — a stat ladder paid in ONE material, its category's at its tier (user: "20 unique
/// material ต้องอัพเดท 20 stats ที่ต่างกัน"): four categories × T3..T7 = twenty crests, twenty stats.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrestSlot {
    pub id: String,
    pub category: crate::crest::CrestKind,
    /// The material tier this crest eats (`<category>_t<tier>`).
    pub tier: u8,
    pub stat: crate::training::TrainStat,
    #[serde(default)]
    pub element: Option<Element>,
    pub per_level: i32,
    pub max_level: u8,
    /// Materials per level = `mats_per_level × level`.
    #[serde(default = "one_u32")]
    pub mats_per_level: u32,
}

fn one_u32() -> u32 {
    1
}

fn ten_u32() -> u32 {
    10
}

/// See `GameData::party`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartyRules {
    #[serde(default)]
    pub slot_gil: Vec<u64>,
}

/// TRAINING RULES — `docs/training-system-plan.md`. WoG's shape: every slot prices its next
/// level off ONE curve (`cost_curve_bp`, index = the level being bought − 1, 10000 = ×1) times
/// the slot's `base_gil` and `cost_mult_bp`; the stat rises by `per_level` per level, linear,
/// to `max_level`. No RNG, no materials — Gil in, stat out.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrainingRules {
    /// Cost multiplier per level bought, 10000 = ×1 at level 1. A slot past the curve's end
    /// cannot be bought (the curve IS the cap when it is shorter than `max_level`).
    #[serde(default)]
    pub cost_curve_bp: Vec<u64>,
    #[serde(default)]
    pub slots: Vec<TrainingSlot>,
    /// BREADTH GATE (user 2026-09-17: "5→6 ต้องให้มีทุกค่ารวมกัน 50 / 10→11 รวมกัน 100 ... ไม่ให้
    /// sink อันเดียว"): a slot standing on a multiple of `gate_step` cannot go up until the SUM of
    /// every slot's level is at least `gate_total_per_step × (level / gate_step)`. 0 = no gate.
    #[serde(default)]
    pub gate_step: u8,
    #[serde(default)]
    pub gate_total_per_step: u32,
}

/// One Training slot. `element` is read by the element-keyed stats only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrainingSlot {
    pub id: String,
    pub stat: crate::training::TrainStat,
    #[serde(default)]
    pub element: Option<Element>,
    /// Stat gained per level, in the stat's own unit (bp for the `*_pct` stats, points for flat).
    pub per_level: i32,
    pub max_level: u8,
    /// Gil for level 1; later levels multiply it by the curve.
    pub base_gil: u64,
    /// Per-slot price scale, 10000 = ×1 (Ward slots ×0.5, Cdr ×5).
    #[serde(default = "ten_thousand")]
    pub cost_mult_bp: u32,
}

fn ten_thousand() -> u32 {
    10_000
}

/// See `LootRules::gems`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GemRules {
    #[serde(default)]
    pub sets: Vec<GemSet>,
    /// Half-width of the value roll on socket, in bp of the mid value (500 = ±5%).
    #[serde(default)]
    pub roll_window_bp: u32,
    /// Gems of one grade that fuse into one of the next (0 = fusion off).
    #[serde(default)]
    pub merge: u32,

    /// Basis points of ONE field / boss kill that pay a gem of the monster's grade odds, split
    /// across the sets by `GemSet::weight`.
    #[serde(default)]
    pub field_bp: u32,
    #[serde(default)]
    pub boss_bp: u32,
    /// The same two BY STAGE (0.0.402), like `GearRule::slot_bp_by_stage`: the first row at or
    /// past the stage wins, past the last row the last holds, no rows = the flat pair above.
    #[serde(default)]
    pub field_bp_by_stage: Vec<GemBpRow>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GemBpRow {
    pub max_stage: u16,
    pub field_bp: u32,
    pub boss_bp: u32,
}

impl GemRules {
    /// (field, boss) gem odds for a monster placed on `stage` (0 = the flat pair).
    pub fn bp_at(&self, stage: u16) -> (u32, u32) {
        if stage == 0 {
            return (self.field_bp, self.boss_bp);
        }
        self.field_bp_by_stage
            .iter()
            .find(|r| stage <= r.max_stage)
            .or_else(|| self.field_bp_by_stage.last())
            .map(|r| (r.field_bp, r.boss_bp))
            .unwrap_or((self.field_bp, self.boss_bp))
    }
}

/// One gem set: a NAME the player learns ("Strife = the hitting one"), the slot group it fits,
/// and its two stats at Common. The grade's RAW ladder (`Grade::raw_bp`, ×1.3/step) scales them.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GemSet {
    pub id: String,
    pub group: crate::gems::GemGroup,
    pub stats: Vec<GemStat>,
    /// Relative drop weight among the sets (default 1) — and the fusion output odds.
    #[serde(default = "one")]
    pub weight: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GemStat {
    pub key: crate::substats::SubstatKey,
    pub base: i32,
    /// The element, for `element_dmg` only (None elsewhere).
    #[serde(default)]
    pub element: Option<crate::grid::Element>,
}

/// See `LootRules::substats` and `docs/tbh-substat-datamine.md` §7.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubstatRules {
    /// Lines per grade, index = `Grade::step()`. TBH's 0/1/1/2/2/3/3/3/3/3.
    #[serde(default)]
    pub lines_by_grade: Vec<u32>,
    /// Extra lines on the utility slots (Necklace / Ring / Earring / Bracer), which have no main
    /// stat — lines are their whole value.
    #[serde(default)]
    pub utility_extra: u32,
    /// Every stat the pools may name, with its value window at each tier (index = tier−1, seven
    /// rows) in the stat's native unit (flat, or basis points), and its cap (0 = none). The
    /// window is what a Common piece carries; the grade's percent ladder multiplies it.
    #[serde(default)]
    pub stats: Vec<SubstatDef>,
    /// Which stats each paper-doll slot may carry. A slot with no row carries nothing.
    #[serde(default)]
    pub slots: Vec<SubstatSlotPool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubstatDef {
    pub key: crate::substats::SubstatKey,
    pub tiers: Vec<(i32, i32)>,
    #[serde(default)]
    pub cap: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubstatSlotPool {
    pub slot: crate::items::EquipSlot,
    pub pool: Vec<crate::substats::SubstatKey>,
}

/// See `LootRules::gear`. A monster's gear TIER (the `_sN` badge, `gear_roll::def_tier`) comes
/// from its level through `bands`; every Weapon / Armor def of that tier is a row. Odds are
/// basis points of ONE kill, taken out of the table's `nothing` share so the authored rows keep
/// their own odds exactly.
///
/// The odds are authored PER SLOT, not per def (0.0.378, user 2026-09-15: "drop weight ให้เท่าๆกัน
/// ก่อนค่อยจูน"): each paper-doll slot of the tier gets `field_slot_bp` (or `boss_slot_bp`) per
/// kill, split across that slot's defs by their relative weights. Per-def odds had made the drop
/// mix a function of how many defs a slot happened to have — six weapon families at three
/// variants each put ~60% of every drop in one of a party member's eleven slots, and adding a
/// weapon family would have shifted every other slot's share without anyone deciding it.
///
/// Within a slot every def weighs 1, daggers (the cross-class weapon) `dagger_mult`. The slotted
/// variants that used to weigh differently are gone (0.0.388): sockets are the grade's.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GearRule {
    /// Ascending `max_stage` → tier. A stage past the last row takes the last tier.
    pub bands: Vec<GearBand>,
    /// Basis points of ONE field kill that each SLOT of the tier pays out — the FLOOR, used when
    /// `slot_bp_by_stage` has no row for the stage (and by hand-built tests).
    pub field_slot_bp: u32,
    /// The same for a zone boss.
    pub boss_slot_bp: u32,
    /// The same two numbers BY STAGE (0.0.398, `stage-ladder-renovation.md` §4: "ดรอปดีขึ้นตามด่าน"):
    /// ascending `max_stage`, the first row at or past the stage wins; past the last row the
    /// last holds. Replaces the loot gap PENALTY as the reason to go forward.
    #[serde(default)]
    pub slot_bp_by_stage: Vec<SlotBpRow>,
    #[serde(default = "one")]
    pub dagger_mult: u32,
    /// TIER MIX (0.0.455, user: "โอกาสดรอปเกียร์ควรเท่าเดิมทุกเลเวล แต่โอกาสโรลไป tier สูงจะเพิ่มขึ้น
    /// ทีละนิด … T1-T2 ควรจะตกเป็นหลักเสมอ"): for a monster on `stage`, the share of each tier
    /// (`mix_bp[tier − 1]`, bp of the gear that drops) — the slot's odds stay what
    /// `slot_bp_by_stage` says; only WHICH tier a piece is moves with the stage. When authored
    /// it replaces the band + next-tier-preview pool; empty = the old bands.
    #[serde(default)]
    pub tier_mix_by_stage: Vec<TierMixRow>,
    /// Monsters that carry NO gear at all (an authored exception, not an omission).
    #[serde(default)]
    pub exclude: Vec<String>,
    /// GRADE odds per band (0.0.373). Authored HERE rather than in code so the numbers most
    /// likely to be swept — how fat the tail is at each band — can be tuned against a
    /// `sim-cli --data <copy>` run with no rebuild and no catalog-digest bump on the real file.
    /// Empty = fall back to `drops::GRADE_ODDS_BP`, which is what a hand-built test state uses.
    #[serde(default)]
    pub grade_odds: Vec<GradeOddsRow>,
}

/// One row of `GearRule::grade_odds`: the odds a dropped piece lands on each grade, for every
/// band up to `max_band`. `bp` is indexed by `Grade::step()` (0 = the floor) and must sum to
/// 10,000 — a row that does not is a silent hole that pays out as Common.
///
/// `max_band` is the DESIGN's band number (20 across the launch ladder), not the catalog's
/// `zone_b<N>`; the resolver maps a monster's level onto it.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GradeOddsRow {
    /// Inclusive top STAGE of the row (0.0.399 — was a design band of twenty).
    #[serde(alias = "max_band")]
    pub max_stage: u16,
    pub bp: Vec<u32>,
}

fn one() -> u32 {
    1
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlotBpRow {
    pub max_stage: u16,
    pub field_slot_bp: u32,
    pub boss_slot_bp: u32,
}

impl GearRule {
    /// The gear TIER dropping on `stage`, and the tier PREVIEWED there — the next one, in the last
    /// quarter of the band (0.0.404), `None` elsewhere and past the top tier.
    pub fn tier_at(&self, stage: u16) -> (u32, Option<u32>) {
        let mut lo: u16 = 1;
        for (i, b) in self.bands.iter().enumerate() {
            if stage <= b.max_stage {
                let span = u32::from(b.max_stage.saturating_sub(lo)).max(1);
                let into = u32::from(stage.saturating_sub(lo));
                let preview = if into * 4 >= span * 3 { self.bands.get(i + 1).map(|n| n.tier).filter(|t| *t > b.tier) } else { None };
                return (b.tier, preview);
            }
            lo = b.max_stage.saturating_add(1);
        }
        (self.bands.last().map(|b| b.tier).unwrap_or(1), None)
    }

    /// (field, boss) slot odds for a monster placed on `stage` (0 = unplaced → the floor).
    pub fn slot_bp_at(&self, stage: u16) -> (u32, u32) {
        if stage == 0 {
            return (self.field_slot_bp, self.boss_slot_bp);
        }
        self.slot_bp_by_stage
            .iter()
            .find(|r| stage <= r.max_stage)
            .or_else(|| self.slot_bp_by_stage.last())
            .map(|r| (r.field_slot_bp, r.boss_slot_bp))
            .unwrap_or((self.field_slot_bp, self.boss_slot_bp))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GearBand {
    #[serde(alias = "max_level")]
    pub max_stage: u16,
    pub tier: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrbRule {
    /// Inclusive top of the monster level range this row applies to (0 = every level).
    #[serde(default)]
    pub max_level: u16,
    /// `true` = zone bosses only, `false` = field monsters only.
    #[serde(default)]
    pub boss: bool,
    pub item_id: String,
    pub server_bp: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapstoneRule {
    pub item_id: String,
    pub server_bp: u32,
    /// The boss must be at least this level (0 = every boss).
    #[serde(default)]
    pub min_level: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TierMixRow {
    pub max_stage: u16,
    /// bp per tier, index = tier − 1; need not sum to 10,000 (normalised in use).
    pub mix_bp: Vec<u32>,
}

impl GearRule {
    /// The tier mix for `stage`: the first row whose `max_stage` covers it, else the last.
    pub fn mix_at(&self, stage: u16) -> Option<&[u32]> {
        if self.tier_mix_by_stage.is_empty() {
            return None;
        }
        self.tier_mix_by_stage
            .iter()
            .find(|r| stage <= r.max_stage)
            .or(self.tier_mix_by_stage.last())
            .map(|r| r.mix_bp.as_slice())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct StageBase {
    #[serde(alias = "level")]
    pub stage: u16,
    pub hp: i32,
    pub atk: i32,
    pub xp: u32,
    /// GIL A KILL PAYS at this stage (0.0.455, user: "ให้มอนสเตอร์ drop gil ไปด้วย … ต้องดูไม่ให้
    /// เยอะเกินไป"): the faucet Gil comes from now that fusion salvage is a fifth of what it
    /// was. Scaled by the monster's `xp_mult_bp` like XP (a boss pays its multiple). 0 = none.
    #[serde(default)]
    pub gil: u32,
    /// Hard DEF on the ×10 stat scale (0.0.399 — was `level × 10`; the table carries it so the
    /// ladder has no level in it). A monster's `def_mult_bp` scales this.
    #[serde(default)]
    pub def: i32,
    /// The standard Toughness gauge on this stage (was `toughness_for_level`); a monster's
    /// `toughness_max` is a PERCENT of it.
    #[serde(default)]
    pub toughness: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ZoneDef {
    pub id: String,
    pub width: i32,
    pub height: i32,
    /// Cell where the player is placed on entering the zone.
    pub player_spawn: Cell,
    /// Which player class to instantiate at `player_spawn`. Validated
    /// against `GameData.units` at load time. Used as the single
    /// member when `party` is empty.
    #[serde(alias = "player_class")]
    pub unit: String,
    /// Party roster: class ids to spawn as controllable members (ids
    /// FIRST_PLAYER_ID..). Empty = solo (`[unit]`). Each is validated
    /// against `units` at load.
    #[serde(default)]
    pub party: Vec<String>,
    /// How many alive monsters the zone tries to maintain via continuous
    /// random spawn. Must be > 0 unless `spawn_pool` is empty.
    pub target_population: u32,
    /// Sim ticks between spawn attempts when below target.
    pub spawn_interval_ticks: u32,
    /// Weighted pool of monster ids the zone draws from. Each pick is
    /// independent (Phase 0: no diminishing-returns or "don't spawn same
    /// type twice in a row" tricks — keep it simple, designer-tunable).
    pub spawn_pool: Vec<SpawnPoolEntry>,
    /// LANE MODE's pool (`crate::lane`, off by default): when non-empty it replaces `spawn_pool`
    /// for the room's groups while lane mode is on. The live field never reads it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lane_pool: Vec<SpawnPoolEntry>,
    /// LANE MODE group size, `[min, max]` per group, drawn per group (user 2026-10-12: stages 1-5,
    /// 3-5 per group): groups are released until the room's quota is out. `None` = the quota split
    /// into `LaneState::groups_per_room` equal groups.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane_group_size: Option<[u8; 2]>,
    /// Regular kills required before the zone boss appears.
    pub boss_threshold: u32,
    /// Monster id used as the zone boss. Must exist in `GameData.monsters`.
    /// Empty string means "no boss" — the zone loops forever without
    /// gating progression.
    pub boss_monster_id: String,
    /// RANDOM boss from the pool: when true, the zone boss is a random eligible pool monster
    /// (any but `mini_ghost` / `bat`) that WEARS `boss_monster_id`'s stats + rewards (so the
    /// boss looks/acts like a band mob but keeps the tuned boss difficulty). false = the fixed
    /// `boss_monster_id` is the boss, as usual. Used for the gothic bands 5-10.
    #[serde(default)]
    pub boss_from_pool: bool,
    /// Total monsters this zone will ever trickle in. 0 = endless (every band zone).
    #[serde(default)]
    pub max_spawns: u32,
    /// PLAYER SKILLS this zone enables (`regent_hand` = pick a member up out of a
    /// telegraphed AoE). These are the PLAYER's abilities, not a character's, and
    /// they are content-gated: a skill not listed here is a no-op in this zone, so
    /// a boss room can hand out an interaction the idle overworld never gets.
    /// A list (not a flag per skill) so a future player skill is one id, not a
    /// schema change. Empty = the zone plays fully hands-off.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub player_skills: Vec<String>,
    /// MULTIPLIER-SCALE base: the band's STANDARD monster id whose stats are the per-band base for
    /// `scale_from_base` mobs in this zone (final stat = base_monster.stat × the mob's mult). Empty
    /// = no base → a `scale_from_base` mob then falls back to its own absolute stats.
    #[serde(default)]
    pub base_monster_id: String,
    /// The STAGE this zone sits on (0.0.398): the row of `stage_base` its monsters are built from
    /// — hp / atk / def / toughness / xp, the gear tier and the grade odds all read it. A ladder
    /// zone's is its number; a dungeon or story zone authors the stage it should fight like. There
    /// is no monster LEVEL any more (0.0.399): the stage is the one difficulty number.
    #[serde(default)]
    pub stage: u16,
    /// Zone id to advance to after the boss dies. Empty string means
    /// "no advance" (terminal zone — keeps looping the cleared arena).
    pub next_zone_id: String,
    /// True on the FINAL room of an instance (its big boss). Clearing this room
    /// banks the loot escrow to the stash and triggers advance-or-farm.
    #[serde(default)]
    pub is_instance_end: bool,
    /// Zone id of room 1 of THIS instance (shared by all its rooms). Used to
    /// farm-loop the instance and to restart here on a party wipe.
    #[serde(default)]
    pub instance_start_zone_id: String,
}

/// Starting stats for the player character (GDD §3.5; §8 open question
/// — Phase 0 ships a single fixed class).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnitDef {
    /// AUTHORED attack growth, in thousandths, replacing what the six attributes
    /// used to derive: `(atk_base_milli + atk_per_level_milli × level) / 1000`.
    /// 0 (the default, so old data loads) = fall back to the attribute-derived
    /// path. The same number feeds BOTH physical and magic power — a character has
    /// one attack stat, and what it is spent on is decided by the skill's damage
    /// type, not by which pool it drew from.
    #[serde(default)]
    pub atk_base_milli: i32,
    #[serde(default)]
    pub atk_per_level_milli: i32,
    pub id: String,
    pub hp: i32,
    pub atk: i32,
    pub def: i32,
    pub element: Element,
    pub speed_milli: u32,
    pub atk_speed_milli: u32,
    /// Chebyshev cell distance the player can attack from (1 = melee).
    pub atk_range: i32,
    /// Ticks between firing the projectile and damage landing on the
    /// target. 0 = instant (melee). > 0 = ranged: an Arrow is spawned
    /// at fire time and resolves on its arrival tick (GDD §3.2).
    pub proj_travel_ticks: u32,
    /// Physical (default) or Magic. Magic classes scale damage off INT
    /// (MATK) and ignore the target's physical DEF.
    #[serde(default)]
    pub atk_type: DamageType,
    /// Marks the class's attack as ranged damage (an archer or a mage
    /// lobbing bolts). Defaults false (melee). Independent of
    /// `proj_travel_ticks` — see `Entity::is_ranged`.
    #[serde(default)]
    pub is_ranged: bool,
    /// Skill ids granted to this class (active + passive). Each is
    /// instantiated at level 1 on spawn / class switch. Validated against
    /// the skill catalog at load. Empty by default.
    #[serde(default)]
    pub skills: Vec<String>,
    /// RO Renewal-style per-class HP growth (see `effective_max_hp`). `hp` is
    /// the level-1 HP base; this adds the per-level growth.
    #[serde(default)]
    pub hp_per_level: i32,
    /// RO HP curve (rAthena `job_basepoints` factor mode). When `hp_factor != 0`
    /// HP uses the RO formula instead of the linear `hp_per_level`: per base level
    /// it gains `(hp_increase + hp_factor·level)/100`, so the per-level gain RISES
    /// with level (a convex curve). `hp_factor` = the exponential term, derived
    /// from the real RO per-class tables. 0 = legacy linear HP.
    #[serde(default)]
    pub hp_factor: i64,
    /// RO HP linear term (`HpIncrease`); paired with `hp_factor`. Only used when
    /// `hp_factor != 0`. 0 = legacy linear HP via `hp_per_level`.
    #[serde(default)]
    pub hp_increase: i64,
    /// Item id auto-equipped on spawn (RO: ATK/MATK come from the weapon,
    /// not a flat class `atk`). Empty = start bare-handed.
    #[serde(default)]
    pub starting_weapon: String,
}

/// One row of a zone's weighted spawn pool. The zone keeps a list of
/// these; at runtime each spawn rolls the pool once with a sim-PRNG
/// draw to pick which monster type appears.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpawnPoolEntry {
    pub monster_id: String,
    pub weight: u32,
}

impl GameData {
    /// Parse from a JSON string and validate. Returns a description
    /// of the first problem found if either fails.
    pub fn from_json_str(s: &str) -> Result<Self, String> {
        let data: GameData = serde_json::from_str(s)
            .map_err(|e| format!("parse error: {e}"))?;
        data.validate()?;
        Ok(data)
    }

    /// Catch typos and dangling references before any runtime ever
    /// sees the data. "Bad data fails loudly at build time, never at
    /// runtime" (GDD §7.1, working agreement 5).
    pub fn validate(&self) -> Result<(), String> {
        // BTreeSet — never HashSet (§4.2 rule 4). Iteration order
        // would otherwise be nondeterministic across runs, which can
        // hide validation order dependencies.
        let mut item_ids: BTreeSet<&str> = BTreeSet::new();
        for it in &self.items {
            if !item_ids.insert(it.id.as_str()) {
                return Err(format!("duplicate item id: {}", it.id));
            }
        }
        let mut monster_ids: BTreeSet<&str> = BTreeSet::new();
        for m in &self.monsters {
            if !monster_ids.insert(m.id.as_str()) {
                return Err(format!("duplicate monster id: {}", m.id));
            }
        }
        let mut zone_ids: BTreeSet<&str> = BTreeSet::new();
        for z in &self.zones {
            if !zone_ids.insert(z.id.as_str()) {
                return Err(format!("duplicate zone id: {}", z.id));
            }
        }
        let mut unit_defs: BTreeSet<&str> = BTreeSet::new();
        for c in &self.units {
            if !unit_defs.insert(c.id.as_str()) {
                return Err(format!("duplicate unit id: {}", c.id));
            }
            for sid in &c.skills {
                if crate::skills::skill_def(sid).is_none() {
                    return Err(format!(
                        "unit {} grants unknown skill {}", c.id, sid
                    ));
                }
            }
        }

        for m in &self.monsters {
            for sid in &m.skills {
                if crate::skills::skill_def(sid).is_none() {
                    return Err(format!("monster {} carries unknown skill {}", m.id, sid));
                }
            }
            for d in &m.drops {
                if !item_ids.contains(d.item_id.as_str()) {
                    return Err(format!(
                        "monster {} drops unknown item {}", m.id, d.item_id
                    ));
                }
            }
            let total: u32 = m.drops.iter().map(|d| d.weight).sum();
            if !m.drops.is_empty() && total == 0 {
                return Err(format!("monster {} has drops with total weight 0", m.id));
            }
        }

        for z in &self.zones {
            if z.width <= 0 || z.height <= 0 {
                return Err(format!("zone {} has non-positive dimensions", z.id));
            }
            if !unit_defs.contains(z.unit.as_str()) {
                return Err(format!(
                    "zone {} references unknown unit {}", z.id, z.unit
                ));
            }
            for pc in &z.party {
                if !unit_defs.contains(pc.as_str()) {
                    return Err(format!(
                        "zone {} party references unknown unit {}", z.id, pc
                    ));
                }
            }
            if z.player_spawn.x < 0 || z.player_spawn.x >= z.width
                || z.player_spawn.y < 0 || z.player_spawn.y >= z.height
            {
                return Err(format!(
                    "zone {} player_spawn out of bounds at ({}, {})",
                    z.id, z.player_spawn.x, z.player_spawn.y
                ));
            }
            if !z.spawn_pool.is_empty() {
                // A pool with NO ambient population is legal ONLY when the zone's boss draws
                // minions from it (`spawn_minions_around` picks from the zone pool) — e.g. a
                // dungeon room that is just a mini-boss + its escort. Otherwise the pool is
                // dead data and the zone would never spawn anything.
                if z.target_population == 0 {
                    let boss_escorted = self
                        .monsters
                        .iter()
                        .find(|m| m.id == z.boss_monster_id)
                        .is_some_and(|m| m.boss_minions > 0);
                    if !boss_escorted {
                        return Err(format!(
                            "zone {} has spawn_pool but target_population is 0", z.id
                        ));
                    }
                }
                let total: u32 = z.spawn_pool.iter().map(|e| e.weight).sum();
                if total == 0 {
                    return Err(format!(
                        "zone {} spawn_pool total weight is 0", z.id
                    ));
                }
                for entry in &z.spawn_pool {
                    if !monster_ids.contains(entry.monster_id.as_str()) {
                        return Err(format!(
                            "zone {} pool references unknown monster {}", z.id, entry.monster_id
                        ));
                    }
                }
            }
            if let Some([lo, hi]) = z.lane_group_size {
                if lo == 0 || lo > hi {
                    return Err(format!("zone {} lane_group_size [{lo}, {hi}] is not 1 <= min <= max", z.id));
                }
            }
            if !z.lane_pool.is_empty() {
                if z.lane_pool.iter().map(|e| e.weight).sum::<u32>() == 0 {
                    return Err(format!("zone {} lane_pool total weight is 0", z.id));
                }
                if let Some(e) = z.lane_pool.iter().find(|e| !monster_ids.contains(e.monster_id.as_str())) {
                    return Err(format!("zone {} lane_pool references unknown monster {}", z.id, e.monster_id));
                }
            }
            // Boss validation: only if boss_monster_id is non-empty.
            if !z.boss_monster_id.is_empty()
                && !monster_ids.contains(z.boss_monster_id.as_str())
            {
                return Err(format!(
                    "zone {} references unknown boss monster {}", z.id, z.boss_monster_id
                ));
            }
            if !z.next_zone_id.is_empty() && !zone_ids.contains(z.next_zone_id.as_str()) {
                return Err(format!(
                    "zone {} next_zone_id {} not found", z.id, z.next_zone_id
                ));
            }
        }

        // Recipes: unique ids + every referenced item def exists + non-empty ingredients.
        let mut recipe_ids: BTreeSet<&str> = BTreeSet::new();
        for r in &self.recipes {
            if !recipe_ids.insert(r.id.as_str()) {
                return Err(format!("duplicate recipe id: {}", r.id));
            }
            if !item_ids.contains(r.output_def_id.as_str()) {
                return Err(format!("recipe {} output references unknown item {}", r.id, r.output_def_id));
            }
            if r.ingredients.is_empty() {
                return Err(format!("recipe {} has no ingredients", r.id));
            }
            for ing in &r.ingredients {
                if !item_ids.contains(ing.def_id.as_str()) {
                    return Err(format!("recipe {} references unknown ingredient {}", r.id, ing.def_id));
                }
            }
        }

        Ok(())
    }

    pub fn item(&self, id: &str) -> Option<&ItemDef> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn recipe(&self, id: &str) -> Option<&CraftRecipeDef> {
        self.recipes.iter().find(|r| r.id == id)
    }

    /// The base stats a monster of `level` is built from.
    ///
    /// Clamped at both ends rather than returning None: below the table is level 1, and ABOVE it
    /// continues at the slope of the last ten levels, so Apex and every future cap raise are
    /// covered without anyone extending the table by hand. A missing table (a fixture that builds
    /// `GameData` by hand) yields a level-1-shaped row of 1s, which keeps such tests running
    /// instead of dividing by zero.
    pub fn stage_stats(&self, stage: u16) -> StageBase {
        if self.stage_base.is_empty() {
            return StageBase { stage, hp: 1, atk: 1, xp: 1, gil: 0, def: 1, toughness: 20 };
        }
        let lv = stage.max(1);
        if let Some(row) = self.stage_base.iter().find(|r| r.stage == lv) {
            return row.clone();
        }
        let last = self.stage_base.last().unwrap();
        if lv <= last.stage {
            return self.stage_base[0].clone();
        }
        // Past the table: geometric continuation off the final decade.
        let back = self
            .stage_base
            .iter()
            .rev()
            .find(|r| r.stage + 10 <= last.stage)
            .unwrap_or(&self.stage_base[0]);
        let span = (last.stage - back.stage).max(1) as i64;
        let over = (lv - last.stage) as i64;
        // `now/then` is the growth across `span` LEVELS (ten, for a full table), so it may be
        // applied once per span — not once per level. Applying it per level was 0.0.274's bug and
        // it compounds: at level 110 it returned 678,728 HP where the curve wants 25,484, 26x
        // over, and it got worse every level after. Latent only because the shipped table stops
        // exactly at the highest zone level.
        //
        // Integer-only: take the whole spans, then walk the remainder LINEARLY across the next
        // one. No roots, no floats, monotone, and exact at every span boundary.
        let grow = |now: i64, then: i64| -> i64 {
            if then <= 0 || now <= 0 { return now; }
            let step = |v: i64| -> i64 { (v.saturating_mul(now) / then.max(1)).min(i32::MAX as i64) };
            let mut v = now;
            for _ in 0..(over / span) {
                v = step(v);
                if v >= i32::MAX as i64 { return i32::MAX as i64; }
            }
            let rem = over % span;
            if rem > 0 {
                let next = step(v);
                v += (next - v) * rem / span;
            }
            v.min(i32::MAX as i64)
        };
        StageBase {
            stage: lv,
            hp: grow(last.hp as i64, back.hp as i64) as i32,
            atk: grow(last.atk as i64, back.atk as i64) as i32,
            xp: last.xp,
            gil: last.gil,
            def: grow(last.def as i64, back.def as i64) as i32,
            toughness: grow(i64::from(last.toughness), i64::from(back.toughness)).min(i64::from(u16::MAX)) as u16,
        }
    }

    pub fn monster(&self, id: &str) -> Option<&MonsterDef> {
        self.monsters.iter().find(|m| m.id == id)
    }

    pub fn zone(&self, id: &str) -> Option<&ZoneDef> {
        self.zones.iter().find(|z| z.id == id)
    }

    pub fn unit(&self, id: &str) -> Option<&UnitDef> {
        self.units.iter().find(|c| c.id == id)
    }

    /// The units a player may be handed or may recruit: the ones with a doctrine — a Potential
    /// board — so a unit that exists can also grow. The ten legacy class stat blocks have none;
    /// they remain in the catalog as the base an Exemplar line builds on and as test fixtures,
    /// but a party screen must not offer a swordmaster that can never rise past rank 1, and the
    /// server must not anchor a genesis on one.
    pub fn creatable_units(&self) -> Vec<&UnitDef> {
        self.units
            .iter()
            .filter(|u| crate::potential::doctrine_for_unit(&u.id).is_some())
            .collect()
    }
}

#[cfg(test)]
mod level_extrapolation_tests {
    use super::*;

    fn catalog() -> GameData {
        GameData::from_json_str(include_str!("../../../../data/game_data.json"))
            .expect("game_data.json must parse")
    }

    /// MEASURES the extrapolated slope against the table's own; a boolean "it returned something"
    /// would have passed all the way through the 26x bug this replaces.
    ///
    /// `level_stats` continues the curve past the authored table so a cap raise, an Apex band or a
    /// story mission can name any level without anyone hand-extending 100 rows. The growth it
    /// continues at is the last decade's — which means the ratio must be applied once per DECADE.
    /// 0.0.274 applied it once per LEVEL: level 110 came back with 678,728 HP where the curve wants
    /// 25,484. It never fired in play only because the shipped table happens to stop at 100 and the
    /// deepest zone is level 100.
    ///
    /// Sabotage that turns this red: put the `for _ in 0..over` loop back (decade ratio becomes
    /// ~1.44^10 = 40x), or drop the remainder interpolation (101-109 all return level 100's number).
    #[test]
    fn past_the_table_a_decade_grows_by_one_decade_not_by_ten() {
        let d = catalog();
        let table_top = d.stage_base.last().expect("a stage table").stage;
        let top = d.stage_stats(table_top);
        let prev = d.stage_stats(table_top - 10);
        // The slope the table itself ends on, in basis points.
        let want_bp = (top.hp as i64) * 10_000 / (prev.hp as i64);

        // Inside the table, `level_stats` must be a lookup, not an estimate.
        for row in &d.stage_base {
            let got = d.stage_stats(row.stage);
            assert_eq!(
                (got.hp, got.atk),
                (row.hp, row.atk),
                "stage {} must come back verbatim from the table",
                row.stage
            );
        }

        // Past it, every decade must grow by that same slope.
        let mut base = top.hp as i64;
        for k in 1..=4 {
            let lv = table_top + 10 * k;
            let hp = d.stage_stats(lv).hp as i64;
            let got_bp = hp * 10_000 / base;
            let err_pct = (got_bp - want_bp).abs() * 100 / want_bp;
            assert!(
                err_pct <= 2,
                "level {lv}: decade grew {got_bp} bp, table's own slope is {want_bp} bp \
                 ({err_pct}% off). Per-LEVEL application would read ~{} bp here.",
                want_bp.pow(2) / 10_000,
            );
            base = hp;
        }

        // The decade check above passes whether or not the levels INSIDE a decade move, because it
        // only ever samples boundaries. So sample the inside directly: without the remainder term
        // every level from 101 to 109 returns level 100's number and the curve is a staircase.
        // (Written after the first version of this test survived exactly that sabotage.)
        let mid = d.stage_stats(table_top + 5).hp;
        let full = d.stage_stats(table_top + 10).hp;
        assert!(
            mid > top.hp && mid < full,
            "mid-decade level {} came back {mid}, outside the ({}, {full}) it must sit between —              the remainder term is missing and the curve is a staircase",
            table_top + 5,
            top.hp,
        );

        // And STRICTLY monotone, level by level: `>=` would let a flat run through.
        let mut last = top.hp;
        for lv in (table_top + 1)..=(table_top + 25) {
            let hp = d.stage_stats(lv).hp;
            assert!(
                hp > last,
                "level {lv} hp {hp} did not rise above level {} hp {last}",
                lv - 1
            );
            last = hp;
        }
    }
}
