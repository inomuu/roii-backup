// WHEN THE ARENA MAKES A NOISE. The trigger half of the audio port — `SoundFx` owns the voices,
// this owns the moments, and the tables in `SoundTable` own which sound each moment makes.
//
// It reads the SAME per-tick view the arena renders from, and it is fed by the arena for the same
// reason the overlay is: one parse, one order, and no second subscription that could see a
// different tick. Nothing here can reach the sim.
//
// FOUR MOMENTS, all copied from the Godot pair (views/arena.gd and proto/arena3d/arena3d_fx.gd):
//   BGM       — follows the BAND. `PlayBgm` de-dups, so calling it every tick is a no-op and only
//               a band change switches the track.
//   BOW SHOT  — a non-magic projectile whose `spawn_tick` is THIS tick has just been loosed.
//               Reading the SPAWN TICK rather than an animation frame keeps the sound on the
//               sim's timing rather than the sprite's.
//   ARROW HIT — arrows deal their damage on ARRIVAL, so they never appear in `recent_hits`. The
//               only signal is a projectile VANISHING, which means diffing the list.
//   SKILL     — on the RISING EDGE of `pending_skill`, the wind-up rather than the resolve,
//               delayed 0.35 s so it lands near contact. Cast-start alone is too early and the
//               resolve is too late.
//
// The monster hurt sound is bucketed like arena.gd's, not like arena3d_fx.gd's: the 3D file
// inlined `is_base_monster` as "a pet or anything not on the Player team", which is true of every
// monster, so its bucket is always "base" and the sound never plays. Unity renders every monster
// from the slime sheets today, which is the 2D rule's other branch — so every monster is "slime"
// here, and a "base" bucket arrives when the base-monster defs are rendered.
//
// NOT WIRED, for want of a signal rather than by choice: the WEAPON hit sound. The view carries
// `weapon_type` per entity, so it is one lookup away — but each of the two melee sounds is also
// the monster's hurt sound on the same frame, and stacking them without hearing the result is
// guesswork. It wants an ear, not another table.
using System.Collections.Generic;
using UnityEngine;

namespace Roi.Audio
{
    public sealed class ArenaSound : MonoBehaviour
    {
        SoundFx _fx;
        readonly Dictionary<long, string> _lastPending = new();
        readonly Dictionary<long, int> _lastProjectiles = new();   // key -> damage, for the diff
        long _tick;

        public static ArenaSound Create(Transform parent)
        {
            var go = new GameObject("ArenaSound");
            go.transform.SetParent(parent, false);
            var a = go.AddComponent<ArenaSound>();
            a._fx = SoundFx.Create(go.transform);
            foreach (var kv in SoundTable.Paths)
                a._fx.Register(kv.Key, kv.Value,
                               SoundTable.Gain.TryGetValue(kv.Key, out var g) ? g : 0f);
            foreach (var kv in SoundTable.Bgm) a._fx.Register(kv.Key, kv.Value);
            return a;
        }

        public void OnView(string json)
        {
            if (_fx == null) return;
            var v = JsonUtility.FromJson<SoundViewDTO>(json);
            if (v == null) return;
            _tick = v.tick;

            int band = BandOfZone(v.current_zone_id);
            if (SoundTable.BandBgm.TryGetValue(band, out var track)) _fx.PlayBgm(track, -18f);

            Shots(v);
            Arrows(v);
            Skills(v);
            Hurts(v);
        }

        /// "zone_b<band>_<room>" -> band. The renderer's `RegionOfZone` folds bands into regions;
        /// music is keyed by the BAND itself, so it needs the number before that fold.
        static int BandOfZone(string zoneId)
        {
            if (string.IsNullOrEmpty(zoneId)) return 0;
            int i = zoneId.IndexOf("_b", System.StringComparison.Ordinal);
            if (i < 0) return 0;
            int j = i + 2, band = 0;
            while (j < zoneId.Length && char.IsDigit(zoneId[j])) { band = band * 10 + (zoneId[j] - '0'); j++; }
            return band;
        }

        void Shots(SoundViewDTO v)
        {
            if (v.projectiles == null) return;
            foreach (var p in v.projectiles)
                if (!p.is_magic && p.spawn_tick == _tick)
                    _fx.Play("bow_shot", -12f, Random.Range(0.96f, 1.05f));
        }

        void Arrows(SoundViewDTO v)
        {
            var now = new Dictionary<long, int>();
            if (v.projectiles != null)
                foreach (var p in v.projectiles)
                {
                    if (p.is_magic) continue;
                    // Identity from what cannot change in flight. Two arrows to the same target on
                    // the same tick are one entry, which is also one sound — and that is right:
                    // they land together and a doubled sample is a flam, not two hits.
                    now[p.target_id * 1000003L + (long)p.spawn_tick] = p.damage;
                }
            foreach (var kv in _lastProjectiles)
                if (!now.ContainsKey(kv.Key) && kv.Value > 0)
                    _fx.Play("arrow_hit", -7f, Random.Range(0.96f, 1.05f));
            _lastProjectiles.Clear();
            foreach (var kv in now) _lastProjectiles[kv.Key] = kv.Value;
        }

        void Skills(SoundViewDTO v)
        {
            if (v.grid == null || v.grid.entities == null) return;
            foreach (var e in v.grid.entities)
            {
                if (e.team != "Player") continue;
                var pend = e.pending_skill ?? "";
                _lastPending.TryGetValue(e.id, out var was);
                if (pend != "" && pend != was
                    && SoundTable.SkillHit.TryGetValue(pend, out var key))
                    _fx.PlayDelayed(key, 0.35f, -8f, Random.Range(0.97f, 1.04f));
                _lastPending[e.id] = pend;
            }
        }

        void Hurts(SoundViewDTO v)
        {
            if (v.recent_hits == null || v.grid == null || v.grid.entities == null) return;
            foreach (var h in v.recent_hits)
            {
                if (h.amount <= 0) continue;
                foreach (var e in v.grid.entities)
                {
                    if (e.id != h.target_id) continue;
                    if (e.team == "Player" || e.is_pet) break;
                    if (SoundTable.MonsterHit.TryGetValue("slime", out var key))
                        _fx.Play(key, -18f, Random.Range(0.94f, 1.08f));
                    break;
                }
            }
        }

        // Its OWN DTOs, declared narrowly. JsonUtility drops what is not declared, so this reads
        // four fields out of a payload the arena reads forty from — and neither has to know what
        // the other wanted.
        [System.Serializable] class SoundViewDTO
        {
            public long tick; public string current_zone_id;
            public SoundGridDTO grid; public SoundHitDTO[] recent_hits; public ProjDTO[] projectiles;
        }
        [System.Serializable] class SoundGridDTO { public SoundEntDTO[] entities; }
        [System.Serializable]
        class SoundEntDTO
        {
            public long id; public string team; public bool is_pet;
            public string pending_skill, weapon_type;
        }
        [System.Serializable] class SoundHitDTO { public long target_id; public long amount; }
        [System.Serializable]
        class ProjDTO { public long target_id; public long spawn_tick; public bool is_magic; public int damage; }
    }
}
