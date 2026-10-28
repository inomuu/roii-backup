using UnityEngine;
using UnityEngine.Rendering;

namespace Roi.Arena
{
    // Presentation clock only. Targets, damage, break and DEF reduction belong to the sim.
    public sealed class SuzuSunderEffect : MonoBehaviour
    {
        public const float HitTime=.5f, Duration=1.7f;
        SpriteRenderer _body;
        Transform _anchor,_blade;
        float _height;
        Arc _core,_rim,_halo,_flame;
        Mesh _groundMesh;
        Material _groundMaterial,_emberMaterial;
        MeshRenderer _ground;
        ParticleSystem _embers;
        ParticleSystemRenderer _emberRenderer;

        public static SuzuSunderEffect Create(SpriteRenderer body,Transform anchor,float height)
        {
            var mesh=Resources.Load<Mesh>("Effects/Suzu/SuzuSlashBaseMesh");
            var bladeMask=Resources.Load<Texture2D>("Effects/SeraphineSlashMask-v1");
            var glow=Resources.Load<Shader>("Effects/CircularSlashGlow");
            var detail=Resources.Load<Shader>("Effects/Suzu/SuzuSlashArc");
            var ground=Resources.Load<Shader>("Effects/BuffGroundGlow");
            var mask=Resources.Load<Texture2D>("Effects/Suzu/SuzuFireTrailMask-v1");
            var flow=Resources.Load<Texture2D>("Cutin/SeraphineAuraFlow-v1");
            if (body==null || anchor==null || mesh==null || bladeMask==null || glow==null || detail==null
                || ground==null || mask==null || flow==null) return null;
            var root=new GameObject("Suzu Sunder"); root.transform.SetParent(anchor,false);
            root.layer=body.gameObject.layer;
            var fx=root.AddComponent<SuzuSunderEffect>();
            fx._body=body; fx._anchor=anchor; fx._height=Mathf.Max(.01f,height);
            fx._blade=new GameObject("Sunder sweep").transform; fx._blade.SetParent(root.transform,false);
            var bladeRect=new Vector4(0,.44f,1,.19f);
            fx._rim=new Arc(fx._blade,"Sunder orange edge",mesh,detail,bladeMask,flow,bladeRect,
                new Color(1,.12f,.003f),new Color(1,.12f,.003f),2.2f,1,1,0,3000);
            fx._core=new Arc(fx._blade,"Sunder hot core",mesh,detail,bladeMask,flow,bladeRect,
                new Color(1,.89f,.65f),new Color(1,.89f,.65f),4.5f,1,.78f,0,3003);
            fx._halo=new Arc(fx._blade,"Sunder fire wake",mesh,detail,mask,flow,new Vector4(0,0,1,1),
                new Color(1,.12f,.003f),new Color(1,.035f,.002f),1.5f,1.27f,1,1,3001);
            fx._flame=new Arc(fx._blade,"Sunder textured flame",mesh,detail,mask,flow,new Vector4(0,0,1,1),
                new Color(1,.65f,.09f),new Color(1,.045f,.002f),3.2f,1.22f,1,0,3002);
            fx.CreateGround(ground); fx.CreateEmbers(glow);
            foreach (var t in root.GetComponentsInChildren<Transform>(true)) t.gameObject.layer=root.layer;
            fx.Hide(); return fx;
        }

        public void Sample(float age,bool visible=true)
        {
            if (!visible || !isActiveAndEnabled || _body==null || !_body.enabled || _anchor==null
                || !(age>=0 && age<Duration)) { Hide(); return; }
            // Normalize parent scale so the effect is measured in visible world-space actor heights.
            transform.SetPositionAndRotation(_anchor.position,Quaternion.Euler(0,_body.transform.eulerAngles.y,0));
            var scale=_anchor.lossyScale;
            transform.localScale=new Vector3((_body.flipX ? -1 : 1)*_height/Mathf.Max(.001f,Mathf.Abs(scale.x)),
                _height/Mathf.Max(.001f,Mathf.Abs(scale.y)),_height/Mathf.Max(.001f,Mathf.Abs(scale.z)));
            _blade.localPosition=Vector3.up*.46f;
            _blade.localRotation=Quaternion.identity;
            float phase=Mathf.SmoothStep(0,1,Mathf.InverseLerp(.36f,.63f,age));
            float secondTurn=Mathf.SmoothStep(0,1,Mathf.InverseLerp(.63f,1.18f,age));
            float tail=1-Mathf.SmoothStep(0,1,Mathf.InverseLerp(1.20f,Duration,age));
            float opacity=Mathf.SmoothStep(0,1,Mathf.InverseLerp(.36f,.41f,age))*tail*tail*_body.color.a;
            // Two UV passes over the fixed 270-degree carrier, not two geometric revolutions.
            float flowPhase=phase+secondTurn;
            float erosion=Mathf.InverseLerp(1.18f,1.65f,age)*1.04f;
            _rim.Sample(opacity,flowPhase,erosion);
            _core.Sample(opacity,flowPhase,erosion);
            _halo.Sample(opacity*.55f,flowPhase-.04f,erosion);
            _flame.Sample(opacity*.85f,flowPhase-.02f,erosion);
            float charge=Mathf.SmoothStep(0,1,Mathf.InverseLerp(.05f,.4f,age));
            _groundMaterial.SetFloat("_Opacity",charge*tail*tail*.42f*_body.color.a);
            _ground.enabled=charge*tail>0;
            _emberMaterial.SetFloat("_Opacity",tail*_body.color.a);
            _emberRenderer.enabled=age>=HitTime;
            if (age>=HitTime) _embers.Simulate(age-HitTime,false,true,true);
            else _embers.Stop(false,ParticleSystemStopBehavior.StopEmittingAndClear);
        }

        void CreateGround(Shader shader)
        {
            _groundMesh=new Mesh { name="Sunder foot light" };
            _groundMesh.vertices=new[] { new Vector3(-.9f,0,-.9f),new Vector3(.9f,0,-.9f),
                new Vector3(-.9f,0,.9f),new Vector3(.9f,0,.9f) };
            _groundMesh.uv=new[] { Vector2.zero,Vector2.right,Vector2.up,Vector2.one };
            _groundMesh.triangles=new[] { 0,2,1,1,2,3 }; _groundMesh.RecalculateBounds();
            _groundMaterial=new Material(shader) { name="Sunder warm ground" };
            _groundMaterial.SetColor("_Color",new Color(1.6f,.095f,.003f));
            _groundMaterial.SetColor("_CoreColor",new Color(2.8f,1.9f,.9f));
            _groundMaterial.SetFloat("_TintStrength",.7f);
            var go=new GameObject("Sunder foot glow",typeof(MeshFilter),typeof(MeshRenderer));
            go.transform.SetParent(transform,false); go.transform.localPosition=Vector3.up*.035f;
            go.GetComponent<MeshFilter>().sharedMesh=_groundMesh;
            _ground=go.GetComponent<MeshRenderer>(); _ground.sharedMaterial=_groundMaterial;
            _ground.shadowCastingMode=ShadowCastingMode.Off; _ground.receiveShadows=false;
        }

        void CreateEmbers(Shader shader)
        {
            var go=new GameObject("Sunder embers"); go.transform.SetParent(transform,false);
            go.transform.localPosition=Vector3.up*.43f;
            _embers=go.AddComponent<ParticleSystem>();
            _embers.Stop(true,ParticleSystemStopBehavior.StopEmittingAndClear);
            _embers.useAutoRandomSeed=false; _embers.randomSeed=9024;
            var main=_embers.main; main.loop=false; main.playOnAwake=false; main.duration=.6f;
            main.startLifetime=new ParticleSystem.MinMaxCurve(.28f,.5f);
            main.startSpeed=new ParticleSystem.MinMaxCurve(.8f,1.8f);
            main.startSize=new ParticleSystem.MinMaxCurve(.045f,.085f);
            main.startColor=new Color(1,.42f,.035f); main.maxParticles=22;
            main.simulationSpace=ParticleSystemSimulationSpace.Local;
            main.scalingMode=ParticleSystemScalingMode.Hierarchy;
            var shape=_embers.shape; shape.shapeType=ParticleSystemShapeType.Circle;
            shape.radius=1.15f; shape.radiusThickness=.12f; shape.rotation=new Vector3(90,0,0);
            var emission=_embers.emission; emission.rateOverTime=0;
            emission.SetBursts(new[] { new ParticleSystem.Burst(0,(short)22) });
            var fade=_embers.colorOverLifetime; fade.enabled=true;
            var gradient=new Gradient();
            gradient.SetKeys(new[] { new GradientColorKey(Color.white,0),new GradientColorKey(new Color(1,.25f,.015f),1) },
                new[] { new GradientAlphaKey(1,0),new GradientAlphaKey(.9f,.3f),new GradientAlphaKey(0,1) });
            fade.color=gradient;
            var size=_embers.sizeOverLifetime; size.enabled=true;
            size.size=new ParticleSystem.MinMaxCurve(1,AnimationCurve.EaseInOut(0,1,1,.15f));
            _emberMaterial=new Material(shader) { name="Sunder ember light" };
            _emberMaterial.SetFloat("_Spark",1); _emberMaterial.SetFloat("_EmissionGain",5);
            _emberRenderer=go.GetComponent<ParticleSystemRenderer>(); _emberRenderer.sharedMaterial=_emberMaterial;
            _emberRenderer.renderMode=ParticleSystemRenderMode.Stretch;
            _emberRenderer.lengthScale=1.6f; _emberRenderer.velocityScale=.025f;
            _emberRenderer.shadowCastingMode=ShadowCastingMode.Off; _emberRenderer.receiveShadows=false;
        }

        public void Hide()
        {
            _core?.Hide(); _rim?.Hide(); _halo?.Hide(); _flame?.Hide();
            if (_ground!=null) _ground.enabled=false;
            if (_emberRenderer!=null) _emberRenderer.enabled=false;
            if (_embers!=null) _embers.Stop(false,ParticleSystemStopBehavior.StopEmittingAndClear);
        }
        void OnDisable() => Hide();
        void OnDestroy()
        {
            _core?.Dispose(); _rim?.Dispose(); _halo?.Dispose(); _flame?.Dispose();
            Release(_groundMesh); Release(_groundMaterial); Release(_emberMaterial);
        }
        static void Release(Object value)
        { if (value!=null) { if (Application.isPlaying) Destroy(value); else DestroyImmediate(value); } }

        // The imported carrier is shared and immutable; only instance materials are owned.
        sealed class Arc
        {
            readonly Material _material;
            readonly MeshRenderer _renderer;
            readonly MaterialPropertyBlock _block=new MaterialPropertyBlock();

            public Arc(Transform parent,string name,Mesh mesh,Shader shader,Texture mask,Texture flow,
                Vector4 rect,Color inner,Color outer,float emission,float scale,float width,float soft,int queue)
            {
                _material=new Material(shader) { name=name, renderQueue=queue };
                _material.SetTexture("_MaskTex",mask); _material.SetTexture("_FlowTex",flow);
                _material.SetVector("_MaskRect",rect);
                _material.SetColor("_InnerTint",inner); _material.SetColor("_OuterTint",outer);
                _material.SetFloat("_EmissionGain",emission); _material.SetFloat("_Width",width);
                _material.SetFloat("_Soft",soft);
                var go=new GameObject(name,typeof(MeshFilter),typeof(MeshRenderer));
                go.transform.SetParent(parent,false); go.transform.localScale=Vector3.one*scale;
                go.GetComponent<MeshFilter>().sharedMesh=mesh;
                _renderer=go.GetComponent<MeshRenderer>(); _renderer.sharedMaterial=_material;
                _renderer.shadowCastingMode=ShadowCastingMode.Off; _renderer.receiveShadows=false;
            }
            public void Sample(float opacity,float phase,float erosion)
            {
                _block.SetFloat("_Opacity",opacity); _block.SetFloat("_Phase",phase); _block.SetFloat("_Erosion",erosion);
                _renderer.SetPropertyBlock(_block); _renderer.enabled=opacity>0;
            }
            public void Hide() { if (_renderer!=null) _renderer.enabled=false; }
            public void Dispose() { Release(_material); }
        }
    }
}
