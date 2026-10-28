using UnityEngine;

namespace Roi.UI
{
    [RequireComponent(typeof(CanvasGroup))]
    public sealed class ExcavationReveal : MonoBehaviour
    {
        CanvasGroup _group;
        float _elapsed;
        public void Play()
        {
            _group=GetComponent<CanvasGroup>();
            if(_group==null)_group=gameObject.AddComponent<CanvasGroup>();
            if(!UiEntrance.MotionAllowed){_group.alpha=1;enabled=false;return;}
            _elapsed=0;_group.alpha=.35f;enabled=true;
        }
        void Update()
        {
            if(_group==null){enabled=false;return;}
            _elapsed+=Time.unscaledDeltaTime;
            float t=Mathf.Clamp01(_elapsed/.24f);
            _group.alpha=Mathf.Lerp(.35f,1,1-(1-t)*(1-t));
            if(t>=1)enabled=false;
        }
        void OnDisable(){if(_group!=null)_group.alpha=1;}
    }
}
