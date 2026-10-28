//! Mailbox — a GENERAL server→player delivery primitive. ONE system delivers compensation
//! (downtime/bug/rollback), rewards, refunds, gifts — distinguished by `MailKind` (provenance).
//! It was built for the Gil market's proceeds/returns (docs/l1-commodity-market-scope.md); that
//! market was removed in 0.0.490, and `SaleProceeds` / `Purchase` / `Return` remain as kinds so
//! stored rows still read.
//!
//! This is the ISOLATED core (scope Slice 2a): NO HTTP, NO Postgres — a closed model so the
//! harness can prove the invariants. Slice 2b wires a claimed payload into the verified
//! `GameState` (via a server-gated `ClaimMail` InputEvent); Slice 3 persists to Neon.
//!
//! It is a server-authored MINT channel (value enters the economy on `send`), so the guards
//! are: claim-once (idempotent — a replayed claim is a no-op, never a double-grant), ownership
//! (only the recipient claims), and provenance (`kind` + `reason` recorded for the mint audit,
//! economy-invariants I6). Authority (a client can't forge a claim) is enforced at the HTTP/
//! verify layer, not here.
//!
//! Determinism: mail ids from a monotonic counter (never a clock); BTreeMap/BTreeSet iteration.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

/// Where a mail came from — the provenance tag for the mint audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MailKind {
    SaleProceeds,
    Purchase,
    Return,
    Compensation,
    Reward,
    Refund,
    Gift,
}

impl MailKind {
    /// Stable wire/column name. Written out by hand rather than derived from `Debug` so that
    /// renaming a variant is a compile error here instead of silently orphaning stored rows.
    pub fn as_str(self) -> &'static str {
        match self {
            MailKind::SaleProceeds => "SaleProceeds",
            MailKind::Purchase => "Purchase",
            MailKind::Return => "Return",
            MailKind::Compensation => "Compensation",
            MailKind::Reward => "Reward",
            MailKind::Refund => "Refund",
            MailKind::Gift => "Gift",
        }
    }

    /// Inverse of [`as_str`]. An unknown string means a row written by a newer build, so it
    /// degrades to `Compensation` (provenance is an audit tag; losing it must not lose the mail).
    pub fn from_str(s: &str) -> MailKind {
        match s {
            "SaleProceeds" => MailKind::SaleProceeds,
            "Purchase" => MailKind::Purchase,
            "Return" => MailKind::Return,
            "Reward" => MailKind::Reward,
            "Refund" => MailKind::Refund,
            "Gift" => MailKind::Gift,
            _ => MailKind::Compensation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailError {
    NotFound,
    NotOwner,
    /// The mail was already claimed — by THIS account (a foreign claimed mail is `NotOwner`). An
    /// honest client can present one after a stale inbox view, so it rejects the save but is not,
    /// on its own, a forgery.
    AlreadyClaimed,
    /// The same mail claimed twice in ONE bundle. An honest client cannot author this
    /// (`claim_mail` removes the mail from the sim's own inbox), so it is a forgery.
    DuplicateInSave,
    EmptyPayload,
    /// The claimed payload doesn't match the real mail (a forged ClaimMail).
    PayloadMismatch,
    /// The mail was withdrawn by a refund longer ago than `WITHDRAWN_GRACE_MS`. Refused, not a
    /// forgery on its own: the claim was authored while the mail was real.
    Withdrawn,
}

/// How long after a refund voids a mail a save may still settle a claim of it (audit 2026-10-03).
/// An honest client saves within minutes of a claim (the sim stalls unsaved at ~5 min); a claim
/// arriving later was held back until the refund cleared.
pub const WITHDRAWN_GRACE_MS: i64 = 30 * 60 * 1000;

/// One piece of mail. Carries a Gil payload and/or an item payload (`def_id` + `qty`);
/// at least one must be non-zero (an empty mail is rejected at `send`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MailItem {
    pub id: u64,
    pub account_id: u64,
    pub kind: MailKind,
    pub gil: u64,
    pub def_id: String,
    pub qty: u64,
    /// A FULL item instance (gacha, battle pass) — delivered intact. None for gil/stackable
    /// mail. Skip-if-none keeps stackable mail byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<sim_core::items::Item>,
    /// Audit: why this was sent (free text) — part of the mint provenance record.
    pub reason: String,
}

/// The payload handed to the caller (the sim, in Slice 2b) when a mail is claimed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Claim {
    pub gil: u64,
    pub def_id: String,
    pub qty: u64,
    pub item: Option<sim_core::items::Item>,
}

/// The mailbox: unclaimed mail by id + the set of already-claimed ids (for idempotency).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Mailbox {
    inbox: BTreeMap<u64, MailItem>,
    /// Ids already claimed — a replayed claim of one of these is a no-op (AlreadyClaimed),
    /// never a second grant. Mirrors refine_consumed.
    claimed: BTreeSet<u64>,
    /// Claimed ids that belong to ANOTHER account (snapshots only): claiming one is `NotOwner`,
    /// never the benign `AlreadyClaimed`.
    #[serde(default)]
    claimed_foreign: BTreeSet<u64>,
    /// This account's mail voided by a refund past the grace window (snapshots only).
    #[serde(default)]
    withdrawn: BTreeMap<u64, i64>,
    next_id: u64,
}

impl Mailbox {
    /// Build a read-only view over rows already fetched from the store, so the PURE reconcile
    /// below can verify a save's claims without any of this module learning about Postgres.
    ///
    /// The store loads only the mails a save actually names, so this is a few rows, not an
    /// inbox — and `next_id` is left at 0 because a snapshot is never sent to.
    pub fn snapshot(unclaimed: Vec<MailItem>, claimed: BTreeSet<u64>, claimed_foreign: BTreeSet<u64>) -> Mailbox {
        Mailbox {
            inbox: unclaimed.into_iter().map(|m| (m.id, m)).collect(),
            claimed,
            claimed_foreign,
            withdrawn: BTreeMap::new(),
            next_id: 0,
        }
    }

    /// Lift the grace limit for claims the server already accepted into a stored save (a tainted
    /// save stores its claims unsettled): refusing them now would refuse every later save too.
    /// Unconditionally: every stored save passed the withdrawn check when it was stored (a tainted
    /// one included), so a claim inside one was accepted in time. Re-judging it by the time of a
    /// LATER save locked an honest account out of saving for good (audit 2026-10-03).
    pub fn forgive_withdrawn(mut self, mail_ids: &BTreeSet<u64>) -> Mailbox {
        self.withdrawn.retain(|id, _| !mail_ids.contains(id));
        self
    }

    /// Mark mails as withdrawn past the grace window (id → `voided_at` ms): claiming one is
    /// `Withdrawn`.
    pub fn with_withdrawn(mut self, ids: BTreeMap<u64, i64>) -> Mailbox {
        self.withdrawn = ids;
        self
    }

    /// Mail ids of `ops`' claims that are withdrawn — checked even on a save that settles nothing.
    pub fn withdrawn_claims(&self, ops: &[sim_core::state::MarketOp]) -> Vec<u64> {
        ops.iter()
            .filter_map(|op| match op {
                sim_core::state::MarketOp::Claim { mail_id, .. } if self.withdrawn.contains_key(mail_id) => Some(*mail_id),
                _ => None,
            })
            .collect()
    }

    /// Deliver a mail to `account_id` (a MINT — value enters here). At least one of Gil/qty
    /// must be non-zero. Returns the new mail id.
    pub fn send(
        &mut self,
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
        let id = self.next_id;
        self.next_id += 1;
        self.inbox.insert(
            id,
            MailItem {
                id,
                account_id,
                kind,
                gil,
                def_id: def_id.to_string(),
                qty,
                item: None,
                reason: reason.to_string(),
            },
        );
        Ok(id)
    }

    /// An account's unclaimed mail, id order.
    pub fn inbox_for(&self, account_id: u64) -> Vec<MailItem> {
        self.inbox.values().filter(|m| m.account_id == account_id).cloned().collect()
    }

    /// Claim a mail: hand back its payload ONCE, then mark it claimed. Idempotent — a second
    /// claim of the same id returns `AlreadyClaimed` and mutates NOTHING (no double-grant).
    /// ATOMIC: ownership/existence checked before any mutation.
    pub fn claim(&mut self, account_id: u64, mail_id: u64) -> Result<Claim, MailError> {
        match self.inbox.entry(mail_id) {
            Entry::Occupied(e) if e.get().account_id != account_id => Err(MailError::NotOwner),
            Entry::Occupied(e) => {
                let m = e.remove();
                self.claimed.insert(mail_id);
                Ok(Claim { gil: m.gil, def_id: m.def_id, qty: m.qty, item: m.item })
            }
            Entry::Vacant(_) if self.claimed.contains(&mail_id) => Err(MailError::AlreadyClaimed),
            Entry::Vacant(_) => Err(MailError::NotFound),
        }
    }

    // --- reconcile (Slice 2b): verify a claim recorded in a verified save, then consume it ---

    /// DRY verification (no mutation) that a `ClaimMail` recorded in a verified save is legitimate:
    /// the mail exists, is owned by `account`, is unclaimed, AND the payload the sim applied matches
    /// the real mail. A forged ClaimMail (real id, lied payload) passes save-verify but fails HERE.
    /// `AlreadyClaimed` = a re-claim in a NEW save (the sim double-applied) → also a forgery.
    pub fn verify_claim(&self, account: u64, mail_id: u64, gil: u64, def_id: &str, qty: u32, item: Option<&sim_core::items::Item>) -> Result<(), MailError> {
        match self.inbox.get(&mail_id) {
            Some(m) if m.account_id != account => Err(MailError::NotOwner),
            Some(m) if m.gil != gil || m.def_id != def_id || m.qty != qty as u64 || m.item.as_ref() != item => Err(MailError::PayloadMismatch),
            Some(_) if self.withdrawn.contains_key(&mail_id) => Err(MailError::Withdrawn),
            Some(_) => Ok(()),
            None if self.claimed_foreign.contains(&mail_id) => Err(MailError::NotOwner),
            None if self.claimed.contains(&mail_id) => Err(MailError::AlreadyClaimed),
            None => Err(MailError::NotFound),
        }
    }

    /// Consume a verified claim (mutating): remove from the inbox + mark claimed. Call ONLY after
    /// `verify_claim` returned Ok AND the whole save is accepted (settlement invariant / atomic).
    pub fn consume_claim(&mut self, mail_id: u64) {
        if self.inbox.remove(&mail_id).is_some() {
            self.claimed.insert(mail_id);
        }
    }

    // --- invariant probes (harness + a future runtime self-check) ---

    /// Σ Gil sitting UNCLAIMED in the inbox.
    pub fn inbox_gil(&self) -> u64 {
        self.inbox.values().map(|m| m.gil).sum()
    }

    /// Σ units of `def_id` sitting UNCLAIMED in the inbox.
    pub fn inbox_units(&self, def_id: &str) -> u64 {
        self.inbox.values().filter(|m| m.def_id == def_id).map(|m| m.qty).sum()
    }

    /// A claimed id can never also be in the inbox (a mail is unclaimed XOR claimed).
    pub fn claimed_and_inbox_disjoint(&self) -> bool {
        self.inbox.keys().all(|id| !self.claimed.contains(id))
    }
}

/// The dry-run reconcile of a verified save's `Claim` ops against the mailbox: which NEW claims
/// (op-id ∉ `consumed_ops`) are legitimate to consume, and which are FORGERIES (reject + flag).
pub struct ClaimRecon {
    /// (op_id, mail_id) pairs to consume — but ONLY if `forged` is empty and the save is accepted.
    pub consumable: Vec<(u64, u64)>,
    /// (mail_id, reason) for each forged/invalid claim → the caller rejects the save + flags.
    pub forged: Vec<(u64, MailError)>,
}

/// Verify (DRY, no mutation) the `MarketOp::Claim`s in a verified save. Idempotent by op-id: a
/// claim whose op-id is already in `consumed_ops` is skipped (a retried/growing-log re-save). New
/// claims are checked with `verify_claim`; any failure is a forgery. The caller commits the
/// `consumable` list (mailbox.consume_claim + record op-ids) ONLY when `forged` is empty AND the
/// rest of the save verifies (atomic — a rejected save consumes nothing).
///
/// TWO independent dedup keys, because they answer different questions and neither implies the
/// other. `consumed_ops` is the OP-id set from earlier saves — it makes a growing input log
/// replayable. `seen` is the MAIL-id set for THIS batch, and it is what stops one mail from being
/// minted N times inside a single bundle: op-ids come from a client-side counter
/// (`mailbox_grant::claim_mail` takes a fresh one per call), so N claims on one mail carry N
/// distinct op-ids and every one of them used to pass. Nothing downstream caught it either —
/// `verify_claim` takes `&self` and the snapshot is frozen for the whole batch, so all N saw the
/// same unclaimed mail.
pub fn reconcile_claims(
    mb: &Mailbox,
    consumed_ops: &std::collections::HashSet<u64>,
    account: u64,
    ops: &[sim_core::state::MarketOp],
) -> ClaimRecon {
    use sim_core::state::MarketOp;
    let mut consumable = Vec::new();
    let mut forged = Vec::new();
    let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    for op in ops {
        if let MarketOp::Claim { id, mail_id, gil, def_id, qty, item } = op {
            if consumed_ops.contains(id) {
                continue; // already reconciled in a prior save → idempotent skip
            }
            if !seen.insert(*mail_id) {
                // A second claim on a mail this batch already took. An honest client cannot
                // author this — `claim_mail` removes the mail from the sim's own inbox — so it is
                // a forgery, not a duplicate to skip: the save is rejected and the account flagged.
                forged.push((*mail_id, MailError::DuplicateInSave));
                continue;
            }
            match mb.verify_claim(account, *mail_id, gil.get(), def_id, *qty, item.as_ref()) {
                Ok(()) => consumable.push((*id, *mail_id)),
                Err(e) => forged.push((*mail_id, e)),
            }
        }
    }
    ClaimRecon { consumable, forged }
}

/// Is every rejected claim merely a mail THIS account already claimed? Then the save is refused
/// (it would pay twice) but not flagged: a stale inbox view produces exactly this, and it gains
/// nothing. Any other reason — someone else's mail, a lied payload, an unknown id, a duplicate
/// inside one bundle — is a forgery.
pub fn only_stale_claims(forged: &[(u64, MailError)]) -> bool {
    !forged.is_empty() && forged.iter().all(|(_, e)| matches!(e, MailError::AlreadyClaimed | MailError::Withdrawn))
}

/// What a verified save's `market_ops` settle. Since the Gil market was removed (0.0.490) that is
/// mail claims only.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ClaimSettlement {
    /// Mail ids to mark claimed, in log order. Each one passed `verify_claim`.
    pub consume: Vec<u64>,
    /// Op ids to record in `consumed_market_ops` and return as `settled_ops` (the client prunes
    /// them). Every NEW op in log order: the verified claims plus any `retired` op.
    pub consumed_ops: Vec<u64>,
    /// NEW ops of a removed trade kind (Deposit / Buy / DepositItem / BuyItem) found in an old
    /// state. Marked consumed so the client prunes them; never settled. A subset of `consumed_ops`.
    pub retired: Vec<u64>,
    /// `op_digest` of each `consumed_ops` entry, same order — stored with the mark so a later save
    /// re-presenting that op id with a DIFFERENT op (a rewind to an older anchor) is caught.
    pub digests: Vec<String>,
}

/// The identity of one op: an honest re-save re-presents the same claim; a rewound state that
/// reuses the op id for a forged claim does not. Built from the claim's VALUE fields only — mail id,
/// Gil, def and quantity, and an item's def and quantity — never the op's serialized form (audit
/// 2026-10-03: a new or renamed `Item` field would have changed every honest digest and flagged
/// every account that ever claimed an item mail).
pub fn op_digest(op: &sim_core::state::MarketOp) -> String {
    use sim_core::state::MarketOp;
    let key = match op {
        MarketOp::Claim { mail_id, gil, def_id, qty, item, .. } => format!(
            "claim|{mail_id}|{}|{def_id}|{qty}|{}|{}",
            gil.get(),
            item.as_ref().map_or("", |i| i.def_id.as_str()),
            item.as_ref().map_or(0, |i| i.quantity),
        ),
        other => format!("op|{}", other.id()),
    };
    blake3::hash(key.as_bytes()).to_hex().to_string()
}

/// DECIDE the settlement of a verified save's ops (pure; the caller writes it). Any forged claim
/// rejects the WHOLE save: `Err` carries `(mail_id, why)` and nothing is consumed. Ops whose id is
/// in `consumed_ops` were settled by an earlier save and are skipped (growing-log idempotency).
pub fn settle_claims(
    mb: &Mailbox,
    consumed_ops: &std::collections::HashSet<u64>,
    account: u64,
    ops: &[sim_core::state::MarketOp],
) -> Result<ClaimSettlement, Vec<(u64, MailError)>> {
    use sim_core::state::MarketOp;
    let recon = reconcile_claims(mb, consumed_ops, account, ops);
    if !recon.forged.is_empty() {
        return Err(recon.forged);
    }
    // With nothing forged, every NEW claim op is in `recon.consumable` (a duplicate mail id would
    // have been forged), so the walk below marks exactly those claims plus the retired ops.
    let mut s = ClaimSettlement {
        consume: recon.consumable.iter().map(|&(_, mail_id)| mail_id).collect(),
        ..ClaimSettlement::default()
    };
    for op in ops {
        let id = op.id();
        if consumed_ops.contains(&id) {
            continue;
        }
        match op {
            MarketOp::Claim { .. } => {}
            MarketOp::Deposit { .. } | MarketOp::Buy { .. } | MarketOp::DepositItem { .. } | MarketOp::BuyItem { .. } => {
                s.retired.push(id)
            }
        }
        s.consumed_ops.push(id);
        s.digests.push(op_digest(op));
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::rng::Pcg32;
    use sim_core::state::MarketOp;
    use std::collections::HashSet;

    fn claim_op(id: u64, mail_id: u64, gil: u64) -> MarketOp {
        MarketOp::Claim { id, mail_id, gil: sim_core::state::Gil::new(gil), def_id: String::new(), qty: 0, item: None }
    }

    #[test]
    fn settle_claims_consumes_verified_claims_and_retires_trade_ops_unsettled() {
        let mut mb = Mailbox::default();
        let a = mb.send(1, MailKind::Reward, 76, "", 0, "r").unwrap();
        let b = mb.send(1, MailKind::Reward, 0, "red_herb", 3, "r").unwrap();
        let ops = vec![
            MarketOp::Deposit { id: 1, def_id: "herb".into(), qty: 2, unit_price: 5 },
            claim_op(2, a, 76),
            MarketOp::BuyItem { id: 3, listing_id: 9, max_price: sim_core::state::Gil::new(10) },
            MarketOp::Claim { id: 4, mail_id: b, gil: sim_core::state::Gil::new(0), def_id: "red_herb".into(), qty: 3, item: None },
            claim_op(5, a, 76), // already settled by an earlier save → skipped, not a duplicate
        ];
        let consumed: HashSet<u64> = HashSet::from([5]);
        let s = settle_claims(&mb, &consumed, 1, &ops).expect("nothing forged");
        assert_eq!(s.consume, vec![a, b], "the two verified claims, log order");
        assert_eq!(s.consumed_ops, vec![1, 2, 3, 4], "every NEW op is marked so the client prunes it");
        assert_eq!(s.retired, vec![1, 3], "trade ops are pruned, never settled");
        // A re-save presenting the same ops after they were recorded settles nothing.
        let all: HashSet<u64> = HashSet::from([1, 2, 3, 4, 5]);
        assert_eq!(settle_claims(&mb, &all, 1, &ops), Ok(ClaimSettlement::default()));
    }

    #[test]
    fn only_a_mail_you_already_claimed_is_stale_everything_else_is_forged() {
        use MailError::*;
        assert!(only_stale_claims(&[(1, AlreadyClaimed), (2, AlreadyClaimed)]));
        assert!(!only_stale_claims(&[]), "nothing rejected is not a stale claim");
        for bad in [NotOwner, NotFound, PayloadMismatch, DuplicateInSave, EmptyPayload] {
            let what = format!("{bad:?}");
            assert!(!only_stale_claims(&[(1, AlreadyClaimed), (2, bad)]), "{what} still flags");
        }
    }

    #[test]
    fn settle_claims_rejects_the_whole_save_on_one_forged_claim() {
        let mut mb = Mailbox::default();
        let a = mb.send(1, MailKind::Reward, 76, "", 0, "r").unwrap();
        let b = mb.send(1, MailKind::Reward, 10, "", 0, "r").unwrap();
        let ops = vec![claim_op(1, a, 76), claim_op(2, b, 999)];
        assert_eq!(settle_claims(&mb, &HashSet::new(), 1, &ops), Err(vec![(b, MailError::PayloadMismatch)]));
        // Another account's mail, and the same mail twice in one bundle, are forgeries too.
        assert_eq!(settle_claims(&mb, &HashSet::new(), 2, &[claim_op(1, a, 76)]), Err(vec![(a, MailError::NotOwner)]));
        assert_eq!(
            settle_claims(&mb, &HashSet::new(), 1, &[claim_op(1, a, 76), claim_op(2, a, 76)]),
            Err(vec![(a, MailError::DuplicateInSave)])
        );
    }

    /// A refund-withdrawn mail past the grace window is refused unflagged — unless the claim was
    /// already in a stored save, which is forgiven so the account can keep saving.
    #[test]
    fn a_withdrawn_mail_is_refused_unless_its_claim_was_already_stored() {
        let mut mb = Mailbox::default();
        let a = mb.send(1, MailKind::Reward, 76, "", 0, "r").unwrap();
        let mb = mb.with_withdrawn(BTreeMap::from([(a, 1_000)]));
        let ops = [claim_op(1, a, 76)];
        let r = settle_claims(&mb, &HashSet::new(), 1, &ops);
        assert_eq!(r, Err(vec![(a, MailError::Withdrawn)]));
        assert!(only_stale_claims(&r.unwrap_err()), "refused, not flagged");
        assert_eq!(mb.withdrawn_claims(&ops), vec![a], "seen on a save that settles nothing too");
        let mb = mb.forgive_withdrawn(&BTreeSet::from([a]));
        assert!(settle_claims(&mb, &HashSet::new(), 1, &ops).is_ok(), "a claim already stored settles");
    }

    #[test]
    fn reconcile_accepts_a_matching_claim_and_consumes_once() {
        let mut mb = Mailbox::default();
        let mid = mb.send(1, MailKind::SaleProceeds, 76, "", 0, "sold").unwrap();
        let ops = vec![MarketOp::Claim { id: 10, mail_id: mid, gil: sim_core::state::Gil::new(76), def_id: String::new(), qty: 0, item: None }];
        let mut consumed: HashSet<u64> = HashSet::new();
        // First save: the claim is legit → consumable, no forgery.
        let r = reconcile_claims(&mb, &consumed, 1, &ops);
        assert_eq!(r.consumable, vec![(10, mid)]);
        assert!(r.forged.is_empty());
        // Commit: consume + record the op-id.
        mb.consume_claim(mid);
        consumed.insert(10);
        // A growing-log re-save with the SAME op → idempotent skip (not re-consumed, not forged).
        let r2 = reconcile_claims(&mb, &consumed, 1, &ops);
        assert!(r2.consumable.is_empty() && r2.forged.is_empty(), "op-id already consumed → skip");
    }

    /// ONE mail, MANY claims, ONE bundle — the double-mint this dedup exists for.
    ///
    /// Op-ids come from a client-side counter (`mailbox_grant::claim_mail` takes a fresh one per
    /// call), so N claims on one mail carry N DISTINCT op-ids and the `consumed_ops` check waves
    /// every one of them through. `verify_claim` cannot catch it either: it takes `&self` and the
    /// snapshot is frozen for the batch, so all N see the same unclaimed mail.
    ///
    /// MEASURED, not asserted as a boolean: what matters is how much Gil the settlement authorises
    /// against how much the mail holds. A test that only checked `forged.is_empty() == false`
    /// would pass with 999 of 1000 claims still consumable.
    #[test]
    fn one_mail_claimed_a_thousand_times_in_one_bundle_pays_exactly_once() {
        let mut mb = Mailbox::default();
        let mid = mb.send(1, MailKind::SaleProceeds, 76, "", 0, "sold").unwrap();
        // The same mail, 1000 times, each with its own op-id — exactly what a modified client
        // emits by calling claim on one mail in a loop before saving.
        let ops: Vec<MarketOp> = (0..1000)
            .map(|i| MarketOp::Claim { id: 100 + i, mail_id: mid, gil: sim_core::state::Gil::new(76), def_id: String::new(), qty: 0, item: None })
            .collect();
        let r = reconcile_claims(&mb, &HashSet::new(), 1, &ops);

        let authorised: u64 = r.consumable.len() as u64 * 76;
        assert_eq!(
            authorised, 76,
            "the batch authorises {authorised} Gil against a 76 Gil mail — {} claims survived",
            r.consumable.len()
        );
        assert_eq!(r.consumable, vec![(100, mid)], "the FIRST claim is the legitimate one");
        assert_eq!(r.forged.len(), 999, "every later claim on the same mail is a forgery");
        assert!(
            r.forged.iter().all(|(m, e)| *m == mid && *e == MailError::DuplicateInSave),
            "a duplicate must be reported as DuplicateInSave so the save is rejected + flagged"
        );
        assert!(!only_stale_claims(&r.forged), "a duplicate inside one bundle is never a stale claim");
    }

    /// The dedup must not eat the ordinary case: several DIFFERENT mails claimed in one save is
    /// what happens every time a player empties their mailbox, and all of them must pay.
    #[test]
    fn many_different_mails_in_one_bundle_all_still_pay() {
        let mut mb = Mailbox::default();
        let ids: Vec<u64> = (0..5).map(|i| mb.send(1, MailKind::SaleProceeds, 10 + i, "", 0, "sold").unwrap()).collect();
        let ops: Vec<MarketOp> = ids
            .iter()
            .enumerate()
            .map(|(i, m)| MarketOp::Claim { id: 200 + i as u64, mail_id: *m, gil: sim_core::state::Gil::new(10 + i as u64), def_id: String::new(), qty: 0, item: None })
            .collect();
        let r = reconcile_claims(&mb, &HashSet::new(), 1, &ops);
        assert!(r.forged.is_empty(), "distinct mails are not duplicates: {:?}", r.forged);
        assert_eq!(r.consumable.len(), 5);
    }

    #[test]
    fn reconcile_flags_forged_payload_and_phantom_and_reclaim() {
        let mut mb = Mailbox::default();
        let mid = mb.send(1, MailKind::SaleProceeds, 76, "", 0, "sold").unwrap();
        let consumed: HashSet<u64> = HashSet::new();
        // Forged payload: real mail id, lied 999999 gil (this PASSES save-verify → must fail here).
        let forged_pay = vec![MarketOp::Claim { id: 1, mail_id: mid, gil: sim_core::state::Gil::new(999_999), def_id: String::new(), qty: 0, item: None }];
        assert_eq!(reconcile_claims(&mb, &consumed, 1, &forged_pay).forged, vec![(mid, MailError::PayloadMismatch)]);
        // Wrong owner claiming it.
        let wrong_owner = vec![MarketOp::Claim { id: 2, mail_id: mid, gil: sim_core::state::Gil::new(76), def_id: String::new(), qty: 0, item: None }];
        assert_eq!(reconcile_claims(&mb, &consumed, 2, &wrong_owner).forged, vec![(mid, MailError::NotOwner)]);
        // Phantom mail id.
        let phantom = vec![MarketOp::Claim { id: 3, mail_id: 999, gil: sim_core::state::Gil::new(5), def_id: String::new(), qty: 0, item: None }];
        assert_eq!(reconcile_claims(&mb, &consumed, 1, &phantom).forged, vec![(999, MailError::NotFound)]);
        // Re-claim in a new save (op-id 4, mail already consumed) → double-apply → forgery.
        mb.consume_claim(mid);
        let reclaim = vec![MarketOp::Claim { id: 4, mail_id: mid, gil: sim_core::state::Gil::new(76), def_id: String::new(), qty: 0, item: None }];
        assert_eq!(reconcile_claims(&mb, &consumed, 1, &reclaim).forged, vec![(mid, MailError::AlreadyClaimed)]);
    }

    #[test]
    fn claim_hands_payload_once_then_is_a_noop() {
        let mut mb = Mailbox::default();
        let id = mb.send(1, MailKind::SaleProceeds, 76, "", 0, "sold 4 herb").unwrap();
        assert_eq!(mb.inbox_for(1).len(), 1);
        let c = mb.claim(1, id).unwrap();
        assert_eq!(c, Claim { gil: 76, def_id: "".into(), qty: 0, item: None });
        assert!(mb.inbox_for(1).is_empty(), "claimed → out of inbox");
        // Idempotent: a replayed claim is a no-op, NOT a second 76 gil.
        assert_eq!(mb.claim(1, id), Err(MailError::AlreadyClaimed));
    }

    #[test]
    fn only_the_recipient_can_claim() {
        let mut mb = Mailbox::default();
        let id = mb.send(1, MailKind::Return, 0, "red_herb", 6, "cancelled").unwrap();
        let before = mb.clone();
        assert_eq!(mb.claim(2, id), Err(MailError::NotOwner));
        assert_eq!(mb, before, "a rejected claim mutates nothing");
        let c = mb.claim(1, id).unwrap();
        assert_eq!(c, Claim { gil: 0, def_id: "red_herb".into(), qty: 6, item: None });
    }

    #[test]
    fn empty_and_unknown_mail_are_rejected() {
        let mut mb = Mailbox::default();
        assert_eq!(mb.send(1, MailKind::Reward, 0, "", 0, "oops"), Err(MailError::EmptyPayload));
        assert_eq!(mb.claim(1, 999), Err(MailError::NotFound));
    }

    /// Conservation + claim-once + atomicity over 20 seeds × 2000 random ops. Value delivered
    /// (accumulated from claim returns) + value still in the inbox must equal value ever sent —
    /// per Gil and per def. A double-grant or a phantom claim would break it.
    #[test]
    fn fuzz_claim_once_and_provenance_conservation() {
        let defs = ["red_herb", "ore"];
        let n_accounts: u64 = 4;
        for seed in 0..20u64 {
            let mut rng = Pcg32::new(seed);
            let mut mb = Mailbox::default();
            let kinds = [
                MailKind::SaleProceeds,
                MailKind::Return,
                MailKind::Compensation,
                MailKind::Reward,
                MailKind::Refund,
                MailKind::Gift,
            ];
            let mut sent_gil: u64 = 0;
            let mut sent_units: [u64; 2] = [0, 0];
            let mut delivered_gil: u64 = 0;
            let mut delivered_units: [u64; 2] = [0, 0];
            for _ in 0..2000 {
                let before = mb.clone();
                let acct = rng.gen_range_u32(0, n_accounts as u32) as u64;
                if rng.gen_range_u32(0, 2) == 0 {
                    // SEND (mint): random gil and/or units.
                    let kind = kinds[rng.gen_range_u32(0, kinds.len() as u32) as usize];
                    let gil = rng.gen_range_u32(0, 50) as u64;
                    let di = rng.gen_range_u32(0, defs.len() as u32) as usize;
                    let qty = rng.gen_range_u32(0, 20) as u64;
                    match mb.send(acct, kind, gil, defs[di], qty, "fuzz") {
                        Ok(_) => {
                            sent_gil += gil;
                            sent_units[di] += qty;
                        }
                        Err(_) => assert_eq!(mb, before, "rejected send mutates nothing"),
                    }
                } else {
                    // CLAIM a random (maybe wrong / already-claimed) mail.
                    let ids: Vec<u64> = mb.inbox_for(acct).iter().map(|m| m.id).collect();
                    // Bias toward valid claims but also try junk ids.
                    let mail_id = if !ids.is_empty() && rng.gen_range_u32(0, 4) != 0 {
                        ids[rng.gen_range_u32(0, ids.len() as u32) as usize]
                    } else {
                        rng.gen_range_u32(0, 30) as u64
                    };
                    match mb.claim(acct, mail_id) {
                        Ok(c) => {
                            delivered_gil += c.gil;
                            if let Some(i) = defs.iter().position(|d| *d == c.def_id) {
                                delivered_units[i] += c.qty;
                            }
                        }
                        Err(_) => assert_eq!(mb, before, "rejected claim mutates nothing"),
                    }
                }
                // Conservation: delivered + still-in-inbox == ever-sent (per currency).
                assert_eq!(delivered_gil + mb.inbox_gil(), sent_gil, "gil conserved");
                for (i, d) in defs.iter().enumerate() {
                    assert_eq!(delivered_units[i] + mb.inbox_units(d), sent_units[i], "units[{d}] conserved");
                }
                assert!(mb.claimed_and_inbox_disjoint(), "a mail is unclaimed XOR claimed");
            }
        }
    }
}
