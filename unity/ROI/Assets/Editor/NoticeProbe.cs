// THE TWO SESSION NOTICES, drawn on their own. Neither can be provoked against a healthy local
// server — a failing save and a superseded token are exactly the states a working setup does not
// produce — so they are raised by hand and photographed.
//
//   echo exec:NoticeProbe.Run > unity/ROI/.roi_cmd
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class NoticeProbe
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
        if (++_frame < 150) return;
        var bar = Object.FindFirstObjectByType<Roi.UI.HudBar>();
        if (bar == null)
        {
            if (_frame < 1800) return;
            EditorApplication.update -= Tick;
            Debug.LogError("NOTICE: no bar");
            return;
        }
        EditorApplication.update -= Tick;
        UnityEditor.PlayModeWindow.SetCustomRenderingResolution(2560, 1440, "ROI capture");
        Roi.Net.OnlineService.Instance.Run(ToEnd(Drive()));
    }

    static IEnumerator ToEnd(IEnumerator inner)
    {
        yield return inner;
        EditorApplication.isPlaying = false;
    }

    static IEnumerator Drive()
    {
        Roi.UI.SessionNotice.Warn("Not saved for 3 attempts. Your progress is not being kept.");
        for (int i = 0; i < 30; i++) yield return null;
        Describe("SyncBanner");
        ScreenCapture.CaptureScreenshot("/tmp/roi_banner.png");
        for (int i = 0; i < 30; i++) yield return null;

        Roi.UI.SessionNotice.Ended("Session ended",
            "This account is now active on another device. Nothing more will be saved from here; "
            + "sign in again to continue on this one.", () => { });
        for (int i = 0; i < 30; i++) yield return null;
        Describe("SessionEnded");
        ScreenCapture.CaptureScreenshot("/tmp/roi_session_ended.png");
        for (int i = 0; i < 30; i++) yield return null;
        Debug.Log("NOTICE done");
    }

    /// Where the thing actually IS. A notice that is built but sits off screen, behind a canvas or
    /// at zero size looks identical to one that was never raised at all.
    static void Describe(string name)
    {
        foreach (var t in Object.FindObjectsByType<RectTransform>(
                     FindObjectsInactive.Include, FindObjectsSortMode.None))
        {
            if (t.name != name) continue;
            var c = t.GetComponentInParent<Canvas>();
            var strip = t.GetComponentInChildren<UnityEngine.UI.Image>(true);
            var corners = new Vector3[4];
            if (strip != null) strip.rectTransform.GetWorldCorners(corners);
            Debug.Log($"NOTICE {name}: active={t.gameObject.activeInHierarchy} "
                    + $"canvas={(c == null ? "none" : c.name + "/" + c.sortingOrder)} "
                    + $"first-image={(strip == null ? "none" : strip.name)} "
                    + $"corners={corners[0]}..{corners[2]}");
            return;
        }
        Debug.LogError($"NOTICE {name}: not in the scene at all");
    }
}
