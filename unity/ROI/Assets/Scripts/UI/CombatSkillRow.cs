// Godot overlay_view.gd / hud_icon.gd: retained icons, top-down cooldown mask,
// ceiling seconds, shared tooltip. Values come from the host, never a client timer.
using System;
using System.Collections.Generic;
using TMPro;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class CombatSkillRow : MonoBehaviour
    {
        const float Size = 28f, Gap = 4f;
        bool _belowHealth;
        public void PlaceBelowHealth() => _belowHealth = true;
        readonly List<CombatSkillIcon> _icons = new();
        static readonly Dictionary<string, Texture2D> Art = new();
        static Dictionary<string, string> _files;
        public void Refresh(Roi.Sim.SimSession session, long member)
        {
            var data = JsonUtility.FromJson<Rows>("{\"rows\":" + session.QueryJson("combat_skills:" + member) + "}");
            var rows = data?.rows ?? Array.Empty<Skill>();
            for (int i = 0; i < rows.Length; i++)
            {
                if (i == _icons.Count)
                {
                    var bg = UiTheme.Box(transform, "Skill" + i, UiTheme.Surface);
                    var rt = bg.rectTransform;
                    rt.anchorMin = rt.anchorMax = new Vector2(0, _belowHealth ? 0 : 1);
                    rt.pivot = new Vector2(0, 0);
                    float size = _belowHealth ? 21f : Size;
                    rt.sizeDelta = new Vector2(size, size);
                    rt.anchoredPosition = _belowHealth
                        ? new Vector2((i % 5) * (size + Gap), -(i / 5) * (size + Gap))
                        : new Vector2(8 + (i % 5) * (Size + Gap), 5 + (i / 5) * (Size + Gap));
                    var icon = bg.gameObject.AddComponent<CombatSkillIcon>();
                    icon.Build(rt);
                    _icons.Add(icon);
                }
                _icons[i].gameObject.SetActive(true);
                _icons[i].Paint(rows[i], GetArt(rows[i].id));
            }
            for (int i = rows.Length; i < _icons.Count; i++) _icons[i].gameObject.SetActive(false);
        }
        static Texture2D GetArt(string id)
        {
            if (_files == null)
            {
                _files = new Dictionary<string, string>();
                var map = Resources.Load<TextAsset>("SkillIcons/mapping");
                if (map != null)
                    foreach (var r in JsonUtility.FromJson<Mapping>(map.text).rows) _files[r.id] = r.file;
            }
            if (!Art.TryGetValue(id, out var tex))
            {
                tex = Resources.Load<Texture2D>("SkillIcons/" + (_files.TryGetValue(id, out var file) ? file : "magic_amplification"));
                if (tex != null) tex.filterMode = FilterMode.Point;
                Art[id] = tex;
            }
            return tex;
        }
        [Serializable] class Mapping { public Entry[] rows; }
        [Serializable] class Entry { public string id, file; }
        [Serializable] class Rows { public Skill[] rows; }
        [Serializable] public class Skill
        {
            public string id, name, blurb, detail;
            public int level, cd, cd_max;
            public bool passive;
        }
    }

    public sealed class CombatSkillIcon : MonoBehaviour, IPointerEnterHandler, IPointerExitHandler
    {
        RawImage _art;
        Image _mask, _border;
        TextMeshProUGUI _seconds;
        CombatSkillRow.Skill _skill;
        bool _hover;
        Vector2 _pointer;
        public void Build(RectTransform root)
        {
            _border = UiTheme.Border(root, UiTheme.Line);
            var go = new GameObject("Art", typeof(RectTransform), typeof(RawImage));
            go.transform.SetParent(root, false);
            _art = go.GetComponent<RawImage>(); _art.raycastTarget = false;
            var rt = _art.rectTransform;
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = Vector2.one * 3; rt.offsetMax = Vector2.one * -3;
            _mask = UiTheme.Flat(root, "Cooldown", new Color(0, 0, 0, 0.7f));
            _mask.raycastTarget = false;
            _mask.rectTransform.offsetMin = _mask.rectTransform.offsetMax = Vector2.zero;
            _seconds = UiTheme.Label(root, "Seconds", "", 14, UiTheme.TextHigh, TextAlignmentOptions.Center);
            _seconds.raycastTarget = false;
            _seconds.rectTransform.anchorMin = Vector2.zero; _seconds.rectTransform.anchorMax = Vector2.one;
            _seconds.rectTransform.offsetMin = _seconds.rectTransform.offsetMax = Vector2.zero;
        }
        public void Paint(CombatSkillRow.Skill skill, Texture2D art)
        {
            if (_hover && _skill != null && _skill.id != skill.id) { _hover = false; ItemTooltip.Hide(); }
            bool changed = _skill != null && _skill.level != skill.level;
            _skill = skill; _art.texture = art;
            _border.color = skill.passive ? new Color(0.45f, 0.75f, 0.85f) : UiTheme.Line;
            float fraction = !skill.passive && skill.cd_max > 0 ? Mathf.Clamp01(skill.cd / (float)skill.cd_max) : 0;
            _mask.rectTransform.anchorMin = new Vector2(0, 1 - fraction);
            _mask.rectTransform.anchorMax = Vector2.one;
            _seconds.text = !skill.passive && skill.cd > 0 ? ((skill.cd + 9) / 10).ToString() : "";
            if (_hover && changed) Show();
        }
        void Show()
        {
            if (_skill == null) return;
            ItemTooltip.ShowLines(new[] {
                Loc.GetOr("SKILL_" + _skill.id, _skill.name) + " · " + Loc.Fmt("UI_COMMON_LEVEL_SHORT", _skill.level),
                Loc.Get(_skill.passive ? "UI_SKD_PASSIVE" : "UI_SKD_ACTIVE"),
                Loc.GetOr("SKILL_" + _skill.id + "_B", _skill.blurb),
                Loc.GetOr("SKILL_" + _skill.id + "_D", _skill.detail),
                _skill.passive || _skill.cd_max <= 0 ? "" : Loc.Get("UI_SKD_COOLDOWN") + ": " + (_skill.cd_max / 10f).ToString("0.0") + " s"
            }, _pointer);
        }
        public void OnPointerEnter(PointerEventData e) { _hover = true; _pointer = e.position; Show(); }
        public void OnPointerExit(PointerEventData e) { _hover = false; ItemTooltip.Hide(); }
        void OnDisable() { if (_hover) ItemTooltip.Hide(); _hover = false; }
    }
}
