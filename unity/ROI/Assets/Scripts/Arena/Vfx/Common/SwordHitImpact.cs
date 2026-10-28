using UnityEngine;
using UnityEngine.Rendering;

namespace Roi.Arena
{
    // Presentation only: the caller supplies hit position, swing tangent and age of a confirmed hit.
    public sealed class SwordHitImpact : MonoBehaviour
    {
        public const float Duration=.38f;
        Mesh _diamond,_quad,_outlineDiamond;
        Material _edgeMaterial,_haloMaterial,_sparkMaterial,_outlineMaterial;
        MeshRenderer _core,_cross,_halo,_coreOutline,_crossOutline;
        ParticleSystem _sparks;
        ParticleSystemRenderer _sparkRenderer;
        MaterialPropertyBlock _block;
        float _height;
        bool _fixedCarrier, _placed;

        public void FreezeAtSpawn() => _fixedCarrier=true;

        public static SwordHitImpact Create(Transform parent,float actorHeight)
        {
            var source=Resources.Load<Material>("Effects/SeraphineBasicSlash");
            var spark=Resources.Load<Shader>("Effects/CircularSlashGlow");
            var halo=Resources.Load<Shader>("Effects/SwordImpactHalo");
            if (parent==null || source==null || spark==null || halo==null) return null;
            var root=new GameObject("Sword hit impact"); root.transform.SetParent(parent,false);
            root.layer=parent.gameObject.layer;
            var fx=root.AddComponent<SwordHitImpact>(); fx._height=Mathf.Max(.01f,actorHeight);
            fx._block=new MaterialPropertyBlock();
            fx._diamond=new Mesh { name="Sharp impact diamond" };
            fx._diamond.vertices=new[] { new Vector3(-.35f,0,0),new Vector3(0,-.045f,0),
                new Vector3(.5f,0,0),new Vector3(0,.045f,0) };
            fx._diamond.colors=new[] { Color.white,Color.white,Color.white,Color.white };
            fx._diamond.triangles=new[] { 0,1,2,0,2,3 }; fx._diamond.RecalculateBounds();
            fx._outlineDiamond=Instantiate(fx._diamond); fx._outlineDiamond.name="Impact blue outline diamond";
            var outlineColor=new Color(.005f,.16f,.85f);
            fx._outlineDiamond.colors=new[] { outlineColor,outlineColor,outlineColor,outlineColor };
            fx._quad=new Mesh { name="Impact blue halo quad" };
            fx._quad.vertices=new[] { new Vector3(-.5f,-.5f,0),new Vector3(.5f,-.5f,0),
                new Vector3(-.5f,.5f,0),new Vector3(.5f,.5f,0) };
            fx._quad.uv=new[] { Vector2.zero,Vector2.right,Vector2.up,Vector2.one };
            var blue=new Color(.002f,.18f,1);
            fx._quad.colors=new[] { blue,blue,blue,blue };
            fx._quad.triangles=new[] { 0,2,1,1,2,3 }; fx._quad.RecalculateBounds();
            fx._edgeMaterial=new Material(source) { name="Impact white core",renderQueue=3103 };
            fx._edgeMaterial.SetFloat("_EmissionGain",4.5f);
            fx._outlineMaterial=new Material(source) { name="Impact blue outline",renderQueue=3102 };
            fx._outlineMaterial.SetFloat("_EmissionGain",1.5f);
            fx._haloMaterial=new Material(halo) { name="Impact blue halo",renderQueue=3101 };
            fx._haloMaterial.SetFloat("_EmissionGain",5);
            fx._core=fx.Layer("Impact needle",fx._diamond,fx._edgeMaterial);
            fx._cross=fx.Layer("Impact cross glint",fx._diamond,fx._edgeMaterial);
            fx._cross.transform.localRotation=Quaternion.Euler(0,0,72);
            fx._coreOutline=fx.Layer("Impact needle outline",fx._outlineDiamond,fx._outlineMaterial);
            fx._crossOutline=fx.Layer("Impact cross outline",fx._outlineDiamond,fx._outlineMaterial);
            fx._halo=fx.Layer("Impact blue halo",fx._quad,fx._haloMaterial);
            fx.CreateSparks(spark); fx.Hide(); return fx;
        }

        // Instance-owned meshes/materials only; existing callers retain the original blue palette.
        public void SetAccent(Color halo,Color outline,Color sparks)
        {
            _quad.colors=new[] { halo,halo,halo,halo };
            _outlineDiamond.colors=new[] { outline,outline,outline,outline };
            var main=_sparks.main; main.startColor=sparks;
        }

        public void Sample(float age,Vector3 contact,Vector3 swingTangent,Camera camera,bool visible=true)
        {
            if (!visible || !isActiveAndEnabled || camera==null || !(age>=0 && age<Duration)) { Hide(); return; }
            var normal=camera.transform.forward;
            var along=Vector3.ProjectOnPlane(swingTangent,normal);
            if (along.sqrMagnitude<.0001f) along=camera.transform.right;
            along.Normalize();
            if (!_fixedCarrier || !_placed)
            {
                transform.SetPositionAndRotation(contact-normal*_height*.08f,
                    Quaternion.LookRotation(normal,Vector3.Cross(normal,along).normalized));
                transform.localScale=Vector3.one*_height;
            }
            float coreT=Mathf.Clamp01(age/.19f),crossT=Mathf.Clamp01(age/.13f),haloT=Mathf.Clamp01(age/.27f);
            if (!_fixedCarrier || !_placed)
            {
                _core.transform.localPosition=Vector3.right*(coreT*.1f);
                _core.transform.localScale=new Vector3(Mathf.Lerp(.75f,1.35f,coreT),Mathf.Lerp(1.1f,.25f,coreT),1);
                _cross.transform.localScale=new Vector3(.46f,Mathf.Lerp(.85f,.1f,crossT),1);
                _halo.transform.localScale=new Vector3(Mathf.Lerp(1.35f,1.75f,haloT),Mathf.Lerp(1.05f,1.4f,haloT),1);
            }
            ShowLayer(_core,Fade(coreT)); ShowLayer(_cross,Fade(crossT)*.8f);
            ShowOutline(_coreOutline,_core,coreT);
            ShowOutline(_crossOutline,_cross,crossT);
            _placed=true;
            ShowLayer(_halo,Fade(haloT)*.8f);
            _sparkRenderer.enabled=true;
            _sparks.Simulate(age,false,true,true);
        }

        static float Fade(float t) { float value=1-Mathf.SmoothStep(0,1,t); return value*value; }
        void ShowOutline(MeshRenderer outline,MeshRenderer core,float phase)
        {
            outline.transform.localPosition=core.transform.localPosition;
            outline.transform.localRotation=core.transform.localRotation;
            outline.transform.localScale=Vector3.Scale(core.transform.localScale,new Vector3(1.08f,1.7f,1));
            // Keep a solid coloured edge over the HDR white flash, then fade with the glint's tail.
            ShowLayer(outline,1-Mathf.SmoothStep(0,1,Mathf.InverseLerp(.65f,1,phase)));
        }
        void ShowLayer(MeshRenderer renderer,float opacity)
        {
            _block.SetFloat("_Opacity",opacity); renderer.SetPropertyBlock(_block);
            renderer.enabled=opacity>0;
        }
        MeshRenderer Layer(string name,Mesh mesh,Material material)
        {
            var go=new GameObject(name,typeof(MeshFilter),typeof(MeshRenderer));
            go.layer=gameObject.layer; go.transform.SetParent(transform,false);
            go.GetComponent<MeshFilter>().sharedMesh=mesh;
            var renderer=go.GetComponent<MeshRenderer>(); renderer.sharedMaterial=material;
            renderer.shadowCastingMode=ShadowCastingMode.Off; renderer.receiveShadows=false;
            return renderer;
        }
        void CreateSparks(Shader shader)
        {
            var go=new GameObject("Impact directional sparks"); go.layer=gameObject.layer;
            go.transform.SetParent(transform,false); _sparks=go.AddComponent<ParticleSystem>();
            _sparks.Stop(true,ParticleSystemStopBehavior.StopEmittingAndClear);
            _sparks.useAutoRandomSeed=false; _sparks.randomSeed=1827;
            var main=_sparks.main; main.loop=false; main.playOnAwake=false; main.duration=Duration;
            main.startLifetime=new ParticleSystem.MinMaxCurve(.17f,.29f);
            main.startSpeed=new ParticleSystem.MinMaxCurve(1.2f,2.6f);
            main.startSize=new ParticleSystem.MinMaxCurve(.025f,.045f);
            main.startColor=new Color(.4f,.85f,1); main.maxParticles=10;
            main.simulationSpace=ParticleSystemSimulationSpace.Local;
            main.scalingMode=ParticleSystemScalingMode.Hierarchy;
            var shape=_sparks.shape; shape.shapeType=ParticleSystemShapeType.Cone;
            shape.angle=22; shape.radius=.025f; shape.rotation=new Vector3(0,90,0);
            var emission=_sparks.emission; emission.rateOverTime=0;
            emission.SetBursts(new[] { new ParticleSystem.Burst(0,(short)10) });
            var fade=_sparks.colorOverLifetime; fade.enabled=true;
            var gradient=new Gradient();
            gradient.SetKeys(new[] { new GradientColorKey(Color.white,0),new GradientColorKey(Color.white,1) },
                new[] { new GradientAlphaKey(1,0),new GradientAlphaKey(1,.15f),new GradientAlphaKey(0,1) });
            fade.color=gradient;
            var size=_sparks.sizeOverLifetime; size.enabled=true;
            size.size=new ParticleSystem.MinMaxCurve(1,AnimationCurve.EaseInOut(0,1,1,.1f));
            _sparkMaterial=new Material(shader) { name="Impact spark glow",renderQueue=3104 };
            _sparkMaterial.SetFloat("_Spark",1); _sparkMaterial.SetFloat("_EmissionGain",4);
            _sparkRenderer=go.GetComponent<ParticleSystemRenderer>(); _sparkRenderer.sharedMaterial=_sparkMaterial;
            _sparkRenderer.renderMode=ParticleSystemRenderMode.Stretch;
            _sparkRenderer.lengthScale=2; _sparkRenderer.velocityScale=.06f;
            _sparkRenderer.cameraVelocityScale=0;
            _sparkRenderer.shadowCastingMode=ShadowCastingMode.Off; _sparkRenderer.receiveShadows=false;
        }
        public void Hide()
        {
            if (_core!=null) _core.enabled=false;
            if (_cross!=null) _cross.enabled=false;
            if (_halo!=null) _halo.enabled=false;
            if (_coreOutline!=null) _coreOutline.enabled=false;
            if (_crossOutline!=null) _crossOutline.enabled=false;
            if (_sparkRenderer!=null) _sparkRenderer.enabled=false;
            if (_sparks!=null) _sparks.Stop(false,ParticleSystemStopBehavior.StopEmittingAndClear);
        }
        void OnDisable() => Hide();
        void OnDestroy()
        {
            Release(_diamond); Release(_quad); Release(_edgeMaterial); Release(_haloMaterial); Release(_sparkMaterial);
            Release(_outlineDiamond); Release(_outlineMaterial);
        }
        static void Release(Object asset)
        { if (asset!=null) { if (Application.isPlaying) Destroy(asset); else DestroyImmediate(asset); } }
    }
}
