using UnityEngine;
using UnityEngine.UI;
namespace Roi.UI {
    // Small retained corner details, separate from hit targets and live labels.
    public static class FuturistChrome {
        public static void Attach(RectTransform parent) {
            Rule(parent,"TopSignal",new Vector2(0,1),new Vector2(16,-1),new Vector2(52,2),UiTheme.Accent);
            Rule(parent,"BottomSignal",new Vector2(1,0),new Vector2(-16,1),new Vector2(28,2),new Color(.88f,.79f,.57f,.4f));
        }
        static void Rule(RectTransform parent,string name,Vector2 anchor,Vector2 offset,Vector2 size,Color color) {
            var im=UiTheme.Flat(parent,name,color); im.raycastTarget=false;
            var r=im.rectTransform;r.anchorMin=r.anchorMax=r.pivot=anchor;r.anchoredPosition=offset;r.sizeDelta=size;
        }
    }
}
