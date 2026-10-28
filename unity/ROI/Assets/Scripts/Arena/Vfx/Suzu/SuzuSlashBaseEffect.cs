using UnityEngine;
using UnityEngine.Rendering;

namespace Roi.Arena
{
    // Isolated base study. No impact events, damage, fire, ground glow or mesh deformation.
    public sealed class SuzuSlashBaseEffect : MonoBehaviour
    {
        public const float Duration=1.65f;
        SpriteRenderer _body;
        Transform _anchor;
        float _height;
        Material _material;
        MeshRenderer _renderer;
        MaterialPropertyBlock _block;
        Vector3 _localOffset=Vector3.up*.46f, _size=Vector3.one;
        Quaternion _rotation=Quaternion.identity;

        public static SuzuSlashBaseEffect Create(SpriteRenderer body,Transform anchor,float height)
        {
            var mesh=Resources.Load<Mesh>("Effects/Suzu/SuzuSlashBaseMesh");
            var mask=Resources.Load<Texture2D>("Effects/Suzu/SuzuBaseWedgeMask-v3");
            var shader=Resources.Load<Shader>("Effects/Suzu/SuzuSlashBase");
            if (body==null || anchor==null || mesh==null || mask==null || shader==null) return null;
            var go=new GameObject("Suzu white base",typeof(MeshFilter),typeof(MeshRenderer));
            go.transform.SetParent(anchor,false); go.layer=body.gameObject.layer;
            var fx=go.AddComponent<SuzuSlashBaseEffect>();
            fx._block=new MaterialPropertyBlock();
            fx._body=body; fx._anchor=anchor; fx._height=Mathf.Max(.01f,height);
            fx._material=new Material(shader) { name="Suzu one-shot white base" };
            fx._material.SetTexture("_MaskTex",mask);
            go.GetComponent<MeshFilter>().sharedMesh=mesh;
            fx._renderer=go.GetComponent<MeshRenderer>(); fx._renderer.sharedMaterial=fx._material;
            fx._renderer.shadowCastingMode=ShadowCastingMode.Off; fx._renderer.receiveShadows=false;
            fx.Hide(); return fx;
        }

        public void SetPlacement(Vector3 offset,Quaternion rotation,Vector3 size,Vector2 arcWindow,bool alignBackU=false)
        {
            _localOffset=offset; _rotation=rotation; _size=size;
            float start=Mathf.Clamp(arcWindow.x,0,.999f),end=Mathf.Clamp(arcWindow.y,start+.001f,1);
            _material.SetVector("_ArcWindow",new Vector4(start,end,0,0));
            _material.SetFloat("_AlignBackU",alignBackU ? 1 : 0);
        }

        public void Sample(float age,bool visible=true)
        {
            if (!visible || !isActiveAndEnabled || _body==null || !_body.enabled || _anchor==null
                || !(age>=0 && age<Duration)) { Hide(); return; }
            var yaw=Quaternion.Euler(0,_body.transform.eulerAngles.y,0);
            float mirror=_body.flipX ? -1 : 1;
            var placement=_localOffset; placement.x*=mirror;
            transform.SetPositionAndRotation(_anchor.position+yaw*(placement*_height),yaw*_rotation);
            var s=_anchor.lossyScale;
            transform.localScale=new Vector3(mirror*_height*_size.x/Mathf.Max(.001f,Mathf.Abs(s.x)),
                _height*_size.y/Mathf.Max(.001f,Mathf.Abs(s.y)),_height*_size.z/Mathf.Max(.001f,Mathf.Abs(s.z)));
            float offset=age<.64f
                ? Mathf.Lerp(-1.05f,0,Mathf.SmoothStep(0,1,Mathf.InverseLerp(.36f,.64f,age)))
                : Mathf.Lerp(0,1.05f,Mathf.SmoothStep(0,1,Mathf.InverseLerp(.64f,1.55f,age)));
            float opacity=(1-Mathf.SmoothStep(0,1,Mathf.InverseLerp(1.3f,Duration,age)))*_body.color.a;
            _block.SetFloat("_Offset",offset); _block.SetFloat("_Opacity",opacity);
            _renderer.SetPropertyBlock(_block); _renderer.enabled=opacity>0;
        }
        public void Hide() { if (_renderer!=null) _renderer.enabled=false; }
        void OnDisable() => Hide();
        void OnDestroy()
        {
            if (_material==null) return;
            if (Application.isPlaying) Destroy(_material); else DestroyImmediate(_material);
        }
    }
}
