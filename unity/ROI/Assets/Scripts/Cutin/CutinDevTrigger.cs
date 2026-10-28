// Dev-only in-game trigger for the cut-in: press J while playing, the same key Godot's
// cutin_view.gd uses. EDITOR ONLY on purpose — the shipping trigger is a real game event
// (a Commander skill / a boss), never a key, and it is not wired yet.
#if UNITY_EDITOR
using UnityEditor;
using UnityEngine;
using UnityEngine.InputSystem;

namespace Roi.Cutin
{
    public static class CutinDevTrigger
    {
        const string Model = "Assets/Live2D/TestModels/67/67.prefab";
        const string Motion = "Assets/Live2D/TestModels/67/67_loop.controller";

        [RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.AfterSceneLoad)]
        static void Install()
        {
            var go = new GameObject("~CutinDevTrigger") { hideFlags = HideFlags.HideAndDontSave };
            go.AddComponent<Listener>();
            Object.DontDestroyOnLoad(go);
        }

        /// Start the test cut-in and hand back the instance, or null if it could not.
        ///
        /// The model is loaded through `AssetDatabase`, which is why this whole file is Editor-only
        /// and why a boss break calls it behind `#if UNITY_EDITOR`. Shipping this means the cut-in
        /// assets move to Resources or Addressables first — a real decision about build size, not
        /// something to slip in behind a break.
        public static Live2DCutin Play()
        {
            if (Object.FindFirstObjectByType<Live2DCutin>() != null) return null;   // one at a time
            var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(Model);
            if (prefab == null) { Debug.LogWarning("CUTIN: no model at " + Model); return null; }
            return Live2DCutin.Play(prefab, AssetDatabase.LoadAssetAtPath<RuntimeAnimatorController>(Motion));
        }

        sealed class Listener : MonoBehaviour
        {
            void Update()
            {
                var k = Keyboard.current;
                if (k == null || !k.jKey.wasPressedThisFrame) return;
                Play();
            }
        }
    }
}
#endif
