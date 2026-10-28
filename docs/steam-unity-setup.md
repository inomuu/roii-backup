# Steam login on the Unity client — what is built, and the four things left

The server half has been live since `/auth/steam` landed. The Unity half is built up to the SDK
call and stops there deliberately: the binding is a package this repo does not carry, so
everything that does not need it is written, wired and probed, and the ~20 lines that do are
behind `ROI_STEAM`.

## What the client does today

`SteamAuth.Available` is false without the package, so `OnlineService` takes the development door
and the title screen offers the name field. Nothing about that path changed.

With the package and the symbol, `OnlineService.Login` asks Steam for a **Web-API** auth ticket,
posts the hex to `/auth/steam`, and the server derives the steamid itself. A ticket Steam refuses
does **not** fall through to the dev door: signing in as somebody else because Steam had a bad
minute is worse than not signing in.

  SteamAuthProbe → the façade answers instead of throwing · the route answers in the server's own
  words (`steam auth not configured (STEAM_PUBLISHER_KEY unset)`, which is the 503 proving the
  path, method and body shape are right)

## Behaviour, in every state it can be in

| state | what happens |
|---|---|
| **no SDK** (today) | `Available` false. Name field editable, Play needs a name, login goes to `/auth/dev`. Nothing about the current path changed. |
| **SDK, Steam not running** | `SteamAPI.Init` fails, `Status` says so, and the client behaves exactly as the row above. The game boots. |
| **SDK + Steam, no publisher key** | Name field shows the persona, read-only. Play fetches a ticket, posts it, and the server answers 503: *"steam auth not configured"*. The screen prints that verbatim. |
| **SDK + Steam, ticket refused** | 403 in the server's own words — *"steam ticket rejected"*, or *"family sharing not allowed"*. |
| **everything set** | Ticket → token, seed, account id. From there the session is identical to the dev door's; the account key is `steam:<steamid>`, resolved through `account_identities`. |

**A refused ticket never falls through to the dev door.** Retry is pressing Play again — the
ticket is fetched fresh each time, so a Steam client that has come back to its senses just works.

## The four things left

**1. Add the binding.** Package Manager → **+** → *Add package from git URL*:

```
https://github.com/rlabrecque/Steamworks.NET.git?path=/com.rlabrecque.steamworks.net
```

(or download the `.unitypackage` from that repo's Releases and import it — same result, pinned by
hand instead of by tag.)

Pick a version built on **Steamworks SDK 1.57 or newer**: `GetAuthTicketForWebApi` does not exist
before that, and the older `GetAuthSessionTicket` is the SDK-to-SDK path, which the Web API
rejects. The SDK version is the condition; the package's own version number is not, so do not
trust a number — **the check is whether `SteamAuth.cs` compiles once `ROI_STEAM` is defined**. If
it does, the symbol is there and the version is new enough. If it does not, the package is too old
and no amount of reading release notes changes that.

⚠️ **Restart the Editor after installing.** Steamworks.NET ships a NATIVE library, and the Editor
holds one for the life of the process — the same rule `tools/unity_dylib.sh` prints every time it
stages `libsim_ffi`. On macOS, if the library will not load at all:
`xattr -dr com.apple.quarantine <the package's Plugins folder>`.

Facepunch.Steamworks would work too. Steamworks.NET is the recommendation here for one
repo-specific reason: it wraps the C API 1:1 exactly as GodotSteam does, so `SteamAuth.cs` and
`steam_auth.gd` read as ports of each other rather than two idioms.

**2. `steam_appid.txt`** containing `4958230`, beside the Unity project (for the Editor) and beside
the built player. Same file `godot/steam_appid.txt` already is.

**3. `ROI_STEAM`** in *Edit → Project Settings → Player → Other Settings → Scripting Define
Symbols*. Add it per build target you care about; the Editor uses the Standalone one.

**4. `fly secrets set STEAM_PUBLISHER_KEY=…`** — never in the repo, never in chat. Until it is set,
`/auth/steam` answers 503 and says so, which is what the probe currently asserts.

## Two details that fail silently if changed

**The identity string is EMPTY on both sides.** The client calls
`GetAuthTicketForWebApi(null)`; the server calls `AuthenticateUserTicket` with **no** `identity`
parameter at all (an empty one makes Steam answer error 104). A ticket minted for a different
identity is refused with nothing on either side explaining why.

**The steamid the client knows is never sent.** It is a public number off a profile URL. The
server derives it from the ticket, which is the whole reason the publisher key is server-only.

## What F2P changes about the rest of it

The family-sharing block in `auth_steam` (`ownersteamid != steamid` → reject) becomes a no-op for
a free game: Valve does not family-share F2P titles, everyone installs their own copy. Keep it —
it costs nothing — but the "one copy, one account" argument behind it does not survive F2P, and
the defences that do are elsewhere: earned Crystals land in the **bound** bucket, drop rates are
crowd-scaled, and an idle game's alts are bounded by wall-clock rather than by attention.

Two that are not covered yet, both noted in the audit: story first-clear pays **tradeable** Gil
per account, and `/store/refund` is dev-gated — a manual lever, where a gacha wants a reconcile
job (Steam does not push a refund webhook).
