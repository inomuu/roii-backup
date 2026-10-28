// Write the arena's whole hierarchy to a file, so two moments can be DIFFED:
//
//   echo exec:SceneDump.Fresh  > unity/ROI/.roi_cmd    -> /tmp/roi_scene_fresh.txt
//   echo exec:SceneDump.Return > unity/ROI/.roi_cmd    -> /tmp/roi_scene_return.txt
//
// "Something is loaded twice" is a question about what EXISTS, and the honest way to answer it is
// to list everything at both moments and let the difference say which. Sorted by path so the diff
// is about content and not about sibling order.
using System.Collections.Generic;
using System.IO;
using System.Text;
using UnityEditor;
using UnityEngine;

public static class SceneDump
{
    public static void Fresh() => Write("/tmp/roi_scene_fresh.txt");
    public static void Return() => Write("/tmp/roi_scene_return.txt");

    static void Write(string path)
    {
        if (!EditorApplication.isPlaying) { Debug.LogWarning("SCENEDUMP: not playing"); return; }
        var view = Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
        if (view == null) { Debug.LogWarning("SCENEDUMP: no ArenaView"); return; }
        var lines = new List<string>();
        Walk(view.transform, "", lines);
        lines.Sort(System.StringComparer.Ordinal);
        File.WriteAllText(path, string.Join("\n", lines) + "\n");
        Debug.Log($"SCENEDUMP wrote {lines.Count} nodes to {path}");
    }

    static void Walk(Transform t, string prefix, List<string> outp)
    {
        string p = prefix.Length == 0 ? t.name : prefix + "/" + t.name;
        var sb = new StringBuilder(p);
        sb.Append(t.gameObject.activeInHierarchy ? "  [on]" : "  [off]");
        var mr = t.GetComponent<Renderer>();
        if (mr != null)
        {
            var m = mr.sharedMaterial;
            sb.Append(m == null ? "  mat=NULL"
                                : $"  mat={m.name}/{(m.shader == null ? "NULLSHADER" : m.shader.name)}");
            sb.Append($" enabled={mr.enabled}");
        }
        var mf = t.GetComponent<MeshFilter>();
        if (mf != null && mf.sharedMesh != null) sb.Append($"  mesh={mf.sharedMesh.name}");
        var ter = t.GetComponent<Terrain>();
        if (ter != null) sb.Append($"  TERRAIN details={ter.terrainData.detailPrototypes.Length}"
                                 + $" layers={ter.terrainData.terrainLayers.Length}");
        // Actors and projectiles churn every frame; their names carry ids and would swamp the diff.
        if (t.name.StartsWith("Actor_") || t.name == "Projectiles") { outp.Add(sb.ToString()); return; }
        outp.Add(sb.ToString());
        foreach (Transform c in t) Walk(c, p, outp);
    }
}
