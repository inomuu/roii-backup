using System;
using System.IO;
using Roi.Arena;
using UnityEditor;
using UnityEngine;

public static class HovlBuffProfiles
{
    const string Vendor="Assets/Hovl Studio/Magic effects pack/Prefabs/";
    const string Output="Assets/Resources/BuffEffects";

    [MenuItem("ROI/VFX/Create Hovl Buff Profiles")]
    public static void Create()
    {
        Directory.CreateDirectory(Output); AssetDatabase.Refresh();
        Profile("AegisReceive","Character auras/Buff.prefab",true,new Color(.18f,.75f,1),.65f);
        Profile("HealingReceive","Character auras/Healing.prefab",false,Color.white,.65f);
        Profile("ShieldReceive","Magic shields/Magic shield blue.prefab",false,Color.white,.55f);
        AssetDatabase.SaveAssets();
    }
    static void Profile(string name,string prefabPath,bool recolor,Color tint,float scale)
    {
        string path=Output+"/"+name+".asset";
        if (AssetDatabase.LoadAssetAtPath<BuffEffectProfile>(path)!=null) return;
        var prefab=AssetDatabase.LoadAssetAtPath<GameObject>(Vendor+prefabPath);
        if (prefab==null) throw new InvalidOperationException("Import Magic Effects FREE (247933) before creating buff profiles.");
        foreach (var r in prefab.GetComponentsInChildren<ParticleSystemRenderer>(true))
            foreach (var m in r.sharedMaterials)
                if (m==null || m.shader.name!="Universal Render Pipeline/Particles/Unlit")
                    throw new InvalidOperationException("Convert Hovl materials to URP first: "+prefabPath);
        var profile=ScriptableObject.CreateInstance<BuffEffectProfile>();
        profile.Prefab=prefab; profile.Recolor=recolor; profile.Tint=tint;
        profile.ScalePerHeight=scale; profile.Duration=.9f; profile.Warmup=.15f;
        profile.GroundGlow=name=="AegisReceive";
        profile.RisingAura=name=="AegisReceive";
        if (profile.RisingAura) profile.AuraTexture=Resources.Load<Texture2D>("Cutin/SeraphineAuraFlow-v1");
        AssetDatabase.CreateAsset(profile,path);
    }
}
