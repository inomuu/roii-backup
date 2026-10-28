// Batchmode counterpart of the Godot ARENA_DUMP pass: enters Play mode on the P1 arena, lets
// ArenaView write its per-actor render state as CSV at the requested sim ticks, then exits. The
// pair is diffed by tools/compare_arena.py — the port is checked as NUMBERS, because a screenshot
// pair can only say "looks close" and cannot say "the foot anchor is 0.15 cells off".
//   ARENA_DUMP_TICKS="40,80,160" ARENA_DUMP_OUT=/tmp/unity_arena.csv \
//     Unity -batchmode -projectPath unity/ROI -executeMethod ArenaDump.Run -logFile <f>   (NO -quit)
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class ArenaDump
{
    // Batchmode runs the play loop as fast as it can, so a FRAME budget is not a time budget:
    // 3000 frames went by in a few seconds and the sim (10 Hz wall clock) had not reached the
    // requested tick yet. Bound the wait in seconds, which is what "tick 160" actually costs.
    const double TimeoutSec = 180.0;
    static double _t0;
    static string _out;

    public static void Run()
    {
        _out = System.Environment.GetEnvironmentVariable("ARENA_DUMP_OUT");
        if (string.IsNullOrEmpty(_out)) _out = "/tmp/unity_arena.csv";
        if (System.IO.File.Exists(_out)) System.IO.File.Delete(_out);
        // The cross-engine dump runs the arena's own seed — no login in the way.
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
        if (s != PlayModeStateChange.EnteredPlayMode) return;
        _t0 = EditorApplication.timeSinceStartup;
        EditorApplication.update += Tick;
        Debug.Log("ARENA DUMP entered play mode");
    }

    static void Tick()
    {
        if (!EditorApplication.isPlaying) return;
        if (System.IO.File.Exists(_out))
        {
            Debug.Log("ARENA DUMP done: " + _out);
            EditorApplication.update -= Tick;
            EditorApplication.Exit(0);
            return;
        }
        if (EditorApplication.timeSinceStartup - _t0 > TimeoutSec)
        {
            Debug.LogError("ARENA DUMP timed out with no CSV — is ARENA_DUMP_TICKS set?");
            EditorApplication.update -= Tick;
            EditorApplication.Exit(1);
        }
    }
}
