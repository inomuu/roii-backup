// Headless P0 probe: drive the sim through the P/Invoke binding inside the Unity Editor runtime
// (Mono) without Play Mode, and build the P0 scene.
//   Unity -batchmode -projectPath unity/ROI -executeMethod SimFfiProbe.Run -logFile - -quit
// Mirrors crates/sim-ffi's own tests, so a difference here is a MARSHALLING bug, not a sim bug.
using System;
using System.IO;
using Roi.Sim;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class SimFfiProbe
{
    public static void Run()
    {
        var gdPath = Path.Combine(Application.streamingAssetsPath, "game_data.json");
        var gd = File.ReadAllText(gdPath);
        Debug.Log($"PROBE sim_version={SimSession.SimVersion()} stamp={SimSession.SimGitStamp()}");

        using (var s = SimSession.NewAs(42, gd, "zone_b1_1", "swordsman_t1", "Idlers"))
        {
            if (s == null) { Debug.LogError("PROBE FAIL: roi_new_as returned null"); return; }
            var h0 = s.StateHash();
            s.Tick(300);
            var applied = s.ApplyInput("{\"ResetDps\":null}");
            s.Tick(300);
            var h1 = s.StateHash();
            var view = s.ViewJson();
            var save = s.SaveJson();
            var bundle = s.ExportLog();
            var span0 = s.ReplaySpan();
            s.CommitAnchor();
            var span1 = s.ReplaySpan();
            Debug.Log($"PROBE h0={h0} h1={h1} changed={h0 != h1} applied={applied} view={view.Length}B save={save.Length}B bundle={(bundle == null ? -1 : bundle.Length)}B span {span0}->{span1}");
            using (var l = SimSession.Load(save, gd))
            {
                var hl = l?.StateHash();
                Debug.Log($"PROBE load_hash_equal={hl == h1} ({hl})");
                if (hl != h1) Debug.LogError("PROBE FAIL: load round-trip hash differs");
            }
            if (h0 == h1 || !applied || bundle == null || span0 != 600 || span1 != 0)
                Debug.LogError("PROBE FAIL: see values above");
            else
                Debug.Log("PROBE OK");
        }
        CreateP0Scene();
    }

    /// Assets/Scenes/P0.unity: one GameObject with Bootstrap (tick/hash overlay).
    public static void CreateP0Scene()
    {
        var scene = EditorSceneManager.NewScene(NewSceneSetup.DefaultGameObjects, NewSceneMode.Single);
        var go = new GameObject("Bootstrap");
        go.AddComponent<Bootstrap>();
        Directory.CreateDirectory("Assets/Scenes");
        EditorSceneManager.SaveScene(scene, "Assets/Scenes/P0.unity");
        EditorBuildSettings.scenes = new[] { new EditorBuildSettingsScene("Assets/Scenes/P0.unity", true) };
        Debug.Log("PROBE scene saved: Assets/Scenes/P0.unity");
    }
}
