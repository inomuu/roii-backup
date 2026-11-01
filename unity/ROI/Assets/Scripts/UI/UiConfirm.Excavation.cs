using System;
using TMPro;
using UnityEngine;
using UnityEngine.EventSystems;
using UnityEngine.UI;
using static Roi.UI.GachaArt;

namespace Roi.UI
{
    public sealed partial class UiConfirm
    {
        public static void AskExcavation(int count,Action confirm,ulong? crystals=null,Action cancel=null)
        {
            var bar=UnityEngine.Object.FindFirstObjectByType<HudBar>();
            if(bar==null)return;
            var go=new GameObject("ExcavationConfirmation",typeof(RectTransform));
            go.transform.SetParent(bar.PopupLayer,false);
            var modal=go.AddComponent<UiConfirm>();modal._onConfirm=confirm;modal._onDismissed=cancel;
            var root=(RectTransform)go.transform;root.anchorMin=Vector2.zero;root.anchorMax=Vector2.one;
            root.offsetMin=root.offsetMax=Vector2.zero;
            var scrim=UiTheme.Flat(root,"Scrim",new Color(0,.008f,.02f,.78f));
            scrim.rectTransform.anchorMin=Vector2.zero;scrim.rectTransform.anchorMax=Vector2.one;
            scrim.rectTransform.offsetMin=scrim.rectTransform.offsetMax=Vector2.zero;
            var dismiss=scrim.gameObject.AddComponent<Button>();dismiss.onClick.AddListener(modal.Close);
            dismiss.transition=Selectable.Transition.None;dismiss.navigation=new Navigation{mode=Navigation.Mode.None};
            var panel=Rect(root,"SummonPanel",0,0,756,360);
            panel.anchorMin=panel.anchorMax=panel.pivot=new Vector2(.5f,.5f);panel.anchoredPosition=Vector2.zero;
            var available=((RectTransform)bar.PopupLayer).rect.size;
            panel.localScale=Vector3.one*Mathf.Min(1,Mathf.Min((available.x-32)/756,(available.y-32)/360));
            DialogSurface.Add(panel);
            var crest=new GameObject("ConfirmationCrest",typeof(RectTransform));crest.transform.SetParent(panel,false);
            var glyph=crest.AddComponent<HomeIcon>();glyph.Kind="Summon";glyph.color=Gold;glyph.raycastTarget=false;GachaArt.Place(glyph.rectTransform,360,64,36,36);
            if(crystals.HasValue) {
                Text(panel,"PullCount",string.Format(Loc.Get("UI_EXCAVATION_PULL"),count),105,129,546,26,17,Gold).alignment=TextAlignmentOptions.Center;
                var title=Text(panel,"Price",string.Format(Loc.Get("UI_EXCAVATION_CONFIRM_CRYSTALS"),crystals.Value),105,161,546,47,24,Ivory);
                title.alignment=TextAlignmentOptions.Center;title.enableAutoSizing=true;title.fontSizeMin=18;title.fontSizeMax=24;
            } else {
                var prefix=Text(panel,"Use",Loc.Get("UI_EXCAVATION_CONFIRM_USE"),0,151,150,45,26,Ivory);
                var tail=Text(panel,"Question",string.Format(Loc.Get("UI_EXCAVATION_CONFIRM_TICKETS"),count),0,151,440,45,26,Ivory);
                prefix.fontStyle=tail.fontStyle=FontStyles.Bold;
                float a=prefix.GetPreferredValues(prefix.text).x,b=tail.GetPreferredValues(tail.text).x;
                float left=(756-a-b-62)*.5f;
                GachaArt.Place(prefix.rectTransform,left,151,a+2,45);Ticket(panel,left+a+14,161,35,23);
                GachaArt.Place(tail.rectTransform,left+a+62,151,b+2,45);
            }
            var line=UiTheme.Flat(panel,"Divider",new Color(.64f,.59f,.43f,.35f));GachaArt.Place(line.rectTransform,228,214,300,1);line.raycastTarget=false;
            var diamond=UiTheme.Flat(panel,"Diamond",Gold);GachaArt.Place(diamond.rectTransform,375,211,6,6);diamond.rectTransform.localEulerAngles=new Vector3(0,0,45);diamond.raycastTarget=false;
            Button Action(string name,string text,float x,Action action)
            {
                var rect=Rect(panel,name,x,236,248,64);var skin=rect.gameObject.AddComponent<CodexButtonImage>();
                skin.Primary=name=="Confirm";skin.Raised=true;skin.color=UiTheme.Surface;
                Text(rect,"Label",text,34,13,180,37,22,name=="Confirm"?Gold:Ivory).alignment=TextAlignmentOptions.Center;
                var button=Hit(rect,action);button.targetGraphic=skin;UiButtonFeedback.Add(button);return button;
            }
            DialogSurface.CloseIcon(panel,modal.Close);
            UiEntrance.Play(panel);
            var back=Action("Cancel",Loc.Get("UI_EXCAVATION_CANCEL"),112,modal.Close);
            var ok=Action("Confirm",Loc.Get("UI_EXCAVATION_CONFIRM"),396,()=>{
                var action=modal._onConfirm;modal._onConfirm=null;modal._onDismissed=null;
                // Deactivate immediately so even a repeated input cannot confirm twice.
                modal.gameObject.SetActive(false);modal.Close();action?.Invoke();
            });
            back.navigation=new Navigation{mode=Navigation.Mode.Explicit,selectOnRight=ok};
            ok.navigation=new Navigation{mode=Navigation.Mode.Explicit,selectOnLeft=back};
            EventSystem.current?.SetSelectedGameObject(back.gameObject);
        }
    }
}
