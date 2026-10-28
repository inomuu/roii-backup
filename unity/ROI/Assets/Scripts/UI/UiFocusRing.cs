using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.UI;

namespace Roi.UI
{
    /// Event-driven focus feedback; no animation or per-frame work while the form is idle.
    [RequireComponent(typeof(Selectable))]
    public sealed class UiFocusRing : MonoBehaviour, ISelectHandler, IDeselectHandler
    {
        Image _ring;

        void Awake()
        {
            _ring = UiTheme.Border((RectTransform)transform, UiTheme.Accent);
            _ring.name = "FocusRing";
            _ring.sprite = UiTheme.Frame(8, 2f);
            _ring.rectTransform.offsetMin = new Vector2(-3f, -3f);
            _ring.rectTransform.offsetMax = new Vector2(3f, 3f);
            _ring.raycastTarget = false;
            _ring.enabled = false;
        }

        public void OnSelect(BaseEventData eventData) { if (_ring != null) _ring.enabled = true; }
        public void OnDeselect(BaseEventData eventData) { if (_ring != null) _ring.enabled = false; }
        void OnDisable() { if (_ring != null) _ring.enabled = false; }
    }
}
