using System;
using System.IO;
using Roi.Editor.Vfx;
using UnityEditor;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.RenderGraphModule;
using UnityEngine.Rendering.Universal;
using UnityEngine.SceneManagement;
using Object=UnityEngine.Object;
using static Roi.Editor.Vfx.SuzuSunderFixture;

public static class MaykerFireSlashPreview
{
    const string Prefab="Assets/Editor/Vfx/References/MaykerStudio/FreeSlashVFX/Prefabs/Slash Fire VFX.prefab";
    const float Hit=Start+.36f;
    const float AccentPlaybackSpeed=1.18f;

    [MenuItem("ROI/Art/Capture Mayker Fire Slash Reference")]
    public static void Capture() => Capture(false);

    [MenuItem("ROI/Art/Capture Mayker Solid Fire Orbit")]
    public static void CaptureOrbit() => Capture(true);

    [MenuItem("ROI/Art/Capture Mayker Fire Orbit With Accents")]
    public static void CaptureOrbitAccents() => Capture(true,true);

    [MenuItem("ROI/Art/Capture Suzu Bell Strike")]
    public static void CaptureBellStrike() => Capture(true,true,true);

    [MenuItem("ROI/Art/Compare Bell Strike Fire Shapes")]
    public static void CaptureBellShapes()
    {
        SuzuBellMeshPreview.CaptureShapes();
        Capture(true,true,true);
        Capture(true,true,true,true);
    }

    [MenuItem("ROI/Art/Compare Bell Quarter And Half Arc Fire")]
    public static void CaptureBellArcs()
    {
        SuzuBellMeshPreview.CaptureArcs();
        Capture(true,true,true,true,false,true);
        Capture(true,true,true,true,true,true);
    }

    [MenuItem("ROI/Art/Compare Vendor Fire Prefab And Bell Strike")]
    public static void CaptureBellVendorComparison()
    {
        CaptureBellVendorDirect();
        Capture(true,true,true,true,true,true);
    }

    [MenuItem("ROI/Art/Capture Vendor Fire Prefab At Bell Strike")]
    public static void CaptureBellVendorDirect() => Capture(false,longArcView:true,vendorBell:true);

    [MenuItem("ROI/Art/Capture Vendor Fire With White Core")]
    public static void CaptureBellVendorWhite() => Capture(false,longArcView:true,vendorBell:true,vendorWhite:true);

    public static void CaptureSuzuSkills()
    {
        CaptureOrbitAccents();
        CaptureBellStrike();
    }

    static void Capture(bool orbit,bool withAccents=false,bool bell=false,bool quarterArc=false,bool halfArc=false,
        bool longArcView=false,bool vendorBell=false,bool vendorWhite=false)
    {
        string output=vendorWhite ? "/tmp/roi-bell-vendor-white" : vendorBell ? "/tmp/roi-bell-vendor-direct" : bell ? "/tmp/roi-suzu-bell-strike-"+(halfArc ? "half-arc" : quarterArc ? "quarter-arc" : "rectangle") : withAccents ? "/tmp/roi-mayker-fire-orbit-accents"
            : orbit ? "/tmp/roi-mayker-fire-orbit" : "/tmp/roi-mayker-fire-slash";
        Require(!EditorApplication.isPlaying,"capture outside play mode");
        Directory.CreateDirectory(output);
        using var f=new SuzuSunderFixture(baseOnly:true);
        if (vendorBell) f.SetTargetVisible(1,false);
        var prefab=AssetDatabase.LoadAssetAtPath<GameObject>(Prefab);
        Require(prefab!=null,"import the Free Slash VFX fire reference first");
        var root=Object.Instantiate(prefab);
        SceneManager.MoveGameObjectToScene(root,f.Camera.scene);
        var ps=root.GetComponent<ParticleSystem>();
        var systems=root.GetComponentsInChildren<ParticleSystem>(true);
        var renderers=root.GetComponentsInChildren<ParticleSystemRenderer>(true);
        var lights=root.GetComponentsInChildren<Light>(true);
        var lightPower=new float[lights.Length];
        MaykerFireOrbitStudy study=null;
        MaykerFireOrbitAccents accents=null;
        SuzuBellStrikeStudy bellStudy=null;
        var originalTint=ps.main.startColor;
        var whiteTint=new ParticleSystem.MinMaxGradient(new Color(1,.97f,.91f,1));
        Quaternion vendorParticleRotation=Quaternion.identity;
        var clocks=new[] {
            new CaptureClock(RenderPassEvent.BeforeRenderingShadows),
            new CaptureClock(RenderPassEvent.BeforeRenderingOpaques),
            new CaptureClock(RenderPassEvent.BeforeRenderingTransparents)
        };
        try
        {
            Require(ps!=null,"reference has no root particle system");
            for (int i=0;i<systems.Length;i++)
            {
                systems[i].Stop(false,ParticleSystemStopBehavior.StopEmittingAndClear);
                systems[i].useAutoRandomSeed=false; systems[i].randomSeed=(uint)(1301+i*17);
                var main=systems[i].main; main.scalingMode=ParticleSystemScalingMode.Hierarchy;
            }
            for (int i=0;i<lights.Length;i++) lightPower[i]=lights[i].intensity;
            foreach (var r in renderers)
                foreach (var material in r.sharedMaterials)
                    Require(material!=null && material.shader.isSupported && !ShaderUtil.ShaderHasError(material.shader),
                        "missing or unsupported vendor material on "+r.name);
            var mainRenderer=ps.GetComponent<ParticleSystemRenderer>();
            Require(mainRenderer.mesh!=null,"reference FBX submesh missing");
            Debug.Log("MAYKER FIRE SOURCE: mesh="+mainRenderer.mesh.name+" bounds="+mainRenderer.mesh.bounds
                +" vertices="+mainRenderer.mesh.vertexCount+" material="+mainRenderer.sharedMaterial.name);
            root.transform.position=f.Actor.position+Vector3.up*f.Height*.58f;
            root.transform.rotation=Quaternion.Euler(0,f.Body.transform.eulerAngles.y+180,0);
            ps.Simulate(.12f,true,true,true);
            if (vendorBell)
            {
                var particle=new ParticleSystem.Particle[1];
                Require(ps.GetParticles(particle)==1,"vendor main carrier did not spawn");
                vendorParticleRotation=Quaternion.Euler(particle[0].rotation3D);
            }
            var baked=new Mesh();
            try
            {
                mainRenderer.BakeMesh(baked,f.Camera,true); baked.RecalculateBounds();
                var extent=baked.bounds.size;
                float span=Mathf.Max(extent.x,Mathf.Max(extent.y,extent.z));
                Require(span>.001f,"reference mesh particle did not spawn");
                root.transform.localScale=Vector3.one*(f.Height*(vendorBell ? 4.4f : 3.2f)/span);
                Debug.Log("MAYKER FIRE FIT: baked span="+span+" scale="+root.transform.localScale.x);
            }
            finally { Object.DestroyImmediate(baked); }
            if (orbit)
            {
                study=new MaykerFireOrbitStudy(root,f.Actor.position,f.Height,withAccents);
                if (!bell) study.VerifyOrbit();
                for (int i=0;i<lights.Length;i++) lightPower[i]=lights[i].intensity;
                if (bell)
                {
                    bellStudy=new SuzuBellStrikeStudy(root,f,quarterArc,halfArc);
                    bellStudy.VerifyFlow();
                }
                else if (withAccents) accents=new MaykerFireOrbitAccents(f,root.transform.eulerAngles.y);
            }
            if (vendorBell)
            {
                var source=prefab.GetComponent<ParticleSystemRenderer>();
                Require(mainRenderer.mesh==source.mesh && mainRenderer.sharedMaterial==source.sharedMaterial,
                    "direct vendor comparison replaced the original mesh/material");
                foreach (var renderer in root.GetComponentsInChildren<ParticleSystemRenderer>(true))
                    foreach (var material in renderer.sharedMaterials)
                        Require(AssetDatabase.GetAssetPath(material).Contains("/References/MaykerStudio/"),
                            "direct vendor comparison contains an adapted material");
                Debug.Log("BELL VENDOR DIRECT: original FBX, UVs, source materials, particle modules and native age; spawn placement only");
                if (vendorWhite)
                {
                    var main=ps.main; main.startColor=whiteTint;
                    Debug.Log("BELL VENDOR WHITE: only main carrier start color changed; source shaders, opacity, UVs and other layers retained");
                }
            }
            var cameraData=f.Camera.GetUniversalAdditionalCameraData();
            cameraData.requiresColorTexture=true; cameraData.requiresDepthTexture=true;
            RenderPipelineManager.beginCameraRendering+=EnqueueClock;
            foreach (var size in new[] { new Vector2Int(1280,720),new Vector2Int(540,960) })
                Viewport(size);
        }
        finally
        {
            RenderPipelineManager.beginCameraRendering-=EnqueueClock;
            Object.DestroyImmediate(root);
            study?.Dispose();
            accents?.Dispose();
            bellStudy?.Dispose();
        }
        Debug.Log("MAYKER FIRE CAPTURE PASS: "+output);

        void EnqueueClock(ScriptableRenderContext context,Camera camera)
        {
            if (camera==f.Camera)
                foreach (var clock in clocks)
                    camera.GetUniversalAdditionalCameraData().scriptableRenderer.EnqueuePass(clock);
        }
        void Sample(float time,bool visible=true,MaykerFireOrbitAccents.Layers layers=MaykerFireOrbitAccents.Layers.All,
            bool explosion=true)
        {
            f.Sample(time); f.BaseEffect.Hide(); f.Effect.Hide();
            // Retiming all accent clocks together keeps contacts and ember fans on the sweep.
            float age=(time-Hit)*(withAccents && !bell ? AccentPlaybackSpeed : 1);
            float particleAge=bellStudy!=null ? bellStudy.Sample(age,visible,explosion)
                : study!=null ? study.Sample(age) : age;
            foreach (var clock in clocks) clock.Time=Mathf.Max(0,particleAge);
            for (int i=0;i<lights.Length;i++)
                lights[i].intensity=lightPower[i]*(1-Mathf.SmoothStep(0,1,Mathf.InverseLerp(.08f,.5f,particleAge)));
            root.SetActive(visible && age>=0);
            accents?.Sample(age,visible,layers);
            if (!root.activeSelf) return;
            ps.Simulate(particleAge,true,true,true);
        }
        void Viewport(Vector2Int size)
        {
            string name=size.x>size.y ? "landscape" : "portrait";
            f.FrameCamera(size);
            f.Camera.transform.position-=f.Camera.transform.forward*f.Height*1.8f;
            if (longArcView) f.Camera.transform.position-=f.Camera.transform.right*f.Height*.8f;
            bellStudy?.PlaceAtSpawn();
            if (vendorBell)
            {
                var camera=f.Camera.transform;
                var feet=f.TargetHead(0)-camera.up*f.Height*.85f;
                var anchor=feet+camera.up*f.Height*.015f
                    -(feet-camera.position).normalized*f.Height*SuzuBellMeshPreview.CameraInset;
                root.transform.position=anchor-camera.right*f.Height*1.525f-camera.up*f.Height*.2f;
                // Expose the imported XY arc without changing the prefab's particle rotation.
                root.transform.rotation=camera.rotation*Quaternion.Euler(0,180,0)*Quaternion.Inverse(vendorParticleRotation);
            }
            var target=new RenderTexture(size.x,size.y,24,RenderTextureFormat.ARGBHalf) { antiAliasing=4 };
            var pixels=new Texture2D(size.x,size.y,TextureFormat.RGB24,false);
            f.Camera.targetTexture=target;
            try
            {
                Sample(0); f.Camera.Render(); f.Camera.Render();
                Sample(Hit+.12f,false); Read("empty-"+name); var empty=pixels.GetPixels32();
                Sample(Hit+.12f); Read("peak-"+name); var peak=pixels.GetPixels32();
                long energy=Difference(empty,peak);
                Require(energy>10000,"vendor effect is invisible");
                Sample(Hit+.12f,false); Read("hidden-"+name);
                Reject(()=>Require(Difference(empty,pixels.GetPixels32())>10000,"hidden vendor effect"));
                Sample(Hit+.12f); Read("restart-"+name);
                var restart=pixels.GetPixels32();
                Require(Stable(peak,restart),"vendor restart / shader clock is unstable");
                Sample(Hit+.35f,false); Read("late-empty-"+name); var lateEmpty=pixels.GetPixels32();
                Sample(Hit+.35f); Read("late-"+name);
                Require(Difference(lateEmpty,pixels.GetPixels32())>1000,"vendor tail missing");
                Require(Difference(peak,pixels.GetPixels32())>10000,"vendor effect is frozen");
                Reject(()=>Require(Stable(peak,pixels.GetPixels32()),"changed age accepted as a stable restart"));
                Reject(()=>Require(Difference(peak,peak)>10000,"frozen vendor effect"));
                Sample(Hit+2,false); Read("end-empty-"+name); var endEmpty=pixels.GetPixels32();
                Sample(Hit+2); Read("end-"+name);
                Require(Difference(endEmpty,pixels.GetPixels32())<1000,"vendor effect remains after expiry");
                if (vendorWhite)
                {
                    var main=ps.main;
                    main.startColor=originalTint; Sample(Hit+.12f); Read("white-off-"+name);
                    var off=pixels.GetPixels32();
                    Sample(Hit+.12f); Read("white-disabled-"+name);
                    Reject(()=>Require(Difference(off,pixels.GetPixels32())>10000,"disabled white tint"));
                    main.startColor=whiteTint; Sample(Hit+.12f); Read("white-on-"+name);
                    var on=pixels.GetPixels32();
                    Require(Difference(off,on)>10000,"vendor white core is invisible");
                    int whiteBefore=CountWhite(off),whiteAfter=CountWhite(on);
                    Require(whiteAfter>whiteBefore+100,"vendor core did not become visibly white");
                    Reject(()=>Require(CountWhite(off)>whiteBefore+100,"unchanged color accepted as white core"));
                    Debug.Log("BELL VENDOR WHITE VERIFIED: "+name+" white pixels="+whiteBefore+" -> "+whiteAfter);
                    int CountWhite(Color32[] values)
                    {
                        int count=0;
                        for (int i=0;i<values.Length;i++)
                            if (values[i].r>225 && values[i].g>225 && values[i].b>220
                                && values[i].r-values[i].b<24) count++;
                        return count;
                    }
                }
                if (accents!=null)
                {
                    Sample(Hit+.35f); study.SetHighlightVisible(false); Read("white-off-"+name);
                    var noWhite=pixels.GetPixels32();
                    Read("white-hidden-"+name);
                    Reject(()=>Require(Difference(noWhite,pixels.GetPixels32())>10000,"hidden white highlight"));
                    study.SetHighlightVisible(true); Read("white-on-"+name);
                    long whiteEnergy=Difference(noWhite,pixels.GetPixels32());
                    Require(whiteEnergy>10000,"white band is invisible");
                    Debug.Log("MAYKER WHITE VERIFIED: "+name+" energy="+whiteEnergy);
                    foreach (var layer in new[] { MaykerFireOrbitAccents.Layers.Light,
                        MaykerFireOrbitAccents.Layers.Hits,MaykerFireOrbitAccents.Layers.Embers,
                        MaykerFireOrbitAccents.Layers.Aura })
                    {
                        float at=layer==MaykerFireOrbitAccents.Layers.Embers ? .34f : .23f;
                        Sample(Hit+at,true,MaykerFireOrbitAccents.Layers.None); Read("accent-off-"+layer+"-"+name);
                        var off=pixels.GetPixels32();
                        Sample(Hit+at,true,layer); Read("accent-on-"+layer+"-"+name);
                        long added=Difference(off,pixels.GetPixels32());
                        Require(added>10000,"accent layer invisible: "+layer);
                        Sample(Hit+at,true,MaykerFireOrbitAccents.Layers.None); Read("accent-hidden-"+layer+"-"+name);
                        Reject(()=>Require(Difference(off,pixels.GetPixels32())>10000,"hidden accent: "+layer));
                        Debug.Log("MAYKER ACCENT VERIFIED: "+name+" "+layer+" energy="+added);
                    }
                }
                if (bellStudy!=null)
                {
                    Sample(Hit+.12f); root.SetActive(false); Read("blade-off-"+name);
                    var noBlade=pixels.GetPixels32();
                    Read("blade-hidden-"+name);
                    Reject(()=>Require(Difference(noBlade,pixels.GetPixels32())>10000,"hidden Bell blade"));
                    Sample(Hit+.12f); Read("blade-on-"+name);
                    Require(Difference(noBlade,pixels.GetPixels32())>10000,"Bell blade invisible without its supports");
                    study.SetHighlightVisible(false); Read("white-off-"+name);
                    var noWhite=pixels.GetPixels32();
                    study.SetHighlightVisible(true); Read("white-on-"+name);
                    Require(Difference(noWhite,pixels.GetPixels32())>10000,"Bell white highlight invisible");
                    Sample(Hit+.25f,explosion:false); Read("explosion-off-"+name);
                    var noExplosion=pixels.GetPixels32();
                    Sample(Hit+.25f); Read("explosion-on-"+name);
                    long blast=Difference(noExplosion,pixels.GetPixels32());
                    Require(blast>10000,"Bell explosion invisible");
                    Sample(Hit+.25f,explosion:false); Read("explosion-hidden-"+name);
                    Reject(()=>Require(Difference(noExplosion,pixels.GetPixels32())>10000,"hidden Bell explosion"));
                    Debug.Log("BELL EXPLOSION VERIFIED: "+name+" energy="+blast);
                    CheckUvFlow();
                }
                for (int i=0;i<Frames;i++)
                {
                    Sample(i/(float)Fps); Read("motion-"+name+"-"+i.ToString("D3"));
                }
                Debug.Log("MAYKER FIRE VERIFIED: "+name+" effect energy="+energy
                    +"; visibility, restart, travel, tail, expiry passed; hidden/frozen sabotages caught");
            }
            finally { f.Camera.targetTexture=null; Object.DestroyImmediate(target); Object.DestroyImmediate(pixels); }
            void Read(string file)
            {
                var active=RenderTexture.active;
                try
                {
                    // Settle URP's newly enabled light before reading this sampled frame.
                    f.Camera.Render(); f.Camera.Render(); RenderTexture.active=target;
                    pixels.ReadPixels(new Rect(0,0,size.x,size.y),0,0); pixels.Apply();
                    File.WriteAllBytes(Path.Combine(output,file+".png"),pixels.EncodeToPNG());
                }
                finally { RenderTexture.active=active; }
            }
            void CheckUvFlow()
            {
                var enabled=new bool[lights.Length];
                for (int i=0;i<lights.Length;i++) { enabled[i]=lights[i].enabled; lights[i].enabled=false; }
                try
                {
                    var early=Measure(.04f,"early");
                    var full=Measure(.2f,"full");
                    var late=Measure(.55f,"shrinking");
                    var end=Measure(.7f,"expired");
                    Require(early.count>10 && full.count>early.count*2,"UV trail did not visibly grow");
                    Require(full.span>early.span*2,"UV trail retained a constant rendered length");
                    Require(early.centerU>full.centerU+.2f,"UV head did not visibly travel toward the impact end");
                    Require(late.span<full.span*.65f && late.count>0,"UV trail did not visibly shrink");
                    Require(end.count==0,"UV trail remains after shader expiry");
                    Reject(()=>Require(early.centerU>early.centerU+.2f,"frozen UV image"));
                    Debug.Log("BELL RENDERED UV FLOW: "+name+" UV spans="+early.span+", "+full.span+", "+late.span
                        +"; fixed mesh, head travel, growth/shrink and expiry passed");
                }
                finally { for (int i=0;i<lights.Length;i++) lights[i].enabled=enabled[i]; }
                (int count,float span,float centerU) Measure(float age,string stage)
                {
                    Sample(Hit+age); root.SetActive(false); Read("flow-off-"+stage+"-"+name);
                    var off=pixels.GetPixels32();
                    Sample(Hit+age); Read("flow-on-"+stage+"-"+name);
                    var on=pixels.GetPixels32();
                    int count=0; float sumU=0;
                    float minU=float.PositiveInfinity,maxU=float.NegativeInfinity;
                    for (int i=0;i<on.Length;i++)
                    {
                        int delta=Math.Abs(on[i].r-off[i].r)+Math.Abs(on[i].g-off[i].g)+Math.Abs(on[i].b-off[i].b);
                        if (delta<45) continue;
                        int y=i/size.x; count++;
                        // Recover along-carrier distance from rendered pixels: screen height
                        // also measures strip WIDTH when the quarter arc turns sideways.
                        float u=bellStudy.CarrierCoordinate(new Vector2(i%size.x+.5f,y+.5f));
                        sumU+=u;
                        minU=Mathf.Min(minU,u); maxU=Mathf.Max(maxU,u);
                    }
                    return (count,count>0 ? maxU-minU : 0,count>0 ? sumU/count : 0);
                }
            }
        }
    }
    static long Difference(Color32[] a,Color32[] b)
    {
        long value=0;
        for (int i=0;i<a.Length;i++) value+=Math.Abs(a[i].r-b[i].r)+Math.Abs(a[i].g-b[i].g)+Math.Abs(a[i].b-b[i].b);
        return value;
    }
    static bool Stable(Color32[] a,Color32[] b)
    {
        // HDR readback measured 1-3 levels of variation on identical frames. Allow a
        // small local margin but less than 0.1 level/channel mean error across the frame.
        for (int i=0;i<a.Length;i++)
            if (Math.Abs(a[i].r-b[i].r)>8 || Math.Abs(a[i].g-b[i].g)>8 || Math.Abs(a[i].b-b[i].b)>8)
                return false;
        return Difference(a,b)<a.Length*.3;
    }
    static void Reject(Action action)
    {
        bool caught=false; try { action(); } catch (InvalidOperationException) { caught=true; }
        Require(caught,"vendor check survived sabotage");
    }

    // Pin shader time after URP's global resets, including the battlefield foliage.
    internal sealed class CaptureClock : ScriptableRenderPass
    {
        public float Time;
        sealed class Data { public float Time; }
        public CaptureClock(RenderPassEvent stage) { renderPassEvent=stage; }
        public override void RecordRenderGraph(RenderGraph graph,ContextContainer frameData)
        {
            using var builder=graph.AddUnsafePass<Data>("VFX capture clock",out var data);
            data.Time=Time; builder.AllowPassCulling(false); builder.AllowGlobalStateModification(true);
            builder.SetRenderFunc((Data d,UnsafeGraphContext context)=>
                SetTime(CommandBufferHelpers.GetNativeCommandBuffer(context.cmd),d.Time));
        }
#if !UNITY_6000_6_OR_NEWER
        public override void Execute(ScriptableRenderContext context,ref RenderingData renderingData)
        {
            var cmd=CommandBufferPool.Get("VFX capture clock");
            SetTime(cmd,Time); context.ExecuteCommandBuffer(cmd); CommandBufferPool.Release(cmd);
        }
#endif
        static void SetTime(CommandBuffer cmd,float time)
        {
            cmd.SetGlobalVector("_Time",new Vector4(time/20,time,time*2,time*3));
            cmd.SetGlobalVector("_TimeParameters",new Vector4(time,Mathf.Sin(time),Mathf.Cos(time),0));
            cmd.SetGlobalVector("_SinTime",new Vector4(Mathf.Sin(time/8),Mathf.Sin(time/4),Mathf.Sin(time/2),Mathf.Sin(time)));
            cmd.SetGlobalVector("_CosTime",new Vector4(Mathf.Cos(time/8),Mathf.Cos(time/4),Mathf.Cos(time/2),Mathf.Cos(time)));
        }
    }
}
