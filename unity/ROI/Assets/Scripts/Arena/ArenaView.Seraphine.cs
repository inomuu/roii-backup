using System.Collections.Generic;
using UnityEngine;

namespace Roi.Arena
{
    public sealed partial class ArenaView
    {
        readonly List<SeraphineCast> _seraphineCasts=new();
        readonly HashSet<(long,string)> _seraphinePlayed=new();
        long _seraphineTick=-1, _seraphineOpened=-1;
        long _seraphineBaselineTick=-1;
        string _seraphineZone;

        sealed class SeraphineCast
        {
            public long Caster;
            public string Skill;
            public Actor Actor;
            public GameObject Root;
            public CircularSwordSlash Slash;
            public float Age, Height;
            public Vector3 Origin;
            public readonly List<SeraphineTarget> Targets=new();
        }

        sealed class SeraphineTarget
        {
            public Actor Actor;
            public IronChainsEffect Chain;
            public SwordHitImpact Impact;
            public Vector3 Contact;
            public bool Hit;
        }

        void PrepareSeraphineView(ViewDTO view)
        {
            long opened=view.zone_spawn?.opened_at_tick ?? 0;
            if (_seraphineZone!=view.current_zone_id || _seraphineOpened!=opened || view.tick<_seraphineTick)
                ClearSeraphineSkills();
            _seraphineZone=view.current_zone_id; _seraphineOpened=opened;
            if (_seraphineTick<0) _seraphineBaselineTick=view.tick;
            if (_seraphineTick!=view.tick) { _seraphinePlayed.Clear(); _seraphineTick=view.tick; }
        }

        void FeedSeraphineSkills(ViewDTO view)
        {
            if (_cam==null || view.grid.entities==null || view.tick==_seraphineBaselineTick) return;
            foreach (var entity in view.grid.entities)
            {
                if (entity.unit_def!="seraphine" || entity.team!="Player" || entity.skills==null
                    || !_actors.TryGetValue(entity.id,out var actor) || !actor.Alive) continue;
                foreach (var skill in entity.skills)
                {
                    // Instant self-AoEs do not emit skill_fx in the current sim. last_fired is
                    // authoritative; pending_skill is only a prediction and must never fire VFX.
                    if ((skill.id!="steel_break" && skill.id!="iron_chains") || skill.last_fired<=0
                        || skill.last_fired!=view.tick || !_seraphinePlayed.Add((entity.id,skill.id))) continue;
                    StartSeraphineSkill(entity.id,actor,skill.id,view.recent_hits);
                }
            }
        }

        void StartSeraphineSkill(long caster,Actor actor,string skill,HitDTO[] hits)
        {
            for (int i=_seraphineCasts.Count-1;i>=0;i--)
                if (_seraphineCasts[i].Caster==caster) RemoveSeraphineCast(i);
            var root=new GameObject("Seraphine live "+skill);
            // No actor parenting: slash/aura carriers retain their spawn world transform.
            UnityEngine.SceneManagement.SceneManager.MoveGameObjectToScene(root,gameObject.scene);
            root.transform.position=actor.Root.transform.position;
            root.layer=actor.Body.gameObject.layer;
            var cast=new SeraphineCast { Caster=caster,Actor=actor,Skill=skill,Root=root,
                Height=Mathf.Max(.1f,actor.BarHeadY>0 ? actor.BarHeadY : actor.HeadY) };
            cast.Origin=root.transform.position+Vector3.up*cast.Height*.38f
                +_cam.transform.forward*cast.Height*.18f;
            actor.BasicSwordSwing=false; actor.StrikeLocked=false; actor.FollowUntil=0;
            actor.SwordEdge?.Show(0,0,false);
            _hud?.SpawnSkillLabel(caster,SkillLabelText(skill));
            if (skill=="steel_break")
            {
                cast.Slash=CircularSwordSlash.Create(actor.Body,root.transform,cast.Height);
                if (cast.Slash==null || !cast.Slash.FreezeAtSpawn()) { Kill(root); return; }
            }
            // Use the sim's affected IDs, never a client radius/nearest-enemy search.
            // This kit resolves one instant offensive skill per caster per tick.
            var targets=new HashSet<long>();
            if (hits!=null) foreach (var hit in hits)
            {
                if (hit.attacker_id!=caster || !targets.Add(hit.target_id)
                    || !_actors.TryGetValue(hit.target_id,out var target) || target.IsPlayer) continue;
                var entry=new SeraphineTarget { Actor=target,Contact=target.Body.bounds.center,Hit=hit.amount>0 };
                if (skill=="iron_chains")
                {
                    entry.Chain=IronChainsEffect.Create(root.transform,cast.Height,cast.Targets.Count==0);
                    // Re-time only the visual glide to a position already supplied by the sim.
                    if (target.Alive && target.Gliding)
                    {
                        target.GlideT0=Time.time+IronChainsEffect.PullTime;
                        target.GlideDur=.22f;
                    }
                }
                if (entry.Hit)
                {
                    entry.Impact=SwordHitImpact.Create(root.transform,cast.Height*.65f);
                    entry.Impact?.FreezeAtSpawn();
                }
                cast.Targets.Add(entry);
            }
            _seraphineCasts.Add(cast);
        }

        static bool SeraphineWindup(Actor actor,ClassGeom art,int attackFrame,bool due,out int row,out int frame)
        {
            row=frame=0;
            if (actor.ClassId!="seraphine" || actor.Gliding || !due) return false;
            switch (actor.PendingSkill)
            {
                // special_1's authored open-hand row, also used by the chain reach.
                // The sheet has no distinct Aegis clip; do not invent a sword swing for it.
                case "aegis":
                case "iron_chains":
                    row=3; frame=Mathf.Clamp(attackFrame,0,2); return true;
                case "steel_break":
                    row=art.atk_row; frame=Mathf.Clamp(attackFrame,0,1); return true;
                default: return false;
            }
        }

        bool SeraphinePose(Actor actor,out int row,out int frame)
        {
            row=frame=0;
            // The visual tail can persist, but it must not own the next action's sprite.
            if (actor.MotionLock<=0) return false;
            foreach (var cast in _seraphineCasts)
            {
                if (cast.Actor!=actor || cast.Skill!=actor.ResolvedSkill) continue;
                float age=cast.Age;
                if (cast.Skill=="steel_break" && age<.5f)
                { row=_tab.Class("seraphine").atk_row; frame=Mathf.Min(6,2+(int)(age*10)); return true; }
                if (cast.Skill!="iron_chains" || age>=.82f) continue;
                if (age<IronChainsEffect.PullTime) { row=3; frame=Mathf.Min(2,(int)(age/.065f)); }
                else if (age<.58f) { row=4; frame=Mathf.Min(2,(int)((age-IronChainsEffect.PullTime)/.065f)); }
                else { row=6; frame=Mathf.Min(2,(int)((age-.58f)/.08f)); }
                return true;
            }
            return false;
        }

        void TickSeraphineSkills(float dt)
        {
            for (int i=_seraphineCasts.Count-1;i>=0;i--)
            {
                var cast=_seraphineCasts[i];
                float duration=cast.Skill=="steel_break" ? .5f : IronChainsEffect.Duration;
                if (cast.Root==null || cast.Actor.Root==null || !cast.Actor.Alive || cast.Age>=duration)
                { RemoveSeraphineCast(i); continue; }
                float pose=2+cast.Age*10;
                cast.Slash?.Show((int)pose,pose-(int)pose,true);
                foreach (var target in cast.Targets)
                {
                    if (target.Actor.Root==null) { target.Chain?.Hide(); target.Impact?.Hide(); continue; }
                    target.Chain?.Sample(cast.Age,cast.Origin,target.Actor.Body.bounds.center,_cam);
                    float hitAge=cast.Age-(cast.Skill=="iron_chains" ? IronChainsEffect.CatchTime : 0);
                    target.Impact?.Sample(hitAge,target.Contact,_cam.transform.right,_cam);
                    if (cast.Skill=="steel_break" && target.Hit && cast.Age<FlashDur)
                    {
                        var body=target.Actor.Body;
                        var block=target.Actor.Mpb ??= new MaterialPropertyBlock();
                        body.GetPropertyBlock(block);
                        block.SetColor("_FlashColor",new Color(1.3f,.42f,.36f,1));
                        block.SetFloat("_Flash",Mathf.Pow(1-cast.Age/FlashDur,FlashDecay)*.65f);
                        body.SetPropertyBlock(block);
                    }
                }
                cast.Age+=dt;
            }
        }

        void RemoveSeraphineCast(int index)
        {
            var cast=_seraphineCasts[index];
            foreach (var target in cast.Targets)
                if (target.Actor.Body!=null)
                    ActorHitFlash.Apply(target.Actor.Body,ref target.Actor.Mpb,target.Actor.FlashT);
            if (cast.Root!=null) { cast.Root.SetActive(false); Kill(cast.Root); }
            _seraphineCasts.RemoveAt(index);
        }

        void ClearSeraphineSkills()
        {
            for (int i=_seraphineCasts.Count-1;i>=0;i--) RemoveSeraphineCast(i);
            _seraphinePlayed.Clear(); _seraphineTick=-1; _seraphineBaselineTick=-1;
        }
    }
}
