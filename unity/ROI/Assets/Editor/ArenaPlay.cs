// Open the arena and run it, for WATCHING rather than capturing. Screenshots settled the layout,
// the colours and the light; motion — wind, the grass parting, the swing — is only judgeable live.
//   echo exec:ArenaPlay.Run > unity/ROI/.roi_cmd     (with the Editor open)
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class ArenaPlay
{
    // Grab the arena camera WHILE the Editor is playing. Batchmode cannot attach to a project the
    // Editor holds, so with a live session open this is the only way to measure a frame — and
    // measuring is how every colour question in this port has actually been settled.
    //   echo exec:ArenaPlay.Grab > unity/ROI/.roi_cmd
    public static void Grab()
    {
        Camera cam = null;
        foreach (var c in Object.FindObjectsByType<Camera>(FindObjectsSortMode.None))
            if (c.name == "ArenaCamera") cam = c;
        if (cam == null) { Debug.LogWarning("ARENA GRAB: no ArenaCamera"); return; }
        var rt = new RenderTexture(1280, 720, 24, RenderTextureFormat.ARGB32) { antiAliasing = 4 };
        var prevTarget = cam.targetTexture;
        var prevActive = RenderTexture.active;
        cam.targetTexture = rt;
        cam.Render();
        RenderTexture.active = rt;
        var tex = new Texture2D(1280, 720, TextureFormat.RGB24, false);
        tex.ReadPixels(new Rect(0, 0, 1280, 720), 0, 0);
        tex.Apply();
        RenderTexture.active = prevActive;
        cam.targetTexture = prevTarget;
        System.IO.File.WriteAllBytes("/tmp/roi_live.png", tex.EncodeToPNG());
        Object.DestroyImmediate(tex);
        rt.Release();
        Debug.Log("ARENA GRAB /tmp/roi_live.png");
    }

    public static void Stop() => EditorApplication.isPlaying = false;

    // Zone dressing, switched live: the same authored layout in the Godot look or as a standard
    // Unity 3D scene. HD2D is untouched by the switch — it is the default and rebuilds on demand.
    public static void Style3D() => SetStyle(Roi.Arena.ArenaView.ZoneStyle.Full3D);
    public static void StyleHD2D() => SetStyle(Roi.Arena.ArenaView.ZoneStyle.HD2D);
    static void SetStyle(Roi.Arena.ArenaView.ZoneStyle st)
    {
        var v = Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
        if (v == null) { Debug.LogWarning("ARENA STYLE: not playing"); return; }
        v.SetStyle(st);
        Debug.Log("ARENA STYLE " + st);
    }

    // Every tick over a window: a hurt clip is five frames at 11 fps, and a 20-tick sampler
    // cannot say whether it played, played once, or played at all.
    public static void DumpLiveDense()
    {
        var ticks = new System.Collections.Generic.List<string>();
        for (int t = 40; t <= 300; t++) ticks.Add(t.ToString());
        System.IO.File.WriteAllText(".roi_dump", string.Join(",", ticks) + "\n/tmp/unity_dense.csv\n");
        if (EditorApplication.isPlaying) EditorApplication.isPlaying = false;
        Debug.Log("ARENA DUMP DENSE armed — send exec:ArenaPlay.Run");
    }

    // The cross-engine dump from the open Editor: arm the same ticks the Godot shotter uses,
    // restart play, and ArenaView writes /tmp/unity_arena.csv when the last tick passes.
    public static void DumpLive()
    {
        var ticks = new System.Collections.Generic.List<string>();
        for (int t = 40; t <= 440; t += 20) ticks.Add(t.ToString());
        // A FILE, not a static: the request has to survive the play-mode restart, and a static
        // set here did not reliably reach ArenaView.Start on the other side of it.
        System.IO.File.WriteAllText(".roi_dump", string.Join(",", ticks) + "\n/tmp/unity_arena.csv\n");
        if (EditorApplication.isPlaying) EditorApplication.isPlaying = false;
        Debug.Log("ARENA DUMP LIVE armed — send exec:ArenaPlay.Run to start it");
    }

    // Drop a push centre into a known tuft (region 1's big clump, bottom-left of the field) and
    // clear it again, so the shiver can be measured where blades are, party or no party.
    public static void PushTest() => SetPush(new Vector4(-(-8.5f), 1f, 3.0f, 1.6f));
    public static void PushClear() => SetPush(Vector4.zero);
    static void SetPush(Vector4 p)
    {
        var v = Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
        if (v == null) { Debug.LogWarning("ARENA PUSH: not playing"); return; }
        v.DebugPush = p;
        Debug.Log($"ARENA PUSH {p}");
    }

    // The atmosphere, one piece at a time, so a frame can be judged with and without each.
    public static void ToggleFx() => WithView(v => { v.FxEnabled = !v.FxEnabled; Debug.Log("ARENA FX " + v.FxEnabled); });
    public static void ToggleCloud() => WithView(v => { v.CloudEnabled = !v.CloudEnabled; Debug.Log("ARENA CLOUD " + v.CloudEnabled); });
    public static void ToggleVignette() => ToggleOverride<UnityEngine.Rendering.Universal.Vignette>();
    public static void ToggleDof() => ToggleOverride<UnityEngine.Rendering.Universal.DepthOfField>();
    static void ToggleOverride<T>() where T : UnityEngine.Rendering.VolumeComponent
    {
        var vol = Object.FindFirstObjectByType<UnityEngine.Rendering.Volume>();
        if (vol == null || vol.sharedProfile == null || !vol.sharedProfile.TryGet<T>(out var c)) { Debug.LogWarning("ARENA POST: no " + typeof(T).Name); return; }
        c.active = !c.active;
        Debug.Log($"ARENA POST {typeof(T).Name} {(c.active ? "on" : "off")}");
    }
    static void WithView(System.Action<Roi.Arena.ArenaView> f)
    {
        var v = Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
        if (v == null) { Debug.LogWarning("ARENA: not playing"); return; }
        f(v);
    }

    // Light isolation: a left/right brightness gradient in the blades has to come from ONE of
    // these, and the only way to know which is to take each away in turn.
    public static void ToggleFill() => ToggleLight("GroundFill");
    public static void ToggleSun() => ToggleLight("Sun");
    static void ToggleLight(string name)
    {
        var go = GameObject.Find(name);
        if (go == null) { Debug.LogWarning("ARENA LIGHT: no " + name); return; }
        var l = go.GetComponent<Light>();
        l.enabled = !l.enabled;
        Debug.Log($"ARENA LIGHT {name} {(l.enabled ? "on" : "off")}");
    }

    // Hide/show the blade field, so a with/without pair isolates the BLADE pixels — the ground
    // art wears the same greens, and every crop-based measurement of the grass was reading it.
    public static void ToggleGrass()
    {
        var v = Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
        var t = v != null ? v.transform.Find("Field/GrassField") : null;
        if (t == null) { Debug.LogWarning("ARENA GRASS: no GrassField"); return; }
        t.gameObject.SetActive(!t.gameObject.activeSelf);
        Debug.Log("ARENA GRASS " + (t.gameObject.activeSelf ? "on" : "off"));
    }

    // What the frame actually COSTS. Claiming one grass approach is cheaper than another without
    // this is guessing; UnityStats reads the same counters the Game view's stats panel shows.
    public static void Stats()
    {
#if UNITY_6000_6_OR_NEWER
        const string batchInfo = "batches=unavailable";
#else
        var batchInfo = $"batches={UnityStats.batches}";
#endif
        Debug.Log($"ARENA STATS {batchInfo} drawCalls={UnityStats.drawCalls} " +
                  $"setPass={UnityStats.setPassCalls} tris={UnityStats.triangles} " +
                  $"verts={UnityStats.vertices} frameMs={UnityStats.frameTime * 1000f:F2} " +
                  $"renderMs={UnityStats.renderTime * 1000f:F2}");
    }

    // Time of day, live: 0 noon, 0.25 evening, 0.5 night, 0.75 dawn.
    public static void Noon() => SetTod(0f);
    public static void Evening() => SetTod(0.25f);
    public static void Night() => SetTod(0.5f);
    public static void Dawn() => SetTod(0.75f);

    static void SetTod(float t)
    {
        var v = Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
        if (v == null) { Debug.LogWarning("ARENA TOD: not playing"); return; }
        v.TimeOfDay = t;
        Debug.Log($"ARENA TOD {t}");
    }

    // The BAKED zone scene (ZoneBaker): the environment is scene objects, the view adopts them.
    public static void RunZone() => Run(ProbeScene.Path);

    // The same, on the DEV door with NO Steam pipe — for looking at the arena on a machine whose
    // Steam client is down. `SteamAPI.Init` treats a broken pipe as fatal and takes the Editor
    // with it (twice on 2026-09-16, `pipes.cpp … Fatal assert; application exiting`), so a
    // session that only wants to watch sprites must never open one.
    //   echo exec:ArenaPlay.RunZoneDev > unity/ROI/.roi_cmd
    // Undo RunZoneDev's doors, so the Editor's own Play button opens the title screen (and Steam)
    // again — SessionState outlives play mode, and a dev door left set would silently keep every
    // later session local.
    //   echo exec:ArenaPlay.NormalDoors > unity/ROI/.roi_cmd
    public static void NormalDoors()
    {
        SessionState.EraseString("ROI_BOOT");
        SessionState.EraseString("ROI_STEAM_PIPE");
        Debug.Log("ARENA PLAY doors reset: title screen + Steam on the next play");
    }

    public static void RunZoneDev()
    {
        SessionState.SetString("ROI_BOOT", "dev");
        SessionState.SetString("ROI_STEAM_PIPE", "0");
        Run(ProbeScene.Path);
    }

    public static void Run() => Run("Assets/Scenes/P1_Arena.unity");

    static void Run(string scenePath)
    {
        // Idempotent, not a toggle: this is driven from a script that cannot see the Editor, and
        // a toggle turned a "make sure it is running" into "stop it" — which is exactly how the
        // next Grab found no camera. Use ArenaPlay.Stop to stop.
        if (EditorApplication.isPlaying) { Debug.Log("ARENA PLAY already running"); return; }
        // Domain reload stays ON here, unlike the capture paths: this is a session someone sits
        // and watches, so a correct first frame matters more than the seconds it saves.
        EditorSceneManager.OpenScene(scenePath, OpenSceneMode.Single);
        EditorApplication.EnterPlaymode();
        Debug.Log("ARENA PLAY entered play mode: " + scenePath);
    }
}
