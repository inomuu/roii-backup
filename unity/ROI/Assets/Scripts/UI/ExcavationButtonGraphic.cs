using System.Collections.Generic;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Resolution-independent ornament from the supplied button reference.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class ExcavationButtonGraphic : MaskableGraphic
    {
        static readonly Color Gold = new Color(.70f, .66f, .53f, .95f);
        static readonly Color FineGold = new Color(.57f, .55f, .46f, .72f);

        Vector2 Point(Vector2 p)
        {
            var r = rectTransform.rect;
            return new Vector2(r.xMin + p.x * r.width / 248f, r.yMax - p.y * r.height / 64f);
        }

        void Line(VertexHelper vh, Vector2 a, Vector2 b, float width, Color tint)
        {
            var normal = new Vector2(-(b-a).y, (b-a).x).normalized;
            int start = vh.currentVertCount;
            float[] offsets = {-width*.5f-.55f,-width*.5f,width*.5f,width*.5f+.55f};
            for(int k=0;k<4;k++) {
                var shade=tint*color;if(k==0||k==3)shade.a=0;
                vh.AddVert(Point(a+normal*offsets[k]),shade,Vector2.zero);
                vh.AddVert(Point(b+normal*offsets[k]),shade,Vector2.zero);
            }
            for(int k=0;k<3;k++) {
                int i=start+k*2;
                vh.AddTriangle(i,i+1,i+2);vh.AddTriangle(i+1,i+3,i+2);
            }
        }

        List<Vector2> Rounded(float inset, float radius)
        {
            var path = new List<Vector2>();
            Vector2[] centers = {new Vector2(248-inset-radius,inset+radius),
                new Vector2(248-inset-radius,64-inset-radius),
                new Vector2(inset+radius,64-inset-radius),new Vector2(inset+radius,inset+radius)};
            for(int corner=0;corner<4;corner++)
                for(int j=0;j<=12;j++) {
                    float a=(-90+corner*90+j*7.5f)*Mathf.Deg2Rad;
                    path.Add(centers[corner]+new Vector2(Mathf.Cos(a),Mathf.Sin(a))*radius);
                }
            return path;
        }

        void Stroke(VertexHelper vh,List<Vector2> path,float width,Color tint,bool closed)
        {
            int start=vh.currentVertCount;
            for(int k=0;k<path.Count;k++) {
                var before=path[k==0?(closed?path.Count-1:0):k-1];
                var after=path[k==path.Count-1?(closed?0:k):k+1];
                var tangent=(after-before).normalized;
                var n=new Vector2(-tangent.y,tangent.x);
                float[] offsets={-width*.5f-.55f,-width*.5f,width*.5f,width*.5f+.55f};
                for(int j=0;j<4;j++) {
                    var shade=tint*color;if(j==0||j==3)shade.a=0;
                    vh.AddVert(Point(path[k]+n*offsets[j]),shade,Vector2.zero);
                }
            }
            int segments=closed?path.Count:path.Count-1;
            for(int k=0;k<segments;k++)for(int j=0;j<3;j++) {
                int a=start+k*4+j,b=start+((k+1)%path.Count)*4+j;
                vh.AddTriangle(a,b,a+1);vh.AddTriangle(a+1,b,b+1);
            }
        }
        void Outline(VertexHelper vh,List<Vector2> path,float width,Color tint)
            => Stroke(vh,path,width,tint,true);

        Vector2 Side(Vector2 p,bool right) => right ? new Vector2(248-p.x,p.y) : p;

        void Curve(VertexHelper vh,bool right,Vector2 a,Vector2 b,Vector2 c,Color tint)
        {
            var path=new List<Vector2>();
            for(int i=0;i<=48;i++) {
                float t=i/48f;
                path.Add(Side((1-t)*(1-t)*a+2*(1-t)*t*b+t*t*c,right));
            }
            Stroke(vh,path,.65f,tint,false);
        }

        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();
            var edge=Rounded(1.5f,12);
            vh.AddVert(Point(new Vector2(124,32)),new Color(.09f,.125f,.16f,.98f)*color,Vector2.zero);
            foreach(var p in edge)
                vh.AddVert(Point(p),new Color(.11f,.15f,.18f,.98f)*color,Vector2.zero);
            for(int i=0;i<edge.Count;i++)vh.AddTriangle(0,i+1,(i+1)%edge.Count+1);
            Outline(vh,edge,1,Gold);
            Outline(vh,Rounded(5,8.5f),.65f,FineGold);
            foreach(bool right in new[]{false,true}) {
                // Both ends mirror the same vertically symmetric lens.
                Curve(vh,right,new Vector2(8,7),new Vector2(48,32),new Vector2(8,57),Gold);
                Curve(vh,right,new Vector2(33,7),new Vector2(23,32),new Vector2(33,57),FineGold);
                for(int i=0;i<17;i++) {
                    float t=(i-8)/8f;
                    var p=new Vector2(39-5*(1-t*t),32+25*t);
                    Line(vh,Side(p-new Vector2(0,.18f),right),Side(p+new Vector2(0,.18f),right),.65f,FineGold);
                }
                var d=Side(new Vector2(28,32),right);
                Line(vh,d+new Vector2(-2,0),d+new Vector2(0,-2),.8f,Gold);
                Line(vh,d+new Vector2(0,-2),d+new Vector2(2,0),.8f,Gold);
                Line(vh,d+new Vector2(2,0),d+new Vector2(0,2),.8f,Gold);
                Line(vh,d+new Vector2(0,2),d+new Vector2(-2,0),.8f,Gold);
            }
        }
    }
}
