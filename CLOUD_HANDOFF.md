# Cloud continuation handoff — 2026-10-29

This repository imports the current working source from `D:/roii/idle_mmorpg/idle_mmorpg-master` into `inomuu/roii`. It is a source snapshot, not a migration of the older repository history. The original local parent Git checkout was not a usable source history (its current branch could not be resolved).

Read `AGENTS.md`, `CLAUDE.md`, and the relevant `TODO.md` section before changes. The client is Unity; Godot is historical, except for shared localization source. Preserve deterministic integer-only Rust simulation and the online-required product direction.

## Environment

- Rust stable (workspace minimum 1.80), Cargo, Python 3, Git and Git LFS.
- Fetch LFS assets with `git lfs pull` if this checkout contains LFS pointers.
- Unity editor version: 6000.6.3f1. Client project: `unity/ROI`.
- Start Rust validation with `cargo test -p sim-core --lib`. Use the appropriate checks in CLAUDE.md for subsequent changes.
- Unity rendering and Editor probes require a configured Unity installation and its dependencies; a passing Rust test does not validate the UI.
- Local caches, logs, credentials, compiled native plugins and asset packs excluded by the existing ignore rules are not uploaded. Restore licensed dependencies separately. Build sim-ffi for the target platform before running the Unity client.
- Backend tests need a dedicated test database. Do not use a production or development database for tests that truncate tables.

## Continue work

The current request is to move the latest source to GitHub for Cloud continuation. No new gameplay or UI change was specified. Inspect TODO.md and ask the user which next feature or fix they want; do not assume every backlog item is authorized. Existing docs contain detailed subsystem handoffs.

No application tests were run for this source-transfer task. No deployment is part of this transfer. Configure and publish a Cloud environment for inomuu/roii, then start work from its main branch.

## Source-only first upload (user selected)

The first Cloud upload deliberately omits art, reference/review images, archives, legacy Godot client files, and Unity visual assets/scenes. It includes Rust, Unity C# scripts and Editor tools, project/package settings, localization source, game data, and Markdown documentation. This is suitable for code work and Rust tests, but is NOT a complete runnable Unity project or release build.

The full original snapshot remains locally on branch `local/full-source-snapshot` at d0aaba76b88fa5b0cdc01bcf7b4ee9b4922626db. All omitted files remain on disk. `CLOUD_OMITTED_FILES.txt` lists paths omitted from the first upload; restore them from the local full snapshot in a separate upload before Unity visual testing or release builds. Do not recreate missing assets or delete their references as a fix.
