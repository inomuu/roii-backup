// Retained controls in Debug. Uses SessionBoot for every session transition.
using System;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    public sealed class LocalSavePanel : MonoBehaviour
    {
        readonly Button[] _load = new Button[Roi.Sim.LocalSlotStore.Count];
        readonly Button[] _delete = new Button[Roi.Sim.LocalSlotStore.Count];
        TextMeshProUGUI _message;
        Button _save;
        int _confirm = -1;
        float _confirmUntil;

        public static void Build(RectTransform parent, ref float y)
        {
            var go = new GameObject("LocalSaveSlots", typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var rt = (RectTransform)go.transform;
            rt.anchorMin = new Vector2(0, 1); rt.anchorMax = new Vector2(1, 1); rt.pivot = new Vector2(0, 1);
            rt.anchoredPosition = new Vector2(0, -y); rt.sizeDelta = new Vector2(0, 400);
            go.AddComponent<LocalSavePanel>().BuildRows(rt);
            y += 410;
        }
        void BuildRows(RectTransform root)
        {
            var head = UiTheme.Label(root, "Heading", Loc.Get("UI_LOCAL_SAVE_TITLE"), UiTheme.CaptionSize,
                UiTheme.TextHigh, TextAlignmentOptions.Left);
            Place(head.rectTransform, 0, 0, 480, 26);
            _message = UiTheme.Label(root, "Message", "", UiTheme.CaptionSize, UiTheme.TextLow, TextAlignmentOptions.Left);
            Place(_message.rectTransform, 0, 28, 480, 40);
            _message.textWrappingMode = TextWrappingModes.Normal;
            _save = HudBar.MakeButton(root, Loc.Get("UI_LOCAL_SAVE_NOW"), "", 280, UiLayout.Touch, Save);
            Place((RectTransform)_save.transform, 0, 72, 280, UiLayout.Touch);
            for (int i = 0; i < _load.Length; i++)
            {
                int slot = i;
                _load[i] = HudBar.MakeButton(root, "", "", 340, UiLayout.Touch, () => Switch(slot));
                _delete[i] = HudBar.MakeButton(root, "", "", 120, UiLayout.Touch, () => Delete(slot));
                Place((RectTransform)_load[i].transform, 0, 126 + i * 50, 340, UiLayout.Touch);
                Place((RectTransform)_delete[i].transform, 348, 126 + i * 50, 120, UiLayout.Touch);
            }
            Refresh();
        }
        void OnEnable() { if (_message != null) { _confirm = -1; Refresh(); } }
        void Update()
        {
            if (_confirm >= 0 && Time.unscaledTime > _confirmUntil) { _confirm = -1; Refresh(); }
        }
        public void Refresh()
        {
            var boot = Roi.Sim.SessionBoot.Instance;
            bool enabled = boot != null && boot.CanUseLocalSlots;
            _save.interactable = enabled;
            try
            {
                var slots = Roi.Sim.DevSaveStore.Slots;
                for (int i = 0; i < _load.Length; i++)
                {
                    SetText(_load[i], (i == slots.Active ? "> " : "") + (i + 1) + ": " + Summary(slots, i));
                    SetText(_delete[i], Loc.Get(_confirm == i ? "UI_LOCAL_SAVE_CONFIRM" : "UI_LOCAL_SAVE_DELETE"));
                    _load[i].interactable = enabled;
                    _delete[i].interactable = enabled && (slots.Exists(i) || i == slots.Active);
                }
                _message.text = Loc.Get(enabled ? "UI_LOCAL_SAVE_HINT" : "UI_LOCAL_SAVE_ONLY");
            }
            catch (Exception e) { _message.text = e.Message; }
        }
        static string Summary(Roi.Sim.LocalSlotStore slots, int slot)
        {
            try
            {
                var raw = slots.Read(slot);
                if (raw == null) return Loc.Get("UI_LOCAL_SAVE_EMPTY");
                var v = JsonUtility.FromJson<SummaryView>(raw);
                if (v == null || v.grid == null || v.grid.entities == null) return Loc.Get("UI_LOCAL_SAVE_UNREADABLE");
                foreach (var e in v.grid.entities)
                    if (e.team == "Player" && !e.is_pet)
                        return string.Format(Loc.Get("UI_LOCAL_SAVE_SUMMARY"), v.party_name ?? v.character_name ?? "",
                            e.stats != null ? e.stats.level : 1, GameText.UnitName(e.unit_def));
                return Loc.Get("UI_LOCAL_SAVE_UNREADABLE");
            }
            catch { return Loc.Get("UI_LOCAL_SAVE_UNREADABLE"); }
        }
        void Save()
        {
            _confirm = -1;
            var boot = Roi.Sim.SessionBoot.Instance;
            if (boot == null) return;
            string error; bool ok = boot.SaveLocalSlot(out error);
            Refresh(); _message.text = ok ? Loc.Get("UI_LOCAL_SAVE_SAVED") : error;
        }
        void Switch(int slot)
        {
            _confirm = -1;
            var boot = Roi.Sim.SessionBoot.Instance;
            if (boot == null) return;
            string error; bool ok = boot.SwitchLocalSlot(slot, out error);
            Refresh(); if (!ok) _message.text = error;
        }
        void Delete(int slot)
        {
            var boot = Roi.Sim.SessionBoot.Instance;
            if (boot == null || !boot.CanUseLocalSlots) return;
            if (_confirm != slot || Time.unscaledTime > _confirmUntil)
            {
                _confirm = slot; _confirmUntil = Time.unscaledTime + 5f; Refresh();
                _message.text = Loc.Get("UI_LOCAL_SAVE_DELETE_HINT"); return;
            }
            _confirm = -1;
            string error; bool ok = boot.DeleteLocalSlot(slot, out error);
            Refresh(); if (!ok) _message.text = error;
        }
        static void SetText(Button b, string text) => b.GetComponentInChildren<TextMeshProUGUI>().text = text;
        static void Place(RectTransform r, float x, float y, float w, float h)
        {
            r.anchorMin = r.anchorMax = new Vector2(0, 1); r.pivot = new Vector2(0, 1);
            r.anchoredPosition = new Vector2(x, -y); r.sizeDelta = new Vector2(w, h);
        }
        [Serializable] class SummaryView { public string party_name, character_name; public Grid grid; }
        [Serializable] class Grid { public Entity[] entities; }
        [Serializable] class Entity { public string team, unit_def; public bool is_pet; public Stats stats; }
        [Serializable] class Stats { public int level; }
    }
}
