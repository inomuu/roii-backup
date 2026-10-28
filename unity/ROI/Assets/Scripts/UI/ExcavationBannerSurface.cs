using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Retained banner geometry: the portrait sits above the frame, the caption above the portrait.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class ExcavationBannerSurface : MaskableGraphic
    {
        public bool Selected;
        public bool Caption;

        protected override void OnPopulateMesh(VertexHelper vh)
        {
            var r = rectTransform.rect;
            if (Caption)
            {
                vh.Clear();
                r = new Rect(r.x + 7, r.y + 7, r.width - 14, r.height - 14);
                var top = new Vector2(r.xMin + r.width * .34f, r.yMax);
                var bottom = new Vector2(r.xMax - 12, r.yMin);
                var points = new[] {
                    new Vector2(r.xMin + 5, r.yMax), top, bottom,
                    new Vector2(r.xMin + 5, r.yMin), new Vector2(r.xMin, r.yMin + 5),
                    new Vector2(r.xMin, r.yMax - 5)
                };
                for (int i = 0; i < points.Length; i++)
                {
                    var ink = Color.Lerp(new Color(.022f,.041f,.046f,.99f),
                        new Color(.043f,.064f,.072f,.96f), Mathf.InverseLerp(r.yMin,r.yMax,points[i].y));
                    vh.AddVert(points[i], ink * color, Vector2.zero);
                }
                for (int i = 1; i < points.Length - 1; i++) vh.AddTriangle(0,i,i+1);
                Line(vh, top, bottom, .65f, new Color(.66f,.59f,.42f,Selected ? .58f : .22f) * color);
                return;
            }

            CodexFrameMesh.Draw(vh,r,color,Selected:Selected,DoubleFrame:true,Opaque:true);
            r = new Rect(r.x + 3, r.y + 3, r.width - 6, r.height - 6);
            var edge = (Selected ? UiTheme.Accent : new Color(.56f,.55f,.47f,.8f)) * color;
            float cut = 10, length = 24;
            foreach (int sx in new[] {-1,1}) foreach (int sy in new[] {-1,1})
            {
                var corner = new Vector2(sx < 0 ? r.xMin : r.xMax, sy < 0 ? r.yMin : r.yMax);
                var a = corner - new Vector2(sx * length,0);
                var b = corner - new Vector2(sx * cut,0);
                var c = corner - new Vector2(0,sy * cut);
                var d = corner - new Vector2(0,sy * length);
                Line(vh,a,b,2.2f,edge);Line(vh,b,c,2.2f,edge);Line(vh,c,d,2.2f,edge);
            }
        }

        static void Line(VertexHelper vh,Vector2 a,Vector2 b,float width,Color tint)
        {
            var direction=(b-a).normalized;
            var n=new Vector2(-direction.y,direction.x)*width*.5f;
            int i=vh.currentVertCount;
            vh.AddVert(a-n,tint,Vector2.zero);vh.AddVert(a+n,tint,Vector2.zero);
            vh.AddVert(b+n,tint,Vector2.zero);vh.AddVert(b-n,tint,Vector2.zero);
            vh.AddTriangle(i,i+1,i+2);vh.AddTriangle(i,i+2,i+3);
        }
    }
}
