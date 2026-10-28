using System;
using System.Collections.Generic;
using System.IO;
using Roi.Arena;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using UnityEngine.Rendering;
using Object=UnityEngine.Object;

public static class SuzuSlashBasePreview
{
    const string AssetPath="Assets/Resources/Effects/Suzu/SuzuSlashBaseMesh.asset";
    const string Output="/tmp/roi-suzu-base-mesh";

    [MenuItem("ROI/Art/Capture Suzu Slash Base Mesh")]
    public static void Capture()
    {
        if (EditorApplication.isPlaying) throw new InvalidOperationException("Capture outside play mode.");
        Directory.CreateDirectory(Output);
        var built=SuzuSlashBaseMesh.Build();
        try
        {
            Verify(built);
            var asset=AssetDatabase.LoadAssetAtPath<Mesh>(AssetPath);
            if (asset==null) { AssetDatabase.CreateAsset(built,AssetPath); asset=built; }
            else { EditorUtility.CopySerialized(built,asset); EditorUtility.SetDirty(asset); }
            AssetDatabase.SaveAssetIfDirty(asset);
            Render(asset);
        }
        finally { if (!EditorUtility.IsPersistent(built)) Object.DestroyImmediate(built); }
        Debug.Log("SUZU BASE MESH PASS: "+AssetPath+"; captures="+Output);
    }

    static void Verify(Mesh mesh)
    {
        var original=mesh.vertices; var originalUv=mesh.uv;
        CheckRadii(); CheckArc(); CheckUv(); CheckWinding();
        var broken=(Vector3[])original.Clone(); broken[45]*=1.15f;
        try { mesh.vertices=broken; Reject(CheckRadii); }
        finally { mesh.vertices=original; }
        broken=(Vector3[])original.Clone();
        for (int i=0;i<broken.Length;i++) broken[i].y=0;
        try { mesh.vertices=broken; Reject(CheckRadii); }
        finally { mesh.vertices=original; }
        broken=(Vector3[])original.Clone();
        for (int i=0;i<3;i++) broken[SuzuSlashBaseMesh.VerticesPerSide-3+i]=broken[i];
        try { mesh.vertices=broken; Reject(CheckArc); }
        finally { mesh.vertices=original; }
        var badUv=(Vector2[])originalUv.Clone(); badUv[99].x+=.2f;
        try { mesh.uv=badUv; Reject(CheckUv); }
        finally { mesh.uv=originalUv; }
        badUv=(Vector2[])originalUv.Clone();
        Array.Copy(originalUv,0,badUv,SuzuSlashBaseMesh.VerticesPerSide,SuzuSlashBaseMesh.VerticesPerSide);
        try { mesh.uv=badUv; Reject(CheckUv); }
        finally { mesh.uv=originalUv; }
        var indices=mesh.triangles; var badIndices=(int[])indices.Clone();
        int back=indices.Length/2;
        (badIndices[back+1],badIndices[back+2])=(badIndices[back+2],badIndices[back+1]);
        try { mesh.triangles=badIndices; Reject(CheckWinding); }
        finally { mesh.triangles=indices; }
        Require(mesh.vertexCount==390 && mesh.triangles.Length/3==512,"unexpected topology");
        Debug.Log("SUZU BASE GEOMETRY: 390 vertices, 128 quads / 256 triangles per side; 270 degrees; radii 0.85 / 1.7; "
            +"lens section with +/-0.16 ridge, sharp inner/outer edges, open angular ends, mirrored UV verified; "
            +"flattened-ridge/displaced-vertex/closed-gap/warped-UV/unmirrored-back/reversed-face sabotages caught");

        void CheckRadii()
        {
            float error=0;
            var v=mesh.vertices;
            for (int i=0;i<v.Length;i++)
            {
                float radius=Mathf.Lerp(.85f,1.7f,(i%3)/2f);
                float height=i%3==1 ? .16f*(i<SuzuSlashBaseMesh.VerticesPerSide ? 1 : -1) : 0;
                error=Mathf.Max(error,Mathf.Abs(new Vector2(v[i].x,v[i].z).magnitude-radius),Mathf.Abs(v[i].y-height));
            }
            Require(error<.00001f,"carrier lost its lens section or radial width");
        }
        void CheckArc()
        {
            var v=mesh.vertices; float sweep=0;
            for (int s=1;s<=64;s++)
            {
                var a=v[(s-1)*3]; var b=v[s*3];
                sweep+=Mathf.DeltaAngle(Mathf.Atan2(a.z,a.x)*Mathf.Rad2Deg,Mathf.Atan2(b.z,b.x)*Mathf.Rad2Deg);
            }
            Require(Mathf.Abs(sweep-270)<.001f,"arc is not the specified open 270 degrees");
        }
        void CheckUv()
        {
            var uv=mesh.uv;
            float error=0;
            for (int side=0;side<2;side++) for (int s=0;s<=64;s++) for (int row=0;row<3;row++)
                error=Mathf.Max(error,Vector2.Distance(uv[side*195+s*3+row],new Vector2(side==0 ? s/64f : 1-s/64f,row/2f)));
            Require(error<.00001f,"UV strip is not rectangular with mirrored back U");
        }
        void CheckWinding()
        {
            var v=mesh.vertices; var indices=mesh.triangles;
            for (int i=0;i<indices.Length;i+=3)
            {
                var normal=Vector3.Cross(v[indices[i+1]]-v[indices[i]],v[indices[i+2]]-v[indices[i]]).normalized;
                Require(normal.y*(i<indices.Length/2 ? 1 : -1)>.8f,"incorrect face winding");
            }
        }
    }

    static void Render(Mesh mesh)
    {
        var previous=SceneManager.GetActiveScene();
        var scene=EditorSceneManager.NewPreviewScene();
        var owned=new List<Object>();
        RenderTexture target=null; Texture2D pixels=null;
        try
        {
            var shader=Shader.Find("Universal Render Pipeline/Unlit");
            Require(shader!=null,"preview shader missing");
            var gray=Material("Base gray",new Color(.54f,.57f,.60f));
            gray.shader=Shader.Find("Universal Render Pipeline/Lit");
            gray.SetFloat("_Smoothness",.2f);
            var ink=Material("Topology lines",new Color(.09f,.10f,.12f));
            var gridMat=Material("Grid",new Color(.19f,.21f,.23f));
            // Diagnostic colors identify the same angular columns on both sides, not a slash texture.
            var orange=new Material(Shader.Find("ROI/SwordEdge")) { name="UV correspondence diagnostic" };
            orange.SetFloat("_EmissionGain",1); orange.SetFloat("_Opacity",1); owned.Add(orange);
            var baseRenderer=Layer("Base mesh",mesh,gray);
            var wire=Wire(mesh,false); owned.Add(wire); var wireRenderer=Layer("Quad topology",wire,ink);
            var grid=Grid(); owned.Add(grid); var gridRenderer=Layer("Work grid",grid,gridMat);
            var uvMesh=Object.Instantiate(mesh); owned.Add(uvMesh); uvMesh.name="UV diagnostic only";
            var frontIndices=new int[mesh.triangles.Length/2]; Array.Copy(mesh.triangles,frontIndices,frontIndices.Length);
            uvMesh.triangles=frontIndices;
            var uvVertices=new Vector3[mesh.vertexCount]; var uv=mesh.uv;
            var uvColors=new Color[mesh.vertexCount];
            for (int i=0;i<uv.Length;i++) uvVertices[i]=new Vector3((uv[i].x-.5f)*3,0,(uv[i].y-.5f)*3);
            for (int i=0;i<uvColors.Length;i++) uvColors[i]=Color.Lerp(new Color(.04f,.3f,.75f),new Color(1,.4f,.035f),
                (i%SuzuSlashBaseMesh.VerticesPerSide)/3/64f);
            uvMesh.vertices=uvVertices; uvMesh.colors=uvColors; uvMesh.RecalculateBounds();
            var uvRenderer=Layer("UV layout",uvMesh,orange); uvRenderer.enabled=false;
            var uvWire=Wire(uvMesh,true); owned.Add(uvWire);
            var uvWireRenderer=Layer("UV grid",uvWire,ink); uvWireRenderer.enabled=false;
            var camera=new GameObject("Base mesh camera").AddComponent<Camera>();
            SceneManager.MoveGameObjectToScene(camera.gameObject,scene);
            camera.scene=scene; camera.enabled=false; camera.orthographic=true; camera.orthographicSize=2.08f;
            camera.clearFlags=CameraClearFlags.SolidColor; camera.backgroundColor=new Color(.115f,.13f,.145f);
            camera.nearClipPlane=.01f; camera.farClipPlane=50; camera.allowHDR=false;
            var light=new GameObject("Preview light").AddComponent<Light>(); light.type=LightType.Directional;
            SceneManager.MoveGameObjectToScene(light.gameObject,scene);
            light.transform.rotation=Quaternion.Euler(50,-30,0);
            light.intensity=1.3f;
            target=new RenderTexture(900,900,24,RenderTextureFormat.ARGB32) { antiAliasing=4 };
            pixels=new Texture2D(900,900,TextureFormat.RGB24,false); camera.targetTexture=target;
            camera.transform.position=new Vector3(0,6,0); camera.transform.LookAt(Vector3.zero,Vector3.forward);
            camera.Render(); camera.Render();
            Read("top"); var top=pixels.GetPixels32();
            baseRenderer.enabled=false; wireRenderer.enabled=false; Read("empty");
            var empty=pixels.GetPixels32(); int coverage=Changed(top,empty); Require(coverage>20000,"base mesh invisible");
            Read("hidden-sabotage"); Reject(()=>Require(Changed(empty,pixels.GetPixels32())>20000,"hidden mesh"));
            baseRenderer.enabled=true; wireRenderer.enabled=true;
            camera.transform.position=new Vector3(2.2f,4.5f,-5.3f); camera.transform.LookAt(Vector3.zero);
            Read("angled");
            gridRenderer.enabled=false;
            camera.transform.position=new Vector3(2.2f,-4.5f,-5.3f); camera.transform.LookAt(Vector3.zero);
            Read("back");
            camera.transform.position=new Vector3(0,.6f,-6); camera.transform.LookAt(Vector3.zero);
            Read("low-angle");
            camera.transform.position=new Vector3(0,0,-6); camera.transform.LookAt(Vector3.zero);
            var flat=Object.Instantiate(mesh); owned.Add(flat);
            var flatVertices=flat.vertices;
            for (int i=0;i<flatVertices.Length;i++) flatVertices[i].y=0;
            flat.vertices=flatVertices; flat.RecalculateNormals(); flat.RecalculateBounds();
            var flatRenderer=Layer("Flattened negative control",flat,gray); baseRenderer.enabled=false;
            wireRenderer.enabled=false; Read("flat-render-sabotage");
            var flatPixels=pixels.GetPixels32();
            flatRenderer.enabled=false; baseRenderer.enabled=true; Read("ridge-render");
            int ridgePixels=Changed(flatPixels,pixels.GetPixels32());
            Require(ridgePixels>2000,"ridge has no visible volume");
            baseRenderer.enabled=false; flatRenderer.enabled=true; Read("ridge-hidden-sabotage");
            Reject(()=>Require(Changed(flatPixels,pixels.GetPixels32())>2000,"flattened ridge"));
            flatRenderer.enabled=false; baseRenderer.enabled=true;
            Debug.Log("SUZU BASE RIDGE RENDER: "+ridgePixels+" changed pixels; flattened-ridge sabotage caught");
            var section=Section(mesh); owned.Add(section);
            var sectionRenderer=Layer("Actual radial section",section,Material("Section outline",new Color(1,.65f,.2f)));
            baseRenderer.enabled=false; wireRenderer.enabled=false;
            camera.orthographicSize=.62f;
            camera.transform.position=new Vector3(0,0,-6); camera.transform.LookAt(Vector3.zero);
            Read("section"); sectionRenderer.enabled=false; camera.orthographicSize=2.08f;
            baseRenderer.enabled=false; wireRenderer.enabled=false; gridRenderer.enabled=false;
            uvRenderer.enabled=true; uvWireRenderer.enabled=true;
            camera.transform.position=new Vector3(0,6,0); camera.transform.LookAt(Vector3.zero,Vector3.forward);
            Read("uv-front"); var frontPixels=pixels.GetPixels32();
            for (int i=0;i<SuzuSlashBaseMesh.VerticesPerSide;i++)
            {
                var backUv=uv[i+SuzuSlashBaseMesh.VerticesPerSide];
                uvVertices[i]=new Vector3((backUv.x-.5f)*3,0,(backUv.y-.5f)*3);
            }
            uvMesh.vertices=uvVertices;
            var backIndices=(int[])frontIndices.Clone();
            for (int i=0;i<backIndices.Length;i+=3) (backIndices[i+1],backIndices[i+2])=(backIndices[i+2],backIndices[i+1]);
            uvMesh.triangles=backIndices;
            var wireVertices=(Vector3[])uvVertices.Clone();
            for (int i=0;i<wireVertices.Length;i++) wireVertices[i].y+=.005f;
            uvWire.vertices=wireVertices;
            Read("uv-back");
            int mirrored=Changed(frontPixels,pixels.GetPixels32());
            Require(mirrored>20000,"UV correspondence colors did not mirror");
            // Put the front UVs back deliberately; the mirror guard must reject the unmirrored chart.
            for (int i=0;i<SuzuSlashBaseMesh.VerticesPerSide;i++)
                uvVertices[i]=new Vector3((uv[i].x-.5f)*3,0,(uv[i].y-.5f)*3);
            uvMesh.vertices=uvVertices;
            readUnmirrored();
            Debug.Log("SUZU BASE UV RENDER: mirrored correspondence changes="+mirrored+" pixels; unmirrored-chart sabotage caught");
            Debug.Log("SUZU BASE RENDER: top-view mesh contribution="+coverage+" pixels; hidden-renderer sabotage caught");

            void readUnmirrored()
            {
                Read("uv-unmirrored-sabotage");
                Reject(()=>Require(Changed(frontPixels,pixels.GetPixels32())>20000,"unmirrored UV chart"));
            }

            Material Material(string name,Color color)
            {
                var mat=new Material(shader) { name=name }; owned.Add(mat);
                mat.SetColor("_BaseColor",color); mat.SetFloat("_Cull",(float)CullMode.Back); return mat;
            }
            MeshRenderer Layer(string name,Mesh source,Material mat)
            {
                var go=new GameObject(name,typeof(MeshFilter),typeof(MeshRenderer));
                SceneManager.MoveGameObjectToScene(go,scene); go.GetComponent<MeshFilter>().sharedMesh=source;
                var renderer=go.GetComponent<MeshRenderer>(); renderer.sharedMaterial=mat;
                renderer.shadowCastingMode=ShadowCastingMode.Off; renderer.receiveShadows=false; return renderer;
            }
            void Read(string name)
            {
                var active=RenderTexture.active;
                try
                {
                    camera.Render(); RenderTexture.active=target;
                    pixels.ReadPixels(new Rect(0,0,900,900),0,0); pixels.Apply();
                    File.WriteAllBytes(Path.Combine(Output,name+".png"),pixels.EncodeToPNG());
                }
                finally { RenderTexture.active=active; }
            }
        }
        finally
        {
            EditorSceneManager.ClosePreviewScene(scene);
            if (previous.IsValid()) SceneManager.SetActiveScene(previous);
            if (target!=null) Object.DestroyImmediate(target);
            if (pixels!=null) Object.DestroyImmediate(pixels);
            foreach (var item in owned) if (item!=null) Object.DestroyImmediate(item);
        }
    }
    static Mesh Wire(Mesh source,bool uv)
    {
        var v=source.vertices;
        for (int i=0;i<v.Length;i++) v[i].y+=uv || i<SuzuSlashBaseMesh.VerticesPerSide ? .005f : -.005f;
        var indices=new List<int>();
        for (int side=0;side<(uv ? 1 : 2);side++) for (int s=0;s<=64;s++)
        {
            int offset=side*SuzuSlashBaseMesh.VerticesPerSide;
            for (int row=0;row<2;row++) { indices.Add(offset+s*3+row); indices.Add(offset+s*3+row+1); }
            if (s==64) continue;
            for (int row=0;row<3;row++) { indices.Add(offset+s*3+row); indices.Add(offset+(s+1)*3+row); }
        }
        var mesh=new Mesh { name=uv ? "UV diagnostic lines" : "Quad diagnostic lines",vertices=v };
        mesh.SetIndices(indices,MeshTopology.Lines,0); mesh.RecalculateBounds(); return mesh;
    }
    static Mesh Section(Mesh source)
    {
        var actual=source.vertices;
        var v=new Vector3[4]; int[] sample={ 0,1,2,SuzuSlashBaseMesh.VerticesPerSide+1 };
        float center=(SuzuSlashBaseMesh.InnerRadius+SuzuSlashBaseMesh.OuterRadius)*.5f;
        for (int i=0;i<v.Length;i++)
        {
            var p=actual[sample[i]];
            v[i]=new Vector3(new Vector2(p.x,p.z).magnitude-center,p.y,0);
        }
        var mesh=new Mesh { name="Measured radial cross section",vertices=v };
        mesh.SetIndices(new[] { 0,1,1,2,2,3,3,0 },MeshTopology.Lines,0);
        mesh.RecalculateBounds(); return mesh;
    }
    static Mesh Grid()
    {
        var vertices=new List<Vector3>(); var indices=new List<int>();
        for (int i=-10;i<=10;i++)
        {
            float p=i*.25f;
            vertices.Add(new Vector3(-2.5f,-.02f,p)); vertices.Add(new Vector3(2.5f,-.02f,p));
            vertices.Add(new Vector3(p,-.02f,-2.5f)); vertices.Add(new Vector3(p,-.02f,2.5f));
        }
        for (int i=0;i<vertices.Count;i++) indices.Add(i);
        var mesh=new Mesh { name="Diagnostic work grid" }; mesh.SetVertices(vertices);
        mesh.SetIndices(indices,MeshTopology.Lines,0); mesh.RecalculateBounds(); return mesh;
    }
    static int Changed(Color32[] a,Color32[] b)
    {
        int count=0;
        for (int i=0;i<a.Length;i++) if (Math.Abs(a[i].r-b[i].r)+Math.Abs(a[i].g-b[i].g)+Math.Abs(a[i].b-b[i].b)>30) count++;
        return count;
    }
    static void Require(bool condition,string message)
    { if (!condition) throw new InvalidOperationException("SUZU BASE: "+message); }
    static void Reject(Action assertion)
    {
        bool caught=false; try { assertion(); } catch (InvalidOperationException) { caught=true; }
        Require(caught,"guard survived sabotage");
    }
}
