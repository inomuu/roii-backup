//! ONBOARDING (user 2026-09-29/30), modelled for a headless run: the lead starts alone, the sim
//! opens each party slot on its milestone (`data::open_earned_slots`: slots 2/3/4/5 on reaching
//! stage 2/6/18/24), and each opening pays what the
//! draft says — a fixed unit or N pulls on the newcomer (Starter) banner at the live rates: the
//! standard odds with the SSR guaranteed by the 50th pull. A pulled unit
//! the account does not own joins the queue; queued units fill free slots. Pull RNG is sim-cli's
//! own, so the sim's draw order is untouched.

use sim_core::data::{self, GameData};
use sim_core::grid::Team;
use sim_core::rng::Pcg32;
use sim_core::state::GameState;

use crate::gacha;

/// What opening party slot 2, 3, 4, 5 pays.
enum SlotReward {
    Unit(&'static str),
    Pulls(u32),
}

const FLOW: [SlotReward; 4] = [
    SlotReward::Unit("lumi"),
    SlotReward::Pulls(10),
    SlotReward::Pulls(10),
    SlotReward::Pulls(10),
];

pub struct GachaFlow {
    rng: Pcg32,
    pity: gacha::PityState,
    queue: Vec<String>,
    owned: std::collections::BTreeSet<String>,
    slots_bought: usize,
    weapon_given: bool,
    pub pulls: u32,
    pub units_pulled: u32,
    pub log: Vec<String>,
}

impl GachaFlow {
    pub fn new(seed: u64, lead: &str, quest_weapon: bool) -> Self {
        let mut owned = std::collections::BTreeSet::new();
        owned.insert(lead.to_string());
        GachaFlow {
            rng: Pcg32::new(seed),
            pity: gacha::PityState::default(),
            queue: Vec::new(),
            owned,
            slots_bought: 0,
            weapon_given: !quest_weapon,
            pulls: 0,
            units_pulled: 0,
            log: Vec::new(),
        }
    }

    fn gain(&mut self, unit: &str) -> bool {
        if self.owned.insert(unit.to_string()) {
            self.queue.push(unit.to_string());
            true
        } else {
            false
        }
    }

    fn pull(&mut self, n: u32) -> Vec<String> {
        let banner = gacha::banner("newcomer").expect("newcomer banner");
        let rates = gacha::rates_for(banner.kind);
        let mut got = Vec::new();
        for p in gacha::roll_batch(&rates, &mut self.pity, &mut self.rng, n) {
            self.pulls += 1;
            let def = gacha::starter_payout(banner, &p, &mut self.rng, &self.owned);
            if let Some(unit) = def.strip_prefix("exemplar_") {
                self.units_pulled += 1;
                if self.gain(unit) {
                    got.push(unit.to_string());
                } else {
                    got.push(format!("{unit}(dupe)"));
                }
            }
        }
        got
    }
}

fn party_size(state: &GameState) -> usize {
    state.grid.entities.iter().filter(|e| matches!(e.team, Team::Player) && !e.is_pet).count()
}

/// TUTORIAL QUEST 1 (user 2026-09-30, server-paid in the game): the first boss kill (room 1-1)
/// pays a Rare-grade s1 weapon of the lead's family. Modelled here as banked + equipped at once.
fn first_boss_weapon(state: &mut GameState, data: &GameData) {
    let Some((eid, ex, unit)) = state.grid.entities.iter()
        .find(|e| matches!(e.team, Team::Player) && !e.is_pet)
        .map(|e| (e.id, e.exemplar.as_ref().map(|x| x.id.clone()), e.unit_def.clone()))
    else { return };
    let Some(&wt) = sim_core::items::weapon_types_for_character(ex.as_deref(), &unit).first() else { return };
    let Some(mut it) = data.items.iter()
        .filter(|d| d.weapon_type == wt)
        .map(data::instantiate_item)
        .find(|it| it.item_level() == 1)
    else { return };
    let overrides: Vec<sim_core::data::GearOverride> = data.gear_overrides.iter().filter(|o| o.def_id == it.def_id).cloned().collect();
    let lines = sim_core::substats::lines_by_grade(&data.loot_rules.substats, &it.def_id, it.slot());
    it.assign_grade(sim_core::items::Grade::Rare, &overrides, &lines);
    it.bound = true;
    state.inventory.push(it);
    let idx = state.inventory.len() as u32 - 1;
    sim_core::events::apply_input_with(state, Some(data), sim_core::events::InputEvent::EquipItem { entity_id: eid, inventory_idx: idx });
}

/// One pass: fill free slots from the queue, then pay each slot the sim has opened since.
pub fn step(state: &mut GameState, data: &GameData, flow: &mut GachaFlow) {
    if !flow.weapon_given && state.measures.boss_kills.get() >= 1 {
        flow.weapon_given = true;
        first_boss_weapon(state, data);
        let min = state.tick / 600;
        flow.log.push(format!("{min:>5} min  quest 1: Rare weapon"));
    }
    loop {
        while party_size(state) < state.party_slots() && !flow.queue.is_empty() {
            let unit = flow.queue.remove(0);
            let ok = match data::grant_exemplar_unit(state, data, &unit) {
                Some(char_id) => data::deploy(state, data, char_id).is_ok(),
                None => data::model_add_member(state, data, &unit).is_ok(),
            };
            let min = state.tick / 600;
            if !ok {
                flow.log.push(format!("{min:>5} min  {unit} could NOT join (party {})", party_size(state)));
                break;
            }
            flow.log.push(format!("{min:>5} min  {unit} joins (party {})", party_size(state)));
        }
        let Some(reward) = FLOW.get(flow.slots_bought) else { return };
        if state.party_slots() <= flow.slots_bought + 1 {
            return;
        }
        flow.slots_bought += 1;
        let min = state.tick / 600;
        let slot = flow.slots_bought + 1;
        let got = match reward {
            SlotReward::Unit(u) => {
                flow.gain(u);
                vec![u.to_string()]
            }
            SlotReward::Pulls(n) => flow.pull(*n),
        };
        flow.log.push(format!("{min:>5} min  slot {slot} opens -> {got:?}"));
    }
}
