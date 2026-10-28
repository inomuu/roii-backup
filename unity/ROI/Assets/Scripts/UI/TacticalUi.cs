using UnityEngine;
using UnityEngine.UI;
namespace Roi.UI {
    public sealed class TacticalPrimary : MonoBehaviour { }
    public static class TacticalUi {
        public static void Primary(Image image) {
            image.sprite=UiTheme.CutFill(8); image.type=Image.Type.Sliced;
            image.color=new Color(.88f,.83f,.70f);
            UiTheme.Border(image.rectTransform,UiTheme.Accent);
            if(image.GetComponent<TacticalPrimary>()==null) image.gameObject.AddComponent<TacticalPrimary>();
        }
    }
}

