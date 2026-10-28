using System;
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    /// The complete character sheet for the floating management window. The panel is deliberately
    /// a separate child of the host so it can sit to the left of the paper doll without changing
    /// the inventory layout. Every value comes from the native stats:<entity_id> projection.
    public sealed class FloatingFullStatsPanel
    {
        readonly FloatingInventoryHost _host;
        readonly RectTransform _root, _content;
        readonly System.Collections.Generic.List<GameObject> _built = new();
        string _fingerprint = "";

        [Serializable]
        sealed class StatsDto
        {
            public int level, level_cap;
            public long atk, def;
            public int dodge_bp, crit_bp, crit_dmg_bp, mitigation_bp, atk_speed_x100;
            public long max_hp, xp, level_points, core_unspent, apex_level, apex_unspent;
            public int[] apex_alloc;
        }

        public RectTransform Root => _root;
        public bool Visible => _root != null && _root.gameObject.activeSelf;

        public FloatingFullStatsPanel(RectTransform parent, FloatingInventoryHost host)
        {
            _host = host;
            _root = ForgeUi.Panel(parent, "FullStatsPanel", 0, 0, 320, FloatingInventoryHost.Height,
                                   ForgeUi.Ground).rectTransform;
            UiTheme.Border(_root, new Color(.48f, .43f, .32f), 2);
            var title = ForgeUi.Panel(_root, "StatsDragBar", 1, 1, 318, 35, UiTheme.Surface);
            ForgeUi.Copy(title.transform, "UI_FLOAT_FULL_STATS", 12, 0, 260, 35, 16, ForgeUi.Gold);
            ForgeUi.Button(title.transform, "UI_FLOAT_CLOSE", 284, 5, 26, 26, Hide);
            var scroll = ForgeUi.Rect(_root, "StatsScroll", 12, 48, 296, FloatingInventoryHost.Height - 60);
            _content = UiScroll.Create(scroll, out _);
            _root.gameObject.SetActive(false);
        }

        public void SetVisible(bool visible)
        {
            if (!visible)
            {
                _root.gameObject.SetActive(false);
                _fingerprint = "";
                return;
            }
            bool changed=!Visible;
            _root.gameObject.SetActive(true);
            Refresh(_host.MemberId, _host.CurrentUnitDef);
            if(changed)UiPageTransition.Play(_root);
        }

        void Hide() => _host.SetFullStatsVisible(false);

        public void Refresh(long memberId, string unitDef)
        {
            if (!Visible || memberId <= 0) return;
            var session = _host.Model?.Driver?.Session;
            if (session == null || !session.IsAlive) return;
            var dto = Read(session.QueryJson("stats:" + memberId));
            if (dto == null || dto.level <= 0)
            {
                RebuildMessage("UI_FLOAT_BENCHED");
                return;
            }

            var alloc = dto.apex_alloc == null ? "" : string.Join(", ", dto.apex_alloc);
            var fp = string.Join("|", memberId, unitDef, dto.level, dto.level_cap, dto.atk,
                dto.def, dto.max_hp, dto.dodge_bp, dto.crit_bp,
                dto.crit_dmg_bp, dto.mitigation_bp, dto.atk_speed_x100, dto.xp, dto.level_points,
                dto.core_unspent, dto.apex_level, dto.apex_unspent, alloc);
            if (fp == _fingerprint) return;
            _fingerprint = fp;

            Clear();
            float y = 10;
            Header(ref y, "UI_FLOAT_STAT_COMBAT");
            Row(ref y, "UI_FLOAT_STAT_MAX_HP", dto.max_hp.ToString("N0"));
            Row(ref y, "UI_FLOAT_STAT_ATK", dto.atk.ToString("N0"));
            Row(ref y, "UI_FLOAT_STAT_DEF", dto.def.ToString("N0"));
            Row(ref y, "UI_FLOAT_STAT_DODGE", Percent(dto.dodge_bp));
            Row(ref y, "UI_FLOAT_STAT_CRIT", Percent(dto.crit_bp));
            Row(ref y, "UI_FLOAT_STAT_CRIT_DMG", Percent(dto.crit_dmg_bp));
            Row(ref y, "UI_FLOAT_STAT_ATK_SPEED", (dto.atk_speed_x100 / 100m).ToString("0.00") + "/s");
            Row(ref y, "UI_FLOAT_STAT_MITIGATION", Percent(dto.mitigation_bp));
            Header(ref y, "UI_FLOAT_STAT_PROGRESSION");
            Row(ref y, "UI_FLOAT_STAT_LEVEL", dto.level + " / " + dto.level_cap);
            Row(ref y, "UI_FLOAT_STAT_XP", dto.xp.ToString("N0"));
            Row(ref y, "UI_FLOAT_STAT_LEVEL_POINTS", dto.level_points.ToString("N0"));
            Row(ref y, "UI_FLOAT_STAT_CORE_POINTS", dto.core_unspent.ToString("N0"));
            Row(ref y, "UI_FLOAT_STAT_APEX_LEVEL", dto.apex_level.ToString("N0"));
            Row(ref y, "UI_FLOAT_STAT_APEX_POINTS", dto.apex_unspent.ToString("N0"));
            if (!string.IsNullOrEmpty(alloc)) Row(ref y, "UI_FLOAT_STAT_APEX_ALLOC", alloc);
            UiScroll.Fit(_content, y + 10);
        }

        void RebuildMessage(string key)
        {
            Clear();
            var text = ForgeUi.Copy(_content, key, 8, 10, 280, 60, 13, ForgeUi.Ink);
            _built.Add(text.gameObject);
            ForgeUi.Wrap(text);
            UiScroll.Fit(_content, 80);
        }

        void Header(ref float y, string key)
        {
            var text = ForgeUi.Copy(_content, key, 8, y, 280, 24, 11, ForgeUi.Gold);
            text.characterSpacing = 1.2f;
            _built.Add(text.gameObject);
            y += 28;
        }

        void Row(ref float y, string key, string value)
        {
            var panel = ForgeUi.Panel(_content, "StatRow", 4, y, 288, 27, new Color(.08f, .095f, .12f, .72f));
            var label = ForgeUi.Copy(panel.transform, key, 10, 0, 170, 27, 12, ForgeUi.Ink);
            var amount = ForgeUi.Text(panel.transform, "Value", value, 175, 0, 103, 27, 12, ForgeUi.Gold);
            amount.alignment = TextAlignmentOptions.MidlineRight;
            amount.enableAutoSizing=true; amount.fontSizeMin=8; amount.fontSizeMax=12;
            amount.textWrappingMode=TextWrappingModes.NoWrap;
            _built.Add(panel.gameObject);
            y += 30;
        }

        void Clear()
        {
            foreach (var go in _built) if (go != null) UnityEngine.Object.Destroy(go);
            _built.Clear();
        }

        static string Percent(int basisPoints) => (basisPoints / 100f).ToString("0.0") + "%";

        static StatsDto Read(string json)
        {
            if (string.IsNullOrWhiteSpace(json) || json.TrimStart()[0] != '{') return null;
            try { return JsonUtility.FromJson<StatsDto>(json); }
            catch (ArgumentException) { return null; }
        }
    }
}
