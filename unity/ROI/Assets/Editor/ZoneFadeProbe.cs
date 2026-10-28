// THE BAND-BOUNDARY FADE, driven by hand.
//
// It cannot be waited for: a band is three rooms and a boss, which is many minutes of real
// fighting, and the fade's whole subject is a 2.5-second window inside that. So the probe feeds
// the view frames the sim would produce and asserts the veil moves — and, more importantly, that
// it moves ONLY at a band boundary.
//
// The three cases are the three bugs this shape has: a fade at every room (which would hide the
// continuous march between them), a fade on LOGIN (a save taken inside a clear window resumes with
// `zone_cleared` already true, and unknown→true is not an edge), and a veil that never lifts.
//
//   echo exec:ZoneFadeProbe.Run > unity/ROI/.roi_cmd
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class ZoneFadeProbe
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
        var fade = Object.FindFirstObjectByType<Roi.Arena.ZoneFade>(FindObjectsInactive.Include);
        if (fade == null)
        {
            if (_frame < 1800) return;
            EditorApplication.update -= Tick;
            Debug.LogError("FADE: the veil was never built");
            return;
        }
        EditorApplication.update -= Tick;
        Roi.Net.OnlineService.Instance.Run(ToEnd(Drive(fade)));
    }

    static IEnumerator ToEnd(IEnumerator inner)
    {
        yield return inner;
        EditorApplication.isPlaying = false;
    }

    static string Frame(string zone, bool cleared, bool bandEnd)
        => "{\"current_zone_id\":\"" + zone + "\",\"zone_spawn\":{\"zone_cleared\":"
         + (cleared ? "true" : "false") + ",\"is_instance_end\":" + (bandEnd ? "true" : "false") + "}}";

    static IEnumerator Drive(Roi.Arena.ZoneFade fade)
    {
        // The live arena is feeding this thing every tick, so a hand-fed frame would be overwritten
        // before it could be read. Test the veil on its OWN instance, off the arena's feed.
        var solo = Roi.Arena.ZoneFade.Create(null);
        yield return null;

        // 1. LOGIN INTO A CLEAR WINDOW. The first frame seeds the edge and must do nothing.
        solo.OnView(Frame("zone_b1_3", true, true));
        for (int i = 0; i < 120; i++) yield return null;
        Report("a login inside a clear window does not fade", solo.Alpha < 0.02f, $"alpha {solo.Alpha:0.00}");

        // 2. A ROOM ending mid-band. Not a boundary, so nothing happens.
        solo.OnView(Frame("zone_b1_3", false, false));
        solo.OnView(Frame("zone_b1_3", true, false));
        for (int i = 0; i < 150; i++) yield return null;
        Report("a room inside a band does not fade", solo.Alpha < 0.02f, $"alpha {solo.Alpha:0.00}");

        // 3. THE BAND BOUNDARY. Fades to black across the sim's ~2.5 s clear window.
        solo.OnView(Frame("zone_b1_3", false, true));
        solo.OnView(Frame("zone_b1_3", true, true));
        // WAITED FOR, not counted. The fade is 1.0 s of delay and 1.3 s of tween in REAL time, and
        // a frame budget only measures that correctly at a frame rate the Editor does not promise
        // — the first version of this line read 0.72 and called a working fade a failure.
        yield return Until(() => solo.Alpha > 0.9f, 8f);
        Report("a band boundary fades to black", solo.Alpha > 0.9f, $"alpha {solo.Alpha:0.00}");

        // 4. THE SWAP LANDS. The veil lifts, whatever the destination — so it can never stick.
        solo.OnView(Frame("zone_b2_1", false, false));
        yield return Until(() => solo.Alpha < 0.02f, 8f);
        Report("the new room reveals", solo.Alpha < 0.02f, $"alpha {solo.Alpha:0.00}");

        Object.Destroy(solo.gameObject);
        Debug.Log("FADE done");
    }

    /// Wait for a condition, capped in SECONDS. The two negative cases below still hold a fixed
    /// number of frames on purpose — there is nothing to wait for there, and the point is that
    /// time passing changes nothing.
    static IEnumerator Until(System.Func<bool> done, float seconds)
    {
        float t0 = Time.realtimeSinceStartup;
        while (!done() && Time.realtimeSinceStartup - t0 < seconds) yield return null;
    }

    static void Report(string what, bool ok, string detail)
    {
        if (ok) Debug.Log($"FADE OK — {what}: {detail}");
        else Debug.LogError($"FADE FAILED — {what}: {detail}");
    }
}
