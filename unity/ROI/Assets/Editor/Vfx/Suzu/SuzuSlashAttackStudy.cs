using System;
using System.Collections.Generic;
using Roi.Arena;
using UnityEngine;
using Object=UnityEngine.Object;

namespace Roi.Editor.Vfx
{
    internal sealed class SuzuSlashAttackStudy : IDisposable
    {
        public enum Style { Vertical, Diagonal, Horizontal, Circular, Whirlwind }
        readonly SuzuSunderFixture _fixture;
        readonly Style _style;
        readonly List<Pulse> _pulses=new();
        public readonly List<SuzuSlashBaseEffect> Effects=new();
        struct Pulse { public float Start,Speed; }

        public SuzuSlashAttackStudy(SuzuSunderFixture fixture,Style style)
        {
            _fixture=fixture; _style=style;
            switch (style)
            {
                case Style.Vertical:
                    Add(new Vector3(-.52f,1.12f,0),Quaternion.Euler(-90,0,0),new Vector3(.5f,1,.62f),.2f,1,.36f,1.4f);
                    break;
                case Style.Diagonal:
                    Add(new Vector3(-.48f,1.02f,0),Quaternion.Euler(0,0,-42)*Quaternion.Euler(-90,0,0),
                        new Vector3(.5f,1,.68f),.2f,1,.36f,1.4f);
                    break;
                case Style.Horizontal:
                    Add(new Vector3(-.35f,.58f,0),Quaternion.identity,new Vector3(.87f,1,.7f),.4f,1,.36f,1.4f);
                    break;
                case Style.Circular:
                    Ring(.46f,1,.36f,1.55f,0);
                    break;
                case Style.Whirlwind:
                    Ring(.28f,.88f,.32f,2.4f,0);
                    Ring(.65f,1.04f,.62f,2.4f,35);
                    Ring(1.02f,.82f,.92f,2.4f,70);
                    break;
            }
        }

        void Ring(float height,float scale,float start,float speed,float yaw)
        {
            // Two fixed 180-degree windows cover a full orbit; each remains a one-shot UV stroke.
            Add(Vector3.up*height,Quaternion.Euler(0,yaw,0),Vector3.one*scale,0,2f/3,start,speed);
            Add(Vector3.up*height,Quaternion.Euler(0,yaw+180,0),Vector3.one*scale,0,2f/3,start+.12f,speed);
        }
        void Add(Vector3 offset,Quaternion rotation,Vector3 scale,float from,float to,float start,float speed)
        {
            var fx=Effects.Count==0 ? _fixture.BaseEffect
                : SuzuSlashBaseEffect.Create(_fixture.Body,_fixture.Actor,_fixture.Height);
            SuzuSunderFixture.Require(fx!=null,"attack-study resources missing");
            fx.SetPlacement(offset,rotation,scale,new Vector2(from,to),alignBackU:true);
            Effects.Add(fx); _pulses.Add(new Pulse { Start=start,Speed=speed });
        }
        public void Sample(float age,bool visible=true)
        {
            float poseAge=age;
            if (_style==Style.Whirlwind && age>=.62f && age<1.28f)
                poseAge=.36f+Mathf.Repeat(age-.62f,.3f);
            _fixture.Sample(SuzuSunderFixture.Start+poseAge);
            for (int i=0;i<Effects.Count;i++)
            {
                var pulse=_pulses[i];
                float localAge=.36f+(age-pulse.Start)*pulse.Speed;
                Effects[i].Sample(localAge,visible && age>=pulse.Start);
            }
        }
        public void Hide() { foreach (var fx in Effects) fx.Hide(); }
        public void Dispose()
        {
            for (int i=1;i<Effects.Count;i++) if (Effects[i]!=null) Object.DestroyImmediate(Effects[i].gameObject);
            _fixture.BaseEffect.SetPlacement(Vector3.up*.46f,Quaternion.identity,Vector3.one,new Vector2(0,1));
            _fixture.BaseEffect.Hide();
        }
    }
}
