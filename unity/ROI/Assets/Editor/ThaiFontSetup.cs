using System.Collections.Generic;
using TMPro;
using UnityEditor;
using UnityEngine;

public static class ThaiFontSetup
{
    public static void Run()
    {
        const string path = "Assets/Resources/Fonts/NotoSansThai UI SDF.asset";
        var fallback = AssetDatabase.LoadAssetAtPath<TMP_FontAsset>(path);
        if (fallback == null)
        {
            var source = AssetDatabase.LoadAssetAtPath<Font>("Assets/Resources/Fonts/NotoSansThai-Regular.ttf");
            fallback = TMP_FontAsset.CreateFontAsset(source);
            fallback.name = "NotoSansThai UI SDF";
            AssetDatabase.CreateAsset(fallback, path);
            AssetDatabase.AddObjectToAsset(fallback.material, fallback);
            foreach (var texture in fallback.atlasTextures)
                AssetDatabase.AddObjectToAsset(texture, fallback);
        }
        var main = AssetDatabase.LoadAssetAtPath<TMP_FontAsset>("Assets/Resources/Fonts/NotoSans UI SDF.asset");
        main.fallbackFontAssetTable ??= new List<TMP_FontAsset>();
        if (!main.fallbackFontAssetTable.Contains(fallback)) main.fallbackFontAssetTable.Add(fallback);
        EditorUtility.SetDirty(main);
        EditorUtility.SetDirty(fallback);
        AssetDatabase.SaveAssets();
        Debug.Log("THAI FONT: fallback installed");
    }
}
