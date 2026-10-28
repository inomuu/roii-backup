//! Canonical state hashing.
//!
//! Money-zone code (GDD §7.1) — this is the function the future
//! Phase 2 server uses to verify "did the client really get this
//! outcome from these inputs?" If two implementations of sim-core
//! produce different hashes for the same state, the entire trust
//! model collapses. So:
//!
//! Determinism guarantees:
//!   1. We use `serde_json` to_vec — compact (no whitespace), and
//!      struct fields serialize in declared order, NEVER alphabetic
//!      or randomized.
//!   2. `GameState` contains NO `HashMap`/`HashSet` (GDD §4.2 rule 4).
//!      Every collection is a `Vec` or `BTreeMap` whose order is
//!      part of the state.
//!   3. All numbers are integers (`#![deny(clippy::float_arithmetic)]`
//!      at the crate root). Floats would not be bit-equal across CPUs.
//!   4. blake3 of the canonical bytes → hex string. blake3 is
//!      cryptographically deterministic across all targets.
//!
//! Pre-approved deps (GDD §7.4): blake3, serde, serde_json.

use crate::state::GameState;

/// Hash the entire GameState into a stable hex string.
///
/// The hex string is what we pin in golden tests and chain in save
/// snapshots. Two GameStates produce equal hashes iff they are
/// byte-equal under canonical serialization.
pub fn state_hash(state: &GameState) -> String {
    let bytes = canonical_bytes(state);
    blake3::hash(&bytes).to_hex().to_string()
}

/// One link of the snapshot HASH CHAIN (GDD §4.4). Binds a snapshot's
/// `state_hash` at `tick` to the previous link's `chain_hash`, so a checkpoint
/// can't be swapped, reordered, or re-timed without breaking every later link —
/// tamper-evidence for the whole checkpoint sequence. Genesis link uses
/// `prev_chain = ""`.
pub fn chain_link(prev_chain: &str, tick: u64, state_hash: &str) -> String {
    let mut h = blake3::Hasher::new();
    h.update(prev_chain.as_bytes());
    h.update(&tick.to_le_bytes());
    h.update(state_hash.as_bytes());
    h.finalize().to_hex().to_string()
}

/// Deterministic instance id for a TRADEABLE item at the moment it drops (S3 —
/// docs/s3-economy-design.md §3). Binds the RNG stream POSITION (`rng_fingerprint`
/// — see `Pcg32::fingerprint`) to the drop `tick` and item `def_id`. Because a
/// server replay reaches the exact same RNG position at the same drop, it derives
/// the SAME id — so the ledger can DEDUP (two claims of the same id = one item,
/// dupes/forgeries collide with the real one). Different draws / ticks / defs →
/// different ids. Not stored in GameState (no schema/hash impact) — it's derived at
/// the promotion/register step and lives in the server ledger.
///
/// Returns the FULL 256-bit blake3 as a 64-char hex string (like `state_hash` /
/// `chain_hash`), NOT a truncated integer: this is a money-critical dedup key, and
/// a 64-bit truncation has a small-but-nonzero birthday-collision risk (two legit
/// drops → same id → a real item wrongly rejected as a dupe). Full width = zero.
pub fn instance_id(rng_fingerprint: u64, tick: u64, def_id: &str) -> String {
    let mut h = blake3::Hasher::new();
    h.update(b"item-instance:");
    h.update(&rng_fingerprint.to_le_bytes());
    h.update(&tick.to_le_bytes());
    h.update(def_id.as_bytes());
    h.finalize().to_hex().to_string()
}

/// Canonical byte representation of a `GameState`.
///
/// Phase 0 uses `serde_json` compact output. JSON is not the most
/// compact wire format (CBOR / postcard would be smaller), but:
///   - it's human-readable for debugging golden tests,
///   - serde_json is widely audited and unlikely to change output
///     subtly between versions,
///   - we don't store these bytes per-tick — only on snapshot.
///
/// If profiling shows snapshot writes are a bottleneck (unlikely
/// at the budget in GDD §4.6), switch to postcard at the version
/// boundary.
fn canonical_bytes(state: &GameState) -> Vec<u8> {
    serde_json::to_vec(state).expect("GameState always serializes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{Cell, Entity};

    #[test]
    fn hash_is_stable_for_identical_state() {
        let mut a = GameState::new(42, 16, 8);
        let mut b = GameState::new(42, 16, 8);
        a.grid.spawn(Entity::idle(1, Cell::new(0, 0)).with_walk(Cell::new(5, 3), 400));
        b.grid.spawn(Entity::idle(1, Cell::new(0, 0)).with_walk(Cell::new(5, 3), 400));
        assert_eq!(state_hash(&a), state_hash(&b));
    }

    #[test]
    fn hash_changes_when_state_changes() {
        let mut a = GameState::new(42, 16, 8);
        let mut b = GameState::new(42, 16, 8);
        a.grid.spawn(Entity::idle(1, Cell::new(0, 0)).with_walk(Cell::new(5, 3), 400));
        // b differs only in entity start position.
        b.grid.spawn(Entity::idle(1, Cell::new(1, 0)).with_walk(Cell::new(5, 3), 400));
        assert_ne!(state_hash(&a), state_hash(&b));
    }

    #[test]
    fn hash_changes_when_seed_changes() {
        let a = GameState::new(42, 16, 8);
        let b = GameState::new(43, 16, 8);
        assert_ne!(state_hash(&a), state_hash(&b));
    }

    #[test]
    fn instance_id_is_replayable_and_unique() {
        use crate::rng::Pcg32;
        // Two runs of the SAME seed = the client and a server REPLAY. Advance both
        // to the same drop point (10 draws) → identical RNG position.
        let mut client = Pcg32::new(42);
        let mut server = Pcg32::new(42);
        for _ in 0..10 {
            client.next_u32();
            server.next_u32();
        }
        // Same position + tick + def → SAME id → the ledger dedupes (a forged/duped
        // claim of this item collides with the real one).
        assert_eq!(
            instance_id(client.fingerprint(), 500, "redling"),
            instance_id(server.fingerprint(), 500, "redling"),
            "a server replay derives the same instance id as the client (dedup key)"
        );
        // The very next draw = a new position → a DIFFERENT id (no accidental dup).
        client.next_u32();
        assert_ne!(
            instance_id(client.fingerprint(), 500, "redling"),
            instance_id(server.fingerprint(), 500, "redling"),
        );
        // Different tick / def at the same position → different id.
        assert_ne!(
            instance_id(server.fingerprint(), 500, "redling"),
            instance_id(server.fingerprint(), 501, "redling"),
        );
        assert_ne!(
            instance_id(server.fingerprint(), 500, "redling"),
            instance_id(server.fingerprint(), 500, "poring_card"),
        );
    }
}
