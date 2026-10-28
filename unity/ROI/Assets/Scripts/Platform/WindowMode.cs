// THE PLAYER'S WINDOW — floating above other apps, borderless, moved and resized. The two things
// Unity does not expose on any platform, and the companion widget's whole reason for existing: it
// is meant to sit in a corner of the desktop while the player works, not to be a small picture
// inside a full-size game window.
//
// TWO ROUTES, because the platforms differ in kind:
//   WINDOWS — `user32.dll` is a system library, so a P/Invoke reaches it directly and there is no
//             binary of ours in the build to sign.
//   macOS   — AppKit cannot be called from C#, so `Plugins/macOS/roi_window.bundle` carries the native
//             functions. We compile it from `native/macos/roi_window.m`
//             (`tools/build_mac_plugin.sh`) rather than vendoring someone's prebuilt one: a
//             third-party binary would have to be re-signed with our Team ID before notarisation,
//             and a step like that is missed once and found at submission.
//
// IN THE EDITOR IT DOES NOTHING, deliberately. The window it would touch is the EDITOR's, not a
// player's — resizing or floating that is how a plugin ends up fighting the person using it (the
// known failure of at least one asset in this space). Companion mode still works there; only the
// window part is skipped.
//
// EVERY ENTRY RESTORES WHAT IT FOUND. `Enter` records the size, position and floating state before
// changing anything, and `Restore` puts them back — a mode that leaves the window borderless
// because the player alt-F4'd out of it is a mode nobody turns on twice.
using System;
using System.Runtime.InteropServices;
using UnityEngine;

namespace Roi.Platform
{
    public static class WindowMode
    {
        /// True where the calls actually reach a window: a standalone player on Windows or macOS.
        public static bool Supported =>
#if UNITY_EDITOR
            false;
#elif UNITY_STANDALONE_WIN || UNITY_STANDALONE_OSX
            true;
#else
            false;
#endif

        /// What the OS says right now — not what we asked for. The companion widget logs this
        /// after decorating, which is the only self-check available for "it floats above other
        /// apps" on a machine that cannot script another app's windows.
        public static bool IsTopmost => GetTopmost();
        public static bool IsTransparent => GetTransparent();

        static bool _entered;
        static int _savedW, _savedH, _savedX, _savedY;
        static bool _savedTopmost;
        static FullScreenMode _savedMode;

        /// Shrink to a floating, borderless window of this size, in the bottom-right corner.
        /// Answers false when the platform cannot do it — the caller then keeps its in-window
        /// presentation, which is what the Editor and any other platform get.
        public static bool Enter(int width, int height)
        {
            if (!Supported || _entered) return _entered;
            _savedW = Screen.width;
            _savedH = Screen.height;
            _savedMode = Screen.fullScreenMode;
            _savedTopmost = GetTopmost();
            GetPosition(out _savedX, out _savedY);

            // WINDOWED FIRST, and that is only a REQUEST: Unity applies a resolution change at
            // the end of a frame, so the window is still fullscreen for a moment afterwards. On
            // macOS a style-mask change on a fullscreen window raises an uncaught exception and
            // takes the process with it — which is exactly how the first version of this died. So
            // the decoration is a second step the caller makes once the mode has settled.
            _pendingW = width; _pendingH = height;
            Screen.SetResolution(width, height, FullScreenMode.Windowed);
            _entered = true;
            return true;
        }

        public static void Resize(int width, int height)
        {
            if (!_entered || (_pendingW == width && _pendingH == height)) return;
            _pendingW = width; _pendingH = height;
            Screen.SetResolution(width, height, FullScreenMode.Windowed);
            if (ReadyToDecorate()) Decorate();
        }

        static int _pendingW, _pendingH;
        static float _appearanceUntil;
        public static void SettleAppearance()
        {
            // Unity rebuilds the native content layer asynchronously after SetResolution.
            if (_entered && Time.unscaledTime < _appearanceUntil && ReadyToDecorate())
            {
                SetTransparent(true);
#if !UNITY_EDITOR && UNITY_STANDALONE_OSX
                roi_window_place_in_corner(); // Use the settled native size, in AppKit points.
#endif
            }
        }

        /// Ready for the decoration? False while the window is still in a fullscreen transition.
        public static bool ReadyToDecorate()
        {
#if !UNITY_EDITOR && UNITY_STANDALONE_OSX
            return roi_window_is_fullscreen() == 0;
#else
            return !Screen.fullScreen;
#endif
        }

        /// The second half of `Enter`: strip the frame, float it, and put it in the corner. Safe to
        /// call more than once, and a no-op if `Enter` was refused.
        public static void Decorate()
        {
            if (!_entered) return;
            SetBorderless(true);
            SetTransparent(true);
            _appearanceUntil = Time.unscaledTime + 1f;
            SetTopmost(true);
#if !UNITY_EDITOR && UNITY_STANDALONE_OSX
            roi_window_place_in_corner();
#else
            var res = Screen.currentResolution;
            SetPosition(Mathf.Max(0, res.width - _pendingW - 24),
                        Mathf.Max(0, res.height - _pendingH - 64));
#endif
        }

        /// Put back exactly what `Enter` found.
        public static void Restore()
        {
            if (!_entered) return;
            _entered = false;
            SetTransparent(false);
            SetTopmost(_savedTopmost);
            SetBorderless(false);
            Screen.SetResolution(_savedW, _savedH, _savedMode);
            SetPosition(_savedX, _savedY);
        }

        // --- the platform halves -----------------------------------------------------------------

#if !UNITY_EDITOR && UNITY_STANDALONE_WIN
        const int SwpNoSize = 0x0001, SwpNoMove = 0x0002, SwpNoActivate = 0x0010;
        static readonly IntPtr HwndTopmost = new IntPtr(-1), HwndNoTopmost = new IntPtr(-2);
        const int GwlStyle = -16;
        const uint WsPopup = 0x80000000, WsVisible = 0x10000000, WsOverlappedWindow = 0x00CF0000;

        [DllImport("user32.dll")] static extern IntPtr GetActiveWindow();
        [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr hWnd, IntPtr insertAfter,
            int x, int y, int cx, int cy, uint flags);
        [DllImport("user32.dll", SetLastError = true)] static extern int SetWindowLong(IntPtr hWnd, int index, uint value);
        [DllImport("user32.dll", SetLastError = true)] static extern uint GetWindowLong(IntPtr hWnd, int index);
        [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
        [StructLayout(LayoutKind.Sequential)] struct RECT { public int Left, Top, Right, Bottom; }

        [StructLayout(LayoutKind.Sequential)] struct Margins { public int Left, Right, Top, Bottom; }
        [DllImport("dwmapi.dll")] static extern int DwmExtendFrameIntoClientArea(IntPtr hwnd, ref Margins margins);
        static bool _transparent;
        static void SetTransparent(bool on)
        {
            var margins = on ? new Margins { Left=-1, Right=-1, Top=-1, Bottom=-1 } : new Margins();
            if (DwmExtendFrameIntoClientArea(GetActiveWindow(), ref margins) == 0) _transparent=on;
        }
        static bool GetTransparent() => _transparent;
        static void SetTopmost(bool on) => SetWindowPos(GetActiveWindow(),
            on ? HwndTopmost : HwndNoTopmost, 0, 0, 0, 0, SwpNoMove | SwpNoSize | SwpNoActivate);

        static bool GetTopmost() => false;   // WS_EX_TOPMOST could be read, but Enter is the only
                                             // thing that sets it, so "was normal" is the truth.

        static void SetBorderless(bool on)
        {
            var h = GetActiveWindow();
            SetWindowLong(h, GwlStyle, on ? (WsPopup | WsVisible) : (WsOverlappedWindow | WsVisible));
            // The frame is cached until a SetWindowPos tells the window to recompute it.
            SetWindowPos(h, IntPtr.Zero, 0, 0, 0, 0, SwpNoMove | SwpNoSize | SwpNoActivate | 0x0020);
        }

        static void GetPosition(out int x, out int y)
        {
            x = 0; y = 0;
            if (GetWindowRect(GetActiveWindow(), out var r)) { x = r.Left; y = r.Top; }
        }

        static void SetPosition(int x, int y) => SetWindowPos(GetActiveWindow(), IntPtr.Zero,
            x, y, 0, 0, SwpNoSize | SwpNoActivate);

#elif !UNITY_EDITOR && UNITY_STANDALONE_OSX
        [DllImport("roi_window")] static extern int roi_window_set_topmost(int on);
        [DllImport("roi_window")] static extern int roi_window_set_borderless(int on);
        [DllImport("roi_window")] static extern int roi_window_get_topmost();
        [DllImport("roi_window")] static extern int roi_window_is_fullscreen();
        [DllImport("roi_window")] static extern int roi_window_set_transparent(int on);
        [DllImport("roi_window")] static extern int roi_window_get_transparent();
        static void SetTransparent(bool on) => roi_window_set_transparent(on ? 1 : 0);
        static bool GetTransparent() => roi_window_get_transparent() != 0;

        static void SetTopmost(bool on) => roi_window_set_topmost(on ? 1 : 0);
        static bool GetTopmost() => roi_window_get_topmost() != 0;
        static void SetBorderless(bool on) => roi_window_set_borderless(on ? 1 : 0);
        // Synchronous AppKit movement avoids leaving Unity MoveMainWindowTo AsyncOperations
        // alive during player shutdown (observed Mono finalizer crash after repeated resizes).
        // Both capture and restore use the native top-left in desktop points, including Retina.
        [DllImport("roi_window")] static extern int roi_window_get_position(out int x, out int y);
        [DllImport("roi_window")] static extern int roi_window_set_position(int x, int y);
        [DllImport("roi_window")] static extern int roi_window_place_in_corner();
        static void GetPosition(out int x, out int y) => roi_window_get_position(out x, out y);
        static void SetPosition(int x, int y) => roi_window_set_position(x, y);

#else
        static void SetTransparent(bool on) { }
        static bool GetTransparent() => false;
        static void SetTopmost(bool on) { }
        static bool GetTopmost() => false;
        static void SetBorderless(bool on) { }
        static void GetPosition(out int x, out int y) { x = 0; y = 0; }
        static void SetPosition(int x, int y) { }
#endif
    }
}
