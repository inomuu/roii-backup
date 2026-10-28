using System;
using System.Collections.Generic;
using System.IO;
using Roi;
using Roi.Arena;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using UnityEngine.Rendering.Universal;
using Object=UnityEngine.Object;

public static class SeraphineCircularSlashPreview
{
    const string Output="/tmp/roi-seraphine-circular-slash";
    [MenuItem("ROI/Art/Capture Seraphine Circular Slash")]
    public static void Capture() => Run(false);

    [MenuItem("ROI/Art/Compare Seraphine Mesh and Flipbook")]
    public static void CaptureComparison() => Run(true);

    [MenuItem("ROI/Art/Compare Seraphine Mesh and UV Motion")]
    public static void CaptureUvComparison() => Run(false,true);

    [MenuItem("ROI/Art/Capture Seraphine Sword Impact")]
    public static void CaptureImpact() => Run(false,false,true);

    [MenuItem("ROI/Art/Capture Seraphine Impact With Red Flash")]
    public static void CaptureRedImpact() => Run(false,false,true,true);

    static void Run(bool compare,bool compareUv=false,bool showImpact=false,bool redFlash=false)
    {
        if (EditorApplication.isPlaying) throw new InvalidOperationException("Capture outside play mode.");
        string output=redFlash ? "/tmp/roi-seraphine-impact-red" : showImpact ? "/tmp/roi-seraphine-impact" : compareUv ? "/tmp/roi-seraphine-uv-study" : compare ? "/tmp/roi-seraphine-flipbook-study" : Output;
        Directory.CreateDirectory(output);
        var previous=SceneManager.GetActiveScene();
        var scene=EditorSceneManager.OpenScene(ProbeScene.Path,OpenSceneMode.Additive);
        SceneManager.SetActiveScene(scene);
        var owned=new List<Object>();
        float exposure=Shader.GetGlobalFloat("_ActorExposure");
        SeraphineImpactPreview impact=null;
        try
        {
            Camera camera=null;
            foreach (var root in scene.GetRootGameObjects())
            {
                foreach (var b in root.GetComponentsInChildren<MonoBehaviour>(true)) b.enabled=false;
                foreach (var c in root.GetComponentsInChildren<Camera>(true))
                { if (c.name=="ArenaCamera") camera=c; else c.enabled=false; }
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
            Require(camera!=null,"authored camera missing");
            camera.enabled=false; camera.scene=scene;
            var tables=ArenaView.Tables.Load(); var config=tables.Class("seraphine");
            Require(config!=null,"Seraphine sheet metadata missing");
            Shader.SetGlobalFloat("_ActorExposure",tables.globals.actor_exposure);
            var texture=new Texture2D(2,2,TextureFormat.RGBA32,false); owned.Add(texture);
            texture.LoadImage(File.ReadAllBytes(Path.Combine(Application.streamingAssetsPath,"art",config.sheet)));
            texture.filterMode=FilterMode.Point;
            var sprites=new Sprite[7];
            for (int i=0;i<7;i++) sprites[i]=SpriteAt(config.atk_row,i);
            var idle=SpriteAt(config.idle_row,0);
            var material=new Material(Shader.Find("ROI/ActorSprite")); owned.Add(material);
            var actor=new GameObject("Circular slash preview actor");
            SceneManager.MoveGameObjectToScene(actor,scene);
            var ray=camera.ViewportPointToRay(new Vector3(.5f,.4f,0));
            new Plane(Vector3.up,Vector3.zero).Raycast(ray,out float distance);
            actor.transform.position=ray.GetPoint(distance);
            var body=new GameObject("Body").AddComponent<SpriteRenderer>();
            body.transform.SetParent(actor.transform,false); body.sharedMaterial=material;
            body.transform.rotation=camera.transform.rotation; body.sprite=idle;
            float sheetHeight=config.fh*tables.cells_per_art_px*tables.sprite_scale*tables.actor_scale*config.Scale;
            int bottom=config.fh,top=0;
            var idlePixels=texture.GetPixels(0,texture.height-(config.idle_row+1)*config.fh,config.fw,config.fh);
            for (int y=0;y<config.fh;y++) for (int x=0;x<config.fw;x++)
                if (idlePixels[y*config.fw+x].a>.05f) { bottom=Mathf.Min(bottom,y); top=Mathf.Max(top,y+1); }
            float height=sheetHeight*(top-bottom)/config.fh;
            body.transform.localScale=Vector3.one*sheetHeight;
            body.transform.localPosition=Vector3.up*(sheetHeight*(.5f-bottom/(float)config.fh)+config.lift);
            ActorContactShadow.Create(actor.transform).Bind(height,config.lift,1);
            var slash=CircularSwordSlash.Create(body,actor.transform,height);
            Require(slash!=null,"slash material missing");
            var rendererSlash=slash.GetComponent<MeshRenderer>();
            var detailRenderer=Array.Find(slash.GetComponentsInChildren<Renderer>(true),r=>r.name=="Textured blade streaks");
            Require(detailRenderer!=null,"textured streak renderer missing");
            var flipbook=compare ? CircularSwordFlipbook.Create(body,actor.transform,height) : null;
            Require(!compare || flipbook!=null,"flipbook shader or atlas missing");
            if (showImpact) impact=new SeraphineImpactPreview(actor.transform,body,camera,height,material,tables,redFlash);
            var originalPosition=actor.transform.position;
            foreach (var size in new[] { new Vector2Int(1280,720),new Vector2Int(540,960) })
            {
                string name=size.x==1280 ? "landscape" : "portrait";
                camera.aspect=size.x/(float)size.y;
                camera.orthographic=false;
                float framing=height*2.35f/(Mathf.Tan(camera.fieldOfView*.5f*Mathf.Deg2Rad)*Mathf.Min(1,camera.aspect));
                camera.transform.position=actor.transform.position+Vector3.up*height*.45f-camera.transform.forward*framing;
                var target=new RenderTexture(size.x,size.y,24,RenderTextureFormat.ARGBHalf) { antiAliasing=4 };
                var pixels=new Texture2D(size.x,size.y,TextureFormat.RGB24,false);
                camera.targetTexture=target;
                try
                {
                    body.sprite=idle; slash.Show(0,0,false); camera.Render(); camera.Render();
                    body.sprite=sprites[3]; slash.Show(3,.35f,true);
                    Require(rendererSlash.enabled,"slash missing during strike");
                    Read("edge-"+name); var withSlash=pixels.GetPixels32();
                    detailRenderer.enabled=false; Read("untextured-"+name); detailRenderer.enabled=true;
                    Require(Difference(withSlash,pixels.GetPixels32())>50,"texture detail invisible in full effect");
                    slash.Show(3,.35f,false); Read("original-"+name);
                    int changed=Difference(withSlash,pixels.GetPixels32());
                    Require(changed>300,"circular mesh made no visible contribution");
                    if (flipbook!=null)
                    {
                        var original=pixels.GetPixels32();
                        flipbook.Show(3,.35f,true); Read("flipbook-"+name);
                        Require(Difference(original,pixels.GetPixels32())>300,"flipbook invisible in arena");
                        flipbook.Show(3,.35f,false);
                    }
                    slash.Show(3,.35f,true);
                    var mesh=slash.GetComponent<MeshFilter>().sharedMesh;
                    var left=(Vector3[])mesh.vertices.Clone();
                    foreach (var vertex in left)
                        Require(!float.IsNaN(vertex.sqrMagnitude) && !float.IsInfinity(vertex.sqrMagnitude),"non-finite mesh vertex");
                    var front=slash.transform.TransformPoint(left[left.Length/2]);
                    body.flipX=true; slash.Show(3,.35f,true);
                    Require(Vector3.Distance(front,slash.transform.TransformPoint(left[left.Length/2]))>.01f,"facing flip did not mirror sweep");
                    Read("flipped-"+name); body.flipX=false; slash.Show(3,.35f,true);
                    var data=camera.GetUniversalAdditionalCameraData(); bool post=data.renderPostProcessing;
                    try { data.renderPostProcessing=false; Read("no-post-"+name); }
                    finally { data.renderPostProcessing=post; }
                    int used=0;
                    foreach (var root in scene.GetRootGameObjects())
                        foreach (var t in root.GetComponentsInChildren<Transform>(true)) used|=1<<t.gameObject.layer;
                    int isolatedLayer=31;
                    while (isolatedLayer>=8 && (used & (1<<isolatedLayer))!=0) isolatedLayer--;
                    Require(isolatedLayer>=8,"no free preview layer");
                    var effectRenderers=Array.FindAll(slash.GetComponentsInChildren<Renderer>(true),
                        r=>r.sharedMaterial.shader.name!="ROI/SwordSlashUv");
                    var previousLayers=new int[effectRenderers.Length];
                    for (int i=0;i<effectRenderers.Length;i++) previousLayers[i]=effectRenderers[i].gameObject.layer;
                    int mask=camera.cullingMask;
                    var clear=camera.clearFlags; var background=camera.backgroundColor;
                    try
                    {
                        foreach (var r in effectRenderers) r.gameObject.layer=isolatedLayer;
                        camera.cullingMask=1<<isolatedLayer;
                        camera.clearFlags=CameraClearFlags.SolidColor; camera.backgroundColor=new Color(.025f,.03f,.04f);
                        data.renderPostProcessing=false; Read("curve-only-"+name);
                        foreach (var r in effectRenderers) r.enabled=false;
                        Read("empty-"+name); var empty=pixels.GetPixels32();
                        foreach (var r in effectRenderers)
                        {
                            r.enabled=true; Read("layer-"+r.name.Replace(' ','-')+"-"+name);
                            int contribution=Difference(empty,pixels.GetPixels32());
                            int minimum=r is ParticleSystemRenderer ? 4 : 50;
                            Require(contribution>minimum,"layer made no visible contribution: "+r.name);
                            if (r==detailRenderer) CheckDetails(r,Read,pixels,empty,name);
                            if (r.name=="Sword foot glow")
                            {
                                int white=BrightWhite(pixels.GetPixels32());
                                Require(white>50,"ground glow has no bright white center");
                                var m=r.sharedMaterial; var core=m.GetColor("_CoreColor");
                                try
                                {
                                    m.SetColor("_CoreColor",Color.clear); Read("ground-core-disabled-"+name);
                                    bool coreCaught=false;
                                    try { Require(BrightWhite(pixels.GetPixels32())>50,"ground core sabotage"); }
                                    catch (InvalidOperationException) { coreCaught=true; }
                                    Require(coreCaught,"white-center guard survived disabled core");
                                }
                                finally { m.SetColor("_CoreColor",core); }
                                Debug.Log("CIRCULAR SLASH GROUND: "+name+" white pixels="+white+"; disabled-core sabotage caught");
                            }
                            r.enabled=false; Read("hidden-layer-"+name);
                            bool caught=false;
                            try { Require(Difference(empty,pixels.GetPixels32())>minimum,"hidden layer sabotage"); }
                            catch (InvalidOperationException) { caught=true; }
                            Require(caught,"pixel guard survived disabled-renderer sabotage: "+r.name);
                            Debug.Log("CIRCULAR SLASH LAYER: "+name+" "+r.name+" pixels="+contribution+"; hidden-renderer sabotage caught");
                        }
                        if (flipbook!=null)
                        {
                            var flipRenderer=flipbook.GetComponent<MeshRenderer>();
                            int oldLayer=flipbook.gameObject.layer;
                            flipbook.gameObject.layer=isolatedLayer;
                            try
                            {
                                Color32[] previousFrame=null;
                                for (int index=0;index<CircularSwordFlipbook.FrameCount;index++)
                                {
                                    float pose=2+(index+.5f)/CircularSwordFlipbook.FrameCount*3;
                                    flipbook.Show(Mathf.FloorToInt(pose),pose-Mathf.Floor(pose),true);
                                    Read("atlas-frame-"+name+"-"+index.ToString("D2"));
                                    var current=pixels.GetPixels32();
                                    int contribution=Difference(empty,current);
                                    Require(contribution>1,"empty flipbook cell: "+index);
                                    if (previousFrame!=null)
                                        Require(Difference(previousFrame,current)>1,"duplicate flipbook frame: "+index);
                                    previousFrame=current;
                                    Debug.Log("FLIPBOOK CELL: "+name+" frame="+index+" pixels="+contribution);
                                }
                                flipbook.Show(3,.35f,true); Read("flipbook-repeat-a-"+name);
                                var repeated=pixels.GetPixels32();
                                flipbook.Show(3,.35f,true); Read("flipbook-repeat-b-"+name);
                                bool repeatCaught=false;
                                try { Require(Difference(repeated,pixels.GetPixels32())>1,"frozen-frame sabotage"); }
                                catch (InvalidOperationException) { repeatCaught=true; }
                                Require(repeatCaught,"duplicate frame guard survived a frozen clock");
                                flipRenderer.enabled=false;
                                Read("flipbook-sabotage-"+name);
                                bool caught=false;
                                try { Require(Difference(empty,pixels.GetPixels32())>1,"hidden flipbook sabotage"); }
                                catch (InvalidOperationException) { caught=true; }
                                Require(caught,"flipbook pixel guard survived hidden renderer");
                                flipbook.Show(3,.35f,true);
                                flipbook.Show(6,0,true);
                                Require(!flipRenderer.enabled,"flipbook outlived strike");
                                Require(!ShaderUtil.ShaderHasError(flipRenderer.sharedMaterial.shader),"flipbook shader error");
                                Debug.Log("FLIPBOOK: "+name+" 16 distinct visible cells; hidden-renderer and frozen-clock sabotage caught; recovery passed");
                            }
                            finally { flipbook.gameObject.layer=oldLayer; flipbook.Show(0,0,false); }
                        }
                        slash.Show(3,.35f,true);
                    }
                    finally
                    {
                        for (int i=0;i<effectRenderers.Length;i++) effectRenderers[i].gameObject.layer=previousLayers[i];
                        camera.cullingMask=mask;
                        camera.clearFlags=clear; camera.backgroundColor=background; data.renderPostProcessing=post;
                    }
                    slash.Show(7,0,true); Require(!rendererSlash.enabled,"slash outlived recovery tail");
                    foreach (var r in effectRenderers)
                    {
                        Require(!r.enabled,"accent outlived recovery tail: "+r.name);
                        Require(!ShaderUtil.ShaderHasError(r.sharedMaterial.shader),"sword shader error: "+r.name);
                    }
                    Require(slash.GetComponentInChildren<ParticleSystem>().particleCount==0,"sparks survived recovery");
                    if (compareUv) CheckUv(slash,body,camera,pixels,Read,name,isolatedLayer);
                    CheckTail(slash,camera,pixels,Read,name,isolatedLayer,false);
                    if (compareUv) CheckTail(slash,camera,pixels,Read,name,isolatedLayer,true);
                    if (impact!=null) impact.Check(pixels,Read,name,isolatedLayer);
                    for (int frame=0;frame<84;frame++)
                    {
                        float time=frame/60f;
                        float pose=time<.15f ? -1 : time<.35f ? (time-.15f)*10
                            : time<.75f ? 2+(time-.35f)/.4f*3
                            : time<.95f ? 5+(time-.75f)*10 : -1;
                        int f=pose<0 ? -1 : Mathf.Clamp(Mathf.FloorToInt(pose),0,6);
                        body.sprite=f<0 ? idle : sprites[f];
                        slash.Show(f,pose-f,f>=0);
                        impact?.Sample(time);
                        Require(actor.transform.position.Equals(originalPosition),"slash moved actor");
                        Read("motion-"+name+"-"+frame.ToString("D3"));
                        if (frame==35) CheckStars(slash,camera,pixels,Read,name,(pose-2)*.4f/3);
                        if (impact!=null && !redFlash && (frame==35 || frame==38))
                            impact.CheckOutline(pixels,Read,name+"-"+frame);
                        if (impact!=null)
                        {
                            impact.Sample(time,true,false); Read("without-flash-"+name+"-"+frame.ToString("D3"));
                            impact.Sample(time,false); Read("without-impact-"+name+"-"+frame.ToString("D3"));
                            impact.Sample(time);
                            if (frame==35) Read("impact-peak-"+name);
                        }
                        bool detailVisible=detailRenderer.enabled;
                        detailRenderer.enabled=false;
                        Read("untextured-motion-"+name+"-"+frame.ToString("D3"));
                        detailRenderer.enabled=detailVisible;
                        if (frame==29 || frame==35 || frame==41)
                            Read("shape-"+name+"-"+frame);
                        if (compareUv)
                        {
                            slash.SetUvMotion(true); slash.Show(f,pose-f,f>=0);
                            Require(actor.transform.position.Equals(originalPosition),"UV slash moved actor");
                            Read("uv-motion-"+name+"-"+frame.ToString("D3"));
                            slash.SetUvMotion(false);
                        }
                        if (flipbook!=null)
                        {
                            slash.Show(f,pose-f,false); flipbook.Show(f,pose-f,f>=0);
                            Require(actor.transform.position.Equals(originalPosition),"flipbook moved actor");
                            Read("flipbook-motion-"+name+"-"+frame.ToString("D3"));
                            flipbook.Show(0,0,false);
                        }
                    }
                    Debug.Log("CIRCULAR SLASH: "+name+" changed pixels="+changed+"; visibility, finite mesh, flip, position and recovery passed");
                }
                finally
                {
                    camera.targetTexture=null; RenderTexture.active=null;
                    target.Release(); Object.DestroyImmediate(target); Object.DestroyImmediate(pixels);
                }
                void Read(string name)
                {
                    camera.Render(); RenderTexture.active=target;
                    pixels.ReadPixels(new Rect(0,0,size.x,size.y),0,0); pixels.Apply();
                    File.WriteAllBytes(Path.Combine(output,name+".png"),pixels.EncodeToPNG());
                }
            }
            Sprite SpriteAt(int row,int frame)
            {
                var sprite=Sprite.Create(texture,new Rect(frame*config.fw,texture.height-(row+1)*config.fh,config.fw,config.fh),
                    new Vector2(.5f,.5f),config.fh); owned.Add(sprite); return sprite;
            }
        }
        finally
        {
            impact?.Dispose();
            RenderTexture.active=null; Shader.SetGlobalFloat("_ActorExposure",exposure);
            EditorSceneManager.CloseScene(scene,true);
            if (previous.IsValid()) SceneManager.SetActiveScene(previous);
            foreach (var item in owned) if (item!=null) Object.DestroyImmediate(item);
        }
    }
    static void CheckStars(CircularSwordSlash slash,Camera camera,Texture2D pixels,Action<string> read,string name,float age)
    {
        var ps=Array.Find(slash.GetComponentsInChildren<ParticleSystem>(),p=>p.name=="Sword radial stars");
        Require(ps!=null,"radial star system missing");
        var renderer=ps.GetComponent<ParticleSystemRenderer>();
        var main=ps.main; var speed=main.startSpeed;
        var mask=new bool[pixels.width*pixels.height];
        var particles=new ParticleSystem.Particle[18]; int count=ps.GetParticles(particles);
        for (int i=0;i<count;i++)
        {
            var world=ps.transform.TransformPoint(particles[i].position);
            var centre=camera.WorldToScreenPoint(world);
            float size=particles[i].GetCurrentSize(ps)*Mathf.Abs(ps.transform.lossyScale.x);
            var edge=camera.WorldToScreenPoint(world+camera.transform.right*size);
            int radius=Mathf.CeilToInt(Mathf.Abs(edge.x-centre.x))+4;
            for (int y=Mathf.Max(0,(int)centre.y-radius);y<=Mathf.Min(pixels.height-1,(int)centre.y+radius);y++)
                for (int x=Mathf.Max(0,(int)centre.x-radius);x<=Mathf.Min(pixels.width-1,(int)centre.x+radius);x++)
                    mask[y*pixels.width+x]=true;
        }
        try
        {
            read("with-stars-"+name); var withStars=pixels.GetPixels32();
            renderer.enabled=false; read("without-stars-"+name);
            int changed=Difference(withStars,pixels.GetPixels32(),mask);
            Require(changed>40,"stars invisible in full effect");
            var without=pixels.GetPixels32();
            int white=BrightStarContribution(without,withStars,mask);
            Require(white>30,"stars have no readable bright white cores");
            read("hidden-stars-"+name);
            bool caught=false;
            try { Require(Difference(without,pixels.GetPixels32(),mask)>40,"hidden stars"); }
            catch (InvalidOperationException) { caught=true; }
            Require(caught,"star visibility guard survived hidden renderer");
            caught=false;
            try { Require(BrightStarContribution(without,pixels.GetPixels32(),mask)>30,"hidden white cores"); }
            catch (InvalidOperationException) { caught=true; }
            Require(caught,"white star guard survived hidden renderer");
            float outward=MeanOutward(ps);
            Require(outward>.5f,"stars did not travel radially outward");
            main.startSpeed=new ParticleSystem.MinMaxCurve(-2,-1.1f);
            ps.Simulate(age,false,true,true);
            caught=false;
            try { Require(MeanOutward(ps)>.5f,"inward stars"); }
            catch (InvalidOperationException) { caught=true; }
            Require(caught,"radial guard survived reversed particle speed");
            Debug.Log("SLASH STARS: "+name+" pixels="+changed+" white cores="+white+" outward speed="+outward
                +"; hidden/reversed-speed sabotage caught");
        }
        finally { main.startSpeed=speed; ps.Simulate(age,false,true,true); renderer.enabled=true; }
    }
    static int BrightStarContribution(Color32[] baseline,Color32[] pixels,bool[] mask)
    {
        int count=0;
        for (int i=0;i<pixels.Length;i++)
        {
            var p=pixels[i]; var b=baseline[i];
            if (mask[i] && p.r>220 && p.g>220 && p.b>220 && p.r+p.g+p.b-b.r-b.g-b.b>60) count++;
        }
        return count;
    }
    static float MeanOutward(ParticleSystem ps)
    {
        var particles=new ParticleSystem.Particle[18]; int count=ps.GetParticles(particles);
        Require(count==18,"radial star burst count changed");
        float sum=0;
        for (int i=0;i<count;i++)
        {
            var position=particles[i].position; position.y=0;
            sum+=Vector3.Dot(position.normalized,particles[i].velocity);
        }
        return sum/count;
    }

    static void CheckUv(CircularSwordSlash slash,SpriteRenderer body,Camera camera,Texture2D pixels,
        Action<string> read,string name,int isolatedLayer)
    {
        Require(slash.SetUvMotion(true),"UV shader missing"); slash.Show(3,.35f,true);
        read("uv-edge-"+name);
        var uvMain=Array.Find(slash.GetComponentsInChildren<MeshRenderer>(true),r=>r.name=="UV main blade");
        Require(uvMain!=null,"UV main carrier missing");
        var referenceVertex=uvMain.GetComponent<MeshFilter>().sharedMesh.vertices[0];
        var referenceWorld=uvMain.transform.TransformPoint(referenceVertex);
        body.flipX=true; slash.Show(3,.35f,true); read("uv-flipped-"+name);
        Require(Vector3.Distance(referenceWorld,uvMain.transform.TransformPoint(referenceVertex))>.01f,"UV facing did not mirror");
        body.flipX=false; slash.Show(3,.35f,true);
        Reject(()=>Require(Vector3.Distance(referenceWorld,uvMain.transform.TransformPoint(referenceVertex))>.01f,"unmirrored UV carrier"),
            "UV facing guard survived an unmirrored carrier");
        var renderers=Array.FindAll(slash.GetComponentsInChildren<MeshRenderer>(true),
            r=>r.sharedMaterial.shader.name=="ROI/SwordSlashUv");
        Require(renderers.Length==5,"UV carrier layer missing");
        var meshes=new Mesh[renderers.Length]; var vertices=new Vector3[renderers.Length][];
        var matrices=new Matrix4x4[renderers.Length]; var layers=new int[renderers.Length];
        for (int i=0;i<renderers.Length;i++)
        {
            meshes[i]=renderers[i].GetComponent<MeshFilter>().sharedMesh;
            vertices[i]=meshes[i].vertices; matrices[i]=renderers[i].localToWorldMatrix;
            layers[i]=renderers[i].gameObject.layer;
        }
        slash.Show(4,.15f,true);
        CheckFixed();
        var displaced=(Vector3[])vertices[0].Clone(); displaced[0]+=Vector3.right*.01f;
        meshes[0].vertices=displaced;
        try { Reject(CheckFixed,"fixed-mesh guard survived a displaced vertex"); }
        finally { meshes[0].vertices=vertices[0]; }
        slash.Show(3,.35f,true);
        var data=camera.GetUniversalAdditionalCameraData(); bool post=data.renderPostProcessing;
        int culling=camera.cullingMask; var clear=camera.clearFlags; var background=camera.backgroundColor;
        try
        {
            foreach (var r in renderers) { r.gameObject.layer=isolatedLayer; r.enabled=false; }
            camera.cullingMask=1<<isolatedLayer; camera.clearFlags=CameraClearFlags.SolidColor;
            camera.backgroundColor=new Color(.025f,.03f,.04f); data.renderPostProcessing=false;
            read("uv-empty-"+name); var empty=pixels.GetPixels32();
            foreach (var r in renderers)
            {
                Require(!ShaderUtil.ShaderHasError(r.sharedMaterial.shader),"UV shader compile error");
                r.enabled=true; read(r.name.Replace(' ','-')+"-"+name);
                int contribution=Difference(empty,pixels.GetPixels32());
                Require(contribution>50,"UV layer invisible: "+r.name);
                r.enabled=false; read("uv-hidden-"+name);
                Reject(()=>Require(Difference(empty,pixels.GetPixels32())>50,"hidden UV layer"),
                    "UV contribution guard survived hidden renderer");
                Debug.Log("UV SLASH LAYER: "+name+" "+r.name+" pixels="+contribution);
            }
            var main=renderers[0]; main.enabled=true;
            var original=new MaterialPropertyBlock(); main.GetPropertyBlock(original);
            var block=new MaterialPropertyBlock(); main.GetPropertyBlock(block);
            try
            {
                block.SetFloat("_Reveal",1); block.SetFloat("_Opacity",1);
                block.SetFloat("_Head",.2f); main.SetPropertyBlock(block);
                read("uv-head-a-"+name); var a=pixels.GetPixels32();
                block.SetFloat("_Head",.45f); main.SetPropertyBlock(block);
                read("uv-head-b-"+name); int movement=Difference(a,pixels.GetPixels32());
                Require(movement>100,"UV offset made no pixel movement");
                block.SetFloat("_Head",.2f); main.SetPropertyBlock(block); read("uv-frozen-"+name);
                Reject(()=>Require(Difference(a,pixels.GetPixels32())>100,"frozen UV offset"),
                    "UV movement guard survived a frozen offset");
                block.SetFloat("_Head",.9999f); main.SetPropertyBlock(block); read("uv-seam-before-"+name);
                var before=pixels.GetPixels32(); int coverage=Difference(empty,before);
                block.SetFloat("_Head",1.0001f); main.SetPropertyBlock(block); read("uv-seam-after-"+name);
                int seam=Difference(before,pixels.GetPixels32());
                Require(seam<coverage*.05f,"UV seam caused a visible jump");
                block.SetFloat("_Head",.75f); main.SetPropertyBlock(block); read("uv-seam-sabotage-"+name);
                Reject(()=>Require(Difference(before,pixels.GetPixels32())<coverage*.05f,"jumped offset"),
                    "UV continuity guard survived a discontinuous offset");
                Debug.Log("UV SLASH: "+name+" movement="+movement+" seam delta="+seam+" / coverage="+coverage
                    +"; frozen-offset, displaced-vertex, hidden-layer and discontinuity sabotage caught");
            }
            finally { main.SetPropertyBlock(original); }
        }
        finally
        {
            for (int i=0;i<renderers.Length;i++) renderers[i].gameObject.layer=layers[i];
            camera.cullingMask=culling; camera.clearFlags=clear; camera.backgroundColor=background;
            data.renderPostProcessing=post;
            slash.Show(7,0,true);
            foreach (var r in slash.GetComponentsInChildren<Renderer>(true)) Require(!r.enabled,"UV recovery left a layer visible");
            Require(slash.GetComponentInChildren<ParticleSystem>().particleCount==0,"UV recovery left particles");
            slash.SetUvMotion(false);
        }
        void CheckFixed()
        {
            for (int i=0;i<meshes.Length;i++)
            {
                Require(renderers[i].localToWorldMatrix==matrices[i],"UV carrier transform moved with phase");
                var current=meshes[i].vertices;
                Require(current.Length==vertices[i].Length,"UV carrier topology changed");
                for (int k=0;k<current.Length;k++) Require(current[k].Equals(vertices[i][k]),"UV carrier vertex moved with phase");
            }
        }
    }
    static void CheckTail(CircularSwordSlash slash,Camera camera,Texture2D pixels,Action<string> read,
        string name,int isolatedLayer,bool uv)
    {
        Require(slash.SetUvMotion(uv),"tail mode unavailable");
        string label=(uv ? "uv" : "mesh")+"-"+name;
        var renderers=slash.GetComponentsInChildren<Renderer>(true);
        var layers=new int[renderers.Length];
        var data=camera.GetUniversalAdditionalCameraData(); bool post=data.renderPostProcessing;
        int culling=camera.cullingMask; var clear=camera.clearFlags; var background=camera.backgroundColor;
        try
        {
            for (int i=0;i<renderers.Length;i++)
            { layers[i]=renderers[i].gameObject.layer; renderers[i].gameObject.layer=isolatedLayer; }
            camera.cullingMask=1<<isolatedLayer; camera.clearFlags=CameraClearFlags.SolidColor;
            camera.backgroundColor=new Color(.025f,.03f,.04f); data.renderPostProcessing=false;
            slash.Show(7,0,true); read("tail-empty-"+label); var empty=pixels.GetPixels32();
            slash.Show(3,.35f,true); read("tail-peak-"+label); long peak=Energy(empty,pixels.GetPixels32());
            var samples=new long[5]; float[] poses={ 5,5.5f,6,6.5f,6.95f };
            for (int i=0;i<poses.Length;i++)
            {
                int f=Mathf.FloorToInt(poses[i]); slash.Show(f,poses[i]-f,true);
                read("tail-"+label+"-"+i);
                samples[i]=Energy(empty,pixels.GetPixels32());
                if (i==0) Require(samples[i]>5000,"tail disappeared at start of recovery");
                else Require(samples[i]<samples[i-1]*.95,"tail did not progressively dim");
            }
            Require(samples[4]<samples[0]*.01,"tail still bright immediately before cutoff");
            slash.Show(5,0,false); read("tail-cutoff-sabotage-"+label);
            Reject(()=>Require(Energy(empty,pixels.GetPixels32())>5000,"early tail cutoff"),
                "tail visibility guard survived immediate cutoff");
            slash.Show(5,0,true); read("tail-frozen-sabotage-"+label);
            Reject(()=>Require(Energy(empty,pixels.GetPixels32())<samples[0]*.95,"frozen fade"),
                "tail decay guard survived a frozen pose");
            slash.Show(6,.95f,false); read("tail-cancelled-"+label);
            Require(Energy(empty,pixels.GetPixels32())==0,"cancelled tail left visible pixels");
            slash.Show(3,.35f,true); read("tail-restarted-"+label);
            Require(Energy(empty,pixels.GetPixels32())>peak*.9,"new strike retained the previous fade");
            slash.Show(6,.5f,true); slash.enabled=false;
            // Edit-mode capture does not dispatch this component's play-mode OnDisable callback.
            slash.Show(6,.5f,true); read("tail-disabled-"+label);
            Require(Energy(empty,pixels.GetPixels32())==0,"sampling a disabled component left a tail");
            slash.enabled=true; slash.Show(7,0,true); read("tail-finished-"+label);
            Require(Energy(empty,pixels.GetPixels32())==0,"completed tail left visible pixels");
            foreach (var r in renderers) Require(!r.enabled,"completed tail left a renderer enabled");
            Require(slash.GetComponentInChildren<ParticleSystem>().particleCount==0,"completed tail left particles");
            Debug.Log("SLASH TAIL: "+label+" energy="+string.Join(",",samples)
                +"; early-cutoff and frozen-fade sabotage caught; cancellation, restart, disabled sampling and completion passed");
        }
        finally
        {
            slash.enabled=true; slash.SetUvMotion(false); slash.Show(7,0,false);
            for (int i=0;i<renderers.Length;i++) renderers[i].gameObject.layer=layers[i];
            camera.cullingMask=culling; camera.clearFlags=clear; camera.backgroundColor=background;
            data.renderPostProcessing=post;
        }
    }
    static long Energy(Color32[] background,Color32[] current)
    {
        long energy=0;
        for (int i=0;i<background.Length;i++)
            energy+=Math.Abs(background[i].r-current[i].r)+Math.Abs(background[i].g-current[i].g)
                +Math.Abs(background[i].b-current[i].b);
        return energy;
    }
    static void Reject(Action assertion,string message)
    {
        bool caught=false;
        try { assertion(); } catch (InvalidOperationException) { caught=true; }
        Require(caught,message);
    }
    static void CheckDetails(Renderer renderer,Action<string> read,Texture2D pixels,Color32[] empty,string name)
    {
        var material=renderer.sharedMaterial;
        var mask=material.GetTexture("_MaskTex");
        var original=new MaterialPropertyBlock(); renderer.GetPropertyBlock(original);
        var block=new MaterialPropertyBlock(); renderer.GetPropertyBlock(block);
        try
        {
            block.SetFloat("_Erosion",0); renderer.SetPropertyBlock(block);
            read("detail-intact-"+name); var intact=pixels.GetPixels32();
            int early=Difference(empty,intact);
            material.SetTexture("_MaskTex",Texture2D.whiteTexture);
            read("detail-unmasked-"+name); var solid=pixels.GetPixels32();
            Require(Difference(solid,intact)>50,"texture does not control the streak silhouette");
            read("detail-mask-sabotage-"+name);
            bool maskCaught=false;
            try { Require(Difference(solid,pixels.GetPixels32())>50,"white-mask sabotage"); }
            catch (InvalidOperationException) { maskCaught=true; }
            Require(maskCaught,"mask guard survived a solid white texture");
            material.SetTexture("_MaskTex",mask);
            block.SetFloat("_Erosion",.78f); renderer.SetPropertyBlock(block);
            read("detail-eroded-"+name);
            int late=Difference(empty,pixels.GetPixels32());
            Require(late>0 && late<early*.65f,"erosion did not remove enough visible streak area");
            block.SetFloat("_Erosion",0); renderer.SetPropertyBlock(block);
            read("detail-erosion-sabotage-"+name);
            int frozen=Difference(empty,pixels.GetPixels32());
            bool erosionCaught=false;
            try { Require(frozen>0 && frozen<early*.65f,"disabled-erosion sabotage"); }
            catch (InvalidOperationException) { erosionCaught=true; }
            Require(erosionCaught,"erosion guard survived a frozen threshold");
            Debug.Log("CIRCULAR SLASH TEXTURE: "+name+" intact="+early+" eroded="+late
                +"; white-mask and frozen-erosion sabotage caught");
        }
        finally { material.SetTexture("_MaskTex",mask); renderer.SetPropertyBlock(original); }
    }
    static int Difference(Color32[] a,Color32[] b,bool[] mask=null)
    {
        int changed=0;
        for (int i=0;i<a.Length;i++)
            if ((mask==null || mask[i]) && Math.Abs(a[i].r-b[i].r)+Math.Abs(a[i].g-b[i].g)+Math.Abs(a[i].b-b[i].b)>36) changed++;
        return changed;
    }
    static int BrightWhite(Color32[] pixels)
    {
        int count=0;
        foreach (var c in pixels)
            if (c.r>200 && c.g>210 && c.b>220) count++;
        return count;
    }
    static void Require(bool condition,string message)
    { if (!condition) throw new InvalidOperationException("CIRCULAR SLASH: "+message); }
}
