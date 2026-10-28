using System;
using System.Collections.Generic;
using System.IO;
using Roi;
using Roi.Arena;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using Object=UnityEngine.Object;
using static Roi.Editor.Vfx.SeraphineChainsChecks;

namespace Roi.Editor.Vfx
{
    // Owns a temporary scene and synthetic motion only. Never saves or advances the simulation.
    internal sealed class SeraphineChainsFixture : IDisposable
    {
        public const float Start=.25f;
        public const int FrameCount=126, FramesPerSecond=60;
        readonly List<Object> _owned=new();
        readonly Scene _previous;
        readonly float _exposure;
        readonly Texture2D _sheet;
        readonly ArenaView.ClassGeom _config;
        readonly Sprite _idle;
        readonly Sprite[] _reach=new Sprite[3],_pull=new Sprite[3],_settle=new Sprite[3];
        readonly Vector3 _center;
        Scene _scene;
        bool _disposed;

        public Camera Camera { get; }
        public Transform Actor { get; }
        public SpriteRenderer Body { get; }
        public Target[] Targets { get; }
        public float Height { get; }
        public Vector3 Right { get; }
        public Scene Scene => _scene;
        public float PullRowY => _sheet.height-5*_config.fh;
        public Vector3 Origin => Actor.position+Vector3.up*Height*.38f-Right*Height*.05f+Camera.transform.forward*Height*.18f;

        public SeraphineChainsFixture(bool twoTargets)
        {
            _previous=SceneManager.GetActiveScene();
            _exposure=Shader.GetGlobalFloat("_ActorExposure");
            try
            {
                _scene=EditorSceneManager.OpenScene(ProbeScene.Path,OpenSceneMode.Additive);
                SceneManager.SetActiveScene(_scene);
                foreach (var root in _scene.GetRootGameObjects())
                {
                    foreach (var b in root.GetComponentsInChildren<MonoBehaviour>(true)) b.enabled=false;
                    foreach (var c in root.GetComponentsInChildren<Camera>(true))
                    { if (c.name=="ArenaCamera") Camera=c; else c.enabled=false; }
                    foreach (var renderer in root.GetComponentsInChildren<Renderer>(true))
                    {
                        var materials=renderer.sharedMaterials;
                        for (int i=0;i<materials.Length;i++)
                            if (materials[i]!=null && materials[i].HasProperty("_WindStrength"))
                            {
                                materials[i]=new Material(materials[i]); _owned.Add(materials[i]);
                                materials[i].SetFloat("_WindStrength",0);
                            }
                        renderer.sharedMaterials=materials;
                    }
                }
                Require(Camera!=null,"authored camera missing"); Camera.enabled=false; Camera.scene=_scene;
                var tables=ArenaView.Tables.Load(); _config=tables.Class("seraphine");
                Shader.SetGlobalFloat("_ActorExposure",tables.globals.actor_exposure);
                _sheet=Load(_config.sheet); _idle=Frame(_config.idle_row,0);
                // Visual pose choices, not verified artist animation names. Rows are zero-based.
                for (int i=0;i<3;i++) { _reach[i]=Frame(3,i); _pull[i]=Frame(4,i); _settle[i]=Frame(6,i); }
                var material=new Material(Shader.Find("ROI/ActorSprite")); _owned.Add(material);
                var actor=new GameObject("Chain preview caster"); SceneManager.MoveGameObjectToScene(actor,_scene);
                Actor=actor.transform;
                var ray=Camera.ViewportPointToRay(new Vector3(.5f,.4f,0));
                new Plane(Vector3.up,Vector3.zero).Raycast(ray,out float distance); _center=ray.GetPoint(distance);
                BoundsOf(_sheet,_idle.rect,out int bottom,out int top,out _,out _);
                float sheetHeight=_config.fh*tables.cells_per_art_px*tables.sprite_scale*tables.actor_scale*_config.Scale;
                Height=sheetHeight*(top-bottom)/_config.fh;
                Right=Vector3.ProjectOnPlane(Camera.transform.right,Vector3.up).normalized;
                Actor.position=_center-Right*Height*1.45f;
                Body=CreateBody(Actor,_idle,sheetHeight,material); Body.flipX=true;
                Body.transform.localPosition=Vector3.up*(sheetHeight*(.5f-bottom/(float)_config.fh)+_config.lift);
                ActorContactShadow.Create(Actor).Bind(Height,_config.lift,1);

                var monster=tables.GeomFor("mob_b10_a"); var texture=Load(tables.SheetFor("mob_b10_a",null));
                int frame=monster.idle_frames[0],fw=monster.fw;
                var rect=new Rect(frame%monster.cols*fw,texture.height-(frame/monster.cols+1)*fw,fw,fw);
                BoundsOf(texture,rect,out int mb,out int mt,out int ml,out int mr);
                var sprite=Sprite.Create(texture,rect,new Vector2((ml+mr)*.5f/fw,mb/(float)fw),fw); _owned.Add(sprite);
                Vector3 depth=Vector3.ProjectOnPlane(Camera.transform.up,Vector3.up).normalized;
                Targets=new Target[twoTargets ? 2 : 1];
                for (int i=0;i<Targets.Length;i++)
                {
                    Vector3 start=i==0 ? _center+Right*Height*1.45f+(twoTargets ? depth*Height*1.1f : Vector3.zero)
                        : _center+Right*Height*1.55f-depth*Height*1.1f;
                    Targets[i]=new Target(this,sprite,material,Height*.95f*fw/(mt-mb),!monster.faces_left,start,i);
                }
            }
            catch { Dispose(); throw; }
        }

        public void FrameCamera(Vector2Int size)
        {
            Camera.aspect=size.x/(float)size.y; Camera.orthographic=false;
            float distance=Height*(Camera.aspect<1 ? 2.5f : 2.15f)
                /(Mathf.Tan(Camera.fieldOfView*.5f*Mathf.Deg2Rad)*Mathf.Min(1,Camera.aspect));
            Camera.transform.position=_center+Vector3.up*Height*.5f-Camera.transform.forward*distance;
        }

        public void Sample(float time)
        {
            float age=time-Start;
            if (age<0 || age>=.82f) Body.sprite=_idle;
            else if (age<IronChainsEffect.PullTime) Body.sprite=_reach[Mathf.Min(2,(int)(age/.065f))];
            else if (age<.58f) Body.sprite=_pull[Mathf.Min(2,(int)((age-IronChainsEffect.PullTime)/.065f))];
            else Body.sprite=_settle[Mathf.Min(2,(int)((age-.58f)/.08f))];
            foreach (var target in Targets) target.Sample(age);
        }

        SpriteRenderer CreateBody(Transform parent,Sprite sprite,float scale,Material material)
        {
            var body=new GameObject("Body").AddComponent<SpriteRenderer>(); body.transform.SetParent(parent,false);
            body.transform.rotation=Camera.transform.rotation; body.transform.localScale=Vector3.one*scale;
            body.sharedMaterial=material; body.sprite=sprite; return body;
        }

        Texture2D Load(string file)
        {
            var texture=new Texture2D(2,2,TextureFormat.RGBA32,false); _owned.Add(texture);
            texture.LoadImage(File.ReadAllBytes(Path.Combine(Application.streamingAssetsPath,"art",file)));
            texture.filterMode=FilterMode.Point; return texture;
        }

        Sprite Frame(int row,int frame)
        {
            var sprite=Sprite.Create(_sheet,new Rect(frame*_config.fw,_sheet.height-(row+1)*_config.fh,_config.fw,_config.fh),
                new Vector2(.5f,.5f),_config.fh);
            _owned.Add(sprite); return sprite;
        }

        static void BoundsOf(Texture2D texture,Rect rect,out int bottom,out int top,out int left,out int right)
        {
            int w=(int)rect.width,h=(int)rect.height; bottom=h; top=0; left=w; right=0;
            var pixels=texture.GetPixels((int)rect.x,(int)rect.y,w,h);
            for (int y=0;y<h;y++) for (int x=0;x<w;x++) if (pixels[y*w+x].a>.05f)
            { bottom=Mathf.Min(bottom,y); top=Mathf.Max(top,y+1); left=Mathf.Min(left,x); right=Mathf.Max(right,x+1); }
            Require(top>bottom,"empty sprite");
        }

        public void Dispose()
        {
            if (_disposed) return; _disposed=true;
            Shader.SetGlobalFloat("_ActorExposure",_exposure);
            if (_scene.IsValid() && _scene.isLoaded) EditorSceneManager.CloseScene(_scene,true);
            if (_previous.IsValid()) SceneManager.SetActiveScene(_previous);
            foreach (var item in _owned) if (item!=null) Object.DestroyImmediate(item);
        }

        internal sealed class Target
        {
            readonly SeraphineChainsFixture _fixture;
            readonly Vector3 _start;
            readonly SwordHitImpact _impact;
            readonly Renderer[] _outlines;
            public GameObject Root { get; }
            public SpriteRenderer Body { get; }
            public IronChainsEffect Chain { get; }
            public Vector3 Contact => Body.bounds.center-_fixture.Camera.transform.forward*_fixture.Height*.1f;

            public Target(SeraphineChainsFixture fixture,Sprite sprite,Material material,float scale,bool flip,Vector3 start,int index)
            {
                _fixture=fixture; _start=start;
                Root=new GameObject(index==0 ? "Scheduled chain target" : "Second scheduled chain target");
                SceneManager.MoveGameObjectToScene(Root,fixture.Scene);
                Body=fixture.CreateBody(Root.transform,sprite,scale,material); Body.flipX=flip;
                ActorContactShadow.Create(Root.transform).Bind(fixture.Height*.75f,0,1);
                Chain=IronChainsEffect.Create(fixture.Actor,fixture.Height,index==0);
                Require(Chain!=null,"chain resources missing");
                _impact=SwordHitImpact.Create(fixture.Actor,fixture.Height*.6f);
                _outlines=Array.FindAll(_impact.GetComponentsInChildren<Renderer>(),r=>r.name.EndsWith(" outline"));
            }

            public void Sample(float age)
            {
                float phase=Mathf.InverseLerp(IronChainsEffect.PullTime,IronChainsEffect.PullTime+.22f,age);
                float pull=1-Mathf.Pow(1-phase,4);
                float tension=Mathf.SmoothStep(0,1,Mathf.InverseLerp(IronChainsEffect.CatchTime,IronChainsEffect.PullTime,age))*(1-pull);
                Root.transform.position=_start+_fixture.Right*_fixture.Height*(tension*.055f-1.62f*pull)
                    +Vector3.up*(_fixture.Height*.07f*Mathf.Sin(phase*Mathf.PI));
                Body.transform.rotation=_fixture.Camera.transform.rotation*Quaternion.Euler(0,0,Mathf.Sin(phase*Mathf.PI)*-7);
                Chain.Sample(age,_fixture.Origin,Contact,_fixture.Camera);
                _impact.Sample(age-IronChainsEffect.CatchTime,Contact,_fixture.Right,_fixture.Camera);
                foreach (var r in _outlines) r.enabled=false;
            }
        }
    }
}
