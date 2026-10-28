// THE LOGIN, end to end, through the screen's own controls.
//
// This is the test the whole boot exists to pass: a save produced by a session the login started
// must VERIFY on the server. The failure it is written against is a silent one — a client that
// picks its own seed plays perfectly, looks perfectly correct on screen, and has every save
// rejected as "start snapshot is not anchored" ten seconds later. Nothing on screen says so.
//
// It walks BOTH halves, because they fail differently:
//   FRESH   a new account name -> no stored save -> the founding screen -> `new_game_as` on the
//           server's seed -> the first push must verify.
//   RESUME  return to the login, sign in as the SAME account -> a stored save -> `load_json` ->
//           the next push must verify from the resumed state.
//
// It presses the real buttons and types into the real field. A screenshot would prove the screen
// exists; only the anchor moving proves the server took it.
//
//   echo exec:LoginProbe.Run > unity/ROI/.roi_cmd
using System.Collections;
using System.Collections.Generic;
using TMPro;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.UI;

public static class LoginProbe
{
    const string OutDir = "/tmp";
    static int _frame;
    static string _account;

    public static void Run()
    {
        EditorSettings.enterPlayModeOptionsEnabled = true;
        EditorSettings.enterPlayModeOptions =
            EnterPlayModeOptions.DisableDomainReload | EnterPlayModeOptions.DisableSceneReload;
        // NO door: this is the flow a player takes, and the title screen is the point.
        SessionState.SetString("ROI_BOOT", "");
        // NO STEAM PIPE. This probe signs in BY NAME — it presses the swap past the Steam door on
        // the way — so it has no use for the SDK, and holding a pipe it never reads is how it died:
        // a Steam login on another machine disconnected the client mid-run and the SDK's fatal
        // assert took the whole Editor with it, losing the suite that was running.
        SessionState.SetString("ROI_STEAM_PIPE", "0");
        _account = "unity-login-" + System.DateTime.Now.ToString("MMdd-HHmmss");
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
        if (++_frame < 60) return;
        EditorApplication.update -= Tick;
        // Hosted on OnlineService: it is `DontDestroyOnLoad`, so the coroutine survives the scene
        // reload that returning to the login performs.
        Roi.Net.OnlineService.Instance.Run(Drive());
    }

    static IEnumerator Drive()
    {
        // --- FRESH ACCOUNT ------------------------------------------------------------------
        var title = Object.FindFirstObjectByType<Roi.UI.TitleScreen>();
        if (title == null) { Debug.LogError("LOGIN: no title screen — did the boot land in the scene?"); yield break; }
        yield return Shot("roi_login_1_title");

        if (!Press(title, "TapGate")) { Debug.LogError("LOGIN: no tap gate"); yield break; }
        yield return null;
        ChooseTheNameDoor(title);
        yield return null;
        var field = FindInput(title, "SignIn");
        if (field == null) { Debug.LogError("LOGIN: the sign-in has no name field"); yield break; }
        field.text = _account;
        yield return Shot("roi_login_2_signin");

        if (!Press(title, "Primary_Play")) { Debug.LogError("LOGIN: no Play button"); yield break; }
        Debug.Log($"LOGIN signing in as {_account}");

        // A new account has no save, so the founding screen is the expected answer.
        yield return WaitForCreation(title);
        var panel = Find(title.transform, "Creation");
        if (panel == null || !panel.gameObject.activeSelf)
        {
            Debug.LogError("LOGIN: a fresh account never reached the founding screen — "
                         + StatusOf(title));
            yield break;
        }
        yield return Shot("roi_login_3_create");

        var party = FindInput(title, "Creation");
        if (party != null) party.text = "Probe Party";
        if (!Press(title, "Primary_Begin")) { Debug.LogError("LOGIN: no Begin button"); yield break; }

        yield return WaitForSession("fresh");
        yield return Shot("roi_login_4_game");
        yield return WaitForVerifiedSave();
        if (!_anchorMoved) yield break;

        // --- RESUME -------------------------------------------------------------------------
        var boot = Roi.Sim.SessionBoot.Instance;
        if (boot == null) { Debug.LogError("LOGIN: no boot to return through"); yield break; }
        long tickBefore = (long)Object.FindFirstObjectByType<Roi.Sim.SimDriver>().Tick;
        boot.ReturnToLogin("Returned for the resume half of the probe.");
        for (int i = 0; i < 600 && Object.FindFirstObjectByType<Roi.UI.TitleScreen>() == null; i++)
            yield return null;
        title = Object.FindFirstObjectByType<Roi.UI.TitleScreen>();
        if (title == null) { Debug.LogError("LOGIN: never came back to the title"); yield break; }

        Press(title, "TapGate");
        yield return null;
        ChooseTheNameDoor(title);
        yield return null;
        field = FindInput(title, "SignIn");
        if (field == null) { Debug.LogError("LOGIN: the second sign-in has no name field"); yield break; }
        field.text = _account;                     // the SAME account: there is a save now
        Press(title, "Primary_Play");
        Debug.Log($"LOGIN signing back in as {_account} (expecting a resume, not a founding)");

        yield return WaitForSession("resume");
        var driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        var sim = driver != null ? driver.Session : null;
        if (sim == null) yield break;
        // The proof it RESUMED rather than started again: the state came back at the tick the
        // server had stored, not at zero.
        var view = sim.ViewJson();
        Debug.Log($"LOGIN resumed — session tick from the save, local tick {driver.Tick}, "
                + $"hash {sim.StateHash()}, view {view.Length} bytes (was at tick {tickBefore} before)");
        yield return Shot("roi_login_5_resume");
        yield return WaitForVerifiedSave();

        Debug.Log(_anchorMoved
            ? "LOGIN OK — both a founded party and a resumed save verified on the server."
            : "LOGIN: the resumed session's save never settled.");
        EditorApplication.isPlaying = false;
    }

    // --- steps -------------------------------------------------------------------------------

    static IEnumerator WaitForCreation(Roi.UI.TitleScreen title)
    {
        for (int i = 0; i < 1800; i++)
        {
            var p = Find(title.transform, "Creation");
            if (p != null && p.gameObject.activeSelf) yield break;
            if (title == null) yield break;
            yield return null;
        }
    }

    static IEnumerator WaitForSession(string what)
    {
        for (int i = 0; i < 1800; i++)
        {
            var d = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            if (d != null && d.Session != null && d.Session.IsAlive)
            {
                Debug.Log($"LOGIN {what}: session running, hash {d.Session.StateHash()}");
                yield break;
            }
            yield return null;
        }
        Debug.LogError($"LOGIN {what}: no session ever started");
    }

    static bool _anchorMoved;

    /// THE ASSERTION. `ReplaySpan` is the distance from the session's anchor; the boot only moves
    /// that anchor when the server has VERIFIED the segment. So a span that grows and then falls is
    /// the server accepting the save — the one thing a screenshot cannot show.
    static IEnumerator WaitForVerifiedSave()
    {
        _anchorMoved = false;
        var driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        var sim = driver != null ? driver.Session : null;
        if (sim == null) { Debug.LogError("LOGIN: no session to watch"); yield break; }
        long peak = 0;
        for (int i = 0; i < 3600; i++)                    // up to a minute of play
        {
            long span = sim.ReplaySpan();
            if (span > peak) peak = span;
            if (peak > 0 && span < peak)
            {
                _anchorMoved = true;
                Debug.Log($"LOGIN SAVE VERIFIED — the anchor moved (span {peak} -> {span}).");
                yield break;
            }
            yield return null;
        }
        Debug.LogError($"LOGIN: no save ever settled (span peaked at {peak}) — "
                     + "check the Editor log for the server's rejection reason.");
    }

    // --- poking real controls ------------------------------------------------------------------

    /// THE NAME DOOR, chosen the way a player chooses it. This probe types a name and expects the
    /// server to know it — which only happens on the dev door. On a machine with Steam the panel
    /// opens on the STEAM door instead, and Play there sends a ticket and ignores the field
    /// entirely: the typed name went nowhere, no founding screen ever came, and the probe sat until
    /// its timeout with nothing in the log to say why. Pressing the swap is not a workaround; it is
    /// the step the screen now requires of anyone signing in by name.
    static void ChooseTheNameDoor(Roi.UI.TitleScreen title)
    {
        if (!Roi.Net.SteamAuth.Available) return;      // the panel is already the name door
        foreach (var b in title.GetComponentsInChildren<Button>(true))
            if (b.name == "RouteSwap" && b.gameObject.activeInHierarchy)
            {
                b.onClick.Invoke();
                Debug.Log("LOGIN took the name door past Steam");
                return;
            }
        Debug.LogError("LOGIN: Steam is available and the panel offers no way past it — "
                     + "this probe signs in by NAME and cannot do that through the Steam door.");
    }

    static bool Press(Roi.UI.TitleScreen title, string namePrefix)
    {
        foreach (var b in title.GetComponentsInChildren<Button>(true))
        {
            if (!b.name.StartsWith(namePrefix, System.StringComparison.Ordinal)) continue;
            if (!b.gameObject.activeInHierarchy || !b.interactable) continue;
            b.onClick.Invoke();
            return true;
        }
        return false;
    }

    static TMP_InputField FindInput(Roi.UI.TitleScreen title, string underPanel)
    {
        var panel = Find(title.transform, underPanel);
        return panel != null ? panel.GetComponentInChildren<TMP_InputField>(true) : null;
    }

    static Transform Find(Transform root, string name)
    {
        foreach (var t in root.GetComponentsInChildren<Transform>(true))
            if (t.name == name) return t;
        return null;
    }

    static string StatusOf(Roi.UI.TitleScreen title)
    {
        var t = Find(title.transform, "Status");
        return t != null ? t.GetComponent<TextMeshProUGUI>().text : "(no status)";
    }

    static IEnumerator Shot(string name)
    {
        for (int i = 0; i < 4; i++) yield return null;
        var path = $"{OutDir}/{name}.png";
        ScreenCapture.CaptureScreenshot(path);
        for (int i = 0; i < 12; i++) yield return null;
        Debug.Log("LOGIN shot " + path);
    }
}
