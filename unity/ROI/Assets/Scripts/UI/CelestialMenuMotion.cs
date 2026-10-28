using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Reusable material animation for a menu background; no global time, lighting or portrait changes.
    [RequireComponent(typeof(RawImage))]
    public sealed class CelestialMenuMotion : MonoBehaviour
    {
        [SerializeField, Range(0, 2)] float motionSpeed = .7f;
        [SerializeField, Range(0, 1)] float starStrength = .5f;
        [SerializeField, Range(0, 1)] float hazeStrength = .4f;
        RawImage _image;
        Material _material, _previous;
        float _time;
        static readonly int MotionTime = Shader.PropertyToID("_MotionTime");
        static readonly int Aspect = Shader.PropertyToID("_Aspect");
        static readonly int Stars = Shader.PropertyToID("_StarStrength");
        static readonly int Haze = Shader.PropertyToID("_HazeStrength");

        void OnEnable()
        {
            _image = GetComponent<RawImage>();
            if (_material == null)
            {
                var shader = Resources.Load<Shader>("ExemplarUI/CelestialMotion");
                if (shader == null || !shader.isSupported) return;
                _material = new Material(shader) { name = "Celestial Menu Motion (Instance)", hideFlags = HideFlags.HideAndDontSave };
            }
            _previous = _image.material;
            _image.material = _material;
            Apply();
        }
        void Update()
        {
            _time += Time.unscaledDeltaTime * motionSpeed;
        }
        void LateUpdate() => Apply();
        void Apply()
        {
            if (_material == null) return;
            var size = _image.rectTransform.rect.size;
            var uv = _image.uvRect;
            float aspect = size.y > 0 && Mathf.Abs(uv.width) > .0001f
                ? size.x / size.y * Mathf.Abs(uv.height / uv.width) : 16f / 9f;
            SetProperties(_material, aspect);
            // uGUI may render a stencil clone when this reusable backdrop sits under a Mask.
            var rendered = _image.materialForRendering;
            if (rendered != null && rendered != _material) SetProperties(rendered, aspect);
        }
        void SetProperties(Material material, float aspect)
        {
            material.SetFloat(MotionTime, _time);
            material.SetFloat(Aspect, aspect);
            material.SetFloat(Stars, starStrength);
            material.SetFloat(Haze, hazeStrength);
        }
        void OnDisable()
        {
            if (_image != null && _image.material == _material) _image.material = _previous;
        }
        void OnDestroy()
        {
            if (_material != null) Destroy(_material);
        }
    }
}
