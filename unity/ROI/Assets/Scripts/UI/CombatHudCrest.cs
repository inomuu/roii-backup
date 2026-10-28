using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Filled winged-blade silhouette stays readable at HUD size.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class CombatHudCrest : MaskableGraphic
    {
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();
            Polygon(vh, Blade);
            Polygon(vh, Guard);
            Polygon(vh, Grip);
            Polygon(vh, Pommel);
            for(int side=0;side<2;side++)
                for(int feather=0;feather<2;feather++)
                    Wing(vh,side==1,feather);
        }
        static readonly Vector2[] Blade = { new Vector2(.5f,.96f),new Vector2(.58f,.76f),new Vector2(.55f,.40f),new Vector2(.45f,.40f),new Vector2(.42f,.76f) };
        static readonly Vector2[] Guard = { new Vector2(.27f,.40f),new Vector2(.50f,.46f),new Vector2(.73f,.40f),new Vector2(.65f,.32f),new Vector2(.35f,.32f) };
        static readonly Vector2[] Grip = { new Vector2(.46f,.30f),new Vector2(.54f,.30f),new Vector2(.54f,.16f),new Vector2(.46f,.16f) };
        static readonly Vector2[] Pommel = { new Vector2(.50f,.18f),new Vector2(.58f,.11f),new Vector2(.50f,.04f),new Vector2(.42f,.11f) };
        void Wing(VertexHelper vh,bool mirror,int feather)
        {
            float y=.69f-feather*.115f, x=.08f+feather*.045f;
            int n=vh.currentVertCount;
            Vertex(vh,new Vector2(x,y+.14f),mirror);
            Vertex(vh,new Vector2(.36f,y-.035f),mirror);
            Vertex(vh,new Vector2(.38f,y-.13f),mirror);
            Vertex(vh,new Vector2(x+.045f,y+.01f),mirror);
            vh.AddTriangle(n,n+1,n+2);vh.AddTriangle(n,n+2,n+3);
        }
        void Polygon(VertexHelper vh,Vector2[] points)
        {
            int n=vh.currentVertCount;
            foreach(var p in points)Vertex(vh,p,false);
            for(int i=1;i<points.Length-1;i++)vh.AddTriangle(n,n+i,n+i+1);
        }
        void Vertex(VertexHelper vh,Vector2 p,bool mirror)
        {
            if(mirror)p.x=1f-p.x;
            var r=GetPixelAdjustedRect();
            vh.AddVert(r.min+Vector2.Scale(p,r.size),color,Vector2.zero);
        }
    }
}
