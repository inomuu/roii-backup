using UnityEngine;
// GAME VOCABULARY — the display name of a thing whose id is not its name. One file, because these
// are decisions about what the player is told, and the same class or element appears on half a
// dozen screens.
//
// Godot resolves all of this through the loc table (`CLASS_NAME_<id>`, and the element keys). There
// is no loc layer on the Unity side yet, so these derive the English from the id. That is honest
// scaffolding, not a design: when the loc layer lands, these functions are the ONE place that has
// to start asking it.
namespace Roi.UI
{
    public static class GameText
    {
        /// An item's DISPLAY NAME: `[Prefix] Base [of Suffix]`, composed from the strings table.
        /// Ported from `item_naming.gd`, and it is the reason this client stopped showing
        /// `shield_s1` — the base name lives in the table as `ITEM_<def_id>` for the 115 catalog
        /// defs that carry no name of their own.
        ///
        /// The affix WORDS are chosen by the sim (`affix_pre` / `affix_suf` on the item view): the
        /// bonuses list is a tagged enum JsonUtility cannot type, so the classification lives once
        /// in the shared host instead of twice in two clients.
        ///
        /// PURE. The same item composes the same name everywhere, with no display-time randomness
        /// and nothing stored.
        public static string ItemName(ItemView it)
        {
            if (it == null) return "";
            var basename = Loc.GetOr("ITEM_" + it.def_id,
                                     string.IsNullOrEmpty(it.name) ? it.def_id : it.name);
            var pre = string.IsNullOrEmpty(it.affix_pre) ? "" : Loc.GetOr(it.affix_pre, "");
            var suf = string.IsNullOrEmpty(it.affix_suf) ? "" : Loc.GetOr(it.affix_suf, "");
            if (!string.IsNullOrEmpty(pre)) basename = pre + " " + basename;
            if (!string.IsNullOrEmpty(suf)) basename = basename + " " + suf;
            return GradePrefix(it.grade_step) + basename;
        }

        /// GRADE names, ladder order. The prefix is part of the NAME, not decoration: a Common and
        /// a Beyond of one def differ by ×4.8 in power, and a market that matches listings by name
        /// would otherwise treat them as one product — the cheapest listing would always be the
        /// weakest copy and no buyer could tell them apart. Every grade above the floor says so in
        /// the name; Common says nothing, because "Common Short Sword" is noise on the item every
        /// player is holding forty of.
        /// Ladder order (decided 2026-09-15): Common · Fine · Rare · Elite · Epic · Royal · Legendary ·
        /// Ancient · Divine · Eternal — the words players already rank (Rare < Epic < Legendary) with
        /// the launch ceiling landing on Legendary. Display only: the Rust `Grade` variants keep their
        /// serialized names. Text comes from `GRADE_<step>` so every language names them.
        const int Grades = 10;

        static string GradePrefix(int step)
            => step > 0 && step < Grades ? GradeName(step) + " " : "";
        /// The grade's name alone, for a badge beside an item rather than in its name.
        public static string GradeName(int step)
            => step >= 0 && step < Grades ? Loc.GetOr("GRADE_" + step, "") : "";
        /// "Tier 3 - Rare": the number AND the word, wherever a grade is explained rather than
        /// worn as a prefix (user 2026-09-15: players rank a number instantly, the word is the
        /// colour they will learn).
        public static string GradeLabel(int step)
            => step >= 0 && step < Grades ? Loc.Fmt("UI_GRADE_TIER", step + 1, GradeName(step)) : "";

        /// The colour of each grade step, ladder order — grey · green · blue · purple · pink ·
        /// gold · orange · red · white-pink · space violet-black. Tier 2 starts green so the first
        /// upgrade is immediately distinct from Tier 1; rarity colours (`ItemSlot.RarityColor`)
        /// are for the kinds that have no grade.
        static readonly Color[] GradeColors =
        {
            new Color(0.62f, 0.62f, 0.66f), new Color(0.42f, 0.80f, 0.45f), new Color(0.36f, 0.62f, 0.98f),
            new Color(0.70f, 0.42f, 0.98f), new Color(1.00f, 0.48f, 0.80f), new Color(1.00f, 0.72f, 0.24f),
            new Color(1.00f, 0.50f, 0.16f), new Color(0.96f, 0.26f, 0.26f), new Color(1.00f, 0.78f, 0.90f),
            new Color(0.42f, 0.08f, 0.36f),
        };
        public static Color GradeColor(int step)
            => step >= 0 && step < GradeColors.Length ? GradeColors[step] : GradeColors[0];

        /// The same, for the places that hold only an id and a raw name (a mail row, a shop row,
        /// a market listing) — no affixes, because there are no bonuses to read.
        public static string ItemName(string defId, string rawName)
            => Loc.GetOr("ITEM_" + defId, string.IsNullOrEmpty(rawName) ? defId : rawName);

        /// "swordsman_t1" -> "Swordsman". The tier suffix is dropped rather than shown: a 2nd-tier
        /// job carries its own id and its own name, so "_t2" is never what distinguishes two
        /// entries on a screen.
        public static string UnitName(string id)
        {
            if (string.IsNullOrEmpty(id)) return "?";
            // The TABLE first. Deriving the name from the id made `swordsman_t1` read as
            // "Swordsman" in every language, and made the id — which is the save key of
            // `class_characters` — the only way to change what a player sees.
            var loc = Loc.GetOr("CLASS_NAME_" + id, "");
            if (!string.IsNullOrEmpty(loc)) return loc;
            var s = id;
            int u = s.LastIndexOf('_');
            if (u > 0 && u + 1 < s.Length && s[u + 1] == 't') s = s.Substring(0, u);
            return Title(s.Replace('_', ' '));
        }

        /// The elements as the PLAYER sees them. `Neutral` is shown as **Physical** — decided
        /// 2026-09-07 with the gacha-idle pivot, and it is a DISPLAY rename only: the Rust variant
        /// stays `Neutral` because it is the serde default and is written into save files and into
        /// game_data.json. Renaming it at the source would invalidate every save.
        public static string ElementName(string id)
            => string.IsNullOrEmpty(id) ? ""
             : string.Equals(id, "Neutral", System.StringComparison.OrdinalIgnoreCase) ? "Physical"
             : Title(id.Replace('_', ' '));

        /// The name a SCREEN is shown under, where the game calls it something other than what the
        /// code does. "Summon" is what the bar entry, the window key and the server's endpoints all
        /// call it; **Excavation** is what the GAME calls it (user, 2026-09-07).
        ///
        /// A display map rather than a rename, for the same reason `Neutral` still renders as
        /// "Physical": the internal name is load-bearing. It keys the window dictionary, it matches
        /// the Godot client's node so the two ports stay comparable, and the server's routes are
        /// `/gacha/*` regardless of what the fiction calls it. Renaming the id to match the fiction
        /// is how a screen becomes hard to find in its own codebase.
        /// A Commander skill's display name. A TABLE, copied from `render_data.COMMANDER_SKILL_UI`,
        /// rather than a key built out of the id: deriving "UI_CMDR_" + id would tie the loc key to
        /// the id's spelling, and a rename in Rust would then silently blank the button.
        ///
        /// The raw id is a deliberate last resort — a skill the sim can cast but this client has no
        /// name for must still be pressable, and reading `commander_smite` on a button is a legible
        /// bug report. Dropping the row would hide a shipped skill instead.
        public static string CommanderSkillName(string id) => id switch
        {
            "commander_smite" => Loc.GetOr("UI_CMDR_SMITE", id),
            _ => id,
        };

        public static string ScreenName(string id) => id switch
        {
            "Skills" => Loc.Get("UI_POTENTIAL_TITLE"),
            "Summon" => "Excavation",
            _ => id,
        };

        /// GAME VOCABULARY — a thing whose in-fiction name differs from the one the code and the
        /// server use. The server's wallet, its routes and its JSON all say `crystal`; the game calls
        /// the currency **Crystal** (user, 2026-09-07), which is also the name of the screen you
        /// buy it on.
        ///
        /// Singular on purpose. It is a mass noun the way Gil and Soul already are here — "300
        /// Crystal", not "300 Crystals" — so a count never has to agree with it.
        ///
        /// Same rule as `ScreenName`: display only. Renaming the server's field would mean a
        /// migration, a version bump and a second name for one number.
        public const string Crystal = "Crystal";

        /// What a class DOES, in one line, for the founding screen. Copied verbatim from
        /// `creation_view.gd`'s CLASS_DESC — the wording is the Godot client's and the two ports
        /// should describe the same class the same way. An unknown id answers empty rather than a
        /// guess: a made-up description of a class this build does not know is worse than none.
        public static string ClassDescription(string id) => id switch
        {
            "swordsman_t1" => "Frontline tank - high HP & STR, soaks hits and hits hard up close.",
            "archer_t1" => "Ranged DPS - AGI & DEX for fast, accurate arrows from a safe distance.",
            "mage_t1" => "Burst caster - INT & DEX for powerful elemental bolts, but fragile.",
            "thief_t1" => "Nimble striker - high AGI to dodge and land frequent double attacks.",
            "acolyte_t1" => "Party support - INT & VIT to heal, buff, and outlast the fight.",
            _ => "",
        };

        /// A skill id as a name — "commander_smite" -> "Smite". The `commander_` prefix is dropped
        /// because the screen it appears on is the Commander's; repeating it in every row is noise.
        public static string SkillName(string id)
        {
            if (string.IsNullOrEmpty(id)) return "";
            var s = id.StartsWith("commander_") ? id.Substring("commander_".Length) : id;
            return Title(s.Replace('_', ' '));
        }

        /// What a STAR STONE does, in the fewest words that stay true. The sim names the effect
        /// (`reroll_all`, `star_up` with the star it needs, `insert_rune` with the class); this is
        /// the one place those become a line a player reads.
        public static string OrbEffect(string effect, string arg) => effect switch
        {
            "reroll_all" => "Rerolls every line",
            "reroll_values" => "Rerolls the numbers, keeps the lines",
            "lock_random" => "Locks one random line",
            // WORDS, not a star glyph: the UI face (Noto Sans) has no ★ and draws a box. The item
            // cells show stars as drawn pips for the same reason.
            "star_up" => $"Raises a {arg}-star piece to {Plus(arg)}",
            "insert_rune" => $"Inserts a {UnitName(arg)} skill rune",
            _ => "",
        };

        static string Plus(string n) => int.TryParse(n, out var v) ? (v + 1).ToString() : n;

        /// A BOUND grant's kind as a word. The server names these with its own keys
        /// (`gacha_coupon`, `mileage`); an unknown one falls back to the key made readable rather
        /// than to a guess, so a new kind reads as itself instead of as nothing.
        public static string BoundKind(string kind) => kind switch
        {
            "gacha_coupon" => "Coupon",
            "mileage" => "Mileage",
            "crystal" => Crystal,
            _ => string.IsNullOrEmpty(kind) ? "" : Title(kind.Replace('_', ' ')),
        };

        static string Title(string s)
        {
            var parts = s.Split(' ');
            for (int i = 0; i < parts.Length; i++)
                if (parts[i].Length > 0)
                    parts[i] = char.ToUpperInvariant(parts[i][0]) + parts[i].Substring(1);
            return string.Join(" ", parts);
        }
    }
}
