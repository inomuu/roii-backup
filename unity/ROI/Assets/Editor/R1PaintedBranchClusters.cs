using System;
using System.Collections.Generic;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

// Edit Mode only: sparse branch geometry, reusable assets, and saved scene instances.
public static class R1PaintedBranchClusters
{
    const string Dir = "Assets/Environment/PainterlyMeadow/V2/";
    public static void Build()
    {
        var scene = UnityEngine.SceneManagement.SceneManager.GetActiveScene();
        if (EditorApplication.isPlayingOrWillChangePlaymode || scene.path != "Assets/Scenes/Zone_r1_Painterly.unity")
            throw new InvalidOperationException("Open Zone_r1_Painterly in Edit Mode.");
        var root = GameObject.Find("Arena/Field/PainterlyScenery/NativeFoliageV2");
        var material = AssetDatabase.LoadAssetAtPath<Material>(Dir + "PaintedLeaves.mat");
        if (root == null || material == null) throw new InvalidOperationException("Native foliage or PaintedLeaves material is missing.");
        var previous = new List<GameObject>();
        foreach (var child in root.GetComponentsInChildren<Transform>(true))
            if (child.gameObject.activeInHierarchy && child.name.StartsWith("Bush_", StringComparison.Ordinal)
                && child.GetComponent<LODGroup>() != null) previous.Add(child.gameObject);
        if (previous.Count == 0) throw new InvalidOperationException("No active native stock bushes remain to replace.");
        var prefabs = new[] { MakePrefab(0, material), MakePrefab(1, material) };
        int index = 0;
        foreach (var old in previous)
        {
            Bounds oldBounds = BoundsOf(old);
            var go = (GameObject)PrefabUtility.InstantiatePrefab(prefabs[index % 2]);
            Undo.RegisterCreatedObjectUndo(go, "Place painted branch shrub");
            go.name = "PaintedBranchShrub_" + index.ToString("00");
            go.transform.SetParent(old.transform.parent, false);
            go.transform.position = new Vector3(oldBounds.center.x, oldBounds.min.y, oldBounds.center.z);
            go.transform.rotation = Quaternion.Euler(0, old.transform.eulerAngles.y + (index % 3) * 23, 0);
            Bounds initial = BoundsOf(go);
            float byHeight = oldBounds.size.y / initial.size.y;
            float byWidth = Mathf.Max(oldBounds.size.x, oldBounds.size.z) / Mathf.Max(initial.size.x, initial.size.z);
            go.transform.localScale = Vector3.one * Mathf.Min(byHeight, byWidth * 1.1f);
            Bounds sized = BoundsOf(go);
            go.transform.position += new Vector3(oldBounds.center.x - sized.center.x, oldBounds.min.y - sized.min.y,
                oldBounds.center.z - sized.center.z);
            sized = BoundsOf(go);
            float shift = oldBounds.center.z < 0 ? Mathf.Min(0, -9.7f - sized.max.z) : Mathf.Max(0, 4f - sized.min.z);
            go.transform.position += new Vector3(0, 0, shift);
            PrefabUtility.RecordPrefabInstancePropertyModifications(go);
            PrefabUtility.RecordPrefabInstancePropertyModifications(go.transform);
            Undo.RecordObject(old, "Retain stock bush inactive");
            old.SetActive(false);
            PrefabUtility.RecordPrefabInstancePropertyModifications(old);
            index++;
        }
        AssetDatabase.SaveAssets();
        EditorSceneManager.MarkSceneDirty(scene);
        EditorSceneManager.SaveScene(scene);
        Debug.Log("Saved " + index + " painted branch shrubs; stock bushes retained inactive. Mesh variants use 14 and 17 branch planes.");
    }

    static GameObject MakePrefab(int variant, Material material)
    {
        string stem = Dir + "PaintedBranchCluster_" + (variant == 0 ? "Wide" : "Upright");
        var existing = AssetDatabase.LoadAssetAtPath<GameObject>(stem + ".prefab");
        if (existing != null) return existing;
        var vertices = new List<Vector3>(); var uv = new List<Vector2>(); var triangles = new List<int>();
        int count = variant == 0 ? 14 : 17;
        for (int i = 0; i < count; i++)
        {
            int layer = i < 5 ? 0 : i < 10 ? 1 : 2;
            float angle = (i * 137.5f + variant * 31 + Mathf.Sin(i * 1.7f) * 17) * Mathf.Deg2Rad;
            var outward = new Vector3(Mathf.Cos(angle), 0, Mathf.Sin(angle));
            var side = Vector3.Cross(Vector3.up, outward).normalized;
            float height = (layer == 0 ? 1.06f : layer == 1 ? 1.42f : 1.68f) * (1 + .13f * Mathf.Sin(i * 2.13f + variant));
            float lean = layer == 0 ? .89f : layer == 1 ? .48f : .22f;
            float width = (layer == 0 ? .95f : .84f) * (1 + .12f * Mathf.Cos(i * 1.73f));
            // The painted stem starts at the base; three staggered reaches form real depth.
            var bottom = outward * (.05f + .035f * (i % 3));
            var top = bottom + Vector3.up * height + outward * lean;
            top.x += .14f * (layer == 2 ? 1 : -1) * (variant == 0 ? 1 : -1);
            int start = vertices.Count;
            vertices.Add(bottom - side * width * .5f); vertices.Add(bottom + side * width * .5f);
            vertices.Add(top + side * width * .5f); vertices.Add(top - side * width * .5f);
            uv.Add(new Vector2(0, 0)); uv.Add(new Vector2(1, 0)); uv.Add(new Vector2(1, 1)); uv.Add(new Vector2(0, 1));
            triangles.Add(start); triangles.Add(start + 1); triangles.Add(start + 2);
            triangles.Add(start); triangles.Add(start + 2); triangles.Add(start + 3);
        }
        Bounds bounds = new Bounds(vertices[0], Vector3.zero);
        foreach (var vertex in vertices) bounds.Encapsulate(vertex);
        for (int i = 0; i < vertices.Count; i++)
        {
            Vector3 p = vertices[i];
            vertices[i] = new Vector3(p.x * 3f / bounds.size.x, p.y * 1.7f / bounds.size.y,
                p.z * (variant == 0 ? 2.4f : 2.15f) / bounds.size.z);
        }
        var mesh = new Mesh { name = "PaintedBranchCluster_" + variant };
        mesh.SetVertices(vertices); mesh.SetUVs(0, uv); mesh.SetTriangles(triangles, 0);
        mesh.RecalculateNormals(); mesh.RecalculateTangents(); mesh.RecalculateBounds();
        AssetDatabase.CreateAsset(mesh, stem + ".asset");
        var prototype = new GameObject("PaintedBranchCluster_" + (variant == 0 ? "Wide" : "Upright"));
        prototype.AddComponent<MeshFilter>().sharedMesh = mesh;
        prototype.AddComponent<MeshRenderer>().sharedMaterial = material;
        var prefab = PrefabUtility.SaveAsPrefabAsset(prototype, stem + ".prefab");
        UnityEngine.Object.DestroyImmediate(prototype);
        return prefab;
    }

    static Bounds BoundsOf(GameObject go)
    {
        var renderers = go.GetComponentsInChildren<MeshRenderer>(true);
        if (renderers.Length == 0) throw new InvalidOperationException("No geometry: " + go.name);
        Bounds bounds = renderers[0].bounds;
        for (int i = 1; i < renderers.Length; i++) bounds.Encapsulate(renderers[i].bounds);
        return bounds;
    }
}
