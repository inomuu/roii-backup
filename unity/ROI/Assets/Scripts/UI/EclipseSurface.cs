using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Resolution-independent, chamfered menu surfaces. No image frames or per-frame work.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class EclipseSurface : MaskableGraphic
    {
        public bool Primary;
        public static EclipseSurface Add(Transform parent,bool primary=false)
        {
            var go=new GameObject("EclipseFrame",typeof(RectTransform));go.transform.SetParent(parent,false);
            go.transform.SetAsFirstSibling();var g=go.AddComponent<EclipseSurface>();g.Primary=primary;g.raycastTarget=false;
            var r=g.rectTransform;r.anchorMin=Vector2.zero;r.anchorMax=Vector2.one;r.offsetMin=r.offsetMax=Vector2.zero;return g;
        }
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();var r=rectTransform.rect;float cut=Mathf.Min(24,r.height*.24f);
            var points=new[]{new Vector2(r.xMin+cut,r.yMin),new Vector2(r.xMin,r.yMin+cut),
                new Vector2(r.xMin,r.yMax),new Vector2(r.xMax-cut,r.yMax),
                new Vector2(r.xMax,r.yMax-cut),new Vector2(r.xMax,r.yMin)};
            var top=Primary?new Color(.95f,.89f,.72f):new Color(.105f,.13f,.145f,.93f);
            var bottom=Primary?new Color(.72f,.65f,.45f):new Color(.026f,.036f,.043f,.96f);
            vh.AddVert(r.center,Color.Lerp(bottom,top,.5f)*color,Vector2.zero);
            for(int i=0;i<points.Length;i++)vh.AddVert(points[i],Color.Lerp(bottom,top,Mathf.InverseLerp(r.yMin,r.yMax,points[i].y))*color,Vector2.zero);
            for(int i=0;i<points.Length;i++)vh.AddTriangle(0,i+1,(i+1)%points.Length+1);
            var ink=(Primary?new Color(1,.94f,.74f,.9f):new Color(.57f,.68f,.74f,.40f))*color;
            for(int i=0;i<points.Length;i++)Line(vh,points[i],points[(i+1)%points.Length],.8f,ink);
            Line(vh,new Vector2(r.xMin+cut+8,r.yMax-2),new Vector2(r.xMin+cut+38,r.yMax-2),2,UiTheme.Accent*color);
        }

        static void Line(VertexHelper vh,Vector2 a,Vector2 b,float width,Color c)
        {
            var d=(b-a).normalized;var n=new Vector2(-d.y,d.x)*width*.5f;int i=vh.currentVertCount;
            vh.AddVert(a-n,c,Vector2.zero);vh.AddVert(a+n,c,Vector2.zero);vh.AddVert(b+n,c,Vector2.zero);vh.AddVert(b-n,c,Vector2.zero);
            vh.AddTriangle(i,i+1,i+2);vh.AddTriangle(i,i+2,i+3);
        }
    }
}

