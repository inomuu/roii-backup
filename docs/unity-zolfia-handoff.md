# Unity: Zolfia — handoff 2026-09-24

From the sim/gameplay session to the Unity client (Codex). Sim 0.0.476 / catalog 0.0.211;
restart the Editor after the native library is staged.

## Who

**Zolfia** (`zolfia`, TH ซอลเฟีย) — SSR · Destroyer · Ice · sword and shield. The Dulahaness
boss of Duskmourn (`dullahan`), won to the party: a headless knight who carries her own head,
burning with a cold **blue** flame. Art key is `swordsman` until she has her own sheet; her
off-hand slot is mechanically a shield, but the look can be her head held in that hand.
Regalia: **Winter's Oath** (`regalia_winters_oath`, SSR). Not in any gacha pool — she is meant
to drop from the boss (not built yet).

## Kit (ids for VFX / labels)

| id | slot | what shows |
|---|---|---|
| `coldflame_knight` | passive basic | a melee Ice strike; lays **Winter Chill** on what she hits |
| `coldflame_split` | skill 1 | a heavy Ice blow (`skill_fx`); at S2 a 3×3 sweep around her |
| `remorseless_winter` | skill 2 | an **aura** around her, 5×5, for 8 s: slows and burns every enemy inside once a second (the ticks arrive as `field_hits` from her) |
| `winters_verdict` | ultimate | Ice on every enemy in the room (`ult_fx`) |

## Statuses worth drawing (buff kinds on the enemy)

- `WinterChill` — she has struck it (a quiet frost mark).
- `Shattered` — it broke while chilled: takes +20% Ice and Fire. The payoff state; worth a
  clear look (cracked ice / blue flame on the body).
- `WinterAura` — on HER while the aura is up.

Crit labels: `docs/unity-crit-label-handoff.md`. Element name: `docs/unity-element-ice-handoff.md`.
