// THE SCREEN A FINISHED MISSION ENDS ON: what it was, what it cost, what it pays. Ported from
// views/story_reward_view.gd.
//
// IT GRANTS NOTHING. The reward is a server call keyed on (account, mission) and the mission's
// owner makes it; all this does is show the line the writer put in `% reward_text`, report what
// the run cost, and get out of the way.
//
// THE CLAIM IS A BUTTON, and that is the whole design of this panel rather than an accident of it.
// It used to fire the moment the panel appeared, which made a dropped connection at that exact
// second cost the reward permanently — nothing re-fires it, and the panel appears once per run. A
// button the player presses is also a button they can press AGAIN, and the server's claim row
// makes a second press free: it either pays the first time or says it already did.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.Story
{
    public sealed class StoryReward : MonoBehaviour
    {
        const float W = 460f;

        /// The player dismissed it. Whoever opened it decides what comes next.
        public event System.Action Closed;
        /// The player asked for the payout. This panel never grants anything itself — it asks, and
        /// the owner reports back through `SetRewardLine` / `Settled`.
        public event System.Action ClaimRequested;

        RectTransform _panel, _rows;
        readonly List<GameObject> _built = new();
        TextMeshProUGUI _rewardLine;
        Button _claim;
        float _y;

        public bool IsOpen { get; private set; }

        public static StoryReward Create(Transform parent)
        {
            var go = new GameObject("StoryReward");
            go.transform.SetParent(parent, false);
            var r = go.AddComponent<StoryReward>();
            r.Build();
            r.gameObject.SetActive(false);
            return r;
        }

        void Build()
        {
            var canvas = gameObject.AddComponent<Canvas>();
            canvas.renderMode = RenderMode.ScreenSpaceOverlay;
            // Above the story menu, below tooltips and the bar — the band `UiWindow` documents.
            canvas.sortingOrder = 20;
            gameObject.AddComponent<GraphicRaycaster>();
            var scaler = gameObject.AddComponent<CanvasScaler>();
            scaler.uiScaleMode = CanvasScaler.ScaleMode.ScaleWithScreenSize;
            scaler.referenceResolution = new Vector2(1280f, 720f);
            scaler.matchWidthOrHeight = 1f;

            var root = (RectTransform)transform;
            var scrim = Roi.UI.UiTheme.Flat(root, "Scrim", Roi.UI.UiTheme.Scrim);
            var sr = scrim.rectTransform;
            sr.anchorMin = Vector2.zero; sr.anchorMax = Vector2.one;
            sr.offsetMin = Vector2.zero; sr.offsetMax = Vector2.zero;
            // It BLOCKS. The mission list is underneath and pressing it mid-summary would start a
            // second mission over the one that just ended.
            var block = scrim.gameObject.AddComponent<Button>();
            block.targetGraphic = scrim;
            block.transition = Selectable.Transition.None;

            var box = Roi.UI.UiTheme.Box(root, "Panel", Roi.UI.UiTheme.Panel, 10);
            Roi.UI.UiTheme.Border(box.rectTransform, Roi.UI.UiTheme.Accent, 10);
            _panel = box.rectTransform;
            _panel.anchorMin = _panel.anchorMax = new Vector2(0.5f, 0.5f);
            _panel.pivot = new Vector2(0.5f, 0.5f);
            _panel.sizeDelta = new Vector2(W, 300f);

            var rowsGo = new GameObject("Rows", typeof(RectTransform));
            rowsGo.transform.SetParent(_panel, false);
            _rows = (RectTransform)rowsGo.transform;
            _rows.anchorMin = new Vector2(0f, 1f); _rows.anchorMax = new Vector2(1f, 1f);
            _rows.pivot = new Vector2(0.5f, 1f);
            _rows.offsetMin = new Vector2(Roi.UI.UiTheme.PadLarge, 0f);
            _rows.offsetMax = new Vector2(-Roi.UI.UiTheme.PadLarge, 0f);
        }

        /// `stats` is label → value, in the order it should read. Rebuilt per mission because the
        /// panel appears once per run and holding a pooled layout for that is false economy.
        public void ShowResult(string titleKey, string rewardKey, IList<KeyValuePair<string, string>> stats)
        {
            foreach (var go in _built) Destroy(go);
            _built.Clear();
            _y = Roi.UI.UiTheme.PadLarge;

            Line(Roi.UI.Loc.Get("UI_STORY_CLEARED"), 24f, Roi.UI.UiTheme.Accent, 30f);
            if (!string.IsNullOrEmpty(titleKey))
                Line(Roi.UI.Loc.GetOr(titleKey, titleKey), 17f, Roi.UI.UiTheme.TextHigh, 24f);

            if (stats != null && stats.Count > 0)
            {
                Rule();
                foreach (var kv in stats) Pair(kv.Key, kv.Value);
            }

            if (!string.IsNullOrEmpty(rewardKey))
            {
                Rule();
                Line(Roi.UI.Loc.GetOr(rewardKey, rewardKey), 16f, new Color(0.70f, 0.95f, 0.72f), 40f, wrap: true);
            }

            // ALWAYS present, even with no `% reward_text`: the payout is a separate fact from the
            // flavour line, and it is the one the player is waiting on.
            Rule();
            _rewardLine = Line(Roi.UI.Loc.Get("UI_STORY_REWARD_READY"), 16f, Roi.UI.UiTheme.Accent, 24f);

            _claim = Roi.UI.HudBar.MakeButton(_rows, Roi.UI.Loc.Get("UI_STORY_CLAIM"), "",
                                              W - Roi.UI.UiTheme.PadLarge * 2f, UiLayoutTouch,
                                              () => { if (_claim != null) _claim.interactable = false; ClaimRequested?.Invoke(); });
            Stack((RectTransform)_claim.transform, UiLayoutTouch);
            _built.Add(_claim.gameObject);

            var back = Roi.UI.HudBar.MakeGhostButton(_rows, Roi.UI.Loc.Get("UI_STORY_BACK"),
                                                     W - Roi.UI.UiTheme.PadLarge * 2f, UiLayoutTouch, Dismiss);
            Stack((RectTransform)back.transform, UiLayoutTouch);
            _built.Add(back.gameObject);

            _y += Roi.UI.UiTheme.PadLarge;
            _rows.sizeDelta = new Vector2(0f, _y);
            _panel.sizeDelta = new Vector2(W, _y);
            IsOpen = true;
            gameObject.SetActive(true);
        }

        const float UiLayoutTouch = 40f;

        /// The reward LINE, filled in after the fact. The payout is a round trip and lands a beat
        /// after the press; showing "claiming…" and then the real figure is honest about which of
        /// the two the player is looking at — and if the call never comes back, the placeholder is
        /// what stays, not a number nobody granted.
        public void SetRewardLine(string text)
        {
            if (_rewardLine != null) _rewardLine.text = text;
        }

        /// `settled` = the server gave a final answer (paid, or already paid). The button goes,
        /// since pressing it again can only repeat that answer.
        ///
        /// NOT settled = the call failed or never arrived, and the button COMES BACK. That is the
        /// entire reason this is a button: an auto-claim that failed had no second chance, and a
        /// finished mission's reward was gone for good over a few dropped packets.
        public void Settled(bool settled)
        {
            if (_claim == null) return;
            if (settled) { Destroy(_claim.gameObject); _claim = null; }
            else _claim.interactable = true;
        }

        public void Dismiss()
        {
            if (!IsOpen) return;
            IsOpen = false;
            gameObject.SetActive(false);
            Closed?.Invoke();
        }

        // --- rows ---------------------------------------------------------------------------

        TextMeshProUGUI Line(string text, float size, Color col, float h, bool wrap = false)
        {
            var t = Roi.UI.UiTheme.Label(_rows, "L", text, size, col, TextAlignmentOptions.Center);
            if (wrap) t.textWrappingMode = TextWrappingModes.Normal;
            Stack(t.rectTransform, h);
            _built.Add(t.gameObject);
            return t;
        }

        void Pair(string label, string value)
        {
            var l = Roi.UI.UiTheme.Label(_rows, "K", label, 15f, Roi.UI.UiTheme.TextMid, TextAlignmentOptions.Left);
            var v = Roi.UI.UiTheme.Label(_rows, "V", value, 15f, Roi.UI.UiTheme.TextHigh, TextAlignmentOptions.Right);
            foreach (var t in new[] { l, v })
            {
                var rt = t.rectTransform;
                rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
                rt.pivot = new Vector2(0.5f, 1f);
                rt.offsetMin = new Vector2(0f, 0f); rt.offsetMax = new Vector2(0f, 0f);
                rt.sizeDelta = new Vector2(0f, 22f);
                rt.anchoredPosition = new Vector2(0f, -_y);
                _built.Add(t.gameObject);
            }
            _y += 22f;
        }

        void Rule()
        {
            var img = Roi.UI.UiTheme.Flat(_rows, "Rule", Roi.UI.UiTheme.Line);
            var rt = img.rectTransform;
            rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
            rt.pivot = new Vector2(0.5f, 1f);
            rt.offsetMin = new Vector2(0f, 0f); rt.offsetMax = new Vector2(0f, 0f);
            rt.sizeDelta = new Vector2(0f, 1f);
            rt.anchoredPosition = new Vector2(0f, -(_y + 6f));
            _built.Add(img.gameObject);
            _y += 14f;
        }

        void Stack(RectTransform rt, float h)
        {
            rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
            rt.pivot = new Vector2(0.5f, 1f);
            rt.offsetMin = new Vector2(0f, 0f); rt.offsetMax = new Vector2(0f, 0f);
            rt.sizeDelta = new Vector2(0f, h);
            rt.anchoredPosition = new Vector2(0f, -_y);
            _y += h + 6f;
        }
    }
}
