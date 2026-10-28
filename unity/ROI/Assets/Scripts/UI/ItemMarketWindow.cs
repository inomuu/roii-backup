// ITEM MARKET — the per-instance EQUIPMENT market, ported from `item_market_view.gd`. It shares the
// Trade window with the commodity book, as it does in Godot: one window, two categories, because
// they are one player intent ("trade") split by what is being traded.
//
// THE DIFFERENCE FROM THE COMMODITY BOOK is the whole reason it is a second screen: there is no
// matching. Every listing is a UNIQUE gear instance at a seller-set price, so you browse listings
// and buy the one you want, rather than sending a bid into a book.
//
// Settlement is the same shape: `DepositItem` / `BuyItemListing` are sim events that only become
// real when the server verifies the save carrying them — so listing and buying push immediately —
// and a cancel is a direct op that returns the escrowed item by mail.
//
// NOT PORTED: the substat search. That filter is the centrepiece of this market (find a listing
// with the lines you actually want, not just the right base item), and it deserves its own pass
// rather than a checkbox row bolted on here. Until then this browses everything.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class ItemMarketWindow : MonoBehaviour
    {
        /// Stackables trade on the commodity book; only NON-stackable equipment lists here.
        static readonly HashSet<string> Stackable = new() { "Material", "Junk", "Consumable", "Card" };

        const float TabH = 34f, RowH = 48f, Gap = 4f;

        enum Mode { Buy, Sell, Mine }

        Roi.Sim.SimDriver _driver;
        public RectTransform Root { get; private set; }
        RectTransform _content;
        TextMeshProUGUI _gil, _status;
        readonly List<GameObject> _built = new();
        readonly List<Button> _tabBtns = new();
        Mode _mode = Mode.Buy;
        string _listings = "", _mine = "";
        int _selInv = -1;
        string _selName = "";
        bool _busy;
        string _flash;
        TradeWindow.Stepper _price;

        public static ItemMarketWindow Attach(RectTransform body, System.Action onSwitchCategory)
        {
            var go = new GameObject("ItemMarket", typeof(RectTransform));
            go.transform.SetParent(body, false);
            var w = go.AddComponent<ItemMarketWindow>();
            w.Root = (RectTransform)go.transform;
            w.Root.anchorMin = Vector2.zero; w.Root.anchorMax = Vector2.one;
            w.Root.offsetMin = Vector2.zero; w.Root.offsetMax = Vector2.zero;
            w.Build(onSwitchCategory);
            return w;
        }

        void OnEnable() => Roi.Net.OnlineService.Instance.Changed += OnNet;
        void OnDisable()
        {
            if (Roi.Net.OnlineService.Instance != null)
                Roi.Net.OnlineService.Instance.Changed -= OnNet;
        }
        void OnNet() { if (isActiveAndEnabled && Root.gameObject.activeSelf) Refresh(); }

        void Build(System.Action onSwitchCategory)
        {
            float pad = UiTheme.PadLarge;
            _gil = UiTheme.Label(Root, "Gil", "", UiTheme.HeadSize, UiTheme.Accent,
                                 TextAlignmentOptions.Right);
            var gr = _gil.rectTransform;
            gr.anchorMin = new Vector2(0f, 1f); gr.anchorMax = new Vector2(1f, 1f);
            gr.pivot = new Vector2(0.5f, 1f);
            gr.offsetMin = new Vector2(pad, -40f); gr.offsetMax = new Vector2(-pad, -UiTheme.PadSmall);

            _status = UiTheme.Label(Root, "Status", "", UiTheme.CaptionSize, UiTheme.TextLow,
                                    TextAlignmentOptions.Left);
            _status.textWrappingMode = TextWrappingModes.NoWrap;
            var sr = _status.rectTransform;
            sr.anchorMin = new Vector2(0f, 1f); sr.anchorMax = new Vector2(1f, 1f);
            sr.pivot = new Vector2(0.5f, 1f);
            sr.offsetMin = new Vector2(pad, -40f); sr.offsetMax = new Vector2(-(pad + 260f), -UiTheme.PadSmall);

            var bar = new GameObject("Tabs", typeof(RectTransform));
            bar.transform.SetParent(Root, false);
            var brt = (RectTransform)bar.transform;
            brt.anchorMin = new Vector2(0f, 1f); brt.anchorMax = new Vector2(1f, 1f);
            brt.pivot = new Vector2(0.5f, 1f);
            brt.offsetMin = new Vector2(pad, -(44f + TabH)); brt.offsetMax = new Vector2(-pad, -44f);

            float x = 0f;
            foreach (var m in new[] { Mode.Buy, Mode.Sell, Mode.Mine })
            {
                var mode = m;
                var b = HudBar.MakeGhostButton(brt, TradeWindow.NameOfMode((int)m), 120f, TabH,
                                               () => Select(mode), UiTheme.CaptionSize);
                var rt = (RectTransform)b.transform;
                rt.anchorMin = rt.anchorMax = new Vector2(0f, 0.5f);
                rt.pivot = new Vector2(0f, 0.5f);
                rt.anchoredPosition = new Vector2(x, 0f);
                _tabBtns.Add(b);
                x += 124f;
            }
            // Back to the other half of the same window.
            var swap = HudBar.MakeGhostButton(brt, Loc.Get("UI_COMMON_COMMODITIES"), 140f, TabH, () => onSwitchCategory?.Invoke(),
                                              UiTheme.CaptionSize);
            var srt = (RectTransform)swap.transform;
            srt.anchorMin = srt.anchorMax = new Vector2(1f, 0.5f);
            srt.pivot = new Vector2(1f, 0.5f);
            srt.anchoredPosition = Vector2.zero;

            var host = new GameObject("Scroll", typeof(RectTransform));
            host.transform.SetParent(Root, false);
            var hrt = (RectTransform)host.transform;
            hrt.anchorMin = Vector2.zero; hrt.anchorMax = Vector2.one;
            hrt.offsetMin = new Vector2(pad, UiTheme.Pad);
            hrt.offsetMax = new Vector2(-pad, -(44f + TabH + UiTheme.Gap));
            _content = UiScroll.Create(hrt, out _);
        }

        void Select(Mode m)
        {
            _mode = m;
            _selInv = -1; _selName = "";
            Refresh();
        }

        public void Refresh()
        {
            var net = Roi.Net.OnlineService.Instance;
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            if (!net.LoggedIn)
            {
                _gil.text = "";
                _status.text = "";
                Clear();
                var notice=MenuNotice.Create(_content,"Trade",Loc.Get("UI_NOTICE_CONNECTION"),Loc.Get("UI_NOTICE_TRADE"),()=>net.Connect());
                _built.Add(notice);
                UiScroll.Fit(_content,348f);
                return;
            }
            switch (_mode)
            {
                case Mode.Buy:
                    net.Run(net.Client.ItemMarket((ok, json) =>
                    {
                        _listings = ok ? json : "[]";
                        if (!ok) _status.text = "listings failed: " + json;
                        Rebuild();
                    }));
                    break;
                case Mode.Mine:
                    net.Run(net.Client.ItemMarketMine((ok, json) =>
                    {
                        _mine = ok ? json : "[]";
                        if (!ok) _status.text = "listings failed: " + json;
                        Rebuild();
                    }));
                    break;
                default:
                    Rebuild();
                    break;
            }
        }

        void Rebuild()
        {
            Clear();
            var sim = _driver != null ? _driver.Session : null;
            if (sim == null) return;
            _gil.text = $"{TradeWindow.PurseOf(sim):N0} Gil";
            for (int i = 0; i < _tabBtns.Count; i++)
                _tabBtns[i].GetComponentInChildren<TextMeshProUGUI>().color =
                    (int)_mode == i ? UiTheme.Accent : UiTheme.TextLow;

            float y = 0f;
            switch (_mode)
            {
                case Mode.Buy: BuildBrowse(ref y, sim); break;
                case Mode.Sell: BuildSell(ref y, sim); break;
                default: BuildMine(ref y); break;
            }
            UiScroll.Fit(_content, y + UiTheme.Pad);
        }

        void BuildBrowse(ref float y, Roi.Sim.SimSession sim)
        {
            var rows = MailWindow.RowsOfArray(_listings);
            _status.text = Take() ?? (rows.Count == 0
                ? "Nothing is listed right now."
                : $"{rows.Count} listing{(rows.Count == 1 ? "" : "s")}.");
            long gil = TradeWindow.PurseOf(sim);
            foreach (var raw in rows)
            {
                long id = Field(raw, "id"), price = Field(raw, "price");
                var row = UiTheme.Box(_content, "L" + id, UiTheme.Surface);
                UiTheme.Border(row.rectTransform, UiTheme.Line);
                Place(row.rectTransform, y, RowH);
                _built.Add(row.gameObject);

                var name = UiTheme.Label(row.rectTransform, "N", ItemName(raw), UiTheme.BodySize,
                                         UiTheme.TextHigh, TextAlignmentOptions.Left);
                Inset(name, UiTheme.Pad, 260f);
                var p = UiTheme.Label(row.rectTransform, "P", $"{price:N0} Gil", UiTheme.BodySize,
                                      gil >= price ? UiTheme.Accent : UiTheme.TextLow,
                                      TextAlignmentOptions.Right);
                Inset(p, UiTheme.Pad, 120f);

                long lid = id, lprice = price;
                var buy = HudBar.MakeButton(row.rectTransform, Loc.Get("UI_COMMON_BUY"), "", 96f, 30f, () => Buy(lid, lprice));
                buy.interactable = !_busy && gil >= price;
                var brt = (RectTransform)buy.transform;
                brt.anchorMin = brt.anchorMax = new Vector2(1f, 0.5f);
                brt.pivot = new Vector2(1f, 0.5f);
                brt.anchoredPosition = new Vector2(-UiTheme.Pad, 0f);
                y += RowH + Gap;
            }
        }

        void BuildSell(ref float y, Roi.Sim.SimSession sim)
        {
            // The INDEX is the handle: `DepositItem` takes the inventory slot, so the row's position
            // in the query answer is what the event needs. Filtering must not renumber it, which is
            // why the index is captured from the unfiltered walk.
            var rows = MailWindow.RowsOfArray(sim.QueryJson("inventory"));
            _status.text = Take() ?? "Pick a piece of gear to list.";
            int shown = 0;
            for (int i = 0; i < rows.Count; i++)
            {
                var kind = Str(rows[i], "kind");
                if (Stackable.Contains(kind)) continue;
                shown++;
                int idx = i;
                var label = ItemNameOf(rows[i]);
                var row = UiTheme.Box(_content, "I" + i, idx == _selInv ? UiTheme.SurfaceActive : UiTheme.Surface);
                UiTheme.Border(row.rectTransform, idx == _selInv ? UiTheme.Accent : UiTheme.Line);
                Place(row.rectTransform, y, RowH);
                _built.Add(row.gameObject);

                var btn = row.gameObject.AddComponent<Button>();
                btn.targetGraphic = row;
                btn.navigation = new Navigation { mode = Navigation.Mode.None };
                btn.onClick.AddListener(() => { _selInv = idx; _selName = label; Rebuild(); });

                var name = UiTheme.Label(row.rectTransform, "N", label, UiTheme.BodySize,
                                         UiTheme.TextHigh, TextAlignmentOptions.Left);
                Inset(name, UiTheme.Pad, UiTheme.Pad);
                y += RowH + Gap;
            }
            if (shown == 0)
            {
                var none = UiTheme.Label(_content, "None", Loc.Get("UI_ITEM_MARKET_YOU_ARE_CARRYING_NO_GEAR"),
                                         UiTheme.BodySize, UiTheme.TextLow, TextAlignmentOptions.Left);
                Place(none.rectTransform, y, 24f);
                _built.Add(none.gameObject);
                y += 24f;
                return;
            }
            if (_selInv < 0) return;

            y += UiTheme.Pad;
            var head = UiTheme.Label(_content, "Sel", _selName, UiTheme.HeadSize, UiTheme.TextHigh,
                                     TextAlignmentOptions.Left);
            Place(head.rectTransform, y, 26f);
            _built.Add(head.gameObject);
            y += 26f + Gap;

            _price = TradeWindow.Stepper.Create(_content, "Price", 100, 1, 999_999_999);
            Place(_price.Root, y, TradeWindow.Stepper.H);
            _built.Add(_price.Root.gameObject);
            y += TradeWindow.Stepper.H + UiTheme.Gap;

            var list = HudBar.MakeButton(_content, Loc.Get("UI_COMMON_LIST"), "", 300f, UiLayout.Touch, List);
            list.interactable = !_busy;
            Place(list.GetComponent<RectTransform>(), y, UiLayout.Touch);
            _built.Add(list.gameObject);
            y += UiLayout.Touch;

            var note = UiTheme.Label(_content, "Note",
                Loc.Get("UI_ITEM_MARKET_THE_PIECE_LEAVES_YOUR_BAG"),
                UiTheme.CaptionSize, UiTheme.TextLow, TextAlignmentOptions.TopLeft);
            note.textWrappingMode = TextWrappingModes.Normal;
            Place(note.rectTransform, y + UiTheme.Gap, 36f);
            _built.Add(note.gameObject);
            y += 44f;
        }

        void BuildMine(ref float y)
        {
            var rows = MailWindow.RowsOfArray(_mine);
            _status.text = Take() ?? (rows.Count == 0
                ? "You have nothing listed."
                : "Cancelling returns the piece as mail.");
            foreach (var raw in rows)
            {
                long id = Field(raw, "id"), price = Field(raw, "price");
                var row = UiTheme.Box(_content, "M" + id, UiTheme.Surface);
                UiTheme.Border(row.rectTransform, UiTheme.Line);
                Place(row.rectTransform, y, RowH);
                _built.Add(row.gameObject);

                var name = UiTheme.Label(row.rectTransform, "N", ItemName(raw), UiTheme.BodySize,
                                         UiTheme.TextHigh, TextAlignmentOptions.Left);
                Inset(name, UiTheme.Pad, 260f);
                var p = UiTheme.Label(row.rectTransform, "P", $"{price:N0} Gil", UiTheme.BodySize,
                                      UiTheme.Accent, TextAlignmentOptions.Right);
                Inset(p, UiTheme.Pad, 120f);

                long lid = id;
                var cancel = HudBar.MakeGhostButton(row.rectTransform, Loc.Get("UI_COMMON_CANCEL"), 100f, 30f,
                                                    () => Cancel(lid), UiTheme.CaptionSize);
                var crt = (RectTransform)cancel.transform;
                crt.anchorMin = crt.anchorMax = new Vector2(1f, 0.5f);
                crt.pivot = new Vector2(1f, 0.5f);
                crt.anchoredPosition = new Vector2(-UiTheme.Pad, 0f);
                y += RowH + Gap;
            }
        }

        // --- actions -----------------------------------------------------------------------------

        void Buy(long listingId, long price)
        {
            var sim = _driver != null ? _driver.Session : null;
            if (sim == null || _busy) return;
            _busy = true;
            _status.text = Loc.Get("UI_ITEM_MARKET_BUYING");
            // `max_price` is the price the SCREEN showed. It binds the purchase to what was on
            // offer, so a listing edited between the draw and the press cannot charge more.
            sim.ApplyInput($"{{\"BuyItemListing\":{{\"listing_id\":{listingId},\"max_price\":{price}}}}}");
            Settle("bought — the piece arrives as mail");
        }

        void List()
        {
            var sim = _driver != null ? _driver.Session : null;
            if (sim == null || _busy || _selInv < 0) return;
            _busy = true;
            _status.text = Loc.Get("UI_ITEM_MARKET_LISTING");
            sim.ApplyInput($"{{\"DepositItem\":{{\"inv_idx\":{_selInv},\"price\":{_price.Value}}}}}");
            _selInv = -1;
            Settle("listed");
        }

        void Settle(string done)
        {
            var boot = Roi.Sim.SessionBoot.Instance;
            if (boot == null) { _busy = false; _status.text = Loc.Get("UI_ITEM_MARKET_NO_SESSION_BOOT_TO_PUSH"); return; }
            boot.PushNow((ok, why) =>
            {
                _busy = false;
                _flash = ok ? done : "the server did not accept it: " + why;
                Refresh();
            });
        }

        void Cancel(long id)
        {
            var net = Roi.Net.OnlineService.Instance;
            if (_busy || !net.LoggedIn) return;
            _busy = true;
            _status.text = Loc.Get("UI_ITEM_MARKET_CANCELLING");
            net.Run(net.Client.ItemMarketCancel(id, (ok, body) =>
            {
                _busy = false;
                _flash = ok ? "cancelled — the piece is in your mailbox" : "refused: " + body;
                Refresh();
            }));
        }

        // --- reads --------------------------------------------------------------------------------

        string Take() { var f = _flash; _flash = null; return f; }

        /// A listing's own numeric field, read with the nested ITEM removed first — the item carries
        /// numbers of its own and a regex over the whole row would happily match one of those.
        static long Field(string listingJson, string key)
        {
            var inner = Roi.Net.ServerClient.InnerObject(listingJson, "\"item\":");
            var outer = string.IsNullOrEmpty(inner) ? listingJson : listingJson.Replace(inner, "");
            var m = System.Text.RegularExpressions.Regex.Match(outer, "\"" + key + "\":(\\d+)");
            return m.Success ? long.Parse(m.Groups[1].Value) : 0;
        }

        static string ItemName(string listingJson)
        {
            var inner = Roi.Net.ServerClient.InnerObject(listingJson, "\"item\":");
            return string.IsNullOrEmpty(inner) ? "(item)" : ItemNameOf("{" + inner + "}");
        }

        /// The display name, or the def id when the instance carries none — the sim skips an empty
        /// name in serde, so half the gear in a save has no `name` field at all.
        static string ItemNameOf(string itemJson)
        {
            var def = Str(itemJson, "def_id");
            if (string.IsNullOrEmpty(def)) return "(item)";
            return GameText.ItemName(def, Str(itemJson, "name"));
        }

        static string Str(string json, string key)
        {
            var m = System.Text.RegularExpressions.Regex.Match(json, "\"" + key + "\":\"([^\"]*)\"");
            return m.Success ? m.Groups[1].Value : "";
        }

        static void Inset(TextMeshProUGUI t, float left, float right)
        {
            var rt = t.rectTransform;
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = new Vector2(left, 0f); rt.offsetMax = new Vector2(-right, 0f);
        }

        void Place(RectTransform rt, float y, float h)
        {
            rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
            rt.pivot = new Vector2(0.5f, 1f);
            rt.sizeDelta = new Vector2(0f, h);
            rt.anchoredPosition = new Vector2(0f, -y);
        }

        void Clear()
        {
            foreach (var go in _built) Destroy(go);
            _built.Clear();
        }
    }
}
