using UnityEngine;
using UnityEngine.EventSystems;
namespace Roi.UI
{
    public sealed class HomeCardLift : MonoBehaviour, IPointerEnterHandler, IPointerExitHandler,
        IPointerDownHandler, IPointerUpHandler
    {
        HomeMenuDepth _owner;
        RectTransform _face, _shadow, _contact;
        Material _surface;
        Vector2 _shadowRest, _size;
        float _baseZ, _amount;
        bool _hovered, _pressed;
        public void Bind(HomeMenuDepth owner,RectTransform face,RectTransform shadow,RectTransform contact,float baseZ,Material surface)
        {
            _owner=owner;_face=face;_shadow=shadow;_contact=contact;_baseZ=baseZ;_surface=surface;
            if(shadow!=null)_shadowRest=shadow.anchoredPosition;
            Apply();
        }
        public void OnPointerEnter(PointerEventData e){_hovered=true;}
        public void OnPointerExit(PointerEventData e){_hovered=_pressed=false;}
        public void OnPointerDown(PointerEventData e){_pressed=true;}
        public void OnPointerUp(PointerEventData e){_pressed=false;}
        void LateUpdate()
        {
            if(_face==null)return;
            float target=_owner.DepthEnabled && _hovered && !_pressed?1:0;
            _amount=Mathf.Lerp(_amount,target,1-Mathf.Exp(-16*Time.unscaledDeltaTime));
            Apply();
            var size=_face.rect.size;
            if(size!=_size && size.y>0){_size=size;_surface.SetFloat("_Aspect",size.x/size.y);}
        }
        void Apply()
        {
            if(_face==null)return;
            float z=_owner.DepthEnabled?_baseZ-_amount*18:0;
            var p=_face.localPosition;p.z=z;_face.localPosition=p;
            if(_contact!=null){p=_contact.localPosition;p.z=z+6;_contact.localPosition=p;}
            if(_shadow!=null)_shadow.anchoredPosition=_shadowRest+new Vector2(6,-9)*_amount;
        }
        void OnDisable(){_hovered=_pressed=false;_amount=0;Apply();}
    }
}
