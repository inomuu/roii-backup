using System.Collections.Generic;
// Generates the ONE asset the runtime terrain needs: a grass detail prefab whose material has
// GPU instancing enabled. Unity renders a detail mesh with its own material — and so with real
// lighting and the colours we set — only when the prototype is a prefab ASSET and that material
// allows instancing; the pack ships instancing OFF, which is why the details first came out
// olive and deaf to the scene's light.
//
// An Editor command rather than runtime code, because an asset cannot be written from a build.
//   echo exec:TerrainGrassTest.MakeDetailAsset > unity/ROI/.roi_cmd
using UnityEditor;
using UnityEngine;

public static class TerrainGrassTest
{
    // The pack pieces the FULL-3D zone variant loads at runtime. Copies with fresh GUIDs — the
    // prefabs keep their mesh/material references, the terrain layers their textures — because a
    // build can only reach assets under Resources and two assets sharing one GUID is a corrupt
    // project.
    //   echo exec:TerrainGrassTest.MakeDressing3DAssets > unity/ROI/.roi_cmd
    public static void MakeDressing3DAssets()
    {
        const string Src = "Assets/Idyllic Fantasy Nature";
        const string Dst = "Assets/Resources/Dressing3D";
        if (!AssetDatabase.IsValidFolder(Dst)) AssetDatabase.CreateFolder("Assets/Resources", "Dressing3D");
        var items = new (string, string)[]
        {
            ($"{Src}/Prefabs/BroadleafTree_01_Green.prefab", "Tree.prefab"),
            ($"{Src}/Prefabs/Bush_01_01.prefab", "Bush.prefab"),
            ($"{Src}/Prefabs/Rock_Medium_01.prefab", "Rock.prefab"),
            ($"{Src}/Terrain Layer/Grass_Layer.terrainlayer", "Grass.terrainlayer"),
            ($"{Src}/Terrain Layer/Cobblestone_Layer.terrainlayer", "Cobblestone.terrainlayer"),
            ($"{Src}/Terrain Layer/Dirt_Layer.terrainlayer", "Dirt.terrainlayer"),
            ($"{Src}/Terrain Layer/Rock_Layer.terrainlayer", "Rock.terrainlayer"),   // the road: see BuildTerrain3D StoneLayer
            ($"{Src}/Materials/Skybox/Skybox.mat", "Skybox.mat"),
            // The pack's own atmosphere: light-shaft and dust particle systems, used the pack's
            // way (world-placed) rather than as a port of Godot's screen-quad shaders.
            ($"{Src}/Prefabs/GodRays.prefab", "GodRays.prefab"),
            ($"{Src}/Prefabs/Particles.prefab", "Motes.prefab"),
            // Flora for the Full3D fill (BuildFlora3D / BuildPackGrass): the pack's grass tufts,
            // flowers, plants, a second bush and two more trees.
            ($"{Src}/Prefabs/Grass_01.prefab", "Grass1.prefab"),
            ($"{Src}/Prefabs/Grass_02.prefab", "Grass2.prefab"),
            ($"{Src}/Prefabs/Grass_03.prefab", "Grass3.prefab"),
            ($"{Src}/Prefabs/FlowerMeadow_White.prefab", "Meadow1.prefab"),
            ($"{Src}/Prefabs/FlowerMeadow_Pink.prefab", "Meadow2.prefab"),
            ($"{Src}/Prefabs/FlowerMeadow_Blue.prefab", "Meadow3.prefab"),
            ($"{Src}/Prefabs/Flower_Yellow.prefab", "Flower1.prefab"),
            ($"{Src}/Prefabs/Flower_White.prefab", "Flower2.prefab"),
            ($"{Src}/Prefabs/Flower_Pink.prefab", "Flower3.prefab"),
            ($"{Src}/Prefabs/Plant_01.prefab", "Plant1.prefab"),
            ($"{Src}/Prefabs/Plant_03.prefab", "Plant2.prefab"),
            ($"{Src}/Prefabs/Bush_02_01.prefab", "Bush2.prefab"),
            ($"{Src}/Prefabs/BroadleafTree_02_Green.prefab", "Tree2.prefab"),
            ($"{Src}/Prefabs/BlossomTree_01.prefab", "Tree3.prefab"),
            // The demo's drifting petals and leaves (user: "demo particle สวยดี"). Their emitter
            // shape is the demo's terrain MESH; BuildAtmosphere boxes them over the region art.
            ($"{Src}/Demo/Prefabs/FloatingLeafs_Pink.prefab", "Petals.prefab"),
            ($"{Src}/Prefabs/FloatingLeafs_Green.prefab", "Leaves.prefab"),
        };
        int n = 0;
        foreach (var (from, name) in items)
        {
            var to = $"{Dst}/{name}";
            AssetDatabase.DeleteAsset(to);
            if (AssetDatabase.CopyAsset(from, to)) n++; else Debug.LogWarning("copy failed: " + from);
        }
        AssetDatabase.SaveAssets();
        // The scenery is static-batched at RUNTIME (ArenaView.CombineQueued), and
        // StaticBatchingUtility.Combine silently skips a mesh whose importer has Read/Write off
        // — every pack mesh ships that way (238 tufts, 0 batched, no warning). The pack folder is
        // gitignored, so the flag is set HERE, by the same tool that prepares the pack for the
        // game, rather than by hand in an importer nobody else's checkout has.
        int readable = 0;
        var meshes = new HashSet<string>();
        foreach (var guid in AssetDatabase.FindAssets("t:Prefab", new[] { Dst }))
        {
            var pf = AssetDatabase.LoadAssetAtPath<GameObject>(AssetDatabase.GUIDToAssetPath(guid));
            if (pf == null) continue;
            foreach (var mf in pf.GetComponentsInChildren<MeshFilter>(true))
                if (mf.sharedMesh != null) meshes.Add(AssetDatabase.GetAssetPath(mf.sharedMesh));
        }
        foreach (var path in meshes)
        {
            if (AssetImporter.GetAtPath(path) is not ModelImporter mi || mi.isReadable) continue;
            mi.isReadable = true;
            mi.SaveAndReimport();
            readable++;
        }
        Debug.Log($"DRESSING3D assets written: {n}/{items.Length}; meshes made readable: {readable}/{meshes.Count}");
    }

    const string MatPath = "Assets/Resources/Dressing/GrassDetail.mat";
    const string ProtoPath = "Assets/Resources/Dressing/GrassDetail.prefab";

    public static void MakeDetailAsset()
    {
        var prefab = Resources.Load<GameObject>("Dressing/Grass_01");
        if (prefab == null) { Debug.LogWarning("no Dressing/Grass_01"); return; }
        var mr = prefab.GetComponentInChildren<MeshRenderer>();
        var mf = prefab.GetComponentInChildren<MeshFilter>();
        if (mr == null || mf == null) { Debug.LogWarning("prefab has no mesh"); return; }

        // Our own vegetation shader, not the pack's: the pack bends where Godot shivers, has one
        // push position for the whole world where Godot gives every clump its own, and ignores the
        // root gradient. The pack's TEXTURE is what we wanted from it, and that carries over.
        var sh = Shader.Find("ROI/GrassBlade");
        var mat = new Material(sh != null ? sh : mr.sharedMaterial.shader)
            { enableInstancing = true, name = "GrassDetail" };
        var srcTex = mr.sharedMaterial.HasProperty("_MainTex") ? mr.sharedMaterial.GetTexture("_MainTex")
                   : mr.sharedMaterial.HasProperty("_BaseMap") ? mr.sharedMaterial.GetTexture("_BaseMap") : null;
        if (srcTex != null && mat.HasProperty("_MainTex")) mat.SetTexture("_MainTex", srcTex);
        // Blade height off the MESH, since the shader keys the sway and the root gradient to it
        // and a mesh from another pack has no UV convention to lean on.
        if (mat.HasProperty("_BladeHeight")) mat.SetFloat("_BladeHeight", mf.sharedMesh.bounds.size.y);
        AssetDatabase.DeleteAsset(MatPath);
        AssetDatabase.DeleteAsset(ProtoPath);
        AssetDatabase.CreateAsset(mat, MatPath);
        var tmp = new GameObject("GrassDetail");
        tmp.AddComponent<MeshFilter>().sharedMesh = mf.sharedMesh;
        tmp.AddComponent<MeshRenderer>().sharedMaterial = mat;
        PrefabUtility.SaveAsPrefabAsset(tmp, ProtoPath);
        Object.DestroyImmediate(tmp);
        AssetDatabase.SaveAssets();
        Debug.Log("TERRAIN detail asset written: " + ProtoPath);
    }
}
