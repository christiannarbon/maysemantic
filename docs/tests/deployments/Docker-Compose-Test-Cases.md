# Docker Compose Test Cases

This document outlines the manual test cases required to validate the stability, security, and functionality of the May Semantic Layer's Docker Compose deployment.

## Test Case 1: Secure Credential Enforcement

**Objective:** Verify that the system refuses to start with default or missing credentials.

**Steps:**
1. Ensure you are in the `deployments/docker-compose/` directory.
2. Delete any existing `.env` file (`rm .env`).
3. Run `docker compose up -d`.

**Expected Result:**
- Docker Compose should fail or print warnings about missing variables. 
- The `may-identity-db` container should fail to initialize properly due to missing `POSTGRES_PASSWORD` (or Docker will refuse to create it if variables are completely unset).

## Test Case 2: Successful Stack Initialization

**Objective:** Verify that the full stack boots up successfully when proper credentials are provided.

**Steps:**
1. Copy the example configuration: `cp .env.example .env`.
2. Edit `.env` and set `IDENTITY_DB_PASSWORD=SecureDbPass123` and `MAY_ADMIN_PASSWORD=SecureAdminPass123`.
3. Run `docker compose up -d`.
4. Monitor the status using `docker compose ps`.

**Expected Result:**
- The containers `may-identity-db`, `may-identity-migrate`, and `may_pgwire` are created.
- `may-identity-db` reaches a `(healthy)` state.

## Test Case 3: Migration Execution

**Objective:** Verify that the migration container runs against the database and applies all schemas successfully.

**Steps:**
1. View the logs for the migration container:
   ```bash
   docker compose logs may-identity-migrate
   ```

**Expected Result:**
- The logs show the `sqlx` CLI successfully applying migrations (e.g. `Applied 1/migrate create users table`).
- The container exits with code `0`.

## Test Case 4: Admin Seeding & PGWire Connectivity

**Objective:** Verify that `may_pgwire` correctly connects to the database, and the admin user has been seeded.

**Steps:**
1. View the logs for the PGWire container:
   ```bash
   docker compose logs may_pgwire
   ```
2. Attempt to connect to the PGWire server locally using `psql` (requires `psql` installed locally):
   ```bash
   psql -h 127.0.0.1 -p 5432 -U admin -d may_identity
   ```
   *(When prompted, use the password you set for `MAY_ADMIN_PASSWORD`)*

**Expected Result:**
- The `may_pgwire` container logs show successful startup.
- The `psql` connection succeeds, proving the admin user was securely seeded and PGWire is routing traffic correctly.

## Test Case 5: Cleanup & Data Persistence

**Objective:** Verify that data persists across container restarts unless volumes are explicitly removed.

**Steps:**
1. Stop the stack: `docker compose down`.
2. Start the stack again: `docker compose up -d`.
3. Check the logs for the migration container again.

**Expected Result:**
- The migration container logs indicate that migrations were previously applied (no new schemas run).
- Admin authentication via PGWire still works, proving the Postgres volume correctly persisted data.
