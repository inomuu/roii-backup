// OPTIONS — the settings that this client actually has, and nothing else.
//
// `options_view.gd` also carries audio buses, a language picker and a new-game reset. None of the
// three exists on this side yet: there is no audio in the Unity port at all, no localisation layer
// (`GameText` derives English from ids), and "new game" now means an ACCOUNT, which is the login
// screen's business rather than a settings row. A screen full of controls that do nothing is worse
// than a short one, so this says what is missing instead of drawing dead widgets.
//
// SEGMENTED, NOT SLIDERS. Every setting here is a small set of named choices, and a row of buttons
// is both readable at a glance and thumb-sized — a slider is a drag, which this UI does not use
// (2026-09-07).
//
// SAVED IN PlayerPrefs and applied at BOOT, not only when the window is opened: a frame cap that
// only takes effect after visiting a screen is a frame cap nobody set.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class OptionsWindow : MonoBehaviour
    {
        const string KeyFullscreen = "roi.opt.fullscreen";
        const string KeyVsync = "roi.opt.vsync";
        const string KeyFrameCap = "roi.opt.framecap";
        const string KeyPostFx = "roi.opt.postfx";
        const string KeyStyle = "roi.opt.style";
        const string KeyMusic = "roi.opt.music";
        const string KeySfx = "roi.opt.sfx";

        /// The five steps a volume takes. NOT a slider, and not for want of one: a slider is a
        /// DRAG, and this UI has no drags (2026-09-07 — mobile is planned, and a drag fights touch
        /// scrolling). Five taps cover every setting anyone actually uses.
        static readonly int[] VolumeSteps = { 0, 25, 50, 75, 100 };
        static readonly string[] VolumeLabels = { "Off", "25", "50", "75", "100" };

        const float RowH = 82f, BtnW = 116f, BtnH = 44f;

        RectTransform _content;
        int _page;
        TMP_Text _heading;
        TitleGlyph _headingIcon;
        ShopSelectionArc _arc;
        readonly List<MenuNavigationButton> _tabs=new();

        TextMeshProUGUI _status;
        readonly List<GameObject> _built = new();

        public static OptionsWindow Attach(RectTransform body)
        {
            var w = body.gameObject.AddComponent<OptionsWindow>();
            w.Build(body);
            return w;
        }

        /// Apply what was saved, before anything is drawn. Called by the boot.
        ///
        /// SAVED, not defaulted. Every one of these reads `HasKey` first: a value written here for
        /// a setting nobody touched is not a default, it is this screen overruling the project's
        /// own Player Settings and the scene's own build — which is exactly how a boot ended up
        /// imposing HD-2D on a zone baked as Full3D and tearing its dressing apart.
        public static void ApplySaved()
        {
            if (PlayerPrefs.HasKey(KeyFullscreen))
                Screen.fullScreenMode = PlayerPrefs.GetInt(KeyFullscreen) == 1
                    ? FullScreenMode.FullScreenWindow : FullScreenMode.Windowed;
            if (PlayerPrefs.HasKey(KeyVsync)) QualitySettings.vSyncCount = PlayerPrefs.GetInt(KeyVsync);
            // The two volumes DO get a default written through, unlike the display settings: there
            // is no project setting or scene bake underneath them for this to overrule — silence
            // would be the only alternative, and an idle game left running is exactly the case
            // where the music should start where the player last put it.
            Roi.Audio.SoundFx.SetVolumes(PlayerPrefs.GetInt(KeyMusic, 70) / 100f,
                                         PlayerPrefs.GetInt(KeySfx, 80) / 100f);
            if (PlayerPrefs.HasKey(KeyFrameCap)) Application.targetFrameRate = PlayerPrefs.GetInt(KeyFrameCap);
            ApplyToArena();
        }

        /// The two settings the ARENA owns. Re-applied on demand because the arena is built after
        /// the boot runs, and a setting applied to a camera that does not exist yet is lost.
        ///
        /// ONLY WHAT THE PLAYER ACTUALLY CHOSE. A default written here is not a default, it is an
        /// OVERRIDE: `Zone_r1` is baked as Full3D (`Style: 1`, `EnvBaked: 1`) and a boot that
        /// imposed "HD-2D" on it tore the dressing apart — terrain gone, props and god rays still
        /// standing over a dark ground. That is what an untouched setting looks like when someone
        /// gives it an opinion, so an absent key means "leave the scene as it was built".
        static void ApplyToArena()
        {
            var arena = Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
            if (arena == null) return;
            if (PlayerPrefs.HasKey(KeyPostFx)) arena.SetPostFx(PlayerPrefs.GetInt(KeyPostFx) == 1);
            if (PlayerPrefs.HasKey(KeyStyle))
                arena.SetStyle(PlayerPrefs.GetInt(KeyStyle) == 0
                    ? Roi.Arena.ArenaView.ZoneStyle.HD2D
                    : Roi.Arena.ArenaView.ZoneStyle.Full3D);
        }

        /// What the screen should show for a setting nobody has touched: the scene's own value,
        /// not this file's guess.
        static int StyleNow()
        {
            if (PlayerPrefs.HasKey(KeyStyle)) return PlayerPrefs.GetInt(KeyStyle);
            var arena = Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
            return arena != null && arena.Style == Roi.Arena.ArenaView.ZoneStyle.Full3D ? 1 : 0;
        }

        void Build(RectTransform body)
        {
            var window=body.GetComponentInParent<UiWindow>();
            window.SetImmersive();
            var panel=(RectTransform)body.parent;
            foreach(Transform child in panel)if(child!=body)child.gameObject.SetActive(false);
            var dialog=panel.GetComponent<NoticeDialogStyle>();if(dialog!=null)dialog.enabled=false;
            panel.GetComponent<Image>().sprite=null;
            panel.GetComponent<Image>().color=UiTheme.WithAlpha(UiTheme.Panel,.9f);
            _heading=UiTheme.Label(body,"Heading","",25,UiTheme.TextHigh,TextAlignmentOptions.Left);
            Position(_heading.rectTransform,74,20,600,48);
            _headingIcon=TitleGlyph.Add(body,TitleGlyph.Symbol.Support);Position(_headingIcon.rectTransform,28,26,34,34);
            var close=HudBar.MakeGhostButton(body,"",48,48,()=>window.CloseSettings());
            var cr=(RectTransform)close.transform;cr.anchorMin=cr.anchorMax=cr.pivot=Vector2.one;cr.anchoredPosition=new Vector2(-22,-18);
            var ci=TitleGlyph.Add(close.transform,TitleGlyph.Symbol.Close);ci.rectTransform.sizeDelta=new Vector2(30,30);
            for(int i=0;i<3;i++){int page=i;var b=CollectionBrowser.Quiet(body,"Tab"+i,"",0,0,1,1,()=>{_page=page;Refresh();});Position((RectTransform)b.transform,20,100+i*94,64,72);b.IsTab=true;var icon=TitleGlyph.Add(b.transform,i==0?TitleGlyph.Symbol.Support:i==1?TitleGlyph.Symbol.Settings:TitleGlyph.Symbol.Globe);icon.rectTransform.sizeDelta=new Vector2(40,40);b.CategoryIcon=icon;_tabs.Add(b);}
            var seam=new GameObject("CategorySeam",typeof(RectTransform));seam.transform.SetParent(body,false);var seamRect=(RectTransform)seam.transform;seamRect.anchorMin=new Vector2(0,0);seamRect.anchorMax=new Vector2(0,1);seamRect.pivot=new Vector2(0,1);seamRect.offsetMin=new Vector2(53,20);seamRect.offsetMax=new Vector2(117,-80);_arc=seam.AddComponent<ShopSelectionArc>();_arc.raycastTarget=false;
            float pad = 132;

            _status = UiTheme.Label(body, "Status", "", UiTheme.CaptionSize, UiTheme.TextLow,
                                    TextAlignmentOptions.Left);
            _status.textWrappingMode = TextWrappingModes.NoWrap;
            var sr = _status.rectTransform;
            sr.anchorMin = new Vector2(0f, 1f); sr.anchorMax = new Vector2(1f, 1f);
            sr.pivot = new Vector2(0.5f, 1f);
            sr.offsetMin = new Vector2(pad, -40f); sr.offsetMax = new Vector2(-pad, -UiTheme.PadSmall);

            var host = new GameObject("Scroll", typeof(RectTransform));
            host.transform.SetParent(body, false);
            var hrt = (RectTransform)host.transform;
            hrt.anchorMin = Vector2.zero; hrt.anchorMax = Vector2.one;
            hrt.offsetMin = new Vector2(pad, 30);
            hrt.offsetMax = new Vector2(-60, -94);
            _content = UiScroll.Create(hrt, out _);
        }

        public void Refresh()
        {
            Clear();
            _status.text = "";
            float y = 0f;

            string[] headings={"UI_SETTINGS_SOUND","UI_SETTINGS_GRAPHICS","UI_SETTINGS_LANGUAGE"};
            _heading.text=Loc.Get(headings[_page]);_headingIcon.Kind=_page==0?TitleGlyph.Symbol.Support:_page==1?TitleGlyph.Symbol.Settings:TitleGlyph.Symbol.Globe;_headingIcon.SetVerticesDirty();
            for(int i=0;i<_tabs.Count;i++)_tabs[i].SetActiveState(i==_page);
            Canvas.ForceUpdateCanvases();
            _arc.SelectNormalized((56+_page*94f)/Mathf.Max(1,_arc.rectTransform.rect.height));
            if(_page==1){
            Head(ref y, Loc.Get("UI_SETTINGS_DISPLAY"));
            Segmented(ref y, "Screen", new[] { "Windowed", "Fullscreen" },
                      PlayerPrefs.HasKey(KeyFullscreen) ? PlayerPrefs.GetInt(KeyFullscreen)
                                                        : (Screen.fullScreen ? 1 : 0),
                      i =>
                      {
                          PlayerPrefs.SetInt(KeyFullscreen, i);
                          Screen.fullScreenMode = i == 1 ? FullScreenMode.FullScreenWindow
                                                         : FullScreenMode.Windowed;
                      });
            Segmented(ref y, "V-Sync", new[] { "Off", "On" },
                      PlayerPrefs.HasKey(KeyVsync) ? PlayerPrefs.GetInt(KeyVsync)
                                                   : Mathf.Clamp(QualitySettings.vSyncCount, 0, 1),
                      i =>
                      {
                          PlayerPrefs.SetInt(KeyVsync, i);
                          QualitySettings.vSyncCount = i;
                      });
            // The cap is what an IDLE game is actually for: this window is open for hours in the
            // background, and 30 fps there is most of the machine given back.
            var caps = new[] { 30, 60, 120, -1 };
            Segmented(ref y, "Frame cap", new[] { "30", "60", "120", "Off" },
                      System.Array.IndexOf(caps, PlayerPrefs.HasKey(KeyFrameCap)
                          ? PlayerPrefs.GetInt(KeyFrameCap)
                          : Application.targetFrameRate) is int at && at >= 0 ? at : 1,
                      i =>
                      {
                          PlayerPrefs.SetInt(KeyFrameCap, caps[i]);
                          Application.targetFrameRate = caps[i];
                      });
            var note = UiTheme.Label(_content, "VsyncNote",
                Loc.Get("UI_OPTIONS_V_SYNC_OVERRIDES_THE_FRAME"),
                UiTheme.CaptionSize, UiTheme.TextLow, TextAlignmentOptions.TopLeft);
            Place(note.rectTransform, y, 22f);
            _built.Add(note.gameObject);
            y += 22f + UiTheme.Pad;

            Head(ref y, "Graphics");
            Segmented(ref y, "Post-processing", new[] { "Off", "On" }, PlayerPrefs.GetInt(KeyPostFx, 1),
                      i => { PlayerPrefs.SetInt(KeyPostFx, i); ApplyToArena();
                             Roi.Net.Telemetry.Note(i == 1 ? "fx.on" : "fx.off"); });
            // The two dressings of the same zone, switchable live — one layout, two looks, which is
            // the whole point of having built both.
            Segmented(ref y, "Zone style", new[] { "HD-2D", "Full 3D" }, StyleNow(),
                      i => { PlayerPrefs.SetInt(KeyStyle, i); ApplyToArena(); });

            Segmented(ref y, Loc.Get("UI_OPTIONS_REDUCED_MOTION"), new[] { Loc.Get("UI_OPTIONS_MOTION_FULL"), Loc.Get("UI_OPTIONS_MOTION_REDUCED") },
                      PlayerPrefs.GetInt(UiEntrance.ReducedMotionKey, 0),
                      i => { PlayerPrefs.SetInt(UiEntrance.ReducedMotionKey, i); PlayerPrefs.Save(); });

            }
            if(_page==0){
            Head(ref y, Loc.Get("UI_SETTINGS_VOLUME"));
            Volume(ref y,Loc.Get("UI_SETTINGS_MASTER"),"roi.opt.master",100,v=>AudioListener.volume=v/100f);
            Volume(ref y,Loc.Get("UI_TITLE_MUSIC"),KeyMusic,70,v=>Roi.Audio.SoundFx.SetVolumes(v/100f,Roi.Audio.SoundFx.SfxVolume));
            Volume(ref y,Loc.Get("UI_TITLE_SOUND"),KeySfx,80,v=>Roi.Audio.SoundFx.SetVolumes(Roi.Audio.SoundFx.MusicVolume,v/100f));
            }
            if(_page==2){Head(ref y,Loc.Get("UI_SETTINGS_TEXT"));Segmented(ref y,Loc.Get("UI_SETTINGS_LANGUAGE"),new[]{"English","\u0e44\u0e17\u0e22"},Loc.Language=="th"?1:0,i=>{Loc.Language=i==0?"en":"th";PlayerPrefs.SetString("roi.opt.language",Loc.Language);PlayerPrefs.Save();});}
            UiScroll.Fit(_content, y + UiTheme.Pad);
        }

        /// The step a stored percentage belongs to — nearest, so a value written by an older build
        /// (or by hand) lands on a real button instead of leaving every one of them unlit.
        static int StepOf(int percent)
        {
            int best = 0;
            for (int i = 1; i < VolumeSteps.Length; i++)
                if (Mathf.Abs(VolumeSteps[i] - percent) < Mathf.Abs(VolumeSteps[best] - percent)) best = i;
            return best;
        }

        /// A labelled row of choices, the selected one accented. Rebuilt on every change, which is
        /// cheap here and keeps the drawn state and the stored state from ever disagreeing.
        void Segmented(ref float y, string label, string[] options, int selected,
                       System.Action<int> onPick)
        {
            var ground=UiTheme.Box(_content,"SettingRow",UiTheme.WithAlpha(UiTheme.Surface,.65f),10);ground.sprite=UiTheme.CutFill(8);ground.raycastTarget=false;
            Place(ground.rectTransform,y,72);_built.Add(ground.gameObject);
            var cap = UiTheme.Label(_content, "L_" + label, label, UiTheme.BodySize, UiTheme.TextHigh,
                                    TextAlignmentOptions.Left);
            Place(cap.rectTransform, y, 72f);
            cap.rectTransform.offsetMin=new Vector2(20,cap.rectTransform.offsetMin.y);
            _built.Add(cap.gameObject);

            int selectedIndex=Mathf.Clamp(selected,0,options.Length-1);
            var value=UiTheme.Label(_content,"Value",options[selectedIndex],23,UiTheme.Accent,TextAlignmentOptions.Center);Place(value.rectTransform,y,72);value.rectTransform.anchorMin=new Vector2(.62f,1);value.rectTransform.anchorMax=new Vector2(.9f,1);_built.Add(value.gameObject);
            for(int direction=-1;direction<=1;direction+=2){int step=direction;var button=CollectionBrowser.Quiet(_content,"Choice",direction<0?"<":">",0,0,1,1,()=>{onPick((selectedIndex+step+options.Length)%options.Length);PlayerPrefs.Save();Refresh();});var rt=(RectTransform)button.transform;rt.anchorMin=rt.anchorMax=new Vector2(direction<0?.57f:.92f,1);rt.pivot=new Vector2(.5f,1);rt.anchoredPosition=new Vector2(0,-y-10);rt.sizeDelta=new Vector2(44,52);button.Caption.color=UiTheme.TextHigh;var buttonSurface=button.GetComponent<Image>();buttonSurface.sprite=UiTheme.CutFill(8);buttonSurface.type=Image.Type.Sliced;_built.Add(button.gameObject);}
            y += RowH;
        }

        static void Position(RectTransform rt,float x,float y,float w,float h){rt.anchorMin=rt.anchorMax=rt.pivot=new Vector2(0,1);rt.anchoredPosition=new Vector2(x,-y);rt.sizeDelta=new Vector2(w,h);}
        void Volume(ref float y,string label,string pref,int fallback,System.Action<int> apply){
            var row=UiTheme.Box(_content,"Volume",UiTheme.WithAlpha(UiTheme.Surface,.65f),10);row.sprite=UiTheme.CutFill(8);Place(row.rectTransform,y,72);_built.Add(row.gameObject);
            var text=UiTheme.Label(row.transform,"Label",label,23,UiTheme.TextHigh,TextAlignmentOptions.Left);text.rectTransform.anchorMin=Vector2.zero;text.rectTransform.anchorMax=new Vector2(.52f,1);text.rectTransform.offsetMin=new Vector2(20,0);text.rectTransform.offsetMax=Vector2.zero;
            var host=new GameObject("Slider",typeof(RectTransform));host.transform.SetParent(row.transform,false);var rt=(RectTransform)host.transform;rt.anchorMin=new Vector2(.6f,.5f);rt.anchorMax=new Vector2(.87f,.5f);rt.sizeDelta=new Vector2(0,28);
            var slider=host.AddComponent<Slider>();slider.minValue=0;slider.maxValue=100;slider.wholeNumbers=true;
            var track=UiTheme.Box(rt,"Track",UiTheme.Panel,4);track.rectTransform.anchorMin=new Vector2(0,.5f);track.rectTransform.anchorMax=new Vector2(1,.5f);track.rectTransform.sizeDelta=new Vector2(0,7);
            var fill=UiTheme.Box(track.transform,"Fill",UiTheme.Accent,4);fill.rectTransform.anchorMin=Vector2.zero;fill.rectTransform.anchorMax=Vector2.one;fill.rectTransform.offsetMin=fill.rectTransform.offsetMax=Vector2.zero;slider.fillRect=fill.rectTransform;
            var handle=UiTheme.Box(rt,"Handle",UiTheme.Ivory,7);handle.rectTransform.sizeDelta=new Vector2(14,0);slider.handleRect=handle.rectTransform;slider.targetGraphic=handle;
            var number=UiTheme.Label(row.transform,"Value","",22,UiTheme.Accent,TextAlignmentOptions.Center);var nr=number.rectTransform;nr.anchorMin=new Vector2(.89f,0);nr.anchorMax=new Vector2(1,1);nr.offsetMin=nr.offsetMax=Vector2.zero;
            slider.SetValueWithoutNotify(PlayerPrefs.GetInt(pref,fallback));number.text=slider.value.ToString("0");slider.onValueChanged.AddListener(v=>{int n=Mathf.RoundToInt(v);number.text=n.ToString();PlayerPrefs.SetInt(pref,n);apply(n);PlayerPrefs.Save();});y+=82;
        }

        void Head(ref float y, string text)
        {
            var strip=UiTheme.Box(_content,"Section",UiTheme.Header,6);Place(strip.rectTransform,y,34);_built.Add(strip.gameObject);
            var t = UiTheme.Caps(_content, "H_" + text, text, UiTheme.HeadSize,
                                 UiTheme.Accent, TextAlignmentOptions.Left);
            Place(t.rectTransform, y, 34f);
            t.rectTransform.offsetMin=new Vector2(14,t.rectTransform.offsetMin.y);
            _built.Add(t.gameObject);
            y += 44f;
        }

        void Place(RectTransform rt, float y, float h)
        {
            rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
            rt.pivot = new Vector2(0.5f, 1f);
            rt.sizeDelta = new Vector2(0f, h);
            rt.anchoredPosition = new Vector2(0f, -y);
        }

        void Clear()
        {
            foreach (var go in _built) {go.SetActive(false);Destroy(go);}
            _built.Clear();
        }
    }
}
