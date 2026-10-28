using UnityEngine;
using UnityEngine.Rendering;

namespace Roi.Arena
{
    // Presentation only. Endpoints are supplied by the caller, never resolved as gameplay here.
    public sealed class IronChainsEffect : MonoBehaviour
    {
        public const float CatchTime=.22f, PullTime=.36f, FadeTime=.82f, Duration=1.22f;
        const int MaxLinks=80, Around=24, Tube=6, Samples=96;
        readonly Vector3[] _points=new Vector3[Samples+1];
        readonly float[] _distances=new float[Samples+1];
        readonly Vector3[] _vertices=new Vector3[MaxLinks*Around*Tube];
        readonly Vector3[] _normals=new Vector3[MaxLinks*Around*Tube];
        Mesh _mesh;
        Material _material,_outlineMaterial;
        MeshRenderer _renderer,_outlineRenderer;
        IronChainsAura _aura;
        float _height;
        public int LinkCount { get; private set; }
        public float Opacity { get; private set; }
        public Vector3 Tip { get; private set; }

        public static IronChainsEffect Create(Transform parent,float height,bool casterAura=true)
        {
            var shader=Resources.Load<Shader>("Effects/IronChainMetal");
            var auraShader=Resources.Load<Shader>("Effects/BuffAura");
            var groundShader=Resources.Load<Shader>("Effects/BuffGroundGlow");
            var flow=Resources.Load<Texture2D>("Cutin/SeraphineAuraFlow-v1");
            if (shader==null || parent==null || (casterAura && (auraShader==null || groundShader==null || flow==null))) return null;
            var root=new GameObject("Iron chain links"); root.transform.SetParent(parent,false);
            var fx=root.AddComponent<IronChainsEffect>(); fx._height=Mathf.Max(.01f,height);
            fx._material=new Material(shader);
            fx._mesh=new Mesh { name="Alternating rounded rectangle chain links" }; fx._mesh.MarkDynamic();
            root.AddComponent<MeshFilter>().sharedMesh=fx._mesh;
            fx._renderer=root.AddComponent<MeshRenderer>(); fx._renderer.sharedMaterial=fx._material;
            fx._renderer.shadowCastingMode=ShadowCastingMode.Off; fx._renderer.receiveShadows=false;
            fx._outlineMaterial=new Material(shader) { name="Chain black ink outline",renderQueue=3001 };
            fx._outlineMaterial.SetFloat("_Outline",1);
            fx._outlineMaterial.SetFloat("_OutlineWidth",height*.013f);
            fx._outlineMaterial.SetFloat("_Cull",(float)CullMode.Front);
            var outline=new GameObject("Chain ink silhouette"); outline.transform.SetParent(root.transform,false);
            outline.AddComponent<MeshFilter>().sharedMesh=fx._mesh;
            fx._outlineRenderer=outline.AddComponent<MeshRenderer>(); fx._outlineRenderer.sharedMaterial=fx._outlineMaterial;
            fx._outlineRenderer.shadowCastingMode=ShadowCastingMode.Off; fx._outlineRenderer.receiveShadows=false;
            var indices=new int[MaxLinks*Around*Tube*6]; int at=0;
            for (int link=0;link<MaxLinks;link++) for (int a=0;a<Around;a++) for (int b=0;b<Tube;b++)
            {
                int offset=link*Around*Tube;
                int i=offset+a*Tube+b,j=offset+((a+1)%Around)*Tube+b;
                int k=offset+((a+1)%Around)*Tube+(b+1)%Tube,l=offset+a*Tube+(b+1)%Tube;
                indices[at++]=i; indices[at++]=j; indices[at++]=k;
                indices[at++]=i; indices[at++]=k; indices[at++]=l;
            }
            fx._mesh.vertices=fx._vertices; fx._mesh.triangles=indices;
            if (casterAura) fx._aura=new IronChainsAura(root.transform,fx._height,auraShader,groundShader,flow);
            fx.Hide(); return fx;
        }

        public void Sample(float age,Vector3 origin,Vector3 target,Camera camera,bool visible=true)
        {
            if (!visible || !isActiveAndEnabled || camera==null || !(age>=0 && age<Duration)
                || Vector3.Distance(origin,target)<_height*.05f) { Hide(); return; }
            float flight=Mathf.Clamp01(age/CatchTime);
            float reach=1-Mathf.Pow(1-flight,2);
            Tip=Vector3.Lerp(origin,target,reach);
            float taut=Mathf.SmoothStep(0,1,Mathf.InverseLerp(CatchTime,PullTime,age));
            float slack=(1-taut)*Mathf.Sin(flight*Mathf.PI*.8f)*_height*.3f;
            for (int i=0;i<=Samples;i++)
            {
                float t=i/(float)Samples;
                _points[i]=Vector3.Lerp(origin,Tip,t)
                    -Vector3.up*(Mathf.Sin(t*Mathf.PI)*slack)
                    +camera.transform.up*(Mathf.Sin(t*Mathf.PI*2)*slack*.45f);
                _distances[i]=i==0 ? 0 : _distances[i-1]+Vector3.Distance(_points[i-1],_points[i]);
            }
            float length=_distances[Samples];
            LinkCount=Mathf.Clamp(Mathf.CeilToInt(length/(_height*.16f)),1,MaxLinks);
            float spacing=length/LinkCount;
            int sample=1,vertex=0;
            for (int link=0;link<LinkCount;link++)
            {
                float distance=(link+.5f)*spacing;
                while (sample<Samples && _distances[sample]<distance) sample++;
                float t=Mathf.InverseLerp(_distances[sample-1],_distances[sample],distance);
                Vector3 center=Vector3.Lerp(_points[sample-1],_points[sample],t);
                Vector3 along=(_points[sample]-_points[sample-1]).normalized;
                Vector3 across=Vector3.ProjectOnPlane(camera.transform.up,along).normalized;
                if (across.sqrMagnitude<.01f) across=Vector3.ProjectOnPlane(camera.transform.right,along).normalized;
                across=Quaternion.AngleAxis(link%2==0 ? 18 : 108,along)*across;
                Vector3 normal=Vector3.Cross(along,across).normalized;
                float major=Mathf.Min(_height*.108f,spacing*.68f),minor=_height*.066f,tube=_height*.022f;
                float cornerRadius=Mathf.Min(major,minor)*.48f;
                for (int a=0;a<Around;a++)
                {
                    int corner=a/6;
                    float angle=(corner+(a%6)/5f)*Mathf.PI*.5f;
                    float xSign=corner==0 || corner==3 ? 1 : -1,ySign=corner<2 ? 1 : -1;
                    Vector3 ring=along*(xSign*(major-cornerRadius)+Mathf.Cos(angle)*cornerRadius)
                        +across*(ySign*(minor-cornerRadius)+Mathf.Sin(angle)*cornerRadius);
                    Vector3 outward=along*Mathf.Cos(angle)+across*Mathf.Sin(angle);
                    for (int b=0;b<Tube;b++)
                    {
                        float v=b*Mathf.PI*2/Tube;
                        Vector3 n=outward*Mathf.Cos(v)+normal*Mathf.Sin(v);
                        _vertices[vertex]=transform.InverseTransformPoint(center+ring+n*tube);
                        _normals[vertex++]=transform.InverseTransformDirection(n);
                    }
                }
            }
            // Collapse unused preallocated links; no per-frame mesh or material allocations.
            for (int i=vertex;i<_vertices.Length;i++) { _vertices[i]=Vector3.zero; _normals[i]=Vector3.up; }
            _mesh.vertices=_vertices; _mesh.normals=_normals;
            _mesh.SetSubMesh(0,new SubMeshDescriptor(0,LinkCount*Around*Tube*6),MeshUpdateFlags.DontRecalculateBounds);
            _mesh.RecalculateBounds();
            Opacity=1-Mathf.SmoothStep(0,1,Mathf.InverseLerp(FadeTime,Duration,age));
            _material.SetFloat("_Opacity",Opacity);
            _outlineMaterial.SetFloat("_Opacity",Opacity);
            _material.SetFloat("_Charge",Mathf.Sin(Mathf.Clamp01((age-CatchTime)/.35f)*Mathf.PI));
            _renderer.enabled=Opacity>0;
            _outlineRenderer.enabled=Opacity>0;
            _aura?.Sample(age);
        }

        public void Hide()
        {
            if (_renderer!=null) _renderer.enabled=false;
            if (_outlineRenderer!=null) _outlineRenderer.enabled=false;
            _aura?.Hide();
            LinkCount=0; Opacity=0;
        }
        void OnDisable() => Hide();
        void OnDestroy()
        {
            _aura?.Dispose();
            if (Application.isPlaying)
            { Destroy(_mesh); Destroy(_material); Destroy(_outlineMaterial); }
            else
            { DestroyImmediate(_mesh); DestroyImmediate(_material); DestroyImmediate(_outlineMaterial); }
        }
    }
}
