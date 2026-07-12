# Bundling PostgreSQL Into the Server Installer

This project no longer uses `pg_embed` (which downloaded PostgreSQL from the
internet at runtime). Instead, the **server build** bundles real PostgreSQL
binaries as an installer resource and installs them as a native Windows
Service on first launch — no internet required at install time, no foreign
installer GUI.

You need to do this **once**, manually, before your first `tauri:build:server`.
It is not something the app does for you, because the binaries are ~40 MB+
and shouldn't live in source control.

## 1. Download the binaries-only zip

Go to: https://www.enterprisedb.com/download-postgresql-binaries

Pick a Windows x86-64 build. **PostgreSQL 16 or 17 is recommended** — recent
enough to be supported long-term, old enough to be thoroughly stable.

This is explicitly the "no installer" zip meant for exactly this use case
(bundling Postgres inside another application's installer) — don't use the
regular EDB installer .exe.

## 2. Extract into `src-tauri/resources/pgsql`

After extracting, you should have:

```
src-tauri/
  resources/
    pgsql/
      bin/
        pg_ctl.exe
        initdb.exe
        postgres.exe
        pg_isready.exe
        ... (many more .exe and .dll files)
      lib/
        ...
      share/
        ...
```

## 2a. ⚠️ CRITICAL — Delete the `pgAdmin 4` folder before building

The EnterpriseDB binaries zip includes **pgAdmin 4** — a web-based admin GUI
that HMS does NOT use. It adds ~100+ MB and thousands of files with very long
paths (e.g. `pgsql\pgAdmin 4\docs\en_US\html\procedure_dialog.html`).

**You MUST delete it before running `tauri:build:server`, or the NSIS
installer will fail with "error writing to file" on the long pgAdmin paths.**

Delete this entire folder:
```
src-tauri/resources/pgsql/pgAdmin 4/
```

You can also safely delete these folders to shrink the installer further
(HMS only uses `bin/` and `share/`):

```
src-tauri/resources/pgsql/docs/         # PostgreSQL HTML documentation
src-tauri/resources/pgsql/include/      # C headers (only needed for C extension dev)
```

After cleanup, the `resources/pgsql/` folder should contain only:
```
pgsql/
  bin/      # ← REQUIRED: pg_ctl, initdb, postgres, pg_isready, psql + DLLs
  lib/      # ← REQUIRED: shared libraries used by the bin/ exes
  share/    # ← REQUIRED: locale, timezones, error messages, extensions
```

This reduces the bundle from ~300 MB to ~100 MB and eliminates the
NSIS long-path extraction failure.

The important thing is that `src-tauri/resources/pgsql/bin/initdb.exe` exists
at that exact relative path — `pg_provision.rs` looks for the bundle at
`resources/pgsql` relative to the app's resource directory, and Tauri's
`bundle.resources` config (in `tauri.server.conf.json`) maps
`resources/pgsql` → `pgsql` inside the final installed app.

# Bundling PostgreSQL Into the Server Installer

This project bundles real PostgreSQL binaries as an installer resource and
installs them as a native Windows Service **during installation** — not at
runtime. No internet required at install time, no foreign installer GUI,
no `pg_embed`, and the HMS app itself never needs Administrator rights.

You need to do the binary download **once**, manually, before your first
`tauri:build:server`. It is not something the app does for you, because the
binaries are ~40 MB+ and shouldn't live in source control.

## 1. Download the binaries-only zip

Go to: https://www.enterprisedb.com/download-postgresql-binaries

Pick a Windows x86-64 build. **PostgreSQL 16 or 17 is recommended** — recent
enough to be supported long-term, old enough to be thoroughly stable.

This is explicitly the "no installer" zip meant for exactly this use case
(bundling Postgres inside another application's installer) — don't use the
regular EDB installer .exe.

## 2. Extract into `src-tauri/resources/pgsql`

After extracting, you should have:

```
src-tauri/
  resources/
    pgsql/
      bin/
        pg_ctl.exe
        initdb.exe
        postgres.exe
        pg_isready.exe
        ... (many more .exe and .dll files)
      lib/
        ...
      share/
        ...
```

## 2a. ⚠️ CRITICAL — Delete the `pgAdmin 4` folder before building

The EnterpriseDB binaries zip includes **pgAdmin 4** — a web-based admin GUI
that HMS does NOT use. It adds ~100+ MB and thousands of files with very long
paths (e.g. `pgsql\pgAdmin 4\docs\en_US\html\procedure_dialog.html`).

**You MUST delete it before running `tauri:build:server`, or the NSIS
installer will fail with "error writing to file" on the long pgAdmin paths.**

Delete this entire folder:
```
src-tauri/resources/pgsql/pgAdmin 4/
```

You can also safely delete these folders to shrink the installer further
(HMS only uses `bin/` and `share/`):

```
src-tauri/resources/pgsql/docs/         # PostgreSQL HTML documentation
src-tauri/resources/pgsql/include/      # C headers (only needed for C extension dev)
```

After cleanup, the `resources/pgsql/` folder should contain only:
```
pgsql/
  bin/      # ← REQUIRED: pg_ctl, initdb, postgres, pg_isready, psql + DLLs
  lib/      # ← REQUIRED: shared libraries used by the bin/ exes
  share/    # ← REQUIRED: locale, timezones, error messages, extensions
```

This reduces the bundle from ~300 MB to ~100 MB and eliminates the
NSIS long-path extraction failure.

The important thing is that `src-tauri/resources/pgsql/bin/initdb.exe` exists
at that exact relative path.

## 3. How provisioning actually happens

**All real provisioning happens in `src-tauri/windows/hooks.nsh`**, which
runs automatically during installation, while the NSIS installer still has
the Administrator privileges it needed to install in the first place
(`installMode: perMachine` in `tauri.server.conf.json`). The hook:

1. Creates `C:\ProgramData\HMS` and grants standard users write access to
   it (so the receptionist's normal, non-admin Windows login can save
   settings like clinic name / WhatsApp group later).
2. Copies the bundled PostgreSQL binaries there.
3. Runs `initdb` with a freshly, cryptographically generated random
   password (via PowerShell's .NET crypto APIs — NSIS's native random
   functions are NOT secure enough for this).
4. Locks down `pg_hba.conf` to LAN-only, `scram-sha-256` auth.
5. Registers PostgreSQL as a Windows Service (`HMS-PostgreSQL`), set to
   start automatically on boot.
6. Writes `C:\ProgramData\HMS\config.json` with the generated credentials.

**The HMS app itself never provisions, registers, or elevates anything.**
On every launch, `pg_provision.rs`'s `check_postgres_health()` just verifies
the service the installer already set up is running and accepting
connections. If it isn't, the app shows a clear error pointing back to
re-running the installer — it never tries to silently "fix" this with an
unexpected UAC prompt.

This split exists specifically so that whoever installs the app (e.g. IT,
with an admin account) doesn't have to be the same person who uses it every
day (e.g. the receptionist, with a standard account) — see the comments in
`config.rs` for how the machine-wide config path makes this work regardless
of which Windows user is logged in.

## 4. Build the server installer

```
npm run tauri:build:server
```

This produces an NSIS `.exe` installer with the PostgreSQL binaries and the
`hooks.nsh` provisioning script embedded inside it.

## 5. Build the client installer (no Postgres needed)

```
npm run tauri:build:client
```

This produces a much smaller installer with no database server at all, and
no admin privileges required — client PCs just connect to the reception PC
over the LAN. See the in-app Setup screen and the pairing flow in
`pairing.rs` for how a client PC obtains its database credentials securely.

## Why not provision at runtime (and why not pg_embed)?

Two earlier approaches were tried and rejected:

- **`pg_embed`** downloads PostgreSQL binaries from the internet the first
  time the app runs, which fails with no internet on first launch, isn't
  "in the installer" at all, and pins an old, less-actively-maintained
  crate version.
- **Runtime provisioning from the running app** (an earlier version of
  `pg_provision.rs`) would have required the HMS app itself to request
  Administrator elevation (a UAC prompt) on every single launch, just to
  verify/register the Windows Service — a poor experience for daily use,
  and a larger attack surface than necessary for a hospital deployment.

Provisioning once, at install time, while already elevated, avoids both
problems.

