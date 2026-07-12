# VitalFlow HMS — TLS Fix & Deployment Guide

## What Was Wrong (The Bug Explained)

The error **"Root connect failed: error occurred while attempting to establish a TLS connection: server does not support TLS"** was caused by a chicken-and-egg problem:

```
First launch sequence (BROKEN):
  1. App starts → checks Postgres is running ✅
  2. App tries to enable SSL on Postgres
     → but first needs to connect to verify Postgres is reachable
     → connect_root() always used sslmode=require  ❌
     → Postgres has NO SSL yet (pg_hba.conf still says plain `host`)
     → Postgres rejects the TLS handshake → CRASH
```

**Three specific bugs were fixed:**

### Bug 1 — `db.rs`: No way to connect before SSL is enabled
`connect_root()` always built URLs with `sslmode=require`. On first launch, PostgreSQL hasn't had SSL enabled yet, so there's nothing to connect to with TLS. A new `connect_root_no_ssl()` function was added that uses `sslmode=disable` — used exactly once, during the bootstrap phase.

### Bug 2 — `pg_provision.rs`: Marker file written AFTER restart (crash safety)
If the app crashed between writing `postgresql.conf` and the service restart completing, the next launch would try to write SSL config again, double-appending to `postgresql.conf` and corrupting it. Fixed by writing the marker file **before** the restart.

### Bug 3 — `pg_provision.rs`: Service restart not waiting long enough
`restart_service()` only slept 2 seconds after `sc start`, then returned. PostgreSQL on Windows can take 4–8 seconds to be ready. The health check poll count was also increased from 10 to 20 attempts (10 seconds total).

---

## Files Changed

| File | What Changed |
|------|-------------|
| `src-tauri/src/db.rs` | Added `connect_root_no_ssl()` and `connect_root_internal()`. `build_url()` now takes a `require_ssl: bool` parameter. |
| `src-tauri/src/pg_provision.rs` | Added `is_ssl_already_enabled()`. Fixed marker file order. Increased restart wait times. |
| `src-tauri/src/lib.rs` | `initialize_as_server()` now checks `is_ssl_already_enabled()` first, calls `connect_root_no_ssl()` for the bootstrap connect, then proceeds with normal SSL after. |

---

## How to Build

### Prerequisites (on your development machine)
- Rust + Cargo (https://rustup.rs)
- Node.js 18+ and npm
- Tauri CLI: `cargo install tauri-cli`

### Step 1: Install frontend dependencies
```bash
cd hospital-mgmt
npm install
```

### Step 2a: Build the SERVER installer
```bash
cd src-tauri
cargo tauri build --features server-build --config tauri.server.conf.json
```
The output will be at:
`src-tauri/target/release/bundle/nsis/HMS Server_x.x.x_x64-setup.exe`

### Step 2b: Build the CLIENT installer
```bash
cargo tauri build --features client-build --config tauri.client.conf.json
```
The output will be at:
`src-tauri/target/release/bundle/nsis/HMS Client_x.x.x_x64-setup.exe`

> **Important:** You need the PostgreSQL binaries bundled for the server build.
> See `src-tauri/SETUP_POSTGRES_BINARIES.md` for how to place them.

---

## Server PC Setup (Reception Desk)

This is the PC that runs PostgreSQL and acts as the database host.

### Step 1: Run the installer
1. Copy `HMS Server_x.x.x_x64-setup.exe` to the server PC
2. **Right-click → Run as Administrator**
3. Complete the installation wizard
4. The installer automatically:
   - Copies PostgreSQL binaries to `C:\ProgramData\HMS\pgsql\`
   - Runs `initdb` to create the database cluster
   - Generates a secure random password
   - Registers `HMS-PostgreSQL` as a Windows Service (auto-starts on boot)
   - Writes credentials to `C:\ProgramData\HMS\config.json`

### Step 2: First launch — SSL setup
1. Launch "HMS Server" from the Start Menu (normal user account is fine)
2. You will see these status messages in order:
   - "Checking PostgreSQL service..." ✅
   - "Preparing encrypted connections..." ✅
   - **"Enabling encrypted database connections (first-time setup)..."** ← this is new, takes ~10 seconds
   - "Waiting for PostgreSQL to restart..." ✅
   - "Encrypted connections enabled." ✅
   - "Connecting to database..." ✅
   - "Ready!" ✅

> If you see "Startup Failed" during the first launch, see Troubleshooting below.

### Step 3: Note the server's LAN IP
1. In the app, go to **Settings**
2. Scroll to the bottom, expand **"Advanced / Developer Info"**
3. Note the LAN IP shown (e.g. `192.168.1.50`) — you'll need this for client setup

### Step 4: Set clinic name
1. In Settings, set your clinic name — it appears in WhatsApp messages
2. Save

---

## Client PC Setup (Doctors' Rooms, Nurses' Stations)

Each additional PC that needs to access HMS needs the client build.

### Step 1: Run the installer
1. Copy `HMS Client_x.x.x_x64-setup.exe` to the client PC
2. Install normally (no Administrator needed for client)

### Step 2: First-time pairing
Pairing securely copies the database credentials from the server to this PC over an encrypted connection — no passwords are typed or written down.

**On the SERVER PC:**
1. Go to **Settings → Pairing**
2. Click **"Generate Pairing Code"**
3. A 6-character code appears (e.g. `K7MN2X`) — valid for 10 minutes
4. Tell the person setting up the client PC this code verbally or via phone

**On the CLIENT PC:**
1. Launch HMS — the Setup screen appears automatically
2. Enter the server's LAN IP (e.g. `192.168.1.50`)
3. Enter the 6-character pairing code
4. Click **Pair**
5. The client will:
   - Connect to the server's pairing port (42011) over TLS
   - Receive and pin the server's certificate
   - Save all DB credentials securely
   - Show "Pairing successful!" ✅

### Step 3: Normal operation
After pairing, the client connects automatically on every launch. The server's certificate is pinned — if someone swaps a different machine onto that IP, the client will refuse to connect and show a warning.

---

## Firewall Rules

The installer does **not** add firewall rules automatically. You need to add these on the **server PC** if Windows Firewall is active:

```
Rule name: HMS PostgreSQL
Port: 5432 (TCP inbound)
Direction: Inbound
Action: Allow
Profile: Private (LAN only — do NOT set to Public)

Rule name: HMS Pairing
Port: 42011 (TCP inbound)
Direction: Inbound
Action: Allow
Profile: Private
```

To add via PowerShell (run as Administrator on the server):
```powershell
New-NetFirewallRule -DisplayName "HMS PostgreSQL" -Direction Inbound -Protocol TCP -LocalPort 5432 -Action Allow -Profile Private
New-NetFirewallRule -DisplayName "HMS Pairing" -Direction Inbound -Protocol TCP -LocalPort 42011 -Action Allow -Profile Private
```

---

## Troubleshooting

### "Startup Failed: server does not support TLS"
This means PostgreSQL's SSL setup didn't complete. Most likely causes:

**A) First launch after install — SSL not enabled yet:**
This should not happen with the fixed code, but if it does:
1. Check `C:\ProgramData\HMS\pgdata\.hms_ssl_enabled` exists
2. Check `C:\ProgramData\HMS\tls\server.crt` and `server.key` exist
3. If the cert exists but the marker doesn't: Postgres may have failed to restart. Run in PowerShell:
   ```powershell
   sc.exe start HMS-PostgreSQL
   ```
4. Then re-launch HMS

**B) The marker file exists but SSL still fails:**
The cert path in `postgresql.conf` may be wrong. Check:
```powershell
# Open the config file
notepad C:\ProgramData\HMS\pgdata\postgresql.conf
```
Look for the lines near the bottom:
```
ssl = on
ssl_cert_file = 'C:/ProgramData/HMS/tls/server.crt'
ssl_key_file = 'C:/ProgramData/HMS/tls/server.key'
```
The paths must use forward slashes (or escaped backslashes) and point to where the files actually are.

**C) Fresh install / corrupt state — clean reset:**
> ⚠️ This deletes all patient data. Only do this on a fresh install with no real data.
```powershell
sc.exe stop HMS-PostgreSQL
Remove-Item -Recurse -Force C:\ProgramData\HMS\pgdata
Remove-Item -Recurse -Force C:\ProgramData\HMS\tls
```
Then re-run the installer.

---

### "The PostgreSQL Windows Service is not running"
1. Try restarting it:
   ```powershell
   sc.exe start HMS-PostgreSQL
   ```
2. If that fails, check Windows Event Viewer → Windows Logs → Application for PostgreSQL errors
3. If the service doesn't exist at all, the installer hook failed — re-run the installer as Administrator

---

### Client: "TLS handshake failed" or "Certificate fingerprint mismatch"
This means the server's certificate changed (e.g. the server PC was reinstalled). Re-pair the client:
1. On the client PC, delete `C:\Users\<username>\AppData\Roaming\com.vitalflow.hms\config.json`
   (or wherever Tauri stores app config on this PC)
2. Re-launch HMS — the Setup screen will appear
3. Pair again using a fresh code from the server

---

### Checking service status manually
```powershell
# Is the service running?
sc.exe query HMS-PostgreSQL

# Can Postgres accept connections?
C:\ProgramData\HMS\pgsql\bin\pg_isready.exe -p 5432

# Connect directly (enter password when prompted)
C:\ProgramData\HMS\pgsql\bin\psql.exe -U postgres -h 127.0.0.1 -p 5432 "sslmode=require"
```

---

## What Happens on Subsequent Launches

After the first-launch SSL setup, every launch is fast and simple:

```
Normal launch sequence (after first-time setup):
  1. Check HMS-PostgreSQL service is RUNNING ✅
  2. Load/verify TLS cert (already exists, just reads file) ✅
  3. See .hms_ssl_enabled marker → skip SSL setup ✅
  4. Start pairing listener (TLS-wrapped, binds once) ✅
  5. Connect to DB with sslmode=require ✅
  6. Run migrations (IF NOT EXISTS — fast no-ops after first run) ✅
  7. Start scheduler ✅
  8. "Ready!" ✅
```

Total expected startup time after first launch: **3–6 seconds**.
