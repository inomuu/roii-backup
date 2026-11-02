using TMPro;
using System;
using UnityEngine.UI;
using UnityEngine;

namespace Roi.UI
{
    public sealed partial class HomeMenuLayout : MonoBehaviour
    {
        static readonly string[] MoreEntries=new[]{"Map","Dungeon","Story","Daily","Weekly","Events","Pass","Commander","Outpost","Bestiary","Record","Paths","Cards","Stash","Album","Provisions","Regalia","Apothecary","Skills"};
        Action<string> _open;
        GameObject _group;
        GameObject _groupReturn;
        TMP_FontAsset _font;
        TextMeshProUGUI _profile;
        Roi.Sim.SimDriver _driver;
        float _nextRefresh;
        static readonly Color Ink=new Color(.09f,.105f,.12f);
        static readonly Color Gold=UiTheme.Accent;
        static readonly Color Ivory=new Color(.955f,.96f,.945f);
        public static void Build(Transform parent,Action<string> open=null)
        {
            var root=new GameObject("HomeLayout",typeof(RectTransform));root.transform.SetParent(parent,false);
            var h=root.AddComponent<HomeMenuLayout>();h._open=open;
            h._font=Loc.Language=="th" ? UiTheme.Font : Resources.Load<TMP_FontAsset>("HomeUI/HomeSerif") ?? UiTheme.DisplayFont;
            var rt=(RectTransform)root.transform;Place(rt,0,0,1,1);
            h.Compose(rt);
            UiEntrance.Play(rt); // the home screen arriving is a whole-screen event, not a refresh
            if(h._homeMenu!=null)UiStagger.Play(h._homeMenu.content); // menu tiles cascade in
        }
        TextMeshProUGUI _expedition, _battleState;
        Button _battle;
        static readonly Color Quiet = new Color(.73f,.79f,.85f);
        RectTransform _safe, _ledger;
        Vector2 _viewport;
        Rect _safeRect;
        void Compose(RectTransform rt) => ComposeTactical(rt);
        void LobbyTile(Transform parent,string id,string label,float x,float y,float r,float t)
        {
            var button=HubButton(parent,id,label,x,y,r,t,false);
            Icon(button.transform,id=="More"?"Options":id,.23f,.43f,.77f,.94f,Gold);
            if(parent.name=="BottomDock") {
                button.GetComponent<Image>().color=Color.clear;
                foreach(var img in button.GetComponentsInChildren<Image>())if(img!=button.GetComponent<Image>())img.enabled=false;
            }
            var caption=button.GetComponentInChildren<TextMeshProUGUI>();
            Place(caption.rectTransform,.025f,.025f,.975f,.42f);
            caption.font=UiTheme.Font;caption.fontSharedMaterial=UiTheme.Material;
            caption.alignment=TextAlignmentOptions.Center;caption.textWrappingMode=TextWrappingModes.NoWrap;
            caption.fontSizeMin=16;caption.fontSizeMax=24;caption.fontSize=24;
        }
        Button LedgerLink(Transform parent,string id,string label,float x,float y,float r,float t,float fontSize)
        {
            var hit=Panel(parent,id,x,y,r,t,Color.white);hit.raycastTarget=true;
            var button=hit.gameObject.AddComponent<MenuNavigationButton>();button.targetGraphic=hit;
            button.Caption=Text(hit.transform,"Label",label,.28f,.12f,.99f,.88f,fontSize,Ivory);
            button.Caption.alignment=TextAlignmentOptions.MidlineLeft;
            button.Caption.textWrappingMode=TextWrappingModes.NoWrap;
            button.onClick.AddListener(()=>Navigate(id));button.SetActiveState(false);UiButtonFeedback.Add(button);
            return button;
        }
        void LateUpdate(){LayoutForScreen();}
        void LayoutForScreen()
        {
            if(_safe==null||Screen.width<=0||Screen.height<=0)return;
            var viewport=new Vector2(Screen.width,Screen.height);var safe=Screen.safeArea;
            if(viewport==_viewport&&safe==_safeRect)return;
            _viewport=viewport;_safeRect=safe;
            Place(_safe,safe.xMin/viewport.x,safe.yMin/viewport.y,safe.xMax/viewport.x,safe.yMax/viewport.y);
        }
        void FitStage(bool wide)
        {
            _expedition.enableAutoSizing=true;_expedition.fontSizeMin=16;_expedition.fontSizeMax=48;
        }
        static void DepthShadow(Transform parent)
        {
            var shadow=GameMenuSurface.Add(parent);shadow.gameObject.name="DepthShadow";shadow.color=new Color(0,0,0,.45f);
            shadow.rectTransform.anchoredPosition=new Vector2(7,-10);
        }
        void Quick(Transform parent,string id,string label,float y)
        {
            var button=HubButton(parent,id,label,.027f,y,.120f,y+.110f,false);
            var surface=button.GetComponentInChildren<EclipseSurface>();surface.gameObject.SetActive(false);
            button.targetGraphic=button.GetComponent<Image>();button.GetComponent<Image>().color=new Color(.02f,.04f,.065f,.18f);
            Icon(button.transform,id,.36f,.45f,.64f,.83f,Ivory);
            var copy=button.GetComponentInChildren<TextMeshProUGUI>();Place(copy.rectTransform,0,.025f,1,.35f);copy.fontSizeMin=15;copy.fontSizeMax=17;
        }
        Button HubButton(Transform parent,string id,string label,float x,float y,float right,float top,bool primary)
        {
            var image=Panel(parent,id,x,y,right,top,primary?UiTheme.FramePrimary:UiTheme.WithAlpha(UiTheme.Panel,.88f));
            image.raycastTarget=true;image.sprite=UiTheme.Fill();image.type=Image.Type.Sliced;
            var edge=UiTheme.Border(image.rectTransform,UiTheme.WithAlpha(Gold,primary?1f:.8f));
            edge.sprite=UiTheme.Frame();edge.raycastTarget=false;Decorate(image.rectTransform);
            var button=image.gameObject.AddComponent<Button>();button.targetGraphic=image;
            button.onClick.AddListener(()=>Navigate(id));UiButtonFeedback.Add(button);
            var colors=button.colors;colors.highlightedColor=new Color(1.2f,1.2f,1.2f);colors.pressedColor=new Color(.72f,.72f,.72f);colors.fadeDuration=.12f;button.colors=colors;
            var copy=Text(image.transform,"Label",label,.10f,.12f,.79f,.88f,52,Ivory);copy.alignment=TextAlignmentOptions.Center;
            if(primary) {
                Icon(image.transform,"Combat",.04f,.16f,.27f,.84f,Gold);
                Place(copy.rectTransform,.28f,.12f,.96f,.88f);
                var inner=UiTheme.Border(image.rectTransform,UiTheme.WithAlpha(Gold,.55f));inner.sprite=UiTheme.Frame();inner.raycastTarget=false;
                inner.rectTransform.offsetMin=new Vector2(5,5);inner.rectTransform.offsetMax=new Vector2(-5,-5);
            }
            
            return button;
        }
        void Update()
        {
            if(Time.unscaledTime<_nextRefresh)return;_nextRefresh=Time.unscaledTime+.5f;
            if(_driver==null||_driver.Session==null||!_driver.Session.IsAlive)return;
            var view=JsonUtility.FromJson<HomeView>(_driver.Session.ViewJson());
            if(_level!=null && view.grid?.entities!=null)foreach(var member in view.grid.entities)if(member.team=="Player"&&!member.is_pet&&member.stats!=null){_level.text=member.stats.level.ToString();break;}
            _profile.text=view.party_name??view.character_name??"";
            var zone=System.Text.RegularExpressions.Regex.Match(view.current_zone_id??"",@"zone_b(\d+)_(\d+)");
            string stage=zone.Success?Loc.Fmt("UI_HUB_STAGE",zone.Groups[1].Value,zone.Groups[2].Value):Loc.Get("UI_HUB_EXPEDITION");
            if(_expedition.text!=stage){_expedition.text=stage;FitStage(_safeRect.width/Mathf.Max(1,_safeRect.height)>1.7f);}

            _battleState.text=Loc.Get(_driver.WaitingForSave?"UI_REVIEW_SAVE_WAIT":_driver.Paused?"UI_HUB_PAUSED":"UI_HUB_AUTO_BATTLE");
        }
        [Serializable] class HomeView {public string party_name,character_name,current_zone_id; public HomeGrid grid;}
        [Serializable] class HomeGrid {public HomeMember[] entities;}
        [Serializable] class HomeMember {public string team;public bool is_pet;public HomeStats stats;}
        [Serializable] class HomeStats {public int level;}
        public void OpenMore() => Navigate("More");
        void Navigate(string id)
        {
            if(id=="More"){OpenTerminal();return;}
            if(id=="News"){OpenNews();return;}
            if(id=="Combat") { var modes=UnityEngine.Object.FindFirstObjectByType<ModeTabs>();if(modes!=null)modes.Select(true);else _open?.Invoke("Combat");return; }
            if(id=="Party" || id=="Inventory") { _open?.Invoke(id); return; }
            string[] entries = id switch
            {
                "Party" => new[]{"Party","Commander","Paths"},
                "More" => MoreEntries,
                "Mission" => new[]{"Daily","Weekly","Pass","Events"},
                "Inventory" => new[]{"Inventory","Cards","Stash","Album"},
                "Workshop" => new[]{"Apothecary"},
                "Store" => new[]{"Shop","Trade","Crystal"},
                "Secondary3" => new[]{"Bestiary","Record"},
                _ => null
            };
            if(entries != null && entries.Length == 1) { CloseGroup(); _open?.Invoke(entries[0]); return; }
            if (_group != null) Destroy(_group);
            if(entries == null) { _open?.Invoke(id=="Excavation"?"Summon":id=="Event"?"Events":id); return; }
            _groupReturn=UnityEngine.EventSystems.EventSystem.current?.currentSelectedGameObject;
            var dim=UiTheme.Flat(transform,"GroupOverlay",UiTheme.Scrim);
            _group=dim.gameObject;Place(dim.rectTransform,0,0,1,1);
            var dismiss=dim.gameObject.AddComponent<Button>();dismiss.targetGraphic=dim;
            dismiss.transition=Selectable.Transition.None;dismiss.onClick.AddListener(()=>CloseGroup());
            dismiss.navigation=new Navigation{mode=Navigation.Mode.None};
            var panel=UiTheme.Box(dim.transform,"Group",UiTheme.Panel);
            DialogSurface.Apply(panel);
            DialogSurface.CloseIcon(panel.transform,()=>CloseGroup());
            Place(panel.rectTransform,.23f,.08f,.89f,.88f);
            RoundedFrame(panel);
            string title=id switch {"More"=>Loc.Get("UI_HOME_MORE"),"Workshop"=>Loc.Get("UI_HOME_WORKSHOP"),"Store"=>Loc.Get("UI_HOME_STORE"),"Mission"=>Loc.Get("UI_HOME_MISSION"),"Secondary3"=>Loc.Get("UI_HOME_ACHIEVEMENT"),_=>GameText.ScreenName(id)};
            Text(panel.transform,"GroupTitle",title,.07f,.85f,.83f,.95f,40,UiTheme.TextHigh);
            var scrollGo=new GameObject("Choices",typeof(RectTransform),typeof(Image),typeof(RectMask2D),typeof(ScrollRect));
            scrollGo.transform.SetParent(panel.transform,false);
            var viewport=(RectTransform)scrollGo.transform;Place(viewport,.06f,.20f,.94f,.83f);
            scrollGo.GetComponent<Image>().color=Color.clear;
            var content=new GameObject("Content",typeof(RectTransform)).GetComponent<RectTransform>();content.SetParent(viewport,false);
            content.anchorMin=new Vector2(0,1);content.anchorMax=Vector2.one;content.pivot=new Vector2(.5f,1);
            content.anchoredPosition=Vector2.zero;
            int columns=entries.Length>4?2:1;
            content.sizeDelta=new Vector2(0,Mathf.CeilToInt(entries.Length/(float)columns)*132);
            var scroll=scrollGo.GetComponent<ScrollRect>();scroll.viewport=viewport;scroll.content=content;scroll.horizontal=false;scroll.movementType=ScrollRect.MovementType.Clamped;
            if(entries.Length>4) {
                var track=Panel(panel.transform,"ScrollTrack",.952f,.20f,.957f,.83f,new Color(.3f,.35f,.33f,.4f));
                var handle=Panel(track.transform,"Handle",0,0,1,1,Gold);handle.raycastTarget=true;
                var bar=track.gameObject.AddComponent<Scrollbar>();bar.handleRect=handle.rectTransform;bar.targetGraphic=handle;
                bar.direction=Scrollbar.Direction.BottomToTop;scroll.verticalScrollbar=bar;
            }
            Button first=null;
            var choices=new System.Collections.Generic.List<Button>();
            for(int i=0;i<entries.Length;i++) {
                string entry=entries[i];
                var b=LedgerLink(content,entry,GameText.ScreenName(entry),0,0,1,1,40);
                Icon(b.transform,entry,.035f,.25f,.21f,.75f,Gold);
                b.onClick.RemoveAllListeners();b.onClick.AddListener(()=>{CloseGroup();_open?.Invoke(entry);});
                var br=(RectTransform)b.transform;float col=i%columns;
                br.anchorMin=new Vector2(col/columns,1);br.anchorMax=new Vector2((col+1)/columns,1);
                br.pivot=new Vector2(.5f,1);br.offsetMin=new Vector2(6,-(i/columns)*132-120);br.offsetMax=new Vector2(-6,-(i/columns)*132);
                if(first==null)first=b;choices.Add(b);
            }
            var close=HudBar.MakeButton(panel.transform,Loc.Get("UI_COMMON_CANCEL"),"",200,48,()=>CloseGroup());
            Place((RectTransform)close.transform,.57f,.025f,.93f,.175f);
            var closeLabel=close.GetComponentInChildren<TextMeshProUGUI>();closeLabel.fontSizeMax=40;closeLabel.fontSizeMin=36;
            choices.Add(close);
            for(int i=0;i<choices.Count;i++)choices[i].navigation=new Navigation {
                mode=Navigation.Mode.Explicit,
                selectOnUp=choices[(i+choices.Count-1)%choices.Count],
                selectOnDown=choices[(i+1)%choices.Count]
            };
            first?.Select();
            UiEntrance.Play(panel.rectTransform);
        }

        void OpenNews()
        {
            CloseGroup();
            var dim=UiTheme.Flat(transform,"NewsOverlay",UiTheme.Scrim);_group=dim.gameObject;Place(dim.rectTransform,0,0,1,1);
            var panel=UiTheme.Box(dim.transform,"News",UiTheme.Panel);
            DialogSurface.Apply(panel);DialogSurface.CloseIcon(panel.transform,()=>CloseGroup());Place(panel.rectTransform,.26f,.20f,.74f,.80f);RoundedFrame(panel);
            Text(panel.transform,"Title",Loc.Get("UI_TITLE_NEWS"),.08f,.78f,.90f,.94f,38,Ivory);
            Text(panel.transform,"Version",Loc.Fmt("UI_TITLE_RELEASE",Roi.Sim.SimSession.SimVersion()),.08f,.61f,.92f,.76f,26,Gold);
            Text(panel.transform,"Body",Loc.Get("UI_TITLE_RELEASE_NOTES"),.08f,.22f,.92f,.59f,26,Ivory);
            var close=HudBar.MakeButton(panel.transform,Loc.Get("UI_COMMON_CANCEL"),"",200,48,()=>CloseGroup());Place((RectTransform)close.transform,.55f,.04f,.92f,.19f);
            UiEntrance.Play(panel.rectTransform);
        }
        public bool CloseGroup()
        {
            if(_group==null)return false;
            Destroy(_group);_group=null;
            if(_groupReturn!=null&&_groupReturn.activeInHierarchy)
                UnityEngine.EventSystems.EventSystem.current?.SetSelectedGameObject(_groupReturn);
            _groupReturn=null;return true;
        }
        void OnDisable(){CloseGroup();}
        static void Place(RectTransform rt,float x,float y,float right,float top)
        {rt.anchorMin=new Vector2(x,y);rt.anchorMax=new Vector2(right,top);rt.offsetMin=rt.offsetMax=Vector2.zero;}
        static Image Panel(Transform p,string name,float x,float y,float r,float t,Color color)
        {
            var go=new GameObject(name,typeof(RectTransform),typeof(Image));go.transform.SetParent(p,false);
            var im=go.GetComponent<Image>();im.color=color;im.raycastTarget=false;Place(im.rectTransform,x,y,r,t);return im;
        }
        static void Picture(Transform p,string name,string resource,float x,float y,float r,float t,Color color)
        {
            var tex=Resources.Load<Texture2D>(resource);if(tex==null)return;
            var go=new GameObject(name,typeof(RectTransform),typeof(RawImage));go.transform.SetParent(p,false);
            var im=go.GetComponent<RawImage>();im.texture=tex;im.color=color;im.raycastTarget=false;Place(im.rectTransform,x,y,r,t);
            go.AddComponent<HomeImageCrop>();
        }
        TextMeshProUGUI Text(Transform p,string name,string value,float x,float y,float r,float t,float size,Color color)
        {
            var label=UiTheme.Label(p,name,value,size,color,TextAlignmentOptions.CaplineLeft);
            if(_font!=null){label.font=_font;label.fontSharedMaterial=_font.material;label.characterSpacing=0;}
            Place(label.rectTransform,x,y,r,t);label.enableAutoSizing=true;label.fontSizeMin=size*.65f;label.fontSizeMax=size;return label;
        }
        static void RoundedFrame(Image panel)
        {
            panel.color=UiTheme.WithAlpha(UiTheme.Panel,.94f);panel.sprite=UiTheme.Fill();panel.type=Image.Type.Sliced;
            var frame=UiTheme.Border(panel.rectTransform,UiTheme.WithAlpha(Gold,.55f));frame.sprite=UiTheme.Frame();frame.raycastTarget=false;Decorate(panel.rectTransform);
        }
        static void Decorate(RectTransform parent)
        {
            foreach(float x in new[]{0f,1f})foreach(float y in new[]{0f,1f}) {
                float sx=x==0?1:-1,sy=y==0?1:-1;
                for(int i=0;i<3;i++) {
                    var line=UiTheme.Flat(parent,"GoldCorner",UiTheme.WithAlpha(Gold,.85f));line.raycastTarget=false;
                    var r=line.rectTransform;r.anchorMin=r.anchorMax=new Vector2(x,y);r.sizeDelta=new Vector2(i==1?12:20,1);
                    r.anchoredPosition=new Vector2(sx*(9+i*4),sy*(9+i*3));r.localEulerAngles=new Vector3(0,0,sx*sy*(i==1?55:25));
                }
                var d=UiTheme.Flat(parent,"GoldPoint",Gold);d.raycastTarget=false;var dr=d.rectTransform;
                dr.anchorMin=dr.anchorMax=new Vector2(x,y);dr.anchoredPosition=new Vector2(sx*9,sy*9);dr.sizeDelta=new Vector2(4,4);dr.localEulerAngles=new Vector3(0,0,45);
            }
        }
        static RectTransform Icon(Transform p,string name,float x,float y,float r,float t,Color color)
        {
            var go=new GameObject(name+"Icon",typeof(RectTransform));go.transform.SetParent(p,false);
            var icon=go.AddComponent<HomeIcon>();icon.Kind=name=="News"?"Story":name;icon.color=color;icon.raycastTarget=false;
            Place(icon.rectTransform,x,y,r,t);return icon.rectTransform;
        }
        static void Ornament(Transform p,string name,HomeCelestialArt.Motif kind,float x,float y,float r,float t)
        {
            var go=new GameObject(name,typeof(RectTransform),typeof(CanvasRenderer));go.transform.SetParent(p,false);
            var art=go.AddComponent<HomeCelestialArt>();art.Kind=kind;art.raycastTarget=false;
            Place(art.rectTransform,x,y,r,t);
        }
        void SmallIcon(Transform p,string name,float x,float y,float r,float t)
        {
            var image=Panel(p,name,x,y,r,t,Color.clear);image.raycastTarget=true;
            RoundedFrame(image);
            var button=image.gameObject.AddComponent<Button>();button.targetGraphic=image;button.onClick.AddListener(()=>Navigate(name));
            Icon(image.transform,name,.28f,.28f,.72f,.72f,new Color(.90f,.82f,.65f));
            UiButtonFeedback.Add(button);
        }
        void Card(Transform p,string name,string text,float x,float y,float r,float t,float size,string art=null,bool dark=false,Color? accent=null)
        {
            bool hero=name=="Story";
            var image=Panel(p,name,x,y,r,t,hero?UiTheme.SurfaceHover:UiTheme.WithAlpha(UiTheme.Surface,.9f));
            image.sprite=UiTheme.Fill();image.type=Image.Type.Sliced;
            UiTheme.Border(image.rectTransform,UiTheme.LineStrong);
            image.raycastTarget=true;
            var button=image.gameObject.AddComponent<Button>();button.targetGraphic=image;
            button.navigation=new Navigation{mode=Navigation.Mode.None};
            var colors=button.colors;colors.highlightedColor=new Color(1.65f,1.65f,1.5f);colors.pressedColor=new Color(.8f,.8f,.8f);colors.fadeDuration=UiTheme.HoverDuration;button.colors=colors;
            button.onClick.AddListener(()=>Navigate(name));UiButtonFeedback.Add(button);
            var c=image.transform;
            FuturistChrome.Attach(image.rectTransform);
            if(hero)
            {
                Picture(c,"Landscape","HomeUI/CastleBanner",0,0,1,1,new Color(.7f,.75f,.85f,.65f));
                Panel(c,"TitleShade",0,0,1,.47f,new Color(.025f,.055f,.09f,.94f));
                var title=Text(c,"Label",text,.10f,.18f,.80f,.45f,27,UiTheme.TextHigh);
                title.font=UiTheme.Font;title.fontSharedMaterial=UiTheme.Material;
                var note=Text(c,"Availability",Loc.Get("UI_HOME_COMING_SOON"),.10f,.075f,.86f,.19f,12,UiTheme.TextMid);
                note.alignment=TextAlignmentOptions.Left;
                Text(c,"Arrow","›",.91f,.13f,.98f,.42f,25,UiTheme.Accent);
            }
            else
            {
                var iconArea=Panel(c,"IconArea",.18f,.29f,.82f,.96f,Color.clear);
                var medallion=Panel(iconArea.transform,"Medallion",0,0,1,1,Color.clear);
                var square=medallion.gameObject.AddComponent<AspectRatioFitter>();
                square.aspectMode=AspectRatioFitter.AspectMode.FitInParent;square.aspectRatio=1;
                Icon(medallion.transform,name=="Excavation"?"TartarusWolf":name,.25f,.25f,.75f,.75f,Color.white);
                var label=Text(c,"Label",text,0,.055f,1,.27f,size,UiTheme.TextHigh);
                label.font=UiTheme.Font;label.fontSharedMaterial=UiTheme.Material;
                label.alignment=TextAlignmentOptions.Center;label.characterSpacing=.1f;
            }

        }

    }
    public sealed class HomeLedgerShade : MaskableGraphic
    {
        public float FadeWidth=.32f,Opacity=.83f;
        public override Material defaultMaterial => HomeIconSet.Material ?? base.defaultMaterial;
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();var r=rectTransform.rect;
            var left=new Color(.025f,.042f,.038f,0);var right=new Color(.025f,.042f,.038f,Opacity);
            float middle=r.xMin+r.width*FadeWidth;
            vh.AddVert(new Vector2(r.xMin,r.yMin),left,Vector2.zero);vh.AddVert(new Vector2(r.xMin,r.yMax),left,Vector2.zero);
            vh.AddVert(new Vector2(middle,r.yMax),right,Vector2.zero);vh.AddVert(new Vector2(middle,r.yMin),right,Vector2.zero);
            vh.AddTriangle(0,1,2);vh.AddTriangle(0,2,3);
            vh.AddVert(new Vector2(r.xMax,r.yMax),right,Vector2.zero);vh.AddVert(new Vector2(r.xMax,r.yMin),right,Vector2.zero);
            vh.AddTriangle(3,2,4);vh.AddTriangle(3,4,5);
        }
    }
    // Static, low-contrast engraving; generated once when the layout changes.
    public sealed class HomeLedgerEngraving : MaskableGraphic
    {
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();var r=rectTransform.rect;
            var center=new Vector2(r.xMin+r.width*.58f,r.yMin+r.height*.63f);
            float radius=r.height*.34f;
            for(int ring=0;ring<2;ring++)for(int i=0;i<60;i++) {
                float a=(i*3+8)*Mathf.Deg2Rad,b=((i+1)*3+8)*Mathf.Deg2Rad;
                var pa=center+new Vector2(Mathf.Cos(a),Mathf.Sin(a))*(radius+ring*10);
                var pb=center+new Vector2(Mathf.Cos(b),Mathf.Sin(b))*(radius+ring*10);
                Line(vh,pa,pb,.8f,new Color(.8f,.7f,.43f,.14f));
                if(ring==0&&i%5==0)Line(vh,pa,Vector2.Lerp(pa,center,.035f),1,new Color(.8f,.7f,.43f,.22f));
            }
        }
        static void Line(VertexHelper vh,Vector2 a,Vector2 b,float width,Color c)
        {
            var d=(b-a).normalized;var n=new Vector2(-d.y,d.x)*width*.5f;int i=vh.currentVertCount;
            vh.AddVert(a-n,c,Vector2.zero);vh.AddVert(a+n,c,Vector2.zero);vh.AddVert(b+n,c,Vector2.zero);vh.AddVert(b-n,c,Vector2.zero);
            vh.AddTriangle(i,i+1,i+2);vh.AddTriangle(i,i+2,i+3);
        }
    }
    public sealed class HomeImageCrop : MonoBehaviour
    {
        RawImage _image;Vector2 _size;
        public float VerticalAnchor=.5f;
        void LateUpdate()
        {
            if(_image==null)_image=GetComponent<RawImage>();var size=_image.rectTransform.rect.size;
            if(size==_size||size.y<=0||_image.texture==null)return;_size=size;
            float src=(float)_image.texture.width/_image.texture.height, dst=size.x/size.y;
            if(dst>src){float h=src/dst;_image.uvRect=new Rect(0,(1-h)*VerticalAnchor,1,h);}else{float w=dst/src;_image.uvRect=new Rect((1-w)*.5f,0,w,1);}
        }
    }
}


