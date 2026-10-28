using System.Collections;
using UnityEngine;
namespace Roi.UI {
 // Brief dissolve, with no position/scale changes or delayed gameplay callbacks.
 [RequireComponent(typeof(CanvasGroup))]
 public sealed class UiPageTransition : MonoBehaviour {
  CanvasGroup _group;
  Coroutine _running;
  public bool Complete => _running==null;
  public static void Play(RectTransform target){
   var effect=target.GetComponent<UiPageTransition>()??target.gameObject.AddComponent<UiPageTransition>();
   effect.Begin();
  }
  void Begin(){
   _group=GetComponent<CanvasGroup>();
   if(_running!=null)StopCoroutine(_running);
   _running=null;_group.alpha=1;
   if(!isActiveAndEnabled || !UiEntrance.MotionAllowed)return;
   _running=StartCoroutine(Dissolve());
  }
  IEnumerator Dissolve(){
   float time=0,started=Time.unscaledTime;_group.alpha=UiTheme.PageStartAlpha;
   while(time<UiTheme.PageDuration){
    if(!UiEntrance.MotionAllowed)break;
    time=Time.unscaledTime-started;
    _group.alpha=Mathf.Lerp(UiTheme.PageStartAlpha,1,Mathf.SmoothStep(0,1,time/UiTheme.PageDuration));
    yield return null;
   }
   _group.alpha=1;_running=null;
  }
  // Reopening the parent already dissolves its body. Do not multiply nested fades on enable.
  void OnEnable(){if(_group!=null)_group.alpha=1;}
  void OnDisable(){if(_running!=null)StopCoroutine(_running);_running=null;if(_group!=null)_group.alpha=1;}
 }
}
