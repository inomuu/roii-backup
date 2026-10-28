using System;
using System.Collections.Generic;
using Roi.Sim;

namespace Roi.UI
{
    // Verifies the host's RESULT, not its equip rules or stat formula. Canonical item payloads
    // include rolled affixes, refinement, binding and instance ids, including legacy null ids.
    internal static class ExemplarEquipmentTransfer
    {
        internal static bool Verify(SimSession copy, ExemplarEquipmentActions.Snapshot before,
            ExemplarEquipmentActions.Row chosen, string targetSlot)
        {
            var after = ExemplarEquipmentActions.Read(copy, before.MemberId);
            if (!after.Available || after.UnitDef != before.UnitDef || after.Equipped.Length == 0) return false;
            if (after.InventoryFingerprint == before.InventoryFingerprint
                && after.EquippedFingerprint == before.EquippedFingerprint) return false;
            // Native EquipItem appends the accepted piece. A refused input must not be mistaken
            // for another identical item already worn by the recipient.
            var placed = after.Equipped[after.Equipped.Length - 1];
            if (!string.IsNullOrEmpty(targetSlot) && placed.Slot != targetSlot) return false;
            string identity = Identity(chosen.Raw);
            if (string.IsNullOrEmpty(identity) || Identity(placed.Raw) != identity) return false;
            var expected = new List<string>();
            foreach (var row in before.Equipped)
                if (row.Slot != placed.Slot) expected.Add(row.Raw);
            if (after.Equipped.Length != expected.Count + 1) return false;
            for (int i = 0; i < expected.Count; i++) if (after.Equipped[i].Raw != expected[i]) return false;
            if (chosen.OwnerId > 0)
            {
                expected.Clear();
                foreach (var row in before.Candidates)
                    if (row.OwnerId == chosen.OwnerId && row.Index != chosen.Index) expected.Add(row.Raw);
                if (!after.OwnerFingerprints.TryGetValue(chosen.OwnerId, out var source)
                    || source != "[" + string.Join(",", expected) + "]") return false;
            }
            // Every copy must still exist exactly once across the bag and all live loadouts.
            // Ignoring only the host's placement stamp accommodates accessories/off-hand gear;
            // it does not collapse different rolled/refined/bound copies of one item definition.
            var counts = new Dictionary<string, int>(StringComparer.Ordinal);
            foreach (var row in before.Candidates)
            {
                string key = Identity(row.Raw);
                counts.TryGetValue(key, out int n); counts[key] = n + 1;
            }
            foreach (var row in after.Candidates)
            {
                string key = Identity(row.Raw);
                if (!counts.TryGetValue(key, out int n) || n == 0) return false;
                counts[key] = n - 1;
            }
            foreach (var count in counts.Values) if (count != 0) return false;
            // A successful transfer removes one source copy; a successful bag equip consumes
            // the selected row even when the recipient already wore an indistinguishable copy.
            int displaced = before.Equipped.Length - expectedTargetCount(before, placed.Slot);
            return after.Inventory.Length == before.Inventory.Length + (chosen.OwnerId > 0 ? 0 : -1) + displaced;
        }

        static int expectedTargetCount(ExemplarEquipmentActions.Snapshot before, string slot)
        {
            int count = 0;
            foreach (var row in before.Equipped) if (row.Slot != slot) count++;
            return count;
        }

        // Split only top-level JSON properties, respecting escaped strings and nested values.
        // The native projection writes canonical property order, so preserving every remaining
        // byte makes this comparison stronger than reserializing the lossy ItemView DTO.
        static string Identity(string json)
        {
            if (string.IsNullOrEmpty(json) || json[0] != '{' || json[json.Length - 1] != '}') return "";
            var fields = new List<string>();
            int start = 1, depth = 0;
            bool quoted = false, escaped = false;
            for (int i = 1; i < json.Length; i++)
            {
                char c = json[i];
                if (escaped) { escaped = false; continue; }
                if (quoted && c == '\\') { escaped = true; continue; }
                if (c == '"') { quoted = !quoted; continue; }
                if (quoted) continue;
                if (c == '{' || c == '[') depth++;
                else if ((c == '}' && i != json.Length - 1) || c == ']') depth--;
                if (depth != 0 || (c != ',' && i != json.Length - 1)) continue;
                string field = json.Substring(start, i - start).Trim(); start = i + 1;
                if (!field.StartsWith("\"slot\":", StringComparison.Ordinal)
                    && !field.StartsWith("\"equip_slot\":", StringComparison.Ordinal)) fields.Add(field);
            }
            return quoted || depth != 0 ? "" : string.Join(",", fields);
        }
    }
}
