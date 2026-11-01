// Album collection: a two-pane codex, with an explicit bag/album tab and selected-card actions.
// All ownership, effects, costs and mutations come from the existing simulation queries/inputs.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class AlbumWindow : MonoBehaviour
    {
        const float Gap = 14f;
        Roi.Sim.SimDriver _driver;
        RectTransform _body, _grid, _gridViewport, _detail, _detailViewport;
        AlbumCodexSurface _detailBg, _albumTabSurface, _bagTabSurface;
        Button _albumTab, _bagTab;
        Scrollbar _gridScrollbar, _detailScrollbar;
        TextMeshProUGUI _albumHead, _bagHead, _context, _note;
        readonly List<GameObject> _built = new(), _detailBuilt = new();
        readonly Dictionary<long, string> _names = new();
        static readonly Dictionary<string, Sprite> Portraits = new();
        string _fingerprint = "";
        bool _bagMode, _layingOut, _layoutPending;
        int _sel = -1, _bagSel = -1;
        long _member;
        float _gridWidth, _cardW, _cardH, _detailWidth;
        float _albumScroll = 1, _bagScroll = 1;
        Vector2 _lastSize;
        ItemView[] _cards = new ItemView[0];
        BagCard[] _bagCards = new BagCard[0];

        public static AlbumWindow Attach(RectTransform body, string footnote)
        {
            var w = body.gameObject.AddComponent<AlbumWindow>();
            w.Build(body, footnote);
            return w;
        }
        void OnEnable() { PartySelection.Changed += OnPicked; _fingerprint = ""; }
        void OnDisable() => PartySelection.Changed -= OnPicked;
        void OnPicked() { _fingerprint = ""; if (isActiveAndEnabled) Refresh(); }
        void OnRectTransformDimensionsChange()
        {
            if (_body == null || _grid == null || _layingOut || !isActiveAndEnabled || _body.rect.size == _lastSize) return;
            _layoutPending = true;
        }
        void LateUpdate()
        {
            // Parent canvas resizing can notify children before their final rect is available.
            // Reflow cached records once the canvas settles; resizing must not refetch the sim.
            if (_body == null || _grid == null || _layingOut) return;
            if (!_layoutPending && _body.rect.size == _lastSize) return;
            _layoutPending = false;
            Rebuild();
        }

        void Build(RectTransform body, string footnote)
        {
            _body = body;
            CelestialCardMotif.Add(body);
            _albumTab = Action(body, "AlbumTab", "", false, () => SetBag(false), out _albumTabSurface);
            _bagTab = Action(body, "BagTab", "", false, () => SetBag(true), out _bagTabSurface);
            _albumHead = _albumTab.GetComponentInChildren<TextMeshProUGUI>();
            _bagHead = _bagTab.GetComponentInChildren<TextMeshProUGUI>();
            TabIcon(_albumTab, "Album"); TabIcon(_bagTab, "Inventory");
            _context = UiTheme.Label(body, "CollectionContext", "", 13, UiTheme.TextMid, TextAlignmentOptions.Left);
            _note = UiTheme.Label(body, "Note", footnote, 13, UiTheme.TextLow, TextAlignmentOptions.BottomLeft);
            _grid = ScrollColumn(body, "Collection", out _gridViewport);
            _gridScrollbar = ScrollbarFor(body, _gridViewport);
            _detailBg = AlbumCodexSurface.Create(body, "SelectedCardFrame", true);
            _detail = ScrollColumn(_detailBg.rectTransform, "Detail", out _detailViewport);
            _detailScrollbar = ScrollbarFor(_detailBg.transform, _detailViewport);
        }

        static RectTransform Rect(Transform parent, string name)
        {
            var go = new GameObject(name, typeof(RectTransform)); go.transform.SetParent(parent, false);
            return (RectTransform)go.transform;
        }
        static void Place(RectTransform rt, float x, float y, float w, float h)
        {
            rt.anchorMin = rt.anchorMax = new Vector2(0, 1); rt.pivot = new Vector2(0, 1);
            rt.sizeDelta = new Vector2(w, h); rt.anchoredPosition = new Vector2(x, -y);
        }
        static RectTransform ScrollColumn(RectTransform parent, string name, out RectTransform viewport)
        {
            viewport = Rect(parent, name + "Viewport");
            viewport.gameObject.AddComponent<Image>().color = Color.clear;
            viewport.gameObject.AddComponent<RectMask2D>();
            var content = Rect(viewport, name);
            content.anchorMin = new Vector2(0, 1); content.anchorMax = new Vector2(1, 1); content.pivot = new Vector2(.5f, 1);
            content.anchoredPosition = Vector2.zero; content.sizeDelta = Vector2.zero;
            var scroll = viewport.gameObject.AddComponent<ScrollRect>();
            scroll.viewport = viewport; scroll.content = content; scroll.horizontal = false;
            scroll.movementType = ScrollRect.MovementType.Clamped; scroll.scrollSensitivity = 36;
            return content;
        }
        static Button Action(Transform parent, string name, string text, bool primary, System.Action action, out AlbumCodexSurface frame)
        {
            frame = AlbumCodexSurface.Create(parent, name); frame.Primary = primary; frame.raycastTarget = true;
            var button = frame.gameObject.AddComponent<UiStateButton>(); button.targetGraphic = frame;
            button.colors = UiTheme.ButtonColors;
            button.onClick.AddListener(() => action());
            var label = UiTheme.Label(frame.transform, "Label", text, primary ? 21 : 14,
                primary ? UiTheme.Accent : UiTheme.TextHigh, TextAlignmentOptions.Center);
            if (primary) UiTheme.Display(label);
            label.rectTransform.anchorMin = Vector2.zero; label.rectTransform.anchorMax = Vector2.one;
            label.rectTransform.offsetMin = new Vector2(12, 4); label.rectTransform.offsetMax = new Vector2(-12, -4);
            label.textWrappingMode = TextWrappingModes.Normal;
            return button;
        }
        static Scrollbar ScrollbarFor(Transform parent, RectTransform viewport)
        {
            var track = UiTheme.Flat(parent, viewport.name + "Scroll", new Color(.5f,.55f,.57f,.14f));
            var thumb = UiTheme.Flat(track.transform, "Thumb", new Color(.78f,.71f,.51f,.8f));
            thumb.rectTransform.anchorMin=Vector2.zero;thumb.rectTransform.anchorMax=Vector2.one;
            thumb.rectTransform.offsetMin=thumb.rectTransform.offsetMax=Vector2.zero;
            var bar=track.gameObject.AddComponent<Scrollbar>();bar.handleRect=thumb.rectTransform;bar.targetGraphic=thumb;
            bar.direction=Scrollbar.Direction.BottomToTop;
            var scroll=viewport.GetComponent<ScrollRect>();scroll.verticalScrollbar=bar;
            scroll.verticalScrollbarVisibility=ScrollRect.ScrollbarVisibility.AutoHide;
            return bar;
        }
        static void TabIcon(Button button, string kind)
        {
            var icon = Rect(button.transform, "Icon").gameObject.AddComponent<HomeIcon>(); icon.Kind = kind; icon.color = UiTheme.Accent; icon.raycastTarget = false;
            Place(icon.rectTransform, 44, 5, 24, 24);
            var label = button.GetComponentInChildren<TextMeshProUGUI>();
            label.rectTransform.offsetMin = new Vector2(3, 2);label.rectTransform.offsetMax=new Vector2(-3,-30); label.fontSize = 13;label.enableAutoSizing=true;label.fontSizeMin=10;label.fontSizeMax=13;label.alignment=TextAlignmentOptions.Center;
        }
        void SetBag(bool bag)
        {
            if (_bagMode == bag) return;
            var scroll = _gridViewport.GetComponent<ScrollRect>();
            if (_bagMode) _bagScroll = scroll.verticalNormalizedPosition; else _albumScroll = scroll.verticalNormalizedPosition;
            _bagMode = bag; _fingerprint = ""; Refresh();
            scroll.StopMovement(); scroll.verticalNormalizedPosition = bag ? _bagScroll : _albumScroll;
            UiPageTransition.Play(_gridViewport); UiPageTransition.Play(_detailViewport);
        }

        public void Refresh()
        {
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var session = _driver != null ? _driver.Session : null;
            if (session == null) return;
            _member = ResolveMember(session);
            var album = JsonUtility.FromJson<ItemList>("{\"items\":" + session.QueryJson("card_album") + "}");
            _cards = album?.items ?? new ItemView[0]; _bagCards = CardsInBag(session);
            _sel = _cards.Length == 0 ? -1 : Mathf.Clamp(_sel, 0, _cards.Length - 1);
            _bagSel = _bagCards.Length == 0 ? -1 : Mathf.Clamp(_bagSel, 0, _bagCards.Length - 1);
            var fp = new System.Text.StringBuilder().Append(_member).Append('|').Append(_bagMode).Append('|').Append(_sel).Append('|').Append(_bagSel).Append('|').Append(Loc.Language);
            foreach (var c in _cards) fp.Append('|').Append(Key(c)).Append(':').Append(c.quantity);
            foreach (var b in _bagCards) fp.Append('|').Append(b.idx).Append(':').Append(Key(b.item)).Append(':').Append(b.item.quantity);
            if (_sel >= 0) fp.Append('|').Append(session.QueryJson("bind_info:" + _sel));
            if (fp.ToString() == _fingerprint && _body.rect.size == _lastSize) return;
            _fingerprint = fp.ToString(); Rebuild();
        }
        static string Key(ItemView c) => c.def_id + ":" + c.star + ":" + c.foil;
        BagCard[] CardsInBag(Roi.Sim.SimSession session)
        {
            var inv = JsonUtility.FromJson<ItemList>("{\"items\":" + session.QueryJson("inventory") + "}");
            var items = inv?.items ?? new ItemView[0]; var result = new List<BagCard>();
            for (int i = 0; i < items.Length; i++) if (items[i].kind == "Card") result.Add(new BagCard { idx = i, item = items[i] });
            return result.ToArray();
        }
        long ResolveMember(Roi.Sim.SimSession session)
        {
            var v = JsonUtility.FromJson<LeadView>(session.ViewJson()); var ids = new List<long>(); _names.Clear();
            if (v?.grid?.entities != null) foreach (var e in v.grid.entities)
                if (e.team == "Player" && !e.is_pet) { ids.Add(e.id); _names[e.id] = GameText.UnitName(e.unit_def); }
            return PartySelection.Resolve(ids);
        }

        void LayoutColumns()
        {
            _lastSize = _body.rect.size;
            float pad = 24, top = 20, bottom = 42, avail = _body.rect.width - pad * 2;
            float railW=0;
            _albumTab.gameObject.SetActive(false);_bagTab.gameObject.SetActive(false);
            float detailW = Mathf.Clamp(avail * .32f, 260, 364);
            _gridWidth = avail - detailW - 24 - railW;
            float tabW = Mathf.Min(228, (_gridWidth - 8) * .5f);
            Place(_albumTab.GetComponent<RectTransform>(), pad, top, 112, 64);
            Place(_bagTab.GetComponent<RectTransform>(), pad, top+76, 112, 64);
            Place(_context.rectTransform, pad + railW, top, _gridWidth, 25);
            Place(_gridViewport, pad+railW, top + 38, _gridWidth, Mathf.Max(150, _body.rect.height - top - 38 - bottom));
            Place((RectTransform)_gridScrollbar.transform,pad+railW+_gridWidth+6,top+40,3,_gridViewport.rect.height-4);
            Place(_detailBg.rectTransform, pad + railW+_gridWidth + 24, top, detailW, _body.rect.height - top - 14);
            _detailWidth = detailW - 36;
            Place(_detailViewport, 18, 10, _detailWidth, _detailBg.rectTransform.rect.height - 24);
            Place((RectTransform)_detailScrollbar.transform,detailW-9,18,3,_detailBg.rectTransform.rect.height-36);
            Place(_note.rectTransform, pad, _body.rect.height - 34, _gridWidth, 30);
            _note.textWrappingMode = TextWrappingModes.Normal;
            int cols = Mathf.Max(1,Mathf.FloorToInt((_gridWidth+Gap)/174f));
            _cardW = (_gridWidth - Gap * (cols - 1)) / cols;
            _cardH = _cardW*1.25f;
        }
        static void Clear(List<GameObject> objects)
        { foreach (var go in objects) { go.SetActive(false); Destroy(go); } objects.Clear(); }
        void Rebuild()
        {
            _layingOut = true;
            try
            {
                var scroll = _gridViewport.GetComponent<ScrollRect>(); float offset = _built.Count != _cards.Length + _bagCards.Length ? 0 : Mathf.Max(0, _grid.anchoredPosition.y);
                Clear(_built); LayoutColumns();
                _albumHead.text = Loc.Fmt("UI_ALBUM_TAB_COUNT", Loc.Get("UI_ALBUM_ALBUM"), _cards.Length);
                _bagHead.text = Loc.Fmt("UI_ALBUM_TAB_COUNT", Loc.Get("UI_ALBUM_IN_THE_BAG"), _bagCards.Length);
                _albumTabSurface.Selected = !_bagMode; _bagTabSurface.Selected = _bagMode;
                _albumTabSurface.SetVerticesDirty(); _bagTabSurface.SetVerticesDirty();
                _albumHead.color = !_bagMode ? UiTheme.Accent : UiTheme.TextMid;
                _bagHead.color = _bagMode ? UiTheme.Accent : UiTheme.TextMid;
                _context.text = Loc.Get("UI_ALBUM_GRID_GUIDE");
                _note.text = Loc.Get("UI_ALBUM_HINT");
                int count = _cards.Length + _bagCards.Length;
                int cols = Mathf.Max(1,Mathf.FloorToInt((_gridWidth+Gap)/174f));
                for (int i = 0; i < count; i++)
                {
                    bool bag=i>=_cards.Length; int idx=bag?i-_cards.Length:i;
                    var card=bag?_bagCards[idx].item:_cards[idx];
                    _built.Add(CardCell(card, i % cols * (_cardW + Gap), i / cols * (_cardH + Gap),
                        bag==_bagMode && idx == (bag ? _bagSel : _sel), () => { _bagMode=bag; Select(idx); }));
                }
                if (count == 0) EmptyCollection();
                float height = Mathf.Ceil(count / (float)cols) * (_cardH + Gap) - Gap;
                _grid.sizeDelta = new Vector2(0, Mathf.Max(_gridViewport.rect.height, height));
                scroll.StopMovement(); _grid.anchoredPosition = new Vector2(0, Mathf.Clamp(offset, 0, Mathf.Max(0, _grid.sizeDelta.y - _gridViewport.rect.height)));
                BuildDetail();
            }
            finally { _layingOut = false; }
        }

        GameObject CardCell(ItemView card, float x, float y, bool selected, System.Action pick)
        {
            var frame = AlbumCodexSurface.Create(_grid, "Card_" + card.def_id, true);
            frame.Selected = selected; frame.Insignia = true; frame.raycastTarget = true;
            Place(frame.rectTransform, x, y, _cardW, _cardH);
            var btn = frame.gameObject.AddComponent<UiStateButton>(); btn.targetGraphic = frame; btn.onClick.AddListener(() => pick());
            btn.colors = UiTheme.ButtonColors;
            var art = Portrait(frame.transform, card, false, new Rect((_cardW - 120) / 2, 23, 120, _cardH - 92));
            var name = Text(frame.transform, "Name", GameText.ItemName(card), 16, UiTheme.TextHigh, 10, _cardH - 71, _cardW - 20, 26);
            UiTheme.Display(name); name.enableAutoSizing = true; name.fontSizeMin = 12; name.fontSizeMax = 16;
            var chip = AlbumCodexSurface.Create(frame.transform, "Quantity"); Place(chip.rectTransform, _cardW - 62, 8, 52, 25);
            Text(chip.transform, "Count", "×" + card.quantity, 14, UiTheme.TextHigh, 2, 0, 48, 25, TextAlignmentOptions.Center);
            var rare = Rarity(card);
            var rule = UiTheme.Flat(frame.transform, "Rarity", rare); rule.raycastTarget = false;
            Place(rule.rectTransform, 20, _cardH - 31, _cardW - 40, 2);
            var meta = Loc.GetOr("RARITY_" + (card.rarity ?? "Common").ToUpperInvariant(), card.rarity ?? "");
            if (card.star > 0) meta += "  ·  " + Loc.Fmt("UI_COMMON_LEVEL_SHORT", card.star);
            if (card.foil) meta += "  ·  " + Loc.Get("UI_ALBUM_FOIL_LABEL");
            Text(frame.transform, "RarityLabel", meta, 11, UiTheme.TextMid, 20, _cardH - 27, _cardW - 40, 22, TextAlignmentOptions.Center);
            return frame.gameObject;
        }
        static Color Rarity(ItemView c)
        {
            if (c.foil) return UiTheme.Accent;
            return c.rarity switch { "Mythical" => new Color(.69f,.44f,.89f), "Legendary" => new Color(.92f,.68f,.3f), "Rare" => new Color(.38f,.66f,.9f), "Uncommon" => new Color(.43f,.72f,.52f), "Immortal" => new Color(.94f,.42f,.73f), _ => UiTheme.Line };
        }
        static Image Portrait(Transform parent, ItemView card, bool animated, Rect area)
        {
            var frames = CardArt.Idle(card); if (frames == null || frames.Length == 0) return null;
            var image = Rect(parent, "Monster").gameObject.AddComponent<Image>(); image.raycastTarget = false; image.preserveAspect = true;
            Place(image.rectTransform, area.x, area.y, area.width, area.height);
            if (animated) image.gameObject.AddComponent<MonsterPortraitIdle>().Initialize(image, frames, Mathf.RoundToInt(CardArt.IdleFps));
            else
            {
                if (!Portraits.TryGetValue(card.def_id, out var sprite))
                {
                    var source = frames[0]; var bounds = source.rect; sprite = source;
                    if (source.texture.isReadable)
                    {
                        var pixels = source.texture.GetPixels((int)bounds.x, (int)bounds.y, (int)bounds.width, (int)bounds.height);
                        int xmin = (int)bounds.width, ymin = (int)bounds.height, xmax = -1, ymax = -1;
                        for (int y = 0; y < (int)bounds.height; y++) for (int x = 0; x < (int)bounds.width; x++)
                            if (pixels[y * (int)bounds.width + x].a > .08f) { xmin = Mathf.Min(xmin,x); ymin = Mathf.Min(ymin,y); xmax = Mathf.Max(xmax,x); ymax = Mathf.Max(ymax,y); }
                        if (xmax >= xmin) sprite = Sprite.Create(source.texture, new Rect(bounds.x+xmin,bounds.y+ymin,xmax-xmin+1,ymax-ymin+1), new Vector2(.5f,.5f),100,0,SpriteMeshType.FullRect);
                    }
                    Portraits[card.def_id] = sprite;
                }
                image.sprite = sprite;
            }
            return image;
        }
        static TextMeshProUGUI Text(Transform parent, string name, string value, float size, Color color, float x, float y, float w, float h, TextAlignmentOptions align = TextAlignmentOptions.Left)
        {
            var label = UiTheme.Label(parent, name, value, size, color, align);
            Place(label.rectTransform, x, y, w, h); label.textWrappingMode = TextWrappingModes.Normal;
            label.overflowMode = TextOverflowModes.Overflow;
            return label;
        }
        void EmptyCollection()
        {
            var frame = AlbumCodexSurface.Create(_grid, "EmptyCollection", true); Place(frame.rectTransform, 0, 0, _gridWidth, 194);
            Text(frame.transform, "Empty", Loc.Get(_bagMode ? "UI_ALBUM_EMPTY_BAG" : "UI_ALBUM_EMPTY"), 20, UiTheme.TextHigh, 24, 35, _gridWidth-48, 46, TextAlignmentOptions.Center);
            Text(frame.transform, "Hint", Loc.Get(_bagMode ? "UI_ALBUM_FIND_CARDS" : "UI_ALBUM_EMPTY_GUIDE"), 14, UiTheme.TextMid, 24, 84, _gridWidth-48, 44, TextAlignmentOptions.Center);
            _built.Add(frame.gameObject);
        }

        void BuildDetail()
        {
            Clear(_detailBuilt);
            var root = Rect(_detail, "SelectedCard"); _detailBuilt.Add(root.gameObject);
            int idx = _bagMode ? _bagSel : _sel;
            if (idx < 0)
            {
                Place(root,0,0,_detailWidth,200);
                Text(root,"Hint",Loc.Get("UI_ALBUM_SELECT_CARD"),16,UiTheme.TextMid,10,36,_detailWidth-20,100,TextAlignmentOptions.Center);
                _detail.sizeDelta = new Vector2(0,_detailViewport.rect.height); return;
            }
            var card = _bagMode ? _bagCards[idx].item : _cards[idx];
            var ribbon=AlbumCodexSurface.Create(root,"SelectedRibbon");ribbon.Ribbon=true;Place(ribbon.rectTransform,0,0,90,18);
            Text(root,"Selected",Loc.Get("UI_ALBUM_SELECTED"),10,UiTheme.Panel,6,0,70,18);
            var name = Text(root,"Name",GameText.ItemName(card),24,UiTheme.Accent,0,23,_detailWidth,35);
            UiTheme.Display(name); name.enableAutoSizing=true; name.fontSizeMin=18; name.fontSizeMax=24;
            string meta = Loc.Fmt("UI_ALBUM_HELD",card.quantity);
            if(card.star>0)meta += "  ·  "+Loc.Fmt("UI_COMMON_LEVEL_SHORT",card.star);
            if(card.foil)meta += "  ·  "+Loc.Get("UI_ALBUM_FOIL_LABEL");
            Text(root,"Stock",meta,12,UiTheme.TextMid,0,57,_detailWidth,24,TextAlignmentOptions.Right);
            Portrait(root,card,true,new Rect((_detailWidth-116)/2,73,116,68));
            var rule = UiTheme.Flat(root,"Divider",UiTheme.Line);rule.raycastTarget=false;Place(rule.rectTransform,0,151,_detailWidth,1);
            float y=158;
            var lines = card.foil && card.card_foil_lines != null && card.card_foil_lines.Length>0 ? card.card_foil_lines : card.card_lines;
            if(lines!=null)foreach(var line in lines){var label=Text(root,"Effect",line,17,UiTheme.TextHigh,0,y,_detailWidth,26);float h=Mathf.Max(26,label.GetPreferredValues(line,_detailWidth,0).y);label.rectTransform.sizeDelta=new Vector2(_detailWidth,h);y+=h;}
            y+=8;
            if(_bagMode)
            {
                Text(root,"DepositHint",Loc.Get("UI_ALBUM_DEPOSIT_HINT"),14,UiTheme.TextMid,0,y,_detailWidth,52);y+=62;
                int invIdx=_bagCards[idx].idx;
                var deposit=Action(root,"Deposit",Loc.Get("UI_ALBUM_DEPOSIT"),true,()=>Send("{\"DepositCardToAlbum\":{\"inv_idx\":"+invIdx+"}}"),out _);
                Place(deposit.GetComponent<RectTransform>(),0,y,_detailWidth,44);y+=52;
            }
            else
            {
                var bind=ReadBind(_sel);
                var bonus=AlbumCodexSurface.Create(root,"BindBonus");Place(bonus.rectTransform,0,y,_detailWidth,100);
                Text(bonus.transform,"BindTitle",Loc.Get("UI_ALBUM_BIND_TITLE"),13,UiTheme.Accent,12,6,_detailWidth-24,26);
                Text(bonus.transform,"BindScope",Loc.Get("UI_ALBUM_BIND_SCOPE"),11,UiTheme.TextMid,12,31,_detailWidth-24,22);
                bool hasBind=bind!=null&&!string.IsNullOrEmpty(bind.effect);
                if(hasBind)
                {
                    Text(bonus.transform,"BindEffect",bind.effect,15,UiTheme.TextHigh,12,52,_detailWidth-24,24);
                    Text(bonus.transform,"BindProgress",bind.maxed ? Loc.Fmt("UI_ALBUM_BOUND_MAX",bind.done,bind.cap) : Loc.Fmt("UI_ALBUM_BOUND_PROGRESS",bind.done,bind.cap,CardCount(bind.cost)),11,UiTheme.TextMid,12,76,_detailWidth-24,22);
                }
                else Text(bonus.transform,"NoBind",Loc.Get(card.foil?"UI_ALBUM_FOIL_NO_BIND":"UI_ALBUM_NO_BIND"),12,UiTheme.TextMid,12,48,_detailWidth-24,34);
                y+=110;
                int selNow=_sel;
                var withdraw=Action(root,"Withdraw",Loc.Get("UI_ALBUM_WITHDRAW_TO_THE_BAG"),false,()=>Send("{\"WithdrawCardFromAlbum\":{\"album_idx\":"+selNow+"}}"),out _);
                Place(withdraw.GetComponent<RectTransform>(),0,y,_detailWidth,44);y+=52;
                var feeders=FeedCandidates(_sel);
                var level=Action(root,"LevelUp",Loc.Get("UI_ALBUM_LEVEL_UP"),true,()=>OpenFeedPicker(selNow),out _);
                level.interactable=feeders.Count>0;Place(level.GetComponent<RectTransform>(),0,y,_detailWidth,44);y+=52;
                if(!level.interactable)level.GetComponentInChildren<TextMeshProUGUI>().color=UiTheme.TextLow;
                if(feeders.Count==0){Text(root,"LevelReason",Loc.Get(card.star>=CardStarMax?"UI_ALBUM_LEVEL_MAX":"UI_ALBUM_NO_DUPLICATE"),12,UiTheme.TextMid,0,y-1,_detailWidth,19);y+=23;}
                string bindText=!hasBind?Loc.Get("UI_ALBUM_BIND"):bind.maxed?Loc.Get("UI_ALBUM_BIND_DONE"):Loc.Fmt(card.quantity<bind.cost?"UI_ALBUM_BIND_NEEDS":"UI_ALBUM_BIND_SPENDS",CardCount(bind.cost));
                var bindButton=Action(root,"Bind",bindText,false,()=>AskBind(selNow,bind),out _);
                bindButton.interactable=hasBind&&!bind.maxed&&!card.foil&&card.quantity>=bind.cost;
                if(!bindButton.interactable)bindButton.GetComponentInChildren<TextMeshProUGUI>().color=UiTheme.TextLow;
                Place(bindButton.GetComponent<RectTransform>(),0,y,_detailWidth,44);y+=52;
            }
            Place(root,0,0,_detailWidth,y);
            _detail.sizeDelta=new Vector2(0,Mathf.Max(y,_detailViewport.rect.height));
            _detailViewport.GetComponent<ScrollRect>().verticalNormalizedPosition=1;
        }
        void Select(int idx)
        {
            if(_bagMode)_bagSel=idx;else _sel=idx;
            _fingerprint="";Refresh();UiPageTransition.Play(_detailViewport);
        }
        static string CardCount(int count) => Loc.Fmt(count==1?"UI_ALBUM_ONE_CARD":"UI_ALBUM_MANY_CARDS",count);

        // --- ★ Up ---------------------------------------------------------------------------

        /// Mirrors `handlers::feed_card`'s acceptance rule: same def, a different row or a stack of
        /// at least two, and something to gain — a star while under the cap, or a foil the target
        /// does not have. A candidate that would change nothing is refused by the sim silently, so
        /// offering it would be offering a dead press.
        List<int> FeedCandidates(int target)
        {
            var outp = new List<int>();
            if (target < 0 || target >= _cards.Length) return outp;
            var t = _cards[target];
            for (int j = 0; j < _cards.Length; j++)
            {
                var c = _cards[j];
                if (c.def_id != t.def_id) continue;
                if (j == target && c.quantity < 2) continue;
                int gain = Mathf.Min(1 + c.star, CardStarMax - t.star);
                bool foilGain = c.foil && !t.foil;
                if (gain > 0 || foilGain) outp.Add(j);
            }
            return outp;
        }

        const int CardStarMax = 5;   // items::CARD_STAR_MAX

        void OpenFeedPicker(int target)
        {
            var feeders = FeedCandidates(target);
            if (feeders.Count == 0) return;
            // ONE candidate is the common case by a long way — duplicates stack, so the usual
            // choice is the card feeding itself. Skip straight to the confirmation rather than
            // making the player pick from a list of one.
            if (feeders.Count == 1) { AskFeed(target, feeders[0]); return; }

            var rows = new List<UiPicker.Option>();
            foreach (var j in feeders)
            {
                int feeder = j;
                rows.Add(new UiPicker.Option
                {
                    label = FeederLabel(target, feeder),
                    detail = PreviewText(target, feeder),
                    onPick = () => AskFeed(target, feeder),
                });
            }
            UiPicker.Open(Loc.Get("UI_ALBUM_FEED_PICK"), rows);
        }

        string FeederLabel(int target, int feeder)
        {
            var c = _cards[feeder];
            var what = feeder == target ? Loc.Get("UI_ALBUM_ANOTHER_COPY") : GameText.ItemName(c);
            if (c.star > 0) what += "  " + Loc.Fmt("UI_COMMON_LEVEL_SHORT", c.star);
            if (c.foil) what += "  " + Loc.Get("UI_ALBUM_FOIL_LABEL");
            return what;
        }

        /// What the feed would produce, computed the same way the sim will — including the WASTE,
        /// which is the whole reason this is a confirmation and not a button. Feeding a 3★ into a
        /// 4★ throws away everything above the cap, and a foil into a foil throws away the foil.
        string PreviewText(int target, int feeder)
        {
            var t = _cards[target];
            var f = _cards[feeder];
            int uncapped = t.star + 1 + f.star;
            int newStar = Mathf.Min(CardStarMax, uncapped);
            int waste = Mathf.Max(0, uncapped - CardStarMax);
            var s = Loc.Fmt("UI_ALBUM_LEVEL_PREVIEW", t.star, newStar);
            if (f.foil && !t.foil) s += Loc.Get("UI_ALBUM_BECOMES_FOIL");
            if (waste > 0) s += Loc.Fmt("UI_ALBUM_LEVEL_WASTE", waste);
            else if (f.foil && t.foil) s += Loc.Get("UI_ALBUM_FOIL_WASTE");
            return s;
        }

        void AskFeed(int target, int feeder)
        {
            UiConfirm.Ask(Loc.Get("UI_ALBUM_LEVEL_UP"), PreviewText(target, feeder) + "\n\n" + Loc.Get("UI_ALBUM_FEED_WARNING"), Loc.Get("UI_ALBUM_FEED"),
                          () => Send($"{{\"FeedCard\":{{\"target_idx\":{target},\"feeder_idx\":{feeder}}}}}", true));
        }

        void AskBind(int idx, BindInfo bind)
        {
            if (bind == null) return;
            long who = _member;
            UiConfirm.Ask(Loc.Get("UI_ALBUM_BIND"), bind.effect + "\n\n" + Loc.Fmt("UI_ALBUM_BIND_WARNING", CardCount(bind.cost)), Loc.Get("UI_ALBUM_BIND"),
                          () => Send($"{{\"BindCard\":{{\"entity_id\":{who},\"album_idx\":{idx}}}}}"));
        }

        BindInfo ReadBind(int idx)
        {
            var session = _driver != null ? _driver.Session : null;
            if (session == null) return null;
            var json = session.QueryJson("bind_info:" + idx);
            if (string.IsNullOrEmpty(json) || json == "null") return null;
            return JsonUtility.FromJson<BindInfo>(json);
        }


        void Send(string eventJson, bool selectUpgraded = false)
        {
            var session = _driver != null ? _driver.Session : null;
            if (session == null) return;
            var selected = _sel >= 0 && _sel < _cards.Length ? _cards[_sel] : null;
            string bag = _bagSel >= 0 && _bagSel < _bagCards.Length ? Key(_bagCards[_bagSel].item) : null;
            session.ApplyInput(eventJson);
            // Re-query indices after every mutation; never retain a callback to a shifted row.
            var next = JsonUtility.FromJson<ItemList>("{\"items\":" + session.QueryJson("card_album") + "}")?.items ?? new ItemView[0];
            if (selected != null)
            {
                int exact=-1, same=-1, upgraded=-1;
                for(int i=0;i<next.Length;i++) if(next[i].def_id==selected.def_id)
                {
                    if(same<0)same=i;
                    if(Key(next[i])==Key(selected))exact=i;
                    // Select the row the sim actually added to, including a merge into an
                    // existing upgraded stack. A different higher-level copy is not the result.
                    int prior=0;foreach(var c in _cards)if(Key(c)==Key(next[i]))prior+=c.quantity;
                    if(next[i].quantity>prior)upgraded=i;
                }
                _sel=selectUpgraded&&upgraded>=0?upgraded:exact>=0?exact:same>=0?same:Mathf.Clamp(_sel,0,next.Length-1);
            }
            var nextBag=CardsInBag(session);
            if (bag != null) for (int i=0;i<nextBag.Length;i++) if(Key(nextBag[i].item)==bag){_bagSel=i;break;}
            _fingerprint="";Refresh();
        }
        [System.Serializable] class BindInfo
        { public string effect; public int done, cost, cap; public bool maxed, foil; public int stock; }
        struct BagCard { public int idx; public ItemView item; }
        [System.Serializable] class LeadView { public LeadGrid grid; }
        [System.Serializable] class LeadGrid { public LeadEntity[] entities; }
        [System.Serializable] class LeadEntity { public long id; public string team, unit_def; public bool is_pet; }
    }
}

