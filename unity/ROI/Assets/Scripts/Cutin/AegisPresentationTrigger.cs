using System;
using System.Collections.Generic;
using Roi.Sim;
using UnityEngine;

namespace Roi.Cutin
{
    // Consume the resolved-event contract; only the portrait is once per session.
    public sealed class AegisPresentationTrigger
    {
        SimSession _session;
        bool _shown;
        long _tick=-1;
        readonly HashSet<long> _played=new();
        public bool TryPresent(SimSession session,string json,Func<long,HashSet<long>,bool,bool> play)
        {
            if (session==null) return false;
            if (!ReferenceEquals(session,_session)) { _session=session; _shown=false; _tick=-1; _played.Clear(); }
            if (string.IsNullOrEmpty(json) || !json.Contains("\"ult_fx\"")) return false;
            var v=JsonUtility.FromJson<View>(json);
            if (v?.ult_fx==null || v.grid?.entities==null) return false;
            if (_tick!=v.tick) { _tick=v.tick; _played.Clear(); }
            foreach (var fx in v.ult_fx)
            {
                if (fx==null || fx.skill_id!="aegis" || _played.Contains(fx.caster_id)) continue;
                bool player=false;
                foreach (var e in v.grid.entities)
                    if (e!=null && e.id==fx.caster_id && e.team=="Player" && !e.is_pet) player=true;
                if (!player) continue;
                var recipients=new HashSet<long>();
                foreach (var e in v.grid.entities)
                    if (e!=null && e.alive && !e.is_pet && e.shields!=null)
                        foreach (var shield in e.shields)
                            if (shield.src_id==fx.caster_id && shield.source_skill_id=="aegis" && shield.pool>0)
                                recipients.Add(e.id);
                if (recipients.Count==0 || !play(fx.caster_id,recipients,!_shown)) continue;
                _played.Add(fx.caster_id); _shown=true; return true;
            }
            return false;
        }
        [Serializable] class View { public long tick; public Fx[] ult_fx; public Grid grid; }
        [Serializable] class Fx { public long caster_id; public string skill_id; }
        [Serializable] class Grid { public Entity[] entities; }
        [Serializable] class Entity { public long id; public string team; public bool is_pet,alive; public Shield[] shields; }
        [Serializable] class Shield { public long src_id; public string source_skill_id; public int pool; }
    }
}
