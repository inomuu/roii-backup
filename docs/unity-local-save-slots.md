# Unity local debug save slots

Implemented 2026-09-10, following `godot/save_store.gd` and the slots in
`godot/views/debug_view.gd`. This is a local developer tool, not cloud-save slots.

Use **Local session (dev) → Menu → Debug → Local Save Slots**. Five slots display
party name and lead level/character; `>` marks the active slot. Clicking another
slot saves the departing party before loading it. An empty slot starts a level-1
Seraphine + Rapis party. Save current slot writes immediately; autosave/quit keep
writing the active slot. Delete requires a second press within five seconds.
Deleting the active slot starts a fresh party immediately, preventing autosave
from resurrecting its old state. The old contents are backed up first.

Storage is under Unity's `persistentDataPath/dev_slots/`: `slot_1.json` through
`slot_5.json`, plus `active_slot.txt`. Existing `dev_session.json` imports into
slot 1 once; a migration marker prevents resurrection after deletion. The original
file remains available. Replacements use atomic writes with `.previous` backups;
deletions/unreadable files get uniquely named `.bak` copies. A failed slot load
leaves the current session/selection and unreadable file intact. If the active
slot cannot be loaded on startup, boot reports the failure and preserves it rather
than replacing it; the files can be recovered from backups.

`LocalSlotStore` is filesystem-only. `DevSaveStore` adapts the existing boot/save
hooks; `SessionBoot.LocalSlots` owns session changes. `LocalSavePanel` is retained
UI inside Debug. Actions are refused during online sessions, including direct
calls; local snapshots are never uploaded.

Validation: `LocalSaveSlotsProbe.Filesystem()` tests one-time migration, no
resurrection, five-slot isolation, active-slot persistence, overwrite backup,
and invalid indices. `LocalSaveSlotsProbe.Session()` requires a temporary-root
local preview with Debug open and exercises real load/delete buttons, exact state
hash restoration, isolated currency, confirmation, active reset, preservation of
malformed data, and online refusal. Removing the load button callback made it fail;
restoring it passed. No real local/cloud save was used. Unity compilation and
console checks passed, as did the 1,879-key localization guard.

The same update adds temporary **presentation-only** sprite aliases in
`ArenaView.Tables.Class`: Seraphine → swordsman_t1, Rapis → mage_t1. Both sprites
were checked in Game View; sim IDs, names and Potential Boards remain unchanged.
