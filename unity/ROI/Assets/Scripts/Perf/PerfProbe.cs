// Frame-time probe for a STANDALONE build. The Editor shares the machine with the asset watcher,
// the console and the MCP bridge, so its spikes say little about the game — this measures the
// player and writes one line to the player log.
//   ROI.app/Contents/MacOS/ROI -roiperf -logFile /tmp/roi_perf.log
using UnityEngine;

namespace Roi.Perf
{
    public static class PerfProbe
    {
        [RuntimeInitializeOnLoadMethod(RuntimeInitializeLoadType.AfterSceneLoad)]
        static void Install()
        {
            var args = System.Environment.GetCommandLineArgs();
            bool run = false, ui = false, shot = false;
            foreach (var a in args)
            {
                if (a == "-roiperf") run = true;
                if (a == "-roiperfui") ui = true;
                // A SCREENSHOT FROM INSIDE THE PLAYER. Capturing the desktop instead means
                // fighting focus, permission prompts and whatever is in front — and the question
                // being asked ("does the BUILD look like the Editor?") is about the frame the
                // player renders, not about the window manager.
                if (a == "-roishot") shot = true;
            }
            if (shot)
            {
                var sgo = new GameObject("~ShotProbe");
                sgo.AddComponent<Shot>();
                Object.DontDestroyOnLoad(sgo);
            }
            if (!run) return;
            var go = new GameObject("~PerfProbe");
            go.AddComponent<Probe>().OpenScreen = ui;
            Object.DontDestroyOnLoad(go);
        }

        /// Wait for the world to settle, photograph it, and quit. `-roishot` only.
        sealed class Shot : MonoBehaviour
        {
            float _t;
            bool _taken;

            void Update()
            {
                _t += Time.unscaledDeltaTime;
                if (!_taken && _t > 8f)
                {
                    _taken = true;
                    ScreenCapture.CaptureScreenshot("/tmp/roi_player_view.png");
                    Debug.Log("SHOT written to /tmp/roi_player_view.png");
                }
                if (_taken && _t > 11f) Application.Quit();
            }
        }

        sealed class Probe : MonoBehaviour
        {
            const int Warm = 180, Sample = 600;
            /// Measure with a full-screen SCREEN open rather than the fight alone. Combat shows
            /// two strips; a window adds a grid of cells, a scroll view and forty labels, which is
            /// where the UI's cost actually is. A figure taken with everything closed answers a
            /// question nobody asked.
            public bool OpenScreen;
            bool _opened;
            int _frames;
            double _sum, _worst;
            int _over25, _over33;

            void Update()
            {
                _frames++;
                // Opened during the WARM-UP, so the build cost of the window is excluded and what
                // is measured is the steady state of having it up.
                if (OpenScreen && !_opened && _frames == Warm / 2)
                {
                    var bar = Object.FindFirstObjectByType<Roi.UI.HudBar>();
                    if (bar != null) { bar.Toggle("Inventory"); _opened = true; }
                }
                if (_frames <= Warm) return;
                double ms = Time.unscaledDeltaTime * 1000.0;
                _sum += ms;
                if (ms > _worst) _worst = ms;
                if (ms > 25) _over25++;
                if (ms > 33) _over33++;
                if (_frames < Warm + Sample) return;
                double avg = _sum / Sample;
                // WHAT WAS ON SCREEN, reported alongside the number. A frame time that does not
                // say what it measured is not evidence: the first run of this after the UI port
                // returned 343 fps and nothing in the log could confirm the UI had been built at
                // all, which makes the figure unusable for the question that was being asked.
                int canvases = Object.FindObjectsByType<Canvas>(FindObjectsSortMode.None).Length;
                int texts = Object.FindObjectsByType<TMPro.TextMeshProUGUI>(FindObjectsSortMode.None).Length;
                int graphics = Object.FindObjectsByType<UnityEngine.UI.Graphic>(FindObjectsSortMode.None).Length;
                Debug.Log($"ROIPERF avg {avg:0.00}ms ({1000.0 / avg:0} fps) worst {_worst:0}ms | >25ms {_over25}/{Sample} | >33ms {_over33} | {Screen.width}x{Screen.height} vsync={QualitySettings.vSyncCount} | screen={(OpenScreen ? (_opened ? "open" : "FAILED TO OPEN") : "combat")} | canvases={canvases} tmp={texts} graphics={graphics}");
                Application.Quit();
            }
        }
    }
}
