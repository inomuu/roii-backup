using System.Collections.Generic;
using System.IO;
using Roi.Editor.Vfx;
using UnityEditor;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.Rendering.Universal;
using UnityEngine.SceneManagement;
using Object=UnityEngine.Object;
using static Roi.Editor.Vfx.SuzuSunderFixture;

public static class SuzuBellMeshPreview
{
    internal const float BelowFeet=.2f;
    internal const float CameraInset=.65f;
    [MenuItem("ROI/Art/Inspect Bell Strike Mesh Placement")]
    public static void Capture() => Capture(null,"vendor");

    [MenuItem("ROI/Art/Compare Bell Rectangle And Quarter Arc")]
    public static void CaptureShapes()
    {
        var rectangle=BuildCarrier(false);
        var quarter=BuildCarrier(true);
        try { Capture(rectangle,"rectangle"); Capture(quarter,"quarter-arc"); }
        finally { Object.DestroyImmediate(rectangle); Object.DestroyImmediate(quarter); }
    }

    [MenuItem("ROI/Art/Compare Bell Quarter And Half Arc")]
    public static void CaptureArcs()
    {
        var quarter=BuildCarrier(true);
        var half=BuildCarrier(true,180);
        try { Capture(quarter,"quarter-arc",true); Capture(half,"half-arc",true); }
        finally { Object.DestroyImmediate(quarter); Object.DestroyImmediate(half); }
    }

    internal static Mesh BuildCarrier(bool curved,float arcDegrees=90)
    {
        Require(arcDegrees==90 || arcDegrees==180,"Bell arc must be a quarter or half circle");
        int segments=curved && arcDegrees==180 ? 32 : 16;
        const int columns=3;
        const float height=2,width=1.35f;
        var vertices=new Vector3[(segments+1)*columns];
        var uv=new Vector2[vertices.Length];
        var triangles=new List<int>();
        for (int row=0;row<=segments;row++)
        {
            float t=row/(float)segments,angle=t*arcDegrees*Mathf.Deg2Rad;
            for (int column=0;column<columns;column++)
            {
                float v=column/(float)(columns-1),radius=height+BelowFeet-width+width*v;
                int index=row*columns+column;
                vertices[index]=curved
                    ? new Vector3(Mathf.Cos(angle)*radius-(height+BelowFeet-width*.5f),Mathf.Sin(angle)*radius-BelowFeet,0)
                    : new Vector3((v-.5f)*width,t*(height+BelowFeet)-BelowFeet,0);
                vertices[index].z=column==1 ? -.025f : 0;
                uv[index]=new Vector2(t,v);
                if (row==segments || column==columns-1) continue;
                triangles.AddRange(new[] { index,index+columns,index+1,index+1,index+columns,index+columns+1 });
            }
        }
        var mesh=new Mesh { name=curved ? "Bell arc "+arcDegrees+" degrees" : "Bell rectangular slash carrier" };
        mesh.vertices=vertices; mesh.uv=uv; mesh.SetTriangles(triangles,0);
        mesh.RecalculateNormals(); mesh.RecalculateBounds();
        Require(Mathf.Abs(mesh.bounds.max.y-height)<.001f && Mathf.Abs(mesh.bounds.min.y+BelowFeet)<.001f,
            "carrier must retain its top and extend below the feet");
        for (int row=0;row<=segments;row++)
            Require(Mathf.Abs(Vector3.Distance(vertices[row*columns],vertices[row*columns+columns-1])-1.35f)<.001f,
                "carrier strip width must be 50 percent wider than 0.9");
        return mesh;
    }

    static void Capture(Mesh custom,string variant,bool longArcView=false)
    {
        Require(!EditorApplication.isPlaying,"inspect outside play mode");
        string output=custom!=null ? "/tmp/roi-bell-mesh-shapes" : "/tmp/roi-bell-mesh";
        Directory.CreateDirectory(output);
        using var f=new SuzuSunderFixture(baseOnly:true);
        var prefab=AssetDatabase.LoadAssetAtPath<GameObject>(
            "Assets/Editor/Vfx/References/MaykerStudio/FreeSlashVFX/Prefabs/Slash Fire VFX.prefab");
        Require(prefab!=null,"Mayker reference missing");
        var source=custom!=null ? custom : prefab.GetComponent<ParticleSystemRenderer>().mesh;
        Require(source!=null,"Mayker source mesh missing");
        var shader=Shader.Find("Universal Render Pipeline/Unlit");
        Require(shader!=null && shader.isSupported,"inspection shader unavailable");
        var fill=new Material(shader);
        var ink=new Material(shader);
        fill.SetColor("_BaseColor",new Color(.72f,.72f,.72f));
        ink.SetColor("_BaseColor",new Color(.025f,.045f,.055f));
        fill.SetFloat("_Cull",(float)CullMode.Off);
        ink.SetFloat("_Cull",(float)CullMode.Off);
        var root=new GameObject("Bell source mesh placement");
        SceneManager.MoveGameObjectToScene(root,f.Camera.scene);
        var surface=root.AddComponent<MeshFilter>(); surface.sharedMesh=source;
        root.AddComponent<MeshRenderer>().sharedMaterial=fill;
        var lines=new Mesh { name="Bell inspection edges" };
        lines.vertices=source.vertices;
        var edges=new HashSet<(int,int)>(); var indices=new List<int>();
        var triangles=source.triangles;
        for (int i=0;i<triangles.Length;i+=3)
            for (int j=0;j<3;j++)
            {
                int a=triangles[i+j],b=triangles[i+(j+1)%3];
                var edge=(Mathf.Min(a,b),Mathf.Max(a,b));
                if (edges.Add(edge)) { indices.Add(a); indices.Add(b); }
            }
        lines.SetIndices(indices,MeshTopology.Lines,0);
        var wire=new GameObject("Wireframe"); wire.transform.SetParent(root.transform,false);
        wire.AddComponent<MeshFilter>().sharedMesh=lines;
        wire.AddComponent<MeshRenderer>().sharedMaterial=ink;
        var clocks=new[] {
            new MaykerFireSlashPreview.CaptureClock(RenderPassEvent.BeforeRenderingShadows),
            new MaykerFireSlashPreview.CaptureClock(RenderPassEvent.BeforeRenderingOpaques),
            new MaykerFireSlashPreview.CaptureClock(RenderPassEvent.BeforeRenderingTransparents)
        };
        RenderPipelineManager.beginCameraRendering+=PinClock;
        try
        {
            f.Sample(Start+.48f); f.BaseEffect.Hide(); f.Effect.Hide();
            f.SetTargetVisible(1,false);
            root.transform.localScale=Vector3.one*(custom!=null ? f.Height
                : f.Height*2/Mathf.Max(source.bounds.size.x,source.bounds.size.y));
            Require(surface.sharedMesh==source,"inspection replaced the source geometry");
            foreach (var size in new[] { new Vector2Int(1280,720),new Vector2Int(540,960) })
            {
                f.FrameCamera(size);
                f.Camera.transform.position-=f.Camera.transform.forward*f.Height*1.8f;
                if (longArcView) f.Camera.transform.position-=f.Camera.transform.right*f.Height*.8f;
                PlaceOverHead();
                string view=size.x>size.y ? "landscape" : "portrait";
                var target=new RenderTexture(size.x,size.y,24,RenderTextureFormat.ARGBHalf) { antiAliasing=4 };
                var pixels=new Texture2D(size.x,size.y,TextureFormat.RGB24,false);
                var previous=RenderTexture.active;
                f.Camera.targetTexture=target;
                try
                {
                    CheckPlacement();
                    var position=root.transform.position;
                    bool caught=false;
                    try
                    {
                        root.transform.position+=f.Camera.transform.right*f.Height*.4f;
                        CheckPlacement();
                    }
                    catch (System.InvalidOperationException) { caught=true; }
                    finally { root.transform.position=position; }
                    Require(caught,"sideways placement survived head-alignment check");
                    var rotation=root.transform.rotation;
                    caught=false;
                    try
                    {
                        root.transform.rotation=f.Camera.transform.rotation*Quaternion.Euler(0,90,0);
                        CheckPlacement();
                    }
                    catch (System.InvalidOperationException) { caught=true; }
                    finally { root.transform.rotation=rotation; }
                    Require(caught,"edge-on carrier survived broad-face check");
                    root.SetActive(false); Read(); var empty=pixels.GetPixels32();
                    root.SetActive(true); Read(); var shown=pixels.GetPixels32();
                    long energy=Difference(empty,shown);
                    Require(energy>10000,"inspection mesh invisible");
                    if (custom!=null)
                    {
                        Vector2 head=f.Camera.WorldToScreenPoint(f.TargetHead(0));
                        Vector2 feet=f.Camera.WorldToScreenPoint(ContactPoint());
                        float halfWidth=(head.y-feet.y)*.25f;
                        int changed=0;
                        for (int y=Mathf.CeilToInt(feet.y);y<Mathf.FloorToInt(head.y);y++)
                            for (int x=Mathf.CeilToInt(head.x-halfWidth);x<Mathf.FloorToInt(head.x+halfWidth);x++)
                            {
                                int index=y*size.x+x;
                                if (System.Math.Abs(empty[index].r-shown[index].r)
                                    +System.Math.Abs(empty[index].g-shown[index].g)
                                    +System.Math.Abs(empty[index].b-shown[index].b)>30) changed++;
                            }
                        Require(changed>100,"slash does not overlap the enemy body region");
                        Debug.Log("BELL BODY OVERLAP: "+variant+" "+view+" pixels="+changed);
                    }
                    string file=custom!=null ? variant+"-"+view : "mesh-"+view;
                    File.WriteAllBytes(Path.Combine(output,file+".png"),pixels.EncodeToPNG());
                    root.SetActive(false); Read();
                    Require(Difference(empty,pixels.GetPixels32())<1000,"hidden mesh check failed");
                    root.SetActive(true);
                    Debug.Log("BELL MESH INSPECTION: "+view+" energy="+energy+" source="+source.name);
                }
                finally
                {
                    f.Camera.targetTexture=null; RenderTexture.active=previous;
                    Object.DestroyImmediate(target); Object.DestroyImmediate(pixels);
                }
                void Read()
                {
                    f.Camera.Render(); f.Camera.Render(); RenderTexture.active=target;
                    pixels.ReadPixels(new Rect(0,0,size.x,size.y),0,0); pixels.Apply();
                }
            }
        }
        finally
        {
            RenderPipelineManager.beginCameraRendering-=PinClock;
            Object.DestroyImmediate(root); Object.DestroyImmediate(lines);
            Object.DestroyImmediate(fill); Object.DestroyImmediate(ink);
        }
        Debug.Log("BELL MESH INSPECTION PASS: "+output);
        void PinClock(ScriptableRenderContext context,Camera camera)
        {
            if (camera==f.Camera)
                foreach (var clock in clocks)
                    camera.GetUniversalAdditionalCameraData().scriptableRenderer.EnqueuePass(clock);
        }
        void PlaceOverHead()
        {
            var camera=f.Camera.transform;
            var head=ContactPoint();
            if (custom!=null)
            {
                root.transform.rotation=camera.rotation;
                // Move toward the camera along the same viewing ray so the
                // broad face overlays the target without a screen-space shift.
                root.transform.position=head+camera.up*f.Height*.015f
                    -(head-camera.position).normalized*f.Height*CameraInset;
                wire.transform.localPosition=Vector3.zero;
                wire.transform.position-=camera.forward*.002f;
                return;
            }
            // Downward travel and face orientation are independent: expose the
            // full carrier while keeping its bottom centered over the crown.
            root.transform.rotation=camera.rotation*Quaternion.Euler(0,180,0)*Quaternion.Euler(0,0,90);
            var ray=head-camera.position;
            var screenUp=camera.up-camera.forward*(Vector3.Dot(ray,camera.up)/Vector3.Dot(ray,camera.forward));
            float bottom=float.PositiveInfinity;
            foreach (var vertex in source.vertices)
                bottom=Mathf.Min(bottom,Vector3.Dot(root.transform.TransformVector(vertex-source.bounds.center),screenUp));
            root.transform.position=head-root.transform.TransformVector(source.bounds.center)
                +camera.up*(-bottom+f.Height*.015f);
            wire.transform.localPosition=Vector3.zero;
            wire.transform.position-=camera.forward*.002f;
        }
        void CheckPlacement()
        {
            var min=new Vector2(float.PositiveInfinity,float.PositiveInfinity);
            var max=new Vector2(float.NegativeInfinity,float.NegativeInfinity);
            foreach (var vertex in source.vertices)
            {
                Vector2 p=f.Camera.WorldToScreenPoint(root.transform.TransformPoint(vertex));
                min=Vector2.Min(min,p); max=Vector2.Max(max,p);
            }
            Vector2 head=f.Camera.WorldToScreenPoint(ContactPoint()
                -(custom!=null ? f.Camera.transform.up*f.Height*BelowFeet : Vector3.zero));
            float height=max.y-min.y,width=max.x-min.x;
            Require(height>50 && width/height>(custom!=null ? .18f : .75f),
                "Bell mesh shows its thin edge instead of its broad face");
            Require(Mathf.Abs(Vector3.Dot(root.transform.forward,f.Camera.transform.forward))>.98f,
                "Bell carrier face is turned away from the camera");
            float contactLeft=float.PositiveInfinity,contactRight=float.NegativeInfinity;
            var vertices=source.vertices;
            var uv=source.uv;
            for (int i=0;i<vertices.Length;i++)
            {
                Vector2 p=f.Camera.WorldToScreenPoint(root.transform.TransformPoint(vertices[i]));
                // The half arc has two low endpoints; only U=0 is the impact end.
                if (custom!=null ? uv[i].x>.001f : p.y>min.y+height*.02f) continue;
                contactLeft=Mathf.Min(contactLeft,p.x); contactRight=Mathf.Max(contactRight,p.x);
            }
            Require(Mathf.Abs((contactLeft+contactRight)*.5f-head.x)<height*.025f,"Bell mesh is sideways from the head");
            Require(Mathf.Abs(min.y-head.y)<height*.025f,"Bell mesh does not end on the head");
            Require(min.x>2 && min.y>2 && max.x<f.Camera.pixelWidth-2 && max.y<f.Camera.pixelHeight-2,
                "Bell mesh is clipped by the viewport");
            Debug.Log("BELL CONTACT ALIGNMENT: width="+width+" height="+height+" bottom/contact gap="+(min.y-head.y));
        }
        Vector3 ContactPoint() => f.TargetHead(0)
            -(custom!=null ? f.Camera.transform.up*f.Height*.85f : Vector3.zero);
    }

    static long Difference(Color32[] a,Color32[] b)
    {
        long sum=0;
        for (int i=0;i<a.Length;i++) sum+=System.Math.Abs(a[i].r-b[i].r)
            +System.Math.Abs(a[i].g-b[i].g)+System.Math.Abs(a[i].b-b[i].b);
        return sum;
    }
}
