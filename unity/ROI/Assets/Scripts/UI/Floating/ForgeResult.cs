using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Result belongs to the floating window group. Outside the group combat stays visible/live.
    public sealed class ForgeResult : MonoBehaviour
    {
        RectTransform _root,_glow;
        TextMeshProUGUI _title,_item,_gil,_hint;
        ForgeCell _cell;
        CanvasGroup _fade;
        const float FadeIn=.28f, FadeOut=.32f, Duration=4.5f;
        float _age;
        public bool Visible=>_root!=null&&_root.gameObject.activeSelf;
        public static ForgeResult Create(Transform parent)
        { var result=new GameObject("ForgeResultController").AddComponent<ForgeResult>(); result.transform.SetParent(parent,false); result.Build(parent); return result; }
        void Build(Transform parent)
        {
            _root=ForgeUi.Panel(parent,"Result",0,0,830,560,new Color(.035f,.043f,.06f,.72f)).rectTransform;
            _root.anchorMin=Vector2.zero;_root.anchorMax=Vector2.one;
            _root.offsetMin=_root.offsetMax=Vector2.zero;
            _fade=_root.gameObject.AddComponent<CanvasGroup>();
            var dismiss=_root.gameObject.AddComponent<Button>(); dismiss.onClick.AddListener(Hide);
            var content=ForgeUi.Rect(_root,"ResultContent",0,0,830,560);
            content.anchorMin=content.anchorMax=content.pivot=new Vector2(.5f,.5f);
            content.anchoredPosition=Vector2.zero;
            _glow=ForgeUi.Rect(content,"AcquisitionRays",185,42,460,460);
            // Grow around the reward, not from the top-left corner of its rectangle.
            _glow.pivot=new Vector2(.5f,.5f);
            _glow.anchoredPosition=new Vector2(415,-272);
            var graphic=_glow.gameObject.AddComponent<ForgeSigil>(); graphic.enabled=false; graphic.Rays=false; graphic.color=new Color(.76f,.66f,.42f,.06f); graphic.raycastTarget=false;
            var sheet=ForgeUi.Panel(content,"RewardDialog",80,130,670,260,Color.clear);sheet.raycastTarget=false;DialogSurface.Apply(sheet);
            _title=ForgeUi.Text(content,"ResultTitle","",50,156,730,42,25,ForgeUi.Gold); _title.alignment=TextAlignmentOptions.Center;
            var band=ForgeUi.Panel(content,"RewardBand",104,221,622,101,new Color(.025f,.045f,.043f,.7f)); band.raycastTarget=false;
            _cell=new ForgeCell(band.transform,66); ForgeUi.Place(_cell.Root,30,17,66,66);
            foreach(var cellGraphic in _cell.Root.GetComponentsInChildren<Graphic>())cellGraphic.raycastTarget=false;
            _item=ForgeUi.Text(band.transform,"RewardName","",124,18,470,37,17,UiTheme.TextHigh);
            _gil=ForgeUi.Text(band.transform,"Gil","",124,58,470,25,14,UiTheme.Accent);
            _hint=ForgeUi.Copy(content,"UI_FORGE_RESULT_CLOSE",120,344,590,26,12); _hint.alignment=TextAlignmentOptions.Center;
            Hide();
        }
        public void Show(ForgeModel.Receipt receipt)
        {
            // Reset synchronously: Show can run after this component's Update on a button click.
            // Otherwise the new receipt flashes with the previous result's full opacity for a frame.
            _age=0; _fade.alpha=0; _glow.localScale=Vector3.one*.95f;
            _root.SetAsLastSibling();
            _title.text=Loc.Get(receipt.status=="success"?"UI_FORGE_ACQUIRED":"UI_FORGE_FAILED");
            _cell.Bind(receipt.item,null,false,receipt.kind=="gems",0,""); _cell.Root.gameObject.SetActive(receipt.item!=null);
            _item.text=receipt.item==null?Loc.Get("UI_FORGE_NO_ITEM"):ForgeUi.Name(receipt.item);
            _gil.text=receipt.gil>0?Loc.Fmt("UI_FORGE_GIL_RECEIVED",receipt.gil):Loc.Get("UI_FORGE_GEM_RECEIVED");
            _root.gameObject.SetActive(true);
        }
        public void Hide() { if(_fade!=null)_fade.alpha=0; if(_root!=null)_root.gameObject.SetActive(false); }
        void Update()
        {
            if(!Visible)return; _age+=Time.unscaledDeltaTime;
            float entering=Mathf.SmoothStep(0,1,Mathf.Clamp01(_age/FadeIn));
            float leaving=Mathf.SmoothStep(0,1,Mathf.Clamp01((Duration-_age)/FadeOut));
            _fade.alpha=entering*leaving;
            // Settle quickly so thin diamond outlines do not crawl across pixels for four seconds.
            _glow.localScale=Vector3.one*Mathf.Lerp(.95f,1f,Mathf.SmoothStep(0,1,Mathf.Clamp01(_age/.35f)));
            if(_age>=Duration)Hide();
        }
    }
}
