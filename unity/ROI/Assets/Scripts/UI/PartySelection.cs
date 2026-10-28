// WHO THE SCREENS ARE ABOUT. One number, shared, because a per-character screen is meaningless
// without it: a Cards panel, a stat sheet or a skill page all describe ONE member, and until now
// they all silently assumed the lead.
//
// It lives outside any one screen for the same reason `UiLayout` does — the moment two screens
// each keep their own idea of "the selected member", opening one and then the other shows you two
// different characters and neither is wrong.
namespace Roi.UI
{
    public static class PartySelection
    {
        /// 0 = nothing chosen yet, which resolves to the lead.
        public static long SelectedId { get; private set; }

        /// Raised when the choice changes. Screens subscribe rather than poll: the selection moves
        /// when a finger moves, not ten times a second.
        public static event System.Action Changed;

        public static void Select(long id)
        {
            if (SelectedId == id) return;
            SelectedId = id;
            Changed?.Invoke();
        }

        /// The id a screen should actually use, given who is currently on the field. Falls back to
        /// the first member rather than to nothing: a party always has someone, and a screen that
        /// showed an empty panel because the selected member was benched would look broken.
        public static long Resolve(System.Collections.Generic.IList<long> livePartyIds)
        {
            if (livePartyIds == null || livePartyIds.Count == 0) return 0;
            foreach (var id in livePartyIds) if (id == SelectedId) return id;
            SelectedId = livePartyIds[0];
            return SelectedId;
        }
    }
}
