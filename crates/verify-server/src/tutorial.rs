//! TUTORIAL QUESTS — the chaptered first-day quest list (docs/tutorial-onboarding-design.md).
//!
//! Pure like `daily`: the quest table, what each quest asks and pays, and which are done given
//! `Facts` read from the verified save and the server's own tables. The endpoints in `main.rs`
//! gather the facts and pay; nothing here knows a database.
//!
//! A quest is DONE when its condition holds on the facts, and CLAIMABLE when it is done, its
//! chapter is open (every quest of the chapter before is claimed), and its bit is not yet in the
//! account's claimed mask. Bits are written per quest and never recycled, like the daily board's.

use sim_core::state::Measures;

/// What a quest asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cond {
    BossKills(u64),
    RegaliaEquipped(u64),
    RegaliaEnhanced(u64),
    RegaliaSalvaged(u64),
    Fusions(u64),
    GemsSocketed(u64),
    TrainingLevels(u64),
    PotentialRanks(u64),
    /// Paths of Soul nodes allocated (the Soul web, separate from Potential).
    SoulNodes(u64),
    ItemsDisenchanted(u64),
    /// Items loaded into the party provision rack.
    ProvisionsLoaded(u64),
    /// `band_reached` at least this stage.
    StageReached(u32),
    /// Some fielded member WEARS at least this many pieces right now (user 2026-10-02: a hero
    /// in 3 pieces — an equip counter could be farmed by re-equipping one item).
    MemberGear(u32),
    /// Party members fielded at least this many.
    PartySize(u32),
    /// Starter-banner pulls made at least this many (`gacha_newcomer.made`).
    StarterPulls(u32),
    /// Any daily-ladder rung ever claimed.
    DailyRungClaimed,
    /// Crest levels raised (`Measures::crest_levels`).
    CrestLevels(u64),
    /// Something only the client can see (Companion mode opened): done when the CLAIM says so.
    /// Pays only a token reward for that reason.
    ClientReported,
    /// Story missions cleared (`story_claims` rows).
    StoryCleared(u32),
    /// Commander level rewards accepted through at least this level.
    CommanderClaimed(u32),
}

/// What a quest pays. Tickets are Standard tickets; items arrive BOUND by mail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reward {
    Tickets(u32),
    Cxp(i64),
    Gil(u64),
    Item { def_id: &'static str, qty: u32 },
    /// A Rare-grade tier-1 weapon of the lead's weapon family (quest 1, user 2026-09-30).
    LeadWeapon,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quest {
    pub id: &'static str,
    /// Stable, author-assigned, never recycled.
    pub bit: u8,
    pub chapter: u8,
    pub cond: Cond,
    pub rewards: Vec<Reward>,
}

/// Everything the conditions read. Built by the endpoint from the save and the server tables.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub measures: Measures,
    pub band_reached: u32,
    pub party_size: u32,
    /// The most pieces any fielded member wears.
    pub most_gear: u32,
    pub starter_pulls: u32,
    pub daily_rung_claimed: bool,
    pub commander_claimed_through: u32,
    pub story_cleared: u32,
    /// Set by `/tutorial/claim` only — `/tutorial/state` never reports a `ClientReported` quest done.
    pub client_reported: bool,
}

pub fn quests() -> Vec<Quest> {
    use Cond::*;
    use Reward::*;
    let q = |id, bit, chapter, cond, rewards| Quest { id, bit, chapter, cond, rewards };
    vec![
        // CH.1 — the forced tutorial's steps (the client drives them; the server pays).
        q("first_boss", 0, 1, BossKills(1), vec![LeadWeapon, Cxp(100)]),
        q("deploy_second", 1, 1, PartySize(2), vec![Tickets(1), Cxp(100)]),
        q("first_pull", 2, 1, StarterPulls(10), vec![Tickets(1), Cxp(150)]),
        q("deploy_third", 3, 1, PartySize(3), vec![Tickets(1), Cxp(150)]),
        q("equip_regalia", 4, 1, RegaliaEquipped(1), vec![Tickets(2), Cxp(200)]),
        // CH.2 — build the team.
        // Pays the EXP Elixirs the next quest loads into the provision rack.
        q("equip_gear", 5, 2, MemberGear(3), vec![Tickets(1), Item { def_id: "exp_elixir", qty: 3 }, Cxp(100)]),
        q("load_provision", 18, 2, ProvisionsLoaded(1), vec![Cxp(150)]),
        q("fuse_gear", 6, 2, Fusions(1), vec![Tickets(1), Cxp(200)]),
        // Soul reaches the first node's 500 at ~20-25 min of play; nothing to hand out.
        q("soul_node", 17, 2, SoulNodes(1), vec![Tickets(1), Cxp(150)]),
        q("first_story", 19, 2, StoryCleared(1), vec![Cxp(150)]),
        // Companion mode is client-only, so the claim is the client's word; a token reward.
        q("companion_mode", 21, 2, ClientReported, vec![Item { def_id: "exp_elixir", qty: 1 }, Cxp(100)]),
        q("daily_rung", 7, 2, DailyRungClaimed, vec![Tickets(1), Cxp(150)]),
        // Reaching stage 4 hands out the gems the next quest teaches (drops cannot be relied on).
        q("reach_stage_4", 8, 2, StageReached(4), vec![
            Item { def_id: "gem_strife_g0", qty: 1 },
            Item { def_id: "gem_fortitude_g0", qty: 1 },
            Item { def_id: "gem_life_g0", qty: 1 },
            Cxp(100),
        ]),
        q("socket_gem", 9, 2, GemsSocketed(1), vec![Tickets(2), Cxp(200)]),
        // CH.3 — Memory Fragments.
        q("salvage_regalia", 10, 3, RegaliaSalvaged(1), vec![Tickets(1), Cxp(150)]),
        q("enhance_regalia", 11, 3, RegaliaEnhanced(1), vec![Tickets(2), Cxp(200)]),
        // CH.4 — the long-term systems.
        q("train_1", 12, 4, TrainingLevels(1), vec![Tickets(1), Cxp(150)]),
        // Reaching stage 10 hands out the first Potential node's Sigils and Gil.
        q("reach_stage_10", 13, 4, StageReached(10), vec![Item { def_id: "sigil_1", qty: 6 }, Gil(10_000), Cxp(150)]),
        q("potential_node", 14, 4, PotentialRanks(1), vec![Tickets(2), Cxp(250)]),
        q("commander_level", 15, 4, CommanderClaimed(2), vec![Tickets(1), Cxp(200)]),
        // Pays one material for each category's T3 Crest (the economy lines), so the next quest can
        // raise one before T3 gear drops (user 2026-10-02: "ให้วัตถุดิบไปเลย").
        q("disenchant", 16, 4, ItemsDisenchanted(1), vec![
            Tickets(2),
            Item { def_id: "crest_mat_weapon_t3", qty: 1 },
            Item { def_id: "crest_mat_armor_t3", qty: 1 },
            Item { def_id: "crest_mat_accessory_t3", qty: 1 },
            Item { def_id: "crest_mat_gem_t3", qty: 1 },
            Cxp(250),
        ]),
        q("crest_level", 20, 4, CrestLevels(1), vec![Cxp(150)]),
    ]
}

/// Whether `cond` holds on `f`.
pub fn is_done(cond: Cond, f: &Facts) -> bool {
    let m = &f.measures;
    match cond {
        Cond::BossKills(n) => m.boss_kills.get() >= n,
        Cond::RegaliaEquipped(n) => m.regalia_equipped.get() >= n,
        Cond::RegaliaEnhanced(n) => m.regalia_enhanced.get() >= n,
        Cond::RegaliaSalvaged(n) => m.regalia_salvaged.get() >= n,
        Cond::Fusions(n) => m.fusions.get() >= n,
        Cond::GemsSocketed(n) => m.gems_socketed.get() >= n,
        Cond::TrainingLevels(n) => m.training_levels.get() >= n,
        Cond::PotentialRanks(n) => m.potential_ranks.get() >= n,
        Cond::SoulNodes(n) => m.soul_nodes_allocated.get() >= n,
        Cond::ItemsDisenchanted(n) => m.items_disenchanted.get() >= n,
        Cond::ProvisionsLoaded(n) => m.party_provisions_loaded.get() >= n,
        Cond::StageReached(s) => f.band_reached >= s,
        Cond::PartySize(n) => f.party_size >= n,
        Cond::MemberGear(n) => f.most_gear >= n,
        Cond::StarterPulls(n) => f.starter_pulls >= n,
        Cond::DailyRungClaimed => f.daily_rung_claimed,
        Cond::CommanderClaimed(l) => f.commander_claimed_through >= l,
        Cond::StoryCleared(n) => f.story_cleared >= n,
        Cond::CrestLevels(n) => m.crest_levels.get() >= n,
        Cond::ClientReported => f.client_reported,
    }
}

/// The lowest chapter with an unclaimed quest — every quest up to and including it is visible,
/// later chapters are locked. `u8::MAX` when everything is claimed.
pub fn open_chapter(qs: &[Quest], claimed: u64) -> u8 {
    qs.iter()
        .filter(|q| claimed & (1u64 << q.bit) == 0)
        .map(|q| q.chapter)
        .min()
        .unwrap_or(u8::MAX)
}

/// Whether `q` may be claimed now.
pub fn claimable(q: &Quest, qs: &[Quest], claimed: u64, f: &Facts) -> bool {
    claimed & (1u64 << q.bit) == 0 && q.chapter <= open_chapter(qs, claimed) && is_done(q.cond, f)
}

pub fn quest(id: &str) -> Option<Quest> {
    quests().into_iter().find(|q| q.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Starter banner's 50 free pulls: slots 3-5 pay 30, the quests pay the other 20.
    #[test]
    fn the_quests_pay_twenty_tickets_and_about_three_thousand_cxp() {
        let qs = quests();
        let tickets: u32 = qs.iter().flat_map(|q| q.rewards.iter())
            .map(|r| if let Reward::Tickets(n) = r { *n } else { 0 }).sum();
        let cxp: i64 = qs.iter().flat_map(|q| q.rewards.iter())
            .map(|r| if let Reward::Cxp(n) = r { *n } else { 0 }).sum();
        assert_eq!(tickets, 20);
        assert!((2_500..=3_500).contains(&cxp), "{cxp}");
    }

    #[test]
    fn bits_are_distinct_and_fit_the_mask() {
        let qs = quests();
        let mut bits: Vec<u8> = qs.iter().map(|q| q.bit).collect();
        bits.sort_unstable();
        bits.dedup();
        assert_eq!(bits.len(), qs.len(), "two quests share a bit");
        assert!(qs.iter().all(|q| q.bit < 64));
        let mut ids: Vec<&str> = qs.iter().map(|q| q.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), qs.len(), "two quests share an id");
    }

    /// A chapter opens only once the one before is fully claimed; done-but-locked is not claimable.
    #[test]
    fn chapters_open_in_order() {
        let qs = quests();
        let mut f = Facts::default();
        f.measures.fusions.add(1);
        let fuse = quest("fuse_gear").unwrap();
        assert!(is_done(fuse.cond, &f));
        assert!(!claimable(&fuse, &qs, 0, &f), "chapter 2 is locked while chapter 1 is unclaimed");
        let ch1: u64 = qs.iter().filter(|q| q.chapter == 1).map(|q| 1u64 << q.bit).sum();
        assert!(claimable(&fuse, &qs, ch1, &f), "chapter 1 claimed: chapter 2 opens");
        assert!(!claimable(&fuse, &qs, ch1 | (1 << fuse.bit), &f), "claimed once");
    }

    /// "A hero in 3 pieces" is not already true on a new game, or the quest would teach nothing.
    #[test]
    fn a_new_party_does_not_already_wear_three_pieces() {
        let data = sim_core::data::GameData::from_json_str(include_str!("../../../data/game_data.json")).unwrap();
        let g = sim_core::data::new_game_as(1, &data, "zone_b1_1", "suzu").unwrap();
        let most = g.grid.entities.iter()
            .filter(|e| matches!(e.team, sim_core::grid::Team::Player) && !e.is_pet)
            .map(|e| e.equipped.len()).max().unwrap_or(0);
        assert!(most < 3, "a new lead already wears {most}");
        let q = quest("equip_gear").unwrap();
        assert!(!is_done(q.cond, &Facts { most_gear: most as u32, ..Facts::default() }));
        assert!(is_done(q.cond, &Facts { most_gear: 3, ..Facts::default() }));
    }

    /// A client-reported quest is never done on the state read, only on its own claim.
    #[test]
    fn a_client_reported_quest_is_done_only_when_claimed() {
        let q = quest("companion_mode").unwrap();
        let mut f = Facts::default();
        assert!(!is_done(q.cond, &f));
        f.client_reported = true;
        assert!(is_done(q.cond, &f));
    }

    /// Every item a quest pays is a real catalog def, and never one that mints to Steam.
    #[test]
    fn every_item_reward_is_in_the_catalog() {
        let data = sim_core::data::GameData::from_json_str(include_str!("../../../data/game_data.json")).unwrap();
        for q in quests() {
            for r in &q.rewards {
                if let Reward::Item { def_id, .. } = r {
                    let d = data.item(def_id);
                    assert!(d.is_some(), "{} pays {def_id}, missing from the catalog", q.id);
                    assert!(d.is_some_and(|d| d.steam_itemdef.is_none()), "{} pays a Steam item", q.id);
                }
            }
        }
    }
}
