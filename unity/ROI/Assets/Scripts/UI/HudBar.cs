// The action bar — the bottom strip and the "Menu" grid behind it, ported from
// godot/views/hud_bar.gd. One responsibility, as there: opening and closing windows.
//
// This is STEP ONE of the UI port: the bar, its hotkeys, the exclusive-open rule, the system menu
// on Escape, and a window FRAME per entry. Most screens are not ported — each of those entries
// opens an empty `UiWindow` that says so. Doing the frame first is what makes porting a screen a
// matter of filling one in rather than also deciding where it sits.
//
// LAYERS follow the Godot convention (see the memory note ui-canvas-layer-zorder): the arena
// overlay draws with the world at 0, windows at 10, the bar at 40 so it stays clickable over an
// open screen, and the system menu at 50 above all of it.
//
// Every colour, size and face here comes from UiTheme. The bar is the surface a player looks at
// for the whole session, so it is the one that most has to look bought rather than assembled.
//
// Not ported yet, and each for a reason: the Farm toggle (the sim input it flips does not exist on
// this side), the daily badge (it rides a /save reply there is no server for here), and the
// market's two-views-one-window pairing (both are empty frames).
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.InputSystem;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class HudBar : MonoBehaviour
    {
        // Sized for a THUMB, not a cursor (2026-09-07, the mobile-shaped pass): every control on
        // the bar and in the menu is at least `UiLayout.Touch` tall. A 40 px button is fine with a
        // mouse and a coin-flip with a finger, and this is the surface a player touches most.
        const float BarH = UiLayout.BottomBar, BtnW = 138f, BtnH = UiLayout.Touch;
        const float BtnGap = 8f, BtnPad = UiTheme.Pad;
        const int MenuCols = 3;
        const float CellW = 176f, CellH = UiLayout.Touch, CellGap = 8f, MenuPad = UiTheme.Pad;

        /// label, hotkey, and nothing else yet — the Godot list carries a node per row and a
        /// `station` for the two entries that share the craft bench; both arrive with the screens.
        struct Entry
        {
            public string Label; public string KeyLabel; public Key Key;
            public Entry(string label, string keyLabel, Key key) { Label = label; KeyLabel = keyLabel; Key = key; }
        }

        // The order and the keys are hud_bar.gd's `_buttons()`, verbatim.
        static readonly Entry[] Entries =
        {
            // Character is NOT one of hud_bar.gd's thirty: there, the sheet and the bag share one
            // window. The user split them (2026-09-07) — the bag is the bag and the character gets
            // its own screen.
            //
            // NO HOTKEY, and it is not an oversight. `hud_bar.gd` records that A-Z is full; the two
            // letters that look free are not (Q is the DPS meter, W the companion widget), and F —
            // the one this briefly took — is the FARM toggle. It opens from the Menu grid like
            // Crystal and Daily do.
            new Entry("Character", "", Key.None),
            new Entry("Inventory", "I", Key.I), new Entry("Skills", "K", Key.K),
            new Entry("Cards", "C", Key.C), new Entry("Stash", "S", Key.S),
            new Entry("Album", "A", Key.A), new Entry("Provisions", "M", Key.M),
            new Entry("Exemplar", "U", Key.U), new Entry("Regalia", "L", Key.L),
            new Entry("Apothecary", "X", Key.X),
            new Entry("Mail", "J", Key.J), new Entry("Trade", "G", Key.G),
            new Entry("Party", "Y", Key.Y), new Entry("Paths", "P", Key.P),
            new Entry("Shop", "B", Key.B), new Entry("Bestiary", "N", Key.N),
            new Entry("Map", "T", Key.T),
            new Entry("Dungeon", "D", Key.D), new Entry("Options", "O", Key.O),
            new Entry("Pass", "Z", Key.Z), new Entry("Summon", "H", Key.H),
            new Entry("Crystal", "", Key.None), new Entry("Daily", "", Key.None),
            new Entry("Weekly", "", Key.None), new Entry("Commander", "", Key.None),
            new Entry("Outpost", "", Key.None), new Entry("Record", "", Key.None),
            // NOT one of hud_bar.gd's thirty: Godot puts the event board in its HOME mode, which
            // this client has no equivalent of. No hotkey — A-Z is full there and here.
            new Entry("Events", "", Key.None),
            // STORY. It is a MODE more than a screen — pressing a mission closes this window and
            // the dialogue box plays over the arena — but the way in is still the Menu grid.
            new Entry("Story", "", Key.None),
        };

        // The FEW direct buttons kept in the bar; everything else lives behind the Menu popup and
        // is still reachable by its hotkey regardless.
        static readonly string[] Essentials = { "Inventory" };

        readonly Dictionary<string, UiWindow> _windows = new();
        readonly List<(Button btn, string label)> _windowButtons = new();
        Canvas _windowCanvas, _popupCanvas, _rosterCanvas, _barCanvas, _sysCanvas;

        /// Where a POPUP belongs — above every window, below the bar. See `Build`.
        public Transform PopupLayer => _popupCanvas.transform;
        // ABOVE the bar and the roster. A notice that ends the session has to cover the
        // controls it is telling you no longer do anything — the popup layer sits UNDER
        // both, so a "session ended" panel there leaves Menu and Inventory pressable.
        public Transform SystemLayer => _sysCanvas.transform;
        /// The action-bar layer. The Commander's skill row sits here rather than on its own canvas:
        /// it is part of the bar, and anything that covers one should cover both.
        public Transform BarLayer => _barCanvas.transform;

        /// The damage meter, handed over by whoever built it. Q toggles it.
        public DpsMeter Dps { get; set; }

        /// The companion widget. W toggles it. Held for the same reason the meter is.
        public CompanionWidget Companion { get; set; }
        RectTransform _fade;
        Image _fadeImg;
        CombatBar _combat;
        RectTransform _barRoot, _menuPanel, _sysRoot;
        Button _menuBtn;
        Vector2Int _lastScreen;

        public static HudBar Create(Transform parent)
        {
            var go = new GameObject("HudBar");
            go.transform.SetParent(parent, false);
            var bar = go.AddComponent<HudBar>();
            bar.Build();
            return bar;
        }

        public FloatingInventoryHost Floating { get; private set; }

        void Build()
        {
            Floating = FloatingInventoryHost.Create(transform.parent);
            EnsureEventSystem();

            _windowCanvas = NewCanvas("WindowCanvas", 10);
            // POPUPS sit between the windows and the roster, which is the Godot convention
            // (windows 10 / popups 20 / tooltips 30). The run summary and the damage meter live
            // here: one must cover an open screen, the other must not be closed by opening one.
            _popupCanvas = NewCanvas("PopupCanvas", 20);
            // The roster sits ABOVE the windows on purpose: a screen covers the world, and it must
            // not also cover the party — you should be able to keep fighting while you manage.
            _rosterCanvas = NewCanvas("RosterCanvas", 30);
            _barCanvas = NewCanvas("BarCanvas", 40);
            _sysCanvas = NewCanvas("SystemCanvas", 50);
            // Combat owns its roster; management pages choose members within their own headers.
            _combat = CombatBar.Create(_rosterCanvas.transform);

            foreach (var e in Entries)
                if (!_windows.ContainsKey(e.Label))
                {
                    var label = e.Label;
                    // The window is KEYED on the internal label and TITLED with the display one.
                    // They differ where the game's fiction and the code's vocabulary differ.
                    var w = UiWindow.Create(_windowCanvas.transform, GameText.ScreenName(label),
                                            () => Toggle(label));
                    _windows[label] = w;
                    w.SetIdentity(label);
                    // The screens that ARE ported fill their body; the rest keep the placeholder.
                    // These three are the same grid over three different lists — see ItemGridWindow.
                    string queryKey = null, capKey = null, foot = null;
                    switch (label)
                    {
                        case "Skills":
                            w.ClearPlaceholder();
                            var potential = PotentialWindow.Attach(w.Body);
                            w.Opened += potential.Refresh;
                            break;
                        case "Inventory":
                            queryKey = "inventory"; capKey = "inventory_cap";
                            // The Godot bag shares its window with a paper-doll and a stats column.
                            // Those are NOT coming here (user, 2026-09-07): the character gets its
                            // own screen, so this window is the bag and only the bag.
                            foot = "Sell, discard, deposit, the rare net and the tooltip are all here.";
                            break;
                        case "Stash":
                            queryKey = "stash"; capKey = "stash_cap";
                            foot = "Withdraw sends a piece back to the bag.";
                            break;
                        // The album is NOT the same grid as the bag any more: filing a card,
                        // feeding one and binding one are three different moves on the same
                        // collection, and two of them need a second subject.
                        case "Story":
                            w.ClearPlaceholder();
                            var story = StoryWindow.Attach(w.Body);
                            w.Opened += story.Refresh;
                            break;
                        case "Album":
                            w.ClearPlaceholder();
                            var album = AlbumWindow.Attach(w.Body,
                                Loc.Get("UI_ALBUM_HINT"));
                            w.Opened += album.Refresh;
                            break;
                        // TWO MORE COLLECTIONS, and they are NOT the bag. An Exemplar or a Regalia
                        // carrier never enters `inventory`/`stash`/the market: the sim banks it into
                        // its own account-bound list, and those lists are UNCAPPED so a gacha pull
                        // or a mailed reward can never be lost to a full inventory. That is exactly
                        // why they need their own screens — an Excavation reward was landing in
                        // `owned_regalia` with nowhere in the client to see it.
                        // An EXEMPLAR IS A UNIT (0.0.350): no carrier list, no equip — its own
                        // screen lists the units and spends Resonance on them.
                        case "Exemplar":
                            w.ClearPlaceholder();
                            var exu = ExemplarUnitsWindow.Attach(w.Body);
                            w.Opened += exu.Refresh;
                            break;
                        case "Regalia":
                            queryKey = "owned_regalia"; capKey = "";
                            foot = "A member may wear one Regalia at once.";
                            break;
                        case "Party":
                            w.ClearPlaceholder();
                            var party = PartyWindow.Attach(w.Body);
                            w.Opened += party.Refresh;
                            break;
                        case "Provisions":
                            w.ClearPlaceholder();
                            var prov = ProvisionsWindow.Attach(w.Body);
                            w.Opened += prov.Refresh;
                            break;
                        case "Character":
                            w.ClearPlaceholder();
                            var chr = CharacterWindow.Attach(w.Body,
                                "Stat allocation and the skill bar are not ported yet.");
                            w.Opened += chr.Refresh;
                            break;
                        // ONE screen for two entries: the two endpoints answer an identical
                        // payload and even the same eight tasks, so only the period differs.
                        case "Daily":
                        case "Weekly":
                            w.ClearPlaceholder();
                            var period = PeriodWindow.Attach(w.Body, label.ToLowerInvariant(),
                                                             label == "Daily" ? "day" : "week");
                            w.Opened += period.Refresh;
                            break;
                        case "Bestiary":
                            w.ClearPlaceholder();
                            var best = BestiaryWindow.Attach(w.Body);
                            w.Opened += best.Refresh;
                            break;
                        case "Options":
                            w.ClearPlaceholder();
                            var opts = OptionsWindow.Attach(w.Body);
                            w.Opened += opts.Refresh;
                            break;
                        case "Trade":
                            w.ClearPlaceholder();
                            var trade = TradeWindow.Attach(w.Body);
                            w.Opened += trade.Refresh;
                            break;
                        case "Events":
                            w.ClearPlaceholder();
                            var events = EventsWindow.Attach(w.Body);
                            w.Opened += events.Refresh;
                            break;
                        case "Crystal":
                            w.ClearPlaceholder();
                            var crystal = CrystalWindow.Attach(w.Body);
                            w.Opened += crystal.Refresh;
                            break;
                        case "Record":
                            w.ClearPlaceholder();
                            var rec = RecordWindow.Attach(w.Body);
                            w.Opened += rec.Refresh;
                            break;
                        case "Shop":
                            w.ClearPlaceholder();
                            var shop = ShopWindow.Attach(w.Body);
                            w.Opened += shop.Refresh;
                            break;
                        case "Pass":
                            w.ClearPlaceholder();
                            var pass = PassWindow.Attach(w.Body);
                            w.Opened += pass.Refresh;
                            break;
                        // TWO SCREENS, one payload: `/commander/state` answers the level ladder and
                        // the Outpost's accrual together, and the user split the screens (2026-08-30)
                        // because one is a ladder you read and the other is a button you press.
                        case "Commander":
                            w.ClearPlaceholder();
                            var cmdr = CommanderWindow.Attach(w.Body, CommanderWindow.Mode.Ladder);
                            w.Opened += cmdr.Refresh;
                            break;
                        case "Outpost":
                            w.ClearPlaceholder();
                            var post = CommanderWindow.Attach(w.Body, CommanderWindow.Mode.Outpost);
                            w.Opened += post.Refresh;
                            break;
                        case "Mail":
                            w.ClearPlaceholder();
                            var mail = MailWindow.Attach(w.Body);
                            w.Opened += mail.Refresh;
                            break;
                        case "Summon":
                            w.ClearPlaceholder();
                            var summon = SummonWindow.Attach(w.Body);
                            w.Opened += summon.Refresh;
                            break;
                        case "Paths":
                            w.ClearPlaceholder();
                            var paths = PathsWindow.Attach(w.Body);
                            w.Opened += paths.Refresh;
                            break;
                        case "Dungeon":
                            w.ClearPlaceholder();
                            DungeonWindow.Attach(w.Body);
                            break;
                        case "Map":
                            w.ClearPlaceholder();
                            var map = WorldMapWindow.Attach(w.Body);
                            w.Opened += map.Refresh;
                            break;
                        case "Apothecary":
                            w.ClearPlaceholder();
                            var bench = CraftWindow.Attach(w.Body, label.ToLowerInvariant());
                            w.Opened += bench.Refresh;
                            break;
                        case "Cards":
                            w.ClearPlaceholder();
                            var cards = CardsWindow.Attach(w.Body,
                                Loc.Get("UI_CARDS_ALBUM_GUIDANCE"));
                            w.Opened += cards.Refresh;
                            break;
                    }
                    if (queryKey != null)
                    {
                        w.ClearPlaceholder();
                        var grid = ItemGridWindow.Attach(w.Body, label, queryKey, capKey, foot);
                        w.Opened += grid.Refresh;
                    }
                }

            BuildBar();
            BuildMenu();
            BuildSystemMenu();
            SyncActiveStates();
            var cs = ((RectTransform)_windowCanvas.transform).rect.size;
            _lastScreen = new Vector2Int(Mathf.RoundToInt(cs.x), Mathf.RoundToInt(cs.y));
        }

        /// uGUI needs one, and an arena scene built entirely from code has none. The Input System
        /// module, not the legacy one: this project is on the new input backend only.
        static void EnsureEventSystem()
        {
            if (UnityEngine.EventSystems.EventSystem.current != null) return;
            var go = new GameObject("EventSystem");
            go.AddComponent<UnityEngine.EventSystems.EventSystem>();
            go.AddComponent<UnityEngine.InputSystem.UI.InputSystemUIInputModule>();
        }

        Canvas NewCanvas(string name, int order)
        {
            var go = new GameObject(name);
            go.transform.SetParent(transform, false);
            var c = go.AddComponent<Canvas>();
            c.renderMode = RenderMode.ScreenSpaceOverlay;
            c.sortingOrder = order;
            // PIXEL-PERFECT, and the distinction from the arena overlay is the whole reason both
            // exist. Snapping puts every rect and every text baseline on a whole pixel, which is
            // free sharpness for a surface that does not move. The arena's HUD must NOT do this —
            // its numbers rise through the frame, and snapping a moving glyph makes it step. The
            // chrome does not move, so it takes the sharpness.
            c.pixelPerfect = true;
            // SCALES WITH THE SCREEN, where the arena's HUD does not.
            //
            // At a fixed pixel size, a 13 pt label is 13 physical pixels on a 4K panel as well as
            // on a 720p one: illegibly small there, and — the reason it matters here — rendered
            // from the SDF atlas at 13 px no matter how many pixels the display actually has. On a
            // HiDPI screen that throws away most of the resolution the text could have had.
            //
            // The reference is 720p rather than 1080p on purpose: it makes the scale factor 1 at
            // the smallest size worth shipping and >= 1 everywhere above, so the UI never shrinks
            // below the sizes it was designed at. Height-matched, so an ultrawide display widens
            // the layout instead of shrinking every glyph in it.
            var scaler = go.AddComponent<CanvasScaler>();
            scaler.uiScaleMode = CanvasScaler.ScaleMode.ScaleWithScreenSize;
            scaler.referenceResolution = new Vector2(1280f, 720f);
            scaler.screenMatchMode = CanvasScaler.ScreenMatchMode.MatchWidthOrHeight;
            scaler.matchWidthOrHeight = 1f;
            go.AddComponent<GraphicRaycaster>();
            return c;
        }

        // --- the bar ---------------------------------------------------------------------------

        void BuildBar()
        {
            // The FADE the whole bottom band sits on, behind everything else on this canvas. It
            // replaces two stacked opaque strips, which read as a wall — on a phone that is a third
            // of the display spent on a colour, and the fight is the thing worth spending it on.
            // Behind the portraits, not on the higher action-button canvas.
            var fade = UiTheme.Flat(_rosterCanvas.transform, "Fade", Color.white);
            fade.transform.SetAsFirstSibling();
            fade.sprite = UiTheme.Fade(combat: true);
            fade.type = Image.Type.Simple;
            fade.raycastTarget = false;
            _fadeImg = fade;
            _fade = fade.rectTransform;
            _fade.anchorMin = new Vector2(0f, 0f); _fade.anchorMax = new Vector2(1f, 0f);
            _fade.pivot = new Vector2(0.5f, 0f);
            _fade.anchoredPosition = Vector2.zero;

            // The bar itself is now only a hit target and a place to hang buttons: no fill, no
            // rule. The buttons carry their own surface, which is what makes them legible over
            // bright grass without a band behind them.
            var bg = UiTheme.Flat(_barCanvas.transform, "Bar", new Color(0f, 0f, 0f, 0f));
            bg.raycastTarget = false;
            _barRoot = bg.rectTransform;
            _barRoot.anchorMin = _barRoot.anchorMax = Vector2.one;
            _barRoot.pivot = Vector2.one;
            _barRoot.sizeDelta = new Vector2(BtnW * 2 + BtnGap, BtnH);
            // The mode switch and stage breadcrumb occupy the first row.
            _barRoot.anchoredPosition = new Vector2(-BtnPad, -58f);

            float x = 0f;
            _menuBtn = MakeButton(_barRoot, Loc.Get("UI_COMMON_MENU"), "", BtnW, BtnH,
                                  () => SetMenuOpen(!_menuPanel.gameObject.activeSelf));
            DecorateBarButton(_menuBtn, "Options");
            PlaceInBar(_menuBtn, ref x);
            foreach (var label in Essentials)
            {
                var e = Find(label);
                var b = MakeButton(_barRoot, GameText.ScreenName(e.Label), e.KeyLabel,
                                   BtnW, BtnH, () => Toggle(e.Label));
                DecorateBarButton(b, e.Label);
                _windowButtons.Add((b, e.Label));
                PlaceInBar(b, ref x);
            }
            // Home navigation and keyboard shortcuts replace this duplicate top strip.
            // Retain the objects for the existing menu state/focus machinery.
            _barRoot.gameObject.SetActive(false);
        }

        static void DecorateBarButton(Button button,string icon)
        {
            var label=button.transform.Find("Label").GetComponent<TextMeshProUGUI>();
            label.rectTransform.offsetMin=new Vector2(36,0);
            label.alignment=TextAlignmentOptions.MidlineLeft;
            label.fontSize=16;
            var symbol=new GameObject("NavigationIcon",typeof(RectTransform));symbol.transform.SetParent(button.transform,false);
            var g=symbol.AddComponent<HomeIcon>();g.Kind=icon;g.color=UiTheme.Accent;g.raycastTarget=false;
            g.rectTransform.anchorMin=g.rectTransform.anchorMax=new Vector2(0,.5f);
            g.rectTransform.anchoredPosition=new Vector2(21,0);g.rectTransform.sizeDelta=new Vector2(20,20);
            var key=button.transform.Find("Key");
            if(key!=null){var k=key.GetComponent<TextMeshProUGUI>();k.fontSize=11;k.color=UiTheme.TextLow;}
        }

        void PlaceInBar(Button b, ref float x)
        {
            var rt = b.GetComponent<RectTransform>();
            rt.anchorMin = rt.anchorMax = new Vector2(0f, 0.5f);
            rt.pivot = new Vector2(0f, 0.5f);
            rt.anchoredPosition = new Vector2(x, 0f);
            x += rt.sizeDelta.x + BtnGap;
        }

        // A compact category index; secondary destinations use quiet, full-row hit targets.
        void BuildMenu()
        {
            // Opaque for the same reason UiWindow's body is: linear compositing lets a 0.98 panel
            // show far more of the scene than the number suggests.
            var panel = UiTheme.Box(_barCanvas.transform, "MenuPanel", UiTheme.Panel);
            UiTheme.Border(panel.rectTransform, UiTheme.LineStrong);
            _menuPanel = panel.rectTransform;
            _menuPanel.pivot = Vector2.one;
            _menuPanel.anchorMin = _menuPanel.anchorMax = Vector2.one;
            string[][] groups={
                new[]{"Map","Dungeon","Story","Daily","Weekly","Events","Pass","Commander","Outpost","Record","Bestiary"},
                new[]{"Exemplar","Party","Character","Skills","Paths","Cards","Album","Regalia","Provisions","Summon"},
                new[]{"Inventory","Stash","Shop","Trade","Crystal","Apothecary","Mail","Options"}
            };
            string[] categories={"UI_NAV_ADVENTURE","UI_NAV_ROSTER","UI_NAV_SUPPLIES"};
            float headH=76f;
            float width=760f;
            float tabW=(width-MenuPad*2-CellGap*2)/3;
            float itemW=(width-MenuPad*2-CellGap)/2;
            _menuPanel.sizeDelta=new Vector2(width,244+3*(CellH+CellGap)+MenuPad*2+headH);
            var sections=new GameObject[groups.Length];
            var tabs=new Button[groups.Length];
            void SelectCategory(int selected) {
                for(int i=0;i<sections.Length;i++) {
                    if(sections[i]!=null)sections[i].SetActive(i==selected);
                    if(tabs[i]!=null)SetPressed(tabs[i],i==selected);
                }
            }
            for(int col=0;col<groups.Length;col++) {
                int category=col;
                tabs[col]=MakeMenuNavigation(_menuPanel,Loc.Get(categories[col]),"",tabW,CellH,()=>SelectCategory(category),true);
                var hr=(RectTransform)tabs[col].transform;hr.anchorMin=hr.anchorMax=new Vector2(0,1);hr.pivot=new Vector2(0,1);
                hr.anchoredPosition=new Vector2(MenuPad+col*(tabW+CellGap),-MenuPad);
                var section=new GameObject("MenuCategory"+col,typeof(RectTransform));section.transform.SetParent(_menuPanel,false);sections[col]=section;
                var sectionRect=(RectTransform)section.transform;sectionRect.anchorMin=Vector2.zero;sectionRect.anchorMax=Vector2.one;sectionRect.offsetMin=sectionRect.offsetMax=Vector2.zero;
                if(col==0) {
                    var art=new GameObject("AtlasArtwork",typeof(RectTransform),typeof(RawImage));art.transform.SetParent(sectionRect,false);
                    var raw=art.GetComponent<RawImage>();raw.texture=Resources.Load<Texture2D>("HomeUI/MenuAtlas");raw.raycastTarget=false;
                    var ar=raw.rectTransform;ar.anchorMin=ar.anchorMax=new Vector2(0,1);ar.pivot=new Vector2(0,1);ar.anchoredPosition=new Vector2(MenuPad,-(MenuPad+headH));ar.sizeDelta=new Vector2(width-MenuPad*2,230);
                }
                for(int row=0;row<groups[col].Length;row++) {
                    string id=groups[col][row];var e=System.Array.Find(Entries,item=>item.Label==id);
                    float rowW=col==0 && row>=2?(width-MenuPad*2-CellGap*2)/3:itemW;
                    var button=MakeMenuNavigation(sectionRect,GameText.ScreenName(id),e.KeyLabel,rowW,CellH,()=>{Toggle(id);SetMenuOpen(false);});
                    var label=button.transform.Find("Label").GetComponent<TextMeshProUGUI>();label.alignment=TextAlignmentOptions.Left;
                    label.rectTransform.offsetMin=new Vector2(39,0);label.fontSize=14;
                    _windowButtons.Add((button,id));
                    var r=button.GetComponent<RectTransform>();r.anchorMin=r.anchorMax=new Vector2(0,1);r.pivot=new Vector2(0,1);
                    r.anchoredPosition=new Vector2(MenuPad+(row%2)*(itemW+CellGap),-(MenuPad+headH+(row/2)*(CellH+CellGap)));
                    if(col==0) {
                        if(row<2) r.anchoredPosition=new Vector2(MenuPad+row*(itemW+CellGap),-(MenuPad+headH+176));
                        else r.anchoredPosition=new Vector2(MenuPad+((row-2)%3)*(rowW+CellGap),-(MenuPad+headH+244+((row-2)/3)*(CellH+CellGap)));
                    }
                    var icon=new GameObject("Icon",typeof(RectTransform));icon.transform.SetParent(r,false);
                    var graphic=icon.AddComponent<HomeIcon>();graphic.Kind=id;graphic.color=UiTheme.Accent;graphic.raycastTarget=false;
                    graphic.rectTransform.anchorMin=graphic.rectTransform.anchorMax=new Vector2(0,.5f);graphic.rectTransform.anchoredPosition=new Vector2(21,0);graphic.rectTransform.sizeDelta=new Vector2(22,22);
                }
            }
            // Fine rules and a central jewel echo the celestial icon set without framing every row.
            var rule=UiTheme.Flat(_menuPanel,"CategoryRule",new Color(.65f,.58f,.38f,.32f));
            rule.raycastTarget=false;
            var rr=rule.rectTransform;rr.anchorMin=rr.anchorMax=new Vector2(.5f,1);rr.sizeDelta=new Vector2(width-MenuPad*2,1);rr.anchoredPosition=new Vector2(0,-(MenuPad+CellH+12));
            var jewel=UiTheme.Flat(_menuPanel,"CategoryJewel",UiTheme.Accent);jewel.raycastTarget=false;
            var jr=jewel.rectTransform;jr.anchorMin=jr.anchorMax=new Vector2(.5f,1);jr.sizeDelta=new Vector2(5,5);jr.anchoredPosition=rr.anchoredPosition;jr.localRotation=Quaternion.Euler(0,0,45);
            SelectCategory(0);
            _menuPanel.gameObject.SetActive(false);
        }

        void SetMenuOpen(bool on)
        {
            _menuPanel.gameObject.SetActive(on);
            SetPressed(_menuBtn, on);
            if (on)
            {
                float top = -_barRoot.anchoredPosition.y + BtnH + UiTheme.Gap;
                var canvasSize = ((RectTransform)_barCanvas.transform).rect.size;
                float scale = Mathf.Min(1f, (canvasSize.y - top - BtnPad) / _menuPanel.rect.height,
                                        (canvasSize.x - BtnPad * 2) / _menuPanel.rect.width);
                _menuPanel.localScale = Vector3.one * Mathf.Max(.1f, scale);
                _menuPanel.anchoredPosition = new Vector2(-BtnPad, -top);
            }
        }

        // Escape with nothing open: Return to Login / Quit / Cancel.
        void BuildSystemMenu()
        {
            var dim = UiTheme.Flat(_sysCanvas.transform, "Dim", UiTheme.Scrim);
            _sysRoot = dim.rectTransform;
            _sysRoot.anchorMin = Vector2.zero; _sysRoot.anchorMax = Vector2.one;
            _sysRoot.offsetMin = Vector2.zero; _sysRoot.offsetMax = Vector2.zero;

            var p = UiTheme.Box(_sysRoot, "Panel", UiTheme.Panel);
            DialogSurface.Apply(p);
            var panel = p.rectTransform;
            panel.anchorMin = panel.anchorMax = new Vector2(0.5f, 0.5f);
            panel.pivot = new Vector2(0.5f, 0.5f);
            panel.sizeDelta = new Vector2(340f, 232f);

            var title = UiTheme.Caps(panel, "Title", Loc.Get("UI_HUD_BAR_SYSTEM"), UiTheme.HeadSize,
                                     UiTheme.TextMid, TextAlignmentOptions.Center);
            var tr = title.rectTransform;
            tr.anchorMin = new Vector2(0f, 1f); tr.anchorMax = new Vector2(1f, 1f);
            tr.pivot = new Vector2(0.5f, 1f); tr.sizeDelta = new Vector2(0f, 44f);

            const float w = 340f - UiTheme.PadLarge * 2f;
            // Return to Login drops the session and shows the title again. It stays DISABLED when
            // nothing booted this scene (the arena's own fallback start, and the editor probes) —
            // there is no login to return to there, and a button that silently does nothing is
            // worse than one that says it cannot.
            var boot = Roi.Sim.SessionBoot.Instance;
            var login = MakeButton(panel, Loc.Get("UI_HUD_BAR_RETURN_TO_LOGIN"), "", w, UiTheme.ButtonH,
                                   boot != null ? () => boot.ReturnToLogin() : (System.Action)null);
            Place(login, new Vector2(0.5f, 1f), new Vector2(0f, -52f));
            login.interactable = boot != null;
            if (boot == null) login.GetComponentInChildren<TextMeshProUGUI>().color = UiTheme.TextLow;

            var quit = MakeButton(panel, Loc.Get("UI_COMMON_QUIT"), "", w, UiTheme.ButtonH, Quit);
            Place(quit, new Vector2(0.5f, 1f), new Vector2(0f, -100f));

            var cancel = MakeGhostButton(panel, Loc.Get("UI_COMMON_CANCEL"), w, UiTheme.ButtonH,
                                         () => _sysRoot.gameObject.SetActive(false), UiTheme.HeadSize);
            Place(cancel, new Vector2(0.5f, 1f), new Vector2(0f, -148f));

            _sysRoot.gameObject.SetActive(false);
        }

        static void Quit()
        {
#if UNITY_EDITOR
            UnityEditor.EditorApplication.isPlaying = false;
#else
            Application.Quit();
#endif
        }

        static void Place(Button b, Vector2 anchor, Vector2 offset)
        {
            var rt = b.GetComponent<RectTransform>();
            rt.anchorMin = rt.anchorMax = anchor;
            rt.pivot = new Vector2(0.5f, 1f);
            rt.anchoredPosition = offset;
        }

        // --- toggling --------------------------------------------------------------------------

        static Entry Find(string label)
        {
            foreach (var e in Entries) if (e.Label == label) return e;
            return new Entry(label, "", Key.None);
        }

        /// EXCLUSIVE: opening a window closes every other one, so pressing a second entry SWITCHES
        /// to it (the windows share one rect, so stacking would just hide the new one). Pressing
        /// the same entry again still closes it.
        public void Toggle(string label)
        {
            ItemTooltip.Hide();
            if (label == "Character")
            {
                foreach (var window in _windows.Values) window.SetOpen(false);
                Floating.Toggle(); SyncActiveStates(); return;
            }
            Floating?.Close();
            if (!_windows.TryGetValue(label, out var w)) return;
            bool opening = !w.IsOpen;
            foreach (var kv in _windows) if (kv.Value != w) kv.Value.SetOpen(false);
            w.SetOpen(opening);
            // WHICH SCREENS ARE ACTUALLY USED. The save cannot answer that — a screen opened and
            // backed out of leaves no trace in it — and the answer decides what gets UI polish
            // before launch. Only the handful the server's allow-list names are counted; a
            // free-form key here would be a counter nobody can aggregate.
            if (opening) Roi.Net.Telemetry.Note(TelemetryKey(label));
            SyncActiveStates();
        }

        // Keep sell/discard/deposit and other original bag actions reachable during the MVP.
        public void OpenInventoryTools()
        {
            if (Companion != null && Companion.Visible) Companion.Toggle();
            Floating.Close();
            foreach (var pair in _windows) pair.Value.SetOpen(pair.Key == "Inventory");
            SyncActiveStates();
        }

        /// The counter a screen reports itself as, or "" for the ones the allow-list does not
        /// carry. A TABLE, mirroring `telemetry_key` on the Godot views — the server rejects a
        /// name it does not know, so inventing one per screen would just fill `client.unknown`.
        static string TelemetryKey(string label) => label switch
        {
            "Shop" => "screen.shop",
            "Summon" => "screen.gacha",
            "Trade" => "screen.market",
            "Apothecary" => "screen.craft",
            "Options" => "screen.options",
            "Crystal" => "store.view",
            _ => "",
        };

        bool _homeMode;
        public void SetHomeMode(bool home)
        {
            CloseAll(); SetMenuOpen(false);
            _windowCanvas.sortingOrder = home ? 46 : 10;
            _popupCanvas.sortingOrder = home ? 47 : 20;
            _rosterCanvas.enabled = !home;
            _barCanvas.enabled = !home;
            _homeMode = home;
        }

        public void CloseAll()
        {
            ItemTooltip.Hide();
            UiPicker.Close();
            Floating?.Close();
            foreach (var kv in _windows) kv.Value.SetOpen(false);
            SyncActiveStates();
        }

        bool AnyWindowOpen()
        {
            foreach (var kv in _windows) if (kv.Value.IsOpen) return true;
            return false;
        }

        /// Mirror every window's visibility onto its bar and menu buttons. Event-driven in Godot
        /// (visibility_changed); called after each toggle here, which is the same thing without a
        /// signal — nothing else can open a window on this side yet.
        void SyncActiveStates()
        {
            bool excavation = _windows.TryGetValue("Summon", out var summon) && summon.IsOpen;
            if(_barRoot!=null)_barRoot.gameObject.SetActive(false);
            _rosterCanvas.enabled = !_homeMode && !excavation;
            _barCanvas.enabled = !_homeMode && !excavation;
            _popupCanvas.sortingOrder = excavation ? 60 : (_homeMode ? 47 : 20);
            foreach (var (btn, label) in _windowButtons)
                SetPressed(btn, _windows.TryGetValue(label, out var w) && w.IsOpen);
            // The combat roster is visible only when management pages are closed.
            bool inCombat = !AnyWindowOpen();
            if (_combat != null) _combat.SetVisible(inCombat);
            if (_barRoot != null) _barRoot.anchoredPosition = new Vector2(-BtnPad,
                inCombat ? -58f : -(UiTheme.TitleBarH - BtnH) * .5f);
            // The backdrop covers whatever chrome is actually down there, and it is a different
            // THING in each case. In combat it is a fade, so the fight runs to the bottom edge.
            // With a screen open it is the panel colour, because a solid window with a strip of
            // grass beneath it reads as a window that failed to reach the bottom — the screen
            // should be whole and the bar should float on IT, not on the world.
            if (_fade != null && _fadeImg != null)
            {
                _fade.sizeDelta = new Vector2(0f, inCombat ? 210f : UiLayout.BottomBar);
                _fadeImg.sprite = inCombat ? UiTheme.Fade(combat: true) : null;
                _fadeImg.color = inCombat ? Color.black : UiTheme.Panel;
            }
        }

        /// An open window's button is a slightly lighter surface with a stronger edge, not an
        /// accent fill. With thirty entries in the grid, an accent that lit up on selection would
        /// put the one warm colour in the UI wherever the pointer had last been — and a fill much
        /// lighter than its neighbours is the other failure, which this already made once.
        static void SetPressed(Button b, bool on)
        {
            var menu=b.GetComponent<MenuNavigationButton>();
            if(menu!=null){menu.SetActiveState(on);return;}
            var image=b.GetComponent<Image>();
            image.color = image.GetComponent<TacticalPrimary>()!=null
                ? (on ? new Color(.96f,.90f,.74f) : new Color(.88f,.83f,.70f))
                : (on ? UiTheme.SurfaceActive : UiTheme.Surface);
            var edge = b.transform.Find("Border");
            if (edge != null) edge.GetComponent<Image>().color = on ? UiTheme.LineStrong : UiTheme.Line;
        }

        // --- input -----------------------------------------------------------------------------

        void Update()
        {
            // Re-lay-out on a resize. Measured off the canvas for the same reason UiWindow is.
            var canvasSize = ((RectTransform)_windowCanvas.transform).rect.size;
            var now = new Vector2Int(Mathf.RoundToInt(canvasSize.x), Mathf.RoundToInt(canvasSize.y));
            if (now != _lastScreen)
            {
                _lastScreen = now;
                foreach (var kv in _windows) if (kv.Value.IsOpen) kv.Value.SetOpen(true);
                if (_menuPanel.gameObject.activeSelf) SetMenuOpen(true);
            }

            if(UiInputFocus.Typing||UiInputFocus.Modal)return;
            var k = Keyboard.current;
            if(k==null)return;
            if (Companion != null && Companion.Visible && (k.iKey.wasPressedThisFrame || k.escapeKey.wasPressedThisFrame)) return;

            if (k.escapeKey.wasPressedThisFrame)
            {
                if (Floating != null && Floating.Back()) return;
                var modes=_homeMode?Object.FindFirstObjectByType<ModeTabs>():null;
                if(modes!=null && modes.BackFromExemplars())return;
                var home=_homeMode?Object.FindFirstObjectByType<HomeMenuLayout>():null;
                if(home!=null && home.CloseGroup())return;
                // System menu open -> close it. Any window open -> close windows. Nothing open ->
                // open the system menu.
                if (_sysRoot.gameObject.activeSelf) _sysRoot.gameObject.SetActive(false);
                else if (AnyWindowOpen() || _menuPanel.gameObject.activeSelf) { SetMenuOpen(false); CloseAll(); }
                else _sysRoot.gameObject.SetActive(true);
                return;
            }

            if (_homeMode) return; // Keep Escape and window resizing; combat shortcuts belong to Combat.

            // Q is the DAMAGE METER, and it is NOT one of the thirty bar entries — `hud_bar.gd`
            // records that A-Z is full and that Q belongs to the meter. It toggles a corner HUD
            // rather than a window, so it does not go through `Toggle` and does not close what
            // is open.
            // W is the COMPANION widget, the other of the two letters `hud_bar.gd` keeps out of
            // the bar. Same shape as Q: a corner surface, not a window.
            if (k.wKey.wasPressedThisFrame)
            {
                Companion?.Toggle();
                return;
            }

            if (k.qKey.wasPressedThisFrame)
            {
                // HELD, not searched for: the meter starts hidden, and `FindFirstObjectByType`
                // skips inactive objects by default — so a search would find nothing exactly
                // while the key is meant to bring it back.
                Dps?.Toggle();
                return;
            }

            foreach (var e in Entries)
            {
                if (e.Key == Key.None) continue;
                if (k[e.Key].wasPressedThisFrame) { Toggle(e.Label); return; }
            }

            // A click outside the open menu dismisses it. Clicks on the menu or the Menu button
            // itself are the Buttons' own and must fall through.
            var m = Mouse.current;
            if (m != null && m.leftButton.wasPressedThisFrame && _menuPanel.gameObject.activeSelf)
            {
                var p = m.position.ReadValue();
                if (!Inside(_menuPanel, p) && !Inside(_menuBtn.GetComponent<RectTransform>(), p))
                    SetMenuOpen(false);
            }
        }

        static bool Inside(RectTransform rt, Vector2 screen)
            => RectTransformUtility.RectangleContainsScreenPoint(rt, screen, null);

        static Button MakeMenuNavigation(Transform parent,string label,string hotkey,float w,float h,System.Action click,bool tab=false)
        {
            var image=UiTheme.Flat(parent,"Menu_"+label,Color.white);
            image.rectTransform.sizeDelta=new Vector2(w,h);
            var button=image.gameObject.AddComponent<MenuNavigationButton>();
            button.targetGraphic=image;button.IsTab=tab;
            button.navigation=new Navigation{mode=Navigation.Mode.None};
            button.onClick.AddListener(()=>click?.Invoke());
            var text=UiTheme.Label(image.transform,"Label",label,tab?13:14,UiTheme.TextHigh,tab?TextAlignmentOptions.Center:TextAlignmentOptions.Left);
            text.enableAutoSizing=true;text.fontSizeMin=11;text.fontSizeMax=tab?13:14;
            var tr=text.rectTransform;tr.anchorMin=Vector2.zero;tr.anchorMax=Vector2.one;tr.offsetMin=new Vector2(tab?6:39,0);tr.offsetMax=new Vector2(tab?-6:-28,0);
            button.Caption=text;
            if(!string.IsNullOrEmpty(hotkey)) {
                var key=UiTheme.Label(image.transform,"Key",hotkey,10,UiTheme.TextLow,TextAlignmentOptions.Right);
                var kr=key.rectTransform;kr.anchorMin=Vector2.zero;kr.anchorMax=Vector2.one;kr.offsetMin=new Vector2(0,0);kr.offsetMax=new Vector2(-12,0);
            }
            var mark=UiTheme.Flat(image.transform,"ActiveMark",UiTheme.Accent);mark.raycastTarget=false;
            var mr=mark.rectTransform;mr.anchorMin=mr.anchorMax=tab?new Vector2(.5f,0):new Vector2(0,.5f);
            mr.sizeDelta=tab?new Vector2(w-24,2):new Vector2(2,22);mr.anchoredPosition=tab?new Vector2(0,0):new Vector2(2,0);
            button.AccentMark=mark;button.SetActiveState(false);
            return button;
        }

        // --- widgets ---------------------------------------------------------------------------

        /// A filled button. `hotkey` is set in the same line as the label but a step quieter — a
        /// shortcut is a hint, and giving it the label's weight makes every button read as two
        /// words of equal importance.
        internal static Button MakeButton(Transform parent, string label, string hotkey,
                                          float w, float h, System.Action onClick, bool primary=false)
        {
            var img = CodexButtonImage.Create(parent, "Btn_" + label, primary);
            var go = img.gameObject;
            var btn = go.AddComponent<UiStateButton>();
            btn.targetGraphic = img;
            // Never take keyboard focus: a focused button eats Space and Enter and fights the
            // global hotkeys.
            btn.navigation = new Navigation { mode = Navigation.Mode.None };
            var colors = UiTheme.ButtonColors;
            btn.colors = colors;
            if (onClick != null) btn.onClick.AddListener(() => onClick());
            var rt = img.rectTransform;
            rt.sizeDelta = new Vector2(w, h);
            UiButtonFeedback.Add(btn);

            bool keyed = !string.IsNullOrEmpty(hotkey);
            var t = UiTheme.Label(rt, "Label", label, UiTheme.HeadSize,
                                  primary?UiTheme.Accent:UiTheme.TextHigh, TextAlignmentOptions.Center);
            
            if(primary && h>=36) UiTheme.Display(t);
            t.enableAutoSizing=true;t.fontSizeMin=13;t.fontSizeMax=UiTheme.HeadSize;
            var tr = t.rectTransform;
            tr.anchorMin = Vector2.zero; tr.anchorMax = Vector2.one;
            tr.offsetMin = new Vector2(UiTheme.PadSmall, 0f);
            tr.offsetMax = new Vector2(-(keyed ? UiTheme.PadSmall + 16f : UiTheme.PadSmall), 0f);

            // The shortcut is its OWN element, pinned right and a step quieter. Appending it to
            // the label instead would centre the pair, so a column of buttons would have its
            // labels at thirty different x positions.
            if (keyed)
            {
                var kt = UiTheme.Label(rt, "Key", hotkey, UiTheme.CaptionSize,
                                       UiTheme.TextLow, TextAlignmentOptions.Right);
                var kr = kt.rectTransform;
                kr.anchorMin = Vector2.zero; kr.anchorMax = Vector2.one;
                kr.offsetMin = new Vector2(UiTheme.PadSmall, 0f);
                kr.offsetMax = new Vector2(-UiTheme.PadSmall, 0f);
            }
            return btn;
        }

        /// A button with no surface until it is pointed at — for a secondary action, where a filled
        /// rectangle would give it the same weight as the one the player is meant to take.
        internal static Button MakeGhostButton(Transform parent, string label, float w, float h,
                                               System.Action onClick, float size = UiTheme.HeadSize)
        {
            var img = UiTheme.Box(parent, "Ghost_" + label, Color.white);
            if(w>=70)UiTheme.Border(img.rectTransform,UiTheme.Line);
            var btn = img.gameObject.AddComponent<UiStateButton>();
            btn.targetGraphic = img;
            btn.navigation = new Navigation { mode = Navigation.Mode.None };
            // The state colours ARE the colours here: the image is white, so each one shows
            // through unmultiplied, and "at rest" is fully transparent.
            var colors = UiTheme.GhostColors;
            btn.colors = colors;
            // APPLY the resting colour now. A Selectable runs its first state transition in
            // OnEnable — which, for a button built into a window that is ALREADY on screen, has
            // already happened by the time these colours are assigned. The image then keeps the
            // default white tint until the pointer touches it: every "ghost" button in a screen
            // rebuilt while visible drew as a solid grey slab, which is exactly how the Options
            // screen came out.
            img.CrossFadeColor(colors.normalColor, 0f, true, true);
            if (onClick != null) btn.onClick.AddListener(() => onClick());
            var rt = img.rectTransform;
            rt.sizeDelta = new Vector2(w, h);

            var t = UiTheme.Label(rt, "Label", label, size, UiTheme.TextMid, TextAlignmentOptions.Center);
            t.enableAutoSizing=true;t.fontSizeMin=Mathf.Min(size,13);t.fontSizeMax=size;
            UiButtonFeedback.Add(btn);
            
            var tr = t.rectTransform;
            tr.anchorMin = Vector2.zero; tr.anchorMax = Vector2.one;
            tr.offsetMin = new Vector2(UiTheme.Gap, 0f); tr.offsetMax = new Vector2(-UiTheme.Gap, 0f);
            return btn;
        }

    }
}
