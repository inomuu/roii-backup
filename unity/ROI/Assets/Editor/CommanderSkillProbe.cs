// THE COMMANDER'S SKILL BAR, pressed — and the blast landed where the click said.
//
// Two things are under test and the second is the one that has no other witness:
//   THE BAR only draws skills the player OWNS, so it is empty until the grant. That is the sim's
//     rule (`commander_skills` filters on `commander_skills_owned`) and a bar that drew a locked
//     skill would offer a press `apply_input` refuses.
//   THE AIM maps a screen point to a CELL. Nothing else in the client does that, and a cell
//     computed wrong casts the blast somewhere the player did not click — which on screen looks
//     exactly like a skill with a different radius.
//
//   echo exec:CommanderSkillProbe.Run > unity/ROI/.roi_cmd
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.UI;

public static class CommanderSkillProbe
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
            Debug.LogError("CMDR: no bar");
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
        var driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        var sim = driver != null ? driver.Session : null;
        var arena = Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
        if (sim == null || arena == null) { Debug.LogError("CMDR: no session or arena"); yield break; }

        var cmdr = Object.FindFirstObjectByType<Roi.UI.CommanderBar>(FindObjectsInactive.Include);
        if (cmdr == null) { Debug.LogError("CMDR: the bar was never built"); yield break; }
        Report("locked until granted", Slots(cmdr) == 0, $"{Slots(cmdr)} buttons before the grant");

        // `SetCommanderSkillOwned`, the DEBUG stand-in, not `GrantCommanderSkill`. The real grant
        // is server-authored and carries a keyed nonce the verifier recomputes; a nonce of 0 is the
        // benign default the sim deliberately does NOT apply, so sending one here would have
        // tested nothing and looked like a bar that refused to draw.
        sim.ApplyInput("{\"SetCommanderSkillOwned\":{\"skill_id\":\"commander_smite\",\"owned\":true}}");
        for (int i = 0; i < 60; i++) yield return null;
        Report("the bar draws what the player owns", Slots(cmdr) == 1, $"{Slots(cmdr)} button(s)");
        if (Slots(cmdr) == 0) yield break;

        // Aiming, then a cast at a NAMED cell. The click cannot be faked through the pointer — the
        // Editor has no cursor to move — so the cell goes in directly, which still exercises the
        // one thing that matters here: the event the arena builds from a cell.
        long before = SmiteCooldown(sim);
        Report("nothing is on cooldown yet", before == 0, $"cd {before}");

        int cx = 10, cy = 5;
        arena.CastForTest("commander_smite", cx, cy);
        for (int i = 0; i < 30; i++) yield return null;
        long after = SmiteCooldown(sim);
        Report("the cast fired", after > 0, $"cd {before} -> {after} after a smite at ({cx},{cy})");

        // THE ROUND TRIP. A cell turned into a world point and back must be the same cell, or the
        // footprint preview and the blast disagree — the preview draws where the pointer is and
        // the damage lands where this arithmetic says.
        bool roundTrip = arena.CellRoundTripsForTest(cx, cy);
        Report("a cell survives the trip through world space", roundTrip, $"({cx},{cy})");

        ScreenCapture.CaptureScreenshot("/tmp/roi_commander_bar.png");
        for (int i = 0; i < 20; i++) yield return null;
        Debug.Log("CMDR done");
    }

    static void Report(string what, bool ok, string detail)
    {
        if (ok) Debug.Log($"CMDR OK — {what}: {detail}");
        else Debug.LogError($"CMDR FAILED — {what}: {detail}");
    }

    static int Slots(Roi.UI.CommanderBar bar)
    {
        int n = 0;
        foreach (var b in bar.GetComponentsInChildren<Button>(true)) n++;
        return n;
    }

    /// The smite's cooldown out of the per-tick view — 0 = ready. The one number a cast moves that
    /// no screenshot shows.
    static long SmiteCooldown(Roi.Sim.SimSession sim)
    {
        var j = sim.ViewJson();
        int i = j.IndexOf("\"commander_skills\"", System.StringComparison.Ordinal);
        if (i < 0) return -1;
        i = j.IndexOf("\"cd\":", i, System.StringComparison.Ordinal);
        if (i < 0) return -1;
        i += 5;
        int end = i;
        while (end < j.Length && char.IsDigit(j[end])) end++;
        return long.TryParse(j.Substring(i, end - i), out var v) ? v : -1;
    }
}
