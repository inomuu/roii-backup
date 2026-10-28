// WHAT THE SERVER IS DOING TO THIS SESSION — the two notices an online idle game cannot do without.
// Ported from `sync_banner_view.gd` and `session_ended_view.gd`.
//
//   WARN  — a thin, non-blocking strip along the top while saves are FAILING. The player keeps
//           playing; they are simply told that what they are doing is not being kept. Silence here
//           is the worst outcome in an idle game: hours accumulate and evaporate on the next
//           resume.
//   ENDED — a blocking panel for a session that cannot continue at all: superseded by a login
//           elsewhere, or a version skew that will bounce every save. It must STOP the session
//           rather than let progress pile up that will never be accepted.
//
// The difference is not severity, it is whether playing on is still worth anything.
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class SessionNotice : MonoBehaviour
    {
        static SessionNotice _banner, _modal;

        TextMeshProUGUI _label;

        /// Show or update the non-blocking strip. Repeated calls with the same text are cheap.
        public static void Warn(string text)
        {
            if (_banner == null)
            {
                var bar = Object.FindFirstObjectByType<HudBar>();
                if (bar == null) return;
                var go = new GameObject("SyncBanner");
                go.transform.SetParent(bar.PopupLayer, false);
                _banner = go.AddComponent<SessionNotice>();
                _banner.BuildBanner();
            }
            _banner.gameObject.SetActive(true);
            _banner._label.text = text;
        }

        public static void Clear()
        {
            if (_banner != null) _banner.gameObject.SetActive(false);
        }

        /// The blocking one. `onReturn` is offered as "Back to the login" when there is a way back;
        /// Quit is always there, because a session that cannot save has to be able to end.
        public static void Ended(string heading, string body, System.Action onReturn)
        {
            if (_modal != null) return;      // one is enough; the first reason is the true one
            var bar = Object.FindFirstObjectByType<HudBar>();
            if (bar == null) return;
            var go = new GameObject("SessionEnded");
            // The SYSTEM layer, not the popup one: this panel has to cover the action bar and the
            // roster as well, since nothing on them leads anywhere any more.
            go.transform.SetParent(bar.SystemLayer, false);
            _modal = go.AddComponent<SessionNotice>();
            _modal.BuildModal(heading, body, onReturn);
        }

        void BuildBanner()
        {
            var root = gameObject.AddComponent<RectTransform>();
            root.anchorMin = Vector2.zero; root.anchorMax = Vector2.one;
            root.offsetMin = Vector2.zero; root.offsetMax = Vector2.zero;

            var strip = UiTheme.Flat(root, "Strip", new Color(0.55f, 0.16f, 0.13f, 0.95f));
            // NON-BLOCKING: clicks pass through. The player is being told something, not asked.
            strip.raycastTarget = false;
            var sr = strip.rectTransform;
            sr.anchorMin = new Vector2(0f, 1f); sr.anchorMax = new Vector2(1f, 1f);
            sr.pivot = new Vector2(0.5f, 1f);
            sr.sizeDelta = new Vector2(0f, 28f);
            sr.anchoredPosition = Vector2.zero;

            _label = UiTheme.Label(sr, "Text", "", UiTheme.CaptionSize, Color.white,
                                   TextAlignmentOptions.Center);
            _label.raycastTarget = false;
            var lr = _label.rectTransform;
            lr.anchorMin = Vector2.zero; lr.anchorMax = Vector2.one;
            lr.offsetMin = Vector2.zero; lr.offsetMax = Vector2.zero;
        }

        void BuildModal(string heading, string body, System.Action onReturn)
        {
            var root = gameObject.AddComponent<RectTransform>();
            root.anchorMin = Vector2.zero; root.anchorMax = Vector2.one;
            root.offsetMin = Vector2.zero; root.offsetMax = Vector2.zero;

            var scrim = UiTheme.Flat(root, "Scrim", new Color(0.01f, 0.01f, 0.02f, 0.93f));
            var sr = scrim.rectTransform;
            sr.anchorMin = Vector2.zero; sr.anchorMax = Vector2.one;
            sr.offsetMin = Vector2.zero; sr.offsetMax = Vector2.zero;
            // BLOCKS. There is nothing behind it worth pressing any more.
            var block = scrim.gameObject.AddComponent<Button>();
            block.targetGraphic = scrim;
            block.transition = Selectable.Transition.None;

            var panel = UiTheme.Box(root, "Panel", UiTheme.Panel, 10);
            DialogSurface.Apply(panel);
            var pr = panel.rectTransform;
            pr.anchorMin = pr.anchorMax = new Vector2(0.5f, 0.5f);
            pr.pivot = new Vector2(0.5f, 0.5f);
            pr.sizeDelta = new Vector2(460f, 250f);

            var h = UiTheme.Caps(pr, "Head", heading, UiTheme.TitleSize,
                                 new Color(0.95f, 0.45f, 0.40f), TextAlignmentOptions.Center);
            var hr = h.rectTransform;
            hr.anchorMin = new Vector2(0f, 1f); hr.anchorMax = new Vector2(1f, 1f);
            hr.pivot = new Vector2(0.5f, 1f);
            hr.sizeDelta = new Vector2(0f, 32f);
            hr.anchoredPosition = new Vector2(0f, -UiTheme.PadLarge);

            var b = UiTheme.Label(pr, "Body", body, UiTheme.BodySize, UiTheme.TextMid,
                                  TextAlignmentOptions.Top);
            b.textWrappingMode = TextWrappingModes.Normal;
            var br = b.rectTransform;
            br.anchorMin = new Vector2(0f, 1f); br.anchorMax = new Vector2(1f, 1f);
            br.pivot = new Vector2(0.5f, 1f);
            br.offsetMin = new Vector2(UiTheme.PadLarge, 0f);
            br.offsetMax = new Vector2(-UiTheme.PadLarge, 0f);
            br.sizeDelta = new Vector2(br.sizeDelta.x, 72f);
            br.anchoredPosition = new Vector2(0f, -(UiTheme.PadLarge + 38f));

            float bw = onReturn != null ? 190f : 260f;
            float x = onReturn != null ? -(bw + UiTheme.Gap) / 2f : 0f;
            if (onReturn != null)
            {
                var back = HudBar.MakeButton(pr, Loc.Get("UI_SESSION_NOTICE_BACK_TO_THE_LOGIN"), "", bw, UiLayout.Touch,
                                             () => { _modal = null; onReturn(); Destroy(gameObject); });
                Centre(back, x, -(UiTheme.PadLarge + 130f));
                x += bw + UiTheme.Gap;
            }
            var quit = HudBar.MakeGhostButton(pr, Loc.Get("UI_COMMON_QUIT"), bw, UiLayout.Touch, Quit);
            Centre(quit, x, -(UiTheme.PadLarge + 130f));
        }

        static void Centre(Button b, float x, float y)
        {
            var rt = (RectTransform)b.transform;
            rt.anchorMin = rt.anchorMax = new Vector2(0.5f, 1f);
            rt.pivot = new Vector2(0.5f, 0.5f);
            rt.anchoredPosition = new Vector2(x, y);
        }

        static void Quit()
        {
#if UNITY_EDITOR
            UnityEditor.EditorApplication.isPlaying = false;
#else
            Application.Quit();
#endif
        }
    }
}
