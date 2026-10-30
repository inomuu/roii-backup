using System.IO;
using Roi.UI;
using TMPro;
using UnityEditor;
using UnityEngine;
using UnityEngine.UI;

// Disposable visual fixture; renders the real component without a session or network.
public static class HomeInlineMenuCapture
{
    public static void Run()
    {
        string folder=Path.GetFullPath(Path.Combine(Application.dataPath,"../../Captures/home-inline-menu"));
        Directory.CreateDirectory(folder);
        var host=new GameObject("Home inline menu capture");
        var rt=new RenderTexture(1440,900,24);
        var previous=RenderTexture.active;
        var previousLanguage=Loc.Language;Loc.Language="th";
        try
        {
            var cam=host.AddComponent<Camera>();
            cam.enabled=false;cam.clearFlags=CameraClearFlags.SolidColor;
            cam.backgroundColor=Color.black;cam.cullingMask=1<<30;
            cam.orthographic=true;cam.targetTexture=rt;
            var canvasObject=new GameObject("Canvas",typeof(RectTransform),typeof(Canvas),typeof(CanvasScaler));
            canvasObject.transform.SetParent(host.transform,false);
            var canvas=canvasObject.GetComponent<Canvas>();
            canvas.renderMode=RenderMode.ScreenSpaceCamera;canvas.worldCamera=cam;canvas.planeDistance=1;
            var scaler=canvasObject.GetComponent<CanvasScaler>();
            scaler.uiScaleMode=CanvasScaler.ScaleMode.ScaleWithScreenSize;
            scaler.referenceResolution=new Vector2(1440,900);scaler.matchWidthOrHeight=1;
            string route=null;
            HomeMenuLayout.Build(canvas.transform,id=>route=id);
            var home=host.GetComponentInChildren<HomeMenuLayout>();
            foreach(var group in host.GetComponentsInChildren<CanvasGroup>())group.alpha=1;
            var menu=home.transform.Find("SafeContent/Expedition/HomeMenu").GetComponent<ScrollRect>();
            foreach(var tr in host.GetComponentsInChildren<Transform>(true))tr.gameObject.layer=30;
            Canvas.ForceUpdateCanvases();
            var flags=System.Reflection.BindingFlags.Instance|System.Reflection.BindingFlags.NonPublic;
            typeof(HomeMenuLayout).GetMethod("Update",flags).Invoke(home,null);
            var wallet=host.GetComponentInChildren<HomeWalletBar>();
            typeof(HomeWalletBar).GetMethod("LateUpdate",flags).Invoke(wallet,null);
            foreach(var text in host.GetComponentsInChildren<TextMeshProUGUI>())text.ForceMeshUpdate();
            Canvas.ForceUpdateCanvases();cam.Render();
            RenderTexture.active=rt;
            var texture=new Texture2D(1440,900,TextureFormat.RGB24,false);
            texture.ReadPixels(new Rect(0,0,1440,900),0,0);texture.Apply();
            File.WriteAllBytes(Path.Combine(folder,"Home.png"),texture.EncodeToPNG());
            Object.DestroyImmediate(texture);
            home.OpenMore();
            if(home.transform.Find("GroupOverlay")!=null)throw new System.Exception("More opened a dialog");
            Canvas.ForceUpdateCanvases();cam.Render();RenderTexture.active=rt;
            texture=new Texture2D(1440,900,TextureFormat.RGB24,false);
            texture.ReadPixels(new Rect(0,0,1440,900),0,0);texture.Apply();
            File.WriteAllBytes(Path.Combine(folder,"Home-more.png"),texture.EncodeToPNG());
            Object.DestroyImmediate(texture);
            foreach(var button in menu.content.GetComponentsInChildren<Button>())
            {
                if(button.name=="Store")continue;
                route=null;button.onClick.Invoke();
                string expected=button.name=="Excavation"?"Summon":button.name;
                if(route!=expected)throw new System.Exception("Bad home route: "+button.name+" -> "+route);
            }
            File.WriteAllText(Path.Combine(folder,"result.txt"),"Rendered Home and expanded menu. All direct menu routes passed; More scrolls inline without opening a dialog.");
            Debug.Log("HOME_INLINE_CAPTURE_OK "+folder);
        }
        finally
        {
            Loc.Language=previousLanguage;RenderTexture.active=previous;
            Object.DestroyImmediate(host);rt.Release();Object.DestroyImmediate(rt);
        }
    }
}
