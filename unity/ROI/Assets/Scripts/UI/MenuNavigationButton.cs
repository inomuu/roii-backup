using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Menu-only chrome: the entire row is clickable, but only interaction gets a surface.
    public sealed class MenuNavigationButton : Button
    {
        public TextMeshProUGUI Caption;
        public Image AccentMark;
        public bool IsTab;
        public Graphic CategoryIcon;
        bool _active;

        public void SetActiveState(bool active)
        {
            _active = active;
            DoStateTransition(currentSelectionState, true);
        }

        protected override void DoStateTransition(SelectionState state, bool instant)
        {
            bool enabled = IsInteractable();
            bool focus = enabled && (state == SelectionState.Highlighted || state == SelectionState.Selected || state == SelectionState.Pressed);
            bool lit = enabled && (_active || focus);
            if (targetGraphic != null)
                targetGraphic.CrossFadeColor(lit ? UiTheme.WithAlpha(UiTheme.SurfaceActive, state == SelectionState.Pressed ? .65f : .28f) : Color.clear,
                    instant ? 0 : .12f, true, true);
            if (Caption != null) Caption.color = !enabled ? UiTheme.TextLow : lit ? UiTheme.Accent : IsTab ? UiTheme.TextMid : UiTheme.TextHigh;
            if (CategoryIcon != null) CategoryIcon.color = !enabled ? UiTheme.TextLow : lit ? UiTheme.Accent : UiTheme.TextMid;
            if (AccentMark != null) AccentMark.enabled = enabled && _active;
        }
    }
}
