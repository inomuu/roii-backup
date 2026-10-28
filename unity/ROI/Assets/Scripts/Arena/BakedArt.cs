// THE AUTHORED SCENE, PARKED AND RESTORED.
//
// A baked zone ships its own ground, grass, flora, dressing and scenery in the scene file. Every
// builder in `ArenaView` finds its object with `_fieldRoot.Find(name)` and DESTROYS what it finds,
// and the scene uses those same names — so the first region change deleted the authored zone, and
// coming home rebuilt a procedural copy over whatever scenery had survived. That is what read as
// two maps drawn on top of each other.
//
// `Find` searches DIRECT CHILDREN ONLY. One extra level is therefore a wall: parked under a holder,
// the authored art is invisible to every builder, cannot be destroyed by one, and comes back
// exactly as it was authored — including the several pieces that ship switched OFF, which is why
// each object's own state is remembered rather than everything being turned on together.
using System.Collections.Generic;
using UnityEngine;

namespace Roi.Arena
{
    /// Owned by `ArenaView`; knows nothing about zones or regions, only about holding a set of
    /// transforms out of the way and putting them back.
    public sealed class BakedArt
    {
        /// The names the ground builders create and destroy. A parked scene cannot collide with
        /// these, which is the whole point; the list is here so `KillBuilt` knows what a build
        /// leaves behind when the party comes home to a zone that draws itself.
        static readonly string[] Builds =
        {
            "Ground", "GroundQuad", "Surround", "WetSheen", "Dressing", "GrassField", "Flora",
            "Atmosphere",
        };

        readonly List<(Transform t, bool on)> _art = new();
        Transform _field, _hold;
        bool _shown = true;

        public bool Any => _art.Count > 0;

        /// Take the register. Call while the Field holds nothing but what the scene authored —
        /// before any actor or built ground joins it, or they end up parked too.
        public void Capture(Transform field)
        {
            _field = field;
            _art.Clear();
            _hold = null;
            _shown = true;
            if (field == null) return;
            foreach (Transform c in field) _art.Add((c, c.gameObject.activeSelf));
        }

        /// `true` puts the authored art back under the Field; `false` parks it beyond `Find`.
        public void Show(bool on)
        {
            if (on == _shown || _field == null) return;
            _shown = on;
            if (_hold == null)
            {
                var go = new GameObject("BakedArtHold");
                go.transform.SetParent(_field, false);
                go.SetActive(false);
                _hold = go.transform;
            }
            foreach (var (t, was) in _art)
            {
                if (t == null) continue;
                t.SetParent(on ? _field : _hold, false);
                t.gameObject.SetActive(on && was);
            }
        }

        /// Take down a ground some OTHER region built. Safe to call whatever is parked: while the
        /// authored art is away, everything under these names belongs to the builder.
        public void KillBuilt(System.Action<Object> kill)
        {
            if (_field == null) return;
            foreach (var n in Builds)
            {
                var t = _field.Find(n);
                if (t != null && t != _hold) kill(t.gameObject);
            }
        }
    }
}
