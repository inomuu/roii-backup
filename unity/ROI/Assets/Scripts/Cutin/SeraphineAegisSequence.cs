using System;
using System.Collections.Generic;
using Roi.Sim;
using Roi.Arena;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.SceneManagement;
using UnityEngine.UI;

namespace Roi.Cutin
{
    // One presentation owner: cut-in and in-place field cast, without moving actors.
    [ExecuteAlways]
    public sealed class SeraphineAegisSequence : MonoBehaviour
    {
        public sealed class Actor
        {
            public Transform Root;
            public SpriteRenderer Body;
            public float Height;
            public bool Recipient, Caster;
        }
        sealed class Pose
        {
            public Actor Actor;
            public Color Tint;
            public MeshRenderer Ring, Shield;
            public BuffEffect Buff;
        }
        readonly List<Pose> _poses = new();
        readonly List<Material> _materials = new();
        readonly List<Mesh> _meshes = new();
        MaterialPropertyBlock _properties;
        readonly List<Light> _lights = new();
        readonly List<float> _energies = new();
        Camera _camera;
        SimDriver _driver;
        SeraphineUltimateCutin _cutin;
        Canvas _blocker;
        MeshRenderer _wave;
        Light _key;
        Color _ambient;
        Vector4 _fill;
        Vector3 _cameraPosition;
        Quaternion _cameraRotation, _shieldRotation;
        float _cameraFov, _lead, _exposure;
        bool _restored;
        public float Duration => _lead + 1.45f;
        public bool Restored => _restored;
        public Canvas CutinCanvas => _cutin != null ? _cutin.Surface : null;

        public static SeraphineAegisSequence Create(Camera camera, IList<Actor> actors,
            SimDriver driver, bool portrait)
        {
            var shader=Resources.Load<Shader>("Cutin/SeraphineAegisField");
            var texture=Resources.Load<Texture2D>("Cutin/SeraphineAuraFlow-v1");
            var buff=Resources.Load<BuffEffectProfile>("BuffEffects/AegisReceive");
            if (camera==null || shader==null || !shader.isSupported || texture==null || buff==null || buff.Prefab==null || actors.Count==0) return null;
            var go=new GameObject("Seraphine Aegis sequence");
            SceneManager.MoveGameObjectToScene(go,camera.gameObject.scene);
            var s=go.AddComponent<SeraphineAegisSequence>();
            try
            {
                s._properties=new MaterialPropertyBlock();
                s._camera=camera; s._driver=driver;
                s._lead=portrait ? SeraphineUltimateCutin.Duration : 0;
                s._cameraPosition=camera.transform.position; s._cameraRotation=camera.transform.rotation;
                s._shieldRotation=Quaternion.Euler(0,camera.transform.eulerAngles.y+180,0);
                s._cameraFov=camera.fieldOfView;
                s._ambient=RenderSettings.ambientLight; s._fill=Shader.GetGlobalVector("_FillColor");
                s._exposure=Shader.GetGlobalFloat("_ActorExposure");
                foreach (var root in camera.gameObject.scene.GetRootGameObjects())
                    foreach (var light in root.GetComponentsInChildren<Light>())
                        if (light.enabled) { s._lights.Add(light); s._energies.Add(light.intensity); }
                foreach (var actor in actors)
                    if (actor.Root!=null && actor.Body!=null)
                        s._poses.Add(new Pose { Actor=actor,Tint=actor.Body.color });
                if (!s._poses.Exists(p=>p.Actor.Caster)) throw new InvalidOperationException("Aegis caster is missing.");
                var quad=s.Quad();
                var shieldMesh=BuffShieldMesh.Create(); s._meshes.Add(shieldMesh);
                Material Material(int mode)
                {
                    var m=new Material(shader) { name="Aegis field layer "+mode };
                    m.SetTexture("_MainTex",texture); m.SetFloat("_Mode",mode); s._materials.Add(m); return m;
                }
                var ring=Material(0); var shield=Material(1); var wave=Material(3);
                shield.SetFloat("_Cull",(float)CullMode.Back);
                foreach (var p in s._poses)
                    if (p.Actor.Recipient)
                    {
                        p.Ring=s.Layer("Recipient ground",quad,ring);
                        p.Buff=new BuffEffect(buff,s.transform,p.Actor.Root,p.Actor.Height);
                        p.Shield=s.Layer("Aegis shield",shieldMesh,shield);
                    }
                s._wave=s.Layer("Caster wave",quad,wave);
                s._key=new GameObject("Aegis light").AddComponent<Light>();
                s._key.transform.SetParent(go.transform,false); s._key.type=LightType.Point;
                s._key.color=new Color(.16f,.68f,1); s._key.shadows=LightShadows.None;
                var block=new GameObject("Aegis input hold",typeof(RectTransform),typeof(Canvas),typeof(GraphicRaycaster));
                block.transform.SetParent(go.transform,false);
                s._blocker=block.GetComponent<Canvas>(); s._blocker.renderMode=RenderMode.ScreenSpaceOverlay;
                s._blocker.sortingOrder=89;
                var image=new GameObject("Block",typeof(RectTransform)).AddComponent<Image>();
                image.transform.SetParent(block.transform,false); image.color=Color.clear;
                image.rectTransform.anchorMin=Vector2.zero; image.rectTransform.anchorMax=Vector2.one;
                image.rectTransform.offsetMin=image.rectTransform.offsetMax=Vector2.zero;
                if (portrait)
                {
                    s._cutin=SeraphineUltimateCutin.Create(false);
                    if (s._cutin==null) throw new InvalidOperationException("Aegis cut-in assets missing.");
                    SceneManager.MoveGameObjectToScene(s._cutin.gameObject,go.scene);
                    s._cutin.transform.SetParent(go.transform,false);
                }
                if (driver!=null) driver.HoldPresentation(s);
                s.Sample(0);
                return s;
            }
            catch (Exception e)
            {
                Debug.LogWarning("AEGIS: "+e.Message); s.Restore(); DestroyObject(go); return null;
            }
        }

        public void Sample(float seconds)
        {
            if (_restored) return;
            if (_camera==null || _poses.Exists(p=>p.Actor.Root==null || p.Actor.Body==null)) { Restore(); return; }
            if (seconds>=Duration) { Restore(); return; }
            float t=seconds-_lead;
            float dim=Smooth(-.10f,.20f,t)*(1-Smooth(1.02f,1.45f,t));
            if (_cutin!=null)
            {
                _cutin.Surface.enabled=seconds<SeraphineUltimateCutin.Duration;
                _cutin.Sample(seconds,_camera.aspect);
            }
            for (int i=0;i<_lights.Count;i++) if (_lights[i]!=null) _lights[i].intensity=_energies[i]*Mathf.Lerp(1,.48f,dim);
            RenderSettings.ambientLight=_ambient*Mathf.Lerp(1,.55f,dim);
            Shader.SetGlobalVector("_FillColor",_fill*Mathf.Lerp(1,.48f,dim));
            Shader.SetGlobalFloat("_ActorExposure",_exposure*Mathf.Lerp(1,1.65f,dim));
            var caster=_poses.Find(p=>p.Actor.Caster);
            float unit=Mathf.Max(.3f,caster.Actor.Height);
            float charge=Smooth(.18f,.34f,t)*(1-Smooth(.42f,.75f,t));
            float receive=Smooth(.48f,.62f,t)*(1-Smooth(1.03f,1.38f,t));
            float burst=Smooth(.31f,.39f,t)*(1-Smooth(.65f,1.02f,t));
            float expand=1-Mathf.Pow(1-Mathf.Clamp01((t-.32f)/.65f),3);
            foreach (var p in _poses)
            {
                p.Actor.Body.color=p.Tint*(p.Actor.Recipient || p.Actor.Caster ? Color.white : Color.Lerp(Color.white,new Color(.5f,.58f,.65f,1),dim));
                if (!p.Actor.Recipient) continue;
                float h=Mathf.Max(.3f,p.Actor.Height);
                var ground=p.Actor.Root.position+Vector3.up*.055f;
                Set(p.Ring,ground,Quaternion.Euler(90,0,0),new Vector3(h*1.45f,h*1.45f,1),
                    Mathf.Max(receive*.75f,p.Actor.Caster ? charge*.75f : 0),t,.72f);
                p.Buff.Sample(t-.40f);
                float shieldScale=Mathf.Lerp(.86f,1,Smooth(.48f,.68f,t));
                // Keep the pointed silhouette clear of the tilted sprite's depth plane.
                var shieldFront=_cameraRotation*new Vector3(0,0,-.65f)*h;
                Set(p.Shield,ground+shieldFront+Vector3.up*h*.04f,_shieldRotation,
                    new Vector3(h*shieldScale,h*1.06f,h*shieldScale),receive*.85f,t,0);
            }
            float span=unit*2.4f;
            foreach (var p in _poses) if (p.Actor.Recipient) span=Mathf.Max(span,Vector3.Distance(caster.Actor.Root.position,p.Actor.Root.position)+unit);
            Set(_wave,caster.Actor.Root.position+Vector3.up*.065f,Quaternion.Euler(90,0,0),
                new Vector3(span*2.3f,span*2.3f,1),burst,t,Mathf.Lerp(.06f,.92f,expand));
            _key.transform.position=caster.Actor.Root.position+Vector3.up*unit;
            _key.range=span*2; _key.intensity=(charge+burst*.8f+receive*.35f)*unit*unit*2;
        }

        public void Restore()
        {
            if (_restored) return;
            _restored=true;
            foreach (var p in _poses)
            {
                p.Buff?.Dispose();
                if (p.Actor.Body!=null) p.Actor.Body.color=p.Tint;
            }
            for (int i=0;i<_lights.Count;i++) if (_lights[i]!=null) _lights[i].intensity=_energies[i];
            RenderSettings.ambientLight=_ambient; Shader.SetGlobalVector("_FillColor",_fill);
            Shader.SetGlobalFloat("_ActorExposure",_exposure);
            if (_camera!=null)
            {
                _camera.transform.SetPositionAndRotation(_cameraPosition,_cameraRotation);
                _camera.fieldOfView=_cameraFov;
            }
            foreach (var renderer in GetComponentsInChildren<Renderer>()) renderer.enabled=false;
            if (_key!=null) _key.enabled=false;
            if (_blocker!=null) _blocker.enabled=false;
            if (_cutin!=null) _cutin.Surface.enabled=false;
            if (_driver!=null) _driver.ReleasePresentation(this);
        }
        void OnDisable() => Restore();
        void OnDestroy()
        {
            Restore();
            foreach (var m in _materials) if (m!=null) DestroyObject(m);
            foreach (var m in _meshes) if (m!=null) DestroyObject(m);
        }
        public static void DestroyObject(UnityEngine.Object o)
        {
            if (Application.isPlaying) Destroy(o); else DestroyImmediate(o);
        }
        static float Smooth(float a,float b,float t) => Mathf.SmoothStep(0,1,Mathf.InverseLerp(a,b,t));
        void Set(MeshRenderer renderer,Vector3 at,Quaternion rotation,Vector3 scale,float alpha,float phase,float radius)
        {
            renderer.enabled=alpha>.0001f;
            renderer.transform.SetPositionAndRotation(at,rotation); renderer.transform.localScale=scale;
            _properties.Clear(); _properties.SetFloat("_Opacity",alpha);
            _properties.SetFloat("_Phase",phase); _properties.SetFloat("_Radius",radius);
            renderer.SetPropertyBlock(_properties);
        }
        MeshRenderer Layer(string name,Mesh mesh,Material material)
        {
            var go=new GameObject(name,typeof(MeshFilter),typeof(MeshRenderer)); go.transform.SetParent(transform,false);
            go.GetComponent<MeshFilter>().sharedMesh=mesh;
            var r=go.GetComponent<MeshRenderer>(); r.sharedMaterial=material;
            r.shadowCastingMode=ShadowCastingMode.Off; r.receiveShadows=false; return r;
        }
        Mesh Quad()
        {
            var mesh=new Mesh { name="Aegis quad" };
            mesh.vertices=new[] { new Vector3(-.5f,-.5f,0),new Vector3(.5f,-.5f,0),new Vector3(.5f,.5f,0),new Vector3(-.5f,.5f,0) };
            mesh.uv=new[] { Vector2.zero,Vector2.right,Vector2.one,Vector2.up };
            mesh.triangles=new[] { 0,1,2,0,2,3 }; mesh.RecalculateBounds(); _meshes.Add(mesh); return mesh;
        }
    }
}
