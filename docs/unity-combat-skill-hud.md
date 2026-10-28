# Combat skill HUD (2026-09-11)

Unity CombatBar now shows each member's active loadout, followed by learned passive
skills, above that member's card. Ported from Godot overlay_view.gd/loadout_json and
hud_icon.gd: top-down dark cooldown mask, ceiling seconds (10 ticks/s), no UI timer.
Passive skills have a blue border and no cooldown overlay. Hover either kind for
localized name, level, blurb and detail. Zero-cooldown skills have no countdown;
these icons are informational, not manual Ultimate activation controls.

`combat_skills:<entity_id>` is a read-only sim-host query. It returns active skills
in slot order and learned passives, never another member's skills. Disabling an
active skill removes it from this row. No sim rules, state schema or server change.
The native Unity library must be rebuilt and the Editor fully restarted for the
new query. The staged library was rebuilt with this host addition during validation.

Icon source: existing Godot assets/icons, [VerArc Stash] Basic Skills and Buffs.
Resources/SkillIcons/mapping.json carries Godot's mapping plus approximate aliases
for the new fixed kits. No new generated art. Unknown skills use a fallback icon.
Retained icon objects refresh in place; tooltip paragraphs wrap to measured height.

Validation: sim-host 16 tests passed, localization guard passed, Unity compiled
without console errors. In-game 8 icons (4 per starter), live cooldown numbers,
and EventSystem raycast/pointer-enter verified. Game captures live in
unity/ROI/Temp/CodexCaptures/skill-hud-screen.png and skill-passive-hover-final.png.

Separate online issue: before the UI restart, account 13 PushNow returned
`start snapshot is not anchored (forged/unknown base)`. A recovery copy was saved
in unity/ROI/Temp/CodexSaveBackups/before-skill-hud.json. Subsequent login resumed
from the server checkpoint; no reset or overwrite was performed by this task.
Final explicit save-push verification was blocked by automatic approval review.
