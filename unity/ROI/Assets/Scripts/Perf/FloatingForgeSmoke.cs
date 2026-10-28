// Development-only standalone probe: -roiforgesmoke. Isolated saves, no Steam/server access.
// Exercises the actual native window and UI raycaster; writes evidence and exits nonzero on failure.
// -roiforgepreview uses the same isolated fixtures, then leaves the UI open for manual play.
#if DEVELOPMENT_BUILD || UNITY_EDITOR
using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Roi.Platform;
using Roi.Sim;
using Roi.UI;
using TMPro;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.InputSystem;
using UnityEngine.InputSystem.LowLevel;
using UnityEngine.UI;
using Object = UnityEngine.Object;

namespace Roi.Perf
{
    public sealed partial class FloatingForgeSmoke : MonoBehaviour
    {
        public const string Output = "/private/tmp/roi-floating-forge-native";
        static bool Interactive => Environment.GetCommandLineArgs().Contains("-roiforgepreview");
        static bool RespawnProbe => Environment.GetCommandLineArgs().Contains("-roirespawnsmoke");
        static bool ExitProbe => Environment.GetCommandLineArgs().Contains("-roiexitsmoke");
        static bool DragProbe => Environment.GetCommandLineArgs().Contains("-roidragsmoke");
        static bool AutoFusionProbe => Environment.GetCommandLineArgs().Contains("-roiautofusionsmoke");
        static bool FullStatsProbe => Environment.GetCommandLineArgs().Contains("-roifullstatssmoke");
        static bool TrainingProbe => Environment.GetCommandLineArgs().Contains("-roitrainingsmoke");
        static bool Requested => DragProbe || AutoFusionProbe || ExitProbe || RespawnProbe || FullStatsProbe || TrainingProbe || Interactive || Environment.GetCommandLineArgs().Contains("-roiforgesmoke");
        readonly List<string> _checks = new();
        float _deadline;
        int _runtimeErrors;
        static bool _hadScale, _hadUiScale;
        static int _originalScale, _originalUiScale;
        FloatingInventoryHost _host;
        CompanionWidget _companion;
        SimDriver _driver;

        [RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.BeforeSceneLoad)]
        static void Isolate()
        {
            if (!Requested) return;
            _hadScale=PlayerPrefs.HasKey(CompanionWidget.GameScalePreference);
            _originalScale=PlayerPrefs.GetInt(CompanionWidget.GameScalePreference,2);
            PlayerPrefs.DeleteKey(CompanionWidget.GameScalePreference);
            _hadUiScale=PlayerPrefs.HasKey(CompanionWidget.UiScalePreference);
            _originalUiScale=PlayerPrefs.GetInt(CompanionWidget.UiScalePreference,1);
            PlayerPrefs.DeleteKey(CompanionWidget.UiScalePreference);
            Directory.CreateDirectory(Output);
            DevSaveStore.TestDirectoryOverride = Path.Combine(Output, "save-" + DateTime.UtcNow.Ticks);
            Environment.SetEnvironmentVariable("ROI_BOOT", "dev");
            Environment.SetEnvironmentVariable("ROI_STEAM_PIPE", "0");
        }
        [RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.AfterSceneLoad)]
        static void Install()
        {
            if (!Requested) return;
            var go = new GameObject("~FloatingForgeSmoke");
            DontDestroyOnLoad(go); go.AddComponent<FloatingForgeSmoke>();
        }
        void OnApplicationQuit()
        {
            if(_hadScale)PlayerPrefs.SetInt(CompanionWidget.GameScalePreference,_originalScale);
            else PlayerPrefs.DeleteKey(CompanionWidget.GameScalePreference);
            if(_hadUiScale)PlayerPrefs.SetInt(CompanionWidget.UiScalePreference,_originalUiScale);
            else PlayerPrefs.DeleteKey(CompanionWidget.UiScalePreference);
            PlayerPrefs.Save();
        }
        void OnEnable() => Application.logMessageReceived += ObserveLog;
        void OnDisable() => Application.logMessageReceived -= ObserveLog;
        void ObserveLog(string message,string trace,LogType type)
        { if(type==LogType.Error||type==LogType.Exception||type==LogType.Assert)_runtimeErrors++; }
        void Start() { _deadline=Time.realtimeSinceStartup+100; StartCoroutine(Guard(Drive())); }
        void Update() { if (Time.realtimeSinceStartup>_deadline) Fail("Probe timed out"); }
        void Fail(string error)
        { Debug.LogError("FORGE NATIVE FAIL: " + error); File.WriteAllLines(Path.Combine(Output,"verification.txt"),_checks.Concat(new[]{"FAIL: "+error})); Application.Quit(1); enabled=false; }
        IEnumerator Guard(IEnumerator run)
        {
            while (true)
            {
                object next;
                try { if (!run.MoveNext()) yield break; next=run.Current; }
                catch(Exception e) { Fail(e.ToString()); yield break; }
                yield return next;
            }
        }
        void Check(bool ok,string message)
        { if(!ok)throw new Exception(message); _checks.Add("PASS: "+message);Debug.Log("FORGE NATIVE PASS: "+message); }
        Button Find(Transform root,string name)=>root.GetComponentsInChildren<Button>(true).First(b=>b.name==name&&b.gameObject.activeInHierarchy);
        void Click(Button button)
        {
            Canvas.ForceUpdateCanvases();
            var rect=(RectTransform)button.transform;
            var point=RectTransformUtility.WorldToScreenPoint(null,rect.TransformPoint(rect.rect.center));
            var scroll=button.GetComponentInParent<ScrollRect>();
            if(scroll!=null&&!RectTransformUtility.RectangleContainsScreenPoint(scroll.viewport,point))
            {
                float centerY=-scroll.content.InverseTransformPoint(rect.TransformPoint(rect.rect.center)).y;
                float range=Mathf.Max(0,scroll.content.rect.height-scroll.viewport.rect.height);
                scroll.verticalNormalizedPosition=range<=0?1:1-Mathf.Clamp01((centerY-scroll.viewport.rect.height*.5f)/range);
                Canvas.ForceUpdateCanvases();
                point=RectTransformUtility.WorldToScreenPoint(null,rect.TransformPoint(rect.rect.center));
            }
            var data=new PointerEventData(EventSystem.current){position=point,button=PointerEventData.InputButton.Left};
            var hits=new List<RaycastResult>();EventSystem.current.RaycastAll(data,hits);
            var target=hits.Select(h=>ExecuteEvents.GetEventHandler<IPointerClickHandler>(h.gameObject)).FirstOrDefault(g=>g!=null);
            Check(target==button.gameObject&&button.IsInteractable(),"raycast reaches "+button.name+" at "+point+" (hit "+(target?.name??"none")+")");
            var timer=System.Diagnostics.Stopwatch.StartNew();
            ExecuteEvents.Execute(target,data,ExecuteEvents.pointerClickHandler);
            timer.Stop();
            Debug.Log($"FORGE CLICK {button.name}: {timer.Elapsed.TotalMilliseconds:0.00}ms");
        }
        void Geometry(string stage,int minWidth,int minHeight)
        {
            Check(Screen.width>=minWidth&&Screen.height>=minHeight,stage+" native size "+Screen.width+"x"+Screen.height);
            Check(WindowMode.IsTopmost,stage+" native window is floating");
            if(!_host.IsOpen)return;
            var frame=(RectTransform)_host.transform.Find("WindowGroup");
            var corners=new Vector3[4];frame.GetWorldCorners(corners);
            var gameplay=new Vector3[4];((RectTransform)_companion.transform.Find("Root")).GetWorldCorners(gameplay);
            Check(corners[0].x>=0&&corners[2].x<=Screen.width+1&&corners[0].y>=gameplay[2].y+9&&corners[2].y<=Screen.height+1
                &&gameplay[0].x>=0&&gameplay[2].x<=Screen.width+1&&gameplay[0].y>=0,
                stage+" complete management UI ABOVE gameplay: UI "+corners[0]+" to "+corners[2]+", gameplay "+gameplay[0]+" to "+gameplay[2]);
        }
        Vector2 PaneSize()
        {
            var view=_companion.transform.Find("Root/Pane0/View").GetComponent<RawImage>();
            var size=((RectTransform)view.transform.parent).rect.size;
            Check(view.texture.width==Mathf.Max(1,(int)size.x-4)&&view.texture.height==Mathf.Max(1,(int)size.y-4)
                &&Mathf.Abs((float)view.texture.width/view.texture.height-(size.x-4)/(size.y-4))<.01f,
                "gameplay render matches physical pane pixels without supersampling "+size+" / "+view.texture.width+"x"+view.texture.height);
            return size;
        }
        void FitsDesktop()
        {
            var area=Screen.mainWindowDisplayInfo.workArea;
            Check(Screen.width<=area.width&&Screen.height<=area.height,"selected game scale fits desktop "+Screen.width+"x"+Screen.height+" within "+area);
        }
        Vector2 UiSize()
        {
            var corners=new Vector3[4];((RectTransform)_host.transform.Find("WindowGroup")).GetWorldCorners(corners);
            return corners[2]-corners[0];
        }
        void TransparentFrame()
        {
            Check(WindowMode.IsTransparent,"native window and compositor layer permit transparency");
            var frame=new Texture2D(Screen.width,Screen.height,TextureFormat.RGBA32,false);
            frame.ReadPixels(new Rect(0,0,Screen.width,Screen.height),0,0);frame.Apply();
            float gap=frame.GetPixel(2,100).a;
            float game=frame.GetPixel(Screen.width/2,150).a;
            float panel=frame.GetPixel(24,Screen.height-100).a;
            File.WriteAllBytes(Path.Combine(Output,"11-transparent.png"),frame.EncodeToPNG());
            Object.Destroy(frame);
            Check(gap<.02f&&game>.95f&&panel>.9f,"framebuffer gap is transparent while gameplay and UI stay opaque: "+gap+" / "+game+" / "+panel);
        }
        IEnumerator Shot(string name)
        {yield return new WaitForEndOfFrame();ScreenCapture.CaptureScreenshot(Path.Combine(Output,name+".png"));yield return new WaitForSecondsRealtime(.3f);}
        IEnumerator KeyPress(Key key)
        {
            InputSystem.QueueStateEvent(Keyboard.current,new KeyboardState(key));
            yield return null; yield return null;
            InputSystem.QueueStateEvent(Keyboard.current,new KeyboardState());
            yield return null; yield return new WaitForSecondsRealtime(.7f);
        }
        IEnumerator StressResize()
        {
            _deadline=Time.realtimeSinceStartup+180;
            long before=UnityEngine.Profiling.Profiler.GetAllocatedMemoryForGraphicsDriver();
            for(int i=0;i<24;i++)
            {
                Click(Find(_companion.transform,"GameScale"+(1+i%4)));
                yield return new WaitForSecondsRealtime(.25f);
                Click(Find(_companion.transform,"UiScale"+(i%3)));
                yield return new WaitForSecondsRealtime(.25f);
            }
            Click(Find(_companion.transform,"GameScale2"));
            Click(Find(_companion.transform,"UiScale1"));
            yield return new WaitForSecondsRealtime(3);
            var frames=new List<float>(8192);
            float end=Time.realtimeSinceStartup+60;
            while(Time.realtimeSinceStartup<end)
            { yield return null; frames.Add(Time.unscaledDeltaTime*1000); }
            frames.Sort();
            long after=UnityEngine.Profiling.Profiler.GetAllocatedMemoryForGraphicsDriver();
            var textures=_companion.GetComponentsInChildren<RawImage>(true).Select(i=>i.texture).OfType<RenderTexture>().ToArray();
            long bytes=textures.Sum(t=>UnityEngine.Profiling.Profiler.GetRuntimeMemorySizeLong(t));
            Debug.Log($"FORGE STRESS: {frames.Count} frames, avg={frames.Average():0.00}ms p95={frames[(int)(frames.Count*.95f)]:0.00}ms max={frames.Last():0.00}ms; graphics before={before} after={after}; pane textures={textures.Length}, bytes={bytes}");
            Geometry("after repeated native resizes",846,800);FitsDesktop();PaneSize();
        }
        IEnumerator Drive()
        {
            while((_host=Object.FindFirstObjectByType<FloatingInventoryHost>())==null||(_driver=Object.FindFirstObjectByType<SimDriver>())?.Session==null)yield return null;
            _companion=Object.FindFirstObjectByType<CompanionWidget>(FindObjectsInactive.Include);
            yield return new WaitForSecondsRealtime(1);
            Check(WindowMode.Supported,"standalone native window route (not Editor)");
            var s=SimSession.NewStarter(71,SimSession.LoadCatalog(),"zone_b1_1","Native Forge Preview");_driver.Attach(s);
            for(int i=0;i<14;i++)s.Debug("{\"grant_item\":{\"def_id\":\""+(i%2==0?"sword_s1":"shield_s1")+"\"}}");
            for(int i=0;i<16;i++)s.Debug("{\"grant_item\":{\"def_id\":\"gem_strife_g0\"}}");
            for(int i=0;i<12;i++)s.Debug("{\"grant_item\":{\"def_id\":\"gem_life_g0\"}}");
            s.Debug("{\"give_currency\":{\"gil\":12000,\"soul\":320}}");
            int originalW=Screen.width,originalH=Screen.height;
            bool originalTop=WindowMode.IsTopmost;
            bool originalTransparent=WindowMode.IsTransparent;
            _companion.Toggle();yield return new WaitForSecondsRealtime(1);
            if (DragProbe)
            { yield return Guard(CheckItemDrag()); yield break; }
            if (AutoFusionProbe)
            { yield return Guard(CheckAutoFusion()); yield break; }
            if (ExitProbe)
            { yield return Guard(CheckCompanionExit()); yield break; }
            if (RespawnProbe)
            { yield return Guard(CheckRespawn()); yield break; }
            if (FullStatsProbe)
            { yield return Guard(CheckFullStats()); yield break; }
            if (TrainingProbe)
            { yield return Guard(CheckTraining()); yield break; }
            if (Interactive)
            {
                _host.Open(); _host.Side(1); _host.Model.Mode(false);
                _host.Model.AutoFill(); _host.RefreshNow();
                _deadline=float.PositiveInfinity;
                Debug.Log("FORGE INTERACTIVE READY — isolated equipment/gems fixture; manual controls enabled, no automatic clicks or exit.");
                yield break;
            }
            Check(_companion.GameScale==2&&_companion.UiScale==1.5f,"fresh companion defaults to game 2x and UI 1.5x");
            Geometry("closed",336,264);
            Click(Find(_companion.transform,"GameScale1"));yield return new WaitForSecondsRealtime(.6f);
            var small=PaneSize();
            Check(Screen.width==336&&Screen.height==264&&small==new Vector2(320,180),"1x restores original 320x180 gameplay plus controls");
            Click(Find(_companion.transform,"GameScale2"));yield return new WaitForSecondsRealtime(.6f);
            var medium=PaneSize();
            Check(medium.x>=small.x*1.9f&&medium.y>=small.y*1.9f&&_companion.GameScale==2,"2x visibly doubles gameplay dimensions");
            foreach(int scale in new[]{3,4})
            {
                Click(Find(_companion.transform,"GameScale"+scale));yield return new WaitForSecondsRealtime(.6f);
                var large=PaneSize();
                Check(large.x>=medium.x&&large.y>=medium.y&&_companion.GameScale==scale&&PlayerPrefs.GetInt(CompanionWidget.GameScalePreference)==scale,"size button applies and remembers "+scale+"x");
                FitsDesktop();
            }
            yield return Shot("10-game-4x");
            Click(Find(_companion.transform,"GameScale2"));yield return new WaitForSecondsRealtime(.6f);
            yield return Shot("01-widget");
            Click(Find(_companion.transform,"UI_FLOAT_TITLE"));yield return new WaitForSecondsRealtime(1);
            Check(_host.IsOpen&&_host.CompanionContext,"toolbar opens shared inventory while native HudBar is hidden");
            Geometry("primary",516,800);yield return Shot("02-inventory");
            Click(Find(_host.transform,"UI_BLACKSMITH"));yield return new WaitForSecondsRealtime(1);
            Geometry("forge",846,800);FitsDesktop();
            Click(Find(_companion.transform,"UiScale0"));yield return new WaitForSecondsRealtime(.6f);
            var uiSmall=UiSize();
            Check(Mathf.Abs(uiSmall.x-FloatingInventoryHost.Width)<1&&Mathf.Abs(uiSmall.y-FloatingInventoryHost.Height)<1,"UI 1x matches the current main-plus-side panel dimensions");
            foreach(int index in new[]{1,2})
            {
                Click(Find(_companion.transform,"UiScale"+index));yield return new WaitForSecondsRealtime(.6f);
                float factor=1+index*.5f;var actual=UiSize();
                Check(Mathf.Abs(actual.x/uiSmall.x-factor)<.01f&&Mathf.Abs(actual.y/uiSmall.y-factor)<.01f
                    &&_companion.GameScale==2&&PlayerPrefs.GetInt(CompanionWidget.UiScalePreference)==index,
                    "UI scale grows panel/text together independently of game: "+factor+"x -> "+actual);
                Geometry("UI scale "+factor,846,800);FitsDesktop();
            }
            yield return Shot("12-ui-2x");
            Click(Find(_companion.transform,"UiScale1"));yield return new WaitForSecondsRealtime(.6f);
            Click(Find(_companion.transform,"GameScale4"));yield return new WaitForSecondsRealtime(.6f);
            Geometry("forge at 4x",846,800);FitsDesktop();PaneSize();
            Click(Find(_companion.transform,"GameScale2"));yield return new WaitForSecondsRealtime(.6f);
            Click(Find(_host.transform,"UI_FORGE_AUTO"));yield return Shot("03-forge");
            yield return new WaitForEndOfFrame();TransparentFrame();
            int count=_host.Model.Bag.Length;ulong tick=_driver.Tick;
            Click(Find(_host.transform,"UI_FORGE_COMBINE"));
            Check(_host.Result.Visible&&ForgeModel.Items(s.QueryJson("inventory")).Length==count-5,"native gear fuse consumes six and shows actual result");
            Check(_host.transform.Find("WindowGroup/Result").GetComponent<CanvasGroup>().alpha==0,
                "Result starts transparent synchronously before its first rendered frame");
            yield return new WaitForSecondsRealtime(.4f);
            yield return Shot("04-result");yield return new WaitForSecondsRealtime(1);
            Check(_driver.Tick>tick,"combat advances with native Result open");
            _host.Back();_host.Model.Mode(true);_host.RefreshNow();Click(Find(_host.transform,"UI_FORGE_AUTO"));Click(Find(_host.transform,"UI_FORGE_COMBINE"));
            var receipt=ForgeModel.Parse<ForgeModel.Receipt>(s.QueryJson("forge_receipt"));
            Check(receipt?.kind=="gems"&&receipt.status=="success"&&receipt.gil==0,"native gem fusion returns actual reward");yield return new WaitForSecondsRealtime(.4f);yield return Shot("05-gem-result");
            _host.Back();_host.Side(2);yield return new WaitForSecondsRealtime(.6f);
            Geometry("socket",846,800);
            int socketIndex=Array.FindIndex(_host.Model.Bag,it=>it.gem_slots>0);
            var bag=_host.GetComponentsInChildren<ScrollRect>(true).First(r=>r.name=="BagScroll").content;
            Click(bag.GetComponentsInChildren<Button>().Where(b=>b.name=="Item").ElementAt(socketIndex));
            var socket=_host.transform.Find("WindowGroup/SocketGems");
            yield return Shot("06-before-socket");
            Click(socket.GetComponentsInChildren<Button>().First(b=>b.name=="UI_FORGE_GEMS"&&b.interactable&&b.transform.parent.name=="Content"));
            // A user sees the newly created confirmation on the next rendered frame.
            yield return Shot("09-confirmation");
            Click(Find(_host.transform,"UI_FLOAT_CONFIRM"));
            Check(ForgeModel.Items(s.QueryJson("inventory"))[socketIndex].gems?.Length==1,"native socket action inserts a real held gem");
            yield return Shot("06-socket");
            yield return KeyPress(Key.Digit3);
            Geometry("three panes",846,800);FitsDesktop();
            Check(_companion.Report.Contains("panes=3"),"pane shortcut works with HudBar hidden");
            yield return Shot("08-three-panes");
            if(Environment.GetCommandLineArgs().Contains("-roiforgestress")) yield return StressResize();
            yield return KeyPress(Key.Digit1);
            _host.Side(0);yield return new WaitForSecondsRealtime(.6f);
            Geometry("secondary closed",516,800);
            _host.Close();yield return new WaitForSecondsRealtime(.6f);
            Check(Screen.width==656&&Screen.height==444&&_companion.GameScale==2,"closing inventory restores selected 2x gameplay window 656x444");
            Click(Find(_companion.transform,"UI_FLOAT_CLOSE"));yield return new WaitForSecondsRealtime(1);
            Check(!_companion.Visible&&!_host.CompanionContext,"toolbar exits Companion");
            Check(WindowMode.IsTransparent==originalTransparent,"return to Battle restores native window opacity");
            Check(Screen.width==originalW&&Screen.height==originalH&&WindowMode.IsTopmost==originalTop,"restores original game window and topmost state");
            yield return KeyPress(Key.W);
            Check(_companion.Visible&&_companion.GameScale==2,"W enters native Companion and retains selected game scale");
            yield return KeyPress(Key.I);
            Check(_host.IsOpen,"I opens inventory while HudBar is inactive");
            yield return KeyPress(Key.Escape);
            Check(!_host.IsOpen,"Escape closes primary without exiting Companion");
            yield return KeyPress(Key.W);
            Check(!_companion.Visible&&Screen.width==originalW,"W restores Battle without double toggling");
            _host.Open();_host.Side(1);yield return new WaitForSecondsRealtime(.3f);
            Check(_host.IsOpen,"same primary and forge remain usable back in Battle");yield return Shot("07-battle");
            Check(_runtimeErrors==0,"no runtime exceptions or errors during native smoke");
            File.WriteAllLines(Path.Combine(Output,"verification.txt"),_checks);
            Debug.Log("FORGE NATIVE COMPLETE "+_checks.Count+" checks");Application.Quit(0);
        }

        IEnumerator CheckFullStats()
        {
            _host.Open(); _host.Side(0); _host.RefreshNow();
            yield return new WaitForSecondsRealtime(.6f);
            var button=Find(_host.transform,"UI_FLOAT_FULL_STATS");
            Check(button.interactable,"full stats button is enabled for the active character");
            Click(button); yield return new WaitForSecondsRealtime(.4f);
            var panel=_host.transform.Find("WindowGroup/FullStatsPanel");
            Check(_host.FullStatsOpen&&panel!=null&&panel.gameObject.activeSelf,"full stats panel opens from the character view");
            Check(_host.WindowWidth==(int)FloatingInventoryHost.Width,"opening full stats reserves the left-side panel width");
            Check(panel.GetComponentsInChildren<TMP_Text>(true).Any(t=>t.text==Loc.Get("UI_FLOAT_STAT_COMBAT"))
                &&panel.GetComponentsInChildren<TMP_Text>(true).Any(t=>t.text==Loc.Get("UI_FLOAT_STAT_PROGRESSION")),
                "full stats panel renders combat and progression sections");
            yield return Shot("18-full-stats");
            _host.Back(); yield return null;
            Check(!_host.FullStatsOpen&&_host.WindowWidth==FloatingInventoryHost.MainWidth,"Back closes full stats and restores the primary panel width");
            File.WriteAllLines(Path.Combine(Output,"verification-full-stats.txt"),_checks);
            Debug.Log("FULL STATS NATIVE COMPLETE "+_checks.Count+" checks"); Application.Quit(0);
        }
    }
}
#endif
