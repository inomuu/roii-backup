// The three SERVER LADDER screens, driven end to end: Pass, Commander, Outpost.
//
// Each one is a claim button over somebody else's ledger, and each fails the same invisible way —
// the screen draws, the button presses, and the server refuses for a reason nothing on screen
// shows. So this presses the real controls and reports the STATUS LINE each screen ends up with,
// which is where a refusal lands.
//
// It boots ONLINE (the door in SessionBoot) so the session runs on the server's seed: the pass and
// the commander claim both mint rewards the client applies to the sim, and those only settle in a
// save that anchors.
//
//   echo exec:ScreenProbe.Run > unity/ROI/.roi_cmd
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Networking;
using UnityEngine.UI;

public static class ScreenProbe
{
    static int _frame;

    public static void Run()
    {
        if (!System.Uri.TryCreate(Roi.Net.OnlineService.ServerUrl, System.UriKind.Absolute, out var endpoint)
            || !endpoint.IsLoopback)
        {
            Debug.LogError("ScreenProbe requires ROI_SERVER pointing to a local test server.");
            return;
        }
        // ONLINE from the boot: these screens are the server's, and a dev session has no account.
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
        if (++_frame < 120) return;
        var bar = Object.FindFirstObjectByType<Roi.UI.HudBar>();
        if (bar == null)
        {
            if (_frame < 3600) return;
            EditorApplication.update -= Tick;
            Debug.LogError("SCREENS: no bar — the login never produced a session");
            return;
        }
        EditorApplication.update -= Tick;
        Roi.Net.OnlineService.Instance.Run(Drive(bar));
    }

    static IEnumerator Drive(Roi.UI.HudBar bar)
    {
        // A local server starts with no season, and a Pass screen with no season proves nothing.
        // This is the dev endpoint that exists for exactly this, and `slot_cost` is dropped to 1 so
        // the account is already several levels in rather than 6.6 hours short of the first.
        yield return DevSeason();

        // No verb: the store has nothing to press here — buying needs the platform overlay this
        // client has no SDK for — so this checks the catalog drew and the wallet read.
        yield return Screen(bar, "Events", "Collect");
        yield return Screen(bar, "Crystal", null);
        yield return Screen(bar, "Pass", "Claim");
        yield return Screen(bar, "Commander", "Accept");
        yield return Screen(bar, "Outpost", "Collect");

        Debug.Log("SCREENS done");
        EditorApplication.isPlaying = false;
    }

    /// Open the screen, wait for its fetch, press the button whose label starts with `verb`, and
    /// report the status line before and after. A disabled button is reported, not pressed — "there
    /// was nothing to claim" and "the claim was refused" are different answers.
    static IEnumerator Screen(Roi.UI.HudBar bar, string name, string verb)
    {
        bar.CloseAll();
        bar.Toggle(name);
        for (int i = 0; i < 600 && StatusOf(bar) == null; i++) yield return null;
        for (int i = 0; i < 180; i++) yield return null;      // let the fetch land
        Debug.Log($"SCREENS {name}: {StatusOf(bar)}");

        if (verb == null)
        {
            int rows = 0;
            foreach (var t in bar.GetComponentsInChildren<TMPro.TextMeshProUGUI>(true))
                if (t.gameObject.name == "A" && t.gameObject.activeInHierarchy) rows++;
            Debug.Log($"SCREENS {name}: {rows} rows drawn");
            ScreenCapture.CaptureScreenshot($"/tmp/roi_{name.ToLower()}.png");
            for (int i = 0; i < 20; i++) yield return null;
            yield break;
        }
        var btn = FindPrefix(bar, verb);
        if (btn == null)
        {
            Debug.Log($"SCREENS {name}: no '{verb}' button drawn (nothing claimable, or not running)");
        }
        else if (!btn.interactable)
        {
            Debug.Log($"SCREENS {name}: '{Label(btn)}' is disabled — nothing to take");
        }
        else
        {
            Debug.Log($"SCREENS {name}: pressing '{Label(btn)}'");
            btn.onClick.Invoke();
            for (int i = 0; i < 300; i++) yield return null;
            Debug.Log($"SCREENS {name} after: {StatusOf(bar)}");
        }
        ScreenCapture.CaptureScreenshot($"/tmp/roi_{name.ToLower()}.png");
        for (int i = 0; i < 20; i++) yield return null;
    }

    static IEnumerator DevSeason()
    {
        var url = Roi.Net.OnlineService.ServerUrl + "/dev/bp-season";
        using var req = new UnityWebRequest(url, "POST")
        {
            uploadHandler = new UploadHandlerRaw(System.Text.Encoding.UTF8.GetBytes("{\"days\":30,\"slot_cost\":1}")),
            downloadHandler = new DownloadHandlerBuffer(),
        };
        req.SetRequestHeader("Content-Type", "application/json");
        req.SetRequestHeader("X-Dev-Key", Roi.Net.OnlineService.DefaultDevKey);
        yield return req.SendWebRequest();
        Debug.Log($"SCREENS dev season: {req.responseCode} {req.downloadHandler.text}");
    }

    static string Label(Button b)
    {
        var t = b.GetComponentInChildren<TMPro.TextMeshProUGUI>();
        return t != null ? t.text : b.name;
    }

    static string StatusOf(Roi.UI.HudBar bar)
    {
        foreach (var t in bar.GetComponentsInChildren<TMPro.TextMeshProUGUI>(true))
            if (t.gameObject.name == "Status" && t.gameObject.activeInHierarchy) return t.text;
        return null;
    }

    static Button FindPrefix(Roi.UI.HudBar bar, string label)
    {
        foreach (var b in bar.GetComponentsInChildren<Button>(true))
        {
            if (!b.gameObject.activeInHierarchy) continue;
            var t = b.GetComponentInChildren<TMPro.TextMeshProUGUI>();
            if (t != null && t.text.StartsWith(label, System.StringComparison.Ordinal)) return b;
        }
        return null;
    }
}
