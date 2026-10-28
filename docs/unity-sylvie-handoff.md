# Unity: Sylvie — handoff 2026-09-24

From the sim/gameplay session to the Unity client (Codex). Sim 0.0.473 / catalog 0.0.209;
the native library and StreamingAssets (`game_data.json`, `strings.csv`) are staged — restart
the Editor.

## Who

**Sylvie** (`sylvie`, TH ซิลวี่) — SR · Support · Thunder · staff. Easygoing to a fault,
forgets everything, always smiling as if nothing could be too hot or too cold. A pure buffer:
no heals. Art key (`ExemplarLine.skin_id`) is `mage` until she has her own sheet. In
`SR_POOL`; her Regalia is **Static Charm** (`regalia_static_charm`, SR).

## Kit (ids for VFX / labels)

| id | slot | what shows |
|---|---|---|
| `spark_tap` | passive basic | a ranged Thunder bolt, range 4 (`projectiles[].skill_id`) |
| `surge` | skill 1 | party buff cast — CRIT + attack speed for 10 s (`skill_fx`) |
| `arc_focus` | skill 2 | a buff that lands on ONE ally (the top-ATK damage dealer), not on her |
| `bolt_from_the_blue` | ultimate | party CRIT for 15 s + every ally's cooldowns −3 s (`ult_fx`) |

Ladder effects with something to draw: S4 — while Surge lasts, each crit throws a small
Thunder spark at the same target (arrives as a `field_hits` entry from the critting ally).

Crit labels for everyone: `docs/unity-crit-label-handoff.md`. Ultimate cut-in trigger:
`docs/unity-ult-cutin-handoff.md`.
