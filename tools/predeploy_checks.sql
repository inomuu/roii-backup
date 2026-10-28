-- READ-ONLY checks to run against PRODUCTION before deploying the 0.0.491 server (master after
-- 66d8c875). Nothing here writes. Run in the Neon SQL editor (prod branch) or
--   psql "$DATABASE_URL" -f tools/predeploy_checks.sql
-- Column names are the OLD build's (prod is still 66d8c875: `granted_tradeable`, `tradeable`).

-- 1. Orders the old build left 'init' / 'pending' for over an hour WITH a Steam transid. The new
--    sweeps only finish 'charged' / 'charging' orders, so any of these that Steam actually charged
--    must be granted by hand (/admin/store/grant with force, after checking the partner site).
SELECT order_id, account_id, sku, status, transid, created_at
  FROM store_orders
 WHERE status IN ('init', 'pending')
   AND transid IS NOT NULL AND transid <> ''
   AND created_at < now() - interval '1 hour'
 ORDER BY created_at;

-- 2. Orders charged but never granted (the old sweep should have finished them).
SELECT order_id, account_id, sku, status, created_at
  FROM store_orders
 WHERE status IN ('charged', 'grant_failed')
 ORDER BY created_at;

-- (No ban check: the prod build has no account_bans table yet, so no account is banned.)

-- 4. Sizes that decide the first boot's migration time (trusted prune, commander stage seed parse
--    every save before the port binds; Fly's health check grace is 5 s).
SELECT (SELECT count(*) FROM accounts)          AS accounts,
       (SELECT count(*) FROM saves)             AS saves,
       (SELECT count(*) FROM trusted)           AS trusted_rows,
       (SELECT pg_size_pretty(sum(length(state_json))::bigint) FROM saves) AS save_bytes,
       (SELECT count(*) FROM cheat_flags)       AS cheat_flags,
       (SELECT count(*) FROM mailbox WHERE claimed_at IS NULL) AS unclaimed_mail;

-- 5. Crystal wallets already negative (refund debt) — expected 0 before this deploy.
SELECT account_id, tradeable, bound FROM crystal_wallet WHERE tradeable < 0 OR bound < 0;

-- 6. Weekly rows for the current and previous week ids (the new week numbering continues from the
--    old ids; deploy Friday-Sunday so no mid-week board reset is visible).
SELECT week, count(*) FROM weekly_progress GROUP BY week ORDER BY week DESC LIMIT 3;
