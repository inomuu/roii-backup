// DESKTOP COMPANION — small camera boxes into the live world, each locked on one party member, so
// the fight can be watched while something else has the screen. Ported from `companion_view.gd`.
//
//   W toggles it · 1 / 2 / 3 pick one, two or three panes · - and = zoom · right-click follows the
//   next member.
//
// ONE CAMERA PER PANE, which is the reason it looks steady: a shared camera framing everyone makes
// every box shake whenever anyone moves. Each pane renders its own member from the arena's own
// angle, so the world reads the same as it does on the main screen.
//
// RENDERED ON A CADENCE, not every frame. The cameras are disabled and driven by hand at 30 Hz —
// the widget is meant to run in a corner for hours, and paying full frame rate for four extra
// views of the same world is exactly what an idle player does not want to spend.
//
// Standalone macOS/Windows use WindowMode for the borderless, always-on-top OS window.
// Inventory/Forge expand that same window above a row of gameplay panes. Editor uses an in-game preview.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed partial class CompanionWidget : MonoBehaviour
    {
        const int PaneW = 320, PaneH = 180, Gap = 6, MaxPanes = 3;
        const float RenderHz = 30f;
        /// How far the pane camera sits from its member, along the arena camera's own direction.
        const float BaseBack = 9f;

        Roi.Arena.ArenaView _arena;
        RectTransform _root;
        readonly List<Pane> _panes = new();
        int _count = 1, _offset;
        int _toggleFrame = -1;
        float _zoom = 1f, _renderAcc;

        sealed class Pane
        {
            public Camera Cam;
            public RenderTexture Tex;
            public RawImage Image;
            public TextMeshProUGUI Label;
            public RectTransform DamageLayer;
            public readonly List<TextMeshProUGUI> DamageTexts = new();
            public long FollowId;
        }

        public static CompanionWidget Create(Transform parent)
        {
            // ITS OWN CANVAS, not a layer of the action bar's. In window mode every other piece of
            // chrome is switched off, and a widget parented under the bar would go off with it.
            var go = new GameObject("Companion");
            go.transform.SetParent(parent, false);
            var canvas = go.AddComponent<Canvas>();
            canvas.renderMode = RenderMode.ScreenSpaceOverlay;
            canvas.sortingOrder = 60;
            canvas.pixelPerfect = true;
            var scaler = go.AddComponent<CanvasScaler>();
            scaler.uiScaleMode = CanvasScaler.ScaleMode.ConstantPixelSize;
            go.AddComponent<GraphicRaycaster>();

            var rootGo = new GameObject("Root", typeof(RectTransform));
            rootGo.transform.SetParent(go.transform, false);
            var w = go.AddComponent<CompanionWidget>();
            w.Build((RectTransform)rootGo.transform);
            go.SetActive(false);
            return w;
        }

        public bool Visible => gameObject.activeSelf;

        /// Chrome that must go away in window mode: in a 320-pixel window there is room for the
        /// panes and nothing else, and this is the same choice `companion_view.gd` makes when it
        /// shrinks the OS window around its boxes.
        readonly List<GameObject> _hidden = new();
        bool _ownsWindow;
        FloatingInventoryHost _floating;
        RectTransform _toolbar;
        Camera _clearCamera;

        public void Toggle()
        {
            // Restoring HudBar can make it observe this same key in the same frame.
            if (_toggleFrame == Time.frameCount) return;
            _toggleFrame = Time.frameCount;
            bool on = !gameObject.activeSelf;
            gameObject.SetActive(on);
            foreach (var p in _panes) p.Cam.gameObject.SetActive(on);
            if (_floating == null)
            {
                _floating = FindFirstObjectByType<FloatingInventoryHost>(FindObjectsInactive.Include);
                if (_floating != null) _floating.LayoutChanged += ManagementLayout;
            }
            _floating?.SetCompanionContext(on);
            if (on) EnterWindow(); else LeaveWindow();
            ManagementLayout();
        }

        void EnterWindow()
        {
            EnsurePanes();
            _ownsWindow = Roi.Platform.WindowMode.Enter(PaneW + 4, _count * PaneH + (_count - 1) * Gap + 40);
            if (!_ownsWindow) { Layout(); return; }
            // The frame comes off only once the window has actually left fullscreen — see
            // `WindowMode.Enter`. Waiting is the whole reason this is two steps.
            StartCoroutine(DecorateWhenReady());
            if (_clearCamera == null)
            {
                var clear = new GameObject("CompanionClear"); clear.transform.SetParent(transform, false);
                _clearCamera = clear.AddComponent<Camera>();
                _clearCamera.clearFlags = CameraClearFlags.SolidColor;
                _clearCamera.backgroundColor = Color.clear;
                _clearCamera.allowHDR = false; // Preserve RGBA in the desktop compositor.
                _clearCamera.cullingMask = 0; _clearCamera.depth = -100;
            }
            // With the arena camera disabled, a real screen clear prevents old UI frames persisting.
            _clearCamera.enabled = true;
            // The world's own camera stops drawing to the screen: the panes ARE the picture now,
            // and a squashed full view behind them is only noise the player pays frames for.
            var arena = _arena != null ? _arena : Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
            if (arena != null && arena.MainCamera != null) arena.MainCamera.enabled = false;
            foreach (var t in new Transform[]
                     {
                         arena != null ? arena.transform.Find("HudBar") : null,
                         arena != null ? arena.transform.Find("ArenaOverlay") : null,
                         arena != null ? arena.transform.Find("ArenaHud") : null,
                         arena != null ? arena.transform.Find("ModeTabs") : null,
                     })
            {
                if (t == null || !t.gameObject.activeSelf) continue;
                t.gameObject.SetActive(false);
                _hidden.Add(t.gameObject);
            }
            Layout();
        }

        System.Collections.IEnumerator DecorateWhenReady()
        {
            for (int i = 0; i < 120 && !Roi.Platform.WindowMode.ReadyToDecorate(); i++)
                yield return null;
            if (!_ownsWindow) yield break;
            Roi.Platform.WindowMode.Decorate();
            yield return null;
            // The ASSERTION this feature needs, and the only one a screenshot cannot replace on a
            // machine with no test harness for other apps' windows: the OS says the window floats.
            Debug.Log("COMPANION decorated — " + Report);
        }

        void LeaveWindow()
        {
            if (!_ownsWindow) return;
            _ownsWindow = false;
            if (_clearCamera != null) _clearCamera.enabled = false;
            Roi.Platform.WindowMode.Restore();
            var arena = _arena != null ? _arena : Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
            if (arena != null && arena.MainCamera != null) arena.MainCamera.enabled = true;
            foreach (var go in _hidden) if (go != null) go.SetActive(true);
            _hidden.Clear();
        }

        void Build(RectTransform root)
        {
            _root = root;
            _toolbar = ForgeUi.Rect(root, "ManagementToolbar", 0, 180, PaneW, 32);
            ForgeUi.Button(_toolbar, "UI_FLOAT_TITLE", 0, 0, 260, 30, () =>
            {
                if (_floating == null) _floating = FindFirstObjectByType<FloatingInventoryHost>(FindObjectsInactive.Include);
                _floating?.Toggle();
            });
            ForgeUi.Button(_toolbar, "UI_FLOAT_CLOSE", 266, 0, 54, 30, Toggle);
            BuildSizeControls(root);
            Layout();
        }

        void EnsurePanes()
        {
            if (_arena == null) _arena = Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
            while (_panes.Count < MaxPanes)
            {
                var box = UiTheme.Box(_root, "Pane" + _panes.Count, UiTheme.Panel, 6);
                UiTheme.Border(box.rectTransform, UiTheme.Line, 6);
                var img = new GameObject("View", typeof(RectTransform)).AddComponent<RawImage>();
                img.transform.SetParent(box.rectTransform, false);
                var ir = img.rectTransform;
                ir.anchorMin = Vector2.zero; ir.anchorMax = Vector2.one;
                ir.offsetMin = new Vector2(2f, 2f); ir.offsetMax = new Vector2(-2f, -2f);

                var tex = new RenderTexture(PaneW * 2, PaneH * 2, 16)
                {
                    name = "CompanionPane",
                    // The widget is small and the world is pixel art at this distance; a point
                    // filter keeps the sprites from smearing when the pane is scaled.
                    filterMode = FilterMode.Point,
                    hideFlags = HideFlags.HideAndDontSave,
                };
                img.texture = tex;

                var camGo = new GameObject("PaneCam" + _panes.Count);
                camGo.transform.SetParent(transform, false);
                var cam = camGo.AddComponent<Camera>();
                var main = _arena != null ? _arena.MainCamera : Camera.main;
                if (main != null) cam.CopyFrom(main);
                cam.targetTexture = tex;
                cam.rect = new Rect(0f, 0f, 1f, 1f);
                // DRIVEN BY HAND: disabled so Unity does not render it every frame, then
                // `Render()` on the cadence below.
                cam.enabled = false;

                var label = UiTheme.Label(box.rectTransform, "Name", "", UiTheme.CaptionSize,
                                          UiTheme.TextHigh, TextAlignmentOptions.BottomLeft);
                var lr = label.rectTransform;
                lr.anchorMin = Vector2.zero; lr.anchorMax = new Vector2(1f, 0f);
                lr.pivot = new Vector2(0.5f, 0f);
                lr.sizeDelta = new Vector2(0f, 18f);
                lr.anchoredPosition = new Vector2(0f, 2f);

                var damageGo = new GameObject("DamageNumbers", typeof(RectTransform));
                damageGo.transform.SetParent(box.rectTransform, false);
                var damageLayer = (RectTransform)damageGo.transform;
                damageLayer.anchorMin = Vector2.zero; damageLayer.anchorMax = Vector2.one;
                damageLayer.offsetMin = Vector2.zero; damageLayer.offsetMax = Vector2.zero;
                damageLayer.pivot = new Vector2(.5f, .5f);
                damageLayer.SetAsLastSibling();

                _panes.Add(new Pane { Cam = cam, Tex = tex, Image = img, Label = label, DamageLayer = damageLayer });
            }
        }

        /// What the last toggle actually got. Read by the scripted door in `ArenaView`, because
        /// window mode is the one feature the Editor cannot check at all.
        public string Report =>
            $"supported={Roi.Platform.WindowMode.Supported} owns={_ownsWindow} "
            + $"floating={Roi.Platform.WindowMode.IsTopmost} "
            + $"size={Screen.width}x{Screen.height} panes={_count} gameScale={_gameScale}x";

        void Update()
        {
            Roi.Platform.WindowMode.SettleAppearance();
            RefreshDisplayBounds();
            var k = UnityEngine.InputSystem.Keyboard.current;
            if (k != null && !UiInputFocus.Typing && !UiInputFocus.Modal)
            {
                // Native mode disables HudBar, which normally owns this shortcut.
                if (_ownsWindow && k.wKey.wasPressedThisFrame && _toggleFrame != Time.frameCount)
                { Toggle(); return; }
                int want = _count;
                if (k.digit1Key.wasPressedThisFrame) want = 1;
                else if (k.digit2Key.wasPressedThisFrame) want = 2;
                else if (k.digit3Key.wasPressedThisFrame) want = 3;
                if (want != _count)
                {
                    _count = want;
                    // Keep the original window restoration state and any open management group.
                    ManagementLayout();
                }
                // INTEGER-ish steps. The world is pixel art and a fractional magnification
                // duplicates pixels unevenly — the Godot widget makes the same point.
                if (k.minusKey.wasPressedThisFrame) _zoom = Mathf.Max(0.5f, _zoom - 0.5f);
                if (k.equalsKey.wasPressedThisFrame) _zoom = Mathf.Min(3f, _zoom + 0.5f);
            }
            var m = UnityEngine.InputSystem.Mouse.current;
            // The camera stays LOCKED on whoever it was given; right-click is the only thing that
            // moves it. Auto-switching to whoever is fighting is what makes a companion view
            // unwatchable.
            if (m != null && m.rightButton.wasPressedThisFrame
                && RectTransformUtility.RectangleContainsScreenPoint(_root, m.position.ReadValue())) _offset++;

            Follow();
            UpdateDamageOverlay();

            _renderAcc += Time.unscaledDeltaTime;
            if (_renderAcc < 1f / RenderHz) return;
            _renderAcc = 0f;
            foreach (var p in _panes)
                if (p.Cam.gameObject.activeSelf) p.Cam.Render();
        }

        void Follow()
        {
            if (_arena == null) _arena = Object.FindFirstObjectByType<Roi.Arena.ArenaView>();
            var main = _arena != null ? _arena.MainCamera : null;
            if (main == null) return;
            var ids = _arena.PlayerIds();
            if (ids.Count == 0) return;

            // The arena's angle, kept: a pane is the same world seen closer, not a new framing.
            var dir = main.transform.forward;
            for (int i = 0; i < _count && i < _panes.Count; i++)
            {
                var p = _panes[i];
                p.FollowId = ids[(i + _offset) % ids.Count];
                var at = _arena.ActorAnchor(p.FollowId);
                if (at == Vector3.zero) continue;
                p.Cam.transform.rotation = main.transform.rotation;
                p.Cam.transform.position = at - dir * (BaseBack / Mathf.Max(0.25f, _zoom));
                p.Cam.fieldOfView = main.fieldOfView;
                var cls = _arena.ActorClass(p.FollowId);
                p.Label.text = string.IsNullOrEmpty(cls) ? $"#{p.FollowId}" : GameText.UnitName(cls);
            }
        }

        void OnDisable() { ClearDamageOverlay(); LeaveWindow(); }

        void OnApplicationQuit() => LeaveWindow();

        void OnDestroy()
        {
            if (_floating != null) _floating.LayoutChanged -= ManagementLayout;
            foreach (var p in _panes)
            {
                if (p.Cam != null) p.Cam.targetTexture = null;
                if (p.Tex != null) { p.Tex.Release(); Destroy(p.Tex); }
            }
        }
    }
}
