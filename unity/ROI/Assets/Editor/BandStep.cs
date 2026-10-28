// Leave the baked region and come back, which is the round trip that breaks a zone's art:
//
//   echo exec:BandStep.Out  > unity/ROI/.roi_cmd    # band 6 — region 2, someone else's ground
//   echo exec:BandStep.Home > unity/ROI/.roi_cmd    # band 5 — region 1, the dressed one
//
// Pair it with SceneDump to compare a fresh start against a return: a builder that destroys the
// scene's own objects leaves no error behind, only a different hierarchy, so the diff is the test.
using UnityEditor;
using UnityEngine;

public static class BandStep
{
    public static void Out() => Go(6);
    public static void Home() => Go(5);

    static void Go(int band)
    {
        if (!EditorApplication.isPlaying) { Debug.LogWarning("BANDSTEP: not playing"); return; }
        var drv = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        if (drv == null) { Debug.LogWarning("BANDSTEP: no SimDriver"); return; }
        drv.ApplyInput($"{{\"SelectBand\":{{\"band\":{band}}}}}");
        Debug.Log($"BANDSTEP: asked for band {band}");
    }
}
