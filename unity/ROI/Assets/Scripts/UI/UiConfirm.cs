// CONFIRM — the one modal this UI has, for the actions that cannot be taken back.
//
// It exists because the bag grew a Discard button that destroyed a stack outright with nothing in
// between. Everywhere else the sim is the safety net: a bad input is a silent no-op, a bad purchase
// refuses, a listing can be cancelled. Discard is the exception the sim ALLOWS — it is the one
// liquidation path a bound item accepts, deliberately — so the guard has to be here.
//
// RE-THEMED PER OPEN, as `confirm_popup.gd` is: the same panel serves opposite moods (destroying an
// item is a warning, using an expander is a benefit), and a modal that always looks like an alarm
// teaches people to dismiss it without reading.
//
// THE CALLER DECIDES WHAT CONFIRMING MEANS. This renders and calls back; it never touches the sim.
// That is why it is a static opener rather than a component someone owns — any screen can ask, and
// two screens cannot fight over one instance because the panel is destroyed on close.
using TMPro;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.InputSystem;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed partial class UiConfirm : MonoBehaviour
    {
        const float PanelW = 500f;
        static int _closedFrame = -1;
        static readonly System.Collections.Generic.HashSet<UiConfirm> Active = new();
        void OnEnable() => Active.Add(this);
        void OnDisable() => Active.Remove(this);
        // Keep cancellation owned by this modal even if its Update runs before HudBar.
        public static bool BlocksNavigation => _closedFrame == Time.frameCount
            || Active.Count > 0; // Queried every frame; never scan/sort the whole scene here.

        bool _closing;
        GameObject _previousSelection;
        System.Action _onConfirm;
        System.Action _onDismissed;

        /// Ask, and call back only on Confirm. `danger` colours the panel as a warning — for an
        /// action that destroys something.
        public static void Ask(string title, string body, string confirmLabel,
                               System.Action onConfirm, bool danger = true, bool informationOnly = false)
        {
            var bar = Object.FindFirstObjectByType<HudBar>();
            if (bar == null) { onConfirm?.Invoke(); return; }   // no UI to ask with: do as asked
            var go = new GameObject("Confirm");
            go.transform.SetParent(bar.PopupLayer, false);
            var c = go.AddComponent<UiConfirm>();
            c._previousSelection=EventSystem.current?.currentSelectedGameObject;
            c._onConfirm = onConfirm;
            c.Build(title, body, confirmLabel, danger, informationOnly);
        }

        void Build(string title, string body, string confirmLabel, bool danger, bool informationOnly)
        {
            var root = gameObject.AddComponent<RectTransform>();
            root.anchorMin = Vector2.zero; root.anchorMax = Vector2.one;
            root.offsetMin = Vector2.zero; root.offsetMax = Vector2.zero;

            // The scrim BLOCKS: a modal that lets the screen behind it be clicked is a modal in
            // appearance only, and the press that opened it is often one tap from the press that
            // would undo the asking.
            var scrim = UiTheme.Flat(root, "Scrim", UiTheme.Scrim);
            var sr = scrim.rectTransform;
            sr.anchorMin = Vector2.zero; sr.anchorMax = Vector2.one;
            sr.offsetMin = Vector2.zero; sr.offsetMax = Vector2.zero;
            var block = scrim.gameObject.AddComponent<Button>();
            block.targetGraphic = scrim;
            block.transition = Selectable.Transition.None;
            block.navigation = new Navigation { mode = Navigation.Mode.None };
            // Tapping outside CANCELS, which is the safe half of the choice.
            block.onClick.AddListener(Close);

            var accent = danger ? new Color(0.95f, 0.45f, 0.40f) : UiTheme.Accent;
            var panel = UiTheme.Box(root, "Panel", UiTheme.Panel, 10);
            DialogSurface.Apply(panel);

            var pr = panel.rectTransform;
            pr.anchorMin = pr.anchorMax = new Vector2(0.5f, 0.5f);
            pr.pivot = new Vector2(0.5f, 0.5f);

            float y = UiTheme.PadLarge;
            var t = UiTheme.Label(pr, "Title", title, 28, accent,
                                 TextAlignmentOptions.Left);
            UiTheme.Display(t);Row(t, ref y, 48f);
            // DialogSurface hangs a divider just under the title; leave it room so it never
            // strikes through the first line of the body.
            y += UiTheme.PadSmall;
            var b = UiTheme.Label(pr, "Body", body, UiTheme.BodySize, UiTheme.TextMid,
                                  TextAlignmentOptions.Top);
            b.textWrappingMode = TextWrappingModes.Normal;
            b.overflowMode = TextOverflowModes.Overflow;
            // A FIXED wrap width. An auto-wrapping label whose width is not pinned measures its
            // minimum as "wrap every character", which is how a 420-wide panel ends up as tall as
            // the screen — the same trap the Godot popup writes down.
            Row(b, ref y, Mathf.Max(44f, b.GetPreferredValues(body, PanelW - UiTheme.PadLarge * 2f, Mathf.Infinity).y + 4f));
            y += UiTheme.Pad;

            float bw = informationOnly ? PanelW - UiTheme.PadLarge * 2f : (PanelW - UiTheme.PadLarge * 2f - UiTheme.Gap) / 2f;
            Button cancel=null;
            if(!informationOnly) {
            cancel = HudBar.MakeGhostButton(pr, Loc.Get("UI_COMMON_CANCEL"), bw, UiLayout.Touch, Close);
            Place(cancel, UiTheme.PadLarge, y);
            }
            var ok = HudBar.MakeButton(pr, confirmLabel, "", bw, UiLayout.Touch,
                                       ConfirmOnce,primary:!danger);
            Place(ok, informationOnly ? UiTheme.PadLarge : UiTheme.PadLarge + bw + UiTheme.Gap, y);
            if (danger) ok.GetComponentInChildren<TextMeshProUGUI>().color = accent;
            y += UiLayout.Touch + UiTheme.PadLarge;

            pr.sizeDelta = new Vector2(PanelW, y);
            var available=root.rect.size;
            pr.localScale=Vector3.one*Mathf.Min(1,Mathf.Min((available.x-32)/PanelW,(available.y-32)/y));
            if(cancel!=null){cancel.navigation=new Navigation{mode=Navigation.Mode.Explicit,selectOnRight=ok};ok.navigation=new Navigation{mode=Navigation.Mode.Explicit,selectOnLeft=cancel};}
            EventSystem.current?.SetSelectedGameObject(cancel!=null?cancel.gameObject:ok.gameObject);
            DialogSurface.CloseIcon(pr,Close);
            t.margin=new Vector4(0,0,36,0);
            UiPageTransition.Play(pr);
        }

        void Row(TextMeshProUGUI t, ref float y, float h)
        {
            var rt = t.rectTransform;
            rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
            rt.pivot = new Vector2(0.5f, 1f);
            rt.offsetMin = new Vector2(UiTheme.PadLarge, 0f);
            rt.offsetMax = new Vector2(-UiTheme.PadLarge, 0f);
            rt.sizeDelta = new Vector2(rt.sizeDelta.x, h);
            rt.anchoredPosition = new Vector2(0f, -y);
            y += h;
        }

        static void Place(Button b, float x, float y)
        {
            var rt = (RectTransform)b.transform;
            rt.anchorMin = rt.anchorMax = new Vector2(0f, 1f);
            rt.pivot = new Vector2(0f, 1f);
            rt.anchoredPosition = new Vector2(x, -y);
        }

        void Update()
        {
            // Escape cancels. It is the key a player already presses to get out of things here,
            // and the safe half of the choice is the one it should reach.
            var k = Keyboard.current;
            if (k != null && k.escapeKey.wasPressedThisFrame) Close();
        }

        void ConfirmOnce()
        {
            if(_closing)return;
            var action=_onConfirm;_onConfirm=null;_onDismissed=null;
            Close();action?.Invoke();
        }
        internal void Close()
        {
            if(_closing)return;_closing=true;_closedFrame=Time.frameCount;
            var dismissed=_onDismissed;_onDismissed=null;_onConfirm=null;
            gameObject.SetActive(false);Destroy(gameObject);
            if(_previousSelection!=null&&_previousSelection.activeInHierarchy)EventSystem.current?.SetSelectedGameObject(_previousSelection);
            dismissed?.Invoke();
        }
    }
}
