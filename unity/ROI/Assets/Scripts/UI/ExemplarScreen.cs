using System;
using System.Collections.Generic;
using System.Text;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Inspection is separate from the combat roster. Only existing management widgets send inputs.
    public sealed class ExemplarScreen : MonoBehaviour
    {
        public long InspectedId { get; private set; }
        public string CurrentPage { get; private set; } = "Attributes";
        readonly Dictionary<string,Button> _nav=new();
        readonly Dictionary<long,Graphic> _rosterFaces=new();
        readonly Dictionary<long,RectTransform> _rosterItems=new();
        readonly Dictionary<string,(GameObject go,Action refresh)> _pages=new();
        Roi.Sim.SimDriver _driver;
        ExemplarReadModel.Snapshot _snapshot;
        ExemplarOverview _overview;
        ExemplarPortrait _portrait;
        RectTransform _roster,_rosterHost,_rosterViewport,_legacyHost,_legacyBody;
        GameObject _art,_statsShade,_rosterShade,_rosterRule;
        ScrollRect _rosterScroll;
        Button _rosterPrevious,_rosterNext;
        ExemplarDockWheel _dockWheel;
        bool _dockLayout;
        float _dockCardWidth;
        const float DockGap=10f;
        Vector2 _rosterLastSize;
        TextMeshProUGUI _pageTitle,_count,_artNote,_pageScope;
        Action _close;
        ShopSelectionArc _navigationArc;
        string _rosterFingerprint="",_artClass="";
        float _nextRefresh;
        const float RosterLeft=.921f,RosterRight=.992f;
        const float RosterRowHeight=104f,RosterRowStep=114f;
        static readonly string[] PageIds={"Attributes","Regalia","Equipment","Skills","Apex","Resonance"};
        public static ExemplarScreen Create(Transform parent,Action close)
        {
            var rt=ExemplarUi.Rect(parent,"ExemplarScreen",0,0,1,1);
            var screen=rt.gameObject.AddComponent<ExemplarScreen>();screen._close=close;screen.Build();return screen;
        }
        void Build()
        {
            var bg=ExemplarUi.Panel(transform,"Background",0,0,1,1,UiTheme.Panel);bg.raycastTarget=true;
            RpgBackdrop.Create(transform);
            var art=ExemplarUi.Rect(transform,"CharacterPresentation",0,0,1,1);_art=art.gameObject;
            _portrait=ExemplarPortrait.Create(art);
            ExemplarUi.Panel(transform,"NavigationShade",0,0,.089f,1,UiTheme.WithAlpha(UiTheme.Panel,.35f));
            _statsShade=ExemplarUi.Panel(transform,"StatsShade",.635f,.16f,.975f,.89f,Color.clear).gameObject;GameMenuSurface.Add(_statsShade.transform);
            var seam=ExemplarUi.Rect(transform,"NavigationSeam",.089f,0,.089f,.902f);
            seam.offsetMin=new Vector2(-47,0);seam.offsetMax=new Vector2(17,0);
            _navigationArc=seam.gameObject.AddComponent<ShopSelectionArc>();
            _navigationArc.raycastTarget=false;
            _rosterShade=ExemplarUi.Panel(transform,"RosterShade",RosterLeft,.13f,RosterRight,.88f,UiTheme.WithAlpha(UiTheme.Surface,.62f)).gameObject;
            _rosterRule=ExemplarUi.Panel(transform,"RosterRule",RosterLeft,.887f,RosterRight,.889f,ExemplarUi.Line).gameObject;
            ExemplarUi.Panel(transform,"HeaderSurface",0,.902f,1,1,UiTheme.Header);
            ExemplarUi.Panel(transform,"HeaderRule",0,.901f,1,.902f,UiTheme.Line);
            var back=CollectionBrowser.Quiet(transform,"Back","\u2039",.0445f,.942f,.0445f,.942f,Back);
            ((RectTransform)back.transform).sizeDelta=new Vector2(56,56);
            var backLabel=back.GetComponentInChildren<TextMeshProUGUI>();backLabel.fontSize=34;backLabel.fontSizeMax=34;
            backLabel.fontSizeMin=34;backLabel.enableAutoSizing=false;backLabel.alignment=TextAlignmentOptions.Center;
            ExemplarUi.Place(backLabel.rectTransform,0,0,1,1);
            _pageTitle=ExemplarUi.Text(transform,"PageTitle",Loc.Get("UI_HOME_EXEMPLARS"),.106f,.919f,.68f,.972f,28,ExemplarUi.White);
            _overview=ExemplarOverview.Create(transform,SelectPage);
            for(int i=0;i<PageIds.Length;i++)
            {
                string id=PageIds[i];float top=.86f-i*.125f;
                var b=CollectionBrowser.Quiet(transform,"Nav_"+id,PageName(id),.0083f,top-.101f,.0806f,top,()=>SelectPage(id));
                var navLabel=b.GetComponentInChildren<TextMeshProUGUI>();navLabel.fontSize=16;navLabel.fontSizeMax=16;
                navLabel.enableAutoSizing=true;navLabel.fontSizeMin=13;navLabel.alignment=TextAlignmentOptions.Center;
                ExemplarUi.Place(navLabel.rectTransform,.11f,.13f,.89f,.36f);
                string icon=id=="Attributes"?"Exemplars":id;
                ExemplarUi.Icon(b.transform,icon,.33f,.48f,.67f,.84f,ExemplarUi.Muted);
                b.IsTab=true;b.CategoryIcon=b.GetComponentInChildren<HomeIcon>();b.SetActiveState(i==0);_nav[id]=b;
            }
            _rosterHost=ExemplarUi.Rect(transform,"Roster",RosterLeft,.13f,RosterRight,.88f);
            _roster=UiScroll.Create(_rosterHost,out _rosterScroll);_rosterScroll.horizontal=false;
            _rosterViewport=_rosterScroll.viewport;
            _dockWheel=_rosterViewport.gameObject.AddComponent<ExemplarDockWheel>();_dockWheel.enabled=false;
            _dockWheel.Scroll=delta=>SetDockOffset(-_roster.anchoredPosition.x+delta*80);
            _rosterScroll.onValueChanged.AddListener(_=>{if(_dockLayout)UpdateDockNavigation();});
            _rosterPrevious=ExemplarUi.Button(transform,"RosterPrevious","\u2039",.156f,.035f,.190f,.105f,()=>ShiftRoster(-1));
            _rosterNext=ExemplarUi.Button(transform,"RosterNext","\u203a",.846f,.035f,.880f,.105f,()=>ShiftRoster(1));
            foreach(var arrow in new[]{_rosterPrevious,_rosterNext})
            {
                arrow.GetComponentInChildren<GameMenuSurface>().enabled=false;
                var label=arrow.GetComponentInChildren<TextMeshProUGUI>();label.fontSizeMax=44;label.fontSize=44;
            }
            _rosterPrevious.gameObject.SetActive(false);_rosterNext.gameObject.SetActive(false);
            _count=ExemplarUi.Text(transform,"OwnedCount","",RosterLeft,.072f,RosterRight,.114f,17,ExemplarUi.Muted,TextAlignmentOptions.Center);
            _artNote=ExemplarUi.Text(transform,"ArtPreview",Loc.Get("UI_EXEMPLAR_ART_PREVIEW"),.49f,.044f,.85f,.087f,16,ExemplarUi.Muted,TextAlignmentOptions.Center);
            _legacyHost=ExemplarUi.Rect(transform,"ManagementPage",.085f,.10f,.884f,.89f);
            ExemplarUi.Panel(_legacyHost,"PageShade",0,0,1,1,Color.clear);
            _pageScope=ExemplarUi.Text(_legacyHost,"Scope","",.025f,.926f,.975f,.981f,23,ExemplarUi.Gold);
            _legacyBody=ExemplarUi.Rect(_legacyHost,"Widgets",.02f,.02f,.98f,.98f);
            _pageScope.gameObject.SetActive(false);
            _legacyHost.gameObject.SetActive(false);_statsShade.SetActive(false);PaintNavigation();
            _portrait.SetFraming(.61f,.39f,overview:true);
        }
        void OnEnable(){_nextRefresh=0;}
        void Update()
        {
            if(Time.unscaledTime<_nextRefresh)return;
            _nextRefresh=Time.unscaledTime+1f;Refresh();
        }
        public void Refresh()
        {
            if(_driver==null)_driver=UnityEngine.Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            _snapshot=ExemplarReadModel.Fetch(_driver);
            if(_snapshot.Find(InspectedId)==null)InspectedId=_snapshot.Members.Count>0?_snapshot.Members[0].Id:0;
            var fp=new StringBuilder();foreach(var m in _snapshot.Members)fp.Append(m.Id).Append(':').Append(m.UnitDef).Append(':').Append(m.Level).Append(':').Append(m.Benched).Append('|');
            if(fp.ToString()!=_rosterFingerprint){_rosterFingerprint=fp.ToString();BuildRoster();}
            var current=_snapshot.Find(InspectedId);
            if((current==null||current.Benched||current.Stats==null)&&NeedsDeployed(CurrentPage))SelectPage("Attributes");
            // A retained page may reopen after Combat changed the shared legacy-widget selection.
            if(current!=null&&!current.Benched&&NeedsDeployed(CurrentPage))PartySelection.Select(current.Id);
            ShowMember(current);
            if(_pages.TryGetValue(CurrentPage,out var page))page.refresh?.Invoke();
        }
        void BuildRoster()
        {
            foreach(Transform child in _roster){child.gameObject.SetActive(false);Destroy(child.gameObject);}_rosterFaces.Clear();_rosterItems.Clear();
            int index=0;
            foreach(var m in _snapshot.Members)
            {
                long id=m.Id;var go=ExemplarUi.Rect(_roster,"Character_"+id,0,1,1,1);
                go.pivot=new Vector2(.5f,1);go.sizeDelta=new Vector2(-8,RosterRowHeight);go.anchoredPosition=new Vector2(0,-index*RosterRowStep);_rosterItems[id]=go;index++;
                var im=go.gameObject.AddComponent<Image>();im.color=ExemplarUi.Card;
                var button=go.gameObject.AddComponent<Button>();button.targetGraphic=im;button.navigation=new Navigation{mode=Navigation.Mode.None};button.onClick.AddListener(()=>Inspect(id));
                var portraitRect=ExemplarUi.Rect(go,"Portrait",.14f,.40f,.86f,.96f);
                var maskGraphic=portraitRect.gameObject.AddComponent<RosterPortraitMaskGraphic>();maskGraphic.raycastTarget=false;maskGraphic.color=new Color(.035f,.055f,.065f,.98f);
                var mask=portraitRect.gameObject.AddComponent<Mask>();mask.showMaskGraphic=true;
                var faceRect=ExemplarUi.Rect(portraitRect,"Face",0,0,1,1);
                var illustration=ExemplarPortrait.Illustration(m.UnitDef);
                if(illustration!=null) {
                    var face=faceRect.gameObject.AddComponent<CombatHudPortrait>();
                    face.SetTexture(illustration);face.raycastTarget=false;
                } else {
                    var pic=faceRect.gameObject.AddComponent<Image>();pic.raycastTarget=false;pic.preserveAspect=true;pic.sprite=ExemplarPortrait.Avatar(m.UnitDef);
                    pic.enabled=pic.sprite!=null;
                    if(pic.sprite==null){
                        var name=GameText.UnitName(m.UnitDef);
                        ExemplarUi.Text(faceRect,"Monogram",name.Substring(0,Mathf.Min(3,name.Length)).ToUpperInvariant(),.06f,.10f,.94f,.90f,19,ExemplarUi.Gold,TextAlignmentOptions.Center);
                    }
                }
                var ringRect=ExemplarUi.Rect(go,"SelectionRing",.14f,.40f,.86f,.96f);
                var ring=ringRect.gameObject.AddComponent<RosterMedallionRing>();ring.raycastTarget=false;ring.color=UiTheme.LineStrong;_rosterFaces[id]=ring;
                button.targetGraphic=ring;
                ExemplarDockCard.Create(go,m.UnitDef);
                ExemplarUi.Text(go,"Name",GameText.UnitName(m.UnitDef),.02f,.14f,.98f,.35f,18,ExemplarUi.White,TextAlignmentOptions.Center);
                ExemplarUi.Text(go,"Level",Loc.Get("UI_EXEMPLAR_LEVEL")+" "+m.Level,.02f,.005f,.98f,.18f,16,ExemplarUi.Muted,TextAlignmentOptions.Center);
                if(m.Benched)ExemplarUi.Text(go,"Reserve",Loc.Get("UI_EXEMPLAR_RESERVE"),.02f,.84f,.98f,1,12,ExemplarUi.Muted,TextAlignmentOptions.TopRight);
            }
            UiScroll.Fit(_roster,index*RosterRowStep);_count.text=Loc.Get("UI_EXEMPLAR_OWNED")+"  "+_snapshot.Members.Count;
            ApplyRosterLayout(UsesBottomRoster(CurrentPage));
        }
        public void Inspect(long id)
        {
            var member=_snapshot?.Find(id);if(member==null)return;
            InspectedId=id;
            if(member.Benched&&NeedsDeployed(CurrentPage))SelectPage("Attributes");
            if(!member.Benched&&CurrentPage!="Attributes")PartySelection.Select(id);
            ShowMember(member);
            if(_pages.TryGetValue(CurrentPage,out var page))page.refresh?.Invoke();
            RevealInspectedRosterMember();
        }
        void ShowMember(ExemplarReadModel.Member member)
        {
            _overview.Show(member);
            foreach(var kv in _rosterFaces)kv.Value.color=kv.Key==InspectedId?UiTheme.Accent:UiTheme.LineStrong;
            foreach(var kv in _rosterItems)kv.Value.Find("DockFrame").GetComponent<ExemplarDockCard>().Select(kv.Key==InspectedId);
            foreach(var kv in _nav)kv.Value.interactable=!NeedsDeployed(kv.Key)||(member!=null&&!member.Benched&&member.Stats!=null);
            string classId=member?.UnitDef??"";if(classId!=_artClass){_artClass=classId;_portrait.SetCharacter(classId);}
            _pageTitle.text=Loc.Get("UI_HOME_EXEMPLARS")+"   /   "+PageName(CurrentPage);
            _pageScope.text=member==null?"":GameText.UnitName(member.UnitDef);
            _artNote.gameObject.SetActive(false);
        }
        public void SelectPage(string id)
        {
            if(!_nav.ContainsKey(id))return;
            var member=_snapshot?.Find(InspectedId);
            if(NeedsDeployed(id)&&(member==null||member.Benched||member.Stats==null))return;
            bool changed=CurrentPage!=id;
            CurrentPage=id;
            ApplyRosterLayout(UsesBottomRoster(id));
            foreach(var page in _pages.Values)page.go.SetActive(false);
            bool overview=id=="Attributes";_overview.gameObject.SetActive(overview);_statsShade.SetActive(false);_art.SetActive(id!="Skills");_artNote.gameObject.SetActive(overview);
            _legacyHost.gameObject.SetActive(!overview);
            bool centerPortrait=id=="Equipment"||id=="Regalia"||id=="Resonance";
            bool fitBetweenPanels=id=="Regalia"||id=="Resonance";
            if(id=="Apex")ExemplarUi.Place(_legacyHost,.65f,.10f,.975f,.89f);
            else if(id=="Skills")ExemplarUi.Place(_legacyHost,.085f,.16f,.99f,.89f);
            else if(fitBetweenPanels)ExemplarUi.Place(_legacyHost,.085f,.16f,.99f,.89f);
            else if(centerPortrait)ExemplarUi.Place(_legacyHost,.085f,.16f,.94f,.89f);
            else ExemplarUi.Place(_legacyHost,.085f,.10f,.884f,.89f);
            _portrait.SetFraming(overview?.61f:.60f,overview?.39f:.51f,id=="Apex",centerPortrait,
                fitBetweenPanels,overview);
            if(!overview)
            {
                if(member!=null&&!member.Benched)PartySelection.Select(member.Id);
                if(!_pages.ContainsKey(id))BuildPage(id);
                var page=_pages[id];page.go.SetActive(true);page.refresh?.Invoke();
            }
            ShowMember(member);PaintNavigation();
            if(changed)UiPageTransition.Play(overview?(RectTransform)_overview.transform:_legacyBody);
        }
        static bool UsesBottomRoster(string page)=>page=="Apex"||page=="Equipment"||page=="Regalia"||page=="Resonance"||page=="Skills";
        void ApplyRosterLayout(bool dock)
        {
            if(_rosterHost==null||_rosterViewport==null)return;
            float previousOffset=dock&&_dockLayout?-_roster.anchoredPosition.x:0;
            bool entering=dock&&!_dockLayout;_dockLayout=dock;
            _rosterScroll.StopMovement();_rosterScroll.enabled=true;
            _rosterScroll.horizontal=dock;_rosterScroll.vertical=!dock;
            _dockWheel.enabled=dock;
            _rosterScroll.scrollSensitivity=dock?64:28;
            _rosterShade.SetActive(false);_rosterRule.SetActive(false);
            ExemplarUi.Place(_rosterHost,dock?.19f:RosterLeft,dock?.025f:.13f,dock?.835f:RosterRight,dock?.132f:.88f);
            _rosterViewport.offsetMin=Vector2.zero;_rosterViewport.offsetMax=dock?Vector2.zero:new Vector2(-14,0);
            var track=_rosterHost.Find("ScrollTrack");if(track!=null)track.gameObject.SetActive(!dock);
            _count.rectTransform.SetParent(transform,false);
            _count.text=Loc.Get("UI_EXEMPLAR_OWNED")+"  "+_rosterItems.Count;
            _count.fontSizeMax=dock?15:17;_count.fontSizeMin=12;
            _count.alignment=dock?TextAlignmentOptions.Left:TextAlignmentOptions.Center;
            ExemplarUi.Place(_count.rectTransform,dock?.104f:RosterLeft,dock?.045f:.072f,dock?.155f:RosterRight,dock?.108f:.114f);
            foreach(var pair in _rosterItems)
            {
                var rt=pair.Value;rt.gameObject.SetActive(true);
                rt.Find("Portrait").gameObject.SetActive(!dock);rt.Find("SelectionRing").gameObject.SetActive(!dock);
                rt.Find("DockArtwork").gameObject.SetActive(dock);rt.Find("DockFrame").gameObject.SetActive(dock);
                var reserve=rt.Find("Reserve");if(reserve!=null)reserve.gameObject.SetActive(!dock);
                rt.GetComponent<Image>().color=Color.clear;
                rt.GetComponent<Button>().targetGraphic=dock?rt.Find("DockFrame").GetComponent<Graphic>():_rosterFaces[pair.Key];
            }
            if(dock)
            {
                _roster.anchorMin=Vector2.zero;_roster.anchorMax=new Vector2(0,1);_roster.pivot=new Vector2(0,.5f);
                _roster.sizeDelta=Vector2.zero;_roster.anchoredPosition=Vector2.zero;
                LayoutDock();
                SetDockOffset(previousOffset);
                if(entering)RevealInspectedRosterMember();
            }
            else
            {
                _rosterPrevious.gameObject.SetActive(false);_rosterNext.gameObject.SetActive(false);
                _roster.anchorMin=new Vector2(0,1);_roster.anchorMax=Vector2.one;_roster.pivot=new Vector2(.5f,1);
                _roster.offsetMin=_roster.offsetMax=Vector2.zero;
                UiScroll.Fit(_roster,_rosterItems.Count*RosterRowStep);
                int index=0;
                foreach(var pair in _rosterItems)
                {
                    var rt=pair.Value;rt.anchorMin=new Vector2(0,1);rt.anchorMax=Vector2.one;rt.pivot=new Vector2(.5f,1);
                    rt.sizeDelta=new Vector2(-8,RosterRowHeight);rt.anchoredPosition=new Vector2(0,-index++*RosterRowStep);
                    PlaceRosterFace(rt,.14f,.40f,.86f,.96f);
                    ExemplarUi.Place((RectTransform)rt.Find("Name"),.02f,.14f,.98f,.35f);
                    ExemplarUi.Place((RectTransform)rt.Find("Level"),.02f,.005f,.98f,.16f);
                    var name=rt.Find("Name").GetComponent<TextMeshProUGUI>();name.fontSizeMax=18;name.fontSizeMin=13;name.alignment=TextAlignmentOptions.Center;
                    var level=rt.Find("Level").GetComponent<TextMeshProUGUI>();level.fontSizeMax=13;level.fontSizeMin=10;level.alignment=TextAlignmentOptions.Center;
                }
            }
        }
        void LateUpdate()
        {
            if(_dockLayout&&_rosterViewport.rect.size!=_rosterLastSize){LayoutDock();RevealInspectedRosterMember();}
        }
        void RevealInspectedRosterMember()
        {
            if(!_dockLayout||!_rosterItems.TryGetValue(InspectedId,out var row))return;
            float offset=-_roster.anchoredPosition.x;
            if(row.anchoredPosition.x<offset)offset=row.anchoredPosition.x;
            else if(row.anchoredPosition.x+row.rect.width>offset+_rosterViewport.rect.width)
                offset=row.anchoredPosition.x+row.rect.width-_rosterViewport.rect.width;
            SetDockOffset(offset);
        }
        void ShiftRoster(int direction)
        {
            if(!_dockLayout||UiConfirm.BlocksNavigation)return;
            SetDockOffset(-_roster.anchoredPosition.x+direction*(_dockCardWidth+DockGap)*3);
        }
        void SetDockOffset(float offset)
        {
            _rosterScroll.StopMovement();
            _roster.anchoredPosition=new Vector2(-Mathf.Clamp(offset,0,Mathf.Max(0,_roster.rect.width-_rosterViewport.rect.width)),0);
            UpdateDockNavigation();
        }
        void UpdateDockNavigation()
        {
            float overflow=Mathf.Max(0,_roster.rect.width-_rosterViewport.rect.width);
            bool show=overflow>1;
            _rosterPrevious.gameObject.SetActive(show);_rosterNext.gameObject.SetActive(show);
            _rosterPrevious.interactable=-_roster.anchoredPosition.x>1;
            _rosterNext.interactable=-_roster.anchoredPosition.x<overflow-1;
            foreach(var arrow in new[]{_rosterPrevious,_rosterNext})
                arrow.GetComponentInChildren<TextMeshProUGUI>().color=arrow.interactable?ExemplarUi.White:new Color(.60f,.65f,.69f,.25f);
        }
        void LayoutDock()
        {
            _rosterLastSize=_rosterViewport.rect.size;
            // Six full portraits at the reference width. Smaller viewports overflow naturally.
            _dockCardWidth=Mathf.Clamp((_rosterViewport.rect.width-5*DockGap)/6,136,158);
            float width=_rosterItems.Count*(_dockCardWidth+DockGap)-DockGap;
            _roster.sizeDelta=new Vector2(Mathf.Max(0,width),0);
            int index=0;
            foreach(var pair in _rosterItems)
            {
                var rt=pair.Value;rt.anchorMin=new Vector2(0,0);rt.anchorMax=new Vector2(0,1);rt.pivot=new Vector2(0,.5f);
                rt.sizeDelta=new Vector2(_dockCardWidth,-4);rt.anchoredPosition=new Vector2(index++*(_dockCardWidth+DockGap),0);
                ExemplarUi.Place((RectTransform)rt.Find("Name"),.075f,.055f,.73f,.28f);
                ExemplarUi.Place((RectTransform)rt.Find("Level"),.72f,.06f,.94f,.26f);
                var name=rt.Find("Name").GetComponent<TextMeshProUGUI>();name.fontSizeMax=17;name.fontSizeMin=14;name.alignment=TextAlignmentOptions.Left;
                var level=rt.Find("Level").GetComponent<TextMeshProUGUI>();level.fontSizeMax=13;level.fontSizeMin=11;level.alignment=TextAlignmentOptions.Right;
            }
            SetDockOffset(-_roster.anchoredPosition.x);
        }
        static void PlaceRosterFace(RectTransform row,float x0,float y0,float x1,float y1)
        {
            ExemplarUi.Place((RectTransform)row.Find("Portrait"),x0,y0,x1,y1);
            ExemplarUi.Place((RectTransform)row.Find("SelectionRing"),x0,y0,x1,y1);
        }
        void BuildPage(string id)
        {
            var host=ExemplarUi.Rect(_legacyBody,id,0,0,1,1);
            Action refresh=null;
            switch(id)
            {
                case "Regalia":
                    var regalia=ExemplarRegaliaPage.Create(host);
                    refresh=()=>regalia.Show(_snapshot?.Find(InspectedId));break;
                case "Equipment":
                    var equipment=ExemplarEquipmentPage.Create(host);
                    refresh=()=>equipment.Show(_snapshot?.Find(InspectedId));break;
                case "Skills":
                    // The existing functional graph keeps its 720-unit geometry inside this shell.
                    host.gameObject.AddComponent<ExemplarWidgetScale>();
                    Canvas.ForceUpdateCanvases();host.GetComponent<ExemplarWidgetScale>().Fit();
                    var potential=PotentialWindow.Attach(host,exemplarStyle:true);
                    refresh=potential.Refresh;break;
                case "Apex":
                    var apex=ExemplarApexPage.Create(host);
                    refresh=()=>apex.Show(_snapshot?.Find(InspectedId));break;
                case "Resonance":
                    var resonance=ExemplarResonancePage.Create(host);
                    refresh=()=>resonance.Show(_snapshot?.Find(InspectedId));break;
            }
            _pages[id]=(host.gameObject,refresh);
        }
        void PaintNavigation()
        {
            int selected=Array.IndexOf(PageIds,CurrentPage);
            if(selected>=0)_navigationArc.SelectNormalized(1f-(.86f-selected*.125f-.0505f)/.902f);
            foreach(var kv in _nav)
            {
                if(kv.Value is MenuNavigationButton nav)nav.SetActiveState(kv.Key==CurrentPage);
            }
        }
        public void Back()
        {
            if(UiConfirm.BlocksNavigation)return;
            if(CurrentPage=="Equipment"&&_pages.TryGetValue("Equipment",out var page))
            {
                var equipment=page.go.GetComponentInChildren<ExemplarEquipmentPage>();
                if(equipment!=null&&equipment.TryCancelPicker())return;
            }
            if(CurrentPage!="Attributes")SelectPage("Attributes");else _close?.Invoke();
        }
        static bool NeedsDeployed(string id)=>id!="Attributes"&&id!="Resonance"&&id!="Apex";
        static string PageName(string id)=>id=="Attributes"?Loc.Get("UI_EXEMPLAR_ATTRIBUTES"):id=="Equipment"?Loc.Get("UI_EXEMPLAR_EQUIPMENT"):id=="Resonance"?Loc.Get("UI_EXEMPLAR_RESONANCE_TITLE"):GameText.ScreenName(id);
    }

    public sealed class ExemplarWidgetScale : MonoBehaviour
    {
        Vector2 _size;
        public void Fit()
        {
            var rt=(RectTransform)transform;var p=(RectTransform)rt.parent;var size=p.rect.size;
            if(size==_size)return;_size=size;
            rt.anchorMin=rt.anchorMax=rt.pivot=new Vector2(0,1);rt.anchoredPosition=Vector2.zero;
            rt.localScale=Vector3.one*1.5f;rt.sizeDelta=size/1.5f;
        }
        void LateUpdate()=>Fit();
    }
}
