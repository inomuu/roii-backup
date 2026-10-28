// Pipeline test for the Hovl "Magic effects pack" in the LIVE arena, not a demo scene: spawn one
// effect where the fight is and look at it through the game camera, with the zone's own lighting,
// post and depth.
//   echo exec:VfxTest.Circle > .roi_cmd     (also Aoe / Shield / Slash / Explosion)
//   ROI > VFX > ... in the menu
// The pack ships BUILT-IN pipeline materials; they were converted to URP once, in place
// (Edit > Rendering > Materials > Convert Selected Built-in Materials to URP). A freshly
// re-imported pack needs that again or every effect draws magenta.
using UnityEditor;
using UnityEngine;

public static class VfxTest
{
    const string Root = "Assets/Hovl Studio/Magic effects pack/Prefabs/";
    const float Life = 6f;

    [MenuItem("ROI/VFX/Magic circle", false, 40)] public static void Circle() => Spawn("Magic circles/Magic circle", 1f, onPlayer: false);
    [MenuItem("ROI/VFX/Ground AOE explosion", false, 41)] public static void Aoe() => Spawn("AoE effects/Ground AOE explosion", 1f, onPlayer: false);
    [MenuItem("ROI/VFX/Shield on the player", false, 42)] public static void Shield() => Spawn("Magic shields/Magic shield blue", 1f, onPlayer: true);
    [MenuItem("ROI/VFX/AoE slash blue", false, 43)] public static void Slash() => Spawn("AoE effects/AoE slash blue", 1f, onPlayer: false);
    [MenuItem("ROI/VFX/Clear", false, 44)] public static void Clear()
    {
        foreach (var t in Object.FindObjectsByType<Transform>(FindObjectsSortMode.None))
            if (t.name.StartsWith("VFX_")) Object.DestroyImmediate(t.gameObject);
    }

    /// Spawns `rel` on the ground under the middle of the frame, or on the player.
    public static GameObject Spawn(string rel, float scale, bool onPlayer)
    {
        if (!EditorApplication.isPlaying) { Debug.LogWarning("VFX: enter play mode first (ArenaPlay.RunZone)"); return null; }
        var pf = AssetDatabase.LoadAssetAtPath<GameObject>(Root + rel + ".prefab");
        if (pf == null) { Debug.LogWarning("VFX: no prefab at " + Root + rel); return null; }

        Camera cam = null;
        foreach (var c in Object.FindObjectsByType<Camera>(FindObjectsSortMode.None)) if (c.name == "ArenaCamera") cam = c;
        if (cam == null) { Debug.LogWarning("VFX: no ArenaCamera"); return null; }

        Vector3 at;
        Transform parent = null;
        if (onPlayer)
        {
            var actor = FindPlayerActor();
            if (actor == null) { Debug.LogWarning("VFX: no actor to attach to"); return null; }
            parent = actor; at = actor.position;
        }
        else
        {
            // Middle of the frame, on the ground: the effect lands where the eye already is.
            at = GroundUnderCentre(cam);
        }

        var go = Object.Instantiate(pf, at, Quaternion.identity, parent);
        go.name = "VFX_" + pf.name;
        go.transform.localScale = Vector3.one * scale;
        Report(go);
        Object.Destroy(go, Life);
        return go;
    }

    static Vector3 GroundUnderCentre(Camera cam)
    {
        var ray = cam.ViewportPointToRay(new Vector3(0.5f, 0.45f, 0f));
        if (Physics.Raycast(ray, out var hit, 300f)) return hit.point;
        // No collider? Meet the y = 0 plane instead.
        float t = Mathf.Abs(ray.direction.y) > 1e-4f ? -ray.origin.y / ray.direction.y : 20f;
        return ray.origin + ray.direction * Mathf.Max(t, 1f);
    }

    static Transform FindPlayerActor()
    {
        Transform first = null;
        foreach (var t in Object.FindObjectsByType<Transform>(FindObjectsSortMode.None))
            if (t.name.StartsWith("Actor_") && (first == null || t.name.CompareTo(first.name) < 0)) first = t;
        return first;
    }

    // What the pipeline actually got: sizes, particle counts and the shader every renderer resolved
    // to. A magenta effect shows up here as a Built-in shader that URP cannot draw.
    static void Report(GameObject go)
    {
        var b = new Bounds(go.transform.position, Vector3.zero);
        int systems = 0, renderers = 0;
        var shaders = new System.Collections.Generic.HashSet<string>();
        foreach (var r in go.GetComponentsInChildren<Renderer>(true))
        {
            renderers++;
            b.Encapsulate(r.bounds);
            foreach (var m in r.sharedMaterials) if (m != null && m.shader != null) shaders.Add(m.shader.name);
        }
        foreach (var ps in go.GetComponentsInChildren<ParticleSystem>(true)) systems++;
        Debug.Log($"VFX {go.name} at {go.transform.position:F1} size {b.size:F1} | systems={systems} renderers={renderers} | shaders: {string.Join(", ", shaders)}");
    }
}
