// THE LOCAL SLOT SURVIVES A RESTART, which is the only thing it was added to do.
//
// Asserting "a file appeared" would pass on a file full of nothing. What has to be true is that a
// SECOND session, started from scratch, comes back holding the FIRST one's state — so this runs the
// dev door twice in one play-mode entry and compares the sim's own state hash across the gap.
//
//   echo probe:DevSaveProbe > unity/ROI/.roi_cmd
using System;
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class DevSaveProbe
{
    static int _frame;

    public static void Run()
    {
        SessionState.SetString("ROI_BOOT", "dev");
        SessionState.SetString("ROI_STEAM_PIPE", "0");
        EditorSettings.enterPlayModeOptionsEnabled = true;
        EditorSettings.enterPlayModeOptions =
            EnterPlayModeOptions.DisableDomainReload | EnterPlayModeOptions.DisableSceneReload;
        _frame = 0;
        // A slot left by an earlier run would make the first half pass without proving anything.
        Roi.Sim.DevSaveStore.Clear();
        if (!EditorApplication.isPlaying)
        {
            EditorSceneManager.OpenScene(ProbeScene.Path, OpenSceneMode.Single);
            EditorApplication.playModeStateChanged += OnState;
            EditorApplication.EnterPlaymode();
        }
        else EditorApplication.update += Tick;
    }

    static void OnState(PlayModeStateChange s)
    {
        if (s != PlayModeStateChange.EnteredPlayMode) return;
        EditorApplication.playModeStateChanged -= OnState;
        EditorApplication.update += Tick;
    }

    static void Tick()
    {
        if (++_frame < 150) return;
        EditorApplication.update -= Tick;
        Roi.Net.OnlineService.Instance.Run(ToEnd(Drive()));
    }

    static IEnumerator ToEnd(IEnumerator inner) { yield return inner; EditorApplication.isPlaying = false; }

    static IEnumerator Drive()
    {
        var boot = Roi.Sim.SessionBoot.Instance;
        var driver = UnityEngine.Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        var sim = driver != null ? driver.Session : null;
        for (int i = 0; i < 900 && (sim == null || !sim.IsAlive); i++)
        {
            yield return null;
            driver = UnityEngine.Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            sim = driver != null ? driver.Session : null;
        }
        if (sim == null) { Debug.LogError("DEVSAVE: no local session"); yield break; }

        // Let it play far enough that a resumed state is plainly not a fresh one.
        for (int i = 0; i < 600; i++) yield return null;
        long tick = (long)driver.Tick;
        string hash = sim.StateHash();
        Report("the session ran before saving", tick > 0, $"tick {tick}");

        // The autosave writes on its own timer; this is the same call it makes.
        Roi.Sim.DevSaveStore.Write(sim.SaveJson());
        var stored = Roi.Sim.DevSaveStore.Read();
        Report("the slot holds a save", !string.IsNullOrEmpty(stored),
               stored == null ? "nothing on disk" : $"{stored.Length} bytes at {Roi.Sim.DevSaveStore.Where}");
        if (string.IsNullOrEmpty(stored)) yield break;

        // THE ASSERTION: a session built from the slot is the same state, not a new game.
        var again = Roi.Sim.SimSession.Load(stored, Roi.Sim.SimSession.LoadCatalog());
        Report("a session loaded from the slot is alive", again != null && again.IsAlive, "");
        if (again == null || !again.IsAlive) yield break;
        Report("it is the SAME state, not a fresh one", again.StateHash() == hash,
               $"{hash} vs {again.StateHash()}");
        again.Dispose();

        Debug.Log("DEVSAVE done");
    }

    static void Report(string what, bool ok, string detail)
    {
        if (ok) Debug.Log($"DEVSAVE OK — {what}: {detail}");
        else Debug.LogError($"DEVSAVE FAILED — {what}: {detail}");
    }
}
