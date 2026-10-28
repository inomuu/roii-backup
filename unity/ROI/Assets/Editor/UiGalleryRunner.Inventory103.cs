using System.Collections;
using System.Linq;
using System.IO;
using Roi.UI;
using Roi.Sim;
using UnityEngine;
using UnityEngine.UI;
using UnityEngine.EventSystems;

public sealed partial class UiGalleryRunner
{
    IEnumerator Inventory103Drive()
    {
        var bar=HudBar.Create(transform);yield return null;bar.Toggle("Inventory");yield return new WaitForSecondsRealtime(.5f);
        var host=bar.Floating;var session=host.Model.Driver.Session;var browser=host.GetComponentInChildren<CollectionBrowser>();
        yield return Shot("Inventory-first-open");
        string[] ids={"sword_s1","dagger_s1","greatsword_s1","bow_s1","rod_s1","mace_s1","armor_s1","ring_s1","necklace_s1","shield_s1","gloves_s1","shoes_s1"};
        foreach(var id in ids)session.Debug("{\"grant_item\":{\"def_id\":\""+id+"\"}}");
        for(int i=0;i<10;i++)session.Debug("{\"grant_item\":{\"def_id\":\"shield_s1\"}}");
        host.RefreshNow();yield return new WaitForSecondsRealtime(.3f);
        yield return Shot("Inventory-grid");
        browser.SelectCategory(1);browser.SelectEntry(0);yield return Shot("Inventory-weapon");
        browser.SelectCategory(2);yield return null;
        browser.transform.Find("SafeContent/Sort").GetComponent<Button>().onClick.Invoke();yield return null;
        var snap=ExemplarEquipmentActions.Read(host.Model.Driver,host.MemberId);
        var chosen=browser.VisibleEntries.FirstOrDefault(e=>e.Item.def_id=="shield_s1"&&e.Row.Index>5&&ExemplarEquipmentActions.FreezeEquip(snap,e.Row)!=null);
        if(chosen==null){Debug.LogError("Inventory103 no equip candidate");Finish86();yield break;}
        var inventoryBefore=session.QueryJson("inventory");browser.Pick(chosen);yield return null;
        if(session.QueryJson("inventory")!=inventoryBefore)Debug.LogError("Inventory103 inspection mutated inventory");
        yield return Shot("Inventory-selected");CheckCollection103(browser);
        string expectedInventory=ExistingEquipInventory103(host.Model.Driver,host.MemberId,chosen.Row);
        browser.PrimaryButton.onClick.Invoke();yield return null;host.RefreshNow();
        if(session.QueryJson("inventory")!=expectedInventory)Debug.LogError("Inventory103 action differs from existing equip validation");
        yield return Shot("Inventory-action-feedback");
        browser.SelectCategory(2);yield return null;browser.SelectEntry(0);yield return null;
        session.Debug("{\"grant_item\":{\"def_id\":\"shield_s1\"}}");
        var staleBefore=session.QueryJson("inventory");browser.PrimaryButton.onClick.Invoke();yield return null;
        if(session.QueryJson("inventory")!=staleBefore)Debug.LogError("Inventory103 stale choice applied");
        browser.SelectCategory(4);yield return Shot("Inventory-empty-category");
        if(browser.PrimaryButton.interactable)Debug.LogError("Inventory103 hidden selection actionable");
        browser.SelectCategory(0);browser.GridScroll.verticalNormalizedPosition=0;yield return Shot("Inventory-grid-bottom");
        if(browser.GridScroll.content.anchoredPosition.y<=0)Debug.LogError("Inventory103 grid does not scroll");
        browser.Tools();yield return null;
        if(host.ActiveSide!=FloatingInventoryHost.SideForge||browser.gameObject.activeSelf)Debug.LogError("Inventory103 workshop route failed");
        yield return Shot("Workshop-route");host.Side(FloatingInventoryHost.SideNone);yield return null;
        if(!browser.gameObject.activeSelf)Debug.LogError("Inventory103 workshop return failed");
        bar.CloseAll();bar.SetHomeMode(true);
        var canvas=Canvas86();var scaler=canvas.GetComponent<CanvasScaler>();scaler.referenceResolution=new Vector2(1920,1080);scaler.matchWidthOrHeight=1;
        var screen=ExemplarScreen.Create(canvas.transform,()=>{});screen.Refresh();screen.SelectPage("Equipment");yield return null;
        var page=Find86<ExemplarEquipmentPage>();Call86(page,"SelectSlot",1);Call86(page,"Primary");yield return new WaitForSecondsRealtime(.2f);
        var picker=Find86<ExemplarEquipmentPicker>();var pickView=picker.GetComponentInChildren<CollectionBrowser>();
        yield return Shot("Equipment-picker");inventoryBefore=session.QueryJson("inventory");
        pickView.SelectCategory(1);pickView.SelectEntry(2);yield return null;
        if(session.QueryJson("inventory")!=inventoryBefore)Debug.LogError("Inventory103 picker preview applied input");
        yield return Shot("Equipment-selected");CheckCollection103(pickView);
        pickView.SelectCategory(2);yield return null;
        if(pickView.PrimaryButton.interactable)Debug.LogError("Inventory103 picker filtered selection still actionable");
        pickView.Back();yield return null;
        if(picker.gameObject.activeSelf||session.QueryJson("inventory")!=inventoryBefore)Debug.LogError("Inventory103 cancel failed");
        Call86(page,"SelectSlot",0);Call86(page,"Primary");yield return null;pickView.SelectCategory(1);
        var expected=pickView.VisibleEntries.First(e=>e.Item.def_id=="sword_s1");pickView.Pick(expected);yield return null;
        expectedInventory=ExistingEquipInventory103(host.Model.Driver,screen.InspectedId,expected.Row);
        yield return Shot("Equipment-weapon-comparison");pickView.PrimaryButton.onClick.Invoke();yield return null;
        if(session.QueryJson("inventory")!=expectedInventory)Debug.LogError("Inventory103 picker differs from existing equip validation");
        yield return Shot("Equipment-action-feedback");
        screen.gameObject.SetActive(false);bar.SetHomeMode(false);host.Open();yield return null;browser.SelectCategory(2);browser.SelectEntry(0);yield return null;
        string language=Loc.Language;Loc.Language="th";host.Close();Object.Destroy(browser.gameObject);yield return null;
        Call86(host,"BuildCollection");host.Open();yield return null;browser=host.GetComponentInChildren<CollectionBrowser>();browser.SelectCategory(2);browser.SelectEntry(0);yield return null;
        yield return Shot("Inventory-thai");CheckCollection103(browser);
        float gridOffset=browser.GridScroll.content.anchoredPosition.y;
        browser.DetailScroll.verticalNormalizedPosition=0;yield return null;
        if(browser.DetailScroll.content.anchoredPosition.y<=0||Mathf.Abs(browser.GridScroll.content.anchoredPosition.y-gridOffset)>.1f)Debug.LogError("Inventory103 detail scroll affects grid");
        Loc.Language=language;
        File.WriteAllText(Path.Combine(_output,"checks.txt"),"UX-only revision. Checked inspection without mutation; sorted row actions preserve existing equip validation (including refusal); stale choice rejection; filter selection clearing; workshop and return; picker preview/cancel; mobile hit geometry; English and Thai. No equip rules changed. Disposable native session only.");
        Finish86();
    }
    string ExistingEquipInventory103(SimDriver driver,long member,ExemplarEquipmentActions.Row row)
    {
        var before=ExemplarEquipmentActions.Read(driver,member);
        string original=driver.Session.QueryJson("inventory");
        using(var copy=driver.Session.Clone())
        {
            bool applied=copy.ApplyInput("{\"EquipItem\":{\"entity_id\":"+member+",\"inventory_idx\":"+row.Index+"}}");
            var verify=typeof(ExemplarEquipmentActions).Assembly.GetType("Roi.UI.ExemplarEquipmentTransfer").GetMethod("Verify",System.Reflection.BindingFlags.Static|System.Reflection.BindingFlags.NonPublic);
            bool valid=applied&&(bool)verify.Invoke(null,new object[]{copy,before,row,null});
            File.AppendAllText(Path.Combine(_output,"action-results.txt"),row.Item.def_id+": existing validation "+(valid?"accepted":"refused")+"\n");
            return valid?copy.QueryJson("inventory"):original;
        }
    }
    void CheckCollection103(CollectionBrowser browser)
    {
        var report=new System.Text.StringBuilder();
        var canvas=browser.GetComponentInParent<Canvas>();var raycaster=canvas.GetComponent<GraphicRaycaster>();
        foreach(var button in browser.GetComponentsInChildren<Button>())
        {
            if(!button.interactable||button.name.StartsWith("Item_")||button.name=="Sort")continue;
            var rect=(RectTransform)button.transform;var corners=new Vector3[4];rect.GetWorldCorners(corners);
            float height=Vector3.Distance(corners[0],corners[1]);report.AppendLine(button.name+" "+height.ToString("0.0")+"px");
            if(height<44)Debug.LogError("Inventory103 touch target too short: "+button.name);
            var center=RectTransformUtility.WorldToScreenPoint(null,rect.TransformPoint(rect.rect.center));
            var data=new PointerEventData(EventSystem.current){position=center};var hits=new System.Collections.Generic.List<RaycastResult>();raycaster.Raycast(data,hits);
            if(hits.Count==0||hits[0].gameObject.GetComponentInParent<Button>()!=button)Debug.LogError("Inventory103 blocked button "+button.name);
        }
        File.AppendAllText(Path.Combine(_output,"targets.txt"),report.ToString());
    }
}

