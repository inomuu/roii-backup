using System;
using System.Collections;
using System.Globalization;
using TMPro;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.UI
{
    // Presentation only: the sim owns Gil/Soul; the server owns the premium wallet.
    public sealed class HomeWalletBar : MonoBehaviour
    {
        readonly TextMeshProUGUI[] _values = new TextMeshProUGUI[3];
        readonly TextMeshProUGUI[] _names = new TextMeshProUGUI[3];
        readonly RectTransform[] _strips = new RectTransform[3];
        readonly RectTransform[] _icons = new RectTransform[3];
        readonly RectTransform[] _actions = new RectTransform[3];
        readonly string[] _layoutValues = new string[3];
        Vector2 _layoutSize;
        Roi.Sim.SimDriver _driver;
        Roi.Sim.SimSession _session;
        Roi.Net.OnlineService _net;
        Roi.Net.ServerClient _client;
        Coroutine _request;
        int _generation;
        float _nextPurse, _nextWallet;
        bool _built, _tactical;
        const string Unknown = "—";

        public static HomeWalletBar Create(Transform parent, Action<string> navigate, bool tactical=false)
        {
            var go = new GameObject("HomeWallet", typeof(RectTransform));
            go.transform.SetParent(parent, false);
            Place((RectTransform)go.transform, .475f, .905f, .884f, .987f);
            var bar = go.AddComponent<HomeWalletBar>();
            bar._tactical=tactical;
            bar.Build(navigate);
            bar.Activate();
            return bar;
        }

        void Build(Action<string> navigate)
        {
            var names = new[] {"Gil", "Soul", "Gem"};
            var labels = new[] {Loc.Get("UI_GIL"), Loc.Get("UI_SOUL"), Loc.Get("UI_HOME_GEM")};
            var routes = new[] {"Store", "Paths", "Crystal"};
            var font = _tactical ? (Loc.Language=="th"?UiTheme.Font:Resources.Load<TMP_FontAsset>("HomeUI/HomeHeading")??UiTheme.Font) : Loc.Language=="th" ? UiTheme.Font : Resources.Load<TMP_FontAsset>("HomeUI/HomeSerif") ?? UiTheme.DisplayFont;
            for (int i = 0; i < 3; i++)
            {
                int index = i;
                var strip = new GameObject(names[i], typeof(RectTransform), typeof(Image), typeof(Button));
                strip.transform.SetParent(transform, false);
                var rt = (RectTransform)strip.transform;
                _strips[i] = rt;
                var image = strip.GetComponent<Image>();
                image.color = UiTheme.WithAlpha(UiTheme.Panel,.92f);
                image.sprite=UiTheme.Fill();image.type=Image.Type.Sliced;
                var outline=UiTheme.Border(rt,UiTheme.WithAlpha(UiTheme.Accent,.65f));
                outline.sprite=UiTheme.Frame();outline.raycastTarget=false;
                if(_tactical){image.sprite=null;image.color=new Color(.025f,.04f,.055f,.65f);outline.enabled=false;}
                image.material = HomeIconSet.Material; // Separate alpha blend for the offscreen UI camera.
                var button = strip.GetComponent<Button>();
                button.targetGraphic = image;
                button.navigation = new Navigation {mode = Navigation.Mode.None};
                button.onClick.AddListener(() => navigate?.Invoke(routes[index]));
                var colors = button.colors;
                colors.highlightedColor = new Color(1.35f,1.35f,1.35f,1f);
                colors.pressedColor = new Color(.65f,.65f,.65f,1f);
                button.colors = colors;

                var iconGo = new GameObject("CurrencyIcon", typeof(RectTransform), typeof(CanvasRenderer), typeof(HomeCurrencyIcon));
                iconGo.transform.SetParent(rt, false);
                _icons[i] = (RectTransform)iconGo.transform;
                var icon = iconGo.GetComponent<HomeCurrencyIcon>();
                icon.Kind = names[i]; icon.color = Color.white; icon.raycastTarget = false;

                _names[i] = Label(rt, "Name", labels[i], Loc.Language=="th"?16:21, new Color(.94f,.95f,.95f,.88f), font);
                _values[i] = Label(rt, "Balance", Unknown, 28, Color.white, font);
                _values[i].enableAutoSizing = true; _values[i].fontSizeMin = 21; _values[i].fontSizeMax = 28;

                var actionGo = new GameObject("NavigateIcon", typeof(RectTransform), typeof(CanvasRenderer), typeof(HomeCurrencyIcon));
                actionGo.transform.SetParent(rt, false);
                _actions[i] = (RectTransform)actionGo.transform;
                var action = actionGo.GetComponent<HomeCurrencyIcon>();
                action.Kind = i == 1 ? "Arrow" : "Plus";
                action.color = new Color(.97f,.97f,.94f); action.raycastTarget = false;
                action.material = HomeIconSet.Material;
            }
            _built = true;
        }

        // Reflow only when a balance or the canvas size changes. The action follows the
        // actual text width; the per-currency cap leaves room for three long balances.
        void LateUpdate()
        {
            if (!_built) return;
            var size = ((RectTransform)transform).rect.size;
            bool changed = size != _layoutSize;
            for (int i = 0; i < 3; i++) changed |= _layoutValues[i] != _values[i].text;
            if (!changed || size.x <= 0 || size.y <= 0) return;
            _layoutSize = size;
            bool compact=_tactical || Screen.safeArea.width/Mathf.Max(1,Screen.safeArea.height)>1.7f;
            float height = size.y * .90f;
            float icon = Mathf.Min(compact?48:36, height * .54f);
            float action = compact?0:Mathf.Min(20, height * .35f);
            const float gap = 12, inset = 6, iconGap = 7, actionGap = 5;
            float maxWidth = (size.x - gap * 2) / 3;
            float maxValue = Mathf.Max(20, maxWidth - inset * 2 - icon - iconGap - actionGap - action);
            float cursor = size.x;
            for (int i = 2; i >= 0; i--)
            {
                var value = _values[i];
                _layoutValues[i] = value.text;
                value.fontSizeMin=compact?32:27;value.fontSizeMax=compact?48:36;
                value.fontSize = value.fontSizeMax;
                _names[i].gameObject.SetActive(!compact);_actions[i].gameObject.SetActive(!compact);
                if(_tactical){value.fontSizeMin=22;value.fontSizeMax=34;value.fontSize=34;}
                float valueWidth = Mathf.Clamp(value.GetPreferredValues(value.text).x + 3, 24, maxValue);
                if(_tactical)valueWidth=maxValue;
                float width = inset * 2 + icon + iconGap + valueWidth + actionGap + action;
                cursor -= width;
                Box(_strips[i], cursor, size.y * .05f, width, height);
                Box(_icons[i], inset, (height - icon) * .5f, icon, icon);
                float textX = inset + icon + iconGap;
                Box(_names[i].rectTransform, textX, height * .62f, Mathf.Max(valueWidth+actionGap+action,60), Mathf.Max(22, height * .32f));
                Box(value.rectTransform, textX, height * .10f, valueWidth, height * (compact?.80f:.54f));
                Box(_actions[i], textX + valueWidth + actionGap,
                    height * .40f - action * .5f, action, action);
                cursor -= gap;
            }
        }

        static void Box(RectTransform rect, float x, float y, float width, float height)
        {
            rect.anchorMin = rect.anchorMax = rect.pivot = Vector2.zero;
            rect.anchoredPosition = new Vector2(x, y);
            rect.sizeDelta = new Vector2(width, height);
        }

        static TextMeshProUGUI Label(Transform parent, string name, string text, float size, Color color, TMP_FontAsset font)
        {
            var label = UiTheme.Label(parent,name,text,size,color,TextAlignmentOptions.MidlineLeft);
            if (font != null) {label.font=font; label.fontSharedMaterial=font.material;}
            label.textWrappingMode=TextWrappingModes.NoWrap;
            label.overflowMode=TextOverflowModes.Ellipsis;
            label.raycastTarget=false;
            return label;
        }

        static void Place(RectTransform rect,float x,float y,float right,float top)
        {
            rect.anchorMin=new Vector2(x,y); rect.anchorMax=new Vector2(right,top);
            rect.offsetMin=rect.offsetMax=Vector2.zero;
        }

        void OnEnable() {if (_built) Activate();}
        void Activate()
        {
            _net=Roi.Net.OnlineService.Instance;
            _net.Changed-=OnConnection;
            _net.Changed+=OnConnection;
            _nextPurse=0;
            ResetWallet();
        }
        void OnDisable()
        {
            if (_net!=null) _net.Changed-=OnConnection;
            CancelRequest();
            _session=null; _client=null;
        }
        void OnConnection() {ResetWallet();}
        void CancelRequest()
        {
            _generation++;
            if (_request!=null) StopCoroutine(_request);
            _request=null;
        }
        void ResetWallet()
        {
            CancelRequest();
            _client=_net!=null ? _net.Client : null;
            _nextWallet=0;
            if (_values[2]!=null) _values[2].text=Unknown;
        }

        void Update()
        {
            if (!_built || Time.unscaledTime<_nextPurse) return;
            _nextPurse=Time.unscaledTime+.5f;
            if (_driver==null) _driver=UnityEngine.Object.FindFirstObjectByType<Roi.Sim.SimDriver>();
            var current=_driver!=null ? _driver.Session : null;
            if (!ReferenceEquals(current,_session))
            {
                _session=current;
                ResetWallet();
            }
            if (_session!=null && _session.IsAlive)
            {
                var purse=JsonUtility.FromJson<Purse>(_session.QueryJson("purse"));
                _values[0].text=purse!=null ? purse.gil.ToString("N0",CultureInfo.InvariantCulture) : Unknown;
                _values[1].text=purse!=null ? purse.soul.ToString("N0",CultureInfo.InvariantCulture) : Unknown;
            }
            else {_values[0].text=Unknown; _values[1].text=Unknown;}

            if (_net==null || !_net.LoggedIn) return;
            if (!ReferenceEquals(_client,_net.Client)) ResetWallet();
            if (_request==null && Time.unscaledTime>=_nextWallet)
            {
                _nextWallet=Time.unscaledTime+30f;
                _request=StartCoroutine(ReadWallet(_client,_session,_generation));
            }
        }

        IEnumerator ReadWallet(Roi.Net.ServerClient client,Roi.Sim.SimSession session,int generation)
        {
            long account=client.AccountId;
            yield return client.GachaState((ok,json) =>
            {
                if (this==null || !isActiveAndEnabled || generation!=_generation || _net==null
                    || !ReferenceEquals(client,_net.Client) || client.AccountId!=account
                    || !ReferenceEquals(session,_session) || _driver==null
                    || !ReferenceEquals(session,_driver.Session)) return;
                _values[2].text=Unknown;
                if (!ok) return;
                try
                {
                    var wallet=JsonUtility.FromJson<Wallet>(json);
                    if (wallet!=null && wallet.ok
                        && ulong.TryParse(wallet.tradeable,NumberStyles.None,CultureInfo.InvariantCulture,out var paid)
                        && ulong.TryParse(wallet.bound,NumberStyles.None,CultureInfo.InvariantCulture,out var earned))
                    {
                        // Decimal keeps the sum exact even when both buckets approach UInt64.MaxValue.
                        _values[2].text=((decimal)paid+earned).ToString("N0",CultureInfo.InvariantCulture);
                    }
                }
                catch (ArgumentException) { /* Unavailable payload stays an em dash. */ }
            });
            if (generation==_generation) _request=null;
        }

        [Serializable] sealed class Purse {public long gil,soul;}
        [Serializable] sealed class Wallet {public bool ok; public string tradeable,bound;}
    }
}
