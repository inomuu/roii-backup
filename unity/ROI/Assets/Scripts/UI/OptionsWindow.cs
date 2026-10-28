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

        const float RowH = 60f, BtnW = 116f, BtnH = 44f;

        RectTransform _content;
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
            float pad = UiTheme.PadLarge;
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
            hrt.offsetMin = new Vector2(pad, UiTheme.Pad);
            hrt.offsetMax = new Vector2(-pad, -(40f + UiTheme.Gap));
            _content = UiScroll.Create(hrt, out _);
        }

        public void Refresh()
        {
            Clear();
            _status.text = "";
            float y = 0f;

            Head(ref y, "Display");
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

            Head(ref y, "Audio");
            Segmented(ref y, "Music", VolumeLabels, StepOf(PlayerPrefs.GetInt(KeyMusic, 70)),
                      i =>
                      {
                          PlayerPrefs.SetInt(KeyMusic, VolumeSteps[i]);
                          Roi.Audio.SoundFx.SetVolumes(VolumeSteps[i] / 100f,
                                                       Roi.Audio.SoundFx.SfxVolume);
                      });
            Segmented(ref y, "Sound", VolumeLabels, StepOf(PlayerPrefs.GetInt(KeySfx, 80)),
                      i =>
                      {
                          PlayerPrefs.SetInt(KeySfx, VolumeSteps[i]);
                          Roi.Audio.SoundFx.SetVolumes(Roi.Audio.SoundFx.MusicVolume,
                                                       VolumeSteps[i] / 100f);
                      });

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
            var ground=UiTheme.Flat(_content,"SettingRow",new Color(UiTheme.Surface.r,UiTheme.Surface.g,UiTheme.Surface.b,.85f));ground.raycastTarget=false;
            Place(ground.rectTransform,y,RowH-4);_built.Add(ground.gameObject);
            var cap = UiTheme.Label(_content, "L_" + label, label, UiTheme.BodySize, UiTheme.TextHigh,
                                    TextAlignmentOptions.Left);
            Place(cap.rectTransform, y + 12f, 28f);
            cap.rectTransform.offsetMin=new Vector2(14,cap.rectTransform.offsetMin.y);
            _built.Add(cap.gameObject);

            float x = 220f;
            for (int i = 0; i < options.Length; i++)
            {
                int idx = i;
                var b = HudBar.MakeGhostButton(_content, options[i], BtnW, BtnH,
                                               () => { onPick(idx); Refresh(); }, UiTheme.BodySize);
                var t = b.GetComponentInChildren<TextMeshProUGUI>();
                b.GetComponent<Image>().color = i == selected ? UiTheme.SurfaceActive : UiTheme.Surface;
                b.colors = UiTheme.ButtonColors;
                b.targetGraphic.CrossFadeColor(Color.white,0f,true,true);
                t.color = i == selected ? UiTheme.Accent : UiTheme.TextLow;
                if (i == selected) UiTheme.Border((RectTransform)b.transform, UiTheme.Accent);
                var rt = (RectTransform)b.transform;
                rt.anchorMin = rt.anchorMax = new Vector2(0f, 1f);
                rt.pivot = new Vector2(0f, 1f);
                rt.anchoredPosition = new Vector2(x, -(y + 4f));
                _built.Add(b.gameObject);
                x += BtnW + UiTheme.Gap;
            }
            y += RowH;
        }

        void Head(ref float y, string text)
        {
            var t = UiTheme.Caps(_content, "H_" + text, text, UiTheme.HeadSize,
                                 UiTheme.Accent, TextAlignmentOptions.Left);
            Place(t.rectTransform, y, 26f);
            _built.Add(t.gameObject);
            y += 26f;
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
            foreach (var go in _built) Destroy(go);
            _built.Clear();
        }
    }
}
