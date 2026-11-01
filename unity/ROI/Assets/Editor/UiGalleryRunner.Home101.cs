using System.Collections;
using System.IO;
using Roi.UI;
using UnityEngine;
using UnityEngine.UI;

public sealed partial class UiGalleryRunner
{
    IEnumerator Home101Drive()
    {
        // Same canvas scale as the shipped ModeTabs, with disposable route callbacks.
        var canvas=Canvas86();var scaler=canvas.GetComponent<CanvasScaler>();
        scaler.referenceResolution=new Vector2(1920,1080);scaler.matchWidthOrHeight=1;
        string route=null;
        HomeMenuLayout.Build(canvas.transform,id=>route=id);
        yield return new WaitForSecondsRealtime(1);
        var home=canvas.GetComponentInChildren<HomeMenuLayout>();
        var ledger=home.transform.Find("SafeContent/Expedition");
        yield return Shot("Home");DumpTypography("Home");CheckHome102(home);
        foreach(var id in new[]{"Exemplars","Party","Inventory","Album","Excavation"}) {
            HomeButton(home,id).onClick.Invoke();yield return null;
            string expected=id=="Workshop"?"Apothecary":id=="Excavation"?"Summon":id;
            if(route!=expected)Debug.LogError("Home101 route failed: "+id);
        }
        HomeButton(home,"More").onClick.Invoke();yield return Shot("More");
        var scroll=home.GetComponentInChildren<ScrollRect>();
        if(scroll==null||scroll.content.childCount!=19)Debug.LogError("Home101 lost secondary routes");
        else {scroll.verticalNormalizedPosition=0;yield return Shot("More-bottom");}
        home.CloseGroup();yield return null;
        HomeButton(home,"Store").onClick.Invoke();yield return Shot("Store");home.CloseGroup();yield return null;
        ledger.Find("Combat").GetComponent<Button>().onClick.Invoke();
        if(route!="Combat")Debug.LogError("Home101 battle failed");
        foreach(var key in new[]{"Mail","Options"}) {
            home.transform.Find("SafeContent/"+key).GetComponent<Button>().onClick.Invoke();
            if(route!=key)Debug.LogError("Home101 utility route failed: "+key);
        }
        var report=new System.Text.StringBuilder();
        foreach(var b in home.GetComponentsInChildren<Button>()) {
            var corners=new Vector3[4];((RectTransform)b.transform).GetWorldCorners(corners);
            report.AppendLine(b.name+" "+Vector3.Distance(corners[0],corners[1]).ToString("0.0")+"px high");
            if(_output.EndsWith("mobile")&&Vector3.Distance(corners[0],corners[1])<40)Debug.LogError("Home101 small primary hit target: "+b.name);
        }
        File.WriteAllText(Path.Combine(_output,"targets.txt"),report.ToString());
        string lang=Loc.Language;Loc.Language="th";
        Object.Destroy(home.gameObject);yield return null;
        HomeMenuLayout.Build(canvas.transform,id=>route=id);yield return new WaitForSecondsRealtime(.5f);
        yield return Shot("Home-thai");DumpTypography("Home-thai");CheckHome102(canvas.GetComponentInChildren<HomeMenuLayout>());Loc.Language=lang;
        canvas.SetActive(false);
        var modes=ModeTabs.Create(transform);modes.Select(false);yield return new WaitForSecondsRealtime(.5f);
        var live=modes.GetComponentInChildren<HomeMenuLayout>();
        if(modes.transform.Find("QuickNavigation").gameObject.activeSelf)Debug.LogError("Home101 duplicate battle switch visible");
        yield return Shot("Home-live");
        live.transform.Find("SafeContent/Expedition/Combat").GetComponent<Button>().onClick.Invoke();
        if(!modes.IsCombat)Debug.LogError("Home101 live battle route failed");
        Finish86();
    }
    void CheckHome102(HomeMenuLayout home)
    {
        var raycaster=home.GetComponentInParent<GraphicRaycaster>();
        foreach(var id in new[]{"Combat","Exemplars","Party","Inventory","Album","Store","More","Excavation"}) {
            var button=HomeButton(home,id);
            var rect=(RectTransform)button.transform;
            var center=RectTransformUtility.WorldToScreenPoint(raycaster.eventCamera,rect.TransformPoint(rect.rect.center));
            var pointer=new UnityEngine.EventSystems.PointerEventData(UnityEngine.EventSystems.EventSystem.current){position=center};
            var hits=new System.Collections.Generic.List<UnityEngine.EventSystems.RaycastResult>();raycaster.Raycast(pointer,hits);
            if(hits.Count==0||hits[0].gameObject.GetComponentInParent<Button>()!=button)Debug.LogError("Home102 blocked touch target: "+id);
        }
    }
    static Button HomeButton(HomeMenuLayout home,string id)
    {
        foreach(var b in home.GetComponentsInChildren<Button>())if(b.name==id)return b;
        throw new System.Exception("Missing home button: "+id);
    }
    Vector2 HomeInkY(TMPro.TextMeshProUGUI text)
    {
        float low=float.PositiveInfinity,high=float.NegativeInfinity;
        foreach(var c in text.textInfo.characterInfo) {
            if(!c.isVisible||c.textElement==null)continue;
            var metrics=c.textElement.glyph.metrics;
            float top=c.baseLine+metrics.horizontalBearingY*c.scale;
            low=Mathf.Min(low,top-metrics.height*c.scale);high=Mathf.Max(high,top);
        }
        return new Vector2(text.transform.TransformPoint(new Vector3(0,low,0)).y,text.transform.TransformPoint(new Vector3(0,high,0)).y);
    }
}
