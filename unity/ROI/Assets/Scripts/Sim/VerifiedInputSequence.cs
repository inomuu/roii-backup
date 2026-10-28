using System;

namespace Roi.Sim
{
    public enum VerifiedInputResult { Applied, Refused, Stale, Unavailable }

    // A confirmation-time native preflight for a short sequence of deterministic inputs.
    // The copy is never attached, ticked, saved or published. The existing live session owns
    // every real input and its replay log; no synthetic state is installed into that session.
    internal static class VerifiedInputSequence
    {
        internal static VerifiedInputResult Prepare(SimSession live, string[] inputs,
            Func<SimSession, bool> verifyResult, out string startHash, out string finishHash)
        {
            startHash = finishHash = null;
            if (live == null || !live.IsAlive || inputs == null || inputs.Length == 0 || verifyResult == null)
                return VerifiedInputResult.Unavailable;
            startHash = live.StateHash();
            using (var copy = live.Clone())
            {
                if (copy == null || !copy.IsAlive) return VerifiedInputResult.Unavailable;
                // Keep the equality guard even with exact cloning: previews must prove the
                // result against the same state that receives the real input sequence.
                if (copy.StateHash() != startHash) return VerifiedInputResult.Unavailable;
                foreach (string input in inputs)
                    if (string.IsNullOrEmpty(input) || !copy.ApplyInput(input)) return VerifiedInputResult.Refused;
                if (!verifyResult(copy)) return VerifiedInputResult.Refused;
                finishHash = copy.StateHash();
            }
            return live.IsAlive && live.StateHash() == startHash ? VerifiedInputResult.Applied : VerifiedInputResult.Stale;
        }
    }
}
