// Native X-button regression; measures real hidden HUD traffic before allowing it to draw.
#if DEVELOPMENT_BUILD || UNITY_EDITOR
using System.Collections;
using System.IO;
using System.Reflection;
using Roi.Arena;
using Roi.Platform;
using UnityEngine;
using UnityEngine.InputSystem;
using UnityEngine.UI;

namespace Roi.Perf
{
    public sealed partial class FloatingForgeSmoke
    {
        int HudCount(ArenaHud hud,string field) => ((ICollection)typeof(ArenaHud)
            .GetField(field,BindingFlags.NonPublic|BindingFlags.Instance).GetValue(hud)).Count;
        void HiddenHudEmpty(ArenaHud hud)
        {
            foreach(string field in new[]{"_bars","_popups","_banners","_multi","_multiByKey","_skillLabels","_loot"})
                Check(HudCount(hud,field)==0,"hidden HUD retains zero entries in "+field+" (actual "+HudCount(hud,field)+")");
        }
        static void FeedHud(ArenaHud hud)
        {
            var at=Vector3.zero;
            hud.PushBar(at,Vector3.right,1,0,0);
            hud.SpawnDamage(1,at,999,false); hud.SpawnHeal(1,at,123);
            hud.SpawnText(1,at,"probe",Color.white); hud.SpawnBreakBanner(1,at,"probe",999,false);
            hud.FeedMultiHit("exit-probe",1,at,12,2); hud.SpawnSkillLabel(1,"probe");
            hud.PushLoot("probe",1,"Common"); hud.PushHoverName("probe",at);
        }
        IEnumerator CheckCompanionExit()
        {
            var hud=Object.FindFirstObjectByType<ArenaHud>(FindObjectsInactive.Include);
            var arena=Object.FindFirstObjectByType<ArenaView>();
            Check(hud!=null&&!hud.isActiveAndEnabled,"Companion hides Battle HUD while the arena still runs");
            var players=arena.PlayerIds();
            if(players.Count>0)
            {
                _companion.SpawnDamage(players[0],arena.ActorAnchor(players[0]),999,false);
                yield return null;
                Check(_companion.ActiveDamagePopupCount>0&&_companion.ActiveDamageLabelCount>0,
                    "Companion renders damage numbers in the pane overlay");
            }
            for(int cycle=0;cycle<3;cycle++)
            {
                _host.Open(); _host.Side(1);
                ulong before=_driver.Tick;
                yield return new WaitForSecondsRealtime(5);
                Check(_driver.Tick>before,"hidden-HUD soak advances real simulation ticks, cycle "+cycle);
                // If regressed, fail BEFORE reenabling: drawing thousands of queued bars freezes Unity.
                HiddenHudEmpty(hud);
                for(int i=0;i<1000;i++)FeedHud(hud);
                HiddenHudEmpty(hud);
                int pooled=hud.GetComponentsInChildren<Graphic>(true).Length;
                float start=Time.realtimeSinceStartup;
                Click(Find(_companion.transform,"UI_FLOAT_CLOSE"));
                yield return null;
                float firstFrame=Time.realtimeSinceStartup-start;
                Check(firstFrame<1f,"X returns a frame in under 1s (actual "+firstFrame.ToString("0.000")+"s)");
                yield return new WaitForSecondsRealtime(.5f);
                Check(!_companion.Visible&&!_host.CompanionContext&&hud.isActiveAndEnabled&&arena.MainCamera.enabled,
                    "X restores Battle camera, HUD and floating UI context");
                Check(!WindowMode.IsTopmost&&!WindowMode.IsTransparent,"X restores normal native window appearance");
                int after=hud.GetComponentsInChildren<Graphic>(true).Length;
                Check(after<pooled+256,"return renders current HUD only; pool before="+pooled+", after="+after);
                before=_driver.Tick;
                yield return new WaitForSecondsRealtime(.5f);
                Check(_driver.Tick>before,"Battle keeps simulating after X");
                // Enabling must still permit new effects; hiding must discard those effects.
                FeedHud(hud);
                Check(HudCount(hud,"_popups")>=3&&HudCount(hud,"_multi")>=1,"visible Battle accepts fresh popups and multihits");
                hud.gameObject.SetActive(false);
                HiddenHudEmpty(hud);
                hud.gameObject.SetActive(true);
                yield return KeyPress(Key.W);
                Check(_companion.Visible&&!hud.isActiveAndEnabled,"W remains responsive and reenters Companion");
            }
            Check(_runtimeErrors==0,"no runtime errors during repeated native X exits");
            File.WriteAllLines(Path.Combine(Output,"exit-verification.txt"),_checks);
            Debug.Log("EXIT NATIVE COMPLETE: "+_checks.Count+" checks");
            Application.Quit(0);
        }
    }
}
#endif
