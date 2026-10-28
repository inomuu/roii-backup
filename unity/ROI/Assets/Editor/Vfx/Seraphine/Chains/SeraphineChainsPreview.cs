using System;
using System.IO;
using Roi.Editor.Vfx;
using UnityEditor;
using UnityEngine;
using Object=UnityEngine.Object;

public static class SeraphineChainsPreview
{
    [MenuItem("ROI/Art/Capture Seraphine Iron Chains")]
    public static void Capture() => Run(false);

    [MenuItem("ROI/Art/Capture Seraphine Iron Chains Two Targets")]
    public static void CaptureTwoTargets() => Run(true);

    static void Run(bool twoTargets)
    {
        if (EditorApplication.isPlaying) throw new InvalidOperationException("Capture outside play mode.");
        string output=twoTargets ? "/tmp/roi-seraphine-chains-two-targets" : "/tmp/roi-seraphine-chains";
        Directory.CreateDirectory(output);
        using (var fixture=new SeraphineChainsFixture(twoTargets))
            foreach (var size in new[] { new Vector2Int(1280,720),new Vector2Int(540,960) })
                CaptureViewport(fixture,size,output);
        Debug.Log("IRON CHAINS CAPTURE PASS: "+output);
    }

    static void CaptureViewport(SeraphineChainsFixture fixture,Vector2Int size,string output)
    {
        string name=size.x==1280 ? "landscape" : "portrait";
        fixture.FrameCamera(size);
        var camera=fixture.Camera;
        var target=new RenderTexture(size.x,size.y,24,RenderTextureFormat.ARGBHalf) { antiAliasing=4 };
        var pixels=new Texture2D(size.x,size.y,TextureFormat.RGB24,false);
        camera.targetTexture=target;
        try
        {
            fixture.Sample(0); camera.Render(); camera.Render();
            for (int i=0;i<SeraphineChainsFixture.FrameCount;i++)
            {
                fixture.Sample(i/(float)SeraphineChainsFixture.FramesPerSecond);
                Read("motion-"+name+"-"+i.ToString("D3"));
            }
            SeraphineChainsChecks.Verify(fixture,pixels,Read,name);
        }
        finally { camera.targetTexture=null; Object.DestroyImmediate(target); Object.DestroyImmediate(pixels); }

        void Read(string file)
        {
            var previous=RenderTexture.active;
            try
            {
                camera.Render(); RenderTexture.active=target;
                pixels.ReadPixels(new Rect(0,0,size.x,size.y),0,0); pixels.Apply();
                File.WriteAllBytes(Path.Combine(output,file+".png"),pixels.EncodeToPNG());
            }
            finally { RenderTexture.active=previous; }
        }
    }
}
