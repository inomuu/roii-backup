//! Input events — every player-initiated action passes through here.
//!
//! GDD §4.4 ("event sourcing"): every action that mutates state is
//! either implicit (a tick) or explicit (an `InputEvent`). The save
//! file is `(seed + initial state + Vec<InputEvent>)`, and replays
//! work by re-applying inputs in order onto a fresh state.
//!
//! Rules:
//!   - `apply_input` is pure with respect to GameState (no I/O).
//!   - Invalid inputs are no-ops (NEVER panic) — replay must be
//!     idempotent even across game_data version skews.
//!   - Each event variant gets one match arm here, exactly once.

use serde::{Deserialize, Serialize};

use crate::data::GameData;
use crate::state::{GameState, GilEarn};
use crate::stats::StatKind;

pub(crate) mod handlers;
use handlers::*;
pub(crate) use handlers::{drop_held_and_arm_cooldown, recharge_pickup, tick_commander};
// The renderer needs the footprint SIZE and the cooldown key to draw a preview that agrees
// with the hit test. Exported rather than duplicated in GDScript — see view_state.rs.
pub use handlers::{COMMANDER_SMITE, COMMANDER_SMITE_CD, COMMANDER_SMITE_SIDE};
// The skill BAR is built from this catalog, so which skills exist — and how each asks for its
// target — is answered here rather than by a table in GDScript that could disagree.
pub use handlers::{CommanderSkillDef, CommanderTarget, COMMANDER_SKILLS};

pub use handlers::CONSUMABLE_SLOTS;

#[cfg(test)]
mod tests;

/// Who is allowed to AUTHOR an `InputEvent` — the security taxonomy behind
/// replay-verify. Replay proves "state follows from inputs" but NOT that the
/// inputs were legitimate, so safety reduces to: is every event safe to accept
/// from its claimed author? A server verifier uses this to police a client's log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAuthority {
    /// Client-authored + self-cost-gated: the sim enforces all costs/bounds, so
    /// replaying a forged one is a no-op or does the legal (paid) thing. Safe to
    /// accept from a player's log as-is.
    Client,
    /// Server-authored: changes a reward/economy parameter the client must not
    /// control (e.g. a global drop-rate event, a mail claim, a rolled grant).
    ///
    /// This tag is a CLASSIFICATION, not a guarantee, and reading it as one cost us a live
    /// mint hole. The client writes its own input log and computes the save hash by replaying
    /// it, so it can author any event here and the bundle still verifies — nothing about the
    /// tag stops it. What stops each variant is a SEPARATE check the verifier runs: a
    /// recomputed keyed nonce (`Grant*`, `StartRefine`), a reconcile against the real record
    /// (`ClaimMail`), or a flat rejection when neither exists (`SetCardRateBonus`).
    ///
    /// So a NEW variant here is unprotected until someone writes its check. verify-server's
    /// `server_event_is_checked` enumerates the verdict per variant and defaults to reject,
    /// and `every_server_authored_event_has_a_verdict` fails the build when this list grows.
    Server,
    /// Debug/dev only — must NEVER appear in a production log. A verifier rejects
    /// any log containing one.
    Debug,
}

/// One recorded input in the event log: the event plus the tick it fired at.
/// The save = (seed + initial state + `Vec<RecordedInput>`); a verifier replays
/// these in order onto the start snapshot and checks the hash (GDD §4.4). Ticks
/// are needed because a replay must advance the sim to each event's tick before
/// applying it (the tick evolution between events is deterministic + implicit).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordedInput {
    pub tick: u64,
    pub event: InputEvent,
}

/// serde skip predicate: a 0 `server_roll` (the offline/client-roll default) is
/// omitted so existing grant logs + goldens stay byte-identical.
fn is_zero_u64(n: &u64) -> bool {
    *n == 0
}

fn is_zero_u32(n: &u32) -> bool {
    *n == 0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InputEvent {
    /// Move the item at `entity_id.inventory[inventory_idx]` into the
    /// same entity's `equipped` list.
    ///
    /// Idempotent: out-of-range indices and unknown ids are silent
    /// no-ops (the replay engine will see them; they shouldn't crash).
    EquipItem { entity_id: u32, inventory_idx: u32 },

    /// Reverse of `EquipItem`: move `entity_id.equipped[equipped_slot]`
    /// back into the inventory. Same idempotency rules.
    UnequipItem { entity_id: u32, equipped_slot: u32 },

    /// FUSE six pieces of gear (inventory indices) into one of the next GRADE — the only way up
    /// the ladder that does not involve a drop. See `fusion::fuse`: six of ONE grade, any type,
    /// any tier; the output's tier is the inputs' average rounded by a biased coin, its type is
    /// drawn from the catalog, and the fee is paid whether it succeeds or not.
    ///
    /// CLIENT-authored: it spends the player's own items and Gil and mints nothing the drop
    /// tables could not, so there is nothing here for a server to sign.
    FuseGear {
        picks: [u32; crate::fusion::FUSE_INPUTS],
        /// A SERVER-issued roll (`fusion` module doc): non-zero reads the three outcomes out of
        /// the nonce with no client draw; zero is the client roll. The verifier requires a
        /// non-zero roll from `fusion::FUSE_SERVER_ROLL_FROM` up and recomputes it against the
        /// six products + attempt number (`fusion::binding`) — a forgery rejects the save.
        #[serde(default, skip_serializing_if = "is_zero_u64")]
        server_roll: u64,
    },

    /// Set the PARTY name (also mirrored to `character_name`). Recorded at character
    /// creation so the player-chosen name is an EVENT, not baked into the start snapshot —
    /// keeping the bundle's `start` the server-reproducible genesis (anchor stays valid).
    SetPartyName { name: String },

    /// Apply one STAR STONE `orb` (its catalog id — the stones live in the materials ledger
    /// since 0.0.364, not at a bag index) to `target` (stash gear or a worn piece).
    /// Validation/fees/draw order live in `orbs::use_orb`; anything invalid is a silent
    /// zero-draw no-op.
    /// RETIRED (0.0.381) — a no-op. The Star Stone system left the game; the variant stays because
    /// every save replays its whole input log.
    UseOrb { orb: String, target: crate::items::OrbTarget },

    /// RETIRED (0.0.317) — a no-op. The six attributes are gone, so there is
    /// nothing to raise. The variant stays because `InputEvent` is serialized and
    /// every save replays its whole input log.
    AllocateStatPoint { entity_id: u32, stat: StatKind },

    /// RETIRED (0.0.317) — a no-op, for the same reason as `AllocateStatPoint`.
    ResetStats { entity_id: u32 },

    /// Spend `count` unspent Apex Points into `bucket` (0..APEX_BUCKETS)
    /// on `entity_id`. Clamped to the unspent pool; bad index / no points /
    /// unknown entity = silent no-op (replay-safe). Authority Client (earned,
    /// replay-verified like stat allocation).
    AllocateApex { entity_id: u32, bucket: u8, count: u32 },

    /// Refund all spent Apex Points on `entity_id` back to the unspent pool
    /// (every bucket → 0). `apex_level` is untouched.
    ResetApex { entity_id: u32 },

    /// EXEMPLAR == UNIT (0.0.350). The PLAYER spends one banked Resonance credit on unit `unit_id`
    /// (benched or deployed), raising its stars by one. No credit / not an Exemplar / at cap =
    /// silent no-op. Client authority: the credit was banked upstream by a server grant.
    FeedExemplarUnit { unit_id: u32 },
    /// Dev / story stand-in for a server grant: hand the account Exemplar `exemplar_id` as a
    /// unit (or a Resonance credit if already owned). Debug authority.
    DebugGrantExemplarUnit { exemplar_id: String },

    /// XP REFUND (0.0.349): drain a BENCHED unit's base progression into bound Engram items
    /// (`owned_engrams`) and return it to Lv1. Client authority — it moves only the unit's own
    /// provable XP and mints nothing. No-op on a deployed / unknown / fresh unit.
    RefundUnitXp { unit_id: u32 },
    /// Spend `count` Engrams of `tier` (0..=3 → 1k/10k/100k/1M XP) on the BENCHED unit `unit_id`,
    /// capped at the account's proven ceiling (overflow discarded). Client authority; clamps to
    /// owned stock; no-op on a bad tier / no stock / deployed-or-unknown unit.
    ApplyEngram { unit_id: u32, tier: u8, count: u32 },
    /// "Fill to max": pour Engrams into the BENCHED unit up to the ceiling with the
    /// waste-minimising plan `engram_fill_preview` shows. Client authority. No-op when nothing
    /// would be spent.
    ApplyEngramsToMax { unit_id: u32 },

    /// Raise the level of a known skill by 1, spending one `skill_points`
    /// (earned from Job Level). No-op if: unknown entity/skill, the
    /// entity doesn't know it, no skill points, or already at max level.
    LevelSkill { entity_id: u32, skill_id: String },
    /// Deterministic, self-cost board progression. Every point debit is quoted in sim-core.
    UpgradePotential { entity_id: u32, node_id: String },
    MaxPotential { entity_id: u32, target_id: String },
    ResetPotential { entity_id: u32 },
    /// SIGIL CONVERSION (0.0.362) — `MergeSigil`: `count` × `SIGIL_MERGE` Sigil of `grade` (0-based)
    /// become `count` of the next grade; `SplitSigil`: `count` of `grade` become `count` ×
    /// `SIGIL_MERGE` of the grade below. `element` is the catalog id ("fire"). SELF-COST, lossless
    /// both ways, deterministic, no PRNG; a no-op on a bad element / grade / count or a short
    /// ledger. `InputAuthority::Client`.
    MergeSigil { element: String, grade: u8, count: u32 },
    SplitSigil { element: String, grade: u8, count: u32 },

    /// GEMS (0.0.386, `docs/gem-system-design.md`). `SocketGem`: one `gem` (a ledger def id,
    /// `gem_<set>_g<step>`) into socket `socket` of the piece at `target`, rolling its lines
    /// (one `state.rng` draw per stat). A filled socket is overwritten and the old gem is
    /// destroyed. No-op without a draw on any refusal (`gems::refusal`). `InputAuthority::Client`:
    /// self-cost, and the ±5% window is not worth steering.
    SocketGem { target: crate::gems::GemTarget, socket: u8, gem: String },
    /// `FuseGems`: `loot_rules.gems.merge` gem def ids of ONE grade (any sets) become one gem of
    /// the next grade, set drawn by weight (one draw). Client authority — a gem is not a piece
    /// of gear, and the only thing a steered draw could choose is the set.
    FuseGems { gems: Vec<String> },

    /// Set the PLAYER-CHOSEN cast level of a level-scalable skill (the mage bolts):
    /// it then fires at `level` (1..=learned) instead of the learned level — RO
    /// variable cast level (higher = more bolts but a longer channel). `0` resets
    /// to the learned level. No-op on an unknown entity/skill; clamped to learned.
    SetCastLevel { entity_id: u32, skill_id: String, level: u8 },

    /// Toggle farm mode. When `on`, clearing the instance's final room loops
    /// back to its room 1 (farm the same content) instead of advancing.
    SetFarmMode { on: bool },

    /// Raise a PATHS OF SOUL node by 1 level, spending `soul`. No-op if: unknown
    /// node, prerequisite unmet, already at max level, or not enough soul.
    LevelSoulPath { skill_id: String },

    /// Buy one TRAINING level of `slot_id` with Gil (0.0.406). No-op if: unknown slot, at cap,
    /// or not enough Gil.
    Train { slot_id: String },

    /// DISENCHANT the bag item at `inv_idx` into its category's crest material (0.0.416).
    /// No-op for anything that is not tier-3+ gear or a Rare+ gem, or a unique.
    Disenchant { inv_idx: u32 },

    /// Open the Sealed Cache at bag index `inv_idx` (0.0.449): the box leaves the bag and
    /// `Measures.lootboxes_opened` rises by one — that is ALL the sim does. What was inside is
    /// the SERVER's roll (crystal or Summon coupons, paid per open beyond its watermark, capped
    /// by a monthly server-wide quota per tier); nothing about the contents is client-authored.
    /// No-op unless the index holds a `lootbox`.
    OpenLootbox { inv_idx: u32 },

    /// Open the Farm Banner box at bag index `inv_idx` (0.0.479): the box leaves the bag and
    /// `GameState.farm_opened[def]` rises by one — all the sim does. The server turns each open
    /// into a bound ticket, pulled on the Farm banner (`/farm/roll`). No-op unless the index
    /// holds an item whose catalog def is a `farm_box` (and without the catalog).
    OpenFarmBox { inv_idx: u32 },

    /// Send ONE of the item at bag index `inv_idx` to Steam (0.0.478): it leaves the bag and
    /// `GameState.steam_sent[def_id]` rises by one; the server mints it as a Steam Inventory
    /// item. Only for an item whose catalog def carries a `steam_itemdef`, that is not bound.
    /// No-op otherwise (and without the catalog).
    SendToSteam { inv_idx: u32 },

    /// Lift crest `crest_id` one level, spending its tier's materials (0.0.416). No-op when
    /// short or capped.
    #[serde(alias = "UpgradeRelic")]
    UpgradeCrest { #[serde(alias = "relic_id")] crest_id: String },

    /// RETIRED (0.0.411): vendoring is gone — Gil comes from fusion. Kept so an old replay log
    /// still parses; a silent no-op.
    SellAllJunk,

    /// RETIRED (0.0.411), see `SellAllJunk`. (Renamed from `SellStashItem` at the
    /// stash→inventory rename; alias keeps old replay logs valid.)
    #[serde(alias = "SellStashItem")]
    SellInventoryItem {
        #[serde(alias = "stash_idx")]
        inv_idx: u32,
    },

    /// DISCARD one inventory item at `inv_idx` — remove the whole stack for ZERO
    /// Gil. This is the ONLY liquidation path a `bound` item accepts: it destroys
    /// the item without minting value or touching the market, so it can't launder
    /// (0 Gil in, nothing out). Lets a player clear bound clutter (pass consumables
    /// they'll never use). No-op on a bad index. Client-authored (destroying your
    /// own property creates nothing).
    DiscardItem {
        inv_idx: u32,
    },

    /// USE an Inventory Expander at `inv_idx`: permanently +1 bag slot
    /// (`inventory_cap_bonus`, capped at `MAX_INVENTORY_CAP_BONUS`), consuming one
    /// expander. No-op if the item isn't an expander or the cap is already maxed —
    /// then nothing is consumed. Client-authored (spends the player's own item).
    ExpandInventory {
        inv_idx: u32,
    },

    /// Slot a `Consumable` inventory item into `entity_id`'s consumable bar (cap 3),
    /// so it auto-uses on its trigger. Moves the whole stack out of the inventory.
    /// No-op if: unknown entity, bad index, item isn't a Consumable, or the bar
    /// is already full (3).
    AddConsumable {
        entity_id: u32,
        #[serde(alias = "stash_idx")]
        inv_idx: u32,
    },

    /// Remove the consumable in `entity_id`'s bar `slot`, returning it to the
    /// inventory. No-op on a bad entity / slot index.
    RemoveConsumable { entity_id: u32, slot: u32 },
    /// Move up to `qty` of a provision (a Consumable whose spec declares a `ProvisionKind`) from
    /// the inventory into the RESERVED slot for that kind. The rest of the stack STAYS in the
    /// bag: slots are per-character while stackables merge into one inventory stack, so moving
    /// the whole stack would let the first member take every Meal and leave the other four
    /// unable to eat.
    ///
    /// Topping up the SAME item merges into the slot; a DIFFERENT item of the same kind replaces
    /// it and the old stack returns to the bag.
    /// No-op if: unknown entity, bad index, not a Consumable, it declares no kind, or `qty` is 0.
    AddProvision { entity_id: u32, inv_idx: u32, qty: u32 },
    /// Return a reserved provision slot's item to the inventory.
    RemoveProvision { entity_id: u32, slot: u32 },

    /// Move up to `qty` of a PARTY provision from the inventory into the party rack, which buffs
    /// every living member (docs/party-buff-slots-design.md).
    ///
    /// No-op if: bad index, not a Consumable, its spec does not declare `ProvisionKind::Party`,
    /// `qty` is 0, the rack is full, or THE SAME ITEM IS ALREADY IN THE RACK. That last one is the
    /// design's only rule and it is enforced here rather than in the UI, because the sim decides.
    /// Topping the same slot up is done by removing and re-adding — a second copy is a duplicate.
    AddPartyProvision { inv_idx: u32, qty: u32 },
    /// Return a party rack slot's item to the inventory. No-op on a bad index.
    RemovePartyProvision { slot: u32 },

    /// Craft `recipe_id` from the catalog: consume its material ingredients + Gil, grant its
    /// batch output to the inventory. `InputAuthority::Client` (default) — SELF-COST: it only
    /// converts the player's own materials + Gil (bounded by supply), never a free reward.
    /// Deterministic; no-op if the recipe is unknown or an input is missing. See `crate::craft`.
    CraftItem { recipe_id: String },

    /// Learn `recipe_id` one-time: spend its `learn_soul` + `learn_gil` and add it to
    /// `recipes_learned` (party-shared, survives a wipe like `soul`). `InputAuthority::Client`
    /// — SELF-COST (spends the player's own soul + Gil). Deterministic; no-op if the recipe is
    /// unknown, already learned, or a currency is short. See `crate::craft`.
    LearnRecipe { recipe_id: String },

    /// RETIRED (0.0.490) — the in-game Gil market is gone. These four trade events are kept ONLY
    /// so old saves and input logs still deserialize; applying one is a no-op (a tombstone).
    ///
    /// Was: COMMODITY MARKET — list `qty` of a stackable `def_id` at `unit_price` Gil each.
    DepositCommodity { def_id: String, qty: u32, unit_price: u64 },

    /// RETIRED (0.0.490), tombstone. Was: COMMODITY MARKET — market-buy up to `qty` of `def_id`.
    BuyCommodity { def_id: String, qty: u32, max_price: u64 },

    /// RETIRED (0.0.490), tombstone. Was: ITEM MARKET — list the equipment at `inv_idx`.
    DepositItem { inv_idx: u32, price: u64 },
    /// RETIRED (0.0.490), tombstone. Was: ITEM MARKET — buy listing `listing_id`.
    BuyItemListing { listing_id: u64, max_price: crate::state::Gil },

    /// MAILBOX — claim mail `mail_id`, applying its server-authored payload (`gil` and/or a
    /// stackable `def_id`×`qty`) to GameState. `InputAuthority::Server` — the payload is issued
    /// by the server's mailbox; a client-forged payload is REJECTED at save-verify against the
    /// real mail record (like the refine nonce). FULL-INVENTORY GUARD: if the item needs a new
    /// slot and the inventory is full, the WHOLE claim is a no-op (Gil included) so nothing is
    /// lost — the mail stays claimable. Consumed only on a verified save (exactly-once). A
    /// stackable merging into an existing stack always fits. See `crate::mailbox_grant`.
    ClaimMail {
        mail_id: u64,
        #[serde(default, skip_serializing_if = "crate::state::Gil::is_zero")]
        gil: crate::state::Gil,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        def_id: String,
        #[serde(default, skip_serializing_if = "is_zero_u32")]
        qty: u32,
        /// A FULL item instance (item-market purchase / return), delivered intact. Distinct from the
        /// `def_id`×`qty` stackable path — an equipment piece carries its own stats/refine/stars, so
        /// it can't be rebuilt from a def id. `#[serde(skip_if_none)]` keeps commodity mail identical.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item: Option<crate::items::Item>,
    },

    /// Load ONE elemental quiver from the inventory at `inv_idx` into `entity_id`'s
    /// ammo pool (adds its `shots` rounds of that element). Consumes one quiver
    /// from the stack. No-op if: unknown entity, bad index, or the item isn't a
    /// quiver (a Consumable whose effect is `LoadAmmo`).
    LoadAmmo {
        entity_id: u32,
        #[serde(alias = "stash_idx")]
        inv_idx: u32,
    },

    /// Equip the Card at album `album_idx` into `entity_id`'s CARD PANEL. The panel is
    /// the card's `card_slot` class, EXCEPT an accessory card is generic and may target
    /// EITHER accessory panel via `target` (Necklace/Ring) — the chosen panel is
    /// stamped onto the card. `target` None ⇒ use the card's own class. Cards equip to the
    /// CHARACTER (persist across gear swaps; activation gated by the matching gear sockets).
    /// Cards live in the dex-style CARD ALBUM, not the gear stash → `album_idx` indexes it.
    /// No-op if: bad ids, not a Card, no class, target mismatch, or that panel is full.
    EquipCard {
        entity_id: u32,
        #[serde(alias = "stash_idx")]
        album_idx: u32,
        #[serde(default)]
        target: Option<crate::items::CardClass>,
    },

    /// Unequip the card at `panel_idx` from `entity_id`'s card panel back to the
    /// card album. No-op on bad id/index.
    UnequipCard { entity_id: u32, panel_idx: u32 },

    /// BIND the card at album `album_idx` into `entity_id` for a small PERMANENT
    /// stat bonus (instead of socketing). Escalating cost + cap: the Nth bind of
    /// a card type costs N cards (1, then 2, then 3); only 3 binds per type. No-op
    /// if: bad ids, not a card, no `bind_effect`, the type is maxed (3), or the
    /// album stack holds fewer than N cards. (Renamed from `ConsumeCard`; alias
    /// keeps old replay logs valid.)
    #[serde(alias = "ConsumeCard")]
    BindCard {
        entity_id: u32,
        #[serde(alias = "stash_idx")]
        album_idx: u32,
    },

    /// RETIRED (user 2026-08-12): the 3-hour claim loop it served is gone, and the back-log it
    /// reset — `band_clears` — went with it. A permanent no-op.
    ///
    /// KEPT ANYWAY, and not out of caution about deleting code. A save is `(seed + state +
    /// Vec<InputEvent>)`, so the variant is part of the REPLAY wire format: remove it and any log
    /// that still holds one fails to deserialize, taking the whole save with it rather than
    /// skipping one dead instruction. Nothing ever fired it — the client wrapper had no callers —
    /// so no log should contain one, but "should" is not a property a replay format gets to rely
    /// on. Costs one unreachable match arm.
    ResetBandClears,

    /// Set the minimum rarity routed to the rare bag (player preference). Drops
    /// at or above this land in the rare bag instead of the common stash.
    SetRareThreshold { rarity: crate::items::Rarity },

    /// Set the CARD drop-rate multiplier in basis points (10000 = ×1, 20000 = ×2;
    /// 0 clears it back to ×1). A RECORDED input so drop-rate EVENTS and balance
    /// tests replay bit-identically — the multiplier scales Card entry weights at
    /// roll time (`drops::apply_card_rate_bonus`), never the draw count.
    SetCardRateBonus { bp: u32 },

    /// Move the rare-bag item at `rare_idx` into the inventory (the player
    /// keeps it). Bypasses the inventory gear cap — a deliberate transfer, like
    /// unequip. No-op on a bad index. (Renamed from `MoveRareToStash`; alias
    /// keeps old logs valid.)
    #[serde(alias = "MoveRareToStash")]
    MoveRareToInventory { rare_idx: u32 },

    /// TOMBSTONE (0.0.201): the old dev-only free-grant Exemplar equip. Now a NO-OP —
    /// the variant stays so event logs recorded while it existed still deserialize
    /// (replay must survive), but it grants nothing. Real path: buy the Exemplar ITEM
    /// (shop / future server entitlement) → `EquipExemplar`.
    EquipExemplarTest {},

    /// Equip the OWNED Exemplar carrier at `owned_idx` onto party member `entity_id`
    /// (consumes the carrier; its `resonance_stars` carry the Resonance rank). Same
    /// Exemplar already equipped → MERGE: stars += 1 + the copy's carried stars, capped
    /// at 5 (a no-op at 5★ — the copy is NOT consumed). A different Exemplar → swap (the
    /// old one returns to the owned collection). No-op on bad ids / a non-Exemplar item.
    /// Since 0.0.220 carriers live in `owned_exemplars`, not the inventory (the old
    /// `inv_idx` field name is kept as a serde alias so pre-0.0.220 logs deserialize).
    EquipExemplar { entity_id: u32, #[serde(alias = "inv_idx")] owned_idx: u32 },

    /// Unequip `entity_id`'s Exemplar back into the owned collection (a carrier holding
    /// its `resonance_stars`). No-op if none equipped. `owned_exemplars` is uncapped, so
    /// an unequip never fails or destroys the carrier.
    UnequipExemplar { entity_id: u32 },

    /// RESONANCE feed (0.0.221) — mirrors `FeedCard` (minus foil). Consume the owned
    /// Exemplar at `feeder_idx` into `target_idx` (same def): target rank += 1 + the
    /// feeder's stars (copies conserved), cap 5; the feeder (which must be UNEQUIPPED)
    /// is removed. `InputAuthority::Client` — self-cost. No-op on bad indices, a def
    /// mismatch, a self-feed, an equipped feeder, or a capped target. See
    /// `handlers::feed_exemplar`.
    FeedExemplar { target_idx: u32, feeder_idx: u32 },

    /// Equip the owned Regalia at `owned_idx` onto `entity_id` (its OWN overlay slot, separate
    /// from the Exemplar). Twin of `EquipExemplar`. `InputAuthority::Client`.
    EquipRegalia { entity_id: u32, #[serde(alias = "inv_idx")] owned_idx: u32 },
    /// Unequip `entity_id`'s Regalia back into the owned collection. Twin of `UnequipExemplar`.
    UnequipRegalia { entity_id: u32 },
    /// RESONANCE feed for Regalia — twin of `FeedExemplar`. See `handlers::feed_regalia`.
    FeedRegalia { target_idx: u32, feeder_idx: u32 },
    /// RETIRED (0.0.489) — Enhance fed a card or a Dust carrier by index. Dust is a ledger count
    /// now (`EnhanceRegaliaDust`); kept so an old event log still parses. A no-op.
    EnhanceRegalia { target_idx: u32, feeder_idx: u32 },
    /// ENHANCE (Level axis, 0.0.489) — spend up to `count` of the ledger Dust `dust_id`
    /// (`regalia_fodder` / `regalia_sr_fodder`) on the Regalia at `target_idx`; never more than
    /// reaching `REGALIA_MAX_LEVEL` needs. See `handlers::enhance_regalia`.
    EnhanceRegaliaDust {
        target_idx: u32,
        dust_id: String,
        #[serde(default = "default_one_u32")]
        count: u32,
    },
    /// SALVAGE the owned Regalia at `owned_idx` into Memory Dust (`items::regalia_salvage_yield`).
    /// Refuses a worn or locked copy, and an SSR unless another copy of it is already S5.
    SalvageRegalia { owned_idx: u32 },
    /// Salvage every UNTOUCHED 3★ card (`items::regalia_bulk_salvageable`: 3★, Lv0, S1, unworn,
    /// unlocked). SR, SSR and any invested 3★ go one at a time through `SalvageRegalia`.
    SalvageRegaliaAll {},
    /// Set the player LOCK on the owned Regalia at `owned_idx`.
    LockRegalia { owned_idx: u32, locked: bool },

    /// SERVER-ROLL DROP grant (server-roll-drops-design.md): the server resolved
    /// pending rare-candidate `candidate_seq` as a HIT with nonce `server_roll` (duds
    /// are never sent — they expire by TTL). Applies the candidate's fixed item (the
    /// card) and removes the candidate; the FOIL bit derives from the nonce (server-
    /// authored, zero client draws). The verifier recomputes the nonce (issued? bound
    /// to this account+seq? passes the rate?) — a forgery rejects the save + flags.
    GrantDrop { candidate_seq: u64, server_roll: u64 },

    /// SERVER-ROLL CRAFT grant: the server resolved the pending `CandidateKind::Craft`
    /// candidate `candidate_seq` — the output of a gear recipe — into a star (1-4) and
    /// its bonus lines, both derived from `server_roll`. Unlike `GrantDrop` there is no
    /// rate to pass: the craft was PAID FOR, so every candidate is granted.
    ///
    /// A SEPARATE variant rather than a field on `GrantDrop`, and it consumes ONLY a
    /// `Craft` candidate, because the two have different validation rules. If one event
    /// carried both, a client could take the craft nonce it was legitimately issued for
    /// seq N and replay it against a forged history where seq N is a card candidate —
    /// buying a 1-in-200,000 card with a 30-material craft. The kind guard on both sides
    /// makes that a no-op.
    GrantCraft { candidate_seq: u64, server_roll: u64 },

    /// SERVER-ROLL GEAR grant (stage 3): the star, bonus lines and skill rune for a piece of
    /// equipment that has already dropped. Always granted — the drop happened, only its value
    /// was outstanding.
    ///
    /// A third variant for the same reason `GrantCraft` is a second one: each kind validates
    /// under its own nonce domain, and the verifier can only tell which rule applies from the
    /// event itself. Sharing one event would let a nonce issued for one kind be replayed
    /// against a forged history where that seq is another kind.
    GrantGear { candidate_seq: u64, server_roll: u64 },

    /// PRUNE adjudicated DROP candidates — the server confirming "I have judged every ticket
    /// up to `up_to_seq`; the ones I did not grant were duds, stop carrying them".
    ///
    /// The server evaluates EVERY pending candidate on EVERY save and reports only the hits
    /// (`GrantDrop`). Without this the client cannot tell a dud from an unjudged ticket, so it
    /// holds all of them for the full `RARE_CANDIDATE_TTL_TICKS` — a window measured in TICKS,
    /// which only advance while the game is open. A player who closes the game never drains
    /// anything, and the list sits at its steady state in every save, hash and replay.
    ///
    /// ORDER IS LOAD-BEARING: the same response carries the grants, and the client MUST apply
    /// those FIRST. A prune that runs first deletes the winning ticket before its grant can
    /// find it, and the item is gone.
    ///
    /// DROP candidates only. Craft and Gear tickets always pay and are removed by their own
    /// grants; pruning one would destroy an item the player has already earned.
    ///
    /// Recorded, not silent, for the same reason as `PruneMarketOps` — `rare_candidates` rides
    /// the state hash, so a local trim would desync from the anchor the server trusts.
    PruneCandidates { up_to_seq: u64 },

    /// PRUNE settled market ops — the server confirming "I have these, stop sending them".
    ///
    /// `market_ops` is append-only and was never trimmed, so a client re-presented its entire
    /// trading history on every save: the Vec rides the state hash, every save blob and every
    /// replay, and it only ever grew. It cannot be dropped locally without a recorded event,
    /// because it IS hashed — a silent trim would desync from the anchor the server trusts.
    ///
    /// Server authority, and it has to be: a save can be ACCEPTED with nothing settled (a
    /// tainted session), so "my push succeeded" does not mean "my ops were taken". Pruning on
    /// that assumption would drop a claim the server never recorded.
    ///
    /// Idempotent — an id already gone is simply not found. This Vec is a work queue, not history.
    PruneMarketOps { ids: Vec<u64> },

    /// STASH WAREHOUSE deposit/withdraw (player-driven banking). Move the
    /// inventory item at `inv_idx` into the `stash` warehouse; no-op on a bad
    /// index or a full warehouse (the item stays put — a deposit never destroys).
    DepositToStash { inv_idx: u32 },
    /// Move the warehouse item at `stash_idx` back into the inventory. No-op on a
    /// bad index or a full inventory.
    WithdrawFromStash { stash_idx: u32 },
    /// Move the CARD at inventory `inv_idx` into the card album. No-op if the slot
    /// isn't a Card, the index is bad, or the album is full.
    DepositCardToAlbum { inv_idx: u32 },
    /// Move the album card at `album_idx` back into the inventory. No-op on a bad
    /// index or a full inventory.
    WithdrawCardFromAlbum { album_idx: u32 },

    /// CARD STAR upgrade (0.0.214, gacha-style): feed `copies` duplicate cards
    /// from the album stack at `album_idx` into ONE surviving copy — +1 star per
    /// copy fed (flat; user re-spec from the 1/1/2/3/5 curve), capped at 5★.
    /// Same-stack identity only (def + foil + star — a Foil never feeds a plain
    /// upgrade). `InputAuthority::Client` — SELF-COST (burns the player's own
    /// cards). Deterministic; no-op if the index is bad, the item isn't a card,
    /// the star is capped, or the stack lacks copies+1. See
    /// `handlers::upgrade_card`.
    UpgradeCard {
        album_idx: u32,
        #[serde(default = "default_one_u32")]
        copies: u32,
    },

    /// CARD FEED (0.0.216, picker re-spec): consume ONE copy from the album
    /// stack at `feeder_idx` into the card at `target_idx` — same def, ANY
    /// star/foil (the picker shows every sibling stack; supersedes the
    /// 0★-same-foil-only `UpgradeCard`, kept for replay compat). Star gain =
    /// 1 + the feeder's star (a starred feeder returns every duplicate it
    /// absorbed — copies are conserved), capped at 5★; overflow is forfeit
    /// (the UI previews the waste). A FOIL feeder also converts the target to
    /// foil — feeding a foil into a 5★ plain card is legal as a pure foil
    /// conversion (no star gain). No-op when nothing would change (target 5★
    /// and no foil to transfer), on bad indices, non-cards, a def mismatch, or
    /// a self-feed without a spare copy. `InputAuthority::Client` — self-cost.
    /// See `handlers::feed_card`.
    FeedCard { target_idx: u32, feeder_idx: u32 },

    /// Toggle whether `entity_id`'s learned ACTIVE skill `skill_id` is enabled
    /// (slotted) for casting. Disabling a skill drops it from the combat
    /// rotation (e.g. a Fire skill in a Water zone). The loadout is stored as
    /// the enabled set, normalized to EMPTY when all actives are on (= the
    /// default "everything fires"). No-op if the entity doesn't know the skill,
    /// it isn't Active, or enabling would exceed `ACTIVE_SLOT_CAP`.
    ToggleActiveSkill { entity_id: u32, skill_id: String },

    /// COMMANDER SKILL `commander_smite` — a placed 2×2 blast, PURE damage, flat
    /// `5 × party average level` (`docs/commander-skills-design.md`).
    ///
    /// `x`/`y` is the cell the player POINTED AT, and for an even-sided footprint that cell is the
    /// TOP-LEFT of the block (user, 2026-08-21) — `grid::Blast::sided` owns that rule so the
    /// renderer's preview and this hit test cannot drift apart.
    ///
    /// Client authority and safe as one: it spends the player's own charge and sets the player's
    /// own cooldown, so a forged cell buys nothing a legal one does not. Out-of-bounds, no charge,
    /// or still on cooldown are all silent no-ops — never a panic, because a replay has to survive
    /// game_data version skew.
    ///
    /// Damage reads NO caster stat, so there is no channeller and no attribution: kills route
    /// through `kill_monster(.., None)`, exactly as a click-attack's do.
    CommanderSmite { x: i32, y: i32 },

    /// Player CLICK-ATTACK (PvE, single-player): a flat PURE hit on the monster
    /// `target_id`. Early-game active-play aid — strong vs low early HP, trivial
    /// late (flat damage vs scaling HP). Must be a recorded input so the
    /// 3-hour-loop reconcile replays it. No-op on unknown / dead / non-monster.
    ClickAttack { target_id: u32 },

    /// PLAYER SKILL `regent_hand` — PICK UP a party member (the player literally
    /// takes them off the board to dodge a telegraphed AoE). While held the
    /// character deals no damage, is untargetable, and no AoE reaches it; the hold
    /// auto-drops after `PICKUP_HOLD_MAX_TICKS`. No-op unless the current zone
    /// grants the skill (`player_skill_available`) and the target is a living,
    /// non-pet player past its pickup cooldown.
    PickUpMember { entity_id: u32 },

    /// PLAYER SKILL `regent_hand` — DROP the held member on `(x, y)`. The landing
    /// cell must be in bounds, free, and within `PICKUP_RANGE` of where they were
    /// picked up: that range cap is what keeps this a DODGE rather than free
    /// repositioning, and it is the sim's rule, not the client's. Landing costs a
    /// short act-lock and starts the pickup cooldown. No-op if the entity isn't
    /// held or the cell fails a check (the hold simply continues).
    PlaceMember { entity_id: u32, x: i32, y: i32 },

    /// Reset the live DPS meter (clear the per-class / per-skill totals and
    /// restart the timing window at the current tick). Display-only.
    ResetDps,
    /// BESTIARY: claim milestone `idx` (0..4) of kill-group `group`. Valid only
    /// when `monster_kills[group] >= BESTIARY_MILESTONES[idx].0` and the bit is
    /// unclaimed — else a silent no-op. Client-authored is SAFE: it can only
    /// realise value the deterministic kill tally already earned (0.0.185).
    ClaimBestiaryReward { group: String, idx: u8 },

    /// RETIRED (0.0.445): a tombstone — there is no class to swap to; a unit is its kit.
    /// Read and dropped so an old log parses.
    ChangeClass { unit_def: String },

    /// Re-enter the party's CURRENT zone (respawn / arena reset): clears
    /// monsters, refills HP, replaces the party at spawn. The loot escrow is
    /// KEPT (unlike a wipe or band-warp). DATA-DEPENDENT. No-op if there's no
    /// current zone or no catalog.
    RespawnInZone,

    /// Band picker: warp to room 1 of `band`'s instance (`zone_b{band}_1`).
    /// Clears the loot escrow (leaving the instance) and keeps XP/job/soul.
    /// DATA-DEPENDENT. No-op on `band` < 1, an unknown zone, or no catalog.
    SelectBand { band: u32 },

    /// Leave a dungeon and return to the zone the party entered from
    /// (`GameState.return_zone_id`). The DESTINATION is the sim's, not the client's:
    /// the event carries no zone id, so a forged one can only send you back where you
    /// came from. Also dismisses the clear summary. No-op with nothing to return to.
    LeaveInstance,

    /// DUNGEON picker: warp to room 1 of a STANDALONE instance by its zone id
    /// (`zone_dungeon_*` — off the main band chain). Twin of `SelectBand` but
    /// addressed by arbitrary `start_zone_id` instead of a band number, so a
    /// dungeon reachable only from the Dungeon menu can be entered. Clears the
    /// loot escrow (leaving the current instance) and keeps XP/job/soul.
    /// DATA-DEPENDENT. No-op on an unknown zone or no catalog.
    EnterInstance { start_zone_id: String },

    /// Restart the current instance at room 1 after a party WIPE (clears the
    /// escrow, keeps XP/job/soul — `handle_party_wipe` with `auto_retreat=false`;
    /// the sim-cli-only auto-retreat path is never event-driven). DATA-DEPENDENT.
    RestartInstance,

    /// RETIRED (0.0.413): a tombstone — recruiting for Gil is gone. Parses, does nothing.
    RecruitMember { unit_def: String },

    /// Buy the NEXT party slot with Gil (0.0.413, `party.slot_gil`). No-op at `MAX_PARTY` or
    /// short of Gil. The slot is the only thing Gil buys here — the member comes from gacha.
    BuyPartySlot,

    /// RETIRED (0.0.352): a tombstone — read and dropped so an old log parses.
    AdvanceClass { entity_id: u32, target_class_id: String },

    /// RETIRED (0.0.413): a tombstone — see `RecruitMember`.
    BuyCharacter { unit_def: String },

    /// Deploy the benched roster character `char_id` into the party. DATA-DEPENDENT.
    /// No-op if the party is full / id not in the roster (`data::deploy`).
    DeployCharacter { char_id: u32 },

    /// Bench the deployed party member `char_id` back to the roster. No-op if it's
    /// the last member / not deployed (`data::bench` — needs no catalog).
    BenchCharacter { char_id: u32 },

    /// Buy `qty` of shop item `def_id` for Gil (banks to the stash). DATA-DEPENDENT.
    /// No-op on unknown/not-for-sale item / not enough Gil (`data::buy_item`).
    BuyItem { def_id: String, qty: u32 },

    /// Grant loot item `def_id` into the stash (rare → rare bag). DATA-DEPENDENT.
    /// TRANSITIONAL: a remnant of the removed 3-hour claim flow — slated to become
    /// a flagged debug tool once the claim UI is gone (`data::grant_loot`).
    GrantLoot {
        def_id: String,
        /// Debug: grant the FOIL variant (default false; serde-default keeps old logs).
        #[serde(default)]
        foil: bool,
    },

    /// DEBUG: set whether the player OWNS a Commander skill (`state.commander_skills_owned`).
    /// `owned=true` inserts, `false` removes; idempotent either way. `InputAuthority::Debug` — a
    /// STAND-IN for the real unlock trigger (Commander level → a server-authored grant) while that
    /// is unbuilt, so the gate can be play-tested offline; a verifier rejects any production log
    /// carrying it. The durable half — the owned set, the smite gate, the bar filter — is shared
    /// with the eventual real grant; only this authoring path is throwaway.
    /// See `docs/commander-skills-design.md`.
    SetCommanderSkillOwned { skill_id: String, owned: bool },

    /// PRODUCTION unlock of a Commander skill (into `state.commander_skills_owned`): the server issues
    /// this when the account CLAIMS the level reward that unlocks the skill (accept-to-unlock, not
    /// autogrant — `docs/commander-level-and-outpost.md`). `InputAuthority::Server`; the `nonce` is a
    /// keyed blake3 MAC over `(secret, account_id, skill_id)` that verify-server recomputes
    /// (`commander_grants_valid`) — a client cannot forge one. The sim NEVER reads the nonce (it is
    /// total: it just inserts, idempotently); the nonce exists only for the verifier. The DEBUG twin
    /// `SetCommanderSkillOwned` above is the offline test path.
    GrantCommanderSkill {
        skill_id: String,
        #[serde(default, skip_serializing_if = "is_zero_u64")]
        nonce: u64,
    },
}

impl InputEvent {
    /// Who may legitimately author this event (see [`InputAuthority`]). A server
    /// verifier rejects `Debug` events in any production log and treats `Server`
    /// events as server-truth (replays with its own granted value). Everything
    /// else is `Client` — self-cost-gated, safe to accept as-is.
    ///
    /// NOTE: the catch-all defaults to `Client`. Any NEW event that grants a reward
    /// for free, amplifies drops, or moves economy value MUST be listed here as
    /// `Server`/`Debug` — otherwise it defaults to client-authorable (a hole).
    pub fn authority(&self) -> InputAuthority {
        match self {
            // Global reward multiplier (double-drop game event) — server-issued.
            InputEvent::SetCardRateBonus { .. } => InputAuthority::Server,
            // Fusion: a non-zero `server_roll` is a server-rolled outcome (the verifier recomputes
            // the bound nonce and rejects a forgery); zero is the client roll, valid only below
            // `fusion::FUSE_SERVER_ROLL_FROM` — the verifier's bind-check refuses a zero above it.
            InputEvent::FuseGear { server_roll, .. } => {
                if *server_roll != 0 { InputAuthority::Server } else { InputAuthority::Client }
            }
            // Free arbitrary item grant — 3h-claim remnant, dev-only.
            InputEvent::GrantLoot { .. } => InputAuthority::Debug,
            // Debug stand-in for the real Commander-level unlock trigger — dev/offline only.
            InputEvent::SetCommanderSkillOwned { .. } => InputAuthority::Debug,
            // Dev / story stand-in for a server Exemplar grant.
            InputEvent::DebugGrantExemplarUnit { .. } => InputAuthority::Debug,
            // PRODUCTION grant — server-authored, guarded by the keyed nonce verify-server recomputes.
            // A 0 nonce is the benign no-op default (the sim does not apply it): Client-authored and
            // harmless, so it is never mistaken for an unverifiable Server event.
            InputEvent::GrantCommanderSkill { nonce, .. } => {
                if *nonce != 0 { InputAuthority::Server } else { InputAuthority::Client }
            }
            // Test Exemplar equip — dev-only until the real entitlement path exists.
            InputEvent::EquipExemplarTest {} => InputAuthority::Debug,
            // Server-authored rare grant. Authorable in a client's log like anything else here;
            // what makes it safe is the keyed nonce the verifier recomputes.
            InputEvent::GrantDrop { .. } => InputAuthority::Server,
            // Same: the star + bonus lines on a crafted weapon are the server's roll.
            InputEvent::GrantCraft { .. } => InputAuthority::Server,
            // Star + bonus lines decide what a dropped piece is WORTH — never the client's.
            InputEvent::GrantGear { .. } => InputAuthority::Server,
            // Only the server knows which ops it actually settled — see the variant's doc.
            InputEvent::PruneMarketOps { .. } => InputAuthority::Server,
            // Only the server knows which tickets it has judged.
            InputEvent::PruneCandidates { .. } => InputAuthority::Server,
            // Mailbox claim mints a server-authored payload into GameState → the verifier
            // rejects a client-forged payload against the real mail record (docs/l1-commodity-
            // market-scope.md Slice 2b). DepositCommodity is self-cost → Client (the default).
            InputEvent::ClaimMail { .. } => InputAuthority::Server,
            _ => InputAuthority::Client,
        }
    }
}

/// Data-FREE entry point: for the 60+ call sites (tests, soul_paths, drops)
/// that never issue a data-dependent event. Data-dependent variants
/// (`ChangeClass`, future `ChangeZone`/recruit/buy) are silent no-ops here —
/// route them through `apply_input_with` with the catalog.
fn default_one_u32() -> u32 {
    1
}

pub fn apply_input(state: &mut GameState, event: InputEvent) {
    apply_input_with(state, None, event);
}

/// Longest party name `SetPartyName` keeps.
pub const PARTY_NAME_MAX_CHARS: usize = 24;

/// Full entry point: `data` is the catalog needed to resolve class/item/zone
/// defs for data-dependent events. The bridge (and the server replay verifier)
/// always pass `Some(data)`; data-free events ignore it. Keeping ONE enum and
/// ONE dispatch preserves the single-event-log replay model (GDD §4.4).
pub fn apply_input_with(state: &mut GameState, data: Option<&GameData>, event: InputEvent) {
    // The name is cut BEFORE it is recorded (audit 2026-10-03): cut only in the handler, a huge
    // name still rode the input log into every save bundle. Same result on replay either way.
    let event = match event {
        InputEvent::SetPartyName { name } if name.chars().count() > PARTY_NAME_MAX_CHARS => {
            InputEvent::SetPartyName { name: name.chars().take(PARTY_NAME_MAX_CHARS).collect() }
        }
        e => e,
    };
    // S1: record the input BEFORE dispatch (with the tick it fires at) so the
    // event log replays bit-identically. Gated so headless tools don't accumulate
    // it. Every bridge mutation funnels through here (S0), so this one hook catches
    // the whole save log — no per-action recording to forget.
    if state.record_inputs {
        state.input_log.push(RecordedInput { tick: state.tick, event: event.clone() });
    }
    match event {
        InputEvent::EquipItem { entity_id, inventory_idx } => {
            equip_item(state, entity_id, inventory_idx)
        }
        InputEvent::UnequipItem { entity_id, equipped_slot } => {
            unequip_item(state, entity_id, equipped_slot)
        }
        InputEvent::FuseGear { picks, server_roll } => {
            if let Some(d) = data {
                let _ = crate::fusion::fuse(state, d, &picks, server_roll);
            }
        }
        InputEvent::SetPartyName { name } => {
            // Bounded (audit 2026-10-02): the name is hashed state, so an unbounded one let a
            // client grow every save at will.
            let name: String = name.chars().take(PARTY_NAME_MAX_CHARS).collect();
            state.party_name = name.clone();
            state.character_name = name;
        }
        // Retired in 0.0.381 — read and dropped, so an old save's input log still replays to the
        // same state (the stones themselves are gone from the ledger on load, so there was never
        // anything a replay could have spent).
        InputEvent::UseOrb { .. } => {}
        // Both retired in 0.0.317 — read and dropped, so an old save's input log
        // still replays to the same state.
        InputEvent::AllocateStatPoint { .. } | InputEvent::ResetStats { .. } => {}
        InputEvent::AllocateApex { entity_id, bucket, count } => {
            let Some(e) = state.grid.entity_of_id_mut(entity_id) else { return; };
            e.stats.allocate_apex(bucket as usize, count);
        }
        // A free respec, available at any time, returning BOTH tiers' points to their pools
        // (0.0.347). The rule lives in `reset_apex` so the refund path and this input cannot drift.
        InputEvent::ResetApex { entity_id } => {
            let Some(e) = state.grid.entity_of_id_mut(entity_id) else { return; };
            e.stats.reset_apex();
        }
        InputEvent::FeedExemplarUnit { unit_id } => crate::data::feed_exemplar_unit(state, unit_id),
        InputEvent::DebugGrantExemplarUnit { exemplar_id } => {
            if let Some(d) = data {
                let _ = crate::data::grant_exemplar_unit(state, d, crate::items::current_unit_id(&exemplar_id));
            }
        }
        InputEvent::RefundUnitXp { unit_id } => crate::data::refund_unit_xp(state, unit_id),
        InputEvent::ApplyEngram { unit_id, tier, count } => {
            crate::data::apply_engram(state, unit_id, tier as usize, count)
        }
        InputEvent::ApplyEngramsToMax { unit_id } => crate::data::apply_engrams_to_max(state, unit_id),
        InputEvent::LevelSkill { entity_id, skill_id } => {
            level_skill(state, entity_id, &skill_id);
        }
        InputEvent::UpgradePotential { entity_id, node_id } => {
            crate::potential::actions::upgrade(state, entity_id, &node_id);
        }
        InputEvent::MaxPotential { entity_id, target_id } => {
            crate::potential::actions::upgrade_max(state, entity_id, &target_id);
        }
        InputEvent::ResetPotential { entity_id } => {
            crate::potential::actions::reset(state, entity_id);
        }
        InputEvent::MergeSigil { element, grade, count } => {
            crate::potential::actions::merge_sigil(state, &element, grade, count);
        }
        InputEvent::SocketGem { target, socket, gem } => {
            if let Some(d) = data {
                let _ = crate::gems::socket(state, d, target, socket, &gem);
            }
        }
        InputEvent::FuseGems { gems } => {
            if let Some(d) = data {
                let _ = crate::gems::fuse(state, d, &gems);
            }
        }
        InputEvent::SplitSigil { element, grade, count } => {
            crate::potential::actions::split_sigil(state, &element, grade, count);
        }
        InputEvent::SetCastLevel { entity_id, skill_id, level } => {
            if let Some(e) = state.grid.entity_of_id_mut(entity_id) {
                if let Some(s) = e
                    .skills
                    .iter_mut()
                    .find(|s| s.id == skill_id)
                {
                    // Clamp to what's learned; 0 stays 0 (→ cast at learned level).
                    s.cast_level = level.min(s.level);
                }
            }
        }
        InputEvent::LevelSoulPath { skill_id } => {
            level_soul_path(state, &skill_id);
        }
        InputEvent::Train { slot_id } => {
            if let Some(d) = data {
                crate::training::train(state, d, &slot_id);
            }
        }
        InputEvent::OpenLootbox { inv_idx } => {
            let idx = inv_idx as usize;
            let emptied = match state.inventory.get_mut(idx) {
                Some(it) if it.def_id == crate::drops::LOOTBOX_ID => {
                    it.quantity = it.quantity.saturating_sub(1);
                    state.measures.lootboxes_opened.add(1);
                    it.quantity == 0
                }
                _ => false,
            };
            if emptied {
                state.inventory.remove(idx);
            }
        }
        InputEvent::OpenFarmBox { inv_idx } => {
            let idx = inv_idx as usize;
            let Some(d) = data else { return };
            let emptied = match state.inventory.get_mut(idx) {
                Some(it) if d.item(&it.def_id).is_some_and(|def| def.farm_box) => {
                    it.quantity = it.quantity.saturating_sub(1);
                    *state.farm_opened.entry(it.def_id.clone()).or_insert(0) += 1;
                    it.quantity == 0
                }
                _ => false,
            };
            if emptied {
                state.inventory.remove(idx);
            }
        }
        InputEvent::SendToSteam { inv_idx } => {
            let idx = inv_idx as usize;
            let Some(d) = data else { return };
            let emptied = match state.inventory.get_mut(idx) {
                Some(it) if !it.bound && d.item(&it.def_id).is_some_and(|def| def.steam_itemdef.is_some()) => {
                    it.quantity = it.quantity.saturating_sub(1);
                    *state.steam_sent.entry(it.def_id.clone()).or_insert(0) += 1;
                    it.quantity == 0
                }
                _ => false,
            };
            if emptied {
                state.inventory.remove(idx);
            }
        }
        InputEvent::Disenchant { inv_idx } => {
            if let Some(d) = data {
                crate::crest::disenchant(state, d, inv_idx as usize);
            }
        }
        InputEvent::UpgradeCrest { crest_id } => {
            if let Some(d) = data {
                crate::crest::upgrade(state, d, &crest_id);
            }
        }
        InputEvent::SetFarmMode { on } => {
            state.farm_current_instance = on;
        }
        InputEvent::SellAllJunk => {
            sell_all_junk(state);
        }
        InputEvent::SellInventoryItem { inv_idx } => {
            sell_inventory_item(state, inv_idx as usize);
        }
        InputEvent::DiscardItem { inv_idx } => {
            discard_inventory_item(state, inv_idx as usize);
        }
        InputEvent::ExpandInventory { inv_idx } => {
            expand_inventory(state, inv_idx as usize);
        }
        InputEvent::AddConsumable { entity_id, inv_idx } => {
            add_consumable(state, entity_id, inv_idx as usize);
        }
        InputEvent::AddProvision { entity_id, inv_idx, qty } => {
            add_provision(state, entity_id, inv_idx as usize, qty);
        }
        InputEvent::AddPartyProvision { inv_idx, qty } => {
            add_party_provision(state, inv_idx as usize, qty);
        }
        InputEvent::RemovePartyProvision { slot } => {
            remove_party_provision(state, slot as usize);
        }
        InputEvent::RemoveProvision { entity_id, slot } => {
            remove_provision(state, entity_id, slot as usize);
        }
        InputEvent::LoadAmmo { entity_id, inv_idx } => {
            load_ammo(state, entity_id, inv_idx as usize);
        }
        InputEvent::RemoveConsumable { entity_id, slot } => {
            remove_consumable(state, entity_id, slot as usize);
        }
        InputEvent::ClickAttack { target_id } => {
            crate::tick::click_strike(state, target_id, 10)
        }
        InputEvent::CommanderSmite { x, y } => commander_smite(state, x, y),
        InputEvent::PickUpMember { entity_id } => pick_up_member(state, entity_id),
        InputEvent::PlaceMember { entity_id, x, y } => place_member(state, entity_id, x, y),
        InputEvent::ResetDps => state.reset_dps(),
        InputEvent::ClaimBestiaryReward { group, idx } => {
            // `idx` is client-authored, so the bounds check and the read are ONE expression —
            // a separate `len()` guard is a thing a later edit can move away from its use.
            let Some(&(need, gil, soul)) = crate::state::BESTIARY_MILESTONES.get(idx as usize)
            else {
                return;
            };
            // READ first, insert only on the success path. `entry(group).or_insert(0)` used to run
            // BEFORE the guard, so a claim that failed still left a key behind — and `group` is an
            // arbitrary client string, so 50,000 invented ones (MAX_LOG_LEN) fit in a single save
            // and `bestiary_claimed` accumulates across saves with no bound. Not a mint; a state
            // the client can grow at will, carried in every future bundle and hashed into every
            // future state_hash.
            //
            // Written this way the kills check IS the allowlist: `monster_kills` only ever holds
            // groups the sim itself recorded on a real kill, so an invented string reads 0 and can
            // never reach the insert. Same answer as STORY_MISSIONS, with the sim as the authority
            // instead of a constant.
            let kills = state.monster_kills.get(&group).copied().unwrap_or(0);
            let claimed = state.bestiary_claimed.get(&group).copied().unwrap_or(0);
            let bit = 1u8 << idx;
            if kills < need || claimed & bit != 0 { return; }
            state.bestiary_claimed.insert(group, claimed | bit);
            state.earn_gil(gil, GilEarn::Quest);
            state.earn_soul(soul, crate::state::SoulEarn::Bestiary);
        }
        InputEvent::ToggleActiveSkill { entity_id, skill_id } => {
            toggle_active_skill(state, entity_id, &skill_id);
        }
        InputEvent::EquipCard { entity_id, album_idx, target } => {
            equip_card(state, entity_id, album_idx as usize, target);
        }
        InputEvent::UnequipCard { entity_id, panel_idx } => {
            unequip_card(state, entity_id, panel_idx as usize);
        }
        InputEvent::BindCard { entity_id, album_idx } => {
            bind_card(state, entity_id, album_idx as usize);
        }
        InputEvent::ResetBandClears => {
            // Retired; see the variant's doc for why it is still here.
        }
        InputEvent::SetRareThreshold { rarity } => {
            state.rare_threshold = rarity;
        }
        InputEvent::SetCardRateBonus { bp } => {
            // Bound the global multiplier. A client CAN author this event — authority() is a
            // label, not a gate — so the online defence is the verify server rejecting the
            // bundle; this clamp is what keeps the damage finite everywhere else.
            state.card_rate_bonus_bp = bp.min(crate::drops::MAX_CARD_RATE_BONUS_BP);
        }
        InputEvent::MoveRareToInventory { rare_idx } => {
            move_rare_to_inventory(state, rare_idx as usize);
        }
        InputEvent::DepositToStash { inv_idx } => {
            deposit_to_stash(state, inv_idx as usize);
        }
        InputEvent::WithdrawFromStash { stash_idx } => {
            withdraw_from_stash(state, stash_idx as usize);
        }
        InputEvent::DepositCardToAlbum { inv_idx } => {
            deposit_card_to_album(state, inv_idx as usize);
        }
        InputEvent::UpgradeCard { album_idx, copies } => {
            handlers::upgrade_card(state, album_idx as usize, copies);
        }
        InputEvent::FeedCard { target_idx, feeder_idx } => {
            handlers::feed_card(state, target_idx as usize, feeder_idx as usize);
        }
        InputEvent::WithdrawCardFromAlbum { album_idx } => {
            withdraw_card_from_album(state, album_idx as usize);
        }
        InputEvent::RespawnInZone => {
            // Not on a CLEARED zone (audit 2026-10-02): re-entering a cleared final room replayed
            // its boss and its clear without the rooms before it.
            let cleared = state.zone_spawn.as_ref().is_some_and(|z| z.zone_cleared);
            if let (Some(d), false) = (data, cleared) {
                if let Some(zid) = state.current_zone_id.clone() {
                    let _ = crate::data::change_zone(state, d, &zid);
                }
            }
        }
        InputEvent::SelectBand { band } => {
            if let Some(d) = data {
                // GATE (0.0.295): up to REACHED + 1, never further. This input is
                // `InputAuthority::Client` and its only check used to be `band >= 1`, so a
                // level-1 party could type 20 and stand in the endgame. Replay-verify cannot
                // catch that — the sim accepted it, so the server agrees. An unauthored input
                // has to be refused HERE or nowhere.
                //
                // The ceiling takes the CURRENT ZONE as well as the stored high-water mark,
                // because standing in a band is proof of having reached it. That is what keeps
                // saves written before this field existed from being locked out of where they
                // already are: they default to 0 and the zone speaks for them.
                // The frontier is the band after the highest CLEARED one (audit 2026-10-02):
                // `reached + 1` chained — SelectBand 2, 3, … walked to any band in one tick.
                if band >= 1 && band <= state.band_travel_max() {
                    crate::drops::clear_escrow(state);
                    let _ = crate::data::change_zone(state, d, &format!("zone_b{band}_1"));
                }
                // Out of range is a SILENT no-op, like every other rejected input here: an
                // `apply_input` that panics or errors would break replay of a save that
                // contains one (`apply_input` for invalid input is a no-op — GDD §4.4).
            }
        }
        InputEvent::EnterInstance { start_zone_id } => {
            // A STAGE start is the same travel as SelectBand and takes the same gate (audit
            // 2026-10-02: every `zone_b<N>_1` is an instance start, so this input walked to any
            // band). Dungeon / story chains keep their own rules.
            let stage_too_far = crate::state::band_of_zone(&start_zone_id)
                .is_some_and(|b| u32::from(b) > state.band_travel_max());
            // A DUNGEON opens with progression (user 2026-10-03): its first room's stage must be
            // within reach, like a stage start. Story chains are gated by the previous chapter,
            // which the server and the story screen own, not by stage.
            let dungeon_too_far = start_zone_id.starts_with("zone_dungeon_")
                && data.and_then(|d| d.zone(&start_zone_id))
                    .is_some_and(|z| u32::from(z.stage) > state.band_travel_max());
            let stage_too_far = stage_too_far || dungeon_too_far;
            // A chain is entered at its FIRST room (audit 2026-10-02): any room was accepted, so a
            // party could warp straight into a boss room and loop the boss and its clear.
            let not_a_start = data.is_some_and(|d| {
                d.zone(&start_zone_id).map_or(true, |z| z.instance_start_zone_id != start_zone_id)
            });
            if let (Some(d), false, false) = (data, stage_too_far, not_a_start) {
                // Remember the way back BEFORE we travel — but only when leaving a
                // DIFFERENT instance chain, so re-entering the same dungeon (or the
                // debug shortcut into a later room) can't overwrite the overworld zone
                // we actually came from with a dungeon room.
                let here = state.current_zone_id.clone().unwrap_or_default();
                let same_chain = match (d.zone(&here), d.zone(&start_zone_id)) {
                    (Some(a), Some(b)) => {
                        !a.instance_start_zone_id.is_empty()
                            && a.instance_start_zone_id == b.instance_start_zone_id
                    }
                    _ => false,
                };
                if !here.is_empty() && !same_chain {
                    state.return_zone_id = here;
                }
                crate::drops::clear_escrow(state);
                let _ = crate::data::change_zone(state, d, &start_zone_id);
            }
        }
        InputEvent::LeaveInstance => {
            if let Some(d) = data {
                // The recorded way back, or the band-10 overworld as a last resort. There should
                // ALWAYS be a return point; the fallback exists so a missing one can never strand
                // the party in a room with no exit, which is how this first went wrong.
                // The fallback is the party's own reached band, never a fixed one (audit 2026-10-02:
                // "zone_b10_1" let a brand-new account walk to band 10 — slots, CXP, offline EXP).
                let back = state.return_zone_id.clone();
                // Back to the START of the chain it left (audit 2026-10-03): returning to the exact
                // room let a party stand in a boss room, step into another chain and leave — a fresh
                // boss and a fresh clear every time.
                let back = d.zone(&back).map_or(back.clone(), |z| {
                    if z.instance_start_zone_id.is_empty() { back.clone() } else { z.instance_start_zone_id.clone() }
                });
                let dest = if back.is_empty() { format!("zone_b{}_1", state.band_reached.max(1)) } else { back };
                if crate::data::change_zone(state, d, &dest).is_ok() {
                    state.return_zone_id.clear();
                }
                // DISMISS UNCONDITIONALLY. The travel is best-effort — with no return point on
                // record (entering deeper into a chain you are already inside never records one,
                // because the only candidate would be another dungeon room) the sim genuinely
                // cannot know where to send the party, and the world map is how they get out.
                //
                // But the SCREEN must always close: the client hides it and re-reads state on the
                // next frame, so a summary left in place re-shows it every frame. Tying the
                // dismissal to a successful zone change turned "could not travel" into an
                // UNCLOSABLE clear screen — found by play-test 2026-07-31.
                state.last_clear = None;
            }
        }
        InputEvent::RestartInstance => {
            if let Some(d) = data {
                // RETRY is a player choice, not a defeat. `handle_party_wipe` bumps
                // `consecutive_wipes`, which drives sim-cli's auto-retreat model — counting a
                // deliberate re-run as "the party is walled here" would skew that measurement,
                // and it is plainly wrong after a successful clear.
                let before = state.consecutive_wipes;
                let _ = crate::data::handle_party_wipe(state, d);
                state.consecutive_wipes = before;
                state.last_clear = None;
            }
        }
        // TOMBSTONE (0.0.413) — recruiting for Gil is retired: characters come from gacha. Read
        // and dropped so an old log parses.
        InputEvent::RecruitMember { .. } => {}
        // TOMBSTONE (0.0.490) — party slots open by progress (`data::open_earned_slots`), not Gil.
        InputEvent::BuyPartySlot => {}
        // TOMBSTONES — class advancement (0.0.352) and class swap (0.0.445) are retired with the
        // class system: a unit is its fixed kit. Read and dropped so an old log parses.
        InputEvent::AdvanceClass { .. } | InputEvent::ChangeClass { .. } => {}
        // TOMBSTONE (0.0.413) — see `RecruitMember`.
        InputEvent::BuyCharacter { .. } => {}
        InputEvent::DeployCharacter { char_id } => {
            if let Some(d) = data {
                let _ = crate::data::deploy(state, d, char_id);
            }
        }
        InputEvent::BenchCharacter { char_id } => {
            // `bench` needs no catalog — works on either entry point.
            let _ = crate::data::bench(state, char_id);
        }
        InputEvent::BuyItem { def_id, qty } => {
            if let Some(d) = data {
                let _ = crate::data::buy_item(state, d, &def_id, qty);
            }
        }
        InputEvent::GrantCommanderSkill { skill_id, nonce } => {
            // A 0 nonce is a benign NO-OP (a dropped/missing field or the serde default), never an
            // unlock — this matches the verifier, which skips a 0 nonce rather than flagging a
            // forgery, so a data hiccup cannot desync the sim from an accepted save. A real grant
            // carries a non-zero server nonce; the sim inserts idempotently and leaves the MAC to
            // verify-server (`commander_grants_valid`).
            if nonce != 0 {
                state.commander_skills_owned.insert(skill_id);
            }
        }
        InputEvent::SetCommanderSkillOwned { skill_id, owned } => {
            if owned {
                state.commander_skills_owned.insert(skill_id);
            } else {
                state.commander_skills_owned.remove(&skill_id);
            }
        }
        InputEvent::GrantLoot { def_id, foil } => {
            if let Some(d) = data {
                if foil {
                    if let Some(def) = d.item(&def_id) {
                        let mut it = crate::data::instantiate_item(def);
                        it.foil = true;
                        crate::drops::bank_loot_drop(state, it);
                    }
                } else {
                    let _ = crate::data::grant_loot(state, d, &def_id);
                }
            }
        }
        // TOMBSTONE — the retired free-grant dev event. Kept parseable for old event
        // logs; grants nothing.
        InputEvent::EquipExemplarTest {} => {}
        // TOMBSTONES (0.0.350) — the Exemplar CARRIER flow is retired: an Exemplar is a unit, so
        // there is nothing to equip, unequip or merge. Read and dropped so an old event log
        // still parses; an old save's carriers become units/credits in `migrate_save_items`.
        InputEvent::EquipExemplar { .. }
        | InputEvent::FeedExemplar { .. }
        | InputEvent::UnequipExemplar { .. } => {}
        InputEvent::EquipRegalia { entity_id, owned_idx } => {
            handlers::equip_regalia(state, entity_id, owned_idx as usize);
        }
        InputEvent::UnequipRegalia { entity_id } => {
            handlers::unequip_regalia(state, entity_id);
        }
        InputEvent::FeedRegalia { target_idx, feeder_idx } => {
            handlers::feed_regalia(state, target_idx as usize, feeder_idx as usize);
        }
        InputEvent::EnhanceRegalia { .. } => {}
        InputEvent::EnhanceRegaliaDust { target_idx, dust_id, count } => {
            handlers::enhance_regalia(state, target_idx as usize, &dust_id, count);
        }
        InputEvent::SalvageRegalia { owned_idx } => {
            handlers::salvage_regalia(state, owned_idx as usize);
        }
        InputEvent::SalvageRegaliaAll {} => {
            handlers::salvage_regalia_all(state);
        }
        InputEvent::LockRegalia { owned_idx, locked } => {
            if let Some(it) = state.owned_regalia.get_mut(owned_idx as usize) {
                it.locked = locked;
            }
        }
        InputEvent::GrantDrop { candidate_seq, server_roll } => {
            if let Some(d) = data {
                handlers::grant_drop(state, d, candidate_seq, server_roll);
            }
        }
        InputEvent::GrantCraft { candidate_seq, server_roll } => {
            if let Some(d) = data {
                handlers::grant_craft(state, d, candidate_seq, server_roll);
            }
        }
        InputEvent::GrantGear { candidate_seq, server_roll } => {
            if let Some(d) = data {
                handlers::grant_gear(state, d, candidate_seq, server_roll);
            }
        }
        InputEvent::PruneCandidates { up_to_seq } => {
            state.rare_candidates.retain(|c| !c.kind.is_drop() || c.seq > up_to_seq);
        }
        InputEvent::PruneMarketOps { ids } => {
            if !ids.is_empty() {
                state.market_ops.retain(|op| !ids.contains(&op.id()));
            }
        }
        InputEvent::CraftItem { recipe_id } => {
            if let Some(d) = data {
                crate::craft::craft_item(state, d, &recipe_id);
            }
        }
        InputEvent::LearnRecipe { recipe_id } => {
            if let Some(d) = data {
                crate::craft::learn_recipe(state, d, &recipe_id);
            }
        }
        // THE IN-GAME GIL MARKET IS CANCELLED (user 2026-09-16; closed at the source 0.0.490): the
        // four trade events are tombstones, so no Deposit / Buy / DepositItem / BuyItem op can
        // enter `market_ops` and the server only ever settles mail claims. The server replays
        // with this sim, so a modified client cannot author a trade op either. The trade
        // functions that used to run here were deleted with the server's market ledgers.
        InputEvent::DepositCommodity { .. }
        | InputEvent::BuyCommodity { .. }
        | InputEvent::DepositItem { .. }
        | InputEvent::BuyItemListing { .. } => {}
        InputEvent::ClaimMail { mail_id, gil, def_id, qty, item } => {
            if let Some(d) = data {
                crate::mailbox_grant::claim_mail(state, d, mail_id, gil, &def_id, qty, item);
            }
        }
    }
}

/// What a junk sale would fetch — always 0 since vendoring was retired (0.0.411). Kept for the
/// bridges that still ask.
pub fn pending_junk_gil(_state: &GameState) -> u64 {
    0
}
