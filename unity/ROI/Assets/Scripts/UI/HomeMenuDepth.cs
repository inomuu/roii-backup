using UnityEngine;
using UnityEngine.InputSystem;
using UnityEngine.Rendering.Universal;
using UnityEngine.UI;

namespace Roi.UI
{
    // Cards and wallet share a real perspective plane. Profile, portrait and open windows remain overlay UI.
    public sealed class HomeMenuDepth : MonoBehaviour
    {
        [SerializeField] bool depthEnabled = true;
        [SerializeField] bool pointerMotion = true;
        [SerializeField, Range(0, 35)] float yaw = 8;
        [SerializeField, Range(25, 50)] float fieldOfView = 35;
        const int MenuLayer = 29;
        RectTransform _source, _canvasRect, _deck, _bottom;
        GameObject _space;
        Camera _camera;
        RawImage _composite;
        RenderTexture _target;
        Material _compositeMaterial, _shadowMaterial, _contactMaterial;
        readonly System.Collections.Generic.List<Material> _surfaces = new();
        Vector2 _pointer, _size;
        public bool DepthEnabled => depthEnabled;
        public Camera EventCamera => _camera;

        public static HomeMenuDepth Create(RectTransform root)
        {
            var depth = root.gameObject.AddComponent<HomeMenuDepth>();
            depth.Build(root);
            return depth;
        }

        void Build(RectTransform root)
        {
            _source = root;
            // Separate root avoids inheriting an overlay Canvas' render mode or combat transforms.
            _space = new GameObject("HomePerspectiveSpace");
            _space.transform.position = new Vector3(10000, 10000, 0);
            var cameraGo = new GameObject("HomePerspectiveCamera", typeof(Camera));
            cameraGo.transform.SetParent(_space.transform, false);
            _camera = cameraGo.GetComponent<Camera>();
            _camera.orthographic = false;
            _camera.clearFlags = CameraClearFlags.SolidColor;
            _camera.backgroundColor = Color.clear;
            _camera.cullingMask = 1 << MenuLayer;
            _camera.nearClipPlane = 1; _camera.farClipPlane = 8000;
            _camera.allowHDR = false; _camera.allowMSAA = true;
            _camera.depth = -20; // Render the offscreen cards before the final screen/overlay camera.
            var data = _camera.GetUniversalAdditionalCameraData();
            // Dedicated forward renderer at slot 1 in both quality assets: no SSAO/Cubism/post FX.
            data.SetRenderer(1);
            data.renderPostProcessing = false;
            data.renderShadows = false;
            data.requiresColorOption = CameraOverrideOption.Off;
            data.requiresDepthOption = CameraOverrideOption.Off;
            data.antialiasing = AntialiasingMode.None;

            var go = new GameObject("HomePerspectiveCanvas", typeof(RectTransform), typeof(Canvas), typeof(GraphicRaycaster));
            go.transform.SetParent(_space.transform, false);
            _canvasRect = (RectTransform)go.transform;
            var canvas = go.GetComponent<Canvas>();
            canvas.renderMode = RenderMode.WorldSpace;
            canvas.worldCamera = _camera;
            canvas.sortingOrder = 45;
            canvas.additionalShaderChannels = AdditionalCanvasShaderChannels.TexCoord1 | AdditionalCanvasShaderChannels.Normal | AdditionalCanvasShaderChannels.Tangent;
            _deck = Group(_canvasRect, "MainMenuDeck", new Vector2(.80f, .50f));
            _bottom = Group(_canvasRect, "UtilityDeck", new Vector2(.30f, .08f));
            _shadowMaterial = new Material(Resources.Load<Shader>("HomeUI/HomePanelShadow"));
            _contactMaterial = new Material(_shadowMaterial);
            _contactMaterial.SetFloat("_Feather",.018f);
            foreach (var id in new[] { "Story", "Event", "Mission", "Inventory", "Workshop", "Exemplars", "Party", "Excavation", "Store" })
                MoveCard(root, _deck, id);
            foreach (var id in new[] { "Secondary3" }) MoveCard(root, _bottom, id);
            // One camera and plane keep currency baselines converging with the cards.
            Reparent(root.Find("HomeWallet") as RectTransform, _deck);
            foreach (Transform child in _space.GetComponentsInChildren<Transform>(true)) child.gameObject.layer = MenuLayer;

            var display = new GameObject("PerspectiveCards", typeof(RectTransform), typeof(RawImage));
            display.transform.SetParent(root, false);
            _composite = display.GetComponent<RawImage>();
            var rect = _composite.rectTransform;
            rect.anchorMin = Vector2.zero; rect.anchorMax = Vector2.one;
            rect.offsetMin = rect.offsetMax = Vector2.zero;
            rect.SetSiblingIndex(5); // Above the portrait, below header and overlay windows.
            _composite.raycastTarget = false;
            _compositeMaterial = new Material(Resources.Load<Shader>("HomeUI/HomePerspectiveComposite"));
            _composite.material = _compositeMaterial;
            SyncProjection();
            Apply();
        }

        static RectTransform Group(RectTransform root, string name, Vector2 pivot)
        {
            var rt = (RectTransform)new GameObject(name, typeof(RectTransform)).transform;
            rt.SetParent(root, false); rt.pivot = pivot;
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = rt.offsetMax = Vector2.zero;
            return rt;
        }

        static void Reparent(RectTransform rt, Transform parent)
        {
            if (rt == null) return;
            var min = rt.offsetMin; var max = rt.offsetMax;
            rt.SetParent(parent, false);
            rt.localScale = Vector3.one; rt.localRotation = Quaternion.identity;
            rt.offsetMin = min; rt.offsetMax = max;
            var pos = rt.localPosition; pos.z = 0; rt.localPosition = pos;
        }

        void MoveCard(Transform root, Transform group, string id)
        {
            var shadow = root.Find(id + "Shadow") as RectTransform;
            var contact = root.Find(id + "ContactShadow") as RectTransform;
            var face = root.Find(id) as RectTransform;
            Reparent(shadow, group);
            if (shadow != null)
            {
                shadow.GetComponent<Image>().material = _shadowMaterial;
                shadow.offsetMin += new Vector2(-14, -14);
                shadow.offsetMax += new Vector2(14, 14);
            }
            Reparent(contact, group);
            if(contact!=null)contact.GetComponent<Image>().material=_contactMaterial;
            Reparent(face, group);
            if (face != null)
            {
                foreach (var image in face.GetComponentsInChildren<Image>())
                    if (image.name.EndsWith("Shadow")) image.material = _shadowMaterial;
                float elevation=id switch { "Event"=>55, "Party"=>22, "Store"=>34, "Mission"=>12, "Workshop"=>22, "Inventory"=>16, "Exemplars"=>8, _=>0 };
                var material=new Material(Resources.Load<Shader>("HomeUI/HomePanelSurface"));
                material.SetFloat("_PatternStrength",id.StartsWith("Secondary")?.45f:1);
                _surfaces.Add(material);face.GetComponent<Image>().material=material;
                face.gameObject.AddComponent<HomePanelThickness>().Thickness=id=="Event"?8:6;
                face.gameObject.AddComponent<HomeCardLift>().Bind(this, face, shadow, contact, -elevation, material);
            }
        }

        public void SetDepthEnabled(bool value)
        {
            depthEnabled = value; _pointer = Vector2.zero; Apply();
        }

        void SyncProjection()
        {
            int width = Mathf.Max(1, Screen.width), height = Mathf.Max(1, Screen.height);
            var size = _source.rect.size;
            if (size.x < 1 || size.y < 1) size = new Vector2(1080f * width / height, 1080);
            if (size != _size)
            {
                _size = size;
                _canvasRect.sizeDelta = size;
            }
            _camera.fieldOfView = fieldOfView;
            _camera.aspect = (float)width / height;
            float distance = size.y / (2 * Mathf.Tan(fieldOfView * Mathf.Deg2Rad * .5f));
            _camera.transform.localPosition = new Vector3(0, 0, -distance);
            if (_target != null && _target.width == width && _target.height == height) return;
            var previous = _target;
            _target = new RenderTexture(width, height, 24, RenderTextureFormat.ARGB32)
            {
                name = "Home perspective UI 4x MSAA", antiAliasing = 4,
                filterMode = FilterMode.Bilinear, wrapMode = TextureWrapMode.Clamp
            };
            _target.Create();
            _camera.targetTexture = _target;
            _composite.texture = _target;
            // Destroy is deferred until rendering is finished; releasing an in-flight Metal RT is unsafe.
            if (previous != null) Destroy(previous);
        }

        void LateUpdate()
        {
            if (_camera == null) return;
            SyncProjection();
            var target = Vector2.zero;
            if (depthEnabled && pointerMotion && Application.isFocused && Mouse.current != null)
            {
                var p = Mouse.current.position.ReadValue();
                if (p.x >= 0 && p.y >= 0 && p.x <= Screen.width && p.y <= Screen.height)
                    target = new Vector2(p.x / Mathf.Max(1, Screen.width) * 2 - 1,
                        p.y / Mathf.Max(1, Screen.height) * 2 - 1);
            }
            _pointer = Vector2.Lerp(_pointer, target, 1 - Mathf.Exp(-7 * Time.unscaledDeltaTime));
            Apply();
        }

        void Apply()
        {
            if (_deck == null) return;
            // No Z roll: perspective makes upper rows rise right, lower rows fall right.
            _deck.localRotation = depthEnabled ? Quaternion.Euler(-_pointer.y * .35f, yaw + _pointer.x * .7f, 0) : Quaternion.identity;
            _deck.localScale = Vector3.one * (depthEnabled ? .96f : 1);
            _deck.anchoredPosition = depthEnabled ? new Vector2(-_size.x * .022f, 0) + _pointer * 3 : Vector2.zero;
            _bottom.localRotation = depthEnabled ? Quaternion.Euler(0, -5, 0) : Quaternion.identity;
        }

        void OnEnable() { if (_space != null) _space.SetActive(true); }
        void OnDisable()
        {
            _pointer = Vector2.zero;
            if (_space != null) _space.SetActive(false);
        }
        void OnDestroy()
        {
            if (_camera != null) _camera.targetTexture = null;
            if (_space != null) Destroy(_space);
            if (_target != null) Destroy(_target);
            if (_compositeMaterial != null) Destroy(_compositeMaterial);
            if (_shadowMaterial != null) Destroy(_shadowMaterial);
            if (_contactMaterial != null) Destroy(_contactMaterial);
            foreach(var material in _surfaces)if(material!=null)Destroy(material);
        }
    }
}
