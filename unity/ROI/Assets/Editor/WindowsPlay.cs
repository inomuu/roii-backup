using UnityEditor;
using UnityEditor.SceneManagement;

/// Opens the real game at its normal title screen. The player chooses their login.
public static class WindowsPlay
{
    public static void Capture()
    {
        var view = EditorWindow.GetWindow(System.Type.GetType("UnityEditor.GameView,UnityEditor"));
        view.Show();
        view.Focus();
        PlayModeWindow.SetCustomRenderingResolution(1280, 720, "ROI Windows");
        var output = System.IO.Path.GetFullPath(System.IO.Path.Combine(UnityEngine.Application.dataPath, "../../../../ui-validation"));
        System.IO.Directory.CreateDirectory(output);
        UnityEngine.ScreenCapture.CaptureScreenshot(System.IO.Path.Combine(output, "windows-title.png"));
    }

    public static void Run()
    {
        SessionState.SetString("ROI_BOOT", "");
        SessionState.SetString("ROI_STEAM_PIPE", "");
        if (EditorApplication.isPlaying) return;
        EditorSceneManager.OpenScene(ProbeScene.Path, OpenSceneMode.Single);
        EditorApplication.EnterPlaymode();
    }
}
