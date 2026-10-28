// CRYSTAL — the money-in half, ported from `store_view.gd`. Its twin is Excavation, which spends
// what this sells; they stay apart because the server keeps them apart (`store.rs` turns money into
// Crystal, `gacha.rs` turns Crystal into units), and neither screen has to know how the other is
// priced.
//
// EVERY NUMBER IS THE SERVER'S. The ladder, the bonus and the currency all arrive from `GET /store`
// and `GET /gacha/state`; this file formats them and nothing else. A client that computed a price
// would eventually disagree with what the store charges, and the player would be right.
//
// THE CURRENCY IS THE SERVER'S CHOICE TOO. `/store/checkout` asks the platform which currency the
// buyer's wallet is in and quotes that one, so the catalog ships every currency and this screen can
// only show the base one. Guessing from the device locale is confidently wrong for anyone
// travelling or on a foreign account.
//
// BUYING IS NOT WIRED HERE, and the screen says so rather than offering a button that does nothing:
// a purchase needs the platform overlay (`/store/checkout` opens it, `/store/finalize` grants), and
// this client has no platform SDK. Everything up to that point — the catalog, the wallet, the
// sandbox flag — is real.
using System.Collections.Generic;
using TMPro;
using UnityEngine;

namespace Roi.UI
{
    public sealed class CrystalWindow : MonoBehaviour
    {
        const float RowH = 56f, Gap = 6f;
        /// What the catalog is priced in for display. The real charge is quoted at checkout.
        const string BaseCurrency = "USD";

        RectTransform _content;
        TextMeshProUGUI _wallet, _status;
        readonly List<GameObject> _built = new();
        string _catalog = "", _gacha = "";

        public static CrystalWindow Attach(RectTransform body)
        {
            var w = body.gameObject.AddComponent<CrystalWindow>();
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
            _wallet = UiTheme.Label(body, "Wallet", "", UiTheme.HeadSize, UiTheme.Accent,
                                    TextAlignmentOptions.Right);
            var wr = _wallet.rectTransform;
            wr.anchorMin = new Vector2(0f, 1f); wr.anchorMax = new Vector2(1f, 1f);
            wr.pivot = new Vector2(0.5f, 1f);
            wr.offsetMin = new Vector2(pad, -40f); wr.offsetMax = new Vector2(-pad, -UiTheme.PadSmall);

            _status = UiTheme.Label(body, "Status", "", UiTheme.CaptionSize, UiTheme.TextLow,
                                    TextAlignmentOptions.Left);
            _status.textWrappingMode = TextWrappingModes.NoWrap;
            var sr = _status.rectTransform;
            sr.anchorMin = new Vector2(0f, 1f); sr.anchorMax = new Vector2(1f, 1f);
            sr.pivot = new Vector2(0.5f, 1f);
            sr.offsetMin = new Vector2(pad, -40f); sr.offsetMax = new Vector2(-(pad + 300f), -UiTheme.PadSmall);

            var host = new GameObject("Scroll", typeof(RectTransform));
            host.transform.SetParent(body, false);
            var hrt = (RectTransform)host.transform;
            hrt.anchorMin = Vector2.zero; hrt.anchorMax = Vector2.one;
            hrt.offsetMin = new Vector2(pad, UiTheme.Pad);
            hrt.offsetMax = new Vector2(-pad, -(40f + UiTheme.Gap));
            _content = UiScroll.Create(hrt, out _);
        }

        public void Refresh()
        {
            var net = Roi.Net.OnlineService.Instance;
            if (!net.LoggedIn)
            {
                _wallet.text = "";
                _status.text = "";
                Clear();
                var notice=MenuNotice.Create(_content,"Crystal",Loc.Get("UI_NOTICE_CONNECTION"),Loc.Get("UI_NOTICE_CRYSTAL"),()=>net.Connect());
                _built.Add(notice);
                UiScroll.Fit(_content,348f);
                return;
            }
            // TWO fetches, and the screen draws when the catalog lands: the wallet is a header, and
            // waiting for it to show a price list nobody's balance changes would be waiting twice.
            net.Run(net.Client.GachaState((ok, json) =>
            {
                if (ok) { _gacha = json; Rebuild(); }
            }));
            net.Run(net.Client.StoreCatalog((ok, json) =>
            {
                if (!ok) { _status.text = "catalog failed: " + json; return; }
                _catalog = json;
                Rebuild();
            }));
        }

        void Rebuild()
        {
            Clear();
            var cat = JsonUtility.FromJson<Catalog>(_catalog) ?? new Catalog();
            var wallet = JsonUtility.FromJson<Wallet>(_gacha) ?? new Wallet();

            _wallet.text = $"{Num(wallet.tradeable) + Num(wallet.bound):N0} {GameText.Crystal}";
            _status.text = cat.sandbox
                ? "Store SANDBOX — no real charge is made."
                : "Purchases are not wired in this build.";

            float y = 0f;
            foreach (var sku in cat.skus ?? new Sku[0])
            {
                var row = UiTheme.Box(_content, "S_" + sku.sku, UiTheme.Surface);
                UiTheme.Border(row.rectTransform, UiTheme.Line);
                Place(row.rectTransform, y, RowH);
                _built.Add(row.gameObject);

                var amount = UiTheme.Label(row.rectTransform, "A",
                                           $"{sku.crystal:N0} {GameText.Crystal}", UiTheme.HeadSize,
                                           UiTheme.TextHigh, TextAlignmentOptions.Left);
                Inset(amount, UiTheme.Pad, 320f, RowH * 0.5f, 0f);

                // The FIRST-BUY BONUS is the offer, so it gets its own line rather than being
                // folded into the amount — a doubled first purchase is the reason to pick a tier.
                if (sku.first_buy_bonus > 0)
                {
                    var bonus = UiTheme.Label(row.rectTransform, "B",
                                              $"+{sku.first_buy_bonus:N0} on your first purchase",
                                              UiTheme.CaptionSize, UiTheme.Accent,
                                              TextAlignmentOptions.Left);
                    Inset(bonus, UiTheme.Pad, 320f, 0f, -RowH * 0.5f);
                }

                var price = UiTheme.Label(row.rectTransform, "P", Price(sku), UiTheme.HeadSize,
                                          UiTheme.TextMid, TextAlignmentOptions.Right);
                Inset(price, UiTheme.Pad, UiTheme.Pad, 0f, 0f);
                y += RowH + Gap;
            }

            var note = UiTheme.Label(_content, "Note",
                $"Prices shown in {BaseCurrency}. The store charges in your own currency, quoted "
                + "when the purchase opens.",
                UiTheme.CaptionSize, UiTheme.TextLow, TextAlignmentOptions.TopLeft);
            note.textWrappingMode = TextWrappingModes.Normal;
            Place(note.rectTransform, y + UiTheme.Gap, 40f);
            _built.Add(note.gameObject);
            UiScroll.Fit(_content, y + 48f + UiTheme.Pad);
        }

        /// The base-currency price, formatted from MINOR units — the server sends cents, because a
        /// price in a float is a price that eventually rounds wrong.
        static string Price(Sku sku)
        {
            foreach (var p in sku.prices ?? new PriceRow[0])
                if (p.currency == BaseCurrency)
                    return $"{p.minor / 100}.{p.minor % 100:00} {p.currency}";
            var first = sku.prices != null && sku.prices.Length > 0 ? sku.prices[0] : null;
            return first != null ? $"{first.minor / 100}.{first.minor % 100:00} {first.currency}" : "—";
        }

        /// The wallet numbers arrive as decimal STRINGS: they are u64, and a JSON number would lose
        /// the low bits.
        static long Num(string s) => long.TryParse(s, out var v) ? v : 0;

        static void Inset(TextMeshProUGUI t, float left, float right, float bottom, float top)
        {
            var rt = t.rectTransform;
            rt.anchorMin = Vector2.zero; rt.anchorMax = Vector2.one;
            rt.offsetMin = new Vector2(left, bottom); rt.offsetMax = new Vector2(-right, top);
        }

        void Place(RectTransform rt, float y, float h)
        {
            rt.anchorMin = new Vector2(0f, 1f); rt.anchorMax = new Vector2(1f, 1f);
            rt.pivot = new Vector2(0.5f, 1f);
            rt.sizeDelta = new Vector2(0f, h);
            rt.anchoredPosition = new Vector2(0f, -y);
        }

        void Clear()
        {
            foreach (var go in _built) Destroy(go);
            _built.Clear();
        }

        [System.Serializable] class Catalog { public bool ok, sandbox; public Sku[] skus; }
        [System.Serializable] class Sku { public string sku; public PriceRow[] prices; public long crystal, first_buy_bonus; }
        [System.Serializable] class PriceRow { public string currency; public long minor; }
        [System.Serializable] class Wallet { public string tradeable, bound; }
    }
}
