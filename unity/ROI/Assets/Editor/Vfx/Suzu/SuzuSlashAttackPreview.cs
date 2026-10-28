using System;
using System.IO;
using System.Collections.Generic;
using Roi.Arena;
using Roi.Editor.Vfx;
using UnityEditor;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;
using Object=UnityEngine.Object;
using static Roi.Editor.Vfx.SuzuSunderFixture;

public static class SuzuSlashAttackPreview
{
    const string Output="/tmp/roi-suzu-slash-attacks";

    public static void CaptureDirectional()
    {
        Capture();
        SuzuSlashBaseTexturePreview.Capture();
    }

    [MenuItem("ROI/Art/Capture Suzu Slash Attack Variants")]
    public static void Capture()
    {
        if (EditorApplication.isPlaying) throw new InvalidOperationException("Capture outside play mode.");
        Directory.CreateDirectory(Output); SuzuBaseWedgeTexture.Build();
        using var f=new SuzuSunderFixture(baseOnly:true);
        VerifyLighting(f);
        var fingerprints=new List<Color32[]>();
        foreach (SuzuSlashAttackStudy.Style style in Enum.GetValues(typeof(SuzuSlashAttackStudy.Style)))
        {
            using var study=new SuzuSlashAttackStudy(f,style);
            string name=style.ToString().ToLowerInvariant();
            Viewport(f,study,new Vector2Int(960,540),name,true,fingerprints);
            Viewport(f,study,new Vector2Int(540,960),name+"-portrait",false,null);
            Debug.Log("SUZU ATTACK CAPTURED: "+name);
        }
        Debug.Log("SUZU ATTACK VARIANTS PASS: "+Output);
    }

    static void VerifyLighting(SuzuSunderFixture f)
    {
        var volumes=new List<Volume>();
        foreach (var root in f.Camera.scene.GetRootGameObjects())
            foreach (var volume in root.GetComponentsInChildren<Volume>())
                if (volume.enabled) volumes.Add(volume);
        var target=new RenderTexture(960,540,24,RenderTextureFormat.ARGBHalf);
        var pixels=new Texture2D(960,540,TextureFormat.RGB24,false);
        var active=RenderTexture.active; var previous=f.Camera.targetTexture;
        try
        {
            f.FrameCamera(new Vector2Int(960,540)); f.Sample(0); f.Camera.targetTexture=target;
            foreach (var volume in volumes) volume.enabled=false;
            double before=Read("lighting-before");
            foreach (var volume in volumes) volume.enabled=true;
            double after=Read("lighting-after");
            Require(after>before*1.05,"preview lighting did not brighten the rendered scene");
            foreach (var volume in volumes) volume.enabled=false;
            double broken=Read("lighting-sabotage");
            Reject(()=>Require(broken>before*1.05,"disabled preview lighting"));
            Debug.Log($"SUZU PREVIEW LIGHTING: mean luma {before:F2} -> {after:F2}; disabled-volume sabotage caught.");
        }
        finally
        {
            foreach (var volume in volumes) volume.enabled=true;
            f.Camera.targetTexture=previous; RenderTexture.active=active;
            Object.DestroyImmediate(target); Object.DestroyImmediate(pixels);
        }
        double Read(string name)
        {
            f.Camera.Render(); f.Camera.Render(); RenderTexture.active=target;
            pixels.ReadPixels(new Rect(0,0,960,540),0,0); pixels.Apply();
            File.WriteAllBytes(Path.Combine(Output,name+".png"),pixels.EncodeToPNG());
            double sum=0;
            foreach (var pixel in pixels.GetPixels32()) sum+=.2126*pixel.r+.7152*pixel.g+.0722*pixel.b;
            return sum/(960*540);
        }
    }

    static void Viewport(SuzuSunderFixture f,SuzuSlashAttackStudy study,Vector2Int size,string name,
        bool motion,List<Color32[]> fingerprints)
    {
        f.FrameCamera(size); var camera=f.Camera;
        var target=new RenderTexture(size.x,size.y,24,RenderTextureFormat.ARGBHalf) { antiAliasing=4 };
        var pixels=new Texture2D(size.x,size.y,TextureFormat.RGB24,false);
        camera.targetTexture=target;
        try
        {
            study.Sample(-.1f); camera.Render(); camera.Render();
            Verify(f,study,pixels,Read,name,fingerprints);
            if (motion) for (int i=0;i<Frames;i++)
            {
                study.Sample(i/(float)Fps-Start); Read("motion-"+name+"-"+i.ToString("D3"));
            }
            foreach (float age in new[] { .56f,.76f,1.06f })
            { study.Sample(age); Read(name+"-"+Mathf.RoundToInt(age*100)); }
        }
        finally { camera.targetTexture=null; Object.DestroyImmediate(target); Object.DestroyImmediate(pixels); }
        void Read(string file)
        {
            var active=RenderTexture.active;
            try
            {
                camera.Render(); RenderTexture.active=target;
                pixels.ReadPixels(new Rect(0,0,size.x,size.y),0,0); pixels.Apply();
                File.WriteAllBytes(Path.Combine(Output,file+".png"),pixels.EncodeToPNG());
            }
            finally { RenderTexture.active=active; }
        }
    }

    static void Verify(SuzuSunderFixture f,SuzuSlashAttackStudy study,Texture2D pixels,Action<string> read,
        string name,List<Color32[]> fingerprints)
    {
        var camera=f.Camera; var data=camera.GetUniversalAdditionalCameraData();
        bool post=data.renderPostProcessing,flip=f.Body.flipX; int mask=camera.cullingMask;
        var clear=camera.clearFlags; var background=camera.backgroundColor;
        int used=0;
        foreach (var root in camera.scene.GetRootGameObjects())
            foreach (var t in root.GetComponentsInChildren<Transform>(true)) used|=1<<t.gameObject.layer;
        int layer=31; while (layer>=8 && (used & (1<<layer))!=0) layer--;
        Require(layer>=8,"no free attack-test layer");
        var layers=new List<int>();
        foreach (var fx in study.Effects) { layers.Add(fx.gameObject.layer); fx.gameObject.layer=layer; }
        try
        {
            camera.cullingMask=1<<layer; camera.clearFlags=CameraClearFlags.SolidColor;
            camera.backgroundColor=Color.black; data.renderPostProcessing=false;
            study.Sample(-.1f); read("empty-"+name); var empty=pixels.GetPixels32();
            for (int i=0;i<study.Effects.Count;i++)
            {
                var fx=study.Effects[i]; var renderer=fx.GetComponent<Renderer>();
                Require(!ShaderUtil.ShaderHasError(renderer.sharedMaterial.shader),"attack shader error");
                study.Hide(); fx.Sample(.64f); read("pulse-"+i+"-"+name);
                Require(Energy(empty,pixels.GetPixels32())>2000,"attack pulse invisible");
                Framed(pixels.GetPixels32(),pixels.width,pixels.height);
                fx.Hide(); read("hidden-"+i+"-"+name);
                Reject(()=>Require(Energy(empty,pixels.GetPixels32())>2000,"hidden pulse"));
            }
            study.Sample(.76f); read("combined-"+name); var peak=pixels.GetPixels32();
            Require(Energy(empty,peak)>2000,"combined attack invisible");
            if (fingerprints!=null)
            {
                foreach (var previous in fingerprints)
                    Require(Energy(previous,peak)>3000,"two attack styles rendered identically");
                Reject(()=>Require(Energy(peak,peak)>3000,"duplicate style"));
                fingerprints.Add(peak);
            }
            study.Sample(.98f); read("travel-"+name);
            Require(Energy(peak,pixels.GetPixels32())>3000,"attack has no UV motion");
            study.Sample(.76f); read("frozen-travel-"+name);
            Reject(()=>Require(Energy(peak,pixels.GetPixels32())>3000,"frozen attack"));
            study.Sample(2); read("expired-"+name);
            Require(Energy(empty,pixels.GetPixels32())==0,"expired attack visible");
            study.Sample(.76f); read("expiry-sabotage-"+name);
            Reject(()=>Require(Energy(empty,pixels.GetPixels32())==0,"frozen expiry"));
            study.Sample(.76f,false); read("cancel-"+name);
            Require(Energy(empty,pixels.GetPixels32())==0,"cancelled attack visible");
            study.Sample(.76f); read("restart-"+name);
            Require(Energy(peak,pixels.GetPixels32())==0,"attack restart not repeatable");
            f.Body.flipX=!flip; study.Sample(.76f); read("mirrored-"+name);
            Require(Energy(empty,pixels.GetPixels32())>2000,"mirrored attack invisible");
            Framed(pixels.GetPixels32(),pixels.width,pixels.height);
            var clipped=(Color32[])peak.Clone(); clipped[0]=new Color32(255,255,255,255);
            Reject(()=>Framed(clipped,pixels.width,pixels.height));
            Debug.Log("SUZU ATTACK VERIFIED: "+name+" pulses="+study.Effects.Count
                +"; visible/framed/moving/distinct/cancel/restart/expiry/mirror passed; hidden/frozen/duplicate/clipped sabotages caught");
        }
        finally
        {
            f.Body.flipX=flip; study.Sample(-.1f);
            for (int i=0;i<study.Effects.Count;i++) study.Effects[i].gameObject.layer=layers[i];
            camera.cullingMask=mask; camera.clearFlags=clear; camera.backgroundColor=background; data.renderPostProcessing=post;
        }
    }
    static void Framed(Color32[] pixels,int width,int height)
    {
        for (int y=0;y<height;y++) for (int x=0;x<width;x++)
            if ((x<2 || x>=width-2 || y<2 || y>=height-2) && pixels[y*width+x].r>30)
                Require(false,"attack clipped by viewport");
    }
    static long Energy(Color32[] a,Color32[] b)
    {
        long sum=0;
        for (int i=0;i<a.Length;i++) sum+=Math.Abs(a[i].r-b[i].r)+Math.Abs(a[i].g-b[i].g)+Math.Abs(a[i].b-b[i].b);
        return sum;
    }
    static void Reject(Action check)
    {
        bool caught=false; try { check(); } catch (InvalidOperationException) { caught=true; }
        Require(caught,"attack guard survived sabotage");
    }
}
