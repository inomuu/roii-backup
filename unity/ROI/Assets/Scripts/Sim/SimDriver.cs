// The ONE owner of the sim handle (the Unity counterpart of godot/sim_driver_core.gd, P0 subset).
// Ticks the sim at a fixed 10 Hz on an unscaled-time accumulator so the cadence is independent of
// frame rate (idle-open at 5 fps still advances 10 ticks/s), publishes the view JSON after each
// tick batch, and requests a save bundle every SaveInterval seconds. Views subscribe to the
// events; they never touch the handle. Online push/confirm (P3) plugs into SaveRequested.
using System;
using System.Collections.Generic;
using UnityEngine;

namespace Roi.Sim
{
    public sealed class SimDriver : MonoBehaviour
    {
        public const float TickInterval = 0.1f;   // 1 tick = 100 ms, as in sim-core
        public const int MaxTicksPerFrame = 50;   // catch-up cap after a stall (5 s); beyond that the
                                                  // rest is dropped, like SPAN_STALL freezing in Godot
        public float SaveInterval = 10f;
        public const long MaxUnsavedTicks = 3000;
        public bool RequireVerifiedSaves { get; set; }
        public bool WaitingForSave => RequireVerifiedSaves && Session != null
            && Session.ReplaySpan() >= MaxUnsavedTicks;

        public event Action<string> StateUpdated;   // view JSON, after each tick batch
        public event Action<string> SaveRequested;  // SaveBundle JSON (export_log); confirm -> CommitAnchor()

        public SimSession Session { get; private set; }
        public ulong Tick { get; private set; }

        float _acc;
        float _sinceSave;

        /// Presentation pause — `sim_driver_core.gd::pause_sim_for_cutin`, ported.
        ///
        /// It blocks ONLY the tick loop, and it does NOT bank time while held. That second half is
        /// the whole trick and it is easy to get wrong: a pause that keeps accumulating fires the
        /// backlog as one batch the moment it lifts, which jumps the sim forward — defeating the
        /// point, and reading to the server's tick-rate cap as a fast-forward. Views keep the last
        /// published state, so whatever is holding the sim still sees a live scene.
        ///
        /// Saves continue on wall time while presentation holds the simulation.
        [SerializeField] bool _manualPause;
        [SerializeField] List<UnityEngine.Object> _presentationHolds = new List<UnityEngine.Object>();
        public bool Paused
        {
            get
            {
                for (int i = _presentationHolds.Count - 1; i >= 0; i--)
                    if (_presentationHolds[i] == null) _presentationHolds.RemoveAt(i);
                return _manualPause || _presentationHolds.Count > 0;
            }
            set => _manualPause = value;
        }

        // Independent owners cannot resume each other's pause (for example Break and a cut-in).
        public void HoldPresentation(UnityEngine.Object owner)
        {
            if (owner != null && !_presentationHolds.Contains(owner)) _presentationHolds.Add(owner);
        }

        public void ReleasePresentation(UnityEngine.Object owner) => _presentationHolds.Remove(owner);

        public void Attach(SimSession s)
        {
            if (ReferenceEquals(Session, s)) return;
            Session?.Dispose();
            Session = s;
            _manualPause = false;
            _presentationHolds.Clear();
            _acc = 0f; _sinceSave = 0f; Tick = 0;
            Publish();
        }

        public bool ApplyInput(string eventJson)
        {
            if (Session == null || !Session.IsAlive || WaitingForSave) return false;
            var ok = Session.ApplyInput(eventJson);
            if (ok) Publish();
            return ok;
        }

        /// Confirm-time host validation followed by synchronous replay on the identical live
        /// state. No tick, save callback or UI publication can interleave the real inputs.
        public VerifiedInputResult ApplyVerifiedInputs(SimSession expectedSession, string[] inputs,
            Func<SimSession, bool> verifyResult)
        {
            if (WaitingForSave) return VerifiedInputResult.Unavailable;
            if (!ReferenceEquals(Session, expectedSession)) return VerifiedInputResult.Stale;
            var frozenInputs = inputs == null ? null : (string[])inputs.Clone();
            var result = VerifiedInputSequence.Prepare(Session, frozenInputs, verifyResult,
                out string startHash, out string finishHash);
            if (result != VerifiedInputResult.Applied) return result;
            if (!ReferenceEquals(Session, expectedSession) || Session == null || !Session.IsAlive
                || Session.StateHash() != startHash) return VerifiedInputResult.Stale;
            // Deliberately avoid ApplyInput here: it publishes after EACH input, which could
            // re-enter another action halfway through removing and equipping a shared piece.
            foreach (string input in frozenInputs)
                if (!Session.ApplyInput(input))
                {
                    UnityEngine.Debug.LogError("A native input refused after identical-state preflight.");
                    Publish(); return VerifiedInputResult.Unavailable;
                }
            bool matches = Session.StateHash() == finishHash;
            if (!matches) UnityEngine.Debug.LogError("Native input sequence diverged from its preflight.");
            Publish();
            return matches ? VerifiedInputResult.Applied : VerifiedInputResult.Unavailable;
        }

        /// Call after the server VERIFIED the bundle handed out by SaveRequested.
        public void CommitAnchor() => Session?.CommitAnchor();

        void Update()
        {
            Advance(Time.unscaledDeltaTime);
        }

        // Save retries use wall time, including while progression is waiting for a verified save.
        void Advance(float delta)
        {
            if (Session == null || !Session.IsAlive) return;
            _sinceSave += delta;
            if (!Paused && !WaitingForSave)
            {
                _acc += delta;
                int n = Math.Min((int)(_acc / TickInterval), MaxTicksPerFrame);
                if (RequireVerifiedSaves)
                    n = (int)Math.Min(n, Math.Max(0L, MaxUnsavedTicks - Session.ReplaySpan()));
                _acc -= n * TickInterval;
                if (_acc > TickInterval * MaxTicksPerFrame) _acc = 0f;
                if (n > 0)
                {
                    Session.Tick((ulong)n);
                    Tick += (ulong)n;
                    Publish();
                }
            }
            else _acc = 0f;
            if (_sinceSave >= SaveInterval)
            {
                _sinceSave = 0f;
                var bundle = Session.ExportLog();
                if (bundle != null) SaveRequested?.Invoke(bundle);
            }
        }

        void Publish()
        {
            var handler = StateUpdated;
            if (handler != null) handler(Session != null ? Session.ViewJson() : "{}");
        }

        void OnDestroy()
        {
            Session?.Dispose();
            Session = null;
        }
    }
}
