using UnityEngine;

namespace Roi.UI
{
    // Finite presentation motion: no animator, idle updates, or transaction callbacks.
    [RequireComponent(typeof(CanvasGroup))]
    public sealed class UiEntrance : MonoBehaviour
    {
        public const string ReducedMotionKey = "roi.ui.reduced_motion";
        CanvasGroup _group;
        RectTransform _rect;
        Vector2 _position;
        Vector3 _scale;
        float _elapsed, _started, _delay, _duration, _distance;
        bool _captured;
        public bool Complete { get; private set; } = true;
        public static bool MotionAllowed => PlayerPrefs.GetInt(ReducedMotionKey, 0) == 0
            && !(Object.FindFirstObjectByType<CompanionWidget>()?.Visible ?? false);

        public static UiEntrance Play(RectTransform target, float delay = 0, float duration = .20f, float distance = 6)
        {
            var motion = target.GetComponent<UiEntrance>() ?? target.gameObject.AddComponent<UiEntrance>();
            motion.Finish();
            motion._rect = target;
            motion._position = target.anchoredPosition;
            motion._scale = target.localScale;
            motion._group = target.GetComponent<CanvasGroup>();
            motion._captured = true;
            motion._elapsed = 0; motion._started = Time.unscaledTime; motion._delay = delay; motion._duration = duration; motion._distance = distance;
            if (!MotionAllowed) { motion.Finish(); return motion; }
            motion.Complete = false;
            motion._group.alpha = 0;
            motion.enabled = true;
            return motion;
        }
        void Update()
        {
            if (!MotionAllowed) { Finish(); return; }
            _elapsed = Time.unscaledTime - _started;
            float t = Mathf.Clamp01((_elapsed - _delay) / _duration);
            float ease = 1 - Mathf.Pow(1-t, 3);
            _group.alpha = Mathf.Clamp01(t * 2);
            _rect.anchoredPosition = _position + Vector2.down * (_distance * (1-ease));
            _rect.localScale = _scale * Mathf.Lerp(.995f, 1, ease);
            if (t >= 1) Finish();
        }
        public void Finish()
        {
            if (_captured)
            {
                _group.alpha = 1;
                _rect.anchoredPosition = _position;
                _rect.localScale = _scale;
            }
            Complete = true; enabled = false;
        }
        void OnDisable() { if (_captured && !Complete) Finish(); }
    }
}
