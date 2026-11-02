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
            Pin(_rail,Vector2.zero,new Vector2(RailW,RailStep*5+RailExitGap));
            var news=RailButton("News", "UI_TITLE_NEWS",TitleGlyph.Symbol.News,0,OpenNews);
            var support=RailButton("Support", "UI_TITLE_SUPPORT",TitleGlyph.Symbol.Help,1,OpenSupport);
            var account=RailButton("Account","UI_TITLE_ACCOUNT",TitleGlyph.Symbol.Account,2,OpenSignIn);
            var settings=RailButton("Settings", "UI_TITLE_SETTINGS",TitleGlyph.Symbol.Settings,3,OpenSettings);
            // Exit is destructive: last in the rail, set apart by a gap and a short rule.
            var divider=UiTheme.Flat(_rail,"ExitDivider",UiTheme.LineStrong); divider.raycastTarget=false;
            Anchor(divider.rectTransform,new Vector2(0,-4*RailStep-RailExitGap*.5f+2),new Vector2(36,1)); // centred in the gap above Exit
            var exit=RailButton("Exit","UI_TITLE_EXIT",TitleGlyph.Symbol.Exit,4,ExitTitle,RailExitGap);
            RegisterFocus(_landingOrder,_tapCatcher,news,support,account,settings,exit);
            BuildServerPill();
        }

        // Which server this title signs in to, in the slot a gacha title uses for its region.
        // Display only: there is one server, so a tappable picker would be a dead control.
        void BuildServerPill()
        {
            var pill=UiTheme.Flat(_landing,"ServerPill",new Color(.04f,.05f,.06f,.55f));
            pill.sprite=UiTheme.CutFill(4); pill.type=Image.Type.Sliced; pill.raycastTarget=false;
            _serverPill=pill.rectTransform;
            // Neutral styling: the gold accent belongs to the Start Game call to action.
            UiTheme.Border(_serverPill,UiTheme.LineStrong);
            var label=UiTheme.Label(_serverPill,"Label",Loc.Get("UI_TITLE_SERVER_MAIN"),15,UiTheme.TextMid,TextAlignmentOptions.Center);
            label.raycastTarget=false; Stretch(label.rectTransform);
        }

        RectTransform _serverPill;
        const float RailW=96f, RailStep=64f, RailExitGap=24f;

        static void ExitTitle(){
#if UNITY_EDITOR
            UnityEditor.EditorApplication.isPlaying=false;
#else
            Application.Quit();
#endif
        }

        Button RailButton(string name,string key,TitleGlyph.Symbol symbol,int row,Action click,float extraOffset=0f)
        {
            var button=QuietButton(_rail,name,Loc.Get(key),click);
            // 96x60 hit area keeps the target generous while the visible mark stays small.
            Anchor((RectTransform)button.transform,new Vector2(0,-row*RailStep-extraOffset),new Vector2(RailW,60));
            var label=button.GetComponentInChildren<TextMeshProUGUI>();
            label.fontSize=13;
            Pin(label.rectTransform,new Vector2(0,-15),new Vector2(RailW,20));
            var icon=TitleGlyph.Add(button.transform,symbol,false);
            icon.color=new Color(.92f,.96f,.96f,1);
            Pin(icon.rectTransform,new Vector2(0,9),new Vector2(30,30));
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
            bool shadeWasHidden=!_modalShade.gameObject.activeSelf;
            _landing.gameObject.SetActive(false);
            foreach(var part in _logoParts) part.SetActive(false);
            _modalShade.gameObject.SetActive(true);
            if(shadeWasHidden) FadeShadeIn();
        }

        void StyleFantasySignIn(Image panel,TMP_Text heading,TMP_Text fieldLabel,bool steam)
        {
            panel.GetComponent<NoticeDialogStyle>().CompactLogin=true;
            foreach(var name in new[]{"HeaderWash","HeaderRule","HeadingRule","DialogCrest"}) {
                var old=panel.transform.Find(name);if(old!=null)old.gameObject.SetActive(false);
            }
            float height=steam && DevDoors?360:304;
            _signIn.sizeDelta=new Vector2(420,height);
            _signIn.localScale=Vector3.one;
            var header=UiTheme.Flat(_signIn,"LoginHeader",new Color(.055f,.065f,.068f));
            header.raycastTarget=false;
            Anchor(header.rectTransform,Vector2.zero,new Vector2(420,60));
            header.transform.SetAsFirstSibling();
            heading.text="REIGN OF IDLERS";
            heading.characterSpacing=3;
            Anchor(heading.rectTransform,new Vector2(-12,-12),new Vector2(320,36));
            Anchor(fieldLabel.rectTransform,new Vector2(0,-78),new Vector2(364,24));
            Anchor((RectTransform)_nameField.transform,new Vector2(0,-106),new Vector2(364,46));
            Anchor((RectTransform)_play.transform,new Vector2(0,-174),new Vector2(364,44));
            if(_routeSwap!=null)Anchor((RectTransform)_routeSwap.transform,new Vector2(0,-230),new Vector2(364,44));
            if(_dev!=null)Anchor((RectTransform)_dev.transform,new Vector2(0,steam?-286:-230),new Vector2(364,44));
            var close=(RectTransform)_accountBack.transform;
            close.anchorMin=close.anchorMax=close.pivot=Vector2.one;
            close.anchoredPosition=new Vector2(-6,-6);close.sizeDelta=new Vector2(48,48);
        }

        static void FantasyDiamond(RectTransform parent,Vector2 position,float size,Color color)
        {
            var mark=UiTheme.Flat(parent,"FantasyDiamond",color);mark.raycastTarget=false;
            Anchor(mark.rectTransform,position,new Vector2(size,size));mark.rectTransform.localEulerAngles=new Vector3(0,0,45);
        }

        void ReturnToLanding()
        {
            if (_busy) return;
            _version.gameObject.SetActive(true);
            if (_popup != null) { _popup.gameObject.SetActive(false); Destroy(_popup.gameObject); _popup=null; }
            _popupOrder.Clear();
            _signIn.gameObject.SetActive(false); _creation.gameObject.SetActive(false);
            _modalShade.gameObject.SetActive(false);
            _landing.gameObject.SetActive(true);
            _tapCatcher.gameObject.SetActive(true); _tapLabel.gameObject.SetActive(true);
            foreach(var part in _logoParts) part.SetActive(true);
            PlayEntrance((RectTransform)_tapCatcher.transform,0f,.25f,8f);
            // Nothing pre-selected: the focus ring appears only once the player uses Tab.
            // Enter/Space still start the game with no selection (UpdateLandingInput).
            SetStatus(""); EventSystem.current?.SetSelectedGameObject(null);
        }

        RectTransform OpenPopup(string key)
        {
            if (_busy) return null;
            HideLanding();
            _popupOrder.Clear();
            var panel=UiTheme.Box(_root,"TitlePopup",UiTheme.Surface,12);
            StyleTitleDialog(panel);
            _popup=panel.rectTransform;
            Pin(_popup,Vector2.zero,new Vector2(680,400));
            var head=UiTheme.Label(_popup,"Heading",Loc.Get(key),30,UiTheme.TextHigh,TextAlignmentOptions.Center);
            Anchor(head.rectTransform,new Vector2(-34,-16),new Vector2(548,56));
            AddClose(_popup,_popupOrder);
            SetStatus("");
            FocusFirst(_popupOrder);
            OpenPanelMotion(_popup);
            return _popup;
        }

        Button AddClose(RectTransform parent,List<Selectable> order)
        {
            var button=QuietButton(parent,"Close", "",ReturnToLanding);
            var rt=(RectTransform)button.transform;
            rt.anchorMin=rt.anchorMax=rt.pivot=Vector2.one;
            rt.anchoredPosition=new Vector2(-12,-16); rt.sizeDelta=new Vector2(48,48);
            var icon=TitleGlyph.Add(button.transform,TitleGlyph.Symbol.Close);
            Pin(icon.rectTransform,Vector2.zero,new Vector2(28,28));
            button.targetGraphic=icon;
            RegisterFocus(order,button);
            return button;
        }

        TextMeshProUGUI PopupText(string text,float top,float height)
        {
            var t=UiTheme.Label(_popup,"Body",text,23,UiTheme.TextMid,TextAlignmentOptions.TopLeft);
            t.textWrappingMode=TextWrappingModes.Normal; t.overflowMode=TextOverflowModes.Overflow;
            Anchor(t.rectTransform,new Vector2(0,-top),new Vector2(616,height));
            return t;
        }

        void OpenNews()
        {
            if(OpenPopup("UI_TITLE_NEWS")==null)return;
            PopupText(Loc.Fmt("UI_TITLE_RELEASE",Roi.Sim.SimSession.SimVersion()),112,32);
            PopupText(Loc.Get("UI_TITLE_RELEASE_NOTES"),160,200);
        }

        void OpenSupport()
        {
            if(OpenPopup("UI_TITLE_SUPPORT")==null)return;
            PopupText(Loc.Get("UI_TITLE_HELP_BODY"),112,180);
            Button copy=null;
            copy=MakePrimary(_popup,Loc.Get("UI_TITLE_COPY_DETAILS"),280,44,()=> {
                GUIUtility.systemCopyBuffer="Reign of Idlers\nVersion: "+Roi.Sim.SimSession.SimVersion()+
                    "\nServer: "+new Uri(_serverEndpoint).Host+"\nPlatform: "+Application.platform;
                SetLabel(copy,Loc.Get("UI_TITLE_COPIED"));
            });
            copy.gameObject.name="CopyDiagnostics";
            Anchor((RectTransform)copy.transform,new Vector2(0,-320),new Vector2(280,44));
            RegisterFocus(_popupOrder,copy);
        }

        void OpenSettings() => OpenSettingsPage(0);

        void SettingsRow(string key,string preference,int fallback,int[] values,string[] labels,float top)
        {
            var text=UiTheme.Label(_popup,key,Loc.Get(key),23,UiTheme.TextHigh,TextAlignmentOptions.Left);
            Anchor(text.rectTransform,new Vector2(-220,-top),new Vector2(176,44));
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
                Anchor(rt,new Vector2(-70+i*118,-top),new Vector2(108,44));
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
            _rail.anchoredPosition=new Vector2(-8,-h*.09f);
            _logo.anchorMin=_logo.anchorMax=_logo.pivot=new Vector2(0,1); _logo.anchoredPosition=new Vector2(22,-22); _logo.sizeDelta=new Vector2(84,36);
            Pin((RectTransform)_tapCatcher.transform,new Vector2(0,-h*.5f+h*.043f),new Vector2(w*.77f,38));
            Pin(_serverPill,new Vector2(0,-h*.5f+h*.043f+80),new Vector2(200,34));
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
            if(_lastScreen.x!=Screen.width || _lastScreen.y!=Screen.height || _lastSafe!=Screen.safeArea){FinishIntro();LayoutLanding();}
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
