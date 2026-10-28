// A SECOND sim, for story stages. It can never save and never talks to the server. Ported from
// godot/story_driver.gd.
//
// A story fight is a real fight — the sim has to run it — but nothing in it may reach the player's
// account: no XP, no loot, no save. The cheap way to get that is a second session with its own
// GameState, because then the isolation is STRUCTURAL. The alternative is one sim with "if we are
// in a story, skip the save / skip the XP / skip the drop" scattered through it, and the day
// someone forgets one of those conditions a story fight quietly edits a real account.
//
// So this file simply does not contain a save path or a server client. There is nothing to forget.
//
// This amends the project rule ("there is exactly ONE SimBridge") to "exactly one AUTHORITATIVE
// sim". A session holds only its own handle with no statics, so a second one shares nothing, and a
// tick costs microseconds — running two is free.
using System.Collections.Generic;
using UnityEngine;

namespace Roi.Story
{
    public sealed class StoryDriver : MonoBehaviour
    {
        public const ulong Seed = 20260807;
        const float TickInterval = 0.1f;             // 100 ms per tick, same as the live sim
        const long PresetGil = 100_000_000;          // a sandbox wallet, so a preset cast is free

        /// The view JSON after each tick batch — the same shape the arena renders from.
        public event System.Action<string> StateUpdated;
        /// How a stage stopped. Three outcomes, and they are NOT interchangeable:
        ///   "scripted" — the authored tick limit ran out. The script CONTINUES; this is how a
        ///                written defeat happens, because a pre-made party at power would
        ///                otherwise just win.
        ///   "cleared"  — the zone is done. The script continues.
        ///   "wiped"    — the party actually lost. The mission FAILED and the script is abandoned.
        /// `kills` rides ALONG because the stage's state does not survive the event: `Stop()` runs
        /// first (it has to — a stale session would keep ticking) and drops the state.
        public event System.Action<string, string, int> StageEnded;

        public Roi.Sim.SimSession Session { get; private set; }
        public string StageId { get; private set; } = "";
        public bool Running { get; private set; }

        float _acc;
        string _view = "{}";
        int _endAfterTicks, _ticksRun;
        bool _sawParty;

        public static StoryDriver Create(Transform parent)
        {
            var go = new GameObject("StoryDriver");
            go.transform.SetParent(parent, false);
            return go.AddComponent<StoryDriver>();
        }

        /// One member of a mission's cast.
        public struct Cast
        {
            public string ClassId;
            public int Level;
            public string AdvanceTo;
            public bool MaxKit, Gear;
            public static Cast Of(string unitDef, int level)
                => new Cast { ClassId = unitDef, Level = level, MaxKit = true, Gear = true };
        }

        /// Begin a stage. An EMPTY roster leaves whatever the zone's own start produces.
        /// Returns false and changes nothing if the sim refuses to start.
        public bool StartStage(string id, string zoneId, IList<Cast> roster, int endAfterTicks = 0)
        {
            Stop();
            var gd = Roi.Sim.SimSession.LoadCatalog();
            if (string.IsNullOrEmpty(gd)) { Debug.LogWarning($"STORY: no catalog — '{id}' not started"); return false; }
            // The lead is created WITH the cast's first class rather than bought: the zone's own
            // start already seeds a lead, and buying on top of it fields four for a cast of three.
            var lead = roster != null && roster.Count > 0 ? roster[0].ClassId : "";
            var s = string.IsNullOrEmpty(lead)
                  ? Roi.Sim.SimSession.New(Seed, gd, zoneId)
                  : Roi.Sim.SimSession.NewAs(Seed, gd, zoneId, lead, "Trial");
            if (s == null) { Debug.LogWarning($"STORY: the sim refused zone '{zoneId}'"); return false; }
            // Direct state edits are refused by default because they are not recorded in the input
            // log, so a save containing one would stop verifying. The flag is PER SESSION, and this
            // session has no save and no server — there is nothing here for a fabricated state to
            // invalidate. It is what lets a preset cast be built with no sim-core work at all.
            s.SetDebugAllowed(true);
            Session = s;
            BuildRoster(roster);
            StageId = id;
            _endAfterTicks = endAfterTicks;
            _ticksRun = 0;
            _sawParty = false;
            _acc = 0f;
            Running = true;
            Publish();
            return true;
        }

        /// End the stage and drop the sandbox. The session is released rather than kept warm: a
        /// story GameState that outlives its stage is a thing that can be read by mistake.
        public void Stop()
        {
            bool had = Session != null;
            Running = false;
            Session?.Dispose();
            Session = null;
            StageId = "";
            _ticksRun = 0;
            _sawParty = false;
            _view = "{}";
            // ANNOUNCE the end with an empty state, and it is not a courtesy.
            //
            // Every stage builds a FRESH session, and a fresh GameState restarts the monster id
            // counter — so the second stage's monsters carry the same ids as the first one's. A
            // renderer that was never told the sandbox died still holds actors for those ids at the
            // OLD stage's positions, sees the "same" monster somewhere far away, and glides it in
            // from where its predecessor fell. It cannot happen in the live game because there is
            // only ever one session and ids never rewind.
            if (had) StateUpdated?.Invoke("{}");
        }

        void OnDestroy() => Stop();

        void Update()
        {
            if (!Running || Session == null || !Session.IsAlive) return;
            _acc += Time.unscaledDeltaTime;
            int n = (int)(_acc / TickInterval);
            if (n <= 0) return;
            if (n > 50) n = 50;
            _acc -= n * TickInterval;
            Session.Tick((ulong)n);
            _ticksRun += n;
            Publish();

            var outcome = Outcome();
            if (outcome == "") return;
            var id = StageId;
            int kills = Kills();          // read BEFORE Stop() drops the state
            Stop();
            StageEnded?.Invoke(id, outcome, kills);
        }

        void Publish()
        {
            _view = Session != null ? Session.ViewJson() : "{}";
            StateUpdated?.Invoke(_view);
        }

        /// "" while the stage is still running.
        ///
        /// A wipe is only checked AFTER the party has been seen alive once: at tick 0 the grid is
        /// still being populated, and an empty party then is not a defeat.
        string Outcome()
        {
            var v = JsonUtility.FromJson<StageViewDTO>(_view);
            int alive = 0;
            if (v != null && v.grid != null && v.grid.entities != null)
                foreach (var e in v.grid.entities)
                    if (e.alive && e.team == "Player" && !e.is_pet) alive++;
            if (alive > 0) _sawParty = true;
            if (_sawParty && alive == 0) return "wiped";
            if (_endAfterTicks > 0 && _ticksRun >= _endAfterTicks) return "scripted";
            // The SIM decides what cleared means, not a corpse count here. Counting live enemies
            // was wrong in a way that only shows up sometimes: a zone spawns on an interval, so the
            // gap between one wave dying and the next appearing reads as an empty field and ended
            // the stage early. `zone_cleared` is the same flag the live game uses.
            if (v != null && v.zone_spawn != null && v.zone_spawn.zone_cleared) return "cleared";
            return "";
        }

        int Kills()
        {
            var v = JsonUtility.FromJson<StageViewDTO>(_view);
            return v != null && v.zone_spawn != null ? v.zone_spawn.kill_counter : 0;
        }

        /// Deployed party members, by id. `team == "Player"` is the discriminator the live driver
        /// uses; there is no `is_player` field on a grid entity.
        public List<long> PartyIds()
        {
            var outp = new List<long>();
            var v = JsonUtility.FromJson<StageViewDTO>(_view);
            if (v != null && v.grid != null && v.grid.entities != null)
                foreach (var e in v.grid.entities)
                    if (e.team == "Player" && !e.is_pet) outp.Add(e.id);
            outp.Sort();
            return outp;
        }

        /// The class a member is, for `@ remove_member`.
        public string ClassOf(long id)
        {
            var v = JsonUtility.FromJson<StageViewDTO>(_view);
            if (v != null && v.grid != null && v.grid.entities != null)
                foreach (var e in v.grid.entities)
                    if (e.id == id) return e.unit_def ?? "";
            return "";
        }

        public void Kill(long id) => Session?.Debug("{\"kill\":{\"entity_id\":" + id + "}}");

        // --- the preset cast ------------------------------------------------------------------

        /// Build the mission's cast out of the DEBUG entry points.
        ///
        /// Those exist for cheats, and using them here is safe for exactly the reason this whole
        /// file exists: this sandbox can never save and can never reach the server, so a state it
        /// fabricates cannot become an account's state. It also means a preset cast costs NOTHING
        /// in sim-core — no new input, no VERSION bump, no golden re-bless.
        void BuildRoster(IList<Cast> roster)
        {
            if (roster == null || roster.Count == 0) { Publish(); return; }
            Session.Debug("{\"give_currency\":{\"gil\":" + PresetGil + ",\"soul\":" + PresetGil + "}}");
            for (int i = 1; i < roster.Count; i++)
            {
                if (string.IsNullOrEmpty(roster[i].ClassId)) continue;
                // `BuyCharacter`, NOT `RecruitMember`. The two look interchangeable and are not:
                // recruiting is gated on the LEAD'S LEVEL ("band 3-4 = recruit +1"), and a story
                // cast is built before anything is kitted, so the lead is Lv1 and every recruit
                // refused in silence. A three-person cast fielded one member and the scene played
                // out around a party that was never there.
                Session.ApplyInput("{\"BuyCharacter\":{\"unit_def\":\"" + roster[i].ClassId + "\"}}");
            }
            // Recruiting BENCHES; deploying is a second event, and the id it needs comes from the
            // roster query rather than from the recruit — the input path returns nothing.
            foreach (var charId in BenchedIds())
                Session.ApplyInput("{\"DeployCharacter\":{\"char_id\":" + charId + "}}");
            Publish();

            // Kit in party order, which is id order, which is the order they were added.
            var ids = PartyIds();
            for (int i = 0; i < ids.Count && i < roster.Count; i++) Kit(ids[i], roster[i]);
            Publish();
        }

        List<int> BenchedIds()
        {
            var outp = new List<int>();
            var rows = JsonUtility.FromJson<RosterList>("{\"rows\":" + Session.QueryJson("roster") + "}");
            if (rows != null && rows.rows != null) foreach (var r in rows.rows) outp.Add(r.id);
            return outp;
        }

        void Kit(long entityId, Cast spec)
        {
            // Class advancement is retired (0.0.352); a cast's `AdvanceTo` is ignored.
            if (spec.MaxKit) Session.Debug($"{{\"max_skills\":{{\"entity_id\":{entityId}}}}}");
            if (spec.Gear) Session.Debug($"{{\"give_gear\":{{\"entity_id\":{entityId}}}}}");
            // LAST, and it is not cosmetic. Raising a level or handing over gear moves the max HP
            // ceiling and leaves CURRENT hp where it was, so a level-40 preset cast walked onto the
            // field at 43 of 962 — the level-1 value — and died to the first thing it met. The
            // level cheat refills to the new ceiling, so it goes after everything that moves it.
            if (spec.Level > 1)
                Session.Debug($"{{\"add_levels\":{{\"entity_id\":{entityId},\"n\":{spec.Level - 1}}}}}");
        }

        [System.Serializable] class StageViewDTO { public StageGridDTO grid; public SpawnDTO zone_spawn; }
        [System.Serializable] class StageGridDTO { public StageEntDTO[] entities; }
        [System.Serializable]
        class StageEntDTO { public long id; public string team, unit_def; public bool alive, is_pet; }
        [System.Serializable] class SpawnDTO { public bool zone_cleared; public int kill_counter; }
        [System.Serializable] class RosterList { public RosterRow[] rows; }
        [System.Serializable] class RosterRow { public int id; public string unit_def; }
    }
}
