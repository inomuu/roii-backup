using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Retained decorative mesh. No Update, animation, camera, render target or post effect.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class RpgBackdrop : MaskableGraphic
    {
        public static void Create(Transform parent)
        {
            var go=new GameObject("TacticalBackdrop",typeof(RectTransform));
            go.transform.SetParent(parent,false);
            var rt=(RectTransform)go.transform;rt.anchorMin=Vector2.zero;rt.anchorMax=Vector2.one;rt.offsetMin=rt.offsetMax=Vector2.zero;
            var overlay=new GameObject("Atmosphere",typeof(RectTransform));overlay.transform.SetParent(go.transform,false);
            var or=(RectTransform)overlay.transform;or.anchorMin=Vector2.zero;or.anchorMax=Vector2.one;or.offsetMin=or.offsetMax=Vector2.zero;
            overlay.AddComponent<RpgBackdrop>().raycastTarget=false;
            // Quiet navy/violet atmosphere leaves the inventory art and text in focus.

        }
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();var r=rectTransform.rect;
            var left=UiTheme.BackdropTop;var right=UiTheme.BackdropBottom;
            vh.AddVert(new Vector2(r.xMin,r.yMin),right,Vector2.zero);
            vh.AddVert(new Vector2(r.xMin,r.yMax),left,Vector2.up);
            vh.AddVert(new Vector2(r.xMax,r.yMax),left,Vector2.one);
            vh.AddVert(new Vector2(r.xMax,r.yMin),right,Vector2.right);
            vh.AddTriangle(0,1,2);vh.AddTriangle(0,2,3);

        }
        static void Line(VertexHelper vh,Vector2 a,Vector2 b,float width,Color c)
        {
            var d=(b-a).normalized;var n=new Vector2(-d.y,d.x)*width*.5f;int i=vh.currentVertCount;
            vh.AddVert(a-n,c,Vector2.zero);vh.AddVert(a+n,c,Vector2.zero);vh.AddVert(b+n,c,Vector2.zero);vh.AddVert(b-n,c,Vector2.zero);
            vh.AddTriangle(i,i+1,i+2);vh.AddTriangle(i,i+2,i+3);
        }
    }
}
