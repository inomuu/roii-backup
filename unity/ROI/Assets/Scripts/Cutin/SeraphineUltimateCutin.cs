using UnityEngine;
using UnityEngine.InputSystem;
using UnityEngine.UI;
using Roi.Sim;

namespace Roi.Cutin
{
    // Presentation-only prototype. A gameplay trigger must decide when it is safe to show it.
    [ExecuteAlways]
    public sealed class SeraphineUltimateCutin : MonoBehaviour
    {
        public const float Duration = 1.8f;
        [SerializeField] Material _material;
        [SerializeField] Canvas _canvas;
        [SerializeField] float _age;
        [SerializeField] bool _automatic;
        [SerializeField] SimDriver _driver;
        public Canvas Surface => _canvas;
        public float Age => _age;

        public static SeraphineUltimateCutin Play()
        {
            if (FindFirstObjectByType<Live2DCutin>() != null ||
                FindFirstObjectByType<SeraphineUltimateCutin>() != null) return null;
            var clip = Create(true);
            if (clip != null) clip.HoldSimulation(FindFirstObjectByType<SimDriver>());
            return clip;
        }

        public void HoldSimulation(SimDriver driver)
        {
            ReleaseSimulation();
            _driver = driver;
            if (_driver != null) _driver.HoldPresentation(this);
        }

        void ReleaseSimulation()
        {
            if (_driver != null) _driver.ReleasePresentation(this);
            _driver = null;
        }

        public static SeraphineUltimateCutin Create(bool automatic)
        {
            var shader = Resources.Load<Shader>("Cutin/SeraphineAegisCutin");
            var art = Resources.Load<Texture2D>("EquipmentPortraits/Seraphine");
            var aura = Resources.Load<Texture2D>("Cutin/SeraphineAuraFlow-v1");
            if (shader == null || !shader.isSupported || art == null || aura == null)
            {
                Debug.LogWarning("AEGIS CUTIN: missing or unsupported presentation assets.");
                return null;
            }
            var root = new GameObject("SeraphineAegisCutin", typeof(RectTransform), typeof(Canvas), typeof(GraphicRaycaster));
            var clip = root.AddComponent<SeraphineUltimateCutin>();
            clip._automatic = automatic;
            clip._canvas = root.GetComponent<Canvas>();
            clip._canvas.renderMode = RenderMode.ScreenSpaceOverlay;
            clip._canvas.sortingOrder = 90;
            clip._material = new Material(shader) { name = "Seraphine Aegis instance" };
            clip._material.SetFloat("_Duration", Duration);
            clip._material.SetTexture("_AuraTex", aura);
            // The imported portrait may be resized to a power-of-two rectangle.
            clip._material.SetFloat("_PortraitAspect", 8062f / 6936f);
            var image = new GameObject("Cinematic", typeof(RectTransform)).AddComponent<RawImage>();
            image.transform.SetParent(root.transform, false);
            image.texture = art;
            image.material = clip._material;
            image.raycastTarget = automatic;
            Stretch(image.rectTransform);
            clip.Sample(0f, Screen.width / (float)Mathf.Max(1, Screen.height));
            return clip;
        }

        public void Sample(float seconds, float aspect)
        {
            _age = seconds;
            _material.SetFloat("_Age", seconds);
            _material.SetFloat("_Aspect", Mathf.Max(.1f, aspect));
        }

        void Update()
        {
            if (!_automatic || !Application.isPlaying) return;
            if (_material == null || _canvas == null) { Stop(); return; }
            var keyboard = Keyboard.current;
            if (keyboard != null && keyboard.escapeKey.wasPressedThisFrame) { Stop(); return; }
            Sample(_age + Time.unscaledDeltaTime, Screen.width / (float)Mathf.Max(1, Screen.height));
            if (_age >= Duration) Stop();
        }

        public void Stop()
        {
            ReleaseSimulation();
            if (_canvas != null) _canvas.enabled = false;
            if (Application.isPlaying) Destroy(gameObject);
            else DestroyImmediate(gameObject);
        }

        void OnEnable() { if (_canvas != null) _canvas.enabled = true; }
        void OnDisable()
        {
            ReleaseSimulation();
            if (_canvas != null) _canvas.enabled = false;
        }
        void OnDestroy()
        {
            ReleaseSimulation();
            if (_material == null) return;
            if (Application.isPlaying) Destroy(_material);
            else DestroyImmediate(_material);
        }

        static void Stretch(RectTransform r)
        {
            r.anchorMin = Vector2.zero;
            r.anchorMax = Vector2.one;
            r.offsetMin = r.offsetMax = Vector2.zero;
        }
    }
}
