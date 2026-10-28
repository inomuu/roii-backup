// ONE SCREENSHOT OF THE GAME, and the game left running afterwards.
//
// Not a probe: nothing is asserted and play mode is not exited at the end. The capture is the Game
// view itself, so it carries no Editor chrome — a picture of the game rather than of Unity.
//
//   echo exec:ShotNow.Run > unity/ROI/.roi_cmd
using System.IO;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class ShotNow
{
    const string OutDir = "/Users/lunalieneray/Desktop/ROI";
    static int _frame;
    static string _path;
    static bool _hideUi;
    /// Names the burst so a before and an after can sit side by side.
    public static string _tag = "burst";
    static System.Collections.Generic.List<Canvas> _hidden;

    /// The same shot with every Canvas switched off — the world on its own, no bar, no numbers,
    /// no health pips. Restored right after, because this leaves the game RUNNING and handing it
    /// back without its UI would be worse than not taking the picture.
    public static void RunClean()
    {
        _hideUi = true;
        _burst = 0;
        Run();
    }

    /// SIX SHOTS, a third of a second apart, UI hidden. A hit flash lasts a few frames, so one
    /// still is a coin toss — a burst catches one and makes a before/after comparison fair.
    public static void RunBurst()
    {
        _hideUi = true;
        _burst = 12;
        Run();
    }

    static int _burst;
    static int _shot;

    public static void Run()
    {
        // The dev door: straight into the game with no login to sit through, and the local slot
        // means it opens on whatever the last session was playing.
        SessionState.SetString("ROI_BOOT", "dev");
        SessionState.SetString("ROI_STEAM_PIPE", "0");
        EditorSettings.enterPlayModeOptionsEnabled = true;
        EditorSettings.enterPlayModeOptions =
            EnterPlayModeOptions.DisableDomainReload | EnterPlayModeOptions.DisableSceneReload;
        _frame = 0;
        Directory.CreateDirectory(OutDir);
        _shot = 0;
        _path = $"{OutDir}/game{(_hideUi ? "_clean" : "")}.png";
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

    static bool PartyAlive()
    {
        var d = UnityEngine.Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        var sim = d != null ? d.Session : null;
        if (sim == null || !sim.IsAlive) return false;
        var v = JsonUtility.FromJson<View>(sim.ViewJson());
        if (v?.grid?.entities == null) return false;
        foreach (var e in v.grid.entities)
            if (e != null && e.team == "Player" && e.alive && !e.is_pet) return true;
        return false;
    }

    [System.Serializable] class View { public Grid grid; }
    [System.Serializable] class Grid { public Ent[] entities; }
    [System.Serializable] class Ent { public string team; public bool alive; public bool is_pet; }

    static void Tick()
    {
        _frame++;
        // Long enough for the session, the arena and the bar to be up and the party to be fighting
        // — a shot taken on frame one is a picture of an empty field.
        // WAIT FOR A PARTY TO PHOTOGRAPH. The first clean shot caught the field between a wipe and
        // its auto-restart and came back as an empty meadow — correct, and useless as a picture of
        // the game. The frame counter is held until somebody is standing.
        if (_frame >= 410 && _frame < 415 && !PartyAlive()) { _frame = 409; return; }

        // Switched off a couple of frames EARLY: `CaptureScreenshot` grabs at the end of a frame,
        // so hiding on the same frame races the capture and sometimes keeps the bar.
        if (_hideUi && _frame == 415)
        {
            _hidden = new System.Collections.Generic.List<Canvas>();
            foreach (var c in UnityEngine.Object.FindObjectsByType<Canvas>(FindObjectsSortMode.None))
                if (c.enabled) { c.enabled = false; _hidden.Add(c); }
            Debug.Log($"SHOTNOW hid {_hidden.Count} canvas(es)");
            return;
        }
        if (_burst > 0)
        {
            // Every 20th frame from 420: six shots across two seconds of fighting.
            if (_frame >= 420 && _frame <= 420 + 20 * (_burst - 1) && (_frame - 420) % 20 == 0)
            {
                var shot = $"{OutDir}/{_tag}_{_shot}.png";
                ScreenCapture.CaptureScreenshot(shot);
                Debug.Log("SHOTNOW capturing " + shot);
                _shot++;
                return;
            }
            if (_frame < 420 + 20 * _burst + 40) return;
        }
        else
        {
            if (_frame == 420)
            {
                ScreenCapture.CaptureScreenshot(_path);
                Debug.Log("SHOTNOW capturing " + _path);
                return;
            }
            if (_frame < 480) return;
        }
        if (_hidden != null)
        {
            foreach (var c in _hidden) if (c != null) c.enabled = true;
            Debug.Log($"SHOTNOW restored {_hidden.Count} canvas(es)");
            _hidden = null;
        }
        _hideUi = false;
        EditorApplication.update -= Tick;
        // The game KEEPS RUNNING. Whoever asked for the picture is probably watching it.
        Debug.Log("SHOTNOW done — " + _path);
    }
}
