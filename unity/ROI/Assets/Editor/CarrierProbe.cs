// EQUIPPING A CARRIER, pressed. An Exemplar and a Regalia are never MOVED — equipping one marks it
// as worn by a member and leaves it in the account's list — so the assertion is the mark itself
// (`equipped_by`), read back off the sim. The list length never changes, which is exactly why a
// screenshot proves nothing here.
//
//   echo exec:CarrierProbe.Run > unity/ROI/.roi_cmd
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.UI;

public static class CarrierProbe
{
    // Both are the SWORDSMAN line, because the dev boot founds with the arena's default class and
    // an Exemplar's class gate refuses a mismatch silently — the one refusal that would look
    // exactly like a dead button. Only two of the six Exemplar lines are in `game_data.json`
    // (Seraphine and Lucielle); a def the catalog does not carry grants NOTHING, which reads on
    // this screen as an empty list rather than as a bad id.
    //
    // A Regalia is unrestricted by design (`regalia_allowed_classes` returns nothing): a mage's
    // sigil on a swordsman is already a bad idea because it scales MATK, and refusing it as well
    // would only take away the off-role builds.
    const string ExemplarDef = "exemplar_seraphine", RegaliaDef = "regalia_hearthguard";

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
            Debug.LogError("CARRIER: no bar");
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
        if (sim == null) { Debug.LogError("CARRIER: no session"); yield break; }

        // STOCKED BY BUYING, not by `grant_item`. The debug grant goes through `GrantLoot` ->
        // `bank_loot_drop`, which knows nothing about the two account-bound lists and drops a
        // carrier into the ordinary bag — so it can never stock the screen under test. `BuyItem`
        // is the real acquisition path (and the 10-Gil rows exist for exactly this).
        sim.Debug("{\"give_currency\":{\"gil\":1000}}");
        for (int i = 0; i < 5; i++) yield return null;
        sim.ApplyInput("{\"BuyItem\":{\"def_id\":\"" + ExemplarDef + "\",\"qty\":1}}");
        sim.ApplyInput("{\"BuyItem\":{\"def_id\":\"" + RegaliaDef + "\",\"qty\":1}}");
        for (int i = 0; i < 10; i++) yield return null;

        // An EXEMPLAR IS A UNIT (0.0.350): buying one makes a benched unit, buying it again banks
        // a Resonance credit, and only the player's Feed spends it. There is no carrier leg.
        int units0 = Count(sim.QueryJson("exemplar_units"), "\"unit_id\":");
        Report("an Exemplar arrives as a UNIT", units0 >= 1, $"{units0} unit(s) listed");
        sim.ApplyInput("{\"BuyItem\":{\"def_id\":\"" + ExemplarDef + "\",\"qty\":1}}");
        for (int i = 0; i < 10; i++) yield return null;
        int units1 = Count(sim.QueryJson("exemplar_units"), "\"unit_id\":");
        bool credited = sim.QueryJson("exemplar_units").Contains("\"credits\":1");
        Report("a dupe is a credit, not a second unit", units1 == units0 && credited,
               $"{units1} unit(s), credited={credited}");
        bar.CloseAll();
        bar.Toggle("Exemplar");
        for (int i = 0; i < 90; i++) yield return null;
        Report("the units screen offers Feed", Pressable("Feed"), Pressable("Feed") ? "live" : "no Feed button");
        if (Press(bar, "Feed"))
        {
            for (int i = 0; i < 20; i++) yield return null;
            if (!PressConfirm("Feed")) Debug.LogError("CARRIER: the feed was never confirmed");
            for (int i = 0; i < 30; i++) yield return null;
            bool starred = sim.QueryJson("exemplar_units").Contains("\"stars\":1");
            Report("Feed spends the credit for a star", starred && !sim.QueryJson("exemplar_units").Contains("\"credits\":1"),
                   starred ? "1★" : "still 0★");
        }
        ScreenCapture.CaptureScreenshot("/tmp/roi_exemplar_units.png");

        yield return Leg(bar, "Regalia", "owned_regalia");
        Debug.Log("CARRIER done");
    }

    /// One carrier list, both ways round. The window is the SAME grid over a different list, so a
    /// bug in the shared half shows up on the second leg and not the first.
    static IEnumerator Leg(Roi.UI.HudBar bar, string screen, string key)
    {
        bar.CloseAll();
        bar.Toggle(screen);
        for (int i = 0; i < 90; i++) yield return null;

        var driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        var sim = driver.Session;
        int grids = 0, cells = 0, filled = 0;
        foreach (var g in Object.FindObjectsByType<Roi.UI.ItemGridWindow>(
                     FindObjectsInactive.Exclude, FindObjectsSortMode.None))
        {
            grids++;
            foreach (var slot in g.GetComponentsInChildren<Roi.UI.ItemSlot>(true))
            {
                cells++;
                if (slot.Item != null && !string.IsNullOrEmpty(slot.Item.def_id)) filled++;
            }
        }
        Debug.Log($"CARRIER {screen} stock — list {sim.QueryJson(key).Length} chars, "
                + $"grids {grids}, cells {cells}, filled {filled}");
        if (WornBy(sim, key) != 0) { Debug.LogError($"CARRIER {screen}: already worn before the press"); yield break; }

        if (!TapFirstFilledCell()) { Debug.LogError($"CARRIER {screen}: no cell to select"); yield break; }
        for (int i = 0; i < 10; i++) yield return null;
        if (!Press(bar, "Equip on")) { Debug.LogError($"CARRIER {screen}: no Equip button"); yield break; }
        for (int i = 0; i < 20; i++) yield return null;
        long worn = WornBy(sim, key);
        long lead = Roi.UI.PartySelection.SelectedId;
        Report($"{screen} equip", worn != 0 && worn == lead, $"equipped_by {worn}, member {lead}");

        ScreenCapture.CaptureScreenshot($"/tmp/roi_{screen.ToLowerInvariant()}_equipped.png");
        for (int i = 0; i < 20; i++) yield return null;

        // THE CHARACTER SHEET has to agree. A Regalia is worn in its own slot, not in the
        // paper-doll's grid, so the only place the sheet says anything about it is the crown —
        // and a crown that never repaints looks exactly like a member wearing nothing.
        if (screen == "Regalia")
        {
            bar.CloseAll();
            bar.Toggle("Character");
            for (int i = 0; i < 90; i++) yield return null;
            var crown = Object.FindFirstObjectByType<Roi.UI.RegaliaCrown>(FindObjectsInactive.Include);
            bool shows = false;
            if (crown != null)
                foreach (var t in crown.GetComponentsInChildren<TMPro.TextMeshProUGUI>(true))
                    if (t.gameObject.name == "Initial" && t.gameObject.activeInHierarchy
                        && !string.IsNullOrEmpty(t.text)) shows = true;
            Report("the sheet shows the worn Regalia", crown != null && shows,
                   crown == null ? "no crown on the sheet" : shows ? "the crown is lit" : "the crown is empty");
            ScreenCapture.CaptureScreenshot("/tmp/roi_regalia_crown.png");
            for (int i = 0; i < 20; i++) yield return null;
            bar.CloseAll();
            bar.Toggle(screen);
            for (int i = 0; i < 60; i++) yield return null;
            if (!TapFirstFilledCell()) { Debug.LogError($"CARRIER {screen}: lost the cell after the sheet"); yield break; }
            for (int i = 0; i < 10; i++) yield return null;
        }

        if (!Press(bar, "Unequip")) { Debug.LogError($"CARRIER {screen}: no Unequip button"); yield break; }
        for (int i = 0; i < 20; i++) yield return null;
        Report($"{screen} unequip", WornBy(sim, key) == 0, $"equipped_by {WornBy(sim, key)}");
    }

    static int Count(string json, string needle)
    {
        int n = 0, i = 0;
        while ((i = json.IndexOf(needle, i, System.StringComparison.Ordinal)) >= 0) { n++; i += needle.Length; }
        return n;
    }

    static void Report(string what, bool ok, string detail)
    {
        if (ok) Debug.Log($"CARRIER OK — {what}: {detail}");
        else Debug.LogError($"CARRIER FAILED — {what}: {detail}");
    }

    /// The mark on the FIRST carrier in the list, straight off the sim. Parsed by hand rather than
    /// through a DTO: one number out of one list is not worth a serializable class.
    static long WornBy(Roi.Sim.SimSession sim, string key)
    {
        var j = sim.QueryJson(key);
        int i = j.IndexOf("\"equipped_by\":", System.StringComparison.Ordinal);
        if (i < 0) return 0;
        i += 14;
        int end = i;
        while (end < j.Length && (char.IsDigit(j[end]) || j[end] == '-')) end++;
        return long.TryParse(j.Substring(i, end - i), out var v) ? v : 0;
    }

    static bool TapFirstFilledCell()
    {
        foreach (var grid in Object.FindObjectsByType<Roi.UI.ItemGridWindow>(
                     FindObjectsInactive.Exclude, FindObjectsSortMode.None))
            foreach (var slot in grid.GetComponentsInChildren<Roi.UI.ItemSlot>(true))
            {
                if (slot.Item == null || string.IsNullOrEmpty(slot.Item.def_id)) continue;
                var b = slot.GetComponent<Button>();
                if (b == null || !b.enabled) continue;
                b.onClick.Invoke();
                return true;
            }
        return false;
    }

    /// Any live button anywhere on screen whose label starts with `label` — the units screen is
    /// built inside the window, not under the bar's own hierarchy.
    static bool Pressable(string label)
    {
        foreach (var b in Object.FindObjectsByType<Button>(FindObjectsInactive.Exclude, FindObjectsSortMode.None))
        {
            if (!b.gameObject.activeInHierarchy || !b.interactable) continue;
            var t = b.GetComponentInChildren<TMPro.TextMeshProUGUI>();
            if (t != null && t.text.StartsWith(label, System.StringComparison.Ordinal)) return true;
        }
        return false;
    }

    static bool PressConfirm(string label)
    {
        var modal = Object.FindFirstObjectByType<Roi.UI.UiConfirm>();
        if (modal == null) return false;
        foreach (var b in modal.GetComponentsInChildren<Button>(true))
        {
            var t = b.GetComponentInChildren<TMPro.TextMeshProUGUI>();
            if (t != null && t.text.StartsWith(label, System.StringComparison.Ordinal))
            {
                b.onClick.Invoke();
                return true;
            }
        }
        return false;
    }

    static bool Press(Roi.UI.HudBar bar, string label)
    {
        foreach (var b in Object.FindObjectsByType<Button>(FindObjectsInactive.Exclude, FindObjectsSortMode.None))
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
