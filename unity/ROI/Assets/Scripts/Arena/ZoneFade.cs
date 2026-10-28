// A DIP TO BLACK between bands. Ported from views/transition_view.gd.
//
// A plain alpha fade, no pattern — the diamond wipe the Godot file first had read as eye-straining
// and it says so. It sits UNDER the UI (sorting order 9, below the window canvas at 10) so a panel
// stays visible and usable through the fade, and it never takes input: a transient veil that ate a
// click would be a veil that broke the game for a second and a half.
//
// IT FIRES ONLY AT A BAND BOUNDARY, not at every room. Within a band the arena scrolls
// continuously room to room, so a veil there would hide the very march that replaced it. The sim
// raises `zone_cleared` about 2.5 s before it swaps rooms, which is the window this fades in.
//
// THE FIRST STATE OF A SESSION SEEDS THE EDGE AND DOES NOTHING. A save taken inside a band-end
// clear window resumes with `zone_cleared` already true, and treating that as a rising edge faded
// the screen to black on login — a pitch-black "hang" until the sim's clear-hold elapsed. Unknown
// to true is not an edge.
using UnityEngine;
using UnityEngine.UI;

namespace Roi.Arena
{
    public sealed class ZoneFade : MonoBehaviour
    {
        const float OutDelay = 1.0f, OutDur = 1.3f, InHold = 0.05f, InDur = 1.0f;

        Image _veil;
        float _from, _to, _t, _delay, _dur;
        bool _running;

        bool _seeded, _prevCleared, _veilUp;
        string _prevZone = "";

        public static ZoneFade Create(Transform parent)
        {
            var go = new GameObject("ZoneFade");
            go.transform.SetParent(parent, false);
            var canvas = go.AddComponent<Canvas>();
            canvas.renderMode = RenderMode.ScreenSpaceOverlay;
            // Above the world, BELOW the windows: see the layer table — windows 10, popups 20.
            canvas.sortingOrder = 9;
            var f = go.AddComponent<ZoneFade>();
            f._veil = Roi.UI.UiTheme.Flat(go.transform, "Veil", new Color(0.02f, 0.02f, 0.04f, 0f));
            f._veil.raycastTarget = false;
            var rt = f._veil.rectTransform;
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = Vector2.zero; rt.offsetMax = Vector2.zero;
            return f;
        }

        public void OnView(string json)
        {
            var v = JsonUtility.FromJson<FadeView>(json);
            if (v == null) return;
            bool cleared = v.zone_spawn != null && v.zone_spawn.zone_cleared;
            bool bandEnd = v.zone_spawn != null && v.zone_spawn.is_instance_end;
            var zone = v.current_zone_id ?? "";

            if (!_seeded)
            {
                _seeded = true;
                _prevCleared = cleared;
                _prevZone = zone;
                return;
            }
            if (cleared && !_prevCleared && bandEnd) { Run(1f, OutDelay, OutDur); _veilUp = true; }
            _prevCleared = cleared;
            // A begun fade reveals on the NEXT swap whatever the destination is, so the veil can
            // never stick black.
            if (zone != _prevZone)
            {
                if (_veilUp) { Run(0f, InHold, InDur); _veilUp = false; }
                _prevZone = zone;
            }
        }

        /// Drop a raised veil at once — a party wipe during the clear window means no swap is
        /// coming, and a game-over behind black is a game-over nobody can see.
        public void Abort()
        {
            if (!_veilUp && !_running) return;
            _veilUp = false;
            Run(0f, 0f, 0.25f);
        }

        void Run(float target, float delay, float dur)
        {
            _from = _veil.color.a; _to = target; _delay = delay; _dur = Mathf.Max(0.01f, dur);
            _t = 0f; _running = true;
        }

        void Update()
        {
            if (!_running) return;
            _t += Time.unscaledDeltaTime;
            if (_t < _delay) return;
            float k = Mathf.Clamp01((_t - _delay) / _dur);
            // Sine ease in-out, as the Godot tween is set: gentle, with no hard edge at either end.
            float e = 0.5f - 0.5f * Mathf.Cos(k * Mathf.PI);
            var c = _veil.color;
            c.a = Mathf.Lerp(_from, _to, e);
            _veil.color = c;
            if (k >= 1f) _running = false;
        }

        /// How dark it is right now, for a probe — nothing else can see an alpha.
        public float Alpha => _veil != null ? _veil.color.a : 0f;

        [System.Serializable] class FadeView { public string current_zone_id; public SpawnDTO zone_spawn; }
        [System.Serializable] class SpawnDTO { public bool zone_cleared, is_instance_end; }
    }
}
