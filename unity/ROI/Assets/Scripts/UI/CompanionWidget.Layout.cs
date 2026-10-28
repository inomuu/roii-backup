// Presentation-only companion sizing. Requested scale is independent of camera zoom and UI size.
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed partial class CompanionWidget
    {
        public const string GameScalePreference = "roi.companion.gameScale";
        public const string UiScalePreference = "roi.companion.uiScale";
        const int ToolbarHeight = 68, Margin = 8;
        int _uiScaleIndex = 1;
        RectTransform _uiSizeToolbar;
        readonly Button[] _uiSizeButtons = new Button[3];
        int _gameScale = 2;
        RectTransform _sizeToolbar;
        readonly Button[] _sizeButtons = new Button[4];
        Vector2Int _available;
        bool ManagementOpen => _floating != null && _floating.IsOpen;
        public int GameScale => _gameScale;
        public float UiScale => 1f + _uiScaleIndex * .5f;

        void BuildSizeControls(RectTransform root)
        {
            BuildUiSizeControls(root);
            _gameScale = Mathf.Clamp(PlayerPrefs.GetInt(GameScalePreference, 2), 1, 4);
            _sizeToolbar = ForgeUi.Rect(root, "GameSizeToolbar", 0, 214, PaneW, 30);
            ForgeUi.Copy(_sizeToolbar, "UI_COMPANION_GAME_SIZE", 2, 0, 84, 30, 12);
            for (int i = 0; i < 4; i++)
            {
                int scale = i + 1;
                var button = ForgeUi.Button(_sizeToolbar, "UI_COMPANION_SCALE", 88 + i * 59, 0, 55, 30, () =>
                {
                    _gameScale = scale;
                    PlayerPrefs.SetInt(GameScalePreference, scale);
                    PlayerPrefs.Save();
                    ManagementLayout();
                });
                button.name = "GameScale" + scale;
                button.GetComponentInChildren<TMP_Text>().text = Loc.Fmt("UI_COMPANION_SCALE", scale);
                _sizeButtons[i] = button;
            }
        }

        void BuildUiSizeControls(RectTransform root)
        {
            _uiScaleIndex = Mathf.Clamp(PlayerPrefs.GetInt(UiScalePreference, 1), 0, 2);
            _uiSizeToolbar = ForgeUi.Rect(root, "UiSizeToolbar", 0, 248, PaneW, 30);
            ForgeUi.Copy(_uiSizeToolbar, "UI_COMPANION_UI_SIZE", 2, 0, 84, 30, 12);
            for (int i = 0; i < 3; i++)
            {
                int index = i;
                var button = ForgeUi.Button(_uiSizeToolbar, "UI_COMPANION_SCALE", 88 + i * 79, 0, 74, 30, () =>
                {
                    _uiScaleIndex = index;
                    PlayerPrefs.SetInt(UiScalePreference, index);
                    PlayerPrefs.Save();
                    ManagementLayout();
                });
                button.name = "UiScale" + i;
                button.GetComponentInChildren<TMP_Text>().text = Loc.Fmt("UI_COMPANION_SCALE", 1f + i * .5f);
                _uiSizeButtons[i] = button;
            }
        }

        Vector2Int DisplayBounds()
        {
            if (!Roi.Platform.WindowMode.Supported) return new Vector2Int(Screen.width, Screen.height);
            var area = Screen.mainWindowDisplayInfo.workArea;
            // Leave a margin outside the native window; workArea already excludes the taskbar/Dock.
            return new Vector2Int(Mathf.Max(320, area.width - 24), Mathf.Max(220, area.height - 24));
        }
        void RefreshDisplayBounds()
        {
            var available = DisplayBounds();
            if (_available != available) ManagementLayout();
        }

        void Layout()
        {
            EnsurePanes();
            _available = DisplayBounds();
            bool management = ManagementOpen;
            int uiWidth = management ? _floating.WindowWidth : 0;
            int toolbarHeight = ToolbarHeight + (management ? 34 : 0);
            float uiScale = management ? Mathf.Min(UiScale,
                (_available.x - Margin * 2f) / uiWidth,
                Mathf.Max(.1f, (_available.y - toolbarHeight - 10 - Margin * 2 - 90f) / FloatingInventoryHost.Height)) : 1;
            int uiHeight = management ? Mathf.CeilToInt(FloatingInventoryHost.Height * uiScale) + 10 : 0;
            int gaps = (_count - 1) * Gap;
            float fitWidth = (_available.x - Margin * 2 - (management ? gaps : 0f)) / (PaneW * (management ? _count : 1f));
            float fitHeight = (_available.y - Margin * 2 - toolbarHeight - uiHeight - (management ? 0 : gaps)) / (PaneH * (management ? 1f : _count));
            float scale = Mathf.Max(.1f, Mathf.Min(_gameScale, fitWidth, fitHeight));
            int pw = Mathf.Max(1, Mathf.FloorToInt(PaneW * scale)), ph = Mathf.Max(1, Mathf.FloorToInt(PaneH * scale));
            int w = management ? _count * pw + gaps : pw;
            int h = management ? ph : _count * ph + gaps;
            int windowW = Mathf.Max(Mathf.CeilToInt(uiWidth * uiScale), Mathf.Max(PaneW, w)) + Margin * 2;
            int windowH = uiHeight + h + toolbarHeight + Margin * 2;
            _root.sizeDelta = new Vector2(Mathf.Max(PaneW, w), h + toolbarHeight);
            ForgeUi.Place(_toolbar, (_root.rect.width - PaneW) * .5f, h + 3, PaneW, 30);
            if (_sizeToolbar != null) ForgeUi.Place(_sizeToolbar, (_root.rect.width - PaneW) * .5f, h + 37, PaneW, 30);
            if (_uiSizeToolbar != null)
            {
                _uiSizeToolbar.gameObject.SetActive(management);
                ForgeUi.Place(_uiSizeToolbar, (_root.rect.width - PaneW) * .5f, h + 71, PaneW, 30);
            }
            for (int i = 0; i < _uiSizeButtons.Length; i++)
                if (_uiSizeButtons[i] != null) _uiSizeButtons[i].targetGraphic.color = i == _uiScaleIndex ? new Color(.40f, .34f, .22f) : ForgeUi.Raised;
            for (int i = 0; i < _sizeButtons.Length; i++)
                if (_sizeButtons[i] != null) _sizeButtons[i].targetGraphic.color = i + 1 == _gameScale ? new Color(.40f, .34f, .22f) : ForgeUi.Raised;
            if (management || _ownsWindow)
            {
                _root.anchorMin = _root.anchorMax = new Vector2(.5f, .5f);
                _root.pivot = new Vector2(.5f, 0f);
                _root.anchoredPosition = new Vector2(0, -windowH * .5f + Margin);
            }
            else
            {
                _root.anchorMin = _root.anchorMax = new Vector2(1f, 0f);
                _root.pivot = new Vector2(1f, 0f);
                _root.anchoredPosition = new Vector2(-UiTheme.Pad, UiLayout.BottomBar + UiTheme.Pad);
            }
            for (int i = 0; i < _panes.Count; i++)
            {
                var pane = _panes[i];
                var box = (RectTransform)pane.Image.transform.parent;
                bool on = i < _count;
                box.gameObject.SetActive(on);
                pane.Cam.gameObject.SetActive(on && gameObject.activeSelf);
                if (!on) continue;
                box.anchorMin = box.anchorMax = box.pivot = new Vector2(0, 1);
                box.sizeDelta = new Vector2(pw, ph);
                box.anchoredPosition = management ? new Vector2((_root.rect.width - w) * .5f + i * (pw + Gap), 0) : new Vector2((_root.rect.width - w) * .5f, -i * (ph + Gap));
                ResizeTexture(pane, pw - 4, ph - 4);
            }
            if (management) _floating.SetCompanionLayout(windowH, uiScale);
            if (_ownsWindow) Roi.Platform.WindowMode.Resize(windowW, windowH);
        }

        static void ResizeTexture(Pane pane, int width, int height)
        {
            // Match physical display pixels. UI scaling must not multiply world rendering by 4.
            int tw = Mathf.Max(1, width), th = Mathf.Max(1, height);
            if (pane.Tex.width != tw || pane.Tex.height != th)
            {
                pane.Cam.targetTexture = null;
                pane.Image.texture = null;
                pane.Tex.Release();
                Destroy(pane.Tex);
                pane.Tex = new RenderTexture(tw, th, 16)
                {
                    name = "CompanionPane", filterMode = FilterMode.Point,
                    hideFlags = HideFlags.HideAndDontSave,
                };
                pane.Image.texture = pane.Tex;
                pane.Cam.targetTexture = pane.Tex;
            }
            pane.Cam.aspect = (float)tw / th;
        }
        void ManagementLayout() { if (Visible) Layout(); }
    }
}
