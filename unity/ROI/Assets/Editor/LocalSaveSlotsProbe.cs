using System;
using System.IO;
using System.Reflection;
using Roi.Sim;
using Roi.UI;
using UnityEditor;
using UnityEngine;
using UnityEngine.UI;

public static class LocalSaveSlotsProbe
{
    static void Require(bool condition, string why) { if (!condition) throw new Exception("LOCAL SLOTS: " + why); }
    [MenuItem("ROI/Probes/Local slots filesystem")]
    public static void Filesystem()
    {
        var root = Path.Combine(Path.GetTempPath(), "roi-slots-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        var old = Path.Combine(root, "dev_session.json");
        File.WriteAllText(old, "legacy payload");
        var s = new LocalSlotStore(Path.Combine(root, "slots"));
        s.MigrateLegacy(old);
        Require(s.Read(0) == "legacy payload", "legacy migration lost data");
        s.Delete(0); s.MigrateLegacy(old);
        Require(!s.Exists(0), "deleted legacy slot resurrected");
        for (int i = 0; i < 5; i++) s.Write(i, "payload-" + i);
        s.Select(3);
        var reboot = new LocalSlotStore(Path.Combine(root, "slots"));
        Require(reboot.Active == 3 && reboot.Read(3) == "payload-3", "restart forgot active slot/data");
        s.Write(3, "updated");
        Require(File.ReadAllText(s.PathFor(3) + ".previous") == "payload-3", "overwrite lost previous backup");
        s.Delete(2);
        Require(s.Read(1) == "payload-1" && s.Read(3) == "updated" && !s.Exists(2), "delete touched another slot");
        bool rejected = false;
        try { s.Select(5); } catch (ArgumentOutOfRangeException) { rejected = true; }
        Require(rejected && s.Active == 3, "invalid slot changed selection");
        Debug.Log("LOCAL SLOTS FILESYSTEM PASS: migration, no resurrection, restart, atomic backup, isolation, bounds");
    }

    // Requires an isolated local session with Debug open. Refuses real save roots and online sessions.
    [MenuItem("ROI/Probes/Local slots UI (isolated session)")]
    public static void Session()
    {
        var boot = SessionBoot.Instance;
        var panel = UnityEngine.Object.FindFirstObjectByType<LocalSavePanel>();
        var driver = UnityEngine.Object.FindFirstObjectByType<SimDriver>();
        var slots = DevSaveStore.Slots;
        Require(EditorApplication.isPlaying && boot != null && boot.CanUseLocalSlots && panel != null,
            "need local session and Debug panel");
        Require(slots.PathFor(0).StartsWith(Path.GetTempPath()) || slots.PathFor(0).StartsWith("/private/tmp/"), "refusing real save directory");
        driver.Paused = true;
        string error;
        Require(boot.SwitchLocalSlot(0, out error), error);
        driver.Session.Debug("{\"give_currency\":{\"gil\":12345,\"soul\":0}}");
        string hash0 = driver.Session.StateHash();
        var flags = BindingFlags.Instance | BindingFlags.NonPublic;
        var loads = (Button[])typeof(LocalSavePanel).GetField("_load", flags).GetValue(panel);
        loads[1].onClick.Invoke();
        Require(slots.Active == 1, "load button did not switch to slot 2");
        using (var saved = SimSession.Load(slots.Read(0), SimSession.LoadCatalog()))
            Require(saved != null && saved.StateHash() == hash0, "switch failed to save departing slot");
        Require(driver.Session.StateHash() != hash0, "empty slot inherited previous party state");
        string fresh = driver.Session.StateHash();
        driver.Session.Debug("{\"give_currency\":{\"gil\":99,\"soul\":0}}");
        string hash1 = driver.Session.StateHash();
        loads[0].onClick.Invoke();
        Require(slots.Active == 0 && driver.Session.StateHash() == hash0, "slot 1 did not restore exact state");
        loads[1].onClick.Invoke();
        Require(driver.Session.StateHash() == hash1, "slot 2 did not restore exact state");
        var deletes = (Button[])typeof(LocalSavePanel).GetField("_delete", flags).GetValue(panel);
        deletes[1].onClick.Invoke();
        Require(driver.Session.StateHash() == hash1, "first delete click destroyed save");
        deletes[1].onClick.Invoke();
        Require(driver.Session.StateHash() == fresh, "active delete did not create fresh starter party");
        Require(boot.SaveLocalSlot(out error), error);
        using (var saved = SimSession.Load(slots.Read(1), SimSession.LoadCatalog()))
            Require(saved.StateHash() == fresh, "autosave resurrected deleted state");
        slots.Write(4, "broken-json");
        Require(!boot.SwitchLocalSlot(4, out error), "malformed save accepted");
        Require(slots.Active == 1 && slots.Read(4) == "broken-json" && driver.Session.StateHash() == fresh,
            "failed load changed active state or overwrote malformed save");
        typeof(SessionBoot).GetProperty("Online").SetValue(boot, true);
        try { Require(!boot.SwitchLocalSlot(0, out error) && !boot.DeleteLocalSlot(1, out error)
            && !boot.SaveLocalSlot(out error), "online gate allowed local mutation"); }
        finally { typeof(SessionBoot).GetProperty("Online").SetValue(boot, false); }
        panel.Refresh();
        Debug.Log("LOCAL SLOTS SESSION PASS: UI switching, exact save/load, isolated currency, delete confirmation, active reset, malformed preservation, online gate");
    }
}
