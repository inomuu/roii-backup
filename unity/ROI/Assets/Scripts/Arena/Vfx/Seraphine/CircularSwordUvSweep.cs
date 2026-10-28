using UnityEngine;
using UnityEngine.Rendering;

namespace Roi.Arena
{
    // Alternative carrier for the same five slash layers. Meshes and UVs are built only once.
    public sealed class CircularSwordUvSweep : System.IDisposable
    {
        const int Steps=192;
        readonly Mesh[] _meshes=new Mesh[5];
        readonly Material[] _materials=new Material[5];
        readonly MeshRenderer[] _renderers=new MeshRenderer[5];
        readonly MaterialPropertyBlock _block=new MaterialPropertyBlock();
        readonly Texture2D _profile;
        readonly GameObject _root;
        readonly float[] _offsets={ 0,-16,0,-9,-12 };

        public CircularSwordUvSweep(Transform parent,Shader shader,Texture mask,Texture flow)
        {
            _root=new GameObject("UV sword sweep");
            _root.transform.SetParent(parent,false); _root.layer=parent.gameObject.layer;
            // Bake the approved geometric width curve into a linear lookup, not new artwork.
            _profile=new Texture2D(256,1,TextureFormat.RFloat,false,true)
            { name="Approved blade width profile",wrapMode=TextureWrapMode.Clamp,filterMode=FilterMode.Bilinear };
            var samples=new Color[256];
            for (int i=0;i<samples.Length;i++)
            {
                float u=i/(float)(samples.Length-1);
                float width=Mathf.Pow(Mathf.Max(0,Mathf.Sin(u*Mathf.PI)),.8f)*Mathf.Lerp(.25f,1,u);
                samples[i]=new Color(width,0,0,1);
            }
            _profile.SetPixels(samples); _profile.Apply(false,true);
            Build(0,"UV main blade",1.35f,.16f,330,0,4.5f,new Color(.7f,.94f,1),Color.white,null);
            Build(1,"UV inset blade",1.07f,.045f,330*.91f,0,3.5f,new Color(.45f,.92f,1),new Color(.45f,.92f,1),null);
            Build(2,"UV white halo",1.35f,.25f,330,1,2.3f,new Color(.8f,.93f,1),new Color(.8f,.93f,1),Texture2D.whiteTexture);
            Build(3,"UV blue wake",1.6f,.52f,330,1,3,new Color(.005f,.18f,1),new Color(.005f,.18f,1),flow);
            Build(4,"UV textured streaks",1.48f,.4f,330*.95f,2,3.8f,new Color(.83f,.96f,1),new Color(.83f,.96f,1),flow);

            void Build(int index,string name,float radius,float width,float span,int mode,float gain,Color inner,Color outer,Texture noise)
            {
                var go=new GameObject(name,typeof(MeshFilter),typeof(MeshRenderer));
                go.layer=parent.gameObject.layer; go.transform.SetParent(_root.transform,false);
                var vertices=new Vector3[(Steps+1)*2]; var uv=new Vector2[vertices.Length];
                var indices=new int[Steps*6];
                for (int s=0;s<=Steps;s++)
                {
                    float u=s/(float)Steps,angle=(180+u*360)*Mathf.Deg2Rad;
                    for (int band=0;band<2;band++)
                    {
                        float r=radius+(band==0 ? -width : width);
                        vertices[s*2+band]=new Vector3(Mathf.Cos(angle)*r,index==0 ? 0 : .002f,Mathf.Sin(angle)*r);
                        uv[s*2+band]=new Vector2(u,band);
                    }
                    if (s==Steps) continue;
                    int k=s*2,at=s*6;
                    indices[at]=k; indices[at+1]=k+2; indices[at+2]=k+1;
                    indices[at+3]=k+1; indices[at+4]=k+2; indices[at+5]=k+3;
                }
                _meshes[index]=new Mesh { name=name,vertices=vertices,uv=uv,triangles=indices };
                _meshes[index].RecalculateBounds();
                go.GetComponent<MeshFilter>().sharedMesh=_meshes[index];
                var material=new Material(shader) { name=name,renderQueue=mode==0 ? 3000 : mode==1 ? 3001 : 3002 };
                material.SetTexture("_WidthTex",_profile); material.SetTexture("_MaskTex",mask);
                if (noise!=null) material.SetTexture("_FlowTex",noise);
                material.SetFloat("_Span",span/360); material.SetFloat("_Mode",mode);
                material.SetFloat("_EmissionGain",gain);
                material.SetVector("_InnerTint",inner); material.SetVector("_OuterTint",outer);
                material.SetInt("_DstBlend",(int)(mode==0 ? BlendMode.OneMinusSrcAlpha : BlendMode.One));
                _materials[index]=material;
                var renderer=go.GetComponent<MeshRenderer>(); renderer.sharedMaterial=material;
                renderer.shadowCastingMode=ShadowCastingMode.Off; renderer.receiveShadows=false;
                renderer.enabled=false; _renderers[index]=renderer;
            }
        }

        public void Sample(float sweep,float reveal,float opacity,float phase)
        {
            for (int i=0;i<_renderers.Length;i++)
            {
                float strength=i==2 ? .5f : i==3 ? .85f : i==4 ? .8f : 1;
                _block.SetFloat("_Head",(sweep+_offsets[i])/360);
                _block.SetFloat("_Reveal",reveal); _block.SetFloat("_Phase",phase);
                _block.SetFloat("_Opacity",opacity*strength);
                _block.SetFloat("_Erosion",Mathf.SmoothStep(0,1.05f,Mathf.InverseLerp(.52f,.98f,phase)));
                _renderers[i].SetPropertyBlock(_block); _renderers[i].enabled=true;
            }
        }

        public void Hide() { foreach (var renderer in _renderers) if (renderer!=null) renderer.enabled=false; }
        public void Dispose()
        {
            foreach (var mesh in _meshes) Release(mesh);
            foreach (var material in _materials) Release(material);
            Release(_profile); Release(_root);
        }
        static void Release(Object asset)
        { if (asset!=null) { if (Application.isPlaying) Object.Destroy(asset); else Object.DestroyImmediate(asset); } }
    }
}
