using UnityEngine;
using UnityEngine.Rendering;

namespace Roi.Arena
{
    // A/B study only: a generated atlas on the same world plane as CircularSwordSlash.
    public sealed class CircularSwordFlipbook : MonoBehaviour
    {
        public const int FrameCount=16;
        SpriteRenderer _body;
        Transform _anchor;
        float _height;
        Mesh _mesh;
        MeshRenderer _renderer;
        Material _material;
        MaterialPropertyBlock _properties;

        public static CircularSwordFlipbook Create(SpriteRenderer body,Transform anchor,float height)
        {
            var sheet=Resources.Load<Texture2D>("Effects/SeraphineSpinFlipbook-v2");
            var shader=Resources.Load<Shader>("Effects/SwordFlipbook");
            if (body==null || anchor==null || sheet==null || shader==null) return null;
            var go=new GameObject("Generated sword flipbook",typeof(MeshFilter),typeof(MeshRenderer));
            go.layer=body.gameObject.layer; go.transform.SetParent(anchor,false);
            var fx=go.AddComponent<CircularSwordFlipbook>();
            fx._body=body; fx._anchor=anchor; fx._height=Mathf.Max(.01f,height);
            fx._mesh=new Mesh { name="Sword flipbook plane" };
            fx._mesh.vertices=new[] { new Vector3(-2,0,-2),new Vector3(2,0,-2),
                new Vector3(-2,0,2),new Vector3(2,0,2) };
            fx._mesh.uv=new[] { Vector2.zero,Vector2.right,Vector2.up,Vector2.one };
            fx._mesh.triangles=new[] { 0,2,1,1,2,3 }; fx._mesh.RecalculateBounds();
            go.GetComponent<MeshFilter>().sharedMesh=fx._mesh;
            fx._material=new Material(shader) { name="Generated sword atlas study" };
            fx._material.SetTexture("_Atlas",sheet); fx._material.SetFloat("_EmissionGain",3);
            fx._renderer=go.GetComponent<MeshRenderer>(); fx._renderer.sharedMaterial=fx._material;
            fx._renderer.shadowCastingMode=ShadowCastingMode.Off; fx._renderer.receiveShadows=false;
            fx._renderer.enabled=false; fx._properties=new MaterialPropertyBlock();
            return fx;
        }

        public void Show(int attackFrame,float frameProgress,bool visible)
        {
            visible &= isActiveAndEnabled && _body!=null && _body.enabled && _anchor!=null
                && attackFrame>=2 && attackFrame<=4;
            _renderer.enabled=visible;
            if (!visible) return;
            float phase=Mathf.Clamp01((attackFrame-2+Mathf.Clamp01(frameProgress))/3f);
            int frame=Mathf.Min(FrameCount-1,Mathf.FloorToInt(phase*FrameCount));
            transform.SetPositionAndRotation(_anchor.position+Vector3.up*_height*.46f,
                Quaternion.Euler(0,_body.transform.eulerAngles.y,0));
            var scale=_anchor.lossyScale;
            transform.localScale=new Vector3((_body.flipX ? -1 : 1)*_height/Mathf.Max(.001f,Mathf.Abs(scale.x)),
                _height/Mathf.Max(.001f,Mathf.Abs(scale.y)),_height/Mathf.Max(.001f,Mathf.Abs(scale.z)));
            _properties.SetFloat("_Frame",frame); _properties.SetFloat("_Opacity",_body.color.a);
            _renderer.SetPropertyBlock(_properties);
        }

        void OnDisable() { if (_renderer!=null) _renderer.enabled=false; }
        void OnDestroy() { Release(_mesh); Release(_material); }
        static void Release(Object asset)
        { if (asset!=null) { if (Application.isPlaying) Destroy(asset); else DestroyImmediate(asset); } }
    }
}
