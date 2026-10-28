using System;
using TMPro;
using UnityEngine;
namespace Roi.UI
{
    public sealed class MenuNotice : MonoBehaviour
    {
        float _width = -1;
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
            bool compact=width<480;
            float left=compact?24:92, right=compact?24:32;
            var title=transform.Find("Title").GetComponent<TextMeshProUGUI>();
            var description=transform.Find("Description").GetComponent<TextMeshProUGUI>();
            var emblem=(RectTransform)transform.Find("Emblem");
            emblem.gameObject.SetActive(!compact);
            float titleHeight=Mathf.Max(48,title.GetPreferredValues(title.text,width-left-right,10000).y+4);
            Place(title.rectTransform,left,right,30,titleHeight);
            float messageTop=30+titleHeight+12;
            float messageHeight=Mathf.Max(60,description.GetPreferredValues(description.text,width-left-right,10000).y+4);
            Place(description.rectTransform,left,right,messageTop,messageHeight);
            var button=GetComponentInChildren<UnityEngine.UI.Button>();
            rt.SetSizeWithCurrentAnchors(RectTransform.Axis.Vertical,messageTop+messageHeight+(button!=null?80:28));
            if(button!=null)
            {
                var br=(RectTransform)button.transform;
                br.anchoredPosition=new Vector2(left,24);
                br.SetSizeWithCurrentAnchors(RectTransform.Axis.Horizontal,Mathf.Min(220,width-left-right));
            }
        }
        static void Place(RectTransform r,float left,float right,float top,float height)
        {
            r.anchorMin=new Vector2(0,1);r.anchorMax=new Vector2(1,1);r.pivot=new Vector2(.5f,1);
            r.sizeDelta=new Vector2(-left-right,height);
            r.anchoredPosition=new Vector2((left-right)*.5f,-top);
        }
        public static GameObject Create(RectTransform parent,string kind,string title,string message,Action retry=null,string actionLabel=null)
        {
            var root=new GameObject("MenuNotice",typeof(RectTransform));root.transform.SetParent(parent,false);
            var rt=(RectTransform)root.transform;rt.anchorMin=new Vector2(.5f,1);rt.anchorMax=new Vector2(.5f,1);rt.pivot=new Vector2(.5f,1);rt.sizeDelta=new Vector2(620,retry!=null?230:178);rt.anchoredPosition=new Vector2(0,-44);
            GameMenuSurface.Add(rt);
            var symbol=new GameObject("Emblem",typeof(RectTransform));symbol.transform.SetParent(rt,false);
            var icon=symbol.AddComponent<HomeIcon>();icon.Kind=kind;icon.color=UiTheme.Accent;icon.raycastTarget=false;
            var ir=icon.rectTransform;ir.anchorMin=ir.anchorMax=new Vector2(0,1);ir.sizeDelta=new Vector2(42,42);ir.anchoredPosition=new Vector2(52,-54);
            Label(rt,"Title",title,27,UiTheme.TextHigh,30,48);
            Label(rt,"Description",message,16,UiTheme.TextMid,90,60);
            if(retry!=null) {
                var button=HudBar.MakeButton(rt,actionLabel??Loc.Get("UI_NOTICE_RECONNECT"),"",220,48,retry,true);
                var br=button.GetComponent<RectTransform>();br.anchorMin=br.anchorMax=new Vector2(0,0);br.pivot=new Vector2(0,0);br.anchoredPosition=new Vector2(92,24);
            }
            root.AddComponent<MenuNotice>().Fit();
            return root;
        }
        static void Label(RectTransform parent,string name,string text,float size,Color color,float top,float height)
        {
            var label=UiTheme.Label(parent,name,text,size,color,TextAlignmentOptions.Left);
            label.textWrappingMode=TextWrappingModes.Normal;
            if(name=="Title")UiTheme.Display(label);
            var r=label.rectTransform;r.anchorMin=new Vector2(0,1);r.anchorMax=new Vector2(1,1);r.pivot=new Vector2(.5f,1);r.offsetMin=new Vector2(92,0);r.offsetMax=new Vector2(-32,0);r.sizeDelta=new Vector2(r.sizeDelta.x,height);r.anchoredPosition=new Vector2(30,-top);
        }
    }
}
