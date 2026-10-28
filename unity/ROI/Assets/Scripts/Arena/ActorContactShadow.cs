using UnityEngine;
using UnityEngine.Rendering;

namespace Roi
{
    // Cheap ground contact only; the sprite's directional cast shadow remains independent.
    public sealed class ActorContactShadow : MonoBehaviour
    {
        MeshRenderer _renderer;
        Material _material;
        Mesh _mesh;
        float _width;
        static readonly int Opacity=Shader.PropertyToID("_Opacity");
        public static ActorContactShadow Create(Transform actor)
        {
            var go=new GameObject("ContactShadow");go.layer=actor.gameObject.layer;
            // Above the ground's optional wet-sheen plane at y=.02, but beneath the feet.
            go.transform.SetParent(actor,false);go.transform.localPosition=new Vector3(0,.04f,0);
            var shadow=go.AddComponent<ActorContactShadow>();shadow.Build();return shadow;
        }
        void Build()
        {
            _mesh=new Mesh{name="Actor contact quad"};
            _mesh.vertices=new[]{new Vector3(-.5f,0,-.5f),new Vector3(-.5f,0,.5f),new Vector3(.5f,0,.5f),new Vector3(.5f,0,-.5f)};
            _mesh.uv=new[]{Vector2.zero,Vector2.up,Vector2.one,Vector2.right};
            _mesh.triangles=new[]{0,1,2,0,2,3};_mesh.RecalculateBounds();
            gameObject.AddComponent<MeshFilter>().sharedMesh=_mesh;
            _renderer=gameObject.AddComponent<MeshRenderer>();
            _renderer.shadowCastingMode=ShadowCastingMode.Off;_renderer.receiveShadows=false;
            var shader=Resources.Load<Shader>("ActorContactShadow");
            if(shader==null||!shader.isSupported){_renderer.enabled=false;return;}
            _material=new Material(shader);_renderer.sharedMaterial=_material;
        }
        public void Bind(float visibleHeight,float lift,float alpha)
        {
            float width=Mathf.Clamp(visibleHeight*.65f,.28f,1.6f);
            _width=_width<=0?width:Mathf.Lerp(_width,width,1-Mathf.Exp(-Time.deltaTime*8));
            transform.localScale=new Vector3(_width,1,_width*.66f);
            if(_material!=null)_material.SetFloat(Opacity,.40f*alpha/(1+Mathf.Max(0,lift)*2));
        }
        void OnDestroy(){if(_material!=null)Destroy(_material);if(_mesh!=null)Destroy(_mesh);}
    }
}
