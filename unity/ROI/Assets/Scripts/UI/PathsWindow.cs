// THE PATHS OF SOUL — the party-wide passive web. Ported from godot/views/paths_view.gd and its
// leaf widget soul_web.gd.
//
// The Godot original paints the graph with `_draw`, which its own header calls out as an allowed
// exception to the project's ban on immediate-mode SCREENS: a node graph has no retained-control
// equivalent. uGUI has none either, so the nodes are real Buttons (they have to be tappable, and
// tappable-not-draggable is the rule here) and only the EDGES are drawn — as thin rotated rects,
// because a line between two points is a rect with an angle and that needs no mesh, no shader and
// no library.
//
// NO DRAG-PAN. Godot pans by dragging the canvas; this scrolls instead, because a drag gesture is
// exactly what the no-drag rule is about — on a touch screen it fights the scroll it looks like.
//
// `allocatable` arrives already ANDed with "and you can afford it". Two different refusals — the
// web does not reach the node yet, or the Soul is not there — collapse into the one answer the
// screen acts on, and the sim is what decides both.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class PathsWindow : MonoBehaviour
    {
        const float Scale = 108f, NodeR = 40f, RootR = 44f;

        Roi.Sim.SimDriver _driver;
        RectTransform _content;
        ScrollRect _scroll;
        bool _centred;
        TextMeshProUGUI _soul;
        readonly List<GameObject> _built = new();
        string _fingerprint = "";

        public static PathsWindow Attach(RectTransform body)
        {
            var w = body.gameObject.AddComponent<PathsWindow>();
            w.Build(body);
            return w;
        }

        void Build(RectTransform body)
        {
            float pad = UiTheme.PadLarge;
            _soul = UiTheme.Label(body, "Soul", "", UiTheme.HeadSize,
                                  UiTheme.Accent, TextAlignmentOptions.Right);
            var sr = _soul.rectTransform;
            sr.anchorMin = new Vector2(0f, 1f); sr.anchorMax = new Vector2(1f, 1f);
            sr.pivot = new Vector2(0.5f, 1f);
            sr.offsetMin = new Vector2(pad, -40f); sr.offsetMax = new Vector2(-pad, -UiTheme.PadSmall);

            var hint = UiTheme.Label(body, "Hint",
                Loc.Get("UI_PATHS_PATHS_ARE_PARTY_WIDE_AND"),
                UiTheme.CaptionSize, UiTheme.TextLow, TextAlignmentOptions.Left);
            var hr = hint.rectTransform;
            hr.anchorMin = new Vector2(0f, 1f); hr.anchorMax = new Vector2(1f, 1f);
            hr.pivot = new Vector2(0.5f, 1f);
            hr.offsetMin = new Vector2(pad, -40f); hr.offsetMax = new Vector2(-pad, -UiTheme.PadSmall);
            hr.offsetMax=new Vector2(-180,-UiTheme.PadSmall);

            var host = new GameObject("Scroll", typeof(RectTransform));
            host.transform.SetParent(body, false);
            var hrt = (RectTransform)host.transform;
            hrt.anchorMin = Vector2.zero; hrt.anchorMax = Vector2.one;
            hrt.offsetMin = new Vector2(pad, UiTheme.Pad);
            hrt.offsetMax = new Vector2(-pad, -(40f + UiTheme.Gap));
            _content = UiScroll.Create(hrt, out var scroll);
            _content.anchorMin=_content.anchorMax=new Vector2(0,1);_content.pivot=new Vector2(0,1);
            _content.anchoredPosition=Vector2.zero;
            scroll.horizontal = true;   // the web is wider than it is tall
            _scroll = scroll;
            // Both axes remain reachable with a mouse, wheel or scrollbar on small screens.
            scroll.viewport.offsetMin = new Vector2(0, 14);
            var track = UiTheme.Box(hrt, "HorizontalTrack", UiTheme.Surface, 4);
            var tr = track.rectTransform;
            tr.anchorMin = Vector2.zero; tr.anchorMax = new Vector2(1, 0);
            tr.offsetMin = Vector2.zero; tr.offsetMax = new Vector2(-14, 10);
            var handle = UiTheme.Box(tr, "Handle", UiTheme.TextLow, 4);
            handle.rectTransform.anchorMin = Vector2.zero; handle.rectTransform.anchorMax = Vector2.one;
            handle.rectTransform.offsetMin = new Vector2(0, 2); handle.rectTransform.offsetMax = new Vector2(0, -2);
            var bar = track.gameObject.AddComponent<Scrollbar>();
            bar.direction = Scrollbar.Direction.LeftToRight;
            bar.handleRect = handle.rectTransform; bar.targetGraphic = handle;
            bar.navigation = new Navigation { mode = Navigation.Mode.None };
            scroll.horizontalScrollbar = bar;
            scroll.horizontalScrollbarVisibility = ScrollRect.ScrollbarVisibility.AutoHide;
        }

        public void Refresh()
        {
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var session = _driver != null ? _driver.Session : null;
            if (session == null) return;

            var purse = JsonUtility.FromJson<Purse>(session.QueryJson("purse")) ?? new Purse();
            var list = JsonUtility.FromJson<NodeList>("{\"rows\":" + session.QueryJson("soul_paths") + "}");
            var nodes = list != null && list.rows != null ? list.rows : new Node[0];
            _soul.text = $"{purse.soul:N0} Soul";

            var fp = new System.Text.StringBuilder().Append(purse.soul).Append('|');
            foreach (var n in nodes) fp.Append(n.owned ? '1' : n.allocatable ? '2' : '0');
            if (fp.ToString() == _fingerprint) return;
            _fingerprint = fp.ToString();
            Rebuild(nodes);
        }

        void Rebuild(Node[] nodes)
        {
            foreach (var go in _built) Destroy(go);
            _built.Clear();
            if (nodes.Length == 0) { UiScroll.Fit(_content, 0f); return; }

            // The authored coordinates are centred on nothing in particular, so the web is
            // measured and then shifted into positive space — a node at x = -4 would otherwise sit
            // outside the scroll content and be unreachable.
            float minX = float.MaxValue, maxX = float.MinValue, minY = float.MaxValue, maxY = float.MinValue;
            foreach (var n in nodes)
            {
                minX = Mathf.Min(minX, n.x); maxX = Mathf.Max(maxX, n.x);
                minY = Mathf.Min(minY, n.y); maxY = Mathf.Max(maxY, n.y);
            }
            float m = RootR + UiTheme.Pad;
            Vector2 At(Node n) => new((n.x - minX) * Scale + m, -((n.y - minY) * Scale + m));
            float w = (maxX - minX) * Scale + m * 2f, h = (maxY - minY) * Scale + m * 2f;
            _content.sizeDelta = new Vector2(w, h);

            var byId = new Dictionary<string, Node>();
            foreach (var n in nodes) byId[n.id] = n;

            // EDGES FIRST so the nodes sit on top of them, and each pair only once — an edge drawn
            // from both ends is two overlapping rects at double the opacity.
            var drawn = new HashSet<string>();
            foreach (var n in nodes)
            {
                if (n.neighbors == null) continue;
                foreach (var other in n.neighbors)
                {
                    if (!byId.TryGetValue(other, out var o)) continue;
                    var key = string.CompareOrdinal(n.id, other) < 0 ? n.id + "|" + other : other + "|" + n.id;
                    if (!drawn.Add(key)) continue;
                    Edge(At(n), At(o), n.owned && o.owned);
                }
            }

            foreach (var n in nodes) NodeCell(n, At(n));
            if (!_centred)
            {
                Node origin = nodes[0];
                foreach (var n in nodes) if (n.id == "root" || n.id == "origin") { origin = n; break; }
                Canvas.ForceUpdateCanvases();
                var p = At(origin); var view = _scroll.viewport.rect.size;
                _content.anchoredPosition = new Vector2(
                    -Mathf.Clamp(p.x - view.x * .5f, 0, Mathf.Max(0, w - view.x)),
                    Mathf.Clamp(-p.y - view.y * .5f, 0, Mathf.Max(0, h - view.y)));
                _centred = true;
            }
        }

        /// A line as a thin ROTATED rect. Two owned ends read as a lit path; anything else is the
        /// faint scaffolding that shows where the web could go.
        void Edge(Vector2 a, Vector2 b, bool lit)
        {
            var img = UiTheme.Flat(_content, "Edge", lit ? UiTheme.Accent * 0.75f : UiTheme.Line);
            img.raycastTarget = false;
            var rt = img.rectTransform;
            rt.anchorMin = rt.anchorMax = new Vector2(0f, 1f);
            rt.pivot = new Vector2(0.5f, 0.5f);
            var d = b - a;
            rt.sizeDelta = new Vector2(d.magnitude, lit ? 3f : 1f);
            rt.anchoredPosition = a + d * 0.5f;
            rt.localRotation = Quaternion.Euler(0f, 0f, Mathf.Atan2(d.y, d.x) * Mathf.Rad2Deg);
            _built.Add(img.gameObject);
        }

        void NodeCell(Node n, Vector2 at)
        {
            bool root = n.id == "root" || (n.neighbors != null && n.neighbors.Length >= 4 && n.cost == 0);
            float r = root ? RootR : NodeR;
            var fill = n.owned ? UiTheme.SurfaceActive
                     : n.allocatable ? UiTheme.Surface
                     : UiTheme.Panel;
            var box = UiTheme.Box(_content, "N_" + n.id, fill);
            UiTheme.Border(box.rectTransform,
                           n.owned ? UiTheme.Accent : n.allocatable ? UiTheme.LineStrong : UiTheme.Line);
            var rt = box.rectTransform;
            rt.anchorMin = rt.anchorMax = new Vector2(0f, 1f);
            rt.pivot = new Vector2(0.5f, 0.5f);
            rt.sizeDelta = new Vector2(r * 2f, r * 2f);
            rt.anchoredPosition = at;
            _built.Add(box.gameObject);

            if (n.allocatable && !n.owned)
            {
                var id = n.id;
                var btn = box.gameObject.AddComponent<UiStateButton>();
                btn.targetGraphic = box;
                btn.navigation = new Navigation { mode = Navigation.Mode.None };
                btn.colors = UiTheme.ButtonColors;UiButtonFeedback.Add(btn);
                btn.onClick.AddListener(() => Send(id));
            }

            var label = UiTheme.Label(rt, "L", n.label, 12f,
                                      n.owned ? UiTheme.TextHigh : UiTheme.TextMid,
                                      TextAlignmentOptions.Center);
            label.textWrappingMode = TextWrappingModes.Normal;
            label.characterSpacing = 0f;
            var lr = label.rectTransform;
            lr.anchorMin = Vector2.zero; lr.anchorMax = Vector2.one;
            lr.offsetMin = new Vector2(2f, 2f); lr.offsetMax = new Vector2(-2f, -2f);

            // The effect hangs BELOW the node rather than inside it: at 26 px a node holds a name
            // or a number, not both.
            if (!string.IsNullOrEmpty(n.effect))
            {
                var eff = UiTheme.Label(_content, "E_" + n.id, $"+{n.magnitude} {n.effect}", 11f,
                                        n.owned ? UiTheme.Accent : UiTheme.TextLow,
                                        TextAlignmentOptions.Center);
                var er = eff.rectTransform;
                er.anchorMin = er.anchorMax = new Vector2(0f, 1f);
                er.pivot = new Vector2(0.5f, 1f);
                er.sizeDelta = new Vector2(Scale, 22f);
                er.anchoredPosition = at + new Vector2(0f, -(r + 2f));
                _built.Add(eff.gameObject);
            }
        }

        void Send(string id)
        {
            _driver?.Session?.ApplyInput($"{{\"LevelSoulPath\":{{\"skill_id\":\"{id}\"}}}}");
            _fingerprint = "";
            Refresh();
        }

        [System.Serializable] class Purse { public long gil, soul; public string zone_id; }
        [System.Serializable] class NodeList { public Node[] rows; }
        [System.Serializable]
        class Node
        {
            public string id, label, effect;
            public int magnitude, x, y;
            public long cost;
            public string[] neighbors;
            public bool owned, allocatable;
        }
    }
}
