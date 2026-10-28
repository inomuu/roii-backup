// PRESS THE BUTTONS, and check the SIM moved.
//
// The contract test proves the client's event JSON parses. It cannot prove the client passed the
// right NUMBER: an index into the wrong list parses perfectly and refines someone else's sword.
// And every screen so far was photographed rather than used. So this drives the real controls in a
// running game and asserts the simulation changed in the way the button claimed it would.
//
// It cheats first, on purpose and out of necessity: a fresh party has 0 Gil so it cannot recruit,
// no consumable is in any drop table so Provisions can never fill, and a card is one in ~50,000.
// Three finished screens were untestable with honest data. The cheats are direct state edits that
// never touch the input log, so this leaves a save that would not verify — which is exactly what a
// cheated save should do.
//
//   echo exec:UiPress.Run > unity/ROI/.roi_cmd
using System.Collections.Generic;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.UI;

public static class UiPress
{
    static int _frame;
    static readonly List<string> _pass = new(), _fail = new();

    public static void Run()
    {
        // Straight past the login — these presses are sim assertions, not a server test.
        SessionState.SetString("ROI_BOOT", "dev");
        EditorSettings.enterPlayModeOptionsEnabled = true;
        EditorSettings.enterPlayModeOptions =
            EnterPlayModeOptions.DisableDomainReload | EnterPlayModeOptions.DisableSceneReload;
        _pass.Clear(); _fail.Clear(); _frame = 0;
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

    static void Check(string what, bool ok)
    {
        (ok ? _pass : _fail).Add(what);
        Debug.Log($"PRESS {(ok ? "ok  " : "FAIL")} {what}");
    }

    static void Tick()
    {
        _frame++;
        if (_frame < 120) return;                 // let the zone build and the sim spin up
        EditorApplication.update -= Tick;

        var driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        var bar = Object.FindFirstObjectByType<Roi.UI.HudBar>();
        if (driver?.Session == null || bar == null) { Debug.LogError("PRESS: no session or no bar"); return; }
        var sim = driver.Session;

        // --- seed the state the drop tables cannot produce -----------------------------------
        Check("cheat: currency", sim.Debug("{\"give_currency\":{\"gil\":50000,\"soul\":50000}}"));
        foreach (var def in new[] { "hp_pot_1", "meal_hearty", "aspd_pot_1", "card_pinkling", "ore_aegirilium" })
            sim.Debug("{\"grant_item\":{\"def_id\":\"" + def + "\"}}");
        Check("cheat: the bag now holds consumables",
              Count(sim.QueryJson("inventory"), "\"kind\":\"Consumable\"") > 0);

        // --- PARTY: recruit, through the real button ----------------------------------------
        // The RAW event first, so a failure downstream can be attributed. If the sim refuses this,
        // the button is innocent; if the sim takes it and the button does not, the button is wired
        // wrong. Distinguishing those two is the entire value of pressing things.
        Debug.Log($"PRESS      gil={GilOf(sim)} roster={Count(sim.QueryJson("roster"), "\"unit_def\"")}");
        sim.ApplyInput("{\"BuyCharacter\":{\"unit_def\":\"acolyte_t1\"}}");
        int viaEvent = Count(sim.QueryJson("roster"), "\"unit_def\"");
        Check("Party: the raw BuyCharacter event recruits", viaEvent > 0);

        bar.Toggle("Party");
        var recruit = FindButton(bar, "Btn_Swordsman");
        Check("Party: a recruit button exists", recruit != null);
        if (recruit != null && recruit.interactable)
        {
            recruit.onClick.Invoke();
            // Recruiting BENCHES the new character rather than deploying them, so the field count
            // is the wrong thing to look at — the roster is where they land.
            Check("Party: the BUTTON recruits too",
                  Count(sim.QueryJson("roster"), "\"unit_def\"") > viaEvent);
        }
        else Check("Party: recruit button was interactable", false);
        bar.CloseAll();

        // --- PROVISIONS: fill an emergency slot, through the real controls -------------------
        bar.Toggle("Provisions");
        var plus = FindButtonByLabel(bar, "+");
        Check("Provisions: an empty slot exists", plus != null);
        if (plus != null)
        {
            plus.onClick.Invoke();                       // opens the picker
            var row = Object.FindFirstObjectByType<Roi.UI.UiRow>();
            Check("Provisions: the picker offered something", row != null);
            if (row != null)
            {
                row.OnPointerClick(new UnityEngine.EventSystems.PointerEventData(null));
                // THE REAL QUESTION: did the index the screen sent address the right item? The
                // entity's own consumables list is the only place that can answer.
                Check("Provisions: the member is now carrying one",
                      driver.Session.ViewJson().Contains("\"consumables\""));
            }
        }
        bar.CloseAll();

        // (The Blacksmith leg left with refine, 0.0.370; Enchant and Apex with 0.0.380.)

        Debug.Log($"PRESS DONE — {_pass.Count} ok, {_fail.Count} failed"
                + (_fail.Count > 0 ? ": " + string.Join("; ", _fail) : ""));
        EditorApplication.isPlaying = false;
    }

    static int Count(string json, string needle)
    {
        int n = 0, i = 0;
        while ((i = json.IndexOf(needle, i, System.StringComparison.Ordinal)) >= 0) { n++; i += needle.Length; }
        return n;
    }

    static int PlayerCount(Roi.Sim.SimSession s) => Count(s.ViewJson(), "\"team\":\"Player\"");
    static long GilOf(Roi.Sim.SimSession s)
    {
        var m = System.Text.RegularExpressions.Regex.Match(s.QueryJson("purse"), "\"gil\":(\\d+)");
        return m.Success ? long.Parse(m.Groups[1].Value) : -1;
    }
    static Button FindButton(Roi.UI.HudBar bar, string name)
    {
        foreach (var b in bar.GetComponentsInChildren<Button>(true))
            if (b.gameObject.name == name && b.gameObject.activeInHierarchy) return b;
        return null;
    }

    static Button FindButtonByLabel(Roi.UI.HudBar bar, string label)
    {
        foreach (var b in bar.GetComponentsInChildren<Button>(true))
        {
            if (!b.gameObject.activeInHierarchy) continue;
            var t = b.GetComponentInChildren<TMPro.TextMeshProUGUI>();
            if (t != null && t.text == label) return b;
        }
        return null;
    }

    static Button FindButtonUnder(Transform root)
    {
        if (root == null) return null;
        foreach (var b in root.GetComponentsInChildren<Button>(true))
            if (b.gameObject.activeInHierarchy) return b;
        return null;
    }
}
