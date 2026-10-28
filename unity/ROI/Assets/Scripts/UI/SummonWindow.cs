// EXCAVATION — what this game calls its summon. Ported in shape from godot/views/gacha_view.gd,
// but the substance is the SERVER's, and that is the whole architecture of this screen.
//
// The class, the window key and the server's routes all still say gacha or summon; only the words
// the PLAYER reads say Excavation. The internal name is load-bearing — it keys the window
// dictionary and matches the Godot client's node — and renaming it to match the fiction is how a
// screen becomes hard to find in its own codebase. `GameText.ScreenName` is the one place the two
// vocabularies meet.
//
// THE REWARD IS NOT A CHORE (user, 2026-09-07: mail is "ช่องทางที่ปลอดภัยเฉยๆ"). The server mints
// every pull as MAIL inside the same transaction that debits the wallet, and that is exactly right
// — it is what makes a reward survive a client that dies mid-roll. But it is a DURABILITY
// mechanism, not a step for the player, so this screen applies the mail its own roll produced
// straight to the save before showing the result. The roll's reply carries those mail ROWS in full,
// so nothing has to be fetched and nothing has to be guessed at by scanning the inbox.
//
// No claim endpoint is called — see `MailWindow`. `/mail/claim` is dev-gated because it consumes
// the mail without crediting the save; the production path is ClaimMail -> verified save ->
// consume_claim, and this is the same path, taken for the player instead of by them.
//
// The mailbox stays as what it is: the safety net. Anything whose claim was interrupted is still
// sitting there, which is the whole point of sending it that way.
//
// THE ROLL HAPPENS ON THE SERVER AND NEVER IN THE SIM, for two separate reasons the server states
// outright: a pull is money, so a client-side draw could be save-scummed; and adding a draw to the
// sim's single PCG32 would shift every downstream outcome in the fight. So this screen does not
// roll anything. It asks, shows what came back, and the items arrive as MAIL — the sim only ever
// sees them through `ClaimMail`, which is server-authored and exactly-once.
//
// TWO THINGS THAT LOOK LIKE ERRORS AND ARE NOT:
//
//  402 is a QUOTE. The account is short of coupons, NOTHING has been charged, and the client is
//  expected to show the price and re-send with the confirm. `quoted_crystal` goes back with it so the
//  server cannot charge more than it quoted — the half of "binding" that matters.
//
//  A repeated idem_key is a REPLAY, not a double charge. The server stores the result against the
//  key and returns it for free, which is what makes a dropped reply safe to retry. The key is
//  generated once per attempt and reused across the confirm, because the quote and the confirmed
//  roll are the same call.
using System.Collections.Generic;
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    public sealed partial class SummonWindow : MonoBehaviour
    {
        const float BannerW = 300f, BannerH = 132f, Gap = UiTheme.PadSmall;

        RectTransform _body, _banners, _results;
        TextMeshProUGUI _wallet, _status;
        readonly List<GameObject> _built = new();
        string _stateJson = "", _selected = "";
        string _pendingKey = "";
        string _pendingBanner="";
        int _pendingCount;
        // The last roll and any outstanding quote are STATE, not things drawn once and left. The
        // first version drew the results straight after the roll and then called Refresh, whose
        // rebuild destroyed them a frame later — the pulls appeared and vanished. Anything that
        // must survive a rebuild has to be an input TO the rebuild.
        string _lastPulls = "";
        string _collected = "";
        int _quoteCount; ulong _quoteCrystal;
        Roi.Sim.SimDriver _driver;

        // Subscribed for the window's LIFETIME, not for one event. The first version added the
        // handler when Connect was pressed and removed it on the first callback — but the service
        // reports "connecting…" before it reports "connected", so the window consumed the wrong
        // event, unhooked, and never learned it had logged in. A connection has more than one
        // moment worth redrawing for.
        void OnEnable() => Roi.Net.OnlineService.Instance.Changed += OnNet;
        void OnDisable()
        {
            if (Roi.Net.OnlineService.Instance != null)
                Roi.Net.OnlineService.Instance.Changed -= OnNet;
        }

        public static SummonWindow Attach(RectTransform body)
        {
            var w = body.gameObject.AddComponent<SummonWindow>();
            w.Build(body);
            return w;
        }

        public void Refresh()
        {
            var net = Roi.Net.OnlineService.Instance;
            if (!net.LoggedIn)
            {
                _wallet.text = "";
                _status.text = "";
                ShowConnect();
                return;
            }
            net.Run(net.Client.GachaState((ok, json) =>
            {
                if (!ok) { _status.text = Loc.Get("UI_EXCAVATION_STATE_FAILED"); return; }
                _catalogPreview=false;
                _stateJson = json;
                Rebuild();
            }));
        }

        void OnNet()
        {
            if (isActiveAndEnabled) Refresh();
        }

        /// `confirm` = the player has SEEN a 402's price and agreed to it. The idem key is made
        /// once per attempt and REUSED on the confirm, because the quote and the confirmed roll are
        /// the same call — a fresh key on the confirm would be a second, separately charged pull.
        bool PrepareAttempt(int count)
        {
            if(!string.IsNullOrEmpty(_pendingKey))return _pendingCount==count&&_pendingBanner==_selected;
            _pendingKey=System.Guid.NewGuid().ToString("N");_pendingCount=count;_pendingBanner=_selected;return true;
        }

        void Roll(int count, bool confirm, ulong quoted)
        {
            if(_requestInFlight||_catalogPreview)return;
            var net = Roi.Net.OnlineService.Instance;
            if (!net.LoggedIn) { Refresh(); return; }
            if(!PrepareAttempt(count))return;

            _requestInFlight=true;RenderActionState(net.LoggedIn);
            _status.text = Loc.Get("UI_SUMMON_ROLLING");
            net.Run(net.Client.GachaRoll(_selected, count, _pendingKey, confirm, quoted, o =>
            {
                _requestInFlight=false;
                if (o.NeedsConfirm)
                {
                    _status.text = string.Format(Loc.Get("UI_EXCAVATION_CONFIRM_CRYSTALS"),o.Crystal);
                    _quoteCount = count; _quoteCrystal = o.Crystal;
                    Rebuild();
                    return;
                }
                if (!o.Ok) { if(o.Code>=400&&o.Code<500&&o.Code!=408&&_quoteCount==0)_pendingKey=""; _status.text = Loc.Get("UI_EXCAVATION_REQUEST_FAILED"); RenderActionState(net.LoggedIn); if(_quoteCount>0)ShowConfirm(_quoteCount,_quoteCrystal); return; }
                // The quote is SPENT once it is honoured — leaving Confirm on screen after a
                // successful pull offers to buy something that has already been bought.
                _pendingKey = ""; _quoteCount = 0; _quoteCrystal = 0;
                _lastPulls = o.Json;
                Collect(o.Json);
                Rebuild();
                Refresh();     // the wallet and the pity ladder both moved
            }));
        }

        /// Claim this roll's mail and put it in the save, so the player never has to.
        ///
        /// Sequential on purpose: each claim is a server write followed by a sim event, and firing
        /// them together would race the sim's input log for no gain — a ten-pull is ten quick calls,
        /// not a bulk operation worth inventing.
        ///
        /// A failure here is reported and NOT swallowed. The mail is still on the server if the
        /// claim never happened, so the honest thing is to say so and point at the mailbox rather
        /// than let a paid reward disappear quietly.
        void Collect(string rollJson)
        {
            if (_driver == null) _driver = Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var sim = _driver != null ? _driver.Session : null;
            var rows = MailRows(rollJson);
            int got = 0, failed = 0;
            foreach (var row in rows)
            {
                var m = System.Text.RegularExpressions.Regex.Match(row, "\"id\":(\\d+)");
                if (sim == null || !m.Success) { failed++; continue; }
                if (MailWindow.ClaimFrom(sim, ulong.Parse(m.Groups[1].Value), row)) got++; else failed++;
            }
            // "collected", not "added to your inventory" — a pull's carrier is an Exemplar or a
            // Regalia, and those NEVER enter the bag: the sim banks them into their own uncapped,
            // account-bound collections so a reward can't be lost to a full inventory. The reward
            // can also be plain Gil. One line cannot name the right container, so it names none.
            _collected = failed == 0
                ? string.Format(Loc.Get("UI_EXCAVATION_COLLECTED"),got)
                : string.Format(Loc.Get("UI_EXCAVATION_MAIL_LEFT"),got,failed);
        }

        /// The mail ROWS this roll created, out of its own reply — full objects, not just ids, so
        /// the claim can be built without another request. The reply nests them under "mails".
        static List<string> MailRows(string rollJson)
            => MailWindow.RowsUnder(rollJson, "\"mails\":");

        void ShowConfirm(int count, ulong crystal)
        {
            if(GameObject.Find("ExcavationConfirmation")!=null)return;
            UiConfirm.AskExcavation(count, () => Roll(count, true, crystal), crystal,
                () => { if(_requestInFlight)return; _quoteCount=0; _quoteCrystal=0; _pendingKey=""; _status.text=""; Rebuild(); });
        }

        void Line(RectTransform parent, string text, float size, Color col, float y)
        {
            var t = UiTheme.Label(parent, "L" + y, text, size, col, TextAlignmentOptions.TopLeft);
            var rt = t.rectTransform;
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = new Vector2(14f, 8f); rt.offsetMax = new Vector2(-14f, -y);
        }

        /// The pity block is a MAP keyed by banner type, which JsonUtility cannot parse. One
        /// targeted read beats declaring a DTO per banner kind that would need a new field every
        /// time the server adds one.
        int PityCount(string kind)
        {
            string key = kind == "Newcomer" ? "standard" : kind.ToLowerInvariant();
            var block = System.Text.RegularExpressions.Regex.Match(_stateJson,
                "\"" + System.Text.RegularExpressions.Regex.Escape(key) + "\"\\s*:\\s*\\{([^}]*)}",
                System.Text.RegularExpressions.RegexOptions.IgnoreCase);
            var m = System.Text.RegularExpressions.Regex.Match(block.Value, "\"count\"\\s*:\\s*(\\d+)");
            return m.Success && int.TryParse(m.Groups[1].Value, out var count) ? count : 0;
        }

        /// The wallet numbers arrive as decimal STRINGS: they are u64 and a JSON number would lose
        /// the low bits in a float parser — the same reason the login's seed is a string.
        static long Num(string s) => long.TryParse(s, out var v) ? v : 0;

        void Clear()
        {
            foreach (var go in _built) {if(go!=null)go.SetActive(false);Destroy(go);}
            _built.Clear();
        }

        [System.Serializable]
        class State
        {
            public string tradeable, bound, cost_per_pull, coupons, coupon_price, mileage;
            public Banner[] banners;
        }
        [System.Serializable]
        class Banner
        {
            public string id, kind, featured, ticket_kind;
            public string[] pool;
            public long tickets;
            public bool open;
            public int base_bp, soft, hard, ramp_bp;
            public bool fifty_fifty, has_pity, carry_guarantee;
        }
        [System.Serializable] class RollResult { public bool ok; public string reason; public PullRow[] pulls; }
        [System.Serializable] class PullRow { public string def_id, rarity; public bool featured; }
    }
}
