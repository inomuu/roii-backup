using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class ExcavationChrome : MaskableGraphic
    {
        public bool Gem, Solid;
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();var r=rectTransform.rect;
            if(Gem) {
                Vector2[] points={new Vector2(r.center.x,r.yMax),new Vector2(r.xMax,r.center.y),
                    new Vector2(r.center.x,r.yMin),new Vector2(r.xMin,r.center.y),r.center};
                Color[] colors={new Color(.92f,.97f,1),new Color(.38f,.60f,.80f),new Color(.63f,.79f,.93f),new Color(.76f,.89f,1),new Color(.90f,.96f,1)};
                for(int i=0;i<5;i++)vh.AddVert(points[i],colors[i]*color,Vector2.zero);
                for(int i=0;i<4;i++)vh.AddTriangle(4,i,(i+1)%4);
                return;
            }
            float radius=r.height*.5f-1;
            // A light tint lets the scene show through without dimming the wallet contents.
            var fill=Solid?color:new Color(.025f,.045f,.06f,.22f)*color;
            var line=Solid?color:new Color(.63f,.67f,.66f,.38f)*color;
            vh.AddVert(r.center,fill,Vector2.zero);
            const int n=64;
            for(int i=0;i<n;i++) {
                float a=2*Mathf.PI*i/n;
                float cx=Mathf.Cos(a)>=0?r.xMax-radius-1:r.xMin+radius+1;
                var center=new Vector2(cx,r.center.y);var normal=new Vector2(Mathf.Cos(a),Mathf.Sin(a));
                vh.AddVert(center+normal*(radius-1),fill,Vector2.zero);
                vh.AddVert(center+normal*radius,line,Vector2.zero);
                var clear=line;clear.a=0;vh.AddVert(center+normal*(radius+.75f),clear,Vector2.zero);
            }
            for(int i=0;i<n;i++) {
                int a=1+i*3,b=1+((i+1)%n)*3;vh.AddTriangle(0,a,b);
                for(int j=0;j<2;j++){vh.AddTriangle(a+j,a+j+1,b+j);vh.AddTriangle(a+j+1,b+j+1,b+j);}
            }
        }
    }
}
