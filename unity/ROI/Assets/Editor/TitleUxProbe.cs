// Presentation-only smoke test. No login, remote API call, or player save is created.
// Run via .roi_cmd: exec:TitleUxProbe.Run
using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Roi.UI;
using TMPro;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.InputSystem;
using UnityEngine.InputSystem.LowLevel;
using UnityEngine.UI;
using Object = UnityEngine.Object;

public static class TitleUxProbe
{
    static int _failures;
    static string _language;
    static bool _playOptionsEnabled;
    static EnterPlayModeOptions _playOptions;
    static string Output => Path.GetFullPath(Path.Combine(Application.dataPath, "../../../..", "ui-validation"));

    public static void Run()
    {
        if (EditorApplication.isPlaying) throw new InvalidOperationException("Stop Play mode before this probe.");
        _language = Loc.Language;
        _playOptionsEnabled = EditorSettings.enterPlayModeOptionsEnabled;
        _playOptions = EditorSettings.enterPlayModeOptions;
        EditorSettings.enterPlayModeOptionsEnabled = true;
        EditorSettings.enterPlayModeOptions = EnterPlayModeOptions.DisableDomainReload;
        EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);
        EditorApplication.playModeStateChanged += Entered;
        EditorApplication.EnterPlaymode();
    }

    static void Entered(PlayModeStateChange state)
    {
        if (state != PlayModeStateChange.EnteredPlayMode) return;
        EditorApplication.playModeStateChanged -= Entered;
        EditorSettings.enterPlayModeOptionsEnabled = _playOptionsEnabled;
        EditorSettings.enterPlayModeOptions = _playOptions;
        Directory.CreateDirectory(Output);
        var gameView = EditorWindow.GetWindow(Type.GetType("UnityEditor.GameView,UnityEditor"));
        gameView.Show();
        gameView.Focus();
        PlayModeWindow.SetCustomRenderingResolution(1280, 720, "Title UI validation");
        Loc.Language = "en";
        var host = new GameObject("TitleUxProbe");
        var title = TitleScreen.Create(host.transform, "Luna", "UI preview");
        title.StartCoroutine(Drive(title));
    }

    static void Check(bool condition, string message)
    {
        if (condition) { Debug.Log("TITLE UX PASS: " + message); return; }
        _failures++;
        Debug.LogError("TITLE UX FAIL: " + message);
    }

    static IEnumerator Press(Keyboard keyboard, params Key[] keys)
    {
        InputSystem.QueueStateEvent(keyboard, new KeyboardState(keys));
        yield return null;
        yield return null;
        InputSystem.QueueStateEvent(keyboard, new KeyboardState());
        yield return null;
        yield return null;
    }

    static IEnumerator Shot(string name)
    {
        yield return new WaitForEndOfFrame();
        ScreenCapture.CaptureScreenshot(Path.Combine(Output, name + ".png"));
        yield return null;
        yield return null;
    }

    static IEnumerator Drive(TitleScreen title)
    {
        _failures = 0;
        var keyboard = InputSystem.AddDevice<Keyboard>();
        int playRequests = 0, createRequests = 0;
        title.PlayPressed += (_, __) => playRequests++;
        title.CreatePressed += (_, __) => createRequests++;
        yield return null;
        yield return Shot("title-1280");
        yield return Press(keyboard, Key.Enter);
        var name = title.GetComponentsInChildren<TMP_InputField>().Single();
        var play = title.GetComponentsInChildren<Button>().First(b => b.name == "Primary_Play");
        Check(playRequests == 0, "Enter opens the gate without accidentally signing in");
        Check(EventSystem.current.currentSelectedGameObject == name.gameObject, "Name field receives initial focus");
        yield return Press(keyboard, Key.Tab);
        Check(EventSystem.current.currentSelectedGameObject == play.gameObject, "Tab reaches Play");
        Check(play.GetComponentsInChildren<Image>().Any(i => i.name == "FocusRing" && i.enabled), "Focused Play has a visible ring");
        yield return Shot("signin-1280");
        yield return Press(keyboard, Key.LeftShift, Key.Tab);
        Check(EventSystem.current.currentSelectedGameObject == name.gameObject, "Shift-Tab returns to the name field");
        title.SetBusy(true, "Connecting…");
        Check(title.GetComponentsInChildren<Selectable>().All(s => !s.interactable), "Login controls lock while busy");
        yield return Press(keyboard, Key.Enter);
        Check(playRequests == 0, "Busy login ignores submit");
        title.SetBusy(false, "");
        title.ShowCreation(new List<TitleScreen.ClassInfo>());
        yield return null;
        var party = title.GetComponentsInChildren<TMP_InputField>().Single();
        Check(EventSystem.current.currentSelectedGameObject == party.gameObject, "Creation focuses the party field");
        party.text = "";
        yield return Press(keyboard, Key.Enter);
        Check(createRequests == 0, "Empty party name is rejected");
        Check(title.GetComponentsInChildren<TMP_Text>().Any(t => t.text == Loc.Get("UI_TITLE_NEED_PARTY_NAME")), "Validation error is visible");
        party.text = "Luna";
        yield return Shot("creation-1280");
        title.SetBusy(true);
        Check(title.GetComponentsInChildren<Selectable>().All(s => !s.interactable), "Creation locks name, Random and Begin while busy");
        title.SetBusy(false, "");
        InputSystem.RemoveDevice(keyboard);
        title.GetComponentInChildren<Canvas>().enabled = false;
        yield return null;
        Loc.Language = "th";
        var thai = TitleScreen.Create(new GameObject("ThaiTitleUxProbe").transform, "Luna", "UI preview");
        thai.GetComponentsInChildren<Button>().Single(b => b.name == "TapGate").onClick.Invoke();
        PlayModeWindow.SetCustomRenderingResolution(1024, 768, "Title Thai validation");
        yield return null;
        yield return Shot("signin-th-1024");
        Debug.Log($"TITLE UX RESULT: {(_failures == 0 ? "PASS" : "FAIL")} ({_failures} failures). Screenshots: {Output}");
        Object.Destroy(title.gameObject);
        Loc.Language = _language;
    }
}
