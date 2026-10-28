using System;
using System.Collections.Generic;
using UnityEditor;
using UnityEngine;
using Object=UnityEngine.Object;
using static Roi.Editor.Vfx.SuzuSunderFixture;

// Editor-only adaptation: original vendor geometry, textures and shader graphs stay untouched.
internal sealed class MaykerFireOrbitStudy : IDisposable
{
    const float SweepDuration=.78f;
    readonly Transform _root;
    readonly float _startYaw;
    readonly List<Material> _materials=new();
    ParticleSystemRenderer _highlight;

    public MaykerFireOrbitStudy(GameObject root,Vector3 actor,float height,bool whiteHighlight=false)
    {
        _root=root.transform;
        _startYaw=_root.eulerAngles.y;
        _root.position=actor+Vector3.up*height*.43f;
        _root.localScale*=1.13f;
        var coverage=whiteHighlight ? AssetDatabase.LoadAssetAtPath<Shader>(
            "Assets/Editor/Vfx/Suzu/Shaders/FireOrbitCoverage.shadergraph") : null;
        Require(!whiteHighlight || coverage!=null,"fire UV coverage shader missing");
        var copies=new Dictionary<Material,Material>();
        foreach (var renderer in root.GetComponentsInChildren<ParticleSystemRenderer>(true))
        {
            var materials=renderer.sharedMaterials;
            for (int i=0;i<materials.Length;i++)
            {
                var source=materials[i];
                if (!copies.TryGetValue(source,out var copy))
                {
                    copy=new Material(source) { name=source.name+" (solid orbit study)", hideFlags=HideFlags.HideAndDontSave };
                    _materials.Add(copy); copies.Add(source,copy);
                    Tune(copy,source.name);
                    if (whiteHighlight && source.name=="Slash")
                    {
                        copy.shader=coverage;
                        copy.SetFloat("_Taper",1); copy.SetFloat("_WhiteBand",0);
                    }
                }
                materials[i]=copy;
            }
            renderer.sharedMaterials=materials;
        }
        foreach (var ps in root.GetComponentsInChildren<ParticleSystem>(true))
        {
            var main=ps.main;
            // A local carrier sweeps around the caster; cast age still drives erosion.
            main.simulationSpace=ParticleSystemSimulationSpace.Local;
            if (ps.gameObject==root || ps.name=="Slash Distortion") main.startColor=Color.white;
            if (ps.name=="Flying") main.startSizeMultiplier*=.6f;
        }
        // The source's large single-contact star should not orbit through the caster.
        var impact=_root.Find("Impact");
        if (impact!=null) impact.gameObject.SetActive(false);
        foreach (var light in root.GetComponentsInChildren<Light>(true))
        {
            light.color=new Color(1,.22f,.035f);
            light.intensity*=.12f;
        }
        // A slightly larger copy of the same textured carrier is drawn under the
        // orange body. Only its narrow exposed edge glows; the body stays solid.
        var rim=Object.Instantiate(root);
        rim.name="Fire rim";
        while (rim.transform.childCount>0) Object.DestroyImmediate(rim.transform.GetChild(0).gameObject);
        rim.transform.SetParent(_root,false);
        rim.transform.localPosition=Vector3.zero;
        rim.transform.localRotation=Quaternion.identity;
        rim.transform.localScale=Vector3.one*1.025f;
        var rimRenderer=rim.GetComponent<ParticleSystemRenderer>();
        var rimMaterial=new Material(rimRenderer.sharedMaterial) {
            name="Fire orbit golden edge",hideFlags=HideFlags.HideAndDontSave,renderQueue=2999
        };
        rimMaterial.SetColor("_Color",new Color(1.3f,.65f,.14f,1));
        rimRenderer.sharedMaterial=rimMaterial;
        _materials.Add(rimMaterial);
        if (whiteHighlight)
        {
            var white=Object.Instantiate(rim,_root);
            white.name="Broad white fire highlight";
            white.transform.localScale=Vector3.one;
            _highlight=white.GetComponent<ParticleSystemRenderer>();
            Require(_highlight.mesh==root.GetComponent<ParticleSystemRenderer>().mesh,
                "white band must use the unchanged source mesh");
            var material=new Material(rimMaterial) {
                name="Orbit hot white band",hideFlags=HideFlags.HideAndDontSave,renderQueue=3004
            };
            material.SetColor("_Color",new Color(2.4f,2.25f,2.05f,1));
            material.SetFloat("_Opacity",12);
            material.SetFloat("_WhiteBand",1);
            _highlight.sharedMaterial=material; _materials.Add(material);
        }
    }

    static void Tune(Material material,string source)
    {
        Set("_Distortion",0); Set("_DistortionEnabled",0);
        switch (source)
        {
            case "Slash":
                material.SetColor("_Color",new Color(.95f,.11f,.008f,1));
                Set("_Opacity",16); Set("_Emission",0);
                break;
            case "Slash World Fire":
                material.SetColor("_Color",new Color(1,.12f,.012f,1));
                Set("_ColorStrength",1.5f); Set("_ColorPower",1.8f);
                Set("_Emission",0); Set("_Opacity",40);
                break;
            case "Flying Particles Fire":
                material.SetColor("_FrontColor",new Color(1.4f,.26f,.022f,1));
                material.SetColor("_BackColor",new Color(.48f,.025f,.006f,1));
                break;
            case "Flying var Vertex":
                Set("_Opacity",.4f);
                break;
        }
        void Set(string key,float value) { if (material.HasProperty(key)) material.SetFloat(key,value); }
    }

    public float Sample(float age)
    {
        _root.rotation=Quaternion.Euler(0,_startYaw-360*Mathf.Clamp01(age/SweepDuration),0);
        // Preserve a broad textured blade through the revolution, then run the vendor dissolve out.
        if (age<.1f) return Mathf.Max(0,age)*.7f;
        if (age<SweepDuration) return Mathf.Lerp(.07f,.20f,(age-.1f)/(SweepDuration-.1f));
        return .20f+(age-SweepDuration)*1.8f;
    }

    public void SetHighlightVisible(bool visible) { if (_highlight!=null) _highlight.enabled=visible; }

    public void VerifyOrbit()
    {
        Measure(false);
        bool caught=false;
        try { Measure(true); } catch (InvalidOperationException) { caught=true; }
        Require(caught,"frozen orbit survived the rotation check");
        Sample(0);

        void Measure(bool frozen)
        {
            var center=_root.position;
            Sample(0); float previous=_root.eulerAngles.y,total=0;
            for (int i=1;i<=8;i++)
            {
                Sample(frozen ? 0 : SweepDuration*i/8);
                float yaw=_root.eulerAngles.y;
                total+=Mathf.DeltaAngle(previous,yaw); previous=yaw;
                Require(Vector3.Distance(center,_root.position)<.00001f,"orbit moved off its caster pivot");
            }
            Require(Mathf.Abs(total+360)<.01f,"orbit did not travel one full revolution");
            Debug.Log("MAYKER ORBIT VERIFIED: yaw travel="+total+" degrees, fixed caster pivot");
        }
    }

    public void Dispose()
    {
        foreach (var material in _materials) Object.DestroyImmediate(material);
    }
}
