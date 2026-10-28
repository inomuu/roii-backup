// SHOP — the Gil sink and the consumable supply, ported from `shop_view.gd`.
//
// Buy-focused, as there: selling is click-to-sell in the bag. The catalog is the SIM's (`shop` —
// every def with `buy_price > 0`), and a purchase is `InputEvent::BuyItem`, so the price, the
// affordability and the bag-full refusal are all decided in one place. A client that priced items
// itself would eventually disagree with the sim and refuse a purchase the player was told they
// could make.
//
// THE CATEGORIES ARE COPIED, including the reason the last one exists: `TabOf` returns the FIRST
// match and Materials is the fallback, so Regalia needs its own bucket or a new ItemKind is quietly
// filed as ore.
//
// A refusal is SILENT in the sim by design (replay safety: a malformed or impossible event is a
// no-op, never a panic), so this checks the purse before and after and says so when nothing moved.
// Otherwise a full bag looks exactly like a working button.
using System.Collections.Generic;
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    public sealed class ShopWindow : MonoBehaviour
    {
        const float TabH = 40f, RowH = 180f, Gap = 14f;
        float _layoutWidth;

        static readonly string[] Tabs = { "Gear", "Potions", "Stones", "Materials", "Exemplar", "Regalia" };

        Roi.Sim.SimDriver _driver;
        RectTransform _content, _tabBar;
        TextMeshProUGUI _gil, _status, _categoryTitle;
        readonly List<GameObject> _built = new();
        readonly List<UnityEngine.UI.Button> _tabBtns = new();
        ShopSelectionArc _selectionLine;
        Row[] _rows = new Row[0];
        int _tab;
        Row _selectedItem;
        TextMeshProUGUI _preview;
        ExemplarEquipmentIcon _previewArt;
        UnityEngine.UI.RawImage _previewPicture;
        UnityEngine.UI.Button _purchase;

        public static ShopWindow Attach(RectTransform body)
        {
            var w = body.gameObject.AddComponent<ShopWindow>();
            w.Build(body);
            return w;
        }

        void Build(RectTransform body)
        {
            float pad = UiTheme.PadLarge;
            var header=body.parent.Find("TitleBar");
            var wallet=UiTheme.Box(header,"ShopWallet",new Color(.025f,.045f,.045f,.8f));
            var wr=wallet.rectTransform;wr.anchorMin=wr.anchorMax=new Vector2(1,.5f);wr.pivot=new Vector2(1,.5f);
            wr.sizeDelta=new Vector2(220,36);wr.anchoredPosition=new Vector2(-24,0);
            var iconGo=new GameObject("GilIcon",typeof(RectTransform),typeof(HomeCurrencyIcon));iconGo.transform.SetParent(wr,false);
            var icon=iconGo.GetComponent<HomeCurrencyIcon>();icon.Kind="Gil";icon.color=Color.white;icon.raycastTarget=false;
            var ir=icon.rectTransform;ir.anchorMin=ir.anchorMax=new Vector2(0,.5f);ir.pivot=new Vector2(0,.5f);ir.sizeDelta=new Vector2(28,28);ir.anchoredPosition=new Vector2(10,0);
            _gil=UiTheme.Label(wr,"Gil","",UiTheme.HeadSize,UiTheme.Accent,TextAlignmentOptions.Right);
            Inset(_gil,48,12);

            _status = UiTheme.Label(body, "Status", "", UiTheme.CaptionSize, UiTheme.TextLow,
                                    TextAlignmentOptions.Left);
            _status.textWrappingMode = TextWrappingModes.NoWrap;
            var sr = _status.rectTransform;
            sr.anchorMin = new Vector2(0f, 1f); sr.anchorMax = new Vector2(1f, 1f);
            sr.pivot = new Vector2(0.5f, 1f);
            sr.offsetMin = new Vector2(pad+300, -40f); sr.offsetMax = new Vector2(-(pad + 260f), -UiTheme.PadSmall);

            _categoryTitle=UiTheme.Label(body,"CategoryTitle","",UiTheme.HeadSize,UiTheme.Accent,TextAlignmentOptions.Left);
            var cr=_categoryTitle.rectTransform;cr.anchorMin=cr.anchorMax=new Vector2(0,1);cr.pivot=new Vector2(0,1);cr.sizeDelta=new Vector2(220,40);cr.anchoredPosition=new Vector2(pad+108,-8);
            var bar = new GameObject("Tabs", typeof(RectTransform));
            bar.transform.SetParent(body, false);
            _tabBar = (RectTransform)bar.transform;
            _tabBar.anchorMin = Vector2.zero; _tabBar.anchorMax = new Vector2(0f, 1f);
            _tabBar.pivot = new Vector2(0.5f, 1f);
            _tabBar.offsetMin = new Vector2(pad, 24);
            _tabBar.offsetMax = new Vector2(pad+84, -56f);

            for (int i = 0; i < Tabs.Length; i++)
            {
                int idx = i;
                var b = HudBar.MakeGhostButton(_tabBar, Loc.Get("UI_SHOP_TAB_"+i), 60f, 60, () => Select(idx),
                                               UiTheme.BodySize);
                var rt = (RectTransform)b.transform;
                rt.anchorMin=rt.anchorMax=new Vector2(0,1);rt.pivot=new Vector2(0,1);
                rt.anchoredPosition=new Vector2(0,-i*64);rt.sizeDelta=new Vector2(84,60);
                b.GetComponent<UnityEngine.UI.Image>().color=Color.clear;
                var caption=b.GetComponentInChildren<TextMeshProUGUI>();
                caption.enabled=true;
                caption.fontSize=14f;
                caption.enableAutoSizing=true;caption.fontSizeMax=caption.fontSize;caption.fontSizeMin=11f;
                caption.textWrappingMode=TextWrappingModes.NoWrap;
                caption.alignment=TextAlignmentOptions.Center;
                caption.raycastTarget=false;
                var labelRect=caption.rectTransform;
                labelRect.anchorMin=new Vector2(0,0);labelRect.anchorMax=new Vector2(1,0);
                labelRect.pivot=new Vector2(.5f,0);labelRect.offsetMin=new Vector2(0,0);labelRect.offsetMax=new Vector2(-8,22);
                ExemplarUi.Icon(rt,new[]{"Equipment","Apothecary","Crystal","Inventory","Exemplar","Regalia"}[i],.29f,.42f,.61f,.90f,UiTheme.TextMid);
                _tabBtns.Add(b);

            }

            var seam = new GameObject("SelectionSeam", typeof(RectTransform), typeof(ShopSelectionArc));
            seam.transform.SetParent(_tabBar, false);
            seam.transform.SetAsFirstSibling();
            _selectionLine = seam.GetComponent<ShopSelectionArc>();
            _selectionLine.color = Color.white;
            _selectionLine.raycastTarget = false;
            var seamRect = _selectionLine.rectTransform;
            seamRect.anchorMin = new Vector2(1, 0); seamRect.anchorMax = Vector2.one;
            seamRect.offsetMin = new Vector2(-48, 0); seamRect.offsetMax = new Vector2(16, 0);
            var host = new GameObject("Scroll", typeof(RectTransform));
            host.transform.SetParent(body, false);
            var hrt = (RectTransform)host.transform;
            hrt.anchorMin = Vector2.zero; hrt.anchorMax = Vector2.one;
            hrt.offsetMin = new Vector2(pad+108, 24);
            hrt.offsetMax = new Vector2(-pad, -56);
            hrt.anchorMax=new Vector2(.65f,1);
            _content = UiScroll.Create(hrt, out _);
            var artHost=ExemplarUi.Rect(body,"PreviewArt",.75f,.55f,.94f,.87f);
            _previewArt=ExemplarEquipmentIcon.Create(artHost,"Weapon");_previewArt.enabled=false;
            _previewPicture=artHost.gameObject.AddComponent<UnityEngine.UI.RawImage>();_previewPicture.enabled=false;_previewPicture.raycastTarget=false;
            _preview=ExemplarUi.Text(body,"Preview",Loc.Get("UI_COLLECTION_SELECT"),.69f,.26f,.975f,.52f,UiTheme.HeadSize,UiTheme.TextHigh);
            _preview.textWrappingMode=TextWrappingModes.Normal;
            _purchase=HudBar.MakeButton(body,Loc.Get("UI_COMMON_BUY"),"",200,UiLayout.Touch,()=>{if(_selectedItem!=null)Preview(_selectedItem);},true);
            var pr=(RectTransform)_purchase.transform;pr.anchorMin=pr.anchorMax=new Vector2(.975f,.06f);pr.pivot=new Vector2(1,0);pr.anchoredPosition=Vector2.zero;_purchase.interactable=false;

        }

        void LateUpdate()
        {
            if (_content != null && _rows.Length > 0 && Mathf.Abs(_layoutWidth - _content.rect.width) > 1f)
                Rebuild();
        }

        public void BrowseSupplies() => Select(1);

        void Select(int tab)
        {
            bool changed=_tab!=tab;
            if(changed){_selectedItem=null;_purchase.interactable=false;_preview.text=Loc.Get("UI_COLLECTION_SELECT");_previewArt.enabled=_previewPicture.enabled=false;}
            _tab = tab;
            Rebuild();
            if(changed)UiPageTransition.Play(_content.GetComponentInParent<UnityEngine.UI.ScrollRect>().viewport);
            var scroll = _content.GetComponentInParent<UnityEngine.UI.ScrollRect>();
            if (scroll != null) scroll.verticalNormalizedPosition = 1f;
        }

        public void Refresh()
        {
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var sim = _driver != null ? _driver.Session : null;
            if (sim == null)
            {
                _gil.text = ""; _status.text = Loc.Get("UI_SHOP_NO_SESSION");
                Clear();
                return;
            }
            // The catalog is static, so it is read once — only the purse and affordability change.
            if (_rows.Length == 0)
            {
                var raw = sim.QueryJson("shop");
                var list = JsonUtility.FromJson<RowList>("{\"rows\":" + raw + "}");
                _rows = list?.rows ?? new Row[0];
                // An empty catalog is worth saying out loud: it means the sim answered nothing,
                // which on this side usually means the Editor is still holding an older plugin.
                if (_rows.Length == 0)
                    Debug.LogWarning($"SHOP: the catalog parsed to nothing ({raw.Length} bytes of JSON)");
            }
            Rebuild();
        }

        void Rebuild()
        {
            Clear();
            var sim = _driver != null ? _driver.Session : null;
            if (sim == null) return;
            long gil = Purse(sim);
            _gil.text = $"{gil:N0} Gil";

            _categoryTitle.text=Loc.Get("UI_SHOP_TAB_"+_tab);
            _selectionLine.Select(_tab);
            for (int i = 0; i < _tabBtns.Count; i++)
            {
                var t = _tabBtns[i].GetComponentInChildren<TextMeshProUGUI>();
                t.color = i == _tab ? UiTheme.Accent : UiTheme.TextMid;
                var categoryIcon=_tabBtns[i].GetComponentInChildren<HomeIcon>();if(categoryIcon!=null)categoryIcon.color=i==_tab?UiTheme.Accent:UiTheme.TextLow;
                _tabBtns[i].transition=UnityEngine.UI.Selectable.Transition.None;
                var bg=_tabBtns[i].GetComponent<UnityEngine.UI.Image>();bg.color=Color.clear;bg.CrossFadeColor(Color.clear,0,true,true);
            }

            float y = 0f;
            int shown = 0;
            _layoutWidth = _content.rect.width;
            int columns = 2;
            float cardWidth = (_layoutWidth - (columns - 1) * Gap) / columns;
            foreach (var it in _rows)
            {
                if (TabOf(it) != _tab) continue;
                bool afford = gil >= it.buy_price;
                var row = UiTheme.Box(_content, "I_" + it.def_id, UiTheme.Surface);
                UiTheme.Border(row.rectTransform, UiTheme.Line);
                var surface=new GameObject("CardLight",typeof(RectTransform));surface.transform.SetParent(row.transform,false);
                var wash=surface.AddComponent<InventoryBackdrop>();wash.raycastTarget=false;
                wash.rectTransform.anchorMin=Vector2.zero;wash.rectTransform.anchorMax=Vector2.one;wash.rectTransform.offsetMin=new Vector2(2,2);wash.rectTransform.offsetMax=new Vector2(-2,-2);
                var rr = row.rectTransform;
                rr.anchorMin = rr.anchorMax = new Vector2(0, 1);
                rr.pivot = new Vector2(0, 1);
                rr.sizeDelta = new Vector2(cardWidth, RowH);
                rr.anchoredPosition = new Vector2((shown % columns) * (cardWidth + Gap), -(shown / columns) * (RowH + Gap));
                shown++;
                _built.Add(row.gameObject);

                var art = ExemplarEquipmentIcon.Create(rr, null);
                art.SetItem(new ItemView { def_id=it.def_id, kind=it.kind }, null);
                art.rectTransform.anchorMin = new Vector2(.22f, .40f);
                art.rectTransform.anchorMax = new Vector2(.78f, .95f);
                art.rectTransform.offsetMin = art.rectTransform.offsetMax = Vector2.zero;
                // Gear uses its actual artwork; non-equipment uses an honest category symbol.
                if (it.kind != "Weapon" && it.kind != "Armor")
                {
                    art.gameObject.SetActive(false);
                    var texture=Resources.Load<Texture2D>("TradeArtwork/"+it.def_id);
                    var symbol = new GameObject("Category", typeof(RectTransform));
                    symbol.transform.SetParent(rr, false);
                    var category=it.kind=="Consumable"?"Consumable":it.kind=="Exemplar"?"Exemplar":it.kind=="Regalia"?"Regalia":"Inventory";
                    var icon=symbol.AddComponent<HomeIcon>();icon.Kind=category;
                    if(texture!=null){icon.enabled=false;var artObject=new GameObject("ItemArtwork",typeof(RectTransform),typeof(UnityEngine.UI.RawImage));artObject.transform.SetParent(symbol.transform,false);var picture=artObject.GetComponent<UnityEngine.UI.RawImage>();picture.texture=texture;picture.raycastTarget=false;picture.rectTransform.anchorMin=Vector2.zero;picture.rectTransform.anchorMax=Vector2.one;picture.rectTransform.offsetMin=picture.rectTransform.offsetMax=Vector2.zero;
                        }
                    icon.raycastTarget=false; icon.color=UiTheme.TextMid;
                    icon.rectTransform.anchorMin=new Vector2(.04f,.16f); icon.rectTransform.anchorMax=new Vector2(.34f,.84f);
                    icon.rectTransform.anchorMin=icon.rectTransform.anchorMax=new Vector2(.5f,.68f);icon.rectTransform.pivot=new Vector2(.5f,.5f);icon.rectTransform.sizeDelta=new Vector2(88,88);icon.rectTransform.anchoredPosition=Vector2.zero;
                }

                var name = UiTheme.Label(row.rectTransform, "N", Display(it), UiTheme.BodySize,
                                         UiTheme.TextHigh, TextAlignmentOptions.Left);
                CardText(name, .20f, .38f);
                name.enableAutoSizing=true; name.fontSizeMin=12; name.fontSizeMax=Screen.height<600?UiTheme.BodySize*1.6f:UiTheme.BodySize;

                var price = UiTheme.Label(row.rectTransform, "P", $"{it.buy_price:N0} Gil",
                                          UiTheme.BodySize,
                                          afford ? UiTheme.Accent : UiTheme.TextLow,
                                          TextAlignmentOptions.Left);
                CardText(price, .02f, .20f);if(Screen.height<600)price.fontSize=UiTheme.BodySize*1.6f;if(Screen.height<600)price.fontSize=UiTheme.BodySize*1.6f;

                var entry=it;
                var select=row.gameObject.AddComponent<UnityEngine.UI.Button>();select.targetGraphic=row;
                select.onClick.AddListener(()=>Inspect(entry));

            }

            y = Mathf.CeilToInt(shown / (float)columns) * (RowH + Gap);

            if (shown == 0)
            {
                int next=-1;for(int i=0;i<Tabs.Length;i++)if(i!=_tab&&System.Array.Exists(_rows,item=>TabOf(item)==i)){next=i;break;}
                int destination=next;
                var none=MenuNotice.Create(_content,"Shop",GameText.ScreenName("Shop"),Loc.Get("UI_SHOP_NOTHING_IN_THIS_CATEGORY"),
                    destination>=0?(System.Action)(()=>Select(destination)):null,Loc.Get("UI_UX_BROWSE_ITEMS"));
                _built.Add(none);
                y=300f;
            }
            UiScroll.Fit(_content, y + UiTheme.Pad);
        }

        void Inspect(Row item)
        {
            _selectedItem=item;_preview.text=Loc.Fmt("UI_SHOP_PREVIEW",Display(item),item.buy_price.ToString("N0"));
            var texture=Resources.Load<Texture2D>("TradeArtwork/"+item.def_id);
            _previewPicture.texture=texture;_previewPicture.enabled=texture!=null;
            _previewArt.enabled=texture==null;_previewArt.SetItem(new ItemView{def_id=item.def_id,kind=item.kind},null);
            _purchase.interactable=true;
        }

        void Preview(Row item)
        {
            if(UiConfirm.BlocksNavigation)return;
            bool afford=_driver!=null&&Purse(_driver.Session)>=item.buy_price;
            string detail=Loc.Fmt("UI_SHOP_PREVIEW",Display(item),item.buy_price.ToString("N0"));
            if(afford)UiConfirm.Ask(Display(item),detail,Loc.Get("UI_COMMON_BUY"),()=>Buy(item.def_id),false);
            else UiConfirm.Ask(Display(item),detail+"\n\n"+Loc.Get("UI_UX_NEED_GIL"),Loc.Get("UI_COMMON_CANCEL"),()=>{},false,true);
        }

        /// One at a time. The Godot shop buys ten with Shift held; there is no Shift on a phone, so
        /// that shortcut does not come across — a quantity control is the honest replacement and it
        /// is not built yet.
        void Buy(string defId)
        {
            var sim = _driver != null ? _driver.Session : null;
            if (sim == null) return;
            long before = Purse(sim);
            sim.ApplyInput("{\"BuyItem\":{\"def_id\":\"" + defId + "\",\"qty\":1}}");
            long after = Purse(sim);
            // The sim no-ops what it will not do, so "did the purse move" is the only honest test.
            _status.text = after < before
                ? Loc.Fmt("UI_UX_BOUGHT",GameText.ItemName(defId,defId),before-after)
                : Loc.Get("UI_UX_PURCHASE_REFUSED");
            _status.color=after<before?UiTheme.Accent:new Color(.95f,.55f,.49f);
            Rebuild();
        }

        static long Purse(Roi.Sim.SimSession sim)
        {
            var p = JsonUtility.FromJson<PurseDTO>(sim.QueryJson("purse"));
            return p != null ? p.gil : 0;
        }

        /// First match wins, and Materials is the FALLBACK — which is why Regalia has a bucket of
        /// its own rather than landing among the ore.
        static int TabOf(Row it)
        {
            if (it.kind == "Weapon" || it.kind == "Armor") return 0;
            if (it.kind == "Consumable") return 1;
            if (it.is_orb) return 2;
            if (it.kind == "Exemplar") return 4;
            if (it.kind == "Regalia") return 5;
            return 3;
        }

        static string Display(Row it) => GameText.ItemName(it.def_id, it.name);

        static void CardText(TextMeshProUGUI t, float bottom, float top)
        {
            t.rectTransform.anchorMin = new Vector2(.03f, bottom);
            t.rectTransform.anchorMax = new Vector2(1, top);
            t.rectTransform.offsetMin = new Vector2(8,0);
            t.rectTransform.offsetMax = new Vector2(-20,0);
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
        class Row
        {
            public string def_id, name, kind, rarity;
            public long buy_price;
            /// The sim's own answer to "is this a Star Stone" — see the `shop` query. The `orb`
            /// field beside it is a tagged enum JsonUtility cannot type, which is why the predicate
            /// is asked for directly.
            public bool is_orb;
        }
        [System.Serializable] class RowList { public Row[] rows; }
        [System.Serializable] class PurseDTO { public long gil, soul; public string zone_id; }
    }
}



