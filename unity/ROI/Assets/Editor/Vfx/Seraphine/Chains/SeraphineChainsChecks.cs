using System;
using Roi.Arena;
using UnityEditor;
using UnityEngine;
using UnityEngine.Rendering.Universal;

namespace Roi.Editor.Vfx
{
    internal static class SeraphineChainsChecks
    {
        public static void Verify(SeraphineChainsFixture fixture,Texture2D pixels,Action<string> Read,string name)
        {
            const float Start=SeraphineChainsFixture.Start;
            var camera=fixture.Camera;
            var scene=fixture.Scene;
            var body=fixture.Body;
            var target=fixture.Targets[0].Root;
            var chain=fixture.Targets[0].Chain;
            var secondChain=fixture.Targets.Length>1 ? fixture.Targets[1].Chain : null;
            float height=fixture.Height;
            var right=fixture.Right;
            var chainRenderer=chain.GetComponent<MeshRenderer>();
            var chainRenderers=chain.GetComponentsInChildren<MeshRenderer>();
            var inkRenderer=Array.Find(chainRenderers,r=>r.name=="Chain ink silhouette");
            Require(!ShaderUtil.ShaderHasError(chainRenderer.sharedMaterial.shader),"chain shader compile error");
            void Sample(float time) => fixture.Sample(time);
            Vector3 Origin() => fixture.Origin;
            Vector3 Contact() => fixture.Targets[0].Contact;
            Vector3 SecondContact() => fixture.Targets[1].Contact;
            Sample(Start+.32f); Read("tension-"+name);
            var heldReach=body.sprite;
            Sample(Start+IronChainsEffect.PullTime+.04f); Read("pull-pose-"+name);
            Require(Mathf.Abs(body.sprite.rect.y-fixture.PullRowY)<.01f,"yank did not use retract row");
            body.sprite=heldReach;
            Reject(()=>Require(Mathf.Abs(body.sprite.rect.y-fixture.PullRowY)<.01f,"frozen reach pose"));
            Sample(Start+.32f);
            if (secondChain!=null)
            {
                Require(Vector3.Distance(secondChain.Tip,SecondContact())<height*.001f,"second chain misses its target");
                secondChain.Sample(.32f,Origin(),Contact(),camera);
                Reject(()=>Require(Vector3.Distance(secondChain.Tip,SecondContact())<height*.001f,"both chains attached to first target"));
                Sample(Start+.32f);
            }
            int mask=camera.cullingMask,oldLayer=chain.gameObject.layer,used=0;
            foreach (var root in scene.GetRootGameObjects())
                foreach (var t in root.GetComponentsInChildren<Transform>(true)) used|=1<<t.gameObject.layer;
            int layer=31; while (layer>=8 && (used&(1<<layer))!=0) layer--;
            Require(layer>=8,"no isolated preview layer");
            var data=camera.GetUniversalAdditionalCameraData(); bool post=data.renderPostProcessing;
            var clear=camera.clearFlags; var background=camera.backgroundColor;
            try
            {
                foreach (var r in chainRenderers) r.gameObject.layer=layer;
                camera.cullingMask=1<<layer;
                camera.clearFlags=CameraClearFlags.SolidColor; camera.backgroundColor=Color.black;
                data.renderPostProcessing=false;
                chain.Hide(); Read("empty-"+name); var empty=pixels.GetPixels32();
                Sample(Start+.32f); Read("links-"+name); int contribution=Difference(empty,pixels.GetPixels32());
                Require(contribution>120,"links invisible");
                foreach (var r in chainRenderers) r.enabled=false;
                Read("hidden-"+name);
                Reject(()=>Require(Difference(empty,pixels.GetPixels32())>120,"hidden renderer"));
                camera.backgroundColor=new Color(.65f,.65f,.65f);
                Sample(Start+.32f); inkRenderer.enabled=false; Read("without-ink-"+name); var noInk=pixels.GetPixels32();
                inkRenderer.enabled=true; Read("with-ink-"+name);
                int inkPixels=Difference(noInk,pixels.GetPixels32());
                Require(inkPixels>50,"black ink outline invisible");
                inkRenderer.enabled=false; Read("ink-sabotage-"+name);
                Reject(()=>Require(Difference(noInk,pixels.GetPixels32())>50,"hidden ink outline"));
                Debug.Log("IRON CHAINS INK: "+name+" outline pixels="+inkPixels+"; hidden-outline sabotage caught");
                camera.backgroundColor=Color.black;
                Sample(Start+IronChainsEffect.PullTime+.04f);
                foreach (var r in chainRenderers) r.enabled=false;
                Read("aura-empty-"+name); var auraEmpty=pixels.GetPixels32();
                foreach (var r in chainRenderers)
                {
                    if (r==chainRenderer || r==inkRenderer) continue;
                    r.enabled=true; Read("aura-"+r.name.Replace(' ','-')+"-"+name);
                    int contributionAura=Difference(auraEmpty,pixels.GetPixels32());
                    Require(contributionAura>30,"caster aura layer invisible: "+r.name);
                    r.enabled=false; Read("aura-hidden-"+name);
                    Reject(()=>Require(Difference(auraEmpty,pixels.GetPixels32())>30,"hidden aura layer"));
                    Debug.Log("IRON CHAINS AURA: "+name+" "+r.name+" pixels="+contributionAura+"; hidden-layer sabotage caught");
                }
                Sample(Start+.65f); chainRenderer.enabled=false; inkRenderer.enabled=false;
                Read("aura-fade-early-"+name); int auraEarly=Energy(auraEmpty,pixels.GetPixels32());
                Sample(Start+.95f); chainRenderer.enabled=false; inkRenderer.enabled=false;
                Read("aura-fade-late-"+name); int auraLate=Energy(auraEmpty,pixels.GetPixels32());
                Require(auraLate>0 && auraLate<auraEarly*.4f,"caster aura does not fade gradually");
                Sample(Start+.65f); chainRenderer.enabled=false; inkRenderer.enabled=false;
                Read("aura-fade-sabotage-"+name);
                Reject(()=>Require(Energy(auraEmpty,pixels.GetPixels32())<auraEarly*.4f,"frozen aura clock"));
                Debug.Log("IRON CHAINS AURA FADE: "+name+" energy="+auraEarly+" -> "+auraLate+"; frozen-clock sabotage caught");
                Sample(Start+.86f);
                foreach (var r in chainRenderers) if (r!=chainRenderer && r!=inkRenderer) r.enabled=false;
                Read("fade-early-"+name); int early=Energy(empty,pixels.GetPixels32());
                Sample(Start+1.12f);
                foreach (var r in chainRenderers) if (r!=chainRenderer && r!=inkRenderer) r.enabled=false;
                Read("fade-late-"+name); int late=Energy(empty,pixels.GetPixels32());
                Require(late<early*.4f && late>0,"chain does not fade gradually");
                chainRenderer.sharedMaterial.SetFloat("_Opacity",1); Read("fade-sabotage-"+name);
                Reject(()=>Require(Energy(empty,pixels.GetPixels32())<early*.4f,"opaque fade sabotage"));
                Sample(Start+IronChainsEffect.Duration); Read("expired-"+name);
                Require(Energy(empty,pixels.GetPixels32())==0,"chain visible after expiry");
                Sample(Start+.32f); Read("expiry-sabotage-"+name);
                Reject(()=>Require(Energy(empty,pixels.GetPixels32())==0,"expired clock sabotage"));
                Debug.Log("IRON CHAINS: "+name+" visible pixels="+contribution+" fade energy="+early+" -> "+late+"; visibility/fade/expiry sabotages caught");
                if (secondChain!=null)
                {
                    var renderers=secondChain.GetComponentsInChildren<Renderer>();
                    var layers=Array.ConvertAll(renderers,r=>r.gameObject.layer);
                    try
                    {
                        foreach (var r in renderers) r.gameObject.layer=layer;
                        chain.Hide(); secondChain.Hide(); Read("second-empty-"+name); var secondEmpty=pixels.GetPixels32();
                        Sample(Start+.32f); chain.Hide(); Read("second-links-"+name);
                        int secondPixels=Difference(secondEmpty,pixels.GetPixels32());
                        Require(secondPixels>120,"second chain invisible");
                        secondChain.Hide(); Read("second-hidden-"+name);
                        Reject(()=>Require(Difference(secondEmpty,pixels.GetPixels32())>120,"hidden second chain"));
                        Sample(Start+IronChainsEffect.Duration); Read("second-expired-"+name);
                        Require(Energy(secondEmpty,pixels.GetPixels32())==0,"second chain outlived cast");
                        Sample(Start+.32f); chain.Hide(); Read("second-expiry-sabotage-"+name);
                        Reject(()=>Require(Energy(secondEmpty,pixels.GetPixels32())==0,"second expiry clock frozen"));
                        Debug.Log("IRON CHAINS TWO TARGETS: "+name+" second pixels="+secondPixels+"; hidden, wrong-target and expiry sabotages caught");
                    }
                    finally { for (int i=0;i<renderers.Length;i++) renderers[i].gameObject.layer=layers[i]; }
                }
            }
            finally
            {
                foreach (var r in chainRenderers) r.gameObject.layer=oldLayer;
                camera.cullingMask=mask; camera.clearFlags=clear;
                camera.backgroundColor=background; data.renderPostProcessing=post;
            }
            Sample(Start+.1f); var earlyTip=chain.Tip;
            Sample(Start+.3f); Require(Vector3.Distance(earlyTip,chain.Tip)>height*.5f,"chain tip not advancing");
            Sample(Start+.1f); Reject(()=>Require(Vector3.Distance(earlyTip,chain.Tip)>height*.5f,"frozen tip clock"));
            Sample(Start+.54f); var contact=Contact();
            Require(Vector3.Distance(chain.Tip,contact)<height*.001f,"chain misses target");
            chain.Sample(.54f,Origin(),contact+right*height,camera);
            Reject(()=>Require(Vector3.Distance(chain.Tip,contact)<height*.001f,"offset endpoint sabotage"));
            Sample(Start+.54f); chain.Sample(.54f,Origin(),contact,camera,false);
            Require(Array.TrueForAll(chainRenderers,r=>!r.enabled) && chain.LinkCount==0,"cancel left chain visible");
            Sample(Start+.54f); Require(chainRenderer.enabled,"restart failed");
            Sample(Start+IronChainsEffect.PullTime); var beforePull=target.transform.position;
            Sample(Start+IronChainsEffect.PullTime+.1f); var afterSnap=target.transform.position;
            Sample(Start+IronChainsEffect.PullTime+.22f); var afterPull=target.transform.position;
            float snapDistance=Vector3.Dot(beforePull-afterSnap,right);
            float settleDistance=Vector3.Dot(afterSnap-afterPull,right);
            Require(snapDistance>height*1.3f && settleDistance<height*.2f,"pull lost its sharp initial snap");
            Sample(Start+IronChainsEffect.PullTime);
            Reject(()=>Require(Vector3.Dot(beforePull-target.transform.position,right)>height*1.3f,"frozen pull clock"));
            Debug.Log("IRON CHAINS YANK: "+name+" initial/settle heights="+snapDistance/height+" / "+settleDistance/height);
            Debug.Log("IRON CHAINS: "+name+" endpoints and tip motion verified; clock sabotage caught; cancel/restart passed");
        }

        static int Difference(Color32[] a,Color32[] b)
        {
            int sum=0; for (int i=0;i<a.Length;i++) if (Math.Abs(a[i].r-b[i].r)+Math.Abs(a[i].g-b[i].g)+Math.Abs(a[i].b-b[i].b)>30) sum++; return sum;
        }
        static int Energy(Color32[] a,Color32[] b)
        {
            int sum=0; for (int i=0;i<a.Length;i++) sum+=Math.Abs(a[i].r-b[i].r)+Math.Abs(a[i].g-b[i].g)+Math.Abs(a[i].b-b[i].b); return sum;
        }
        static void Reject(Action guard)
        {
            bool caught=false; try { guard(); } catch (InvalidOperationException) { caught=true; }
            Require(caught,"guard survived sabotage");
        }
        public static void Require(bool condition,string message) { if (!condition) throw new InvalidOperationException(message); }
    }
}
