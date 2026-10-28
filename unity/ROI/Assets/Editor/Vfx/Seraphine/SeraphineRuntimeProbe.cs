using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using Newtonsoft.Json.Linq;
using Roi.Arena;
using Roi.Cutin;
using Roi.Sim;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using Object=UnityEngine.Object;

public static class SeraphineRuntimeProbe
{
    const BindingFlags Flags=BindingFlags.Instance|BindingFlags.Public|BindingFlags.NonPublic;
    const string Output="/tmp/roi-seraphine-runtime";

    [MenuItem("ROI/Art/Verify Seraphine Live Skills")]
    public static void Run()
    {
        Require(!EditorApplication.isPlaying,"Run outside Play mode; never touch a live save.");
        Directory.CreateDirectory(Output);
        var snapshots=new Dictionary<string,(string before,string cast)>();
        using (var sim=SimSession.NewAs(42,SimSession.LoadCatalog(),"zone_b1_1","seraphine","VFX probe"))
        {
            Require(sim!=null,"native sim unavailable");
            string previous=sim.ViewJson();
            for (int i=0;i<500;i++)
            {
                sim.Tick(1); string json=sim.ViewJson(); var view=JObject.Parse(json);
                foreach (var entity in view["grid"]["entities"])
                {
                    if ((string)entity["unit_def"]!="seraphine") continue;
                    foreach (var skill in entity["skills"])
                    {
                        string id=(string)skill["id"];
                        if ((long?)skill["last_fired"]!=(long)view["tick"]) continue;
                        if (id=="iron_chains" && view["recent_hits"].Where(h=>(long)h["attacker_id"]==1)
                            .Select(h=>(long)h["target_id"]).Distinct().Count()!=2) continue;
                        if (!snapshots.ContainsKey(id)) snapshots[id]=(previous,json);
                    }
                }
                previous=json;
            }
        }
        Require(snapshots.ContainsKey("steel_break") && snapshots.ContainsKey("iron_chains"),"real skill snapshots missing");
        CollectAegisTiming(out string aegisWindup,out string aegisLock);
        VerifyTiming(aegisWindup,aegisLock);
        foreach (string skill in new[] { "steel_break","iron_chains" })
        {
            var data=snapshots[skill];
            using (var f=new Fixture())
            {
                f.Feed(data.before); f.Frame(0,.7f);
                f.Feed(data.cast);
                Require(f.Casts.Count==1,"actual resolved cast did not create exactly one presentation");
                object cast=f.Casts[0];
                var targets=(IList)Get(cast,"Targets");
                int expected=JObject.Parse(data.cast)["recent_hits"].Where(h=>(long)h["attacker_id"]==1)
                    .Select(h=>(long)h["target_id"]).Distinct().Count();
                Require(targets.Count==expected,"targets differ from authoritative hit IDs");
                f.Feed(data.cast); Require(f.Casts.Count==1 && ReferenceEquals(cast,f.Casts[0]),"same-tick duplicate restart");
                var root=(GameObject)Get(cast,"Root");
                f.Frame(.06f); f.Frame(0);
                if (skill=="steel_break")
                {
                    var carrier=root.GetComponentsInChildren<MeshFilter>().First(m=>m.name=="UV main blade");
                    var matrix=carrier.transform.localToWorldMatrix; var vertices=carrier.sharedMesh.vertices;
                    var block=new MaterialPropertyBlock(); carrier.GetComponent<Renderer>().GetPropertyBlock(block);
                    float head=block.GetFloat("_Head");
                    f.Frame(.06f); f.Frame(0);
                    carrier.GetComponent<Renderer>().GetPropertyBlock(block);
                    Require(block.GetFloat("_Head")>head+.01f,"UV head did not travel");
                    Require(carrier.transform.localToWorldMatrix==matrix && carrier.sharedMesh.vertices.SequenceEqual(vertices),"carrier moved/deformed");
                    carrier.transform.position+=Vector3.right;
                    Reject(()=>Require(carrier.transform.localToWorldMatrix==matrix,"moving carrier sabotage"));
                    carrier.transform.position-=Vector3.right;
                    var target=Get(targets[0],"Actor"); var body=(SpriteRenderer)Get(target,"Body");
                    body.GetPropertyBlock(block);
                    Require(block.GetColor("_FlashColor").g<.5f,"scoped red flash missing");
                    var actor=Get(cast,"Actor"); ((SpriteRenderer)Get(actor,"Body")).GetPropertyBlock(block);
                    Require(block.GetColor("_FlashColor").g>1,"caster/global flash was recolored");
                }
                else
                {
                    f.Frame(.22f); f.Frame(0);
                    Require(root.GetComponentsInChildren<IronChainsEffect>().Length==2,"two real targets need two chains");
                    Require(root.GetComponentsInChildren<IronChainsEffect>().All(c=>c.LinkCount>0),"empty chain geometry");
                }
                f.Capture(skill,root);
                if (skill=="iron_chains")
                {
                    // Aegis can follow before the old chain's visual tail expires.
                    var actor=Get(cast,"Actor");
                    Set(cast,"Age",.42f); Set(actor,"ResolvedSkill","aegis"); f.Frame(0);
                    Require((int)Get(actor,"PoseRow")==3,"old chain stole the newer Aegis lock pose");
                    Set(actor,"ResolvedSkill","iron_chains"); f.Frame(0);
                    Reject(()=>Require((int)Get(actor,"PoseRow")==3,"stale resolved-skill sabotage"));
                }
                f.Frame(2); f.Frame(0); Require(f.Casts.Count==0,"expired effect leaked");
                f.Feed(data.before); f.Feed(data.cast); Require(f.Casts.Count==1,"rewind/restart failed");
                f.Arena.ResetActors(); Require(f.Casts.Count==0,"reset leaked effects");
                f.Feed(data.cast); Require(f.Casts.Count==0,"initial snapshot replayed a historical cast");
            }
            // Sabotage the actual event inputs, then assert the original positive assertion fails.
            using (var f=new Fixture())
            {
                f.Feed(data.before); f.Frame(0,.7f);
                var broken=JObject.Parse(data.cast);
                foreach (var s in broken["grid"]["entities"][0]["skills"]) s["last_fired"]=0;
                f.Feed(broken.ToString()); Reject(()=>Require(f.Casts.Count==1,"missing event sabotage"));
            }
            using (var f=new Fixture())
            {
                f.Feed(data.before); f.Frame(0,.7f);
                var broken=JObject.Parse(data.cast); broken["recent_hits"]=new JArray();
                f.Feed(broken.ToString());
                Reject(()=>Require(((IList)Get(f.Casts[0],"Targets")).Count>0,"missing targets sabotage"));
                var changed=JObject.Parse(data.cast);
                changed["zone_spawn"]["opened_at_tick"]=(long)changed["tick"];
                changed["tick"]=(long)changed["tick"]+1;
                f.Feed(changed.ToString()); Require(f.Casts.Count==0,"room retry retained old effect");
            }
        }
        Debug.Log("SERAPHINE RUNTIME PASS: real FFI casts, two-target chains, Aegis pending/lock/release, dedup, UV travel/static carrier, scoped flash, expiry/reset/retry, negative controls. "+Output);
    }

    static void CollectAegisTiming(out string windup,out string locked)
    {
        windup=locked=null;
        string catalog=SimSession.LoadCatalog(); JObject save;
        using (var seed=SimSession.NewAs(42,catalog,"zone_b1_1","seraphine","Aegis timing probe"))
        {
            seed.Tick(200); save=JObject.Parse(seed.SaveJson());
        }
        // Own the prerequisite, not the outcome: fill the gauge only. The real sim
        // must choose the pending cast, spend the swing and publish ult_fx + motion_lock.
        save["grid"]["entities"][0]["limit_break"]=1000;
        using (var sim=SimSession.Load(save.ToString(),catalog))
        {
            Require(sim!=null,"isolated Aegis state did not load");
            int fired=0; var trigger=new AegisPresentationTrigger();
            for (int i=0;i<40;i++)
            {
                sim.Tick(1); string json=sim.ViewJson(); var view=JObject.Parse(json);
                var actor=view["grid"]["entities"][0];
                if ((string)actor["pending_skill"]=="aegis" && (long)actor["atk_acc"]>=850) windup=json;
                if ((int?)actor["motion_lock"]>0 && actor["skills"].Any(s=>(string)s["id"]=="aegis"
                    && (long?)s["last_fired"]==(long)view["tick"])) locked=json;
                bool Play(long caster,HashSet<long> targets,bool portrait)
                { Require(caster==1 && targets.Count==1,"Aegis shield recipient mismatch"); fired++; return true; }
                trigger.TryPresent(sim,json,Play);
                Require(!trigger.TryPresent(sim,json,Play),"Aegis same-tick replay");
            }
            Require(fired==1,"isolated charged Aegis did not resolve exactly once: "+fired);
        }
    }

    static void VerifyTiming(string windup,string locked)
    {
        Require(windup!=null && locked!=null,"native sim must expose Aegis pending_skill and motion_lock (restart with 0.0.481+)");
        using (var f=new Fixture())
        {
            f.Feed(windup); f.Frame(0);
            object actor=((IDictionary)Get(f.Arena,"_actors"))[1L];
            void CastPose()
            {
                var sprite=((SpriteRenderer)Get(actor,"Body")).sprite;
                var art=ArenaView.Tables.Load().Class("seraphine");
                int row=(sprite.texture.height-(int)sprite.rect.y)/art.fh-1;
                Require(row==3 && (string)Get(actor,"DbgAnim")=="skill","Aegis must use open-hand cast row, not sword windup");
                Require(!(bool)Get(actor,"BasicSwordSwing") && !(bool)Get(actor,"StrikeLocked"),"basic latch survived skill pose");
            }
            CastPose(); Require(f.Casts.Count==0,"prediction spawned resolved VFX");
            f.CapturePose("aegis-windup",actor);
            var noPending=JObject.Parse(windup); noPending["grid"]["entities"][0]["pending_skill"]=null;
            f.Feed(noPending.ToString()); f.Frame(0); Reject(CastPose);
            Set(actor,"StrikeLocked",true); Set(actor,"FollowUntil",Time.time+10);
            f.Feed(locked); f.Frame(0); CastPose();
            Require((int)Get(actor,"MotionLock")>0,"motion_lock DTO dropped");
            var before=((SpriteRenderer)Get(actor,"Body")).sprite;
            f.Frame(0,.9f); CastPose();
            Require(((SpriteRenderer)Get(actor,"Body")).sprite==before,"held pose advanced into a basic");
            f.CapturePose("aegis-lock",actor);
            // A frozen accumulator near contact must not accidentally open the basic path.
            var stressed=JObject.Parse(locked);
            stressed["grid"]["entities"][0]["atk_acc"]=950;
            stressed["grid"]["entities"][0]["pending_skill"]=null;
            f.Feed(stressed.ToString()); f.Frame(0); CastPose();
            stressed["grid"]["entities"][0]["motion_lock"]=0;
            f.Feed(stressed.ToString()); f.Frame(0); Reject(CastPose);
            Require((string)Get(actor,"DbgAnim")=="attack","basic failed to resume after lock release");
        }
        Debug.Log("Seraphine timing PASS: rendered Aegis row 3 on pending, held row 3 through motion_lock, basic resumes on unlock; missing-key controls rejected.");
    }

    sealed class Fixture : IDisposable
    {
        readonly Scene _scene;
        readonly Camera _camera;
        readonly List<Object> _owned=new();
        readonly float _exposure;
        public readonly ArenaView Arena;
        public IList Casts => (IList)Get(Arena,"_seraphineCasts");

        public Fixture()
        {
            _exposure=Shader.GetGlobalFloat("_ActorExposure");
            _scene=EditorSceneManager.NewPreviewScene();
            var root=new GameObject("Runtime skill probe"); SceneManager.MoveGameObjectToScene(root,_scene);
            Arena=root.AddComponent<ArenaView>(); Arena.enabled=false;
            var field=new GameObject("Field"); field.transform.SetParent(root.transform,false);
            new GameObject("Ground").transform.SetParent(field.transform,false);
            var cg=new GameObject("Camera"); cg.transform.SetParent(root.transform,false);
            _camera=cg.AddComponent<Camera>(); _camera.scene=_scene; _camera.enabled=false;
            _camera.transform.rotation=Quaternion.Euler(28,180,0); _camera.fieldOfView=38;
            _camera.clearFlags=CameraClearFlags.SolidColor; _camera.backgroundColor=new Color(.09f,.13f,.13f);
            var light=new GameObject("Key light").AddComponent<Light>(); light.transform.SetParent(root.transform,false);
            light.type=LightType.Directional; light.intensity=2; light.transform.rotation=Quaternion.Euler(45,140,0);
            var tables=ArenaView.Tables.Load(); Set(Arena,"_tab",tables); Set(Arena,"_cam",_camera);
            Set(Arena,"_fieldRoot",field.transform); Set(Arena,"_region",1);
            Shader.SetGlobalFloat("_ActorExposure",tables.globals.actor_exposure);
        }

        public void Feed(string json)
        {
            var j=JObject.Parse(json);
            Set(Arena,"_zoneSeen",(string)j["current_zone_id"]);
            Set(Arena,"_openedAtSeen",(long?)j["zone_spawn"]?["opened_at_tick"] ?? 0);
            Arena.FeedView(json);
        }

        public void Frame(float dt,float glideTime=0)
        {
            foreach (DictionaryEntry pair in (IDictionary)Get(Arena,"_actors"))
            {
                object actor=pair.Value;
                ((Transform)Get(actor,"Billboard")).rotation=_camera.transform.rotation;
                if ((bool)Get(actor,"IsPlayer")) Call(Arena,"AnimatePlayer",actor,Time.time+glideTime,0f,0f);
                else Call(Arena,"AnimateMonster",actor,Time.time+glideTime,0f);
                Call(Arena,"TickJuice",actor,0f);
            }
            Call(Arena,"TickSeraphineSkills",dt);
        }

        public void Capture(string skill,GameObject effect)
        {
            var actor=Get(Casts[0],"Actor"); float height=(float)Get(Casts[0],"Height");
            var center=((GameObject)Get(actor,"Root")).transform.position+Vector3.up*height*.5f;
            foreach (var size in new[] { new Vector2Int(1280,720),new Vector2Int(540,960) })
            {
                _camera.aspect=size.x/(float)size.y;
                _camera.transform.position=center-_camera.transform.forward*height*(_camera.aspect<1 ? 13 : 5);
                var rt=new RenderTexture(size.x,size.y,24); var pixels=new Texture2D(size.x,size.y,TextureFormat.RGB24,false);
                _camera.targetTexture=rt; var old=RenderTexture.active;
                try
                {
                    Color32[] Read(string suffix)
                    {
                        _camera.Render(); RenderTexture.active=rt;
                        pixels.ReadPixels(new Rect(0,0,size.x,size.y),0,0); pixels.Apply();
                        File.WriteAllBytes(Path.Combine(Output,skill+"-"+size.x+suffix+".png"),pixels.EncodeToPNG());
                        return pixels.GetPixels32();
                    }
                    var with=Read(""); effect.SetActive(false); var without=Read("-without"); effect.SetActive(true);
                    int delta=with.Zip(without,(a,b)=>Math.Abs(a.r-b.r)+Math.Abs(a.g-b.g)+Math.Abs(a.b-b.b)).Count(d=>d>30);
                    Require(delta>150,"skill is blank/tiny in viewport: "+delta);
                    int fake=with.Zip(with,(a,b)=>Math.Abs(a.r-b.r)+Math.Abs(a.g-b.g)+Math.Abs(a.b-b.b)).Count(d=>d>30);
                    Reject(()=>Require(fake>150,"pixel delta sabotage"));
                    Debug.Log("Seraphine "+skill+" "+size+" effect pixels="+delta);
                }
                finally { RenderTexture.active=old; _camera.targetTexture=null; Object.DestroyImmediate(rt); Object.DestroyImmediate(pixels); }
            }
        }

        public void CapturePose(string name,object actor)
        {
            var center=((GameObject)Get(actor,"Root")).transform.position+Vector3.up*(float)Get(actor,"BarHeadY")*.5f;
            _camera.aspect=1; _camera.transform.position=center-_camera.transform.forward*4;
            var rt=new RenderTexture(512,512,24); var pixels=new Texture2D(512,512,TextureFormat.RGB24,false);
            var old=RenderTexture.active; _camera.targetTexture=rt;
            try
            {
                _camera.Render(); RenderTexture.active=rt;
                pixels.ReadPixels(new Rect(0,0,512,512),0,0); pixels.Apply();
                File.WriteAllBytes(Path.Combine(Output,name+".png"),pixels.EncodeToPNG());
            }
            finally { RenderTexture.active=old; _camera.targetTexture=null; Object.DestroyImmediate(rt); Object.DestroyImmediate(pixels); }
        }

        public void Dispose()
        {
            Arena.ResetActors();
            foreach (string name in new[] { "_sprites","_sheets" })
                foreach (DictionaryEntry p in (IDictionary)Get(Arena,name)) if (p.Value is Object obj) _owned.Add(obj);
            EditorSceneManager.ClosePreviewScene(_scene);
            foreach (var item in _owned) if (item!=null) Object.DestroyImmediate(item);
            Shader.SetGlobalFloat("_ActorExposure",_exposure);
        }
    }

    static object Get(object obj,string name) => obj.GetType().GetField(name,Flags).GetValue(obj);
    static void Set(object obj,string name,object value) => obj.GetType().GetField(name,Flags).SetValue(obj,value);
    static object Call(object obj,string name,params object[] args) => obj.GetType().GetMethod(name,Flags).Invoke(obj,args);
    static void Require(bool value,string message) { if (!value) throw new InvalidOperationException(message); }
    static void Reject(Action check)
    {
        bool rejected=false; try { check(); } catch (InvalidOperationException) { rejected=true; }
        Require(rejected,"negative control survived");
    }
}
