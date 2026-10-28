using System.Collections.Generic;
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    public sealed partial class CompanionWidget
    {
        const float DamageLife = .7f;
        const float DamageRise = 22f;
        const int DamageFontSize = 18;
        static readonly Color EnemyDamage = Color.white;
        static readonly Color AllyDamage = new Color(1f, .45f, .40f);

        sealed class DamagePopup
        {
            public long Id;
            public Vector3 World;
            public string Text;
            public Color Color;
            public float Delay;
            public float Age;
        }

        readonly List<DamagePopup> _damage = new();
        TMP_FontAsset _damageFont;

        public int ActiveDamagePopupCount => _damage.Count;
        public int ActiveDamageLabelCount
        {
            get
            {
                int count = 0;
                foreach (var pane in _panes)
                    foreach (var text in pane.DamageTexts)
                        if (text != null && text.gameObject.activeSelf) count++;
                return count;
            }
        }

        /// The arena feeds the same resolved damage that the full HUD receives. Companion mode
        /// hides the screen-space ArenaHud, so these labels are projected into each live pane's
        /// own camera instead of relying on the disabled main-camera overlay.
        public void SpawnDamage(long id, Vector3 world, long value, bool onPlayer, float delay = 0f)
        {
            if (!isActiveAndEnabled || !Visible || value <= 0) return;
            _damage.Add(new DamagePopup
            {
                Id = id,
                World = world,
                Text = value.ToString(),
                Color = onPlayer ? AllyDamage : EnemyDamage,
                Delay = Mathf.Max(0f, delay),
            });
        }

        void UpdateDamageOverlay()
        {
            for (int i = _damage.Count - 1; i >= 0; i--)
            {
                _damage[i].Age += Time.unscaledDeltaTime;
                if (_damage[i].Age >= _damage[i].Delay + DamageLife) _damage.RemoveAt(i);
            }

            foreach (var pane in _panes)
            {
                if (!pane.Cam.gameObject.activeInHierarchy) continue;
                int cursor = 0;
                foreach (var popup in _damage)
                {
                    float age = popup.Age - popup.Delay;
                    if (age < 0f) continue;
                    var screen = pane.Cam.WorldToViewportPoint(popup.World);
                    if (screen.z <= 0f || screen.x < -.10f || screen.x > 1.10f || screen.y < -.15f || screen.y > 1.15f)
                        continue;

                    var text = DamageText(pane, cursor++);
                    float k = Mathf.Clamp01(age / DamageLife);
                    var color = popup.Color; color.a = 1f - k;
                    text.text = popup.Text;
                    text.color = color;
                    text.rectTransform.anchoredPosition = new Vector2(
                        (screen.x - .5f) * pane.DamageLayer.rect.width,
                        (screen.y - .5f) * pane.DamageLayer.rect.height + DamageRise * k);
                    text.rectTransform.localScale = Vector3.one * Mathf.Lerp(1.35f, 1f, Mathf.Clamp01(age / .14f));
                    text.gameObject.SetActive(true);
                }
                for (int i = cursor; i < pane.DamageTexts.Count; i++) pane.DamageTexts[i].gameObject.SetActive(false);
            }
        }

        TextMeshProUGUI DamageText(Pane pane, int index)
        {
            while (pane.DamageTexts.Count <= index)
            {
                var go = new GameObject("Damage", typeof(RectTransform), typeof(CanvasRenderer));
                go.transform.SetParent(pane.DamageLayer, false);
                var text = go.AddComponent<TextMeshProUGUI>();
                _damageFont ??= Resources.Load<TMP_FontAsset>("Fonts/PixelOperator-Bold SDF") ?? TMP_Settings.defaultFontAsset;
                text.font = _damageFont;
                text.fontSize = DamageFontSize;
                text.alignment = TextAlignmentOptions.Center;
                text.textWrappingMode = TextWrappingModes.NoWrap;
                text.overflowMode = TextOverflowModes.Overflow;
                text.raycastTarget = false;
                text.rectTransform.anchorMin = text.rectTransform.anchorMax = text.rectTransform.pivot = new Vector2(.5f, .5f);
                text.rectTransform.sizeDelta = new Vector2(92f, 30f);
                // TMP's outline setter accesses its private renderer field directly;
                // resolve the lazy property when creating text before TMP's OnEnable.
                if (text.canvasRenderer != null && _damageFont != null && _damageFont.material != null)
                {
                    text.fontSharedMaterial = _damageFont.material;
                    text.outlineWidth = .18f;
                    text.outlineColor = new Color(.035f, .04f, .06f, .95f);
                }
                pane.DamageTexts.Add(text);
            }
            return pane.DamageTexts[index];
        }

        void ClearDamageOverlay()
        {
            _damage.Clear();
            foreach (var pane in _panes)
                foreach (var text in pane.DamageTexts)
                    if (text != null) text.gameObject.SetActive(false);
        }
    }
}
