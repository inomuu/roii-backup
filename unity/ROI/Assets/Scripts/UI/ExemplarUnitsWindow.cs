// EXEMPLAR UNITS (0.0.350) — every Exemplar the account holds IS a unit, benched or on the field.
// Ported from godot/views/exemplar_view.gd. One row per unit: name, stars, level, where it is,
// and the Resonance credits waiting for it (a duplicate pull banks one; nothing spends it but the
// player). Feed opens a confirm and sends `FeedExemplarUnit`; the sim decides everything else —
// the cap, whether a credit exists, what a rank grants. Nothing is equipped here any more: there
// is no carrier to wear.
//
// EVERY NUMBER IS THE SIM'S. `exemplar_units` is one read both clients share, so the two cannot
// list different things.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class ExemplarUnitsWindow : MonoBehaviour
    {
        const float HeadH = 30f, RowH = 46f, BtnW = 92f;

        Roi.Sim.SimDriver _driver;
        RectTransform _content;
        TextMeshProUGUI _hint;
        readonly List<GameObject> _built = new();
        string _fingerprint = null;

        public static ExemplarUnitsWindow Attach(RectTransform body)
        {
            var w = body.gameObject.AddComponent<ExemplarUnitsWindow>();
            w.Build(body);
            return w;
        }

        void Build(RectTransform body)
        {
            float pad = UiTheme.PadLarge;
            _hint = UiTheme.Label(body, "Hint",
                Loc.Get("UI_COLLECTION_RESONANCE_HINT"),
                UiTheme.CaptionSize, UiTheme.TextLow, TextAlignmentOptions.TopLeft);
            var hr = _hint.rectTransform;
            hr.anchorMin = new Vector2(0f, 1f); hr.anchorMax = new Vector2(1f, 1f);
            hr.pivot = new Vector2(0.5f, 1f);
            hr.offsetMin = new Vector2(pad, -HeadH); hr.offsetMax = new Vector2(-pad, -UiTheme.PadSmall);

            var scrollHost = new GameObject("Scroll", typeof(RectTransform));
            scrollHost.transform.SetParent(body, false);
            var host = (RectTransform)scrollHost.transform;
            host.anchorMin = Vector2.zero; host.anchorMax = Vector2.one;
            host.offsetMin = new Vector2(pad, UiTheme.Pad);
            host.offsetMax = new Vector2(-pad, -(HeadH + UiTheme.Gap));
            _content = UiScroll.Create(host, out _);
        }

        /// Rebuilt only when the list actually CHANGES — a button under the pointer must not be
        /// destroyed by the 10 Hz tick.
        public void Refresh()
        {
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var session = _driver != null ? _driver.Session : null;
            if (session == null) { _hint.text = "No session."; return; }

            var list = JsonUtility.FromJson<RowList>("{\"rows\":" + session.QueryJson("exemplar_units") + "}");
            var rows = list != null && list.rows != null ? list.rows : new Row[0];

            var fp = new System.Text.StringBuilder();
            foreach (var r in rows)
                fp.Append(r.unit_id).Append(':').Append(r.stars).Append(':').Append(r.level)
                  .Append(':').Append(r.deployed).Append(':').Append(r.credits).Append(',');
            if (fp.ToString() == _fingerprint) return;
            _fingerprint = fp.ToString();
            Rebuild(rows);
        }

        void Rebuild(Row[] rows)
        {
            foreach (var go in _built) Destroy(go);
            _built.Clear();
            float y = 0f;
            if (rows.Length == 0)
            {
                var notice=MenuNotice.Create(_content,"Exemplar",GameText.ScreenName("Exemplar"),
                    Loc.Get("UI_COLLECTION_NO_UNITS"),
                    () => Object.FindFirstObjectByType<HudBar>()?.Toggle("Summon"),GameText.ScreenName("Summon"));
                _built.Add(notice);
                UiScroll.Fit(_content,300f);
                return;
            }
            foreach (var r in rows)
            {
                var card = UiTheme.Box(_content, "Unit_" + r.unit_id, UiTheme.Surface);
                UiTheme.Border(card.rectTransform, r.credits > 0 ? UiTheme.Accent : UiTheme.Line);
                Place(card.rectTransform, y, RowH);
                _built.Add(card.gameObject);

                string name = string.IsNullOrEmpty(r.name) ? r.exemplar_id : r.name;
                var nameT = UiTheme.Label(card.rectTransform, "Name", name, UiTheme.BodySize,
                                          UiTheme.TextHigh, TextAlignmentOptions.Left);
                var nr = nameT.rectTransform;
                nr.anchorMin = nr.anchorMax = new Vector2(0f, 0.5f);
                nr.pivot = new Vector2(0f, 0.5f);
                nr.sizeDelta = new Vector2(220f, 20f);
                nr.anchoredPosition = new Vector2(UiTheme.Pad, 0f);

                // Stars, level, where it is — the sim's facts, in the order a player scans them.
                // Written "3/5 stars", not with U+2605: the UI font is Noto Sans, whose atlas has no
                // star glyph, and a missing glyph draws as a tofu box (the album learned this first).
                string detail = $"{r.stars}/5 stars   Lv {r.level}" + (r.deployed ? "   on the field" : "   bench")
                              + (r.credits > 0 ? $"   Resonance ×{r.credits}" : "");
                var det = UiTheme.Label(card.rectTransform, "Detail", detail, UiTheme.CaptionSize,
                                        r.credits > 0 ? UiTheme.Accent : UiTheme.TextMid,
                                        TextAlignmentOptions.Left);
                var dr = det.rectTransform;
                dr.anchorMin = dr.anchorMax = new Vector2(0f, 0.5f);
                dr.pivot = new Vector2(0f, 0.5f);
                dr.sizeDelta = new Vector2(420f, 18f);
                dr.anchoredPosition = new Vector2(UiTheme.Pad + 228f, 0f);

                if (r.credits > 0 && r.stars < 5)
                {
                    long id = r.unit_id;
                    int from = r.stars;
                    var b = HudBar.MakeGhostButton(card.rectTransform, "Feed", BtnW, 30f, () =>
                        UiConfirm.Ask("Resonance",
                            $"Spend one Resonance credit on {name}: {from} → {from + 1} stars.\nThis cannot be undone.",
                            "Feed", () => Send($"{{\"FeedExemplarUnit\":{{\"unit_id\":{id}}}}}"),
                            danger: false),
                        UiTheme.CaptionSize);
                    var br = b.GetComponent<RectTransform>();
                    br.anchorMin = br.anchorMax = new Vector2(1f, 0.5f);
                    br.pivot = new Vector2(1f, 0.5f);
                    br.anchoredPosition = new Vector2(-UiTheme.PadSmall, 0f);
                    UiTheme.Border(br, UiTheme.Line);
                }
                y += RowH + UiTheme.Gap;
            }
        }

        void Place(RectTransform rt, float y, float h)
        {
            rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
            rt.pivot = new Vector2(0.5f, 1f);
            rt.sizeDelta = new Vector2(0f, h);
            rt.anchoredPosition = new Vector2(0f, -y);
        }

        void Send(string eventJson)
        {
            var session = _driver != null ? _driver.Session : null;
            if (session == null) return;
            session.ApplyInput(eventJson);
            _fingerprint = "";
            Refresh();
        }

        [System.Serializable] class RowList { public Row[] rows; }
        [System.Serializable]
        class Row
        {
            public long unit_id; public string exemplar_id, name, unit_def;
            public int stars, level, credits; public bool deployed;
        }
    }
}
