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
    public sealed class ClearSummary : MonoBehaviour
    {
        const float RowH = 27f, Countdown = 10f;

        Roi.Sim.SimDriver _driver;
        RectTransform _panel, _rows;
        TextMeshProUGUI _title, _timer;
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

            var scrim = UiTheme.Flat(root, "Scrim",new Color(.025f,.055f,.060f,.985f));
            var sr=scrim.rectTransform;sr.anchorMin=Vector2.zero;sr.anchorMax=Vector2.one;sr.offsetMin=sr.offsetMax=Vector2.zero;
            _panel=ExemplarUi.Rect(root,"ResultContent",.22f,.14f,.78f,.86f);
            var glow=UiTheme.Flat(_panel,"ResultGlow",Color.white);
            glow.sprite=ResultGlow();glow.raycastTarget=false;
            ExemplarUi.Place(glow.rectTransform,.12f,.76f,.88f,.94f);
            _title=ExemplarUi.Text(_panel,"Title","",0,.72f,1,.83f,26,UiTheme.Accent);
            _title.alignment=TextAlignmentOptions.Center;UiTheme.Display(_title);
            var host=ExemplarUi.Rect(_panel,"ResultScroll",.12f,.25f,.88f,.66f);
            _rows=UiScroll.Create(host,out _);
            ExemplarUi.Panel(_panel,"FooterRule",.04f,.17f,.96f,.172f,UiTheme.LineStrong);
            var leave=HudBar.MakeGhostButton(_panel,Loc.Get("UI_CLEAR_SUMMARY_LEAVE"),180,UiLayout.Touch,Leave,16);
            var lr=(RectTransform)leave.transform;lr.anchorMin=new Vector2(.24f,.035f);lr.anchorMax=new Vector2(.48f,.10f);lr.offsetMin=lr.offsetMax=Vector2.zero;
            var again=HudBar.MakeGhostButton(_panel,Loc.Get("UI_CLEAR_SUMMARY_RUN_IT_AGAIN"),180,UiLayout.Touch,Retry,16);
            var ar=(RectTransform)again.transform;ar.anchorMin=new Vector2(.52f,.035f);ar.anchorMax=new Vector2(.76f,.10f);ar.offsetMin=ar.offsetMax=Vector2.zero;
            _timer=ExemplarUi.Text(_panel,"Timer","",0,.11f,1,.16f,UiTheme.CaptionSize,UiTheme.TextLow);
            _timer.alignment=TextAlignmentOptions.Center;
        }

        static Sprite _glow;
        static Sprite ResultGlow() {
            if(_glow!=null)return _glow;
            var t=new Texture2D(128,32,TextureFormat.RGBA32,false);
            t.hideFlags=HideFlags.HideAndDontSave;t.wrapMode=TextureWrapMode.Clamp;
            var pixels=new Color[128*32];
            for(int y=0;y<32;y++)for(int x=0;x<128;x++) {
                float u=(x-63.5f)/64f,v=(y-15.5f)/16f;
                float a=Mathf.Exp(-u*u*9-v*v*18)*.22f;
                a=Mathf.Max(a,Mathf.Clamp01(1-Mathf.Abs(u)*70-Mathf.Abs(v)*1.4f));
                a=Mathf.Max(a,Mathf.Clamp01(1-Mathf.Abs(u)*5-Mathf.Abs(v)*35)*.7f);
                pixels[y*128+x]=new Color(.82f,.87f,.65f,a);
            }
            t.SetPixels(pixels);t.Apply();
            _glow=Sprite.Create(t,new Rect(0,0,128,32),new Vector2(.5f,.5f));
            _glow.hideFlags=HideFlags.HideAndDontSave;return _glow;
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
            _title.color = c.failed ? new Color(1f, 0.55f, 0.50f) : UiTheme.Accent;

            float y = 0f;
            long secs = c.ticks / 10;      // the sim's clock is ten ticks to the second
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
            var stripe=UiTheme.Flat(_rows,"StatRow",new Color(.20f,.30f,.32f,.045f));Place(stripe.rectTransform,y);_built.Add(stripe.gameObject);
            var l = UiTheme.Label(_rows, "S_" + label, label, 16, UiTheme.TextMid,
                                  TextAlignmentOptions.Left);
            Place(l.rectTransform, y);
            _built.Add(l.gameObject);
            var v = UiTheme.Label(_rows, "V_" + label, value, 16, UiTheme.Accent,
                                  TextAlignmentOptions.Right);
            Place(v.rectTransform, y);
            _built.Add(v.gameObject);
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
