// The arena's readout, including the two states a probe cannot wait for.
//
// The zone strip fills as the room is cleared, and that is reachable by playing. The BOSS PLATE and
// the AREA CLEARED card are not: a boss is eleven kills away at best and a band-clear is three
// rooms, which is minutes of waiting for a drawing check. So this feeds the overlay a HAND-WRITTEN
// view of the shape the sim emits — the same payload, with a boss in it — and shoots that.
//
// It is a drawing test, and it says so: it proves the plate lays out, clamps and reads. Whether the
// sim ever produces that payload is the arena's own business, and `zone_spawn` is already in the
// trimmed view (view_state.rs keeps kill_counter / boss_threshold / boss_spawned / zone_cleared).
//
//   echo exec:OverlayProbe.Run > unity/ROI/.roi_cmd
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class OverlayProbe
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
        if (++_frame < 180) return;
        var overlay = Object.FindFirstObjectByType<Roi.UI.ArenaOverlay>();
        if (overlay == null)
        {
            if (_frame < 1800) return;
            EditorApplication.update -= Tick;
            Debug.LogError("OVERLAY: no ArenaOverlay in the scene");
            return;
        }
        EditorApplication.update -= Tick;
        Roi.Net.OnlineService.Instance.Run(ToEnd(Drive(overlay)));
    }

    static IEnumerator ToEnd(IEnumerator inner)
    {
        yield return inner;
        EditorApplication.isPlaying = false;
    }

    static IEnumerator Drive(Roi.UI.ArenaOverlay overlay)
    {
        // STOP THE SIM first. The arena publishes a view every tick and hands it to the overlay,
        // so a hand-written frame is overwritten before the screenshot is written — which is what
        // the first run of this probe actually photographed.
        var driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        if (driver != null) driver.enabled = false;
        yield return null;

        // A boss at two thirds, in a room that has spawned it.
        const string boss = "{\"current_zone_id\":\"zone_b3_2\",\"loot_gap_mult_bp\":7200,"
            + "\"zone_spawn\":{\"zone_cleared\":false,\"boss_spawned\":true,\"is_instance_end\":false,"
            + "\"kill_counter\":11,\"boss_threshold\":11},"
            + "\"grid\":{\"entities\":[{\"is_boss\":true,\"alive\":true,\"hp\":8200,"
            + "\"max_hp\":12000,\"eff_max_hp\":12000,\"name\":\"Duskmourn Warden\"}]}}";
        overlay.OnView(boss);
        for (int i = 0; i < 20; i++) yield return null;
        ScreenCapture.CaptureScreenshot("/tmp/roi_overlay_boss.png");
        for (int i = 0; i < 20; i++) yield return null;
        Debug.Log("OVERLAY boss plate drawn");

        // The band-end clear, which is a RISING EDGE — so it needs a not-cleared view first.
        const string before = "{\"current_zone_id\":\"zone_b7_3\",\"loot_gap_mult_bp\":10000,"
            + "\"zone_spawn\":{\"zone_cleared\":false,\"boss_spawned\":true,\"is_instance_end\":true,"
            + "\"kill_counter\":11,\"boss_threshold\":11},\"grid\":{\"entities\":[]}}";
        const string after = "{\"current_zone_id\":\"zone_b7_3\",\"loot_gap_mult_bp\":10000,"
            + "\"zone_spawn\":{\"zone_cleared\":true,\"boss_spawned\":true,\"is_instance_end\":true,"
            + "\"kill_counter\":11,\"boss_threshold\":11},\"grid\":{\"entities\":[]}}";
        overlay.OnView(before);
        yield return null;
        overlay.OnView(after);
        for (int i = 0; i < 30; i++) yield return null;   // mid-hold, while the card is opaque
        ScreenCapture.CaptureScreenshot("/tmp/roi_overlay_clear.png");
        for (int i = 0; i < 20; i++) yield return null;
        // Band 7 of 5-per-region is region 2, sub 2 — the label a player reads on the world map.
        Debug.Log("OVERLAY clear card drawn (band 7 = area 2-2)");
        // The RUN SUMMARY, fed the same way and for the same reason: a dungeon clear is minutes
        // of play away, and this is a drawing test.
        var clear = Object.FindFirstObjectByType<Roi.UI.ClearSummary>(FindObjectsInactive.Include);
        if (clear == null) { Debug.LogError("OVERLAY: no ClearSummary"); yield break; }
        const string run = "{\"last_clear\":{\"instance_id\":\"zone_b4_1\",\"ticks\":4130,"
            + "\"kills\":37,\"xp\":12400,\"gil\":860,\"failed\":false,"
            + "\"loot\":[{\"def_id\":\"ore_aegirilium\",\"name\":\"Aegirilium\",\"quantity\":3},"
            + "{\"def_id\":\"shield_s1\",\"name\":\"Kite Shield\",\"quantity\":1}]}}";
        clear.OnView(run);
        for (int i = 0; i < 20; i++) yield return null;
        ScreenCapture.CaptureScreenshot("/tmp/roi_clear_summary.png");
        for (int i = 0; i < 20; i++) yield return null;
        var scrim = clear.transform.Find("Scrim")?.GetComponent<UnityEngine.UI.Image>();
        Debug.Log($"OVERLAY run summary drawn — scrim {(scrim == null ? "MISSING" : scrim.color + " rect " + scrim.rectTransform.rect.size)}");

        // The DAMAGE METER reads the sim, so it needs the sim back.
        if (driver != null) driver.enabled = true;
        var dps = Object.FindFirstObjectByType<Roi.UI.DpsMeter>(FindObjectsInactive.Include);
        if (dps == null) { Debug.LogError("OVERLAY: no DpsMeter"); yield break; }
        dps.Toggle();
        for (int i = 0; i < 300; i++) yield return null;   // let the party land some hits
        ScreenCapture.CaptureScreenshot("/tmp/roi_dps.png");
        for (int i = 0; i < 20; i++) yield return null;
        Debug.Log($"OVERLAY damage meter open={dps.Visible}");

        // The COMPANION widget: three panes, each locked on its own member, rendered on a cadence.
        var comp = Object.FindFirstObjectByType<Roi.UI.CompanionWidget>(FindObjectsInactive.Include);
        if (comp == null) { Debug.LogError("OVERLAY: no CompanionWidget"); yield break; }
        comp.Toggle();
        for (int i = 0; i < 90; i++) yield return null;
        ScreenCapture.CaptureScreenshot("/tmp/roi_companion.png");
        for (int i = 0; i < 20; i++) yield return null;
        Debug.Log($"OVERLAY companion open={comp.Visible}");

        Debug.Log("OVERLAY done");
    }
}
