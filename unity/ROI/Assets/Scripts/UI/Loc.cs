// THE STRINGS TABLE — `i18n/strings.csv`, the Godot client's single source for every player-facing
// word, read here so the Unity client stops deriving English from ids.
//
// WHY IT MATTERS BEYOND TRANSLATION. Half the catalog's gear carries no `name` in `game_data.json`
// (115 of 432 defs): the name lives in this table as `ITEM_<def_id>`, which is why every screen in
// this client has been showing `shield_s1` where the Godot one shows "Bronze Shield". Item naming
// and localisation are the same job — there was never a way to do one without the other.
//
// KEY-ONLY, NO FALLBACK, which is the project's own rule: a missing key renders as the KEY, loudly,
// rather than quietly reverting to something invented here. A blank cell for the chosen language
// falls back to ENGLISH — that is a translation gap, not a missing string, and showing the key
// there would make an untranslated build unusable.
//
// Loaded once, from StreamingAssets, on first use. 1,700 rows is nothing to hold and the file is
// read exactly once per run.
using System.Collections.Generic;
using UnityEngine;

namespace Roi.UI
{
    public static class Loc
    {
        static Dictionary<string, string> _table;
        static string _lang = "en";

        /// The column to read. `en` until the client has a language picker; the table already
        /// carries eighteen.
        public static string Language
        {
            get => _lang;
            set { if (_lang != value) { _lang = value; _table = null; } }
        }

        /// The string for `key`, or the KEY itself when there is none. Never an invented default.
        public static string Get(string key)
        {
            if (string.IsNullOrEmpty(key)) return "";
            Load();
            return _table.TryGetValue(key, out var v) && !string.IsNullOrEmpty(v) ? v : key;
        }

        /// The string for `key`, or `fallback` when the table has none — for the places where a
        /// missing entry has a sensible answer already (an item whose name the CATALOG carries).
        public static string GetOr(string key, string fallback)
        {
            if (string.IsNullOrEmpty(key)) return fallback;
            Load();
            return _table.TryGetValue(key, out var v) && !string.IsNullOrEmpty(v) ? v : fallback;
        }

        /// A key whose string carries VALUES — "Lv {0}", "{0} of {1} done". Kept separate from
        /// `Get` so the guard can see it, and so a translator meets a placeholder rather than a
        /// sentence a client has already glued together in English word order.
        public static string Fmt(string key, params object[] args)
        {
            var s = Braces(Get(key));
            try { return string.Format(s, args); }
            // A translation with a stray brace must not take the screen down; showing the raw
            // string is the failure a translator can actually see and fix.
            catch (System.FormatException) { return s; }
        }

        /// `%d` / `%s` / `%.1f` -> `{0}` / `{1}` / `{2}`, in order.
        ///
        /// The table is SHARED with the Godot client, whose rows are written in GDScript's
        /// printf style, and rewriting them to C# braces would break every one of those call
        /// sites. Converting on this side is the only version of "one table, two clients" that
        /// does not mean two spellings of the same sentence.
        static string Braces(string s)
        {
            if (s == null || s.IndexOf('%') < 0) return s;
            var sb = new System.Text.StringBuilder(s.Length + 8);
            int n = 0;
            for (int i = 0; i < s.Length; i++)
            {
                if (s[i] != '%') { if (s[i] == '{' || s[i] == '}') sb.Append(s[i]); sb.Append(s[i]); continue; }
                if (i + 1 < s.Length && s[i + 1] == '%') { sb.Append('%'); i++; continue; }
                int j = i + 1;
                while (j < s.Length && (char.IsDigit(s[j]) || s[j] == '.')) j++;   // width/precision
                if (j < s.Length && (s[j] == 'd' || s[j] == 's' || s[j] == 'f')) { sb.Append('{').Append(n++).Append('}'); i = j; }
                else sb.Append('%');
            }
            return sb.ToString();
        }

        public static bool Has(string key)
        {
            Load();
            return !string.IsNullOrEmpty(key) && _table.ContainsKey(key);
        }

        static void Load()
        {
            if (_table != null) return;
            _table = new Dictionary<string, string>(2048);
            var path = System.IO.Path.Combine(Application.streamingAssetsPath, "strings.csv");
            if (!System.IO.File.Exists(path))
            {
                Debug.LogWarning("LOC: strings.csv is not in StreamingAssets — every key will "
                               + "render as itself.");
                return;
            }
            var lines = System.IO.File.ReadAllLines(path);
            if (lines.Length == 0) return;
            var head = SplitCsv(lines[0]);
            int en = System.Array.IndexOf(head, "en");
            int col = System.Array.IndexOf(head, _lang);
            if (en < 0) en = 2;
            if (col < 0) col = en;
            for (int i = 1; i < lines.Length; i++)
            {
                var f = SplitCsv(lines[i]);
                if (f.Length <= en || string.IsNullOrEmpty(f[0])) continue;
                // The chosen language, falling back to English for an untranslated row.
                var v = col < f.Length && !string.IsNullOrEmpty(f[col]) ? f[col] : f[en];
                _table[f[0]] = v;
            }
        }

        /// A CSV row, honouring quoted fields and doubled quotes inside them. Written out rather
        /// than split on commas: the table holds sentences, and a sentence with a comma in it is
        /// how a naive split silently shifts every column after it.
        static string[] SplitCsv(string line)
        {
            var outp = new List<string>();
            var sb = new System.Text.StringBuilder();
            bool q = false;
            for (int i = 0; i < line.Length; i++)
            {
                char c = line[i];
                if (q)
                {
                    if (c == '"')
                    {
                        if (i + 1 < line.Length && line[i + 1] == '"') { sb.Append('"'); i++; }
                        else q = false;
                    }
                    else sb.Append(c);
                }
                else if (c == '"') q = true;
                else if (c == ',') { outp.Add(sb.ToString()); sb.Clear(); }
                else sb.Append(c);
            }
            outp.Add(sb.ToString());
            return outp.ToArray();
        }
    }
}
