// HOLDS THE HIT FLASH ON, from LateUpdate, for looking at a bloom setting.
//
// It has to be a component and it has to be LateUpdate: `ArenaView.TickJuice` writes `_Flash` into
// every actor's property block on its own Update, from the actor's own timer, which is zero when
// nothing has been hit. Anything setting the flash from `EditorApplication.update` is overwritten
// before the frame draws — the slimes simply keep looking like slimes, which is exactly what
// happened.
//
// Dev-only in practice: nothing adds this except an editor script.
using UnityEngine;

namespace Roi.Arena
{
    public sealed class FlashHold : MonoBehaviour
    {
        /// One value per sprite, left to right on screen. Kept public so an editor script can set
        /// the ladder without this file knowing what is being compared.
        public float[] Values = { 1.0f, 2.2f, 5.0f };

        MaterialPropertyBlock _mpb;

        void LateUpdate()
        {
            _mpb ??= new MaterialPropertyBlock();
            var lit = new System.Collections.Generic.List<SpriteRenderer>();
            foreach (var r in FindObjectsByType<SpriteRenderer>(FindObjectsSortMode.None))
                if (r.sharedMaterial != null && r.sharedMaterial.HasProperty("_Flash")
                    && r.gameObject.activeInHierarchy)
                    lit.Add(r);
            if (lit.Count == 0) return;
            lit.Sort((a, b) => a.transform.position.x.CompareTo(b.transform.position.x));

            int n = Mathf.Min(Values.Length, lit.Count);
            for (int i = 0; i < n; i++)
            {
                lit[i].GetPropertyBlock(_mpb);
                _mpb.SetFloat("_Flash", 1f);
                _mpb.SetColor("_FlashColor", new Color(Values[i], Values[i], Values[i], 1f));
                lit[i].SetPropertyBlock(_mpb);
            }
        }
    }
}
