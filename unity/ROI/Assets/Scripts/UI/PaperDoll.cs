// THE PAPER-DOLL — the equipped-gear cells around a character, as a SHARED component.
//
// Ported from godot/views/paper_doll.gd, whose own header says why it is a component and not part
// of a screen: the Character sheet draws it, and so does the Enchant workbench, so the
// doll is drawn ONE way. The user confirmed the same for this port ("paper doll ก็ยังคงต้องทำ
// เพราะต้องใช้ตรงหน้า blacksmith"), which is the reason this is its own file on day one rather
// than something extracted from the Character screen later.
//
// The API is the original's, kept deliberately: the owner sets a per-cell highlight STATE
// (normal / selected / eligible / dim) and listens for a click carrying the EQUIPPED INDEX. That
// index is the thing a workbench needs — "enchant the item in equipped slot 3" — and it is why the
// doll reports an index rather than a slot name.
//
// The slot arrangement, the labels, the 64 px cells and the 80 px row step are copied value for
// value. So is the dual-wield rule: an off-hand WEAPON shows in the Shield cell.
//
// NOT PORTED: the character sprite in the centre. Godot draws it with `CharSprite` against a
// texture cache the 2D renderer owns; on this side the equivalent is the arena's billboard
// pipeline, and pulling a UI-space idle frame out of it is a sprite job rather than a layout one.
// The cells are the functional half and the half a workbench needs.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class PaperDoll : MonoBehaviour
    {
        public enum State { Normal, Selected, Eligible, Dim }

        const float SlotPx = 64f, RowStep = 80f, SideLabelW = 132f, CentreW = 320f;
        public const float Width = SlotPx * 2f + CentreW;
        public const float Height = RowStep * 6f + 12f; // six rows on the right since Bracer (0.0.383)

        // (slot key, display label). Keys are `EquipSlot`'s own spelling — renamed in 0.0.375 so
        // the label and the key finally say the same thing (the old pairs read
        // "HeadMid" → "Face" and "HeadLow" → "Aura", which is how a mask ended up in a head slot
        // and an aura on the legs). Defence down the left, utility down the right.
        static readonly (string Key, string Label)[] Left =
        {
            ("Helmet", "Helmet"), ("Armor", "Armor"), ("Gloves", "Gloves"),
            ("Shoes", "Shoes"),
        };
        static readonly (string Key, string Label)[] Right =
        {
            ("Weapon", "Weapon"), ("Shield", "Off-Hand"), ("Necklace", "Necklace"),
            ("Ring", "Ring"), ("Earring", "Earring"), ("Bracer", "Bracer"),
        };

        /// Fired with the EQUIPPED INDEX of the cell tapped. An index, not a slot name, because
        /// that is what a workbench acts on.
        public event System.Action<int> SlotClicked;

        /// Show each item's name and stat beside its cell, on the side facing the character (the
        /// RO status-window look). Opt-in: the Character sheet wants it, a workbench does not.
        public bool SideLabels { get; set; }

        RectTransform _rt;
        readonly List<Cell> _cells = new();

        sealed class Cell
        {
            public GameObject Go;
            public Image Bg, Border;
            public int EqIdx;
            public bool Has;
        }

        public static PaperDoll Attach(RectTransform parent, bool sideLabels = false)
        {
            var go = new GameObject("PaperDoll", typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var d = go.AddComponent<PaperDoll>();
            d._rt = (RectTransform)go.transform;
            d._rt.sizeDelta = new Vector2(Width, Height);
            d.SideLabels = sideLabels;
            return d;
        }

        public RectTransform Rect => _rt;

        /// `equipped` is the entity's list as the view sends it, already through `item_to_view`.
        public void SetPlayer(ItemView[] equipped)
        {
            foreach (var c in _cells) Destroy(c.Go);
            _cells.Clear();
            equipped ??= new ItemView[0];

            // Slot key -> index into `equipped`. Built once rather than searched per cell.
            var bySlot = new Dictionary<string, int>();
            for (int i = 0; i < equipped.Length; i++)
                if (!string.IsNullOrEmpty(equipped[i].slot)) bySlot[equipped[i].slot] = i;

            // The columns differ in length now (Bracer makes six on the right, 0.0.383); each
            // draws its own rows. DUAL WIELD: the off-hand dagger wears the Shield slot itself —
            // the Off-Hand cell shows whichever it holds.
            for (int row = 0; row < Left.Length; row++) AddCell(Left[row], equipped, bySlot, row, true);
            for (int row = 0; row < Right.Length; row++) AddCell(Right[row], equipped, bySlot, row, false);
        }

        void AddCell((string Key, string Label) slot, ItemView[] eq, Dictionary<string, int> bySlot,
                     int row, bool left)
        {
            bool has = bySlot.TryGetValue(slot.Key, out int eqIdx);
            var item = has && eqIdx < eq.Length ? eq[eqIdx] : null;

            float x = left ? 2f : Width - SlotPx - 2f;
            float y = 6f + row * RowStep;

            var box = new GameObject("Cell_" + slot.Key, typeof(RectTransform));
            box.transform.SetParent(_rt, false);
            var brt = (RectTransform)box.transform;
            brt.anchorMin = brt.anchorMax = new Vector2(0f, 1f);
            brt.pivot = new Vector2(0f, 1f);
            brt.sizeDelta = new Vector2(SlotPx, RowStep - 4f);
            brt.anchoredPosition = new Vector2(x, -y);

            var lbl = UiTheme.Caps(brt, "Label", slot.Label, 9f, UiTheme.TextLow,
                                   TextAlignmentOptions.Center);
            var lr = lbl.rectTransform;
            lr.anchorMin = new Vector2(0f, 1f); lr.anchorMax = new Vector2(1f, 1f);
            lr.pivot = new Vector2(0.5f, 1f);
            // 16, not 12. A TMP label whose rect is shorter than one line renders NOTHING, and at
            // 12 px every slot name on this doll silently vanished — the same trap the item cell
            // hit, which is why the number is stated rather than eyeballed.
            lr.sizeDelta = new Vector2(0f, 16f);
            lbl.characterSpacing = 2f;

            var bg = UiTheme.Box(brt, "Bg", has ? UiTheme.Surface : new Color(0.031f, 0.033f, 0.045f, 1f), 4);
            bg.sprite = UiTheme.ItemCardFill; // WuWa idiom: chamfered cell, matches every other item slot
            var border = UiTheme.Border(bg.rectTransform, has ? RarityLine(item) : UiTheme.Line, 4);
            border.sprite = UiTheme.ItemCardFrame;
            var sr = bg.rectTransform;
            sr.anchorMin = sr.anchorMax = new Vector2(0.5f, 1f);
            sr.pivot = new Vector2(0.5f, 1f);
            sr.sizeDelta = new Vector2(SlotPx, SlotPx);
            sr.anchoredPosition = new Vector2(0f, -14f);

            if (has)
            {
                var btn = bg.gameObject.AddComponent<Button>();
                btn.targetGraphic = bg;
                btn.navigation = new Navigation { mode = Navigation.Mode.None };
                var colors = btn.colors;
                colors.highlightedColor = new Color(1.35f, 1.35f, 1.35f, 1f);
                colors.fadeDuration = 0.08f;
                btn.colors = colors;
                int idx = eqIdx;
                btn.onClick.AddListener(() => SlotClicked?.Invoke(idx));

                // The three-letter stub is ONLY for a doll with no side labels: with them on it
                // repeats the first word of a name already written beside the cell.
                if (!SideLabels)
                {
                    var kind = UiTheme.Label(sr, "Kind", Short(item), 10f,
                                             UiTheme.TextMid, TextAlignmentOptions.BottomLeft);
                    var kr = kind.rectTransform;
                    kr.anchorMin = Vector2.zero; kr.anchorMax = Vector2.one;
                    kr.offsetMin = new Vector2(5f, 4f); kr.offsetMax = new Vector2(-5f, -4f);
                }
            }

            if (SideLabels && has)
            {
                var side = UiTheme.Label(_rt, "Side_" + slot.Key, SideText(item), UiTheme.CaptionSize,
                                         UiTheme.TextMid,
                                         left ? TextAlignmentOptions.Left : TextAlignmentOptions.Right);
                side.textWrappingMode = TextWrappingModes.NoWrap;
                var vr = side.rectTransform;
                vr.anchorMin = vr.anchorMax = new Vector2(0f, 1f);
                vr.pivot = new Vector2(0f, 1f);
                vr.sizeDelta = new Vector2(SideLabelW, SlotPx);
                vr.anchoredPosition = new Vector2(left ? x + SlotPx + 6f : x - SideLabelW - 6f,
                                                  -(y + 14f));
                side.transform.SetParent(_rt, true);
            }

            _cells.Add(new Cell { Go = box, Bg = bg, Border = border, EqIdx = has ? eqIdx : -1, Has = has });
        }

        /// Highlight a cell by its equipped index. A no-op for a slot this doll does not show, so
        /// a workbench can mark every eligible item without first asking which ones are visible.
        public void SetState(int eqIdx, State st)
        {
            foreach (var c in _cells)
                if (c.EqIdx == eqIdx) { Paint(c, st); return; }
        }

        public void ClearStates()
        {
            foreach (var c in _cells) Paint(c, State.Normal);
        }

        void Paint(Cell c, State st)
        {
            if (!c.Has) return;
            switch (st)
            {
                case State.Selected:
                    c.Bg.color = UiTheme.SurfaceActive; c.Border.color = UiTheme.Accent; break;
                case State.Eligible:
                    // Half-strength ACCENT, not a brighter grey. On the enchant bench "eligible"
                    // is the answer the screen exists to give, and a hairline two shades lighter
                    // than the resting border is not an answer anyone can see.
                    c.Bg.color = UiTheme.Surface;
                    c.Border.color = new Color(UiTheme.Accent.r, UiTheme.Accent.g, UiTheme.Accent.b, 0.55f);
                    break;
                case State.Dim:
                    c.Bg.color = new Color(0.031f, 0.033f, 0.045f, 1f);
                    c.Border.color = new Color(1f, 1f, 1f, 0.04f); break;
                default:
                    c.Bg.color = UiTheme.Surface; c.Border.color = UiTheme.Line; break;
            }
        }

        /// Name on one line, the stat on the next — `_side_text` minus the
        /// socketed-card lines, which need the card list the view does not send with equipped gear.
        static string SideText(ItemView it)
        {
            var name = string.IsNullOrEmpty(it.name) ? it.def_id : it.name;
            var head = name;
            var stat = ItemSlot.StatLine(it);
            return string.IsNullOrEmpty(stat) ? head : head + "\n" + stat;
        }

        /// Two or three letters inside the cell: with no icon art, an empty square and a filled one
        /// look identical from across the screen.
        static string Short(ItemView it)
        {
            var s = string.IsNullOrEmpty(it.name) ? it.def_id : it.name;
            return s.Length <= 3 ? s : s.Substring(0, 3).ToUpperInvariant();
        }

        static Color RarityLine(ItemView it)
        {
            // Gear's line is its GRADE (0.0.380); a Common piece keeps the plain frame.
            if (it.grade_step <= 0 && !it.foil) return UiTheme.LineStrong;
            var c = ItemSlot.ItemColor(it);
            return new Color(c.r, c.g, c.b, 0.55f);
        }
    }
}
