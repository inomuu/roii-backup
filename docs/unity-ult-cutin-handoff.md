# Unity ultimate cut-in trigger — handoff 2026-09-23

From the sim/gameplay session to the Unity client (Codex). The sim side is done; the client
side is yours — the reference below is a suggestion, not committed code.

## Rule (user, 2026-09-23)

A party member's ultimate plays its cut-in the **first time it resolves in a session**, never
again until the next session (login / new `SimSession`). One per CHARACTER, not one per session
total: a five-unit party shows at most five cut-ins per session.

This replaces the sim-side "≤20 ultimates/hour" cap. Cut-in frequency is a presentation rule —
the sim no longer retunes an ultimate for how often it fires.

## Sim contract (new, sim-host view JSON)

```json
"ult_fx": [ { "caster_id": 1, "skill_id": "aegis" } ]
```

- Per-tick, like `skill_fx`: present every tick, usually `[]`; one entry per ultimate that
  resolved THIS tick. Pushed from sim-core `unit_rank::after_ult`, which every ult cast path
  goes through, so no cast is missed and nothing is inferred from the Limit Break gauge.
- `skill_id` is the ultimate's id; each unit has exactly one, so keying "already shown" on it
  is keying on the character.
- Monsters can appear as casters in principle — filter on the caster being a `Player`-team,
  non-pet entity.
- `SimDriver` publishes after a tick BATCH; after a catch-up of several ticks only the last
  tick's list is seen. For a once-per-session trigger a missed cast just means the next one
  shows it.

Native library: `tools/unity_dylib.sh` (already staged at the time of writing) + restart the
Editor.

## Where it should hang

Feed it from `ArenaView.OnView` beside `_death` / `_sound`: that callback only runs while the
LIVE sim is on screen (`SuspendLiveView` detaches it during a story stage), so no cut-in lands
over a mission. Reset the "shown" set when `_driver.Session` changes (DeathBanner's
`_seenSession` pattern). Mark an ult as shown only when its cut-in actually started — a cast
that lands while another cut-in is up, or before that unit has cut-in art, should leave the
first showing for a later cast.

Only Seraphine (`aegis` → `SeraphineUltimateCutin.Play()`) has a Unity cut-in today.

## Reference sketch

```csharp
// The gameplay trigger for ultimate cut-ins: a party member's ultimate plays its cut-in the FIRST
// time it resolves in a session, and never again until the next session (user 2026-09-23 — this
// replaced the sim-side ≤20 ults/hour cap, which throttled balance to fix a presentation problem).
//
// Fed from ArenaView.OnView, which only runs while the LIVE sim is on screen: a story stage
// suspends that feed, so no cut-in lands over a mission. The sim reports each ultimate in
// `ult_fx` (pushed from sim-core `unit_rank::after_ult`, the one funnel every ult path takes).
using System.Collections.Generic;
using Roi.Sim;
using UnityEngine;

namespace Roi.Cutin
{
    public sealed class UltCutinTrigger
    {
        readonly HashSet<string> _shown = new HashSet<string>();
        SimDriver _driver;
        SimSession _seenSession;

        public void Bind(SimDriver driver) => _driver = driver;

        public void OnView(string json)
        {
            if (_driver == null || _driver.Session == null) return;
            if (!ReferenceEquals(_seenSession, _driver.Session))
            {
                _seenSession = _driver.Session;
                _shown.Clear();
            }
            // Almost every tick carries no ultimate; skip the parse unless the list is non-empty.
            if (json == null || !json.Contains("\"ult_fx\":[{")) return;
            var v = JsonUtility.FromJson<View>(json);
            if (v?.ult_fx == null || v.grid?.entities == null) return;
            foreach (var f in v.ult_fx)
            {
                if (f == null || string.IsNullOrEmpty(f.skill_id) || _shown.Contains(f.skill_id)) continue;
                if (!IsParty(v.grid.entities, f.caster_id)) continue;
                // Marked only once it actually played: a cast that lands while another cut-in is
                // up (or one with no cut-in yet) leaves the first showing for a later cast.
                if (TryPlay(f.skill_id)) _shown.Add(f.skill_id);
            }
        }

        static bool IsParty(EntityDTO[] entities, long id)
        {
            foreach (var e in entities)
                if (e != null && e.id == id) return e.team == "Player" && !e.is_pet;
            return false;
        }

        /// Keyed by the ULTIMATE's skill id — one per unit, so "once per ultimate" is "once per
        /// character". A unit without a cut-in yet returns false and is retried on its next cast.
        static bool TryPlay(string ultId)
        {
            switch (ultId)
            {
                case "aegis": return SeraphineUltimateCutin.Play() != null;
                default: return false;
            }
        }

        [System.Serializable] class View { public UltFxDTO[] ult_fx; public GridDTO grid; }
        [System.Serializable] class UltFxDTO { public long caster_id; public string skill_id; }
        [System.Serializable] class GridDTO { public EntityDTO[] entities; }
        [System.Serializable] class EntityDTO { public long id; public string team; public bool is_pet; }
    }
}
```
