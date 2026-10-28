// THE CHEAT SHEET — the dev screen that makes the other screens testable at all.
//
// Ported in shape from godot/views/debug_view.gd, and the reason it exists here is concrete: a
// fresh party has 0 Gil, so it cannot recruit; no consumable is in any drop table, so Provisions
// can never fill; and a card drops at a weight of 1 in ~50,000. Three finished screens could not
// be exercised with real data at all. A cheat is not a shortcut here, it is the only way to press
// the buttons.
//
// EVERY BUTTON IS A DIRECT STATE EDIT, NOT AN INPUT, and that is the safety argument rather than
// an implementation detail. The sim is replay-verified: a save is (start + the input log), and the
// server checks it by replaying. If granting Gil were an input it would replay cleanly and the
// server would accept a fabricated fortune as legitimate play. Because these edit the state
// directly they are invisible to the log — so a cheated save simply STOPS VERIFYING, which is the
// outcome you want. A cheat should cost you the ability to push, not the server's trust.
//
// So the screen refuses itself while online, and says why. The one exception is granting an item,
// which stays an input because `GrantLoot` already is one: the server authors item grants, so that
// is where it belongs.
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    public sealed class DebugWindow : MonoBehaviour
    {
        const float BtnW = 260f, Gap = UiTheme.Gap;

        Roi.Sim.SimDriver _driver;
        RectTransform _content;
        TextMeshProUGUI _status;

        public static DebugWindow Attach(RectTransform body)
        {
            var w = body.gameObject.AddComponent<DebugWindow>();
            w.Build(body);
            return w;
        }

        void Build(RectTransform body)
        {
            float pad = UiTheme.PadLarge;
            _status = UiTheme.Label(body, "Status", "", UiTheme.CaptionSize,
                                    UiTheme.TextLow, TextAlignmentOptions.Left);
            var sr = _status.rectTransform;
            sr.anchorMin = new Vector2(0f, 1f); sr.anchorMax = new Vector2(1f, 1f);
            sr.pivot = new Vector2(0.5f, 1f);
            sr.offsetMin = new Vector2(pad, -52f); sr.offsetMax = new Vector2(-pad, -UiTheme.PadSmall);
            _status.textWrappingMode = TextWrappingModes.Normal;

            var host = new GameObject("Scroll", typeof(RectTransform));
            host.transform.SetParent(body, false);
            var hrt = (RectTransform)host.transform;
            hrt.anchorMin = Vector2.zero; hrt.anchorMax = Vector2.one;
            hrt.offsetMin = new Vector2(pad, UiTheme.Pad);
            hrt.offsetMax = new Vector2(-pad, -(52f + UiTheme.Gap));
            _content = UiScroll.Create(hrt, out _);

            float y = 0f;
            LocalSavePanel.Build(_content, ref y);
            Head(ref y, "Currency");
            Cheat(ref y, "+10,000 Gil", "{\"give_currency\":{\"gil\":10000,\"soul\":0}}");
            Cheat(ref y, "+50,000 Soul", "{\"give_currency\":{\"gil\":0,\"soul\":50000}}");

            y += UiTheme.Pad;
            Head(ref y, "The lead character");
            Cheat(ref y, "+10 levels", "{\"add_levels\":{\"entity_id\":%LEAD%,\"n\":10}}");
            Cheat(ref y, "Max skills", "{\"max_skills\":{\"entity_id\":%LEAD%}}");
            Cheat(ref y, "Give a gear set", "{\"give_gear\":{\"entity_id\":%LEAD%}}");
            // The ONLY way to see the Apex screen with anything on it: its points come from XP
            // earned past the level cap, so it forces the cap and half-fills the bar as well.
            Cheat(ref y, "+20 Apex Points", "{\"grant_apex\":{\"entity_id\":%LEAD%,\"n\":20}}");

            y += UiTheme.Pad;
            // These three exist BECAUSE the screens that need them cannot otherwise be tested:
            // nothing in the game drops a consumable, and a card is one in fifty thousand.
            Head(ref y, "Items the drop tables cannot produce");
            Cheat(ref y, "10 Red Phials (emergency slot)", "{\"grant_item\":{\"def_id\":\"hp_pot_1\"}}");
            Cheat(ref y, "A Meal (reserved slot)", "{\"grant_item\":{\"def_id\":\"meal_hearty\"}}");
            Cheat(ref y, "A Tonic (reserved slot)", "{\"grant_item\":{\"def_id\":\"aspd_pot_1\"}}");
            Cheat(ref y, "A Pinkling Card", "{\"grant_item\":{\"def_id\":\"card_pinkling\"}}");
            Cheat(ref y, "A FOIL Pinkling Card", "{\"grant_item\":{\"def_id\":\"card_pinkling\",\"foil\":true}}");
            Cheat(ref y, "A Glass Flask", "{\"grant_item\":{\"def_id\":\"glass_flask\"}}");

            UiScroll.Fit(_content, y + UiTheme.Pad);
        }

        public void Refresh()
        {
            var slots = GetComponentInChildren<LocalSavePanel>();
            if (slots != null) slots.Refresh();
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var s = _driver != null ? _driver.Session : null;
            _status.text = s == null
                ? "No session."
                : "These edit the sim's state DIRECTLY and are never recorded in the input log, so a "
                + "save touched by one stops verifying against the server. Refused once online.";
        }

        void Head(ref float y, string text)
        {
            var t = UiTheme.Caps(_content, "H_" + text, text, UiTheme.CaptionSize,
                                 UiTheme.TextLow, TextAlignmentOptions.Left);
            Place(t.rectTransform, y, 26f);
            y += 26f;
        }

        void Cheat(ref float y, string label, string cmd)
        {
            var btn = HudBar.MakeButton(_content, label, "", BtnW, UiLayout.Touch, () => Fire(cmd));
            btn.GetComponentInChildren<TextMeshProUGUI>().alignment = TextAlignmentOptions.Left;
            var rt = btn.GetComponent<RectTransform>();
            rt.anchorMin = rt.anchorMax = new Vector2(0f, 1f);
            rt.pivot = new Vector2(0f, 1f);
            rt.anchoredPosition = new Vector2(0f, -y);
            y += UiLayout.Touch + Gap;
        }

        /// `%LEAD%` is filled in here rather than at build time: the party changes, and a button
        /// that cheated whoever was lead when the window was CREATED is a bug that only shows up
        /// after a bench.
        void Fire(string cmd)
        {
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var s = _driver != null ? _driver.Session : null;
            if (s == null) return;
            if (cmd.Contains("%LEAD%")) cmd = cmd.Replace("%LEAD%", LeadId(s).ToString());
            bool ok = s.Debug(cmd);
            _status.text = ok ? "applied: " + cmd
                              : "REFUSED (online, or unknown): " + cmd;
        }

        static long LeadId(Roi.Sim.SimSession s)
        {
            var v = JsonUtility.FromJson<LeadView>(s.ViewJson());
            if (v != null && v.grid != null && v.grid.entities != null)
                foreach (var e in v.grid.entities)
                    if (e.team == "Player" && !e.is_pet) return e.id;
            return 0;
        }

        void Place(RectTransform rt, float y, float h)
        {
            rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
            rt.pivot = new Vector2(0.5f, 1f);
            rt.sizeDelta = new Vector2(0f, h);
            rt.anchoredPosition = new Vector2(0f, -y);
        }

        [System.Serializable] class LeadView { public LeadGrid grid; }
        [System.Serializable] class LeadGrid { public LeadEntity[] entities; }
        [System.Serializable] class LeadEntity { public long id; public string team; public bool is_pet; }
    }
}
