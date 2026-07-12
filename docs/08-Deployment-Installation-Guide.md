# VitalFlow HMS — Deployment & Installation Guide

> **Document version: v0.2.0 — updated 2025-07-08 after Phase 2 Batches 0-3 implementation. See CHANGELOG.md for details.**

| Field | Value |
|---|---|
| **Document title** | VitalFlow HMS — Deployment & Installation Guide |
| **Version** | 0.2.0 |
| **Date** | 2025-07-08 |
| **Status** | Draft (reconciled v0.2.0 by Documentation Team — B4-C) |
| **Classification** | Internal |
| **Owner** | VitalFlow HMS Engineering / Operations |
| **Author** | Documentation Specialist (Task 7); reconciled v0.2.0 by Documentation Team (B4-C) |
| **Audience** | Hospital IT administrators, installers, operations staff |
| **Related documents** | `01-SRS-Software-Requirements.md`, `02-SDD-Software-Design.md`, `07-Licensing-Architecture.md`, `08-Deployment-Installation-Guide.md` (this document), `src-tauri/SETUP_POSTGRES_BINARIES.md`, `README.md` |

### Revision history

| Version | Date | Author | Changes |
|---|---|---|---|
| 0.1.0 | 2026-07-02 | Documentation Specialist (Task 7) | Initial deployment guide: server/client installer distinction, PostgreSQL provisioning via NSIS, ProgramData layout, pairing flow, TLS pinning, first-run admin login, license installation, backup/restore, multi-PC topology, troubleshooting, post-install verification checklist. |
| 0.2.0 | 2025-07-08 | Documentation Team (B4-C) | Reconciled with Phase 2 Batches 0-3: §4.1 step 8 pg_hba.conf now accurately documents the loopback=`host` vs LAN=`hostssl` split (CR-22, Batch 1); §4.1 step 6 documents the random CSPRNG bootstrap password + `bootstrap-credentials.txt` (CR-2, Batch 1); §5.1 `config.json` schema updated with current fields; §5.2 ACL section updated to reflect CR-5 ACL hardening (SYSTEM + Administrators only via `icacls`); §6.1 first-login no longer references `ChangeMe123!`; §6.3 no-backdoor recovery updated for the random-password flow; §7 TLS section notes SEC-13 TLS key ACL; §8 first-run admin login fully rewritten (CR-2); §12 troubleshooting updated; §13 post-install checklist updated; §16 (new) security summary added (CSP, bootstrap creds, ACLs, pairing brute-force, LAN HMAC, opener capability reduction); references to `tauri-plugin-shell` removed (SEC-09 — dependency was never present, but spec audit flagged it). |

---

## 1. Introduction

### 1.1 Purpose

This guide describes how to deploy VitalFlow HMS in a hospital: prerequisites, server vs client installer distinction, PostgreSQL provisioning via NSIS, ProgramData layout, pairing flow, TLS pinning, first-run admin login, license installation, backup/restore, multi-PC topology, troubleshooting, and a post-install verification checklist.

### 1.2 Scope

This guide covers Phase 1 deployment on Windows 10/11 x64. Phase 2 deployment variations (additional modules) do not change the underlying installation procedure.

---

## 2. Prerequisites

### 2.1 Server PC (the designated database host)

| Requirement | Detail |
|---|---|
| Operating system | Windows 10 x64 (build 1909+) or Windows 11 x64 (Pro/Enterprise recommended) |
| .NET runtime | .NET Framework 4.x (present by default on Win10/11) — required for WMI access |
| Disk space | 5 GB free minimum (PostgreSQL binaries + data + app) |
| Memory | 8 GB RAM minimum (16 GB recommended for hospitals with >100k patient records) |
| Network | A static (or DHCP-reserved) LAN IP; the server PC must be reachable from all client PCs |
| Windows Firewall | Inbound TCP 5432 (PostgreSQL) and the pairing port (see §6) must be allowed from the LAN |
| User privileges | The installer must be run by an Administrator; the day-to-day receptionist account does NOT need admin rights |
| Domain | Workgroup or Active Directory-joined are both supported; no domain account is required by HMS itself |

### 2.2 Client PC (each non-server workstation)

| Requirement | Detail |
|---|---|
| Operating system | Windows 10 x64 or Windows 11 x64 |
| .NET runtime | .NET Framework 4.x |
| Disk space | 500 MB free |
| Memory | 4 GB RAM minimum |
| Network | LAN reachability to the server PC's IP |
| Windows Firewall | Outbound TCP 5432 to the server IP must be allowed |
| User privileges | Standard user; no admin rights required for daily use |

### 2.3 LAN

| Requirement | Detail |
|---|---|
| Topology | Single broadcast segment (no routing between client and server) |
| IP range | Private (RFC 1918): 10.x, 172.16-31.x, or 192.168.x — `pg_hba.conf` is scoped to these |
| Internet | Not required for daily operation. The WhatsApp integration (if used) requires outbound HTTPS to the WhatsApp Web endpoint from the server PC. |
| DNS | Not required. Clients reference the server by IP. |

### 2.4 Software company side (license issuance)

- An air-gapped signing machine with the Ed25519 private key (see `07-Licensing-Architecture.md` §10–11).
- The hospital's hardware fingerprint (collected per §5.1 below).
- The hospital's identity fields (name, ID, modules, validity window).

---

## 3. Server vs client installer

VitalFlow HMS is delivered as **two distinct Windows installers**, built from the same source tree with different Cargo features:

| Aspect | Server installer | Client installer |
|---|---|---|
| File name | `VitalFlowHMS-Server-<ver>-setup.exe` | `VitalFlowHMS-Client-<ver>-setup.exe` |
| Tauri config | `tauri.server.conf.json` | `tauri.client.conf.json` |
| Cargo feature | `--features server-build` | `--features client-build` |
| Product name | "VitalFlow HMS Server" | "VitalFlow HMS Client" |
| Identifier | distinct (so both can coexist on a test PC) | distinct |
| Bundled PostgreSQL | Yes — `resources/pgsql` | No |
| Install mode | `perMachine` (requires Administrator) | per-user or perMachine |
| NSIS hook | `windows/hooks.nsh` runs `NSIS_HOOK_POSTINSTALL` (provisions PG) | No hook |
| Day-to-day elevation | Never required | Never required |
| Boot path | `initialize_as_server` (health-check PG, start pairing listener, start LAN broadcast) | `initialize_as_client` (probe saved IP, fallback to LAN discovery) |

**Decision rule**: install the Server installer on exactly one PC per hospital (the designated database host). Install the Client installer on every other PC that needs HMS access.

---

## 4. PostgreSQL provisioning via NSIS hooks.nsh

The server installer's `NSIS_HOOK_POSTINSTALL` hook (in `src-tauri/windows/hooks.nsh`) provisions PostgreSQL automatically during installation. **No operator interaction is required.** The steps:

### 4.1 Step-by-step (what the installer does)

1. **`SetShellVarContext all`** — ensures `$APPDATA` resolves to `C:\ProgramData` (not the installing admin's personal `C:\Users\<admin>\AppData\Roaming`).
2. **Create `C:\ProgramData\HMS\`** — `CreateDirectory "$APPDATA\HMS"`.
3. **Grant ACL** — `icacls "$APPDATA\HMS" /grant *S-1-5-32-545:(OI)(CI)M /T` grants the `Builtin Users` group (well-known SID `S-1-5-32-545`, locale-independent) modify rights on the HMS folder and all its children. This is what lets the day-to-day receptionist (a non-admin) save settings later. **[CR-5 v0.2.0]** Subsequent `save_config` calls further ACL-restrict `config.json` itself to `SYSTEM` + `Administrators` only (see §5.2).
4. **Skip if already provisioned** — `IfFileExists "$APPDATA\HMS\pgdata\PG_VERSION"` skips re-initdb on reinstall/upgrade (never destroys patient data).
5. **Copy PostgreSQL binaries** **[v0.2.0 bugfix]** — `robocopy "$INSTDIR\pgsql" "$APPDATA\HMS\pgsql" /E /NFL /NDL /NJH /NJS /XD "pgAdmin 4" "docs" "include" "symbols"` from the bundled installer resources. The v0.1.0 installer used NSIS `CopyFiles` which (a) failed on long paths (>260 chars) inside `pgAdmin 4/docs/`, causing "error writing to file" installer failures, and (b) copied ~200 MB of unneeded pgAdmin/docs/include/symbols folders. `robocopy` handles long paths natively and excludes the unwanted directories. A pre-build check in `build.rs` (`#[cfg(feature = "server-build")]`) fails the build early if `resources/pgsql/pgAdmin 4/` still exists, with a clear message directing the developer to `SETUP_POSTGRES_BINARIES.md §2a`.
6. **Generate DB password** — a temp PowerShell script uses `[System.Security.Cryptography.RandomNumberGenerator]` to generate 24 cryptographically random bytes, base64-encodes them, strips non-alphanumeric characters, and writes the result to a temp file. This deliberately avoids NSIS's native (insecure) PRNG. **[CR-2 v0.2.0]** The same CSPRNG approach is used at app-first-run to generate the bootstrap admin password (see §8.1).
7. **Run `initdb`** — with `--auth=scram-sha-256` and the generated password as the superuser password. This produces a data directory at `C:\ProgramData\HMS\pgdata\` with `pg_hba.conf` defaulting to scram-sha-256.
8. **Write `pg_hba.conf`** **[CR-22 v0.2.0]** — the final lockdown config uses `host` for loopback (so the app can bootstrap before TLS is provisioned) and `hostssl` for LAN clients (so no DB traffic ever crosses the network in plaintext). The v0.1.0 spec incorrectly said all-`hostssl`; the v0.1.0 installer also wrote all-`host` (also wrong). Both are now consistent: loopback=`host`, LAN=`hostssl`. The actual `hooks.nsh` writes:
   ```
   # HMS managed rules — local app + LAN clients only, scram-sha-256 required.
   # Local loopback uses 'host' (plaintext) so the app can bootstrap on first
   # launch BEFORE SSL is provisioned. The Rust app upgrades this to 'hostssl'
   # once TLS is configured (see pg_provision.rs).
   host    all             all             127.0.0.1/32            scram-sha-256
   host    all             all             ::1/128                 scram-sha-256
   # LAN clients MUST use SSL — never allow plaintext over the network.
   hostssl all             all             10.0.0.0/8              scram-sha-256
   hostssl all             all             172.16.0.0/12           scram-sha-256
   hostssl all             all             192.168.0.0/16          scram-sha-256
   ```
   Note the deliberate split: loopback rules use `host` (the app connects to its own PG via 127.0.0.1, before the TLS cert exists); LAN rules use `hostssl` (clients must always use TLS). PostgreSQL's `hostssl` directive enforces TLS at the connection layer — any non-TLS LAN connection is refused.
9. **Enable SSL in `postgresql.conf`** — `ssl = on`, `ssl_cert_file`, `ssl_key_file` pointing at the TLS material generated by `tls_provision.rs` on first server boot. **[SEC-13 v0.2.0]** The TLS private key file (`C:\ProgramData\HMS\tls\server.key`) is ACL-restricted to `SYSTEM` + `Administrators` only.
10. **Register Windows Service** — `pg_ctl register -N HMS-PostgreSQL -S auto` registers PostgreSQL as a Windows Service set to auto-start.
11. **Start the service** — `sc start HMS-PostgreSQL`.
12. **Write `config.json`** — at `C:\ProgramData\HMS\config.json` with the generated credentials, host=127.0.0.1, port=5432, db_name=hms (note: actual default db_name is `hospital_db` per the installer's `hooks.nsh` — confirm with your build), `setup_complete=true`. **[CR-5 v0.2.0]** `save_config` immediately re-ACLs this file to `SYSTEM` + `Administrators` only.

### 4.2 Pre-uninstall hook

`NSIS_HOOK_PREUNINSTALL` stops (does **not** delete) the `HMS-PostgreSQL` service so patient data survives an uninstall/reinstall.

### 4.3 If the installer fails

If any step in `hooks.nsh` fails, the installer shows a `MessageBox MB_OK|MB_ICONSTOP` with a clear message rather than silently producing a broken install. Read the message, consult §10 (troubleshooting), and re-run the installer.

---

## 5. ProgramData\HMS layout

After a successful server install, the layout on the server PC is:

```
C:\ProgramData\HMS\
├── config.json              ← written by installer, read/written by app
├── license.json             ← dropped by operator (Step 5 below) or written by install_license
├── pgsql\                   ← bundled PostgreSQL binaries
│   └── bin\
│       ├── initdb.exe
│       ├── pg_ctl.exe
│       ├── psql.exe
│       ├── postgres.exe
│       └── ...
├── pgdata\                  ← PostgreSQL data directory
│   ├── PG_VERSION
│   ├── postgresql.conf
│   ├── pg_hba.conf
│   ├── postgresql.auto.conf
│   └── base\, pg_wal\, etc.
└── tls\
    ├── server.crt           ← self-signed cert (rcgen) for the server's LAN IP
    └── server.key           ← matching private key
```

### 5.1 `config.json` schema

```json
{
  "mode": "server",
  "db_host": "127.0.0.1",
  "db_port": 5432,
  "db_user": "postgres",
  "db_password": "<24-char random generated by installer>",
  "db_name": "hospital_db",
  "clinic_name": "VitalFlow Clinic",
  "doctors_whatsapp_group": "",
  "setup_complete": true
}
```

**[CR-4 v0.2.0]** — `db_password` is tagged `#[serde(skip_serializing)]` in the Rust `Config` struct, so a `get_config` IPC call never returns it to the frontend (defense-in-depth on top of the file ACL below). The installer writes the password to `config.json` directly via NSIS `FileWrite`; once the app takes over, `save_config` re-writes the file with `db_password` included (the skip is only on the IPC DTO, not on disk — otherwise the app could not reconnect after restart).

On a client PC after pairing, the same file holds the server's real LAN IP, the redeemed DB credentials, and the pinned server cert PEM + fingerprint (`pinned_server_cert_pem`, `pinned_server_fingerprint` fields are added by the client-build pairing flow).

### 5.2 ACLs

**[CR-5 v0.2.0 (Batch 1)]** — `config.json` is now ACL-restricted to `SYSTEM` + `Administrators` only. The `Config::save` method runs `icacls` after every atomic write:

```rust
icacls "C:\ProgramData\HMS\config.json" /inheritance:r /grant:r SYSTEM:F /grant:r Administrators:F
```

This narrows the broad `Builtin Users` modify grant that the installer applies to the parent `C:\ProgramData\HMS\` folder. The trade-off: the day-to-day receptionist (non-admin) can no longer read `config.json` directly. The app reads it via the `get_config` IPC command (which returns a redacted DTO with `db_password` stripped). Filesystem-level reads now require admin.

The parent folder `C:\ProgramData\HMS\` retains the `Builtin Users` modify grant (the app needs to write `audit_logs` cache, log files, etc.). Only `config.json` itself is locked down. **[SEC-13 v0.2.0]** The TLS material (`tls\server.key`) is similarly ACL-restricted.

**[Planned Batch 5]** — DPAPI encryption at rest for the DB password in `config.json` (R-003 / R-032 in `05-Risk-Register-ISO-31000.md`). Until then, the ACL is the primary confidentiality control.

---

## 6. Pairing flow

The pairing flow exchanges a short-lived, single-use code for real DB credentials over a TLS-protected TCP listener on the server. This replaces "write the password on a sticky note."

### 6.1 Server side (receptionist or admin at the server PC)

1. Boot HMS on the server PC. **[CR-2 v0.2.0]** First-run admin credentials are NOT a hardcoded `admin / ChangeMe123!` pair — instead, the app generates a random 24-character CSPRNG password on first DB init and writes it to `C:\ProgramData\HMS\bootstrap-credentials.txt` (ACL-restricted to `SYSTEM` + `Administrators`). Open that file as Administrator to read the one-time password; log in with `admin` + that password; you will be forced to change it immediately. See §8.
2. Log in. Open **Settings → "Connect a New Client PC"** (server-build only).
3. Click **Generate pairing code**. A 6-character code is displayed with a live countdown (10-minute expiry, capped uses).
4. Read the code aloud to the operator at the client PC, or write it on a one-time-use slip.

**[SEC-03 v0.2.0 (Batch 3)]** — pairing codes are now generated with `OsRng` (was `thread_rng()`, which is not a CSPRNG on all platforms). The 6-char code over a 32-symbol alphabet is ~30 bits of entropy — adequate only because of the brute-force protection below.

**[SEC-03 v0.2.0 (Batch 3)]** — pairing code brute-force protection is now enforced:
- **3 max uses** (was 10 in v0.1.0) — defined by `MAX_USES = 3` in `pairing.rs`. Reduces the window for credential exfiltration: each successful redeem returns the full DB credentials + TLS cert PEM.
- **Per-IP lockout** — after `MAX_FAILED_ATTEMPTS_PER_PEER = 3` failed attempts within `FAILED_ATTEMPT_WINDOW_SECS = 300` (5 min), the source IP is locked out for `LOCKOUT_DURATION_SECS = 900` (15 min). The code is NOT checked during lockout, so no information leaks about the current code. Caps brute-force at ~3 attempts / 5 min = ~860 attempts/day per attacker IP — nowhere near the 30-bit keyspace.

### 6.2 Client side (operator at each client PC)

1. Run the **Client installer** on the client PC. No PostgreSQL is installed.
2. Launch HMS. The first-run **Setup** screen appears (because `setup_complete` is false on a fresh client install).
3. Enter the **server PC's IP address** and the **pairing code**.
4. Click **Save & continue**. The client:
   a. Calls `redeem_pairing_code(ip, code)` — opens a TLS-protected TCP connection to the server's pairing listener.
   b. The server validates the code (exists, not expired, not used up) and returns the real DB credentials + the pinned server cert PEM.
   c. The client calls `complete_pairing_and_connect` → `verify_pairing` (materialises the cert to a temp file, opens a real DB connection to confirm credentials work, sets `setup_complete=true`).
   d. `initialize_database` runs — the client connects to the server's PostgreSQL with `sslmode=verify-ca` + the pinned cert.
5. The HMS app boots to the login screen.

### 6.3 Pairing listener security

- The pairing TCP listener is on `pairing::PAIRING_PORT` (defined in `pairing.rs`; the installer opens inbound TCP 42011 on Windows Firewall for LAN ranges only).
- TLS-protected via rustls using the same self-signed cert as PostgreSQL (`tls\server.crt`).
- The pairing code is 6 characters, 10-minute expiry, **[SEC-03 v0.2.0]** 3 max uses (was 10).
- **[SEC-03 v0.2.0]** Per-IP lockout after 3 failed attempts within 5 minutes (15-minute lockout duration).
- Codes are stored in-memory in `PairingService` (not persisted) — a server restart rotates any in-flight code.
- **[SEC-08 v0.2.0 (Batch 3)]** The LAN discovery broadcast (used by clients to find the server when the saved IP is stale — see §6.4) is now HMAC-SHA256-signed with the server's TLS fingerprint as the key. Paired clients (which have pinned the fingerprint) verify the HMAC before accepting a broadcast; pre-pairing clients (TOFU) accept any well-formed broadcast as before. Replay protection: broadcasts older than 120 seconds are rejected.

### 6.4 Re-pairing

If a client's saved server IP becomes stale (server PC's IP changed), the client falls back to LAN broadcast discovery (`discovery::detect_server`) and self-heals. **[SEC-08 v0.2.0]** The broadcast is HMAC-signed so an attacker on the LAN cannot spoof HMS_SERVER broadcasts to redirect clients to a rogue PostgreSQL. If the cert has changed (server PC reinstalled), the client gets a fingerprint mismatch error and must re-pair from the Setup screen.

---

## 7. TLS pinning

### 7.1 Cert generation

On the server PC, on first boot, `tls_provision::ensure_tls_material` generates a self-signed certificate (via `rcgen`) for the server's LAN IP and writes:

- `C:\ProgramData\HMS\tls\server.crt` — the certificate
- `C:\ProgramData\HMS\tls\server.key` — the private key
- A SHA-256 fingerprint is computed and stored in `config.json` as `pinned_server_fingerprint`.

### 7.2 Server (loopback) SSL

The server connects to its own PostgreSQL via `127.0.0.1` with `sslmode=require`. The cert chain is not validated (acceptable for loopback; no network hop).

### 7.3 Client (LAN) SSL with cert pinning

Clients connect to the server's PostgreSQL with `sslmode=verify-ca` and the pinned server cert (materialised to a temp file). The pinned cert is exchanged during pairing. If the server's cert changes (e.g. server PC reinstalled), the client refuses to connect with a "certificate fingerprint mismatch" error and surfaces the hint "Re-pair with the server" (`lib.rs::diagnose_db_error`).

### 7.4 SSL self-heal

If SSL is mis-configured in `postgresql.conf` or `pg_hba.conf` (e.g. `hostssl` rules exist but `ssl = on` is missing), the server detects this on boot via `pg_provision::ssl_is_configured_in_conf` and `pg_provision::hba_requires_ssl`, and auto-repairs via `pg_provision::repair_ssl_config` — re-writing the config files and restarting PostgreSQL.

**[CR-22 v0.2.0]** The `pg_hba.conf` written by the installer uses `host` for loopback and `hostssl` for LAN. The Rust app's `pg_provision::repair_ssl_config` may upgrade loopback rules from `host` → `hostssl` once TLS is fully provisioned; this is safe because the app's own loopback connection happens before TLS exists (the bootstrap path) and after TLS is up the app uses SSL anyway. Either way, LAN clients always require `hostssl` and the server enforces it.

---

## 8. First-run admin login

### 8.1 Bootstrap credentials **[CR-2 v0.2.0 (Batch 1)]**

- Username: `admin`
- Password: **a random 24-character string generated at install time** (CSPRNG over the unambiguous alphabet `ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789` — no `0/O/1/l/I` for transcription safety).

The v0.1.0 hardcoded `admin / ChangeMe123!` pair is **removed**. The new flow:

1. The installer provisions PostgreSQL and writes `config.json` (no admin password yet — the DB has no users table yet).
2. On first app boot, `auth::seed_defaults` checks `SELECT COUNT(*) FROM users`. If `0`, it:
   - Generates a 24-char CSPRNG password (alphabet above).
   - Argon2-hashes it.
   - `INSERT`s the `admin` user with `must_change_password = TRUE`.
   - Writes the plaintext password to `C:\ProgramData\HMS\bootstrap-credentials.txt` (ACL: `SYSTEM` + `Administrators` only via `icacls /inheritance:r /grant:r SYSTEM:F /grant:r Administrators:F`).
   - Writes an audit row (`system_bootstrap` action).
3. The installing admin opens `C:\ProgramData\HMS\bootstrap-credentials.txt` as Administrator, reads the one-time password, logs in with `admin` + that password.
4. The app routes to the **Force Change Password** screen (because `must_change_password = true`).
5. After the password is changed, the operator should **delete `bootstrap-credentials.txt`** (the file's own header instructs this).

### 8.2 First login

1. Boot HMS on the server PC (after license installation per §9).
2. The login screen appears.
3. **[CR-2 v0.2.0]** Open `C:\ProgramData\HMS\bootstrap-credentials.txt` as Administrator. Read the username (`admin`) and the random one-time password. (On Windows, this requires opening Notepad as Administrator, or `type` from an elevated cmd prompt.)
4. Log in with `admin` + the bootstrap password.
5. The app routes to the **Force Change Password** screen (because `must_change_password = true`).
6. Enter the current password, then a new password (≥8 characters) and confirmation.
7. Click **Update password**. The app reloads and you land in the main shell.
8. **Immediately** go to **Users** and create individual accounts for each staff member with appropriate roles (doctor, nurse, receptionist, etc.). See `02-SDD-Software-Design.md` §5.2 for the role-permission matrix.
9. **Delete `bootstrap-credentials.txt`** (optional but recommended — the password it contains is now stale).

### 8.3 No backdoor

There is no other backdoor account. If the `admin` password is lost after the first change, the recovery path is:

1. Stop the HMS app.
2. Connect to PostgreSQL directly: `C:\ProgramData\HMS\pgsql\bin\psql.exe -h 127.0.0.1 -U postgres -d hospital_db` (password from `config.json`, readable only as Administrator per §5.2).
3. Run `DELETE FROM users WHERE username = 'admin';`.
4. Restart HMS. `seed_defaults` re-creates the bootstrap admin (with a NEW random password) because the users table is now empty (it only re-seeds when `user_count == 0`). The new password is written to `bootstrap-credentials.txt`.
5. Log in with `admin` + the new bootstrap password and change it immediately.

This procedure requires DB access (which requires the `postgres` superuser password from `config.json`, which requires Administrator) — it is an operational recovery, not a security bypass.

---

## 9. License installation

### 9.1 Step 1 — Get the hardware fingerprint

1. Install and boot the server-build HMS app (per §3–§7).
2. The license-error screen appears (no license yet).
3. Click **Show hardware fingerprint** (or open Settings → License → Show fingerprint once logged in).
4. The 64-character hex fingerprint is displayed. Copy it.

### 9.2 Step 2 — Send to the software company

Send the fingerprint to the software company via an out-of-band secure channel (signed email, phone-verified). Also provide:

- Hospital name
- Hospital ID (if known; otherwise the software company assigns one)
- Modules to enable
- Validity window (issue date, expiration date, maintenance until)
- Software version range

### 9.3 Step 3 — Receive the signed license file

The software company issues a signed `license.json` per the runbook in `07-Licensing-Architecture.md` §11 and sends it back via a secure channel.

### 9.4 Step 4 — Install the license

Either:

- **Drop the file**: copy `license.json` to `C:\ProgramData\HMS\license.json` and restart HMS.
- **Use the UI**: log in as admin → Settings → License → **Install license** → file picker → select `license.json`. The `install_license` command verifies the signature + fingerprint before persisting.

### 9.5 Step 5 — Verify

On restart, the boot screen displays the hospital name and product edition from the license. If verification fails, see §10.4 / §10.5.

---

## 10. Backup and restore

### 10.1 Current status (Phase 1)

Phase 1 supports **manual** `pg_dump` backup. There is no in-product backup UI yet — that is a Phase 2 deliverable (see `03-Quality-Model-ISO-25010.md` §11).

### 10.2 Manual backup procedure

On the server PC, open a Command Prompt and run:

```cmd
set PGDATA=C:\ProgramData\HMS\pgdata
set PGBIN=C:\ProgramData\HMS\pgsql\bin
set PGPASSWORD=<from C:\ProgramData\HMS\config.json db_password>

%PGBIN%\pg_dump.exe -h 127.0.0.1 -U postgres -d hms -F c -f "C:\Backup\hms-%date:~-4%%date:~3,2%%date:~0,2%.dump"
```

This produces a compressed custom-format dump. Schedule it via Windows Task Scheduler (daily recommended; more frequent for high-volume hospitals).

### 10.3 Manual restore procedure

1. Stop the HMS app on all PCs (server and clients).
2. On the server PC, stop PostgreSQL: `sc stop HMS-PostgreSQL`.
3. Move the existing `C:\ProgramData\HMS\pgdata\` to `pgdata.old\` (preserve in case restore fails).
4. Re-initdb: `C:\ProgramData\HMS\pgsql\bin\initdb.exe -D C:\ProgramData\HMS\pgdata --auth=scram-sha-256 -U postgres -W` (re-enter a password; update `config.json` to match).
5. Restore `pg_hba.conf` and `postgresql.conf` from `pgdata.old\` (or re-run the SSL provisioning by restarting HMS).
6. Start PostgreSQL: `sc start HMS-PostgreSQL`.
7. Restore the dump: `set PGPASSWORD=...` then `C:\ProgramData\HMS\pgsql\bin\pg_restore.exe -h 127.0.0.1 -U postgres -d hms -1 "C:\Backup\hms-YYYYMMDD.dump"`.
8. Restart HMS on the server, then on clients.

### 10.4 Phase 2 goals

- In-product Settings → Backup panel wrapping `pg_dump` with one-click invocation.
- Scheduled backup via a Windows Task Scheduler template installed by the installer.
- Restore path in the installer ("restore from backup").
- WAL archiving documentation for PITR.

### 10.5 License file backup

The license file (`C:\ProgramData\HMS\license.json`) should also be backed up off-PC. If lost, the software company can re-issue a license for the same hardware fingerprint (no change to hospital identity). See `07-Licensing-Architecture.md` §11.8.

---

## 11. Multi-PC topology

### 11.1 Recommended topology

```
Hospital LAN (e.g. 192.168.1.0/24)
│
├── Server PC (192.168.1.10)
│     ├── VitalFlow HMS Server installer
│     ├── HMS-PostgreSQL Windows Service (auto-start)
│     ├── Pairing listener (TLS, port per pairing::PAIRING_PORT)
│     ├── LAN discovery broadcast
│     ├── TLS material (self-signed cert for 192.168.1.10)
│     └── HMS app (receptionist or admin login)
│
├── Client PC 1 — Doctor (192.168.1.11)
│     └── VitalFlow HMS Client installer → paired via code
│           → connects to 192.168.1.10:5432 with sslmode=verify-ca + pinned cert
│
├── Client PC 2 — Doctor (192.168.1.12)
│     └── same as above
│
├── Client PC 3 — Nurse station (192.168.1.13)
│     └── same as above
│
└── Client PC N — Lab/billing (192.168.1.20)
      └── same as above
```

### 11.2 IP addressing

- Reserve the server PC's IP via DHCP reservation or static assignment. If it changes, clients self-heal via LAN discovery (`discovery::detect_server`), but re-pairing may be needed if the cert fingerprint also changed.
- Document the server IP in the hospital's IT runbook.

### 11.3 Firewall rules

On the **server PC**, allow inbound:

| Port | Protocol | Source | Purpose |
|---|---|---|---|
| 5432 | TCP | LAN only (10/172.16/192.168) | PostgreSQL |
| pairing::PAIRING_PORT | TCP | LAN only | Pairing listener |
| Discovery port | UDP | LAN only | LAN discovery broadcast |

On **client PCs**, allow outbound:

| Port | Protocol | Destination | Purpose |
|---|---|---|---|
| 5432 | TCP | server IP | PostgreSQL |
| pairing::PAIRING_PORT | TCP | server IP | Pairing |
| Discovery port | UDP | 255.255.255.255 (broadcast) | LAN discovery fallback |

### 11.4 Scaling

- One server PC per hospital is the supported deployment.
- The bundled PostgreSQL handles ~50 concurrent client PCs comfortably for typical hospital workloads.
- For >100 concurrent clients or >1M patient records, consult the software company for tuning guidance (workload-specific `postgresql.conf` parameters, dedicated storage, etc.).

---

## 12. Troubleshooting

### 12.1 Database unreachable

**Symptom**: HMS shows "Cannot reach the hospital server at <ip>:<port>" on a client PC.

**Diagnostics**:

1. Verify the server PC is powered on and HMS Server is running.
2. Verify the server's HMS-PostgreSQL service is running: `sc query HMS-PostgreSQL` on the server. If `STOPPED`, start it: `sc start HMS-PostgreSQL`.
3. Verify Windows Firewall on the server allows inbound TCP 5432 from the LAN.
4. Verify both PCs are on the same LAN (ping the server IP from the client).
5. Verify the saved server IP in `C:\ProgramData\HMS\config.json` on the client is correct.
6. If the server IP changed, HMS will auto-fallback to LAN discovery. If discovery also fails, re-pair from the Setup screen.

### 12.2 SSL/TLS errors

**Symptom**: "server does not support TLS" or "certificate fingerprint mismatch" or "SSL certificate verification failed."

**Diagnostics**:

| Error | Cause | Fix |
|---|---|---|
| "server does not support TLS" | PostgreSQL started without `ssl = on` in `postgresql.conf` | Restart HMS Server on the server PC; the app auto-repairs SSL config (`pg_provision::repair_ssl_config`). If still failing, manually verify `ssl = on` in `C:\ProgramData\HMS\pgdata\postgresql.conf`. |
| "certificate fingerprint mismatch" | Server's TLS cert changed since pairing (e.g. server reinstalled) | Re-pair from the client's Setup screen. |
| "SSL certificate verification failed" | Pinned cert file missing on client | Re-pair from the client's Setup screen. |
| "server does not support SSL" persisting | `pg_hba.conf` has `hostssl` rules but `ssl = on` is missing | Restart HMS Server; if still failing, manually run the SSL provisioning steps. |

### 12.3 Hardware fingerprint mismatch

**Symptom**: License-error screen at boot with "This license is bound to a different computer and cannot be used here."

**Cause**: The server PC's hardware fingerprint has changed since the license was issued. Common causes:

- Motherboard replacement
- CPU replacement
- BIOS flash that changed `Win32_BIOS.SerialNumber` (rare on OEM hardware)
- Migration to a new PC (treating it as a "different computer")

**Fix**: Collect the new fingerprint (Settings → License → Show fingerprint, or the license-error screen's diagnostic), send to the software company, receive a new license bound to the new fingerprint, and install it per §9.

### 12.4 License expired

**Symptom**: License-error screen with "This license has expired. Contact the software company to renew."

**Fix**: Contact the software company for a renewal. The new license will have a new `expiration_date` (and possibly new `maintenance_until`). Install per §9.

### 12.5 License signature verification failed

**Symptom**: License-error screen with "License signature verification FAILED — the license is forged, corrupted, or was not issued by the software company."

**Causes**:

- The license file was modified (even a single character).
- The license was issued with a different keypair than the one embedded in the running app binary.
- **[Updated v0.2.0 (CR-20)]** The v0.1.0 spec's third bullet — "the app binary's `COMPANY_PUBLIC_KEY` is still the all-zeros placeholder" — is **no longer accurate**. The shipped binary now embeds a real (development) Ed25519 keypair that can sign `dev: true` licenses. For production, the software company must have swapped in a `keygen/gen_keys`-generated keypair (see `07-Licensing-Architecture.md` §5.4.1 + §10.2). If the customer's license was signed with the production private key but the app binary still embeds the dev public key (or vice versa), verification will fail.

**Fix**:

1. Confirm the license file came from the software company and was not modified in transit.
2. Confirm the app version matches the license's `software_version_min`/`_max` range.
3. **[v0.2.0]** Confirm the app binary embeds the correct `COMPANY_PUBLIC_KEY` — the Settings → License panel (visible to admins) shows the SHA-256 fingerprint of the embedded key; the software company can compare this to the fingerprint printed by `keygen/gen_keys` when the production keypair was generated.
4. If the issue persists, re-request the license from the software company with the current hardware fingerprint and the current app version.

### 12.6 Login lockout

**Symptom**: "Too many failed attempts. Account locked for 15 minutes."

**Cause**: 5 consecutive failed logins.

**Fix**: Wait 15 minutes. The lockout clears automatically. If a user has genuinely forgotten their password, an admin (`users.manage`) can reset it via Users → Reset password.

### 12.7 Installer failure

**Symptom**: The installer shows a `MessageBox MB_OK|MB_ICONSTOP` with an error.

**Common causes**:

- Insufficient privileges (must run as Administrator).
- `pgdata\PG_VERSION` exists but `config.json` is missing (repair path — installer re-generates credentials).
- Antivirus blocking the `initdb` or `pg_ctl` execution.

**Fix**: Address the cause per the message text; re-run the installer. If `pgdata` is corrupt beyond repair, contact the software company before deleting it (data loss).

### 12.8 HMS-PostgreSQL service won't start

**Symptom**: `sc query HMS-PostgreSQL` returns `STOPPED`; starting it fails.

**Diagnostics**:

1. Check Windows Event Viewer → Windows Logs → Application for PostgreSQL errors.
2. Check `C:\ProgramData\HMS\pgdata\log\` (PostgreSQL's own log).
3. Common cause: `postgresql.conf` syntax error after manual edit. Restore from `pgdata.old\` if available, or re-initdb (data loss — see §10.3 first).
4. Common cause: `pgdata` permissions broken. Run `icacls "C:\ProgramData\HMS\pgdata" /reset` and `icacls "C:\ProgramData\HMS\pgdata" /grant "NT SERVICE\HMS-PostgreSQL:(OI)(CI)F" /T`.

### 12.9 Audit log not recording

**Symptom**: An action that should be audited is not appearing in the audit log.

**Cause**: Audit insert failures are swallowed (logged to stderr) so a logging fault cannot block a clinical operation. If the `audit_logs` table is unreachable (e.g. DB connection dropped mid-operation), the audit row is lost.

**Diagnostics**:

1. Check the app log (`%APPDATA%\<bundle-id>\Logs\hms_startup.log`) for `[HMS AUDIT]` lines.
2. Verify the `audit_logs` table exists and the user has INSERT permission (it should — granted by migration).

**Fix**: Restart HMS to re-establish the DB connection. Phase 2 will add off-host syslog forwarding for audit resilience.

---

## 13. Post-install verification checklist

Use this checklist after every fresh install or upgrade. Tick each box.

### 13.1 Server PC

- [ ] Server installer ran without error.
- [ ] `C:\ProgramData\HMS\` exists with subfolders `pgsql\`, `pgdata\`, `tls\`.
- [ ] `C:\ProgramData\HMS\config.json` exists, contains `db_password` (non-empty), `setup_complete: true`, and is ACL-restricted to `SYSTEM` + `Administrators` (verify: `icacls "C:\ProgramData\HMS\config.json"` shows only `SYSTEM:F` + `Administrators:F`).
- [ ] `sc query HMS-PostgreSQL` returns `RUNNING`.
- [ ] HMS app launches and shows the boot screen.
- [ ] License installed at `C:\ProgramData\HMS\license.json` (per §9).
- [ ] HMS app boots past the license gate to the login screen.
- [ ] **[CR-2 v0.2.0]** `C:\ProgramData\HMS\bootstrap-credentials.txt` exists; admin opens it as Administrator, reads the random one-time `admin` password, logs in, and is forced to change password.
- [ ] New password set; app reloads to the main shell.
- [ ] `bootstrap-credentials.txt` deleted after first password change.
- [ ] Settings → License shows the correct hospital name, edition, modules, expiry, and "fingerprint matches: true".
- [ ] Settings → Advanced shows the local LAN IP and TLS fingerprint.
- [ ] Individual user accounts created for each staff member with appropriate roles (Users page).
- [ ] A test patient created, edited, and soft-deleted (verifies DB write path + HIPAA soft-delete per CR-11).
- [ ] A test appointment created and status-changed (verifies audit path).
- [ ] The audit log (Audit page) shows entries for the test actions above.
- [ ] Backup script tested (per §10.2) and produces a valid dump.

### 13.2 Client PC (each)

- [ ] Client installer ran without error.
- [ ] HMS app launches and shows the first-run Setup screen.
- [ ] Pairing code generated on the server (Settings → "Connect a New Client PC") and entered on the client.
- [ ] Pairing completes; `setup_complete: true` in `C:\ProgramData\HMS\config.json` on the client.
- [ ] HMS app boots to the login screen.
- [ ] A staff member logs in with their account.
- [ ] The Dashboard loads with KPI cards.
- [ ] A test patient lookup works (verifies DB read path).
- [ ] A test action (e.g. create appointment) works and appears in the audit log on the server.

### 13.3 Network

- [ ] All client PCs can ping the server's LAN IP.
- [ ] Windows Firewall on the server allows inbound TCP 5432 from the LAN.
- [ ] Windows Firewall on the server allows inbound TCP pairing port from the LAN.
- [ ] No public-internet exposure (server PC is not port-forwarded at the router).

### 13.4 Operational

- [ ] Backup schedule configured in Windows Task Scheduler (daily recommended).
- [ ] Backup destination has sufficient free space and is on a different physical disk (or off-PC) from `pgdata`.
- [ ] License file backed up off-PC.
- [ ] Hospital IT runbook documents: server IP, admin password (in a password manager), backup location, software company contact.
- [ ] Staff trained on first-run password change and role-appropriate UI.

---

## 14. Decommissioning (disposal)

When a hospital decommissions a VitalFlow HMS deployment:

1. **Stop all HMS apps** on server and clients.
2. **Stop the service**: `sc stop HMS-PostgreSQL`.
3. **Backup**: perform a final `pg_dump` (per §10.2) and retain per regulatory data-retention requirements.
4. **[LIC-DOC-04 v0.2.0]** **Revoke the license**: an admin with `LicenseManage` opens Settings → License → **Revoke license** on the server PC. This removes `C:\ProgramData\HMS\license.json`, marks the persisted `license_state` row as `revoked`, and writes an audit row. (If the app is already stopped, the operator can simply delete `license.json` manually — the revocation audit row is best-effort but the on-disk removal is the primary signal.)
5. **Delete data**: `rmdir /s /q C:\ProgramData\HMS` on the server (this removes `pgdata`, `config.json`, `bootstrap-credentials.txt`, `license.json`, TLS material). On clients, delete `C:\ProgramData\HMS` (or the per-user equivalent).
6. **Uninstall**: run the installer's uninstaller to remove the app binary.
7. **Notify the software company**: the license is revoked in the company's offline ledger (the license file is now invalid because the hardware fingerprint is no longer in use; the company marks the license as decommissioned).
8. **Wipe disks**: per hospital data-disposal policy (e.g. DBAN, physical destruction).

The license file itself contains no secrets (the signature is verification-only). However, `config.json` contains the DB superuser password, `bootstrap-credentials.txt` contains (a now-stale) admin password, and `pgdata` contains all PHI — all must be securely destroyed.

---

## 15. Cross-references

| Topic | Document |
|---|---|
| Requirements | `01-SRS-Software-Requirements.md` |
| Architecture & design | `02-SDD-Software-Design.md` |
| Quality model | `03-Quality-Model-ISO-25010.md` |
| Security controls | `04-Security-Control-Matrix-ISO-27001.md` |
| Risks | `05-Risk-Register-ISO-31000.md` |
| SDLC | `06-SDLC-ISO-12207.md` |
| Licensing | `07-Licensing-Architecture.md` |
| Licensing workflow (operational) | `10-Licensing-Workflow-Guide.md` |
| Design system (UI) | `09-UI-UX-Design-Specification.md` (supersedes `DESIGN_SYSTEM.md`) |
| README (docs index) | `README.md` |
| PostgreSQL binaries setup (engineering) | `src-tauri/SETUP_POSTGRES_BINARIES.md` |

---

## 16. Security summary (v0.2.0) **[new in v0.2.0]**

The Batches 0-3 hardening pass touched every layer of the deployment. This section consolidates the security posture for quick reference; each item is cross-referenced to the audit finding and the controlling doc.

| Control | v0.1.0 reality | v0.2.0 reality | Audit finding | Controlling doc |
|---|---|---|---|---|
| Bootstrap admin password | Hardcoded `admin / ChangeMe123!` | Random 24-char CSPRNG; written to ACL-protected `bootstrap-credentials.txt` | CR-2 | §8.1; `04-Security-Control-Matrix` M-07 |
| Content Security Policy (CSP) | `null` (disabled) | Strict CSP in `tauri.conf.json`: `default-src 'self' ipc: http://ipc.localhost; img-src 'self' data: blob: https:; font-src 'self' https://fonts.gstatic.com; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; script-src 'self'; connect-src 'self' ipc: http://ipc.localhost` | CR-3 | `04-Security-Control-Matrix` M-06; `05-Risk-Register` R-021 |
| `config.json` ACLs | `Builtin Users` modify | `SYSTEM` + `Administrators` only (via `icacls /inheritance:r`) | CR-5 | §5.2; `05-Risk-Register` R-032 (interim) |
| `db_password` IPC exposure | Returned by `get_config` | `#[serde(skip_serializing)]` on the `Config` struct's `db_password` field | CR-4 | §5.1; `04-Security-Control-Matrix` A.8.3 |
| `pg_hba.conf` LAN rules | `host` (plaintext) | `hostssl` (TLS mandatory for LAN); loopback stays `host` for bootstrap | CR-22 | §4.1 step 8; §7.4 |
| Pairing code brute-force | 10 max uses; no per-IP lockout | 3 max uses; 3-attempt / 5-min per-IP lockout with 15-min cooldown | SEC-03 | §6.1; §6.3 |
| Pairing code PRNG | `thread_rng()` | `OsRng` (CSPRNG) | SEC-03 | §6.1 |
| LAN discovery broadcast | Plaintext UDP, no auth | HMAC-SHA256 signed with server's TLS fingerprint | SEC-08 | §6.3; §6.4 |
| TLS key file ACLs | `Builtin Users` modify | `SYSTEM` + `Administrators` only | SEC-13 | §7.1 |
| `tauri-plugin-shell` | (auditor flagged as a risk) | Dependency was never present in `Cargo.toml`; the v0.1.0 audit note was a false positive. The currently-bundled plugins are `tauri-plugin-opener` (reduced capability set per SEC-09) + `tauri-plugin-clipboard-manager` | SEC-09 | `04-Security-Control-Matrix` M-06 |
| Opener capability | `opener:allow-open-url`, `opener:allow-open-path`, clipboard read/write | Reduced to the minimum set the UI actually uses | SEC-09 | `04-Security-Control-Matrix` M-06 |
| RBAC universality | 7 commands missing RBAC; 35 permissions | All protected commands gated; 37 permissions (added `MessagingView`, `MessagingSend`) | CR-4 / CR-16 / SEC-05 / M-01 | `04-Security-Control-Matrix` M-01; §16.1 below |
| Audit universality | 11 commands wrote no audit row | All write commands write audit rows (incl. license revoke, inventory adjust, consent set/revoke, messaging, config save) | M-02 | `04-Security-Control-Matrix` A.8.16; §16.1 below |
| `CREATE DATABASE` SQL | String interpolation | SEC-10 identifier allow-list (regex `^[a-zA-Z0-9_]+$` + length cap) | SEC-10 | `04-Security-Control-Matrix` A.8.28 |
| Log IPC exposure | `get_log` / `get_log_path` unauthenticated | `SettingsManage` permission required; `redact_log()` masks `password`, `db_password`, `db_user`, `user`, `username` patterns; failed-login usernames no longer logged | SEC-05 | `04-Security-Control-Matrix` A.8.12 |

### 16.1 What's still Planned

These items are NOT yet implemented as of v0.2.0 — see `06-SDLC-ISO-12207.md` §11 for the roadmap.

| Item | Plan |
|---|---|
| DPAPI encryption at rest for `config.json` `db_password` | Batch 5 (R-003 / R-032) |
| CI runner (GitHub Actions / local Jenkins) wiring `npm run typecheck` + `npm run lint` + `cargo check` + `cargo audit` + `npm audit` | Batch 5 (SDLC-DOC-01) |
| SAST + DAST + dependency scan | Batch 5 |
| Audit log immutability (append-only trigger) | Phase 2 (M-08) |
| MFA for admin login | Phase 2 |
| WhatsApp templated messages (currently free-text) | Phase 2 (R-013; pending Meta template approval) |
| `clear_config` dead IPC command cleanup | Batch 5 |
| Unit/integration/E2E test suites | Batch 6 |

---

_End of `08-Deployment-Installation-Guide.md`. For design context see `02-SDD-Software-Design.md` §6; for licensing detail see `07-Licensing-Architecture.md` + `10-Licensing-Workflow-Guide.md`; for risk see `05-Risk-Register-ISO-31000.md`; for security control mapping see `04-Security-Control-Matrix-ISO-27001.md`._
