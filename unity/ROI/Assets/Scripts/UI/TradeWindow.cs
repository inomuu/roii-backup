// TRADE — the commodity market: the player Gil order book for stackables, ported from
// `market_view.gd`. ONLINE only, because the book IS the server's.
//
// THREE TABS, which is the user's own split there: BUY shows the market (pick a commodity, see its
// book, buy), SELL shows what you HOLD (pick a stack you own, see the book you would be listing
// into, list), and MINE is the only place an order can be cancelled.
//
// HOW A TRADE ACTUALLY SETTLES, and why this screen pushes a save in the middle of a button press:
//
//   LISTING is `DepositCommodity` — the units leave the inventory NOW, optimistically, and the
//   server creates the order when it verifies the save. BUYING is `BuyCommodity` — it holds
//   qty x max_price Gil now, and the server matches cheapest-first and delivers units, proceeds
//   and any refund BY MAIL on the verified save.
//
//   So the order only exists after a push. Refetching the book without pushing first shows a book
//   that does not contain what you just did, which reads as the button having failed.
//
// CANCELLING is the exception: a direct server op, because the units come back as Return mail
// rather than out of the save.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class TradeWindow : MonoBehaviour
    {
        /// What can be traded on the COMMODITY book: fungible stacks. Equipment is per-instance and
        /// belongs to the item market, which is a different screen and a different endpoint.
        static readonly HashSet<string> Stackable = new() { "Material", "Junk", "Consumable", "Card" };

        const float TabH = 34f, ChipH = 32f, RowH = 40f, Gap = 12f;

        enum Mode { Buy, Sell, Mine }

        Roi.Sim.SimDriver _driver;
        /// Everything this half of the window owns. The item market is the OTHER half and lives in
        /// the same body — one window, two categories, as in Godot — so each half has one object to
        /// switch off.
        RectTransform _root;
        ItemMarketWindow _gear;
        RectTransform _content, _list;
        long _selectedOrder;
        TextMeshProUGUI _gil, _status;
        readonly List<GameObject> _built = new();
        readonly List<Button> _tabBtns = new();
        Mode _mode = Mode.Buy;

        Row[] _catalog = new Row[0];      // buyable stackables, from the sim
        string _selDef = "", _selName = "";
        string _bookJson = "", _mineJson = "";
        bool _busy;
        Stepper _qty, _price;
        /// What the last action did. Every action refreshes the screen, and the redraw would
        /// otherwise overwrite the one line the player pressed the button to read.
        string _flash;

        public static TradeWindow Attach(RectTransform body)
        {
            var w = body.gameObject.AddComponent<TradeWindow>();
            w.Build(body);
            return w;
        }

        void OnEnable() => Roi.Net.OnlineService.Instance.Changed += OnNet;
        void OnDisable()
        {
            if (Roi.Net.OnlineService.Instance != null)
                Roi.Net.OnlineService.Instance.Changed -= OnNet;
        }
        void OnNet() { if (isActiveAndEnabled) Refresh(); }

        void Build(RectTransform body)
        {
            float pad = UiTheme.PadLarge;
            var rootGo = new GameObject("Commodities", typeof(RectTransform));
            rootGo.transform.SetParent(body, false);
            _root = (RectTransform)rootGo.transform;
            _root.anchorMin = Vector2.zero; _root.anchorMax = Vector2.one;
            _root.offsetMin = Vector2.zero; _root.offsetMax = Vector2.zero;

            _gil = UiTheme.Label(_root, "Gil", "", UiTheme.HeadSize, UiTheme.Accent,
                                 TextAlignmentOptions.Right);
            var gr = _gil.rectTransform;
            gr.anchorMin = new Vector2(0f, 1f); gr.anchorMax = new Vector2(1f, 1f);
            gr.pivot = new Vector2(0.5f, 1f);
            gr.offsetMin = new Vector2(pad, -40f); gr.offsetMax = new Vector2(-pad, -UiTheme.PadSmall);

            _status = UiTheme.Label(_root, "Status", "", UiTheme.CaptionSize, UiTheme.TextLow,
                                    TextAlignmentOptions.Left);
            _status.textWrappingMode = TextWrappingModes.NoWrap;
            var sr = _status.rectTransform;
            sr.anchorMin = new Vector2(0f, 1f); sr.anchorMax = new Vector2(1f, 1f);
            sr.pivot = new Vector2(0.5f, 1f);
            sr.offsetMin = new Vector2(pad, -40f); sr.offsetMax = new Vector2(-(pad + 260f), -UiTheme.PadSmall);

            var bar = new GameObject("Tabs", typeof(RectTransform));
            bar.transform.SetParent(_root, false);
            var brt = (RectTransform)bar.transform;
            brt.anchorMin = new Vector2(0f, 1f); brt.anchorMax = new Vector2(1f, 1f);
            brt.pivot = new Vector2(0.5f, 1f);
            brt.offsetMin = new Vector2(pad, -(44f + TabH)); brt.offsetMax = new Vector2(-pad, -44f);

            float x = 0f;
            foreach (var m in new[] { Mode.Buy, Mode.Sell, Mode.Mine })
            {
                var mode = m;
                var b = HudBar.MakeGhostButton(brt, Name(m), 120f, TabH, () => Select(mode),
                                               UiTheme.CaptionSize);
                var rt = (RectTransform)b.transform;
                rt.anchorMin = rt.anchorMax = new Vector2(0f, 0.5f);
                rt.pivot = new Vector2(0f, 0.5f);
                rt.anchoredPosition = new Vector2(x, 0f);
                _tabBtns.Add(b);
                x += 124f;
            }
            // The other category. Equipment is per-instance and has no order book, so it is a
            // different screen — but the same window, because "trade" is one intent.
            var swap = HudBar.MakeGhostButton(brt, Loc.Get("UI_COMMON_GEAR"), 140f, TabH, () => ShowGear(body),
                                              UiTheme.CaptionSize);
            var swrt = (RectTransform)swap.transform;
            swrt.anchorMin = swrt.anchorMax = new Vector2(1f, 0.5f);
            swrt.pivot = new Vector2(1f, 0.5f);
            swrt.anchoredPosition = Vector2.zero;

            _list=UiScroll.Create(Pane(_root,"Items",0,.53f),out _,false);
            _content=UiScroll.Create(Pane(_root,"Details",.53f,1f),out _,false);
        }

        RectTransform Pane(RectTransform parent,string name,float left,float right)
        {
            var box=UiTheme.Box(parent,name,new Color(.015f,.045f,.045f,.82f));
            var rt=box.rectTransform;rt.anchorMin=new Vector2(left,0);rt.anchorMax=new Vector2(right,1);
            rt.offsetMin=new Vector2(left==0?24:8,24);rt.offsetMax=new Vector2(right==1?-24:-8,-94);
            UiTheme.Border(rt,UiTheme.Line);
            var inner=new GameObject("PaddedContent",typeof(RectTransform));
            inner.transform.SetParent(rt,false);
            var inset=(RectTransform)inner.transform;
            inset.anchorMin=Vector2.zero;inset.anchorMax=Vector2.one;
            inset.offsetMin=new Vector2(20,20);inset.offsetMax=new Vector2(-20,-20);
            return inset;
        }

        void ShowGear(RectTransform body)
        {
            if (_gear == null) _gear = ItemMarketWindow.Attach(body, ShowCommodities);
            _root.gameObject.SetActive(false);
            _gear.Root.gameObject.SetActive(true);
            _gear.Refresh();
        }

        void ShowCommodities()
        {
            if (_gear != null) _gear.Root.gameObject.SetActive(false);
            _root.gameObject.SetActive(true);
            Refresh();
        }

        /// The mode names, shared with the item market so both halves label their tabs identically.
        internal static string NameOfMode(int m) => Name((Mode)m);

        static string Name(Mode m) => m switch
        {
            Mode.Buy => "Buy", Mode.Sell => "Sell", _ => "My listings",
        };

        void Select(Mode m)
        {
            _mode = m; _selectedOrder=0;
            _selDef = ""; _selName = ""; _bookJson = "";
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
                SetPanes(false);
                var notice=MenuNotice.Create(_root,"Trade",Loc.Get("UI_NOTICE_CONNECTION"),Loc.Get("UI_NOTICE_TRADE"),()=>net.Connect());
                _built.Add(notice);
                MenuNotice.Center(notice);
                return;
            }

            if (_mode == Mode.Mine)
            {
                net.Run(net.Client.MyListings((ok, json) =>
                {
                    _mineJson = ok ? json : "[]";
                    if (!ok) _status.text = "listings failed: " + json;
                    Rebuild();
                }));
                return;
            }
            if (!string.IsNullOrEmpty(_selDef))
            {
                net.Run(net.Client.CommodityBook(_selDef, (ok, json) =>
                {
                    _bookJson = ok ? json : "[]";
                    if (!ok) _status.text = "book failed: " + json;
                    Rebuild();
                }));
                return;
            }
            Rebuild();
        }

        void SetPanes(bool visible)
        {
            _root.Find("Items").gameObject.SetActive(visible);
            _root.Find("Details").gameObject.SetActive(visible);
        }
        void EmptyMarket(string message)
        {
            SetPanes(false); _status.text="";
            var notice=MenuNotice.Create(_root,"Trade",GameText.ScreenName("Trade"),message);
            MenuNotice.Center(notice); _built.Add(notice);
        }
        void Rebuild()
        {
            Clear();
            SetPanes(true);
            var sim = _driver != null ? _driver.Session : null;
            if (sim == null) return;
            _gil.text = $"{Purse(sim):N0} Gil";
            for (int i = 0; i < _tabBtns.Count; i++)
                _tabBtns[i].GetComponentInChildren<TextMeshProUGUI>().color =
                    (int)_mode == i ? UiTheme.Accent : UiTheme.TextLow;

            float y = 0f;
            if (_mode == Mode.Mine) {
                BuildMine(ref y); UiScroll.Fit(_content,y+UiTheme.Pad);

                return;
            }

            // The pickable set differs by side and that IS the difference between the two tabs:
            // BUY offers every tradeable commodity in the catalog, SELL offers only what you hold.
            var chips = _mode == Mode.Buy ? CatalogChips(sim) : HeldChips(sim);
            if (chips.Count == 0)
            {
                                EmptyMarket(Loc.Get(_mode==Mode.Buy?"UI_MKT_EMPTY_CATALOG":"UI_MKT_EMPTY_HELD"));
return;
            }
            BuildChips(chips, ref y);
            y=0;

            if (string.IsNullOrEmpty(_selDef))
            {
                _status.text = Take() ?? (_mode == Mode.Buy
                    ? "Pick a commodity to see its orders."
                    : "Pick a stack of yours to list.");
                Hint(_content, "Select an item", _mode==Mode.Buy?"Choose an item on the left to view orders and buy.":"Choose a stack on the left to set quantity and price.");
                UiScroll.Fit(_content, 180);
                return;
            }

            ItemHeader(_selDef,_selName,$"Owned · {Held(sim,_selDef):N0}",ref y);

            _status.text = Take() ?? (_mode==Mode.Buy?"Buy · ":"Sell · ")+_selName;
            BuildBook(ref y);

            y += UiTheme.Pad;
            BuildAction(ref y, sim);
            UiScroll.Fit(_content, y + UiTheme.Pad);
        }

        // --- the pickers -----------------------------------------------------------------------

        struct Chip { public string Def, Name; public long Held; }

        List<Chip> CatalogChips(Roi.Sim.SimSession sim)
        {
            if (_catalog.Length == 0)
            {
                var list = JsonUtility.FromJson<RowList>("{\"rows\":" + sim.QueryJson("shop") + "}");
                _catalog = list?.rows ?? new Row[0];
            }
            var outp = new List<Chip>();
            foreach (var r in _catalog)
                if (Stackable.Contains(r.kind))
                    outp.Add(new Chip { Def = r.def_id, Name = Display(r), Held = Held(sim, r.def_id) });
            return outp;
        }

        /// Your own stacks, AGGREGATED by def: the bag holds several partial stacks of one material
        /// and the book does not care which one you sell from. Potential materials are not in the
        /// bag at all — they are a ledger, read through `materials` — and sell the same way.
        List<Chip> HeldChips(Roi.Sim.SimSession sim)
        {
            var byDef = new Dictionary<string, Chip>();
            foreach (var r in HeldRows(sim))
            {
                if (!Stackable.Contains(r.kind) || string.IsNullOrEmpty(r.def_id)) continue;
                byDef.TryGetValue(r.def_id, out var c);
                c.Def = r.def_id; c.Name = Display(r);
                c.Held += System.Math.Max(1, r.quantity);
                byDef[r.def_id] = c;
            }
            return new List<Chip>(byDef.Values);
        }

        void BuildChips(List<Chip> chips, ref float y)
        {
            foreach (var c in chips)
            {
                var chip=c;
                var b=HudBar.MakeGhostButton(_list,"",300,70,()=>{_selDef=chip.Def;_selName=chip.Name;Refresh();},UiTheme.BodySize);
                b.gameObject.name="Chip_"+chip.Def;
                var text=b.GetComponentInChildren<TextMeshProUGUI>();
                text.text=chip.Name+"\n"+(chip.Held>0?"Owned · "+chip.Held:"View market orders")+"   ›";
                text.alignment=TextAlignmentOptions.Left;
                text.margin=new Vector4(92,0,16,0);
                ItemArt(b.transform,chip.Def,14,56);
                text.color=chip.Def==_selDef?UiTheme.Accent:UiTheme.TextHigh;
                Place((RectTransform)b.transform,y,70);_built.Add(b.gameObject);y+=80;
            }
            UiScroll.Fit(_list,y+16);
        }

        void ItemArt(Transform parent,string def,float left,float size)
        {
            var texture=Resources.Load<Texture2D>("TradeArtwork/"+def);
            if(texture==null){ExemplarUi.Icon(parent,ItemSymbol(def),.02f,.15f,.10f,.85f,UiTheme.Accent);return;}
            var go=new GameObject("ItemArtwork",typeof(RectTransform),typeof(RawImage));
            go.transform.SetParent(parent,false);
            var image=go.GetComponent<RawImage>();image.texture=texture;image.raycastTarget=false;
            var rt=image.rectTransform;rt.anchorMin=rt.anchorMax=new Vector2(0,.5f);
            rt.pivot=new Vector2(0,.5f);rt.sizeDelta=new Vector2(size,size);rt.anchoredPosition=new Vector2(left,0);
        }

        void ItemHeader(string def,string title,string detail,ref float y)
        {
            var go=new GameObject("SelectedItem",typeof(RectTransform));go.transform.SetParent(_content,false);
            var rt=(RectTransform)go.transform;Place(rt,y,80);_built.Add(go);
            ItemArt(rt,def,0,76);
            var name=UiTheme.Label(rt,"Name",title+"\n<size=80%>"+detail+"</size>",UiTheme.HeadSize,UiTheme.TextHigh,TextAlignmentOptions.Left);
            Inset(name,92,0);y+=96;
        }

        string ItemSymbol(string def)
        {
            foreach(var row in _catalog)if(row.def_id==def)return row.kind=="Consumable"?"Apothecary":row.kind=="Card"?"Cards":"Inventory";
            return "Inventory";
        }

        void Hint(RectTransform parent,string title,string message)
        {
            var label=UiTheme.Label(parent,"SelectionHint",title+"\n\n"+message,UiTheme.BodySize,UiTheme.TextLow,TextAlignmentOptions.TopLeft);
            label.textWrappingMode=TextWrappingModes.Normal;Place(label.rectTransform,20,160);_built.Add(label.gameObject);
        }

        void BuildBook(ref float y)
        {
            var book = JsonUtility.FromJson<OrderList>("{\"rows\":" + (string.IsNullOrEmpty(_bookJson) ? "[]" : _bookJson) + "}");
            var rows = book?.rows ?? new Order[0];
            if (rows.Length == 0)
            {
                var none = UiTheme.Label(_content, "Empty", Loc.Get("UI_TRADE_NO_OPEN_ORDERS"), UiTheme.BodySize,
                                         UiTheme.TextLow, TextAlignmentOptions.Left);
                Place(none.rectTransform, y, 24f);
                _built.Add(none.gameObject);
                y += 24f;
                return;
            }
            foreach (var o in rows)
            {
                var row = UiTheme.Box(_content, "O" + o.id, UiTheme.Surface);
                UiTheme.Border(row.rectTransform, UiTheme.Line);
                Place(row.rectTransform, y, RowH);
                _built.Add(row.gameObject);

                var who = UiTheme.Label(row.rectTransform, "W", $"×{o.qty:N0}", UiTheme.BodySize,
                                        UiTheme.TextHigh, TextAlignmentOptions.Left);
                Inset(who, UiTheme.Pad, 200f);
                var price = UiTheme.Label(row.rectTransform, "P", $"{o.unit_price:N0} Gil each",
                                          UiTheme.BodySize, UiTheme.Accent, TextAlignmentOptions.Right);
                Inset(price, UiTheme.Pad, UiTheme.Pad);
                y += RowH + Gap;
            }
        }

        void BuildAction(ref float y, Roi.Sim.SimSession sim)
        {
            bool buying = _mode == Mode.Buy;
            long best = BestPrice();
            _qty = Stepper.Create(_content, "Quantity", 1, 1, buying ? 999 : Held(sim, _selDef));
            Place(_qty.Root, y, Stepper.H);
            _qty.FitTrade();
            _built.Add(_qty.Root.gameObject);
            y += Stepper.H + Gap;

            // The default price is the BOOK's, not a guess: buying at the cheapest ask fills, and
            // listing at it is the price that actually competes.
            _price = Stepper.Create(_content, buying ? "Max price each" : "Price each",
                                    best > 0 ? best : 1, 1, 99_999_999);
            Place(_price.Root, y, Stepper.H);
            _price.FitTrade();
            _built.Add(_price.Root.gameObject);
            y += Stepper.H + 20;

            var btn = HudBar.MakeButton(_content, buying ? "Buy" : "List", "", 300f, UiLayout.Touch,
                                        buying ? Buy : (System.Action)List);
            btn.interactable = !_busy && (!buying || best > 0) && (buying || Held(sim, _selDef) > 0);
            Place(btn.GetComponent<RectTransform>(), y, UiLayout.Touch);
            _built.Add(btn.gameObject);
            y += UiLayout.Touch;

            var note = UiTheme.Label(_content, "Note", buying
                ? "Your Gil is held now; the server matches cheapest-first and mails the units, the change and any refund."
                : "The units leave your bag now; the order appears once the server has verified the save.",
                UiTheme.CaptionSize, UiTheme.TextLow, TextAlignmentOptions.TopLeft);
            note.textWrappingMode = TextWrappingModes.Normal;
            Place(note.rectTransform, y + 16, 72f);
            _built.Add(note.gameObject);
            y += 88f;
        }

        void GridCell(RectTransform rt,int index,int columns,float top,float height)
        {
            int col=index%columns;
            rt.anchorMin=new Vector2((float)col/columns,1);rt.anchorMax=new Vector2((float)(col+1)/columns,1);
            rt.pivot=new Vector2(.5f,1);
            rt.offsetMin=new Vector2(12f*col/columns,-top-(index/columns)*(height+12)-height);
            rt.offsetMax=new Vector2(-12f*(columns-1-col)/columns,-top-(index/columns)*(height+12));
        }

        void BuildMine(ref float y)
        {
            Canvas.ForceUpdateCanvases();
            var sim=_driver.Session;
            CatalogChips(sim); // Resolve names even when My listings is the first tab opened.
            var mine=JsonUtility.FromJson<OrderList>("{\"rows\":"+(string.IsNullOrEmpty(_mineJson)?"[]":_mineJson)+"}");
            var rows=mine?.rows??new Order[0];
            if(rows.Length==0){EmptyMarket(Loc.Get("UI_MKT_NOLISTINGS"));return;}
            _status.text=Take()??Loc.Get("UI_MKT_LISTINGSHINT");
            var heading=UiTheme.Label(_list,"ListingsHeading",Loc.Get("UI_MKT_MYLISTINGS")+" · "+rows.Length,UiTheme.BodySize,UiTheme.Accent,TextAlignmentOptions.Left);
            Place(heading.rectTransform,0,24);_built.Add(heading.gameObject);
            Order selected=null;
            int slots=Mathf.Max(8,rows.Length);
            for(int i=0;i<slots;i++)
            {
                if(i<rows.Length)
                {
                    var entry=rows[i];
                    var button=HudBar.MakeGhostButton(_list,"",240,84,()=>{_selectedOrder=entry.id;Rebuild();},UiTheme.CaptionSize);
                    GridCell((RectTransform)button.transform,i,2,40,70);_built.Add(button.gameObject);
                    ItemArt(button.transform,entry.def_id,12,48);
                    var copy=button.GetComponentInChildren<TextMeshProUGUI>();
                    copy.text=NameOf(entry.def_id)+"  ×"+entry.qty+"\n"+entry.unit_price.ToString("N0")+" Gil";
                    copy.alignment=TextAlignmentOptions.Left;copy.margin=new Vector4(76,0,16,0);
                    copy.color=entry.id==_selectedOrder?UiTheme.Accent:UiTheme.TextHigh;
                    if(entry.id==_selectedOrder)selected=entry;
                }
                else
                {
                    var empty=UiTheme.Box(_list,"UnusedListing",new Color(.04f,.07f,.07f,.35f));
                    GridCell(empty.rectTransform,i,2,40,70);UiTheme.Border(empty.rectTransform,new Color(.3f,.35f,.35f,.3f));_built.Add(empty.gameObject);
                }
            }
            float bottom=40+Mathf.CeilToInt(slots/2f)*82-12+20;
            var withdraw=HudBar.MakeButton(_list,Loc.Get("UI_COMMON_WITHDRAW"),"",300,UiLayout.Touch,()=>{if(_selectedOrder!=0)Cancel(_selectedOrder);});
            withdraw.interactable=selected!=null&&!_busy;Place((RectTransform)withdraw.transform,bottom,UiLayout.Touch);_built.Add(withdraw.gameObject);
            UiScroll.Fit(_list,bottom+UiLayout.Touch+12);

            var bagTitle=UiTheme.Label(_content,"BagTitle",Loc.Get("UI_STASH_INVENTORY"),UiTheme.BodySize,UiTheme.Accent,TextAlignmentOptions.Left);
            Place(bagTitle.rectTransform,0,24);_built.Add(bagTitle.gameObject);
            var held=HeldChips(sim);int columns=Mathf.Max(4,Mathf.CeilToInt(_content.rect.width/116f));int bagSlots=Mathf.Max(columns*3,held.Count);
            float cellSize=Mathf.Max(64,(_content.rect.width-12*(columns-1))/columns);
            for(int i=0;i<bagSlots;i++)
            {
                var cell=UiTheme.Box(_content,"BagSlot",UiTheme.Surface);
                GridCell(cell.rectTransform,i,columns,40,cellSize);UiTheme.Border(cell.rectTransform,UiTheme.Line);_built.Add(cell.gameObject);
                if(i>=held.Count)continue;
                var chip=held[i];ItemArt(cell.transform,chip.Def,0,60);
                var art=cell.transform.Find("ItemArtwork") as RectTransform;
                if(art!=null){art.anchorMin=art.anchorMax=new Vector2(.5f,.5f);art.pivot=new Vector2(.5f,.5f);art.anchoredPosition=new Vector2(0,4);}
                var amount=UiTheme.Label(cell.rectTransform,"Amount",chip.Held.ToString("N0"),UiTheme.CaptionSize,UiTheme.TextHigh,TextAlignmentOptions.BottomRight);
                Inset(amount,10,10);amount.rectTransform.offsetMin=new Vector2(10,8);amount.rectTransform.offsetMax=new Vector2(-10,-8);
                var choose=cell.gameObject.AddComponent<Button>();choose.targetGraphic=cell;
                choose.onClick.AddListener(()=>{_mode=Mode.Sell;_selectedOrder=0;_selDef=chip.Def;_selName=chip.Name;_bookJson="";Refresh();});
            }
            y=40+Mathf.CeilToInt((float)bagSlots/columns)*(cellSize+12)-12;
            var hint=UiTheme.Label(_content,"BagHint",Loc.Get("UI_TRADE_BAG_POST_HINT"),UiTheme.CaptionSize,UiTheme.TextLow,TextAlignmentOptions.TopLeft);
            Place(hint.rectTransform,y+20,56);_built.Add(hint.gameObject);y+=76;
        }

        // --- the three actions -------------------------------------------------------------------

        void Buy()
        {
            var sim = _driver != null ? _driver.Session : null;
            if (sim == null || _busy) return;
            long qty = _qty.Value, max = _price.Value;
            _busy = true;
            _status.text = Loc.Get("UI_TRADE_BUYING");
            sim.ApplyInput($"{{\"BuyCommodity\":{{\"def_id\":\"{_selDef}\",\"qty\":{qty},\"max_price\":{max}}}}}");
            Settle("bought — units, change and any refund arrive as mail");
        }

        void List()
        {
            var sim = _driver != null ? _driver.Session : null;
            if (sim == null || _busy) return;
            long qty = _qty.Value, price = _price.Value;
            _busy = true;
            _status.text = Loc.Get("UI_TRADE_LISTING");
            sim.ApplyInput($"{{\"DepositCommodity\":{{\"def_id\":\"{_selDef}\",\"qty\":{qty},\"unit_price\":{price}}}}}");
            Settle("listed");
        }

        /// PUSH, then refetch. The event above is only a local intent until the server has verified
        /// the save that carries it — refetching first shows a book without it and reads as failure.
        void Settle(string done)
        {
            var boot = Roi.Sim.SessionBoot.Instance;
            if (boot == null) { _busy = false; _status.text = Loc.Get("UI_TRADE_NO_SESSION_BOOT_TO_PUSH"); return; }
            boot.PushNow((ok, why) =>
            {
                _busy = false;
                _flash = ok ? done : "the server did not accept it: " + why;
                Refresh();
            });
        }

        void Cancel(long orderId)
        {
            var net = Roi.Net.OnlineService.Instance;
            if (_busy || !net.LoggedIn) return;
            _busy = true;
            _status.text = Loc.Get("UI_TRADE_CANCELLING");
            net.Run(net.Client.CommodityCancel(orderId, (ok, body) =>
            {
                _busy = false;
                _flash = ok ? "cancelled — the unsold units are in your mailbox"
                            : "refused: " + body;
                Refresh();
            }));
        }

        // --- reads ---------------------------------------------------------------------------

        /// The pending message, consumed. Null when there is none.
        string Take() { var f = _flash; _flash = null; return f; }

        long BestPrice()
        {
            var book = JsonUtility.FromJson<OrderList>("{\"rows\":" + (string.IsNullOrEmpty(_bookJson) ? "[]" : _bookJson) + "}");
            long best = 0;
            foreach (var o in book?.rows ?? new Order[0])
                if (best == 0 || o.unit_price < best) best = o.unit_price;
            return best;
        }

        long Held(Roi.Sim.SimSession sim, string defId)
        {
            if (string.IsNullOrEmpty(defId)) return 0;
            long n = 0;
            foreach (var r in HeldRows(sim))
                if (r.def_id == defId) n += System.Math.Max(1, r.quantity);
            return n;
        }

        /// Every row the player holds that the book could list: the bag, then the materials ledger.
        static List<Row> HeldRows(Roi.Sim.SimSession sim)
        {
            var rows = new List<Row>();
            foreach (var key in new[] { "inventory", "materials" })
            {
                var list = JsonUtility.FromJson<RowList>("{\"rows\":" + sim.QueryJson(key) + "}");
                if (list?.rows != null) rows.AddRange(list.rows);
            }
            return rows;
        }

        string NameOf(string defId)
        {
            foreach (var r in _catalog) if (r.def_id == defId) return Display(r);
            return defId;
        }

        static string Display(Row r) => GameText.ItemName(r.def_id, r.name);

        internal static long PurseOf(Roi.Sim.SimSession sim) => Purse(sim);

        static long Purse(Roi.Sim.SimSession sim)
        {
            var m = System.Text.RegularExpressions.Regex.Match(sim.QueryJson("purse"), "\"gil\":(-?\\d+)");
            return m.Success ? long.Parse(m.Groups[1].Value) : 0;
        }

        // --- widgets ---------------------------------------------------------------------------

        /// A number with a minus and a plus, and a field for typing it. No drag anywhere (mobile is
        /// planned), and no free-text-only field either: a price is the one input where a typo is
        /// expensive, so the two buttons are the safe path and the field is the fast one.
        internal sealed class Stepper
        {
            public const float H = 40f;
            public RectTransform Root;
            public long Value => long.TryParse(_field.text, out var v) ? Mathf.Clamp((int)v, (int)_min, (int)_max) : _min;
            TMP_InputField _field;
            long _min, _max;

            internal static Stepper Create(Transform parent, string label, long value, long min, long max)
            {
                var s = new Stepper { _min = min, _max = System.Math.Max(min, max) };
                var go = new GameObject("Stepper_" + label, typeof(RectTransform));
                go.transform.SetParent(parent, false);
                s.Root = (RectTransform)go.transform;

                var cap = UiTheme.Label(s.Root, "L", label, UiTheme.BodySize, UiTheme.TextMid,
                                        TextAlignmentOptions.Left);
                var cr = cap.rectTransform;
                cr.anchorMin = new Vector2(0f, 0f); cr.anchorMax = new Vector2(0f, 1f);
                cr.pivot = new Vector2(0f, 0.5f);
                cr.sizeDelta = new Vector2(180f, 0f);
                cr.anchoredPosition = new Vector2(0f, 0f);

                var minus = HudBar.MakeButton(s.Root, "−", "", 40f, H, () => s.Bump(-1));
                Pin(minus, 190f);
                var box = UiTheme.Box(s.Root, "Field", UiTheme.Panel);
                UiTheme.Border(box.rectTransform, UiTheme.Line);
                var brt = box.rectTransform;
                brt.anchorMin = brt.anchorMax = new Vector2(0f, 0.5f);
                brt.pivot = new Vector2(0f, 0.5f);
                brt.sizeDelta = new Vector2(140f, H);
                brt.anchoredPosition = new Vector2(234f, 0f);

                var area = new GameObject("TextArea", typeof(RectTransform));
                area.transform.SetParent(brt, false);
                var art = (RectTransform)area.transform;
                art.anchorMin = Vector2.zero; art.anchorMax = Vector2.one;
                art.offsetMin = new Vector2(UiTheme.Gap, 1f); art.offsetMax = new Vector2(-UiTheme.Gap, -1f);
                area.AddComponent<RectMask2D>();
                var txt = UiTheme.Label(art, "Text", "", UiTheme.HeadSize, UiTheme.TextHigh,
                                        TextAlignmentOptions.Center);
                txt.overflowMode = TextOverflowModes.Overflow;
                var tr = txt.rectTransform;
                tr.anchorMin = Vector2.zero; tr.anchorMax = Vector2.one;
                tr.offsetMin = Vector2.zero; tr.offsetMax = Vector2.zero;

                s._field = box.gameObject.AddComponent<TMP_InputField>();
                s._field.textViewport = art;
                s._field.textComponent = txt;
                s._field.targetGraphic = box;
                s._field.lineType = TMP_InputField.LineType.SingleLine;
                s._field.contentType = TMP_InputField.ContentType.IntegerNumber;
                s._field.characterLimit = 9;
                s._field.text = value.ToString();

                var plus = HudBar.MakeButton(s.Root, "+", "", 40f, H, () => s.Bump(1));
                Pin(plus, 380f);
                return s;
            }

            // Only commodity Trade uses this fluid layout; gear market retains its existing widget.
            internal void FitTrade()
            {
                var label=(RectTransform)Root.GetChild(0);
                label.anchorMin=Vector2.zero;label.anchorMax=new Vector2(.42f,1);
                label.offsetMin=Vector2.zero;label.offsetMax=new Vector2(-8,0);
                var minus=(RectTransform)Root.GetChild(1);
                minus.anchorMin=minus.anchorMax=new Vector2(.42f,.5f);minus.anchoredPosition=Vector2.zero;
                var field=(RectTransform)Root.GetChild(2);
                field.anchorMin=new Vector2(.42f,0);field.anchorMax=Vector2.one;
                field.offsetMin=new Vector2(48,0);field.offsetMax=new Vector2(-48,0);
                var plus=(RectTransform)Root.GetChild(3);
                plus.anchorMin=plus.anchorMax=new Vector2(1,.5f);plus.pivot=new Vector2(1,.5f);plus.anchoredPosition=Vector2.zero;
            }

            static void Pin(Button b, float x)
            {
                var rt = (RectTransform)b.transform;
                rt.anchorMin = rt.anchorMax = new Vector2(0f, 0.5f);
                rt.pivot = new Vector2(0f, 0.5f);
                rt.anchoredPosition = new Vector2(x, 0f);
            }

            /// Steps by ORDER OF MAGNITUDE once the number is big: +1 on a price of 40,000 is a
            /// button nobody can use.
            void Bump(int dir)
            {
                long v = Value;
                long step = v >= 10000 ? 1000 : v >= 1000 ? 100 : v >= 100 ? 10 : 1;
                v = Mathf.Clamp((int)(v + dir * step), (int)_min, (int)_max);
                _field.text = v.ToString();
            }
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

        [System.Serializable]
        class Row { public string def_id, name, kind; public int quantity; }
        [System.Serializable] class RowList { public Row[] rows; }
        [System.Serializable]
        class Order { public long id, qty, unit_price; public string def_id, seller; }
        [System.Serializable] class OrderList { public Order[] rows; }
    }
}
