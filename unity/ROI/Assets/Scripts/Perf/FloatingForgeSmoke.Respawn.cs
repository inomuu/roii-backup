// Focused native regression: -roirespawnsmoke uses the same replaced-session fixture.
#if DEVELOPMENT_BUILD || UNITY_EDITOR
using System;
using System.Collections;
using System.IO;
using System.Linq;
using Roi.UI;
using Roi.Platform;
using UnityEngine;

namespace Roi.Perf
{
    public sealed partial class FloatingForgeSmoke
    {
        [Serializable] sealed class RespawnView { public RespawnGrid grid; }
        [Serializable] sealed class RespawnGrid { public RespawnEntity[] entities; }
        [Serializable] sealed class RespawnEntity { public uint id; public string team; public bool alive, is_pet; }
        RespawnEntity[] Party() => JsonUtility.FromJson<RespawnView>(_driver.Session.ViewJson())
            .grid.entities.Where(e=>e.team=="Player"&&!e.is_pet).ToArray();
        IEnumerator CheckRespawn()
        {
            _host.Open(); _host.Side(1);
            yield return new WaitForSecondsRealtime(.5f);
            var session=_driver.Session;
            Check(Party().Count(e=>e.alive)>=2,"respawn fixture has two living party members");
            // Keep one member down while the other lives; publish real views without combat.
            _driver.Paused=true;
            uint first=Party().First().id;
            Check(session.Debug("{\"kill\":{\"entity_id\":"+first+"}}"),"down one member");
            float until=Time.realtimeSinceStartup+DeathBanner.RespawnDelay+.5f;
            while(Time.realtimeSinceStartup<until)
            {
                _driver.ApplyInput("{\"SetPartyName\":{\"name\":\"Respawn probe\"}}");
                yield return new WaitForSecondsRealtime(.1f);
            }
            Check(Party().Any(e=>e.alive)&&Party().Any(e=>e.id==first&&!e.alive),
                "a single fallen member does not restart the party");
            _driver.Paused=false;
            for(int cycle=0;cycle<2;cycle++)
            {
                foreach(var e in Party()) Check(session.Debug("{\"kill\":{\"entity_id\":"+e.id+"}}"),"wipe member "+e.id);
                Check(!Party().Any(e=>e.alive),"whole party is actually down before restart "+cycle);
                // SimDriver uses unscaled time; the respawn countdown must do the same.
                Time.timeScale=cycle==1?0f:1f;
                float start=Time.realtimeSinceStartup;
                until=start+DeathBanner.RespawnDelay+1.5f;
                while(!Party().Any(e=>e.alive)&&Time.realtimeSinceStartup<until)yield return null;
                float elapsed=Time.realtimeSinceStartup-start;
                Check(Party().All(e=>e.alive)&&Party().Length>=2,
                    "current session automatically revives the whole party (cycle "+cycle+", "+elapsed.ToString("0.00")+"s)");
                Check(elapsed>=DeathBanner.RespawnDelay-.1f&&elapsed<DeathBanner.RespawnDelay+1.5f,
                    "respawn respects three real seconds with timeScale="+Time.timeScale);
                Time.timeScale=1f;
                Check(ReferenceEquals(session,_driver.Session)&&_host.IsOpen&&_host.CompanionContext&&WindowMode.IsTopmost,
                    "same session, floating Inventory and Forge remain open after wipe");
                ulong tick=_driver.Tick;
                yield return new WaitForSecondsRealtime(.5f);
                Check(_driver.Tick>tick,"simulation continues after automatic restart");
            }
            Check(_runtimeErrors==0,"no runtime errors during death and restart");
            File.WriteAllLines(Path.Combine(Output,"respawn-verification.txt"),_checks);
            Debug.Log("RESPAWN NATIVE COMPLETE: "+_checks.Count+" checks");
            Application.Quit(0);
        }
    }
}
#endif
