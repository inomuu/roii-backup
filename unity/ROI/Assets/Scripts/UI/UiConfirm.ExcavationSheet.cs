using System;
using TMPro;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.UI;
using static Roi.UI.GachaArt;
namespace Roi.UI {
 public sealed partial class UiConfirm {
  public static UiConfirm ExcavationSheet(string title,Action<RectTransform> content,Action closed=null,float sheetHeight=580) {
   var bar=UnityEngine.Object.FindFirstObjectByType<HudBar>();if(bar==null)return null;
   var go=new GameObject("ExcavationSheet",typeof(RectTransform));go.transform.SetParent(bar.PopupLayer,false);
   var modal=go.AddComponent<UiConfirm>();modal._onDismissed=closed;
   var root=(RectTransform)go.transform;root.anchorMin=Vector2.zero;root.anchorMax=Vector2.one;root.offsetMin=root.offsetMax=Vector2.zero;
   var scrim=UiTheme.Flat(root,"Scrim",new Color(.004f,.008f,.02f,.87f));scrim.rectTransform.anchorMin=Vector2.zero;scrim.rectTransform.anchorMax=Vector2.one;scrim.rectTransform.offsetMin=scrim.rectTransform.offsetMax=Vector2.zero;
   var dismiss=scrim.gameObject.AddComponent<Button>();dismiss.onClick.AddListener(modal.Close);dismiss.navigation=new Navigation{mode=Navigation.Mode.None};
   var panel=Rect(root,"Sheet",0,0,980,sheetHeight);panel.anchorMin=panel.anchorMax=panel.pivot=new Vector2(.5f,.5f);panel.anchoredPosition=Vector2.zero;
   var available=((RectTransform)bar.PopupLayer).rect.size;panel.localScale=Vector3.one*Mathf.Min(1,Mathf.Min((available.x-32)/980,(available.y-32)/sheetHeight));
   DialogSurface.Add(panel);
   Text(panel,"Title",title,36,26,900,48,30,Gold);
   content(Rect(panel,"Content",36,92,908,sheetHeight-206));
   var close=Rect(panel,"Close",366,sheetHeight-88,248,64);var skin=close.gameObject.AddComponent<CodexButtonImage>();skin.Raised=true;skin.color=UiTheme.Surface;
   Text(close,"Label",Loc.Get("UI_STORY_BACK"),30,14,188,35,22,Ivory).alignment=TextAlignmentOptions.Center;
   var closeButton=Hit(close,modal.Close);closeButton.targetGraphic=skin;UiButtonFeedback.Add(closeButton);EventSystem.current?.SetSelectedGameObject(close.gameObject);
   UiEntrance.Play(panel);
   return modal;
  }
 }
}
