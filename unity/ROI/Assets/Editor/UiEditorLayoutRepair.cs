using UnityEditor;
using UnityEngine;
public static class UiEditorLayoutRepair
{
    public static void Run()
    {
        bool invoked=EditorApplication.ExecuteMenuItem("Window/Layouts/Default");
        Debug.Log("UI layout reset requested: "+invoked);
    }
}
