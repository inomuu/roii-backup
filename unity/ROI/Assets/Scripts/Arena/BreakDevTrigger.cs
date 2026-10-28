// Dev keys for the break presentation, so it can be judged by pressing something rather than by
// waiting for the fight to produce one:
//
//   B   the BOSS break — full-size word, camera push, sim held
//   N   the ordinary one, for comparing the two sizes back to back
//   V   step to the next easing on the camera punch AND fire it, so a list of easings can be
//       compared by feel in a few seconds rather than by one recompile each
//
// EDITOR ONLY, and it installs itself the same way `CutinDevTrigger` does — a hidden object added
// after the scene loads — because a dev key is not a game event and must not exist in a build.
// The real trigger is the sim's own `break` packet; both go through `ArenaView.PlayBreak`, so what
// a key shows is what a fight shows.
#if UNITY_EDITOR
using UnityEngine;
using UnityEngine.InputSystem;

namespace Roi.Arena
{
    public static class BreakDevTrigger
    {
        [RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.AfterSceneLoad)]
        static void Install()
        {
            var go = new GameObject("~BreakDevTrigger") { hideFlags = HideFlags.HideAndDontSave };
            go.AddComponent<Listener>();
            Object.DontDestroyOnLoad(go);
        }

        sealed class Listener : MonoBehaviour
        {
            void Update()
            {
                var k = Keyboard.current;
                if (k == null) return;
                bool boss = k.bKey.wasPressedThisFrame;
                bool cycle = k.vKey.wasPressedThisFrame;
                if (!boss && !cycle && !k.nKey.wasPressedThisFrame) return;
                var view = FindFirstObjectByType<ArenaView>();
                if (view == null) return;
#if ROI_DOTWEEN
                if (cycle) { boss = true; Debug.Log("BEAT EASE: " + view.CycleFocusEase()); }
#else
                if (cycle) { Debug.Log("BEAT EASE: needs DOTween"); return; }
#endif
                if (!view.DebugBreak(boss)) Debug.Log("BREAKTEST: no living monster on screen");
            }
        }
    }
}
#endif
