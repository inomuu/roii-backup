// Send the sim the same RestartInstance the death banner sends:
//
//   echo exec:RestartTest.Run > unity/ROI/.roi_cmd
//
// Testing the retry path by dying takes a wipe and several minutes; this is the same input event
// through the same driver, so what it exercises is what a real death exercises.
using UnityEditor;
using UnityEngine;

public static class RestartTest
{
    public static void Run()
    {
        if (!EditorApplication.isPlaying) { Debug.LogWarning("RESTARTTEST: not playing"); return; }
        var drv = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
        if (drv == null) { Debug.LogWarning("RESTARTTEST: no SimDriver"); return; }
        Debug.Log("RESTARTTEST: sending RestartInstance");
        drv.ApplyInput("\"RestartInstance\"");
    }
}
