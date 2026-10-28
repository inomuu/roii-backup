// TELEMETRY, sent and ACCEPTED.
//
// The whole value of this layer is that the server's allow-list took the name. A counter the
// server rejects is not an error anywhere — it lands on `client.unknown` and reads zero forever,
// which looks exactly like a screen nobody opens. So the assertion is the server's own count.
//
// It flushes BY HAND. A send is a minute away by design, and a probe that waited for it would be
// measuring the clock rather than the wire.
//
//   echo exec:TelemetryProbe.Run > unity/ROI/.roi_cmd
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class TelemetryProbe
{
    static int _frame;

    public static void Run()
    {
        SessionState.SetString("ROI_BOOT", "online");
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
            if (_frame < 3600) return;
            EditorApplication.update -= Tick;
            Debug.LogError("TELEM: no bar — the login never produced a session");
            return;
        }
        EditorApplication.update -= Tick;
        Roi.Net.OnlineService.Instance.Run(ToEnd(Drive(bar)));
    }

    static IEnumerator ToEnd(IEnumerator inner)
    {
        yield return inner;
        EditorApplication.isPlaying = false;
    }

    static IEnumerator Drive(Roi.UI.HudBar bar)
    {
        var t = Object.FindFirstObjectByType<Roi.Net.Telemetry>(FindObjectsInactive.Include);
        Report("the session brought telemetry with it", t != null, t == null ? "none" : "attached");
        if (t == null) yield break;

        // OPENING A SCREEN is what this exists to count, so the probe opens one rather than
        // calling `Note` directly — a counter fired by hand proves the wire and not the wiring.
        int before = t.Buffered;
        bar.CloseAll();
        bar.Toggle("Shop");
        for (int i = 0; i < 60; i++) yield return null;
        bar.CloseAll();
        bar.Toggle("Options");
        for (int i = 0; i < 60; i++) yield return null;
        Report("opening a screen records it", t.Buffered >= before + 2,
               $"buffered {before} -> {t.Buffered}");

        // AND THE SERVER TOOK THEM. `accepted` is the only proof the names are on the allow-list;
        // a rejected one is invisible from this side forever.
        int accepted = -1, rejected = -1;
        var client = Roi.Net.OnlineService.Instance.Client;
        var batch = new[] { "session.start", "screen.shop", "screen.options" };
        yield return client.Telemetry(batch, (a, r) => { accepted = a; rejected = r; });
        Report("the server accepted every name", accepted == batch.Length && rejected == 0,
               $"accepted {accepted}, rejected {rejected} of {batch.Length}");

        // A NAME THE ALLOW-LIST DOES NOT CARRY must be refused, not quietly counted — that refusal
        // is what stops the counter space becoming free-form.
        int badAccepted = -1, badRejected = -1;
        yield return client.Telemetry(new[] { "screen.not_a_real_screen" },
                                      (a, r) => { badAccepted = a; badRejected = r; });
        Report("an unknown name is rejected", badAccepted == 0 && badRejected == 1,
               $"accepted {badAccepted}, rejected {badRejected}");

        yield return t.FlushNow();
        Report("the buffer drains on a flush", t.Buffered == 0, $"{t.Buffered} left");

        Debug.Log("TELEM done");
    }

    static void Report(string what, bool ok, string detail)
    {
        if (ok) Debug.Log($"TELEM OK — {what}: {detail}");
        else Debug.LogError($"TELEM FAILED — {what}: {detail}");
    }
}
