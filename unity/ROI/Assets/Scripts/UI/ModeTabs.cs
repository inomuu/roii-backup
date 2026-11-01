using System;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Godot mode_tabs.gd: navigation owns visibility/input, never the session clock.
    // The Home layout is presentation only. The combat scene stays alive.
    public sealed partial class ModeTabs : MonoBehaviour
    {
        public bool IsCombat { get; private set; } = true;
        public event Action<bool> ModeChanged;
        public event Action<string> OpenRequested;
        GameObject _menu;
        ExemplarScreen _exemplars;
        Button _switchButton;
        bool _returnToCombat;

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
            BuildCombatMenu();
            var strip=UiTheme.Flat(transform,"QuickNavigation",Color.clear);strip.raycastTarget=false;
            var rt=strip.rectTransform;rt.anchorMin=rt.anchorMax=rt.pivot=Vector2.one;
            rt.sizeDelta=new Vector2(576,86);rt.anchoredPosition=new Vector2(-16,-8);
            string[] ids={"Daily","Party","Inventory","Map","Mail","Options","Menu"};
            for(int i=0;i<ids.Length;i++){
                string id=ids[i];var button=HudBar.MakeGhostButton(rt,"",72,72,()=>{if(id=="Menu")ToggleCombatMenu();else NavigateCombat(id);});button.name="Open_"+id;if(id=="Menu")_switchButton=button;
                var br=(RectTransform)button.transform;br.anchorMin=br.anchorMax=br.pivot=new Vector2(0,1);br.anchoredPosition=new Vector2(i*84,0);
                button.GetComponent<Image>().color=new Color(.035f,.065f,.075f,.6f);
                button.GetComponent<Image>().sprite=UiTheme.Fill(16);button.GetComponent<Image>().type=Image.Type.Sliced;
                var go=new GameObject("Icon",typeof(RectTransform));go.transform.SetParent(button.transform,false);
                var icon=go.AddComponent<HomeIcon>();icon.Kind=id=="Menu"?"Story":id;icon.color=UiTheme.TextHigh;icon.raycastTarget=false;icon.rectTransform.sizeDelta=new Vector2(42,42);
            }
        }

        void Update()
        {
            var strip=transform.Find("QuickNavigation");if(strip==null)return;
            bool open=false;foreach(var window in UnityEngine.Object.FindObjectsByType<UiWindow>(FindObjectsSortMode.None))if(window.IsOpen){open=true;break;}
            strip.gameObject.SetActive(!open && IsCombat && (_exemplars==null||!_exemplars.gameObject.activeSelf));
            if(open && _menu.activeSelf)_menu.SetActive(false);
            if(UnityEngine.InputSystem.Keyboard.current?.escapeKey.wasPressedThisFrame==true && _menu.activeSelf)_menu.SetActive(false);
        }
        static void Place(Button b, float x)
        {
            var rt = (RectTransform)b.transform;
            rt.anchorMin = rt.anchorMax = rt.pivot = new Vector2(0.5f, 0.5f);
            rt.anchoredPosition = new Vector2(x, 0);
        }

        public void Select(bool combat)
        {
            if(UiConfirm.BlocksNavigation)return;
            // Home is now an overlay on the battle scene; simulation and HUD stay in combat.
            IsCombat=true;
            if(_exemplars!=null)_exemplars.gameObject.SetActive(false);
            _menu.SetActive(!combat);
            Paint();
        }

        public void OpenExemplars()
        {
            if(UiConfirm.BlocksNavigation)return;
            _returnToCombat=IsCombat;

            _menu.SetActive(false);
            if(_exemplars==null)_exemplars=ExemplarScreen.Create(transform,ReturnHome);
            _exemplars.gameObject.SetActive(true);
            _exemplars.Refresh();
            Paint();
        }
        void ReturnHome()
        {
            if(_exemplars!=null)_exemplars.gameObject.SetActive(false);
            Select(true);
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
                _switchButton.interactable=enabled;
        }
        void Paint()
        {
            Update();
        }
    }
}
