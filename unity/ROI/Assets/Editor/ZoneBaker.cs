// Bakes a zone's Full3D environment into a SCENE of real objects + assets (user 2026-09-06:
// "ทำ scene + object เลยดีกว่า อะไรไม่จำเป็น ไม่ให้สร้างตอน runtime จะได้ดูง่ายๆ"). The builders in
// ArenaView run once here, in the Editor, and everything they made in memory — materials, the
// terrain data, the tinted layers, the cookie, the post profile, the sprites — is written under
// Assets/Zones/<zone>/ so the scene can reference it. At runtime ArenaView finds these objects
// (EnvBaked) instead of building them; only actors and per-frame animation stay code.
//   menu ROI/Bake Zone r1, or: echo exec:ZoneBaker.BakeR1 > unity/ROI/.roi_cmd
using System.Collections.Generic;
using System.IO;
using Roi.Arena;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Rendering;

public static class ZoneBaker
{
    [MenuItem("ROI/Bake Zone r1 (Full3D)")]
    public static void BakeR1() => Bake("zone_b1_1", "r1");

    public static void Bake(string zoneId, string zoneName)
    {
        string assetDir = $"Assets/Zones/{zoneName}";
        string scenePath = $"Assets/Scenes/Zone_{zoneName}.unity";

        // A clean asset folder per bake: the previous bake's assets belong to the previous scene.
        if (AssetDatabase.IsValidFolder(assetDir)) AssetDatabase.DeleteAsset(assetDir);
        if (!AssetDatabase.IsValidFolder("Assets/Zones")) AssetDatabase.CreateFolder("Assets", "Zones");
        AssetDatabase.CreateFolder("Assets/Zones", zoneName);

        var scene = EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);
        var go = new GameObject("Arena");
        var view = go.AddComponent<ArenaView>();
        view.ZoneId = zoneId;
        view.BakeEnvironment(zoneId);

        // Static scenery batches at load through the STATIC FLAG — the runtime Combine is for
        // the un-baked path. Terrains are not batched (they are their own renderer).
        foreach (var n in new[] { "GrassField", "Flora", "Dressing", "WetSheen", "Surround" })
        {
            var t = go.transform.Find("Field/" + n);
            if (t == null) continue;
            foreach (var mr in t.GetComponentsInChildren<MeshRenderer>(true))
                GameObjectUtility.SetStaticEditorFlags(mr.gameObject, StaticEditorFlags.BatchingStatic);
        }

        System.Func<string> groundFrac = () => { var tr = go.transform.Find("Field/Ground"); var td0 = tr != null ? tr.GetComponent<Terrain>().terrainData : null; return td0 == null ? "-" : Frac(td0.GetAlphamaps(0, 0, td0.alphamapWidth, td0.alphamapHeight), 1).ToString("F3"); };
        Debug.Log("ZONE BAKE ground ch1 before persist=" + groundFrac());
        int saved = Persist(go.transform, assetDir);
        Debug.Log("ZONE BAKE ground ch1 after persist=" + groundFrac());
        AssetDatabase.SaveAssets();
        Debug.Log("ZONE BAKE ground ch1 after SaveAssets=" + groundFrac());

        Directory.CreateDirectory("Assets/Scenes");
        EditorSceneManager.SaveScene(scene, scenePath);
        Debug.Log("ZONE BAKE ground ch1 after SaveScene=" + groundFrac());
        var scenes = new List<EditorBuildSettingsScene>(EditorBuildSettings.scenes);
        if (!scenes.Exists(s => s.path == scenePath)) scenes.Insert(0, new EditorBuildSettingsScene(scenePath, true));
        EditorBuildSettings.scenes = scenes.ToArray();
        Debug.Log($"ZONE BAKE {zoneName}: scene {scenePath}, {saved} assets in {assetDir}");
    }

    // Every object the hierarchy references that is not yet an asset becomes one. Walks the
    // serialized properties of every component, so nothing is special-cased by name: a runtime
    // Material, the Texture2D it samples, a Sprite made from a StreamingAssets PNG, a TerrainData
    // and its TerrainLayers, the Volume profile and its overrides.
    // Alphamaps and heights of every TerrainData under the root, read BEFORE anything is
    // persisted: CreateAsset on a TerrainLayer resets the alphamaps of every TerrainData that
    // references it, and the field and the surround SHARE their tinted layers — so persisting
    // the surround zeroed the field's road before the field's turn came (v38, three bakes).
    static readonly Dictionary<TerrainData, (float[,,] maps, float[,] heights)> _terrainSnap = new();

    static int Persist(Transform root, string dir)
    {
        var done = new Dictionary<Object, Object>();
        int count = 0;
        _terrainSnap.Clear();
        foreach (var t in root.GetComponentsInChildren<Terrain>(true))
        {
            var td = t.terrainData;
            if (td == null || EditorUtility.IsPersistent(td) || _terrainSnap.ContainsKey(td)) continue;
            _terrainSnap[td] = (td.GetAlphamaps(0, 0, td.alphamapWidth, td.alphamapHeight),
                                td.GetHeights(0, 0, td.heightmapResolution, td.heightmapResolution));
            Debug.Log($"ZONE BAKE snapshot {t.name}: layers={td.terrainLayers.Length} alpha={td.alphamapWidth} ch1={Frac(_terrainSnap[td].maps, 1):F3}");
        }
        foreach (var c in root.GetComponentsInChildren<Component>(true))
        {
            if (c == null) continue;
            var so = new SerializedObject(c);
            var it = so.GetIterator();
            bool changed = false;
            while (it.Next(true))
            {
                if (it.propertyType != SerializedPropertyType.ObjectReference) continue;
                var obj = it.objectReferenceValue;
                if (obj == null || obj is Component || obj is GameObject || obj is MonoScript) continue;
                var persisted = Save(obj, dir, done, ref count);
                if (persisted != null && persisted != obj) { it.objectReferenceValue = persisted; changed = true; }
            }
            if (changed) so.ApplyModifiedPropertiesWithoutUndo();
        }
        return count;
    }

    static Object Save(Object obj, string dir, Dictionary<Object, Object> done, ref int count)
    {
        if (obj == null) return null;
        if (EditorUtility.IsPersistent(obj)) return obj;
        if (done.TryGetValue(obj, out var already)) return already;
        string safe = Sanitize(string.IsNullOrEmpty(obj.name) ? obj.GetType().Name : obj.name);
        string path = AssetDatabase.GenerateUniqueAssetPath($"{dir}/{obj.GetType().Name}_{safe}.asset");
        switch (obj)
        {
            case Material m:
                foreach (var tn in m.GetTexturePropertyNames())
                {
                    var t = m.GetTexture(tn);
                    if (t != null) m.SetTexture(tn, Save(t, dir, done, ref count) as Texture);
                }
                AssetDatabase.CreateAsset(m, path); break;
            case Sprite sp:
                {
                    // The sprite's texture becomes a PNG asset (above); a Sprite cannot be
                    // re-pointed, so it is re-created on the imported texture with the same rect,
                    // pivot and pixels-per-unit, and saved beside it.
                    var t2 = Save(sp.texture, dir, done, ref count) as Texture2D;
                    var pivot = new Vector2(sp.pivot.x / sp.rect.width, sp.pivot.y / sp.rect.height);
                    var re = Sprite.Create(t2, sp.rect, pivot, sp.pixelsPerUnit, 0, SpriteMeshType.FullRect);
                    re.name = sp.name;
                    AssetDatabase.CreateAsset(re, path);
                    done[obj] = re; count++;
                    return re;
                }
            case Texture2D tex:
                {
                    // A PNG on disk, not a serialized .asset: the region art as a text asset was
                    // 17 MB for a 2 MB image, and an importer keeps the settings visible.
                    var png = tex.EncodeToPNG();
                    if (png == null) { AssetDatabase.CreateAsset(obj, path); break; }
                    string pngPath = AssetDatabase.GenerateUniqueAssetPath($"{dir}/{safe}.png");
                    File.WriteAllBytes(pngPath, png);
                    AssetDatabase.ImportAsset(pngPath, ImportAssetOptions.ForceSynchronousImport);
                    if (AssetImporter.GetAtPath(pngPath) is TextureImporter ti)
                    {
                        ti.textureType = TextureImporterType.Default;
                        ti.sRGBTexture = true;
                        ti.mipmapEnabled = tex.mipmapCount > 1;
                        ti.filterMode = tex.filterMode;
                        ti.wrapMode = tex.wrapMode;
                        ti.npotScale = TextureImporterNPOTScale.None;
                        ti.maxTextureSize = Mathf.Max(tex.width, tex.height) > 2048 ? 4096 : 2048;
                        ti.isReadable = true;   // the sheen/bake code samples the art on CPU
                        ti.textureCompression = TextureImporterCompression.Uncompressed;   // pixel art: no block artefacts
                        ti.SaveAndReimport();
                    }
                    var imported = AssetDatabase.LoadAssetAtPath<Texture2D>(pngPath);
                    done[obj] = imported; count++;
                    return imported;
                }
            case Mesh _:
                AssetDatabase.CreateAsset(obj, path); break;
            case TerrainLayer tl:
                AssetDatabase.CreateAsset(tl, path.Replace(".asset", ".terrainlayer")); break;
            case TerrainData td:
                // Persist the layers but do NOT assign the array back: setting terrainLayers on
                // a TerrainData resets its alphamaps, and the first bake came out all grass —
                // the road and the dirt patch gone. The objects are the same instances, only
                // now on disk; the references inside td already point at them.
                {
                    // ORDER MATTERS, measured three times: persisting a TerrainLayer the data
                    // references (CreateAsset on it) RESETS the TerrainData's alphamaps, and so
                    // does SetHeights. Read the maps and heights out FIRST, persist the layers,
                    // create the asset, then write both back into it. Reading after the layers
                    // captured zeros — an all-grass road (v38).
                    (float[,,] maps, float[,] heights) snap = _terrainSnap.TryGetValue(td, out var sn) ? sn
                        : (td.GetAlphamaps(0, 0, td.alphamapWidth, td.alphamapHeight), td.GetHeights(0, 0, td.heightmapResolution, td.heightmapResolution));
                    foreach (var l in td.terrainLayers) Save(l, dir, done, ref count);
                    AssetDatabase.CreateAsset(td, path);
                    td.SetHeights(0, 0, snap.heights);
                    td.SetAlphamaps(0, 0, snap.maps);
                    // The alphamaps live in Texture2D objects of their own. When the asset was
                    // created the data had none (the layer save had just reset them), so the
                    // textures SetAlphamaps makes now are loose objects, not sub-assets, and the
                    // save drops them (written 0.174 -> after SaveAssets 0.000). Attach them.
                    foreach (var at in td.alphamapTextures)
                        if (at != null && !EditorUtility.IsPersistent(at)) AssetDatabase.AddObjectToAsset(at, td);
                    EditorUtility.SetDirty(td);
                    // FLUSH NOW. The next PNG import (any texture persisted after this) refreshes
                    // the AssetDatabase, which reloads this asset from DISK — where CreateAsset
                    // left it with no alphamaps — and the road vanished between "written 0.174"
                    // and "after persist 0.000".
                    AssetDatabase.SaveAssets();
                    Debug.Log($"ZONE BAKE terrain {td.name}: snap ch1={Frac(snap.maps, 1):F3} written ch1={Frac(td.GetAlphamaps(0, 0, td.alphamapWidth, td.alphamapHeight), 1):F3}");
                }
                break;
            case VolumeProfile vp:
                AssetDatabase.CreateAsset(vp, path);
                foreach (var comp in vp.components) if (comp != null && !EditorUtility.IsPersistent(comp)) AssetDatabase.AddObjectToAsset(comp, vp);
                break;
            default:
                Debug.LogWarning($"ZONE BAKE: not persisted ({obj.GetType().Name}) {obj.name}");
                done[obj] = obj; return obj;
        }
        done[obj] = obj; count++;
        return obj;
    }

    static float Frac(float[,,] maps, int ch)
    {
        if (maps == null || maps.GetLength(2) <= ch) return -1f;
        double sum = 0; int n = 0;
        for (int y = 0; y < maps.GetLength(0); y++) for (int x = 0; x < maps.GetLength(1); x++) { sum += maps[y, x, ch]; n++; }
        return n > 0 ? (float)(sum / n) : 0f;
    }

    static string Sanitize(string s)
    {
        foreach (var ch in Path.GetInvalidFileNameChars()) s = s.Replace(ch, '_');
        return s.Replace(' ', '_');
    }
}
