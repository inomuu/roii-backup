using System;
using Roi.Arena;
using UnityEditor;
using UnityEngine;
using UnityEngine.Rendering.Universal;
using static Roi.Editor.Vfx.SuzuSunderFixture;

namespace Roi.Editor.Vfx
{
    internal static class SuzuSunderChecks
    {
        public static void Verify(SuzuSunderFixture f,Texture2D pixels,Action<string> read,string name)
        {
            var camera=f.Camera; var data=camera.GetUniversalAdditionalCameraData();
            bool post=data.renderPostProcessing; int mask=camera.cullingMask;
            var clear=camera.clearFlags; var background=camera.backgroundColor;
            var renderers=f.Effect.GetComponentsInChildren<Renderer>(true);
            var layers=new int[renderers.Length];
            int used=0;
            foreach (var root in camera.scene.GetRootGameObjects())
                foreach (var t in root.GetComponentsInChildren<Transform>(true)) used|=1<<t.gameObject.layer;
            int isolated=31;
            while (isolated>=8 && (used & (1<<isolated))!=0) isolated--;
            Require(isolated>=8,"no free test layer");
            try
            {
                for (int i=0;i<renderers.Length;i++)
                { layers[i]=renderers[i].gameObject.layer; renderers[i].gameObject.layer=isolated; }
                camera.cullingMask=1<<isolated; camera.clearFlags=CameraClearFlags.SolidColor;
                camera.backgroundColor=Color.black; data.renderPostProcessing=false;
                f.Sample(0); read("empty-"+name); var empty=pixels.GetPixels32();
                CheckUvTravel(f,pixels,read,empty,name);
                f.Sample(Start+.55f);
                foreach (var r in renderers) r.enabled=false;
                foreach (var r in renderers)
                {
                    Require(!ShaderUtil.ShaderHasError(r.sharedMaterial.shader),"shader error: "+r.name);
                    r.enabled=true; read("layer-"+r.name.Replace(' ','-')+"-"+name);
                    long energy=Energy(empty,pixels.GetPixels32()); Require(energy>100,"invisible layer: "+r.name);
                    if (r.name=="Sunder textured flame" || r.name=="Sunder hot core") CheckFireMask(r,pixels,read,empty,name);
                    r.enabled=false; read("hidden-"+name);
                    Reject(()=>Require(Energy(empty,pixels.GetPixels32())>100,"hidden layer"));
                    Debug.Log("SUZU SUNDER LAYER: "+name+" "+r.name+" energy="+energy+"; hidden-layer sabotage caught");
                }
                f.Sample(Start+.55f); read("peak-"+name); var peak=pixels.GetPixels32();
                Palette(peak,out int warm,out int white);
                f.Effect.Hide(); read("palette-hidden-"+name);
                Reject(()=>Palette(pixels.GetPixels32(),out _,out _));
                long previous=long.MaxValue;
                float previousAge=0;
                foreach (float age in new[] { 1.30f,1.42f,1.54f,1.62f })
                {
                    f.Sample(Start+age); read("tail-"+age.ToString("F2",System.Globalization.CultureInfo.InvariantCulture)+"-"+name);
                    long energy=Energy(empty,pixels.GetPixels32());
                    Require(energy>0 && energy<previous*.95,"tail did not dim smoothly");
                    if (previous!=long.MaxValue)
                    {
                        f.Sample(Start+previousAge); read("tail-frozen-"+name);
                        Reject(()=>Require(Energy(empty,pixels.GetPixels32())<previous*.95,"frozen tail"));
                    }
                    previous=energy; previousAge=age;
                    Debug.Log("SUZU SUNDER TAIL: "+name+" age="+age+" energy="+energy);
                }
                // The last few HDR values can quantize to black in the RGB24 capture before lifetime ends.
                f.Sample(Start+1.67f); read("tail-near-cutoff-"+name);
                Require(Energy(empty,pixels.GetPixels32())<Energy(empty,peak)*.01,"visible pop at lifetime cutoff");
                f.Sample(Start+SuzuSunderEffect.Duration+.01f); read("expired-"+name);
                Require(Energy(empty,pixels.GetPixels32())==0,"expired effect left pixels");
                f.Sample(Start+.55f); read("expiry-frozen-"+name);
                Reject(()=>Require(Energy(empty,pixels.GetPixels32())==0,"frozen expiry"));
                f.Sample(Start+.55f); f.Effect.Sample(.55f,false); read("cancelled-"+name);
                Require(Energy(empty,pixels.GetPixels32())==0,"cancelled effect left pixels");
                f.Sample(Start+.55f); read("restart-"+name);
                Require(Energy(peak,pixels.GetPixels32())==0,"restart is not repeatable");
                f.Effect.enabled=false; f.Sample(Start+.55f); read("disabled-"+name);
                Require(Energy(empty,pixels.GetPixels32())==0,"disabled sampling left pixels");
                f.Effect.enabled=true;
                Debug.Log("SUZU SUNDER: "+name+" warm="+warm+" white="+white+"; expiry/cancel/restart/disabled passed");
                f.Effect.Hide();
                foreach (var impact in f.Impacts)
                {
                    var ir=impact.GetComponentsInChildren<Renderer>(true);
                    var old=new int[ir.Length];
                    try
                    {
                        for (int i=0;i<ir.Length;i++) { old[i]=ir[i].gameObject.layer; ir[i].gameObject.layer=isolated; }
                        f.Sample(Start+.54f); f.Effect.Hide();
#if UNITY_6000_6_OR_NEWER
                        read("impact-"+impact.GetEntityId()+"-"+name);
#else
                        read("impact-"+impact.GetInstanceID()+"-"+name);
#endif
                        long energy=Energy(empty,pixels.GetPixels32()); Require(energy>500,"target impact invisible");
                        impact.Hide(); read("impact-hidden-"+name);
                        Reject(()=>Require(Energy(empty,pixels.GetPixels32())>500,"hidden impact"));
                        Debug.Log("SUZU SUNDER IMPACT: "+name+" energy="+energy+"; hidden-impact sabotage caught");
                    }
                    finally { for (int i=0;i<ir.Length;i++) ir[i].gameObject.layer=old[i]; }
                }
            }
            finally
            {
                f.Effect.enabled=true; f.Sample(0);
                for (int i=0;i<renderers.Length;i++) renderers[i].gameObject.layer=layers[i];
                camera.cullingMask=mask; camera.clearFlags=clear; camera.backgroundColor=background; data.renderPostProcessing=post;
            }
        }
        static void CheckUvTravel(SuzuSunderFixture f,Texture2D pixels,Action<string> read,Color32[] empty,string name)
        {
            var filters=Array.FindAll(f.Effect.GetComponentsInChildren<MeshFilter>(),m=>m.name!="Sunder foot glow");
            var asset=Resources.Load<Mesh>("Effects/Suzu/SuzuSlashBaseMesh");
            var vertices=asset.vertices;
            f.Sample(Start+.55f);
            var matrices=Array.ConvertAll(filters,m=>m.transform.localToWorldMatrix);
            foreach (float age in new[] { .4f,.63f,.82f,1.05f,1.18f,1.5f })
            {
                f.Sample(Start+age);
                CheckStatic();
            }
            var tr=filters[0].transform; var rotation=tr.localRotation;
            try { tr.localRotation=Quaternion.Euler(0,10,0); Reject(CheckStatic); }
            finally { tr.localRotation=rotation; }
            var core=Array.Find(filters,m=>m.name=="Sunder hot core").GetComponent<Renderer>();
            var block=new MaterialPropertyBlock();
            f.Sample(Start+.68f); core.GetPropertyBlock(block); float first=block.GetFloat("_Phase");
            foreach (var r in f.Effect.GetComponentsInChildren<Renderer>()) r.enabled=r==core;
            read("uv-first-"+name); var early=pixels.GetPixels32();
            f.Sample(Start+.93f); core.GetPropertyBlock(block); float next=block.GetFloat("_Phase");
            Require(next>first+.2f,"UV clock did not advance");
            foreach (var r in f.Effect.GetComponentsInChildren<Renderer>()) r.enabled=r==core;
            read("uv-second-"+name);
            Require(Energy(early,pixels.GetPixels32())>5000,"UV travel did not move rendered silhouette");
            Require(Energy(empty,pixels.GetPixels32())>5000,"second pass invisible");
            block.SetFloat("_Phase",first); core.SetPropertyBlock(block); read("uv-frozen-"+name);
            Reject(()=>Require(Energy(early,pixels.GetPixels32())>5000,"frozen UV travel"));
            foreach (var impact in f.Impacts)
                foreach (var r in impact.GetComponentsInChildren<Renderer>()) Require(!r.enabled,"second pass replayed contact");
            Debug.Log("SUZU SUNDER UV: "+name+" static imported mesh; UV travel="+first+" -> "+next
                +"; rotated-carrier/frozen-UV sabotage caught; no repeated impact");

            void CheckStatic()
            {
                for (int i=0;i<filters.Length;i++)
                {
                    Require(filters[i].sharedMesh==asset,"not using imported base mesh");
                    Require(filters[i].transform.localToWorldMatrix==matrices[i],"carrier moved during sweep");
                }
                var actual=asset.vertices;
                Require(actual.Length==vertices.Length,"carrier topology changed");
                for (int i=0;i<actual.Length;i++) Require(actual[i]==vertices[i],"carrier deformed");
            }
        }
        static void CheckFireMask(Renderer renderer,Texture2D pixels,Action<string> read,Color32[] empty,string name)
        {
            var material=renderer.sharedMaterial; var mask=material.GetTexture("_MaskTex");
            var original=new MaterialPropertyBlock(); renderer.GetPropertyBlock(original);
            var block=new MaterialPropertyBlock(); renderer.GetPropertyBlock(block);
            var masked=pixels.GetPixels32(); long early=Energy(empty,masked);
            try
            {
                material.SetTexture("_MaskTex",Texture2D.whiteTexture); read("fire-unmasked-"+name);
                var solid=pixels.GetPixels32(); long difference=Energy(masked,solid);
                Require(difference>1000,"fire mask has no rendered silhouette contribution");
                read("fire-mask-sabotage-"+name);
                Reject(()=>Require(Energy(solid,pixels.GetPixels32())>1000,"white mask"));
                material.SetTexture("_MaskTex",mask);
                block.SetFloat("_Erosion",.9f); renderer.SetPropertyBlock(block); read("fire-eroded-"+name);
                long late=Energy(empty,pixels.GetPixels32()); Require(late<early*.65,"fire erosion did not remove the tail");
                renderer.SetPropertyBlock(original); read("fire-erosion-frozen-"+name);
                Reject(()=>Require(Energy(empty,pixels.GetPixels32())<early*.65,"frozen fire erosion"));
                Debug.Log("SUZU SUNDER FIRE MASK: "+name+" contribution="+difference+" erosion="+early+" -> "+late
                    +"; white-mask and frozen-erosion sabotage caught");
            }
            finally { material.SetTexture("_MaskTex",mask); renderer.SetPropertyBlock(original); }
        }
        static void Palette(Color32[] pixels,out int warm,out int white)
        {
            warm=0; white=0;
            foreach (var c in pixels)
            {
                if (c.r>120 && c.r>c.g*1.3f && c.g>c.b*1.3f) warm++;
                if (c.r>220 && c.g>210 && c.b>190) white++;
            }
            Require(warm>50 && white>50,"missing warm edge or white core");
        }
        static long Energy(Color32[] a,Color32[] b)
        {
            long sum=0;
            for (int i=0;i<a.Length;i++) sum+=Math.Abs(a[i].r-b[i].r)+Math.Abs(a[i].g-b[i].g)+Math.Abs(a[i].b-b[i].b);
            return sum;
        }
        static void Reject(Action assertion)
        {
            bool caught=false; try { assertion(); } catch (InvalidOperationException) { caught=true; }
            Require(caught,"guard survived sabotage");
        }
    }
}
