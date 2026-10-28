// THE BAG'S TWO VERBS, pressed: deposit and withdraw. (The rare net is retired, 0.0.380.)
//
// Each one moves an item between two lists the sim owns, and each is a silent no-op when the sim
// refuses — so the assertion is the COUNTS on both sides, never the screen. A deposit that shows a
// cell disappearing while the warehouse stays empty is exactly what this catches.
//
//   echo exec:BagProbe.Run > unity/ROI/.roi_cmd
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.UI;

public static class BagProbe
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
            Debug.LogError("BAG: no bar");
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
        var driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        var sim = driver != null ? driver.Session : null;
        if (sim == null) { Debug.LogError("BAG: no session"); yield break; }

        // Stock: three ordinary shields for the bag, and one catch for the net.
        for (int i = 0; i < 3; i++) sim.Debug("{\"grant_item\":{\"def_id\":\"shield_s1\"}}");
        // A LEGENDARY WEAPON, granted through the drop road (the `foil` grant banks via
        // `bank_loot_drop`; the plain one drops straight into the bag). Gear, not a card: the net
        // takes only NON-STACKABLE items at or above the threshold, and cards stack — they go to
        // the bag and from there to the album. Getting that wrong is what the first run of this
        // probe found.
        sim.Debug("{\"grant_item\":{\"def_id\":\"unique_blade\",\"foil\":true}}");
        for (int i = 0; i < 10; i++) yield return null;
        Debug.Log($"BAG stock — bag {Count(sim, "inventory")}, stash {Count(sim, "stash")}");

        // --- DEPOSIT ---------------------------------------------------------------------------
        bar.CloseAll();
        bar.Toggle("Inventory");
        for (int i = 0; i < 90; i++) yield return null;
        if (!TapFirstFilledCell(bar, out var moved)) { Debug.LogError("BAG: no filled cell to select"); yield break; }
        for (int i = 0; i < 10; i++) yield return null;
        // COUNTED PER def_id, NOT AS TOTALS. The party keeps fighting through the press, so a drop
        // landing in the same window holds the bag total steady while the deposit really happened —
        // this probe reported `bag 3->3, stash 0->1` as a FAILURE for exactly that reason. A total
        // that moves for reasons other than the thing under test is a second source of failure, not
        // a second check. Both sides are still asserted, which is what catches a "deposit" that
        // copies instead of moves; only the quantity changed.
        int bag0 = CountOf(sim, "inventory", moved), stash0 = CountOf(sim, "stash", moved);
        int bagAll0 = Count(sim, "inventory");
        if (!Press(bar, "Deposit")) { Debug.LogError("BAG: no Deposit button"); yield break; }
        for (int i = 0; i < 20; i++) yield return null;
        int bag1 = CountOf(sim, "inventory", moved), stash1 = CountOf(sim, "stash", moved);
        Report("deposit", bag1 == bag0 - 1 && stash1 == stash0 + 1,
               $"{moved}: bag {bag0}->{bag1}, stash {stash0}->{stash1} "
             + $"(bag total {bagAll0}->{Count(sim, "inventory")}, which the sim also moves)");

        // --- WITHDRAW --------------------------------------------------------------------------
        bar.CloseAll();
        bar.Toggle("Stash");
        for (int i = 0; i < 90; i++) yield return null;
        if (!TapFirstFilledCell(bar, out var back)) { Debug.LogError("BAG: no stash cell to select"); yield break; }
        for (int i = 0; i < 10; i++) yield return null;
        int wBag0 = CountOf(sim, "inventory", back), wStash0 = CountOf(sim, "stash", back);
        if (!Press(bar, "Withdraw")) { Debug.LogError("BAG: no Withdraw button"); yield break; }
        for (int i = 0; i < 20; i++) yield return null;
        int wBag1 = CountOf(sim, "inventory", back), wStash1 = CountOf(sim, "stash", back);
        Report("withdraw", wBag1 == wBag0 + 1 && wStash1 == wStash0 - 1,
               $"{back}: bag {wBag0}->{wBag1}, stash {wStash0}->{wStash1}");

        DumpNames();
        ScreenCapture.CaptureScreenshot("/tmp/roi_bag.png");
        for (int i = 0; i < 20; i++) yield return null;

        Debug.Log("BAG done");
    }

    /// What each cell's name label actually resolved to. A name that wraps mid-word is a layout
    /// bug you can only see at this resolution, and the numbers behind it are invisible on screen.
    static void DumpNames()
    {
        var grid = Object.FindFirstObjectByType<Roi.UI.ItemGridWindow>();
        if (grid == null) return;
        foreach (var t in grid.GetComponentsInChildren<TMPro.TextMeshProUGUI>(true))
        {
            if (t.name != "Name" || string.IsNullOrEmpty(t.text)) continue;
            Debug.Log($"BAG name '{t.text}' size={t.fontSize} rect={t.rectTransform.rect.width:0.0} "
                    + $"pref={t.GetPreferredValues(t.text).x:0.0} lines={t.textInfo.lineCount}");
        }
    }

    static void Report(string what, bool ok, string detail)
    {
        if (ok) Debug.Log($"BAG OK — {what}: {detail}");
        else Debug.LogError($"BAG FAILED — {what}: {detail}");
    }

    /// Reports WHICH item it tapped, because the counts these moves are judged on have to be the
    /// counts of that item and not of the bag.
    static bool TapFirstFilledCell(Roi.UI.HudBar bar, out string defId)
    {
        defId = null;
        var grid = Object.FindFirstObjectByType<Roi.UI.ItemGridWindow>();
        if (grid == null) return false;
        foreach (var slot in grid.GetComponentsInChildren<Roi.UI.ItemSlot>(true))
        {
            if (slot.Item == null || string.IsNullOrEmpty(slot.Item.def_id)) continue;
            var b = slot.GetComponent<Button>();
            if (b == null || !b.enabled) continue;
            defId = slot.Item.def_id;
            b.onClick.Invoke();
            return true;
        }
        return false;
    }

    /// How many of ONE def_id a list holds.
    static int CountOf(Roi.Sim.SimSession sim, string key, string defId)
    {
        if (string.IsNullOrEmpty(defId)) return 0;
        var j = sim.QueryJson(key);
        var needle = "\"def_id\":\"" + defId + "\"";
        int n = 0, i = 0;
        while ((i = j.IndexOf(needle, i, System.StringComparison.Ordinal)) >= 0) { n++; i += needle.Length; }
        return n;
    }

    static int Count(Roi.Sim.SimSession sim, string key)
    {
        var j = sim.QueryJson(key);
        int n = 0, i = 0;
        while ((i = j.IndexOf("\"def_id\"", i, System.StringComparison.Ordinal)) >= 0) { n++; i += 8; }
        return n;
    }

    static bool Press(Roi.UI.HudBar bar, string label)
    {
        foreach (var b in bar.GetComponentsInChildren<Button>(true))
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
}
