// THE ITEM TOOLTIP — what a piece actually is, on hover. Ported from `item_tooltip.gd` +
// `item_desc.gd`, which every screen in the Godot client shares for the same reason this is one
// component: the bag, the shop and the bench must describe an item identically, and three copies
// of the description is how they stop.
//
// THE SIM WRITES THE LINES. `bonus_lines` on the item view is `stat_mod_text` — the same
// vocabulary the card and overlay lines already use — so a rolled bonus reads the same here as it
// does anywhere else, and no formatter is re-implemented on this side to drift from it.
//
// HOVER IS A MOUSE IDEA. There is no hover on a phone, and this client is mobile-shaped, so the
// tooltip is an ADDITION for desktop rather than the only way to read an item: the cell itself
// already shows name, stars and the headline stat. When the touch pass comes, the same
// panel opens from a long press.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class ItemTooltip : MonoBehaviour
    {
        const float W = 300f, Pad = 12f, LineH = 19f;

        static ItemTooltip _instance;
        FloatingItemTooltip _itemCard;

        RectTransform _panel;
        readonly List<GameObject> _lines = new();

        /// Show it for this item, anchored near a screen point. Creates the one instance on first
        /// use; there is never a second, so two cells cannot leave two tooltips on screen.
        public static void Show(ItemView item, Vector2 screenPos)
        {
            if (item == null || string.IsNullOrEmpty(item.def_id)) { Hide(); return; }
            if (!Ensure()) return;
            _instance.gameObject.SetActive(true);
            _instance._itemCard ??= FloatingItemTooltip.Create(_instance.transform.parent);
            _instance._panel.gameObject.SetActive(false);
            _instance._itemCard.Show(FloatingItemDescription.Format(item), screenPos, item);
            _instance.Place(screenPos);
        }

        /// The same panel, for something that is NOT an item — the Regalia slot on the character
        /// sheet is the first. It exists so that slot does not paint a tooltip of its own: the
        /// Godot original did, and a change to this frame or its padding then reached every screen
        /// except that one, silently.
        ///
        /// The FIRST line is the title and takes the head size; the rest are body.
        public static void ShowLines(string[] lines, Vector2 screenPos)
        {
            if (lines == null || lines.Length == 0) { Hide(); return; }
            if (!Ensure()) return;
            _instance.gameObject.SetActive(true);
            _instance._itemCard?.Hide();
            _instance._panel.gameObject.SetActive(true);
            _instance.FillLines(lines);
            _instance.Place(screenPos);
        }

        public static void Hide()
        {
            if (_instance != null) _instance.gameObject.SetActive(false);
        }

        /// The one instance, built on first use. There is never a second, so two hovers cannot
        /// leave two panels on screen.
        static bool Ensure()
        {
            if (_instance != null) return true;
            var bar = Object.FindFirstObjectByType<HudBar>();
            if (bar == null) return false;
            var go = new GameObject("ItemTooltip");
            go.transform.SetParent(bar.PopupLayer, false);
            _instance = go.AddComponent<ItemTooltip>();
            _instance.Build();
            return true;
        }

        void Update()
        {
            if (_itemCard != null && _itemCard.Visible && _itemCard.CanScroll)
                _itemCard.Scroll((UnityEngine.InputSystem.Mouse.current?.scroll.ReadValue().y ?? 0f) / 120f);
        }
        void OnDisable() => _itemCard?.Hide();
        void OnDestroy()
        {
            if (_itemCard != null) Destroy(_itemCard.gameObject);
        }

        void FillLines(string[] lines)
        {
            foreach (var go in _lines) Destroy(go);
            _lines.Clear();
            float y = Pad;
            for (int i = 0; i < lines.Length; i++)
            {
                if (string.IsNullOrEmpty(lines[i])) continue;
                Line(ref y, lines[i], i == 0 ? UiTheme.HeadSize : UiTheme.BodySize,
                     i == 0 ? UiTheme.TextHigh : UiTheme.TextMid, i == 0 ? 22f : 18f, true);
            }
            _panel.sizeDelta = new Vector2(W, y + Pad);
        }

        void Build()
        {
            var root = gameObject.AddComponent<RectTransform>();
            root.anchorMin = Vector2.zero; root.anchorMax = Vector2.one;
            root.offsetMin = Vector2.zero; root.offsetMax = Vector2.zero;
            // OPAQUE. At 96% the grid cell underneath still read through the panel, which is a
            // tooltip you have to look past to read.
            var box = UiTheme.Box(root, "Panel", new Color(0.04f, 0.04f, 0.055f, 1f), 8);
            UiTheme.Border(box.rectTransform, UiTheme.LineStrong, 8);
            box.raycastTarget = false;
            _panel = box.rectTransform;
            _panel.anchorMin = _panel.anchorMax = new Vector2(0f, 0f);
            _panel.pivot = new Vector2(0f, 0f);
            _panel.sizeDelta = new Vector2(W, 100f);
        }

        void Fill(ItemView it)
        {
            foreach (var go in _lines) Destroy(go);
            _lines.Clear();

            float y = Pad;
            var rarity = ItemSlot.ItemColor(it);
            Line(ref y, GameText.ItemName(it), UiTheme.HeadSize, rarity, 22f);

            // WHAT IT IS, in one quiet line: kind, slot, and the level it needs.
            var what = it.kind;
            // The SLOT only means something for a wearable. `item_to_view` derives one for every
            // item, so a Junk bead was reading "Junk · Armor".
            bool wearable = it.kind == "Weapon" || it.kind == "Armor";
            if (wearable && !string.IsNullOrEmpty(it.slot) && it.slot != "None") what += " · " + it.slot;
            // The grade, spelled out: "Tier 3 - Rare". The name already wears the word; this is
            // where a player learns which number it is.
            if (wearable) what = GameText.GradeLabel(it.grade_step) + " · " + what;
            if (it.level_req > 0) what += $" · Lv {it.level_req}";
            Line(ref y, what, UiTheme.CaptionSize, UiTheme.TextLow, LineH);
            y += 4f;

            // The numbers the fight actually uses: the GRADED stat. No ★ floor since 0.0.382 —
            // a piece is its authored stat at its grade, and nothing else.
            Stat(ref y, "ATK", it.graded_atk);
            Stat(ref y, "MATK", it.graded_matk);
            Stat(ref y, "DEF", it.graded_def);

            // The grade's authored lines (the same on every copy of the product).
            foreach (var b in it.authored_lines ?? new string[0])
                Line(ref y, "  " + b, UiTheme.CaptionSize, new Color(0.72f, 0.80f, 0.72f), LineH);
            if (wearable)
                Line(ref y, Loc.Fmt("UI_FLOAT_SOCKETS", it.socket_count, it.gem_slots), UiTheme.CaptionSize, UiTheme.TextLow, LineH);
            foreach (var group in it.gem_line_groups ?? new GemLineGroup[0])
                foreach (var line in group.lines ?? new string[0])
                    Line(ref y, Loc.Get("UI_FORGE_GEMS") + ": " + line, UiTheme.CaptionSize, new Color(.57f,.78f,.88f), LineH);
            if (it.gems?.Length > 0)
                Line(ref y, Loc.Get("UI_FLOAT_MARKET_GEM"), UiTheme.CaptionSize, UiTheme.TextLow, LineH);
            foreach (var b in it.bonus_lines ?? new string[0])
                Line(ref y, "  " + b, UiTheme.CaptionSize, new Color(0.72f, 0.80f, 0.72f), LineH);
        }

        void Stat(ref float y, string label, int total)
        {
            if (total == 0) return;
            Line(ref y, $"{label}  {total}", UiTheme.BodySize, UiTheme.TextHigh, LineH + 2f);
        }

        void Line(ref float y, string text, float size, Color col, float h, bool wrap = false)
        {
            var t = UiTheme.Label(_panel, "L", text, size, col, TextAlignmentOptions.TopLeft);
            t.raycastTarget = false;
            t.overflowMode = TextOverflowModes.Overflow;
            if (wrap)
            {
                t.textWrappingMode = TextWrappingModes.Normal;
                h = Mathf.Max(h, Mathf.Ceil(t.GetPreferredValues(text, W - Pad * 2f, Mathf.Infinity).y) + 4f);
            }
            var rt = t.rectTransform;
            rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
            rt.pivot = new Vector2(0.5f, 1f);
            rt.offsetMin = new Vector2(Pad, 0f); rt.offsetMax = new Vector2(-Pad, 0f);
            rt.sizeDelta = new Vector2(rt.sizeDelta.x, h);
            rt.anchoredPosition = new Vector2(0f, -y);
            _lines.Add(t.gameObject);
            y += h;
        }

        /// Beside the cursor, and FLIPPED at the screen edge rather than clipped — a tooltip that
        /// runs off the right-hand column is one nobody can read on the half of the screen where
        /// most of the cells are.
        void Place(Vector2 screenPos)
        {
            var canvas = _panel.GetComponentInParent<Canvas>();
            float scale = canvas != null ? canvas.scaleFactor : 1f;
            var size = _panel.sizeDelta;
            float x = screenPos.x / scale + 16f;
            float y = screenPos.y / scale + 16f;
            float w = Screen.width / scale, h = Screen.height / scale;
            if (x + size.x > w) x = screenPos.x / scale - size.x - 16f;
            if (y + size.y > h) y = h - size.y - 8f;
            _panel.anchoredPosition = new Vector2(Mathf.Max(4f, x), Mathf.Max(4f, y));
        }
    }
}
