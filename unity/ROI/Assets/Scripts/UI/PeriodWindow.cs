// DAILY and WEEKLY — one screen, because they are one screen.
//
// The two server endpoints answer an IDENTICAL payload: a board of eight tasks with a target and a
// progress, a ladder of point milestones each worth Crystal, and a reset time. Only the period
// differs — `day` against `week` — and the tasks are even the same eight with bigger numbers. A
// second file would have been the same file with one word changed, which is how two screens start
// drifting apart on the day someone fixes a bug in one of them.
//
// THE PERIOD ID TRAVELS WITH THE CLAIM. The server refuses a claim whose id does not match its own,
// and that is a real guard rather than a formality: a window left open across the reset, or a
// client whose clock stopped overnight, would otherwise be paid yesterday's ladder from a screen
// showing yesterday's numbers. So the id sent is the one the screen was DRAWN from — never re-read
// at the moment of pressing.
//
// Rewards land in the BOUND bucket: earned, not bought. The screen says so, because the difference
// matters the moment a player tries to spend them somewhere that only takes the tradeable kind.
using System.Collections.Generic;
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    public sealed class PeriodWindow : MonoBehaviour
    {
        const float RowH = 74f, Gap = 10f;

        /// "daily" or "weekly" — the path segment AND the name of the id field the claim carries.
        string _period, _idField;

        RectTransform _content, _footer;
        TextMeshProUGUI _head, _status;
        readonly List<GameObject> _built = new();
        string _json = "";
        long _periodId;

        public static PeriodWindow Attach(RectTransform body, string period, string idField)
        {
            var w = body.gameObject.AddComponent<PeriodWindow>();
            w._period = period; w._idField = idField;
            w.Build(body);
            return w;
        }

        void OnEnable() => Roi.Net.OnlineService.Instance.Changed += OnNet;
        void OnDisable()
        {
            if (Roi.Net.OnlineService.Instance != null)
                Roi.Net.OnlineService.Instance.Changed -= OnNet;
        }
        void OnNet() { if (isActiveAndEnabled) Refresh(); }

        void Build(RectTransform body)
        {
            float pad = UiTheme.PadLarge;
            ActivityBackdrop.Add(body);
            ActivityRail(body,_period=="daily"?"Daily":"Weekly");
            _footer=ExemplarUi.Rect(body,"Milestones",.115f,0,.97f,0);_footer.sizeDelta=new Vector2(0,110);_footer.pivot=new Vector2(.5f,0);_footer.anchoredPosition=new Vector2(0,12);
            _head = UiTheme.Label(body, "Head", "", UiTheme.HeadSize,
                                  UiTheme.Accent, TextAlignmentOptions.Right);
            var hr = _head.rectTransform;
            hr.anchorMin = new Vector2(0f, 1f); hr.anchorMax = new Vector2(1f, 1f);
            hr.pivot = new Vector2(0.5f, 1f);
            hr.anchorMin=new Vector2(.115f,1);hr.offsetMin = new Vector2(0, -40f); hr.offsetMax = new Vector2(-pad, -UiTheme.PadSmall);

            _status = UiTheme.Label(body, "Status", "", UiTheme.CaptionSize,
                                    UiTheme.TextLow, TextAlignmentOptions.Left);
            _status.textWrappingMode = TextWrappingModes.NoWrap;
            var sr = _status.rectTransform;
            sr.anchorMin = new Vector2(0f, 1f); sr.anchorMax = new Vector2(1f, 1f);
            sr.pivot = new Vector2(0.5f, 1f);
            sr.anchorMin=new Vector2(.115f,1);sr.offsetMin = new Vector2(0, -40f); sr.offsetMax = new Vector2(-(pad + 340f), -UiTheme.PadSmall);

            var host = new GameObject("Scroll", typeof(RectTransform));
            host.transform.SetParent(body, false);
            var hrt = (RectTransform)host.transform;
            hrt.anchorMin = Vector2.zero; hrt.anchorMax = Vector2.one;
            hrt.anchorMin=new Vector2(.115f,0);hrt.offsetMin = new Vector2(0,120);
            hrt.offsetMax = new Vector2(-pad, -(40f + UiTheme.Gap));
            _content = UiScroll.Create(hrt, out _);
        }

        public void Refresh()
        {
            var net = Roi.Net.OnlineService.Instance;
            if (!net.LoggedIn)
            {
                BuildLocalBoard();
                Rebuild();
                _head.text=Loc.Get("UI_PERIOD_LOCAL_RECORD");
                _status.text=Loc.Get("UI_PERIOD_LOCAL_REWARDS");
                return;
            }
            net.Run(net.Client.PeriodState(_period, (ok, json) =>
            {
                if (!ok) { _status.text = "state failed: " + json; return; }
                _json = json;
                Rebuild();
            }));
        }

        // Local development displays the real save's cumulative counters; server period
        // windows and payouts remain owned by the server.
        void BuildLocalBoard()
        {
            var driver=Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var saved=driver?.Session?.SaveJson();
            var measures=string.IsNullOrEmpty(saved)?new LocalMeasures():JsonUtility.FromJson<LocalSave>(saved)?.measures??new LocalMeasures();
            string[] ids={"kills_2500","rooms_10","damage_1m","fuse_3","gem_fuse_1","train_1","spend_gil","soul_node_1"};
            long[] targets=_period=="daily"?new long[]{2500,10,1000000,3,1,1,10000,1}:new long[]{12500,50,5000000,15,1,5,50000,5};
            long[] progress={measures.kills,measures.rooms_cleared,measures.damage_dealt,measures.fusions,measures.gem_fusions,measures.training_levels,measures.gil_spent,measures.soul_nodes_allocated};
            var state=new PeriodState{ok=true,board=new Task[ids.Length],ladder=new Rung[0]};
            for(int i=0;i<ids.Length;i++)state.board[i]=new Task{id=ids[i],target=targets[i],progress=System.Math.Min(targets[i],progress[i]),done=progress[i]>=targets[i]};
            _json=JsonUtility.ToJson(state);
        }
        [System.Serializable] class LocalSave { public LocalMeasures measures; }
        [System.Serializable] class LocalMeasures { public long kills,rooms_cleared,damage_dealt,fusions,gem_fusions,training_levels,gil_spent,soul_nodes_allocated; }

        void Rebuild()
        {
            Clear();
            var st = JsonUtility.FromJson<PeriodState>(_json) ?? new PeriodState();
            _periodId = _period == "daily" ? st.day : st.week;

            long left = st.resets_at_ms - st.now_ms;
            _head.text = $"{st.points} points   ·   resets in {Span(left)}";
            _status.text = st.claimable_crystal > 0
                ? $"{st.claimable_crystal:N0} {GameText.Crystal} ready to claim"
                : "Nothing to claim yet.";

            float y = 0f;
            
            foreach (var t in st.board ?? new Task[0])
            {
                var row = UiTheme.Box(_content, "T_" + t.id, UiTheme.Surface);
                row.sprite=UiTheme.CutFill(8);
                UiTheme.Border(row.rectTransform,UiTheme.Line);
                Place(row.rectTransform,y,RowH);_built.Add(row.gameObject);
                RewardArt(row.transform,.015f,.08f,.105f,.92f);
                var name=ExemplarUi.Text(row.transform,"Task",TaskName(t.id),.16f,.42f,.84f,.88f,UiTheme.BodySize,UiTheme.TextHigh);
                name.fontStyle=FontStyles.Bold;name.enableAutoSizing=true;name.fontSizeMin=12;name.fontSizeMax=UiTheme.BodySize;
                ExemplarUi.Text(row.transform,"Progress",$"{t.progress:N0} / {t.target:N0}",.16f,.12f,.84f,.41f,UiTheme.CaptionSize,UiTheme.TextLow);
                ActivityStatus.Add(row.transform,.87f,.15f,.97f,.85f,t.done,t.done);
                y += RowH + Gap;
            }

            UiScroll.Fit(_content,y+UiTheme.Pad);
            if(!Roi.Net.OnlineService.Instance.LoggedIn)return;
            var rungs=st.ladder??new Rung[0];
            var track=ExemplarUi.Panel(_footer,"Track",.04f,.56f,.94f,.575f,UiTheme.LineStrong);_built.Add(track.gameObject);
            int max=rungs.Length>0?rungs[rungs.Length-1].points:0;
            var fill=ExemplarUi.Panel(_footer,"TrackProgress",.04f,.56f,.04f+.90f*(max>0?Mathf.Clamp01(st.points/(float)max):0),.575f,UiTheme.Accent);_built.Add(fill.gameObject);
            for(int i=0;i<rungs.Length;i++){
                var rung=rungs[i];float x=.04f+.90f*(max>0?rung.points/(float)max:0);
                var node=ExemplarUi.Rect(_footer,"Milestone"+rung.points,x-.055f,.08f,x+.055f,.94f);_built.Add(node.gameObject);
                var disk=ExemplarUi.Panel(node,"RewardDisk",.17f,.43f,.83f,.93f,UiTheme.WithAlpha(UiTheme.Panel,.9f));disk.sprite=UiTheme.NoticePill;var circle=disk.gameObject.AddComponent<UnityEngine.UI.AspectRatioFitter>();circle.aspectMode=UnityEngine.UI.AspectRatioFitter.AspectMode.FitInParent;circle.aspectRatio=1;
                ActivityStatus.Add(node,.15f,.42f,.85f,.94f,rung.claimed,rung.reached);
                var hit=node.gameObject.AddComponent<UnityEngine.UI.Button>();hit.targetGraphic=disk;
                hit.interactable=rung.reached&&!rung.claimed;hit.onClick.AddListener(Claim);
                var value=ExemplarUi.Text(node,"Reward",rung.points+"\n"+rung.crystal+" "+GameText.Crystal,0,0,1,.40f,12,UiTheme.TextHigh);value.alignment=TextAlignmentOptions.Center;
            }
            var medal=RewardArt(_footer,0,.58f,.065f,1);_built.Add(medal.gameObject);
            var points=ExemplarUi.Text(_footer,"Points",st.points.ToString(),.07f,.62f,.15f,1,28,UiTheme.Accent);_built.Add(points.gameObject);

        }

        internal static RectTransform RewardArt(Transform parent,float x,float y,float r,float t)
        {
            var rt=ExemplarUi.Rect(parent,"ActivityMedallion",x,y,r,t);
            var picture=ExemplarUi.Rect(rt,"Artwork",0,0,1,1);
            var art=picture.gameObject.AddComponent<UnityEngine.UI.RawImage>();
            art.texture=Resources.Load<Texture2D>("HomeUI/ActivityMedallion-v1");art.raycastTarget=false;
            var fit=picture.gameObject.AddComponent<UnityEngine.UI.AspectRatioFitter>();
            fit.aspectMode=UnityEngine.UI.AspectRatioFitter.AspectMode.FitInParent;
            fit.aspectRatio=art.texture!=null?(float)art.texture.width/art.texture.height:1;
            return rt;
        }

        // Shared navigation for the three activity boards; it invokes the existing routes.
        internal static void ActivityRail(RectTransform body,string selected)
        {
            var rail=ExemplarUi.Rect(body,"ActivityNavigation",.01f,.20f,.09f,.94f);
            string[] routes={"Daily","Weekly","Events"};
            for(int i=0;i<routes.Length;i++){
                string route=routes[i];float top=1-i*.24f;
                var button=CollectionBrowser.Quiet(rail,route,"",0,top-.20f,1,top,()=>{if(route!=selected)Object.FindFirstObjectByType<HudBar>()?.Toggle(route);});
                ExemplarUi.Icon(button.transform,route,.25f,.22f,.75f,.78f,route==selected?UiTheme.Accent:UiTheme.TextMid);
                var label=ExemplarUi.Text(button.transform,"Caption",Loc.Get("UI_ACTIVITY_"+route.ToUpperInvariant()),0,0,1,.30f,11,route==selected?UiTheme.Accent:UiTheme.TextMid);label.alignment=TextAlignmentOptions.Center;label.gameObject.SetActive(false);
            }
            var seam=ExemplarUi.Rect(body,"ActivitySelection",.092f,.20f,.092f,.94f);seam.offsetMin=new Vector2(-40,0);seam.offsetMax=new Vector2(18,0);
            var arc=seam.gameObject.AddComponent<ShopSelectionArc>();arc.raycastTarget=false;arc.SelectNormalized(System.Array.IndexOf(routes,selected)*.24f+.10f);
        }

        /// Sends the id the screen was DRAWN from, not one read at the moment of pressing. That is
        /// what makes the server's mismatch check meaningful rather than a formality.
        void Claim()
        {
            var net = Roi.Net.OnlineService.Instance;
            if (!net.LoggedIn) return;
            _status.text = Loc.Get("UI_PERIOD_CLAIMING");
            net.Run(net.Client.PeriodClaim(_period, _idField, _periodId, (ok, body) =>
            {
                if (!ok) { _status.text = "refused: " + body; return; }
                var r = JsonUtility.FromJson<ClaimResp>(body);
                _status.text = r != null
                    ? $"claimed {r.crystal:N0} {GameText.Crystal} from {r.claimed_rungs} rungs"
                    : "claimed";
                Refresh();
            }));
        }

        /// The board ids are stable keys, not sentences. Until the localisation layer exists this
        /// is where they become English — one table, the same shape `GameText` uses.
        static string TaskName(string id) => id switch
        {
            "kills_2500" => "Defeat monsters",
            "rooms_10" => "Clear rooms",
            "damage_1m" => "Deal damage",
            "fuse_3" => "Fuse gear",
            "gem_fuse_1" => Loc.Get("UI_PERIOD_GEM_FUSE"),
            "card_feed_1" => "Feed a card to gear",
            "train_1" => "Train a stat",
            "spend_gil" => "Spend Gil",
            "soul_node_1" => "Take a soul path node",
            _ => id,
        };

        static string Span(long ms)
        {
            if (ms <= 0) return "now";
            long s = ms / 1000, h = s / 3600, m = (s % 3600) / 60;
            return h >= 24 ? $"{h / 24}d {h % 24}h" : h > 0 ? $"{h}h {m}m" : $"{m}m";
        }

        void Head(ref float y, string text)
        {
            var t = UiTheme.Caps(_content, "H_" + text, text, UiTheme.CaptionSize,
                                 UiTheme.TextLow, TextAlignmentOptions.Left);
            Place(t.rectTransform, y, 26f);
            _built.Add(t.gameObject);
            y += 26f;
        }

        static void Inset(TextMeshProUGUI t, float left, float right)
        {
            var rt = t.rectTransform;
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = new Vector2(left, 0f); rt.offsetMax = new Vector2(-right, 0f);
        }

        void Place(RectTransform rt, float y, float h = 0f)
        {
            if (h > 0f)
            {
                rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
                rt.pivot = new Vector2(0.5f, 1f);
                rt.sizeDelta = new Vector2(0f, h);
                rt.anchoredPosition = new Vector2(0f, -y);
            }
            else
            {
                rt.anchorMin = rt.anchorMax = new Vector2(0f, 1f);
                rt.pivot = new Vector2(0f, 1f);
                rt.anchoredPosition = new Vector2(0f, -y);
            }
        }

        void Clear()
        {
            foreach (var go in _built) {go.SetActive(false);Destroy(go); }
            _built.Clear();
        }

        [System.Serializable]
        class PeriodState
        {
            public bool ok;
            public long day, week, now_ms, resets_at_ms, claimable_crystal;
            public int points;
            public Task[] board;
            public Rung[] ladder;
        }
        [System.Serializable]
        class Task { public string id, kind; public long target, progress; public bool done; }
        [System.Serializable]
        class Rung { public int points; public long crystal; public bool reached, claimed; }
        [System.Serializable]
        class ClaimResp { public bool ok; public long crystal, xp; public int claimed_rungs; }
    }
    [RequireComponent(typeof(CanvasRenderer))]
    public sealed class ActivityStatus : UnityEngine.UI.MaskableGraphic
    {
        bool _done,_ready;
        public static void Add(Transform parent,float x,float y,float r,float t,bool done,bool ready)
        {
            var rt=ExemplarUi.Rect(parent,"ActivityStatus",x,y,r,t);
            var graphic=rt.gameObject.AddComponent<ActivityStatus>();graphic._done=done;graphic._ready=ready;
            graphic.color=parent.name.StartsWith("Milestone") ? UiTheme.TextHigh : (done||ready?UiTheme.Accent:UiTheme.TextLow);graphic.raycastTarget=false;
        }
        protected override void OnPopulateMesh(UnityEngine.UI.VertexHelper vh)
        {
            vh.Clear();var bounds=rectTransform.rect;var c=bounds.center;float radius=Mathf.Min(bounds.width,bounds.height)*.36f;
            for(int i=0;i<40;i++){
                float a=i*Mathf.PI*2/40,b=(i+1)*Mathf.PI*2/40;
                Line(vh,c+new Vector2(Mathf.Cos(a),Mathf.Sin(a))*radius,c+new Vector2(Mathf.Cos(b),Mathf.Sin(b))*radius,1.3f);
            }
            if(_done){Line(vh,c+new Vector2(-.5f,0)*radius,c+new Vector2(-.12f,-.35f)*radius,3);Line(vh,c+new Vector2(-.12f,-.35f)*radius,c+new Vector2(.55f,.4f)*radius,3);}
            else if(_ready){Line(vh,c+new Vector2(0,-.45f)*radius,c+new Vector2(0,.45f)*radius,3);Line(vh,c+new Vector2(-.45f,0)*radius,c+new Vector2(.45f,0)*radius,3);}
            else Line(vh,c+new Vector2(-.35f,0)*radius,c+new Vector2(.35f,0)*radius,2);
        }
        void Line(UnityEngine.UI.VertexHelper vh,Vector2 a,Vector2 b,float width)
        {
            var delta=(b-a).normalized;var n=new Vector2(-delta.y,delta.x)*width*.5f;int i=vh.currentVertCount;
            vh.AddVert(a-n,color,Vector2.zero);vh.AddVert(a+n,color,Vector2.zero);vh.AddVert(b+n,color,Vector2.zero);vh.AddVert(b-n,color,Vector2.zero);
            vh.AddTriangle(i,i+1,i+2);vh.AddTriangle(i,i+2,i+3);
        }
    }

}

namespace Roi.UI {
 [RequireComponent(typeof(CanvasRenderer))]
 public sealed class ActivityBackdrop : UnityEngine.UI.MaskableGraphic {
  public static void Add(RectTransform parent){var go=new GameObject("ActivityAtmosphere",typeof(RectTransform));go.transform.SetParent(parent,false);go.transform.SetAsFirstSibling();var g=go.AddComponent<ActivityBackdrop>();g.raycastTarget=false;var r=g.rectTransform;r.anchorMin=Vector2.zero;r.anchorMax=Vector2.one;r.offsetMin=r.offsetMax=Vector2.zero;}
  // WuWa idiom: the same charcoal-lit-from-upper-left gradient as RpgBackdrop, not the old teal.
  protected override void OnPopulateMesh(UnityEngine.UI.VertexHelper vh){vh.Clear();var r=rectTransform.rect;
   var lit=UiTheme.BackdropTop;var deep=UiTheme.BackdropBottom;var mid=Color.Lerp(lit,deep,.55f);
   vh.AddVert(new Vector2(r.xMin,r.yMin),mid,Vector2.zero);vh.AddVert(new Vector2(r.xMin,r.yMax),lit,Vector2.up);vh.AddVert(new Vector2(r.xMax,r.yMax),mid,Vector2.one);vh.AddVert(new Vector2(r.xMax,r.yMin),deep,Vector2.right);vh.AddTriangle(0,1,2);vh.AddTriangle(0,2,3);}
 }
}
