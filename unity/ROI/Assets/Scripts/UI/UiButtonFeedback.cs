using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.UI;

namespace Roi.UI
{
    // Event driven: a quiet border on hover, gold on explicit focus; no idle animation.
    [DisallowMultipleComponent, RequireComponent(typeof(Selectable))]
    public sealed class UiButtonFeedback : MonoBehaviour, IPointerEnterHandler, IPointerExitHandler, ISelectHandler, IDeselectHandler
    {
        Selectable _control;
        Image _edge;
        CodexButtonImage _frame;
        bool _hover, _selected;
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
        void OnDisable(){_hover=_selected=false;Paint();}
    }
}
