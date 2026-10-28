using System;
using System.IO;
using System.Reflection;
using UnityEditor;
using UnityEngine;
public static class ExcavationEditorRepair {
 public static void Run() {
  if(EditorApplication.isPlaying){Debug.LogWarning("Stop Play Mode before resetting the editor layout.");return;}
  var backup=Path.GetFullPath(Path.Combine(Application.dataPath,"../../../../../ui-editor-layout-backup"));Directory.CreateDirectory(backup);
  var source=Path.GetFullPath(Path.Combine(Application.dataPath,"../UserSettings/Layouts"));
  if(Directory.Exists(source))foreach(var file in Directory.GetFiles(source,"*.wlt"))File.Copy(file,Path.Combine(backup,Path.GetFileName(file)),true);
  var layout=Path.Combine(EditorApplication.applicationContentsPath,"Resources/Layouts/Default.wlt");
  var method=typeof(EditorUtility).GetMethod("LoadWindowLayout",BindingFlags.Static|BindingFlags.Public,null,new[]{typeof(string)},null);
  if(method==null)throw new MissingMethodException("EditorUtility.LoadWindowLayout(string) unavailable; saved layout was not changed.");
  method.Invoke(null,new object[]{layout});Debug.Log("EXCAVATION EDITOR LAYOUT RESET; backup: "+backup);
 }
}
