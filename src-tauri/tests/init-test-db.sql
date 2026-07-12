-- IT-001: Test database initialization script.
--
-- This script runs once when the test PostgreSQL container is first created.
-- It creates the test database and user. The actual schema migrations are
-- run by the Rust test suite (via crate::db::run_migrations) — NOT by this
-- script — so the tests always test the current migration code.

-- The POSTGRES_DB env var already creates the 'hms_test' database.
-- This script is a placeholder for any additional test-specific setup.

-- Enable the pgcrypto extension (may be needed for gen_random_uuid if used
-- by future migrations).
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Log that the init script ran.
DO $$
BEGIN
    RAISE NOTICE 'IT-001: Test database initialized. Migrations will be run by cargo test.';
END
$$;
