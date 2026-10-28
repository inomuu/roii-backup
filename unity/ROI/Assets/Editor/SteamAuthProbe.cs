// THE STEAM DOOR, as far as it can be walked without the SDK.
//
// Three things are checkable today and all three are the ones that break silently:
//   the FAÇADE reports unavailable rather than throwing, so a machine with no Steam still boots;
//   the CLIENT builds the request the server expects — a route that answers at all proves the
//     path, the method and the body shape, which is most of what a typo breaks;
//   a REJECTION carries the server's own words, because "login failed" hides the difference
//     between an unset publisher key and a ticket Steam refused.
//
// A REAL ticket is checked too, but only when the SDK is compiled in, Steam is signed in AND the
// server holds a publisher key. Any of those missing is reported as a SKIP naming which one — a
// probe that passed quietly on the façade alone would imply a door it never opened.
//
//   echo exec:SteamAuthProbe.Run > unity/ROI/.roi_cmd
using System.Collections;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class SteamAuthProbe
{
    static int _frame;

    public static void Run()
    {
        SessionState.SetString("ROI_BOOT", "dev");
        EditorSettings.enterPlayModeOptionsEnabled = true;
        EditorSettings.enterPlayModeOptions =
            EnterPlayModeOptions.DisableDomainReload | EnterPlayModeOptions.DisableSceneReload;
        _frame = 0;
        if (!EditorApplication.isPlaying)
        {
            EditorSceneManager.OpenScene(ProbeScene.Path, OpenSceneMode.Single);
            EditorApplication.playModeStateChanged += OnState;
            EditorApplication.EnterPlaymode();
        }
        else EditorApplication.update += Tick;
    }

    static void OnState(PlayModeStateChange s)
    {
        if (s != PlayModeStateChange.EnteredPlayMode) return;
        EditorApplication.playModeStateChanged -= OnState;
        EditorApplication.update += Tick;
    }

    static void Tick()
    {
        if (++_frame < 150) return;
        EditorApplication.update -= Tick;
        Roi.Net.OnlineService.Instance.Run(ToEnd(Drive()));
    }

    static IEnumerator ToEnd(IEnumerator inner)
    {
        yield return inner;
        EditorApplication.isPlaying = false;
    }

    static IEnumerator Drive()
    {
        // 1. THE FAÇADE. Without the SDK this must be a quiet false, not an exception — the game
        // has to boot on a machine that has never heard of Steam.
        bool available = Roi.Net.SteamAuth.Init();
        Debug.Log($"STEAMAUTH façade: available={available} — {Roi.Net.SteamAuth.Status}");
        Report("the façade answers instead of throwing", true, available ? "SDK present" : "no SDK, falls back");

        string ticket = null;
        yield return Roi.Net.SteamAuth.GetWebTicket(t => ticket = t);
        Report("a ticket request always answers", ticket != null,
               string.IsNullOrEmpty(ticket) ? "empty, as expected without Steam" : $"{ticket.Length / 2} bytes");

        // 2. THE REQUEST. A bogus ticket is the point: the server must reach its own validation and
        // answer in its own words. Anything else — a 404, a 405, a parse error — is this client
        // building the request wrong, which is what no screenshot would ever show.
        var client = new Roi.Net.ServerClient("http://127.0.0.1:8787");
        bool ok = true; string why = "";
        yield return client.LoginSteam("00ff", (o, m) => { ok = o; why = m; });
        Report("the server answered the steam route", !ok && !string.IsNullOrEmpty(why), why);

        // 3. AND IT SAID WHY. The two answers worth telling apart: no publisher key on this deploy,
        // or a ticket Steam itself rejected.
        bool named = why.Contains("steam") || why.Contains("ticket") || why.Contains("not configured");
        Report("the rejection carries the server's reason", named, why);

        // 4. THE REAL DOOR. Everything above is still reachable with no SDK at all — it proves the
        // shape of the request, never that a ticket Steam minted is one this server accepts. This
        // leg is the only one that does, and it needs three things at once: the SDK compiled in, a
        // signed-in Steam client, and STEAM_PUBLISHER_KEY on the server. Each missing piece is
        // reported as a SKIP with the one command that fixes it — a skip is not a pass, and saying
        // which of the three is absent is the whole reason this is not a single boolean.
        if (!available)
        {
            Debug.Log("STEAMAUTH SKIP — a real ticket needs Steamworks.NET, ROI_STEAM defined, and a "
                    + "running Steam client. The façade reported: " + Roi.Net.SteamAuth.Status);
        }
        else if (why.Contains("not configured"))
        {
            Debug.Log("STEAMAUTH SKIP — the server has no STEAM_PUBLISHER_KEY, so a real ticket "
                    + "cannot be validated. Start it with tools/local_server.sh after writing the "
                    + "key to .secrets/steam_publisher_key.");
        }
        else
        {
            bool loggedIn = false; string detail = "";
            yield return client.LoginSteam(ticket, (o, m) => { loggedIn = o; detail = m; });
            Report("a real Steam ticket is accepted", loggedIn, loggedIn
                ? $"account {client.AccountId}, sim {client.ServerSimVersion}" : detail);

            // An account is what the ticket is FOR. A token with no account behind it would still
            // read as a successful login and break at the first authenticated call instead.
            Report("the login resolved to an account", client.AccountId > 0 && !string.IsNullOrEmpty(client.Token),
                   $"id={client.AccountId}");

            // Twice, on a FRESH ticket, must land on the SAME account. This is the assertion that
            // account_identities exists to satisfy: a second sign-in resolves the identity it
            // already stored instead of minting a second account for the same person.
            long first = client.AccountId;
            string again = null;
            yield return Roi.Net.SteamAuth.GetWebTicket(t => again = t);
            var second = new Roi.Net.ServerClient("http://127.0.0.1:8787");
            bool twice = false; string why2 = "";
            yield return second.LoginSteam(again, (o, m) => { twice = o; why2 = m; });
            Report("signing in again lands on the same account", twice && second.AccountId == first,
                   twice ? $"{first} then {second.AccountId}" : why2);
        }
        Debug.Log("STEAMAUTH done");
    }

    static void Report(string what, bool ok, string detail)
    {
        if (ok) Debug.Log($"STEAMAUTH OK — {what}: {detail}");
        else Debug.LogError($"STEAMAUTH FAILED — {what}: {detail}");
    }
}
