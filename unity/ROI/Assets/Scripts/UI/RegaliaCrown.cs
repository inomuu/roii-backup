// THE EQUIPPED-REGALIA SLOT on the character sheet. Ported from views/regalia_crown.gd.
//
// A Regalia has TWO axes and the slot shows both: the Enhancement Level (top-left) and the
// Resonance stars (along the foot). Steel-blue, because that is what tells it apart from the gold
// Exemplar slot at a glance — a distinction that survives even though the Exemplar's own crown is
// not ported (Exemplar became a unit, so it has no carrier to wear).
//
// EMPTY IS A STATE, not an absence: an unworn slot draws as a plain "Regalia" cell, so the screen
// says the slot exists and is empty rather than saying nothing at all.
//
// HOVER IS REPORTED, NOT PAINTED. The Godot original drew its own tooltip panel, which made it one
// of two places in that client that did — so a change to the shared tooltip's frame or padding
// reached every screen except this one, silently. This feeds `ItemTooltip` like every other slot.
//
// PRESSING IT OPENS THE REGALIA SCREEN. Godot pops an equip picker here; this client already has a
// screen that owns that verb, and a second way to do the same thing is a second thing to keep in
// step.
using TMPro;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class RegaliaCrown : MonoBehaviour, IPointerEnterHandler, IPointerExitHandler
    {
        public const float Size = 64f;
        const int MaxStars = 5;

        static readonly Color Steel = new Color(0.62f, 0.78f, 1.0f);
        static readonly Color SteelDim = new Color(0.30f, 0.34f, 0.42f);

        Image _bg, _border;
        TextMeshProUGUI _initial, _level, _empty;
        readonly Image[] _stars = new Image[MaxStars];
        View _rd;

        public static RegaliaCrown Create(Transform parent)
        {
            var img = UiTheme.Box(parent, "RegaliaCrown", UiTheme.Surface, 6);
            var c = img.gameObject.AddComponent<RegaliaCrown>();
            c._bg = img;
            var rt = img.rectTransform;
            rt.sizeDelta = new Vector2(Size, Size);
            c._border = UiTheme.Border(rt, UiTheme.Line, 6);

            var btn = img.gameObject.AddComponent<Button>();
            btn.targetGraphic = img;
            btn.navigation = new Navigation { mode = Navigation.Mode.None };
            btn.transition = Selectable.Transition.None;
            btn.onClick.AddListener(() =>
            {
                var bar = Object.FindFirstObjectByType<HudBar>();
                if (bar != null) bar.Toggle("Regalia");
            });

            c._empty = UiTheme.Label(rt, "Empty", Loc.Get("UI_REGALIA_CROWN_EMPTY"), 12f,
                                     UiTheme.TextLow, TextAlignmentOptions.Center);
            Fill(c._empty.rectTransform);

            // The INITIAL rather than the name: 64 px holds one letter legibly and a name not at
            // all, and the full name is one hover away.
            c._initial = UiTheme.Label(rt, "Initial", "", 28f, Steel, TextAlignmentOptions.Center);
            Fill(c._initial.rectTransform);

            c._level = UiTheme.Label(rt, "Level", "", 11f, new Color(0.80f, 0.90f, 1.0f),
                                     TextAlignmentOptions.TopLeft);
            var lr = c._level.rectTransform;
            lr.anchorMin = Vector2.zero; lr.anchorMax = Vector2.one;
            lr.offsetMin = new Vector2(4f, 0f); lr.offsetMax = new Vector2(-4f, -3f);

            // Pips, not glyphs — the UI face carries no star, and a bullet at this size reads as an
            // ellipsis. The same five pips the item cells use, so one rating looks like another.
            for (int i = 0; i < MaxStars; i++)
            {
                var pip = UiTheme.Box(rt, "Star" + i, Steel, 4);
                pip.raycastTarget = false;
                var pr = pip.rectTransform;
                pr.anchorMin = pr.anchorMax = new Vector2(0.5f, 0f);
                pr.pivot = new Vector2(0.5f, 0f);
                pr.sizeDelta = new Vector2(6f, 6f);
                pr.anchoredPosition = new Vector2((i - 2) * 10f, 4f);
                c._stars[i] = pip;
            }
            c.SetRegalia(null);
            return c;
        }

        static void Fill(RectTransform rt)
        {
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = Vector2.zero; rt.offsetMax = Vector2.zero;
        }

        public RectTransform Rect => (RectTransform)transform;

        /// `null` = nothing worn. The payload is the entity's own `regalia` block from the per-tick
        /// view — the sim resolves the stat lines there at the worn level and stars, so this never
        /// folds `effect` × `per_level` itself.
        public void SetRegalia(View rd)
        {
            _rd = rd != null && !string.IsNullOrEmpty(rd.id) ? rd : null;
            bool has = _rd != null;
            _empty.gameObject.SetActive(!has);
            _initial.gameObject.SetActive(has);
            _level.gameObject.SetActive(has);
            for (int i = 0; i < MaxStars; i++)
            {
                _stars[i].gameObject.SetActive(has);
                if (has) _stars[i].color = i < _rd.stars ? Steel : SteelDim;
            }
            _bg.color = has ? new Color(0.09f, 0.12f, 0.18f) : UiTheme.Surface;
            _border.color = has ? Steel : UiTheme.Line;
            if (!has) return;
            var name = string.IsNullOrEmpty(_rd.name) ? _rd.id : _rd.name;
            _initial.text = name.Substring(0, 1).ToUpperInvariant();
            _level.text = Loc.Fmt("UI_REGALIA_CROWN_LEVEL", _rd.level);
        }

        public void OnPointerEnter(PointerEventData e)
        {
            if (_rd == null) return;
            ItemTooltip.ShowLines(TipLines(), e.position);
        }

        public void OnPointerExit(PointerEventData e) => ItemTooltip.Hide();

        /// The hover card, as the shared tooltip's lines. The stat text is the SIM's — `view_state`
        /// injects `lines` resolved at the worn level and stars — because the stored record
        /// serializes none of the pieces the client would need to fold them itself.
        string[] TipLines()
        {
            var name = string.IsNullOrEmpty(_rd.name) ? _rd.id : _rd.name;
            var head = new System.Collections.Generic.List<string>
            {
                name,
                Loc.Fmt("UI_REGALIA_CROWN_AXES", _rd.level, MaxStars, _rd.stars, MaxStars),
            };
            if (_rd.lines != null && _rd.lines.Length > 0) head.Add(string.Join("  ", _rd.lines));
            return head.ToArray();
        }

        /// The entity's `regalia` block. Declared here rather than in the character screen because
        /// this widget is the only thing that reads it.
        [System.Serializable]
        public class View
        {
            public string id, name;
            public int level, stars;
            public string[] lines;
        }
    }
}
