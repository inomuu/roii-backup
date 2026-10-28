using System;
using System.Collections.Generic;
using System.IO;
using Roi;
using Roi.Arena;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;
using Object=UnityEngine.Object;

namespace Roi.Editor.Vfx
{
    internal sealed class SuzuSunderFixture : IDisposable
    {
        public const float Start=.35f;
        public const int Frames=168, Fps=60;
        readonly List<Object> _owned=new();
        readonly Scene _previous;
        readonly float _exposure;
        readonly Sprite[] _attack;
        readonly Sprite _idle;
        readonly Vector3 _center;
        readonly SpriteRenderer[] _targets=new SpriteRenderer[2];
        readonly Vector3[] _targetHeads=new Vector3[2];
        readonly MaterialPropertyBlock[] _flash=new MaterialPropertyBlock[2];
        Scene _scene;
        bool _disposed;
        public Camera Camera { get; }
        public Transform Actor { get; }
        public SpriteRenderer Body { get; }
        public SuzuSunderEffect Effect { get; }
        public SuzuSlashBaseEffect BaseEffect { get; }
        public SwordHitImpact[] Impacts { get; }=new SwordHitImpact[2];
        public float Height { get; }
        public Vector3 TargetContact(int index) => _targets[index].bounds.center;
        public Vector3 TargetHead(int index) => _targets[index].transform.TransformPoint(_targetHeads[index]);
        public void SetTargetVisible(int index,bool visible) => _targets[index].transform.parent.gameObject.SetActive(visible);

        public SuzuSunderFixture(bool baseOnly=false)
        {
            _previous=SceneManager.GetActiveScene(); _exposure=Shader.GetGlobalFloat("_ActorExposure");
            try
            {
                _scene=EditorSceneManager.OpenScene(ProbeScene.Path,OpenSceneMode.Additive);
                SceneManager.SetActiveScene(_scene);
                foreach (var root in _scene.GetRootGameObjects())
                {
                    // Freeze gameplay, not the authored lighting and camera render settings.
                    foreach (var b in root.GetComponentsInChildren<MonoBehaviour>(true))
                        if (!(b is Volume) && !(b is UniversalAdditionalCameraData)
                            && !(b is UniversalAdditionalLightData)) b.enabled=false;
                    foreach (var volume in root.GetComponentsInChildren<Volume>(true))
                    {
                        if (volume.sharedProfile==null) continue;
                        var profile=ScriptableObject.CreateInstance<VolumeProfile>();
                        profile.hideFlags=HideFlags.HideAndDontSave; _owned.Add(profile);
                        foreach (var source in volume.sharedProfile.components)
                        {
                            var component=Object.Instantiate(source);
                            component.hideFlags=HideFlags.HideAndDontSave;
                            profile.components.Add(component); _owned.Add(component);
                        }
                        // Close-up inspection needs more exposure and no arena-distance focus blur.
                        if (profile.TryGet<ColorAdjustments>(out var color))
                            color.postExposure.Override(color.postExposure.value+.8f);
                        if (profile.TryGet<DepthOfField>(out var focus)) focus.active=false;
                        volume.sharedProfile=profile;
                    }
                    foreach (var c in root.GetComponentsInChildren<Camera>(true))
                    { if (c.name=="ArenaCamera") Camera=c; else c.enabled=false; }
                    foreach (var r in root.GetComponentsInChildren<Renderer>(true))
                    {
                        var materials=r.sharedMaterials;
                        for (int i=0;i<materials.Length;i++)
                            if (materials[i]!=null && materials[i].HasProperty("_WindStrength"))
                            {
                                materials[i]=new Material(materials[i]); _owned.Add(materials[i]);
                                materials[i].SetFloat("_WindStrength",0);
                            }
                        r.sharedMaterials=materials;
                    }
                }
                Require(Camera!=null,"authored camera missing"); Camera.enabled=false; Camera.scene=_scene;
                var tables=ArenaView.Tables.Load();
                // Explicit editor stand-in, not a gameplay class or a new global art alias.
                var config=tables.Class("swordsman_t1");
                Shader.SetGlobalFloat("_ActorExposure",tables.globals.actor_exposure);
                var sheet=Load(config.sheet);
                _idle=Frame(sheet,config.idle_row,0,config.fw,config.fh);
                _attack=new Sprite[config.atk_frames];
                for (int i=0;i<_attack.Length;i++) _attack[i]=Frame(sheet,config.atk_row,i,config.fw,config.fh);
                BoundsOf(sheet,_idle.rect,out int bottom,out int top);
                float sheetHeight=config.fh*tables.cells_per_art_px*tables.sprite_scale*tables.actor_scale*config.Scale;
                Height=sheetHeight*(top-bottom)/config.fh;
                var material=new Material(Shader.Find("ROI/ActorSprite")); _owned.Add(material);
                var ray=Camera.ViewportPointToRay(new Vector3(.5f,.4f,0));
                new Plane(Vector3.up,Vector3.zero).Raycast(ray,out float distance); _center=ray.GetPoint(distance);
                var right=Vector3.ProjectOnPlane(Camera.transform.right,Vector3.up).normalized;
                var depth=Vector3.ProjectOnPlane(Camera.transform.up,Vector3.up).normalized;
                Actor=new GameObject("Suzu placeholder caster").transform;
                SceneManager.MoveGameObjectToScene(Actor.gameObject,_scene);
                Actor.position=_center-right*Height*.48f;
                Body=BodyAt(Actor,_idle,material,sheetHeight);
                Body.flipX=config.faces_left;
                Body.transform.localPosition=Vector3.up*(sheetHeight*(.5f-bottom/(float)config.fh)+config.lift);
                ActorContactShadow.Create(Actor).Bind(Height,config.lift,1);
                Effect=SuzuSunderEffect.Create(Body,Actor,Height); Require(Effect!=null,"Sunder resources missing");
                if (baseOnly)
                {
                    BaseEffect=SuzuSlashBaseEffect.Create(Body,Actor,Height);
                    Require(BaseEffect!=null,"base study resources missing");
                }
                var monster=tables.GeomFor("mob_b10_a"); var texture=Load(tables.SheetFor("mob_b10_a",null));
                int frame=monster.idle_frames[0],fw=monster.fw;
                var targetSprite=Frame(texture,frame/monster.cols,frame%monster.cols,fw,fw);
                BoundsOf(texture,targetSprite.rect,out int low,out int high);
                var crown=texture.GetPixels((int)targetSprite.rect.x,(int)targetSprite.rect.y+high-1,fw,1);
                int left=fw,rightEdge=0;
                for (int x=0;x<fw;x++) if (crown[x].a>.05f)
                { left=Mathf.Min(left,x); rightEdge=Mathf.Max(rightEdge,x+1); }
                for (int i=0;i<2;i++)
                {
                    var root=new GameObject("Sunder target "+(i+1)).transform;
                    SceneManager.MoveGameObjectToScene(root.gameObject,_scene);
                    root.position=Actor.position+right*Height*1.2f+depth*Height*(i==0 ? .66f : -.66f);
                    float scale=Height*.85f*fw/(high-low);
                    _targets[i]=BodyAt(root,targetSprite,material,scale); _targets[i].flipX=!monster.faces_left;
                    float headX=(left+rightEdge)*.5f/fw-.5f;
                    _targetHeads[i]=new Vector3(_targets[i].flipX ? -headX : headX,high/(float)fw-.5f,0);
                    _targets[i].transform.localPosition=Vector3.up*scale*(.5f-low/(float)fw);
                    ActorContactShadow.Create(root).Bind(Height*.7f,0,1);
                    Impacts[i]=SwordHitImpact.Create(Actor,Height*.82f);
                    Require(Impacts[i]!=null,"impact resources missing");
                    Impacts[i].SetAccent(new Color(1,.09f,.003f),new Color(.85f,.1f,.003f),new Color(1,.5f,.05f));
                }
            }
            catch { Dispose(); throw; }
        }

        public void FrameCamera(Vector2Int size)
        {
            Camera.aspect=size.x/(float)size.y; Camera.orthographic=false;
            float distance=Height*2.55f/(Mathf.Tan(Camera.fieldOfView*.5f*Mathf.Deg2Rad)*Mathf.Min(1,Camera.aspect));
            Camera.transform.position=_center+Vector3.up*Height*.42f-Camera.transform.forward*distance;
        }
        public void Sample(float time)
        {
            float age=time-Start;
            int pose=age<.13f ? 0 : age<.26f ? 1 : age<.4f ? 2 : age<.5f ? 3 : age<.59f ? 4 : 5;
            Body.sprite=age<0 || age>1.42f ? _idle : _attack[Mathf.Min(pose,_attack.Length-1)];
            if (BaseEffect!=null)
            {
                Effect.Hide(); BaseEffect.Sample(age);
                for (int i=0;i<Impacts.Length;i++)
                {
                    Impacts[i].Hide(); ActorHitFlash.Apply(_targets[i],ref _flash[i],0);
                }
                return;
            }
            Effect.Sample(age);
            for (int i=0;i<Impacts.Length;i++)
            {
                var contact=_targets[i].bounds.center;
                Impacts[i].Sample(age-SuzuSunderEffect.HitTime,contact,Camera.transform.right+Camera.transform.up*.3f,Camera);
                // Restrained warm flash only on these fixture targets, so the contact remains readable.
                ActorHitFlash.Apply(_targets[i],ref _flash[i],age>=SuzuSunderEffect.HitTime
                    ? ArenaView.FlashDur-(age-SuzuSunderEffect.HitTime) : 0);
                _flash[i].SetColor("_FlashColor",new Color(1.15f,.44f,.12f));
                _flash[i].SetFloat("_Flash",_flash[i].GetFloat("_Flash")*.42f);
                _targets[i].SetPropertyBlock(_flash[i]);
            }
        }
        SpriteRenderer BodyAt(Transform root,Sprite sprite,Material material,float scale)
        {
            var body=new GameObject("Body").AddComponent<SpriteRenderer>(); body.transform.SetParent(root,false);
            body.transform.rotation=Camera.transform.rotation; body.transform.localScale=Vector3.one*scale;
            body.sharedMaterial=material; body.sprite=sprite; return body;
        }
        Texture2D Load(string file)
        {
            var texture=new Texture2D(2,2,TextureFormat.RGBA32,false); _owned.Add(texture);
            texture.LoadImage(File.ReadAllBytes(Path.Combine(Application.streamingAssetsPath,"art",file)));
            texture.filterMode=FilterMode.Point; return texture;
        }
        Sprite Frame(Texture2D texture,int row,int column,int width,int height)
        {
            var sprite=Sprite.Create(texture,new Rect(column*width,texture.height-(row+1)*height,width,height),
                new Vector2(.5f,.5f),height); _owned.Add(sprite); return sprite;
        }
        static void BoundsOf(Texture2D texture,Rect rect,out int bottom,out int top)
        {
            int w=(int)rect.width,h=(int)rect.height; bottom=h; top=0;
            var pixels=texture.GetPixels((int)rect.x,(int)rect.y,w,h);
            for (int y=0;y<h;y++) for (int x=0;x<w;x++) if (pixels[y*w+x].a>.05f)
            { bottom=Mathf.Min(bottom,y); top=Mathf.Max(top,y+1); }
            Require(top>bottom,"empty sprite");
        }
        public static void Require(bool value,string message)
        { if (!value) throw new InvalidOperationException("SUZU SUNDER: "+message); }
        public void Dispose()
        {
            if (_disposed) return; _disposed=true;
            Shader.SetGlobalFloat("_ActorExposure",_exposure);
            if (_scene.IsValid() && _scene.isLoaded) EditorSceneManager.CloseScene(_scene,true);
            if (_previous.IsValid()) SceneManager.SetActiveScene(_previous);
            foreach (var item in _owned) if (item!=null) Object.DestroyImmediate(item);
        }
    }
}
