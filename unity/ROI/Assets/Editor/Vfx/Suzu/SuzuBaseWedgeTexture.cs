using System;
using System.Collections.Generic;
using System.IO;
using UnityEditor;
using UnityEngine;

namespace Roi.Editor.Vfx
{
    internal static class SuzuBaseWedgeTexture
    {
        const string Path="Assets/Resources/Effects/Suzu/SuzuBaseWedgeMask-v3.png";

        public static void Build()
        {
            const int size=512, samples=4, side=size*samples;
            var outline=new List<Vector2> { new Vector2(0,1) };
            Curve(new Vector2(.035f,.995f),new Vector2(.07f,.97f),new Vector2(.105f,.945f));
            Curve(new Vector2(.15f,.912f),new Vector2(.215f,.864f),new Vector2(.203f,.824f));
            Curve(new Vector2(.196f,.798f),new Vector2(.269f,.741f),new Vector2(.32f,.686f));
            Curve(new Vector2(.36f,.645f),new Vector2(.397f,.633f),new Vector2(.462f,.607f));
            Curve(new Vector2(.448f,.588f),new Vector2(.407f,.565f),new Vector2(.42f,.53f));
            Curve(new Vector2(.469f,.471f),new Vector2(.437f,.401f),new Vector2(.37f,.355f));
            Curve(new Vector2(.43f,.336f),new Vector2(.533f,.304f),new Vector2(.589f,.276f));
            Curve(new Vector2(.648f,.246f),new Vector2(.644f,.199f),new Vector2(.712f,.18f));
            Curve(new Vector2(.759f,.166f),new Vector2(.875f,.169f),new Vector2(.858f,.119f));
            Curve(new Vector2(.849f,.092f),new Vector2(.883f,.046f),new Vector2(.91f,0));
            outline.Add(new Vector2(1,0)); outline.Add(Vector2.one);
            CheckContour(outline);
            bool caught=false;
            try { CheckContour(new List<Vector2> { Vector2.zero,Vector2.one,Vector2.up,Vector2.right }); }
            catch (InvalidOperationException) { caught=true; }
            if (!caught) throw new InvalidOperationException("Contour guard survived crossing sabotage.");

            // Scan-convert the 2D contour so backwards-pointing overhangs are not flattened into a width curve.
            var coverage=new byte[size*size]; var crossings=new List<float>();
            for (int sy=0;sy<side;sy++)
            {
                float v=((sy+.5f)/side-.02f)/.96f; crossings.Clear();
                for (int i=0,j=outline.Count-1;i<outline.Count;j=i++)
                {
                    var a=outline[j]; var b=outline[i];
                    if ((a.y>v)==(b.y>v)) continue;
                    crossings.Add(a.x+(v-a.y)*(b.x-a.x)/(b.y-a.y));
                }
                crossings.Sort();
                if (crossings.Count%2!=0) throw new InvalidOperationException("Wedge outline has an unmatched crossing.");
                for (int i=0;i<crossings.Count;i+=2)
                {
                    int first=Mathf.Clamp(Mathf.CeilToInt((.02f+.96f*crossings[i])*side-.5f),0,side);
                    int last=Mathf.Clamp(Mathf.CeilToInt((.02f+.96f*crossings[i+1])*side-.5f),0,side);
                    for (int sx=first;sx<last;sx++) coverage[(sy/samples)*size+sx/samples]++;
                }
            }
            var pixels=new Color32[size*size];
            for (int i=0;i<pixels.Length;i++)
            {
                byte value=(byte)Mathf.RoundToInt(255f*coverage[i]/(samples*samples));
                pixels[i]=new Color32(value,value,value,255);
            }
            var texture=new Texture2D(size,size,TextureFormat.RGBA32,false,true);
            try
            {
                texture.SetPixels32(pixels); texture.Apply();
                var bytes=texture.EncodeToPNG();
                if (!File.Exists(Path) || !System.Linq.Enumerable.SequenceEqual(File.ReadAllBytes(Path),bytes))
                    File.WriteAllBytes(Path,bytes);
                AssetDatabase.ImportAsset(Path,ImportAssetOptions.ForceSynchronousImport);
                var importer=(TextureImporter)AssetImporter.GetAtPath(Path);
                importer.sRGBTexture=false; importer.alphaSource=TextureImporterAlphaSource.None;
                importer.mipmapEnabled=true; importer.wrapMode=TextureWrapMode.Clamp;
                importer.filterMode=FilterMode.Trilinear; importer.textureCompression=TextureImporterCompression.Uncompressed;
                importer.maxTextureSize=size;
                importer.userData="Code-authored asymmetric wedge: broad shallow ripples and one dominant swept cusp, no hooked saw teeth. White coverage; not an AI image or copied reference texture.";
                importer.SaveAndReimport();
            }
            finally { UnityEngine.Object.DestroyImmediate(texture); }

            void Curve(Vector2 c1,Vector2 c2,Vector2 end)
            {
                var start=outline[outline.Count-1];
                for (int i=1;i<=16;i++)
                {
                    float t=i/16f,s=1-t;
                    outline.Add(s*s*s*start+3*s*s*t*c1+3*s*t*t*c2+t*t*t*end);
                }
            }
            Debug.Log("SUZU WEDGE V3: shallow asymmetric contour verified; crossing sabotage caught.");
        }

        static void CheckContour(List<Vector2> outline)
        {
            for (int i=0;i<outline.Count;i++) for (int j=i+2;j<outline.Count;j++)
            {
                if (i==0 && j==outline.Count-1) continue;
                var a=outline[i]; var b=outline[(i+1)%outline.Count];
                var c=outline[j]; var d=outline[(j+1)%outline.Count];
                if (Side(a,b,c)*Side(a,b,d)<0 && Side(c,d,a)*Side(c,d,b)<0)
                    throw new InvalidOperationException("Wedge contour crosses itself.");
            }
            float Side(Vector2 a,Vector2 b,Vector2 c)
                => (b.x-a.x)*(c.y-a.y)-(b.y-a.y)*(c.x-a.x);
        }
    }
}
