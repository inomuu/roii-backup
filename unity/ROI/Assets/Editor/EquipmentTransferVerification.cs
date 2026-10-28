// Editor-only scenarios against the real helper and shipped native host. Owns every fixture;
// never attaches to a game driver, changes a user save, or needs the online dev account.
using System;
using System.Collections.Generic;
using System.Text;
using Roi.Sim;
using Roi.UI;
using UnityEngine;
using Actions = Roi.UI.ExemplarEquipmentActions;

public static class EquipmentTransferVerification
{
    public static string Run()
    {
        var report = new StringBuilder("Equipment transfer verification — " + SimSession.SimVersion() + "\n");
        int passed = 0, failed = 0, assertions = 0;
        var cases = new KeyValuePair<string, Action<Fixture>>[]
        {
            Case("bag equip targets Rapis and preserves untouched Seraphine", BagEquip),
            Case("transfer preserves the exact piece, replacement, log and atomic publication", Transfer),
            Case("wrong destination slot refuses before the donor loses equipment", WrongSlot),
            Case("same-name copies remain distinguishable by native payload", DuplicateNames),
            Case("recipient's own item cannot become an equip choice", AlreadyOwned),
            Case("bag reorder invalidates a frozen choice even at unchanged count", StaleBag),
            Case("donor loadout change invalidates confirmation with unchanged bag", StaleSource),
            Case("recipient loadout change invalidates confirmation with unchanged bag", StaleTarget),
            Case("inspected identity change cannot redirect the frozen choice", StaleIdentity),
            Case("new session cannot consume a previous session's confirmation", StaleSession)
        };
        foreach (var test in cases)
        {
            Fixture fixture = null;
            try
            {
                fixture = new Fixture(); test.Value(fixture); passed++;
                report.Append("PASS ").Append(test.Key).Append('\n');
            }
            catch (Exception exception)
            {
                failed++; report.Append("FAIL ").Append(test.Key).Append(": ").Append(exception.Message).Append('\n');
            }
            finally
            {
                if (fixture != null) { assertions += fixture.Assertions; fixture.Dispose(); }
            }
        }
        report.Append(passed).Append("/").Append(cases.Length).Append(" scenarios; ")
            .Append(assertions).Append(" assertions; failures=").Append(failed).Append('\n');
        report.Append("Fixtures use native grants with legacy null instance IDs. Same-name copies differ by native foil payload; no fabricated IDs or catalog aliases.\n");
        report.Append("No UI clicks, server replay, bench transfer, or concurrent-tick behavior are claimed by this synchronous runner.");
        return report.ToString();
    }

    static KeyValuePair<string, Action<Fixture>> Case(string name, Action<Fixture> test)
        => new KeyValuePair<string, Action<Fixture>>(name, test);

    static void BagEquip(Fixture f)
    {
        f.Grant("armor_s1");
        var ring = f.Grant("ring_s1"); // Not bag index zero: filtered-list renumbering must fail.
        string untouched = f.Worn(1), hash = f.Driver.Session.StateHash(), log = f.Driver.Session.ExportLog();
        var snapshot = f.Read(2);
        var choice = Actions.FreezeEquip(snapshot, ring, "Necklace");
        f.Check(choice != null && !choice.RequiresConfirmation && choice.MemberId == 2, "Rapis bag choice");
        f.Check(hash == f.Driver.Session.StateHash() && log == f.Driver.Session.ExportLog(), "preview must not emit input");
        f.Check(Actions.Apply(f.Driver, choice, 2) == Actions.Result.Applied, "native ring equip applied");
        f.Check(f.Has(f.Read(2).Equipped, ring) && !f.Has(f.Read(2).Inventory, ring), "chosen ring moved from bag to Rapis");
        f.Check(untouched == f.Worn(1), "Seraphine loadout unchanged");
        f.Check(f.Read(2).Inventory.Length == snapshot.Inventory.Length - 1, "one exact bag item consumed");
    }

    static void Transfer(Fixture f)
    {
        var source = f.SeedWorn(2, "armor_s1", true);
        var replaced = f.SeedWorn(1, "armor_s1_1s");
        var before = f.Read(1);
        f.Check(Array.Exists(before.Candidates, r => r.OwnerId == 2 && r.Raw == source.Raw), "donor appears as candidate");
        var choice = Actions.FreezeEquip(before, source, "Armor");
        f.Check(choice != null && choice.RequiresConfirmation && choice.SourceOwnerId == 2, "borrowed item requires confirmation");
        int publishes = 0; bool completeAtPublish = false;
        f.Driver.StateUpdated += _ => { publishes++; completeAtPublish = !f.Has(f.Read(2).Equipped, source) && f.Has(f.Read(1).Equipped, source); };
        var priorLog = f.Driver.Session.ExportLog();
        f.Check(Actions.Apply(f.Driver, choice, 1) == Actions.Result.Applied, "native transfer applied");
        var after = f.Read(1);
        f.Check(f.Has(after.Equipped, source) && !f.Has(f.Read(2).Equipped, source), "exact donor piece moved");
        f.Check(f.Has(after.Inventory, replaced), "recipient's displaced armor recovered in bag");
        f.Check(after.Candidates.Length == before.Candidates.Length && after.Inventory.Length == before.Inventory.Length + 1,
            "all equipment conserved; replaced piece adds one bag row");
        f.Check(publishes == 1 && completeAtPublish, "only completed transfer is published");
        var newLog = f.Driver.Session.ExportLog();
        f.Check(Count(newLog, "UnequipItem") == Count(priorLog, "UnequipItem") + 1
            && Count(newLog, "EquipItem") == Count(priorLog, "EquipItem") + 1, "both real inputs retained in replay log");
        f.Rejected(choice, 1, Actions.Result.Stale); // Double confirmation cannot apply twice.
    }

    static void WrongSlot(Fixture f)
    {
        var source = f.SeedWorn(2, "armor_s1");
        var choice = Actions.FreezeEquip(f.Read(1), source, "Helmet");
        f.Check(choice != null, "metadata admits armor before native routing validation");
        f.Rejected(choice, 1, Actions.Result.Refused);
        f.Check(f.Has(f.Read(2).Equipped, source) && !f.Has(f.Read(1).Equipped, source), "donor remains equipped after wrong-slot rejection");
    }

    static void DuplicateNames(Fixture f)
    {
        var plain = f.Grant("armor_s1"); var foil = f.Grant("armor_s1", true);
        f.Check(plain.Item.name == foil.Item.name && plain.Item.def_id == foil.Item.def_id
            && plain.Item.foil != foil.Item.foil && plain.Raw != foil.Raw, "real same-name copies differ by per-instance field");
        f.Check(plain.Key != foil.Key, "duplicate candidate keys remain distinct");
        var choice = Actions.FreezeEquip(f.Read(1), foil, "Armor");
        f.Check(Actions.Apply(f.Driver, choice, 1) == Actions.Result.Applied, "selected second copy equipped");
        f.Check(f.Has(f.Read(1).Equipped, foil) && f.Has(f.Read(1).Inventory, plain)
            && !f.Has(f.Read(1).Inventory, foil), "foil selected copy moved; identical-name plain copy retained");
    }

    static void AlreadyOwned(Fixture f)
    {
        var owned = f.SeedWorn(1, "armor_s1"); var s = f.Read(1);
        string hash = f.Driver.Session.StateHash(), log = f.Driver.Session.ExportLog();
        f.Check(Actions.EquipIssue(s, owned) == "UI_EX_GEAR_ALREADY_EQUIPPED", "already-equipped issue");
        f.Check(Actions.FreezeEquip(s, owned, "Armor") == null, "own worn item cannot be equipped twice");
        f.Check(hash == f.Driver.Session.StateHash() && log == f.Driver.Session.ExportLog(), "own-item preview is read-only");
    }

    static void StaleBag(Fixture f)
    {
        var armor = f.Grant("armor_s1"); var ring = f.Grant("ring_s1"); f.Grant("garment_s1");
        var before = f.Read(1); var choice = Actions.FreezeEquip(before, armor, "Armor");
        string bag = f.Driver.Session.QueryJson("inventory");
        f.DirectEquip(2, ring); var worn = f.Find(f.Read(2).Equipped, "ring_s1");
        f.Native("{\"UnequipItem\":{\"entity_id\":2,\"equipped_slot\":" + worn.Index + "}}");
        f.Check(f.Read(1).Inventory.Length == before.Inventory.Length && bag != f.Driver.Session.QueryJson("inventory"),
            "real swap-remove plus return changes bag order, not count");
        f.Rejected(choice, 1, Actions.Result.Stale);
    }

    static void StaleSource(Fixture f) => ChangedLoadout(f, 2);
    static void StaleTarget(Fixture f) => ChangedLoadout(f, 1);
    static void ChangedLoadout(Fixture f, long changedOwner)
    {
        var source = f.SeedWorn(2, "armor_s1"); var garment = f.Grant("garment_s1");
        var choice = Actions.FreezeEquip(f.Read(1), source, "Armor");
        string bag = f.Driver.Session.QueryJson("inventory"), priorWorn = f.Worn(changedOwner);
        f.DirectEquip(changedOwner, garment); f.Grant("garment_s1");
        f.Check(bag == f.Driver.Session.QueryJson("inventory") && priorWorn != f.Worn(changedOwner),
            "loadout mutation is isolated from bag fingerprint");
        f.Check(f.Find(f.Read(2).Equipped, "armor_s1").Raw == source.Raw, "chosen donor piece itself stays unchanged");
        f.Rejected(choice, 1, Actions.Result.Stale);
    }

    static void StaleIdentity(Fixture f)
    {
        var source = f.SeedWorn(2, "armor_s1");
        var choice = Actions.FreezeEquip(f.Read(1), source, "Armor");
        f.Rejected(choice, 2, Actions.Result.Stale);
    }

    static void StaleSession(Fixture f)
    {
        var armor = f.Grant("armor_s1"); var choice = Actions.FreezeEquip(f.Read(1), armor, "Armor");
        f.Restart(); f.Grant("armor_s1");
        f.Rejected(choice, 1, Actions.Result.Stale);
    }

    static int Count(string value, string needle)
    {
        int count = 0, offset = 0;
        while ((offset = (value ?? "").IndexOf(needle, offset, StringComparison.Ordinal)) >= 0) { count++; offset += needle.Length; }
        return count;
    }

    sealed class Fixture : IDisposable
    {
        public readonly SimDriver Driver;
        public int Assertions;
        readonly GameObject _host;
        public Fixture()
        {
            _host = new GameObject("DisposableEquipmentTransferVerification") { hideFlags = HideFlags.HideAndDontSave };
            Driver = _host.AddComponent<SimDriver>(); Driver.enabled = false;
            try { Restart(); }
            catch { Dispose(); throw; }
        }
        public void Restart()
        {
            var session = SimSession.NewStarter(3914, SimSession.LoadCatalog(), "zone_b1_1", "Equipment transfer verification");
            Check(session != null && session.IsAlive, "native starter fixture exists"); Driver.Attach(session);
            Check(Read(1).UnitDef == "seraphine" && Read(2).UnitDef == "rapis", "real starter identity contract 1/2");
        }
        public Actions.Snapshot Read(long id)
        {
            var s = Actions.Read(Driver, id); Check(s.Available, "available member " + id); return s;
        }
        public Actions.Row Grant(string def, bool foil = false)
        {
            int before = Read(1).Inventory.Length;
            Check(Driver.Session.Debug("{\"grant_item\":{\"def_id\":\"" + def + "\",\"foil\":" + (foil ? "true" : "false") + "}}"), "grant command parsed");
            var bag = Read(1).Inventory;
            Check(bag.Length == before + 1 && bag[before].Item.def_id == def, "native grant appends requested piece " + def);
            return bag[before];
        }
        public Actions.Row SeedWorn(long owner, string def, bool foil = false)
        {
            var row = Grant(def, foil); DirectEquip(owner, row); return Find(Read(owner).Equipped, def);
        }
        public void DirectEquip(long owner, Actions.Row row)
        {
            Native("{\"EquipItem\":{\"entity_id\":" + owner + ",\"inventory_idx\":" + row.Index + "}}");
            Check(Has(Read(owner).Equipped, row), "native fixture equipped exact piece on " + owner);
        }
        public void Native(string input) => Check(Driver.ApplyInput(input), "fixture input parses");
        public Actions.Row Find(Actions.Row[] rows, string def)
        {
            var row = Array.Find(rows, r => r.Item.def_id == def); Check(row != null, "fixture row " + def); return row;
        }
        public bool Has(Actions.Row[] rows, Actions.Row piece)
            => Array.Exists(rows, r => r.Item.def_id == piece.Item.def_id && r.Item.foil == piece.Item.foil
                && r.Item.grade_step == piece.Item.grade_step && r.InstanceId == piece.InstanceId);
        public string Worn(long id)
        {
            var rows = Read(id).Equipped; return string.Join("|", Array.ConvertAll(rows, r => r.Raw));
        }
        public void Rejected(Actions.Choice choice, long inspected, Actions.Result expected)
        {
            Check(choice != null, "frozen choice exists");
            string hash = Driver.Session.StateHash(), log = Driver.Session.ExportLog();
            var result = Actions.Apply(Driver, choice, inspected);
            Check(result == expected, "expected " + expected + ", received " + result);
            Check(hash == Driver.Session.StateHash(), "rejection preserves native state hash");
            Check(log == Driver.Session.ExportLog(), "rejection emits no input");
        }
        public void Check(bool condition, string context)
        {
            Assertions++; if (!condition) throw new InvalidOperationException(context);
        }
        public void Dispose() { if (_host != null) UnityEngine.Object.DestroyImmediate(_host); }
    }
}
