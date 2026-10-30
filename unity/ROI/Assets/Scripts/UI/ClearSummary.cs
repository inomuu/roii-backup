// THE DUNGEON CLEAR SUMMARY — what the run cost, what it paid, and the way out. Ported from
// `dungeon_clear_view.gd`.
//
// ONE TRIGGER, and it is the sim's: `last_clear` in the per-tick view. Its presence means a clear
// is owed; its absence means the panel is done. This screen never decides that a run ended, and it
// never names a destination — `LeaveInstance` picks it from `return_zone_id`, so the client cannot
// use the way out to travel somewhere it was not sent.
//
// SAME PANEL FOR A WIPE. `failed` flips the verdict and the colour, because a run that ended badly
// is still a run the player wants the numbers for.
//
// The countdown is presentation: it presses the same button a few seconds later, so an idle player
// is not left standing in a cleared room.
using System.Collections.Generic;
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    public sealed class ClearSummary : MonoBehaviour, UnityEngine.EventSystems.IPointerClickHandler
    {
        const float RowH = 36f, Countdown = 10f;

        Roi.Sim.SimDriver _driver;
        RectTransform _panel, _rows;
        TextMeshProUGUI _title, _timer, _performance;
        readonly List<GameObject> _built = new();
        string _shownFor = "";
        float _left;

        public static ClearSummary Create(Transform parent)
        {
            var go = new GameObject("ClearSummary", typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var c = go.AddComponent<ClearSummary>();
            c.Build((RectTransform)go.transform);
            go.SetActive(false);
            return c;
        }

        void Build(RectTransform root)
        {
            var overlay=root.gameObject.AddComponent<Canvas>();
            overlay.overrideSorting=true;overlay.sortingOrder=100;
            root.gameObject.AddComponent<UnityEngine.UI.GraphicRaycaster>();
            root.anchorMin = Vector2.zero; root.anchorMax = Vector2.one;
            root.offsetMin = Vector2.zero; root.offsetMax = Vector2.zero;

            var scrim = UiTheme.Flat(root, "Scrim",Color.white);
            scrim.sprite=ResultAtmosphere();
            var sr=scrim.rectTransform;sr.anchorMin=Vector2.zero;sr.anchorMax=Vector2.one;sr.offsetMin=sr.offsetMax=Vector2.zero;
            _panel=ExemplarUi.Rect(root,"ResultContent",.22f,.10f,.78f,.74f);
            _title=ExemplarUi.Text(_panel,"Title","",0,.85f,1,.96f,28,UiTheme.TextHigh);
            _title.alignment=TextAlignmentOptions.Center;UiTheme.Display(_title);
            _performance=ExemplarUi.Text(_panel,"Performance","",.14f,.76f,.86f,.86f,27,UiTheme.TextHigh);
            _performance.alignment=TextAlignmentOptions.Center;_performance.richText=true;UiTheme.Display(_performance);
            var host=ExemplarUi.Rect(_panel,"ResultScroll",.05f,.23f,.98f,.735f);
            _rows=UiScroll.Create(host,out _,false);
            for(int i=0;i<120;i++)
                ExemplarUi.Panel(_panel,"FooterDot",i/120f,.112f,(i+.32f)/120f,.115f,new Color(.64f,.73f,.71f,.40f));
            var leave=HudBar.MakeGhostButton(_panel,Loc.Get("UI_RESULT_TAP_CLOSE"),180,UiLayout.Touch,Leave,13);
            var lr=(RectTransform)leave.transform;lr.anchorMin=new Vector2(.24f,.025f);lr.anchorMax=new Vector2(.76f,.095f);lr.offsetMin=lr.offsetMax=Vector2.zero;
            var again=HudBar.MakeGhostButton(_panel,Loc.Get("UI_CLEAR_SUMMARY_RUN_IT_AGAIN"),180,UiLayout.Touch,Retry,13);
            var ar=(RectTransform)again.transform;ar.anchorMin=new Vector2(.78f,.025f);ar.anchorMax=new Vector2(1f,.095f);ar.offsetMin=ar.offsetMax=Vector2.zero;
            leave.transform.Find("Border")?.gameObject.SetActive(false);
            again.transform.Find("Border")?.gameObject.SetActive(false);
            leave.GetComponent<UnityEngine.UI.Image>().color=Color.clear;
            again.GetComponent<UnityEngine.UI.Image>().color=Color.clear;
            _timer=ExemplarUi.Text(_panel,"Timer","",0,-.035f,1,.005f,11,UiTheme.TextLow);
            _timer.alignment=TextAlignmentOptions.Center;
        }

        static Sprite _atmosphere;
        static Sprite ResultAtmosphere()
        {
            if (_atmosphere != null) return _atmosphere;
            var plate=Resources.Load<Texture2D>("ResultUI/ClearAtmosphere");
            if(plate!=null)
            {
                _atmosphere=Sprite.Create(plate,new Rect(0,0,plate.width,plate.height),new Vector2(.5f,.5f));
                _atmosphere.hideFlags=HideFlags.HideAndDontSave;
                return _atmosphere;
            }
            // Cached static atmosphere avoids a per-frame blur or particle pass.
            const int w=1024,h=576;
            var t=new Texture2D(w,h,TextureFormat.RGBA32,false);
            t.hideFlags=HideFlags.HideAndDontSave;t.wrapMode=TextureWrapMode.Clamp;
            var pixels=new Color[w*h];
            for(int y=0;y<h;y++)for(int x=0;x<w;x++)
            {
                float u=x/(float)(w-1),v=y/(float)(h-1),dx=u-.5f,dy=v-.758f;
                float mist=Mathf.Exp(-dx*dx*7)*Mathf.Pow(1-v,1.9f);
                float cloud=Mathf.PerlinNoise(u*6,v*5)*.025f;
                Color c=new Color(.065f+mist*.12f+cloud,.088f+mist*.20f+cloud,.085f+mist*.22f+cloud,1);
                float halo=Mathf.Exp(-dx*dx*110-dy*dy*800);
                float horizon=Mathf.Exp(-dy*dy*180000)*Mathf.Exp(-dx*dx*11);
                float beam=Mathf.Exp(-dx*dx*170000)*Mathf.Exp(-dy*dy*14);
                float crown=0;
                for(int k=-2;k<=2;k++)
                {
                    float cx=dx-k*.026f;
                    float height=k==0?.115f:(Mathf.Abs(k)==1?.054f:.027f);
                    float width=.0011f+.010f*Mathf.Exp(-Mathf.Max(0,dy)*100);
                    if(dy>=-.008f && dy<height)
                        crown=Mathf.Max(crown,Mathf.Exp(-cx*cx/(width*width))*Mathf.Pow(Mathf.Max(0,1-dy/height),.65f));
                }
                c+=new Color(.66f,.64f,.33f,0)*(halo*.40f+horizon*.40f);
                c+=new Color(.66f,.80f,.78f,0)*beam*.14f;
                c=Color.Lerp(c,new Color(.96f,.99f,.87f,1),Mathf.Clamp01(crown));
                for(int k=0;k<9;k++)
                {
                    float px=.12f+Mathf.Repeat(k*.173f,.84f),py=.10f+Mathf.Repeat(k*.137f,.31f);
                    float d=Mathf.Abs(u-px)+Mathf.Abs(v-py)*.56f;
                    float mote=Mathf.Exp(-d*d/.00032f)*.035f;
                    c+=new Color(mote,mote,mote,0);
                }
                c.a=.98f;pixels[y*w+x]=c;
            }
            t.SetPixels(pixels);t.Apply(false,true);
            _atmosphere=Sprite.Create(t,new Rect(0,0,w,h),new Vector2(.5f,.5f));
            _atmosphere.hideFlags=HideFlags.HideAndDontSave;return _atmosphere;
        }

        /// Fed the same view the arena reads.
        public void OnView(string json)
        {
            var v = JsonUtility.FromJson<View>(json);
            var clear = v?.last_clear;
            if (clear == null || string.IsNullOrEmpty(clear.instance_id))
            {
                // The summary is gone from the state, so the guard resets with it — this is what
                // lets the NEXT run show, and what keeps a dismissed one from re-raising itself
                // while the sim still holds it for a frame.
                _shownFor = "";
                gameObject.SetActive(false);
                return;
            }
            if (clear.instance_id == _shownFor) return;
            _shownFor = clear.instance_id;
            _left = Countdown;
            Fill(clear);
            gameObject.SetActive(true);
        }

        void Fill(Clear c)
        {
            foreach (var go in _built) Destroy(go);
            _built.Clear();

            _title.text = Loc.Get(c.failed ? "UI_RESULT_FAILED" : "UI_RESULT_CLEARED");
            _title.color = c.failed ? new Color(1f, 0.55f, 0.50f) : UiTheme.TextHigh;

            float y = 0f;
            long secs = c.ticks / 10;      // the sim's clock is ten ticks to the second
            _performance.text=$"<size=12>{Loc.Get("UI_RESULT_TIME")}</size> {secs/60}:{secs%60:00}   <size=20>›</size>   <size=12>{Loc.Get("UI_RESULT_DEFEATED")}</size> <color=#E8E0A3>{c.kills:N0}</color>";
            Stat(ref y, Loc.Get("UI_RESULT_TIME"), $"{secs / 60}:{secs % 60:00}");
            Stat(ref y, Loc.Get("UI_RESULT_DEFEATED"), c.kills.ToString("N0"));
            Stat(ref y, Loc.Get("UI_RESULT_EXP"), c.xp.ToString("N0"));
            Stat(ref y, Loc.Get("UI_RESULT_GIL"), c.gil.ToString("N0"));
            var loot = c.loot ?? new LootItem[0];
            Stat(ref y, Loc.Get("UI_RESULT_LOOT"), loot.Length.ToString());
            // The ITEMS, not a bare count: a number alone reads as "nothing happened" even when
            // something did. The sim collects them at the drop, capped at RUN_LOOT_SHOWN.
            foreach (var it in loot)
            {
                var name = GameText.ItemName(it.def_id, it.name);
                if (it.quantity > 1) name += $"  ×{it.quantity}";
                var l = UiTheme.Label(_rows, "L", name, UiTheme.CaptionSize,
                                      new Color(0.85f, 0.90f, 0.78f), TextAlignmentOptions.Left);
                var lr = l.rectTransform;
                lr.anchorMin = new Vector2(0f, 1f); lr.anchorMax = new Vector2(1f, 1f);
                lr.pivot = new Vector2(0.5f, 1f);
                lr.offsetMin = new Vector2(UiTheme.Pad, 0f); lr.offsetMax = new Vector2(0f, 0f);
                lr.sizeDelta = new Vector2(lr.sizeDelta.x, RowH);
                lr.anchoredPosition = new Vector2(UiTheme.Pad / 2f, -y);
                _built.Add(l.gameObject);
                y += RowH;
            }
            UiScroll.Fit(_rows,y);
        }

        void Stat(ref float y, string label, string value)
        {
            var stripe=UiTheme.Flat(_rows,"StatRow",new Color(.035f,.065f,.065f,.18f));Place(stripe.rectTransform,y);_built.Add(stripe.gameObject);
            var l = UiTheme.Label(_rows, "S_" + label, label, 14, UiTheme.TextMid,
                                  TextAlignmentOptions.Left);
            Place(l.rectTransform, y);
            _built.Add(l.gameObject);
            var v = UiTheme.Label(_rows, "V_" + label, value, 14, UiTheme.Accent,
                                  TextAlignmentOptions.Right);
            UiTheme.Display(v);
            Place(v.rectTransform, y);
            _built.Add(v.gameObject);
            var arrow=UiTheme.Label(_rows,"Arrow","›",15,UiTheme.TextMid,TextAlignmentOptions.Center);
            Place(arrow.rectTransform,y);
            arrow.rectTransform.anchorMin=new Vector2(.70f,1f);
            arrow.rectTransform.anchorMax=new Vector2(.76f,1f);
            _built.Add(arrow.gameObject);
            y += RowH + 2f;
        }

        void Place(RectTransform rt, float y)
        {
            rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
            rt.pivot = new Vector2(0.5f, 1f);
            rt.sizeDelta = new Vector2(0f, RowH);
            rt.anchoredPosition = new Vector2(0f, -y);
        }

        void Update()
        {
            if (!gameObject.activeSelf) return;
            _left -= Time.unscaledDeltaTime;
            _timer.text = Loc.Fmt("UI_RESULT_LEAVING",Mathf.CeilToInt(Mathf.Max(0f, _left)));
            if (_left <= 0f) Leave();
        }

        public void OnPointerClick(UnityEngine.EventSystems.PointerEventData eventData)
        {
            if (eventData.button == UnityEngine.EventSystems.PointerEventData.InputButton.Left) Leave();
        }

        void Leave()
        {
            if (!gameObject.activeSelf) return;
            // Hidden immediately, but `_shownFor` is KEPT: clearing it here would drop the
            // "already showed this run" guard, and if the sim has not dropped the summary by the
            // next view the panel re-raises itself — an unclosable clear screen.
            gameObject.SetActive(false);
            Fire("\"LeaveInstance\"");
        }

        void Retry()
        {
            gameObject.SetActive(false);
            Fire("\"RestartInstance\"");
        }

        void Fire(string eventJson)
        {
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            _driver?.ApplyInput(eventJson);
        }

        [System.Serializable] class LootItem { public string def_id, name; public int quantity; }
        [System.Serializable]
        class Clear
        {
            public string instance_id;
            public long ticks, kills, xp, gil;
            public bool failed;
            public LootItem[] loot;
        }
        [System.Serializable] class View { public Clear last_clear; }
    }
}
