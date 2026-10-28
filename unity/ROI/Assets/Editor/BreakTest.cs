// Fire the BOSS break presentation on the nearest monster, without first reaching a boss:
//
//   echo exec:BreakTest.Run > unity/ROI/.roi_cmd
//
// In the game window the same thing is on a key — B for a boss break, N for an ordinary one; see
// Scripts/Arena/BreakDevTrigger.cs.
//
// It runs the same `ArenaView.PlayBreak` the sim's own break event runs, at boss weight — shards,
// the full-size word, the camera push, the sim held, and the cut-in over the top. A separate copy
// of the sequence for testing would be a copy that drifts.
using UnityEditor;
using UnityEngine;

public static class BreakTest
{
    public static void Run()
    {
        if (!EditorApplication.isPlaying) { Debug.LogWarning("BREAKTEST: not playing"); return; }
        var view = Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
        if (view == null) { Debug.LogWarning("BREAKTEST: no ArenaView"); return; }
        Debug.Log(view.DebugBreak(true) ? "BREAKTEST: fired" : "BREAKTEST: no living monster on screen");
    }
}
