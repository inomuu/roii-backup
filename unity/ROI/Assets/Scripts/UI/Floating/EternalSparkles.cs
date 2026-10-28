using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // A small, deterministic jewel shimmer for the top two tiers. It is a retained graphic and
    // stays entirely inside the existing cell, so it adds no texture or layout dependency.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class EternalSparkles : MaskableGraphic
    {
        static readonly Vector2[] Points =
        {
            new(.17f,.72f),new(.34f,.27f),new(.56f,.62f),new(.82f,.38f),
            new(.77f,.82f),new(.46f,.48f),new(.23f,.34f),new(.62f,.86f),
        };
        bool _enabled, _divine;
        public void Bind(bool enabled, bool divine=false)
        {
            if(_enabled==enabled&&_divine==divine)return;
            _enabled=enabled;_divine=divine;gameObject.SetActive(enabled);
        }
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();if(!_enabled)return;
            var rect=rectTransform.rect;float scale=Mathf.Min(rect.width,rect.height);
            for(int i=0;i<Points.Length;i++)
            {
                var p=new Vector2(Mathf.Lerp(rect.xMin,rect.xMax,Points[i].x),Mathf.Lerp(rect.yMin,rect.yMax,Points[i].y));
                float pulse=.5f+.5f*Mathf.Sin(Time.unscaledTime*(1.35f+(i%3)*.32f)+i*1.7f);
                float r=scale*(i%3==0?.052f:.027f);
                var color=i%3==0
                    ?(_divine?new Color(1f,.92f,.98f,.34f+.34f*pulse):new Color(1f,.84f,.48f,.30f+.35f*pulse))
                    :(_divine?new Color(1f,.72f,.86f,.18f+.19f*pulse):new Color(.85f,.72f,1f,.16f+.19f*pulse));
                Diamond(vh,p,r,color);
                if(i%3==0)Cross(vh,p,r*1.9f,_divine?new Color(1f,.96f,1f,.16f+.16f*pulse):new Color(1f,.92f,.68f,.12f+.16f*pulse));
            }
        }
        void Update(){if(_enabled)SetVerticesDirty();}
        static void Diamond(VertexHelper vh,Vector2 c,float r,Color color)
        {int n=vh.currentVertCount;vh.AddVert(c+Vector2.up*r,color,Vector2.zero);vh.AddVert(c+Vector2.right*r*.62f,color,Vector2.zero);vh.AddVert(c+Vector2.down*r,color,Vector2.zero);vh.AddVert(c+Vector2.left*r*.62f,color,Vector2.zero);vh.AddTriangle(n,n+1,n+2);vh.AddTriangle(n,n+2,n+3);}
        static void Cross(VertexHelper vh,Vector2 c,float r,Color color)
        {int n=vh.currentVertCount;vh.AddVert(c+Vector2.up*r,color,Vector2.zero);vh.AddVert(c+Vector2.right*r*.1f,color,Vector2.zero);vh.AddVert(c+Vector2.down*r,color,Vector2.zero);vh.AddVert(c+Vector2.left*r*.1f,color,Vector2.zero);vh.AddTriangle(n,n+1,n+2);vh.AddTriangle(n,n+2,n+3);}
    }
}
