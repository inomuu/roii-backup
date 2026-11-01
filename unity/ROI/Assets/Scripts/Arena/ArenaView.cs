// P1 arena. Reads the trimmed view JSON (grid + entities + per-tick FX) the sim emits every tick
// and renders each entity as a camera-facing sprite billboard. Everything visual is COPIED from the
// Godot HD-2D renderer, never re-invented:
//   FRAMING  arena3d_scene.gd: camera (0,13.5,13) fov 42; 1 world unit = 1 grid cell, field centred,
//            FIELD_Z_SHIFT -2; sun pitch -34 yaw -38 energy 1.55, ambient 0.30.
//   SPRITES  render_data.gd / player_animator.gd -> arena_tables.json (sheet geom, anim, impact/lunge).
//            Vertical anchor MEASURES the opaque bottom row.
//   MOVEMENT glide: smooth lerp between cells over one tick; walk fps derived from the step duration
//            (WALK_CELLS_PER_CYCLE), clamped [8,16].
//   PLAYER   player_animator.gd swing: atk_acc drives a wind-up capped at lunge_from; when the frame
//            reaches lunge_from a strike LOCKS (armed once per swing), plays lunge_from..impact and
//            HOLDS impact until the sim's damage wrap; then follow-through impact+1..strike_end
//            (+impact_hold). Lunge surges 0->1 to the contact then eases back, melee only. Facing:
//            motion then target, with a flip cooldown, never mid-strike. Hurt flash + hit-stop.
//   MONSTER  base_monster_3d.gd: attack is TIME-DRIVEN — when time_to_hit <= windup (hit_frame/fps,
//            capped to the swing interval) the clip triggers from frame 0 and plays once, latched
//            once per swing (re-armed on the hit); lunge surge peaks on the contact frame; a damaged
//            clip plays on an HP drop; die plays once on death.
using System.Collections.Generic;
using Roi.Sim;
#if ROI_DOTWEEN
using DG.Tweening;      // Append/Kill/SetEase are EXTENSION methods — qualifying the type is not enough
#endif
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;

namespace Roi.Arena
{
    public sealed partial class ArenaView : MonoBehaviour
    {
        const float FieldZShift = -2.0f;

        // Two ways to dress the same zone, switchable live. HD2D is the Godot look ported: the
        // painted region art on the floor, props as billboards. Full3D is the standard-Unity
        // reading of the same authored layout: the pack's terrain layers painted from the SAME
        // art (stone where the art is stone, dirt where it is brown), the pack's trees and bushes
        // at the SAME prop positions, a skybox and distance haze, and a lower camera that sees the
        // horizon — sprites for the cast either way. One layout, two dressings, so the two can be
        // put beside each other rather than argued about.
        public enum ZoneStyle { HD2D, Full3D }
        public ZoneStyle Style = ZoneStyle.HD2D;
        /// Godot's EnemyAnim.BOSS_SCALE_DEFAULT — the size a boss renders at when its own scale is
        /// not reachable (a bespoke boss is keyed by a `sprite` it shares with its trash variant).
        const float BossScaleFloor = 1.6f;

        /// ONE monster's idle frame, as a UI sprite. The Bestiary shows the real creature, and it
        /// gets it from the SAME sheet cache and the same frame geometry the arena animates with —
        /// a second loader would be a second place for the frame size to be wrong.
        public Sprite MonsterPortrait(string spriteId)
        {
            if (_tab == null) _tab = Tables.Load();
            var sheet = _tab.SheetFor(spriteId, null);
            if (string.IsNullOrEmpty(sheet)) return null;
            var tex = Sheet(sheet);
            if (tex == null) return null;
            var m = _tab.GeomFor(spriteId);
            int rows = tex.height / Mathf.Max(1, m.fw);
            int frame = m.idle_frames != null && m.idle_frames.Length > 0 ? m.idle_frames[0] : 0;
            return FrameSprite(sheet, m.cols, rows, m.fw, m.fw, frame);
        }

        // UI presentation uses the exact authored idle sequence and shared frame cache.
        public Sprite[] MonsterIdlePortraits(string spriteId, out int fps)
        {
            if (_tab == null) _tab = Tables.Load();
            var m = _tab.GeomFor(spriteId);fps=Mathf.Clamp(m.idle_fps,1,12);
            var sheet=_tab.SheetFor(spriteId,null);if(string.IsNullOrEmpty(sheet))return System.Array.Empty<Sprite>();
            var tex=Sheet(sheet);if(tex==null)return System.Array.Empty<Sprite>();
            var indices=m.idle_frames!=null&&m.idle_frames.Length>0?m.idle_frames:new[]{0};
            var frames=new Sprite[indices.Length];
            for(int i=0;i<indices.Length;i++)frames[i]=FrameSprite(sheet,m.cols,tex.height/Mathf.Max(1,m.fw),m.fw,m.fw,indices[i]);
            return frames;
        }

        /// Post-processing on or off, live. The Options screen's one graphics lever that costs
        /// real frames — the grade, the vignette and the depth of field all ride this flag, so a
        /// weak machine turns off one thing rather than six.
        public void SetPostFx(bool on)
        {
            if (_cam == null) return;
            var data = _cam.GetUniversalAdditionalCameraData();
            if (data != null) data.renderPostProcessing = on;
        }

        public void SetStyle(ZoneStyle st)
        {
            Style = st;
            ApplyCameraPreset();
            _region = -1;   // forces RebuildGround (ground + dressing + blades) on the next view
        }

        // ONE camera for both dressings — the table's (user, 2026-09-06: "ต้องปรับกล้องให้มุมเดียวกับ
        // อีกอันด้วย เป็นเหตุผลด้าน gameplay"). The framing is a gameplay rule, not a look: how
        // much of the 20x10 field is on screen, where a monster entering from the right is first
        // seen, how big a sprite reads. A low horizon-seeking lens was tried for Full3D and made
        // the scenery the subject; the surround terrain now fills what THIS lens sees past the
        // art. Only the sky, fog and clear colour differ between the two.
        void ApplyCameraPreset()
        {
            if (_cam == null) return;
            var f = _tab.field;
            _cam.transform.position = new Vector3(f.cam_pos[0], f.cam_pos[1], f.cam_pos[2]);
            // The preset is the UNPANNED position; the march is measured from it, so re-applying a
            // preset mid-band cannot inherit a pan and double it.
            _camBaseX = f.cam_pos[0];
            _camBaseY = f.cam_pos[1];
            _camBaseZ = f.cam_pos[2];   // this path just placed the camera there, so the table IS the base
            if (_fill != null) _fillBaseX = _fill.transform.position.x;
            _cam.transform.rotation = Quaternion.Euler(-f.cam_pitch, 180f, 0f);
            _camBaseRot = _cam.transform.rotation;
            if (Style == ZoneStyle.HD2D)
            {
                _cam.clearFlags = CameraClearFlags.SolidColor;
                RenderSettings.skybox = null;
                RenderSettings.fog = false;
            }
            else
            {
                var sky = Resources.Load<Material>("Dressing3D/Skybox");
                _cam.clearFlags = sky != null ? CameraClearFlags.Skybox : CameraClearFlags.SolidColor;
                RenderSettings.skybox = sky;
                RenderSettings.fog = true;
                RenderSettings.fogMode = FogMode.Linear;
                RenderSettings.fogColor = new Color(0.72f, 0.80f, 0.90f);
                // Sized to the surround: the far hills sit ~70 units out and should be haze,
                // not a hard tree line; the field itself (under ~35) stays clear.
                RenderSettings.fogStartDistance = 30f;
                RenderSettings.fogEndDistance = 95f;
            }
        }
        // Godot puts actor sprites on render layer 2 so the ground fill can be culled off them.
        // Unity user layer 8 plays the same role; it needs no name to work as a mask.
        const int ActorLayer = 8;
        // Ground and blades light the pool THEMSELVES, with Godot's linear falloff (see
        // GodotFill in their shaders); the engine's inverse-square point light is culled off
        // them so the pool is not applied twice. Props and actors keep the engine light.
        const int GroundLayer = 9;
        // Copied constants (player_animator.gd)
        const float WalkCellsPerCycle = 2.0f, WalkFpsMin = 8f, WalkFpsMax = 16f;
        const float FacingFlipCooldown = 0.35f, AnimSwitchDelay = 0.14f;
        const float PlayerHurtFlashDur = 0.16f, HitstopDur = 0.10f, DeathAlpha = 0.85f;

        // BAKED ZONE (user 2026-09-06: "ทำ scene + object เลยดีกว่า อะไรไม่จำเป็น ไม่ให้สร้างตอน
        // runtime"). ZoneBaker runs the same builders ONCE in the Editor and saves the result as
        // scene objects + assets; at runtime this view then ADOPTS what is in the scene instead of
        // building it, and only the actors and the per-frame animation are code. Both flags are
        // set by the baker, never by hand.
        public bool EnvBaked;
        public int BakedRegion = -1;

        [Tooltip("Use the authored Area 1–5 map for later areas until their maps are ready.")]
        public bool UseEarlyAreaMapPlaceholder = true;

        public ulong Seed = 42;
        public string ZoneId = "zone_b1_1";
        public string ClassId = "swordsman_t1";
        public string PartyName = "Idlers";

        Tables _tab;
        readonly Dictionary<string, Texture2D> _sheets = new();
        readonly Dictionary<string, Sprite> _sprites = new();
        readonly Dictionary<string, float> _botPad = new();
        readonly Dictionary<string, float> _topPad = new();

        SimDriver _driver;
        Camera _cam;
        Transform _fieldRoot;
        readonly Dictionary<long, Actor> _actors = new();
        readonly HashSet<long> _seen = new();
        readonly HashSet<long> _knockback = new();   // ids shoved this tick — their glide arcs
        /// base_monster_3d / arena3d_scene: a shove is a FIXED-length glide with a parabolic hop,
        /// not the speed-based flat step. KNOCKBACK_HOP is 14 px at the universal scale.
        const float KnockbackDur = 0.28f, KnockbackHopPx = 14f;
        readonly Dictionary<long, float> _entX = new();
        float _tickTime, _tickDur = 0.1f;
        long _tick;                       // sim tick of the last view (dump keys off it)
        /// The sim tick on screen — ArenaShot grabs at a TICK, so the pair of screenshots the
        /// environment work is judged on shows the same moment in both engines.
        public long CurrentTick => _tick;
        /// The region on screen, so an Editor tool can read the same dressing rows this does.
        /// (Not `Region` — that name is taken by the nested table type.)
        public int CurrentRegion => _region;
        public Tables TableData => _tab;
        int _gw = 20, _gh = 10, _region = -1;

        /// HOW FAR ONE ROOM IS FROM THE NEXT, in world units, and the one number in this port that
        /// could NOT be copied from Godot.
        ///
        /// Godot advances by one FIELD WIDTH, because its band art is a strip exactly three fields
        /// wide. Region 1 here is authored terrain instead, and its three rooms were placed 52 m
        /// apart — so the fight grid (still 20 cells) sits in the middle of a room that is wider
        /// than it, and marching 20 would stop the party in the gap between two authored rooms.
        ///
        /// The SIGN does not carry either: `CellToWorld` negates x (the camera's yaw-180 goes with
        /// it), so forward here is -x, which is why the authored rooms are at 0, -52 and -104.
        /// Copying Godot's `+=` would have walked the party away from the art rather than into it.
        ///
        /// `R1ThreeRoomLayout` places the rooms from THIS constant. The direction matters: the game
        /// has to walk the distance, so the game owns it and the authoring tool follows. Editor code
        /// can read a runtime constant; runtime code cannot read an editor one, and a second copy of
        /// 52 in the tool would drift the day either side moved.
        public const float RoomSpacing = 52f;

        /// Rooms in a band. The sim's own rule — a band is three rooms and the third one ends it —
        /// and the number of combat boxes the Scene-view gizmo draws.
        public const int RoomsPerBand = 3;

        /// The world x every cell is offset by — the accumulated march across a band's three rooms.
        float _scrollX;
        /// The band the current scroll run belongs to; -1 until the first state tick seeds it.
        int _scrollBand = -1;
        /// Room number of the last zone seen, so "forward one room" can be told from a jump back.
        int _scrollRoom;
        /// Camera pan: where it is, where it is going, and the ease between them.
        float _panFrom, _panTo, _panStart, _panDur;
        float _camBaseX, _fillBaseX;
        // The UNPANNED camera height and depth. Read once with `_camBaseX`, so the per-frame write
        // below is absolute: taking y and z back off the live transform folds the focus beat's own
        // offset into the next frame's base, and the camera walks itself out of the world.
        float _camBaseY, _camBaseZ;
        /// The authored orientation. The beat is the only thing that ever turns the camera, and it
        /// writes an ABSOLUTE rotation off this — the same rule the position had to learn.
        Quaternion _camBaseRot = Quaternion.identity;

        // THE BOSS BREAK BEAT. A trash mob breaking is a flash and a handful of glass; a boss
        // breaking is the thing the fight was building to, so the camera goes and looks at it and
        // the fight waits. Only bosses get it — on a slime it would fire every few seconds and stop
        // meaning anything.
        //
        // The SIM is what pauses, not the frame. Freezing the whole renderer would also freeze the
        // shards in mid-air, and glass hanging still reads as a hitch rather than as a held moment.
        // So combat stops, the debris keeps falling, and the camera pushes in over the top.
// --- THE BOSS BREAK BEAT, TUNED IN THE INSPECTOR ------------------------------------------
        //
        // Unity has no tween. `AnimationCurve` is the nearest thing it ships and is the better tool
        // here anyway: the curve is DRAGGABLE WHILE THE GAME RUNS, so the shape of the move can be
        // found by pulling a handle and pressing B, instead of by recompiling a guess. Everything
        // else the beat touches is beside it for the same reason — what limited this was the length
        // of the tuning loop, not which easing function was picked.
        //
        // The curve is the envelope: 0 at rest, 1 at full push, and it MAY GO ABOVE 1 — that is the
        // overshoot, and it is why this is a curve rather than a 0-1 lerp.
        [Header("Boss break beat")]
        [Tooltip("Punch in. Ease.OutBack, with the overshoot DOTween itself defines — nothing hand-set.")]
        public float FocusPunchDur = 0.34f;
        [Tooltip("Held DEAD STILL at the mark. Not a tween — a wait.")]
        public float FocusHoldDur = 0.55f;
        [Tooltip("Eased back out. Longer than the punch: arriving is an impact, leaving is a decision.")]
        public float FocusReleaseDur = 0.75f;
#if ROI_DOTWEEN
        /// THE PUNCH'S EASE, CYCLED IN GAME with V. Comparing easings by recompiling one at a time
        /// is the slow loop that made every earlier round of this a guess; the answer to "which of
        /// these feels right" is to feel them back to back. Ordered from no rebound at all to the
        /// most, so stepping through the list is stepping through how much bounce there is.
        static readonly Ease[] PunchEases =
        {
            Ease.OutCubic, Ease.OutQuart, Ease.OutQuint, Ease.OutCirc, Ease.OutExpo,
            Ease.OutBack, Ease.OutElastic, Ease.OutBounce,
        };
        int _punchEase = 3;                  // OutCirc — chosen by eye against the others (user
                                             // 2026-09-09), and it is the one that holds speed
                                             // longest before braking, so the stop reads as the
                                             // camera arriving rather than as it easing off early.
        Ease PunchEase => PunchEases[_punchEase % PunchEases.Length];

        /// Step to the next ease and say which it is. `ArenaView` owns it because it owns the beat;
        /// the key that calls it lives in the editor-only trigger next door.
        public string CycleFocusEase()
        {
            _punchEase = (_punchEase + 1) % PunchEases.Length;
            return PunchEase.ToString();
        }
#endif
        [Tooltip("Only used when DOTween is NOT imported: the same envelope as one draggable curve.")]
        public AnimationCurve FocusCurve = new AnimationCurve(
            new Keyframe(0f, 0f, 0f, 6.5f),
            new Keyframe(0.17f, 1.09f, 0f, 0f),
            new Keyframe(0.30f, 1f, 0f, 0f),
            new Keyframe(0.55f, 1f, 0f, 0f),
            new Keyframe(0.78f, -0.04f, -2.6f, -2.6f),
            new Keyframe(1f, 0f, 0f, 0f));
        [Tooltip("Seconds the sim is held. Shorter than the beat: the fight resumes while the camera is still easing back.")]
        public float FocusPauseDur = 0.95f;
        [Tooltip("Field of view multiplier at full push.")]
        public float FocusZoom = 0.62f;
        [Tooltip("Degrees the camera arcs DOWN around the boss. The zone is painted for 42 degrees up; too far below shows the ground is a sheet.")]
        public float FocusPitch = 16f;
        [Tooltip("What the sun and ground fill fall to at full push.")]
        public float FocusDim = 0.18f;
        [Tooltip("Energy of the light that picks the boss out of the dark.")]
        public float FocusKeyEnergy = 45f;
        public float FocusKeyRange = 9f;
        [Tooltip("Playback speed of the glass while the beat runs.")]
        public float FocusShardSpeed = 1f;
        // -------------------------------------------------------------------------------------

        float _focusPitchDir = 1f;   // which way around the target is DOWN; solved, not assumed
        Vector3 _focusTarget;        // what the beat is looking at
        float _focusE;               // the envelope: 0 at rest, 1 at full push, higher on overshoot
        float FocusDur => FocusPunchDur + FocusHoldDur + FocusReleaseDur;
#if ROI_DOTWEEN
        Sequence _focusSeq;
#endif

        /// Drive the envelope for one beat.
        ///
        /// THE SHAPE IS THREE MOVES, NOT ONE. Everything riding a single curve arrived, stopped
        /// dead and left again on the same schedule, which is what read as weightless: a shot that
        /// parks is a still frame with extra steps. Punch in past the mark, keep creeping while it
        /// holds, then ease out over twice the time it took to arrive.
        void DriveFocus()
        {
#if ROI_DOTWEEN
            _focusSeq?.Kill();
            _focusE = 0f;
            _focusSeq = DOTween.Sequence()
                .Append(DOTween.To(() => _focusE, v => _focusE = v, 1f, FocusPunchDur)
                               .SetEase(PunchEase))     // DOTween's own overshoot, not a number of mine
                // A WAIT, NOT A TWEEN. The hold used to creep on to 1.02 on the theory that a shot
                // which parks dead still reads as cheap. It does not — it reads as arriving, and
                // the creep read as the camera never deciding where it was going: with the punch's
                // overshoot in front of it the beat changed direction three times (user
                // 2026-09-09: "มันขยับไปมาหลังถึงเป้าหมาย ... ปกติ tween จะคมกว่านี้ที่ใช้ใน godot").
                // Arrive, stop, leave.
                .AppendInterval(FocusHoldDur)
                .Append(DOTween.To(() => _focusE, v => _focusE = v, 0f, FocusReleaseDur)
                               .SetEase(Ease.InOutQuad))
                .SetUpdate(true);          // UNSCALED: the beat is presentation and outlives any pause
#else
            _focusE = 0f;
#endif
        }

        float _focusAge = -1f;               // < 0 = no beat running
        float _focusFovBase;
        /// The camera translation that puts the boss in the MIDDLE OF THE FRAME, solved once when
        /// the beat starts. Leaning a fixed fraction of the way toward it left the target off
        /// centre and the zoom then magnified the error; and solving it per frame would jitter,
        /// which is what read as the camera not moving smoothly.
        Vector3 _focusShift;
        Light _focusKey;
        float _sunBase, _fillBase;
        Color _breakAmbientBase;
        Vector4 _breakFillColourBase;
        readonly List<Light> _breakEnvironmentLights = new List<Light>();
        readonly List<float> _breakEnvironmentEnergy = new List<float>();
        /// The scene's own ground and scenery, parked while the party stands in a region it was
        /// not dressed for. Parked rather than merely hidden because the builders destroy BY NAME
        /// and a baked scene uses their names — see `BakedArt`.
        readonly BakedArt _baked = new();

        /// A march is owed to the party: the field moved this frame and they have to walk it.
        bool _marchPending;
        bool _walkInPending;
        long _openedAtSeen = -1;
        float _zoneOpenedAt = -1f;   // for the dead-air measurement below
        bool _firstHitLogged;

        /// HOW FAST THE PARTY WALKS BETWEEN ROOMS, in cells per second. Godot's `WALK_SPEED_CPS`,
        /// and the reason the march is a DURATION rather than a constant: distance / speed, so a
        /// wider room takes longer to cross instead of the party moving faster to hide it.
        ///
        /// 5. Godot walks at 3, but it only crosses one field width; region 1's rooms are 52 m
        /// apart, and at 3 that is 17 s of nothing happening in an idle game. 10 was the first
        /// try and read as too fast on sight (user 2026-09-09: "pan ไวไป เดินไวไป") — the camera
        /// shares this duration by design, so one number sets both. At 5 the long room is 10.4 s
        /// and the short one about 7, which `DEPLOY_GRACE_TICKS` is sized to hold.
        /// THE HIT FLASH, set here and not read from `arena_tables.json` while it is being tuned.
        /// 0.3 s at 4.5. The table's 2.2 never crossed this scene's Bloom threshold hard enough
        /// to read; the DURATION was never the problem (0.5 s was tried and looked far too long,
        /// user 2026-09-09) — the brightness was. Held here so both can be changed and looked at
        /// without an export round trip through the Godot side that owns the table.
        ///
        /// When they settle, they belong back in `base_monster_3d.gd` (`FLASH_DUR`, `FLASH_TINT`)
        /// and re-exported, so both clients agree — but that also changes how Godot looks, which
        /// is a decision rather than a copy.
        public const float FlashDur = 0.3f;
        /// Past the scene's Bloom threshold (1.4) by a wide margin, so the flash blooms and nothing
        /// else does. 7, not 4: widening the bloom's `scatter` spreads the SAME energy over more
        /// pixels, so the core dims as the halo grows — the only way to have both is more energy at
        /// the source. This is that.
        public const float FlashTint = 6f;
        /// How sharply the flash falls out of SATURATION, which is the only part of it anyone can
        /// see. Measured per frame: `lerp(lit, 4.5, flash)` stays at or above 1.0 — pure white to
        /// the eye — until flash drops under about 0.2. At 0.15 s and an exponent of 1.8 that left
        /// five of seven frames identically white and the whole visible fade in the last two, which
        /// is why every curve tried before this read as "it just vanishes" and none of them helped.
        ///
        /// 4 gets out of saturation inside the first third instead of past the halfway mark, and
        /// `FlashDur` is doubled so what remains is eight or nine frames of visible fade rather than
        /// two. The pop is as bright as before; only the part you can watch got longer.
        public const float FlashDecay = 4f;

        public const float MarchSpeedCps = 5f;
        /// Mirrors `tick::MARCH_SETTLE_TICKS` (3 ticks): the beat between arriving and the fight.
        public const float MarchSettleSeconds = 0.3f;

        /// `DEPLOY_GRACE_TICKS` in seconds — the sim skips combat for 60 ticks at 10 Hz when a
        /// zone re-opens (110 ticks at 10 Hz), and that is the whole window the party has to arrive
        /// in. MIRRORS a sim-core constant: if the two drift the cap either bites early (a slide) or leaves the
        /// party still walking when the first monster swings. Both were moved together, and the
        /// warning below fires the moment they stop agreeing.
        /// The fallback when the sim has not said yet. The live number is `_graceSeconds`, read
        /// from `zone_spawn.deploy_grace_ticks` every view.
        public const float GraceSeconds = 11f;
        float _graceSeconds = GraceSeconds;
        /// How far LEFT of its cell a party member starts its ENTRANCE — `arena3d_scene.gd`'s
        /// WALK_IN_CELLS. Short on purpose: the leg has to fit inside the deploy grace, and Godot
        /// records that a longer one meant the fight started mid-stride.
        public const float WalkInCells = 3f;
        string _status = "starting";
        string _zoneSeen = "";

        sealed class Actor
        {
            public GameObject Root;
            public Transform Billboard;
            public SpriteRenderer Body;
            public SeraphineBasicSlash SwordEdge;
            public ActorContactShadow ContactShadow;
            public float HeadY;          // sprite cell height retained for existing VFX framing
            public float BarHeadY;       // visible alpha-trimmed top for the projected health bar
            public float BarForH;        // sprite scale BarHeadY was measured at (re-measure on change)
            public float LungeX;         // this frame's lunge offset — the bar does not follow it
            public float CastFrac = -1f; // -1 = not casting
            public string MonName;
            // snapshot
            public bool IsPlayer, Alive, IsBoss;
            public string ClassId, Sprite, Element;
            public int CellX, CellY, PrevCellX, TargetX, AtkRange, ProjTravel;
            public long AtkAcc, EffAspd, AtkSpeed, SpeedMilli, TargetId, Hp, MaxHp;
            // glide
            public Vector3 GlideFrom, GlideTo; public float GlideT0, GlideDur; public bool Gliding, Bounce;
            // facing
            public int Facing = 1; public float FacingFlipAt;
            // player swing
            public long PrevAtkAcc; public bool StrikeArmed = true, StrikeLocked;
            public float AttackUntil;         // player_animator.gd st["attack_until"]
            public float StrikeT0, FollowUntil, FollowT0, LungeDir;
            public string PendingSkill;
            public string ResolvedSkill;
            public int MotionLock, PoseRow, PoseFrame;
            public bool BasicSwordSwing;
            /// While this is in the future the actor faces the way it is MARCHING and nothing may
            /// argue. Its own field rather than borrowing `FollowUntil`, which is a swing's
            /// follow-through and drives a pose as well as a facing — a ten-second march would have
            /// held the strike pose for the whole crossing.
            public float MarchUntil;
            // idle commit
            public string AnimName = "idle", PendingName = "idle"; public float PendingSince;
            // monster swing
            public bool MonLatch, MonPlaying, MonDmgPlaying, MonAtkOver; public float MonAtkT0, MonDmgT0;
            // hit juice (base_monster_3d): a one-shot flash + squash, ONLY for hit_juice defs
            public float FlashT, SquashT; public MaterialPropertyBlock Mpb;
            // shared
            public float DeathT, HurtUntil, HitstopUntil, AnimT; public int HitstopFrame = -1;
            public int Toughness, ToughnessMax;
            /// A killing multi-hit spells out every number before the body plays its death clip
            /// (number_fx: "a target dying NEVER truncates a group"). Until then the sprite is
            /// animated as if it were still alive; the BAR still drops at 0, as Godot's does.
            public float DeathHoldUntil;
            public bool Init;
            // grass push: energy earned by walking through a clump, bleeding away otherwise
            public float PushEnergy; public Vector3 PushLast; public bool PushInit;
            // cross-engine dump only: the clip + frame actually shown this LateUpdate
            public string DbgAnim = ""; public int DbgFrame = -1; public float DbgPx;
        }

        /// Set by `SessionBoot` in its Awake — before any Start, which is what makes this
        /// reliable. It means "a login owns the session", so the stage is built and then nothing
        /// runs until `BeginSession` is called. Without a boot in the scene this stays false and
        /// the arena starts its own session exactly as it always did, which is what keeps the
        /// editor probes and the side-by-side dump working.
        public bool AwaitBoot;

        void OnDisable()
        {
            EndAegisPresentation(false);
            ClearSeraphineSkills();
            if (_focusAge >= 0f) EndFocus();
            Shader.SetGlobalVector("_BreakKeyColor", Vector4.zero);
        }

        void Start()
        {
            Shader.SetGlobalVector("_BreakKeyColor", Vector4.zero);
            var gd = SimSession.LoadCatalog();
            if (gd == null) { _status = "no game_data.json"; return; }
            _tab = Tables.Load();
            ArmDump();
            _tickDur = _tab.tick_interval;
            if (EnvBaked) AdoptBakedEnvironment(); else BuildStage();
            _status = $"sim {SimSession.SimVersion()}";
            if (AwaitBoot) { _status += " — waiting for login"; return; }
            var session = SimSession.NewAs(Seed, gd, ZoneId, ClassId, PartyName);
            if (session == null) { _status = "roi_new_as failed"; return; }
            BeginSession(session);
        }

        /// Run this session. The ONE place the arena adopts a sim — a login hands one over here,
        /// and so does the arena's own fallback start above.
        public SimDriver BeginSession(SimSession session)
        {
            EndAegisPresentation(false);
            ClearSeraphineSkills();
            if (_driver == null)
            {
                _driver = gameObject.AddComponent<SimDriver>();
                _driver.StateUpdated += OnView;
            }
            _driver.Attach(session);
            _status = $"sim {SimSession.SimVersion()}";
            // The banner needs the driver to send `RestartInstance`; the bar is built below, so
            // bind after BuildBar rather than here — see the call at the end of this method.
            // The chrome arrives WITH the session. Building the bar before there is a game gives
            // the player thirty screens that all open onto nothing, over a login screen.
            BuildBar();
            if (_death != null) _death.Bind(_driver);
            return _driver;
        }

        // --- aiming a Commander skill -------------------------------------------------------
        //
        // The BAR says which skill; the ARENA decides what that means, because the arena is what
        // knows where the cursor is and what a cell is. That split is the reason the bar is
        // renderer-agnostic: it never learns that a 3D board exists.

        Roi.UI.CommanderBar _cmdr;
        string _aiming = "";
        int _aimSide;
        Transform _aimPreview;

        void OnSkillRequested(string skillId)
        {
            if (_modeTabs != null && !_modeTabs.IsCombat) return;
            var def = SkillDef(skillId);
            if (def == null) return;
            switch (def.target)
            {
                // Pressing the same skill again cancels, so the button is its own off switch.
                case "place_cell": SetAim(_aiming == skillId ? "" : skillId, def.side); break;
                case "instant": SetAim("", 0); Cast(skillId, 0, 0); break;
                // `pick_ally` / `pick_enemy` are declared in `CommanderTarget` because the design
                // needs them, with no skill behind either yet. Deliberately NOT pre-built: a picker
                // written against no caller is code nothing runs, and its shape depends on whether
                // the first such skill wants the click snapped. Add the branch WITH the skill.
                default: SetAim("", 0); break;
            }
        }

        void SetAim(string skillId, int side)
        {
            _aiming = skillId ?? "";
            _aimSide = side;
            if (_cmdr != null) _cmdr.SetAiming(_aiming);
            if (_aimPreview != null) _aimPreview.gameObject.SetActive(_aiming != "");
        }

        /// One skill's row out of the SIM's catalog. The renderer keeps no copy of its own:
        /// cooldown, target kind and footprint all move the moment the sim says they do.
        Roi.UI.CommanderBar.Skill SkillDef(string id)
        {
            if (string.IsNullOrEmpty(id) || _skills == null) return null;
            foreach (var s in _skills) if (s.id == id) return s;
            return null;
        }

        Roi.UI.CommanderBar.Skill[] _skills = new Roi.UI.CommanderBar.Skill[0];

        void Cast(string skillId, int x, int y)
        {
            // One skill, one event. A second Commander skill adds a case here and a row in the
            // sim's catalog — not a generic "cast" event, because each one takes its own payload.
            if (skillId == "commander_smite")
                _driver?.ApplyInput($"{{\"CommanderSmite\":{{\"x\":{x},\"y\":{y}}}}}");
        }

        /// The cell under the pointer, or false when the ray misses the board.
        ///
        /// The inverse of `CellToWorld`, written out rather than solved at the call site: the two
        /// have to agree exactly or the footprint preview lands on one cell and the blast on
        /// another, which is the one failure here no compiler can catch.
        bool CellUnderPointer(out int cx, out int cy)
        {
            cx = cy = 0;
            if (_modeTabs != null && !_modeTabs.IsCombat) return false;
            if (_cam == null) return false;
            var mouse = UnityEngine.InputSystem.Mouse.current;
            if (mouse == null) return false;
            var ray = _cam.ScreenPointToRay(mouse.position.ReadValue());
            if (!new Plane(Vector3.up, Vector3.zero).Raycast(ray, out float enter)) return false;
            var p = ray.GetPoint(enter);
            cx = Mathf.RoundToInt(-p.x + _gw * 0.5f - 0.5f);
            cy = Mathf.RoundToInt(p.z + _gh * 0.5f - 0.5f - FieldZShift);
            return cx >= 0 && cy >= 0 && cx < _gw && cy < _gh;
        }

        void TickAim()
        {
            if (_aiming == "") return;
            bool on = CellUnderPointer(out int cx, out int cy);
            if (_aimPreview == null) _aimPreview = BuildAimPreview();
            _aimPreview.gameObject.SetActive(on);
            if (on)
            {
                // Anchored the way `grid::Blast::sided` anchors it — the cell pressed is the
                // top-left of the footprint, not its centre. The SIDE comes from the sim's row.
                float half = (_aimSide - 1) * 0.5f;
                _aimPreview.position = CellToWorld(cx + half, cy + half) + new Vector3(0f, 0.02f, 0f);
                _aimPreview.localScale = new Vector3(_aimSide, 1f, _aimSide);
            }
            var mouse = UnityEngine.InputSystem.Mouse.current;
            if (mouse == null || !mouse.leftButton.wasPressedThisFrame) return;
            // A click OFF the board cancels rather than casting: the sim would refuse an
            // out-of-bounds anchor anyway, and a press that silently does nothing reads as a bug.
            if (on) Cast(_aiming, cx, cy);
            SetAim("", 0);
        }

        /// EDITOR PROBE DOORS. The Editor has no pointer to move, so the two halves the aim is made
        /// of are exposed separately: build the event from a named cell, and check the cell↔world
        /// arithmetic on its own. Neither is reachable from a build's input path.
        public void CastForTest(string skillId, int x, int y) => Cast(skillId, x, y);

        public bool CellRoundTripsForTest(int cx, int cy)
        {
            var w = CellToWorld(cx, cy);
            int bx = Mathf.RoundToInt(-w.x + _gw * 0.5f - 0.5f);
            int by = Mathf.RoundToInt(w.z + _gh * 0.5f - 0.5f - FieldZShift);
            return bx == cx && by == cy;
        }

        Transform BuildAimPreview()
        {
            var go = GameObject.CreatePrimitive(PrimitiveType.Quad);
            go.name = "AimPreview";
            Destroy(go.GetComponent<Collider>());
            go.transform.SetParent(_fieldRoot, false);
            go.transform.rotation = Quaternion.Euler(90f, 0f, 0f);
            var mr = go.GetComponent<MeshRenderer>();
            mr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            mr.receiveShadows = false;
            var m = new Material(Shader.Find("Universal Render Pipeline/Unlit"));
            m.SetFloat("_Surface", 1f);                       // transparent
            m.SetFloat("_Blend", 0f);                          // alpha
            m.SetInt("_SrcBlend", (int)UnityEngine.Rendering.BlendMode.SrcAlpha);
            m.SetInt("_DstBlend", (int)UnityEngine.Rendering.BlendMode.OneMinusSrcAlpha);
            m.SetInt("_ZWrite", 0);
            m.renderQueue = 3000;
            m.color = new Color(0.95f, 0.80f, 0.35f, 0.30f);
            mr.sharedMaterial = m;
            return go.transform;
        }

        // --- lending the stage --------------------------------------------------------------

        /// Stop rendering the LIVE sim, or start again.
        ///
        /// A story stage is a second sim, and the arena can only draw one. Suspending is the whole
        /// mechanism: the live driver keeps ticking (it must — the idle game is the game), it just
        /// stops being what the screen is looking at. Nothing about the live session changes, so
        /// there is no state to restore beyond pointing the eye back.
        public void SuspendLiveView(bool suspend)
        {
            _aegisLiveSuspended = suspend;
            if (suspend) EndAegisPresentation(false);
            if (suspend) ClearSeraphineSkills();
            if (_driver == null) return;
            _driver.Paused = false;      // a teardown mid-beat must not leave the sim held
            _driver.StateUpdated -= OnView;
            if (!suspend) _driver.StateUpdated += OnView;
        }

        /// Draw one frame from somewhere else. The payload is the same view JSON — a second sim
        /// produces exactly the same shape, which is what makes this a loan rather than a port.
        public void FeedView(string json) => OnView(json);

        /// Drop every actor. Called when the eye moves between sims: a fresh GameState restarts
        /// the monster id counter, so the next sim's monsters carry ids this one has actors for,
        /// and they would be glided in from wherever their namesakes died.
        public void ResetActors()
        {
            EndAegisPresentation(false);
            ClearSeraphineSkills();
            foreach (var kv in _actors) Kill(kv.Value.Root);
            _actors.Clear();
        }

        /// EDITOR ENTRY (ZoneBaker): build the whole Full3D environment for `zoneId` under this
        /// object with no sim running, so the scene can be saved with it. Same builders as the
        /// runtime path — the derivations (art-driven tint, classifier, layout) are the point.
        public void BakeEnvironment(string zoneId)
        {
            ResetStatics();
            _tab = Tables.Load();
            _gw = 20; _gh = 10;
            Style = ZoneStyle.Full3D;
            _region = RegionOfZone(zoneId);
            var old = transform.Find("Field");
            if (old != null) Kill(old.gameObject);
            foreach (var n in new[] { "ArenaCamera", "Sun", "GroundFill", "PostFX" }) { var t = transform.Find(n); if (t != null) Kill(t.gameObject); }
            BuildStage();
            RebuildGround();
            EnvBaked = true;
            BakedRegion = _region;
            Debug.Log($"ZONE BAKED region {_region} under {name}");
        }

        /// The caches that outlive a bake in the Editor (statics survive until a domain reload):
        /// a stale one hands the next bake an asset the previous bake owned.
        public static void ResetStatics()
        {
            _cloudTex = null; _actorMat = null; _grassMat = null;
            _grassMats.Clear(); _layerMean.Clear(); _vegTint.Clear();
        }

        /// RUNTIME with a baked scene: find what ZoneBaker saved instead of building it.
        void AdoptBakedEnvironment()
        {
            _fieldRoot = transform.Find("Field");
            HideUnavailableDecoration();
            // THE BAKED ART IS ONE REGION'S. Captured here, before any actor or built ground joins
            // the Field, so the set is exactly what the scene authored — and with each object's own
            // active state, because several of these ship switched OFF and turning them all on when
            // the party comes home would be a different scene than the one that was dressed.
            _baked.Capture(_fieldRoot);
            _cam = GetComponentInChildren<Camera>(true);
            // THE AUTHORED CAMERA IS THE BASE HERE, not `field.cam_pos`. This path takes the camera
            // the zone scene was dressed around and never repositions it, so the table's numbers
            // describe a camera that is not the one on screen. Only x was ever read back before,
            // and the authored x happens to be 0 like the table's — which is why forcing y and z
            // from the table put the camera at the origin and blacked the frame from the first one.
            if (_cam != null)
            {
                var cp0 = _cam.transform.position;
                _camBaseX = cp0.x; _camBaseY = cp0.y; _camBaseZ = cp0.z;
                _camBaseRot = _cam.transform.rotation;
            }
            var lights = GetComponentsInChildren<Light>(true);
            foreach (var l in lights) { if (l.name == "Sun") _sun = l; else if (l.name == "GroundFill") _fill = l; }
            var vol = GetComponentInChildren<Volume>(true);
            if (vol != null && vol.sharedProfile != null) { vol.sharedProfile.TryGet(out _vignette); vol.sharedProfile.TryGet(out _dof); }
            _atmos = _fieldRoot != null ? _fieldRoot.Find("Atmosphere") : null;
            _atmosCam = _cam != null ? _cam.transform.Find("AtmosphereCam") : null;
            var ws = _fieldRoot != null ? _fieldRoot.Find("WetSheen") : null;
            _wetSheen = ws != null ? ws.GetComponent<MeshRenderer>().sharedMaterial : null;
            if (_sun != null)
            {
                _sunData = _sun.GetComponent<UniversalAdditionalLightData>();
                _cloudTex = _sun.cookie as Texture2D;
                _cloudPeriod = 6f / Mathf.Max(_tab.look.cloud_scale, 0.0001f);
            }
            var gf = _fieldRoot != null ? _fieldRoot.Find("GrassField") : null;
            if (gf != null)
                foreach (var mr in gf.GetComponentsInChildren<MeshRenderer>(true))
                    foreach (var m in mr.sharedMaterials)
                        if (m != null && m.HasProperty("_Top_Color") && !_tuftMats.Contains(m)) _tuftMats.Add(m);
            Style = ZoneStyle.Full3D;
            _region = BakedRegion;
            ApplyDaylight(true);
            ApplyActorExposure();
            BuildHud();
            Debug.Log($"ZONE ADOPTED region {_region}: cam={_cam != null} sun={_sun != null} fill={_fill != null} post={_vignette != null} atmos={_atmos != null} rays={_atmosCam != null} sheen={_wetSheen != null} tufts={_tuftMats.Count}");
        }

        // Optional imported foliage/particles can be absent from a checkout/build. Missing
        // particle materials become opaque pink cards, including rays attached to the camera.
        // Keep them from covering combat. The authored objects/assets are retained;
        // restoring their dependencies makes them render again on the next scene load.
        void HideUnavailableDecoration()
        {
            int hidden = 0;
            foreach (var renderer in GetComponentsInChildren<Renderer>(true))
            {
                if (renderer.forceRenderingOff) continue;
                bool scenery = false;
                for (var parent = renderer.transform; parent != null && parent != transform; parent = parent.parent)
                    if (parent.name == "PainterlyScenery" || parent.name == "Atmosphere"
                        || parent.name == "AtmosphereCam" || parent.name == "MeadowAtmosphere")
                    { scenery = true; break; }
                if (!scenery) continue;
                foreach (var material in renderer.sharedMaterials)
                {
                    if (material != null && material.shader != null && material.shader.isSupported
                        && material.shader.name != "Hidden/InternalErrorShader") continue;
                    renderer.forceRenderingOff = true;
                    hidden++;
                    break;
                }
            }
            if (hidden > 0) Debug.LogWarning($"SCENERY: {hidden} optional renderers have missing/unsupported materials and are hidden. Restore the original decoration dependencies to display them.");
        }

        void BuildStage()
        {
            _fieldRoot = new GameObject("Field").transform;
            _fieldRoot.SetParent(transform, false);

            var f = _tab.field;
            var camGo = new GameObject("ArenaCamera");
            camGo.transform.SetParent(transform, false);   // part of this object, so a bake saves it
            _cam = camGo.AddComponent<Camera>();
            _cam.transform.position = new Vector3(f.cam_pos[0], f.cam_pos[1], f.cam_pos[2]);
            // The same unpanned base the preset path records. This path built the camera and left
            // the base at zero, which the pan alone survived (it was re-derived every frame from a
            // zero that happened to match) and the focus beat would not.
            _camBaseX = f.cam_pos[0];
            _camBaseY = f.cam_pos[1];
            _camBaseZ = f.cam_pos[2];
            // Yaw 180 goes with the mirrored world x in CellToWorld; pitch is Godot's, negated
            // because Godot measures it as a rotation and Unity as a look-down angle.
            _cam.transform.rotation = Quaternion.Euler(-f.cam_pitch, 180f, 0f);
            _camBaseRot = _cam.transform.rotation;
            _cam.fieldOfView = f.cam_fov;
            _cam.backgroundColor = new Color(0.05f, 0.06f, 0.09f);   // Environment.background_color
            _cam.clearFlags = CameraClearFlags.SolidColor;
            var camData = camGo.GetComponent<UniversalAdditionalCameraData>();
            if (camData == null) camData = camGo.AddComponent<UniversalAdditionalCameraData>();
            camData.renderPostProcessing = true;

            BuildLights();
            BuildPost();
            BuildHud();
        }

        System.Collections.IEnumerator ArmCompanion()
        {
            yield return new WaitForSecondsRealtime(3f);
            _companion.Toggle();
            Debug.Log("COMPANION on — " + _companion.Report);
            // And OFF again, because the half that matters is the one nobody watches: a mode that
            // leaves the window borderless and floating is a mode nobody turns on twice.
            yield return new WaitForSecondsRealtime(6f);
            _companion.Toggle();
            yield return new WaitForSecondsRealtime(1f);
            Debug.Log("COMPANION off — " + _companion.Report);
        }

        // The 2D overlay. Built from BOTH entry paths for the same reason ApplyActorExposure is:
        // a baked zone never runs BuildStage.
        void BuildHud()
        {
            if (_hud != null) { _hud.SetCamera(_cam); return; }
            _hud = ArenaHud.Create(transform, _cam);
            // Numbers RIDE their victim while it lives and freeze where it died once it is gone.
            _hud.AnchorOf = (id, frozen) =>
                _actors.TryGetValue(id, out var a) && a.Root != null
                    ? a.Root.transform.position + Vector3.up * (a.HeadY * 0.7f)
                    : frozen;
            _hud.TryActorGround = TryActorGround;
            // The action bar is not part of the arena — in Godot it is a sibling of the arena
            // under main.tscn. There is no orchestrator scene on this side yet, so it is created
            // here, once, and moves out the day one exists.
            BuildBar();
        }

        Roi.UI.ArenaOverlay _overlay;
        Roi.UI.ClearSummary _clear;
        Roi.UI.DpsMeter _dps;
        Roi.UI.CompanionWidget _companion;
        Roi.Audio.ArenaSound _sound;
        ZoneFade _fade;
        Roi.UI.DeathBanner _death;
        CanvasGroup _combatPopupGroup;
        ArenaSparks _sparks;
        ArenaShards _shards;
        ArenaBreakFlash _breakFlash;
        ArenaProjectiles _shots;

        /// Built on the FIRST session, never before one. Called from both BuildHud (the arena's own
        /// start) and BeginSession (a login's), so whichever comes second finds it already there.
        void BuildBar()
        {
            if (_bar != null || (AwaitBoot && _driver == null)) return;
            _bar = Roi.UI.HudBar.Create(transform);
            // The room's own readout arrives with the chrome, and from the same view the arena
            // renders: how far this room is from its boss, the boss's health, the clear card.
            _overlay = Roi.UI.ArenaOverlay.Create(transform, _tab.field.zones_per_region);
            // Both of these ride the per-tick view too, and both belong to the FIGHT rather than
            // to the menus: the run summary raises itself when a dungeon falls, and the meter is
            // read while the fight runs.
            var combatPopups=new GameObject("CombatPopups",typeof(RectTransform),typeof(CanvasGroup));
            combatPopups.transform.SetParent(_bar.PopupLayer,false);
            var combatRect=(RectTransform)combatPopups.transform;
            combatRect.anchorMin=Vector2.zero;combatRect.anchorMax=Vector2.one;
            combatRect.offsetMin=combatRect.offsetMax=Vector2.zero;
            _combatPopupGroup=combatPopups.GetComponent<CanvasGroup>();
            _clear = Roi.UI.ClearSummary.Create(combatPopups.transform);
            // The Commander's row, on the BAR layer: it is part of the action bar, and anything
            // that covers one should cover both.
            _cmdr = Roi.UI.CommanderBar.Create(_bar.BarLayer);
            _cmdr.ActivateRequested += OnSkillRequested;
            // The band-boundary dip to black. Its own canvas UNDER the windows, so a screen left
            // open stays readable through the fade.
            _fade = ZoneFade.Create(transform);
            // On the POPUP layer with the clear summary, not the world: a wipe is a screen event.
            _death = Roi.UI.DeathBanner.Create(combatPopups.transform);
            _sparks = ArenaSparks.Create(transform);
            _sparks.Dur = FlashDur;
            _sparks.Bind(_cam);
            _shards = ArenaShards.Create(transform);
            _breakFlash = ArenaBreakFlash.Create(transform);
            _shards.Bind();
            // THINGS IN FLIGHT. Under the field root so it moves with the board, and on the actor
            // material so a bolt is lit by the same sun the sprites are.
            _shots = ArenaProjectiles.Create(_fieldRoot, this, _cam, ActorMat());
            // The AUDIO reads the same view, fed the same way and for the same reason: one parse,
            // one order, and no second subscription that could see a different tick.
            _sound = Roi.Audio.ArenaSound.Create(transform);
            _dps = Roi.UI.DpsMeter.Create(combatPopups.transform);
            _bar.Dps = _dps;
            // Parented to the ARENA, not to the bar: window mode switches the bar off.
            _companion = Roi.UI.CompanionWidget.Create(transform);
            _bar.Companion = _companion;
            BuildModeTabs();
            // A SCRIPTED DOOR for the one feature the Editor cannot test: `WindowMode` is a no-op
            // there on purpose, so proving it means a build, and a build cannot press W.
            foreach (var a in System.Environment.GetCommandLineArgs())
                if (a == "-roicompanion") { StartCoroutine(ArmCompanion()); break; }
        }

        void BuildModeTabs()
        {
            if (_modeTabs != null || _bar == null) return;
            _modeTabs = Roi.UI.ModeTabs.Create(transform);
            _modeTabs.OpenRequested += id => _bar.Toggle(id);
            _modeTabs.ModeChanged += combat =>
            {
                SetAim("", 0);
                _bar.CloseAll();
                if(!combat && _companion.Visible) _companion.Toggle();
                _bar.SetHomeMode(!combat);
                _combatPopupGroup.alpha=combat?1:0;
                _combatPopupGroup.blocksRaycasts=combat;
                _combatPopupGroup.interactable=combat;
                _cmdr.enabled = combat;
                _companion.enabled = combat;
            };
        }

        Roi.UI.HudBar _bar;
        Roi.UI.ModeTabs _modeTabs;

        /// The arena's own camera, for a second view that must frame the world the same way (the
        /// companion widget). Read-only by convention: nothing outside moves it.
        public Camera MainCamera => _cam;

        /// The PLAYER members on the field, in spawn order — who a companion pane can follow.
        public System.Collections.Generic.List<long> PlayerIds()
        {
            var list = new System.Collections.Generic.List<long>();
            foreach (var kv in _actors)
                if (kv.Value.IsPlayer && kv.Value.Alive) list.Add(kv.Key);
            list.Sort();
            return list;
        }

        /// One actor's class id, for a label that should say who it is watching rather than which
        /// entity number it is.
        public string ActorClass(long id)
            => _actors.TryGetValue(id, out var a) ? a.ClassId : "";

        /// Ids of the actors on screen, and one's popup anchor — for the HUD's test command.
        public System.Collections.Generic.List<long> ActorIds()
        {
            var list = new System.Collections.Generic.List<long>();
            foreach (var kv in _actors) if (kv.Value.Alive) list.Add(kv.Key);
            return list;
        }

        public Vector3 ActorAnchor(long id)
            => _actors.TryGetValue(id, out var a) && a.Root != null
                ? a.Root.transform.position + Vector3.up * (a.HeadY * 0.7f) : Vector3.zero;

        bool TryActorGround(long id, out Vector3 ground)
        {
            if (_actors.TryGetValue(id, out var a) && a.Root != null && a.Alive)
            { ground = a.Root.transform.position; return true; }
            ground = default; return false;
        }

        ArenaHud _hud;

        // arena3d_scene._build_lights + arena3d_look._apply_look. Two lights, and the SPLIT
        // between them is the point: the sun lights everything, the ground pool is culled off the
        // actor layer. It exists to vary the light across a flat floor; on a character it reads as
        // over-lit at point-blank range (Godot comment, 2026-08-24).
        void BuildLights()
        {
            var lk = _tab.look;
            var sunGo = new GameObject("Sun");
            sunGo.transform.SetParent(transform, false);
            var sun = sunGo.AddComponent<Light>();
            sun.type = LightType.Directional;
            sun.color = Col(lk.sun_color);
            sun.intensity = lk.key_energy;
            sun.shadows = LightShadows.Soft;
            sun.shadowStrength = lk.shadow_opacity;
            sunGo.transform.forward = SunDir(lk.sun_pitch, lk.sun_yaw);
            BuildCloudCookie(sun, lk);

            var fillGo = new GameObject("GroundFill");
            fillGo.transform.SetParent(transform, false);
            var fill = fillGo.AddComponent<Light>();
            fill.type = LightType.Point;
            fill.color = Col(lk.sun_color);
            // FALLOFF MODELS DIFFER, so the energy cannot be copied. Godot's omni fades as
            // (1 - d/range); URP's point light is inverse-square. At the pool's own height that
            // is 0.65 against 0.012 — a factor of ~54, which is why the ground read flat and dark
            // with the number copied across. Match them where the light is FOR: the ground under
            // the pool.
            fill.intensity = FillIntensity(lk);   // re-derived per tint in ApplyDaylight
            fill.range = lk.fill_range;
            fill.shadows = LightShadows.None;
            fill.cullingMask = ~((1 << ActorLayer) | (1 << GroundLayer));
            fill.enabled = lk.fill_energy > 0f;
            // Over the middle of the PLAY AREA, not of the art: the pool brightens where the
            // fight is.
            fillGo.transform.position = new Vector3(0f, lk.fill_height, FieldZShift);

            // ambient_light_color x ambient_light_energy — Godot keeps them apart, Unity has one
            // colour, so the product is what carries over. A flat grey here (what this scene had)
            // is both the wrong hue and ~3x the strength.
            _sun = sun; _fill = fill;
            ApplyDaylight(true);
            ApplyActorExposure();

        }

        // A GLOBAL, exactly as Godot sets it: one value every actor material reads, so the actors
        // can be dimmed without touching the ground, the props or the lights. Set from BOTH entry
        // paths — it used to live at the end of BuildLights, which a BAKED zone never runs, so a
        // build that booted straight into Zone_r1 multiplied every sprite by 0 and every actor and
        // prop rendered as a black silhouette.
        void ApplyActorExposure() => Shader.SetGlobalFloat("_ActorExposure", _tab.globals.actor_exposure);

        // The sun's DIRECTION, derived rather than copied as Euler angles. This world mirrors x
        // (CellToWorld) and leaves z alone, so re-using Godot's yaw in a Unity rotation flips BOTH
        // axes and the shadows fall the wrong way — visible the moment anything tall casts one.
        // Godot: the light points along its local -Z after rotate(pitch, yaw). Mirror the result's
        // x, keep y and z, and set that as forward.
        static Vector3 SunDir(float pitchDeg, float yawDeg)
        {
            float p = pitchDeg * Mathf.Deg2Rad, y = yawDeg * Mathf.Deg2Rad;
            // Rx(p) applied to (0,0,-1), then Ry(y).
            float dy = Mathf.Sin(p), dz = -Mathf.Cos(p);
            float dx = dz * Mathf.Sin(y);
            dz = dz * Mathf.Cos(y);
            return new Vector3(-dx, dy, dz).normalized;   // -x: this world is mirrored in x
        }

        // Environment tonemap + adjustments + glow, as a runtime URP Volume. Godot puts these on
        // the Environment; URP has no equivalent outside a volume, so one is built here rather
        // than authored in the scene — the values must keep coming from the generated table.
        void BuildPost()
        {
            var g = _tab.globals;
            var go = new GameObject("PostFX");
            go.transform.SetParent(transform, false);
            var vol = go.AddComponent<Volume>();
            vol.isGlobal = true;
            var profile = ScriptableObject.CreateInstance<VolumeProfile>();
            vol.sharedProfile = profile;

            var tm = profile.Add<Tonemapping>(true);
            tm.mode.overrideState = true;
            // NEUTRAL, not ACES. ACES crushes the midtones and desaturates the highlights; the
            // scene came out 36% darker than the Godot frame measured over the same crop, and this
            // is half of that gap. Godot's FILMIC is the milder curve, and Neutral is its match.
            // Neutral, not None either: a soft roll-off on the highlights so a sunlit road and a
            // lit sprite stop clipping to paper white (v39 had both).
            tm.mode.value = TonemappingMode.Neutral;

            var ca = profile.Add<ColorAdjustments>(true);
            ca.contrast.overrideState = true;
            // Godot 1.16 -> URP +16, plus 10: under Neutral's roll-off the frame went milky (v40),
            // and the reference frames are contrasty — soft highlights, firm midtones.
            ca.contrast.value = (g.contrast - 1f) * 100f + 10f;
            ca.saturation.overrideState = true;
            // Godot's -2 plus a pull on the neon: the region art's greens are pure pigment and
            // read as a lawn in a photo; the reference frames sit nearer -10.
            ca.saturation.value = (g.saturation - 1f) * 100f + 4f;   // -8 read as a faded print (v40); +4 with the split toning holds cozy
            ca.postExposure.overrideState = true;
            // Godot's adjustment_brightness is a MULTIPLY; URP post-exposure is in stops.
            ca.postExposure.value = Mathf.Log(Mathf.Max(0.01f, g.brightness), 2f) + 0.12f;   // Neutral takes a little top end back

            // THE GRADE (user 2026-09-06: "คิดถึง cinematography + color grading ของฉาก ถึงจะเป็นแค่
            // ฉากเล็กๆ"): one idea, warm light / cool shade. The sun is already warm and the
            // ambient already sky-blue (Godot's), so the split toning pushes the same two ways —
            // highlights toward sun-gold, shadows toward a dusty blue — and the lift keeps the
            // blacks from going to ink, which a cozy daylight frame needs. All of it lives in the
            // profile asset, so it is a slider in the Inspector, not a rebuild.
            var st = profile.Add<SplitToning>(true);
            // Settled live over v40-v42: the first pass (0.42,0.50,0.68 / balance -15 / lift 0.015)
            // was a dusty blue wash; this keeps the split and loses the dust.
            st.shadows.overrideState = true;    st.shadows.value = new Color(0.50f, 0.56f, 0.72f);
            st.highlights.overrideState = true; st.highlights.value = new Color(1.0f, 0.90f, 0.74f);
            st.balance.overrideState = true;    st.balance.value = -5f;
            var lgg = profile.Add<LiftGammaGain>(true);
            lgg.lift.overrideState = true;  lgg.lift.value = new Vector4(1f, 1f, 1.03f, 0.006f);   // a breath of blue in the blacks
            lgg.gamma.overrideState = true; lgg.gamma.value = new Vector4(1f, 1f, 1f, 0f);
            lgg.gain.overrideState = true;  lgg.gain.value = new Vector4(1.04f, 1.0f, 0.95f, 0f);  // warm the top end

            var bl = profile.Add<Bloom>(true);
            bl.threshold.overrideState = true;
            bl.threshold.value = g.glow_threshold;
            bl.intensity.overrideState = true;
            bl.intensity.value = g.glow_intensity;
            bl.scatter.overrideState = true;
            bl.scatter.value = 0.7f;

            // The atmosphere, the URP way: volume overrides rather than a port of Godot's
            // camera-quad vignette and CameraAttributes DoF. Godot's vignette is a
            // smoothstep(0.38, 1) on the frame radius at 0.65; URP's own curve is a different
            // shape, so intensity/smoothness are set for the same READ (dark surround, play
            // area untouched), not the same numbers. Gated by the table: 0 = the override is off.
            var lk = _tab.look;
            var vg = profile.Add<Vignette>(true);
            vg.active = lk.vignette > 0f;
            vg.color.overrideState = true;
            vg.color.value = new Color(0.02f, 0.02f, 0.05f);     // vignette.gdshader `shade`
            vg.intensity.overrideState = true;
            // 0.37 of Godot's 0.65: at 0.55 the back rows of grass went black on this map
            // (user: "vignette อาจจะมืดไป หญ้าด้านหลังมืดหมด"); the darkening now stays in the corners.
            vg.intensity.value = Mathf.Clamp01(lk.vignette * 0.37f);
            vg.smoothness.overrideState = true;
            vg.smoothness.value = 0.6f;
            vg.rounded.overrideState = true;
            vg.rounded.value = false;                            // follows the 16:9 frame, as Godot's UV falloff does
            _vignette = vg;

            // Far blur only: Godot's near band exists for the HD2D foreground canopy, which
            // this scene has not got. Gaussian = URP's cheap mode; start/end are Godot's
            // far distance + transition, measured along the view ray in both engines.
            var dof = profile.Add<DepthOfField>(true);
            dof.active = lk.dof_far_distance > 0f;
            dof.mode.overrideState = true;
            dof.mode.value = DepthOfFieldMode.Gaussian;
            dof.gaussianStart.overrideState = true;
            dof.gaussianStart.value = lk.dof_far_distance;
            dof.gaussianEnd.overrideState = true;
            dof.gaussianEnd.value = lk.dof_far_distance + lk.dof_far_transition;
            dof.gaussianMaxRadius.overrideState = true;
            // dof_amount IS the blur radius and it was being ignored — hardcoding 1 blurred the
            // whole frame (user, 2026-09-06: "เบลอผิดปกติ"). Godot's dof_blur_amount runs 0..1 and
            // the look is tuned to 0.13 ("pixel art smears fast, so kept modest"); URP's
            // gaussianMaxRadius runs 0.5..1.5, so the same fraction lands just above its minimum.
            dof.gaussianMaxRadius.value = Mathf.Clamp(0.5f + lk.dof_amount, 0.5f, 1.5f);
            _dof = dof;
        }

        Vignette _vignette;
        DepthOfField _dof;
        /// The per-frame decorative motion gate (ArenaRenderData.FX_ENABLED): god rays and
        /// motes animate every frame and go dark in battery/companion mode. Vignette and DoF are
        /// static post and stay.
        public bool FxEnabled = true;
        bool _fxApplied = true, _sunnyApplied;
        Transform _atmos;

        // Light shafts and dust, the pack's way: ITS particle prefabs placed in the WORLD, not
        // Godot's screen-quad shaders re-derived. Sunny regions only (the same SUNNY_REGIONS
        // gate as Godot: a gothic manor gets no sunbeams). The shafts stand along the far side
        // of the play area, leaning WITH the sun — the prefab's shaft runs up its local Y, so
        // that axis is pointed back at the light. Placement is scenery, not authored data.
        Transform _atmosCam;
        // LookSettings.GODRAY_ANGLE is -0.35 rad; the sign is a screen convention, so this is the
        // reference's read — shafts falling from the top-left — not a copied number.
        const float RayAngleDeg = 20f;

        void BuildAtmosphere()
        {
            if (_atmos != null) Kill(_atmos.gameObject);
            if (_atmosCam != null) Kill(_atmosCam.gameObject);
            _atmos = null; _atmosCam = null;
            var r = _tab.RegionOf(_region);
            var lk = _tab.look;
            bool sunny = r != null && r.sunny;
            _sunnyApplied = sunny;
            if (!sunny) return;
            _atmos = new GameObject("Atmosphere").transform;
            _atmos.SetParent(_fieldRoot, false);
            float cz = FieldZShift + (r != null ? r.z_shift : 0f);

            var rays = Resources.Load<GameObject>("Dressing3D/GodRays");
            if (rays != null && lk.godray > 0f)
            {
                // ON THE CAMERA, not in the scene (user 2026-09-06: "ต้อง reference กับกล้อง ไม่ใช่
                // reference กับฉาก… ของ godot ก็ติดกล้อง"). Godot's rays are a quad riding the
                // lens; these ride it the same way — laid out in CAMERA space across the top
                // third of the frame, so any map gets them in the same place and a camera nudge
                // carries them along. ONE screen angle for all of them (Godot's GODRAY_ANGLE, a
                // single tilt in UV space): a tilt about the view axis, with the ray quads in the
                // image plane. Leaning them toward the sun in WORLD instead gave five parallel
                // shafts that perspective turned into five different angles on screen
                // (user: "มันมีหลายมุมปนๆกัน").
                var camRoot = new GameObject("AtmosphereCam").transform;
                camRoot.SetParent(_cam.transform, false);
                _atmosCam = camRoot;
                var tilt = Quaternion.Euler(0f, 0f, RayAngleDeg);
                float d = 18f;                                        // ahead of the lens
                float halfH = d * Mathf.Tan(_cam.fieldOfView * 0.5f * Mathf.Deg2Rad);
                float halfW = halfH * Mathf.Max(_cam.aspect, 1f);
                var rng = new System.Random(919);
                // Three, short and sparse (user: "godray เยอะไปหน่อย กับ ยาวไป"): five at 5 tall
                // covered the whole top of the frame in light.
                for (int i = 0; i < 3; i++)
                {
                    float x = Mathf.Lerp(-halfW, halfW, (i + 0.5f) / 3f) + (float)(rng.NextDouble() - 0.5) * 2f;
                    float y = halfH * Mathf.Lerp(0.35f, 0.8f, (float)rng.NextDouble());   // top third
                    var go = Instantiate(rays, camRoot);
                    go.name = "GodRays";
                    go.transform.localPosition = new Vector3(x, y, d);
                    go.transform.localRotation = tilt;
                    go.transform.localScale = new Vector3(1f, 3f, 1f);
                    var ps = go.GetComponent<ParticleSystem>();
                    if (ps != null)
                    {
                        // Shorter lives and a stronger sway (user: "animate ให้ชัดขึ้น"): the pack's
                        // rays live 20 s and barely move, which at this size reads as a static
                        // overlay. 6..10 s means a shaft is always fading in or out somewhere.
                        // 10..16 s lives and a faint sway: 6..10 s with a 0.6 noise was "ขยับเยอะไป".
                        var em = ps.emission; em.rateOverTimeMultiplier = 3f;
                        var mn0 = ps.main; mn0.startLifetime = new ParticleSystem.MinMaxCurve(10f, 16f);
                        var nz = ps.noise; nz.enabled = true; nz.strengthMultiplier = 0.2f; nz.frequency = 0.08f; nz.scrollSpeedMultiplier = 0.15f;
                        // The pack's ray is 7% alpha for a first-person walk under the canopy;
                        // from this height and pitch it needs more to be a beam at all.
                        var mn = ps.main; var col = mn.startColor; var c = col.color; c.a *= 2.2f; col.color = c; mn.startColor = col;
                    }
                }
            }
            // Drifting petals (the blossom trees') and leaves, from the pack's demo. Their emitter
            // is the demo terrain's mesh; here a box over the region art, a few units up, the
            // count kept to the art's area. Same gate as the motes: a sunny region, FX on.
            foreach (var (name, height, perArea) in new[] { ("Petals", 5f, 0.004f), ("Leaves", 6f, 0.003f) })
            {
                var pf = Resources.Load<GameObject>("Dressing3D/" + name);
                if (pf == null || lk.motes <= 0f) continue;
                var go = Instantiate(pf, _atmos);
                go.name = name;
                go.transform.localPosition = new Vector3(0f, height, cz);
                foreach (var ps in go.GetComponentsInChildren<ParticleSystem>())
                {
                    var sh = ps.shape;
                    sh.shapeType = ParticleSystemShapeType.Box;
                    sh.scale = new Vector3(r.w_cells, 1f, r.h_cells);
                    var em = ps.emission;
                    em.rateOverTimeMultiplier = r.w_cells * r.h_cells * perArea;
                }
            }
            var motes = Resources.Load<GameObject>("Dressing3D/Motes");
            if (motes != null && lk.motes > 0f)
            {
                var go = Instantiate(motes, _atmos);
                go.name = "Motes";
                go.transform.localPosition = new Vector3(0f, 0.5f, cz);
                var ps = go.GetComponent<ParticleSystem>();
                if (ps != null)
                {
                    // The prefab covers a 100x100 terrain; box it to the region art and keep
                    // the same density per unit area, scaled by the table's knob.
                    var sh = ps.shape;
                    float w = r.w_cells, h = r.h_cells;
                    var was = sh.scale;
                    sh.scale = new Vector3(w, was.y, h);
                    var em = ps.emission;
                    float area = (w * h) / Mathf.Max(1f, was.x * was.z);
                    em.rateOverTimeMultiplier = em.rateOverTimeMultiplier * area * lk.motes * 0.6f;
                    // The pack's dust is sized for a first-person walk; from this lens the
                    // motes were coins. Under half size, and Godot's motes barely creep, so
                    // fewer of them too.
                    var mn = ps.main;
                    mn.startSizeMultiplier = mn.startSizeMultiplier * 0.45f;
                }
            }
            HideUnavailableDecoration();
            _fxApplied = !FxEnabled;   // force the gate to apply on the next frame
        }

        // DRIFTING CLOUD SHADOW, the URP way: a light COOKIE on the sun. Godot shares one noise
        // between the ground shader and every sprite shader (cloud_shadow.gdshaderinc) so the
        // same shade lands on the floor and on whoever stands on it; a cookie on the light does
        // that for free — it is a mask on the light itself, so everything the sun touches reads
        // it (TerrainLit and the pack's shaders included, and ours via _LIGHT_COOKIES).
        //
        // The texture is Godot's shaping baked once: the same value noise and hash, two octaves,
        // biased so most ground is in sun (smoothstep 0.55..0.85), 1 - shade. What is lost:
        // Godot moves its two octaves at different speeds so the pattern never repeats; a
        // cookie is one sheet that translates, so the second octave is 2x (not 2.3x) to keep it
        // TILEABLE — the lattice wraps every P cells and the cookie repeats every P/cloud_scale
        // world units. That is the trade for not writing a fourth shader.
        static Texture2D _cloudTex;
        UniversalAdditionalLightData _sunData;
        float _cloudPeriod;
        Vector2 _cloudOffset;
        public bool CloudEnabled = true;

        static float CloudHash(double x, double y)
        {
            double px = x * 123.34, py = y * 345.45;
            px -= System.Math.Floor(px); py -= System.Math.Floor(py);
            double d = px * (px + 34.345) + py * (py + 34.345);
            px += d; py += d;
            double v = px * py;
            return (float)(v - System.Math.Floor(v));
        }

        /// Godot's _cloud_vnoise on a lattice that wraps every `period` cells.
        static float CloudNoise(float x, float y, int period)
        {
            int ix = Mathf.FloorToInt(x), iy = Mathf.FloorToInt(y);
            float fx = x - ix, fy = y - iy;
            fx = fx * fx * (3f - 2f * fx); fy = fy * fy * (3f - 2f * fy);
            int x0 = ((ix % period) + period) % period, x1 = (x0 + 1) % period;
            int y0 = ((iy % period) + period) % period, y1 = (y0 + 1) % period;
            float a = CloudHash(x0, y0), b = CloudHash(x1, y0), c = CloudHash(x0, y1), d = CloudHash(x1, y1);
            return Mathf.Lerp(Mathf.Lerp(a, b, fx), Mathf.Lerp(c, d, fx), fy);
        }

        static float Smooth01(float e0, float e1, float x)
        {
            x = Mathf.Clamp01((x - e0) / (e1 - e0));
            return x * x * (3f - 2f * x);
        }

        void BuildCloudCookie(Light sun, Look lk)
        {
            if (lk.cloud <= 0f) return;
            const int P = 6, N = 256;
            if (_cloudTex == null)
            {
                _cloudTex = new Texture2D(N, N, TextureFormat.RGB24, false)
                { wrapMode = TextureWrapMode.Repeat, filterMode = FilterMode.Bilinear, name = "CloudCookie" };
                var px = new Color[N * N];
                for (int y = 0; y < N; y++)
                    for (int x = 0; x < N; x++)
                    {
                        float u = x / (float)N * P, v = y / (float)N * P;
                        float n = CloudNoise(u, v, P) * 0.6f + CloudNoise(u * 2f, v * 2f, P * 2) * 0.4f;
                        float shade = Smooth01(0.55f, 0.85f, n) * lk.cloud;
                        px[y * N + x] = Color.white * (1f - shade);
                    }
                _cloudTex.SetPixels(px);
                _cloudTex.Apply(false);
            }
            _cloudPeriod = P / Mathf.Max(lk.cloud_scale, 0.0001f);
            _sunData = sun.GetComponent<UniversalAdditionalLightData>();
            if (_sunData == null) _sunData = sun.gameObject.AddComponent<UniversalAdditionalLightData>();
            _sunData.lightCookieSize = Vector2.one * _cloudPeriod;
            sun.cookie = _cloudTex;
            Debug.Log($"cloud cookie {N}px, period {_cloudPeriod:F1} units, shade {lk.cloud}");
        }

        void ApplyCloud(float dt)
        {
            if (_sun == null || _sunData == null) return;
            bool on = FxEnabled && CloudEnabled && _tab.look.cloud > 0f;
            var want = on ? _cloudTex : null;
            if (_sun.cookie != want) _sun.cookie = want;
            if (!on) return;
            // Godot drifts octave 1 by (t, 0.3t) lattice cells at cloud_speed world units/s.
            _cloudOffset += new Vector2(1f, 0.3f) * (_tab.look.cloud_speed * dt / _cloudPeriod);
            _cloudOffset = new Vector2(Mathf.Repeat(_cloudOffset.x, 1f), Mathf.Repeat(_cloudOffset.y, 1f));
            _sunData.lightCookieOffset = _cloudOffset;
        }

        void ApplyAtmosphere()
        {
            if (_fxApplied == FxEnabled) return;
            if (_atmos == null) { _fxApplied = FxEnabled; if (_wetSheen != null) _wetSheen.SetFloat("_WetRipple", FxEnabled ? 0.5f : 0f); return; }
            _fxApplied = FxEnabled;
            if (_wetSheen != null) _wetSheen.SetFloat("_WetRipple", FxEnabled ? 0.5f : 0f);
            foreach (var root in new[] { _atmos, _atmosCam })
            {
                if (root == null) continue;
                foreach (var ps in root.GetComponentsInChildren<ParticleSystem>(true))
                {
                    if (FxEnabled) { ps.gameObject.SetActive(true); ps.Play(true); }
                    else ps.gameObject.SetActive(false);
                }
            }
        }

        static Material _actorMat;
        Material ActorMat()
        {
            if (_actorMat != null) return _actorMat;
            var sh = Shader.Find("ROI/ActorSprite");
            if (sh == null) { Debug.LogError("ROI/ActorSprite shader missing"); return null; }
            _actorMat = new Material(sh);
            return _actorMat;
        }

        // TIME OF DAY, the arena3d_look pass. The tint multiplies the sun, the fill and the
        // ambient, so it reaches the pixel art as well as the ground — that is the whole point of
        // the sprites being LIT rather than unlit, and it is why an evening scene reads as evening
        // instead of as a warm floor with daylight characters standing on it.
        [Range(0f, 1f)] public float TimeOfDay = 0f;
        Light _sun, _fill;
        float _todApplied = -1f;

        /// The stops are a CYCLE — 0 and 1 are both noon — evenly spaced, so a driver can run past
        /// 1.0 without a special case (ArenaRenderData.daylight_color).
        Color DaylightColor(float t)
        {
            var st = _tab.daylight_stops;
            if (st == null || st.Length < 2) return Color.white;
            int segs = st.Length - 1;
            float x = Mathf.Repeat(t, 1f) * segs;
            int i = Mathf.Clamp((int)x, 0, segs - 1);
            return Color.Lerp(Col(st[i].rgb), Col(st[i + 1].rgb), x - i);
        }

        void ApplyDaylight(bool force = false)
        {
            if (!force && Mathf.Approximately(_todApplied, TimeOfDay)) return;
            _todApplied = TimeOfDay;
            var lk = _tab.look;
            Color c = DaylightColor(TimeOfDay);
            // The tint can only SUBTRACT, so a golden hour takes most of the blue with it. Divide
            // the loss back out by LUMA (not the channel average — an average over-corrects a blue
            // night and under-corrects a yellow dusk), and let night_dim be the only thing that
            // darkens. Otherwise "warmer" and "darker" become the same knob.
            float luma = Mathf.Max(Luma(c), 0.25f);
            float dim = Mathf.Lerp(lk.night_dim, 1f, (c.r + c.g + c.b) / 3f);
            Color sunCol = Col(lk.sun_color) * c;
            if (_sun != null) { _sun.color = sunCol; _sun.intensity = lk.key_energy / luma * dim; }
            // The same pool, for the surfaces that shade it themselves. Energy through the same
            // units bridge as the engine light (FillMatch), tint and dim exactly as above.
            Shader.SetGlobalVector("_FillPos", new Vector4(0f, lk.fill_height, FieldZShift, lk.fill_range));
            // Energy x the units bridge only: the shader applies Godot's own `1 - d/range`, so
            // handing it FillIntensity (which folds that attenuation in for the engine light)
            // would count the falloff twice.
            Shader.SetGlobalVector("_FillColor", sunCol * (lk.fill_energy * FillMatch / luma * dim));
            if (_fill != null)
            {
                _fill.color = sunCol;
                _fill.intensity = FillIntensity(lk) / luma * dim;
            }
            RenderSettings.ambientMode = AmbientMode.Flat;
            RenderSettings.ambientLight = Col(lk.ambient_color) * c * lk.ambient_energy;
            TintGrassForHour(c);
        }

        // The ground pool, converted between two different falloff models AND then calibrated.
        //
        // Godot's omni fades as `1 - d/range`, URP's point light as inverse-square, so the energy
        // cannot be copied — that part is derived. What the derivation alone does NOT give is the
        // engines' light UNITS, and matching at the pool's own height put Unity far over: measured
        // in the middle of the play area, Godot reads 145,196,76 with 8% of pixels clipped where
        // Unity read 208,249,124 with 86% clipped. `Match` is that measurement, and it is the only
        // number in this file taken from a picture rather than from a table — so it stays named,
        // alone, and next to the evidence.
        const float FillMatch = 0.4f;

        static float FillIntensity(Look lk)
        {
            float fd = Mathf.Max(0.01f, lk.fill_height);
            float godotAtt = Mathf.Max(0f, 1f - fd / Mathf.Max(0.01f, lk.fill_range));
            return lk.fill_energy * godotAtt * fd * fd * FillMatch;
        }

        /// Destroy that works in BOTH modes: the environment builders run at runtime and from
        /// the Editor bake (ZoneBaker), and Destroy() throws outside play mode.
        static void Kill(Object o)
        {
            if (o == null) return;
            if (Application.isPlaying) Object.Destroy(o); else Object.DestroyImmediate(o);
        }

        static Color Col(float[] c) =>
            c != null && c.Length >= 3 ? new Color(c[0], c[1], c[2]) : Color.white;

        // arena3d_scene._refresh_background: ONE background PNG per region, laid on a quad sized by
        // the SHARED pixel scale so the art and the sprites keep the same pixel size. Art at least
        // 3 rooms wide is a band STRIP centred on the middle room (the scroll never moves it);
        // anything narrower is a single-room floor. No art for the region -> a flat fallback, which
        // is what this scene had for every region before.
        /// Drop the vegetation materials this view CLONED, so the next region makes its own.
        ///
        /// `_vegTint` is static and keyed by source material, and its values are `new Material(m)`
        /// clones carrying one region's palette. Nothing cleared them between regions, so coming
        /// back to band 5 from band 6 re-dressed band 5 with band 6's clones — and where a clone
        /// had been destroyed under it, with magenta. `ResetStatics` exists for exactly this and is
        /// called only by the Editor bake; a region change needs the same thing.
        ///
        /// ONLY the clones are destroyed. `_tuftMats` also collects materials off the ADOPTED scene
        /// (see AdoptBakedEnvironment) — those are project assets, and destroying one would damage
        /// the scene on disk rather than just this session.
        void ResetVegetationCache()
        {
            foreach (var kv in _vegTint) Kill(kv.Value);
            _vegTint.Clear();
            _tuftMats.Clear();          // cleared, never destroyed: it holds borrowed materials too
            _grassMats.Clear();
            _layerMean.Clear();
            if (_grassMat != null) { Kill(_grassMat); _grassMat = null; }
        }

        void RebuildGround()
        {
            var existing = _fieldRoot.Find("Ground");
            if (existing != null) Kill(existing.gameObject);
            var surround = _fieldRoot.Find("Surround");
            if (surround != null) Kill(surround.gameObject);
            var r = _tab.RegionOf(_region);
            var tex = r != null ? Sheet(r.bg) : null;

            if (tex == null)
            {
                var g = GameObject.CreatePrimitive(PrimitiveType.Plane);
                g.name = "Ground";
                g.transform.SetParent(_fieldRoot, false);
                g.transform.localScale = new Vector3(_gw / 10f, 1f, _gh / 10f);
                g.transform.localPosition = new Vector3(0f, 0f, FieldZShift);
                g.GetComponent<MeshRenderer>().sharedMaterial = MakeMat(new Color(0.22f, 0.42f, 0.30f));
                return;
            }

            BuildAtmosphere();
            if (Style == ZoneStyle.Full3D) { BuildDressing(); BuildTerrain3D(r, tex); return; }
            var q = GameObject.CreatePrimitive(PrimitiveType.Quad);
            q.name = "GroundQuad";
            q.transform.SetParent(_fieldRoot, false);
            q.transform.localRotation = Quaternion.Euler(90f, 0f, 0f);   // stand the quad on the floor
            // NEGATIVE x: this scene mirrors world x to cancel the 180-yaw camera (see CellToWorld).
            // Points survive that with a sign flip; a TEXTURE does not — unflipped, the art would be
            // the mirror image of the same art in Godot while every sprite sat the right way round.
            q.transform.localScale = new Vector3(-r.w_cells, r.h_cells, 1f);
            float x = r.strip ? -_gw : 0f;   // strip is centred on the middle room of the band
            q.transform.localPosition = new Vector3(x, 0f, FieldZShift + r.z_shift);
            BuildDressing();
            var sh = Shader.Find("Universal Render Pipeline/Lit") ?? Shader.Find("Standard");
            var m = new Material(sh);
            if (m.HasProperty("_BaseMap")) m.SetTexture("_BaseMap", tex); else m.mainTexture = tex;
            // V is flipped as well as U. The mirrored x scale above turns the quad over, and the
            // rotation that stands it on the floor puts the image's top edge on the NEAR side —
            // so the art arrived upside down while the path and the sprites still lined up, which
            // is the kind of wrong that reads as "the map is rotated" rather than "the UV is".
            // Flip it in UV space, not with another negative scale: a second mirrored axis turns
            // the normal back down and the quad disappears.
            var st = new Vector4(1f, -1f, 0f, 1f);
            if (m.HasProperty("_BaseMap")) { m.SetTextureScale("_BaseMap", new Vector2(st.x, st.y)); m.SetTextureOffset("_BaseMap", new Vector2(st.z, st.w)); }
            else { m.mainTextureScale = new Vector2(st.x, st.y); m.mainTextureOffset = new Vector2(st.z, st.w); }
            if (m.HasProperty("_Smoothness")) m.SetFloat("_Smoothness", 0.05f);
            q.GetComponent<MeshRenderer>().sharedMaterial = m;
            BuildTerrain(r, tex, q.transform.localPosition);
            q.GetComponent<MeshRenderer>().enabled = false;   // the terrain IS the ground
        }

        // The ground as a TERRAIN, with the grass as its DETAILS — chosen over scattered prefabs
        // because the ground art is heading for textures rather than hand-painted pixels, and a
        // terrain brings layers, LOD, detail density and collision with it, all of which are
        // hand-written work on a quad. Details are also instanced: 165 draw calls against 437.
        void BuildTerrain(Region r, Texture2D tex, Vector3 centre)
        {
            var old = _fieldRoot.Find("Ground");
            if (old != null) Kill(old.gameObject);
            float w = r.w_cells, h = r.h_cells;
            var td = new TerrainData { heightmapResolution = 33, size = new Vector3(w, 1f, h) };
            // A terrain cannot be mirrored (no negative scale) and this scene mirrors world x, so
            // the TEXTURE is flipped instead — the same trick the quad's material pulls with UV.
            td.terrainLayers = new[]
            {
                new TerrainLayer
                {
                    diffuseTexture = FlipTexture(tex), tileSize = new Vector2(w, h),
                    specular = Color.black, metallic = 0f, smoothness = 0f,
                }
            };
            var go = Terrain.CreateTerrainGameObject(td);
            go.name = "Ground";
            go.layer = GroundLayer;
            go.transform.SetParent(_fieldRoot, false);
            go.transform.localPosition = new Vector3(centre.x - w * 0.5f, centre.y - 0.01f, centre.z - h * 0.5f);
            var terrain = go.GetComponent<Terrain>();
            terrain.materialTemplate = GroundMaterial(FlipTexture(tex), r);
            terrain.drawInstanced = false;   // the custom ground shader has no instancing variant
            BuildBladeField(tex, centre, w, h);
            Debug.Log($"terrain {w:F1}x{h:F1}");
        }

        // The terrain's surface runs the ported ground shader rather than TerrainLit: a flat
        // texture is not what this floor looks like in Godot. Wet is gated on the region being
        // SUNNY, the same gate `_apply_look` uses — a shaded region has no sun to glint off.
        Material GroundMaterial(Texture2D tex, Region r)
        {
            var sh = Shader.Find("ROI/GroundArt");
            if (sh == null) { Debug.LogWarning("ROI/GroundArt missing"); return null; }
            var lk = _tab.look;
            var m = new Material(sh);
            m.SetTexture("_MainTex", tex);
            m.SetFloat("_ShadeCrush", lk.shade_crush);
            m.SetFloat("_ShadeCeiling", lk.shade_ceiling);
            m.SetFloat("_WetAmount", r.sunny ? lk.wet : 0f);
            m.SetFloat("_WetRough", lk.wet_rough);
            m.SetFloat("_WetSat", lk.wet_sat);
            m.SetFloat("_WetGain", lk.wet_gain);
            m.SetFloat("_SunPitch", lk.sun_pitch);
            m.SetFloat("_SunYaw", lk.sun_yaw);
            m.SetColor("_SunCol", Col(lk.sun_color));
            return m;
        }

        // THE BLADE FIELD. BinbunGrass is a MultiMesh of thin quads, not a bush model, and that
        // silhouette is the thing the pack's mesh could not give — so the blades are built here,
        // one mesh of quads, exactly as grass_clump.gd lays them out:
        //   r = radius * pow(rand, 0.6)   leans the scatter toward the middle. 0.5 spreads it
        //                                 evenly and gives a clump a hard edge; this thins it so
        //                                 clumps dropped next to each other blend.
        //   scale 0.75..1.35, free spin, base at y = 0 so the clump stands ON the ground.
        // Deterministic from the authored layout_seed, the way a seeded MultiMesh is.
        // The Full3D ground: the pack's terrain layers, PAINTED from the same region art so the
        // layout is the authored one — cobblestone where the art is grey stone, dirt where it is
        // brown, grass elsewhere. Same size and placement as the HD2D terrain; the blades and
        // props are rebuilt on top by the same code paths.
        void BuildTerrain3D(Region r, Texture2D tex)
        {
            var old = _fieldRoot.Find("Ground");
            if (old != null) Kill(old.gameObject);
            var grassL = Resources.Load<TerrainLayer>("Dressing3D/Grass");
            var stoneL = Resources.Load<TerrainLayer>("Dressing3D/" + StoneLayer) ?? Resources.Load<TerrainLayer>("Dressing3D/Cobblestone");
            var dirtL = Resources.Load<TerrainLayer>("Dressing3D/Dirt");
            if (grassL == null || stoneL == null || dirtL == null) { Debug.LogWarning("Dressing3D layers missing — run TerrainGrassTest.MakeDressing3DAssets"); return; }

            float w = r.w_cells, h = r.h_cells;
            var centre = new Vector3(0f, 0f, FieldZShift + r.z_shift);
            // The pack's layers wear the pack's palette — its grass is a yellow the region art
            // never has (measured: art grass 100,158,46 against the layer's 167,165,19). Each
            // layer is tinted to the MEAN of the art pixels it stands in for, so the 3D ground
            // keeps the region's colours the way it keeps the region's layout. Copies, not the
            // assets: Resources.Load hands back the asset itself, and a remap set on that would
            // be saved into the project by the Editor.
            var art = ArtMeans(tex);
            // WET, the PBR way: the stone layer gets the smoothness Godot's roughness implies
            // and a touch darker (GroundArt's _WetDarken); the real BRDF and the sky reflection
            // do the rest. Godot builds its glint by hand because from this camera a high sun
            // reflects into the sky, not the lens — so what reads here is the fresnel/sky term,
            // not a hot spot, and the frame decides whether that is enough. Same gates as the
            // HD2D ground: sunny region, table strength.
            var lk = _tab.look;
            bool wet = r.sunny && lk.wet > 0f;
            var layers = new[] {
                TintedLayer(grassL, art[0]),
                TintedLayer(stoneL, art[1], wet ? (1f - lk.wet_rough) * lk.wet : 0f, wet ? 0.22f * lk.wet : 0f, StoneTile),
                TintedLayer(dirtL, art[2]) };
            var td = new TerrainData { heightmapResolution = 65, size = new Vector3(w, 4f, h) };
            td.terrainLayers = layers;
            const int A = 128;
            td.alphamapResolution = A;
            var maps = new float[A, A, 3];
            for (int y = 0; y < A; y++)
                for (int x = 0; x < A; x++)
                {
                    int k = ArtClass(tex.GetPixelBilinear(1f - (x + 0.5f) / A, 1f - (y + 0.5f) / A));
                    maps[y, x, 0] = k == 0 ? 1f : 0f;
                    maps[y, x, 1] = k == 1 ? 1f : 0f;
                    maps[y, x, 2] = k == 2 ? 1f : 0f;
                }
            td.SetAlphamaps(0, 0, maps);

            var go = Terrain.CreateTerrainGameObject(td);
            go.name = "Ground";
            // DEFAULT layer, not GroundLayer: that layer exists to cull the engine's fill pool off
            // surfaces that light the pool THEMSELVES (GroundArt, the blades). TerrainLit does not,
            // so on GroundLayer the Full3D floor got sun + ambient only and the back of the field
            // read dark; on the default layer the same pool the HD2D floor computes lands here.
            go.layer = 0;
            go.transform.SetParent(_fieldRoot, false);
            go.transform.localPosition = new Vector3(centre.x - w * 0.5f, centre.y - 0.01f, centre.z - h * 0.5f);
            go.GetComponent<Terrain>().drawInstanced = true;
            BuildSurround3D(centre, w, h, layers[0], layers[2]);
            if (PackGrass) BuildPackGrass(tex, centre, w, h); else BuildBladeField(tex, centre, w, h);
            BuildFlora3D(tex, centre, w, h);
            BuildWetSheen(r, tex, wet);
            Debug.Log($"terrain3d {w:F1}x{h:F1}");
        }

        // The wet sheen over the Full3D road (WetSheen.shader): an additive quad laid EXACTLY
        // where the HD2D ground quad goes — same mirrored x scale, same V flip — so the art's
        // saturation mask lands on the terrain's cobble to the pixel. Smoothness on the layer
        // darkened the road and gave no glint; this is the glint, and the ripple that keeps it
        // alive ("texture wetness วิ่งบนพื้น", user 2026-09-06). Ripple is per-frame, so FX-gated.
        Material _wetSheen;

        void BuildWetSheen(Region r, Texture2D tex, bool wet)
        {
            _wetSheen = null;
            if (!wet) return;
            var sh = Shader.Find("ROI/WetSheen");
            if (sh == null) { Debug.LogWarning("ROI/WetSheen missing"); return; }
            var lk = _tab.look;
            var stale = _fieldRoot.Find("WetSheen");
            if (stale != null) Kill(stale.gameObject);   // every other builder does this; this one did not
            var q = GameObject.CreatePrimitive(PrimitiveType.Quad);
            q.name = "WetSheen";
            q.layer = GroundLayer;
            Kill(q.GetComponent<Collider>());
            q.transform.SetParent(_fieldRoot.Find("Ground"), false);
            q.transform.SetParent(_fieldRoot, true);
            q.transform.localRotation = Quaternion.Euler(90f, 0f, 0f);
            q.transform.localScale = new Vector3(-r.w_cells, r.h_cells, 1f);
            float x = r.strip ? -_gw : 0f;
            q.transform.localPosition = new Vector3(x, 0.02f, FieldZShift + r.z_shift);
            var m = new Material(sh);
            m.SetTexture("_MainTex", tex);
            m.SetTextureScale("_MainTex", new Vector2(1f, -1f));
            m.SetTextureOffset("_MainTex", new Vector2(0f, 1f));
            m.SetFloat("_WetAmount", lk.wet);
            m.SetFloat("_WetRough", lk.wet_rough);
            m.SetFloat("_WetSat", lk.wet_sat);
            // Under half the table's gain: the pack's cobble is far lighter than the HD2D art
            // this glint was tuned over, and the bloom blooms an additive pass. At 1.5 the road
            // went white under the party; 0.7 reads as wet stone (v11 vs v12 shots).
            m.SetFloat("_WetGain", lk.wet_gain * 0.47f);
            m.SetFloat("_WetRipple", FxEnabled ? 0.5f : 0f);   // LookSettings.WET_RIPPLE
            m.SetFloat("_SunPitch", lk.sun_pitch);
            m.SetFloat("_SunYaw", lk.sun_yaw);
            m.SetColor("_SunCol", _sun != null ? _sun.color : Col(lk.sun_color));
            var mr = q.GetComponent<MeshRenderer>();
            mr.sharedMaterial = m;
            mr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            mr.receiveShadows = false;
            _wetSheen = m;
        }

        /// 0 grass, 1 stone, 2 dirt — the one classifier the alphamap and the tint agree on.
        /// Stone is near-zero SATURATION; dirt is what is left where red keeps up with green;
        /// everything else in this palette is grass.
        static int ArtClass(Color c)
        {
            float mx = Mathf.Max(c.r, Mathf.Max(c.g, c.b)), mn = Mathf.Min(c.r, Mathf.Min(c.g, c.b));
            float sat = mx > 0.001f ? (mx - mn) / mx : 0f;
            if (sat < 0.16f) return 1;
            return c.r > c.g * 0.85f ? 2 : 0;
        }

        /// Mean LINEAR colour of the art per class (grass, stone, dirt). Linear because that is
        /// the space the terrain shader scales albedo in; a ratio of sRGB means is the wrong tint.
        static Color[] ArtMeans(Texture2D tex)
        {
            var sum = new Vector3[3]; var n = new int[3];
            var px = tex.GetPixels();
            for (int i = 0; i < px.Length; i += 4)   // every 4th pixel: 1920x1152 is 2.2M reads
            {
                int k = ArtClass(px[i]);
                var l = px[i].linear;
                sum[k] += new Vector3(l.r, l.g, l.b); n[k]++;
            }
            var outc = new Color[3];
            for (int k = 0; k < 3; k++)
                outc[k] = n[k] > 0 ? new Color(sum[k].x / n[k], sum[k].y / n[k], sum[k].z / n[k]) : Color.gray;
            return outc;
        }

        // Layer albedo means, by texture: the pack's textures are not readable, so each is
        // blitted down and read back once and kept.
        static readonly Dictionary<Texture, Color> _layerMean = new();

        static Color LayerMean(Texture src)
        {
            if (_layerMean.TryGetValue(src, out var have)) return have;
            var rt = RenderTexture.GetTemporary(256, 256, 0, RenderTextureFormat.ARGB32, RenderTextureReadWrite.sRGB);
            Graphics.Blit(src, rt);
            var prev = RenderTexture.active; RenderTexture.active = rt;
            var t = new Texture2D(256, 256, TextureFormat.RGBA32, false);
            t.ReadPixels(new Rect(0, 0, 256, 256), 0, 0); t.Apply();
            RenderTexture.active = prev; RenderTexture.ReleaseTemporary(rt);
            var px = t.GetPixels(); Vector3 sum = Vector3.zero;
            foreach (var c in px) { var l = c.linear; sum += new Vector3(l.r, l.g, l.b); }
            Kill(t);
            var m = new Color(sum.x / px.Length, sum.y / px.Length, sum.z / px.Length);
            _layerMean[src] = m;
            return m;
        }

        /// A runtime copy of the pack layer whose mean albedo lands on `target`. URP's TerrainLit
        /// applies diffuseRemapMax - diffuseRemapMin as a per-channel SCALE on the albedo (see
        /// TerrainLitPasses.hlsl, _DiffuseRemapScale), so with min at zero the ratio of means is
        /// the exact remap. Clamped: the pack grass has almost no blue, and a 40x scale on a
        /// near-empty channel amplifies its noise, not its colour.
        static TerrainLayer TintedLayer(TerrainLayer src, Color target, float smoothness = 0f, float darken = 0f, float tile = 0f)
        {
            var l = Instantiate(src);
            l.name = src.name + "_tinted";
            if (tile > 0f) l.tileSize = new Vector2(tile, tile);
            var m = LayerMean(src.diffuseTexture);
            float k = 1f - darken;
            float r = Mathf.Clamp(target.r / Mathf.Max(m.r, 0.002f), 0.15f, 6f) * k;
            float g = Mathf.Clamp(target.g / Mathf.Max(m.g, 0.002f), 0.15f, 6f) * k;
            float b = Mathf.Clamp(target.b / Mathf.Max(m.b, 0.002f), 0.15f, 6f) * k;
            l.diffuseRemapMin = Vector4.zero;
            l.diffuseRemapMax = new Vector4(r, g, b, 1f);
            l.smoothness = smoothness;
            Debug.Log($"layer tint {src.name}: mean {m} -> {target} scale ({r:F2},{g:F2},{b:F2})");
            return l;
        }

        // The land past the region art. The art is a table top: 50x30 cells of authored ground
        // with sky under its far edge and the skybox's floor under its near one. This is the rest
        // of the world — flat under and around the art (a hair lower, so the art stays the
        // surface), rising into rolling hills the further back and further out it goes, never in
        // front of the camera. The pack's grass and dirt, trees and bushes on the slopes, and the
        // haze takes the far edge. Nothing here is authored: it is scenery for the frame, seeded
        // so every rebuild draws the same hills.
        const float SurroundW = 220f, SurroundD = 140f;

        void BuildSurround3D(Vector3 centre, float w, float h, TerrainLayer grassL, TerrainLayer dirtL)
        {
            const int H = 129, A = 128;
            var td = new TerrainData { heightmapResolution = H, size = new Vector3(SurroundW, 14f, SurroundD) };
            td.terrainLayers = new[] { grassL, dirtL };
            var origin = new Vector3(centre.x - SurroundW * 0.5f, centre.y - 0.03f, centre.z - SurroundD * 0.5f);

            // How far a world point is outside the art rect, with a margin so the art's edge is
            // on flat ground; the near side (toward the camera, +z) never counts.
            float Outside(float x, float z)
            {
                float ox = Mathf.Max(0f, Mathf.Abs(x - centre.x) - (w * 0.5f + 3f));
                float oz = Mathf.Max(0f, (centre.z - z) - (h * 0.5f + 3f));
                return Mathf.Sqrt(ox * ox + oz * oz);
            }
            float Hill(float x, float z)
            {
                float d = Outside(x, z);
                if (d <= 0f) return 0f;
                // Over 45 units, not 28: the first cut climbed to its full height within a
                // room's depth of the art and read as a green wall behind the tree line.
                float rise = Mathf.SmoothStep(0f, 1f, Mathf.Clamp01(d / 45f));
                float broad = Mathf.PerlinNoise(x * 0.028f + 7.3f, z * 0.028f + 2.1f);
                float fine = Mathf.PerlinNoise(x * 0.09f + 3.7f, z * 0.09f + 5.9f);
                // A general lift plus the noise: a bowl whose rim is broken by hills, not a
                // flat plain with bumps — the horizon has to sit ABOVE the field plane.
                return rise * (1.5f + broad * 6.5f + fine * 1.2f);
            }
            var heights = new float[H, H];
            for (int zi = 0; zi < H; zi++)
                for (int xi = 0; xi < H; xi++)
                {
                    float wx = origin.x + xi / (float)(H - 1) * SurroundW;
                    float wz = origin.z + zi / (float)(H - 1) * SurroundD;
                    heights[zi, xi] = Mathf.Clamp01(Hill(wx, wz) / td.size.y);
                }
            td.SetHeights(0, 0, heights);

            td.alphamapResolution = A;
            var maps = new float[A, A, 2];
            for (int zi = 0; zi < A; zi++)
                for (int xi = 0; xi < A; xi++)
                {
                    float wx = origin.x + (xi + 0.5f) / A * SurroundW;
                    float wz = origin.z + (zi + 0.5f) / A * SurroundD;
                    float n = Mathf.PerlinNoise(wx * 0.06f + 11.1f, wz * 0.06f + 4.4f);
                    float dirt = Mathf.SmoothStep(0f, 1f, Mathf.Clamp01((n - 0.58f) / 0.12f)) * 0.8f;
                    maps[zi, xi, 0] = 1f - dirt;
                    maps[zi, xi, 1] = dirt;
                }
            td.SetAlphamaps(0, 0, maps);

            var go = Terrain.CreateTerrainGameObject(td);
            go.name = "Surround";
            go.layer = 0;                       // lit by the fill pool too, see BuildTerrain3D
            go.transform.SetParent(_fieldRoot, false);
            go.transform.localPosition = origin;
            var terrain = go.GetComponent<Terrain>();
            terrain.drawInstanced = true;
            terrain.heightmapPixelError = 8f;

            // Trees on the ridge, bushes on the slopes. Kept off the art and off the near side,
            // so nothing planted here can stand in front of the cast.
            var tree = Resources.Load<GameObject>("Dressing3D/Tree");
            var bush = Resources.Load<GameObject>("Dressing3D/Bush");
            var rng = new System.Random(1731);
            int planted = 0;
            void Scatter(GameObject pf, int count, float minOut, float height, float jitter)
            {
                if (pf == null) return;
                for (int tries = 0, done = 0; done < count && tries < count * 20; tries++)
                {
                    float wx = origin.x + 6f + (float)rng.NextDouble() * (SurroundW - 12f);
                    float wz = origin.z + 6f + (float)rng.NextDouble() * (SurroundD - 12f);
                    if (wz > centre.z + h * 0.5f - 2f) continue;          // never in front
                    if (Outside(wx, wz) < minOut) continue;
                    float wy = terrain.SampleHeight(new Vector3(wx, 0f, wz)) + origin.y;
                    var inst = Instantiate(pf, go.transform);
                    inst.transform.position = new Vector3(wx, wy, wz);
                    inst.transform.rotation = Quaternion.Euler(0f, (float)rng.NextDouble() * 360f, 0f);
                    var rs = inst.GetComponentsInChildren<Renderer>();
                    if (rs.Length > 0)
                    {
                        var b = rs[0].bounds; foreach (var rr in rs) b.Encapsulate(rr.bounds);
                        float want = height * Mathf.Lerp(1f - jitter, 1f + jitter, (float)rng.NextDouble());
                        if (b.size.y > 0.01f) inst.transform.localScale = Vector3.one * (want / b.size.y);
                    }
                    TintVegetation(inst);
                    done++; planted++;
                }
            }
            Scatter(tree, 42, 12f, 6.0f, 0.2f);
            Scatter(bush, 60, 4f, 1.3f, 0.3f);
            Debug.Log($"surround {SurroundW}x{SurroundD}, {planted} plants");
        }

        // Full3D grass = the PACK's tufts at the AUTHORED clump positions (same table rows as the
        // blade field, so the layout is still the artist's), instead of the ported blade shader.
        // Trade: the pack's vegetation shader has wind but no walk-through shiver. Full3D only.
        // Flip to compare the pack against the hand-built blades. NOT `const`: as a compile-time
        // constant it makes the other arm unreachable code, and the point of a toggle is to flip it.
        static readonly bool PackGrass = true;
        // The road's stone. Compared live on the baked scene (2026-09-06): Rock at tile 4 shows
        // stones but is a rock BED, not a paved walk ("ไม่ใช่หินแบบทางเท้า"); Dirt_Stone is a sand
        // track; Cobblestone at the pack's tile 2 is the footpath — kept. 0 = the pack's tile.
        const string StoneLayer = "Cobblestone";
        const float StoneTile = 0f;

        void BuildPackGrass(Texture2D groundTex, Vector3 centre, float w, float h)
        {
            var old = _fieldRoot.Find("GrassField");
            if (old != null) Kill(old.gameObject);
            var clumps = _tab.DressingOf(_region)?.grass;
            var blade = _tab.grass_blade;
            if (clumps == null || clumps.Length == 0) return;
            var tufts = new[] { Resources.Load<GameObject>("Dressing3D/Grass1"), Resources.Load<GameObject>("Dressing3D/Grass2"), Resources.Load<GameObject>("Dressing3D/Grass3") };
            if (tufts[0] == null) { Debug.LogWarning("Dressing3D grass missing — run TerrainGrassTest.MakeDressing3DAssets"); return; }
            var root = new GameObject("GrassField").transform;
            root.SetParent(_fieldRoot, false);
            int planted = 0;
            foreach (var g in clumps)
            {
                if (g.pos == null || g.pos.Length < 3) continue;
                var rng = new System.Random(g.layout_seed);
                var origin = new Vector3(-g.pos[0], g.pos[1], g.pos[2]);
                // A tuft is many blades: one per ~6 authored blades keeps the clump's density
                // without a thousand mesh renderers.
                int n = Mathf.Max(2, g.blade_count / 10);   // /6 read as hedges (v20)
                // 1.44 = the authored 0.8 x1.8 (user, 2026-09-06: "1.8 ดีๆ"). The Full3D camera
                // sits far enough back that tufts at the authored size read as scattered dots;
                // this is a taller meadow, chosen against x1.0 / x1.4 / x1.8 captures.
                float height = g.blade_size * BladeTall * 1.44f;
                for (int i = 0; i < n; i++)
                {
                    float rad = g.radius * Mathf.Pow((float)rng.NextDouble(), 0.6f);
                    float ang = (float)rng.NextDouble() * Mathf.PI * 2f;
                    var at = origin + new Vector3(rad * Mathf.Cos(ang), 0f, rad * Mathf.Sin(ang));
                    if (IsPaved(at, groundTex, centre, w, h)) continue;
                    var pf = tufts[rng.Next(tufts.Length)] ?? tufts[0];
                    var inst = Instantiate(pf, root);
                    inst.transform.localPosition = at;
                    inst.transform.localRotation = Quaternion.Euler(0f, (float)rng.NextDouble() * 360f, 0f);
                    ScaleToHeight(inst, height * Mathf.Lerp(0.8f, 1.35f, (float)rng.NextDouble()));
                    TintVegetation(inst, tuft: true);
                    planted++;
                }
            }
            if (Application.isPlaying) _combineQueue.Add(root.gameObject);   // a bake marks them static instead
            Debug.Log($"pack grass: {planted} tufts");
        }

        // The pack's Vegetation shader paints its leaves and blades with a Top/Bottom colour pair
        // ("Custom Color", pack doc) — a dry yellow-green authored for a GAMMA project, which is
        // why every tree, bush and tuft sat off the region art (user: "สีใบไม้กับสีต้นไม้ไม่เข้ากับ
        // พื้นหลังเลย"). Every green vegetation material gets a COPY wearing the game's grass
        // palette instead — tip at the top, root at the base — the same pair the blade field, the
        // HD2D ground and the terrain tint agree on. Non-green (blossom pink) is left alone; it is
        // an accent, not grass. One copy per source material, and no GPU instancing on it:
        // StaticBatchingUtility.Combine batched NOTHING with instancing on (708 batches, 0 static)
        // where the plain material went 1005 -> 148.
        static readonly Dictionary<(Material, bool), Material> _vegTint = new();

        // The tufts' pair, settled in live rounds with the user (2026-09-06), measured on tuft
        // pixels against the ground under them: the palette straight came out darker than the
        // floor (82,119,43 on 112,151,58); a lifted root melted the clumps into it; a warm yellow
        // tip read as olive ("เขียวขี้ม้า"); tuft shadows separated the clumps but cost a shadow
        // pass for 238 renderers. FINAL: no shadows, a DARK root does the separating, and the tip
        // goes light and COOL — green up, blue up, red down (v35).
        // LIVE in the Inspector (ArenaView on the arena object while playing): edit, look, copy
        // the numbers back here. Gains on the palette pair, not colours, so a palette change
        // still carries through.
        [Header("Tuft tint (gains on the grass palette)")]
        public Color TuftTipGain = new(1.08f, 1.36f, 1.12f);
        public float TuftRootGain = 0.85f;
        // Canopies and bushes: the palette straight went near-black on a dense mesh lit from
        // above (user: "พุ่มไม้ + ต้นไม้ยังไม่ปรับสี"). Lifted and a touch warm at the top — sun on
        // leaves — cooler and deeper at the base, so a tree carries the same warm-light /
        // cool-shade split the grade gives the frame. Inspector fields like the tuft pair.
        public Color LeafTipGain = new(1.25f, 1.2f, 0.95f);
        public Color LeafRootGain = new(0.95f, 1.1f, 1.15f);
        // BUSHES on top of that. Diagnosed with a magenta/cyan pair (v47): the colour reaches the
        // shader, but the pack's bush — its Bush_Branch texture times a dense mesh lit from
        // above — comes out ~2.5x darker than the colour asked for, so three "lighter" pairs
        // all read as the same black blob ("พุ่มไม้สีมันโดดขึ้นมา"). Measured: x2.2 puts the bush
        // level with the lawn (161,191,86 on 162,201,87); 1.8 keeps it a shade deeper, as a
        // shrub against grass should be.
        public float BushGain = 1.8f;
        Color _tuftTipApplied; float _tuftRootApplied = -1f;
        readonly List<Material> _tuftMats = new();

        /// Push a changed Inspector value into every tuft material copy. Per frame, but a
        /// compare on two values.
        void ApplyTuftTint()
        {
            if (TuftTipGain == _tuftTipApplied && Mathf.Approximately(TuftRootGain, _tuftRootApplied)) return;
            _tuftTipApplied = TuftTipGain; _tuftRootApplied = TuftRootGain;
            var pal = _tab?.grass_palette;
            if (pal == null || pal.tip == null || pal.root == null) return;
            foreach (var m in _tuftMats)
            {
                if (m == null) continue;
                m.SetColor("_Top_Color", Col(pal.tip) * TuftTipGain);
                m.SetColor("_Bottom_Color", Col(pal.root) * TuftRootGain);
            }
        }

        /// `tuft` = the ground-level grass: the cool light tip / dark root above. Leaves and
        /// bushes take the palette straight. Nothing here casts a shadow the pack did not.
        void TintVegetation(GameObject go, bool tuft = false)
        {
            var pal = _tab.grass_palette;
            if (pal == null || pal.tip == null || pal.root == null) return;
            foreach (var mr in go.GetComponentsInChildren<Renderer>())
            {
                // No shadow either way (user: "ไม่ต้อง cast shadow เพราะกินสเปค แต่ให้โคนเข้มแทน"), and
                // a tuft shading ITSELF read as a black clump, so it receives none.
                if (tuft) { mr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off; mr.receiveShadows = false; }
                var mats = mr.sharedMaterials;
                bool changed = false;
                for (int i = 0; i < mats.Length; i++)
                {
                    var m = mats[i];
                    if (m == null || !m.HasProperty("_Top_Color") || !m.HasProperty("_Custom_Color")) continue;
                    var top = m.GetColor("_Top_Color");
                    if (top.g < top.r * 0.9f || top.g < top.b) continue;   // not a green: leave it
                    if (!_vegTint.TryGetValue((m, tuft), out var c))
                    {
                        c = new Material(m);
                        c.SetFloat("_Custom_Color", 1f);
                        var tip = Col(pal.tip); var root = Col(pal.root);
                        float bush = !tuft && m.name.IndexOf("Bush", System.StringComparison.OrdinalIgnoreCase) >= 0 ? BushGain : 1f;
                        c.SetColor("_Top_Color", tuft ? tip * TuftTipGain : tip * LeafTipGain * bush);
                        c.SetColor("_Bottom_Color", tuft ? root * TuftRootGain : root * LeafRootGain * bush);
                        if (tuft) _tuftMats.Add(c);
                        _vegTint[(m, tuft)] = c;
                    }
                    mats[i] = c; changed = true;
                }
                if (changed) mr.sharedMaterials = mats;
            }
        }

        /// Static-batch scenery roots a FRAME AFTER they are built. Called in the same frame as
        /// the Instantiate (and from a coroutine one frame on), Combine marked nothing static
        /// (238 renderers, 0 batched); the identical call from the Editor loop batched all 238
        /// and took the frame from 1005 batches to ~150. Scenery never moves, so a delay costs
        /// nothing; the result is logged so a silent no-op cannot pass for a win again.
        readonly List<GameObject> _combineQueue = new();
        int _combineQueuedFrame = -1;

        void CombineQueued()
        {
            if (_combineQueue.Count == 0) return;
            if (_combineQueuedFrame < 0) { _combineQueuedFrame = Time.frameCount; return; }
            if (Time.frameCount - _combineQueuedFrame < 2) return;
            foreach (var go in _combineQueue)
            {
                if (go == null) continue;
                StaticBatchingUtility.Combine(go);
                int s = 0, t = 0;
                foreach (var mr in go.GetComponentsInChildren<MeshRenderer>()) { t++; if (mr.isPartOfStaticBatch) s++; }
                Debug.Log($"static batch {go.name}: {s}/{t}");
            }
            _combineQueue.Clear();
            _combineQueuedFrame = -1;
        }

        static void ScaleToHeight(GameObject go, float want)
        {
            var rs = go.GetComponentsInChildren<Renderer>();
            if (rs.Length == 0) return;
            var b = rs[0].bounds; foreach (var rr in rs) b.Encapsulate(rr.bounds);
            if (b.size.y > 0.01f) go.transform.localScale = Vector3.one * (want / b.size.y);
        }

        // The rest of the region art, filled (user: "มันโล่งๆ ไป"): trees, bushes and plants in
        // the ring OUTSIDE the play area, meadow patches out there too, and only small flowers
        // inside it — nothing that could read as an obstacle where the fight is ("อย่าใส่ไอเทมที่ดู
        // obstruct ใน play area"). Off the paved pixels, seeded, scenery not authored data — so it
        // lives under its own root and stays out of the dressing dump.
        void BuildFlora3D(Texture2D groundTex, Vector3 centre, float w, float h)
        {
            var old = _fieldRoot.Find("Flora");
            if (old != null) Kill(old.gameObject);
            var root = new GameObject("Flora").transform;
            root.SetParent(_fieldRoot, false);
            float playX = _gw * 0.5f + 2f, playZ0 = FieldZShift - _gh * 0.5f - 2f, playZ1 = FieldZShift + _gh * 0.5f + 2f;
            bool InPlay(Vector3 p) => Mathf.Abs(p.x) < playX && p.z > playZ0 && p.z < playZ1;
            // Anything TALL on the near side (+z, between the play area and the lens) projects UP
            // into the play area on screen from this camera — a tree there is a tree in front of
            // the fight (v20). Tall things go to the far side or well out to the sides only.
            bool TallOk(Vector3 p) => p.z < playZ0 || (Mathf.Abs(p.x) > playX + 3f && p.z < playZ1);
            var rng = new System.Random(4242);
            var placed = new List<Vector3>();
            int total = 0;
            void Scatter(string[] names, int count, float height, float jitter, bool outsideOnly, float spacing, bool tall = false)
            {
                var pfs = new List<GameObject>();
                foreach (var n in names) { var p = Resources.Load<GameObject>("Dressing3D/" + n); if (p != null) pfs.Add(p); }
                if (pfs.Count == 0) return;
                for (int tries = 0, done = 0; done < count && tries < count * 30; tries++)
                {
                    var at = centre + new Vector3(((float)rng.NextDouble() - 0.5f) * (w - 2f), 0f, ((float)rng.NextDouble() - 0.5f) * (h - 2f));
                    if (outsideOnly && InPlay(at)) continue;
                    if (tall && !TallOk(at)) continue;
                    if (IsPaved(at, groundTex, centre, w, h)) continue;
                    bool near = false;
                    foreach (var q in placed) if ((q - at).sqrMagnitude < spacing * spacing) { near = true; break; }
                    if (near) continue;
                    var inst = Instantiate(pfs[rng.Next(pfs.Count)], root);
                    inst.transform.localPosition = at;
                    inst.transform.localRotation = Quaternion.Euler(0f, (float)rng.NextDouble() * 360f, 0f);
                    ScaleToHeight(inst, height * Mathf.Lerp(1f - jitter, 1f + jitter, (float)rng.NextDouble()));
                    TintVegetation(inst);
                    placed.Add(at); done++; total++;
                }
            }
            Scatter(new[] { "Tree", "Tree2", "Tree3" }, 7, 5.5f, 0.2f, true, 5f, tall: true);
            Scatter(new[] { "Bush", "Bush2" }, 12, 1.3f, 0.3f, true, 2.5f, tall: true);
            Scatter(new[] { "Plant1", "Plant2" }, 10, 0.9f, 0.3f, true, 2f);
            Scatter(new[] { "Meadow1", "Meadow2", "Meadow3" }, 16, 0.5f, 0.3f, true, 2f);
            Scatter(new[] { "Flower1", "Flower2", "Flower3" }, 14, 0.45f, 0.3f, false, 1.5f);
            if (Application.isPlaying) _combineQueue.Add(root.gameObject);
            Debug.Log($"flora: {total} placed");
        }

        void BuildBladeField(Texture2D groundTex, Vector3 centre, float w, float h)
        {
            var blade = _tab.grass_blade;
            var clumps = _tab.DressingOf(_region)?.grass;
            if (blade == null || clumps == null || clumps.Length == 0) return;
            var shape = Sheet(blade.shape);
            if (shape == null) { Debug.LogWarning($"grass shape {blade.shape} missing"); return; }

            var verts = new List<Vector3>();
            var uvs = new List<Vector2>();
            var tris = new List<int>();
            int planted = 0;
            foreach (var g in clumps)
            {
                if (g.pos == null || g.pos.Length < 3) continue;
                var rng = new System.Random(g.layout_seed);
                var origin = new Vector3(-g.pos[0], g.pos[1], g.pos[2]);
                for (int i = 0; i < g.blade_count; i++)
                {
                    float rad = g.radius * Mathf.Pow((float)rng.NextDouble(), 0.6f);
                    float ang = (float)rng.NextDouble() * Mathf.PI * 2f;
                    var at = origin + new Vector3(rad * Mathf.Cos(ang), 0f, rad * Mathf.Sin(ang));
                    if (IsPaved(at, groundTex, centre, w, h)) continue;
                    float sc = g.blade_size * Mathf.Lerp(0.75f, 1.35f, (float)rng.NextDouble());
                    float spin = (float)rng.NextDouble() * Mathf.PI * 2f;
                    AddBlade(verts, uvs, tris, at, sc, spin);
                    planted++;
                }
            }
            if (verts.Count == 0) return;

            var mesh = new Mesh { indexFormat = UnityEngine.Rendering.IndexFormat.UInt32 };
            mesh.SetVertices(verts);
            mesh.SetUVs(0, uvs);
            mesh.SetTriangles(tris, 0);
            mesh.RecalculateBounds();

            var go = new GameObject("GrassField");
            go.layer = GroundLayer;
            go.transform.SetParent(_fieldRoot, false);
            go.AddComponent<MeshFilter>().sharedMesh = mesh;
            var mr = go.AddComponent<MeshRenderer>();
            mr.sharedMaterial = BladeMaterial(shape, blade.size);
            mr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            mr.receiveShadows = false;
            Debug.Log($"grass field: {planted} blades");
        }

        // One blade: a quad standing on its base, facing the camera. The camera does not move, so
        // the billboard is baked into the geometry instead of costing a rotation per vertex per
        // frame — the same trade the props make.
        void AddBlade(List<Vector3> verts, List<Vector2> uvs, List<int> tris, Vector3 at, float size, float spin)
        {
            // FULL width, always. Godot billboards every blade, so each one faces the lens at its
            // whole width; folding the spin into the width instead turned half the field edge-on
            // and the grass read as sparse slivers. The spin varies the width a little, nothing
            // more — enough that the blades are not identical cards.
            var right = _cam.transform.right * (size * 0.5f * Mathf.Lerp(0.8f, 1.2f, Mathf.Abs(Mathf.Cos(spin))));
            // Godot's blade quad is SQUARE (blade_size x blade_size). Taller reads better at
            // this camera pitch — a square blade is mostly hidden behind the one in front of it —
            // so height gets its own multiplier rather than scaling the width with it.
            var up = Vector3.up * (size * BladeTall);
            int b = verts.Count;
            verts.Add(at - right); verts.Add(at + right);
            verts.Add(at - right + up); verts.Add(at + right + up);
            uvs.Add(new Vector2(0, 0)); uvs.Add(new Vector2(1, 0));
            uvs.Add(new Vector2(0, 1)); uvs.Add(new Vector2(1, 1));
            tris.Add(b); tris.Add(b + 2); tris.Add(b + 1);
            tris.Add(b + 1); tris.Add(b + 2); tris.Add(b + 3);
        }

        // Taste, and named as such: the shape and the layout are Godot's, this is not.
        const float BladeTall = 1.9f;
        // 1 / 0.72: the mean of the measured tip/body/root shortfall against the Godot frame,
        // weighted toward the body, which carries most of the blade. See the shader.
        const float BladeExposure = 1.39f;

        Material BladeMaterial(Texture2D shape, float bladeSize)
        {
            // No early-return on the static. Domain reload is disabled for the capture paths, so
            // statics SURVIVE a play session — and a stale one here handed the blades the pack's
            // material, which is why they rendered as solid rectangles with our shader never
            // running at all.
            var sh = Shader.Find("ROI/GrassBlade");
            if (sh == null) { Debug.LogWarning("ROI/GrassBlade missing"); return null; }
            _grassMat = new Material(sh);
            _grassMat.SetTexture("_MainTex", shape);
            Debug.Log($"blade shape {shape.name} {shape.width}x{shape.height} cutoff {_grassMat.GetFloat("_Cutoff")}");
            _grassMat.SetFloat("_BladeHeight", bladeSize * BladeTall);
            _grassMat.SetFloat("_Exposure", BladeExposure);
            TintGrass(_grassMat);
            return _grassMat;
        }

        /// Is the ground ART here paved? Stone reads as near-zero SATURATION where everything
        /// plantable in this palette is green or brown, and saturation (not brightness) is what
        /// tells a sunlit path from a shaded one.
        bool IsPaved(Vector3 at, Texture2D tex, Vector3 centre, float w, float h)
        {
            float u = 0.5f - (at.x - centre.x) / w;
            float v = 0.5f - (at.z - centre.z) / h;
            if (u < 0f || u > 1f || v < 0f || v > 1f) return false;
            var c = tex.GetPixelBilinear(u, v);
            float mx = Mathf.Max(c.r, Mathf.Max(c.g, c.b)), mn = Mathf.Min(c.r, Mathf.Min(c.g, c.b));
            return mx <= 0.001f || (mx - mn) / mx < 0.16f;
        }

        /// The scene mirrors world x and the quad flips V in its material; a terrain layer can do
        /// neither, so the texture itself is flipped once.
        static Texture2D FlipTexture(Texture2D t)
        {
            var px = t.GetPixels();
            var outp = new Color[px.Length];
            for (int y = 0; y < t.height; y++)
                for (int x = 0; x < t.width; x++)
                    outp[y * t.width + x] = px[(t.height - 1 - y) * t.width + (t.width - 1 - x)];
            var f = new Texture2D(t.width, t.height, TextureFormat.RGBA32, false) { filterMode = FilterMode.Point };
            f.SetPixels(outp);
            f.Apply();
            return f;
        }

        // The region's HAND-PLACED props, from the same generated table. Placement is authored in
        // zone_dressing_r<N>.tscn and exported; nothing here decides where anything goes.
        //
        // Sizing is Godot's Sprite3D rule: world size = texture pixels x pixel_size, so the sprite
        // is built at 1/pixel_size pixels-per-unit rather than scaled after the fact. `offset` is
        // in sprite PIXELS and is what puts the node at the prop's FOOT — dropping it stands every
        // tree with its middle on the ground.
        void BuildDressing()
        {
            var old = _fieldRoot.Find("Dressing");
            if (old != null) Kill(old.gameObject);
            var d = _tab.DressingOf(_region);
            if (d == null || d.props == null) return;

            var root = new GameObject("Dressing").transform;
            root.SetParent(_fieldRoot, false);
            // _refresh_dressing: the scene RIDES THE FIELD — props sit on the cells they were
            // placed against no matter how the combat area is shifted for framing. Leaving this
            // at the origin puts every prop two cells off, which reads as "the map is offset".
            root.localPosition = new Vector3(0f, 0f, FieldZShift);
            foreach (var p in d.props)
            {
                if (Style == ZoneStyle.Full3D && Props3D && Spawn3DProp(root, p)) continue;
                var tex = Sheet(p.tex);
                if (tex == null) continue;
                var go = new GameObject(p.name);
                go.transform.SetParent(root, false);
                float ppu = p.pixel_size > 0f ? 1f / p.pixel_size : 100f;
                // PIVOT, not a position offset. Godot's `offset` shifts the quad in sprite space
                // while the node stays at the prop's foot, so the billboard turns about the BASE.
                // Lifting a centre-pivot sprite instead puts the turn axis half-way up the trunk:
                // the moment the quad tilts to face a pitched camera its bottom edge leaves the
                // ground, and the shadow detaches from the trunk.
                float ox = p.offset != null && p.offset.Length > 0 ? p.offset[0] : 0f;
                float oy = p.offset != null && p.offset.Length > 1 ? p.offset[1] : 0f;
                var pivot = new Vector2(0.5f - ox / tex.width, 0.5f - oy / tex.height);
                var spr = Sprite.Create(tex, new Rect(0, 0, tex.width, tex.height),
                                        pivot, ppu, 0, SpriteMeshType.FullRect);
                spr.name = p.name;
                var sr = go.AddComponent<SpriteRenderer>();
                sr.sprite = spr;
                sr.sharedMaterial = ActorMat();
                sr.color = Col(p.tint);
                // cast_shadow 2 = DOUBLE_SIDED in Godot; 0 = off.
                sr.shadowCastingMode = p.cast_shadow == 0
                    ? UnityEngine.Rendering.ShadowCastingMode.Off
                    : UnityEngine.Rendering.ShadowCastingMode.TwoSided;
                sr.receiveShadows = false;
                var pos = P3(p.pos);
                go.transform.localPosition = new Vector3(-pos.x, pos.y, pos.z);
                // Static camera, so the billboard is set once instead of every frame.
                go.transform.rotation = _cam.transform.rotation;
            }
        }

        // GRASS. The Godot field is BinbunGrass — a .gdshader plus MultiMesh clumps that part
        // where a walker passes; none of that crosses to Unity, so the ART is replaced by an
        // Asset Store pack (docs/unity-env-plan.md E4b) and only the LAYOUT is ported: the same
        // 28 clump positions, radii and seeds the Godot scene authored. A clump is scaled to the
        // exported radius by MEASURING the prefab's bounds — a prefab from a different pack has
        // no reason to be the size Godot's clump was, and a hand-picked scale would silently
        // stop matching the moment the prefab changed.

        // The pack's grass wears its own palette (a yellow-olive top, a dark base). The GAME's
        // grass colours are the BinbunGrass gradient, exported beside everything else, so the
        // replacement art can still wear them: lightest stop on the tips, mid on the body, and
        // the base only slightly darker — Godot's `root_shade` sinks the roots, not the whole
        // blade, and copying its full 0.28 onto a mesh with no root gradient just muddies it.
        static Material _grassMat;
        static Color _grassTip, _grassBody, _grassRoot;   // before the hour's tint
        // Every material wearing the grass palette. The terrain A/B renders from its OWN copy
        // (a detail prototype needs an asset), and a copy does not receive the hour — at night
        // the ground went blue and those blades stayed put. Registering it fixes that at the
        // source instead of duplicating the tint code in the Editor tool.
        static readonly List<Material> _grassMats = new();

        /// Register a material that should follow the hour with the grass. Idempotent.
        public void RegisterGrassMaterial(Material m)
        {
            if (m == null || _grassMats.Contains(m)) return;
            _grassMats.Add(m);
            TintGrassForHour(DaylightColor(TimeOfDay));
        }
        void TintGrass(Material mat)
        {
            var pal = _tab.grass_palette;
            if (pal == null || pal.tip == null || pal.body == null || pal.root == null) return;
            if (_grassMat != mat)
            {
                _grassMat = mat;
                // The gradient AS AUTHORED, nothing derived from the ground. Pulling the body
                // toward the floor's average was a fix for the pack's bush mesh, which wore its
                // own colour; these blades run Godot's shader with Godot's layout, and Godot never
                // samples the ground — it uses this gradient, which is what the Godot frame was
                // measured wearing: tips 197,232,107 down to roots 85,119,39, warmer than any
                // green the ground art has. Top = tip stop, Bottom = root stop; the shader picks
                // between them per blade by noise and sinks the base itself.
                Color tip = Col(pal.tip), body = Col(pal.body), root = Col(pal.root);
                _grassTip = tip; _grassBody = body; _grassRoot = root;
                Debug.Log($"grass tint tip={tip} body={body} root={root}");
                ApplyGrassWind();
                TintGrassForHour(DaylightColor(TimeOfDay));
            }
        }

        /// Mean ground colour at the authored clump centres, or null when there are none.
        Color? GroundColorUnderClumps(Texture2D tex)
        {
            var d = _tab.DressingOf(_region);
            if (d?.grass == null || d.grass.Length == 0) return null;
            var r = _tab.RegionOf(_region);
            var ground = _fieldRoot.Find("Ground");
            if (r == null || ground == null) return null;
            float rr = 0f, gg = 0f, bb = 0f; int n = 0;
            foreach (var g in d.grass)
            {
                if (g.pos == null || g.pos.Length < 3) continue;
                float u = 0.5f - (-g.pos[0] - ground.localPosition.x) / r.w_cells;
                float v = 0.5f - (g.pos[2] - ground.localPosition.z) / r.h_cells;
                if (u < 0f || u > 1f || v < 0f || v > 1f) continue;
                var c = tex.GetPixelBilinear(u, v);
                rr += c.r; gg += c.g; bb += c.b; n++;
            }
            return n > 0 ? new Color(rr / n, gg / n, bb / n, 1f) : (Color?)null;
        }

        /// Mean colour of a texture, subsampled — it is a tint reference, not a histogram.
        static Color AverageColor(Texture2D t)
        {
            var px = t.GetPixels32();
            if (px.Length == 0) return Color.white;
            long r = 0, g = 0, b = 0; int n = 0;
            for (int i = 0; i < px.Length; i += 37) { r += px[i].r; g += px[i].g; b += px[i].b; n++; }
            return new Color(r / (255f * n), g / (255f * n), b / (255f * n), 1f);
        }

        static float Luma(Color c) => 0.2126f * c.r + 0.7152f * c.g + 0.0722f * c.b;

        // WIND. Only the DIRECTION crosses over.
        //
        // Rate and strength do NOT: Godot scrolls a noise texture (`wind_speed` 3.0 over
        // `wind_scale` 0.15) where the pack drives a vertex bend (`_Wind_Speed` 0.14,
        // `_Wind_Strength` 0.25). Copying the numbers across put the grass at 21x the speed and
        // 5x the throw, which does not read as a stronger breeze — it reads as the blades
        // vibrating. The pack's values were authored FOR this mesh, so they stay; the aim comes
        // from the Godot material so the gusts blow the same way in both engines.
        //
        // (`_WindSpeed`/`_WindStrength` without the separator are a different shader's leftovers
        // on the same material. Writing both spellings is how the wrong one got driven.)
        void ApplyGrassWind()
        {
            var w = _tab.grass_wind;
            if (w == null || _grassMat == null) return;
            var v = w.velocity != null && w.velocity.Length > 1
                ? new Vector2(w.velocity[0], w.velocity[1]) : Vector2.right;
            v = v.normalized;
            SetIfHas(_grassMat, new Color(v.x, v.y, 0f, 0f), "_Wind_Direction");
            // Our own shader takes the Godot values directly — same units, because it is the same
            // shader — where the pack's needed only the direction.
            if (_grassMat.HasProperty("_WindDir"))
            {
                _grassMat.SetVector("_WindDir", new Vector4(w.velocity[0], w.velocity[1], 0f, 0f));
                _grassMat.SetFloat("_WindSpeed", w.speed);
                _grassMat.SetFloat("_WindScale", w.scale);
                _grassMat.SetFloat("_PushStrength", w.push_strength);

            }
            // The walker PUSH: the pack bends away from a world position, which is the same idea
            // as BinbunGrass's per-clump `set_push`. One position, not one per clump — the pack
            // has a single Player Position, so the LEAD member is the one the grass parts for.
            SetFloatIfHas(_grassMat, w.push_strength, "_Bend_Strength");
        }

        // The pack's grass runs its OWN lighting (Shader/CustomLighting.hlsl), so it dims with
        // the sun but never takes the ambient's colour: at night the ground went blue and the
        // blades only went dark. The hour is therefore applied to the grass COLOURS directly.
        //
        // Normalised by luma, so only the HUE shift is applied — the darkening already happens
        // through the light, and multiplying the tint in raw would dim it a second time.
        void TintGrassForHour(Color c)
        {
            float l = Mathf.Max(Luma(c), 0.01f);
            // HUE from the tint, and only PART of its darkness. The pack's shader takes the sun
            // but ignores ambient, so it already dims — just not enough: measured noon -> night,
            // the ground fell to 0.58 of its daylight luma and the blades only to 0.73. Applying
            // the tint raw would dim them twice (0.36) and applying none leaves them glowing at
            // dusk. `HourDim` closes exactly that measured gap: lerp(1, luma, 0.41) = 0.79 at
            // night, and 0.79 x 0.73 = 0.58, which is where the ground is.
            const float HourDim = 0.41f;
            float dim = Mathf.Lerp(1f, l, HourDim);
            Color hue = new Color(c.r / l * dim, c.g / l * dim, c.b / l * dim, 1f);
            if (_grassMat != null && !_grassMats.Contains(_grassMat)) _grassMats.Add(_grassMat);
            foreach (var m in _grassMats)
            {
                if (m == null) continue;
                // The pack's own documentation settles which properties matter: "Custom Color —
                // Activates the Top and Bottom Color, OTHERWISE THE TEXTURE COLOR IS TAKEN". So
                // the graph reads Top/Bottom and nothing else; `_BaseColor`/`_Color` are leftovers
                // of the Lit shader sitting on the same material, and writing them (which is what
                // was happening) tinted nothing at all.
                SetFloatIfHas(m, 1f, "_Custom_Color", "_CustomColor");
                SetFloatIfHas(m, 1f, "_Enable_Top_Color");
                SetIfHas(m, _grassTip * hue, "_Top_Color", "_TopColor");
                SetFloatIfHas(m, _tab.grass_palette.root_shade, "_RootShade");
                SetFloatIfHas(m, _tab.grass_palette.root_sat, "_RootSat");
                SetIfHas(m, _grassRoot * hue, "_Bottom_Color", "_BottomColor");
                var gp = _tab.grass_palette;
                if (gp != null && gp.stop0 != null && m.HasProperty("_Stop0"))
                {
                    m.SetColor("_Stop0", Col(gp.stop0) * hue); m.SetFloat("_Off0", gp.off0);
                    m.SetColor("_Stop1", Col(gp.stop1) * hue); m.SetFloat("_Off1", gp.off1);
                    m.SetColor("_Stop2", Col(gp.stop2) * hue); m.SetFloat("_Off2", gp.off2);
                }
            }
        }

        static void SetFloatIfHas(Material m, float f, params string[] names)
        {
            foreach (var n in names) if (m.HasProperty(n)) m.SetFloat(n, f);
        }

        // Everyone standing in the grass, not just the lead. Godot gives each CLUMP its own push
        // centre; the equivalent here is a list the shader walks, so a party spread across the
        // field disturbs it in several places at once instead of one.
        static readonly Vector4[] _pushPoints = new Vector4[8];
        /// A push centre injected from outside the sim (Editor tooling), so the shiver can be
        /// tested on a known tuft without waiting for the party to walk into one. w <= 0 = off.
        public Vector4 DebugPush;
        void PushGrass()
        {
            // No HasProperty gate on the array: `_PushPoints` is declared in HLSL only (an array
            // cannot live in the Properties block), and HasProperty answers FALSE for it — which
            // silently returned here every frame. The push never reached the shader at all; the
            // one "proof" of it was gust noise. This is the whole "grass does not move when
            // someone walks in".
            if (_grassMat == null) return;
            int n = 0;
            if (DebugPush.w > 0f) _pushPoints[n++] = DebugPush;
            var w = _tab.grass_wind;
            float gain = w != null ? w.push_gain : 9f, decay = w != null ? w.push_decay : 2.5f;
            var clumps = _tab.DressingOf(_region)?.grass;
            foreach (var a in _actors.Values)
            {
                if (!a.Alive) continue;
                var p = a.Root.transform.position;
                // grass_clump.set_push, per walker instead of per clump: energy is EARNED by
                // moving while inside a clump's react radius (distance x gain, capped at 1) and
                // bleeds away at decay/s. Standing still earns nothing; so does walking up to
                // the edge — which is what made the old always-on push read as triggering before
                // the walker had touched anything.
                if (!a.PushInit) { a.PushLast = p; a.PushInit = true; }
                bool inside = false;
                if (clumps != null)
                    foreach (var g in clumps)
                    {
                        if (g.pos == null || g.pos.Length < 3) continue;
                        float dx = p.x - (-g.pos[0]), dz = p.z - g.pos[2];
                        if (dx * dx + dz * dz < g.react_radius * g.react_radius) { inside = true; _pushRadius = g.react_radius; break; }
                    }
                if (inside) a.PushEnergy = Mathf.Min(a.PushEnergy + Vector3.Distance(a.PushLast, p) * gain, 1f);
                a.PushLast = p;
                a.PushEnergy = Mathf.Max(a.PushEnergy - Time.deltaTime * decay, 0f);
                if (a.PushEnergy <= 0f || n >= _pushPoints.Length) continue;
                // xz = walker, y = energy, w = radius — the packing Godot's per-instance push uses.
                _pushPoints[n++] = new Vector4(p.x, a.PushEnergy, p.z, _pushRadius);
            }
            for (int i = n; i < _pushPoints.Length; i++) _pushPoints[i] = Vector4.zero;
            _grassMat.SetVectorArray("_PushPoints", _pushPoints);
            _grassMat.SetInt("_PushCount", n);
        }
        float _pushRadius = 1.6f;

        static void SetIfHas(Material m, Color c, params string[] names)
        {
            foreach (var n in names) if (m.HasProperty(n)) m.SetColor(n, c);
        }


        // OFF (user 2026-09-06: "พุ่มไม้สีมันโดดขึ้นมา"): the pack's bush mesh in the PLAY AREA read
        // as a black blob against the lit floor — three colour pairs and no received shadow did
        // not move it; a dense mesh lit from above is dark by geometry. The authored props go
        // back to their SPRITES, the way the HD-2D references keep the foreground pixel art and
        // put the 3D in the background. The pack's trees and bushes stay on the surround and in
        // the flora ring, outside the fight.
        const bool Props3D = true;   // kept on for now: the user wants the colour fixed, not the mesh swapped

        // A prop's 3D twin from the pack, by what its sprite is called. No twin (the barrel) ->
        // the billboard stays, which is also how a mixed HD-2D scene tends to look anyway.
        bool Spawn3DProp(Transform root, Prop p)
        {
            string key = p.tex.ToLowerInvariant();
            string name = key.Contains("tree") ? "Tree" : key.Contains("bush") ? "Bush" : (key.Contains("rock") || key.Contains("stone")) ? "Rock" : null;
            if (name == null) return false;
            var pf = Resources.Load<GameObject>("Dressing3D/" + name);
            if (pf == null) return false;
            var go = Instantiate(pf, root);
            go.name = p.name + "_3D";
            var pos = P3(p.pos);
            go.transform.localPosition = new Vector3(-pos.x, pos.y, pos.z);
            go.transform.localRotation = Quaternion.Euler(0f, (p.name.GetHashCode() & 0x7fffffff) % 360, 0f);
            // Match the SPRITE's height, so the 3D tree stands where the painted one did and the
            // layout the artist made keeps its proportions.
            var tex = Sheet(p.tex);
            if (tex != null)
            {
                float spriteH = tex.height * p.pixel_size;
                var rs = go.GetComponentsInChildren<Renderer>();
                if (rs.Length > 0)
                {
                    var b = rs[0].bounds; foreach (var rr in rs) b.Encapsulate(rr.bounds);
                    if (b.size.y > 0.01f) go.transform.localScale = Vector3.one * (spriteH / b.size.y);
                }
            }
            TintVegetation(go);
            StripIdleParticles(go);
            return true;
        }

        // The pack's tree and bush prefabs each carry a LeafParticleSystem that emits NOTHING in
        // this scene — 73 of them across the zone, every one costing a system update and a
        // culling entry per frame for no pixels (measured: ~0.9 ms). Anything that would actually
        // emit is left alone.
        static void StripIdleParticles(GameObject go)
        {
            foreach (var ps in go.GetComponentsInChildren<ParticleSystem>(true))
            {
                var em = ps.emission;
                if (!em.enabled) { Destroy(ps.gameObject); continue; }
                if (em.rateOverTime.constantMax <= 0f && em.rateOverDistance.constantMax <= 0f && em.burstCount == 0)
                    Destroy(ps.gameObject);
            }
        }

        static Vector3 P3(float[] v) =>
            v != null && v.Length >= 3 ? new Vector3(v[0], v[1], v[2]) : Vector3.zero;

        /// The BAND from "zone_b<band>_<room>", or 0 when the id is not that shape.
        static int BandOfZone(string zoneId)
        {
            if (string.IsNullOrEmpty(zoneId)) return 0;
            int i = zoneId.IndexOf("_b", System.StringComparison.Ordinal);
            if (i < 0) return 0;
            int j = i + 2, band = 0;
            while (j < zoneId.Length && char.IsDigit(zoneId[j])) { band = band * 10 + (zoneId[j] - '0'); j++; }
            return band;
        }

        /// The ROOM from "zone_b<band>_<room>", or 0. Needed on its own because a band's rooms
        /// scroll and a band change does not — telling those apart is the whole decision.
        static int RoomOfZone(string zoneId)
        {
            if (string.IsNullOrEmpty(zoneId)) return 0;
            int i = zoneId.LastIndexOf('_');
            if (i < 0 || i + 1 >= zoneId.Length) return 0;
            int room = 0;
            for (int j = i + 1; j < zoneId.Length; j++)
            {
                if (!char.IsDigit(zoneId[j])) return 0;
                room = room * 10 + (zoneId[j] - '0');
            }
            return room;
        }

        /// band_of_zone + OverlayView.region_of, from "zone_b<band>_<room>".
        int RegionOfZone(string zoneId)
        {
            int band = BandOfZone(zoneId);
            if (band < 1) return 0;
            return (band - 1) / Mathf.Max(1, _tab.field.zones_per_region) + 1;
        }

        // Presentation only: later areas temporarily reuse the authored Area 1–5 strip.
        // Keep the real zone id for UI, enemies, progression and room/band scrolling.
        // Resolve before the rebuild decision so no old terrain or dressing is created.
        int EnvironmentRegionOfZone(string zoneId)
        {
            int region = RegionOfZone(zoneId);
            return UseEarlyAreaMapPlaceholder && EnvBaked && BakedRegion == 1 && region > 1
                ? BakedRegion : region;
        }

        /// THE MARCH, decided the way `arena3d.gd` decides it.
        ///
        /// Rooms scroll only when the party moves FORWARD one room inside one band. A jump back to
        /// room 1 of the same band — the farm loop re-entering, a restart after a wipe — would
        /// otherwise keep extending the scroll and walk the party off the end of the authored art.
        /// Godot learned that one the hard way and says so in a comment; it is copied here as a
        /// rule, not as an accident of the arithmetic.
        void OnZoneChanged(string zoneNow)
        {
            int band = BandOfZone(zoneNow);
            int room = RoomOfZone(zoneNow);
            // SEEDED, not left at -1. Without this the session's FIRST room change falls into the
            // "unknown band" branch and swaps instead of scrolling — a fade where a march belongs.
            if (_scrollBand < 0)
            {
                // THE FIRST ROOM STILL HAS AN ENTRANCE. This branch seeded the scroll state and
                // returned, so the session opened with the party already standing in formation
                // while nothing happened — Godot walks them in here ("Room 1 enters from
                // off-screen (WALK_IN_CELLS)", arena3d.gd) and that was simply never ported.
                _scrollBand = band; _scrollRoom = room;
                return;   // the entrance is triggered by `opened_at_tick`, which covers this too
            }

            bool forward = band == _scrollBand && room > 0 && room == _scrollRoom + 1;
            _scrollBand = band;
            _scrollRoom = room;
            if (forward)
            {
                // Forward is -x here; see RoomSpacing.
                _scrollX -= RoomSpacing;
                // The camera's duration is set with the party's, below, once the walk is measured —
                // it FOLLOWS them rather than running an ease of its own, or the two disagree and
                // the party arrives to a camera still travelling (or worse, already parked).
                _marchPending = true;
            }
            else
            {
                // A new band swaps behind ZoneFade's dip to black, so there is no travel to show —
                // but the party still ENTERS the new room rather than being found already in it.
                // Godot's split exactly: within the scroll they march from where they stand, across
                // a swap they come in off the left edge.
                _scrollX = 0f;
                StartPan(0f, 0f);
            }
        }

        /// One ease per march, sine in-out, matching the single tween Godot starts per room change.
        /// A duration of 0 snaps, which is what a band swap behind the fade wants.
        void StartPan(float to, float dur)
        {
            _panFrom = CurrentPan();
            _panTo = to;
            _panStart = Time.time;
            _panDur = dur;
        }

        /// LINEAR, and matching the party exactly — not the sine ease Godot's camera tween uses.
        ///
        /// The actors glide on a plain `Vector3.Lerp`, so a camera easing in and out over the same
        /// duration travels at a different speed than the thing it is following: it falls behind at
        /// the start, overtakes in the middle and settles back at the end. Godot gets away with the
        /// mismatch because its march is about a second; over a ten-second crossing it reads as the
        /// camera sliding around the party (user 2026-09-09: "ตอนเคลื่อนกล้องยังไม่ smooth").
        ///
        /// Matching the party matters more than an eased start. "The camera follows the characters"
        /// is only true if it moves at their speed, and they are walking at a constant one.
        /// EVERYTHING A BREAK LOOKS LIKE, in one place so the test path and the real one cannot
        /// drift apart. A trash break is the first three lines; a boss adds the beat.
        void PlayBreak(Actor bt, long id, long amount, bool boss)
        {
            if (bt.Root == null) return;
            var centre = bt.Root.transform.position + Vector3.up * (bt.HeadY * 0.5f);
            // The guard shatters, the word announces it, and the number goes back to being a
            // number — the label it used to carry is the banner's job now.
            if (_shards != null) _shards.Burst(centre);
            if (_breakFlash != null) _breakFlash.Burst(bt.Body.bounds.center, bt.HeadY * (boss ? 1f : .75f), _cam, bt.Body.transform);
            if (_hud != null)
                _hud.SpawnBreakBanner(id, bt.Root.transform.position + Vector3.up * (bt.HeadY * 0.7f),
                                      Roi.UI.Loc.Get("UI_ARENA_BREAK_WORD"), amount, boss);
            // A break is a bigger event than the swing that caused it, so it gets a longer flash
            // than a hit rather than the same one twice.
            bt.FlashT = FlashDur * 1.8f;
            if (boss) FocusOnBreak(centre, bt.HeadY);
        }

#if UNITY_EDITOR
        /// Fire a break presentation on the nearest living monster, so it can be looked at without
        /// first reaching a boss. `boss` picks the full beat over the small one.
        public bool DebugBreak(bool boss, bool largest = false)
        {
            Actor best = null; long bestId = 0; float bestD = float.MaxValue;
            var eye = _cam != null ? _cam.transform.position : Vector3.zero;
            foreach (var entry in _actors)
            {
                var a = entry.Value;
                if (a.IsPlayer || !a.Alive || a.Root == null) continue;
                float distance = (a.Root.transform.position - eye).sqrMagnitude;
                float d = largest ? -a.HeadY * 1000f + distance * .001f : distance;
                if (d < bestD) { bestD = d; best = a; bestId = entry.Key; }
            }
            if (best == null) return false;
            PlayBreak(best, bestId, 999, boss);
            return true;
        }
#endif

        /// Start the beat on a boss break at `at`.
        void FocusOnBreak(Vector3 at, float targetHeight)
        {
            if (_cam == null) return;
            // A SECOND BREAK MID-BEAT MUST NOT INHERIT THE FIRST ONE'S CAMERA. The base fov and the
            // solved shift are what the beat unwinds back to; capturing them while the previous
            // beat still had the camera zoomed and slid made the "original" a moved camera, and it
            // never came back. End the running one first, then read a clean camera.
            if (_focusAge >= 0f) EndFocus();
            _focusAge = 0f;
            _focusFovBase = _cam.fieldOfView;
            _focusShift = SolveCentre(at);
            _focusTarget = at;
            // WHICH WAY IS DOWN. Rather than reason out the sign of a rotation about an axis that
            // is itself yawed 180 degrees, rotate a probe both ways and keep the one that lowers
            // the camera. The intent is "come down to the boss", so let the intent be the test.
            var axis = _camBaseRot * Vector3.right;
            var arm = new Vector3(_camBaseX + CurrentPan(), _camBaseY, _camBaseZ) - at;
            _focusPitchDir = (Quaternion.AngleAxis(10f, axis) * arm).y < arm.y ? 1f : -1f;
            _sunBase = _sun != null ? _sun.intensity : 0f;
            _fillBase = _fill != null ? _fill.intensity : 0f;
            _breakAmbientBase = RenderSettings.ambientLight;
            _breakFillColourBase = Shader.GetGlobalVector("_FillColor");
            _breakEnvironmentLights.Clear(); _breakEnvironmentEnergy.Clear();
            foreach (var light in GetComponentsInChildren<Light>())
                if (light.enabled && (light.name == "CanopyBounce" || light.name == "Reference_SunBreak"))
                { _breakEnvironmentLights.Add(light); _breakEnvironmentEnergy.Add(light.intensity); }
            if (_focusKey == null)
            {
                var go = new GameObject("BreakKey");
                go.transform.SetParent(transform, false);
                _focusKey = go.AddComponent<Light>();
                _focusKey.type = LightType.Spot;
                _focusKey.spotAngle = 82f;
                _focusKey.innerSpotAngle = 48f;
                _focusKey.color = new Color(0.72f, 0.88f, 1f);
                _focusKey.shadows = LightShadows.Hard;
                _focusKey.shadowStrength = 1f;
                _focusKey.shadowBias = 0.02f;
                _focusKey.shadowNormalBias = 0.02f;
            }
            // Camera-side key: the silhouette casts away from the viewer.
            var front = Vector3.ProjectOnPlane(_camBaseRot * Vector3.back, Vector3.up).normalized;
            // Keep the key just above the silhouette: a high key makes a short, unreadable shadow.
            _focusKey.transform.position = at + front * Mathf.Max(2.4f, targetHeight * 1.4f)
                + (_camBaseRot * Vector3.right) * .7f + Vector3.up * Mathf.Max(.5f, targetHeight * .5f);
            _focusKey.transform.LookAt(at);
            _focusKey.range = FocusKeyRange;
            _focusKey.enabled = true;
            if (_shards != null) _shards.SetSpeed(FocusShardSpeed);
            if (_driver != null) _driver.Paused = true;
            DriveFocus();
        }

        /// How far the camera must move for `at` to sit dead centre.
        ///
        /// A point is centred when it lies on the view axis, so the answer is just its offset from
        /// that axis: project the target onto the camera's own right and up and slide by exactly
        /// that. No fov, no screen size, no `WorldToScreenPoint` — the first version went out
        /// through pixels and back, which made the result depend on the Game view's size and on
        /// whatever the camera happened to be doing at the time.
        ///
        /// Solved against the camera's UNPANNED base rather than its live transform, so a beat
        /// starting while another is unwinding still measures from the same place.
        Vector3 SolveCentre(Vector3 at)
        {
            var rot = _camBaseRot;
            var basePos = new Vector3(_camBaseX + CurrentPan(), _camBaseY, _camBaseZ);
            var rel = at - basePos;
            if (Vector3.Dot(rel, rot * Vector3.forward) <= 0.01f) return Vector3.zero;   // behind us
            return (rot * Vector3.right) * Vector3.Dot(rel, rot * Vector3.right)
                 + (rot * Vector3.up) * Vector3.Dot(rel, rot * Vector3.up);
        }

        /// Advance the beat and let the sim go again once the hold is over. UNSCALED, and it must
        /// stay that way: the beat is presentation, and tying it to sim time would make it a thing
        /// the pause it owns can never end.
        void TickFocus()
        {
            if (_focusAge < 0f) return;
            _focusAge += Time.unscaledDeltaTime;
#if !ROI_DOTWEEN
            _focusE = FocusCurve.Evaluate(_focusAge / Mathf.Max(FocusDur, 0.01f));
#endif
            if (_driver != null && _focusAge >= FocusPauseDur) _driver.Paused = false;
            if (_focusAge >= FocusDur) EndFocus();
        }

        /// Put back everything the beat borrowed. Idempotent, and the ONLY place that undoes it,
        /// so an interrupted beat and a finished one leave the scene in the same state.
        void EndFocus()
        {
            _focusAge = -1f;
            _focusE = 0f;
#if ROI_DOTWEEN
            _focusSeq?.Kill();
            _focusSeq = null;
#endif
            _focusShift = Vector3.zero;
            if (_cam != null && _focusFovBase > 0f) _cam.fieldOfView = _focusFovBase;
            if (_cam != null) _cam.transform.rotation = _camBaseRot;
            if (_focusKey != null) _focusKey.enabled = false;
            Shader.SetGlobalVector("_BreakKeyColor", Vector4.zero);
            for (int i = 0; i < _breakEnvironmentLights.Count; i++)
                if (_breakEnvironmentLights[i] != null) _breakEnvironmentLights[i].intensity = _breakEnvironmentEnergy[i];
            _breakEnvironmentLights.Clear(); _breakEnvironmentEnergy.Clear();
            if (_shards != null) _shards.SetSpeed(1f);
            if (_driver != null) _driver.Paused = false;
            // ApplyDaylight owns the lights; asking it to re-run is how they go back to the values
            // the zone actually wants, rather than to whatever this beat happened to cache.
            ApplyDaylight(true);
        }

        float FocusEnvelope() => _focusAge < 0f ? 0f : _focusE;

        float CurrentPan()
        {
            if (_panDur <= 0f) return _panTo;
            float u = Mathf.Clamp01((Time.time - _panStart) / _panDur);
            return Mathf.Lerp(_panFrom, _panTo, u);
        }

#if UNITY_EDITOR
        /// WHERE THE FIGHT ACTUALLY HAPPENS, drawn in the Scene view so a room can be dressed
        /// AROUND it instead of near it.
        ///
        /// The grid is 20x10 cells and a room of authored art is 52 m wide, so most of what is on
        /// screen is scenery and only the middle strip is walkable. Nothing said which strip. A prop
        /// standing a metre inside the line is a prop the party walks through; a landmark centred on
        /// the art rather than on the field sits off to one side in play, and both mistakes are
        /// invisible until someone plays it and only obvious once. This is the cheap fix: three
        /// boxes, one per room, at exactly the offsets the march will use.
        ///
        /// Drawn from the SAME `RoomSpacing` the march steps by, so a change to the spacing moves
        /// the boxes too — the gizmo cannot quietly disagree with the code it is describing.
        void OnDrawGizmos()
        {
            // Cell space is mirrored on x (see CellToWorld); this is that rectangle in world units.
            var size = new Vector3(_gw, 0.02f, _gh);
            for (int room = 0; room < RoomsPerBand; room++)
            {
                var centre = new Vector3(-RoomSpacing * room, 0f, FieldZShift);
                // Room 1 solid, the rooms ahead progressively fainter — the band reads left to
                // right in the order the party will walk it.
                float a = room == 0 ? 0.9f : 0.45f - room * 0.1f;
                Gizmos.color = new Color(0.35f, 0.85f, 1f, a);
                Gizmos.DrawWireCube(centre, size);
                // The centre line, which is what a landmark should be centred on.
                Gizmos.color = new Color(1f, 0.85f, 0.3f, a * 0.7f);
                Gizmos.DrawLine(centre + new Vector3(0f, 0f, -_gh * 0.5f),
                                centre + new Vector3(0f, 0f, _gh * 0.5f));
                UnityEditor.Handles.color = Gizmos.color;
                UnityEditor.Handles.Label(centre + new Vector3(0f, 0.5f, _gh * 0.5f),
                                          $"room {room + 1} — combat {_gw}x{_gh}");
            }
        }
#endif

        Vector3 CellToWorld(float cx, float cy) =>
            new Vector3(-(cx - _gw * 0.5f + 0.5f) + _scrollX, 0f, cy - _gh * 0.5f + 0.5f + FieldZShift);

        static float DisplayPx(Tables t, float px) => px * t.cells_per_art_px * t.sprite_scale * t.actor_scale;

        // --- per-tick view ingest -------------------------------------------------------------
        void OnView(string json)
        {
            if (DeferForAegis(json)) return;
            // The overlay reads the SAME payload, and it reads it first: it has to draw a boss
            // that just spawned on the tick the arena is about to place it.
            if (_overlay != null) _overlay.OnView(json);
            if (_clear != null) _clear.OnView(json);
            if (_death != null) _death.OnView(json);
            if (_sound != null) _sound.OnView(json);
            if (_cmdr != null) _cmdr.OnView(json);
            if (_fade != null) _fade.OnView(json);
            if (_shots != null) _shots.OnView(json, _tickDur);
            var cs = JsonUtility.FromJson<CmdrView>(json);
            _skills = cs != null && cs.commander_skills != null ? cs.commander_skills
                                                                : new Roi.UI.CommanderBar.Skill[0];
            var v = JsonUtility.FromJson<ViewDTO>(json);
            if (v == null || v.grid == null) return;
            PrepareSeraphineView(v);
            if (v.grid.width > 0) _gw = v.grid.width;
            if (v.grid.height > 0) _gh = v.grid.height;
            int region = EnvironmentRegionOfZone(v.current_zone_id);
            if (v.current_zone_id != _zoneSeen)
            {
                _zoneSeen = v.current_zone_id;
                OnZoneChanged(v.current_zone_id);
            }
            // A ROOM OPENED. `OnZoneChanged` has already claimed this one if it is a forward march
            // within the scroll — that walk continues from where the party stands and must not be
            // replaced by an entrance. Everything else enters from off the left edge, which now
            // includes a RETRY: it reopens the same zone id, so nothing here used to notice it and
            // the party was found already standing in formation for the whole deploy grace.
            if (v.zone_spawn != null && v.zone_spawn.deploy_grace_ticks > 0)
                _graceSeconds = v.zone_spawn.deploy_grace_ticks * SimDriver.TickInterval;
            long opened = v.zone_spawn != null ? v.zone_spawn.opened_at_tick : 0;
            if (opened != _openedAtSeen)
            {
                _openedAtSeen = opened;
                if (!_marchPending) _walkInPending = true;
            }
            var ground = _fieldRoot.Find("Ground");
            // THE AUTHORED ART BELONGS TO ONE REGION. The ground is built for every region, baked
            // one included — `AdoptBakedEnvironment` calls `RebuildGround` itself, and the authored
            // rooms are scenery standing on it, not a second map. What was missing is that they
            // stayed lit after the party left: band 6's terrain came up underneath region 1's three
            // rooms, and that IS two maps at once.
            // The baked region draws ITSELF and must never be rebuilt — that is what a fresh start
            // does, and matching it is the whole fix. Its arm of the `ground == null` test is off
            // as well, or a region that never builds a "Ground" would retry every frame.
            bool baked = EnvBaked && region == BakedRegion;
            if (region != _region || (!baked && ground == null))
            {
                bool moved = region != _region;
                _region = region;
                // A CACHED HANDLE REACHES PAST THE WALL: `BuildAtmosphere` kills `_atmos` rather
                // than looking it up, so parking saves the authored Atmosphere from `Find` and not
                // from that. Letting go has to happen BEFORE the builder runs — dropping the handle
                // afterwards is dropping it after the object it named is already gone.
                if (!baked) { _baked.Show(false); _atmos = null; }
                // Only on an actual MOVE: the `ground == null` arm is a first build of the region
                // already current, and there is nothing stale to drop there.
                if (moved) ResetVegetationCache();
                if (baked)
                {
                    _baked.KillBuilt(Kill);
                    _baked.Show(true);
                    _atmos = _fieldRoot != null ? _fieldRoot.Find("Atmosphere") : null;
                }
                else RebuildGround();
            }

            _tick = v.tick;
            _tickTime = Time.time;
            _seen.Clear();
            _knockback.Clear();
            if (v.knockbacks != null) foreach (var id in v.knockbacks) _knockback.Add(id);
            _entX.Clear();
            var ents = v.grid.entities;
            if (ents != null) foreach (var e in ents) _entX[e.id] = e.pos.x;
            if (ents != null) foreach (var e in ents) { _seen.Add(e.id); Ingest(e); }

            // THE MARCH ITSELF. Ingest only starts a glide when the sim's CELL changes, and a room
            // change does not move anybody's cell — the field moved under them. Without this the
            // party is simply left standing in the room behind while the world slides away.
            //
            // One duration for everyone, taken from the FURTHEST walk (Godot's `maxf`), so they
            // cross in formation and arrive together instead of stringing out by speed.
            // THE ENTRANCE. Off the left edge and into formation, at the same pace as the march.
            // Separate from `_marchPending` because it is a different move: the march continues a
            // walk already in progress from the room behind, this one starts off-screen.
            if (_walkInPending)
            {
                _walkInPending = false;
                float now = Time.time;
                float far = 0f;
                foreach (var a in _actors.Values)
                {
                    if (!a.IsPlayer) continue;
                    a.GlideFrom = CellToWorld(-WalkInCells, a.CellY);
                    a.GlideTo = CellToWorld(a.CellX, a.CellY);
                    far = Mathf.Max(far, Vector3.Distance(a.GlideFrom, a.GlideTo));
                }
                // Land `MarchSettleSeconds` before the sim resumes combat (0.0.398: the open grace
                // is sized to this walk plus that beat, so this is the pace, not a cap).
                float dur = Mathf.Clamp(far / MarchSpeedCps, 0.3f, Mathf.Max(0.3f, _graceSeconds - MarchSettleSeconds));
                foreach (var a in _actors.Values)
                {
                    if (!a.IsPlayer) continue;
                    a.GlideT0 = now; a.GlideDur = dur; a.Gliding = true;
                    a.Facing = a.GlideTo.x < a.GlideFrom.x ? 1 : -1;
                    a.FacingFlipAt = now;
                    a.MarchUntil = now + dur;
                }
                _zoneOpenedAt = now; _firstHitLogged = false;
                Debug.Log($"ARENA walk-in: {far:0.#} units over {dur:0.##}s at {MarchSpeedCps} cells/s");
            }

            if (_marchPending)
            {
                _marchPending = false;
                float now = Time.time;
                float far = 0f;
                foreach (var a in _actors.Values)
                {
                    if (!a.IsPlayer || !a.Init) continue;
                    a.GlideFrom = CurrentGlidePos(a, now);
                    a.GlideTo = CellToWorld(a.CellX, a.CellY);
                    far = Mathf.Max(far, Vector3.Distance(a.GlideFrom, a.GlideTo));
                }
                // CAPPED TO THE DEPLOY GRACE, because the sim resumes combat when the grace ends
                // and a party still sliding when the first monster swings is the thing the user
                // rejected in Godot on 2026-09-03 ("the fight started mid-stride").
                //
                // In Godot the cap never bites: its rooms are one field width apart and the walk
                // lands around 1.2 s. Here it bites hard, and the log says so rather than quietly
                // running the party at a speed nobody chose — a 52 m room crossed inside a 2 s
                // grace is not a walk, and pretending otherwise is how a number nobody decided ends
                // up shipping.
                // THE SIM SETS THE PACE (0.0.398): `deploy_grace_ticks` is the walk for THIS
                // party's move speed plus a settle beat, so the march takes exactly the grace
                // minus that beat and arrives under half a second before the first swing — a
                // Move line is a visibly faster crossing. `MarchSpeedCps` is only the fallback
                // pace when the sim has not said.
                float dur = _graceSeconds > 0f
                    ? Mathf.Max(0.3f, _graceSeconds - MarchSettleSeconds)
                    : Mathf.Max(0.3f, far / MarchSpeedCps);
                // FACE THE WAY THEY ARE WALKING. `UpdateFacing` reads a CELL change and a room
                // change does not move anybody's cell, so without this the party crosses the whole
                // room still facing whatever it last swung at — backwards, most of the time.
                // Higher cell x is screen right (see UpdateFacing), and the field slides -x, so the
                // party travels screen-right: +1.
                foreach (var a in _actors.Values)
                {
                    if (!a.IsPlayer || !a.Init) continue;
                    a.GlideT0 = now; a.GlideDur = dur; a.Gliding = true;
                    a.Facing = a.GlideTo.x < a.GlideFrom.x ? 1 : -1;
                    a.FacingFlipAt = now;
                    a.MarchUntil = now + dur;
                }
                // The camera spans exactly the party's walk, which is what "the camera follows the
                // characters" has to mean when both are moving on the same axis.
                StartPan(_scrollX, dur);
                Debug.Log($"ARENA march: {far:0.#} units over {dur:0.##}s at {far / dur:0.#} cells/s");
            }

            FeedHud(v);

            // A preview can change before resolve; an actual skill cancels the basic-only edge.
            if (v.skill_fx != null)
                foreach (var fx in v.skill_fx)
                    if (_actors.TryGetValue(fx.caster_id, out var caster)) caster.BasicSwordSwing = false;

            if (v.recent_hits != null)
                foreach (var h in v.recent_hits)
                    if (h.amount > 0 && _actors.TryGetValue(h.target_id, out var ta))
                    {
                        if (!_firstHitLogged && _zoneOpenedAt >= 0f)
                        {
                            _firstHitLogged = true;
                            Debug.Log($"ARENA dead air: first hit {Time.time - _zoneOpenedAt:0.##}s "
                                    + "after the room opened");
                        }
                        ta.HitstopUntil = Time.time + HitstopDur;
                        if (ta.IsPlayer) { ta.HurtUntil = Time.time + PlayerHurtFlashDur; continue; }
                        // base_monster_3d: the hit juice — an overbright flash that REPLACES the
                        // colour, and a squash-stretch pulse — runs only for hit_juice defs. The
                        // generic slime ships hit_juice = false: it plays its hurt clip and does
                        // not change colour at all. A red tint on every monster hit was the port's
                        // own invention and is exactly what read as wrong.
                        // The round glow rides the same hit as the flash, at the sprite's middle,
                        // and the debris is thrown along the line of the blow — attacker to target.
                        if (_sparks != null)
                        {
                            var away = _actors.TryGetValue(h.attacker_id, out var atkr)
                                     ? ta.Root.transform.position - atkr.Root.transform.position
                                     : Vector3.zero;
                            _sparks.Spawn(ta.Root.transform.position + Vector3.up * (ta.HeadY * 0.5f), away);
                        }
                        // The overbright flash is Unity's, not a port: the r1 slimes carry
                        // hit_juice = false, so Godot answers a hit with the hurt clip alone and
                        // its world light. Unity has no such light on a sprite, so the flash is
                        // what stands in for it and runs on every hit. The SQUASH is the part
                        // that really is hit_juice, and it stays off for the defs that say so.
                        ta.FlashT = FlashDur;
                        if (_tab.monster != null && _tab.monster.hit_juice)
                            ta.SquashT = _tab.globals.monster_squash_dur;
                    }

            FeedSeraphineSkills(v);

            var stale = new List<long>();
            foreach (var kv in _actors) if (!_seen.Contains(kv.Key)) stale.Add(kv.Key);
            foreach (var id in stale) { Kill(_actors[id].Root); _actors.Remove(id); }
        }

        void Ingest(EntityDTO e)
        {
            if (!_actors.TryGetValue(e.id, out var a)) { a = BuildActor(e.id); _actors[e.id] = a; }
            float now = Time.time;
            bool alive = e.alive && e.hp > 0;
            int cx = (int)e.pos.x, cy = (int)e.pos.y;

            a.IsPlayer = e.team == "Player";
            a.IsBoss = e.is_boss;
            a.ClassId = e.unit_def; a.Sprite = e.sprite; a.Element = e.element;
            a.PendingSkill = e.pending_skill;
            a.MotionLock = e.motion_lock;
            if (e.skills != null)
            {
                long latest = 0;
                a.ResolvedSkill = null;
                foreach (var skill in e.skills)
                    if (skill.last_fired > latest)
                    { latest = skill.last_fired; a.ResolvedSkill = skill.id; }
            }
            if (a.MotionLock > 0)
            {
                a.BasicSwordSwing = false; a.StrikeLocked = false;
                a.FollowUntil = 0; a.StrikeArmed = true;
            }
            if (a.ClassId == "seraphine" && a.SwordEdge == null)
                a.SwordEdge = SeraphineBasicSlash.Create(a.Body);
            if (e.casting != null) a.BasicSwordSwing = false;
            a.Toughness = e.toughness; a.ToughnessMax = e.toughness_max;
            a.TargetId = e.target_id;
            a.AtkRange = (int)e.atk_range; a.ProjTravel = (int)e.proj_travel_ticks;
            a.EffAspd = e.eff_atk_speed_milli > 0 ? e.eff_atk_speed_milli : (e.atk_speed_milli > 0 ? e.atk_speed_milli : 100);
            a.AtkSpeed = e.atk_speed_milli > 0 ? e.atk_speed_milli : 100;
            a.SpeedMilli = e.speed_milli > 0 ? e.speed_milli : 1000;
            a.MaxHp = e.eff_max_hp > 0 ? e.eff_max_hp : e.max_hp;
            _entX.TryGetValue(e.target_id, out var tgx);
            a.TargetX = Mathf.RoundToInt(tgx);

            // glide: start a lerp when the cell changes
            var toWorld = CellToWorld(cx, cy);
            if (!a.Init) { a.GlideFrom = a.GlideTo = toWorld; a.Gliding = false; a.PrevCellX = cx; }
            else if (cx != a.CellX || cy != a.CellY)
            {
                a.GlideFrom = CurrentGlidePos(a, now);
                a.GlideTo = toWorld; a.GlideT0 = now; a.Gliding = true;
                // A SHOVE, not a step: fixed short duration and an arc. Same split arena3d.gd makes
                // — a speed-based flat slide reads as walking, and a knockback must not.
                a.Bounce = _knockback.Contains(e.id);
                a.GlideDur = a.Bounce ? KnockbackDur : StepDuration(a.SpeedMilli);
            }
            a.PrevCellX = a.CellX; a.CellX = cx; a.CellY = cy;

            long atk = e.atk_acc, prev = a.PrevAtkAcc;
            bool wrapped = a.Init && atk < prev;
            bool gotHit = a.Init && e.hp < a.Hp;

            if (a.IsPlayer)
            {
                // strike release on the damage wrap -> follow-through
                // The swing WINDOW (st["attack_until"]): refreshed while a swing is actually in
                // progress, so a member whose accumulator stalls (no target, motion lock) falls back
                // to idle instead of standing in a wind-up pose forever.
                if (alive && a.MotionLock == 0 && (atk > 0 || wrapped))
                    a.AttackUntil = now + 100f / Mathf.Max(1f, e.atk_speed_milli);
                if (wrapped && a.StrikeLocked)
                {
                    a.StrikeLocked = false;
                    var c = _tab.Class(a.ClassId);
                    if (c != null && c.atk_strike_end > c.atk_impact)
                    {
                        a.FollowT0 = now;
                        a.FollowUntil = now + FollowDur(a, c);
                    }
                }
                else if (wrapped && a.MotionLock == 0 && a.AnimName == "attack")
                {
                    // MISSED STRIKE (user 2026-09-27: the basic attack was cut). The strike latches
                    // only on a frame at `atk_lunge_from`, a window of about one frame before the
                    // wrap; at an editor frame rate the wrap often lands first, and the wind-up
                    // snapped to idle with no hit. Play the hit and its follow-through instead.
                    // A swing a skill took has a motion lock and is left to the skill's pose.
                    var c = _tab.Class(a.ClassId);
                    if (c != null && a.PoseRow == c.atk_row && c.atk_strike_end > c.atk_impact)
                    {
                        a.StrikeArmed = false;
                        a.BasicSwordSwing = string.IsNullOrEmpty(a.PendingSkill) && a.CastFrac < 0f;
                        a.LungeDir = LungeDirTo(a);
                        a.FollowT0 = now;
                        a.FollowUntil = now + FollowDur(a, c);
                    }
                }
            }
            else
            {
                // monster: re-arm the one-swing latch on the hit; trigger the clip when the
                // contact frame is `windup` seconds away (base_monster_3d.apply).
                if (wrapped) a.MonLatch = false;
                var m = _tab.GeomFor(a.Sprite);
                if (alive && m.atk_frames != null && m.atk_frames.Length > 0)
                {
                    float fps = Mathf.Max(1f, m.atk_fps);
                    float windup = Mathf.Min(m.atk_impact / fps, 100f / Mathf.Max(1f, a.AtkSpeed));
                    float timeToHit = (1000f - atk) / Mathf.Max(1f, a.AtkSpeed) * _tickDur;
                    bool hasTarget = e.target_id != 0;
                    if (hasTarget && !a.MonLatch && timeToHit <= windup)
                    {
                        a.MonLatch = true; a.MonPlaying = true; a.MonAtkT0 = now;
                        // The swing REPLACES a hurt in progress, as `sprite.animation = "attack"`
                        // does in _drive_anim — the hurt is forgotten, not paused. Left pending,
                        // its last frame surfaced for one tick after the swing ended: three of
                        // the four remaining hurt disagreements in the dense sweep were that.
                        a.MonDmgPlaying = false;
                    }
                }
                // ORDER matters here and it is Godot's: the hit is judged against the attack clip
                // BEFORE that clip's own finish is honoured (_drive_anim runs before
                // _on_anim_finished on the same tick). Clearing MonPlaying in the previous
                // LateUpdate let a hit that landed as the swing ended play a one-tick hurt
                // that Godot drops — four times in a 260-tick sweep, and each read as a flicker.
                if (gotHit && alive && !a.MonPlaying && m.damaged_frames != null && m.damaged_frames.Length > 0)
                { a.MonDmgPlaying = true; a.MonDmgT0 = now; }
                if (a.MonAtkOver) { a.MonPlaying = false; a.MonAtkOver = false; }
            }

            // The collapsed per-tick damage number comes from the HP DIFF, exactly as the Godot
            // 3D path does — read it before the snapshot below overwrites the old value. Whether
            // it is actually drawn is decided in FeedHud, after the multi-hit lists have claimed
            // their targets.
            if (a.Init && e.hp < a.Hp) _pendingDamage.Add((e.id, a.Hp - e.hp));
            a.PrevAtkAcc = atk; a.AtkAcc = atk; a.Hp = e.hp; a.Alive = alive; a.Init = true;

            // The HP bar is DRAWN BY THE OVERLAY now (ArenaHud), projected from the head like
            // arena3d_draw does — the world-space quad that used to live here was the port's own
            // invention and never shrank with distance or carried a cast bar.
            a.MonName = e.mon_name;
            a.CastFrac = e.casting != null && e.casting.ticks_total > 0
                ? Mathf.Clamp01(1f - (float)e.casting.ticks_left / e.casting.ticks_total)
                : -1f;
        }

        // ORDER MATTERS, and it is arena3d_fx's: dodge/guard text first (they are never a
        // number), then the multi-hit grouping claims its targets, then damage FIELDS, and only
        // then the collapsed hp-delta fills in whatever is left. Reversed, a Sonic Blow would
        // show its eight numbers AND a ninth for the same damage.
        readonly HashSet<long> _multiTargets = new();
        readonly List<(long id, long amount)> _pendingDamage = new();

        void FeedHud(ViewDTO v)
        {
            // Clear the queue even with no overlay, or a HUD that failed to build turns into an
            // unbounded list of damage nobody will ever draw.
            if (_hud == null && (_companion == null || !_companion.Visible)) { _pendingDamage.Clear(); return; }
            _multiTargets.Clear();

            if (v.recent_hits != null)
            {
                foreach (var h in v.recent_hits)
                {
                    // BREAK — its own packet from the sim, so its own number, and NOT folded into
                    // the swing that caused it. Ported from `arena3d_fx.gd`: amber, labelled, and
                    // floated higher than the damage number so the two do not stack on each other.
                    //
                    // Without this it fell through to the ordinary number path and rendered as a
                    // second, identical, unexplained hit — which is exactly how it was reported
                    // (user 2026-09-09: "ตอน swordman โจมตีมี damage เพิ่มมาอีก 1 instance บางครั้ง
                    // คืออะไร"). The sim had labelled it all along; the client threw the label away.
                    if (h.kind == "break")
                    {
                        if (!_actors.TryGetValue(h.target_id, out var bt)) continue;
                        PlayBreak(bt, h.target_id, h.amount, bt.IsBoss);
                        continue;
                    }
                    if (h.kind != "dodge" && h.kind != "guard") continue;
                    if (!_actors.TryGetValue(h.target_id, out var t)) continue;
                    var at = t.Root.transform.position + Vector3.up * (t.HeadY * 0.7f);
                    // A MONSTER dodging a player's hit reads as the attacker's MISS (red, RO
                    // style); a PLAYER dodging a monster's hit stays "Dodged" (blue).
                    if (h.kind == "guard") _hud.SpawnText(h.target_id, at, "Guarded", ArenaHud.GuardColor);
                    else if (t.IsPlayer) _hud.SpawnText(h.target_id, at, "Dodged", ArenaHud.DodgeColor);
                    else _hud.SpawnText(h.target_id, at, "Miss", ArenaHud.MissColor);
                }
                FeedMultiHits(v);
            }

            if (v.field_hits != null)
                foreach (var f in v.field_hits)
                {
                    if (f.amount <= 0 || !_actors.TryGetValue(f.target_id, out var t)) continue;
                    var at = t.Root.transform.position + Vector3.up * (t.HeadY * 0.7f);
                    int n = Mathf.Max(1, f.hits);
                    if (n > 1)
                    {
                        // A discrete multi-hit STRIKE (one Meteor Rain meteor): N staggered
                        // numbers at a readable cadence, not a rapid blur.
                        int per = (int)(f.amount / (float)n);
                        var key = $"fieldstrike:{f.target_id}:{_tick}";
                        for (int k = 0; k < n; k++)
                        {
                            _hud.FeedMultiHit(key, f.target_id, at, per, n, k * 0.13f);
                            _companion?.SpawnDamage(f.target_id, at, per, t.IsPlayer, k * 0.13f);
                        }
                        if (!t.Alive) t.DeathHoldUntil = Time.time + (n - 1) * 0.13f + 0.05f;
                    }
                    else
                    {
                        // A per-interval tick (Vermilion / Glacial) grows ONE running total.
                        _hud.FeedMultiHit($"field:{f.target_id}", f.target_id, at, (int)f.amount, 0);
                        _companion?.SpawnDamage(f.target_id, at, (int)f.amount, t.IsPlayer);
                    }
                    _multiTargets.Add(f.target_id);
                }

            // The collapsed hp-delta, for everything the lists above did not already show.
            foreach (var (id, amount) in _pendingDamage)
            {
                if (_multiTargets.Contains(id)) continue;
                if (!_actors.TryGetValue(id, out var t)) continue;
                var at = t.Root.transform.position + Vector3.up * (t.HeadY * 0.7f);
                _hud.SpawnDamage(id, at, amount, t.IsPlayer);
                _companion?.SpawnDamage(id, at, amount, t.IsPlayer);
            }
            _pendingDamage.Clear();

            if (v.heal_fx != null)
                foreach (var h in v.heal_fx)
                {
                    if (h.amount <= 0 || !_actors.TryGetValue(h.target_id, out var t)) continue;
                    _hud.SpawnHeal(h.target_id, t.Root.transform.position + Vector3.up * (t.HeadY * 0.7f), h.amount);
                }

            if (v.loot_fx != null)
                foreach (var it in v.loot_fx)
                {
                    // The name is the item's own. Godot derives a richer one through ItemNaming
                    // (material + noun + affix, from the id and the stats) — that derivation is
                    // not ported, so this is the plain name until it is.
                    var nm = !string.IsNullOrEmpty(it.name) ? it.name : it.def_id;
                    // Gear is coloured by GRADE (0.0.380): the toast takes a colour, not a word.
                    bool gear = it.kind == "Weapon" || it.kind == "Armor";
                    if (gear && it.grade_step > 0) nm = Roi.UI.GameText.GradeName(it.grade_step) + " " + nm;
                    if (!string.IsNullOrEmpty(nm))
                        _hud.PushLoot(nm, Mathf.Max(1, it.quantity), it.rarity,
                                      gear ? Roi.UI.GameText.GradeColor(it.grade_step) * 0.62f : (Color?)null);
                }

            if (v.skill_fx != null)
                foreach (var f in v.skill_fx)
                {
                    if (string.IsNullOrEmpty(f.skill_id)) continue;
                    // Godot puts the banner on a rising edge of the skill's own `last_fired`;
                    // `skill_fx` IS that edge, already resolved by the sim, so it is read here
                    // instead of re-deriving it from the skill list.
                    _hud.SpawnSkillLabel(f.caster_id, SkillLabelText(f.skill_id));
                }
        }

        void FeedMultiHits(ViewDTO v)
        {
            var byTarget = new Dictionary<long, List<HitDTO>>();
            foreach (var h in v.recent_hits)
            {
                // `break` too: it already drew its own labelled number above, and counting it here
                // is what made one swing look like two hits.
                if (h.kind == "dodge" || h.kind == "guard" || h.kind == "break") continue;
                if (!byTarget.TryGetValue(h.target_id, out var list))
                    byTarget[h.target_id] = list = new List<HitDTO>();
                list.Add(h);
            }
            foreach (var kv in byTarget)
            {
                int totalNumbers = 0;
                foreach (var e in kv.Value) totalNumbers += Mathf.Max(1, e.hits);
                if (totalNumbers <= 1) continue;          // one number: the hp-delta path has it
                if (!_actors.TryGetValue(kv.Key, out var t)) continue;
                var at = t.Root.transform.position + Vector3.up * (t.HeadY * 0.7f);
                var key = $"multi:{kv.Key}:{_tick}";
                float run = 0f, lastAt = 0f;
                foreach (var e in kv.Value)
                {
                    int n = Mathf.Max(1, e.hits);
                    int per = (int)(e.amount / (float)n);
                    // The gaps come from the ATTACKER's swing, so a fast weapon's numbers arrive
                    // fast — the rhythm matches what the sprite is doing.
                    long aspd = _actors.TryGetValue(e.attacker_id, out var atk) ? atk.EffAspd : 0;
                    ArenaHud.SwingGaps(aspd, out float within, out float between);
                    for (int k = 0; k < n; k++)
                    {
                        _hud.FeedMultiHit(key, kv.Key, at, per, n, run);
                        _companion?.SpawnDamage(kv.Key, at, per, t.IsPlayer, run);
                        lastAt = run;
                        run += within;
                    }
                    run += between;
                }
                _multiTargets.Add(kv.Key);
                // A target dying NEVER truncates its group: hold the death clip until the last
                // number has shown (number_fx's rule, via arena._death_hold).
                if (!t.Alive) t.DeathHoldUntil = Time.time + lastAt + 0.05f;
            }
        }

        /// "sonic_blow" -> "Sonic Blow !!". Godot resolves the display name through Loc; there is
        /// no localisation layer on this side yet, so the id is titled until there is.
        static string SkillLabelText(string skillId)
        {
            var parts = skillId.Split('_');
            for (int i = 0; i < parts.Length; i++)
                if (parts[i].Length > 0) parts[i] = char.ToUpperInvariant(parts[i][0]) + parts[i].Substring(1);
            return string.Join(" ", parts) + " !!";
        }

        Vector3 CurrentGlidePos(Actor a, float now)
        {
            if (!a.Gliding) return a.GlideTo;
            float k = a.GlideDur > 0 ? Mathf.Clamp01((now - a.GlideT0) / a.GlideDur) : 1f;
            var pos = Vector3.Lerp(a.GlideFrom, a.GlideTo, k);
            // Knockback hop: a parabola that lifts +Y and lands (sin is 0 at both ends, so the
            // start and the destination stay flat). arena3d_scene converts KNOCKBACK_HOP px to
            // world cells with the universal pixel scale; the same product the lunge uses.
            if (a.Bounce)
                pos.y += KnockbackHopPx * _tab.cells_per_art_px * _tab.sprite_scale * Mathf.Sin(k * Mathf.PI);
            return pos;
        }

        // --- per-render-frame animation -------------------------------------------------------
        void LateUpdate()
        {
            if (TickAegisPresentation()) return;
            // THE CAMERA AND THE GROUND LIGHT, together. Godot snaps the fill pool at the swap and
            // it popped the ground lighting a full room mid-march, so the light glides in parallel
            // with the pan rather than after it.
            //
            // The TERRAIN does not move, which is where this parts company with Godot: there the
            // band art is one strip the renderer slides, here region 1 is authored terrain whose
            // three rooms already stand at 0, -52 and -104. Moving it would drag the art away from
            // the party it was placed around. Only the fight grid, the camera and the light travel.
            TickFocus();
            if (_cam != null)
            {
                float pan = CurrentPan();
                // The push is FOV plus a lateral lean, never a change of height or pitch: those two
                // are what the whole HD-2D framing is built on, and moving them re-frames the zone
                // rather than the boss.
                float e = FocusEnvelope();
                var shift = Vector3.zero;
                if (Mathf.Abs(e) > 0.001f)
                {
                    shift = _focusShift * e;
                    _cam.fieldOfView = Mathf.Lerp(_focusFovBase, _focusFovBase * FocusZoom, e);
                    // DIM THE WORLD, KEEP THE BOSS. The scene lights come down and a key light
                    // sitting on the target comes up, so the drop in light reads as attention
                    // rather than as the zone going dark.
                    float lit = Mathf.Clamp01(e);      // the lights do not overshoot; that reads as a flicker
                    float k = Mathf.Lerp(1f, FocusDim, lit);
                    if (_sun != null) _sun.intensity = _sunBase * k;
                    if (_fill != null) _fill.intensity = _fillBase * k;
                    RenderSettings.ambientLight = _breakAmbientBase * Mathf.Lerp(1f, .08f, lit);
                    Shader.SetGlobalVector("_FillColor", _breakFillColourBase * k);
                    for (int i = 0; i < _breakEnvironmentLights.Count; i++)
                        if (_breakEnvironmentLights[i] != null) _breakEnvironmentLights[i].intensity = _breakEnvironmentEnergy[i] * k;
                    if (_focusKey != null)
                    {
                        _focusKey.intensity = FocusKeyEnergy * lit;
                        var kp = _focusKey.transform.position;
                        Shader.SetGlobalVector("_BreakKeyPosition", new Vector4(kp.x, kp.y, kp.z, FocusKeyRange));
                        Shader.SetGlobalVector("_BreakKeyColor", (Vector4)(_focusKey.color * (_focusKey.intensity * .32f)));
                    }
                }
                // The centred position first, then the arc around the target from there. Both are
                // rebuilt from the base every frame — at e = 0 the yaw is 0 and this collapses back
                // to exactly the authored camera, which is what makes the beat leave no residue.
                var pos = new Vector3(_camBaseX + pan, _camBaseY, _camBaseZ) + shift;
                if (e > 0f)
                {
                    var swing = Quaternion.AngleAxis(FocusPitch * _focusPitchDir * e,
                                                     _camBaseRot * Vector3.right);
                    pos = _focusTarget + swing * (pos - _focusTarget);
                    _cam.transform.rotation = swing * _camBaseRot;   // turn WITH the arc, or the target leaves frame
                }
                _cam.transform.position = pos;
                if (_fill != null)
                {
                    var fp = _fill.transform.position;
                    _fill.transform.position = new Vector3(_fillBaseX + pan, fp.y, fp.z);
                }
            }
            TickAim();
            if (_cam == null || _tab == null) return;
            float now = Time.time, dt = Time.deltaTime;
            float tickFrac = Mathf.Clamp01((now - _tickTime) / _tickDur);
            var camRot = _cam.transform.rotation;

            foreach (var a in _actors.Values)
            {
                a.Billboard.rotation = camRot;
                a.Gliding = a.Gliding && (now - a.GlideT0) < a.GlideDur;
                UpdateFacing(a, now);
                if (a.IsPlayer) AnimatePlayer(a, now, dt, tickFrac);
                else AnimateMonster(a, now, dt);

                Color tint = a.Alive ? Color.white : new Color(1f, 1f, 1f, DeathAlpha);
                if (a.IsPlayer && a.Alive && now < a.HurtUntil)
                {
                    float k = (a.HurtUntil - now) / PlayerHurtFlashDur;   // fades out
                    tint = Color.Lerp(Color.white, new Color(1f, 0.55f, 0.55f, 1f), k);
                }
                a.Body.color = tint;
                TickJuice(a, dt);
            }
            TickSeraphineSkills(dt);
            PushBars();
            PushHoverName();
            ApplyDaylight();
            ApplyAtmosphere();
            ApplyCloud(dt);
            ApplyTuftTint();
            CombineQueued();
            PushGrass();
            DumpIfDue();
        }

        // One bar per LIVING actor. Dropped the instant HP hits 0 — the sprite lingers to play
        // its death clip, but an empty bar hanging over a corpse reads as a bug.
        void PushBars()
        {
            if (_hud == null || !_hud.isActiveAndEnabled || _cam == null) return;
            var right = _cam.transform.right * (0.7f * 0.5f);   // catalog hp_bar_width, in cells
            foreach (var a in _actors.Values)
            {
                if (!a.Alive || a.MaxHp <= 0 || a.Hp <= 0 || a.Root == null) continue;
                // Up the CAMERA's axis, not the world's: a full billboard aligns its vertical
                // axis to the camera, so an actor near the screen edge has its head leaning off
                // world-up and a world-up bar drifts off-centre there.
                // The walk moves the bar, the attack lunge does not.
                var root = a.Root.transform;
                var lunge = root.parent != null ? root.parent.TransformVector(new Vector3(a.LungeX, 0f, 0f))
                                                : new Vector3(a.LungeX, 0f, 0f);
                var head = root.position - lunge + _cam.transform.up * a.BarHeadY;
                float brk = a.ToughnessMax > 0
                          ? Mathf.Clamp01((float)a.Toughness / a.ToughnessMax) : -1f;
                _hud.PushBar(head, head + right, Mathf.Clamp01((float)a.Hp / a.MaxHp), a.CastFrac, brk);
            }
        }

        // The monster under the cursor. Measured on the FLOOR in cells (0.7 of a cell, the 2D
        // rule) rather than in screen pixels, so the reach does not change with camera distance.
        /// Alive for ANIMATION purposes: a corpse whose numbers are still landing keeps standing.
        static bool AnimAlive(Actor a) => a.Alive || Time.time < a.DeathHoldUntil;

        void PushHoverName()
        {
            if (_hud == null || !_hud.isActiveAndEnabled || _cam == null) return;
            var mouse = UnityEngine.InputSystem.Mouse.current;
            if (mouse == null) return;
            var mp = mouse.position.ReadValue();
            var ray = _cam.ScreenPointToRay(mp);
            if (!new Plane(Vector3.up, Vector3.zero).Raycast(ray, out float enter)) return;
            var ground = ray.GetPoint(enter);
            string best = null; float bestD = 0.7f; Vector3 bestAt = default;
            foreach (var a in _actors.Values)
            {
                if (a.IsPlayer || !a.Alive || string.IsNullOrEmpty(a.MonName) || a.Root == null) continue;
                var at = a.Root.transform.position;
                float d = Vector2.Distance(new Vector2(at.x, at.z), new Vector2(ground.x, ground.z));
                if (d >= bestD) continue;
                bestD = d; best = a.MonName; bestAt = at;
            }
            if (best != null) _hud.PushHoverName(best, bestAt);
        }

        // base_monster_3d._tick_flash + _tick_squash. Flash: lerp from white toward FLASH_TINT
        // by the time left, handed to the shader's REPLACE path (an overbright multiply cannot
        // survive the 8-bit vertex colour — the Godot shader header says why). Squash: one sin
        // pulse, wider then taller then settle, on the sprite child so the footprint stays put.
        void TickJuice(Actor a, float dt)
        {
            var g = _tab.globals;
            if (a.FlashT > 0f) a.FlashT -= dt;
            ActorHitFlash.Apply(a.Body,ref a.Mpb,a.FlashT);

            float sq = 1f, sqY = 1f;
            if (a.SquashT > 0f)
            {
                float elapsed = g.monster_squash_dur - a.SquashT;
                a.SquashT -= dt;
                float pp = Mathf.Sin(elapsed / Mathf.Max(g.monster_squash_dur, 1e-3f) * Mathf.PI * 2f);
                sq = 1f + g.monster_squash_amp * pp;
                sqY = 1f - g.monster_squash_amp * pp;
            }
            var ls = a.Body.transform.localScale;
            float h = Mathf.Abs(ls.y) > 1e-4f ? Mathf.Abs(ls.y) : Mathf.Abs(ls.x);   // Place set uniform h
            a.Body.transform.localScale = new Vector3(h * sq, h * sqY, ls.z == 0 ? 1f : ls.z);
        }

        // --- cross-engine numeric dump (DEV ONLY, env-gated exactly like the Godot side) -------
        // ARENA_DUMP_TICKS="40,80,160" ARENA_DUMP_OUT=/tmp/unity_arena.csv
        // Emitted in CELL space so it can be diffed against godot_arena.csv row for row; the two
        // engines disagree on axis sign + camera yaw by design, and world coords would report that
        // as a difference on every row while hiding the real ones.
        readonly List<int> _dumpTicks = new();
        readonly HashSet<int> _dumpDone = new();
        readonly List<string> _dumpRows = new();
        string _dumpOut;
        bool _dumpArmed;

        /// Set by Editor tooling before play starts, so the dump can run from an OPEN Editor —
        /// batchmode cannot attach to a project the Editor holds, and env vars are read once.
        public static string DumpTicksOverride, DumpOutOverride;

        void ArmDump()
        {
            // Three sources, in order: a file the Editor tool drops next to the project (survives
            // anything — statics did not reliably reach Start across a play restart), the static
            // override, then the env vars batchmode passes.
            string ticks = null;
            var req = System.IO.Path.Combine(System.IO.Directory.GetCurrentDirectory(), ".roi_dump");
            if (System.IO.File.Exists(req))
            {
                var parts = System.IO.File.ReadAllText(req).Trim().Split('\n');
                ticks = parts.Length > 0 ? parts[0].Trim() : null;
                _dumpOut = parts.Length > 1 ? parts[1].Trim() : null;
                System.IO.File.Delete(req);   // one dump per request
            }
            ticks ??= DumpTicksOverride ?? System.Environment.GetEnvironmentVariable("ARENA_DUMP_TICKS");
            _dumpOut ??= DumpOutOverride ?? System.Environment.GetEnvironmentVariable("ARENA_DUMP_OUT");
            Debug.Log($"dump armed: ticks={(ticks ?? "none")} out={(_dumpOut ?? "none")}");
            _dumpArmed = !string.IsNullOrEmpty(ticks);
            if (!_dumpArmed) return;
            if (string.IsNullOrEmpty(_dumpOut)) _dumpOut = "/tmp/unity_arena.csv";
            foreach (var t in ticks.Split(','))
                if (int.TryParse(t.Trim(), out var n)) _dumpTicks.Add(n);
            _dumpRows.Add("tick,id,kind,cell_x,cell_y,clip,frame,facing,px_size,alpha,sheet");
        }

        void DumpIfDue()
        {
            if (!_dumpArmed) return;
            foreach (var target in _dumpTicks)
                if (_tick >= target && _dumpDone.Add(target)) DumpAt(target);
            if (_dumpDone.Count < _dumpTicks.Count) return;
            System.IO.File.WriteAllText(_dumpOut, string.Join("\n", _dumpRows) + "\n");
            Debug.Log($"ARENA_DUMPER wrote {_dumpOut} ({_dumpRows.Count - 1} rows)");
            _dumpArmed = false;
        }

        void DumpAt(int tick)
        {
            // The ground is measurable — emit it, so the environment port is checked the way the
            // actors are rather than by eye.
            var gt = _fieldRoot.Find("Ground");
            if (gt != null)
            {
                // The ground is a Terrain now: its size lives in TerrainData and its transform
                // sits at a CORNER, so report the centre the quad used to — same row, same
                // meaning, or the comparison reports 84 ground mismatches that are all this line.
                var terr = gt.GetComponent<Terrain>();
                float sx = terr != null ? terr.terrainData.size.x : Mathf.Abs(gt.localScale.x);
                float sz = terr != null ? terr.terrainData.size.z : Mathf.Abs(gt.localScale.y);
                float cx = terr != null ? gt.localPosition.x + sx * 0.5f : gt.localPosition.x;
                float cz = terr != null ? gt.localPosition.z + sz * 0.5f : gt.localPosition.z;
                _dumpRows.Add(string.Format(System.Globalization.CultureInfo.InvariantCulture,
                    "{0},-1,ground,{1:F4},{2:F4},floor,0,0,{3:F5},{4:F3},", tick, -cx, cz, sx, sz));
            }
            var dress = _fieldRoot.Find("Dressing");
            if (dress != null)
                for (int i = 0; i < dress.childCount; i++)
                {
                    var t = dress.GetChild(i);
                    var sr = t.GetComponent<SpriteRenderer>();
                    float ps = sr != null && sr.sprite != null ? 1f / sr.sprite.pixelsPerUnit : 0f;
                    _dumpRows.Add(string.Format(System.Globalization.CultureInfo.InvariantCulture,
                        "{0},{1},prop,{2:F4},{3:F4},{4},0,0,{5:F5},{6:F3},",
                        tick, -100 - i, -t.position.x, t.position.z, t.name, ps, t.position.y));
                }
            if (_cam != null)
                _dumpRows.Add(string.Format(System.Globalization.CultureInfo.InvariantCulture,
                    "{0},-2,camera,{1:F4},{2:F4},cam,{3},{4},{5:F5},{6:F3},",
                    tick, -_cam.transform.position.x, _cam.transform.position.z,
                    Mathf.RoundToInt(-_cam.transform.eulerAngles.x),
                    Mathf.RoundToInt(_cam.transform.eulerAngles.y),
                    _cam.fieldOfView, _cam.transform.position.y));
            var ids = new List<long>(_actors.Keys);
            ids.Sort();
            foreach (var id in ids)
            {
                var a = _actors[id];
                var p = a.Root.transform.localPosition;
                float cx = _gw * 0.5f - 0.5f - p.x;              // inverse of CellToWorld
                float cy = p.z - FieldZShift + _gh * 0.5f - 0.5f;
                float px = a.DbgPx;   // as APPLIED (incl. any boss scale), not the table constant
                string sheet = a.IsPlayer ? (_tab.Class(a.ClassId)?.sheet ?? "") : (_tab.SheetFor(a.Sprite, a.Element) ?? "");
                _dumpRows.Add(string.Format(System.Globalization.CultureInfo.InvariantCulture,
                    "{0},{1},{2},{3:F4},{4:F4},{5},{6},{7},{8:F5},{9:F3},{10}",
                    tick, id, a.IsPlayer ? "player" : "monster", cx, cy,
                    a.DbgAnim, a.DbgFrame, a.Facing, px, a.Body.color.a, sheet));
            }
        }

        void UpdateFacing(Actor a, float now)
        {
            // Cell space (Godot _update_facing): higher cell x = screen right = Facing +1. Motion
            // first, then the target's cell. faces_left sheets flip when Facing > 0.
            // A MARCH OWNS THE FACING. Setting it once when the march starts was not enough: cells
            // do not move during a crossing, so this runs every frame, falls through to the TARGET
            // branch, and snaps the actor back to whatever it was last swinging at — which is
            // behind it. The party walked the whole room backwards (user 2026-09-09, twice).
            if (now < a.MarchUntil) return;

            int desired = a.Facing;
            bool fromMotion = false;
            if (a.CellX > a.PrevCellX) { desired = 1; fromMotion = true; }
            else if (a.CellX < a.PrevCellX) { desired = -1; fromMotion = true; }
            else if (a.TargetId != 0)
            {
                if (a.TargetX > a.CellX) desired = 1; else if (a.TargetX < a.CellX) desired = -1;
            }
            if (desired == a.Facing) return;
            // A skill's motion lock holds the facing too (user 2026-09-27: Steel Break's slash did
            // not turn with her). Its carrier is fixed at spawn, so a flip mid-skill left the
            // sprite and the slash facing opposite ways.
            bool locked = a.StrikeLocked || now < a.FollowUntil || a.MotionLock > 0;
            if (!locked && (fromMotion || now - a.FacingFlipAt >= FacingFlipCooldown))
            { a.Facing = desired; a.FacingFlipAt = now; }
        }

        void AnimatePlayer(Actor a, float now, float dt, float tickFrac)
        {
            var c = _tab.Class(a.ClassId);
            if (c == null) return;
            float h = DisplayPx(_tab, c.fh) * c.Scale * (a.IsBoss ? 1.6f : 1f);
            int frame, row; float lungeK = 0f, frameProgress = 0f; string anim; bool swingOver = false;

            if (!AnimAlive(a))
            {
                a.DeathT += dt;
                frame = Mathf.Min(c.death_frames - 1, Mathf.FloorToInt(a.DeathT * c.death_fps));
                row = c.death_row; anim = "death";
            }
            else if (a.Alive && SeraphinePose(a, out int skillRow, out int skillFrame))
            {
                row=skillRow; frame=skillFrame; anim="skill";
                a.BasicSwordSwing=false; a.StrikeLocked=false; a.FollowUntil=0;
            }
            else if (a.MotionLock > 0)
            {
                // The lock is authoritative, not a timer guessed from frozen atk_acc.
                // Unmapped sheets retain their last pose; Seraphine has an authored cast pose.
                row=a.PoseRow; frame=a.PoseFrame; anim="skill";
                if (a.ClassId=="seraphine" && a.ResolvedSkill=="aegis") { row=3; frame=2; }
            }
            else
            {
                float fps = SwingFps(a, c);
                float accS = Mathf.Clamp(a.AtkAcc + tickFrac * a.EffAspd, 0f, 1000f);
                float ttw = (1000f - accS) / Mathf.Max(1f, a.EffAspd) * _tickDur;
                int accFrame = Mathf.Clamp(c.atk_impact - Mathf.CeilToInt(ttw * fps), 0, c.atk_impact);
                // VISIBILITY runs off the REAL accumulator, the frame number off the smoothed one
                // (player_animator.gd: swing_start_at uses ttw_real). Gating on the smoothed value
                // starts the wind-up up to a full tick early — 0.1 s against a 0.29 s wind-up.
                float ttwReal = (1000f - Mathf.Clamp(a.AtkAcc, 0f, 1000f)) / Mathf.Max(1f, a.EffAspd) * _tickDur;
                bool windupDue = ttwReal <= c.atk_impact / fps && now < a.AttackUntil;
                float strikeDur = (c.atk_impact - c.atk_lunge_from + 1) / fps;

                if (SeraphineWindup(a,c,accFrame,windupDue,out int castRow,out int castFrame))
                {
                    row=castRow; frame=castFrame; anim="skill";
                    a.BasicSwordSwing=false; a.StrikeLocked=false; a.FollowUntil=0;
                }
                else if (a.StrikeLocked)
                {
                    float el = now - a.StrikeT0;
                    frame = Mathf.Clamp(c.atk_lunge_from + Mathf.FloorToInt(el * fps), c.atk_lunge_from, c.atk_impact);
                    frameProgress = Mathf.Clamp01(c.atk_lunge_from + el * fps - frame);
                    row = c.atk_row; anim = "attack";
                    lungeK = strikeDur > 0 ? EaseOut(Mathf.Clamp01(el / strikeDur)) : 1f;
                }
                else if (now < a.FollowUntil)
                {
                    float fe = now - a.FollowT0;
                    int adv = Mathf.FloorToInt(fe * fps);
                    frame = adv < c.atk_impact_hold ? c.atk_impact
                          : Mathf.Clamp(c.atk_impact + 1 + (adv - c.atk_impact_hold), c.atk_impact + 1, c.atk_strike_end);
                    frameProgress = fe * fps - adv;
                    row = c.atk_row; anim = "attack";
                    float outDur = Mathf.Max(0.09f, (c.atk_strike_end - c.atk_impact) / fps);
                    lungeK = EaseIn(1f - Mathf.Clamp01(fe / outDur));
                }
                else
                {
                    // re-arm the strike latch continuously while not yet at lunge_from
                    if (accFrame < c.atk_lunge_from) a.StrikeArmed = true;

                    // The swing is genuinely OVER and the next wind-up is not due yet. The
                    // idle commit delay below exists to stop a 1-tick gap between swings flashing
                    // a pose; here that flicker cannot happen, and waiting only kept the finished
                    // attack clip on screen after the shot had landed (player_animator.gd).
                    swingOver = !windupDue;

                    if (a.Gliding && c.walk_frames > 0)
                    {
                        // MOVING beats the swing wind-up (Godot): walk while the glide is in flight.
                        a.AnimT += dt; frame = Mathf.FloorToInt(a.AnimT * WalkFps(a, c.walk_frames)) % Mathf.Max(1, c.walk_frames);
                        row = c.walk_row; anim = "walk";
                    }
                    else if (c.atk_frames > 0 && windupDue && accFrame >= c.atk_lunge_from && a.StrikeArmed)
                    {
                        a.StrikeArmed = false; a.StrikeLocked = true; a.StrikeT0 = now;
                        a.BasicSwordSwing = string.IsNullOrEmpty(a.PendingSkill) && a.CastFrac < 0f;
                        a.LungeDir = LungeDirTo(a);
                        frame = c.atk_lunge_from; row = c.atk_row; anim = "attack"; lungeK = 0f;
                    }
                    else if (c.atk_frames > 0 && windupDue && accFrame > 0)
                    {
                        frame = Mathf.Clamp(accFrame, 0, c.atk_lunge_from); row = c.atk_row; anim = "attack";
                    }
                    else
                    {
                        a.AnimT += dt; frame = Mathf.FloorToInt(a.AnimT * c.idle_fps) % Mathf.Max(1, c.idle_frames);
                        row = c.idle_row; anim = "idle";
                    }
                }
            }

            // idle commit delay so a 1-tick gap between swings doesn't flash a pose
            if (anim == "idle" && a.AnimName != "idle" && !swingOver)
            {
                if (a.PendingName != "idle") { a.PendingName = "idle"; a.PendingSince = now; }
                if (now - a.PendingSince < AnimSwitchDelay) { row = c.atk_row; frame = c.atk_strike_end; anim = a.AnimName; }
                else a.AnimName = "idle";
            }
            else { a.AnimName = anim; a.PendingName = anim; }

            // hit-stop: freeze the struck frame briefly, never mid-swing
            bool midSwing = anim == "attack" || anim == "skill";
            if (!midSwing && a.Alive && now < a.HitstopUntil)
            { if (a.HitstopFrame < 0) a.HitstopFrame = frame; frame = a.HitstopFrame; }
            else a.HitstopFrame = -1;

            float lungeCells = 0f;
            if (anim == "attack" && a.ProjTravel == 0 && a.AtkRange <= 1) lungeCells = a.LungeDir * c.atk_lunge_cells * lungeK;
            var spr = FrameSprite(c.sheet, c.cols, c.rows, c.fw, c.fh, row * c.cols + frame);
            a.PoseRow=row; a.PoseFrame=frame;
            a.DbgAnim = anim; a.DbgFrame = frame; a.DbgPx = h / Mathf.Max(1, c.fh);
            Place(a, spr, h, c.foot, c.lift, c.faces_left, lungeCells, now);
            a.SwordEdge?.Show(frame, frameProgress, a.ClassId == "seraphine" && a.Alive
                && anim == "attack" && a.BasicSwordSwing && a.ProjTravel == 0 && a.AtkRange <= 1);
        }

        void AnimateMonster(Actor a, float now, float dt)
        {
            var m = _tab.GeomFor(a.Sprite);
            var sheet = _tab.SheetFor(a.Sprite, a.Element);
            if (sheet == null) return;
            var tex = Sheet(sheet);
            int rows = tex != null ? tex.height / m.fw : 16;
            // A BOSS FLOOR, because a boss is keyed by its `sprite` and a bespoke boss shares the
            // trash variant's sprite (chomboom -> chomb, boss_scale 1.0). Godot reaches the boss's
            // own row by def_id, which the view does not carry; without the floor the boss renders
            // at trash size. `BossScaleFloor` is Godot's BOSS_SCALE_DEFAULT.
            float bs = a.IsBoss ? Mathf.Max(m.boss_scale, BossScaleFloor) : 1f;
            float h = DisplayPx(_tab, m.fw) * bs;
            int fi; float lungeCells = 0f;

            if (!AnimAlive(a))
            {
                // base_monster_3d: a monster WITH a hurt clip flinches on the killing blow and
                // only then melts (_death_after_hurt). Cutting straight to the death clip made a
                // one-shot kill read as an instant disappearance.
                a.DeathT += dt;
                var die = m.die_frames;
                var hurt = m.damaged_frames;
                float hurtDur = hurt != null && hurt.Length > 0 ? hurt.Length / Mathf.Max(1f, m.damaged_fps) : 0f;
                if (a.DeathT < hurtDur)
                {
                    fi = hurt[Mathf.Min(hurt.Length - 1, Mathf.FloorToInt(a.DeathT * m.damaged_fps))];
                    a.DbgFrame = fi; a.DbgPx = h / Mathf.Max(1, m.fw); a.DbgAnim = "hurt";
                    Place(a, FrameSprite(sheet, m.cols, rows, m.fw, m.fw, fi), h, m.foot, 0f, m.faces_left, 0f, now);
                    return;
                }
                float dieT = a.DeathT - hurtDur;
                fi = die != null && die.Length > 0 ? die[Mathf.Min(die.Length - 1, Mathf.FloorToInt(dieT * m.die_fps))] : 0;
            }
            else if (a.MonPlaying && m.atk_frames != null && m.atk_frames.Length > 0)
            {
                float fps = Mathf.Max(1f, m.atk_fps);
                int f = Mathf.FloorToInt((now - a.MonAtkT0) * fps);
                if (f >= m.atk_frames.Length) { a.MonAtkOver = true; fi = m.idle_frames != null && m.idle_frames.Length > 0 ? m.idle_frames[0] : 0; }
                else
                {
                    fi = m.atk_frames[f];
                    // _start_lunge_tween: HOLD still through the wind-up, snap in over
                    // LUNGE_SURGE (cubic out) so the peak lands exactly on the contact frame,
                    // then ease back (quad in) over the clip's remaining frames. Easing out from
                    // frame 0 instead is a long slow glide — the same distance, the wrong read.
                    float hitT = m.atk_impact / fps;                 // contact time from clip start
                    float el = now - a.MonAtkT0;
                    float surge = Mathf.Min(_tab.globals.monster_lunge_surge, hitT);
                    float hold = hitT - surge;
                    float back = Mathf.Max(1, m.atk_frames.Length - m.atk_impact) / fps;
                    float k;
                    if (el < hold) k = 0f;
                    else if (el < hitT) { float t = Mathf.Clamp01((el - hold) / Mathf.Max(1e-4f, surge)); k = 1f - Mathf.Pow(1f - t, 3f); }
                    else { float t = Mathf.Clamp01((el - hitT) / back); k = 1f - t * t; }
                    // LUNGE_PX is sprite-local: convert with the shared pixel scale, don't guess cells.
                    float peak = _tab.globals.monster_lunge_px * _tab.cells_per_art_px * _tab.sprite_scale;
                    lungeCells = LungeDirTo(a) * peak * k;
                }
            }
            else if (a.MonDmgPlaying && m.damaged_frames != null && m.damaged_frames.Length > 0)
            {
                int f = Mathf.FloorToInt((now - a.MonDmgT0) * m.damaged_fps);
                if (f >= m.damaged_frames.Length) { a.MonDmgPlaying = false; fi = m.idle_frames[0]; }
                else fi = m.damaged_frames[f];
            }
            else if (a.Gliding && m.move_frames != null && m.move_frames.Length > 0)
            { a.AnimT += dt; fi = m.move_frames[Mathf.FloorToInt(a.AnimT * m.move_fps) % m.move_frames.Length]; }
            else
            { a.AnimT += dt; fi = m.idle_frames != null && m.idle_frames.Length > 0 ? m.idle_frames[Mathf.FloorToInt(a.AnimT * m.idle_fps) % m.idle_frames.Length] : 0; }

            if (a.AtkRange > 1) lungeCells = 0f;
            var spr = FrameSprite(sheet, m.cols, rows, m.fw, m.fw, fi);
            a.DbgFrame = fi; a.DbgPx = h / Mathf.Max(1, m.fw);
            a.DbgAnim = !a.Alive ? "death" : (a.MonPlaying && !a.MonAtkOver) ? "attack" : a.MonDmgPlaying ? "hurt"
                      : a.Gliding ? "move" : "idle";
            Place(a, spr, h, m.foot, 0f, m.faces_left, lungeCells, now);
        }

        float LungeDirTo(Actor a)
        {
            float sx = -(a.CellX - _gw * 0.5f + 0.5f), tx = -(a.TargetX - _gw * 0.5f + 0.5f);
            return Mathf.Approximately(tx, sx) ? a.Facing : Mathf.Sign(tx - sx);
        }

        // 1:1 port of ArenaRender.step_duration — the glide time to cross ONE cell, so the walk
        // spans the real ticks-per-cell instead of a fixed tick and freezing (Godot: "เดินแล้วหยุด").
        float StepDuration(long speedMilli)
        {
            long sp = System.Math.Max(1, speedMilli);
            float ticksPerCell = Mathf.Ceil(1000f / sp);
            return Mathf.Clamp(ticksPerCell * _tickDur, _tickDur, 3.0f);
        }

        float WalkFps(Actor a, int frames)
        {
            float step = Mathf.Max(0.03f, a.GlideDur > 0 ? a.GlideDur : _tickDur);
            return Mathf.Clamp(frames / (WalkCellsPerCycle * step), WalkFpsMin, WalkFpsMax);
        }

        const float MonsterBarGap = .16f;   // world units above a monster's tallest pose

        // The follow-through, plus ONE frame held on `atk_strike_end`: without it the last frame's
        // window was a single frame at 14 fps and an editor frame rate skipped it, ending the swing
        // on frame 4-5 and snapping to idle (measured 2026-09-27).
        float FollowDur(Actor a, ClassGeom c) =>
            (c.atk_strike_end - c.atk_impact + c.atk_impact_hold + 1) / SwingFps(a, c);

        // The attack clip's frame rate, SPED UP to fit the swing interval (user 2026-09-27: "เร่ง
        // speed animation"). One swing shows `atk_impact` frames of wind-up and the follow-through
        // with its held frame; when that is longer than the gap between swings the clip plays
        // faster instead of the next wind-up cutting it. Never slower than the authored rate.
        float SwingFps(Actor a, ClassGeom c)
        {
            float fps = Mathf.Max(1f, c.atk_fps);
            float frames = c.atk_strike_end + c.atk_impact_hold + 1;
            float interval = 1000f / Mathf.Max(1f, a.EffAspd) * _tickDur;
            return interval > 0f ? Mathf.Max(fps, frames / interval) : fps;
        }

        static float EaseOut(float t) => 1f - (1f - t) * (1f - t);
        static float EaseIn(float t) => t * t;

        void Place(Actor a, Sprite spr, float h, float foot, float lift, bool facesLeft, float lungeCells, float now)
        {
            if (spr != null) a.Body.sprite = spr;
            a.Body.transform.localScale = Vector3.one * h;
            float botFrac = spr != null ? BottomPadFrac(spr) : (1f - foot);
            a.Body.transform.localPosition = new Vector3(0f, h * (0.5f - botFrac) + lift, 0f);
            a.Body.flipX = facesLeft ? a.Facing > 0 : a.Facing < 0;

            var pos = CurrentGlidePos(a, now);
            pos.x += lungeCells;   // lunge is horizontal in world x, already signed by direction
            a.LungeX = lungeCells;
            a.Root.transform.localPosition = pos;
            float topFrac=spr!=null&&_topPad.TryGetValue(spr.name,out var top)?top:0;
            float visibleHeight=h*Mathf.Max(.1f,1-botFrac-topFrac);
            float head=visibleHeight+lift+.045f;
            a.HeadY=h+.08f;
            // Measured ONCE per sprite scale, not per frame: a per-frame top made the bar ride
            // every pose (raised weapon, bob) — user 2026-09-26 "หลอดเลือดขยับตามการเคลื่อนไหว".
            // A monster's bar sits over the TALLEST pose seen (it only ever rises, so it never rides
            // the animation) with a wider gap — "เลือดมอนสเตอร์อยู่ติดหัวเกิน" (user 2026-09-26).
            float barY=visibleHeight+(a.IsPlayer ? .045f : MonsterBarGap);
            if (a.BarHeadY<=0 || !Mathf.Approximately(a.BarForH,h)) { a.BarHeadY=barY; a.BarForH=h; }
            else if (!a.IsPlayer && barY>a.BarHeadY) a.BarHeadY=barY;
            a.ContactShadow?.Bind(visibleHeight,lift,a.Alive?1:DeathAlpha);
        }

        // --- sprite plumbing ------------------------------------------------------------------
        float BottomPadFrac(Sprite spr)
        {
            var key = spr.name;
            if (_botPad.TryGetValue(key, out var f)) return f;
            var r = spr.rect;
            int x0 = (int)r.x, y0 = (int)r.y, w = (int)r.width, hgt = (int)r.height;
            float frac = 0f;
            try
            {
                var px = spr.texture.GetPixels(x0, y0, w, hgt);
                int lowest = -1,highest=-1;
                for (int yy = 0; yy < hgt; yy++)
                    for (int xx = 0; xx < w; xx++)
                        if (px[yy * w + xx].a > 0.05f) { if(lowest<0)lowest=yy;highest=yy;break; }
                if (lowest > 0) frac = lowest / (float)hgt;
                _topPad[key]=highest>=0?(hgt-1-highest)/(float)hgt:0;
            }
            catch { frac = 0f; }
            _botPad[key] = frac;
            return frac;
        }

        Sprite FrameSprite(string sheet, int cols, int rows, int fw, int fh, int idx)
        {
            var key = $"{sheet}#{idx}";
            if (_sprites.TryGetValue(key, out var cached)) return cached;
            var tex = Sheet(sheet);
            if (tex == null) return null;
            int col = idx % cols, row = idx / cols;
            int x = col * fw, y = tex.height - (row + 1) * fh;
            if (x < 0 || y < 0 || x + fw > tex.width || y + fh > tex.height) return null;
            var spr = Sprite.Create(tex, new Rect(x, y, fw, fh), new Vector2(0.5f, 0.5f), fh);
            spr.name = key;
            _sprites[key] = spr;
            return spr;
        }

        /// The same loader the actors use, for anything else that needs a sheet out of
        /// StreamingAssets. Static because a projectile layer has no arena to ask before one
        /// exists, and point-filtered because every sheet here is pixel art.
        public static Texture2D LoadArtTexture(string file)
        {
            var path = System.IO.Path.Combine(Application.streamingAssetsPath, "art", file);
            if (!System.IO.File.Exists(path)) return null;
            var tex = new Texture2D(2, 2, TextureFormat.RGBA32, false)
                { filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp };
            return tex.LoadImage(System.IO.File.ReadAllBytes(path)) ? tex : null;
        }

        /// Cell -> world, for the layers that draw between the actors rather than as one.
        public Vector3 CellToWorldPublic(float cx, float cy) => CellToWorld(cx, cy);

        Texture2D Sheet(string file)
        {
            if (_sheets.TryGetValue(file, out var t)) return t;
            var path = System.IO.Path.Combine(Application.streamingAssetsPath, "art", file);
            if (!System.IO.File.Exists(path)) { _sheets[file] = null; return null; }
            var tex = new Texture2D(2, 2, TextureFormat.RGBA32, false) { filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp };
            tex.LoadImage(System.IO.File.ReadAllBytes(path));
            _sheets[file] = tex;
            return tex;
        }

        Actor BuildActor(long id)
        {
            var root = new GameObject($"Actor_{id}");
            root.transform.SetParent(_fieldRoot, false);
            var bill = new GameObject("Billboard").transform;
            bill.SetParent(root.transform, false);

            var bodyGo = new GameObject("Body");
            bodyGo.transform.SetParent(bill, false);
            var sr = bodyGo.AddComponent<SpriteRenderer>();
            // Lit, and casting a shadow: the stock sprite material is unlit, so every light built
            // above would miss the actors entirely (sprite_flash.gdshader exists for the same
            // reason on the Godot side).
            sr.sharedMaterial = ActorMat();
            sr.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.On;
            sr.receiveShadows = false;   // see ActorSprite.shader: a billboard shadows itself

            // Whole actor onto the actor layer, so the ground fill can be culled off it exactly
            // as `view.layers = 2` does in Godot.
            foreach (var t in root.GetComponentsInChildren<Transform>(true)) t.gameObject.layer = ActorLayer;

            return new Actor { Root = root, Billboard = bill, Body = sr, ContactShadow=ActorContactShadow.Create(root.transform) };
        }

        static Material MakeMat(Color c)
        {
            var sh = Shader.Find("Universal Render Pipeline/Unlit") ?? Shader.Find("Unlit/Color");
            var m = new Material(sh) { color = c };
            if (m.HasProperty("_BaseColor")) m.SetColor("_BaseColor", c);
            return m;
        }

        [SerializeField] bool showDiagnostics = false;

        void OnGUI()
        {
#if UNITY_EDITOR || DEVELOPMENT_BUILD
            // Opt-in diagnostics must never cover a gameplay window heading.
            if (!showDiagnostics || _driver == null || _driver.Session == null ||
                (_modeTabs != null && !_modeTabs.IsCombat)) return;
            foreach (var window in UnityEngine.Object.FindObjectsByType<Roi.UI.UiWindow>(FindObjectsSortMode.None))
                if (window.IsOpen) return;
            GUI.Label(new Rect(10, 10, 800, 24), $"P1 arena — {_status}   actors {_actors.Count}   grid {_gw}x{_gh}");
#endif
        }

        // --- view JSON DTOs (JsonUtility ignores the other top-level keys) ---
        [System.Serializable] class ZoneSpawnDTO
        {
            public long opened_at_tick;
            // HOW LONG THE SIM IS HOLDING THE FIGHT, straight from the sim. It varies by what the
            // open has to cover (0.0.343), so a mirrored constant here would be wrong for three of
            // the four cases and there would be no way to see it — the party would simply be
            // sliding when the first monster swings, or standing when it need not.
            public int deploy_grace_ticks;
        }
        [System.Serializable] class ViewDTO
        {
            public long tick; public string current_zone_id; public GridDTO grid;
            // Entity ids SHOVED this tick. The sim reports them (`state.knockbacks`, in the pinned
            // view contract); their glide arcs instead of sliding flat. Ported from arena3d.gd's
            // `kb_ids`, which was never carried across.
            public long[] knockbacks;
            // WHEN THE ROOM OPENED. The zone id is the wrong signal for "a room began": a retry
            // reopens the SAME zone, so the id never changes and the entrance never fired. This is
            // the sim's own origin for the deploy window and it moves on every open — first room,
            // room advance, band swap and retry alike.
            public ZoneSpawnDTO zone_spawn;
            public HitDTO[] recent_hits; public FieldHitDTO[] field_hits; public HealDTO[] heal_fx;
            public SkillFxDTO[] skill_fx; public LootDTO[] loot_fx;
        }
        [System.Serializable] class CmdrView { public Roi.UI.CommanderBar.Skill[] commander_skills; }
        [System.Serializable] class GridDTO { public int width; public int height; public EntityDTO[] entities; }
        [System.Serializable] class HitDTO { public long target_id; public long attacker_id; public long amount; public int hits; public string kind; }
        [System.Serializable] class FieldHitDTO { public long target_id; public long amount; public int hits; }
        [System.Serializable] class HealDTO { public long target_id; public long healer_id; public long amount; public string kind; }
        [System.Serializable] class SkillFxDTO { public long caster_id; public long target_id; public string skill_id; }
        // The loot banner needs a name, a count and a rarity; item_to_view sends the whole item
        // and JsonUtility drops what is not declared here.
        [System.Serializable] class LootDTO { public string name; public string def_id; public string rarity, kind; public int quantity, grade_step; }
        [System.Serializable]
        class EntityDTO
        {
            public long id; public string team; public PosDTO pos; public bool alive;
            public long hp; public long max_hp; public long eff_max_hp;
            public string unit_def; public string sprite; public string element; public bool is_boss;
            public string pending_skill;
            public int motion_lock;
            public SkillStateDTO[] skills;
            public long target_id; public long atk_acc; public long atk_speed_milli; public long eff_atk_speed_milli;
            public long atk_range; public long proj_travel_ticks; public long speed_milli;
            public string mon_name; public CastDTO casting;
            // The gauge is already in the view JSON — `Entity` serialises it and `view_state`
            // builds from `to_value(GameState)`. It only ever had to be DECLARED here: JsonUtility
            // drops what a DTO does not name. `skip_serializing_if = is_zero` means a broken gauge
            // omits `toughness` entirely, which arrives as the 0 it means.
            public int toughness; public int toughness_max;
        }
        [System.Serializable] class CastDTO { public int ticks_left; public int ticks_total; }
        [System.Serializable] class SkillStateDTO { public string id; public long last_fired; }
        [System.Serializable] struct PosDTO { public float x; public float y; }

        // --- arena_tables.json (copied from the Godot renderer) ---
        [System.Serializable]
        public class Tables
        {
            public float cells_per_art_px = 0.052f, sprite_scale = 0.5f, actor_scale = 1.2f, tick_interval = 0.1f;
            public Globals globals = new Globals();
            public Field field = new Field();
            public Look look = new Look();
            public Region[] regions;
            public Dressing[] dressing;
            public GrassPalette grass_palette;
            public GrassWind grass_wind;
            public GrassBlade grass_blade;
            public DaylightStop[] daylight_stops;
            public ClassGeom[] classes;
            public MonSheet[] monster_sheets;
            public MonSheetElem[] monster_sheets_by_element;
            public BaseMon[] base_monster;
            public MonGeom monster;
            [System.NonSerialized] System.Collections.Generic.Dictionary<string, BaseMon> _bespoke;
            [System.NonSerialized] System.Collections.Generic.Dictionary<string, MonGeom> _geomCache;

            /// The bespoke art row for a monster whose `sprite` names one, or null for a slime.
            /// Keyed by the entity's `sprite` field, which for the 6 catalogued monsters equals a
            /// `base_monster` def_id and for the other 61 does not — so a miss here IS the slime
            /// path, exactly as Godot's `BASE_MONSTER.has(did)` decides.
            BaseMon Bespoke(string sprite)
            {
                if (string.IsNullOrEmpty(sprite) || base_monster == null) return null;
                if (_bespoke == null)
                {
                    _bespoke = new();
                    foreach (var b in base_monster)
                        if (b != null && !string.IsNullOrEmpty(b.def_id)) _bespoke[b.def_id] = b;
                }
                return _bespoke.TryGetValue(sprite, out var r) ? r : null;
            }

            /// The geometry a monster animates by — its own if it is a bespoke def, the generic
            /// slime otherwise. Mirrors `_spawn_monster`, which reads its cfg from BASE_MONSTER
            /// when the def is catalogued and SLIME_DEF when it is not.
            public MonGeom GeomFor(string sprite)
            {
                var b = Bespoke(sprite);
                if (b == null) return monster;
                _geomCache ??= new();
                if (_geomCache.TryGetValue(sprite, out var g)) return g;
                g = FromBespoke(b);
                _geomCache[sprite] = g;
                return g;
            }

            static MonGeom FromBespoke(BaseMon b)
            {
                var g = new MonGeom
                {
                    cols = b.cols, fw = b.fw, foot = b.foot_drop, faces_left = !b.faces_right,
                    boss_scale = b.boss_scale, atk_impact = b.hit_frame, hit_juice = b.hit_juice,
                };
                foreach (var c in b.clips ?? System.Array.Empty<Clip>())
                {
                    // A clip is row/start/count on a `cols`-wide sheet; the animator wants ABSOLUTE
                    // frame indices, the same flat shape the generic slime ships.
                    var frames = new int[Mathf.Max(0, c.count)];
                    for (int i = 0; i < frames.Length; i++) frames[i] = c.row * b.cols + c.start + i;
                    int fps = Mathf.Max(1, Mathf.RoundToInt(c.fps));
                    switch (c.name)
                    {
                        case "idle":   g.idle_frames = frames;    g.idle_fps = fps;    break;
                        case "move":   g.move_frames = frames;    g.move_fps = fps;    break;
                        case "attack": g.atk_frames = frames;     g.atk_fps = fps;     break;
                        case "hurt":   g.damaged_frames = frames; g.damaged_fps = fps; break;
                        case "death":  g.die_frames = frames;     g.die_fps = fps;     break;
                    }
                }
                return g;
            }

            /// The sheet a monster draws from — Godot's `_slime_sheet` chain with the bespoke row in
            /// front of it: a def's own sheet, then the sprite key among the slimes, then an element
            /// tint, then pink. The old resolver stopped at the sprite key and returned null, which
            /// drew NOTHING for every band past the three slimes (user 2026-09-11: "พอไป area 11
            /// แล้ว monster ไม่แสดง" — chomb/chomber/chomboom all carry sprite `chomb`, in none of
            /// the three slime rows).
            public string SheetFor(string sprite, string element)
            {
                var b = Bespoke(sprite);
                if (b != null && !string.IsNullOrEmpty(b.sheet)) return b.sheet;
                if (monster_sheets != null)
                    foreach (var m in monster_sheets) if (m.sprite == sprite) return m.sheet;
                if (monster_sheets_by_element != null)
                    foreach (var m in monster_sheets_by_element) if (m.element == element) return m.sheet;
                return "slime_sheet.png";   // Godot's SLIME_SHEET — the pink default
            }

            public static Tables Load()
            {
                var path = System.IO.Path.Combine(Application.streamingAssetsPath, "art", "arena_tables.json");
                if (System.IO.File.Exists(path))
                    return JsonUtility.FromJson<Tables>(System.IO.File.ReadAllText(path));
                Debug.LogError("arena_tables.json missing");
                return new Tables { classes = new ClassGeom[0], monster_sheets = new MonSheet[0], monster = new MonGeom() };
            }
            public ClassGeom Class(string id)
            {
                // Temporary presentation alias until Rapis has an authored sheet (Seraphine's
                // landed 2026-09-16: `seraphine.png`). Keep Actor.ClassId and every sim/UI
                // identity unchanged.
                if (id == "rapis") id = "mage_t1";
                if (classes != null) foreach (var c in classes) if (c.id == id) return c;
                return null;
            }
            public Dressing DressingOf(int r)
            {
                if (dressing != null) foreach (var d in dressing) if (d.region == r) return d;
                return null;
            }
            public Region RegionOf(int r)
            {
                if (regions != null) foreach (var x in regions) if (x.region == r) return x;
                return null;
            }
        }
        [System.Serializable]
        public class ClassGeom
        {
            public string id, sheet; public int cols, rows, fw, fh; public float foot, lift; public bool faces_left;
            /// On-screen size multiplier, 1 = one art pixel per arena pixel like every other
            /// sheet. Kept at 1 for Seraphine (user 2026-09-16: a 0.7 shrink made her PIXELS
            /// smaller than the party's — the authored figure is simply taller than the 60 px
            /// placeholders, and that is the art, not a bug). JsonUtility leaves a missing field
            /// at 0, so 0 reads as 1.
            public float scale;
            public float Scale => scale > 0f ? scale : 1f;
            public int idle_row, idle_frames, idle_fps, walk_row, walk_frames, walk_fps;
            public int atk_row, atk_frames, atk_fps, atk_impact, atk_lunge_from, atk_strike_end, atk_impact_hold;
            public float atk_lunge_cells;
            public int death_row, death_frames, death_fps;
        }
        /// `hit_juice` PER MONSTER, which the table has always carried and this class did not
        /// declare — so JsonUtility dropped it and the code had to read the generic `monster` def
        /// instead. Most sheets say true; the generic says false; the result was that NO monster
        /// ever flashed, including the ones whose own data asked for it. Defaults true because
        /// that is what the table's majority says, so a sheet added without the field behaves like
        /// its neighbours.
        [System.Serializable] public class MonSheet { public string sprite, sheet; }
        [System.Serializable] public class MonSheetElem { public string element, sheet; }
        [System.Serializable] public class Clip { public string name; public int row, start, count; public float fps; }
        [System.Serializable] public class BaseMon
        {
            public string def_id, sheet;
            public int fw = 32, cols = 6, hit_frame = 0;
            public float boss_scale = 1f, foot_drop = 0.5f;
            public bool faces_right, hit_juice = true;
            public Clip[] clips;
        }
        [System.Serializable]
        public class Field
        {
            public float z_shift = -2f, cam_pitch = -42f, cam_fov = 42f;
            public float[] cam_pos = { 0f, 13.5f, 13f };
            public int zones_per_region = 5;
        }
        [System.Serializable]
        public class Region
        {
            public int region; public string bg;
            public float w_cells, h_cells, z_shift; public bool strip, sunny;
        }
        [System.Serializable]
        public class Look
        {
            public float sun_pitch = -34f, sun_yaw = -38f, key_energy = 1.55f;
            public float ambient_energy = 0.30f, shadow_opacity = 0.97f, night_dim = 0.35f;
            public float shade_crush = 0.4f, shade_ceiling = 0.6f;
            public float wet = 0.6f, wet_rough = 0.18f, wet_sat = 0.35f, wet_gain = 1.5f;
            public float fill_energy = 3f, fill_height = 9f, fill_range = 26f;
            // Atmosphere strengths + gates (0 = off), see the exporter's note.
            public float vignette = 0f, godray = 0f, motes = 0f;
            public float cloud = 0f, cloud_scale = 0.13f, cloud_speed = 1.2f;
            public float dof_far_distance = 0f, dof_far_transition = 10f, dof_amount = 0.13f;
            public float[] sun_color = { 1f, 0.97f, 0.91f };
            public float[] ambient_color = { 0.26f, 0.33f, 0.52f };
        }
        [System.Serializable]
        public class GrassPalette
        {
            public float[] tip, body, root; public float root_shade, root_sat;
            public float[] stop0, stop1, stop2; public float off0, off1, off2;
        }
        // One stop per element — a jagged array does not survive JsonUtility (it arrives empty
        // and nothing errors), so the exporter writes objects and this mirrors them.
        [System.Serializable] public class DaylightStop { public float[] rgb; }
        [System.Serializable] public class GrassBlade { public string shape; public float size; }
        [System.Serializable]
        public class GrassWind
        {
            public float[] velocity; public float speed, scale, push_strength, push_gain = 9f, push_decay = 2.5f;
        }
        [System.Serializable]
        public class Dressing
        {
            public int region; public Prop[] props; public GrassClump[] grass;
        }
        [System.Serializable]
        public class Prop
        {
            public string name, tex;
            public float[] pos, offset, tint;
            public float pixel_size, sway, sway_speed, react_radius, react_squash;
            public bool wind, reacts, pool;
            public int cast_shadow;
        }
        [System.Serializable]
        public class GrassClump
        {
            public float[] pos; public float radius, blade_size, react_radius;
            public int blade_count, layout_seed; public bool reacts;
        }
        [System.Serializable]
        public class Globals
        {
            // base_monster_3d.gd authors the lunge in SPRITE-LOCAL px, not cells.
            public float monster_lunge_px = 16f, monster_lunge_surge = 0.1f;
            public float contrast = 1.16f, saturation = 0.98f, brightness = 1.05f;
            public float glow_intensity = 0.95f, glow_bloom = 0.04f, glow_threshold = 0.78f;
            public float actor_exposure = 0.82f;
            public float monster_squash_dur = 0.3f, monster_squash_amp = 0.22f, monster_flash_dur = 0.14f;
            public float[] monster_flash_tint = { 2.2f, 2.2f, 2.2f };
        }
        [System.Serializable]
        public class MonGeom
        {
            public int cols = 6, fw = 32; public float foot = 0.5f; public bool faces_left = true;
            public float boss_scale = 1.6f;   // per-def; the generic slime's is 2.5, NOT the 1.6 default
            public bool hit_juice = true;
            public int[] idle_frames, move_frames, atk_frames, damaged_frames, die_frames;
            public int idle_fps = 4, move_fps = 7, atk_fps = 14, atk_impact = 3, damaged_fps = 11, die_fps = 8;
            public float atk_lunge_cells = 0.4f;
        }
    }
}
