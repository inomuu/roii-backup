-- Full wipe of the verify-server database (user decision 2026-09-19: the class system is gone
-- in 0.0.445 and old saves are NOT migrated — "ล้าง db เก่าหมดได้เลย").
--
-- Run in the Neon SQL editor against the PROD branch (or `psql "$DATABASE_URL" -f tools/wipe_prod_db.sql`),
-- BEFORE `tools/deploy.sh` — the server loads every save into memory at boot, so wipe first,
-- then deploy, and the new build starts empty. Schema (tables, migrations, schema_flags) is kept;
-- only rows go.
--
-- EVERY table in the schema except `schema_flags` and `seasons` (content, not player data — a
-- wipe that took it left the Battle Pass off until a season was re-created), found at run time
-- (audit 2026-10-02). The old
-- hand-written list fell behind the server: tables keyed by account_id but outside it (bans,
-- monthly card, shop buys, pity, Steam mints, …) survived, and RESTART IDENTITY hands account #N
-- to a new player — who then inherited the old #N's ban, card and pending mints. A listed table
-- the server no longer creates also made the whole TRUNCATE fail.
DO $$
DECLARE
    tables TEXT;
BEGIN
    SELECT string_agg(format('%I', tablename), ', ')
      INTO tables
      FROM pg_tables
     WHERE schemaname = current_schema()
       AND tablename NOT IN ('schema_flags', 'seasons');
    IF tables IS NOT NULL THEN
        EXECUTE 'TRUNCATE ' || tables || ' RESTART IDENTITY CASCADE';
    END IF;
END $$;
