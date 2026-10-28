using System.Collections;
using Roi.UI;
using UnityEngine;

public sealed partial class UiGalleryRunner
{
    IEnumerator InventoryRestored104Drive()
    {
        var bar=HudBar.Create(transform);yield return null;
        bar.Toggle("Inventory");yield return new WaitForSecondsRealtime(.4f);
        var host=bar.Floating;
        yield return Shot("Inventory");
        foreach(var side in new[]{1,2,3,4,5})
        {
            host.Side(side);yield return Shot("Inventory-side-"+side);
        }
        host.Side(0);host.SetFullStatsVisible(true);yield return Shot("Inventory-stats");
        host.SetFullStatsVisible(false);host.Close();bar.Toggle("Inventory");
        yield return Shot("Inventory-reopened");
        Finish86();
    }
}
