using System;
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.InputSystem;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed partial class TitleScreen
    {
        RectTransform _canvasRoot, _landing, _rail, _logo, _popup;
        RawImage _backdrop;
        Image _modalShade;
        Button _accountBack;
        readonly List<Selectable> _landingOrder = new();
        readonly List<Selectable> _popupOrder = new();
        string _serverEndpoint;
        Vector2Int _lastScreen;
        Rect _lastSafe;

        void BuildSafeArea()
        {
            _canvasRoot = _root;
            _root = Rect(_canvasRoot, "SafeArea");
            Stretch(_root);
        }

        static RectTransform Rect(Transform parent, string name)
        {
            var go = new GameObject(name, typeof(RectTransform));
            go.transform.SetParent(parent, false);
            return (RectTransform)go.transform;
        }

        void BuildLandingTools(string endpoint)
        {
            _serverEndpoint = Uri.TryCreate(endpoint, UriKind.Absolute, out var uri) &&
                (uri.Scheme == "https" || uri.Scheme == "http") ? endpoint : Roi.Net.OnlineService.ServerUrl;
            _rail=Rect(_landing,"UtilityRail");
            Pin(_rail,Vector2.zero,new Vector2(116,432));
            var settings=RailButton("Settings", "UI_TITLE_SETTINGS",TitleGlyph.Symbol.Settings,0,OpenSettings);
            var news=RailButton("News", "UI_TITLE_NEWS",TitleGlyph.Symbol.News,1,OpenNews);
            var support=RailButton("Support", "UI_TITLE_SUPPORT",TitleGlyph.Symbol.Support,2,OpenSupport);
            var account=RailButton("Account", "UI_TITLE_ACCOUNT",TitleGlyph.Symbol.Account,3,OpenSignIn);
            RegisterFocus(_landingOrder,_tapCatcher,settings,news,support,account);
        }

        Button RailButton(string name,string key,TitleGlyph.Symbol symbol,int row,Action click)
        {
            var button=QuietButton(_rail,name,Loc.Get(key),click);
            Anchor((RectTransform)button.transform,new Vector2(0,-row*104),new Vector2(116,100));
            var label=button.GetComponentInChildren<TextMeshProUGUI>();
            label.fontSize=19;
            Pin(label.rectTransform,new Vector2(0,-30),new Vector2(116,34));
            var icon=TitleGlyph.Add(button.transform,symbol,true);
            Pin(icon.rectTransform,new Vector2(0,15),new Vector2(56,56));
            return button;
        }

        static Button QuietButton(Transform parent,string name,string text,Action click)
        {
            var ground=UiTheme.Flat(parent,name,new Color(1,1,1,0));
            var button=ground.gameObject.AddComponent<Button>(); button.targetGraphic=ground;
            button.navigation=new Navigation { mode=Navigation.Mode.None };
            var label=UiTheme.Label(ground.transform,"Label",text,24,UiTheme.TextHigh,TextAlignmentOptions.Center);
            Stretch(label.rectTransform); label.raycastTarget=false;
            button.onClick.AddListener(()=>click?.Invoke());
            // Tint the text for touch/hover feedback; the transparent hit area stays generous.
            button.targetGraphic=label;
            button.colors=UiTheme.ButtonColors;
            return button;
        }

        void BuildModalShade()
        {
            _modalShade=UiTheme.Flat(_root,"ModalShade",UiTheme.Scrim);
            Stretch(_modalShade.rectTransform); _modalShade.gameObject.SetActive(false);
        }

        // Local title-dialog styling: do not change gameplay dialog surfaces.
        static void StyleTitleDialog(Image panel)
        {
            DialogSurface.Apply(panel);
            TitleOrnament(panel.transform,"DialogCrest",HomeCelestialArt.Motif.Sigil,new Vector2(0,0),new Vector2(92,92),true);
            var header=UiTheme.Flat(panel.transform,"HeaderWash",UiTheme.WithAlpha(UiTheme.SurfaceHover,.35f));
            header.raycastTarget=false;
            var hr=header.rectTransform;
            hr.anchorMin=new Vector2(0,1); hr.anchorMax=Vector2.one; hr.pivot=new Vector2(.5f,1);
            hr.offsetMin=new Vector2(2,-88); hr.offsetMax=new Vector2(-2,-2);
            var rule=UiTheme.Flat(panel.transform,"HeaderRule",UiTheme.LineStrong);
            rule.raycastTarget=false;
            var rt=rule.rectTransform;
            rt.anchorMin=new Vector2(0,1); rt.anchorMax=Vector2.one; rt.pivot=new Vector2(.5f,1);
            rt.offsetMin=new Vector2(28,-89); rt.offsetMax=new Vector2(-28,-88);
            TitleOrnament(panel.transform,"HeadingRule",HomeCelestialArt.Motif.Rule,new Vector2(0,-88),new Vector2(320,24));
        }

        static void TitleOrnament(Transform parent,string name,HomeCelestialArt.Motif motif,Vector2 pos,Vector2 size,bool crest=false)
        {
            var go=new GameObject(name,typeof(RectTransform));go.transform.SetParent(parent,false);
            var art=go.AddComponent<HomeCelestialArt>();art.Kind=motif;art.raycastTarget=false;
            var rt=(RectTransform)go.transform;Anchor(rt,pos,size);
            if(crest) {rt.anchoredPosition=new Vector2(0,48);}
        }

        void HideLanding()
        {
            _landing.gameObject.SetActive(false);
            foreach(var part in _logoParts) part.SetActive(false);
            _modalShade.gameObject.SetActive(true);
        }

        void StyleFantasySignIn(Image panel,TMP_Text heading,TMP_Text fieldLabel,bool steam)
        {
            foreach(var name in new[]{"DialogSurface","HeaderWash","HeaderRule"})
            { var old=panel.transform.Find(name); if(old!=null)old.gameObject.SetActive(false); }
            float height=steam && DevDoors?570:475;
            _signIn.sizeDelta=new Vector2(600,height);
            _signIn.localScale=Vector3.one*.85f;
            panel.color=UiTheme.WithAlpha(UiTheme.Panel,.97f);
            panel.sprite=UiTheme.Fill(8); panel.type=Image.Type.Sliced;
            var gold=UiTheme.Accent;
            var border=UiTheme.Border(_signIn,gold); border.sprite=UiTheme.Frame(8); border.raycastTarget=false;
            heading.fontSize=32;
            TitleOrnament(_signIn,"AccountCrest",HomeCelestialArt.Motif.Sigil,Vector2.zero,new Vector2(84,84),true);
            Anchor(heading.rectTransform,new Vector2(0,-32),new Vector2(440,52));
            var rule=UiTheme.Flat(_signIn,"FantasyDivider",UiTheme.WithAlpha(gold,.65f));
            rule.raycastTarget=false; Anchor(rule.rectTransform,new Vector2(0,-98),new Vector2(320,1));
            FantasyDiamond(_signIn,new Vector2(0,-98),5,gold);
            FantasyDiamond(_signIn,new Vector2(-15,-98),2,gold);
            FantasyDiamond(_signIn,new Vector2(15,-98),2,gold);
            TitleOrnament(_signIn,"AccountRule",HomeCelestialArt.Motif.Rule,new Vector2(0,-98),new Vector2(320,24));
            foreach(float x in new[]{-1f,1f})foreach(float y in new[]{0f,1f})
            {
                float top=y==0?-12:-height+12;
                FantasyDiamond(_signIn,new Vector2(x*286,top),3,gold);
                FantasyDiamond(_signIn,new Vector2(x*278,top+(y==0?-7:7)),2,gold);
            }
            Anchor(fieldLabel.rectTransform,new Vector2(0,-130),new Vector2(512,32));
            Anchor((RectTransform)_nameField.transform,new Vector2(0,-166),new Vector2(512,66));
            var input=_nameField.GetComponent<Image>(); input.sprite=UiTheme.Fill();input.color=UiTheme.Surface;
            foreach(var graphic in input.GetComponentsInChildren<Image>())if(graphic!=input)
            {graphic.sprite=UiTheme.Frame();graphic.color=UiTheme.LineStrong;graphic.raycastTarget=false;}
            Anchor((RectTransform)_play.transform,new Vector2(0,-266),new Vector2(504,90));
            var primary=_play.GetComponent<Image>();primary.sprite=UiTheme.Fill();primary.color=UiTheme.FramePrimary;
            foreach(var graphic in primary.GetComponentsInChildren<Image>())if(graphic!=primary)
            {graphic.sprite=UiTheme.Frame();graphic.color=gold;graphic.raycastTarget=false;}
            _play.GetComponentInChildren<TMP_Text>().color=UiTheme.TextHigh;
            _play.GetComponentInChildren<TMP_Text>().fontSize=30;
            TitleOrnament(_play.transform,"PlayFrame",HomeCelestialArt.Motif.BattleFrame,new Vector2(0,0),new Vector2(504,90));
            var playFrame=(RectTransform)_play.transform.Find("PlayFrame");
            Stretch(playFrame);playFrame.SetAsFirstSibling();
            var glow=UiTheme.Border((RectTransform)_play.transform,UiTheme.WithAlpha(UiTheme.Accent,.28f));
            glow.sprite=UiTheme.Frame(8,2);glow.raycastTarget=false;
            glow.rectTransform.offsetMin=new Vector2(-3,-3);glow.rectTransform.offsetMax=new Vector2(3,3);
            if(_routeSwap!=null)Anchor((RectTransform)_routeSwap.transform,new Vector2(0,-366),new Vector2(512,80));
            if(_dev!=null)
            {
                Anchor((RectTransform)_dev.transform,new Vector2(0,steam?-456:-376),new Vector2(512,72));
                foreach(var graphic in _dev.GetComponentsInChildren<Graphic>())if(!(graphic is TMP_Text))graphic.color=Color.clear;
                var label=_dev.GetComponentInChildren<TMP_Text>();label.color=UiTheme.TextMid;label.fontSize=20;
                _dev.targetGraphic=label;
            }
        }

        static void FantasyDiamond(RectTransform parent,Vector2 position,float size,Color color)
        {
            var mark=UiTheme.Flat(parent,"FantasyDiamond",color);mark.raycastTarget=false;
            Anchor(mark.rectTransform,position,new Vector2(size,size));mark.rectTransform.localEulerAngles=new Vector3(0,0,45);
        }

        void ReturnToLanding()
        {
            if (_busy) return;
            if (_popup != null) { _popup.gameObject.SetActive(false); Destroy(_popup.gameObject); _popup=null; }
            _popupOrder.Clear();
            _signIn.gameObject.SetActive(false); _creation.gameObject.SetActive(false);
            _modalShade.gameObject.SetActive(false);
            _landing.gameObject.SetActive(true);
            _tapCatcher.gameObject.SetActive(true); _tapLabel.gameObject.SetActive(true);
            foreach(var part in _logoParts) part.SetActive(true);
            SetStatus(""); _tapCatcher.Select();
        }

        RectTransform OpenPopup(string key)
        {
            if (_busy) return null;
            HideLanding();
            _popupOrder.Clear();
            var panel=UiTheme.Box(_root,"TitlePopup",UiTheme.Surface,12);
            StyleTitleDialog(panel);
            _popup=panel.rectTransform;
            Pin(_popup,Vector2.zero,new Vector2(780,560));
            var head=UiTheme.Label(_popup,"Heading",Loc.Get(key),30,UiTheme.TextHigh,TextAlignmentOptions.Center);
            Anchor(head.rectTransform,new Vector2(0,-24),new Vector2(570,56));
            AddClose(_popup,_popupOrder);
            SetStatus("");
            FocusFirst(_popupOrder);
            return _popup;
        }

        Button AddClose(RectTransform parent,List<Selectable> order)
        {
            var button=QuietButton(parent,"Close", "",ReturnToLanding);
            var rt=(RectTransform)button.transform;
            rt.anchorMin=rt.anchorMax=rt.pivot=Vector2.one;
            rt.anchoredPosition=new Vector2(-4,-4); rt.sizeDelta=new Vector2(88,88);
            var icon=TitleGlyph.Add(button.transform,TitleGlyph.Symbol.Close);
            Pin(icon.rectTransform,Vector2.zero,new Vector2(40,40));
            button.targetGraphic=icon;
            RegisterFocus(order,button);
            return button;
        }

        TextMeshProUGUI PopupText(string text,float top,float height)
        {
            var t=UiTheme.Label(_popup,"Body",text,23,UiTheme.TextMid,TextAlignmentOptions.TopLeft);
            t.textWrappingMode=TextWrappingModes.Normal; t.overflowMode=TextOverflowModes.Overflow;
            Anchor(t.rectTransform,new Vector2(0,-top),new Vector2(680,height));
            return t;
        }

        void OpenNews()
        {
            if(OpenPopup("UI_TITLE_NEWS")==null)return;
            PopupText(Loc.Fmt("UI_TITLE_RELEASE",Roi.Sim.SimSession.SimVersion()),110,60);
            PopupText(Loc.Get("UI_TITLE_RELEASE_NOTES"),190,280);
        }

        void OpenSupport()
        {
            if(OpenPopup("UI_TITLE_SUPPORT")==null)return;
            PopupText(Loc.Get("UI_TITLE_HELP_BODY"),105,260);
            Button copy=null;
            copy=MakePrimary(_popup,Loc.Get("UI_TITLE_COPY_DETAILS"),680,88,()=> {
                GUIUtility.systemCopyBuffer="Reign of Idlers\nVersion: "+Roi.Sim.SimSession.SimVersion()+
                    "\nServer: "+new Uri(_serverEndpoint).Host+"\nPlatform: "+Application.platform;
                SetLabel(copy,Loc.Get("UI_TITLE_COPIED"));
            });
            copy.gameObject.name="CopyDiagnostics";
            Anchor((RectTransform)copy.transform,new Vector2(0,-418),new Vector2(680,88));
            RegisterFocus(_popupOrder,copy);
        }

        void OpenSettings()
        {
            if(OpenPopup("UI_TITLE_SETTINGS")==null)return;
            SettingsRow("UI_TITLE_MUSIC","roi.opt.music",70,new[]{0,50,100},new[]{"0%","50%","100%"},120);
            SettingsRow("UI_TITLE_SOUND","roi.opt.sfx",80,new[]{0,50,100},new[]{"0%","50%","100%"},250);
            SettingsRow("UI_TITLE_FRAME_RATE","roi.opt.framecap",60,new[]{30,60,120},new[]{"30","60","120"},380);
        }

        void SettingsRow(string key,string preference,int fallback,int[] values,string[] labels,float top)
        {
            var text=UiTheme.Label(_popup,key,Loc.Get(key),23,UiTheme.TextHigh,TextAlignmentOptions.Left);
            Anchor(text.rectTransform,new Vector2(-233,-top),new Vector2(214,88));
            var buttons=new List<Button>();
            for(int i=0;i<values.Length;i++)
            {
                int index=i;
                var button=QuietButton(_popup,preference+"_"+values[i],labels[i],()=> {
                    PlayerPrefs.SetInt(preference,values[index]); PlayerPrefs.Save();
                    Roi.Audio.SoundFx.SetVolumes(PlayerPrefs.GetInt("roi.opt.music",70)/100f,PlayerPrefs.GetInt("roi.opt.sfx",80)/100f);
                    if(preference=="roi.opt.framecap")Application.targetFrameRate=values[index];
                    for(int k=0;k<buttons.Count;k++)buttons[k].GetComponentInChildren<TMP_Text>().color=
                        k==index?UiTheme.Accent:UiTheme.TextMid;
                });
                var rt=(RectTransform)button.transform;
                Anchor(rt,new Vector2(-54+i*142,-top),new Vector2(132,88));
                var plate=UiTheme.Box(rt,"Plate",UiTheme.Surface,8);
                Stretch(plate.rectTransform); plate.raycastTarget=false; plate.transform.SetAsFirstSibling();
                UiTheme.Border(rt,UiTheme.LineStrong);
                button.GetComponentInChildren<TMP_Text>().color=PlayerPrefs.GetInt(preference,fallback)==values[i]?UiTheme.Accent:UiTheme.TextMid;
                buttons.Add(button); RegisterFocus(_popupOrder,button);
            }
        }

        void LayoutLanding(UnityEngine.Rect? safeOverride = null)
        {
            _lastScreen=new Vector2Int(Screen.width,Screen.height); _lastSafe=Screen.safeArea;
            var safe=safeOverride ?? _lastSafe;
            if(safe.width<=0 || safe.height<=0)safe=new UnityEngine.Rect(0,0,Screen.width,Screen.height);
            _root.anchorMin=new Vector2(safe.xMin/Screen.width,safe.yMin/Screen.height);
            _root.anchorMax=new Vector2(safe.xMax/Screen.width,safe.yMax/Screen.height);
            _root.offsetMin=_root.offsetMax=Vector2.zero;
            Canvas.ForceUpdateCanvases();
            float w=_root.rect.width,h=_root.rect.height;
            _rail.anchorMin=_rail.anchorMax=_rail.pivot=new Vector2(1,1);
            _rail.anchoredPosition=new Vector2(-24,-30);
            Pin(_logo,new Vector2(-12,h*.30f),new Vector2(Mathf.Min(960,w-240),150));
            Pin((RectTransform)_tapCatcher.transform,new Vector2(0,-h*.325f),new Vector2(560,100));
            if(_backdrop!=null && _backdrop.texture!=null)
            {
                float imageAspect=(float)_backdrop.texture.width/_backdrop.texture.height;
                float screenAspect=(float)Screen.width/Screen.height;
                _backdrop.uvRect=screenAspect>imageAspect
                    ?new UnityEngine.Rect(0,(1-imageAspect/screenAspect)*.5f,1,imageAspect/screenAspect)
                    :new UnityEngine.Rect((1-screenAspect/imageAspect)*.5f,0,screenAspect/imageAspect,1);
            }
        }

        bool UpdateLandingInput(Keyboard keyboard)
        {
            if(_lastScreen.x!=Screen.width || _lastScreen.y!=Screen.height || _lastSafe!=Screen.safeArea)LayoutLanding();
            if(_busy || keyboard==null)return _popup!=null;
            if(keyboard.escapeKey.wasPressedThisFrame && (_popup!=null || _signIn.gameObject.activeSelf))
            { ReturnToLanding(); return true; }
            if(_popup!=null)
            {
                if(keyboard.tabKey.wasPressedThisFrame)MoveFocus(_popupOrder,keyboard.shiftKey.isPressed?-1:1);
                return true;
            }
            if(!_landing.gameObject.activeSelf)return false;
            if(keyboard.tabKey.wasPressedThisFrame)MoveFocus(_landingOrder,keyboard.shiftKey.isPressed?-1:1);
            var selected=EventSystem.current?.currentSelectedGameObject;
            if((selected==null || selected==_tapCatcher.gameObject) &&
                (keyboard.enterKey.wasPressedThisFrame || keyboard.numpadEnterKey.wasPressedThisFrame || keyboard.spaceKey.wasPressedThisFrame))
                OpenSignIn();
            return true;
        }
    }
}
