// Filesystem-only local debug slots. Separate from cloud saves and the simulation.
using System;
using System.IO;

namespace Roi.Sim
{
    public sealed class LocalSlotStore
    {
        public const int Count = 5;
        readonly string _root;
        string Pointer => System.IO.Path.Combine(_root, "active_slot.txt");
        public LocalSlotStore(string root) { _root = root; }
        public string PathFor(int slot)
        {
            if (slot < 0 || slot >= Count) throw new ArgumentOutOfRangeException(nameof(slot));
            return System.IO.Path.Combine(_root, "slot_" + (slot + 1) + ".json");
        }
        public int Active
        {
            get
            {
                int n;
                return File.Exists(Pointer) && int.TryParse(File.ReadAllText(Pointer), out n)
                    && n >= 0 && n < Count ? n : 0;
            }
        }
        public bool Exists(int slot) => File.Exists(PathFor(slot));
        public string Read(int slot) => Exists(slot) ? File.ReadAllText(PathFor(slot)) : null;
        public void Write(int slot, string json)
        {
            if (string.IsNullOrWhiteSpace(json)) throw new ArgumentException("Empty save");
            AtomicWrite(PathFor(slot), json);
        }
        public void Select(int slot) { PathFor(slot); AtomicWrite(Pointer, slot.ToString()); }
        public string Preserve(int slot, string reason)
        {
            var path = PathFor(slot);
            if (!File.Exists(path)) return null;
            var backup = path + "." + reason + "-" + Guid.NewGuid().ToString("N") + ".bak";
            File.Copy(path, backup);
            return backup;
        }
        public void Delete(int slot)
        {
            Preserve(slot, "deleted");
            File.Delete(PathFor(slot));
        }
        public void MigrateLegacy(string legacy)
        {
            Directory.CreateDirectory(_root);
            var marker = System.IO.Path.Combine(_root, "legacy_imported.txt");
            if (File.Exists(marker) || !File.Exists(legacy)) return;
            if (!Exists(0)) Write(0, File.ReadAllText(legacy));
            // Retain the original, but never resurrect it after deleting slot 1.
            AtomicWrite(marker, "imported");
        }
        static void AtomicWrite(string path, string text)
        {
            Directory.CreateDirectory(System.IO.Path.GetDirectoryName(path));
            var tmp = path + ".tmp-" + Guid.NewGuid().ToString("N");
            try
            {
                File.WriteAllText(tmp, text);
                if (File.Exists(path)) File.Replace(tmp, path, path + ".previous");
                else File.Move(tmp, path);
            }
            finally { if (File.Exists(tmp)) File.Delete(tmp); }
        }
    }
}
