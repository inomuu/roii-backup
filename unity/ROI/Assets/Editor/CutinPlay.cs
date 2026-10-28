// Terminal-driven Live2D cut-in test (RemoteControl exec targets):
//   echo exec:CutinPlay.Run > .roi_cmd          # P1 arena into play mode
//   echo exec:CutinPlay.Loop > .roi_cmd         # replay forever (StopLoop to end)
//   echo exec:CutinPlay.PlayAndGrab > .roi_cmd  # play the cut-in, write /tmp/roi_cutin_{1..4}.png
//                                                 (one per framing beat) + /tmp/roi_cutin_screen.png
using System.IO;
using Roi.Cutin;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class CutinPlay
{
    const string Model = "Assets/Live2D/TestModels/67/67.prefab";
    const string Motion = "Assets/Live2D/TestModels/67/67_loop.controller";
    static readonly float[] GrabAt = { 3.95f, 4.5f, 5.7f, 5.9f };

    static Live2DCutin _cutin;
    static int _grabbed;

    // Menu + a dev key, so the cut-in can be triggered by hand and not only from the terminal.
    // ROI > Cut-in > ... in the menu bar; J while the game is playing (the same key Godot's
    // cutin_view.gd uses for its dev trigger).
    [MenuItem("ROI/Cut-in/Play once _j", false, 20)]
    public static void MenuPlayOnce()
    {
        if (!EditorApplication.isPlaying) { Run(); EditorApplication.delayCall += () => Debug.Log("CUTIN: entering play mode — press J again once the arena is up"); return; }
        Start();
    }

    [MenuItem("ROI/Cut-in/Loop", false, 21)]
    public static void MenuLoop() { if (EditorApplication.isPlaying) Loop(); else Run(); }

    [MenuItem("ROI/Cut-in/Stop loop", false, 22)]
    public static void MenuStopLoop() => StopLoop();

    public static void Run()
    {
        if (EditorApplication.isPlaying) { Debug.Log("CUTIN already playing"); return; }
        EditorSceneManager.OpenScene("Assets/Scenes/P1_Arena.unity", OpenSceneMode.Single);
        EditorApplication.EnterPlaymode();
        Debug.Log("CUTIN entered play mode");
    }

    public static void Play() => Start();

    // Replay back-to-back (0.6 s black between) until play mode stops or StopLoop.
    static bool _loop;
    static double _restartAt;
    public static void Loop() { if (Start() == null) return; _loop = true; _restartAt = 0; EditorApplication.update += LoopPoll; }
    public static void StopLoop() { _loop = false; EditorApplication.update -= LoopPoll; }
    static void LoopPoll()
    {
        if (!_loop || !EditorApplication.isPlaying) { StopLoop(); return; }
        if (_cutin != null && !_cutin.Finished) return;
        if (_restartAt == 0) _restartAt = EditorApplication.timeSinceStartup + 0.6;
        if (EditorApplication.timeSinceStartup < _restartAt) return;
        _restartAt = 0;
        Start();
    }

    public static void PlayAndGrab()
    {
        if (Start() == null) return;
        _grabbed = 0;
        EditorApplication.update += Poll;
    }

    static Live2DCutin Start()
    {
        if (!EditorApplication.isPlaying) { Debug.LogWarning("CUTIN: not in play mode"); return null; }
        var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(Model);
        var motion = AssetDatabase.LoadAssetAtPath<RuntimeAnimatorController>(Motion);
        if (prefab == null) { Debug.LogWarning("CUTIN: no model at " + Model); return null; }
        _cutin = Live2DCutin.Play(prefab, motion);
        Debug.Log($"CUTIN started model={prefab.name} motion={(motion ? motion.name : "none")}");
        return _cutin;
    }

    static void Poll()
    {
        if (_cutin == null || _cutin.Finished || !EditorApplication.isPlaying)
        {
            EditorApplication.update -= Poll;
            Debug.Log($"CUTIN done, grabbed {_grabbed}");
            return;
        }
        if (_grabbed < GrabAt.Length && _cutin.Elapsed >= GrabAt[_grabbed])
        {
            _grabbed++;
            GrabRt(_cutin.Rt, $"/tmp/roi_cutin_{_grabbed}.png");
            if (_grabbed == 3) ScreenCapture.CaptureScreenshot("/tmp/roi_cutin_screen.png");
        }
    }

    // Diagnostics: a long cut-in WITHOUT a motion, and after a second a dump of what the Cubism
    // pass sees: the group's controllers, the model's bounds, the camera, the first drawables.
    public static void Diag()
    {
        if (!EditorApplication.isPlaying) { Debug.LogWarning("CUTIN: not in play mode"); return; }
        var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(Model);
        _cutin = Live2DCutin.Play(prefab, null);
        _grabbed = 0;
        EditorApplication.update += DiagPoll;
    }

    static void DiagPoll()
    {
        if (_cutin == null || _cutin.Finished || !EditorApplication.isPlaying) { EditorApplication.update -= DiagPoll; return; }
        if (_cutin.Elapsed < 1f) return;
        EditorApplication.update -= DiagPoll;
        var group = Live2D.Cubism.Rendering.CubismRenderControllerGroup.GetInstance();
        var cam = _cutin.Cam;
        Debug.Log($"CUTIN DIAG controllers={group.RenderControllers?.Length} bounds={_cutin.ModelBoundsAtStart} " +
                  $"cam pos={cam.transform.position} size={cam.orthographicSize} aspect={cam.aspect} target={(cam.targetTexture ? cam.targetTexture.name : "screen")} " +
                  $"model pos={_cutin.Model.transform.position} layer={_cutin.Model.layer}");
        var n = 0;
        foreach (var r in _cutin.Model.GetComponentsInChildren<Live2D.Cubism.Rendering.CubismRenderer>())
        {
            if (n++ >= 3) break;
            var mr = r.MeshRenderer;
            Debug.Log($"CUTIN DIAG drawable {r.name} mrEnabled={mr.enabled} visible={mr.isVisible} bounds={mr.bounds} " +
                      $"shader={(mr.sharedMaterial ? mr.sharedMaterial.shader.name : "none")} skip={r.SkipRendering} verts={(r.Mesh ? r.Mesh.vertexCount : -1)}");
        }
        GrabRt(_cutin.Rt, "/tmp/roi_cutin_diag.png");
    }

    public static void Stop() { if (_cutin != null) Object.Destroy(_cutin.gameObject); }

    static void GrabRt(RenderTexture rt, string path)
    {
        var prev = RenderTexture.active;
        RenderTexture.active = rt;
        var tex = new Texture2D(rt.width, rt.height, TextureFormat.RGB24, false);
        tex.ReadPixels(new Rect(0, 0, rt.width, rt.height), 0, 0);
        tex.Apply();
        RenderTexture.active = prev;
        File.WriteAllBytes(path, tex.EncodeToPNG());
        Object.DestroyImmediate(tex);
        Debug.Log("CUTIN grab " + path);
    }
}
