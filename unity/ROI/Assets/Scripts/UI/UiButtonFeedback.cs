using DG.Tweening;      // Append/Kill/SetEase are EXTENSION methods — qualifying the type is not enough
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.UI;

namespace Roi.UI
{
    // Event driven: a quiet border on hover, gold on explicit focus, a small press-scale on tap.
    [DisallowMultipleComponent, RequireComponent(typeof(Selectable))]
    public sealed class UiButtonFeedback : MonoBehaviour, IPointerEnterHandler, IPointerExitHandler, ISelectHandler, IDeselectHandler, IPointerDownHandler, IPointerUpHandler
    {
        Selectable _control;
        Image _edge;
        CodexButtonImage _frame;
        bool _hover, _selected;
        Tween _press;
        public static void Add(Selectable control)
        { if(control.GetComponent<UiButtonFeedback>()==null) control.gameObject.AddComponent<UiButtonFeedback>(); }
        void Awake()
        {
            _control=GetComponent<Selectable>();
            _frame=GetComponent<CodexButtonImage>();
            if(_frame!=null)return;
            _edge=UiTheme.Border((RectTransform)transform,UiTheme.LineStrong);
            _edge.name="InteractionEdge";
            _edge.enabled=false;
        }
        void Paint()
        {
            if(_control==null)return;
            if(_frame!=null){_frame.Focused=(_hover||_selected)&&_control.IsInteractable();_frame.SetVerticesDirty();return;}
            if(_edge==null)return;
            _edge.enabled=(_hover||_selected)&&_control.IsInteractable();
            _edge.color=_selected?UiTheme.Accent:UiTheme.LineStrong;
        }
        public void RefreshState()=>Paint();
        public void OnPointerEnter(PointerEventData e){_hover=true;Paint();}
        public void OnPointerExit(PointerEventData e){_hover=false;Paint();}
        public void OnSelect(BaseEventData e){_selected=true;Paint();}
        public void OnDeselect(BaseEventData e){_selected=false;Paint();}
        void OnDisable(){_hover=_selected=false;Paint();_press?.Kill();transform.localScale=Vector3.one;}

        // The one thing a tap always confirms, even on a button whose color state does not change
        // (a ghost row, a disabled-looking tab). Same reduced-motion/companion gate as everything
        // else — UiEntrance.MotionAllowed, not a flag of its own.
        public void OnPointerDown(PointerEventData e)
        {
            if(_control==null||!_control.IsInteractable()||!UiEntrance.MotionAllowed)return;
            _press?.Kill();
            _press=transform.DOScale(.96f,.06f).SetEase(Ease.OutQuad).SetUpdate(true);
        }
        public void OnPointerUp(PointerEventData e)
        {
            if(_control==null)return;
            _press?.Kill();
            _press=transform.DOScale(1f,.14f).SetEase(Ease.OutBack).SetUpdate(true);
        }
    }
}
