#if DEVELOPMENT_BUILD || UNITY_EDITOR
using System;
using System.Collections;
using System.IO;
using System.Linq;
using Roi.UI;
using UnityEngine;

namespace Roi.Perf
{
    public sealed partial class FloatingForgeSmoke
    {
        IEnumerator CheckAutoFusion()
        {
            _driver.Paused=true;_host.Open();_host.Side(1);_host.Model.Mode(false);_host.RefreshNow();
            yield return new WaitForSecondsRealtime(.5f);
            int eligible=_host.Model.Bag.Count(item=>item!=null&&item.grade_step<=1&&_host.Model.Eligible(item,true));
            Check(eligible>=6,"Auto Fusion fixture has at least one Common/Fine batch: "+eligible);
            int before=_host.Model.Bag.Length;
            Click(Find(_host.transform,"UI_FORGE_AUTO_FUSION_OFF"));
            Check(_host.Model.AutoFusionEnabled,"Auto Fusion toggle turns on");
            yield return new WaitForSecondsRealtime(2);
            Check(_host.Model.Bag.Length==before,"Auto Fusion waits for its 10-second cadence");
            float deadline=Time.realtimeSinceStartup+12;
            while(Time.realtimeSinceStartup<deadline&&_host.Model.Bag.Length>=before)yield return new WaitForSecondsRealtime(.2f);
            Check(_host.Model.Bag.Length<before,"Auto Fusion consumes a verified six-item batch");
            Check(_host.Result.Visible,"Auto Fusion displays the normal Result modal");
            int afterFirst=_host.Model.Bag.Length;
            // Hold the normal 4.5s result open to test the next 10s cycle's modal guard.
            _host.Result.enabled=false;
            yield return new WaitForSecondsRealtime(11);
            Check(_host.Model.Bag.Length==afterFirst,"Result modal prevents overlapping auto batches");
            _host.Result.enabled=true;
            _host.Close();
            deadline=Time.realtimeSinceStartup+12;
            while(Time.realtimeSinceStartup<deadline&&_host.Model.Bag.Length>=afterFirst)yield return new WaitForSecondsRealtime(.2f);
            Check(_host.Model.Bag.Length<afterFirst,"Auto Fusion continues while Forge is closed");
            Check(_host.Result.Visible&&_host.IsOpen,"Background Auto Fusion opens its Result");
            _host.Result.Hide();
            _host.Open();_host.Side(1);yield return null;
            Click(Find(_host.transform,"UI_FORGE_AUTO_FUSION_OFF"));
            Check(!_host.Model.AutoFusionEnabled,"Auto Fusion toggle turns off");
            int stable=_host.Model.Bag.Length;yield return new WaitForSecondsRealtime(.7f);
            Check(_host.Model.Bag.Length==stable,"turning Auto Fusion off stops further batches");
            File.WriteAllLines(Path.Combine(Output,"autofusion-verification.txt"),_checks);
            Debug.Log("AUTO FUSION NATIVE COMPLETE: "+_checks.Count+" checks");Application.Quit(0);
        }
    }
}
#endif
