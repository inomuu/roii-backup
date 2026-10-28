// THE MARCH, MEASURED. Does the party actually arrive where the art is?
//
// Everything else about the walk-through band is checkable by eye and nothing else: the camera
// glides, the seams line up, the room looks right. This is the one part that is a NUMBER, and it is
// the number two engines disagree about. Godot advances one field width (20) in +x because its band
// art is a strip three fields wide. Unity's rooms are authored terrain 52 m apart and its world x is
// mirrored, so the same march is -52. Copying either half of that wrong lands the party in the gap
// between two rooms, or walks it away from the art entirely — and NOTHING ELSE NOTICES. Probes press
// controls and read the sim; none of them looks at the ground.
//
// MEASURED THROUGH `CellToWorld`, not off a party member. Where a character stands is a fight
// outcome and moves every tick; where CELL (0,0) IS is the room's own anchor, and it is the exact
// function every prop, actor and light is placed by. If this is right, the whole field is right.
//
// A DEV SESSION, because it has to cheat. Clearing a room means killing everything in it, and the
// sim refuses debug edits once the client is online — correctly, a cheated save stops verifying.
// The march is renderer arithmetic with no server in it, so a local session is the honest place to
// test it and costs no account state.
//
//   echo probe:MarchProbe > unity/ROI/.roi_cmd
using System;
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class MarchProbe
{
    static int _frame;

    public static void Run()
    {
        SessionState.SetString("ROI_BOOT", "dev");
        SessionState.SetString("ROI_STEAM_PIPE", "0");
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
        Roi.Arena.ArenaView arena = null;
        Roi.Sim.SimSession sim = null;
        for (int i = 0; i < 1800 && (arena == null || sim == null); i++)
        {
            arena = UnityEngine.Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
            var d = UnityEngine.Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            sim = d != null ? d.Session : null;
            if (arena == null || sim == null || !sim.IsAlive) { sim = null; yield return null; }
        }
        if (arena == null || sim == null) { Debug.LogError("MARCH: no arena or no session"); yield break; }

        // START FROM ROOM 1, ALWAYS. This measured whatever room the session happened to be in,
        // which is whatever the probes before it left behind — and the two steps it checks are
        // room 1->2 and 2->3, so starting anywhere else spends step 1 on a BAND change and fails
        // on a march that never went wrong. `SelectBand` lands on an instance's room 1 and is the
        // sim's own way of saying so, rather than this file learning how a zone id is spelled.
        string here = ZoneOf(sim);
        int band = BandOf(here);
        if (RoomOf(here) != 1)
        {
            sim.ApplyInput($"{{\"SelectBand\":{{\"band\":{band}}}}}");
            for (int i = 0; i < 120 && RoomOf(ZoneOf(sim)) != 1; i++) yield return null;
            Debug.Log($"MARCH: reset {here} -> {ZoneOf(sim)} so the walk starts at room 1");
        }
        // The renderer needs the frame after the swap to place the field.
        for (int i = 0; i < 30; i++) yield return null;

        float home = arena.CellToWorldPublic(0f, 0f).x;
        string zone = ZoneOf(sim);
        Debug.Log($"MARCH starting in {zone}, cell (0,0) at x={home:0.##}");
        if (RoomOf(zone) != 1)
        {
            Debug.LogError($"MARCH: could not reach room 1 (stuck in {zone}) — the walk is not measurable from here");
            yield break;
        }

        // ROOM 1 -> 2 and 2 -> 3. Each step must move the field by exactly one RoomSpacing, and the
        // room number must go UP by one — a jump back to room 1 is the case the scroll must refuse.
        for (int step = 1; step <= 2; step++)
        {
            string from = zone;
            yield return ClearTheRoom(sim, from);
            zone = ZoneOf(sim);
            if (zone == from) { Debug.LogError($"MARCH: room never advanced out of {from}"); yield break; }
            // The renderer applies the scroll on the view AFTER the sim swaps; give it that frame.
            for (int i = 0; i < 30; i++) yield return null;

            float now = arena.CellToWorldPublic(0f, 0f).x;
            float want = home - Roi.Arena.ArenaView.RoomSpacing * step;
            Report($"step {step}: {from} -> {zone} moved exactly one room",
                   Mathf.Abs(now - want) < 0.01f,
                   $"x={now:0.##}, expected {want:0.##} (home {home:0.##} - {step}x{Roi.Arena.ArenaView.RoomSpacing})");
            Report($"step {step}: the room number went up by one",
                   RoomOf(zone) == RoomOf(from) + 1, $"{from} -> {zone}");
        }

        // ROOM 3 -> THE NEXT BAND. Not a march: the screen swaps behind the fade, so the scroll has
        // to RESET. Godot's comment says this branch was learned the hard way — a band change that
        // kept extending the scroll walks the party off the end of the art, one room further every
        // band, and looks fine until it doesn't.
        {
            string from = zone;
            yield return ClearTheRoom(sim, from);
            zone = ZoneOf(sim);
            for (int i = 0; i < 30; i++) yield return null;
            float now = arena.CellToWorldPublic(0f, 0f).x;
            bool bandChanged = BandOf(zone) != BandOf(from);
            if (!bandChanged)
                Debug.Log($"MARCH: {from} -> {zone} stayed in the band — no reset to check here");
            else
                Report("a new band resets the scroll instead of extending it",
                       Mathf.Abs(now - home) < 0.01f,
                       $"x={now:0.##}, expected the starting {home:0.##} ({from} -> {zone})");
        }

        Debug.Log("MARCH done");
    }

    /// Finish the room the way the SIM finishes it, minus the waiting.
    ///
    /// `mark_zone_cleared_if_swept` wants the boss SPAWNED and nothing left alive, and the boss only
    /// spawns once `kill_counter` reaches `boss_threshold`. A debug `kill` sets `alive = false`
    /// directly and never touches that counter — the increment lives in the combat death path
    /// (`combat_step/xp.rs`) — so killing everything in sight cleared nothing at all, forever. This
    /// probe's first version did exactly that and reported "room never advanced" after 90 seconds
    /// of killing.
    ///
    /// So `meet_boss_threshold` skips the FARM and nothing else: the boss still has to spawn, the
    /// field still has to end up empty, and the sim still decides when the zone swaps. The rule
    /// under test stays the sim's.
    static IEnumerator ClearTheRoom(Roi.Sim.SimSession sim, string from)
    {
        sim.Debug("{\"meet_boss_threshold\":{}}");
        for (int i = 0; i < 5400 && ZoneOf(sim) == from; i++)
        {
            if (i % 15 == 0)
            {
                // Re-armed each pass: a room that respawns while being swept pushes the counter
                // back down, and one nudge at the start would leave the boss never spawning.
                sim.Debug("{\"meet_boss_threshold\":{}}");
                foreach (var id in MonsterIds(sim))
                    sim.Debug("{\"kill\":{\"entity_id\":" + id + "}}");
            }
            yield return null;
        }
    }

    static long[] MonsterIds(Roi.Sim.SimSession sim)
    {
        var v = JsonUtility.FromJson<View>(sim.ViewJson());
        if (v?.grid?.entities == null) return Array.Empty<long>();
        var list = new System.Collections.Generic.List<long>();
        foreach (var e in v.grid.entities)
            if (e != null && e.alive && e.team == "Monster") list.Add(e.id);
        return list.ToArray();
    }

    static string ZoneOf(Roi.Sim.SimSession sim)
    {
        var v = JsonUtility.FromJson<View>(sim.ViewJson());
        return v != null ? v.current_zone_id ?? "" : "";
    }

    /// "zone_b<band>_<room>" — the room is the tail, the band is between "_b" and the last "_".
    static int RoomOf(string zoneId)
    {
        if (string.IsNullOrEmpty(zoneId)) return 0;
        int i = zoneId.LastIndexOf('_');
        return i < 0 || !int.TryParse(zoneId.Substring(i + 1), out var r) ? 0 : r;
    }

    static int BandOf(string zoneId)
    {
        if (string.IsNullOrEmpty(zoneId)) return 0;
        int i = zoneId.IndexOf("_b", StringComparison.Ordinal);
        if (i < 0) return 0;
        int j = i + 2, band = 0;
        while (j < zoneId.Length && char.IsDigit(zoneId[j])) { band = band * 10 + (zoneId[j] - '0'); j++; }
        return band;
    }

    static void Report(string what, bool ok, string detail)
    {
        if (ok) Debug.Log($"MARCH OK — {what}: {detail}");
        else Debug.LogError($"MARCH FAILED — {what}: {detail}");
    }

    [Serializable] class View { public string current_zone_id; public Grid grid; }
    [Serializable] class Grid { public Ent[] entities; }
    [Serializable] class Ent { public long id; public string team; public bool alive; }
}
