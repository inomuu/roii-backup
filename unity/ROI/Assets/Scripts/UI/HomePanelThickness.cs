using System.Collections.Generic;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Thin real side faces, drawn before the original uGUI front face.
    [RequireComponent(typeof(Graphic))]
    public sealed class HomePanelThickness : BaseMeshEffect
    {
        public float Thickness = 6;
        readonly List<UIVertex> _front = new();
        public override void ModifyMesh(VertexHelper mesh)
        {
            if (!IsActive() || mesh.currentVertCount == 0) return;
            _front.Clear(); mesh.GetUIVertexStream(_front);
            var r = ((RectTransform)transform).rect;
            var c = _front[0].color;
            mesh.Clear();
            var bl = new Vector3(r.xMin,r.yMin,0); var tl = new Vector3(r.xMin,r.yMax,0);
            var tr = new Vector3(r.xMax,r.yMax,0); var br = new Vector3(r.xMax,r.yMin,0);
            Side(mesh,bl,tl,c,.55f); Side(mesh,tl,tr,c,.72f);
            Side(mesh,tr,br,c,.35f); Side(mesh,br,bl,c,.28f);
            // AddUIVertexTriangleStream replaces the native buffers; append explicitly.
            int frontStart=mesh.currentVertCount;
            foreach(var v in _front)mesh.AddVert(v);
            for(int i=0;i<_front.Count;i+=3)mesh.AddTriangle(frontStart+i,frontStart+i+1,frontStart+i+2);
        }
        void Side(VertexHelper mesh,Vector3 a,Vector3 b,Color color,float value)
        {
            color = new Color(color.r*value,color.g*value,color.b*value,color.a);
            var back = new Vector3(0,0,Thickness);
            int i=mesh.currentVertCount;
            mesh.AddVert(a,color,Vector2.zero);mesh.AddVert(b,color,Vector2.up);
            mesh.AddVert(b+back,color,Vector2.one);mesh.AddVert(a+back,color,Vector2.right);
            mesh.AddTriangle(i,i+1,i+2);mesh.AddTriangle(i+2,i+3,i);
        }
    }
}
