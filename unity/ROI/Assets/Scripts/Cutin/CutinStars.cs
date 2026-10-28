// The three star emitters of the acolyte cut-in (cutin_acolyte.tscn Stars / StarBurst /
// StarShimmer), as Unity ParticleSystems in FRAME units: 1 unit = 900 px, origin at the frame
// centre, y up. Godot values are converted at the boundary (px * Px, y flipped) and nowhere else.
using UnityEngine;

namespace Roi.Cutin
{
    public sealed class CutinStars
    {
        const float Px = 1f / CutinAcolyteClip.FrameH;
        const float Sprite = 96f;          // star.png
        const float StarsRate = 88f / 2.6f;  // amount / lifetime = continuous emit rate

        // Stars: emission_points — the frame's four edges.
        static readonly Vector2[] StarPoints =
        {
            new Vector2(518.1f, 13.6f), new Vector2(1041.5f, 6.5f), new Vector2(857.4f, 32.9f), new Vector2(92.8f, 45.7f),
            new Vector2(60f, 39f), new Vector2(111.8f, 8.2f), new Vector2(679.2f, 74.4f), new Vector2(198.1f, 20.1f),
            new Vector2(1003.9f, 85.3f), new Vector2(923.4f, 35.7f), new Vector2(1562f, 4.2f), new Vector2(1373.5f, 26.1f),
            new Vector2(230.8f, 10.6f), new Vector2(493.6f, 73.5f), new Vector2(289.2f, 52.3f), new Vector2(1022.3f, 33.5f),
            new Vector2(876.4f, 5.7f), new Vector2(95.4f, 18.5f), new Vector2(1088.6f, 38.5f), new Vector2(502.6f, 52.7f),
            new Vector2(725.1f, 27f), new Vector2(1271f, 62.9f), new Vector2(390.6f, 51.7f), new Vector2(840.3f, 78.8f),
            new Vector2(1167.1f, 25.9f), new Vector2(1568.3f, 10.6f), new Vector2(669f, 878.1f), new Vector2(243.2f, 854f),
            new Vector2(62.7f, 870.1f), new Vector2(1223.3f, 861.6f), new Vector2(1400.8f, 838.2f), new Vector2(1112.5f, 863.5f),
            new Vector2(927.8f, 851.1f), new Vector2(1343.9f, 895f), new Vector2(758.6f, 869.8f), new Vector2(97.1f, 873.1f),
            new Vector2(1035.4f, 899.4f), new Vector2(1315.1f, 835.6f), new Vector2(617.3f, 870.2f), new Vector2(36.1f, 851.6f),
            new Vector2(268.9f, 820.5f), new Vector2(94.3f, 879.1f), new Vector2(206.9f, 832.3f), new Vector2(625.5f, 888.4f),
            new Vector2(128.9f, 850.4f), new Vector2(879.1f, 889.5f), new Vector2(1310.8f, 887.8f), new Vector2(445.5f, 847.4f),
            new Vector2(574f, 889.6f), new Vector2(1532.4f, 823.6f), new Vector2(281.9f, 830.9f), new Vector2(373.3f, 853.6f),
            new Vector2(64.8f, 236.5f), new Vector2(0.5f, 377.1f), new Vector2(40.6f, 509.7f), new Vector2(104.8f, 621.4f),
            new Vector2(56.7f, 555.8f), new Vector2(74.4f, 48.6f), new Vector2(98.9f, 702f), new Vector2(96.2f, 718.1f),
            new Vector2(43.2f, 359.1f), new Vector2(11.4f, 570.9f), new Vector2(6.8f, 60.6f), new Vector2(23f, 146.1f),
            new Vector2(37.4f, 47.3f), new Vector2(0f, 136.1f), new Vector2(11.2f, 327.2f), new Vector2(2.8f, 786.9f),
            new Vector2(67.5f, 133.7f), new Vector2(27.7f, 312.7f), new Vector2(1530.1f, 110.6f), new Vector2(1583.4f, 893.8f),
            new Vector2(1541.3f, 435.5f), new Vector2(1499.4f, 92f), new Vector2(1527.7f, 238.3f), new Vector2(1581.2f, 145.3f),
            new Vector2(1492.5f, 855.9f), new Vector2(1548.1f, 131.9f), new Vector2(1549.7f, 24.3f), new Vector2(1548.1f, 880.7f),
            new Vector2(1585f, 626.6f), new Vector2(1518.7f, 330f), new Vector2(1508.4f, 694.7f), new Vector2(1548.6f, 701.1f),
            new Vector2(1526.3f, 200.7f), new Vector2(1579.3f, 886.4f), new Vector2(1583.8f, 725.5f), new Vector2(1580f, 665.9f),
        };

        readonly ParticleSystem _stars, _burst, _shimmer;
        readonly Material _starsMat, _burstMat, _shimmerMat;
        readonly float _z;
        float _acc;
        bool _burstDone, _shimmerOn;

        public CutinStars(Transform parent, Texture2D tex, Shader shader, float z)
        {
            _z = z;
            _stars = Make("Stars", parent, tex, shader, z, out _starsMat);
            var sm = _stars.main;
            sm.gravityModifier = -(22f * Px) / 9.81f;                  // gravity (0, -22): up

            _burst = Make("StarBurst", parent, tex, shader, z - 0.01f, out _burstMat);
            var bm = _burst.main;
            bm.gravityModifier = (500f * Px) / 9.81f;                   // gravity (0, 500): down
            // radial_accel 1600-2500 with damping: the burst ACCELERATES outward. Unity has no
            // radial force; a rising speed modifier over the particle's life reads the same.
            var vel = _burst.velocityOverLifetime;
            vel.enabled = true;
            vel.speedModifier = new ParticleSystem.MinMaxCurve(1f, AnimationCurve.Linear(0f, 1f, 1f, 5f));

            _shimmer = Make("StarShimmer", parent, tex, shader, z - 0.02f, out _shimmerMat);
            _shimmer.transform.localPosition = new Vector3(0f, (450f - 430f) * Px, z - 0.02f);
            var hm = _shimmer.main;
            hm.gravityModifier = -(8f * Px) / 9.81f;
            hm.startLifetime = new ParticleSystem.MinMaxCurve(1.4f * 0.3f, 1.4f);   // lifetime_randomness 0.7
            hm.startSpeed = new ParticleSystem.MinMaxCurve(4f * Px, 22f * Px);
            hm.startSize = new ParticleSystem.MinMaxCurve(Sprite * Px * 0.1f, Sprite * Px * 0.4f);
            var sh = _shimmer.shape;
            sh.enabled = true;
            sh.shapeType = ParticleSystemShapeType.Box;
            sh.scale = new Vector3(2f * 520f * Px, 2f * 380f * Px, 0f);   // emission_rect_extents
            sh.randomDirectionAmount = 1f;                                // spread 180
            var em = _shimmer.emission;
            em.rateOverTime = 34f / 1.4f;
            em.enabled = false;
        }

        static ParticleSystem Make(string name, Transform parent, Texture2D tex, Shader shader, float z, out Material mat)
        {
            var go = new GameObject(name) { layer = parent.gameObject.layer };
            go.transform.SetParent(parent, false);
            go.transform.localPosition = new Vector3(0f, 0f, z);
            var ps = go.AddComponent<ParticleSystem>();
            ps.Stop(true, ParticleSystemStopBehavior.StopEmittingAndClear);
            var main = ps.main;
            main.playOnAwake = false;
            main.loop = true;
            main.simulationSpace = ParticleSystemSimulationSpace.World;
            main.scalingMode = ParticleSystemScalingMode.Local;
            main.maxParticles = 512;
            main.startRotation = new ParticleSystem.MinMaxCurve(0f, Mathf.PI * 2f);
            var em = ps.emission;
            em.enabled = false;
            var sh = ps.shape;
            sh.enabled = false;
            var col = ps.colorOverLifetime;
            col.enabled = true;
            col.color = new ParticleSystem.MinMaxGradient(StarRamp());
            var sz = ps.sizeOverLifetime;
            sz.enabled = true;
            sz.size = new ParticleSystem.MinMaxCurve(1f, StarCurve());
            var r = go.GetComponent<ParticleSystemRenderer>();
            mat = new Material(shader) { mainTexture = tex };
            r.material = mat;
            r.renderMode = ParticleSystemRenderMode.Billboard;
            r.sortMode = ParticleSystemSortMode.None;
            ps.Play();
            return ps;
        }

        // Grad_star: white in, gold, gone.
        static Gradient StarRamp()
        {
            var g = new Gradient();
            g.SetKeys(
                new[]
                {
                    new GradientColorKey(Color.white, 0f), new GradientColorKey(Color.white, 0.15f),
                    new GradientColorKey(new Color(1f, 0.82f, 0.45f), 0.5f), new GradientColorKey(new Color(1f, 0.72f, 0.38f), 1f),
                },
                new[]
                {
                    new GradientAlphaKey(0f, 0f), new GradientAlphaKey(1f, 0.15f),
                    new GradientAlphaKey(1f, 0.5f), new GradientAlphaKey(0f, 1f),
                });
            return g;
        }

        // Curve_star: 0.15 -> 1.0 at 35% -> 0.1.
        static AnimationCurve StarCurve() => new AnimationCurve(new Keyframe(0f, 0.15f), new Keyframe(0.35f, 1f), new Keyframe(1f, 0.1f));

        Vector3 Frame(float px, float py) => new Vector3((px - 800f) * Px, (450f - py) * Px, _z);

        public void Tick(float t, float dt)
        {
            _acc += dt * StarsRate;
            while (_acc >= 1f) { _acc -= 1f; EmitStar(); }
            if (!_burstDone && t >= CutinAcolyteClip.StarBurstAt) { _burstDone = true; Burst(); }
            if (!_shimmerOn && t >= CutinAcolyteClip.ShimmerAt)
            {
                _shimmerOn = true;
                var em = _shimmer.emission;
                em.enabled = true;
            }
            _starsMat.SetColor(Tint, new Color(1f, 1f, 1f, CutinAcolyteClip.Eval(t, CutinAcolyteClip.StarsAlphaT, CutinAcolyteClip.StarsAlpha, false)));
            _burstMat.SetColor(Tint, new Color(1f, 1f, 1f, 1f));
            _shimmerMat.SetColor(Tint, new Color(1.4f, 1.4f, 1.3f, CutinAcolyteClip.Eval(t, CutinAcolyteClip.ShimmerAlphaT, CutinAcolyteClip.ShimmerAlpha, false)));
        }

        static readonly int Tint = Shader.PropertyToID("_Tint");

        // Stars: direction (0,-1) = up, spread 70, velocity 12-48, scale 0.32-1.05, spin +-40.
        void EmitStar()
        {
            var p = StarPoints[Random.Range(0, StarPoints.Length)];
            var dir = Quaternion.Euler(0f, 0f, Random.Range(-70f, 70f)) * Vector3.up;
            var e = new ParticleSystem.EmitParams
            {
                position = Frame(p.x, p.y),
                velocity = dir * (Random.Range(12f, 48f) * Px),
                startLifetime = 2.6f * (1f - Random.value * 0.4f),
                startSize = Sprite * Px * Random.Range(0.32f, 1.05f),
                rotation = Random.Range(0f, 360f),
                angularVelocity = Random.Range(-40f, 40f),
                startColor = Color.white,
                applyShapeToPosition = false,
            };
            _stars.Emit(e, 1);
        }

        // StarBurst: 64 one-shot from the centre (radius 24), every direction, scale 0.45-1.6.
        void Burst()
        {
            for (var i = 0; i < 64; i++)
            {
                var dir = Random.insideUnitCircle.normalized;
                var c = Frame(800f, 450f) + (Vector3)(dir * (Random.Range(0f, 24f) * Px));
                var e = new ParticleSystem.EmitParams
                {
                    position = new Vector3(c.x, c.y, _z - 0.01f),
                    velocity = (Vector3)dir * (Random.Range(300f, 600f) * Px),
                    startLifetime = 1.6f,
                    startSize = Sprite * Px * Random.Range(0.45f, 1.6f),
                    rotation = Random.Range(0f, 360f),
                    angularVelocity = Random.Range(-120f, 120f),
                    startColor = Color.white,
                    applyShapeToPosition = false,
                };
                _burst.Emit(e, 1);
            }
        }

        public void Destroy()
        {
            Object.Destroy(_starsMat); Object.Destroy(_burstMat); Object.Destroy(_shimmerMat);
        }
    }
}
