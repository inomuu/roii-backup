using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // State-driven caption feedback. Preserve authored rarity/element colours and never poll.
    public sealed class UiStateButton : Button
    {
        protected override void Start()
        {
            base.Start();PaintCaptions(currentSelectionState,true);
        }
        protected override void DoStateTransition(SelectionState state,bool instant)
        {
            instant|=!UiEntrance.MotionAllowed;
            base.DoStateTransition(state,instant);
            PaintCaptions(state,instant);
            GetComponent<UiButtonFeedback>()?.RefreshState();
        }
        void PaintCaptions(SelectionState state,bool instant)
        {
            // Renderer tint multiplies, rather than overwrites, each label's semantic colour.
            Color tint=state==SelectionState.Disabled?UiTheme.DisabledCaption:Color.white;
            foreach(var label in GetComponentsInChildren<TextMeshProUGUI>(true))
                label.CrossFadeColor(tint,instant?0:UiTheme.HoverDuration,true,true);
        }
    }
}
