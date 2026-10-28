using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using Roi.UI;
using TMPro;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.UI;

// A single end-to-end title journey. UI-only; never requests authentication or touches a save.
public static class TitleLandingProbe
{
    static bool _optionsEnabled;
    static EnterPlayModeOptions _options;
    static TitleScreen _title;
    static string _language, _clipboard;
    static bool _musicSaved;
    static int _music;
    static float _musicVolume, _sfxVolume;
    static int _checks;
    static bool _sabotage;
    static string Output => Path.GetFullPath(Path.Combine(Application.dataPath,"../../../..","ui-validation","title-redesign"));

    public static void Run()
    {
        _sabotage=false;
        if(EditorApplication.isPlaying)throw new Exception("Stop Play mode first.");
        var importer=(TextureImporter)AssetImporter.GetAtPath("Assets/Resources/TitleUI/Backdrop.png");
        importer.mipmapEnabled=false; importer.npotScale=TextureImporterNPOTScale.None;
        importer.wrapMode=TextureWrapMode.Clamp; importer.maxTextureSize=2048;
        importer.SaveAndReimport();
        _language=Loc.Language; _clipboard=GUIUtility.systemCopyBuffer;
        _musicSaved=PlayerPrefs.HasKey("roi.opt.music"); _music=PlayerPrefs.GetInt("roi.opt.music",70);
        _musicVolume=Roi.Audio.SoundFx.MusicVolume; _sfxVolume=Roi.Audio.SoundFx.SfxVolume;
        _optionsEnabled=EditorSettings.enterPlayModeOptionsEnabled; _options=EditorSettings.enterPlayModeOptions;
        EditorSettings.enterPlayModeOptionsEnabled=true; EditorSettings.enterPlayModeOptions=EnterPlayModeOptions.DisableDomainReload;
        EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Single);
        EditorApplication.playModeStateChanged+=Entered; EditorApplication.playModeStateChanged+=Restore;
        EditorApplication.EnterPlaymode();
    }
    public static void RunSabotage() { Run(); _sabotage=true; }

    static string _bootDoor, _steamPipe;
    public static void OpenGamePreview()
    {
        if(EditorApplication.isPlaying)throw new Exception("Stop Play mode first.");
        _bootDoor=Environment.GetEnvironmentVariable("ROI_BOOT");
        _steamPipe=Environment.GetEnvironmentVariable("ROI_STEAM_PIPE");
        _language=Loc.Language;
        Environment.SetEnvironmentVariable("ROI_BOOT","title");
        Environment.SetEnvironmentVariable("ROI_STEAM_PIPE","0");
        Loc.Language="th";
        _optionsEnabled=EditorSettings.enterPlayModeOptionsEnabled; _options=EditorSettings.enterPlayModeOptions;
        EditorSettings.enterPlayModeOptionsEnabled=true; EditorSettings.enterPlayModeOptions=EnterPlayModeOptions.DisableDomainReload;
        EditorSceneManager.OpenScene("Assets/Scenes/Zone_r1_Painterly_3Rooms.unity");
        EditorApplication.playModeStateChanged+=PreviewEntered;
        EditorApplication.playModeStateChanged+=PreviewRestore;
        EditorApplication.EnterPlaymode();
    }
    static void PreviewRestore(PlayModeStateChange state)
    {
        if(state!=PlayModeStateChange.ExitingPlayMode)return;
        EditorApplication.playModeStateChanged-=PreviewRestore;
        Environment.SetEnvironmentVariable("ROI_BOOT",_bootDoor);
        Environment.SetEnvironmentVariable("ROI_STEAM_PIPE",_steamPipe);
        Loc.Language=_language;
    }
    static void PreviewEntered(PlayModeStateChange state)
    {
        if(state!=PlayModeStateChange.EnteredPlayMode)return;
        EditorApplication.playModeStateChanged-=PreviewEntered;
        EditorSettings.enterPlayModeOptionsEnabled=_optionsEnabled; EditorSettings.enterPlayModeOptions=_options;
        var window=EditorWindow.GetWindow(Type.GetType("UnityEditor.GameView,UnityEditor"));window.Show();window.Focus();
        PlayModeWindow.SetCustomRenderingResolution(1280,720,"Reign of Idlers title");
        var boot=UnityEngine.Object.FindFirstObjectByType<Roi.Sim.SessionBoot>();
        if(boot==null)throw new Exception("Game preview has no SessionBoot");
        boot.StartCoroutine(CaptureGame());
    }
    static IEnumerator CaptureGame()
    {
        yield return null; yield return null;
        var title=UnityEngine.Object.FindFirstObjectByType<TitleScreen>();
        if(title==null)throw new Exception("Game boot did not create title screen");
        yield return Shot("actual-game-boot");
        Debug.Log("TITLE GAME PREVIEW READY: real SessionBoot created the redesigned title.");
    }

    static void Restore(PlayModeStateChange state)
    {
        if(state!=PlayModeStateChange.ExitingPlayMode)return;
        EditorApplication.playModeStateChanged-=Restore;
        Loc.Language=_language; GUIUtility.systemCopyBuffer=_clipboard;
        if(_musicSaved)PlayerPrefs.SetInt("roi.opt.music",_music); else PlayerPrefs.DeleteKey("roi.opt.music");
        PlayerPrefs.Save(); Roi.Audio.SoundFx.SetVolumes(_musicVolume,_sfxVolume);
    }

    static void Entered(PlayModeStateChange state)
    {
        if(state!=PlayModeStateChange.EnteredPlayMode)return;
        EditorApplication.playModeStateChanged-=Entered;
        EditorSettings.enterPlayModeOptionsEnabled=_optionsEnabled; EditorSettings.enterPlayModeOptions=_options;
        Directory.CreateDirectory(Output);
        var window=EditorWindow.GetWindow(Type.GetType("UnityEditor.GameView,UnityEditor")); window.Show(); window.Focus();
        PlayModeWindow.SetCustomRenderingResolution(1280,720,"Title landing validation");
        Loc.Language="th"; _checks=0;
        _title=TitleScreen.Create(new GameObject("TitleLandingProbe").transform,"Luna",Roi.Net.OnlineService.ServerUrl);
        _title.StartCoroutine(Drive());
    }

    static void Require(bool value,string description)
    {
        if(!value)throw new Exception("TITLE LANDING FAIL: "+description);
        _checks++; Debug.Log("TITLE LANDING CHECK: "+description);
    }
    static Button Find(string name) => _title.GetComponentsInChildren<Button>().Single(b=>b.name==name);
    static void Click(string name)
    {
        var button=Find(name); var rt=(RectTransform)button.transform;
        var point=RectTransformUtility.WorldToScreenPoint(null,rt.TransformPoint(rt.rect.center));
        var data=new PointerEventData(EventSystem.current){position=point,button=PointerEventData.InputButton.Left};
        var hits=new List<RaycastResult>(); EventSystem.current.RaycastAll(data,hits);
        Require(hits.Count>0 && hits[0].gameObject.GetComponentInParent<Button>()==button,"Touch reaches "+name);
        ExecuteEvents.Execute(button.gameObject,data,ExecuteEvents.pointerClickHandler);
    }
    static IEnumerator Shot(string name)
    {
        yield return null; yield return new WaitForEndOfFrame();
        ScreenCapture.CaptureScreenshot(Path.Combine(Output,name+".png"));
        yield return null; yield return null;
    }
    static void Bounds(Rect safe)
    {
        Canvas.ForceUpdateCanvases();
        foreach(var b in _title.GetComponentsInChildren<Button>())
        {
            var corners=new Vector3[4]; ((RectTransform)b.transform).GetWorldCorners(corners);
            Require(corners[0].x>=safe.xMin-1 && corners[0].y>=safe.yMin-1 &&
                corners[2].x<=safe.xMax+1 && corners[2].y<=safe.yMax+1,b.name+" stays inside safe area");
            Require(corners[2].y-corners[0].y>=44 && corners[2].x-corners[0].x>=44,b.name+" has a 44px touch target");
        }
    }
    static IEnumerator Drive()
    {
        yield return Shot("landing-th-1280");
        Bounds(new Rect(0,0,1280,720));
        Require(!_title.GetComponentsInChildren<Transform>(true).Any(t=>t.name=="ServerPicker"),"Single-server landing has no server picker");
        if(_sabotage)Find("TapGate").onClick.RemoveAllListeners();
        Click("TapGate"); yield return null;
        Require(_title.GetComponentsInChildren<TMP_InputField>().Length==1,"Start Game opens account form");
        Click("Close"); yield return null;
        Click("Settings"); yield return null;
        Click("roi.opt.music_50");
        Require(PlayerPrefs.GetInt("roi.opt.music")==50 && Mathf.Approximately(Roi.Audio.SoundFx.MusicVolume,.5f),"Settings apply and persist volume");
        yield return Shot("settings"); Click("Close"); yield return null;
        Click("News"); yield return Shot("news"); Click("Close"); yield return null;
        Click("Support"); yield return null; Click("CopyDiagnostics");
        Require(GUIUtility.systemCopyBuffer.Contains(Roi.Sim.SimSession.SimVersion()) &&
            GUIUtility.systemCopyBuffer.Contains(new Uri(Roi.Net.OnlineService.ServerUrl).Host),"Support copies version and host");
        yield return Shot("support"); Click("Close"); yield return null;
        int requests=0; _title.PlayPressed+=(_,__)=>requests++;
        Click("Account"); yield return null;
        Require(requests==0 && _title.GetComponentsInChildren<TMP_InputField>().Length==1,"Account opens form without signing in");
        _title.SetBusy(true,Loc.Get("UI_NOTICE_RECONNECT"));
        Require(_title.GetComponentsInChildren<Selectable>().All(s=>!s.interactable),"Busy account locks all visible controls");
        _title.SetBusy(false,Loc.Get("UI_LOGIN_UPDATE_REQUIRED")); yield return Shot("account-error");
        Click("Close"); yield return null;
        Require(EventSystem.current.currentSelectedGameObject==Find("TapGate").gameObject,"Closing restores Start Game focus");
        PlayModeWindow.SetCustomRenderingResolution(844,390,"Landscape phone"); yield return Shot("phone-844x390");
        Bounds(new Rect(0,0,844,390));
        var safe=new Rect(44,16,756,374);
        typeof(TitleScreen).GetMethod("LayoutLanding",BindingFlags.Instance|BindingFlags.NonPublic).Invoke(_title,new object[]{(Rect?)safe});
        yield return Shot("phone-safe-area"); Bounds(safe);
        Click("Settings"); yield return Shot("phone-settings"); Click("Close"); yield return null;
        PlayModeWindow.SetCustomRenderingResolution(2340,1080,"Wide landscape phone"); yield return Shot("wide-2340x1080");
        Bounds(new Rect(0,0,2340,1080));
        PlayModeWindow.SetCustomRenderingResolution(1024,768,"Tablet"); yield return Shot("tablet-1024x768");
        Bounds(new Rect(0,0,1024,768));
        PlayModeWindow.SetCustomRenderingResolution(1280,720,"Title preview"); yield return Shot("final-preview");
        File.WriteAllText(Path.Combine(Output,"result.txt"),"PASS: title journey, "+_checks+" checks. Includes real raycast clicks, settings persistence, Start Game navigation, account busy lock, focus return, 44px targets, 16:9, 20:9, tablet and simulated notch safe area.");
        Debug.Log("TITLE LANDING RESULT: PASS ("+_checks+" checks)");
    }
}
