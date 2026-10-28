//! Mailbox persistence — the Postgres adapter for `mailbox.rs`.
//!
//! WHY THIS IS A SEPARATE MODULE. `mailbox.rs` is a closed core: no HTTP, no Postgres, so its
//! fuzz harness can prove claim-once and value conservation over random operation sequences. That
//! property is worth keeping, so the rules stay there and only the STORAGE moves here — the model
//! defines what is legal, this file makes Postgres do the same thing, and `pg_mailbox_*` checks
//! that it does.
//!
//! WHY MAIL LEFT THE BLOB (`docs/gacha-arch-review.md` P0). Mail used to persist inside the
//! `market_kv` JSON snapshot with the market ledgers, which put three costs on the one path that
//! takes real money: every gacha mint serialized through the process-wide `market_persist_lock`,
//! a paid delivery inherited the market's `market_poisoned` failure state, and appending one mail
//! rewrote the entire structure. As rows, a mint is an INSERT in the same transaction that debits
//! the crystal — no lock, no coupling, no rewrite.
//!
//! Ids come from `BIGSERIAL`, which keeps the monotonic-counter contract the model documents
//! (ids are never reused, so a consumed id can never be handed back out). They stay far below
//! 2^53, so they cross the FFI as JSON numbers exactly as they did before.

use crate::mailbox::{Claim, MailError, MailItem, MailKind, Mailbox};
use sqlx::{PgExecutor, Row};

/// One row → the model's `MailItem`.
fn row_to_item(r: &sqlx::postgres::PgRow) -> MailItem {
    let item_json: Option<String> = r.get("item_json");
    MailItem {
        id: r.get::<i64, _>("id") as u64,
        account_id: r.get::<i64, _>("account_id") as u64,
        kind: MailKind::from_str(r.get::<&str, _>("kind")),
        gil: r.get::<i64, _>("gil") as u64,
        def_id: r.get("def_id"),
        qty: r.get::<i64, _>("qty") as u64,
        item: item_json.and_then(|j| serde_json::from_str(&j).ok()),
        reason: r.get("reason"),
    }
}

const COLS: &str = "id, account_id, kind, gil, def_id, qty, item_json, reason";

/// Deliver a Gil/stackable mail. Mirrors `Mailbox::send`, including its empty-payload rejection —
/// an empty mail is the model's one invalid send and must not become a row.
pub async fn send<'e, E: PgExecutor<'e>>(
    db: E,
    account_id: u64,
    kind: MailKind,
    gil: u64,
    def_id: &str,
    qty: u64,
    reason: &str,
) -> Result<u64, MailError> {
    if gil == 0 && qty == 0 {
        return Err(MailError::EmptyPayload);
    }
    let row = sqlx::query(
        "INSERT INTO mailbox (account_id, kind, gil, def_id, qty, reason) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(account_id as i64)
    .bind(kind.as_str())
    .bind(gil as i64)
    .bind(def_id)
    .bind(qty as i64)
    .bind(reason)
    .fetch_one(db)
    .await
    .map_err(|_| MailError::NotFound)?;
    Ok(row.get::<i64, _>("id") as u64)
}

/// Deliver a FULL item instance (gacha, battle pass, compensation).
pub async fn send_item<'e, E: PgExecutor<'e>>(
    db: E,
    account_id: u64,
    kind: MailKind,
    item: &sim_core::items::Item,
    reason: &str,
) -> Result<u64, sqlx::Error> {
    let row = sqlx::query(
        "INSERT INTO mailbox (account_id, kind, item_json, reason) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(account_id as i64)
    .bind(kind.as_str())
    // REFUSE, do not swallow. This path used to take a serialization failure and write an EMPTY
    // payload instead.
    // Downstream is unguarded — the TEXT column accepts it, `row_to_item` reads back
    // `item: None, gil: 0, qty: 0`, and `claim` returns Ok — so the player is told their unique
    // item came home and nothing arrives. Unreachable today (`Item` is derive-only across its
    // whole graph, checked 2026-08-18), which is why this is a one-line symmetry fix and not an
    // incident: two standards for the same failure is what the next reader gets wrong.
    .bind(match serde_json::to_string(item) {
        Ok(j) => j,
        Err(e) => {
            return Err(sqlx::Error::Protocol(format!(
                "mail for acct {account_id}: the item would not serialize ({e}) — refusing to send \
                 empty mail in place of the only path home"
            )))
        }
    })
    .bind(reason)
    .fetch_one(db)
    .await?;
    Ok(row.get::<i64, _>("id") as u64)
}

/// An account's UNCLAIMED mail, id order (the model's `inbox_for`).
pub async fn inbox_for<'e, E: PgExecutor<'e>>(db: E, account_id: u64) -> Result<Vec<MailItem>, sqlx::Error> {
    // A VOIDED row (a refunded pack's mail) is withdrawn from the inbox but kept, so a claim the
    // client already took into its sim still settles (`snapshot_for_claims` reads it).
    let sql = format!("SELECT {COLS} FROM mailbox WHERE account_id = $1 AND claimed_at IS NULL AND voided_at IS NULL ORDER BY id");
    Ok(sqlx::query(&sql)
        .bind(account_id as i64)
        .fetch_all(db)
        .await?
        .iter()
        .map(row_to_item)
        .collect())
}

/// Claim one mail and hand back its payload, exactly once.
///
/// Claim-once is enforced by the UPDATE's own predicate rather than by reading first and writing
/// after: `WHERE claimed_at IS NULL` means two concurrent claims of the same mail leave exactly
/// one with `rows_affected() == 1`. A check-then-act would let both through.
pub async fn claim(
    tx: &mut sqlx::PgConnection,
    account_id: u64,
    mail_id: u64,
    now_ms: i64,
) -> Result<Claim, MailError> {
    let sql = format!(
        "UPDATE mailbox SET claimed_at = $1 WHERE id = $2 AND claimed_at IS NULL AND voided_at IS NULL RETURNING {COLS}"
    );
    let row = sqlx::query(&sql)
        .bind(now_ms)
        .bind(mail_id as i64)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| MailError::NotFound)?;
    let Some(row) = row else {
        // Nothing updated: either it never existed or it is already claimed. Distinguish the two
        // so the caller returns 404 vs 409 the way the in-memory model did.
        let exists: Option<(Option<i64>, Option<i64>)> = sqlx::query_as("SELECT claimed_at, voided_at FROM mailbox WHERE id = $1")
            .bind(mail_id as i64)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| MailError::NotFound)?;
        return Err(match exists {
            None => MailError::NotFound,
            Some((None, Some(_))) => MailError::Withdrawn,
            Some(_) => MailError::AlreadyClaimed,
        });
    };
    let m = row_to_item(&row);
    if m.account_id != account_id {
        // Ownership is checked AFTER the update, so undo it — a stranger's failed claim must not
        // consume the mail. Returning Err rolls the caller's transaction back, which is what
        // makes this safe; the explicit error keeps the status mapping honest.
        return Err(MailError::NotOwner);
    }
    Ok(Claim { gil: m.gil, def_id: m.def_id, qty: m.qty, item: m.item })
}

/// Mark a mail claimed after a verified save consumed it (the model's `consume_claim`).
///
/// NOT idempotent, deliberately. The `claimed_at IS NULL` predicate has always made a second
/// consume a no-op, but the no-op used to be SILENT — the `rows_affected` this returns was
/// discarded, so a settlement that took one mail twice committed without a complaint. Reading it
/// turns the predicate from a guard nobody checked into the last line of defence: `RowNotFound`
/// propagates out of `consume_all`, the caller's transaction rolls back, and the whole save is
/// rejected rather than half-applied.
///
/// This must never fire in normal play — `reconcile_claims` rejects a duplicated mail-id before
/// anything reaches here. It fires only if a future path settles a claim without reconciling, and
/// that is exactly when a silent no-op would be a mint.
pub async fn consume<'e, E: PgExecutor<'e>>(db: E, mail_id: u64, now_ms: i64) -> Result<(), sqlx::Error> {
    let done = sqlx::query("UPDATE mailbox SET claimed_at = $1 WHERE id = $2 AND claimed_at IS NULL")
        .bind(now_ms)
        .bind(mail_id as i64)
        .execute(db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(sqlx::Error::RowNotFound);
    }
    Ok(())
}

/// Mark every mail a verified save claimed, inside the caller's transaction. Any id that is not
/// an unclaimed row fails the whole transaction (see `consume`).
pub async fn consume_all(tx: &mut sqlx::PgConnection, mail_ids: &[u64], now_ms: i64) -> Result<(), sqlx::Error> {
    for id in mail_ids {
        consume(&mut *tx, *id, now_ms).await?;
    }
    Ok(())
}

/// Load JUST the mails a save's `Claim` ops name, as a `Mailbox` the PURE `mailbox::settle_claims`
/// can verify against: it takes `&Mailbox`, and it does not care that this one holds four rows
/// instead of an entire inbox.
///
/// Both halves matter — an unclaimed row proves the claim is live, and a CLAIMED row is what
/// turns a replayed claim into `AlreadyClaimed` (a forgery) instead of `NotFound`.
pub async fn snapshot_for_claims<'e, E: PgExecutor<'e>>(
    db: E,
    account: u64,
    ops: &[sim_core::state::MarketOp],
) -> Result<Mailbox, sqlx::Error> {
    let ids: Vec<i64> = ops
        .iter()
        .filter_map(|op| match op {
            sim_core::state::MarketOp::Claim { mail_id, .. } => Some(*mail_id as i64),
            _ => None,
        })
        .collect();
    if ids.is_empty() {
        return Ok(Mailbox::default());
    }
    let sql = format!("SELECT {COLS}, claimed_at, voided_at FROM mailbox WHERE id = ANY($1)");
    let rows = sqlx::query(&sql).bind(&ids).fetch_all(db).await?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
    let mut withdrawn = std::collections::BTreeMap::new();
    let mut unclaimed = Vec::new();
    let mut claimed = std::collections::BTreeSet::new();
    let mut claimed_foreign = std::collections::BTreeSet::new();
    for r in &rows {
        if r.get::<Option<i64>, _>("claimed_at").is_some() {
            let id = r.get::<i64, _>("id") as u64;
            if r.get::<i64, _>("account_id") as u64 == account {
                claimed.insert(id);
            } else {
                claimed_foreign.insert(id);
            }
        } else {
            let m = row_to_item(r);
            // A refund's void, past the grace window, on this account's own mail.
            if let Some(v) = r.get::<Option<i64>, _>("voided_at").filter(|v| m.account_id == account && now - v > crate::mailbox::WITHDRAWN_GRACE_MS) {
                withdrawn.insert(m.id, v);
            }
            unclaimed.push(m);
        }
    }
    Ok(Mailbox::snapshot(unclaimed, claimed, claimed_foreign).with_withdrawn(withdrawn))
}
