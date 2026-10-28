//! GEMS — the owner's layer on a fungible product (0.0.386, `docs/gem-system-design.md`).
//!
//! Gear is a PRODUCT: every copy of "Legendary Adamant Saber" is identical, which is what lets a
//! market price it by name. That also means nothing on a piece is the player's own. Gems are
//! that half: a piece has sockets (by grade, minus its card slots — the same holes), a gem is a
//! SET of two stats that fits a slot group (War of Genesis's jewels: Strife = ATK% + crit
//! damage), and socketing rolls each value inside a narrow window — enough to make one piece
//! "mine", not enough to be worth cheating for, which is why the roll is a client draw.
//!
//! Gems are CONSUMED: socketing over a filled socket destroys what was there, there is no
//! unsocket, fusing a gemmed piece eats its gems, and six of one grade fuse into one of the next
//! (the gear rule). That is what keeps them a permanent demand in the material market.
//!
//! A gem is a BAG STACK (0.0.448 — user: "gem ต้องเข้ากระเป๋า"; it was a ledger material from
//! 0.0.386, which left it with no bag index for Disenchant and no slot cost): a `Material`-kind
//! item keyed by def id `gem_<set>_g<step>`, one def per (set × grade), stacking in the inventory
//! like any material, so the commodity market and Steam's registry each see a plain product.
//! `held_count` / `take` below are the two places that know where a gem lives. Grade rides the
//! gear ladder: same ten names, same colours, raw multiplier ×1.3/step.
//!
//! What is CODE here and what is CATALOG: the sets, their base values, the roll window, the merge
//! count and the drop odds are `loot_rules.gems` (tuned by editing JSON). Sockets per grade and
//! the per-stat caps are constants — they shape the ladder the way `GRADE_RAW_BP` does, and the
//! fight (which holds no catalog) has to clamp the totals.

use serde::{Deserialize, Serialize};

use crate::data::{GameData, GemSet};
use crate::grid::Entity;
use crate::items::{EquipSlot, Grade, Item, ItemKind, SocketedGem, StatMod};
use crate::rng::Pcg32;
use crate::state::GameState;
use crate::substats::SubstatKey;

/// SOCKETS ARE THE GRADE'S (user 2026-09-16: "เรียงตามเกรด … เข้าใจง่ายกว่า"): a piece has the gem
/// and card sockets its grade says, whatever its def. Index = `Grade::step()`, read as
/// "Tier 1 none · Tier 2 gem 1 · Tier 3 gem 1 card 1 · Tier 4-5 gem 2 card 1 · Tier 6-7 gem 2
/// card 2"; the three grades past the launch ladder extend it one gem. The slotted def
/// variants (`_1s` / `_2s`) that used to carry the card count were retired in the same patch.
pub const GEM_SLOTS_BY_GRADE: [u8; 10] = [0, 1, 1, 2, 2, 2, 2, 3, 3, 3];
pub const CARD_SLOTS_BY_GRADE: [u8; 10] = [0, 0, 1, 1, 1, 2, 2, 2, 2, 2];

/// Gems of `def_id` in the bag (bound and unbound alike — socketing and fusing are uses, not
/// sales). Where gems live since 0.0.448; the ledger holds none.
pub fn held_count(state: &GameState, def_id: &str) -> u32 {
    state
        .inventory
        .iter()
        .filter(|it| it.def_id == def_id && it.is_stackable())
        .map(|it| it.quantity)
        .sum()
}

/// Every (def id, count) gem stack in the bag, bag order — for the socket / fuse views.
pub fn held<'a>(state: &'a GameState, data: &'a GameData) -> impl Iterator<Item = (&'a str, u32)> + 'a {
    state
        .inventory
        .iter()
        .filter(|it| it.is_stackable() && set_of(data, &it.def_id).is_some())
        .map(|it| (it.def_id.as_str(), it.quantity))
}

/// Remove `n` gems of `def_id` from the bag, bound stacks last. False (nothing moved) when short.
fn take(state: &mut GameState, def_id: &str, mut n: u32) -> bool {
    if held_count(state, def_id) < n {
        return false;
    }
    for pass in [false, true] {
        let mut i = 0;
        while n > 0 {
            let Some(it) = state.inventory.get_mut(i) else { break };
            if it.def_id == def_id && it.is_stackable() && it.bound == pass {
                let k = it.quantity.min(n);
                it.quantity -= k;
                n -= k;
                if it.quantity == 0 {
                    state.inventory.remove(i);
                    continue;
                }
            }
            i += 1;
        }
    }
    n == 0
}

/// Gem sockets a piece has at `grade` — every wearable slot (user 2026-09-16: "มี gem ทุกไอเทม
/// แต่ gem แบ่งเป็นประเภท weapon / armor / accessory" — 2 + 4 + 4 slots); the SET decides which slots it fits.
pub fn gem_slots_at(grade: Grade) -> u8 {
    GEM_SLOTS_BY_GRADE.get(grade.step()).copied().unwrap_or(0)
}

/// Card sockets at `grade` — every wearable slot, since cards exist for all of them.
pub fn card_slots_at(grade: Grade) -> u8 {
    CARD_SLOTS_BY_GRADE.get(grade.step()).copied().unwrap_or(0)
}

/// The CAP on what gems alone may contribute to one stat on one unit, in the stat's own unit
/// (0 = none). The sensitive rates — the ones a full set of top gems would otherwise carry past
/// what the combat model was tuned against (design §3: 18 crit gems ≈ +26%) — are held here;
/// flat stats are not (495 flat ATK beside a 7,361-ATK Beyond weapon is noise). The totals are clamped per unit across every
/// worn piece, AFTER summing, so eighteen small gems and three big ones meet the same wall.
pub fn cap_of(key: SubstatKey) -> i32 {
    match key {
        SubstatKey::Crit | SubstatKey::Dodge | SubstatKey::MoveSpeed => 1_500,
        // CDR halved with every other CDR source (0.0.490).
        SubstatKey::Cdr => 750,
        SubstatKey::CritDmg | SubstatKey::AtkPct | SubstatKey::HpPct | SubstatKey::Heal
        | SubstatKey::IgnoreDef | SubstatKey::CastSpeed | SubstatKey::ElementDmg | SubstatKey::BossDmg => 5_000,
        // Charge rate / break build-up: a HIGH wall the source is tuned to stay under (user
        // 2026-09-16: "ลดต้นทาง cap 100% ให้ถึง cap ยากๆ") — Charge is an accessory set now (8 sockets
        // at the launch cap ≈ 58%); only a min-maxed post-launch set could touch this. Break DMG sits on one weapon set
        // and cannot pass 50%.
        SubstatKey::BreakBuildup | SubstatKey::ChargeRate => 10_000,
        SubstatKey::BreakDmg => 5_000,
        SubstatKey::Atk | SubstatKey::Hp | SubstatKey::Def | SubstatKey::HpRegen => 0,
    }
}

/// Which paper-doll slots a set fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GemGroup {
    /// Any slot (WoG's "Mana" jewel). RETIRED from the catalog (user 2026-09-16: "เอาสากลออก
    /// ปรับยาก") — the variant stays so the group table is total; no authored set uses it.
    #[default]
    Universal,
    Weapon,
    Armor,
    Accessory,
}

impl GemGroup {
    /// The slot family a set fits; a universal set fits everything.
    pub fn fits(self, slot: EquipSlot) -> bool {
        match self {
            GemGroup::Universal => true,
            GemGroup::Weapon => matches!(slot, EquipSlot::Weapon | EquipSlot::Shield),
            GemGroup::Armor => matches!(slot, EquipSlot::Helmet | EquipSlot::Armor | EquipSlot::Gloves | EquipSlot::Shoes),
            GemGroup::Accessory => matches!(
                slot,
                EquipSlot::Necklace | EquipSlot::Ring | EquipSlot::Earring | EquipSlot::Bracer
            ),
        }
    }
}

/// The def id of `set` at `grade` — the one naming rule every path shares (catalog, drop
/// re-key, fusion output, host queries).
pub fn def_id(set: &str, grade: Grade) -> String {
    format!("gem_{set}_g{}", grade.step())
}

/// The set a gem def belongs to, with its grade — `None` for anything that is not a gem.
pub fn set_of<'a>(data: &'a GameData, gem_def: &str) -> Option<(&'a GemSet, Grade)> {
    let spec = data.item(gem_def)?.gem.as_ref()?;
    let set = data.loot_rules.gems.sets.iter().find(|s| s.id == spec.set)?;
    Some((set, spec.grade))
}

/// The MID value of one stat of `set` at `grade` — the base on the raw ladder, rounded up like
/// every graded raw stat (`Grade::scale_raw`), so a grade step never reads as "+0".
pub fn mid_value(base: i32, grade: Grade) -> i32 {
    grade.scale_raw(base)
}

/// The window `[lo, hi]` a stat's roll lands in: `mid ± window_bp`, the bounds rounded OUTWARD
/// so a small mid still has a window at all (3 ± 5% is 3..3 otherwise, and the roll still draws).
pub fn window(mid: i32, window_bp: u32) -> (i32, i32) {
    let w = (i64::from(mid.abs()) * i64::from(window_bp) + 9_999) / 10_000;
    let w = w.clamp(0, i32::MAX as i64) as i32;
    (mid.saturating_sub(w), mid.saturating_add(w))
}

/// Roll the lines a gem gets on the way into a socket: ONE draw per stat, always taken, whatever
/// the window (a zero-width window still draws — the stream must not depend on the tuning).
pub fn roll_lines(rng: &mut Pcg32, set: &GemSet, grade: Grade, window_bp: u32) -> Vec<StatMod> {
    set.stats
        .iter()
        .map(|st| {
            let mid = mid_value(st.base, grade);
            let (lo, hi) = window(mid, window_bp);
            let span = (i64::from(hi) - i64::from(lo)).clamp(0, u32::MAX as i64) as u32;
            let r = rng.gen_range_u32(0, span.saturating_add(1));
            st.key.make_for(st.element, lo.saturating_add(r as i32))
        })
        .collect()
}

/// Where the piece being gemmed sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GemTarget {
    Inventory { idx: u32 },
    Equipped { entity_id: u32, equipped_slot: u32 },
}

fn target_item(state: &GameState, target: GemTarget) -> Option<&Item> {
    match target {
        GemTarget::Inventory { idx } => state.inventory.get(idx as usize),
        GemTarget::Equipped { entity_id, equipped_slot } => {
            let e = state.grid.entities.get(state.grid.idx_of_id(entity_id)?)?;
            e.equipped.get(equipped_slot as usize)
        }
    }
}

fn target_item_mut(state: &mut GameState, target: GemTarget) -> Option<&mut Item> {
    match target {
        GemTarget::Inventory { idx } => state.inventory.get_mut(idx as usize),
        GemTarget::Equipped { entity_id, equipped_slot } => {
            let i = state.grid.idx_of_id(entity_id)?;
            let e = state.grid.entities.get_mut(i)?;
            e.equipped.get_mut(equipped_slot as usize)
        }
    }
}

/// Why a socket would be refused — the reasons a UI shows instead of a dead button. `None` =
/// it goes in. Pure: reads the state, moves nothing.
pub fn refusal(state: &GameState, data: &GameData, target: GemTarget, socket: u8, gem_def: &str) -> Option<&'static str> {
    let Some((set, _grade)) = set_of(data, gem_def) else { return Some("not a gem") };
    if held_count(state, gem_def) == 0 {
        return Some("none held");
    }
    let Some(item) = target_item(state, target) else { return Some("no such item") };
    if !matches!(item.kind, ItemKind::Weapon | ItemKind::Armor) {
        return Some("not gear");
    }
    if socket >= item.gem_slots() {
        return Some("no such socket");
    }
    // Sockets fill in order: the next empty one, or any filled one (overwrite).
    if usize::from(socket) > item.gems.len() {
        return Some("fill the sockets in order");
    }
    if !set.group.fits(item.slot()) {
        return Some("this set does not fit that slot");
    }
    None
}

/// `SocketGem`: put one `gem_def` from the bag into `socket` of the target piece, rolling
/// its lines (one draw per stat). A filled socket is OVERWRITTEN and the old gem is gone; there
/// is no unsocket. No-op (no draw, nothing moved) on any refusal.
pub fn socket(state: &mut GameState, data: &GameData, target: GemTarget, socket: u8, gem_def: &str) -> bool {
    if refusal(state, data, target, socket, gem_def).is_some() {
        return false;
    }
    let Some((set, grade)) = set_of(data, gem_def) else { return false };
    let set = set.clone();
    // The gem leaves the bag FIRST, and a bag target is an INDEX: if the stack that empties sat
    // before the piece, the piece is one slot lower now. Count the stacks that will vanish ahead
    // of it before taking, and re-aim.
    let target = match target {
        GemTarget::Inventory { idx } => {
            // The stack `take` will draw from — exactly its order: the first UNBOUND stack, else
            // the first BOUND one (audit 2026-10-02: counting unbound stacks only missed a bound
            // single ahead of the piece, so the gem landed on the next item or was lost).
            let is_gem = |it: &crate::items::Item, bound: bool| it.def_id == gem_def && it.is_stackable() && it.bound == bound;
            let pick = state.inventory.iter().position(|it| is_gem(it, false))
                .or_else(|| state.inventory.iter().position(|it| is_gem(it, true)));
            let gone_before = pick
                .filter(|&p| p < idx as usize)
                .and_then(|p| state.inventory.get(p))
                .is_some_and(|it| it.quantity == 1);
            GemTarget::Inventory { idx: idx.saturating_sub(u32::from(gone_before)) }
        }
        other => other,
    };
    if !take(state, gem_def, 1) {
        return false;
    }
    let lines = roll_lines(&mut state.rng, &set, grade, data.loot_rules.gems.roll_window_bp);
    let Some(item) = target_item_mut(state, target) else { return false };
    let placed = SocketedGem { def_id: gem_def.to_string(), lines };
    match item.gems.get_mut(usize::from(socket)) {
        Some(slot) => *slot = placed,
        None => item.gems.push(placed),
    }
    state.measures.gems_socketed.add(1);
    true
}

/// The party's GEM CRAFTER level — gem fusions run, on the shared `loot_rules.fusion` ladder.
pub fn crafter_level(state: &GameState, data: &GameData) -> u32 {
    crate::fusion::crafter_level(&data.loot_rules.fusion, state.measures.gem_fusions.get())
}

/// The fail odds a gem fusion from `grade` faces NOW, after the crafter's bonus.
pub fn fail_bp(state: &GameState, data: &GameData, grade: Grade) -> u32 {
    crate::fusion::fail_bp_for(&data.loot_rules.fusion, state.measures.gem_fusions.get(), grade)
}

/// `FuseGems`: `merge` gem defs of ONE grade (any sets, repeats allowed) become one gem of the
/// next grade whose set is drawn by the sets' weights — TWO draws, always taken once the inputs
/// are accepted: the fail check (`fail_bp_by_grade`, by input grade), then the set. A failed
/// fusion credits one gem of the INPUT grade instead (the five others are gone), the way TBH's
/// synthesis fails at the top grades. Refused without a draw when: fusion is off, the count is wrong, any id is not a
/// gem, the grades differ, the next grade is past `fusion::FUSE_GRADE_CEILING`, or the ledger is
/// short. Client authority: a gem is worth far less than a piece of gear and the only thing a
/// steered draw could pick is the set.
pub fn fuse(state: &mut GameState, data: &GameData, gems: &[String]) -> bool {
    let rules = &data.loot_rules.gems;
    if rules.merge == 0 || gems.len() != rules.merge as usize || rules.sets.is_empty() {
        return false;
    }
    let mut grade: Option<Grade> = None;
    for id in gems {
        let Some((_, g)) = set_of(data, id) else { return false };
        if grade.is_some_and(|have| have != g) {
            return false;
        }
        grade = Some(g);
    }
    let Some(grade) = grade else { return false };
    let Some(next) = grade.next().filter(|n| *n <= crate::fusion::FUSE_GRADE_CEILING) else { return false };
    // Need per distinct id, checked against the bag BEFORE anything is taken.
    let mut need: Vec<(&str, u32)> = Vec::new();
    for id in gems {
        match need.iter_mut().find(|(k, _)| *k == id.as_str()) {
            Some((_, n)) => *n += 1,
            None => need.push((id.as_str(), 1)),
        }
    }
    if need.iter().any(|(id, n)| held_count(state, id) < *n) {
        return false;
    }
    for (id, n) in &need {
        take(state, id, *n);
    }
    let fail_bp = fail_bp(state, data, grade);
    let failed = state.rng.gen_range_u32(0, 10_000) < fail_bp;
    let total: u32 = rules.sets.iter().map(|s| s.weight).sum();
    let mut r = state.rng.gen_range_u32(0, total.max(1));
    let mut chosen = rules.sets.first();
    for s in &rules.sets {
        if r < s.weight {
            chosen = Some(s);
            break;
        }
        r -= s.weight;
    }
    let Some(out) = chosen else { return false };
    // The output goes to the bag through the loot path: six stacks just left, so a slot is free
    // unless every one was a partial stack — then the overflow policy applies like any drop.
    if let Some(def) = data.item(&def_id(&out.id, if failed { grade } else { next })) {
        crate::drops::bank_purchase_to_inventory(state, crate::data::instantiate_item(def));
    }
    state.measures.gem_fusions.add(1);
    if failed {
        state.measures.gem_fusions_failed.add(1);
    }
    true
}

/// Fold every gem line worn by `e`, SUMMED per stat across all pieces and clamped by `cap_of`,
/// into `f` as one line per stat — the fight's view of the gems. Fixed-size, no allocation:
/// this runs inside every stat sum.
pub fn for_each_gem_mod(e: &Entity, f: &mut impl FnMut(&StatMod)) {
    let mut sums = [0i32; SubstatKey::ALL.len()];
    // Element lines sum PER ELEMENT (a Holy line and a Shadow line are different stats).
    let mut by_element: [(Option<crate::grid::Element>, i32); 8] = [(None, 0); 8];
    let mut any = false;
    for it in &e.equipped {
        for g in &it.gems {
            for m in &g.lines {
                if let StatMod::ElementDmgBp(el, v) = *m {
                    if let Some(slot) = by_element.iter_mut().find(|(e, _)| *e == Some(el) || e.is_none()) {
                        *slot = (Some(el), slot.1.saturating_add(v));
                        any = true;
                    }
                    continue;
                }
                let Some(key) = SubstatKey::of(m) else { continue };
                if let Some(slot) = sums.get_mut(key as usize) {
                    *slot = slot.saturating_add(m.value());
                    any = true;
                }
            }
        }
    }
    if !any {
        return;
    }
    for (key, v) in SubstatKey::ALL.iter().zip(sums) {
        if v == 0 {
            continue;
        }
        let cap = cap_of(*key);
        let v = if cap > 0 { v.min(cap) } else { v };
        f(&key.make(v));
    }
    let cap = cap_of(SubstatKey::ElementDmg);
    for (el, v) in by_element {
        if let (Some(el), true) = (el, v != 0) {
            f(&StatMod::ElementDmgBp(el, if cap > 0 { v.min(cap) } else { v }));
        }
    }
}

/// Trim a loaded piece's gems to the sockets it has NOW — a re-tuned socket table, or a card
/// slot count that moved, must not leave a gem in a hole that no longer exists.
pub fn trim_to_sockets(item: &mut Item) {
    let n = usize::from(item.gem_slots());
    if item.gems.len() > n {
        item.gems.truncate(n);
    }
}

/// The grade-`g` def ids of every set, for a table (the substat CSV's twin) or a guard.
pub fn all_def_ids(data: &GameData) -> Vec<String> {
    let mut out = Vec::new();
    for s in &data.loot_rules.gems.sets {
        for g in crate::items::GRADES {
            out.push(def_id(&s.id, g));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{Rarity, GRADES};

    fn data() -> GameData {
        GameData::from_json_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/game_data.json"))).expect("catalog")
    }

    fn state_with(data: &GameData) -> GameState {
        let mut s = crate::data::new_game_in_zone(7, data, "zone_b1_1").expect("zone");
        let mut sword = crate::data::instantiate_item(data.item("sword_s3").expect("sword_s3"));
        sword.grade = Grade::Legendary; // step 3: two gem sockets
        s.inventory.push(sword);
        s
    }

    /// Sockets are the grade's on every wearable slot; cards follow their own grade table.
    /// Put `n` gems of `id` in the bag, the way a drop lands them.
    fn give(s: &mut GameState, d: &GameData, id: &str, n: u32) {
        let mut it = crate::data::instantiate_item(d.item(id).expect(id));
        it.quantity = n;
        crate::drops::bank_purchase_to_inventory(s, it);
    }

    #[test]
    fn sockets_follow_the_grade_on_every_slot() {
        let d = data();
        let mut sword = crate::data::instantiate_item(d.item("sword_s3").expect("def"));
        let mut ring = crate::data::instantiate_item(d.item("ring_s3").expect("def"));
        for g in GRADES {
            sword.grade = g;
            ring.grade = g;
            assert_eq!(sword.gem_slots(), GEM_SLOTS_BY_GRADE[g.step()], "{g:?}");
            assert_eq!(sword.socket_count(), CARD_SLOTS_BY_GRADE[g.step()], "{g:?}");
            assert_eq!(ring.gem_slots(), GEM_SLOTS_BY_GRADE[g.step()], "{g:?}");
            assert_eq!(ring.socket_count(), CARD_SLOTS_BY_GRADE[g.step()], "{g:?}");
        }
        let card = Item::new("c", ItemKind::Card, 0, 0, Rarity::Common);
        assert_eq!(card.gem_slots(), 0);
        assert_eq!(card.socket_count(), 0);
        assert_eq!(gem_slots_at(Grade::Common), 0, "Tier 1 has no socket");
        assert_eq!(gem_slots_at(Grade::Beyond), 2, "the launch cap carries two");
        assert_eq!(card_slots_at(Grade::Beyond), 2);
        assert!(GemGroup::Armor.fits(EquipSlot::Shoes) && !GemGroup::Armor.fits(EquipSlot::Ring));
        assert!(GemGroup::Universal.fits(EquipSlot::Bracer));
        assert!(!d.loot_rules.gems.sets.iter().any(|s| s.group == GemGroup::Universal), "no universal set is authored");
    }

    /// A socket takes one gem from the bag, takes one draw per stat, lands inside the window, and fills in
    /// order; overwriting keeps the count and loses the old gem; the wrong group is refused.
    #[test]
    fn socketing_rolls_inside_the_window_and_overwrites_in_place() {
        let d = data();
        let mut s = state_with(&d);
        let strife = def_id("strife", Grade::Rare);
        let sword_idx = s.inventory.len() as u32 - 1;
        give(&mut s, &d, &strife, 3);
        let t = GemTarget::Inventory { idx: sword_idx };
        assert_eq!(refusal(&s, &d, t, 1, &strife), Some("fill the sockets in order"));
        assert!(socket(&mut s, &d, t, 0, &strife));
        assert_eq!(held_count(&s, &strife), 2);
        let it = s.inventory.get(sword_idx as usize).expect("sword");
        assert_eq!(it.gems.len(), 1);
        let (set, g) = set_of(&d, &strife).expect("strife");
        for (line, st) in it.gems[0].lines.iter().zip(&set.stats) {
            let (lo, hi) = window(mid_value(st.base, g), d.loot_rules.gems.roll_window_bp);
            assert!(line.value() >= lo && line.value() <= hi, "{line:?} outside {lo}..{hi}");
            assert_eq!(SubstatKey::of(line), Some(st.key));
        }
        assert!(!it.market_clean(), "a gemmed piece is not a clean product");
        // Overwrite socket 0: count stays 1, a gem paid again.
        assert!(socket(&mut s, &d, t, 0, &strife));
        assert_eq!(s.inventory.get(sword_idx as usize).expect("sword").gems.len(), 1);
        assert_eq!(held_count(&s, &strife), 1);
        // An armour set on a weapon is refused, nothing moves.
        let life = def_id("life", Grade::Rare);
        give(&mut s, &d, &life, 1);
        let draws_before = s.rng.clone();
        assert!(!socket(&mut s, &d, t, 1, &life));
        assert_eq!(held_count(&s, &life), 1);
        assert_eq!(s.rng, draws_before, "a refusal takes no draw");
        // A second weapon set fills the next socket.
        let ruin = def_id("ruin", Grade::Rare);
        give(&mut s, &d, &ruin, 1);
        assert!(socket(&mut s, &d, t, 1, &ruin));
        assert_eq!(s.inventory.get(sword_idx as usize).expect("sword").gems.len(), 2);
    }

    /// AUDIT 2026-10-02: a BOUND single gem ahead of the piece is the stack `take` empties, so the
    /// piece's index shifts — the gem must still land on the piece that was aimed at.
    #[test]
    fn a_bound_single_gem_ahead_of_the_piece_still_lands_on_it() {
        let d = data();
        let mut s = crate::data::new_game_in_zone(7, &d, "zone_b1_1").expect("zone");
        s.inventory.clear();
        let strife = def_id("strife", Grade::Rare);
        let mut gem = crate::data::instantiate_item(d.item(&strife).expect("gem"));
        gem.quantity = 1;
        gem.bound = true;
        s.inventory.push(gem);
        let mut sword = crate::data::instantiate_item(d.item("sword_s3").expect("sword_s3"));
        sword.grade = Grade::Legendary;
        s.inventory.push(sword);
        s.inventory.push(crate::data::instantiate_item(d.item("ring_s3").expect("ring")));
        assert!(socket(&mut s, &d, GemTarget::Inventory { idx: 1 }, 0, &strife));
        assert_eq!(s.inventory.len(), 2, "the bound single was taken");
        assert_eq!(s.inventory[0].def_id, "sword_s3");
        assert_eq!(s.inventory[0].gems.len(), 1, "the sword got the gem");
        assert!(s.inventory[1].gems.is_empty(), "not the ring after it");
    }

    /// The fold sums per stat across pieces and stops at the cap.
    #[test]
    fn worn_gem_lines_sum_per_stat_and_meet_the_cap() {
        let d = data();
        let mut s = state_with(&d);
        let sword = s.inventory.pop().expect("sword");
        let idx = s.grid.entities.iter().position(|e| matches!(e.team, crate::grid::Team::Player)).expect("player");
        // Two pieces, each carrying an over-cap crit line and a flat ATK line.
        for _ in 0..2 {
            let mut it = sword.clone();
            it.gems.push(SocketedGem { def_id: "x".into(), lines: vec![StatMod::CritBp(1_000), StatMod::AtkFlat(7)] });
            s.grid.entities[idx].equipped.push(it);
        }
        let mut seen = Vec::new();
        for_each_gem_mod(&s.grid.entities[idx], &mut |m| seen.push(*m));
        assert!(seen.contains(&StatMod::CritBp(1_500)), "2,000 clamps to the 1,500 cap: {seen:?}");
        assert!(seen.contains(&StatMod::AtkFlat(14)), "flat ATK is uncapped and summed: {seen:?}");
        assert_eq!(seen.len(), 2, "one line per stat");
        assert_eq!(crate::combat::stat_mods::crit_bp(&s.grid.entities[idx]), 1_500, "the fight reads the fold");
        // Element lines sum PER ELEMENT: a Twilight pair on every worn piece = Holy ×n, Shadow ×n.
        use crate::grid::Element;
        let worn = s.grid.entities[idx].equipped.len() as i32;
        for it in s.grid.entities[idx].equipped.iter_mut() {
            it.gems.push(SocketedGem { def_id: "y".into(), lines: vec![StatMod::ElementDmgBp(Element::Light, 300), StatMod::ElementDmgBp(Element::Dark, 300)] });
        }
        let mut seen = Vec::new();
        for_each_gem_mod(&s.grid.entities[idx], &mut |m| seen.push(*m));
        assert!(seen.contains(&StatMod::ElementDmgBp(Element::Light, 300 * worn)), "{seen:?}");
        assert!(seen.contains(&StatMod::ElementDmgBp(Element::Dark, 300 * worn)), "{seen:?}");
        assert_eq!(crate::combat::stat_mods::element_dmg_bp(&s.grid.entities[idx], Element::Light), 300 * worn);
        assert_eq!(crate::combat::stat_mods::element_dmg_bp(&s.grid.entities[idx], Element::Fire), 0, "Fire gets nothing from a Twilight gem");
    }

    /// Six of one grade become one of the next; mixed grades, a short ledger, the wrong count and
    /// the ceiling are refused without a draw.
    #[test]
    fn six_gems_of_one_grade_fuse_into_one_of_the_next() {
        let d = data();
        let mut s = state_with(&d);
        let a = def_id("strife", Grade::Common);
        let b = def_id("life", Grade::Common);
        give(&mut s, &d, &a, 4);
        give(&mut s, &d, &b, 2);
        let six = vec![a.clone(), a.clone(), a.clone(), a.clone(), b.clone(), b.clone()];
        let before = s.rng.clone();
        assert!(!fuse(&mut s, &d, &six[..5]), "five is not six");
        assert!(!fuse(&mut s, &d, &[six.clone(), vec![a.clone()]].concat()), "seven is not six");
        let mixed = vec![a.clone(), a.clone(), a.clone(), a.clone(), b.clone(), def_id("life", Grade::Rare)];
        assert!(!fuse(&mut s, &d, &mixed), "grades must match");
        assert_eq!(s.rng, before, "refusals take no draw");
        assert!(fuse(&mut s, &d, &six));
        assert_eq!(held_count(&s, &a), 0);
        assert_eq!(held_count(&s, &b), 0);
        let bag: Vec<(String, u32)> = held(&s, &d).map(|(k, v)| (k.to_string(), v)).collect();
        let gems: Vec<&(String, u32)> = bag.iter().collect();
        assert_eq!(gems.len(), 1);
        assert!(gems[0].0.ends_with("_g1"), "one Uncommon gem: {gems:?}");
        assert_eq!(gems[0].1, 1);
        // FAILURE: with a 100% fail row the six come back as ONE of the input grade, and the
        // draw count is the same as a success (two).
        let mut d2 = d.clone();
        d2.loot_rules.fusion.fail_bp_by_grade = vec![10_000];
        let c = def_id("wind", Grade::Common);
        give(&mut s, &d, &c, 6);
        let before = s.rng.clone();
        let mut probe = before.clone();
        probe.gen_range_u32(0, 10_000);
        probe.gen_range_u32(0, 10_000);
        assert!(fuse(&mut s, &d2, &vec![c.clone(); 6]));
        assert_eq!(s.rng, probe, "two draws, fail or not");
        let common_gems: u32 = held(&s, &d).filter_map(|(k, n)| set_of(&d, k).filter(|(_, g)| *g == Grade::Common).map(|_| n)).sum();
        assert_eq!(common_gems, 1, "one Common gem handed back");
        assert_eq!(s.measures.gem_fusions_failed.get(), 1);
        // CRAFTER LEVEL: the shared ladder, on the gem counter.
        let mut d3 = d.clone();
        d3.loot_rules.fusion.fail_bp_by_grade = vec![5_000];
        d3.loot_rules.fusion.crafter_fusions_per_level = 1;
        d3.loot_rules.fusion.crafter_level_cap = 30;
        d3.loot_rules.fusion.crafter_bp_per_level = 100;
        assert_eq!(crafter_level(&s, &d3), s.measures.gem_fusions.get() as u32);
        assert_eq!(fail_bp(&s, &d3, Grade::Rare), 5_000 - 100 * s.measures.gem_fusions.get() as u32);
        s.measures.gem_fusions.add(1_000);
        assert_eq!(crafter_level(&s, &d3), 30, "capped");
        assert_eq!(fail_bp(&s, &d3, Grade::Rare), 2_000, "50% − 30 × 1%");
        assert_eq!(fail_bp(&s, &d, Grade::Common), 0, "the catalog's Common row is free");
        // The ceiling: six at the cap grade refuse.
        let top = def_id("strife", crate::fusion::FUSE_GRADE_CEILING);
        give(&mut s, &d, &top, 6);
        assert!(!fuse(&mut s, &d, &vec![top.clone(); 6]));
        assert_eq!(held_count(&s, &top), 6);
    }
}
