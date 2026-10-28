using System;
using System.Collections.Generic;
using System.IO;
using TMPro;
using UnityEditor;
using UnityEngine;
using UnityEngine.TextCore.LowLevel;

// Author once in the editor. All glyphs in the supplied fonts are baked before shipping.
public static class EclipseTypographySetup
{
    const string Root="Assets/Resources/Fonts/";
    public static void Bake()
    {
        if(EditorApplication.isPlaying)throw new InvalidOperationException("Stop Play mode before baking fonts.");
        var body=BakeFont("BaiJamjuree-Medium","EclipseUI SDF",48);
        var thaiTitle=BakeFont("BaiJamjuree-SemiBold","EclipseTitleThai SDF",64);
        var display=BakeFont("Marcellus-Regular","EclipseDisplay SDF",64);
        var legacy=AssetDatabase.LoadAssetAtPath<TMP_FontAsset>(Root+"NotoSans UI SDF.asset");
        body.fallbackFontAssetTable=new List<TMP_FontAsset>{legacy};
        thaiTitle.fallbackFontAssetTable=new List<TMP_FontAsset>{legacy};
        display.fallbackFontAssetTable=new List<TMP_FontAsset>{thaiTitle,legacy};
        foreach(var font in new[]{body,thaiTitle,display})EditorUtility.SetDirty(font);
        AssetDatabase.SaveAssets();
        Debug.Log("ECLIPSE TYPE: baked Marcellus + Bai Jamjuree; original international fallback retained.");
    }
    static TMP_FontAsset BakeFont(string file,string name,int sample)
    {
        string path=Root+name+".asset";
        var existing=AssetDatabase.LoadAssetAtPath<TMP_FontAsset>(path);
        if(existing!=null)return existing;
        string sourcePath=Root+"Eclipse/"+file+".ttf";
        AssetDatabase.ImportAsset(sourcePath,ImportAssetOptions.ForceSynchronousImport);
        var source=AssetDatabase.LoadAssetAtPath<Font>(sourcePath);
        if(source==null)throw new InvalidOperationException("Missing type source: "+sourcePath);
        var font=TMP_FontAsset.CreateFontAsset(source,sample,8,GlyphRenderMode.SDFAA_HINTED,2048,2048,AtlasPopulationMode.Dynamic,false);
        string glyphs=File.ReadAllText("Assets/Editor/EclipseTypography/"+file+"-glyphs.txt");
        if(!font.TryAddCharacters(glyphs,out string missing,true))
            throw new InvalidOperationException("Font atlas did not bake all supplied glyphs: "+file+" ("+missing.Length+")");
        font.name=name; font.atlasPopulationMode=AtlasPopulationMode.Static;
        var serialized=new SerializedObject(font);
        serialized.FindProperty("m_ClearDynamicDataOnBuild").boolValue=false;
        serialized.ApplyModifiedPropertiesWithoutUndo();
        AssetDatabase.CreateAsset(font,path);
        font.material.name=name+" Material";
        AssetDatabase.AddObjectToAsset(font.material,font);
        foreach(var atlas in font.atlasTextures){atlas.name=name+" Atlas";AssetDatabase.AddObjectToAsset(atlas,font);}
        EditorUtility.SetDirty(font);
        return font;
    }
}
