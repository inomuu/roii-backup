// THE DPS METER — damage per second by class and by skill, in a corner, toggled with Q. Ported
// from `dps_view.gd`.
//
// A RESETTABLE WINDOW, and that is what separates it from the two neighbours it is easy to confuse
// it with: `Record` is cumulative forever, and a character's stats are something else again. The
// sim keeps the window (`dps` query); this divides by the elapsed seconds and nothing more.
//
// A CORNER HUD, not a window: no dim, no Escape, no centring, and it does not close what is open.
// It is meant to be read WHILE the fight runs, which is the whole point of a damage meter.
using System.Collections.Generic;
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    public sealed class DpsMeter : MonoBehaviour
    {
        const float PanelW = 300f, RowH = 20f, HeadH = 24f;
        /// The sim's clock: ten ticks to the second, everywhere.
        const float TicksPerSec = 10f;

        Roi.Sim.SimDriver _driver;
        RectTransform _panel, _content;
        TextMeshProUGUI _title;
        readonly List<GameObject> _built = new();
        string _fingerprint = "";
        float _poll;

        public static DpsMeter Create(Transform parent)
        {
            var go = new GameObject("DpsMeter", typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var m = go.AddComponent<DpsMeter>();
            m.Build((RectTransform)go.transform);
            go.SetActive(false);
            return m;
        }

        public bool Visible => gameObject.activeSelf;

        public void Toggle()
        {
            gameObject.SetActive(!gameObject.activeSelf);
            if (gameObject.activeSelf) { _fingerprint = ""; Refresh(); }
        }

        void Build(RectTransform root)
        {
            root.anchorMin = Vector2.zero; root.anchorMax = Vector2.one;
            root.offsetMin = Vector2.zero; root.offsetMax = Vector2.zero;

            var box = UiTheme.Box(root, "Panel", new Color(0.02f, 0.02f, 0.03f, 0.86f));
            UiTheme.Border(box.rectTransform, UiTheme.Line);
            _panel = box.rectTransform;
            _panel.anchorMin = _panel.anchorMax = new Vector2(1f, 1f);
            _panel.pivot = new Vector2(1f, 1f);
            _panel.sizeDelta = new Vector2(PanelW, 260f);
            // BELOW the roster strip, right-hand side: the strip owns the top and this must not
            // sit on it.
            _panel.anchoredPosition = new Vector2(-UiTheme.Pad, -(UiLayout.Top + UiTheme.Pad));

            _title = UiTheme.Caps(_panel, "Title", Loc.Get("UI_DPS_METER_DAMAGE"), UiTheme.CaptionSize, UiTheme.TextLow,
                                  TextAlignmentOptions.Left);
            var tr = _title.rectTransform;
            tr.anchorMin = new Vector2(0f, 1f); tr.anchorMax = new Vector2(1f, 1f);
            tr.pivot = new Vector2(0.5f, 1f);
            tr.offsetMin = new Vector2(UiTheme.PadSmall, -HeadH);
            tr.offsetMax = new Vector2(-(UiTheme.PadSmall + 70f), 0f);

            var reset = HudBar.MakeGhostButton(_panel, Loc.Get("UI_DPS_METER_RESET"), 64f, 22f, Reset, UiTheme.CaptionSize);
            var rr = (RectTransform)reset.transform;
            rr.anchorMin = rr.anchorMax = new Vector2(1f, 1f);
            rr.pivot = new Vector2(1f, 1f);
            rr.anchoredPosition = new Vector2(-UiTheme.PadSmall, -2f);

            var content = new GameObject("Rows", typeof(RectTransform));
            content.transform.SetParent(_panel, false);
            _content = (RectTransform)content.transform;
            _content.anchorMin = new Vector2(0f, 1f); _content.anchorMax = new Vector2(1f, 1f);
            _content.pivot = new Vector2(0.5f, 1f);
            _content.offsetMin = new Vector2(UiTheme.PadSmall, 0f);
            _content.offsetMax = new Vector2(-UiTheme.PadSmall, -HeadH);
        }

        void Update()
        {
            // Twice a second. The numbers move while the fight runs, but not at a rate anyone
            // reads faster than that, and this is a HUD element that must not cost frames.
            _poll += Time.unscaledDeltaTime;
            if (_poll < 0.5f) return;
            _poll = 0f;
            Refresh();
        }

        void Refresh()
        {
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var sim = _driver != null ? _driver.Session : null;
            if (sim == null) return;
            var d = JsonUtility.FromJson<Dps>(sim.QueryJson("dps"));
            if (d == null) return;
            float secs = Mathf.Max(1f, d.elapsed_ticks / TicksPerSec);
            _title.text = $"Damage · {(int)secs}s";

            // Rebuilt only when the SET changes; the numbers themselves are written in place. A
            // meter that recreates its rows twice a second is the churn that froze buttons here
            // before (per-tick-control-rebuild-churn).
            var fp = new System.Text.StringBuilder();
            foreach (var r in d.by_class ?? new Row[0]) fp.Append(r.name).Append(',');
            fp.Append('|');
            foreach (var r in d.by_skill ?? new Row[0]) fp.Append(r.name).Append(',');
            if (fp.ToString() != _fingerprint)
            {
                _fingerprint = fp.ToString();
                Rebuild(d);
                return;
            }
            Write(d, secs);
        }

        readonly List<TextMeshProUGUI> _values = new();

        void Rebuild(Dps d)
        {
            foreach (var go in _built) Destroy(go);
            _built.Clear();
            _values.Clear();

            float y = 0f;
            Head(ref y, "By member");
            // The sim answers a CLASS ID here (the skill rows are already display names), so this
            // is the one place the meter has to speak player vocabulary.
            foreach (var r in d.by_class ?? new Row[0]) Row_(ref y, GameText.UnitName(r.name));
            y += UiTheme.Gap;
            Head(ref y, "By skill");
            foreach (var r in d.by_skill ?? new Row[0]) Row_(ref y, r.name);
            _panel.sizeDelta = new Vector2(PanelW, y + HeadH + UiTheme.PadSmall);
            Write(d, 1f);
        }

        void Write(Dps d, float secs)
        {
            int i = 0;
            foreach (var r in d.by_class ?? new Row[0])
                if (i < _values.Count) _values[i++].text = $"{r.total / secs:N0}/s";
            foreach (var r in d.by_skill ?? new Row[0])
                if (i < _values.Count) _values[i++].text = $"{r.total / secs:N0}/s";
        }

        void Reset()
        {
            var sim = _driver != null ? _driver.Session : null;
            // The window belongs to the sim, so resetting it is an EVENT, not a local clear —
            // otherwise the meter and the sim would disagree about when "now" started.
            sim?.ApplyInput("\"ResetDps\"");
            _fingerprint = "";
            Refresh();
        }

        void Head(ref float y, string text)
        {
            var t = UiTheme.Caps(_content, "H_" + text, text, UiTheme.CaptionSize, UiTheme.TextLow,
                                 TextAlignmentOptions.Left);
            Place(t.rectTransform, y, RowH);
            _built.Add(t.gameObject);
            y += RowH;
        }

        void Row_(ref float y, string name)
        {
            var n = UiTheme.Label(_content, "N_" + name, name, UiTheme.CaptionSize, UiTheme.TextMid,
                                  TextAlignmentOptions.Left);
            Place(n.rectTransform, y, RowH);
            _built.Add(n.gameObject);
            var v = UiTheme.Label(_content, "V_" + name, "", UiTheme.CaptionSize, UiTheme.TextHigh,
                                  TextAlignmentOptions.Right);
            Place(v.rectTransform, y, RowH);
            _built.Add(v.gameObject);
            _values.Add(v);
            y += RowH;
        }

        void Place(RectTransform rt, float y, float h)
        {
            rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
            rt.pivot = new Vector2(0.5f, 1f);
            rt.sizeDelta = new Vector2(0f, h);
            rt.anchoredPosition = new Vector2(0f, -y);
        }

        [System.Serializable] class Row { public string name; public long total; }
        [System.Serializable] class Dps { public long elapsed_ticks; public Row[] by_class, by_skill; }
    }
}
