using UnityEngine;
using UnityEngine.UI;
namespace Roi.UI
{
    // Shared retained geometry. No texture sampling or per-frame layout work.
    internal static class CodexFrameMesh
    {
        internal static void Draw(VertexHelper vh, Rect r, Color color, bool Selected=false,
            bool Primary=false, bool DoubleFrame=false, bool Insignia=false, bool Ribbon=false, bool Opaque=false, bool Raised=false)
        {
            vh.Clear();
            if (Ribbon)
            {
                vh.AddVert(new Vector2(r.xMin,r.yMin),UiTheme.Accent*color,Vector2.zero);
                vh.AddVert(new Vector2(r.xMin,r.yMax),UiTheme.Accent*color,Vector2.zero);
                vh.AddVert(new Vector2(r.xMax,r.yMax),UiTheme.Accent*color,Vector2.zero);
                vh.AddVert(new Vector2(r.xMax-r.height,r.yMin),UiTheme.Accent*color,Vector2.zero);
                vh.AddTriangle(0,1,2);vh.AddTriangle(0,2,3);return;
            }
            r.xMin += 2; r.xMax -= 2; r.yMin += 2; r.yMax -= 2;
            if (r.width <= 0 || r.height <= 0) return;
            float cut = Primary ? Mathf.Min(22, r.height * .48f) : Mathf.Min(7, r.height * .16f);
            var p = Corners(r, cut);
            Color top = Primary ? UiTheme.FramePrimary : UiTheme.FrameTop;
            Color bottom = UiTheme.FrameBottom;
            if (Raised)
            {
                top = Selected ? UiTheme.FrameSelected : UiTheme.SurfaceActive;
                bottom = UiTheme.Surface;
            }
            if(Opaque){top.a=1;bottom.a=1;}
            vh.AddVert(r.center, Color.Lerp(bottom, top, .5f) * color, Vector2.zero);
            foreach (var pt in p)
                vh.AddVert(pt, Color.Lerp(bottom, top, Mathf.InverseLerp(r.yMin, r.yMax, pt.y)) * color, Vector2.zero);
            for (int i = 0; i < p.Length; i++) vh.AddTriangle(0, i + 1, (i + 1) % p.Length + 1);
            var edge = Selected || Primary ? UiTheme.Accent : Raised ? UiTheme.FrameRaisedEdge : UiTheme.FrameEdge;
            Outline(vh, p, Selected || Primary ? 1.5f : Raised ? 1.2f : 1f, edge * color);
            if (Raised)
            {
                Line(vh,new Vector2(r.xMin+cut,r.yMin-1),new Vector2(r.xMax-cut,r.yMin-1),2,new Color(.005f,.012f,.016f,.9f)*color);
                Line(vh,new Vector2(r.xMin+cut+1,r.yMax-1.5f),new Vector2(r.xMax-cut-1,r.yMax-1.5f),.8f,new Color(.88f,.93f,.94f,.25f)*color);
            }
            // A single edge keeps dense menus quiet; gold is reserved for action/selection.
            if (Selected && Insignia)
                Diamond(vh, new Vector2(r.xMax - 20, r.yMax - 20), 4.5f, UiTheme.Accent * color);

        }

        static Vector2[] Corners(Rect r, float c)
        {
            c=Mathf.Min(c,Mathf.Min(r.width,r.height)*.5f);
            var points=new System.Collections.Generic.List<Vector2>(28);
            var centers=new[]{new Vector2(r.xMin+c,r.yMin+c),new Vector2(r.xMin+c,r.yMax-c),
                new Vector2(r.xMax-c,r.yMax-c),new Vector2(r.xMax-c,r.yMin+c)};
            for(int corner=0;corner<4;corner++)for(int step=0;step<=6;step++)
            {
                float angle=(-90-corner*90-step*15)*Mathf.Deg2Rad;
                points.Add(centers[corner]+new Vector2(Mathf.Cos(angle),Mathf.Sin(angle))*c);
            }
            return points.ToArray();
        }
        static void Outline(VertexHelper vh, Vector2[] p, float w, Color c)
        { for (int i = 0; i < p.Length; i++) Line(vh, p[i], p[(i + 1) % p.Length], w, c); }
        static void Line(VertexHelper vh, Vector2 a, Vector2 b, float w, Color c)
        {
            var d = (b-a).normalized; var n = new Vector2(-d.y,d.x)*w*.5f; int i = vh.currentVertCount;
            vh.AddVert(a-n,c,Vector2.zero);vh.AddVert(a+n,c,Vector2.zero);vh.AddVert(b+n,c,Vector2.zero);vh.AddVert(b-n,c,Vector2.zero);
            vh.AddTriangle(i,i+1,i+2);vh.AddTriangle(i,i+2,i+3);
        }
        static void Diamond(VertexHelper vh, Vector2 p, float s, Color c)
        {
            int i=vh.currentVertCount;
            vh.AddVert(p+Vector2.up*s,c,Vector2.zero);vh.AddVert(p+Vector2.right*s,c,Vector2.zero);
            vh.AddVert(p+Vector2.down*s,c,Vector2.zero);vh.AddVert(p+Vector2.left*s,c,Vector2.zero);
            vh.AddTriangle(i,i+1,i+2);vh.AddTriangle(i,i+2,i+3);
        }
    }
}
