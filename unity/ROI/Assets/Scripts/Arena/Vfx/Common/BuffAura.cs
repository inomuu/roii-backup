using System;
using UnityEngine;
using UnityEngine.Rendering;
using Object=UnityEngine.Object;

namespace Roi.Arena
{
    // Local-space geometry in visible actor heights; sampled by the shared buff clock.
    public sealed class BuffAura : IDisposable
    {
        readonly GameObject _root;
        readonly Mesh[] _meshes=new Mesh[2];
        readonly Material[] _materials=new Material[2];
        readonly Transform _ribbon;
        readonly bool _staticCarrier;
        bool _disposed;

        public BuffAura(Transform parent,Shader shader,Texture texture,Color color,float scale,float coreWhiteness=.7f,
            bool staticCarrier=false)
        {
            _staticCarrier=staticCarrier;
            _root=new GameObject("Buff rising aura");
            _root.transform.SetParent(parent,false);
            _root.transform.localScale=Vector3.one*scale;
            if (staticCarrier) _root.transform.SetParent(null,true);
            for (int mode=0;mode<2;mode++)
            {
                var mesh=BuildMesh(mode==1); _meshes[mode]=mesh;
                var material=new Material(shader) { name="Buff aura layer "+mode };
                material.SetTexture("_MainTex",texture); material.SetColor("_Color",color);
                material.SetFloat("_CoreWhiteness",Mathf.Clamp01(coreWhiteness));
                material.SetFloat("_Mode",mode); _materials[mode]=material;
                var layer=new GameObject(mode==0 ? "Buff aura curtain" : "Buff aura ribbon",
                    typeof(MeshFilter),typeof(MeshRenderer));
                layer.transform.SetParent(_root.transform,false);
                layer.GetComponent<MeshFilter>().sharedMesh=mesh;
                var renderer=layer.GetComponent<MeshRenderer>(); renderer.sharedMaterial=material;
                renderer.shadowCastingMode=ShadowCastingMode.Off; renderer.receiveShadows=false;
                if (mode==1) _ribbon=layer.transform;
            }
        }

        public void Sample(float phase,float opacity)
        {
            if (_disposed) return;
            // One rise and a partial orbit, not a continuously pulsing status loop.
            // The legacy mode is retained only for existing effects pending migration.
            if (!_staticCarrier)
            {
                _ribbon.localRotation=Quaternion.Euler(0,-35+phase*115,0);
                _ribbon.localPosition=Vector3.up*(phase*.18f);
            }
            foreach (var material in _materials)
            {
                material.SetFloat("_Phase",phase);
                material.SetFloat("_Opacity",opacity);
            }
        }

        static Mesh BuildMesh(bool ribbon)
        {
            int columns=ribbon ? 96 : 64, rows=ribbon ? 4 : 8;
            var vertices=new Vector3[(columns+1)*(rows+1)];
            var uv=new Vector2[vertices.Length];
            var indices=new int[columns*rows*6];
            for (int x=0;x<=columns;x++)
                for (int y=0;y<=rows;y++)
                {
                    float u=x/(float)columns,v=y/(float)rows;
                    float angle=u*Mathf.PI*2*(ribbon ? 1.15f : 1);
                    float height,radius;
                    if (ribbon)
                    {
                        float taper=Mathf.Sin(u*Mathf.PI);
                        height=.08f+u*.94f+(v-.5f)*.18f*taper;
                        radius=.57f+.07f*u;
                    }
                    else
                    {
                        height=v*1.45f;
                        radius=.54f+.10f*v*v;
                    }
                    int index=x*(rows+1)+y;
                    vertices[index]=new Vector3(Mathf.Cos(angle)*radius,height,Mathf.Sin(angle)*radius);
                    uv[index]=new Vector2(u,v);
                    if (x==columns || y==rows) continue;
                    int at=(x*rows+y)*6,next=index+rows+1;
                    indices[at]=index; indices[at+1]=next; indices[at+2]=index+1;
                    indices[at+3]=index+1; indices[at+4]=next; indices[at+5]=next+1;
                }
            var mesh=new Mesh { name=ribbon ? "Buff helical ribbon" : "Buff open aura curtain" };
            mesh.vertices=vertices; mesh.uv=uv; mesh.triangles=indices;
            mesh.RecalculateNormals(); mesh.RecalculateBounds(); return mesh;
        }

        public void Dispose()
        {
            if (_disposed) return;
            _disposed=true;
            if (_root!=null) { _root.SetActive(false); Destroy(_root); }
            foreach (var mesh in _meshes) if (mesh!=null) Destroy(mesh);
            foreach (var material in _materials) if (material!=null) Destroy(material);
        }
        static void Destroy(Object value)
        { if (Application.isPlaying) Object.Destroy(value); else Object.DestroyImmediate(value); }
    }
}
