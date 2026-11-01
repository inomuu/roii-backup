// THE AREA SELECTOR — top right, always on screen.
//
// The picker itself already existed (`WorldMapWindow`, and `InputEvent::SelectBand` behind it), but
// the only way to reach it was Menu -> Map. A band is the thing an idle player changes most often
// and it sat three clicks deep, which is the shape of a feature that is built and unreachable.
//
// Arrows step ONE band, because that is the common move: finish here, go next. The small map button
// beside them opens the full grid for a jump of several bands at once. Both send the same event, so
// there is one place that decides what a selection means.
//
// The ceiling is the sim's, not this file's: `band_select_max` is computed in `view_state` by the
// exact arithmetic `InputEvent::SelectBand` uses to accept or refuse, so a button that looks
// pressable is one the sim will honour. Deriving it here would let the two drift, and the failure
// would read as "the picker is broken" rather than "that band is locked".
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class AreaSelector : MonoBehaviour
    {
        const float BoxW = 400f, BoxH = 64f, Margin = 12f;
        // The map button carries the SCREEN'S OWN NAME rather than a glyph. `☷` came out as a
        // tofu box: Noto Sans has the arrows but not the trigram, and a missing glyph is silent —
        // it just draws an empty square. `GameText.ScreenName` is the same source the menu's own
        // button uses, so the two always read the same.
        const float ArrowW = 48f, MapW = 80f, Pad = 6f;

        RectTransform _root;
        TextMeshProUGUI _name, _sub;
        Button _prev, _next;
        int _band = 1, _max = 1;
        Roi.Sim.SimDriver _driver;

        public static AreaSelector Create(Transform parent)
        {
            var go = new GameObject("AreaSelector", typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var sel = go.AddComponent<AreaSelector>();
            sel.Build();
            return sel;
        }

        void Build()
        {
            var canvasGo = new GameObject("AreaSelectorCanvas", typeof(RectTransform));
            canvasGo.transform.SetParent(transform, false);
            var canvas = canvasGo.AddComponent<Canvas>();
            canvas.renderMode = RenderMode.ScreenSpaceOverlay;
            canvas.sortingOrder = 40;                 // with the HUD bar, under windows and tooltips
            canvasGo.AddComponent<GraphicRaycaster>();
            var scaler = canvasGo.AddComponent<CanvasScaler>();
            scaler.uiScaleMode = CanvasScaler.ScaleMode.ScaleWithScreenSize;
            scaler.referenceResolution = new Vector2(1920f, 1080f);
            scaler.matchWidthOrHeight = 1f;

            var plate = UiTheme.Box(canvasGo.transform, "Plate", new Color(0.02f, 0.02f, 0.03f, 0.78f));
            _root = plate.rectTransform;
            _root.anchorMin = _root.anchorMax = new Vector2(1f, 1f);
            _root.pivot = new Vector2(1f, 1f);
            _root.sizeDelta = new Vector2(BoxW, BoxH);
            _root.anchoredPosition = new Vector2(-Margin, -100f);

            _prev = HudBar.MakeButton(_root, "‹", "", ArrowW, BoxH - Pad * 2f, () => Step(-1));
            Place(_prev, Pad, 0f);
            _next = HudBar.MakeButton(_root, "›", "", ArrowW, BoxH - Pad * 2f, () => Step(+1));
            Place(_next, BoxW - Pad - MapW - Pad - ArrowW, 0f);
            var map = HudBar.MakeButton(_root, GameText.ScreenName("Map"), "", MapW, BoxH - Pad * 2f, OpenMap);
            Place(map, BoxW - Pad - MapW, 0f);

            float textX = Pad + ArrowW;
            float textW = BoxW - textX - (Pad + MapW + Pad + ArrowW);
            _name = UiTheme.Label(_root, "Name", "", UiTheme.BodySize,
                                  UiTheme.TextHigh, TextAlignmentOptions.Center);
            _name.enableAutoSizing = true;
            _name.fontSizeMin = 9f;
            _name.fontSizeMax = UiTheme.BodySize;
            Span(_name.rectTransform, textX, textW, 8f, 20f);
            _sub = UiTheme.Label(_root, "Sub", "", UiTheme.CaptionSize,
                                 UiTheme.TextLow, TextAlignmentOptions.Center);
            Span(_sub.rectTransform, textX, textW, 28f, 16f);
        }

        static void Place(Button b, float x, float _)
        {
            var rt = b.GetComponent<RectTransform>();
            rt.anchorMin = rt.anchorMax = new Vector2(0f, 0.5f);
            rt.pivot = new Vector2(0f, 0.5f);
            rt.anchoredPosition = new Vector2(x, 0f);
        }

        static void Span(RectTransform rt, float x, float w, float top, float h)
        {
            rt.anchorMin = rt.anchorMax = new Vector2(0f, 1f);
            rt.pivot = new Vector2(0f, 1f);
            rt.sizeDelta = new Vector2(w, h);
            rt.anchoredPosition = new Vector2(x, -top);
        }

        /// Called with the same view JSON the arena renders from.
        public void OnView(string json)
        {
            var v = JsonUtility.FromJson<View>(json);
            if (v == null) return;
            int band = BandOf(v.current_zone_id);
            // `band_select_max` is the frontier the sim will accept; 0 means an older payload that
            // does not carry it, and locking every arrow on a missing key would be worse than
            // letting the sim refuse a step it does not like.
            int max = v.band_select_max > 0 ? v.band_select_max : band + 1;
            if (band == _band && max == _max) return;
            _band = band; _max = max;
            Refresh();
        }

        void Refresh()
        {
            _name.text = WorldMapWindow.RegionNameOfBand(_band);
            _sub.text = Loc.Fmt("UI_AREA_BAND", _band);
            SetOn(_prev, _band > 1);
            SetOn(_next, _band < _max);
        }

        static void SetOn(Button b, bool on)
        {
            b.interactable = on;
            var t = b.GetComponentInChildren<TextMeshProUGUI>();
            if (t != null) t.color = on ? UiTheme.TextHigh : UiTheme.TextLow;
        }

        void Step(int by)
        {
            int want = Mathf.Clamp(_band + by, 1, _max);
            if (want == _band) return;
            _driver ??= Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            _driver?.Session?.ApplyInput($"{{\"SelectBand\":{{\"band\":{want}}}}}");
        }

        void OpenMap()
        {
            var bar = Object.FindFirstObjectByType<HudBar>();
            bar?.Toggle("Map");     // the bar owns every window; opening one behind its back would
        }                           // leave its own button showing the screen as closed

        /// `zone_bN_M` -> N. Anything else is band 1, which is where a party with no zone stands.
        static int BandOf(string zoneId)
        {
            if (string.IsNullOrEmpty(zoneId) || !zoneId.StartsWith("zone_b")) return 1;
            int i = 6, n = 0;
            while (i < zoneId.Length && char.IsDigit(zoneId[i])) { n = n * 10 + (zoneId[i] - '0'); i++; }
            return n > 0 ? n : 1;
        }

        [System.Serializable] class View { public string current_zone_id; public int band_select_max; }
    }
}
