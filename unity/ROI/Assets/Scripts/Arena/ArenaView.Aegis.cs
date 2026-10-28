using System.Collections.Generic;
using Roi.Cutin;
using Roi.Sim;
using UnityEngine;
using UnityEngine.InputSystem;

namespace Roi.Arena
{
    public sealed partial class ArenaView
    {
        readonly AegisPresentationTrigger _aegisTrigger=new();
        SeraphineAegisSequence _aegis;
        SimSession _aegisSession;
        string _aegisPending, _aegisZone;
        long _aegisTick;
        float _aegisStart, _aegisScaledStart;
        bool _aegisHolding, _aegisReplay, _aegisHudHeld, _aegisLiveSuspended;
        readonly Dictionary<Behaviour,bool> _aegisBehaviours=new();
        readonly Dictionary<Renderer,bool> _aegisRenderers=new();
        readonly List<ParticleSystem> _aegisParticles=new();

        bool DeferForAegis(string json)
        {
            if (_aegisReplay || _aegisLiveSuspended || _driver==null) return false;
            if (_aegisHolding)
            {
                var state=JsonUtility.FromJson<AegisStamp>(json);
                if (ReferenceEquals(_aegisSession,_driver.Session) && state!=null &&
                    state.current_zone_id==_aegisZone && state.tick==_aegisTick)
                { _aegisPending=json; return true; }
                EndAegisPresentation(false);
            }
            if (!isActiveAndEnabled || (_modeTabs!=null && !_modeTabs.IsCombat)) return false;
            if (!_aegisTrigger.TryPresent(_driver.Session,json,StartAegisPresentation)) return false;
            var stamp=JsonUtility.FromJson<AegisStamp>(json);
            _aegisPending=json; _aegisZone=stamp.current_zone_id; _aegisTick=stamp.tick;
            return true;
        }

        bool StartAegisPresentation(long caster,HashSet<long> recipients,bool portrait)
        {
            if (_cam==null || _driver==null || _aegisLiveSuspended || _aegisHolding || !_actors.TryGetValue(caster,out var source) || !source.Alive) return false;
            if (FindFirstObjectByType<Live2DCutin>()!=null || FindFirstObjectByType<SeraphineUltimateCutin>()!=null) return false;
            // Do not compete with a room march or an existing Break camera beat.
            if (_focusAge>=0 || _marchPending || _walkInPending) return false;
            var actors=new List<SeraphineAegisSequence.Actor>();
            foreach (var pair in _actors)
            {
                var actor=pair.Value;
                if (actor.Root==null || actor.Body==null || !actor.Init) continue;
                actors.Add(new SeraphineAegisSequence.Actor { Root=actor.Root.transform,Body=actor.Body,
                    Height=actor.BarHeadY>0 ? actor.BarHeadY : actor.HeadY,
                    Recipient=recipients.Contains(pair.Key),Caster=pair.Key==caster });
            }
            _aegis=SeraphineAegisSequence.Create(_cam,actors,_driver,portrait);
            if (_aegis==null) return false;
            _aegisSession=_driver.Session; _aegisStart=Time.unscaledTime; _aegisScaledStart=Time.time;
            _aegisHolding=true;
            if (_hud!=null)
            {
                _aegisHudHeld=_hud.PresentationHeld; _hud.PresentationHeld=true;
                foreach (var canvas in _hud.GetComponentsInChildren<Canvas>()) HoldBehaviour(canvas);
            }
            foreach (var component in new MonoBehaviour[] { _sparks,_shots,_breakFlash,_shards })
            {
                if (component==null) continue;
                HoldBehaviour(component);
                foreach (var r in component.GetComponentsInChildren<Renderer>()) HoldRenderer(r);
                foreach (var ps in component.GetComponentsInChildren<ParticleSystem>())
                    if (ps.isPlaying) { _aegisParticles.Add(ps); ps.Pause(false); }
            }
            foreach (var a in _actors.Values)
                if (a.SwordEdge!=null)
                    foreach (var r in a.SwordEdge.GetComponentsInChildren<Renderer>()) HoldRenderer(r);
            foreach (var cast in _seraphineCasts)
                if (cast.Root!=null)
                    foreach (var r in cast.Root.GetComponentsInChildren<Renderer>()) HoldRenderer(r);
            return true;
        }

        void HoldBehaviour(Behaviour b)
        {
            if (_aegisBehaviours.ContainsKey(b)) return;
            _aegisBehaviours[b]=b.enabled; b.enabled=false;
        }
        void HoldRenderer(Renderer r)
        {
            if (_aegisRenderers.ContainsKey(r)) return;
            _aegisRenderers[r]=r.enabled; r.enabled=false;
        }
        bool TickAegisPresentation()
        {
            if (!_aegisHolding) return false;
            if (_aegis==null || _aegis.Restored || _driver==null ||
                !ReferenceEquals(_aegisSession,_driver.Session) || (_modeTabs!=null && !_modeTabs.IsCombat))
            { EndAegisPresentation(false); return true; }
            if (Keyboard.current!=null && Keyboard.current.escapeKey.wasPressedThisFrame)
            { EndAegisPresentation(true); return true; }
            _aegis.Sample(Time.unscaledTime-_aegisStart);
            if (_aegis.Restored) EndAegisPresentation(true);
            return true;
        }

        void EndAegisPresentation(bool replay)
        {
            if (!_aegisHolding) return;
            _aegisHolding=false;
            if (_aegis!=null) { _aegis.Restore(); SeraphineAegisSequence.DestroyObject(_aegis.gameObject); }
            _aegis=null;
            float elapsed=Time.time-_aegisScaledStart;
            // Absolute animation deadlines resume at their captured phase, not at wall-clock time.
            _tickTime+=elapsed; _panStart+=elapsed;
            foreach (var a in _actors.Values)
            {
                a.GlideT0+=elapsed; a.FacingFlipAt+=elapsed; a.AttackUntil+=elapsed;
                a.StrikeT0+=elapsed; a.FollowUntil+=elapsed; a.FollowT0+=elapsed;
                a.MarchUntil+=elapsed; a.PendingSince+=elapsed; a.MonAtkT0+=elapsed;
                a.MonDmgT0+=elapsed; a.HurtUntil+=elapsed; a.HitstopUntil+=elapsed;
                a.DeathHoldUntil+=elapsed;
            }
            if (_sparks!=null) _sparks.DelayPresentation(elapsed);
            if (_shots!=null) _shots.DelayPresentation(elapsed);
            if (_breakFlash!=null) _breakFlash.DelayPresentation(Time.unscaledTime-_aegisStart);
            foreach (var pair in _aegisBehaviours) if (pair.Key!=null) pair.Key.enabled=pair.Value;
            foreach (var pair in _aegisRenderers) if (pair.Key!=null) pair.Key.enabled=pair.Value;
            foreach (var ps in _aegisParticles) if (ps!=null) ps.Play(false);
            _aegisBehaviours.Clear(); _aegisRenderers.Clear(); _aegisParticles.Clear();
            if (_hud!=null) _hud.PresentationHeld=_aegisHudHeld;
            var pending=_aegisPending; _aegisPending=null;
            if (!replay || pending==null || _driver==null || !ReferenceEquals(_aegisSession,_driver.Session)) return;
            _aegisReplay=true;
            try { OnView(pending); }
            finally { _aegisReplay=false; }
        }

#if UNITY_EDITOR
        public bool DebugAegis(bool portrait=true)
        {
            long caster=0;
            var targets=new HashSet<long>();
            foreach (var pair in _actors)
                if (pair.Value.Alive && pair.Value.IsPlayer)
                {
                    targets.Add(pair.Key);
                    if (pair.Value.ClassId=="seraphine") caster=pair.Key;
                }
            return caster!=0 && StartAegisPresentation(caster,targets,portrait);
        }
#endif
        [System.Serializable] class AegisStamp { public long tick; public string current_zone_id; }
    }
}
