using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Static thin celestial rings; the transform rotates, the mesh is not rebuilt per frame.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class CrestOrbit : MaskableGraphic
    {
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();var center=rectTransform.rect.center;
            float radius=Mathf.Min(rectTransform.rect.width,rectTransform.rect.height)*.49f;
            for(int ring=0;ring<2;ring++)for(int i=0;i<128;i++)
            {
                if(ring==1&&(i%32)<9)continue;
                float r=radius-ring*8,a=i*Mathf.PI/64,b=(i+1)*Mathf.PI/64;
                var p=new Vector2(Mathf.Cos(a),Mathf.Sin(a));var q=new Vector2(Mathf.Cos(b),Mathf.Sin(b));
                int n=vh.currentVertCount;
                vh.AddVert(center+p*r,color,Vector2.zero);vh.AddVert(center+q*r,color,Vector2.zero);
                vh.AddVert(center+q*(r-.65f),color,Vector2.zero);vh.AddVert(center+p*(r-.65f),color,Vector2.zero);
                vh.AddTriangle(n,n+1,n+2);vh.AddTriangle(n,n+2,n+3);
            }
            for(int i=0;i<8;i++)
            {
                float a=i*Mathf.PI/4;var p=center+new Vector2(Mathf.Cos(a),Mathf.Sin(a))*(radius-4);
                int n=vh.currentVertCount;
                vh.AddVert(p+Vector2.up*3,color,Vector2.zero);vh.AddVert(p+Vector2.right*2,color,Vector2.zero);
                vh.AddVert(p-Vector2.up*3,color,Vector2.zero);vh.AddVert(p-Vector2.right*2,color,Vector2.zero);
                vh.AddTriangle(n,n+1,n+2);vh.AddTriangle(n,n+2,n+3);
            }
        }
    }
}
