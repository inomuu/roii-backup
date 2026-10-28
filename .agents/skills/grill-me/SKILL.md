---
name: grill-me
description: Relentlessly interview the user to stress-test a plan or design BEFORE building — walk every branch of the design tree, one decision at a time, each with a recommendation grounded in the code/docs. Use when the user wants to lock a design before implementing, says "grill me", "stress-test this", "let's nail the design", or is about to start a non-trivial feature with open decisions. Adapted for this Rust+Godot deterministic-sim project.
---

Interview the user relentlessly about a plan or design until every decision branch is resolved and you share one understanding. Then capture the result as a plan. This is the "decide before you build" gate — it exists to prevent rework on a codebase where a wrong call means re-pinning goldens, a VERSION bump, or unwinding a sim/renderer split.

## Core rules

1. **Ground every question in the code first — never ask theory.** Before asking, read the source of truth: the relevant Rust modules, `docs/taskbar-arpg-gdd-phase0.md` (design + §4 technical rules), `docs/combat-formulas.md`, `docs/skills-catalog.md`, `TODO.md`, and the auto-memory index. If the codebase already answers a question, state the answer instead of asking. Prefer the Explore agent for broad sweeps.
2. **Walk the design tree, one branch at a time.** Resolve dependencies in order — a downstream choice you can't decide until an upstream one is settled waits its turn. Don't jump around.
3. **One decision per question (small batches OK via the tool).** Use `AskUserQuestion`. Each question is a real fork where the answer changes what you build — not a preference with an obvious default (pick those yourself and say so). Group at most a few tightly-related forks per call.
4. **Always recommend.** Make your recommended option the FIRST choice and append "(Recommended)" to its label, with a one-line why. The user is choosing between informed options, not generating them from scratch.
5. **Surface the cost of each fork in this project's terms.** When an option implies a golden re-bless, a `sim_core::VERSION` bump, a new serialized field, a PRNG draw-order change, a new EquipSlot/InputEvent variant, or a sim-vs-renderer boundary decision — say so in the option description. These are the expensive, hard-to-reverse choices here.
6. **Respect the invariants — don't offer options that break them.** sim-core is integer-only, no floats, single seeded PCG32, deterministic, gated draws keep goldens pinned (see AGENTS.md). Gameplay logic lives in the sim; Godot only renders. If a fork tempts a violation, flag it rather than listing it as a viable branch.

## Loop

1. **Map the tree.** From the request, list the open decisions and their dependencies. Read the code/docs to prune any the codebase already decides.
2. **Ask the next unresolved branch** (most upstream first) via `AskUserQuestion`, recommendation first, costs noted. Honor "Other" / free-text answers — they often redraw the tree; re-map if so.
3. **Record + propagate.** Apply the answer, then re-check which downstream branches it just opened or closed.
4. **Repeat** until no branch with a material consequence remains.
5. **Confirm scope explicitly.** State what is IN and what is OUT this round (this project ships features in tight slices — e.g. "dual-wield system only, other Assassin skills separate").

## Output

When the tree is fully resolved, write the locked design as a plan: enter plan mode (`EnterPlanMode`) and author the plan file with a **Context** (why), the recommended approach only, the concrete files to touch, the determinism/VERSION/golden implications, and a verification section (`cargo test -p sim-core --lib` / `--tests`, `tools/smoke.sh`). Then `ExitPlanMode` for approval. If already mid-implementation, instead append the resolved decisions to the active plan file or a `docs/session-*.md` log.

## Stop conditions

- Every branch with a real consequence is decided → write the plan.
- The user says stop / "good enough" → summarize what's locked and what's still open, then stop.
- A decision needs data you can get from a headless run → propose a quick `cargo run -p sim-cli` to settle it with numbers rather than guessing.
