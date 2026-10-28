//! ONE integration-test binary for the whole sim.
//!
//! Every scenario used to be its own file directly under `tests/`, and Rust compiles each such
//! file into a SEPARATE crate + executable that static-links sim-core. Editing one line of the
//! lib therefore relinked all of them, and that link (symbol resolution + debug info) — not the
//! test code — was the bulk of `cargo test` wall-clock. Collapsing them into modules of a single
//! binary makes it ONE link.
//!
//! Nothing about the scenarios changed: same code, same pinned hashes. Run one of them with
//!   cargo test -p sim-core --test suite golden_beta9
//! (a filter on the test NAME, where you used to pass `--test golden_beta9`).
//!
//! Adding a scenario = drop the file in `tests/suite/` and add its `mod` line here.

mod anchor_roundtrip;
mod catalog_hygiene;
mod diag_band4;
mod diag_inventory_replay;
mod fuzz_anchor_roundtrip;
mod golden_alpha2;
mod golden_alpha3;
mod golden_alpha4;
mod golden_alpha6;
mod golden_beta4;
mod golden_beta5;
mod golden_beta6;
mod golden_beta7;
mod golden_beta9;
mod golden_roster_a;
mod golden_roster_b;
mod roster_common;
mod monster_active_cast;
mod recipe_supply;
mod overlay_catalog;
mod potential_coverage;
mod retired_key;
mod save_shape;
