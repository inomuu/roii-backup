// One inventory cell, ported from godot/views/item_slot.gd's `draw_slot`. Everything it paints is
// a decision that file already made and this one keeps: the rarity BORDER (a foil card overrides
// it, so a foil reads as special before there is art), the mark for a bound item (bound cannot be
// sold, and the mark is what says so at a glance), the stack `xN`, and one line
// of stat at the foot for gear only.
//
// What changed in the port is the SHAPE, not the meaning: rounded corners, a hairline at rest, and
// a cell tall enough for an item's name to wrap onto a second line instead of being cut.
// Known equipment now uses the shared Tier artwork; names remain the fallback for other items.
//
// Drawn as retained Controls rather than an immediate `_draw`, per the project's UI discipline —
// a grid of these is rebuilt only when the item SET changes, never per tick.
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class ItemSlot : MonoBehaviour
    {
        // item_slot.gd's RARITY_COLORS.
        static readonly Dictionary<string, Color> Rarity = new Dictionary<string, Color>
        {
            { "Common", new Color(0.62f, 0.62f, 0.66f) },
            { "Uncommon", new Color(0.42f, 0.80f, 0.45f) },
            { "Rare", new Color(0.36f, 0.62f, 0.98f) },
            { "Mythical", new Color(0.70f, 0.42f, 0.98f) },
            { "Legendary", new Color(1.00f, 0.72f, 0.24f) },
            { "Immortal", new Color(1.00f, 0.36f, 0.85f) },
        };

        // A CELL BIG ENOUGH TO READ. 64 px was the Godot number, and it assumed an item ICON with the
        // name in a tooltip. There is no icon art yet, so the name IS the cell — and a two-line name
        // squeezed into 72 px had to auto-shrink to 9 pt, which is where it started to break up.
        // The fix is the cell, not the shader.
        public const float SlotW = 96f, SlotH = 122f;

        /// An empty cell is a HOLE, not a tile: darker than the panel it sits on, with no border to
        /// speak of. Fifty of them drawn as surfaces would read as fifty things rather than as room.
        static readonly Color EmptyFill = new Color(0.031f, 0.033f, 0.045f, 1f);

        public static ItemSlot Create(Transform parent)
        {
            var go = new GameObject("Slot", typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var s = go.AddComponent<ItemSlot>();
            s.Build();
            return s;
        }

        const int MaxStars = 5;

        RectTransform _rt;
        Image _bg, _border, _bound, _rarityLine, _rarityBacking;
        UnityEngine.UI.Button _btn;
        bool _selected;
        readonly Image[] _stars = new Image[MaxStars];
        const float NameSize = 12f, NameSizeSmall = 10f;
        TextMeshProUGUI _name, _mid, _stat, _rarityLabel;
        ExemplarEquipmentIcon _art;
        WuwaGradient _glow;

        void Build()
        {
            _rt = (RectTransform)transform;
            _rt.sizeDelta = new Vector2(SlotW, SlotH);

            _bg = UiTheme.Box(transform, "Bg", EmptyFill, 4);
            _bg.sprite = UiTheme.ItemCardFill; // WuWa idiom: chamfered cell, not rounded
            Stretch(_bg.rectTransform);
            // TAP-SELECT, not drag (2026-09-07, the no-drag rule): the fill IS the hit target, so a
            // whole cell is a 96x104 touch target rather than a small icon inside one. Only screens
            // that give it an action wire the tap — everywhere else this stays inert.
            _btn = gameObject.AddComponent<UnityEngine.UI.Button>();
            _btn.targetGraphic = _bg;
            _btn.navigation = new UnityEngine.UI.Navigation { mode = UnityEngine.UI.Navigation.Mode.None };
            _btn.transition = UnityEngine.UI.Selectable.Transition.None;
            _btn.enabled = false;
            // The border is a 9-sliced ring drawn OVER the fill — one image, one draw, and it does
            // not dim with the fill the way a tinted parent would.
            _border = UiTheme.Border(_rt, UiTheme.Line, 4);
            _border.sprite = UiTheme.ItemCardFrame; // matches the chamfered Bg above
            // WuWa rarity glow: the item's tier rises as light from the foot of the cell.
            _glow = WuwaGradient.Add(transform, Color.clear, Color.clear, false);
            _glow.transform.SetSiblingIndex(_bg.transform.GetSiblingIndex() + 1);
            _glow.rectTransform.offsetMin = new Vector2(1f, 1f); _glow.rectTransform.offsetMax = new Vector2(-1f, -1f);

            // Bound: a small square in the top-right corner. The Godot original draws a wedge; a
            // wedge needs its own sprite here and says exactly as much as a dot does.
            _bound = UiTheme.Flat(transform, "Bound", UiTheme.Accent);
            _bound.raycastTarget = false;
            var wr = _bound.rectTransform;
            wr.anchorMin = wr.anchorMax = new Vector2(1f, 1f);
            wr.pivot = new Vector2(1f, 1f);
            wr.sizeDelta = new Vector2(7f, 7f);
            wr.anchoredPosition = new Vector2(-7f, -7f);

            // Four bands, measured from the BOTTOM of the cell so the cell's height is one number
            // to change rather than eight. Name on top because until there is item art the name
            // is the icon; the numbers sit along the foot where the eye scans a grid for them.
            _name = Label("Name", NameSize, UiTheme.TextHigh, TextAlignmentOptions.TopLeft);
            Band(_name, 40f, SlotH - 9f, 9f, 15f);
            _name.textWrappingMode = TextWrappingModes.Normal;
            _name.overflowMode = TextOverflowModes.Ellipsis;
            _name.characterSpacing = 0f;      // a wrapped name wants its width back
            _name.lineSpacing = -8f;
            var artRoot = new GameObject("ItemArtwork", typeof(RectTransform)).GetComponent<RectTransform>();
            artRoot.SetParent(transform, false);
            artRoot.anchorMin = artRoot.anchorMax = artRoot.pivot = new Vector2(.5f, 1f);
            artRoot.anchoredPosition = new Vector2(0, -7f);
            artRoot.sizeDelta = new Vector2(SlotW - 18f, SlotH - 56f);
            _art = ExemplarEquipmentIcon.Create(artRoot, "Weapon");
            _art.enabled = false;
            // NOT auto-sized, deliberately. Auto-sizing settles on a FRACTIONAL point size, and a
            // fractional size is most of why small SDF text looks eaten — the glyph stops falling
            // on a consistent sample of the atlas from one label to the next. The cell was made big
            // enough to hold the name instead, and an over-long one ellipses like any other label.

            // The star rating is DRAWN, not typed. The UI face carries no star, diamond or filled
            // circle — checked, not assumed — and a bullet at 9 px reads as an ellipsis, which is
            // what it looked like. Five pips, lit up to the item's rating, say it without a glyph
            // and without a second font in the build.
            for (int i = 0; i < MaxStars; i++)
            {
                var pip = UiTheme.Box(transform, "Star" + i, UiTheme.Accent, 4);
                pip.raycastTarget = false;
                var pr = pip.rectTransform;
                pr.anchorMin = pr.anchorMax = new Vector2(0f, 0f);
                pr.pivot = new Vector2(0f, 0f);
                pr.sizeDelta = new Vector2(6f, 6f);
                pr.anchoredPosition = new Vector2(9f + i * 10f, 29f);
                _stars[i] = pip;
            }

            _stat = Label("Stat", 11f, UiTheme.TextLow, TextAlignmentOptions.BottomLeft);
            Band(_stat, 5f, 26f, 9f, 9f);
            _stat.characterSpacing = 0f;

            _mid = Label("Mid", 15f, UiTheme.Accent, TextAlignmentOptions.BottomRight);
            Band(_mid, 4f, 27f, 9f, 9f);
            _mid.characterSpacing = 0f;
            _rarityBacking=UiTheme.Flat(transform,"RarityBacking",UiTheme.Panel);_rarityBacking.raycastTarget=false;
            var badge=_rarityBacking.rectTransform;badge.anchorMin=badge.anchorMax=new Vector2(0,0);badge.pivot=Vector2.zero;badge.anchoredPosition=new Vector2(4,27);badge.sizeDelta=new Vector2(SlotW-8,20);
            _rarityLine=UiTheme.Flat(transform,"RarityLine",UiTheme.Accent);_rarityLine.raycastTarget=false;
            var line=_rarityLine.rectTransform;line.anchorMin=line.anchorMax=new Vector2(0,0);line.pivot=Vector2.zero;line.anchoredPosition=new Vector2(4,47);line.sizeDelta=new Vector2(SlotW-8,3);
            _rarityLabel=Label("Rarity",10,UiTheme.TextHigh,TextAlignmentOptions.Center);Band(_rarityLabel,28,46,5,5);
            _rarityLabel.enableAutoSizing=true;_rarityLabel.fontSizeMin=8;_rarityLabel.fontSizeMax=10;_rarityLabel.raycastTarget=false;

        }

        /// One point smaller when a SINGLE WORD is wider than the cell.
        ///
        /// TMP's normal wrapping breaks inside a word once that word cannot fit a line on its own,
        /// so "Dawnbreaker" printed as "Dawnbreake" over "r". Auto-sizing would fix it and is
        /// refused above for a good reason; this steps between two WHOLE point sizes instead, which
        /// keeps every glyph on the same atlas sample it would have had.
        void FitName()
        {
            _name.fontSize = NameSize;
            // The band's width from the CONSTANTS, not `rect.width`: a cell is filled before the
            // grid has laid it out, so the live rect is still zero on the pass that matters.
            const float band = SlotW - 9f - 15f;
            foreach (var word in _name.text.Split(' '))
                if (_name.GetPreferredValues(word).x > band) { _name.fontSize = NameSizeSmall; return; }
        }

        /// Place a label in a horizontal band `bottom`..`top` pixels up from the cell's foot.
        ///
        /// The band must be TALLER THAN ONE LINE. A TMP label set to ellipsis overflow whose rect
        /// is shorter than its own line height does not clip — it renders NOTHING, silently, and
        /// three of these cells' four labels vanished that way with their text and colours intact.
        static void Band(TextMeshProUGUI t, float bottom, float top, float left, float right)
        {
            var rt = t.rectTransform;
            rt.offsetMin = new Vector2(left, bottom);
            rt.offsetMax = new Vector2(-right, top - SlotH);
            // These three are one line by construction; letting them overflow their band is safer
            // than trusting every future font's line height to fit inside it.
            if (t.name != "Name") t.overflowMode = TextOverflowModes.Overflow;
        }

        TextMeshProUGUI Label(string name, float size, Color col, TextAlignmentOptions align)
        {
            var t = UiTheme.Label(transform, name, "", size, col, align);
            var rt = t.rectTransform;
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            return t;
        }

        static void Stretch(RectTransform rt, float inset = 0f)
        {
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = new Vector2(inset, inset);
            rt.offsetMax = new Vector2(-inset, -inset);
        }

        ItemView _item;

        /// What this cell is showing — for a screenshot probe, which has no pointer to hover with.
        public ItemView Item => _item;

        /// The pointer entered or left this cell — the tooltip follows. Added by `SetTap` so a
        /// cell nobody can act on is also a cell that does not raycast.
        sealed class Hover : UnityEngine.EventSystems.EventTrigger
        {
            public ItemSlot Slot;
            void OnDisable() => ItemTooltip.Hide();

            public override void OnPointerEnter(UnityEngine.EventSystems.PointerEventData e)
                => ItemTooltip.Show(Slot._item, e.position);

            public override void OnPointerExit(UnityEngine.EventSystems.PointerEventData e)
                => ItemTooltip.Hide();
        }

        /// Give this cell an action. Called by the screens that have one (the bag); the rest leave
        /// the button disabled so an inert grid does not eat clicks meant for what is behind it.
        public void SetTap(System.Action onTap)
        {
            _btn.onClick.RemoveAllListeners();
            _btn.enabled = onTap != null;
            _bg.raycastTarget = onTap != null;
            if (onTap != null) _btn.onClick.AddListener(() => onTap());
            if (onTap != null && GetComponent<Hover>() == null)
                gameObject.AddComponent<Hover>().Slot = this;
        }

        /// The ELIGIBLE mark — half-strength accent, the same weight the paper doll uses. Distinct
        /// from selection on purpose: one cell is chosen, several are possible.
        public void SetEligible(bool on)
        {
            if (_selected) return;
            _border.color = on
                ? new Color(UiTheme.Accent.r, UiTheme.Accent.g, UiTheme.Accent.b, 0.55f)
                : _restBorder;
        }

        /// The selection ring. Full-strength accent, where rarity is held to a hairline — the one
        /// cell the next button press will act on has to be unmistakable.
        public void SetSelected(bool on)
        {
            _selected = on;
            _border.color = on ? UiTheme.Accent : _restBorder;
        }

        Color _restBorder = UiTheme.Line;

        /// `item` null = an EMPTY slot, which is drawn rather than skipped: the grid shows the
        /// bag's whole capacity so "how much room is left" is visible without reading the header.
        public void SetItem(ItemView item)
        {
            _item = item;
            bool has = item != null && !string.IsNullOrEmpty(item.def_id);
            bool hasArt = EquipmentArtwork.TryGet(item, out _, out _);
            _art.SetItem(item, item?.equip_slot);
            _art.enabled = hasArt;
            _name.enabled = !hasArt;
            _bound.enabled = has && item.bound;
            bool badge=has&&item.kind!="Card";
            _rarityBacking.enabled=_rarityLine.enabled=_rarityLabel.enabled=badge;
            if(badge){var tone=ItemColor(item);_rarityLine.color=tone;_rarityBacking.color=Color.Lerp(UiTheme.Panel,tone,.24f);
                _rarityLabel.color=Color.Lerp(tone,Color.white,.55f);
                _rarityLabel.text=item.kind=="Weapon"||item.kind=="Armor"?GameText.GradeName(item.grade_step):Loc.GetOr(item.rarity??"",item.rarity??"");
            }
            if (!has)
            {
                _bg.color = EmptyFill;
                _glow.Set(Color.clear, Color.clear);
                _restBorder = UiTheme.Line;
                _border.color = _selected ? UiTheme.Accent : _restBorder;
                _name.text = _mid.text = _stat.text = "";
                SetStars(0);
                return;
            }
            _bg.color = UiTheme.Surface;
            var col = ItemColor(item);
            _glow.Set(new Color(col.r, col.g, col.b, 0f), new Color(col.r, col.g, col.b, .38f));
            // The rarity is the border, held back to a hairline's weight. At full strength, a bag
            // of commons became a grid of grey boxes competing with the names inside them.
            _restBorder = new Color(col.r, col.g, col.b, 0.55f);
            _border.color = _selected ? UiTheme.Accent : _restBorder;

            _name.text = GameText.ItemName(item);
            FitName();
            // Pips only for a CARD's star (its level); gear has no ★ since 0.0.382.
            SetStars(item.kind == "Card" ? item.star : 0);
            _mid.text = item.quantity > 1 ? "×" + item.quantity : "";
            _mid.color = UiTheme.TextMid;
            _stat.text = StatLine(item);
            _stat.color = UiTheme.TextLow;   // a cell reused after SetFootnote kept its colour
            // The two share the foot, so the stat gives up exactly the width the quantity chip takes
            // and no more. Fixed insets had "+210 ATK" printing straight through "+10".
            // GetPreferredValues, not `preferredWidth`: the latter reports the width of the
            // LAST laid-out text, and the chip's text was only assigned a line ago — it came back
            // zero, and the two printed straight through each other.
            float chip = _mid.text.Length > 0
                ? _mid.GetPreferredValues(_mid.text).x + UiTheme.Gap : 0f;
            _stat.rectTransform.offsetMax = new Vector2(-(9f + chip), _stat.rectTransform.offsetMax.y);
            FitStat(SlotW - 18f - chip);
        }

        /// Replace the cell's foot line with a caption of the screen's own choosing.
        ///
        /// A carrier has no ATK line worth printing, and what the player needs from the cell is
        /// who is WEARING it. Called after `SetItem`, which is what computes the line this
        /// overrides — the order matters, and reversing it silently prints the stat again.
        public void SetFootnote(string text, Color colour)
        {
            _stat.text = text ?? "";
            _stat.color = colour;
            float chip = _mid.text.Length > 0 ? _mid.GetPreferredValues(_mid.text).x + UiTheme.Gap : 0f;
            FitStat(SlotW - 18f - chip);
        }

        /// The stat line has to share the cell's foot with the quantity chip, and "+210 ATK" beside
        /// a "+10" does not fit a 96 px cell at full size. Step DOWN in whole points rather
        /// than letting TMP auto-size: auto-sizing lands on a fractional size, and a fractional size
        /// is what makes SDF text look eaten. Three whole-number steps stay crisp and never overlap.
        static readonly float[] StatSteps = { 11f, 10f, 9f };

        void FitStat(float avail)
        {
            if (_stat.text.Length == 0) return;
            foreach (var size in StatSteps)
            {
                _stat.fontSize = size;
                if (_stat.GetPreferredValues(_stat.text).x <= avail) return;
            }
        }

        /// Light `n` of the five pips; a rating of zero hides the row rather than showing five
        /// empty ones — an unstarred item has no rating, which is not the same as a rating of nil.
        void SetStars(int n)
        {
            n = Mathf.Clamp(n, 0, MaxStars);
            for (int i = 0; i < MaxStars; i++) _stars[i].enabled = i < n;
        }

        /// item_slot.stat_str: the one line a gear slot shows. MATK wins a tie because a caster's
        /// weapon reads as a caster's weapon.
        /// The rarity colour a name should take. Shared with the tooltip so the two agree.
        public static Color RarityColor(string rarity, bool foil)
        {
            if (foil) return new Color(1f, 0.90f, 0.45f);
            return Rarity.TryGetValue(rarity ?? "", out var c) ? c : new Color(0.45f, 0.45f, 0.50f);
        }

        /// THE colour of an item: gear by GRADE (its one axis since 0.0.380), everything else by
        /// rarity. Every cell, border, tooltip title and loot line goes through here so a piece
        /// cannot be one colour in the bag and another on the doll.
        public static Color ItemColor(ItemView it)
        {
            if (it == null) return new Color(0.45f, 0.45f, 0.50f);
            if (it.foil) return new Color(1f, 0.90f, 0.45f);
            if (it.kind == "Weapon" || it.kind == "Armor") return GameText.GradeColor(it.grade_step);
            return RarityColor(it.rarity, false);
        }

        public static string StatLine(ItemView it)
        {
            // GRADED, not authored: `graded_*` is what the fight uses (base × grade).
            int atk = it.graded_atk;
            int matk = it.graded_matk;
            int def = it.graded_def;
            if (matk > 0 && matk >= atk) return "+" + matk + " MATK";
            if (atk > 0) return "+" + atk + " ATK";
            if (def > 0) return "+" + def + " DEF";
            return "";
        }
    }

    /// The item fields the bag draws. `item_to_view` sends far more; JsonUtility keeps what is
    /// declared and drops the rest, so this list IS the screen's contract with the sim.
    [System.Serializable]
    public class ItemView
    {
        public string def_id, name, kind, rarity, equip_slot, weapon_type, instance_id, grade;
        /// Loc KEYS for the words the item's rolled bonuses earn — see GameText.ItemName.
        public string affix_pre, affix_suf;
        /// The rolled bonuses as TEXT, written by the sim (`stat_mod_text`) so a tooltip and a
        /// stat sheet cannot describe the same bonus differently.
        public string[] bonus_lines, authored_lines;
        public string[] wieldable, special_vals;
        public GemLineGroup[] gem_line_groups;
        public SocketedGemView[] gems;
        public int gem_slots;
        public bool market_clean;
        public int locked_slot = -1;
        /// A card's effect lines and an Exemplar/Regalia's overlay lines — both written by the sim
        /// in the same vocabulary as `bonus_lines`.
        public string[] card_lines, card_foil_lines, bind_lines, overlay_lines;
        /// Which card PANEL this card belongs to ("Weapon", "Necklace", ...). Only a Card
        /// carries it; the panel it is currently sitting in is stamped onto it when equipped.
        public string card_slot;
        /// The EQUIP slot this piece of gear occupies ("Weapon", "Armor", "Helmet", ...). This is
        /// what the paper-doll matches its cells against, and it is `item_to_view`'s own field —
        /// derived from the item, not stored on it.
        public string slot;
        public int quantity = 1, star, item_level, level_req, sell_value, socket_count;
        public int atk_bonus, matk_bonus, def_bonus;
        /// The sim's GRADE-scaled stats — base × the grade multiplier, computed in `item_view`.
        /// Print these, never the authored `*_bonus` fields beside them.
        public int graded_atk, graded_matk, graded_def, graded_atk_speed_bp, grade_step;
        public bool bound, foil, unique, marketable;
        /// A card's ELEMENT, which picks its frame colour. Present on every item the sim sends;
        /// only the card face reads it.
        public string element;
        /// The entity id of the party member WEARING this carrier, 0 for none. Only an
        /// Exemplar or a Regalia uses it: equipping one marks the carrier rather than
        /// moving it, so ownership and wear are two different facts about the same item.
        public long equipped_by;
        /// Present only on a Consumable — `Item.consumable` is skipped in serde when absent, and
        /// the item view is the whole item plus derived fields, so it arrives for free. Guard on
        /// `kind` before reading it: JsonUtility hands back a default instance, never null.
        public ConsumableSpecView consumable;
    }

    /// The half of a consumable's spec a screen needs: which reserved slot it belongs in, if any.
    /// "Meal" / "Tonic" / "Party", or empty for a plain emergency consumable.
    [System.Serializable]
    public class ConsumableSpecView { public string provision; }

    /// JsonUtility cannot parse a top-level array, so the query's payload is wrapped.
    [System.Serializable] public class GemLineGroup { public string[] lines; }
    [System.Serializable] public class SocketedGemView { public string def_id; }

    [System.Serializable]
    public class ItemList { public ItemView[] items; }
}
