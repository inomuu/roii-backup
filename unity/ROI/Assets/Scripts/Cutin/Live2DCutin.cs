// The cut-in as ONE offscreen 16:9 render, the way godot/views/cutin_view.gd does it: a fixed
// orthographic camera draws the Live2D model + the effects into a 1600x900 texture, and a
// screen-space canvas shows that texture full-screen. Nothing of the arena's pipeline touches it.
//
// Direction = CutinAcolyteClip, copied from the Godot clip. As in Godot the CAMERA never moves:
// the Art node is what the clip animates (offsets / scale), so here the MODEL's transform is what
// moves, and the screen-space effects (rays, vignette, bloom, stars) sit still in the frame.
//
// Why offscreen and not a second camera on the screen: the Cubism URP pass draws EVERY model for
// EVERY game camera (it ignores culling masks), into textures cloned from that camera's colour
// target. On the arena camera that target is HDR + MSAA 4 + post, and the pass's full-screen blit
// came back as noise (Screenshots/live2d_cutin_arena.png). A plain ARGB32 target with no MSAA and
// no HDR is the configuration the SDK's own sample scene renders with. While the cut-in plays the
// arena camera is disabled, so the pass has exactly one camera to draw for.
using Live2D.Cubism.Rendering;
using UnityEngine;
using UnityEngine.InputSystem;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;
using UnityEngine.UI;

namespace Roi.Cutin
{
    public sealed class Live2DCutin : MonoBehaviour
    {
        public const int W = 1600, H = 900;          // the AUTHORING frame; the RT is the window's size
        const int MaxH = 2160;
        const int Layer = 30;
        const float Px = 1f / CutinAcolyteClip.FrameH;   // frame units: 1 = frame height

        public RenderTexture Rt { get; private set; }
        public float Elapsed { get; private set; }
        public bool Finished { get; private set; }
        public GameObject Model { get; private set; }
        public Camera Cam => _cam;
        public Bounds ModelBoundsAtStart => _bounds;

        Camera _cam;
        RawImage _image;
        // SERIALIZED so a script recompile mid-play (which reloads the domain and wipes
        // every plain field) still knows which cameras to switch back on.
        [SerializeField] Camera[] _suspended;
        Bounds _bounds;              // model at origin, scale 1
        CubismRenderController[] _controllers;
        Material _rays, _veil;
        static readonly int VeilColor = Shader.PropertyToID("_Tint");
        Bloom _bloom;
        VolumeProfile _profile;
        CutinStars _stars;

        public static Live2DCutin Play(GameObject modelPrefab, RuntimeAnimatorController motion)
        {
            var root = new GameObject("Cutin");
            var c = root.AddComponent<Live2DCutin>();
            c.Build(modelPrefab, motion);
            return c;
        }

        void Build(GameObject modelPrefab, RuntimeAnimatorController motion)
        {
            // A ROOT object, not a child of this one: the Cubism URP shader places drawables from
            // the render controller's localPosition/localScale, not its world matrix, so a parented
            // model draws at the parent's origin while its Transform (and bounds) sit elsewhere.
            var model = Instantiate(modelPrefab);
            Model = model;
            model.transform.position = Vector3.zero;
            SetLayer(model.transform, Layer);
            var animator = model.GetComponent<Animator>();
            if (animator != null && motion != null) animator.runtimeAnimatorController = motion;
            _bounds = ModelBounds(model);
            _controllers = model.GetComponentsInChildren<CubismRenderController>(true);

            BuildCamera();
            foreach (var rc in _controllers) rc.CameraToFace = _cam;
            BuildPost();
            BuildVeil();
            BuildRays();
            var tex = Resources.Load<Texture2D>("Cutin/star");
            _stars = new CutinStars(_cam.transform, tex, Shader.Find("ROI/CutinParticle"), 8.5f);
            BuildCanvas();
            SuspendOtherCameras();
            BuildDisplayCamera();
            Apply(0f);
        }

        void BuildCamera()
        {
            // 16:9 fitted inside the window, in device pixels: a 1600x900 target upscaled onto a
            // 2027x1166 retina Game view was the "not sharp". Everything is in frame units, so the
            // resolution is free to follow the window.
            var h = Mathf.Min(Screen.height, MaxH);
            var w = Mathf.RoundToInt(h * (float)W / H);
            if (w > Screen.width) { w = Screen.width; h = Mathf.RoundToInt(w * (float)H / W); }
            Rt = new RenderTexture(Mathf.Max(w, 16), Mathf.Max(h, 9), 24, RenderTextureFormat.ARGB32) { antiAliasing = 1, name = "CutinRT" };
            var camGo = new GameObject("CutinCamera") { layer = Layer };
            camGo.transform.SetParent(transform, false);
            camGo.transform.position = new Vector3(0f, 0f, -10f);
            _cam = camGo.AddComponent<Camera>();
            _cam.orthographic = true;
            _cam.orthographicSize = 0.5f;                  // frame height = 1 unit
            _cam.clearFlags = CameraClearFlags.SolidColor;
            _cam.backgroundColor = CutinAcolyteClip.Backdrop;
            _cam.allowHDR = false;
            _cam.allowMSAA = false;
            _cam.cullingMask = 1 << Layer;
            _cam.nearClipPlane = 0.1f;
            _cam.farClipPlane = 50f;
            _cam.targetTexture = Rt;
            var data = camGo.AddComponent<UniversalAdditionalCameraData>();
            data.renderType = CameraRenderType.Base;
            data.renderPostProcessing = true;              // Bloom + Vignette are URP overrides
            data.volumeLayerMask = 1 << Layer;
            data.antialiasing = AntialiasingMode.None;
            data.requiresDepthOption = CameraOverrideOption.Off;
            data.requiresColorOption = CameraOverrideOption.Off;
        }

        // Godot's Bloom (screen blur above threshold, warm tint) and Vignette ColorRects are URP
        // Volume overrides here — the native way, per docs/unity-stack-choices.
        void BuildPost()
        {
            var go = new GameObject("CutinVolume") { layer = Layer };
            go.transform.SetParent(transform, false);
            var vol = go.AddComponent<Volume>();
            vol.isGlobal = true;
            vol.priority = 100f;
            _profile = ScriptableObject.CreateInstance<VolumeProfile>();
            vol.sharedProfile = _profile;
            _bloom = _profile.Add<Bloom>(true);
            _bloom.threshold.value = CutinAcolyteClip.BloomThreshold;
            _bloom.intensity.value = CutinAcolyteClip.Bloom[0];
            _bloom.tint.value = CutinAcolyteClip.BloomTint;
            _bloom.scatter.value = CutinAcolyteClip.BloomScatter;
            _bloom.highQualityFiltering.value = true;
            var vig = _profile.Add<Vignette>(true);
            vig.color.value = Color.black;
            vig.intensity.value = CutinAcolyteClip.VignetteStrength * 0.8f;
            vig.smoothness.value = 1f - CutinAcolyteClip.VignetteInner;
            vig.rounded.value = false;
        }

        // Between the model (world z 0) and the stars/rays: what the cut-in fades in and out.
        void BuildVeil()
        {
            var go = new GameObject("ModelVeil") { layer = Layer };
            go.transform.SetParent(_cam.transform, false);
            go.transform.localPosition = new Vector3(0f, 0f, 9.5f);
            go.transform.localScale = new Vector3((float)W / H * 1.05f, 1.05f, 1f);
            go.AddComponent<MeshFilter>().sharedMesh = Resources.GetBuiltinResource<Mesh>("Quad.fbx");
            _veil = new Material(Shader.Find("ROI/CutinVeil"));
            var mr = go.AddComponent<MeshRenderer>();
            mr.sharedMaterial = _veil;
            mr.shadowCastingMode = ShadowCastingMode.Off;
            mr.receiveShadows = false;
        }

        // Full-frame additive quad in front of the model (Godot draws GodRays after Art).
        void BuildRays()
        {
            var go = new GameObject("GodRays") { layer = Layer };
            go.transform.SetParent(_cam.transform, false);
            go.transform.localPosition = new Vector3(0f, 0f, 9f);
            go.transform.localScale = new Vector3((float)W / H, 1f, 1f);
            go.AddComponent<MeshFilter>().sharedMesh = Resources.GetBuiltinResource<Mesh>("Quad.fbx");
            _rays = new Material(Shader.Find("ROI/CutinGodRays"));
            _rays.SetFloat("_RayPosition", CutinAcolyteClip.RayPosition);
            _rays.SetFloat("_Spread", CutinAcolyteClip.Spread);
            _rays.SetFloat("_Cutoff", CutinAcolyteClip.Cutoff);
            _rays.SetFloat("_Falloff", CutinAcolyteClip.Falloff);
            _rays.SetFloat("_EdgeFade", CutinAcolyteClip.EdgeFade);
            _rays.SetFloat("_Speed", CutinAcolyteClip.RaySpeed);
            _rays.SetFloat("_Ray1Density", CutinAcolyteClip.Ray1Density);
            _rays.SetFloat("_Ray2Density", CutinAcolyteClip.Ray2Density);
            _rays.SetFloat("_Ray2Intensity", CutinAcolyteClip.Ray2Intensity);
            _rays.SetFloat("_Seed", CutinAcolyteClip.RaySeed);
            var mr = go.AddComponent<MeshRenderer>();
            mr.sharedMaterial = _rays;
            mr.shadowCastingMode = ShadowCastingMode.Off;
            mr.receiveShadows = false;
        }

        // The cut-in camera renders into the RT, and the arena camera is suspended, so Display 1
        // would have NO camera and the Game view shows Unity's "No cameras rendering" placeholder
        // under the overlay canvas. This one exists only to own the display: it draws nothing and
        // clears to the backdrop.
        void BuildDisplayCamera()
        {
            var go = new GameObject("CutinDisplayCamera") { layer = Layer };
            go.transform.SetParent(transform, false);
            var cam = go.AddComponent<Camera>();
            cam.clearFlags = CameraClearFlags.SolidColor;
            cam.backgroundColor = Color.black;
            cam.cullingMask = 0;
            cam.depth = -100f;
            cam.allowHDR = false;
            cam.allowMSAA = false;
            var data = go.AddComponent<UniversalAdditionalCameraData>();
            data.renderType = CameraRenderType.Base;
            data.renderPostProcessing = false;
            data.volumeLayerMask = 0;
            data.antialiasing = AntialiasingMode.None;
            data.requiresDepthOption = CameraOverrideOption.Off;
            data.requiresColorOption = CameraOverrideOption.Off;
        }

        void BuildCanvas()
        {
            var canvasGo = new GameObject("CutinCanvas");
            canvasGo.transform.SetParent(transform, false);
            var canvas = canvasGo.AddComponent<Canvas>();
            canvas.renderMode = RenderMode.ScreenSpaceOverlay;
            canvas.sortingOrder = 100;

            var black = new GameObject("Black").AddComponent<Image>();
            black.transform.SetParent(canvasGo.transform, false);
            black.color = Color.black;
            Stretch(black.rectTransform);

            _image = new GameObject("Frame").AddComponent<RawImage>();
            _image.transform.SetParent(canvasGo.transform, false);
            _image.texture = Rt;
            Stretch(_image.rectTransform);
            // Keep 16:9 inside whatever the window is (letterbox), like the Godot blit.
            var fit = _image.gameObject.AddComponent<AspectRatioFitter>();
            fit.aspectRatio = (float)W / H;
            fit.aspectMode = AspectRatioFitter.AspectMode.FitInParent;
        }

        static void Stretch(RectTransform r)
        {
            r.anchorMin = Vector2.zero; r.anchorMax = Vector2.one;
            r.offsetMin = Vector2.zero; r.offsetMax = Vector2.zero;
        }

        void SuspendOtherCameras()
        {
            var all = FindObjectsByType<Camera>(FindObjectsSortMode.None);
            var list = new System.Collections.Generic.List<Camera>();
            foreach (var c in all) if (c != _cam && c.enabled) { c.enabled = false; list.Add(c); }
            _suspended = list.ToArray();
        }

        void Update()
        {
            if (Finished) return;
            // A recompile while playing reloads the domain and leaves this component alive with
            // its runtime pieces gone. Without this the next line threw every frame, the cut-in
            // froze on screen and never ended (user, 2026-09-06: "ไลฟ2d ไม่ขยับซะแล้ว").
            if (_stars == null || _rays == null || _cam == null) { End(); return; }
            if (SkipPressed()) { End(); return; }
            var dt = Time.deltaTime;
            Elapsed += dt;
            Apply(Elapsed);
            _stars.Tick(Elapsed, dt);
            if (Elapsed >= CutinAcolyteClip.Length) End();
        }

        // Godot: any click, or Space / Enter / Esc.
        static bool SkipPressed()
        {
            var k = Keyboard.current;
            if (k != null && (k.spaceKey.wasPressedThisFrame || k.enterKey.wasPressedThisFrame || k.escapeKey.wasPressedThisFrame)) return true;
            var m = Mouse.current;
            return m != null && m.leftButton.wasPressedThisFrame;
        }

        void Apply(float t)
        {
            PlaceModel(t);
            // Fade a black veil over the finished model, NOT the drawables' own opacity: a Live2D
            // rig is dozens of overlapping meshes, so per-drawable alpha reveals every internal
            // seam mid-fade. Godot faded one composited TextureRect; the veil is that composite.
            var artA = CutinAcolyteClip.Eval(t, CutinAcolyteClip.ArtAlphaT, CutinAcolyteClip.ArtAlpha, false);
            _veil.SetColor(VeilColor, new Color(0f, 0f, 0f, 1f - artA));
            _rays.SetFloat("_Angle", CutinAcolyteClip.Eval(t, CutinAcolyteClip.RaysAngleT, CutinAcolyteClip.RaysAngle, false));
            _rays.SetColor("_Color", CutinAcolyteClip.Eval(t, CutinAcolyteClip.RaysColorT, CutinAcolyteClip.RaysColor, true));
            _rays.SetFloat("_Modulate", CutinAcolyteClip.Eval(t, CutinAcolyteClip.RaysAlphaT, CutinAcolyteClip.RaysAlpha, false));
            _bloom.intensity.value = CutinAcolyteClip.Eval(t, CutinAcolyteClip.BloomT, CutinAcolyteClip.Bloom, false);
        }

        // Godot: Art is a TextureRect anchored to the full frame, its rect = the anchor rect plus
        // offset_left/top/right/bottom, the art fitted keep-aspect-centred inside it, then
        // Art:scale about the frame centre. The PNG's opaque content box maps onto the model's
        // bounds, so the rig stands exactly where the drawn figure would.
        void PlaceModel(float t)
        {
            var l = CutinAcolyteClip.Eval(t, CutinAcolyteClip.RectT, CutinAcolyteClip.Left, true);
            var tp = CutinAcolyteClip.Eval(t, CutinAcolyteClip.RectT, CutinAcolyteClip.Top, true);
            var r = CutinAcolyteClip.FrameW + CutinAcolyteClip.Eval(t, CutinAcolyteClip.RectT, CutinAcolyteClip.Right, true);
            var b = CutinAcolyteClip.FrameH + CutinAcolyteClip.Eval(t, CutinAcolyteClip.RectT, CutinAcolyteClip.Bottom, true);
            var s = CutinAcolyteClip.Eval(t, CutinAcolyteClip.ArtScaleT, CutinAcolyteClip.ArtScale, false) * CutinAcolyteClip.Settle(t);
            var fit = Mathf.Min((r - l) / CutinAcolyteClip.ArtW, (b - tp) / CutinAcolyteClip.ArtH) * s;
            // PNG px -> frame px (y down): art centred in the rect, rect scaled about (800,450).
            var artCx = 800f + ((l + r) * 0.5f - 800f) * s;
            var artCy = 450f + ((tp + b) * 0.5f - 450f) * s;
            var contentCx = artCx + ((CutinAcolyteClip.ContentX0 + CutinAcolyteClip.ContentX1) * 0.5f - CutinAcolyteClip.ArtW * 0.5f) * fit;
            var contentCy = artCy + ((CutinAcolyteClip.ContentY0 + CutinAcolyteClip.ContentY1) * 0.5f - CutinAcolyteClip.ArtH * 0.5f) * fit;
            var contentH = (CutinAcolyteClip.ContentY1 - CutinAcolyteClip.ContentY0) * fit;
            var k = (contentH * Px) / _bounds.size.y;                     // frame units per model unit
            var centre = new Vector3((contentCx - 800f) * Px, (450f - contentCy) * Px, 0f);
            Model.transform.localScale = new Vector3(k, k, 1f);
            Model.transform.position = centre - _bounds.center * k;
        }

        void End()
        {
            Finished = true;
            RestoreCameras();
            Destroy(gameObject);
        }

        void RestoreCameras()
        {
            if (_suspended == null) return;
            foreach (var c in _suspended) if (c != null) c.enabled = true;
        }

        void OnDestroy()
        {
            if (!Finished) RestoreCameras();
            if (Model != null) Destroy(Model);
            _stars?.Destroy();
            if (_rays != null) Destroy(_rays);
            if (_veil != null) Destroy(_veil);
            if (_profile != null) Destroy(_profile);
            if (Rt != null) Rt.Release();
        }

        // From the drawables' vertices: the prefab's serialized meshes carry zero bounds until a
        // vertex update recalculates them, and that has not happened on the first frame.
        static Bounds ModelBounds(GameObject model)
        {
            var any = false;
            var b = new Bounds();
            foreach (var r in model.GetComponentsInChildren<CubismRenderer>(true))
            {
                var mesh = r.Mesh;
                if (mesh == null) continue;
                foreach (var v in mesh.vertices)
                {
                    if (!any) { b = new Bounds(v, Vector3.zero); any = true; } else b.Encapsulate(v);
                }
            }
            if (!any || b.size.y < 0.01f) b = new Bounds(Vector3.zero, new Vector3(1f, 1f, 0.1f));
            return b;
        }

        static void SetLayer(Transform t, int layer)
        {
            t.gameObject.layer = layer;
            for (var i = 0; i < t.childCount; i++) SetLayer(t.GetChild(i), layer);
        }
    }
}
