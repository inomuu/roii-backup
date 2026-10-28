using System;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Godot mode_tabs.gd: navigation owns visibility/input, never the session clock.
    // The Home layout is presentation only. The combat scene stays alive.
    public sealed class ModeTabs : MonoBehaviour
    {
        public bool IsCombat { get; private set; } = true;
        public event Action<bool> ModeChanged;
        public event Action<string> OpenRequested;
        GameObject _menu;
        ExemplarScreen _exemplars;
        Button _switchButton;
        HomeIcon _switchIcon;
        TMPro.TextMeshProUGUI _switchLabel;

        public static ModeTabs Create(Transform parent)
        {
            var go = new GameObject("ModeTabs", typeof(RectTransform), typeof(Canvas),
                typeof(CanvasScaler), typeof(GraphicRaycaster));
            go.transform.SetParent(parent, false);
            var canvas = go.GetComponent<Canvas>();
            canvas.renderMode = RenderMode.ScreenSpaceOverlay;
            canvas.sortingOrder = 45; // Above combat chrome, below session/system notices (50).
            var scaler = go.GetComponent<CanvasScaler>();
            scaler.uiScaleMode = CanvasScaler.ScaleMode.ScaleWithScreenSize;
            scaler.referenceResolution = new Vector2(1920, 1080);
            scaler.matchWidthOrHeight = 1;
            var tabs = go.AddComponent<ModeTabs>();
            tabs.Build();
            return tabs;
        }

        void Build()
        {
            var blank = UiTheme.Box(transform, "MenuPage", new Color(0.055f, 0.06f, 0.075f, 1f));
            var rt = blank.rectTransform;
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = rt.offsetMax = Vector2.zero;
            blank.raycastTarget = false; // Perspective card raycasts must reach their own camera.
            _menu = blank.gameObject;
            HomeMenuLayout.Build(blank.transform, id => { if(id=="Exemplars") OpenExemplars(); else OpenRequested?.Invoke(id); });
            _menu.SetActive(false);

            var strip = UiTheme.Box(transform, "Tabs", UiTheme.Panel);
            var navCanvas=strip.gameObject.AddComponent<Canvas>();
            navCanvas.overrideSorting=true; navCanvas.sortingOrder=49;
            strip.gameObject.AddComponent<GraphicRaycaster>();
            rt = strip.rectTransform;
            rt.anchorMin = rt.anchorMax = new Vector2(1f, 1);
            rt.pivot = new Vector2(1f, 1);
            rt.sizeDelta = new Vector2(190, 58);
            rt.anchoredPosition = new Vector2(-16, -12);
            _switchButton = HudBar.MakeButton(rt, "", "", 184, 52, () => Select(!IsCombat));
            _switchLabel = _switchButton.GetComponentInChildren<TMPro.TextMeshProUGUI>();
            _switchButton.name="ModeSwitch";
            var font=UiTheme.Font;
            if(font!=null){_switchLabel.font=font;_switchLabel.fontSharedMaterial=font.material;}
            _switchLabel.fontSize=27;
            _switchLabel.alignment=TMPro.TextAlignmentOptions.CaplineLeft;
            _switchLabel.rectTransform.offsetMin=new Vector2(43,0);
            _switchLabel.rectTransform.offsetMax=new Vector2(-10,0);
            var iconGo=new GameObject("ModeIcon",typeof(RectTransform),typeof(HomeIcon));
            iconGo.transform.SetParent(_switchButton.transform,false);
            _switchIcon=iconGo.GetComponent<HomeIcon>();_switchIcon.raycastTarget=false;_switchIcon.color=new Color(.94f,.92f,.87f);
            var ir=_switchIcon.rectTransform;ir.anchorMin=ir.anchorMax=new Vector2(0,.5f);
            ir.pivot=new Vector2(0,.5f);ir.anchoredPosition=new Vector2(12,0);ir.sizeDelta=new Vector2(26,26);
            Place(_switchButton, 0);
            Paint();
        }

        static void Place(Button b, float x)
        {
            var rt = (RectTransform)b.transform;
            rt.anchorMin = rt.anchorMax = rt.pivot = new Vector2(0.5f, 0.5f);
            rt.anchoredPosition = new Vector2(x, 0);
        }

        public void Select(bool combat)
        {
            if (IsCombat == combat || UiConfirm.BlocksNavigation) return;
            IsCombat = combat;
            // Close camera-owning combat widgets before the Home portrait takes ownership.
            ModeChanged?.Invoke(combat);
            if(_exemplars!=null)_exemplars.gameObject.SetActive(false);
            _menu.SetActive(!combat);
            Paint();
        }

        public void OpenExemplars()
        {
            if(UiConfirm.BlocksNavigation)return;
            if(IsCombat)Select(false);
            _menu.SetActive(false);
            if(_exemplars==null)_exemplars=ExemplarScreen.Create(transform,ReturnHome);
            _exemplars.gameObject.SetActive(true);
            _exemplars.Refresh();
            Paint();
        }
        void ReturnHome()
        {
            if(_exemplars!=null)_exemplars.gameObject.SetActive(false);
            _menu.SetActive(true);
            Paint();
        }
        public bool BackFromExemplars()
        {
            if(UiConfirm.BlocksNavigation)return true;
            if(_exemplars==null||!_exemplars.gameObject.activeSelf)return false;
            _exemplars.Back();return true;
        }

        public void SetModeSwitchInputEnabled(bool enabled)
        {
            // A transient item browser owns outside clicks; leave this button visible but let
            // its first click reach the browser's dismiss scrim instead of switching modes.
            if(_switchButton!=null)
                _switchButton.transform.parent.GetComponent<GraphicRaycaster>().enabled=enabled;
        }
        void Paint()
        {
            // Home owns the battle action; do not overlay a duplicate on its wallet.
            _switchButton.transform.parent.gameObject.SetActive(IsCombat || !_menu.activeSelf);
            _switchIcon.Kind=IsCombat?"Story":"Combat";_switchIcon.SetVerticesDirty();
            _switchLabel.text = Loc.Get(IsCombat ? "UI_COMMON_MENU" : "UI_STAT_SEC_COMBAT");
        }
    }
}
