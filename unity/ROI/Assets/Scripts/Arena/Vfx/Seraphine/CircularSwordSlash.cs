using UnityEngine;
using UnityEngine.Rendering;

namespace Roi.Arena
{
    // A world-space sweep sampled over the existing attack's strike frames, not a new attack.
    public sealed class CircularSwordSlash : MonoBehaviour
    {
        const int Steps=96, Bands=2;
        readonly Vector3[] _vertices=new Vector3[(Steps+1)*Bands];
        readonly Color[] _colors=new Color[(Steps+1)*Bands];
        SpriteRenderer _body;
        Transform _anchor;
        Mesh _mesh;
        MeshRenderer _renderer;
        Material _material;
        MaterialPropertyBlock _properties;
        Ribbon _inset, _coreGlow, _glow, _details;
        ParticleSystem _sparks;
        ParticleSystemRenderer _sparkRenderer;
        Material _sparkMaterial;
        ParticleSystem _stars;
        ParticleSystemRenderer _starRenderer;
        Material _starMaterial;
        BuffAura _aura;
        Transform _auraRoot;
        Renderer[] _auraRenderers;
        Mesh _groundMesh;
        Material _groundMaterial;
        float _height;
        CircularSwordUvSweep _uvSweep;
        bool _useUvMotion;
        bool _fixedCarrier;

        // Live skills own a world-space anchor for one cast, never a moving actor parent.
        public bool FreezeAtSpawn()
        {
            if (!SetUvMotion(true)) return false;
            PlaceCarrier();
            _auraRoot.position=_anchor.position+Vector3.up*.055f;
            _fixedCarrier=true;
            return true;
        }

        void PlaceCarrier()
        {
            transform.SetPositionAndRotation(_anchor.position+Vector3.up*_height*.46f,
                Quaternion.Euler(0,_body.transform.eulerAngles.y,0));
            var scale=_anchor.lossyScale;
            transform.localScale=new Vector3((_body.flipX ? -1 : 1)*_height/Mathf.Max(.001f,Mathf.Abs(scale.x)),
                _height/Mathf.Max(.001f,Mathf.Abs(scale.y)),_height/Mathf.Max(.001f,Mathf.Abs(scale.z)));
        }

        public bool SetUvMotion(bool enabled)
        {
            if (enabled && _uvSweep==null)
            {
                var shader=Resources.Load<Shader>("Effects/SwordSlashUv");
                if (shader==null) return false;
                _uvSweep=new CircularSwordUvSweep(transform,shader,
                    Resources.Load<Texture2D>("Effects/SeraphineSlashMask-v1"),
                    Resources.Load<Texture2D>("Cutin/SeraphineAuraFlow-v1"));
            }
            _useUvMotion=enabled;
            _renderer.enabled=false; HideAccents();
            return true;
        }

        public static CircularSwordSlash Create(SpriteRenderer body,Transform anchor,float height)
        {
            var source=Resources.Load<Material>("Effects/SeraphineBasicSlash");
            var glowShader=Resources.Load<Shader>("Effects/CircularSlashGlow");
            var detailShader=Resources.Load<Shader>("Effects/SwordSlashDetail");
            var detailMask=Resources.Load<Texture2D>("Effects/SeraphineSlashMask-v1");
            var flow=Resources.Load<Texture2D>("Cutin/SeraphineAuraFlow-v1");
            var auraShader=Resources.Load<Shader>("Effects/BuffAura");
            var groundShader=Resources.Load<Shader>("Effects/BuffGroundGlow");
            var starShader=Resources.Load<Shader>("Effects/SwordStarSpark");
            if (body==null || anchor==null || source==null || glowShader==null || flow==null
                || auraShader==null || groundShader==null || detailShader==null || detailMask==null || starShader==null) return null;
            var go=new GameObject("Circular sword slash",typeof(MeshFilter),typeof(MeshRenderer));
            go.transform.SetParent(anchor,false); go.layer=body.gameObject.layer;
            var fx=go.AddComponent<CircularSwordSlash>();
            fx._body=body; fx._anchor=anchor; fx._height=Mathf.Max(.01f,height);
            fx._mesh=new Mesh { name="Single sharp sword curve" }; fx._mesh.MarkDynamic();
            fx._mesh.vertices=fx._vertices;
            var indices=new int[Steps*6];
            int at=0;
            for (int step=0;step<Steps;step++)
            {
                int k=step*Bands;
                indices[at++]=k; indices[at++]=k+Bands; indices[at++]=k+1;
                indices[at++]=k+1; indices[at++]=k+Bands; indices[at++]=k+Bands+1;
            }
            fx._mesh.triangles=indices;
            fx._mesh.bounds=new Bounds(Vector3.zero,new Vector3(3.4f,.1f,3.4f));
            go.GetComponent<MeshFilter>().sharedMesh=fx._mesh;
            fx._material=new Material(source) { name="Circular sword sharp edge" };
            fx._material.SetFloat("_EmissionGain",4.5f);
            fx._renderer=go.GetComponent<MeshRenderer>(); fx._renderer.sharedMaterial=fx._material;
            fx._renderer.shadowCastingMode=ShadowCastingMode.Off; fx._renderer.receiveShadows=false;
            fx._renderer.enabled=false; fx._properties=new MaterialPropertyBlock();
            fx._inset=new Ribbon(go.transform,"Inset blade",new Material(source));
            fx._inset.Material.SetFloat("_EmissionGain",3.5f);
            fx._coreGlow=new Ribbon(go.transform,"White blade halo",new Material(glowShader));
            fx._coreGlow.Material.SetFloat("_EmissionGain",2.3f);
            fx._glow=new Ribbon(go.transform,"Blue wake",new Material(glowShader));
            fx._glow.Material.SetTexture("_FlowTex",flow);
            fx._glow.Material.SetFloat("_EmissionGain",3);
            fx._details=new Ribbon(go.transform,"Textured blade streaks",new Material(detailShader));
            fx._details.Material.SetTexture("_MaskTex",detailMask);
            fx._details.Material.SetTexture("_FlowTex",flow);
            fx.CreateSparks(glowShader);
            fx.CreateStars(starShader);
            fx.CreateAura(auraShader,groundShader,flow);
            return fx;
        }

        public void Show(int attackFrame,float frameProgress,bool visible)
        {
            float pose=attackFrame+Mathf.Clamp01(frameProgress);
            visible &= isActiveAndEnabled && _body!=null && _body.enabled && _anchor!=null && pose>=2 && pose<7;
            _renderer.enabled=visible && !_useUvMotion;
            if (!visible) { HideAccents(); return; }
            float phase=Mathf.Clamp01((pose-2)/3f);
            float sweep=Mathf.SmoothStep(0,1,phase)*360;
            float arc=330*Mathf.SmoothStep(0,1,Mathf.Clamp01(phase/.4f));
            if (!_useUvMotion) for (int step=0;step<=Steps;step++)
            {
                float u=step/(float)Steps;
                float angle=(180+sweep-arc*(1-u))*Mathf.Deg2Rad;
                float width=.16f*Taper(u);
                for (int band=0;band<Bands;band++)
                {
                    int k=step*Bands+band;
                    float radius=1.35f+(band==0 ? -width : width);
                    _vertices[k]=new Vector3(Mathf.Cos(angle)*radius,0,Mathf.Sin(angle)*radius);
                    _colors[k]=band==0 ? new Color(.7f,.94f,1) : Color.white;
                }
            }
            if (!_useUvMotion) { _mesh.vertices=_vertices; _mesh.colors=_colors; }
            if (!_fixedCarrier) PlaceCarrier();
            // Movement ends with the strike, but its light decays through both recovery frames.
            // Squaring the tail lets the HDR-white core leave saturation before the last frame.
            float tail=1-Mathf.SmoothStep(0,1,Mathf.InverseLerp(4,7,pose));
            float opacity=Mathf.SmoothStep(0,1,Mathf.Clamp01(phase/.08f))*tail*tail;
            opacity*=_body.color.a;
            _properties.SetFloat("_Opacity",opacity); _renderer.SetPropertyBlock(_properties);
            if (_useUvMotion) _uvSweep.Sample(sweep,arc/330,opacity,phase);
            else
            {
                _inset.Sample(sweep-16,arc*.91f,1.07f,.045f,new Color(.45f,.92f,1),opacity,phase);
                _coreGlow.Sample(sweep,arc,1.35f,.25f,new Color(.8f,.93f,1),opacity*.5f,phase);
                _glow.Sample(sweep-9,arc,1.6f,.52f,new Color(.005f,.18f,1),opacity*.85f,phase);
                _details.Sample(sweep-12,arc*.95f,1.48f,.4f,new Color(.83f,.96f,1),opacity*.8f,phase,
                    Mathf.SmoothStep(0,1.05f,Mathf.InverseLerp(.52f,.98f,phase)),false);
            }
            if (!_fixedCarrier) _auraRoot.position=_anchor.position+Vector3.up*.055f;
            _aura.Sample(phase,opacity*.7f);
            _groundMaterial.SetFloat("_Opacity",opacity*.9f);
            foreach (var r in _auraRenderers) r.enabled=true;
            // Resampling the native system keeps editor scrubbing and interrupted strikes repeatable.
            _sparkMaterial.SetFloat("_Opacity",opacity);
            _sparkRenderer.enabled=true;
            _sparks.Simulate(phase*.4f,false,true,true);
            _starMaterial.SetFloat("_Opacity",opacity);
            _starRenderer.enabled=true;
            // Continue drifting during recovery rather than freezing at the end of the sweep.
            _stars.Simulate((pose-2)*.4f/3,false,true,true);
        }

        static float Taper(float u) => Mathf.Pow(Mathf.Max(0,Mathf.Sin(u*Mathf.PI)),.8f)*Mathf.Lerp(.25f,1,u);

        void CreateAura(Shader auraShader,Shader groundShader,Texture flow)
        {
            _auraRoot=new GameObject("Sword aura at feet").transform;
            _auraRoot.SetParent(transform,false);
            _aura=new BuffAura(_auraRoot,auraShader,flow,new Color(.008f,.2f,1.8f),.95f,.06f);
            _groundMaterial=new Material(groundShader) { name="Sword foot glow" };
            _groundMaterial.SetColor("_Color",new Color(.005f,.11f,1.7f));
            _groundMaterial.SetFloat("_TintStrength",.65f);
            _groundMaterial.SetColor("_CoreColor",new Color(3,3.4f,3.8f,1));
            _groundMesh=new Mesh { name="Sword foot glow plane" };
            _groundMesh.vertices=new[] { new Vector3(-1.1f,0,-1.1f),new Vector3(1.1f,0,-1.1f),
                new Vector3(-1.1f,0,1.1f),new Vector3(1.1f,0,1.1f) };
            _groundMesh.uv=new[] { Vector2.zero,Vector2.right,Vector2.up,Vector2.one };
            _groundMesh.triangles=new[] { 0,2,1,1,2,3 }; _groundMesh.RecalculateBounds();
            var ground=new GameObject("Sword foot glow",typeof(MeshFilter),typeof(MeshRenderer));
            ground.transform.SetParent(_auraRoot,false);
            ground.GetComponent<MeshFilter>().sharedMesh=_groundMesh;
            var renderer=ground.GetComponent<MeshRenderer>(); renderer.sharedMaterial=_groundMaterial;
            renderer.shadowCastingMode=ShadowCastingMode.Off; renderer.receiveShadows=false;
            foreach (var t in _auraRoot.GetComponentsInChildren<Transform>(true)) t.gameObject.layer=gameObject.layer;
            _auraRenderers=_auraRoot.GetComponentsInChildren<Renderer>(true);
            foreach (var r in _auraRenderers) r.enabled=false;
        }

        void CreateSparks(Shader shader)
        {
            var go=new GameObject("Sword sparks"); go.layer=gameObject.layer;
            go.transform.SetParent(transform,false);
            _sparks=go.AddComponent<ParticleSystem>();
            _sparks.Stop(true,ParticleSystemStopBehavior.StopEmittingAndClear);
            _sparks.useAutoRandomSeed=false; _sparks.randomSeed=731;
            var main=_sparks.main;
            main.loop=false; main.playOnAwake=false; main.duration=.4f;
            main.startLifetime=new ParticleSystem.MinMaxCurve(.18f,.3f);
            main.startSpeed=new ParticleSystem.MinMaxCurve(.9f,2.2f);
            main.startSize=new ParticleSystem.MinMaxCurve(.025f,.055f);
            main.startColor=new Color(.45f,.85f,1);
            main.maxParticles=18; main.simulationSpace=ParticleSystemSimulationSpace.Local;
            main.scalingMode=ParticleSystemScalingMode.Hierarchy;
            var shape=_sparks.shape; shape.shapeType=ParticleSystemShapeType.Sphere;
            shape.radius=1.25f; shape.radiusThickness=.08f; shape.scale=new Vector3(1,.12f,1);
            var emission=_sparks.emission; emission.rateOverTime=0;
            emission.SetBursts(new[] { new ParticleSystem.Burst(.065f,(short)18) });
            var fade=_sparks.colorOverLifetime; fade.enabled=true;
            var gradient=new Gradient();
            gradient.SetKeys(new[] { new GradientColorKey(Color.white,0),new GradientColorKey(Color.white,1) },
                new[] { new GradientAlphaKey(1,0),new GradientAlphaKey(1,.25f),new GradientAlphaKey(0,1) });
            fade.color=gradient;
            var shrink=_sparks.sizeOverLifetime; shrink.enabled=true;
            shrink.size=new ParticleSystem.MinMaxCurve(1,AnimationCurve.EaseInOut(0,1,1,.1f));
            _sparkMaterial=new Material(shader) { name="Circular slash sparks" };
            _sparkMaterial.SetFloat("_Spark",1); _sparkMaterial.SetFloat("_EmissionGain",5);
            _sparkRenderer=go.GetComponent<ParticleSystemRenderer>();
            _sparkRenderer.sharedMaterial=_sparkMaterial;
            _sparkRenderer.renderMode=ParticleSystemRenderMode.Billboard;
            _sparkRenderer.shadowCastingMode=ShadowCastingMode.Off;
            _sparkRenderer.receiveShadows=false; _sparkRenderer.enabled=false;
        }

        void CreateStars(Shader shader)
        {
            var go=new GameObject("Sword radial stars"); go.layer=gameObject.layer;
            go.transform.SetParent(transform,false); _stars=go.AddComponent<ParticleSystem>();
            _stars.Stop(true,ParticleSystemStopBehavior.StopEmittingAndClear);
            _stars.useAutoRandomSeed=false; _stars.randomSeed=1947;
            var main=_stars.main; main.loop=false; main.playOnAwake=false; main.duration=.6f;
            main.startLifetime=new ParticleSystem.MinMaxCurve(.28f,.42f);
            main.startSpeed=new ParticleSystem.MinMaxCurve(1.1f,2);
            main.startSize=new ParticleSystem.MinMaxCurve(.18f,.26f);
            main.startRotation=new ParticleSystem.MinMaxCurve(-.2f,.2f);
            main.startColor=new Color(.005f,.2f,1); main.maxParticles=18;
            main.simulationSpace=ParticleSystemSimulationSpace.Local;
            main.scalingMode=ParticleSystemScalingMode.Hierarchy;
            var shape=_stars.shape; shape.shapeType=ParticleSystemShapeType.Circle;
            shape.radius=1.4f; shape.radiusThickness=0; shape.rotation=new Vector3(90,0,0);
            shape.arc=360; shape.arcMode=ParticleSystemShapeMultiModeValue.BurstSpread;
            var emission=_stars.emission; emission.rateOverTime=0;
            emission.SetBursts(new[] { new ParticleSystem.Burst(.08f,(short)18) });
            var fade=_stars.colorOverLifetime; fade.enabled=true;
            var gradient=new Gradient();
            gradient.SetKeys(new[] { new GradientColorKey(Color.white,0),new GradientColorKey(Color.white,1) },
                new[] { new GradientAlphaKey(0,0),new GradientAlphaKey(1,.12f),new GradientAlphaKey(.85f,.45f),new GradientAlphaKey(0,1) });
            fade.color=gradient;
            var size=_stars.sizeOverLifetime; size.enabled=true;
            size.size=new ParticleSystem.MinMaxCurve(1,AnimationCurve.EaseInOut(0,1,1,.25f));
            _starMaterial=new Material(shader) { name="Four point sword stars" };
            _starRenderer=go.GetComponent<ParticleSystemRenderer>(); _starRenderer.sharedMaterial=_starMaterial;
            _starRenderer.renderMode=ParticleSystemRenderMode.Billboard;
            _starRenderer.shadowCastingMode=ShadowCastingMode.Off; _starRenderer.receiveShadows=false;
            _starRenderer.enabled=false;
        }

        void HideAccents()
        {
            _uvSweep?.Hide();
            if (_inset!=null) _inset.Renderer.enabled=false;
            if (_coreGlow!=null) _coreGlow.Renderer.enabled=false;
            if (_glow!=null) _glow.Renderer.enabled=false;
            if (_details!=null) _details.Renderer.enabled=false;
            if (_sparks!=null) _sparks.Stop(false,ParticleSystemStopBehavior.StopEmittingAndClear);
            if (_sparkRenderer!=null) _sparkRenderer.enabled=false;
            if (_stars!=null) _stars.Stop(false,ParticleSystemStopBehavior.StopEmittingAndClear);
            if (_starRenderer!=null) _starRenderer.enabled=false;
            if (_auraRenderers!=null) foreach (var r in _auraRenderers) if (r!=null) r.enabled=false;
        }

        void OnDisable() { if (_renderer!=null) _renderer.enabled=false; HideAccents(); }
        void OnDestroy()
        {
            Release(_mesh); Release(_material); Release(_sparkMaterial);
            Release(_starMaterial);
            _aura?.Dispose(); Release(_groundMesh); Release(_groundMaterial);
            _inset?.Dispose(); _coreGlow?.Dispose(); _glow?.Dispose(); _details?.Dispose();
            _uvSweep?.Dispose();
        }

        static void Release(Object asset)
        { if (asset!=null) { if (Application.isPlaying) Destroy(asset); else DestroyImmediate(asset); } }

        sealed class Ribbon
        {
            public readonly MeshRenderer Renderer;
            public readonly Material Material;
            readonly Mesh _mesh;
            readonly Vector3[] _points=new Vector3[(Steps+1)*Bands];
            readonly Color[] _tints=new Color[(Steps+1)*Bands];
            readonly MaterialPropertyBlock _block=new MaterialPropertyBlock();

            public Ribbon(Transform parent,string name,Material material)
            {
                Material=material; Material.name=name;
                var go=new GameObject(name,typeof(MeshFilter),typeof(MeshRenderer));
                go.layer=parent.gameObject.layer; go.transform.SetParent(parent,false);
                _mesh=new Mesh { name=name }; _mesh.MarkDynamic(); _mesh.vertices=_points;
                var uv=new Vector2[_points.Length]; var indices=new int[Steps*6];
                for (int s=0;s<=Steps;s++)
                {
                    uv[s*2]=new Vector2(s/(float)Steps,0); uv[s*2+1]=new Vector2(s/(float)Steps,1);
                    if (s==Steps) continue;
                    int k=s*2,at=s*6;
                    indices[at]=k; indices[at+1]=k+2; indices[at+2]=k+1;
                    indices[at+3]=k+1; indices[at+4]=k+2; indices[at+5]=k+3;
                }
                _mesh.uv=uv; _mesh.triangles=indices;
                _mesh.bounds=new Bounds(Vector3.zero,new Vector3(4.4f,.1f,4.4f));
                go.GetComponent<MeshFilter>().sharedMesh=_mesh;
                Renderer=go.GetComponent<MeshRenderer>(); Renderer.sharedMaterial=Material;
                Renderer.shadowCastingMode=ShadowCastingMode.Off; Renderer.receiveShadows=false;
                Renderer.enabled=false;
            }

            public void Sample(float sweep,float arc,float radius,float width,Color tint,float opacity,float phase,float erosion=0,bool taper=true)
            {
                for (int s=0;s<=Steps;s++)
                {
                    float u=s/(float)Steps,angle=(180+sweep-arc*(1-u))*Mathf.Deg2Rad;
                    for (int band=0;band<Bands;band++)
                    {
                        // A painted mask already tapers its tips; do not pinch them a second time.
                        float r=radius+(band==0 ? -1 : 1)*width*(taper ? Taper(u) : 1);
                        _points[s*2+band]=new Vector3(Mathf.Cos(angle)*r,.002f,Mathf.Sin(angle)*r);
                        _tints[s*2+band]=tint;
                    }
                }
                _mesh.vertices=_points; _mesh.colors=_tints;
                _block.SetFloat("_Opacity",opacity); _block.SetFloat("_Phase",phase);
                _block.SetFloat("_Erosion",erosion);
                Renderer.SetPropertyBlock(_block); Renderer.enabled=true;
            }

            public void Dispose() { Release(_mesh); Release(Material); }
        }
    }
}
