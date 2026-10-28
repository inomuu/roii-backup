// JSON view helpers for items — shared by the stash / escrow / rare-bag
// getters and the per-entity `equipped` enrichment in `view_state()`.
//
// These are pure FREE FUNCTIONS (no `#[func]`, no Godot types) so they live
// outside the `#[godot_api] impl` block; the bridge methods call into them.

use sim_core::items::Item;

/// Human "+N STAT" text for ONE `StatMod` — the card/overlay display vocabulary. Basis-point
/// families render as a percentage so a tooltip never shows a raw `300`.
pub fn stat_mod_text(m: &sim_core::items::StatMod) -> String {
    use sim_core::items::StatMod as S;
    let pct = |v: i32| format!("{}{}.{:02}%", if v < 0 { "-" } else { "+" }, v.abs() / 100, v.abs() % 100);
    match *m {
        // Flat amounts on the display scale (`crate::disp`).
        S::AtkFlat(v) => format!("{:+} ATK", crate::disp(v.into())),
        S::DefFlat(v) => format!("{:+} DEF", crate::disp(v.into())),
        // PLAYER-SIDE HP lines read on the sim scale, like the bar and every hit on a member
        // (`disp_for`, user 2026-09-18). On `disp` they read a tenth of what they do: Guard said
        // −2 while it took 20 off a 9, flooring it at 1 (user 2026-09-27).
        S::MaxHp(v) => format!("{:+} HP", crate::disp_for(true, v.into())),
        // A line whose stat no longer exists. It is worth nothing and says so.
        S::Retired(_) => "(retired line)".to_string(),
        S::MaxHpBp(v) => format!("{} MaxHP", pct(v)),
        S::AtkBp(v) => format!("{} ATK", pct(v)),
        S::CritBp(v) => format!("{} CRIT", pct(v)),
        S::CritDmgBp(v) => format!("{} CRIT DMG", pct(v)),
        S::HealBp(v) => format!("{} HEAL", pct(v)),
        S::IgnoreDefBp(v) => format!("{} Ignore DEF", pct(v)),
        S::CastBp(v) => format!("{} Cast Time", pct(-v)),
        S::CooldownBp(v) => format!("{} Cooldown", pct(-v)),
        S::MoveSpeedBp(v) => format!("{} Move Speed", pct(v)),
        S::DrVsRanged(v) => format!("{} DMG taken (ranged)", pct(-v)),
        S::DmgVsElement(el, v) => format!("{} DMG vs {el:?}", pct(v)),
        S::DmgVsRace(r, v) => format!("{} DMG vs {r:?}", pct(v)),
        S::DmgVsSize(sz, v) => format!("{} DMG vs {sz:?}", pct(v)),
        S::ResistElement(el, v) => format!("{} Resist {el:?}", pct(v)),
        S::DrVsRace(r, v) => format!("{} DMG taken from {r:?}", pct(-v)),
        S::DrVsSize(sz, v) => format!("{} DMG taken from {sz:?}", pct(-v)),
        S::ResistStatusBp(code, v) => format!("{} Resist status #{code}", pct(v)),
        S::ElementDmgBp(el, v) => format!("{} {el:?} DMG", pct(v)),
        S::DmgVsBossBp(v) => format!("{} Boss DMG", pct(v)),
        S::HpRegenFlat(v) => format!("{:+} HP Regen", crate::disp_for(true, v.into())),
        S::DefBp(v) => format!("{} DEF", pct(v)),
        S::ChargeRateBp(v) => format!("{} Charge Rate", pct(v)),
        S::BreakDmgBp(v) => format!("{} Break DMG", pct(v)),
        S::BreakBuildupBp(v) => format!("{} Break Build-up", pct(v)),
        S::DodgeBp(v) => format!("{} Dodge", pct(v)),
        S::XpGainBp(v) => format!("{} XP Gain", pct(v)),
        S::GilGainBp(v) => format!("{} Gil Gain", pct(v)),
        S::SoulGainBp(v) => format!("{} Soul Gain", pct(v)),
        S::DropRateBp(v) => format!("{} Drop Rate", pct(v)),
        S::DmgReduceFlat(v) => format!("{:+} Damage Taken", -crate::disp_for(true, v.into())),
    }
}

/// Serialize an item for the UI, augmenting it with the SIM's refine bonuses
/// (`refine_atk` / `refine_matk` / `refine_def`) so item tooltips show effective ATK/MATK/DEF without
/// re-deriving the `refine_level * 2` rule in GDScript — single source of truth.
/// The AFFIX WORDS an item's rolled bonuses earn, as localisation keys — `[prefix, suffix]`, either
/// half empty when nothing classifies.
///
/// The classification is `views/item_naming.gd`'s, moved here rather than copied into a second
/// client: `bonuses` is a serde-tagged enum list, which Unity's JsonUtility cannot type at all, so
/// the choice was the sim naming the words or the C# side hand-parsing the JSON to reach the same
/// table. One table in the shared host is the version that can stop drifting — the GDScript copy
/// stays for now and should eventually read these two fields instead.
///
/// PRESENTATION ONLY. It never changes what a bonus does; it decides which word decorates a name.
/// At most one of each, taken in the bonuses' stored order, which is canonical state — so every
/// client composes the same name for the same item with no display-time randomness.
fn affix_keys(it: &Item) -> (String, String) {
    use sim_core::items::{ItemKind, StatMod as S};
    if !matches!(it.kind, ItemKind::Weapon | ItemKind::Armor) || it.unique {
        return (String::new(), String::new());
    }
    // SHORTER THAN IT LOOKS, ON PURPOSE. The gacha pivot deleted the six RO attributes and the
    // MATK / HIT / FLEE / ignore-MDEF lines from `StatMod`, so the arms that named them went with
    // them — `AFFIX_SUF_str` and friends are gone from strings.csv too. Both matches end in `_ =>
    // String::new()`, so the lines the pivot ADDED (break, charge rate, dodge, element damage)
    // fall through and simply do not contribute a name yet. That is a naming gap to fill when
    // those lines get affix words, not a compile error waiting to happen.
    let mut pre = String::new();
    let mut suf = String::new();
    for b in &it.bonuses {
        if pre.is_empty() {
            pre = match b {
                S::AtkBp(_) | S::AtkFlat(_) => "AFFIX_PRE_atk".into(),
                S::CritBp(_) | S::CritDmgBp(_) => "AFFIX_PRE_crit".into(),
                S::IgnoreDefBp(_) => "AFFIX_PRE_pierce".into(),
                S::DmgVsRace(..) => "AFFIX_PRE_vs_race".into(),
                S::DmgVsSize(..) => "AFFIX_PRE_vs_size".into(),
                // Parameterised on the element, exactly as the client table is.
                S::DmgVsElement(el, _) => format!("AFFIX_PRE_elem_{}", format!("{el:?}").to_lowercase()),
                _ => String::new(),
            };
        }
        if suf.is_empty() {
            suf = match b {
                S::MaxHp(_) | S::MaxHpBp(_) => "AFFIX_SUF_hp".into(),
                S::MoveSpeedBp(_) => "AFFIX_SUF_move".into(),
                S::ResistStatusBp(..) => "AFFIX_SUF_status".into(),
                S::DrVsRace(..) | S::DrVsSize(..) => "AFFIX_SUF_dr".into(),
                S::DrVsRanged(_) => "AFFIX_SUF_ranged".into(),
                S::CooldownBp(_) => "AFFIX_SUF_cdr".into(),
                S::CastBp(_) => "AFFIX_SUF_cast".into(),
                S::HealBp(_) => "AFFIX_SUF_heal".into(),
                S::ResistElement(el, _) => format!("AFFIX_SUF_res_{}", format!("{el:?}").to_lowercase()),
                _ => String::new(),
            };
        }
        if !pre.is_empty() && !suf.is_empty() {
            break;
        }
    }
    (pre, suf)
}

pub fn item_to_view(it: &Item) -> serde_json::Value {
    let mut v = serde_json::to_value(it).unwrap_or(serde_json::Value::Null);
    if let Some(o) = v.as_object_mut() {
        // GRADED stats (0.0.372) — what the FIGHT reads, base × the grade multiplier. The raw
        // `atk_bonus` / `def_bonus` fields are still in the serialized item beside these, and a
        // tooltip that shows those instead would print the authored number while the character
        // sheet prints the real one. The sim scales; the UI prints.
        // On the DISPLAY scale (`crate::disp`): the raw `atk_bonus` / `def_bonus` beside these
        // are the sim's numbers; a client prints these.
        o.insert("graded_atk".into(), serde_json::json!(crate::disp(it.graded_atk().into())));
        o.insert("graded_def".into(), serde_json::json!(crate::disp(it.graded_def().into())));
        o.insert("graded_atk_speed_bp".into(), serde_json::json!(it.graded_atk_speed_bp()));
        // The ladder POSITION as a number (0 = the floor), so a client can colour or sort by
        // grade without learning the name list — the names are still provisional.
        o.insert("grade_step".into(), serde_json::json!(it.grade.step()));
        // AUTHORED sub-stat lines (def × grade). Serialized beside the instance's own `bonuses`
        // rather than merged into them: one set is what the PRODUCT is (and is identical on every
        // copy, which is what a market matches on), the other is what this OWNER did to it.
        // The PRODUCT key (def × grade) — what a market must match listings on. `def_id` alone
        // would pool a Common and a Beyond under one name; see `Item::product_key`.
        o.insert("product_key".into(), serde_json::json!(it.product_key()));
        // The grade's AUTHORED lines, as text through the same formatter as `bonus_lines`, so the
        // two read alike and the client never types a StatMod.
        let authored: Vec<String> = it.lines.iter().map(stat_mod_text).collect();
        o.insert("authored_lines".into(), serde_json::json!(authored));
        // GEMS (0.0.386): how many sockets this piece has at its grade (after its card slots),
        // and each socketed gem's rolled lines as text — `gems` itself is serialized beside these
        // as the tagged enum, for the same reason `bonuses` is.
        o.insert("gem_slots".into(), serde_json::json!(it.gem_slots()));
        let gem_lines: Vec<Vec<String>> = it.gems.iter().map(|g| g.lines.iter().map(stat_mod_text).collect()).collect();
        o.insert("gem_lines".into(), serde_json::json!(gem_lines));
        o.insert("gem_line_groups".into(), serde_json::json!(gem_lines.iter().map(|lines| serde_json::json!({"lines":lines})).collect::<Vec<_>>()));
        o.insert("market_clean".into(), serde_json::json!(it.market_clean()));
        // Item level (the def's _sN badge) + its equip level requirement —
        // the sim's formula, never re-derived in GDScript.
        o.insert("item_level".into(), serde_json::json!(it.item_level()));
        // UNIQUE, as the FLAG rather than the id's spelling. The client had to infer it from
        // `def_id.begins_with("unique_")` in three places because this key did not exist — and the
        // sim's own rule reads `Item.unique`, so the two sides
        // were answering the same question by different means. Adding a key is free; the contract
        // test guards LOSS only.
        o.insert("unique".into(), serde_json::json!(it.unique));
        o.insert("level_req".into(), serde_json::json!(it.level_req()));
        // WHO PREFERS THIS: the characters whose weapon family includes it
        // (`items::weapon_types_for_character`). Anyone can wear it (2026-09-29); outside this list
        // it gives 80% of its own ATK/DEF. Empty means no family (armour, bare-hand types).
        o.insert(
            "wieldable".into(),
            serde_json::json!(if matches!(it.kind, sim_core::items::ItemKind::Weapon) {
                sim_core::items::units_that_can_wield(it.weapon_type)
            } else {
                Vec::new()
            }),
        );
        // OFF-HAND KIND (0.0.446): "Shield" / "Tome" / "Quiver" / "HiddenKnife", "None" for
        // anything that is not an off-hand piece. The item itself carries it (`Item.offhand_type`
        // is serialized skip-if-None, so the raw field is absent on most items); this string is
        // always present so a tooltip can key on it without a null check.
        o.insert("offhand_type".into(), serde_json::json!(it.offhand_type.key()));
        // and who may carry an off-hand piece — the units whose family has that kind.
        if !it.offhand_type.is_none() {
            o.insert(
                "wieldable".into(),
                serde_json::json!(sim_core::items::units_that_can_carry(it.offhand_type)),
            );
        }
        let (pre, suf) = affix_keys(it);
        o.insert("affix_pre".into(), serde_json::json!(pre));
        o.insert("affix_suf".into(), serde_json::json!(suf));
        // THE ROLLED LINES AS TEXT, for a tooltip. `bonuses` is a tagged enum; the vocabulary that
        // turns one into "+3.00% CRIT" is `stat_mod_text`, and it already serves the card and
        // overlay lines above. Emitting it here is what stops a second client re-implementing the
        // same formatter — which is how a tooltip and a stat sheet start disagreeing.
        if !it.bonuses.is_empty() {
            let lines: Vec<String> = it.bonuses.iter().map(stat_mod_text).collect();
            o.insert("bonus_lines".into(), serde_json::json!(lines));
            // Which of them is LOCKED (a Death Star Stone's mark), so the tooltip can say so
            // rather than showing five identical rows one of which cannot be rerolled.
            o.insert("locked_slot".into(), serde_json::json!(it.locked_slot));
        }
        // The canonical equip SLOT (Item::slot() — never a UI guess), so the
        // comparison matches the RIGHT equipped piece.
        o.insert("slot".into(), serde_json::json!(it.slot()));
        // Actual card sockets (weapons capped by weapon level) → the UI's per-class
        // activation cap. Cards live on the character's card panel now, not in the item.
        o.insert("socket_count".into(), serde_json::json!(it.socket_count()));
        // CARD special display values (0.0.218): resolved by the sim so EVERY card
        // tooltip (inventory / album / stash / shop) can render the special line
        // without the bridge — one source, the UI only formats. Empty for cards
        // with no special / non-cards → the key is simply absent.
        if matches!(it.kind, sim_core::items::ItemKind::Card) {
            let vals = sim_core::combat::card_specials::display_values(&it.def_id, it.star);
            if !vals.is_empty() {
                o.insert("special_vals".into(), serde_json::json!(vals));
            }
            // CARD stat lines ALREADY RESOLVED at this instance's star (0.0.243): the sim
            // owns the star ladder, the UI only prints. Split epic vs foil-extra so the
            // tooltip can keep its "what the shiny adds" section.
            // MERGE same-family lines first: the star ladder emits one entry per source
            // (base ++ per-star step ++ each rank bump), so an un-merged Brutus at 5★ would
            // read "+5 STR +1 STR +1 STR +1 STR" instead of "+8 STR".
            let lines = |mods: Vec<sim_core::items::StatMod>| -> Vec<String> {
                sim_core::items::StatMod::merged(&mods)
                    .iter()
                    .map(stat_mod_text)
                    .collect()
            };
            let epic = lines(it.card_mods_at_star());
            if !epic.is_empty() {
                o.insert("card_lines".into(), serde_json::json!(epic));
            }
            let foil = lines(it.foil_mods_at_star());
            if !foil.is_empty() {
                o.insert("card_foil_lines".into(), serde_json::json!(foil));
            }
            // What BINDING this card grants permanently (0.0.245 — `bind_mods`, never
            // star-scaled). Same resolved-string contract as the socket lines.
            let bind = lines(it.bind_mods.clone());
            if !bind.is_empty() {
                o.insert("bind_lines".into(), serde_json::json!(bind));
            }
        }
        // OVERLAY carriers (Exemplar / Regalia) describe themselves the same way a card does:
        // the bridge resolves the numbers, the tooltip only prints. Without this the shop showed
        // a name and a price and nothing else, so a hero and a weapon were indistinguishable
        // from each other and from a rock.
        //
        // Base, per-star and per-level are SEPARATE lists rather than one blob, because the
        // three answer different questions — what it is now, what a duplicate buys, what fodder
        // buys — and a player deciding whether to pull needs them apart.
        overlay_view(&it.def_id, it.resonance_stars, it.enhance_level, o);
    }
    v
}

/// Resolved display data for an Exemplar / Regalia def id. No-op for anything else.
fn overlay_view(def_id: &str, stars: u8, level: u8, o: &mut serde_json::Map<String, serde_json::Value>) {
    use sim_core::items::StatMod;
    let lines = |mods: &[StatMod]| -> Vec<String> {
        StatMod::merged(mods).iter().map(stat_mod_text).collect()
    };
    // Skills go across as id + level, never as text: SkillDef has no name field and the client
    // resolves display names through `Loc`, so an English string invented here would bypass
    // localisation entirely.
    let skills = |gs: &[sim_core::items::GrantedSkill]| -> Vec<serde_json::Value> {
        gs.iter().map(|g| serde_json::json!({"id": g.id, "level": g.level})).collect()
    };

    if let Some(def) = sim_core::items::exemplar_def(def_id) {
        o.insert("overlay_kind".into(), serde_json::json!("exemplar"));
        o.insert("overlay_lines".into(), serde_json::json!(lines(&def.base_mods)));
        if let Some(first) = def.star_ranks.first() {
            o.insert("overlay_star_lines".into(), serde_json::json!(lines(&first.mods)));
        }
        o.insert("overlay_skills".into(), serde_json::json!(skills(&def.base_skills)));
        // 0.0.445: an Exemplar IS its unit — there is no class to lock it to. Kept as an empty
        // list so both clients' payload shape holds (Godot `item_slot.gd`, Unity
        // `ExemplarRegaliaPage`).
        o.insert("overlay_classes".into(), serde_json::json!(Vec::<&str>::new()));
        // RESONANCE S1..S6 (0.0.441): six lists of `{key, args, skill}` lines — loc key
        // `RANK_*`, numbers, and the skill id the line names (empty when none).
        o.insert("overlay_ranks".into(), serde_json::json!(sim_core::unit_rank::describe(&def.id)));
    } else if let Some(def) = sim_core::items::regalia_def(def_id) {
        o.insert("overlay_kind".into(), serde_json::json!("regalia"));
        // Lv1 lines, and the same lines at the level cap (`overlay_level_lines`, 0.0.400) so a
        // tooltip can print "→ at Lv60"; the rank ladder is empty and says so.
        o.insert("overlay_lines".into(), serde_json::json!(lines(&def.mods_at(1, 0))));
        o.insert("overlay_level_lines".into(), serde_json::json!(if def.lump_base > 0 { lines(&def.mods_at(sim_core::items::REGALIA_MAX_LEVEL, 0)) } else { Vec::<String>::new() }));
        o.insert("overlay_level_cap".into(), serde_json::json!(sim_core::items::REGALIA_MAX_LEVEL));
        // RESONANCE READS S1..S5 (0.0.486): `resonance_stars` 0..4 is S1..S5 — the client prints
        // `resonance_s`, never the raw count.
        o.insert("resonance_s".into(), serde_json::json!(stars.min(sim_core::items::REGALIA_MAX_STARS) + 1));
        o.insert("resonance_s_max".into(), serde_json::json!(sim_core::items::REGALIA_MAX_STARS + 1));
        // PRIMARY / SECONDARY MOD (0.0.485, user 2026-09-27 — the ATK lump and the advanced stat;
        // the Passive is the third part): each resolved at THIS carrier's level and at the cap,
        // so a card nobody wears still shows its real numbers without the client doing the curve.
        let primary = |lv: u8| (def.lump_base > 0).then(|| {
            stat_mod_text(&StatMod::AtkFlat(sim_core::items::regalia_lump_at(def.lump_base, lv)))
        });
        let secondary = |lv: u8| def.advanced.map(|adv| {
            stat_mod_text(&adv.with_value(sim_core::items::regalia_advanced_at(adv.value(), lv)))
        });
        let cap = sim_core::items::REGALIA_MAX_LEVEL;
        o.insert("primary_mod".into(), serde_json::json!(primary(level)));
        o.insert("primary_mod_max".into(), serde_json::json!(primary(cap)));
        o.insert("secondary_mod".into(), serde_json::json!(secondary(level)));
        o.insert("secondary_mod_max".into(), serde_json::json!(secondary(cap)));
        o.insert("overlay_star_lines".into(), serde_json::json!(def.star_ranks.first().map(|r| lines(&r.mods)).unwrap_or_default()));
        o.insert("overlay_skills".into(), serde_json::json!(skills(&def.base_skills)));
        // INTENDED, not allowed: a Regalia is equippable by anyone; the tooltip says which
        // DOCTRINE it was built for — its passive works only on a wearer of that doctrine.
        o.insert(
            "overlay_doctrine".into(),
            serde_json::json!(sim_core::items::regalia_doctrine(&def.id).map(|d| d.id()).unwrap_or("")),
        );
        o.insert("overlay_classes".into(), serde_json::json!(Vec::<&str>::new()));
        // The PASSIVE (0.0.435), as loc keys + numbers at this carrier's stars: `{0}` now, `{1}` at
        // S5, `{2}` the extra (seconds / count / range / threshold). Empty for R / SR pieces.
        o.insert(
            "overlay_passive".into(),
            serde_json::json!(sim_core::regalia_passive::describe(&def.id, stars)),
        );
    }
}

#[cfg(test)]
mod tests {
    /// PRIMARY / SECONDARY MOD (0.0.485): resolved at the carrier's own level and at the cap, on
    /// the display scale — the client prints them and never runs the curve itself.
    #[test]
    fn a_regalia_carries_its_primary_and_secondary_mod_at_its_level() {
        let mut it = Item::new("regalia_hearthguard", ItemKind::Regalia, 0, 0, Rarity::Rare);
        let v = super::item_to_view(&it);
        assert_eq!(v["primary_mod"], "+10 ATK", "100 raw at Lv1 on the ÷10 display scale");
        assert_eq!(v["primary_mod_max"], "+180 ATK", "the authored 1,800 raw at the Lv70 cap");
        assert_eq!(v["secondary_mod"], super::stat_mod_text(&sim_core::items::StatMod::DefBp(360)));
        assert_eq!(v["secondary_mod_max"], super::stat_mod_text(&sim_core::items::StatMod::DefBp(1_050)));
        it.enhance_level = sim_core::items::REGALIA_BASE_CAP;
        assert_eq!(super::item_to_view(&it)["primary_mod"], "+150 ATK", "a levelled card reads its own level (1,503 at Lv60)");
    }

    /// PLAYER-SIDE HP lines read on the sim scale, as the bar and every hit on a member do — a
    /// Guard that takes 20 off a hit must not say 2 (user 2026-09-27). ATK / DEF stay on `disp`.
    #[test]
    fn player_side_hp_lines_read_unscaled() {
        use sim_core::items::StatMod as S;
        assert_eq!(super::stat_mod_text(&S::DmgReduceFlat(20)), "-20 Damage Taken");
        assert_eq!(super::stat_mod_text(&S::MaxHp(50)), "+50 HP");
        assert_eq!(super::stat_mod_text(&S::HpRegenFlat(30)), "+30 HP Regen");
        assert_eq!(super::stat_mod_text(&S::AtkFlat(20)), "+2 ATK");
    }

    use sim_core::items::{Item, ItemKind, Rarity};

    /// The tooltip reads these keys with `.get(key, default)`, so a renamed or dropped one blanks
    /// a section instead of erroring — the exact silent failure CLAUDE.md warns about for the FFI
    /// boundary. Pin the SHAPE and that the content is non-empty, because a key that exists and
    /// carries nothing looks identical in game to a key that is missing.
    #[test]
    fn overlay_carriers_describe_themselves() {
        let cases = [
            ("regalia_hearthguard", "regalia", true),   // R weapon: has a per-LEVEL axis
            ("exemplar_seraphine", "exemplar", false),  // SSR hero: stars only, and has skills
        ];
        for (def_id, want_kind, has_levels) in cases {
            let item = Item::new(def_id, ItemKind::Regalia, 0, 0, Rarity::Legendary);
            let v = super::item_to_view(&item);
            let o = v.as_object().expect("object");

            assert_eq!(o.get("overlay_kind").and_then(|x| x.as_str()), Some(want_kind), "{def_id}");
            let a = o.get("overlay_lines").and_then(|x| x.as_array())
                .unwrap_or_else(|| panic!("{def_id}: overlay_lines missing"));
            assert!(!a.is_empty(), "{def_id}: overlay_lines is present but empty");
            // A Regalia names its DOCTRINE (0.0.436); the class list is retired and stays empty.
            if want_kind == "regalia" {
                assert!(o.get("overlay_doctrine").and_then(|x| x.as_str()).is_some_and(|d| !d.is_empty()), "{def_id}: overlay_doctrine");
            }
            assert_eq!(
                o.get("overlay_level_lines").and_then(|x| x.as_array()).is_some_and(|a| !a.is_empty()),
                has_levels,
                "{def_id}: only a Regalia takes Enhancement levels"
            );
        }

        // The two authored heroes carry skills; a stat-stick must not claim to.
        let sk = |id: &str| {
            let v = super::item_to_view(&Item::new(id, ItemKind::Exemplar, 0, 0, Rarity::Legendary));
            v.as_object().unwrap().get("overlay_skills").and_then(|x| x.as_array()).cloned().unwrap_or_default().len()
        };
        assert!(sk("exemplar_seraphine") > 0, "Seraphine's authored kit vanished");
        assert_eq!(sk("exemplar_rowena"), 0, "a stat stick must not list skills");

        // Plain gear must not grow overlay keys.
        let sword = super::item_to_view(&Item::new("sword_short", ItemKind::Weapon, 5, 0, Rarity::Common));
        assert!(sword.as_object().unwrap().get("overlay_kind").is_none());
    }
}
