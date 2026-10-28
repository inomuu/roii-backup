// THE JSON CONTRACT between the sim and the two screens that read it by field name.
//
// `JsonUtility` fills a field it cannot find with ZERO. No exception, no warning, no red line in
// the log — a screen whose data source dropped a key renders a confident, wrong number instead. It
// is the quietest failure this client has, and the whole probe suite is blind to it: every screen
// still draws, every button still works, and 23/23 stays green while the sheet reads all noughts.
//
// That is not hypothetical, and this file exists because it already happened. The gacha pivot
// deleted `eff_str…eff_luk`, `points_unspent`, `mdef`, `hit` and `flee` from `stats:<id>` and
// `str/agi/…` from `classes`. Merging it would have put six zero bars on the CLASS-CREATION screen
// — the first screen a new player ever sees — and nine zero rows on the character sheet, past a
// fully green suite. Both screens were rewritten in the merge; the lists below are the contract
// AFTER that, and their job is to catch the next one.
//
// SO THIS ASSERTS PRESENCE, NOT VALUE. "Every number is non-zero" was the obvious version and it is
// wrong twice over: a swordsman's MATK is legitimately 0, and after the merge enough rows survive
// that a "some number is non-zero" check passes while nine are dead. A key being absent is the
// actual defect, it is exactly detectable, and it has no false positives.
//
// WHEN THIS FAILS, IT IS USUALLY RIGHT AND THE SCREEN IS WRONG. The fix is to update the screen for
// the new field set and then update the list here — never to delete the name from the list so the
// probe goes quiet, which converts a caught break into an uncaught one.
//
//   echo probe:ContractProbe > unity/ROI/.roi_cmd
using System;
using System.Collections;
using System.Collections.Generic;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class ContractProbe
{
    // Mirrors `SessionBoot.ClassRow`. One field, and that is the point: the six stat bars this used
    // to feed are gone with the RO attributes, so the list shrank WITH the screen rather than being
    // trimmed to make the probe pass.
    static readonly string[] ClassKeys = { "id" };

    // Mirrors `CharacterWindow.Eff` — every field the character sheet reads from `stats:<id>`.
    static readonly string[] StatKeys =
    {
        "atk", "atk_equip", "matk", "def", "def_equip",
        "dodge_bp", "crit_bp", "crit_dmg_bp", "mitigation_bp", "aspd",
        "level", "level_cap", "max_hp", "xp",
    };

    // Mirrors `CharacterWindow.CharStats`, read off the entity in the view.
    static readonly string[] EntityStatKeys = { "level" };

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
        EditorApplication.update -= Tick;
        Roi.Net.OnlineService.Instance.Run(ToEnd(Drive()));
    }

    static IEnumerator ToEnd(IEnumerator inner)
    {
        yield return inner;
        EditorApplication.isPlaying = false;
    }

    static IEnumerator Drive()
    {
        Roi.Sim.SimSession sim = null;
        for (int i = 0; i < 1800 && sim == null; i++)
        {
            var d = UnityEngine.Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            if (d != null && d.Session != null && d.Session.IsAlive) sim = d.Session;
            else yield return null;
        }
        if (sim == null) { Debug.LogError("CONTRACT: no session"); yield break; }

        // 1. THE CLASS CATALOG, which the founding screen turns into six bars.
        var classes = sim.QueryJson("units");
        Check("units", classes, ClassKeys, "SessionBoot.ClassRow → TitleScreen's stat bars");

        // 2. THE CHARACTER SHEET. Its entity is picked the way the WINDOW picks it, copied rather
        // than reinvented: `team == "Player"` with the capital, because `Team::Player` serializes
        // through serde with no rename and a lowercase compare silently matches nothing — and pets
        // excluded, because the sheet is about the character.
        var view = JsonUtility.FromJson<View>(sim.ViewJson());
        Ent me = null;
        if (view?.grid?.entities != null)
            foreach (var e in view.grid.entities)
                if (e != null && e.team == "Player" && !e.is_pet) { me = e; break; }
        if (me == null) { Debug.LogError("CONTRACT: nobody deployed — cannot read a stat sheet"); yield break; }

        Check("the deployed entity's stats", sim.ViewJson(), EntityStatKeys,
              "CharacterWindow.CharStats → the base half of each attribute row");
        Check($"stats:{me.id}", sim.QueryJson("stats:" + me.id), StatKeys,
              "CharacterWindow.Eff → every number on the sheet");

        Debug.Log("CONTRACT done");
    }

    /// Presence, by key. A key is looked for as `"name":` so a value that merely CONTAINS the word
    /// cannot stand in for the field.
    static void Check(string what, string json, string[] keys, string who)
    {
        if (string.IsNullOrEmpty(json)) { Debug.LogError($"CONTRACT FAILED — {what} answered nothing ({who})"); return; }
        var missing = new List<string>();
        foreach (var k in keys)
            if (json.IndexOf("\"" + k + "\":", StringComparison.Ordinal) < 0) missing.Add(k);
        if (missing.Count == 0)
            Debug.Log($"CONTRACT OK — {what} carries all {keys.Length} fields {who} reads");
        else
            Debug.LogError($"CONTRACT FAILED — {what} is missing {missing.Count} of {keys.Length} "
                         + $"field(s) that {who} reads: {string.Join(", ", missing)}. "
                         + "JsonUtility renders each of these as 0 with no error.");
    }

    [Serializable] class View { public Grid grid; }
    [Serializable] class Grid { public Ent[] entities; }
    [Serializable] class Ent { public long id; public string team; public bool is_pet; }
}
