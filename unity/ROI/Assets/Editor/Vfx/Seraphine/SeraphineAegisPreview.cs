using System;
using System.Collections.Generic;
using System.IO;
using System.Reflection;
using Roi;
using Roi.Arena;
using Roi.Cutin;
using Roi.Sim;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using UnityEngine.Rendering.Universal;
using Object=UnityEngine.Object;

public static class SeraphineAegisPreview
{
    const string Output="/tmp/roi-seraphine-aegis";
    [MenuItem("ROI/Cut-in/Seraphine Aegis/Play battlefield sequence")]
    public static void Play()
    {
        var arena=Object.FindFirstObjectByType<ArenaView>();
        if (!EditorApplication.isPlaying || arena==null || !arena.DebugAegis())
            Debug.LogWarning("AEGIS FIELD: requires a live arena with Seraphine and no competing focus.");
    }
    [MenuItem("ROI/Cut-in/Seraphine Aegis/Capture battlefield preview")]
    public static void Capture()
    {
        if (EditorApplication.isPlaying) throw new InvalidOperationException("Capture outside play mode.");
        HovlBuffProfiles.Create();
        Directory.CreateDirectory(Output);
        var previous=SceneManager.GetActiveScene();
        var scene=EditorSceneManager.OpenScene(ProbeScene.Path,OpenSceneMode.Additive);
        SceneManager.SetActiveScene(scene);
        var owned=new List<Object>();
        float previousExposure=Shader.GetGlobalFloat("_ActorExposure");
        SeraphineAegisSequence sequence=null;
        try
        {
            Camera camera=null;
            foreach (var root in scene.GetRootGameObjects())
            {
                foreach (var behaviour in root.GetComponentsInChildren<MonoBehaviour>(true)) behaviour.enabled=false;
                foreach (var c in root.GetComponentsInChildren<Camera>(true))
                { if (c.name=="ArenaCamera") camera=c; else c.enabled=false; }
                // Freeze only the capture's foliage copies, so before/after pixels compare poses.
                foreach (var renderer in root.GetComponentsInChildren<Renderer>(true))
                {
                    var materials=renderer.sharedMaterials;
                    for (int i=0;i<materials.Length;i++)
                        if (materials[i]!=null && materials[i].HasProperty("_WindStrength"))
                        {
                            materials[i]=new Material(materials[i]); owned.Add(materials[i]);
                            materials[i].SetFloat("_WindStrength",0);
                        }
                    renderer.sharedMaterials=materials;
                }
            }
            Require(camera!=null,"missing authored camera");
            camera.enabled=false; camera.scene=scene;
            var tables=ArenaView.Tables.Load();
            Shader.SetGlobalFloat("_ActorExposure",tables.globals.actor_exposure);
            var material=new Material(Shader.Find("ROI/ActorSprite")); owned.Add(material);
            var actors=new List<SeraphineAegisSequence.Actor>();
            string[] sheets={ "seraphine","rapis","acolyte_t1","archer_t1","thief_t1" };
            Vector2[] positions={ new(.43f,.44f),new(.68f,.43f),new(.29f,.56f),new(.76f,.29f),new(.24f,.32f) };
            for (int i=0;i<sheets.Length;i++)
            {
                // Legacy art IDs here are stand-in sheets only, never gameplay identities.
                var c=tables.Class(sheets[i]); Require(c!=null,"missing preview sheet "+sheets[i]);
                var texture=new Texture2D(2,2,TextureFormat.RGBA32,false);
                texture.LoadImage(File.ReadAllBytes(Path.Combine(Application.streamingAssetsPath,"art",c.sheet)));
                texture.filterMode=FilterMode.Point; owned.Add(texture);
                var rect=new Rect(0,texture.height-(c.idle_row+1)*c.fh,c.fw,c.fh);
                var sprite=Sprite.Create(texture,rect,new Vector2(.5f,.5f),c.fh); owned.Add(sprite);
                var root=new GameObject("Aegis preview actor "+i); SceneManager.MoveGameObjectToScene(root,scene);
                var ray=camera.ViewportPointToRay(new Vector3(positions[i].x,positions[i].y,0));
                new Plane(Vector3.up,Vector3.zero).Raycast(ray,out float distance); root.transform.position=ray.GetPoint(distance);
                var body=new GameObject("Body").AddComponent<SpriteRenderer>(); body.transform.SetParent(root.transform,false);
                body.sharedMaterial=material; body.sprite=sprite; body.transform.rotation=camera.transform.rotation;
                float h=c.fh*tables.cells_per_art_px*tables.sprite_scale*tables.actor_scale*c.Scale;
                int bottom=c.fh,top=0;
                var pixels=texture.GetPixels((int)rect.x,(int)rect.y,c.fw,c.fh);
                for (int y=0;y<c.fh;y++) for (int x=0;x<c.fw;x++)
                    if (pixels[y*c.fw+x].a>.05f) { bottom=Mathf.Min(bottom,y); top=Mathf.Max(top,y+1); }
                Require(top>bottom,"empty preview sprite");
                body.transform.localScale=Vector3.one*h;
                body.transform.localPosition=Vector3.up*(h*(.5f-bottom/(float)c.fh)+c.lift);
                float visible=h*(top-bottom)/c.fh;
                ActorContactShadow.Create(root.transform).Bind(visible,c.lift,1);
                actors.Add(new SeraphineAegisSequence.Actor { Root=root.transform,Body=body,Height=visible+c.lift+.045f,Recipient=true,Caster=i==0 });
            }
            var driver=new GameObject("Aegis preview driver").AddComponent<SimDriver>();
            SceneManager.MoveGameObjectToScene(driver.gameObject,scene);
            var session=SimSession.NewAs(42,SimSession.LoadCatalog(),"zone_b1_1","seraphine","Aegis probe");
            Require(session!=null,"native test session missing"); driver.Attach(session);
            CheckTrigger(session);
            var original=new Vector3[actors.Count];
            foreach (var size in new[] { new Vector2Int(1280,720),new Vector2Int(1680,720),new Vector2Int(540,960) })
            {
                camera.aspect=size.x/(float)size.y;
                // Place the test encounter once per viewport, before presentation owns anything.
                for (int i=0;i<actors.Count;i++)
                {
                    var ray=camera.ViewportPointToRay(new Vector3(positions[i].x,positions[i].y,0));
                    new Plane(Vector3.up,Vector3.zero).Raycast(ray,out float distance);
                    actors[i].Root.position=ray.GetPoint(distance);
                    original[i]=actors[i].Root.localPosition;
                }
                string name=size.x==1280 ? "landscape" : size.x==1680 ? "ultrawide" : "portrait";
                sequence=SeraphineAegisSequence.Create(camera,actors,driver,true);
                Require(sequence!=null,"sequence failed to create");
                Require(driver.Paused,"sequence did not hold native driver");
                var hash=session.StateHash();
                typeof(SimDriver).GetField("_acc",BindingFlags.Instance|BindingFlags.NonPublic).SetValue(driver,.2f);
                UpdateDriver(driver);
                Require(session.StateHash()==hash,"native sim advanced while held");
                var target=new RenderTexture(size.x,size.y,24,RenderTextureFormat.ARGBHalf) { antiAliasing=4 };
                var pixels=new Texture2D(size.x,size.y,TextureFormat.RGB24,false);
                camera.targetTexture=target;
                if (sequence.CutinCanvas!=null)
                {
                    sequence.CutinCanvas.renderMode=RenderMode.ScreenSpaceCamera;
                    sequence.CutinCanvas.worldCamera=camera; sequence.CutinCanvas.planeDistance=.5f;
                }
                try
                {
                    Canvas.ForceUpdateCanvases();
                    // Let URP populate its first-render lighting and post-processing resources.
                    camera.Render(); camera.Render();
                    Render(0,"before-"+name); var before=pixels.GetPixels32();
                    Render(.25f,"entry-"+name);
                    Render(.7f,"portrait-"+name);
                    Render(1.65f,"reveal-"+name);
                    Render(2.52f,"field-"+name);
                    int shields=0;
                    foreach (var filter in sequence.GetComponentsInChildren<MeshFilter>())
                        if (filter.name=="Aegis shield")
                        {
                            var mesh=filter.sharedMesh;
                            Require(mesh!=null && mesh.bounds.size.z>.1f && mesh.bounds.size.z<.4f && mesh.vertexCount>20,"shield must be a convex plate, not a billboard or enclosure");
                            Require(Vector3.Dot(filter.transform.up,Vector3.up)>.999f,"shield tilts with camera instead of standing on ground");
                            shields++;
                        }
                    Require(shields==actors.Count,"missing recipient shield mesh");
                    Require(!ShaderUtil.ShaderHasError(Resources.Load<Shader>("Cutin/SeraphineAegisField")),"field shader compilation failed");
                    Require(!ShaderUtil.ShaderHasError(Resources.Load<Shader>("Effects/BuffGroundGlow")),"buff ground shader compilation failed");
                    Require(!ShaderUtil.ShaderHasError(Resources.Load<Shader>("Effects/BuffAura")),"buff aura shader compilation failed");
                    Require(Difference(before,pixels.GetPixels32())>size.x*size.y/50,"field effect blank");
                    var auraLayers=new List<MeshRenderer>();
                    Transform ribbon=null;
                    foreach (var renderer in sequence.GetComponentsInChildren<MeshRenderer>())
                        if (renderer.sharedMaterial.shader.name=="ROI/BuffAura")
                        {
                            auraLayers.Add(renderer);
                            if (renderer.name=="Buff aura ribbon") ribbon=renderer.transform;
                        }
                    Require(auraLayers.Count==actors.Count*2 && ribbon!=null,"missing recipient aura layers");
                    int auraPixels=CaptureAuraOnly(camera,auraLayers,pixels,target,"aura-only-"+name);
                    Debug.Log("BUFF AURA: "+name+" changed pixels="+auraPixels);
                    Require(auraPixels>size.x*size.y/10000,"aura layers made no visible contribution");
                    var rotation=ribbon.localRotation;
                    sequence.Sample(2.62f);
                    Require(Quaternion.Angle(rotation,ribbon.localRotation)>5,"aura ribbon did not orbit");
                    sequence.Sample(2.52f);
                    Require(Quaternion.Angle(rotation,ribbon.localRotation)<.01f,"aura backward seek drifted");
                    Render(2.72f,"aura-rise-"+name);
                    Render(2.52f,"field-"+name);
                    if (name=="landscape")
                        for (int frame=0;frame<105;frame++) Render(frame/30f,"motion-"+frame.ToString("D3"));
                    if (name=="portrait")
                        for (int frame=0;frame<44;frame++) Render(1.8f+frame/30f,"aura-portrait-"+frame.ToString("D3"));
                    Render(sequence.Duration,"after-"+name);
                    for (int i=0;i<actors.Count;i++) Require(actors[i].Root.localPosition.Equals(original[i]),"actor moved at sequence end");
                    Require(!driver.Paused,"sequence leaked pause");
                    Require(session.StateHash()==hash,"presentation mutated native sim");
                    Require(Difference(before,pixels.GetPixels32())<size.x*size.y/100,"render did not restore");
                    Debug.Log("AEGIS FIELD: "+name+" in-place cut-in/field, native hash and restoration passed");
                }
                finally
                {
                    camera.targetTexture=null; RenderTexture.active=null;
                    target.Release(); Object.DestroyImmediate(target); Object.DestroyImmediate(pixels);
                    Object.DestroyImmediate(sequence.gameObject); sequence=null;
                }
                void Render(float age,string file)
                {
                    sequence.Sample(age);
                    for (int i=0;i<actors.Count;i++)
                        Require(actors[i].Root.localPosition.Equals(original[i]),"presentation moved actor "+i+" at "+age+"s");
                    Canvas.ForceUpdateCanvases(); camera.Render(); RenderTexture.active=target;
                    pixels.ReadPixels(new Rect(0,0,size.x,size.y),0,0); pixels.Apply();
                    File.WriteAllBytes(Path.Combine(Output,file+".png"),pixels.EncodeToPNG());
                }
            }
            sequence=SeraphineAegisSequence.Create(camera,actors,driver,false);
            sequence.Sample(.7f);
            for (int i=0;i<actors.Count;i++) Require(actors[i].Root.localPosition==original[i],"repeat cast restaged actors");
            driver.Paused=true; sequence.enabled=false;
            Require(driver.Paused,"disable released another pause"); driver.Paused=false;
            Require(!driver.Paused,"disable leaked hold"); Object.DestroyImmediate(sequence.gameObject); sequence=null;
            UpdateDriver(driver); Require(driver.Tick>0,"native driver did not resume");
            Debug.Log("AEGIS FIELD: short repeat, disable/manual pause and native resume passed. "+Output);
            CaptureSharedBuffs(camera,actors);
        }
        finally
        {
            if (sequence!=null) Object.DestroyImmediate(sequence.gameObject);
            RenderTexture.active=null;
            Shader.SetGlobalFloat("_ActorExposure",previousExposure);
            EditorSceneManager.CloseScene(scene,true);
            if (previous.IsValid()) SceneManager.SetActiveScene(previous);
            foreach (var o in owned) if (o!=null) Object.DestroyImmediate(o);
        }
    }
    static void CaptureSharedBuffs(Camera camera,List<SeraphineAegisSequence.Actor> actors)
    {
        string[] names={ "HealingReceive","AegisReceive","ShieldReceive" };
        var effects=new List<BuffEffect>();
        var target=new RenderTexture(1280,720,24,RenderTextureFormat.ARGBHalf);
        var pixels=new Texture2D(1280,720,TextureFormat.RGB24,false);
        var positions=new Vector3[actors.Count];
        var active=new bool[actors.Count];
        for (int i=0;i<actors.Count;i++) { positions[i]=actors[i].Root.position; active[i]=actors[i].Root.gameObject.activeSelf; }
        try
        {
            camera.aspect=1280f/720; camera.targetTexture=target;
            for (int i=0;i<actors.Count;i++)
            {
                actors[i].Root.gameObject.SetActive(i<3);
                if (i>=3) continue;
                var ray=camera.ViewportPointToRay(new Vector3(.28f+i*.22f,.40f,0));
                new Plane(Vector3.up,positions[i]).Raycast(ray,out float distance);
                actors[i].Root.position=ray.GetPoint(distance);
                effects.Add(new BuffEffect(Resources.Load<BuffEffectProfile>("BuffEffects/"+names[i]),
                    camera.transform.parent,actors[i].Root,actors[i].Height));
            }
            Render(-1,"shared-before"); var before=pixels.GetPixels32();
            Render(.5f,"shared-profiles");
            Require(Difference(before,pixels.GetPixels32())>500,"shared buff prefabs invisible");
            foreach (var effect in effects) Require(effect.ParticleCount>0,"shared profile emitted no particles");
            for (int frame=0;frame<45;frame++) Render(frame/30f,"shared-motion-"+frame.ToString("D3"));
            Render(1.2f,"shared-after");
            Require(Difference(before,pixels.GetPixels32())<1280*720/100,"shared buff left visible particles after expiry");
            Debug.Log("SHARED BUFF: healing, recolored buff and shield rendered through one player and expired cleanly.");
        }
        finally
        {
            foreach (var effect in effects) effect.Dispose();
            for (int i=0;i<actors.Count;i++) { actors[i].Root.position=positions[i]; actors[i].Root.gameObject.SetActive(active[i]); }
            camera.targetTexture=null; RenderTexture.active=null; target.Release();
            Object.DestroyImmediate(target); Object.DestroyImmediate(pixels);
        }
        void Render(float age,string name)
        {
            foreach (var effect in effects) effect.Sample(age);
            camera.Render(); RenderTexture.active=target;
            pixels.ReadPixels(new Rect(0,0,1280,720),0,0); pixels.Apply();
            File.WriteAllBytes(Path.Combine(Output,name+".png"),pixels.EncodeToPNG());
        }
    }
    static int CaptureAuraOnly(Camera camera,List<MeshRenderer> layers,Texture2D pixels,RenderTexture target,string name)
    {
        int used=0;
        foreach (var root in camera.scene.GetRootGameObjects())
            foreach (var transform in root.GetComponentsInChildren<Transform>(true)) used|=1<<transform.gameObject.layer;
        int layer=31;
        while (layer>=8 && (used & (1<<layer))!=0) layer--;
        Require(layer>=8,"no free layer for isolated aura capture");
        var data=camera.GetUniversalAdditionalCameraData();
        bool post=data.renderPostProcessing;
        int mask=camera.cullingMask;
        var clear=camera.clearFlags; var background=camera.backgroundColor;
        var original=new int[layers.Count];
        for (int i=0;i<layers.Count;i++) original[i]=layers[i].gameObject.layer;
        try
        {
            data.renderPostProcessing=false;
            camera.clearFlags=CameraClearFlags.SolidColor; camera.backgroundColor=Color.black;
            camera.cullingMask=0; Read(); var blank=pixels.GetPixels32();
            foreach (var renderer in layers) renderer.gameObject.layer=layer;
            camera.cullingMask=1<<layer; Read();
            File.WriteAllBytes(Path.Combine(Output,name+".png"),pixels.EncodeToPNG());
            return Difference(blank,pixels.GetPixels32());
        }
        finally
        {
            for (int i=0;i<layers.Count;i++) layers[i].gameObject.layer=original[i];
            camera.cullingMask=mask; camera.clearFlags=clear; camera.backgroundColor=background;
            data.renderPostProcessing=post;
        }
        void Read()
        {
            camera.Render(); RenderTexture.active=target;
            pixels.ReadPixels(new Rect(0,0,target.width,target.height),0,0); pixels.Apply();
        }
    }
    static int Difference(Color32[] a,Color32[] b)
    {
        int count=0;
        for (int i=0;i<a.Length;i++) if (Math.Abs(a[i].r-b[i].r)+Math.Abs(a[i].g-b[i].g)+Math.Abs(a[i].b-b[i].b)>20) count++;
        return count;
    }
    static void UpdateDriver(SimDriver driver) => typeof(SimDriver)
        .GetMethod("Update",BindingFlags.Instance|BindingFlags.NonPublic).Invoke(driver,null);
    static void CheckTrigger(SimSession session)
    {
        var trigger=new AegisPresentationTrigger();
        string View(int tick)=>"{\"tick\":"+tick+",\"ult_fx\":[{\"caster_id\":1,\"skill_id\":\"aegis\"}],\"grid\":{\"entities\":[{\"id\":1,\"team\":\"Player\",\"alive\":true,\"shields\":[{\"src_id\":1,\"source_skill_id\":\"aegis\",\"pool\":10}]}]}}";
        bool portrait=false;
        Require(trigger.TryPresent(session,View(1),(id,targets,show)=>{ portrait=show; return targets.Contains(id); }) && portrait,"first cast lost portrait");
        Require(!trigger.TryPresent(session,View(1),(id,targets,show)=>true),"duplicate event replayed");
        Require(trigger.TryPresent(session,View(2),(id,targets,show)=>{ portrait=show; return true; }) && !portrait,"repeat field cast missing or portrait repeated");
        Debug.Log("AEGIS FIELD: resolved-event targeting, dedup and portrait frequency passed");
    }
    static void Require(bool condition,string message)
    {
        if (!condition) throw new InvalidOperationException("AEGIS FIELD: "+message);
    }
}
