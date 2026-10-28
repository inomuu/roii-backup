// Focused native regression: -roitrainingsmoke exercises the real Training query and Train event.
#if DEVELOPMENT_BUILD || UNITY_EDITOR
using System;
using System.Collections;
using System.IO;
using System.Linq;
using Roi.Platform;
using Roi.UI;
using UnityEngine;
using UnityEngine.UI;

namespace Roi.Perf
{
    public sealed partial class FloatingForgeSmoke
    {
        IEnumerator CheckTraining()
        {
            _host.Open();
            _host.Side(FloatingInventoryHost.SideTraining);
            yield return new WaitForSecondsRealtime(.6f);
            var training = _host.GetComponent<TrainingModel>();
            var panel = _host.transform.Find("WindowGroup/Training");
            Check(training != null && panel != null && panel.gameObject.activeSelf,
                "Training opens as the right floating panel");
            Check(training.Available && training.Slots.Length == 23,
                "host exposes all 23 Training slots");
            Check(training.Slots.Count(s => s.id.StartsWith("attune_", StringComparison.Ordinal)) == 6
                && training.Slots.Count(s => s.id.StartsWith("ward_", StringComparison.Ordinal)) == 6,
                "Training groups contain six Attunement and six Ward slots");
            Check(training.Slots.All(s => s.has_next_cost || s.level >= s.max_level),
                "every live slot distinguishes a real next cost from a capped null cost");
            yield return new WaitForEndOfFrame();
            ScreenCapture.CaptureScreenshot(Path.Combine(Output, "training-panel.png"));
            yield return new WaitForSecondsRealtime(.5f);
            Check(UiNumber.Exact(ulong.MaxValue) == "18,446,744,073,709,551,615"
                && UiNumber.Compact(999_999) == "999.99K"
                && UiNumber.Compact(1_000_000) == "1M",
                "large currency formatter preserves exact u64 boundaries");

            var slot = training.Find("might");
            Check(slot != null && training.CanTrain(slot), "starter fixture can afford the first Might level");
            int before = slot.level;
            ulong gilBefore = training.Gil;
            var train = panel.GetComponentsInChildren<Button>(true).First(b => b.name == "UI_TRAIN_BUY");
            Click(train);
            yield return new WaitForSecondsRealtime(.5f);
            training.Refresh();
            var after = training.Find("might");
            Check(after.level == before + 1 && training.Gil < gilBefore,
                "Train button applies one level and spends Gil through the native sim");
            Check(_driver.Tick > 0, "combat tick continues while Training is open");

            _host.SetCompanionContext(true);
            _host.Open();
            _host.Side(FloatingInventoryHost.SideTraining);
            yield return null;
            Check(_host.CompanionContext && _host.IsOpen && _host.ActiveSide == FloatingInventoryHost.SideTraining,
                "same Training panel can be reopened in Companion context");
            File.WriteAllLines(Path.Combine(Output, "training-verification.txt"), _checks);
            Debug.Log("TRAINING NATIVE COMPLETE: " + _checks.Count + " checks");
            Application.Quit(0);
        }
    }
}
#endif
