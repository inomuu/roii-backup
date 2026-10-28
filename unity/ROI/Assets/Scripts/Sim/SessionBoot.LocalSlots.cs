using System;

namespace Roi.Sim
{
    public sealed partial class SessionBoot
    {
        public bool CanUseLocalSlots => !Online && _devSaveAt > 0f && _driver != null
            && _driver.Session != null && _driver.Session.IsAlive
            && !Roi.Net.OnlineService.Instance.LoggedIn;

        public bool SaveLocalSlot(out string error)
        {
            error = "";
            if (!CanUseLocalSlots) { error = Roi.UI.Loc.Get("UI_LOCAL_SAVE_ONLY"); return false; }
            try { var slots = DevSaveStore.Slots; slots.Write(slots.Active, _driver.Session.SaveJson()); return true; }
            catch (Exception e) { error = e.Message; return false; }
        }

        public bool SwitchLocalSlot(int target, out string error)
        {
            error = "";
            if (!CanUseLocalSlots) { error = Roi.UI.Loc.Get("UI_LOCAL_SAVE_ONLY"); return false; }
            SimSession next = null;
            try
            {
                var slots = DevSaveStore.Slots;
                slots.PathFor(target); // Validate before saving or changing anything.
                if (target == slots.Active) return SaveLocalSlot(out error);
                var raw = slots.Read(target);
                next = raw == null ? FreshLocalSlot() : SimSession.Load(raw, _catalog);
                if (next == null || !next.IsAlive)
                {
                    slots.Preserve(target, "unreadable");
                    error = Roi.UI.Loc.Get("UI_LOCAL_SAVE_UNREADABLE");
                    return false;
                }
                slots.Write(slots.Active, _driver.Session.SaveJson());
                if (raw == null) slots.Write(target, next.SaveJson());
                slots.Select(target);
                AttachLocalSlot(next);
                next = null;
                return true;
            }
            catch (Exception e) { error = e.Message; return false; }
            finally { next?.Dispose(); }
        }

        public bool DeleteLocalSlot(int target, out string error)
        {
            error = "";
            if (!CanUseLocalSlots) { error = Roi.UI.Loc.Get("UI_LOCAL_SAVE_ONLY"); return false; }
            SimSession next = null;
            try
            {
                var slots = DevSaveStore.Slots;
                slots.PathFor(target);
                if (target != slots.Active) { slots.Delete(target); return true; }
                // Deleting the active slot starts a fresh party, so autosave cannot restore its old state.
                next = FreshLocalSlot();
                if (next == null) { error = Roi.UI.Loc.Get("UI_LOCAL_SAVE_UNREADABLE"); return false; }
                slots.Write(target, _driver.Session.SaveJson());
                slots.Preserve(target, "deleted");
                slots.Write(target, next.SaveJson());
                AttachLocalSlot(next);
                next = null;
                return true;
            }
            catch (Exception e) { error = e.Message; return false; }
            finally { next?.Dispose(); }
        }

        SimSession FreshLocalSlot() => SimSession.NewStarter(
            _arena != null ? _arena.Seed : DevSeed, _catalog, StartZone,
            _arena != null ? _arena.PartyName : "Idlers");

        void AttachLocalSlot(SimSession next)
        {
            next.SetDebugAllowed(true);
            // Arena reuses its one driver; do not subscribe SaveRequested a second time.
            _driver = _arena.BeginSession(next);
            _devSaveAt = UnityEngine.Time.time + DevSaveEvery;
        }
    }
}
