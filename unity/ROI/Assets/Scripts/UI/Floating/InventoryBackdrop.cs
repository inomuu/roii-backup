using UnityEngine;
using UnityEngine.UI;
namespace Roi.UI {
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class InventoryBackdrop : MaskableGraphic {
        protected override void Awake() { base.Awake(); raycastTarget=false; }
        protected override void OnPopulateMesh(VertexHelper vh) {
            vh.Clear(); var r=rectTransform.rect;
            var dark=UiTheme.Panel; var light=UiTheme.SurfaceHover;
            vh.AddVert(new Vector2(r.xMin,r.yMin),dark,Vector2.zero);
            vh.AddVert(new Vector2(r.xMin,r.yMax),dark,Vector2.up);
            vh.AddVert(new Vector2(r.xMax,r.yMax),light,Vector2.one);
            vh.AddVert(new Vector2(r.xMax,r.yMin),light,Vector2.right);
            vh.AddTriangle(0,1,2); vh.AddTriangle(2,3,0);
        }
    }
}
