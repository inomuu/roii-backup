// Build the standalone player, for the one thing the Editor cannot answer: what the FRAME TIME is.
//
// The Editor shares the machine with its own asset watcher, the console and the MCP bridge, and its
// spikes say nothing about the game — the project already learned this once and wrote it down. So
// every perf claim here comes from a player build run with `-roiperf`.
//
//   echo exec:BuildPerf.Run > unity/ROI/.roi_cmd
//   /tmp/roi_perf/ROI.app/Contents/MacOS/ROI -roiperf -logFile /tmp/roi_perf.log -batchmode
using UnityEditor;
using UnityEditor.Build.Reporting;
using UnityEngine;

public static class BuildPerf
{
    public const string OutDir = "/tmp/roi_perf";

    public static void Run()
    {
        var opts = new BuildPlayerOptions
        {
            scenes = new[] { ProbeScene.Path },
            locationPathName = OutDir + "/ROI.app",
            target = BuildTarget.StandaloneOSX,
            options = BuildOptions.Development,
        };
        var report = BuildPipeline.BuildPlayer(opts);
        var s = report.summary;
        Debug.Log($"BUILD {s.result} in {s.totalTime.TotalSeconds:0}s, {s.totalSize / 1024 / 1024} MB -> {s.outputPath}");
        if (s.result != BuildResult.Succeeded)
            Debug.LogError($"BUILD FAILED: {s.totalErrors} errors");
    }
}
