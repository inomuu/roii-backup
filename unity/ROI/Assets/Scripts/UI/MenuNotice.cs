using System;
using TMPro;
using UnityEngine;
namespace Roi.UI
{
    public sealed class MenuNotice : MonoBehaviour
    {
        float _width = -1;
        float _iconSize = 44;
        void LateUpdate() => Fit();
        void Fit()
        {
            var rt=(RectTransform)transform;
            var parent=rt.parent as RectTransform;
            if(parent==null||parent.rect.width<=0)return;
            float width=Mathf.Min(620,Mathf.Max(1,parent.rect.width-24));
            if(Mathf.Abs(width-_width)<.5f)return;
            _width=width;
            rt.SetSizeWithCurrentAnchors(RectTransform.Axis.Horizontal,width);

            float left=24, right=24;
            var title=transform.Find("Title").GetComponent<TextMeshProUGUI>();
            var description=transform.Find("Description").GetComponent<TextMeshProUGUI>();
            var emblem=(RectTransform)transform.Find("Emblem");
            emblem.gameObject.SetActive(true);
            float titleHeight=Mathf.Max(48,title.GetPreferredValues(title.text,width-left-right,10000).y+4);
            Place(title.rectTransform,left,right,_iconSize+44,titleHeight);
            float messageTop=_iconSize+44+titleHeight+8;
            float messageHeight=Mathf.Max(44,description.GetPreferredValues(description.text,width-left-right,10000).y+4);
            Place(description.rectTransform,left,right,messageTop,messageHeight);
            var button=GetComponentInChildren<UnityEngine.UI.Button>();
            rt.SetSizeWithCurrentAnchors(RectTransform.Axis.Vertical,messageTop+messageHeight+(button!=null?80:28));
            if(button!=null)
            {
                var br=(RectTransform)button.transform;
                br.anchorMin=br.anchorMax=new Vector2(.5f,0); br.pivot=new Vector2(.5f,0); br.anchoredPosition=new Vector2(0,20);
                br.SetSizeWithCurrentAnchors(RectTransform.Axis.Horizontal,Mathf.Min(220,width-left-right));
            }
        }
        public static void Center(GameObject notice)
        {
            var r=(RectTransform)notice.transform;
            r.anchorMin=r.anchorMax=r.pivot=new Vector2(.5f,.5f);
            r.anchoredPosition=Vector2.zero;
        }
        static void Place(RectTransform r,float left,float right,float top,float height)
        {
            r.anchorMin=new Vector2(0,1);r.anchorMax=new Vector2(1,1);r.pivot=new Vector2(.5f,1);
            r.sizeDelta=new Vector2(-left-right,height);
            r.anchoredPosition=new Vector2((left-right)*.5f,-top);
        }
        public static GameObject Create(RectTransform parent,string kind,string title,string message,Action retry=null,string actionLabel=null,float iconSize=44)
        {
            var root=new GameObject("MenuNotice",typeof(RectTransform));root.transform.SetParent(parent,false);
            var rt=(RectTransform)root.transform;rt.anchorMin=new Vector2(.5f,1);rt.anchorMax=new Vector2(.5f,1);rt.pivot=new Vector2(.5f,1);rt.sizeDelta=new Vector2(620,retry!=null?230:178);rt.anchoredPosition=new Vector2(0,-44);
            // The page supplies the background for this inline status.
            var symbol=new GameObject("Emblem",typeof(RectTransform));symbol.transform.SetParent(rt,false);
            var icon=symbol.AddComponent<HomeIcon>();icon.Kind=kind;icon.color=UiTheme.Accent;icon.raycastTarget=false;
            var ir=icon.rectTransform;ir.anchorMin=ir.anchorMax=new Vector2(.5f,1);ir.sizeDelta=new Vector2(iconSize,iconSize);ir.anchoredPosition=new Vector2(0,-20-iconSize*.5f);
            Label(rt,"Title",title,23,UiTheme.TextHigh,30,48);
            Label(rt,"Description",message,16,UiTheme.TextMid,90,60);
            if(retry!=null) {
                var button=HudBar.MakeButton(rt,actionLabel??Loc.Get("UI_NOTICE_RECONNECT"),"",220,48,retry,true);
                var br=button.GetComponent<RectTransform>();br.anchorMin=br.anchorMax=new Vector2(0,0);br.pivot=new Vector2(0,0);br.anchoredPosition=new Vector2(92,24);
            }
            var notice=root.AddComponent<MenuNotice>();notice._iconSize=iconSize;notice.Fit();
            return root;
        }
        static void Label(RectTransform parent,string name,string text,float size,Color color,float top,float height)
        {
            var label=UiTheme.Label(parent,name,text,size,color,TextAlignmentOptions.Center);
            label.textWrappingMode=TextWrappingModes.Normal;
            if(name=="Title")UiTheme.Display(label);
            var r=label.rectTransform;r.anchorMin=new Vector2(0,1);r.anchorMax=new Vector2(1,1);r.pivot=new Vector2(.5f,1);r.offsetMin=new Vector2(92,0);r.offsetMax=new Vector2(-32,0);r.sizeDelta=new Vector2(r.sizeDelta.x,height);r.anchoredPosition=new Vector2(30,-top);
        }
    }
}
