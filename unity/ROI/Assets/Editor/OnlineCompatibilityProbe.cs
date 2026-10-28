using System;
using System.Collections;
using System.IO;
using System.Linq;
using Roi.Net;
using Roi.UI;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

// Public version endpoint only. An incompatible build must stop before login or save.
public static class OnlineCompatibilityProbe
{
    public static void Run()
    {
        if (EditorApplication.isPlaying) throw new InvalidOperationException("Stop Play mode first.");
        EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);
        EditorApplication.playModeStateChanged += Entered;
        EditorApplication.EnterPlaymode();
    }
    static void Entered(PlayModeStateChange state)
    {
        if (state != PlayModeStateChange.EnteredPlayMode) return;
        EditorApplication.playModeStateChanged -= Entered;
        PlayModeWindow.SetCustomRenderingResolution(1280, 720, "Online compatibility");
        var title = TitleScreen.Create(new GameObject("OnlineCompatibilityProbe").transform, "Connection test", "");
        title.GetComponentsInChildren<UnityEngine.UI.Button>().Single(b=>b.name=="TapGate").onClick.Invoke();
        title.StartCoroutine(Drive(title));
    }
    static IEnumerator Drive(TitleScreen title)
    {
        var local = ServerClient.LocalVersion();
        ServerClient.VersionInfo remote = null;
        string error = null;
        var client = new ServerClient(OnlineService.ServerUrl);
        yield return client.GetVersion((version, reason) => { remote = version; error = reason; });
        if (remote == null || !string.IsNullOrEmpty(error)) throw new Exception("LIVE VERSION FAIL: " + error);
        string mismatch = ServerClient.CompatibilityError(local, remote);
        var evidence = "Server: " + client.BaseUrl + "\nLocal: " + JsonUtility.ToJson(local)
            + "\nRemote: " + JsonUtility.ToJson(remote) + "\nCompatibility: " + mismatch;
        if (string.IsNullOrEmpty(mismatch))
        {
            evidence += "\nMatching version; authenticated gameplay requires an authorized test account.";
        }
        else
        {
            var online = OnlineService.Instance;
            title.SetBusy(true, Loc.Get("UI_NOTICE_RECONNECT"));
            yield return online.ConnectAndWait("codex-version-check", false);
            if (online.Ready || online.LoggedIn || online.LastError != mismatch)
                throw new Exception("LIVE VERSION FAIL: incompatible build did not stop before authentication");
            title.SetBusy(false, online.LastError);
            evidence += "\nPASS: live server mismatch stops login before authentication, save or account changes.";
        }
        var output = Path.GetFullPath(Path.Combine(Application.dataPath, "../../../..", "ui-validation"));
        Directory.CreateDirectory(output);
        File.WriteAllText(Path.Combine(output, "live-server-compatibility.txt"), evidence);
        yield return new WaitForEndOfFrame();
        ScreenCapture.CaptureScreenshot(Path.Combine(output, "live-server-compatibility.png"));
        Debug.Log("LIVE VERSION RESULT: " + evidence);
    }
}
