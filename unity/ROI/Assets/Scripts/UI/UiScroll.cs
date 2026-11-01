// A SCROLLING COLUMN, on the theme. Every screen left to port is a list of something — party
// members, monsters, cards, provisions — and a list longer than its window is the first thing each
// of them needs. Building it once here is the difference between porting a screen and re-deciding
// how a scrollbar looks four more times.
//
// Pooled rows, not a rebuild: `UiRow` instances are handed back and reused, for the reason the
// item grid states outright — freeing and recreating controls under a live pointer froze buttons
// mid-press in the Godot client, and the fix there was the same one.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.UI;

namespace Roi.UI
{
    public static class UiScroll
    {
        /// A vertical scroll area filling `parent`. Returns the CONTENT rect: parent rows to it,
        /// position them from its top, and call `Fit` when the row count changes.
        ///
        /// The viewport gets a RectMask2D rather than a Mask: a Mask needs its own graphic and a
        /// stencil pass per masked element, and this clips a plain rectangle.
        public static RectTransform Create(RectTransform parent, out ScrollRect scroll, bool showScrollbar = true)
        {
            var viewGo = new GameObject("Viewport", typeof(RectTransform));
            viewGo.transform.SetParent(parent, false);
            var view = (RectTransform)viewGo.transform;
            view.anchorMin = Vector2.zero; view.anchorMax = Vector2.one;
            view.offsetMin = Vector2.zero; view.offsetMax = Vector2.zero;
            viewGo.AddComponent<RectMask2D>();
            // Invisible, but it must still take the pointer or the wheel has nothing to scroll.
            var catcher = viewGo.AddComponent<Image>();
            catcher.color = new Color(0f, 0f, 0f, 0f);
            catcher.raycastTarget = true;

            var contentGo = new GameObject("Content", typeof(RectTransform));
            contentGo.transform.SetParent(view, false);
            var content = (RectTransform)contentGo.transform;
            content.anchorMin = new Vector2(0f, 1f); content.anchorMax = new Vector2(1f, 1f);
            content.pivot = new Vector2(0.5f, 1f);
            content.offsetMin = new Vector2(0f, 0f); content.offsetMax = new Vector2(0f, 0f);

            scroll = parent.gameObject.AddComponent<ScrollRect>();
            scroll.viewport = view;
            scroll.content = content;
            scroll.horizontal = false;
            scroll.movementType = ScrollRect.MovementType.Clamped;
            scroll.scrollSensitivity = 28f;
            // No inertia. A flick that keeps gliding is a touch idiom; on a mouse it reads as the
            // list refusing to stop where it was put.
            scroll.inertia = false;
            if (showScrollbar)
            {
                view.offsetMax = new Vector2(-14f,0f);
                var track = UiTheme.Box(parent,"ScrollTrack",UiTheme.Surface,4);
                var tr = track.rectTransform;
                tr.anchorMin = new Vector2(1f,0f); tr.anchorMax = Vector2.one;
                tr.offsetMin = new Vector2(-10f,2f); tr.offsetMax = new Vector2(0f,-2f);
                var handle = UiTheme.Box(tr,"Handle",UiTheme.TextLow,4);
                handle.rectTransform.anchorMin = Vector2.zero; handle.rectTransform.anchorMax = Vector2.one;
                handle.rectTransform.offsetMin = new Vector2(2f,0f); handle.rectTransform.offsetMax = new Vector2(-2f,0f);
                var bar = track.gameObject.AddComponent<Scrollbar>();
                bar.direction = Scrollbar.Direction.BottomToTop;
                bar.handleRect = handle.rectTransform; bar.targetGraphic = handle;
                bar.navigation = new Navigation { mode = Navigation.Mode.None };
                scroll.verticalScrollbar = bar;
                scroll.verticalScrollbarVisibility = ScrollRect.ScrollbarVisibility.AutoHide;
            }
            return content;
        }

        /// Set the content's height from what is in it. A ScrollRect with a content shorter than
        /// its viewport simply does not scroll, which is the wanted behaviour for a short list.
        public static void Fit(RectTransform content, float height)
            => content.sizeDelta = new Vector2(content.sizeDelta.x, Mathf.Max(0f, height));
    }

    /// ONE ROW of a list: a name, a quiet value on the right, and a selected state. Selection is
    /// a lighter surface and a left accent tick — the same vocabulary a window title uses, so the
    /// player learns one signal for "this is the one" rather than one per screen.
    public sealed class UiRow : MonoBehaviour, IPointerClickHandler, IPointerEnterHandler, IPointerExitHandler
    {
        public const float Height = 38f;

        Image _bg, _tick;
        TextMeshProUGUI _label, _value;
        bool _selected, _hover;
        System.Action _onClick;

        public static UiRow Create(RectTransform parent)
        {
            var img = UiTheme.Box(parent, "Row", Color.clear, 4);
            var row = img.gameObject.AddComponent<UiRow>();
            row._bg = img;
            var rt = img.rectTransform;
            rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
            rt.pivot = new Vector2(0.5f, 1f);
            rt.sizeDelta = new Vector2(0f, Height);

            row._tick = UiTheme.Flat(rt, "Tick", UiTheme.Accent);
            row._tick.raycastTarget = false;
            var tr = row._tick.rectTransform;
            tr.anchorMin = tr.anchorMax = new Vector2(0f, 0.5f);
            tr.pivot = new Vector2(0f, 0.5f);
            tr.sizeDelta = new Vector2(2f, 14f);
            tr.anchoredPosition = new Vector2(4f, 0f);
            row._tick.enabled = false;

            row._label = UiTheme.Label(rt, "Label", "", UiTheme.BodySize,
                                       UiTheme.TextHigh, TextAlignmentOptions.Left);
            Fill(row._label.rectTransform, UiTheme.PadSmall, 120f);

            row._value = UiTheme.Label(rt, "Value", "", UiTheme.CaptionSize,
                                       UiTheme.TextLow, TextAlignmentOptions.Right);
            Fill(row._value.rectTransform, UiTheme.PadSmall, UiTheme.PadSmall);

            row.Paint();
            return row;
        }

        static void Fill(RectTransform rt, float left, float right)
        {
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = new Vector2(left, 0f); rt.offsetMax = new Vector2(-right, 0f);
        }

        public void Set(string label, string value, System.Action onClick)
        {
            _label.text = label;
            _value.text = value;
            _onClick = onClick;
        }

        public void SetSelected(bool on) { _selected = on; Paint(); }

        void Paint()
        {
            _bg.color = _selected ? UiTheme.SurfaceActive
                      : _hover ? UiTheme.Surface
                      : Color.clear;
            _tick.enabled = _selected;
            _label.color = _selected || _hover ? UiTheme.TextHigh : UiTheme.TextMid;
        }

        public void OnPointerEnter(PointerEventData e) { _hover = true; Paint(); }
        public void OnPointerExit(PointerEventData e) { _hover = false; Paint(); }
        public void OnPointerClick(PointerEventData e)
        {
            // Scrolling a list or opening a context menu must not select its row.
            if (e.button != PointerEventData.InputButton.Left || e.dragging) return;
            _onClick?.Invoke();
        }
    }
}
