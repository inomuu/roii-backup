// A LIST TO CHOOSE FROM — one option per row, a label and a line of consequence under it.
//
// It exists because "tap to select, then tap to act" (2026-09-07) runs out of room the moment an
// action needs a SECOND subject: feeding a card asks which copy to spend, and the answer is not
// always the obvious one — a foil copy carries its foil across, a starred one gives more than one
// star. A confirmation can only ask yes or no, and a third column of cells for a choice made once
// in a session would cost the screen its layout.
//
// Every row states what it WOULD DO, not just what it is. That is the point of the widget: the
// difference between two candidates is the outcome, and a list of names makes the player guess it.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class UiPicker : MonoBehaviour
    {
        public struct Option
        {
            public string label, detail;
            public System.Action onPick;
        }

        const float W = 460f, RowH = 52f, HeadH = 40f, FootH = 52f, MaxRows = 6f;

        static UiPicker _open;
        static int _closedFrame=-1;
        static bool _cancelHeld;
        bool _picked;
        GameObject _previousSelection;
        public static bool BlocksNavigation { get { if(UnityEngine.InputSystem.Keyboard.current?.escapeKey.isPressed!=true)_cancelHeld=false;return _open!=null || _closedFrame==Time.frameCount || _cancelHeld; } }
        void Update(){if(UnityEngine.InputSystem.Keyboard.current?.escapeKey.wasPressedThisFrame==true)Close();}

        /// Shows the list. A second call replaces the first: two stacked pickers would leave the
        /// one underneath able to fire, and it is the same press that opened the new one.
        public static void Open(string title, IList<Option> options)
        {
            Close();
            if (options == null || options.Count == 0) return;
            var bar = Object.FindFirstObjectByType<HudBar>();
            if (bar == null) return;
            var go = new GameObject("Picker");
            go.transform.SetParent(bar.PopupLayer, false);
            _open = go.AddComponent<UiPicker>();
            _open.Build(title, options);
        }

        public static void Close()
        {
            if (_open != null) {
                _closedFrame=Time.frameCount;_cancelHeld=UnityEngine.InputSystem.Keyboard.current?.escapeKey.isPressed==true;
                var selection=_open._previousSelection;
                _open.gameObject.SetActive(false);Destroy(_open.gameObject);
                if(selection!=null&&selection.activeInHierarchy)UnityEngine.EventSystems.EventSystem.current?.SetSelectedGameObject(selection);
            }
            _open = null;
        }

        void Build(string title, IList<Option> options)
        {
            _previousSelection=UnityEngine.EventSystems.EventSystem.current?.currentSelectedGameObject;
            var root = gameObject.AddComponent<RectTransform>();
            root.anchorMin = Vector2.zero; root.anchorMax = Vector2.one;
            root.offsetMin = Vector2.zero; root.offsetMax = Vector2.zero;

            var scrim = UiTheme.Flat(root, "Scrim", UiTheme.Scrim);
            var sr = scrim.rectTransform;
            sr.anchorMin = Vector2.zero; sr.anchorMax = Vector2.one;
            sr.offsetMin = Vector2.zero; sr.offsetMax = Vector2.zero;
            // Tapping outside CANCELS. A picker is a question, and a question the player did not
            // mean to ask has to be dismissible without answering it.
            var dismiss = scrim.gameObject.AddComponent<Button>();
            dismiss.targetGraphic = scrim;
            dismiss.transition = Selectable.Transition.None;
            dismiss.onClick.AddListener(Close);

            float rows = Mathf.Min(options.Count, MaxRows);
            var panel = UiTheme.Box(root, "Panel", UiTheme.Panel, 10);
            DialogSurface.Apply(panel);
            var pr = panel.rectTransform;
            pr.anchorMin = pr.anchorMax = new Vector2(0.5f, 0.5f);
            pr.pivot = new Vector2(0.5f, 0.5f);
            // The PadSmall the list's top inset takes has to be in the height too, or the
            // last row is cut off by exactly that much — which reads as a row that failed
            // to draw rather than as a panel one gap too short.
            var canvas=(RectTransform)GetComponentInParent<Canvas>().transform;
            pr.sizeDelta = new Vector2(Mathf.Min(W,canvas.rect.width-32), Mathf.Min(HeadH + UiTheme.PadSmall + rows * RowH + FootH,canvas.rect.height-32));

            var head = UiTheme.Caps(pr, "Head", title, UiTheme.HeadSize, UiTheme.TextHigh,
                                    TextAlignmentOptions.Left);
            var hr = head.rectTransform;
            hr.anchorMin = new Vector2(0f, 1f); hr.anchorMax = new Vector2(1f, 1f);
            hr.pivot = new Vector2(0.5f, 1f);
            hr.offsetMin = new Vector2(UiTheme.PadLarge, 0f);
            hr.offsetMax = new Vector2(-UiTheme.PadLarge, 0f);
            hr.sizeDelta = new Vector2(hr.sizeDelta.x, HeadH);
            hr.anchoredPosition = new Vector2(0f, -UiTheme.PadSmall);

            // The list scrolls past six. A picker that grows until it is taller than the display
            // is the failure the modal-sizing rule was written for.
            var listGo = new GameObject("List", typeof(RectTransform));
            listGo.transform.SetParent(pr, false);
            var list = (RectTransform)listGo.transform;
            list.anchorMin = new Vector2(0f, 0f); list.anchorMax = new Vector2(1f, 1f);
            list.offsetMin = new Vector2(UiTheme.Pad, FootH);
            list.offsetMax = new Vector2(-UiTheme.Pad, -(HeadH + UiTheme.PadSmall));
            var content = UiScroll.Create(list, out _);

            float y = 0f;
            foreach (var o in options)
            {
                var pick = o.onPick;
                var cell = UiTheme.Flat(content, "Option", Color.white);
                var btn = cell.gameObject.AddComponent<Button>();
                btn.targetGraphic = cell;
                btn.navigation = new Navigation { mode = Navigation.Mode.None };
                var colors = btn.colors;
                colors.normalColor = Color.clear;
                colors.highlightedColor = new Color(.45f, .49f, .44f, .16f);
                colors.pressedColor = new Color(.60f, .52f, .32f, .24f);
                colors.selectedColor = colors.highlightedColor;
                colors.disabledColor = Color.clear;
                colors.fadeDuration = 0.08f;
                btn.colors = colors;
                cell.CrossFadeColor(colors.normalColor, 0f, true, true);
                btn.onClick.AddListener(() => { if(_picked||_open!=this)return;_picked=true;Close();pick?.Invoke(); });

                var cr = cell.rectTransform;
                cr.anchorMin = new Vector2(0f, 1f); cr.anchorMax = new Vector2(1f, 1f);
                cr.pivot = new Vector2(0.5f, 1f);
                cr.sizeDelta = new Vector2(0f, RowH - 4f);
                cr.anchoredPosition = new Vector2(0f, -y);

                var label = UiTheme.Label(cr, "Label", o.label, UiTheme.BodySize, UiTheme.TextHigh,
                                          TextAlignmentOptions.TopLeft);
                var lr = label.rectTransform;
                lr.anchorMin = Vector2.zero; lr.anchorMax = Vector2.one;
                lr.offsetMin = new Vector2(UiTheme.Pad, 0f);
                lr.offsetMax = new Vector2(-UiTheme.Pad, -6f);

                var detail = UiTheme.Label(cr, "Detail", o.detail ?? "", UiTheme.CaptionSize,
                                           UiTheme.TextMid, TextAlignmentOptions.BottomLeft);
                var dr = detail.rectTransform;
                dr.anchorMin = Vector2.zero; dr.anchorMax = Vector2.one;
                dr.offsetMin = new Vector2(UiTheme.Pad, 6f);
                dr.offsetMax = new Vector2(-UiTheme.Pad, 0f);

                y += RowH;
            }
            UiScroll.Fit(content, y);

            var cancel = HudBar.MakeButton(pr, Loc.Get("UI_COMMON_CANCEL"), "", 140f, UiLayout.Touch, Close);
            var qr = (RectTransform)cancel.transform;
            qr.anchorMin = qr.anchorMax = new Vector2(1f, 0f);
            qr.pivot = new Vector2(1f, 0f);
            qr.anchoredPosition = new Vector2(-UiTheme.PadLarge, UiTheme.Pad);
        }
    }
}
