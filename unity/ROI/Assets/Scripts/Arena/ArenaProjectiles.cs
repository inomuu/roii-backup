// THINGS IN FLIGHT. Ported from the projectile half of views/arena_fx.gd.
//
// The client has been firing arrows and bolts with nothing on screen: the sim has carried
// `projectiles` in the view all along, and the only thing reading it here was the SOUND layer. An
// archer shot, a sound played, and the target lost health with nothing between the two.
//
// EVERYTHING IS THE SIM'S. Where it starts, where it lands, when it arrives — a projectile's whole
// path is `spawn_pos → target_pos` across `spawn_tick → arrive_at_tick`, so this interpolates and
// invents none of it. A renderer that timed its own flight would drift out of step with the damage.
//
// WHAT DECIDES THE LOOK, in the order the Godot file decides it:
//   sky_fall  — a multi-bolt SKILL rains from above rather than flying from the caster.
//   is_magic  — a bolt (animated fireball frames, element-tinted); otherwise an arrow.
//   is_basic  — a skill cast is bigger than a basic attack. Not an `is_magic` sniff: those are
//               different questions and the sim answers both.
//   aoe_radius — a blast bolt is 3x, so what is coming is legible before it lands.
//
// The muzzle offsets are in CELLS, which is why they cross engines at all: a basic bolt leaves the
// staff tip, lifted and pushed out to the target's side. Pixel offsets would have been meaningless
// here (see the note on spatial values never copying between renderers).
using System.Collections.Generic;
using UnityEngine;

namespace Roi.Arena
{
    public sealed class ArenaProjectiles : MonoBehaviour
    {
        public void DelayPresentation(float seconds)
        {
            foreach (var shot in _live.Values) shot.T0+=seconds;
        }
        // --- the numbers, all from views/render_data.gd -----------------------------------
        const int ArrowFrames = 5;               // 160x32 sheet, 5 frames of 32
        const float ArrowDisplayCells = 1.2f;
        const int FireballRow = 7, FireballFlyFrames = 4, PlayerFramePx = 100;
        const float FireballFlyFps = 14f, FireballAoeScale = 3f;
        const float MagicMuzzleLift = 0.42f, MagicMuzzleFwd = 0.87f;
        /// How high a bolt flies. A 2D renderer has no answer to this and did not need one; here a
        /// projectile at ground level disappears into the floor.
        const float FlyHeight = 0.55f;
        const float SkyFallCells = 5f;

        /// Element -> tint. Only three are painted, exactly as the Godot table is: the rest fly
        /// white, which is the arrow's own colour and the fireball's own art.
        static readonly Dictionary<string, Color> BoltTint = new()
        {
            ["Holy"] = new Color(1.35f, 1.30f, 0.85f),
            ["Light"] = new Color(1.35f, 1.30f, 0.85f),
            ["Water"] = new Color(0.70f, 0.95f, 1.35f),
            ["Wind"] = new Color(0.75f, 1.30f, 0.80f),
        };

        ArenaView _arena;
        Camera _cam;
        Material _mat;
        Sprite _arrow;
        Sprite[] _fireball;
        readonly Dictionary<long, Shot> _live = new();
        readonly List<long> _gone = new();

        sealed class Shot
        {
            public GameObject Go;
            public SpriteRenderer Sr;
            public Vector3 From, To;
            public float T0, Dur;
            public bool Magic, Sky;
            public float Scale;
            public Color Tint;
        }

        public static ArenaProjectiles Create(Transform parent, ArenaView arena, Camera cam, Material mat)
        {
            var go = new GameObject("Projectiles");
            go.transform.SetParent(parent, false);
            var p = go.AddComponent<ArenaProjectiles>();
            p._arena = arena;
            p._cam = cam;
            p._mat = mat;
            return p;
        }

        public void OnView(string json, float tickDur)
        {
            var v = JsonUtility.FromJson<ProjView>(json);
            if (v == null) return;
            var seen = new HashSet<long>();

            if (v.projectiles != null)
                foreach (var p in v.projectiles)
                {
                    long key = p.target_id * 1000003L + (long)p.spawn_tick;
                    seen.Add(key);
                    if (_live.ContainsKey(key)) continue;
                    var shot = Spawn(p, v.tick, tickDur);
                    if (shot != null) _live[key] = shot;
                }

            // A projectile that VANISHED has arrived — the sim removes it on the tick it resolves.
            // That is also the only signal there is: arrival damage does not appear in
            // `recent_hits`, which is why the sound layer diffs the same list.
            _gone.Clear();
            foreach (var kv in _live) if (!seen.Contains(kv.Key)) _gone.Add(kv.Key);
            foreach (var k in _gone)
            {
                if (_live[k].Go != null) Destroy(_live[k].Go);
                _live.Remove(k);
            }
        }

        Shot Spawn(Proj p, long tick, float tickDur)
        {
            if (_arena == null) return null;
            var from = _arena.CellToWorldPublic(p.spawn_pos.x, p.spawn_pos.y);
            var to = _arena.CellToWorldPublic(p.target_pos.x, p.target_pos.y);
            from.y = to.y = FlyHeight;

            if (p.is_magic && !p.sky_fall)
            {
                // The staff tip: a fixed lift, and a push toward the target's SIDE rather than
                // along the aim vector — an enemy directly above or below would otherwise shove
                // the spawn up or down instead of out to where the staff is.
                from.y += MagicMuzzleLift;
                from.x += Mathf.Sign(to.x - from.x) * MagicMuzzleFwd;
            }
            if (p.is_magic && p.sky_fall) from = to + new Vector3(0f, SkyFallCells, 0f);

            var go = new GameObject("Shot");
            go.transform.SetParent(transform, false);
            var sr = go.AddComponent<SpriteRenderer>();
            sr.sharedMaterial = _mat;
            sr.sprite = p.is_magic ? Fireball(0) : Arrow();
            if (sr.sprite == null) { Destroy(go); return null; }
            // A SKILL cast is bigger than a basic attack, and a blast bigger again — so what is
            // coming reads before it lands.
            float scale = (p.is_basic ? 1f : 1.6f) * (p.aoe_radius > 0 ? FireballAoeScale : 1f);
            sr.color = BoltTint.TryGetValue(p.element ?? "", out var t) ? t : Color.white;

            // The flight is the SIM's clock: ticks between spawn and arrival, at the tick length
            // the arena already runs on. Ticks that have already passed are subtracted, so a
            // projectile that was mid-flight when the screen started does not restart.
            long total = System.Math.Max(1, (long)p.arrive_at_tick - (long)p.spawn_tick);
            long elapsed = System.Math.Max(0, tick - (long)p.spawn_tick);
            var shot = new Shot
            {
                Go = go, Sr = sr, From = from, To = to,
                Dur = total * tickDur,
                T0 = Time.time - elapsed * tickDur,
                Magic = p.is_magic, Sky = p.sky_fall, Scale = scale,
                Tint = sr.color,
            };
            Place(shot);
            return shot;
        }

        void LateUpdate()
        {
            if (_cam == null) return;
            float now = Time.time;
            foreach (var kv in _live)
            {
                var s = kv.Value;
                if (s.Go == null) continue;
                Place(s, now);
            }
        }

        void Place(Shot s, float now = -1f)
        {
            if (now < 0f) now = Time.time;
            float k = Mathf.Clamp01(s.Dur <= 0f ? 1f : (now - s.T0) / s.Dur);
            s.Go.transform.position = Vector3.Lerp(s.From, s.To, k);

            // BILLBOARD, then ROLL. Facing the camera is what every sprite here does; the roll is
            // the 2D renderer's `angle` — the direction of travel as the player sees it, which is
            // a screen-space angle and cannot be taken from the world vector.
            s.Go.transform.rotation = _cam.transform.rotation;
            var a = _cam.WorldToScreenPoint(s.From);
            var b = _cam.WorldToScreenPoint(s.To);
            float deg = s.Sky ? -90f : Mathf.Atan2(b.y - a.y, b.x - a.x) * Mathf.Rad2Deg;
            s.Go.transform.Rotate(Vector3.forward, deg, Space.Self);
            s.Go.transform.localScale = Vector3.one * s.Scale;

            if (s.Magic && _fireball != null && _fireball.Length > 0)
                s.Sr.sprite = _fireball[(int)(now * FireballFlyFps) % _fireball.Length];
        }

        // --- art ---------------------------------------------------------------------------

        Sprite Arrow()
        {
            if (_arrow != null) return _arrow;
            var tex = ArenaView.LoadArtTexture("arrow_sheet.png");
            if (tex == null) return null;
            int fw = tex.width / ArrowFrames;
            // The LAST frame, as the 2D renderer draws it — the earlier ones are the nock and draw
            // of a bow animation, not an arrow in flight.
            var r = new Rect((ArrowFrames - 1) * fw, 0f, fw, tex.height);
            _arrow = Sprite.Create(tex, r, new Vector2(0.5f, 0.5f), fw / ArrowDisplayCells);
            return _arrow;
        }

        Sprite Fireball(int i)
        {
            if (_fireball == null)
            {
                var tex = ArenaView.LoadArtTexture("fireball_sheet.png");
                if (tex == null) return null;
                _fireball = new Sprite[FireballFlyFrames];
                for (int f = 0; f < FireballFlyFrames; f++)
                {
                    var r = new Rect(f * PlayerFramePx,
                                     tex.height - (FireballRow + 1) * PlayerFramePx,
                                     PlayerFramePx, PlayerFramePx);
                    _fireball[f] = Sprite.Create(tex, r, new Vector2(0.5f, 0.5f), PlayerFramePx);
                }
            }
            return _fireball[Mathf.Clamp(i, 0, _fireball.Length - 1)];
        }

        /// How many are in the air — nothing else can see a transient.
        public int Count => _live.Count;

        [System.Serializable] class ProjView { public long tick; public Proj[] projectiles; }
        [System.Serializable]
        class Proj
        {
            public long target_id, spawn_tick, arrive_at_tick;
            public CellDTO spawn_pos, target_pos;
            public bool is_magic, sky_fall, is_basic;
            public int aoe_radius, damage;
            public string element, skill_id;
        }
        [System.Serializable] struct CellDTO { public float x, y; }
    }
}
