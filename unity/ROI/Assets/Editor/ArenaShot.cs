// Batchmode filmstrip of the P1 arena for a GIF (verify walk/facing/glide/combat without a human).
// Enters Play mode (domain reload disabled so statics survive), reads back the arena camera every
// few frames over a window, then exits.
//   Unity -batchmode -projectPath unity/ROI -executeMethod ArenaShot.Run -logFile <f>   (NO -quit)
using System.IO;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class ArenaShot
{
    const int W = 1280, H = 720, Start = 30, End = 480, Step = 3;
    // ARENA_SHOT_TICK: grab ONE frame at that sim tick and quit, matching the Godot shotter, so a
    // luminance or framing comparison is of the same moment and not of two different fights.
    static int _tickTarget = -1;
    static int _frames, _shot;
    static Camera _cam;
    static RenderTexture _rt;

    public static void Run()
    {
        var env = System.Environment.GetEnvironmentVariable("ARENA_SHOT_TICK");
        if (!string.IsNullOrEmpty(env) && int.TryParse(env, out var n)) _tickTarget = n;
        // The shot pass runs the arena's own seed — no login in the way.
        SessionState.SetString("ROI_BOOT", "dev");
        EditorSettings.enterPlayModeOptionsEnabled = true;
        EditorSettings.enterPlayModeOptions =
            EnterPlayModeOptions.DisableDomainReload | EnterPlayModeOptions.DisableSceneReload;
        EditorSceneManager.OpenScene("Assets/Scenes/P1_Arena.unity", OpenSceneMode.Single);
        EditorApplication.playModeStateChanged += OnState;
        EditorApplication.EnterPlaymode();
    }

    static void OnState(PlayModeStateChange s)
    {
        if (s == PlayModeStateChange.EnteredPlayMode)
        {
            _frames = 0; _shot = 0;
            // MSAA on the capture too, or the grab throws away the anti-aliasing the pipeline
            // just did and every alpha-clipped blade comes back with stair-stepped tips.
            _rt = new RenderTexture(W, H, 24, RenderTextureFormat.ARGB32) { antiAliasing = 4 };
            EditorApplication.update += Tick;
            Debug.Log("ARENA SHOT entered play mode");
        }
    }

    static void Tick()
    {
        if (!EditorApplication.isPlaying) return;
        _frames++;
        if (_cam == null)
            foreach (var c in Object.FindObjectsByType<Camera>(FindObjectsSortMode.None))
                if (c.name == "ArenaCamera") { _cam = c; c.targetTexture = _rt; }
        if (_tickTarget >= 0)
        {
            var view = Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
            if (_cam == null || view == null || view.CurrentTick < _tickTarget) return;
            Grab($"/tmp/roi_tick_{_tickTarget}.png");
            Debug.Log($"ARENA SHOT tick {_tickTarget}");
            EditorApplication.update -= Tick;
            EditorApplication.Exit(0);
            return;
        }
        if (_cam != null && _frames >= Start && _frames <= End && (_frames - Start) % Step == 0)
        { Grab($"/tmp/roi_seq_{_shot:D3}.png"); _shot++; }
        if (_frames > End + 2)
        {
            Debug.Log($"ARENA SHOT frames={_shot}");
            EditorApplication.update -= Tick;
            EditorApplication.Exit(0);
        }
    }

    static void Grab(string path)
    {
        var prev = RenderTexture.active;
        RenderTexture.active = _rt;
        var tex = new Texture2D(W, H, TextureFormat.RGB24, false);
        tex.ReadPixels(new Rect(0, 0, W, H), 0, 0);
        tex.Apply();
        RenderTexture.active = prev;
        File.WriteAllBytes(path, tex.EncodeToPNG());
        Object.DestroyImmediate(tex);
    }
}
