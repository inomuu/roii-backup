using System;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;
using Object=UnityEngine.Object;

namespace Roi.Arena
{
    // A manually sampled receive effect. It follows an anchor, but never moves it or owns sim time.
    public sealed class BuffEffect : IDisposable
    {
        readonly GameObject _root;
        readonly Transform _anchor;
        readonly BuffEffectProfile _profile;
        readonly ParticleSystem[] _particles;
        readonly List<(Renderer Renderer,int Index,Color Color)> _materials=new();
        readonly MaterialPropertyBlock _properties=new();
        readonly Material _groundMaterial;
        readonly Mesh _groundMesh;
        readonly BuffAura _aura;
        bool _disposed;
        public int ParticleCount
        {
            get { int count=0; foreach (var p in _particles) if (p!=null) count+=p.particleCount; return count; }
        }

        public BuffEffect(BuffEffectProfile profile,Transform parent,Transform anchor,float height)
        {
            if (profile==null || profile.Prefab==null || anchor==null) throw new ArgumentException("Buff effect needs a prefab and anchor.");
            var groundShader=profile.GroundGlow ? Resources.Load<Shader>("Effects/BuffGroundGlow") : null;
            if (profile.GroundGlow && (groundShader==null || !groundShader.isSupported))
                throw new InvalidOperationException("Buff ground glow shader missing or unsupported.");
            var auraShader=profile.RisingAura ? Resources.Load<Shader>("Effects/BuffAura") : null;
            if (profile.RisingAura && (auraShader==null || !auraShader.isSupported || profile.AuraTexture==null))
                throw new InvalidOperationException("Buff aura shader or texture missing or unsupported.");
            _profile=profile; _anchor=anchor;
            _root=Object.Instantiate(profile.Prefab,parent);
            _root.name=profile.name+" receive";
            _root.transform.localScale=Vector3.one*Mathf.Max(.01f,height*profile.ScalePerHeight);
            _particles=_root.GetComponentsInChildren<ParticleSystem>(true);
            for (int i=0;i<_particles.Length;i++)
            {
                var p=_particles[i]; p.Stop(false,ParticleSystemStopBehavior.StopEmittingAndClear);
                var main=p.main;
                main.playOnAwake=false; main.loop=false; main.scalingMode=ParticleSystemScalingMode.Hierarchy;
                main.stopAction=ParticleSystemStopAction.None;
                p.useAutoRandomSeed=false; p.randomSeed=(uint)(741+i*97);
                if (profile.Recolor)
                {
                    main.startColor=Map(main.startColor,profile.Tint);
                    var lifetime=p.colorOverLifetime;
                    if (lifetime.enabled) lifetime.color=Map(lifetime.color,Color.white);
                    var speed=p.colorBySpeed;
                    if (speed.enabled) speed.color=Map(speed.color,Color.white);
                }
            }
            foreach (var renderer in _root.GetComponentsInChildren<ParticleSystemRenderer>(true))
            {
                var materials=renderer.sharedMaterials;
                for (int i=0;i<materials.Length;i++)
                {
                    var m=materials[i];
                    if (m==null || !m.HasProperty("_BaseColor")) continue;
                    _materials.Add((renderer,i,m.GetColor("_BaseColor")));
                }
            }
            if (groundShader!=null)
            {
                _groundMaterial=new Material(groundShader); _groundMaterial.SetColor("_Color",profile.GroundColor);
                _groundMesh=new Mesh { name="Buff ground quad" };
                _groundMesh.vertices=new[] { new Vector3(-.5f,-.5f),new Vector3(.5f,-.5f),new Vector3(.5f,.5f),new Vector3(-.5f,.5f) };
                _groundMesh.uv=new[] { Vector2.zero,Vector2.right,Vector2.one,Vector2.up };
                _groundMesh.triangles=new[] { 0,1,2,0,2,3 }; _groundMesh.RecalculateBounds();
                var ground=new GameObject("Buff ground glow",typeof(MeshFilter),typeof(MeshRenderer));
                ground.transform.SetParent(_root.transform,false);
                ground.transform.localRotation=Quaternion.Euler(90,0,0);
                ground.transform.localScale=Vector3.one*(profile.GroundDiameter/Mathf.Max(.01f,profile.ScalePerHeight));
                ground.GetComponent<MeshFilter>().sharedMesh=_groundMesh;
                var renderer=ground.GetComponent<MeshRenderer>(); renderer.sharedMaterial=_groundMaterial;
                renderer.shadowCastingMode=ShadowCastingMode.Off; renderer.receiveShadows=false;
            }
            if (auraShader!=null)
                _aura=new BuffAura(_root.transform,auraShader,profile.AuraTexture,profile.AuraColor,
                    1/Mathf.Max(.01f,profile.ScalePerHeight));
            Sample(-1);
        }

        public void Sample(float age)
        {
            if (_root==null) return;
            bool visible=_anchor!=null && age>=0 && age<_profile.Duration;
            _root.SetActive(visible);
            if (!visible) return;
            _root.transform.position=_anchor.position+Vector3.up*.055f;
            float duration=Mathf.Max(.1f,_profile.Duration);
            float alpha=Mathf.SmoothStep(0,1,Mathf.Clamp01(age/.10f))
                *(1-Mathf.SmoothStep(0,1,Mathf.InverseLerp(duration*.65f,duration,age)));
            if (_groundMaterial!=null) _groundMaterial.SetFloat("_Opacity",alpha);
            _aura?.Sample(age/duration,alpha);
            foreach (var p in _particles)
                p.Simulate(Mathf.Max(0,age*_profile.PlaybackSpeed+_profile.Warmup),false,true,true);
            foreach (var binding in _materials)
            {
                var color=binding.Color; color.a*=alpha;
                _properties.Clear(); _properties.SetColor("_BaseColor",color);
                binding.Renderer.SetPropertyBlock(_properties,binding.Index);
            }
        }

        static ParticleSystem.MinMaxGradient Map(ParticleSystem.MinMaxGradient source,Color tint)
        {
            Color Recolor(Color c)
            {
                float value=Mathf.Max(c.r,Mathf.Max(c.g,c.b));
                return new Color(tint.r*value,tint.g*value,tint.b*value,c.a*tint.a);
            }
            Gradient Remap(Gradient sourceGradient)
            {
                var keys=sourceGradient.colorKeys;
                for (int i=0;i<keys.Length;i++) keys[i].color=Recolor(keys[i].color);
                var result=new Gradient { mode=sourceGradient.mode };
                result.SetKeys(keys,sourceGradient.alphaKeys); return result;
            }
            switch (source.mode)
            {
                case ParticleSystemGradientMode.Color: return new ParticleSystem.MinMaxGradient(Recolor(source.color));
                case ParticleSystemGradientMode.TwoColors: return new ParticleSystem.MinMaxGradient(Recolor(source.colorMin),Recolor(source.colorMax));
                case ParticleSystemGradientMode.TwoGradients: return new ParticleSystem.MinMaxGradient(Remap(source.gradientMin),Remap(source.gradientMax));
                default:
                    var mapped=new ParticleSystem.MinMaxGradient(Remap(source.gradient)); mapped.mode=source.mode; return mapped;
            }
        }
        public void Dispose()
        {
            if (_disposed) return;
            _disposed=true;
            _aura?.Dispose();
            if (_root!=null) { _root.SetActive(false); Destroy(_root); }
            if (_groundMaterial!=null) Destroy(_groundMaterial);
            if (_groundMesh!=null) Destroy(_groundMesh);
        }
        static void Destroy(Object value)
        { if (Application.isPlaying) Object.Destroy(value); else Object.DestroyImmediate(value); }
    }
}
