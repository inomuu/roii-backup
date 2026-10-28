using System;
using System.Collections.Generic;
using System.Linq;
using Roi.Sim;
using UnityEngine;

namespace Roi.UI
{
    /// Presentation read model for the party-wide Training ladder. Rules and costs are emitted
    /// by sim-host; this class deliberately contains no balance formula or client-side purchase.
    public sealed class TrainingModel : MonoBehaviour
    {
        [Serializable] public sealed class Slot
        {
            public string id, stat, element, text, text_per_level;
            public int per_level, max_level, level, value_now, gate_total_needed;
            public ulong base_gil, cost_mult_bp, next_cost;
            public bool gate_open;
            [NonSerialized] public bool has_next_cost;
        }

        [Serializable] sealed class Rules
        {
            public int levels_bought, total_levels, gate_step, gate_total_per_step, stash_slots;
            public ulong[] cost_curve_bp;
            public Slot[] slots;

            public Slot Find(string id)
                => slots?.FirstOrDefault(s => string.Equals(s.id, id, StringComparison.Ordinal));
        }

        [Serializable] sealed class Purse { public ulong gil, soul; }

        public SimDriver Driver { get; private set; }
        public Slot[] Slots { get; private set; } = Array.Empty<Slot>();
        public int TotalLevels { get; private set; }
        public int StashSlots { get; private set; }
        public ulong Gil { get; private set; }
        public bool Available { get; private set; }
        public bool Busy { get; private set; }
        public string StatusKey { get; private set; } = "UI_TRAIN_SELECT";
        public event Action Changed;

        SimSession _session;
        string _rawRules;

        public void Bind(SimDriver driver)
        {
            Driver = driver;
            Refresh();
        }

        public bool Refresh()
        {
            var session = Driver == null ? null : Driver.Session;
            if (!ReferenceEquals(session, _session))
            {
                _session = session;
                Busy = false;
                StatusKey = "UI_TRAIN_SELECT";
            }
            if (session == null || !session.IsAlive)
            {
                Available = false;
                Slots = Array.Empty<Slot>();
                TotalLevels = StashSlots = 0;
                Gil = 0;
                _rawRules = null;
                return false;
            }

            _rawRules = session.QueryJson("training_rules");
            var rules = Parse<Rules>(_rawRules);
            var purse = Parse<Purse>(session.QueryJson("purse"));
            Available = rules != null && rules.slots != null && rules.slots.Length > 0;
            Slots = rules?.slots ?? Array.Empty<Slot>();
            foreach (var slot in Slots)
                slot.has_next_cost = HasNumber(_rawRules, slot.id, "next_cost");
            TotalLevels = rules?.total_levels ?? 0;
            StashSlots = rules?.stash_slots ?? 0;
            Gil = purse?.gil ?? 0;
            return Available;
        }

        public Slot Find(string id)
            => Slots.FirstOrDefault(s => string.Equals(s.id, id, StringComparison.Ordinal));

        public bool CanTrain(Slot slot)
            => Available && !Busy && slot != null && slot.level < slot.max_level
                && slot.gate_open && slot.has_next_cost && Gil >= slot.next_cost;

        public string Reason(Slot slot)
        {
            if (!Available) return "UI_TRAIN_UNAVAILABLE";
            if (slot == null) return "UI_TRAIN_UNAVAILABLE";
            if (slot.level >= slot.max_level || !slot.has_next_cost) return "UI_TRAIN_MAXED";
            if (!slot.gate_open && slot.gate_total_needed > 0) return "UI_TRAIN_GATE_REQUIRED";
            if (Gil < slot.next_cost) return "UI_TRAIN_GIL_REQUIRED";
            return "UI_TRAIN_READY";
        }

        public void Train(string id)
        {
            if (Busy || !Refresh()) return;
            var slot = Find(id);
            if (slot == null) { StatusKey = "UI_TRAIN_UNAVAILABLE"; Changed?.Invoke(); return; }
            if (!CanTrain(slot)) { StatusKey = Reason(slot); Changed?.Invoke(); return; }

            var session = _session;
            int beforeLevel = slot.level;
            string input = "{\"Train\":{\"slot_id\":" + ForgeModel.QuoteString(id) + "}}";
            Busy = true;
            StatusKey = "UI_TRAIN_PENDING";
            Changed?.Invoke();

            var result = Driver.ApplyVerifiedInputs(session, new[] { input }, copy =>
            {
                var after = Read(copy.QueryJson("training_rules"));
                var trained = after?.Find(id);
                // A true input acceptance is not enough: the event is allowed to no-op for
                // stale funds/gates, so verify the level actually rose in the clone.
                return trained != null && trained.level == beforeLevel + 1;
            });

            Busy = false;
            Refresh();
            StatusKey = result == VerifiedInputResult.Applied ? "UI_TRAIN_DONE" : "UI_TRAIN_STALE";
            Changed?.Invoke();
        }

        static Rules Read(string raw)
        {
            var rules = Parse<Rules>(raw);
            if (rules?.slots == null) return rules;
            foreach (var slot in rules.slots)
                slot.has_next_cost = HasNumber(raw, slot.id, "next_cost");
            return rules;
        }

        static bool HasNumber(string raw, string id, string property)
        {
            if (string.IsNullOrEmpty(raw) || string.IsNullOrEmpty(id)) return false;
            string marker = "\"id\":\"" + id + "\"";
            int start = raw.IndexOf(marker, StringComparison.Ordinal);
            if (start < 0) return false;
            int next = raw.IndexOf("\"id\":\"", start + marker.Length, StringComparison.Ordinal);
            int end = next < 0 ? raw.Length : next;
            int prop = raw.IndexOf("\"" + property + "\"", start, end - start, StringComparison.Ordinal);
            if (prop < 0) return false;
            int colon = raw.IndexOf(':', prop);
            if (colon < 0 || colon >= end) return false;
            int p = colon + 1;
            while (p < end && char.IsWhiteSpace(raw[p])) p++;
            return p < end && !raw.Substring(p, Math.Min(4, end - p)).StartsWith("null", StringComparison.Ordinal);
        }

        static T Parse<T>(string raw) where T : class
        {
            if (string.IsNullOrEmpty(raw) || raw == "{}") return null;
            try { return JsonUtility.FromJson<T>(raw); }
            catch (ArgumentException) { return null; }
        }
    }
}
