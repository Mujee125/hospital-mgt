# VitalFlow HMS — Licensing Workflow Guide

> **Document version: v0.2.0 — updated 2025-07-08 after Phase 2 Batches 0-3 implementation. See CHANGELOG.md for details.**

| Field | Value |
|---|---|
| **Document title** | VitalFlow HMS — Licensing Workflow Guide |
| **Version** | 0.2.0 |
| **Date** | 2025-07-08 |
| **Status** | Draft (reconciled v0.2.0 by Documentation Team — B4-C) |
| **Classification** | Internal — Software Company Confidential (Key Management) |
| **Owner** | VitalFlow HMS Engineering / Software Company Issuer |
| **Author** | Documentation Specialist (Task 7); reconciled v0.2.0 by Documentation Team (B4-C) |
| **Related documents** | `07-Licensing-Architecture.md`, `08-Deployment-Installation-Guide.md`, `keygen/README.md` |

This document explains the complete licensing workflow for both development and
production deployments of VitalFlow HMS. **[Updated v0.2.0 (Batch 2 CR-20)]**
The `keygen/` project now exists with the three binaries this guide references;
the v0.1.0 version of this doc described them as planned. The workflow below
is the actual implemented workflow.

### Revision history

| Version | Date | Author | Changes |
|---|---|---|---|
| 0.1.0 | 2026-07-02 | Documentation Specialist (Task 7) | Initial licensing workflow guide. |
| 0.2.0 | 2025-07-08 | Documentation Team (B4-C) | Reconciled with Phase 2 Batches 0-3: §Files updated to reference the actual `keygen/` files now in the repo (CR-20, Batch 2); §Production Licensing workflow updated to the 6-step actual flow (gen_keys → install public key → get_fingerprint → sign_license → install_license → revoke_license); §Key Management rewritten to use `gen_keys` + `--out-dir`; §Security notes expanded with the `keygen/.gitignore` safety net; revoke flow added (LIC-DOC-04); 7-day grace period noted (LIC-DOC-07); transfer flow documented as revoke + re-install (LIC-DOC-08). |

## Architecture Overview

| Context | What happens automatically | What you do manually |
|---|---|---|
| `npm run tauri:dev` | Dev license auto-generated for current machine, signed with dev key, installed to dev path | Nothing |
| `npm run tauri:build` (release) | Production build embeds public key only; no signing happens at build time | Nothing |
| Customer installer | App first-run wizard shows fingerprint; customer emails it to you; you sign and send back; customer pastes license in wizard | Customer sends fingerprint, receives license |
| You sign a customer license | `keygen/sign_license` CLI tool takes payload JSON + private key → produces signed `.license` file | Run the signing tool |

## Dev Mode Auto-Licensing

### How it works

1. You run `npm run tauri:dev` (or `tauri:dev:server` / `tauri:dev:client`).
2. The `dev:license` npm script runs `cargo run --bin dev_auto_license`.
3. The `dev_auto_license` binary:
   - Computes this machine's hardware fingerprint.
   - Builds a dev license JSON with `dev: true`.
   - Signs it with the committed dev private key.
   - Writes it to `~/.vitalflow-dev/license.json`.
4. The app starts in debug mode, finds the dev license, verifies it, and launches.

You never think about licensing during development. Every `npm run tauri:dev`
regenerates the dev license for the current machine.

### Dev/prod separation

- The dev private key is committed to the repo (in `src-tauri/src/bin/dev_auto_license.rs::DEV_PRIVATE_KEY`).
  It can only sign dev-flagged licenses (`dev: true`).
- The matching public key is `COMPANY_PUBLIC_KEY` in `src-tauri/src/license.rs`.
  It is a REAL 32-byte Ed25519 keypair (not the all-zeros placeholder that the
  v0.1.0 spec incorrectly documented — see `07-Licensing-Architecture.md` §5.4.1).
- Release builds (compiled with `--release`) reject any license with
  `dev: true` — enforced at the `verify_license_file` level.
- Dev licenses are stored at `~/.vitalflow-dev/license.json` (per-user),
  separate from the production path `C:\ProgramData\HMS\license.json`.

This means a dev license can never be used in production, even if copied.

## Production Licensing

### Customer flow (6 steps)

1. Customer installs the app via the MSI/NSIS installer.
2. App launches → no license found → **License Setup Wizard** appears.
3. **Step 1 — Fingerprint**: the wizard shows the machine's 64-character
   hardware fingerprint (calls the Tauri `get_install_fingerprint` command) with
   a "Copy" button. Customer emails this to you.
   - **Alternative**: ship the standalone `keygen/get_fingerprint.exe` binary
     to the customer (for airgapped networks / locked-down IT environments
     where they cannot install the full app first). Customer runs it on the
     designated server PC and emails the 64-char hex output.
4. **Step 2 — Sign the license**: on your offline signing workstation, you run
   `keygen/sign_license` with a payload JSON containing the customer's
   hospital identity, purchased modules, validity window, and the fingerprint
   from step 3. This produces a signed `customer.license` file. Email it back
   to the customer.
5. **Step 3 — Install license**: customer pastes the JSON into the wizard and
   clicks "Install & verify" (or uses the Settings → License → **Install
   license** file picker once logged in). The app verifies the signature +
   fingerprint, writes the license to `C:\ProgramData\HMS\license.json`, and
   proceeds to the login screen.
6. **Step 4 (ongoing) — Verify**: on every startup, `verify_license` re-checks
   signature + fingerprint + expiry (with the 7-day grace window per
   LIC-DOC-07). If any check fails, the app refuses to proceed past the
   license gate. **[LIC-DOC-04 v0.2.0]** If the customer needs to revoke
   (machine decommission, suspected compromise, or transfer to a new PC), an
   admin with `LicenseManage` opens Settings → License → **Revoke license**.

### Your flow (signing)

Prerequisites: you've run `cargo run --release --bin gen_keys -- --out-dir ./prod-keys`
once to generate your company keypair (see §Key Management below). The private
key is in `prod-keys/private_key.pem` — keep this secret, offline, and backed
up.

```sh
cd keygen
cargo run --release --bin sign_license -- \
  --payload  customer.json \
  --key      ./prod-keys/private_key.pem \
  --out      customer.license
```

The `customer.json` payload (see `keygen/README.md` §License payload format
for the full schema):

```json
{
  "license_id": "LIC-2026-001",
  "hospital_id": "H001",
  "hospital_name": "City General Hospital",
  "deployment_id": "DEP-H001-2026-001",
  "hardware_fingerprint": "646408e902d017a7...",
  "license_version": "1.0",
  "product_edition": "Enterprise",
  "enabled_modules": ["dashboard","patients","appointments","queue","ipd","lab","billing","inventory","audit","users","settings"],
  "issue_date": "2025-07-08T00:00:00Z",
  "expiration_date": "2026-07-08T00:00:00Z",
  "maintenance_until": "2026-01-08T00:00:00Z",
  "software_version_min": "0.1.0",
  "software_version_max": "0.99.99",
  "dev": false
}
```

Email `customer.license` to the customer.

### Standalone fingerprint tool

For customers who can't run the full app (airgapped networks, IT policy):

```sh
cd keygen
cargo build --release --bin get_fingerprint
# Ship target/release/get_fingerprint.exe to the customer.
```

The customer runs `get_fingerprint.exe` on their designated server PC. It
prints the 64-char hex fingerprint to stdout. They email it to you.

`--verbose` prints the raw WMI component values (CPU ProcessorId, baseboard
serial, BIOS serial) to stderr — useful for debugging a fingerprint mismatch
with the customer. **These are hardware serial numbers; treat them as
sensitive.**

On non-Windows the tool exits with an error by default. Pass
`--insecure-dev-fallback` to compute a hostname-based dev fingerprint for
testing the sign → verify round-trip on a dev machine. **Never** use the
fallback fingerprint in a real customer license — it will not match the
customer's Windows machine and `verify_license` will reject the license with
`fingerprint_mismatch`.

### License revocation **[LIC-DOC-04 v0.2.0 (Batch 3)]**

Revocation removes the on-disk `license.json` file, marks the persisted
`license_state` row as `verification_status = 'revoked'`, and writes an audit
row. The `revoke_license` Tauri command is gated behind `Permission::LicenseManage`
— the same permission as `install_license`.

Operator path: Settings → License → **Revoke license** (requires confirmation
dialog). The next `verify_license` call at boot will fail with
`Err("License file not readable ...")` and the app will route to the
license-error screen, where the operator can install a new (renewed) license
or decommission the machine.

Use cases:
- **License transfer** — see §License transfer below.
- **Suspected compromise** — revoke locally AND contact the software company
  to invalidate the `license_id` server-side.
- **Machine decommission** — revoke before wiping the disk so the license
  file doesn't survive.

### License transfer **[LIC-DOC-08 v0.2.0 (Batch 3)]**

There is no dedicated `transfer_license` Tauri command, by design. License
transfer is the operational sequence:

1. On the OLD machine: an admin opens Settings → License → **Revoke**. This
   runs `revoke_license`, removing the on-disk `license.json` and writing an
   audit row.
2. On the NEW machine: the operator runs `get_install_fingerprint` (Tauri
   command, or the standalone `keygen/get_fingerprint` binary) to get the new
   hardware fingerprint. They send the 64-char hex to you.
3. You create a new payload JSON with the new fingerprint + the customer's
   existing hospital identity + their existing module entitlement, and run
   `keygen/sign_license` to produce a new `.license` file. Email it back.
4. On the NEW machine: the operator opens Settings → License → **Install
   license** (or the first-run License Setup Wizard), selects the new
   `.license` file, and the app verifies the signature + fingerprint before
   persisting.

This is simpler than a dedicated `transfer_license` command because it reuses
the existing `install_license` + `revoke_license` primitives. The trade-off:
your offline process must handle the "is this customer allowed to re-issue?"
check manually — update the license ledger to mark the old `license_id` as
decommissioned so it cannot be re-signed.

### 7-day grace period **[LIC-DOC-07 v0.2.0 (Batch 3)]**

License verification now has a 7-day grace window after `expiration_date`.
The `LICENSE_GRACE_PERIOD_DAYS = 7` constant in `license.rs` defines this.
Behaviour:

- If `now > exp_dt` but `now <= exp_dt + 7 days`, `status = "grace"` — the app
  continues to operate; the UI surfaces a "license expiring — please renew"
  warning.
- If `now > exp_dt + 7 days`, `status = "expired"` — the app refuses to boot.

The grace window prevents a hard outage at midnight on the expiry date (which
could hit in the middle of a clinical shift). The customer should still renew
before the grace window elapses.

## Key Management

### Generating a new keypair

**[Updated v0.2.0 (Batch 2 CR-20)]** — the `keygen/` project now exists. Use
`gen_keys` (was previously documented as a planned tool):

```sh
cd keygen
cargo run --release --bin gen_keys -- --out-dir ./prod-keys
```

This produces, in `./prod-keys/`:

| File | Contents |
|---|---|
| `private_key.pem` | 32-byte Ed25519 secret seed, PEM-wrapped base64 |
| `private_key.bin` | 32-byte Ed25519 secret seed, raw binary |
| `public_key.pem` | 32-byte Ed25519 verifying key, PEM-wrapped |
| `public_key.bin` | 32-byte Ed25519 verifying key, raw binary |

On Unix, the private-key files are chmod'd to `0600` automatically.

**Prints to stdout:**
- The public key as a Rust array literal, in the **exact format** of
  `license.rs::COMPANY_PUBLIC_KEY`, ready to paste:
  ```rust
  pub const COMPANY_PUBLIC_KEY: [u8; 32] = [
      0x.., 0x.., 0x.., 0x.., 0x.., 0x.., 0x.., 0x..,
      0x.., 0x.., 0x.., 0x.., 0x.., 0x.., 0x.., 0x..,
      0x.., 0x.., 0x.., 0x.., 0x.., 0x.., 0x.., 0x..,
      0x.., 0x.., 0x.., 0x.., 0x.., 0x.., 0x.., 0x..,
  ];
  ```
- The SHA-256 fingerprint of the public key (64 hex chars). After rebuilding
  the app with the new key, the Settings → License panel should display this
  same fingerprint.

**Prints to stderr:** a loud security warning.

`--force` overwrites existing key files. **Refused by default** to prevent
accidentally destroying a production private key (which would invalidate every
license ever signed with it).

### Embedding the public key

1. Paste the printed array literal into `src-tauri/src/license.rs`, replacing
   the `COMPANY_PUBLIC_KEY` constant (currently the dev keypair).
2. Rebuild the app: `npm run tauri:build` (release mode).
3. Confirm the public-key fingerprint: open Settings → License in the rebuilt
   app — it must match the fingerprint printed by `gen_keys`.

### Rotating the dev key

If the dev private key leaks (it shouldn't matter since release builds reject
dev licenses, but for hygiene):

1. Run `cargo run --release --bin gen_keys -- --out-dir ./dev-keys` in the keygen project.
2. Update `DEV_PRIVATE_KEY` in `src-tauri/src/bin/dev_auto_license.rs` (paste
   the private key bytes).
3. Update `COMPANY_PUBLIC_KEY` in `src-tauri/src/license.rs` (paste the public
   key bytes).
4. Delete all dev licenses: `rm -rf ~/.vitalflow-dev`
5. Run `npm run tauri:dev` — new dev license auto-generated.

### Rotating the production key

If the production private key is compromised:

1. Generate a new production keypair with `gen_keys`.
2. Replace `COMPANY_PUBLIC_KEY` in `src-tauri/src/license.rs` with the new
   public key array literal.
3. Rebuild and ship a new app version. Every customer must upgrade — old
   licenses (signed with the old key) will fail verification against the new
   public key.
4. Re-sign every active customer's license with the new private key and
   redistribute. (This is why you must keep a record of every issued license's
   payload — so you can re-sign without re-collecting fingerprints.)
5. Destroy the old private key.

> Rotation is expensive (every customer must upgrade + re-install their
> license). Treat the private key as irreplaceable; rotation is a last
> resort, not a routine operation.

## Security notes

### Private key protection

- [ ] Private key generated on an **offline** machine.
- [ ] `private_key.pem` / `private_key.bin` stored offline (encrypted USB /
      HSM / vault), never on the signing workstation's networked disk longer
      than necessary.
- [ ] **[v0.2.0]** `keygen/.gitignore` covers `*.pem`, `*.bin`,
      `private_key*`, `*.license` — but **double-check** before every
      `git add` (gitignore is a safety net, not a guarantee). The full
      `.gitignore` content:
      ```
      # Private keys — NEVER commit these.
      # (Safety net — always double-check before `git add`.)
      *.pem
      *.bin
      private_key*

      # Signed license files (may contain customer-identifying data).
      *.license

      # Cargo build artifacts.
      /target/
      ```
- [ ] Private key is **never** shipped with the app, installer, or any
      customer deliverable. Only the **public** key is embedded
      (`COMPANY_PUBLIC_KEY` in `license.rs`).
- [ ] `dev: true` is **never** set on a production license (release builds
      reject it, but don't rely on that alone — audit your payloads).
- [ ] The signing workstation has no internet connection while the private
      key is loaded.
- [ ] Every issued license payload is archived (so you can re-sign after a
      key rotation without re-collecting fingerprints).
- [ ] Backups of the private key exist (encrypted, offline, access-controlled).
      Losing the private key means you can never issue or renew licenses
      without rotating the embedded public key.
- [ ] The public-key fingerprint printed by `gen_keys` matches what the
      app's Settings → License panel displays after the key is embedded.

### Compatibility notes

- **Crate versions** in `keygen/Cargo.toml` match `src-tauri/Cargo.toml`
  (`ed25519-dalek = "2"`, `sha2 = "0.10"`, `hex = "0.4"`, `base64 = "0.22"`,
  `serde`/`serde_json`/`chrono` same majors). If the app upgrades any of
  these, update `keygen/Cargo.toml` to match and re-test the sign → verify
  round-trip.
- The **canonical bytes** construction in `sign_license.rs` is a verbatim
  copy of `LicenseFile::canonical_bytes()` in `license.rs`. If the struct
  gains/loses/renames a field, update **both** files (and
  `dev_auto_license.rs`) — the `canonical_bytes_round_trips` test in
  `license.rs` catches verifier-side drift, but not signer-side drift.
- The **fingerprint algorithm** in `get_fingerprint.rs` is a verbatim copy
  of `fingerprint::compute()` (Windows path). If `fingerprint.rs` changes,
  update this file too.

## Testing

Run the unit tests:

```sh
cd src-tauri
cargo test
```

Tests verify:
- `canonical_bytes_round_trips` — the canonical format is stable across serialization.
- `canonical_bytes_includes_dev_field` — the `dev` field is in the canonical bytes.
- `missing_dev_field_defaults_to_false` — backward compatibility with old signers.
- `fingerprint_is_stable` — the fingerprint is deterministic on the same machine.

## Files

**[Updated v0.2.0 (Batch 2 CR-20)]** — the `keygen/` files now exist in the
repository (was a v0.1.0 spec-only item).

| File | Purpose |
|---|---|
| `src-tauri/src/fingerprint.rs` | Shared fingerprint computation (single source of truth) |
| `src-tauri/src/license.rs` | License struct, verification, Tauri commands (`verify_license`, `install_license`, `revoke_license`, `get_license_info`, `get_install_fingerprint`, `get_license_public_key_fingerprint`) |
| `src-tauri/src/bin/dev_auto_license.rs` | Dev auto-license generator binary |
| `keygen/Cargo.toml` | Keygen crate manifest; crate versions pinned to match `src-tauri/Cargo.toml` |
| `keygen/README.md` | Operator runbook (one-time setup, per-customer issuance, key rotation, security checklist) |
| `keygen/.gitignore` | Ignores `*.pem`, `*.bin`, `private_key*`, `*.license`, `/target/` — safety net against committing production private keys |
| `keygen/src/bin/gen_keys.rs` | Generate company keypair (private_key.pem + private_key.bin + public_key.pem + public_key.bin); prints Rust array literal for `COMPANY_PUBLIC_KEY` |
| `keygen/src/bin/sign_license.rs` | Sign customer licenses (payload JSON + private key → signed `.license` file); self-verifies after signing |
| `keygen/src/bin/get_fingerprint.rs` | Standalone fingerprint tool for customers (Windows WMI; `--insecure-dev-fallback` for non-Windows dev) |
| `src/App.tsx` `LicenseSetupScreen` | Customer-facing first-run wizard |
| `src/pages/Settings.tsx` `LicensePanel` | Settings → License panel (install + revoke + show fingerprint) **[new v0.2.0 Batch 2 CR-19]** |

---

_End of `10-Licensing-Workflow-Guide.md`. Cross-reference `07-Licensing-Architecture.md` for the full cryptographic architecture, `08-Deployment-Installation-Guide.md` §9 for the customer-side license installation steps, and `keygen/README.md` for the binary-level reference._
