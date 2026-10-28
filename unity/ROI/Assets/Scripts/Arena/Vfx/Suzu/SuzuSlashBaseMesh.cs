using UnityEngine;

namespace Roi.Arena
{
    // Untextured carrier only. Silhouette, UV motion and erosion belong to later material work.
    public static class SuzuSlashBaseMesh
    {
        public const int Segments=64, Rows=3;
        public const int VerticesPerSide=(Segments+1)*Rows;
        public const float InnerRadius=.85f, OuterRadius=1.7f, ArcDegrees=270;
        public const float RidgeHalfHeight=.16f;

        public static Mesh Build()
        {
            var vertices=new Vector3[VerticesPerSide*2];
            var uv=new Vector2[vertices.Length];
            int indicesPerSide=Segments*(Rows-1)*6;
            var triangles=new int[indicesPerSide*2];
            // Upper/lower ridges form a lens section, meeting at sharp inner/outer edges.
            // Angular ends stay open; each surface keeps its own mirrored UV strip.
            for (int side=0;side<2;side++) for (int s=0;s<=Segments;s++)
            {
                float u=s/(float)Segments,angle=(-45+ArcDegrees*u)*Mathf.Deg2Rad;
                for (int row=0;row<Rows;row++)
                {
                    float v=row/(float)(Rows-1),radius=Mathf.Lerp(InnerRadius,OuterRadius,v);
                    int k=side*VerticesPerSide+s*Rows+row;
                    float height=row==1 ? RidgeHalfHeight*(side==0 ? 1 : -1) : 0;
                    vertices[k]=new Vector3(Mathf.Cos(angle)*radius,height,Mathf.Sin(angle)*radius);
                    uv[k]=new Vector2(side==0 ? u : 1-u,v);
                    if (s==Segments || row==Rows-1) continue;
                    int at=side*indicesPerSide+(s*(Rows-1)+row)*6;
                    triangles[at]=k; triangles[at+1]=k+Rows; triangles[at+2]=k+1;
                    triangles[at+3]=k+1; triangles[at+4]=k+Rows; triangles[at+5]=k+Rows+1;
                    if (side==1)
                    {
                        (triangles[at+1],triangles[at+2])=(triangles[at+2],triangles[at+1]);
                        (triangles[at+4],triangles[at+5])=(triangles[at+5],triangles[at+4]);
                    }
                }
            }
            var mesh=new Mesh { name="Suzu slash base - 270 degree arc" };
            mesh.vertices=vertices; mesh.uv=uv; mesh.triangles=triangles;
            mesh.RecalculateNormals(); mesh.RecalculateBounds(); return mesh;
        }
    }
}
