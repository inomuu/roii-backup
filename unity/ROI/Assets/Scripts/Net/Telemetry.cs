// TELEMETRY — the CLIENT half of the record. Ported from godot/net/telemetry.gd.
//
// WHAT IT IS FOR. The server already records everything it can read off a save it replayed: level,
// zone, gil, grants issued. It cannot see a session that never saved, a screen someone opened and
// backed out of, or the frame rate on their machine. Those are the questions that decide whether
// HD-2D survives and whether the store converts, and nobody can answer them from here.
//
// IT MUST NEVER COST A FRAME OR A REQUEST. Names go into a list; one POST goes out per flush
// carrying the batch. The server folds it into the same in-memory accumulator the save path uses
// and writes nothing, so the added DB write rate is zero.
//
// IT MUST NEVER BREAK A SESSION. Offline is not an error, it is the normal state before login: the
// buffer fills to a cap and stops. A failed POST is DROPPED rather than retried — these are trend
// counters, and a retry queue would be a second thing to get wrong for numbers that tolerate loss
// by design.
using System.Collections;
using System.Collections.Generic;
using UnityEngine;

namespace Roi.Net
{
    public sealed class Telemetry : MonoBehaviour
    {
        /// Long, because nothing here is time-sensitive and every send is a TLS round trip on
        /// someone's laptop.
        const float FlushSecs = 60f;
        /// Reached only while offline or while sends are failing; past it, names are dropped.
        /// Bounded loss beats a list that grows for the length of a session left open all day.
        const int BufCap = 200;

        /// Frame-rate BUCKETS, high to low. Buckets rather than a number because a number would be
        /// a free-form counter key, and the server's allow-list exists to stop exactly that.
        static readonly (float floor, string name)[] FpsBuckets =
            { (50f, "fps.over50"), (30f, "fps.30to50"), (0f, "fps.under30") };

        static Telemetry _instance;

        ServerClient _server;
        readonly List<string> _buf = new();
        float _since;
        bool _sending;
        int _dropped;
        int _frames;
        float _frameTime;

        public static Telemetry Create(Transform parent)
        {
            if (_instance != null) return _instance;
            var go = new GameObject("Telemetry");
            go.transform.SetParent(parent, false);
            _instance = go.AddComponent<Telemetry>();
            return _instance;
        }

        /// Called when a session goes online, and with null when it goes offline.
        public static void Attach(ServerClient server)
        {
            if (_instance != null) _instance._server = server;
        }

        /// Record one event. The name must be in `metrics::client::ALLOWED` on the server; an
        /// unrecognised one is not silently discarded there, it lands on `client.unknown`, which is
        /// how a rename shows up instead of reading zero forever.
        public static void Note(string name)
        {
            if (_instance == null || string.IsNullOrEmpty(name)) return;
            if (_instance._buf.Count >= BufCap) { _instance._dropped++; return; }
            _instance._buf.Add(name);
        }

        void OnDestroy() { if (_instance == this) _instance = null; }

        void Update()
        {
            // Averaged over the window rather than read per frame: one reading a minute is enough
            // to bucket a machine, and `1/deltaTime` on any single frame is noise.
            _frames++;
            _frameTime += Time.unscaledDeltaTime;

            _since += Time.unscaledDeltaTime;
            if (_since < FlushSecs) return;
            _since = 0f;
            NoteFps(_frameTime > 0f ? _frames / _frameTime : 0f);
            _frames = 0; _frameTime = 0f;
            Note("session.heartbeat");
            if (!_sending) StartCoroutine(Flush());
        }

        void NoteFps(float fps)
        {
            foreach (var b in FpsBuckets)
                if (fps >= b.floor) { Note(b.name); return; }
        }

        /// Send and forget. `_sending` is not a nicety: a flush that took longer than the interval
        /// would otherwise overlap itself and send the same names twice.
        IEnumerator Flush()
        {
            if (_buf.Count == 0 || _server == null || !_server.LoggedIn) yield break;
            if (_dropped > 0)
            {
                // The count itself is not reportable — there is no counter for it and inventing
                // one would be a free-form key. A warning is the honest channel: it means the cap
                // is too low or a send path is broken, and both are our bug, not the player's.
                Debug.LogWarning($"TELEMETRY: dropped {_dropped} events at the buffer cap");
                _dropped = 0;
            }
            var batch = _buf.ToArray();
            _buf.Clear();
            _sending = true;
            // Awaited but not acted on. There is nothing useful to do with a failure: the names
            // are already out of the buffer, and re-queueing them is the retry queue this
            // deliberately has no room for.
            yield return _server.Telemetry(batch, null);
            _sending = false;
        }

        /// FOR A PROBE. Sending is a minute away by design, and a test that waited that long would
        /// be measuring the clock rather than the wire.
        public IEnumerator FlushNow() => Flush();

        public int Buffered => _buf.Count;
    }
}
