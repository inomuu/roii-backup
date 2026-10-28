// THE ROUND GLOW ON A HIT — `arena3d_fx.gd::_sync_sparks`, ported.
//
// WHY IT HAS TO BE ITS OWN OBJECT, which is the whole reason this file exists: bloom spreads the
// SHAPE that is already on screen. A slime sprite flashed white blooms in the shape of a slime, and
// no amount of scatter turns that into a halo. A round glow needs something round emitting it.
//
// One pooled quad per live hit: camera-facing, additive, unlit, on a radial falloff. It EXPANDS as
// it FADES — Godot's `d = 2 * (0.7 + 0.7k)` against `alpha = (1 - k) * 0.9` — which is what reads
// as a burst rather than a sticker. The texture is generated here rather than shipped: it is a
// gradient, and an asset would be one more thing to keep in step with the colour it is tinted by.
using System.Collections.Generic;
using UnityEngine;

namespace Roi.Arena
{
    public sealed class ArenaSparks : MonoBehaviour
    {
        public void DelayPresentation(float seconds)
        {
            for (int i=0;i<_live.Count;i++) { var spark=_live[i]; spark.T0+=seconds; _live[i]=spark; }
        }
        /// Godot's `Color(1.0, 0.90, 0.62, …)` — warm, not white, so it reads as a strike rather
        /// than as more of the flash it sits on top of.
        static readonly Color Warm = new Color(1f, 0.90f, 0.62f, 1f);
        /// How long one spark lives. Matches the flash it accompanies.
        public float Dur = 0.3f;
        /// Multiplies the additive colour. Past 1 so the spark itself crosses the bloom threshold
        /// and blooms — the halo is the point, and an LDR sprite cannot make one.
        public float Gain = 3f;

        struct Spark { public Vector3 Pos; public float T0; }

        readonly List<Spark> _live = new();
        readonly List<MeshRenderer> _pool = new();
        Material _mat;
        Camera _cam;
        ParticleSystem _motes;

        /// How many motes a hit throws. Small: they are seasoning on the flash and the halo, and a
        /// cloud of them turns a strike into a firework.
        public int MoteCount = 20;

        public static ArenaSparks Create(Transform parent)
        {
            var go = new GameObject("ArenaSparks");
            go.transform.SetParent(parent, false);
            return go.AddComponent<ArenaSparks>();
        }

        public void Bind(Camera cam)
        {
            _cam = cam;
            _motes ??= BuildMotes();
        }

        /// `away` is the horizontal direction from the attacker to the thing it hit — the way the
        /// debris should go. Zero falls back to straight up, which is what a hit with no known
        /// attacker (a trap, a field) should throw.
        public void Spawn(Vector3 at, Vector3 away)
        {
            _live.Add(new Spark { Pos = at, T0 = Time.time });
            EmitMotes(at, away);
        }

        /// A BURST OF MOTES, thrown outward and pulled down.
        ///
        /// Unity's own ParticleSystem rather than a hand-rolled pool: this is exactly what it is
        /// for, it batches on the GPU, and the project's rule is to reach for the engine's system
        /// before porting one (`unity-native-before-porting`). Nothing crosses from Godot here —
        /// its hit has a flash, a halo and a squash, and no particles at all; this is new.
        void EmitMotes(Vector3 at, Vector3 away)
        {
            if (_motes == null) return;
            // AIM THE CONE AWAY FROM THE SWING, tilted up. Straight up threw the same fan whichever
            // side the blow came from, which reads as the monster erupting rather than as something
            // being knocked off it. `away + up` is a 45-degree lift off the horizontal — enough to
            // arc, not so much that gravity is the only thing the eye follows.
            away.y = 0f;
            var dir = away.sqrMagnitude > 1e-4f ? (away.normalized + Vector3.up).normalized : Vector3.up;
            var sh = _motes.shape;
            sh.rotation = Quaternion.LookRotation(dir, Vector3.up).eulerAngles;
            var p = new ParticleSystem.EmitParams
            {
                position = at,
                applyShapeToPosition = true,
                // Past 1 so the motes cross the bloom threshold and glow like the halo they came
                // out of. An LDR mote is a grey speck.
                startColor = Warm * Gain,
            };
            _motes.Emit(p, MoteCount);
        }

        /// Built in code, like every other piece of arena chrome — the scene carries no prefab for
        /// it, so there is nothing to fall out of step with this file.
        ParticleSystem BuildMotes()
        {
            var go = new GameObject("motes");
            go.transform.SetParent(transform, false);
            var ps = go.AddComponent<ParticleSystem>();
            ps.Stop();

            var main = ps.main;
            main.duration = 1f;
            main.loop = false;
            main.playOnAwake = false;
            main.startLifetime = new ParticleSystem.MinMaxCurve(0.22f, 0.45f);
            main.startSpeed = new ParticleSystem.MinMaxCurve(2f, 4.5f);
            main.startSize = new ParticleSystem.MinMaxCurve(0.30f, 0.70f);
            // Pulled down hard enough to arc inside their own lifetime — motes that fly straight
            // read as a starburst decal rather than as debris off a hit. Raised with the speeds cut
            // because the first pass threw them well over the monster's head (user 2026-09-09):
            // debris off a sword should barely clear the thing it came off.
            main.gravityModifier = 2.2f;
            main.maxParticles = 400;
            main.simulationSpace = ParticleSystemSimulationSpace.World;
            main.scalingMode = ParticleSystemScalingMode.Hierarchy;

            var em = ps.emission;
            em.enabled = false;              // emitted by hand, one burst per hit

            // A CONE AIMED UP, not a sphere: a sphere throws as many motes into the ground as into
            // the air, and the half that goes down is never seen. Aimed up they arc, which is the
            // shape a struck thing throws debris in.
            var sh = ps.shape;
            sh.enabled = true;
            sh.shapeType = ParticleSystemShapeType.Cone;
            sh.angle = 42f;
            sh.radius = 0.25f;
            // Aimed per burst in EmitMotes — away from whoever swung. The value here is only a
            // sane default for a burst with no direction.
            sh.rotation = new Vector3(-90f, 0f, 0f);

            // Fade out rather than vanish, for the same reason the flash needed a curve.
            var col = ps.colorOverLifetime;
            col.enabled = true;
            col.color = new ParticleSystem.MinMaxGradient(new Gradient
            {
                colorKeys = new[] { new GradientColorKey(Color.white, 0f), new GradientColorKey(Color.white, 1f) },
                alphaKeys = new[] { new GradientAlphaKey(1f, 0f), new GradientAlphaKey(1f, 0.45f),
                                    new GradientAlphaKey(0f, 1f) },
            });
            var siz = ps.sizeOverLifetime;
            siz.enabled = true;
            siz.size = new ParticleSystem.MinMaxCurve(1f, AnimationCurve.EaseInOut(0f, 1f, 1f, 0.25f));

            var r = ps.GetComponent<ParticleSystemRenderer>();
            r.renderMode = ParticleSystemRenderMode.Billboard;
            // ITS OWN SHADER, and this is why the motes were invisible on the first try:
            // `URP/Unlit` has no per-particle colour. Every mote rendered with the MATERIAL's
            // `_BaseColor`, which the halo sets through a property block that particles never see —
            // so `startColor` was thrown away and they drew as nothing. `URP/Particles/Unlit`
            // multiplies by the particle's own colour, which is the whole point of setting one.
            r.sharedMaterial = BuildMoteMat();
            r.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            r.receiveShadows = false;
            ps.Play();
            return ps;
        }

        void LateUpdate()
        {
            for (int i = _live.Count - 1; i >= 0; i--)
                if (Time.time - _live[i].T0 >= Dur) _live.RemoveAt(i);

            while (_pool.Count < _live.Count) _pool.Add(NewQuad());
            for (int i = 0; i < _pool.Count; i++) _pool[i].enabled = i < _live.Count;
            if (_cam == null || _live.Count == 0) return;

            for (int i = 0; i < _live.Count; i++)
            {
                float k = Mathf.Clamp01((Time.time - _live[i].T0) / Mathf.Max(0.05f, Dur));
                var t = _pool[i].transform;
                t.position = _live[i].Pos;
                // Face the camera. The quad's own +Z is its face, so look ALONG the camera's
                // forward rather than at the camera — at this pitch the two differ visibly.
                t.rotation = Quaternion.LookRotation(_cam.transform.forward, _cam.transform.up);
                float d = 2f * (0.7f + 0.7f * k);
                t.localScale = new Vector3(d, d, 1f);

                var mpb = new MaterialPropertyBlock();
                _pool[i].GetPropertyBlock(mpb);
                var c = Warm * Gain * ((1f - k) * 0.9f);
                mpb.SetColor("_BaseColor", new Color(c.r, c.g, c.b, 1f));
                _pool[i].SetPropertyBlock(mpb);
            }
        }

        MeshRenderer NewQuad()
        {
            var q = GameObject.CreatePrimitive(PrimitiveType.Quad);
            q.name = "spark";
            q.transform.SetParent(transform, false);
            var col = q.GetComponent<Collider>();
            if (col != null) Destroy(col);
            var r = q.GetComponent<MeshRenderer>();
            r.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            r.receiveShadows = false;
            r.sharedMaterial = _mat ??= BuildMat();
            return r;
        }

        /// The motes' material: same additive idea as the halo, on the shader that reads a
        /// particle's own colour.
        Material BuildMoteMat()
        {
            // Resource templates keep dynamically used shaders/variants in standalone builds.
            var template = Resources.Load<Material>("Effects/RuntimeSparkSoft");
            var sh = template != null ? template.shader : Shader.Find("Universal Render Pipeline/Particles/Unlit");
            if (sh == null) return _mat ??= BuildMat();   // fall back rather than draw nothing
            var m = new Material(sh) { name = "MoteAdditive" };
            SetAdditive(m);
            m.SetTexture("_BaseMap", RadialTex());
            m.SetColor("_BaseColor", Color.white);
            return m;
        }

        /// Unlit + additive: the glow ADDS to what is behind it, so it never darkens the ground it
        /// sits on. `Blend One One` with ZWrite off is the whole of that.
        Material BuildMat()
        {
            var template = Resources.Load<Material>("Effects/RuntimeSparkAdditive");
            var sh = template != null ? template.shader : Shader.Find("Universal Render Pipeline/Unlit");
            var m = new Material(sh) { name = "SparkAdditive" };
            SetAdditive(m);
            m.SetTexture("_BaseMap", RadialTex());
            return m;
        }

        /// Force `Blend One One` on a URP Unlit / Particles-Unlit material and make every property
        /// URP consults AGREE with it. `_Blend` is URP's own blend-mode enum (0 Alpha, 1 Premultiply,
        /// 2 Additive, 3 Multiply): this used to say 1 — PREMULTIPLY — while the blend factors said
        /// additive, and whichever pass Unity re-validated the material on (Unity 6 does, on load)
        /// put the factors back to `One OneMinusSrcAlpha`. With a texture whose alpha is 1 everywhere
        /// that is `src + 0`: the whole quad painted black around a bright disc (2026-09-18 report).
        /// Setting the alpha factors as well leaves nothing for a validator to disagree with.
        static void SetAdditive(Material m)
        {
            m.SetFloat("_Surface", 1f);              // transparent
            m.SetFloat("_Blend", 2f);                // ADDITIVE, in URP's enum
            m.SetFloat("_AlphaClip", 0f);
            m.SetFloat("_SrcBlend", (float)UnityEngine.Rendering.BlendMode.One);
            m.SetFloat("_DstBlend", (float)UnityEngine.Rendering.BlendMode.One);
            m.SetFloat("_SrcBlendAlpha", (float)UnityEngine.Rendering.BlendMode.One);
            m.SetFloat("_DstBlendAlpha", (float)UnityEngine.Rendering.BlendMode.One);
            m.SetFloat("_ZWrite", 0f);
            m.SetOverrideTag("RenderType", "Transparent");
            m.renderQueue = (int)UnityEngine.Rendering.RenderQueue.Transparent;
            m.EnableKeyword("_SURFACE_TYPE_TRANSPARENT");
            m.DisableKeyword("_ALPHAPREMULTIPLY_ON");
            m.DisableKeyword("_ALPHAMODULATE_ON");
        }

        /// A soft disc, with the falloff in RGB and alpha left at 1.
        ///
        /// THAT IS NOT A DETAIL. Additive blending is `One One`: it adds the source COLOUR and
        /// never looks at alpha. A texture that is white everywhere with the disc shape in its
        /// alpha channel therefore adds full white across the whole quad — which is exactly what a
        /// square of light looks like, and is what this drew on the first attempt.
        ///
        /// Squared falloff rather than linear: a linear ramp reaches zero at a visible edge, and an
        /// edge is the one thing a glow must not have.
        static Texture2D RadialTex()
        {
            const int N = 128;
            var tex = new Texture2D(N, N, TextureFormat.RGBA32, false) { wrapMode = TextureWrapMode.Clamp };
            var px = new Color[N * N];
            for (int y = 0; y < N; y++)
                for (int x = 0; x < N; x++)
                {
                    float dx = (x + 0.5f) / N - 0.5f, dy = (y + 0.5f) / N - 0.5f;
                    float r = Mathf.Sqrt(dx * dx + dy * dy) * 2f;          // 0 centre, 1 edge
                    float a = Mathf.Clamp01(1f - r);
                    a *= a;
                    px[y * N + x] = new Color(a, a, a, 1f);
                }
            tex.SetPixels(px);
            tex.Apply();
            return tex;
        }
    }
}
