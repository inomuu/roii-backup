using UnityEditor;
using UnityEditor.Build.Reporting;
using UnityEngine;

public static class BuildFloatingForge
{
    public const string Output="/private/tmp/roi_floating_forge/ROI.app";
    [MenuItem("ROI/UI/Build Floating Forge Standalone Probe")]
    public static void Run()
    {
        if(EditorApplication.isPlaying){Debug.LogError("FORGE BUILD: exit Play mode first.");return;}
        EnsureSparkMaterials();
        var report=BuildPipeline.BuildPlayer(new BuildPlayerOptions{
            scenes=new[]{ProbeScene.Path},locationPathName=Output,target=BuildTarget.StandaloneOSX,options=BuildOptions.Development
        });
        Debug.Log($"FORGE BUILD: {report.summary.result}, {report.summary.totalErrors} errors, {report.summary.totalTime.TotalSeconds:0}s -> {Output}");
        if(report.summary.result!=BuildResult.Succeeded)Debug.LogError("FORGE BUILD FAILED");
    }
    // These resources also fix normal standalone builds; they contain no probe-only visuals.
    public static void EnsureSparkMaterials()
    {
        const string folder="Assets/Resources/Effects";
        if(!AssetDatabase.IsValidFolder(folder)) AssetDatabase.CreateFolder("Assets/Resources","Effects");
        MakeSpark(folder+"/RuntimeSparkAdditive.mat","Universal Render Pipeline/Unlit",UnityEngine.Rendering.BlendMode.One);
        MakeSpark(folder+"/RuntimeSparkSoft.mat","Universal Render Pipeline/Particles/Unlit",UnityEngine.Rendering.BlendMode.SrcAlpha);
        AssetDatabase.SaveAssets();
    }
    static void MakeSpark(string path,string shaderName,UnityEngine.Rendering.BlendMode source)
    {
        if(AssetDatabase.LoadAssetAtPath<Material>(path)!=null)return;
        var shader=Shader.Find(shaderName);
        if(shader==null)throw new System.InvalidOperationException("Missing required runtime shader: "+shaderName);
        var material=new Material(shader);
        material.SetFloat("_Surface",1);material.SetFloat("_Blend",1);
        material.SetInt("_SrcBlend",(int)source);material.SetInt("_DstBlend",(int)UnityEngine.Rendering.BlendMode.One);
        material.SetInt("_ZWrite",0);material.renderQueue=3000;material.EnableKeyword("_SURFACE_TYPE_TRANSPARENT");
        AssetDatabase.CreateAsset(material,path);
    }
}
