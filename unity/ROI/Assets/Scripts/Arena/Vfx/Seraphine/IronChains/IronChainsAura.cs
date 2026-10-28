using System;
using UnityEngine;
using UnityEngine.Rendering;
using Object=UnityEngine.Object;

namespace Roi.Arena
{
    // Owned by the first chain only; additional target chains do not duplicate the caster aura.
    internal sealed class IronChainsAura : IDisposable
    {
        readonly GameObject _root;
        readonly BuffAura _aura;
        readonly Mesh _groundMesh;
        readonly Material _groundMaterial;
        readonly Renderer[] _renderers;
        bool _disposed;

        public IronChainsAura(Transform parent,float height,Shader auraShader,Shader groundShader,Texture flow)
        {
            _root=new GameObject("Chain caster aura"); _root.transform.SetParent(parent,false);
            _root.transform.localPosition=Vector3.up*.055f;
            _root.transform.localScale=Vector3.one*height;
            _aura=new BuffAura(_root.transform,auraShader,flow,new Color(.012f,.32f,1.7f),.87f,.22f);
            _groundMaterial=new Material(groundShader) { name="Chain caster foot glow" };
            _groundMaterial.SetColor("_Color",new Color(.006f,.15f,1.5f));
            _groundMaterial.SetColor("_CoreColor",new Color(2.2f,2.7f,3,1));
            _groundMaterial.SetFloat("_TintStrength",.55f);
            _groundMesh=new Mesh { name="Chain caster ground plane" };
            _groundMesh.vertices=new[] { new Vector3(-.85f,0,-.85f),new Vector3(.85f,0,-.85f),
                new Vector3(-.85f,0,.85f),new Vector3(.85f,0,.85f) };
            _groundMesh.uv=new[] { Vector2.zero,Vector2.right,Vector2.up,Vector2.one };
            _groundMesh.triangles=new[] { 0,2,1,1,2,3 }; _groundMesh.RecalculateBounds();
            var ground=new GameObject("Chain caster foot glow",typeof(MeshFilter),typeof(MeshRenderer));
            ground.transform.SetParent(_root.transform,false);
            ground.GetComponent<MeshFilter>().sharedMesh=_groundMesh;
            var renderer=ground.GetComponent<MeshRenderer>(); renderer.sharedMaterial=_groundMaterial;
            renderer.shadowCastingMode=ShadowCastingMode.Off; renderer.receiveShadows=false;
            _renderers=_root.GetComponentsInChildren<Renderer>();
            foreach (var r in _renderers) r.gameObject.layer=parent.gameObject.layer;
        }

        public void Sample(float age)
        {
            if (_disposed) return;
            float opacity=Mathf.SmoothStep(0,1,Mathf.Clamp01(age/.09f))
                *(1-Mathf.SmoothStep(0,1,Mathf.InverseLerp(.58f,1.04f,age)));
            float yankPulse=Mathf.Exp(-Mathf.Pow((age-(IronChainsEffect.PullTime+.04f))/.105f,2));
            _aura.Sample(Mathf.Clamp01(age/.8f),opacity*(.4f+.35f*yankPulse));
            _groundMaterial.SetFloat("_Opacity",opacity*(.48f+.22f*yankPulse));
            foreach (var r in _renderers) r.enabled=opacity>0;
        }

        public void Hide()
        {
            if (_disposed) return;
            foreach (var r in _renderers) if (r!=null) r.enabled=false;
        }

        public void Dispose()
        {
            if (_disposed) return;
            Hide(); _disposed=true;
            _aura.Dispose(); Release(_groundMesh); Release(_groundMaterial); Release(_root);
        }

        static void Release(Object item)
        { if (Application.isPlaying) Object.Destroy(item); else Object.DestroyImmediate(item); }
    }
}
