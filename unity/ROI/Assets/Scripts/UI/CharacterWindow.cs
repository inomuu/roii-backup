// THE CHARACTER SHEET — the paper-doll on the left, the numbers on the right, for whichever member
// the tabs have picked. Ported from the left half of godot/views/character_view.gd.
//
// The bag half of that Godot window is NOT here and will not be: the user split them
// (2026-09-07), so Inventory is the bag and this is the character. Same reason the paper-doll is
// its own component — the Blacksmith needs the doll without needing this screen.
//
// EVERY NUMBER COMES FROM THE SIM, through the `stats:<entity_id>` query. Not one of them is
// recomputed here. The Godot view had the same rule and it matters more than it looks: `atk` is a
// function of gear, stars, buffs, cards, passives and paths, and a client that added
// those up itself would eventually print a number the fight does not use.
//
// The two halves of each of the six are both shown — the base the character owns and the delta
// buffs and gear add — because "STR 12" and "STR 12 +8" are different facts and only one of them
// survives unequipping.
//
// NOT PORTED: stat ALLOCATION (the +/- buttons and the reset). `AllocateStatPoint` exists and is
// replay-safe, but docs/skill-system-redesign.md is redoing progression around a Potential Board,
// and buttons that spend a currency the redesign may delete are the wrong thing to build today.
// The unspent pool is shown, so nothing is hidden.
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    public sealed class CharacterWindow : MonoBehaviour
    {
        const float ColGap = 28f, RowH = 26f, HeadH = 26f;

        Roi.Sim.SimDriver _driver;
        RectTransform _body, _right;
        PaperDoll _doll;
        RegaliaCrown _regalia;
        TextMeshProUGUI _title;
        readonly System.Collections.Generic.List<GameObject> _built = new();
        string _fingerprint = "";
        long _entityId;

        public static CharacterWindow Attach(RectTransform body, string footnote)
        {
            var w = body.gameObject.AddComponent<CharacterWindow>();
            w.Build(body, footnote);
            return w;
        }

        void OnEnable() => PartySelection.Changed += OnPicked;
        void OnDisable() => PartySelection.Changed -= OnPicked;

        void OnPicked()
        {
            _fingerprint = "";
            if (isActiveAndEnabled) Refresh();
        }

        void Build(RectTransform body, string footnote)
        {
            _body = body;
            float pad = UiTheme.PadLarge;

            _title = UiTheme.Label(body, "Who", "", UiTheme.HeadSize,
                                   UiTheme.TextHigh, TextAlignmentOptions.Left);
            var tr = _title.rectTransform;
            tr.anchorMin = new Vector2(0f, 1f); tr.anchorMax = new Vector2(1f, 1f);
            tr.pivot = new Vector2(0.5f, 1f);
            tr.offsetMin = new Vector2(pad, -40f); tr.offsetMax = new Vector2(-pad, -UiTheme.PadSmall);

            _doll = PaperDoll.Attach(body, sideLabels: true);
            var dr = _doll.Rect;
            dr.anchorMin = dr.anchorMax = new Vector2(0f, 1f);
            dr.pivot = new Vector2(0f, 1f);
            dr.anchoredPosition = new Vector2(pad, -(40f + UiTheme.Gap));

            // The worn REGALIA, under the paper-doll. It is not a gear slot — a Regalia is an
            // overlay in its own slot, not a piece in the doll's grid — so it sits beside the doll
            // rather than in it.
            _regalia = RegaliaCrown.Create(body);
            var cr = _regalia.Rect;
            cr.anchorMin = cr.anchorMax = new Vector2(0f, 1f);
            cr.pivot = new Vector2(0f, 1f);
            cr.anchoredPosition = new Vector2(pad + (PaperDoll.Width - RegaliaCrown.Size) * 0.5f,
                                              -(40f + UiTheme.Gap + PaperDoll.Height + UiTheme.Gap));

            // SCROLLED, because the list grows: the redesign adds rows and a tall phone screen is
            // not a promise. Clipping the last two silently is how MITIGATION and MAX HP went
            // missing from the first capture.
            var rightGo = new GameObject("Numbers", typeof(RectTransform));
            rightGo.transform.SetParent(body, false);
            var host = (RectTransform)rightGo.transform;
            host.anchorMin = new Vector2(0f, 0f); host.anchorMax = new Vector2(1f, 1f);
            host.offsetMin = new Vector2(pad + PaperDoll.Width + ColGap, 40f);
            host.offsetMax = new Vector2(-pad, -(40f + UiTheme.Gap));
            _right = UiScroll.Create(host, out _);

            var note = UiTheme.Label(body, "Note", footnote, UiTheme.CaptionSize,
                                     UiTheme.TextLow, TextAlignmentOptions.BottomLeft);
            var nr = note.rectTransform;
            nr.anchorMin = Vector2.zero; nr.anchorMax = new Vector2(1f, 0f);
            nr.pivot = new Vector2(0.5f, 0f);
            nr.offsetMin = new Vector2(pad, UiTheme.PadSmall);
            nr.offsetMax = new Vector2(-pad, UiTheme.PadSmall + 22f);
        }

        public void Refresh()
        {
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var session = _driver != null ? _driver.Session : null;
            if (session == null) { _title.text = Loc.Get("UI_CHARACTER_NO_SESSION"); return; }

            var view = JsonUtility.FromJson<CharView>(session.ViewJson());
            CharEntity me = null;
            var ids = new System.Collections.Generic.List<long>();
            if (view != null && view.grid != null && view.grid.entities != null)
                foreach (var e in view.grid.entities)
                    if (e.team == "Player" && !e.is_pet) ids.Add(e.id);
            _entityId = PartySelection.Resolve(ids);
            if (view != null && view.grid != null && view.grid.entities != null)
                foreach (var e in view.grid.entities)
                    if (e.id == _entityId) { me = e; break; }
            if (me == null) { _title.text = Loc.Get("UI_CHARACTER_NOBODY_DEPLOYED"); return; }

            var es = JsonUtility.FromJson<Eff>(session.QueryJson("stats:" + _entityId)) ?? new Eff();

            _title.text = $"{GameText.UnitName(me.unit_def)}   ·   Lv {es.level}";

            // Fingerprinted on what is DISPLAYED, not on the fight: hp moves every tick and none
            // of these do, so rebuilding on hp would churn forty labels ten times a second.
            var fp = new System.Text.StringBuilder()
                .Append(_entityId).Append('|').Append(es.level).Append('|')
                .Append(es.atk).Append(',').Append(es.def).Append(',')
                .Append(es.dodge_bp).Append(',').Append(es.mitigation_bp).Append(',')
                .Append(es.crit_bp).Append(',').Append(es.crit_dmg_bp).Append(',')
                .Append(es.atk_speed_x100).Append(',').Append(es.max_hp).Append('|');
            if (me.equipped != null)
                foreach (var it in me.equipped)
                    fp.Append(it.def_id).Append(',');
            fp.Append('|').Append(me.regalia != null ? me.regalia.id : "")
              .Append(':').Append(me.regalia != null ? me.regalia.level : 0)
              .Append(':').Append(me.regalia != null ? me.regalia.stars : 0);
            if (fp.ToString() == _fingerprint) return;
            _fingerprint = fp.ToString();

            _doll.SetPlayer(me.equipped);
            _regalia.SetRegalia(me.regalia);
            Rebuild(me, es);
        }

        void Rebuild(CharEntity me, Eff es)
        {
            foreach (var go in _built) Destroy(go);
            _built.Clear();
            float y = 0f;

            // NO ATTRIBUTES BLOCK. The gacha pivot deleted the six RO attributes from the sim, so
            // `stats:<id>` no longer answers `eff_str…eff_luk` and `Stats` no longer carries
            // `str_…luk`. Those rows are not "showing zero because the character has none" — the
            // numbers do not exist, and JsonUtility would have drawn six confident 0s with no error
            // anywhere. MDEF, HIT and FLEE went the same way; DODGE replaces the FLEE half.
            //
            // What tells a character apart now lives on the Potential Board, which is not built
            // (docs/skill-system-redesign.md §5.3, checklist items 6 and 11). When it is, this is
            // where its allocation belongs — below Combat, not above it, because the board is the
            // choice and these numbers are its result.
            Head(ref y, "Combat");
            // Final native combat totals; subtracting raw gear contributions would mislabel
            // modifiers as base stats. Physical and magic skills share the one ATK pool.
            Row(ref y, "ATK", es.atk.ToString("N0"));
            Row(ref y, "DEF", es.def.ToString("N0"));
            Row(ref y, "DODGE", (es.dodge_bp / 100f).ToString("0.0") + "%");
            Row(ref y, "CRIT", (es.crit_bp / 100f).ToString("0.0") + "%");
            Row(ref y, "CRIT DMG", (es.crit_dmg_bp / 100f).ToString("0") + "%");
            // ATTACK SPEED reads as attacks per second (sim 0.0.367 sends atk/s ×100).
            Row(ref y, "ATK SPEED", (es.atk_speed_x100 / 100f).ToString("0.00") + "/s");
            Row(ref y, "MITIGATION", (es.mitigation_bp / 100f).ToString("0.0") + "%");
            Row(ref y, "MAX HP", es.max_hp.ToString("N0"));
            UiScroll.Fit(_right, y + UiTheme.Pad);
        }

        void Head(ref float y, string text)
        {
            var t = UiTheme.Caps(_right, "H_" + text, text, UiTheme.CaptionSize,
                                 UiTheme.TextLow, TextAlignmentOptions.Left);
            Place(t.rectTransform, y, HeadH);
            _built.Add(t.gameObject);
            y += HeadH;
        }

        /// One of the six. The base is the number the character owns; the delta is what buffs,
        /// cards, passives and paths are adding right now, and it disappears when they do.
        void Row(ref float y, string label, string value)
        {
            var l = UiTheme.Label(_right, "L_" + label, label, UiTheme.BodySize,
                                  UiTheme.TextMid, TextAlignmentOptions.Left);
            Place(l.rectTransform, y, RowH);
            _built.Add(l.gameObject);
            var v = UiTheme.Label(_right, "V_" + label, value, UiTheme.BodySize,
                                  UiTheme.TextHigh, TextAlignmentOptions.Right);
            Place(v.rectTransform, y, RowH);
            _built.Add(v.gameObject);
            y += RowH;
        }

        void Place(RectTransform rt, float y, float h)
        {
            rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
            rt.pivot = new Vector2(0.5f, 1f);
            rt.sizeDelta = new Vector2(0f, h);
            rt.anchoredPosition = new Vector2(0f, -y);
        }


        [System.Serializable] class CharView { public CharGrid grid; }
        [System.Serializable] class CharGrid { public CharEntity[] entities; }
        [System.Serializable]
        class CharEntity
        {
            public long id; public string team, unit_def;
            public bool is_pet;
            public CharStats stats;
            public ItemView[] equipped;
            /// The worn Regalia, resolved by the sim at its level and stars. Absent = nothing worn;
            /// JsonUtility hands back a default instance rather than null, so the crown checks the
            /// id rather than the reference.
            public RegaliaCrown.View regalia;
        }
        [System.Serializable] class CharStats { public int level; }
        [System.Serializable]
        class Eff
        {
            public long atk, def;
            public int dodge_bp, crit_bp, crit_dmg_bp, mitigation_bp, atk_speed_x100;
            public int level, level_cap;
            public long max_hp, xp;
        }
    }
}
