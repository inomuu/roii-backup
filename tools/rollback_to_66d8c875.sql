-- ROLLBACK the verify-server database to what build 66d8c875 (sim 0.0.489) reads.
--
-- The newer build renames three crystal columns at boot (`tradeable` → `paid`, 2026-10-01) and
-- the old binary never renames them back: booted on a migrated database it serves, but every
-- crystal and store path fails on the missing column. Run this BEFORE booting the old binary
-- (`psql "$DATABASE_URL" -f tools/rollback_to_66d8c875.sql`), never while the new one is up.
--
-- It also DROPS `commander.outpost_claimed_at_ms`, which the old Commander endpoints read (they
-- 500 without it) — restored below at 0, so the first Outpost claim after rollback pays nothing.
-- Not reversible, and lost on rollback: the seven market tables the new build drops (the old
-- binary recreates them empty) and the `gacha_coupon` rows it deletes (old binary shows 0).
-- Everything else the newer build added is additive (new tables / columns the old binary does not
-- read) and is left in place, so rolling forward again needs nothing. Two things the old binary
-- will NOT honour while it runs:
--   - mailbox.voided_at: mail withdrawn by a refund shows in the inbox again and can be claimed;
--   - store_orders.status 'charging': such an order is invisible to its sweeps (settle by hand).
-- Each statement is a no-op when the column already has the old name.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = current_schema()
               AND table_name = 'crystal_wallet' AND column_name = 'paid') THEN
        ALTER TABLE crystal_wallet RENAME COLUMN paid TO tradeable;
    END IF;
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = current_schema()
               AND table_name = 'crystal_ledger' AND column_name = 'd_paid') THEN
        ALTER TABLE crystal_ledger RENAME COLUMN d_paid TO d_tradeable;
    END IF;
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = current_schema()
               AND table_name = 'store_orders' AND column_name = 'granted_paid') THEN
        ALTER TABLE store_orders RENAME COLUMN granted_paid TO granted_tradeable;
    END IF;
END $$;
ALTER TABLE commander ADD COLUMN IF NOT EXISTS outpost_claimed_at_ms BIGINT NOT NULL DEFAULT 0;
