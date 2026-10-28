using System;
using System.Collections.Generic;
using System.Linq;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    /// Right-hand floating panel for the party-wide Training ladder. It is deliberately a panel,
    /// not a scene, so Battle and Companion share one state and the sim keeps ticking underneath.
    public sealed class TrainingPanel
    {
        public readonly RectTransform Root;
        readonly TrainingModel _model;
        readonly FloatingInventoryHost _host;
        readonly TextMeshProUGUI _summary, _status;
        readonly RectTransform _content;
        readonly List<TrainingRow> _rows = new();
        Toggle _sortToggle;
        Image _sortBackground;
        Image _sortBox;
        Image _sortCheck;
        TMP_Text _sortLabel;
        int _group;
        bool _readyFirst;

        public TrainingPanel(Transform parent, TrainingModel model, FloatingInventoryHost host)
        {
            _model = model; _host = host;
            Root = ForgeUi.Panel(parent, "Training", 510, 0, 320, 560, ForgeUi.Ground).rectTransform;
            UiTheme.Border(Root, new Color(.48f, .43f, .32f), 2);
            ForgeUi.Heading(ForgeUi.Copy(Root, "UI_TRAINING", 14, 4, 154, 32, 17, ForgeUi.Ink),22);
            _sortBackground = ForgeUi.Panel(Root, "UpgradeSortToggle", 174, 5, 108, 32, ForgeUi.Raised);
            UiTheme.Border(_sortBackground.rectTransform, new Color(.32f, .34f, .37f), 1);
            _sortToggle = _sortBackground.gameObject.AddComponent<Toggle>();
            _sortToggle.targetGraphic = _sortBackground;
            _sortToggle.onValueChanged.AddListener(ToggleUpgradeFirst);
            _sortBox = ForgeUi.Panel(_sortBackground.transform, "CheckBox", 3, 6, 14, 14, new Color(.07f, .08f, .10f, .9f));
            UiTheme.Border(_sortBox.rectTransform, new Color(.62f, .56f, .40f), 1);
            _sortBox.raycastTarget = false;
            _sortCheck = ForgeUi.Panel(_sortBox.transform, "Check", 3, 3, 8, 8, ForgeUi.Gold);
            _sortCheck.raycastTarget = false;
            _sortToggle.graphic = _sortCheck;
            _sortLabel = ForgeUi.Copy(_sortBackground.transform, "UI_TRAIN_SORT_UPGRADE_LABEL", 22, 0, 82, 32, 10);
            _sortLabel.text = _sortLabel.text.Replace("\\n", "\n");
            _sortLabel.textWrappingMode = TextWrappingModes.Normal;
            _sortLabel.overflowMode = TextOverflowModes.Overflow;
            _sortLabel.alignment = TextAlignmentOptions.Center;
            _sortLabel.raycastTarget = false;
            ForgeUi.Button(Root, "UI_FLOAT_CLOSE", 286, 5, 26, 26, () => _host.Side(FloatingInventoryHost.SideNone));
            _summary = ForgeUi.Text(Root, "TrainingSummary", "", 14, 41, 292, 26, 12, ForgeUi.Gold);
            _summary.alignment = TextAlignmentOptions.MidlineLeft;

            MakeGroupButton("UI_TRAIN_GROUP_CORE", 12, 76, 94, 0);
            MakeGroupButton("UI_TRAIN_GROUP_ATTUNE", 113, 76, 94, 1);
            MakeGroupButton("UI_TRAIN_GROUP_WARD", 214, 76, 94, 2);

            var view = ForgeUi.Rect(Root, "TrainingScroll", 12, 114, 296, 397);
            _content = UiScroll.Create(view, out _);
            _status = ForgeUi.Text(Root, "TrainingStatus", "", 12, 516, 296, 38, 12, UiTheme.TextMid);
            ForgeUi.Wrap(_status);
            Root.gameObject.SetActive(false);
            _sortToggle.isOn = false;
        }

        void MakeGroupButton(string key, float x, float y, float width, int group)
        {
            var button = ForgeUi.Button(Root, key, x, y, width, 29, () => SetGroup(group));
            var label=button.GetComponentInChildren<TMP_Text>();label.fontSizeMin=12;label.fontSizeMax=14;label.fontSize=14;
        }

        public void Bind()
        {
            if (!Root.gameObject.activeSelf) return;
            if (!_model.Available) { _summary.text = Loc.Get("UI_TRAIN_UNAVAILABLE"); _status.text = Loc.Get(_model.StatusKey); return; }

            _summary.text = Loc.Fmt("UI_TRAIN_HEADER", UiNumber.Exact(_model.Gil), _model.TotalLevels);
            var slots = _model.Slots.Where(s => GroupOf(s.id) == _group);
            if (_readyFirst)
                slots = slots.OrderByDescending(_model.CanTrain).ThenBy(s => Array.IndexOf(_model.Slots, s));
            var visibleSlots = slots.ToArray();
            while (_rows.Count < visibleSlots.Length) _rows.Add(new TrainingRow(_content, _model, id => _model.Train(id)));
            for (int i = 0; i < _rows.Count; i++)
            {
                bool visible = i < visibleSlots.Length;
                _rows[i].Root.gameObject.SetActive(visible);
                if (!visible) continue;
                _rows[i].Bind(visibleSlots[i]);
                ForgeUi.Place(_rows[i].Root, 0, i * TrainingRow.Height, 282, TrainingRow.Height - 8);
            }
            UiScroll.Fit(_content, visibleSlots.Length * TrainingRow.Height);
            _status.text = Loc.Get(_model.StatusKey);
        }

        void ToggleUpgradeFirst(bool value)
        {
            _readyFirst = value;
            RefreshSortButton();
            Bind();
        }

        void RefreshSortButton()
        {
            if (_sortToggle == null || _sortBackground == null) return;
            if (_sortToggle != null && _sortToggle.isOn != _readyFirst)
                _sortToggle.SetIsOnWithoutNotify(_readyFirst);
            if (_sortBackground != null)
                _sortBackground.color = _readyFirst ? new Color(.34f, .30f, .21f) : ForgeUi.Raised;
        }

        void SetGroup(int group)
        {
            if (_group == group) return;
            _group = group;
            Bind();
        }

        static int GroupOf(string id)
        {
            if (id != null && id.StartsWith("attune_", StringComparison.Ordinal)) return 1;
            if (id != null && id.StartsWith("ward_", StringComparison.Ordinal)) return 2;
            return 0;
        }
    }

    sealed class TrainingRow
    {
        public const float Height = 124;
        public readonly RectTransform Root;
        readonly TrainingModel _model;
        readonly Action<string> _train;
        readonly Image _icon;
        readonly TextMeshProUGUI _name, _level, _bonus, _gate;
        readonly Button _button;

        public TrainingRow(Transform parent, TrainingModel model, Action<string> train)
        {
            _model = model; _train = train;
            var panel = ForgeUi.Panel(parent, "TrainingRow", 0, 0, 282, Height - 8, UiTheme.Surface);
            Root = panel.rectTransform;
            UiTheme.Border(Root, new Color(.31f, .32f, .34f), 1);
            var iconRect = ForgeUi.Rect(Root, "Icon", 8, 12, 34, 34);
            _icon = iconRect.gameObject.AddComponent<Image>();
            _icon.preserveAspect = true;
            _icon.raycastTarget = false;
            _name = ForgeUi.Text(Root, "Name", "", 48, 7, 144, 24, 14, ForgeUi.Ink);
            _level = ForgeUi.Text(Root, "Level", "", 198, 7, 76, 24, 12, UiTheme.TextMid);
            _level.alignment=TextAlignmentOptions.Right;
            _bonus = ForgeUi.Text(Root, "Bonus", "", 48, 33, 224, 35, 12, ForgeUi.Gold);ForgeUi.Wrap(_bonus);
            _gate = ForgeUi.Text(Root, "Gate", "", 12, 77, 148, 30, 11, UiTheme.Warning);
            ForgeUi.Wrap(_gate);
            _button = ForgeUi.Button(Root, "UI_TRAIN_BUY", 170, 72, 104, 36, () => _train?.Invoke(_boundId), true);
            var text = _button.GetComponentInChildren<TMP_Text>();ForgeUi.Display(text,12);text.fontSizeMin=11;text.fontSizeMax=12;ForgeUi.Wrap(text);text.alignment=TextAlignmentOptions.Center;
        }

        string _boundId;

        public void Bind(TrainingModel.Slot slot)
        {
            _boundId = slot.id;
            _icon.sprite = TrainingIconAtlas.Get(slot.id);
            _icon.enabled = _icon.sprite != null;
            _name.text = Loc.Get("TRAIN_" + slot.id);
            _level.text = Loc.Fmt("UI_TRAIN_LEVEL", slot.level, slot.max_level);
            _bonus.text = Bonus(slot);
            _gate.text = Gate(slot);
            bool can = _model.CanTrain(slot);
            _button.interactable = can;
            var label = _button.GetComponentInChildren<TMP_Text>();
            label.text = slot.has_next_cost ? Loc.Get("UI_TRAIN_ACTION") + "\n" + UiNumber.Exact(slot.next_cost) + " " + Loc.Get("UI_GIL") : Loc.Get("UI_TRAIN_MAXED");
            label.color = can ? UiTheme.Accent : UiTheme.TextMid;
            _button.GetComponent<Image>().color = can ? UiTheme.SurfaceActive : UiTheme.Surface;
        }

        string Bonus(TrainingModel.Slot slot)
        {
            if (!string.IsNullOrEmpty(slot.text) && !string.IsNullOrEmpty(slot.text_per_level))
                return slot.text + "\n" + Loc.Fmt("UI_TRAIN_PER_LEVEL", slot.text_per_level);
            if (!string.IsNullOrEmpty(slot.text)) return slot.text;
            if (!string.IsNullOrEmpty(slot.text_per_level)) return Loc.Fmt("UI_TRAIN_PER_LEVEL", slot.text_per_level);
            // Satchel is intentionally not a StatMod in sim-core; keep its units explicit.
            return Loc.Fmt("UI_TRAIN_CAPACITY", slot.value_now, slot.per_level);
        }

        string Gate(TrainingModel.Slot slot)
        {
            if (!_model.Available || slot == null) return "";
            if (!slot.gate_open && slot.gate_total_needed > 0)
                return Loc.Fmt("UI_TRAIN_GATE", slot.gate_total_needed, _model.TotalLevels);
            string reason = _model.Reason(slot);
            if (reason == "UI_TRAIN_GIL_REQUIRED") return Loc.Get(reason);
            if (reason == "UI_TRAIN_MAXED") return Loc.Get(reason);
            return "";
        }
    }
}
