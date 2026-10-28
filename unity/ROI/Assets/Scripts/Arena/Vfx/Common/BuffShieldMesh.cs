using System.Collections.Generic;
using UnityEngine;

namespace Roi.Arena
{
    public static class BuffShieldMesh
    {
        // A pointed heraldic shield with a convex face, a back and a separate bevel rim.
        public static Mesh Create()
        {
            var outline=new[] { new Vector2(-.48f,.91f),new Vector2(0,1),new Vector2(.48f,.91f),
                new Vector2(.45f,.43f),new Vector2(.28f,.18f),new Vector2(0,0),
                new Vector2(-.28f,.18f),new Vector2(-.45f,.43f) };
            const int subdivisions=6, rings=6;
            int count=outline.Length*subdivisions;
            var vertices=new List<Vector3>(); var uv=new List<Vector2>();
            var surface=new List<Vector2>(); var triangles=new List<int>();
            var center=new Vector2(0,.55f);
            Vector2 Boundary(int i) => Vector2.Lerp(outline[(i/subdivisions)%outline.Length],
                outline[(i/subdivisions+1)%outline.Length],i%subdivisions/(float)subdivisions);
            void Vertex(Vector2 xy,float radius,bool back,bool edge)
            {
                vertices.Add(new Vector3(xy.x,xy.y,back ? -.04f : .20f*(1-radius*radius)));
                uv.Add(new Vector2(xy.x+.5f,xy.y)); surface.Add(new Vector2(radius,edge ? 1 : 0));
            }
            void Triangle(int a,int b,int c,bool back)
            { triangles.Add(a); triangles.Add(back ? c : b); triangles.Add(back ? b : c); }
            for (int face=0;face<2;face++)
            {
                bool back=face==1; int start=vertices.Count;
                Vertex(center,0,back,false);
                for (int ring=1;ring<=rings;ring++)
                    for (int i=0;i<count;i++)
                    {
                        float radius=ring/(float)rings;
                        Vertex(Vector2.Lerp(center,Boundary(i),radius),radius,back,false);
                        int here=start+1+(ring-1)*count+i, next=start+1+(ring-1)*count+(i+1)%count;
                        if (ring==1) Triangle(start,next,here,back);
                        else
                        {
                            Triangle(here-count,next,here,back);
                            Triangle(here-count,next-count,next,back);
                        }
                    }
            }
            int rim=vertices.Count;
            for (int i=0;i<=count;i++)
            {
                Vertex(Boundary(i%count),1,false,true); Vertex(Boundary(i%count),1,true,true);
                if (i==count) continue;
                int a=rim+i*2;
                Triangle(a,a+2,a+1,false); Triangle(a+1,a+2,a+3,false);
            }
            var mesh=new Mesh { name="Convex pointed buff shield" };
            mesh.SetVertices(vertices); mesh.SetUVs(0,uv); mesh.SetUVs(1,surface); mesh.SetTriangles(triangles,0);
            mesh.RecalculateNormals(); mesh.RecalculateBounds(); return mesh;
        }
    }
}
