using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Retained graphics for the reference-based Excavation screen.
    // The screen scales uniformly, so portrait crops and ornamental skins never distort.
    internal static class GachaArt
    {
        public static readonly Color Ink = Hex("0C1220"), Ivory = Hex("FFF5DA"), Gold = Hex("E8D7A8");
        static readonly Dictionary<string, Texture2D> Textures = new();

        public static Color Hex(string hex)
        {
            ColorUtility.TryParseHtmlString("#" + hex, out var value);
            return value;
        }

        public static RectTransform Rect(Transform parent, string name, float x, float y, float w, float h)
        {
            var go = new GameObject(name, typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var rt = (RectTransform)go.transform;
            Place(rt, x, y, w, h);
            return rt;
        }

        public static void Place(RectTransform rt, float x, float y, float w, float h)
        {
            rt.anchorMin = rt.anchorMax = new Vector2(0, 1);
            rt.pivot = new Vector2(0, 1);
            rt.anchoredPosition = new Vector2(x, -y);
            rt.sizeDelta = new Vector2(w, h);
        }

        public static Image Panel(Transform parent, string name, float x, float y, float w, float h,
                                  Color color, float radius = 8)
        {
            var image = UiTheme.Box(parent, name, color);
            image.pixelsPerUnitMultiplier = 8 / radius;
            Place(image.rectTransform, x, y, w, h);
            return image;
        }

        public static TextMeshProUGUI Text(Transform parent, string name, string value, float x, float y,
                                          float w, float h, float size, Color color, bool serif = true, bool bold = false)
        {
            var t = UiTheme.Label(parent, name, value, size, color, TextAlignmentOptions.TopLeft);
            if (serif) UiTheme.Display(t);
            t.characterSpacing = 0;
            t.overflowMode = TextOverflowModes.Overflow;
            t.fontStyle = bold ? FontStyles.Bold : FontStyles.Normal;
            t.textWrappingMode = TextWrappingModes.Normal;
            Place(t.rectTransform, x, y, w, h);
            return t;
        }

        public static RawImage Art(Transform parent, string name, float x, float y, float w, float h,
                                   bool cover = false, bool top = false)
        {
            var rt = Rect(parent, name, x, y, w, h);
            var image = rt.gameObject.AddComponent<RawImage>();
            if (!Textures.TryGetValue(name, out var tex))
            {
                tex = Resources.Load<Texture2D>("GachaUI/" + name);
                if (tex != null) Textures.Add(name, tex);
                else Debug.LogError("Missing GachaUI asset: " + name);
            }
            image.texture = tex;
            image.raycastTarget = false;
            if (cover && tex != null)
            {
                float ratio = (w / h) / ((float)tex.width / tex.height);
                image.uvRect = ratio < 1 ? new Rect((1 - ratio) / 2, 0, ratio, 1)
                    : new Rect(0, top ? 1 - 1 / ratio : (1 - 1 / ratio) / 2, 1, 1 / ratio);
            }
            return image;
        }

        public static HomeCurrencyIcon Ticket(Transform parent,float x,float y,float w,float h)
        {
            var icon=Rect(parent,"Ticket",x,y,w,h).gameObject.AddComponent<HomeCurrencyIcon>();
            icon.Kind="Ticket";icon.color=HomeCurrencyIcon.TicketColor;icon.raycastTarget=false;
            return icon;
        }

        public static Button Hit(RectTransform rt, System.Action action)
        {
            var target = rt.GetComponent<Graphic>();
            if (target == null) { target = rt.gameObject.AddComponent<Image>(); target.color = Color.clear; }
            target.raycastTarget = true;
            var b = rt.gameObject.AddComponent<Button>();
            b.targetGraphic = target;
            b.navigation = new Navigation { mode = Navigation.Mode.Automatic };
            var colors = b.colors;
            colors.highlightedColor = new Color(1.1f, 1.1f, 1.1f);
            colors.pressedColor = new Color(.78f, .78f, .78f);
            b.colors = colors;
            b.onClick.AddListener(() => action?.Invoke());
            return b;
        }

        public static void Border(RectTransform rect, Color color, float radius = 8)
            => UiTheme.Border(rect, color).pixelsPerUnitMultiplier = 8 / radius;

        public static void Fade(Transform parent, float x, float y, float w, float h, bool horizontal)
        {
            var rt = Rect(parent, "Readability", x, y, w, h);
            var fade = rt.gameObject.AddComponent<GachaFade>();
            fade.Horizontal = horizontal;
            fade.raycastTarget = false;
        }
    }

    // Readability gradients, drawn as two retained quads.
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class GachaFade : MaskableGraphic
    {
        public bool Horizontal;
        protected override void OnPopulateMesh(VertexHelper vh)
        {
            vh.Clear();
            var r = rectTransform.rect;
            float[] stops = Horizontal ? new[] { 0f, .6f, 1f } : new[] { 0f, .35f, 1f };
            float[] alpha = Horizontal ? new[] { .98f, .85f, 0f } : new[] { .98f, .75f, 0f };
            for (int i = 0; i < 3; i++)
            {
                var color = new Color(10 / 255f, 17 / 255f, 29 / 255f, alpha[i]);
                float p = Mathf.Lerp(Horizontal ? r.xMin : r.yMin, Horizontal ? r.xMax : r.yMax, stops[i]);
                vh.AddVert(Horizontal ? new Vector3(p, r.yMin) : new Vector3(r.xMin, p), color, Vector2.zero);
                vh.AddVert(Horizontal ? new Vector3(p, r.yMax) : new Vector3(r.xMax, p), color, Vector2.zero);
            }
            for (int i = 0; i < 2; i++) { int n = i * 2; vh.AddTriangle(n, n + 1, n + 2); vh.AddTriangle(n + 1, n + 3, n + 2); }
        }
    }
}
