//! The live skills whose behaviour depends on WHERE things stand, under the lane rules
//! (`docs/lane-combat-design.md`). Every number (power, cooldown, radius, field hits, taunt
//! length) is read from `sim_core::skills::skill_def`, so a retune in the live catalog reaches the
//! trial. Skills that do not care about position are not modelled here yet.
//!
//! Lane rules (user 2026-10-04):
//! - AoE shapes stay as live: radius / 3×3 / 5×5 / field, even though 3 lanes means they cover
//!   every lane.
//! - Iron Chains pulls from any position: its targets land next to Seraphine wherever she stands,
//!   ignoring the gap rule and the stacking cap, and stay locked onto her.
//! - Abyssal Embrace is centred on a FIXED cell: the middle lane, the first enemy-side cell in
//!   front of the front column. It pulls everything in its radius onto that cell, ignoring the cap.
//! - Frost Taunt (and the Iron Chains lock): a taunted monster crosses the gap rule to its taunter.
//!
//! A cast takes the unit's swing slot (`atk_cd`), like the live one action timeline. Ultimates
//! (no cooldown: the live sim gates them on the Limit Break gauge) do NOT fire here until that
//! gauge is modelled.

use sim_core::combat;
use sim_core::grid::{DamageType, Entity};
use sim_core::skills::{skill_def, SkillDef};

use crate::{cheb, Battle, Event, Team, LANES, WIDTH};

pub const LANE_SKILLS: &[&str] = &[
    "iron_chains", "steel_break", "frost_taunt", "fireball", "tolling_blow", "sunder",
    "passage_of_no_return", "storm_field", "velvet_bog", "eclipse_rain", "abyssal_embrace",
];

#[derive(Clone, Debug)]
pub struct LaneSkill {
    pub id: &'static str,
    pub level: u8,
    pub cd: u32,
}

impl LaneSkill {
    fn interval(def: &SkillDef) -> u32 {
        def.cooldown_ticks
    }
}

/// A placed damage field: waves of `power` hit every surface monster within `radius` of its centre.
#[derive(Clone, Debug)]
pub struct Field {
    pub skill: &'static str,
    pub owner: u32,
    caster: Box<Entity>,
    pub x: i32,
    pub lane: i32,
    pub radius: i32,
    hits_left: u32,
    every: u32,
    next_in: u32,
    power: u32,
    kind: DamageType,
}

/// The lane-relevant skills a live entity carries, cooldowns started at half (a fight does not open
/// with every skill at once).
pub fn kit_of(ent: &Entity) -> Vec<LaneSkill> {
    ent.skills.iter().filter_map(|s| {
        let def = skill_def(&s.id)?;
        // An ultimate (cooldown 0) waits on the Limit Break gauge, which the trial does not model.
        (LANE_SKILLS.contains(&def.id) && def.cooldown_ticks > 0)
            .then_some(LaneSkill { id: def.id, level: s.level, cd: def.cooldown_ticks / 2 })
    }).collect()
}

/// One skill hit through the live formula: `build_damage_scaled` off ATK, `apply_damage` with the
/// target's mob count, crit as its expected value.
fn skill_blow(attacker: &Entity, target: &Entity, mob: u32, power: u32, kind: DamageType) -> i64 {
    let hit = |crit| i64::from(combat::apply_damage(target, &combat::build_damage_scaled(attacker, crit, power, kind, true), mob));
    let c = i64::from(combat::effective_crit_chance_bp(attacker).min(10_000));
    ((hit(false) * (10_000 - c) + hit(true) * c) / 10_000).max(1)
}

impl Battle {
    fn surface_monsters(&self) -> Vec<usize> {
        self.live(Team::Monster).filter(|&m| self.units[m].under == 0).collect()
    }

    fn skill_hit(&mut self, skill: &'static str, caster: &Entity, caster_id: u32, v: usize, power: u32, kind: DamageType) {
        let Some(ve) = self.units[v].ent.as_deref() else { return };
        let dmg = skill_blow(caster, ve, self.units[v].engaged, power, kind);
        if let Some(a) = self.units.iter_mut().find(|u| u.id == caster_id) {
            a.dealt += dmg;
        }
        *self.skill_dmg.entry(skill).or_insert(0) += dmg;
        let vu = &mut self.units[v];
        vu.hp -= dmg;
        let to = vu.id;
        self.events.push((self.tick, Event::Attack { from: caster_id, to, dmg }));
        if vu.hp <= 0 && vu.alive {
            vu.alive = false;
            self.events.push((self.tick, Event::Died { id: to }));
        }
    }

    /// The first enemy-side cell in front of the formation's front column, in the middle lane.
    fn abyss_point(&self) -> (i32, i32) {
        let front = self.live(Team::Party).filter_map(|p| self.units[p].home.map(|h| h.0)).max().unwrap_or(8);
        ((front + 1).min(WIDTH - 1), 1)
    }

    /// Try every ready lane skill of party unit `i`; true when one was cast (the swing is spent).
    pub(crate) fn try_cast(&mut self, i: usize) -> bool {
        if self.units[i].atk_cd > 0 || self.units[i].ent.is_none() {
            return false;
        }
        let ready: Vec<usize> = self.units[i].kit.iter().enumerate().filter(|(_, s)| s.cd == 0).map(|(k, _)| k).collect();
        for k in ready {
            let (id, level) = (self.units[i].kit[k].id, self.units[i].kit[k].level);
            let Some(def) = skill_def(id) else { continue };
            if self.cast(i, def, level) {
                self.units[i].kit[k].cd = LaneSkill::interval(def);
                self.units[i].atk_cd = self.units[i].stats.atk_interval;
                self.events.push((self.tick, Event::Cast { id: self.units[i].id, skill: def.id }));
                return true;
            }
        }
        false
    }

    fn cast(&mut self, i: usize, def: &'static SkillDef, level: u8) -> bool {
        let Some(caster) = self.units[i].ent.as_deref().cloned() else { return false };
        let caster_id = self.units[i].id;
        let power = def.power_at(level);
        let kind = def.damage_type.unwrap_or(caster.atk_type);
        let (cx, cl) = (self.units[i].x, self.units[i].lane);
        let mons = self.surface_monsters();

        // Fields (Velvet Bog, Eclipse Rain, Abyssal Embrace).
        if def.ground_field_radius > 0 {
            let r = def.ground_field_radius;
            let (fx, fl) = if def.id == "abyssal_embrace" {
                let (px, pl) = self.abyss_point();
                if !mons.iter().any(|&m| cheb(px, pl, self.units[m].x, self.units[m].lane) <= r + 2) {
                    return false;
                }
                // Everything in the radius is hauled onto the point, past the stacking cap.
                for &m in &mons {
                    let mu = &self.units[m];
                    if cheb(px, pl, mu.x, mu.lane) <= r && !self.occupants(px, pl, Some(m)).any(|o| self.units[o].team == Team::Party) {
                        self.units[m].x = px;
                        self.units[m].lane = pl;
                    }
                }
                (px, pl)
            } else {
                let in_range: Vec<usize> = mons.iter().copied().filter(|&m| self.dist(i, m) <= def.range.max(1)).collect();
                let best = in_range.iter().copied().max_by_key(|&m| {
                    let (x, l) = (self.units[m].x, self.units[m].lane);
                    let n = mons.iter().filter(|&&o| cheb(x, l, self.units[o].x, self.units[o].lane) <= r).count();
                    (n, std::cmp::Reverse(self.dist(i, m)), std::cmp::Reverse(self.units[m].id))
                });
                let Some(b) = best else { return false };
                (self.units[b].x, self.units[b].lane)
            };
            let hits = def.field_hits.max(1) + def.field_hits_per_level * u32::from(level.saturating_sub(1));
            let every = (def.field_duration_ticks / hits).max(1);
            self.fields.push(Field {
                skill: def.id, owner: caster_id, caster: Box::new(caster), x: fx, lane: fl, radius: r,
                hits_left: hits, every, next_in: 0, power, kind,
            });
            return true;
        }

        // Caster-centred (Iron Chains, Steel Break, Frost Taunt, Tolling Blow, Sunder, Passage).
        if def.self_aoe_radius > 0 {
            let r = def.self_aoe_radius;
            let mut victims: Vec<usize> = mons.iter().copied().filter(|&m| self.dist(i, m) <= r).collect();
            if victims.is_empty() {
                return false;
            }
            if def.pull > 0 {
                // Iron Chains: the farthest in the ring, hauled next to the caster from wherever she stands.
                victims.sort_by_key(|&m| (std::cmp::Reverse(self.dist(i, m)), self.units[m].id));
                if def.self_aoe_max_targets > 0 {
                    victims.truncate(def.self_aoe_max_targets as usize);
                }
                for &m in &victims {
                    let side = if self.units[m].traits.rear { -1 } else { 1 };
                    let spots = [(cx + side, cl), (cx + side, cl - 1), (cx + side, cl + 1), (cx, cl - 1), (cx, cl + 1), (cx - side, cl), (cx - side, cl - 1), (cx - side, cl + 1)];
                    let spot = spots.iter().copied().find(|&(x, l)| {
                        (0..WIDTH).contains(&x) && (0..LANES).contains(&l) && !self.occupants(x, l, Some(m)).any(|o| self.units[o].team == Team::Party)
                    });
                    if let Some((x, l)) = spot {
                        self.units[m].x = x;
                        self.units[m].lane = l;
                    }
                }
            } else if def.self_aoe_max_targets > 0 {
                victims.sort_by_key(|&m| (self.dist(i, m), self.units[m].id));
                victims.truncate(def.self_aoe_max_targets as usize);
            }
            if def.taunt_on_hit_ticks > 0 {
                let until = self.tick + u64::from(def.taunt_on_hit_ticks);
                for &m in &victims {
                    self.units[m].taunted = Some((caster_id, until));
                }
            }
            for v in victims {
                self.skill_hit(def.id, &caster, caster_id, v, power, kind);
            }
            return true;
        }

        // Targeted, with or without a blast (Fireball, Storm Field).
        let target = self.nearest(i, mons.iter().copied().filter(|&m| self.dist(i, m) <= def.range.max(1)));
        let Some(t) = target else { return false };
        let (tx, tl) = (self.units[t].x, self.units[t].lane);
        let victims: Vec<usize> = if def.aoe_radius > 0 {
            mons.iter().copied().filter(|&m| cheb(tx, tl, self.units[m].x, self.units[m].lane) <= def.aoe_radius).collect()
        } else {
            vec![t]
        };
        for v in victims {
            self.skill_hit(def.id, &caster, caster_id, v, power, kind);
        }
        true
    }

    /// Advance every placed field one tick; a field resolves a wave every `every` ticks.
    pub(crate) fn tick_fields(&mut self) {
        let mut fields = std::mem::take(&mut self.fields);
        for f in fields.iter_mut() {
            if f.next_in > 0 {
                f.next_in -= 1;
                continue;
            }
            f.next_in = f.every - 1;
            f.hits_left = f.hits_left.saturating_sub(1);
            let victims: Vec<usize> = self.surface_monsters().into_iter()
                .filter(|&m| cheb(f.x, f.lane, self.units[m].x, self.units[m].lane) <= f.radius).collect();
            for v in victims {
                self.skill_hit(f.skill, &f.caster, f.owner, v, f.power, f.kind);
            }
        }
        fields.retain(|f| f.hits_left > 0);
        self.fields = fields;
    }
}

#[cfg(test)]
mod tests {
    use crate::bench::{self, BenchSpec};
    use crate::{Battle, Col, Event, Rules, Team, Traits};
    use sim_core::data::GameData;

    fn data() -> GameData {
        GameData::from_json_str(include_str!("../../../data/game_data.json")).expect("catalog")
    }

    /// A bench party member + `n` live stage monsters in `lane` (their HP raised so the fight lasts).
    fn setup(unit: &str, col: Col, lane: i32, mons: &[(i32, Traits)]) -> (Battle, u32) {
        let d = data();
        let spec = BenchSpec::default();
        let mut b = Battle::new(Rules::default());
        let (ent, traits) = bench::unit(&d, unit, &spec).expect("unit");
        let id = b.place_entity(unit, ent, traits, col, lane).unwrap();
        let ms = bench::monsters(&d, &spec, mons.len()).expect("monsters");
        for ((l, t), mut e) in mons.iter().zip(ms) {
            e.max_hp = 50_000_000;
            e.hp = e.max_hp;
            b.spawn_entity("m", e, *t, *l);
        }
        (b, id)
    }

    fn casts(b: &Battle, skill: &str) -> usize {
        b.events.iter().filter(|(_, e)| matches!(e, Event::Cast { skill: s, .. } if *s == skill)).count()
    }

    #[test]
    fn iron_chains_hauls_its_two_targets_next_to_seraphine_even_from_the_back_row() {
        let (mut b, sera) = setup("seraphine", Col::Back, 1, &[(0, Traits::default()), (2, Traits::default()), (1, Traits::default())]);
        let mut pulled = false;
        for _ in 0..400 {
            b.step();
            if casts(&b, "iron_chains") > 0 {
                let s = b.units.iter().find(|u| u.id == sera).unwrap();
                let beside = b.units.iter().filter(|u| u.team == Team::Monster && u.alive
                    && crate::cheb(u.x, u.lane, s.x, s.lane) <= 1 && u.taunted.is_some_and(|(t, _)| t == sera)).count();
                pulled = beside >= 1;
                break;
            }
        }
        assert!(pulled, "a chained monster stands beside her, locked on\n{}", b.render());
    }

    fn immortal(b: &mut Battle, id: u32) {
        let u = b.units.iter_mut().find(|u| u.id == id).unwrap();
        u.stats.hp = 1_000_000_000;
        u.hp = u.stats.hp;
    }

    /// Abyssal Embrace is an ultimate, so the trial never casts it on its own (no Limit Break
    /// gauge yet); cast directly, it hauls its radius onto the fixed point past the cap.
    #[test]
    fn abyssal_embrace_piles_its_radius_onto_the_fixed_point_past_the_cap() {
        let (mut b, nemu) = setup("nemu", Col::Back, 1, &(0..6).map(|k| (k % 3, Traits::default())).collect::<Vec<_>>());
        immortal(&mut b, nemu);
        assert!(b.units[0].kit.iter().all(|k| k.id != "abyssal_embrace"), "ultimates wait on the gauge");
        while b.units.iter().filter(|u| u.team == Team::Monster && u.alive && { let (px, pl) = b.abyss_point(); crate::cheb(u.x, u.lane, px, pl) <= 2 }).count() < 4 && b.tick < 500 {
            b.step();
        }
        let (px, pl) = b.abyss_point();
        let i = b.units.iter().position(|u| u.id == nemu).unwrap();
        assert!(b.cast(i, sim_core::skills::skill_def("abyssal_embrace").unwrap(), 5));
        let piled = b.units.iter().filter(|u| u.team == Team::Monster && u.alive && (u.x, u.lane) == (px, pl)).count();
        assert!(piled > b.rules.stack_cap, "{piled} on the point\n{}", b.render());
    }

    #[test]
    fn a_taunted_monster_crosses_the_gap_rule_to_its_taunter() {
        let d = data();
        let spec = BenchSpec::default();
        let mut b = Battle::new(Rules::default());
        let (tank, tt) = bench::unit(&d, "seraphine", &spec).unwrap();
        let sera = b.place_entity("sera", tank, tt, Col::Front, 1).unwrap();
        let (lumi, lt) = bench::unit(&d, "lumi", &spec).unwrap();
        let lumi_id = b.place_entity("lumi", lumi, lt, Col::Back, 1).unwrap();
        immortal(&mut b, sera);
        immortal(&mut b, lumi_id);
        let mut e = bench::monsters(&d, &spec, 1).unwrap().remove(0);
        e.max_hp = 50_000_000;
        e.hp = e.max_hp;
        let m = b.spawn_entity("m", e, Traits::default(), 1);
        while b.units.iter().find(|u| u.id == m).is_none_or(|u| u.x > Col::Front.x() + 1) && b.tick < 500 {
            b.step();
        }
        let until = b.tick + 300;
        b.units.iter_mut().find(|u| u.id == m).unwrap().taunted = Some((lumi_id, until));
        let mut crossed = false;
        for _ in 0..300 {
            b.step();
            let front = b.units.iter().find(|u| u.id == sera).map_or(0, |u| u.x);
            crossed |= b.units.iter().any(|u| u.id == m && u.x < front);
        }
        assert!(crossed, "the taunted monster went past the front line to Lumi\n{}", b.render());
    }
}
