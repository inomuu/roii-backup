// SOMETHING IS ACTUALLY IN THE AIR.
//
// A projectile is a TRANSIENT: it exists for the ticks between the shot and the hit and then the
// sim removes it, so a screenshot taken a frame late shows an empty field and proves nothing. The
// assertion is that the renderer held one WHILE the sim had one — and let it go when the sim did.
//
// The party has to be shooting for that to happen, so the probe recruits an archer and waits. Band
// 1 spawns on an interval; a few hundred ticks is enough for a ranged member to loose something.
//
//   echo exec:ProjectileProbe.Run > unity/ROI/.roi_cmd
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class ProjectileProbe
{
    static int _frame;

    public static void Run()
    {
        SessionState.SetString("ROI_BOOT", "dev");
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
        var bar = Object.FindFirstObjectByType<Roi.UI.HudBar>();
        if (bar == null)
        {
            if (_frame < 1800) return;
            EditorApplication.update -= Tick;
            Debug.LogError("SHOT: no bar");
            return;
        }
        EditorApplication.update -= Tick;
        UnityEditor.PlayModeWindow.SetCustomRenderingResolution(2560, 1440, "ROI capture");
        Roi.Net.OnlineService.Instance.Run(ToEnd(Drive()));
    }

    static IEnumerator ToEnd(IEnumerator inner)
    {
        yield return inner;
        EditorApplication.isPlaying = false;
    }

    static IEnumerator Drive()
    {
        var driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        var sim = driver != null ? driver.Session : null;
        var shots = Object.FindFirstObjectByType<Roi.Arena.ArenaProjectiles>(FindObjectsInactive.Include);
        if (sim == null || shots == null) { Debug.LogError("SHOT: no session or no projectile layer"); yield break; }

        // AN ARCHER, because the starting class is melee and a melee party never fires anything.
        sim.Debug("{\"give_currency\":{\"gil\":100000,\"soul\":0}}");
        // `BuyCharacter`: recruiting is gated on the lead's LEVEL and this lead is Lv1.
        sim.ApplyInput("{\"BuyCharacter\":{\"unit_def\":\"archer_t1\"}}");
        for (int i = 0; i < 10; i++) yield return null;
        foreach (var id in BenchedIds(sim)) sim.ApplyInput("{\"DeployCharacter\":{\"char_id\":" + id + "}}");
        for (int i = 0; i < 30; i++) yield return null;

        // Wait for the sim to have one in flight AND the renderer to be holding it. Reading them
        // together is the point: either alone would pass while the other did nothing.
        Debug.Log("SHOT party: " + PartyClasses(sim));
        int peakSim = 0, peakDrawn = 0;
        for (int i = 0; i < 5400; i++)
        {
            int inSim = InFlight(sim);
            peakSim = Mathf.Max(peakSim, inSim);
            peakDrawn = Mathf.Max(peakDrawn, shots.Count);
            if (inSim > 0 && shots.Count > 0)
            {
                // A few frames IN, not on the frame it appeared: at k=0 the bolt is still inside
                // the caster and the picture shows an archer with nothing in front of them.
                for (int f = 0; f < 4 && shots.Count > 0; f++) yield return null;
                ScreenCapture.CaptureScreenshot("/tmp/roi_projectile.png");
                break;
            }
            yield return null;
        }
        Report("the sim fired something", peakSim > 0, $"{peakSim} in flight at once");
        Report("the renderer drew it", peakDrawn > 0, $"{peakDrawn} on screen at once");

        // AND LET GO. A layer that spawns but never destroys looks identical for one shot and
        // buries the field after a minute.
        for (int i = 0; i < 900 && shots.Count > 0; i++) yield return null;
        Report("it is cleaned up on arrival", shots.Count == 0, $"{shots.Count} left in the air");

        for (int i = 0; i < 20; i++) yield return null;
        Debug.Log("SHOT done");
    }

    static void Report(string what, bool ok, string detail)
    {
        if (ok) Debug.Log($"SHOT OK — {what}: {detail}");
        else Debug.LogError($"SHOT FAILED — {what}: {detail}");
    }

    static string PartyClasses(Roi.Sim.SimSession sim)
    {
        var j = sim.ViewJson();
        var sb = new System.Text.StringBuilder();
        int i = 0;
        while ((i = j.IndexOf("\"unit_def\":\"", i, System.StringComparison.Ordinal)) >= 0)
        {
            i += 12;
            int end = j.IndexOf('"', i);
            sb.Append(j.Substring(i, end - i)).Append(' ');
            i = end;
        }
        var pj = j.IndexOf("\"projectiles\"", System.StringComparison.Ordinal);
        sb.Append("| projectiles key ").Append(pj >= 0 ? "present" : "MISSING");
        if (pj >= 0) sb.Append(": ").Append(j.Substring(pj, System.Math.Min(80, j.Length - pj)));
        return sb.ToString();
    }

    static int InFlight(Roi.Sim.SimSession sim)
    {
        var j = sim.ViewJson();
        int i = j.IndexOf("\"projectiles\":[", System.StringComparison.Ordinal);
        if (i < 0) return 0;
        int n = 0, end = j.IndexOf(']', i);
        for (int k = i; k >= 0 && k < end; )
        {
            k = j.IndexOf("\"arrive_at_tick\"", k, System.StringComparison.Ordinal);
            if (k < 0 || k > end) break;
            n++; k += 16;
        }
        return n;
    }

    static System.Collections.Generic.List<int> BenchedIds(Roi.Sim.SimSession sim)
    {
        var outp = new System.Collections.Generic.List<int>();
        var j = sim.QueryJson("roster");
        int i = 0;
        while ((i = j.IndexOf("\"id\":", i, System.StringComparison.Ordinal)) >= 0)
        {
            i += 5;
            int end = i;
            while (end < j.Length && char.IsDigit(j[end])) end++;
            if (int.TryParse(j.Substring(i, end - i), out var v)) outp.Add(v);
        }
        return outp;
    }
}
