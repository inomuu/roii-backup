//! verify-server — S2 Slice 0: a headless replay-verify server (docs/s2-server-build-plan.md).
//!
//! Links `sim-core` directly ("one binary, two homes", GDD §9) so it runs the SAME
//! deterministic sim the client did. Endpoints:
//!   - GET  /health        — liveness
//!   - GET  /demo/bundle   — dev helper: play a short session, return a real SaveBundle
//!   - POST /verify        — the anti-cheat check (server-plan Model C)

// A panic here is not a failed request — `panic = "abort"` makes it a dead process, and
// `min_machines_running = 0` makes the next player pay a cold start. So production code carries
// NO `.unwrap()`: handle the None/Err, or `.expect()` with the reason it cannot happen.
// Raw `.lock()` is banned too (clippy.toml) — locks go through `LockExt::held` below, so the
// poison policy lives in one place instead of being inferred from Cargo.toml at 80 call sites.
// Both fire under `cargo clippy -p verify-server`, NOT under `cargo test`.
#![deny(clippy::disallowed_methods, clippy::unwrap_used, clippy::indexing_slicing)]
// Tests may unwrap and index freely: a panic there is a failed assertion, which is the point.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::indexing_slicing))]

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Query, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
mod bp;
mod commander;
mod daily;
mod weekly;
mod gacha;
mod lootbox;
mod farm;
mod steam_inv;
mod mailbox;
mod mailbox_store;
mod metrics;
mod schedule;
mod mint_coupon;
mod steam_import;
mod shop;
mod store;
mod tutorial;

/// The ONE place this server decides what a poisoned mutex means.
///
/// `panic = "abort"` in the workspace `Cargo.toml` means a panic never unwinds, so in a release
/// build a mutex cannot actually be poisoned and `.lock()` is infallible. That is a real
/// guarantee — and it lives in a build profile with nothing connecting it to the ~80 call sites
/// that depend on it. Flip that one line and every one of them becomes a latent panic that takes
/// the whole process down with it, because abort is also what makes a panic fatal.
///
/// So the call sites go through here instead. The policy is one line; changing it (to propagate
/// an error, to log, to deliberately die) is one edit rather than eighty, and no call site's
/// safety is an inference about a file it never mentions.
trait LockExt<T> {
    /// Lock, recovering the guard if a previous holder panicked.
    fn held(&self) -> std::sync::MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    #[allow(clippy::disallowed_methods)] // the one sanctioned `.lock()` — this IS the policy
    fn held(&self) -> std::sync::MutexGuard<'_, T> {
        // Recover rather than die. Unreachable in release (abort), but in a debug or test build
        // one panicking case must not wedge every later caller of the same mutex.
        self.lock().unwrap_or_else(|e| e.into_inner())
    }
}

use sim_core::data::{self, GameData};
use sim_core::events::{apply_input_with, InputAuthority, InputEvent};
use sim_core::hash;
use sim_core::replay::{self, SaveBundle, Snapshot};

/// PLACEHOLDER auth (S2 Slice 1). Real login = Steam encrypted app ticket → the
/// SAME issue: mint a session token + a SERVER-CHOSEN seed. In-memory only (no
/// Postgres yet); the interface is what stays when Steam replaces `/auth/dev`.
#[derive(Default)]
struct Sessions {
    next_id: AtomicU64,
    by_token: Mutex<HashMap<String, Account>>,
    /// name → account, so the SAME name always maps to the SAME account_id + seed
    /// (a stable identity → resume works, no new account each login). Placeholder
    /// for a real accounts table (Neon); real login keys on the Steam id.
    /// identity -> the account it signs into. MANY identities may point at one account, which is
    /// the whole reason this is not keyed by the account's own name any more.
    by_identity: Mutex<HashMap<String, Account>>,
    /// SINGLE ACTIVE SESSION: account_id → its current token. A new login supersedes the
    /// old (drops the old token from `by_token` → its requests 401) so two devices can't
    /// run the same account at once (concurrent saves clobber + open economy dup).
    active: Mutex<HashMap<u64, String>>,
    /// TAINTED sessions: accounts whose last save was REJECTED for an integrity reason
    /// (replay divergence, unanchored/forged start, forged/mismatched fusion nonce). The
    /// token stays valid (so the client can pull-and-revert), but `/fuse/roll` and the gacha
    /// are refused while tainted — a doomed session can't keep harvesting rolls (or spamming
    /// outstanding rows). Cleared when the account next stores a VERIFIED save (it recovered) or re-logs
    /// in. In-memory only: a restart clears it; the next save re-taints if still broken.
    tainted: Mutex<HashSet<u64>>,
}

#[derive(Clone, serde::Serialize)]
struct Account {
    account_id: u64,
    name: String,
    /// SERVER-chosen RNG seed for this account's game — kills client seed-grinding
    /// (the client can't pick a lucky seed; new_game(seed) uses this).
    seed: u64,
}

/// Placeholder server secret — real deploy loads it from env/secrets. Only used to
/// make the dev token non-trivial to forge.
const DEV_SECRET: &str = "dev-placeholder-secret";

/// A fresh SERVER-random seed for a new account, stored once → stable forever. NOT
/// derived from the player-chosen name, so a player can't pick their seed by trying
/// names (seed-grinding). Masked to a POSITIVE i64: the Godot client holds the seed
/// in a signed 64-bit int and `SimBridge.init_game(seed: i64)` takes i64 (a full u64
/// > i64::MAX would overflow there). 63 bits of entropy is plenty.
fn random_seed() -> u64 {
    rand::random::<u64>() & 0x7FFF_FFFF_FFFF_FFFF
}


/// The AUTHORITATIVE cloud save for one account — stored ONLY after the pushed
/// bundle verified, so the server never persists a forged state. In-memory
/// placeholder (Postgres + object storage is a later slice).
#[derive(Clone, serde::Serialize)]
struct StoredSave {
    /// Serialized final GameState (the client resumes by `load_json`-ing this).
    state_json: String,
    final_tick: u64,
    final_hash: String,
    /// Server clock when this save was accepted. Only the boot anchor reads it — see
    /// `RESTART_GRACE_MS`. 0 = written before the column existed (treated as ancient).
    #[serde(skip)]
    saved_at_ms: i64,
}

/// The last PINNED checkpoint of an account's hash chain (S1c) — the append-only
/// head the server commits the client to. Ingested segments must CONTINUE this
/// head (chain-link consistent + strictly later tick); anything that diverges is a
/// rollback/dupe/rewrite and is rejected. Pinning is O(1) (no replay) — the cheap
/// continuous-integrity layer; real replay (Slice 4) runs only on selected segments.
#[derive(Clone)]
struct PinnedHead {
    head: Snapshot,
    /// How many checkpoints have been pinned for this account (the chain length).
    count: usize,
}

/// Running verify-cost telemetry (Slice 4). `replayed / total` is the fraction of
/// submissions that actually cost a sim replay — the design goal is to keep this
/// LOW (only tradeable/audited/suspicious paths), so compute scales with tradeable
/// activity, not with farming volume.
#[derive(Default)]
struct VerifyStats {
    total: AtomicU64,
    replayed: AtomicU64,
}

#[derive(Clone)]
struct AppState {
    data: Arc<GameData>,
    /// Durable backing for accounts/saves/trusted (None = in-memory only).
    db: Option<sqlx::PgPool>,
    sessions: Arc<Sessions>,
    saves: Arc<Mutex<HashMap<u64, StoredSave>>>,
    /// Accounts with a POST /save in progress — one at a time each (audit 2026-10-02): a save is a
    /// full replay on the shared CPU, and nothing stopped one account from running many at once.
    saving: Arc<Mutex<HashSet<u64>>>,
    /// Replays running at once, server-wide. The permit lives INSIDE the blocking task (audit
    /// 2026-10-03): a timed-out request frees its in-flight permit and save slot, but its replay
    /// cannot be cancelled, so under load orphaned replays piled up without bound.
    replay_slots: Arc<tokio::sync::Semaphore>,
    /// Ticks replayed per account in the current window: (window start ms, ticks). Charged before
    /// the replay, REJECTED saves included (audit 2026-10-03: a refused save cost a full replay
    /// and used none of the time budget, so one account could replay garbage back to back).
    replay_budget: Arc<Mutex<HashMap<u64, (i64, u64)>>>,
    /// Last Steam inventory read per account (ms): each one is a call on the publisher key, which
    /// Steam rate-limits for the whole app.
    steam_reads: Arc<Mutex<HashMap<u64, i64>>>,
    pins: Arc<Mutex<HashMap<u64, PinnedHead>>>,
    stats: Arc<VerifyStats>,
    /// Per-account set of TRUSTED state hashes — the anchor for replay. A bundle's
    /// `start` is only accepted if its hash is in here: the genesis (new_game from
    /// the account's server seed, which we reproduce) or an end-state we previously
    /// confirmed. Stops a forged `start` (a god character + empty log) from passing
    /// verify. Seeded lazily with the genesis on first save.
    trusted: Arc<Mutex<HashMap<u64, HashSet<String>>>>,
    /// (Mail lives in the `mailbox` TABLE via `mailbox_store`; settled claim op-ids live in the
    /// `consumed_market_ops` TABLE, looked up per save for just the ops that save presents. The
    /// in-game Gil market and its in-memory ledgers were removed in 0.0.490.)
    /// How many crystal-conservation breaks the last sweep found. A CACHED number: `/health` reads
    /// it, the hourly sweep writes it. Computing it per health check would run a GROUP BY over
    /// the whole ledger every few seconds for a question that changes hourly at most.
    crystal_breaks: Arc<std::sync::atomic::AtomicUsize>,
    /// Shared secret gating the dev/economy endpoints (mail-send,
    /// bp-season, flags). From the DEV_KEY env. None → those endpoints are DISABLED
    /// (fail closed). Callers pass `X-Dev-Key`. Always `None` under `PROD=1`.
    dev_key: Option<String>,
    /// Shared secret for the OPS routes (refund, compensation mail, battle-pass season, flags,
    /// storage / balance reports) — from `ADMIN_KEY`, callers pass `X-Admin-Key`. Separate from
    /// `dev_key` so production can run ops without carrying the test-fixture key. None → the ops
    /// routes answer 503 (fail closed), except that a non-PROD server also accepts `X-Dev-Key`.
    admin_key: Option<String>,
    /// `PROD=1`: the test-fixture routes (`/auth/dev`, crystal grants, demo / verify / ingest /
    /// submit, mail/claim, steam-mint-test) are not mounted at all and `DEV_KEY` is ignored.
    prod: bool,
    /// LIVE multiplier on every stage-2 rate, in basis points (10000 = x1, unchanged).
    ///
    /// Restores the dial that 0.0.256 removed. Stage 2 used to be a flat `DROP_HIT_DEN` env
    /// var, so a rate could be retuned by restarting this process; moving the rate onto the
    /// ticket made it a SIM constant, which is correct for determinism and useless for
    /// operations. `docs/crowd-scaled-rare-drops.md` needs to scale rates HOURLY against the
    /// number of active farmers, and that cannot mean deploying sim-core every hour.
    ///
    /// So the sim keeps the deterministic BASE rate and this scales it at issuance. Safe by
    /// construction: grants are server-authored, and `drop_grants_valid` checks only that the
    /// nonce is ours — it never re-derives the odds — so the multiplier never has to agree
    /// with anything the client replayed.
    ///
    /// `RARE_RATE_MULT_BP`. Defaults to 10000, so deploying this changes nothing.
    rare_rate_mult_bp: u32,
    /// Telemetry accumulator. In memory and flushed on a timer — see `metrics.rs` for why it is
    /// not written per save (the DB write rate is the measured scaling wall, not CPU).
    metrics: Arc<Mutex<metrics::Metrics>>,
    /// Coarse per-IP DoS backstop: (request count, window start) per client IP. Loose
    /// (only catches flooding) — the real per-action bound is the replay-gate.
    rate: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    /// Global in-flight request cap (load-shed). `rate` bounds one caller's RATE; this bounds
    /// the server's TOTAL concurrent work, however the load is spread across IPs. On the 256MB
    /// machine (`fly.toml`) it is really an OOM guard: an unbounded flood of request futures,
    /// each able to hold up to the 2MB body, would exhaust RAM and abort the process (= outage).
    /// Sized by `MAX_INFLIGHT`; a permit is taken per request in `concurrency_limit`.
    inflight: Arc<tokio::sync::Semaphore>,
    /// The per-server secret keying every server-roll nonce (`drop_nonce` and friends each add
    /// their own blake3 domain string, so the streams stay independent). SERVER_ROLL_SECRET env,
    /// falling back to the legacy REFINE_SECRET name, then a dev value.
    roll_secret: String,
    /// The FARM BANNER events (`farm.rs`), from the `FARM_EVENT` env var at boot — past ones
    /// included, since their tickets still pull. Empty = no event.
    farm_events: Vec<farm::Event>,
    /// Server-roll FUSION (design §7.8): the per-account OUTSTANDING roll set — issued by
    /// `/fuse/roll`, not yet seen consumed by a verified save. A roll that stays here is a
    /// peek-and-skip suspect (the client saw the outcome and never pressed the button).
    fuse_rolls: Arc<Mutex<HashMap<u64, Vec<OutstandingRoll>>>>,
    /// Fusion nonces already consumed by a verified save — so a growing-log re-save of the same
    /// session does not re-flag them as forgeries.
    fuse_consumed: Arc<Mutex<HashMap<u64, HashSet<u64>>>>,
    /// Anti-fast-forward: per-account SESSION anchor `(tick, server_ms)`. A save's tick
    /// advance since the anchor may not exceed real elapsed server time × rate (the game
    /// runs at 10 t/s and has NO offline earnings, so ticks only accrue during connected
    /// real time). Anchored at each `GET /save` (handshake/resume) — NOT per push, so
    /// request-spamming can't inflate the allowance. Replay-verify alone proves a run is
    /// deterministic, NOT that it fits real time; this closes the "push a 14h log in 2s"
    /// fast-forward. See `docs/online-robustness.md`.
    tick_ref: Arc<Mutex<HashMap<u64, TickAnchor>>>,
    /// LAYER 2 (`docs/competition-integrity.md`): durable per-account cumulative excess. Layer 1
    /// is per-session and its anchor resets on every handshake; this one does not, so it is the
    /// only thing that sees a re-login cheater. Signed and bounded by `EXCESS_FLOOR`.
    lifetime_excess: Arc<Mutex<HashMap<u64, i64>>>,
    /// ACTIVE battle-pass season (loaded at startup; None = BP disabled, fail-closed).
    season: Arc<Mutex<Option<bp::Season>>>,
    /// SCHEDULE TRACKS, built ONCE at boot. Config, not state — nothing mutates it, so no lock.
    ///
    /// Not rebuilt per request: `any_opens_on` is consulted on every accepted save, and a save
    /// lands every 10 s per online player. Rebuilding the list there would allocate a Vec and its
    /// Strings on the measured scaling path, for an answer that cannot change between deploys.
    tracks: Arc<Vec<schedule::TrackDef>>,
    /// Precomputed Commander-level CXP curve (`commander::build_level_table`), built ONCE at boot like
    /// `tracks` — config, not state, nothing mutates it. `table[L]` = cumulative CXP to reach level L.
    commander_levels: Arc<Vec<i64>>,
    /// Per-level reward config, loaded ONCE at boot from `data/commander_rewards.json` (the SAME file the
    /// client renders from). Static, so it is never re-read per request nor sent over the wire — the
    /// authoritative source for the Crystal a claim credits. See `commander::parse_rewards`.
    commander_rewards: Arc<Vec<commander::RewardRow>>,
}

/// Create the durable tables if absent.
async fn migrate(db: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    for stmt in [
        "CREATE TABLE IF NOT EXISTS accounts (account_id BIGSERIAL PRIMARY KEY, name TEXT UNIQUE NOT NULL, seed BIGINT NOT NULL)",
        // WAYS TO SIGN IN, many to one account. `accounts.name` was the login key itself, which
        // made an account and an identity the same object — and that is wrong for a game whose
        // accounts hold money, in three ways that have nothing to do with adding a platform:
        // a player who loses their Steam account cannot be given theirs back, a ban lands on an
        // identity rather than on a person, and NO second way to sign in can ever be added
        // (a recovery address, a companion login) without this same change.
        //
        // `accounts.name` stays as the identity the account was CREATED with — support and logs
        // read it, and dropping it would mean a nullable column for nothing.
        "CREATE TABLE IF NOT EXISTS account_identities (identity TEXT PRIMARY KEY, account_id BIGINT NOT NULL)",
        "CREATE INDEX IF NOT EXISTS account_identities_account ON account_identities (account_id)",
        // BACKFILL, and it runs every boot on purpose: it is `ON CONFLICT DO NOTHING`, so it is a
        // no-op once done, and it means an account created by an older build is never left
        // unable to log in. Self-healing beats a one-shot migration nobody re-runs.
        "INSERT INTO account_identities (identity, account_id) SELECT name, account_id FROM accounts ON CONFLICT DO NOTHING",
        "CREATE TABLE IF NOT EXISTS saves (account_id BIGINT PRIMARY KEY, state_json TEXT NOT NULL, final_tick BIGINT NOT NULL, final_hash TEXT NOT NULL)",
        // When this save was accepted, server clock. Feeds the boot anti-fast-forward anchor:
        // the budget after a restart is the AGE of the account's own last save, which is the
        // same quantity as the backlog it is about to present. DEFAULT 0 backfills existing
        // rows to "ancient", so they take the RESTART_GRACE_MS clamp on the first boot and a
        // real timestamp from their next save.
        "ALTER TABLE saves ADD COLUMN IF NOT EXISTS saved_at_ms BIGINT NOT NULL DEFAULT 0",
        "CREATE TABLE IF NOT EXISTS trusted (account_id BIGINT NOT NULL, state_hash TEXT NOT NULL, PRIMARY KEY (account_id, state_hash))",
        // LAYER 2 anti-cheat (docs/competition-integrity.md): cumulative excess ticks per
        // account. Durable BECAUSE that is the whole point — the in-memory session anchor
        // resets on every handshake, which is the hole this closes.
        "CREATE TABLE IF NOT EXISTS integrity (account_id BIGINT PRIMARY KEY, lifetime_excess BIGINT NOT NULL DEFAULT 0)",
        // MAILBOX as real rows (docs/gacha-arch-review.md P0). It used to live inside the
        // market_kv JSON snapshot, which made every gacha mint take the process-wide market lock
        // and inherit the market's poisoned state — on the one path that takes real money.
        // BIGSERIAL keeps the model's monotonic-id contract: an id is never reused.
        "CREATE TABLE IF NOT EXISTS mailbox (id BIGSERIAL PRIMARY KEY, account_id BIGINT NOT NULL, kind TEXT NOT NULL, gil BIGINT NOT NULL DEFAULT 0, def_id TEXT NOT NULL DEFAULT \'\', qty BIGINT NOT NULL DEFAULT 0, item_json TEXT, reason TEXT NOT NULL DEFAULT \'\', claimed_at BIGINT)",
        "CREATE INDEX IF NOT EXISTS mailbox_unclaimed ON mailbox (account_id) WHERE claimed_at IS NULL",
        // Withdrawn by a refund (audit 2026-10-03). Deleting it flagged a player whose client had
        // already taken the mail into its sim: the save's claim found no row and read as forged.
        "ALTER TABLE mailbox ADD COLUMN IF NOT EXISTS voided_at BIGINT",
        // Market op-ids (mail claims, plus any retired trade op an old state still carried) this
        // account has already settled — the idempotency set for a growing-log re-save. One INSERT
        // per NEW op; the read is a lookup of just the ids a save presents.
        //
        // Still append-only, DELIBERATELY. Since 0.0.253 the client prunes its own `market_ops`
        // once told what settled, so it seems safe to delete the matching rows here — it is not.
        // `trusted` is never pruned either ("Pruning on anchor-advance is a Postgres-slice TODO"
        // above), so a client can legitimately submit a bundle starting from an OLD anchor that
        // still contains those ops. Drop the rows and an honest re-save of an already-paid claim
        // reads `AlreadyClaimed` — rejected and flagged as a forgery. Trusted-anchor pruning has
        // to land FIRST; the rows are two bigints and Postgres does not care about millions.
        "CREATE TABLE IF NOT EXISTS consumed_market_ops (account_id BIGINT NOT NULL, op_id BIGINT NOT NULL, PRIMARY KEY (account_id, op_id))",
        // The op's digest at settlement (audit 2026-10-02): an op id is a client-side counter, so a
        // rewind to an older trusted anchor can reuse a settled id for a forged claim. NULL = a row
        // settled before this column (skipped as before).
        "ALTER TABLE consumed_market_ops ADD COLUMN IF NOT EXISTS digest TEXT",
        // Chat was removed from the game (user 2026-10-03); the persona names it showed went with it.
        "DROP TABLE IF EXISTS personas",
        // OFFLINE EXP is paid at the LOGIN (GET /save), in one transaction: the time since
        // max(latest save, `through_ms`), then `through_ms` = now. The earlier design measured at
        // the login and paid at the next save, and every cross-request hand-off it needed raced
        // (2026-10-03). The two tables it used are gone.
        "CREATE TABLE IF NOT EXISTS offline_paid (account_id BIGINT PRIMARY KEY, through_ms BIGINT NOT NULL)",
        // MINT COUPONS (`mint_coupon.rs`): the accrual clock per account, started at its login.
        "CREATE TABLE IF NOT EXISTS mint_coupon_clock (account_id BIGINT PRIMARY KEY, anchor_ms BIGINT NOT NULL)",
        // STEAM IMPORT (`steam_import.rs`): one row per Steam item id ever taken in. `id` is the
        // ConsumeItem requestid, so a re-sent consume is answered with the first one's result.
        "CREATE TABLE IF NOT EXISTS steam_imports (id BIGSERIAL PRIMARY KEY, steam_itemid TEXT NOT NULL UNIQUE, account_id BIGINT NOT NULL, def_id TEXT NOT NULL, qty INT NOT NULL, status TEXT NOT NULL DEFAULT 'consuming', attempts INT NOT NULL DEFAULT 0, note TEXT, created_at TIMESTAMPTZ NOT NULL DEFAULT now())",
        "DROP TABLE IF EXISTS offline_handshake",
        "DROP TABLE IF EXISTS offline_owed",
        // Server-roll FUSION: issued-but-unconsumed rolls (survive scale-to-zero so a restart
        // cannot false-flag a legitimate fusion as forged) + the consumed set (idempotent
        // re-saves). The retired `refine_outstanding` / `refine_consumed` tables were the same
        // shape for the same reason and are left in place, undropped.
        "CREATE TABLE IF NOT EXISTS fuse_outstanding (account_id BIGINT NOT NULL, nonce BIGINT NOT NULL, key TEXT NOT NULL, attempt BIGINT NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT now(), PRIMARY KEY (account_id, nonce))",
        "CREATE TABLE IF NOT EXISTS fuse_consumed (account_id BIGINT NOT NULL, nonce BIGINT NOT NULL, PRIMARY KEY (account_id, nonce))",
        // The refine tables (refine_outstanding / refine_consumed) went with the refine system
        // in 0.0.370. They are NOT dropped here: a DROP in the boot path would delete data on
        // every deploy of an old binary, and an unread table costs nothing. Drop them by hand
        // once the last pre-0.0.370 server is gone.
        // GEM -> CRYSTAL (0.0.371): the premium currency is named CRYSTAL now, so the word "gem"
        // is free for the decoration item. RENAME the live tables rather than letting
        // `CREATE TABLE IF NOT EXISTS crystal_wallet` make an EMPTY one beside the old data —
        // that would read as "every player's balance is zero". `IF EXISTS` + the fact that a
        // renamed table no longer matches makes this idempotent: it fires once, on the first
        // boot of this build, and is a no-op forever after.
        // Guarded (audit 2026-10-02): an old binary booted after the rename would CREATE an empty
        // gem_wallet beside crystal_wallet, and a bare RENAME then failed every later boot.
        "DO $$ BEGIN IF to_regclass('gem_wallet') IS NOT NULL AND to_regclass('crystal_wallet') IS NULL THEN ALTER TABLE gem_wallet RENAME TO crystal_wallet; END IF; END $$",
        "DO $$ BEGIN IF to_regclass('gem_ledger') IS NOT NULL AND to_regclass('crystal_ledger') IS NULL THEN ALTER TABLE gem_ledger RENAME TO crystal_ledger; END IF; END $$",
        // GACHA (docs/gacha-economy-design.md). Real per-row tables, NOT the market_kv JSON
        // snapshot: this is real money and must not sit behind a blob rewrite.
        // Two buckets, never summed and never converted into each other (invariant I11):
        // `paid` is directly purchased, `bound` is faucet / pass / rebate.
        "CREATE TABLE IF NOT EXISTS crystal_wallet (account_id BIGINT PRIMARY KEY, paid BIGINT NOT NULL DEFAULT 0, bound BIGINT NOT NULL DEFAULT 0)",
        // Append-only audit of every crystal movement — what I1 (conservation) and I7 (sinks
        // accounted) are checked against. Deltas are signed; a spend is negative.
        "CREATE TABLE IF NOT EXISTS crystal_ledger (id BIGSERIAL PRIMARY KEY, account_id BIGINT NOT NULL, d_paid BIGINT NOT NULL, d_bound BIGINT NOT NULL, reason TEXT NOT NULL, ref TEXT NOT NULL DEFAULT '', at TIMESTAMPTZ NOT NULL DEFAULT now())",
        // `tradeable` → `paid` (2026-10-01): crystal never reach a market; the bucket means PAID.
        "DO $$ BEGIN IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = current_schema() AND table_name = 'crystal_wallet' AND column_name = 'tradeable') THEN ALTER TABLE crystal_wallet RENAME COLUMN tradeable TO paid; END IF; END $$",
        "DO $$ BEGIN IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = current_schema() AND table_name = 'crystal_ledger' AND column_name = 'd_tradeable') THEN ALTER TABLE crystal_ledger RENAME COLUMN d_tradeable TO d_paid; END IF; END $$",
        // Pity per BANNER TYPE (not per banner id) so a rotation never resets progress.
        // `lost_5050` is character-only; the weapon banner has no 50/50.
        "CREATE TABLE IF NOT EXISTS gacha_pity (account_id BIGINT NOT NULL, banner_type TEXT NOT NULL, count INT NOT NULL DEFAULT 0, pulls_since_sr INT NOT NULL DEFAULT 0, lost_5050 BOOL NOT NULL DEFAULT FALSE, PRIMARY KEY (account_id, banner_type))",
        // Idempotency + audit: the client's key is the PK, so a retry (or a scale-to-zero
        // mid-flight) replays the SAME results and charges once.
        "CREATE TABLE IF NOT EXISTS gacha_rolls (idem_key TEXT PRIMARY KEY, account_id BIGINT NOT NULL, banner_id TEXT NOT NULL, results_json TEXT NOT NULL, at TIMESTAMPTZ NOT NULL DEFAULT now())",
        // THE KEY IS PER ACCOUNT. It was `idem_key TEXT PRIMARY KEY` — unique across every account
        // — while the replay lookup has always been `WHERE idem_key = $1 AND account_id = $2`. The
        // two scopes disagreed, and the client mints keys from `Time.get_ticks_msec()` with no
        // account component, so two players whose sessions are the same age produce the same
        // string. The second one's account-scoped SELECT finds nothing, falls through to
        // `ON CONFLICT DO NOTHING`, sees 0 rows and returns "roll already in flight" — for a pull
        // that was never in flight. Reproduced on a first-ever pull. Nothing prunes `gacha_rolls`,
        // so that key is refused for that account forever and the refusal rate only climbs.
        //
        // Widening the LOOKUP instead would be far worse: the audit tried it and the second player
        // received the first one's results, SSR included, charged nothing and delivered nothing.
        //
        // `CREATE TABLE IF NOT EXISTS` above is a no-op on a database that already has the table —
        // that is how a DDL edit silently misses every existing deployment — so the change has to
        // be an explicit ALTER. Idempotent via the constraint name, and safe on existing rows
        // because widening a key can never conflict.
        "DO $$ BEGIN \
           IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'gacha_rolls_acct_idem') THEN \
             ALTER TABLE gacha_rolls DROP CONSTRAINT IF EXISTS gacha_rolls_pkey; \
             ALTER TABLE gacha_rolls ADD CONSTRAINT gacha_rolls_acct_idem PRIMARY KEY (account_id, idem_key); \
           END IF; \
         END $$;",
        // The server's half of the roll seed (C04b). Added as a nullable-with-default column so an
        // existing deployment migrates in place; rows written before this landed keep 0, which is
        // exactly what they were rolled with, so their stored results stay reproducible.
        "ALTER TABLE gacha_rolls ADD COLUMN IF NOT EXISTS server_nonce BIGINT NOT NULL DEFAULT 0",
        // Daily GEM milestones already paid today, one bit per rung. Its own column rather than
        // bits borrowed from done_mask: two pay-once ledgers sharing one integer is how one of
        // them eventually pays twice.
        // deal_json held the day's board, 682 bytes of it, once per player per DAY. That was
        // load-bearing while the board was a per-account draw; since it became the same fixed
        // eight for everyone it is `season.config` written out 36 million times a year at 100k
        // DAU. Derived on read now. (Postgres DROP COLUMN is metadata-only; the bytes come back
        // on the next vacuum.)
        // DAILY COMMISSION — its own table, no season column. The board is a constant, so a
        // row holds only what is actually per-player-per-day: which commissions are done, which
        // ladder rungs have paid, and the counters the day opened on.
        "CREATE TABLE IF NOT EXISTS daily_progress (account_id BIGINT NOT NULL, day BIGINT NOT NULL, done_mask SMALLINT NOT NULL DEFAULT 0, milestone_mask SMALLINT NOT NULL DEFAULT 0, start_values_json TEXT NOT NULL DEFAULT '[]', PRIMARY KEY (account_id, day))",
        // Renaming a column needs an explicit ALTER: CREATE TABLE IF NOT EXISTS is a no-op once
        // the table exists, so an edited CREATE reaches only fresh databases. The pg test caught
        // this — the same shape as the bp_daily miss two commits ago.
        "ALTER TABLE daily_progress ADD COLUMN IF NOT EXISTS start_values_json TEXT NOT NULL DEFAULT '[]'",
        "ALTER TABLE daily_progress DROP COLUMN IF EXISTS day_start_json",
        // Rungs PAID, separate from `milestone_mask` (rungs reached). One column meant accrual
        // marked a rung paid on the way past it, so `/daily/claim` would have handed over nothing.
        "ALTER TABLE daily_progress ADD COLUMN IF NOT EXISTS claimed_mask SMALLINT NOT NULL DEFAULT 0",
        // WEEKLY COMMISSION (docs/weekly): the daily board ×5 on a seven-day key. Same shape as
        // daily_progress — a `(account, week)` row with two pay-once masks and the opening snapshot.
        "CREATE TABLE IF NOT EXISTS weekly_progress (account_id BIGINT NOT NULL, week BIGINT NOT NULL, done_mask SMALLINT NOT NULL DEFAULT 0, milestone_mask SMALLINT NOT NULL DEFAULT 0, claimed_mask SMALLINT NOT NULL DEFAULT 0, start_values_json TEXT NOT NULL DEFAULT '[]', PRIMARY KEY (account_id, week))",
        // bp_daily is superseded. Dropped rather than migrated: it holds at most one day of
        // in-flight progress and the board it recorded no longer exists.
        "DROP TABLE IF EXISTS bp_daily",
        // SCHEDULE + CLAIM — one row per (account, track). No `day` column: unlike a commission
        // day, a login track is ONE run spanning many days, and `position` is how far along it the
        // account is. A track that repeats is a new track id, never a second key.
        //
        // No index on `track`: the hot read is "every track of one account", which the PK already
        // serves. The one query that wants `track` leading is the retirement sweep, and
        // `schedule::retired_tracks` answers from config first — so on almost every run it issues
        // no query at all rather than scanning a large table to find nothing.
        "CREATE TABLE IF NOT EXISTS claim_progress (account_id BIGINT NOT NULL, track TEXT NOT NULL, position INT NOT NULL DEFAULT 0, claimed_mask BIGINT NOT NULL DEFAULT 0, last_day BIGINT NOT NULL DEFAULT -1, PRIMARY KEY (account_id, track))",
        // The day THIS ACCOUNT started the track. A `SinceStart` window ("90 days from creating
        // the id") expires at a different moment for every account, so the clock cannot live in
        // config — and this is also what the per-account row sweep deletes by. A separate ALTER
        // because CREATE TABLE IF NOT EXISTS is a no-op once the table exists, so an edited CREATE
        // reaches only fresh databases.
        // DEFAULT 0, not -1. A `SinceStart` window is `day < opened_day + days`, so a row
        // inheriting -1 reads as having opened before the epoch — permanently and silently closed.
        // The same -1 next door in `last_day` is a SAFE sentinel ("never advanced"), which is what
        // made copying it look right. 0 is equally wrong as a date but fails OPEN, and no live row
        // takes either since the only INSERT names the column.
        "ALTER TABLE claim_progress ADD COLUMN IF NOT EXISTS opened_day BIGINT NOT NULL DEFAULT 0",
        // BOUND ITEMS — countable, spend-once redeemables that must not take a bag slot and must
        // not be lost to a full bag: gacha coupons today, EXP tickets later. ROWS, not columns, so
        // a new kind is data rather than a migration. Server-side per
        // docs/bound-items-design.md: a coupon takes no part in the simulation while it is held.
        "CREATE TABLE IF NOT EXISTS bound_items (account_id BIGINT NOT NULL, kind TEXT NOT NULL, qty BIGINT NOT NULL DEFAULT 0, PRIMARY KEY (account_id, kind))",
        // PARTY-SLOT TICKETS (sim 0.0.413): every party slot bought with Gil pays ten gacha
        // coupons, credited from the VERIFIED save's `measures.party_slots_bought`. `paid` is the
        // watermark — the count already rewarded — so a replayed or re-uploaded save pays nothing
        // twice and a save that skips ahead pays every slot it skipped.
        "CREATE TABLE IF NOT EXISTS slot_tickets (account_id BIGINT PRIMARY KEY, paid BIGINT NOT NULL DEFAULT 0)",
        // SEALED CACHE (sim 0.0.449): per-account watermark of opens paid, and the server-wide
        // monthly quota per coupon tier — `won` counts up to `lootbox::quota_of`, and doubles as
        // the serial in the announcement.
        "CREATE TABLE IF NOT EXISTS lootbox_opens (account_id BIGINT PRIMARY KEY, paid BIGINT NOT NULL DEFAULT 0)",
        // FARM BANNER (farm.rs): online-time accrual per event, and the opens watermark + pity.
        "CREATE TABLE IF NOT EXISTS farm_progress (account_id BIGINT NOT NULL, event_id TEXT NOT NULL,          last_tick BIGINT NOT NULL DEFAULT 0, banked BIGINT NOT NULL DEFAULT 0, rolls BIGINT NOT NULL DEFAULT 0,          day BIGINT NOT NULL DEFAULT 0, boxes_today INT NOT NULL DEFAULT 0, boxes_total INT NOT NULL DEFAULT 0,          PRIMARY KEY (account_id, event_id))",
        "CREATE TABLE IF NOT EXISTS farm_opens (account_id BIGINT PRIMARY KEY, paid BIGINT NOT NULL DEFAULT 0,          since INT NOT NULL DEFAULT 0, featured_won INT NOT NULL DEFAULT 0)",
        // 0.0.479 tickets: `farm_opens` above is the 0.0.477 pay-on-open watermark, read once to
        // seed these two for `farm_box` and never written again.
        "CREATE TABLE IF NOT EXISTS farm_box_opens (account_id BIGINT NOT NULL, box_def TEXT NOT NULL,          paid BIGINT NOT NULL DEFAULT 0, PRIMARY KEY (account_id, box_def))",
        "CREATE TABLE IF NOT EXISTS farm_pity (account_id BIGINT NOT NULL, box_def TEXT NOT NULL,          pulls BIGINT NOT NULL DEFAULT 0, since INT NOT NULL DEFAULT 0, won INT NOT NULL DEFAULT 0,          PRIMARY KEY (account_id, box_def))",
        // STEAM INVENTORY (steam_inv.rs): one row per item sent to Steam — pending → inflight →
        // done / pending (Steam refused) / uncertain (outcome unknown; a human reconciles).
        "CREATE TABLE IF NOT EXISTS steam_mints (account_id BIGINT NOT NULL, def_id TEXT NOT NULL, seq BIGINT NOT NULL, itemdefid BIGINT NOT NULL, status TEXT NOT NULL DEFAULT 'pending', steam_itemid TEXT, note TEXT, attempts INT NOT NULL DEFAULT 0, PRIMARY KEY (account_id, def_id, seq))",
        // NEWCOMER BANNER (gacha 3-banner shape): pulls made and whether the first SSR closed it.
        "CREATE TABLE IF NOT EXISTS gacha_newcomer (account_id BIGINT PRIMARY KEY, made INT NOT NULL DEFAULT 0, closed BOOL NOT NULL DEFAULT FALSE)",
        "CREATE TABLE IF NOT EXISTS lootbox_quota (month TEXT NOT NULL, coupons INT NOT NULL, won INT NOT NULL DEFAULT 0, PRIMARY KEY (month, coupons))",
        // Its AUDIT TRAIL, and `docs/bound-items-design.md` asks for it by name: a coupon bought
        // with real money is something the operator has to be able to answer questions about.
        // `crystal_ledger`'s twin, and for the same reason — a counter that can be edited and drift
        // silently is what `docs/economy-invariants.md` I7 exists to stop.
        "CREATE TABLE IF NOT EXISTS bound_ledger (id BIGSERIAL PRIMARY KEY, account_id BIGINT NOT NULL, kind TEXT NOT NULL, delta BIGINT NOT NULL, reason TEXT NOT NULL, ref TEXT NOT NULL DEFAULT '', at TIMESTAMPTZ NOT NULL DEFAULT now())",
        "CREATE INDEX IF NOT EXISTS bound_ledger_acct ON bound_ledger (account_id, kind)",
        // History reads this account newest-first; the PK is on idem_key, which does not help.
        "CREATE INDEX IF NOT EXISTS gacha_rolls_acct_at ON gacha_rolls (account_id, at DESC)",
        // Recorded per pull because they CANNOT be reconstructed later — the counters advance and
        // the old value is gone. -1 on rows written before these columns existed, which is a
        // different fact from 0 and has to stay distinguishable.
        "ALTER TABLE gacha_rolls ADD COLUMN IF NOT EXISTS pity_count INT NOT NULL DEFAULT -1",
        "ALTER TABLE gacha_rolls ADD COLUMN IF NOT EXISTS pulls_since_sr INT NOT NULL DEFAULT -1",
        // MILEAGE — one shared, never-expiring pool per ACCOUNT (not per banner type: the whole
        // point is that it crosses rotations). Minted 1 per pull; `gacha::MILEAGE_COST` buys any
        // EXCHANGE_POOL unit. Its own table, never a column on crystal_wallet: mileage is not a crystal,
        // cannot be purchased, and must never be summed with or converted into one (I11).
        "CREATE TABLE IF NOT EXISTS gacha_mileage (account_id BIGINT PRIMARY KEY, tickets BIGINT NOT NULL DEFAULT 0)",
        // STANDARD MILEAGE (2026-09-30): Starter/Standard pulls mint here and buy Memory Fragments
        // only; `gacha_mileage` stays the Limited currency (and keeps every pre-split balance).
        "CREATE TABLE IF NOT EXISTS gacha_mileage_std (account_id BIGINT PRIMARY KEY, tickets BIGINT NOT NULL DEFAULT 0)",
        // COMMANDER PROGRESS CXP: the highest stage already paid (`commander::CXP_PER_NEW_STAGE`).
        "CREATE TABLE IF NOT EXISTS commander_stage (account_id BIGINT PRIMARY KEY, reached INT NOT NULL DEFAULT 0)",
        // TUTORIAL QUESTS: one bit per quest claimed (`tutorial::Quest::bit`).
        "CREATE TABLE IF NOT EXISTS tutorial_progress (account_id BIGINT PRIMARY KEY, claimed BIGINT NOT NULL DEFAULT 0)",
        // Crystal-shop purchases: the idempotency key AND the daily-cap count (sum of qty per day).
        "CREATE TABLE IF NOT EXISTS shop_buys (account_id BIGINT NOT NULL, idem_key TEXT NOT NULL, def_id TEXT NOT NULL, qty INT NOT NULL, day INT NOT NULL, crystal BIGINT NOT NULL, at TIMESTAMPTZ NOT NULL DEFAULT now(), PRIMARY KEY (account_id, idem_key))",
        "CREATE INDEX IF NOT EXISTS shop_buys_day ON shop_buys (account_id, def_id, day)",
        // TWO TICKETS (2026-09-30): the universal `gacha_coupon` is gone — balance and ledger rows
        // deleted outright (user: "ลบ gacha_coupon เดิมทิ้งไปเลย", pre-launch). Idempotent: nothing
        // writes that kind any more, so after the first boot these match no rows.
        "DELETE FROM bound_items WHERE kind = 'gacha_coupon'",
        "DELETE FROM bound_ledger WHERE kind = 'gacha_coupon'",
        // STANDARD SELECTOR: Standard pulls made, and the one SSR chosen once `made` reaches
        // `gacha::STANDARD_SELECT_AT` (NULL until then).
        "CREATE TABLE IF NOT EXISTS gacha_standard (account_id BIGINT PRIMARY KEY, made INT NOT NULL DEFAULT 0, selected TEXT)",
        // Idempotency + audit for the exchange half, mirroring gacha_rolls.
        "CREATE TABLE IF NOT EXISTS gacha_exchanges (idem_key TEXT PRIMARY KEY, account_id BIGINT NOT NULL, def_id TEXT NOT NULL, spent BIGINT NOT NULL, at TIMESTAMPTZ NOT NULL DEFAULT now())",
        // Same defect, thinner key: the client's exchange key is `\"x-%d-%s\"` with no count
        // component at all. See the note on `gacha_rolls`.
        "DO $$ BEGIN \
           IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'gacha_exchanges_acct_idem') THEN \
             ALTER TABLE gacha_exchanges DROP CONSTRAINT IF EXISTS gacha_exchanges_pkey; \
             ALTER TABLE gacha_exchanges ADD CONSTRAINT gacha_exchanges_acct_idem PRIMARY KEY (account_id, idem_key); \
           END IF; \
         END $$;",
        // The single ACTIVE session token per account, persisted so a redeploy/restart
        // doesn't wipe it and 401 every connected client (which would show them the
        // "logged in elsewhere" kick modal). Reloaded at startup; a new login overwrites it.
        "CREATE TABLE IF NOT EXISTS active_sessions (account_id BIGINT PRIMARY KEY, token TEXT NOT NULL)",
        // Append-only CHEAT-SUSPICION audit: one row per save rejected for an integrity
        // reason (forged nonce, fast-forward, tampered/unanchored state). For HUMAN review
        // + MANUAL ban — never auto-ban (a flag spike across many accounts = OUR bug, not a
        // cheat wave; auto-banning legit players on our own divergence is the TBH mistake).
        "CREATE TABLE IF NOT EXISTS cheat_flags (id BIGSERIAL PRIMARY KEY, account_id BIGINT NOT NULL, reason TEXT NOT NULL, final_tick BIGINT NOT NULL, at_ms BIGINT NOT NULL)",
        "CREATE INDEX IF NOT EXISTS cheat_flags_recent ON cheat_flags (account_id, reason, at_ms)",
        // WHICH CLIENT raised it (2026-09-29): sim + catalog version and the platform the client
        // reports. A divergence flag that clusters on one build or one platform is OUR bug.
        "ALTER TABLE cheat_flags ADD COLUMN IF NOT EXISTS detail TEXT NOT NULL DEFAULT ''",
        // BANS (2026-09-29): a human decision recorded after reviewing `cheat_flags`. NULL
        // `until_ms` = permanent. Checked at every login; issuing one ends the live session.
        "CREATE TABLE IF NOT EXISTS account_bans (account_id BIGINT PRIMARY KEY, until_ms BIGINT, reason TEXT NOT NULL, at_ms BIGINT NOT NULL)",
        // THE IN-GAME GIL MARKET IS GONE (cancelled 2026-09-16, removed 0.0.490). Its ledger
        // tables are dropped; production held no open orders or listings when this shipped, and
        // the only op ever settled there were mail claims, which live on in `mailbox` +
        // `consumed_market_ops`. IF EXISTS keeps this idempotent on every boot and on a fresh DB.
        "DROP TABLE IF EXISTS market_kv",
        "DROP TABLE IF EXISTS market_accounts",
        "DROP TABLE IF EXISTS market_holdings",
        "DROP TABLE IF EXISTS sell_orders",
        "DROP TABLE IF EXISTS item_listings",
        "DROP TABLE IF EXISTS market_meta",
        "DROP TABLE IF EXISTS trade_ledger",
        // BATTLE PASS (P1, docs/battle-pass-architecture.md). bp_state carries the accrual
        // anchor (last tick + counter snapshot) — accrual diffs verified saves against it,
        // so a crash between the save commit and the BP commit self-heals next save.
        "CREATE TABLE IF NOT EXISTS seasons (season_id BIGSERIAL PRIMARY KEY, starts_at_ms BIGINT NOT NULL, ends_at_ms BIGINT NOT NULL, seed BIGINT NOT NULL, config_json TEXT NOT NULL)",
        "CREATE TABLE IF NOT EXISTS bp_state (account_id BIGINT NOT NULL, season_id BIGINT NOT NULL, xp BIGINT NOT NULL DEFAULT 0, last_tick BIGINT NOT NULL DEFAULT 0, snapshot_json TEXT NOT NULL DEFAULT '{}', PRIMARY KEY (account_id, season_id))",
        // bp_claims: idempotency + PERMANENT history (rows never deleted; refund clawback
        // marks voided_at). Claims land in P2.
        "CREATE TABLE IF NOT EXISTS story_claims (account_id BIGINT NOT NULL, mission_id TEXT NOT NULL, claimed_at TIMESTAMPTZ NOT NULL DEFAULT now(), PRIMARY KEY (account_id, mission_id))",
        "CREATE TABLE IF NOT EXISTS bp_claims (account_id BIGINT NOT NULL, season_id BIGINT NOT NULL, track SMALLINT NOT NULL, slot SMALLINT NOT NULL, claimed_at TIMESTAMPTZ NOT NULL DEFAULT now(), voided_at TIMESTAMPTZ, void_reason TEXT, PRIMARY KEY (account_id, season_id, track, slot))",
        "CREATE TABLE IF NOT EXISTS bp_entitlements (entitle_id BIGSERIAL PRIMARY KEY, account_id BIGINT NOT NULL, season_id BIGINT NOT NULL, tier TEXT NOT NULL, source TEXT NOT NULL, txn_id TEXT, active BOOL NOT NULL DEFAULT TRUE, revoked_at TIMESTAMPTZ)",
        // STORE / money-in (docs/unbuilt-systems-architecture.md §5). `mtx_txns` predates this as
        // DDL nobody referenced; the live tables are below. An ORDER is keyed by the id WE mint,
        // because it has to exist before Steam has told us anything — the Steam `transid` arrives
        // only after InitTxn succeeds.
        "CREATE TABLE IF NOT EXISTS mtx_txns (txn_id TEXT PRIMARY KEY, account_id BIGINT NOT NULL, sku TEXT NOT NULL, status TEXT NOT NULL, order_id TEXT)",
        "CREATE TABLE IF NOT EXISTS store_orders (order_id BIGINT PRIMARY KEY, account_id BIGINT NOT NULL, sku TEXT NOT NULL, idem_key TEXT NOT NULL UNIQUE, price_cents BIGINT NOT NULL, transid TEXT, status TEXT NOT NULL, first_buy BOOLEAN NOT NULL DEFAULT false, granted_paid BIGINT NOT NULL DEFAULT 0, granted_bound BIGINT NOT NULL DEFAULT 0, created_at TIMESTAMPTZ NOT NULL DEFAULT now())",
        "DO $$ BEGIN IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = current_schema() AND table_name = 'store_orders' AND column_name = 'granted_tradeable') THEN ALTER TABLE store_orders RENAME COLUMN granted_tradeable TO granted_paid; END IF; END $$",
        // SCOPE THE UNIQUENESS TO THE ACCOUNT, matching the lookup. `idem_key` was globally UNIQUE
        // while `store_checkout` replays it per account — the same constraint-vs-lookup mismatch
        // that `638cbbe` fixed for `gacha_rolls`, left standing on the table that takes real money.
        // A client key that collides across accounts locks the second player out of checkout
        // permanently, and no error path distinguishes that from a duplicate of their own.
        //
        // Dropping the old constraint by its generated name, then adding the composite. Both are
        // `IF EXISTS`/`IF NOT EXISTS`-shaped so a fresh database and an existing one converge —
        // `migrate` runs on every boot and a statement that can fail once is a server that will
        // not start.
        "ALTER TABLE store_orders DROP CONSTRAINT IF EXISTS store_orders_idem_key_key",
        "CREATE UNIQUE INDEX IF NOT EXISTS store_orders_acct_idem ON store_orders (account_id, idem_key)",
        "CREATE INDEX IF NOT EXISTS store_orders_acct ON store_orders (account_id, sku)",
        // An entitlement is the durable "this account owns this SKU" fact. Kept SEPARATE from the
        // order so a refund can reverse the value without erasing the history that it happened —
        // refunds are compensating entries, never deletions.
        // TELEMETRY. Two shapes because two questions: additive counters answer "is anything
        // dead / how much happened", and one row per player per day answers "who is here, how
        // far are they, did they come back" without a save every 10 s becoming a row every 10 s.
        "CREATE TABLE IF NOT EXISTS metrics_daily (day INT NOT NULL, key TEXT NOT NULL, value BIGINT NOT NULL, PRIMARY KEY (day, key))",
        "CREATE TABLE IF NOT EXISTS player_daily (day INT NOT NULL, account_id BIGINT NOT NULL, level INT NOT NULL, tick BIGINT NOT NULL, gil BIGINT NOT NULL, pending_drops INT NOT NULL, zone TEXT NOT NULL DEFAULT '', seen_at TIMESTAMPTZ NOT NULL DEFAULT now(), PRIMARY KEY (day, account_id))",
        "ALTER TABLE player_daily ADD COLUMN IF NOT EXISTS zone TEXT NOT NULL DEFAULT ''",
        // FIRST ARRIVAL per zone. The (account, zone) key IS the "first" rule — no diff against
        // a previous state, nothing to keep warm across a restart.
        "CREATE TABLE IF NOT EXISTS first_clear (account_id BIGINT NOT NULL, zone TEXT NOT NULL, level INT NOT NULL, tick BIGINT NOT NULL, party TEXT NOT NULL, at TIMESTAMPTZ NOT NULL DEFAULT now(), PRIMARY KEY (account_id, zone))",
        "CREATE TABLE IF NOT EXISTS entitlements (id BIGSERIAL PRIMARY KEY, account_id BIGINT NOT NULL, sku TEXT NOT NULL, order_id BIGINT NOT NULL, granted_at TIMESTAMPTZ NOT NULL DEFAULT now(), revoked_at TIMESTAMPTZ)",
        "CREATE INDEX IF NOT EXISTS entitlements_acct_sku ON entitlements (account_id, sku)",
        // PACKS (2026-09-29). A Battle Pass order is bound to the season running at checkout, so a
        // purchase that finalizes after the rollover still unlocks the season that was paid for.
        "ALTER TABLE store_orders ADD COLUMN IF NOT EXISTS season_id BIGINT",
        // Sweep bookkeeping (audit 2026-10-02): failed grant attempts, so a permanently failing
        // order can leave the queue (`grant_failed`) instead of blocking it.
        "ALTER TABLE store_orders ADD COLUMN IF NOT EXISTS grant_attempts INT NOT NULL DEFAULT 0",
        // The day window [from, to) a monthly card order added — so a refund with two cards banked
        // takes back exactly that card's claims and unused days (audit 2026-10-03).
        "ALTER TABLE store_orders ADD COLUMN IF NOT EXISTS monthly_from_day BIGINT",
        "ALTER TABLE store_orders ADD COLUMN IF NOT EXISTS monthly_to_day BIGINT",
        // When the order was DELIVERED: a refunded order uses up a capped SKU only if it was.
        // Backfilled from what older rows show: every grant wrote a 'purchase' ledger row, even a
        // pack or pass that carries no crystal.
        "ALTER TABLE store_orders ADD COLUMN IF NOT EXISTS granted_at TIMESTAMPTZ",
        "ALTER TABLE store_orders ADD COLUMN IF NOT EXISTS charging_at TIMESTAMPTZ",
        "ALTER TABLE store_orders ADD COLUMN IF NOT EXISTS refund_checked_at TIMESTAMPTZ",
        "UPDATE store_orders SET granted_at = created_at WHERE granted_at IS NULL \
           AND (status = 'granted' OR (status = 'refunded' AND (granted_paid + granted_bound > 0 \
                OR EXISTS (SELECT 1 FROM crystal_ledger l WHERE l.reason = 'purchase' \
                           AND l.ref = store_orders.order_id::text))))",
        // SKU RENAME gems_* → crystal_* (47a4b23b) carried no data migration (audit 2026-10-02):
        // an old `gems_300` entitlement read as "never bought crystal_300" (first-buy bonus twice)
        // and a charged `gems_*` order could never be granted. Idempotent: no row matches twice.
        r"UPDATE store_orders SET sku = 'crystal_' || substr(sku, 6) WHERE sku LIKE 'gems\_%' ESCAPE '\'",
        r"UPDATE entitlements SET sku = 'crystal_' || substr(sku, 6) WHERE sku LIKE 'gems\_%' ESCAPE '\'",
        r"UPDATE mtx_txns SET sku = 'crystal_' || substr(sku, 6) WHERE sku LIKE 'gems\_%' ESCAPE '\'",
        // MONTHLY CARD. `ends_day` is exclusive: the card pays on every day d < ends_day.
        // `last_claim_day` is the exactly-once guard for the daily drip.
        "CREATE TABLE IF NOT EXISTS monthly_card (account_id BIGINT PRIMARY KEY, ends_day BIGINT NOT NULL, last_claim_day BIGINT NOT NULL DEFAULT -1)",
        // COMMANDER LEVEL (docs/commander-level-and-outpost.md) — account-meta CXP, server-side only,
        // never in the sim save. `cxp` is the total; the level is derived from the boot-built curve
        // table. `reward_claimed_through` is the durable exactly-once high-water (levels whose reward
        // has been accepted).
        "CREATE TABLE IF NOT EXISTS commander (account_id BIGINT PRIMARY KEY, cxp BIGINT NOT NULL DEFAULT 0, reward_claimed_through INT NOT NULL DEFAULT 0)",
        // The Outpost is gone (user 2026-09-30: its items were all cut; daily rungs pay the CXP).
        "ALTER TABLE commander DROP COLUMN IF EXISTS outpost_claimed_at_ms",
        // The per-level reward is STATIC config in `data/commander_rewards.json` (user 2026-08-30), NOT a
        // DB table — loaded at boot into AppState.commander_rewards and never queried per request. See
        // commander.rs § the reward file.
    ] {
        sqlx::query(stmt).execute(db).await?;
    }
    // ONE-TIME DATA MIGRATIONS, flagged so they never run twice. `schema_flags` holds the name of
    // every one that has run; a boot after the flag is a no-op.
    sqlx::query("CREATE TABLE IF NOT EXISTS schema_flags (name TEXT PRIMARY KEY, at TIMESTAMPTZ NOT NULL DEFAULT now())")
        .execute(db)
        .await?;
    // GIL ×100 (sim 0.0.412): the sim lifts a save's balance on load, but Gil the SERVER holds
    // for the player — unclaimed mail payloads — is in the old unit until this runs. Same factor,
    // once, in one transaction with the flag. (It also scaled the market purse, whose table is
    // dropped above; every live database ran this long before that.)
    let done: Option<(String,)> = sqlx::query_as("SELECT name FROM schema_flags WHERE name = 'gil_x100'")
        .fetch_optional(db)
        .await?;
    if done.is_none() {
        let mut tx = db.begin().await?;
        sqlx::query("UPDATE mailbox SET gil = gil * 100 WHERE claimed_at IS NULL AND gil > 0").execute(&mut *tx).await?;
        sqlx::query("INSERT INTO schema_flags (name) VALUES ('gil_x100') ON CONFLICT (name) DO NOTHING").execute(&mut *tx).await?;
        tx.commit().await?;
    }
    // TRUSTED PRUNE (audit 2026-10-02): an account with a save needs only that save's hash as a
    // base now (later saves keep {latest, its base}); the old rows grew one per save forever and
    // are loaded whole at boot. Once; accounts active in the last hour prune on their next save.
    let done: Option<(String,)> = sqlx::query_as("SELECT name FROM schema_flags WHERE name = 'trusted_prune'")
        .fetch_optional(db)
        .await?;
    if done.is_none() {
        let mut tx = db.begin().await?;
        // Only accounts idle for an hour (audit 2026-10-03): the base of a save that landed just
        // before the restart is not recorded anywhere, and a client re-sending that bundle read
        // as "not anchored" and was flagged. A recent account's next save prunes its own rows.
        sqlx::query(
            "DELETE FROM trusted t USING saves s WHERE t.account_id = s.account_id AND t.state_hash <> s.final_hash \
               AND s.saved_at_ms < $1",
        ).bind(now_ms() - 3_600_000).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO schema_flags (name) VALUES ('trusted_prune') ON CONFLICT (name) DO NOTHING").execute(&mut *tx).await?;
        tx.commit().await?;
    }
    // OP DIGEST v2 (audit 2026-10-03): `op_digest` is now built from claim values, so digests
    // stored by the serialized-bytes version never match and would flag every re-sent op. Cleared
    // once, before serving; a NULL digest is not compared.
    let done: Option<(String,)> = sqlx::query_as("SELECT name FROM schema_flags WHERE name = 'op_digest_v2'")
        .fetch_optional(db)
        .await?;
    if done.is_none() {
        let mut tx = db.begin().await?;
        sqlx::query("UPDATE consumed_market_ops SET digest = NULL WHERE digest IS NOT NULL").execute(&mut *tx).await?;
        sqlx::query("INSERT INTO schema_flags (name) VALUES ('op_digest_v2') ON CONFLICT (name) DO NOTHING").execute(&mut *tx).await?;
        tx.commit().await?;
    }
    // COMMANDER STAGE WATERMARK SEED (audit 2026-10-02): CXP per new stage started at 0, so the
    // first save after the deploy that added it would have paid every stage an existing account
    // had already reached. Seed the watermark from each stored save's `band_reached` instead.
    let done: Option<(String,)> = sqlx::query_as("SELECT name FROM schema_flags WHERE name = 'commander_stage_seed'")
        .fetch_optional(db)
        .await?;
    if done.is_none() {
        let rows: Vec<(i64, String)> = sqlx::query_as("SELECT account_id, state_json FROM saves").fetch_all(db).await?;
        let mut tx = db.begin().await?;
        for (account_id, json) in rows {
            let reached = commander::seed_stage_of(&json);
            sqlx::query(
                "INSERT INTO commander_stage (account_id, reached) VALUES ($1, $2) \
                 ON CONFLICT (account_id) DO UPDATE SET reached = GREATEST(commander_stage.reached, EXCLUDED.reached)",
            ).bind(account_id).bind(reached).execute(&mut *tx).await?;
        }
        sqlx::query("INSERT INTO schema_flags (name) VALUES ('commander_stage_seed') ON CONFLICT (name) DO NOTHING").execute(&mut *tx).await?;
        tx.commit().await?;
    }
    Ok(())
}

/// Which of the op-ids in THIS save were already settled. Looking up only the presented ids
/// keeps the query O(ops in the bundle) rather than O(the account's whole trading history) —
/// which matters because the history never shrinks: `GameState.market_ops` is append-only and
/// never trimmed, so a client re-presents every op it has ever made on every save.
async fn already_consumed(db: &sqlx::PgPool, account_id: u64, ops: &[sim_core::state::MarketOp]) -> Result<HashSet<u64>, sqlx::Error> {
    let ids: Vec<i64> = ops.iter().map(|o| o.id() as i64).collect();
    if ids.is_empty() {
        return Ok(HashSet::new());
    }
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT op_id FROM consumed_market_ops WHERE account_id = $1 AND op_id = ANY($2)",
    ).bind(account_id as i64).bind(&ids).fetch_all(db).await?;
    Ok(rows.into_iter().map(|(o,)| o as u64).collect())
}

/// Mail ids (or op ids for a non-claim op) whose op id this account already settled with a
/// DIFFERENT op — the same id re-presented with other content. An honest growing log re-presents
/// identical bytes; only a rewind to an older anchor that reused the id produces this.
async fn reused_op_ids(db: &sqlx::PgPool, account_id: u64, ops: &[sim_core::state::MarketOp]) -> Result<Vec<u64>, sqlx::Error> {
    let ids: Vec<i64> = ops.iter().map(|o| o.id() as i64).collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT op_id, digest FROM consumed_market_ops WHERE account_id = $1 AND op_id = ANY($2) AND digest IS NOT NULL",
    ).bind(account_id as i64).bind(&ids).fetch_all(db).await?;
    let stored: HashMap<u64, String> = rows.into_iter().map(|(o, d)| (o as u64, d)).collect();
    Ok(ops.iter()
        .filter(|op| stored.get(&op.id()).is_some_and(|d| *d != mailbox::op_digest(op)))
        .map(|op| match op {
            sim_core::state::MarketOp::Claim { mail_id, .. } => *mail_id,
            other => other.id(),
        })
        .collect())
}

/// Op ids a bundle PRUNES that this account never had settled (audit 2026-10-02). The client may
/// only drop an op the server returned in `settled_ops`; pruning an unsettled claim erased the
/// only record that the mail was taken, so the same mail could be claimed again and again.
async fn unsettled_prunes(db: &sqlx::PgPool, account_id: u64, log: &[sim_core::events::RecordedInput]) -> Result<Vec<u64>, sqlx::Error> {
    let ids: Vec<i64> = log.iter()
        .filter_map(|r| match &r.event { InputEvent::PruneMarketOps { ids } => Some(ids.clone()), _ => None })
        .flatten()
        .map(|i| i as i64)
        .collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT op_id FROM consumed_market_ops WHERE account_id = $1 AND op_id = ANY($2)",
    ).bind(account_id as i64).bind(&ids).fetch_all(db).await?;
    let settled: HashSet<i64> = rows.into_iter().map(|(o,)| o).collect();
    Ok(ids.into_iter().filter(|i| !settled.contains(i)).map(|i| i as u64).collect())
}

/// Record the ops a verified save just settled. `ON CONFLICT DO NOTHING` because a concurrent
/// duplicate is a no-op, not an error — the set is idempotent by construction.
async fn mark_consumed(tx: &mut sqlx::PgConnection, account_id: u64, ops: &[u64], digests: &[String]) -> Result<(), sqlx::Error> {
    for (i, id) in ops.iter().enumerate() {
        let done = sqlx::query("INSERT INTO consumed_market_ops (account_id, op_id, digest) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
            .bind(account_id as i64).bind(*id as i64).bind(digests.get(i))
            .execute(&mut *tx).await?;
        // A CONFLICT here means this op was settled by someone else — and since `already_consumed`
        // said it was not, that someone else is a CONCURRENT save of this same account. The read
        // is a pool SELECT taken before this transaction opens, so two in-flight duplicate saves
        // both pass it; this is the only place the race becomes visible.
        //
        // `PRIMARY KEY (account_id, op_id)` is what makes the detection sound rather than
        // best-effort: the loser blocks on the winner's row lock until it commits, so by the time
        // `rows_affected` is read the answer is final, not a stale snapshot. Erroring rolls the
        // whole settlement back — the correct outcome, because the winner already applied it.
        if done.rows_affected() == 0 {
            eprintln!("[MARKET] op {id} for acct {account_id} was consumed concurrently — save rejected");
            return Err(sqlx::Error::RowNotFound);
        }
    }
    Ok(())
}

/// Settled by an accepted save in its own commit (`persist_save_and_claims`).
#[derive(Default)]
struct AcceptedExtras {
    fuse_consumed: Vec<u64>,
    /// The stored-at time to write (0 = now). The caller keeps the same value in memory.
    now_ms: i64,
    /// Already-settled ops this save re-presented, with their digest: filled in where the stored
    /// digest is NULL (cleared by `op_digest_v2`, or settled before digests existed), so the
    /// reuse check covers them again from here on.
    refill_digests: Vec<(u64, String)>,
}

/// ATOMIC durability for a verified save: the save row + trusted anchor + the mails its claims
/// consumed + the consumed-op marks commit in ONE transaction — commit-or-rollback. A claim can
/// never be durable without being marked consumed (the next save would pay it again), and a mail
/// can never be marked claimed without the save that holds its payload.
async fn persist_save_and_claims(
    db: &sqlx::PgPool,
    account_id: u64,
    state_json: &str,
    final_tick: u64,
    final_hash: &str,
    claims: &mailbox::ClaimSettlement,
    expected_prev: Option<&str>,
    keep: &[&str],
    after: &AcceptedExtras,
) -> Result<bool, sqlx::Error> {
    // ONE stored-at time for the row and for memory (audit 2026-10-03): two `now_ms()` calls left
    // them a transaction apart, so after a restart (memory reloaded from the row) a login's gap,
    // recorded from memory, never matched its save again and was dropped.
    let now = if after.now_ms > 0 { after.now_ms } else { now_ms() };
    let mut tx = db.begin().await?;
    // COMPARE-AND-SET on the latest save (audit 2026-10-02): two bundles from the same base used
    // to both pass the "latest only" check and both write, forking the account (one sends a box
    // to Steam, the other opens it). The row moves only if it still holds the save this bundle
    // was checked against; the loser writes nothing and is told to reload.
    let moved = match expected_prev {
        Some(prev) => sqlx::query(
            "UPDATE saves SET state_json = $2, final_tick = $3, final_hash = $4, saved_at_ms = $5 \
             WHERE account_id = $1 AND final_hash = $6",
        )
        .bind(account_id as i64).bind(state_json).bind(final_tick as i64).bind(final_hash).bind(now).bind(prev)
        .execute(&mut *tx).await?.rows_affected(),
        None => sqlx::query(
            "INSERT INTO saves (account_id, state_json, final_tick, final_hash, saved_at_ms) VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (account_id) DO NOTHING",
        )
        .bind(account_id as i64).bind(state_json).bind(final_tick as i64).bind(final_hash).bind(now)
        .execute(&mut *tx).await?.rows_affected(),
    };
    if moved != 1 {
        let _ = tx.rollback().await;
        return Ok(false);
    }
    sqlx::query("INSERT INTO trusted (account_id, state_hash) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(account_id as i64).bind(final_hash)
        .execute(&mut *tx).await?;
    // Only the new save and the base(s) it came from (`keep`) stay trusted (audit 2026-10-02): the
    // table grew a row per save forever and was loaded whole at boot. The base is kept so a
    // re-send of THIS bundle is still recognised as one.
    let keep: Vec<String> = keep.iter().map(|k| k.to_string()).collect();
    // And present: a base trusted only in memory (genesis, the old base of a continuation) must
    // survive a restart too, or the re-send it exists for reads as a forgery.
    for k in &keep {
        sqlx::query("INSERT INTO trusted (account_id, state_hash) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(account_id as i64).bind(k).execute(&mut *tx).await?;
    }
    sqlx::query("DELETE FROM trusted WHERE account_id = $1 AND state_hash <> $2 AND NOT (state_hash = ANY($3))")
        .bind(account_id as i64).bind(final_hash).bind(&keep)
        .execute(&mut *tx).await?;
    mailbox_store::consume_all(&mut tx, &claims.consume, now).await?;
    // What the accept settles beyond the save itself, in the SAME commit (audit 2026-10-03): the
    // fusion rolls it used and the offline gap it owes. Written after it in separate steps, a
    // request cut off between them left a roll outstanding or paid a gap twice.
    for n in &after.fuse_consumed {
        sqlx::query("DELETE FROM fuse_outstanding WHERE account_id = $1 AND nonce = $2")
            .bind(account_id as i64).bind(*n as i64).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO fuse_consumed (account_id, nonce) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(account_id as i64).bind(*n as i64).execute(&mut *tx).await?;
    }
    for (id, digest) in &after.refill_digests {
        sqlx::query("UPDATE consumed_market_ops SET digest = $3 WHERE account_id = $1 AND op_id = $2 AND digest IS NULL")
            .bind(account_id as i64).bind(*id as i64).bind(digest).execute(&mut *tx).await?;
    }
    mark_consumed(&mut tx, account_id, &claims.consumed_ops, &claims.digests).await?;
    tx.commit().await?;
    Ok(true)
}

/// How many times to retry the initial connect before giving up. Neon autosuspends, so a boot
/// that lands on a cold database gets a first-connect failure that means "wait", not "broken".
const DB_CONNECT_ATTEMPTS: u32 = 5;
const DB_RETRY_DELAY: Duration = Duration::from_secs(2);

/// Connect + migrate. **Fails CLOSED**: `DATABASE_URL` set but unusable exits the process.
///
/// This used to degrade to `None` — "in-memory only" — on any failure, to avoid crash-looping on
/// a bad URL. That trade is wrong now that real saves exist: the server came up healthy, `/health`
/// answered ok, and every save it accepted went into a HashMap that a restart discards. Progress
/// vanishing silently is far worse than a machine that visibly will not start, and Fly surfaces
/// the second one immediately.
///
/// Same shape as the roll secret: absent = a deliberate dev run (in-memory, said out loud);
/// present but unusable = a deployment that must not serve. The retry exists so Neon's cold start
/// is not mistaken for the latter.
async fn init_db() -> Option<sqlx::PgPool> {
    let Some(url) = std::env::var("DATABASE_URL").ok().filter(|u| !u.is_empty()) else {
        println!("no DATABASE_URL — IN-MEMORY ONLY, nothing is durable (dev run)");
        return None;
    };
    let mut last = String::new();
    for attempt in 1..=DB_CONNECT_ATTEMPTS {
        match sqlx::postgres::PgPoolOptions::new().max_connections(5).connect(&url).await {
            Ok(pool) => match migrate(&pool).await {
                Ok(()) => {
                    println!("connected to Postgres (durable)");
                    return Some(pool);
                }
                Err(e) => {
                    // A failed migration is NOT retried: the schema is now in an unknown state,
                    // and trying again is as likely to compound it as to fix it.
                    eprintln!("FATAL: DATABASE_URL is set but the migration failed ({e}).");
                    eprintln!("FATAL: refusing to serve — a half-migrated schema must not take writes.");
                    std::process::exit(1);
                }
            },
            Err(e) => {
                last = e.to_string();
                eprintln!("Postgres connect attempt {attempt}/{DB_CONNECT_ATTEMPTS} failed ({e})");
                if attempt < DB_CONNECT_ATTEMPTS {
                    tokio::time::sleep(DB_RETRY_DELAY).await;
                }
            }
        }
    }
    eprintln!("FATAL: DATABASE_URL is set but unreachable after {DB_CONNECT_ATTEMPTS} attempts ({last}).");
    eprintln!("FATAL: refusing to serve — accepting saves with no database loses them silently.");
    std::process::exit(1);
}

/// The gated routes `main` merges into the public router: the OPS group (ADMIN_KEY) always, the
/// test-fixture group (DEV_KEY) only outside production. Built here, not inline in `main`, so a
/// test can hold exactly what production serves.
fn gated_routes(state: &AppState) -> Router<AppState> {
    let dev_routes = Router::new()
        // ACCOUNT TAKEOVER, closed 2026-08-20. This was on the PUBLIC router with no gate at all,
        // and `login_as` keys accounts on the raw name — the same function `/auth/steam` calls as
        // `format!("steam:{steamid}")`. A SteamID64 is public; it is in the profile URL. So
        // `POST /auth/dev {"name":"steam:<their id>"}` returned the victim's account_id, their
        // server-chosen SEED, evicted their live token, upserted `active_sessions` so the takeover
        // survived a redeploy, and cleared their taint. Everything token-gated followed, including
        // `/store/checkout`, which fires a Steam MicroTxn InitTxn at the victim's steamid.
        // Squatting `steam:<id>` before a victim's first login worked identically.
        //
        // `docs/steam-integration.md` carried "prod: ปิด /auth/dev (env gate)" as an unchecked box.
        // An intent nothing enforces is not a control. `require_dev` fails closed when DEV_KEY is
        // unset, so a deployment that forgets the key refuses the route rather than opening it.
        .route("/auth/dev", post(auth_dev))
        // PROTOTYPE / DIAGNOSTIC routes that run a full sim per request with no account (a CPU
        // DoS surface) — dev-gated 2026-09-29, not public.
        .route("/demo/bundle", get(demo_bundle))
        .route("/verify", post(verify))
        .route("/ingest", post(ingest))
        .route("/submit", post(submit))
        .route("/dev/steam-mint-test", post(dev_steam_mint_test))
        .route("/dev/gacha-crystal", post(dev_gacha_crystal))
        // (audit MED) /mail/claim consumes REAL mailbox state out-of-band (no GameState credit) →
        // value loss + self-taint. The production claim path is ClaimMail → verified save →
        // consume_claim; this stays for tests only → DEV-GATED.
        .route("/mail/claim", post(mail_claim))
        // route_layer: gate ONLY this group's routes. `.layer` also wraps the fallback, which turned
        // every unknown path into a 403 from this gate instead of a 404.
        .route_layer(middleware::from_fn_with_state(state.clone(), require_dev));
    // OPS routes (2026-09-29): what production needs to run — refunds, compensation mail, the
    // battle-pass season, the audit and storage reports — behind ADMIN_KEY, not the fixture key.
    let admin_routes = Router::new()
        .route("/dev/mail-send", post(mail_send))
        .route("/dev/bp-season", post(dev_bp_season))
        .route("/dev/storage", get(dev_storage))
        .route("/dev/balance", get(dev_balance))
        .route("/store/refund", post(store_refund))
        .route("/admin/store/grant", post(admin_store_grant))
        .route("/admin/ban", post(admin_ban))
        .route("/admin/unban", post(admin_unban))
        .route("/admin/bans", get(admin_bans))
        .route("/flags", get(list_flags))
        // route_layer: gate ONLY this group's routes. `.layer` also wraps the fallback, which turned
        // every unknown path into a 403 from this gate instead of a 404.
        .route_layer(middleware::from_fn_with_state(state.clone(), require_admin));
    // PROD=1: the test fixtures are not served at all — not gated, ABSENT (404).
    if state.prod {
        println!("PROD=1 — test-fixture routes unmounted, DEV_KEY ignored");
        admin_routes
    } else {
        admin_routes.merge(dev_routes)
    }
}

/// The production `Router`, built here rather than inline in `main` so a test can hold one.
///
/// Nothing about the routes changes by extracting them — that is the point. A test that builds
/// its own router proves things about the test's router; this is the one the server serves.
fn build_router(state: AppState, dev_routes: Router<AppState>) -> Router {
    let app = Router::new()
        .route("/health", get(health))
        .route("/version", get(version))
        .route("/steam/icon/:name", get(steam_icon))
        .route("/auth/steam", post(auth_steam))
        .route("/me", get(me))
        .route("/save", post(put_save).get(get_save))
        .route("/daily/state", get(daily_state_ep))
        .route("/daily/claim", post(daily_claim))
        .route("/tutorial/state", get(tutorial_state))
        .route("/tutorial/claim", post(tutorial_claim))
        .route("/badges", get(badges))
        .route("/shop", get(shop_state))
        .route("/mint-coupons", get(mint_coupons_state))
        .route("/mint-coupons/claim", post(mint_coupons_claim))
        .route("/steam/inventory", get(steam_inventory))
        .route("/steam/import", post(steam_import_items))
        .route("/shop/buy", post(shop_buy))
        .route("/weekly/state", get(weekly_state_ep))
        .route("/weekly/claim", post(weekly_claim))
        .route("/commander/state", get(commander_state_ep))
        .route("/commander/claim", post(commander_claim))
        .route("/claim/state", get(claim_state_ep))
        .route("/claim/collect", post(claim_collect))
        .route("/bp/state", get(bp_state_ep))
        .route("/bp/claim", post(bp_claim))
        .route("/story/claim", post(story_claim))
        .route("/story/progress", get(story_progress))
        .route("/gacha/roll", post(gacha_roll))
        .route("/fuse/roll", post(fuse_roll))
        .route("/gacha/exchange", post(gacha_exchange))
        .route("/gacha/standard-select", post(gacha_standard_select))
        .route("/gacha/state", get(gacha_state))
        .route("/gacha/history", get(gacha_history))
        .route("/farm/roll", post(farm_roll))
        .route("/store/checkout", post(store_checkout))
        .route("/store/finalize", post(store_finalize))
        .route("/telemetry", post(telemetry))
        .route("/store", get(store_catalog))
        .route("/store/me", get(store_me))
        .route("/monthly/claim", post(monthly_claim))
        .route("/mail", get(mail_inbox));
    app.merge(dev_routes)
        // Onion order: the LAST .layer() added is the OUTERMOST (runs first). So a request
        // passes rate_limit (per-IP) → buffer_body (whole body read, bounded) →
        // concurrency_limit (global load-shed) → request_timeout
        // (whole-request deadline) → access_log → handler. Cheap per-IP reject first, then shed
        // on total load, then bound how long the surviving work may run.
        .layer(middleware::from_fn(access_log))
        .layer(middleware::from_fn(request_timeout))
        .layer(middleware::from_fn_with_state(state.clone(), concurrency_limit))
        .layer(middleware::from_fn(buffer_body))
        .layer(middleware::from_fn_with_state(state.clone(), rate_limit))
        .with_state(state)
}

/// A table the server must hold in memory could not be read at boot. EXIT rather than start
/// empty (audit 2026-10-02): an empty `saves` map answered GET /save with 404, the client founded
/// a new party, and its first save overwrote the real one in the database.
fn boot_load_failed<T>(e: &sqlx::Error) -> T {
    eprintln!("[BOOT] loading state from the database failed: {e} — refusing to start empty");
    std::process::exit(1)
}

/// Load the durable tables into the in-memory working set at startup.
async fn load_from_db(
    db: &sqlx::PgPool,
    sessions: &Sessions,
    saves: &Mutex<HashMap<u64, StoredSave>>,
    trusted: &Mutex<HashMap<u64, HashSet<String>>>,
    fuse_rolls: &Mutex<HashMap<u64, Vec<OutstandingRoll>>>,
    fuse_consumed: &Mutex<HashMap<u64, HashSet<u64>>>,
) {
    let accounts: Vec<(i64, String, i64)> =
        sqlx::query_as("SELECT account_id, name, seed FROM accounts").fetch_all(db).await.unwrap_or_else(|e| boot_load_failed(&e));
    // EVERY identity, not one per account. Reading `accounts.name` alone would leave a second
    // sign-in unable to resolve until its owner logged in through the first one.
    let identities: Vec<(String, i64)> =
        sqlx::query_as("SELECT identity, account_id FROM account_identities").fetch_all(db).await.unwrap_or_else(|e| boot_load_failed(&e));
    let mut max_id = 0i64;
    let mut by_id: HashMap<u64, Account> = HashMap::new();
    for (id, name, seed) in &accounts {
        by_id.insert(*id as u64, Account { account_id: *id as u64, name: name.clone(), seed: *seed as u64 });
        max_id = max_id.max(*id);
    }
    {
        let mut by_identity = sessions.by_identity.held();
        for (identity, account_id) in &identities {
            if let Some(acct) = by_id.get(&(*account_id as u64)) {
                by_identity.insert(identity.clone(), acct.clone());
            }
        }
    }
    sessions.next_id.store((max_id + 1).max(1) as u64, Ordering::SeqCst);

    // Restore the active-session tokens so a restart doesn't 401 (kick) connected clients.
    // Never a banned account's (audit 2026-10-03): an older build's eviction could fail or race a
    // login and leave the row behind, and restoring it handed the banned account its session back.
    let arows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT s.account_id, s.token FROM active_sessions s WHERE NOT EXISTS (SELECT 1 FROM account_bans b \
           WHERE b.account_id = s.account_id AND (b.until_ms IS NULL OR b.until_ms > $1))",
    ).bind(now_ms()).fetch_all(db).await.unwrap_or_else(|e| boot_load_failed(&e));
    {
        let mut active = sessions.active.held();
        let mut by_token = sessions.by_token.held();
        for (id, token) in arows {
            if let Some(acct) = by_id.get(&(id as u64)) {
                active.insert(id as u64, token.clone());
                by_token.insert(token, acct.clone());
            }
        }
    }

    let srows: Vec<(i64, String, i64, String, i64)> =
        sqlx::query_as("SELECT account_id, state_json, final_tick, final_hash, saved_at_ms FROM saves").fetch_all(db).await.unwrap_or_else(|e| boot_load_failed(&e));
    {
        let mut s = saves.held();
        for (id, sj, ft, fh, at) in srows {
            s.insert(id as u64, StoredSave { state_json: sj, final_tick: ft as u64, final_hash: fh, saved_at_ms: at });
        }
    }

    let trows: Vec<(i64, String)> =
        sqlx::query_as("SELECT account_id, state_hash FROM trusted").fetch_all(db).await.unwrap_or_else(|e| boot_load_failed(&e));
    {
        let mut t = trusted.held();
        for (id, h) in trows {
            t.entry(id as u64).or_default().insert(h);
        }
    }

    // Server-roll FUSION: outstanding rolls + consumed nonces, so a restart keeps the record and
    // cannot false-flag a legitimate fusion as forged. Sweep orphans first (issued but never
    // consumed past the TTL — an abandoned or doomed session) so they do not reload.
    let pruned = sqlx::query("DELETE FROM fuse_outstanding WHERE created_at < now() - INTERVAL '1 hour'")
        .execute(db).await.map(|r| r.rows_affected()).unwrap_or(0);
    if pruned > 0 {
        println!("pruned {pruned} orphaned fusion roll(s)");
    }
    let orows: Vec<(i64, i64, String, i64)> =
        sqlx::query_as("SELECT account_id, nonce, key, attempt FROM fuse_outstanding")
            .fetch_all(db).await.unwrap_or_else(|e| boot_load_failed(&e));
    {
        let mut r = fuse_rolls.held();
        for (id, nonce, key, attempt) in orows {
            r.entry(id as u64).or_default().push(OutstandingRoll {
                key,
                attempt: attempt as u64,
                nonce: nonce as u64,
                issued_at: SystemTime::now(),
            });
        }
    }
    let crows: Vec<(i64, i64)> =
        sqlx::query_as("SELECT account_id, nonce FROM fuse_consumed").fetch_all(db).await.unwrap_or_else(|e| boot_load_failed(&e));
    {
        let mut c = fuse_consumed.held();
        for (id, nonce) in crows {
            c.entry(id as u64).or_default().insert(nonce as u64);
        }
    }
    println!("loaded {} accounts from Postgres", accounts.len());
}

#[tokio::main]
async fn main() {
    // REAL MONEY ONLY IN PRODUCTION MODE (2026-09-29): a server that takes real money with the
    // test fixtures mounted — `/auth/dev` can sign in as any Steam account — must not start.
    if let Some(why) = live_money_misconfig(
        steam_mtx_live(),
        prod_mode(),
        std::env::var("DEV_KEY").is_ok_and(|k| !k.is_empty()),
    ) {
        eprintln!("FATAL: {why}");
        std::process::exit(1);
    }
    // The server loads the SAME catalog the client ran (version-checked per bundle).
    let path = std::env::var("GAME_DATA_PATH").unwrap_or_else(|_| "data/game_data.json".to_string());
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"));
    let data = Arc::new(GameData::from_json_str(&json).expect("parse game_data.json"));
    println!("loaded game_data v{} from {path}", data.version);

    // Commander reward config — the SAME file the client renders from (a byte-identical copy ships under
    // godot/). A malformed or drifted file is a boot panic, not a silent bad payout (parse_rewards
    // cross-checks level coverage + the skill column against the signed-grant authority).
    let rewards_path =
        std::env::var("COMMANDER_REWARDS_PATH").unwrap_or_else(|_| "data/commander_rewards.json".to_string());
    let rewards_json = std::fs::read_to_string(&rewards_path)
        .unwrap_or_else(|e| panic!("read {rewards_path}: {e}"));
    let commander_rewards = Arc::new(
        commander::parse_rewards(&rewards_json).unwrap_or_else(|e| panic!("parse {rewards_path}: {e}")),
    );
    println!("loaded {} commander reward levels from {rewards_path}", commander_rewards.len());

    // Durable store (accounts / saves / trusted-anchors). Overlay model: the maps
    // below are the working set; the DB is loaded into them at startup and
    // written-through on change. No DATABASE_URL → in-memory only (not durable).
    let db: Option<sqlx::PgPool> = init_db().await;

    // ROLL SECRET — keys every money-bearing draw (gacha seeds, refine nonces) through blake3.
    //
    // FAIL CLOSED. This used to end in `.unwrap_or_else(|| "dev-refine-secret".into())`, so an
    // environment where nobody set the variable silently ran on a constant that is published in
    // this file. That is not a guessing problem, it is a SELECTION one: the gacha seed is a
    // deterministic function of the client's own `idem_key`, so anyone holding the secret can
    // enumerate keys offline until one yields an SSR and then send that key.
    //
    // Either name is accepted — production has long set REFINE_SECRET and both feed the same
    // keyed hash — but the literal is now dev-only, and "dev" means "no database", the same
    // signal the rest of the server already uses to tell a local run from a deployment.
    // FARM BANNER: an event runs only when FARM_EVENT holds a valid one — never by default.
    let farm_events = match farm::Event::list_from_env_str(std::env::var("FARM_EVENT").ok().as_deref()) {
        Ok(list) => list,
        Err(e) => {
            eprintln!("[farm] FARM_EVENT is set but invalid ({e}) — no event runs");
            Vec::new()
        }
    };
    for e in &farm_events {
        println!("farm banner: {} ({}..{} ms), box {}, featured {}", e.id, e.starts_ms, e.ends_ms, e.box_def, e.featured);
    }
    let roll_secret = match std::env::var("SERVER_ROLL_SECRET").ok().filter(|k| !k.is_empty())
        .or_else(|| std::env::var("REFINE_SECRET").ok().filter(|k| !k.is_empty()))
    {
        Some(s) => {
            println!("roll secret: from env ({} chars)", s.len());
            s
        }
        None if db.is_none() => {
            println!("roll secret: DEV FALLBACK (no database, no env) — never in production");
            "dev-refine-secret".to_string()
        }
        None => {
            eprintln!(
                "FATAL: a database is configured but neither SERVER_ROLL_SECRET nor REFINE_SECRET \
                 is set. Refusing to start on the hardcoded dev secret — every gacha pull and \
                 refine roll would be selectable by any client that reads this repo.\n\
                 Fix: fly secrets set SERVER_ROLL_SECRET=$(openssl rand -hex 32)"
            );
            std::process::exit(1);
        }
    };

    let sessions = Arc::new(Sessions {
        next_id: AtomicU64::new(1),
        by_token: Mutex::new(HashMap::new()),
        by_identity: Mutex::new(HashMap::new()),
        active: Mutex::new(HashMap::new()),
        tainted: Mutex::new(HashSet::new()),
    });
    let saves = Arc::new(Mutex::new(HashMap::new()));
    let trusted = Arc::new(Mutex::new(HashMap::new()));
    let fuse_rolls = Arc::new(Mutex::new(HashMap::new()));
    let fuse_consumed = Arc::new(Mutex::new(HashMap::new()));
    if let Some(pool) = &db {
        load_from_db(pool, &sessions, &saves, &trusted, &fuse_rolls, &fuse_consumed).await;
    }
    // Seed the fast-forward anchor for every loaded account at STARTUP, backdated by
    // RESTART_GRACE_MS. A connected client re-`GET`s on its next handshake and re-anchors
    // anyway; the grace is for the one that does NOT — it was playing (or holding a stalled
    // backlog) while we were down, and anchoring at bare `now` would cheat-flag it for ticks
    // it earned legitimately. Still bounded, so a scale-to-zero + one huge push can't
    // fast-forward more than the grace.
    let tick_ref = {
        let floor = now_ms() - RESTART_GRACE_MS;
        let m: HashMap<u64, TickAnchor> = saves
            .held()
            .iter()
            .map(|(id, s)| {
                (*id, TickAnchor {
                    tick: s.final_tick,
                    ms: boot_anchor_ms(s.saved_at_ms, floor),
                    recorded: 0,
                    from_handshake: false,
                    synthesized: false,
                })
            })
            .collect();
        Arc::new(Mutex::new(m))
    };

    // LAYER 2: the cumulative-excess counters. Durable, so a restart does not hand every
    // account a clean slate — that would make "wait for a deploy" a way to launder the counter.
    let lifetime_excess = Arc::new(Mutex::new(HashMap::new()));
    if let Some(pool) = &db {
        // A failed read must not start every account at zero excess (audit 2026-10-03): that is the
        // launder-by-redeploy hole this table closes.
        let rows: Vec<(i64, i64)> = sqlx::query_as("SELECT account_id, lifetime_excess FROM integrity")
            .fetch_all(pool)
            .await
            .unwrap_or_else(|e| boot_load_failed(&e));
        let mut m = lifetime_excess.held();
        for (id, e) in rows {
            m.insert(id as u64, e);
        }
        println!("loaded {} integrity counters", m.len());
    }

    let rare_rate_mult_bp = parse_rate_mult(std::env::var("RARE_RATE_MULT_BP").ok().as_deref());
    let state = AppState {
        fuse_rolls,
        fuse_consumed,
        data,
        db,
        sessions,
        saves,
        trusted,
        saving: Default::default(),
        replay_slots: Arc::new(tokio::sync::Semaphore::new(replay_parallelism())),
        replay_budget: Default::default(),
        steam_reads: Default::default(),
        pins: Arc::new(Mutex::new(HashMap::new())),
        stats: Arc::new(VerifyStats { total: AtomicU64::new(0), replayed: AtomicU64::new(0) }),
        crystal_breaks: Arc::default(),
        dev_key: if prod_mode() { None } else { std::env::var("DEV_KEY").ok().filter(|k| !k.is_empty()) },
        admin_key: std::env::var("ADMIN_KEY").ok().filter(|k| !k.is_empty()),
        prod: prod_mode(),
        rare_rate_mult_bp,
        metrics: Arc::new(Mutex::new(metrics::Metrics::default())),
        rate: Arc::new(Mutex::new(HashMap::new())),
        inflight: Arc::new(tokio::sync::Semaphore::new(MAX_INFLIGHT)),
        roll_secret,
        farm_events,
        tick_ref,
        lifetime_excess,
        season: Arc::new(Mutex::new(None)),
        tracks: Arc::new(schedule::tracks()),
        commander_levels: Arc::new(commander::build_level_table()),
        commander_rewards,
    };
    if state.rare_rate_mult_bp != RARE_RATE_MULT_DEFAULT_BP {
        println!(
            "rare-rate multiplier ACTIVE: x{}.{:02} (RARE_RATE_MULT_BP={})",
            state.rare_rate_mult_bp / 10_000,
            (state.rare_rate_mult_bp % 10_000) / 100,
            state.rare_rate_mult_bp,
        );
    }
    // Load the ACTIVE season (now within [starts, ends)) — absent/garbled = BP disabled.
    if let Some(db) = &state.db {
        match load_active_season(db, now_ms()).await {
            Some(sn) => {
                println!("bp: season {} active (ends_at_ms {})", sn.season_id, sn.ends_at_ms);
                *state.season.held() = Some(sn);
            }
            None => println!("bp: no active season — battle pass disabled"),
        }
    }
    // Ungated dev/economy endpoints (account-drain / fake-mint surface) → require
    // X-Dev-Key. Grouped so one layer gates them all.
    let metrics_state = state.clone();
    let refund_state = state.clone();
    let mint_state = state.clone();
    let dev_routes = gated_routes(&state);

    // ONE SWEEP AT BOOT, before serving. The hourly loop only sees drift that happens while this
    // process is up; a restart is the moment accumulated drift from every previous process shows
    // itself, and it is also the only moment anyone is reading the log on purpose.
    sweep_crystal_conservation(&state).await;

    let app = build_router(state.clone(), dev_routes);

    let addr = std::env::var("BIND").unwrap_or_else(|_| "127.0.0.1:8787".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind");
    println!("verify-server (sim-core {}) listening on http://{addr}", sim_core::VERSION);
    // CHARGED-BUT-NOT-GRANTED recovery: every 5 minutes, grant any order Steam charged whose grant
    // did not complete — the player is paid even if they never retry.
    if let Some(db) = state.db.clone() {
        let data = state.data.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(300));
            loop {
                tick.tick().await;
                sweep_charged_orders(&db, &data).await;
            }
        });
    }
    tokio::spawn({
        // 60 s. A lost flush costs at most a minute of resolution on aggregate counters, which
        // is the trade `metrics.rs` documents; anything that must be exact lives in a ledger.
        let st = metrics_state;
        async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
            let mut since_prune = 0u32;
            loop {
                tick.tick().await;
                flush_metrics(&st).await;
                // Hourly, on the loop that already exists. A closed commission day answers no
                // question — unclaimed rewards are forfeited at the boundary, so the row is dead
                // the moment the day turns. Left alone it would grow by one row per active player
                // per day, forever.
                since_prune += 1;
                if since_prune >= 60 {
                    since_prune = 0;
                    prune_daily_progress(&st).await;
                    prune_weekly_progress(&st).await;
                    retire_tracks(&st).await;
                    sweep_crystal_conservation(&st).await;
                    refresh_season(&st).await;
                }
            }
        }
    });
    // STEAM CALLS ON THEIR OWN LOOPS (audit 2026-10-02): run inline above, up to 500 sequential
    // 10 s refund lookups stalled the metrics flush and the prunes for over an hour while Steam was
    // slow, and the missed ticks then fired back to back. `Delay` = no burst catch-up.
    tokio::spawn({
        let st = refund_state;
        async move {
            // First run an hour after boot, not at it: a crash loop must not hit Steam per restart.
            let hour = std::time::Duration::from_secs(3600);
            let mut tick = tokio::time::interval_at(tokio::time::Instant::now() + hour, hour);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tick.tick().await;
                // Hourly is the right cadence for money coming BACK: a refund is not urgent to the
                // player and not urgent to us beyond the day, but leaving it undone means crystal
                // that were paid for and then un-paid-for stay spendable forever.
                reconcile_steam_refunds(&st).await;
            }
        }
    });
    // `charging` orders on their own loop: up to 50 sequential QueryTxn calls must not stall the
    // charged-order sweep that pays players.
    if let Some(db) = state.db.clone() {
        tokio::spawn(async move {
            let five = std::time::Duration::from_secs(300);
            let mut tick = tokio::time::interval_at(tokio::time::Instant::now() + five, five);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tick.tick().await;
                sweep_charging_orders(&db).await;
            }
        });
    }
    tokio::spawn({
        let st = mint_state;
        async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tick.tick().await;
                // Every minute: mint what players sent to Steam (a no-op unless switched on).
                mint_steam_items(&st).await;
            }
        }
    });
    axum::serve(listener, app).await.expect("serve");
}

/// Commission days to keep (user 2026-08-12). A month is far more than correctness needs — a
/// closed day is forfeited and never read again — and it is the window a support question can
/// still plausibly reach back into.
///
/// What matters is that it is BOUNDED: storage becomes a function of how many people played this
/// month, not of how long the game has been live.
///
/// Nothing reads a closed day: rewards are forfeited at the boundary and the accrual only ever
/// addresses `day_of_ms(now)`. Retention here buys diagnosis, not correctness.
const DAILY_KEEP_DAYS: i64 = 30;

/// Delete commission rows for days that have closed.
///
/// Bounded by construction rather than by a schedule: rows exist only for (account, day) pairs
/// that actually played, and this keeps at most `BP_DAILY_KEEP_DAYS` of them. Storage stops being
/// a function of how long the game has been live and becomes a function of how many people played
/// this week, which is the only shape that does not need watching.
/// Delete the rows of every track that can never be claimed from again.
///
/// CONFIG ANSWERS FIRST. `retired_tracks` reads the in-memory list and returns empty on almost
/// every run — so the usual hour issues NO query at all, rather than scanning a table that grows
/// with (accounts x events ever run) to discover there is nothing to do. That is also why
/// `claim_progress` needs no index on `track`: the only query that would want one runs a handful
/// of times a year.
///
/// A track with NO WINDOW is never returned here, and that is load-bearing rather than tidy. A
/// finished 7-day ladder looks exactly like a dead row — every cell claimed, nothing left to do —
/// but deleting it is indistinguishable from a row that never existed, so the next advance
/// re-creates it at position 1 and the account walks the whole ladder again. Rows die with their
/// TRACK, never on their own. `schedule::a_windowless_track_is_never_retired` is the guard.
///
/// The config entry must OUTLIVE the rows: dropping a TrackDef first leaves rows nothing can
/// render or delete. They are inert if that happens — an unknown track never advances, never shows
/// and never pays — but they are also permanent, so the order is retire, sweep, then remove.
/// Evaluate the crystal conservation law and remember what it found.
///
/// `crystal_conservation_violations` existed for a day with NO caller but a test — the audit's words:
/// "in production the conservation law is never evaluated. It is a function that exists so a test
/// can call it." A law nobody evaluates is a comment. This is the runner.
///
/// It LOGS and records; it does not refuse service. A break means the books disagree, which needs
/// a human reading them — turning the store off would turn a bookkeeping question into an outage,
/// and the wallet is still the number the player spends either way.
async fn sweep_crystal_conservation(st: &AppState) {
    let Some(db) = &st.db else { return };
    match crystal_conservation_violations(db).await {
        Ok(v) => {
            if !v.is_empty() {
                eprintln!(
                    "[GEMS] CONSERVATION BROKEN on {} account/bucket(s) — the wallet and the ledger \
                     disagree. Short = crystal the ledger promised and the wallet never got; \
                     Unexplained = crystal with no row behind them. First few: {:?}",
                    v.len(),
                    v.iter().take(5).collect::<Vec<_>>()
                );
            }
            st.crystal_breaks.store(v.len(), Ordering::Relaxed);
        }
        // A failed query is not a clean bill of health. Leave the previous count standing rather
        // than reporting zero breaks because the check could not run.
        Err(e) => eprintln!("[GEMS] conservation sweep failed, last count kept: {e}"),
    }
}

async fn retire_tracks(st: &AppState) {
    let Some(db) = &st.db else { return };
    let day = daily::day_of_ms(now_ms() as u64);
    let ids: Vec<String> =
        schedule::retired_tracks(&st.tracks, day).iter().map(|s| s.to_string()).collect();
    if ids.is_empty() {
        return;
    }
    match sqlx::query("DELETE FROM claim_progress WHERE track = ANY($1)").bind(&ids).execute(db).await {
        Ok(r) => println!("retired {:?}: {} claim row(s) deleted", ids, r.rows_affected()),
        Err(e) => println!("claim_progress retirement failed (retrying next hour): {e}"),
    }
}

async fn prune_daily_progress(st: &AppState) {
    let Some(db) = &st.db else { return };
    let cutoff = daily::day_of_ms(now_ms() as u64) - DAILY_KEEP_DAYS;
    match sqlx::query("DELETE FROM daily_progress WHERE day < $1").bind(cutoff).execute(db).await {
        Ok(r) if r.rows_affected() > 0 => {
            println!("pruned {} closed commission day row(s)", r.rows_affected());
        }
        Ok(_) => {}
        // Never fatal: a failed prune costs disk, not correctness, and the next hour retries.
        Err(e) => println!("daily_progress prune failed (retrying next hour): {e}"),
    }
}

/// Weeks of closed `weekly_progress` rows to keep. Like the daily prune, a closed week can never be
/// claimed again, so it is dead storage the moment the week turns.
const WEEKLY_KEEP_WEEKS: i64 = 4;

async fn prune_weekly_progress(st: &AppState) {
    let Some(db) = &st.db else { return };
    let cutoff = weekly::week_of_ms(now_ms() as u64) - WEEKLY_KEEP_WEEKS;
    match sqlx::query("DELETE FROM weekly_progress WHERE week < $1").bind(cutoff).execute(db).await {
        Ok(r) if r.rows_affected() > 0 => {
            println!("pruned {} closed commission week row(s)", r.rows_affected());
        }
        Ok(_) => {}
        Err(e) => println!("weekly_progress prune failed (retrying next hour): {e}"),
    }
}

/// LIVENESS. Always 200, deliberately: Fly restarts an instance whose health check fails, and a
/// crystal-conservation break is a finding to investigate, not a reason to restart. Liveness is
/// "the process is answering"; the break count rides in the body.
async fn health(State(st): State<AppState>) -> String {
    let mut out = String::from("ok");
    // Cached from the hourly sweep, never computed here. Absent means zero, so a healthy server
    // still answers a bare "ok" and nothing has to parse a number that is usually noise.
    let breaks = st.crystal_breaks.load(Ordering::Relaxed);
    if breaks > 0 {
        out.push_str(&format!(" crystal_breaks={breaks}"));
    }
    out
}

/// The sim + game_data versions this server verifies against. A client must match both
/// (a bundle with a different version is rejected) — this lets a pre-flight check catch a
/// client/server skew BEFORE playing, instead of hitting silent save rejections in-game.
async fn version(State(st): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "sim_version": sim_core::VERSION,
        "game_data_version": st.data.version,
        // Git identity of the deployed sim (build-arg via Dockerfile) — deploy
        // tooling compares this against local HEAD + the client dylib's stamp,
        // so "same VERSION string, different code" skew can't ship silently.
        "git_sha": sim_core::GIT_STAMP.trim_start_matches("SIMGIT:"),
        "sim_build": sim_core::SIM_BUILD,
        // Deployment mode, so deploy tooling can show it and refuse the one bad pairing.
        "prod": st.prod,
        "money_live": steam_mtx_live(),
    }))
}

/// Gate for the still-ungated dev/economy endpoints: require `X-Dev-Key` == DEV_KEY.
/// No DEV_KEY set → 503 (fail closed). Wrong/absent key → 403. Closes the PoC
/// account-drain / fake-mint holes to random callers until the real gates land.
/// `PROD=1` in the environment: production mode (test-fixture routes unmounted, DEV_KEY ignored).
fn prod_mode() -> bool {
    std::env::var("PROD").is_ok_and(|v| v == "1")
}

/// Does header `name` carry `expected`? Compared as blake3 digests, so the time taken does not
/// depend on how many leading bytes of a guess were right.
fn header_key_matches(req: &Request, name: &str, expected: &str) -> bool {
    req.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|k| blake3::hash(k.as_bytes()) == blake3::hash(expected.as_bytes()))
}

/// Gate for the OPS routes: `X-Admin-Key` == ADMIN_KEY. Outside production a matching `X-Dev-Key`
/// is also accepted, so local tools and probes keep one key. Fails closed when neither is set.
async fn require_admin(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let admin = st.admin_key.as_deref().is_some_and(|k| header_key_matches(&req, "x-admin-key", k));
    let dev = !st.prod && st.dev_key.as_deref().is_some_and(|k| header_key_matches(&req, "x-dev-key", k));
    if admin || dev {
        next.run(req).await
    } else if st.admin_key.is_none() && (st.prod || st.dev_key.is_none()) {
        (StatusCode::SERVICE_UNAVAILABLE, "endpoint disabled (no ADMIN_KEY set)").into_response()
    } else {
        (StatusCode::FORBIDDEN, "forbidden (bad/missing X-Admin-Key)").into_response()
    }
}

async fn require_dev(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let Some(expected) = st.dev_key.as_deref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "endpoint disabled (no DEV_KEY set)").into_response();
    };
    let ok = header_key_matches(&req, "x-dev-key", expected);
    if ok {
        next.run(req).await
    } else {
        (StatusCode::FORBIDDEN, "forbidden (bad/missing X-Dev-Key)").into_response()
    }
}

/// Loose per-IP fixed-window DoS backstop. 429 over `RATE_MAX` requests per
/// `RATE_WINDOW_SECS`. Skips /health (Fly's internal probe). Keyed on Fly-Client-IP.
const RATE_WINDOW_SECS: u64 = 60;
const RATE_MAX: u32 = 120;
/// Size at which `rate_limit` sweeps expired windows out of the per-IP map.
const RATE_MAP_PRUNE_AT: usize = 10_000;

async fn rate_limit(State(st): State<AppState>, req: Request, next: Next) -> Response {
    if req.uri().path() == "/health" {
        return next.run(req).await;
    }
    let ip = req
        .headers()
        .get("fly-client-ip")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();
    let now = Instant::now();
    let over = {
        let mut m = st.rate.held();
        // Bounded: every unique IP used to stay in the map forever. Past RATE_MAP_PRUNE_AT entries,
        // drop every window that has already expired (those IPs would reset on their next hit
        // anyway, so nothing a live client sees changes).
        if m.len() >= RATE_MAP_PRUNE_AT {
            m.retain(|_, e| now.duration_since(e.1).as_secs() < RATE_WINDOW_SECS);
        }
        let e = m.entry(ip).or_insert((0, now));
        if now.duration_since(e.1).as_secs() >= RATE_WINDOW_SECS {
            *e = (0, now);
        }
        e.0 += 1;
        e.0 > RATE_MAX
    };
    if over {
        (StatusCode::TOO_MANY_REQUESTS, "rate limited").into_response()
    } else {
        next.run(req).await
    }
}

/// Access log: prints "METHOD path -> status" per request.
async fn access_log(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let resp = next.run(req).await;
    println!("{method} {path} -> {}", resp.status().as_u16());
    resp
}

/// Inbound whole-request deadline. A handler still running past this is abandoned and the
/// caller gets 504 — the catch-all that stops a wedged handler or a slow request body from
/// pinning a worker forever. The outbound Steam (5s) and DB-acquire (2s) timeouts cap those
/// specific calls; this bounds the request as a whole. Generous vs. a healthy request
/// (<1s; a worst-case 50k-tick replay is ~0.18s).
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// Largest request body any route takes (axum's `Json` default, which every route relied on).
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
/// How long a client may take to SEND its body.
const BODY_READ_SECS: u64 = 10;

/// Read the whole body BEFORE the request takes any shared permit (audit 2026-10-03): `Json` read
/// it inside the handler, so a client trickling a byte every few seconds held an in-flight permit
/// for the full request timeout and 64 of them shed everything with 503.
///
/// RAM is charged by the bytes ACTUALLY buffered, one `BODY_UNIT` permit of `BODY_BUDGET` per
/// 64 KiB as the body arrives (round 10: charging the DECLARED size up front let 8 IPs trickling
/// "2 MiB" bodies hold the whole budget, and an undeclared GET body was not charged at all), and
/// no IP may hold more than `BODY_UNITS_PER_IP`. The permits are held through the handler while
/// the body is in memory. A body slower than `BODY_MIN_RATE` after `BODY_RATE_GRACE` is cut off;
/// at most `BODY_READS_PER_IP` reads per client IP run at once; a GET may not carry a body.
async fn buffer_body(req: Request, next: Next) -> Response {
    use futures_util::StreamExt;
    let declared = req.headers().get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<usize>().ok());
    if declared.is_some_and(|n| n > MAX_BODY_BYTES) {
        return (StatusCode::PAYLOAD_TOO_LARGE, "request body too large").into_response();
    }
    let limit = if req.method() == axum::http::Method::GET { 0 } else { declared.unwrap_or(MAX_BODY_BYTES) };
    let ip = req.headers().get("fly-client-ip").and_then(|v| v.to_str().ok()).unwrap_or("unknown").to_string();
    let Some(reading) = BodyRead::begin(ip) else {
        return (StatusCode::TOO_MANY_REQUESTS, "too many uploads at once").into_response();
    };
    let (parts, body) = req.into_parts();
    let started = Instant::now();
    let mut buf: Vec<u8> = Vec::new();
    let mut held: Option<tokio::sync::SemaphorePermit<'static>> = None;
    let mut stream = body.into_data_stream();
    loop {
        let left = Duration::from_secs(BODY_READ_SECS).saturating_sub(started.elapsed());
        let next_chunk = match tokio::time::timeout(left.min(Duration::from_secs(1)), stream.next()).await {
            Ok(Some(Ok(c))) => Some(c),
            Ok(Some(Err(_))) => return (StatusCode::BAD_REQUEST, "request body unreadable").into_response(),
            Ok(None) => break,
            Err(_) => None, // a quiet second: fall through to the rate check
        };
        if let Some(c) = next_chunk {
            if buf.len() + c.len() > limit {
                return (StatusCode::PAYLOAD_TOO_LARGE, "request body too large").into_response();
            }
            // Charge the new bytes before keeping them.
            let need = (buf.len() + c.len()).div_ceil(BODY_UNIT);
            let have = held.as_ref().map_or(0, |p| p.num_permits());
            if need > have {
                let more = u32::try_from(need - have).unwrap_or(u32::MAX);
                if !reading.charge(more) {
                    return (StatusCode::TOO_MANY_REQUESTS, "too many uploads at once").into_response();
                }
                match BODY_BUDGET.try_acquire_many(more) {
                    Ok(p) => match held.as_mut() {
                        Some(h) => h.merge(p),
                        None => held = Some(p),
                    },
                    Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, "server busy").into_response(),
                }
            }
            buf.extend_from_slice(&c);
        }
        let elapsed = started.elapsed();
        let floor = elapsed.saturating_sub(BODY_RATE_GRACE).as_millis().saturating_mul(u128::from(BODY_MIN_RATE)) / 1000;
        if elapsed >= Duration::from_secs(BODY_READ_SECS) || (buf.len() as u128) < floor {
            return (StatusCode::REQUEST_TIMEOUT, "request body too slow").into_response();
        }
    }
    drop(reading);
    // The permits are held through the handler: the buffered body lives that long.
    let _held = held;
    next.run(Request::from_parts(parts, axum::body::Body::from(buf))).await
}

/// RAM all request bodies being read or handled may hold at once, in `BODY_UNIT` permits: 64 MiB
/// of the 256 MB VM, charged as bytes arrive.
const BODY_UNIT: usize = 64 * 1024;
static BODY_BUDGET: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1024);
/// Units one IP's in-progress reads may hold together (4 MiB): no single client takes a large
/// slice of `BODY_BUDGET`.
const BODY_UNITS_PER_IP: u32 = 64;
/// After this long, a body must keep arriving at `BODY_MIN_RATE` bytes/s or be cut off.
const BODY_RATE_GRACE: Duration = Duration::from_secs(2);
const BODY_MIN_RATE: u64 = 32 * 1024;
/// Body reads one client IP may have in progress at once.
const BODY_READS_PER_IP: u32 = 4;
/// Per IP: (reads in progress, units those reads have charged).
static BODY_READS_BY_IP: std::sync::LazyLock<Mutex<HashMap<String, (u32, u32)>>> = std::sync::LazyLock::new(Default::default);

/// One in-progress body read, counted (with the units it charged) against its IP until dropped.
struct BodyRead {
    ip: String,
    units: std::cell::Cell<u32>,
}

impl BodyRead {
    fn begin(ip: String) -> Option<BodyRead> {
        let mut m = BODY_READS_BY_IP.held();
        let e = m.entry(ip.clone()).or_insert((0, 0));
        if e.0 >= BODY_READS_PER_IP {
            return None;
        }
        e.0 += 1;
        Some(BodyRead { ip, units: std::cell::Cell::new(0) })
    }

    /// Count `n` more units against this IP; false when that would pass `BODY_UNITS_PER_IP`.
    fn charge(&self, n: u32) -> bool {
        let mut m = BODY_READS_BY_IP.held();
        let Some(e) = m.get_mut(&self.ip) else { return false };
        if e.1.saturating_add(n) > BODY_UNITS_PER_IP {
            return false;
        }
        e.1 += n;
        self.units.set(self.units.get() + n);
        true
    }
}

impl Drop for BodyRead {
    fn drop(&mut self) {
        let mut m = BODY_READS_BY_IP.held();
        if let Some(e) = m.get_mut(&self.ip) {
            e.0 = e.0.saturating_sub(1);
            e.1 = e.1.saturating_sub(self.units.get());
            if e.0 == 0 {
                m.remove(&self.ip);
            }
        }
    }
}

async fn request_timeout(req: Request, next: Next) -> Response {
    match tokio::time::timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS), next.run(req)).await {
        Ok(resp) => resp,
        Err(_) => (StatusCode::GATEWAY_TIMEOUT, "request timed out").into_response(),
    }
}

/// Global in-flight cap. Once `MAX_INFLIGHT` requests are already running, the next is SHED
/// with 503 rather than queued, so a flood cannot pile unbounded futures onto the single
/// machine. This is the OOM guard for the 256MB VM (`fly.toml`): the shed keeps concurrent
/// 2MB-body requests well under RAM. A 503 is recoverable — the client retries/auto-reverts
/// exactly as it does for a rejected save. Sized conservatively for the current box; raise it
/// together with the VM's memory. Skips /health (Fly's probe must never be shed).
const MAX_INFLIGHT: usize = 64;

async fn concurrency_limit(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    if path == "/health" {
        return next.run(req).await;
    }
    let Ok(_permit) = st.inflight.clone().try_acquire_owned() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "server busy").into_response();
    };
    next.run(req).await
}

#[derive(serde::Deserialize)]
struct AuthReq {
    name: String,
}

#[derive(serde::Serialize)]
struct AuthResp {
    token: String,
    account_id: u64,
    name: String,
    /// Decimal STRING, not a number: the seed exceeds 2^53, so a JSON number loses
    /// precision in the client's float-based parser — which corrupted the seed, so the
    /// client's genesis never matched the server's and NO online save could anchor.
    seed: String,
    /// The server's sim version, so the client can detect a version SKEW at LOGIN (before
    /// playing) instead of only when a save is rejected mid-session.
    sim_version: String,
    /// The catalog version and sim source hash the server replays with (audit 2026-10-03): a skew
    /// in either rejects every save, and the login is the moment to say so.
    game_data_version: String,
    sim_build: String,
}

/// PLACEHOLDER login: any name → a fresh account + session token + server seed.
/// Real Steam login swaps this handler (verify the app ticket, resolve the steam
/// id → account); everything downstream (token, seed) is identical.
async fn auth_dev(State(st): State<AppState>, Json(req): Json<AuthReq>) -> (StatusCode, Json<serde_json::Value>) {
    match login_checked(&st, req.name).await {
        Ok(auth) => (StatusCode::OK, Json(serde_json::to_value(auth).expect("AuthResp is a plain struct"))),
        Err((code, why)) => (code, Json(serde_json::json!({"_error": why}))),
    }
}

/// Shared account resolve + session mint, keyed by a stable identity string: dev = the typed name,
/// Steam = "steam:<steamid>". Everything downstream (token/seed/single-session/taint) is identical.
#[cfg_attr(not(test), allow(dead_code))]
async fn login_as(st: &AppState, identity: String) -> Option<AuthResp> {
    let account = resolve_account(st, identity).await?;
    mint_session(st, account).await.ok()
}

/// The login the HTTP doors use: resolve the identity, refuse a banned account BEFORE a session is
/// minted (minting would evict the account's live token), then mint. `Err` carries the
/// player-facing reason.
/// `Err((status, reason))`: 403 for a ban, 503 when the database could not answer — a client must
/// be able to tell "retry" from "you are banned" (audit 2026-10-03).
async fn login_checked(st: &AppState, identity: String) -> Result<AuthResp, (StatusCode, String)> {
    let Some(account) = resolve_account(st, identity).await else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "sign-in is unavailable right now — try again".to_string()));
    };
    mint_session(st, account).await
}

/// Replay ticks one account may spend per `REPLAY_WINDOW_MS`: three full-span saves. An honest
/// client replays ~6,000 ticks per 10 minutes of play, plus the odd long reconnect.
const REPLAY_BUDGET_TICKS: u64 = 3 * MAX_REPLAY_SPAN;
const REPLAY_WINDOW_MS: i64 = 10 * 60 * 1000;

/// Charge `span` replay ticks to `account_id`; false (and nothing charged) when over budget.
fn charge_replay_budget(st: &AppState, account_id: u64, span: u64, now: i64) -> bool {
    let mut m = st.replay_budget.held();
    let e = m.entry(account_id).or_insert((now, 0));
    if now - e.0 >= REPLAY_WINDOW_MS {
        *e = (now, 0);
    }
    if e.1.saturating_add(span) > REPLAY_BUDGET_TICKS {
        return false;
    }
    e.1 += span;
    true
}

/// Give back a save's replay charge when the SERVER failed it (busy, persistence, a lost
/// compare-and-set): an honest client re-sends every 10 s through an outage, and each attempt
/// used to burn budget until it was locked out (audit 2026-10-03).
fn refund_replay_budget(st: &AppState, account_id: u64, span: u64) {
    if let Some(e) = st.replay_budget.held().get_mut(&account_id) {
        e.1 = e.1.saturating_sub(span);
    }
}

/// `refund_replay_budget`, then answer `resp`.
fn refunded<T>(st: &AppState, account_id: u64, span: u64, resp: T) -> T {
    refund_replay_budget(st, account_id, span);
    resp
}

/// Concurrent replays: the machine's cores (a replay is pure CPU), at least one.
fn replay_parallelism() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get()).max(1)
}

/// Run a replay (or other CPU-bound check) on the blocking pool while holding a `replay_slots`
/// permit for its WHOLE run. `None` when the semaphore is closed or the task died.
async fn run_replay<T: Send + 'static>(st: &AppState, f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let permit = st.replay_slots.clone().acquire_owned().await.ok()?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        f()
    }).await.ok()
}

/// THE FIRST LOCK of every per-account money transaction (audit 2026-10-03): the account row,
/// FOR UPDATE. Handlers then touched wallet / bound items / commander / monthly / BP rows in
/// different orders, and two claims of one account at once deadlocked (one aborted with a 500).
/// One shared first lock serialises them instead. An error aborts the transaction, so every
/// later statement in it fails and the handler reports that.
async fn lock_account(tx: &mut sqlx::PgConnection, account_id: i64) {
    let _ = sqlx::query("SELECT 1 FROM accounts WHERE account_id = $1 FOR UPDATE")
        .bind(account_id).execute(tx).await;
}

/// Advisory-lock key that serialises one account's logins, bans and evictions.
fn session_lock_key(account_id: u64) -> i64 {
    (0x5345_5353_i64 << 32) | (account_id as i64 & 0xffff_ffff)
}

/// Make `token` (or nothing) the account's ONE live session in memory, under both maps' locks at
/// once (active, then by_token — the order every caller takes them in). Taken one map at a time,
/// two concurrent logins each removed the other's not-yet-inserted token and both stayed valid.
fn swap_session(sessions: &Sessions, account_id: u64, next: Option<(String, Account)>) {
    let mut active = sessions.active.held();
    let mut by_token = sessions.by_token.held();
    if let Some(old) = active.remove(&account_id) {
        by_token.remove(&old);
    }
    if let Some((token, account)) = next {
        active.insert(account_id, token.clone());
        by_token.insert(token, account);
    }
}

/// The player-facing reason for a ban row at `now`, or `None` when it has expired.
fn ban_reason(until: Option<i64>, reason: &str, now: i64) -> Option<String> {
    match until {
        None => Some(format!("account suspended permanently: {reason}")),
        Some(u) if u > now => Some(format!("account suspended until {u} (unix ms): {reason}")),
        Some(_) => None,
    }
}

/// Identity → account (creating both on a miss). The first half of `login_as`.
/// `None` = the database could not answer (the caller refuses the login). It used to `.expect()`
/// on the insert — under `panic = "abort"` one DB blip on a first sign-in took the whole server
/// down — and to treat a failed lookup as "new identity" (audit 2026-10-02).
async fn resolve_account(st: &AppState, identity: String) -> Option<Account> {
    // AN IDENTITY, not an account name. The two were one thing until now: `steam:<steamid>` was
    // both how you signed in and what the account WAS, so an account could never gain a second way
    // in nor lose its first one. This resolves identity -> account and creates both on a miss.
    //
    // Note what has NOT changed: an unknown identity still mints a fresh account. Attaching a new
    // identity to an EXISTING account is a linking flow, and there is no second provider to link
    // yet — writing that flow now would be a path nothing can walk.
    let name = identity.clone();
    // Existing account? (in-memory working set — loaded from DB at startup). Scope
    // the lock so it's dropped before any await.
    let existing = st.sessions.by_identity.held().get(&identity).cloned();
    let account = match existing {
        Some(a) => a,
        None => {
            // THE DB IS ASKED BEFORE ANYTHING IS CREATED. The in-memory map is a working set, and
            // a miss in it does not mean the identity is new — it means this process has not seen
            // it. The old code leaned on `ON CONFLICT (name)` to make that harmless; resolving the
            // identity first says the same thing out loud and keeps the identity table
            // authoritative, which is what makes a second sign-in possible at all.
            let known: Option<(i64, String, i64)> = if let Some(db) = &st.db {
                match sqlx::query_as(
                    "SELECT a.account_id, a.name, a.seed FROM account_identities i \
                     JOIN accounts a ON a.account_id = i.account_id WHERE i.identity = $1",
                )
                .bind(&identity)
                .fetch_optional(db)
                .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("[AUTH] identity lookup failed: {e}");
                        return None;
                    }
                }
            } else {
                None
            };
            if let Some((id, acct_name, acct_seed)) = known {
                let a = Account { account_id: id as u64, name: acct_name, seed: acct_seed as u64 };
                st.sessions.by_identity.held().insert(identity.clone(), a.clone());
                st.trusted
                    .held()
                    .entry(a.account_id)
                    .or_insert_with(|| genesis_candidates(&st.data, a.seed));
                a
            } else {
            // New account. Seed is SERVER-random (not name-derived → no pick-your-seed).
            // DB assigns the account_id (BIGSERIAL); in-memory falls back to the
            // counter. Also anchor the genesis into `trusted`.
            let mut seed = random_seed();
            let account_id = if let Some(db) = &st.db {
                // RETURNING the STORED seed: on a name conflict the row already exists and its
                // seed is the account's, not the fresh random one.
                let row: (i64, i64) = match sqlx::query_as(
                    "INSERT INTO accounts (name, seed) VALUES ($1, $2) \
                     ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name RETURNING account_id, seed",
                )
                .bind(&name)
                .bind(seed as i64)
                .fetch_one(db)
                .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("[AUTH] account insert failed: {e}");
                        return None;
                    }
                };
                seed = row.1 as u64;
                // The identity that OPENED the account. `DO NOTHING` rather than an upsert: if two
                // logins race, the first one's mapping is the true one and the second must not
                // re-point an identity at a different account.
                let _ = sqlx::query(
                    "INSERT INTO account_identities (identity, account_id) VALUES ($1, $2) \
                     ON CONFLICT (identity) DO NOTHING",
                )
                .bind(&identity)
                .bind(row.0)
                .execute(db)
                .await;
                for genesis in genesis_candidates(&st.data, seed) {
                    let _ = sqlx::query("INSERT INTO trusted (account_id, state_hash) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                        .bind(row.0)
                        .bind(&genesis)
                        .execute(db)
                        .await;
                }
                row.0 as u64
            } else {
                st.sessions.next_id.fetch_add(1, Ordering::SeqCst)
            };
            let a = Account { account_id, name: name.clone(), seed };
            st.sessions.by_identity.held().insert(identity.clone(), a.clone());
            st.trusted
                .held()
                .entry(account_id)
                .or_insert_with(|| genesis_candidates(&st.data, seed));
            a
            }
        }
    };
    Some(account)
}

/// Mint the account's single active session. The second half of `login_as`.
async fn mint_session(st: &AppState, account: Account) -> Result<AuthResp, (StatusCode, String)> {
    // Single active session: mint a UNIQUE token (random per login, NOT name-derived) and
    // drop the account's PREVIOUS token — an older session is superseded (its next request
    // 401s), so two devices can't run the same account at once (concurrent saves clobber +
    // would open economy dup). Steam auth gives this for free; the placeholder does it here.
    let token = blake3::hash(format!("{DEV_SECRET}:{}:{}", account.name, random_seed()).as_bytes())
        .to_hex()
        .to_string();
    // ONE critical section per account (audit 2026-10-03): the ban check, the durable session row
    // and the memory swap run under an advisory lock `admin_ban` also takes, so a ban cannot land
    // between the check and the mint, and two logins land in memory in the order they land in the
    // database (the last one wins in both). The row persists the session so a redeploy keeps it.
    if let Some(db) = &st.db {
        let fail = |e: sqlx::Error| {
            eprintln!("[AUTH] session mint failed for acct {}: {e}", account.account_id);
            (StatusCode::SERVICE_UNAVAILABLE, "sign-in is unavailable right now — try again".to_string())
        };
        let mut tx = db.begin().await.map_err(fail)?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)").bind(session_lock_key(account.account_id))
            .execute(&mut *tx).await.map_err(fail)?;
        let ban: Option<(Option<i64>, String)> = sqlx::query_as("SELECT until_ms, reason FROM account_bans WHERE account_id = $1")
            .bind(account.account_id as i64).fetch_optional(&mut *tx).await.map_err(fail)?;
        if let Some(why) = ban.and_then(|(until, reason)| ban_reason(until, &reason, now_ms())) {
            println!("login refused: acct={} is banned ({why})", account.account_id);
            return Err((StatusCode::FORBIDDEN, why));
        }
        sqlx::query(
            "INSERT INTO active_sessions (account_id, token) VALUES ($1, $2) \
             ON CONFLICT (account_id) DO UPDATE SET token = EXCLUDED.token",
        )
        .bind(account.account_id as i64)
        .bind(&token)
        .execute(&mut *tx)
        .await
        .map_err(fail)?;
        // The mint-coupon clock starts at the first login (user 2026-10-03); later logins keep it.
        sqlx::query("INSERT INTO mint_coupon_clock (account_id, anchor_ms) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(account.account_id as i64).bind(now_ms()).execute(&mut *tx).await.map_err(fail)?;
        swap_session(&st.sessions, account.account_id, Some((token.clone(), account.clone())));
        tx.commit().await.map_err(fail)?;
    } else {
        swap_session(&st.sessions, account.account_id, Some((token.clone(), account.clone())));
    }
    // Fresh login = a fresh session: clear any taint from a prior doomed session (it either
    // recovered here or was replaced), so this login isn't blocked from refining. Its tick-rate
    // anchor goes too: the session's GET /save sets a new one, and an old one (a boot anchor
    // especially) must not outlive the session it was made for.
    st.sessions.tainted.held().remove(&account.account_id);
    st.tick_ref.held().remove(&account.account_id);
    Ok(AuthResp {
        token,
        account_id: account.account_id,
        name: account.name,
        seed: account.seed.to_string(),
        sim_version: sim_core::VERSION.to_string(),
        game_data_version: st.data.version.clone(),
        sim_build: sim_core::SIM_BUILD.to_string(),
    })
}

// ==================== STORE — money in (docs/unbuilt-systems-architecture.md §5) ============

/// A random u64 from the OS. Only used for the order id, which needs to be unguessable but is
/// not a secret — the money guarantees all come from Steam's own authorization.
fn rand_u64() -> u64 {
    use rand::Rng;
    rand::thread_rng().gen()
}

/// SANDBOX BY DEFAULT. `ISteamMicroTxnSandbox` charges nothing and is the only endpoint safe to
/// point at while the flow is being built. Real money requires setting `STEAM_MTX_LIVE=1`
/// deliberately — and `docs/economy-invariants.md` gates that on I1-I10 being green, so the
/// default is not a placeholder to be tidied up later, it is the safety interlock.
fn steam_mtx_iface() -> &'static str {
    if steam_mtx_live() { "ISteamMicroTxn" } else { "ISteamMicroTxnSandbox" }
}

/// `STEAM_MTX_LIVE=1`: purchases charge real money.
fn steam_mtx_live() -> bool {
    static LIVE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *LIVE.get_or_init(|| std::env::var("STEAM_MTX_LIVE").ok().as_deref() == Some("1"))
}

/// Why this configuration must not serve, or `None`. Real money (`STEAM_MTX_LIVE=1`) requires
/// `PROD=1` (test fixtures unmounted) and no `DEV_KEY` left set — forgetting either is the launch
/// mistake this makes impossible rather than merely documented.
fn live_money_misconfig(money_live: bool, prod: bool, dev_key_set: bool) -> Option<String> {
    if !money_live {
        return None;
    }
    if !prod {
        return Some("STEAM_MTX_LIVE=1 (real money) without PROD=1 — the test fixtures, including the \
                     /auth/dev account door, would be served. Set PROD=1 and ADMIN_KEY first."
            .into());
    }
    if dev_key_set {
        return Some("STEAM_MTX_LIVE=1 with DEV_KEY still set — unset it (fly secrets unset DEV_KEY); ops \
                     routes use ADMIN_KEY in production."
            .into());
    }
    None
}

/// Call one Steam MicroTxn method. Form-encoded POST, JSON back, 10s ceiling so a hung Steam
/// cannot wedge a purchase handler.
async fn steam_mtx(method: &str, params: &[(&str, String)]) -> Result<serde_json::Value, String> {
    steam_mtx_verdict(method, params).await.map_err(|e| e.msg)
}

/// Why a MicroTxn call did not return OK. `definite`: Steam answered no, or nothing was sent —
/// the call certainly had no effect. Otherwise (a timeout or a broken reply after the request
/// left) Steam may have acted, and only `QueryTxn` can say.
struct MtxError {
    definite: bool,
    msg: String,
}

async fn steam_mtx_verdict(method: &str, params: &[(&str, String)]) -> Result<serde_json::Value, MtxError> {
    let sure = |msg: String| MtxError { definite: true, msg };
    let key = std::env::var("STEAM_PUBLISHER_KEY").map_err(|_| sure("STEAM_PUBLISHER_KEY unset".to_string()))?;
    let url = format!("https://partner.steam-api.com/{}/{method}/v1/", steam_mtx_iface());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| sure(e.to_string()))?;
    let mut form: Vec<(&str, String)> = vec![("key", key), ("appid", STEAM_APP_ID.to_string())];
    form.extend(params.iter().cloned());
    let resp = client.post(&url).form(&form).send().await
        .map_err(|e| MtxError { definite: e.is_connect(), msg: e.without_url().to_string() })?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await
        .map_err(|e| MtxError { definite: false, msg: e.without_url().to_string() })?;
    // Steam reports failure INSIDE a 200 body, so the HTTP status is not the verdict — but only an
    // explicit `result` is one (audit 2026-10-03): a 5xx or a body without it says nothing about
    // whether the call took effect.
    match body.pointer("/response/result").and_then(|v| v.as_str()) {
        Some("OK") => Ok(body),
        Some(_) if status.is_success() => {
            let msg = body.pointer("/response/error/errordesc").and_then(|v| v.as_str()).unwrap_or("unknown");
            Err(sure(format!("{method}: {msg}")))
        }
        _ => Err(MtxError { definite: false, msg: format!("{method}: HTTP {status}, no result") }),
    }
}

/// Steam's word for what happened to one transaction. Only the values that mean MONEY WENT BACK
/// are actionable; everything else is either fine or still in flight.
///
/// `PartialRefund` is deliberately NOT actionable. Revoking a whole grant because part of it was
/// returned takes more from the player than Steam gave back, and there is no partial revocation to
/// reach for — the grant is one number of crystal, some of which are probably spent. It is logged so
/// a human can settle it, which is the right owner for a case that needs a judgement.
fn refund_means_revoke(status: &str) -> bool {
    matches!(status.to_ascii_lowercase().as_str(), "refunded" | "chargeback" | "reversed")
}

/// Ask Steam what became of ONE transaction.
///
/// ⚠️ UNVERIFIED AGAINST LIVE STEAM. The interface version below is the one the Steamworks docs
/// carried when this was written and nothing here has spoken to a real MicroTxn endpoint — the app
/// is still on the sandbox. A wrong version answers with an error, which this reports loudly and
/// hourly rather than swallowing: a reconcile that fails silently is indistinguishable from one
/// that found nothing, and that is the one outcome this must never look like. Confirm the version
/// before `STEAM_MTX_LIVE=1`.
async fn steam_txn_status(transid: &str) -> Result<String, String> {
    let key = std::env::var("STEAM_PUBLISHER_KEY").map_err(|_| "STEAM_PUBLISHER_KEY unset".to_string())?;
    let url = format!("https://partner.steam-api.com/{}/QueryTxn/v3/", steam_mtx_iface());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(&url)
        .query(&[("key", key.as_str()), ("appid", &STEAM_APP_ID.to_string()), ("transid", transid)])
        .send()
        .await
        // `without_url`: a reqwest error's Display appends the URL, and this one carries the
        // publisher key in its query (audit 2026-10-02).
        .map_err(|e| e.without_url().to_string())?;
    let body: serde_json::Value = resp.json().await.map_err(|e| e.without_url().to_string())?;
    // Steam reports failure INSIDE a 200 body, the same as the POST surface.
    if let Some(desc) = body.pointer("/response/error/errordesc").and_then(|v| v.as_str()) {
        return Err(format!("QueryTxn {transid}: {desc}"));
    }
    body.pointer("/response/params/status")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("QueryTxn {transid}: no status in the answer"))
}

/// THE HALF OF A REFUND THAT NOBODY WAS DOING. `store_refund` takes an order back correctly and
/// always did; nothing ever CALLED it except a person with the dev key. Steam refunds happen on
/// Valve's side — a player asks, or a bank charges back — and this server was never told, so the
/// crystal stayed granted and paid for nothing. In a game whose currency is bought, that is not an
/// edge case, it is the ordinary end of a fraction of every day's purchases.
///
/// DRIVEN FROM OUR ORDERS, NOT FROM STEAM'S REPORT. `GetReport` is the endpoint built for this and
/// it is the right answer at volume — one call covers a window instead of one call per order. It
/// is not the right answer FIRST: it returns everything the app ever sold, in a shape that has to
/// be matched back to rows we own, and every part of that is guesswork until someone has seen a
/// real payload. Asking about orders we granted is bounded by our own table, needs no time-window
/// formatting (there is no date crate here), and is verifiable one row at a time. Swap it for
/// `GetReport` when the per-order call rate starts to matter — the revocation underneath does not
/// change.
///
/// The window is 30 days because Steam's own refund window is 14 and a chargeback can arrive later;
/// past that an order is settled and re-asking about it forever is pure cost.
async fn reconcile_steam_refunds(st: &AppState) {
    let Some(db) = &st.db else { return };
    if std::env::var("STEAM_PUBLISHER_KEY").is_err() {
        return;  // not a deployment that can be paid at all
    }
    let rows: Vec<(i64, i64, String, i64, i64)> = match sqlx::query_as(
        // Only orders that took money, least recently checked first (audit 2026-10-03): newest
        // 500 of ANY status meant abandoned checkouts crowded out a paid order refunded on day 10,
        // and it was never revoked. Every order checked is stamped, so the whole set rotates.
        "SELECT order_id, account_id, transid, granted_paid, granted_bound \
         FROM store_orders \
         WHERE status IN ('granted', 'charged', 'grant_failed', 'charging') \
           AND transid IS NOT NULL AND transid <> '' \
           AND created_at > now() - interval '30 days' \
         ORDER BY refund_checked_at NULLS FIRST, created_at LIMIT 500",
    )
    .fetch_all(db)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[REFUND] could not list open orders: {e}");
            return;
        }
    };
    let (mut checked, mut revoked, mut failed) = (0u32, 0u32, 0u32);
    for (order_id, acct_id, transid, gt, gb) in rows {
        checked += 1;
        let _ = sqlx::query("UPDATE store_orders SET refund_checked_at = now() WHERE order_id = $1")
            .bind(order_id).execute(db).await;
        let status = match steam_txn_status(&transid).await {
            Ok(s) => s,
            Err(e) => {
                failed += 1;
                // ONE line per failure, not a swallowed error. If the interface version above is
                // wrong every order fails and this is the only thing that will say so.
                eprintln!("[REFUND] order {order_id}: {e}");
                continue;
            }
        };
        if status.eq_ignore_ascii_case("partialrefund") {
            eprintln!("[REFUND] order {order_id} (acct {acct_id}) is a PARTIAL refund — \
                       needs a human: revoking {gt}t/{gb}b would take back more than Steam returned");
            continue;
        }
        if !refund_means_revoke(&status) {
            continue;
        }
        match revoke_order(db, order_id, acct_id, gt, gb).await {
            Ok(true) => {
                revoked += 1;
                flag_cheat(st, acct_id as u64, "refund", 0).await;
                println!("[REFUND] steam says {status} — revoked order={order_id} acct={acct_id} -{gt}t/-{gb}b");
            }
            Ok(false) => {}  // somebody got there first; both callers may race
            Err(()) => {
                failed += 1;
                eprintln!("[REFUND] order {order_id}: revocation failed to persist — will retry next sweep");
            }
        }
    }
    if revoked > 0 || failed > 0 {
        println!("[REFUND] sweep: {checked} checked, {revoked} revoked, {failed} could not be asked");
    }
}

/// Which currency Steam will bill this user in. `GetUserInfo` is a GET, unlike the rest of the
/// MicroTxn surface — worth stating because sending it as a POST returns "Method Not Allowed"
/// rather than anything that looks like a currency problem.
///
/// A failure here is NOT fatal: we fall back to the base currency, which is a real price in a
/// real currency. Better a charge Steam might decline than a purchase blocked because a lookup
/// was slow.
async fn steam_user_currency(steamid: u64) -> Option<String> {
    let key = std::env::var("STEAM_PUBLISHER_KEY").ok()?;
    let url = format!("https://partner.steam-api.com/{}/GetUserInfo/v2/", steam_mtx_iface());
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(5)).build().ok()?;
    let body: serde_json::Value = client
        .get(&url)
        .query(&[("key", key), ("appid", STEAM_APP_ID.to_string()), ("steamid", steamid.to_string())])
        .send().await.ok()?
        .json().await.ok()?;
    body.pointer("/response/params/currency").and_then(|v| v.as_str()).map(str::to_string)
}

/// The numeric steamid behind an account, or None for a dev login. Accounts are named
/// `steam:<id>` by `/auth/steam`; a purchase is only meaningful for those.
fn steamid_of(acct: &Account) -> Option<u64> {
    acct.name.strip_prefix("steam:").and_then(|s| s.parse().ok())
}

#[derive(serde::Deserialize)]
struct CheckoutReq {
    sku: String,
    idem_key: String,
}

/// `GET /store` — the catalog the storefront renders. Public: prices are not a secret, and the
/// client needs them before anyone is logged in.
async fn store_catalog(State(st): State<AppState>) -> Json<serde_json::Value> {
    // No season running (or it has ended) → the Battle Pass is not on the shelf at all (user
    // 2026-10-02), so the client has nothing to draw a buy button for.
    let season_live = live_season(&st).is_some();
    Json(serde_json::json!({
        "ok": true,
        "sandbox": steam_mtx_iface().ends_with("Sandbox"),
        "skus": store::CATALOG.iter()
            .filter(|s| season_live || s.section != store::Section::BattlePass)
            .map(|s| serde_json::json!({
            "sku": s.id,
            "prices": s.prices.iter().map(|p| serde_json::json!({
                "currency": p.currency, "minor": p.minor,
            })).collect::<Vec<_>>(),
            "crystal": s.crystal,
            "first_buy_bonus": s.first_buy_bonus,
            "section": format!("{:?}", s.section).to_lowercase(),
            "bound_crystal": s.bound_crystal,
            "coupons": s.coupons,
            "items": s.items.iter().map(|&(def_id, qty)| serde_json::json!({
                "def_id": def_id, "qty": qty,
                "name": st.data.item(def_id).map_or(def_id, |d| d.name.as_str()),
            })).collect::<Vec<_>>(),
            "limit": s.limit,
            "commander_level": match s.gate { store::Gate::CommanderLevel(l) => l, store::Gate::Open => 0 },
            "monthly_days": if s.section == store::Section::Monthly { store::MONTHLY_DAYS } else { 0 },
            "monthly_daily_crystal": if s.section == store::Section::Monthly { store::MONTHLY_DAILY_CRYSTAL } else { 0 },
        })).collect::<Vec<_>>(),
    }))
}

/// `POST /store/checkout` — start a purchase. Records the order, asks Steam to open the overlay,
/// returns the ids the client needs to finalize. NOTHING is granted here.
///
/// `idem_key` is the client's retry handle: a repeat replays the stored order instead of opening a
/// second overlay for the same intent, which is the difference between a flaky network and a
/// double charge.
async fn store_checkout(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CheckoutReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "store needs a database"})));
    };
    let Some(sku) = store::sku(&req.sku) else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"ok": false, "reason": "unknown sku"})));
    };
    let Some(steamid) = steamid_of(&acct) else {
        return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": "purchases require a Steam login"})));
    };
    // Replay an existing order for this key rather than charging twice — FOR THIS ACCOUNT.
    // Without the account clause this handed the caller somebody else's order_id, Steam transid
    // and status, for a SKU it never compared. `store_finalize` twenty lines below checks
    // ownership and 403s, so the leak was here alone; the client's key happens to carry `randi()`,
    // which made it hard to hit rather than impossible.
    let prior: Option<(i64, Option<String>, String)> = sqlx::query_as(
        "SELECT order_id, transid, status FROM store_orders WHERE idem_key = $1 AND account_id = $2",
    ).bind(&req.idem_key).bind(acct.account_id as i64).fetch_optional(db).await.ok().flatten();
    if let Some((order_id, transid, status)) = prior {
        return (StatusCode::OK, Json(serde_json::json!({
            "ok": true, "reason": "idempotent",
            "order_id": order_id.to_string(), "transid": transid, "status": status,
        })));
    }
    // A NEW checkout of this SKU replaces this account's own unfinished one (audit 2026-10-03): a
    // closed overlay left an `init` / `pending` row that blocked a capped SKU for an hour. Steam
    // takes nothing until FinalizeTxn, and finalize refuses a `failed` order.
    let _ = sqlx::query("UPDATE store_orders SET status = 'failed' WHERE account_id = $1 AND sku = $2 AND status IN ('init', 'pending')")
        .bind(acct.account_id as i64).bind(&req.sku).execute(db).await;
    match purchase_refusal(&st, db, acct.account_id, sku).await {
        Ok(None) => {}
        Ok(Some(why)) => return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": why}))),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    }
    let season_id = match sku.section {
        store::Section::BattlePass => live_season(&st).map(|s| s.season_id),
        _ => None,
    };
    // Price in the buyer's OWN currency. Steam bills the wallet's currency, so quoting USD to a
    // THB wallet would either be declined or — far worse — read as 1.99 baht. The lookup decides
    // the number; it is not decoration.
    let currency = steam_user_currency(steamid).await.unwrap_or_else(|| store::BASE_CURRENCY.to_string());
    let price = store::price_for(sku, &currency);
    // Our own order id. Random rather than sequential so it carries no information about volume.
    let order_id: i64 = (rand_u64() >> 1) as i64;
    if sqlx::query(
        "INSERT INTO store_orders (order_id, account_id, sku, idem_key, price_cents, status, season_id) \
         VALUES ($1, $2, $3, $4, $5, 'init', $6)",
    )
    .bind(order_id).bind(acct.account_id as i64).bind(&req.sku).bind(&req.idem_key).bind(price.minor as i64).bind(season_id)
    .execute(db).await.is_err()
    {
        return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": "duplicate order"})));
    }
    // ONE-PER-X SKUs count orders still in flight, not only granted ones (audit 2026-10-02): the
    // refusal above reads granted entitlements, so two open checkouts both passed it and both
    // could be finalized. Counted AFTER our own insert, so two racing checkouts see each other
    // and the later one backs out.
    match orders_in_flight_over_cap(db, acct.account_id, sku, season_id).await {
        Ok(false) => {}
        Ok(true) => {
            let _ = sqlx::query("DELETE FROM store_orders WHERE order_id = $1 AND status = 'init'").bind(order_id).execute(db).await;
            return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": "a purchase of this pack is already in progress"})));
        }
        Err(e) => {
            let _ = sqlx::query("DELETE FROM store_orders WHERE order_id = $1 AND status = 'init'").bind(order_id).execute(db).await;
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
        }
    }
    let params = [
        ("orderid", order_id.to_string()),
        ("steamid", steamid.to_string()),
        ("itemcount", "1".to_string()),
        ("language", "en".to_string()),
        ("currency", price.currency.to_string()),
        ("itemid[0]", "1".to_string()),
        ("qty[0]", "1".to_string()),
        ("amount[0]", price.minor.to_string()),
        ("description[0]", match sku.section {
            store::Section::Crystal => format!("{} crystal", sku.crystal),
            _ => sku.id.to_string(),
        }),
    ];
    match steam_mtx("InitTxn", &params).await {
        Ok(body) => {
            let transid = body.pointer("/response/params/transid").and_then(|v| v.as_str()).unwrap_or("").to_string();
            // From `init` only (audit 2026-10-03): a newer checkout may already have retired it.
            let _ = sqlx::query("UPDATE store_orders SET transid = $1, status = 'pending' WHERE order_id = $2 AND status = 'init'")
                .bind(&transid).bind(order_id).execute(db).await;
            println!("store-checkout acct={} sku={} order={order_id} {} {}", acct.account_id, req.sku, price.minor, price.currency);
            (StatusCode::OK, Json(serde_json::json!({
                "ok": true, "reason": "initiated",
                "order_id": order_id.to_string(), "transid": transid, "status": "pending",
            })))
        }
        Err(e) => {
            let _ = sqlx::query("UPDATE store_orders SET status = 'failed' WHERE order_id = $1")
                .bind(order_id).execute(db).await;
            eprintln!("[STORE] InitTxn failed acct={} sku={}: {e}", acct.account_id, req.sku);
            (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"ok": false, "reason": "payment provider rejected the order"})))
        }
    }
}

#[derive(serde::Deserialize)]
struct FinalizeReq {
    order_id: String,
}

/// `POST /store/finalize` — the player approved the overlay; take the money and grant.
///
/// The client saying "they approved" is not evidence. `FinalizeTxn` is: Steam refuses it unless
/// the user actually authorized, so a forged call grants nothing. That is why this endpoint can
/// be client-triggered at all.
async fn store_finalize(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<FinalizeReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "store needs a database"})));
    };
    let Ok(order_id) = req.order_id.parse::<i64>() else {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"ok": false, "reason": "bad order id"})));
    };
    let row: Option<(i64, String, String, i64, i64)> = sqlx::query_as(
        "SELECT account_id, sku, status, granted_paid, granted_bound FROM store_orders WHERE order_id = $1",
    ).bind(order_id).fetch_optional(db).await.ok().flatten();
    let Some((owner, sku_id, status, gt, gb)) = row else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"ok": false, "reason": "no such order"})));
    };
    if owner != acct.account_id as i64 {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"ok": false, "reason": "not your order"})));
    }
    // Already granted → replay the same answer. The grant itself is guarded by the status check
    // inside the transaction below, so this is the fast path, not the safety net.
    if status == "granted" {
        return (StatusCode::OK, Json(serde_json::json!({
            "ok": true, "reason": "idempotent", "paid": gt.to_string(), "tradeable": gt.to_string(), "bound": gb.to_string(),
        })));
    }
    if store::sku(&sku_id).is_none() {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": "sku retired mid-order"})));
    }
    // A `failed` / `refunded` order can never be granted (`claim_order_for_grant`), so it must
    // never reach FinalizeTxn either (audit 2026-10-03): Steam would take the money for nothing.
    if status == "charging" {
        return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": "payment in progress — retry"})));
    }
    if !matches!(status.as_str(), "init" | "pending" | "charged" | "grant_failed") {
        return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": "that order is closed"})));
    }
    // `grant_failed` is a charged order parked by the sweep: the money is taken, never finalize
    // it again.
    let charged = matches!(status.as_str(), "charged" | "grant_failed");
    // A BATTLE PASS is sold for the CURRENT season only (user 2026-10-02). An order opened before
    // its season ended is refused here, BEFORE Steam takes the money — once charged it is always
    // delivered, so this is the last point a dead season can be refused cleanly.
    // THE CAP AGAIN, before the money (audit 2026-10-02): checkout counts only orders opened in the
    // last hour, so an authorized-but-unfinalized checkout older than that let a second one in.
    // Whatever else of this capped SKU is already paid for decides now.
    if !charged && !bp_order_season_is_current(&st, db, order_id).await {
        let _ = sqlx::query("UPDATE store_orders SET status = 'failed' WHERE order_id = $1 AND status IN ('init', 'pending')")
            .bind(order_id).execute(db).await;
        return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": "that season has ended"})));
    }
    // TAKE THE MONEY FIRST — unless an earlier attempt already did. A `charged` order was
    // finalized with Steam and only its grant failed; finalizing it again would be refused by
    // Steam (or worse, charge twice), so a retry goes straight to the grant.
    if !charged {
        // CLAIM the order before Steam is asked (audit 2026-10-03): read unlocked above, a checkout
        // of the same SKU during FinalizeTxn marked it `failed`, Steam took the money and the grant
        // then refused a `failed` order. `charging` is outside every state checkout or a second
        // finalize may touch; a crash inside it is settled by `sweep_charging_orders`.
        // The cap is counted and the order claimed in ONE transaction under a per-(account, SKU)
        // lock (audit 2026-10-03): counted on the pool first, two finalizes of two orders of one
        // capped SKU both saw zero and both charged. A `charging` order counts toward the cap.
        enum Claim { Ok, Capped, Changed }
        let claimed = async {
            let mut tx = db.begin().await?;
            sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
                .bind(format!("store-sku:{owner}:{sku_id}")).execute(&mut *tx).await?;
            if capped_sku_already_bought(&mut *tx, order_id, owner, &sku_id).await {
                sqlx::query("UPDATE store_orders SET status = 'failed' WHERE order_id = $1 AND status IN ('init', 'pending')")
                    .bind(order_id).execute(&mut *tx).await?;
                tx.commit().await?;
                return Ok::<Claim, sqlx::Error>(Claim::Capped);
            }
            let n = sqlx::query(
                "UPDATE store_orders SET status = 'charging', charging_at = now() WHERE order_id = $1 AND status IN ('init', 'pending')",
            ).bind(order_id).execute(&mut *tx).await?.rows_affected();
            tx.commit().await?;
            Ok(if n == 1 { Claim::Ok } else { Claim::Changed })
        }.await;
        match claimed {
            Ok(Claim::Ok) => {}
            Ok(Claim::Capped) => return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": "purchase limit reached"}))),
            Ok(Claim::Changed) => return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": "order changed — retry"}))),
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
        }
        if let Err(e) = steam_mtx_verdict("FinalizeTxn", &[("orderid", order_id.to_string())]).await {
            eprintln!("[STORE] FinalizeTxn failed order={order_id} (definite={}): {}", e.definite, e.msg);
            if !e.definite {
                // Steam may have charged (a timeout after the request left): the order stays
                // `charging` and `sweep_charging_orders` asks QueryTxn which side it is on.
                return (StatusCode::ACCEPTED, Json(serde_json::json!({"ok": false, "reason": "payment is being confirmed — your purchase will arrive shortly if it went through"})));
            }
            let _ = sqlx::query("UPDATE store_orders SET status = 'pending' WHERE order_id = $1 AND status = 'charging'")
                .bind(order_id).execute(db).await;
            return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"ok": false, "reason": "payment was not completed"})));
        }
        // DURABLE "the money is taken", written BEFORE the grant is attempted and outside its
        // transaction, so a failed grant leaves an order every recovery path can see and finish
        // (the client's retry, the sweep, an admin) instead of one that rolled back to `pending`
        // and could never be granted again. If THIS write fails the grant below still runs from
        // init/pending; only a failure of both needs the log line and a human.
        let _ = sqlx::query("UPDATE store_orders SET status = 'charged' WHERE order_id = $1 AND status = 'charging'")
            .bind(order_id).execute(db).await;
    }
    match grant_order(db, &st.data, order_id).await {
        Ok(GrantOutcome::Granted { first_buy, paid, bound }) => {
            println!("store-granted acct={} sku={sku_id} order={order_id} first_buy={first_buy} +{paid}t/+{bound}b", acct.account_id);
            (StatusCode::OK, Json(serde_json::json!({
                "ok": true, "reason": "granted", "first_buy": first_buy,
                "paid": paid.to_string(), "tradeable": paid.to_string(), "bound": bound.to_string(),
            })))
        }
        Ok(GrantOutcome::AlreadySettled) => (StatusCode::OK, Json(serde_json::json!({"ok": true, "reason": "idempotent"}))),
        Err(e) => {
            // Charged, not granted. The order sits at `charged`: the client's retry, the
            // `sweep_charged_orders` loop and `/admin/store/grant` all finish it.
            eprintln!("[STORE] CHARGED BUT NOT GRANTED order={order_id} acct={} sku={sku_id} ({e}) — left at 'charged' for the sweep", acct.account_id);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": "purchase recorded — your crystal will arrive shortly"})))
        }
    }
}

/// A refund can leave a crystal bucket NEGATIVE (the refunded crystal was already spent). While
/// either is, no crystal may be spent — only a purchase brings the account back to zero.
fn wallet_in_debt(paid: i64, bound: i64) -> bool {
    paid < 0 || bound < 0
}

/// Whether a capped SKU (a `limit`, or one Battle Pass per season) already has enough OTHER
/// orders that took money — charged, granted, parked or refunded — for `order_id` to exceed it.
/// A lookup error answers true: refusing a finalize costs a retry, granting past a cap costs value.
async fn capped_sku_already_bought<'e, E: sqlx::PgExecutor<'e>>(db: E, order_id: i64, account_id: i64, sku_id: &str) -> bool {
    let Some(sku) = store::sku(sku_id) else { return false };
    let cap = match (sku.limit, sku.section) {
        (Some(l), _) => i64::from(l),
        (None, store::Section::BattlePass) => 1,
        _ => return false,
    };
    sqlx::query_as::<_, (i64,)>(
        "SELECT count(*) FROM store_orders o WHERE o.account_id = $1 AND o.sku = $2 AND o.order_id <> $3 \
           AND (o.status IN ('charging', 'charged', 'grant_failed', 'granted') \
                OR (o.status = 'refunded' AND o.granted_at IS NOT NULL)) \
           AND (o.season_id IS NOT DISTINCT FROM (SELECT season_id FROM store_orders WHERE order_id = $3) \
                OR $4)",
    )
    .bind(account_id).bind(sku_id).bind(order_id).bind(sku.section != store::Section::BattlePass)
    .fetch_one(db).await
    .map_or(true, |(n,)| n >= cap)
}

/// True unless the order is a Battle Pass whose season is not the one running now. Any other SKU,
/// or a lookup error (a recovery path will retry), answers true.
async fn bp_order_season_is_current(st: &AppState, db: &sqlx::PgPool, order_id: i64) -> bool {
    let row: Option<(String, Option<i64>)> = sqlx::query_as("SELECT sku, season_id FROM store_orders WHERE order_id = $1")
        .bind(order_id).fetch_optional(db).await.ok().flatten();
    let Some((sku, season)) = row else { return true };
    if store::sku(&sku).map(|s| s.section) != Some(store::Section::BattlePass) {
        return true;
    }
    live_season(st).is_some_and(|cur| Some(cur.season_id) == season)
}

/// Why `account_id` may not buy `sku` right now, or `None` when it may.
///
/// Checked at CHECKOUT, before Steam is asked for anything. The grant does not re-check: by then
/// the money is taken, and a paid order is always delivered.
async fn purchase_refusal(
    st: &AppState,
    db: &sqlx::PgPool,
    account_id: u64,
    sku: &store::Sku,
) -> Result<Option<&'static str>, sqlx::Error> {
    if let Some(limit) = sku.limit {
        // Orders, not live entitlements: a REFUNDED purchase still used up the limit (user
        // 2026-10-02: "refund แล้วซื้อใหม่ไม่ได้"), and one in flight is about to.
        if orders_toward_cap(db, account_id, sku, None).await? >= i64::from(limit) {
            return Ok(Some("purchase limit reached"));
        }
    }
    if let store::Gate::CommanderLevel(need) = sku.gate {
        let cxp: Option<(i64,)> = sqlx::query_as("SELECT cxp FROM commander WHERE account_id = $1")
            .bind(account_id as i64).fetch_optional(db).await?;
        if commander::level_for_cxp(&st.commander_levels, cxp.map_or(0, |r| r.0)) < need {
            return Ok(Some("commander level too low"));
        }
    }
    match sku.section {
        store::Section::Monthly => {
            let today = daily::day_of_ms(now_ms() as u64);
            let ends: Option<(i64,)> = sqlx::query_as("SELECT ends_day FROM monthly_card WHERE account_id = $1")
                .bind(account_id as i64).fetch_optional(db).await?;
            if ends.map_or(0, |r| r.0) - today > store::MONTHLY_MAX_BANKED_DAYS {
                return Ok(Some("monthly card already banked"));
            }
        }
        store::Section::BattlePass => {
            // A season past its end is no season (user 2026-10-02: no Battle Pass on sale then).
            let Some(season) = live_season(st) else {
                return Ok(Some("no active season"));
            };
            if bp_premium_owned(db, account_id, season.season_id).await?
                || orders_toward_cap(db, account_id, sku, Some(season.season_id)).await? >= 1
            {
                return Ok(Some("battle pass already owned"));
            }
        }
        store::Section::Crystal | store::Section::Pack => {}
    }
    Ok(None)
}

/// Orders of `sku` that use up its cap: charged, granted or REFUNDED (a refund does not give the
/// purchase back — user 2026-10-02; only one that was GRANTED: an order Steam reversed before we
/// delivered it bought nothing), plus a checkout opened in the last hour (older `init` /
/// `pending` rows are abandoned). `season_id` scopes a Battle Pass to its season.
async fn orders_toward_cap(
    db: &sqlx::PgPool,
    account_id: u64,
    sku: &store::Sku,
    season_id: Option<i64>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_as::<_, (i64,)>(
        "SELECT count(*) FROM store_orders WHERE account_id = $1 AND sku = $2 \
           AND ($3::BIGINT IS NULL OR season_id = $3) \
           AND (status IN ('charging', 'charged', 'grant_failed', 'granted') \
                OR (status = 'refunded' AND granted_at IS NOT NULL) \
                OR (status IN ('init', 'pending') AND created_at > now() - interval '1 hour'))",
    ).bind(account_id as i64).bind(sku.id).bind(season_id).fetch_one(db).await.map(|(n,)| n)
}

/// Whether this account's orders of a capped SKU (a `limit`, or the Battle Pass for one season)
/// now EXCEED the cap — checked after the new order is inserted, so it counts itself.
async fn orders_in_flight_over_cap(
    db: &sqlx::PgPool,
    account_id: u64,
    sku: &store::Sku,
    season_id: Option<i64>,
) -> Result<bool, sqlx::Error> {
    let cap = match (sku.limit, sku.section) {
        (Some(l), _) => i64::from(l),
        (None, store::Section::BattlePass) => 1,
        _ => return Ok(false),
    };
    Ok(orders_toward_cap(db, account_id, sku, season_id).await? > cap)
}

/// Deliver everything a SKU grants beyond the crystal wallet, inside the grant's transaction.
/// Any failure fails the whole grant, which leaves the order `charged` for the sweep.
async fn grant_contents(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    data: &GameData,
    account_id: i64,
    order_id: i64,
    sku: &store::Sku,
    season_id: Option<i64>,
) -> Result<(), String> {
    let order_ref = order_id.to_string();
    if sku.coupons > 0 {
        sqlx::query(
            "INSERT INTO bound_items (account_id, kind, qty) VALUES ($1, $2, $3) \
             ON CONFLICT (account_id, kind) DO UPDATE SET qty = bound_items.qty + EXCLUDED.qty",
        )
        .bind(account_id).bind(gacha::STANDARD_COUPON_KIND).bind(i64::from(sku.coupons))
        .execute(&mut **tx).await.map_err(|e| e.to_string())?;
        sqlx::query("INSERT INTO bound_ledger (account_id, kind, delta, reason, ref) VALUES ($1, $2, $3, 'purchase', $4)")
            .bind(account_id).bind(gacha::STANDARD_COUPON_KIND).bind(i64::from(sku.coupons)).bind(&order_ref)
            .execute(&mut **tx).await.map_err(|e| e.to_string())?;
    }
    for &(def_id, qty) in sku.items {
        let def = data.item(def_id).ok_or_else(|| format!("pack item {def_id} is not in the catalog"))?;
        let mut item = data::instantiate_item(def);
        item.quantity = qty.max(1);
        item.bound = true;
        mailbox_store::send_item(&mut **tx, account_id as u64, mailbox::MailKind::Reward, &item, &store_mail_reason(order_id))
            .await.map_err(|e| e.to_string())?;
    }
    match sku.section {
        store::Section::Monthly => {
            let today = daily::day_of_ms(now_ms() as u64);
            let (to,): (i64,) = sqlx::query_as(
                "INSERT INTO monthly_card (account_id, ends_day) VALUES ($1, $2 + $3) \
                 ON CONFLICT (account_id) DO UPDATE SET ends_day = GREATEST(monthly_card.ends_day, $2) + $3 \
                 RETURNING ends_day",
            )
            .bind(account_id).bind(today).bind(store::MONTHLY_DAYS)
            .fetch_one(&mut **tx).await.map_err(|e| e.to_string())?;
            sqlx::query("UPDATE store_orders SET monthly_from_day = $2, monthly_to_day = $3 WHERE order_id = $1")
                .bind(order_id).bind(to - store::MONTHLY_DAYS).bind(to)
                .execute(&mut **tx).await.map_err(|e| e.to_string())?;
        }
        store::Section::BattlePass => {
            let season_id = season_id.ok_or("battle pass order carries no season")?;
            sqlx::query(
                "INSERT INTO bp_entitlements (account_id, season_id, tier, source, txn_id) \
                 VALUES ($1, $2, 'premium', 'store', $3)",
            )
            .bind(account_id).bind(season_id).bind(&order_ref)
            .execute(&mut **tx).await.map_err(|e| e.to_string())?;
        }
        store::Section::Crystal | store::Section::Pack => {}
    }
    Ok(())
}

/// The mail `reason` a purchase's items carry — what a refund finds them by.
fn store_mail_reason(order_id: i64) -> String {
    format!("store:{order_id}")
}

/// What `grant_order` did.
#[derive(Debug, PartialEq, Eq)]
enum GrantOutcome {
    Granted { first_buy: bool, paid: u64, bound: u64 },
    /// Already granted / refunded / failed — nothing to do (idempotent).
    AlreadySettled,
}

/// GRANT a paid order's crystal, in ONE transaction: lock the account (the first-buy bonus is a
/// once-per-SKU rule), decide first-buy, flip the order to `granted` from an unsettled state, and
/// credit wallet + ledger + entitlement. The one grant path — `/store/finalize`, the
/// `sweep_charged_orders` loop and `/admin/store/grant` all call it, so there is no second copy of
/// the bonus rule to drift. Callers must only reach it for an order Steam has charged.
async fn grant_order(db: &sqlx::PgPool, data: &GameData, order_id: i64) -> Result<GrantOutcome, String> {
    let row: Option<(i64, String, Option<i64>)> = sqlx::query_as("SELECT account_id, sku, season_id FROM store_orders WHERE order_id = $1")
        .bind(order_id).fetch_optional(db).await.map_err(|e| e.to_string())?;
    let Some((account_id, sku_id, season_id)) = row else { return Err("no such order".into()) };
    let sku = store::sku(&sku_id).ok_or_else(|| format!("sku {sku_id} retired"))?;
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    // SERIALISE THIS ACCOUNT for the rest of the transaction. The first-buy bonus is a
    // once-per-SKU rule, and the read that decides it used to run on `db` BEFORE the transaction
    // even began — so two finalizes of two DIFFERENT orders of the same SKU both saw zero
    // entitlements and both paid the bonus. Locking the account row matches the SCOPE of the rule
    // exactly (per account) and needs no migration.
    sqlx::query("SELECT 1 FROM accounts WHERE account_id = $1 FOR UPDATE")
        .bind(account_id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    // Through `owns_sku`, the ONE place that knows a revoked entitlement is not owned. An error
    // FAILS the grant rather than guessing "not owned" and paying the bonus twice.
    let first_buy = !owns_sku(&mut *tx, account_id as u64, &sku_id).await.map_err(|e| e.to_string())?;
    let grant = store::grant_for(sku, first_buy);
    // From an UNSETTLED state only — an allow-list (`claim_order_for_grant`), so a refunded or
    // failed order can never be granted again.
    if !claim_order_for_grant(&mut tx, order_id, first_buy, grant.paid as i64, grant.bound as i64).await {
        return Ok(GrantOutcome::AlreadySettled);
    }
    sqlx::query("INSERT INTO crystal_wallet (account_id, paid, bound) VALUES ($1, $2, $3) \
                 ON CONFLICT (account_id) DO UPDATE SET paid = crystal_wallet.paid + EXCLUDED.paid, bound = crystal_wallet.bound + EXCLUDED.bound")
        .bind(account_id).bind(grant.paid as i64).bind(grant.bound as i64)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    sqlx::query("INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES ($1, $2, $3, 'purchase', $4)")
        .bind(account_id).bind(grant.paid as i64).bind(grant.bound as i64).bind(order_id.to_string())
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    sqlx::query("INSERT INTO entitlements (account_id, sku, order_id) VALUES ($1, $2, $3)")
        .bind(account_id).bind(&sku_id).bind(order_id)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    grant_contents(&mut tx, data, account_id, order_id, sku, season_id).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(GrantOutcome::Granted { first_buy, paid: grant.paid, bound: grant.bound })
}

/// Finish every order Steam charged but the grant did not complete (`status = 'charged'`). Runs on
/// a timer so a player who closed the game after a failed grant is still paid without retrying.
async fn sweep_charged_orders(db: &sqlx::PgPool, data: &GameData) {
    // A charged order still failing a day after it was placed is not a blip: park it as
    // `grant_failed` (admin grant still finishes it) so it cannot fill every sweep's batch and
    // starve the paid orders behind it (audit 2026-10-02). A day, not N tries: the sweep runs every
    // five minutes, so a count would park a good order during a half-hour database outage.
    match sqlx::query(
        "UPDATE store_orders SET status = 'grant_failed' \
         WHERE status = 'charged' AND grant_attempts > 0 AND created_at < now() - interval '24 hours' \
         RETURNING order_id",
    ).fetch_all(db).await {
        Ok(rows) => {
            for r in rows {
                let id: i64 = sqlx::Row::try_get(&r, 0).unwrap_or(0);
                eprintln!("[STORE] ALERT order={id} charged but ungrantable for 24h — parked as 'grant_failed'; finish it with /admin/store/grant or refund it");
            }
        }
        Err(e) => eprintln!("[STORE] could not park failing orders: {e}"),
    }
    // Fewest attempts first, so the orders that keep failing never crowd the fresh ones out.
    let orders: Vec<(i64,)> = match sqlx::query_as(
        "SELECT order_id FROM store_orders WHERE status = 'charged' ORDER BY grant_attempts, order_id LIMIT 100",
    )
        .fetch_all(db).await
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("[STORE] charged-order sweep could not read orders: {e}");
            return;
        }
    };
    for (order_id,) in orders {
        match grant_order(db, data, order_id).await {
            Ok(GrantOutcome::Granted { paid, bound, .. }) => println!("[STORE] sweep granted order={order_id} +{paid}t/+{bound}b"),
            Ok(GrantOutcome::AlreadySettled) => {}
            Err(e) => {
                eprintln!("[STORE] sweep could not grant order={order_id}: {e} — will retry");
                let _ = sqlx::query("UPDATE store_orders SET grant_attempts = grant_attempts + 1 WHERE order_id = $1")
                    .bind(order_id).execute(db).await;
            }
        }
    }
}

/// Orders left `charging` by a process that died inside FinalizeTxn: Steam says which side of the
/// charge they are on. Succeeded → `charged` (the sweep then grants it); never finalized → back to
/// `pending`; anything else stays and is logged for a person.
async fn charging_for_long(db: &sqlx::PgPool, order_id: i64) -> bool {
    sqlx::query_as::<_, (bool,)>("SELECT charging_at < now() - interval '1 hour' FROM store_orders WHERE order_id = $1")
        .bind(order_id).fetch_optional(db).await.ok().flatten().is_some_and(|(b,)| b)
}

async fn sweep_charging_orders(db: &sqlx::PgPool) {
    if std::env::var("STEAM_PUBLISHER_KEY").is_err() {
        return;
    }
    let rows: Vec<(i64, Option<String>)> = match sqlx::query_as(
        "SELECT order_id, transid FROM store_orders WHERE status = 'charging' AND charging_at < now() - interval '10 minutes' LIMIT 50",
    ).fetch_all(db).await {
        Ok(r) => r,
        Err(e) => { eprintln!("[STORE] charging sweep could not read orders: {e}"); return; }
    };
    for (order_id, transid) in rows {
        let Some(transid) = transid.filter(|t| !t.is_empty()) else {
            eprintln!("[STORE] ALERT order={order_id} stuck 'charging' with no transid — needs a person");
            continue;
        };
        let next = match steam_txn_status(&transid).await {
            Ok(s) => match s.to_ascii_lowercase().as_str() {
                "succeeded" => "charged",
                "init" => "pending",
                // Authorized, not finalized — yet. Steam may still complete the FinalizeTxn that
                // timed out; only after an hour is it treated as never charged.
                "approved" if charging_for_long(db, order_id).await => "pending",
                "approved" => continue,
                "failed" => "failed",
                other => {
                    eprintln!("[STORE] ALERT order={order_id} stuck 'charging', Steam says '{other}' — needs a person");
                    continue;
                }
            },
            Err(e) => { eprintln!("[STORE] charging order={order_id}: {e}"); continue; }
        };
        let _ = sqlx::query("UPDATE store_orders SET status = $2 WHERE order_id = $1 AND status = 'charging'")
            .bind(order_id).bind(next).execute(db).await;
        println!("[STORE] charging order={order_id} settled as '{next}'");
    }
}

#[derive(serde::Deserialize)]
struct AdminBanReq {
    account_id: u64,
    /// Hours from now; absent = permanent.
    #[serde(default)]
    hours: Option<u64>,
    reason: String,
}

#[derive(serde::Deserialize)]
struct AdminUnbanReq {
    account_id: u64,
}


/// POST /admin/ban (ADMIN_KEY) — suspend an account (`hours`) or ban it (no `hours`). A HUMAN
/// decision, made after reviewing `/flags`; the server never bans on its own. The live session
/// ends immediately and every later login is refused until `until_ms` (or `/admin/unban`).
async fn admin_ban(State(st): State<AppState>, Json(req): Json<AdminBanReq>) -> (StatusCode, Json<serde_json::Value>) {
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "bans need a database"})));
    };
    if req.reason.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"ok": false, "reason": "a ban needs a reason"})));
    }
    let now = now_ms();
    let until = req.hours.map(|h| now.saturating_add((h as i64).saturating_mul(3_600_000)));
    // Ban + eviction under the account's session lock (see `mint_session`): a login racing the
    // ban either finishes first and is evicted here, or waits and is refused.
    let r = async {
        let mut tx = db.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)").bind(session_lock_key(req.account_id)).execute(&mut *tx).await?;
        sqlx::query(
            "INSERT INTO account_bans (account_id, until_ms, reason, at_ms) VALUES ($1, $2, $3, $4) \
             ON CONFLICT (account_id) DO UPDATE SET until_ms = EXCLUDED.until_ms, reason = EXCLUDED.reason, at_ms = EXCLUDED.at_ms",
        ).bind(req.account_id as i64).bind(until).bind(&req.reason).bind(now).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM active_sessions WHERE account_id = $1").bind(req.account_id as i64).execute(&mut *tx).await?;
        swap_session(&st.sessions, req.account_id, None);
        tx.commit().await
    }.await;
    if let Err(e) = r {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
    }
    println!("ADMIN ban acct={} until_ms={until:?} reason={:?}", req.account_id, req.reason);
    (StatusCode::OK, Json(serde_json::json!({"ok": true, "account_id": req.account_id, "until_ms": until})))
}

/// POST /admin/unban (ADMIN_KEY) — lift a ban; the player can sign in again.
async fn admin_unban(State(st): State<AppState>, Json(req): Json<AdminUnbanReq>) -> (StatusCode, Json<serde_json::Value>) {
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "bans need a database"})));
    };
    let n = sqlx::query("DELETE FROM account_bans WHERE account_id = $1").bind(req.account_id as i64).execute(db).await
        .map(|r| r.rows_affected()).unwrap_or(0);
    println!("ADMIN unban acct={} (had ban: {})", req.account_id, n == 1);
    (StatusCode::OK, Json(serde_json::json!({"ok": true, "was_banned": n == 1})))
}

/// GET /admin/bans (ADMIN_KEY) — every recorded ban, newest first (expired ones included).
async fn admin_bans(State(st): State<AppState>) -> Json<serde_json::Value> {
    let Some(db) = &st.db else { return Json(serde_json::json!({"bans": [], "note": "no db"})) };
    let rows: Vec<(i64, Option<i64>, String, i64)> = sqlx::query_as(
        "SELECT account_id, until_ms, reason, at_ms FROM account_bans ORDER BY at_ms DESC LIMIT 500",
    ).fetch_all(db).await.unwrap_or_default();
    Json(serde_json::json!({"bans": rows.into_iter().map(|(a, u, r, at)| serde_json::json!({
        "account_id": a, "until_ms": u, "reason": r, "at_ms": at})).collect::<Vec<_>>()}))
}

#[derive(serde::Deserialize)]
struct AdminGrantReq {
    order_id: String,
    /// Grant an order still at `init` / `pending` — ONLY after checking in the Steam partner site
    /// that the transaction was actually charged. A `charged` order needs no force.
    #[serde(default)]
    force: bool,
}

/// POST /admin/store/grant (ADMIN_KEY) — finish a paid order by hand. A `charged` order is granted
/// as the sweep would; an `init` / `pending` one only with `force` (the operator vouches that Steam
/// took the money). Settled orders answer `idempotent`; nothing here can grant twice.
async fn admin_store_grant(State(st): State<AppState>, Json(req): Json<AdminGrantReq>) -> (StatusCode, Json<serde_json::Value>) {
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "store needs a database"})));
    };
    let Ok(order_id) = req.order_id.parse::<i64>() else {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"ok": false, "reason": "bad order id"})));
    };
    let status: Option<(String,)> = sqlx::query_as("SELECT status FROM store_orders WHERE order_id = $1")
        .bind(order_id).fetch_optional(db).await.ok().flatten();
    let Some((status,)) = status else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"ok": false, "reason": "no such order"})));
    };
    if matches!(status.as_str(), "init" | "pending" | "charging") && !req.force {
        return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": format!(
            "order is '{status}' — Steam may not have charged it; verify in the partner site, then resend with force")})));
    }
    match grant_order(db, &st.data, order_id).await {
        Ok(GrantOutcome::Granted { first_buy, paid, bound }) => {
            println!("[STORE] ADMIN granted order={order_id} (was '{status}') +{paid}t/+{bound}b");
            (StatusCode::OK, Json(serde_json::json!({"ok": true, "reason": "granted", "first_buy": first_buy,
                "paid": paid.to_string(), "tradeable": paid.to_string(), "bound": bound.to_string()})))
        }
        Ok(GrantOutcome::AlreadySettled) => (StatusCode::OK, Json(serde_json::json!({"ok": true, "reason": "idempotent", "status": status}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e}))),
    }
}

#[derive(serde::Deserialize)]
struct RefundReq {
    order_id: String,
}

/// Claim an order for granting: flips it to `granted` and records what it paid, returning true
/// EXACTLY ONCE per order. False means somebody else already settled it.
///
/// Extracted so it can be TESTED. `store_finalize` calls Steam's FinalizeTxn before it reaches
/// here, so the handler cannot be driven from a test at all — and the one behaviour that has to
/// be right is this guard. A test that replays the statement inline instead would agree with
/// whatever the test wrote, which is the failure this crate keeps producing.
///
/// The guard is an ALLOW-LIST. It was `<> 'granted'`, which let a REFUNDED order be finalized a
/// second time — and because the first-buy count ignores revoked rows, the refund re-armed the
/// bonus it had just taken away. A deny-list also silently admits whatever status is added next.
/// `init` is allowed because the update that moves an order to `pending` after InitTxn is a
/// `let _ =`, so a lost result leaves a good order sitting at `init`.
async fn claim_order_for_grant(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    order_id: i64,
    first_buy: bool,
    paid: i64,
    bound: i64,
) -> bool {
    sqlx::query(
        "UPDATE store_orders SET status = 'granted', first_buy = $1, granted_paid = $2, \
         granted_bound = $3, granted_at = now() WHERE order_id = $4 AND status IN ('init', 'pending', 'charging', 'charged', 'grant_failed')",
    )
    .bind(first_buy).bind(paid).bind(bound).bind(order_id)
    .execute(&mut **tx).await.map(|r| r.rows_affected() == 1).unwrap_or(false)
}

/// CONSERVATION for the crystal wallet: every crystal a wallet holds must be explained by a ledger row.
///
/// `crystal_ledger` was built as the audit trail for crystal, and nothing had ever compared the
/// two. The (since removed) Gil market had exactly this gap: its I7 formula was
/// wrong for months and the fixture was too narrow to contradict it, so both agreed and neither
/// was right.
///
/// THE LAW IS EXACT EQUALITY, per account, per bucket: `wallet == SUM(ledger)`.
///
/// It was not, for one day. `store_refund` clamps at zero — a player who already spent the crystal
/// keeps what they rolled — while logging the full negative delta, so a clamped refund left the
/// wallet permanently ABOVE its ledger with nothing recording by how much. That is the same shape
/// as crystal minted from nowhere, which meant the audit trail could not tell the two apart on any
/// account that had ever been refunded. The refund now writes a `refund-shortfall` row for what
/// the clamp could not take, so the sum balances again and the full reversal is still visible.
///
/// The direction is reported because it says WHICH failure happened:
///
/// - `Short` — `wallet < SUM(ledger)`. The ledger promises crystal the wallet does not hold: a
///   ledger row whose wallet write never landed. `/dev/gacha-crystal` can still produce this, since
///   it writes the pair outside a transaction and discards the ledger result.
/// - `Unexplained` — `wallet > SUM(ledger)`. Crystal with no row behind them. This is the direction
///   that costs money, and it is only checkable at all because the shortfall is now recorded.
#[derive(Debug, PartialEq)]
enum CrystalBreak {
    Short,
    Unexplained,
}

async fn crystal_conservation_violations(
    db: &sqlx::PgPool,
) -> Result<Vec<(i64, &'static str, CrystalBreak, i64, i64)>, sqlx::Error> {
    // LEFT JOIN from the wallet: an account with a wallet row and no ledger rows at all is exactly
    // the case worth catching, and an INNER JOIN would hide it.
    let rows: Vec<(i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT w.account_id, w.paid, w.bound, \
                COALESCE(SUM(l.d_paid), 0)::bigint, COALESCE(SUM(l.d_bound), 0)::bigint \
         FROM crystal_wallet w LEFT JOIN crystal_ledger l ON l.account_id = w.account_id \
         GROUP BY w.account_id, w.paid, w.bound",
    )
    .fetch_all(db)
    .await?;
    let mut out = Vec::new();
    for (acct, wt, wb, lt, lb) in rows {
        for (bucket, w, l) in [("paid", wt, lt), ("bound", wb, lb)] {
            if w < l {
                out.push((acct, bucket, CrystalBreak::Short, w, l));
            } else if w > l {
                out.push((acct, bucket, CrystalBreak::Unexplained, w, l));
            }
        }
    }
    Ok(out)
}

/// `POST /store/refund` (dev-gated) — Steam refunded a purchase.
///
/// COMPENSATING, never deleting: the entitlement is marked revoked and the crystal come back out as
/// a NEGATIVE ledger delta, so the audit trail shows a purchase and a reversal rather than an
/// absence. The wallet is allowed to go negative in effect — a player who already spent the crystal
/// keeps what they rolled, and that is what the review flag is for. Auto-punishing here would
/// mean a refund could take away items, which is a support problem, not a cheat.
async fn store_refund(
    State(st): State<AppState>,
    Json(req): Json<RefundReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "store needs a database"})));
    };
    let Ok(order_id) = req.order_id.parse::<i64>() else {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"ok": false, "reason": "bad order id"})));
    };
    let row: Option<(i64, String, String, i64, i64)> = sqlx::query_as(
        "SELECT account_id, sku, status, granted_paid, granted_bound FROM store_orders WHERE order_id = $1",
    ).bind(order_id).fetch_optional(db).await.ok().flatten();
    let Some((acct_id, sku_id, status, gt, gb)) = row else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"ok": false, "reason": "no such order"})));
    };
    if status == "refunded" {
        return (StatusCode::OK, Json(serde_json::json!({"ok": true, "reason": "idempotent"})));
    }
    match revoke_order(db, order_id, acct_id, gt, gb).await {
        Ok(false) => return (StatusCode::OK, Json(serde_json::json!({"ok": true, "reason": "idempotent"}))),
        Err(()) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": "persist failed — retry"}))),
        Ok(true) => {}
    }
    flag_cheat(&st, acct_id as u64, "refund", 0).await;
    println!("store-refunded acct={acct_id} sku={sku_id} order={order_id} -{gt}t/-{gb}b");
    (StatusCode::OK, Json(serde_json::json!({"ok": true, "reason": "refunded"})))
}

/// TAKE ONE ORDER BACK. Extracted from `store_refund` so the two ways a refund can arrive — an
/// operator calling the dev endpoint, and the hourly reconcile noticing Steam refunded it — cannot
/// drift apart. A revocation that behaves differently depending on who asked for it is worse than
/// one that only exists in one place: the difference would show up as a wallet that is correct
/// only when a human did it.
///
/// `Ok(false)` means the order was already refunded — the caller should treat that as success, not
/// as an error, because both callers can legitimately race.
async fn revoke_order(
    db: &sqlx::PgPool,
    order_id: i64,
    acct_id: i64,
    gt: i64,
    gb: i64,
) -> Result<bool, ()> {
    let Ok(mut tx) = db.begin().await else { return Err(()) };
    lock_account(&mut tx, acct_id).await;
    // The status it had, under the row lock: only a GRANTED order has anything to take back
    // (audit 2026-10-02). Reversing a `charged` / `pending` one took coupons and monthly days that
    // came from OTHER purchases, since `revoke_contents` sizes itself from the SKU.
    // The AMOUNTS too, under the same lock (audit 2026-10-03): read by the caller before the lock,
    // a refund racing a late grant took the contents back and 0 crystal.
    let was: Option<(String, i64, i64)> = match sqlx::query_as(
        "SELECT status, granted_paid, granted_bound FROM store_orders WHERE order_id = $1 FOR UPDATE",
    )
        .bind(order_id).fetch_optional(&mut *tx).await
    {
        Ok(r) => r,
        Err(_) => return Err(()),
    };
    let was_granted = matches!(was.as_ref().map(|(s, _, _)| s.as_str()), Some("granted"));
    let (gt, gb) = was.as_ref().map_or((gt, gb), |(_, p, b)| (*p, *b));
    let claimed = sqlx::query("UPDATE store_orders SET status = 'refunded' WHERE order_id = $1 AND status <> 'refunded'")
        .bind(order_id).execute(&mut *tx).await.map(|r| r.rows_affected() == 1).unwrap_or(false);
    if !claimed {
        return Ok(false);
    }
    if !was_granted {
        return if tx.commit().await.is_ok() { Ok(true) } else { Err(()) };
    }
    let writes = async {
        sqlx::query("UPDATE entitlements SET revoked_at = now() WHERE order_id = $1 AND revoked_at IS NULL")
            .bind(order_id).execute(&mut *tx).await?;
        // THE WHOLE GRANT COMES BACK, into DEBT if it was spent (user 2026-10-02: "ควรต้องติดลบด้วย").
        // A negative bucket blocks every crystal spend (`wallet_in_debt`) until a purchase refills
        // it, so a refund cannot keep what the crystal bought AND the money. The row is locked so a
        // concurrent pull cannot interleave.
        sqlx::query("INSERT INTO crystal_wallet (account_id) VALUES ($1) ON CONFLICT (account_id) DO NOTHING")
            .bind(acct_id).execute(&mut *tx).await?;
        sqlx::query("SELECT 1 FROM crystal_wallet WHERE account_id = $1 FOR UPDATE")
            .bind(acct_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES ($1, $2, $3, 'refund', $4)")
            .bind(acct_id).bind(-gt).bind(-gb).bind(order_id.to_string())
            .execute(&mut *tx).await?;
        sqlx::query("UPDATE crystal_wallet SET paid = paid - $1, bound = bound - $2 WHERE account_id = $3")
            .bind(gt).bind(gb).bind(acct_id).execute(&mut *tx).await?;
        revoke_contents(&mut tx, order_id, acct_id).await?;
        Ok::<(), sqlx::Error>(())
    }.await;
    if writes.is_err() || tx.commit().await.is_err() {
        return Err(());
    }
    Ok(true)
}

/// Undo what `grant_contents` delivered, as far as it is still unspent: pack mail not yet claimed
/// is withdrawn, unspent coupons are taken back, the monthly card loses its 30 days (never into
/// the past), and the Battle Pass entitlement is deactivated. Claimed items and spent coupons stay
/// with the player, like spent crystal — the refund flag is what reviews them.
async fn revoke_contents(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    order_id: i64,
    acct_id: i64,
) -> Result<(), sqlx::Error> {
    let sku_id: Option<(String,)> = sqlx::query_as("SELECT sku FROM store_orders WHERE order_id = $1")
        .bind(order_id).fetch_optional(&mut **tx).await?;
    let Some(sku) = sku_id.and_then(|(id,)| store::sku(&id)) else { return Ok(()) };
    sqlx::query("UPDATE mailbox SET voided_at = $3 WHERE account_id = $1 AND reason = $2 AND claimed_at IS NULL AND voided_at IS NULL")
        .bind(acct_id).bind(store_mail_reason(order_id)).bind(now_ms() as i64).execute(&mut **tx).await?;
    if sku.coupons > 0 {
        let held: Option<(i64,)> = sqlx::query_as("SELECT qty FROM bound_items WHERE account_id = $1 AND kind = $2 FOR UPDATE")
            .bind(acct_id).bind(gacha::STANDARD_COUPON_KIND).fetch_optional(&mut **tx).await?;
        let took = i64::from(sku.coupons).min(held.map_or(0, |r| r.0.max(0)));
        if took > 0 {
            sqlx::query("UPDATE bound_items SET qty = qty - $3 WHERE account_id = $1 AND kind = $2")
                .bind(acct_id).bind(gacha::STANDARD_COUPON_KIND).bind(took).execute(&mut **tx).await?;
            sqlx::query("INSERT INTO bound_ledger (account_id, kind, delta, reason, ref) VALUES ($1, $2, $3, 'refund', $4)")
                .bind(acct_id).bind(gacha::STANDARD_COUPON_KIND).bind(-took).bind(order_id.to_string())
                .execute(&mut **tx).await?;
        }
    }
    match sku.section {
        store::Section::Monthly => {
            let today = daily::day_of_ms(now_ms() as u64);
            let window: Option<(Option<i64>, Option<i64>)> = sqlx::query_as(
                "SELECT monthly_from_day, monthly_to_day FROM store_orders WHERE order_id = $1",
            ).bind(order_id).fetch_optional(&mut **tx).await?;
            let last: Option<(i64,)> = sqlx::query_as("SELECT last_claim_day FROM monthly_card WHERE account_id = $1 FOR UPDATE")
                .bind(acct_id).fetch_optional(&mut **tx).await?;
            match window {
                // THIS card's window only (audit 2026-10-03): with two cards banked, the old
                // "everything since the purchase" took the other card's claims and days.
                Some((Some(from), Some(to))) => {
                    let claimed_today = last.is_some_and(|(d,)| d >= today);
                    let first_unused = from.max(if claimed_today { today + 1 } else { today });
                    let unused = (to - first_unused).clamp(0, store::MONTHLY_DAYS);
                    sqlx::query("UPDATE monthly_card SET ends_day = ends_day - $2 WHERE account_id = $1")
                        .bind(acct_id).bind(unused).execute(&mut **tx).await?;
                    // Cards banked AFTER this one move down by the days it gave up (audit
                    // 2026-10-03): their days now start earlier, and a later refund of one of them
                    // must find the claims made on those days inside its own window.
                    if unused > 0 {
                        sqlx::query(
                            "UPDATE store_orders SET monthly_from_day = monthly_from_day - $3, monthly_to_day = monthly_to_day - $3 \
                             WHERE account_id = $1 AND order_id <> $2 AND status = 'granted' AND monthly_from_day >= $4",
                        ).bind(acct_id).bind(order_id).bind(unused).bind(to).execute(&mut **tx).await?;
                    }
                    let (paid_out,): (i64,) = sqlx::query_as(
                        "SELECT COALESCE(SUM(d_bound), 0)::bigint FROM crystal_ledger \
                         WHERE account_id = $1 AND reason = 'monthly' \
                           AND (CASE WHEN ref ~ '^day:[0-9]+$' THEN substr(ref, 5)::bigint END) >= $2 \
                           AND (CASE WHEN ref ~ '^day:[0-9]+$' THEN substr(ref, 5)::bigint END) < $3",
                    ).bind(acct_id).bind(from).bind(to).fetch_one(&mut **tx).await?;
                    claw_back_bound_crystal(tx, acct_id, paid_out.max(0), "refund-monthly", order_id).await?;
                }
                // An order granted before the window was recorded: the old rule.
                _ => {
                    sqlx::query("UPDATE monthly_card SET ends_day = GREATEST($2, ends_day - $3) WHERE account_id = $1")
                        .bind(acct_id).bind(today).bind(store::MONTHLY_DAYS).execute(&mut **tx).await?;
                }
            }
        }
        store::Section::BattlePass => {
            sqlx::query("UPDATE bp_entitlements SET active = false, revoked_at = now() WHERE txn_id = $1 AND source = 'store' AND revoked_at IS NULL")
                .bind(order_id.to_string()).execute(&mut **tx).await?;
            // AND what the premium track already paid in currency (audit 2026-10-02): crystal into
            // debt, tickets as far as they are held. Items mailed and claimed stay, like a pack's.
            let season: Option<(i64, String)> = sqlx::query_as(
                "SELECT s.season_id, s.config_json FROM seasons s, store_orders o WHERE o.order_id = $1 AND s.season_id = o.season_id",
            ).bind(order_id).fetch_optional(&mut **tx).await?;
            if let Some((season_id, cfg)) = season {
                if let Ok(cfg) = serde_json::from_str::<bp::SeasonConfig>(&cfg) {
                    let slots: Vec<(i16,)> = sqlx::query_as(
                        "UPDATE bp_claims SET voided_at = now(), void_reason = 'refund' \
                         WHERE account_id = $1 AND season_id = $2 AND track = 1 AND voided_at IS NULL RETURNING slot",
                    ).bind(acct_id).bind(season_id).fetch_all(&mut **tx).await?;
                    let (mut crystal, mut std_t, mut lim_t) = (0i64, 0i64, 0i64);
                    for (slot,) in slots {
                        let Some(r) = cfg.premium_track.get((slot as usize).saturating_sub(1)) else { continue };
                        match r.kind {
                            bp::RewardKind::Crystal => crystal += i64::from(r.qty),
                            bp::RewardKind::StandardTicket => std_t += i64::from(r.qty),
                            bp::RewardKind::LimitedTicket => lim_t += i64::from(r.qty),
                            bp::RewardKind::Item | bp::RewardKind::Gil => {}
                        }
                    }
                    claw_back_bound_crystal(tx, acct_id, crystal, "refund-bp", order_id).await?;
                    // Premium rewards mailed and not yet claimed are withdrawn like a pack's.
                    sqlx::query(
                        "UPDATE mailbox SET voided_at = $3 WHERE account_id = $1 AND reason LIKE $2 \
                           AND claimed_at IS NULL AND voided_at IS NULL",
                    ).bind(acct_id).bind(format!("bp s{season_id} premium slot %")).bind(now_ms() as i64)
                    .execute(&mut **tx).await?;
                    for (kind, n) in [(gacha::STANDARD_COUPON_KIND, std_t), (gacha::LIMITED_COUPON_KIND, lim_t)] {
                        let held: Option<(i64,)> = sqlx::query_as("SELECT qty FROM bound_items WHERE account_id = $1 AND kind = $2 FOR UPDATE")
                            .bind(acct_id).bind(kind).fetch_optional(&mut **tx).await?;
                        let took = n.min(held.map_or(0, |r| r.0.max(0)));
                        if took > 0 {
                            sqlx::query("UPDATE bound_items SET qty = qty - $3 WHERE account_id = $1 AND kind = $2")
                                .bind(acct_id).bind(kind).bind(took).execute(&mut **tx).await?;
                            sqlx::query("INSERT INTO bound_ledger (account_id, kind, delta, reason, ref) VALUES ($1, $2, $3, 'refund', $4)")
                                .bind(acct_id).bind(kind).bind(-took).bind(order_id.to_string())
                                .execute(&mut **tx).await?;
                        }
                    }
                }
            }
        }
        store::Section::Crystal | store::Section::Pack => {}
    }
    Ok(())
}

/// Take `n` BOUND crystal back for a refund, into debt if spent (as a refunded crystal pack).
async fn claw_back_bound_crystal(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    acct_id: i64,
    n: i64,
    reason: &str,
    order_id: i64,
) -> Result<(), sqlx::Error> {
    if n <= 0 {
        return Ok(());
    }
    sqlx::query("INSERT INTO crystal_wallet (account_id) VALUES ($1) ON CONFLICT (account_id) DO NOTHING")
        .bind(acct_id).execute(&mut **tx).await?;
    sqlx::query("UPDATE crystal_wallet SET bound = bound - $2 WHERE account_id = $1")
        .bind(acct_id).bind(n).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES ($1, 0, $2, $3, $4)")
        .bind(acct_id).bind(-n).bind(reason).bind(order_id.to_string())
        .execute(&mut **tx).await?;
    Ok(())
}

/// `POST /monthly/claim` — today's monthly-card crystal. Once per day while the card runs; a day
/// not claimed is forfeited (the design's "miss a day = forfeit that day").
async fn monthly_claim(State(st): State<AppState>, headers: HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "no database"})));
    };
    let today = daily::day_of_ms(now_ms() as u64);
    let crystal = store::MONTHLY_DAILY_CRYSTAL as i64;
    let res = async {
        let mut tx = db.begin().await?;
        lock_account(&mut tx, acct.account_id as i64).await;
        let claimed = sqlx::query(
            "UPDATE monthly_card SET last_claim_day = $2              WHERE account_id = $1 AND ends_day > $2 AND last_claim_day < $2",
        )
        .bind(acct.account_id as i64).bind(today).execute(&mut *tx).await?.rows_affected() == 1;
        if !claimed {
            return Ok::<bool, sqlx::Error>(false);
        }
        sqlx::query(
            "INSERT INTO crystal_wallet (account_id, paid, bound) VALUES ($1, 0, $2)              ON CONFLICT (account_id) DO UPDATE SET bound = crystal_wallet.bound + EXCLUDED.bound",
        )
        .bind(acct.account_id as i64).bind(crystal).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES ($1, 0, $2, 'monthly', $3)")
            .bind(acct.account_id as i64).bind(crystal).bind(format!("day:{today}"))
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }.await;
    match res {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({"ok": true, "crystal": crystal}))),
        Ok(false) => (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": "nothing to claim today"}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    }
}

/// `GET /store/me` — this account's side of the storefront: what each SKU still allows, the
/// monthly card and the Battle Pass premium state.
async fn store_me(State(st): State<AppState>, headers: HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "no database"})));
    };
    let res = async {
        let mut skus = Vec::new();
        for sku in store::CATALOG {
            let bought = sku_purchases(db, acct.account_id, sku.id).await?;
            let refusal = purchase_refusal(&st, db, acct.account_id, sku).await?;
            skus.push(serde_json::json!({
                "sku": sku.id, "bought": bought, "buyable": refusal.is_none(), "reason": refusal.unwrap_or(""),
            }));
        }
        let today = daily::day_of_ms(now_ms() as u64);
        let card: Option<(i64, i64)> = sqlx::query_as("SELECT ends_day, last_claim_day FROM monthly_card WHERE account_id = $1")
            .bind(acct.account_id as i64).fetch_optional(db).await?;
        let (ends, last) = card.unwrap_or((0, -1));
        let season = live_season(&st);
        let premium = match season {
            Some(season) => bp_premium_owned(db, acct.account_id, season.season_id).await?,
            None => false,
        };
        Ok::<_, sqlx::Error>(serde_json::json!({
            "ok": true,
            "day": today,
            "skus": skus,
            "monthly": {
                "active": ends > today,
                "days_left": (ends - today).max(0),
                "claimable_today": ends > today && last < today,
                "daily_crystal": store::MONTHLY_DAILY_CRYSTAL,
            },
            "battle_pass_premium": premium,
        }))
    }.await;
    match res {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    }
}

#[derive(serde::Deserialize)]
struct SteamAuthReq {
    ticket: String,
}

const STEAM_APP_ID: u32 = 4958230;

/// POST /auth/steam — validate a Steam Web-API auth ticket (server-authoritative; the client can't
/// forge it), resolve the steamid → account. Same response shape as /auth/dev on success.
async fn auth_steam(
    State(st): State<AppState>,
    Json(req): Json<SteamAuthReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Ok(key) = std::env::var("STEAM_PUBLISHER_KEY") else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"_error": "steam auth not configured (STEAM_PUBLISHER_KEY unset)"})),
        );
    };
    // Length sanity only (a real Web-API ticket is a few hundred bytes to a couple KB of hex). The
    // earlier hex allow-list also rejected valid tickets → dropped; injection is prevented below by
    // passing the ticket through reqwest's query builder (auto URL-encoded), not string-splicing.
    if req.ticket.is_empty() || req.ticket.len() > 65536 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"_error": "malformed ticket"})));
    }
    // Timeout so a slow/hung Steam Web API can't stall the login handler indefinitely.
    let client = match reqwest::Client::builder().timeout(std::time::Duration::from_secs(5)).build() {
        Ok(c) => c,
        Err(e) => return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"_error": format!("http client: {e}")}))),
    };
    // No identity param (empty → Steam errorcode 104). query() URL-encodes each value → the untrusted
    // ticket can't inject extra params.
    let appid = STEAM_APP_ID.to_string();
    let resp = match client
        .get("https://api.steampowered.com/ISteamUserAuth/AuthenticateUserTicket/v1/")
        .query(&[("key", key.as_str()), ("appid", appid.as_str()), ("ticket", req.ticket.as_str())])
        .send()
        .await
    {
        Ok(r) => r,
        // Never echo the transport error: its Display carries the URL, and the URL carries the
        // publisher key (audit 2026-10-02). Logged without it.
        Err(e) => {
            eprintln!("[AUTH] steam AuthenticateUserTicket failed: {}", e.without_url());
            return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"_error": "steam api unreachable"})));
        }
    };
    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[AUTH] steam AuthenticateUserTicket body: {}", e.without_url());
            return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"_error": "steam api bad json"})));
        }
    };
    // { response: { params: { result: "OK", steamid: "7656…", ownersteamid: "…" } } }
    let params = body.get("response").and_then(|r| r.get("params"));
    let result_ok = params.and_then(|p| p.get("result")).and_then(|v| v.as_str()) == Some("OK");
    let steamid = params.and_then(|p| p.get("steamid")).and_then(|v| v.as_str());
    let (true, Some(steamid)) = (result_ok, steamid) else {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"_error": "steam ticket rejected", "steam": body})));
    };
    // Block Steam Family Sharing: a borrower's steamid differs from the game's ownersteamid. Require
    // the playing account to actually OWN the game (economy integrity — one copy ≠ many accounts).
    // (Confirmed 2026-07-12 that a Developer-Comp login has ownersteamid == steamid, so a real owner
    // is not blocked.)
    let ownersteamid = params.and_then(|p| p.get("ownersteamid")).and_then(|v| v.as_str());
    if ownersteamid.is_some_and(|owner| owner != steamid) {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"_error": "family sharing not allowed — this account must own the game"})));
    }
    // Steam's own bans. A PUBLISHER ban is ours (set from the partner site) → refuse. A VAC ban is
    // another game's anti-cheat verdict → logged only, never enforced here.
    let flag = |k: &str| params.and_then(|p| p.get(k)).and_then(|v| v.as_bool()).unwrap_or(false);
    if flag("publisherbanned") {
        println!("login refused: steam {steamid} is publisher-banned");
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"_error": "this Steam account is banned from the game"})));
    }
    if flag("vacbanned") {
        println!("login note: steam {steamid} carries a VAC ban (another game) — allowed");
    }
    let auth = match login_checked(&st, format!("steam:{steamid}")).await {
        Ok(a) => a,
        Err((code, why)) => return (code, Json(serde_json::json!({"_error": why}))),
    };
    // Cannot fail: `auth` is a plain struct of scalars — no non-string map key, no Serialize impl
    // of ours that can error. The handler's return type is `Json<Value>` because the error paths
    // build their bodies with `json!`, so the conversion has to happen somewhere.
    let body = serde_json::to_value(auth).expect("AuthResp is a plain struct");
    (StatusCode::OK, Json(body))
}

/// Resolve the caller's account from the `Authorization: Bearer <token>` header.
fn account_of(st: &AppState, headers: &HeaderMap) -> Option<Account> {
    let raw = headers.get("authorization")?.to_str().ok()?;
    let token = raw.strip_prefix("Bearer ").unwrap_or(raw);
    st.sessions.by_token.held().get(token).cloned()
}

/// First 8 bytes of a blake3 digest, little-endian. Infallible BY CONSTRUCTION: `as_bytes()`
/// returns a `[u8; 32]`, so the 8-byte prefix is an array pattern the compiler checks — not a
/// slice `try_into()` that has to be unwrapped. Every server nonce/seed derives through here.
fn digest_u64(h: &blake3::Hasher) -> u64 {
    let [a, b, c, d, e, f, g, i, ..] = *h.finalize().as_bytes();
    u64::from_le_bytes([a, b, c, d, e, f, g, i])
}

// ==================== SERVER-ROLL FUSION (design §7.8) ====================================
//
// The shape is the retired refine roll's (0.0.370, `docs/refine-server-roll-2b.md`), which
// closed the same hole: a client that holds the seed can read the sim's next draws and act
// only when they say what it wants. Issue a keyed nonce on request, COMMIT to it (outstanding
// until a verified save consumes it), read the outcome from the nonce instead of the stream,
// and at promotion recompute the nonce from the replayed state so it cannot have been issued
// for a different six. Only fusions whose output is `fusion::FUSE_SERVER_ROLL_FROM` or better
// need one; the rest stay client-rolled and never touch this code.

/// A fusion roll the server has ISSUED but not yet seen consumed. Matched against the
/// `FuseGear` events of each synced bundle; one that is never consumed is a peek-and-skip
/// suspect. Kept per account, insertion-ordered.
#[derive(Clone, serde::Serialize)]
struct OutstandingRoll {
    /// `sim_core::fusion::binding` — the six sorted product keys.
    key: String,
    /// The attempt number the roll was issued for (`measures.fusions` at issue time).
    attempt: u64,
    nonce: u64,
    /// When this roll was issued (or loaded), for TTL pruning. A legitimate roll is consumed on
    /// the next autosave (~10 s); one still outstanding after `OUTSTANDING_TTL` is an abandoned
    /// or doomed-session orphan and is GC'd. Not serialized; loaded rolls are re-stamped `now`
    /// (the DB startup sweep already dropped the genuinely old rows).
    #[serde(skip, default = "SystemTime::now")]
    issued_at: SystemTime,
}

/// How long an issued-but-unconsumed roll may linger before it is pruned as an orphan. An hour
/// is far past any real issue→consume gap, so pruning cannot false-forge a slow-but-legit fusion.
const OUTSTANDING_TTL: Duration = Duration::from_secs(3600);

/// Drop outstanding rolls older than `ttl`; returns the pruned nonces (for a DB delete).
/// A roll with an `issued_at` in the FUTURE (clock skew) is kept, not pruned.
fn prune_outstanding(rolls: &mut Vec<OutstandingRoll>, now: SystemTime, ttl: Duration) -> Vec<u64> {
    let mut pruned = Vec::new();
    rolls.retain(|r| {
        let stale = now.duration_since(r.issued_at).map(|age| age >= ttl).unwrap_or(false);
        if stale {
            pruned.push(r.nonce);
        }
        !stale
    });
    pruned
}

/// The server-authored fusion nonce: blake3 keyed by the server secret, bound to (account, the
/// six products, attempt). Unpredictable (no secret), un-re-rollable (the attempt counter only
/// moves forward, and a skipped attempt keeps its number — the same request returns the same
/// nonce), and unspendable on a different six (the binding). The sim reads all three outcomes
/// out of it (`fusion::rolls_from_nonce`) against the published odds — the numbers are the
/// same, only the entropy is ours.
///
/// The key is length-prefixed so two keys cannot collide by concatenation. 63-bit masked (the
/// client carries it through JSON as a string) and never 0, the sim's client-roll sentinel.
fn fuse_nonce(secret: &str, account_id: u64, key: &str, attempt: u64) -> u64 {
    let mut h = blake3::Hasher::new();
    h.update(b"fuse-roll:v1");
    h.update(secret.as_bytes());
    h.update(&account_id.to_le_bytes());
    h.update(&(key.len() as u64).to_le_bytes());
    h.update(key.as_bytes());
    h.update(&attempt.to_le_bytes());
    let n = digest_u64(&h) & 0x7FFF_FFFF_FFFF_FFFF;
    if n == 0 { 1 } else { n }
}

#[derive(serde::Deserialize)]
struct FuseRollReq {
    /// `sim_core::fusion::binding` of the six the client is about to fuse.
    key: String,
    /// The client's `measures.fusions` — the attempt this roll is for. The verifier recomputes
    /// it from replay, so lying here only produces a nonce the bind-check will reject.
    attempt: u64,
}

#[derive(serde::Serialize)]
struct FuseRollResp {
    issued: bool,
    /// Decimal string of the u64 nonce (a JSON number past 2^53 loses precision in a
    /// float-based parser). "0" when not issued.
    nonce: String,
    reason: String,
}

/// Open (issued, unused) fusion rolls one account may hold — a bound on memory and DB rows.
const FUSE_OUTSTANDING_MAX: usize = 4;

/// Longest binding key accepted: six product keys of a plausible length. A key is stored per
/// outstanding row, so an unbounded one is a storage lever.
const FUSE_KEY_MAX: usize = 6 * 64;

/// `POST /fuse/roll` — issue (and COMMIT to) a server-authored fusion roll. Issuing is binding:
/// it records an outstanding roll the client must later consume, so it cannot peek the outcome
/// and skip a failure. Idempotent — re-requesting the same (key, attempt) returns the same nonce
/// (a retry or an abandon-resume cannot re-roll).
async fn fuse_roll(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<FuseRollReq>,
) -> (StatusCode, Json<FuseRollResp>) {
    let refuse = |code: StatusCode, why: &str| {
        (code, Json(FuseRollResp { issued: false, nonce: "0".into(), reason: why.into() }))
    };
    let Some(acct) = account_of(&st, &headers) else {
        return refuse(StatusCode::UNAUTHORIZED, "missing/invalid token");
    };
    if req.key.is_empty() || req.key.len() > FUSE_KEY_MAX {
        return refuse(StatusCode::BAD_REQUEST, "key must be 1..384 chars");
    }
    // A TAINTED session (its last save was rejected for divergence/forgery) cannot fuse until
    // it recovers — refuse the roll so a doomed session stops harvesting nonces (and spamming
    // outstanding rows). The client declines to fuse on a refused roll.
    if st.sessions.tainted.held().contains(&acct.account_id) {
        return refuse(StatusCode::CONFLICT, "session out of sync — resync before fusing");
    }
    // An attempt the verified save has already used is stale: its roll could only be peeked.
    let verified_fusions = match &st.db {
        Some(db) => verified_state(&st, db, acct.account_id).await.ok().flatten().map(|g| g.measures.fusions.get()),
        None => None,
    };
    if verified_fusions.is_some_and(|f| req.attempt < f) {
        return refuse(StatusCode::CONFLICT, "that fusion attempt is already used");
    }
    // The nonce is deterministic, so a re-request always yields the same value; the outstanding
    // check only avoids a duplicate record (and reports issued vs idempotent).
    let nonce = fuse_nonce(&st.roll_secret, acct.account_id, &req.key, req.attempt);
    let already_used = st.fuse_consumed.held().get(&acct.account_id).is_some_and(|c| c.contains(&nonce));
    let (is_new, pruned, held_back) = {
        let mut rolls = st.fuse_rolls.held();
        let outstanding = rolls.entry(acct.account_id).or_default();
        // GC orphaned rolls for this account while the lock is held (issue is the natural touch
        // point for an active account; the DB startup sweep covers inactive ones).
        let pruned = prune_outstanding(outstanding, SystemTime::now(), OUTSTANDING_TTL);
        // PEEK-AND-SKIP (audit 2026-10-02): the outcome is computable from the nonce, so an issued
        // roll commits that ATTEMPT to its six. Refused until the open roll is used or expires:
        //  - another six for the same attempt (shopping subsets for the one that succeeds);
        //  - a later attempt while an earlier roll sits unused (peek, burn the attempt on a cheap
        //    fusion, ask again);
        //  - more than `FUSE_OUTSTANDING_MAX` open rolls (memory).
        let held_back = if already_used {
            // A retry racing the save that just consumed this roll (audit 2026-10-03) must not
            // re-open it as outstanding: that blocked later high fusions for up to an hour.
            Some("that fusion roll was already used".to_string())
        } else if let Some(r) = outstanding.iter().find(|r| r.attempt == req.attempt && r.nonce != nonce) {
            Some(format!("a roll for this attempt is already out for other pieces: {}", r.key))
        } else if outstanding.iter().any(|r| r.attempt < req.attempt) {
            Some("an earlier fusion roll is not settled yet — retry after the next save (an unused one expires within the hour)".to_string())
        } else if outstanding.len() >= FUSE_OUTSTANDING_MAX && !outstanding.iter().any(|r| r.nonce == nonce) {
            Some("too many open fusion rolls".to_string())
        } else {
            None
        };
        let is_new = if held_back.is_some() || outstanding.iter().any(|r| r.nonce == nonce) {
            false
        } else {
            outstanding.push(OutstandingRoll {
                key: req.key.clone(),
                attempt: req.attempt,
                nonce,
                issued_at: SystemTime::now(),
            });
            true
        };
        (is_new, pruned, held_back)
    }; // lock dropped before the awaits below
    if let Some(db) = &st.db {
        // Write-through the prune so a scale-to-zero restart does not reload the orphans.
        for n in &pruned {
            let _ = sqlx::query("DELETE FROM fuse_outstanding WHERE account_id = $1 AND nonce = $2")
                .bind(acct.account_id as i64).bind(*n as i64).execute(db).await;
        }
        // Persist the outstanding roll so a restart cannot lose it (which would false-flag the
        // fusion as forged at reconcile). On EVERY answer that hands the nonce out, not only the
        // first (audit 2026-10-03): a first write that failed or was cut off by the request
        // timeout left the roll in memory only, and the "idempotent" retry never wrote it. The
        // nonce is answered only once it is durable.
        if held_back.is_none() {
            let wrote = sqlx::query(
                "INSERT INTO fuse_outstanding (account_id, nonce, key, attempt) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
            )
            .bind(acct.account_id as i64)
            .bind(nonce as i64)
            .bind(&req.key)
            .bind(req.attempt as i64)
            .execute(db)
            .await;
            if let Err(e) = wrote {
                eprintln!("[FUSE] could not persist roll for acct {}: {e} — retry", acct.account_id);
                // Not handed out, so not outstanding either: a client that never retries must not
                // be blocked by it for the hour it would take to expire.
                if is_new {
                    if let Some(out) = st.fuse_rolls.held().get_mut(&acct.account_id) {
                        out.retain(|r| r.nonce != nonce);
                    }
                }
                return refuse(StatusCode::SERVICE_UNAVAILABLE, "fusion roll not recorded — retry");
            }
        }
    }
    if let Some(why) = held_back {
        return refuse(StatusCode::CONFLICT, &why);
    }
    let reason = if is_new { "issued" } else { "idempotent" };
    (StatusCode::OK, Json(FuseRollResp { issued: true, nonce: nonce.to_string(), reason: reason.into() }))
}

/// The non-zero `server_roll` nonces the fusions in a log actually USED. A zero is a
/// client-rolled fusion and is skipped here — the bind-check is what rejects a zero on a
/// fusion that needed the server.
fn fuse_nonces_used(log: &[sim_core::events::RecordedInput]) -> Vec<u64> {
    log.iter()
        .filter_map(|ri| match &ri.event {
            InputEvent::FuseGear { server_roll, .. } if *server_roll != 0 => Some(*server_roll),
            _ => None,
        })
        .collect()
}

/// Result of the CHEAP, every-sync reconcile. Matches by NONCE value — a nonce is unforgeable
/// (no secret), so a used-but-never-issued nonce is a forgery (hard reject), and an
/// issued-but-unused one is a peek-and-skip suspect. Binding to the RIGHT six + attempt is
/// enforced by the promotion replay (`verify_fuse_bindings`).
#[derive(Debug, Default, PartialEq, Eq)]
struct Reconcile {
    forged: Vec<u64>,
    /// Issued rolls not yet consumed. A transient one is fine (issued then applied across a
    /// sync boundary); one that PERSISTS is a dropped failure → peek-and-skip.
    unconsumed: Vec<u64>,
    /// Nonces matched (outstanding → consumed) THIS pass — the DB write-through list.
    consumed: Vec<u64>,
}

/// Move matched nonces from `outstanding` into `consumed`, collect forgeries, and report whatever
/// remained unconsumed. Idempotent across a GROWING-log re-save (the same session's later bundle
/// repeats earlier nonces): a nonce already in `consumed` is accepted, not re-flagged. A nonce in
/// neither is unforgeable → a forgery.
fn reconcile_fuse(
    outstanding: &mut Vec<OutstandingRoll>,
    consumed: &mut HashSet<u64>,
    used: &[u64],
) -> Reconcile {
    let mut forged = Vec::new();
    let mut newly_consumed = Vec::new();
    for &n in used {
        if let Some(pos) = outstanding.iter().position(|r| r.nonce == n) {
            outstanding.remove(pos);
            consumed.insert(n);
            newly_consumed.push(n);
        } else if consumed.contains(&n) {
            // Already consumed on a prior save of this growing log — idempotent.
        } else {
            forged.push(n);
        }
    }
    let unconsumed = outstanding.iter().map(|r| r.nonce).collect();
    Reconcile { forged, unconsumed, consumed: newly_consumed }
}

/// Verdict of the promotion-time (full replay) bind-check for ONE fusion.
#[derive(Debug, PartialEq, Eq)]
enum FuseVerdict {
    Ok,
    /// A fusion whose output is Legendary or better carried no server roll — a client rolling
    /// locally (and steering) where the rule says the server rolls.
    HighNoRoll,
    /// The roll is not the nonce bound to THESE six at THIS attempt — a roll issued for another
    /// combine (target-selection) or an outright forgery.
    BadNonce,
}

/// Check ONE fusion against its bound server roll, using the REAL pre-fusion state (from
/// replay). A combine below the line needs no roll; one at or above it must carry the exact
/// nonce bound to (account, these six products, this attempt). This is what the cheap nonce-set
/// reconcile CANNOT see: that the nonce was issued for THIS combine. A combine the sim would
/// reject (`binding` = None) is a no-op in the sim and nothing to bind.
fn verify_fuse_binding(
    secret: &str,
    account_id: u64,
    state: &sim_core::state::GameState,
    picks: &[u32; sim_core::fusion::FUSE_INPUTS],
    server_roll: u64,
) -> FuseVerdict {
    let Some((key, attempt)) = sim_core::fusion::binding(state, picks) else {
        return FuseVerdict::Ok;
    };
    let grade = state.inventory.get(picks[0] as usize).map(|i| i.grade);
    let needs = grade.is_some_and(sim_core::fusion::needs_server_roll);
    if server_roll == 0 {
        return if needs { FuseVerdict::HighNoRoll } else { FuseVerdict::Ok };
    }
    if server_roll == fuse_nonce(secret, account_id, &key, attempt) { FuseVerdict::Ok } else { FuseVerdict::BadNonce }
}

/// Promotion-time (full replay) fusion bind-check: step the log and, BEFORE each fusion, read the
/// six's real products + the attempt number to recompute and confirm the bound nonce. Catches
/// target-selection and a high fusion that skipped the server — neither visible to the cheap
/// nonce-set reconcile. Errors on the first bad fusion. Skips the replay entirely when the log
/// holds no fusion at all, which is most saves.
fn verify_fuse_bindings(
    secret: &str,
    account_id: u64,
    initial: &sim_core::state::GameState,
    data: &GameData,
    log: &[sim_core::events::RecordedInput],
) -> Result<(), String> {
    if !log.iter().any(|r| matches!(r.event, InputEvent::FuseGear { .. })) {
        return Ok(());
    }
    let mut s = initial.clone();
    s.record_inputs = false;
    for rec in log {
        let cur = s.tick;
        if rec.tick > cur {
            replay::advance(&mut s, data, rec.tick - cur);
        }
        if let InputEvent::FuseGear { picks, server_roll } = &rec.event {
            match verify_fuse_binding(secret, account_id, &s, picks, *server_roll) {
                FuseVerdict::Ok => {}
                FuseVerdict::HighNoRoll => {
                    return Err(format!("fusion at tick {} needed a server roll and carries none", rec.tick))
                }
                FuseVerdict::BadNonce => {
                    return Err(format!("fusion at tick {} has a mismatched/forged nonce", rec.tick))
                }
            }
        }
        apply_input_with(&mut s, Some(data), rec.event.clone());
    }
    Ok(())
}

/// Whoami — proves a token works (401 without a valid one).
async fn me(State(st): State<AppState>, headers: HeaderMap) -> (StatusCode, Json<Option<Account>>) {
    match account_of(&st, &headers) {
        Some(a) => (StatusCode::OK, Json(Some(a))),
        None => (StatusCode::UNAUTHORIZED, Json(None)),
    }
}

/// DEV HELPER: play a short deterministic session and hand back a real, replayable
/// `SaveBundle` — so you can POST it to /verify (or tamper it and watch it fail).
async fn demo_bundle(State(st): State<AppState>) -> Json<SaveBundle> {
    let d = &*st.data;
    let mut live = data::new_game_in_zone(42, d, "zone_b1_1").expect("new game");
    let start = live.clone();
    live.record_inputs = true;
    live.snapshot_interval = 50; // several hash-chain checkpoints

    replay::advance(&mut live, d, 100);
    apply_input_with(&mut live, Some(d), InputEvent::SelectBand { band: 2 });
    replay::advance(&mut live, d, 100);

    Json(SaveBundle::capture(sim_core::VERSION, &d.version, start, &live))
}

#[derive(serde::Serialize)]
struct VerifyResponse {
    valid: bool,
    reason: String,
    final_tick: u64,
    checkpoints: usize,
}

/// THE anti-cheat check. Policy first (version compat + no forbidden events), then
/// the deterministic replay (`SaveBundle::verify` — re-run the log, match end hash +
/// the whole checkpoint chain). A forged state can't survive the replay.
async fn verify(
    State(st): State<AppState>,
    Json(bundle): Json<SaveBundle>,
) -> (StatusCode, Json<VerifyResponse>) {
    let d = &*st.data;
    let reply = |code: StatusCode, valid: bool, reason: String| {
        (
            code,
            Json(VerifyResponse {
                valid,
                reason,
                final_tick: bundle.final_tick,
                checkpoints: bundle.snapshots.len(),
            }),
        )
    };

    if let Err((code, msg)) = policy_check(d, &bundle) {
        return reply(code, false, msg);
    }
    if let Err((code, msg)) = bundle_within_limits(&bundle) {
        return reply(code, false, msg);
    }
    // Core — deterministic replay-verify.
    if bundle.verify(d) {
        reply(StatusCode::OK, true, "verified".to_string())
    } else {
        reply(StatusCode::OK, false, "replay hash mismatch — tampered or diverged".to_string())
    }
}

/// Max ticks a single bundle may advance from its start (~83 min of play at 10 t/s).
/// Bounds replay CPU per request → kills the "final_tick = 100M" DoS. A session that
/// legitimately runs longer must checkpoint (resume) so its bundle span resets.
///
/// A healthy client's span is ~100 ticks: it re-anchors on every confirmed push (10s). The span
/// only grows while pushes FAIL, so this doubles as the ceiling on how long a client may keep
/// playing unreachable. Lowered from 500_000 (~14h) on 2026-07-31: at ~3.7 µs/tick a 500k bundle
/// was ~1.8 s of CPU per request, so ~1 req/s saturated a core — 50k is ~0.18 s.
///
/// ⚠️ The client MUST stop before it reaches this, or it becomes permanently unsaveable — the
/// span never shrinks without a confirmed push. `SPAN_STALL_TICKS` in `sim_driver_core.gd` pauses
/// the sim below this bound; keep the two in step when changing either.
const MAX_REPLAY_SPAN: u64 = 50_000;
/// Max recorded inputs in one bundle (idle play is input-sparse; this is generous).
const MAX_LOG_LEN: usize = 50_000;

/// Milliseconds of real time per sim tick (10 ticks/s). Anti-fast-forward budget.
const MS_PER_TICK: i64 = 100;
/// Flat ADDITIVE slack on the tick budget (ticks = 30s at 10 t/s). ADDITIVE, not
/// multiplicative: legit play is ≤1.0× real time (the sim can't outrun the wall clock), so
/// `allowed = elapsed_ticks + SLACK` never false-positives — the only positive excess is
/// client/server clock RATE skew, and a realistic 0.01% skew needs an ~83h CONTINUOUS session
/// (the anchor resets every resume) to reach 300. The detectable cheat margin SHRINKS as the
/// session runs (a cheat trips once its excess passes SLACK: detection = SLACK/((rate−1)×10)s
/// → 2× ~30s, 1.5× ~60s, 1.05× ~10min). A multiplicative ×2 margin, by contrast, let ANY rate
/// ≤2× farm forever. SLACK is a per-SESSION cushion (anchor is fixed per session, not per
/// push); a re-login cheater who never accrues SLACK excess in one session is caught by the
/// LIFETIME layer instead (see `competition-integrity.md`). Also bounds the per-session cheat
/// LEAK (accepted-under-slack progress the auto-revert can't undo) to ~SLACK ticks = ~30s.
const FASTFWD_SLACK_TICKS: u64 = 300;

/// How far BEFORE boot the startup anti-fast-forward anchor is backdated, in ms.
///
/// The anchor is a wall-clock budget: "you cannot have advanced more ticks than I have been
/// watching you." A restart used to reset that clock to `now`, which is wrong for the client —
/// it kept playing (or kept a stalled backlog) through the downtime, so its first push after a
/// redeploy presents ticks the server has no budget for. With only `FASTFWD_SLACK_TICKS`, any
/// backlog past ~30 s came back as `fast-forward` — REJECTED **and cheat-flagged**, on a client
/// whose only mistake was being connected when we deployed. That is a false positive on an
/// honest player, which is worse than the leak it prevents.
///
/// Backdating a fixed grace covers the downtime without handing over the whole offline gap:
/// anchoring at the save's real timestamp would let a client that skips the handshake convert
/// hours of being logged OFF into instant farming, which is the exploit the startup seeding was
/// there to stop.
///
/// ⚠️ Must exceed `SPAN_STALL_TICKS`: a client that stalls the full span and reconnects to a
/// just-restarted server has to fit inside this budget or it loses the backlog anyway.
///
/// This is now a CLAMP, not the everyday budget. `boot_anchor_ms` anchors each account at its
/// OWN last save, so the ordinary case needs nothing from this constant; it only binds when a
/// save is older than the clamp. What it stops is the offline-claim: `fly.toml` runs
/// `min_machines_running = 0`, so without a ceiling an account whose save is hours old would be
/// handed hours of budget on a first push that skips the handshake.
///
/// Do not raise it — every extra minute is minutes a woken machine can mint. It also cannot go
/// much lower: a client that stalls its full `SPAN_STALL_TICKS` backlog and reconnects to a
/// just-booted server has to fit inside it, which
/// `the_client_stalls_before_the_server_refuses_its_bundle` enforces with 500 ticks of margin.
const RESTART_GRACE_MS: i64 = 6 * 60 * 1000;
/// How long after it was set a boot-synthesized anchor is honoured: the grace it is backdated by
/// plus the few minutes a connected client takes to push its backlog.
const BOOT_ANCHOR_TTL_MS: i64 = RESTART_GRACE_MS + 10 * 60 * 1000;

// ---- LAYER 2: lifetime cumulative excess (docs/competition-integrity.md) --------------------
//
// Layer 1's slack is per SESSION, and `GET /save` re-anchors unconditionally, so a re-login loop
// mints SLACK ticks per handshake with nothing counting across passes. Layer 2 accumulates the
// SAME excess layer 1 already computes into a durable per-account counter, which a new session
// adds onto instead of resetting.

/// The rate treated as "not cheating", in basis points of real time. 10_010 = 1.001×.
///
/// The doc's original design compared against exactly 1.0×, which cannot work HERE: this is an
/// idle game, so connected time accrues around the clock, and a client counts ticks off an
/// undisciplined monotonic clock while the server measures wall time. A crystal a few hundred
/// ppm fast therefore adds excess forever, and a LIFETIME counter never forgets it:
///
/// | client drift | excess/s | hits EXCESS_THRESHOLD after |
/// |---|---|---|
/// |  50 ppm (typical) | +0.0005 | 69 days connected |
/// | 200 ppm (in spec) | +0.002  | 17 days connected |
/// | 500 ppm (out of spec) | +0.005 | 7 days connected |
///
/// i.e. the honest heavy player is flagged, which is the one outcome this system must not
/// produce. Tolerating 0.1% makes anything below it accumulate NEGATIVE forever, so the whole
/// class is gone at any duration — and it costs nothing in detection, because the gap to a real
/// cheat is ~250×: a 1.05× run still nets +0.49/s and trips in ~1.7 h, and the re-login loop is
/// untouched (its `elapsed` is ~0, so the tolerance grants it ~nothing).
const TOLERATED_RATE_BP: u64 = 10_010;

/// Flag + reject once the counter passes this. ~5 min of gained time.
const EXCESS_THRESHOLD: i64 = 3_000;

/// Floor on the counter. An honest account drifts negative forever under the tolerance above,
/// and an unbounded negative is a BANKED cheat budget — a year of honest play would buy days of
/// undetected 10×. The doc says "don't floor", which is right for a floor at ZERO (positive
/// jitter would accumulate against it); once the drift tolerance lives in the rate instead, the
/// floor's only job is to bound the bank, so −5 min is both safe and necessary.
const EXCESS_FLOOR: i64 = -3_000;

/// Per-account anti-fast-forward anchor + the layer-2 bookkeeping that rides with it.
#[derive(Debug, Clone, Copy)]
struct TickAnchor {
    tick: u64,
    ms: i64,
    /// Session excess ALREADY added to the lifetime counter, so each save contributes only its
    /// increment. Excess is measured from the anchor (it spans the session), so without this a
    /// client could inflate the counter — or dodge it — purely by changing how often it saves.
    recorded: i64,
    /// False for an anchor synthesized at BOOT. Those are backdated by `RESTART_GRACE_MS` and
    /// carry no `recorded` history (the process that had it is gone), so accumulating against
    /// them would post a large bogus delta. Such an account resumes counting at its next real
    /// handshake.
    from_handshake: bool,
    /// Installed by a save that found no live anchor (round 11): counted by layer 2 like a
    /// handshake, but a layer-1 failure against it answers "reload" UNFLAGGED (round 12) — the
    /// client that reaches it honestly is one that kept running through a long server outage, and
    /// refusing the save already gains a cheater nothing.
    synthesized: bool,
}

// ==================== BATTLE PASS (P1 — season core) ====================

/// The held season if it is running NOW. The held value is loaded at boot and refreshed hourly,
/// so between its end and the next refresh it is stale (audit 2026-10-03): XP, claims and badges
/// must ask this, never `st.season` directly.
fn live_season(st: &AppState) -> Option<bp::Season> {
    let now = now_ms();
    st.season.held().clone().filter(|s| s.starts_at_ms <= now && now < s.ends_at_ms)
}

/// Hourly: pick up the next season (a process up across a season boundary kept the old one until
/// restart). A failed read keeps what is held unless that has ended.
async fn refresh_season(st: &AppState) {
    let Some(db) = &st.db else { return };
    let now = now_ms();
    let loaded = load_active_season(db, now).await;
    let mut held = st.season.held();
    match loaded {
        Some(sn) => {
            if held.as_ref().map(|h| h.season_id) != Some(sn.season_id) {
                println!("bp: season {} now active (ends_at_ms {})", sn.season_id, sn.ends_at_ms);
            }
            *held = Some(sn);
        }
        None => {
            if held.as_ref().is_some_and(|h| h.ends_at_ms <= now) {
                *held = None;
            }
        }
    }
}

/// Load the season active at `now` from the `seasons` table (latest start wins).
async fn load_active_season(db: &sqlx::PgPool, now: i64) -> Option<bp::Season> {
    let row: Option<(i64, i64, i64, i64, String)> = sqlx::query_as(
        "SELECT season_id, starts_at_ms, ends_at_ms, seed, config_json FROM seasons \
         WHERE starts_at_ms <= $1 AND ends_at_ms > $1 ORDER BY starts_at_ms DESC LIMIT 1",
    )
    .bind(now)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    let (season_id, starts_at_ms, ends_at_ms, seed, config_json) = row?;
    // Garbled config = season fails CLOSED (BP disabled), previous seasons unaffected.
    let config: bp::SeasonConfig = serde_json::from_str(&config_json).ok()?;
    Some(bp::Season { season_id, starts_at_ms, ends_at_ms, seed: seed as u64, config })
}

/// DOES THIS ACCOUNT OWN THIS SKU RIGHT NOW — the one question, asked one way.
///
/// A refund writes `revoked_at`, and until this existed that column had exactly ONE production
/// reader: the first-buy bonus count. Nothing asked whether a capability was still owned, because
/// nothing sold a capability yet. The moment one ships — the +3 rare-bag-slot SKU is next — whoever
/// writes that check writes a raw query, and a raw query that forgets `revoked_at IS NULL` hands a
/// refunded player the thing they were refunded for, permanently, with the refund looking like it
/// worked.
///
/// So the filter lives INSIDE the asking verb rather than in a rule people have to remember, and
/// `every_entitlement_read_goes_through_owns_sku` fails the build if a new query goes round it.
/// That is a source scanner, not a compiler gate — the honest strongest thing available when the
/// query is a string. It cannot stop a query built by concatenation; it can stop the ordinary way
/// of writing one.
///
/// ⚠️ ERRORS ARE NOT ANSWERED HERE, and the first version of this got that exactly backwards.
///
/// It swallowed the error and returned FALSE, with a comment calling that "the safe direction:
/// an unanswerable question about a paid capability must not grant it". True of a caller asking
/// "may they use this" — and its ONE caller asks the INVERSE: `first_buy = !owns_sku(..)`. A
/// database hiccup therefore read as "no entitlement", which is "first buy", which PAYS THE BONUS
/// AGAIN. A default cannot be safe in both directions at once, so there is no default: the caller
/// gets the error and fails the transaction.
async fn owns_sku<'e, E: sqlx::PgExecutor<'e>>(
    db: E,
    account_id: u64,
    sku: &str,
) -> Result<bool, sqlx::Error> {
    sku_purchases(db, account_id, sku).await.map(|c| c > 0)
}

/// How many unrefunded purchases of `sku` this account holds. The ONE read of `entitlements`.
async fn sku_purchases<'e, E: sqlx::PgExecutor<'e>>(
    db: E,
    account_id: u64,
    sku: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_as::<_, (i64,)>(
        "SELECT count(*) FROM entitlements WHERE account_id = $1 AND sku = $2 AND revoked_at IS NULL",
    )
    .bind(account_id as i64)
    .bind(sku)
    .fetch_one(db)
    .await
    .map(|(c,)| c)
}

/// Ensure today's `daily_progress` row exists inside `tx`, returning
/// frozen at row creation (= the accrual snapshot at that moment) so dailies read TODAY's
/// delta, never lifetime totals.
/// This account's commission day, creating it on first touch.
///
/// Keyed on (account, day) with NO season: a commission board is not a season's property. It is
/// the same eight for everyone forever, so tying its rows to a season would have made the daily
/// undeployable without one and would have reset everyone's day whenever a season rolled.
///
/// `day_start` is frozen at row creation from the previous accrual snapshot, so progress is
/// TODAY'S DELTA. That is what stops a lifetime total from satisfying a commission.
///
/// The leading `bool` is TRUE when this call CREATED the row — this is the first accepted save of
/// a new commission day for the account. That is the login edge every schedule track hangs off: it
/// fires exactly once per account per day, inside the post-verify transaction, so it means "played
/// and produced a save the server accepted" rather than "sent a request".
async fn daily_row(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: u64,
    day: i64,
    board: &[daily::DailyDef],
    now: &sim_core::state::Measures,
) -> Result<(bool, u16, u16, u16, Vec<u64>), sqlx::Error> {
    let row: Option<(i16, i16, i16, String)> = sqlx::query_as(
        "SELECT done_mask, milestone_mask, claimed_mask, start_values_json FROM daily_progress \
         WHERE account_id = $1 AND day = $2",
    )
    .bind(account_id as i64).bind(day)
    .fetch_optional(&mut **tx)
    .await?;
    if let Some((mask, ms_mask, cl_mask, sv_json)) = row {
        let start_values: Vec<u64> = serde_json::from_str(&sv_json).unwrap_or_default();
        return Ok((false, mask as u16, ms_mask as u16, cl_mask as u16, start_values));
    }
    // Opening a day captures the counters AS THEY ARE, so the day starts at zero and nothing
    // earned before the boundary counts toward it.
    let start_values = daily::open_day(board, now);
    let sv_json = serde_json::to_string(&start_values).unwrap_or_default();
    sqlx::query(
        "INSERT INTO daily_progress (account_id, day, done_mask, milestone_mask, start_values_json) \
         VALUES ($1, $2, 0, 0, $3) ON CONFLICT DO NOTHING",
    )
    .bind(account_id as i64).bind(day).bind(&sv_json)
    .execute(&mut **tx)
    .await?;
    Ok((true, 0, 0, 0, start_values))
}

/// This account's commission WEEK, creating it on first touch — the `daily_row` shape on a seven-day
/// key. Same reasoning throughout: the opening snapshot freezes at row creation so progress is the
/// WEEK'S delta, and the `NO_START` hole (via `open_day`) stops a lifetime total from finishing a
/// commission. No `bool` opened-flag is returned: the weekly does not drive schedule tracks.
async fn weekly_row(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: u64,
    week: i64,
    board: &[daily::DailyDef],
    now: &sim_core::state::Measures,
) -> Result<(u16, u16, u16, Vec<u64>), sqlx::Error> {
    let row: Option<(i16, i16, i16, String)> = sqlx::query_as(
        "SELECT done_mask, milestone_mask, claimed_mask, start_values_json FROM weekly_progress \
         WHERE account_id = $1 AND week = $2",
    )
    .bind(account_id as i64).bind(week)
    .fetch_optional(&mut **tx)
    .await?;
    if let Some((mask, ms_mask, cl_mask, sv_json)) = row {
        let start_values: Vec<u64> = serde_json::from_str(&sv_json).unwrap_or_default();
        return Ok((mask as u16, ms_mask as u16, cl_mask as u16, start_values));
    }
    let start_values = daily::open_day(board, now);
    let sv_json = serde_json::to_string(&start_values).unwrap_or_default();
    sqlx::query(
        "INSERT INTO weekly_progress (account_id, week, done_mask, milestone_mask, start_values_json) \
         VALUES ($1, $2, 0, 0, $3) ON CONFLICT DO NOTHING",
    )
    .bind(account_id as i64).bind(week).bind(&sv_json)
    .execute(&mut **tx)
    .await?;
    Ok((0, 0, 0, start_values))
}

/// Step every OPEN schedule track forward by one day, for one account.
///
/// TWO STATEMENTS, whatever the number of tracks. One login advances every open track at once, so
/// a loop would put N round trips on the save path — and that path is the measured scaling wall
/// (the whole `state_json` is upserted every 10 s per online player). The INSERT creates rows for
/// tracks this account has never touched; the UPDATE steps the ones that already exist. Neither
/// grows with the track count.
///
/// IDEMPOTENT, PER ROW. `last_day < $day` is the guard and it lives on the ROW, not on the call —
/// so an advance interrupted halfway leaves some tracks stepped and some not, and simply running
/// again finishes it without double-stepping the ones that landed. Accrual sits outside the save
/// transaction and self-heals by re-running, so that is the only shape that survives.
///
/// A new row opens at position 1, not 0: the day that created it is a day played.
async fn advance_tracks(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: u64,
    day: i64,
    tracks: &[schedule::TrackDef],
) -> Result<(), sqlx::Error> {
    // WHEN THIS ACCOUNT STARTED EACH TRACK. A `SinceStart` track's expiry is per-account, so
    // whether it is still open cannot be answered from config alone. One extra SELECT, and only
    // on the days this whole block runs at all — the first accepted save after the boundary.
    let rows: Vec<(String, i64)> =
        sqlx::query_as("SELECT track, opened_day FROM claim_progress WHERE account_id = $1")
            .bind(account_id as i64)
            .fetch_all(&mut **tx)
            .await?;
    let opened: std::collections::BTreeMap<String, i64> = rows.into_iter().collect();
    let ids: Vec<String> = schedule::active_ids(tracks, day, &opened)
        .iter()
        .map(|s| s.to_string())
        .collect();
    if ids.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO claim_progress (account_id, track, position, claimed_mask, last_day, opened_day) \
         SELECT $1, t, 1, 0, $3, $3 FROM unnest($2::text[]) AS t ON CONFLICT DO NOTHING",
    )
    .bind(account_id as i64).bind(&ids).bind(day)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE claim_progress SET position = position + 1, last_day = $3 \
         WHERE account_id = $1 AND track = ANY($2) AND last_day < $3",
    )
    .bind(account_id as i64).bind(&ids).bind(day)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Accrue BP XP from a VERIFIED final state (put_save calls this post-verify). One
/// transaction across bp_state + daily_progress; never rejects the save (self-healing anchor).
/// Post-verify accrual for a saved game: the Daily Commission first, the Battle Pass after.
///
/// Returns the crystal the daily ladder owes, which the save response carries so the HUD badge
/// lights within one save interval instead of on a separate poll — accrual happens inside this
/// very request, so the moment the server learns a rung was reached it is already replying.
///
/// Add `cxp` to an account's Commander total (creating the row). Levels and their rewards are still
/// claimed on `/commander/claim`.
async fn add_commander_cxp(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: u64,
    cxp: i64,
) -> Result<(), sqlx::Error> {
    if cxp <= 0 {
        return Ok(());
    }
    sqlx::query("INSERT INTO commander (account_id) VALUES ($1) ON CONFLICT (account_id) DO NOTHING")
        .bind(account_id as i64).execute(&mut **tx).await?;
    sqlx::query("UPDATE commander SET cxp = cxp + $2 WHERE account_id = $1")
        .bind(account_id as i64).bind(cxp).execute(&mut **tx).await?;
    Ok(())
}

/// Commander CXP for every stage a verified save reached past the `commander_stage` watermark.
/// The watermark row is locked, so two saves of one account cannot pay the same stage twice; a
/// save behind the mark pays nothing.
async fn commander_stage_cxp(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: u64,
    band_reached: u32,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO commander_stage (account_id) VALUES ($1) ON CONFLICT (account_id) DO NOTHING")
        .bind(account_id as i64).execute(&mut **tx).await?;
    let (paid,): (i32,) = sqlx::query_as("SELECT reached FROM commander_stage WHERE account_id = $1 FOR UPDATE")
        .bind(account_id as i64).fetch_one(&mut **tx).await?;
    let paid = paid.max(0) as u32;
    let stages = band_reached.saturating_sub(paid);
    if stages == 0 {
        return Ok(());
    }
    sqlx::query("UPDATE commander_stage SET reached = $2 WHERE account_id = $1")
        .bind(account_id as i64).bind(band_reached as i32).execute(&mut **tx).await?;
    add_commander_cxp(tx, account_id, i64::from(stages) * commander::CXP_PER_NEW_STAGE).await
}

/// The account's last verified save as a `GameState` — the in-memory cache first, then the table.
async fn verified_state<'e, E: sqlx::PgExecutor<'e>>(
    st: &AppState,
    db: E,
    account_id: u64,
) -> Result<Option<sim_core::state::GameState>, sqlx::Error> {
    let json = st.saves.held().get(&account_id).map(|s| s.state_json.clone());
    let json = match json {
        Some(j) => Some(j),
        None => sqlx::query_as::<_, (String,)>("SELECT state_json FROM saves WHERE account_id = $1")
            .bind(account_id as i64).fetch_optional(db).await?.map(|(j,)| j),
    };
    Ok(json.and_then(|j| serde_json::from_str::<sim_core::state::GameState>(&j).ok()))
}

/// The facts the tutorial quests read: the save's measures, stage and party, plus the server's
/// own Starter pulls, daily claims and Commander claims.
async fn tutorial_facts(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: u64,
    state: Option<&sim_core::state::GameState>,
) -> Result<tutorial::Facts, sqlx::Error> {
    let mut f = tutorial::Facts::default();
    if let Some(g) = state {
        f.measures = g.measures.clone();
        f.band_reached = g.band_reached;
        f.party_size = g.grid.entities.iter()
            .filter(|e| matches!(e.team, sim_core::grid::Team::Player) && !e.is_pet)
            .count() as u32;
        f.most_gear = g.grid.entities.iter()
            .filter(|e| matches!(e.team, sim_core::grid::Team::Player) && !e.is_pet)
            .map(|e| e.equipped.len() as u32)
            .max()
            .unwrap_or(0);
    }
    let pulls: Option<(i32,)> = sqlx::query_as("SELECT made FROM gacha_newcomer WHERE account_id = $1")
        .bind(account_id as i64).fetch_optional(&mut **tx).await?;
    f.starter_pulls = pulls.map_or(0, |r| r.0.max(0) as u32);
    let (rungs,): (i64,) = sqlx::query_as("SELECT count(*) FROM daily_progress WHERE account_id = $1 AND claimed_mask <> 0")
        .bind(account_id as i64).fetch_one(&mut **tx).await?;
    f.daily_rung_claimed = rungs > 0;
    let cmd: Option<(i32,)> = sqlx::query_as("SELECT reward_claimed_through FROM commander WHERE account_id = $1")
        .bind(account_id as i64).fetch_optional(&mut **tx).await?;
    f.commander_claimed_through = cmd.map_or(0, |r| r.0.max(0) as u32);
    let (story,): (i64,) = sqlx::query_as("SELECT count(*) FROM story_claims WHERE account_id = $1")
        .bind(account_id as i64).fetch_one(&mut **tx).await?;
    f.story_cleared = story.max(0) as u32;
    Ok(f)
}

/// Quest 1's weapon: a Rare-grade tier-1 weapon of the lead's weapon family (the first fielded
/// member, its worn Exemplar deciding the family), bound.
fn lead_weapon_item(st: &AppState, state: &sim_core::state::GameState) -> Option<sim_core::items::Item> {
    let lead = state.grid.entities.iter()
        .find(|e| matches!(e.team, sim_core::grid::Team::Player) && !e.is_pet)?;
    let ex = lead.exemplar.as_ref().map(|x| x.id.as_str());
    let wt = *sim_core::items::weapon_types_for_character(ex, &lead.unit_def).first()?;
    let mut it = st.data.items.iter()
        .filter(|d| d.weapon_type == wt)
        .map(data::instantiate_item)
        .find(|it| it.item_level() == 1)?;
    let overrides: Vec<sim_core::data::GearOverride> =
        st.data.gear_overrides.iter().filter(|o| o.def_id == it.def_id).cloned().collect();
    let lines = sim_core::substats::lines_by_grade(&st.data.loot_rules.substats, &it.def_id, it.slot());
    it.assign_grade(sim_core::items::Grade::Rare, &overrides, &lines);
    it.bound = true;
    Some(it)
}

fn tutorial_reward_view(r: &tutorial::Reward) -> serde_json::Value {
    match r {
        tutorial::Reward::Tickets(n) => serde_json::json!({"kind": "tickets", "ticket_kind": "standard", "qty": n}),
        tutorial::Reward::Cxp(n) => serde_json::json!({"kind": "cxp", "qty": n}),
        tutorial::Reward::Gil(n) => serde_json::json!({"kind": "gil", "qty": n}),
        tutorial::Reward::Item { def_id, qty } => serde_json::json!({"kind": "item", "def_id": def_id, "qty": qty}),
        tutorial::Reward::LeadWeapon => serde_json::json!({"kind": "lead_weapon", "grade": "Rare"}),
    }
}

/// `GET /tutorial/state` — every quest with its chapter, done / claimed / claimable, and rewards;
/// `open_chapter` is the lowest chapter with an unclaimed quest (later chapters are locked).
async fn tutorial_state(State(st): State<AppState>, headers: HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "no database"})));
    };
    let res = async {
        let state = verified_state(&st, db, acct.account_id).await?;
        let mut tx = db.begin().await?;
        let f = tutorial_facts(&mut tx, acct.account_id, state.as_ref()).await?;
        let claimed: Option<(i64,)> = sqlx::query_as("SELECT claimed FROM tutorial_progress WHERE account_id = $1")
            .bind(acct.account_id as i64).fetch_optional(&mut *tx).await?;
        tx.commit().await?;
        Ok::<_, sqlx::Error>((f, claimed.map_or(0, |r| r.0 as u64)))
    }.await;
    let (f, claimed) = match res {
        Ok(v) => v,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    let qs = tutorial::quests();
    let open = tutorial::open_chapter(&qs, claimed);
    let quests: Vec<serde_json::Value> = qs.iter().map(|q| serde_json::json!({
        "id": q.id,
        "chapter": q.chapter,
        "done": tutorial::is_done(q.cond, &f),
        "claimed": claimed & (1u64 << q.bit) != 0,
        "claimable": tutorial::claimable(q, &qs, claimed, &f),
        "rewards": q.rewards.iter().map(tutorial_reward_view).collect::<Vec<_>>(),
    })).collect();
    (StatusCode::OK, Json(serde_json::json!({"ok": true, "open_chapter": open, "quests": quests})))
}

#[derive(serde::Deserialize)]
struct TutorialClaimReq {
    id: String,
}

/// `POST /tutorial/claim {id}` — pay one done quest of an open chapter, once. Standard tickets and
/// CXP credit directly; Gil and items arrive by mail, bound. The claimed bit and every payout share
/// one transaction, so a claim that records cannot fail to pay and vice versa.
async fn tutorial_claim(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<TutorialClaimReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "no database"})));
    };
    let Some(q) = tutorial::quest(&req.id) else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"ok": false, "reason": "unknown quest"})));
    };
    let account_id = acct.account_id;
    let res = async {
        let state = verified_state(&st, db, account_id).await?;
        let mut tx = db.begin().await?;
        lock_account(&mut tx, acct.account_id as i64).await;
        sqlx::query("INSERT INTO tutorial_progress (account_id) VALUES ($1) ON CONFLICT (account_id) DO NOTHING")
            .bind(account_id as i64).execute(&mut *tx).await?;
        let (claimed,): (i64,) = sqlx::query_as("SELECT claimed FROM tutorial_progress WHERE account_id = $1 FOR UPDATE")
            .bind(account_id as i64).fetch_one(&mut *tx).await?;
        let claimed = claimed as u64;
        let mut f = tutorial_facts(&mut tx, account_id, state.as_ref()).await?;
        f.client_reported = q.cond == tutorial::Cond::ClientReported;
        let qs = tutorial::quests();
        if claimed & (1u64 << q.bit) != 0 {
            return Ok::<Result<(), &str>, sqlx::Error>(Err("already claimed"));
        }
        if !tutorial::claimable(&q, &qs, claimed, &f) {
            return Ok(Err("not done yet"));
        }
        let reason = format!("tutorial {}", q.id);
        for r in &q.rewards {
            match r {
                tutorial::Reward::Tickets(n) => {
                    sqlx::query(
                        "INSERT INTO bound_items (account_id, kind, qty) VALUES ($1, $2, $3) \
                         ON CONFLICT (account_id, kind) DO UPDATE SET qty = bound_items.qty + EXCLUDED.qty",
                    )
                    .bind(account_id as i64).bind(gacha::STANDARD_COUPON_KIND).bind(i64::from(*n))
                    .execute(&mut *tx).await?;
                    sqlx::query("INSERT INTO bound_ledger (account_id, kind, delta, reason, ref) VALUES ($1, $2, $3, 'tutorial', $4)")
                        .bind(account_id as i64).bind(gacha::STANDARD_COUPON_KIND).bind(i64::from(*n)).bind(q.id)
                        .execute(&mut *tx).await?;
                }
                tutorial::Reward::Cxp(n) => add_commander_cxp(&mut tx, account_id, *n).await?,
                tutorial::Reward::Gil(n) => {
                    mailbox_store::send(&mut *tx, account_id, mailbox::MailKind::Reward, *n, "", 0, &reason)
                        .await.map_err(|e| sqlx::Error::Protocol(format!("{e:?}")))?;
                }
                tutorial::Reward::Item { def_id, qty } => {
                    let def = st.data.item(def_id).ok_or_else(|| sqlx::Error::Protocol(format!("{def_id} not in the catalog")))?;
                    let mut it = data::instantiate_item(def);
                    it.quantity = (*qty).max(1);
                    it.bound = true;
                    mailbox_store::send_item(&mut *tx, account_id, mailbox::MailKind::Reward, &it, &reason).await?;
                }
                tutorial::Reward::LeadWeapon => {
                    let it = state.as_ref().and_then(|g| lead_weapon_item(&st, g))
                        .ok_or_else(|| sqlx::Error::Protocol("no lead weapon family".into()))?;
                    mailbox_store::send_item(&mut *tx, account_id, mailbox::MailKind::Reward, &it, &reason).await?;
                }
            }
        }
        sqlx::query("UPDATE tutorial_progress SET claimed = claimed | $2 WHERE account_id = $1")
            .bind(account_id as i64).bind((1u64 << q.bit) as i64).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Ok(()))
    }.await;
    match res {
        Ok(Ok(())) => (StatusCode::OK, Json(serde_json::json!({
            "ok": true, "id": q.id,
            "rewards": q.rewards.iter().map(tutorial_reward_view).collect::<Vec<_>>(),
        }))),
        Ok(Err(why)) => (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": why}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    }
}

/// OFFLINE EXP for the time since the previous save (`away_ms`; None = no previous save, nothing
/// owed). The client saves every 10 s, so a gap of `OFFLINE_MIN_MINUTES` is time away, not a
/// reconnect. Paid as an `offline_exp` envelope (EXP per fielded member, rate from the verified
/// save's latest band), mailed so the claim is reconciled like every other server payout.
async fn offline_exp_mail(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    st: &AppState,
    account_id: u64,
    final_state: &sim_core::state::GameState,
    away_ms: Option<i64>,
) -> Result<Option<mailbox::MailItem>, sqlx::Error> {
    let Some(away) = away_ms else { return Ok(None) };
    let minutes = (away.max(0) / 60_000) as u64;
    let xp = sim_core::tick::offline_xp_per_member(final_state, &st.data, minutes);
    let Some(def) = st.data.item(OFFLINE_EXP_DEF).filter(|_| xp > 0) else { return Ok(None) };
    let mut it = data::instantiate_item(def);
    it.quantity = u32::try_from(xp).unwrap_or(u32::MAX);
    it.bound = true;
    let reason = format!("offline {}m", minutes.min(sim_core::tick::OFFLINE_MAX_MINUTES));
    let id = mailbox_store::send_item(&mut **tx, account_id, mailbox::MailKind::Reward, &it, &reason).await?;
    Ok(Some(mailbox::MailItem {
        id, account_id, kind: mailbox::MailKind::Reward, gil: 0, def_id: String::new(), qty: 0, item: Some(it), reason,
    }))
}

const OFFLINE_EXP_DEF: &str = "offline_exp";

/// Mail the offline-EXP envelope for `now − max(saved_at_ms, through_ms)` and move `through_ms`
/// to now, in one commit (a repeat login pays only time since the last one).
async fn pay_offline_at_login(
    db: &sqlx::PgPool,
    st: &AppState,
    account_id: u64,
    saved_at_ms: i64,
    state: &sim_core::state::GameState,
) -> Result<(), sqlx::Error> {
    let now = now_ms();
    let mut tx = db.begin().await?;
    lock_account(&mut tx, account_id as i64).await;
    let through: Option<(i64,)> = sqlx::query_as("SELECT through_ms FROM offline_paid WHERE account_id = $1 FOR UPDATE")
        .bind(account_id as i64).fetch_optional(&mut *tx).await?;
    // The save's stored-at from its ROW, not memory (round-9 audit): a save whose commit landed
    // but whose memory update was cut off by a timeout would otherwise pay its play as offline.
    let row_at: Option<(i64,)> = sqlx::query_as("SELECT saved_at_ms FROM saves WHERE account_id = $1")
        .bind(account_id as i64).fetch_optional(&mut *tx).await?;
    let from = saved_at_ms.max(row_at.map_or(0, |(t,)| t)).max(through.map_or(0, |(t,)| t));
    sqlx::query("INSERT INTO offline_paid (account_id, through_ms) VALUES ($1, $2) \
                 ON CONFLICT (account_id) DO UPDATE SET through_ms = EXCLUDED.through_ms")
        .bind(account_id as i64).bind(now).execute(&mut *tx).await?;
    let _ = offline_exp_mail(&mut tx, st, account_id, state, Some((now - from).max(0))).await?;
    tx.commit().await
}

/// `GET /badges` — every server-held "something to claim" count in one call, for the client's
/// red dots. Read-only; each number is computed exactly as its own endpoint computes it, so a dot
/// never promises what the claim then refuses. Save-held badges (stat points, bag full) are in
/// the sim-host view, not here.
async fn badges(State(st): State<AppState>, headers: HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "no database"})));
    };
    let account_id = acct.account_id;
    let id = account_id as i64;
    let res = async {
        let now = now_ms();
        let day = daily::day_of_ms(now as u64);
        let week = weekly::week_of_ms(now as u64);

        // Daily / weekly ladders: rungs reached but not claimed.
        let ladder_rungs = |row: Option<(i16, i16)>, board: &[daily::DailyDef], ladder: &[daily::Milestone]| {
            let (done, claimed) = row.map_or((0u16, 0u16), |(d, c)| (d as u16, c as u16));
            let points = daily::points_of(board, ladder, done);
            daily::claimable(ladder, points, claimed).0.count_ones()
        };
        let d: Option<(i16, i16)> = sqlx::query_as("SELECT done_mask, claimed_mask FROM daily_progress WHERE account_id = $1 AND day = $2")
            .bind(id).bind(day).fetch_optional(db).await?;
        let daily_rungs = ladder_rungs(d, &daily::board(), &daily::ladder());
        let w: Option<(i16, i16)> = sqlx::query_as("SELECT done_mask, claimed_mask FROM weekly_progress WHERE account_id = $1 AND week = $2")
            .bind(id).bind(week).fetch_optional(db).await?;
        let weekly_rungs = ladder_rungs(w, &weekly::board(), &weekly::ladder());

        // Schedule tracks (/claim/state): reached-but-unpaid cells over every active track.
        let rows: Vec<(String, i32, i64, i64)> = sqlx::query_as(
            "SELECT track, position, claimed_mask, opened_day FROM claim_progress WHERE account_id = $1",
        ).bind(id).fetch_all(db).await?;
        let tracks: usize = rows.iter()
            .filter_map(|(tid, pos, mask, opened)| {
                let t = st.tracks.iter().find(|t| &t.id == tid && t.is_active(day, Some(*opened)))?;
                Some(schedule::owed(&t.entries, (*pos).max(0) as u32, *mask as u64).1.len())
            })
            .sum();

        // Battle Pass: reached, unclaimed slots per track (premium only when owned).
        let season = live_season(&st);
        let (mut bp_free, mut bp_premium) = (0u32, 0u32);
        if let Some(season) = season {
            let xp: Option<(i64,)> = sqlx::query_as("SELECT xp FROM bp_state WHERE account_id = $1 AND season_id = $2")
                .bind(id).bind(season.season_id).fetch_optional(db).await?;
            let level = bp::level_of(&season.config, xp.map_or(0, |r| r.0.max(0) as u64));
            let unclaimed = |claimed: Vec<u32>| (1..=level).filter(|s| !claimed.contains(s)).count() as u32;
            bp_free = unclaimed(bp_claimed_slots(db, account_id, season.season_id, 0).await);
            if bp_premium_owned(db, account_id, season.season_id).await? {
                bp_premium = unclaimed(bp_claimed_slots(db, account_id, season.season_id, 1).await);
            }
        }

        let (mail,): (i64,) = sqlx::query_as("SELECT count(*) FROM mailbox WHERE account_id = $1 AND claimed_at IS NULL AND voided_at IS NULL")
            .bind(id).fetch_one(db).await?;

        let cmd: Option<(i64, i32)> = sqlx::query_as("SELECT cxp, reward_claimed_through FROM commander WHERE account_id = $1")
            .bind(id).fetch_optional(db).await?;
        let (cxp, through) = cmd.unwrap_or((0, 0));
        let level = commander::level_for_cxp(&st.commander_levels, cxp);
        let commander_levels = level.saturating_sub(through.max(0) as u32);

        let card: Option<(i64, i64)> = sqlx::query_as("SELECT ends_day, last_claim_day FROM monthly_card WHERE account_id = $1")
            .bind(id).fetch_optional(db).await?;
        let monthly = card.is_some_and(|(ends, last)| ends > day && last < day);

        let state = verified_state(&st, db, account_id).await?;
        let mut tx = db.begin().await?;
        let f = tutorial_facts(&mut tx, account_id, state.as_ref()).await?;
        tx.commit().await?;
        let tclaimed: Option<(i64,)> = sqlx::query_as("SELECT claimed FROM tutorial_progress WHERE account_id = $1")
            .bind(id).fetch_optional(db).await?;
        let tclaimed = tclaimed.map_or(0, |r| r.0 as u64);
        let qs = tutorial::quests();
        let tutorial_n = qs.iter().filter(|q| tutorial::claimable(q, &qs, tclaimed, &f)).count();

        let std_sel: Option<(i32, Option<String>)> = sqlx::query_as("SELECT made, selected FROM gacha_standard WHERE account_id = $1")
            .bind(id).fetch_optional(db).await?;
        let standard_select = std_sel.is_some_and(|(made, sel)| made.max(0) as u32 >= gacha::STANDARD_SELECT_AT && sel.is_none());
        let ticket = |kind: &'static str| async move {
            sqlx::query_as::<_, (i64,)>("SELECT qty FROM bound_items WHERE account_id = $1 AND kind = $2")
                .bind(id).bind(kind).fetch_optional(db).await.map(|r| r.map_or(0, |r| r.0.max(0)))
        };
        let tickets_std = ticket(gacha::STANDARD_COUPON_KIND).await?;
        let tickets_limited = ticket(gacha::LIMITED_COUPON_KIND).await?;
        // Mint coupons a claim would collect right now (0 when the held balance is full).
        let clock: Option<(i64,)> = sqlx::query_as("SELECT anchor_ms FROM mint_coupon_clock WHERE account_id = $1")
            .bind(id).fetch_optional(db).await?;
        let mint_held = u32::try_from(ticket(mint_coupon::KIND).await?).unwrap_or(u32::MAX);
        let mint_coupons = clock.map_or(0, |(a,)| mint_coupon::pending(a, now).0)
            .min(mint_coupon::HELD_CAP.saturating_sub(mint_held));

        Ok::<_, sqlx::Error>(serde_json::json!({
            "ok": true,
            "daily_rungs": daily_rungs,
            "weekly_rungs": weekly_rungs,
            "tracks": tracks,
            "bp_free": bp_free,
            "bp_premium": bp_premium,
            "mail": mail,
            "commander_levels": commander_levels,
            "monthly": monthly,
            "tutorial": tutorial_n,
            "standard_select": standard_select,
            "tickets_std": tickets_std,
            "tickets_limited": tickets_limited,
            "mint_coupons": mint_coupons,
        }))
    }.await;
    match res {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    }
}

/// `GET /shop` — the crystal shelf and, with a token, what this account bought of each today.
async fn shop_state(State(st): State<AppState>, headers: HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
    let acct = account_of(&st, &headers);
    let mut bought: HashMap<String, i64> = HashMap::new();
    if let (Some(acct), Some(db)) = (&acct, &st.db) {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT def_id, sum(qty)::bigint FROM shop_buys WHERE account_id = $1 AND day = $2 GROUP BY def_id",
        ).bind(acct.account_id as i64).bind(shop_day()).fetch_all(db).await.unwrap_or_default();
        bought.extend(rows);
    }
    let items: Vec<serde_json::Value> = shop::SHELF.iter().map(|r| {
        let today = bought.get(r.def_id).copied().unwrap_or(0);
        serde_json::json!({
            "def_id": r.def_id,
            "name": st.data.item(r.def_id).map(|d| d.name.clone()).unwrap_or_default(),
            "crystal": r.crystal,
            "daily_limit": r.daily_limit,
            "bought_today": today,
            "left_today": i64::from(r.daily_limit).saturating_sub(today).max(0),
        })
    }).collect();
    (StatusCode::OK, Json(serde_json::json!({"ok": true, "items": items})))
}

/// `GET /mint-coupons` — held / uncollected coupons, the caps, and when the next one accrues.
/// `room` is how many a claim would take now, so the client can warn before a claim that leaves
/// coupons behind (user: "ถ้า claim แล้วเกินจะบอกผู้เล่นก่อน").
async fn mint_coupons_state(State(st): State<AppState>, headers: HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "needs a database"})));
    };
    let id = acct.account_id as i64;
    let res = async {
        let anchor: Option<(i64,)> = sqlx::query_as("SELECT anchor_ms FROM mint_coupon_clock WHERE account_id = $1")
            .bind(id).fetch_optional(db).await?;
        let held: Option<(i64,)> = sqlx::query_as("SELECT qty FROM bound_items WHERE account_id = $1 AND kind = $2")
            .bind(id).bind(mint_coupon::KIND).fetch_optional(db).await?;
        Ok::<_, sqlx::Error>((anchor.map(|(a,)| a), held.map_or(0, |(q,)| q.max(0))))
    }.await;
    let (anchor, held) = match res {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    let now = now_ms();
    let anchor = anchor.unwrap_or(now);
    let (pending, _) = mint_coupon::pending(anchor, now);
    let held = u32::try_from(held).unwrap_or(u32::MAX);
    (StatusCode::OK, Json(serde_json::json!({
        "ok": true,
        "held": held,
        "pending": pending,
        "held_cap": mint_coupon::HELD_CAP,
        "pending_cap": mint_coupon::PENDING_CAP,
        "room": pending.min(mint_coupon::HELD_CAP.saturating_sub(held)),
        "next_at_ms": mint_coupon::next_at(anchor, now),
        "now_ms": now,
    })))
}

/// `POST /mint-coupons/claim` — move uncollected coupons into the held balance, up to the cap;
/// what does not fit stays uncollected. One transaction, ledgered.
async fn mint_coupons_claim(State(st): State<AppState>, headers: HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "needs a database"})));
    };
    let id = acct.account_id as i64;
    let now = now_ms();
    let res = async {
        let mut tx = db.begin().await?;
        lock_account(&mut tx, id).await;
        sqlx::query("INSERT INTO mint_coupon_clock (account_id, anchor_ms) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(id).bind(now).execute(&mut *tx).await?;
        let (anchor,): (i64,) = sqlx::query_as("SELECT anchor_ms FROM mint_coupon_clock WHERE account_id = $1 FOR UPDATE")
            .bind(id).fetch_one(&mut *tx).await?;
        let held: Option<(i64,)> = sqlx::query_as("SELECT qty FROM bound_items WHERE account_id = $1 AND kind = $2 FOR UPDATE")
            .bind(id).bind(mint_coupon::KIND).fetch_optional(&mut *tx).await?;
        let held = u32::try_from(held.map_or(0, |(q,)| q.max(0))).unwrap_or(u32::MAX);
        let (take, new_anchor) = mint_coupon::claim(anchor, held, now);
        if take > 0 {
            sqlx::query("UPDATE mint_coupon_clock SET anchor_ms = $2 WHERE account_id = $1")
                .bind(id).bind(new_anchor).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO bound_items (account_id, kind, qty) VALUES ($1, $2, $3) \
                         ON CONFLICT (account_id, kind) DO UPDATE SET qty = bound_items.qty + EXCLUDED.qty")
                .bind(id).bind(mint_coupon::KIND).bind(i64::from(take)).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO bound_ledger (account_id, kind, delta, reason, ref) VALUES ($1, $2, $3, 'mint-coupon-claim', '')")
                .bind(id).bind(mint_coupon::KIND).bind(i64::from(take)).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        let left = mint_coupon::pending(if take > 0 { new_anchor } else { anchor }, now).0;
        Ok::<_, sqlx::Error>((take, held + take, left))
    }.await;
    match res {
        Ok((claimed, held, pending)) => (StatusCode::OK, Json(serde_json::json!({
            "ok": true, "claimed": claimed, "held": held, "pending": pending,
        }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    }
}

/// Least time between two Steam inventory reads for one account.
const STEAM_READ_GAP_MS: i64 = 5_000;
/// Re-sent consumes before an import still answering `Unknown` is left for a person.
const IMPORT_MAX_ATTEMPTS: i32 = 10;
/// The mail reason every import carries — the client's mailbox "From Steam" tab filters on it.
const STEAM_IMPORT_REASON: &str = "steam import";

/// Who may import, or the refusal: a database, the Steam switch + key, a Steam login, and no read
/// in the last `STEAM_READ_GAP_MS`.
fn steam_import_gate(st: &AppState, acct: &Account) -> Result<(String, u64), (StatusCode, Json<serde_json::Value>)> {
    let refuse = |code: StatusCode, reason: &str| (code, Json(serde_json::json!({"ok": false, "reason": reason})));
    if st.db.is_none() {
        return Err(refuse(StatusCode::SERVICE_UNAVAILABLE, "needs a database"));
    }
    let key = std::env::var("STEAM_PUBLISHER_KEY").ok().filter(|k| !k.is_empty());
    let Some(key) = key.filter(|_| steam_inv::live(std::env::var("STEAM_INVENTORY_LIVE").ok().as_deref(), true)) else {
        return Err(refuse(StatusCode::SERVICE_UNAVAILABLE, "steam inventory is off"));
    };
    let Some(steamid) = steamid_of(acct) else {
        return Err(refuse(StatusCode::BAD_REQUEST, "not a Steam account"));
    };
    let now = now_ms();
    let mut reads = st.steam_reads.held();
    if reads.get(&acct.account_id).is_some_and(|&t| now - t < STEAM_READ_GAP_MS) {
        return Err(refuse(StatusCode::TOO_MANY_REQUESTS, "too soon"));
    }
    reads.insert(acct.account_id, now);
    Ok((key, steamid))
}

/// The catalog def a Steam itemdef stands for — only items this game mints map back.
fn def_of_itemdef(data: &GameData, itemdefid: i64) -> Option<&sim_core::data::ItemDef> {
    data.items.iter().find(|d| d.steam_itemdef.is_some_and(|x| i64::from(x) == itemdefid))
}

/// `GET /steam/inventory` — the player's Steam items that can come into the game, read from
/// Steam by the server. Settles any import whose consume answer was lost first, so a settled
/// item shows in the mailbox rather than here.
async fn steam_inventory(State(st): State<AppState>, headers: HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let (key, steamid) = match steam_import_gate(&st, &acct) { Ok(g) => g, Err(r) => return r };
    let Some(db) = &st.db else { return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false}))) };
    let settled = settle_steam_imports(db, &st, &acct, &key, steamid).await;
    let items = match steam_get_inventory(&key, steamid).await {
        Ok(i) => i,
        Err(e) => return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"ok": false, "reason": format!("steam: {e}")}))),
    };
    let open: Vec<(String,)> = sqlx::query_as("SELECT steam_itemid FROM steam_imports WHERE account_id = $1 AND status = 'consuming'")
        .bind(acct.account_id as i64).fetch_all(db).await.unwrap_or_default();
    let open: HashSet<String> = open.into_iter().map(|(i,)| i).collect();
    let list: Vec<serde_json::Value> = items.iter().filter_map(|it| {
        let def = def_of_itemdef(&st.data, it.itemdefid)?;
        Some(serde_json::json!({
            "steam_itemid": it.itemid, "itemdefid": it.itemdefid, "def_id": def.id,
            "quantity": it.quantity, "importing": open.contains(&it.itemid),
        }))
    }).collect();
    (StatusCode::OK, Json(serde_json::json!({"ok": true, "items": list, "mails": settled})))
}

#[derive(serde::Deserialize)]
struct SteamImportReq {
    steam_itemids: Vec<String>,
}

/// `POST /steam/import {steam_itemids}` — take Steam items into the game: free, no wait, and the
/// game item stays tradeable. Each id must be in the player's Steam inventory as the server reads
/// it and map to a catalog def. Answers the delivered `mails` (the client claims them at once),
/// plus ids still `pending` (Steam's answer was lost; settled on the next read) and `refused`.
async fn steam_import_items(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SteamImportReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    if req.steam_itemids.is_empty() || req.steam_itemids.len() > steam_import::IMPORT_PER_REQUEST {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"ok": false, "reason": "1..=50 items per import"})));
    }
    let (key, steamid) = match steam_import_gate(&st, &acct) { Ok(g) => g, Err(r) => return r };
    let Some(db) = &st.db else { return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false}))) };
    let mut mails = settle_steam_imports(db, &st, &acct, &key, steamid).await;
    let held = match steam_get_inventory(&key, steamid).await {
        Ok(i) => i,
        Err(e) => return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"ok": false, "reason": format!("steam: {e}")}))),
    };
    let (mut pending, mut refused) = (Vec::new(), Vec::new());
    let wanted: std::collections::BTreeSet<&String> = req.steam_itemids.iter().collect();
    for id in wanted {
        let Some((it, def)) = held.iter().find(|h| &h.itemid == id)
            .and_then(|h| def_of_itemdef(&st.data, h.itemdefid).map(|d| (h, d))) else {
            refused.push(id.clone());
            continue;
        };
        let qty = it.quantity.min(steam_import::IMPORT_PER_REQUEST as u32);
        // Claimed BEFORE Steam is asked: the UNIQUE id stops a second request consuming beside it.
        let row: Option<(i64,)> = match sqlx::query_as(
            "INSERT INTO steam_imports (steam_itemid, account_id, def_id, qty) VALUES ($1, $2, $3, $4) \
             ON CONFLICT (steam_itemid) DO NOTHING RETURNING id",
        ).bind(id).bind(acct.account_id as i64).bind(&def.id).bind(qty as i32).fetch_optional(db).await {
            Ok(r) => r,
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
        };
        let Some((row_id,)) = row else { pending.push(id.clone()); continue };
        match finish_steam_import(db, &st, &acct, &key, steamid, row_id, id, &def.id, qty).await {
            ImportOutcome::Delivered(m) => mails.extend(m),
            ImportOutcome::Refused => refused.push(id.clone()),
            ImportOutcome::Pending => pending.push(id.clone()),
        }
    }
    (StatusCode::OK, Json(serde_json::json!({"ok": true, "mails": mails, "pending": pending, "refused": refused})))
}

enum ImportOutcome {
    Delivered(Vec<mailbox::MailItem>),
    Refused,
    Pending,
}

/// Ask Steam to consume the claimed row's item and act on the answer. Re-sent with the row's id as
/// `requestid`, so a retry of a consume that already happened is answered, not repeated.
#[allow(clippy::too_many_arguments)]
async fn finish_steam_import(
    db: &sqlx::PgPool, st: &AppState, acct: &Account, key: &str, steamid: u64,
    row_id: i64, itemid: &str, def_id: &str, qty: u32,
) -> ImportOutcome {
    let _ = sqlx::query("UPDATE steam_imports SET attempts = attempts + 1 WHERE id = $1").bind(row_id).execute(db).await;
    match steam_consume_item(key, steamid, itemid, qty, row_id).await {
        steam_import::ConsumeResult::Consumed => match deliver_steam_import(db, st, acct.account_id, row_id, def_id, qty).await {
            Ok(m) => ImportOutcome::Delivered(m),
            Err(e) => {
                // Consumed on Steam, not yet mailed: the row stays `consuming` and the next read
                // re-sends the same requestid, which answers Consumed again and delivers.
                eprintln!("[steam-import] {itemid} consumed but not delivered: {e} — retried on the next read");
                ImportOutcome::Pending
            }
        },
        steam_import::ConsumeResult::Refused(why) => {
            let _ = sqlx::query("DELETE FROM steam_imports WHERE id = $1 AND status = 'consuming'").bind(row_id).execute(db).await;
            eprintln!("[steam-import] {itemid} refused: {why}");
            ImportOutcome::Refused
        }
        steam_import::ConsumeResult::Unknown(why) => {
            let _ = sqlx::query("UPDATE steam_imports SET note = $2 WHERE id = $1").bind(row_id).bind(&why).execute(db).await;
            eprintln!("[steam-import] {itemid} outcome unknown: {why}");
            ImportOutcome::Pending
        }
    }
}

/// Mail `qty` tradeable copies of `def_id` and mark the row `done`, in one transaction. A row
/// already `done` delivers nothing (two settles racing).
async fn deliver_steam_import(
    db: &sqlx::PgPool, st: &AppState, account_id: u64, row_id: i64, def_id: &str, qty: u32,
) -> Result<Vec<mailbox::MailItem>, sqlx::Error> {
    let Some(def) = st.data.item(def_id) else {
        return Err(sqlx::Error::Protocol(format!("'{def_id}' left the catalog")));
    };
    let mut tx = db.begin().await?;
    lock_account(&mut tx, account_id as i64).await;
    let n = sqlx::query("UPDATE steam_imports SET status = 'done', note = NULL WHERE id = $1 AND status = 'consuming'")
        .bind(row_id).execute(&mut *tx).await?.rows_affected();
    let mut out = Vec::new();
    if n == 1 {
        for _ in 0..qty.max(1) {
            let mut it = data::instantiate_item(def);
            it.bound = false;
            let id = mailbox_store::send_item(&mut *tx, account_id, mailbox::MailKind::Reward, &it, STEAM_IMPORT_REASON).await?;
            out.push(mailbox::MailItem {
                id, account_id, kind: mailbox::MailKind::Reward, gil: 0, def_id: String::new(), qty: 0,
                item: Some(it), reason: STEAM_IMPORT_REASON.into(),
            });
        }
    }
    tx.commit().await?;
    Ok(out)
}

/// Re-send the consume of every import of this account still `consuming` (its answer was lost),
/// fewest attempts first. One left for a person after `IMPORT_MAX_ATTEMPTS`.
async fn settle_steam_imports(db: &sqlx::PgPool, st: &AppState, acct: &Account, key: &str, steamid: u64) -> Vec<mailbox::MailItem> {
    let rows: Vec<(i64, String, String, i32)> = sqlx::query_as(
        "SELECT id, steam_itemid, def_id, qty FROM steam_imports \
         WHERE account_id = $1 AND status = 'consuming' AND attempts < $2 ORDER BY attempts, id LIMIT 10",
    ).bind(acct.account_id as i64).bind(IMPORT_MAX_ATTEMPTS).fetch_all(db).await.unwrap_or_default();
    let mut mails = Vec::new();
    for (row_id, itemid, def_id, qty) in rows {
        if let ImportOutcome::Delivered(m) =
            finish_steam_import(db, st, acct, key, steamid, row_id, &itemid, &def_id, u32::try_from(qty).unwrap_or(1)).await
        {
            mails.extend(m);
        }
    }
    mails
}

/// `IInventoryService/GetInventory` for one player (publisher key: any player of this app).
async fn steam_get_inventory(key: &str, steamid: u64) -> Result<Vec<steam_import::SteamItem>, String> {
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build().map_err(|e| e.to_string())?;
    let resp = client.get("https://partner.steam-api.com/IInventoryService/GetInventory/v1/")
        .query(&[("key", key.to_string()), ("appid", STEAM_APP_ID.to_string()), ("steamid", steamid.to_string())])
        .send().await.map_err(|e| e.without_url().to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let body = resp.json::<serde_json::Value>().await.map_err(|e| e.without_url().to_string())?;
    steam_import::parse_inventory(&body)
}

/// `IInventoryService/ConsumeItem` for one item, `requestid` = the import row's id. Anything
/// after the request may have left is `Unknown`.
async fn steam_consume_item(key: &str, steamid: u64, itemid: &str, qty: u32, requestid: i64) -> steam_import::ConsumeResult {
    let client = match reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build() {
        Ok(c) => c,
        Err(e) => return steam_import::ConsumeResult::Unknown(format!("client: {e}")),
    };
    let form = [
        ("key", key.to_string()),
        ("appid", STEAM_APP_ID.to_string()),
        ("steamid", steamid.to_string()),
        ("itemid", itemid.to_string()),
        ("quantity", qty.max(1).to_string()),
        ("requestid", requestid.to_string()),
    ];
    let resp = match client.post("https://partner.steam-api.com/IInventoryService/ConsumeItem/v1/").form(&form).send().await {
        Ok(r) => r,
        Err(e) => return steam_import::ConsumeResult::Unknown(format!("send: {}", e.without_url())),
    };
    if resp.status().is_client_error() {
        return steam_import::ConsumeResult::Refused(format!("HTTP {}", resp.status()));
    }
    match resp.json::<serde_json::Value>().await {
        Ok(body) => steam_import::parse_consume(&body, itemid),
        Err(e) => steam_import::ConsumeResult::Unknown(format!("body: {}", e.without_url())),
    }
}

#[derive(serde::Deserialize)]
struct ShopBuyReq {
    def_id: String,
    qty: u32,
    idem_key: String,
}

/// `POST /shop/buy {def_id, qty, idem_key}` — debit crystal (bound first) and mail `qty` BOUND
/// units. Refused whole past the daily cap (409 `daily limit`) or underfunded (402). A repeat of
/// the same `idem_key` answers `idempotent` and charges nothing.
async fn shop_buy(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ShopBuyReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "no database"})));
    };
    let Some(row) = shop::row(&req.def_id) else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"ok": false, "reason": "not on the shelf"})));
    };
    if req.idem_key.is_empty() || req.idem_key.len() > 64 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"ok": false, "reason": "idem_key must be 1..64 chars"})));
    }
    let Some(def) = st.data.item(row.def_id) else {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": "item not in the catalog"})));
    };
    let account_id = acct.account_id as i64;
    let day = shop_day();
    let res = async {
        let mut tx = db.begin().await?;
        lock_account(&mut tx, account_id).await;
        // The wallet row exists for the spend below; `lock_account` above already serialises buys.
        sqlx::query("INSERT INTO crystal_wallet (account_id) VALUES ($1) ON CONFLICT (account_id) DO NOTHING")
            .bind(account_id).execute(&mut *tx).await?;
        let (paid, bound): (i64, i64) = sqlx::query_as(
            "SELECT paid, bound FROM crystal_wallet WHERE account_id = $1 FOR UPDATE",
        ).bind(account_id).fetch_one(&mut *tx).await?;
        let seen: Option<(String, i32)> = sqlx::query_as(
            "SELECT def_id, qty FROM shop_buys WHERE account_id = $1 AND idem_key = $2",
        ).bind(account_id).bind(&req.idem_key).fetch_optional(&mut *tx).await?;
        if let Some((def_id, qty)) = seen {
            return Ok::<Result<serde_json::Value, (StatusCode, &str)>, sqlx::Error>(Ok(serde_json::json!({
                "ok": true, "reason": "idempotent", "def_id": def_id, "qty": qty,
            })));
        }
        let (today,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(sum(qty), 0)::bigint FROM shop_buys WHERE account_id = $1 AND def_id = $2 AND day = $3",
        ).bind(account_id).bind(row.def_id).bind(day).fetch_one(&mut *tx).await?;
        let cost = match shop::price(row, req.qty, u32::try_from(today.max(0)).unwrap_or(u32::MAX)) {
            Ok(c) => c,
            Err(shop::ShopError::BadQty) => return Ok(Err((StatusCode::BAD_REQUEST, "bad qty"))),
            Err(shop::ShopError::DailyLimit) => return Ok(Err((StatusCode::CONFLICT, "daily limit"))),
        };
        if wallet_in_debt(paid, bound) {
            return Ok(Err((StatusCode::CONFLICT, "crystal balance is negative")));
        }
        let mut wallet = gacha::Wallet { paid: paid.max(0) as u64, bound: bound.max(0) as u64 };
        let Ok(spend) = wallet.spend(cost) else {
            return Ok(Err((StatusCode::PAYMENT_REQUIRED, "insufficient crystal")));
        };
        let debited = sqlx::query(
            "UPDATE crystal_wallet SET paid = paid - $2, bound = bound - $3 \
             WHERE account_id = $1 AND paid >= $2 AND bound >= $3",
        ).bind(account_id).bind(spend.from_paid as i64).bind(spend.from_bound as i64)
            .execute(&mut *tx).await?.rows_affected();
        if debited != 1 {
            return Err(sqlx::Error::RowNotFound);
        }
        sqlx::query("INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES ($1, $2, $3, 'shop', $4)")
            .bind(account_id).bind(-(spend.from_paid as i64)).bind(-(spend.from_bound as i64)).bind(&req.idem_key)
            .execute(&mut *tx).await?;
        sqlx::query("INSERT INTO shop_buys (account_id, idem_key, def_id, qty, day, crystal) VALUES ($1, $2, $3, $4, $5, $6)")
            .bind(account_id).bind(&req.idem_key).bind(row.def_id).bind(req.qty as i32).bind(day).bind(cost as i64)
            .execute(&mut *tx).await?;
        let mut it = data::instantiate_item(def);
        it.quantity = req.qty;
        it.bound = true;
        mailbox_store::send_item(&mut *tx, acct.account_id, mailbox::MailKind::Reward, &it, &format!("shop {}", row.def_id)).await?;
        tx.commit().await?;
        Ok(Ok(serde_json::json!({
            "ok": true, "def_id": row.def_id, "qty": req.qty, "crystal": cost,
            "left_today": i64::from(row.daily_limit) - today - i64::from(req.qty),
        })))
    }.await;
    match res {
        Ok(Ok(v)) => (StatusCode::OK, Json(v)),
        Ok(Err((code, why))) => (code, Json(serde_json::json!({"ok": false, "reason": why}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    }
}

/// Standard tickets per party slot opened (slots 3-5): the ten Starter pulls that fill it.
const SLOT_TICKET_COUPONS: i64 = 10;

/// What opening slot 2 pays instead (user 2026-09-30): the fixed first Starter pull, Lumi.
const SLOT_TWO_UNIT: &str = "exemplar_lumi";

/// Pay for every party slot the verified save says was opened beyond the watermark in
/// `slot_tickets` — slot 2 mails `SLOT_TWO_UNIT`, every later slot `SLOT_TICKET_COUPONS` — and move
/// the watermark. Slots open by progress in the sim (`data::open_earned_slots`; bought with Gil
/// before 0.0.490), counted in `measures.party_slots_bought`. Row locked FOR UPDATE so two saves
/// of one account cannot both pay the same slot. Nothing to pay → no writes at all.
async fn slot_tickets(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: u64,
    bought: u64,
    slot_two: Option<&sim_core::items::Item>,
) -> Result<(), sqlx::Error> {
    // The row exists BEFORE it is locked: `FOR UPDATE` on a missing row locks nothing, and two
    // accruals of one account's first slot-buying save would both read 0 and both pay.
    sqlx::query("INSERT INTO slot_tickets (account_id, paid) VALUES ($1, 0) ON CONFLICT (account_id) DO NOTHING")
        .bind(account_id as i64)
        .execute(&mut **tx)
        .await?;
    let paid: (i64,) =
        sqlx::query_as("SELECT paid FROM slot_tickets WHERE account_id = $1 FOR UPDATE")
            .bind(account_id as i64)
            .fetch_one(&mut **tx)
            .await?;
    let paid = paid.0.max(0) as u64;
    if bought <= paid {
        return Ok(());
    }
    let last = bought.min(paid + u64::from(sim_core::data::MAX_PARTY as u32));
    let mut coupons = 0i64;
    for opened in (paid + 1)..=last {
        // The Nth opening is party slot N+1.
        match (opened + 1, slot_two) {
            (2, Some(unit)) => {
                mailbox_store::send_item(&mut **tx, account_id, mailbox::MailKind::Reward, unit, "party slot 2")
                    .await.map_err(|e| sqlx::Error::Protocol(format!("slot 2 mail: {e:?}")))?;
            }
            (2, None) => {
                // A catalog without the unit must not short-change the player.
                eprintln!("[SLOT] {SLOT_TWO_UNIT} not in the catalog — slot 2 pays coupons instead");
                coupons += SLOT_TICKET_COUPONS;
            }
            _ => coupons += SLOT_TICKET_COUPONS,
        }
    }
    if coupons > 0 {
        sqlx::query(
            "INSERT INTO bound_items (account_id, kind, qty) VALUES ($1, $2, $3) \
             ON CONFLICT (account_id, kind) DO UPDATE SET qty = bound_items.qty + EXCLUDED.qty",
        )
        .bind(account_id as i64).bind(gacha::STANDARD_COUPON_KIND).bind(coupons)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "INSERT INTO bound_ledger (account_id, kind, delta, reason, ref) VALUES ($1, $2, $3, 'party_slot', $4)",
        )
        .bind(account_id as i64).bind(gacha::STANDARD_COUPON_KIND).bind(coupons)
        .bind(format!("slots {}..{}", paid + 1, last))
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query(
        "INSERT INTO slot_tickets (account_id, paid) VALUES ($1, $2) \
         ON CONFLICT (account_id) DO UPDATE SET paid = EXCLUDED.paid",
    )
    .bind(account_id as i64).bind(last as i64)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Pay what was inside every Sealed Cache opened beyond the watermark in `lootbox_opens`, and
/// move it. Same shape as `slot_tickets`: row locked FOR UPDATE, nothing to pay → no writes.
/// Each open's outcome is `lootbox::roll` of a keyed nonce on (account, open index) — the
/// verdict never changes however many times the save is seen. A coupon tier past its monthly
/// quota pays `lootbox::CONSOLATION`. Wins are not announced (user 2026-10-03: no name broadcasts).
///
/// Opens beyond `LOOTBOX_OPENS_PER_SAVE` in one save wait for the next — the sim can only open
/// what it dropped, and a save carrying thousands of opens is a client that should not exist.
const LOOTBOX_OPENS_PER_SAVE: u64 = 100;


async fn lootbox_opens(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    secret: &str,
    account_id: u64,
    opened: u64,
    now_ms: u64,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO lootbox_opens (account_id, paid) VALUES ($1, 0) ON CONFLICT (account_id) DO NOTHING")
        .bind(account_id as i64)
        .execute(&mut **tx)
        .await?;
    let paid: (i64,) =
        sqlx::query_as("SELECT paid FROM lootbox_opens WHERE account_id = $1 FOR UPDATE")
            .bind(account_id as i64)
            .fetch_one(&mut **tx)
            .await?;
    let paid = paid.0.max(0) as u64;
    if opened <= paid {
        return Ok(());
    }
    let upto = opened.min(paid + LOOTBOX_OPENS_PER_SAVE);
    let month = lootbox::month_key(now_ms);
    // Lock every tier's quota row up front in ONE order (audit 2026-10-03): taken as the nonces
    // came, two accounts opening tiers in opposite orders deadlocked on these shared rows.
    for (tier, _) in lootbox::QUOTA_TIERS {
        sqlx::query("INSERT INTO lootbox_quota (month, coupons, won) VALUES ($1, $2, 0) ON CONFLICT (month, coupons) DO NOTHING")
            .bind(&month).bind(tier as i32)
            .execute(&mut **tx)
            .await?;
    }
    sqlx::query("SELECT 1 FROM lootbox_quota WHERE month = $1 ORDER BY coupons FOR UPDATE")
        .bind(&month)
        .execute(&mut **tx)
        .await?;
    let mut crystal: i64 = 0;
    let mut coupons: i64 = 0;
    for open in (paid + 1)..=upto {
        let nonce = keyed_nonce(b"lootbox-open:v1", secret, account_id, open);
        let mut outcome = lootbox::roll(nonce);
        if let lootbox::Outcome::Coupons(n) = outcome {
            let quota = lootbox::quota_of(n).unwrap_or(0);
            // Claim one of the month's `quota` for this tier, or fall back. Locked by the row.
            sqlx::query("INSERT INTO lootbox_quota (month, coupons, won) VALUES ($1, $2, 0) ON CONFLICT (month, coupons) DO NOTHING")
                .bind(&month).bind(n as i32)
                .execute(&mut **tx)
                .await?;
            let won: (i32,) = sqlx::query_as("SELECT won FROM lootbox_quota WHERE month = $1 AND coupons = $2 FOR UPDATE")
                .bind(&month).bind(n as i32)
                .fetch_one(&mut **tx)
                .await?;
            let won = won.0.max(0) as u32;
            if won < quota {
                sqlx::query("UPDATE lootbox_quota SET won = won + 1 WHERE month = $1 AND coupons = $2")
                    .bind(&month).bind(n as i32)
                    .execute(&mut **tx)
                    .await?;
            } else {
                outcome = lootbox::CONSOLATION;
            }
        }
        match outcome {
            lootbox::Outcome::Crystal(c) => crystal += i64::from(c),
            lootbox::Outcome::Coupons(n) => coupons += i64::from(n),
        }
    }
    if crystal > 0 {
        sqlx::query(
            "INSERT INTO crystal_wallet (account_id, paid, bound) VALUES ($1, 0, $2) \
             ON CONFLICT (account_id) DO UPDATE SET bound = crystal_wallet.bound + EXCLUDED.bound",
        )
        .bind(account_id as i64).bind(crystal)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES ($1, 0, $2, 'lootbox', $3)",
        )
        .bind(account_id as i64).bind(crystal).bind(format!("opens {}..{}", paid + 1, upto))
        .execute(&mut **tx)
        .await?;
    }
    if coupons > 0 {
        sqlx::query(
            "INSERT INTO bound_items (account_id, kind, qty) VALUES ($1, $2, $3) \
             ON CONFLICT (account_id, kind) DO UPDATE SET qty = bound_items.qty + EXCLUDED.qty",
        )
        .bind(account_id as i64).bind(gacha::STANDARD_COUPON_KIND).bind(coupons)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "INSERT INTO bound_ledger (account_id, kind, delta, reason, ref) VALUES ($1, $2, $3, 'lootbox', $4)",
        )
        .bind(account_id as i64).bind(gacha::STANDARD_COUPON_KIND).bind(coupons)
        .bind(format!("opens {}..{}", paid + 1, upto))
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query(
        "INSERT INTO lootbox_opens (account_id, paid) VALUES ($1, $2) \
         ON CONFLICT (account_id) DO UPDATE SET paid = EXCLUDED.paid",
    )
    .bind(account_id as i64).bind(upto as i64)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// `GET /steam/icon/:name` — the item icons Steam's itemdefs point at (`icon_url` /
/// `icon_url_large`). Public and immutable: Steam fetches and caches them, and the repo is private,
/// so the server is the one host the item art has. Baked into the binary; an unknown name is 404.
async fn steam_icon(axum::extract::Path(name): axum::extract::Path<String>) -> axum::response::Response {
    use axum::response::IntoResponse;
    let bytes: &'static [u8] = match name.as_str() {
        "farm_box.png" => include_bytes!("../assets/steam/farm_box.png"),
        "farm_box_large.png" => include_bytes!("../assets/steam/farm_box_large.png"),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    (
        [(axum::http::header::CONTENT_TYPE, "image/png"), (axum::http::header::CACHE_CONTROL, "public, max-age=86400")],
        bytes,
    )
        .into_response()
}

/// STEAM queue (`steam_inv.rs`): one `steam_mints` row per item this verified save sent past the
/// account's highest queued seq for its def. The itemdef comes from the catalog — an item without
/// one could not have been sent, so a missing one is logged and skipped, never guessed.
async fn steam_queue(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    st: &AppState,
    account_id: u64,
    sent: &std::collections::BTreeMap<String, u64>,
) -> Result<(), sqlx::Error> {
    if sent.is_empty() {
        return Ok(());
    }
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT def_id, max(seq) FROM steam_mints WHERE account_id = $1 GROUP BY def_id",
    )
    .bind(account_id as i64)
    .fetch_all(&mut **tx).await?;
    let queued: std::collections::BTreeMap<String, u64> =
        rows.into_iter().map(|(d, s)| (d, s.max(0) as u64)).collect();
    for (def, seq) in steam_inv::plan(sent, &queued) {
        let Some(itemdef) = st.data.item(&def).and_then(|d| d.steam_itemdef) else {
            eprintln!("[steam] '{def}' was sent to Steam but has no steam_itemdef — not queued");
            continue;
        };
        sqlx::query(
            "INSERT INTO steam_mints (account_id, def_id, seq, itemdefid) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
        )
        .bind(account_id as i64).bind(&def).bind(seq as i64).bind(i64::from(itemdef))
        .execute(&mut **tx).await?;
    }
    Ok(())
}

/// The STEAM worker: mint up to 20 pending rows. Each is claimed `inflight` in a committed
/// statement BEFORE the call, so a crash mid-call leaves it `inflight` (visible, reconciled by
/// hand) rather than `pending` (which would grant it twice). A no-op unless `steam_inv::live`.
async fn mint_steam_items(st: &AppState) {
    let key = std::env::var("STEAM_PUBLISHER_KEY").ok().filter(|k| !k.is_empty());
    if !steam_inv::live(std::env::var("STEAM_INVENTORY_LIVE").ok().as_deref(), key.is_some()) {
        return;
    }
    let (Some(db), Some(key)) = (&st.db, key) else { return };
    let claimed: Vec<(i64, String, i64, i64, String)> = match sqlx::query_as(
        "UPDATE steam_mints m SET status = 'inflight', attempts = m.attempts + 1 \
         FROM (SELECT s.account_id, s.def_id, s.seq FROM steam_mints s \
               JOIN bound_items b ON b.account_id = s.account_id AND b.kind = $1 AND b.qty > 0 \
               WHERE s.status = 'pending' \
               ORDER BY s.attempts, s.account_id, s.def_id, s.seq LIMIT 20 FOR UPDATE OF s SKIP LOCKED) p, accounts a \
         WHERE m.account_id = p.account_id AND m.def_id = p.def_id AND m.seq = p.seq AND a.account_id = m.account_id \
         RETURNING m.account_id, m.def_id, m.seq, m.itemdefid, a.name",
    )
    .bind(mint_coupon::KIND)
    .fetch_all(db).await {
        Ok(r) => r,
        Err(e) => { eprintln!("[steam] claim failed: {e}"); return; }
    };
    for (acct, def, seq, itemdef, name) in claimed {
        // ONE MINT COUPON per item sent (user 2026-10-03), spent here, before Steam is asked.
        // Only accounts holding a coupon were claimed, but two rows of one account may share its
        // last coupon: the second goes back to waiting.
        let mint_ref = format!("{acct}/{def}/{seq}");
        if !spend_mint_coupon(db, acct, &mint_ref).await {
            let _ = sqlx::query("UPDATE steam_mints SET status = 'pending', attempts = GREATEST(attempts - 1, 0), note = 'awaiting a mint coupon' \
                                 WHERE account_id = $1 AND def_id = $2 AND seq = $3")
                .bind(acct).bind(&def).bind(seq).execute(db).await;
            continue;
        }
        let Some(steamid) = name.strip_prefix("steam:").and_then(|s| s.parse::<u64>().ok()) else {
            // A dev login has no Steam account to mint into — park it, say why.
            refund_mint_coupon(db, acct, &mint_ref).await;
            let _ = sqlx::query("UPDATE steam_mints SET status = 'uncertain', note = 'no steamid' WHERE account_id = $1 AND def_id = $2 AND seq = $3")
                .bind(acct).bind(&def).bind(seq).execute(db).await;
            continue;
        };
        let mut refused_parked = false;
        let (status, itemid, note) = match steam_add_item(&key, steamid, itemdef).await {
            steam_inv::MintResult::Granted(id) => ("done", Some(id), None),
            // Retried — but not forever (audit 2026-10-02): twenty rows Steam always refuses (a bad
            // itemdef, an account that cannot receive items) were re-claimed every minute and
            // nothing behind them ever minted. Fewest attempts go first, and a row refused
            // `MINT_MAX_ATTEMPTS` times is parked for a person.
            steam_inv::MintResult::Refused(why) => {
                let parked = sqlx::query_as::<_, (i32,)>("SELECT attempts FROM steam_mints WHERE account_id = $1 AND def_id = $2 AND seq = $3")
                    .bind(acct).bind(&def).bind(seq).fetch_optional(db).await.ok().flatten()
                    .is_some_and(|(a,)| a >= MINT_MAX_ATTEMPTS);
                if parked {
                    eprintln!("[steam] mint {acct}/{def}/{seq} refused {MINT_MAX_ATTEMPTS} times — parked as uncertain: {why}");
                    refused_parked = true;
                    ("uncertain", None, Some(why))
                } else {
                    ("pending", None, Some(why))
                }
            }
            steam_inv::MintResult::Unknown(why) => ("uncertain", None, Some(why)),
            // Steam unreachable is not a refusal (audit 2026-10-03): ten minutes of outage parked
            // every row it touched. The claim's attempt is given back.
            steam_inv::MintResult::NotSent(why) => {
                let _ = sqlx::query("UPDATE steam_mints SET attempts = GREATEST(attempts - 1, 0) WHERE account_id = $1 AND def_id = $2 AND seq = $3")
                    .bind(acct).bind(&def).bind(seq).execute(db).await;
                ("pending", None, Some(why))
            }
        };
        // The coupon pays for an item that reached Steam — or MIGHT have (Unknown). A refusal or a
        // request that never left gives it back.
        if !(status == "done" || (status == "uncertain" && !refused_parked)) {
            refund_mint_coupon(db, acct, &mint_ref).await;
        }
        if status == "uncertain" {
            eprintln!("[steam] mint {acct}/{def}/{seq} outcome UNKNOWN — reconcile by hand: {}", note.as_deref().unwrap_or(""));
        }
        if let Err(e) = sqlx::query("UPDATE steam_mints SET status = $4, steam_itemid = $5, note = $6 WHERE account_id = $1 AND def_id = $2 AND seq = $3")
            .bind(acct).bind(&def).bind(seq).bind(status).bind(itemid).bind(note)
            .execute(db).await
        {
            eprintln!("[steam] mint {acct}/{def}/{seq} ({status}) could not be recorded: {e} — row stays inflight");
        }
    }
}

/// Spend one held mint coupon for the mint `mint_ref`; false when the account has none.
async fn spend_mint_coupon(db: &sqlx::PgPool, acct: i64, mint_ref: &str) -> bool {
    let res = async {
        let mut tx = db.begin().await?;
        let n = sqlx::query("UPDATE bound_items SET qty = qty - 1 WHERE account_id = $1 AND kind = $2 AND qty > 0")
            .bind(acct).bind(mint_coupon::KIND).execute(&mut *tx).await?.rows_affected();
        if n == 1 {
            sqlx::query("INSERT INTO bound_ledger (account_id, kind, delta, reason, ref) VALUES ($1, $2, -1, 'steam-mint', $3)")
                .bind(acct).bind(mint_coupon::KIND).bind(mint_ref).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok::<bool, sqlx::Error>(n == 1)
    }.await;
    res.unwrap_or(false)
}

/// Give back the coupon `spend_mint_coupon` took for `mint_ref` (Steam refused, or nothing was
/// sent). Past the held cap if need be: it was the player's already.
async fn refund_mint_coupon(db: &sqlx::PgPool, acct: i64, mint_ref: &str) {
    let res = async {
        let mut tx = db.begin().await?;
        sqlx::query("INSERT INTO bound_items (account_id, kind, qty) VALUES ($1, $2, 1) \
                     ON CONFLICT (account_id, kind) DO UPDATE SET qty = bound_items.qty + 1")
            .bind(acct).bind(mint_coupon::KIND).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO bound_ledger (account_id, kind, delta, reason, ref) VALUES ($1, $2, 1, 'steam-mint-refund', $3)")
            .bind(acct).bind(mint_coupon::KIND).bind(mint_ref).execute(&mut *tx).await?;
        tx.commit().await
    }.await;
    if let Err(e) = res {
        eprintln!("[steam] mint coupon refund for {mint_ref} failed: {e} — give it back by hand");
    }
}

/// Refusals before a mint row is parked as `uncertain` for a person to look at.
const MINT_MAX_ATTEMPTS: i32 = 10;

/// `IInventoryService/AddItem` for one item. Transport failures after the request may have left
/// are `Unknown`, so the caller never retries them blind.
async fn steam_add_item(key: &str, steamid: u64, itemdef: i64) -> steam_inv::MintResult {
    let client = match reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build() {
        Ok(c) => c,
        Err(e) => return steam_inv::MintResult::NotSent(format!("client: {e}")),
    };
    let form = [
        ("key", key.to_string()),
        ("appid", STEAM_APP_ID.to_string()),
        ("steamid", steamid.to_string()),
        ("itemdefid[0]", itemdef.to_string()),
        ("notify", "1".to_string()),
    ];
    let resp = match client.post("https://partner.steam-api.com/IInventoryService/AddItem/v1/").form(&form).send().await {
        Ok(r) => r,
        Err(e) if e.is_connect() => return steam_inv::MintResult::NotSent(format!("connect: {}", e.without_url())),
        Err(e) => return steam_inv::MintResult::Unknown(format!("send: {}", e.without_url())),
    };
    if resp.status().is_client_error() {
        return steam_inv::MintResult::Refused(format!("HTTP {}", resp.status()));
    }
    match resp.json::<serde_json::Value>().await {
        Ok(body) => steam_inv::parse_add_item(&body),
        Err(e) => steam_inv::MintResult::Unknown(format!("body: {}", e.without_url())),
    }
}

/// Most Farm Banner opens one save can pay (the lootbox bound, for the same reason).
const FARM_OPENS_PER_SAVE: u64 = 100;

/// FARM BANNER accrual (`farm.rs`): for every event not yet over, credit the verified ticks
/// since this account's anchor, roll every whole hour of them, and MAIL the event's tradeable
/// box per hit (daily-capped). Before the window it only moves the anchor. Inside the save's
/// non-rejecting transaction, like the Sealed Cache: a failure defers the delta to the next
/// accepted save.
async fn farm_accrue(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    st: &AppState,
    account_id: u64,
    final_tick: u64,
    day: i64,
    now_ms: u64,
) -> Result<Vec<mailbox::MailItem>, sqlx::Error> {
    let mut drops = Vec::new();
    for ev in st.farm_events.iter().filter(|e| now_ms < e.ends_ms) {
        drops.extend(farm_accrue_one(tx, st, ev, account_id, final_tick, day, now_ms).await?);
    }
    Ok(drops)
}

async fn farm_accrue_one(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    st: &AppState,
    ev: &farm::Event,
    account_id: u64,
    final_tick: u64,
    day: i64,
    now_ms: u64,
) -> Result<Vec<mailbox::MailItem>, sqlx::Error> {
    sqlx::query("INSERT INTO farm_progress (account_id, event_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
        .bind(account_id as i64).bind(&ev.id)
        .execute(&mut **tx).await?;
    let row: (i64, i64, i64, i64, i32) = sqlx::query_as(
        "SELECT last_tick, banked, rolls, day, boxes_today FROM farm_progress          WHERE account_id = $1 AND event_id = $2 FOR UPDATE",
    )
    .bind(account_id as i64).bind(&ev.id)
    .fetch_one(&mut **tx).await?;
    let before = farm::Progress {
        last_tick: row.0.max(0) as u64,
        banked: row.1.max(0) as u64,
        rolls: row.2.max(0) as u64,
        day: row.3,
        boxes_today: row.4.max(0) as u32,
    };
    let secret = st.roll_secret.clone();
    // The first event keeps its 0.0.477 domain, so its rows' sequence continues unchanged.
    let domain = if ev.box_def == sim_core::drops::FARM_BOX_ID {
        b"farm-box:v1".to_vec()
    } else {
        format!("farm-box:v1:{}", ev.box_def).into_bytes()
    };
    let a = farm::accrue(ev, before, final_tick, day, ev.live_at(now_ms), |seq| {
        keyed_nonce(&domain, &secret, account_id, seq)
    });
    let mut drops = Vec::new();
    for _ in 0..a.boxes {
        let Some(def) = st.data.item(&ev.box_def).filter(|d| d.farm_box) else {
            eprintln!("[farm] event '{}' box '{}' is not a farm_box catalog item — nothing mailed", ev.id, ev.box_def);
            break;
        };
        let item = data::instantiate_item(def);
        let id = mailbox_store::send_item(&mut **tx, account_id, mailbox::MailKind::Reward, &item, "farm banner").await?;
        drops.push(mailbox::MailItem {
            id, account_id, kind: mailbox::MailKind::Reward,
            gil: 0, def_id: String::new(), qty: 0, item: Some(item), reason: "farm banner".into(),
        });
    }
    sqlx::query(
        "UPDATE farm_progress SET last_tick = $3, banked = $4, rolls = $5, day = $6, boxes_today = $7,          boxes_total = boxes_total + $8 WHERE account_id = $1 AND event_id = $2",
    )
    .bind(account_id as i64).bind(&ev.id)
    .bind(a.progress.last_tick as i64).bind(a.progress.banked as i64).bind(a.progress.rolls as i64)
    .bind(a.progress.day).bind(a.progress.boxes_today as i32).bind(a.boxes as i32)
    .execute(&mut **tx).await?;
    Ok(drops)
}

/// FARM BANNER tickets: every `OpenFarmBox` past this account's watermark for that box credits
/// one BOUND ticket (`farm::ticket_kind`), with a ledger row. Only catalog `farm_box` defs count.
/// `legacy` is the 0.0.477 counter (`Measures.farm_boxes_opened`), all `farm_box`: those opens
/// were paid by the old pay-on-open roll, so the first sight seeds the watermark from
/// `farm_opens.paid` and they credit nothing twice.
async fn farm_tickets(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    st: &AppState,
    account_id: u64,
    opened: &std::collections::BTreeMap<String, u64>,
    legacy: u64,
) -> Result<(), sqlx::Error> {
    let mut counts = opened.clone();
    if legacy > 0 {
        *counts.entry(sim_core::drops::FARM_BOX_ID.to_string()).or_default() += legacy;
    }
    for (def, n) in counts {
        if !st.data.item(&def).is_some_and(|d| d.farm_box) {
            continue;
        }
        sqlx::query(
            "INSERT INTO farm_box_opens (account_id, box_def, paid) VALUES ($1, $2, \
             CASE WHEN $2 = $3 THEN COALESCE((SELECT paid FROM farm_opens WHERE account_id = $1), 0) ELSE 0 END) \
             ON CONFLICT DO NOTHING",
        )
        .bind(account_id as i64).bind(&def).bind(sim_core::drops::FARM_BOX_ID)
        .execute(&mut **tx).await?;
        let row: (i64,) = sqlx::query_as("SELECT paid FROM farm_box_opens WHERE account_id = $1 AND box_def = $2 FOR UPDATE")
            .bind(account_id as i64).bind(&def)
            .fetch_one(&mut **tx).await?;
        let paid = row.0.max(0) as u64;
        if n <= paid {
            continue;
        }
        let upto = n.min(paid + FARM_OPENS_PER_SAVE);
        let owed = (upto - paid) as i64;
        let kind = farm::ticket_kind(&def);
        sqlx::query(
            "INSERT INTO bound_items (account_id, kind, qty) VALUES ($1, $2, $3) \
             ON CONFLICT (account_id, kind) DO UPDATE SET qty = bound_items.qty + EXCLUDED.qty",
        )
        .bind(account_id as i64).bind(&kind).bind(owed)
        .execute(&mut **tx).await?;
        sqlx::query("INSERT INTO bound_ledger (account_id, kind, delta, reason, ref) VALUES ($1, $2, $3, 'farm_box', $4)")
            .bind(account_id as i64).bind(&kind).bind(owed).bind(format!("opens {}..{}", paid + 1, upto))
            .execute(&mut **tx).await?;
        sqlx::query("UPDATE farm_box_opens SET paid = $3 WHERE account_id = $1 AND box_def = $2")
            .bind(account_id as i64).bind(&def).bind(upto as i64)
            .execute(&mut **tx).await?;
    }
    Ok(())
}

/// Why a Farm Banner pull was refused.
#[derive(Debug)]
enum FarmPullError {
    BadCount,
    NoEvent,
    NotEnoughTickets { have: u64 },
    InFlight,
    Misconfigured,
    Db(sqlx::Error),
}

impl From<sqlx::Error> for FarmPullError {
    fn from(e: sqlx::Error) -> Self { FarmPullError::Db(e) }
}

/// What one Farm Banner request paid.
struct FarmPulled {
    /// Per pull: `{"kind": "featured"|"gil"|"sigil", "def_id", "count"}`.
    results: serde_json::Value,
    tickets_left: u64,
    since: u32,
    replayed: bool,
}

/// `count` pulls on `box_def`'s banner in the caller's transaction: claim `idem_key` (a replay
/// returns the stored results and spends nothing), spend the tickets, roll each pull with a keyed
/// nonce on (account, box, pull seq), advance the pity, and MAIL the payouts. A featured def that
/// does not resolve aborts the whole request, as on the store banners — the tickets are spent in
/// the same transaction.
async fn farm_pull(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    st: &AppState,
    account_id: u64,
    box_def: &str,
    count: u32,
    idem_key: &str,
) -> Result<FarmPulled, FarmPullError> {
    if count != 1 && count != 10 {
        return Err(FarmPullError::BadCount);
    }
    let Some(ev) = st.farm_events.iter().find(|e| e.box_def == box_def) else { return Err(FarmPullError::NoEvent) };
    let banner_id = format!("farm:{box_def}");
    let stored: Option<(String,)> = sqlx::query_as("SELECT results_json FROM gacha_rolls WHERE idem_key = $1 AND account_id = $2")
        .bind(idem_key).bind(account_id as i64).fetch_optional(&mut **tx).await?;
    let kind = farm::ticket_kind(box_def);
    if let Some((results,)) = stored {
        let have = farm_tickets_held(tx, account_id, &kind).await?;
        let p: Option<(i32,)> = sqlx::query_as("SELECT since FROM farm_pity WHERE account_id = $1 AND box_def = $2")
            .bind(account_id as i64).bind(box_def).fetch_optional(&mut **tx).await?;
        return Ok(FarmPulled {
            results: serde_json::from_str(&results).unwrap_or(serde_json::Value::Null),
            tickets_left: have,
            since: p.map_or(0, |(s,)| s.max(0) as u32),
            replayed: true,
        });
    }
    let claimed = sqlx::query("INSERT INTO gacha_rolls (idem_key, account_id, banner_id, results_json, server_nonce) VALUES ($1, $2, $3, '[]', 0) ON CONFLICT DO NOTHING")
        .bind(idem_key).bind(account_id as i64).bind(&banner_id)
        .execute(&mut **tx).await?;
    if claimed.rows_affected() != 1 {
        return Err(FarmPullError::InFlight);
    }
    let have = farm_tickets_held(tx, account_id, &kind).await?;
    if have < u64::from(count) {
        return Err(FarmPullError::NotEnoughTickets { have });
    }
    sqlx::query(
        "INSERT INTO farm_pity (account_id, box_def, since) VALUES ($1, $2, \
         CASE WHEN $2 = $3 THEN COALESCE((SELECT since FROM farm_opens WHERE account_id = $1), 0) ELSE 0 END) \
         ON CONFLICT DO NOTHING",
    )
    .bind(account_id as i64).bind(box_def).bind(sim_core::drops::FARM_BOX_ID)
    .execute(&mut **tx).await?;
    let prow: (i64, i32) = sqlx::query_as("SELECT pulls, since FROM farm_pity WHERE account_id = $1 AND box_def = $2 FOR UPDATE")
        .bind(account_id as i64).bind(box_def).fetch_one(&mut **tx).await?;
    let pulls_before = prow.0.max(0) as u64;
    let mut since = prow.1.max(0) as u32;
    let domain = format!("farm-pull:v1:{box_def}");
    let mut gil: u64 = 0;
    let mut sigils: std::collections::BTreeMap<&'static str, u32> = std::collections::BTreeMap::new();
    let mut featured: Vec<sim_core::items::Item> = Vec::new();
    let mut out = Vec::with_capacity(count as usize);
    for i in 1..=u64::from(count) {
        let nonce = keyed_nonce(domain.as_bytes(), &st.roll_secret, account_id, pulls_before + i);
        let (outcome, next) = farm::pull(ev, since, nonce);
        since = next;
        match outcome {
            farm::Outcome::Featured => {
                let Some(item) = gacha_item(st, &ev.featured) else {
                    eprintln!("[farm] featured '{}' does not resolve — pull ABORTED, nothing spent", ev.featured);
                    return Err(FarmPullError::Misconfigured);
                };
                out.push(serde_json::json!({"kind": "featured", "def_id": ev.featured, "count": 1}));
                featured.push(item);
            }
            farm::Outcome::Gil(g) => {
                out.push(serde_json::json!({"kind": "gil", "def_id": "", "count": g}));
                gil += g;
            }
            farm::Outcome::Sigil { id, count } => {
                out.push(serde_json::json!({"kind": "sigil", "def_id": id, "count": count}));
                *sigils.entry(id).or_default() += count;
            }
        }
    }
    let spent = sqlx::query("UPDATE bound_items SET qty = qty - $3 WHERE account_id = $1 AND kind = $2 AND qty >= $3")
        .bind(account_id as i64).bind(&kind).bind(i64::from(count))
        .execute(&mut **tx).await?;
    if spent.rows_affected() != 1 {
        return Err(FarmPullError::NotEnoughTickets { have });
    }
    sqlx::query("INSERT INTO bound_ledger (account_id, kind, delta, reason, ref) VALUES ($1, $2, $3, 'spend', $4)")
        .bind(account_id as i64).bind(&kind).bind(-i64::from(count)).bind(idem_key)
        .execute(&mut **tx).await?;
    let reason = "farm banner";
    let won = featured.len() as i32;
    for item in featured {
        mailbox_store::send_item(&mut **tx, account_id, mailbox::MailKind::Reward, &item, reason).await?;
    }
    if gil > 0 {
        mailbox_store::send(&mut **tx, account_id, mailbox::MailKind::Reward, gil, "", 0, reason)
            .await
            .map_err(|e| sqlx::Error::Protocol(format!("farm gil mail: {e:?}")))?;
    }
    // One BOUND stack per Sigil id per request.
    for (id, n) in sigils {
        let Some(def) = st.data.item(id) else {
            eprintln!("[farm] Sigil '{id}' is not in the catalog — skipped");
            continue;
        };
        let mut item = data::instantiate_item(def);
        item.bound = true;
        item.quantity = n;
        mailbox_store::send_item(&mut **tx, account_id, mailbox::MailKind::Reward, &item, reason).await?;
    }
    sqlx::query("UPDATE farm_pity SET pulls = pulls + $3, since = $4, won = won + $5 WHERE account_id = $1 AND box_def = $2")
        .bind(account_id as i64).bind(box_def).bind(i64::from(count)).bind(since as i32).bind(won)
        .execute(&mut **tx).await?;
    let results = serde_json::Value::Array(out);
    sqlx::query("UPDATE gacha_rolls SET results_json = $1 WHERE idem_key = $2 AND account_id = $3")
        .bind(results.to_string()).bind(idem_key).bind(account_id as i64)
        .execute(&mut **tx).await?;
    Ok(FarmPulled { results, tickets_left: have - u64::from(count), since, replayed: false })
}

/// An account's tickets for one box, row locked.
async fn farm_tickets_held(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: u64,
    kind: &str,
) -> Result<u64, sqlx::Error> {
    let r: Option<(i64,)> = sqlx::query_as("SELECT qty FROM bound_items WHERE account_id = $1 AND kind = $2 FOR UPDATE")
        .bind(account_id as i64).bind(kind).fetch_optional(&mut **tx).await?;
    Ok(r.map_or(0, |(q,)| q.max(0) as u64))
}

#[derive(serde::Deserialize)]
struct FarmRollReq {
    box_def: String,
    count: u32,
    idem_key: String,
}

/// `POST /farm/roll` — spend Farm Banner tickets (×1 / ×10) on their event's banner. One
/// transaction; a replayed `idem_key` returns the stored results and spends nothing.
async fn farm_roll(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<FarmRollReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "farm banner needs a database"})));
    };
    if st.sessions.tainted.held().contains(&acct.account_id) {
        return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": "session out of sync — resync before pulling"})));
    }
    if req.idem_key.is_empty() || req.idem_key.len() > 64 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"ok": false, "reason": "idem_key must be 1..64 chars"})));
    }
    let mut tx = match db.begin().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    lock_account(&mut tx, acct.account_id as i64).await;
    let pulled = match farm_pull(&mut tx, &st, acct.account_id, &req.box_def, req.count, &req.idem_key).await {
        Ok(p) => p,
        Err(e) => {
            let _ = tx.rollback().await;
            let (code, body) = match e {
                FarmPullError::BadCount => (StatusCode::BAD_REQUEST, serde_json::json!({"ok": false, "reason": "pull count must be 1 or 10"})),
                FarmPullError::NoEvent => (StatusCode::NOT_FOUND, serde_json::json!({"ok": false, "reason": "unknown farm banner"})),
                FarmPullError::NotEnoughTickets { have } => (StatusCode::CONFLICT, serde_json::json!({"ok": false, "reason": "not enough tickets", "tickets": have})),
                FarmPullError::InFlight => (StatusCode::CONFLICT, serde_json::json!({"ok": false, "reason": "roll already in flight — retry"})),
                FarmPullError::Misconfigured => (StatusCode::INTERNAL_SERVER_ERROR, serde_json::json!({"ok": false, "reason": "banner misconfigured — nothing was spent"})),
                FarmPullError::Db(e) => {
                    eprintln!("[farm] pull failed for acct {}: {e}", acct.account_id);
                    (StatusCode::INTERNAL_SERVER_ERROR, serde_json::json!({"ok": false, "reason": "persist failed — retry"}))
                }
            };
            return (code, Json(body));
        }
    };
    if tx.commit().await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": "persist failed — retry"})));
    }
    if !pulled.replayed {
        println!("farm-roll acct={} box={} n={}", acct.account_id, req.box_def, req.count);
    }
    (StatusCode::OK, Json(serde_json::json!({
        "ok": true,
        "reason": if pulled.replayed { "idempotent" } else { "rolled" },
        "pulls": pulled.results,
        "tickets": pulled.tickets_left.to_string(),
        "since": pulled.since,
    })))
}

/// The Farm Banner section of `/gacha/state`: one entry per event this account holds tickets
/// for (user: the banner is hidden without a ticket, even while its event runs).
async fn farm_banners_for(db: &sqlx::PgPool, st: &AppState, account_id: u64, now_ms: u64) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for ev in &st.farm_events {
        let t: Option<(i64,)> = sqlx::query_as("SELECT qty FROM bound_items WHERE account_id = $1 AND kind = $2")
            .bind(account_id as i64).bind(farm::ticket_kind(&ev.box_def))
            .fetch_optional(db).await.ok().flatten();
        let tickets = t.map_or(0, |(q,)| q.max(0));
        if tickets == 0 {
            continue;
        }
        let p: Option<(i32,)> = sqlx::query_as("SELECT since FROM farm_pity WHERE account_id = $1 AND box_def = $2")
            .bind(account_id as i64).bind(&ev.box_def)
            .fetch_optional(db).await.ok().flatten();
        out.push(serde_json::json!({
            "event_id": ev.id,
            "box_def": ev.box_def,
            "featured": ev.featured,
            "tickets": tickets.to_string(),
            "since": p.map_or(0, |(s,)| s.max(0)),
            "pity": ev.pity,
            "featured_ppm": ev.featured_ppm,
            // Boxes still drop while this is true; the banner stays either way while tickets last.
            "farming": ev.live_at(now_ms),
            "ends_ms": ev.ends_ms.to_string(),
        }));
    }
    out
}

/// DAILY RUNS UNCONDITIONALLY, and that ordering is the point. It used to sit below three early
/// returns that all belong to the pass — no season, season out of window, and the first accrual
/// of a season (which committed and returned before ever reaching the daily block). The result
/// was that with no active season the Daily Commission never advanced at all, which quietly made
/// the "no season: crystal still pay, XP evaporates" rule unreachable — there was never anything to
/// claim. `daily` genuinely owns its own table and its own module; it should not have depended on
/// a pass existing, and now it does not.
///
/// One transaction across both. Never rejects the save: a failure here defers the delta to the
/// next accepted save (the anchors are self-healing).
async fn accrue_after_save(
    st: &AppState,
    account_id: u64,
    final_state: &sim_core::state::GameState,
) -> Result<(u32, Vec<mailbox::MailItem>), sqlx::Error> {
    let Some(db) = &st.db else { return Ok((0, Vec::new())) };
    let now = now_ms();
    let day = daily::day_of_ms(now as u64);
    let mut tx = db.begin().await?;
    lock_account(&mut tx, account_id as i64).await;

    // --- DAILY COMMISSION: its own module, its own table, no idea a pass exists ---
    let board = daily::board();
    let (day_opened, mask, ms_mask, claimed, start_values) =
        daily_row(&mut tx, account_id, day, &board, &final_state.measures).await?;
    let d = daily::accrue(
        &board,
        &daily::ladder(),
        &final_state.measures,
        &daily::DayState { done_mask: mask, milestone_mask: ms_mask, start_values: &start_values },
    );
    sqlx::query(
        "UPDATE daily_progress SET done_mask = $3, milestone_mask = $4 WHERE account_id = $1 AND day = $2",
    )
    .bind(account_id as i64).bind(day)
    .bind(d.done_mask as i16)
    .bind(d.milestone_mask as i16)
    .execute(&mut *tx)
    .await?;

    // --- WEEKLY COMMISSION: the SAME machinery on a seven-day key (weekly board ×5, flat ladder) ---
    // Advances in this same transaction so a save moves both boards at once. Like the daily, nothing
    // is paid here — `/weekly/claim` hands the crystal over. The weekly does NOT touch schedule tracks
    // or the pass, so there is no opened-flag and no XP: its whole reward is crystal.
    //
    // ONE tx with the daily on purpose: the save path is the measured scaling wall, so a second
    // commit per save is a cost to avoid, and this whole function is self-healing (put_save never
    // rejects the save on an accrual error — the delta defers to the next save). The coupling this
    // adds is the coupling the pass block below already has; `daily`'s "depends on nothing outside
    // itself" is a MODULE property (daily.rs imports no weekly), preserved here. `weekly_progress`
    // cannot be missing at serve time — `migrate()` is fail-fast (main refuses to serve a
    // half-migrated schema), so either the table exists or the process never started.
    let week = weekly::week_of_ms(now as u64);
    let wboard = weekly::board();
    let (wmask, wms_mask, _wclaimed, wstart) =
        weekly_row(&mut tx, account_id, week, &wboard, &final_state.measures).await?;
    let wk = daily::accrue(
        &wboard,
        &weekly::ladder(),
        &final_state.measures,
        &daily::DayState { done_mask: wmask, milestone_mask: wms_mask, start_values: &wstart },
    );
    sqlx::query(
        "UPDATE weekly_progress SET done_mask = $3, milestone_mask = $4 WHERE account_id = $1 AND week = $2",
    )
    .bind(account_id as i64).bind(week)
    .bind(wk.done_mask as i16)
    .bind(wk.milestone_mask as i16)
    .execute(&mut *tx)
    .await?;
    // --- COMMANDER CXP for stages newly reached (daily rungs pay theirs on `/daily/claim`) ---
    commander_stage_cxp(&mut tx, account_id, final_state.band_reached).await?;
    // NOTHING IS PAID HERE. Accrual only advances which commissions are done; crystal and pass XP
    // are handed over by `/daily/claim`, together, on one press (user 2026-08-12). Splitting the
    // payout across two moments gave the two halves different forfeiture rules, which is not
    // something a player can be expected to hold in their head.

    // --- PARTY-SLOT TICKETS: ten coupons per slot bought, once (user 2026-09-17: "ทุก slot เลย") ---
    //
    // The sim only counts (`Measures.party_slots_bought`, monotonic, success-path); the payout
    // lives here, in the same non-rejecting transaction as the commissions, against a watermark
    // so the delta is paid exactly once however many times the save is seen.
    let slot_two = gacha_item(st, SLOT_TWO_UNIT);
    slot_tickets(&mut tx, account_id, final_state.measures.party_slots_bought.get(), slot_two.as_ref()).await?;

    // --- SEALED CACHES: what was inside every box opened since the watermark (0.0.449) ---
    lootbox_opens(&mut tx, &st.roll_secret, account_id, final_state.measures.lootboxes_opened.get(), now as u64).await?;

    // --- STEAM: queue every item this save sent to Steam past the watermark (the worker mints) ---
    steam_queue(&mut tx, st, account_id, &final_state.steam_sent).await?;

    // --- FARM BANNER: boxes for the online time this save verified, then a ticket per opened box ---
    let drops = farm_accrue(&mut tx, st, account_id, final_state.tick, day, now as u64).await?;
    farm_tickets(&mut tx, st, account_id, &final_state.farm_opened, final_state.measures.farm_boxes_opened.get()).await?;

    // --- SCHEDULE TRACKS: one step per day PLAYED, on every open track ---
    //
    // TWO triggers, and the second is not redundant. `day_opened` is the normal path: the first
    // accepted save after the boundary. `any_opens_on` covers an event whose window opens while a
    // player is ALREADY in session — their commission day opened hours earlier, so hanging the
    // advance on that alone would hand them nothing until tomorrow. It reads config only, so on
    // every other day it costs no query and no allocation.
    //
    // Nothing is paid here either, for the same reason as above.
    if day_opened || schedule::any_opens_on(&st.tracks, day) {
        advance_tracks(&mut tx, account_id, day, &st.tracks).await?;
    }

    // --- BATTLE PASS: only while a season is actually running ---
    let season = live_season(&st);
    if let Some(season) = season {
        if now >= season.starts_at_ms && now < season.ends_at_ms {
            // The accrual anchor (FOR UPDATE: one accrue at a time per account).
            let row: Option<(i64, i64, String)> = sqlx::query_as(
                "SELECT xp, last_tick, snapshot_json FROM bp_state WHERE account_id = $1 AND season_id = $2 FOR UPDATE",
            )
            .bind(account_id as i64).bind(season.season_id)
            .fetch_optional(&mut *tx)
            .await?;
            if row.is_none() {
                // First accrue of the season: anchor at THIS save (no retroactive pre-season XP);
                // counters start diffing from the default (a brand-new account) — for an existing
                // account the first save's totals are absorbed into the anchor, not paid out.
                sqlx::query(
                    "INSERT INTO bp_state (account_id, season_id, xp, last_tick, snapshot_json) VALUES ($1, $2, 0, $3, $4)",
                )
                .bind(account_id as i64).bind(season.season_id)
                .bind(final_state.tick as i64)
                .bind(serde_json::to_string(&final_state.measures).unwrap_or_default())
                .execute(&mut *tx)
                .await?;
            } else {
                // The pass earns NOTHING here. Every point it has comes from a claimed mission, so
                // accrual only advances the anchor — `/daily/claim` is what moves XP.
                sqlx::query(
                    "UPDATE bp_state SET last_tick = $3, snapshot_json = $4 WHERE account_id = $1 AND season_id = $2",
                )
                .bind(account_id as i64).bind(season.season_id)
                .bind(final_state.tick as i64)
                .bind(serde_json::to_string(&final_state.measures).unwrap_or_default())
                .execute(&mut *tx)
                .await?;
            }
        }
    }
    tx.commit().await?;
    // What Claim would hand over if pressed now. Derived, never accumulated — same input the
    // endpoint uses, so the badge cannot promise a number the claim then disagrees with.
    Ok((daily::claimable(&daily::ladder(), d.points, claimed).1 as u32, drops))
}


#[derive(serde::Serialize)]
struct BpStateResp {
    ok: bool,
    reason: String,
    season_id: i64,
    ends_at_ms: i64,
    now_ms: i64,
    xp: u64,
    level: u32,
    slots: u32,
    slot_cost: u64,
    /// FREE-track rewards, 1 per slot in order (enriched for rendering).
    free_track: Vec<BpRewardView>,
    /// FREE-track slots already claimed (for the ✓ overlay).
    claimed: Vec<u32>,
    /// PREMIUM-track rewards, same shape as `free_track`.
    premium_track: Vec<BpRewardView>,
    /// Whether this account holds the premium track for the season.
    premium: bool,
    /// PREMIUM-track slots already claimed.
    claimed_premium: Vec<u32>,
}

/// A reward cell enriched from the catalog so the client renders name/rarity without a
/// local item lookup.
#[derive(serde::Serialize)]
struct BpRewardView {
    slot: u32,
    def_id: String,
    qty: u32,
    name: String,
    rarity: String,
    kind: String,
    /// What the slot pays: `item`, `standard_ticket`, `limited_ticket`, `crystal` or `gil`. For
    /// the currencies `def_id` is `gacha_coupon_std` / `gacha_coupon_limited` / `crystal` / `gil`
    /// and `kind` is `Currency`.
    reward: bp::RewardKind,
}

/// Build the enriched free-track view from a season config + the item catalog.
fn bp_free_track_view(st: &AppState, cfg: &bp::SeasonConfig) -> Vec<BpRewardView> {
    bp_track_view(st, &cfg.free_track)
}

fn bp_track_view(st: &AppState, track: &[bp::SlotReward]) -> Vec<BpRewardView> {
    track
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let currency = match r.kind {
                bp::RewardKind::Item => None,
                bp::RewardKind::StandardTicket => Some((gacha::STANDARD_COUPON_KIND, "Standard Ticket", "Rare")),
                bp::RewardKind::LimitedTicket => Some((gacha::LIMITED_COUPON_KIND, "Limited Ticket", "Legendary")),
                bp::RewardKind::Crystal => Some(("crystal", "Crystal", "Rare")),
                bp::RewardKind::Gil => Some(("gil", "Gil", "Common")),
            };
            if let Some((id, name, rarity)) = currency {
                return BpRewardView {
                    slot: i as u32 + 1, def_id: id.into(), qty: r.qty, name: name.into(),
                    rarity: rarity.into(), kind: "Currency".into(), reward: r.kind,
                };
            }
            let (name, rarity, kind) = match st.data.item(&r.def_id) {
                Some(def) => (
                    def.name.clone(),
                    format!("{:?}", def.rarity),
                    format!("{:?}", def.kind),
                ),
                None => (r.def_id.clone(), "Common".into(), "Material".into()),
            };
            BpRewardView { slot: i as u32 + 1, def_id: r.def_id.clone(), qty: r.qty, name, rarity, kind, reward: r.kind }
        })
        .collect()
}


// ---------------------------------------------------------------- schedule + claim

/// GET /claim/state — every OPEN track, with each cell's state.
///
/// A LIST, not one track: several login events run at once by design, so a screen that could only
/// show one would be wrong on the day a second opens.
///
/// Read-only. A GET must not advance a track — that happens on a verified save and nowhere else,
/// or the ladder is farmable by refreshing a screen.
///
/// Three cell states, and they are not interchangeable. LOCKED (`position < at`) was never this
/// account's to take — someone who joins on the last day of an event reaches cell 1 and no more.
/// CLAIMABLE is already theirs and simply waiting. CLAIMED is done. Collapsing the first two into
/// "not claimed" is how a player is told they lost something they never had.
async fn claim_state_ep(State(st): State<AppState>, headers: HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "no database"})));
    };
    let now = now_ms();
    let day = daily::day_of_ms(now as u64);
    // NOT `unwrap_or_default()`. An empty result and a failed query are the same value and
    // opposite meanings: defaulting would draw every ladder back at position 0 with nothing
    // claimed, which reads as "my progress was wiped" rather than "the server is having trouble".
    let rows: Vec<(String, i32, i64, i64)> = match sqlx::query_as(
        "SELECT track, position, claimed_mask, opened_day FROM claim_progress WHERE account_id = $1",
    )
    .bind(acct.account_id as i64)
    .fetch_all(db).await
    {
        Ok(r) => r,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
        }
    };
    let opened: std::collections::BTreeMap<String, i64> =
        rows.iter().map(|(t, _, _, o)| (t.clone(), *o)).collect();
    let mut out = Vec::new();
    for t in st.tracks.iter().filter(|t| t.is_active(day, opened.get(&t.id).copied())) {
        // No row = the track has not opened for this account yet (no accepted save since it began).
        // Position 0, which reaches nothing — the honest answer, not a write from a screen refresh.
        let (position, claimed_mask) = rows
            .iter()
            .find(|(id, _, _, _)| id == &t.id)
            .map(|(_, p, m, _)| ((*p).max(0) as u32, *m as u64))
            .unwrap_or((0, 0));
        let (_, pending) = schedule::owed(&t.entries, position, claimed_mask);
        let cells: Vec<serde_json::Value> = t.entries.iter().map(|e| {
            let claimed = e.id <= schedule::MAX_ENTRY_ID && claimed_mask & (1u64 << e.id) != 0;
            serde_json::json!({
                "id": e.id,
                "at": e.at,
                "state": if claimed { "claimed" } else if position >= e.at { "claimable" } else { "locked" },
                "reward": e.reward.iter().map(|g| grant_view(&st, g)).collect::<Vec<_>>(),
            })
        }).collect();
        out.push(serde_json::json!({
            "track": t.id,
            "position": position,
            // The client counts down from SERVER truth rather than its own clock — the same rule
            // `/daily/state` follows with `resets_at_ms`. null = this track never expires.
            // The account's OWN deadline. An absolute window closes on the same date for
            // everyone; a SinceStart one closes `days` after this account's first day, and is
            // null until they have started it — nothing is counting down yet.
            "closes_at_ms": match t.window {
                Some(schedule::Window::Absolute { closes_day, .. }) => {
                    Some(daily::day_end_ms(closes_day))
                }
                Some(schedule::Window::SinceStart { days }) => opened
                    .get(&t.id)
                    .map(|start| daily::day_end_ms(start + days - 1)),
                None => None,
            },
            "claimable": pending.len(),
            "cells": cells,
        }));
    }
    (StatusCode::OK, Json(serde_json::json!({"ok": true, "now_ms": now, "day": day, "tracks": out})))
}

/// A reward cell enriched from the catalog so the client renders name/rarity without a local
/// lookup — the same shape `bp_free_track_view` builds for the pass.
fn grant_view(st: &AppState, g: &schedule::Grant) -> serde_json::Value {
    match g {
        schedule::Grant::Crystal(n) => serde_json::json!({"kind": "crystal", "qty": n}),
        schedule::Grant::Gil(n) => serde_json::json!({"kind": "gil", "qty": n}),
        schedule::Grant::Item { def_id, qty } => {
            let (name, rarity) = match st.data.item(def_id) {
                Some(d) => (d.name.clone(), format!("{:?}", d.rarity)),
                None => (def_id.clone(), "Common".into()),
            };
            serde_json::json!({"kind": "item", "def_id": def_id, "qty": qty, "name": name, "rarity": rarity})
        }
        schedule::Grant::Bound { kind, qty } => {
            serde_json::json!({"kind": "bound", "bound_kind": kind, "qty": qty})
        }
    }
}

#[derive(serde::Deserialize)]
struct ClaimCollectReq {
    /// Omit to collect EVERY open track in one press. With several events running at once, asking
    /// the player to press N times to receive what one login earned is a worse screen, not a safer
    /// one — and it is the same transaction either way.
    #[serde(default)]
    track: Option<String>,
}

/// POST /claim/collect — hand over every reached-but-unpaid cell, across one track or all of them.
///
/// ONE PRESS PAYS EVERYTHING OWED. A player who misses a day loses nothing: the next login reaches
/// the next cell, and both are collected together. That differs from `/daily/claim`, which forfeits
/// at the day boundary — deliberately, because a commission's unit is the DAY while a cell's unit
/// is "your Nth day played", which no calendar can expire. The only thing that forfeits here is an
/// event WINDOW closing.
///
/// EXACTLY-ONCE without an idempotency key: `SELECT ... FOR UPDATE` plus `claimed_mask`, in a
/// single transaction with the payout. A second press finds nothing owed and pays nothing; that is
/// not an error.
///
/// GEMS go to the BOUND bucket — only crystal bought with real money are paid crystal. ITEMS and GIL go
/// through the MAILBOX, the one road by which server-authored value enters a replayed inventory:
/// it already carries the full-bag guard (`claim_mail` no-ops and leaves the mail claimable) and
/// consume-on-verified-save. That is also what makes a ten-day backlog safe — the burst lands in
/// the mailbox, and the bag is only involved one mail at a time.
async fn claim_collect(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ClaimCollectReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "no database"})));
    };
    let day = daily::day_of_ms(now_ms() as u64);
    let opened: std::collections::BTreeMap<String, i64> = match sqlx::query_as(
        "SELECT track, opened_day FROM claim_progress WHERE account_id = $1",
    )
    .bind(acct.account_id as i64)
    .fetch_all(db).await
    {
        Ok(r) => r.into_iter().collect(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    let wanted: Vec<&schedule::TrackDef> = st
        .tracks
        .iter()
        .filter(|t| {
            t.is_active(day, opened.get(&t.id).copied())
                && req.track.as_deref().is_none_or(|id| id == t.id)
        })
        .collect();
    if wanted.is_empty() {
        // A named track that is closed or unknown. Not an error the client can act on differently
        // from "nothing owed", but it must not read as a successful collection of nothing.
        return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": "no such open track"})));
    }
    let mut tx = match db.begin().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    lock_account(&mut tx, acct.account_id as i64).await;
    let mut crystal_total = 0u64;
    let mut collected = Vec::new();
    // What the crystal ledger records. The DAY alone is not enough: two tracks collected on one day
    // write ONE ledger row, so a double credit would show up as a larger number rather than as a
    // duplicate — which is the failure `/daily/claim`'s own `ref` comment exists to prevent.
    let mut paid_ref: Vec<String> = Vec::new();
    for t in wanted {
        let row: Option<(i32, i64)> = match sqlx::query_as(
            "SELECT position, claimed_mask FROM claim_progress \
             WHERE account_id = $1 AND track = $2 FOR UPDATE",
        )
        .bind(acct.account_id as i64).bind(&t.id)
        .fetch_optional(&mut *tx).await
        {
            Ok(r) => r,
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
        };
        // No row = the track has not opened for this account. Nothing is owed, and a GET must not
        // have created one either.
        let Some((position, claimed_mask)) = row else { continue };
        let (pay, cells) = schedule::owed(&t.entries, position.max(0) as u32, claimed_mask as u64);
        if pay == 0 {
            continue;
        }
        let mut crystal = 0u64;
        for cell in &cells {
            for g in &cell.reward {
                match g {
                    schedule::Grant::Crystal(n) => crystal += u64::from(*n),
                    // `reason` carries the track and cell, so a double credit shows in the audit
                    // trail as a duplicate rather than as a larger number.
                    schedule::Grant::Gil(n) => {
                        if let Err(e) = mailbox_store::send(
                            &mut *tx, acct.account_id, mailbox::MailKind::Reward, *n, "", 0,
                            &format!("{}:{}", t.id, cell.id),
                        ).await {
                            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": format!("{e:?}")})));
                        }
                    }
                    // COUNTER, not mail. Credited in this transaction beside the mask write, so a
                    // claim that records cannot fail to pay and vice versa.
                    schedule::Grant::Bound { kind, qty } => {
                        if let Err(e) = sqlx::query(
                            "INSERT INTO bound_items (account_id, kind, qty) VALUES ($1, $2, $3) \
                             ON CONFLICT (account_id, kind) DO UPDATE SET qty = bound_items.qty + EXCLUDED.qty",
                        )
                        .bind(acct.account_id as i64).bind(kind).bind(i64::from(*qty))
                        .execute(&mut *tx).await
                        {
                            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
                        }
                        // Same transaction as the mask write, so a credit cannot exist unrecorded
                        // and a record cannot exist uncredited.
                        if let Err(e) = sqlx::query(
                            "INSERT INTO bound_ledger (account_id, kind, delta, reason, ref) \
                             VALUES ($1, $2, $3, 'claim', $4)",
                        )
                        .bind(acct.account_id as i64).bind(kind).bind(i64::from(*qty))
                        .bind(format!("{}:{}", t.id, cell.id))
                        .execute(&mut *tx).await
                        {
                            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
                        }
                    }
                    schedule::Grant::Item { def_id, qty } => {
                        if let Err(e) = mailbox_store::send(
                            &mut *tx, acct.account_id, mailbox::MailKind::Reward, 0, def_id, u64::from(*qty),
                            &format!("{}:{}", t.id, cell.id),
                        ).await {
                            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": format!("{e:?}")})));
                        }
                    }
                }
            }
        }
        // The mask is written in the SAME transaction as the payout. Split them and a mail that
        // lands while the mask write fails is a reward handed out again on the next press.
        if let Err(e) = sqlx::query(
            "UPDATE claim_progress SET claimed_mask = claimed_mask | $3 \
             WHERE account_id = $1 AND track = $2",
        )
        .bind(acct.account_id as i64).bind(&t.id).bind(pay as i64)
        .execute(&mut *tx).await
        {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
        }
        crystal_total += crystal;
        let ids: Vec<String> = cells.iter().map(|c| c.id.to_string()).collect();
        paid_ref.push(format!("{}:{}", t.id, ids.join(",")));
        collected.push(serde_json::json!({
            "track": t.id,
            "cells": cells.iter().map(|c| c.id).collect::<Vec<_>>(),
        }));
    }
    if crystal_total > 0 {
        if let Err(e) = sqlx::query(
            "INSERT INTO crystal_wallet (account_id, paid, bound) VALUES ($1, 0, $2) \
             ON CONFLICT (account_id) DO UPDATE SET bound = crystal_wallet.bound + EXCLUDED.bound",
        )
        .bind(acct.account_id as i64).bind(crystal_total as i64)
        .execute(&mut *tx).await
        {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
        }
        if let Err(e) = sqlx::query(
            "INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) \
             VALUES ($1, 0, $2, 'schedule', $3)",
        )
        .bind(acct.account_id as i64).bind(crystal_total as i64)
        .bind(format!("day:{day}|{}", paid_ref.join("|")))
        .execute(&mut *tx).await
        {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
        }
    }
    if let Err(e) = tx.commit().await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
    }
    (StatusCode::OK, Json(serde_json::json!({
        "ok": true, "crystal": crystal_total, "collected": collected,
    })))
}

/// GET /weekly/state — everything the Weekly Commission screen renders. Mirror of `/daily/state`
/// on a seven-day key: the weekly board (daily targets ×5), the flat 100-a-rung ladder, and the
/// week's countdown. Read-only — the row opens on the next verified save, never on a screen refresh.
async fn weekly_state_ep(State(st): State<AppState>, headers: HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "no database"})));
    };
    let now = now_ms();
    let week = weekly::week_of_ms(now as u64);
    let board = weekly::board();
    let ladder = weekly::ladder();
    let row: Option<(i16, i16, i16, String)> = sqlx::query_as(
        "SELECT done_mask, milestone_mask, claimed_mask, start_values_json FROM weekly_progress \
         WHERE account_id = $1 AND week = $2",
    )
    .bind(acct.account_id as i64).bind(week)
    .fetch_optional(db).await.ok().flatten();
    let (done_mask, claimed_mask, start_values): (u16, u16, Vec<u64>) = match &row {
        Some((d, _ms, c, sv)) => (*d as u16, *c as u16, serde_json::from_str(sv).unwrap_or_default()),
        None => (0, 0, Vec::new()),
    };
    let measures: sim_core::state::Measures = st
        .saves
        .held()
        .get(&acct.account_id)
        .and_then(|sv| serde_json::from_str::<sim_core::state::GameState>(&sv.state_json).ok())
        .map(|g| g.measures)
        .unwrap_or_default();
    let points = daily::points_of(&board, &ladder, done_mask);
    let items: Vec<serde_json::Value> = board
        .iter()
        .map(|d| {
            let done = (d.bit as usize) < daily::MAX_DAILIES && done_mask & (1u16 << d.bit) != 0;
            let progress = start_values
                .get(d.bit as usize)
                .filter(|s| **s != daily::NO_START)
                .map(|s| d.counter.read(&measures).saturating_sub(*s).min(d.target))
                .unwrap_or(0);
            serde_json::json!({
                "id": d.id,
                "target": d.target,
                "kind": format!("{:?}", d.kind),
                "progress": if done { d.target } else { progress },
                "done": done,
            })
        })
        .collect();
    let reached = daily::reached_mask(&ladder, points);
    let (_pay, claimable_crystal) = daily::claimable(&ladder, points, claimed_mask);
    let rungs: Vec<serde_json::Value> = ladder
        .iter()
        .enumerate()
        .map(|(i, m)| serde_json::json!({
            "points": m.points,
            "crystal": m.crystal,
            "reached": reached & (1u16 << i) != 0,
            "claimed": claimed_mask & (1u16 << i) != 0,
        }))
        .collect();
    (StatusCode::OK, Json(serde_json::json!({
        "ok": true,
        "week": week,
        "now_ms": now,
        "resets_at_ms": weekly::week_end_ms(week),
        "points": points,
        "board": items,
        "ladder": rungs,
        "claimable_crystal": claimable_crystal,
    })))
}

#[derive(serde::Deserialize)]
struct WeeklyClaimReq {
    /// The week the CLIENT believes it is — rejected unless it matches the server's, the same
    /// stale-screen guard `/daily/claim` uses.
    week: i64,
}

/// POST /weekly/claim — hand over every reached-but-unpaid rung, in one press. Crystal go to the BOUND
/// bucket (earned, not bought), and each rung pays the season's `xp_per_weekly_rung` of pass XP
/// (user 2026-10-01). Exactly-once via `SELECT ... FOR UPDATE` + `claimed_mask` in one
/// transaction; a second press finds nothing owed and pays nothing.
async fn weekly_claim(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<WeeklyClaimReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "no database"})));
    };
    let week = weekly::week_of_ms(now_ms() as u64);
    if req.week != week {
        return (StatusCode::CONFLICT, Json(serde_json::json!({
            "ok": false, "reason": "not this week", "week": week,
        })));
    }
    let board = weekly::board();
    let ladder = weekly::ladder();
    let mut tx = match db.begin().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    lock_account(&mut tx, acct.account_id as i64).await;
    let row: Option<(i16, i16)> = match sqlx::query_as(
        "SELECT done_mask, claimed_mask FROM weekly_progress \
         WHERE account_id = $1 AND week = $2 FOR UPDATE",
    )
    .bind(acct.account_id as i64).bind(week)
    .fetch_optional(&mut *tx).await
    {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    let (done_mask, claimed_mask) = match row {
        Some((d, c)) => (d as u16, c as u16),
        None => (0u16, 0u16),
    };
    let points = daily::points_of(&board, &ladder, done_mask);
    let (pay, crystal) = daily::claimable(&ladder, points, claimed_mask);
    if pay == 0 {
        let _ = tx.rollback().await;
        return (StatusCode::OK, Json(serde_json::json!({
            "ok": true, "claimed_rungs": 0, "crystal": 0, "points": points,
        })));
    }
    let rungs = pay.count_ones() as u64;
    if let Err(e) = sqlx::query(
        "INSERT INTO crystal_wallet (account_id, paid, bound) VALUES ($1, 0, $2) \
         ON CONFLICT (account_id) DO UPDATE SET bound = crystal_wallet.bound + EXCLUDED.bound",
    )
    .bind(acct.account_id as i64).bind(crystal as i64)
    .execute(&mut *tx).await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
    }
    if let Err(e) = sqlx::query(
        "INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) \
         VALUES ($1, 0, $2, 'weekly', $3)",
    )
    .bind(acct.account_id as i64).bind(crystal as i64).bind(format!("week:{week}|rungs:{pay:b}"))
    .execute(&mut *tx).await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
    }
    // Pass XP per weekly rung (user 2026-10-01), priced by the season like the daily's.
    let season = live_season(&st);
    let mut xp_paid = 0u64;
    if let Some(season) = season {
        let xp = rungs * season.config.xp_per_weekly_rung;
        if let Err(e) = sqlx::query(
            "INSERT INTO bp_state (account_id, season_id, xp) VALUES ($1, $2, $3) \
             ON CONFLICT (account_id, season_id) DO UPDATE SET xp = bp_state.xp + EXCLUDED.xp",
        )
        .bind(acct.account_id as i64).bind(season.season_id).bind(xp as i64)
        .execute(&mut *tx).await
        {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
        }
        xp_paid = xp;
    }
    if let Err(e) = sqlx::query(
        "INSERT INTO weekly_progress (account_id, week, done_mask, milestone_mask, claimed_mask) \
         VALUES ($1, $2, 0, 0, $3) \
         ON CONFLICT (account_id, week) DO UPDATE SET claimed_mask = weekly_progress.claimed_mask | EXCLUDED.claimed_mask",
    )
    .bind(acct.account_id as i64).bind(week).bind(pay as i16)
    .execute(&mut *tx).await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
    }
    if let Err(e) = tx.commit().await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
    }
    (StatusCode::OK, Json(serde_json::json!({
        "ok": true, "claimed_rungs": rungs, "crystal": crystal, "points": points, "xp": xp_paid,
    })))
}

/// GET /daily/state — everything the Daily Commission screen renders.
///
/// The CURRENT day only. Yesterday is not a thing the client can see or claim (user
/// 2026-08-12): unclaimed rungs are forfeited at the boundary, and a screen that shows a day you
/// can no longer act on is a screen that only ever delivers bad news.
async fn daily_state_ep(State(st): State<AppState>, headers: HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "no database"})));
    };
    let now = now_ms();
    let day = daily::day_of_ms(now as u64);
    let board = daily::board();
    let ladder = daily::ladder();
    // Read-only: a GET must not open the day. The row appears on the next verified save, and
    // until then the honest answer is "nothing done yet", not a write from a screen refresh.
    let row: Option<(i16, i16, i16, String)> = sqlx::query_as(
        "SELECT done_mask, milestone_mask, claimed_mask, start_values_json FROM daily_progress \
         WHERE account_id = $1 AND day = $2",
    )
    .bind(acct.account_id as i64).bind(day)
    .fetch_optional(db).await.ok().flatten();
    let (done_mask, claimed_mask, start_values): (u16, u16, Vec<u64>) = match &row {
        Some((d, _ms, c, sv)) => (*d as u16, *c as u16, serde_json::from_str(sv).unwrap_or_default()),
        None => (0, 0, Vec::new()),
    };
    // Progress needs the account's live counters, which live on its last accepted save.
    let measures: sim_core::state::Measures = st
        .saves
        .held()
        .get(&acct.account_id)
        .and_then(|sv| serde_json::from_str::<sim_core::state::GameState>(&sv.state_json).ok())
        .map(|g| g.measures)
        .unwrap_or_default();
    let points = daily::points_of(&board, &ladder, done_mask);
    let items: Vec<serde_json::Value> = board
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let _ = i;
            let done = (d.bit as usize) < daily::MAX_DAILIES && done_mask & (1u16 << d.bit) != 0;
            // No opening reading = a commission added after this day opened, or a hole in the row.
            // It cannot be finished today (see `accrue`), so it reports zero rather than the
            // player's lifetime total. BY BIT, matching how the row was written.
            let progress = start_values
                .get(d.bit as usize)
                .filter(|s| **s != daily::NO_START)
                .map(|s| d.counter.read(&measures).saturating_sub(*s).min(d.target as u64))
                .unwrap_or(0);
            serde_json::json!({
                "id": d.id,
                "target": d.target,
                "kind": format!("{:?}", d.kind),
                "progress": if done { d.target as u64 } else { progress },
                "done": done,
            })
        })
        .collect();
    let reached = daily::reached_mask(&ladder, points);
    let (_pay, claimable_crystal) = daily::claimable(&ladder, points, claimed_mask);
    let rungs: Vec<serde_json::Value> = ladder
        .iter()
        .enumerate()
        .map(|(i, m)| serde_json::json!({
            "points": m.points,
            "crystal": m.crystal,
            "reached": reached & (1u16 << i) != 0,
            "claimed": claimed_mask & (1u16 << i) != 0,
        }))
        .collect();
    (StatusCode::OK, Json(serde_json::json!({
        "ok": true,
        "day": day,
        "now_ms": now,
        // The boundary, so the client counts down from server truth instead of its own clock.
        "resets_at_ms": daily::day_end_ms(day),
        "points": points,
        "board": items,
        "ladder": rungs,
        "claimable_crystal": claimable_crystal,
    })))
}

#[derive(serde::Deserialize)]
struct DailyClaimReq {
    /// The day the CLIENT believes it is. Rejected unless it matches the server's — which is how
    /// "current day only" is enforced against a client whose clock stopped overnight, rather than
    /// silently paying out yesterday's ladder from a stale screen.
    day: i64,
}

/// POST /daily/claim — hand over every reached-but-unpaid rung, in one press.
///
/// Crystal go to the BOUND bucket: only crystal bought with real money are paid crystal. Pass XP rides
/// along at `XP_PER_RUNG` per rung — one press, one payout, one forfeiture rule.
///
/// EXACTLY-ONCE without an idempotency key: `SELECT ... FOR UPDATE` plus `claimed_mask` inside a
/// single transaction. A second press finds nothing owed and pays nothing; it is not an error.
async fn daily_claim(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<DailyClaimReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "no database"})));
    };
    let day = daily::day_of_ms(now_ms() as u64);
    if req.day != day {
        return (StatusCode::CONFLICT, Json(serde_json::json!({
            "ok": false, "reason": "not today", "day": day,
        })));
    }
    let board = daily::board();
    let ladder = daily::ladder();
    let mut tx = match db.begin().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    lock_account(&mut tx, acct.account_id as i64).await;
    let row: Option<(i16, i16)> = match sqlx::query_as(
        "SELECT done_mask, claimed_mask FROM daily_progress \
         WHERE account_id = $1 AND day = $2 FOR UPDATE",
    )
    .bind(acct.account_id as i64).bind(day)
    .fetch_optional(&mut *tx).await
    {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    // No row = the day has not opened (no verified save since the boundary). Nothing is owed.
    let (done_mask, claimed_mask) = match row {
        Some((d, c)) => (d as u16, c as u16),
        None => (0u16, 0u16),
    };
    let points = daily::points_of(&board, &ladder, done_mask);
    let (pay, crystal) = daily::claimable(&ladder, points, claimed_mask);
    if pay == 0 {
        let _ = tx.rollback().await;
        return (StatusCode::OK, Json(serde_json::json!({
            "ok": true, "claimed_rungs": 0, "crystal": 0, "xp": 0, "points": points,
        })));
    }
    let rungs = pay.count_ones() as u64;
    // THE SEASON PRICES ITS OWN RUNG. `bp::SeasonConfig::xp_per_daily_rung` is documented as the
    // seam where a pass decides what a daily is worth to it, and nothing read it — a hardcoded
    // constant sat on top. `/daily/claim` is the pass's ONLY XP source, so that constant was the
    // entire earn rate and a season shipping its own number was silently ignored. Falls back to
    // the constant when no season runs, which is when the XP evaporates anyway.
    let xp_per_rung = st
        .season
        .held()
        .as_ref()
        .map(|s| s.config.xp_per_daily_rung)
        .unwrap_or(daily::XP_PER_RUNG);
    let xp = rungs * xp_per_rung;
    // BOUND, never the paid bucket — this crystal was earned, not bought.
    if let Err(e) = sqlx::query(
        "INSERT INTO crystal_wallet (account_id, paid, bound) VALUES ($1, 0, $2) \
         ON CONFLICT (account_id) DO UPDATE SET bound = crystal_wallet.bound + EXCLUDED.bound",
    )
    .bind(acct.account_id as i64).bind(crystal as i64)
    .execute(&mut *tx).await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
    }
    // `ref` carries the day AND THE RUNGS. The day alone cannot tell a legitimate second partial
    // claim from a duplicate credit — both are two rows for one day — so the rungs paid are what
    // makes a repeat visible as a repeat.
    if let Err(e) = sqlx::query(
        "INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) \
         VALUES ($1, 0, $2, 'daily', $3)",
    )
    .bind(acct.account_id as i64).bind(crystal as i64).bind(format!("day:{day}|rungs:{pay:b}"))
    .execute(&mut *tx).await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
    }
    // No season running = the XP has nowhere to go, and it evaporates (user 2026-08-12). The
    // crystal still pay: they are the headline reward and are not tied to a pass.
    // Commander CXP rides the same claim as the crystal (user 2026-09-30): per rung, not per task.
    let cxp = rungs as i64 * commander::CXP_PER_DAILY_RUNG;
    if let Err(e) = add_commander_cxp(&mut tx, acct.account_id, cxp).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
    }
    let season = live_season(&st);
    let mut xp_paid = 0u64;
    if let Some(season) = season {
        if sqlx::query(
            "INSERT INTO bp_state (account_id, season_id, xp) VALUES ($1, $2, $3) \
             ON CONFLICT (account_id, season_id) DO UPDATE SET xp = bp_state.xp + EXCLUDED.xp",
        )
        .bind(acct.account_id as i64).bind(season.season_id).bind(xp as i64)
        .execute(&mut *tx).await.is_ok()
        {
            xp_paid = xp;
        }
    }
    // The row may not exist yet (claimed before any save opened the day) — but `pay` is only
    // non-zero when `done_mask` was, which cannot happen without a row. Upsert regardless so the
    // mask is never lost to a missing row.
    if let Err(e) = sqlx::query(
        "INSERT INTO daily_progress (account_id, day, done_mask, milestone_mask, claimed_mask) \
         VALUES ($1, $2, 0, 0, $3) \
         ON CONFLICT (account_id, day) DO UPDATE SET claimed_mask = daily_progress.claimed_mask | EXCLUDED.claimed_mask",
    )
    .bind(acct.account_id as i64).bind(day).bind(pay as i16)
    .execute(&mut *tx).await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
    }
    if let Err(e) = tx.commit().await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
    }
    (StatusCode::OK, Json(serde_json::json!({
        "ok": true, "claimed_rungs": rungs, "crystal": crystal, "xp": xp_paid, "points": points,
        "cxp": cxp,
    })))
}

/// GET /commander/state — everything the Commander screen renders (server truth only): level, CXP
/// progress, the Outpost accrual PREVIEW (not claimed), and which level rewards are pending.
async fn commander_state_ep(State(st): State<AppState>, headers: HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "no database"})));
    };
    let row: Option<(i64, i32)> = match sqlx::query_as(
        "SELECT cxp, reward_claimed_through FROM commander WHERE account_id = $1",
    )
    .bind(acct.account_id as i64).fetch_optional(db).await
    {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    let (cxp, claimed_through) = row.unwrap_or((0, 0));
    let (level, into_level, to_next) = commander::progress(&st.commander_levels, cxp);
    // Pending = reward levels reached but not yet accepted. Level 1 IS a reward (free welcome), so a
    // fresh account at level 1 with claimed_through 0 already has [1] pending.
    let ct = claimed_through as u32;
    let pending: Vec<u32> = if level > ct { ((ct + 1)..=level).collect() } else { Vec::new() };
    // Re-offer the skill grants for every reached unlock level (idempotent), so a client whose earlier
    // unlock echo was lost can reconcile it on load without a fresh crystal-paying claim.
    let skill_grants = commander_skill_grants(&st.roll_secret, acct.account_id, level);
    // The reward TRACK itself is STATIC config the client already holds (byte-identical copy) — the
    // server sends only this account's dynamic position (`level` = reached, `reward_claimed_through` =
    // already accepted) and the client renders each level's Crystal/skill from its local file.
    (StatusCode::OK, Json(serde_json::json!({
        "ok": true,
        "level": level, "cxp": cxp, "into_level": into_level, "to_next": to_next,
        "max_level": commander::MAX_LEVEL,
        "reward_claimed_through": claimed_through,
        "pending_reward_levels": pending,
        "skill_grants": skill_grants,
    })))
}

/// The skill-unlock grants an account at `level` should hold, as `{skill_id, nonce}` JSON. Re-offered
/// (not gated by the crystal high-water) because a skill unlock rides an echoed `GrantCommanderSkill` that
/// only lands on a verified save — if that echo is lost it must be re-issued, and the sim + verifier
/// both treat the insert as idempotent, so re-offering an owned skill is harmless. `nonce` is a
/// decimal string (u64 exceeds JS/GDScript float precision).
///
/// ⚠️ SECURITY INVARIANT (review, defense-in-depth): this `skills_unlocked_up_to(level)` gate is the
/// ONLY authorization for an unlock — `commander_grants_valid` verifies the nonce is authentic but
/// does NOT re-check the level (it is a pure replay function with no DB level to read). The nonce is
/// level-independent and eternal, so this function MUST stay the sole issuer and MUST stay level-gated.
/// Any NEW path that emits a commander nonce (a debug/admin endpoint, a wider `skills_unlocked_up_to`)
/// needs its own level check, or it hands out an unlock the verifier cannot second-guess.
fn commander_skill_grants(secret: &str, account_id: u64, level: u32) -> Vec<serde_json::Value> {
    commander::skills_unlocked_up_to(level)
        .into_iter()
        .map(|skill| {
            let nonce = commander_nonce(secret, account_id, skill);
            serde_json::json!({"skill_id": skill, "nonce": nonce.to_string()})
        })
        .collect()
}

/// POST /commander/claim — accept the pending level rewards in ONE action (direct, no mailbox hop).
/// For every newly-reached level: credit that level's BOUND Crystal (from the config) and, if the level unlocks a skill, hand back
/// the `GrantCommanderSkill` nonce for the client to echo into its next save log. EXACTLY-ONCE via the
/// `reward_claimed_through` high-water advanced in the SAME transaction as the crystal credit — a second
/// press finds nothing owed and pays nothing.
#[derive(serde::Deserialize)]
struct CommanderClaimReq {
    /// Claim the reward track up to and including this level; 0 (or absent) = claim all reached.
    #[serde(default)]
    up_to: u32,
}

async fn commander_claim(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: Option<Json<CommanderClaimReq>>,
) -> (StatusCode, Json<serde_json::Value>) {
    // `up_to` = claim the reward TRACK up to and including this level (a per-row "Claim" on the ZZZ
    // track); 0 / no body = claim every reached level (the "Claim All" button). Option<Json> so an
    // empty POST body is still accepted.
    let up_to = body.map(|b| b.0.up_to).unwrap_or(0);
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "no database"})));
    };
    let mut tx = match db.begin().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    lock_account(&mut tx, acct.account_id as i64).await;
    if let Err(e) = sqlx::query("INSERT INTO commander (account_id) VALUES ($1) ON CONFLICT (account_id) DO NOTHING")
        .bind(acct.account_id as i64).execute(&mut *tx).await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
    }
    let (cxp, claimed_through): (i64, i32) = match sqlx::query_as(
        "SELECT cxp, reward_claimed_through FROM commander WHERE account_id = $1 FOR UPDATE",
    )
    .bind(acct.account_id as i64).fetch_one(&mut *tx).await
    {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    let level = commander::level_for_cxp(&st.commander_levels, cxp);
    // Skill grants are RE-OFFERED for every reached unlock level, independent of the crystal high-water
    // (see `commander_skill_grants`), so they ride even a claim that owes no NEW crystal — the recovery
    // path for a lost unlock echo (review finding). The crystal reward below stays exactly-once.
    let grants = commander_skill_grants(&st.roll_secret, acct.account_id, level);
    // The high-water is the last claimed level (0 on a fresh account). Level 1 is a real reward (free
    // welcome), so base 0 means level 1 is claimable immediately.
    let base = claimed_through as u32;
    // Claim up to `up_to` (a per-row claim) but never past what's actually been reached; 0 = claim all.
    let target = if up_to == 0 { level } else { up_to.min(level) };
    if target <= base {
        let _ = tx.rollback().await;
        return (StatusCode::OK, Json(serde_json::json!({"ok": true, "crystal": 0, "claimed_levels": [], "grants": grants})));
    }
    let newly: Vec<u32> = ((base + 1)..=target).collect();
    // Sum the per-level reward from the loaded config over the newly-reached levels — each level credits
    // its OWN Crystal, from the SAME file the client rendered, so payout matches display exactly.
    let crystal: i64 = commander::crystal_between(&st.commander_rewards, base, target);
    // BOUND crystal — earned, not bought.
    if let Err(e) = sqlx::query(
        "INSERT INTO crystal_wallet (account_id, paid, bound) VALUES ($1, 0, $2) \
         ON CONFLICT (account_id) DO UPDATE SET bound = crystal_wallet.bound + EXCLUDED.bound",
    )
    .bind(acct.account_id as i64).bind(crystal).execute(&mut *tx).await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
    }
    // `ref` carries the level RANGE, so a double credit is a duplicate row rather than a bigger number.
    if let Err(e) = sqlx::query(
        "INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES ($1, 0, $2, 'commander_level', $3)",
    )
    .bind(acct.account_id as i64).bind(crystal).bind(format!("cmdr:{}-{}", base + 1, target))
    .execute(&mut *tx).await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
    }
    // Advance the exactly-once high-water in the SAME tx. GREATEST guards a concurrent claim.
    if let Err(e) = sqlx::query(
        "UPDATE commander SET reward_claimed_through = GREATEST(reward_claimed_through, $2) WHERE account_id = $1",
    )
    .bind(acct.account_id as i64).bind(target as i32).execute(&mut *tx).await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
    }
    if let Err(e) = tx.commit().await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
    }
    // `grants` was computed before the tx (re-offered for every reached unlock level); the client
    // echoes each as `GrantCommanderSkill { skill_id, nonce }` into its next save, where the verifier
    // recomputes the same nonce. Idempotent, so re-applying an owned skill is a no-op.
    (StatusCode::OK, Json(serde_json::json!({
        "ok": true, "crystal": crystal, "claimed_levels": newly, "grants": grants, "level": level,
    })))
}

async fn bp_claimed_slots(db: &sqlx::PgPool, account_id: u64, season_id: i64, track: i16) -> Vec<u32> {
    sqlx::query_as::<_, (i16,)>(
        "SELECT slot FROM bp_claims WHERE account_id = $1 AND season_id = $2 AND track = $3 AND voided_at IS NULL",
    )
    .bind(account_id as i64).bind(season_id).bind(track)
    .fetch_all(db).await.unwrap_or_default()
    .into_iter().map(|(s,)| s as u32).collect()
}

/// Whether `account_id` holds the premium track for `season_id` (an active, unrevoked entitlement).
/// An error is returned, not defaulted: callers that GRANT on the answer must not guess.
async fn bp_premium_owned<'e, E: sqlx::PgExecutor<'e>>(db: E, account_id: u64, season_id: i64) -> Result<bool, sqlx::Error> {
    sqlx::query_as::<_, (i64,)>(
        "SELECT count(*) FROM bp_entitlements WHERE account_id = $1 AND season_id = $2 \
         AND tier = 'premium' AND active AND revoked_at IS NULL",
    )
    .bind(account_id as i64).bind(season_id)
    .fetch_one(db).await
    .map(|(c,)| c > 0)
}

/// GET /bp/state — everything the BP screen renders (server truth only).
async fn bp_state_ep(State(st): State<AppState>, headers: HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "bp needs a database"})));
    };
    let season = live_season(&st);
    let Some(season) = season else {
        return (StatusCode::OK, Json(serde_json::json!({"ok": false, "reason": "no active season"})));
    };
    let now = now_ms();
    let day = daily::day_of_ms(now as u64);
    let state_row: Option<(i64, String)> = sqlx::query_as(
        "SELECT xp, snapshot_json FROM bp_state WHERE account_id = $1 AND season_id = $2",
    )
    .bind(acct.account_id as i64).bind(season.season_id)
    .fetch_optional(db).await.ok().flatten();
    let (xp, snapshot): (u64, sim_core::state::Measures) = match &state_row {
        Some((x, sj)) => (*x as u64, serde_json::from_str(sj).unwrap_or_default()),
        None => (0, Default::default()),
    };
    let resp = BpStateResp {
        ok: true,
        reason: String::new(),
        season_id: season.season_id,
        ends_at_ms: season.ends_at_ms,
        now_ms: now,
        xp,
        level: bp::level_of(&season.config, xp),
        slots: season.config.slots,
        slot_cost: season.config.slot_cost,
        free_track: bp_free_track_view(&st, &season.config),
        claimed: bp_claimed_slots(db, acct.account_id, season.season_id, 0).await,
        premium_track: bp_track_view(&st, &season.config.premium_track),
        premium: bp_premium_owned(db, acct.account_id, season.season_id).await.unwrap_or(false),
        claimed_premium: bp_claimed_slots(db, acct.account_id, season.season_id, 1).await,
    };
    (StatusCode::OK, Json(serde_json::to_value(resp).unwrap_or_default()))
}


#[derive(serde::Deserialize)]
struct DevBpSeasonReq {
    /// Season length in days (default `bp::SEASON_DAYS`). Config = bp::default_config().
    days: Option<i64>,
    /// DEV overrides for fast E2E testing (skip the 6.6h grind to level 1).
    slot_cost: Option<u64>,
}

/// POST /dev/bp-season — create + activate a dev season starting now (dev-gated).
async fn dev_bp_season(State(st): State<AppState>, Json(req): Json<DevBpSeasonReq>) -> (StatusCode, Json<serde_json::Value>) {
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "bp needs a database"})));
    };
    // Never over a running season (round-10 audit): `bp_claim` reads the newest live season, so a
    // second one moved every premium buyer onto a pass they did not buy. And in production the
    // season is the real one: no test-speed `slot_cost`.
    if live_season(&st).is_some() {
        return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": "a season is already running"})));
    }
    let now = now_ms();
    let days = req.days.unwrap_or(bp::SEASON_DAYS).clamp(1, 120);
    let mut cfg = bp::default_config();
    if let (Some(v), false) = (req.slot_cost, st.prod) { cfg.slot_cost = v.max(1); }
    let config_json = serde_json::to_string(&cfg).unwrap_or_default();
    let seed = (now as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let row: Result<(i64,), _> = sqlx::query_as(
        "INSERT INTO seasons (starts_at_ms, ends_at_ms, seed, config_json) VALUES ($1, $2, $3, $4) RETURNING season_id",
    )
    .bind(now).bind(now + days * 86_400_000).bind(seed as i64).bind(&config_json)
    .fetch_one(db).await;
    match row {
        Ok((id,)) => {
            let season = bp::Season { season_id: id, starts_at_ms: now, ends_at_ms: now + days * 86_400_000, seed, config: cfg };
            *st.season.held() = Some(season);
            (StatusCode::OK, Json(serde_json::json!({"ok": true, "season_id": id})))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    }
}


/// Every mission the server will pay for, by `% id`. The client's mission list is a directory of
/// `.dialogue` files and the server has no access to it, so this is the second copy — and a second
/// copy that must agree with a directory is precisely the drift the story-mode plan warned about.
/// It is allowed here for one reason: `the_server_pays_for_exactly_the_missions_that_exist` binds
/// the two, so they fail together instead of diverging quietly.
///
/// Adding a mission is therefore two edits and a REDEPLOY. That is the cost of the server holding
/// the purse, and it is the right cost: the alternative is what this replaced, where any string a
/// client invented was a mission worth 1,000 Gil.
///
/// Flat reward for now (`MISSION_GIL`). When missions pay different amounts this becomes
/// `&[(&str, u64)]` and the constant goes away; nothing else about the shape changes.
/// In PLAY ORDER (user 2026-10-03: the prologue is the first chapter): each unlocks after the one
/// before it. `/story/progress` reports what is cleared; the story screen gates on it.
const STORY_MISSIONS: &[&str] = &["prologue", "1-1", "1-2"];

/// GET /story/progress → `{ok, cleared:[mission_id]}` — which missions this account has finished.
///
/// `story_claims` was write-only: the row that makes a reward once-only was also the only record
/// that a mission was cleared, and nothing could read it. That is what blocked sequential unlock.
///
/// CLEARED == PAID, and that is a decision, not an accident. The two can only diverge offline,
/// where the reward call never reaches the server — and story mode refuses to start offline for
/// exactly this reason (2026-08-10). Anything else would need a second table recording completion
/// separately, which would then have to agree with this one.
///
/// The list is ADVISORY. The client hides locked missions with it; the server does not refuse a
/// claim for a mission whose prerequisite is unmet, and it does not need to — `STORY_MISSIONS`
/// already caps the payout at each mission once per account, so playing out of order changes the
/// ORDER a player is paid, never the total. Enforcing sequence server-side would buy nothing and
/// cost a rule that has to stay in step with the writer's files.
async fn story_progress(State(st): State<AppState>, headers: HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "story progress needs a database"})));
    };
    let rows: Vec<(String,)> = match sqlx::query_as(
        "SELECT mission_id FROM story_claims WHERE account_id = $1 ORDER BY mission_id",
    )
    .bind(acct.account_id as i64)
    .fetch_all(db)
    .await
    {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    let cleared: Vec<String> = rows.into_iter().map(|(m,)| m).collect();
    (StatusCode::OK, Json(serde_json::json!({"ok": true, "cleared": cleared})))
}

#[derive(serde::Deserialize)]
struct StoryClaimReq {
    /// The mission's file basename, e.g. "trial". The PRIMARY KEY with the account, so a retry
    /// after a dropped response is a no-op rather than a second payout.
    mission_id: String,
}

/// A mission's first-clear reward. The client cannot mint it: it says WHICH mission finished and
/// the server decides whether that is worth anything, exactly once, and delivers it as mail — the
/// same road the battle pass takes. Nothing lands in the sim until the player claims that mail
/// through `/mail/claim`, which is `InputAuthority::Server` and so survives replay verification.
///
/// A client-authored gil grant would be a mint from the untrusted side, and the save carrying it
/// would fail replay. That is the whole reason this endpoint exists rather than a line in the
/// story view.
async fn story_claim(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<StoryClaimReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    /// TEST TUNING: every mission pays the same flat sum until a per-mission reward table exists.
    /// A table is a data question (which mission pays what), not a plumbing one, and the plumbing
    /// is what this is for.
    ///
    /// ⚠️ WHOEVER WRITES THAT TABLE OWNS AN ECONOMY DECISION, so it is written down here rather
    /// than discovered afterwards. This is a FIXED per-account mint of a FULLY LIQUID currency —
    /// the one shape the anti-alt design deliberately avoids everywhere else. Drops are
    /// crowd-scaled (∝1/N farmers) precisely so that more accounts do not mean more supply; store
    /// Crystals earned in play land in the BOUND bucket and cannot be moved. Story pays neither
    /// way: every new account gets the same Gil, and Gil goes straight to the market.
    ///
    /// Today that is 3 missions × 1,000 = 3,000 per account, ever — the 2nd party member (1,000)
    /// with change, or a third of the 3rd (10,000). Small enough to leave alone. At 50 missions it
    /// is 50,000, which is the 4th member, per alt, funnelled to one main by selling anything.
    ///
    /// So the table is where this gets decided, and there are only three honest answers: keep the
    /// total flat as the story grows (front-load it and pay later missions in something else), pay
    /// in a currency that cannot be traded (Gil has NO bound/tradeable split anywhere in this
    /// codebase — that is a sim-core change, not a line here), or accept it and say so in
    /// `docs/economy-invariants.md`. What must not happen is the number growing with the story
    /// because each mission was priced on its own.
    /// 10,000 a mission (user 2026-10-03; was 100,000): a mission is paid on the client's word, so
    /// the per-account total stays small.
    const MISSION_GIL: u64 = 10_000;

    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "story rewards need a database"})));
    };
    let mission = req.mission_id.trim();
    if mission.is_empty() || mission.len() > 64 || !mission.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"ok": false, "reason": "bad mission id"})));
    }
    // The mission must be one the SERVER knows. Without this the charset check above was the only
    // bound on `mission_id`, and it bounds nothing that matters: every distinct string is a fresh
    // `(account, mission_id)` row, so `a`, `b`, `c`, … each minted MISSION_GIL. An unbounded
    // faucet, reachable by anyone with a token and no need to open story mode at all.
    //
    // The server cannot verify that a mission was PLAYED — story mode runs on a sandbox bridge
    // outside the replay contract by design, so nothing the client sends is evidence. What it can
    // do is refuse to pay for missions that do not exist, which turns "unbounded" into "N missions
    // once per account, ever". That is the whole of the fix and it is worth being clear that it is
    // a bound, not a proof.
    if !STORY_MISSIONS.contains(&mission) {
        eprintln!("[STORY] acct {} claimed unknown mission '{mission}'", acct.account_id);
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"ok": false, "reason": "unknown mission"})));
    }

    let mut tx = match db.begin().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    lock_account(&mut tx, acct.account_id as i64).await;
    // The claim row and the mail are the SAME transaction: either the mission is marked paid AND
    // the gil is in the inbox, or neither. `ON CONFLICT DO NOTHING` makes the retry free.
    let ins = sqlx::query(
        "INSERT INTO story_claims (account_id, mission_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(acct.account_id as i64)
    .bind(mission)
    .execute(&mut *tx)
    .await;
    match ins {
        Ok(r) if r.rows_affected() == 1 => {}
        Ok(_) => {
            let _ = tx.commit().await;
            return (StatusCode::OK, Json(serde_json::json!({
                "ok": true, "mails": [], "gil": 0, "reason": "already claimed"
            })));
        }
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    }

    let reason = format!("story first clear: {mission}");
    let id = match mailbox_store::send(
        &mut *tx, acct.account_id, mailbox::MailKind::Reward, MISSION_GIL, "", 0, &reason,
    ).await {
        Ok(id) => id,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": format!("{e:?}")}))),
    };
    if tx.commit().await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": "persist failed — retry"})));
    }
    let mail = mailbox::MailItem {
        id, account_id: acct.account_id, kind: mailbox::MailKind::Reward,
        gil: MISSION_GIL, def_id: String::new(), qty: 0, item: None, reason,
    };
    println!("story-claim acct={} mission={mission} gil={MISSION_GIL} mail={id}", acct.account_id);
    (StatusCode::OK, Json(serde_json::json!({"ok": true, "mails": [mail], "gil": MISSION_GIL})))
}

#[derive(serde::Deserialize)]
struct BpClaimReq {
    /// Slots to claim (1-indexed).
    slots: Vec<u32>,
    /// 0 = free track (default), 1 = premium track (needs the `battle_pass` entitlement).
    #[serde(default)]
    track: u8,
}

/// POST /bp/claim — claim earned FREE-track slots. Idempotent per slot (bp_claims PK,
/// BP3); rewards mint BOUND and deliver as mail (BP5/BP6) — the BP screen claims that
/// mail immediately in the same click, so the mailbox is just the safe transport (a
/// full-inventory claim degrades gracefully into a waiting mail instead of vanishing).
/// The claims insert + the mail rows commit in ONE transaction: never a claim row without
/// its mail.
async fn bp_claim(State(st): State<AppState>, headers: HeaderMap, Json(req): Json<BpClaimReq>) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "bp needs a database"})));
    };
    let season = live_season(&st);
    let Some(season) = season else {
        return (StatusCode::OK, Json(serde_json::json!({"ok": false, "reason": "no active season"})));
    };
    if req.slots.is_empty() || req.slots.len() > 64 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"ok": false, "reason": "1..64 slots per call"})));
    }
    let Some(rewards) = bp::track_rewards(&season.config, req.track) else {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"ok": false, "reason": "unknown track"})));
    };
    let track = i16::from(req.track);
    if req.track == 1 {
        match bp_premium_owned(db, acct.account_id, season.season_id).await {
            Ok(true) => {}
            Ok(false) => return (StatusCode::FORBIDDEN, Json(serde_json::json!({"ok": false, "reason": "premium track not owned"}))),
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
        }
    }
    // Level gate reads the accrued truth.
    let xp: u64 = sqlx::query_as::<_, (i64,)>(
        "SELECT xp FROM bp_state WHERE account_id = $1 AND season_id = $2",
    )
    .bind(acct.account_id as i64).bind(season.season_id)
    .fetch_optional(db).await.ok().flatten().map(|(x,)| x as u64).unwrap_or(0);
    let level = bp::level_of(&season.config, xp);
    // No market lock: mail is rows now, so the deliveries below are plain INSERTs in the same
    // transaction as the claim rows. They used to rewrite the shared market blob, which forced
    // every claim through the process-wide lock and made it inherit `market_poisoned`.
    let mut tx = match db.begin().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    lock_account(&mut tx, acct.account_id as i64).await;
    // Premium again, LOCKED inside the claim's transaction (audit 2026-10-03): the check above runs
    // on the pool, so a refund committing between it and these inserts voided the claims that
    // existed and missed the ones written here. A refund's UPDATE of the entitlement row and this
    // FOR SHARE serialize; after it commits the row no longer matches.
    if req.track == 1 {
        let held: Result<Vec<(i64,)>, _> = sqlx::query_as(
            "SELECT account_id FROM bp_entitlements WHERE account_id = $1 AND season_id = $2 \
             AND tier = 'premium' AND active AND revoked_at IS NULL FOR SHARE",
        ).bind(acct.account_id as i64).bind(season.season_id).fetch_all(&mut *tx).await;
        match held {
            Ok(rows) if !rows.is_empty() => {}
            Ok(_) => return (StatusCode::FORBIDDEN, Json(serde_json::json!({"ok": false, "reason": "premium track not owned"}))),
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
        }
    }
    let mut slots: Vec<u32> = req.slots.clone();
    slots.sort_unstable();
    slots.dedup();
    let mut fresh: Vec<u32> = Vec::new();
    for slot in slots {
        if slot == 0 || slot > season.config.slots || slot > level {
            continue; // unearned/out-of-range → skip (client shouldn't send these)
        }
        let r = sqlx::query(
            "INSERT INTO bp_claims (account_id, season_id, track, slot) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
        )
        .bind(acct.account_id as i64).bind(season.season_id).bind(track).bind(slot as i16)
        .execute(&mut *tx).await;
        match r {
            Ok(res) if res.rows_affected() == 1 => fresh.push(slot),
            Ok(_) => {} // already claimed — idempotent no-op (BP3)
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
        }
    }
    if fresh.is_empty() {
        let _ = tx.commit().await;
        return (StatusCode::OK, Json(serde_json::json!({"ok": true, "mails": [], "reason": "nothing new to claim"})));
    }
    // Deliver the rewards as mail rows in the SAME transaction as the claim rows: either the
    // slot is marked claimed AND the item is in the inbox, or neither happened. Nothing to
    // hand-revert on failure — the transaction is the revert.
    let mut mails: Vec<mailbox::MailItem> = Vec::new();
    let mut granted: Vec<serde_json::Value> = Vec::new();
    for slot in &fresh {
        let Some(reward) = rewards.get(*slot as usize - 1) else { continue };
        let reason = format!("bp s{} {} slot {}", season.season_id, if track == 1 { "premium" } else { "free" }, slot);
        if let Err(e) = bp_grant_currency(&mut tx, acct.account_id, reward, &reason).await {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
        }
        match reward.kind {
            bp::RewardKind::Item => {}
            bp::RewardKind::Gil => {
                match mailbox_store::send(&mut *tx, acct.account_id, mailbox::MailKind::Reward, u64::from(reward.qty), "", 0, &reason).await {
                    Ok(id) => mails.push(mailbox::MailItem {
                        id, account_id: acct.account_id, kind: mailbox::MailKind::Reward,
                        gil: u64::from(reward.qty), def_id: String::new(), qty: 0, item: None, reason,
                    }),
                    Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": format!("{e:?}")}))),
                }
                continue;
            }
            k => {
                granted.push(serde_json::json!({"slot": slot, "kind": k, "qty": reward.qty}));
                continue;
            }
        }
        let Some(def) = st.data.item(&reward.def_id) else {
            eprintln!("[BP] season {} slot {} reward def '{}' missing from catalog — skipped", season.season_id, slot, reward.def_id);
            continue;
        };
        let mut item = data::instantiate_item(def);
        item.quantity = reward.qty.max(1);
        item.bound = true; // pass rewards = unsellable everywhere (BP6)
        match mailbox_store::send_item(&mut *tx, acct.account_id, mailbox::MailKind::Reward, &item, &reason).await {
            Ok(id) => mails.push(mailbox::MailItem {
                id, account_id: acct.account_id, kind: mailbox::MailKind::Reward,
                gil: 0, def_id: String::new(), qty: 0, item: Some(item), reason,
            }),
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
        }
    }
    if tx.commit().await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": "persist failed — retry"})));
    }
    println!("bp-claim acct={} slots={:?} mails={}", acct.account_id, fresh, mails.len());
    (StatusCode::OK, Json(serde_json::json!({"ok": true, "mails": mails, "granted": granted})))
}

/// Credit a pass slot's TICKETS or CRYSTAL straight to the account, ledgered; items and Gil go by
/// mail in `bp_claim`, so they are a no-op here.
async fn bp_grant_currency(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: u64,
    reward: &bp::SlotReward,
    reason: &str,
) -> Result<(), sqlx::Error> {
    let qty = i64::from(reward.qty);
    let ticket = match reward.kind {
        bp::RewardKind::StandardTicket => Some(gacha::STANDARD_COUPON_KIND),
        bp::RewardKind::LimitedTicket => Some(gacha::LIMITED_COUPON_KIND),
        _ => None,
    };
    if let Some(kind) = ticket {
        sqlx::query(
            "INSERT INTO bound_items (account_id, kind, qty) VALUES ($1, $2, $3) \
             ON CONFLICT (account_id, kind) DO UPDATE SET qty = bound_items.qty + EXCLUDED.qty",
        )
        .bind(account_id as i64).bind(kind).bind(qty).execute(&mut **tx).await?;
        sqlx::query("INSERT INTO bound_ledger (account_id, kind, delta, reason, ref) VALUES ($1, $2, $3, 'bp', $4)")
            .bind(account_id as i64).bind(kind).bind(qty).bind(reason).execute(&mut **tx).await?;
    }
    if reward.kind == bp::RewardKind::Crystal {
        sqlx::query(
            "INSERT INTO crystal_wallet (account_id, paid, bound) VALUES ($1, 0, $2) \
             ON CONFLICT (account_id) DO UPDATE SET bound = crystal_wallet.bound + EXCLUDED.bound",
        )
        .bind(account_id as i64).bind(qty).execute(&mut **tx).await?;
        sqlx::query("INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES ($1, 0, $2, 'bp', $3)")
            .bind(account_id as i64).bind(qty).bind(reason).execute(&mut **tx).await?;
    }
    Ok(())
}

/// The unit ids this account owns or has on the way: every character in its last verified save
/// (fielded and benched) plus every character still waiting in its mailbox. The Starter banner's
/// SR rule reads it so a new-character pull cannot pay one the player already has.
async fn owned_characters(
    st: &AppState,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: u64,
) -> Result<std::collections::BTreeSet<String>, sqlx::Error> {
    let mut owned = std::collections::BTreeSet::new();
    let json = st.saves.held().get(&account_id).map(|s| s.state_json.clone());
    let json = match json {
        Some(j) => Some(j),
        None => sqlx::query_as::<_, (String,)>("SELECT state_json FROM saves WHERE account_id = $1")
            .bind(account_id as i64).fetch_optional(&mut **tx).await?.map(|(j,)| j),
    };
    if let Some(g) = json.and_then(|j| serde_json::from_str::<sim_core::state::GameState>(&j).ok()) {
        for e in &g.grid.entities {
            if matches!(e.team, sim_core::grid::Team::Player) && !e.is_pet && !e.unit_def.is_empty() {
                owned.insert(e.unit_def.clone());
            }
        }
        for rc in g.roster.values() {
            owned.insert(rc.unit_def.clone());
        }
    }
    let mail: Vec<(Option<String>,)> = sqlx::query_as(
        "SELECT item_json FROM mailbox WHERE account_id = $1 AND claimed_at IS NULL AND voided_at IS NULL AND item_json LIKE '%exemplar_%'",
    ).bind(account_id as i64).fetch_all(&mut **tx).await?;
    for (j,) in mail {
        if let Some(it) = j.and_then(|j| serde_json::from_str::<sim_core::items::Item>(&j).ok()) {
            if let Some(u) = it.def_id.strip_prefix("exemplar_") {
                owned.insert(u.to_string());
            }
        }
    }
    Ok(owned)
}

/// Seed the gacha RNG from the server secret + the client's idempotency key. Deterministic in
/// the key, so a RETRY that reaches the roll before the row committed still produces the SAME
/// results — the idempotency row is the fast path, this is the belt-and-braces one. Keyed by the
/// secret so the outcome is unguessable, and domain-separated from the refine/drop nonces.
/// The seed a pull is rolled from.
///
/// `server_nonce` is the point (C04b). Without it the seed was a pure function of the secret, the
/// account id and `idem_key` — and the client chooses `idem_key`. That made the outcome the one
/// thing a client could search: pick a key, see what it pays, and if the secret ever leaked, grind
/// keys offline until one lands an SSR before ever spending a crystal. Even intact, "the player picks
/// an input to the draw" is not a property a paid random should have.
///
/// The nonce is drawn from the OS when the roll's idempotency row is claimed, and STORED on that
/// row. Storing it is what keeps a retry honest: the same `idem_key` replays the stored results,
/// and if that fast path is ever changed to re-derive instead, it re-derives the same seed.
fn gacha_seed(secret: &str, account_id: u64, idem_key: &str, server_nonce: u64) -> u64 {
    let mut h = blake3::Hasher::new();
    h.update(b"gacha-roll:v2");
    h.update(secret.as_bytes());
    h.update(&account_id.to_le_bytes());
    h.update(&server_nonce.to_le_bytes());
    h.update(&(idem_key.len() as u64).to_le_bytes());
    h.update(idem_key.as_bytes());
    digest_u64(&h)
}

fn gacha_status(e: gacha::GachaError) -> (StatusCode, String) {
    match e {
        gacha::GachaError::InsufficientCrystal => (StatusCode::CONFLICT, "not enough crystal".into()),
        gacha::GachaError::BadPullCount => (StatusCode::BAD_REQUEST, "pull count must be 1 or 10".into()),
        gacha::GachaError::InsufficientMileage => (StatusCode::CONFLICT, "not enough mileage".into()),
        gacha::GachaError::NotExchangeable => (StatusCode::BAD_REQUEST, "not an exchangeable unit".into()),
    }
}

/// Build the carrier item a pull pays out. Exemplars come from the catalog (they carry a name +
/// rarity); REGALIA carriers are code-only by design (`items::regalia_def`), so they are built
/// directly — the same construction `debug_grant_regalia` uses.
fn gacha_item(st: &AppState, def_id: &str) -> Option<sim_core::items::Item> {
    use sim_core::items::{Item, ItemKind, Rarity};
    if let Some(def) = st.data.item(def_id) {
        return Some(data::instantiate_item(def));
    }
    // EXACT lookup, not a `starts_with("regalia_")` prefix. Regalia carriers are code-only by
    // design, so the question is "does a def exist for this id", and `regalia_def` answers it
    // exactly. The prefix answered a different question that happened to agree: a Regalia whose id
    // broke the convention became unresolvable, and an unresolvable payout ABORTS the whole pull
    // (deliberately — see the caller). Fodder is not in `regalia_def`, so it keeps its own arm.
    if def_id == gacha::REGALIA_FODDER {
        return Some(Item::new(def_id, ItemKind::Regalia, 0, 0, Rarity::Rare));
    }
    if sim_core::items::regalia_def(def_id).is_some() {
        return Some(Item::new(def_id, ItemKind::Regalia, 0, 0, Rarity::Legendary));
    }
    None
}

#[derive(serde::Deserialize)]
struct GachaHistoryReq {
    /// Newest-first page size. Clamped server-side; a client asking for everything gets a page.
    #[serde(default)]
    limit: u32,
    /// Keyset cursor: return rolls strictly OLDER than this epoch-ms. 0 = start at the newest.
    /// A cursor rather than an offset because rows are only ever appended — OFFSET would re-read
    /// and re-skip the whole prefix on every page.
    #[serde(default)]
    before_ms: i64,
}

/// GET /gacha/history — the player's own pull record, newest first.
///
/// The rows already existed and nothing could read them: `gacha_rolls` is written on every pull
/// and never pruned, but `results_json` was only ever selected by `idem_key` on the retry path.
/// Same shape as `story_claims` before `/story/progress` — a complete record with no way out.
///
/// Worth having beyond the genre convention: it is the only answer to "I paid and got nothing"
/// that does not involve someone querying Postgres by hand, and it is the player-facing half of
/// the trust story whose other half is the rate-verification query (C05).
///
/// `pity_count` / `pulls_since_sr` are recorded per pull and cannot be backfilled, which is why
/// they are stored now rather than when something needs them. Today they are a STREAK, not a
/// mechanic: the banners are 4% flat with no soft ramp and no ceiling (gacha.rs, 2026-07-31), so
/// there is no pity curve for them to describe. `-1` marks rolls written before the columns existed.
async fn gacha_history(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(req): Query<GachaHistoryReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "gacha history needs a database"})));
    };
    let limit = req.limit.clamp(1, 200) as i64;
    let before = if req.before_ms > 0 { req.before_ms } else { i64::MAX };
    let rows: Vec<(String, String, i64, i32, i32)> = match sqlx::query_as(
        "SELECT banner_id, results_json, (EXTRACT(EPOCH FROM at) * 1000)::bigint, pity_count, pulls_since_sr \
         FROM gacha_rolls WHERE account_id = $1 AND (EXTRACT(EPOCH FROM at) * 1000)::bigint < $2 \
         ORDER BY at DESC LIMIT $3",
    )
    .bind(acct.account_id as i64).bind(before).bind(limit)
    .fetch_all(db).await
    {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    let mut oldest = 0i64;
    let entries: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(banner_id, results, at_ms, pity, since_sr)| {
            oldest = at_ms;
            serde_json::json!({
                "banner_id": banner_id,
                "at_ms": at_ms,
                "pity_count": pity,
                "pulls_since_sr": since_sr,
                // An in-flight roll that never committed leaves '[]'; it is not a pull and the
                // client should render nothing for it rather than an empty row.
                "pulls": serde_json::from_str::<serde_json::Value>(&results).unwrap_or(serde_json::Value::Array(vec![])),
            })
        })
        .collect();
    // The cursor for the next page. Absent when this page was not full — there is no next.
    let more = entries.len() as i64 == limit;
    (StatusCode::OK, Json(serde_json::json!({
        "ok": true,
        "entries": entries,
        "next_before_ms": if more { oldest } else { 0 },
    })))
}

#[derive(serde::Deserialize)]
struct GachaRollReq {
    banner_id: String,
    count: u32,
    idem_key: String,
    /// The player has SEEN the shortfall and agreed to buy it. Absent or false and a pull needing
    /// coupons the account does not hold is REFUSED with the price, rather than quietly spending
    /// crystal — the confirm is the whole point of the two-step.
    #[serde(default)]
    buy_shortfall: bool,
    /// What the 402 quoted. Optional so an older client still works; when present the server
    /// refuses to charge more than it, which is the half of "binding" that matters.
    #[serde(default)]
    quoted_crystal: Option<u64>,
}

/// `POST /gacha/roll` — debit, roll, advance pity and mint the rewards as MAIL, all in ONE
/// transaction. Replaying the same `idem_key` returns the stored results and charges nothing.
///
/// The roll happens HERE and never in the sim: a pull is money, so a client-side draw could be
/// save-scummed, and adding a draw to the sim's single PCG32 shifts every downstream outcome.
/// The sim only ever sees the delivered item (mail → `ClaimMail`, server-authored + exactly-once).
async fn gacha_roll(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<GachaRollReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "gacha needs a database"})));
    };
    if st.sessions.tainted.held().contains(&acct.account_id) {
        return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": "session out of sync — resync before pulling"})));
    }
    if req.idem_key.is_empty() || req.idem_key.len() > 64 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"ok": false, "reason": "idem_key must be 1..64 chars"})));
    }
    let Some(cfg) = gacha::banner(&req.banner_id) else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"ok": false, "reason": "unknown banner"})));
    };
    // Validate the count here; the PRICE is settled below, once the coupon balance is known.
    if let Err(e) = gacha::cost_of(req.count) {
        let (code, reason) = gacha_status(e);
        return (code, Json(serde_json::json!({"ok": false, "reason": reason})));
    }
    // FAST PATH: this key already rolled → replay the stored result, charge nothing.
    if let Ok(Some((results,))) = sqlx::query_as::<_, (String,)>(
        "SELECT results_json FROM gacha_rolls WHERE idem_key = $1 AND account_id = $2",
    )
    .bind(&req.idem_key).bind(acct.account_id as i64)
    .fetch_optional(db).await
    {
        let pulls: serde_json::Value = serde_json::from_str(&results).unwrap_or(serde_json::Value::Null);
        return (StatusCode::OK, Json(serde_json::json!({"ok": true, "reason": "idempotent", "pulls": pulls})));
    }

    // No market lock. The deliveries below are mail ROWS in this transaction, not a rewrite of
    // the shared market blob — which is the whole reason mail left the blob: a pull is the one
    // path that takes real money and it should not queue behind unrelated market traffic.
    let mut tx = match db.begin().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    lock_account(&mut tx, acct.account_id as i64).await;
    // Claim the key INSIDE the tx — two concurrent retries can't both roll.
    // Drawn HERE, inside the transaction that claims the key, so the nonce and the roll it seeds
    // live or die together: an aborted pull rolls the row back and a retry draws a fresh one.
    let server_nonce: u64 = rand::random();
    match sqlx::query("INSERT INTO gacha_rolls (idem_key, account_id, banner_id, results_json, server_nonce) VALUES ($1, $2, $3, '[]', $4) ON CONFLICT DO NOTHING")
        .bind(&req.idem_key).bind(acct.account_id as i64).bind(&req.banner_id).bind(server_nonce as i64)
        .execute(&mut *tx).await
    {
        Ok(r) if r.rows_affected() == 1 => {}
        Ok(_) => return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": "roll already in flight — retry"}))),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    }
    // WALLET — locked for the duration so a parallel pull can't double-spend.
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT paid, bound FROM crystal_wallet WHERE account_id = $1 FOR UPDATE",
    ).bind(acct.account_id as i64).fetch_optional(&mut *tx).await.ok().flatten();
    let mut wallet = gacha::Wallet {
        paid: row.map(|(t, _)| t).unwrap_or(0).max(0) as u64,
        bound: row.map(|(_, b)| b).unwrap_or(0).max(0) as u64,
    };
    // ---- COUPONS ARE THE PULL CURRENCY (user 2026-08-19) --------------------------------------
    //
    // Crystal buy coupons, coupons buy pulls. Holding 2 and pressing x10 does not fail: the server
    // answers with the shortfall and its price, the player confirms, and the SAME request returns
    // with `buy_shortfall` — buy and roll then happen in ONE transaction.
    //
    // Which is also the answer to "how do two steps stay idempotent": they do not need to. The
    // purchase lives inside the transaction that claims `idem_key`, so a retry replays the stored
    // result and charges nothing, for the crystal exactly as for the pulls. A separate buy endpoint
    // would have needed a second key and a way to reconcile a buy whose roll never happened.
    // THE BANNER'S OWN TICKET (2026-09-30): Standard tickets on Starter/Standard, Limited tickets
    // on Limited/Weapon; there is no ticket that spends everywhere.
    let ticket = gacha::ticket_kind(cfg.kind);
    let held: Option<(i64,)> = sqlx::query_as(
        "SELECT qty FROM bound_items WHERE account_id = $1 AND kind = $2 FOR UPDATE",
    )
    .bind(acct.account_id as i64).bind(ticket)
    .fetch_optional(&mut *tx).await.ok().flatten();
    let have = held.map(|(q,)| q).unwrap_or(0).max(0) as u64;
    // NEWCOMER (Starter): at most `NEWCOMER_PULLS`, open for all of them — refused past the end
    // rather than trimmed. Row locked with the wallet so two presses cannot both be "the last ten".
    let newcomer: Option<(i32, bool)> = if cfg.kind == gacha::BannerType::Newcomer {
        if let Err(e) = sqlx::query("INSERT INTO gacha_newcomer (account_id, made, closed) VALUES ($1, 0, FALSE) ON CONFLICT (account_id) DO NOTHING")
            .bind(acct.account_id as i64).execute(&mut *tx).await
        {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
        }
        let row: Option<(i32, bool)> = sqlx::query_as("SELECT made, closed FROM gacha_newcomer WHERE account_id = $1 FOR UPDATE")
            .bind(acct.account_id as i64).fetch_optional(&mut *tx).await.ok().flatten();
        let (made, closed) = row.unwrap_or((0, false));
        if gacha::newcomer_remaining(made.max(0) as u32, closed) < req.count {
            let _ = tx.rollback().await;
            return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": "newcomer banner is closed for this account"})));
        }
        Some((made, closed))
    } else {
        None
    };
    // STANDARD opens once the Starter banner is spent (user 2026-09-30), and counts its own pulls
    // toward the one-time SSR selector.
    let standard_made: Option<i32> = if cfg.kind == gacha::BannerType::Standard {
        let nc: Option<(i32, bool)> = sqlx::query_as("SELECT made, closed FROM gacha_newcomer WHERE account_id = $1")
            .bind(acct.account_id as i64).fetch_optional(&mut *tx).await.ok().flatten();
        if gacha::newcomer_remaining(nc.map_or(0, |r| r.0.max(0) as u32), nc.is_some_and(|r| r.1)) > 0 {
            let _ = tx.rollback().await;
            return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": "finish the starter banner first"})));
        }
        if let Err(e) = sqlx::query("INSERT INTO gacha_standard (account_id) VALUES ($1) ON CONFLICT (account_id) DO NOTHING")
            .bind(acct.account_id as i64).execute(&mut *tx).await
        {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
        }
        let made: Option<(i32,)> = sqlx::query_as("SELECT made FROM gacha_standard WHERE account_id = $1 FOR UPDATE")
            .bind(acct.account_id as i64).fetch_optional(&mut *tx).await.ok().flatten();
        Some(made.map_or(0, |r| r.0))
    } else {
        None
    };
    let (need_coupons, crystal_cost) = match gacha::pull_price_on(cfg.kind, req.count, have) {
        Ok(p) => p,
        Err(e) => { let (code, reason) = gacha_status(e); return (code, Json(serde_json::json!({"ok": false, "reason": reason}))); }
    };
    // THE QUOTE IS A CEILING, not a guess. `have` is re-read under `FOR UPDATE` on the confirm, so
    // a coupon that arrived in between makes the real price LOWER than what was shown — which is
    // fine and is the only direction it can move, because nothing else spends coupons. What must
    // never happen is charging MORE than the quote: the client sends back what it was told, and a
    // price above it aborts rather than silently taking the difference.
    if let Some(quoted) = req.quoted_crystal {
        if req.buy_shortfall && crystal_cost > quoted {
            let _ = tx.rollback().await;
            return (StatusCode::CONFLICT, Json(serde_json::json!({
                "ok": false, "reason": "price moved", "crystal": crystal_cost, "quoted": quoted,
            })));
        }
    }
    if crystal_cost > 0 && !req.buy_shortfall {
        // Not an error — a QUOTE. The client shows "you have 2, buy 8 for 2400?" and re-sends the
        // same idem_key with the confirm. Nothing has been charged, and rolling back releases the
        // key so the confirmed call can claim it.
        let _ = tx.rollback().await;
        return (StatusCode::PAYMENT_REQUIRED, Json(serde_json::json!({
            "ok": false, "reason": "need coupons",
            "have": have, "need": need_coupons,
            "buy": need_coupons.saturating_sub(have), "crystal": crystal_cost,
        })));
    }
    if crystal_cost > 0 && row.is_some_and(|(p, b)| wallet_in_debt(p, b)) {
        let _ = tx.rollback().await;
        // 409, not 402 (audit 2026-10-03): the client reads every 402 from a roll as a coupon
        // quote, so the debt showed as "short 0 coupons, buy for 0 Crystal" in a loop.
        return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": "crystal balance is negative", "debt": true})));
    }
    let spend = match wallet.spend(crystal_cost) {
        Ok(s) => s,
        Err(e) => { let (code, reason) = gacha_status(e); return (code, Json(serde_json::json!({"ok": false, "reason": reason}))); }
    };
    // SUBTRACT what is spent, never assign what is left.
    //
    // The row is locked FOR UPDATE above, so `have` is current — but writing `qty = kept` still
    // makes the balance a function of a value read before the write, and any credit landing
    // between them (a ladder claim in another request) would be erased. Buying is not written at
    // all: the shortfall exists only to be spent in the same breath, so the net effect on the
    // stored balance is `- min(need, have)`.
    let spent_coupons = need_coupons.min(have) as i64;
    if let Err(e) = sqlx::query(
        "UPDATE bound_items SET qty = GREATEST(qty - $3, 0) WHERE account_id = $1 AND kind = $2",
    )
    .bind(acct.account_id as i64).bind(ticket).bind(spent_coupons)
    .execute(&mut *tx).await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
    }
    // THE AUDIT TRAIL. Both movements, both in this transaction — the coupons BOUGHT with crystal and
    // the coupons SPENT on the pull. Recorded separately rather than netted, because "bought 8,
    // spent 10" and "spent 2" are different facts about the same press and only the first can be
    // reconciled against the crystal side.
    // BOTH ROWS ARE REAL MOVEMENTS, and together they must net to what the balance did.
    //
    // The spend is the FULL requirement, not the part that came out of the stored balance: a pull
    // of ten consumes ten coupons whether eight of them were bought a microsecond earlier or not.
    // Logging `-min(need, have)` alongside `+bought` made the two disagree — holding 2 and pulling
    // 10 wrote +8 and -2 for a net of +6 against a balance that fell by 2, so eight coupons
    // existed in the audit trail that no counter ever held.
    let (bought, spent_logged) = gacha::coupon_movements(need_coupons, have);
    for (delta, reason) in [(bought as i64, "buy"), (-(spent_logged as i64), "spend")] {
        if delta == 0 {
            continue;
        }
        if let Err(e) = sqlx::query(
            "INSERT INTO bound_ledger (account_id, kind, delta, reason, ref) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(acct.account_id as i64).bind(ticket).bind(delta).bind(reason)
        .bind(&req.idem_key)
        .execute(&mut *tx).await
        {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()})));
        }
    }
    // PITY — same row lock, same reason.
    let bt = gacha::pity_key(cfg.kind);
    let prow: Option<(i32, i32, bool)> = sqlx::query_as(
        "SELECT count, pulls_since_sr, lost_5050 FROM gacha_pity WHERE account_id = $1 AND banner_type = $2 FOR UPDATE",
    ).bind(acct.account_id as i64).bind(bt).fetch_optional(&mut *tx).await.ok().flatten();
    let mut pity = prow.map(|(c, s, l)| gacha::PityState {
        count: c.max(0) as u32, pulls_since_sr: s.max(0) as u32, lost_5050: l,
    }).unwrap_or_default();
    // MILEAGE — minted on EVERY pull regardless of what it produced (it is a parallel faucet,
    // not a pity backstop — see the gacha.rs module header). Same row lock, same tx.
    // In the banner's own currency (`gacha::mileage_kind`); the table name is one of two constants.
    let mtable = gacha::mileage_table(gacha::mileage_kind(cfg.kind));
    let mrow: Option<(i64,)> = sqlx::query_as(&format!(
        "SELECT tickets FROM {mtable} WHERE account_id = $1 FOR UPDATE",
    )).bind(acct.account_id as i64).fetch_optional(&mut *tx).await.ok().flatten();
    let mut mileage = gacha::Mileage { tickets: mrow.map(|(t,)| t).unwrap_or(0).max(0) as u64 };
    mileage.earn(req.count);
    // The Starter banner's SR rule needs what the account already has.
    let mut owned = if newcomer.is_some() {
        match owned_characters(&st, &mut tx, acct.account_id).await {
            Ok(o) => o,
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
        }
    } else {
        std::collections::BTreeSet::new()
    };

    let mut rng = sim_core::rng::Pcg32::new(gacha_seed(&st.roll_secret, acct.account_id, &req.idem_key, server_nonce));
    let rates = gacha::rates_for(cfg.kind);
    let pulls = gacha::roll_batch(&rates, &mut pity, &mut rng, req.count);

    // RESOLVE EVERY PAYOUT BEFORE MINTING ANY OF THEM. An unresolvable def_id must abort the
    // whole pull, not be skipped: the crystal are already debited in this transaction, so skipping
    // silently short-changes a player who paid full price — the exact thing `payout_def`'s own
    // "never charged for an empty result" rule exists to prevent. Two passes means there is
    // nothing to undo; the mailbox is untouched until every item is known good, and the early
    // return drops `tx`, rolling back the debit AND releasing the idempotency key.
    // `payout_def` draws from `rng`, so this pass must run exactly once per pull, in order.
    let mut resolved: Vec<(&'static str, sim_core::items::Item)> = Vec::with_capacity(pulls.len());
    for p in &pulls {
        let def_id = if newcomer.is_some() {
            let d = gacha::starter_payout(cfg, p, &mut rng, &owned);
            if let Some(u) = d.strip_prefix("exemplar_") {
                owned.insert(u.to_string());
            }
            d
        } else {
            gacha::payout_def(cfg, p, &mut rng)
        };
        let Some(item) = gacha_item(&st, def_id) else {
            eprintln!("[GACHA] banner {} payout '{}' not resolvable — pull ABORTED, nothing charged", cfg.id, def_id);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "ok": false, "reason": "banner misconfigured — nothing was charged",
            })));
        };
        resolved.push((def_id, item));
    }

    // Deliver as mail ROWS inside the pull's own transaction, so a delivery cannot survive a
    // rolled-back debit or vice versa. Built before the writes below only to keep `results_json`
    // available for the `gacha_rolls` audit row.
    let mut mails: Vec<mailbox::MailItem> = Vec::new();
    let mut out = Vec::new();
    let reason = format!("gacha {}", cfg.id);
    for (p, (def_id, item)) in pulls.iter().zip(resolved) {
        out.push(serde_json::json!({
            "def_id": def_id,
            "rarity": format!("{:?}", p.rarity),
            "featured": p.featured,
        }));
        match mailbox_store::send_item(&mut *tx, acct.account_id, mailbox::MailKind::Reward, &item, &reason).await {
            Ok(id) => mails.push(mailbox::MailItem {
                id, account_id: acct.account_id, kind: mailbox::MailKind::Reward,
                gil: 0, def_id: String::new(), qty: 0, item: Some(item), reason: reason.clone(),
            }),
            Err(e) => {
                eprintln!("[GACHA] delivery failed for acct {} — pull rolled back: {e}", acct.account_id);
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": "persist failed — retry"})));
            }
        }
    }
    let results_json = serde_json::Value::Array(out);

    let writes = async {
        // A DELTA WITH A FLOOR, never an absolute write.
        //
        // This used to store the post-spend totals it had computed in memory: `SET paid =
        // EXCLUDED.paid`. Two things were wrong with that, and only one of them is a race.
        //
        //   The audit's point: turn the upsert into a no-op — drop the ON CONFLICT clause — and
        //   every pull becomes FREE while all 183 tests stay green, byte for byte. Nothing
        //   asserted that crystal left the wallet, because the write and the check were the same
        //   number computed once in memory.
        //
        //   And it writes a value derived from a read. Any credit landing between them — a store
        //   grant, a daily claim — is erased by the pull that follows it.
        //
        // As a delta, both close together: the debit is what the DATABASE subtracts, and the
        // `>= $2` guard means a debit that cannot be paid matches zero rows and fails the roll
        // instead of granting it. A missed debit is now a failed pull, which is the direction to
        // fail in.
        let debited = sqlx::query(
            "UPDATE crystal_wallet SET paid = paid - $2, bound = bound - $3 \
             WHERE account_id = $1 AND paid >= $2 AND bound >= $3",
        )
            .bind(acct.account_id as i64)
            .bind(spend.from_paid as i64)
            .bind(spend.from_bound as i64)
            .execute(&mut *tx).await?
            .rows_affected();
        if debited != 1 && (spend.from_paid > 0 || spend.from_bound > 0) {
            // The balance moved under us, or there is no wallet row to debit. Either way the
            // transaction rolls back and nothing was rolled.
            return Err(sqlx::Error::RowNotFound);
        }
        sqlx::query("INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES ($1, $2, $3, 'gacha-pull', $4)")
            .bind(acct.account_id as i64).bind(-(spend.from_paid as i64)).bind(-(spend.from_bound as i64)).bind(&req.idem_key)
            .execute(&mut *tx).await?;
        if let Some((made, _)) = newcomer {
            // Closed by the fiftieth pull; an SSR before it no longer closes the banner (2026-09-30).
            let made_now = made.max(0) + req.count as i32;
            let closed_now = made_now >= gacha::NEWCOMER_PULLS as i32;
            sqlx::query("UPDATE gacha_newcomer SET made = $2, closed = $3 WHERE account_id = $1")
                .bind(acct.account_id as i64).bind(made_now).bind(closed_now)
                .execute(&mut *tx).await?;
        }
        sqlx::query("INSERT INTO gacha_pity (account_id, banner_type, count, pulls_since_sr, lost_5050) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (account_id, banner_type) DO UPDATE SET count = EXCLUDED.count, pulls_since_sr = EXCLUDED.pulls_since_sr, lost_5050 = EXCLUDED.lost_5050")
            .bind(acct.account_id as i64).bind(bt).bind(pity.count as i32).bind(pity.pulls_since_sr as i32).bind(pity.lost_5050)
            .execute(&mut *tx).await?;
        sqlx::query(&format!("INSERT INTO {mtable} (account_id, tickets) VALUES ($1, $2) ON CONFLICT (account_id) DO UPDATE SET tickets = EXCLUDED.tickets"))
            .bind(acct.account_id as i64).bind(mileage.tickets as i64)
            .execute(&mut *tx).await?;
        if let Some(made) = standard_made {
            sqlx::query("UPDATE gacha_standard SET made = $2 WHERE account_id = $1")
                .bind(acct.account_id as i64).bind(made.max(0) + req.count as i32)
                .execute(&mut *tx).await?;
        }
        // ACCOUNT-SCOPED. This was safe only while `idem_key` was a global primary key; widening
        // the PK to (account_id, idem_key) made two accounts able to hold the same key, and an
        // unscoped UPDATE then writes B's results over A's row as well as its own. A replaying its
        // key — or opening /gacha/history — would read B's pulls: the cross-account leak the
        // widening was meant to close, moved one statement along.
        sqlx::query("UPDATE gacha_rolls SET results_json = $1, pity_count = $3, pulls_since_sr = $4 WHERE idem_key = $2 AND account_id = $5")
            .bind(results_json.to_string()).bind(&req.idem_key)
            .bind(pity.count as i32).bind(pity.pulls_since_sr as i32)
            .bind(acct.account_id as i64)
            .execute(&mut *tx).await?;
        Ok::<(), sqlx::Error>(())
    }.await;

    let committed = match writes { Ok(()) => tx.commit().await.is_ok(), Err(_) => false };
    if !committed {
        // Nothing to hand-revert: the deliveries are rows in the same transaction as the debit,
        // so the rollback takes them with it.
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": "persist failed — retry"})));
    }
    println!("gacha-roll acct={} banner={} n={} coupons={} crystal={}", acct.account_id, cfg.id, req.count, need_coupons, crystal_cost);
    (StatusCode::OK, Json(serde_json::json!({
        "ok": true, "reason": "rolled",
        "pulls": results_json,
        "mails": mails,
        // u64 as STRINGS — Godot's JSON parser is float-based and loses precision past 2^53.
        "paid": wallet.paid.to_string(),
        // Legacy key until the Unity client reads `paid` (2026-10-01).
        "tradeable": wallet.paid.to_string(),
        "bound": wallet.bound.to_string(),
        "mileage": mileage.tickets.to_string(),
        "mileage_kind": if mtable == "gacha_mileage" { "limited" } else { "standard" },
        "pity": {"count": pity.count, "pulls_since_sr": pity.pulls_since_sr, "lost_5050": pity.lost_5050},
    })))
}

#[derive(serde::Deserialize)]
struct GachaExchangeReq {
    def_id: String,
    idem_key: String,
    /// "limited" (default — the pre-split currency) or "standard" (Memory Fragments only).
    #[serde(default)]
    currency: Option<String>,
}

/// `POST /gacha/exchange` — spend `gacha::MILEAGE_COST` tickets on any `EXCHANGE_POOL` unit,
/// delivered as MAIL like a pull. One transaction, idempotent on `idem_key`.
///
/// This is the half that makes a pull never wasted: tickets accrue on EVERY pull of EVERY banner
/// into one never-expiring pool, and the player picks what to spend them on — including a unit
/// that is not currently featured. It is NOT a pity payout; see the gacha.rs module header.
async fn gacha_exchange(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<GachaExchangeReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "gacha needs a database"})));
    };
    if req.idem_key.is_empty() || req.idem_key.len() > 64 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"ok": false, "reason": "idem_key must be 1..64 chars"})));
    }
    let kind = match req.currency.as_deref() {
        None | Some("limited") => gacha::MileageKind::Limited,
        Some("standard") => gacha::MileageKind::Standard,
        Some(_) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"ok": false, "reason": "unknown currency"}))),
    };
    if !gacha::is_exchangeable_with(kind, &req.def_id) {
        let (code, reason) = gacha_status(gacha::GachaError::NotExchangeable);
        return (code, Json(serde_json::json!({"ok": false, "reason": reason})));
    }
    let mtable = gacha::mileage_table(kind);
    // FAST PATH: already exchanged on this key → report it, spend nothing.
    if let Ok(Some((def_id,))) = sqlx::query_as::<_, (String,)>(
        "SELECT def_id FROM gacha_exchanges WHERE idem_key = $1 AND account_id = $2",
    )
    .bind(&req.idem_key).bind(acct.account_id as i64)
    .fetch_optional(db).await
    {
        return (StatusCode::OK, Json(serde_json::json!({"ok": true, "reason": "idempotent", "def_id": def_id})));
    }
    // Resolve the carrier BEFORE anything is charged — an unresolvable def must cost nothing.
    let Some(item) = gacha_item(&st, &req.def_id) else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"ok": false, "reason": "unit not resolvable"})));
    };

    // No market lock. The deliveries below are mail ROWS in this transaction, not a rewrite of
    // the shared market blob — which is the whole reason mail left the blob: a pull is the one
    // path that takes real money and it should not queue behind unrelated market traffic.
    let mut tx = match db.begin().await {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    lock_account(&mut tx, acct.account_id as i64).await;
    match sqlx::query("INSERT INTO gacha_exchanges (idem_key, account_id, def_id, spent) VALUES ($1, $2, $3, 0) ON CONFLICT DO NOTHING")
        .bind(&req.idem_key).bind(acct.account_id as i64).bind(&req.def_id)
        .execute(&mut *tx).await
    {
        Ok(r) if r.rows_affected() == 1 => {}
        Ok(_) => return (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": "exchange already in flight — retry"}))),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    }
    let mrow: Option<(i64,)> = sqlx::query_as(&format!(
        "SELECT tickets FROM {mtable} WHERE account_id = $1 FOR UPDATE",
    )).bind(acct.account_id as i64).fetch_optional(&mut *tx).await.ok().flatten();
    let mut mileage = gacha::Mileage { tickets: mrow.map(|(t,)| t).unwrap_or(0).max(0) as u64 };
    let spent = match mileage.exchange_in(kind, &req.def_id) {
        Ok(s) => s,
        Err(e) => { let (code, reason) = gacha_status(e); return (code, Json(serde_json::json!({"ok": false, "reason": reason}))); }
    };

    let mail_id = match mailbox_store::send_item(&mut *tx, acct.account_id, mailbox::MailKind::Reward, &item, "gacha mileage").await {
        Ok(id) => id,
        Err(e) => {
            eprintln!("[GACHA] exchange delivery failed for acct {} — rolled back: {e}", acct.account_id);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": "persist failed — retry"})));
        }
    };

    let writes = async {
        sqlx::query(&format!("INSERT INTO {mtable} (account_id, tickets) VALUES ($1, $2) ON CONFLICT (account_id) DO UPDATE SET tickets = EXCLUDED.tickets"))
            .bind(acct.account_id as i64).bind(mileage.tickets as i64)
            .execute(&mut *tx).await?;
        // Account-scoped for the same reason as `gacha_rolls` above.
        sqlx::query("UPDATE gacha_exchanges SET spent = $1 WHERE idem_key = $2 AND account_id = $3")
            .bind(spent as i64).bind(&req.idem_key).bind(acct.account_id as i64)
            .execute(&mut *tx).await?;
        Ok::<(), sqlx::Error>(())
    }.await;

    let committed = match writes { Ok(()) => tx.commit().await.is_ok(), Err(_) => false };
    if !committed {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": "persist failed — retry"})));
    }
    let mails = vec![mailbox::MailItem {
        id: mail_id, account_id: acct.account_id, kind: mailbox::MailKind::Reward,
        gil: 0, def_id: String::new(), qty: 0, item: Some(item), reason: "gacha mileage".into(),
    }];
    println!("gacha-exchange acct={} def={} spent={}", acct.account_id, req.def_id, spent);
    (StatusCode::OK, Json(serde_json::json!({
        "ok": true, "reason": "exchanged",
        "def_id": req.def_id,
        "mails": mails,
        "mileage": mileage.tickets.to_string(),
    })))
}

#[derive(serde::Deserialize)]
struct StandardSelectReq {
    def_id: String,
}

/// `POST /gacha/standard-select` — the one-time Standard SSR selector (user 2026-09-30): once the
/// account has made `STANDARD_SELECT_AT` Standard pulls, it names one `STANDARD_POOL` character and
/// receives it as mail. `selected` is the exactly-once guard; repeating the same pick is idempotent.
async fn gacha_standard_select(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<StandardSelectReq>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "gacha needs a database"})));
    };
    if !gacha::is_standard_selectable(&req.def_id) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"ok": false, "reason": "not a standard SSR"})));
    }
    let Some(item) = gacha_item(&st, &req.def_id) else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"ok": false, "reason": "unit not resolvable"})));
    };
    let res = async {
        let mut tx = db.begin().await?;
        lock_account(&mut tx, acct.account_id as i64).await;
        let row: Option<(i32, Option<String>)> = sqlx::query_as(
            "SELECT made, selected FROM gacha_standard WHERE account_id = $1 FOR UPDATE",
        ).bind(acct.account_id as i64).fetch_optional(&mut *tx).await?;
        let (made, selected) = row.unwrap_or((0, None));
        if let Some(prev) = selected {
            return Ok::<_, sqlx::Error>(Err(if prev == req.def_id { "idempotent" } else { "already selected" }));
        }
        if (made.max(0) as u32) < gacha::STANDARD_SELECT_AT {
            return Ok(Err("not enough standard pulls"));
        }
        sqlx::query("UPDATE gacha_standard SET selected = $2 WHERE account_id = $1")
            .bind(acct.account_id as i64).bind(&req.def_id).execute(&mut *tx).await?;
        let id = mailbox_store::send_item(&mut *tx, acct.account_id, mailbox::MailKind::Reward, &item, "gacha standard select")
            .await.map_err(|e| sqlx::Error::Protocol(format!("{e:?}")))?;
        tx.commit().await?;
        Ok(Ok(id))
    }.await;
    match res {
        Ok(Ok(mail_id)) => (StatusCode::OK, Json(serde_json::json!({"ok": true, "reason": "selected", "def_id": req.def_id, "mail_id": mail_id}))),
        Ok(Err("idempotent")) => (StatusCode::OK, Json(serde_json::json!({"ok": true, "reason": "idempotent", "def_id": req.def_id}))),
        Ok(Err(why)) => (StatusCode::CONFLICT, Json(serde_json::json!({"ok": false, "reason": why}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    }
}

/// `GET /gacha/state` — wallet + mileage + pity + the live banner list, for the UI.
async fn gacha_state(State(st): State<AppState>, headers: HeaderMap) -> (StatusCode, Json<serde_json::Value>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"ok": false, "reason": "missing/invalid token"})));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "gacha needs a database"})));
    };
    let w: Option<(i64, i64)> = sqlx::query_as("SELECT paid, bound FROM crystal_wallet WHERE account_id = $1")
        .bind(acct.account_id as i64).fetch_optional(db).await.ok().flatten();
    let (paid, bound) = w.unwrap_or((0, 0));
    let m: Option<(i64,)> = sqlx::query_as("SELECT tickets FROM gacha_mileage WHERE account_id = $1")
        .bind(acct.account_id as i64).fetch_optional(db).await.ok().flatten();
    let mileage = m.map(|(t,)| t).unwrap_or(0).max(0);
    // COUPONS ARE THE PULL CURRENCY, so the screen has to be able to say how many the player has.
    // Until this they were invisible: the balance reached no field of any response, so a newcomer
    // holding ten free pulls saw a 3,000-crystal price tag on a free action and nothing to contradict
    // it. The only place it ever surfaced was the 402 quote, which never fires when there are
    // enough — the one case where it mattered most.
    let c: Option<(i64,)> = sqlx::query_as(
        "SELECT qty FROM bound_items WHERE account_id = $1 AND kind = $2")
        .bind(acct.account_id as i64).bind(gacha::LIMITED_COUPON_KIND)
        .fetch_optional(db).await.ok().flatten();
    let coupons = c.map(|(q,)| q).unwrap_or(0).max(0);
    let cs: Option<(i64,)> = sqlx::query_as(
        "SELECT qty FROM bound_items WHERE account_id = $1 AND kind = $2")
        .bind(acct.account_id as i64).bind(gacha::STANDARD_COUPON_KIND)
        .fetch_optional(db).await.ok().flatten();
    let coupons_std = cs.map(|(q,)| q).unwrap_or(0).max(0);
    let prows: Vec<(String, i32, i32, bool)> = sqlx::query_as(
        "SELECT banner_type, count, pulls_since_sr, lost_5050 FROM gacha_pity WHERE account_id = $1",
    ).bind(acct.account_id as i64).fetch_all(db).await.unwrap_or_default();
    let pity: serde_json::Value = prows.into_iter().map(|(t, c, s, l)| {
        (t, serde_json::json!({"count": c, "pulls_since_sr": s, "lost_5050": l}))
    }).collect::<serde_json::Map<_, _>>().into();
    let nrow: Option<(i32, bool)> = sqlx::query_as("SELECT made, closed FROM gacha_newcomer WHERE account_id = $1")
        .bind(acct.account_id as i64).fetch_optional(db).await.ok().flatten();
    let (nc_made, nc_closed) = nrow.unwrap_or((0, false));
    let starter_left = gacha::newcomer_remaining(nc_made.max(0) as u32, nc_closed);
    let ms: Option<(i64,)> = sqlx::query_as("SELECT tickets FROM gacha_mileage_std WHERE account_id = $1")
        .bind(acct.account_id as i64).fetch_optional(db).await.ok().flatten();
    let mileage_std = ms.map(|(t,)| t).unwrap_or(0).max(0);
    let srow: Option<(i32, Option<String>)> = sqlx::query_as("SELECT made, selected FROM gacha_standard WHERE account_id = $1")
        .bind(acct.account_id as i64).fetch_optional(db).await.ok().flatten();
    let (std_made, std_selected) = srow.unwrap_or((0, None));
    let banners: Vec<serde_json::Value> = gacha::BANNERS.iter().map(|b| {
        let r = gacha::rates_for(b.kind);
        let remaining = if b.kind == gacha::BannerType::Newcomer {
            Some(gacha::newcomer_remaining(nc_made.max(0) as u32, nc_closed))
        } else {
            None
        };
        serde_json::json!({
            "id": b.id,
            "kind": format!("{:?}", b.kind),
            "featured": b.featured,
            "pool": b.ssr_off,
            // Newcomer only: pulls left before it closes (0 = closed; absent elsewhere).
            "remaining": remaining,
            // Standard opens once the Starter banner is spent.
            "open": b.kind != gacha::BannerType::Standard || starter_left == 0,
            "mileage_kind": if gacha::mileage_kind(b.kind) == gacha::MileageKind::Limited { "limited" } else { "standard" },
            // The ticket this banner spends and how many the account holds.
            "ticket_kind": if gacha::ticket_kind(b.kind) == gacha::STANDARD_COUPON_KIND { "standard" } else { "limited" },
            "tickets": if gacha::ticket_kind(b.kind) == gacha::STANDARD_COUPON_KIND { coupons_std } else { coupons },
            "multi_coupons": u64::from(gacha::BATCH_MULTI),
            // Odds must be disclosed on every banner (design doc guardrail). `soft`/`hard` are 0
            // under the live tables — there is no pity, and the UI must say so rather than
            // rendering "0/0" as if it were a counter.
            "base_bp": r.base_bp, "soft": r.soft, "hard": r.hard, "ramp_bp": r.ramp_bp,
            // SR odds (0.0.489): `sr_bp` is the roll after a missed SSR; `sr_rate_bp` the overall
            // share to print; R is the rest.
            "sr_bp": r.sr_bp,
            "sr_rate_bp": u64::from(10_000 - r.base_bp.min(10_000)) * u64::from(r.sr_bp) / 10_000,
            "fifty_fifty": r.fifty_fifty,
            "has_pity": r.hard > 0,
            "carry_guarantee": r.carry_guarantee,
        })
    }).collect();
    (StatusCode::OK, Json(serde_json::json!({
        "ok": true,
        "paid": paid.to_string(),
        // Legacy key until the Unity client reads `paid` (2026-10-01).
        "tradeable": paid.to_string(),
        "bound": bound.to_string(),
        "cost_per_pull": gacha::TOKEN_PER_PULL.to_string(),
        // The client prices a pull as `max(0, n - coupons) * coupon_price`, which is what the
        // server will actually charge — one number, derived the same way on both sides.
        // TWO TICKETS: `coupons` = Limited tickets (Limited/Weapon banners), `coupons_std` =
        // Standard tickets (Starter/Standard). Each banner row also carries its own `tickets`.
        "coupons": coupons.to_string(),
        "coupons_std": coupons_std.to_string(),
        "coupon_price": gacha::COUPON_PRICE_GEMS.to_string(),
        // MILEAGE — the parallel track. `pool` is what the tickets can buy, which deliberately
        // includes units no banner currently features.
        "mileage": mileage.to_string(),
        "mileage_cost": gacha::MILEAGE_COST.to_string(),
        "mileage_per_pull": gacha::MILEAGE_PER_PULL.to_string(),
        "exchange_pool": gacha::EXCHANGE_POOL,
        // STANDARD MILEAGE (Starter/Standard pulls): Memory Fragments only.
        "mileage_std": mileage_std.to_string(),
        "exchange_pool_std": gacha::EXCHANGE_POOL.iter().filter(|d| gacha::is_exchangeable_with(gacha::MileageKind::Standard, d)).collect::<Vec<_>>(),
        // The one-time Standard SSR selector.
        "standard_made": std_made,
        "standard_select_at": gacha::STANDARD_SELECT_AT,
        "standard_selected": std_selected,
        "standard_select_pool": gacha::STANDARD_POOL,
        "pity": pity,
        "banners": banners,
        // FARM BANNER: the events this account holds tickets for (`farm_banners_for`).
        "farm": farm_banners_for(db, &st, acct.account_id, now_ms().max(0) as u64).await,
    })))
}

#[derive(serde::Deserialize)]
struct DevCrystalReq { account_id: u64, #[serde(alias = "tradeable")] paid: i64, bound: i64 }

/// `GET /dev/storage` — READ-ONLY size report. Answers "how big are the mailbox and the saves"
/// without anyone copying a production connection string onto a laptop, which is the only way
/// to ask this today. Dev-gated like the rest of this group; it mutates nothing.
///
/// Two things it is for, both of them recurring rather than one-off:
///   * mailbox rows, split unclaimed vs claimed — claimed rows are retained for replay safety
///     and only ever grow, so this is the number that decides whether they need archiving.
///   * `saves` count/avg/max — the CCU ceiling at the top of TODO.md is save size x concurrency,
///     and the projection there (~295 KB at a 10-character endgame party) deserves measurement
///     rather than extrapolation.
/// GET /dev/balance (dev-gated) — the read path, without which the tables just accumulate and
/// nobody ever looks. Deliberately a handful of aggregates rather than a query console: the
/// point is that the four questions worth asking have an answer one curl away.
///
/// Everything here is a DISTRIBUTION, not an average. An average hides the shape that matters —
/// a median player wiping twice at band 7 is fine, 20% wiping thirty times is a wall, and the
/// mean of those two looks healthy.
async fn dev_balance(State(st): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "balance needs a database"})));
    };
    // THE DIFFICULTY CURVE: what level do people arrive at each band with? A jump between
    // consecutive zones is a wall.
    let arrival: Vec<(String, i64, i64, i64)> = sqlx::query_as(
        "SELECT zone, count(*), min(level)::bigint, max(level)::bigint          FROM first_clear GROUP BY zone ORDER BY zone",
    ).fetch_all(db).await.unwrap_or_default();
    // WHERE PEOPLE STOP: seen in a zone, never seen in a later one. The definition of a wall,
    // and not derivable from anything else.
    let stuck: Vec<(String, i64)> = sqlx::query_as(
        "SELECT zone, count(*) FROM player_daily WHERE day >= $1 GROUP BY zone ORDER BY count(*) DESC LIMIT 15",
    ).bind(today_num() - 7).fetch_all(db).await.unwrap_or_default();
    // WHICH COMPS GET FURTHEST.
    let comps: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT party, count(*), max(level)::bigint FROM first_clear          WHERE party <> '' GROUP BY party ORDER BY count(*) DESC LIMIT 15",
    ).fetch_all(db).await.unwrap_or_default();
    // DAU + the counters, last 7 days.
    let dau: Vec<(i32, i64)> = sqlx::query_as(
        "SELECT day, count(*) FROM player_daily WHERE day >= $1 GROUP BY day ORDER BY day",
    ).bind(today_num() - 7).fetch_all(db).await.unwrap_or_default();
    let counters: Vec<(i32, String, i64)> = sqlx::query_as(
        "SELECT day, key, value FROM metrics_daily WHERE day >= $1 ORDER BY day, key",
    ).bind(today_num() - 7).fetch_all(db).await.unwrap_or_default();
    let (pending_counters, pending_players) = st.metrics.held().pending();
    (StatusCode::OK, Json(serde_json::json!({
        "ok": true,
        "arrival_level_by_zone": arrival.iter()
            .map(|(z, n, lo, hi)| serde_json::json!({"zone": z, "players": n, "level_min": lo, "level_max": hi}))
            .collect::<Vec<_>>(),
        "players_by_zone_7d": stuck.iter()
            .map(|(z, n)| serde_json::json!({"zone": z, "players": n})).collect::<Vec<_>>(),
        "party_comps": comps.iter()
            .map(|(p, n, hi)| serde_json::json!({"party": p, "arrivals": n, "max_level": hi}))
            .collect::<Vec<_>>(),
        "dau_7d": dau.iter().map(|(d, n)| serde_json::json!({"day": d, "players": n})).collect::<Vec<_>>(),
        "counters_7d": counters.iter()
            .map(|(d, k, v)| serde_json::json!({"day": d, "key": k, "value": v})).collect::<Vec<_>>(),
        // Unflushed. Non-zero is normal (the flush is on a 60 s timer); permanently large means
        // the flush loop has died.
        "unflushed": {"counters": pending_counters, "players": pending_players},
    })))
}

async fn dev_storage(State(st): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "storage report needs a database"})));
    };
    // Mail is a TABLE now (`mailbox_store`), so count rows rather than parsing a blob.
    // `claimed_at IS NOT NULL` rows are kept for replay safety and only ever accumulate —
    // this is the number that says whether they need an archival policy.
    let mail: Option<(i64, i64)> = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE claimed_at IS NULL), count(*) FILTER (WHERE claimed_at IS NOT NULL) FROM mailbox",
    ).fetch_optional(db).await.ok().flatten();
    let (unclaimed, claimed) = mail.unwrap_or((0, 0));
    // The other set that only grows — settled claim op-ids, kept forever because an old trusted
    // anchor can re-present any of them on any save.
    let consumed: Option<(i64,)> = sqlx::query_as("SELECT count(*) FROM consumed_market_ops")
        .fetch_optional(db).await.ok().flatten();
    let saves: Option<(i64, Option<i64>, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT count(*), sum(octet_length(state_json))::bigint, \
                avg(octet_length(state_json))::bigint, max(octet_length(state_json))::bigint FROM saves",
    ).fetch_optional(db).await.ok().flatten();
    let (n, total, avg, max) = saves.unwrap_or((0, None, None, None));
    (StatusCode::OK, Json(serde_json::json!({
        "ok": true,
        "mailbox": {"unclaimed": unclaimed, "claimed": claimed},
        "consumed_market_ops": consumed.map(|(c,)| c).unwrap_or(0),
        "saves": {"accounts": n, "total_bytes": total, "avg_bytes": avg, "max_bytes": max},
    })))
}

/// `POST /dev/gacha-crystal` — credit crystal without a purchase flow. The real `/gacha/purchase`
/// waits on the Steam txn plumbing (`mtx_txns` is DDL-only today), and everything else is
/// testable without it.
async fn dev_gacha_crystal(State(st): State<AppState>, Json(req): Json<DevCrystalReq>) -> (StatusCode, Json<serde_json::Value>) {
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "gacha needs a database"})));
    };
    let r = sqlx::query("INSERT INTO crystal_wallet (account_id, paid, bound) VALUES ($1, GREATEST($2,0), GREATEST($3,0)) ON CONFLICT (account_id) DO UPDATE SET paid = GREATEST(crystal_wallet.paid + $2, 0), bound = GREATEST(crystal_wallet.bound + $3, 0)")
        .bind(req.account_id as i64).bind(req.paid).bind(req.bound)
        .execute(db).await;
    match r {
        Ok(_) => {
            let _ = sqlx::query("INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES ($1, $2, $3, 'dev-grant', '')")
                .bind(req.account_id as i64).bind(req.paid).bind(req.bound).execute(db).await;
            (StatusCode::OK, Json(serde_json::json!({"ok": true, "reason": "granted"})))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    }
}

/// Server wall-clock in epoch millis (verify-server may use wall-clock; sim-core may not).
fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Anti-fast-forward: is `final_tick`'s advance since the session `anchor` (tick, ms)
/// within what real elapsed server time allows? A legit client runs at ≤10 t/s and earns
/// nothing offline, so ticks-since-anchor can't outrun wall-time-since-anchor. Pure so it
/// unit-tests. A backward clock (now < anchor_ms) clamps elapsed to 0 (grace still applies).
fn within_tick_rate(anchor: (u64, i64), final_tick: u64, now: i64) -> bool {
    let (anchor_tick, anchor_ms) = anchor;
    let elapsed_ms = (now - anchor_ms).max(0) as u64;
    let allowed = elapsed_ms / MS_PER_TICK as u64 + FASTFWD_SLACK_TICKS;
    final_tick.saturating_sub(anchor_tick) <= allowed
}

/// The anti-fast-forward anchor timestamp for an account at BOOT: its own last save's time,
/// but never earlier than `floor` (= `now − RESTART_GRACE_MS`).
///
/// Using the save's own timestamp is what removes the profit from the restart grace. The budget
/// becomes the AGE of that account's last confirmed save — which is the SAME quantity as the
/// backlog the client is about to present, since a backlog is exactly the play since the last
/// confirmed save. So an honest client always fits, and a cheater who idles the machine to sleep
/// and wakes it can only claim wall time that genuinely elapsed since they last saved: no gain.
///
/// The floor is still needed in the other direction. Without it, an account whose last save is
/// hours old (logged off overnight, `min_machines_running = 0`) would be handed hours of budget
/// on a first push that skips the handshake — turning time spent logged OFF into instant
/// farming, which is the exploit the boot seeding exists to stop. `saved_at_ms == 0` is a row
/// written before the column existed and lands on the floor, same as an ancient save.
fn boot_anchor_ms(saved_at_ms: i64, floor: i64) -> i64 {
    saved_at_ms.max(floor)
}

/// LAYER 2 — how far this session's tick advance runs ahead of the real time it was given,
/// measured against `TOLERATED_RATE_BP` rather than exactly 1.0×.
///
/// Negative for any honest client: the sim is driven by real time so it cannot outrun it, and
/// load-after-handshake plus network latency put `final_tick` further behind the server clock.
/// Positive only for a consistent bias — which is the signal.
fn session_excess(anchor: (u64, i64), final_tick: u64, now: i64) -> i64 {
    let (anchor_tick, anchor_ms) = anchor;
    let elapsed_ms = (now - anchor_ms).max(0) as u64;
    let granted = elapsed_ms / MS_PER_TICK as u64 * TOLERATED_RATE_BP / 10_000;
    final_tick.saturating_sub(anchor_tick) as i64 - granted as i64
}

/// Fold this save's INCREMENT of session excess into a lifetime counter, clamped at the floor.
/// Returns the new counter. Split out from the handler so the arithmetic is unit-testable
/// without a request, a database, or a clock.
fn fold_excess(lifetime: i64, recorded: i64, excess: i64) -> i64 {
    (lifetime + (excess - recorded)).max(EXCESS_FLOOR)
}

/// A save was rejected for an integrity/cheat reason: TAINT the session (transient — blocks
/// `/refine/roll` until it recovers) AND record a DURABLE cheat FLAG for human review. Ban
/// is MANUAL only — this never bans; a spike of flags across many accounts means OUR bug
/// (a determinism/version drift), not a cheat wave, and auto-banning legit players on our
/// own divergence is the TBH mistake. Fire-and-forget on the DB write.
/// Days since the epoch. The only clock telemetry touches — `metrics.rs` itself takes the day
/// as a parameter so it stays testable.
/// Party class ids, sorted and comma-joined — the grouping key for "which comps get furthest".
/// Living members only, pets excluded; a pet is not a build choice.
fn party_comp(state: &sim_core::state::GameState) -> String {
    let mut v: Vec<&str> = state
        .grid
        .entities
        .iter()
        .filter(|e| matches!(e.team, sim_core::grid::Team::Player) && !e.is_pet)
        .map(|e| e.unit_def.as_str())
        .filter(|c| !c.is_empty())
        .collect();
    v.sort_unstable();
    v.dedup();
    v.join(",")
}

/// The shop's cap day: the game day (04:00 Thailand time, like every daily), not the UTC date
/// (user 2026-10-03 — the shop used to roll over at 07:00 while everything else did at 04:00).
fn shop_day() -> i32 {
    i32::try_from(daily::day_of_ms(now_ms() as u64)).unwrap_or(i32::MAX)
}

fn today_num() -> i32 {
    (now_ms() / 86_400_000) as i32
}

/// Persist whatever has accumulated. Called on a timer, never per save: the measured scaling
/// wall is the DB write rate, so the added cost has to track the flush interval and the number
/// of distinct keys, not concurrency.
async fn flush_metrics(st: &AppState) {
    let Some(db) = &st.db else { return };
    let batch = st.metrics.held().drain();
    if batch.is_empty() {
        return;
    }
    for (day, key, value) in &batch.counters {
        let _ = sqlx::query(
            "INSERT INTO metrics_daily (day, key, value) VALUES ($1, $2, $3) \
             ON CONFLICT (day, key) DO UPDATE SET value = metrics_daily.value + excluded.value",
        )
        .bind(day).bind(key).bind(value)
        .execute(db).await;
    }
    for (account_id, zone, f) in &batch.first_seen {
        let _ = sqlx::query(
            "INSERT INTO first_clear (account_id, zone, level, tick, party) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT (account_id, zone) DO NOTHING",
        )
        .bind(*account_id as i64).bind(zone).bind(f.level as i32)
        .bind(f.tick as i64).bind(&f.party)
        .execute(db).await;
    }
    for (day, account_id, p) in &batch.players {
        let _ = sqlx::query(
            "INSERT INTO player_daily (day, account_id, level, tick, gil, pending_drops, zone) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (day, account_id) DO UPDATE SET level = excluded.level, \
             tick = excluded.tick, gil = excluded.gil, \
             pending_drops = excluded.pending_drops, zone = excluded.zone, seen_at = now()",
        )
        .bind(day).bind(*account_id as i64).bind(p.level as i32)
        .bind(p.tick as i64).bind(p.gil as i64).bind(p.pending_drops as i32).bind(&p.zone)
        .execute(db).await;
    }
}

async fn flag_cheat(st: &AppState, account_id: u64, reason: &str, final_tick: u64) {
    flag_cheat_detail(st, account_id, reason, final_tick, "").await;
}

/// Where a divergence flag came from: the bundle's sim + catalog version and the platform the
/// client reports in `X-Client-Platform` (absent = "unknown"). Kept OUT of `reason`, which keys
/// the rejection metrics and must stay a small fixed set.
fn client_detail(headers: &HeaderMap, bundle: &SaveBundle) -> String {
    let platform = headers
        .get("x-client-platform")
        .and_then(|v| v.to_str().ok())
        .map(|p| p.chars().filter(|c| c.is_ascii_graphic()).take(32).collect::<String>())
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "unknown".into());
    format!("sim={} data={} platform={platform}", bundle.sim_version, bundle.game_data_version)
}

/// `flag_cheat` with a free-text `detail` stored beside the reason.
async fn flag_cheat_detail(st: &AppState, account_id: u64, reason: &str, final_tick: u64, detail: &str) {
    // Counted HERE rather than at each rejection site: every reject already funnels through
    // this function, so a new rejection reason is measured the day it is written and nobody has
    // to remember to add a counter. Keyed by reason, because a bare total cannot tell one broken
    // client apart from a version skew hitting everybody.
    st.metrics
        .held()
        .incr(today_num(), &metrics::keys::rejected_key(reason));
    st.sessions.tainted.held().insert(account_id);
    println!("CHEAT-FLAG account={account_id} reason={reason} final_tick={final_tick} {detail} (review, do NOT auto-ban)");
    if let Some(db) = &st.db {
        // At most one row per (account, reason) per hour (audit 2026-10-03): a cheap rejected
        // save (fast-forward, not-anchored) wrote a row each time — ~170k rows a day from one IP.
        // The taint and the metric above still count every one.
        let now = now_ms();
        let _ = sqlx::query(
            "INSERT INTO cheat_flags (account_id, reason, final_tick, at_ms, detail) \
             SELECT $1, $2, $3, $4, $5 WHERE NOT EXISTS (SELECT 1 FROM cheat_flags \
               WHERE account_id = $1 AND reason = $2 AND at_ms > $4 - 3600000)",
        )
        .bind(account_id as i64)
        .bind(reason)
        .bind(final_tick as i64)
        .bind(now)
        .bind(detail)
        .execute(db)
        .await;
    }
}

#[derive(serde::Serialize)]
struct FlagRow {
    account_id: u64,
    reason: String,
    final_tick: u64,
    at_ms: i64,
    detail: String,
}

/// GET /flags (dev-gated) — the cheat-suspicion audit for MANUAL review: the recent flags
/// plus a per-account count. A human reads this to decide bans; the server NEVER auto-bans
/// (a count spike shared across many accounts = OUR bug, not a cheat wave).
async fn list_flags(State(st): State<AppState>) -> Json<serde_json::Value> {
    let Some(db) = &st.db else {
        return Json(serde_json::json!({ "recent": [], "by_account": [], "note": "no db" }));
    };
    let rows: Vec<(i64, String, i64, i64, String)> = sqlx::query_as(
        "SELECT account_id, reason, final_tick, at_ms, detail FROM cheat_flags ORDER BY id DESC LIMIT 200",
    ).fetch_all(db).await.unwrap_or_default();
    let counts: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT account_id, COUNT(*) FROM cheat_flags GROUP BY account_id ORDER BY COUNT(*) DESC",
    ).fetch_all(db).await.unwrap_or_default();
    let recent: Vec<FlagRow> = rows.into_iter()
        .map(|(a, r, ft, at, d)| FlagRow { account_id: a as u64, reason: r, final_tick: ft as u64, at_ms: at, detail: d })
        .collect();
    let by_account: Vec<serde_json::Value> = counts.into_iter()
        .map(|(a, c)| serde_json::json!({ "account_id": a, "flags": c }))
        .collect();
    // LAYER 2 review list: accounts ranked by cumulative excess. Unlike a flag count this is a
    // MAGNITUDE — an honest account sits at EXCESS_FLOOR, so anything near or above zero is the
    // thing to look at, whether or not it has crossed the threshold yet.
    let excess: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT account_id, lifetime_excess FROM integrity ORDER BY lifetime_excess DESC LIMIT 50",
    ).fetch_all(db).await.unwrap_or_default();
    let by_excess: Vec<serde_json::Value> = excess.into_iter()
        .map(|(a, e)| serde_json::json!({ "account_id": a, "lifetime_excess": e }))
        .collect();
    Json(serde_json::json!({
        "recent": recent,
        "by_account": by_account,
        "by_excess": by_excess,
        "excess_threshold": EXCESS_THRESHOLD,
        "excess_floor": EXCESS_FLOOR,
    }))
}

/// Bound the replay cost of a bundle before running it (anti-DoS).
fn bundle_within_limits(bundle: &SaveBundle) -> Result<(), (StatusCode, String)> {
    // The checkpoint spacing is the CLIENT's number, and every checkpoint is a full state hash in
    // the replay (audit 2026-10-03): `snapshot_interval: 1` made one save cost ~400× an honest
    // one. Honest clients send `SNAPSHOT_INTERVAL_TICKS`; 0 (no chain) and coarser are cheaper.
    if bundle.snapshot_interval != 0 && bundle.snapshot_interval < sim_core::replay::SNAPSHOT_INTERVAL_TICKS {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("snapshot_interval {} below {}", bundle.snapshot_interval, sim_core::replay::SNAPSHOT_INTERVAL_TICKS),
        ));
    }
    // THE LOG'S TICKS ARE PART OF THE SPAN. `replay` advances to each entry's OWN tick before it
    // ever looks at `final_tick`:
    //
    //     for rec in log { if rec.tick > s.tick { advance(rec.tick - s.tick) } ; apply(rec) }
    //     if final_tick > s.tick { advance(...) }      // a no-op when final_tick is behind
    //
    // so a single entry dated 60_000 buys 60_000 ticks of simulation while `final_tick` says 0.
    // Every budget on the server read the wrong number: the span below came from `final_tick`
    // alone, and `within_tick_rate` / `session_excess` are both fed `bundle.final_tick`. An audit
    // took the SAME 20_000 ticks the server refuses when declared honestly, relabelled them into
    // the log with `final_tick: 0`, and got 200 — plus a server-rolled gear grant minted off the
    // free ticks, and `session_excess` going NEGATIVE, so layer 2 was not bypassed but CREDITED.
    // The cheating account then sorts as the most honest row in `/dev/balance`.
    //
    // Bounding the log's reach is what makes every existing budget mean what it says: after this,
    // the replay cannot advance past `final_tick`, which is the number they all measure. Honest
    // logs are unaffected — an input is recorded at the tick it happened, which is at or before
    // the end of the run, and `reanchor` drains from the front without rewriting ticks, so a
    // resumed bundle's entries still sit at or after `start.tick`.
    let mut prev = bundle.start.tick;
    for (i, rec) in bundle.log.iter().enumerate() {
        if rec.tick < bundle.start.tick || rec.tick > bundle.final_tick {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "log[{i}] is dated tick {} but the run is {}..={} — an input outside the run \
                     silently moves the replay past every budget the server checks",
                    rec.tick, bundle.start.tick, bundle.final_tick
                ),
            ));
        }
        if rec.tick < prev {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("log[{i}] goes backwards in time: tick {} after {prev}", rec.tick),
            ));
        }
        prev = rec.tick;
    }
    let span = bundle.final_tick.saturating_sub(bundle.start.tick);
    if span > MAX_REPLAY_SPAN {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("replay span {span} ticks > max {MAX_REPLAY_SPAN} (checkpoint/resume to reset)"),
        ));
    }
    if bundle.log.len() > MAX_LOG_LEN {
        return Err((StatusCode::BAD_REQUEST, format!("log too long: {} > {MAX_LOG_LEN}", bundle.log.len())));
    }
    Ok(())
}

/// Shared acceptance policy for any submitted bundle (used by /verify AND /save):
/// the verifier must run the SAME sim + catalog, and a production log must carry no
/// debug-only (`InputAuthority::Debug`) events. `Ok` = passed the policy gate (the
/// deterministic replay still has to succeed separately).
fn policy_check(data: &GameData, bundle: &SaveBundle) -> Result<(), (StatusCode, String)> {
    if bundle.sim_version != sim_core::VERSION {
        return Err((
            StatusCode::CONFLICT,
            format!("sim version skew: bundle {} != server {}", bundle.sim_version, sim_core::VERSION),
        ));
    }
    // Same version string, different commit (audit 2026-10-02): refused cleanly as skew instead of
    // replaying into a hash mismatch that flags the player. Compared only when both sides know
    // their commit.
    if !bundle.sim_build.is_empty() && bundle.sim_build != sim_core::SIM_BUILD {
        return Err((
            StatusCode::CONFLICT,
            format!("sim build skew: bundle {} != server {}", bundle.sim_build, sim_core::SIM_BUILD),
        ));
    }
    if bundle.game_data_version != data.version {
        return Err((
            StatusCode::CONFLICT,
            format!("game_data version skew: bundle {} != server {}", bundle.game_data_version, data.version),
        ));
    }
    if let Some(rec) = bundle.log.iter().find(|r| r.event.authority() == InputAuthority::Debug) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("log contains a forbidden debug event: {:?}", rec.event),
        ));
    }
    if let Some(rec) = bundle.log.iter().find(|r| !server_event_is_checked(&r.event)) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("log contains an unverifiable server-authored event: {:?}", rec.event),
        ));
    }
    Ok(())
}

/// Is this `InputAuthority::Server` event one the server can actually VERIFY?
///
/// `authority()` is a label, not a gate. The client writes its own input log and computes the
/// hash by replaying it, so it can author any `Server` event it likes and the bundle still
/// verifies — the authority tag never stopped it. What stops each variant is a SEPARATE
/// downstream check, and until 2026-08-10 nothing said which variants had one. `SetCardRateBonus`
/// did not: a client could author a x10 card event for itself, and because the odds ride on the
/// candidate's own `hit_ppm` (which `drop_grants_valid` deliberately does not re-derive) the
/// server would then issue the grants for it.
///
/// So the answer is enumerated per variant rather than assumed, and
/// `every_server_authored_event_has_a_verdict` fails when a new one is added without a decision.
/// Client-authority events are not this function's business — they are cost-gated by the sim and
/// covered by the replay.
fn server_event_is_checked(ev: &InputEvent) -> bool {
    if ev.authority() != InputAuthority::Server {
        return true;
    }
    match ev {
        // Nonce recomputed inside the replay loop (`verify_fuse_bindings`) against the six
        // products + attempt number; a mismatch, or a zero roll from Legendary up, rejects.
        InputEvent::FuseGear { .. } => true,
        // Keyed blake3 MAC recomputed by `drop_grants_valid` / `craft_grants_valid` /
        // `gear_grants_valid`; a forgery rejects the save and flags the account.
        InputEvent::GrantDrop { .. } | InputEvent::GrantCraft { .. } | InputEvent::GrantGear { .. } => true,
        // Keyed blake3 MAC recomputed by `commander_grants_valid`; a forged unlock rejects the save.
        InputEvent::GrantCommanderSkill { .. } => true,
        // Prune-only: mints nothing ONLY because `unsettled_prunes` (put_save) refuses a prune of
        // an op the server never settled — without that, pruning an unsettled claim erased the
        // only record of it and the mail could be claimed again (audit 2026-10-02).
        InputEvent::PruneMarketOps { .. } | InputEvent::PruneCandidates { .. } => true,
        // Reconciled against the real mail record by `mailbox::reconcile_claims` — payload, owner,
        // claimed-state, and (since 2026-08-10) duplicate mail-ids within one batch.
        InputEvent::ClaimMail { .. } => true,
        // NO server-side check exists. There is also no issuance path — the server never grants a
        // card-rate event today — so rejecting it costs nothing and closes the hole. When a real
        // double-card event ships it needs a nonce like every other server grant, and this arm is
        // where that requirement gets noticed.
        InputEvent::SetCardRateBonus { .. } => false,
        // A new Server-authority variant defaults to REJECTED, which is the safe direction: the
        // test above turns it into a red build so the decision is made deliberately.
        _ => false,
    }
}

#[derive(serde::Serialize)]
struct SaveResp {
    ok: bool,
    /// Crystal the Daily Commission ladder owes RIGHT NOW, so the HUD badge can light within one
    /// save interval instead of on a poll. Accrual happens inside this very request, so the
    /// moment the server learns a rung was reached is the moment it is already replying.
    #[serde(default)]
    daily_claimable_crystal: u32,
    reason: String,
    final_tick: u64,
    /// Server-roll DROP grants (server-roll-drops-design.md): every pending candidate
    /// in the verified state whose keyed nonce HIT — the client applies each as a
    /// `GrantDrop` event. Duds are never sent (zero-byte convention); the nonce is
    /// deterministic, so repeated saves re-issue the same hits idempotently (the
    /// client's candidate removal makes re-application a no-op). Skipped when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    grants: Vec<DropGrant>,
    /// Server-roll CRAFT grants — the gear output of every pending `Craft` candidate,
    /// rolled to 1-4★. A SEPARATE list from `grants` because the rules differ in both
    /// directions: these have no rate to pass (a craft was paid for, so all of them are
    /// issued), and their nonce is a different domain, so neither list's nonces can be
    /// replayed as the other's. The client applies each as a `GrantCraft` event.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    craft_grants: Vec<DropGrant>,
    /// Server-roll GEAR grants (stage 3) — the star, bonus lines and rune for every piece
    /// that dropped and is still waiting to be valued. Like `craft_grants`, all of them are
    /// issued: the drop already happened. Applied as `GrantGear`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    gear_grants: Vec<DropGrant>,
    /// Market op-ids this save SETTLED — the client prunes them from `market_ops` with a
    /// `PruneMarketOps` event. Only what was actually taken: a save can be accepted with nothing
    /// settled (tainted session), and the client must not prune on those.
    /// `candidate_seq`-style plain numbers: op ids come from a per-save counter and stay far
    /// below 2^53, unlike the keyed nonces that have to travel as strings.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    settled_ops: Vec<u64>,
    /// The highest DROP candidate seq this save adjudicated. Everything at or below it has had
    /// its nonce computed and either came back in `grants` or was a dud, so the client can drop
    /// the rest (`PruneCandidates`). Absent when there was nothing to judge.
    ///
    /// Plain number, not a string: candidate seqs come from a per-save counter and stay far
    /// below 2^53 — unlike the keyed nonces, which have to travel as decimal strings.
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved_seq: Option<u64>,
    /// DROPS delivered by this save (Farm Banner boxes): the mail rows it just minted, in the
    /// shape of `/gacha/roll`'s `mails`, so the client claims them straight into the bag. Each
    /// row appears in exactly one reply — a relisted row would be claimed twice. One that fails
    /// to claim (a full bag) waits in the mailbox.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    drops: Vec<mailbox::MailItem>,
}

#[derive(serde::Serialize)]
struct DropGrant {
    candidate_seq: u64,
    /// Decimal STRING of the u64 nonce (JSON numbers > 2^53 lose precision in the
    /// client's float parser — same convention as the refine nonce / account seed).
    server_roll: String,
}

/// How far the DROP judging got: the highest `Drop` seq in the verified state. Everything at or
/// below it has had its nonce computed and either came back in `grants` or was a dud, so the
/// client can drop the rest (`PruneCandidates`).
///
/// DROP seqs ONLY, never the max across all kinds. Craft and Gear tickets are not judged — they
/// always pay — so letting one of their seqs raise the watermark would authorise pruning DROP
/// tickets that sit below it and were never adjudicated.
///
/// `None` when there is nothing to judge, which the response omits and the client reads as
/// "do not prune".
///
/// A free function rather than three lines inside `put_save` for one reason: `put_save` needs a
/// database, auth and a whole bundle, so anything living inside it is untestable. That is
/// exactly how the rate-multiplier clamp ended up with a test that re-implemented the clamp.
fn resolved_drop_seq(state: &sim_core::state::GameState) -> Option<u64> {
    state
        .rare_candidates
        .iter()
        .filter(|c| matches!(c.kind, sim_core::state::CandidateKind::Drop))
        .map(|c| c.seq)
        .max()
}

/// STAGE 2 — does this candidate pay out? The odds live ON the candidate (`hit_ppm`, parts
/// per million) rather than in a constant here, because they depend on the monster that
/// dropped the ticket and this side only ever sees the ticket.
///
/// Until 0.0.256 this was a flat 1-in-1000 for everything and the per-item rarity lived in
/// STAGE 1, on the client — the inversion of the two-stage design that let the candidate list
/// peg at its cap. `sim_core::drops::stage2_hit_ppm` derives the replacement so every authored
/// final rate is preserved exactly.
/// x1 — the multiplier is off unless someone sets it.
const RARE_RATE_MULT_DEFAULT_BP: u32 = 10_000;
/// x50 ceiling. Generous enough for a low-CCU launch, far short of "everything is a jackpot".
const RARE_RATE_MULT_MAX_BP: u32 = 500_000;

fn candidate_hits(nonce: u64, hit_ppm: u32, mult_bp: u32) -> bool {
    // Both inputs are u32, so the product cannot overflow u64 and needs no clamping — and
    // since the left side is `% 1_000_000`, a scaled value at or above 1,000,000 already means
    // "always pays". Nothing here can wrap a generous multiplier into never-pays. Stated
    // rather than defended: a sabotage run showed the guards that used to sit here could not
    // be made to fail, which means they were decoration, not safety.
    let scaled = hit_ppm as u64 * mult_bp as u64 / 10_000;
    nonce % 1_000_000 < scaled
}

/// Read + clamp the live rate multiplier. Separate from `main` so it is actually testable —
/// the first version of this lived inline and its "test" re-implemented the clamp in the test
/// body, which passes whatever the real code does.
///
/// A fat-fingered value must not be able to turn a 1-in-200,000 card into a guaranteed one, and
/// 0 would silently switch every server-roll drop OFF — a worse accident than a generous one,
/// so the floor is x0.01 rather than x0. Unset or unparseable = x1, unchanged.
fn parse_rate_mult(raw: Option<&str>) -> u32 {
    raw.and_then(|v| v.parse::<u32>().ok())
        .map(|v| v.clamp(100, RARE_RATE_MULT_MAX_BP))
        .unwrap_or(RARE_RATE_MULT_DEFAULT_BP)
}

/// The server-authored rare-DROP nonce — blake3 keyed by the server secret, bound to
/// (account, candidate_seq). Deterministic → verification AND issuance are stateless +
/// idempotent (no outstanding ledger, unlike refine: a candidate's verdict never
/// changes, and seq never reuses). 63-bit masked, never 0 (the sim's no-roll sentinel).
fn drop_nonce(secret: &str, account_id: u64, candidate_seq: u64) -> u64 {
    keyed_nonce(b"drop-roll:v1", secret, account_id, candidate_seq)
}

/// The one construction behind every candidate nonce. `domain` is what keeps the kinds apart:
/// a nonce issued for one kind is not a valid nonce for another at the same seq, so it cannot
/// be replayed across kinds even against a forged history. ONE function rather than a copy per
/// kind — three near-identical hashers is how the domain tag eventually gets pasted wrong.
///
/// 63-bit masked and never 0 (the sim reads 0 as "no server roll").
fn keyed_nonce(domain: &[u8], secret: &str, account_id: u64, candidate_seq: u64) -> u64 {
    let mut h = blake3::Hasher::new();
    h.update(domain);
    h.update(secret.as_bytes());
    h.update(&account_id.to_le_bytes());
    h.update(&candidate_seq.to_le_bytes());
    let n = digest_u64(&h) & 0x7FFF_FFFF_FFFF_FFFF;
    if n == 0 { 1 } else { n }
}

/// The server-authored CRAFT nonce. Same construction as `drop_nonce` under a DIFFERENT
/// domain tag, which is the whole point: a nonce issued for a craft candidate is not a
/// valid drop nonce for the same seq and vice versa, so neither can be replayed as the
/// other even against a forged history. The sim's kind guard is the second lock.
///
/// No hit rule applies — a craft was paid for with materials and Gil, so every candidate
/// is granted. This nonce only decides WHICH weapon comes out (star + bonus lines).
fn gear_nonce(secret: &str, account_id: u64, candidate_seq: u64) -> u64 {
    keyed_nonce(b"gear-roll:v1", secret, account_id, candidate_seq)
}

fn craft_nonce(secret: &str, account_id: u64, candidate_seq: u64) -> u64 {
    keyed_nonce(b"craft-roll:v1", secret, account_id, candidate_seq)
}

/// The server-authored COMMANDER-SKILL grant nonce. Same construction as the roll nonces but keyed
/// by the skill id STRING rather than a numeric seq — a Commander skill is granted once per account,
/// so its identity is the skill id, not a per-drop candidate. Its own domain tag keeps it
/// un-replayable as any other kind, and the skill id is length-prefixed so `"a"+"bc"` and `"ab"+"c"`
/// cannot collide (the `refine_nonce` length-prefix lesson). 63-bit masked, never 0.
fn commander_nonce(secret: &str, account_id: u64, skill_id: &str) -> u64 {
    let mut h = blake3::Hasher::new();
    h.update(b"cmdr-skill:v1");
    h.update(secret.as_bytes());
    h.update(&account_id.to_le_bytes());
    h.update(&(skill_id.len() as u64).to_le_bytes());
    h.update(skill_id.as_bytes());
    let n = digest_u64(&h) & 0x7FFF_FFFF_FFFF_FFFF;
    if n == 0 { 1 } else { n }
}

/// Validate every `GrantDrop` in a log: the roll must be OUR nonce for that seq AND an
/// actual HIT. Returns the first offending seq. Pure (unit-testable) — deterministic
/// recompute, no ledger.
/// The nonce IS the authorisation. It is `blake3(secret, account, seq)`, so a client cannot
/// compute one; the server only ever issues nonces for candidates that HIT; therefore
/// presenting a valid nonce proves the grant was issued. The old extra `% den == 0` test was
/// belt-and-braces on top of that, and it cannot survive stage 2 moving onto the candidate —
/// the log carries a seq, not the ticket, so this side no longer knows the odds that applied.
/// Nothing is lost: forging still requires the secret.
fn drop_grants_valid(secret: &str, account_id: u64, log: &[sim_core::events::RecordedInput]) -> Result<(), u64> {
    for ri in log {
        if let InputEvent::GrantDrop { candidate_seq, server_roll } = &ri.event {
            if *server_roll != drop_nonce(secret, account_id, *candidate_seq) {
                return Err(*candidate_seq);
            }
        }
    }
    Ok(())
}

/// Validate every `GrantCraft` in a log: the roll must be OUR craft nonce for that seq.
/// No hit test — the rate rule belongs to drops, not to a purchase. Returns the first
/// offending seq.
fn craft_grants_valid(secret: &str, account_id: u64, log: &[sim_core::events::RecordedInput]) -> Result<(), u64> {
    for ri in log {
        if let InputEvent::GrantCraft { candidate_seq, server_roll } = &ri.event {
            if *server_roll != craft_nonce(secret, account_id, *candidate_seq) {
                return Err(*candidate_seq);
            }
        }
    }
    Ok(())
}

/// Validate every `GrantGear` in a log. Like craft: no hit test, the drop already happened —
/// this nonce decides the star, bonus lines and rune, not whether the item exists.
fn gear_grants_valid(secret: &str, account_id: u64, log: &[sim_core::events::RecordedInput]) -> Result<(), u64> {
    for ri in log {
        if let InputEvent::GrantGear { candidate_seq, server_roll } = &ri.event {
            if *server_roll != gear_nonce(secret, account_id, *candidate_seq) {
                return Err(*candidate_seq);
            }
        }
    }
    Ok(())
}

/// Validate every `GrantCommanderSkill` in a log: the `nonce` must be OUR commander nonce for that
/// (account, skill_id). Returns the first offending skill id. Pure/unit-testable — deterministic
/// recompute, no ledger. The nonce IS the authorisation: `commander_nonce(secret, account, skill)`,
/// which a client cannot compute; the server only issues it once the account has reached the level
/// that unlocks the skill and the reward is claimed. A valid nonce therefore proves the grant was
/// issued. The sim's insert is total (it never checks the nonce), so THIS is the only gate.
fn commander_grants_valid(
    secret: &str,
    account_id: u64,
    log: &[sim_core::events::RecordedInput],
) -> Result<(), String> {
    for ri in log {
        if let InputEvent::GrantCommanderSkill { skill_id, nonce } = &ri.event {
            // A 0 nonce is a benign no-op (the sim does not apply it either) — skip it rather than
            // flag a forgery, so a dropped field cannot brand a legitimate account a cheat.
            if *nonce != 0 && *nonce != commander_nonce(secret, account_id, skill_id) {
                return Err(skill_id.clone());
            }
        }
    }
    Ok(())
}

/// The one-save-per-account claim, released on every return path by `Drop`.
struct SaveSlot {
    set: Arc<Mutex<HashSet<u64>>>,
    account_id: u64,
}

impl SaveSlot {
    fn take(set: &Arc<Mutex<HashSet<u64>>>, account_id: u64) -> Option<Self> {
        set.held().insert(account_id).then(|| SaveSlot { set: set.clone(), account_id })
    }
}

impl Drop for SaveSlot {
    fn drop(&mut self) {
        self.set.held().remove(&self.account_id);
    }
}

/// PUT /save (auth) — the AUTHORITATIVE cloud save. The client pushes a bundle; the
/// server runs policy + replay-verify and stores the RESULTING final state ONLY if
/// it checks out. A forged/tampered bundle is rejected and NOT stored — so the
/// authoritative save can never hold an illegitimate state.
async fn put_save(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(bundle): Json<SaveBundle>,
) -> (StatusCode, Json<SaveResp>) {
    // Shared with the blocking replays below (`spawn_blocking` needs owned data).
    let bundle = Arc::new(bundle);
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: "missing/invalid token".into(), final_tick: 0, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
    };
    let Some(_slot) = SaveSlot::take(&st.saving, acct.account_id) else {
        return (StatusCode::TOO_MANY_REQUESTS, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: "a save is already in progress".into(), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
    };
    let d = &*st.data;
    if let Err((code, msg)) = policy_check(d, &bundle) {
        return (code, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: msg, final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
    }
    if let Err((code, msg)) = bundle_within_limits(&bundle) {
        return (code, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: msg, final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
    }
    // ANTI-FAST-FORWARD: a legit session's tick advance can't outrun real server time
    // (10 t/s, no offline earnings). The anchor is set at the session's GET /save; a
    // client that skips the handshake anchors from now (→ bounded to the flat grace).
    // Replay-verify proves determinism, NOT that a 14h log wasn't produced in 2s.
    // A BOOT-synthesized anchor serves only the backlog of a client that was connected across
    // the restart (audit 2026-10-03): it is honoured for `BOOT_ANCHOR_TTL_MS`, and the first save
    // accepted against it replaces it (below). Held open, it let a client that never re-handshook
    // cash every second since the boot as fast-forwarded play, unflagged.
    // With no live anchor, measure from the account's LATEST STORED tick at now (round-10 audit):
    // falling back to tick 0 refused every save of a client that slept through a restart past
    // the TTL, and the Unity client never recovers from that on its own. From the stored tick,
    // an honest few seconds of play pass and a fast-forward is still refused.
    //
    // That fallback is a REAL session anchor (round-11 audit): installed in `tick_ref` and counted
    // by layer 2 like a handshake. Left uncounted, a client could log in, skip GET /save, push 300
    // free ticks, and repeat — never flagged, never in `lifetime_excess`. Only a still-young BOOT
    // anchor is exempt from layer 2 and from the flag.
    let stored_tick = st.saves.held().get(&acct.account_id).map_or(0, |s| s.final_tick);
    let anchor = {
        let mut m = st.tick_ref.held();
        let live = m.get(&acct.account_id).copied()
            .filter(|a| a.from_handshake || now_ms() - a.ms <= BOOT_ANCHOR_TTL_MS);
        match live {
            Some(a) => a,
            None => {
                let a = TickAnchor { tick: stored_tick, ms: now_ms(), recorded: 0, from_handshake: true, synthesized: true };
                m.insert(acct.account_id, a);
                a
            }
        }
    };
    if !within_tick_rate((anchor.tick, anchor.ms), bundle.final_tick, now_ms()) {
        // Against an anchor synthesized at BOOT the backlog is the server's own downtime, played
        // through by a client that kept running (audit 2026-10-03: every connected player was
        // flagged after a long restart). Refused with "reload" — a fresh GET /save re-anchors —
        // but flagged only against a real handshake.
        if !anchor.from_handshake || anchor.synthesized {
            return (StatusCode::CONFLICT, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: "session clock reset by a server restart — reload".into(), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
        }
        flag_cheat(&st, acct.account_id, "fast-forward", bundle.final_tick).await;
        return (StatusCode::CONFLICT, Json(SaveResp {
            ok: false,
            daily_claimable_crystal: 0,
            reason: "tick advance exceeds real elapsed time (fast-forward) — rejected".into(),
            final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
    }
    // LAYER 2 — the same excess, accumulated ACROSS sessions. The check above resets with every
    // handshake, so on its own `auth → GET /save → POST /save (+SLACK)` is an unbounded loop;
    // this counter is the only thing that sees it. Runs BEFORE replay so the loop is cut off at
    // its cheapest point, and skipped for a boot-synthesized anchor (see `TickAnchor`).
    if anchor.from_handshake {
        let excess = session_excess((anchor.tick, anchor.ms), bundle.final_tick, now_ms());
        let total = {
            let mut m = st.lifetime_excess.held();
            let e = m.entry(acct.account_id).or_insert(0);
            *e = fold_excess(*e, anchor.recorded, excess);
            *e
        };
        // Only into the anchor this excess was measured from (audit 2026-10-03): a login landing
        // in between replaces it, and writing into the new one cancelled this save's excess out.
        if let Some(a) = st.tick_ref.held().get_mut(&acct.account_id).filter(|a| a.ms == anchor.ms && a.tick == anchor.tick) {
            a.recorded = excess;
        }
        if let Some(db) = &st.db {
            let _ = sqlx::query(
                "INSERT INTO integrity (account_id, lifetime_excess) VALUES ($1, $2) \
                 ON CONFLICT (account_id) DO UPDATE SET lifetime_excess = EXCLUDED.lifetime_excess",
            ).bind(acct.account_id as i64).bind(total).execute(db).await;
        }
        if total > EXCESS_THRESHOLD {
            // Reject as well as flag. The doc specified flag-only, but a flag just taints the
            // session (blocking refine + market settlement) and lets the save through, so a 20×
            // re-login loop would keep minting until a human bans it. Rejecting is not a ban —
            // the client auto-reverts to its last verified save exactly as it does for layer 1,
            // and the ban stays manual. Safe against false positives only BECAUSE of the rate
            // tolerance: an honest account sits pinned at EXCESS_FLOOR, never near this.
            flag_cheat(&st, acct.account_id, "lifetime-excess", bundle.final_tick).await;
            return (StatusCode::CONFLICT, Json(SaveResp {
                ok: false,
                daily_claimable_crystal: 0,
                reason: "cumulative tick advance exceeds real elapsed time — rejected".into(),
                final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
        }
    }
    // ANCHOR: the bundle's `start` must be a TRUSTED base for this account — the
    // genesis we reproduce from the account's server seed, or an end-state we
    // confirmed before. Without this, a client could send start = a forged god
    // character + empty log and pass verify trivially.
    let mut start_hash = hash::state_hash(&bundle.start);
    let orig_start_hash = start_hash.clone();
    // Compute the verdict INSIDE the lock, drop the guard, then flag/return — a std Mutex
    // guard may not be held across the async flag_cheat().await (non-Send).
    // ONLY THE LATEST VERIFIED SAVE is a base once the account has one (audit 2026-10-02). Any
    // earlier confirmed end used to be accepted, so a client could rewind to it and re-earn what
    // happened after: a Farm Box sent to Steam opened again, a failed fusion undone, a settled
    // mail op id reused. Genesis is a base only for an account that has never saved.
    let (latest, latest_tick) = st.saves.held().get(&acct.account_id)
        .map_or((None, 0), |s| (Some(s.final_hash.clone()), s.final_tick));
    let (anchored, earlier_anchor) = {
        let mut trusted = st.trusted.held();
        let set = trusted
            .entry(acct.account_id)
            .or_insert_with(|| genesis_candidates(d, acct.seed));
        match &latest {
            Some(h) => (*h == start_hash, set.contains(&start_hash)),
            None => {
                if set.contains(&start_hash) {
                    (true, false)
                } else {
                    // The set may predate a code change (an account whose DB-loaded anchors are
                    // just the old bare genesis) — recompute the current candidates and retry
                    // before rejecting. Still rejects a genuinely forged (non-seed) start.
                    set.extend(genesis_candidates(d, acct.seed));
                    (set.contains(&start_hash), false)
                }
            }
        }
    };
    // Belt-and-suspenders (should already be handled by the GET /save migrate-on-resume): a
    // start that is the legit CATALOG-MIGRATION of the stored save is NOT a forgery — the
    // client migrates held items to the current catalog on load, which shifts the hash. Accept
    // + trust it. This is what makes a resume survive a balance patch; the alternative is the
    // TBH mistake of flagging every returning player after a catalog change.
    let migration_case = !anchored && {
        let stored_json = st.saves.held().get(&acct.account_id).map(|s| s.state_json.clone());
        stored_json
            .and_then(|j| serde_json::from_str::<sim_core::state::GameState>(&j).ok())
            .map(|mut orig| { data::migrate_save_items(&mut orig, d); hash::state_hash(&orig) == start_hash })
            .unwrap_or(false)
    };
    // An EARLIER confirmed base: honest only as a RE-SEND of the bundle that produced the latest
    // save (its response was lost, or the client re-sent while it was in flight). That replays to
    // exactly the stored state → answer ok and change nothing. Anything else from an old base is a
    // rewind (or a desync): refused, not flagged — refusing it is the whole defence.
    // (The migration case is checked FIRST: GET /save trusts the migrated start, so it is also an
    // "earlier anchor" — and a returning player after a catalog change must not read as a rewind.)
    // A CONTINUATION from that base (audit 2026-10-03): the first save landed but its reply was
    // lost, so the client kept its old base and played on. If the replay passes through the latest
    // save's exact state at its tick, it extends that save rather than rewinding it — accepted as if
    // it started there. Without this the client was refused on every retry until a restart.
    // REPLAY BUDGET: every bundle that reaches a replay is charged its span first.
    let span = bundle.final_tick.saturating_sub(bundle.start.tick);
    if !charge_replay_budget(&st, acct.account_id, span, now_ms()) {
        return (StatusCode::TOO_MANY_REQUESTS, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: "too much replay in a short time — retry later".into(), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
    }
    let mut continuation = false;
    // The verified end of a continuation, kept so it is not replayed a second time below.
    let mut replayed: Option<sim_core::state::GameState> = None;
    if !anchored && !migration_case && earlier_anchor {
        let via = match latest.clone() {
            Some(h) => {
                let (b, data) = (bundle.clone(), st.data.clone());
                match run_replay(&st, move || b.verified_state_via(&data, latest_tick, &h)).await {
                    Some(r) => r,
                    // The server could not replay: "busy, retry", never "reload" (round-8 audit).
                    None => return refunded(&st, acct.account_id, span, (StatusCode::SERVICE_UNAVAILABLE, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: "server busy — retry".into(), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }))),
                }
            }
            None => None,
        };
        if via.as_ref().is_some_and(|(f, _)| latest.as_deref() == Some(hash::state_hash(f).as_str())) {
            println!("resend acct={} final_tick={} — already the latest save, ok (idempotent)", acct.account_id, bundle.final_tick);
            // NOT refunded (round-9 audit): a re-send costs a full replay, and refunding it let
            // one account replay the same bundle in a loop. An honest re-send fits the budget.
            return (StatusCode::OK, Json(SaveResp { ok: true, daily_claimable_crystal: 0, reason: "idempotent".into(), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
        }
        if let Some((f, passed)) = via {
            continuation = passed && bundle.final_tick > latest_tick;
            if continuation {
                replayed = Some(f);
            }
        }
    }
    if continuation {
        println!("continuation acct={} start_tick={} via latest tick {latest_tick} — accepted as extending it", acct.account_id, bundle.start.tick);
        // The base this save is trusted FROM is the latest save, not the older start it carried.
        if let Some(h) = &latest {
            start_hash = h.clone();
        }
    } else if !anchored && !migration_case && earlier_anchor {
        println!("STALE-ANCHOR acct={} start_tick={} — an earlier save, not the latest; rejected", acct.account_id, bundle.start.tick);
        return (StatusCode::CONFLICT, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: "start is an earlier save, not the latest — reload".into(), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
    }
    if migration_case {
        println!("anchor-via-migration acct={} tick={} (start == migrate(stored) — catalog change, trusted)", acct.account_id, bundle.start.tick);
        st.trusted.held().entry(acct.account_id).or_default().insert(start_hash.clone());
    } else if !anchored && !continuation {
        // Genuinely unknown base — not genesis, not a confirmed end, not a migration of the
        // stored save. This IS the forgery signal (the "correct warning" for the real case).
        println!("NOT-ANCHORED(forgery) acct={} start_tick={} start_hash={} — unknown base, rejected", acct.account_id, bundle.start.tick, start_hash);
        flag_cheat_detail(&st, acct.account_id, "not-anchored", bundle.final_tick, &client_detail(&headers, &bundle)).await;
        return (StatusCode::CONFLICT, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: "start snapshot is not anchored (forged/unknown base) — rejected".into(), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
    }
    // Verify AND extract the authoritative final state in one replay.
    // REPLAYS RUN OFF THE ASYNC WORKERS (audit 2026-10-03): a long replay polled inline pinned a
    // runtime thread for its whole length, and the request timeout cannot interrupt a poll that
    // never yields — a few heavy saves stalled every request, health checks included.
    let final_state = match replayed {
        Some(f) => Some(f),
        None => {
            let (b, data) = (bundle.clone(), st.data.clone());
            match run_replay(&st, move || b.verified_state(&data)).await {
                Some(r) => r,
                // The replay never ran to a verdict (server side): not the player's fault, no flag.
                None => return refunded(&st, acct.account_id, span, (StatusCode::SERVICE_UNAVAILABLE, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: "server busy — retry".into(), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }))),
            }
        }
    };
    let Some(final_state) = final_state else {
        flag_cheat_detail(&st, acct.account_id, "hash-mismatch", bundle.final_tick, &client_detail(&headers, &bundle)).await;
        return (StatusCode::OK, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: "replay hash mismatch — not stored".into(), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
    };
    // Server-roll DROPS: every GrantDrop in the log must carry OUR nonce for its seq
    // AND that nonce must be a genuine HIT — a forged/dud-forced grant rejects the save.
    if let Err(bad_seq) = drop_grants_valid(&st.roll_secret, acct.account_id, &bundle.log) {
        flag_cheat(&st, acct.account_id, "forged-drop-grant", bundle.final_tick).await;
        return (StatusCode::OK, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: format!("forged drop grant (candidate {bad_seq}) — rejected"), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
    }
    // Server-roll CRAFTS: same forgery check under the craft nonce domain. No hit rule —
    // the star + bonus lines are what the nonce decides, not whether the item exists.
    if let Err(bad_seq) = craft_grants_valid(&st.roll_secret, acct.account_id, &bundle.log) {
        flag_cheat(&st, acct.account_id, "forged-craft-grant", bundle.final_tick).await;
        return (StatusCode::OK, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: format!("forged craft grant (candidate {bad_seq}) — rejected"), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
    }
    // Same for stage-3 GEAR grants.
    if let Err(bad_seq) = gear_grants_valid(&st.roll_secret, acct.account_id, &bundle.log) {
        flag_cheat(&st, acct.account_id, "forged-gear-grant", bundle.final_tick).await;
        return (StatusCode::OK, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: format!("forged gear grant (candidate {bad_seq}) — rejected"), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
    }
    // Server-authored COMMANDER-SKILL unlocks: the nonce must be OUR commander nonce for that skill,
    // or a forged unlock (a client granting itself a skill it never earned) rejects the save.
    if let Err(bad_skill) = commander_grants_valid(&st.roll_secret, acct.account_id, &bundle.log) {
        flag_cheat(&st, acct.account_id, "forged-commander-grant", bundle.final_tick).await;
        return (StatusCode::OK, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: format!("forged commander grant ({bad_skill}) — rejected"), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
    }
    // Server-roll FUSION (design §7.8): the full-replay bind-check (target-selection / a high
    // fusion that skipped the server) then the nonce-set reconcile (forgery / consume-all). A
    // failure rejects the save.
    let bind = {
        let (b, data, secret, id) = (bundle.clone(), st.data.clone(), st.roll_secret.clone(), acct.account_id);
        match run_replay(&st, move || verify_fuse_bindings(&secret, id, &b.start, &data, &b.log)).await {
            Some(r) => r,
            None => return refunded(&st, acct.account_id, span, (StatusCode::SERVICE_UNAVAILABLE, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: "server busy — retry".into(), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }))),
        }
    };
    if let Err(reason) = bind {
        flag_cheat(&st, acct.account_id, "fuse-bind-fail", bundle.final_tick).await;
        return (StatusCode::OK, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: format!("fusion bind-check failed — {reason}"), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
    }
    // Reconciled on COPIES (audit 2026-10-03): moving rolls to "consumed" before the save was
    // accepted let a refused save (stale claim, CAS loss, persistence error) still retire its roll,
    // which reopened the peek-and-skip the fuse guards close. Applied after the commit below.
    let recon = {
        let used = fuse_nonces_used(&bundle.log);
        let mut outstanding = st.fuse_rolls.held().get(&acct.account_id).cloned().unwrap_or_default();
        let mut consumed_set = st.fuse_consumed.held().get(&acct.account_id).cloned().unwrap_or_default();
        reconcile_fuse(&mut outstanding, &mut consumed_set, &used)
    };
    if !recon.forged.is_empty() {
        flag_cheat(&st, acct.account_id, "forged-fuse-nonce", bundle.final_tick).await;
        return (StatusCode::OK, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: format!("forged fusion nonce(s) {:?} — rejected", recon.forged), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
    }
    // MAIL CLAIMS — the only market ops a save still settles (the Gil market was removed in
    // 0.0.490). Runs ONLY here, post-verify. A forged CLAIM (real mail, lied payload) passes
    // replay-verify but is caught here → reject + flag.
    // TAINT GATE (audit F5, defense-in-depth): an account flagged for a PRIOR integrity-rejected
    // save consumes nothing on the save that RECOVERS it — the ops stay unconsumed (in the
    // hash-covered log) and settle next save, after this save clears the taint below.
    let was_tainted = st.sessions.tainted.held().contains(&acct.account_id);
    // Fetch JUST the mails this save's Claim ops name; the pure `settle_claims` verifies against
    // that snapshot.
    let mail_snapshot = if let Some(db) = &st.db {
        // Claims already inside the verified base this bundle starts from were authored before the
        // refund reached the server (audit 2026-10-03); only a claim NEW in this bundle is held to
        // the withdrawn-mail grace window.
        let in_base: std::collections::BTreeSet<u64> = bundle.start.market_ops.iter()
            .filter_map(|op| match op {
                sim_core::state::MarketOp::Claim { mail_id, .. } => Some(*mail_id),
                _ => None,
            })
            .collect();
        match mailbox_store::snapshot_for_claims(db, acct.account_id, &final_state.market_ops).await {
            Ok(m) => m.forgive_withdrawn(&in_base),
            Err(e) => {
                eprintln!("[MAIL] mail snapshot failed for acct {} — save rejected: {e}", acct.account_id);
                return refunded(&st, acct.account_id, span, (StatusCode::OK, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: "server persistence error — not stored, retry".into(), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() })));
            }
        }
    } else {
        mailbox::Mailbox::default()
    };
    // A tainted save settles nothing, but it is still held to the withdrawn-mail window: it is
    // stored, and its claims then count as "already in a stored save" (audit 2026-10-03).
    if was_tainted {
        let late = mail_snapshot.withdrawn_claims(&final_state.market_ops);
        if !late.is_empty() {
            println!("stale-claim acct={} mails={late:?} (withdrawn by a refund) on a tainted save — rejected, not flagged", acct.account_id);
            return (StatusCode::OK, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: format!("mail {late:?} was already claimed or withdrawn — rejected"), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
        }
    }
    // Same shape as the mail snapshot: look up only the op-ids THIS save presents.
    let cset = if was_tainted {
        HashSet::new()
    } else if let Some(db) = &st.db {
        match already_consumed(db, acct.account_id, &final_state.market_ops).await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[MAIL] consumed-op lookup failed for acct {} — save rejected: {e}", acct.account_id);
                return refunded(&st, acct.account_id, span, (StatusCode::OK, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: "server persistence error — not stored, retry".into(), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() })));
            }
        }
    } else {
        HashSet::new()
    };
    // A settled op id re-presented with different content: a rewind reusing the id for a forged
    // claim (audit 2026-10-02). The plain `cset` skip below would wave it through unverified.
    if let Some(db) = &st.db {
        match unsettled_prunes(db, acct.account_id, &bundle.log).await {
            Ok(ids) if !ids.is_empty() => {
                flag_cheat(&st, acct.account_id, "unsettled-prune", bundle.final_tick).await;
                return (StatusCode::OK, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: format!("pruned op(s) {ids:?} were never settled — rejected"), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("[MAIL] prune lookup failed for acct {} — save rejected: {e}", acct.account_id);
                return refunded(&st, acct.account_id, span, (StatusCode::OK, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: "server persistence error — not stored, retry".into(), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() })));
            }
        }
    }
    if !was_tainted {
        if let Some(db) = &st.db {
            match reused_op_ids(db, acct.account_id, &final_state.market_ops).await {
                Ok(ids) if !ids.is_empty() => {
                    flag_cheat(&st, acct.account_id, "reused-op-id", bundle.final_tick).await;
                    return (StatusCode::OK, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: format!("settled op re-presented with different content on {ids:?} — rejected"), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("[MAIL] reused-op lookup failed for acct {} — save rejected: {e}", acct.account_id);
                    return refunded(&st, acct.account_id, span, (StatusCode::OK, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: "server persistence error — not stored, retry".into(), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() })));
                }
            }
        }
    }
    let claims = if was_tainted {
        mailbox::ClaimSettlement::default()
    } else {
        match mailbox::settle_claims(&mail_snapshot, &cset, acct.account_id, &final_state.market_ops) {
            Ok(c) => c,
            Err(forged) => {
                let ids: Vec<u64> = forged.iter().map(|(m, _)| *m).collect();
                if mailbox::only_stale_claims(&forged) {
                    // A mail this account already claimed, presented again — a stale inbox, not
                    // a forgery. Refuse (it would pay twice), do not flag; the client reverts to
                    // its last verified save.
                    println!("stale-claim acct={} mails={ids:?} (already claimed, or withdrawn by a refund) — rejected, not flagged", acct.account_id);
                    return (StatusCode::OK, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: format!("mail {ids:?} was already claimed or withdrawn — rejected"), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
                }
                flag_cheat(&st, acct.account_id, "forged-claim", bundle.final_tick).await;
                return (StatusCode::OK, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: format!("forged market claim(s) on mail {ids:?} — rejected"), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
            }
        }
    };
    if !claims.retired.is_empty() {
        // Only an old state can carry these: the trade inputs are tombstones since 0.0.490.
        eprintln!(
            "[MAIL] acct {} presented {} op(s) of the removed Gil market {:?} — marked consumed, NOT settled",
            acct.account_id,
            claims.retired.len(),
            claims.retired
        );
    }
    let state_json = serde_json::to_string(&final_state).unwrap_or_default();
    // THE REPLAY'S HASH, not the client's claim. These three sites used `bundle.final_hash` — the
    // number the client sent — and the only thing binding it to reality was the comparison inside
    // `verified_state`. Delete that comparison and the claimed hash becomes the next session's
    // TRUSTED ANCHOR: push an honest log with `final_hash = state_hash(cooked)`, and push two
    // arrives with `start = cooked` already anchored. An audit deleted the verdict and the whole
    // suite stayed green, because the only well-formed POST /save in it uses a bundle whose hash
    // is correct by construction.
    //
    // Hashing `final_state` here means the stored row, the anchor and the durable write cannot
    // disagree with the replay even if the verdict above is ever weakened — the field stops being
    // a claim and becomes a consequence.
    let verified_hash = hash::state_hash(&final_state);
    // A continuation also keeps the OLD base its bundle carried (audit 2026-10-03): if this reply
    // is lost too, the client re-sends the same bundle, which must read as a re-send, not a forgery.
    let refill_digests = final_state.market_ops.iter()
        .filter(|op| cset.contains(&op.id()))
        .map(|op| (op.id(), mailbox::op_digest(op)))
        .collect();
    let stored_at = now_ms();
    let extras = AcceptedExtras { fuse_consumed: recon.consumed.clone(), now_ms: stored_at, refill_digests };
    let keep_bases: Vec<&str> = if continuation { vec![start_hash.as_str(), orig_start_hash.as_str()] } else { vec![start_hash.as_str()] };
    let stale = || (StatusCode::CONFLICT, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: "another save landed first — reload".into(), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() }));
    // ATOMIC durability (audit HIGH-1/HIGH-2): the save row + trusted anchor + the claimed mails +
    // the consumed-op marks commit in ONE transaction — a mail can never be claimed without the
    // save that spent it, or the reverse. On failure REJECT: nothing was consumed durably, and
    // the client's retry re-presents the same ops idempotently. Memory moves only AFTER this
    // commits (audit 2026-10-02: a failed write used to stay "latest" in memory, so the retry
    // was answered "idempotent" and the save was never stored).
    if let Some(db) = &st.db {
        match persist_save_and_claims(db, acct.account_id, &state_json, bundle.final_tick, &verified_hash, &claims, latest.as_deref(), &keep_bases, &extras).await {
            Ok(true) => {}
            Ok(false) => {
                // The row moved. If it moved to THIS save (an earlier attempt committed and was then
                // cut off before memory caught up — a request timeout), memory is resynced and the
                // retry is answered ok; otherwise memory is resynced to the row and this is stale
                // (audit 2026-10-03: memory kept the old hash and every retry and reload looped).
                let row: Option<(String, i64, String, i64)> = sqlx::query_as(
                    "SELECT state_json, final_tick, final_hash, saved_at_ms FROM saves WHERE account_id = $1",
                ).bind(acct.account_id as i64).fetch_optional(db).await.ok().flatten();
                if let Some((sj, ft, fh, at)) = row {
                    let same = fh == verified_hash;
                    st.saves.held().insert(acct.account_id, StoredSave { state_json: sj, final_tick: ft as u64, final_hash: fh.clone(), saved_at_ms: at });
                    {
                        let mut trusted = st.trusted.held();
                        trusted.entry(acct.account_id).or_default().insert(fh);
                    }
                    if same {
                        // This save DID land (an earlier attempt committed and was cut off): its
                        // durable halves committed with it; catch memory up (taint, fusion rolls).
                        st.sessions.tainted.held().remove(&acct.account_id);
                        if !recon.consumed.is_empty() {
                            if let Some(out) = st.fuse_rolls.held().get_mut(&acct.account_id) {
                                out.retain(|r| !recon.consumed.contains(&r.nonce));
                            }
                            st.fuse_consumed.held().entry(acct.account_id).or_default().extend(recon.consumed.iter().copied());
                        }
                        return refunded(&st, acct.account_id, span, (StatusCode::OK, Json(SaveResp { ok: true, daily_claimable_crystal: 0, reason: "idempotent".into(), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() })));
                    }
                }
                println!("CONCURRENT-SAVE acct={} — the latest save moved during this one; rejected", acct.account_id);
                { refund_replay_budget(&st, acct.account_id, span); return stale(); }
            }
            Err(e) => {
                eprintln!("[DURABILITY] atomic persist failed for acct {} — rejecting: {e}", acct.account_id);
                return refunded(&st, acct.account_id, span, (StatusCode::OK, Json(SaveResp { ok: false, daily_claimable_crystal: 0, reason: "server persistence error — not stored, retry".into(), final_tick: bundle.final_tick, grants: Vec::new(), craft_grants: Vec::new(), gear_grants: Vec::new(), resolved_seq: None, settled_ops: Vec::new(), drops: Vec::new() })));
            }
        }
    }
    // The previous accepted save, swapped out under ONE guard that also re-checks it is still the
    // one this bundle was checked against (the in-memory twin of the DB compare-and-set).
    let prev = {
        let mut saves = st.saves.held();
        if saves.get(&acct.account_id).map(|s| s.final_hash.clone()) != latest {
            None
        } else {
            Some(saves.insert(
                acct.account_id,
                StoredSave { state_json: state_json.clone(), final_tick: bundle.final_tick, final_hash: verified_hash.clone(), saved_at_ms: stored_at },
            ))
        }
    };
    if prev.is_none() {
        { refund_replay_budget(&st, acct.account_id, span); return stale(); }
    }
    // ACCEPTED. Memory follows the commit before any `.await` (a request cut off now must not
    // leave the taint set or the fusion rolls outstanding — audit 2026-10-03); their durable
    // halves and the offline gap committed with the save.
    st.sessions.tainted.held().remove(&acct.account_id);
    if !recon.consumed.is_empty() {
        {
            let mut rolls = st.fuse_rolls.held();
            if let Some(out) = rolls.get_mut(&acct.account_id) {
                out.retain(|r| !recon.consumed.contains(&r.nonce));
            }
        }
        st.fuse_consumed.held().entry(acct.account_id).or_default().extend(recon.consumed.iter().copied());
    }
    // A save accepted against a boot or synthesized anchor re-anchors the session HERE, as a
    // handshake would — so the unflagged-refusal exemption ends once the backlog is saved.
    if !anchor.from_handshake || anchor.synthesized {
        st.tick_ref.held().insert(acct.account_id, TickAnchor {
            tick: bundle.final_tick, ms: stored_at, recorded: 0, from_handshake: true,
            synthesized: false,
        });
    }
    // The confirmed end becomes the trusted base for the next session; only it and the base it
    // came from stay trusted (a re-send of this bundle is still recognised).
    {
        let mut trusted = st.trusted.held();
        let set = trusted.entry(acct.account_id).or_default();
        set.retain(|h| *h == start_hash || (continuation && *h == orig_start_hash));
        set.insert(verified_hash.clone());
    }
    let consumed_now = claims.consumed_ops;

    // BATTLE PASS accrual (P1): turn this VERIFIED save's tick/counter deltas into XP.
    // Own transaction, self-healing (bp_state snapshot is the anchor) — a failure only
    // defers the delta to the next accepted save, so it must never reject the save.
    // The offline gap is read from `offline_owed` and zeroed inside the accrual's own commit.
    let (daily_owed, drops) = match accrue_after_save(&st, acct.account_id, &final_state).await {
        Ok(g) => g,
        Err(e) => {
            eprintln!("[accrue] failed for acct {} (deferred to next save): {e}", acct.account_id);
            // The save still stands; the badge just stays dark until the next one succeeds.
            (0, Vec::new())
        }
    };
    // Server-roll DROPS: resolve every PENDING candidate in the verified state — HITS
    // return as grants; duds return nothing (they expire client-side by TTL). The nonce
    // is deterministic, so a candidate the client hasn't consumed yet simply re-issues
    // the same grant next save (idempotent — the sim's seq-consume makes replays no-op).
    let grants: Vec<DropGrant> = final_state
        .rare_candidates
        .iter()
        .filter(|c| matches!(c.kind, sim_core::state::CandidateKind::Drop))
        .filter_map(|c| {
            let n = drop_nonce(&st.roll_secret, acct.account_id, c.seq);
            candidate_hits(n, c.hit_ppm, st.rare_rate_mult_bp).then(|| DropGrant {
                candidate_seq: c.seq,
                server_roll: n.to_string(),
            })
        })
        .collect();
    // Server-roll CRAFTS: EVERY pending craft candidate is issued — no rate filter. The
    // player already paid for it, and a craft candidate the client hasn't consumed simply
    // re-issues the same deterministic nonce next save (the sim's seq-consume makes the
    // replay a no-op), so an interrupted save can never swallow a crafted weapon.
    let craft_grants: Vec<DropGrant> = final_state
        .rare_candidates
        .iter()
        .filter(|c| matches!(c.kind, sim_core::state::CandidateKind::Craft))
        .map(|c| DropGrant {
            candidate_seq: c.seq,
            server_roll: craft_nonce(&st.roll_secret, acct.account_id, c.seq).to_string(),
        })
        .collect();
    let gear_grants: Vec<DropGrant> = final_state
        .rare_candidates
        .iter()
        .filter(|c| matches!(c.kind, sim_core::state::CandidateKind::Gear { .. }))
        .map(|c| DropGrant {
            candidate_seq: c.seq,
            server_roll: gear_nonce(&st.roll_secret, acct.account_id, c.seq).to_string(),
        })
        .collect();
    if !grants.is_empty() {
        println!("drop-grants acct={} n={} (of {} pending)", acct.account_id, grants.len(), final_state.rare_candidates.len());
    }
    if !craft_grants.is_empty() {
        println!("craft-grants acct={} n={}", acct.account_id, craft_grants.len());
    }
    if !gear_grants.is_empty() {
        println!("gear-grants acct={} n={}", acct.account_id, gear_grants.len());
    }
    // Every DROP ticket in the verified state just had its nonce computed above — hits came
    // back in `grants`, the rest are duds. Tell the client how far that goes so it can stop
    // carrying them; otherwise only the tick-based TTL ever clears a dud, and ticks do not
    // advance while the game is closed.
    let resolved_seq = resolved_drop_seq(&final_state);
    // TELEMETRY. Read off the state the SERVER replayed, so none of it depends on trusting the
    // client. `grant.*` at a flat zero is the alarm that would have caught the three months the
    // whole server-roll pipeline was dead.
    {
        let day = today_num();
        let mut m = st.metrics.held();
        m.incr(day, metrics::keys::SAVE_OK);
        m.bump(day, metrics::keys::GRANT_DROP, grants.len() as i64);
        m.bump(day, metrics::keys::GRANT_CRAFT, craft_grants.len() as i64);
        m.bump(day, metrics::keys::GRANT_GEAR, gear_grants.len() as i64);
        let level = sim_core::drops::party_max_level(&final_state);
        let zone = final_state.current_zone_id.clone().unwrap_or_default();
        m.observe_player(day, acct.account_id, metrics::PlayerDay {
            level,
            tick: final_state.tick,
            gil: final_state.gil(),
            pending_drops: sim_core::drops::candidate_counts(&final_state).0 as u32,
            zone: zone.clone(),
        });
        m.observe_zone(acct.account_id, &zone, metrics::FirstSeen {
            level,
            tick: final_state.tick,
            party: party_comp(&final_state),
        });
    }
    (StatusCode::OK, Json(SaveResp { ok: true, daily_claimable_crystal: daily_owed, reason: "stored (verified)".into(), final_tick: bundle.final_tick, grants, craft_grants, gear_grants, resolved_seq, settled_ops: consumed_now, drops }))
}

/// The zone a fresh game starts in (matches the client's ZONE_ID). Genesis anchor.
const START_ZONE: &str = "zone_b1_1";

/// All valid GENESIS hashes for a seed — the states the server can reproduce itself (it
/// issued the seed). Character creation picks a class (baked into the start) but the party
/// NAME is a recorded event, so a freshly-created online character's start is `new_game_as`
/// for one CREATABLE class (or the bare zone-default `new_game_in_zone`). The trust anchor:
/// a legit `start` is one of these OR a previously-confirmed end-state. A forged
/// (non-seed-derived) start still can't match any candidate.
fn genesis_candidates(data: &GameData, seed: u64) -> HashSet<String> {
    let mut set = HashSet::new();
    if let Ok(s) = data::new_starter_party(seed, data, START_ZONE) {
        set.insert(hash::state_hash(&s));
    }
    if let Ok(s) = data::new_game_in_zone(seed, data, START_ZONE) {
        set.insert(hash::state_hash(&s));
    }
    for class in data.creatable_units() {
        if let Ok(s) = data::new_game_as(seed, data, START_ZONE, &class.id) {
            set.insert(hash::state_hash(&s));
        }
    }
    set
}

/// GET /save (auth) — resume: hand back this account's authoritative save (the
/// client `load_json`s `state_json`). 404 if the account has no save yet.
async fn get_save(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> (StatusCode, Json<Option<StoredSave>>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(None));
    };
    // Never alongside a save of this account: the offline time paid below must not overlap a
    // bundle still being verified (it would be paid as offline AND kept as online play).
    let Some(_slot) = SaveSlot::take(&st.saving, acct.account_id) else {
        return (StatusCode::TOO_MANY_REQUESTS, Json(None));
    };
    let mut stored = st.saves.held().get(&acct.account_id).cloned();
    // MIGRATE-ON-RESUME. The client runs `migrate_save_items` when it `load_json`s the save
    // (propagating a catalog rebalance to held items). If the catalog changed since the save
    // was stored, that migration shifts the state HASH — so the client's post-load `start`
    // would differ from `final_hash` (which is what's trusted) → a spurious "not anchored".
    // Fix: migrate here with the CURRENT catalog, trust the migrated hash, and serve the
    // migrated state, so the client's post-load hash matches a trusted base. Safe — the
    // migration is a deterministic derivation of an already-verified (trusted) state.
    if let Some(s) = stored.as_mut() {
        if let Ok(mut state) = serde_json::from_str::<sim_core::state::GameState>(&s.state_json) {
            data::migrate_save_items(&mut state, &st.data);
            let mig_hash = hash::state_hash(&state);
            if mig_hash != s.final_hash {
                // A catalog change since the save was stored → migration shifted the hash.
                // Benign + expected; trust the migrated hash + serve the migrated state.
                println!("migrate-on-resume acct={} tick={} rehashed after a catalog change (trusted)", acct.account_id, s.final_tick);
                st.trusted.held().entry(acct.account_id).or_default().insert(mig_hash.clone());
                if let Some(db) = &st.db {
                    let _ = sqlx::query("INSERT INTO trusted (account_id, state_hash) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                        .bind(acct.account_id as i64).bind(&mig_hash).execute(db).await;
                }
                s.state_json = serde_json::to_string(&state).unwrap_or_else(|_| s.state_json.clone());
                s.final_hash = mig_hash;
            }
        }
    }
    // OFFLINE EXP for the absence this login ends, paid HERE in one transaction (user 2026-10-02;
    // redesigned 2026-10-03): the time since max(latest save, last payout), then the payout clock
    // moves to now. The anchor reset just below starts the tick-rate clock at this same moment, so
    // time paid as offline can never also be saved as online play.
    if let (Some(db), Some(s)) = (&st.db, stored.as_ref().filter(|s| s.saved_at_ms > 0)) {
        if let Ok(state) = serde_json::from_str::<sim_core::state::GameState>(&s.state_json) {
            if let Err(e) = pay_offline_at_login(db, &st, acct.account_id, s.saved_at_ms, &state).await {
                eprintln!("[offline] payout failed for acct {} (not retried; nothing paid): {e}", acct.account_id);
            }
        }
    }
    // Anchor the anti-fast-forward clock at this resume: the session's ticks may advance
    // from HERE (the resumed tick, or 0 for a fresh character) at real time going forward.
    st.tick_ref.held().insert(
        acct.account_id,
        TickAnchor {
            tick: stored.as_ref().map(|s| s.final_tick).unwrap_or(0),
            ms: now_ms(),
            // A new session records from zero. The LIFETIME counter is what carries over — this
            // reset is exactly the layer-1 hole that layer 2 exists to cover.
            recorded: 0,
            from_handshake: true,
            synthesized: false,
        },
    );
    match stored {
        Some(s) => (StatusCode::OK, Json(Some(s))),
        None => (StatusCode::NOT_FOUND, Json(None)),
    }
}

#[derive(serde::Deserialize)]
struct IngestReq {
    /// New checkpoints since the account's pinned head, in order.
    segment: Vec<Snapshot>,
}

#[derive(serde::Deserialize)]
struct TelemetryReq {
    /// Event names from `metrics::client::ALLOWED`. Names, not keys — the `client.` prefix is
    /// added server-side so a forged name cannot reach a server-observed counter.
    events: Vec<String>,
}

#[derive(serde::Serialize)]
struct TelemetryResp {
    accepted: usize,
    rejected: usize,
}

/// POST /telemetry (auth) — the CLIENT half of the record. Everything the save path collects is
/// read off state the server replayed; this is the part only the client can witness: a session
/// that never saved, a screen opened and abandoned, the frame rate on real hardware.
///
/// NO DB WRITE, NO REPLAY, NO SIM. It folds names into the same in-memory accumulator `put_save`
/// uses and returns — the timer flush persists it. That is what lets the client call this without
/// adding to the write rate that `TODO.md` measures as the scaling wall.
///
/// A separate endpoint rather than a field on the save bundle, deliberately: `SaveBundle` lives in
/// sim-core, and hanging telemetry off it would change the serialized shape of every save and put
/// an I/O concern inside the deterministic core, for nothing.
///
/// Every safety rule lives in `metrics::client`, not here, so a second caller cannot skip them.
/// This endpoint is a token check and a fold. It can never gate a reward and must never grow one.
async fn telemetry(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<TelemetryReq>,
) -> (StatusCode, Json<TelemetryResp>) {
    if account_of(&st, &headers).is_none() {
        return (StatusCode::UNAUTHORIZED, Json(TelemetryResp { accepted: 0, rejected: 0 }));
    }
    let (accepted, rejected) = st.metrics.held().observe_client(today_num(), &req.events);
    (StatusCode::OK, Json(TelemetryResp { accepted, rejected }))
}

#[derive(serde::Serialize)]
struct IngestResp {
    pinned: bool,
    reason: String,
    head_tick: u64,
    total_pinned: usize,
}

/// POST /ingest (auth) — the cheap continuous-integrity layer (Slice 3). Accepts a
/// stream segment of hash-chain checkpoints and PINS it WITHOUT replaying: each must
/// (1) advance the tick strictly, and (2) be chain-link-consistent with the pinned
/// head (`hash::chain_link(prev_chain, tick, state_hash) == chain_hash`). Any
/// divergence — a rewritten past, a rolled-back tick, a duped branch — is rejected
/// and the pinned head is left untouched (monotonic). Pinning ≠ verifying: it commits
/// the client to this chain; a real replay (Slice 4) later proves the state is real.
async fn ingest(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<IngestReq>,
) -> (StatusCode, Json<IngestResp>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(IngestResp { pinned: false, reason: "missing/invalid token".into(), head_tick: 0, total_pinned: 0 }));
    };
    // Taking the head IS the emptiness check: one expression both rejects an empty segment and
    // produces the value pinned below, so the guard can never drift away from its use.
    let Some(head) = req.segment.last().cloned() else {
        return (StatusCode::BAD_REQUEST, Json(IngestResp { pinned: false, reason: "empty segment".into(), head_tick: 0, total_pinned: 0 }));
    };

    let mut pins = st.pins.held();
    let cur = pins.get(&acct.account_id);
    // Genesis (no pins yet) starts from prev_chain = "" at tick 0.
    let (mut prev_chain, mut prev_tick, mut count) = match cur {
        Some(p) => (p.head.chain_hash.clone(), p.head.tick, p.count),
        None => (String::new(), 0u64, 0usize),
    };

    for cp in &req.segment {
        if cp.tick <= prev_tick {
            return (StatusCode::CONFLICT, Json(IngestResp {
                pinned: false,
                reason: format!("non-monotonic tick {} (pinned head at {prev_tick})", cp.tick),
                head_tick: prev_tick,
                total_pinned: count,
            }));
        }
        let expect = hash::chain_link(&prev_chain, cp.tick, &cp.state_hash);
        if expect != cp.chain_hash {
            return (StatusCode::CONFLICT, Json(IngestResp {
                pinned: false,
                reason: format!("chain divergence at tick {} — rollback/dupe/rewrite rejected", cp.tick),
                head_tick: prev_tick,
                total_pinned: count,
            }));
        }
        prev_chain = cp.chain_hash.clone();
        prev_tick = cp.tick;
        count += 1;
    }

    let head_tick = head.tick;
    pins.insert(acct.account_id, PinnedHead { head, count });
    (StatusCode::OK, Json(IngestResp { pinned: true, reason: "pinned".into(), head_tick, total_pinned: count }))
}

/// Fraction of non-promoted gameplay submissions to spot-check (basis points).
/// 1000 = 10% (the decided audit rate). Real deploy makes this risk-weighted
/// (higher for high-value / flagged accounts) and adjustable under load.
const AUDIT_RATE_BP: u32 = 1000;

/// Trigger policy (Slice 4): decide whether a submission must pay for a real sim
/// replay, or can ride on cheap pinning alone. Replay ONLY when it matters:
///   - `promote` = the client wants to make an item TRADEABLE (export/mint gate) —
///     provenance MUST be proven, so ALWAYS replay (value can't leave unchecked).
///   - random AUDIT — a ~10% spot-check of the rest, rolled with the SERVER's own
///     RNG so it's UNPREDICTABLE to the client (a predictable "every-Nth" audit is
///     worthless — a cheater just avoids the checked slots). Deters forged progress
///     / leaderboard claims that never promote.
///   - (future) suspicious flags: prior pin divergence, timing anomalies; and
///     risk-weighting the audit rate.
/// `None` = skip replay (provisional, pinned only). Farming bound loot rarely
/// triggers → the common case is free.
fn should_verify(promote: bool) -> Option<&'static str> {
    if promote {
        return Some("promotion (tradeable export)");
    }
    // Server-side RNG (the server is NOT sim-core → real randomness is fine, and
    // the client can't predict which submissions get audited).
    if rand::random::<u32>() % 10_000 < AUDIT_RATE_BP {
        return Some("random audit (~10%)");
    }
    None
}

#[derive(serde::Deserialize)]
struct SubmitReq {
    bundle: SaveBundle,
    /// Client requests a tradeable promotion for this segment → forces replay.
    #[serde(default)]
    promote: bool,
}

#[derive(serde::Serialize)]
struct SubmitResp {
    /// "provisional" (pinned, not replayed) · "registered" (replayed OK, tradeable) ·
    /// "rejected" (replayed, failed).
    status: String,
    replayed: bool,
    trigger: String,
    total: u64,
    replayed_count: u64,
    replay_rate_pct: u64,
}

/// POST /submit (auth) — the trigger-gated ingest (Slice 4). Cheap by default:
/// pins provisionally with NO replay, and only pays for a real `verified_state`
/// replay when `should_verify` fires (promotion / random audit / suspicious). A
/// registered submission's verified state is stored authoritatively; a failed
/// replay is rejected. Response carries the running replay-rate — the compute-cost
/// telemetry that should stay low.
async fn submit(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SubmitReq>,
) -> (StatusCode, Json<SubmitResp>) {
    let snapshot = |st: &AppState, status: &str, replayed: bool, trigger: String| {
        let total = st.stats.total.load(Ordering::SeqCst);
        let replayed_count = st.stats.replayed.load(Ordering::SeqCst);
        let rate = if total == 0 { 0 } else { replayed_count * 100 / total };
        Json(SubmitResp {
            status: status.to_string(),
            replayed,
            trigger,
            total,
            replayed_count,
            replay_rate_pct: rate,
        })
    };

    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, snapshot(&st, "unauthorized", false, "none".into()));
    };
    st.stats.total.fetch_add(1, Ordering::SeqCst);
    let d = &*st.data;

    let Some(reason) = should_verify(req.promote) else {
        // Cheap path: no trigger → provisional (pinned only, no replay).
        return (StatusCode::OK, snapshot(&st, "provisional", false, "none (pinned only)".into()));
    };

    // Triggered → pay for a real replay.
    st.stats.replayed.fetch_add(1, Ordering::SeqCst);
    if let Err((code, msg)) = policy_check(d, &req.bundle) {
        return (code, snapshot(&st, "rejected", true, format!("{reason}: {msg}")));
    }
    if let Err((code, msg)) = bundle_within_limits(&req.bundle) {
        return (code, snapshot(&st, "rejected", true, format!("{reason}: {msg}")));
    }
    match req.bundle.verified_state(d) {
        Some(_final_state) => {
            // THIS USED TO WRITE `st.saves`, and that was the forgery hole. `/submit` is the
            // audit/telemetry route: it runs `policy_check` + `bundle_within_limits` + the replay,
            // and NOT the anchor check, `within_tick_rate`, `session_excess` or the taint gate.
            // The replay proves nothing on its own — with `log: []` and `final_tick == start.tick`
            // it clones `start`, advances zero ticks and hashes to exactly the `final_hash` the
            // caller computed, so `state_hash(end) == final_hash` holds BY CONSTRUCTION for any
            // hand-written state.
            //
            // Writing that state made it the authoritative save, and `put_save`'s migration
            // exception then read it back: `migrate_save_items` is idempotent on an
            // already-current catalog, so the comparison was trivially true and the forged hash
            // was inserted into `trusted`. Every later save chained from it legitimately. An audit
            // reproduced the whole loop on the live router — 409 NOT-ANCHORED, then `/submit`,
            // then the SAME bundle accepted with `anchor-via-migration`.
            //
            // The exception is only safe if what it compares against is itself anchored. Nothing
            // outside `put_save` may decide what a stored save is.
            (StatusCode::OK, snapshot(&st, "registered", true, reason.to_string()))
        }
        None => (StatusCode::OK, snapshot(&st, "rejected", true, format!("{reason}: replay mismatch"))),
    }
}

// --- MAILBOX (Slice 2a) — general server→player delivery over mailbox::Mailbox ------
//
// Ops send mail (dev-gated); a player lists + claims their own (auth). Isolated in this
// slice: /mail/claim returns the payload the sim WILL apply — Slice 2b wires ClaimMail into
// the verified GameState. Claim is idempotent (claim-once) at the ledger level here.

#[derive(serde::Deserialize)]
struct MailSendReq {
    to_account: u64,
    kind: mailbox::MailKind,
    #[serde(default)] gil: u64,
    #[serde(default)] def_id: String,
    #[serde(default)] qty: u64,
    #[serde(default)] reason: String,
}
#[derive(serde::Serialize)]
struct MailSendResp { ok: bool, reason: String, #[serde(skip_serializing_if = "Option::is_none")] mail_id: Option<u64> }

#[derive(serde::Deserialize)]
struct MailClaimReq { mail_id: u64 }
#[derive(serde::Serialize)]
struct MailClaimResp { ok: bool, reason: String, #[serde(skip_serializing_if = "Option::is_none")] payload: Option<mailbox::Claim> }

fn mail_status(e: mailbox::MailError) -> (StatusCode, String) {
    use mailbox::MailError::*;
    match e {
        NotFound => (StatusCode::NOT_FOUND, "no such mail".into()),
        NotOwner => (StatusCode::FORBIDDEN, "not your mail".into()),
        AlreadyClaimed => (StatusCode::CONFLICT, "already claimed".into()),
        DuplicateInSave => (StatusCode::CONFLICT, "the same mail claimed twice".into()),
        EmptyPayload => (StatusCode::BAD_REQUEST, "mail must carry Gil or an item".into()),
        PayloadMismatch => (StatusCode::CONFLICT, "claim payload does not match the mail".into()),
        Withdrawn => (StatusCode::GONE, "mail was withdrawn by a refund".into()),
    }
}

/// POST /dev/mail-send — OPS: deliver mail to `to_account` (compensation / reward / refund /
/// gift / AH proceeds). Dev-gated. In production this is the audited ops path.
async fn mail_send(State(st): State<AppState>, Json(req): Json<MailSendReq>) -> (StatusCode, Json<MailSendResp>) {
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(MailSendResp { ok: false, reason: "mail needs a database".into(), mail_id: None }));
    };
    match mailbox_store::send(db, req.to_account, req.kind, req.gil, &req.def_id, req.qty, &req.reason).await {
        Ok(id) => (StatusCode::OK, Json(MailSendResp { ok: true, reason: "sent".into(), mail_id: Some(id) })),
        Err(e) => { let (code, reason) = mail_status(e); (code, Json(MailSendResp { ok: false, reason, mail_id: None })) }
    }
}

#[derive(serde::Deserialize)]
struct DevSteamMintReq {
    /// The public SteamID64 of an account that has logged in with Steam (`steam:<id>`).
    steamid: u64,
    /// Catalog id to mint; must carry a `steam_itemdef`. Default `farm_box`.
    #[serde(default)]
    def_id: String,
}

/// `POST /dev/steam-mint-test` (DEV_KEY) — queue ONE `steam_mints` row for a Steam account as if
/// it had sent the item, so the worker's real `AddItem` path can be tested end to end before the
/// client has a Send-to-Steam button. The seq continues the account's own, so a real send later
/// never collides. Nothing leaves any bag: this mints an item that was never in the game.
async fn dev_steam_mint_test(State(st): State<AppState>, Json(req): Json<DevSteamMintReq>) -> (StatusCode, Json<serde_json::Value>) {
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok": false, "reason": "needs a database"})));
    };
    let def = if req.def_id.is_empty() { sim_core::drops::FARM_BOX_ID.to_string() } else { req.def_id };
    let Some(itemdef) = st.data.item(&def).and_then(|d| d.steam_itemdef) else {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"ok": false, "reason": format!("'{def}' has no steam_itemdef")})));
    };
    let acct: Option<(i64,)> = match sqlx::query_as("SELECT account_id FROM accounts WHERE name = $1")
        .bind(format!("steam:{}", req.steamid)).fetch_optional(db).await
    {
        Ok(a) => a,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    };
    let Some((account_id,)) = acct else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"ok": false, "reason": "no account for that steamid — log in with Steam once first"})));
    };
    let row: Result<(i64,), sqlx::Error> = sqlx::query_as(
        "INSERT INTO steam_mints (account_id, def_id, seq, itemdefid, note) \
         SELECT $1, $2, COALESCE(MAX(seq), 0) + 1, $3, 'dev test' FROM steam_mints WHERE account_id = $1 AND def_id = $2 \
         RETURNING seq",
    )
    .bind(account_id).bind(&def).bind(i64::from(itemdef))
    .fetch_one(db).await;
    match row {
        Ok((seq,)) => (StatusCode::OK, Json(serde_json::json!({"ok": true, "account_id": account_id, "def_id": def, "seq": seq, "itemdefid": itemdef,
            "note": "queued — the worker mints within a minute when STEAM_INVENTORY_LIVE=1"}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok": false, "reason": e.to_string()}))),
    }
}

/// GET /mail — the caller's unclaimed inbox.
async fn mail_inbox(State(st): State<AppState>, headers: HeaderMap) -> (StatusCode, Json<Vec<mailbox::MailItem>>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(vec![]));
    };
    let Some(db) = &st.db else { return (StatusCode::SERVICE_UNAVAILABLE, Json(vec![])) };
    match mailbox_store::inbox_for(db, acct.account_id).await {
        Ok(items) => (StatusCode::OK, Json(items)),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(vec![])),
    }
}

/// POST /mail/claim — claim the caller's mail; returns the payload (Gil/item) to apply. Idempotent
/// (a re-claim → 409 already-claimed, never a second grant). Slice 2b applies the payload to GameState.
async fn mail_claim(State(st): State<AppState>, headers: HeaderMap, Json(req): Json<MailClaimReq>) -> (StatusCode, Json<MailClaimResp>) {
    let Some(acct) = account_of(&st, &headers) else {
        return (StatusCode::UNAUTHORIZED, Json(MailClaimResp { ok: false, reason: "unauthorized".into(), payload: None }));
    };
    let Some(db) = &st.db else {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(MailClaimResp { ok: false, reason: "mail needs a database".into(), payload: None }));
    };
    // In a transaction so a NotOwner claim ROLLS BACK the conditional update that discovered it —
    // the update has to fire first to make claim-once atomic, so the rollback is what stops a
    // stranger's failed claim from consuming someone else's mail.
    let Ok(mut tx) = db.begin().await else {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(MailClaimResp { ok: false, reason: "persist failed — retry".into(), payload: None }));
    };
    match mailbox_store::claim(&mut tx, acct.account_id, req.mail_id, now_ms()).await {
        Ok(c) => {
            if tx.commit().await.is_err() {
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(MailClaimResp { ok: false, reason: "persist failed — retry".into(), payload: None }));
            }
            (StatusCode::OK, Json(MailClaimResp { ok: true, reason: "claimed".into(), payload: Some(c) }))
        }
        Err(e) => { let (code, reason) = mail_status(e); (code, Json(MailClaimResp { ok: false, reason, payload: None })) }
    }
}

/// REAL-Postgres integration tests for the L1 market durability fixes (audit HIGH-1/HIGH-2).
/// Gated on `TEST_DATABASE_URL` — SKIP (pass) when unset so normal `cargo test` isn't blocked.
/// Run: `TEST_DATABASE_URL=postgres://... cargo test -p verify-server pg_ -- --test-threads=1`.
#[cfg(test)]
mod pg_integration {
    use super::*;
    /// Fixed listing clock — these tests are about persistence, not expiry.
    const T0: i64 = 1_000_000_000_000;
    use mailbox::MailKind;
    use std::sync::Arc;

    /// Every pg test shares ONE database and `setup()` TRUNCATEs it, so they must not overlap.
    /// cargo runs tests on parallel threads by default; before these ran by default the race was
    /// invisible because a human always typed `--test-threads=1`. Holding this for the life of a
    /// test serialises them without forcing that flag on the whole suite.
    static PG_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    /// The pg tests cover the MONEY path — wallet, ledger, idempotency, the mileage exchange —
    /// and the SQL they exercise is otherwise only type-checked, so a wrong column or a bad
    /// ON CONFLICT target would compile and fail in production.
    ///
    /// They used to require `TEST_DATABASE_URL`, which nothing sets: not smoke.sh, not CI, not
    /// the Makefile. In practice they ran only when someone remembered to export it, and when
    /// they did not, `cargo test` still printed ok — green because UNTESTED, the same silent-pass
    /// shape as a `.get(key, default)` on a key that no longer exists.
    ///
    /// So the URL now DEFAULTS to the conventional local dev database. Skipping is still fine
    /// (no Postgres on this machine is a legitimate state) but it is now LOUD and says exactly
    /// what to do about it.
    async fn setup() -> Option<(sqlx::PgPool, tokio::sync::MutexGuard<'static, ()>)> {
        let guard = PG_LOCK.lock().await;
        let url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
            let user = std::env::var("USER").unwrap_or_else(|_| "postgres".into());
            format!("postgres://{user}@localhost/roi_test")
        });
        // REFUSE A REMOTE DATABASE. `setup` TRUNCATEs every money table including `saves`, and
        // `migrate` DROPs the retired market tables; pointed at production it erases the game, and `cargo test` is not a
        // command anyone runs expecting that. The env var is the only way this URL is not the
        // local default, so the check belongs exactly here.
        //
        // Escape hatch on purpose: a throwaway remote database is a legitimate thing to want, and
        // a guard with no way past it gets deleted rather than respected.
        assert!(
            is_local_db(&url) || std::env::var("ROI_ALLOW_REMOTE_TEST_DB").is_ok(),
            "TEST_DATABASE_URL points somewhere that is not localhost:\n  {url}\n\
             These tests TRUNCATE and DROP. If that database is disposable, set \
             ROI_ALLOW_REMOTE_TEST_DB=1."
        );
        // Short timeout ON PURPOSE: sqlx defaults to 30s, which turns "this machine has no
        // Postgres" into a minute of dead air on every `cargo test` run.
        let pool = match sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_secs(2))
            .connect(&url)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                eprintln!(
                    "\n[pg] SKIPPED — no database at {url} ({e}).\n\
                     [pg] These tests cover the money path. To run them:\n\
                     [pg]   pg_ctl -D /opt/homebrew/var/postgresql@16 start && createdb roi_test\n\
                     [pg] or point TEST_DATABASE_URL somewhere else.\n"
                );
                return None;
            }
        };
        migrate(&pool).await.expect("migrate");
        sqlx::query("TRUNCATE account_bans, saves, trusted, mailbox, cheat_flags, integrity, consumed_market_ops, store_orders, entitlements, crystal_wallet, crystal_ledger, gacha_pity, gacha_rolls, gacha_mileage, gacha_mileage_std, gacha_standard, gacha_newcomer, gacha_exchanges, commander_stage, tutorial_progress, shop_buys, bp_state, bp_claims, bp_entitlements, monthly_card, bound_items, bound_ledger, daily_progress, commander, offline_paid, mint_coupon_clock, seasons, weekly_progress, claim_progress, slot_tickets, lootbox_opens, lootbox_quota, farm_box_opens, farm_opens, farm_pity, farm_progress, steam_mints, steam_imports, fuse_outstanding, fuse_consumed, story_claims, first_clear, player_daily, metrics_daily, mtx_txns, active_sessions")
            .execute(&pool).await.expect("truncate");
        Some((pool, guard))
    }

    /// Run the accrual's OWN SQL against a real database.
    ///
    /// This exists because of a bug that shipped past everything else: the bp_state SELECT was
    /// widened to read `time_xp_day` and `time_xp_today` while the ALTER that adds them silently
    /// failed to land, so `/save` would have failed in production. It compiled, every unit test
    /// passed, and even the pg suite passed — because `migrate()` catches bad DDL but nothing was
    /// RUNNING these queries. A column that does not exist is only a problem at execution time.
    ///
    /// So this asserts nothing clever. It executes the statements the accrual executes and
    /// round-trips a commission day, which is exactly the coverage that was missing.
    /// True when a Postgres URL addresses this machine. Host is what lies between the last `@`
    /// (or `://` when there are no credentials) and the next `/`, `:` or `?`.
    fn is_local_db(url: &str) -> bool {
        let after_scheme = url.split("://").nth(1).unwrap_or(url);
        let hostish = after_scheme.rsplit('@').next().unwrap_or(after_scheme);
        let host = hostish
            .split(['/', ':', '?'])
            .next()
            .unwrap_or("")
            .trim_start_matches('[')
            .trim_end_matches(']');
        // A `host=` / `hostaddr=` query parameter overrides the URL's host (libpq / sqlx): only a
        // unix-socket directory there is local (audit 2026-10-03).
        // Keys are percent-decoded the way sqlx decodes them (`?%68ost=` is `?host=`).
        let decode = |k: &str| {
            let b = k.as_bytes();
            let mut out = Vec::with_capacity(b.len());
            let mut i = 0;
            while i < b.len() {
                let hex = |c: u8| (c as char).to_digit(16);
                match (b.get(i), b.get(i + 1).copied().and_then(hex), b.get(i + 2).copied().and_then(hex)) {
                    (Some(b'%'), Some(h), Some(l)) => { out.push((h * 16 + l) as u8); i += 3; }
                    (Some(c), _, _) => { out.push(*c); i += 1; }
                    _ => break,
                }
            }
            String::from_utf8_lossy(&out).to_ascii_lowercase()
        };
        let socket = |v: &str| v.starts_with('/') || v.to_ascii_lowercase().starts_with("%2f");
        let mut query_host = None;
        let query_host_ok = url.split_once('?').is_none_or(|(_, q)| {
            q.split('&').all(|kv| match kv.split_once('=') {
                Some((k, v)) if matches!(decode(k).as_str(), "host" | "hostaddr") => {
                    query_host = Some(v.to_string());
                    socket(v)
                }
                _ => true,
            })
        });
        // An EMPTY host with no `host=` falls back to `PGHOST`, which may name a remote server.
        let env_host_ok = !host.is_empty() || query_host.is_some()
            || std::env::var("PGHOST").map_or(true, |h| h.is_empty() || h.starts_with('/') || matches!(h.as_str(), "localhost" | "127.0.0.1" | "::1"));
        matches!(host, "localhost" | "127.0.0.1" | "::1" | "") && query_host_ok && env_host_ok
    }

    #[test]
    fn only_a_local_database_is_accepted_for_tests() {
        for ok in [
            "postgres://me@localhost/roi_test",
            "postgres://localhost/roi_test",
            "postgres://me:pw@127.0.0.1:5432/roi_test",
            "postgresql://me@[::1]:5432/roi_test",
            "postgres://me@/roi_test?host=/tmp",
        ] {
            assert!(is_local_db(ok), "{ok} is local");
        }
        for bad in [
            "postgres://user:pw@ep-cool-name-123.us-east-2.aws.neon.tech/roi?sslmode=require",
            "postgres://me@10.0.0.5/roi",
            // The one that matters: a hostname that merely CONTAINS the safe word.
            "postgres://me@localhost.evil.example/roi",
            "postgres://me@not-localhost/roi",
            "postgres://me@localhost/roi?host=ep-x.neon.tech",
            "postgres://me@localhost/roi?sslmode=disable&hostaddr=10.0.0.5",
            "postgres://me@localhost/roi?%68ost=ep-x.neon.tech",
        ] {
            assert!(!is_local_db(bad), "{bad} must be refused — setup() TRUNCATEs and DROPs");
        }
    }

    /// A REFUND MUST TAKE THE CAPABILITY BACK, and `owns_sku` is what makes that true anywhere.
    ///
    /// Until this, `revoked_at` had one production reader — the first-buy count — so a refund
    /// looked like it worked while the account kept whatever the SKU granted. Nothing sells a
    /// capability YET; the +3 rare-bag-slot SKU is the one that will, and this is the check it
    /// will be built on.
    /// GEMS ACTUALLY LEAVE THE WALLET, and a debit that cannot be paid fails the roll.
    ///
    /// The audit's demonstration: make the wallet upsert a no-op and every pull becomes free while
    /// all 183 tests stay green — because the write and the assertion were the same number
    /// computed once in memory. This drives the REAL statement and reads the row back.
    /// TWO CONCURRENT PULLS ON ONE KEY: exactly one may roll.
    ///
    /// The guard is the `ON CONFLICT DO NOTHING` that claims `idem_key` INSIDE the transaction,
    /// and nothing had ever touched it. The audit removed it and ran two calls together: 6,000
    /// crystal spent instead of 3,000, twenty reward mails, pity advanced 17, and the second request
    /// overwrote the first's stored results — so the player is charged twice and can only ever
    /// see one of the two hauls.
    ///
    /// Driven as two REAL transactions racing on the real table, because the claim's whole
    /// meaning is what happens when two of them arrive at once.
    /// EVERY COUPON A WALLET HOLDS IS EXPLAINED BY THE LEDGER — the `crystal_ledger` invariant,
    /// applied to the counter that now pays for pulls.
    ///
    /// `docs/bound-items-design.md` asks for this by name: a coupon bought with real money is
    /// something the operator has to answer questions about, and a counter that can be edited and
    /// drift with nothing to compare it against is the I7 shape.
    /// THE WHOLE COUPON ROAD, in one test: the ladder credits, the pull spends, the ledger
    /// explains the balance at every step.
    ///
    /// Everything else here checks ONE joint — and that is how "the ledger over-states the
    /// balance" survived: the credit test and the debit test each agreed with themselves. This
    /// walks the road a real newcomer walks.
    /// PARTY-SLOT TICKETS pay once per slot: slot 2 mails Lumi, every later slot ten coupons; a
    /// save seen twice pays nothing the second time, a save that jumped two slots pays both, and
    /// the ledger explains every coupon.
    #[tokio::test]
    async fn pg_party_slot_tickets_pay_once_per_slot_and_never_twice() {
        let Some((pool, _g)) = setup().await else { return };
        let (acct, kind) = (8941i64, gacha::STANDARD_COUPON_KIND);
        for t in ["bound_items", "bound_ledger", "slot_tickets"] {
            sqlx::query(&format!("DELETE FROM {t} WHERE account_id = $1")).bind(acct)
                .execute(&pool).await.expect("clean");
        }
        let held = |pool: sqlx::PgPool| async move {
            let b: Option<(i64,)> = sqlx::query_as(
                "SELECT qty FROM bound_items WHERE account_id = $1 AND kind = $2")
                .bind(acct).bind(kind).fetch_optional(&pool).await.expect("balance");
            let l: (Option<i64>,) = sqlx::query_as(
                "SELECT sum(delta)::bigint FROM bound_ledger WHERE account_id = $1 AND kind = $2 AND reason = 'party_slot'")
                .bind(acct).bind(kind).fetch_one(&pool).await.expect("ledger");
            (b.map(|(q,)| q).unwrap_or(0), l.0.unwrap_or(0))
        };
        let st = test_state(None);
        let lumi = gacha_item(&st, SLOT_TWO_UNIT).expect("Lumi resolves");
        let pay = |pool: sqlx::PgPool, bought: u64, lumi: sim_core::items::Item| async move {
            let mut tx = pool.begin().await.expect("tx");
            slot_tickets(&mut tx, acct as u64, bought, Some(&lumi)).await.expect("slot_tickets");
            tx.commit().await.expect("commit");
        };
        let lumi_mails = |pool: sqlx::PgPool| async move {
            let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM mailbox WHERE account_id = $1 AND reason = 'party slot 2'")
                .bind(acct).fetch_one(&pool).await.expect("mail");
            n
        };
        pay(pool.clone(), 0, lumi.clone()).await;
        assert_eq!(held(pool.clone()).await, (0, 0), "nothing opened, nothing paid");
        pay(pool.clone(), 1, lumi.clone()).await;
        assert_eq!((held(pool.clone()).await, lumi_mails(pool.clone()).await), ((0, 0), 1), "slot 2 pays Lumi, not coupons");
        pay(pool.clone(), 1, lumi.clone()).await;
        assert_eq!((held(pool.clone()).await, lumi_mails(pool.clone()).await), ((0, 0), 1), "the same save again pays nothing");
        pay(pool.clone(), 3, lumi.clone()).await;
        assert_eq!(held(pool.clone()).await, (20, 20), "slots 3 and 4 pay ten each");
        pay(pool.clone(), 2, lumi.clone()).await;
        assert_eq!(held(pool.clone()).await, (20, 20), "an older save never pays or claws back");
        let mark: (i64,) = sqlx::query_as("SELECT paid FROM slot_tickets WHERE account_id = $1")
            .bind(acct).fetch_one(&pool).await.expect("watermark");
        assert_eq!(mark.0, 3);
    }

    /// STEAM queue: every item a verified save sent past the watermark becomes one `pending`
    /// row with the catalog's itemdef; the same save again, or an older one, queues nothing.
    /// The worker stays off without `STEAM_INVENTORY_LIVE` and a key, so rows only wait.
    #[tokio::test]
    async fn pg_steam_queue_makes_one_pending_row_per_sent_item_once() {
        let Some((pool, _g)) = setup().await else { return };
        let acct = 8953i64;
        sqlx::query("DELETE FROM steam_mints WHERE account_id = $1").bind(acct).execute(&pool).await.expect("clean");
        let st = test_state(Some(pool.clone()));
        async fn queue(pool: &sqlx::PgPool, st: &AppState, acct: i64, n: u64) {
            let mut tx = pool.begin().await.expect("tx");
            let sent: std::collections::BTreeMap<String, u64> = [("farm_box".to_string(), n)].into_iter().collect();
            steam_queue(&mut tx, st, acct as u64, &sent).await.expect("queue");
            tx.commit().await.expect("commit");
        }
        let rows = |pool: sqlx::PgPool| async move {
            let r: Vec<(i64, i64, String)> = sqlx::query_as(
                "SELECT seq, itemdefid, status FROM steam_mints WHERE account_id = $1 ORDER BY seq")
                .bind(acct).fetch_all(&pool).await.expect("rows");
            r
        };
        queue(&pool, &st, acct, 2).await;
        assert_eq!(rows(pool.clone()).await, vec![(1, 1001, "pending".into()), (2, 1001, "pending".into())]);
        queue(&pool, &st, acct, 2).await;
        queue(&pool, &st, acct, 1).await;
        assert_eq!(rows(pool.clone()).await.len(), 2, "the same save, or an older one, queues nothing");
        queue(&pool, &st, acct, 3).await;
        assert_eq!(rows(pool.clone()).await.len(), 3, "one more sent, one more row");
        mint_steam_items(&st).await; // switched off: a no-op
        assert!(rows(pool.clone()).await.iter().all(|r| r.2 == "pending"), "nothing minted while off");
    }

    /// FARM BANNER: only verified ticks the server saw earn boxes (a first sight only anchors),
    /// the daily cap holds, a save seen twice mails nothing more; each opened box credits one
    /// bound ticket once (a 0.0.477 pay-on-open watermark is honoured); a pull spends tickets,
    /// pays the fixed bundle or the character on its pity, and a replayed key spends nothing.
    #[tokio::test]
    async fn pg_farm_banner_boxes_become_tickets_and_tickets_pull_once() {
        let Some((pool, _g)) = setup().await else { return };
        let acct = 8952i64;
        for t in ["farm_progress", "farm_opens", "farm_box_opens", "farm_pity", "mailbox", "bound_items", "bound_ledger", "gacha_rolls"] {
            sqlx::query(&format!("DELETE FROM {t} WHERE account_id = $1")).bind(acct)
                .execute(&pool).await.expect("clean");
        }
        let mut st = test_state(Some(pool.clone()));
        st.farm_events = vec![farm::Event {
            id: "test-farm".into(),
            starts_ms: 0,
            ends_ms: u64::MAX,
            featured: "exemplar_zolfia".into(),
            box_def: "farm_box".into(),
            roll_ticks: 36_000,
            box_ppm: 1_000_000, // every roll hits: the cap is what is under test
            daily_cap: 2,
            featured_ppm: 0,     // only the pity can pay it
            pity: 3,
        }];
        let mails = |pool: sqlx::PgPool, def: &'static str| async move {
            let n: (i64,) = sqlx::query_as(
                "SELECT count(*) FROM mailbox WHERE account_id = $1 AND reason = 'farm banner' AND item_json LIKE $2")
                .bind(acct).bind(format!("%\"{def}\"%")).fetch_one(&pool).await.expect("mail");
            n.0
        };
        let tickets = |pool: sqlx::PgPool| async move {
            let n: Option<(i64,)> = sqlx::query_as("SELECT qty FROM bound_items WHERE account_id = $1 AND kind = 'farm_ticket:farm_box'")
                .bind(acct).fetch_optional(&pool).await.expect("tickets");
            n.map_or(0, |r| r.0)
        };
        async fn accrue(pool: &sqlx::PgPool, st: &AppState, acct: i64, tick: u64) -> usize {
            let mut tx = pool.begin().await.expect("tx");
            let drops = farm_accrue(&mut tx, st, acct as u64, tick, 100, 1).await.expect("accrue");
            assert!(drops.iter().all(|m| m.reason == "farm banner" && m.item.as_ref().is_some_and(|i| i.def_id == "farm_box")));
            tx.commit().await.expect("commit");
            drops.len()
        }
        async fn open(pool: &sqlx::PgPool, st: &AppState, acct: i64, n: u64, legacy: u64) {
            let mut tx = pool.begin().await.expect("tx");
            let opened: std::collections::BTreeMap<String, u64> = [("farm_box".to_string(), n), ("gil".to_string(), 9)].into_iter().collect();
            farm_tickets(&mut tx, st, acct as u64, &opened, legacy).await.expect("tickets");
            tx.commit().await.expect("commit");
        }
        async fn pull(pool: &sqlx::PgPool, st: &AppState, acct: i64, count: u32, key: &str) -> Result<FarmPulled, FarmPullError> {
            let mut tx = pool.begin().await.expect("tx");
            let r = farm_pull(&mut tx, st, acct as u64, "farm_box", count, key).await;
            if r.is_ok() { tx.commit().await.expect("commit") } else { tx.rollback().await.expect("rollback") }
            r
        }
        assert_eq!(accrue(&pool, &st, acct, 1_000).await, 0);
        assert_eq!(mails(pool.clone(), "farm_box").await, 0, "a first sight only anchors");
        assert_eq!(accrue(&pool, &st, acct, 1_000 + 3 * 36_000).await, 2, "the save's drops are the rows it minted");
        assert_eq!(mails(pool.clone(), "farm_box").await, 2, "three hours, three hits, the cap is two");
        assert_eq!(accrue(&pool, &st, acct, 1_000 + 3 * 36_000).await, 0, "a row is listed in one reply only");
        assert_eq!(mails(pool.clone(), "farm_box").await, 2, "the same save again mails nothing");

        // A 0.0.477 account: one box already opened AND paid under the old flow.
        sqlx::query("INSERT INTO farm_opens (account_id, paid, since) VALUES ($1, 1, 1)").bind(acct)
            .execute(&pool).await.expect("legacy");
        open(&pool, &st, acct, 2, 1).await; // 2 new opens + the legacy one
        assert_eq!(tickets(pool.clone()).await, 2, "two tickets: the paid legacy open credits none, a non-box id none");
        open(&pool, &st, acct, 2, 1).await;
        assert_eq!(tickets(pool.clone()).await, 2, "the same save again credits nothing");

        assert!(matches!(pull(&pool, &st, acct, 10, "k0").await, Err(FarmPullError::NotEnoughTickets { have: 2 })));
        assert!(matches!(pull(&pool, &st, acct, 2, "k0").await, Err(FarmPullError::BadCount)));
        // Pity carried from the old flow (since 1): pull 1 is the 2nd without, pull 2 the 3rd = pity.
        let a = pull(&pool, &st, acct, 1, "k1").await.expect("pull 1");
        assert!(a.tickets_left == 1 && a.since == 2, "no character yet");
        let b = pull(&pool, &st, acct, 1, "k2").await.expect("pull 2");
        assert_eq!((b.tickets_left, b.since), (0, 0), "the pity pull");
        assert_eq!(mails(pool.clone(), "exemplar_zolfia").await, 1, "the character is mailed");
        let again = pull(&pool, &st, acct, 1, "k2").await.expect("replay");
        assert!(again.replayed && again.results == b.results, "a replayed key spends and pays nothing");
        assert_eq!(mails(pool.clone(), "exemplar_zolfia").await, 1);
        assert!(matches!(pull(&pool, &st, acct, 1, "k3").await, Err(FarmPullError::NotEnoughTickets { have: 0 })));
        let first = a.results.get(0).and_then(|r| r.get("kind")).and_then(|k| k.as_str()).unwrap_or_default().to_string();
        assert!(first == "gil" || first == "sigil", "the first pull paid the bundle: {first}");
        let paid: (i64,) = sqlx::query_as(
            "SELECT count(*) FROM mailbox WHERE account_id = $1 AND reason = 'farm banner' AND (gil > 0 OR item_json LIKE '%\"bound\":true%')")
            .bind(acct).fetch_one(&pool).await.expect("bundle");
        assert_eq!(paid.0, 1, "one bundle mail: Gil, or a bound Sigil stack");
        let ledger: (i64,) = sqlx::query_as("SELECT COALESCE(sum(delta), 0)::bigint FROM bound_ledger WHERE account_id = $1 AND kind = 'farm_ticket:farm_box'")
            .bind(acct).fetch_one(&pool).await.expect("ledger");
        assert_eq!(ledger.0, 0, "credits and spends net to the balance");
    }

    /// SEALED CACHE opens pay once per open past the watermark, every payout has a ledger row,
    /// the verdict for an open is the same however many times its save is seen, and a coupon
    /// tier that is out of quota pays the consolation crystal instead — the quota row moves only
    /// on real wins. Run against a known secret so the nonce → outcome map is fixed.
    #[tokio::test]
    async fn pg_lootbox_opens_pay_once_and_the_monthly_quota_caps_the_coupons() {
        let Some((pool, _g)) = setup().await else { return };
        let acct = 8951i64;
        for t in ["bound_items", "bound_ledger", "crystal_wallet", "crystal_ledger", "lootbox_opens"] {
            sqlx::query(&format!("DELETE FROM {t} WHERE account_id = $1")).bind(acct)
                .execute(&pool).await.expect("clean");
        }
        sqlx::query("DELETE FROM lootbox_quota WHERE month = '1970-01'").execute(&pool).await.expect("clean");
        let secret = "test-secret";
        // Predict from the pure roll what opens 1..=N produce, and total it two ways.
        let predict = |n: u64| -> (i64, i64) {
            let mut c = 0i64;
            let mut k = 0i64;
            for open in 1..=n {
                match lootbox::roll(keyed_nonce(b"lootbox-open:v1", secret, acct as u64, open)) {
                    lootbox::Outcome::Crystal(x) => c += i64::from(x),
                    lootbox::Outcome::Coupons(x) => k += i64::from(x),
                }
            }
            (c, k)
        };
        let held = |pool: sqlx::PgPool| async move {
            let w: Option<(i64,)> = sqlx::query_as("SELECT bound FROM crystal_wallet WHERE account_id = $1")
                .bind(acct).fetch_optional(&pool).await.expect("wallet");
            let wl: (Option<i64>,) = sqlx::query_as(
                "SELECT sum(d_bound)::bigint FROM crystal_ledger WHERE account_id = $1 AND reason = 'lootbox'")
                .bind(acct).fetch_one(&pool).await.expect("ledger");
            let b: Option<(i64,)> = sqlx::query_as(
                "SELECT qty FROM bound_items WHERE account_id = $1 AND kind = $2")
                .bind(acct).bind(gacha::STANDARD_COUPON_KIND).fetch_optional(&pool).await.expect("coupons");
            let bl: (Option<i64>,) = sqlx::query_as(
                "SELECT sum(delta)::bigint FROM bound_ledger WHERE account_id = $1 AND kind = $2 AND reason = 'lootbox'")
                .bind(acct).bind(gacha::STANDARD_COUPON_KIND).fetch_one(&pool).await.expect("ledger");
            (w.map(|(q,)| q).unwrap_or(0), wl.0.unwrap_or(0), b.map(|(q,)| q).unwrap_or(0), bl.0.unwrap_or(0))
        };
        let pay = |pool: sqlx::PgPool, opened: u64| async move {
            let mut tx = pool.begin().await.expect("tx");
            lootbox_opens(&mut tx, secret, acct as u64, opened, 0).await.expect("lootbox_opens");
            tx.commit().await.expect("commit");
        };
        pay(pool.clone(), 0).await;
        assert_eq!(held(pool.clone()).await, (0, 0, 0, 0), "nothing opened, nothing paid");
        // Enough opens that at least one coupon tier is hit with this secret (checked below).
        let n = (1..=400u64).find(|n| predict(*n).1 > 0).expect("some open in 1..400 wins coupons");
        let (c, k) = predict(n);
        pay(pool.clone(), n).await;
        assert_eq!(held(pool.clone()).await, (c, c, k, k), "crystal and coupons, each with its ledger");
        pay(pool.clone(), n).await;
        assert_eq!(held(pool.clone()).await, (c, c, k, k), "the same save again pays nothing");
        pay(pool.clone(), n - 1).await;
        assert_eq!(held(pool.clone()).await, (c, c, k, k), "an older save never pays or claws back");
        // Exhaust the ×1 quota by hand: the next ×1 win pays the consolation instead.
        sqlx::query("INSERT INTO lootbox_quota (month, coupons, won) VALUES ('1970-01', 1, $1) \
                     ON CONFLICT (month, coupons) DO UPDATE SET won = EXCLUDED.won")
            .bind(lootbox::quota_of(1).unwrap() as i32).execute(&pool).await.expect("exhaust");
        let m = ((n + 1)..=(n + 2_000)).find(|m| {
            matches!(lootbox::roll(keyed_nonce(b"lootbox-open:v1", secret, acct as u64, *m)), lootbox::Outcome::Coupons(1))
        }).expect("a ×1 win within 2,000 opens");
        // Pay up to (but not including) m, then m alone, in two saves.
        // A save pays at most LOOTBOX_OPENS_PER_SAVE opens, so walk up to m − 1 in saves of that size.
        let before = held(pool.clone()).await;
        let mut upto = n;
        while upto < m - 1 {
            upto = (upto + LOOTBOX_OPENS_PER_SAVE).min(m - 1);
            pay(pool.clone(), upto).await;
        }
        let mid = held(pool.clone()).await;
        pay(pool.clone(), m).await;
        let after = held(pool.clone()).await;
        assert_eq!(after.2, mid.2, "the out-of-quota ×1 win pays no coupon");
        let consolation = match lootbox::CONSOLATION { lootbox::Outcome::Crystal(x) => i64::from(x), _ => 0 };
        assert_eq!(after.0 - mid.0, consolation, "it pays the consolation crystal");
        assert!(mid.0 >= before.0);
        let won: (i32,) = sqlx::query_as("SELECT won FROM lootbox_quota WHERE month = '1970-01' AND coupons = 1")
            .fetch_one(&pool).await.expect("quota row");
        assert_eq!(won.0, lootbox::quota_of(1).unwrap() as i32, "the quota did not move past its cap");
    }

    #[tokio::test]
    async fn pg_the_coupon_road_from_ladder_to_pull_stays_consistent() {
        let Some((pool, _g)) = setup().await else { return };
        let (acct, kind) = (8931i64, gacha::STANDARD_COUPON_KIND);
        for t in ["bound_items", "bound_ledger"] {
            sqlx::query(&format!("DELETE FROM {t} WHERE account_id = $1")).bind(acct)
                .execute(&pool).await.expect("clean");
        }
        let balance = |pool: sqlx::PgPool| async move {
            let b: Option<(i64,)> = sqlx::query_as(
                "SELECT qty FROM bound_items WHERE account_id = $1 AND kind = $2")
                .bind(acct).bind(kind).fetch_optional(&pool).await.expect("balance");
            let l: (Option<i64>,) = sqlx::query_as(
                "SELECT sum(delta)::bigint FROM bound_ledger WHERE account_id = $1 AND kind = $2")
                .bind(acct).bind(kind).fetch_one(&pool).await.expect("ledger");
            (b.map(|(q,)| q).unwrap_or(0), l.0.unwrap_or(0))
        };

        // 1. THE LADDER. `claim_collect`'s Grant::Bound arm, statement for statement.
        let credit = "INSERT INTO bound_items (account_id, kind, qty) VALUES ($1, $2, $3) \
                      ON CONFLICT (account_id, kind) DO UPDATE SET qty = bound_items.qty + EXCLUDED.qty";
        let log = "INSERT INTO bound_ledger (account_id, kind, delta, reason, ref) VALUES ($1, $2, $3, $4, $5)";
        for qty in [1i64, 1, 2, 1, 1, 1, 3] {   // the shipped newcomer ladder
            sqlx::query(credit).bind(acct).bind(kind).bind(qty).execute(&pool).await.expect("credit");
            sqlx::query(log).bind(acct).bind(kind).bind(qty).bind("claim").bind("newcomer")
                .execute(&pool).await.expect("log");
        }
        let (held, summed) = balance(pool.clone()).await;
        assert_eq!(held, 10, "the newcomer ladder did not credit ten coupons");
        assert_eq!(summed, held, "the ledger already disagrees with the balance");

        // 2. THE PULL, with enough coupons: no crystal, ten coupons gone.
        let have = held as u64;
        let (need, crystal_cost) = gacha::pull_price(10, have).expect("valid count");
        assert_eq!(crystal_cost, 0, "a player holding ten coupons was quoted crystal");
        let (bought, spent_logged) = gacha::coupon_movements(need, have);
        let debit = "UPDATE bound_items SET qty = GREATEST(qty - $3, 0) WHERE account_id = $1 AND kind = $2";
        sqlx::query(debit).bind(acct).bind(kind).bind(need.min(have) as i64)
            .execute(&pool).await.expect("debit");
        for (delta, reason) in [(bought as i64, "buy"), (-(spent_logged as i64), "spend")] {
            if delta == 0 { continue; }
            sqlx::query(log).bind(acct).bind(kind).bind(delta).bind(reason).bind("idem-1")
                .execute(&pool).await.expect("log");
        }
        let (held, summed) = balance(pool.clone()).await;
        assert_eq!(held, 0, "the pull did not consume the coupons");
        assert_eq!(summed, held, "THE LEDGER NO LONGER EXPLAINS THE BALANCE after a free pull");

        // 3. THE PULL WITH A SHORTFALL — the case that broke it. Two owned, ten wanted.
        for _ in 0..2 {
            sqlx::query(credit).bind(acct).bind(kind).bind(1i64).execute(&pool).await.expect("credit");
            sqlx::query(log).bind(acct).bind(kind).bind(1i64).bind("claim").bind("later")
                .execute(&pool).await.expect("log");
        }
        let have = 2u64;
        let (need, crystal_cost) = gacha::pull_price(10, have).expect("valid count");
        assert_eq!(crystal_cost, 8 * gacha::COUPON_PRICE_GEMS, "the shortfall was priced wrong");
        let (bought, spent_logged) = gacha::coupon_movements(need, have);
        sqlx::query(debit).bind(acct).bind(kind).bind(need.min(have) as i64)
            .execute(&pool).await.expect("debit");
        for (delta, reason) in [(bought as i64, "buy"), (-(spent_logged as i64), "spend")] {
            if delta == 0 { continue; }
            sqlx::query(log).bind(acct).bind(kind).bind(delta).bind(reason).bind("idem-2")
                .execute(&pool).await.expect("log");
        }
        let (held, summed) = balance(pool.clone()).await;
        assert_eq!(held, 0, "the shortfall pull left coupons behind");
        assert_eq!(summed, held,
            "THE LEDGER OVER-STATES THE BALANCE on a shortfall pull — this is the exact case that \
             logged +8 and -2 against a balance that fell by 2");

        for t in ["bound_items", "bound_ledger"] {
            sqlx::query(&format!("DELETE FROM {t} WHERE account_id = $1")).bind(acct)
                .execute(&pool).await.expect("clean up");
        }
    }

    #[tokio::test]
    async fn pg_the_coupon_balance_is_explained_by_its_ledger() {
        let Some((pool, _g)) = setup().await else { return };
        let (acct, kind) = (8921i64, gacha::STANDARD_COUPON_KIND);
        for t in ["bound_items", "bound_ledger"] {
            sqlx::query(&format!("DELETE FROM {t} WHERE account_id = $1")).bind(acct)
                .execute(&pool).await.expect("clean");
        }
        // The two movements a pull makes, written the way the handler writes them.
        let credit = "INSERT INTO bound_items (account_id, kind, qty) VALUES ($1, $2, $3) \
                      ON CONFLICT (account_id, kind) DO UPDATE SET qty = bound_items.qty + EXCLUDED.qty";
        let debit = "UPDATE bound_items SET qty = GREATEST(qty - $3, 0) WHERE account_id = $1 AND kind = $2";
        let log = "INSERT INTO bound_ledger (account_id, kind, delta, reason, ref) VALUES ($1, $2, $3, $4, 'probe')";

        for (q, reason) in [(10i64, "claim"), (3, "claim")] {
            sqlx::query(credit).bind(acct).bind(kind).bind(q).execute(&pool).await.expect("credit");
            sqlx::query(log).bind(acct).bind(kind).bind(q).bind(reason).execute(&pool).await.expect("log");
        }
        sqlx::query(debit).bind(acct).bind(kind).bind(10i64).execute(&pool).await.expect("debit");
        sqlx::query(log).bind(acct).bind(kind).bind(-10i64).bind("spend").execute(&pool).await.expect("log");

        let held: (i64,) = sqlx::query_as("SELECT qty FROM bound_items WHERE account_id = $1 AND kind = $2")
            .bind(acct).bind(kind).fetch_one(&pool).await.unwrap();
        // `sum()` over BIGINT is NUMERIC in Postgres; cast rather than decode it as i64.
        let summed: (Option<i64>,) = sqlx::query_as(
            "SELECT sum(delta)::bigint FROM bound_ledger WHERE account_id = $1 AND kind = $2")
            .bind(acct).bind(kind).fetch_one(&pool).await.unwrap();
        assert_eq!(held.0, 3, "the balance is not what the movements say");
        assert_eq!(summed.0, Some(3),
            "the ledger does not explain the balance — a coupon appeared or vanished unrecorded");

        for t in ["bound_items", "bound_ledger"] {
            sqlx::query(&format!("DELETE FROM {t} WHERE account_id = $1")).bind(acct)
                .execute(&pool).await.expect("clean up");
        }
    }

    #[tokio::test]
    async fn pg_two_pulls_on_one_idem_key_cannot_both_roll() {
        let Some((pool, _g)) = setup().await else { return };
        let (acct, key) = (8911i64, "probe-race-key");
        sqlx::query("DELETE FROM gacha_rolls WHERE idem_key = $1").bind(key)
            .execute(&pool).await.expect("clean");

        let claim = "INSERT INTO gacha_rolls (idem_key, account_id, banner_id, results_json, server_nonce) \
                     VALUES ($1, $2, $3, '[]', $4) ON CONFLICT DO NOTHING";
        let mut a = pool.begin().await.expect("tx a");
        let mut b = pool.begin().await.expect("tx b");
        let got_a = sqlx::query(claim).bind(key).bind(acct).bind("banner_char").bind(1i64)
            .execute(&mut *a).await.expect("claim a").rows_affected();
        assert_eq!(got_a, 1, "the first pull did not claim the key");
        a.commit().await.expect("commit a");

        // The second transaction, already open, now tries the same key.
        let got_b = sqlx::query(claim).bind(key).bind(acct).bind("banner_char").bind(2i64)
            .execute(&mut *b).await.expect("claim b").rows_affected();
        assert_eq!(got_b, 0,
            "BOTH pulls claimed the same key — the player is charged twice, pity advances twice, \
             and the second roll overwrites results the first already showed them");
        b.commit().await.expect("commit b");

        // One row, and it is the FIRST one's nonce — the loser must not have rewritten it.
        let rows: (i64,) = sqlx::query_as("SELECT count(*) FROM gacha_rolls WHERE idem_key = $1")
            .bind(key).fetch_one(&pool).await.unwrap();
        assert_eq!(rows.0, 1);
        let nonce: (i64,) = sqlx::query_as("SELECT server_nonce FROM gacha_rolls WHERE idem_key = $1")
            .bind(key).fetch_one(&pool).await.unwrap();
        assert_eq!(nonce.0, 1, "the losing request overwrote the winner's roll seed");

        sqlx::query("DELETE FROM gacha_rolls WHERE idem_key = $1").bind(key)
            .execute(&pool).await.expect("clean up");
    }

    #[tokio::test]
    async fn pg_a_pull_debit_is_a_delta_the_database_applies() {
        let Some((pool, _g)) = setup().await else { return };
        let acct = 8901i64;
        sqlx::query("DELETE FROM crystal_wallet WHERE account_id = $1").bind(acct)
            .execute(&pool).await.expect("clean");
        sqlx::query("INSERT INTO crystal_wallet (account_id, paid, bound) VALUES ($1, 1000, 500)")
            .bind(acct).execute(&pool).await.expect("seed");

        // The statement `gacha_roll` runs, with a spend it can afford.
        let sql = "UPDATE crystal_wallet SET paid = paid - $2, bound = bound - $3 \
                   WHERE account_id = $1 AND paid >= $2 AND bound >= $3";
        let n = sqlx::query(sql).bind(acct).bind(300i64).bind(0i64)
            .execute(&pool).await.expect("debit").rows_affected();
        assert_eq!(n, 1, "the debit did not apply");
        let w: (i64, i64) = sqlx::query_as("SELECT paid, bound FROM crystal_wallet WHERE account_id = $1")
            .bind(acct).fetch_one(&pool).await.unwrap();
        assert_eq!(w, (700, 500), "crystal did not actually leave the wallet");

        // A CREDIT LANDING MID-PULL survives, which an absolute write would have erased.
        sqlx::query("UPDATE crystal_wallet SET paid = paid + 5000 WHERE account_id = $1")
            .bind(acct).execute(&pool).await.expect("credit");
        sqlx::query(sql).bind(acct).bind(300i64).bind(0i64).execute(&pool).await.expect("debit 2");
        let w2: (i64, i64) = sqlx::query_as("SELECT paid, bound FROM crystal_wallet WHERE account_id = $1")
            .bind(acct).fetch_one(&pool).await.unwrap();
        assert_eq!(w2.0, 5400, "a concurrent credit was erased by the pull's write");

        // AND IT CANNOT GO NEGATIVE: an unaffordable debit matches zero rows, which is what makes
        // the roll fail instead of being free.
        let n2 = sqlx::query(sql).bind(acct).bind(999_999i64).bind(0i64)
            .execute(&pool).await.expect("debit 3").rows_affected();
        assert_eq!(n2, 0, "an unaffordable pull was charged anyway");
        let w3: (i64, i64) = sqlx::query_as("SELECT paid, bound FROM crystal_wallet WHERE account_id = $1")
            .bind(acct).fetch_one(&pool).await.unwrap();
        assert_eq!(w3.0, 5400, "the wallet moved on a debit that should have matched nothing");

        sqlx::query("DELETE FROM crystal_wallet WHERE account_id = $1").bind(acct)
            .execute(&pool).await.expect("clean up");
    }

    /// The status word is the whole decision, so it is worth pinning what each one MEANS rather
    /// than trusting a `matches!` to be read correctly later. `partialrefund` being false is the
    /// load-bearing case: it is the one that looks like a refund and must not be treated as one.
    #[test]
    fn a_partial_refund_is_not_a_revocation() {
        for yes in ["Refunded", "refunded", "CHARGEBACK", "Reversed"] {
            assert!(refund_means_revoke(yes), "{yes} should revoke");
        }
        for no in ["PartialRefund", "partialrefund", "Succeeded", "Init", "Approved", "", "Failed"] {
            assert!(!refund_means_revoke(no), "{no} must NOT revoke");
        }
    }

    /// BOTH CALLERS MAY RACE. An operator can hit the dev endpoint while the hourly sweep is
    /// asking Steam about the same order; whichever loses must report success and take nothing,
    /// or the wallet is debited twice for one refund. `Ok(false)` is that contract, and this is
    /// the test that stops it being "refactored" into an error.
    #[tokio::test]
    async fn pg_revoking_the_same_order_twice_takes_the_money_once() {
        let Some((db, _g)) = setup().await else { return };
        let acct = 8802i64;
        sqlx::query("INSERT INTO accounts (account_id, name, seed) VALUES ($1, $2, 7) ON CONFLICT DO NOTHING")
            .bind(acct).bind(format!("probe-{acct}")).execute(&db).await.ok();
        sqlx::query("DELETE FROM store_orders WHERE order_id = 909091").execute(&db).await.ok();
        sqlx::query("INSERT INTO crystal_wallet (account_id, paid, bound) VALUES ($1, 500, 0) \
                     ON CONFLICT (account_id) DO UPDATE SET paid = 500, bound = 0")
            .bind(acct).execute(&db).await.expect("wallet");
        sqlx::query(
            "INSERT INTO store_orders (order_id, account_id, sku, idem_key, price_cents, transid, \
             status, granted_paid, granted_bound) \
             VALUES (909091, $1, 'crystal_small', 'revoke-twice-key', 499, 'T1', 'granted', 500, 0)",
        ).bind(acct).execute(&db).await.expect("order");

        assert_eq!(revoke_order(&db, 909091, acct, 500, 0).await, Ok(true), "first revoke acts");
        assert_eq!(revoke_order(&db, 909091, acct, 500, 0).await, Ok(false), "second is a no-op");

        let (t, b): (i64, i64) = sqlx::query_as("SELECT paid, bound FROM crystal_wallet WHERE account_id = $1")
            .bind(acct).fetch_one(&db).await.expect("wallet back");
        assert_eq!((t, b), (0, 0), "the wallet gave back 500 once, not twice");
    }

    #[tokio::test]
    async fn pg_a_refund_takes_the_capability_back() {
        let Some((pool, _g)) = setup().await else { return };
        let (acct, order, sku) = (8801i64, 88_010i64, "sku_probe_capability");
        sqlx::query("DELETE FROM entitlements WHERE account_id = $1").bind(acct)
            .execute(&pool).await.expect("clean");
        sqlx::query("INSERT INTO accounts (account_id, name, seed) VALUES ($1, $2, 7) ON CONFLICT DO NOTHING")
            .bind(acct).bind(format!("probe-{acct}")).execute(&pool).await.ok();

        assert!(!owns_sku(&pool, acct as u64, sku).await.expect("query"), "owned before it was ever bought");
        sqlx::query("INSERT INTO entitlements (account_id, sku, order_id) VALUES ($1, $2, $3)")
            .bind(acct).bind(sku).bind(order).execute(&pool).await.expect("grant");
        assert!(owns_sku(&pool, acct as u64, sku).await.expect("query"), "a granted entitlement is not owned");

        // The refund path, exactly as `store_refund` writes it.
        sqlx::query("UPDATE entitlements SET revoked_at = now() WHERE order_id = $1 AND revoked_at IS NULL")
            .bind(order).execute(&pool).await.expect("revoke");
        assert!(!owns_sku(&pool, acct as u64, sku).await.expect("query"),
            "THE REFUNDED PLAYER STILL OWNS THE CAPABILITY — the row is revoked and the check \
             cannot see it");
        // The row is still there: revocation is an audit trail, not a delete.
        let rows: (i64,) = sqlx::query_as("SELECT count(*) FROM entitlements WHERE order_id = $1")
            .bind(order).fetch_one(&pool).await.unwrap();
        assert_eq!(rows.0, 1, "revocation deleted the audit trail instead of marking it");

        // Buying again after a refund is a FIRST buy again, which is what the bonus intends.
        sqlx::query("DELETE FROM entitlements WHERE account_id = $1").bind(acct)
            .execute(&pool).await.expect("clean up");
    }

    /// Drive the REAL router the way the game client does — the thing nothing in this crate did.
    ///
    /// Most tests here call internal functions directly. That is the right way to test a ledger,
    /// but it leaves the handler wiring — which checks run, in which order, on which request —
    /// held by source scans, and source scans in this file have been defeated repeatedly (a
    /// COMMENT satisfied one, a test's own text another, a hand-typed list was simply short).
    ///
    /// `oneshot` runs the router in-process: no socket, no port, no ordering with other tests.
    async fn call(app: &axum::Router, req: axum::http::Request<axum::body::Body>) -> (StatusCode, String) {
        use tower::ServiceExt;
        let res = app.clone().oneshot(req).await.expect("router responded");
        let status = res.status();
        let bytes = http_body_util::BodyExt::collect(res.into_body()).await.expect("body").to_bytes();
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    fn post_json(path: &str, token: &str, body: String) -> axum::http::Request<axum::body::Body> {
        axum::http::Request::builder()
            .method("POST").uri(path)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(axum::body::Body::from(body)).expect("request")
    }

    fn get_req(path: &str) -> axum::http::Request<axum::body::Body> {
        axum::http::Request::builder()
            .method("GET").uri(path)
            .body(axum::body::Body::empty()).expect("request")
    }

    /// PROD=1 serves the OPS routes behind ADMIN_KEY and does not serve the test fixtures at all;
    /// a dev server keeps both, and still takes the dev key on the ops routes.
    #[tokio::test]
    async fn prod_mode_unmounts_fixtures_and_gates_ops_on_the_admin_key() {
        let req = |path: &str, header: Option<(&str, &str)>| {
            let mut b = axum::http::Request::builder().method("GET").uri(path);
            if let Some((k, v)) = header {
                b = b.header(k, v);
            }
            b.body(axum::body::Body::empty()).expect("request")
        };
        let app_for = |st: AppState| build_router(st.clone(), gated_routes(&st));

        let mut prod = test_state(None);
        prod.prod = true;
        prod.admin_key = Some("adm".into());
        prod.dev_key = None;
        let app = app_for(prod);
        assert_eq!(call(&app, req("/flags", Some(("x-admin-key", "adm")))).await.0, StatusCode::OK);
        assert_eq!(call(&app, req("/flags", Some(("x-admin-key", "nope")))).await.0, StatusCode::FORBIDDEN);
        assert_eq!(call(&app, req("/flags", Some(("x-dev-key", "adm")))).await.0, StatusCode::FORBIDDEN, "no dev key in prod");
        let (code, _) = call(&app, req("/demo/bundle", Some(("x-dev-key", "anything")))).await;
        assert_eq!(code, StatusCode::NOT_FOUND, "a fixture route is ABSENT in prod, not merely gated");
        assert_eq!(call(&app, axum::http::Request::builder().method("POST").uri("/auth/dev")
            .header("content-type", "application/json").body(axum::body::Body::from("{}")).unwrap()).await.0,
            StatusCode::NOT_FOUND, "no dev login in prod");

        let mut prod_no_admin = test_state(None);
        prod_no_admin.prod = true;
        assert_eq!(call(&app_for(prod_no_admin), req("/flags", None)).await.0, StatusCode::SERVICE_UNAVAILABLE, "fails closed");

        let mut dev = test_state(None);
        dev.dev_key = Some("dk".into());
        let app = app_for(dev);
        assert_eq!(call(&app, req("/flags", Some(("x-dev-key", "dk")))).await.0, StatusCode::OK, "dev tools keep one key");
        assert_ne!(call(&app, req("/demo/bundle", Some(("x-dev-key", "dk")))).await.0, StatusCode::NOT_FOUND, "fixtures served outside prod");
    }

    /// Real money never runs outside production mode: live + no PROD, or live + a leftover
    /// DEV_KEY, refuses to boot; sandbox runs anywhere.
    #[test]
    fn real_money_requires_production_mode() {
        assert!(live_money_misconfig(false, false, true).is_none(), "sandbox: anything goes");
        assert!(live_money_misconfig(true, false, false).is_some_and(|w| w.contains("PROD=1")));
        assert!(live_money_misconfig(true, true, true).is_some_and(|w| w.contains("DEV_KEY")));
        assert!(live_money_misconfig(true, true, false).is_none(), "the launch configuration boots");
    }


    /// The Steam item icons are served publicly (itemdef `icon_url` points here); others are 404.
    #[tokio::test]
    async fn steam_icons_are_served_and_unknown_names_are_404() {
        let app = build_router(test_state(None), Router::new());
        let (code, _) = call(&app, get_req("/steam/icon/farm_box.png")).await;
        assert_eq!(code, StatusCode::OK);
        let (code, _) = call(&app, get_req("/steam/icon/farm_box_large.png")).await;
        assert_eq!(code, StatusCode::OK);
        let (code, _) = call(&app, get_req("/steam/icon/../../etc/passwd")).await;
        assert_eq!(code, StatusCode::NOT_FOUND);
    }

    /// The global in-flight cap sheds with 503 once saturated, and never sheds /health.
    ///
    /// Proven by CALLING the real router: a source scan can't tell a live 503 from a dead
    /// constant. Saturate the shared `inflight` semaphore, then require that an ordinary
    /// request is shed while Fly's health probe still gets through, and that freeing a permit
    /// re-admits the next request. Needs no DB — the paths exercised are public.
    #[tokio::test]
    async fn the_in_flight_cap_sheds_with_503_and_exempts_health() {
        let st = test_state(None);
        let app = build_router(st.clone(), Router::new());

        // Hold every permit → the machine is at capacity.
        let held = st.inflight.clone().try_acquire_many_owned(MAX_INFLIGHT as u32).expect("permits free");

        let (code, body) = call(&app, get_req("/version")).await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "a saturated server must shed, got: {body}");

        // Fly's probe is exempt — a health check on a busy machine must not read as down.
        let (code, _) = call(&app, get_req("/health")).await;
        assert_eq!(code, StatusCode::OK, "/health must never be shed");

        // Releasing capacity re-admits the next request.
        drop(held);
        let (code, _) = call(&app, get_req("/version")).await;
        assert_ne!(code, StatusCode::SERVICE_UNAVAILABLE, "a freed permit must admit the next request");
    }

    /// An account the server will accept, with the token to speak as it.
    ///
    /// `seed` matters: the bundle's `start` is only trusted if it is a genesis the server can
    /// REPRODUCE from this account's server-chosen seed (`genesis_candidates`), which is the check
    /// that stops a forged god-character start with an empty log.
    fn seed_session(st: &AppState, account_id: u64, seed: u64) -> String {
        let token = format!("test-token-{account_id}");
        let acct = Account { account_id, name: format!("tester{account_id}"), seed };
        st.sessions.by_token.held().insert(token.clone(), acct.clone());
        st.sessions.by_identity.held().insert(acct.name.clone(), acct);
        st.sessions.active.held().insert(account_id, token.clone());
        token
    }

    /// A bundle that actually PASSES replay verification — the expensive half of this harness.
    ///
    /// `SaveBundle::capture` cannot be used: it reads the log back out of `live.input_log`, and
    /// `replay()` sets `record_inputs = false`, so a replayed state carries an EMPTY log and the
    /// bundle would verify against the wrong thing. Built to mirror `verified_state` exactly
    /// instead — same base, same `snapshot_interval`, same replay — so the hash matches by
    /// construction rather than by luck.
    fn verifiable_bundle(
        data: &GameData,
        seed: u64,
        ticks: u64,
        log: Vec<sim_core::events::RecordedInput>,
    ) -> SaveBundle {
        let start = data::new_game_in_zone(seed, data, START_ZONE).expect("genesis");
        let mut base = start.clone();
        base.snapshot_interval = 0;
        base.snapshots.clear();
        let live = sim_core::replay::replay(&base, data, &log, ticks);
        SaveBundle {
            sim_version: sim_core::VERSION.into(),
            game_data_version: data.version.clone(),
            start,
            log,
            final_tick: ticks,
            final_hash: hash::state_hash(&live),
            snapshot_interval: 0,
            snapshots: live.snapshots.clone(),
            sim_build: String::new(),
        }
    }

    /// The harness proves itself first: an honest save is ACCEPTED end to end.
    ///
    /// Without this, every rejection test below would pass on a harness that rejects everything —
    /// which is the "green because nothing ran" shape this file has been bitten by repeatedly.
    #[tokio::test]
    async fn pg_an_honest_save_is_accepted_through_the_real_router() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let app = build_router(st.clone(), Router::new());
        let token = seed_session(&st, 4242, 4242);
        let bundle = verifiable_bundle(&st.data, 4242, 40, Vec::new());

        let (code, body) = call(&app, post_json("/save", &token, serde_json::to_string(&bundle).unwrap())).await;
        assert_eq!(code, StatusCode::OK, "the router rejected an honest save: {body}");
        assert!(body.contains("\"ok\":true"),
            "an honestly replayed save was refused — every rejection test in this harness would \
             then pass for the wrong reason: {body}");

        let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM saves WHERE account_id = 4242")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(n, 1, "and it reached the database");
    }

    /// A verifiable bundle from any trusted `start` (the next segment after a confirmed save),
    /// built the way `verified_state` replays it. Returns the bundle and the state it ends in.
    fn bundle_from(data: &GameData, start: sim_core::state::GameState, final_tick: u64) -> (SaveBundle, sim_core::state::GameState) {
        let mut base = start.clone();
        base.snapshot_interval = 0;
        base.snapshots.clear();
        let live = sim_core::replay::replay(&base, data, &[], final_tick);
        (SaveBundle {
            sim_version: sim_core::VERSION.into(),
            game_data_version: data.version.clone(),
            start,
            log: Vec::new(),
            final_tick,
            final_hash: hash::state_hash(&live),
            snapshot_interval: 0,
            snapshots: live.snapshots.clone(),
            sim_build: String::new(),
        }, live)
    }

    /// AUDIT 2026-10-02 (peek-and-skip): an issued fusion roll commits its attempt to its six.
    #[tokio::test]
    async fn pg_a_fusion_roll_commits_its_attempt() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let token = seed_session(&st, 4444, 4444);
        let auth = || {
            let mut h = HeaderMap::new();
            h.insert("authorization", format!("Bearer {token}").parse().unwrap());
            h
        };
        let roll = |key: &str, attempt: u64| fuse_roll(State(st.clone()), auth(), Json(FuseRollReq { key: key.into(), attempt }));
        let (code, Json(r)) = roll("six-a", 5).await;
        assert_eq!((code, r.issued), (StatusCode::OK, true));
        let (code, Json(r)) = roll("six-a", 5).await;
        assert_eq!((code, r.reason.as_str()), (StatusCode::OK, "idempotent"), "the same six again");
        let (code, Json(r)) = roll("six-b", 5).await;
        assert_eq!(code, StatusCode::CONFLICT, "other six for the same attempt: {}", r.reason);
        let (code, Json(r)) = roll("six-c", 6).await;
        assert_eq!(code, StatusCode::CONFLICT, "a later attempt while attempt 5 is unused: {}", r.reason);
    }

    /// AUDIT 2026-10-02 (critical): pruning a claim op the server never settled erased the only
    /// record of the claim, so a mail could be claimed again and again. Such a bundle is refused.
    #[tokio::test]
    async fn pg_pruning_an_unsettled_claim_is_refused() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let app = build_router(st.clone(), Router::new());
        let token = seed_session(&st, 4646, 4646);
        let mail = mailbox_store::send(&pool, 4646, MailKind::Reward, 500, "", 0, "t").await.unwrap();
        let log = vec![
            sim_core::events::RecordedInput { tick: 1, event: InputEvent::ClaimMail { mail_id: mail, gil: sim_core::state::Gil::new(500), def_id: String::new(), qty: 0, item: None } },
            sim_core::events::RecordedInput { tick: 2, event: InputEvent::PruneMarketOps { ids: vec![1] } },
        ];
        let b = verifiable_bundle(&st.data, 4646, 40, log);
        let (code, body) = call(&app, post_json("/save", &token, serde_json::to_string(&b).unwrap())).await;
        assert!(body.contains("never settled"), "{code} {body}");
        assert!(claimed_at(&pool, mail).await.is_none(), "nothing consumed");
    }

    /// AUDIT 2026-10-02: a refunded Battle Pass takes back the currency its premium slots paid —
    /// crystal into debt, tickets as far as they are held.
    #[tokio::test]
    async fn pg_a_refunded_battle_pass_takes_back_its_premium_currency() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let token = seed_session(&st, 4747, 4747);
        let auth = || {
            let mut h = HeaderMap::new();
            h.insert("authorization", format!("Bearer {token}").parse().unwrap());
            h
        };
        let cfg = bp::default_config();
        let (sid,): (i64,) = sqlx::query_as("INSERT INTO seasons (starts_at_ms, ends_at_ms, seed, config_json) VALUES ($1, $2, 1, $3) RETURNING season_id")
            .bind(now_ms() - 1000).bind(now_ms() + 86_400_000).bind(serde_json::to_string(&cfg).unwrap()).fetch_one(&pool).await.unwrap();
        *st.season.held() = Some(bp::Season { season_id: sid, starts_at_ms: now_ms() - 1000, ends_at_ms: now_ms() + 86_400_000, seed: 1, config: cfg });
        sqlx::query("INSERT INTO bp_state (account_id, season_id, xp) VALUES (4747, $1, 5000)").bind(sid).execute(&pool).await.unwrap();
        sku_order(&pool, 7601, 4747, "battle_pass", Some(sid)).await;
        assert!(matches!(grant_order(&pool, &st.data, 7601).await, Ok(GrantOutcome::Granted { .. })));
        // Slot 4 = 100 bound crystal, slot 10 = 2 Limited tickets.
        let (code, _) = bp_claim(State(st.clone()), auth(), Json(BpClaimReq { slots: vec![4, 10], track: 1 })).await;
        assert_eq!(code, StatusCode::OK);
        sqlx::query("UPDATE crystal_wallet SET bound = 30 WHERE account_id = 4747").execute(&pool).await.unwrap();
        assert_eq!(revoke_order(&pool, 7601, 4747, 0, 0).await, Ok(true));
        assert_eq!(wallet(&pool, 4747).await, (0, -70), "100 taken back from 30: in debt");
        let (lim,): (i64,) = sqlx::query_as("SELECT qty FROM bound_items WHERE account_id = 4747 AND kind = $1")
            .bind(gacha::LIMITED_COUPON_KIND).fetch_one(&pool).await.unwrap();
        assert_eq!(lim, 0, "the two tickets are taken back");
    }

    /// REGRESSION GUARD (audit 2026-10-02): after a catalog change the client resumes from
    /// `migrate(stored)`, whose hash is not the stored one — that start must still be accepted,
    /// not read as a rewind to an earlier save.
    #[tokio::test]
    async fn pg_a_resume_from_the_migrated_save_is_accepted() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let app = build_router(st.clone(), Router::new());
        let token = seed_session(&st, 4545, 4545);
        let genesis = data::new_game_in_zone(4545, &st.data, START_ZONE).expect("genesis");
        let (b1, mut s1) = bundle_from(&st.data, genesis, 40);
        let post = |b: &SaveBundle| post_json("/save", &token, serde_json::to_string(b).unwrap());
        let (code, body) = call(&app, post(&b1)).await;
        assert!(code == StatusCode::OK && body.contains("\"ok\":true"), "first save: {body}");
        // Pretend the stored save predates a catalog change: it still holds a retired item, so
        // migrating it changes its hash. Store it the way the server would hold it.
        s1.inventory.push(sim_core::items::Item::new("meal_feast", sim_core::items::ItemKind::Consumable, 0, 0, sim_core::items::Rarity::Rare));
        let old_hash = hash::state_hash(&s1);
        sqlx::query("UPDATE saves SET state_json = $2, final_hash = $3 WHERE account_id = $1")
            .bind(4545i64).bind(serde_json::to_string(&s1).unwrap()).bind(&old_hash).execute(&pool).await.unwrap();
        st.saves.held().insert(4545, StoredSave { state_json: serde_json::to_string(&s1).unwrap(), final_tick: 40, final_hash: old_hash.clone(), saved_at_ms: now_ms() });
        st.trusted.held().entry(4545).or_default().insert(old_hash);
        // The client resumes from what GET /save serves: the migrated state.
        let (code, body) = call(&app, axum::http::Request::builder().uri("/save")
            .header("authorization", format!("Bearer {token}")).body(axum::body::Body::empty()).unwrap()).await;
        assert_eq!(code, StatusCode::OK, "{body}");
        let mut migrated = s1.clone();
        data::migrate_save_items(&mut migrated, &st.data);
        assert_ne!(hash::state_hash(&migrated), hash::state_hash(&s1), "the fixture must actually migrate");
        let (b2, _) = bundle_from(&st.data, migrated, 80);
        let (code, body) = call(&app, post(&b2)).await;
        assert!(code == StatusCode::OK && body.contains("\"ok\":true"), "the migrated resume must save: {body}");
    }

    /// The save row moves only if it still holds the save the bundle was checked against.
    #[tokio::test]
    async fn pg_a_save_write_is_compare_and_set_on_the_latest() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let none = mailbox::ClaimSettlement::default();
        assert!(persist_save_and_claims(&pool, 46, "{}", 1, "hA", &none, None, &[], &AcceptedExtras::default()).await.unwrap(), "first write");
        assert!(!persist_save_and_claims(&pool, 46, "{}", 2, "hB", &none, None, &[], &AcceptedExtras::default()).await.unwrap(), "a second first-write loses");
        assert!(!persist_save_and_claims(&pool, 46, "{}", 2, "hB", &none, Some("hX"), &[], &AcceptedExtras::default()).await.unwrap(), "a stale base loses");
        assert!(persist_save_and_claims(&pool, 46, "{}", 2, "hB", &none, Some("hA"), &["hA"], &AcceptedExtras::default()).await.unwrap(), "the current base wins");
    }

    /// AUDIT 2026-10-02: once an account has saved, only its LATEST save is a base. Re-sending the
    /// bundle that produced it is an idempotent ok; any older base is refused (not flagged).
    #[tokio::test]
    async fn pg_only_the_latest_save_is_a_base_and_a_resend_is_idempotent() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let app = build_router(st.clone(), Router::new());
        let token = seed_session(&st, 4343, 4343);
        let genesis = data::new_game_in_zone(4343, &st.data, START_ZONE).expect("genesis");
        let (b1, s1) = bundle_from(&st.data, genesis, 40);
        let post = |b: &SaveBundle| post_json("/save", &token, serde_json::to_string(b).unwrap());
        let (code, body) = call(&app, post(&b1)).await;
        assert!(code == StatusCode::OK && body.contains("\"ok\":true"), "first save: {body}");
        let (b2, _) = bundle_from(&st.data, s1, 80);
        let (code, body) = call(&app, post(&b2)).await;
        assert!(code == StatusCode::OK && body.contains("\"ok\":true"), "second save: {body}");

        let (code, body) = call(&app, post(&b2)).await;
        assert!(code == StatusCode::OK && body.contains("idempotent"), "the same bundle again: {body}");
        // Genesis is two saves back: no longer trusted at all (only {latest, its base} are kept),
        // so it reads as an unknown base — refused and flagged; no honest client sends it.
        let (code, body) = call(&app, post(&b1)).await;
        assert_eq!(code, StatusCode::CONFLICT, "a rewind to the earlier base: {body}");
        let (rows,): (i64,) = sqlx::query_as("SELECT count(*) FROM trusted WHERE account_id = 4343")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(rows, 2, "trusted holds only the latest save and its base");
    }

    /// AUDIT 2026-10-03: a save landed but its reply was lost; the client kept its old base and
    /// played on. A bundle from that base that passes through the latest save is a continuation and
    /// is accepted; one that took another path before it is still refused.
    #[tokio::test]
    async fn pg_a_continuation_from_the_previous_base_is_accepted_and_a_rewind_is_not() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let app = build_router(st.clone(), Router::new());
        let token = seed_session(&st, 4344, 4344);
        let genesis = data::new_game_in_zone(4344, &st.data, START_ZONE).expect("genesis");
        let post = |b: &SaveBundle| post_json("/save", &token, serde_json::to_string(b).unwrap());
        let (b1, _) = bundle_from(&st.data, genesis.clone(), 40);
        let (code, body) = call(&app, post(&b1)).await;
        assert!(code == StatusCode::OK && body.contains("\"ok\":true"), "first save: {body}");
        // Rewind: same base, a different input before tick 40.
        let log = vec![sim_core::events::RecordedInput { tick: 10, event: sim_core::events::InputEvent::SetFarmMode { on: true } }];
        let alt = sim_core::replay::replay(&genesis, &st.data, &log, 60);
        let rewind = SaveBundle { log, final_tick: 60, final_hash: hash::state_hash(&alt), ..b1.clone() };
        let (code, body) = call(&app, post(&rewind)).await;
        assert_eq!(code, StatusCode::CONFLICT, "another path from the old base: {body}");
        // Continuation: the same path, played further.
        let (c, s2) = bundle_from(&st.data, genesis, 80);
        let (code, body) = call(&app, post(&c)).await;
        assert!(code == StatusCode::OK && body.contains("\"ok\":true") && !body.contains("idempotent"), "continuation: {body}");
        assert_eq!(st.saves.held().get(&4344).map(|s| s.final_hash.clone()), Some(hash::state_hash(&s2)));
        // Its reply lost too: the same bundle again is a re-send, not a forgery.
        let (code, body) = call(&app, post(&c)).await;
        assert!(code == StatusCode::OK && body.contains("idempotent"), "continuation re-sent: {body}");
        let (flags,): (i64,) = sqlx::query_as("SELECT count(*) FROM cheat_flags WHERE account_id = 4344")
            .fetch_one(&pool).await.unwrap_or((0,));
        assert_eq!(flags, 0, "nothing here is flagged");
    }

    /// OFFLINE EXP is paid AT THE LOGIN (GET /save), once per absence: a save without a login owes
    /// nothing however long since the last one (an outage the client played through); a second
    /// login right after pays only the seconds between them; a login while a save of the account
    /// is running is refused.
    #[tokio::test]
    async fn pg_offline_time_is_paid_at_the_login_once() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let app = build_router(st.clone(), Router::new());
        let token = seed_session(&st, 4346, 4346);
        let post = |b: &SaveBundle| post_json("/save", &token, serde_json::to_string(b).unwrap());
        let get = || axum::http::Request::builder().uri("/save")
            .header("authorization", format!("Bearer {token}")).body(axum::body::Body::empty()).unwrap();
        let envelopes = || async {
            sqlx::query_as::<_, (String,)>("SELECT reason FROM mailbox WHERE account_id = 4346 AND reason LIKE 'offline %' ORDER BY id")
                .fetch_all(&pool).await.unwrap().into_iter().map(|(r,)| r).collect::<Vec<_>>()
        };
        let backdate = |hours: i64| {
            if let Some(s) = st.saves.held().get_mut(&4346) { s.saved_at_ms -= hours * 3_600_000; }
        };
        let genesis = data::new_game_in_zone(4346, &st.data, START_ZONE).expect("genesis");
        let (b1, s1) = bundle_from(&st.data, genesis, 40);
        assert!(call(&app, post(&b1)).await.1.contains("\"ok\":true"));
        // Two hours "since the last save", no login: nothing.
        backdate(2);
        let (b2, _) = bundle_from(&st.data, s1, 80);
        assert!(call(&app, post(&b2)).await.1.contains("\"ok\":true"));
        assert!(envelopes().await.is_empty(), "no login, no offline time");
        // A login two hours after the last save pays those two hours, at the login (the payout
        // reads the stored-at from the row as well as memory, so both are moved back).
        backdate(2);
        sqlx::query("UPDATE saves SET saved_at_ms = saved_at_ms - 7200000 WHERE account_id = 4346").execute(&pool).await.unwrap();
        let (code, body) = call(&app, get()).await;
        assert_eq!(code, StatusCode::OK, "{body}");
        let got = envelopes().await;
        assert!(got == ["offline 119m"] || got == ["offline 120m"], "the two hours: {got:?}");
        // Logging in again at once pays nothing more.
        let (code, _) = call(&app, get()).await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(envelopes().await.len(), 1, "paid once");
        // A login while a save of the account is running waits for it.
        let _held = SaveSlot::take(&st.saving, 4346).expect("free");
        let (code, _) = call(&app, get()).await;
        assert_eq!(code, StatusCode::TOO_MANY_REQUESTS);
    }

    /// MINT COUPONS (user 2026-10-03): 1 per 4 h of real time up to 5 uncollected; a claim fills
    /// the held balance to 10 and leaves the rest uncollected; a mint spends one, a refusal refunds.
    #[tokio::test]
    async fn pg_mint_coupons_accrue_claim_to_the_cap_and_pay_for_a_mint() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let token = seed_session(&st, 9610, 9610);
        let h = || {
            let mut h = HeaderMap::new();
            h.insert("authorization", format!("Bearer {token}").parse().unwrap());
            h
        };
        let hour = 3_600_000i64;
        sqlx::query("INSERT INTO mint_coupon_clock (account_id, anchor_ms) VALUES (9610, $1)").bind(now_ms() - 30 * hour)
            .execute(&pool).await.unwrap();
        let (_, Json(v)) = mint_coupons_state(State(st.clone()), h()).await;
        assert_eq!((v["held"].as_u64(), v["pending"].as_u64(), v["room"].as_u64()), (Some(0), Some(5), Some(5)), "capped at 5: {v}");
        let (_, Json(v)) = mint_coupons_claim(State(st.clone()), h()).await;
        assert_eq!((v["claimed"].as_u64(), v["held"].as_u64(), v["pending"].as_u64()), (Some(5), Some(5), Some(0)), "{v}");
        // Five more accrue; only five fit under ten... then two more with eight held.
        sqlx::query("UPDATE bound_items SET qty = 8 WHERE account_id = 9610 AND kind = $1").bind(mint_coupon::KIND).execute(&pool).await.unwrap();
        sqlx::query("UPDATE mint_coupon_clock SET anchor_ms = $1 WHERE account_id = 9610").bind(now_ms() - 20 * hour).execute(&pool).await.unwrap();
        let (_, Json(v)) = mint_coupons_state(State(st.clone()), h()).await;
        assert_eq!(v["room"].as_u64(), Some(2), "the client can warn before claiming: {v}");
        let (_, Json(v)) = mint_coupons_claim(State(st.clone()), h()).await;
        assert_eq!((v["claimed"].as_u64(), v["held"].as_u64(), v["pending"].as_u64()), (Some(2), Some(10), Some(3)), "the rest stays uncollected: {v}");
        // A mint spends one; a refusal gives it back; with none held, nothing is spent.
        assert!(spend_mint_coupon(&pool, 9610, "9610/x/1").await);
        refund_mint_coupon(&pool, 9610, "9610/x/1").await;
        let (q,): (i64,) = sqlx::query_as("SELECT qty FROM bound_items WHERE account_id = 9610 AND kind = $1").bind(mint_coupon::KIND).fetch_one(&pool).await.unwrap();
        assert_eq!(q, 10);
        sqlx::query("UPDATE bound_items SET qty = 0 WHERE account_id = 9610 AND kind = $1").bind(mint_coupon::KIND).execute(&pool).await.unwrap();
        assert!(!spend_mint_coupon(&pool, 9610, "9610/x/2").await, "no coupon, no mint");
    }

    /// STEAM IMPORT: the item id is claimed once, a consumed import is mailed once and tradeable,
    /// and the endpoints refuse while Steam inventory is switched off.
    #[tokio::test]
    async fn pg_a_steam_import_is_claimed_once_and_delivered_once_tradeable() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let token = seed_session(&st, 9620, 9620);
        let mut h = HeaderMap::new();
        h.insert("authorization", format!("Bearer {token}").parse().unwrap());
        if std::env::var("STEAM_INVENTORY_LIVE").is_err() {
            let (code, _) = steam_inventory(State(st.clone()), h.clone()).await;
            assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "off: never calls Steam");
        }
        let def = st.data.items.iter().find(|d| d.steam_itemdef.is_some()).expect("a minted def").id.clone();
        assert_eq!(def_of_itemdef(&st.data, i64::from(st.data.item(&def).unwrap().steam_itemdef.unwrap())).map(|d| d.id.clone()), Some(def.clone()));
        let claim = || sqlx::query_as::<_, (i64,)>(
            "INSERT INTO steam_imports (steam_itemid, account_id, def_id, qty) VALUES ('777', 9620, $1, 1) \
             ON CONFLICT (steam_itemid) DO NOTHING RETURNING id").bind(def.clone()).fetch_optional(&pool);
        let (row,) = claim().await.unwrap().expect("first claim");
        assert!(claim().await.unwrap().is_none(), "a second request cannot claim the same Steam item");
        let first = deliver_steam_import(&pool, &st, 9620, row, &def, 1).await.unwrap();
        assert_eq!(first.len(), 1);
        let it = first[0].item.as_ref().unwrap();
        assert!(!it.bound && it.def_id == def, "tradeable: it may go back to Steam");
        assert_eq!(first[0].reason, STEAM_IMPORT_REASON, "the mailbox tab filters on it");
        assert!(deliver_steam_import(&pool, &st, 9620, row, &def, 1).await.unwrap().is_empty(), "a racing settle mails nothing");
        let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM mailbox WHERE account_id = 9620").fetch_one(&pool).await.unwrap();
        assert_eq!(n, 1);
    }

    /// AUDIT 2026-10-03: the durable halves of a continuation and of the save gate.
    #[tokio::test]
    async fn pg_a_continuation_keeps_its_old_base_trusted_in_the_db_and_one_save_runs_at_a_time() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let app = build_router(st.clone(), Router::new());
        let token = seed_session(&st, 4349, 4349);
        let post = |b: &SaveBundle| post_json("/save", &token, serde_json::to_string(b).unwrap());
        let genesis = data::new_game_in_zone(4349, &st.data, START_ZONE).expect("genesis");
        let g_hash = hash::state_hash(&genesis);
        let (b1, _) = bundle_from(&st.data, genesis.clone(), 40);
        assert!(call(&app, post(&b1)).await.1.contains("\"ok\":true"));
        let (c, _) = bundle_from(&st.data, genesis, 80);
        let (_, body) = call(&app, post(&c)).await;
        assert!(body.contains("\"ok\":true"), "continuation: {body}");
        let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM trusted WHERE account_id = 4349 AND state_hash = $1")
            .bind(&g_hash).fetch_one(&pool).await.unwrap();
        assert_eq!(n, 1, "the old base survives in the DB, so a re-send after a restart still anchors");
        let _held = SaveSlot::take(&st.saving, 4349).expect("free");
        let (code, _) = call(&app, post(&c)).await;
        assert_eq!(code, StatusCode::TOO_MANY_REQUESTS, "a second save while one runs");
    }

    /// AUDIT 2026-10-03 (round 9): a boot-synthesized tick anchor is honoured only briefly and is
    /// replaced by the first save accepted against it, so time since a restart cannot be cashed
    /// in as fast-forwarded play by a client that never re-handshakes.
    #[tokio::test]
    async fn pg_a_boot_anchor_expires_and_is_replaced_by_the_first_save() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let app = build_router(st.clone(), Router::new());
        let token = seed_session(&st, 4354, 4354);
        let post = |b: &SaveBundle| post_json("/save", &token, serde_json::to_string(b).unwrap());
        let genesis = data::new_game_in_zone(4354, &st.data, START_ZONE).expect("genesis");
        let (b1, s1) = bundle_from(&st.data, genesis, 40);
        assert!(call(&app, post(&b1)).await.1.contains("\"ok\":true"));
        // A fresh boot anchor 5 minutes old: a backlog within it is accepted, and re-anchors.
        st.tick_ref.held().insert(4354, TickAnchor { tick: 40, ms: now_ms() - 5 * 60_000, recorded: 0, from_handshake: false, synthesized: false });
        let (b2, s2) = bundle_from(&st.data, s1, 40 + 2_000);
        let (_, body) = call(&app, post(&b2)).await;
        assert!(body.contains("\"ok\":true"), "{body}");
        assert!(st.tick_ref.held().get(&4354).is_some_and(|a| a.from_handshake && a.tick == 2_040), "re-anchored");
        // An expired boot anchor is no anchor: the save is measured from the stored tick at now —
        // a client that merely slept through the restart (a few seconds of new play) saves...
        st.tick_ref.held().insert(4354, TickAnchor { tick: 2_040, ms: now_ms() - BOOT_ANCHOR_TTL_MS - 60_000, recorded: 0, from_handshake: false, synthesized: false });
        let (b4, s4) = bundle_from(&st.data, s2, 2_040 + 40);
        let (_, body) = call(&app, post(&b4)).await;
        assert!(body.contains("\"ok\":true"), "a woken client is not locked out: {body}");
        // ...and that fallback is a real, counted anchor (round 11), but a big push from it — what
        // a client that kept running through a long outage sends — is refused with "reload",
        // unflagged (round 12).
        st.tick_ref.held().insert(4354, TickAnchor { tick: 2_080, ms: now_ms() - BOOT_ANCHOR_TTL_MS - 60_000, recorded: 0, from_handshake: false, synthesized: false });
        let (b5, _) = bundle_from(&st.data, s4, 2_080 + 2_000);
        let (code, body) = call(&app, post(&b5)).await;
        assert_eq!(code, StatusCode::CONFLICT, "{body}");
        assert!(body.contains("reload"), "{body}");
        let (flags,): (i64,) = sqlx::query_as("SELECT count(*) FROM cheat_flags WHERE account_id = 4354").fetch_one(&pool).await.unwrap();
        assert_eq!(flags, 0, "a refusal gains nothing, so it is not flagged");
        // And the login-then-no-GET loop gets no fresh slack: login drops the anchor, the next save
        // installs a counted one at (stored tick, now).
        st.tick_ref.held().remove(&4354);
        let (_, _) = call(&app, post(&b5)).await;
        assert!(st.tick_ref.held().get(&4354).is_some_and(|a| a.from_handshake && a.synthesized), "the fallback anchor is installed and counted");
    }

    /// AUDIT 2026-10-03 (round 8): the save gate's server-side guards — budget, replay permit,
    /// flag dedup, body size, one stored-at time, login on a dead database.
    #[tokio::test]
    async fn pg_the_save_gates_server_side_guards_hold() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        assert!(charge_replay_budget(&st, 1, REPLAY_BUDGET_TICKS, 0));
        assert!(!charge_replay_budget(&st, 1, 1, 0));
        refund_replay_budget(&st, 1, 1);
        assert!(charge_replay_budget(&st, 1, 1, 0), "a refund gives the ticks back");
        assert!(charge_replay_budget(&st, 1, 1, REPLAY_WINDOW_MS), "a new window");
        flag_cheat(&st, 4350, "x", 1).await;
        flag_cheat(&st, 4350, "x", 2).await;
        flag_cheat(&st, 4350, "y", 3).await;
        let (x,): (i64,) = sqlx::query_as("SELECT count(*) FROM cheat_flags WHERE account_id = 4350 AND reason = 'x'").fetch_one(&pool).await.unwrap();
        let (all,): (i64,) = sqlx::query_as("SELECT count(*) FROM cheat_flags WHERE account_id = 4350").fetch_one(&pool).await.unwrap();
        assert_eq!((x, all), (1, 2), "one row per (account, reason) per hour");

        let app = build_router(st.clone(), Router::new());
        let token = seed_session(&st, 4351, 4351);
        let genesis = data::new_game_in_zone(4351, &st.data, START_ZONE).expect("genesis");
        let (b1, _) = bundle_from(&st.data, genesis, 40);
        let big = axum::http::Request::builder().method("POST").uri("/save")
            .header("authorization", format!("Bearer {token}")).header("content-type", "application/json")
            .header("content-length", (MAX_BODY_BYTES + 1).to_string())
            .body(axum::body::Body::from("{}")).unwrap();
        assert_eq!(call(&app, big).await.0, StatusCode::PAYLOAD_TOO_LARGE, "refused before it is read");
        // A GET may not carry a body (round 10: an undeclared GET body went uncharged).
        let get_body = axum::http::Request::builder().method("GET").uri("/version")
            .body(axum::body::Body::from(vec![b'x'; 70_000])).unwrap();
        assert_eq!(call(&app, get_body).await.0, StatusCode::PAYLOAD_TOO_LARGE);
        st.replay_slots.close();
        let (code, body) = call(&app, post_json("/save", &token, serde_json::to_string(&b1).unwrap())).await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        let (flags,): (i64,) = sqlx::query_as("SELECT count(*) FROM cheat_flags WHERE account_id = 4351").fetch_one(&pool).await.unwrap();
        assert_eq!(flags, 0, "nobody is flagged for the server's failure");

        let st2 = test_state(Some(pool.clone()));
        let app2 = build_router(st2.clone(), Router::new());
        let token2 = seed_session(&st2, 4352, 4352);
        let g2 = data::new_game_in_zone(4352, &st2.data, START_ZONE).expect("genesis");
        let (b2, _) = bundle_from(&st2.data, g2, 40);
        assert!(call(&app2, post_json("/save", &token2, serde_json::to_string(&b2).unwrap())).await.1.contains("\"ok\":true"));
        let (row_at,): (i64,) = sqlx::query_as("SELECT saved_at_ms FROM saves WHERE account_id = 4352").fetch_one(&pool).await.unwrap();
        assert_eq!(st2.saves.held().get(&4352).map(|v| v.saved_at_ms), Some(row_at), "one stored-at time");

        let dead = test_state(Some(pool.clone()));
        pool.close().await;
        let code = mint_session(&dead, Account { account_id: 4353, name: "dead".into(), seed: 1 }).await.err().map(|e| e.0);
        assert_eq!(code, Some(StatusCode::SERVICE_UNAVAILABLE), "a dead database is 503, never a ban's 403");
    }

    /// AUDIT 2026-10-03: `persist_save_and_claims` fills a NULL digest for a settled op re-presented.
    #[tokio::test]
    async fn pg_a_null_op_digest_is_refilled_when_the_op_comes_back() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        sqlx::query("INSERT INTO consumed_market_ops (account_id, op_id, digest) VALUES (47, 5, NULL)").execute(&pool).await.unwrap();
        let extras = AcceptedExtras { refill_digests: vec![(5, "d5".into())], ..AcceptedExtras::default() };
        assert!(persist_save_and_claims(&pool, 47, "{}", 1, "h47", &mailbox::ClaimSettlement::default(), None, &[], &extras).await.unwrap());
        let (d,): (Option<String>,) = sqlx::query_as("SELECT digest FROM consumed_market_ops WHERE account_id = 47 AND op_id = 5")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(d.as_deref(), Some("d5"));
    }

    /// AUDIT 2026-10-03: a fusion roll is answered only once it is durable, a lost row is restored by
    /// the retry, and a roll a save already used is never re-opened.
    #[tokio::test]
    async fn pg_a_fusion_roll_is_durable_and_a_used_one_is_not_reopened() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let token = seed_session(&st, 9604, 9604);
        let h = || {
            let mut h = HeaderMap::new();
            h.insert("authorization", format!("Bearer {token}").parse().unwrap());
            h
        };
        let req = || Json(FuseRollReq { key: "k".into(), attempt: 0 });
        let rows = || async {
            sqlx::query_as::<_, (i64,)>("SELECT count(*) FROM fuse_outstanding WHERE account_id = 9604").fetch_one(&pool).await.unwrap().0
        };
        let (code, Json(v)) = fuse_roll(State(st.clone()), h(), req()).await;
        assert_eq!((code, v.reason.as_str()), (StatusCode::OK, "issued"));
        assert_eq!(rows().await, 1);
        sqlx::query("DELETE FROM fuse_outstanding WHERE account_id = 9604").execute(&pool).await.unwrap();
        let (code, Json(v)) = fuse_roll(State(st.clone()), h(), req()).await;
        assert_eq!((code, v.reason.as_str()), (StatusCode::OK, "idempotent"));
        assert_eq!(rows().await, 1, "the retry writes the lost row back");
        let nonce: u64 = v.nonce.parse().unwrap();
        st.fuse_rolls.held().remove(&9604);
        st.fuse_consumed.held().entry(9604).or_default().insert(nonce);
        sqlx::query("DELETE FROM fuse_outstanding WHERE account_id = 9604").execute(&pool).await.unwrap();
        let (code, _) = fuse_roll(State(st.clone()), h(), req()).await;
        assert_eq!(code, StatusCode::CONFLICT, "a used roll is refused");
        assert_eq!(rows().await, 0, "and not re-opened");
    }

    /// The crystal wallet never holds crystal the ledger cannot explain — across EVERY way crystal move.
    ///
    /// The fixture is the point. `crystal_ledger` writes carry six reasons in production —
    /// `purchase`, `refund`, `daily`, `schedule`, `gacha-pull`, `dev-grant` — and a test that
    /// exercised one of them would agree with a formula that only works for one of them. That is
    /// precisely how the market's I7 check stayed wrong for months: its fixture had no item sale
    /// in it, so `SUM(fee)` and one counter matched by coincidence.
    ///
    /// Each movement writes the wallet and the ledger the way its handler does, including the
    /// shapes that differ: grants ADD, a gacha pull writes an ABSOLUTE wallet after
    /// `Wallet::spend`, and a refund CLAMPS at zero while logging the full negative delta.
    ///
    /// WHAT THIS TEST ESTABLISHED, by failing first. The obvious law — `wallet >= SUM(ledger)` —
    /// is the wrong way round for the case that costs money: minting leaves the wallet ABOVE the
    /// ledger, not below. And so does a clamped refund. On an account that has ever been refunded
    /// the two are indistinguishable from these tables, because the clamp records nothing about
    /// what it actually took. That is why the checker splits the two directions and only demands
    /// exactness where no refund has happened.
    #[tokio::test]
    async fn pg_the_crystal_wallet_never_holds_crystal_the_ledger_cannot_explain() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let credit = |acct: i64, t: i64, b: i64, reason: &'static str| {
            let p = pool.clone();
            async move {
                sqlx::query("INSERT INTO crystal_wallet (account_id, paid, bound) VALUES ($1,$2,$3) \
                     ON CONFLICT (account_id) DO UPDATE SET paid = crystal_wallet.paid + EXCLUDED.paid, \
                     bound = crystal_wallet.bound + EXCLUDED.bound")
                    .bind(acct).bind(t).bind(b).execute(&p).await.unwrap();
                sqlx::query("INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES ($1,$2,$3,$4,'')")
                    .bind(acct).bind(t).bind(b).bind(reason).execute(&p).await.unwrap();
            }
        };

        // Account 1: money in from every granting path, then a pull that spends across buckets.
        credit(1, 300, 0, "purchase").await;
        credit(1, 0, 50, "daily").await;
        credit(1, 0, 25, "schedule").await;
        // A pull writes the wallet ABSOLUTELY after `Wallet::spend` (bound is consumed first) and
        // logs the negative delta. Writing it the handler's way makes this a test of the shape
        // rather than of my arithmetic: 300/75 spend 135 -> bound 75 then paid 60.
        sqlx::query("INSERT INTO crystal_wallet (account_id, paid, bound) VALUES ($1,$2,$3) \
             ON CONFLICT (account_id) DO UPDATE SET paid = EXCLUDED.paid, bound = EXCLUDED.bound")
            .bind(1i64).bind(240i64).bind(0i64).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES ($1,$2,$3,'gacha-pull','k1')")
            .bind(1i64).bind(-60i64).bind(-75i64).execute(&pool).await.unwrap();

        // Account 2: never bought anything, only ever received bound crystal.
        credit(2, 0, 40, "daily").await;

        let bad = crystal_conservation_violations(&pool).await.expect("query");
        assert!(bad.is_empty(), "an honest history was reported as a break: {bad:?}");

        // MINTING — the direction that costs money. No refund in this account's history, so the
        // ledger must explain the wallet exactly, and a surplus can only be crystal from nowhere.
        sqlx::query("UPDATE crystal_wallet SET bound = bound + 500 WHERE account_id = 2")
            .execute(&pool).await.unwrap();
        let minted = crystal_conservation_violations(&pool).await.expect("query");
        assert!(minted.iter().any(|(a, b, k, _, _)| (*a, *b, k) == (2, "bound", &CrystalBreak::Unexplained)),
            "500 bound crystal appeared with no ledger row and nothing noticed: {minted:?}");
        sqlx::query("UPDATE crystal_wallet SET bound = bound - 500 WHERE account_id = 2").execute(&pool).await.unwrap();

        // SHORT — a ledger row whose wallet write never landed. `/dev/gacha-crystal` writes the two
        // outside a transaction and discards the ledger result, so this pair really can come apart.
        sqlx::query("INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES (2,0,99,'dev-grant','')")
            .execute(&pool).await.unwrap();
        let short = crystal_conservation_violations(&pool).await.expect("query");
        assert!(short.iter().any(|(a, b, k, _, _)| (*a, *b, k) == (2, "bound", &CrystalBreak::Short)),
            "the ledger promises 99 crystal the wallet does not hold and nothing noticed: {short:?}");

        // THE REFUND, driven through the REAL handler. Account 1 holds 240 paid and is
        // refunded 300 — the wallet goes to -60 and the ledger says the same. Calling `store_refund` rather than replaying its
        // SQL is the whole point: a fixture that re-implements the code under test agrees with
        // whatever the fixture does, which is how the first version of this test passed while the
        // shortfall row did not exist.
        sqlx::query("INSERT INTO store_orders (order_id, account_id, sku, idem_key, price_cents, status, granted_paid, granted_bound) \
             VALUES (901, 1, 'sku_crystal', 'idem-901', 999, 'granted', 300, 0)")
            .execute(&pool).await.unwrap();
        let st = test_state(Some(pool.clone()));
        let (code, _) = store_refund(State(st), Json(RefundReq { order_id: "901".into() })).await;
        assert_eq!(code, StatusCode::OK, "the refund must go through");

        // Since 2026-10-02 the refund takes the WHOLE grant back, into debt (user: "ควรต้องติดลบด้วย").
        let (paid,): (i64,) = sqlx::query_as("SELECT paid FROM crystal_wallet WHERE account_id = 1")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(paid, 240 - 300, "300 refunded from 240: the account owes 60");

        let after = crystal_conservation_violations(&pool).await.expect("query");
        assert!(!after.iter().any(|(a, _, _, _, _)| *a == 1),
            "a refund that recorded its shortfall still broke the law — the compensating row is \
             the wrong sign or the wrong amount: {after:?}");

        // AND THE THING THAT USED TO BE INVISIBLE. Before the shortfall row, a refunded account
        // was exempt from the surplus check — the clamp and a mint looked identical — and this
        // assertion existed to document that hole. It is now the opposite: minting on a refunded
        // account is caught like anywhere else.
        sqlx::query("UPDATE crystal_wallet SET paid = paid + 777 WHERE account_id = 1")
            .execute(&pool).await.unwrap();
        let hidden = crystal_conservation_violations(&pool).await.expect("query");
        assert!(hidden.iter().any(|(a, b, k, _, _)| (*a, *b, k) == (1, "paid", &CrystalBreak::Unexplained)),
            "777 crystal minted on a REFUNDED account went unseen — the clamp is hiding them again, \
             which means the refund-shortfall row is missing or wrong: {hidden:?}");
    }

    /// A settled order can never be granted again — and "settled" includes REFUNDED.
    ///
    /// The guard was `status <> 'granted'`, so a refunded order sailed through it. That alone
    /// would be a second helping of crystal; worse, the first-buy count ignores revoked rows, so the
    /// refund that took the bonus away also re-armed it. Its own comment said "only from
    /// 'pending'" — which nobody had written, and which would have refused every real purchase,
    /// because `store_checkout` inserts `init` and the update to `pending` is a `let _ =` whose
    /// lost result leaves a perfectly good order at `init`.
    ///
    /// Driven through `claim_order_for_grant`, the real function `store_finalize` calls. The
    /// handler itself cannot be tested — it talks to Steam's FinalizeTxn first — and replaying its
    /// UPDATE inline would only prove the test agrees with itself.
    #[tokio::test]
    async fn pg_a_settled_order_cannot_be_granted_again() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let seed = |id: i64, status: &'static str| {
            let p = pool.clone();
            async move {
                sqlx::query("INSERT INTO store_orders (order_id, account_id, sku, idem_key, price_cents, status) \
                     VALUES ($1, 5, 'sku_crystal', $2, 499, $3)")
                    .bind(id).bind(format!("idem-{id}")).bind(status).execute(&p).await.unwrap();
            }
        };
        seed(1001, "init").await;
        seed(1002, "pending").await;
        seed(1003, "granted").await;
        seed(1004, "refunded").await;
        seed(1005, "failed").await;

        let mut tx = pool.begin().await.unwrap();
        // The two states that mean "paid for, not yet settled" must claim exactly once each.
        assert!(claim_order_for_grant(&mut tx, 1001, true, 100, 0).await, "an `init` order must grant");
        assert!(!claim_order_for_grant(&mut tx, 1001, true, 100, 0).await, "and only once");
        assert!(claim_order_for_grant(&mut tx, 1002, true, 100, 0).await, "a `pending` order must grant");

        // The settled ones must not.
        assert!(!claim_order_for_grant(&mut tx, 1003, true, 100, 0).await, "already granted");
        assert!(!claim_order_for_grant(&mut tx, 1004, true, 100, 0).await,
            "a REFUNDED order was granted a second time — the player keeps the crystal from the first \
             grant, gets a second helping, and the first-buy bonus is re-armed because the refund \
             revoked the entitlement the count looks at");
        assert!(!claim_order_for_grant(&mut tx, 1005, true, 100, 0).await,
            "a FAILED order was granted — Steam never took the money for it");
        tx.commit().await.unwrap();
    }

    /// THE PULL'S WALLET WRITE IS A GUARDED DELTA, pinned by source.
    ///
    /// `pg_a_pull_debit_is_a_delta_the_database_applies` drives a COPY of the statement, so it
    /// stays green if the handler changes underneath it — which is exactly the gap that let an
    /// absolute write live here. This reads the handler.
    ///
    /// An absolute write (`SET paid = EXCLUDED.paid`) stores a number computed in
    /// memory: turn the upsert into a no-op and every pull is free with every test still passing,
    /// and any credit landing mid-pull is erased. A delta with a `>=` floor makes an unpayable
    /// debit match zero rows and fail the roll instead.
    #[test]
    fn the_pull_debits_the_wallet_with_a_guarded_delta() {
        let src = include_str!("main.rs");
        let body = src.split("async fn gacha_roll(").nth(1).expect("gacha_roll exists");
        let body = &body[..body.find("\nasync fn ").unwrap_or(body.len())];
        let code: String = body.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
        assert!(
            code.contains("SET paid = paid - $2"),
            "the pull no longer subtracts from the stored balance — an absolute write makes a \
             missed debit a FREE pull and erases any credit that landed mid-pull"
        );
        assert!(
            code.contains("paid >= $2") && code.contains("bound >= $3"),
            "the debit lost its floor: an unaffordable pull must match zero rows and fail, not \
             drive the wallet negative"
        );
        assert!(
            !code.contains("INSERT INTO crystal_wallet"),
            "the pull is upserting the wallet again — it must only ever subtract from a row that \
             already exists, or a pull can CREATE a wallet and pay itself"
        );
    }

    /// EVERY WRITE TO THE COUPON COUNTER SITS BESIDE A LEDGER ROW.
    ///
    /// The conservation test proves the invariant holds over a set of movements. It cannot prove
    /// the handler keeps WRITING them: delete the spend-side ledger insert and everything compiles
    /// and stays green while the balance quietly stops being explainable. `crystal_ledger` has the
    /// same shape of exposure and `docs/economy-invariants.md` I7 is about exactly this — a
    /// formula and a fixture agreeing with each other while nobody tests the claim.
    ///
    /// Two halves, like the market's custody scanner. The list cannot discover a writer nobody
    /// told it about — that is the honest limit — but the FIRST half is what covers that: a write
    /// outside the listed functions fails wherever it appears.
    #[test]
    fn every_coupon_counter_write_is_accompanied_by_a_ledger_row() {
        let src = include_str!("main.rs");
        let prod = &src[..src.find("#[cfg(test)]").unwrap_or(src.len())];

        // Split production into functions so a write can be attributed to one.
        let mut fns: Vec<(String, String)> = Vec::new();
        let mut name = String::from("<file>");
        let mut body = String::new();
        for line in prod.lines() {
            let t = line.trim_start();
            if let Some(rest) = t.strip_prefix("async fn ").or_else(|| t.strip_prefix("fn ")) {
                fns.push((name.clone(), std::mem::take(&mut body)));
                name = rest.split('(').next().unwrap_or("?").to_string();
            }
            body.push('\n');
            body.push_str(line);
        }
        fns.push((name, body));

        // The functions allowed to move the counter. Adding one here is a deliberate act; the
        // assertion below then forces it to carry its own ledger row.
        const WRITERS: &[&str] = &["claim_collect", "gacha_roll", "slot_tickets", "lootbox_opens", "farm_tickets", "farm_pull", "grant_contents", "revoke_contents", "tutorial_claim", "bp_grant_currency", "mint_coupons_claim", "spend_mint_coupon", "refund_mint_coupon"];

        let mut seen: Vec<&str> = Vec::new();
        for (fname, fbody) in &fns {
            // Skip the DDL: `migrate` names every table it creates.
            if fname == "migrate" {
                continue;
            }
            let writes = fbody.contains("INSERT INTO bound_items")
                || fbody.contains("UPDATE bound_items");
            if !writes {
                continue;
            }
            let allowed = WRITERS.iter().find(|w| *w == fname);
            assert!(
                allowed.is_some(),
                "`{fname}` moves the coupon counter and is not a declared writer. Either route it \
                 through one, or add it to WRITERS — which obliges it to log the movement."
            );
            assert!(
                fbody.contains("INSERT INTO bound_ledger"),
                "`{fname}` changes the coupon balance without writing `bound_ledger`. A counter \
                 that moves unrecorded cannot be reconciled, and coupons are bought with real \
                 money — see docs/bound-items-design.md and economy-invariants.md I7."
            );
            seen.push(allowed.copied().unwrap_or(""));
        }
        // And the list is not stale: every declared writer still writes.
        for w in WRITERS {
            assert!(seen.contains(w), "`{w}` is declared a coupon writer but no longer writes one");
        }
    }

    /// THE PULL CLAIMS ITS KEY WITH `ON CONFLICT DO NOTHING`, pinned by source.
    ///
    /// `pg_two_pulls_on_one_idem_key_cannot_both_roll` races two real transactions — but on a COPY
    /// of the statement, so it stays green if the handler's own claim changes. That is the same
    /// gap that let an absolute wallet write survive here, so the claim gets the same treatment as
    /// the debit.
    ///
    /// `DO UPDATE` instead of `DO NOTHING` is the shape that breaks it: both requests then "claim"
    /// the key, both roll, the player is charged twice and the second overwrites results the first
    /// already showed them.
    #[test]
    fn the_pull_claims_its_key_before_it_charges() {
        let src = include_str!("main.rs");
        let body = src.split("async fn gacha_roll(").nth(1).expect("gacha_roll exists");
        let body = &body[..body.find("\nasync fn ").unwrap_or(body.len())];
        let code: String = body.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");

        let claim = code.find("INSERT INTO gacha_rolls").expect("the pull claims its idem_key");
        let tail = &code[claim..(claim + 260).min(code.len())];
        assert!(tail.contains("ON CONFLICT DO NOTHING"),
            "the key claim no longer refuses a duplicate — two concurrent pulls both roll, the \
             player is charged twice and the loser overwrites the winner's results. Found: {tail:?}");
        // And it must come BEFORE the money moves, or the guard guards nothing.
        let debit = code.find("SET paid = paid - $2").expect("the pull debits the wallet");
        assert!(claim < debit, "the wallet is charged before the key is claimed");
        // AND ITS RESULT MUST BE READ. `ON CONFLICT DO NOTHING` only guards anything if the zero
        // rows it returns are acted on — pinning the SQL text alone would leave a handler that
        // ignores `rows_affected` looking correct.
        let after = &code[claim..(claim + 700).min(code.len())];
        assert!(after.contains("rows_affected"),
            "the claim's result is never inspected, so a duplicate key rolls anyway");
    }

    /// EVERY entitlement read goes through `owns_sku`. A raw query that forgets
    /// `revoked_at IS NULL` hands a refunded player the capability they were refunded for,
    /// permanently, while the refund itself looks like it worked.
    ///
    /// A source scanner, not a compiler gate — the query is a string, so this is the strongest
    /// thing available. It cannot see a query built by concatenation; it can see the ordinary way
    /// of writing one, which is how this would actually be reintroduced.
    #[test]
    fn every_entitlement_read_goes_through_owns_sku() {
        let src = include_str!("main.rs");
        // Production only: the test module below legitimately reads the table to set fixtures up
        // and to assert on what the helper did.
        let prod = &src[..src.find("#[cfg(test)]").unwrap_or(src.len())];
        // WHOLE TEXT, not per line. The commit that added this named its own limit as "a query
        // built by concatenation" — and the real hole was ordinary `\`-continued multi-line string
        // literals, which is how most SQL in this file is written: put `FROM entitlements` on the
        // continuation line and a per-line scan sees nothing at all.
        let flat = prod.replace('\n', " ");
        let mut reads = Vec::new();
        let mut from = 0usize;
        while let Some(i) = flat[from..].find("FROM entitlements") {
            let at = from + i;
            let start = at.saturating_sub(90);
            reads.push((at, flat[start..(at + 90).min(flat.len())].to_string()));
            from = at + 1;
        }
        assert_eq!(
            reads.len(),
            1,
            "a SECOND entitlement read appeared outside `owns_sku`. Route it through the helper — \
             the `revoked_at IS NULL` filter lives there so nobody has to remember it: {reads:?}"
        );
        // And that one read is the helper's own, filtered.
        let (_, only) = &reads[0];
        assert!(only.contains("revoked_at IS NULL"), "the helper stopped filtering revoked rows");
        // The helper must not answer an error with a guess: its one caller asks the INVERSE
        // question, so a default is wrong in one direction whichever way it points.
        let helper = prod.split("async fn owns_sku").nth(1).expect("owns_sku exists");
        let helper = &helper[..helper.find("\nasync fn ").unwrap_or(helper.len())];
        assert!(!helper.contains("unwrap_or("),
            "`owns_sku` is defaulting on a database error again — its caller computes \
             `first_buy = !owns_sku(..)`, so a default of false PAYS THE BONUS TWICE");
    }

    /// The first-buy bonus is decided INSIDE the transaction that grants it.
    ///
    /// It used to be a `SELECT ... fetch_one(db)` taken before `db.begin()`, so two finalizes of
    /// two DIFFERENT orders of the same SKU both counted zero entitlements and both paid the
    /// once-per-SKU bonus. The status guard cannot stop that — it keys on `order_id`, and those
    /// are two different orders.
    ///
    /// This pins the ordering by source, which is the honest limit: the race itself needs Steam's
    /// FinalizeTxn to run twice concurrently, and `store_finalize` cannot be driven from a test.
    /// What a source check CAN say is that the read is not before the transaction — and this file
    /// has been bitten enough times by scans that match comments for the search to skip them.
    #[test]
    fn the_first_buy_count_is_read_inside_the_grant_transaction() {
        let src = include_str!("main.rs");
        // The grant moved into `grant_order` (2026-09-29) so the finalize, the charged-order sweep
        // and the admin grant share one copy of the bonus rule — that is the body to read.
        let body = src.split("async fn grant_order(").nth(1).expect("grant_order exists");
        let body = &body[..body.find("\nasync fn ").unwrap_or(body.len())];
        // Comments have satisfied a scan in this file before; strip them.
        let code: String = body.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");

        // The read moved INTO `owns_sku`, so what is checked here is the CALL. Same property,
        // one indirection along: the question must be asked on the transaction that grants.
        let count = code.find("owns_sku(").expect("the first-buy decision asks owns_sku");
        // WHICH EXECUTOR, not which line. The first version of this check compared the POSITION of
        // `db.begin()` against the position of the query, and moving the executor back from
        // `&mut *tx` to `db` — the exact bug — left both positions unchanged and the check green.
        // A statement's transaction is decided by what it is executed ON, so that is what to read.
        let exec_arg = &code[count..(count + 40).min(code.len())];
        assert!(exec_arg.contains("&mut *tx"),
            "the first-buy count runs on the POOL, not the transaction that grants — so two \
             concurrent finalizes of the same SKU both see zero entitlements and both pay the \
             once-per-SKU bonus. Found: {exec_arg:?}");
        let begin = code.find("db.begin()").expect("grant_order opens a transaction");
        assert!(begin < count, "the transaction must be open before the count is taken");
        let lock = code.find("FROM accounts WHERE account_id = $1 FOR UPDATE").expect("it locks the account");
        assert!(lock < count,
            "the account row is not locked before the count — the lock is what serialises two \
             orders of the same SKU, since there is no row to lock for an entitlement that does \
             not exist yet");
    }

    /// The conservation law is EVALUATED somewhere that runs, and `/health` says so.
    ///
    /// `crystal_conservation_violations` shipped with no caller but a test. The audit put it plainly:
    /// "in production the conservation law is never evaluated — it is a function that exists so a
    /// test can call it." A law nobody runs is a comment with a return type.
    #[tokio::test]
    async fn pg_the_crystal_sweep_reports_breaks_through_health() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));

        sweep_crystal_conservation(&st).await;
        assert_eq!(health(State(st.clone())).await, "ok", "a balanced ledger says nothing extra");

        // Crystal with no ledger row behind them — the direction that costs money.
        sqlx::query("INSERT INTO crystal_wallet (account_id, paid, bound) VALUES (77, 500, 0)")
            .execute(&pool).await.unwrap();
        sweep_crystal_conservation(&st).await;
        let body = health(State(st.clone())).await;
        assert!(body.contains("crystal_breaks=1"),
            "500 crystal exist with no row explaining them and /health still says {body:?} — the \
             sweep is not wired to anything that runs, which is what it was written to fix");
        assert!(body.starts_with("ok"),
            "/health must keep answering ok: fly.toml points its check here and a failing check \
             restarts the machine, which fixes no bookkeeping and drops the in-memory market");

        // And it clears when the books agree again — a counter that only ever goes up gets muted.
        sqlx::query("INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES (77,500,0,'purchase','')")
            .execute(&pool).await.unwrap();
        sweep_crystal_conservation(&st).await;
        assert_eq!(health(State(st.clone())).await, "ok", "the break was resolved and the report did not clear");

        // A SWEEP THAT COULD NOT RUN IS NOT A CLEAN BILL OF HEALTH. Reporting zero breaks because
        // the query failed is the "green because UNTESTED" shape this crate keeps producing — the
        // last known count has to stand instead.
        sqlx::query("INSERT INTO crystal_wallet (account_id, paid, bound) VALUES (78, 900, 0)")
            .execute(&pool).await.unwrap();
        sweep_crystal_conservation(&st).await;
        assert!(health(State(st.clone())).await.contains("crystal_breaks=1"), "the break is seen first");
        pool.close().await;
        sweep_crystal_conservation(&st).await;
        assert!(health(State(st)).await.contains("crystal_breaks=1"),
            "the sweep could not reach the database and reported the books CLEAN — an outage now \
             looks exactly like conservation holding");
    }

    /// A save carrying one honest `ClaimMail` for `mail_id` (Gil only), replay-verifiable.
    fn claim_bundle(data: &GameData, seed: u64, ticks: u64, mail_id: u64, gil: u64) -> SaveBundle {
        let log = vec![sim_core::events::RecordedInput {
            tick: 1,
            event: InputEvent::ClaimMail { mail_id, gil: sim_core::state::Gil::new(gil), def_id: String::new(), qty: 0, item: None },
        }];
        verifiable_bundle(data, seed, ticks, log)
    }

    async fn claimed_at(pool: &sqlx::PgPool, mail_id: u64) -> Option<i64> {
        let (c,): (Option<i64>,) = sqlx::query_as("SELECT claimed_at FROM mailbox WHERE id = $1")
            .bind(mail_id as i64).fetch_one(pool).await.unwrap();
        c
    }

    fn settled_ops(body: &str) -> Vec<u64> {
        let v: serde_json::Value = serde_json::from_str(body).unwrap();
        // Absent when empty (`skip_serializing_if`).
        v["settled_ops"].as_array().map(|a| a.iter().map(|x| x.as_u64().unwrap()).collect()).unwrap_or_default()
    }

    /// The mail claim, end to end through `/save`: a verified claim consumes its mail and its op
    /// in the save's own transaction, hands the op back in `settled_ops` (the client prunes by it),
    /// and a re-push of the same log settles nothing a second time.
    #[tokio::test]
    async fn pg_a_mail_claim_settles_through_the_real_router_exactly_once() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let app = build_router(st.clone(), Router::new());
        let token = seed_session(&st, 9300, 9300);
        let mail = mailbox_store::send(&pool, 9300, MailKind::Reward, 7600, "", 0, "test").await.unwrap();

        let b = claim_bundle(&st.data, 9300, 40, mail, 7600);
        let (code, body) = call(&app, post_json("/save", &token, serde_json::to_string(&b).unwrap())).await;
        assert_eq!(code, StatusCode::OK, "{body}");
        assert!(body.contains("\"ok\":true"), "{body}");
        let ops = settled_ops(&body);
        assert_eq!(ops.len(), 1, "the claim op comes back for the client to prune: {body}");
        assert!(claimed_at(&pool, mail).await.is_some(), "the mail is consumed with the save");
        let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM consumed_market_ops WHERE account_id = 9300 AND op_id = $1")
            .bind(ops[0] as i64).fetch_one(&pool).await.unwrap();
        assert_eq!(n, 1, "and its op is recorded as consumed");

        // The same log again (a lost response → the client retries): idempotent, nothing re-paid.
        let (code, body) = call(&app, post_json("/save", &token, serde_json::to_string(&b).unwrap())).await;
        assert_eq!(code, StatusCode::OK, "{body}");
        assert!(body.contains("\"ok\":true"), "a re-push of a settled claim is not a forgery: {body}");
        assert!(settled_ops(&body).is_empty(), "nothing new settled: {body}");
    }

    /// AUDIT 2026-10-03: a mail withdrawn by a refund past the grace window — a NEW claim of it is
    /// refused unflagged even on a tainted save; a claim already stored (by a tainted save, in
    /// time) settles later however late, so the account is never locked out of saving.
    #[tokio::test]
    async fn pg_a_withdrawn_mail_is_refused_on_a_tainted_save_and_a_stored_claim_is_forgiven() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let app = build_router(st.clone(), Router::new());
        let late = || now_ms() - mailbox::WITHDRAWN_GRACE_MS - 60_000;
        // (a) A new claim of a long-withdrawn mail on a tainted save: refused, not flagged.
        let token = seed_session(&st, 9360, 9360);
        let mail = mailbox_store::send(&pool, 9360, MailKind::Reward, 7600, "", 0, "test").await.unwrap();
        sqlx::query("UPDATE mailbox SET voided_at = $2 WHERE id = $1").bind(mail as i64).bind(late()).execute(&pool).await.unwrap();
        st.sessions.tainted.held().insert(9360);
        let b = claim_bundle(&st.data, 9360, 40, mail, 7600);
        let (_, body) = call(&app, post_json("/save", &token, serde_json::to_string(&b).unwrap())).await;
        assert!(body.contains("\"ok\":false") && body.contains("withdrawn"), "{body}");
        let (flags,): (i64,) = sqlx::query_as("SELECT count(*) FROM cheat_flags WHERE account_id = 9360").fetch_one(&pool).await.unwrap();
        assert_eq!(flags, 0, "a late claim is refused, not flagged");

        // (b) Stored unsettled by a tainted save while still in the window, then the window passes.
        let token = seed_session(&st, 9361, 9361);
        let mail = mailbox_store::send(&pool, 9361, MailKind::Reward, 7600, "", 0, "test").await.unwrap();
        sqlx::query("UPDATE mailbox SET voided_at = $2 WHERE id = $1").bind(mail as i64).bind(now_ms()).execute(&pool).await.unwrap();
        st.sessions.tainted.held().insert(9361);
        let b1 = claim_bundle(&st.data, 9361, 40, mail, 7600);
        let (_, body) = call(&app, post_json("/save", &token, serde_json::to_string(&b1).unwrap())).await;
        assert!(body.contains("\"ok\":true"), "stored while tainted: {body}");
        assert!(claimed_at(&pool, mail).await.is_none(), "a tainted save settles nothing");
        sqlx::query("UPDATE mailbox SET voided_at = $2 WHERE id = $1").bind(mail as i64).bind(late()).execute(&pool).await.unwrap();
        let s1 = b1.verified_state(&st.data).expect("verifies");
        let (b2, _) = bundle_from(&st.data, s1, 80);
        let (_, body) = call(&app, post_json("/save", &token, serde_json::to_string(&b2).unwrap())).await;
        assert!(body.contains("\"ok\":true"), "the stored claim is forgiven: {body}");
        assert!(claimed_at(&pool, mail).await.is_some(), "and settles now");
    }

    /// A claim whose payload lies about the real mail is rejected, flagged, and consumes nothing.
    #[tokio::test]
    async fn pg_a_forged_mail_claim_is_rejected_and_flagged() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let app = build_router(st.clone(), Router::new());
        let token = seed_session(&st, 9350, 9350);
        let mail = mailbox_store::send(&pool, 9350, MailKind::Reward, 7600, "", 0, "test").await.unwrap();

        let b = claim_bundle(&st.data, 9350, 40, mail, 999_999);
        let (code, body) = call(&app, post_json("/save", &token, serde_json::to_string(&b).unwrap())).await;
        assert_eq!(code, StatusCode::OK, "{body}");
        assert!(body.contains("\"ok\":false") && body.contains("forged"), "{body}");
        assert!(claimed_at(&pool, mail).await.is_none(), "a forged save consumes nothing");
        let (saves,): (i64,) = sqlx::query_as("SELECT count(*) FROM saves WHERE account_id = 9350").fetch_one(&pool).await.unwrap();
        assert_eq!(saves, 0, "and is not stored");
        let (flags,): (i64,) = sqlx::query_as("SELECT count(*) FROM cheat_flags WHERE account_id = 9350 AND reason = 'forged-claim'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(flags, 1, "the account is flagged");
        assert!(st.sessions.tainted.held().contains(&9350), "and tainted");
    }

    async fn wallet(pool: &sqlx::PgPool, acct: i64) -> (i64, i64) {
        sqlx::query_as("SELECT paid, bound FROM crystal_wallet WHERE account_id = $1")
            .bind(acct).fetch_optional(pool).await.unwrap().unwrap_or((0, 0))
    }

    async fn order_row(pool: &sqlx::PgPool, order: i64, acct: i64, status: &str) {
        sqlx::query("INSERT INTO accounts (account_id, name, seed) VALUES ($1, $2, 1) ON CONFLICT DO NOTHING")
            .bind(acct).bind(format!("buyer{acct}")).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO store_orders (order_id, account_id, sku, idem_key, price_cents, status) VALUES ($1, $2, 'crystal_300', $3, 499, $4)")
            .bind(order).bind(acct).bind(format!("idem-{order}")).bind(status).execute(pool).await.unwrap();
    }

    async fn sku_order(pool: &sqlx::PgPool, order: i64, acct: i64, sku: &str, season: Option<i64>) {
        sqlx::query("INSERT INTO accounts (account_id, name, seed) VALUES ($1, $2, 1) ON CONFLICT DO NOTHING")
            .bind(acct).bind(format!("buyer{acct}")).execute(pool).await.unwrap();
        sqlx::query("INSERT INTO store_orders (order_id, account_id, sku, idem_key, price_cents, status, season_id) VALUES ($1, $2, $3, $4, 99, 'charged', $5)")
            .bind(order).bind(acct).bind(sku).bind(format!("idem-{order}")).bind(season).execute(pool).await.unwrap();
    }

    /// AUDIT 2026-10-02: refunding an order that was never granted takes nothing from what OTHER
    /// orders granted — here, a granted monthly card keeps its 30 days.
    #[tokio::test]
    async fn pg_refunding_an_ungranted_order_takes_back_nothing() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        sku_order(&pool, 7301, 9701, "monthly_card", None).await;
        assert!(matches!(grant_order(&pool, &st.data, 7301).await, Ok(GrantOutcome::Granted { .. })));
        let ends = || async {
            sqlx::query_as::<_, (i64,)>("SELECT ends_day FROM monthly_card WHERE account_id = 9701").fetch_one(&pool).await.unwrap().0
        };
        let before = ends().await;
        sku_order(&pool, 7302, 9701, "monthly_card", None).await; // charged, grant never ran
        assert_eq!(revoke_order(&pool, 7302, 9701, 0, 0).await, Ok(true));
        assert_eq!(ends().await, before, "the granted card keeps its days");
        let (status,): (String,) = sqlx::query_as("SELECT status FROM store_orders WHERE order_id = 7302").fetch_one(&pool).await.unwrap();
        assert_eq!(status, "refunded");
    }

    /// AUDIT 2026-10-03: the cap is counted under the per-(account, SKU) lock with the order claim:
    /// a capped pack whose other order is mid-FinalizeTxn (`charging`) is refused BEFORE Steam, and
    /// the refused order is closed, never moved to `charging`.
    #[tokio::test]
    async fn pg_finalize_counts_a_charging_order_toward_the_cap() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let token = seed_session(&st, 9602, 9602);
        let mut h = HeaderMap::new();
        h.insert("authorization", format!("Bearer {token}").parse().unwrap());
        sku_order(&pool, 7210, 9602, "pack_starter", None).await;
        sqlx::query("UPDATE store_orders SET status = 'charging', charging_at = now() WHERE order_id = 7210").execute(&pool).await.unwrap();
        sku_order(&pool, 7211, 9602, "pack_starter", None).await;
        sqlx::query("UPDATE store_orders SET status = 'pending' WHERE order_id = 7211").execute(&pool).await.unwrap();
        let (code, Json(v)) = store_finalize(State(st.clone()), h, Json(FinalizeReq { order_id: "7211".into() })).await;
        assert_eq!((code, v["reason"].as_str()), (StatusCode::CONFLICT, Some("purchase limit reached")));
        let (status,): (String,) = sqlx::query_as("SELECT status FROM store_orders WHERE order_id = 7211").fetch_one(&pool).await.unwrap();
        assert_eq!(status, "failed");
    }

    /// AUDIT 2026-10-03: the save gate refuses a non-standard checkpoint spacing before any replay,
    /// and a different sim build without flagging.
    #[tokio::test]
    async fn pg_a_save_with_a_cheap_checkpoint_spacing_or_another_sim_build_is_refused() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let app = build_router(st.clone(), Router::new());
        let token = seed_session(&st, 4347, 4347);
        let genesis = data::new_game_in_zone(4347, &st.data, START_ZONE).expect("genesis");
        let (b1, _) = bundle_from(&st.data, genesis, 40);
        let dense = SaveBundle { snapshot_interval: 1, ..b1.clone() };
        let (code, body) = call(&app, post_json("/save", &token, serde_json::to_string(&dense).unwrap())).await;
        assert_eq!(code, StatusCode::BAD_REQUEST, "{body}");
        let skew = SaveBundle { sim_build: "deadbeef".into(), ..b1 };
        let (code, body) = call(&app, post_json("/save", &token, serde_json::to_string(&skew).unwrap())).await;
        assert_eq!(code, StatusCode::CONFLICT, "{body}");
        assert!(body.contains("sim build skew"), "{body}");
        let (flags,): (i64,) = sqlx::query_as("SELECT count(*) FROM cheat_flags WHERE account_id = 4347").fetch_one(&pool).await.unwrap();
        assert_eq!(flags, 0, "a skewed client is not a cheater");
    }

    /// AUDIT 2026-10-03: a held season past its end is no season, before the hourly refresh.
    #[tokio::test]
    async fn pg_an_ended_held_season_is_not_live() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let token = seed_session(&st, 9603, 9603);
        let mut h = HeaderMap::new();
        h.insert("authorization", format!("Bearer {token}").parse().unwrap());
        let now = now_ms();
        *st.season.held() = Some(bp::Season { season_id: 77, starts_at_ms: now - 2_000, ends_at_ms: now - 1_000, seed: 1, config: bp::default_config() });
        assert!(live_season(&st).is_none());
        let (_, Json(v)) = bp_claim(State(st.clone()), h, Json(BpClaimReq { track: 0, slots: vec![1] })).await;
        assert_eq!(v["reason"].as_str(), Some("no active season"));
    }

    /// AUDIT 2026-10-02: two open checkouts of a one-per-account pack — the second sees the first
    /// (in flight, not yet granted) and is over the cap; an abandoned hour-old checkout is not.
    #[tokio::test]
    async fn pg_a_second_open_checkout_of_a_capped_pack_is_over_the_cap() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let starter = store::sku("pack_starter").unwrap();
        sku_order(&pool, 7201, 9601, "pack_starter", None).await;
        assert!(!orders_in_flight_over_cap(&pool, 9601, starter, None).await.unwrap(), "one order: at the cap");
        sku_order(&pool, 7202, 9601, "pack_starter", None).await;
        sqlx::query("UPDATE store_orders SET status = 'init' WHERE order_id = 7202").execute(&pool).await.unwrap();
        assert!(orders_in_flight_over_cap(&pool, 9601, starter, None).await.unwrap(), "a second open order: over");
        sqlx::query("UPDATE store_orders SET created_at = now() - interval '2 hours' WHERE order_id = 7202").execute(&pool).await.unwrap();
        assert!(!orders_in_flight_over_cap(&pool, 9601, starter, None).await.unwrap(), "abandoned: not counted");
        let crystal = store::sku("crystal_300").unwrap();
        assert!(!orders_in_flight_over_cap(&pool, 9601, crystal, None).await.unwrap(), "uncapped SKUs never are");
        let bp = store::sku("battle_pass").unwrap();
        sku_order(&pool, 7203, 9601, "battle_pass", Some(5)).await;
        sku_order(&pool, 7204, 9601, "battle_pass", Some(5)).await;
        assert!(orders_in_flight_over_cap(&pool, 9601, bp, Some(5)).await.unwrap(), "two passes for one season");
        assert!(!orders_in_flight_over_cap(&pool, 9601, bp, Some(6)).await.unwrap(), "the next season is its own");
    }

    /// AUDIT 2026-10-02: an order that cannot be granted for a day leaves the sweep's queue as
    /// `grant_failed` (and an admin grant still finishes it); a fresh failure stays queued.
    #[tokio::test]
    async fn pg_a_permanently_failing_order_is_parked_not_left_blocking_the_sweep() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        sku_order(&pool, 7501, 9901, "sku_retired_long_ago", None).await;
        sku_order(&pool, 7502, 9901, "sku_retired_long_ago", None).await;
        sqlx::query("UPDATE store_orders SET created_at = now() - interval '25 hours' WHERE order_id = 7501").execute(&pool).await.unwrap();
        sweep_charged_orders(&pool, &st.data).await; // both fail once
        sweep_charged_orders(&pool, &st.data).await; // the day-old one is parked
        let status = |id: i64| {
            let pool = pool.clone();
            async move {
                sqlx::query_as::<_, (String, i32)>("SELECT status, grant_attempts FROM store_orders WHERE order_id = $1")
                    .bind(id).fetch_one(&pool).await.unwrap()
            }
        };
        assert_eq!(status(7501).await.0, "grant_failed", "a day of failures: parked");
        assert_eq!(status(7502).await.0, "charged", "a fresh failure stays in the queue");
        assert!(status(7502).await.1 >= 1, "its attempts are counted");

        // A parked order of a real SKU is still finishable by the admin path (`grant_order`).
        sku_order(&pool, 7503, 9901, "pack_starter", None).await;
        sqlx::query("UPDATE store_orders SET status = 'grant_failed' WHERE order_id = 7503").execute(&pool).await.unwrap();
        assert!(matches!(grant_order(&pool, &st.data, 7503).await, Ok(GrantOutcome::Granted { .. })));
    }

    /// A Battle Pass order finalizes only into the season running now (user 2026-10-02).
    #[tokio::test]
    async fn pg_a_battle_pass_order_only_finalizes_into_the_current_season() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let now = now_ms();
        *st.season.held() = Some(bp::Season {
            season_id: 8, starts_at_ms: now - 1000, ends_at_ms: now + 86_400_000, seed: 1, config: bp::default_config(),
        });
        sku_order(&pool, 7401, 9801, "battle_pass", Some(8)).await;
        sku_order(&pool, 7402, 9801, "battle_pass", Some(7)).await;
        sku_order(&pool, 7403, 9801, "pack_starter", None).await;
        assert!(bp_order_season_is_current(&st, &pool, 7401).await, "this season");
        assert!(!bp_order_season_is_current(&st, &pool, 7402).await, "a past season");
        assert!(bp_order_season_is_current(&st, &pool, 7403).await, "not a pass");
        *st.season.held() = None;
        assert!(!bp_order_season_is_current(&st, &pool, 7401).await, "no season running");
    }

    async fn coupons(pool: &sqlx::PgPool, acct: i64) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT qty FROM bound_items WHERE account_id = $1 AND kind = $2")
            .bind(acct).bind(gacha::STANDARD_COUPON_KIND).fetch_optional(pool).await.unwrap().map_or(0, |r| r.0)
    }

    /// Starter: ten coupons, once per account; a refund takes back the coupons still unspent.
    /// Growth: gated on Commander level at checkout, items arrive as BOUND mail, and a refund
    /// withdraws the mail not yet claimed.
    #[tokio::test]
    async fn pg_packs_deliver_their_contents_once_and_a_refund_takes_back_the_rest() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let starter = store::sku("pack_starter").unwrap();
        assert_eq!(purchase_refusal(&st, &pool, 9501, starter).await.unwrap(), None);
        sku_order(&pool, 7101, 9501, "pack_starter", None).await;
        assert!(matches!(grant_order(&pool, &st.data, 7101).await, Ok(GrantOutcome::Granted { .. })));
        assert_eq!(coupons(&pool, 9501).await, 10, "ten pulls");
        assert_eq!(wallet(&pool, 9501).await, (0, 0), "the starter carries no crystal");
        assert_eq!(purchase_refusal(&st, &pool, 9501, starter).await.unwrap(), Some("purchase limit reached"));
        assert!(matches!(grant_order(&pool, &st.data, 7101).await, Ok(GrantOutcome::AlreadySettled)));
        assert_eq!(coupons(&pool, 9501).await, 10, "a second grant pays nothing");
        sqlx::query("UPDATE bound_items SET qty = 4 WHERE account_id = 9501").execute(&pool).await.unwrap();
        assert_eq!(revoke_order(&pool, 7101, 9501, 0, 0).await, Ok(true));
        assert_eq!(coupons(&pool, 9501).await, 0, "the unspent four are taken back, never below zero");
        assert_eq!(purchase_refusal(&st, &pool, 9501, starter).await.unwrap(), Some("purchase limit reached"), "a refunded starter is not sold again (user 2026-10-02)");

        let growth = store::sku("pack_growth_10").unwrap();
        assert_eq!(purchase_refusal(&st, &pool, 9502, growth).await.unwrap(), Some("commander level too low"));
        sqlx::query("INSERT INTO commander (account_id, cxp) VALUES (9502, 1000000000)").execute(&pool).await.unwrap();
        assert_eq!(purchase_refusal(&st, &pool, 9502, growth).await.unwrap(), None);
        sku_order(&pool, 7102, 9502, "pack_growth_10", None).await;
        assert!(matches!(grant_order(&pool, &st.data, 7102).await, Ok(GrantOutcome::Granted { .. })));
        assert_eq!(wallet(&pool, 9502).await, (0, 1_500), "growth crystal is BOUND");
        let mails: Vec<(String,)> = sqlx::query_as("SELECT item_json FROM mailbox WHERE account_id = 9502 AND reason = 'store:7102' ORDER BY id")
            .fetch_all(&pool).await.unwrap();
        assert_eq!(mails.len(), growth.items.len(), "one mail per item line");
        for (json,) in &mails {
            let it: sim_core::items::Item = serde_json::from_str(json).unwrap();
            assert!(it.bound, "pack items are untradeable: {json}");
        }
        assert_eq!(revoke_order(&pool, 7102, 9502, 0, 1_500).await, Ok(true));
        let (left,): (i64,) = sqlx::query_as("SELECT count(*) FROM mailbox WHERE account_id = 9502 AND reason = 'store:7102' AND voided_at IS NULL")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(left, 0, "unclaimed pack mail is withdrawn");
        // Withdrawn, not deleted: a claim the client already took into its sim still verifies.
        let ids: Vec<(i64, String, i64, String)> = sqlx::query_as(
            "SELECT id, def_id, qty, item_json FROM mailbox WHERE account_id = 9502 AND reason = 'store:7102' ORDER BY id",
        ).fetch_all(&pool).await.unwrap();
        assert!(!ids.is_empty());
        assert!(mailbox_store::inbox_for(&pool, 9502).await.unwrap().is_empty(), "not in the inbox");
        {
            let mut conn = pool.acquire().await.unwrap();
            assert_eq!(mailbox_store::claim(&mut conn, 9502, ids[0].0 as u64, now_ms() as i64).await.err(), Some(mailbox::MailError::Withdrawn),
                "a direct claim of a voided mail is Withdrawn, not AlreadyClaimed");
        }
        let (mail_id, def_id, qty, json) = &ids[0];
        let op = sim_core::state::MarketOp::Claim {
            id: 1, mail_id: *mail_id as u64, gil: sim_core::state::Gil::new(0),
            def_id: def_id.clone(), qty: *qty as u32, item: Some(serde_json::from_str(json).unwrap()),
        };
        let mb = mailbox_store::snapshot_for_claims(&pool, 9502, std::slice::from_ref(&op)).await.unwrap();
        let r = mailbox::settle_claims(&mb, &Default::default(), 9502, std::slice::from_ref(&op));
        assert!(r.is_ok(), "an in-flight claim is not a forgery: {:?}", r.err());
        // Past the grace window: refused, and not as a forgery.
        sqlx::query("UPDATE mailbox SET voided_at = voided_at - $2 WHERE id = $1")
            .bind(mail_id).bind(mailbox::WITHDRAWN_GRACE_MS + 1).execute(&pool).await.unwrap();
        let mb = mailbox_store::snapshot_for_claims(&pool, 9502, std::slice::from_ref(&op)).await.unwrap();
        let r = mailbox::settle_claims(&mb, &Default::default(), 9502, &[op]);
        assert!(r.as_ref().is_err_and(|f| mailbox::only_stale_claims(f)), "withdrawn: refused, not flagged: {r:?}");

        // A refund BEFORE delivery bought nothing, so the cap is still open.
        sku_order(&pool, 7105, 9505, "pack_starter", None).await;
        assert_eq!(revoke_order(&pool, 7105, 9505, 0, 0).await, Ok(true));
        assert_eq!(purchase_refusal(&st, &pool, 9505, starter).await.unwrap(), None, "an undelivered refund keeps the cap open");
        assert_eq!(wallet(&pool, 9502).await, (0, 0));
    }

    /// Monthly card: 300 PAID crystal on purchase, 150 bound a day while it runs, one claim a day,
    /// at most two cards banked, and a refund removes the 30 days.
    #[tokio::test]
    async fn pg_monthly_card_pays_daily_and_banks_at_most_two() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let token = seed_session(&st, 9503, 9503);
        let auth = || {
            let mut h = HeaderMap::new();
            h.insert("authorization", format!("Bearer {token}").parse().unwrap());
            h
        };
        let card = store::sku("monthly_card").unwrap();
        let today = daily::day_of_ms(now_ms() as u64);
        let (code, _) = monthly_claim(State(st.clone()), auth()).await;
        assert_eq!(code, StatusCode::CONFLICT, "no card, nothing to claim");
        sku_order(&pool, 7103, 9503, "monthly_card", None).await;
        assert!(matches!(grant_order(&pool, &st.data, 7103).await, Ok(GrantOutcome::Granted { .. })));
        assert_eq!(wallet(&pool, 9503).await, (300, 0), "300 paid crystal, no first-buy bonus");
        let (code, Json(v)) = monthly_claim(State(st.clone()), auth()).await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(v["crystal"].as_i64(), Some(150));
        let (code, _) = monthly_claim(State(st.clone()), auth()).await;
        assert_eq!(code, StatusCode::CONFLICT, "once a day");
        assert_eq!(wallet(&pool, 9503).await, (300, 150));
        assert_eq!(purchase_refusal(&st, &pool, 9503, card).await.unwrap(), None, "a second card may be banked");
        sku_order(&pool, 7104, 9503, "monthly_card", None).await;
        assert!(matches!(grant_order(&pool, &st.data, 7104).await, Ok(GrantOutcome::Granted { .. })));
        let (ends,): (i64,) = sqlx::query_as("SELECT ends_day FROM monthly_card WHERE account_id = 9503").fetch_one(&pool).await.unwrap();
        assert_eq!(ends, today + 60, "the second card extends the first");
        assert_eq!(purchase_refusal(&st, &pool, 9503, card).await.unwrap(), Some("monthly card already banked"));
        assert_eq!(revoke_order(&pool, 7104, 9503, 300, 0).await, Ok(true));
        let (ends,): (i64,) = sqlx::query_as("SELECT ends_day FROM monthly_card WHERE account_id = 9503").fetch_one(&pool).await.unwrap();
        assert_eq!(ends, today + 30, "the refund removes that card's 30 days");
        assert_eq!(wallet(&pool, 9503).await, (300, 150), "today's claim was the FIRST card's: it stays");
        // The first card: today is claimed, so 29 unused days go and today's 150 comes back.
        assert_eq!(revoke_order(&pool, 7103, 9503, 0, 0).await, Ok(true));
        let (ends,): (i64,) = sqlx::query_as("SELECT ends_day FROM monthly_card WHERE account_id = 9503").fetch_one(&pool).await.unwrap();
        assert_eq!(ends, today + 1, "only the claimed day is left");
        assert_eq!(wallet(&pool, 9503).await, (0, 0), "amounts are read under the lock, not from the caller");
    }

    /// AUDIT 2026-10-03: refunding the FIRST of two banked monthly cards moves the second card's
    /// window down by the days given up, so a later refund of the second finds its own claims.
    #[tokio::test]
    async fn pg_refunding_the_first_monthly_card_shifts_the_second_cards_window() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let token = seed_session(&st, 9507, 9507);
        let mut h = HeaderMap::new();
        h.insert("authorization", format!("Bearer {token}").parse().unwrap());
        let today = daily::day_of_ms(now_ms() as u64);
        for o in [7108, 7109] {
            sku_order(&pool, o, 9507, "monthly_card", None).await;
            assert!(matches!(grant_order(&pool, &st.data, o).await, Ok(GrantOutcome::Granted { .. })));
        }
        let (code, _) = monthly_claim(State(st.clone()), h).await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(revoke_order(&pool, 7108, 9507, 0, 0).await, Ok(true));
        let (from, to): (Option<i64>, Option<i64>) = sqlx::query_as("SELECT monthly_from_day, monthly_to_day FROM store_orders WHERE order_id = 7109")
            .fetch_one(&pool).await.unwrap();
        assert_eq!((from, to), (Some(today + 1), Some(today + 31)), "the second card now starts tomorrow");
        // Tomorrow's claim (written as the handler writes it) belongs to the second card.
        sqlx::query("UPDATE crystal_wallet SET bound = bound + 150 WHERE account_id = 9507").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES (9507, 0, 150, 'monthly', $1)")
            .bind(format!("day:{}", today + 1)).execute(&pool).await.unwrap();
        assert_eq!(revoke_order(&pool, 7109, 9507, 0, 0).await, Ok(true));
        assert_eq!(wallet(&pool, 9507).await, (0, 0), "both refunds took back everything both cards paid");
    }

    /// AUDIT 2026-10-03: boot never restores a banned account's session row.
    #[tokio::test]
    async fn pg_boot_does_not_restore_a_banned_accounts_session() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        for (id, tok) in [(9607i64, "tok-banned"), (9608, "tok-ok")] {
            sqlx::query("INSERT INTO accounts (account_id, name, seed) VALUES ($1, $2, 1) ON CONFLICT DO NOTHING")
                .bind(id).bind(format!("boot{id}")).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO active_sessions (account_id, token) VALUES ($1, $2)").bind(id).bind(tok).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO account_bans (account_id, until_ms, reason, at_ms) VALUES (9607, NULL, 'test', 0)").execute(&pool).await.unwrap();
        let st = test_state(Some(pool.clone()));
        load_from_db(&pool, &st.sessions, &st.saves, &st.trusted, &st.fuse_rolls, &st.fuse_consumed).await;
        assert!(!st.sessions.by_token.held().contains_key("tok-banned"), "the banned account's session stays dead");
        assert!(st.sessions.by_token.held().contains_key("tok-ok"));
    }

    /// AUDIT 2026-10-03: a new checkout of a SKU retires the account's own unfinished order of it
    /// (the retire runs before Steam is asked, so no Steam key is needed here).
    #[tokio::test]
    async fn pg_a_new_checkout_retires_the_accounts_stale_order_of_that_sku() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let acct = Account { account_id: 9605, name: "steam:76561190000000001".into(), seed: 1 };
        st.sessions.by_token.held().insert("tok9605".into(), acct.clone());
        st.sessions.active.held().insert(9605, "tok9605".into());
        let mut h = HeaderMap::new();
        h.insert("authorization", "Bearer tok9605".parse().unwrap());
        sku_order(&pool, 7220, 9605, "pack_starter", None).await;
        sqlx::query("UPDATE store_orders SET status = 'pending' WHERE order_id = 7220").execute(&pool).await.unwrap();
        sku_order(&pool, 7221, 9606, "pack_starter", None).await;
        sqlx::query("UPDATE store_orders SET status = 'pending' WHERE order_id = 7221").execute(&pool).await.unwrap();
        let _ = store_checkout(State(st.clone()), h, Json(CheckoutReq { sku: "pack_starter".into(), idem_key: "fresh".into() })).await;
        let status = |o: i64| { let pool = pool.clone(); async move {
            sqlx::query_as::<_, (String,)>("SELECT status FROM store_orders WHERE order_id = $1").bind(o).fetch_one(&pool).await.unwrap().0
        } };
        assert_eq!(status(7220).await, "failed", "this account's stale order is retired");
        assert_eq!(status(7221).await, "pending", "another account's is untouched");
    }

    /// AUDIT 2026-10-03: a closed order never reaches FinalizeTxn (Steam would charge for nothing),
    /// and a parked `grant_failed` one is granted without finalizing again.
    #[tokio::test]
    async fn pg_finalize_refuses_closed_orders_and_grants_parked_ones_without_charging() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let token = seed_session(&st, 9506, 9506);
        let auth = || {
            let mut h = HeaderMap::new();
            h.insert("authorization", format!("Bearer {token}").parse().unwrap());
            h
        };
        sku_order(&pool, 7106, 9506, "crystal_300", None).await;
        sqlx::query("UPDATE store_orders SET status = 'failed' WHERE order_id = 7106").execute(&pool).await.unwrap();
        let (code, Json(v)) = store_finalize(State(st.clone()), auth(), Json(FinalizeReq { order_id: "7106".into() })).await;
        assert_eq!((code, v["reason"].as_str()), (StatusCode::CONFLICT, Some("that order is closed")), "a failed order is closed");
        sku_order(&pool, 7107, 9506, "crystal_300", None).await;
        sqlx::query("UPDATE store_orders SET status = 'grant_failed' WHERE order_id = 7107").execute(&pool).await.unwrap();
        let (code, Json(v)) = store_finalize(State(st.clone()), auth(), Json(FinalizeReq { order_id: "7107".into() })).await;
        assert_eq!((code, v["reason"].as_str()), (StatusCode::OK, Some("granted")), "no FinalizeTxn: no Steam key here");
        // An order mid-FinalizeTxn is nobody else's to touch: a second finalize waits.
        sku_order(&pool, 7110, 9506, "crystal_300", None).await;
        sqlx::query("UPDATE store_orders SET status = 'charging', charging_at = now() WHERE order_id = 7110").execute(&pool).await.unwrap();
        let (code, Json(v)) = store_finalize(State(st.clone()), auth(), Json(FinalizeReq { order_id: "7110".into() })).await;
        assert_eq!((code, v["reason"].as_str()), (StatusCode::CONFLICT, Some("payment in progress — retry")));
    }

    /// Battle Pass: the premium track is claimable only with the entitlement the SKU grants, for
    /// the season the order was placed in; a refund closes it again.
    #[tokio::test]
    async fn pg_battle_pass_premium_needs_the_entitlement() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let token = seed_session(&st, 9504, 9504);
        let auth = || {
            let mut h = HeaderMap::new();
            h.insert("authorization", format!("Bearer {token}").parse().unwrap());
            h
        };
        let bp_sku = store::sku("battle_pass").unwrap();
        assert_eq!(purchase_refusal(&st, &pool, 9504, bp_sku).await.unwrap(), Some("no active season"));
        let now = now_ms();
        *st.season.held() = Some(bp::Season {
            season_id: 42, starts_at_ms: now - 1000, ends_at_ms: now + 86_400_000, seed: 1,
            config: bp::default_config(),
        });
        sqlx::query("INSERT INTO bp_state (account_id, season_id, xp) VALUES (9504, 42, 500)").execute(&pool).await.unwrap();
        let claim = |slots: Vec<u32>| bp_claim(State(st.clone()), auth(), Json(BpClaimReq { slots, track: 1 }));
        let (code, _) = claim(vec![1]).await;
        assert_eq!(code, StatusCode::FORBIDDEN, "no premium without the pass");
        assert_eq!(purchase_refusal(&st, &pool, 9504, bp_sku).await.unwrap(), None);
        sku_order(&pool, 7105, 9504, "battle_pass", Some(42)).await;
        assert!(matches!(grant_order(&pool, &st.data, 7105).await, Ok(GrantOutcome::Granted { .. })));
        assert_eq!(purchase_refusal(&st, &pool, 9504, bp_sku).await.unwrap(), Some("battle pass already owned"));
        let (code, Json(v)) = claim(vec![1, 2, 6]).await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(v["mails"].as_array().map(Vec::len), Some(2), "slots 1-2 reached at level 5, slot 6 not");
        let (_, Json(v)) = claim(vec![1, 2]).await;
        assert_eq!(v["mails"].as_array().map(Vec::len), Some(0), "exactly once");
        let (_, Json(free)) = bp_claim(State(st.clone()), auth(), Json(BpClaimReq { slots: vec![1], track: 0 })).await;
        assert_eq!(free["mails"].as_array().map(Vec::len), Some(1), "the free track is its own row");
        assert_eq!(revoke_order(&pool, 7105, 9504, 0, 0).await, Ok(true));
        let (code, _) = claim(vec![3]).await;
        assert_eq!(code, StatusCode::FORBIDDEN, "a refunded pass closes the premium track");
    }

    /// Pass slots that pay currency: Gil by mail, tickets into `bound_items`, crystal into the bound
    /// bucket — each ledgered, each once.
    #[tokio::test]
    async fn pg_battle_pass_pays_tickets_crystal_and_gil() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let acct = 9505u64;
        let token = seed_session(&st, acct, acct);
        let auth = || {
            let mut h = HeaderMap::new();
            h.insert("authorization", format!("Bearer {token}").parse().unwrap());
            h
        };
        let now = now_ms();
        *st.season.held() = Some(bp::Season {
            season_id: 43, starts_at_ms: now - 1000, ends_at_ms: now + 86_400_000, seed: 1,
            config: bp::default_config(),
        });
        sqlx::query("INSERT INTO bp_state (account_id, season_id, xp) VALUES ($1, 43, 5000)")
            .bind(acct as i64).execute(&pool).await.unwrap();
        sku_order(&pool, 7106, acct as i64, "battle_pass", Some(43)).await;
        assert!(matches!(grant_order(&pool, &st.data, 7106).await, Ok(GrantOutcome::Granted { .. })));
        let claim = |track: u8, slots: Vec<u32>| bp_claim(State(st.clone()), auth(), Json(BpClaimReq { slots, track }));
        let qty = |kind: &'static str| {
            let pool = pool.clone();
            async move {
                sqlx::query_as::<_, (i64,)>("SELECT qty FROM bound_items WHERE account_id = $1 AND kind = $2")
                    .bind(acct as i64).bind(kind).fetch_optional(&pool).await.unwrap().map_or(0, |r| r.0)
            }
        };

        // Free: slot 2 = Gil 20,000 (mail), slot 10 = a Standard ticket.
        let (code, Json(v)) = claim(0, vec![2, 10]).await;
        assert_eq!(code, StatusCode::OK, "{v}");
        assert_eq!(v["mails"][0]["gil"], 20_000);
        assert_eq!(v["granted"][0]["kind"], "standard_ticket");
        assert_eq!(qty(gacha::STANDARD_COUPON_KIND).await, 1);
        // Premium: slot 4 = 100 bound crystal, slot 10 = 2 Limited tickets.
        let (code, Json(v)) = claim(1, vec![4, 10]).await;
        assert_eq!(code, StatusCode::OK, "{v}");
        assert_eq!(qty(gacha::LIMITED_COUPON_KIND).await, 2);
        assert_eq!(wallet(&pool, acct as i64).await, (0, 100));
        let (_, Json(v)) = claim(1, vec![4, 10]).await;
        assert_eq!(v["reason"], "nothing new to claim");
        assert_eq!(wallet(&pool, acct as i64).await, (0, 100), "once");
        let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM bound_ledger WHERE account_id = $1 AND reason = 'bp'")
            .bind(acct as i64).fetch_one(&pool).await.unwrap();
        assert_eq!(n, 2, "each ticket grant is ledgered");
    }

    /// A weekly rung pays the season's pass XP alongside its crystal, once.
    #[tokio::test]
    async fn pg_weekly_rungs_pay_pass_xp() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let acct = 9506u64;
        let token = seed_session(&st, acct, acct);
        let auth = || {
            let mut h = HeaderMap::new();
            h.insert("authorization", format!("Bearer {token}").parse().unwrap());
            h
        };
        let now = now_ms();
        *st.season.held() = Some(bp::Season {
            season_id: 44, starts_at_ms: now - 1000, ends_at_ms: now + 86_400_000, seed: 1,
            config: bp::default_config(),
        });
        let week = weekly::week_of_ms(now as u64);
        // Three commissions done = three rungs reached.
        sqlx::query("INSERT INTO weekly_progress (account_id, week, done_mask, milestone_mask, claimed_mask) VALUES ($1, $2, $3, 0, 0) \
                     ON CONFLICT (account_id, week) DO UPDATE SET done_mask = EXCLUDED.done_mask, claimed_mask = 0")
            .bind(acct as i64).bind(week).bind(0b111i16).execute(&pool).await.unwrap();
        let (code, Json(v)) = weekly_claim(State(st.clone()), auth(), Json(WeeklyClaimReq { week })).await;
        assert_eq!(code, StatusCode::OK, "{v}");
        assert_eq!(v["claimed_rungs"], 3);
        assert_eq!(v["xp"], 3 * bp::DEFAULT_XP_PER_WEEKLY_RUNG);
        let (_, Json(v)) = weekly_claim(State(st.clone()), auth(), Json(WeeklyClaimReq { week })).await;
        assert_eq!(v["claimed_rungs"], 0);
        let (xp,): (i64,) = sqlx::query_as("SELECT xp FROM bp_state WHERE account_id = $1 AND season_id = 44")
            .bind(acct as i64).fetch_one(&pool).await.unwrap();
        assert_eq!(xp as u64, 3 * bp::DEFAULT_XP_PER_WEEKLY_RUNG, "once");
    }

    /// Offline EXP: a gap of two hours mails an `offline_exp` envelope worth the sim's rate for
    /// the save's band; a reconnect-sized gap and a first save mail nothing.
    #[tokio::test]
    async fn pg_offline_gap_mails_an_exp_envelope() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let acct = 9508u64;
        let mut g = data::new_game_as(1, &st.data, "zone_b1_1", "suzu").unwrap();
        g.band_reached = 20;
        let mut tx = pool.begin().await.unwrap();
        assert!(offline_exp_mail(&mut tx, &st, acct, &g, None).await.unwrap().is_none(), "first save");
        assert!(offline_exp_mail(&mut tx, &st, acct, &g, Some(5 * 60_000)).await.unwrap().is_none(), "a reconnect");
        let mail = offline_exp_mail(&mut tx, &st, acct, &g, Some(2 * 3_600_000)).await.unwrap().expect("two hours away");
        tx.commit().await.unwrap();
        let it = mail.item.expect("an envelope");
        assert_eq!(it.def_id, OFFLINE_EXP_DEF);
        assert_eq!(u64::from(it.quantity), sim_core::tick::offline_xp_per_member(&g, &st.data, 120));
        assert!(it.quantity > 0 && it.bound);
        assert_eq!(mail.reason, "offline 120m");
    }

    /// `/badges` counts what each claim endpoint would pay, and drops back once it is claimed.
    #[tokio::test]
    async fn pg_badges_count_what_the_claims_would_pay() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let acct = 9507u64;
        let token = seed_session(&st, acct, acct);
        let auth = || {
            let mut h = HeaderMap::new();
            h.insert("authorization", format!("Bearer {token}").parse().unwrap());
            h
        };
        let (code, Json(v)) = badges(State(st.clone()), auth()).await;
        assert_eq!(code, StatusCode::OK, "{v}");
        assert_eq!((v["daily_rungs"].as_u64(), v["mail"].as_i64(), v["tickets_std"].as_i64()), (Some(0), Some(0), Some(0)));

        let week = weekly::week_of_ms(now_ms() as u64);
        sqlx::query("INSERT INTO weekly_progress (account_id, week, done_mask, milestone_mask, claimed_mask) VALUES ($1, $2, 3, 0, 0) \
                     ON CONFLICT (account_id, week) DO UPDATE SET done_mask = 3, claimed_mask = 0")
            .bind(acct as i64).bind(week).execute(&pool).await.unwrap();
        mailbox_store::send(&pool, acct, mailbox::MailKind::Reward, 100, "", 0, "test").await.unwrap();
        sqlx::query("INSERT INTO bound_items (account_id, kind, qty) VALUES ($1, $2, 4)")
            .bind(acct as i64).bind(gacha::STANDARD_COUPON_KIND).execute(&pool).await.unwrap();
        let (_, Json(v)) = badges(State(st.clone()), auth()).await;
        assert_eq!(v["weekly_rungs"], 2);
        assert_eq!(v["mail"], 1);
        assert_eq!(v["tickets_std"], 4);
        assert!(v["commander_levels"].as_u64() >= Some(1), "level 1 is itself a reward: {v}");

        let (_, Json(c)) = weekly_claim(State(st.clone()), auth(), Json(WeeklyClaimReq { week })).await;
        assert_eq!(c["claimed_rungs"], 2);
        let (_, Json(v)) = badges(State(st.clone()), auth()).await;
        assert_eq!(v["weekly_rungs"], 0, "claimed rungs no longer light the dot");
    }

    /// THE BANNER FLOW (2026-09-30), through the real handlers: the Starter banner stays open past
    /// an SSR and pays every SR as a character the account neither holds nor has in its mailbox;
    /// Standard is refused until the Starter 50 are spent; mileage lands in the banner's own
    /// currency; the Standard selector pays once, at 100 Standard pulls.
    #[tokio::test]
    async fn pg_starter_then_standard_banner_flow() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let acct = 9601u64;
        let token = seed_session(&st, acct, acct);
        let auth = || {
            let mut h = HeaderMap::new();
            h.insert("authorization", format!("Bearer {token}").parse().unwrap());
            h
        };
        sqlx::query("INSERT INTO accounts (account_id, name, seed) VALUES ($1, 'banner', 1) ON CONFLICT DO NOTHING")
            .bind(acct as i64).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO bound_items (account_id, kind, qty) VALUES ($1, $2, 1000)")
            .bind(acct as i64).bind(gacha::STANDARD_COUPON_KIND).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO bound_items (account_id, kind, qty) VALUES ($1, $2, 7)")
            .bind(acct as i64).bind(gacha::LIMITED_COUPON_KIND).execute(&pool).await.unwrap();
        let qty = |kind: &'static str| {
            let pool = pool.clone();
            async move {
                sqlx::query_as::<_, (i64,)>("SELECT qty FROM bound_items WHERE account_id = $1 AND kind = $2")
                    .bind(acct as i64).bind(kind).fetch_one(&pool).await.unwrap().0
            }
        };
        let roll = |banner: &str, n: u32, key: String| gacha_roll(State(st.clone()), auth(), Json(GachaRollReq {
            banner_id: banner.into(), count: n, idem_key: key, buy_shortfall: false, quoted_crystal: None,
        }));

        let (code, _) = roll("standard", 10, "std-early".into()).await;
        assert_eq!(code, StatusCode::CONFLICT, "Standard stays shut until the Starter 50 are spent");

        let mut chars: Vec<String> = Vec::new();
        for i in 0..5 {
            let std_before = qty(gacha::STANDARD_COUPON_KIND).await;
            let (code, Json(v)) = roll("newcomer", 10, format!("starter-{i}")).await;
            assert_eq!(code, StatusCode::OK, "starter pull {i}: {v}");
            for p in v["pulls"].as_array().unwrap() {
                let def = p["def_id"].as_str().unwrap();
                // A repeat is legal only once every SR character is owned (then the ordinary SR
                // payout applies).
                let all_sr_owned = gacha::SR_UNITS.iter().all(|u| chars.iter().any(|c| c == u));
                if p["rarity"] == "Sr" && def.starts_with("exemplar_") && !all_sr_owned {
                    assert!(!chars.contains(&def.to_string()), "SR {def} paid twice while new SR characters remained");
                }
                if def.starts_with("exemplar_") {
                    chars.push(def.to_string());
                }
            }
            assert_eq!(v["mileage_kind"], "standard");
            // A Starter x10 costs 10 Standard tickets (no discount); Limited tickets are never touched.
            assert_eq!(qty(gacha::STANDARD_COUPON_KIND).await, std_before - 10, "Starter spends Standard tickets");
            assert_eq!(qty(gacha::LIMITED_COUPON_KIND).await, 7, "Limited tickets do not spend on the Starter");
        }
        let (made, closed): (i32, bool) = sqlx::query_as("SELECT made, closed FROM gacha_newcomer WHERE account_id = $1")
            .bind(acct as i64).fetch_one(&pool).await.unwrap();
        assert_eq!((made, closed), (50, true), "open for all 50, closed at the fiftieth");
        let (code, _) = roll("newcomer", 1, "starter-51".into()).await;
        assert_eq!(code, StatusCode::CONFLICT, "no 51st Starter pull");
        let (std_tickets,): (i64,) = sqlx::query_as("SELECT tickets FROM gacha_mileage_std WHERE account_id = $1")
            .bind(acct as i64).fetch_one(&pool).await.unwrap();
        assert_eq!(std_tickets, 50, "Starter pulls mint Standard mileage");

        let (code, Json(v)) = roll("standard", 10, "std-1".into()).await;
        assert_eq!(code, StatusCode::OK, "{v}");
        let (m,): (i32,) = sqlx::query_as("SELECT made FROM gacha_standard WHERE account_id = $1")
            .bind(acct as i64).fetch_one(&pool).await.unwrap();
        assert_eq!(m, 10);

        let select = |def: &str| gacha_standard_select(State(st.clone()), auth(), Json(StandardSelectReq { def_id: def.into() }));
        let (code, _) = select("exemplar_seraphine").await;
        assert_eq!(code, StatusCode::CONFLICT, "not before 100 Standard pulls");
        sqlx::query("UPDATE gacha_standard SET made = 100 WHERE account_id = $1").bind(acct as i64).execute(&pool).await.unwrap();
        let (code, _) = select("exemplar_aveline").await;
        assert_eq!(code, StatusCode::BAD_REQUEST, "an SR is not on the selector");
        let (code, Json(v)) = select("exemplar_seraphine").await;
        assert_eq!((code, v["reason"].as_str()), (StatusCode::OK, Some("selected")));
        let (code, Json(v)) = select("exemplar_seraphine").await;
        assert_eq!((code, v["reason"].as_str()), (StatusCode::OK, Some("idempotent")));
        let (code, _) = select("exemplar_lapis").await;
        assert_eq!(code, StatusCode::CONFLICT, "once per account");
        let (mails,): (i64,) = sqlx::query_as("SELECT count(*) FROM mailbox WHERE account_id = $1 AND reason = 'gacha standard select'")
            .bind(acct as i64).fetch_one(&pool).await.unwrap();
        assert_eq!(mails, 1, "the selector paid exactly once");

        // Standard mileage buys a Memory Fragment, never a character.
        sqlx::query("UPDATE gacha_mileage_std SET tickets = 400 WHERE account_id = $1").bind(acct as i64).execute(&pool).await.unwrap();
        let ex = |def: &str, key: &str| gacha_exchange(State(st.clone()), auth(), Json(GachaExchangeReq {
            def_id: def.into(), idem_key: key.into(), currency: Some("standard".into()),
        }));
        let (code, _) = ex("exemplar_nemu", "ex-1").await;
        assert_ne!(code, StatusCode::OK, "standard mileage cannot buy a character");
        let (code, Json(v)) = ex("regalia_aurora_edge", "ex-2").await;
        assert_eq!(code, StatusCode::OK, "{v}");
        assert_eq!(v["mileage"], "200");
    }

    /// The universal pre-split coupon is deleted on boot, balance and ledger both; the two kinds
    /// that replaced it are untouched.
    #[tokio::test]
    async fn pg_migrate_deletes_the_old_universal_coupon() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        for kind in ["gacha_coupon", gacha::STANDARD_COUPON_KIND, gacha::LIMITED_COUPON_KIND] {
            sqlx::query("INSERT INTO bound_items (account_id, kind, qty) VALUES (9701, $1, 5)").bind(kind).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO bound_ledger (account_id, kind, delta, reason) VALUES (9701, $1, 5, 'test')").bind(kind).execute(&pool).await.unwrap();
        }
        migrate(&pool).await.expect("migrate");
        let kinds: Vec<(String,)> = sqlx::query_as("SELECT kind FROM bound_items WHERE account_id = 9701 ORDER BY kind")
            .fetch_all(&pool).await.unwrap();
        assert_eq!(kinds.into_iter().map(|(k,)| k).collect::<Vec<_>>(), vec![gacha::LIMITED_COUPON_KIND.to_string(), gacha::STANDARD_COUPON_KIND.to_string()], "sorted by kind");
        let (old,): (i64,) = sqlx::query_as("SELECT count(*) FROM bound_ledger WHERE account_id = 9701 AND kind = 'gacha_coupon'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(old, 0, "the old kind's ledger rows are gone too");
    }

    /// Stage CXP: every stage past the watermark pays once and a save behind the mark pays nothing.
    #[tokio::test]
    async fn pg_commander_cxp_from_new_stages_pays_once() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let acct = 9801u64;
        let cxp = |pool: sqlx::PgPool| async move {
            sqlx::query_as::<_, (i64,)>("SELECT cxp FROM commander WHERE account_id = $1")
                .bind(acct as i64).fetch_optional(&pool).await.unwrap().map_or(0, |r| r.0)
        };
        let run = |pool: sqlx::PgPool, reached: u32| async move {
            let mut tx = pool.begin().await.unwrap();
            commander_stage_cxp(&mut tx, acct, reached).await.unwrap();
            tx.commit().await.unwrap();
        };
        run(pool.clone(), 5).await;
        assert_eq!(cxp(pool.clone()).await, 5 * commander::CXP_PER_NEW_STAGE, "stages 1-5");
        run(pool.clone(), 5).await;
        assert_eq!(cxp(pool.clone()).await, 5 * commander::CXP_PER_NEW_STAGE, "the same stage never pays twice");
        run(pool.clone(), 3).await;
        assert_eq!(cxp(pool.clone()).await, 5 * commander::CXP_PER_NEW_STAGE, "an older save pays nothing");
        run(pool.clone(), 7).await;
        assert_eq!(cxp(pool.clone()).await, 7 * commander::CXP_PER_NEW_STAGE);
    }

    /// Claiming daily rungs pays Commander CXP with the crystal, per rung, exactly once.
    #[tokio::test]
    async fn pg_daily_claim_pays_commander_cxp_per_rung() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let acct = 9802u64;
        let token = seed_session(&st, acct, acct);
        let auth = || {
            let mut h = HeaderMap::new();
            h.insert("authorization", format!("Bearer {token}").parse().unwrap());
            h
        };
        let day = daily::day_of_ms(now_ms() as u64);
        // Three commissions done: 300 points, rungs 1-3 claimable.
        sqlx::query("INSERT INTO daily_progress (account_id, day, done_mask, milestone_mask, claimed_mask, start_values_json) VALUES ($1, $2, 7, 0, 0, '[]')")
            .bind(acct as i64).bind(day).execute(&pool).await.unwrap();
        let cxp = || {
            let pool = pool.clone();
            async move {
                sqlx::query_as::<_, (i64,)>("SELECT cxp FROM commander WHERE account_id = $1")
                    .bind(acct as i64).fetch_optional(&pool).await.unwrap().map_or(0, |r| r.0)
            }
        };
        let (_, Json(v)) = daily_claim(State(st.clone()), auth(), Json(DailyClaimReq { day })).await;
        assert_eq!(v["claimed_rungs"].as_u64(), Some(3), "{v}");
        assert_eq!(cxp().await, 3 * commander::CXP_PER_DAILY_RUNG, "100 CXP per rung, with the crystal");
        let (_, Json(v)) = daily_claim(State(st.clone()), auth(), Json(DailyClaimReq { day })).await;
        assert_eq!(v["claimed_rungs"].as_u64(), Some(0));
        assert_eq!(cxp().await, 3 * commander::CXP_PER_DAILY_RUNG, "a second press pays no CXP");
    }

    /// Tutorial quests through the real handlers: quest 1 pays a Rare weapon of the lead's family
    /// by mail and CXP, once; a done quest in a later chapter stays locked; a party of two pays the
    /// Standard ticket.
    #[tokio::test]
    async fn pg_tutorial_quests_pay_once_in_chapter_order() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let acct = 9901u64;
        let token = seed_session(&st, acct, acct);
        let auth = || {
            let mut h = HeaderMap::new();
            h.insert("authorization", format!("Bearer {token}").parse().unwrap());
            h
        };
        let mut g = data::new_game_as(1, &st.data, "zone_b1_1", "suzu").unwrap();
        g.measures.boss_kills.add(1);
        g.measures.fusions.add(1);
        let put = |g: &sim_core::state::GameState| {
            st.saves.held().insert(acct, StoredSave {
                state_json: serde_json::to_string(g).unwrap(), final_tick: 0, final_hash: String::new(), saved_at_ms: 0,
            });
        };
        put(&g);
        let claim = |id: &str| tutorial_claim(State(st.clone()), auth(), Json(TutorialClaimReq { id: id.into() }));

        let (_, Json(state)) = tutorial_state(State(st.clone()), auth()).await;
        assert_eq!(state["open_chapter"], 1);
        let (code, _) = claim("fuse_gear").await;
        assert_eq!(code, StatusCode::CONFLICT, "done, but chapter 2 is locked");
        let (code, Json(v)) = claim("first_boss").await;
        assert_eq!(code, StatusCode::OK, "{v}");
        let (code, _) = claim("first_boss").await;
        assert_eq!(code, StatusCode::CONFLICT, "once");
        let (item,): (String,) = sqlx::query_as("SELECT item_json FROM mailbox WHERE account_id = $1 AND reason = 'tutorial first_boss'")
            .bind(acct as i64).fetch_one(&pool).await.unwrap();
        let it: sim_core::items::Item = serde_json::from_str(&item).unwrap();
        let fam = sim_core::items::weapon_types_for_character(None, "suzu");
        assert!(fam.contains(&it.weapon_type), "the lead's family: {:?}", it.weapon_type);
        assert_eq!((it.grade, it.item_level(), it.bound), (sim_core::items::Grade::Rare, 1, true));
        let (cxp,): (i64,) = sqlx::query_as("SELECT cxp FROM commander WHERE account_id = $1").bind(acct as i64).fetch_one(&pool).await.unwrap();
        assert_eq!(cxp, 100);

        let (code, _) = claim("deploy_second").await;
        assert_eq!(code, StatusCode::CONFLICT, "one member is not two");
        g.party_slots = 2;
        let id = data::grant_exemplar_unit(&mut g, &st.data, "lumi").expect("lumi");
        data::deploy(&mut g, &st.data, id).expect("deploy");
        put(&g);
        let (code, Json(v)) = claim("deploy_second").await;
        assert_eq!(code, StatusCode::OK, "{v}");
        let (tickets,): (i64,) = sqlx::query_as("SELECT qty FROM bound_items WHERE account_id = $1 AND kind = $2")
            .bind(acct as i64).bind(gacha::STANDARD_COUPON_KIND).fetch_one(&pool).await.unwrap();
        assert_eq!(tickets, 1, "a Standard ticket, with its ledger row");
    }

    /// The crystal shop: bound crystal spent first, items mailed BOUND, a repeat key charges
    /// nothing, and the eleventh Elixir of the day is refused whole.
    #[tokio::test]
    async fn pg_shop_buys_spend_bound_first_and_stop_at_the_daily_cap() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let acct = 9902u64;
        let token = seed_session(&st, acct, acct);
        let auth = || {
            let mut h = HeaderMap::new();
            h.insert("authorization", format!("Bearer {token}").parse().unwrap());
            h
        };
        sqlx::query("INSERT INTO crystal_wallet (account_id, paid, bound) VALUES ($1, 5000, 1000)")
            .bind(acct as i64).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES ($1, 5000, 1000, 'dev-grant', '')")
            .bind(acct as i64).execute(&pool).await.unwrap();
        let buy = |qty: u32, key: &str| shop_buy(State(st.clone()), auth(),
            Json(ShopBuyReq { def_id: "exp_elixir".into(), qty, idem_key: key.into() }));

        let (code, Json(v)) = buy(8, "k1").await;
        assert_eq!(code, StatusCode::OK, "{v}");
        assert_eq!(v["crystal"], 960);
        assert_eq!(wallet(&pool, acct as i64).await, (5000, 40), "bound first");
        let (code, Json(v)) = buy(8, "k1").await;
        assert_eq!((code, v["reason"].as_str()), (StatusCode::OK, Some("idempotent")));
        assert_eq!(wallet(&pool, acct as i64).await, (5000, 40), "a repeat key charges nothing");
        let (code, _) = buy(3, "k2").await;
        assert_eq!(code, StatusCode::CONFLICT, "8 + 3 crosses the cap of 10");
        let (code, Json(v)) = buy(2, "k3").await;
        assert_eq!(code, StatusCode::OK, "{v}");
        assert_eq!(wallet(&pool, acct as i64).await, (4800, 0), "the rest from paid");
        let (code, _) = buy(1, "k4").await;
        assert_eq!(code, StatusCode::CONFLICT);

        let rows: Vec<(String,)> = sqlx::query_as("SELECT item_json FROM mailbox WHERE account_id = $1 AND reason = 'shop exp_elixir' ORDER BY id")
            .bind(acct as i64).fetch_all(&pool).await.unwrap();
        let qty: Vec<(u32, bool)> = rows.iter().map(|(j,)| {
            let it: sim_core::items::Item = serde_json::from_str(j).unwrap();
            (it.quantity, it.bound)
        }).collect();
        assert_eq!(qty, vec![(8, true), (2, true)]);
        let (spent,): (i64,) = sqlx::query_as("SELECT sum(d_paid + d_bound)::bigint FROM crystal_ledger WHERE account_id = $1 AND reason = 'shop'")
            .bind(acct as i64).fetch_one(&pool).await.unwrap();
        assert_eq!(spent, -1200);
        let (_, Json(v)) = shop_state(State(st.clone()), auth()).await;
        assert_eq!(v["items"][0]["left_today"], 0);

        // A refund left the account in debt: nothing crystal can buy until it is repaid.
        sqlx::query("UPDATE crystal_wallet SET paid = -10, bound = 5000 WHERE account_id = $1")
            .bind(acct as i64).execute(&pool).await.unwrap();
        sqlx::query("DELETE FROM shop_buys WHERE account_id = $1").bind(acct as i64).execute(&pool).await.unwrap();
        let (code, Json(v)) = buy(1, "k5").await;
        assert_eq!((code, v["reason"].as_str()), (StatusCode::CONFLICT, Some("crystal balance is negative")));
    }

    /// CHARGED BUT NOT GRANTED is recoverable: an order left at `charged` is paid by the sweep
    /// exactly once (first-buy bonus included), a refunded one never, and the admin grant refuses
    /// an order Steam may not have charged unless the operator forces it.
    #[tokio::test]
    async fn pg_a_charged_order_is_granted_once_by_the_sweep_and_never_after_a_refund() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        order_row(&pool, 7001, 9401, "charged").await;
        order_row(&pool, 7002, 9402, "refunded").await;
        sweep_charged_orders(&pool, &test_state(None).data).await;
        assert_eq!(wallet(&pool, 9401).await, (300, 300), "300 + the first-buy bonus");
        sweep_charged_orders(&pool, &test_state(None).data).await;
        assert_eq!(wallet(&pool, 9401).await, (300, 300), "a second sweep pays nothing");
        let (status,): (String,) = sqlx::query_as("SELECT status FROM store_orders WHERE order_id = 7001").fetch_one(&pool).await.unwrap();
        assert_eq!(status, "granted");
        assert_eq!(wallet(&pool, 9402).await, (0, 0), "a refunded order is never granted");

        let mut st = test_state(Some(pool.clone()));
        st.admin_key = Some("adm".into());
        let app = build_router(st.clone(), gated_routes(&st));
        order_row(&pool, 7003, 9403, "pending").await;
        let grant = |force: bool| axum::http::Request::builder().method("POST").uri("/admin/store/grant")
            .header("content-type", "application/json").header("x-admin-key", "adm")
            .body(axum::body::Body::from(format!("{{\"order_id\":\"7003\",\"force\":{force}}}"))).unwrap();
        let (code, _) = call(&app, grant(false)).await;
        assert_eq!(code, StatusCode::CONFLICT, "pending needs the operator to vouch for the charge");
        assert_eq!(wallet(&pool, 9403).await, (0, 0));
        let (code, body) = call(&app, grant(true)).await;
        assert_eq!(code, StatusCode::OK, "{body}");
        assert_eq!(wallet(&pool, 9403).await, (300, 300));
        let (_, body) = call(&app, grant(true)).await;
        assert!(body.contains("idempotent"), "a forced re-grant still pays once: {body}");
        assert_eq!(wallet(&pool, 9403).await, (300, 300));
    }

    /// A ban ends the live session, refuses every login until it lapses or is lifted, and a
    /// temporary one lapses on its own.
    #[tokio::test]
    async fn pg_a_ban_ends_the_session_and_refuses_login_until_lifted() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let mut st = test_state(Some(pool.clone()));
        st.admin_key = Some("adm".into());
        st.dev_key = Some("dk".into());
        let app = build_router(st.clone(), gated_routes(&st));
        let post = |path: &str, body: String| axum::http::Request::builder().method("POST").uri(path)
            .header("content-type", "application/json").header("x-admin-key", "adm").header("x-dev-key", "dk")
            .body(axum::body::Body::from(body)).unwrap();
        let login = || post("/auth/dev", "{\"name\":\"ban_target\"}".into());

        let (code, body) = call(&app, login()).await;
        assert_eq!(code, StatusCode::OK, "{body}");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        let acct = v["account_id"].as_u64().unwrap();
        let token = v["token"].as_str().unwrap().to_string();
        assert!(st.sessions.by_token.held().contains_key(&token));

        let (code, _) = call(&app, post("/admin/ban", format!("{{\"account_id\":{acct},\"reason\":\"test\"}}"))).await;
        assert_eq!(code, StatusCode::OK);
        assert!(!st.sessions.by_token.held().contains_key(&token), "the live session ends at once");
        let (code, body) = call(&app, login()).await;
        assert_eq!(code, StatusCode::FORBIDDEN, "a banned account cannot sign in: {body}");
        assert!(body.contains("suspended"), "{body}");

        let (code, _) = call(&app, post("/admin/unban", format!("{{\"account_id\":{acct}}}"))).await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(call(&app, login()).await.0, StatusCode::OK, "lifted");

        // A suspension that has already run out does not block.
        sqlx::query("INSERT INTO account_bans (account_id, until_ms, reason, at_ms) VALUES ($1, 1, 'old', 0) \
                     ON CONFLICT (account_id) DO UPDATE SET until_ms = 1").bind(acct as i64).execute(&pool).await.unwrap();
        assert_eq!(call(&app, login()).await.0, StatusCode::OK, "an expired suspension lapses");
        // A ban needs a reason.
        let (code, _) = call(&app, post("/admin/ban", format!("{{\"account_id\":{acct},\"reason\":\" \"}}"))).await;
        assert_eq!(code, StatusCode::BAD_REQUEST);
    }

    /// A mail the account ALREADY claimed, presented again (a stale inbox view), is refused — it
    /// would pay twice — but is NOT a forgery: no flag, no taint. The same mail claimed from
    /// someone else's account stays a flagged forgery.
    #[tokio::test]
    async fn pg_re_claiming_your_own_claimed_mail_is_refused_but_not_flagged() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let app = build_router(st.clone(), Router::new());
        let token = seed_session(&st, 9360, 9360);
        let mail = mailbox_store::send(&pool, 9360, MailKind::Reward, 7600, "", 0, "test").await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        mailbox_store::claim(&mut tx, 9360, mail, 1).await.unwrap();
        tx.commit().await.unwrap();

        let b = claim_bundle(&st.data, 9360, 40, mail, 7600);
        let (code, body) = call(&app, post_json("/save", &token, serde_json::to_string(&b).unwrap())).await;
        assert_eq!(code, StatusCode::OK, "{body}");
        assert!(body.contains("\"ok\":false") && body.contains("already claimed"), "{body}");
        let (flags,): (i64,) = sqlx::query_as("SELECT count(*) FROM cheat_flags WHERE account_id = 9360")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(flags, 0, "a stale claim is not a forgery");
        assert!(!st.sessions.tainted.held().contains(&9360), "and does not taint");

        // Another account presenting that claimed mail is still a forgery.
        let thief = seed_session(&st, 9361, 9361);
        let b = claim_bundle(&st.data, 9361, 40, mail, 7600);
        let (_, body) = call(&app, post_json("/save", &thief, serde_json::to_string(&b).unwrap())).await;
        assert!(body.contains("forged"), "{body}");
        let (flags,): (i64,) = sqlx::query_as("SELECT count(*) FROM cheat_flags WHERE account_id = 9361 AND reason = 'forged-claim'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(flags, 1, "someone else's claimed mail is flagged");
    }

    /// A flagged session settles nothing on the save that clears its own flag.
    ///
    /// The rule (audit F5) is that an account flagged for a PRIOR integrity rejection does not
    /// consume mail on the push that RECOVERS it; the claim stays unconsumed in the hash-covered
    /// log and settles on the next save, after the flag is cleared.
    #[tokio::test]
    async fn pg_a_tainted_session_settles_nothing_on_the_save_that_clears_its_flag() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let app = build_router(st.clone(), Router::new());
        let token = seed_session(&st, 9400, 9400);
        let mail = mailbox_store::send(&pool, 9400, MailKind::Reward, 7600, "", 0, "test").await.unwrap();

        // FLAGGED. The save must still be ACCEPTED — refusing it would strand the account, which
        // is the opposite of what the taint is for.
        st.sessions.tainted.held().insert(9400);
        let b1 = claim_bundle(&st.data, 9400, 40, mail, 7600);
        let (code, body) = call(&app, post_json("/save", &token, serde_json::to_string(&b1).unwrap())).await;
        assert_eq!(code, StatusCode::OK, "a tainted account must still be able to save: {body}");
        assert!(body.contains("\"ok\":true"), "{body}");
        assert!(settled_ops(&body).is_empty(), "a FLAGGED session settled a claim: {body}");
        assert!(claimed_at(&pool, mail).await.is_none(), "the mail stays unclaimed on the recovering save");
        assert!(!st.sessions.tainted.held().contains(&9400), "and the accepted save clears the flag");

        // Recovered: the next save — the next segment from the confirmed state, as the client sends
        // it (the same bundle again is now an idempotent re-send) — settles what the first left alone.
        let s1 = b1.verified_state(&st.data).expect("b1 replays");
        let (b2, _) = bundle_from(&st.data, s1, 80);
        let (code, body) = call(&app, post_json("/save", &token, serde_json::to_string(&b2).unwrap())).await;
        assert_eq!(code, StatusCode::OK, "{body}");
        assert_eq!(settled_ops(&body).len(), 1, "{body}");
        assert!(claimed_at(&pool, mail).await.is_some(),
            "after the flag cleared the claim must settle — a taint that never lets go is a \
             permanent freeze, not a delay");
    }

    /// A forged `final_hash` is refused, and never becomes an anchor.
    ///
    /// `trusted` and the stored row used to take the CLIENT's `bundle.final_hash`. The only thing
    /// binding it to reality was the comparison inside `verified_state` — an audit deleted that
    /// verdict and the whole suite stayed green, because the only well-formed POST /save in it
    /// uses a bundle whose hash is correct by construction, so the stored row was byte-identical
    /// either way. Two pushes then launder a state: push one is an honest log claiming
    /// `final_hash = state_hash(cooked)`, and push two arrives with `start = cooked`, anchored.
    ///
    /// `put_save` now hashes the REPLAYED state, so the field cannot disagree with the replay.
    /// This test pins the outer behaviour: a lie is refused, and nothing it claimed is anchored.
    #[tokio::test]
    async fn pg_a_forged_final_hash_is_refused_and_never_becomes_an_anchor() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let app = build_router(st.clone(), Router::new());
        let token = seed_session(&st, 9200, 9200);

        let mut lying = verifiable_bundle(&st.data, 9200, 40, Vec::new());
        let forged = "f".repeat(64);
        lying.final_hash = forged.clone();

        let (code, body) = call(&app, post_json("/save", &token, serde_json::to_string(&lying).unwrap())).await;
        assert!(!body.contains("\"ok\":true"),
            "a bundle whose final_hash does not match its own replay was accepted: {code} {body}");
        assert!(
            !st.trusted.held().get(&9200).is_some_and(|set| set.contains(&forged)),
            "the forged hash was anchored — the next push can arrive with that state as its START \
             and every save after it chains from a forgery legitimately");
        let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM saves WHERE account_id = 9200")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(n, 0, "and nothing was stored");
        // The flag says WHICH client diverged, so a spike on one build or platform reads as ours.
        let (detail,): (String,) = sqlx::query_as(
            "SELECT detail FROM cheat_flags WHERE account_id = 9200 AND reason = 'hash-mismatch'",
        ).fetch_one(&pool).await.unwrap();
        assert!(detail.contains(&format!("sim={}", sim_core::VERSION)) && detail.contains("platform=unknown"), "{detail}");

        // The honest twin: accepted, and what gets anchored is the REPLAY's hash.
        let honest = verifiable_bundle(&st.data, 9200, 40, Vec::new());
        let real = honest.final_hash.clone();
        let (code, body) = call(&app, post_json("/save", &token, serde_json::to_string(&honest).unwrap())).await;
        assert_eq!(code, StatusCode::OK, "{body}");
        assert!(st.trusted.held().get(&9200).is_some_and(|set| set.contains(&real)),
            "an accepted save must anchor, or the next resume cannot chain from it");
    }

    /// `MAX_REPLAY_SPAN` refuses an over-long run, driven rather than compared to a constant.
    ///
    /// The only test naming the constant parsed a number out of `sim_driver_core.gd` and compared
    /// the two. Zeroing the span cap's input left the suite green while the function runs on every
    /// POST /save.
    #[tokio::test]
    async fn pg_a_run_longer_than_the_replay_span_is_refused() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let app = build_router(st.clone(), Router::new());
        let token = seed_session(&st, 9300, 9300);

        let over = verifiable_bundle(&st.data, 9300, MAX_REPLAY_SPAN + 1, Vec::new());
        let (code, body) = call(&app, post_json("/save", &token, serde_json::to_string(&over).unwrap())).await;
        assert_eq!(code, StatusCode::BAD_REQUEST,
            "a run longer than MAX_REPLAY_SPAN was not refused — the cap is what bounds how much \
             CPU one authenticated request can buy: {body}");
        assert!(body.contains("replay span"), "and it must say which limit: {body}");
    }

    /// A log entry cannot date itself outside the run, so the replay cannot outrun every budget.
    ///
    /// `replay` advances to each entry's OWN tick before it looks at `final_tick`, and nothing on
    /// the server read that number: the span was computed from `final_tick` alone, and both
    /// fast-forward layers are fed `final_tick`. An audit took the same 20_000 ticks the server
    /// refuses when they are declared honestly, relabelled them into the log with `final_tick: 0`,
    /// and got 200 — with a server-rolled gear grant minted off the free ticks and `session_excess`
    /// going NEGATIVE, which CREDITS layer 2 instead of tripping it.
    ///
    /// Driven through the real router, because the point is what the server accepts.
    #[tokio::test]
    async fn pg_a_log_entry_cannot_buy_ticks_the_budget_never_sees() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let app = build_router(st.clone(), Router::new());
        let token = seed_session(&st, 9100, 9100);

        // The honest control: a bundle that DECLARES its ticks is measured and refused.
        let honest = verifiable_bundle(&st.data, 9100, 60_000, Vec::new());
        let (code, body) = call(&app, post_json("/save", &token, serde_json::to_string(&honest).unwrap())).await;
        assert_ne!(code, StatusCode::OK, "a 60k-tick run declared honestly must not be accepted: {body}");

        // The same reach, relabelled into the log. Before the fix this was 200 plus a gear grant.
        let mut sneaky = verifiable_bundle(&st.data, 9100, 40, Vec::new());
        sneaky.log = vec![sim_core::events::RecordedInput {
            tick: 60_000,
            event: InputEvent::SetFarmMode { on: true },
        }];
        let (code, body) = call(&app, post_json("/save", &token, serde_json::to_string(&sneaky).unwrap())).await;
        assert_ne!(code, StatusCode::OK,
            "a log entry dated 60_000 inside a run declared as 40 ticks was ACCEPTED — the replay \
             advances to the LOG's tick, so this buys 60_000 ticks of simulation, drop rolls and \
             daily accrual that no limit and neither fast-forward layer measures: {body}");
        assert!(body.contains("outside the run"), "and it must say why: {body}");

        // Backwards time is refused too — the log is append-ordered by construction.
        let mut backwards = verifiable_bundle(&st.data, 9100, 100, Vec::new());
        backwards.log = vec![
            sim_core::events::RecordedInput { tick: 90, event: InputEvent::SetFarmMode { on: true } },
            sim_core::events::RecordedInput { tick: 10, event: InputEvent::SetFarmMode { on: false } },
        ];
        let (code, _) = call(&app, post_json("/save", &token, serde_json::to_string(&backwards).unwrap())).await;
        assert_ne!(code, StatusCode::OK, "a log that goes backwards in time must be refused");

        // And an ordinary save still works, or this is a wall rather than a gate.
        let ok = verifiable_bundle(&st.data, 9100, 40, Vec::new());
        let (code, body) = call(&app, post_json("/save", &token, serde_json::to_string(&ok).unwrap())).await;
        assert_eq!(code, StatusCode::OK, "an honest save must still be accepted: {body}");
    }

    /// `/auth/dev` is NOT reachable on the production router, and `/submit` cannot register a save.
    ///
    /// Two live holes an audit reproduced against the real router on 2026-08-20, both of which
    /// let a player who controls their machine become somebody else or become rich.
    ///
    /// TAKEOVER: `/auth/dev` sat in the always-on chain and `login_as` keys accounts on the raw
    /// name — the same function `/auth/steam` calls as `steam:{steamid}`. A SteamID64 is public.
    /// One unauthenticated POST returned the victim's account_id and server seed and evicted their
    /// token.
    ///
    /// FORGERY: `/submit` wrote `st.saves` after a replay that proves nothing on an empty log, and
    /// `put_save`'s migration exception read that back and anchored it into `trusted`.
    ///
    /// Checked against the ROUTER, not the handler list, because both defects were about where a
    /// route is mounted rather than what it does.
    #[test]
    fn the_public_router_exposes_no_way_to_forge_an_identity_or_a_save() {
        let src = include_str!("main.rs");
        let prod_raw = &src[..src.find("mod pg_integration").unwrap_or(src.len())];
        // CODE ONLY. The first version of this scan bounded the dev group at the first mention of
        // `require_dev` — and the comment explaining WHY /auth/dev moved names `require_dev`, so
        // the window closed before reaching the route it was looking for and the test failed on
        // correct code. A comment has satisfied a scan in this file three times; this is the same
        // hazard from the other side.
        let prod: String = prod_raw.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
        let dev_start = prod.find("let dev_routes").expect("the dev group exists");
        let dev_end = prod[dev_start..].find("require_dev").map(|i| dev_start + i).expect("gated by require_dev");
        let dev_group = &prod[dev_start..dev_end];

        assert!(dev_group.contains(r#".route("/auth/dev""#),
            "/auth/dev is not inside the require_dev group — it mints a session for ANY name, and \
             `/auth/steam` names accounts `steam:<public steamid>`, so an unauthenticated POST is \
             an account takeover");

        // The audit's own words about the doc that already said to do this: an intent nothing
        // enforces is not a control. So assert the enforcement, not the intention.
        let submit = prod.split("async fn submit(").nth(1).expect("submit exists");
        let code = &submit[..submit.find("\nasync fn ").unwrap_or(submit.len())];
        assert!(!code.contains("st.saves"),
            "/submit writes the stored save again. It runs no anchor check, no tick-rate check and \
             no taint check, and its replay is satisfied by construction on an empty log — so the \
             write hands `put_save`'s migration exception a forged state to anchor");
    }

    /// Claiming the reached Commander level credits BOUND crystal,
    /// unlocks the skill via a real nonce, and is exactly-once. Runs the endpoints' OWN SQL against a
    /// real database — the coverage this pg suite exists for (a wrong column or ON CONFLICT target
    /// compiles and only fails here).
    #[tokio::test]
    async fn pg_commander_claim_flow() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let st = test_state(Some(pool.clone()));
        let token = seed_session(&st, 777, 777);
        let auth = || {
            let mut h = HeaderMap::new();
            h.insert("authorization", format!("Bearer {token}").parse().unwrap());
            h
        };
        // 1,000 CXP (> the L2 threshold of 700), credited the way progress credits it.
        let mut tx = pool.begin().await.unwrap();
        add_commander_cxp(&mut tx, 777, 1000).await.expect("seed commander");
        tx.commit().await.unwrap();
        let (_c, Json(state)) = commander_state_ep(State(st.clone()), auth()).await;
        assert_eq!(state["level"].as_u64(), Some(2), "1000 CXP reaches level 2");

        // Claiming credits levels 1+2 (300 + 300 = 600 bound crystal — level 1 is the free welcome) and
        // hands back the smite unlock nonce.
        let (_c, Json(cl)) = commander_claim(State(st.clone()), auth(), None).await;
        assert_eq!(cl["crystal"].as_i64(), Some(600), "levels 1+2, 300 each = 600 bound crystal");
        let grants = cl["grants"].as_array().expect("grants array");
        let grant = grants.first().expect("one grant at level 2");
        assert_eq!(grants.len(), 1, "exactly one skill unlocked so far");
        assert_eq!(grant["skill_id"], "commander_smite");
        assert_eq!(
            grant["nonce"].as_str(),
            Some(commander_nonce(&st.roll_secret, 777, "commander_smite").to_string().as_str()),
            "the issued nonce is the one the verifier will recompute",
        );

        // The crystal really landed in the BOUND bucket.
        let bound: (i64,) = sqlx::query_as("SELECT bound FROM crystal_wallet WHERE account_id = $1")
            .bind(777i64).fetch_one(&pool).await.expect("wallet row");
        assert_eq!(bound.0, 600, "600 bound crystal in the wallet (levels 1+2)");

        // Exactly-once is about the GEMS: a second claim pays no new crystal. The SKILL grant is still
        // re-offered (idempotent, for lost-echo recovery) — it is not stranded behind the crystal high-water.
        let (_c, Json(again)) = commander_claim(State(st.clone()), auth(), None).await;
        assert_eq!(again["crystal"].as_i64(), Some(0), "a second claim pays no new crystal");
        let again_grants = again["grants"].as_array().expect("array");
        assert_eq!(again_grants.len(), 1, "smite is re-offered for recovery, not lost");
        assert_eq!(again_grants.first().expect("re-offer")["skill_id"], "commander_smite");
    }

    /// AN ACCOUNT IS NOT ITS SIGN-IN. The point of `account_identities`, asserted in the only
    /// way that distinguishes the new shape from the old one: two identities, one account.
    ///
    /// The old design could not express this at all — the identity WAS the account row's key — so
    /// a player who lost their Steam account lost the account, a ban landed on a credential rather
    /// than a person, and no second way to sign in could ever be added.
    #[tokio::test]
    async fn pg_two_identities_can_be_one_account() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        migrate(&pool).await.expect("migrate");
        let st = test_state(Some(pool.clone()));

        // Unique per run: `setup()` does not truncate `accounts`, and an account is durable.
        let stamp = now_ms();
        let first = format!("steam:{stamp}");
        let second = format!("recovery:{stamp}");

        let a = login_as(&st, first.clone()).await.expect("db up");
        assert!(a.account_id > 0, "a new identity opens an account");

        // THE SAME identity is the same account, not a second one.
        let again = login_as(&st, first.clone()).await.expect("db up");
        assert_eq!(again.account_id, a.account_id, "one identity, one account");
        assert_eq!(again.seed, a.seed, "and the same server-chosen seed");

        // A SECOND identity attached to that account — what a link would write. There is no flow
        // for it yet, which is the point: the schema can express it before the UI can perform it.
        sqlx::query("INSERT INTO account_identities (identity, account_id) VALUES ($1, $2)")
            .bind(&second)
            .bind(a.account_id as i64)
            .execute(&pool)
            .await
            .expect("link the second identity");
        // Cleared so the lookup has to go to the DB, which is what a different process would do.
        st.sessions.by_identity.held().remove(&second);

        let linked = login_as(&st, second.clone()).await.expect("db up");
        assert_eq!(linked.account_id, a.account_id,
                   "the second identity signs into the SAME account, not a new one");
        assert_eq!(linked.seed, a.seed, "same account, same seed");

        // And an unrelated identity is still a fresh account — resolving must not collapse
        // everyone onto one row.
        let other = login_as(&st, format!("steam:{}", stamp + 1)).await.expect("db up");
        assert_ne!(other.account_id, a.account_id, "an unknown identity opens its own account");

        let (n,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM account_identities WHERE account_id = $1",
        )
        .bind(a.account_id as i64)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, 2, "both identities point at the one account");
    }

    /// The BACKFILL: an account made by a build that predates the table must still be able to log
    /// in. `migrate` re-runs it every boot for exactly this reason, so the test drops the mapping
    /// and asks migrate to put it back.
    #[tokio::test]
    async fn pg_an_account_from_before_the_table_can_still_sign_in() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        migrate(&pool).await.expect("migrate");
        let st = test_state(Some(pool.clone()));
        let ident = format!("steam:legacy{}", now_ms());

        let a = login_as(&st, ident.clone()).await.expect("db up");
        // Rewind to the old world: the account row exists, the mapping does not.
        sqlx::query("DELETE FROM account_identities WHERE identity = $1")
            .bind(&ident).execute(&pool).await.expect("unlink");
        st.sessions.by_identity.held().remove(&ident);

        migrate(&pool).await.expect("re-migrate backfills");
        let back = login_as(&st, ident.clone()).await.expect("db up");
        assert_eq!(back.account_id, a.account_id,
                   "the backfill reconnected the account rather than minting a second one");
    }

    fn test_state(db: Option<sqlx::PgPool>) -> AppState {
        AppState {
            data: Arc::new(GameData::from_json_str(include_str!("../../../data/game_data.json")).unwrap()),
            db,
            sessions: Arc::new(Sessions { next_id: AtomicU64::new(1), ..Default::default() }),
            saves: Default::default(),
                saving: Default::default(),
                replay_slots: Arc::new(tokio::sync::Semaphore::new(2)),
                replay_budget: Default::default(),
                steam_reads: Default::default(),
            pins: Default::default(),
            stats: Default::default(),
            trusted: Default::default(),
            crystal_breaks: Arc::default(),
            dev_key: None,
            admin_key: None,
            prod: false,
            rare_rate_mult_bp: 10_000,
            metrics: Default::default(),
            rate: Default::default(),
            inflight: Arc::new(tokio::sync::Semaphore::new(MAX_INFLIGHT)),
            roll_secret: "test-secret".into(),
            farm_events: Vec::new(),
            fuse_rolls: Default::default(),
            fuse_consumed: Default::default(),
            tick_ref: Default::default(),
            lifetime_excess: Default::default(),
            season: Default::default(),
            tracks: Arc::new(schedule::tracks()),
            commander_levels: Arc::new(commander::build_level_table()),
            // Load the real shared config via a manifest-relative path so tests are CWD-independent.
            commander_rewards: Arc::new(
                commander::parse_rewards(
                    &std::fs::read_to_string(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../data/commander_rewards.json"
                    ))
                    .expect("read data/commander_rewards.json"),
                )
                .expect("parse commander_rewards.json"),
            ),
        }
    }

    /// The SCHEDULE advance, against a real database.
    ///
    /// Unit tests cover the arithmetic; nothing there executes SQL, and the two statements
    /// `advance_tracks` runs are the parts nothing else can check — an `unnest($2::text[])` insert
    /// and an `ANY($2)` update, both binding a Rust `Vec<String>` to a Postgres text array.
    ///
    /// The properties, and why each would be a real bug:
    ///   - a second save the SAME day must not advance a track (every save would be worth a day)
    ///   - a new day must advance by exactly one (or the ladder skips cells, which the user ruled
    ///     out explicitly)
    ///   - an unclaimed cell must still be owed the next day, ALONGSIDE the new one
    ///   - a second claim must pay nothing — the double-payout `claimed_mask` exists to stop
    #[tokio::test]
    async fn pg_schedule_advances_once_a_day_and_pays_each_cell_once() {
        let Some((pool, _g)) = setup().await else { return };
        let acct = 7311i64;
        let tracks = schedule::tracks();
        sqlx::query("DELETE FROM claim_progress WHERE account_id = $1").bind(acct)
            .execute(&pool).await.expect("clean");

        async fn read(pool: &sqlx::PgPool, acct: i64) -> Option<(i32, i64)> {
            sqlx::query_as(
                "SELECT position, claimed_mask FROM claim_progress WHERE account_id = $1 AND track = $2")
                .bind(acct).bind(schedule::LOGIN7)
                .fetch_optional(pool).await.expect("read claim_progress")
        }

        let day = 30_000i64;
        for pass in 0..3 {
            let mut tx = pool.begin().await.expect("begin");
            advance_tracks(&mut tx, acct as u64, day, &tracks).await.expect("advance");
            tx.commit().await.expect("commit");
            assert_eq!(read(&pool, acct).await, Some((1, 0)),
                "pass {pass} of the SAME day moved the track — every save would be worth a day");
        }

        // A new day: exactly one step, never two.
        let mut tx = pool.begin().await.expect("begin");
        advance_tracks(&mut tx, acct as u64, day + 1, &tracks).await.expect("advance");
        tx.commit().await.expect("commit");
        assert_eq!(read(&pool, acct).await, Some((2, 0)), "a new day did not advance by exactly one");

        // Two days played, nothing claimed: BOTH cells are owed on one press. This is the case the
        // user asked about — yesterday's reward is not lost by logging in again.
        let t = schedule::track(schedule::LOGIN7).expect("login7 exists");
        let (pay, cells) = schedule::owed(&t.entries, 2, 0);
        assert_eq!(cells.len(), 2, "yesterday's unclaimed cell was dropped");
        sqlx::query("UPDATE claim_progress SET claimed_mask = claimed_mask | $3 WHERE account_id = $1 AND track = $2")
            .bind(acct).bind(schedule::LOGIN7).bind(pay as i64)
            .execute(&pool).await.expect("claim");
        let (_, mask) = read(&pool, acct).await.expect("row");
        assert_eq!(schedule::owed(&t.entries, 2, mask as u64).0, 0, "a second press PAID AGAIN");

        // The advance still steps a claimed track — claiming is not what moves it, login is.
        let mut tx = pool.begin().await.expect("begin");
        advance_tracks(&mut tx, acct as u64, day + 2, &tracks).await.expect("advance");
        tx.commit().await.expect("commit");
        let (pos, mask) = read(&pool, acct).await.expect("row");
        assert_eq!(pos, 3);
        assert_eq!(schedule::owed(&t.entries, pos as u32, mask as u64).1.len(), 1,
            "the third day should owe exactly the third cell");

        sqlx::query("DELETE FROM claim_progress WHERE account_id = $1").bind(acct)
            .execute(&pool).await.expect("clean up");
    }

    #[tokio::test]
    async fn pg_daily_and_bp_state_queries_execute() {
        let Some((pool, _g)) = setup().await else { return };
        let (acct, season, day) = (7001i64, 1i64, 20_123i64);
        sqlx::query("INSERT INTO bp_state (account_id, season_id, xp, last_tick, snapshot_json) VALUES ($1, $2, 0, 0, '{}')")
            .bind(acct).bind(season).execute(&pool).await.expect("seed bp_state");

        // The exact SELECT `bp_accrue` runs.
        let row: Option<(i64, i64, String)> = sqlx::query_as(
            "SELECT xp, last_tick, snapshot_json FROM bp_state WHERE account_id = $1 AND season_id = $2",
        )
        .bind(acct).bind(season).fetch_optional(&pool).await.expect("bp_state SELECT");
        assert!(row.is_some(), "seeded row did not read back");

        // ...and the exact UPDATE.
        sqlx::query("UPDATE bp_state SET last_tick = $3, snapshot_json = $4 WHERE account_id = $1 AND season_id = $2")
            .bind(acct).bind(season).bind(42i64).bind("{}")
            .execute(&pool).await.expect("bp_state UPDATE");

        // A commission day, created then read back through the real helper.
        let mut m = sim_core::state::Measures::default();
        m.kills.set_for_test(500);
        let mut tx = pool.begin().await.expect("begin");
        let board = daily::board();
        let (opened, mask, ms, claimed, sv) =
            daily_row(&mut tx, acct as u64, day, &board, &m).await.expect("create day");
        assert!(opened, "the first touch of a day must report that it OPENED it");
        assert_eq!((mask, ms, claimed), (0, 0, 0), "a fresh day starts clean");
        // Start values are indexed by each commission's FIXED bit; a retired bit stays a hole.
        let top = board.iter().map(|d| usize::from(d.bit) + 1).max().unwrap_or(0);
        assert_eq!(sv.len(), top, "one cell per bit up to the highest in use");
        assert!(board.iter().all(|d| sv.get(usize::from(d.bit)).is_some_and(|v| *v != daily::NO_START)),
            "every commission opens on a counter");
        assert_eq!(sv[0], 500, "the kills commission must open on the counter it saw");
        sqlx::query("UPDATE daily_progress SET done_mask = $3, milestone_mask = $4 WHERE account_id = $1 AND day = $2")
            .bind(acct).bind(day).bind(3i16).bind(1i16)
            .execute(&mut *tx).await.expect("daily UPDATE");
        sqlx::query("UPDATE daily_progress SET claimed_mask = $3 WHERE account_id = $1 AND day = $2")
            .bind(acct).bind(day).bind(1i16)
            .execute(&mut *tx).await.expect("claimed UPDATE");
        tx.commit().await.expect("commit");

        let mut tx2 = pool.begin().await.expect("begin2");
        let (opened2, mask2, ms2, claimed2, sv2) = daily_row(&mut tx2, acct as u64, day, &board, &Default::default())
            .await
            .expect("reread day");
        assert!(!opened2, "re-reading an existing day must NOT report it as opened");
        // claimed_mask is READ BACK too: the ALTER that adds it is a no-op on an existing table,
        // so a column that never round-trips is how a migration silently does not land.
        assert_eq!((mask2, ms2, claimed2), (3, 1, 1), "masks did not survive the round trip");
        assert_eq!(sv2[0], 500, "start values were overwritten on re-read");
        tx2.commit().await.expect("commit2");
    }

    /// The CLAIM path, executed against a real database: crystal land in the BOUND bucket, the
    /// ledger records why, the mask closes — and a second press pays nothing.
    ///
    /// Written against the statements the handler runs rather than the handler itself, for the
    /// same reason `pg_daily_and_bp_state_queries_execute` is: the risk here is SQL that does not
    /// match the schema, and a mocked pool would prove nothing about that.
    #[tokio::test]
    async fn pg_a_daily_claim_pays_bound_crystal_once() {
        let Some((pool, _g)) = setup().await else { return };
        let acct: i64 = 990_413;
        let day: i64 = 4_242;
        sqlx::query("DELETE FROM daily_progress WHERE account_id = $1").bind(acct)
            .execute(&pool).await.expect("clean day");
        sqlx::query("DELETE FROM crystal_wallet WHERE account_id = $1").bind(acct)
            .execute(&pool).await.expect("clean wallet");
        sqlx::query("DELETE FROM crystal_ledger WHERE account_id = $1").bind(acct)
            .execute(&pool).await.expect("clean ledger");
        // A day with three commissions done → 300 points → rungs 1..3 owed.
        sqlx::query(
            "INSERT INTO daily_progress (account_id, day, done_mask, milestone_mask, claimed_mask, start_values_json) \
             VALUES ($1, $2, 7, 0, 0, '[]')",
        ).bind(acct).bind(day).execute(&pool).await.expect("seed day");

        let ladder = daily::ladder();
        let mut paid_total = 0u64;
        // Two presses, the second immediately after the first. Only the first may pay.
        for press in 0..2 {
            let mut tx = pool.begin().await.expect("begin");
            let (done, claimed): (i16, i16) = sqlx::query_as(
                "SELECT done_mask, claimed_mask FROM daily_progress \
                 WHERE account_id = $1 AND day = $2 FOR UPDATE",
            ).bind(acct).bind(day).fetch_one(&mut *tx).await.expect("read row");
            let points: u32 = daily::board().iter().enumerate()
                .filter(|(i, _)| done as u16 & (1u16 << i) != 0)
                .map(|(_, d)| d.points).sum();
            let (pay, crystal) = daily::claimable(&ladder, points, claimed as u16);
            if pay != 0 {
                sqlx::query(
                    "INSERT INTO crystal_wallet (account_id, paid, bound) VALUES ($1, 0, $2) \
                     ON CONFLICT (account_id) DO UPDATE SET bound = crystal_wallet.bound + EXCLUDED.bound",
                ).bind(acct).bind(crystal as i64).execute(&mut *tx).await.expect("credit");
                sqlx::query(
                    "INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) \
                     VALUES ($1, 0, $2, 'daily', $3)",
                ).bind(acct).bind(crystal as i64).bind(format!("day:{day}"))
                    .execute(&mut *tx).await.expect("ledger");
                sqlx::query(
                    "UPDATE daily_progress SET claimed_mask = claimed_mask | $3 \
                     WHERE account_id = $1 AND day = $2",
                ).bind(acct).bind(day).bind(pay as i16).execute(&mut *tx).await.expect("close mask");
                paid_total += crystal;
            }
            tx.commit().await.expect("commit");
            if press == 1 {
                assert_eq!(pay, 0, "the SECOND press owed something — claim is not exactly-once");
            }
        }

        let want: u64 = ladder.iter().take(3).map(|m| m.crystal as u64).sum();
        assert_eq!(paid_total, want, "the press did not pay rungs 1..3");
        let (paid, bound): (i64, i64) =
            sqlx::query_as("SELECT paid, bound FROM crystal_wallet WHERE account_id = $1")
                .bind(acct).fetch_one(&pool).await.expect("wallet");
        assert_eq!(bound as u64, want, "crystal did not land in the bound bucket");
        assert_eq!(paid, 0, "an EARNED crystal landed in the paid bucket");
        let (rows, sum): (i64, Option<i64>) = sqlx::query_as(
            "SELECT count(*), sum(d_bound)::bigint FROM crystal_ledger WHERE account_id = $1 AND reason = 'daily'",
        ).bind(acct).fetch_one(&pool).await.expect("ledger read");
        assert_eq!(rows, 1, "the audit trail should show ONE daily credit, not one per press");
        assert_eq!(sum.unwrap_or(0) as u64, want);
    }

    /// AUDIT 2026-10-02 (rewind + op-id reuse): a settled op id re-presented with a different op
    /// is caught; the same op re-presented (a growing log) is not.
    #[tokio::test]
    async fn pg_a_settled_op_id_reused_for_another_claim_is_caught() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let mail = mailbox_store::send(&pool, 6, MailKind::Reward, 76, "", 0, "r").await.unwrap();
        let real = sim_core::state::MarketOp::Claim {
            id: 21, mail_id: mail, gil: sim_core::state::Gil::new(76), def_id: String::new(), qty: 0, item: None,
        };
        let s = mailbox::settle_claims(
            &mailbox_store::snapshot_for_claims(&pool, 6, std::slice::from_ref(&real)).await.unwrap(),
            &HashSet::new(), 6, std::slice::from_ref(&real),
        ).expect("a real claim");
        assert!(persist_save_and_claims(&pool, 6, "{}", 1, "h6", &s, None, &[], &AcceptedExtras::default()).await.expect("settles"));
        assert!(reused_op_ids(&pool, 6, std::slice::from_ref(&real)).await.unwrap().is_empty(), "same op again: fine");
        let forged = sim_core::state::MarketOp::Claim {
            id: 21, mail_id: mail, gil: sim_core::state::Gil::new(9_999_999), def_id: String::new(), qty: 0, item: None,
        };
        assert_eq!(reused_op_ids(&pool, 6, &[forged]).await.unwrap(), vec![mail], "same id, other payload: caught");
    }

    /// `persist_save_and_claims` is one transaction: the save row, the anchor, the claimed mail
    /// and the consumed op commit together — and when any part fails, none of them do.
    #[tokio::test]
    async fn pg_a_save_and_its_claims_commit_together_or_not_at_all() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let mail = mailbox_store::send(&pool, 5, MailKind::Reward, 76, "", 0, "r").await.unwrap();
        let ok = mailbox::ClaimSettlement { consume: vec![mail], consumed_ops: vec![11, 12], retired: vec![12], digests: Vec::new() };
        assert!(persist_save_and_claims(&pool, 5, "{\"gil\":42}", 100, "hashA", &ok, None, &[], &AcceptedExtras::default()).await.expect("commits"));
        let (sj,): (String,) = sqlx::query_as("SELECT state_json FROM saves WHERE account_id = 5").fetch_one(&pool).await.unwrap();
        assert_eq!(sj, "{\"gil\":42}");
        assert!(mailbox_store::inbox_for(&pool, 5).await.unwrap().is_empty(), "the claimed mail is consumed");
        let ops = vec![
            sim_core::state::MarketOp::Claim { id: 11, mail_id: mail, gil: sim_core::state::Gil::new(76), def_id: String::new(), qty: 0, item: None },
            sim_core::state::MarketOp::Deposit { id: 12, def_id: "herb".into(), qty: 1, unit_price: 1 },
        ];
        assert_eq!(already_consumed(&pool, 5, &ops).await.unwrap(), HashSet::from([11, 12]),
            "both ops are marked, so a growing-log re-save skips them");

        // A second mail, and a settlement that also names a mail id that is not an unclaimed row
        // (the one above, already consumed): the whole transaction must roll back.
        let mail2 = mailbox_store::send(&pool, 5, MailKind::Reward, 10, "", 0, "r").await.unwrap();
        let bad = mailbox::ClaimSettlement { consume: vec![mail2, mail], consumed_ops: vec![13, 14], retired: Vec::new(), digests: Vec::new() };
        assert!(persist_save_and_claims(&pool, 5, "{\"gil\":99}", 200, "hashB", &bad, Some("hashA"), &[], &AcceptedExtras::default()).await.is_err());
        let (sj,): (String,) = sqlx::query_as("SELECT state_json FROM saves WHERE account_id = 5").fetch_one(&pool).await.unwrap();
        assert_eq!(sj, "{\"gil\":42}", "the save row rolled back with the failed claim");
        assert_eq!(mailbox_store::inbox_for(&pool, 5).await.unwrap().len(), 1, "mail2 is still claimable");
        let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM consumed_market_ops WHERE account_id = 5 AND op_id IN (13, 14)")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(n, 0, "no op of the failed settlement is marked");
        let (t,): (i64,) = sqlx::query_as("SELECT count(*) FROM trusted WHERE account_id = 5 AND state_hash = 'hashB'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(t, 0, "and its hash never became an anchor");
    }

    /// The durable backstop under `reconcile_claims`: taking the SAME mail twice inside one
    /// settlement must abort the settlement, not quietly no-op.
    ///
    /// `consume`'s `WHERE claimed_at IS NULL` always made the second UPDATE affect zero rows, but
    /// the count was discarded — so a settlement that double-took a mail committed silently, and
    /// the only thing standing between that and a mint was a pure function in another file. This
    /// asserts the SQL itself refuses, which is what makes the reconcile fix defence-in-depth
    /// rather than the single point of failure.
    #[tokio::test]
    async fn pg_a_mail_consumed_twice_in_one_settlement_aborts_it() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let mail_id = mailbox_store::send(&pool, 9, MailKind::SaleProceeds, 76, "", 0, "sold").await.unwrap();

        let mut tx = pool.begin().await.unwrap();
        mailbox_store::consume(&mut *tx, mail_id, T0).await.expect("the first take is legitimate");
        let second = mailbox_store::consume(&mut *tx, mail_id, T0).await;
        assert!(second.is_err(), "the second take of one mail must fail, not no-op");
        drop(tx); // no commit → the whole settlement rolls back

        let inbox = mailbox_store::inbox_for(&pool, 9).await.unwrap();
        assert_eq!(inbox.len(), 1, "the aborted settlement left the mail unclaimed and claimable");
        assert_eq!(inbox[0].gil, 76, "and its payload intact — nothing was minted, nothing lost");
    }

    /// The H3 race detector. `already_consumed` is a pool SELECT taken BEFORE the write
    /// transaction opens, so two concurrent duplicate saves both read an empty consumed-set and
    /// both settle. `ON CONFLICT DO NOTHING` used to swallow the collision.
    ///
    /// `PRIMARY KEY (account_id, op_id)` is what makes reading `rows_affected` sound rather than
    /// best-effort — the loser blocks on the winner's row lock until it commits, so the answer is
    /// final by the time it is read. Simulated here with two real transactions on two connections,
    /// because a single-connection version would prove nothing about the race.
    #[tokio::test]
    async fn pg_two_concurrent_settlements_of_one_op_cannot_both_commit() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let ops = vec![sim_core::state::MarketOp::Deposit { id: 77, def_id: "herb".into(), qty: 1, unit_price: 1 }];
        // Both requests read the consumed-set before either writes — the production ordering.
        assert!(already_consumed(&pool, 3, &ops).await.unwrap().is_empty());
        assert!(already_consumed(&pool, 3, &ops).await.unwrap().is_empty());

        let mut winner = pool.begin().await.unwrap();
        mark_consumed(&mut winner, 3, &[77], &[]).await.expect("the first settlement takes the op");
        winner.commit().await.unwrap();

        let mut loser = pool.begin().await.unwrap();
        let r = mark_consumed(&mut loser, 3, &[77], &[]).await;
        assert!(r.is_err(), "the second settlement of one op must be rejected, not silently dropped");
        drop(loser);

        let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM consumed_market_ops WHERE account_id=3 AND op_id=77")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(n, 1, "exactly one settlement is durable");
    }

    /// The gacha SQL, executed for real. Everything else about the roll is covered by pure
    /// unit tests, but these statements had only ever been type-checked — a wrong column, a
    /// bad ON CONFLICT target or an overflowing cast would all compile and only fail in
    /// production. Runs the same statements `gacha_roll` does, in the same order, inside one
    /// transaction, and asserts the state they leave behind.
    #[tokio::test]
    async fn pg_gacha_roll_statements_round_trip() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let acct: i64 = 4242;
        let idem = "idem-key-abc";

        // Seed a wallet the way /dev/gacha-crystal does (GREATEST guards a negative grant).
        sqlx::query("INSERT INTO crystal_wallet (account_id, paid, bound) VALUES ($1, GREATEST($2,0), GREATEST($3,0)) ON CONFLICT (account_id) DO UPDATE SET paid = GREATEST(crystal_wallet.paid + $2, 0), bound = GREATEST(crystal_wallet.bound + $3, 0)")
            .bind(acct).bind(5_000_i64).bind(500_i64)
            .execute(&pool).await.expect("seed wallet");

        let mut tx = pool.begin().await.expect("begin");
        // 1. claim the idempotency key
        let r = sqlx::query("INSERT INTO gacha_rolls (idem_key, account_id, banner_id, results_json) VALUES ($1, $2, $3, '[]') ON CONFLICT DO NOTHING")
            .bind(idem).bind(acct).bind("standard")
            .execute(&mut *tx).await.expect("claim key");
        assert_eq!(r.rows_affected(), 1, "first claim wins");

        // 2. lock + read the wallet
        let (t, b): (i64, i64) = sqlx::query_as("SELECT paid, bound FROM crystal_wallet WHERE account_id = $1 FOR UPDATE")
            .bind(acct).fetch_one(&mut *tx).await.expect("lock wallet");
        let mut w = gacha::Wallet { paid: t as u64, bound: b as u64 };
        let spend = w.spend(gacha::cost_of(10).unwrap()).expect("afford");
        assert_eq!(spend.from_bound, 500, "bound bucket drains first");
        assert_eq!(spend.from_paid, 2_500, "the rest comes from paid");

        // 3. lock + read pity (absent row => default)
        let prow: Option<(i32, i32, bool)> = sqlx::query_as("SELECT count, pulls_since_sr, lost_5050 FROM gacha_pity WHERE account_id = $1 AND banner_type = $2 FOR UPDATE")
            .bind(acct).bind("character").fetch_optional(&mut *tx).await.expect("lock pity");
        assert!(prow.is_none(), "no pity row yet");
        let mut pity = gacha::PityState::default();
        let mut rng = sim_core::rng::Pcg32::new(gacha_seed("test-secret", acct as u64, idem, 0));
        let pulls = gacha::roll_batch(&gacha::CHARACTER, &mut pity, &mut rng, 10);
        assert_eq!(pulls.len(), 10);

        // 4. the writes the handler makes. THE WALLET IS A GUARDED DELTA — this test claims to run
        // "the same statements, in the same order", and it went on executing the ABSOLUTE upsert
        // after the handler stopped using one, so it was round-tripping a statement the server no
        // longer issues and confirming behaviour that had been replaced.
        let debited = sqlx::query(
            "UPDATE crystal_wallet SET paid = paid - $2, bound = bound - $3 \
             WHERE account_id = $1 AND paid >= $2 AND bound >= $3")
            .bind(acct).bind(spend.from_paid as i64).bind(spend.from_bound as i64)
            .execute(&mut *tx).await.expect("wallet debit").rows_affected();
        assert_eq!(debited, 1, "the guarded delta did not apply");
        sqlx::query("INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES ($1, $2, $3, 'gacha-pull', $4)")
            .bind(acct).bind(-(spend.from_paid as i64)).bind(-(spend.from_bound as i64)).bind(idem)
            .execute(&mut *tx).await.expect("ledger insert");
        sqlx::query("INSERT INTO gacha_pity (account_id, banner_type, count, pulls_since_sr, lost_5050) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (account_id, banner_type) DO UPDATE SET count = EXCLUDED.count, pulls_since_sr = EXCLUDED.pulls_since_sr, lost_5050 = EXCLUDED.lost_5050")
            .bind(acct).bind("character").bind(pity.count as i32).bind(pity.pulls_since_sr as i32).bind(pity.lost_5050)
            .execute(&mut *tx).await.expect("pity upsert");
        // MILEAGE accrues on the batch — 1 per pull, absent row starting from 0.
        let mrow: Option<(i64,)> = sqlx::query_as("SELECT tickets FROM gacha_mileage WHERE account_id = $1 FOR UPDATE")
            .bind(acct).fetch_optional(&mut *tx).await.expect("lock mileage");
        assert!(mrow.is_none(), "no mileage row yet");
        let mut mileage = gacha::Mileage { tickets: 0 };
        mileage.earn(10);
        sqlx::query("INSERT INTO gacha_mileage (account_id, tickets) VALUES ($1, $2) ON CONFLICT (account_id) DO UPDATE SET tickets = EXCLUDED.tickets")
            .bind(acct).bind(mileage.tickets as i64)
            .execute(&mut *tx).await.expect("mileage upsert");
        // ACCOUNT-SCOPED, like the handler: the PK is (account_id, idem_key), so an unscoped
        // update writes over another account's row holding the same key.
        sqlx::query("UPDATE gacha_rolls SET results_json = $1 WHERE idem_key = $2 AND account_id = $3")
            .bind("[{\"def_id\":\"regalia_fodder\"}]").bind(idem).bind(acct)
            .execute(&mut *tx).await.expect("results update");
        tx.commit().await.expect("commit");

        let (tk,): (i64,) = sqlx::query_as("SELECT tickets FROM gacha_mileage WHERE account_id = $1")
            .bind(acct).fetch_one(&pool).await.expect("mileage row");
        assert_eq!(tk, 10, "a x10 mints 10 tickets regardless of what it rolled");

        // Debited exactly 3000: the bound bucket emptied first, the rest off paid.
        let (t2, b2): (i64, i64) = sqlx::query_as("SELECT paid, bound FROM crystal_wallet WHERE account_id = $1")
            .bind(acct).fetch_one(&pool).await.expect("read back");
        assert_eq!((t2, b2), (2_500, 0), "5000+500 - 3000, bound first");

        // The ledger records the split as signed deltas (I1/I7 audit against this).
        let (dt, db): (i64, i64) = sqlx::query_as("SELECT d_paid, d_bound FROM crystal_ledger WHERE ref = $1")
            .bind(idem).fetch_one(&pool).await.expect("ledger row");
        assert_eq!((dt, db), (-2_500, -500));
        assert_eq!(-(dt + db) as u64, gacha::cost_of(10).unwrap(), "ledger sums to the price");

        // Pity persisted and round-trips through the i32 columns unchanged.
        let (c, ps, l): (i32, i32, bool) = sqlx::query_as("SELECT count, pulls_since_sr, lost_5050 FROM gacha_pity WHERE account_id = $1 AND banner_type = $2")
            .bind(acct).bind("character").fetch_one(&pool).await.expect("pity row");
        assert_eq!((c as u32, ps as u32, l), (pity.count, pity.pulls_since_sr, pity.lost_5050));

        // IDEMPOTENCY: replaying the key must not claim it a second time.
        let again = sqlx::query("INSERT INTO gacha_rolls (idem_key, account_id, banner_id, results_json) VALUES ($1, $2, $3, '[]') ON CONFLICT DO NOTHING")
            .bind(idem).bind(acct).bind("standard")
            .execute(&pool).await.expect("replay");
        assert_eq!(again.rows_affected(), 0, "the key is taken — the retry reads the stored result");
        let stored: (String,) = sqlx::query_as("SELECT results_json FROM gacha_rolls WHERE idem_key = $1")
            .bind(idem).fetch_one(&pool).await.expect("stored results");
        assert!(stored.0.contains("regalia_fodder"), "the replay path has something to return");
    }

    /// The store's SQL, executed for real: grant, then refund. This is the money-IN path, and
    /// every guarantee on it is a database guarantee — the idempotent claim, the bucket split,
    /// and the rule that a refund COMPENSATES rather than deletes.
    #[tokio::test]
    async fn pg_store_grants_once_and_refunds_without_erasing() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let acct: i64 = 8100;
        let order: i64 = 999_001;
        let s = store::sku("crystal_1620").expect("catalog");

        sqlx::query("INSERT INTO store_orders (order_id, account_id, sku, idem_key, price_cents, status) VALUES ($1,$2,$3,$4,$5,'pending')")
            .bind(order).bind(acct).bind(s.id).bind("idem-1").bind(store::price_for(s, "USD").minor as i64)
            .execute(&pool).await.expect("order");

        // FIRST buy: no prior entitlement, so the bonus applies and lands BOUND.
        let first: (i64,) = sqlx::query_as("SELECT count(*) FROM entitlements WHERE account_id=$1 AND sku=$2 AND revoked_at IS NULL")
            .bind(acct).bind(s.id).fetch_one(&pool).await.unwrap();
        let grant = store::grant_for(s, first.0 == 0);
        assert_eq!((grant.paid, grant.bound), (1_620, 1_620));

        // The handler's claim + credit, run as it runs.
        let mut tx = pool.begin().await.unwrap();
        let claimed = sqlx::query("UPDATE store_orders SET status='granted', first_buy=$1, granted_paid=$2, granted_bound=$3 WHERE order_id=$4 AND status <> 'granted'")
            .bind(true).bind(grant.paid as i64).bind(grant.bound as i64).bind(order)
            .execute(&mut *tx).await.unwrap().rows_affected();
        assert_eq!(claimed, 1, "first finalize claims the order");
        sqlx::query("INSERT INTO crystal_wallet (account_id, paid, bound) VALUES ($1,$2,$3) ON CONFLICT (account_id) DO UPDATE SET paid = crystal_wallet.paid + EXCLUDED.paid, bound = crystal_wallet.bound + EXCLUDED.bound")
            .bind(acct).bind(grant.paid as i64).bind(grant.bound as i64).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES ($1,$2,$3,'purchase',$4)")
            .bind(acct).bind(grant.paid as i64).bind(grant.bound as i64).bind(order.to_string()).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO entitlements (account_id, sku, order_id) VALUES ($1,$2,$3)")
            .bind(acct).bind(s.id).bind(order).execute(&mut *tx).await.unwrap();
        tx.commit().await.unwrap();

        let w: (i64, i64) = sqlx::query_as("SELECT paid, bound FROM crystal_wallet WHERE account_id=$1")
            .bind(acct).fetch_one(&pool).await.unwrap();
        assert_eq!(w, (1_620, 1_620), "bought crystal paid, bonus crystal bound");

        // A SECOND finalize of the same order must not credit again.
        let again = sqlx::query("UPDATE store_orders SET status='granted' WHERE order_id=$1 AND status <> 'granted'")
            .bind(order).execute(&pool).await.unwrap().rows_affected();
        assert_eq!(again, 0, "the claim is single-use, so a replay cannot double-grant");

        // And the SKU is no longer a first buy.
        let now_first: (i64,) = sqlx::query_as("SELECT count(*) FROM entitlements WHERE account_id=$1 AND sku=$2 AND revoked_at IS NULL")
            .bind(acct).bind(s.id).fetch_one(&pool).await.unwrap();
        assert_eq!(store::grant_for(s, now_first.0 == 0).bound, 0, "the bonus is spent");

        // REFUND: compensating, not deleting.
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("UPDATE store_orders SET status='refunded' WHERE order_id=$1").bind(order).execute(&mut *tx).await.unwrap();
        sqlx::query("UPDATE entitlements SET revoked_at = now() WHERE order_id=$1 AND revoked_at IS NULL").bind(order).execute(&mut *tx).await.unwrap();
        sqlx::query("INSERT INTO crystal_ledger (account_id, d_paid, d_bound, reason, ref) VALUES ($1,$2,$3,'refund',$4)")
            .bind(acct).bind(-(grant.paid as i64)).bind(-(grant.bound as i64)).bind(order.to_string()).execute(&mut *tx).await.unwrap();
        sqlx::query("UPDATE crystal_wallet SET paid = GREATEST(0, paid - $1), bound = GREATEST(0, bound - $2) WHERE account_id=$3")
            .bind(grant.paid as i64).bind(grant.bound as i64).bind(acct).execute(&mut *tx).await.unwrap();
        tx.commit().await.unwrap();

        let w2: (i64, i64) = sqlx::query_as("SELECT paid, bound FROM crystal_wallet WHERE account_id=$1")
            .bind(acct).fetch_one(&pool).await.unwrap();
        assert_eq!(w2, (0, 0), "the crystal came back out");
        let rows: (i64,) = sqlx::query_as("SELECT count(*) FROM entitlements WHERE order_id=$1").bind(order).fetch_one(&pool).await.unwrap();
        assert_eq!(rows.0, 1, "the entitlement is REVOKED, not deleted — history survives a refund");
        let led: (i64,) = sqlx::query_as("SELECT sum(d_paid)::bigint FROM crystal_ledger WHERE ref=$1").bind(order.to_string()).fetch_one(&pool).await.unwrap();
        assert_eq!(led.0, 0, "purchase + reversal net to zero, and BOTH entries are on the record");
    }

    /// `mailbox_store` against a real database. `mailbox.rs`'s fuzz harness proves the RULES over
    /// random operation sequences; nothing but this proves Postgres implements the same ones —
    /// and the rules here are the ones a duplicate would turn into free money.
    #[tokio::test]
    async fn pg_mailbox_rows_honour_the_model() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let (owner, stranger) = (7001_u64, 7002_u64);

        // An empty mail is the model's one invalid send and must not become a row.
        assert_eq!(
            mailbox_store::send(&pool, owner, MailKind::Reward, 0, "", 0, "empty").await,
            Err(mailbox::MailError::EmptyPayload),
        );
        let gil_id = mailbox_store::send(&pool, owner, MailKind::SaleProceeds, 76, "", 0, "sold").await.unwrap();
        let item = sim_core::items::Item::new(
            "sword_short", sim_core::items::ItemKind::Weapon, 20, 0, sim_core::items::Rarity::Rare,
        );
        let item_id = mailbox_store::send_item(&pool, owner, MailKind::Purchase, &item, "bought").await.unwrap();
        assert!(item_id > gil_id, "ids stay monotonic");

        let inbox = mailbox_store::inbox_for(&pool, owner).await.unwrap();
        assert_eq!(inbox.len(), 2);
        assert_eq!(inbox[0].kind, MailKind::SaleProceeds, "kind round-trips through TEXT");
        assert_eq!(inbox[1].item.as_ref().map(|i| i.atk_bonus), Some(20), "the full instance survives");
        assert!(mailbox_store::inbox_for(&pool, stranger).await.unwrap().is_empty());

        // A stranger's claim must consume nothing — the conditional UPDATE fires first, so the
        // rollback is what protects the mail.
        let mut tx = pool.begin().await.unwrap();
        assert_eq!(
            mailbox_store::claim(&mut tx, stranger, gil_id, 1).await,
            Err(mailbox::MailError::NotOwner),
        );
        drop(tx);
        assert_eq!(mailbox_store::inbox_for(&pool, owner).await.unwrap().len(), 2, "still unclaimed");

        // Claim-once: the payload comes back exactly once, a replay is AlreadyClaimed.
        let mut tx = pool.begin().await.unwrap();
        let c = mailbox_store::claim(&mut tx, owner, gil_id, 1).await.unwrap();
        assert_eq!(c.gil, 76);
        tx.commit().await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        assert_eq!(
            mailbox_store::claim(&mut tx, owner, gil_id, 2).await,
            Err(mailbox::MailError::AlreadyClaimed),
            "a replayed claim is never a second grant",
        );
        drop(tx);
        let mut tx = pool.begin().await.unwrap();
        assert_eq!(mailbox_store::claim(&mut tx, owner, 999_999, 3).await, Err(mailbox::MailError::NotFound));
        drop(tx);
        assert_eq!(mailbox_store::inbox_for(&pool, owner).await.unwrap().len(), 1, "claimed leaves the inbox");

        // The reconcile snapshot must carry BOTH halves: an unclaimed row proves a claim is live,
        // a claimed one is what makes a replayed claim read as a forgery rather than as missing.
        let ops = vec![
            sim_core::state::MarketOp::Claim { id: 1, mail_id: gil_id, gil: sim_core::state::Gil::new(76), def_id: String::new(), qty: 0, item: None },
            sim_core::state::MarketOp::Claim { id: 2, mail_id: item_id, gil: sim_core::state::Gil::new(0), def_id: String::new(), qty: 0, item: Some(item.clone()) },
        ];
        let snap = mailbox_store::snapshot_for_claims(&pool, owner, &ops).await.unwrap();
        assert_eq!(
            snap.verify_claim(owner, gil_id, 76, "", 0, None),
            Err(mailbox::MailError::AlreadyClaimed),
            "the consumed mail is remembered, not forgotten",
        );
        // Taken from ANOTHER account's view, that same claimed mail is not "stale" — it is someone
        // else's, and claiming it stays a forgery.
        let foreign = mailbox_store::snapshot_for_claims(&pool, stranger, &ops).await.unwrap();
        assert_eq!(foreign.verify_claim(stranger, gil_id, 76, "", 0, None), Err(mailbox::MailError::NotOwner));
        assert_eq!(snap.verify_claim(owner, item_id, 0, "", 0, Some(&item)), Ok(()), "the live one verifies");
        assert_eq!(
            snap.verify_claim(owner, item_id, 999, "", 0, Some(&item)),
            Err(mailbox::MailError::PayloadMismatch),
            "a lied payload is still caught",
        );
    }

    /// The layer-2 counter's SQL, executed for real. The arithmetic is unit-tested above, but
    /// the upsert and the review query are plain strings that nothing else would exercise — and
    /// the counter is durable precisely so a restart can't launder it, which is a claim only a
    /// real round-trip can support.
    #[tokio::test]
    async fn pg_lifetime_excess_persists_and_ranks() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let (honest, cheat): (i64, i64) = (6001, 6002);
        for (acct, val) in [(honest, EXCESS_FLOOR), (cheat, 0)] {
            sqlx::query(
                "INSERT INTO integrity (account_id, lifetime_excess) VALUES ($1, $2) \
                 ON CONFLICT (account_id) DO UPDATE SET lifetime_excess = EXCLUDED.lifetime_excess",
            ).bind(acct).bind(val).execute(&pool).await.expect("seed");
        }
        // Run the loop's folds through the SAME upsert the handler uses.
        let mut total = 0_i64;
        for _ in 0..11 {
            total = fold_excess(total, 0, FASTFWD_SLACK_TICKS as i64);
            sqlx::query(
                "INSERT INTO integrity (account_id, lifetime_excess) VALUES ($1, $2) \
                 ON CONFLICT (account_id) DO UPDATE SET lifetime_excess = EXCLUDED.lifetime_excess",
            ).bind(cheat).bind(total).execute(&pool).await.expect("upsert");
        }
        let (back,): (i64,) = sqlx::query_as("SELECT lifetime_excess FROM integrity WHERE account_id = $1")
            .bind(cheat).fetch_one(&pool).await.expect("read back");
        assert!(back > EXCESS_THRESHOLD, "the counter survives as a value, not just in memory");

        // The review query ranks worst-first, so the cheater outranks the honest account.
        let rows: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT account_id, lifetime_excess FROM integrity ORDER BY lifetime_excess DESC LIMIT 50",
        ).fetch_all(&pool).await.expect("review query");
        let pos = |a: i64| rows.iter().position(|(id, _)| *id == a).expect("present");
        assert!(pos(cheat) < pos(honest), "worst-first: the cheater sorts above the floor-pinned account");
    }

    /// The mileage-exchange SQL, executed for real — same reasoning as the roll test above.
    /// Covers the two things only a live database can catch: that a partial balance is refused
    /// WITHOUT being debited, and that the idempotency key is single-use.
    #[tokio::test]
    async fn pg_gacha_exchange_statements_round_trip() {
        let Some((pool, _pg_guard)) = setup().await else { return };
        let acct: i64 = 5150;
        let idem = "idem-exchange-1";
        let def = "regalia_litany_of_dawn"; // deliberately an OFF-BANNER unit

        // One ticket short: the exchange must refuse and leave the balance alone.
        sqlx::query("INSERT INTO gacha_mileage (account_id, tickets) VALUES ($1, $2) ON CONFLICT (account_id) DO UPDATE SET tickets = EXCLUDED.tickets")
            .bind(acct).bind((gacha::MILEAGE_COST - 1) as i64)
            .execute(&pool).await.expect("seed mileage");
        let (short,): (i64,) = sqlx::query_as("SELECT tickets FROM gacha_mileage WHERE account_id = $1")
            .bind(acct).fetch_one(&pool).await.unwrap();
        let mut m = gacha::Mileage { tickets: short as u64 };
        assert_eq!(m.exchange(def), Err(gacha::GachaError::InsufficientMileage));
        assert_eq!(m.tickets, gacha::MILEAGE_COST - 1, "a refused exchange spends nothing");

        // Top up by one and run the handler's statements in order.
        m.earn(1);
        let mut tx = pool.begin().await.expect("begin");
        let r = sqlx::query("INSERT INTO gacha_exchanges (idem_key, account_id, def_id, spent) VALUES ($1, $2, $3, 0) ON CONFLICT DO NOTHING")
            .bind(idem).bind(acct).bind(def)
            .execute(&mut *tx).await.expect("claim key");
        assert_eq!(r.rows_affected(), 1, "first claim wins");
        let (tk,): (i64,) = sqlx::query_as("SELECT tickets FROM gacha_mileage WHERE account_id = $1 FOR UPDATE")
            .bind(acct).fetch_one(&mut *tx).await.expect("lock mileage");
        let mut live = gacha::Mileage { tickets: tk as u64 + 1 }; // the earn(1) above
        let spent = live.exchange(def).expect("affordable now");
        assert_eq!(spent, gacha::MILEAGE_COST);
        sqlx::query("INSERT INTO gacha_mileage (account_id, tickets) VALUES ($1, $2) ON CONFLICT (account_id) DO UPDATE SET tickets = EXCLUDED.tickets")
            .bind(acct).bind(live.tickets as i64)
            .execute(&mut *tx).await.expect("mileage upsert");
        sqlx::query("UPDATE gacha_exchanges SET spent = $1 WHERE idem_key = $2")
            .bind(spent as i64).bind(idem)
            .execute(&mut *tx).await.expect("spent update");
        tx.commit().await.expect("commit");

        let (after,): (i64,) = sqlx::query_as("SELECT tickets FROM gacha_mileage WHERE account_id = $1")
            .bind(acct).fetch_one(&pool).await.expect("read back");
        assert_eq!(after, 0, "exactly MILEAGE_COST left the pool");
        let (sdef, sspent): (String, i64) = sqlx::query_as("SELECT def_id, spent FROM gacha_exchanges WHERE idem_key = $1")
            .bind(idem).fetch_one(&pool).await.expect("exchange row");
        assert_eq!((sdef.as_str(), sspent as u64), (def, gacha::MILEAGE_COST));

        // IDEMPOTENCY: the key is single-use, so a retry reads the row instead of spending again.
        let again = sqlx::query("INSERT INTO gacha_exchanges (idem_key, account_id, def_id, spent) VALUES ($1, $2, $3, 0) ON CONFLICT DO NOTHING")
            .bind(idem).bind(acct).bind(def)
            .execute(&pool).await.expect("replay");
        assert_eq!(again.rows_affected(), 0, "the key is taken");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every item a schedule track hands out must EXIST in the catalog.
    ///
    /// A typo does not fail loudly anywhere: `/claim/collect` writes the mail regardless, the cell
    /// is marked claimed, and only then does `claim_mail` find no such def, return false and leave
    /// the mail sitting in the inbox — permanently unclaimable, with the cell already spent. The
    /// player is left holding a reward they can see and can never take.
    ///
    /// `schedule.rs` cannot check this itself: it is a closed core with no catalog, deliberately,
    /// so the check lives here where GameData is loaded.
    #[test]
    fn every_scheduled_reward_names_an_item_that_exists() {
        let data = GameData::from_json_str(include_str!("../../../data/game_data.json"))
            .expect("game_data.json must parse");
        let mut checked = 0;
        for t in schedule::tracks() {
            for cell in &t.entries {
                for g in &cell.reward {
                    if let schedule::Grant::Item { def_id, qty } = g {
                        assert!(data.item(def_id).is_some(),
                            "{} cell {} hands out `{def_id}`, which is not in the catalog — the \
                             mail would be undeliverable and the cell already spent", t.id, cell.id);
                        assert!(*qty > 0, "{} cell {} grants 0 of `{def_id}`", t.id, cell.id);
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 0, "no item grants were checked — this test would pass on an empty table");
    }

    /// The client MUST stop ticking below `MAX_REPLAY_SPAN`. Past that bound the server refuses
    /// every bundle, and the span only shrinks on a CONFIRMED push — so a client that sails over
    /// the cap can never save again. The two constants live in different languages, so nothing
    /// but this test keeps them ordered; lowering the server cap under the client's threshold
    /// would brick long-disconnected sessions silently.
    ///
    /// Reads the .gd at RUNTIME (not `include_str!`) on purpose: the server must not fail to
    /// BUILD because a Godot file moved. A missing file skips, a present-but-unparseable one
    /// fails — the constant disappearing is exactly the drift worth catching.
    #[test]
    fn the_client_stalls_before_the_server_refuses_its_bundle() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../godot/sim_driver_core.gd");
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("skip: {path} not present (packaged build?)");
            return;
        };
        let line = src
            .lines()
            .find(|l| l.trim_start().starts_with("const SPAN_STALL_TICKS"))
            .expect("sim_driver_core.gd must declare SPAN_STALL_TICKS — the client-side span guard");
        let client: u64 = line
            .rsplit(':')
            .next()
            .and_then(|v| v.trim().trim_start_matches('=').trim().parse().ok())
            .unwrap_or_else(|| panic!("could not read a number out of: {line}"));
        assert!(
            client < MAX_REPLAY_SPAN,
            "client stalls at {client} but the server refuses past {MAX_REPLAY_SPAN} — a client \
             that reaches the cap can never save again",
        );
        // Enough headroom for the ticks that accrue during the async confirm round-trip.
        assert!(
            MAX_REPLAY_SPAN - client >= 1_000,
            "only {} ticks of headroom between the client stall and the server cap",
            MAX_REPLAY_SPAN - client,
        );
        // The span cap is not the tighter bound. A client that stalls the FULL span and then
        // reconnects to a just-restarted server presents its whole backlog against the
        // restart grace — over that, the push is rejected AND cheat-flagged, so the stall
        // would lose the progress it exists to protect.
        // Not just `<`: the grace is deliberately kept small (it is farmable — see the constant),
        // so the gap between it and the stall is the thing at risk of being squeezed to nothing.
        let restart_budget = (RESTART_GRACE_MS / MS_PER_TICK) as u64 + FASTFWD_SLACK_TICKS;
        assert!(
            client + 500 <= restart_budget,
            "client stalls at {client} ticks but a post-restart push only has {restart_budget} \
             of fast-forward budget — the backlog would be flagged as a fast-forward",
        );
    }

    /// The scenario the constants above encode, run through the real predicate: a client that
    /// stalled its full backlog reconnects seconds after a redeploy. Before the restart grace
    /// this was a `fast-forward` rejection plus a durable cheat flag on an honest player.
    #[test]
    fn a_stalled_backlog_survives_a_redeploy_instead_of_being_flagged() {
        let boot = 1_000_000_000_i64;
        let floor = boot - RESTART_GRACE_MS;
        let tick = 500_000_u64;
        // The client stalled 5 min ago, which is also when it last saved — those are the same
        // moment by definition, since a backlog IS the play since the last confirmed save.
        let anchor = (tick, boot_anchor_ms(boot - 5 * 60 * 1000, floor));
        // Its next 10s push lands 8s after the machine answers, carrying the backlog.
        let push_at = boot + 8_000;
        let backlog = 3_000_u64;
        assert!(
            within_tick_rate(anchor, tick + backlog, push_at),
            "a stalled client's backlog must survive a redeploy",
        );
        // Without any backdating this is exactly the false positive: same push, anchor at boot.
        assert!(
            !within_tick_rate((tick, boot), tick + backlog, push_at),
            "guard the regression: un-backdated anchor flags the same honest push",
        );
        // Still a BOUND — a skipped handshake must not launder a whole offline session.
        let offline_gap = 8 * 60 * 60 * 10; // 8h of ticks
        assert!(
            !within_tick_rate(anchor, tick + offline_gap, push_at),
            "the grace must not let a skipped handshake claim hours of offline progress",
        );
    }

    /// What anchoring at the account's OWN save time buys: the sleep-and-wake trade stops paying.
    /// `fly.toml` runs `min_machines_running = 0`, so anyone alone on the server can idle the
    /// machine down and wake it to collect the boot grace. The budget now equals the age of their
    /// last save, so they can only claim wall time that genuinely elapsed — the same progress
    /// they would have had by leaving the game running.
    #[test]
    fn waking_a_slept_machine_pays_only_the_time_that_actually_passed() {
        // Must be well UNDER RESTART_GRACE_MS or the clamp hides the effect: at exactly the
        // grace, "save age" and "flat backdate" are the same number and this proves nothing.
        let idle_ms = 2 * 60 * 1000_i64; // idle the machine down, then wake it
        assert!(idle_ms * 2 < RESTART_GRACE_MS, "the save age must be the binding term here");
        let boot = 1_000_000_000_i64 + idle_ms;
        let floor = boot - RESTART_GRACE_MS;
        let tick = 500_000_u64;
        // They pushed a save immediately before going idle, so their save is `idle_ms` old.
        let anchor = (tick, boot_anchor_ms(boot - idle_ms, floor));
        let push_at = boot + 1_000;

        // Claiming the idle time itself is allowed — it really elapsed, and leaving the game
        // running would have earned it. That is the point: no PROFIT, not "no progress".
        let earned = (idle_ms / MS_PER_TICK) as u64;
        assert!(within_tick_rate(anchor, tick + earned, push_at));
        // Claiming meaningfully more is not.
        assert!(
            !within_tick_rate(anchor, tick + earned * 2, push_at),
            "the wake must not pay more than the sleep cost",
        );

        // And the clamp still holds the other end: an ancient save (or a pre-migration row with
        // saved_at_ms = 0) gets the grace, never its full age.
        assert_eq!(boot_anchor_ms(0, floor), floor, "row predating the column lands on the floor");
        assert_eq!(boot_anchor_ms(boot - 8 * 60 * 60 * 1000, floor), floor, "8h-old save is clamped");
        let ancient = (tick, boot_anchor_ms(0, floor));
        let grace_ticks = (RESTART_GRACE_MS / MS_PER_TICK) as u64;
        assert!(!within_tick_rate(ancient, tick + grace_ticks * 4, push_at));
    }

    /// The hole layer 2 exists for: `auth → GET /save → POST /save (+SLACK)` in a tight loop.
    /// Layer 1 accepts every pass (each one re-anchors, and +300 sits exactly on the slack), so
    /// only the cumulative counter sees it.
    #[test]
    fn the_relogin_loop_trips_the_lifetime_counter() {
        let mut t = 1_000_000_000_i64;
        let mut tick = 500_000_u64;
        let mut lifetime = 0_i64;
        let mut passes = 0;
        while lifetime <= EXCESS_THRESHOLD {
            let anchor = (tick, t); // fresh handshake
            t += 50; // a tight loop: the push lands ~immediately
            tick += FASTFWD_SLACK_TICKS; // mint exactly what layer 1 allows
            assert!(within_tick_rate(anchor, tick, t), "layer 1 accepts every pass — that is the hole");
            lifetime = fold_excess(lifetime, 0, session_excess(anchor, tick, t));
            passes += 1;
            assert!(passes < 100, "counter is not climbing — the loop would be unbounded");
        }
        // ~40 passes/min is the per-IP rate-limit ceiling (120 req/min ÷ 3 requests per pass).
        assert_eq!(passes, 11, "trips in {passes} passes ≈ {}s at the rate-limit ceiling", passes * 60 / 40);
    }

    /// The false positive the rate tolerance exists to prevent, and the reason the doc's
    /// compare-against-exactly-1.0× could not be used here. A client whose clock runs 200 ppm
    /// fast is honest; over a day of connected play it must NOT accumulate.
    #[test]
    fn a_fast_client_clock_can_never_accumulate() {
        let t0 = 1_000_000_000_i64;
        let day_ms = 24 * 60 * 60 * 1000_i64;
        // 200 ppm fast: the client counts 1.0002 ticks for every tick of server time.
        let ticks = (day_ms as u64 / MS_PER_TICK as u64) * 10_002 / 10_000;
        let anchor = (0_u64, t0);
        assert!(
            session_excess(anchor, ticks, t0 + day_ms) < 0,
            "200 ppm drift must read as NEGATIVE, or a lifetime counter eventually flags it",
        );
        // Guard the regression: without the tolerance this same honest day is POSITIVE, which
        // is what would have walked an idle player to the threshold in a couple of weeks.
        let untolerated = ticks as i64 - (day_ms / MS_PER_TICK);
        assert!(untolerated > 0, "the drift really is positive against a bare 1.0× compare");

        // And the tolerance costs nothing against a real cheat — the gap to 1.05× is ~250×, so
        // the subtle-cheat case the whole system is FOR still trips inside two hours.
        let two_h = 2 * 60 * 60 * 1000_i64;
        let cheat = (two_h as u64 / MS_PER_TICK as u64) * 105 / 100;
        assert!(
            session_excess(anchor, cheat, t0 + two_h) > EXCESS_THRESHOLD,
            "a 1.05× run must still be caught quickly",
        );
    }

    /// Honest play reads negative every session and pins at the floor, so it is never near the
    /// threshold — and the floor bounds how much cheat budget a long history can bank.
    #[test]
    fn honest_play_pins_at_the_floor_and_the_floor_bounds_the_bank() {
        let mut lifetime = 0_i64;
        for session in 0..500 {
            let t0 = 1_000_000_000_i64 + session * 10_000_000;
            let hour_ms = 3_600_000_i64;
            // Real-time ticking, minus ~2s of load after the handshake before the sim starts.
            let ticks = ((hour_ms - 2_000) / MS_PER_TICK) as u64;
            let e = session_excess((0, t0), ticks, t0 + hour_ms);
            assert!(e < 0, "an honest session reads negative (session {session}, excess {e})");
            lifetime = fold_excess(lifetime, 0, e);
        }
        assert_eq!(lifetime, EXCESS_FLOOR, "500 honest sessions bank no more than the floor");
        // From the floor, the re-login loop still trips in a bounded number of passes.
        let passes = (EXCESS_THRESHOLD - EXCESS_FLOOR) / FASTFWD_SLACK_TICKS as i64 + 1;
        assert!(passes <= 25, "a long-lived honest account would need {passes} passes to trip");
    }

    /// Excess is measured from the session ANCHOR, so it spans the whole session and every save
    /// re-presents it. Without the `recorded` delta a client would inflate the counter — or dodge
    /// it — purely by choosing how often to save.
    #[test]
    fn saving_more_often_does_not_change_the_counter() {
        let t0 = 1_000_000_000_i64;
        let anchor = (0_u64, t0);
        // One save at the end of a 60s window vs six saves through it must agree.
        let ticks_at = |s: i64| ((s * 1000 - 500) / MS_PER_TICK) as u64;
        let once = fold_excess(0, 0, session_excess(anchor, ticks_at(60), t0 + 60_000));
        let mut many = 0_i64;
        let mut recorded = 0_i64;
        for s in (10..=60).step_by(10) {
            let e = session_excess(anchor, ticks_at(s as i64), t0 + s as i64 * 1000);
            many = fold_excess(many, recorded, e);
            recorded = e;
        }
        assert_eq!(once, many, "the counter must not depend on save frequency");
    }

    #[test]
    fn fastfwd_allows_realtime_play_rejects_a_14h_burst() {
        let anchor = (1_000u64, 10_000i64); // tick 1000, anchored at t=10s
        // 10s later, +100 ticks (one autosave of real play) → fine.
        assert!(within_tick_rate(anchor, 1_100, 20_000));
        // 2s later, claiming +500k ticks (~14h) in a burst → rejected.
        assert!(!within_tick_rate(anchor, 501_000, 12_000));
        // Same real second, a big jump → rejected (spam can't beat a fixed anchor).
        assert!(!within_tick_rate(anchor, 50_000, 10_000));
    }

    #[test]
    fn fastfwd_slack_covers_a_short_push_and_a_backward_clock() {
        let anchor = (0u64, 100_000i64);
        // ~0 elapsed, a small push within the flat slack → allowed.
        assert!(within_tick_rate(anchor, FASTFWD_SLACK_TICKS, 100_000));
        assert!(!within_tick_rate(anchor, FASTFWD_SLACK_TICKS + 1, 100_000));
        // Clock ran backwards (now < anchor): elapsed clamps to 0, slack still applies.
        assert!(within_tick_rate(anchor, 200, 90_000));
        assert!(!within_tick_rate(anchor, 500_000, 90_000));
    }

    /// The ADDITIVE slack (vs a multiplicative ×2 margin) catches a SUSTAINED slow cheat:
    /// its detectable margin shrinks as the session runs, so a steady 1.5× is rejected once
    /// its excess passes the slack (~5 min) — where a ×2 margin would have let it farm forever.
    #[test]
    fn fastfwd_additive_slack_catches_sustained_slow_cheat() {
        let anchor = (0u64, 0i64); // tick 0 at t=0, SLACK = 300
        // Legit 1.0× for an hour: 36000 ticks in 3_600_000 ms → within (== elapsed, < slack over).
        assert!(within_tick_rate(anchor, 36_000, 3_600_000));
        // 1.5× for 40s (400 legit ticks → 600): allowed 400+300=700 → not yet caught.
        assert!(within_tick_rate(anchor, 600, 40_000));
        // 1.5× sustained to 90s (900 legit → 1350): allowed 900+300=1200 → REJECTED (~60s in).
        // (An old ×2 margin allowed 900×2+300 = 2100 → would have MISSED it.)
        assert!(!within_tick_rate(anchor, 1_350, 90_000));
    }

    fn oroll(nonce: u64) -> OutstandingRoll {
        OutstandingRoll { key: "k".into(), attempt: 0, nonce, issued_at: SystemTime::now() }
    }

    #[test]
    fn prune_outstanding_keeps_future_dated_rolls() {
        // A roll stamped in the FUTURE (clock skew) must not be pruned as stale.
        let now = SystemTime::now();
        let mut skewed = oroll(33);
        skewed.issued_at = now + Duration::from_secs(600);
        let mut rolls = vec![skewed, oroll(44)];
        rolls[1].issued_at = now - Duration::from_secs(7200);
        assert_eq!(prune_outstanding(&mut rolls, now, Duration::from_secs(3600)), vec![44]);
        assert_eq!(rolls.len(), 1, "future-dated roll kept, the two-hour orphan pruned");
    }

    #[test]
    fn reconcile_consume_all_when_every_issued_roll_is_used() {
        let (mut out, mut consumed) = (vec![oroll(11), oroll(22)], HashSet::new());
        let r = reconcile_fuse(&mut out, &mut consumed, &[11, 22]);
        assert!(out.is_empty(), "all consumed");
        assert!(r.forged.is_empty() && r.unconsumed.is_empty(), "no forgeries, nothing unconsumed");
        assert_eq!(r.consumed, vec![11, 22], "both matched this pass");
    }

    #[test]
    fn reconcile_flags_peek_and_skip_as_unconsumed() {
        // Issued a roll (a failure), never used it → it persists, flagged.
        let (mut out, mut consumed) = (vec![oroll(11), oroll(22)], HashSet::new());
        let r = reconcile_fuse(&mut out, &mut consumed, &[11]); // used 11, skipped 22
        assert_eq!(r.forged, Vec::<u64>::new());
        assert_eq!(r.unconsumed, vec![22], "the dropped failure is a peek-and-skip suspect");
        assert_eq!(out.iter().map(|o| o.nonce).collect::<Vec<_>>(), vec![22], "kept for next sync");
    }

    #[test]
    fn reconcile_rejects_a_forged_unissued_nonce() {
        let (mut out, mut consumed) = (vec![oroll(11)], HashSet::new());
        let r = reconcile_fuse(&mut out, &mut consumed, &[11, 999]); // 999 never issued
        assert_eq!(r.forged, vec![999], "a nonce we never issued is unforgeable → forgery");
        assert!(out.is_empty(), "the legit one still consumed");
    }

    #[test]
    fn reconcile_is_idempotent_across_a_growing_log_resave() {
        // Save #1 consumes nonce 11. Save #2 (same session, growing log) repeats 11 — it is now
        // in `consumed`, so it must NOT be re-flagged as a forgery.
        let (mut out, mut consumed) = (vec![oroll(11)], HashSet::new());
        let r1 = reconcile_fuse(&mut out, &mut consumed, &[11]);
        assert_eq!(r1.consumed, vec![11], "matched on the first save");
        let r2 = reconcile_fuse(&mut out, &mut consumed, &[11]); // re-save
        assert_eq!(r2.forged, Vec::<u64>::new(), "a re-saved already-consumed nonce is fine");
        assert!(r2.consumed.is_empty(), "not re-consumed → no duplicate DB write");
    }

    /// A bag holding six of `def` at `grade` plus one odd piece, so pick sets can differ.
    fn fuse_ready(grade: sim_core::items::Grade) -> sim_core::state::GameState {
        use sim_core::items::{Item, ItemKind, Rarity};
        let mut s = sim_core::state::GameState::new(1, 8, 8);
        for d in ["sword_s3", "sword_s3", "sword_s3", "sword_s3", "sword_s3", "sword_s4", "sword_s5"] {
            let mut it = Item::new(d, ItemKind::Weapon, 10, 0, Rarity::Common);
            it.grade = grade;
            s.inventory.push(it);
        }
        s
    }

    #[test]
    fn fuse_bind_check_accepts_the_bound_nonce_and_rejects_forgery_and_target_selection() {
        use sim_core::items::Grade;
        let (secret, acct) = ("s", 7u64);
        let s = fuse_ready(Grade::Rare); // Rare in → Legendary out: needs the server
        let picks = [0, 1, 2, 3, 4, 5];
        let (key, attempt) = sim_core::fusion::binding(&s, &picks).expect("bindable");
        // The nonce actually bound to THESE six at THIS attempt → Ok, in any pick order.
        let good = fuse_nonce(secret, acct, &key, attempt);
        assert_eq!(verify_fuse_binding(secret, acct, &s, &picks, good), FuseVerdict::Ok);
        assert_eq!(verify_fuse_binding(secret, acct, &s, &[5, 4, 3, 2, 1, 0], good), FuseVerdict::Ok);
        // A nonce issued for a DIFFERENT six (swap the s4 for the s5) applied here → BadNonce.
        let (other_key, _) = sim_core::fusion::binding(&s, &[0, 1, 2, 3, 4, 6]).expect("bindable");
        assert_ne!(other_key, key);
        let other = fuse_nonce(secret, acct, &other_key, attempt);
        assert_eq!(verify_fuse_binding(secret, acct, &s, &picks, other), FuseVerdict::BadNonce);
        // The SAME six but a different attempt (a re-roll) → BadNonce.
        let stale = fuse_nonce(secret, acct, &key, attempt + 1);
        assert_eq!(verify_fuse_binding(secret, acct, &s, &picks, stale), FuseVerdict::BadNonce);
        // Another account's nonce for the identical combine → BadNonce.
        let theirs = fuse_nonce(secret, acct + 1, &key, attempt);
        assert_eq!(verify_fuse_binding(secret, acct, &s, &picks, theirs), FuseVerdict::BadNonce);
    }

    #[test]
    fn fuse_bind_check_rejects_a_high_fusion_with_no_roll_but_allows_a_low_one() {
        use sim_core::items::Grade;
        let (secret, acct) = ("s", 7u64);
        let picks = [0, 1, 2, 3, 4, 5];
        // Rare → Legendary rolled locally: the client kept the seed's steering wheel.
        assert_eq!(
            verify_fuse_binding(secret, acct, &fuse_ready(Grade::Rare), &picks, 0),
            FuseVerdict::HighNoRoll,
        );
        // Uncommon → Rare needs no roll → Ok at 0.
        assert_eq!(verify_fuse_binding(secret, acct, &fuse_ready(Grade::Uncommon), &picks, 0), FuseVerdict::Ok);
        // A combine the sim rejects (mixed grades) is a no-op and nothing to bind, roll or not.
        let mut mixed = fuse_ready(Grade::Rare);
        mixed.inventory[0].grade = Grade::Common;
        assert_eq!(verify_fuse_binding(secret, acct, &mixed, &picks, 0), FuseVerdict::Ok);
        assert_eq!(verify_fuse_binding(secret, acct, &mixed, &picks, 12345), FuseVerdict::Ok);
    }

    /// The replay walk: a log whose fusion carries the right nonce passes; the same log with the
    /// nonce swapped for another combine's fails at that tick. Runs the real catalog so the
    /// fusion actually resolves (and moves the attempt counter for a second one).
    #[test]
    fn fuse_bindings_walk_the_log_and_bind_each_fusion_to_its_own_attempt() {
        use sim_core::events::RecordedInput;
        use sim_core::items::Grade;
        let (secret, acct) = ("s", 7u64);
        let d = test_data();
        let start = fuse_ready(Grade::Rare);
        let picks = [0u32, 1, 2, 3, 4, 5];
        let (key, a0) = sim_core::fusion::binding(&start, &picks).expect("bindable");
        assert_eq!(a0, 0);
        let n0 = fuse_nonce(secret, acct, &key, 0);
        let log = vec![RecordedInput { tick: 0, event: InputEvent::FuseGear { picks, server_roll: n0 } }];
        assert_eq!(verify_fuse_bindings(secret, acct, &start, &d, &log), Ok(()));
        // The nonce for attempt 1 spent at attempt 0 — a re-roll — fails at tick 0.
        let n1 = fuse_nonce(secret, acct, &key, 1);
        let bad = vec![RecordedInput { tick: 0, event: InputEvent::FuseGear { picks, server_roll: n1 } }];
        assert!(verify_fuse_bindings(secret, acct, &start, &d, &bad).unwrap_err().contains("tick 0"));
        // No fusion in the log at all → nothing to walk, Ok without a replay.
        let quiet = vec![RecordedInput { tick: 3, event: InputEvent::EquipItem { entity_id: 1, inventory_idx: 0 } }];
        assert_eq!(verify_fuse_bindings(secret, acct, &start, &d, &quiet), Ok(()));
    }

    #[test]
    fn fuse_nonces_used_picks_nonzero_rolls_only() {
        use sim_core::events::RecordedInput;
        let p = [0u32, 1, 2, 3, 4, 5];
        let log = vec![
            RecordedInput { tick: 1, event: InputEvent::FuseGear { picks: p, server_roll: 42 } },
            RecordedInput { tick: 2, event: InputEvent::FuseGear { picks: p, server_roll: 0 } },
            RecordedInput { tick: 3, event: InputEvent::FuseGear { picks: p, server_roll: 77 } },
            RecordedInput { tick: 4, event: InputEvent::EquipItem { entity_id: 1, inventory_idx: 0 } },
        ];
        assert_eq!(fuse_nonces_used(&log), vec![42, 77]);
    }

    fn test_data() -> GameData {
        GameData::from_json_str(include_str!("../../../data/game_data.json")).unwrap()
    }

    #[test]
    fn a_freshly_created_character_anchors_to_a_genesis_candidate() {
        // The client's create_character does new_game_as(seed, class) with the party NAME
        // left out of the start (it's a SetPartyName event). That start MUST be one of the
        // server's genesis candidates — else no online save anchors (the bug we hit). Uses a
        // real > 2^53 seed to also pin that the seed round-trips exactly (no float loss).
        let data = test_data();
        let seed: u64 = 331_144_809_649_603_285;
        let candidates = genesis_candidates(&data, seed);
        let pair = data::new_starter_party(seed, &data, START_ZONE).unwrap();
        assert!(candidates.contains(&hash::state_hash(&pair)), "fixed starter pair must anchor");
        let other_seed = data::new_starter_party(seed + 1, &data, START_ZONE).unwrap();
        assert!(!candidates.contains(&hash::state_hash(&other_seed)), "wrong server seed must not anchor");
        let bare = data::new_game_in_zone(seed, &data, START_ZONE).unwrap();
        assert!(candidates.contains(&hash::state_hash(&bare)), "bare new-game must anchor");
        let mut classes = 0;
        for class in data.creatable_units() {
            let start = data::new_game_as(seed, &data, START_ZONE, &class.id).unwrap();
            assert!(
                candidates.contains(&hash::state_hash(&start)),
                "a created {} character's start must anchor to a genesis candidate",
                class.id
            );
            classes += 1;
        }
        assert!(classes > 0, "there must be creatable classes to test");
    }

    #[test]
    fn the_account_seed_serializes_as_a_json_string_not_a_number() {
        // The seed exceeds 2^53 → it MUST be a JSON string, or the client's float-based parser
        // corrupts it and no online save could anchor.
        let auth = AuthResp {
            token: "t".into(),
            account_id: 5,
            name: "x".into(),
            seed: "331144809649603285".into(),
            sim_version: "0.0.0".into(),
            game_data_version: "0".into(),
            sim_build: "0".into(),
        };
        assert!(serde_json::to_value(&auth).unwrap()["seed"].is_string(), "seed must be a JSON string");
    }

    #[test]
    fn commander_grants_validate_the_nonce() {
        use sim_core::events::RecordedInput;
        // Only the server can compute commander_nonce, so a forged unlock rejects. The nonce is bound
        // to its OWN skill id AND its own domain: neither a different skill nor a drop nonce stands in.
        let secret = "s3cret";
        let acct = 42u64;
        let gc = |skill: &str, nonce: u64| RecordedInput {
            tick: 1,
            event: InputEvent::GrantCommanderSkill { skill_id: skill.into(), nonce },
        };
        let smite = commander_nonce(secret, acct, "commander_smite");
        assert!(commander_grants_valid(secret, acct, &[gc("commander_smite", smite)]).is_ok());
        assert_eq!(
            commander_grants_valid(secret, acct, &[gc("commander_smite", 1234)]),
            Err("commander_smite".into()),
            "a forged nonce rejects",
        );
        assert_eq!(
            commander_grants_valid(secret, acct, &[gc("commander_bulwark", smite)]),
            Err("commander_bulwark".into()),
            "a nonce for one skill is not valid for another",
        );
        assert_eq!(
            commander_grants_valid(secret, acct, &[gc("commander_smite", drop_nonce(secret, acct, 0))]),
            Err("commander_smite".into()),
            "a drop nonce is not a commander nonce (domain separation)",
        );
    }

    #[test]
    fn drop_grants_validate_the_nonce() {
        use sim_core::events::RecordedInput;
        // The nonce IS the authorisation: only the server can compute it, and it only issues
        // one for a candidate that HIT. A forged roll rejects.
        let secret = "s3cret";
        let acct = 42u64;
        let gd = |seq: u64, roll: u64| RecordedInput {
            tick: 1,
            event: InputEvent::GrantDrop { candidate_seq: seq, server_roll: roll },
        };
        assert!(drop_grants_valid(secret, acct, &[gd(7, drop_nonce(secret, acct, 7))]).is_ok());
        assert_eq!(drop_grants_valid(secret, acct, &[gd(7, 1234)]), Err(7));
        // A nonce is bound to its OWN seq — replaying one against a different candidate fails.
        assert_eq!(drop_grants_valid(secret, acct, &[gd(8, drop_nonce(secret, acct, 7))]), Err(8));
        // The grant response is string-typed (client float-parser precision).
        let g = DropGrant { candidate_seq: 1, server_roll: "9007199254740993".into() };
        assert!(serde_json::to_value(&g).unwrap()["server_roll"].is_string());
    }

    /// STAGE 2 reproduces every authored final rate exactly. For an item with authored
    /// `server_bp` on a monster whose entries sum to W, the pipeline is
    /// `1% x (bp/W) x hit_ppm/1e6`, and that must equal the old `bp/1e4 x 1/1000`.
    #[test]
    fn stage2_preserves_every_authored_final_rate() {
        for (bp, total) in [(50u32, 50u32), (50, 13_250), (4000, 13_250), (2500, 5050)] {
            let ppm = sim_core::drops::stage2_hit_ppm(total);
            // final, scaled by 1e12 to stay in integers
            let got = 1_000_000_000_000u128
                * sim_core::drops::CANDIDATE_RATE_BP as u128 * bp as u128 * ppm as u128
                / (10_000u128 * total as u128 * 1_000_000u128);
            let want = 1_000_000_000_000u128 * bp as u128 / 10_000_000u128;
            assert_eq!(got, want, "bp={bp} total={total}: {got} != {want}");
        }
        // And the odds stay a probability — never above 100% for any authored total.
        assert!(sim_core::drops::stage2_hit_ppm(13_250) < 1_000_000);
    }

    #[test]
    fn candidate_hit_rule_reads_the_ticket_not_a_constant() {
        const X1: u32 = RARE_RATE_MULT_DEFAULT_BP;
        assert!(candidate_hits(999_999_000_000, 1_000_000, X1), "100% always pays");
        assert!(!candidate_hits(999_999_000_000, 0, X1), "0% never pays");
        // A 13.25% ticket pays on ~13.25% of nonces.
        let hits = (0..100_000u64).filter(|n| candidate_hits(n * 7_919, 132_500, X1)).count();
        assert!((12_000..15_000).contains(&hits), "got {hits} of 100000");
    }

    /// The live multiplier: OFF unless set, and it really does scale the ticket.
    #[test]
    fn the_live_rate_multiplier_scales_the_ticket_and_is_off_by_default() {
        let sample = |ppm: u32, mult: u32| {
            (0..200_000u64).filter(|n| candidate_hits(n * 7_919, ppm, mult)).count()
        };
        // Cards are 1,000 ppm. x1 must reproduce that exactly — deploying the dial unset
        // changes nothing about any rate.
        let base = sample(1_000, RARE_RATE_MULT_DEFAULT_BP);
        assert!((150..250).contains(&base), "x1 is ~0.1% of 200k = ~200, got {base}");
        let ten = sample(1_000, 100_000);
        let ratio = ten as f64 / base.max(1) as f64;
        assert!((8.0..12.0).contains(&ratio), "x10 gave {ten} against a base of {base}");
        // A multiplier past 100% saturates at always-pays rather than wrapping to never-pays.
        assert_eq!(sample(500_000, RARE_RATE_MULT_MAX_BP), 200_000);
    }

    /// The env parse itself — the clamp is the only thing standing between a typo and either
    /// "every drop is a jackpot" or "no drop ever lands again".
    #[test]
    fn the_rate_multiplier_env_is_clamped() {
        assert_eq!(parse_rate_mult(None), RARE_RATE_MULT_DEFAULT_BP, "unset = x1");
        assert_eq!(parse_rate_mult(Some("")), RARE_RATE_MULT_DEFAULT_BP, "empty = x1");
        assert_eq!(parse_rate_mult(Some("banana")), RARE_RATE_MULT_DEFAULT_BP, "garbage = x1");
        assert_eq!(parse_rate_mult(Some("100000")), 100_000, "x10 passes through");
        assert_eq!(parse_rate_mult(Some("0")), 100, "0 would switch drops OFF — floored");
        assert_eq!(parse_rate_mult(Some("4294967295")), RARE_RATE_MULT_MAX_BP, "capped");
    }

    /// The prune watermark. Wrong here means one of two things, and they fail in opposite
    /// directions: too LOW and duds pile up forever (the bug prune exists to fix), too HIGH and
    /// the client deletes tickets the server never judged — silent loss, the worse one.
    #[test]
    fn the_prune_watermark_is_the_highest_judged_drop_seq() {
        use sim_core::state::{CandidateKind, GameState, RareCandidate};
        let mk = |kinds: &[(u64, CandidateKind)]| {
            let mut st = GameState::new(1, 8, 8);
            for (seq, kind) in kinds {
                st.rare_candidates.push(RareCandidate {
                    seq: *seq, def_id: "card_chomb".into(), tick: 0,
                    kind: *kind, hit_ppm: 1_000,
                });
            }
            st
        };
        let gear = CandidateKind::Gear { boss: false };

        assert_eq!(resolved_drop_seq(&mk(&[])), None, "nothing pending — do not prune");
        assert_eq!(
            resolved_drop_seq(&mk(&[(1, CandidateKind::Craft), (2, gear)])), None,
            "craft and gear are never JUDGED — they always pay, so they set no watermark",
        );
        assert_eq!(
            resolved_drop_seq(&mk(&[(3, CandidateKind::Drop), (1, CandidateKind::Drop)])), Some(3),
            "the highest drop seq, regardless of list order",
        );
        // THE ONE THAT MATTERS. A gear ticket sitting above the drops must not lift the
        // watermark past them: the client prunes DROP tickets at or below it, and anything it
        // has accrued since the replay is unjudged.
        assert_eq!(
            resolved_drop_seq(&mk(&[(4, CandidateKind::Drop), (9, gear), (7, CandidateKind::Craft)])),
            Some(4),
            "a craft/gear seq above the drops must NOT authorise pruning past seq 4",
        );

        // Absent from the JSON when there is nothing to judge — the client reads 0 and skips.
        let none: Option<u64> = None;
        let body = serde_json::json!({ "resolved_seq": none });
        assert!(body["resolved_seq"].is_null());
    }

    /// CRAFT grants validate under their OWN nonce domain, with no hit rule, and the two
    /// domains do not overlap.
    ///
    /// The separation is the security property, not a tidiness one. A craft nonce is issued
    /// for every candidate, so a client always has one in hand; if it also validated as a
    /// drop grant, that client could replay it against a forged history where the same seq
    /// is a card candidate and mint a 1-in-200,000 card from a 30-material craft. Each
    /// direction is asserted below; the sim's kind guard is the second lock.
    #[test]
    fn craft_grants_validate_under_their_own_nonce_domain() {
        use sim_core::events::RecordedInput;
        let secret = "s3cret";
        let acct = 42u64;
        let ev = |seq: u64, roll: u64| RecordedInput {
            tick: 1,
            event: InputEvent::GrantCraft { candidate_seq: seq, server_roll: roll },
        };
        // EVERY seq is grantable — a craft has no stage-2 odds to pass at all, which is the
        // whole difference between a purchase and a lottery ticket.
        for seq in [1u64, 2, 7, 4242] {
            assert!(
                craft_grants_valid(secret, acct, &[ev(seq, craft_nonce(secret, acct, seq))]).is_ok(),
                "seq {seq} is payable — a craft was paid for",
            );
        }
        assert_eq!(craft_grants_valid(secret, acct, &[ev(7, 1234)]), Err(7), "forged roll rejects");
        // Cross-domain, both directions.
        assert_ne!(craft_nonce(secret, acct, 7), drop_nonce(secret, acct, 7));
        assert_eq!(
            craft_grants_valid(secret, acct, &[ev(7, drop_nonce(secret, acct, 7))]),
            Err(7),
            "a DROP nonce is not a valid craft grant",
        );
        let hit_seq = 11u64;
        let as_drop = RecordedInput {
            tick: 1,
            event: InputEvent::GrantDrop {
                candidate_seq: hit_seq,
                server_roll: craft_nonce(secret, acct, hit_seq),
            },
        };
        assert_eq!(
            drop_grants_valid(secret, acct, &[as_drop]),
            Err(hit_seq),
            "a CRAFT nonce is not a valid drop grant",
        );
        // Each validator ignores the other's events entirely — one log carrying both is fine.
        let both = vec![ev(7, craft_nonce(secret, acct, 7)), RecordedInput {
            tick: 2,
            event: InputEvent::GrantDrop {
                candidate_seq: hit_seq,
                server_roll: drop_nonce(secret, acct, hit_seq),
            },
        }];
        assert!(craft_grants_valid(secret, acct, &both).is_ok());
        assert!(drop_grants_valid(secret, acct, &both).is_ok());
    }

    /// A bundle carrying `log`, versions filled in so `policy_check` reaches the authority gate
    /// instead of stopping at a version skew.
    fn bundle_with(data: &GameData, log: Vec<sim_core::events::RecordedInput>) -> SaveBundle {
        SaveBundle {
            sim_version: sim_core::VERSION.into(),
            game_data_version: data.version.clone(),
            start: data::new_game_in_zone(1, data, START_ZONE).unwrap(),
            log,
            final_tick: 0,
            final_hash: String::new(),
            snapshot_interval: 0,
            snapshots: Vec::new(),
            sim_build: String::new(),
        }
    }

    /// The hole H2 closed: a client authoring its OWN x10 card-drop event.
    ///
    /// `SetCardRateBonus` is `InputAuthority::Server`, and that was mistaken for a defence — the
    /// client writes its own input log and computes the hash by replaying it, so the tag stops
    /// nothing. The boost then rides all the way to the value the server trusts:
    /// `accrue_candidate` multiplies each card entry's `server_bp` by it and stores
    /// `hit_ppm: stage2_hit_ppm(total_bp)`, and `drop_grants_valid` deliberately does not
    /// re-derive the odds.
    #[test]
    fn a_client_cannot_author_its_own_card_rate_event() {
        let data = test_data();
        let evil = bundle_with(
            &data,
            vec![sim_core::events::RecordedInput {
                tick: 1,
                event: InputEvent::SetCardRateBonus { bp: sim_core::drops::MAX_CARD_RATE_BONUS_BP },
            }],
        );
        let err = policy_check(&data, &evil).expect_err("a client-authored card event must be rejected");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);

        // And the gate must not be a blanket "no Server-authority events": the ordinary ones are
        // checked downstream and have to keep passing, or every honest save with a mail claim or a
        // server grant in its log breaks.
        let honest = bundle_with(
            &data,
            vec![
                sim_core::events::RecordedInput { tick: 1, event: InputEvent::ClaimMail { mail_id: 7, gil: sim_core::state::Gil::new(76), def_id: String::new(), qty: 0, item: None } },
                sim_core::events::RecordedInput { tick: 2, event: InputEvent::GrantDrop { candidate_seq: 1, server_roll: 12345 } },
                sim_core::events::RecordedInput { tick: 3, event: InputEvent::PruneMarketOps { ids: vec![1] } },
            ],
        );
        assert!(policy_check(&data, &honest).is_ok(), "downstream-checked events must still pass");
    }

    /// C04b — the gacha seed must move when only the SERVER's nonce moves.
    ///
    /// Before it existed the seed was `f(secret, account_id, idem_key)` and the client picks
    /// `idem_key`, so the outcome was the one thing a player could search: try a key, see the
    /// payout, and with a leaked secret grind keys offline until one lands an SSR before spending
    /// a crystal. The nonce is what makes the draw unreachable from the request.
    ///
    /// Also pins that the same nonce still reproduces the same seed — a stored nonce has to give a
    /// stable answer or the idempotent replay of a roll would drift.
    #[test]
    fn a_gacha_roll_cannot_be_steered_by_the_key_the_client_chose() {
        let s = "secret";
        let a = gacha_seed(s, 7, "key-a", 1);
        let b = gacha_seed(s, 7, "key-a", 2);
        assert_ne!(a, b, "same key, different server nonce → the seed must move, or the nonce does nothing");
        assert_eq!(a, gacha_seed(s, 7, "key-a", 1), "same inputs must reproduce — retries replay, not re-roll");
        assert_ne!(a, gacha_seed(s, 7, "key-b", 1), "the key still contributes");
        assert_ne!(a, gacha_seed(s, 8, "key-a", 1), "so does the account");

        // The property that matters, measured rather than asserted on one pair: across many
        // nonces for ONE client-chosen key, the seeds are all distinct — so no amount of picking
        // the key narrows what the roll will be.
        let seeds: std::collections::HashSet<u64> =
            (0..512u64).map(|n| gacha_seed(s, 7, "fixed-key", n)).collect();
        assert_eq!(seeds.len(), 512, "server nonces collapsed onto the same seed");
    }

    /// `STORY_MISSIONS` must be exactly the set of `% id` in `godot/story/`.
    ///
    /// Two lists in two languages with no compiler between them — the drift the story-mode plan
    /// warns about, allowed only because this test exists. Both directions are bugs and neither
    /// announces itself:
    ///
    /// - In the directory, not in the server: the mission plays to the end and the reward call
    ///   comes back "unknown mission". The player finishes a mission and is told it does not exist.
    /// - In the server, not in the directory: a mission id nobody can earn is still payable by
    ///   anyone who guesses it. That is the hole this list was added to close, left half-open.
    ///
    /// Reads the directory at RUNTIME, not `include_str!` — the server must not fail to BUILD
    /// because the Godot tree is absent (packaged/CI). A missing directory skips; a present one
    /// that disagrees fails. Same discipline as `the_client_stalls_before_the_server_refuses_it`.
    #[test]
    fn the_server_pays_for_exactly_the_missions_that_exist() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../godot/story");
        let Ok(entries) = std::fs::read_dir(dir) else {
            eprintln!("skip: {dir} not present (packaged build?)");
            return;
        };
        let mut on_disk: Vec<String> = Vec::new();
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|s| s.to_str()) != Some("dialogue") {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(&path) else { continue };
            if let Some(id) = src
                .lines()
                .map(str::trim)
                .find_map(|l| l.strip_prefix("% id ").map(|v| v.trim().to_string()))
                .filter(|v| !v.is_empty())
            {
                on_disk.push(id);
            }
        }
        on_disk.sort();
        assert!(!on_disk.is_empty(), "no `% id` found in {dir} — this guard checked nothing");

        let mut listed: Vec<String> = STORY_MISSIONS.iter().map(|s| s.to_string()).collect();
        listed.sort();
        assert_eq!(
            listed, on_disk,
            "STORY_MISSIONS and godot/story/ disagree.\n  server: {listed:?}\n  on disk: {on_disk:?}\n\
             A mission missing from the server cannot be paid; one missing from disk can be paid \
             by anyone who guesses it. Update STORY_MISSIONS and REDEPLOY."
        );
    }

    /// The guard that outlives this fix.
    ///
    /// `authority()` and `server_event_is_checked` are two lists in two crates that must agree:
    /// every variant the first one lifts out of the default `Client` authority needs a verdict in
    /// the second, or it is trusted by nobody and checked by nobody — which is exactly how
    /// `SetCardRateBonus` sat unguarded. Nothing binds them but this count.
    ///
    /// It is a COUNT rather than a name-by-name diff on purpose: the arms are written several
    /// ways (`{ .. }`, `{}`, an `|`-joined pair, one with a conditional body), and a parser clever
    /// enough to read all of them is a parser that can be wrong in silence. A count cannot be.
    #[test]
    fn every_server_authored_event_has_a_verdict() {
        // 12 since 0.0.370: `StartRefine` / `StartRefineInventory` left with the refine system.
        // (14 since 0.0.350: `DebugGrantExemplarUnit` (Debug). VERDICT: Debug-authority events are
        // rejected wholesale by the policy gate above (`find(authority() == Debug)`), so a client
        // that authors a free unit grant loses the save — no per-variant check is needed.)
        // (13 since 0.0.377: `FuseGear` — Server when it carries a `server_roll`, checked by
        // `verify_fuse_bindings` + `reconcile_fuse`.)
        const KNOWN_NON_CLIENT_VARIANTS: usize = 13;

        let src = include_str!("../../sim-core/src/events/mod.rs");
        let at = src.find("fn authority(").expect("authority() must exist");
        let open = src[at..].find('{').expect("authority() must have a body") + at;
        let mut depth = 0usize;
        let mut end = open;
        for (i, c) in src[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + i;
                        break;
                    }
                }
                _ => {}
            }
        }
        let body = &src[open..end];

        let mut named: Vec<&str> = Vec::new();
        for (i, _) in body.match_indices("InputEvent::") {
            let rest = &body[i + "InputEvent::".len()..];
            let n = rest.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(rest.len());
            let name = &rest[..n];
            if !name.is_empty() && !named.contains(&name) {
                named.push(name);
            }
        }
        named.sort_unstable();
        assert_eq!(
            named.len(),
            KNOWN_NON_CLIENT_VARIANTS,
            "`InputEvent::authority()` now names a different number of variants than \
             {KNOWN_NON_CLIENT_VARIANTS}: {named:?}\n\
             A variant with a non-Client authority needs a VERDICT in `server_event_is_checked` — \
             either the downstream check that verifies it, or a reject. The default is reject, so \
             leaving it out is safe but silently breaks the feature. Decide, then update this count.",
        );
    }
}
