using TMPro;
using UnityEngine;
using UnityEngine.UI;
namespace Roi.UI
{
    public sealed partial class CardsWindow
    {
        const float CardW=104, CardH=132;
        // --- cells ---------------------------------------------------------------------------

        /// Static and public so the capture tool — which lives in the EDITOR assembly, where
        /// `internal` does not reach — can draw a sheet of card states without a
        /// session: a card drops at a weight of 1 in 50,000, so a play session will not produce
        /// one to photograph, and a cell design nobody has looked at is a cell design nobody has
        /// checked.
        public static GameObject Card(RectTransform parent, float x, float y, ItemView card,
                                        string badge, System.Action onTap)
        {
            var img = UiTheme.Box(parent, "Card_" + card.def_id, UiTheme.Surface, 4);
            img.color = Color.clear;
            GameMenuSurface.Add(img.rectTransform);
            var rarity=UiTheme.Flat(img.rectTransform,"Rarity",RarityOf(card));rarity.raycastTarget=false;
            var rr=rarity.rectTransform;rr.anchorMin=new Vector2(0,0);rr.anchorMax=new Vector2(1,0);
            rr.pivot=new Vector2(.5f,0);rr.offsetMin=new Vector2(12,3);rr.offsetMax=new Vector2(-12,5);
            var btn = img.gameObject.AddComponent<Button>();
            btn.targetGraphic = img;
            btn.navigation = new Navigation { mode = Navigation.Mode.None };
            var colors = btn.colors;
            colors.highlightedColor = new Color(1.35f, 1.35f, 1.35f, 1f);
            colors.fadeDuration = 0.08f;
            btn.colors = colors;
            if (onTap != null) btn.onClick.AddListener(() => onTap());
            var rt = img.rectTransform;
            rt.anchorMin = rt.anchorMax = new Vector2(0f, 1f);
            rt.pivot = new Vector2(0f, 1f);
            rt.sizeDelta = new Vector2(CardW, CardH);
            rt.anchoredPosition = new Vector2(x, -y);

            // The monster it depicts, animated on the arena's own idle clip. Behind the name, so a
            // long name still reads.
            var idle = CardArt.Idle(card);
            if (idle != null && idle.Length > 0)
            {
                var mon = new GameObject("Mon", typeof(RectTransform));
                mon.transform.SetParent(rt, false);
                var mi = mon.AddComponent<Image>();
                mi.raycastTarget = false;
                mi.preserveAspect = true;
                mi.sprite = idle[0];
                var mr = mi.rectTransform;
                mr.anchorMin = new Vector2(0.5f, 0f); mr.anchorMax = new Vector2(0.5f, 0f);
                mr.pivot = new Vector2(0.5f, 0f);
                mr.sizeDelta = new Vector2(CardW * 0.82f, CardW * 0.82f);
                mr.anchoredPosition = new Vector2(0f, CardH * 0.34f);
                mon.AddComponent<CardIdle>().Play(idle, CardArt.IdleFpsFor(card));
            }

            var name = UiTheme.Label(rt, "Name", GameText.ItemName(card),
                                     UiTheme.CaptionSize, UiTheme.TextHigh, TextAlignmentOptions.TopLeft);
            name.textWrappingMode = TextWrappingModes.Normal;
            name.characterSpacing = 0f;
            var nr = name.rectTransform;
            nr.anchorMin = Vector2.zero; nr.anchorMax = Vector2.one;
            nr.offsetMin = new Vector2(9f, 22f); nr.offsetMax = new Vector2(-9f, -88f);
            name.fontSize=11; name.enableAutoSizing=true; name.fontSizeMin=10; name.fontSizeMax=11;

            if (card.star > 0)
            {
                var stars = UiTheme.Label(rt, "Stars", Loc.Fmt("UI_COMMON_LEVEL_SHORT", card.star), 10f,
                                          UiTheme.Accent, TextAlignmentOptions.BottomLeft);
                var sr = stars.rectTransform;
                sr.anchorMin = Vector2.zero; sr.anchorMax = Vector2.one;
                sr.offsetMin = new Vector2(9f, 6f); sr.offsetMax = new Vector2(-9f, -CardH + 24f);
            }
            if (!string.IsNullOrEmpty(badge))
            {
                var b = UiTheme.Label(rt, "Badge", badge, 10f,
                                      badge == "inactive" ? new Color(0.86f, 0.45f, 0.40f) : UiTheme.TextMid,
                                      TextAlignmentOptions.BottomRight);
                var br = b.rectTransform;
                br.anchorMin = Vector2.zero; br.anchorMax = Vector2.one;
                br.offsetMin = new Vector2(9f, 6f); br.offsetMax = new Vector2(-9f, -CardH + 24f);
            }
            if (card.foil)
            {
                var f = UiTheme.Flat(rt, "Foil", new Color(1f, 0.90f, 0.45f));
                f.raycastTarget = false;
                var fr = f.rectTransform;
                fr.anchorMin = fr.anchorMax = new Vector2(1f, 1f);
                fr.pivot = new Vector2(1f, 1f);
                fr.sizeDelta = new Vector2(7f, 7f);
                fr.anchoredPosition = new Vector2(-7f, -7f);
            }
            return img.gameObject;
        }

        public static GameObject EmptyCard(RectTransform parent, float x, float y, string label)
        {
            var img = UiTheme.Box(parent, "Empty", new Color(0.031f, 0.033f, 0.045f, 1f), 4);
            img.raycastTarget = false;
            UiTheme.Border(img.rectTransform, UiTheme.Line);
            var rt = img.rectTransform;
            rt.anchorMin = rt.anchorMax = new Vector2(0f, 1f);
            rt.pivot = new Vector2(0f, 1f);
            rt.sizeDelta = new Vector2(CardW, CardH);
            rt.anchoredPosition = new Vector2(x, -y);
            var t = UiTheme.Label(rt, "Label", label, 10f, UiTheme.TextLow, TextAlignmentOptions.Center);
            var tr = t.rectTransform;
            tr.anchorMin = Vector2.zero; tr.anchorMax = Vector2.one;
            tr.offsetMin = Vector2.zero; tr.offsetMax = Vector2.zero;
            return img.gameObject;
        }

        /// The idle clip, ticked. Its own component so each cell keeps its own phase — one shared
        /// clock made a screen of cards blink in unison, which reads as a rendering glitch rather
        /// than as a row of living things.
        sealed class CardIdle : MonoBehaviour
        {
            Sprite[] _frames;
            float _fps, _t;
            Image _img;

            public void Play(Sprite[] frames, float fps)
            {
                _frames = frames; _fps = fps;
                _img = GetComponent<Image>();
                _t = Random.value * frames.Length;   // stagger the phase across a grid
            }

            void Update()
            {
                if (_frames == null || _frames.Length < 2 || _img == null || !UiEntrance.MotionAllowed || PlayerPrefs.GetInt("roi.opt.postfx",1)==0 || _img.canvasRenderer.cull) return;
                _t += Time.unscaledDeltaTime * _fps;
                _img.sprite = _frames[(int)_t % _frames.Length];
            }
        }

        static Color RarityOf(ItemView card)
        {
            if (card.foil) return new Color(1f, 0.90f, 0.45f);
            switch (card.rarity)
            {
                case "Uncommon": return new Color(0.42f, 0.80f, 0.45f, 0.55f);
                case "Rare": return new Color(0.36f, 0.62f, 0.98f, 0.55f);
                case "Mythical": return new Color(0.70f, 0.42f, 0.98f, 0.55f);
                case "Legendary": return new Color(1.00f, 0.72f, 0.24f, 0.55f);
                case "Immortal": return new Color(1.00f, 0.36f, 0.85f, 0.55f);
                default: return UiTheme.Line;
            }
        }

    }
}
