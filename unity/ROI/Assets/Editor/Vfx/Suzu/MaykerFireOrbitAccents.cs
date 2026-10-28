using System;
using Roi.Arena;
using Roi.Editor.Vfx;
using UnityEngine;
using UnityEngine.Rendering;
using Object=UnityEngine.Object;
using static Roi.Editor.Vfx.SuzuSunderFixture;

// Sampled editor presentation only; contact times are staged, not gameplay hit detection.
internal sealed class MaykerFireOrbitAccents : IDisposable
{
    [Flags] public enum Layers { None=0, Light=1, Hits=2, Embers=4, Aura=8, All=15 }
    readonly SuzuSunderFixture _fixture;
    readonly GameObject _root;
    readonly Mesh _groundMesh;
    readonly MeshRenderer _ground;
    readonly Material _groundMaterial,_emberMaterial;
    readonly Light _light;
    readonly BuffAura _aura;
    readonly ParticleSystem[] _embers=new ParticleSystem[9];
    readonly ParticleSystemRenderer[] _renderers=new ParticleSystemRenderer[9];

    public MaykerFireOrbitAccents(SuzuSunderFixture fixture,float startYaw)
    {
        _fixture=fixture;
        float h=fixture.Height;
        var glow=Resources.Load<Shader>("Effects/BuffGroundGlow");
        var spark=Resources.Load<Shader>("Effects/CircularSlashGlow");
        Require(glow!=null && spark!=null,"orbit accent shaders missing");
        _root=new GameObject("Fire orbit accents");
        _root.transform.SetParent(fixture.Actor,false);
        var auraShader=Resources.Load<Shader>("Effects/BuffAura");
        var auraTexture=Resources.Load<Texture2D>("Cutin/SeraphineAuraFlow-v1");
        Require(auraShader!=null && auraTexture!=null,"cast aura assets missing");
        _aura=new BuffAura(_root.transform,auraShader,auraTexture,new Color(1.35f,.24f,.025f),h*1.12f,.18f);
        _groundMesh=new Mesh { name="Fire orbit ground pool" };
        float radius=h*1.85f;
        _groundMesh.vertices=new[] { new Vector3(-radius,0,-radius),new Vector3(radius,0,-radius),
            new Vector3(-radius,0,radius),new Vector3(radius,0,radius) };
        _groundMesh.uv=new[] { Vector2.zero,Vector2.right,Vector2.up,Vector2.one };
        _groundMesh.triangles=new[] { 0,2,1,1,2,3 }; _groundMesh.RecalculateBounds();
        _groundMaterial=new Material(glow) { name="Orbit amber foot light",renderQueue=2985 };
        _groundMaterial.SetColor("_Color",new Color(1.3f,.14f,.008f));
        _groundMaterial.SetColor("_CoreColor",new Color(1.8f,.95f,.24f));
        _groundMaterial.SetFloat("_TintStrength",.45f);
        var plane=new GameObject("Ground glow",typeof(MeshFilter),typeof(MeshRenderer));
        plane.transform.SetParent(_root.transform,false); plane.transform.localPosition=Vector3.up*.035f;
        plane.GetComponent<MeshFilter>().sharedMesh=_groundMesh;
        _ground=plane.GetComponent<MeshRenderer>(); _ground.sharedMaterial=_groundMaterial;
        _ground.shadowCastingMode=ShadowCastingMode.Off; _ground.receiveShadows=false;
        var lamp=new GameObject("Warm cast light"); lamp.transform.SetParent(_root.transform,false);
        lamp.transform.localPosition=Vector3.up*h*.8f;
        _light=lamp.AddComponent<Light>(); _light.type=LightType.Point;
        _light.color=new Color(1,.31f,.065f); _light.range=h*4;
        _light.shadows=LightShadows.None;
        _emberMaterial=new Material(spark) { name="Orbit golden embers",renderQueue=3105 };
        _emberMaterial.SetFloat("_Spark",1); _emberMaterial.SetFloat("_EmissionGain",5.2f);
        for (int i=0;i<_embers.Length;i++)
        {
            float angle=(startYaw-70-i*40)*Mathf.Deg2Rad;
            var radial=new Vector3(Mathf.Sin(angle),0,Mathf.Cos(angle));
            var tangent=Vector3.Cross(Vector3.up,radial);
            var emitter=new GameObject("Rim ember fan "+i);
            emitter.transform.SetParent(_root.transform,false);
            emitter.transform.localPosition=(radial*1.68f+Vector3.up*.43f)*h;
            emitter.transform.localRotation=Quaternion.LookRotation(radial*.8f-tangent*.35f+Vector3.up*.5f);
            var ps=emitter.AddComponent<ParticleSystem>(); _embers[i]=ps;
            ps.Stop(true,ParticleSystemStopBehavior.StopEmittingAndClear);
            ps.useAutoRandomSeed=false; ps.randomSeed=(uint)(9103+i*31);
            var main=ps.main; main.loop=false; main.playOnAwake=false; main.duration=.7f;
            main.startLifetime=new ParticleSystem.MinMaxCurve(.36f,.64f);
            main.startSpeed=new ParticleSystem.MinMaxCurve(h*.9f,h*2.1f);
            main.startSize=new ParticleSystem.MinMaxCurve(h*.09f,h*.14f);
            main.startColor=new Color(1,.78f,.3f); main.maxParticles=8;
            main.simulationSpace=ParticleSystemSimulationSpace.Local;
            var shape=ps.shape; shape.shapeType=ParticleSystemShapeType.Cone;
            shape.angle=24; shape.radius=h*.04f;
            var emission=ps.emission; emission.rateOverTime=0;
            emission.SetBursts(new[] { new ParticleSystem.Burst(0,(short)8) });
            var color=ps.colorOverLifetime; color.enabled=true;
            var gradient=new Gradient();
            gradient.SetKeys(new[] { new GradientColorKey(Color.white,0),
                new GradientColorKey(new Color(1,.22f,.015f),1) },
                new[] { new GradientAlphaKey(1,0),new GradientAlphaKey(1,.2f),new GradientAlphaKey(0,1) });
            color.color=gradient;
            var size=ps.sizeOverLifetime; size.enabled=true;
            size.size=new ParticleSystem.MinMaxCurve(1,AnimationCurve.EaseInOut(0,1,1,.1f));
            var renderer=emitter.GetComponent<ParticleSystemRenderer>(); _renderers[i]=renderer;
            renderer.sharedMaterial=_emberMaterial; renderer.renderMode=ParticleSystemRenderMode.Stretch;
            renderer.lengthScale=1.7f; renderer.velocityScale=.04f; renderer.cameraVelocityScale=0;
            renderer.shadowCastingMode=ShadowCastingMode.Off; renderer.receiveShadows=false;
        }
        Sample(-1,false);
    }

    public void Sample(float age,bool visible=true,Layers layers=Layers.All)
    {
        bool active=visible && age>=0 && age<1.5f;
        float auraEnvelope=Mathf.SmoothStep(0,1,Mathf.InverseLerp(-.3f,-.12f,age))
            *(1-Mathf.SmoothStep(0,1,Mathf.InverseLerp(.55f,1.05f,age)));
        _aura.Sample(Mathf.InverseLerp(-.3f,.85f,age),
            visible && (layers&Layers.Aura)!=0 ? auraEnvelope*.68f : 0);
        float envelope=Mathf.SmoothStep(0,1,Mathf.InverseLerp(0,.075f,age))
            *(1-Mathf.SmoothStep(0,1,Mathf.InverseLerp(.72f,1.2f,age)));
        bool light=active && (layers&Layers.Light)!=0;
        _ground.enabled=light && envelope>0;
        _groundMaterial.SetFloat("_Opacity",envelope*.32f);
        _light.enabled=light && envelope>0; _light.intensity=envelope*1.6f;
        for (int i=0;i<_fixture.Impacts.Length;i++)
            _fixture.Impacts[i].Sample(age-(.09f+i*.10f),_fixture.TargetContact(i),
                _fixture.Camera.transform.right+_fixture.Camera.transform.up*(i==0 ? .4f : -.35f),
                _fixture.Camera,active && (layers&Layers.Hits)!=0);
        for (int i=0;i<_embers.Length;i++)
        {
            float emberAge=age-(.035f+i*.078f);
            bool show=active && (layers&Layers.Embers)!=0 && emberAge>=0 && emberAge<.7f;
            _renderers[i].enabled=show;
            if (show) _embers[i].Simulate(emberAge,false,true,true);
            else _embers[i].Stop(false,ParticleSystemStopBehavior.StopEmittingAndClear);
        }
    }

    public void Dispose()
    {
        foreach (var hit in _fixture.Impacts) hit.Hide();
        _aura.Dispose();
        Object.DestroyImmediate(_root); Object.DestroyImmediate(_groundMesh);
        Object.DestroyImmediate(_groundMaterial); Object.DestroyImmediate(_emberMaterial);
    }
}
