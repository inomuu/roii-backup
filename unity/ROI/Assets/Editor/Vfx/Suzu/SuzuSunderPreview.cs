using System;
using System.IO;
using Roi.Editor.Vfx;
using UnityEditor;
using UnityEngine;
using Object=UnityEngine.Object;

public static class SuzuSunderPreview
{
    [MenuItem("ROI/Art/Capture Suzu Sunder")]
    public static void Capture()
    {
        if (EditorApplication.isPlaying) throw new InvalidOperationException("Capture outside play mode.");
        const string output="/tmp/roi-suzu-sunder"; Directory.CreateDirectory(output);
        using (var fixture=new SuzuSunderFixture())
            foreach (var size in new[] { new Vector2Int(1280,720),new Vector2Int(540,960) })
                CaptureViewport(fixture,size,output);
        Debug.Log("SUZU SUNDER CAPTURE PASS: "+output);
    }
    static void CaptureViewport(SuzuSunderFixture fixture,Vector2Int size,string output)
    {
        string name=size.x==1280 ? "landscape" : "portrait";
        fixture.FrameCamera(size); var camera=fixture.Camera;
        var target=new RenderTexture(size.x,size.y,24,RenderTextureFormat.ARGBHalf) { antiAliasing=4 };
        var pixels=new Texture2D(size.x,size.y,TextureFormat.RGB24,false);
        camera.targetTexture=target;
        try
        {
            fixture.Sample(0); camera.Render(); camera.Render();
            for (int i=0;i<SuzuSunderFixture.Frames;i++)
            {
                fixture.Sample(i/(float)SuzuSunderFixture.Fps);
                Read("motion-"+name+"-"+i.ToString("D3"));
            }
            fixture.Sample(SuzuSunderFixture.Start+.52f); Read("sunder-"+name);
            SuzuSunderChecks.Verify(fixture,pixels,Read,name);
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
