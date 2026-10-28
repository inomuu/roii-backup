using System;
using System.IO;
using Roi.Cutin;
using Roi.Sim;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using UnityEngine.UI;
using Object = UnityEngine.Object;

public static class SeraphineCutinPreview
{
    const string Output = "/tmp/roi-seraphine-cutin";

    [MenuItem("ROI/Cut-in/Seraphine Aegis/Play once")]
    public static void Play()
    {
        if (EditorApplication.isPlaying) SeraphineUltimateCutin.Play();
        else Debug.Log("AEGIS CUTIN: enter play mode, or use Capture preview.");
    }

    [MenuItem("ROI/Cut-in/Seraphine Aegis/Capture preview")]
    public static void Capture()
    {
        Directory.CreateDirectory(Output);
        var scene = EditorSceneManager.NewPreviewScene();
        var previous = RenderTexture.active;
        var backdrop = new Texture2D(2,2,TextureFormat.RGBA32,false);
        try
        {
            var path = Path.GetFullPath(Path.Combine(Application.dataPath,
                "../../../docs/art-review-r1/painterly-v2/unity-play.png"));
            if (!backdrop.LoadImage(File.ReadAllBytes(path))) throw new InvalidOperationException("Missing arena preview image.");
            var aura = Resources.Load<Texture2D>("Cutin/SeraphineAuraFlow-v1");
            Require(aura != null,"missing generated aura texture");
            var importer = AssetImporter.GetAtPath(AssetDatabase.GetAssetPath(aura)) as TextureImporter;
            Require(importer != null && !importer.sRGBTexture && importer.mipmapEnabled &&
                importer.wrapMode == TextureWrapMode.Mirror,"aura mask import settings changed");
            CheckPauseOwnership(scene);
            CaptureSize(scene, backdrop, 1280, 720, true, "landscape");
            CaptureSize(scene, backdrop, 1680, 720, false, "ultrawide");
            CaptureSize(scene, backdrop, 540, 960, false, "portrait");
            Debug.Log("AEGIS CUTIN: capture passed at 16:9, 21:9 and 9:16. " + Output);
        }
        finally
        {
            RenderTexture.active = previous;
            EditorSceneManager.ClosePreviewScene(scene);
            Object.DestroyImmediate(backdrop);
        }
    }

    static void CaptureSize(Scene scene, Texture2D backdrop, int width, int height, bool motion, string name)
    {
        var cameraGo = new GameObject("AegisPreviewCamera");
        SceneManager.MoveGameObjectToScene(cameraGo, scene);
        var camera = cameraGo.AddComponent<Camera>();
        camera.scene = scene;
        camera.orthographic = true;
        camera.orthographicSize = 5;
        camera.transform.position = new Vector3(0,0,-10);
        camera.clearFlags = CameraClearFlags.SolidColor;
        camera.backgroundColor = Color.black;
        camera.enabled = false;
        var target = new RenderTexture(width,height,24,RenderTextureFormat.ARGB32);
        var readback = new Texture2D(width,height,TextureFormat.RGB24,false);
        camera.targetTexture = target;
        SeraphineUltimateCutin cutin = null;
        try
        {
            cutin = SeraphineUltimateCutin.Create(false);
            if (cutin == null) throw new InvalidOperationException("Cut-in did not build.");
            SceneManager.MoveGameObjectToScene(cutin.gameObject,scene);
            var canvas = cutin.Surface;
            canvas.renderMode = RenderMode.ScreenSpaceCamera;
            canvas.worldCamera = camera;
            canvas.planeDistance = 1;
            var bg = new GameObject("ArenaStill",typeof(RectTransform)).AddComponent<RawImage>();
            bg.transform.SetParent(canvas.transform,false);
            bg.transform.SetAsFirstSibling();
            bg.texture = backdrop;
            bg.raycastTarget = false;
            bg.rectTransform.anchorMin = Vector2.zero;
            bg.rectTransform.anchorMax = Vector2.one;
            bg.rectTransform.offsetMin = bg.rectTransform.offsetMax = Vector2.zero;
            float sourceAspect = backdrop.width/(float)backdrop.height;
            float frameAspect = width/(float)height;
            float crop = Mathf.Min(1,frameAspect/sourceAspect);
            bg.uvRect = new Rect((1-crop)*.5f,0,crop,1);
            Canvas.ForceUpdateCanvases();
            Render(-.1f,"before-"+name+".png");
            var before = readback.GetPixels32();
            Render(SeraphineUltimateCutin.Duration*.55f,"hero-"+name+".png");
            int changed = Difference(before,readback.GetPixels32());
            if (changed < width*height/3) throw new InvalidOperationException("Blank cut-in: "+name);
            if (motion)
            {
                var textured = readback.GetPixels32();
                var material = canvas.transform.Find("Cinematic").GetComponent<RawImage>().material;
                Require(material.GetTexture("_AuraTex") != null,"aura texture not bound");
                material.SetFloat("_AuraDetail",0);
                Render(SeraphineUltimateCutin.Duration*.55f,"hero-procedural-"+name+".png");
                int textureChanged = Difference(textured,readback.GetPixels32());
                material.SetFloat("_AuraDetail",1);
                Require(textureChanged > width*height/100,"aura texture has no visible effect");
                Debug.Log("AEGIS CUTIN: generated texture A/B changed="+textureChanged);
            }
            Render(SeraphineUltimateCutin.Duration+.1f,"after-"+name+".png");
            if (Difference(before,readback.GetPixels32(),0) != 0) throw new InvalidOperationException("Cut-in did not clear: "+name);
            Debug.Log("AEGIS CUTIN: "+name+" changed="+changed+"; end restores backdrop exactly.");
            if (motion)
                for (int i=0;i<Mathf.RoundToInt((SeraphineUltimateCutin.Duration+.9f)*30);i++)
                    Render(i/30f-.3f,"motion-"+i.ToString("D3")+".png");
            Render(.12f,"entry-early-"+name+".png");
            Render(.28f,"entry-late-"+name+".png");
            cutin.enabled = false;
            if (canvas.enabled) throw new InvalidOperationException("Disabled cut-in remains visible.");

            void Render(float t,string file)
            {
                cutin.Sample(t,frameAspect);
                Canvas.ForceUpdateCanvases();
                camera.Render();
                RenderTexture.active=target;
                readback.ReadPixels(new Rect(0,0,width,height),0,0);
                readback.Apply();
                File.WriteAllBytes(Path.Combine(Output,file),readback.EncodeToPNG());
            }
        }
        finally
        {
            if (cutin != null) Object.DestroyImmediate(cutin.gameObject);
            Object.DestroyImmediate(cameraGo);
            RenderTexture.active=null;
            target.Release();
            Object.DestroyImmediate(target);
            Object.DestroyImmediate(readback);
        }
    }

    static void CheckPauseOwnership(Scene scene)
    {
        var go = new GameObject("PreviewSimDriver");
        SceneManager.MoveGameObjectToScene(go,scene);
        var driver = go.AddComponent<SimDriver>();
        var cutin = SeraphineUltimateCutin.Create(false);
        if (cutin == null) throw new InvalidOperationException("Cut-in assets unavailable.");
        SceneManager.MoveGameObjectToScene(cutin.gameObject,scene);
        try
        {
            cutin.HoldSimulation(driver);
            Require(driver.Paused,"cut-in did not pause sim");
            driver.Paused = false;
            Require(driver.Paused,"another owner released cut-in pause");
            driver.Paused = true;
            cutin.enabled = false;
            Require(driver.Paused,"cut-in disable released manual pause");
            driver.Paused = false;
            Require(!driver.Paused,"cut-in disable leaked pause");
            cutin.enabled = true;
            cutin.HoldSimulation(driver);
            driver.HoldPresentation(go);
            cutin.Stop();
            Require(driver.Paused,"cut-in stop released another hold");
            driver.ReleasePresentation(go);
            Require(!driver.Paused,"cut-in stop leaked pause");
            Debug.Log("AEGIS CUTIN: independent pause, disable and stop checks passed.");
        }
        finally
        {
            if (cutin != null) Object.DestroyImmediate(cutin.gameObject);
            Object.DestroyImmediate(go);
        }
    }

    static void Require(bool condition,string reason)
    {
        if (!condition) throw new InvalidOperationException("AEGIS CUTIN: "+reason);
    }

    static int Difference(Color32[] a,Color32[] b,int tolerance=12)
    {
        int changed=0;
        for(int i=0;i<a.Length;i++)
            if (Math.Abs(a[i].r-b[i].r)+Math.Abs(a[i].g-b[i].g)+Math.Abs(a[i].b-b[i].b)>tolerance) changed++;
        return changed;
    }
}
