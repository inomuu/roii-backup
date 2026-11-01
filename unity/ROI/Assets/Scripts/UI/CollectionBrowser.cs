using System;
using System.Collections.Generic;
using System.Linq;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Shared, retained collection presentation. Entries keep their native row identity when sorted.
    public sealed class CollectionBrowser : MonoBehaviour
    {
        public sealed class Entry
        {
            public ItemView Item;
            public ExemplarEquipmentActions.Row Row;
            public string Key;
            public bool Gem, IsNew;
        }
        sealed class Tile
        {
            public RectTransform Root;
            public Button Button;
            public Image Background, Accent, Border, Portrait;
            public ExemplarEquipmentIcon Icon;
            public ForgeSigil Gem;
            public TextMeshProUGUI Caption, New;
            public Entry Entry;
        }
        public Action<Entry> Pick;
        public Action Primary, Back, Tools, PreviousMember, NextMember;
        public Func<int,Entry,bool> Matches;
        public Button PrimaryButton { get; private set; }
        public IReadOnlyList<Entry> VisibleEntries => _visible;
        public ScrollRect GridScroll => _scroll;
        public ScrollRect DetailScroll => _detailScroll;
        readonly List<Tile> _tiles=new();
        readonly List<MenuNavigationButton> _categories=new();
        readonly List<Entry> _visible=new();
        List<Entry> _entries=new();
        RectTransform _safe,_grid,_content,_detailContent;
        ScrollRect _scroll,_detailScroll;
        TextMeshProUGUI _title,_count,_name,_meta,_owner,_body,_empty,_hint,_notice,_action,_sortText,_member;
        ExemplarEquipmentIcon _art;
        ForgeSigil _gemArt;
        Image _grade,_memberPortrait;
        CombatHudPortrait _memberFace;
        GameObject _details;
        string _selectedKey,_detailKey,_layoutKey;
        int _filter,_sort,_columns;
        ShopSelectionArc _categoryArc;
        bool _picker;

        public static CollectionBrowser Create(Transform parent,bool picker=false)
        {
            var root=ExemplarUi.Rect(parent,picker?"EquipmentCollection":"InventoryCollection",0,0,1,1);
            var view=root.gameObject.AddComponent<CollectionBrowser>();view._picker=picker;view.Build();return view;
        }
        static TextMeshProUGUI Copy(Transform p,string name,string value,float x,float y,float r,float t,float size,Color color)
        {
            var text=ExemplarUi.Text(p,name,value,x,y,r,t,size,color);
            text.font=UiTheme.Font;text.fontSharedMaterial=UiTheme.Font.material;
            text.enableAutoSizing=false;text.raycastTarget=false;return text;
        }
        public static MenuNavigationButton Quiet(Transform p,string name,string value,float x,float y,float r,float t,Action click)
        {
            var surface=ExemplarUi.Panel(p,name,x,y,r,t,Color.clear);surface.raycastTarget=true;
            var b=surface.gameObject.AddComponent<MenuNavigationButton>();b.targetGraphic=surface;
            b.navigation=new Navigation{mode=Navigation.Mode.None};b.onClick.AddListener(()=>click?.Invoke());
            b.Caption=Copy(b.transform,"Label",value,.08f,.08f,.92f,.92f,30,UiTheme.TextHigh);
            b.Caption.alignment=TextAlignmentOptions.Center;
            return b;
        }
        void Build()
        {
            var background=ExemplarUi.Panel(transform,"Background",0,0,1,1,UiTheme.Panel);background.raycastTarget=true;
            RpgBackdrop.Create(transform);
            ExemplarUi.Panel(transform,"Veil",0,0,1,1,UiTheme.WithAlpha(UiTheme.Panel,.18f));
            _safe=ExemplarUi.Rect(transform,"SafeContent",0,0,1,1);
            ExemplarUi.Panel(_safe,"HeaderSurface",0,.887f,1,1,UiTheme.Header);
            _title=ExemplarUi.Text(_safe,"Title","",.025f,.895f,.65f,.985f,38,UiTheme.TextHigh);
            Quiet(_safe,"Tools",Loc.Get(pickerKey()),.76f,.883f,.985f,.998f,()=>Tools?.Invoke());
            ExemplarUi.Panel(_safe,"HeaderRule",.025f,.887f,.975f,.8885f,new Color(.46f,.50f,.48f,.4f));
            _count=Copy(_safe,"Count","",.14f,.813f,.60f,.874f,27,UiTheme.TextMid);
            var seam=ExemplarUi.Rect(_safe,"CategorySeam",.12f,.165f,.12f,.85f);
            seam.offsetMin=new Vector2(-47,0);seam.offsetMax=new Vector2(17,0);
            _categoryArc=seam.gameObject.AddComponent<ShopSelectionArc>();_categoryArc.raycastTarget=false;
            string[] labels=_picker?new[]{"UI_COLLECTION_ALL","UI_EX_GEAR_IN_BAG","UI_EX_GEAR_EQUIPPED","UI_COLLECTION_USABLE"}
                :new[]{"UI_COLLECTION_ALL","UI_COLLECTION_WEAPONS","UI_COLLECTION_ARMOR","UI_FORGE_GEMS","UI_COLLECTION_CARDS","UI_COLLECTION_MATERIALS"};
            string[] icons=_picker?new[]{"Equipment","Inventory","Exemplars","Attributes"}:new[]{"Inventory","Equipment","Regalia","Crystal","Cards","Stash"};
            for(int i=0;i<labels.Length;i++)
            {
                int index=i;float top=.85f-i*.116f;
                var b=Quiet(_safe,"Category_"+i,Loc.Get(labels[i]),.014f,top-.115f,.108f,top,()=>SelectCategory(index));
                ExemplarUi.Place(b.Caption.rectTransform,.01f,.01f,.99f,.36f);b.Caption.fontSize=30;
                b.Caption.enableAutoSizing=true;b.Caption.fontSizeMin=22;b.Caption.fontSizeMax=30;
                ExemplarUi.Icon(b.transform,icons[i],.32f,.40f,.68f,.91f,UiTheme.Accent);
                b.IsTab=true;b.CategoryIcon=b.GetComponentInChildren<HomeIcon>();

                b.SetActiveState(i==0);
                _categories.Add(b);
            }
            _grid=ExemplarUi.Rect(_safe,"CollectionGrid",.139f,.16f,.637f,.806f);
            _content=UiScroll.Create(_grid,out _scroll,false);_scroll.inertia=true;_scroll.decelerationRate=.12f;
            _empty=Copy(_grid,"Empty",Loc.Get("UI_COLLECTION_EMPTY"),.08f,.30f,.92f,.70f,32,UiTheme.TextMid);
            _empty.alignment=TextAlignmentOptions.Center;_empty.textWrappingMode=TextWrappingModes.Normal;
            ExemplarUi.Panel(_safe,"DetailRule",.66f,.16f,.661f,.85f,UiTheme.Line);
            var inspector=ExemplarUi.Rect(_safe,"Inspector",.684f,.16f,.978f,.87f);
            _hint=Copy(inspector,"SelectHint",Loc.Get("UI_COLLECTION_SELECT"),.05f,.28f,.95f,.68f,34,UiTheme.TextMid);
            _hint.alignment=TextAlignmentOptions.Center;_hint.textWrappingMode=TextWrappingModes.Normal;
            _details=ExemplarUi.Rect(inspector,"SelectedItem",0,0,1,1).gameObject;
            _name=ExemplarUi.Text(_details.transform,"Name","",0,.875f,1,1,36,UiTheme.Accent);
            _name.textWrappingMode=TextWrappingModes.Normal;_name.fontSizeMin=28;_name.fontSizeMax=36;
            _meta=Copy(_details.transform,"Meta","",0,.80f,1,.875f,25,UiTheme.TextMid);
            _meta.enableAutoSizing=true;_meta.fontSizeMin=21;_meta.fontSizeMax=25;
            var artwork=ExemplarUi.Rect(_details.transform,"Artwork",.23f,.515f,.77f,.795f);
            _art=ExemplarEquipmentIcon.Create(artwork,"Weapon");
            _gemArt=artwork.gameObject.AddComponent<ForgeSigil>();_gemArt.raycastTarget=false;_gemArt.color=UiTheme.Accent;
            _grade=ExemplarUi.Panel(_details.transform,"Rarity",0,.50f,1,.504f,UiTheme.Accent);
            _owner=Copy(_details.transform,"Owner","",0,.427f,1,.49f,25,UiTheme.TextMid);
            var detail=ExemplarUi.Rect(_details.transform,"DetailsScroll",0,0,1,.421f);
            _detailContent=UiScroll.Create(detail,out _detailScroll,true);_detailScroll.inertia=true;
            _body=Copy(_detailContent,"Description","",0,0,1,1,28,UiTheme.TextHigh);
            _body.richText=true;
            _body.alignment=TextAlignmentOptions.TopLeft;_body.textWrappingMode=TextWrappingModes.Normal;
            _body.rectTransform.anchorMin=new Vector2(0,1);_body.rectTransform.anchorMax=Vector2.one;
            _body.rectTransform.pivot=new Vector2(.5f,1);_body.lineSpacing=7;
            ExemplarUi.Panel(_safe,"FooterRule",.14f,.143f,.975f,.1445f,new Color(.46f,.50f,.48f,.35f));
            var sort=Quiet(_safe,"Sort","",.134f,.018f,.345f,.135f,()=>{_sort=(_sort+1)%3;Draw(true);});_sortText=sort.Caption;_sortText.fontSize=26;
            Quiet(_safe,"PreviousMember","‹",.36f,.018f,.414f,.135f,()=>PreviousMember?.Invoke());
            _memberPortrait=ExemplarUi.Panel(_safe,"MemberPortrait",.425f,.04f,.461f,.12f,Color.white);_memberPortrait.preserveAspect=true;
            _memberFace=ExemplarUi.Rect(_memberPortrait.transform,"Face",0,0,1,1).gameObject.AddComponent<CombatHudPortrait>();
            _memberFace.Crop=new Rect(.29f,.71f,.28f,.28f);_memberFace.raycastTarget=false;
            _member=Copy(_safe,"Member","",.469f,.025f,.586f,.135f,25,UiTheme.TextHigh);_member.enableAutoSizing=true;_member.fontSizeMin=20;_member.fontSizeMax=25;
            Quiet(_safe,"NextMember","›",.59f,.018f,.644f,.135f,()=>NextMember?.Invoke());
            PrimaryButton=ExemplarUi.Button(_safe,"Primary","",.69f,.018f,.977f,.135f,()=>Primary?.Invoke(),true);
            _action=PrimaryButton.GetComponentInChildren<TextMeshProUGUI>();_action.fontSize=32;_action.fontSizeMin=25;_action.fontSizeMax=32;
            _notice=Copy(_safe,"Notice","",.685f,.132f,.977f,.16f,19,UiTheme.Accent);
            _notice.overflowMode=TextOverflowModes.Ellipsis;
        }
        string pickerKey()=>_picker?"UI_COMMON_CANCEL":"UI_COLLECTION_TOOLS";
        public void SetHeader(string title,string count,string member,string unit,bool switchMember)
        {
            _title.text=title;_count.text=count;_member.text=member;
            _memberPortrait.sprite=ExemplarPortrait.Avatar(unit);_memberPortrait.enabled=_memberPortrait.sprite!=null;
            var illustration=ExemplarPortrait.Illustration(unit);_memberFace.SetTexture(illustration);_memberFace.enabled=illustration!=null;if(illustration!=null)_memberPortrait.enabled=false;
            _safe.Find("PreviousMember").gameObject.SetActive(switchMember);_safe.Find("NextMember").gameObject.SetActive(switchMember);
        }
        public void Bind(List<Entry> entries,string selectedKey)
        { _entries=entries;_selectedKey=selectedKey;Draw(false); }
        public void SelectCategory(int category)
        { _filter=category;Draw(true);if(_selectedKey!=null&&!_visible.Any(e=>e.Key==_selectedKey))Pick?.Invoke(null); }
        public void SelectEntry(int index) { if(index>=0&&index<_visible.Count)Pick?.Invoke(_visible[index]); }
        void Draw(bool reset)
        {
            _categoryArc.SelectNormalized((_filter*.116f+.0575f)/.685f);
            _visible.Clear();_visible.AddRange(_entries.Where(e=>Matches==null||Matches(_filter,e)));
            if(_sort==1)_visible.Sort((a,b)=>{int v=b.Item.grade_step.CompareTo(a.Item.grade_step);return v!=0?v:string.CompareOrdinal(a.Key,b.Key);});
            if(_sort==2)_visible.Sort((a,b)=>{int v=b.Item.level_req.CompareTo(a.Item.level_req);return v!=0?v:string.CompareOrdinal(a.Key,b.Key);});
            _sortText.text=Loc.Get(_sort==0?"UI_COLLECTION_ORDER":_sort==1?"UI_COLLECTION_GRADE":"UI_COLLECTION_LEVEL")+"  ↕";
            for(int i=0;i<_categories.Count;i++)_categories[i].SetActiveState(i==_filter);
            for(int i=0;i<_visible.Count;i++)
            {
                if(i==_tiles.Count)CreateTile(i);
                var tile=_tiles[i];var e=_visible[i];tile.Entry=e;tile.Root.gameObject.SetActive(true);
                bool selected=e.Key==_selectedKey;
                tile.Background.color=selected?new Color(.14f,.19f,.18f,.95f):new Color(.02f,.045f,.043f,.78f);
                tile.Border.enabled=selected;tile.Accent.color=ItemGradeSurface.Accent(e.Item,e.Gem);
                tile.Icon.SetItem(e.Item,e.Item.slot);tile.Icon.enabled=!e.Gem;tile.Gem.enabled=e.Gem;
                tile.Caption.text=e.Item.kind=="Weapon"||e.Item.kind=="Armor"?Loc.Fmt("UI_COMMON_LEVEL_SHORT",e.Item.level_req):"×"+e.Item.quantity;
                tile.New.text=e.IsNew?"•":"";
                tile.Portrait.sprite=e.Row?.OwnerId>0?ExemplarPortrait.Avatar(e.Row.OwnerUnitDef):null;tile.Portrait.enabled=tile.Portrait.sprite!=null;
            }
            for(int i=_visible.Count;i<_tiles.Count;i++)_tiles[i].Root.gameObject.SetActive(false);
            _empty.gameObject.SetActive(_visible.Count==0);LayoutGrid();
            if(reset){_scroll.StopMovement();_scroll.verticalNormalizedPosition=1;}
        }
        void CreateTile(int index)
        {
            var bg=ExemplarUi.Panel(_content,"Item_"+index,0,0,1,1,Color.white);bg.sprite=UiTheme.Fill(7);bg.type=Image.Type.Sliced;bg.raycastTarget=true;
            var tile=new Tile{Root=bg.rectTransform,Background=bg};
            tile.Button=bg.gameObject.AddComponent<Button>();tile.Button.targetGraphic=bg;tile.Button.navigation=new Navigation{mode=Navigation.Mode.None};
            tile.Button.onClick.AddListener(()=>Pick?.Invoke(tile.Entry));
            var icon=ExemplarUi.Rect(tile.Root,"Art",.12f,.24f,.88f,.94f);tile.Icon=ExemplarEquipmentIcon.Create(icon,"Weapon");
            tile.Gem=icon.gameObject.AddComponent<ForgeSigil>();tile.Gem.raycastTarget=false;tile.Gem.color=UiTheme.Accent;
            tile.Accent=ExemplarUi.Panel(tile.Root,"Grade",.06f,.205f,.94f,.221f,UiTheme.Accent);
            tile.Accent.rectTransform.anchorMin=new Vector2(.06f,.22f);tile.Accent.rectTransform.anchorMax=new Vector2(.94f,.22f);tile.Accent.rectTransform.sizeDelta=new Vector2(0,2);
            tile.Caption=Copy(tile.Root,"Level","",.08f,.01f,.92f,.205f,26,UiTheme.TextHigh);tile.Caption.alignment=TextAlignmentOptions.MidlineRight;
            tile.Caption.enableAutoSizing=false;tile.Caption.overflowMode=TextOverflowModes.Overflow;
            tile.New=Copy(tile.Root,"New","",.06f,.76f,.25f,.99f,30,UiTheme.Accent);
            tile.Portrait=ExemplarUi.Panel(tile.Root,"Owner",.025f,.74f,.27f,.985f,Color.white);tile.Portrait.preserveAspect=true;
            tile.Border=UiTheme.Border(tile.Root,UiTheme.Accent,6);tile.Border.raycastTarget=false;_tiles.Add(tile);
        }
        void LayoutGrid()
        {
            float width=_grid.rect.width;if(width<=1)return;
            var corners=new Vector3[4];_grid.GetWorldCorners(corners);float pixels=Vector3.Distance(corners[0],corners[3]);
            _columns=Mathf.Clamp(Mathf.FloorToInt(pixels/104),3,6);
            float cell=width/_columns;float height=cell*1.10f;
            for(int i=0;i<_visible.Count;i++)
            {
                var rt=_tiles[i].Root;rt.anchorMin=new Vector2(i%_columns/(float)_columns,1);rt.anchorMax=new Vector2((i%_columns+1)/(float)_columns,1);rt.pivot=new Vector2(.5f,1);
                rt.offsetMin=new Vector2(7,-(i/_columns+1)*height+10);rt.offsetMax=new Vector2(-7,-i/_columns*height);
                _tiles[i].Caption.fontSize=Mathf.Clamp(height*.18f,20,28);
            }
            UiScroll.Fit(_content,Mathf.CeilToInt(_visible.Count/(float)_columns)*height);
        }
        public void ShowDetails(Entry e,string description,ItemView worn,string owner,string action,bool enabled,string notice="")
        {
            _details.SetActive(e!=null);_hint.gameObject.SetActive(e==null);_action.text=action;PrimaryButton.interactable=enabled;_notice.text="";
            PrimaryButton.gameObject.SetActive(!string.IsNullOrEmpty(action));
            _hint.text=string.IsNullOrEmpty(notice)?Loc.Get("UI_COLLECTION_SELECT"):notice;
            if(e==null){_detailKey=null;return;}
            _name.text=GameText.ItemName(e.Item);_meta.text=GameText.GradeLabel(e.Item.grade_step)+" · "+Loc.GetOr("UI_SLOT_"+e.Item.slot,Loc.GetOr("UI_FLOAT_KIND_"+e.Item.kind,e.Item.kind));
            _art.SetItem(e.Item,e.Item.slot);_art.enabled=!e.Gem;_gemArt.enabled=e.Gem;_grade.color=ItemGradeSurface.Accent(e.Item,e.Gem);_owner.text=owner;
            string body=description??"";int first=body.IndexOf('\n');if(first>=0)body=body.Substring(first+1);int second=body.IndexOf('\n');if(second>=0)body=body.Substring(second+1);
            if(_picker&&(e.Item.kind=="Weapon"||e.Item.kind=="Armor"))
            {
                body=Loc.Get("UI_EX_GEAR_COMPARE_COLUMNS")+"\n"+Loc.Get("UI_EX_GEAR_CURRENT")+": "+(worn==null?Loc.Get("UI_REVIEW_EMPTY_SLOT"):GameText.ItemName(worn))+"\n"+Compare("UI_EXEMPLAR_ATK",worn?.graded_atk??0,e.Item.graded_atk)+Compare("UI_EXEMPLAR_DEF",worn?.graded_def??0,e.Item.graded_def)+"\n"+body+"\n\n"+Loc.Get("UI_EX_GEAR_ITEM_ONLY");
            }
            if(!enabled&&!string.IsNullOrEmpty(notice))body="<color=#D8BE81>"+notice+"</color>\n\n"+body;
            _body.text=body;FitDescription();
            if(_detailKey!=e.Key){_detailScroll.StopMovement();_detailScroll.verticalNormalizedPosition=1;}_detailKey=e.Key;
        }
        static string Compare(string key,int before,int after)
        {
            if(before==0&&after==0)return "";int delta=after-before;
            return Loc.Get(key)+"  "+before+" → "+after+"  <color="+(delta>=0?"#AFC9AB":"#D8A39B")+">("+(delta>0?"+":"")+delta+")</color>\n";
        }
        void FitDescription()
        {
            float width=_detailContent.rect.width;if(width<=1)return;
            float height=_body.GetPreferredValues(_body.text,width,10000).y+24;
            _body.rectTransform.sizeDelta=new Vector2(0,height);_body.rectTransform.anchoredPosition=Vector2.zero;UiScroll.Fit(_detailContent,height);
        }
        void LateUpdate()
        {
            // A fixed logical height keeps labels and touch targets consistent across parent canvases.
            var root=(RectTransform)transform;var parent=(RectTransform)transform.parent;
            float scale=parent.rect.height/900f;
            if(scale>0){root.anchorMin=root.anchorMax=new Vector2(.5f,.5f);root.pivot=new Vector2(.5f,.5f);root.sizeDelta=new Vector2(parent.rect.width/scale,900);root.anchoredPosition=Vector2.zero;root.localScale=new Vector3(scale,scale,1);}
            var safe=Screen.safeArea;_safe.anchorMin=new Vector2(safe.xMin/Screen.width,safe.yMin/Screen.height);_safe.anchorMax=new Vector2(safe.xMax/Screen.width,safe.yMax/Screen.height);
            string size=Screen.width+":"+Screen.height+":"+_grid.rect.width;
            if(size!=_layoutKey){_layoutKey=size;LayoutGrid();FitDescription();}
        }
    }
}
