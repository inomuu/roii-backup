using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Sample the user's supplied English button artwork without redrawing its lettering.
    // The original reference remains untouched; UVs select only the button.
    public sealed class ExcavationReferenceButton : RawImage
    {
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();
            var r=rectTransform.rect;
            float radius=r.height*.17f;
            Vector2[] centers={new Vector2(r.xMax-radius,r.yMax-radius),
                new Vector2(r.xMax-radius,r.yMin+radius),
                new Vector2(r.xMin+radius,r.yMin+radius),
                new Vector2(r.xMin+radius,r.yMax-radius)};
            Add(vh,r.center,r);
            for(int corner=0;corner<4;corner++)for(int i=0;i<=16;i++) {
                float a=(90-corner*90-i*90f/16)*Mathf.Deg2Rad;
                Add(vh,centers[corner]+new Vector2(Mathf.Cos(a),Mathf.Sin(a))*radius,r);
            }
            int count=vh.currentVertCount-1;
            for(int i=0;i<count;i++)vh.AddTriangle(0,i+1,(i+1)%count+1);
        }
        void Add(VertexHelper vh,Vector2 p,Rect r)
        {
            var uv=new Vector2(uvRect.x+(p.x-r.xMin)/r.width*uvRect.width,
                uvRect.y+(p.y-r.yMin)/r.height*uvRect.height);
            vh.AddVert(p,color,uv);
        }
    }
}
