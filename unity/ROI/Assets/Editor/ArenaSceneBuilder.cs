// Builds Assets/Scenes/P1_Arena.unity: one GameObject running ArenaView (self-boots the sim and
// renders the HD-2D arena). Editor-only; run headless via RemoteControl (exec:ArenaSceneBuilder.Build)
// or from the menu. Also ArenaSceneBuilder.Probe ticks a headless session and logs entity counts so
// the render path can be checked without opening the Game view.
using System.IO;
using Roi.Arena;
using Roi.Sim;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class ArenaSceneBuilder
{
    [MenuItem("ROI/Build P1 Arena Scene")]
    public static void Build()
    {
        var scene = EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);
        var go = new GameObject("Arena");
        go.AddComponent<ArenaView>();
        Directory.CreateDirectory("Assets/Scenes");
        EditorSceneManager.SaveScene(scene, "Assets/Scenes/P1_Arena.unity");
        EditorBuildSettings.scenes = new[]
        {
            new EditorBuildSettingsScene("Assets/Scenes/P1_Arena.unity", true),
            new EditorBuildSettingsScene("Assets/Scenes/P0.unity", true),
        };
        Debug.Log("ARENA scene saved: Assets/Scenes/P1_Arena.unity");
    }

    // Headless sanity: boot a session, tick, count Player/Monster entities in the view JSON.
    public static void Probe()
    {
        var gd = SimSession.LoadCatalog();
        if (gd == null) { Debug.LogError("ARENA PROBE FAIL: no game_data.json"); return; }
        using var s = SimSession.NewAs(42, gd, "zone_b1_1", "swordsman_t1", "Idlers");
        if (s == null) { Debug.LogError("ARENA PROBE FAIL: roi_new_as null"); return; }
        s.Tick(120);
        var view = s.ViewJson();
        int players = CountTeam(view, "Player"), monsters = CountTeam(view, "Monster");
        int posCount = Occurrences(view, "\"pos\"");
        Debug.Log($"ARENA PROBE players={players} monsters={monsters} pos_fields={posCount} view={view.Length}B");
        if (players < 1 || monsters < 1 || posCount < 2) Debug.LogError("ARENA PROBE FAIL: too few entities");
        else Debug.Log("ARENA PROBE OK");
    }

    static int CountTeam(string json, string team)
    {
        int n = 0, i = 0; var needle = $"\"team\":\"{team}\"";
        while ((i = json.IndexOf(needle, i, System.StringComparison.Ordinal)) >= 0) { n++; i += needle.Length; }
        return n;
    }
    static int Occurrences(string json, string needle)
    {
        int n = 0, i = 0;
        while ((i = json.IndexOf(needle, i, System.StringComparison.Ordinal)) >= 0) { n++; i += needle.Length; }
        return n;
    }
}
