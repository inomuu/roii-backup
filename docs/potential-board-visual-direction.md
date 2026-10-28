# Potential Board — visual direction and graph audit

Date: 2026-09-10. Status: **interactive graphic concept for review**.

The first specimen is **Seraphine / Sentinel / Physical**. This work explores the
screen, node hierarchy, layout and selection feedback before implementing the
Potential Board in the game. The preview does not spend game inventory, save
progress, implement combat effects or establish final economy values.

## Visual references

- [Himeko / Erudition — game screenshot in the BlueStacks guide](https://www.bluestacks.com/blog/game-guides/honkai-star-rail/hsr-himeko-character-guide-en.html):
  useful for a clear central structure and space between the main ability nodes.
- [Luocha / Abundance — game screenshot in the Millenium guide](https://www.millenium.org/guide/412800.html):
  useful for curved framing and restrained botanical decoration behind the graph.
- [Seele / Hunt — game screenshot in the Eurogamer guide](https://www.eurogamer.es/honkai-star-rail-seele-mejor-build-materiales-ascension-habilidades-equipo-cono-de-luz-eidolon):
  useful for an elongated silhouette with diagonal branches.

These are visual references, not specifications for our unlock rules. The concept
uses original vector graphics; screenshots are not production UI assets. The
direction combines the legibility of the Himeko example with the organic framing
of the Luocha example: ivory icons, aged gold, a dark green/teal field and low
contrast ornament. The graph must remain readable without decorative effects.

## Canonical design and what is provisional

Read [Skill System Redesign §5.3 and §5.5](skill-system-redesign.md) together:
§5.5 records the 2026-09-07 choice of the HSR-inspired graph. The earlier
14-node linear proposal and the “not decided” comparison above §5.5 remain in
the document as older alternatives; they do not override that later decision.

- **17 purchasable nodes plus one root:** 10 one-time stat nodes (`s1`–`s10`),
  four skill nodes (`k1`–`k4`) and three one-time major traces (`A`, `B`, `C`).
- A node is reachable when at least one connected neighbour is unlocked. This
  is an undirected graph, not a fixed prerequisite sequence.
- Major traces A/B/C have Job tier gates **2 / 4 / 6** respectively.
- Ability level is capped by Job Level tier; the proposed skills rank **1–10**.
- `k4`, Basic, is a special case: it opens after any **two of three** major traces
  are unlocked. It has no line to the graph in the source sketch.
- Production direction is **one shared graph per Doctrine**: Sentinel, Striker,
  Destroyer, Catalyst and Support. Characters in one Doctrine share placement,
  connections and stat nodes; their skills and three major trace effects differ.

The interactive layout variants are visual studies. They are not five approved
Doctrine graphs. If the same source graph is shown in several arrangements,
changing arrangement must not quietly change its edge list or stat allocation.

## Graph transcription for the preview

The following is a reasonable explicit transcription of the ASCII sketch in
§5.3, including the provisional interpretation of `s4` described below:

```text
root — s1     root — s5
s1 — s2       s2 — k1
k1 — s4       s4 — A
k1 — s3       s3 — B
B — k2        k2 — s1
B — s7        s7 — s6       s6 — s5
k1 — s10      s10 — k3      k3 — s9      s9 — s8      s8 — C

k4: separate, unlocked by two major traces; no invented graph edge.
```

Source stat labels, retained as specimen data rather than rebalanced for Sentinel:

| Node | Source label | Node | Source label |
|---|---|---|---|
| s1 | ATK +4% | s6 | DEF +4% |
| s2 | ATK +3% | s7 | HP +4% |
| s3 | ATK +3% | s8 | Element DMG +4% |
| s4 | CRIT +2% | s9 | Charge Rate +6% |
| s5 | HP +4% | s10 | Element DMG +6% |

Two ambiguities must remain visible to the next implementer:

1. **`s4` is printed beside the A–k1 vertical line**, without a separate connector.
   The preview treats it as inline (`k1–s4–A`). Confirm this when finalizing the
   production graph; do not present the interpretation as an explicit edge table
   already authored in the design.
2. **C has one entry through `k1–s10`.** The graph provides multiple approaches
   elsewhere, but the source does not draw another entry into the C chain.
   Do not add a cross-link to make every major have two routes. The prose about
   alternative paths is not an instruction to invent an edge absent from the sketch.

Decorative strokes must look different from real unlock connections. In
particular, an ornament passing behind `k4` must not imply a prerequisite edge.

## Seraphine content

Use [Exemplar Roster §8](exemplar-roster.md), which supersedes the older Holy
Seraphine art brief. Seraphine is Physical, Doctrine Sentinel, wielding a two-handed
sword. Skill assignment from the kit and graph roles:

| Node | Name | Authored role |
|---|---|---|
| k1 | Iron Chains | Pull and taunt enemies; increases her DEF. |
| k2 | Steel Break | Nearby Physical area attack with extra Toughness damage. |
| k3 | Aegis | Party absorption shields that burst when broken. |
| k4 | Basic | Two-handed Physical attack; no formal skill name is authored. |
| A | Iron Edge | ATK increases by 25% of current DEF, recalculated every second. |
| B | Iron Wall | The whole party receives 10% less damage. |
| C | Second Wall | Broken Aegis shields refund Seraphine's Limit Break gauge. |

The roster explicitly leaves major-trace numbers unsettled; the displayed 25%
and 10% are authored proposals, not tuned release values. Second Wall's refund
amount and trigger cap are not specified. Do not invent a numeric rank preview
for it. Major traces have no upgrade ranks.

The kit also names the passive **Bulwark** (DEF from taunted enemies), but does not
assign it to an additional board slot. Do not replace `k4` with Bulwark or add an
eighteenth purchasable node without reconciling that design.

Lucielle is **Support / Holy**. Her roster names **Sagitta Lucis**, **Benedictio
Lucis** and **Lumen Contagio**, but its older kit does not author a complete
four-slot Potential mapping or three new A/B/C major traces. Her older
star-to-skill-level rules are not a ready-made board implementation. Use Seraphine
for populated detail-panel examples rather than inventing Lucielle passives.

## Review interactions

- Select a node to inspect its type, effect, rank or one-time status and gate.
- Distinguish purchased, reachable, gated and selected states with both shape/icon
  treatment and colour. Selection must not imply a purchase.
- Show the path to a selected destination independently of decorative ornament.
- A preview unlock or reset affects only mockup state. Any example Job tier,
  currency or material quantity must be labelled as specimen data.
- A layout switch changes the visual arrangement while preserving the specimen
  connections and current selection wherever possible.
- Keep the right-hand details panel readable at the target desktop viewport;
  avoid hiding important requirements in tiny text on the graph.

## Reusable Unity handoff

Proposed presentation components, not a claim that they already exist:

| Piece | Responsibility |
|---|---|
| Doctrine layout asset | Normalized node coordinates, authored edges and ornament reference; stable node IDs. |
| Node view | Reusable minor, skill and major visual variants; selected, reachable, locked and purchased states. |
| Connection view | Draw only explicit edges; show unlocked links and inspected routes. |
| Detail panel | Bind selected-node name, icon, effect, rank, gate, material cost and action availability. |
| Board presenter | Read authoritative view state and dispatch upgrade requests through the existing bridge. |
| Ornament/background | Independent noninteractive graphic layer, reusable across characters of one Doctrine. |

Game eligibility, spending, rank changes, effects and save state belong in Rust;
the Unity layer presents those results. A Unity layout asset may store positions
and visual references, but must not become an independent gameplay rule source.

## Decisions still required before shipping mechanics

- Actual Job Level-to-tier mapping and the Commander Level part of the two-cap rule.
- Node costs, Growth Stone rarity progression and farming rates. The design calls
  for element-typed stones from ordinary farming, not stamina or weekday domains.
- Final stat distribution per Doctrine. The ATK-heavy sample is not a finalized
  Sentinel board; the design says Sentinel should emphasize HP/DEF.
- Per-character major-trace balance, Second Wall's refund and trigger limit, and
  reconciliation of the existing Bulwark passive with the board structure.
- Refund/reset policy and path selection for the “upgrade as far as materials
  allow” action, especially when equal-length routes exist.
- Final confirmation of the graph transcription and production layouts.

These decisions do not block reviewing the graphic concept. They do block
treating its demonstration data or interaction rules as shipping game mechanics.
