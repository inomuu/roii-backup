using System.Collections.Generic;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Retained slate gradient and a soft layered shadow for plain UI panels; no texture allocation.
    [RequireComponent(typeof(Graphic))]
    public sealed class EquipmentSurface : BaseMeshEffect
    {
        readonly List<UIVertex> _vertices=new();
        float _opacity=1f;
        public static void Apply(Image image,float opacity=1f)
        {
            var surface=image.GetComponent<EquipmentSurface>();
            if(surface==null)surface=image.gameObject.AddComponent<EquipmentSurface>();
            surface._opacity=opacity;image.SetVerticesDirty();
        }
        public override void ModifyMesh(VertexHelper vh)
        {
            if(!IsActive()||vh.currentVertCount==0)return;
            _vertices.Clear();vh.GetUIVertexStream(_vertices);vh.Clear();
            var rect=((RectTransform)transform).rect;
            float alpha=_vertices[0].color.a/255f*_opacity;
            for(int layer=6;layer>0;layer--)
            {
                float spread=layer*1.45f;
                var shadow=new Rect(rect.xMin-spread,rect.yMin-spread-4,rect.width+2*spread,rect.height+2*spread);
                Quad(vh,shadow,new Color(.015f,.025f,.045f,alpha*.009f*(7-layer)));
            }
            for(int i=0;i<_vertices.Count;i++)
            {
                var v=_vertices[i];Color c=v.color;
                float t=Mathf.InverseLerp(rect.yMin,rect.yMax,v.position.y);
                float light=Mathf.Lerp(.76f,1.07f,t);
                v.color=new Color(c.r*light,c.g*light,c.b*light,c.a*_opacity);_vertices[i]=v;
            }
            int start=vh.currentVertCount;
            foreach(var v in _vertices)vh.AddVert(v);
            for(int i=0;i<_vertices.Count;i+=3)vh.AddTriangle(start+i,start+i+1,start+i+2);
        }
        static void Quad(VertexHelper vh,Rect r,Color color)
        {
            int n=vh.currentVertCount;
            vh.AddVert(new Vector3(r.xMin,r.yMin),color,Vector2.zero);
            vh.AddVert(new Vector3(r.xMin,r.yMax),color,Vector2.zero);
            vh.AddVert(new Vector3(r.xMax,r.yMax),color,Vector2.zero);
            vh.AddVert(new Vector3(r.xMax,r.yMin),color,Vector2.zero);
            vh.AddTriangle(n,n+1,n+2);vh.AddTriangle(n,n+2,n+3);
        }
    }
}
