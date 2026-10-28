// THE BREAK SHATTER — the glass a guard breaks into.
//
// The sim has had the event all along (`recent_hits` with `kind == "break"`, once a monster's
// toughness gauge empties); the client answered it with a number and nothing else. A break is the
// loudest thing that happens in a fight and it read quieter than an ordinary swing.
//
// Shards are their own system rather than more of `ArenaSparks` because they are the opposite
// object. A spark is a round additive glow that exists to be blurred — it has no edge and bloom is
// the point. A shard has to keep a HARD edge to read as glass, so it is alpha-blended with a solid
// silhouette, and only its colour is pushed past 1 so the bloom catches the highlight without
// eating the shape. Additive shards turn into a white cloud at the exact moment the effect is
// supposed to be legible.
using UnityEngine;

namespace Roi.Arena
{
    public sealed class ArenaShards : MonoBehaviour
    {
        /// Cold and pale — Octopath's break is glass and frost, not the warm spark of a hit. Kept
        /// distinct from `ArenaSparks.Warm` on purpose: the two land in the same frame and the
        /// break has to be the one that reads.
        static readonly Color Ice = new Color(0.78f, 0.92f, 1f, 1f);

        /// Past 1 so the shards cross the bloom threshold. Lower than the spark's gain because a
        /// shard is a large solid area and the same brightness on it is much more light.
        public float Gain = 2.2f;
        /// How many pieces one break throws.
        public int ShardCount = 12;

        ParticleSystem _ps;
        Material _mat;
        Mesh[] _meshes;

        public static ArenaShards Create(Transform parent)
        {
            var go = new GameObject("ArenaShards");
            go.transform.SetParent(parent, false);
            return go.AddComponent<ArenaShards>();
        }

        public void Bind() => _ps ??= Build();

        void OnDestroy()
        {
            if (_mat != null) Destroy(_mat);
            if (_meshes != null) foreach (var mesh in _meshes) if (mesh != null) Destroy(mesh);
        }

        /// Play the glass back slower. Used for the boss beat: the shards are the thing worth
        /// looking at and at full speed they are gone before the camera has finished arriving.
        /// This is the PARTICLE system's own clock, not `Time.timeScale` — the arena, the HUD and
        /// the camera all keep running at their own pace.
        public void SetSpeed(float s)
        {
            if (_ps == null) return;
            var main = _ps.main;
            main.simulationSpeed = s;
        }

        /// Shatter at `at`. The burst is symmetrical: a break is the target's own guard failing, not
        /// something the attacker threw, so unlike a hit's debris it has no direction to lean on.
        public void Burst(Vector3 at)
        {
            if (_ps == null) return;
            var camera = GetComponentInParent<ArenaView>().GetComponentInChildren<Camera>();
            var rotation = camera != null ? camera.transform.rotation : Quaternion.identity;
            var right = rotation * Vector3.right;
            var up = rotation * Vector3.up;
            for (int i = 0; i < ShardCount; i++)
            {
                float angle = Random.Range(0f, Mathf.PI * 2f);
                var direction = right * Mathf.Cos(angle) + up * Mathf.Sin(angle);
                float size = i < 2 ? Random.Range(.18f, .28f) : Random.Range(.05f, .13f);
                var p = new ParticleSystem.EmitParams
                {
                    position = at + direction * Random.Range(.12f, .4f),
                    velocity = direction * Random.Range(1.5f, 3.8f) + up * 1.1f,
                    rotation3D = (rotation * Quaternion.Euler(Random.Range(-50f, 50f), Random.Range(-55f, 55f), Random.Range(0f, 360f))).eulerAngles,
                    startSize3D = new Vector3(size, size * Random.Range(.8f, 1.5f), size),
                    startColor = new Color(Ice.r * Gain, Ice.g * Gain, Ice.b * Gain, .92f),
                };
                _ps.Emit(p, 1);
            }
        }

        ParticleSystem Build()
        {
            var go = new GameObject("Shards");
            go.transform.SetParent(transform, false);
            var ps = go.AddComponent<ParticleSystem>();

            var main = ps.main;
            main.useUnscaledTime = true;  // same presentation clock as BREAK title and focus
            main.startLifetime = new ParticleSystem.MinMaxCurve(1.05f, 1.5f);
            main.startSpeed = new ParticleSystem.MinMaxCurve(3.5f, 8f);
            // NON-UNIFORM on purpose: one texture plus a random aspect per particle is what makes
            // twenty copies of the same sliver read as twenty different pieces of glass.
            main.startSize3D = true;
            main.startSizeX = new ParticleSystem.MinMaxCurve(0.10f, 0.24f);
            main.startSizeY = new ParticleSystem.MinMaxCurve(0.30f, 0.75f);
            main.startSizeZ = new ParticleSystem.MinMaxCurve(1f);
            main.startRotation3D = true;
            main.startRotationZ = new ParticleSystem.MinMaxCurve(0f, Mathf.PI * 2f);
            main.gravityModifier = .3f;
            main.simulationSpace = ParticleSystemSimulationSpace.World;
            main.maxParticles = 400;
            main.playOnAwake = false;

            var em = ps.emission;
            em.enabled = false;                 // hand-emitted, one burst per break

            var sh = ps.shape;
            sh.enabled = true;
            sh.shapeType = ParticleSystemShapeType.Sphere;
            sh.radius = 0.35f;

            // Tumbling. Glass that holds one angle all the way down reads as a paper cut-out.
            var rot = ps.rotationOverLifetime;
            rot.enabled = true;
            rot.separateAxes = false;
            rot.z = new ParticleSystem.MinMaxCurve(-9f, 9f);   // radians/sec

            // Air. The pieces leave fast and lose it, so the arc peaks early and the fall is what
            // the eye follows — the same shape as real debris.
            var lim = ps.limitVelocityOverLifetime;
            lim.enabled = true;
            lim.dampen = 0.12f;
            lim.limit = new ParticleSystem.MinMaxCurve(2.2f);

            // Hold full brightness while the break is being read, then go entirely. The alpha ramp
            // is the whole fade: dimming the colour instead would grey the glass before it left.
            var col = ps.colorOverLifetime;
            col.enabled = true;
            var grad = new Gradient();
            grad.SetKeys(
                new[] { new GradientColorKey(Color.white, 0f), new GradientColorKey(Color.white, 1f) },
                new[] { new GradientAlphaKey(1f, 0f), new GradientAlphaKey(1f, 0.55f),
                        new GradientAlphaKey(0f, 1f) });
            col.color = new ParticleSystem.MinMaxGradient(grad);

            var r = ps.GetComponent<ParticleSystemRenderer>();
            r.renderMode = ParticleSystemRenderMode.Mesh;
            _meshes = new[] { GlassMesh(0), GlassMesh(1), GlassMesh(2) };
            r.SetMeshes(_meshes);
            r.alignment = ParticleSystemRenderSpace.View;
            r.sharedMaterial = _mat ??= BuildMat();
            r.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            r.receiveShadows = false;
            ps.Play();
            return ps;
        }

        /// Alpha-blended, not additive — see the header. `URP/Particles/Unlit` is the one that reads
        /// a particle's own colour; plain `URP/Unlit` ignores it and every shard comes out white.
        Material BuildMat()
        {
            var sh = Shader.Find("Universal Render Pipeline/Particles/Unlit");
            if (sh == null) sh = Shader.Find("Universal Render Pipeline/Unlit");
            var m = new Material(sh) { name = "ShardGlass" };
            m.SetFloat("_Surface", 1f);      // transparent
            m.SetFloat("_Blend", 0f);        // alpha, so the silhouette survives
            m.SetInt("_SrcBlend", (int)UnityEngine.Rendering.BlendMode.SrcAlpha);
            m.SetInt("_DstBlend", (int)UnityEngine.Rendering.BlendMode.OneMinusSrcAlpha);
            m.SetInt("_ZWrite", 0);
            m.renderQueue = (int)UnityEngine.Rendering.RenderQueue.Transparent;
            m.EnableKeyword("_SURFACE_TYPE_TRANSPARENT");
            m.SetTexture("_BaseMap", Texture2D.whiteTexture);
            m.SetFloat("_Cull", 0f);
            m.SetColor("_BaseColor", Color.white);
            return m;
        }

        // Broad asymmetric pieces, triangulated into coloured facets with a bright rim.
        // Mesh silhouettes stay crisp; no texture atlas or image generation is required.
        static Mesh GlassMesh(int variant)
        {
            Vector2[][] outlines = {
                new[] { new Vector2(-.5f,-.28f), new Vector2(.16f,-.5f), new Vector2(.48f,.18f), new Vector2(-.12f,.5f) },
                new[] { new Vector2(-.45f,-.4f), new Vector2(.5f,-.08f), new Vector2(.14f,.48f) },
                new[] { new Vector2(-.5f,-.16f), new Vector2(-.16f,-.5f), new Vector2(.4f,-.3f), new Vector2(.48f,.38f), new Vector2(-.25f,.5f) }
            };
            var shape = outlines[variant];
            var vertices = new System.Collections.Generic.List<Vector3>();
            var colours = new System.Collections.Generic.List<Color>();
            var triangles = new System.Collections.Generic.List<int>();
            Color[] face = { new Color(.68f,.78f,1f), new Color(.34f,.45f,.76f), new Color(.90f,.96f,1f), new Color(.51f,.60f,.85f) };
            for (int i = 0; i < shape.Length; i++)
            {
                Vector3 a = shape[i], b = shape[(i+1)%shape.Length];
                int start = vertices.Count;
                vertices.Add(new Vector3(.04f,-.02f,-.08f)); vertices.Add(a*.86f); vertices.Add(b*.86f);
                for(int j=0;j<3;j++) { var tint = face[(i+variant)%face.Length]; tint.a = .65f; colours.Add(tint); triangles.Add(start+j); }
                start = vertices.Count;
                vertices.Add(a); vertices.Add(b); vertices.Add(b*.86f); vertices.Add(a*.86f);
                for(int j=0;j<4;j++) colours.Add(new Color(1.15f,1.2f,1.3f));
                triangles.AddRange(new[]{start,start+1,start+2,start,start+2,start+3});
            }
            var mesh = new Mesh { name = "BreakGlassFacet" + variant };
            mesh.SetVertices(vertices); mesh.SetColors(colours); mesh.SetTriangles(triangles,0);
            mesh.RecalculateNormals(); mesh.RecalculateBounds();
            return mesh;
        }
    }
}
