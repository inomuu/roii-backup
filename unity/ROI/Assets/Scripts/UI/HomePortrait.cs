using Live2D.Cubism.Rendering;
using UnityEngine;
using UnityEngine.Rendering.Universal;
using UnityEngine.UI;
namespace Roi.UI
{
    // Own render target: the portrait is independent of combat lighting/postprocessing.
    public sealed class HomePortrait : MonoBehaviour
    {
        GameObject _model;
        Camera _camera, _arena;
        RenderTexture _target;
        bool _arenaWasEnabled;
        Bounds _bounds;
        RawImage _image;
        float _aspect;
        [SerializeField, Range(.15f, .7f)] float framingHeight = .31f;
        [SerializeField, Range(0f, 1f)] float horizontalAnchor = .31f;
        [SerializeField, Range(.1f, 2f)] float animationSpeed = .5f;
        public void Configure(float framing, float anchor)
        { framingHeight = framing; horizontalAnchor = anchor; }
        void Start()
        {
            var prefab=Resources.Load<GameObject>("HomeUI/Portrait");if(prefab==null)return;
            _model=Instantiate(prefab);_model.name="HomeLive2D";_model.transform.position=Vector3.zero;
            var animator=_model.GetComponent<Animator>();
            if(animator!=null)
            {
                animator.runtimeAnimatorController=Resources.Load<RuntimeAnimatorController>("HomeUI/PortraitLoop");
                // Only this Home instance slows down; shared clips and cut-in playback stay intact.
                animator.speed=animationSpeed;
            }
            foreach(var t in _model.GetComponentsInChildren<Transform>(true))t.gameObject.layer=30;
            Bounds bounds=new Bounds();bool any=false;
            foreach(var r in _model.GetComponentsInChildren<CubismRenderer>(true))
                if(r.Mesh!=null)foreach(var v in r.Mesh.vertices){if(!any){bounds=new Bounds(v,Vector3.zero);any=true;}else bounds.Encapsulate(v);}
            if(!any||bounds.size.y<.01f)bounds=new Bounds(Vector3.zero,new Vector3(1,2,.1f));
            _bounds=bounds;
            var go=new GameObject("HomePortraitCamera");go.transform.SetParent(transform,false);
            _camera=go.AddComponent<Camera>();_camera.orthographic=true;
            _camera.orthographicSize=bounds.size.y*.59f;
            _camera.transform.position=new Vector3(bounds.center.x,bounds.center.y,-10);
            _camera.clearFlags=CameraClearFlags.SolidColor;_camera.backgroundColor=Color.clear;
            _camera.cullingMask=1<<30;_camera.allowHDR=false;_camera.allowMSAA=false;
            _camera.nearClipPlane=.1f;_camera.farClipPlane=30;
            var data=go.AddComponent<UniversalAdditionalCameraData>();data.renderPostProcessing=false;data.volumeLayerMask=0;
            _image=gameObject.AddComponent<RawImage>();_image.raycastTarget=false;
            Canvas.ForceUpdateCanvases();
            UpdateFraming();
            foreach(var c in _model.GetComponentsInChildren<CubismRenderController>(true))c.CameraToFace=_camera;
            var arena=Object.FindFirstObjectByType<Roi.Arena.ArenaView>();_arena=arena==null?null:arena.MainCamera;
            SuspendArena();
        }
        void LateUpdate(){UpdateFraming();}
        void UpdateFraming()
        {
            if(_camera==null||_image==null)return;
            var size=((RectTransform)transform).rect.size;
            if(size.x<=0||size.y<=0)return;
            float aspect=size.x/size.y;
            if(_target==null||Mathf.Abs(aspect-_aspect)>.002f)
            {
                _aspect=aspect;
                _camera.targetTexture=null;
                if(_target!=null){_target.Release();Destroy(_target);}
                int width=Mathf.Clamp(Mathf.RoundToInt(1400*aspect),16,4096);
                _target=new RenderTexture(width,Mathf.RoundToInt(width/aspect),24,RenderTextureFormat.ARGB32);
                _target.name="HomePortraitRT";_camera.targetTexture=_target;_image.texture=_target;
            }
            // Full-screen alpha target: only the screen edge crops the oversized portrait.
            _camera.orthographicSize=_bounds.size.y*framingHeight;
            float worldWidth=2*_camera.orthographicSize*aspect;
            _camera.transform.position=new Vector3(_bounds.center.x+(.5f-horizontalAnchor)*worldWidth,
                _bounds.center.y+.10f*_bounds.size.y,-10);
        }
        void SuspendArena(){if(_arena!=null){_arenaWasEnabled=_arena.enabled;_arena.enabled=false;}}
        void OnEnable(){if(_model!=null){_model.SetActive(true);_camera.enabled=true;SuspendArena();}}
        void OnDisable(){if(_model!=null)_model.SetActive(false);if(_camera!=null)_camera.enabled=false;if(_arena!=null)_arena.enabled=_arenaWasEnabled;}
        void OnDestroy(){if(_model!=null)Destroy(_model);if(_target!=null){_target.Release();Destroy(_target);}}
    }
}
