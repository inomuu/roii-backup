// Regalia inspection binds the explicit Exemplar id. Browsing never changes PartySelection.
// Effect text, enhancement level and resonance are host projections, not client calculations.
using System;
using System.Collections.Generic;
using System.Text;
using Roi.Sim;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class ExemplarRegaliaPage : MonoBehaviour
    {
        SimDriver _driver;
        SimSession _session;
        ExemplarReadModel.Member _member;
        OwnedRegalia[] _items = Array.Empty<OwnedRegalia>();
        readonly List<int> _visible = new();
        readonly List<OwnedCell> _cells = new();
        readonly Dictionary<long,string> _names = new();
        RectTransform _collection, _detailContent, _leftPanel, _rightPanel;
        Vector2 _leftSize, _rightSize;
        bool _panelLayoutDirty=true;
        TextMeshProUGUI _memberName, _equipped, _selectedName, _axes, _wearer, _details, _status, _count, _empty;
        Button _equip, _unequip;
        string _fingerprint = "", _pickIdentity = "";
        int _selected = -1;
        float _gridWidth = -1, _detailWidth = -1;
        string _detailValue = "";
        bool _available;

        public static ExemplarRegaliaPage Create(Transform parent)
        {
            var rt = ExemplarUi.Rect(parent,"ExemplarRegaliaPage",0,0,1,1);
            var page = rt.gameObject.AddComponent<ExemplarRegaliaPage>();page.Build();return page;
        }

        void Build()
        {
            var left = ExemplarUi.Rect(transform,"RegaliaDetails",0,.035f,.255f,.99f);
            _leftPanel=left;
            ExemplarUi.Panel(left,"Shade",0,0,1,1,ExemplarUi.Surface);
            ExemplarUi.Panel(left,"Header",0,.895f,1,1,ExemplarUi.Deep);
            ExemplarUi.Panel(left,"EquippedTile",.06f,.746f,.94f,.878f,ExemplarUi.Card);
            ExemplarUi.Panel(left,"EquippedEdge",.06f,.746f,.064f,.878f,ExemplarUi.Gold);
            ExemplarUi.Panel(left,"TopRule",.06f,.99f,.94f,.992f,ExemplarUi.Gold);
            ExemplarUi.Panel(left,"LeftEdge",0,0,.0025f,1,ExemplarUi.Line);
            _memberName = ExemplarUi.Text(left,"MemberName","",.075f,.915f,.925f,.972f,32,ExemplarUi.White);
            ExemplarUi.Text(left,"EquippedCaption",Loc.Get("UI_EXEMPLAR_EQUIPPED"),.095f,.834f,.905f,.867f,17,ExemplarUi.Muted);
            _equipped = ExemplarUi.Text(left,"EquippedRegalia","",.095f,.763f,.905f,.824f,22,ExemplarUi.White);
            _equipped.textWrappingMode = TextWrappingModes.Normal;
            ExemplarUi.Panel(left,"Divider",.075f,.722f,.925f,.724f,ExemplarUi.Line);
            ExemplarUi.Text(left,"SelectionCaption",Loc.Get("UI_EXEMPLAR_REGALIA_PREVIEW"),.075f,.665f,.925f,.704f,19,ExemplarUi.Gold);
            _selectedName = ExemplarUi.Text(left,"SelectedName","",.075f,.568f,.925f,.650f,30,ExemplarUi.White);
            _axes = ExemplarUi.Text(left,"EnhancementResonance","",.075f,.508f,.925f,.555f,19,ExemplarUi.White);
            _wearer = ExemplarUi.Text(left,"Wearer","",.075f,.455f,.925f,.497f,18,ExemplarUi.Muted);
            var detailHost = ExemplarUi.Rect(left,"EffectsScroll",.075f,.294f,.925f,.438f);
            _detailContent = UiScroll.Create(detailHost,out _);
            _details = ExemplarUi.Text(_detailContent,"Effects","",0,0,1,1,20,ExemplarUi.White,TextAlignmentOptions.TopLeft);
            _details.enableAutoSizing = false;_details.textWrappingMode = TextWrappingModes.Normal;
            _details.overflowMode = TextOverflowModes.Overflow;
            // The information stack scrolls as a whole, so longer names and effects never
            // take space away from the fixed action footer.
            foreach(var name in new[]{"EquippedTile","EquippedEdge","EquippedCaption","EquippedRegalia","Divider","SelectionCaption","SelectedName","EnhancementResonance","Wearer"})
                left.Find(name).SetParent(_detailContent,false);
            _details.transform.SetAsLastSibling();
            _status = ExemplarUi.Text(left,"Status","",.075f,.232f,.925f,.279f,17,ExemplarUi.Gold);
            _status.textWrappingMode = TextWrappingModes.Normal;
            _equip = ExemplarUi.Button(left,"EquipRegalia",Loc.Get("UI_COMMON_EQUIP"),.075f,.133f,.925f,.211f,Equip,true);
            _unequip = ExemplarUi.Button(left,"UnequipRegalia",Loc.Get("UI_COMMON_UNEQUIP"),.075f,.035f,.925f,.113f,Unequip);

            // The unpainted center retains the same Live2D presentation as Attributes.
            var right = ExemplarUi.Rect(transform,"OwnedRegalia",.76f,.035f,1,.99f);
            _rightPanel=right;
            ExemplarUi.Panel(right,"Shade",0,0,1,1,ExemplarUi.Surface);
            ExemplarUi.Panel(right,"Header",0,.832f,1,1,ExemplarUi.Deep);
            ExemplarUi.Panel(right,"TopRule",.055f,.99f,.945f,.992f,ExemplarUi.Gold);
            ExemplarUi.Panel(right,"CountPlate",.815f,.865f,.935f,.945f,ExemplarUi.Card);
            var collectionTitle=ExemplarUi.Text(right,"CollectionTitle",Loc.Get("UI_EXEMPLAR_REGALIA_COLLECTION"),.075f,.855f,.775f,.955f,23,ExemplarUi.White);
            collectionTitle.textWrappingMode=TextWrappingModes.Normal;collectionTitle.fontSizeMin=16;
            _count = ExemplarUi.Text(right,"OwnedCount","",.82f,.870f,.93f,.940f,21,ExemplarUi.White,TextAlignmentOptions.Center);
            ExemplarUi.Panel(right,"Rule",.075f,.832f,.925f,.834f,ExemplarUi.Line);
            var gridHost = ExemplarUi.Rect(right,"CollectionScroll",.06f,.035f,.94f,.805f);
            _collection = UiScroll.Create(gridHost,out _);
            _empty = ExemplarUi.Text(right,"EmptyCollection","",.075f,.40f,.925f,.75f,23,ExemplarUi.Muted,TextAlignmentOptions.Center);
            Paint();
        }

        public void Show(ExemplarReadModel.Member member)
        {
            if(_driver == null)_driver = UnityEngine.Object.FindFirstObjectByType<SimDriver>();
            var session = _driver != null ? _driver.Session : null;
            bool changed = (_member?.Id ?? 0) != (member?.Id ?? 0) || session != _session;
            _member = member;_session = session;
            if(changed){_selected = -1;_pickIdentity = "";_fingerprint = "";_status.text = "";}
            var json = session != null && session.IsAlive ? session.QueryJson("owned_regalia") : null;
            var items = ReadItems(json);
            _available = member != null && member.Id > 0 && !member.Benched && member.Stats != null && items != null;
            if(items == null)
            {
                _items = Array.Empty<OwnedRegalia>();_visible.Clear();_selected = -1;_fingerprint = "";
                foreach(var cell in _cells)cell.Root.gameObject.SetActive(false);
            }
            else if(json != _fingerprint)
            {
                _items = items;_fingerprint = json;
                if(_selected >= items.Length || (_selected >= 0 && Identity(items[_selected]) != _pickIdentity))_selected = -1;
                _visible.Clear();
                for(int i=0;i<items.Length;i++)
                    // Fodder has no equippable overlay definition. Keep source indices unchanged.
                    if(items[i] != null && items[i].kind == "Regalia" && items[i].overlay_kind == "regalia")_visible.Add(i);
                ReadNames();
                if(_selected < 0)
                {
                    foreach(int i in _visible)if(items[i].equipped_by == (member?.Id ?? 0) && member != null){_selected = i;break;}
                    if(_selected < 0 && _visible.Count > 0)_selected = _visible[0];
                }
                _pickIdentity = Pick != null ? Identity(Pick) : "";
                SyncCells();
            }
            Paint();
        }

        OwnedRegalia Pick => _selected >= 0 && _selected < _items.Length ? _items[_selected] : null;
        void ReadNames()
        {
            _names.Clear();
            if(_session == null || !_session.IsAlive)return;
            var view = ReadObject<WearerView>(_session.ViewJson());
            if(view?.grid?.entities == null)return;
            foreach(var entity in view.grid.entities)
                if(entity != null && entity.team == "Player" && !entity.is_pet)_names[entity.id] = GameText.UnitName(entity.unit_def);
        }
        string Wearer(long id) => _names.TryGetValue(id,out var name) ? name : "#"+id;
        static string Identity(OwnedRegalia item) => item == null ? "" : JsonUtility.ToJson(item);

        void SyncCells()
        {
            while(_cells.Count < _visible.Count)_cells.Add(CreateCell());
            for(int n=0;n<_cells.Count;n++)
            {
                var cell = _cells[n];cell.Root.gameObject.SetActive(n < _visible.Count);
                if(n >= _visible.Count)continue;
                int index = _visible[n];var item = _items[index];cell.Index = index;
                cell.Root.name = "Regalia_"+index;cell.Name.text = GameText.ItemName(item);
                cell.Level.text = Loc.Fmt("UI_COMMON_LEVEL_SHORT",item.enhance_level)+"  ·  "+Loc.Get("UI_RES_WORD")+" "+item.resonance_stars;
                cell.Wearer.text = item.equipped_by == 0 ? Loc.Get("UI_EXEMPLAR_REGALIA_UNEQUIPPED")
                    : item.equipped_by == (_member?.Id ?? 0) ? Loc.Get("UI_EXEMPLAR_EQUIPPED") : Loc.Fmt("UI_EXEMPLAR_REGALIA_WEARER",Wearer(item.equipped_by));
                cell.Rarity.color = ItemSlot.RarityColor(item.rarity,false);
            }
            _gridWidth = -1;LayoutCells();
        }

        OwnedCell CreateCell()
        {
            var root = ExemplarUi.Rect(_collection,"Regalia",0,1,0,1);
            var cell = new OwnedCell { Root = root };
            var b = ExemplarUi.Button(root,"Select","",0,0,1,1,()=>Select(cell.Index));
            b.GetComponentInChildren<TextMeshProUGUI>().gameObject.SetActive(false);
            cell.Face = b.GetComponent<Image>();
            ExemplarUi.Panel(root,"TileHeader",0,.80f,1,1,ExemplarUi.Deep);
            ExemplarUi.Panel(root,"TileTopEdge",0,.994f,1,1,ExemplarUi.Line);
            ExemplarUi.Panel(root,"FooterRule",.085f,.273f,.935f,.277f,ExemplarUi.Line);
            cell.Rarity = ExemplarUi.Panel(root,"Rarity",0,0,1,.020f,ExemplarUi.Gold);
            cell.Tick = ExemplarUi.Panel(root,"Selection",0,.08f,.018f,.96f,ExemplarUi.Gold);
            cell.Name = ExemplarUi.Text(root,"Name","",.085f,.415f,.935f,.94f,24,ExemplarUi.White,TextAlignmentOptions.TopLeft);
            cell.Name.textWrappingMode = TextWrappingModes.Normal;cell.Name.overflowMode = TextOverflowModes.Ellipsis;
            cell.Level = ExemplarUi.Text(root,"Progression","",.085f,.281f,.94f,.413f,16,ExemplarUi.Gold);
            cell.Wearer = ExemplarUi.Text(root,"Wearer","",.085f,.06f,.94f,.25f,15,ExemplarUi.Muted);
            cell.Wearer.overflowMode = TextOverflowModes.Ellipsis;
            return cell;
        }
        void Select(int index)
        {
            if(!_visible.Contains(index))return;
            _selected = index;_pickIdentity = Identity(Pick);_status.text = "";Paint();
        }
        void LateUpdate()
        {
            if(_details != null && _detailContent.rect.width > 0 &&
                (_detailWidth != _detailContent.rect.width || _detailValue != _details.text))
            {
                _detailWidth = _detailContent.rect.width;_detailValue = _details.text;
                _panelLayoutDirty=true;
            }
            LayoutPanels();LayoutCells();
        }
        // Content uses reference-unit padding, not fractions of the panel height. Wrapped
        // names reserve their measured height; the effects area scrolls above a fixed footer.
        void LayoutPanels()
        {
            if(_leftPanel==null||_rightPanel==null)return;
            var leftSize=_leftPanel.rect.size;var rightSize=_rightPanel.rect.size;
            if(leftSize.x<=0||rightSize.x<=0)return;
            if(!_panelLayoutDirty&&leftSize==_leftSize&&rightSize==_rightSize)return;
            _panelLayoutDirty=false;_leftSize=leftSize;_rightSize=rightSize;
            const float pad=24, gap=16, buttonH=48, buttonGap=12;
            float width=leftSize.x-pad*2;
            TopInset(_leftPanel,"TopRule",pad,pad,8,2);
            float y=24;
            FlowPanel(_memberName,ref y,pad,width,42,18);
            TopInset(_leftPanel,"Header",1,1,1,y-9);
            float actionTop=pad+buttonH*2+buttonGap;
            BottomInset((RectTransform)_unequip.transform,pad,pad,pad,buttonH);
            BottomInset((RectTransform)_equip.transform,pad,pad,pad+buttonH+buttonGap,buttonH);
            BottomInset(_status.rectTransform,pad,pad,actionTop+12,40);
            var effects=(RectTransform)_leftPanel.Find("EffectsScroll");
            effects.anchorMin=Vector2.zero;effects.anchorMax=Vector2.one;
            effects.offsetMin=new Vector2(pad,actionTop+64);effects.offsetMax=new Vector2(-pad,-y);
            float contentWidth=_detailContent.rect.width>1?_detailContent.rect.width:width-14;
            float rowY=12;
            var equippedCaption=_detailContent.Find("EquippedCaption").GetComponent<TextMeshProUGUI>();
            FlowPanel(equippedCaption,ref rowY,16,contentWidth-32,22,8);
            FlowPanel(_equipped,ref rowY,16,contentWidth-32,28,12);
            TopInset(_detailContent,"EquippedTile",0,0,0,rowY);
            TopInset(_detailContent,"EquippedEdge",0,contentWidth-2,0,rowY);
            rowY+=gap;
            TopInset(_detailContent,"Divider",0,0,rowY,1);rowY+=gap;
            var selection=_detailContent.Find("SelectionCaption").GetComponent<TextMeshProUGUI>();
            FlowPanel(selection,ref rowY,0,contentWidth,24,12);
            FlowPanel(_selectedName,ref rowY,0,contentWidth,38,16);
            FlowPanel(_axes,ref rowY,0,contentWidth,24,8);
            FlowPanel(_wearer,ref rowY,0,contentWidth,24,16);
            FlowPanel(_details,ref rowY,0,contentWidth,24,16);
            UiScroll.Fit(_detailContent,rowY);
            foreach(var b in new[]{_equip,_unequip})
            {
                var label=b.GetComponentInChildren<TextMeshProUGUI>().rectTransform;
                label.anchorMin=Vector2.zero;label.anchorMax=Vector2.one;
                label.offsetMin=new Vector2(16,8);label.offsetMax=new Vector2(-16,-8);
            }
            TopInset(_rightPanel,"TopRule",pad,pad,8,2);
            var title=_rightPanel.Find("CollectionTitle").GetComponent<TextMeshProUGUI>();
            float titleWidth=rightSize.x-pad*2-44-16;
            float headerH=Mathf.Max(48,title.GetPreferredValues(title.text,Mathf.Max(1,titleWidth),0).y+4);
            TopInset(title.rectTransform,pad,pad+60,24,headerH);
            TopInset(_rightPanel,"CountPlate",rightSize.x-pad-44,pad,24,44);
            TopInset(_count.rectTransform,rightSize.x-pad-44,pad,24,44);
            float ruleY=24+headerH+20;
            TopInset(_rightPanel,"Header",1,1,1,ruleY-1);
            TopInset(_rightPanel,"Rule",pad,pad,ruleY,1);
            var collection=(RectTransform)_rightPanel.Find("CollectionScroll");
            collection.anchorMin=Vector2.zero;collection.anchorMax=Vector2.one;
            collection.offsetMin=new Vector2(pad,pad);collection.offsetMax=new Vector2(-pad,-ruleY-20);
            _empty.rectTransform.anchorMin=Vector2.zero;_empty.rectTransform.anchorMax=Vector2.one;
            _empty.rectTransform.offsetMin=new Vector2(pad,pad);_empty.rectTransform.offsetMax=new Vector2(-pad,-ruleY-20);
        }
        static void FlowPanel(TextMeshProUGUI label,ref float y,float inset,float width,float minHeight,float gap)
        {
            bool visible=!string.IsNullOrEmpty(label.text);label.gameObject.SetActive(visible);if(!visible)return;
            float height=Mathf.Max(minHeight,label.GetPreferredValues(label.text,Mathf.Max(1,width),0).y+4);
            TopInset(label.rectTransform,inset,inset,y,height);y+=height+gap;
        }
        static void TopInset(Transform parent,string name,float left,float right,float y,float height)
            =>TopInset((RectTransform)parent.Find(name),left,right,y,height);
        static void TopInset(RectTransform rect,float left,float right,float y,float height)
        {
            rect.anchorMin=new Vector2(0,1);rect.anchorMax=Vector2.one;rect.pivot=new Vector2(.5f,1);
            rect.offsetMin=new Vector2(left,-y-height);rect.offsetMax=new Vector2(-right,-y);
        }
        static void BottomInset(RectTransform rect,float left,float right,float y,float height)
        {
            rect.anchorMin=Vector2.zero;rect.anchorMax=new Vector2(1,0);rect.pivot=new Vector2(.5f,0);
            rect.offsetMin=new Vector2(left,y);rect.offsetMax=new Vector2(-right,y+height);
        }
        void LayoutCells()
        {
            if(_collection == null)return;
            float width = _collection.rect.width;if(Mathf.Abs(width-_gridWidth) < .1f || width <= 0)return;_gridWidth = width;
            int columns = width >= 325 ? 2 : 1;const float gap = 12,height = 166;
            float cellWidth = (width-gap*(columns-1))/columns;
            for(int i=0;i<_visible.Count;i++)
            {
                var rt = _cells[i].Root;rt.anchorMin = rt.anchorMax = new Vector2(0,1);rt.pivot = new Vector2(0,1);
                rt.sizeDelta = new Vector2(cellWidth,height);rt.anchoredPosition = new Vector2((i%columns)*(cellWidth+gap),-(i/columns)*(height+gap));
            }
            UiScroll.Fit(_collection,Mathf.CeilToInt((float)_visible.Count/columns)*(height+gap));
        }

        void Paint()
        {
            if(_memberName == null)return;
            _memberName.text = _member == null ? Loc.Get("UI_EXEMPLAR_NO_CHARACTER") : GameText.UnitName(_member.UnitDef);
            var worn = _member?.Regalia;
            _equipped.text = !_available ? Loc.Get("UI_EXEMPLAR_REGALIA_UNAVAILABLE") : worn == null || string.IsNullOrEmpty(worn.id)
                ? Loc.Get("UI_EXEMPLAR_REGALIA_EMPTY_SLOT") : GameText.ItemName(worn.id.StartsWith("regalia_") ? worn.id : "regalia_"+worn.id,worn.name);
            _count.text = _visible.Count.ToString("N0");
            _empty.gameObject.SetActive(_visible.Count == 0);
            _empty.text = _session == null || !_session.IsAlive ? Loc.Get("UI_SHOP_NO_SESSION") : Loc.Get("UI_REGALIA_NONE");
            var pick = Pick;
            _selectedName.text = pick == null ? Loc.Get("UI_GEAR_PICKHINT") : GameText.ItemName(pick);
            _axes.text = pick == null ? "" : Loc.Fmt("UI_EXEMPLAR_REGALIA_LEVEL",pick.enhance_level)+"  ·  "+Loc.Get("UI_RES_WORD")+" "+pick.resonance_stars;
            _wearer.text = pick == null ? "" : pick.equipped_by == 0 ? Loc.Get("UI_EXEMPLAR_REGALIA_UNEQUIPPED") : Loc.Fmt("UI_EXEMPLAR_REGALIA_WEARER",Wearer(pick.equipped_by));
            var effects = new StringBuilder();
            if(pick != null)
            {
                bool resolved = _available && pick.equipped_by == _member.Id && worn != null && (worn.id == pick.def_id || "regalia_"+worn.id == pick.def_id);
                AddLines(effects,Loc.Get(resolved ? "UI_EXEMPLAR_REGALIA_EFFECTS" : "UI_EXEMPLAR_REGALIA_BASE"),resolved ? worn.lines : pick.overlay_lines);
                if(!resolved)AddLines(effects,Loc.Get("UI_OVERLAY_PER_LEVEL"),pick.overlay_level_lines);
                // The host exposes the first rank's mods, not a uniform bonus for every star.
                AddLines(effects,Loc.Get("UI_EXEMPLAR_REGALIA_FIRST_RANK"),pick.overlay_star_lines);
                if(pick.overlay_skills != null && pick.overlay_skills.Length > 0)
                {
                    var grants = new List<string>();
                    foreach(var skill in pick.overlay_skills)
                        if(skill != null && !string.IsNullOrEmpty(skill.id))grants.Add(GameText.SkillName(skill.id)+"  "+Loc.Fmt("UI_COMMON_LEVEL_SHORT",skill.level));
                    AddLines(effects,Loc.Get("UI_EXEMPLAR_REGALIA_BASE_SKILLS"),grants.ToArray());
                }
                // A Regalia is built for a DOCTRINE (0.0.436): anyone equips it and takes the stats;
                // its passive works only on a wearer of that doctrine. The class list is retired.
                if(!string.IsNullOrEmpty(pick.overlay_doctrine))
                    AddLines(effects,Loc.Get("UI_OVERLAY_BUILT_FOR"),new[]{Loc.Get("UI_DOCTRINE_"+pick.overlay_doctrine)});
            }
            _details.text = effects.ToString();
            _equip.GetComponentInChildren<TextMeshProUGUI>().text = _member == null ? Loc.Get("UI_COMMON_EQUIP") : Loc.Fmt("UI_EXEMPLAR_REGALIA_EQUIP_ON",GameText.UnitName(_member.UnitDef));
            _equip.interactable = _available && pick != null && pick.equipped_by != _member.Id;
            _unequip.interactable = _available && Array.Exists(_items,it=>it != null && it.equipped_by == _member.Id);
            foreach(var cell in _cells)
            {
                bool selected = cell.Index == _selected;cell.Tick.gameObject.SetActive(selected);
                cell.Face.color = selected ? ExemplarUi.Selected : ExemplarUi.Card;
            }
            _panelLayoutDirty=true;LayoutPanels();
        }
        static void AddLines(StringBuilder text,string title,string[] lines)
        {
            if(lines == null || lines.Length == 0)return;
            if(text.Length > 0)text.Append("\n\n");text.Append(title).Append('\n').Append(string.Join("\n",lines));
        }

        void Equip()
        {
            if(!_equip.interactable || Pick == null)return;
            long memberId = _member.Id;int index = _selected;var session = _session;string version = _fingerprint;
            var pick = Pick;
            Action apply = ()=>Apply(memberId,index,session,version,false);
            if(pick.equipped_by != 0)
                UiConfirm.Ask(Loc.Get("UI_REGALIA_EQUIP_TITLE"),Loc.Fmt("UI_EXEMPLAR_REGALIA_TRANSFER",GameText.ItemName(pick),Wearer(pick.equipped_by),GameText.UnitName(_member.UnitDef)),Loc.Get("UI_COMMON_EQUIP"),apply,false);
            else apply();
        }
        void Unequip()
        {
            if(!_unequip.interactable)return;
            Apply(_member.Id,-1,_session,_fingerprint,true);
        }
        void Apply(long memberId,int index,SimSession session,string version,bool remove)
        {
            // Confirmations never retarget to a new selection, save slot, or shifted owned index.
            if(this == null || !isActiveAndEnabled || _member?.Id != memberId)return;
            if(_driver == null || _driver.Session != session || session == null || !session.IsAlive || session.QueryJson("owned_regalia") != version)
            { _status.text = Loc.Get("UI_EXEMPLAR_REGALIA_CHANGED");return; }
            var current = ExemplarReadModel.Fetch(_driver).Find(memberId);
            if(current == null || current.Benched || current.Stats == null)
            { _status.text = Loc.Get("UI_EXEMPLAR_REGALIA_UNAVAILABLE");return; }
            string input = remove ? "{\"UnequipRegalia\":{\"entity_id\":"+memberId+"}}"
                : "{\"EquipRegalia\":{\"entity_id\":"+memberId+",\"owned_idx\":"+index+"}}";
            bool accepted = _driver.ApplyInput(input);
            var after = ReadItems(session.QueryJson("owned_regalia"));
            bool changed = accepted && after != null && (remove ? !Array.Exists(after,it=>it != null && it.equipped_by == memberId)
                : index >= 0 && index < after.Length && after[index] != null && after[index].equipped_by == memberId);
            _fingerprint = "";_selected = -1;Show(ExemplarReadModel.Fetch(_driver).Find(memberId));
            _status.text = Loc.Get(changed ? "UI_EXEMPLAR_REGALIA_UPDATED" : "UI_EXEMPLAR_REGALIA_REFUSED");
        }
        static T ReadObject<T>(string json) where T:class
        {
            if(string.IsNullOrWhiteSpace(json) || json.TrimStart()[0] != '{')return null;
            try{return JsonUtility.FromJson<T>(json);}catch(ArgumentException){return null;}
        }
        static OwnedRegalia[] ReadItems(string json)
        {
            if(string.IsNullOrWhiteSpace(json) || json.TrimStart()[0] != '[')return null;
            return ReadObject<OwnedList>("{\"items\":"+json+"}")?.items;
        }
        sealed class OwnedCell
        {
            public int Index;public RectTransform Root;public Image Face,Tick,Rarity;
            public TextMeshProUGUI Name,Level,Wearer;
        }
        [Serializable] sealed class OwnedList { public OwnedRegalia[] items; }
        [Serializable] sealed class OwnedRegalia : ItemView
        {
            public int enhance_level,resonance_stars;
            public string overlay_kind;
            public string[] overlay_level_lines,overlay_star_lines,overlay_classes;
            public string overlay_doctrine;
            public GrantedSkill[] overlay_skills;
        }
        [Serializable] sealed class GrantedSkill { public string id;public int level; }
        [Serializable] sealed class WearerView { public WearerGrid grid; }
        [Serializable] sealed class WearerGrid { public WearerEntity[] entities; }
        [Serializable] sealed class WearerEntity { public long id;public string team,unit_def;public bool is_pet; }
    }
}
