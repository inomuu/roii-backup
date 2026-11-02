// Shared bounds for management screens, which start at the top edge.
// The combat roster is visible only outside these screens.
using UnityEngine;

namespace Roi.UI
{
    public static class UiLayout
    {
        /// The combat bar, along the bottom. Taller than the tabs: it carries a health bar, room
        /// for the skill charge to come, and targets sized for a thumb.
        public const float RosterH = 96f;
        /// The member tabs, along the top of an open screen. A tab is a name and a level.
        public const float TabsH = 60f;
        /// Management screens reach the top edge; member selection lives within each header.
        public const float Top = 0f;
        /// Chrome reserved at the bottom: the action bar. The roster docks above it in combat, but
        /// no window is open then, so a window never has to account for it.
        public const float BottomBar = 76f;
        /// Space a management window leaves at the bottom. Zero: the action bar is hidden while a
        /// window is open (HudBar.SyncActiveStates), so reserving BottomBar left an empty strip.
        public const float WindowBottom = 0f;

        /// A window is FULL-BLEED horizontally. Kept as a named zero rather than deleted: the
        /// argument for margins comes back every time someone sees the layout on a wide monitor,
        /// and this is where the answer lives.
        public const float SidePad = 0f;
        public const float EdgeGap = 0f;
        /// Default share of the width a side inset may eat, for a view that asks for one.
        public const float SideFactor = 0.15f;

        /// The rect a window should occupy, in screen pixels with y UP (Unity's convention; the
        /// Godot original measures y down and is converted here, once, rather than at each caller).
        public static Rect ContentRect(Vector2 vp, float sidePad = SidePad, float sideFactor = SideFactor,
                                       float extraTop = 0f)
        {
            float x = sidePad > 0f ? Mathf.Min(sidePad, vp.x * sideFactor) : 0f;
            float top = Top + extraTop;
            // Max: a window on a very short viewport would otherwise be handed a negative height,
            // which draws inside-out.
            float h = Mathf.Max(0f, vp.y - WindowBottom - top);
            return new Rect(x, WindowBottom, vp.x - 2f * x, h);
        }

        /// Y of the top of the action bar — for things that DOCK to it rather than fill the screen.
        public static float BottomBarY(Vector2 vp) => BottomBar;

        /// The smallest a tappable thing may be. 44 is the figure both Apple and Google publish for
        /// a finger, and the reason every control here grew: a 30 px button is fine with a mouse
        /// and a coin-flip with a thumb.
        public const float Touch = 44f;
    }
}
