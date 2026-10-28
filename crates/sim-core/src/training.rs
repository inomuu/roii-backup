//! TRAINING (ฝึกฝน, 0.0.406) — the party-wide Gil ladders. `docs/training-system-plan.md`;
//! the model is WoG's Training window (`docs/wog-training-study.md`).
//!
//! One slot per stat. A slot has a level 0..`max_level`; buying the next level costs
//! `base_gil × cost_mult × cost_curve[level]` and raises the stat by `per_level`, linear. No
//! PRNG, no materials, no failure — Gil in, stat out. It is the second party-wide meta layer
//! beside Paths of Soul: Soul answers "which way does the build go", Gil answers "where does the
//! money go once the party is recruited" (the only Gil sink that never ends).
//!
//! The levels live in `GameState.training` (party-shared: a swapped lead keeps them). The stat
//! reaches combat as `Entity.training_mods`, rewritten every tick from the levels, so every
//! `StatMod` consumer folds it for free and a save from before the system reads correctly the
//! moment it loads. `StashCap` is the one non-StatMod slot: `GameState::inventory_cap` reads it.
//!
//! CONSERVATIVE BY DESIGN (user 2026-09-17): every percentage multiplies FIVE members where
//! WoG's multiplies one, so the per-level values are half WoG's and the caps lower.

use crate::data::{GameData, TrainingRules, TrainingSlot};
use crate::grid::Element;
use crate::items::StatMod;
use crate::state::GameState;
use serde::{Deserialize, Serialize};

/// What a slot raises. Variant order append-only (serialized in the catalog).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainStat {
    AtkPct,
    HpPct,
    DefPct,
    Crit,
    CritDmg,
    BossDmg,
    MoveSpeed,
    Cdr,
    HpRegen,
    BreakDmg,
    /// `+% damage with attacks of `element` — the Attunement slots.
    ElementDmg,
    /// `+% resistance to `element` — the Ward slots.
    ResistElement,
    /// +stash slots. Not a StatMod: `GameState::inventory_cap` adds it directly.
    StashCap,
    // Appended 0.0.417 for the twenty crests — every stat the vocabulary already prices.
    BreakBuildup,
    ChargeRate,
    IgnoreDef,
    Dodge,
    /// Damage taken from RANGED attackers −%.
    DrVsRanged,
    Heal,
    AtkFlat,
    HpFlat,
    DefFlat,
    // Appended 0.0.433 — the T3 crests are ECONOMY, not combat (user: "xp gain / gil gain /
    // soul gain / drop rate มาใส่เลเวลแรกของทุกอัน").
    XpGain,
    GilGain,
    SoulGain,
    DropRate,
    // Appended 0.0.447 — the flat early-game line (user: "Training น่าจะเพิ่ม flat stat หน่อย
    // ช่วงต้นเกมมันไม่ค่อยได้ใช้"): damage taken −N after everything.
    DmgReduceFlat,
}

/// Stash slots per Satchel level. Fixed here rather than in the slot's `per_level` because
/// `inventory_cap` has no catalog in hand.
pub const STASH_SLOTS_PER_LEVEL: u32 = 1;

pub fn slot<'a>(data: &'a GameData, id: &str) -> Option<&'a TrainingSlot> {
    data.training.slots.iter().find(|s| s.id == id)
}

pub fn level(state: &GameState, id: &str) -> u8 {
    state.training.get(id).copied().unwrap_or(0)
}

/// Gil for buying level `next` (1-based) of `slot`, `None` when the curve or the slot's cap
/// ends before it. Integer throughout: `base × mult/10000 × curve/10000`, in that order, so a
/// small base still steps by the curve.
pub fn cost(rules: &TrainingRules, slot: &TrainingSlot, next: u8) -> Option<u64> {
    if next == 0 || next > slot.max_level {
        return None;
    }
    let curve = *rules.cost_curve_bp.get(usize::from(next) - 1)?;
    let base = slot.base_gil.saturating_mul(u64::from(slot.cost_mult_bp)) / 10_000;
    Some(base.saturating_mul(curve) / 10_000)
}

/// The next level's cost for `id` as the state stands — `None` at cap / unknown slot.
pub fn next_cost(state: &GameState, data: &GameData, id: &str) -> Option<u64> {
    let s = slot(data, id)?;
    let next = level(state, id).checked_add(1)?;
    cost(&data.training, s, next)
}

/// Every slot's level summed — what the breadth gate reads.
pub fn total_levels(state: &GameState) -> u32 {
    state.training.values().map(|v| u32::from(*v)).sum()
}

/// The party-wide total the breadth gate wants before `id` can leave its CURRENT level —
/// `None` when the slot is not standing on a gate step (or there is no gate).
pub fn gate_total_needed(rules: &TrainingRules, lv: u8) -> Option<u32> {
    if rules.gate_step == 0 || lv == 0 || lv % rules.gate_step != 0 {
        return None;
    }
    Some(rules.gate_total_per_step.saturating_mul(u32::from(lv / rules.gate_step)))
}

/// Whether the breadth gate lets `id` go up right now.
pub fn gate_open(state: &GameState, data: &GameData, id: &str) -> bool {
    match gate_total_needed(&data.training, level(state, id)) {
        Some(need) => total_levels(state) >= need,
        None => true,
    }
}

/// Buy one level. Silent no-op on any refusal (replay must survive a catalog without the slot).
pub fn train(state: &mut GameState, data: &GameData, id: &str) -> bool {
    let Some(gil) = next_cost(state, data, id) else { return false };
    if state.gil() < gil || !gate_open(state, data, id) {
        return false;
    }
    state.spend_gil(gil, crate::state::GilSpend::Training);
    let lv = level(state, id) + 1;
    state.training.insert(id.to_string(), lv);
    state.measures.training_levels.add(1);
    resolve(state, data);
    true
}

impl TrainStat {
    /// The StatMod this stat is worth at `v` (its own unit), `None` for the non-StatMod
    /// `StashCap`. Shared by Training and the Crests (0.0.416), which price the same vocabulary.
    pub fn stat_mod(self, element: Option<Element>, v: i32) -> Option<StatMod> {
        let el = element.unwrap_or(Element::Physical);
        Some(match self {
            TrainStat::AtkPct => StatMod::AtkBp(v),
            TrainStat::HpPct => StatMod::MaxHpBp(v),
            TrainStat::DefPct => StatMod::DefBp(v),
            TrainStat::Crit => StatMod::CritBp(v),
            TrainStat::CritDmg => StatMod::CritDmgBp(v),
            TrainStat::BossDmg => StatMod::DmgVsBossBp(v),
            TrainStat::MoveSpeed => StatMod::MoveSpeedBp(v),
            TrainStat::Cdr => StatMod::CooldownBp(v),
            TrainStat::HpRegen => StatMod::HpRegenFlat(v),
            TrainStat::BreakDmg => StatMod::BreakDmgBp(v),
            TrainStat::ElementDmg => StatMod::ElementDmgBp(el, v),
            TrainStat::ResistElement => StatMod::ResistElement(el, v),
            TrainStat::StashCap => return None,
            TrainStat::BreakBuildup => StatMod::BreakBuildupBp(v),
            TrainStat::ChargeRate => StatMod::ChargeRateBp(v),
            TrainStat::IgnoreDef => StatMod::IgnoreDefBp(v),
            TrainStat::Dodge => StatMod::DodgeBp(v),
            TrainStat::DrVsRanged => StatMod::DrVsRanged(v),
            TrainStat::Heal => StatMod::HealBp(v),
            TrainStat::AtkFlat => StatMod::AtkFlat(v),
            TrainStat::HpFlat => StatMod::MaxHp(v),
            TrainStat::DefFlat => StatMod::DefFlat(v),
            TrainStat::XpGain => StatMod::XpGainBp(v),
            TrainStat::GilGain => StatMod::GilGainBp(v),
            TrainStat::SoulGain => StatMod::SoulGainBp(v),
            TrainStat::DropRate => StatMod::DropRateBp(v),
            TrainStat::DmgReduceFlat => StatMod::DmgReduceFlat(v),
        })
    }
}

/// The StatMod a slot at `level` is worth (`None` for `StashCap` and level 0).
pub fn slot_mod(s: &TrainingSlot, level: u8) -> Option<StatMod> {
    if level == 0 {
        return None;
    }
    s.stat.stat_mod(s.element, s.per_level.saturating_mul(i32::from(level)))
}

/// Every bought slot as StatMods, in catalog order. Empty when nothing is bought.
pub fn mods(state: &GameState, data: &GameData) -> Vec<StatMod> {
    if state.training.is_empty() {
        return Vec::new();
    }
    data.training
        .slots
        .iter()
        .filter_map(|s| slot_mod(s, level(state, &s.id)))
        .collect()
}

/// Extra stash slots from every `StashCap` slot bought.
pub fn stash_bonus(state: &GameState, data: &GameData) -> u32 {
    data.training
        .slots
        .iter()
        .filter(|s| matches!(s.stat, TrainStat::StashCap))
        .map(|s| u32::from(level(state, &s.id)) * STASH_SLOTS_PER_LEVEL)
        .sum()
}

/// RESOLVE the levels against the catalog into `GameState.training_mods` /
/// `training_stash_slots`. Called where a catalog is in hand — every `train` and the load-time
/// migration — because `tick` has none; `apply` then copies the resolved lines onto the party
/// every tick. A catalog re-tune reaches an old save at its next load.
pub fn resolve(state: &mut GameState, data: &GameData) {
    state.training_mods = mods(state, data);
    state.training_stash_slots = stash_bonus(state, data);
}

/// Copy the resolved lines onto every player each tick (dead ones too, as the Soul-path auras
/// do), so a recruit or a swapped lead carries them without an event having to remember.
pub fn apply(state: &mut GameState) {
    if state.training_mods.is_empty() && state.grid.entities.iter().all(|e| e.training_mods.is_empty()) {
        return;
    }
    let mods = state.training_mods.clone();
    for e in state.grid.entities.iter_mut() {
        if !matches!(e.team, crate::grid::Team::Player) || e.is_pet || e.owner_id != 0 {
            continue;
        }
        if e.training_mods != mods {
            e.training_mods = mods.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> TrainingRules {
        TrainingRules {
            cost_curve_bp: vec![10_000, 40_000, 76_000],
            gate_step: 0,
            gate_total_per_step: 0,
            slots: vec![
                TrainingSlot { id: "might".into(), stat: TrainStat::AtkPct, element: None, per_level: 100, max_level: 3, base_gil: 50, cost_mult_bp: 10_000 },
                TrainingSlot { id: "fire_ward".into(), stat: TrainStat::ResistElement, element: Some(Element::Fire), per_level: 100, max_level: 5, base_gil: 50, cost_mult_bp: 5_000 },
                TrainingSlot { id: "satchel".into(), stat: TrainStat::StashCap, element: None, per_level: 1, max_level: 2, base_gil: 50, cost_mult_bp: 10_000 },
            ],
        }
    }

    fn data() -> GameData {
        let mut d = crate::data::GameData::from_json_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json"))).unwrap();
        d.training = rules();
        d
    }

    #[test]
    fn the_curve_prices_each_level_and_ends_the_ladder() {
        let r = rules();
        let s = &r.slots[0];
        assert_eq!(cost(&r, s, 1), Some(50));
        assert_eq!(cost(&r, s, 2), Some(200));
        assert_eq!(cost(&r, s, 3), Some(380));
        assert_eq!(cost(&r, s, 4), None, "past max_level");
        let w = &r.slots[1];
        assert_eq!(cost(&r, w, 1), Some(25), "×0.5 slot");
        assert_eq!(cost(&r, w, 4), None, "the curve is shorter than max_level — it caps");
    }

    #[test]
    fn training_spends_gil_raises_the_level_and_refuses_when_short() {
        let d = data();
        let mut s = crate::data::new_game_in_zone(7, &d, "zone_b1_1").unwrap();
        s.set_gil_for_test(629);
        assert!(train(&mut s, &d, "might"));
        assert!(train(&mut s, &d, "might"));
        assert_eq!(level(&s, "might"), 2);
        assert_eq!(s.gil(), 629 - 50 - 200);
        assert!(!train(&mut s, &d, "might"), "380 > 379 left");
        assert!(!train(&mut s, &d, "nope"), "unknown slot is a no-op");
        assert_eq!(s.measures.training_levels.get(), 2);
        assert_eq!(mods(&s, &d), vec![StatMod::AtkBp(200)]);
    }

    #[test]
    fn the_breadth_gate_holds_a_slot_on_its_step_until_the_party_total_catches_up() {
        let mut d = data();
        d.training.cost_curve_bp = vec![10_000; 20];
        d.training.gate_step = 2;
        d.training.gate_total_per_step = 5; // leaving Lv2 needs a total of 5, Lv4 needs 10
        let mut s = crate::data::new_game_in_zone(7, &d, "zone_b1_1").unwrap();
        s.set_gil_for_test(1_000_000);
        assert!(train(&mut s, &d, "might"));
        assert!(train(&mut s, &d, "might"));
        assert!(!gate_open(&s, &d, "might"), "Lv2 with a total of 2 < 5");
        assert!(!train(&mut s, &d, "might"));
        assert_eq!(gate_total_needed(&d.training, 2), Some(5));
        assert_eq!(gate_total_needed(&d.training, 3), None);
        assert!(train(&mut s, &d, "fire_ward"));
        assert!(train(&mut s, &d, "fire_ward"));
        assert!(!train(&mut s, &d, "fire_ward"), "the ward stands on Lv2 too, total 4 < 5");
        assert!(train(&mut s, &d, "satchel"));
        assert_eq!(total_levels(&s), 5);
        assert!(train(&mut s, &d, "might"), "the total reached 5 → Lv3");
        assert_eq!(level(&s, "might"), 3);
    }

    #[test]
    fn a_level_reaches_the_party_as_a_stat_mod_and_the_satchel_as_slots() {
        let d = data();
        let mut s = crate::data::new_game_in_zone(7, &d, "zone_b1_1").unwrap();
        s.set_gil_for_test(10_000);
        let before = s.inventory_cap();
        assert!(train(&mut s, &d, "fire_ward"));
        assert!(train(&mut s, &d, "satchel"));
        apply(&mut s);
        let lead = s.grid.entities.iter().find(|e| matches!(e.team, crate::grid::Team::Player)).unwrap();
        assert_eq!(lead.training_mods, vec![StatMod::ResistElement(Element::Fire, 100)]);
        assert_eq!(crate::combat::stat_mods::resist_element_bp(lead, Element::Fire), 100);
        assert_eq!(s.inventory_cap(), before + 1);
    }
}
