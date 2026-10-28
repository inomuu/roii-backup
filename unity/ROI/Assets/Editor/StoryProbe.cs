// STORY MODE, played. Three things are under test and only the third is about the screen:
//
//   1. THE FILES PARSE. Both clients read the same `.dialogue` sources, so a Unity-side parser
//      that disagreed about one of them would be a second dialect of the format. Zero errors on
//      every mission file, or the port is not a port.
//   2. THE SANDBOX IS SEALED. A story stage runs a SECOND sim that hands itself a hundred million
//      Gil and a level-50 cast. If any of that could reach the live session, the whole design is
//      pointless — so the probe reads the LIVE purse and the LIVE party either side of a mission
//      and requires them not to have moved.
//   3. THE SCRIPT WALKS. Lines advance, a `@ battle` hands off and comes back, a choice is
//      answered, and the mission reaches its end.
//
//   echo exec:StoryProbe.Run > unity/ROI/.roi_cmd
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.UI;

public static class StoryProbe
{
    static int _frame;

    public static void Run()
    {
        // ONLINE: the mission summary claims its reward from the server, which is the half
        // that was missing when this probe was written.
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
            if (_frame < 1800) return;
            EditorApplication.update -= Tick;
            Debug.LogError("STORY: no bar");
            return;
        }
        EditorApplication.update -= Tick;
        UnityEditor.PlayModeWindow.SetCustomRenderingResolution(2560, 1440, "ROI capture");
        Roi.Net.OnlineService.Instance.Run(ToEnd(Drive(bar)));
    }

    static IEnumerator ToEnd(IEnumerator inner)
    {
        yield return inner;
        EditorApplication.isPlaying = false;
    }

    static IEnumerator Drive(Roi.UI.HudBar bar)
    {
        // --- 1. THE FILES ---------------------------------------------------------------------
        var dir = System.IO.Path.Combine(Application.streamingAssetsPath, "story");
        if (!System.IO.Directory.Exists(dir)) { Debug.LogError("STORY: no story directory"); yield break; }
        int files = 0, bad = 0;
        foreach (var path in System.IO.Directory.GetFiles(dir, "*.dialogue"))
        {
            files++;
            var script = Roi.Story.DialogueScript.LoadFile(path);
            foreach (var e in script.Errors)
            {
                bad++;
                Debug.LogError($"STORY FAILED — {System.IO.Path.GetFileName(path)}: {e}");
            }
        }
        Report("every mission file parses", files > 0 && bad == 0, $"{files} files, {bad} errors");

        var driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        var live = driver != null ? driver.Session : null;
        if (live == null) { Debug.LogError("STORY: no live session"); yield break; }
        // Put something IN the live purse first. "0 -> 0" would pass with the seal wide open —
        // an assertion that a number did not change is only worth what the number was.
        live.Debug("{\"give_currency\":{\"gil\":1234,\"soul\":0}}");
        for (int i = 0; i < 5; i++) yield return null;
        long gil0 = Gil(live);
        int party0 = PartySize(live);
        if (gil0 <= 0) { Debug.LogError("STORY: could not put Gil in the live purse to test with"); yield break; }

        // --- 3. THE SCRIPT --------------------------------------------------------------------
        bar.CloseAll();
        bar.Toggle("Story");
        for (int i = 0; i < 90; i++) yield return null;
        if (!Press(bar, "Prologue") && !PressAny(bar, "prologue"))
        {
            Debug.LogError("STORY: no Prologue button in the list");
            yield break;
        }
        for (int i = 0; i < 40; i++) yield return null;

        var box = Object.FindFirstObjectByType<Roi.Story.DialogueBox>();
        if (box == null) { Debug.LogError("STORY: the dialogue box never opened"); yield break; }
        // The DIM is the difference between a scene and a caption over a fight. A modal script is
        // meant to stop the game being answerable behind it, and an inactive scrim looks exactly
        // like a design choice — so it is asserted rather than eyeballed.
        var dim = FindChild(box.transform, "Dim");
        var dimImg = dim != null ? dim.GetComponent<UnityEngine.UI.Image>() : null;
        Report("the modal scrim is up", dimImg != null && dim.gameObject.activeInHierarchy
                                        && dimImg.enabled && dimImg.color.a > 0.2f,
               dimImg == null ? "no scrim at all"
             : $"active={dim.gameObject.activeInHierarchy} alpha={dimImg.color.a:0.00}");

        ScreenCapture.CaptureScreenshot("/tmp/roi_story.png");
        for (int i = 0; i < 20; i++) yield return null;

        bool sawStage = false, sawChoice = false;
        int guard = 0;
        // Walk it the way a player would: tap to finish the typing, tap again to advance, and
        // answer a decision when one appears. The tick budget has to cover the authored 600-tick
        // battle, which is a real minute of sim time.
        // The guard bounds the SCRIPT's steps, not the fight's length — a stage runs on the sim's
        // clock and a 600-tick battle is a real minute of frames. Counting those against the same
        // budget made a working mission time out the moment its cast grew from one member to
        // three and the fight started lasting as long as it was written to.
        while (box.IsBusy && guard < 6000)
        {
            var story = Object.FindFirstObjectByType<Roi.Story.StoryDriver>();
            if (story != null && story.Running)
            {
                if (!sawStage)
                {
                    // THE CAST HAS TO BE THERE. `@ roster preset trial_three` fields three, and a
                    // stage that runs with one member is a scene played around a party that does
                    // not exist — which looks like a working battle from the outside.
                    int cast = story.PartyIds().Count;
                    Report("the preset cast was fielded", cast == 3, $"{cast} member(s) on the field");
                }
                sawStage = true;
                // A stage runs on its own clock. Do NOT tap through it — the box is held and the
                // fight is the scene.
                yield return null;
                continue;
            }
            guard++;
            var choice = FirstChoice(box);
            if (choice != null) { sawChoice = true; choice.onClick.Invoke(); }
            else box.Skip();
            for (int i = 0; i < 3; i++) yield return null;
        }
        Report("the script reached its end", !box.IsBusy, $"{guard} steps");

        // THE SUMMARY, and the payout it asks for. The claim is a BUTTON — an automatic one that
        // failed had no second chance — so the probe presses it like a player would.
        var reward = Object.FindFirstObjectByType<Roi.Story.StoryReward>(FindObjectsInactive.Include);
        for (int i = 0; i < 120 && (reward == null || !reward.IsOpen); i++)
        {
            reward = Object.FindFirstObjectByType<Roi.Story.StoryReward>(FindObjectsInactive.Include);
            yield return null;
        }
        Report("the mission ends on its summary", reward != null && reward.IsOpen,
               reward == null ? "no panel" : reward.IsOpen ? "open" : "built but closed");
        if (reward != null && reward.IsOpen)
        {
            ScreenCapture.CaptureScreenshot("/tmp/roi_story_reward.png");
            for (int i = 0; i < 20; i++) yield return null;
            var claim = FindIn(reward.gameObject, "Claim");
            if (claim == null) Debug.LogError("STORY: the summary drew no Claim button");
            else
            {
                claim.onClick.Invoke();
                // The reward is paid as MAIL, so the proof is a line that stops saying "waiting" —
                // either a figure or "already claimed", both of which are the server answering.
                string line = "";
                for (int i = 0; i < 600; i++)
                {
                    line = RewardLine(reward);
                    if (line != Roi.UI.Loc.Get("UI_STORY_REWARD_READY")
                        && line != Roi.UI.Loc.Get("UI_STORY_REWARD_PENDING")) break;
                    yield return null;
                }
                // The line must have MOVED. Accepting "anything that is not the failure text" let
                // a claim that never left the client pass, because the placeholder is neither.
                bool answered = line.Length > 0
                                && line != Roi.UI.Loc.Get("UI_STORY_REWARD_READY")
                                && line != Roi.UI.Loc.Get("UI_STORY_REWARD_PENDING")
                                && line != Roi.UI.Loc.Get("UI_STORY_REWARD_FAILED");
                Report("the server answered the claim", answered, line);
            }
            reward.Dismiss();
            for (int i = 0; i < 30; i++) yield return null;
        }
        Report("a `@ battle` handed off to the sandbox", sawStage, sawStage ? "a stage ran" : "no stage ever started");
        Report("a decision was offered", sawChoice, sawChoice ? "and answered" : "none appeared");

        // --- 2. THE SEAL ----------------------------------------------------------------------
        // The sandbox gave ITSELF a hundred million Gil and a level-50 cast of three. If a single
        // Gil of that shows up here, the second-sim design has failed at the only thing it is for.
        for (int i = 0; i < 30; i++) yield return null;
        long gil1 = Gil(live);
        int party1 = PartySize(live);
        Report("the live purse did not move", gil1 == gil0, $"{gil0} -> {gil1} Gil");
        Report("the live party did not change", party1 == party0, $"{party0} -> {party1} members");

        Debug.Log("STORY done");
    }

    static Button FindIn(GameObject root, string label)
    {
        foreach (var b in root.GetComponentsInChildren<Button>(true))
        {
            var t = b.GetComponentInChildren<TMPro.TextMeshProUGUI>();
            if (t != null && t.text.StartsWith(label, System.StringComparison.Ordinal)) return b;
        }
        return null;
    }

    /// The payout line, whatever it currently says. It is the only place the server's answer
    /// reaches the screen, so it is the only place a probe can read it.
    static string RewardLine(Roi.Story.StoryReward reward)
    {
        foreach (var t in reward.GetComponentsInChildren<TMPro.TextMeshProUGUI>(true))
            if (t.gameObject.name == "L" && t.color == Roi.UI.UiTheme.Accent
                && t.fontSize <= 17f) return t.text;
        return "";
    }

    static Transform FindChild(Transform root, string name)
    {
        foreach (Transform c in root) if (c.name == name) return c;
        return null;
    }

    static void Report(string what, bool ok, string detail)
    {
        if (ok) Debug.Log($"STORY OK — {what}: {detail}");
        else Debug.LogError($"STORY FAILED — {what}: {detail}");
    }

    static long Gil(Roi.Sim.SimSession s)
    {
        var j = s.ViewJson();
        int i = j.IndexOf("\"gil\":", System.StringComparison.Ordinal);
        if (i < 0) return -1;
        i += 6;
        int end = i;
        while (end < j.Length && char.IsDigit(j[end])) end++;
        return long.TryParse(j.Substring(i, end - i), out var v) ? v : -1;
    }

    static int PartySize(Roi.Sim.SimSession s)
    {
        var v = JsonUtility.FromJson<LiveView>(s.ViewJson());
        int n = 0;
        if (v != null && v.grid != null && v.grid.entities != null)
            foreach (var e in v.grid.entities) if (e.team == "Player" && !e.is_pet) n++;
        return n;
    }

    [System.Serializable] class LiveView { public LiveGrid grid; }
    [System.Serializable] class LiveGrid { public LiveEnt[] entities; }
    [System.Serializable] class LiveEnt { public string team; public bool is_pet; }

    /// A choice button belongs to the BOX, so it is found inside it rather than anywhere on screen
    /// — the arena behind a non-modal script is full of buttons.
    static Button FirstChoice(Roi.Story.DialogueBox box)
    {
        foreach (Transform child in box.transform)
        {
            var choices = child.Find("Choices");
            if (choices == null) continue;
            foreach (Transform c in choices)
            {
                var b = c.GetComponent<Button>();
                if (b != null && b.isActiveAndEnabled) return b;
            }
        }
        return null;
    }

    static bool Press(Roi.UI.HudBar bar, string label)
    {
        foreach (var b in Object.FindObjectsByType<Button>(
                     FindObjectsInactive.Exclude, FindObjectsSortMode.None))
        {
            if (!b.gameObject.activeInHierarchy || !b.interactable) continue;
            var t = b.GetComponentInChildren<TMPro.TextMeshProUGUI>();
            if (t != null && t.text.StartsWith(label, System.StringComparison.Ordinal))
            {
                b.onClick.Invoke();
                return true;
            }
        }
        return false;
    }

    /// The mission's TITLE is a loc key that may not resolve, in which case the button carries the
    /// mission id instead. Both spellings are the same button.
    static bool PressAny(Roi.UI.HudBar bar, string contains)
    {
        foreach (var b in Object.FindObjectsByType<Button>(
                     FindObjectsInactive.Exclude, FindObjectsSortMode.None))
        {
            if (!b.gameObject.activeInHierarchy || !b.interactable) continue;
            var t = b.GetComponentInChildren<TMPro.TextMeshProUGUI>();
            if (t != null && t.text.ToLowerInvariant().Contains(contains))
            {
                b.onClick.Invoke();
                return true;
            }
        }
        return false;
    }
}
