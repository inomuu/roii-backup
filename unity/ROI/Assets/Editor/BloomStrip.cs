// THE HIT FLASH HELD ON THREE MONSTERS AT THREE VALUES, so "does it bloom" stops being an argument
// about numbers and becomes something you can see — on the actual sprite, with the actual shader,
// not on a flat quad standing in for one.
//
// The hit flash lasts a few frames; three bursts of twelve stills caught it once, by luck, which is
// no way to judge a grade. These are static, unlit, and output an exact value each — so whichever
// ones glow tell you where this scene's bloom threshold actually sits, in this colour space, after
// this tonemapper, with no reasoning in between.
//
// The flash REPLACES the lit colour after exposure (`lit = lerp(lit, _FlashColor, _Flash)`), so
// what each monster shows IS its value — the sun and the SH probe do not touch it. Held every
// editor frame because the arena rewrites its own property block on each view, and a value set once
// would be gone by the next tick.
//
//   echo exec:BloomStrip.Run > unity/ROI/.roi_cmd
using UnityEditor;
using UnityEngine;

public static class BloomStrip
{
    const string OutDir = "/Users/lunalieneray/Desktop/ROI";
    /// 1.0 is the LDR ceiling — it must never bloom. 2.2 is what the monster flash writes. 5.0 is
    /// well past anything the scene contains. If 2.2 does not glow while 5.0 does, the threshold is
    /// simply set above the flash.
    static readonly float[] Values = { 1.0f, 2.2f, 5.0f };
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
            UnityEditor.SceneManagement.EditorSceneManager.OpenScene(
                ProbeScene.Path, UnityEditor.SceneManagement.OpenSceneMode.Single);
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
        _frame++;
        // Every frame from 300: the arena keeps rewriting the block, so this has to keep winning.
        if (_frame >= 300) Build();
        if (_frame == 340)
        {
            ScreenCapture.CaptureScreenshot($"{OutDir}/bloom_strip.png");
            Debug.Log($"BLOOMSTRIP capturing {OutDir}/bloom_strip.png");
        }
        if (_frame < 400) return;
        Debug.Log("BLOOMSTRIP done — the patches stay up; look at them live too.");
        EditorApplication.update -= Tick;
    }

    /// Adds the LateUpdate holder once. Setting the block from here loses to
    /// `ArenaView.TickJuice`, which writes `_Flash` from the actor's own timer on its Update.
    static void Build()
    {
        if (Object.FindFirstObjectByType<Roi.Arena.FlashHold>() != null) return;
        var go = new GameObject("FlashHold");
        var h = go.AddComponent<Roi.Arena.FlashHold>();
        h.Values = Values;
        Debug.Log("BLOOMSTRIP holding the flash from LateUpdate at 1.0 / 2.2 / 5.0 (left to right)");
    }
}
