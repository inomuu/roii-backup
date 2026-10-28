using UnityEngine;

namespace Roi.UI
{
    // Short local reveal when the inspected item changes; never touches gameplay time.
    public sealed class EquipmentDetailReveal : MonoBehaviour
    {
        CanvasGroup _group;
        float _elapsed;
        public static void Play(GameObject target)
        {
            if(!target.activeInHierarchy)return;
            var reveal=target.GetComponent<EquipmentDetailReveal>();
            if(reveal==null)reveal=target.AddComponent<EquipmentDetailReveal>();
            if(reveal._group==null)
            {
                reveal._group=target.GetComponent<CanvasGroup>();
                if(reveal._group==null)reveal._group=target.AddComponent<CanvasGroup>();
            }
            reveal._elapsed=0;reveal._group.alpha=.55f;reveal.enabled=true;
        }
        void Update()
        {
            if(_group==null){enabled=false;return;}
            _elapsed+=Time.unscaledDeltaTime;
            _group.alpha=Mathf.Lerp(.55f,1,Mathf.SmoothStep(0,1,_elapsed/.15f));
            if(_elapsed>=.15f)enabled=false;
        }
        void OnDisable(){if(_group!=null)_group.alpha=1;}
    }
}
