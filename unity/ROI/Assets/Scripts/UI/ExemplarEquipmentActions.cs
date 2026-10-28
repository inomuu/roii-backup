using System;
using System.Collections.Generic;
using Roi.Sim;
using UnityEngine;

namespace Roi.UI
{
    // Frozen collection indices, explicit character targets, and host-owned gear rules.
    // Inspecting this model never applies an input or changes PartySelection.
    public static class ExemplarEquipmentActions
    {
        public enum Result { Applied, Refused, Stale, Unavailable }

        public sealed class Row
        {
            public int Index;
            public ItemView Item;
            public string Raw, InstanceId;
            public string[] Wieldable;
            public long OwnerId; // 0 = bag; otherwise the exact deployed character id.
            public string OwnerUnitDef;
            public string Slot => Item?.slot;
            public string Key => OwnerId + ":" + Index;
        }

        public sealed class Snapshot
        {
            public bool Available, HasBenchedEquipment;
            public long MemberId;
            public string UnitDef;
            public int Level;
            public Row[] Inventory = Array.Empty<Row>(), Equipped = Array.Empty<Row>();
            public Row[] Candidates = Array.Empty<Row>();
            public string CandidatesFingerprint;
            internal SimSession Session;
            internal string InventoryFingerprint, EquippedFingerprint;
            internal readonly Dictionary<long, string> OwnerFingerprints = new Dictionary<long, string>();
        }

        // Only the freeze methods construct a choice. A confirmation retains this object,
        // never an inventory index freshly resolved from a mutable UI selection.
        public sealed class Choice
        {
            public readonly long MemberId, SourceOwnerId;
            public readonly int Index;
            public readonly bool Remove;
            public readonly string ItemRaw, SourceOwnerUnitDef, TargetSlot;
            public bool RequiresConfirmation => !Remove && SourceOwnerId > 0;
            internal readonly SimSession Session;
            internal readonly string UnitDef, InventoryFingerprint, EquippedFingerprint, SourceFingerprint;
            internal Choice(Snapshot snapshot, Row row, bool remove, string targetSlot = null)
            {
                MemberId = snapshot.MemberId; Index = row.Index; Remove = remove; ItemRaw = row.Raw;
                SourceOwnerId = remove ? 0 : row.OwnerId; SourceOwnerUnitDef = row.OwnerUnitDef;
                TargetSlot = targetSlot; Session = snapshot.Session; UnitDef = snapshot.UnitDef;
                InventoryFingerprint = snapshot.InventoryFingerprint;
                EquippedFingerprint = snapshot.EquippedFingerprint;
                if (SourceOwnerId > 0) snapshot.OwnerFingerprints.TryGetValue(SourceOwnerId, out SourceFingerprint);
            }
        }

        public static Snapshot Read(SimDriver driver, long memberId) => Read(driver != null ? driver.Session : null, memberId);

        internal static Snapshot Read(SimSession session, long memberId)
        {
            var snapshot = new Snapshot { MemberId = memberId };
            if (session == null || !session.IsAlive || memberId <= 0 || memberId > uint.MaxValue) return snapshot;
            var viewJson = session.ViewJson();
            if (!ObjectJson(viewJson)) return snapshot;
            string inventoryJson = session.QueryJson("inventory");
            if (string.IsNullOrWhiteSpace(inventoryJson) || inventoryJson.TrimStart()[0] != '[') return snapshot;
            var inventory = Parse<ItemList>("{\"items\":" + inventoryJson + "}")?.items;
            var inventoryRows = MailWindow.RowsOfArray(inventoryJson);
            if (inventory == null || inventory.Length != inventoryRows.Count) return snapshot;
            var bag = ReadRows(inventory, inventoryRows.ToArray(), 0, "");
            if (bag == null) return snapshot;
            var candidates = new List<Row>(bag);
            var fingerprint = new System.Text.StringBuilder(inventoryJson);
            var seen = new HashSet<long>();
            EntityDto found = null;
            foreach (var raw in MailWindow.RowsUnder(viewJson, "\"entities\""))
            {
                var entity = Parse<EntityDto>(raw);
                if (entity == null || entity.team != "Player" || entity.is_pet) continue;
                // Never silently choose one of two malformed identities.
                if (entity.id <= 0 || entity.id > uint.MaxValue || !seen.Add(entity.id)
                    || entity.stats == null || entity.stats.level <= 0 || string.IsNullOrEmpty(entity.unit_def)
                    || entity.equipped == null) return snapshot;
                var rawRows = MailWindow.RowsUnder(raw, "\"equipped\"");
                if (entity.equipped.Length != rawRows.Count) return snapshot;
                var worn = ReadRows(entity.equipped, rawRows.ToArray(), entity.id, entity.unit_def);
                if (worn == null) return snapshot;
                var wornFingerprint = "[" + string.Join(",", rawRows) + "]";
                snapshot.OwnerFingerprints.Add(entity.id, wornFingerprint);
                candidates.AddRange(worn);
                fingerprint.Append('|').Append(entity.id).Append(':').Append(entity.unit_def).Append(':').Append(wornFingerprint);
                if (entity.id != memberId) continue;
                found = entity; snapshot.Equipped = worn; snapshot.EquippedFingerprint = wornFingerprint;
            }
            if (found == null) return snapshot;
            // Bench rows have no enriched item projection and native EquipItem requires a grid
            // entity. Report the gap; never deploy/bench a character merely to browse its gear.
            string bench = Roi.Net.ServerClient.InnerObject(viewJson, "\"roster\":");
            snapshot.HasBenchedEquipment = System.Text.RegularExpressions.Regex.IsMatch(bench,
                @"""equipped""\s*:\s*\[\s*\{");
            snapshot.Session = session; snapshot.UnitDef = found.unit_def; snapshot.Level = found.stats.level;
            snapshot.Inventory = bag; snapshot.InventoryFingerprint = inventoryJson;
            snapshot.Candidates = candidates.ToArray(); snapshot.CandidatesFingerprint = fingerprint.ToString();
            snapshot.Available = true;
            return snapshot;
        }

        public static Row FindCandidate(Snapshot snapshot, Row row)
        {
            if (snapshot == null || row == null) return null;
            foreach (var candidate in snapshot.Candidates)
                if (candidate.OwnerId == row.OwnerId && candidate.Index == row.Index) return candidate;
            return null;
        }

        // Metadata restrictions only. Mastery and native routing are checked by the host on
        // a disposable copy ONLY after Equip/transfer confirmation, never on item selection.
        public static string EquipIssue(Snapshot snapshot, Row row)
        {
            if (!Ready(snapshot, row) || row.Wieldable == null) return "UI_EX_GEAR_UNAVAILABLE";
            if (row.OwnerId == snapshot.MemberId) return "UI_EX_GEAR_ALREADY_EQUIPPED";
            if (!UnitFits(snapshot.UnitDef, row.Wieldable)) return "UI_EX_GEAR_CLASS_BLOCKED";
            if (row.Item.level_req <= 0) return "UI_EX_GEAR_UNAVAILABLE";
            return snapshot.Level < row.Item.level_req ? "UI_EX_GEAR_LEVEL_BLOCKED" : null;
        }

        public static string UnequipIssue(Snapshot snapshot, Row row)
        {
            if (!Ready(snapshot, row) || row.Wieldable == null) return "UI_EX_GEAR_UNAVAILABLE";
            // The 346 native host starts Seraphine/Rapis with weapons its old class matrix
            // refuses to re-equip. Do not strand the player by exposing Remove for that case.
            return UnitFits(snapshot.UnitDef, row.Wieldable) ? null : "UI_EX_GEAR_RESTORE_BLOCKED";
        }

        public static Choice FreezeEquip(Snapshot snapshot, int inventoryIndex)
            => FreezeEquip(snapshot, At(snapshot?.Inventory, inventoryIndex));

        public static Choice FreezeEquip(Snapshot snapshot, Row row, string targetSlot = null)
        {
            var current = FindCandidate(snapshot, row);
            if (current == null || current.Raw != row.Raw || current.OwnerUnitDef != row.OwnerUnitDef
                || EquipIssue(snapshot, current) != null) return null;
            return new Choice(snapshot, current, false, targetSlot);
        }

        public static Choice FreezeUnequip(Snapshot snapshot, int equippedIndex)
        {
            var row = At(snapshot?.Equipped, equippedIndex);
            return row != null && UnequipIssue(snapshot, row) == null ? new Choice(snapshot, row, true) : null;
        }

        internal static string PreviewDropSlot(SimDriver driver, Snapshot snapshot, Row row)
        {
            if(driver==null||row==null||!ReferenceEquals(driver.Session,snapshot?.Session))return null;
            bool remove=row.OwnerId==snapshot.MemberId;
            if((remove?UnequipIssue(snapshot,row):EquipIssue(snapshot,row))!=null)return null;
            if(row.OwnerId>0&&!remove)return null;
            string input=remove?UnequipInput(row.OwnerId,row.Index)
                :"{\"EquipItem\":{\"entity_id\":"+snapshot.MemberId+",\"inventory_idx\":"+row.Index+"}}";
            string target=null;
            var result=VerifiedInputSequence.Prepare(driver.Session,new[]{input},copy=>
            {
                var after=Read(copy,snapshot.MemberId);
                if(!after.Available)return false;
                if(remove)
                {
                    if(after.Equipped.Length!=snapshot.Equipped.Length-1||after.Inventory.Length!=snapshot.Inventory.Length+1)return false;
                    target="Inventory";return true;
                }
                if(!ExemplarEquipmentTransfer.Verify(copy,snapshot,row,null)||after.Equipped.Length==0)return false;
                target=after.Equipped[after.Equipped.Length-1].Slot;return true;
            },out _,out _);
            return result==VerifiedInputResult.Applied?target:null;
        }

        public static Result Apply(SimDriver driver, Choice choice, long currentInspectedId)
        {
            if (driver == null || choice == null) return Result.Unavailable;
            if (currentInspectedId != choice.MemberId || !ReferenceEquals(driver.Session, choice.Session)) return Result.Stale;
            if (choice.Session == null || !choice.Session.IsAlive) return Result.Unavailable;
            var current = Read(driver, choice.MemberId);
            if (!current.Available) return Result.Unavailable;
            if (current.UnitDef != choice.UnitDef || current.InventoryFingerprint != choice.InventoryFingerprint
                || current.EquippedFingerprint != choice.EquippedFingerprint) return Result.Stale;
            Row row = null;
            if (choice.Remove) row = At(current.Equipped, choice.Index);
            else foreach (var candidate in current.Candidates)
                if (candidate.OwnerId == choice.SourceOwnerId && candidate.Index == choice.Index) { row = candidate; break; }
            if (row == null || row.Raw != choice.ItemRaw || (!choice.Remove && row.OwnerUnitDef != choice.SourceOwnerUnitDef))
                return Result.Stale;
            if (choice.SourceOwnerId > 0 && (!current.OwnerFingerprints.TryGetValue(choice.SourceOwnerId, out var source)
                || source != choice.SourceFingerprint)) return Result.Stale;
            if ((choice.Remove ? UnequipIssue(current, row) : EquipIssue(current, row)) != null) return Result.Refused;
            if (choice.Remove) return Remove(driver, choice, current);

            // Unequip appends the original item at the bag tail. Both inputs are preflighted
            // against an identical native snapshot; publication is deferred until both finish.
            int bagIndex = choice.SourceOwnerId > 0 ? current.Inventory.Length : choice.Index;
            var inputs = new List<string>();
            if (choice.SourceOwnerId > 0) inputs.Add(UnequipInput(choice.SourceOwnerId, choice.Index));
            inputs.Add("{\"EquipItem\":{\"entity_id\":" + choice.MemberId + ",\"inventory_idx\":" + bagIndex + "}}");
            var result = driver.ApplyVerifiedInputs(choice.Session, inputs.ToArray(), copy =>
                ExemplarEquipmentTransfer.Verify(copy, current, row, choice.TargetSlot));
            return result == VerifiedInputResult.Applied ? Result.Applied : result == VerifiedInputResult.Stale ? Result.Stale
                : result == VerifiedInputResult.Unavailable ? Result.Unavailable : Result.Refused;
        }

        static Result Remove(SimDriver driver, Choice choice, Snapshot current)
        {
            // true only means the event parsed; read the host's changed loadout back.
            if (!driver.ApplyInput(UnequipInput(choice.MemberId, choice.Index))) return Result.Refused;
            if (!ReferenceEquals(driver.Session, choice.Session) || !choice.Session.IsAlive) return Result.Stale;
            var after = Read(driver, choice.MemberId);
            return !after.Available ? Result.Unavailable : after.EquippedFingerprint != current.EquippedFingerprint
                ? Result.Applied : Result.Refused;
        }

        static string UnequipInput(long id, int index)
            => "{\"UnequipItem\":{\"entity_id\":" + id + ",\"equipped_slot\":" + index + "}}";
        static bool Ready(Snapshot snapshot, Row row)
            => snapshot != null && snapshot.Available && snapshot.Session != null && snapshot.Session.IsAlive
                && snapshot.MemberId > 0 && row?.Item != null && !string.IsNullOrEmpty(row.Raw)
                && (row.Item.kind == "Weapon" || row.Item.kind == "Armor");
        static bool UnitFits(string classId, string[] wieldable)
            => wieldable.Length == 0 || Array.IndexOf(wieldable, classId) >= 0;
        static Row At(Row[] rows, int index)
            => rows != null && index >= 0 && index < rows.Length && rows[index]?.Index == index ? rows[index] : null;

        static Row[] ReadRows(ItemView[] items, string[] rawRows, long ownerId, string ownerUnitDef)
        {
            var rows = new Row[items.Length];
            for (int i = 0; i < rows.Length; i++)
            {
                if (items[i] == null || string.IsNullOrEmpty(items[i].def_id)) return null;
                var identity = Parse<IdentityDto>(rawRows[i]);
                if (identity == null) return null;
                rows[i] = new Row { Index = i, Item = items[i], Raw = rawRows[i], OwnerId = ownerId,
                    OwnerUnitDef = ownerUnitDef, InstanceId = identity.instance_id, Wieldable = identity.wieldable };
            }
            return rows;
        }
        static bool ObjectJson(string json) => !string.IsNullOrWhiteSpace(json) && json.TrimStart()[0] == '{';
        static T Parse<T>(string json) where T : class
        {
            if (!ObjectJson(json)) return null;
            try { return JsonUtility.FromJson<T>(json); }
            catch (ArgumentException) { return null; }
        }
        [Serializable] sealed class IdentityDto { public string instance_id; public string[] wieldable; }
        [Serializable] sealed class EntityDto
        {
            public long id; public string team, unit_def; public bool is_pet;
            public LevelDto stats; public ItemView[] equipped;
        }
        [Serializable] sealed class LevelDto { public int level; }
    }
}
