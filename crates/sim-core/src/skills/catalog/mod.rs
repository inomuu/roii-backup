//! Skill catalog — the `SkillDef` table + id/display lookup.
//!
//! Same "tune in code, move to data later" stance as the level curves.
//! **Adding a skill = (1) a `pub const` in the unit's submodule, (2) ONE row in
//! `REGISTRY` below (it feeds BOTH `skill_def` and `skill_display`, so they can
//! never drift apart), and (3) a node in the unit's kit in `skills::tree`.**
//!
//! Submodules group skills by unit (seraphine / lapis / exemplar = the pulled
//! units) plus the monster-only set. This module owns the public lookups and
//! re-exports every const so callers keep using `skills::IRON_CHAINS`, etc.

use super::SkillDef;

// Monster-only skills (no unit kit):
mod monster;
// Unit kits:
mod airi;
mod astier;
mod charon;
mod mirei;
mod suzu;
mod exemplar;
mod lumi;
mod nemu;
mod lapis;
mod seraphine;
mod sylvie;
mod zolfia;

pub use airi::*;
pub use astier::*;
pub use charon::*;
pub use mirei::*;
pub use suzu::*;
pub use exemplar::*;
pub use lumi::*;
pub use nemu::*;
pub use lapis::*;
pub use seraphine::*;
pub use sylvie::*;
pub use zolfia::*;
pub use monster::*;

/// One catalog row: the definition + its player-facing name/blurb. The id is the
/// single source of truth on `def.id` — there's no separate id field to drift.
struct SkillEntry {
    def: &'static SkillDef,
    name: &'static str,
    blurb: &'static str,
    /// Long "click to inspect" MECHANICS prose for the skill-detail panel.
    /// Prose only — numbers that live in `SkillDef` fields are rendered by the
    /// UI from the def itself so they can never drift from a balance change;
    /// numbers appearing here should only be ones whose formula lives in code
    /// (Taunt's cooldown curve, Endure's per-enemy DEF). "" = no detail yet
    /// (the UI falls back to the blurb).
    detail: &'static str,
}

const fn e(def: &'static SkillDef, name: &'static str, blurb: &'static str) -> SkillEntry {
    SkillEntry { def, name, blurb, detail: "" }
}

/// Row WITH the long inspect text (`ed` = entry + detail).
const fn ed(
    def: &'static SkillDef,
    name: &'static str,
    blurb: &'static str,
    detail: &'static str,
) -> SkillEntry {
    SkillEntry { def, name, blurb, detail }
}

/// THE registry — every learnable skill, grouped by class. Both `skill_def` and
/// `skill_display` read from this single table.
const REGISTRY: &[SkillEntry] = &[
    // --- Exemplar-granted (Resonance signatures; no class tree) ---
    // --- Rapis (Fire · Striker, AoE) ---
    ed(&FIREBALL, "Fireball", "An exploding magic bolt; hits a 3x3 area.",
        "A sphere of compressed flame that bursts on arrival, washing the \
        target and everything around it in fire. The first spell an apprentice \
        learns to throw, and the last thing many enemies see."),
    ed(&FLAME_SHIELD, "Flame Shield",
        "Self shield absorbs INT*lvl*2 dmg, then EXPLODES (3x3 Fire + knock).",
        "A skin of fire worn as armor. It drinks the blows meant for flesh — \
        and when it has drunk its fill, it gives everything back at once."),
    ed(&EMBER_CASCADE, "Ember Cascade",
        "Answers every Toughness break, anywhere, with a Fireball at whatever broke.",
        "She does not watch the enemy so much as listen for the moment its guard \
        gives. Whatever cracks, cracks alight."),
    ed(&FIRE_CAT_METEOR, "Fire Cat Meteor",
        "Every enemy in the room takes one burning blow at the same moment.",
        "The sky opens once, and lands everywhere at once."),
    // --- Astier (Water · Support, the tide) ---
    ed(&TIDEBLESS, "Tidebless",
        "For 12s the whole party deals +20% damage (+2% per level), and one element the party carries deals +10% more on top.",
        "She reads the room and blesses what is already there."),
    // --- Suzu (Fire · Destroyer, the bright one) ---
    ed(&BRIGHT_EDGE, "Bright Edge", "Her basic is a plain Fire swing.", "She is very pleased with it."),
    ed(&SUNDER, "Sunder",
        "A 3\u{00d7}3 sweep at double Toughness damage that cuts every guard it touches: DEF \u{2212}20% for 8s.",
        "The shield goes first. Everything after is easier."),
    ed(&BELL_STRIKE, "Bell Strike",
        "One heavy blow at double Toughness damage \u{2014} and its cooldown clears the moment anything on the field breaks.",
        "Ring it again."),
    ed(&BREAK_FEVER, "Break Fever",
        "Ultimate: for 15s she swings 40% faster, her basic chips Toughness at double, and she can break ANY enemy \u{2014} element or not.",
        "Fifteen seconds is plenty."),
    // --- Mirei (Thunder · Catalyst, the elder sister) ---
    ed(&CONDUCTOR, "Conductor",
        "Her basic is a Thunder bolt at range 6 that aims at whoever still carries her spark, sets one off, and does not spend it.",
        "She charges the room. The others collect."),
    ed(&STORM_FIELD, "Storm Field",
        "A 5\u{00d7}5 sheet of lightning that leaves five Static Sparks on everything inside \u{2014} anyone's blow sets one off for 40% of her ATK.",
        "Stand there. Something will hit them eventually."),
    ed(&QUICKEN, "Quicken", "She hurries her own hands: attack speed +30% for 6s.", "Impatience, elegantly."),
    ed(&THUNDERFALL, "Thunderfall",
        "Ultimate: Thunder on every enemy on the board, and every Static Spark on every one of them goes off at once.",
        "All of it, at the same moment."),
    // --- Charon (Dark · Destroyer, the ferryman) ---
    ed(&FERRYMANS_TOLL, "Ferryman's Toll",
        "Her oar chips Toughness at twice the usual rate, and every hit on a Broken target deals +30%.",
        "She smiles like she has already read the ending."),
    ed(&TOLLING_BLOW, "Tolling Blow",
        "The oar hits the ground: Dark damage to the 3\u{00d7}3 around her, triple Toughness damage.",
        "A bell somewhere below answers."),
    ed(&BLACKWATER_MARK, "Blackwater Mark",
        "A seal on the boss in range (else the biggest): for 10 s her hits chip its Toughness for +50% when it is weak to Dark, and at half even when it is not.",
        "Everyone crosses. This one crosses first."),
    ed(&PASSAGE_OF_NO_RETURN, "Passage of No Return",
        "The gate opens behind her and the oar sweeps the 5\u{00d7}5 for Dark damage at five times the Toughness damage.",
        "No one has come back to complain."),
    // --- Airi (Thunder · Striker, the loud one) ---
    ed(&STATIC_SHOT, "Static Shot",
        "Her basic is a Thunder arrow at range 6; every hit leaves Static: +2% Thunder damage taken, three deep. Every hit she lands on a taunted target deals +20%.",
        "She calls every shot before it lands. Most of them do."),
    ed(&CHAIN_BOLT, "Chain Bolt",
        "One arrow that jumps: Thunder damage to the three lowest enemies in range.",
        "One arrow, three targets. She counted."),
    ed(&OVERLOAD, "Overload",
        "She overcharges her own hands: attack speed +30% for 6 s.",
        "Faster. No, faster than that."),
    ed(&STORM_VOLLEY, "Storm Volley",
        "Five arrows of lightning on the five lowest enemies in range 8.",
        "Watch this."),
    // --- Lumi (Ice · Sentinel, the timid one) ---
    ed(&CHILL_TOUCH, "Chill Touch",
        "Her basic is a cold poke; every hit she lands leaves Chill: +2% Ice and Thunder damage taken, slower feet and slower hands, three deep.",
        "She did not mean to. It just happens when people get close."),
    ed(&FROST_TAUNT, "Frost Taunt",
        "Ice damage to the 5\u{00d7}5 around her; everything hit turns on her for 5 s.",
        "Please look somewhere else. No, wait."),
    ed(&ICE_MEND, "Ice Mend",
        "Heals herself for 10% of her max HP, +1% per level.",
        "A deep breath and a little frost."),
    ed(&FROST_AURA, "Frost Aura",
        "The party gains DEF +30% for 30 s, and every enemy that strikes a party member is left with her Chill.",
        "Cold enough that nobody wants to come closer."),
    // --- Nemu (Dark · Striker, the succubus) ---
    ed(&SWEET_POISON, "Sweet Poison",
        "Her basic is a Dark bolt at range 6; every hit she lands weakens the target: +3% Dark damage taken, slower feet and slower hands, ten deep.",
        "It never hurts at first. That is the whole trick."),
    ed(&VELVET_BOG, "Velvet Bog",
        "A 3\u{00d7}3 pool where the enemy is thickest: half speed, and a burn every half-second for six seconds.",
        "Soft as a bed and just as hard to leave."),
    ed(&HEARTSTRING, "Heartstring",
        "A tether to one enemy: Dark damage every second for six seconds, a tenth of it returned to her as health.",
        "She takes only what they would have given anyway."),
    ed(&ABYSSAL_EMBRACE, "Abyssal Embrace",
        "A black hole where the enemy is thickest: for three seconds it draws everything in and holds it, then closes on all of it at once.",
        "Come closer. Closer still."),
    // --- Seraphine (Physical · Sentinel) ---
    ed(&IRON_CHAINS, "Iron Chains",
        "Hauls the two nearest enemies in a 7\u{00d7}7 to her feet, damages them, locks them onto her, and raises her guard.",
        "Chain after chain goes out across the ring and comes back heavy. \
        Whatever the links close around is dragged to the ground at her feet and \
        finds it has no one else to look at."),
    ed(&STEEL_BREAK, "Steel Break",
        "A sweep around her that batters everything she has gathered.",
        "The answer to a crowd standing too close: a full turn of the blade at \
        waist height, meant less to kill than to break what is holding together."),
    ed(&AEGIS, "Aegis",
        "Shields the whole party at once; each shield bursts when it breaks.",
        "She plants the greatsword and the guard goes up over everyone behind \
        her. What the shield cannot hold, it throws back."),
    ed(&BULWARK, "Bulwark",
        "Hard DEF for every enemy she holds under her own Taunt.",
        "A knight who has gathered a crowd has also chosen her ground. The more \
        that press in, the less she gives."),
    ed(&SANCTUS_TERRA, "Sanctus Terra",
        "Consecrate the ground underfoot: a 5x5 holy aura that follows the caster for 15s, pulsing every second (magic damage from ATK); enemies inside take increased Holy damage.",
        "The knight plants her blade and the earth answers in light. Hallowed ground \
        walks with her — each step a benediction, each pulse a sentence passed on \
        the profane. Those who stand within the radiance find their darkness laid \
        bare, and every holy strike cuts the deeper for it."),
    ed(&FIAT_LUX, "Fiat Lux",
        "Passive: basic attacks have a 20% chance to smite the target with a twin flash of holy light (magic damage from ATK).",
        "Let there be light — and there is, sudden and merciless. The knight's blade \
        needs no incantation: every so often a strike simply IGNITES, and judgement \
        falls upon the struck in a cascade of radiance."),
    ed(&BENEDICTIO_LUCIS, "Benedictio Lucis",
        "Ultimate: bless the whole party with +20% ATK (+2% per level) for 30s. Gated by the Limit Break gauge.",
        "The Radiant Saint lifts her cross and the light settles over her companions \
        like a hand of grace — arms grow surer, prayers strike harder, and every \
        blow struck in the name of the light carries a little more of its wrath."),
    ed(&LUX_MEDICA, "Lux Medica",
        "Heal the most wounded ally in range for 50% ATK (+5% per level), every 8s.",
        "A single word of light, spoken over the one who needs it most. It asks nothing \
        of the saint but her conviction — and her conviction is a weapon."),
    ed(&COMMUNIO, "Communio",
        "Passive: whenever Lucielle casts a skill, every ally of her element or of the Sentinel doctrine is also healed for 10% of her ATK.",
        "Grace shared is grace doubled. Those who stand in the light with her, and those \
        who stand in front of it, are mended by every prayer she speaks."),
    ed(&SAGITTA_LUCIS, "Sagitta Lucis",
        "Passive: basic attacks become a ranged holy beam dealing 50% ATK (magic).",
        "No mace, no reach — only light. The Radiant Saint lets her prayer fly as a \
        lance of dawn, striking from afar where once she could not touch."),
    ed(&LUMEN_CONTAGIO, "Lumen Contagio",
        "Curse an unmarked enemy with holy vulnerability; when a marked foe dies it bursts in a 5x5 holy blast that spreads the curse.",
        "She names the profane, and the light remembers. Where the marked fall, dawn \
        erupts — and in its wake the curse leaps to the next, and the next, until the \
        whole host is kindling for the pyre of heaven."),
    ed(&UMBRA_FLETCH, "Umbra Fletch",
        "Passive: every arrow is Shadow, and every hit marks the target: +5% Shadow damage taken, up to 3 stacks.",
        "She fletches with the feathers of dusk. Nothing she looses is ever quite in the light."),
    ed(&NIGHTFALL_VOLLEY, "Nightfall Volley",
        "Loose three Shadow arrows at the three weakest enemies in range for 84% ATK each (+6% per level).",
        "Three shafts, three throats, one breath. The Starfletcher does not aim at the strong — \
        she ends what is already ending."),
    ed(&SHADOWBIND, "Shadowbind",
        "Pin one enemy for 90% ATK and open a bleed of 180% ATK over 5s (+15% per level).",
        "The arrow is only the beginning; the wound is the weapon."),
    ed(&ECLIPSE_RAIN, "Eclipse Rain",
        "Ultimate: a 5x5 volley on the boss, or the densest knot of enemies, within 8 — 2,250% ATK (+150% per level) to each. Gated by the Limit Break gauge.",
        "For one heartbeat the sky goes out, and what falls from it is not rain."),
    ed(&UMBRAL_EXPLOIT, "Sticky Fingers",
        "Passive: every blade is Dark, each distinct status on the target adds +10% damage, and a monster she finishes herself pays +20% Gil.",
        "She does not fight the whole enemy. She fights the crack in it — and empties its pockets on the way past."),
    ed(&VEIL_SLASH, "Pickpocket",
        "No damage: a touch that dazzles the target — it misses 20% of its blows (10% for a boss) for 8s — and lifts 10% of its price into the purse at once.",
        "You will not miss it. You were not going to spend it."),
    ed(&REND, "Rend",
        "Strike for 150% ATK (+15% per level) and open a rend of 225% ATK over 5s (+22.5% per level).",
        "The blade goes in shallow. It is the leaving that does the work."),
    ed(&UMBRAL_REQUIEM, "Grand Heist",
        "Ultimate: every enemy on the field takes 500% ATK (+37.5% per level) of Dark, is dazzled and rent, and pays 10% of its price. Gated by the Limit Break gauge.",
        "Everything in the room, in one pass. She counts it later."),
    ed(&UNDA_VITAE, "Unda Vitae",
        "Passive: basic attacks become a ranged water bolt dealing 50% ATK, a quarter of the damage heals the most wounded ally, and every ally she heals carries +4% damage (3 stacks) for 8s.",
        "The Tidewarden does not pray over the fallen — she turns the wound itself \
        into water, and the water finds whoever is bleeding worst."),
    ed(&RIVUS, "Rivus",
        "Heal the ally in the worst shape, then bounce to the next two for 70% and 50% of it.",
        "She opens a channel rather than a fountain. The water finds the worst wound \
        first, and what it does not spend there it carries on to the next."),
    ed(&FONS_VITAE, "Fons Vitae",
        "Ultimate: for 8s every ally is healed 5% (+0.5% per level) of their own max HP a second and fills their Limit Break gauge twice as fast.",
        "The spring is not hers. She only knows where it runs, and how to open the \
        ground above it when there is nothing left in her companions to give."),
    // --- Sylvie (Thunder · Support, the one who forgets) ---
    ed(&SPARK_TAP, "Spark Tap",
        "Passive: basic attacks become a ranged Thunder bolt dealing 60% ATK.",
        "She was aiming somewhere else. It worked anyway."),
    ed(&SURGE, "Surge",
        "For 10s the whole party gains CRIT +15% (+1% per level) and attack speed +10% (+1% per level); Thunder allies gain half as much attack speed again.",
        "Everyone hurry up! ...Where were we going?"),
    ed(&ARC_FOCUS, "Arc Focus",
        "The ally with the highest ATK gains CRIT DMG +50% (+5% per level) and ATK +15% (+1.5% per level) for 8s.",
        "You're the strong one, right? Go on then."),
    ed(&BOLT_FROM_THE_BLUE, "Bolt from the Blue",
        "Ultimate: for 15s the whole party gains CRIT +25%, and every ally's skill cooldowns drop by 3s at once.",
        "Nobody saw it coming. Least of all her."),
    // --- Zolfia (Ice · Destroyer, the headless knight) ---
    ed(&COLDFLAME_KNIGHT, "Coldflame Knight",
        "Passive: basic attacks strike for 100% ATK of Ice and lay Winter Chill for 8s. A chilled enemy that breaks, from anyone, is Shattered: +20% Ice and Fire damage taken for the break and 5s after.",
        "The fire is blue because it has forgotten how to be warm."),
    ed(&COLDFLAME_SPLIT, "Coldflame Split",
        "A heavy blow for 350% ATK (+35% per level) of Ice at triple Toughness damage.",
        "It is not the edge that breaks them. It is the cold behind it."),
    ed(&REMORSELESS_WINTER, "Remorseless Winter",
        "For 8s an aura around her slows every enemy within 2 cells (attack speed \u{2212}20%, \u{2212}1% per level) and deals 45% ATK (+4.5% per level) of Ice to each every second.",
        "She does not hurry. Nothing near her can."),
    ed(&WINTERS_VERDICT, "Winter's Verdict",
        "Ultimate: 500% ATK (+50% per level) of Ice to every enemy on the field at double Toughness damage, chilling each.",
        "The sword goes into the ground. The winter comes up out of it."),
    // --- Monster-only (carried via MonsterDef.skills; no class tree) ---
    e(&SELF_DESTRUCT, "Self-Destruct", "On death: a 3x3 blast that damages + knocks back both teams."),
    e(&BURST_BLAST, "Burst Blast", "On death: a big 5x5 blast (20 dmg) that knocks back both teams."),
    e(&MENDING_BURST, "Mending Burst", "On death: heal every ally monster in a 5x5 for 50 HP."),
    e(&TOXIC_BURST, "Toxic Burst", "On death: a 3x3 blast that damages, knocks back, and poisons."),
    e(&POISON_ATTACK, "Poison Attack", "Active: a bite dealing 150% that also poisons (30s cd)."),
    e(&UMBRAL_ROOTS, "Umbral Roots", "Active: a 3x3 Shadow field at a target — ticks damage and slows."),
    e(&BLOOD_SLASH, "Blood Slash", "Active: a Shadow slash dealing 80% that opens a strong bleed."),
    e(&BLOODFRENZY, "Bloodfrenzy", "Passive: each landed hit ramps the biter's ASPD +5%, up to +100%."),
    e(&DEATHKNELL, "Deathknell", "Active: a 3s wind-up, then five Shadow bells crash across a 7x7."),
    e(&PYRE_MARK, "Pyre Mark", "Active: brands a random foe — 3s later a 3x3 Fire blast on that ground."),
    e(&PYRE_MARK_WOUNDED, "Pyre Mark", "Active (below 50% HP): the same brand, twice as often."),
    e(&HANDMAIDS_GRASP, "Handmaid's Grasp", "Active: a weak clutch that taunts its victim onto the maid for 4s."),
    e(&RITE_OF_THE_SEVERED_HEAD, "Rite of the Severed Head", "Active (below 50% HP): the household answers — four handmaids join her."),
    e(&PUPPET_LUNGE, "Puppet Lunge", "Active: teleport behind the lowest-HP foe for a 250% backstab."),
    e(&FERAL_TRANSFORMATION, "Feral Transformation", "Enrage below 50% HP (50% chance): a thicker hide — -50% incoming damage."),
    e(&WAIL, "Wail", "Active: a Shadow scream that slows -30% ASPD to the whole party caught in it."),
    e(&MENDING_DIRGE, "Mending Dirge", "Active: AoE heal — every ally in a 5×5 mends 15% of its max HP."),
    e(&FRENZIED_RITE, "Frenzied Rite", "Active: goad the pack — +30% ASPD to every ally."),
    e(&VANISH, "Vanish", "Active: wink out of sight (untargetable) and stalk toward the prey."),
    e(&QUICK_STEP, "Quick Step", "Active: nimble footwork — +80 Flee and +30% ASPD for a while."),
    e(&WITCH_FIREBALL, "Witch's Fireball", "Active: a 200% Fire blast over a 3x3 area."),
    e(&SUMMON_BATS, "Summon Bats", "Active: call a pair of bats to swarm the party."),
];

/// Look up a skill definition by id. `None` for unknown ids (replay-safe — an
/// unknown skill on a save just does nothing).
pub fn skill_def(id: &str) -> Option<&'static SkillDef> {
    REGISTRY.iter().map(|r| r.def).find(|d| d.id == id)
}

/// Player-facing display name + one-line blurb for a skill id; `("", "")` when
/// unknown. Original names (not RO trademarks); the internal `id` stays stable so
/// saves / goldens never churn. Consumed by the bridge's `skill_tree_json`.
pub fn skill_display(id: &str) -> (&'static str, &'static str) {
    REGISTRY
        .iter()
        .find(|r| r.def.id == id)
        .map(|r| (r.name, r.blurb))
        .unwrap_or(("", ""))
}

/// Long "click to inspect" detail prose for a skill ("" = none authored yet —
/// the UI falls back to the blurb).
pub fn skill_detail(id: &str) -> &'static str {
    REGISTRY
        .iter()
        .find(|r| r.def.id == id)
        .map(|r| r.detail)
        .unwrap_or("")
}

/// Every catalog row's `(id, name, blurb, detail)` in registry order. Tooling
/// hook — sim-cli `--dump-strings` feeds these to the localization extractor so
/// the registry stays the single source of skill display text.
pub fn all_skill_display()
-> impl Iterator<Item = (&'static str, &'static str, &'static str, &'static str)> {
    REGISTRY.iter().map(|r| (r.def.id, r.name, r.blurb, r.detail))
}

#[cfg(test)]
mod registry_tests {
    use crate::skills::{unit_kit_tree, skill_def, skill_display};

    /// Every node in every class tree must resolve to a SkillDef AND a display
    /// name — catches a tree node added without its REGISTRY row (the exact
    /// 4-place-registration slip the table was built to prevent).
    #[test]
    fn every_tree_node_is_registered() {
        for class in ["swordsman_t1", "mage_t1", "archer_t1", "thief_t1",
                      "acolyte_t1", "assassin_t2"] {
            for node in unit_kit_tree(class) {
                assert!(skill_def(node.skill_id).is_some(),
                    "{} (in {}) has no SkillDef row", node.skill_id, class);
                assert!(!skill_display(node.skill_id).0.is_empty(),
                    "{} (in {}) has no display name", node.skill_id, class);
            }
        }
    }
}
