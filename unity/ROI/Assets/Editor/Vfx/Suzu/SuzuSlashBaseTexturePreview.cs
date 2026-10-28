using System;
using System.IO;
using Roi.Arena;
using Roi.Editor.Vfx;
using UnityEditor;
using UnityEngine;
using UnityEngine.Rendering.Universal;
using Object=UnityEngine.Object;
using static Roi.Editor.Vfx.SuzuSunderFixture;

public static class SuzuSlashBaseTexturePreview
{
    const string Output="/tmp/roi-suzu-base-texture";

    [MenuItem("ROI/Art/Capture Suzu Base Texture")]
    public static void Capture()
    {
        if (EditorApplication.isPlaying) throw new InvalidOperationException("Capture outside play mode.");
        Directory.CreateDirectory(Output); SuzuBaseWedgeTexture.Build();
        using (var fixture=new SuzuSunderFixture(baseOnly:true))
            foreach (var size in new[] { new Vector2Int(1280,720),new Vector2Int(540,960) })
                CaptureViewport(fixture,size);
        Debug.Log("SUZU BASE TEXTURE PASS: "+Output);
    }

    static void CaptureViewport(SuzuSunderFixture f,Vector2Int size)
    {
        string name=size.x==1280 ? "landscape" : "portrait";
        f.FrameCamera(size); var camera=f.Camera;
        var target=new RenderTexture(size.x,size.y,24,RenderTextureFormat.ARGBHalf) { antiAliasing=4 };
        var pixels=new Texture2D(size.x,size.y,TextureFormat.RGB24,false);
        camera.targetTexture=target;
        try
        {
            f.Sample(0); camera.Render(); camera.Render();
            for (int i=0;i<Frames;i++) { f.Sample(i/(float)Fps); Read("motion-"+name+"-"+i.ToString("D3")); }
            f.Sample(Start+.64f); Read("base-"+name);
            Verify(f,pixels,Read,name);
        }
        finally { camera.targetTexture=null; Object.DestroyImmediate(target); Object.DestroyImmediate(pixels); }
        void Read(string file)
        {
            var active=RenderTexture.active;
            try
            {
                camera.Render(); RenderTexture.active=target;
                pixels.ReadPixels(new Rect(0,0,size.x,size.y),0,0); pixels.Apply();
                File.WriteAllBytes(System.IO.Path.Combine(Output,file+".png"),pixels.EncodeToPNG());
            }
            finally { RenderTexture.active=active; }
        }
    }

    static void Verify(SuzuSunderFixture f,Texture2D pixels,Action<string> read,string name)
    {
        var camera=f.Camera; var data=camera.GetUniversalAdditionalCameraData();
        bool post=data.renderPostProcessing,flip=f.Body.flipX;
        int mask=camera.cullingMask,oldLayer=f.BaseEffect.gameObject.layer;
        var clear=camera.clearFlags; var background=camera.backgroundColor;
        int used=0;
        foreach (var root in camera.scene.GetRootGameObjects())
            foreach (var t in root.GetComponentsInChildren<Transform>(true)) used|=1<<t.gameObject.layer;
        int layer=31; while (layer>=8 && (used & (1<<layer))!=0) layer--;
        Require(layer>=8,"no free base-test layer");
        var renderer=f.BaseEffect.GetComponent<MeshRenderer>(); var material=renderer.sharedMaterial;
        var texture=material.GetTexture("_MaskTex"); var block=new MaterialPropertyBlock();
        var mesh=f.BaseEffect.GetComponent<MeshFilter>().sharedMesh; var vertices=mesh.vertices;
        try
        {
            f.BaseEffect.gameObject.layer=layer; camera.cullingMask=1<<layer;
            camera.clearFlags=CameraClearFlags.SolidColor; camera.backgroundColor=Color.black;
            data.renderPostProcessing=false;
            Require(!ShaderUtil.ShaderHasError(material.shader),"base shader error");
            f.Sample(0); read("empty-"+name); var empty=pixels.GetPixels32();
            f.Sample(Start+.64f); read("peak-"+name); var peak=pixels.GetPixels32();
            Require(Energy(empty,peak)>5000,"base invisible");
            foreach (var r in f.Effect.GetComponentsInChildren<Renderer>()) Require(!r.enabled,"support layer leaked into base study");
            foreach (var impact in f.Impacts)
                foreach (var r in impact.GetComponentsInChildren<Renderer>()) Require(!r.enabled,"impact leaked into base study");
            f.BaseEffect.Hide(); read("hidden-"+name);
            Reject(()=>Require(Energy(empty,pixels.GetPixels32())>5000,"hidden base"));
            f.Sample(Start+.64f);
            material.SetTexture("_MaskTex",Texture2D.whiteTexture); read("white-mask-"+name);
            var solid=pixels.GetPixels32(); long detail=Energy(peak,solid);
            Require(detail>5000,"wedge mask did not shape the base");
            read("white-mask-sabotage-"+name);
            Reject(()=>Require(Energy(solid,pixels.GetPixels32())>5000,"missing wedge"));
            material.SetTexture("_MaskTex",texture);

            var matrix=f.BaseEffect.transform.localToWorldMatrix;
            f.Sample(Start+.92f); read("travel-"+name);
            Require(Energy(peak,pixels.GetPixels32())>5000,"UV motion missing");
            renderer.GetPropertyBlock(block); block.SetFloat("_Offset",0); renderer.SetPropertyBlock(block);
            read("frozen-travel-"+name);
            Reject(()=>Require(Energy(peak,pixels.GetPixels32())>5000,"frozen UV clock"));
            foreach (float offset in new[] { -1.2f,1.2f,2.2f })
            {
                block.SetFloat("_Offset",offset); block.SetFloat("_Opacity",1); renderer.SetPropertyBlock(block);
                read("outside-"+offset+"-"+name);
                Require(Energy(empty,pixels.GetPixels32())==0,"mask wrapped back into the window");
            }
            block.SetFloat("_Offset",0); renderer.SetPropertyBlock(block); read("repeat-sabotage-"+name);
            Reject(()=>Require(Energy(empty,pixels.GetPixels32())==0,"repeated mask"));

            long previous=long.MaxValue; float previousAge=0;
            foreach (float age in new[] { .92f,1.05f,1.18f,1.28f })
            {
                f.Sample(Start+age); read("tail-"+age+"-"+name); long energy=Energy(empty,pixels.GetPixels32());
                Require(energy>0 && energy<previous*.95,"base exit did not shrink progressively");
                Require(f.BaseEffect.transform.localToWorldMatrix==matrix,"base geometry moved");
                var current=mesh.vertices;
                for (int i=0;i<current.Length;i++) Require(current[i]==vertices[i],"base vertices deformed");
                if (previous!=long.MaxValue)
                {
                    f.Sample(Start+previousAge); read("frozen-tail-"+name);
                    Reject(()=>Require(Energy(empty,pixels.GetPixels32())<previous*.95,"frozen exit"));
                }
                previous=energy; previousAge=age;
                Debug.Log("SUZU BASE TEXTURE TAIL: "+name+" age="+age+" energy="+energy);
            }
            foreach (float age in new[] { 1.55f,SuzuSlashBaseEffect.Duration+.01f })
            { f.Sample(Start+age); read("ended-"+age+"-"+name); Require(Energy(empty,pixels.GetPixels32())==0,"base remained after exit"); }
            f.Sample(Start+.64f); f.BaseEffect.Sample(.64f,false); read("cancel-"+name);
            Require(Energy(empty,pixels.GetPixels32())==0,"cancelled base visible");
            f.Sample(Start+.64f); read("restart-"+name);
            Require(Energy(peak,pixels.GetPixels32())==0,"base restart not deterministic");
            f.BaseEffect.enabled=false; f.Sample(Start+.64f); read("disabled-"+name);
            Require(Energy(empty,pixels.GetPixels32())==0,"disabled base visible");
            f.BaseEffect.enabled=true; f.Body.flipX=!flip; f.Sample(Start+.64f); read("mirrored-actor-"+name);
            Require(Energy(empty,pixels.GetPixels32())>5000,"mirrored actor lost base mesh");
            Debug.Log("SUZU BASE TEXTURE VERIFIED: "+name+" mask difference="+detail
                +"; hidden/missing-mask/frozen-UV/repeat/frozen-tail sabotages caught; cancel/restart/disabled/mirror passed");
        }
        finally
        {
            material.SetTexture("_MaskTex",texture); f.BaseEffect.enabled=true; f.Body.flipX=flip; f.Sample(0);
            f.BaseEffect.gameObject.layer=oldLayer; camera.cullingMask=mask;
            camera.clearFlags=clear; camera.backgroundColor=background; data.renderPostProcessing=post;
        }
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
        Require(caught,"base guard survived sabotage");
    }
}
