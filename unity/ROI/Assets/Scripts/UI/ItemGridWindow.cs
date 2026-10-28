// A GRID OF ITEMS — the bag, the warehouse, the album. One screen, because they differ only in
// which list they ask the sim for and which cap they count against; the bag half of
// character_view_bag.gd, the stash view and the album view all draw the same thing in Godot too,
// and porting them as three files would be three places to fix the next layout decision.
//
// FETCHED, NOT STREAMED. `view_state` deliberately sends these lists empty and keeps only their
// counts — at ten ticks a second the bag alone stuttered — so the grid asks the sim for it when
// the window opens, through `SimSession.QueryJson`. That is the whole reason the query entry
// point exists.
//
// REBUILT ONLY WHEN THE SET CHANGES. The Godot original fingerprints the items and rebuilds on a
// change, never per tick, with the reason stated outright: churn froze buttons mid-press once.
// The same fingerprint is here.
//
// Not this slice, and each window says as much at its foot: the paper-doll, the stats column, the
// skill bar, the rare-loot strip, sell/discard corners, and the hover tooltip.
using System.Text;
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    public sealed class ItemGridWindow : MonoBehaviour
    {
        const float Gap = 12f;          // INV_PAD, opened up: the grid is the window's whole content
        const float HeadH = 26f, FootH = 22f;

        Roi.Sim.SimDriver _driver;
        TextMeshProUGUI _count, _note, _empty;
        RectTransform _grid, _gridHost, _emptyPanel;
        float _gridWidth;
        int _category; bool _reverse;
        ShopSelectionArc _categoryArc;
        TextMeshProUGUI _detailText,_detailName,_detailGrade;
        UnityEngine.UI.Image _detailRarity;
        ExemplarEquipmentIcon _detailIcon;
        readonly System.Collections.Generic.List<MenuNavigationButton> _categoryButtons=new();
        readonly System.Collections.Generic.List<ItemSlot> _slots = new();
        string _fingerprint = "";
        int _cap = 20;
        /// The bag is the only one of these lists with actions on it. The warehouse and the album
        /// are read-only here for now, and a Sell button on the album would be a way to lose a card.
        bool _canAct;
        /// The stash acts too, with one verb of its own. Kept apart from `_canAct` because the two
        /// lists offer opposite moves: the bag sends things away, the warehouse brings them back.
        bool _isStash;
        /// "Regalia" for the one CARRIER list, empty otherwise. Its verb is a
        /// third shape again: a carrier is never moved, only MARKED as worn by one member, so the
        /// action needs a target as well as a selection — and the target is the party tabs'
        /// choice, the same one the Cards panel obeys.
        string _carrier = "";
        long _member;
        UnityEngine.UI.Button _equip, _unequip, _emptyAction;
        int _sel = -1;
        UnityEngine.UI.Button _discard, _disenchant;
        CrestModel.Choice _disenchantChoice;
        string _selectedRaw;
        System.Collections.Generic.List<string> _renderedRows;
        ItemView[] _items = new ItemView[0];

        string _queryKey, _capKey, _title, _footnote;

        /// `capKey` empty = an UNCAPPED list (the card album has no cap since 0.0.217), which is
        /// shown as a plain count rather than as a fraction against a cap that does not exist.
        public static ItemGridWindow Attach(RectTransform body, string title,
                                            string queryKey, string capKey, string footnote)
        {
            var w = body.gameObject.AddComponent<ItemGridWindow>();
            w._title = title; w._queryKey = queryKey; w._capKey = capKey; w._footnote = footnote;
            w._canAct = queryKey == "inventory";
            w._isStash = queryKey == "stash";
            // Only Regalia is a carrier now: an Exemplar is a unit (0.0.350) with its own screen.
            w._carrier = queryKey == "owned_regalia" ? "Regalia" : "";
            w.Build(body);
            return w;
        }

        void Build(RectTransform body)
        {
            float pad = UiTheme.PadLarge;

            // The window's title bar already names the screen. Repeating it here would be the
            // second-largest text on the panel saying nothing new, so the head carries only the
            // one thing the player came to check: how full it is.
            _count = UiTheme.Label(body, "Count", "", UiTheme.HeadSize,
                                   UiTheme.TextHigh, TextAlignmentOptions.Right);
            var hr = _count.rectTransform;
            hr.anchorMin = new Vector2(0f, 1f); hr.anchorMax = new Vector2(1f, 1f);
            hr.pivot = new Vector2(0.5f, 1f);
            hr.offsetMin = new Vector2(pad, -HeadH - UiTheme.PadSmall);
            hr.offsetMax = new Vector2(_canAct?-200:-pad, -UiTheme.PadSmall);

            float actionRoom = _canAct || _isStash || _carrier != "" ? UiLayout.Touch + UiTheme.Gap : 0f;
            // The rare strip sits ABOVE the grid, in the bag only: it is the net that catches what
            // the bag could not hold, so it belongs beside the bag and nowhere else.
            const float rareRoom = 0f; // the rare net is retired (0.0.380)
            var gridGo = new GameObject("Grid", typeof(RectTransform));
            gridGo.transform.SetParent(body, false);
            _grid = (RectTransform)gridGo.transform;
            _grid.anchorMin = Vector2.zero; _grid.anchorMax = Vector2.one;
            _grid.offsetMin = new Vector2(pad, FootH + UiTheme.Pad + actionRoom);
            _grid.offsetMax = new Vector2(-pad, -(HeadH + UiTheme.Pad + UiTheme.PadSmall + rareRoom));

            _gridHost = _grid;
            _grid = UiScroll.Create(_gridHost, out var scroll);
            _gridHost.anchorMin=new Vector2(_isStash?.09f:.13f,0);_gridHost.anchorMax=new Vector2(.65f,1);
            var rail=ExemplarUi.Rect(body,"Categories",.015f,.20f,.105f,.90f);
            string[] labels={"UI_COLLECTION_ALL","UI_COLLECTION_WEAPONS","UI_COLLECTION_ARMOR","UI_COLLECTION_CARDS","UI_COLLECTION_MATERIALS"};
            string[] icons={"Inventory","Equipment","Regalia","Cards","Stash"};
            var seam=ExemplarUi.Rect(body,"CategorySeam",.115f,.20f,.115f,.90f);seam.offsetMin=new Vector2(-47,0);seam.offsetMax=new Vector2(17,0);
            _categoryArc=seam.gameObject.AddComponent<ShopSelectionArc>();_categoryArc.raycastTarget=false;
            for(int i=0;i<labels.Length;i++){
                int index=i;float top=1-i*.18f;
                var b=CollectionBrowser.Quiet(rail,"Category_"+i,Loc.Get(labels[i]),0,top-.17f,1,top,()=>{_category=index;LayoutGrid(_items.Length);});
                ExemplarUi.Place(b.Caption.rectTransform,0,0,1,.35f);b.Caption.enableAutoSizing=true;b.Caption.fontSizeMin=11;b.Caption.fontSizeMax=14;
                ExemplarUi.Icon(b.transform,icons[i],.31f,.44f,.69f,.94f,UiTheme.TextMid);b.CategoryIcon=b.GetComponentInChildren<HomeIcon>();if(_isStash)b.Caption.gameObject.SetActive(false);_categoryButtons.Add(b);
            }
            ExemplarUi.Panel(body,"InspectorDivider",.665f,.14f,.666f,.94f,UiTheme.LineStrong);
            _detailName=ExemplarUi.Text(body,"ItemName","",.69f,.84f,.97f,.94f,UiTheme.TitleSize,UiTheme.Accent);
            _detailName.enableAutoSizing=true;_detailName.fontSizeMin=16;_detailName.fontSizeMax=UiTheme.TitleSize;
            _detailGrade=ExemplarUi.Text(body,"SelectedGrade","",.69f,.69f,.81f,.82f,UiTheme.BodySize,UiTheme.TextMid);
            _detailGrade.textWrappingMode=TextWrappingModes.Normal;
            var art=ExemplarUi.Rect(body,"SelectedArtwork",.81f,.61f,.97f,.83f);_detailIcon=ExemplarEquipmentIcon.Create(art,"Weapon");_detailIcon.enabled=false;
            var band=ExemplarUi.Rect(body,"SelectedRarityBand",.69f,.60f,.97f,.606f);
            _detailRarity=UiTheme.Flat(band,"Band",UiTheme.Accent);_detailRarity.rectTransform.anchorMin=Vector2.zero;_detailRarity.rectTransform.anchorMax=Vector2.one;_detailRarity.rectTransform.offsetMin=_detailRarity.rectTransform.offsetMax=Vector2.zero;_detailRarity.enabled=false;
            var detailHost=ExemplarUi.Rect(body,"ItemDetails",.69f,.20f,.97f,.57f);
            var detailContent=UiScroll.Create(detailHost,out _);
            _detailText=UiTheme.Label(detailContent,"Description",Loc.Get("UI_COLLECTION_SELECT"),UiTheme.BodySize,UiTheme.TextHigh,TextAlignmentOptions.TopLeft);
            _detailText.textWrappingMode=TextWrappingModes.Normal;_detailText.rectTransform.anchorMin=new Vector2(0,1);_detailText.rectTransform.anchorMax=Vector2.one;_detailText.rectTransform.pivot=new Vector2(.5f,1);_detailText.rectTransform.sizeDelta=new Vector2(0,120);UiScroll.Fit(detailContent,120);
            var sort=HudBar.MakeGhostButton(body,_isStash?Loc.Get("UI_COLLECTION_GRADE")+" ↕":"↕",_isStash?200:44,UiLayout.Touch,()=>{_reverse=!_reverse;LayoutGrid(_items.Length);});
            var sortRect=(RectTransform)sort.transform;sortRect.anchorMin=sortRect.anchorMax=new Vector2(.60f,0);sortRect.anchoredPosition=new Vector2(0,112);if(_isStash){sortRect.anchorMin=sortRect.anchorMax=new Vector2(.10f,0);sortRect.pivot=new Vector2(0,0);sortRect.anchoredPosition=new Vector2(0,24);}
            if(_canAct){var tools=HudBar.MakeGhostButton(body,Loc.Get("UI_BLACKSMITH"),150,UiLayout.Touch,()=>Object.FindFirstObjectByType<HudBar>()?.Floating.Open());var tr=(RectTransform)tools.transform;tr.anchorMin=tr.anchorMax=new Vector2(1,1);tr.pivot=new Vector2(1,1);tr.anchoredPosition=new Vector2(-24,-4);}


            var emptyCard = UiTheme.Box(body, "EmptyState", UiTheme.Surface);
            _emptyPanel = emptyCard.rectTransform;
            _emptyPanel.anchorMin = new Vector2(.14f,.5f); _emptyPanel.anchorMax = new Vector2(.63f,.5f);
            _emptyPanel.offsetMin = new Vector2(0,-130); _emptyPanel.offsetMax = new Vector2(0,130);
            UiTheme.Border(_emptyPanel, UiTheme.LineStrong);
            FuturistChrome.Attach(_emptyPanel);
            _empty = UiTheme.Label(_emptyPanel, "Empty", "", UiTheme.BodySize,
                                   UiTheme.TextLow, TextAlignmentOptions.Center);
            var er = _empty.rectTransform;
            er.anchorMin = Vector2.zero; er.anchorMax = Vector2.one;
            er.offsetMin = new Vector2(24,80); er.offsetMax = new Vector2(-24,-28);
            _empty.textWrappingMode = TextWrappingModes.Normal;
            _empty.gameObject.SetActive(false);
            _emptyPanel.gameObject.SetActive(false);
            _empty.fontSize=20;_empty.color=UiTheme.TextHigh;
            if(_isStash||_carrier!="") {
                _emptyAction=HudBar.MakeButton(_emptyPanel,Loc.Get(_isStash?"UI_STASH_INVENTORY":"UI_UX_VIEW_BANNERS"),"",240,48,()=> {
                    var hud=Object.FindFirstObjectByType<HudBar>();
                    if(hud!=null){if(_isStash)hud.OpenInventoryTools();else hud.Toggle("Summon");}
                },true);
                var a=(RectTransform)_emptyAction.transform;
                a.anchorMin=a.anchorMax=a.pivot=new Vector2(.5f,.5f);
                a.anchoredPosition=new Vector2(0,-65);
                _emptyAction.gameObject.SetActive(false);
            }


            if (_canAct || _isStash || _carrier != "") BuildActions(body, pad);

            _note = UiTheme.Label(body, "Note", Loc.Get("UI_COLLECTION_SELECT"), UiTheme.CaptionSize,
                                  UiTheme.TextLow, TextAlignmentOptions.BottomLeft);
            var nr = _note.rectTransform;
            nr.anchorMin = Vector2.zero; nr.anchorMax = new Vector2(1f, 0f);
            nr.pivot = new Vector2(0.5f, 0f);
            nr.offsetMin = new Vector2(pad, UiTheme.PadSmall);
            nr.offsetMax = new Vector2(-pad, UiTheme.PadSmall + FootH);
        }

        /// The party tabs moved. Only the carrier lists care — whose Exemplar this is IS the
        /// screen — so the others never pay for the subscription.
        void OnEnable() { if (_carrier != "") PartySelection.Changed += OnMemberPicked; }
        void OnDisable() { if (_carrier != "") PartySelection.Changed -= OnMemberPicked; }

        void OnMemberPicked()
        {
            _fingerprint = "";
            if (isActiveAndEnabled) Refresh();
        }

        /// Called when the window opens. Nothing polls: a bag the player is not looking at costs
        /// nothing, and the contents cannot change while they are looking without a tick they
        /// would see anyway.
        public void Refresh()
        {
            // Looked up HERE, not at build time: the bar is created while the arena is still
            // starting, before the driver component exists on it.
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var session = _driver != null ? _driver.Session : null;
            if (session == null) { _count.text = ""; ShowEmpty("No session."); return; }

            if (_carrier != "") _member = ResolveMember(session);
            var json = session.QueryJson(_queryKey);
            var list = JsonUtility.FromJson<ItemList>("{\"items\":" + json + "}");
            var items = list != null && list.items != null ? list.items : new ItemView[0];
            _items = items;
            _renderedRows=MailWindow.RowsOfArray(json);
            if(_sel>=0&&_selectedRaw!=null)
            {
                var rawRows=MailWindow.RowsOfArray(json);
                if(_sel>=rawRows.Count||rawRows[_sel]!=_selectedRaw){_sel=-1;_selectedRaw=null;}
            }
            if (_sel >= items.Length) _sel = -1;
            SyncActions();
            bool capped = !string.IsNullOrEmpty(_capKey);
            if (capped) _cap = ReadCap(session.ViewJson(), _capKey, _cap);
            // The used count is the reading; the cap is context. Rich text rather than two labels
            // so the pair stays kerned as one number no matter how many digits it grows.
            _count.text = capped
                ? $"{items.Length}<color=#{Hex(UiTheme.TextLow)}> / {_cap}</color>"
                : $"{items.Length}<color=#{Hex(UiTheme.TextLow)}> total</color>";

            // Render owned items only. Capacity stays in the counter; empty space is not content.
            int total = items.Length;
            if (total == 0) { ShowEmpty(Loc.Get("UI_GRID_EMPTY_TITLE") + "\n<size=14>" + Loc.Get(_isStash ? "UI_STASH_EMPTY_HINT" : _carrier!=""?"UI_UX_REGALIA_HINT":"UI_GRID_EMPTY_HINT") + "</size>"); return; }
            _empty.gameObject.SetActive(false);
            _emptyPanel.gameObject.SetActive(false);
            _gridHost.gameObject.SetActive(true);
            _grid.gameObject.SetActive(true);
            if(_emptyAction!=null)_emptyAction.gameObject.SetActive(false);

            var fp = new StringBuilder().Append(total).Append('|').Append(_member).Append('|');
            foreach (var it in items)
                fp.Append(it.def_id).Append(':')
                  .Append(it.quantity).Append(':').Append(it.equipped_by).Append(',');
            if (fp.ToString() == _fingerprint) return;
            _fingerprint = fp.ToString();
            Rebuild(items, total);
        }

        void ShowEmpty(string message)
        {
            _empty.text = message;
            _empty.gameObject.SetActive(true);
            _emptyPanel.gameObject.SetActive(true);
            _gridHost.gameObject.SetActive(false);
            if(_emptyAction!=null)_emptyAction.gameObject.SetActive(_isStash||_carrier!="");
            _fingerprint = "";
        }

        void Rebuild(ItemView[] items, int total)
        {
            while (_slots.Count < total) _slots.Add(ItemSlot.Create(_grid));
            for (int i = 0; i < _slots.Count; i++)
            {
                bool used = i < total;
                _slots[i].gameObject.SetActive(used);
                if (!used) continue;
                _slots[i].SetItem(i < items.Length ? items[i] : null);
                _slots[i].SetSelected(i == _sel);
                // WHO IS WEARING IT, in place of the stat line a carrier does not have. Without
                // this the only way to tell an equipped carrier from an idle one is to visit each
                // member in turn, and equipping a second one onto the same member silently
                // replaces the first.
                if (_carrier != "" && i < items.Length)
                {
                    long by = items[i].equipped_by;
                    _slots[i].SetFootnote(by == 0 ? "" : by == _member ? "Worn" : Wearer(by),
                                          by == _member ? UiTheme.Accent : UiTheme.TextLow);
                }
                // The WAREHOUSE selects too — it has a verb of its own — and a list with no verb
                // stays inert so its cells do not eat clicks meant for what is behind them.
                if (!_canAct && !_isStash && _carrier == "") continue;
                int idx = i;
                bool filled = idx < items.Length;
                _slots[i].SetTap(filled ? () => Select(idx) : (System.Action)null);
            }
            LayoutGrid(total);
        }

        /// Columns from the width the window actually has, the way `_recolumn_inventory` does it,
        /// so a resize re-flows instead of clipping. The leftover width is spread BETWEEN the
        /// columns rather than left at the right edge — a grid that hugs one side is the tell
        /// that a layout was computed and not designed.
        void LayoutGrid(int total)
        {
            float avail = _grid.rect.width;
            int cols = Mathf.Max(1, Mathf.FloorToInt((avail + Gap) / (ItemSlot.SlotW + Gap)));
            float step = cols > 1 ? Mathf.Min((avail - ItemSlot.SlotW) / (cols - 1),
                                              ItemSlot.SlotW + Gap * 2f)
                                  : ItemSlot.SlotW + Gap;
            var visible=new System.Collections.Generic.List<int>();
            for(int i=0;i<_items.Length&&i<_slots.Count;i++){
                string kind=_items[i].kind;
                bool show=_category==0||(_category==1&&kind=="Weapon")||(_category==2&&kind=="Armor")||(_category==3&&kind=="Card")||(_category==4&&kind!="Weapon"&&kind!="Armor"&&kind!="Card");
                _slots[i].gameObject.SetActive(show);if(show)visible.Add(i);
            }
            visible.Sort((a,b)=>{int order=_items[a].grade_step.CompareTo(_items[b].grade_step);return order!=0?order:a.CompareTo(b);});
            if(_reverse)visible.Reverse();
            for(int i=0;i<_categoryButtons.Count;i++)_categoryButtons[i].SetActiveState(i==_category);
            _categoryArc.SelectNormalized(_category*.18f+.085f);
            UiScroll.Fit(_grid,Mathf.CeilToInt(visible.Count/(float)cols)*(ItemSlot.SlotH+Gap));_gridWidth=avail;
            for(int i=0;i<visible.Count;i++){
                var rt=(RectTransform)_slots[visible[i]].transform;rt.anchorMin=rt.anchorMax=new Vector2(0,1);rt.pivot=new Vector2(0,1);
                rt.anchoredPosition=new Vector2(i%cols*step,-(i/cols)*(ItemSlot.SlotH+Gap));
            }
        }

        void LateUpdate()
        {
            if (_grid != null && _grid.gameObject.activeSelf && Mathf.Abs(_grid.rect.width - _gridWidth) > 1f)
                LayoutGrid(_slots.FindAll(s => s.gameObject.activeSelf).Count);
        }

        // --- the carriers' verb ------------------------------------------------------------------

        readonly System.Collections.Generic.Dictionary<long, string> _names = new();

        /// WHOSE Exemplar. The party tabs' choice, resolved against who is actually on the field,
        /// exactly as the Cards panel does it — a screen that assumed the lead left a five-member
        /// party with four members it could not reach.
        long ResolveMember(Roi.Sim.SimSession session)
        {
            var v = JsonUtility.FromJson<LeadView>(session.ViewJson());
            var ids = new System.Collections.Generic.List<long>();
            _names.Clear();
            if (v != null && v.grid != null && v.grid.entities != null)
                foreach (var e in v.grid.entities)
                {
                    if (e.team != "Player" || e.is_pet) continue;
                    ids.Add(e.id);
                    _names[e.id] = GameText.UnitName(e.unit_def);
                }
            return PartySelection.Resolve(ids);
        }

        /// A member who is not on the field any more still shows as an id rather than as nothing:
        /// "worn by someone" is the truth, and a blank cell would read as unworn.
        string Wearer(long id) => _names.TryGetValue(id, out var n) ? n : "Worn by #" + id;

        void Equip()
        {
            if (_sel < 0 || _member <= 0) return;
            Fire("{\"Equip" + _carrier + "\":{\"entity_id\":" + _member
                 + ",\"owned_idx\":" + _sel + "}}");
        }

        void Unequip()
        {
            if (_member <= 0) return;
            Fire("{\"Unequip" + _carrier + "\":{\"entity_id\":" + _member + "}}");
        }

        /// The class gate lives in the SIM (`exemplar_allowed_classes`), and it refuses a mismatch
        /// silently like every other bad input. The button cannot ask it — the allow-list is not in
        /// the item view — so the press is offered and the cell simply does not change. Worth
        /// stating rather than inventing a second copy of the table here to grey the button with.
        [System.Serializable] class LeadView { public LeadGrid grid; }
        [System.Serializable] class LeadGrid { public LeadEntity[] entities; }
        [System.Serializable] class LeadEntity { public long id; public string team, unit_def; public bool is_pet; }

        // --- the bag's actions -----------------------------------------------------------------

        /// TAP-SELECT then TAP-ACT, never drag (2026-09-07): a cell selects, and the buttons below
        /// act on the selection. It is also the only shape that works with one thumb.
        UnityEngine.UI.Button _move;

        void BuildActions(RectTransform body, float pad)
        {
            const float w = 150f;
            if (_carrier != "")
            {
                _equip = HudBar.MakeButton(body, Loc.Get("UI_COMMON_EQUIP"), "", 190f, UiLayout.Touch, Equip);
                PlaceAction(_equip, pad, 0f);
                _unequip = HudBar.MakeGhostButton(body, Loc.Get("UI_COMMON_UNEQUIP"), w, UiLayout.Touch, Unequip);
                PlaceAction(_unequip, pad, 190f + UiTheme.Gap);
                return;
            }
            if (_isStash)
            {
                // The warehouse has ONE verb. Selling from here is not offered on purpose: the
                // sim's sell takes a bag index, and a screen that quietly moved the item first
                // would be doing two things behind one press.
                _move = HudBar.MakeButton(body, Loc.Get("UI_COMMON_WITHDRAW"), "", w, UiLayout.Touch, Withdraw);
                PlaceAction(_move, pad, 0f);
                var withdrawRect=(RectTransform)_move.transform;withdrawRect.anchorMin=withdrawRect.anchorMax=new Vector2(.975f,0);withdrawRect.pivot=new Vector2(1,0);withdrawRect.anchoredPosition=new Vector2(0,24);withdrawRect.sizeDelta=new Vector2(220,UiLayout.Touch);
                return;
            }
            // Vendoring is retired (sim 0.0.411): Gil comes from fusion, so the bag offers
            // discard and deposit only.
            _discard = HudBar.MakeGhostButton(body, Loc.Get("UI_COMMON_DISCARD"), w, UiLayout.Touch, Discard);
            PlaceAction(_discard, pad, 0f);
            _move = HudBar.MakeGhostButton(body, Loc.Get("UI_COMMON_DEPOSIT"), w, UiLayout.Touch, Deposit);
            PlaceAction(_move, pad, 2*(w + UiTheme.Gap));
            _disenchant=HudBar.MakeGhostButton(body,Loc.Get("UI_DISENCHANT"),w,UiLayout.Touch,Disenchant);
            PlaceAction(_disenchant,pad,w+UiTheme.Gap);
        }

        void PlaceAction(UnityEngine.UI.Button b, float pad, float x)
        {
            var rt = (RectTransform)b.transform;
            rt.anchorMin = rt.anchorMax = new Vector2(0f, 0f);
            rt.pivot = new Vector2(0f, 0f);
            rt.anchoredPosition = new Vector2(pad + x, FootH + UiTheme.PadSmall);
        }

        void Select(int idx)
        {
            var fresh=MailWindow.RowsOfArray(_driver.Session.QueryJson(_queryKey));
            if(_renderedRows==null||idx<0||idx>=_renderedRows.Count||idx>=fresh.Count||fresh[idx]!=_renderedRows[idx])
            { _sel=-1;_selectedRaw=null;_fingerprint="";Refresh();return; }
            _sel = _sel == idx ? -1 : idx;      // tapping the selected cell again clears it
            _selectedRaw=_sel>=0?_renderedRows[_sel]:null;
            for (int i = 0; i < _slots.Count; i++) _slots[i].SetSelected(i == _sel);
            SyncActions();
        }

        /// What the buttons can do RIGHT NOW. Discard is the one liquidation path left (vendoring
        /// retired, sim 0.0.411); a bound item accepts it too.
        void SyncActions()
        {
            if(_detailText!=null){bool chosen=_sel>=0&&_sel<_items.Length;_detailIcon.enabled=chosen;
                _detailName.text=chosen?GameText.ItemName(_items[_sel]):"";
                _detailText.text=chosen?FloatingItemDescription.Format(_items[_sel],inspector:true):Loc.Get("UI_COLLECTION_SELECT");
                float width=Mathf.Max(200,_detailText.rectTransform.rect.width);
                float height=Mathf.Max(80,_detailText.GetPreferredValues(_detailText.text,width,10000).y+16);
                _detailText.rectTransform.sizeDelta=new Vector2(0,height);UiScroll.Fit((RectTransform)_detailText.transform.parent,height);
                _detailRarity.enabled=chosen;_detailGrade.text="";
                if(chosen){
                    var item=_items[_sel];var tone=ItemSlot.ItemColor(item);
                    _detailName.color=Color.Lerp(tone,UiTheme.TextHigh,.35f);_detailRarity.color=tone;
                    _detailGrade.text=item.kind=="Weapon"||item.kind=="Armor"?GameText.GradeLabel(item.grade_step):Loc.GetOr(item.rarity??"",item.rarity??"");
                    if(item.kind=="Card")_detailGrade.text+="\n"+Loc.Fmt("UI_FLOAT_STARS",item.star);
                    _detailIcon.SetItem(item,item.slot);
                }
            }
            if (_carrier != "")
            {
                if (_equip == null) return;
                var who = _member > 0 && _names.TryGetValue(_member, out var n) ? n : "";
                var pick = _sel >= 0 && _sel < _items.Length ? _items[_sel] : null;
                // The button NAMES its target. "Equip" alone on a screen whose target is chosen by
                // a strip of tabs somewhere else is the press people get wrong.
                _equip.GetComponentInChildren<TextMeshProUGUI>().text =
                    who == "" ? "Equip" : "Equip on " + who;
                _equip.interactable = pick != null && _member > 0 && pick.equipped_by != _member;
                if(pick==null)_equip.GetComponentInChildren<TextMeshProUGUI>().text=Loc.Get("UI_UX_SELECT_ITEM");
                else if(pick.equipped_by==_member)_equip.GetComponentInChildren<TextMeshProUGUI>().text=Loc.Get("UI_UX_EQUIPPED");
                _equip.gameObject.SetActive(_items.Length>0);
                _unequip.gameObject.SetActive(_items.Length>0);
                bool wearing = false;
                foreach (var owned in _items) if (owned.equipped_by == _member) { wearing = true; break; }
                _unequip.interactable = wearing;
                return;
            }
            if (_isStash)
            {
                if (_move != null) {
                    _move.interactable = _sel >= 0 && _sel < _items.Length;
                    _move.gameObject.SetActive(_items.Length>0);
                }
                if(_note!=null)_note.gameObject.SetActive(_items.Length>0);
                return;
            }
            if (!_canAct || _discard == null) return;
            if (_move != null) _move.interactable = _sel >= 0 && _sel < _items.Length;
            var it = _sel >= 0 && _sel < _items.Length ? _items[_sel] : null;
            _discard.interactable = it != null;
            _disenchantChoice=it==null?null:CrestModel.Freeze(_driver.Session,_sel);
            if(_disenchant!=null)_disenchant.interactable=_disenchantChoice!=null;
            if(_disenchantChoice!=null)_note.text=CrestModel.Yield(_disenchantChoice.Reward);
            else if(_note!=null)_note.text=_isStash?"":Loc.Get("UI_COLLECTION_SELECT");
        }

        void Disenchant()
        {
            var choice=_disenchantChoice;
            if(choice==null)return;
            UiConfirm.Ask(Loc.Get("UI_DISENCHANT"),Loc.Fmt("UI_DISENCHANT_CONFIRM",ForgeUi.Name(_items[_sel]))+"\n"+CrestModel.Yield(choice.Reward),
                Loc.Get("UI_DISENCHANT"),()=>
                {
                    var result=CrestModel.Disenchant(_driver,choice);
                    _sel=-1;_selectedRaw=null;_fingerprint="";Refresh();
                    _note.text=result==Roi.Sim.VerifiedInputResult.Applied?CrestModel.Yield(choice.Reward):Loc.Get("UI_EX_GEAR_STALE");
                });
        }

        void Discard()
        {
            if (_sel < 0 || _sel >= _items.Length) return;
            var it = _items[_sel];
            var name = string.IsNullOrEmpty(it.name) ? it.def_id : it.name;
            int idx = _sel;
            // THE ONE ACTION THAT ASKS. Everywhere else the sim is the net — a bad input is a
            // silent no-op, a purchase refuses, a listing can be cancelled — but discarding is
            // the destruction the sim deliberately allows, so the guard has to be in the UI.
            UiConfirm.Ask("Discard", $"Destroy {name}? It is gone for good, and nothing is paid "
                                   + "for it.", "Discard",
                          () => Fire("{\"DiscardItem\":{\"inv_idx\":" + idx + "}}"));
        }

        /// Bag -> warehouse. The stash is where gear waits without eating the bag's cap, and this
        /// is the only way in.
        void Deposit()
        {
            if (_sel < 0) return;
            Fire("{\"DepositToStash\":{\"inv_idx\":" + _sel + "}}");
        }

        /// Warehouse -> bag. Refused by the sim when the bag is full, which is a real answer: the
        /// purse-style check below reports it rather than pretending the press did nothing.
        void Withdraw()
        {
            if (_sel < 0) return;
            Fire("{\"WithdrawFromStash\":{\"stash_idx\":" + _sel + "}}");
        }

        void Fire(string eventJson)
        {
            var session = _driver != null ? _driver.Session : null;
            if (session == null) return;
            session.ApplyInput(eventJson);
            _sel = -1;
            _fingerprint = "";      // the set changed under us; force the rebuild
            Refresh();
        }

        static string Hex(Color c) => ColorUtility.ToHtmlStringRGB(c);

        /// The cap rides the per-tick view (it is one number, not a list). Read with a regex
        /// rather than a DTO: a whole view class here would be a second contract to keep.
        static int ReadCap(string viewJson, string key, int fallback)
        {
            var m = System.Text.RegularExpressions.Regex.Match(viewJson, "\"" + key + "\":(\\d+)");
            return m.Success && int.TryParse(m.Groups[1].Value, out var v) ? v : fallback;
        }
    }
}
