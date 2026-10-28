// Eclipse bestiary: presentation only. Discovery, bands, drops and rewards come from the sim.
using System;
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class BestiaryWindow : MonoBehaviour
    {
        Roi.Sim.SimDriver _driver;
        Roi.Arena.ArenaView _arena;
        RectTransform _body, _rail, _detail, _railHost, _dossierHeader;
        TextMeshProUGUI _status;
        TMP_InputField _search;
        Button _filterButton;
        GameObject _filterPopup;
        static BestiaryWindow _filterOwner;
        static int _filterClosedFrame=-1;
        public static bool BlocksNavigation => (_filterOwner!=null&&_filterOwner._filterPopup!=null)||_filterClosedFrame==Time.frameCount;
        void CloseFilter(){if(_filterPopup==null)return;_filterPopup.SetActive(false);Destroy(_filterPopup);_filterPopup=null;_filterOwner=null;_filterClosedFrame=Time.frameCount;}
        void Update(){if(_filterPopup!=null&&UnityEngine.InputSystem.Keyboard.current?.escapeKey.wasPressedThisFrame==true)CloseFilter();}
        ScrollRect _listScroll, _detailScroll;
        readonly List<GameObject> _railBuilt = new(), _detailBuilt = new();
        Payload _data = new();
        string _sel = "", _query = "";
        int _filter, _tab;
        float _lastWidth;
        ShopSelectionArc _categoryArc;
        readonly List<MenuNavigationButton> _categories=new();
        Image _portrait;
        readonly Dictionary<string, Sprite> _portraits = new();

        public static BestiaryWindow Attach(RectTransform body)
        {
            var w=body.gameObject.AddComponent<BestiaryWindow>();w.Build(body);return w;
        }
        static string T(string key)=>Loc.Get("UI_MONSTER_"+key);
        static RectTransform Rect(Transform p,string name,float x,float y,float w,float h)
        {
            var r=new GameObject(name,typeof(RectTransform)).GetComponent<RectTransform>();r.SetParent(p,false);
            Fixed(r,x,y,w,h);return r;
        }
        static void Fixed(RectTransform r,float x,float y,float w,float h)
        {
            r.anchorMin=r.anchorMax=new Vector2(0,1);r.pivot=new Vector2(0,1);
            r.anchoredPosition=new Vector2(x,-y);r.sizeDelta=new Vector2(w,h);
        }
        static void Fill(RectTransform r){r.anchorMin=Vector2.zero;r.anchorMax=Vector2.one;r.offsetMin=r.offsetMax=Vector2.zero;}
        static void Icon(Transform p,string kind,float x,float y,float size,Color c)
        {
            var r=Rect(p,kind+"Insignia",x,y,size,size);var i=r.gameObject.AddComponent<HomeIcon>();i.Kind=kind;i.color=c;i.raycastTarget=false;
        }
        static TextMeshProUGUI Text(Transform p,string name,string value,float x,float y,float w,float h,float size,Color color)
        {
            var t=UiTheme.Label(p,name,value,size,color,TextAlignmentOptions.Left);Fixed(t.rectTransform,x,y,w,h);
            t.textWrappingMode=TextWrappingModes.Normal;
            t.enableAutoSizing=true;t.fontSizeMin=Mathf.Min(12,size);t.fontSizeMax=size;t.raycastTarget=false;return t;
        }
        void Build(RectTransform body)
        {
            _body=body;
            var left=Rect(body,"Catalogue",20,16,480,44);
            left.anchorMin=new Vector2(.09f,0);left.anchorMax=new Vector2(.65f,0);left.pivot=new Vector2(0,0);left.offsetMin=new Vector2(0,16);left.offsetMax=new Vector2(0,60);
            _filterButton=HudBar.MakeButton(left,T("ALL"),"",120,44,()=>ToggleFilter());
            Fixed((RectTransform)_filterButton.transform,0,0,120,44);
            var box=UiTheme.Box(left,"Search",UiTheme.Surface);var br=box.rectTransform;
            Fill(br);br.offsetMin=new Vector2(130,0);UiTheme.Border(br,UiTheme.LineStrong);
            var area=Rect(br,"TextArea",12,0,200,44);Fill(area);area.offsetMin=new Vector2(12,2);area.offsetMax=new Vector2(-42,-2);area.gameObject.AddComponent<RectMask2D>();
            var ph=UiTheme.Label(area,"Placeholder",T("SEARCH"),16,UiTheme.TextLow,TextAlignmentOptions.Left);Fill(ph.rectTransform);
            var tx=UiTheme.Label(area,"Text","",16,UiTheme.TextHigh,TextAlignmentOptions.Left);Fill(tx.rectTransform);tx.overflowMode=TextOverflowModes.Overflow;
            _search=box.gameObject.AddComponent<TMP_InputField>();_search.textViewport=area;_search.textComponent=tx;_search.placeholder=ph;_search.targetGraphic=box;
            _search.lineType=TMP_InputField.LineType.SingleLine;_search.characterLimit=50;
            _search.onValueChanged.AddListener(v=>{_query=v.Trim();FilterChanged();});
            var clear=HudBar.MakeButton(br,"×","",38,40,()=>{_search.text="";});var cr=(RectTransform)clear.transform;
            cr.anchorMin=cr.anchorMax=new Vector2(1,.5f);cr.pivot=new Vector2(1,.5f);cr.anchoredPosition=new Vector2(-2,0);
            _status=Text(body,"Status","",110,12,400,24,13,UiTheme.TextLow);
            var categoryRail=ExemplarUi.Rect(body,"Categories",.012f,.18f,.075f,.92f);
            var seam=ExemplarUi.Rect(body,"CategorySeam",.08f,.18f,.08f,.92f);seam.offsetMin=new Vector2(-47,0);seam.offsetMax=new Vector2(17,0);
            _categoryArc=seam.gameObject.AddComponent<ShopSelectionArc>();_categoryArc.raycastTarget=false;
            string[] keys={"ALL","RECORDED","UNKNOWN"};string[] icons={"Bestiary","Album","Record"};
            for(int i=0;i<3;i++){int choice=i;float top=1-i*.20f;
                var b=CollectionBrowser.Quiet(categoryRail,"Category_"+i,T(keys[i]),0,top-.18f,1,top,()=>{_filter=choice;FilterChanged();});
                ExemplarUi.Place(b.Caption.rectTransform,0,0,1,.32f);b.Caption.enableAutoSizing=true;b.Caption.fontSizeMin=10;b.Caption.fontSizeMax=12;
                ExemplarUi.Icon(b.transform,icons[i],.24f,.40f,.76f,.94f,UiTheme.TextMid);b.CategoryIcon=b.GetComponentInChildren<HomeIcon>();b.Caption.gameObject.SetActive(false);_categories.Add(b);
            }
            _railHost=Rect(body,"MonsterGrid",20,98,480,500);_railHost.anchorMin=new Vector2(.09f,0);_railHost.anchorMax=new Vector2(.65f,1);
            _railHost.offsetMin=new Vector2(0,80);_railHost.offsetMax=new Vector2(-12,-48);
            _rail=UiScroll.Create(_railHost,out _listScroll);
            var detailHost=Rect(body,"MonsterDossier",0,0,500,500);detailHost.anchorMin=new Vector2(.67f,0);detailHost.anchorMax=Vector2.one;
            detailHost.offsetMin=new Vector2(12,18);detailHost.offsetMax=new Vector2(-20,-16);
            var divider=ExemplarUi.Panel(detailHost,"DossierDivider",0,0,.002f,1,UiTheme.LineStrong);
            // Keep the selected creature and tabs visible while its facts or rewards scroll.
            _dossierHeader=Rect(detailHost,"DossierHeader",0,0,500,298);
            _dossierHeader.anchorMax=new Vector2(1,1);_dossierHeader.sizeDelta=new Vector2(0,298);
            var factsHost=Rect(detailHost,"DossierFacts",0,0,500,200);
            Fill(factsHost);factsHost.offsetMin=new Vector2(20,16);factsHost.offsetMax=new Vector2(-16,-308);
            _detail=UiScroll.Create(factsHost,out _detailScroll);
        }
        void LateUpdate()
        {
            if(_rail==null||_data.entries==null)return;
            if(Mathf.Abs(_lastWidth-_rail.rect.width)>2){_lastWidth=_rail.rect.width;BuildRail(_data.entries);BuildDetail(Find(_sel));}
        }
        public void Refresh()
        {
            if(_driver==null)_driver=UnityEngine.Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            if(_arena==null)_arena=UnityEngine.Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
            var sim=_driver!=null?_driver.Session:null;
            if(sim==null){_status.text=Loc.Get("UI_BESTIARY_NO_SESSION");return;}
            _data=JsonUtility.FromJson<Payload>(sim.QueryJson("bestiary"))??new Payload();
            Render();
        }
        void Render()
        {
            for(int i=0;i<_categories.Count;i++)_categories[i].SetActiveState(i==_filter);
            _categoryArc.SelectNormalized(_filter*.20f+.09f);
            _filterButton.GetComponentInChildren<TextMeshProUGUI>().text=T(new[]{"ALL","RECORDED","UNKNOWN"}[_filter]);
            ((TextMeshProUGUI)_search.placeholder).text=T("SEARCH");
            var entries=_data.entries??Array.Empty<Entry>();
            if(Find(_sel)==null&&entries.Length>0){_sel=entries[0].key;foreach(var e in entries)if(e.kills>0){_sel=e.key;break;}}
            BuildRail(entries);BuildDetail(Find(_sel));
        }
        bool Matches(Entry e)=> (_filter!=1||e.kills>0)&&(_filter!=2||e.kills==0)&&
            (string.IsNullOrEmpty(_query)||(e.kills>0&&e.name.IndexOf(_query,StringComparison.CurrentCultureIgnoreCase)>=0));
        void ToggleFilter()
        {
            if(_filterPopup!=null){CloseFilter();return;}
            var overlay=UiTheme.Flat(_body,"FilterDismiss",Color.clear);Fill(overlay.rectTransform);_filterPopup=overlay.gameObject;_filterOwner=this;
            var dismiss=overlay.gameObject.AddComponent<Button>();dismiss.onClick.AddListener(()=>CloseFilter());
            var panel=UiTheme.Box(overlay.transform,"FilterOptions",UiTheme.Panel);Fixed(panel.rectTransform,20,62,190,156);UiTheme.Border(panel.rectTransform,UiTheme.LineStrong);
            string[] keys={"ALL","RECORDED","UNKNOWN"};for(int i=0;i<3;i++){int choice=i;
                var b=HudBar.MakeButton(panel.transform,T(keys[i]),"",178,46,()=>{_filter=choice;CloseFilter();FilterChanged();});Fixed((RectTransform)b.transform,6,6+i*48,178,46);}
        }
        void OnDisable(){CloseFilter();}
        void FilterChanged()
        {
            for(int i=0;i<_categories.Count;i++)_categories[i].SetActiveState(i==_filter);
            _categoryArc.SelectNormalized(_filter*.20f+.09f);
            _filterButton.GetComponentInChildren<TextMeshProUGUI>().text=T(new[]{"ALL","RECORDED","UNKNOWN"}[_filter]);
            var entries=_data.entries??Array.Empty<Entry>();
            var selected=Find(_sel);if(selected==null||!Matches(selected)){_sel="";foreach(var e in entries)if(Matches(e)){_sel=e.key;break;}}
            BuildRail(entries);BuildDetail(Find(_sel));_listScroll.verticalNormalizedPosition=1;_detailScroll.verticalNormalizedPosition=1;
        }
        Sprite Portrait(Entry e)
        {
            if(e.kills==0||string.IsNullOrEmpty(e.sprite)||_arena==null)return null;
            if(!_portraits.TryGetValue(e.sprite,out var s)){s=_arena.MonsterPortrait(e.sprite);_portraits[e.sprite]=s;}return s;
        }
        static void Clear(List<GameObject> list){foreach(var go in list){go.SetActive(false);Destroy(go);}list.Clear();}
        void BuildRail(Entry[] entries)
        {
            Clear(_railBuilt);
            var sorted=new List<Entry>();int found=0;foreach(var e in entries){if(e.kills>0)found++;if(Matches(e))sorted.Add(e);}
            sorted.Sort((a,b)=>{int k=(b.kills>0).CompareTo(a.kills>0);if(k!=0)return k;int d=a.stage_min.CompareTo(b.stage_min);return d!=0?d:string.CompareOrdinal(a.key,b.key);});
            _status.text=Loc.Fmt("UI_MONSTER_COUNT",found,entries.Length);
            float width=Mathf.Max(240,_rail.rect.width);int cols=Mathf.Max(2,Mathf.FloorToInt((width+10)/92));float cw=(width-(cols-1)*10)/cols;
            float y=0;int col=0,band=-1;
            foreach(var e in sorted)
            {
                bool active=e.key==_sel,known=e.kills>0;
                var tile=CodexButtonImage.Create(_rail,"Monster_"+e.key);tile.color=active?UiTheme.SurfaceActive:UiTheme.Surface;Fixed(tile.rectTransform,col*(cw+10),y,cw,116);
                _railBuilt.Add(tile.gameObject);
                var b=tile.gameObject.AddComponent<UiStateButton>();b.targetGraphic=tile;b.colors=UiTheme.ButtonColors;UiButtonFeedback.Add(b);
                string key=e.key;b.onClick.AddListener(()=>{_sel=key;BuildRail(_data.entries);BuildDetail(Find(key));_detailScroll.verticalNormalizedPosition=1;UiPageTransition.Play(_detailScroll.viewport);});
                var s=Portrait(e);if(s!=null){var im=Rect(tile.transform,"Creature",10,8,cw-20,66).gameObject.AddComponent<Image>();im.sprite=s;im.preserveAspect=true;im.raycastTarget=false;}
                else Icon(tile.transform,"Bestiary",(cw-40)/2,18,40,known?UiTheme.Accent:UiTheme.TextLow*.6f);
                var label=Text(tile.transform,"Name",known?e.name:"???",8,74,cw-16,34,13,active?UiTheme.Accent:UiTheme.TextHigh);label.alignment=TextAlignmentOptions.Center;label.textWrappingMode=TextWrappingModes.Normal;
                if(active){var mark=UiTheme.Flat(tile.transform,"Selected",UiTheme.Accent);Fixed(mark.rectTransform,12,0,cw-24,3);mark.raycastTarget=false;}
                if(++col==cols){col=0;y+=126;}
            }
            if(col>0)y+=126;
            if(sorted.Count==0){var no=Text(_rail,"NoMatches",T("NO_MATCHES"),8,16,width-16,80,18,UiTheme.TextMid);_railBuilt.Add(no.gameObject);y=110;}
            UiScroll.Fit(_rail,y+8);
        }
        void BuildDetail(Entry e)
        {
            Clear(_detailBuilt);_portrait=null;
            float w=Mathf.Max(300,_dossierHeader.rect.width),inner=w-48;
            var root=Rect(_dossierHeader,"Dossier",0,0,w,298);_detailBuilt.Add(root.gameObject);
            if(e==null){Text(root,"Empty",T("SELECT"),24,50,w-48,90,22,UiTheme.TextMid);UiScroll.Fit(_detail,180);return;}
            bool known=e.kills>0;
            Icon(root,"Bestiary",24,20,26,UiTheme.Accent);
            var title=Text(root,"Name",known?e.name:"???",62,14,w-86,42,28,UiTheme.Accent);UiTheme.Display(title);
            Text(root,"Classification",known?e.race+"  /  "+e.size:T("UNKNOWN"),24,61,inner*.55f,26,14,UiTheme.TextLow);
            if(known){var stage=Text(root,"StageRange",Loc.Fmt("UI_MONSTER_STAGES",e.stage_min,e.stage_max),24+inner*.55f,61,inner*.45f,26,14,UiTheme.Accent);stage.alignment=TextAlignmentOptions.Right;}
            // A fixed specimen plinth grounds the creature; no background update/animation cost.
            var plinth=UiTheme.Box(root,"SpecimenPlinth",UiTheme.SurfaceActive);Fixed(plinth.rectTransform,32,242,Mathf.Min(252,w*.55f),6);plinth.raycastTarget=false;
            var sprite=Portrait(e);
            if(sprite!=null){_portrait=Rect(root,"Portrait",32,94,Mathf.Min(252,w*.55f),148).gameObject.AddComponent<Image>();_portrait.sprite=sprite;_portrait.preserveAspect=true;_portrait.raycastTarget=false;
                var frames=_arena.MonsterIdlePortraits(e.sprite,out int fps);
                _portrait.gameObject.AddComponent<MonsterPortraitIdle>().Initialize(_portrait,frames,fps);}
            else Icon(root,"Bestiary",68,122,84,known?UiTheme.Accent:UiTheme.TextLow*.6f);
            float sx=w*.62f,sw=w-sx-20;
            Text(root,"RecordLabel",T("DEFEATED"),sx,119,sw,24,13,UiTheme.TextLow);
            Text(root,"RecordCount",known?e.kills.ToString("N0"):"—",sx,145,sw,42,30,UiTheme.Accent);
            Text(root,"RecordHint",known?T("FAMILY_RECORD"):T("UNKNOWN"),sx,191,sw,36,13,UiTheme.TextMid);
            float tabw=inner/3;
            string[] names={"TRAITS","DROPS","REWARDS"};
            for(int i=0;i<3;i++){int idx=i;var b=HudBar.MakeButton(root,T(names[i]),"",tabw-6,40,()=>{_tab=idx;BuildDetail(Find(_sel));_detailScroll.verticalNormalizedPosition=1;UiPageTransition.Play(_detailScroll.viewport);});Fixed((RectTransform)b.transform,24+i*tabw,258,tabw-6,40);
                if(i==_tab){b.targetGraphic.color=UiTheme.SurfaceActive;var strip=UiTheme.Flat(b.transform,"ActiveTab",UiTheme.Accent);Fixed(strip.rectTransform,8,38,tabw-22,2);strip.raycastTarget=false;}}
            // Independent fact scroller, with consistent insets and four visible trait rows.
            w=Mathf.Max(1,_detail.rect.width);inner=w-8;
            root=Rect(_detail,"Facts",0,0,w,200);_detailBuilt.Add(root.gameObject);
            float y=0;
            if(!known){Text(root,"Locked",T("DISCOVER"),4,y,inner,70,18,UiTheme.TextMid);y+=90;}
            else if(_tab==0)
            {
                DetailRow(root,ref y,w,T("WEAK"),Elements(e.weaknesses),"Skills");
                DetailRow(root,ref y,w,T("RESIST"),Elements(e.resists),"Commander");
                DetailRow(root,ref y,w,T("HP"),Band(e.hp_band),"Provisions");
                DetailRow(root,ref y,w,T("ATK"),Band(e.atk_band),"Combat");
                var others=new List<string>();foreach(var v in e.variants??Array.Empty<string>())if(v!=e.name)others.Add(v);
                if(others.Count>0){Text(root,"Variants",T("VARIANTS")+"  "+string.Join(", ",others),4,y+4,inner,64,13,UiTheme.TextLow);y+=72;}
            }
            else if(_tab==1)
            {
                Text(root,"DropNote",T("DROP_NOTE"),4,y,inner,42,13,UiTheme.TextLow);y+=50;
                foreach(var d in e.drops??Array.Empty<Drop>())DetailRow(root,ref y,w,d.name,Pct(d.rate_bp),"Inventory");
                if(e.drops==null||e.drops.Length==0){Text(root,"NoDrops",T("NO_DROPS"),4,y,inner,56,16,UiTheme.TextMid);y+=64;}
            }
            else
            {
                var stones=_data.milestones??Array.Empty<Milestone>();
                for(int i=0;i<stones.Length;i++){
                    var ms=stones[i];bool claimed=(e.claimed&(1<<i))!=0,reached=e.kills>=ms.kills;
                    var row=UiTheme.Box(root,"Milestone_"+i,reached&&!claimed?UiTheme.SurfaceActive:UiTheme.Surface);Fixed(row.rectTransform,4,y,inner,88);UiTheme.Border(row.rectTransform,UiTheme.Line);
                    Text(row.transform,"Target",Loc.Fmt("UI_MONSTER_KILLS",ms.kills.ToString("N0")),12,8,inner-136,30,16,UiTheme.TextHigh);
                    Text(row.transform,"Reward",$"{ms.gil:N0} Gil · {ms.soul:N0} Soul",12,44,inner-136,26,13,UiTheme.Accent);
                    int index=i;string key=e.key;var claim=HudBar.MakeButton(row.transform,claimed?Loc.Get("UI_BESTIARY_CLAIMED"):reached?Loc.Get("UI_COMMON_CLAIM"):$"{e.kills:N0}/{ms.kills:N0}","",112,44,()=>Claim(key,index));
                    Fixed((RectTransform)claim.transform,inner-124,22,112,44);claim.interactable=reached&&!claimed;y+=96;
                }
            }
            root.sizeDelta=new Vector2(w,y+4);UiScroll.Fit(_detail,y+4);
        }
        static string Elements(string[] a)=>a==null||a.Length==0?T("NONE"):string.Join(" · ",Array.ConvertAll(a,GameText.ElementName));
        static void DetailRow(Transform p,ref float y,float w,string label,string value,string icon)
        {
            var row=UiTheme.Box(p,"Fact",UiTheme.Surface);Fixed(row.rectTransform,4,y,w-8,38);
            Icon(row.transform,icon,10,9,20,UiTheme.TextLow);
            Text(row.transform,"Label",label,40,2,w*.45f-40,34,14,UiTheme.TextMid);
            var t=Text(row.transform,"Value",value,w*.45f+8,2,w*.55f-30,34,15,UiTheme.TextHigh);t.alignment=TextAlignmentOptions.Right;y+=44;
        }
        void Claim(string group, int idx)
        {
            var sim = _driver != null ? _driver.Session : null;
            if (sim == null) return;
            // The sim owns every condition — the tally, the bit, the payout. This is a request.
            sim.ApplyInput($"{{\"ClaimBestiaryReward\":{{\"group\":\"{group}\",\"idx\":{idx}}}}}");
            Refresh();
        }

        Entry Find(string key)
        {
            foreach (var e in _data.entries ?? new Entry[0]) if (e.key == key) return e;
            return null;
        }

        /// The band keys the sim hands back, as words. They are a scale, not a stat — see the file
        /// header for why the raw numbers are not shown.
        static string Band(string key) => string.IsNullOrEmpty(key) ? "—" : key switch
        {
            "trivial" => "Trivial", "low" => "Low", "mid" => "Average",
            "high" => "High", "elite" => "Elite", "boss" => "Boss",
            _ => char.ToUpperInvariant(key[0]) + key.Substring(1),
        };

        static string Pct(long rateBp)
        {
            // rate_bp is per-100000: 1 = 0.001%. A rate the sim rounded to zero is still a real
            // drop — a card at roughly one in fifty thousand lands here — and printing "0.000%"
            // says it cannot happen, which is the opposite of true.
            if (rateBp <= 0) return "<0.001%";
            double pct = rateBp / 1000.0;
            return pct >= 1.0 ? $"{pct:0.#}%" : pct >= 0.1 ? $"{pct:0.00}%" : $"{pct:0.000}%";
        }

        void Head(ref float y, string text)
        {
            var t = UiTheme.Caps(_detail, "H_" + text, text, UiTheme.CaptionSize,
                                 UiTheme.TextLow, TextAlignmentOptions.Left);
            PlaceAt(t.rectTransform, 0f, y, 26f);
            _detailBuilt.Add(t.gameObject);
            y += 26f;
        }

        void Line(ref float y, float x, string text)
        {
            var t = UiTheme.Label(_detail, "T", text, UiTheme.BodySize, UiTheme.TextMid,
                                  TextAlignmentOptions.Left);
            t.overflowMode = TextOverflowModes.Overflow;
            PlaceAt(t.rectTransform, x, y, 24f);
            _detailBuilt.Add(t.gameObject);
            y += 24f;
        }

        static string Hex(Color c) => ColorUtility.ToHtmlStringRGB(c);

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

        /// Placed from the LEFT edge at `x`, for the detail column's two-column header.
        void PlaceAt(RectTransform rt, float x, float y, float h)
        {
            rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
            rt.pivot = new Vector2(0f, 1f);
            rt.offsetMin = new Vector2(x, 0f); rt.offsetMax = new Vector2(0f, 0f);
            rt.sizeDelta = new Vector2(rt.sizeDelta.x, h);
            rt.anchoredPosition = new Vector2(x, -y);
        }

        [System.Serializable] class Payload { public Milestone[] milestones; public Entry[] entries; }
        [System.Serializable] class Milestone { public long kills, gil, soul; }
        [System.Serializable]
        class Entry
        {
            public string key, name, race, size, hp_band, atk_band, sprite;
            public string[] weaknesses, resists;
            public int stage_min, stage_max;
            public long kills;
            public int claimed;
            public string[] variants;
            public Drop[] drops;
        }
        [System.Serializable] class Drop { public string name; public long rate_bp; }
    }
}
