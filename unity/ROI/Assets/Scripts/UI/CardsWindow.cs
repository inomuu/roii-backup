// Cards: browse first, inspect, then explicitly equip or return a card to the album.
// Capacity, live sockets, effects and original album/panel indices are owned by the sim.
using System;
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed partial class CardsWindow : MonoBehaviour
    {
        Roi.Sim.SimDriver _driver;
        RectTransform _body, _toolbar, _gridHost, _grid, _dossier, _factsHost, _facts;
        ScrollRect _gridScroll, _factsScroll;
        TMP_InputField _search;
        Button _filterButton;
        TextMeshProUGUI _summary, _note;
        readonly List<GameObject> _tiles=new(), _detail=new();
        SlotRow[] _rows=Array.Empty<SlotRow>();
        ItemView[] _owned=Array.Empty<ItemView>();
        readonly List<int> _visible=new();
        string _filter="", _query="", _target="Weapon", _cardKey="", _fingerprint="";
        int _albumIndex=-1, _panelIndex=-1, _tab;
        long _entityId;
        Vector2 _lastSize;
        float _leftWidth, _rightWidth, _height;
        bool _layout;
        static readonly Dictionary<string,Sprite> Thumbnails=new();
        static string T(string key)=>Loc.Get("UI_CARDS_"+key);
        static string Key(ItemView c)=>c==null?"":c.def_id+":"+c.star+":"+c.foil;

        public static CardsWindow Attach(RectTransform body,string footnote)
        {var w=body.gameObject.AddComponent<CardsWindow>();w.Build(body);return w;}
        void OnEnable()=>PartySelection.Changed+=OnPicked;
        void OnDisable()=>PartySelection.Changed-=OnPicked;
        void OnPicked(){_panelIndex=-1;_fingerprint="";if(isActiveAndEnabled)Refresh();}
        void Build(RectTransform body)
        {
            _body=body;_toolbar=Rect(body,"CardTools");
            _filterButton=Action(_toolbar,"ClassFilter",T("ALL"),false,OpenFilter);
            var box=UiTheme.Box(_toolbar,"CardSearch",UiTheme.Surface);
            UiTheme.Border(box.rectTransform,UiTheme.LineStrong);
            var area=Rect(box.transform,"TextArea");Fill(area);area.offsetMin=new Vector2(14,3);area.offsetMax=new Vector2(-44,-3);area.gameObject.AddComponent<RectMask2D>();
            var ph=UiTheme.Label(area,"Placeholder",T("SEARCH"),15,UiTheme.TextMid,TextAlignmentOptions.Left);Fill(ph.rectTransform);
            var tx=UiTheme.Label(area,"SearchText","",15,UiTheme.TextHigh,TextAlignmentOptions.Left);Fill(tx.rectTransform);
            _search=box.gameObject.AddComponent<TMP_InputField>();_search.textViewport=area;_search.textComponent=tx;_search.placeholder=ph;_search.targetGraphic=box;
            _search.lineType=TMP_InputField.LineType.SingleLine;_search.characterLimit=60;
            _search.onValueChanged.AddListener(v=>{_query=v.Trim();FilterChanged();});
            var clear=Action(box.transform,"ClearSearch","×",false,()=>_search.text="");
            var cr=(RectTransform)clear.transform;cr.anchorMin=cr.anchorMax=new Vector2(1,.5f);cr.pivot=new Vector2(1,.5f);cr.sizeDelta=new Vector2(38,38);cr.anchoredPosition=new Vector2(-3,0);
            _summary=Text(body,"CollectionSummary","",15,UiTheme.TextMid);
            _note=Text(body,"CardGuidance",T("BROWSE_HINT"),13,UiTheme.TextMid);
            _gridHost=Rect(body,"CardCollection");_grid=UiScroll.Create(_gridHost,out _gridScroll);
            var frame=AlbumCodexSurface.Create(body,"CardDossier",true);_dossier=frame.rectTransform;
            _factsHost=Rect(_dossier,"CardFacts");_facts=UiScroll.Create(_factsHost,out _factsScroll);
            Layout();
        }
        void LateUpdate(){if(_body!=null&&(_lastSize-_body.rect.size).sqrMagnitude>1)Layout();}
        void Layout()
        {
            if(_layout||_body==null)return;_layout=true;
            try{
                _lastSize=_body.rect.size;_height=Mathf.Max(400,_body.rect.height-32);
                float width=_body.rect.width-48;_rightWidth=Mathf.Clamp(width*.36f,320,410);_leftWidth=width-_rightWidth-24;
                Place(_toolbar,24,16,_leftWidth,44);
                float filterW=Mathf.Clamp(_leftWidth*.34f,155,210);
                Place((RectTransform)_filterButton.transform,0,0,filterW,44);
                Place((RectTransform)_search.transform,filterW+10,0,_leftWidth-filterW-10,44);
                Place(_summary.rectTransform,24,69,_leftWidth,26);
                Place(_gridHost,24,106,_leftWidth,_height-144);
                Place(_note.rectTransform,24,_height-17,_leftWidth,35);
                Place(_dossier,24+_leftWidth+24,16,_rightWidth,_height);
                Render();
            }finally{_layout=false;}
        }
        public void Refresh()
        {
            if(_driver==null)_driver=UnityEngine.Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var sim=_driver!=null?_driver.Session:null;if(sim==null){_summary.text=T("UNAVAILABLE");return;}
            var view=JsonUtility.FromJson<LeadView>(sim.ViewJson());var ids=new List<long>();
            if(view?.grid?.entities!=null)foreach(var e in view.grid.entities)if(e.team=="Player"&&!e.is_pet)ids.Add(e.id);
            _entityId=PartySelection.Resolve(ids);
            string slots=sim.QueryJson(_entityId>0?"card_slots:"+_entityId:"card_slots"), album=sim.QueryJson("card_album");
            string fp=slots+album+_entityId+Loc.Language;
            if(fp==_fingerprint)return;_fingerprint=fp;
            _rows=JsonUtility.FromJson<SlotList>("{\"rows\":"+slots+"}")?.rows??Array.Empty<SlotRow>();
            _owned=JsonUtility.FromJson<ItemList>("{\"items\":"+album+"}")?.items??Array.Empty<ItemView>();
            Render();
        }
        void Render()
        {
            if(_grid==null)return;
            _filterButton.GetComponentInChildren<TextMeshProUGUI>().text=(_filter==""?T("ALL"):Pretty(_filter))+"  ›";
            ((TMP_Text)_search.placeholder).text=T("SEARCH");_note.text=T("BROWSE_HINT");
            _visible.Clear();for(int i=0;i<_owned.Length;i++)if(Matches(_owned[i]))_visible.Add(i);
            _albumIndex=-1;bool reveal=false;
            if(_panelIndex>=0){var row=TargetRow();if(row?.cards==null||Array.Find(row.cards,c=>c.panel_idx==_panelIndex&&Key(c.item)==_cardKey)==null)_panelIndex=-1;}
            if(_panelIndex<0){foreach(int i in _visible)if(Key(_owned[i])==_cardKey){_albumIndex=i;break;}
                if(_albumIndex<0&&_visible.Count>0){_albumIndex=_visible[0];reveal=true;}
                if(_albumIndex>=0){var c=_owned[_albumIndex];_cardKey=Key(c);if(!Fits(c.card_slot,_target))_target=c.card_slot;}
                else _cardKey="";}
            BuildGrid();if(reveal)RevealSelected();BuildDetail();
        }
        bool Matches(ItemView card)
        {
            if(_filter!=""&&!Fits(card.card_slot,_filter))return false;
            if(_query=="")return true;
            string text=GameText.ItemName(card)+" "+Pretty(card.card_slot)+" "+string.Join(" ",card.foil?card.card_foil_lines??card.card_lines??Array.Empty<string>():card.card_lines??Array.Empty<string>());
            return text.IndexOf(_query,StringComparison.CurrentCultureIgnoreCase)>=0;
        }
        void FilterChanged(){_panelIndex=-1;Render();_gridScroll.verticalNormalizedPosition=1;RevealSelected();}
        void SetFilter(string cls){_filter=cls;FilterChanged();}
        void OpenFilter()
        {
            var options=new List<UiPicker.Option>{new(){label=T("ALL"),onPick=()=>SetFilter("")}};
            foreach(var row in _rows){string cls=row.@class;options.Add(new UiPicker.Option{label=Pretty(cls),onPick=()=>SetFilter(cls)});}
            UiPicker.Open(T("FILTER"),options);
        }
        void SelectCard(int index)
        {
            if(index<0||index>=_owned.Length)return;
            _cardKey=Key(_owned[index]);_albumIndex=index;_panelIndex=-1;
            if(!Fits(_owned[index].card_slot,_target))_target=_owned[index].card_slot;
            BuildGrid();RevealSelected();BuildDetail();UiPageTransition.Play(_factsHost);
        }
        void SelectPlaced(int panelIndex)
        {
            var row=TargetRow();var card=row?.cards==null?null:Array.Find(row.cards,c=>c.panel_idx==panelIndex);
            if(card==null)return;_panelIndex=panelIndex;_albumIndex=-1;_cardKey=Key(card.item);BuildGrid();BuildDetail();
        }
        SlotRow TargetRow()=>Array.Find(_rows,r=>r.@class==_target);
        void RevealSelected()
        {
            if(_albumIndex<0)return;
            // Keep the entire chosen tile visible when a row sits across the viewport edge.
            var frame=_tiles.Find(go=>go!=null&&go.GetComponent<AlbumCodexSurface>()?.Selected==true);
            if(frame==null)return;
            var rect=(RectTransform)frame.transform;
            float top=-rect.anchoredPosition.y,bottom=top+rect.rect.height;
            float offset=_grid.anchoredPosition.y,view=_gridScroll.viewport.rect.height;
            if(top<offset)offset=top;else if(bottom>offset+view)offset=bottom-view;
            float range=_grid.rect.height-view;
            if(range>0)_gridScroll.verticalNormalizedPosition=1-Mathf.Clamp01(offset/range);
        }
        ItemView SelectedCard()=>_panelIndex>=0?Array.Find(TargetRow()?.cards??Array.Empty<PanelCard>(),c=>c.panel_idx==_panelIndex)?.item:_albumIndex>=0&&_albumIndex<_owned.Length?_owned[_albumIndex]:null;
        void BuildGrid()
        {
            float scroll=_grid.rect.height>_gridHost.rect.height+1?_gridScroll.verticalNormalizedPosition:1;Clear(_tiles);
            _summary.text=Loc.Fmt("UI_CARDS_COUNT",_visible.Count,_owned.Length);
            float width=Mathf.Max(100,_leftWidth-16),gap=10;
            int cols=Mathf.Max(2,Mathf.FloorToInt((width+gap)/128));float cw=(width-gap*(cols-1))/cols,ch=154,y=0;
            foreach(var row in _rows)
            {
                var group=_visible.FindAll(i=>_owned[i].card_slot==row.@class);if(group.Count==0)continue;
                var title=Text(_grid,"Group_"+row.@class,Pretty(row.@class)+"  ·  "+group.Count,14,UiTheme.TextMid);
                Place(title.rectTransform,2,y,width,25);_tiles.Add(title.gameObject);y+=31;
                for(int n=0;n<group.Count;n++){
                    int index=group[n];var c=_owned[index];float x=n%cols*(cw+gap),cy=y+n/cols*(ch+gap);
                    var frame=AlbumCodexSurface.Create(_grid,"Card_"+c.def_id,true);frame.Selected=_panelIndex<0&&index==_albumIndex;frame.Insignia=frame.Selected;
                    Place(frame.rectTransform,x,cy,cw,ch);_tiles.Add(frame.gameObject);
                    var b=frame.gameObject.AddComponent<UiStateButton>();frame.raycastTarget=true;b.targetGraphic=frame;b.colors=UiTheme.ButtonColors;b.onClick.AddListener(()=>SelectCard(index));
                    Portrait(frame.transform,c,false,new Rect(15,14,cw-30,70));
                    var titleCard=Text(frame.transform,"CardName",GameText.ItemName(c),14,UiTheme.TextHigh);Place(titleCard.rectTransform,10,88,cw-20,39);titleCard.alignment=TextAlignmentOptions.Center;
                    var rule=UiTheme.Flat(frame.transform,"Rarity",RarityOf(c));rule.raycastTarget=false;Place(rule.rectTransform,13,132,cw-26,2);
                    var meta=Text(frame.transform,"CardLevel",c.foil?Loc.Get("UI_ALBUM_FOIL_LABEL"):Loc.Fmt("UI_COMMON_LEVEL_SHORT",c.star),11,UiTheme.TextMid);Place(meta.rectTransform,10,134,cw-48,19);
                    var qty=Text(frame.transform,"Copies","×"+c.quantity,12,UiTheme.TextHigh);Place(qty.rectTransform,cw-42,134,32,19);qty.alignment=TextAlignmentOptions.Right;
                }
                y+=Mathf.Ceil(group.Count/(float)cols)*(ch+gap)+8;
            }
            if(_visible.Count==0){
                var empty=AlbumCodexSurface.Create(_grid,"EmptyCollection");Place(empty.rectTransform,0,10,width,220);_tiles.Add(empty.gameObject);
                var heading=Text(empty.transform,"EmptyTitle",T(_owned.Length==0?"EMPTY":"NO_MATCH"),23,UiTheme.TextHigh);UiTheme.Display(heading);Place(heading.rectTransform,24,30,width-48,38);
                var hint=Text(empty.transform,"EmptyHint",T(_owned.Length==0?"EMPTY_HINT":"SEARCH_HINT"),15,UiTheme.TextMid);Place(hint.rectTransform,24,78,width-48,58);
                var action=Action(empty.transform,"EmptyAction",T(_owned.Length==0?"OPEN_ALBUM":"CLEAR_FILTERS"),true,()=>{if(_owned.Length==0)OpenAlbum();else{_filter="";_query="";_search.SetTextWithoutNotify("");FilterChanged();}});
                Place((RectTransform)action.transform,24,152,Mathf.Min(250,width-48),44);y=240;
            }
            UiScroll.Fit(_grid,Mathf.Max(y,_gridHost.rect.height));_gridScroll.verticalNormalizedPosition=scroll;
        }
        void BuildDetail()
        {
            Clear(_detail);ClearChildren(_facts);
            var root=Rect(_dossier,"SelectedCard");Fill(root);_detail.Add(root.gameObject);
            var card=SelectedCard();float w=_rightWidth-36,h=_height;
            float hero=Mathf.Clamp(h*.43f,210,240),tabsY=hero+8,footerY=h-136;
            Place(_factsHost,18,tabsY+44,w,Mathf.Max(40,footerY-tabsY-54));
            if(card==null){
                var prompt=Text(root,"SelectCard",Loc.Get("UI_ALBUM_SELECT_CARD"),23,UiTheme.TextHigh);UiTheme.Display(prompt);Place(prompt.rectTransform,24,30,w-12,76);
                var hint=Text(root,"SelectHint",T("DETAIL_HINT"),15,UiTheme.TextMid);Place(hint.rectTransform,24,122,w-12,75);
                BuildSockets(root,footerY,w,null);return;
            }
            var name=Text(root,"SelectedName",GameText.ItemName(card),25,UiTheme.Accent);UiTheme.Display(name);Place(name.rectTransform,20,12,w-4,36);
            string meta=Pretty(card.card_slot)+"  ·  "+Loc.Fmt("UI_COMMON_LEVEL_SHORT",card.star);
            if(card.foil)meta+="  ·  "+Loc.Get("UI_ALBUM_FOIL_LABEL");
            var sub=Text(root,"SelectedMeta",meta,13,UiTheme.TextMid);Place(sub.rectTransform,20,50,w-4,23);
            float artH=hero-83,artW=Mathf.Min(210,w-40);
            var artFrame=AlbumCodexSurface.Create(root,"CardPortraitFrame",true);Place(artFrame.rectTransform,(_rightWidth-artW)/2,79,artW,artH);artFrame.Insignia=true;
            Portrait(artFrame.transform,card,true,new Rect(24,9,artW-48,artH-20));
            for(int i=0;i<2;i++){int tab=i;var b=Action(root,i==0?"EffectsTab":"BonusTab",T(i==0?"EFFECTS":"COLLECTION_BONUS"),i==_tab,()=>{_tab=tab;BuildDetail();UiPageTransition.Play(_factsHost);});Place((RectTransform)b.transform,18+i*(w+8)/2,tabsY,(w-8)/2,36);}
            float y=0;
            if(_tab==0){
                var lines=card.foil&&card.card_foil_lines?.Length>0?card.card_foil_lines:card.card_lines;
                if(lines==null||lines.Length==0)lines=new[]{Loc.Get("UI_CARD_NO_EFFECT")};
                foreach(string line in lines)y=Fact("Effect",line,y,19,UiTheme.TextHigh);

            }else{
                y=Fact("BindScope",Loc.Get("UI_ALBUM_BIND_SCOPE"),y,13,UiTheme.TextMid);
                var sim=_driver?.Session;
                var bind=_albumIndex>=0&&sim!=null?JsonUtility.FromJson<BindInfo>(sim.QueryJson("bind_info:"+_albumIndex)):null;
                if(card.foil)y=Fact("NoBind",Loc.Get("UI_ALBUM_FOIL_NO_BIND"),y+6,15,UiTheme.TextMid);
                else if(bind!=null&&!string.IsNullOrEmpty(bind.effect)){
                    y=Fact("BindEffect",bind.effect,y+6,19,UiTheme.TextHigh);
                    y=Fact("BindProgress",Loc.Fmt("UI_CARDS_BIND_PROGRESS",bind.done,bind.cap),y+4,13,UiTheme.TextMid);
                }else if(card.bind_lines?.Length>0){foreach(var line in card.bind_lines)y=Fact("BindEffect",line,y+6,19,UiTheme.TextHigh);}
                else y=Fact("NoBind",Loc.Get("UI_ALBUM_NO_BIND"),y+6,15,UiTheme.TextMid);
                y=Fact("BindHint",T("BIND_HINT"),y+8,13,UiTheme.TextMid);
            }
            UiScroll.Fit(_facts,Mathf.Max(y,_factsHost.rect.height));_factsScroll.verticalNormalizedPosition=1;
            BuildSockets(root,footerY,w,card);
            _factsHost.SetAsLastSibling();
        }
        float Fact(string name,string value,float y,float size,Color color)
        {
            var t=Text(_facts,name,value,size,color);float w=Mathf.Max(100,_rightWidth-52),height=Mathf.Max(24,t.GetPreferredValues(value,w,0).y+4);
            Place(t.rectTransform,0,y,w,height);return y+height;
        }
        void BuildSockets(RectTransform root,float y,float w,ItemView card)
        {
            var row=TargetRow();var cards=row?.cards??Array.Empty<PanelCard>();int capacity=row?.cap??0,sockets=row?.sockets??0;
            var line=UiTheme.Flat(root,"SocketDivider",UiTheme.Line);line.raycastTarget=false;Place(line.rectTransform,18,y-6,w,1);
            float targetW=Mathf.Min(140,w*.4f);
            var target=Action(root,"TargetPanel",Pretty(_target)+"  ›\n"+Loc.Fmt("UI_CARDS_PLACED",cards.Length,capacity),false,OpenTarget);
            target.GetComponentInChildren<TextMeshProUGUI>().fontSize=12;
            Place((RectTransform)target.transform,18,y,targetW,44);
            float sw=capacity>0?Mathf.Min(44,(w-targetW-10-6*(capacity-1))/capacity):44;
            for(int i=0;i<capacity;i++){
                int pi=i<cards.Length?cards[i].panel_idx:-1;
                var slot=Action(root,"Socket_"+i,i<cards.Length?"":(i+1).ToString(),pi==_panelIndex&&pi>=0,()=>{if(pi>=0)SelectPlaced(pi);});
                Place((RectTransform)slot.transform,18+targetW+10+i*(sw+6),y,sw,44);slot.interactable=pi>=0;
                if(pi>=0){Portrait(slot.transform,cards[i].item,false,new Rect(6,5,sw-12,32));
                    var status=UiTheme.Flat(slot.transform,"ActiveMarker",i<sockets?UiTheme.Accent:UiTheme.TextLow);status.raycastTarget=false;Place(status.rectTransform,9,39,sw-18,2);}
            }
            bool full=capacity<=cards.Length;bool inactive=_panelIndex>=0?Array.FindIndex(cards,c=>c.panel_idx==_panelIndex)>=sockets:cards.Length>=sockets;
            string state=T(sockets==0?"NO_SOCKETS":inactive?"NEXT_INACTIVE":"SOCKETS_HINT");
            if(_panelIndex>=0)state=T(inactive?"PLACED_INACTIVE":"PLACED_ACTIVE");
            else if(full&&card!=null)state=T("FULL_HINT");
            var hint=Text(root,"SocketHint",state,12,UiTheme.TextMid);Place(hint.rectTransform,18,y+49,w,34);
            var equip=Action(root,"EquipAction",T(_panelIndex>=0?"REMOVE":full?"FULL":"EQUIP"),true,Submit);Place((RectTransform)equip.transform,18,y+88,w*.59f,40);
            equip.interactable=card!=null&&_entityId>0&&(_panelIndex>=0||!full&&Fits(card.card_slot,_target));
            var album=Action(root,"OpenAlbum",T("OPEN_ALBUM"),false,OpenAlbum);Place((RectTransform)album.transform,18+w*.59f+8,y+88,w*.41f-8,40);
        }
        void OpenTarget()
        {
            var card=SelectedCard();var options=new List<UiPicker.Option>();
            foreach(var row in _rows){string cls=row.@class;
                // With a card selected, only compatible equipment can be a destination.
                if(card!=null&&_panelIndex<0&&!Fits(card.card_slot,cls))continue;
                options.Add(new UiPicker.Option{label=Pretty(cls),detail=Loc.Fmt("UI_CARDS_PLACED",row.cards?.Length??0,row.cap),onPick=()=>{if(_panelIndex>=0){_filter=cls;_cardKey="";}_target=cls;_panelIndex=-1;Render();}});
            }
            UiPicker.Open(T("TARGET"),options);
        }
        void Submit()
        {
            string key=_cardKey,target=_target;long who=_entityId;int panel=_panelIndex;
            _fingerprint="";Refresh();
            if(key!=_cardKey||who!=_entityId||target!=_target||panel!=_panelIndex)return;
            var sim=_driver?.Session;var row=TargetRow();if(sim==null||row==null)return;
            string input;
            if(panel>=0)input=$"{{\"UnequipCard\":{{\"entity_id\":{who},\"panel_idx\":{panel}}}}}";
            else{
                if(_albumIndex<0||!Fits(_owned[_albumIndex].card_slot,target)||(row.cards?.Length??0)>=row.cap)return;
                input=$"{{\"EquipCard\":{{\"entity_id\":{who},\"album_idx\":{_albumIndex},\"target\":\"{target}\"}}}}";
            }
            sim.ApplyInput(input);_panelIndex=-1;_fingerprint="";Refresh();
            if(panel<0){var placed=TargetRow()?.cards;if(placed?.Length>0&&Key(placed[placed.Length-1].item)==key)SelectPlaced(placed[placed.Length-1].panel_idx);}
        }
        void OpenAlbum(){var bar=UnityEngine.Object.FindFirstObjectByType<HudBar>();if(bar!=null){bar.CloseAll();bar.Toggle("Album");}}
        static bool IsAccessory(string cls)=>cls=="Necklace"||cls=="Ring"||cls=="Earring";
        static bool Fits(string cls,string panel)=>!string.IsNullOrEmpty(cls)&&(cls==panel||IsAccessory(cls)&&IsAccessory(panel));
        static string Pretty(string cls)=>Loc.GetOr("UI_CARDS_CLASS_"+(cls??"").ToUpperInvariant(),cls??"");
        static RectTransform Rect(Transform parent,string name){var r=new GameObject(name,typeof(RectTransform)).GetComponent<RectTransform>();r.SetParent(parent,false);return r;}
        static void Place(RectTransform r,float x,float y,float w,float h){r.anchorMin=r.anchorMax=new Vector2(0,1);r.pivot=new Vector2(0,1);r.anchoredPosition=new Vector2(x,-y);r.sizeDelta=new Vector2(w,h);}
        static void Fill(RectTransform r){r.anchorMin=Vector2.zero;r.anchorMax=Vector2.one;r.offsetMin=r.offsetMax=Vector2.zero;}
        static TextMeshProUGUI Text(Transform p,string name,string value,float size,Color color){var t=UiTheme.Label(p,name,value,size,color,TextAlignmentOptions.Left);t.textWrappingMode=TextWrappingModes.Normal;t.raycastTarget=false;return t;}
        static Button Action(Transform p,string name,string label,bool primary,Action callback)
        {
            var frame=AlbumCodexSurface.Create(p,name);frame.Primary=primary;frame.Selected=primary;frame.raycastTarget=true;
            var b=frame.gameObject.AddComponent<UiStateButton>();b.targetGraphic=frame;b.colors=UiTheme.ButtonColors;b.onClick.AddListener(()=>callback());
            var t=Text(frame.transform,"Label",label,15,primary?UiTheme.Accent:UiTheme.TextHigh);Fill(t.rectTransform);t.rectTransform.offsetMin=new Vector2(10,2);t.rectTransform.offsetMax=new Vector2(-10,-2);t.alignment=TextAlignmentOptions.Center;
            return b;
        }
        static void Clear(List<GameObject> objects){foreach(var go in objects)if(go!=null){go.SetActive(false);Destroy(go);}objects.Clear();}
        static void ClearChildren(RectTransform parent){for(int i=parent.childCount-1;i>=0;i--){var go=parent.GetChild(i).gameObject;go.SetActive(false);Destroy(go);}}
        static void Portrait(Transform parent,ItemView card,bool animated,Rect area)
        {
            var frames=CardArt.Idle(card);if(frames==null||frames.Length==0)return;
            var r=Rect(parent,"MonsterPortrait");Place(r,area.x,area.y,area.width,area.height);var image=r.gameObject.AddComponent<Image>();image.raycastTarget=false;image.preserveAspect=true;
            if(animated){r.gameObject.AddComponent<MonsterPortraitIdle>().Initialize(image,frames,Mathf.RoundToInt(CardArt.IdleFpsFor(card)));return;}
            if(!Thumbnails.TryGetValue(card.def_id,out var sprite)){
                sprite=frames[0];var bounds=sprite.rect;
                if(sprite.texture.isReadable){var pixels=sprite.texture.GetPixels((int)bounds.x,(int)bounds.y,(int)bounds.width,(int)bounds.height);int x0=(int)bounds.width,y0=(int)bounds.height,x1=-1,y1=-1;
                    for(int y=0;y<(int)bounds.height;y++)for(int x=0;x<(int)bounds.width;x++)if(pixels[y*(int)bounds.width+x].a>.08f){x0=Mathf.Min(x0,x);y0=Mathf.Min(y0,y);x1=Mathf.Max(x1,x);y1=Mathf.Max(y1,y);}
                    if(x1>=x0)sprite=Sprite.Create(sprite.texture,new Rect(bounds.x+x0,bounds.y+y0,x1-x0+1,y1-y0+1),new Vector2(.5f,.5f),100,0,SpriteMeshType.FullRect);
                }Thumbnails[card.def_id]=sprite;
            }image.sprite=sprite;
        }
        [Serializable] class SlotList{public SlotRow[] rows;}
        [Serializable] class SlotRow{public string @class;public int sockets,cap;public PanelCard[] cards;}
        [Serializable] class PanelCard{public int panel_idx;public ItemView item;}
        [Serializable] class LeadView{public LeadGrid grid;}
        [Serializable] class LeadGrid{public LeadEntity[] entities;}
        [Serializable] class LeadEntity{public long id;public string team;public bool is_pet;}
        [Serializable] class BindInfo{public string effect;public int done,cap;}
    }
}
