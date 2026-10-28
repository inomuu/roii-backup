// Read-only projection for the Exemplar screen. Browsing owns no gameplay state: it neither
// changes PartySelection nor deploys a character to make a deployed-only query available.
using System;
using System.Collections.Generic;
using Roi.Sim;
using UnityEngine;

namespace Roi.UI
{
    public static class ExemplarReadModel
    {
        public sealed class Snapshot
        {
            public bool Available;
            public readonly List<Member> Members = new List<Member>();

            public Member Find(long id)
            {
                foreach (var member in Members)
                    if (member.Id == id) return member;
                return null;
            }
        }

        public sealed class Member
        {
            public long Id;
            public string UnitDef, Element;
            public int Level, JobLevel;
            public bool Benched;
            // Null means this projection is unavailable, not a character with zero stats or an
            // empty loadout. The host currently exposes only identity and levels for the bench.
            public Eff Stats;
            public ItemView[] Equipment;
            public RegaliaCrown.View Regalia;
        }

        public sealed class Eff
        {
            public int Level, LevelCap;
            public long Experience, MaxHp;
            public long Attack, Defense;
            public int DodgeBp, CritBp, CritDamageBp, MitigationBp, AttackSpeedX100;
        }

        /// Fetch on the Unity main thread, on open and at the screen's refresh interval. The
        /// session is borrowed for this call only; a new login or save slot may replace it later.
        public static Snapshot Fetch(SimDriver driver)
        {
            var result = new Snapshot();
            var session = driver != null ? driver.Session : null;
            if (session == null || !session.IsAlive) return result;

            var view = ReadObject<ViewDto>(session.ViewJson());
            if (view?.grid?.entities == null) return result;
            result.Available = true;
            var seen = new HashSet<long>();
            foreach (var entity in view.grid.entities)
            {
                // A downed ally remains owned and deployed until the sim actually benches it.
                if (entity == null || entity.team != "Player" || entity.is_pet
                    || entity.id <= 0 || !seen.Add(entity.id)) continue;
                var stats = ReadStats(session.QueryJson("stats:" + entity.id));
                result.Members.Add(new Member
                {
                    Id = entity.id,
                    UnitDef = entity.unit_def ?? "",
                    Element = entity.element ?? "",
                    Level = stats != null ? stats.Level : entity.stats?.level ?? 0,
                    JobLevel = entity.stats?.job_level ?? 0,
                    Benched = false,
                    Stats = stats,
                    Equipment = entity.equipped,
                    Regalia = entity.regalia
                });
            }

            // This query is the bench, whereas owned_exemplars is the legacy wearable-carrier
            // collection. Combining carriers with entities would invent playable characters.
            var roster = ReadArray<RosterDto>(session.QueryJson("roster"));
            if (roster?.rows != null)
                foreach (var row in roster.rows)
                {
                    if (row == null || row.id <= 0 || !seen.Add(row.id)) continue;
                    result.Members.Add(new Member
                    {
                        Id = row.id,
                        UnitDef = row.unit_def ?? "",
                        Element = "",
                        Level = row.level,
                        JobLevel = row.job_level,
                        Benched = true
                    });
                }
            return result;
        }

        static Eff ReadStats(string json)
        {
            var stats = ReadObject<StatsDto>(json);
            // The host answers {} for a missing entity. JsonUtility otherwise turns that into
            // confident zero values; a playable character always has a positive base level.
            if (stats == null || stats.level <= 0) return null;
            return new Eff
            {
                Level = stats.level,
                LevelCap = stats.level_cap,
                Experience = stats.xp,
                MaxHp = stats.max_hp,
                Attack = stats.atk,
                Defense = stats.def,
                DodgeBp = stats.dodge_bp,
                CritBp = stats.crit_bp,
                CritDamageBp = stats.crit_dmg_bp,
                MitigationBp = stats.mitigation_bp,
                AttackSpeedX100 = stats.atk_speed_x100
            };
        }

        static T ReadObject<T>(string json) where T : class
        {
            if (string.IsNullOrWhiteSpace(json) || json.TrimStart()[0] != '{') return null;
            // Only malformed JSON is handled here. Native-library or programmer errors should
            // surface instead of being disguised as an empty roster.
            try { return JsonUtility.FromJson<T>(json); }
            catch (ArgumentException) { return null; }
        }

        static T ReadArray<T>(string json) where T : class
        {
            if (string.IsNullOrWhiteSpace(json) || json.TrimStart()[0] != '[') return null;
            return ReadObject<T>("{\"rows\":" + json + "}");
        }

        [Serializable] sealed class ViewDto { public GridDto grid; }
        [Serializable] sealed class GridDto { public EntityDto[] entities; }
        [Serializable] sealed class EntityDto
        {
            public long id;
            public string team, unit_def, element;
            public bool is_pet;
            public LevelDto stats;
            public ItemView[] equipped;
            public RegaliaCrown.View regalia;
        }
        [Serializable] sealed class LevelDto { public int level, job_level; }
        [Serializable] sealed class RosterDto { public BenchDto[] rows; }
        [Serializable] sealed class BenchDto
        {
            public long id;
            public string unit_def;
            public int level, job_level;
        }
        [Serializable] sealed class StatsDto
        {
            public int level, level_cap;
            public long xp, max_hp, atk, def;
            public int dodge_bp, crit_bp, crit_dmg_bp, mitigation_bp, atk_speed_x100;
        }
    }
}
