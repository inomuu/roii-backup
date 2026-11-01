// ROI > UI > Floating Forge Preview. Isolated fixtures never touch player saves or the server.
using System;
using System.Collections;
using System.IO;
using System.Linq;
using Roi.Sim;
using Roi.UI;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.UI;
using Object=UnityEngine.Object;

public static class FloatingForgeProbe
{
    static int _frames;
    public static string Output="/private/tmp/roi-floating-forge";
    [MenuItem("ROI/UI/Floating Forge Preview")]
    public static void Run()
    {
        if(EditorApplication.isPlaying){Debug.LogError("FORGE PROBE: leave Play mode before starting the isolated fixture.");return;}
        Directory.CreateDirectory(Output);
        DevSaveStore.TestDirectoryOverride=Path.Combine(Output,"save-"+DateTime.UtcNow.Ticks);
        SessionState.SetString("ROI_BOOT","dev");SessionState.SetString("ROI_STEAM_PIPE","0");
        EditorSettings.enterPlayModeOptionsEnabled=true;
        EditorSettings.enterPlayModeOptions=EnterPlayModeOptions.DisableDomainReload|EnterPlayModeOptions.DisableSceneReload;
        EditorSceneManager.OpenScene(ProbeScene.Path,OpenSceneMode.Single);
        EditorApplication.playModeStateChanged+=Entered;
        EditorApplication.EnterPlaymode();
    }
    static void Entered(PlayModeStateChange change)
    {
        if(change!=PlayModeStateChange.EnteredPlayMode)return;
        EditorApplication.playModeStateChanged-=Entered;
        PlayModeWindow.SetCustomRenderingResolution(1600,1000,"Floating Forge preview");
        _frames=0;EditorApplication.update+=Wait;
    }
    static void Wait()
    {
        var host=Object.FindFirstObjectByType<FloatingInventoryHost>();
        var driver=Object.FindFirstObjectByType<SimDriver>();
        if(++_frames<100||host==null||driver?.Session==null)
        {if(_frames>2400){EditorApplication.update-=Wait;Debug.LogError("FORGE PROBE: no ready session");}return;}
        EditorApplication.update-=Wait;host.StartCoroutine(Drive(host,driver));
    }
        static void Check(bool ok,string message)
    { if(!ok)throw new Exception("FORGE PROBE FAIL: "+message);Debug.Log("FORGE PROBE PASS: "+message); }
    public static void CaptureArena()
    {
        var host=Object.FindFirstObjectByType<FloatingInventoryHost>();
        host.Close();host.StartCoroutine(Shot("arena-fixed"));
    }
static Button Button(Transform root,string name)=>root.GetComponentsInChildren<Button>(true).First(b=>b.name==name&&b.gameObject.activeInHierarchy);
    static IEnumerator Shot(string name)
    {yield return new WaitForSecondsRealtime(.4f);ScreenCapture.CaptureScreenshot(Path.Combine(Output,name+".png"));yield return new WaitForSecondsRealtime(.25f);}
    static IEnumerator Drive(FloatingInventoryHost host,SimDriver driver)
    {
        var session=SimSession.NewStarter(71,SimSession.LoadCatalog(),"zone_b1_1","Forge Preview");
        Check(session!=null,"isolated starter session");driver.Attach(session);
        for(int i=0;i<14;i++)session.Debug("{\"grant_item\":{\"def_id\":\""+(i%2==0?"sword_s1":"shield_s1")+"\"}}");
        for(int i=0;i<12;i++)session.Debug("{\"grant_item\":{\"def_id\":\"gem_strife_g0\"}}");
        for(int i=0;i<4;i++)session.Debug("{\"grant_item\":{\"def_id\":\"gem_life_g0\"}}");
        session.Debug("{\"give_currency\":{\"gil\":12000,\"soul\":320}}");
        host.Open();host.Side(1);host.Model.Mode(false);host.RefreshNow();
        Button(host.transform,"UI_FORGE_AUTO").onClick.Invoke();
        Check(host.Model.Count==6&&host.Model.CanSubmit,"Auto-fill chooses six actual gear instances");
        Check(host.Model.Gear.Select(p=>p.Index).Distinct().Count()==6,"no duplicate indices");
        yield return Shot("01-equipment");
        var count=ForgeModel.Items(session.QueryJson("inventory")).Length;
        Button(host.transform,"UI_FORGE_COMBINE").onClick.Invoke();
        Check(ForgeModel.Items(session.QueryJson("inventory")).Length==count-5,"gear fusion consumes six and awards one");
        Check(!host.Result.Visible,"actual acquisition keeps the result dialog hidden");
        ulong before=driver.Tick;
        yield return Shot("02-result");yield return new WaitForSecondsRealtime(.6f);
        Check(driver.Tick>before,"combat ticks continue while Result is open");
        host.Result.Hide();
        host.Side(2);host.RefreshNow();
        // The upgraded gear has a real gem socket; select its actual bag cell and confirm insertion.
        int socketIndex=Array.FindIndex(host.Model.Bag,it=>it.gem_slots>0);
        Check(socketIndex>=0,"fusion output exposes gem slots");
        var bagContent=host.transform.Find("WindowGroup/CharacterInventory/BagScroll/Viewport/Content");
        if(bagContent==null)bagContent=host.GetComponentsInChildren<ScrollRect>(true).First(s=>s.name=="BagScroll").content;
        var cells=bagContent.GetComponentsInChildren<Button>().Where(b=>b.name=="Item").ToArray();
        cells[socketIndex].onClick.Invoke();host.RefreshNow();
        var socketPanel=host.transform.Find("WindowGroup/SocketGems");
        var gemChoice=socketPanel.GetComponentsInChildren<Button>().First(b=>b.name=="UI_FORGE_GEMS"&&b.interactable&&b.transform.parent.name=="Content");
        gemChoice.onClick.Invoke();
        Button(host.transform,"UI_FLOAT_CONFIRM").onClick.Invoke();
        Check(ForgeModel.Items(session.QueryJson("inventory"))[socketIndex].gems?.Length==1
            &&ForgeModel.Items(session.QueryJson("inventory")).Where(m=>m.def_id=="gem_life_g0").Sum(m=>m.quantity)==3,
            "socket confirmation consumes a held gem and updates equipment");
        Check(host.InspectedIndex==socketIndex&&host.Inspected?.gems?.Length==1,"socket result keeps the updated item selected");
        yield return Shot("07-socket");
        // Preserve the old gem until replacement has been explicitly confirmed.
        gemChoice=socketPanel.GetComponentsInChildren<Button>().First(b=>b.name=="UI_FORGE_GEMS"&&b.interactable&&b.transform.parent.name=="Content");
        string beforeCancel=session.QueryJson("inventory");
        gemChoice.onClick.Invoke();Button(host.transform,"UI_FLOAT_CANCEL").onClick.Invoke();
        Check(session.QueryJson("inventory")==beforeCancel,"cancel overwrite leaves gems and equipment unchanged");
        for(int i=0;i<3;i++)
        {
            gemChoice=socketPanel.GetComponentsInChildren<Button>().First(b=>b.name=="UI_FORGE_GEMS"&&b.interactable&&b.transform.parent.name=="Content");
            gemChoice.onClick.Invoke();Button(host.transform,"UI_FLOAT_CONFIRM").onClick.Invoke();
        }
        Check(host.InspectedIndex==socketIndex-1&&host.Inspected?.gems?.Length==1
            &&!ForgeModel.Items(session.QueryJson("inventory")).Any(m=>m.def_id=="gem_life_g0"),
            "using the final gem before equipment follows its shifted inventory index");
        int confirmations=0;
        host.Confirm("Duplicate click probe",()=>confirmations++);
        var confirm=Button(host.transform,"UI_FLOAT_CONFIRM");confirm.onClick.Invoke();confirm.onClick.Invoke();
        Check(confirmations==1,"double confirmation executes only once");
        host.Confirm("Cancelled click probe",()=>confirmations++);
        confirm=Button(host.transform,"UI_FLOAT_CONFIRM");Button(host.transform,"UI_FLOAT_CANCEL").onClick.Invoke();confirm.onClick.Invoke();
        Check(confirmations==1,"cancelled confirmation cannot run a stale callback");
        host.Side(1);host.Model.Mode(true);host.RefreshNow();
        Button(host.transform,"UI_FORGE_AUTO").onClick.Invoke();
        Check(host.Model.Gems.Count==6,"gem Auto-fill fills one batch");
        host.Model.Clear();
        for(int i=0;i<6;i++)host.Model.PickGem("gem_strife_g0");
        host.RefreshNow();
        Check(host.Model.Gems.Count==6&&host.Model.Gems.Distinct().Count()==1,"gem quantity permits six from one inventory stack");
        yield return Shot("03-gems");
        Button(host.transform,"UI_FORGE_COMBINE").onClick.Invoke();
        var receipt=ForgeModel.Parse<ForgeModel.Receipt>(session.QueryJson("forge_receipt"));
        Check(receipt?.status=="success"&&receipt.kind=="gems"&&receipt.gil==0,"gem receipt has actual random-set reward and no Gil");
        yield return Shot("04-gem-result");host.Result.Hide();
        host.Model.Mode(false);host.Model.AutoFill();host.RefreshNow();
        var draft=host.Model.DraftKey;var removed=host.Model.Gear[0].Index;
        driver.ApplyInput("{\"DiscardItem\":{\"inv_idx\":"+removed+"}}");
        int afterDiscard=ForgeModel.Items(session.QueryJson("inventory")).Length;
        host.Model.Submit(draft);
        Check(ForgeModel.Items(session.QueryJson("inventory")).Length==afterDiscard&&!host.Model.CanSubmit,"shifted inventory invalidates draft without spending replacements");
        host.Model.AutoFill();host.RefreshNow();
        var companion=Object.FindFirstObjectByType<CompanionWidget>(FindObjectsInactive.Include);
        Check(companion!=null,"Companion entry exists");companion.Toggle();
        Check(host.CompanionContext&&host.IsOpen,"same open primary and secondary survive Companion switch");
        before=driver.Tick;yield return Shot("05-companion");
        Check(driver.Tick>before,"combat ticks continue in Companion with inventory open");
        companion.Toggle();host.Side(0);
        Check(host.IsOpen&&host.ActiveSide==0,"closing secondary preserves primary");
        var bar=Object.FindFirstObjectByType<HudBar>();
        // The current inventory layout exposes the original deposit screen from Stash.
        bar.Toggle("Stash");yield return null;
        var stash=Object.FindFirstObjectByType<ItemGridWindow>();
        var openBag=stash.GetComponentsInChildren<Button>().First(b=>b.GetComponentInChildren<TMPro.TMP_Text>()?.text==Loc.Get("UI_STASH_INVENTORY"));
        openBag.onClick.Invoke();
        Check(!host.IsOpen,"stash entry reaches original inventory actions");
        bar.Toggle("Inventory");
        Check(host.IsOpen,"return from original inventory restores floating primary");
        host.Side(1);host.Model.Mode(false);host.Model.AutoFill();host.RefreshNow();
        yield return Shot("06-ready");
        Debug.Log("FORGE PROBE COMPLETE — preview left open; fixtures saved only under /private/tmp.");
    }
}
