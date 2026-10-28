// WATCH THE MARCH. Not an assertion — a sequence of frames through the walk between two rooms, so
// a person can see whether it reads as walking or as a slide.
//
// `MarchProbe` already proves the arithmetic: the field moves exactly one RoomSpacing, in the right
// direction, and resets at a band edge. None of that says how it LOOKS, and the numbers behind this
// were chosen for how it looks — a 6 s grace holding a 5.2 s crossing at 10 cells/s, picked over a
// 17 s walk and a 26 cells/s slide. A choice made on those grounds has to be checked on them.
//
//   echo probe:MarchShot > unity/ROI/.roi_cmd
using System;
using System.Collections;
using System.IO;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class MarchShot
{
    const string OutDir = "/Users/lunalieneray/Desktop/ROI";
    static int _frame;

    public static void Run()
    {
        SessionState.SetString("ROI_BOOT", "dev");
        SessionState.SetString("ROI_STEAM_PIPE", "0");
        EditorSettings.enterPlayModeOptionsEnabled = true;
        EditorSettings.enterPlayModeOptions =
            EnterPlayModeOptions.DisableDomainReload | EnterPlayModeOptions.DisableSceneReload;
        _frame = 0;
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
        Directory.CreateDirectory(OutDir);
        Roi.Sim.SimSession sim = null;
        for (int i = 0; i < 1800 && sim == null; i++)
        {
            var d = UnityEngine.Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            sim = d != null && d.Session != null && d.Session.IsAlive ? d.Session : null;
            if (sim == null) yield return null;
        }
        if (sim == null) { Debug.LogError("SHOT: no session"); yield break; }

        string from = ZoneOf(sim);
        yield return Shot("march_0_before");

        // Finish the room so the sim swaps it, then photograph the crossing itself.
        sim.Debug("{\"meet_boss_threshold\":{}}");
        for (int i = 0; i < 5400 && ZoneOf(sim) == from; i++)
        {
            if (i % 15 == 0)
            {
                sim.Debug("{\"meet_boss_threshold\":{}}");
                var v = JsonUtility.FromJson<View>(sim.ViewJson());
                if (v?.grid?.entities != null)
                    foreach (var e in v.grid.entities)
                        if (e != null && e.alive && e.team == "Monster")
                            sim.Debug("{\"kill\":{\"entity_id\":" + e.id + "}}");
            }
            yield return null;
        }
        Debug.Log($"SHOT: {from} -> {ZoneOf(sim)}, walking now");

        // Eight frames across the grace window — enough to see the party move and the camera hold
        // them, and few enough to look at without scrubbing.
        for (int k = 1; k <= 8; k++)
        {
            for (int i = 0; i < 45; i++) yield return null;   // ~0.75 s apart
            yield return Shot($"march_{k}");
        }
        yield return Shot("march_9_settled");
        Debug.Log($"SHOT done — {OutDir}");
    }

    static IEnumerator Shot(string name)
    {
        var path = $"{OutDir}/{name}.png";
        ScreenCapture.CaptureScreenshot(path);
        for (int i = 0; i < 8; i++) yield return null;
        Debug.Log("SHOT " + path);
    }

    static string ZoneOf(Roi.Sim.SimSession sim)
    {
        var v = JsonUtility.FromJson<View>(sim.ViewJson());
        return v != null ? v.current_zone_id ?? "" : "";
    }

    [Serializable] class View { public string current_zone_id; public Grid grid; }
    [Serializable] class Grid { public Ent[] entities; }
    [Serializable] class Ent { public long id; public string team; public bool alive; }
}
