using System.Collections.Generic;
using UnityEngine;
using UnityEngine.Rendering;

namespace Roi.Arena
{
    // The guard's white-out, separate from incoming attack VFX and the real shadow-casting key.
    [DefaultExecutionOrder(100)] // follow after the arena updates visual sprite interpolation
    public sealed class ArenaBreakFlash : MonoBehaviour
    {
        public void DelayPresentation(float seconds)
        {
            foreach (var pair in _pairs) if (pair.Active) pair.Until+=seconds;
        }
        sealed class Pair
        {
            public Transform Root, Target;
            public ParticleSystem Core, Halo;
            public Vector3 Offset;
            public float Until;
            public bool Active;
        }
        readonly List<Pair> _pairs = new List<Pair>(4);
        Material _coreMaterial, _haloMaterial;
        public static ArenaBreakFlash Create(Transform parent)
        {
            var go = new GameObject("ArenaBreakFlash");
            go.transform.SetParent(parent, false);
            var flash = go.AddComponent<ArenaBreakFlash>();
            flash.Build();
            return flash;
        }
        void Build()
        {
            var shader = Resources.Load<Shader>("Shaders/BreakFlash");
            if (shader == null) { Debug.LogError("BREAK: missing Resources/Shaders/BreakFlash shader"); return; }
            _haloMaterial = new Material(shader) { name = "BREAK icy haze", renderQueue = 2990 };
            _haloMaterial.SetColor("_Color", new Color(.48f, .62f, 1.15f, 1f));
            _coreMaterial = new Material(shader) { name = "BREAK white core", renderQueue = 2990 };
            _coreMaterial.SetColor("_Color", new Color(4.4f, 4.8f, 5.2f, 1f));
            // Grow only when simultaneous breaks exceed the warm pool, never during LateUpdate.
            NewPair(); NewPair();
        }
        Pair NewPair()
        {
            var root = new GameObject("Flash pair").transform;
            root.SetParent(transform, false);
            var pair = new Pair { Root = root };
            pair.Halo = Layer(root, "Halo", _haloMaterial, 1.3f, new[] {
                new GradientAlphaKey(0f, 0f), new GradientAlphaKey(.55f, .04f),
                new GradientAlphaKey(.42f, .54f), new GradientAlphaKey(.16f, .8f),
                new GradientAlphaKey(0f, 1f) });
            pair.Core = Layer(root, "Core", _coreMaterial, 1.15f, new[] {
                new GradientAlphaKey(0f, 0f), new GradientAlphaKey(1f, .035f),
                new GradientAlphaKey(.95f, .61f), new GradientAlphaKey(.45f, .79f),
                new GradientAlphaKey(0f, 1f) });
            root.gameObject.SetActive(false);
            _pairs.Add(pair);
            return pair;
        }
        public void Burst(Vector3 centre, float targetHeight, Camera cam, Transform target = null)
        {
            if (_coreMaterial == null || _haloMaterial == null) return;
            Pair pair = null;
            for (int i = 0; i < _pairs.Count; i++)
                if (!_pairs[i].Active) { pair = _pairs[i]; break; }
            if (pair == null) pair = NewPair();
            float h = Mathf.Max(.2f, targetHeight);
            var towardCamera = cam != null ? (cam.transform.position - centre).normalized : Vector3.forward;
            // Keep feet readable: core sits at upper/mid body and in front of its opaque sprite.
            var at = centre + Vector3.up * (h * .13f) + towardCamera * (h * .16f);
            pair.Root.SetPositionAndRotation(at, Quaternion.identity);
            pair.Target = target;
            pair.Offset = target != null ? at - target.position : Vector3.zero;
            pair.Until = Time.unscaledTime + 1.3f;
            pair.Active = true;
            pair.Root.gameObject.SetActive(true);
            pair.Core.Clear(); pair.Halo.Clear();
            pair.Core.Play(); pair.Halo.Play();
            Emit(pair.Halo, Vector3.zero, new Vector3(h * 1.48f, h * 1.9f, 1f), -7f);
            Emit(pair.Core, towardCamera * (h * .025f), new Vector3(h * .82f, h * 1.16f, 1f), 4f);
        }
        void LateUpdate()
        {
            for (int i = 0; i < _pairs.Count; i++)
            {
                var pair = _pairs[i];
                if (!pair.Active) continue;
                if (Time.unscaledTime >= pair.Until)
                {
                    pair.Core.Stop(true, ParticleSystemStopBehavior.StopEmittingAndClear);
                    pair.Halo.Stop(true, ParticleSystemStopBehavior.StopEmittingAndClear);
                    pair.Root.gameObject.SetActive(false);
                    pair.Active = false; pair.Target = null;
                }
                else if (pair.Target != null) pair.Root.position = pair.Target.position + pair.Offset;
            }
        }
        static void Emit(ParticleSystem ps, Vector3 at, Vector3 size, float angle)
        {
            ps.Emit(new ParticleSystem.EmitParams {
                position = at, velocity = Vector3.zero, startSize3D = size,
                startColor = Color.white, rotation = angle }, 1);
        }
        ParticleSystem Layer(Transform parent, string name, Material material, float life, GradientAlphaKey[] alpha)
        {
            var go = new GameObject(name);
            go.transform.SetParent(parent, false);
            var ps = go.AddComponent<ParticleSystem>();
            ps.Stop(true, ParticleSystemStopBehavior.StopEmittingAndClear);
            var main = ps.main;
            main.useUnscaledTime = true;
            main.startLifetime = life;
            main.startSpeed = 0f;
            main.startSize3D = true;
            main.simulationSpace = ParticleSystemSimulationSpace.Local;
            main.maxParticles = 1;
            main.playOnAwake = false;
            var emission = ps.emission; emission.enabled = false;
            var shape = ps.shape; shape.enabled = false;
            var gradient = new Gradient();
            gradient.SetKeys(new[] { new GradientColorKey(Color.white, 0f), new GradientColorKey(Color.white, 1f) }, alpha);
            var color = ps.colorOverLifetime;
            color.enabled = true; color.color = new ParticleSystem.MinMaxGradient(gradient);
            var size = ps.sizeOverLifetime;
            size.enabled = true;
            size.size = new ParticleSystem.MinMaxCurve(1f, AnimationCurve.Linear(0f, .9f, 1f, 1.08f));
            var renderer = ps.GetComponent<ParticleSystemRenderer>();
            renderer.renderMode = ParticleSystemRenderMode.Billboard;
            renderer.alignment = ParticleSystemRenderSpace.View;
            renderer.sharedMaterial = material;
            renderer.shadowCastingMode = ShadowCastingMode.Off;
            renderer.receiveShadows = false;
            renderer.maxParticleSize = 1f;
            ps.Play();
            return ps;
        }
        void OnDestroy()
        {
            if (_coreMaterial != null) Destroy(_coreMaterial);
            if (_haloMaterial != null) Destroy(_haloMaterial);
        }
    }
}
