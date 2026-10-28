// THE CRAFTING BENCHES — Apothecary and Kitchen, ported from godot/views/craft_view.gd, which is
// one screen with a station tab because the two differ only in which recipes they list.
//
// The same screen serves the Blacksmith's Forge tab in Godot (`ForgePanel` shares `RecipeCard`),
// so `station` is a parameter here rather than a copy-paste.
//
// RECIPES COME FROM THE SIM, not from the catalog file the client also happens to hold. The
// catalog knows the rows; only the save knows which are LEARNED, and `server_rolled` is the sim's
// own predicate — a second copy of that is how a recipe ends up craftable in the UI and a silent
// no-op in the sim.
//
// Two buttons, two events: `LearnRecipe` buys the recipe, `CraftItem` spends the materials. Both
// are replay-safe inputs, so nothing here needs a new entry point.
using System.Collections.Generic;
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    public sealed class CraftWindow : MonoBehaviour
    {
        const float CardH = 128f, Gap = UiTheme.Gap;

        Roi.Sim.SimDriver _driver;
        RectTransform _content;
        TextMeshProUGUI _purse;
        readonly List<GameObject> _built = new();
        string _station, _fingerprint = "";

        public static CraftWindow Attach(RectTransform body, string station)
        {
            var w = body.gameObject.AddComponent<CraftWindow>();
            w._station = station;
            w.Build(body);
            return w;
        }

        void Build(RectTransform body)
        {
            float pad = UiTheme.PadLarge;
            _purse = UiTheme.Label(body, "Purse", "", UiTheme.HeadSize,
                                   UiTheme.Accent, TextAlignmentOptions.Right);
            var pr = _purse.rectTransform;
            pr.anchorMin = new Vector2(0f, 1f); pr.anchorMax = new Vector2(1f, 1f);
            pr.pivot = new Vector2(0.5f, 1f);
            pr.offsetMin = new Vector2(pad, -40f); pr.offsetMax = new Vector2(-pad, -UiTheme.PadSmall);

            var host = new GameObject("Scroll", typeof(RectTransform));
            host.transform.SetParent(body, false);
            var hrt = (RectTransform)host.transform;
            hrt.anchorMin = Vector2.zero; hrt.anchorMax = Vector2.one;
            hrt.offsetMin = new Vector2(pad, UiTheme.Pad);
            hrt.offsetMax = new Vector2(-pad, -(40f + UiTheme.Gap));
            _content = UiScroll.Create(hrt, out _);
        }

        public void Refresh()
        {
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var session = _driver != null ? _driver.Session : null;
            if (session == null) { _purse.text = ""; return; }

            var purse = JsonUtility.FromJson<Purse>(session.QueryJson("purse")) ?? new Purse();
            var list = JsonUtility.FromJson<RecipeList>("{\"rows\":" + session.QueryJson("recipes") + "}");
            var all = list != null && list.rows != null ? list.rows : new Recipe[0];
            var bag = JsonUtility.FromJson<ItemList>("{\"items\":" + session.QueryJson("inventory") + "}");
            var items = bag != null && bag.items != null ? bag.items : new ItemView[0];

            _purse.text = $"{purse.gil:N0} Gil   ·   {purse.soul:N0} Soul";

            // Have-counts are summed client-side, the way the original does it: the sim already
            // sent the bag, and asking it to count for us would be a second answer to the same
            // question.
            var have = new Dictionary<string, int>();
            foreach (var it in items)
            {
                have.TryGetValue(it.def_id, out int n);
                have[it.def_id] = n + Mathf.Max(1, it.quantity);
            }

            var mine = new List<Recipe>();
            foreach (var r in all) if (r.station == _station) mine.Add(r);

            var fp = new System.Text.StringBuilder().Append(purse.gil).Append('|').Append(purse.soul).Append('|');
            foreach (var r in mine)
            {
                fp.Append(r.id).Append(r.learned ? '1' : '0');
                if (r.ingredients != null)
                    foreach (var g in r.ingredients)
                    { have.TryGetValue(g.def_id, out int n); fp.Append(':').Append(n); }
                fp.Append(',');
            }
            if (fp.ToString() == _fingerprint) return;
            _fingerprint = fp.ToString();
            Rebuild(mine, have, purse);
        }

        void Rebuild(List<Recipe> mine, Dictionary<string, int> have, Purse purse)
        {
            foreach (var go in _built) Destroy(go);
            _built.Clear();
            float y = 0f;

            if (mine.Count == 0)
            {
                var none=MenuNotice.Create(_content,"Workshop",GameText.ScreenName(_station=="kitchen"?"Kitchen":"Apothecary"),Loc.Get("UI_CRAFT_THIS_BENCH_HAS_NO_RECIPES"));
                _built.Add(none);
                UiScroll.Fit(_content,348f);
                return;
            }

            foreach (var r in mine)
            {
                var card = UiTheme.Box(_content, "R_" + r.id, UiTheme.Surface);
                GameMenuSurface.Add(card.rectTransform);
                Place(card.rectTransform, y, CardH);
                _built.Add(card.gameObject);
                var rt = card.rectTransform;

                var title = UiTheme.Label(rt, "Name",
                    r.output_qty > 1 ? $"{r.output_name}  ×{r.output_qty}" : r.output_name,
                    UiTheme.BodySize, UiTheme.TextHigh, TextAlignmentOptions.TopLeft);
                UiTheme.Display(title);title.fontSize=24;
                Band(title, 24f, -18f, CardH - 54f, 220f);

                // Every ingredient with have/need, and the ones you are short of in red. The
                // decision this screen exists for is "can I make it", so the answer is the row.
                bool enough = purse.gil >= r.gil_cost;
                var parts = new List<string>();
                if (r.ingredients != null)
                    foreach (var g in r.ingredients)
                    {
                        have.TryGetValue(g.def_id, out int n);
                        if (n < g.qty) enough = false;
                        var col = n >= g.qty ? UiTheme.TextMid : new Color(0.90f, 0.45f, 0.40f);
                        parts.Add($"<color=#{ColorUtility.ToHtmlStringRGB(col)}>{g.name} {n}/{g.qty}</color>");
                    }
                if (r.gil_cost > 0) parts.Add($"{r.gil_cost:N0} Gil");
                var mats = UiTheme.Label(rt, "Mats", string.Join("   ", parts), UiTheme.CaptionSize,
                                         UiTheme.TextMid, TextAlignmentOptions.TopLeft);
                mats.fontSize=15;mats.textWrappingMode=TextWrappingModes.Normal;
                Band(mats, 24f, -64f, 16f, 220f);

                string id = r.id;
                if (!r.learned)
                {
                    // Not learned yet: the button buys the recipe, and its price is Soul or Gil.
                    bool can = purse.soul >= r.learn_soul && purse.gil >= r.learn_gil;
                    var cost = r.learn_soul > 0 ? $"{r.learn_soul:N0} Soul" : $"{r.learn_gil:N0} Gil";
                    var learn = HudBar.MakeButton(rt, Loc.Get("UI_CRAFT_LEARN"), "", 168f, UiLayout.Touch,
                        () => Send($"{{\"LearnRecipe\":{{\"recipe_id\":\"{id}\"}}}}"));
                    learn.interactable = can;
                    var lt = learn.GetComponentInChildren<TextMeshProUGUI>();
                    lt.text = Loc.Fmt(can?"UI_UX_LEARN_COST":"UI_UX_NEED_COST",cost);
                    lt.fontSize = UiTheme.CaptionSize;lt.enableAutoSizing=true;lt.fontSizeMin=12;lt.fontSizeMax=UiTheme.CaptionSize;
                    if (!can) lt.color = UiTheme.TextLow;
                    Right(learn.GetComponent<RectTransform>());
                }
                else if (r.server_rolled)
                {
                    // The sim's own predicate: this output is rolled by the SERVER, so crafting it
                    // offline would spend the materials on a candidate nothing will ever resolve.
                    var no = UiTheme.Label(rt, "Blocked", Loc.Get("UI_CRAFT_NEEDS_A_SERVER"), UiTheme.CaptionSize,
                                           UiTheme.TextLow, TextAlignmentOptions.Right);
                    Band(no, 16f, -12f, 12f, 16f);
                }
                else
                {
                    var craft = HudBar.MakeButton(rt, Loc.Get("UI_CRAFT_CRAFT"), "", 168f, UiLayout.Touch,
                        () => Send($"{{\"CraftItem\":{{\"recipe_id\":\"{id}\"}}}}"));
                    craft.interactable = enough;
                    if (!enough) {
                        var label=craft.GetComponentInChildren<TextMeshProUGUI>();
                        label.enableAutoSizing=true;label.fontSizeMin=12;label.fontSizeMax=UiTheme.HeadSize;
                        label.text=Loc.Get(purse.gil<r.gil_cost?"UI_UX_NEED_GIL":"UI_UX_NEED_MATERIALS");label.color=UiTheme.TextLow;
                    }
                    Right(craft.GetComponent<RectTransform>());
                }
                y += CardH + Gap;
            }
            UiScroll.Fit(_content, y + UiTheme.Pad);
        }

        static void Right(RectTransform rt)
        {
            rt.anchorMin = rt.anchorMax = new Vector2(1f, 0.5f);
            rt.pivot = new Vector2(1f, 0.5f);
            rt.anchoredPosition = new Vector2(-16f, 0f);
        }

        static void Band(TextMeshProUGUI t, float left, float top, float bottom, float right)
        {
            var rt = t.rectTransform;
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = new Vector2(left, bottom);
            rt.offsetMax = new Vector2(-right, top);
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
            _driver?.Session?.ApplyInput(eventJson);
            _fingerprint = "";
            Refresh();
        }

        [System.Serializable] class Purse { public long gil, soul; public string zone_id; }
        [System.Serializable] class RecipeList { public Recipe[] rows; }
        [System.Serializable]
        class Recipe
        {
            public string id, name, output_def_id, output_name, station;
            public int output_qty;
            public long gil_cost, learn_soul, learn_gil;
            public bool learned, server_rolled;
            public Ingredient[] ingredients;
        }
        [System.Serializable] class Ingredient { public string def_id, name; public int qty; }
    }
}
