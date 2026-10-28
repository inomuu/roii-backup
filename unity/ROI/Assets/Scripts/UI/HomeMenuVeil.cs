using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // A continuous local backdrop gradient; it leaves the face/left half at full brightness.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class HomeMenuVeil : MaskableGraphic
    {
        protected override void OnPopulateMesh(VertexHelper mesh)
        {
            mesh.Clear(); var r=rectTransform.rect;
            const int steps=24;
            for(int n=0;n<=steps;n++)
            {
                float t=(float)n/steps;
                float opacity=Mathf.SmoothStep(0,1,t)*color.a;
                var c=new Color(color.r,color.g,color.b,opacity);
                float x=Mathf.Lerp(r.xMin,r.xMax,t);
                mesh.AddVert(new Vector3(x,r.yMin),c,Vector2.zero);
                mesh.AddVert(new Vector3(x,r.yMax),c,Vector2.up);
                if(n==0)continue;int i=n*2;
                mesh.AddTriangle(i-2,i-1,i+1);mesh.AddTriangle(i+1,i,i-2);
            }
        }
    }
}
