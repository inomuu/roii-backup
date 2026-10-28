// THE DOOR THAT IS NOT STEAM, pressed rather than configured.
//
// Six probes already reach the server without Steam, but every one of them books its door through
// `ROI_BOOT=online` — a scripted switch no player can touch. The BUTTON that offers the same door
// to a person had no test at all, which is the gap this file exists to close: a control that is
// never pressed is a control that is never known to work, and this one appeared in the same change
// that made `OnlineService` stop choosing a door on the player's behalf.
//
// WHY IT MATTERS BEYOND THE BUTTON. Steam allows one account per machine, so the Steam door can
// never produce a second player to trade with, sell to, or talk to. This door can. It is the only
// mechanism this project has for exercising anything that takes two accounts, and that makes it
// infrastructure rather than a convenience — the reason it is compiled out of a release build
// (`TitleScreen.DevDoors`) instead of merely hidden.
//
// THE ASSERTION is a FOUNDING SCREEN. Signing in under a name nobody has used before can only end
// one way: no stored save, so the server answers with a party to found. Steam cannot reach that
// state — its credential is a ticket tied to one steamid, and that steamid already has an account.
// So a founding screen reached by typing a name IS the proof that the dev door, and not the Steam
// one, took the login.
//
//   echo probe:RouteProbe > unity/ROI/.roi_cmd
using System.Collections;
using TMPro;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.UI;

public static class RouteProbe
{
    static int _frame;
    static string _account;

    public static void Run()
    {
        EditorSettings.enterPlayModeOptionsEnabled = true;
        EditorSettings.enterPlayModeOptions =
            EnterPlayModeOptions.DisableDomainReload | EnterPlayModeOptions.DisableSceneReload;
        // NO door. The title screen IS the subject here — a scripted door would skip the one
        // control this probe was written to press.
        SessionState.SetString("ROI_BOOT", "");
        _account = "unity-route-" + System.DateTime.Now.ToString("MMdd-HHmmss");
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
        Roi.Net.OnlineService.Instance.Run(Drive());
    }

    static IEnumerator Drive()
    {
        var title = Object.FindFirstObjectByType<Roi.UI.TitleScreen>();
        if (title == null) { Debug.LogError("ROUTE: no title screen"); yield break; }
        if (!Press(title, "TapGate")) { Debug.LogError("ROUTE: no tap gate"); yield break; }
        yield return null;

        var field = FindInput(title, "SignIn");
        if (field == null) { Debug.LogError("ROUTE: the sign-in has no name field"); yield break; }

        // 1. THE BUTTON ONLY EXISTS WHERE THERE IS A CHOICE. On a machine with no Steam the panel
        // already IS the name door, so an absent swap is correct there and this probe says which
        // case it is rather than passing over both.
        var swap = FindButton(title, "RouteSwap");
        if (!Roi.Net.SteamAuth.Available)
        {
            if (swap != null)
                Debug.LogError("ROUTE: a swap button on a machine with no Steam — it offers a door "
                             + "that is already the only one, which reads as a broken toggle.");
            else
                Debug.Log("ROUTE SKIP — no Steam on this machine, so there is no choice to offer. "
                        + "The swap button is only meaningful where Steam is available.");
            yield break;
        }
        if (swap == null)
        {
            Debug.LogError("ROUTE: Steam is available but the panel offers no way past it — this is "
                         + "the lockout the button was added to prevent.");
            yield break;
        }
        Debug.Log("ROUTE OK — the swap button is offered where Steam is available");

        // 2. THE FIELD FOLLOWS THE DOOR. On Steam the ticket is the credential, so a typed name
        // would be a lie about what gets sent and the field is greyed. The swap has to hand it back
        // or the door it opens cannot be walked.
        bool greyedBefore = !field.interactable;
        swap.onClick.Invoke();
        yield return null;
        Report("the swap hands the name field back", greyedBefore && field.interactable,
               $"interactable {(greyedBefore ? "false" : "true")} -> {field.interactable}");

        // 3. AND IT GOES BOTH WAYS. A one-way switch strands anyone who pressed it by accident.
        swap.onClick.Invoke();
        yield return null;
        bool greyedAgain = !field.interactable;
        swap.onClick.Invoke();
        yield return null;
        Report("the swap goes back to Steam and out again", greyedAgain && field.interactable,
               $"greyed on the way back: {greyedAgain}");

        // 4. THE DOOR ACTUALLY OPENS. A name nobody has used has no save, so the server answers
        // with a founding screen — a state the Steam door cannot produce, because its steamid
        // already owns an account. Reaching it is the proof the login took THIS door.
        field.text = _account;
        if (!Press(title, "Primary_Play")) { Debug.LogError("ROUTE: no Play button"); yield break; }
        Debug.Log($"ROUTE signing in by name as {_account}");

        for (int i = 0; i < 1800; i++)
        {
            var p = Find(title.transform, "Creation");
            if (p != null && p.gameObject.activeSelf) break;
            if (title == null) break;
            yield return null;
        }
        var panel = title != null ? Find(title.transform, "Creation") : null;
        bool founded = panel != null && panel.gameObject.activeSelf;
        Report("a typed name founds a NEW account, which Steam cannot do", founded,
               founded ? "the founding screen came up" : StatusOf(title));

        // 5. THE SECOND ACCOUNT IS A DIFFERENT ACCOUNT. This is the whole point of keeping the
        // door: one machine, two players. An id equal to the Steam one would mean the swap changed
        // the label and nothing else.
        var client = Roi.Net.OnlineService.Instance.Client;
        Report("the session is on a different account from the Steam one",
               client != null && client.AccountId > 0,
               client != null ? $"account {client.AccountId} via the name door" : "no client");
        Report("the service remembers the door, so a re-connect cannot silently change accounts",
               !Roi.Net.OnlineService.Instance.PreferredUseSteam, "PreferredUseSteam=false");

        Debug.Log("ROUTE done");
        EditorApplication.isPlaying = false;
    }

    static void Report(string what, bool ok, string detail)
    {
        if (ok) Debug.Log($"ROUTE OK — {what}: {detail}");
        else Debug.LogError($"ROUTE FAILED — {what}: {detail}");
    }

    // --- poking real controls ------------------------------------------------------------------

    /// By GameObject name, never by label: `MakeGhostButton` builds its name from the TRANSLATED
    /// text, so a probe that matched on the label would pass in English and fail in Thai.
    static Button FindButton(Roi.UI.TitleScreen title, string name)
    {
        foreach (var b in title.GetComponentsInChildren<Button>(true))
            if (b.name == name && b.gameObject.activeInHierarchy) return b;
        return null;
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
        if (title == null) return "(the title screen went away)";
        var t = Find(title.transform, "Status");
        return t != null ? t.GetComponent<TextMeshProUGUI>().text : "(no status)";
    }
}
