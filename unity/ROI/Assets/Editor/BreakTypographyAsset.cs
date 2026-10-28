using System;
using System.Collections.Generic;
using System.Text;
using TMPro;
using UnityEditor;
using UnityEngine;
using UnityEngine.TextCore.LowLevel;

// Editor-only authoring. Bake once; the shipped HUD loads the saved font and atlas.
public static class BreakTypographyAsset
{
    const string Source = "Assets/Resources/Fonts/CrimsonText-SemiBoldItalic.ttf";
    const string Destination = "Assets/Resources/Fonts/CrimsonText-Break SDF.asset";

    [MenuItem("ROI/Art/Break/Bake Display Font")]
    public static void Bake()
    {
        if (EditorApplication.isPlaying)
            throw new InvalidOperationException("Bake BREAK typography outside Play Mode.");
        AssetDatabase.ImportAsset(Source, ImportAssetOptions.ForceSynchronousImport);
        var existing = AssetDatabase.LoadAssetAtPath<TMP_FontAsset>(Destination);
        if (existing != null)
        {
            Debug.Log("BREAK display font already baked: " + Destination);
            return;
        }
        var source = AssetDatabase.LoadAssetAtPath<Font>(Source);
        if (source == null) throw new InvalidOperationException("Missing bundled Crimson Text source font.");
        var asset = TMP_FontAsset.CreateFontAsset(source, 112, 16, GlyphRenderMode.SDFAA,
            1024, 1024, AtlasPopulationMode.Dynamic, false);
        if (asset == null) throw new InvalidOperationException("Could not create BREAK SDF font.");
        var characters = new StringBuilder();
        for (int i = 32; i <= 126; i++) characters.Append((char)i);
        string missing;
        if (!asset.TryAddCharacters(characters.ToString(), out missing, true))
            throw new InvalidOperationException("BREAK font is missing ASCII glyphs: " + missing);
        asset.name = "CrimsonText-Break SDF";
        asset.atlasPopulationMode = AtlasPopulationMode.Static;
        var serialized = new SerializedObject(asset);
        serialized.FindProperty("m_ClearDynamicDataOnBuild").boolValue = false;
        serialized.ApplyModifiedPropertiesWithoutUndo();
        // Keep translated text eligible for the project's existing UI fallback chain.
        var fallback = AssetDatabase.LoadAssetAtPath<TMP_FontAsset>("Assets/Resources/Fonts/NotoSans UI SDF.asset");
        if (fallback != null) asset.fallbackFontAssetTable = new List<TMP_FontAsset> { fallback };
        AssetDatabase.CreateAsset(asset, Destination);
        asset.material.name = "CrimsonText-Break SDF Material";
        AssetDatabase.AddObjectToAsset(asset.material, asset);
        for (int i = 0; i < asset.atlasTextures.Length; i++)
        {
            var atlas = asset.atlasTextures[i];
            atlas.name = "CrimsonText-Break SDF Atlas " + i;
            AssetDatabase.AddObjectToAsset(atlas, asset);
        }
        EditorUtility.SetDirty(asset);
        AssetDatabase.SaveAssets();
        AssetDatabase.ImportAsset(Destination, ImportAssetOptions.ForceSynchronousImport);
        Debug.Log("BREAK display font baked: " + Destination + " (" + asset.characterTable.Count + " glyphs)");
    }
}
