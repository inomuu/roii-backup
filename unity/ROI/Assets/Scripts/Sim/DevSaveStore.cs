// Local debug saves only. Existing single-file saves import into slot 1 once.
using System;
using System.IO;
using UnityEngine;

namespace Roi.Sim
{
    public static class DevSaveStore
    {
        #if UNITY_EDITOR || DEVELOPMENT_BUILD
        // Isolated development probes never read/write a developer's real slots.
        public static string TestDirectoryOverride { private get; set; }
#endif
        public static LocalSlotStore Slots
        {
            get
            {
                string root = Application.persistentDataPath;
#if UNITY_EDITOR || DEVELOPMENT_BUILD
                if (!string.IsNullOrEmpty(TestDirectoryOverride)) root = TestDirectoryOverride;
#endif
                var store = new LocalSlotStore(System.IO.Path.Combine(root, "dev_slots"));
                store.MigrateLegacy(System.IO.Path.Combine(root, "dev_session.json"));
                return store;
            }
        }
        public static string Where => Slots.PathFor(Slots.Active);
        public static string Read()
        {
            try { var slots = Slots; return slots.Read(slots.Active); }
            catch (Exception e) { Debug.LogWarning("DEV SAVE: " + e.Message); return null; }
        }
        public static void Write(string json)
        {
            try { var slots = Slots; slots.Write(slots.Active, json); }
            catch (Exception e) { Debug.LogWarning("DEV SAVE: " + e.Message); }
        }
        public static void Preserve()
        {
            try { var slots = Slots; Debug.LogWarning("DEV SAVE preserved: " + slots.Preserve(slots.Active, "unreadable")); }
            catch (Exception e) { Debug.LogWarning("DEV SAVE: " + e.Message); }
        }
        public static void Clear()
        {
            try { var slots = Slots; slots.Delete(slots.Active); }
            catch (Exception e) { Debug.LogWarning("DEV SAVE: " + e.Message); }
        }
    }
}
