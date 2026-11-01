// THE PARTY IS DOWN — say so, count down, and put them back. Ported from `sim_driver.gd`'s
// `_check_player_alive_state` and `modals_view.gd`'s death box, which between them already had every
// rule this needs; none of it was carried over with the rest of the client, so a wipe in a band zone
// left the party lying there with no screen, no button and no way back.
//
// PARTY-AWARE, not member-aware. One member falling is a fight in progress; the run only ends when
// every non-pet member is down. Getting that wrong shows a death screen mid-battle.
//
// A DUNGEON WIPE IS NOT THIS. `last_clear` means the fail summary is up, and that screen owns the
// decision — it runs its own countdown and offers Retry. Auto-restarting underneath it would yank
// the party away while the player is still reading what happened.
//
// AUTOMATIC, AND THE BANNER IS NOT A BUTTON. Godot's comment is the reasoning and it holds harder
// here: this is an idle game, so the one thing a death must never do is stop the game until somebody
// comes back to press something. `InputEvent::RestartInstance` is the same event the dungeon's Retry
// sends — the sim keeps XP, job and soul, and drops the loot escrow.
using System.Collections;
using UnityEngine;
using UnityEngine.UI;
using TMPro;

namespace Roi.UI
{
    public sealed class DeathBanner : MonoBehaviour
    {
        /// `PLAYER_RESPAWN_DELAY` in `sim_driver_core.gd`. Long enough to read what happened, short
        /// enough that an idle session does not stall on it.
        public const float RespawnDelay = 3f;

        Roi.Sim.SimDriver _driver;
        Roi.Sim.SimSession _seenSession;
        Image _box;
        TextMeshProUGUI _title, _count;
        float _deadSince = -1f;    // negative = the party is up
        bool _restartPending;
        bool _summaryOwnsIt;

        public static DeathBanner Create(Transform parent)
        {
            var go = new GameObject("DeathBanner", typeof(RectTransform));
            go.transform.SetParent(parent, false);
            var b = go.AddComponent<DeathBanner>();
            b.Build((RectTransform)go.transform);
            return b;
        }

        void Build(RectTransform root)
        {
            root.anchorMin = new Vector2(0.5f, 0.5f);
            root.anchorMax = new Vector2(0.5f, 0.5f);
            root.sizeDelta = new Vector2(420f, 120f);
            root.anchoredPosition = Vector2.zero;

            _box = UiTheme.Box(root, "Box", new Color(0.06f, 0.05f, 0.06f, 0.88f));
            DialogSurface.Apply(_box);
            var br = _box.rectTransform;
            br.anchorMin = Vector2.zero; br.anchorMax = Vector2.one;
            br.offsetMin = Vector2.zero; br.offsetMax = Vector2.zero;
            _box.raycastTarget = false;   // NOT a button: it must never eat a tap meant for the game

            _title = UiTheme.Caps(br, "Title", "", UiTheme.HeadSize,
                                  new Color(1f, 0.45f, 0.42f), TextAlignmentOptions.Center);
            var tr = _title.rectTransform;
            tr.anchorMin = new Vector2(0f, 0.5f); tr.anchorMax = new Vector2(1f, 1f);
            tr.offsetMin = Vector2.zero; tr.offsetMax = Vector2.zero;
            _title.raycastTarget = false;

            _count = UiTheme.Label(br, "Countdown", "", UiTheme.BodySize,
                                   UiTheme.TextMid, TextAlignmentOptions.Center);
            var cr = _count.rectTransform;
            cr.anchorMin = Vector2.zero; cr.anchorMax = new Vector2(1f, 0.5f);
            cr.offsetMin = Vector2.zero; cr.offsetMax = Vector2.zero;
            _count.raycastTarget = false;

            DialogSurface.CloseIcon(br,()=>_box.gameObject.SetActive(false));
            gameObject.SetActive(false);
        }

        public void Bind(Roi.Sim.SimDriver driver) => _driver = driver;

        void ResetCountdown()
        {
            _deadSince = -1f;
            _restartPending = false;
            gameObject.SetActive(false);
        }

        public void OnView(string json)
        {
            if (_driver == null || _driver.Session == null || !_driver.Session.IsAlive) return;
            if (!ReferenceEquals(_seenSession, _driver.Session))
            {
                _seenSession = _driver.Session;
                ResetCountdown();
            }
            var v = JsonUtility.FromJson<View>(json);
            if (v?.grid?.entities == null) return;

            bool anyUp = false, hasParty = false;
            foreach (var e in v.grid.entities)
            {
                if (e == null || e.team != "Player" || e.is_pet) continue;
                hasParty = true;
                if (e.alive) { anyUp = true; break; }
            }

            if (anyUp || !hasParty)
            {
                ResetCountdown();
                return;
            }

            if (_deadSince < 0f)
            {
                _box.gameObject.SetActive(true);
                _deadSince = Time.unscaledTime;
                // Read ONCE, on the edge. A dungeon summary that appears later must not silently
                // take over a countdown already running, and one that is up now must own it for the
                // whole wipe rather than flickering as the field changes.
                _summaryOwnsIt = json.IndexOf("\"last_clear\"", System.StringComparison.Ordinal) >= 0;
                gameObject.SetActive(!_summaryOwnsIt);
                if (_summaryOwnsIt) return;
                _title.text = Loc.Get("UI_DEATH_TITLE");
            }
            if (_summaryOwnsIt) return;

            float left = Mathf.Max(0f, RespawnDelay - (Time.unscaledTime - _deadSince));
            _count.text = Loc.Fmt("UI_DEATH_RESPAWN_IN", left);
            if (left > 0f || _restartPending) return;

            // Finish distributing this dead view before publishing the restarted one. Run on
            // the driver: Companion/Menu can hide this banner's entire hierarchy.
            _restartPending = true;
            _driver.StartCoroutine(RestartAfterView(_seenSession, _deadSince));
        }

        IEnumerator RestartAfterView(Roi.Sim.SimSession session, float deadSince)
        {
            yield return null;
            // A save switch or revival during the deferred frame cancels this request.
            if (this == null || _driver == null || !ReferenceEquals(_driver.Session, session)
                || !_restartPending || _deadSince != deadSince || _summaryOwnsIt) yield break;
            _restartPending = false;
            // The driver resolves the CURRENT handle and publishes the result to every view.
            // Parsing an event is not proof it succeeded: OnView resets only on actual revival.
            _driver.ApplyInput("\"RestartInstance\"");
            if (_deadSince < 0f)
                Debug.Log("DEATH: party revived — restarted the current instance");
            else
                _deadSince = Time.unscaledTime; // refused/no-op: bounded retry, no false success log
        }

        [System.Serializable] class View { public Grid grid; }
        [System.Serializable] class Grid { public Ent[] entities; }
        [System.Serializable] class Ent { public string team; public bool alive; public bool is_pet; }
    }
}
