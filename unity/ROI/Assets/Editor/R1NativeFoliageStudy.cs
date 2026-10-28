using System;
using System.Collections.Generic;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

// Explicit Edit Mode authoring. Nothing from this helper is attached to the saved scene.
public static class R1NativeFoliageStudy
{
    const string ScenePath = "Assets/Scenes/Zone_r1_Painterly.unity";
    const string PrefabDir = "Assets/Idyllic Fantasy Nature/Prefabs/";
    const string VariantDir = "Assets/Environment/PainterlyMeadow/V2";

    [MenuItem("ROI/Art/Build Native Foliage Study V2")]
    public static void Build()
    {
        var scene = UnityEngine.SceneManagement.SceneManager.GetActiveScene();
        if (EditorApplication.isPlayingOrWillChangePlaymode || scene.path != ScenePath)
            throw new InvalidOperationException("Open Zone_r1_Painterly in Edit Mode first.");
        var scenery = GameObject.Find("Arena/Field/PainterlyScenery");
        if (scenery == null || scenery.scene != scene)
            throw new InvalidOperationException("PainterlyScenery is missing from the active scene.");
        if (scenery.transform.Find("NativeFoliageV2") != null)
            throw new InvalidOperationException("NativeFoliageV2 is already authored; edit its saved instances.");

        var prefabs = new Dictionary<string, GameObject>();
        foreach (string name in new[] { "Bush_01_02", "Bush_02_02", "Bush_03_01", "Plant_01", "Plant_03",
            "BroadleafTree_01_Green", "BroadleafTree_02_Green", "BroadleafTree_05_Green",
            "Stone_Medium_01", "Stone_Medium_02", "Stone_Big_02" })
        {
            var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(PrefabDir + name + ".prefab");
            if (prefab == null) throw new InvalidOperationException("Missing native prefab: " + name);
            prefabs.Add(name, prefab);
        }
        Terrain terrain = null;
        foreach (var candidate in UnityEngine.Object.FindObjectsByType<Terrain>(FindObjectsSortMode.None))
            if (candidate.gameObject.scene == scene) { terrain = candidate; break; }
        if (terrain == null) throw new InvalidOperationException("The scene terrain is missing.");

        Undo.IncrementCurrentGroup();
        int undoGroup = Undo.GetCurrentGroup();
        Undo.SetCurrentGroupName("Native foliage art study V2");
        try
        {
            var root = new GameObject("NativeFoliageV2");
            Undo.RegisterCreatedObjectUndo(root, "Create native foliage group");
            root.transform.SetParent(scenery.transform, false);
            var back = Group(root.transform, "Back_Groves");
            var edge = Group(root.transform, "Wall_LowGrowth");
            var front = Group(root.transform, "Foreground_RocksAndBranches");
            var author = new Author(prefabs, terrain);

            // All back geometry, including the canopy bounds, stays behind the boundary.
            author.Place(back, "BroadleafTree_01_Green", 18.0f, -18.4f, 9.2f, 24, -10.9f, true);
            author.Place(back, "BroadleafTree_02_Green", 9.8f, -20.1f, 8.5f, 147, -10.9f, true);
            author.Place(back, "BroadleafTree_05_Green", 2.6f, -19.5f, 8.0f, 291, -10.9f, true);
            author.Place(back, "BroadleafTree_02_Green", -13.8f, -18.6f, 9.4f, 214, -10.9f, true);
            author.Place(back, "BroadleafTree_01_Green", -22.1f, -20.0f, 8.7f, 73, -10.9f, true);

            // Asymmetric islands: wide shrubs, branching shrubs, low ferns, and occasional rock.
            author.Island(back, 21.6f, -12.8f, 1.85f, 27);
            author.Island(back, 17.3f, -13.1f, 1.35f, 104);
            author.Island(back, 12.9f, -13.4f, 1.65f, 238);
            author.Island(back, 8.7f, -12.5f, 1.95f, 181);
            author.Island(back, 4.1f, -13.1f, 1.40f, 315);
            author.Island(back, .5f, -12.6f, 1.70f, 69);
            // Leave the shrine at x=-5.8 visually open.
            author.Island(back, -2.1f, -14.1f, 1.15f, 152);
            author.Island(back, -9.0f, -13.2f, 1.50f, 256);
            author.Island(back, -13.6f, -12.7f, 1.90f, 32);
            author.Island(back, -17.4f, -13.9f, 1.25f, 193);
            author.Island(back, -21.8f, -12.8f, 1.75f, 278);
            author.Place(back, "Stone_Big_02", 6.0f, -15.0f, 1.55f, 26, -10.9f, true);
            author.Place(back, "Stone_Medium_02", -16.6f, -13.3f, 1.05f, 138, -10.9f, true);

            // Low accents on the wall's near side; every bound stays outside the combat lane.
            float[] edgeX = { -22.8f, -18.9f, -16.6f, -10.1f, -7.8f, -3.1f, 1.8f, 5.7f, 9.2f, 15.4f, 19.7f, 22.1f };
            for (int i = 0; i < edgeX.Length; i++)
            {
                float x = edgeX[i];
                author.Place(edge, i % 3 == 0 ? "Bush_02_02" : "Plant_03", x,
                    -9.55f - .12f * (i % 3), .34f + .07f * (i % 4), 31 + i * 71, -8.6f, true);
                if (i % 3 != 1)
                    author.Place(edge, "Plant_01", x + .46f, -9.3f, .24f + .04f * (i % 3), i * 47, -8.6f, true);
            }

            // Foreground has heavy corners and gaps through the center, not a repeated hedge.
            author.Place(front, "Stone_Big_02", 11.0f, 7.4f, 1.40f, 232, 6.8f, false);
            author.Place(front, "Stone_Medium_02", 7.9f, 7.5f, .95f, 62, 6.8f, false);
            author.Place(front, "Stone_Medium_01", 3.5f, 8.2f, .64f, 144, 6.8f, false);
            author.Place(front, "Stone_Medium_02", -6.2f, 7.6f, .90f, 285, 6.8f, false);
            author.Place(front, "Stone_Big_02", -10.6f, 7.6f, 1.27f, 85, 6.8f, false);
            author.Place(front, "Bush_02_02", 13.5f, 7.4f, 1.72f, 83, 6.8f, false);
            author.Place(front, "Bush_01_02", 10.0f, 8.6f, 1.47f, 202, 6.8f, false);
            author.Place(front, "Bush_03_01", 6.4f, 8.6f, 1.00f, 15, 6.8f, false);
            author.Place(front, "Plant_03", 4.8f, 7.5f, .46f, 296, 6.8f, false);
            author.Place(front, "Plant_01", 1.4f, 8.4f, .40f, 62, 6.8f, false);
            author.Place(front, "Bush_02_02", -4.5f, 8.4f, .76f, 129, 6.8f, false);
            author.Place(front, "Bush_01_02", -8.0f, 8.8f, 1.31f, 266, 6.8f, false);
            author.Place(front, "Bush_02_02", -12.7f, 7.5f, 1.65f, 174, 6.8f, false);
            author.Place(front, "Plant_03", -9.7f, 7.0f, .49f, 43, 6.8f, false);

            int disabled = DisableGeneratedCards(scenery.transform, root.transform);
            ValidateOutsideCombat(root.transform);
            AssetDatabase.SaveAssets();
            EditorSceneManager.MarkSceneDirty(scene);
            if (!EditorSceneManager.SaveScene(scene)) throw new InvalidOperationException("Could not save the scene.");
            Undo.CollapseUndoOperations(undoGroup);
            Debug.Log("NativeFoliageV2 saved: " + author.Count + " native props; " + disabled + " previous dressing objects retained inactive.");
        }
        catch
        {
            Undo.RevertAllDownToGroup(undoGroup);
            throw;
        }
    }

    static Transform Group(Transform parent, string name)
    {
        var go = new GameObject(name);
        Undo.RegisterCreatedObjectUndo(go, "Create foliage subgroup");
        go.transform.SetParent(parent, false);
        return go.transform;
    }

    static int DisableGeneratedCards(Transform scenery, Transform newRoot)
    {
        int count = 0;
        foreach (var child in scenery.GetComponentsInChildren<Transform>(true))
        {
            if (child.IsChildOf(newRoot) || !child.name.StartsWith("FramingBoulder_", StringComparison.Ordinal)
                || !child.gameObject.activeSelf) continue;
            Undo.RecordObject(child.gameObject, "Retain previous framing rock inactive");
            child.gameObject.SetActive(false);
            PrefabUtility.RecordPrefabInstancePropertyModifications(child.gameObject);
            count++;
        }
        foreach (var renderer in scenery.GetComponentsInChildren<MeshRenderer>(true))
        {
            if (renderer.transform.IsChildOf(newRoot)) continue;
            bool generated = renderer.name.StartsWith("PainterlyOak_", StringComparison.Ordinal);
            foreach (var material in renderer.sharedMaterials)
            {
                if (material == null) continue;
                var texture = material.HasProperty("_BaseMap") ? material.GetTexture("_BaseMap") : null;
                string label = material.name + " " + (texture == null ? "" : texture.name);
                generated |= label.IndexOf("Hazel", StringComparison.OrdinalIgnoreCase) >= 0
                    || label.IndexOf("SelectedOak", StringComparison.OrdinalIgnoreCase) >= 0
                    || AssetDatabase.GetAssetPath(material) == "Assets/Environment/PainterlyMeadow/Oak.mat"
                    || (texture != null && AssetDatabase.GetAssetPath(texture) == "Assets/Environment/PainterlyMeadow/Oak.png");
            }
            if (!generated || !renderer.gameObject.activeSelf) continue;
            Undo.RecordObject(renderer.gameObject, "Retain generated card inactive");
            renderer.gameObject.SetActive(false);
            PrefabUtility.RecordPrefabInstancePropertyModifications(renderer.gameObject);
            count++;
        }
        return count;
    }

    public static void ValidateOutsideCombat(Transform root)
    {
        foreach (var renderer in root.GetComponentsInChildren<MeshRenderer>(true))
        {
            var b = renderer.bounds;
            if (b.max.x > -10 && b.min.x < 10 && b.max.z > -7 && b.min.z < 3)
                throw new InvalidOperationException("Native foliage enters the combat rectangle: " + renderer.transform.parent.name);
        }
    }

    static Bounds MeshBounds(GameObject go)
    {
        var renderers = go.GetComponentsInChildren<MeshRenderer>(true);
        if (renderers.Length == 0) throw new InvalidOperationException("Prefab has no mesh renderer: " + go.name);
        var bounds = renderers[0].bounds;
        for (int i = 1; i < renderers.Length; i++) bounds.Encapsulate(renderers[i].bounds);
        if (bounds.size.y < .001f) throw new InvalidOperationException("Prefab has no measurable height: " + go.name);
        return bounds;
    }

    sealed class Author
    {
        readonly Dictionary<string, GameObject> prefabs;
        readonly Terrain terrain;
        Material greenBush03;
        public int Count { get; private set; }

        public Author(Dictionary<string, GameObject> prefabs, Terrain terrain)
        {
            this.prefabs = prefabs;
            this.terrain = terrain;
        }

        public void Island(Transform parent, float x, float z, float height, float yaw)
        {
            string main = Mathf.RoundToInt(yaw) % 3 == 0 ? "Bush_02_02" : "Bush_01_02";
            Place(parent, main, x, z, height, yaw, -10.9f, true);
            Place(parent, "Bush_03_01", x + 1.22f, z - .60f, height * .63f, yaw + 91, -10.9f, true);
            Place(parent, "Plant_03", x - .97f, z + .46f, .60f, yaw + 177, -10.9f, true);
            Place(parent, "Plant_01", x + .70f, z + .84f, .43f, yaw + 58, -10.9f, true);
        }

        public void Place(Transform parent, string asset, float x, float z, float height, float yaw, float boundaryZ, bool behind)
        {
            var go = (GameObject)PrefabUtility.InstantiatePrefab(prefabs[asset]);
            Undo.RegisterCreatedObjectUndo(go, "Place native foliage");
            go.name = asset + "_" + Count.ToString("00");
            go.transform.SetParent(parent, false);
            go.transform.position = new Vector3(x, 0, z);
            go.transform.rotation = Quaternion.Euler(0, yaw, 0);
            var initial = MeshBounds(go);
            go.transform.localScale *= height / initial.size.y;
            var sized = MeshBounds(go);
            // Centre the visible geometry, independent of the imported prefab's pivot.
            go.transform.position += new Vector3(x - sized.center.x, 0, z - sized.center.z);
            sized = MeshBounds(go);
            float correctionZ = behind ? Mathf.Min(0, boundaryZ - sized.max.z) : Mathf.Max(0, boundaryZ - sized.min.z);
            go.transform.position += new Vector3(0, 0, correctionZ);
            sized = MeshBounds(go);
            float groundY = terrain.SampleHeight(go.transform.position) + terrain.transform.position.y;
            go.transform.position += Vector3.up * (groundY - sized.min.y - .025f);

            // These are scene dressing, so the vendor's interaction/particle examples stay off.
            foreach (var collider in go.GetComponentsInChildren<Collider>(true))
            { collider.enabled = false; PrefabUtility.RecordPrefabInstancePropertyModifications(collider); }
            foreach (var behaviour in go.GetComponentsInChildren<MonoBehaviour>(true))
                if (behaviour != null) { behaviour.enabled = false; PrefabUtility.RecordPrefabInstancePropertyModifications(behaviour); }
            foreach (var particle in go.GetComponentsInChildren<ParticleSystem>(true))
                if (particle.gameObject != go)
                { particle.gameObject.SetActive(false); PrefabUtility.RecordPrefabInstancePropertyModifications(particle.gameObject); }
            foreach (var lod in go.GetComponentsInChildren<LODGroup>(true))
            {
                var levels = lod.GetLODs();
                for (int i = 0; i < levels.Length; i++)
                    levels[i].screenRelativeTransitionHeight = i == 0 ? .15f : i == 1 ? .055f : .01f / (i - 1);
                lod.SetLODs(levels);
                lod.RecalculateBounds();
                PrefabUtility.RecordPrefabInstancePropertyModifications(lod);
            }
            // Stock Bush_03 has a pink tip tint. Own only this green variant; leave vendor assets intact.
            if (asset == "Bush_03_01")
                foreach (var renderer in go.GetComponentsInChildren<MeshRenderer>(true))
                {
                    var materials = renderer.sharedMaterials;
                    for (int i = 0; i < materials.Length; i++)
                        if (materials[i] != null && materials[i].name == "Bush_03") materials[i] = GreenBush03(materials[i]);
                    renderer.sharedMaterials = materials;
                    PrefabUtility.RecordPrefabInstancePropertyModifications(renderer);
                }
            PrefabUtility.RecordPrefabInstancePropertyModifications(go);
            PrefabUtility.RecordPrefabInstancePropertyModifications(go.transform);
            Count++;
        }

        Material GreenBush03(Material original)
        {
            if (greenBush03 != null) return greenBush03;
            if (!AssetDatabase.IsValidFolder(VariantDir)) AssetDatabase.CreateFolder("Assets/Environment/PainterlyMeadow", "V2");
            string path = VariantDir + "/Bush03_MeadowGreen.mat";
            greenBush03 = AssetDatabase.LoadAssetAtPath<Material>(path);
            if (greenBush03 != null) return greenBush03;
            greenBush03 = new Material(original);
            greenBush03.SetColor("_Top_Color", new Color(.69f, .75f, .35f, 0));
            greenBush03.SetColor("_Bottom_Color", new Color(.37f, .48f, .23f, 0));
            greenBush03.SetFloat("_Wind_Strength", .15f);
            greenBush03.SetFloat("_Wind_Speed", .18f);
            AssetDatabase.CreateAsset(greenBush03, path);
            return greenBush03;
        }
    }
}
