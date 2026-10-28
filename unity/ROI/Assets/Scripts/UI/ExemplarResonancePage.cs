using System;
using System.Text;
using Roi.Sim;
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    // Inspection only. The host currently stores Resonance on an equipped Exemplar carrier,
    // not on a playable character. Never infer a unit rank from unbound copies or a matching name.
    public sealed class ExemplarResonancePage : MonoBehaviour
    {
        SimDriver _driver;
        SimSession _session;
        OwnedExemplar[] _owned;
        string _json, _classId;
        long _memberId;
        bool _layoutDirty = true;
        TextMeshProUGUI _who, _rank, _state, _guidance, _sourceName, _effects;
        GameObject _baseCaption;
        RectTransform _detailContent;

        public static ExemplarResonancePage Create(Transform parent)
        {
            var rt = ExemplarUi.Rect(parent, "ResonancePage", 0, 0, 1, 1);
            var page = rt.gameObject.AddComponent<ExemplarResonancePage>();
            page.Build();
            return page;
        }

        void Build()
        {
            var left = ExemplarUi.Panel(transform, "ResonanceIdentity", .005f, .035f, .255f, .98f,
                ExemplarUi.Surface).transform;
            ExemplarUi.Panel(left, "TopEdge", 0, .997f, 1, 1, ExemplarUi.Line);
            ExemplarUi.Panel(left, "RankSurface", .045f, .429f, .955f, .75f, ExemplarUi.Deep);
            ExemplarUi.Panel(left, "IdentityRule", .025f, .812f, .031f, .965f, ExemplarUi.Gold);
            ExemplarUi.Text(left, "Title", Loc.Get("UI_EXEMPLAR_RESONANCE_TITLE"), .075f, .894f, .95f, .963f,
                34, ExemplarUi.White);
            _who = ExemplarUi.Text(left, "MemberName", "", .075f, .812f, .95f, .898f, 28, ExemplarUi.Gold);
            ExemplarUi.Panel(left, "Divider", .075f, .769f, .925f, .772f,
                ExemplarUi.Line);
            ExemplarUi.Text(left, "RankCaption", Loc.Get("UI_EXEMPLAR_RESONANCE_RANK"), .075f, .675f, .925f, .744f,
                20, ExemplarUi.Muted);
            _rank = ExemplarUi.Text(left, "Rank", "—", .075f, .427f, .81f, .668f, 88, ExemplarUi.Gold);
            _state = ExemplarUi.Text(left, "State", "", .075f, .257f, .925f, .416f,
                22, ExemplarUi.White, TextAlignmentOptions.TopLeft);
            _state.textWrappingMode = TextWrappingModes.Normal;
            _guidance = ExemplarUi.Text(left, "Guidance", "", .075f, .047f, .925f, .241f,
                18, ExemplarUi.Muted, TextAlignmentOptions.TopLeft);
            _guidance.textWrappingMode = TextWrappingModes.Normal;

            // Only the two side surfaces are painted; the shared Live2D remains visible between them.
            var right = ExemplarUi.Panel(transform, "ResonanceDetails", .76f, .035f, 1, .98f,
                ExemplarUi.Surface).transform;
            ExemplarUi.Panel(right, "TopEdge", 0, .997f, 1, 1, ExemplarUi.Line);
            ExemplarUi.Panel(right, "EffectsSurface", .045f, .043f, .955f, .608f, ExemplarUi.Card);
            ExemplarUi.Panel(right, "TopRule", .075f, .962f, .24f, .966f, ExemplarUi.Gold);
            ExemplarUi.Text(right, "EffectsTitle", Loc.Get("UI_EXEMPLAR_RESONANCE_BONUS"), .075f, .857f, .93f, .953f,
                29, ExemplarUi.White);
            _sourceName = ExemplarUi.Text(right, "EffectSource", "", .075f, .707f, .93f, .85f,
                25, ExemplarUi.Gold, TextAlignmentOptions.TopLeft);
            _sourceName.textWrappingMode = TextWrappingModes.Normal;
            _baseCaption = ExemplarUi.Text(right, "BaseCaption", Loc.Get("UI_EXEMPLAR_RESONANCE_BASE"),
                .075f, .638f, .93f, .70f, 18, ExemplarUi.Muted).gameObject;
            ExemplarUi.Panel(right, "EffectsDivider", .075f, .619f, .925f, .622f,
                ExemplarUi.Line);
            var host = ExemplarUi.Rect(right, "EffectsScroll", .075f, .072f, .93f, .585f);
            _detailContent = UiScroll.Create(host, out _);
            _effects = ExemplarUi.Text(_detailContent, "Effects", "", 0, 0, 1, 1,
                21, ExemplarUi.White, TextAlignmentOptions.TopLeft);
            _effects.enableAutoSizing = false;
            _effects.textWrappingMode = TextWrappingModes.Normal;
            _effects.overflowMode = TextOverflowModes.Overflow;
            Show(null);
        }

        public void Show(ExemplarReadModel.Member member)
        {
            if (_driver == null) _driver = UnityEngine.Object.FindFirstObjectByType<SimDriver>();
            var session = _driver != null ? _driver.Session : null;
            string json = member != null && member.Id > 0 && session != null && session.IsAlive
                ? session.QueryJson("owned_exemplars") : null;
            bool changed = !ReferenceEquals(session, _session) || json != _json;
            if (changed) _owned = ReadItems(json);
            bool changedMember = _memberId != (member?.Id ?? 0) || _classId != member?.UnitDef;
            _session = session;
            _json = json;
            _memberId = member?.Id ?? 0;
            _classId = member?.UnitDef;
            if (!changed && !changedMember && !_layoutDirty) return;
            _layoutDirty = false;
            var linked = BoundTo(_owned, _memberId);
            _who.text = member == null ? Loc.Get("UI_EXEMPLAR_NO_CHARACTER") : GameText.UnitName(member.UnitDef);
            _rank.text = linked == null ? "—" : linked.resonance_stars.ToString();
            _state.text = linked == null ? Loc.Get("UI_EXEMPLAR_RESONANCE_EMPTY")
                : GameText.ItemName(linked.def_id, linked.name);
            _guidance.text = linked == null ? Loc.Get("UI_EXEMPLAR_RESONANCE_WAIT") : "";
            _sourceName.text = linked == null ? "" : GameText.ItemName(linked.def_id, linked.name);

            // overlay_lines is the host's base_mods projection. overlay_star_lines is only
            // star_ranks.first().mods; it is not cumulative or resolved at resonance_stars.
            // Consequently no star-effect preview is presented as the character's current bonus.
            var text = new StringBuilder();
            if (linked?.overlay_lines != null)
                foreach (var line in linked.overlay_lines)
                    if (!string.IsNullOrEmpty(line))
                    {
                        if (text.Length > 0) text.Append("\n\n");
                        text.Append(line);
                    }
            bool hasBase = text.Length > 0;
            _baseCaption.SetActive(hasBase);
            _effects.text = hasBase ? text.ToString() : Loc.Get("UI_EXEMPLAR_RESONANCE_NO_BONUS");
            _effects.color = hasBase ? ExemplarUi.White : ExemplarUi.Muted;
            FitDetails();
        }

        static OwnedExemplar BoundTo(OwnedExemplar[] owned, long memberId)
        {
            if (owned == null || memberId <= 0) return null;
            OwnedExemplar linked = null;
            foreach (var item in owned)
            {
                if (item == null || item.equipped_by != memberId || item.kind != "Exemplar"
                    || item.overlay_kind != "exemplar" || string.IsNullOrEmpty(item.def_id)
                    || item.resonance_stars < 0) continue;
                // A malformed ambiguous binding is not resolved by arbitrarily picking a copy.
                if (linked != null) return null;
                linked = item;
            }
            return linked;
        }

        static OwnedExemplar[] ReadItems(string json)
        {
            if (string.IsNullOrWhiteSpace(json) || json.TrimStart()[0] != '[') return null;
            try { return JsonUtility.FromJson<OwnedList>("{\"items\":" + json + "}")?.items; }
            catch (ArgumentException) { return null; }
        }

        void FitDetails()
        {
            float width = _detailContent.rect.width > 1 ? _detailContent.rect.width : 310;
            float height = Mathf.Max(40, _effects.GetPreferredValues(_effects.text, width, Mathf.Infinity).y + 8);
            var rt = _effects.rectTransform;
            rt.anchorMin = new Vector2(0, 1); rt.anchorMax = new Vector2(1, 1); rt.pivot = new Vector2(.5f, 1);
            rt.sizeDelta = new Vector2(0, height); rt.anchoredPosition = Vector2.zero;
            UiScroll.Fit(_detailContent, height);
        }

        void OnRectTransformDimensionsChange() => _layoutDirty = true;

        [Serializable] sealed class OwnedList { public OwnedExemplar[] items; }
        [Serializable] sealed class OwnedExemplar
        {
            public string def_id, name, kind, overlay_kind;
            public long equipped_by;
            // Rust intentionally omits a zero rank; default zero is valid only after exact binding.
            public int resonance_stars;
            public string[] overlay_lines;
        }
    }
}
