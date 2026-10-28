using TMPro;
using UnityEngine.EventSystems;
namespace Roi.UI
{
    public static class UiInputFocus
    {
        public static bool Typing {
            get { var go=EventSystem.current?.currentSelectedGameObject;var field=go!=null?go.GetComponent<TMP_InputField>():null;return field!=null&&field.isFocused; }
        }
        public static bool Modal => UiConfirm.BlocksNavigation || UiPicker.BlocksNavigation || ProvisionsWindow.BlocksNavigation || BestiaryWindow.BlocksNavigation;
    }
}
