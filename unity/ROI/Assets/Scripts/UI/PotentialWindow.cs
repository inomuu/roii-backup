// The per-character Potential Board. The host owns reachability, tier gates, prices, refunds
// and batch routes; this window displays its on-demand projection and sends replayable inputs.
// The functional graph uses retained uGUI nodes and line Images. Artwork can be layered behind it
// later without changing positions, connections, interaction, or any gameplay rule.
using System;
using System.Collections.Generic;
using System.Text;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class PotentialWindow : MonoBehaviour
    {
        const float HeaderH = 112f, ActionH = 212f;
        Roi.Sim.SimDriver _driver;
        RectTransform _body, _boardHost, _boardCanvas, _detailHost, _details, _actions;
        TextMeshProUGUI _header, _points, _detailText;
        Button _upgrade, _max, _mergeLow, _mergeHigh, _splitLow, _splitHigh;
        readonly List<GameObject> _graphObjects = new();
        Board _board;
        long _member;
        string _selected = "", _fingerprint = "", _notice = "", _classId = "";
        bool _exemplarStyle;
        float _nextRefresh;

        public static PotentialWindow Attach(RectTransform body, bool exemplarStyle = false)
        {
            var w = body.gameObject.AddComponent<PotentialWindow>();
            w._exemplarStyle = exemplarStyle;
            w.Build(body);
            return w;
        }

        void OnEnable() => PartySelection.Changed += OnPicked;
        void OnDisable() { PartySelection.Changed -= OnPicked; }
        void OnPicked()
        {
            _fingerprint = ""; _notice = "";
            if (_body != null && isActiveAndEnabled) Refresh();
        }

        void Build(RectTransform body)
        {
            _body = body;
            _header = UiTheme.Label(body, "Character", "", UiTheme.HeadSize,
                UiTheme.TextHigh, TextAlignmentOptions.TopLeft);
            _points = UiTheme.Label(body, "Points", "", UiTheme.BodySize,
                UiTheme.Accent, TextAlignmentOptions.TopLeft);
            Top(_header.rectTransform, 8f, 32f);
            _header.richText = false;
            _points.characterSpacing = 0.5f;
            Top(_points.rectTransform, 44f, 62f);
            _boardHost = Rect(body, "Board");
            // The Exemplar shell supplies the starfield; only a light veil belongs behind the
            // diagram. Solid node faces preserve glyph contrast wherever a bright star falls.
            Color boardTint = new Color(0.035f, 0.039f, 0.053f, 0.72f);
            if (_exemplarStyle) { boardTint = ExemplarUi.Surface; boardTint.a = .28f; }
            var boardBg = UiTheme.Box(_boardHost, "BoardField", boardTint);
            boardBg.raycastTarget = false;
            _boardCanvas = boardBg.rectTransform;
            Stretch(_boardCanvas, 0f);
            UiTheme.Border(_boardCanvas, _exemplarStyle ? ExemplarUi.Line : UiTheme.Line);
            _detailHost = Rect(body, "Details");
            if (_exemplarStyle)
            {
                var shade = UiTheme.Box(_detailHost, "DetailShade", ExemplarUi.Surface);
                Stretch(shade.rectTransform, 0);
                shade.raycastTarget = false;
                UiTheme.Border(shade.rectTransform, ExemplarUi.Line);
            }
            var detailScrollHost = Rect(_detailHost, "DetailScroll");
            Stretch(detailScrollHost, _exemplarStyle ? 18f : 0f);
            _details = UiScroll.Create(detailScrollHost, out _);
            _detailText = UiTheme.Label(_details, "Description", "", UiTheme.BodySize,
                UiTheme.TextHigh, TextAlignmentOptions.TopLeft);
            _detailText.textWrappingMode = TextWrappingModes.Normal;
            _detailText.overflowMode = TextOverflowModes.Overflow;
            _detailText.characterSpacing = 0.5f;
            _detailText.richText = false;
            _detailText.rectTransform.anchorMin = new Vector2(0f, 1f);
            _detailText.rectTransform.anchorMax = Vector2.one;
            _detailText.rectTransform.pivot = new Vector2(0.5f, 1f);
            _actions = Rect(body, "Actions");
            _upgrade = Action("Upgrade", Loc.Get("UI_POTENTIAL_UPGRADE"), 0f, Upgrade);
            _max = Action("Max", Loc.Get("UI_POTENTIAL_MAX"), 56f, Max);
            // Sigil conversion, both ways at 3:1 (sim `SIGIL_MERGE`): the two grades a stuck
            // purse can trade between. Merge I→II / II→III, Split II→I / III→II.
            _mergeLow = Action("MergeLow", Loc.Fmt("UI_POTENTIAL_MERGE", "I", "II"), 112f, () => Convert(true, 0), 0f, 0.5f);
            _mergeHigh = Action("MergeHigh", Loc.Fmt("UI_POTENTIAL_MERGE", "II", "III"), 112f, () => Convert(true, 1), 0.5f, 1f);
            _splitLow = Action("SplitLow", Loc.Fmt("UI_POTENTIAL_SPLIT", "II", "I"), 168f, () => Convert(false, 1), 0f, 0.5f);
            _splitHigh = Action("SplitHigh", Loc.Fmt("UI_POTENTIAL_SPLIT", "III", "II"), 168f, () => Convert(false, 2), 0.5f, 1f);
            if (_exemplarStyle)
            {
                _header.fontSize = 21; _header.color = ExemplarUi.White; _header.characterSpacing = 0;
                _points.fontSize = 15; _points.color = ExemplarUi.Gold; _points.characterSpacing = 0;
                _detailText.fontSize = 16; _detailText.characterSpacing = 0; _detailText.color = ExemplarUi.White;
                _detailText.lineSpacing = 3;
                foreach (var b in new[] { _upgrade, _max, _mergeLow, _mergeHigh, _splitLow, _splitHigh })
                {
                    var label = b.GetComponentInChildren<TextMeshProUGUI>();
                    label.fontSize = 15; label.characterSpacing = 0;
                    b.GetComponent<Image>().color = UiTheme.Surface;
                    b.GetComponent<CodexButtonImage>().Primary = b == _upgrade;
                    label.color = b == _upgrade ? UiTheme.Accent : UiTheme.TextHigh;
                }
            }
            Layout();
        }

        Button Action(string name, string text, float y, System.Action click, float x0 = 0f, float x1 = 1f)
        {
            var b = HudBar.MakeButton(_actions, text, "", 200f, UiLayout.Touch, click);
            b.name = name;
            var rt = (RectTransform)b.transform;
            rt.anchorMin = new Vector2(x0, 1f); rt.anchorMax = new Vector2(x1, 1f);
            rt.pivot = new Vector2(0.5f, 1f);
            rt.sizeDelta = new Vector2(0f, UiLayout.Touch);
            rt.anchoredPosition = new Vector2(0f, -y);
            b.GetComponentInChildren<TextMeshProUGUI>().fontSize = UiTheme.BodySize;
            if (x1 - x0 < .6f)
            {
                rt.offsetMin += new Vector2(x0 > 0 ? 6f : 0f,0);
                rt.offsetMax -= new Vector2(x0 == 0 ? 6f : 0f,0);
                var label = b.GetComponentInChildren<TextMeshProUGUI>();
                label.text = text.Replace(" → ", "\n→ ");
                label.textWrappingMode = TextWrappingModes.Normal;
                label.fontSizeMin = 12;
                label.fontSizeMax = 14;
            }
            return b;
        }

        void Update()
        {
            if (_body == null || Time.unscaledTime < _nextRefresh) return;
            _nextRefresh = Time.unscaledTime + 0.25f;
            Refresh();
        }

        public void Refresh()
        {
            if (_body == null) return;
            if (_driver == null) _driver = UnityEngine.Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var session = _driver != null ? _driver.Session : null;
            var view = session != null ? JsonUtility.FromJson<PartyView>(session.ViewJson()) : null;
            var members = new List<long>();
            if (view?.grid?.entities != null)
                foreach (var e in view.grid.entities)
                    if (e.team == "Player" && !e.is_pet) members.Add(e.id);
            long member = PartySelection.Resolve(members);
            if (_member != member) { _selected = ""; _notice = ""; }
            _member = member;
            var json = session != null && member != 0 ? session.QueryJson("potential:" + member) : "{}";
            // Unknown queries return [], rather than an object. Do not turn that into a set of
            // confident zero values when the editor still has an older native library loaded.
            _board = json != null && json.TrimStart().StartsWith("{", StringComparison.Ordinal)
                ? JsonUtility.FromJson<Board>(json) : null;
            string fp = member + "|" + json + "|" + _selected + "|" + _notice;
            if (fp == _fingerprint) return;
            _fingerprint = fp;
            Entity picked = null;
            if (view?.grid?.entities != null)
                foreach (var e in view.grid.entities) if (e.id == member) { picked = e; break; }
            _classId = picked != null ? picked.unit_def : "";
            _header.text = picked != null ? GameText.UnitName(picked.unit_def) : Loc.Get("UI_POTENTIAL_EMPTY");
            bool available = _board != null && _board.supported;
            _points.text = available ? PurseText() : "";
            var nodes = available && _board.nodes != null ? _board.nodes : Array.Empty<Node>();
            if (FindNode(_selected) == null) _selected = nodes.Length > 0 ? nodes[0].id : "";
            RefreshGraph(nodes);
            RefreshDetails();
        }

        void RefreshGraph(Node[] nodes)
        {
            foreach (var go in _graphObjects) if (go != null) Destroy(go);
            _graphObjects.Clear();
            if (_boardCanvas == null || nodes.Length == 0) return;

            var byId = new Dictionary<string, Node>(StringComparer.Ordinal);
            foreach (var n in nodes) byId[n.id] = n;

            // Lines are created first so every node and its hit target stays above them.
            var drawn = new HashSet<string>(StringComparer.Ordinal);
            foreach (var n in nodes)
            {
                if (n.links == null) continue;
                foreach (var otherId in n.links)
                {
                    if (!byId.TryGetValue(otherId, out var other)) continue;
                    string key = string.CompareOrdinal(n.id, otherId) < 0
                        ? n.id + "|" + otherId : otherId + "|" + n.id;
                    if (!drawn.Add(key)) continue;
                    Edge(Point(n), Point(other), n.rank > 0 && other.rank > 0);
                }
            }

            foreach (var n in nodes) NodeCell(n, Point(n));
        }

        Vector2 Point(Node n)
        {
            float w = Mathf.Max(320f, _boardHost.rect.width);
            float h = Mathf.Max(300f, _boardHost.rect.height);
            const float margin = 42f;
            return new Vector2(
                Mathf.Lerp(margin, w - margin, n.x / 100f),
                -Mathf.Lerp(margin, h - margin, n.y / 100f));
        }

        void Edge(Vector2 a, Vector2 b, bool unlocked)
        {
            Color line = unlocked
                ? new Color(UiTheme.Accent.r, UiTheme.Accent.g, UiTheme.Accent.b, 0.62f)
                : new Color(1f, 1f, 1f, 0.12f);
            if (_exemplarStyle)
            {
                line = unlocked ? ExemplarUi.Gold : ExemplarUi.Muted;
                line.a = unlocked ? .84f : .40f;
            }
            var img = UiTheme.Flat(_boardCanvas, "Connection", line);
            img.raycastTarget = false;
            var rt = img.rectTransform;
            rt.anchorMin = rt.anchorMax = new Vector2(0f, 1f);
            rt.pivot = new Vector2(0.5f, 0.5f);
            var d = b - a;
            rt.sizeDelta = new Vector2(d.magnitude, _exemplarStyle ? (unlocked ? 2.5f : 1.5f) : (unlocked ? 3f : 2f));
            rt.anchoredPosition = a + d * 0.5f;
            rt.localRotation = Quaternion.Euler(0f, 0f, Mathf.Atan2(d.y, d.x) * Mathf.Rad2Deg);
            _graphObjects.Add(img.gameObject);
        }

        void NodeCell(Node n, Vector2 at)
        {
            // The desktop board is only ~350 logical px tall beside the details panel. These
            // sizes preserve a real gap at the graph's tightest tier spacing (A-s4 and B-s7),
            // rather than letting adjacent hit targets overlap even when their fills look separate.
            float size = n.kind == "major" ? 48f : n.kind == "skill" ? 42f : n.kind == "root" ? 42f : 28f;
            bool selected = n.id == _selected;
            Color fill = n.rank > 0 ? new Color(0.24f, 0.20f, 0.12f, 1f)
                : n.can_upgrade ? UiTheme.Surface
                : new Color(0.055f, 0.059f, 0.074f, 1f);
            if (_exemplarStyle)
                fill = selected ? ExemplarUi.Selected : n.rank > 0 || n.can_upgrade ? ExemplarUi.Card : ExemplarUi.Deep;
            int cornerRadius = _exemplarStyle ? (n.kind == "major" ? 10 : (int)(size/2)) : n.kind == "major" ? 7 : 4;
            var img = UiTheme.Box(_boardCanvas, "Node_" + n.id, fill, cornerRadius);
            var rt = img.rectTransform;
            rt.anchorMin = rt.anchorMax = new Vector2(0f, 1f);
            rt.pivot = new Vector2(0.5f, 0.5f);
            rt.sizeDelta = new Vector2(size, size);
            rt.anchoredPosition = at;
            Color rim = selected ? UiTheme.Accent
                : n.rank > 0 ? new Color(UiTheme.Accent.r, UiTheme.Accent.g, UiTheme.Accent.b, 0.55f)
                : n.can_upgrade ? UiTheme.LineStrong : UiTheme.Line;
            if (_exemplarStyle)
            {
                rim = selected ? ExemplarUi.White : n.rank > 0 ? ExemplarUi.Gold : ExemplarUi.Muted;
                rim.a = selected || n.rank > 0 ? 1f : n.can_upgrade ? .85f : .46f;
            }
            UiTheme.Border(rt, rim, cornerRadius, selected ? 2f : 1f);

            var id = n.id;
            var button = img.gameObject.AddComponent<Button>();
            button.targetGraphic = img;
            button.navigation = new Navigation { mode = Navigation.Mode.None };
            var colors = button.colors;
            colors.highlightedColor = new Color(1.3f, 1.3f, 1.3f, 1f);
            colors.pressedColor = new Color(0.78f, 0.78f, 0.78f, 1f);
            colors.fadeDuration = 0.08f;
            button.colors = colors;
            button.onClick.AddListener(() => Pick(id));

            Color ink = n.rank > 0 ? UiTheme.Accent : n.can_upgrade ? UiTheme.TextHigh : UiTheme.TextLow;
            if (_exemplarStyle)
                ink = selected ? ExemplarUi.White : n.rank > 0 ? ExemplarUi.Gold : n.can_upgrade ? ExemplarUi.White : ExemplarUi.Muted;
            var glyph = UiTheme.Label(rt, "Glyph", Glyph(n), n.kind == "major" ? 15f : n.kind == "skill" ? 11f : 12f,
                ink, TextAlignmentOptions.Center);
            if (_exemplarStyle) glyph.characterSpacing = 0;
            Stretch(glyph.rectTransform, 2f);
            glyph.textWrappingMode = TextWrappingModes.NoWrap;
            glyph.raycastTarget = false;

            _graphObjects.Add(img.gameObject);
        }

        static string Glyph(Node n)
        {
            if (n.kind == "root") return "R";
            if (n.kind == "major") return n.id;
            if (n.kind == "skill") return n.id.ToUpperInvariant() + (n.rank > 0 ? "\n" + n.rank : "");
            return n.rank > 0 ? "●" : "+";
        }

        void Pick(string id)
        {
            _selected = id; _notice = ""; _fingerprint = "";
            Refresh();
        }

        Node FindNode(string id)
        {
            if (_board?.nodes != null)
                foreach (var n in _board.nodes) if (n.id == id) return n;
            return null;
        }

        void RefreshDetails()
        {
            bool available = _board != null && _board.supported;
            var n = available ? FindNode(_selected) : null;
            var text = new StringBuilder();
            if (!available)
                text.Append(_member == 0 ? Loc.Get("UI_POTENTIAL_EMPTY")
                    : !string.IsNullOrEmpty(_board?.reason) ? _board.reason : Loc.Get("UI_POTENTIAL_UNAVAILABLE"));
            else if (n == null) text.Append(Loc.Get("UI_POTENTIAL_SELECT"));
            else
            {
                text.Append(NodeName(n)).Append('\n').Append(Loc.Fmt("UI_POTENTIAL_RANK", n.rank, n.max_rank));
                if (n.required_tier > 0) text.Append('\n').Append(Loc.Fmt("UI_POTENTIAL_REQUIRED_TIER", n.required_tier));
                if (!string.IsNullOrEmpty(n.description)) text.Append("\n\n").Append(n.description.Replace("Potential Board", Loc.Get("UI_POTENTIAL_TITLE")));
                Effect(text, "UI_POTENTIAL_CURRENT", n.current_effect);
                Effect(text, "UI_POTENTIAL_NEXT", n.next_effect);
                if (n.rank < n.max_rank && n.cost != null && n.cost.gil > 0)
                    text.Append("\n\n").Append(Loc.Get("UI_POTENTIAL_COST")).Append(": ")
                        .Append(CostText(n.cost));
                if (!string.IsNullOrEmpty(n.reason)) text.Append('\n').Append(n.reason);
                if (n.links != null && n.links.Length > 0)
                    text.Append("\n\n").Append(Loc.Get("UI_POTENTIAL_CONNECTIONS")).Append(": ")
                        .Append(string.Join(" · ", n.links));
                if (n.can_max || !string.IsNullOrEmpty(n.max_reason))
                {
                    text.Append("\n\n").Append(Loc.Get("UI_POTENTIAL_MAX_COST")).Append('\n')
                        .Append(Loc.Fmt("UI_POTENTIAL_MAX_STEPS", n.max_steps));
                    if (!string.IsNullOrEmpty(n.max_reason)) text.Append('\n').Append(n.max_reason);
                }
            }
            // NO RESET (0.0.351): a rank bought stays bought — said once, in the sim's words.
            if (available && !string.IsNullOrEmpty(_board.reset_reason))
                text.Append("\n\n").Append(_board.reset_reason);
            if (!string.IsNullOrEmpty(_notice)) text.Append("\n\n").Append(_notice);
            _detailText.text = text.ToString();
            _upgrade.interactable = n != null && n.can_upgrade;
            _max.interactable = n != null && n.can_max;
            var p = available ? _board.purse : null;
            _mergeLow.interactable = p != null && At(p.sigil, 0) >= 3;
            _mergeHigh.interactable = p != null && At(p.sigil, 1) >= 3;
            _splitLow.interactable = p != null && At(p.sigil, 1) >= 1;
            _splitHigh.interactable = p != null && At(p.sigil, 2) >= 1;
            FitDetails();
        }

        string NodeName(Node n) => Loc.GetOr("POTENTIAL_NODE_" + _classId + "_" + n.id, n.name ?? n.id);
        static string GradeName(int g) => g == 0 ? "I" : g == 1 ? "II" : "III";
        static long At(long[] a, int i) => a != null && i < a.Length ? a[i] : 0;

        /// The purse header: this unit's own-element Sigil by grade, then the shared families.
        /// The Sigil family follows the unit's ELEMENT, so a Fire unit reads "Fire Sigil".
        string PurseText()
        {
            var p = _board.purse;
            if (p == null) return "";
            string element = GameText.ElementName(p.element);
            return Loc.Fmt("UI_POTENTIAL_PURSE_SIGIL", element, At(p.sigil, 0), At(p.sigil, 1), At(p.sigil, 2)) + "\n"
                + Loc.Fmt("UI_POTENTIAL_PURSE_RARE", p.core, p.track, p.gil) + "\n"
                + Loc.Fmt("UI_POTENTIAL_CAP", _board.cap, _board.soft_cap);
        }

        /// One rank's recipe, families joined with " · " and zero-count families omitted.
        string CostText(Cost c)
        {
            var parts = new List<string>();
            string element = GameText.ElementName(_board?.purse?.element);
            if (c.sigil > 0) parts.Add(Loc.Fmt("UI_POTENTIAL_COST_SIGIL", c.sigil, element, GradeName(c.sigil_grade)));
            if (c.core > 0) parts.Add(Loc.Fmt("UI_POTENTIAL_COST_CORE", c.core));
            if (c.track > 0) parts.Add(Loc.Fmt("UI_POTENTIAL_COST_TRACK", c.track));
            if (c.gil > 0) parts.Add(Loc.Fmt("UI_POTENTIAL_COST_GIL", c.gil));
            return string.Join(" · ", parts);
        }
        static void Effect(StringBuilder text, string key, string value)
        {
            if (!string.IsNullOrEmpty(value)) text.Append("\n\n").Append(Loc.Get(key)).Append(":\n").Append(value);
        }
        void Upgrade() { if (FindNode(_selected)?.can_upgrade == true) Send(JsonUtility.ToJson(new UpgradeEvent { UpgradePotential = new NodeInput { entity_id = _member, node_id = _selected } })); }
        /// One conversion of the purse's own element: `merge` turns 3 of `grade` into one of the
        /// next, else one of `grade` into 3 of the one below.
        void Convert(bool merge, int grade)
        {
            var p = _board?.purse;
            if (p == null || string.IsNullOrEmpty(p.element)) return;
            var input = new SigilInput { element = p.element, grade = grade, count = 1 };
            Send(merge ? JsonUtility.ToJson(new MergeEvent { MergeSigil = input })
                       : JsonUtility.ToJson(new SplitEvent { SplitSigil = input }));
        }
        void Max() { if (FindNode(_selected)?.can_max == true) Send(JsonUtility.ToJson(new MaxEvent { MaxPotential = new TargetInput { entity_id = _member, target_id = _selected } })); }
        void Send(string json)
        {
            string before = JsonUtility.ToJson(_board);
            _notice = "";
            _driver?.ApplyInput(json);
            _fingerprint = "";
            Refresh();
            // ApplyInput's bool means parsed, not accepted. Only the new projection can show
            // the outcome; an invalid/stale input is intentionally a sim-side silent no-op.
            if (before == JsonUtility.ToJson(_board))
            { _notice = Loc.Get("UI_POTENTIAL_UNCHANGED"); _fingerprint = ""; Refresh(); }
        }

        void OnRectTransformDimensionsChange() { if (_body != null) Layout(); }
        void Layout()
        {
            float pad = UiTheme.PadLarge;
            bool wide = _body.rect.width >= 680f;
            float bottom = ActionH + pad + 24f;
            float divide = _exemplarStyle ? .61f : .48f;
            // In the character shell the purse belongs to the left column. Use the free
            // space beside it for the description rather than squeezing text above Upgrade.
            float headerRight = wide && _exemplarStyle ? divide-.02f : 1f;
            _header.rectTransform.anchorMax = new Vector2(headerRight, 1f);
            _points.rectTransform.anchorMax = new Vector2(headerRight, 1f);
            Pane(_actions, wide ? divide : 0f, 1f, pad, pad, -pad, pad + ActionH, false);
            if (wide)
            {
                Pane(_boardHost, 0f, divide-.02f, pad, pad, -pad, -HeaderH, true);
                Pane(_detailHost, divide, 1f, pad, bottom, -pad, _exemplarStyle ? -pad : -HeaderH, true);
            }
            else
            {
                float available = Mathf.Max(100f, _body.rect.height - HeaderH - bottom);
                float detailHeight = available * 0.52f;
                Pane(_detailHost, 0f, 1f, pad, bottom, -pad, bottom + detailHeight, false);
                Pane(_boardHost, 0f, 1f, pad, bottom + detailHeight + 12f, -pad, -HeaderH, true);
            }
            if (_board?.nodes != null) RefreshGraph(_board.nodes);
            FitDetails();
        }
        static void Pane(RectTransform rt, float x0, float x1, float l, float b, float r, float t, bool stretchY)
        {
            rt.anchorMin = new Vector2(x0, 0f); rt.anchorMax = new Vector2(x1, stretchY ? 1f : 0f);
            rt.offsetMin = new Vector2(l, b); rt.offsetMax = new Vector2(r, t);
        }
        void FitDetails()
        {
            if (_detailText == null || _detailHost == null) return;
            float h = _detailText.GetPreferredValues(_detailText.text, Mathf.Max(80f, _details.rect.width - 8f), 0f).y + 18f;
            _detailText.rectTransform.sizeDelta = new Vector2(-8f, h);
            UiScroll.Fit(_details, h);
        }
        static RectTransform Rect(Transform parent, string name)
        {
            var go = new GameObject(name, typeof(RectTransform)); go.transform.SetParent(parent, false);
            return (RectTransform)go.transform;
        }
        static void Stretch(RectTransform rt, float inset)
        {
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = new Vector2(inset, inset); rt.offsetMax = new Vector2(-inset, -inset);
        }
        static void Top(RectTransform rt, float y, float h)
        {
            rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = Vector2.one; rt.pivot = new Vector2(0.5f, 1f);
            rt.offsetMin = new Vector2(UiTheme.PadLarge, -y - h); rt.offsetMax = new Vector2(-UiTheme.PadLarge, -y);
        }

        [Serializable] sealed class PartyView { public Grid grid; }
        [Serializable] sealed class Grid { public Entity[] entities; }
        [Serializable] sealed class Entity { public long id; public string team, unit_def; public bool is_pet; }
        [Serializable] sealed class Board
        {
            public bool supported, can_reset; public string reason, doctrine, element, reset_reason;
            public int cap, soft_cap, level; public Node[] nodes; public Purse purse;
            // Potential Shards in the bag, one count per grade (0.0.351), and the catalog ids.
        }
        [Serializable] sealed class Node
        {
            public string id, name, kind, reason, description, current_effect, next_effect, max_reason;
            public int rank, max_rank, required_tier, max_steps; public Cost cost;
            public bool reachable, can_upgrade, can_max; public string[] links; public float x, y;
        }
        /// The unit's own-element Sigil by grade (`element` names which), then the shared families.
        [Serializable] sealed class Purse { public string element; public long[] sigil; public long core, track, gil; }
        /// One rank's recipe. JsonUtility zero-fills a missing object, so `gil > 0` is the
        /// "there is a next rank" test — every ladder row charges Gil.
        [Serializable] sealed class Cost { public int sigil_grade; public long sigil, core, track, gil; }
        [Serializable] sealed class MemberInput { public long entity_id; }
        [Serializable] sealed class NodeInput { public long entity_id; public string node_id; }
        [Serializable] sealed class TargetInput { public long entity_id; public string target_id; }
        [Serializable] sealed class SigilInput { public string element; public int grade; public long count; }
        [Serializable] sealed class MergeEvent { public SigilInput MergeSigil; }
        [Serializable] sealed class SplitEvent { public SigilInput SplitSigil; }
        [Serializable] sealed class UpgradeEvent { public NodeInput UpgradePotential; }
        [Serializable] sealed class MaxEvent { public TargetInput MaxPotential; }
    }
}
