// A window SLOT — the frame every screen lives in. It is the half of the port that is layout
// rather than content: the chrome-derived rect, the title, the close, and the visibility the
// action bar mirrors onto its buttons.
//
// Deliberately empty inside. Porting a screen means filling one of these, not inventing where it
// sits — the same split godot/views/ui_layout.gd exists to enforce.
//
// FULL-BLEED since 2026-09-07 (user: "ทำเป็น full screen ให้หมดเลย"). A screen fills everything
// between the roster strip and the action bar — no margins, no corner radius, no border. Those
// were the right shape for a floating desktop panel and the wrong one for a phone: a rounded
// corner against the display edge reads as a mistake, and margins on a 6-inch screen are width
// spent on nothing. The chrome is now two hairlines, the roster's and the bar's.
//
// The LOOK is otherwise UiTheme's: a title set in caps with tracking behind a short accent rule.
// The accent appears once per window, on the title, and nowhere else.
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class UiWindow : MonoBehaviour
    {
        public string Title { get; private set; }
        public bool IsOpen => _root != null && _root.activeSelf;
        /// The area a screen fills. Empty here on purpose: this class owns WHERE a window sits,
        /// never what is in it.
        public RectTransform Body { get; private set; }
        /// Raised when the window opens, which is when a screen fetches what it needs — the item
        /// lists are pulled on demand, not streamed.
        public event System.Action Opened;

        readonly System.Collections.Generic.Dictionary<ScrollRect,Vector2> _scrollPositions=new();
        GameObject _root;
        RectTransform _rect;
        TextMeshProUGUI _titleText, _body;

        public static UiWindow Create(Transform parent, string title, System.Action onClose)
        {
            var go = new GameObject("Window_" + title, typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var w = go.AddComponent<UiWindow>();
            w.Build(title, onClose);
            return w;
        }

        void Build(string title, System.Action onClose)
        {
            Title = title;
            // This object is only a holder, and a fresh RectTransform defaults to a 100x100 box
            // pinned to the canvas CENTRE — which silently offset every child by half the screen.
            // Stretch it to the canvas so its children can use canvas coordinates directly.
            var self = (RectTransform)transform;
            self.anchorMin = Vector2.zero; self.anchorMax = Vector2.one;
            self.offsetMin = Vector2.zero; self.offsetMax = Vector2.zero;
            self.pivot = new Vector2(0.5f, 0.5f);

            // OPAQUE, where the Godot panel is 0.98. The project renders in linear space and the
            // canvas composites there, so a 0.98 dark panel over a lit scene came back with about
            // a tenth of the fight still showing through — measured, not guessed. 1.0 is what
            // reproduces the reference's LOOK; copying its number would not have.
            var bg = UiTheme.Flat(transform, "Panel", UiTheme.Panel);
            _root = bg.gameObject;
            _rect = bg.rectTransform;
            _rect.anchorMin = _rect.anchorMax = Vector2.zero;
            _rect.pivot = Vector2.zero;
            RpgBackdrop.Create(_rect);

            // --- title bar: no fill of its own. A rule under it separates the head from the body
            // with one hairline instead of a second block of colour.
            var bar = NewRect(_rect, "TitleBar");
            bar.anchorMin = new Vector2(0f, 1f); bar.anchorMax = new Vector2(1f, 1f);
            bar.pivot = new Vector2(0.5f, 1f);
            bar.sizeDelta = new Vector2(0f, UiTheme.TitleBarH);
            var header = UiTheme.Flat(bar, "HeaderSurface", UiTheme.Header);
            header.raycastTarget = false;
            header.rectTransform.anchorMin = Vector2.zero;
            header.rectTransform.anchorMax = Vector2.one;
            header.rectTransform.offsetMin = header.rectTransform.offsetMax = Vector2.zero;

            var accent = UiTheme.Flat(bar, "Accent", UiTheme.Accent);
            accent.raycastTarget = false;
            var ar = accent.rectTransform;
            ar.anchorMin = ar.anchorMax = new Vector2(0f, 0.5f);
            ar.pivot = new Vector2(0.5f, 0.5f);
            ar.localRotation = Quaternion.Euler(0, 0, 45);
            ar.sizeDelta = new Vector2(6f, 6f);
            ar.anchoredPosition = new Vector2(28f, 0f);

            _titleText = UiTheme.Label(bar, "Title", title, UiTheme.TitleSize,
                                      UiTheme.TextHigh, TextAlignmentOptions.Left);
            _titleText.enableAutoSizing=true;_titleText.fontSizeMin=16;_titleText.fontSizeMax=25;
            _titleText.characterSpacing=.3f;
            UiTheme.Display(_titleText);_titleText.fontSize=25;
            var tr = _titleText.rectTransform;
            tr.anchorMin = new Vector2(0f, 0f); tr.anchorMax = new Vector2(1f, 1f);
            tr.offsetMin = new Vector2(48f, 0f);
            tr.offsetMax = new Vector2(-320f, 0f);

            var rule = UiTheme.Flat(bar, "Rule", UiTheme.Line);
            rule.raycastTarget = false;
            var rr = rule.rectTransform;
            rr.anchorMin = new Vector2(0f, 0f); rr.anchorMax = new Vector2(1f, 0f);
            rr.pivot = new Vector2(0.5f, 0f);
            rr.sizeDelta = new Vector2(-UiTheme.Pad * 2f, 1f);

            var bodyGo = new GameObject("Body", typeof(RectTransform));
            bodyGo.transform.SetParent(_rect, false);
            Body = (RectTransform)bodyGo.transform;
            Body.anchorMin = Vector2.zero; Body.anchorMax = Vector2.one;
            Body.offsetMin = Vector2.zero; Body.offsetMax = new Vector2(0f, -UiTheme.TitleBarH);

            _body = UiTheme.Label(Body, "Placeholder", Loc.Get("UI_UI_NOT_PORTED_YET"), UiTheme.BodySize,
                                  UiTheme.TextLow, TextAlignmentOptions.Center);
            var br = _body.rectTransform;
            br.anchorMin = Vector2.zero; br.anchorMax = Vector2.one;
            br.offsetMin = Vector2.zero; br.offsetMax = Vector2.zero;

            _root.SetActive(false);
        }

        public void SetIdentity(string id)
        {
            var bar=_rect.Find("TitleBar");bar.Find("Accent").gameObject.SetActive(false);
            var go=new GameObject("ScreenIcon",typeof(RectTransform));go.transform.SetParent(bar,false);
            var icon=go.AddComponent<HomeIcon>();icon.Kind=id;icon.color=_titleText.color;icon.raycastTarget=false;
            var r=icon.rectTransform;r.anchorMin=r.anchorMax=new Vector2(0,.5f);r.anchoredPosition=new Vector2(28,0);r.sizeDelta=new Vector2(24,24);
            _titleText.rectTransform.offsetMin=new Vector2(52,0);
            if(id=="Cards"||id=="Album"||id=="Character"||id=="Skills"||id=="Regalia")
                PartyMemberPicker.Create((RectTransform)bar,_titleText);
        }

        /// Re-laid out on every open, not once: the chrome rect is derived from the screen size.
        public void SetOpen(bool on)
        {
            if (on) Layout();
            // Resize is layout only: do not refetch/rebuild the active page or reset its selection.
            if (on == IsOpen) return;
            StopAllCoroutines();
            if (!on)
            {
                _scrollPositions.Clear();
                foreach(var scroll in Body.GetComponentsInChildren<ScrollRect>(true))
                    _scrollPositions[scroll]=scroll.normalizedPosition;
            }
            _root.SetActive(on);
            if (on) { Opened?.Invoke(); UiPageTransition.Play(Body); StartCoroutine(RestoreScroll()); }
        }

        System.Collections.IEnumerator RestoreScroll()
        {
            yield return null;
            if(!IsOpen)yield break;
            Canvas.ForceUpdateCanvases();
            foreach(var pair in _scrollPositions)if(pair.Key!=null){pair.Key.StopMovement();pair.Key.normalizedPosition=pair.Value;}
        }

        /// A screen has taken the body over; the "not ported yet" note goes away.
        public void ClearPlaceholder()
        {
            if (_body != null) _body.gameObject.SetActive(false);
        }

        /// Sized from the CANVAS, not from `Screen`. In the Editor the game view can render at a
        /// different resolution than the window reports, and a window laid out against Screen then
        /// lands off the canvas entirely — which is exactly what it did.
        bool _immersive;
        internal void SetImmersive()
        {
            _immersive=true;
            _rect.Find("TitleBar").gameObject.SetActive(false);
            Body.offsetMin=Body.offsetMax=Vector2.zero;
            var c=_root.AddComponent<Canvas>();c.overrideSorting=true;c.sortingOrder=55;
            _root.AddComponent<GraphicRaycaster>();Layout();
        }

        void Layout()
        {
            var canvas = GetComponentInParent<Canvas>();
            var canvasRect = canvas != null ? (RectTransform)canvas.transform : null;
            var size = canvasRect != null ? canvasRect.rect.size : new Vector2(Screen.width, Screen.height);
            var r = _immersive ? new Rect(0,0,size.x,size.y) : UiLayout.ContentRect(size);
            _rect.anchoredPosition = new Vector2(Mathf.Round(r.x), Mathf.Round(r.y));
            _rect.sizeDelta = new Vector2(Mathf.Round(r.width), Mathf.Round(r.height));
        }

        void OnRectTransformDimensionsChange() { if (IsOpen) Layout(); }

        static RectTransform NewRect(Transform parent, string name)
        {
            var go = new GameObject(name, typeof(RectTransform));
            go.transform.SetParent(parent, false);
            return (RectTransform)go.transform;
        }
    }
}
