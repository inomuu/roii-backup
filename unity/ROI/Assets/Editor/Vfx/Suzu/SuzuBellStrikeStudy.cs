using System;
using Roi.Arena;
using Roi.Editor.Vfx;
using UnityEditor;
using UnityEngine;
using Object=UnityEngine.Object;
using static Roi.Editor.Vfx.SuzuSunderFixture;

// Single-target presentation study using the previously imported Hovl explosion.
internal sealed class SuzuBellStrikeStudy : IDisposable
{
    const float Contact=.16f;
    readonly SuzuSunderFixture _f;
    readonly Transform _blade;
    readonly GameObject _explosion;
    readonly ParticleSystem _particles;
    readonly Vector3 _contact;
    readonly Mesh _mesh;
    readonly BuffAura _aura;
    readonly bool _quarterArc;
    readonly float _arcRadians;
    readonly Material[] _flow=new Material[3];

    public SuzuBellStrikeStudy(GameObject blade,SuzuSunderFixture fixture,bool quarterArc=false,bool halfArc=false)
    {
        _f=fixture; _blade=blade.transform; _contact=fixture.TargetContact(0); _quarterArc=quarterArc || halfArc;
        _arcRadians=halfArc ? Mathf.PI : Mathf.PI*.5f;
        fixture.SetTargetVisible(1,false);
        _mesh=SuzuBellMeshPreview.BuildCarrier(_quarterArc,halfArc ? 180 : 90);
        var shader=AssetDatabase.LoadAssetAtPath<Shader>("Assets/Editor/Vfx/Suzu/Shaders/FireOrbitCoverage.shadergraph");
        Require(shader!=null && shader.isSupported && !ShaderUtil.ShaderHasError(shader),"orbit fire shader missing or invalid");
        _blade.localScale=Vector3.one*fixture.Height;
        foreach (string child in new[] { "Smoke","Flying","Impact","Slash Distortion" })
        {
            var t=_blade.Find(child); if (t!=null) t.gameObject.SetActive(false);
        }
        int layer=0;
        foreach (var carrier in new[] { _blade,_blade.Find("Fire rim"),_blade.Find("Broad white fire highlight") })
        {
            Require(carrier!=null,"Bell fire carrier layer missing");
            var ps=carrier.GetComponent<ParticleSystem>();
            ps.Stop(false,ParticleSystemStopBehavior.StopEmittingAndClear);
            var main=ps.main;
            main.startSize3D=false; main.startSize=1; main.startSpeed=0;
            main.startRotation3D=true; main.startRotationX=0; main.startRotationY=0; main.startRotationZ=0;
            main.simulationSpace=ParticleSystemSimulationSpace.Local;
            var shape=ps.shape; shape.enabled=false;
            var renderer=carrier.GetComponent<ParticleSystemRenderer>();
            renderer.mesh=_mesh;
            renderer.alignment=ParticleSystemRenderSpace.Local;
            var material=renderer.sharedMaterial;
            Require(material.shader==shader,"Bell must retain the orbit material pipeline");
            foreach (string texture in new[] { "_Main", "_MaskTex", "_DissolveTex" })
                Require(material.GetTexture(texture)!=null,"vendor fire texture missing: "+texture);
            material.SetFloat("_BellStrip",1);
            _flow[layer++]=material;
        }
        var auraShader=Resources.Load<Shader>("Effects/BuffAura");
        var auraTexture=Resources.Load<Texture2D>("Cutin/SeraphineAuraFlow-v1");
        Require(auraShader!=null && auraTexture!=null,"Bell caster aura resources missing");
        _aura=new BuffAura(fixture.Actor,auraShader,auraTexture,new Color(1.35f,.24f,.025f),fixture.Height*1.12f,.18f,
            staticCarrier:true);
        var prefab=AssetDatabase.LoadAssetAtPath<GameObject>(
            "Assets/Hovl Studio/Magic effects pack/Prefabs/Hits and explosions/Explosion.prefab");
        Require(prefab!=null,"previously imported Hovl Explosion prefab missing");
        _explosion=Object.Instantiate(prefab,fixture.Actor);
        _explosion.name="Bell Strike Hovl explosion";
        _explosion.transform.position=_contact;
        _explosion.transform.rotation=Quaternion.identity;
        _explosion.transform.localScale=Vector3.one*fixture.Height*.22f;
        _particles=_explosion.GetComponent<ParticleSystem>();
        Require(_particles!=null,"Hovl Explosion root particle system missing");
        var systems=_explosion.GetComponentsInChildren<ParticleSystem>(true);
        for (int i=0;i<systems.Length;i++)
        {
            var ps=systems[i]; ps.Stop(false,ParticleSystemStopBehavior.StopEmittingAndClear);
            ps.useAutoRandomSeed=false; ps.randomSeed=(uint)(14327+i*19);
            var main=ps.main; main.loop=false; main.playOnAwake=false;
            main.simulationSpace=ParticleSystemSimulationSpace.Local;
            main.scalingMode=ParticleSystemScalingMode.Hierarchy;
        }
        foreach (var renderer in _explosion.GetComponentsInChildren<ParticleSystemRenderer>(true))
            foreach (var material in renderer.sharedMaterials)
                Require(material!=null && material.shader.isSupported && !ShaderUtil.ShaderHasError(material.shader)
                    && material.shader.name=="Universal Render Pipeline/Particles/Unlit",
                    "Hovl explosion needs its existing URP material conversion");
        Debug.Log("BELL EXPLOSION SOURCE: existing Hovl prefab, "+systems.Length+" particle systems; original materials");
        PlaceAtSpawn(); Sample(-1,false);
    }

    public void PlaceAtSpawn()
    {
        var camera=_f.Camera.transform;
        var feet=_f.TargetHead(0)-camera.up*_f.Height*.85f;
        _blade.position=feet+camera.up*_f.Height*.015f
            -(feet-camera.position).normalized*_f.Height*SuzuBellMeshPreview.CameraInset;
        _blade.rotation=camera.rotation;
    }

    public float Sample(float age,bool visible=true,bool explosion=true)
    {
        float head=1-Mathf.Pow(Mathf.Clamp01(age/.2f),1.45f);
        float tail=1-Mathf.Pow(Mathf.Clamp01((age-.18f)/.44f),1.3f);
        float fade=Mathf.SmoothStep(0,1,Mathf.InverseLerp(0,.025f,age))
            *(1-Mathf.SmoothStep(0,1,Mathf.InverseLerp(.4f,.68f,age)));
        foreach (var material in _flow)
        {
            material.SetFloat("_Head",head); material.SetFloat("_Tail",tail);
            material.SetFloat("_Fade",visible ? fade : 0);
        }
        float aura=Mathf.SmoothStep(0,1,Mathf.InverseLerp(-.28f,-.08f,age))
            *(1-Mathf.SmoothStep(0,1,Mathf.InverseLerp(.2f,.7f,age)));
        _aura.Sample(Mathf.InverseLerp(-.28f,.7f,age),visible ? aura*.55f : 0);
        float burstAge=age-Contact;
        bool active=visible && explosion && burstAge>=0 && burstAge<1.4f;
        _explosion.SetActive(active);
        if (active) _particles.Simulate(burstAge,true,true,true);
        else _particles.Stop(true,ParticleSystemStopBehavior.StopEmittingAndClear);
        _f.Impacts[0].Sample(burstAge,_contact,-_f.Camera.transform.up,_f.Camera,active);
        _f.Impacts[1].Hide();
        // Keep the source particle custom-data/dissolve lifecycle, sampled in
        // its broad-blade phase during travel, then let the native erosion run out.
        if (age<.1f) return Mathf.Max(0,age)*.7f;
        if (age<.52f) return Mathf.Lerp(.07f,.17f,(age-.1f)/.42f);
        return .17f+(age-.52f)*1.8f;
    }

    public float CarrierCoordinate(Vector2 pixel)
    {
        var ray=_f.Camera.ScreenPointToRay(pixel);
        var plane=new Plane(_blade.forward,_blade.position);
        Require(plane.Raycast(ray,out float distance),"Bell carrier is edge-on to capture ray");
        var local=_blade.InverseTransformPoint(ray.GetPoint(distance));
        if (!_quarterArc) return (local.y+.2f)/2.2f;
        float angle=Mathf.Atan2(local.y+.2f,local.x+2.2f-1.35f*.5f);
        // Bloom can extend just past the upper angle boundary at the half-arc tail.
        if (angle< -Mathf.PI*.5f) angle+=Mathf.PI*2;
        return angle/_arcRadians;
    }

    public void VerifyFlow()
    {
        Measure(false);
        bool caught=false;
        try { Measure(true); } catch (InvalidOperationException) { caught=true; }
        Require(caught,"frozen Bell UV flow survived the check");
        var position=_blade.position;
        caught=false;
        try { Measure(false,true); } catch (InvalidOperationException) { caught=true; }
        finally { _blade.position=position; }
        Require(caught,"moving Bell carrier survived the fixed-transform check");
        VerifyCarrier();
        Sample(-1,false);
        void Measure(bool frozen,bool moveCarrier=false)
        {
            Sample(.03f); var start=_blade.localToWorldMatrix;
            float firstHead=_flow[0].GetFloat("_Head");
            float firstLength=_flow[0].GetFloat("_Tail")-firstHead;
            Sample(frozen ? .03f : .2f);
            if (moveCarrier) _blade.position+=Vector3.up*.1f;
            Require(_blade.localToWorldMatrix==start,"Bell carrier moved instead of flowing UVs");
            float hitHead=_flow[0].GetFloat("_Head");
            float hitLength=_flow[0].GetFloat("_Tail")-hitHead;
            Require(firstHead-hitHead>.8f,"Bell UV head did not travel down the fixed mesh");
            Require(hitLength>firstLength*5,"Bell trail did not grow during the strike");
            Sample(.55f);
            Require(_blade.localToWorldMatrix==start,"Bell carrier moved during recovery");
            float lateLength=_flow[0].GetFloat("_Tail")-_flow[0].GetFloat("_Head");
            Require(lateLength<hitLength*.4f && lateLength>0,"Bell trail did not contract after impact");
            Sample(.68f);
            Require(_flow[0].GetFloat("_Fade")==0,"Bell UV flow did not expire");
            Debug.Log("BELL UV FLOW VERIFIED: fixed carrier; trail lengths="+firstLength+", "+hitLength+", "+lateLength);
        }
    }

    void VerifyCarrier()
    {
        Sample(.12f,false);
        var ps=_blade.GetComponent<ParticleSystem>();
        ps.Simulate(.12f,true,true,true);
        var particles=new ParticleSystem.Particle[1];
        Require(ps.GetParticles(particles)==1,"Bell carrier particle missing");
        Require(ps.GetComponent<ParticleSystemRenderer>().mesh==_mesh,"Bell carrier lost its approved mesh");
        Require(Vector3.Distance(particles[0].GetCurrentSize3D(ps),Vector3.one)<.001f,
            "Bell particle rescaled the approved mesh");
        Require(particles[0].position.sqrMagnitude<.000001f,"Bell particle moved off the approved anchor");
        var rotation=Quaternion.Euler(particles[0].rotation3D);
        Measure();
        var correct=_blade.rotation;
        bool caught=false;
        try
        {
            _blade.rotation=_f.Camera.transform.rotation*Quaternion.Euler(0,68,0)*Quaternion.Euler(-90,0,0);
            Measure();
        }
        catch (InvalidOperationException) { caught=true; }
        finally { _blade.rotation=correct; }
        Require(caught,"edge-on Bell carrier survived the projection check");

        void Measure()
        {
            float shortest=1;
            foreach (var axis in new[] { Vector3.right,Vector3.up })
            {
                var world=_blade.TransformVector(rotation*axis);
                float ratio=Vector3.ProjectOnPlane(world,_f.Camera.transform.forward).magnitude/world.magnitude;
                shortest=Mathf.Min(shortest,ratio);
            }
            Require(shortest>.98f,"Bell carrier is compressed by its viewing angle");
            Debug.Log("BELL CARRIER VERIFIED: minimum projected axis ratio="+shortest);
        }
    }

    public void Dispose()
    {
        _f.Impacts[0].Hide(); _aura.Dispose();
        Object.DestroyImmediate(_explosion); Object.DestroyImmediate(_mesh);
    }
}
