using System.IO;
using Roi.UI;
using TMPro;
using UnityEditor;
using UnityEngine;
using UnityEngine.UI;

// Disposable visual fixture; renders the real component without a session or network.
public static class ClearReferenceCapture
{
    public static void Run()
    {
        string folder=Path.GetFullPath(Path.Combine(Application.dataPath,"../../Captures/clear-reference"));
        Directory.CreateDirectory(folder);
        var host=new GameObject("Clear reference capture");
        var rt=new RenderTexture(1440,810,24);
        var previous=RenderTexture.active;
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
            scaler.referenceResolution=new Vector2(1280,720);scaler.matchWidthOrHeight=1;
            var clear=ClearSummary.Create(canvas.transform);
            clear.OnView("{\"last_clear\":{\"instance_id\":\"reference\",\"ticks\":1230,\"kills\":20,\"xp\":100,\"gil\":50,\"failed\":false,\"loot\":[{\"def_id\":\"shield_s1\",\"name\":\"Bronze Shield\",\"quantity\":1}]}}");
            clear.enabled=false;
            foreach(var tr in host.GetComponentsInChildren<Transform>(true))tr.gameObject.layer=30;
            Canvas.ForceUpdateCanvases();
            foreach(var text in host.GetComponentsInChildren<TextMeshProUGUI>())text.ForceMeshUpdate();
            Canvas.ForceUpdateCanvases();cam.Render();
            RenderTexture.active=rt;
            var texture=new Texture2D(1440,810,TextureFormat.RGB24,false);
            texture.ReadPixels(new Rect(0,0,1440,810),0,0);texture.Apply();
            File.WriteAllBytes(Path.Combine(folder,"Dungeon-clear.png"),texture.EncodeToPNG());
            Object.DestroyImmediate(texture);
            clear.OnPointerClick(new UnityEngine.EventSystems.PointerEventData(null) { button=UnityEngine.EventSystems.PointerEventData.InputButton.Left });
            if(clear.gameObject.activeSelf)throw new System.Exception("Tap-to-close failed");
            clear.OnView("{\"last_clear\":{\"instance_id\":\"reference\"}}");
            if(clear.gameObject.activeSelf)throw new System.Exception("Dismissed run reopened");
            File.WriteAllText(Path.Combine(folder,"result.txt"),"Rendered real ClearSummary at 1440x810. Tap-to-close and dismissed-run guard passed. No session or account changes.");
            Debug.Log("CLEAR_REFERENCE_CAPTURE_OK "+folder);
        }
        finally
        {
            RenderTexture.active=previous;
            Object.DestroyImmediate(host);rt.Release();Object.DestroyImmediate(rt);
        }
    }
}
