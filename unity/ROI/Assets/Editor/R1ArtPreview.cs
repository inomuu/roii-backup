// Editor-only camera capture for the authored zone. Never builds scenery at runtime.
using System.IO;
using System.Text;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class R1ArtPreview
{
    [MenuItem("ROI/Art/Capture Region 1")]
    public static void Capture()
    {
        if (EditorApplication.isPlaying) throw new System.InvalidOperationException("Exit Play mode first.");
        var scene = UnityEngine.SceneManagement.SceneManager.GetActiveScene();
        if (scene.path != ProbeScene.Path)
        {
            if (scene.isDirty) throw new System.InvalidOperationException("Save the current scene first.");
            EditorSceneManager.OpenScene(ProbeScene.Path);
        }
        Grab();
    }

    static void Grab()
    {
        var cam = GameObject.Find("Arena/ArenaCamera").GetComponent<Camera>();
        var prevTarget = cam.targetTexture;
        var prevActive = RenderTexture.active;
        var rt = new RenderTexture(1600, 900, 24, RenderTextureFormat.ARGBHalf) { antiAliasing = 4 };
        var tex = new Texture2D(1600, 900, TextureFormat.RGB24, false);
        try
        {
            cam.targetTexture = rt;
            cam.Render();
            RenderTexture.active = rt;
            tex.ReadPixels(new Rect(0, 0, 1600, 900), 0, 0);
            tex.Apply();
            File.WriteAllBytes("/tmp/roi-r1-art.png", tex.EncodeToPNG());
            var report = new StringBuilder();
            foreach (var group in new[] { "R1_AuthoredScenery", "Dressing", "GrassField", "Flora", "Surround" })
            {
                var groupObject = GameObject.Find("Arena/Field/" + group);
                if (groupObject == null) continue;
                var root = groupObject.transform;
                report.AppendLine(group + " children=" + root.childCount);
                foreach (Transform child in root)
                {
                    var renderers = child.GetComponentsInChildren<MeshRenderer>();
                    var b = new Bounds(child.position, Vector3.zero);
                    if (renderers.Length > 0) { b = renderers[0].bounds; foreach (var r in renderers) b.Encapsulate(r.bounds); }
                    report.AppendLine(child.name + " pos=" + child.position + " bounds=" + b + " active=" + child.gameObject.activeSelf);
                }
            }
            File.WriteAllText("/tmp/roi-r1-art-inventory.txt", report.ToString());
            Debug.Log("R1 ART CAPTURE /tmp/roi-r1-art.png");
        }
        finally
        {
            cam.targetTexture = prevTarget;
            RenderTexture.active = prevActive;
            Object.DestroyImmediate(tex);
            rt.Release();
            Object.DestroyImmediate(rt);
        }
    }
}
