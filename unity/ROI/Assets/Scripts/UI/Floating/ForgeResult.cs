using UnityEngine;

namespace Roi.UI
{
    // Fusion rewards update the inventory directly without a blocking result dialog.
    // Keep the API for existing callers and editor fixtures.
    public sealed class ForgeResult : MonoBehaviour
    {
        public bool Visible => false;
        public static ForgeResult Create(Transform parent)
        {
            var result = new GameObject("ForgeResultController").AddComponent<ForgeResult>();
            result.transform.SetParent(parent, false);
            return result;
        }
        public void Show(ForgeModel.Receipt receipt) { }
        public void Hide() { }
    }
}