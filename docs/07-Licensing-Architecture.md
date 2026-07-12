# VitalFlow HMS — Licensing Architecture

> **Document version: v0.2.0 — updated 2025-07-08 after Phase 2 Batches 0-3 implementation. See CHANGELOG.md for details.**

| Field | Value |
|---|---|
| **Document title** | VitalFlow HMS — Licensing Architecture |
| **Version** | 0.2.0 |
| **Date** | 2025-07-08 |
| **Status** | Draft (reconciled v0.2.0 by Documentation Team — B4-C) |
| **Classification** | Internal — Software Company Confidential (Key Management) |
| **Owner** | VitalFlow HMS Engineering / Software Company Issuer |
| **Author** | Documentation Specialist (Task 7); reconciled v0.2.0 by Documentation Team (B4-C) |
| **Related documents** | `01-SRS-Software-Requirements.md` §6, `02-SDD-Software-Design.md` §2.3 + §5.4, `04-Security-Control-Matrix-ISO-27001.md` A.8.24, `05-Risk-Register-ISO-31000.md` R-002/R-010/R-018, `08-Deployment-Installation-Guide.md`, `10-Licensing-Workflow-Guide.md`, `keygen/README.md` |

### Revision history

| Version | Date | Author | Changes |
|---|---|---|---|
| 0.1.0 | 2026-07-02 | Documentation Specialist (Task 7) | Initial licensing architecture: single-hospital model, hardware fingerprint, Ed25519 signing, verification sequence, key management runbook. |
| 0.2.0 | 2025-07-08 | Documentation Team (B4-C) | Reconciled with Phase 2 Batches 0-3: §5.4 revocation flow now implemented (LIC-DOC-04, Batch 3); §5.4 `COMPANY_PUBLIC_KEY` updated from "all-zeros placeholder" to the real committed dev keypair (CR-20, Batch 2); §6 grace-period status `"grace"` documented (LIC-DOC-07, Batch 3); §8 rejection-cases table updated to drop the stale "all-zeros placeholder" row; new §5.5 License Transfer subsection (LIC-DOC-08); new §5.6 keygen/ project subsection (CR-20, Batch 2); §7.4 Settings → License panel documented as implemented (CR-19, Batch 2); §13 traceability table extended with FR-0254 revoke_license. |

## 1. Purpose

This document specifies the licensing architecture for VitalFlow HMS: the single-hospital license model, hardware fingerprint algorithm, license file format, canonical signing scheme, verification sequence, enforcement points, rejection cases, deployment isolation, and key management. It includes a step-by-step "issuing a new hospital license" runbook with a Rust code snippet the software company runs offline.

The implementation lives in `src-tauri/src/license.rs`. This document is the canonical reference; the code is the implementation.

---

## 2. Single-hospital license model

### 2.1 Principle

Each VitalFlow HMS deployment is licensed to exactly **one hospital** and bound to exactly **one designated server PC**. There is no generic multi-hospital reuse. The license file binds together:

1. **Hospital identity** — `hospital_id` (stable internal identifier) and `hospital_name` (human-readable, shown on boot).
2. **Deployment identity** — `deployment_id` (issuer-assigned per hospital site).
3. **Hardware identity** — `hardware_fingerprint` (SHA-256 over stable Windows WMI identifiers; see §3).
4. **Module entitlement** — `enabled_modules` (list of module keys this license permits).
5. **Validity window** — `issue_date`, optional `expiration_date`, `maintenance_until`.
6. **Software version range** — `software_version_min` / `software_version_max`.

### 2.2 Deployment isolation

A deployment shall not share any of the following with any other deployment:

| Resource | Isolation mechanism |
|---|---|
| PostgreSQL database | One cluster per server PC; DB name defaults to `hms`; no shared cluster |
| Signing keys | The license private key is held offline by the software company; the embedded public key is shared across all deployments but is verification-only (cannot forge) |
| License file | Each license is bound to a unique `hardware_fingerprint`; cannot be reused on a different machine |
| Hospital identity | `hospital_id` + `deployment_id` are unique per license; embedded in the signed JSON |

### 2.3 Why single-hospital

The single-hospital model is a deliberate product constraint, not a limitation:

- **Compliance simplicity** — each hospital's data stays within its own LAN; no multi-tenant data plane to audit.
- **Operational simplicity** — one PostgreSQL cluster, one backup, one license, one IT owner.
- **License integrity** — binding to a specific hardware fingerprint prevents a single license from being reused across hospital sites, defeating casual piracy and accidental cross-deployment.

Multi-hospital SaaS is explicitly out of scope (see `01-SRS-Software-Requirements.md` §2.2).

---

## 3. Hardware fingerprint algorithm

### 3.1 Definition

On Windows, the hardware fingerprint is:

```
SHA-256(
    b"vitalflow-hms-fp-v1\0"   // 22-byte domain-separation prefix
    || cpu_id                   // Win32_Processor.ProcessorId (UTF-8)
    || b"\0"                    // separator
    || board_sn                 // Win32_BaseBoard.SerialNumber (UTF-8)
    || b"\0"                    // separator
    || bios_sn                  // Win32_BIOS.SerialNumber (UTF-8)
)
```

The 32-byte digest is hex-encoded (lowercase, 64 characters) and stored as `LicenseFile.hardware_fingerprint`.

### 3.2 Implementation

`src-tauri/src/license.rs::compute_hardware_fingerprint` branches on the build target:

- `#[cfg(target_os = "windows")]` — `compute_fingerprint_windows()` uses the `wmi` crate to query the three WMI classes. COM is initialised via `COMLibrary::new()`; queries use `WMIConnection::raw_query`.
- `#[cfg(not(target_os = "windows"))]` — `compute_fingerprint_fallback()` returns `SHA-256(b"vitalflow-hms-fp-dev-v1\0" || hostname || b"\0" || OS)` for development only. **This is NOT a production fingerprint.** It exists only so the app compiles and runs for development on non-Windows machines.

### 3.3 Stability rationale

| Component | Stable across | Changes when |
|---|---|---|
| `Win32_Processor.ProcessorId` | OS updates, driver updates, reboots | CPU is replaced |
| `Win32_BaseBoard.SerialNumber` | OS updates, BIOS updates, reboots | Motherboard is replaced |
| `Win32_BIOS.SerialNumber` | OS updates, reboots | Motherboard or BIOS is replaced (some OEMs update it on BIOS flash) |

The combination is stable across routine maintenance (driver updates, OS patches, peripheral swaps) and changes only on substantive hardware replacement — exactly the trigger we want for license re-issue.

### 3.4 Drift handling

If a genuine hardware change occurs (e.g. motherboard replacement under warranty), the fingerprint changes and the existing license is rejected at boot with `status = "fingerprint_mismatch"`. The operator's recovery path is:

1. Boot the HMS app — it shows the license-error screen.
2. Click "Show hardware fingerprint" (Settings → License, or the license-error screen's diagnostic) to display the new fingerprint.
3. Send the new fingerprint to the software company (see §10 runbook).
4. The software company issues a new license bound to the new fingerprint (same `hospital_id`, new `deployment_id` or same — issuer's choice).
5. Drop the new `license.json` at `C:\ProgramData\HMS\license.json` and restart.

The old license remains valid cryptographically but is no longer bound to this machine — it cannot be reused elsewhere because the original machine's fingerprint no longer matches anywhere.

### 3.5 Edge cases

| Case | Behaviour |
|---|---|
| WMI returns empty string for one of the three components | The empty string is included in the SHA-256 input; the resulting fingerprint is still deterministic for this machine. A machine with all three empty would have a weak fingerprint — but no production Windows PC returns all three empty. |
| `wmi` crate fails to initialise COM | `compute_fingerprint_windows` returns `Err("WMI COM init failed: ...")`; the license gate fails closed. |
| Hyper-V / VM environment | WMI typically returns synthetic values; the fingerprint is stable for the VM's lifetime but changes if the VM is moved to new virtual hardware. The software company should treat VMs as a special case (a "VM fingerprint" note in the license record). |
| Windows locked down by group policy | WMI queries are usually still available to interactive users; if blocked, the license gate fails closed. |

---

## 4. License file format

### 4.1 Fields

| Field | Type | Required | Purpose |
|---|---|---|---|
| `license_id` | string | yes | Issuer-assigned unique ID (e.g. UUID v4) |
| `hospital_id` | string | yes | Stable hospital identifier |
| `hospital_name` | string | yes | Human-readable hospital name (shown on boot screen) |
| `deployment_id` | string | yes | Issuer-assigned deployment UUID (per site) |
| `hardware_fingerprint` | string (hex 64) | yes | Target machine fingerprint |
| `license_version` | string | yes | License format version (currently `"1.0"`) |
| `product_edition` | string | yes | Edition tag (e.g. `"Enterprise"`, `"Standard"`) |
| `enabled_modules` | string[] | yes | Module entitlement list (e.g. `["dashboard","patients","ipd","lab","billing"]`) |
| `issue_date` | ISO-8601 datetime | yes | Issue timestamp (UTC) |
| `expiration_date` | ISO-8601 datetime OR null | yes | Hard expiry; `null` = perpetual |
| `maintenance_until` | ISO-8601 datetime | yes | Update entitlement window |
| `software_version_min` | string | yes | Minimum app version this license permits |
| `software_version_max` | string | yes | Maximum app version this license permits |
| `signature` | base64 string | yes | Ed25519 signature over canonical bytes of all other fields |

### 4.2 Example license JSON (unsigned, for illustration)

```json
{
  "license_id": "f4c5a2b1-7e8d-4a3b-9c2d-1e0f1a2b3c4d",
  "hospital_id": "HOSP-METRO-001",
  "hospital_name": "Metro General Hospital",
  "deployment_id": "DEP-2026-001",
  "hardware_fingerprint": "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2",
  "license_version": "1.0",
  "product_edition": "Enterprise",
  "enabled_modules": [
    "dashboard", "patients", "appointments", "queue",
    "doctors", "ipd", "lab", "billing", "inventory",
    "audit", "users", "settings"
  ],
  "issue_date": "2026-07-01T00:00:00Z",
  "expiration_date": "2027-07-01T00:00:00Z",
  "maintenance_until": "2027-01-01T00:00:00Z",
  "software_version_min": "0.1.0",
  "software_version_max": "0.99.99",
  "signature": "<base64 Ed25519 signature over canonical bytes>"
}
```

### 4.3 On-disk location

`C:\ProgramData\HMS\license.json` (Windows). Resolved by `license_file_path()`:

```rust
pub fn license_file_path(app_handle: &tauri::AppHandle) -> PathBuf {
    if let Some(pd) = std::env::var_os("ProgramData") {
        return PathBuf::from(pd).join("HMS").join("license.json");
    }
    // Fallback to per-user app data (dev mode / non-Windows).
    app_handle.path().app_data_dir()
        .map(|d| d.join("license.json"))
        .unwrap_or_else(|_| PathBuf::from("license.json"))
}
```

The file is plain JSON; its integrity comes from the Ed25519 signature, not from filesystem ACLs.

### 4.4 Persistence to DB

After successful verification, `license.rs::persist_verification` upserts a single row in `license_state`:

```sql
INSERT INTO license_state
    (license_json, hardware_fingerprint, installed_at, last_verified_at, verification_status)
VALUES ($1, $2, NOW(), NOW(), $3)
ON CONFLICT (id) DO UPDATE SET
    license_json = EXCLUDED.license_json,
    hardware_fingerprint = EXCLUDED.hardware_fingerprint,
    last_verified_at = NOW(),
    verification_status = EXCLUDED.verification_status
```

The table is single-row (id = 1 by convention; the `ON CONFLICT (id)` assumes a default row exists — the migration creates it with `id SERIAL PRIMARY KEY` and the first upsert hits the conflict path because `id=1` exists after the first insert). The DB row is for the Settings → License panel display; it is **not** consulted during boot verification (which is DB-free).

---

## 5. Canonical signing

### 5.1 Algorithm

Ed25519 (RFC 8032). Implementation: `ed25519-dalek` 2.x. The signer's signing key is 32 bytes; the verifier's verifying key is 32 bytes; the signature is 64 bytes.

### 5.2 Canonical byte representation

Both signer and verifier construct the identical byte sequence over which the signature is computed:

```rust
pub fn canonical_bytes(&self) -> Vec<u8> {
    let mut map: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
    map.insert("license_id", serde_json::json!(self.license_id));
    map.insert("hospital_id", serde_json::json!(self.hospital_id));
    map.insert("hospital_name", serde_json::json!(self.hospital_name));
    map.insert("deployment_id", serde_json::json!(self.deployment_id));
    map.insert("hardware_fingerprint", serde_json::json!(self.hardware_fingerprint));
    map.insert("license_version", serde_json::json!(self.license_version));
    map.insert("product_edition", serde_json::json!(self.product_edition));
    map.insert("enabled_modules", serde_json::json!(self.enabled_modules));
    map.insert("issue_date", serde_json::json!(self.issue_date));
    map.insert("expiration_date", serde_json::json!(self.expiration_date));
    map.insert("maintenance_until", serde_json::json!(self.maintenance_until));
    map.insert("software_version_min", serde_json::json!(self.software_version_min));
    map.insert("software_version_max", serde_json::json!(self.software_version_max));
    serde_json::to_vec(&map).expect("canonical serialization is infallible")
}
```

Key properties:

- **BTreeMap** guarantees lexicographic key ordering, independent of struct field declaration order or insertion order.
- `serde_json::to_vec` produces **compact** JSON (no whitespace, no pretty-printing).
- The `signature` field is **excluded** from the canonical bytes (it is the output, not an input).
- Both signer (offline, in the issuer's tool) and verifier (in-app) use the **exact same construction**, guaranteeing byte-identical input to the Ed25519 verify operation.

### 5.3 Signature verification

```rust
pub fn verify_signature(&self) -> Result<(), String> {
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(&self.signature)
        .map_err(|e| format!("License signature is not valid base64: {}", e))?;
    let signature = Signature::from_slice(&sig_bytes)
        .map_err(|_| format!(
            "License signature is wrong length ({} bytes, expected {}).",
            sig_bytes.len(), SIGNATURE_LENGTH
        ))?;
    let vk = VerifyingKey::from_bytes(&COMPANY_PUBLIC_KEY)
        .map_err(|e| format!("Embedded company public key is invalid: {}", e))?;
    vk.verify(&self.canonical_bytes(), &signature)
        .map_err(|_| "License signature verification FAILED — the license is forged, corrupted, or was not issued by the software company.".to_string())
}
```

### 5.4 License revocation flow

**[Implemented v0.2.0 (Batch 3 LIC-DOC-04) — the `revoke_license` command now exists in `src-tauri/src/license.rs`.]**

Revocation is the operational "undo" for a license install: it removes the on-disk `license.json` file at `C:\ProgramData\HMS\license.json`, marks the persisted `license_state` row as `verification_status = 'revoked'`, and writes an audit row recording who revoked it and when. After revocation, the next `verify_license` call at boot will fail with `Err("License file not readable ...")` because the file is gone — the app routes to the license-error screen, where the operator can either install a new (renewed) license or decommission the machine.

The `revoke_license` Tauri command:

```rust
#[tauri::command]
pub async fn revoke_license(
    app_handle: tauri::AppHandle,
    pool: tauri::State<'_, PgPool>,
    session_state: tauri::State<'_, Arc<Mutex<Option<Session>>>>,
) -> Result<(), String> {
    let session = rbac::require(&session_state, Permission::LicenseManage)?;
    // 1. Best-effort: capture license_id BEFORE deleting the file (for the audit row).
    // 2. Remove the on-disk license file.
    // 3. UPDATE license_state SET verification_status='revoked' WHERE id=1.
    // 4. audit::for_session(..., "license_revoke", "license", ...).
    Ok(())
}
```

**Permission gate:** `Permission::LicenseManage` — the same permission as `install_license`. Both commands are exposed in the Settings → License panel (see §7.4) so an admin can install or revoke from the same surface.

**Use cases** (per SDD §5.4 revocation flow):

1. **License transfer** — revoke on the OLD machine, then `install_license` on the NEW machine. See §5.5.
2. **Suspected compromise** — if an operator suspects the `license.json` file has been exfiltrated, revoke locally AND contact the software company to invalidate the `license_id` server-side (the company can refuse to re-issue for that ID).
3. **Machine decommission** — revoke before wiping / disposing of the hardware so the license file doesn't survive on disk.

The audit row is the durable record of revocation — it survives even after the license file itself is gone. See `08-Deployment-Installation-Guide.md` §14 (Decommissioning) for the operator-side procedure.

### 5.4.1 Embedded public key

```rust
// For dev mode, the keypair below (DEV_PRIVATE_KEY in dev_auto_license.rs +
// COMPANY_PUBLIC_KEY here) is committed to the repo — it can only sign
// dev-flagged licenses (dev: true), which release builds reject at the
// cryptographic level. Safe to commit.
//
// For production: generate a separate keypair with `cargo run --bin gen_keys`
// in the keygen project (see §5.6), replace COMPANY_PUBLIC_KEY below with the
// real public key, and destroy the production private key after signing
// customer licenses.
pub const COMPANY_PUBLIC_KEY: [u8; 32] = [
    0x09, 0xbb, 0xa3, 0x04, 0x12, 0x3e, 0x7a, 0x0a,
    0xa7, 0x81, 0xdc, 0xf1, 0x6f, 0x75, 0x59, 0x1e,
    0x94, 0xef, 0x9f, 0x9f, 0xdd, 0xcf, 0x40, 0xd5,
    0xaa, 0x28, 0x58, 0xc6, 0xa0, 0x4d, 0x6e, 0x8c,
];
```

**[Updated v0.2.0 (Batch 2 CR-20)]** — the v0.1.0 spec documented `COMPANY_PUBLIC_KEY` as an all-zeros placeholder (`[0u8; 32]`) that would reject every signature by design. That was inaccurate even at audit time: the source had already shipped a real (development) Ed25519 keypair. The v0.2.0 reality:

- The constant above is a **real 32-byte Ed25519 verifying key** (not all-zeros).
- The matching private key is `DEV_PRIVATE_KEY` in `src-tauri/src/bin/dev_auto_license.rs`.
- This dev keypair can only sign licenses with `dev: true`; release builds (`cfg(not(debug_assertions))`) reject `dev: true` licenses at the cryptographic level.
- For production, the software company MUST generate a separate keypair with `keygen/gen_keys` (see §5.6) and replace `COMPANY_PUBLIC_KEY` with the production public key. The dev keypair is **never** used to sign customer licenses.

This matches the SRS R-6.12 and SDD §7 closure notes — see those docs for the cross-references.

### 5.5 License transfer

**[Implemented v0.2.0 (Batch 3 LIC-DOC-08)]** — there is no dedicated `transfer_license` Tauri command, by design. License transfer is the operational sequence:

1. **On the OLD machine**: an admin with `LicenseManage` opens Settings → License → **Revoke**. This runs `revoke_license` (§5.4), which removes the on-disk `license.json` and writes an audit row.
2. **On the NEW machine**: the operator runs `get_install_fingerprint` (Tauri command, or the standalone `keygen/get_fingerprint` binary) to compute the new machine's hardware fingerprint, and sends the 64-char hex string to the software company over a secure out-of-band channel.
3. **Software company side**: the issuer runs `keygen/sign_license` (see §5.6) with the new fingerprint + the customer's hospital identity + their existing module entitlement. The new license's `license_id` and `deployment_id` MAY be new; the `hospital_id` SHOULD be the same.
4. **On the NEW machine**: the operator opens Settings → License → **Install license** (or the first-run License Setup Wizard), selects the new `.license` file, and the app verifies the signature + fingerprint before persisting.

This is simpler than a dedicated `transfer_license` command because it reuses the existing `install_license` + `revoke_license` primitives and does not require a server-side transfer API (which would itself be a security-sensitive operation requiring careful auth). The trade-off is that the software company's offline process must handle the "is this customer allowed to re-issue?" check manually — the license ledger record must be updated to reflect the decommissioned `license_id` so it cannot be re-signed.

**Why the old license cannot be replayed on the new machine:** the old license's `hardware_fingerprint` field is cryptographically bound into the canonical bytes (§5.2). The new machine's actual fingerprint will not match → `verify_license_file` step 5 returns `fingerprint_mismatch` → boot refuses. The fingerprint binding is the security boundary; revocation is the operational hygiene.

### 5.6 keygen/ project

**[Implemented v0.2.0 (Batch 2 CR-20)]** — the `keygen/` Cargo project now exists at the repository root (was a v0.1.0 spec-only item). It is a standalone Rust toolkit for the **software company** (license issuer) — it is NOT part of the Tauri app and is never shipped to customers. It produces three binaries:

| Binary | Purpose |
|---|---|
| `keygen/src/bin/gen_keys.rs` | Generates a fresh Ed25519 keypair. Writes `private_key.pem`, `private_key.bin`, `public_key.pem`, `public_key.bin` to `--out-dir`. Prints the public key as a Rust array literal in the exact format of `license.rs::COMPANY_PUBLIC_KEY`, ready to paste. Also prints the SHA-256 fingerprint of the public key (matches `license::get_license_public_key_fingerprint` in the app). Refuses to overwrite existing keys unless `--force` is passed (safety net against destroying a production private key). |
| `keygen/src/bin/sign_license.rs` | Signs a license payload JSON. Reads `--payload customer.json` (all `LicenseFile` fields except `signature`), computes the canonical bytes (byte-identical to `LicenseFile::canonical_bytes` in the app), signs with `--key ./private_key.pem`, writes the signed license JSON to `--out customer.license`. Self-verifies after signing as a sanity check. |
| `keygen/src/bin/get_fingerprint.rs` | Computes a machine's hardware fingerprint. Run on the customer's designated server PC. Prints the 64-char hex fingerprint to stdout. `--verbose` prints the raw WMI component values to stderr for debugging a mismatch (these are hardware serials — treat as sensitive). On non-Windows the tool exits with an error by default; pass `--insecure-dev-fallback` to compute a hostname-based dev fingerprint for testing the sign→verify round-trip only. |

Files at `keygen/`:

- `keygen/Cargo.toml` — crate manifest; crate versions match `src-tauri/Cargo.toml` (`ed25519-dalek = "2"`, `sha2 = "0.10"`, `hex = "0.4"`, `base64 = "0.22"`).
- `keygen/README.md` — operator runbook (one-time setup, per-customer issuance, key rotation, security checklist).
- `keygen/.gitignore` — ignores `*.pem`, `*.bin`, `private_key*`, `*.license`, `/target/`. Safety net against accidentally committing a production private key.
- `keygen/src/bin/{gen_keys,sign_license,get_fingerprint}.rs` — the three binaries.

**Workflow (now that keygen exists):**

1. One-time company setup: on an offline signing workstation, `cd keygen && cargo build --release && ./target/release/gen_keys --out-dir ./prod-keys`. Paste the printed array literal into `src-tauri/src/license.rs::COMPANY_PUBLIC_KEY`. Move `prod-keys/` to offline storage (encrypted USB / HSM / vault).
2. Per-customer issuance: customer runs `get_fingerprint.exe` → sends 64-char hex → issuer creates `customer.json` payload → `./sign_license --payload customer.json --key ./prod-keys/private_key.pem --out customer.license` → send `.license` file to customer → customer installs via Settings → License → **Install license**.

For the full operational runbook see `keygen/README.md` and `10-Licensing-Workflow-Guide.md`.

---

## 6. Verification sequence

### 6.1 When verification runs

| Event | Verification call | Behaviour on failure |
|---|---|---|
| Install (first time) | `install_license` → `verify_signature` + fingerprint match | Reject; do not persist |
| First activation | `verify_license` at boot | License-error screen; refuse to boot |
| Every startup | `verify_license` at boot (before `initialize_database`) | License-error screen; refuse to boot |
| Upgrade | `verify_license` at first boot after upgrade | License-error screen; refuse to boot |
| Renewal | `install_license` (new license replaces old) | Reject new; old remains valid until replaced |
| Settings → License panel | `get_license_info` (reads `license_state` row; does NOT re-verify) | Display last-known status |

### 6.2 `verify_license` (DB-free boot gate)

```rust
#[tauri::command]
pub async fn verify_license(app_handle: tauri::AppHandle) -> Result<LicenseInfo, String> {
    let path = license_file_path(&app_handle);
    let info = verify_license_file(&path)?;

    if info.status != "valid" {
        return Err(match info.status.as_str() {
            "expired" => "This license has expired. Contact the software company to renew.".to_string(),
            "fingerprint_mismatch" => "This license is bound to a different computer and cannot be used here.".to_string(),
            _ => "License verification failed.".to_string(),
        });
    }
    Ok(info)
}
```

This is called from `App.tsx::verifyLicenseAndBoot` **before** `initialize_database`. It is deliberately DB-free: license verification is a precondition for opening the database connection, so it cannot itself depend on the pool.

### 6.3 `verify_license_file` (per-file logic)

1. Read file → `String`. Fail: `Err("License file not readable (...)")`.
2. `serde_json::from_str` → `LicenseFile`. Fail: `Err("License file is not valid JSON")`.
3. `license.verify_signature()` — Ed25519 verify against `COMPANY_PUBLIC_KEY`. Fail: `Err("signature verification FAILED")`.
4. `compute_hardware_fingerprint()` (WMI on Windows). Fail: `Err("WMI ...")`.
5. `fingerprint_matches = license.hardware_fingerprint == actual`.
6. **[LIC-DOC-07 v0.2.0]** Parse `expiration_date` (if present). If `now > exp_dt` but `now <= exp_dt + 7 days`, set `status = "grace"`. If `now > exp_dt + 7 days`, set `status = "expired"`. The 7-day grace period is defined by `LICENSE_GRACE_PERIOD_DAYS = 7` in `license.rs`.
7. If `!fingerprint_matches && status == "valid"`, set `status = "fingerprint_mismatch"`.
8. Return `LicenseInfo { ..., status }`.

**[LIC-DOC-07 v0.2.0 (Batch 3)]** — the `status` field on `LicenseInfo` is now one of: `"valid"`, `"grace"`, `"expired"`, `"fingerprint_mismatch"`, `"unsigned"`, `"missing"`, `"revoked"`. The v0.1.0 spec documented only `valid` / `expired` / `fingerprint_mismatch`. The full current set:

| Status | Meaning | Boot proceeds? |
|---|---|---|
| `valid` | Signature OK, fingerprint matches, no expiry (or `now <= exp_dt`). | Yes |
| `grace` | `now > exp_dt` but `now <= exp_dt + 7 days`. License is past expiry but within the LIC-DOC-07 grace window. The app continues to operate; the UI surfaces a "license expiring — please renew" warning. | Yes (with warning) |
| `expired` | `now > exp_dt + 7 days`. The grace window has elapsed. | No |
| `fingerprint_mismatch` | The license's `hardware_fingerprint` does not match this machine. | No |
| `revoked` | The `revoke_license` command (§5.4) was run; `license_state.verification_status = 'revoked'`. | No (file removed) |
| `unsigned` / `missing` | Returned by some display paths when the file or signature is absent. | No |

The `verify_license` boot gate accepts both `valid` and `grace`; all other statuses fail closed. The 7-day grace period exists to prevent a hard outage at midnight on the expiry date (which could hit in the middle of a clinical shift).

### 6.4 ASCII flow diagram

```
                ┌──────────────────────────────────────────┐
                │ App.tsx phase=verifyingLicense           │
                │   invoke("verify_license")               │
                └──────────────────┬───────────────────────┘
                                   │
                                   v
        ┌──────────────────────────────────────────────────────────┐
        │ license.rs::verify_license(app_handle)                  │
        │   path = license_file_path()                            │
        │   info = verify_license_file(path)?                     │
        └──────────────────┬───────────────────────────────────────┘
                           │
                           v
        ┌──────────────────────────────────────────────────────────┐
        │ verify_license_file(path)                                │
        │                                                          │
        │  1. fs::read_to_string(path)                             │
        │       └── fail → Err("not readable")                     │
        │  2. serde_json::from_str → LicenseFile                   │
        │       └── fail → Err("not valid JSON")                   │
        │  3. license.verify_signature():                          │
        │       base64 decode → Signature                          │
        │       VerifyingKey::from_bytes(COMPANY_PUBLIC_KEY)       │
        │       vk.verify(canonical_bytes(), sig)                  │
        │       └── fail → Err("forged/corrupted")                 │
        │  4. compute_hardware_fingerprint() (WMI on Windows)      │
        │  5. fingerprint_matches = license.fp == actual_fp        │
        │  6. if expiration_date < now:                            │
        │       if now <= exp + 7d → status = "grace"   [LIC-DOC-07]│
        │       else                → status = "expired"            │
        │  7. if !fingerprint_matches && status=="valid"           │
        │        → status = "fingerprint_mismatch"                 │
        │  8. return LicenseInfo                                   │
        └──────────────────┬───────────────────────────────────────┘
                           │
                           v
        ┌──────────────────────────────────────────────────────────┐
        │ verify_license (cont.)                                   │
        │   if info.status not in {"valid", "grace"}   [LIC-DOC-07] │
        │     return Err(match status {                            │
        │       "expired"              => "license expired (the    ",
        │                                  "7-day grace period has  ",
        │                                  "elapsed)",                │
        │       "fingerprint_mismatch" => "bound to a different PC",│
        │       _                     => "verification failed",     │
        │     })                                                    │
        │   else return Ok(info)   // valid OR grace                │
        └──────────────────┬───────────────────────────────────────┘
                           │
              ok           │           err
              ┌────────────┘           └─────────────┐
              v                                      v
        bootApp(cfg)                           phase=licenseError
        (open DB pool)                         (block boot; show retry)
```

**[LIC-DOC-07 v0.2.0 (Batch 3)]** — the `grace` branch (step 6 first arm) and the `status not in {valid, grace}` accept-list (final gate) are the only changes to this flow vs. v0.1.0. The grace status lets an expired-but-still-recent license continue to operate for 7 days so a hospital doesn't go dark at midnight on the expiry date; the UI surfaces a warning.

---

## 7. Enforcement points

### 7.1 Frontend gate (UX)

`App.tsx::verifyLicenseAndBoot` calls `invoke("verify_license")` and routes to `phase="licenseError"` on failure. The license-error screen shows:

- A clear "License required" message.
- The error string from `verify_license`.
- The on-disk path the operator should drop a new license into (`C:\ProgramData\HMS\license.json`).
- A "Retry verification" button.

The frontend gate is UX only — the backend `verify_license` command is the authoritative enforcement.

### 7.2 Backend verify_license (pre-DB gate)

`license.rs::verify_license` runs before `initialize_database`. It cannot touch the database (no pool yet). This is the security precondition: the app refuses to open the database if the license is invalid.

### 7.3 Install-time enforcement

`license.rs::install_license` verifies the signature + fingerprint before persisting to disk and to `license_state`. An invalid license is rejected and not written.

### 7.4 Settings → License panel

**[Implemented v0.2.0 (Batch 2 CR-19)]** — the Settings → License panel (`src/pages/Settings.tsx`, `LicensePanel` component) is now in the application. It is exposed at `Settings → License` and surfaces:

- Hospital name, product edition, license ID, issue date, expiration date, maintenance-until date (read from the persisted `license_state` row via `get_license_info`).
- The current status badge (`valid` / `grace` / `expired` / `fingerprint_mismatch` / `revoked`) with a colour-coded chip.
- The fingerprint match indicator (boolean).
- The list of `enabled_modules` from the license.
- Two operator actions:
  - **Install license** — opens a file picker, calls `install_license` (§7.3).
  - **Revoke license** — calls `revoke_license` (§5.4). Requires confirmation.
- A **Show fingerprint** action that calls `get_install_fingerprint` and displays the 64-char hex string for the operator to send to the software company.

The panel is gated by `Permission::SettingsManage` OR `Permission::LicenseManage`. The revoke action is additionally gated by `LicenseManage`.

`get_license_info` reads the `license_state` row. It does **not** re-verify the signature (that's the job of `verify_license` at boot). It displays the last-known status for the operator's situational awareness. The Settings panel is the operator's primary interface for both initial license installation and operational license management (renew, revoke, transfer).

---

## 8. Rejection cases

| Condition | Detected at | Status returned | User-facing message | Boot proceeds? |
|---|---|---|---|---|
| File missing | `verify_license_file` step 1 | n/a (Err returned) | "License file not readable (...)" | No |
| JSON malformed | `verify_license_file` step 2 | n/a (Err returned) | "License file is not valid JSON" | No |
| Signature invalid (forged/corrupted) | `verify_license_file` step 3 | n/a (Err returned) | "License signature verification FAILED — forged, corrupted, or not issued by the software company" | No |
| ~~Public key all-zeros (placeholder)~~ **[Resolved v0.2.0 (CR-20)]** | ~~`verify_license_file` step 3~~ | ~~n/a (every sig fails)~~ | The v0.1.0 spec documented an all-zeros placeholder that would reject every signature by design. The actual code now ships a real (development) Ed25519 keypair; production builds must swap in a `keygen/gen_keys`-generated keypair (see §5.4.1 + §5.6). | n/a |
| Hardware fingerprint mismatch | `verify_license_file` step 5/7 | `"fingerprint_mismatch"` | "This license is bound to a different computer and cannot be used here." | No |
| Hard expiry in the past, within 7-day grace window **[LIC-DOC-07 v0.2.0]** | `verify_license_file` step 6 | `"grace"` | (warning) "License is past its expiration date but within the 7-day grace period — please renew." | Yes (with warning) |
| Hard expiry in the past, grace window elapsed **[LIC-DOC-07 v0.2.0]** | `verify_license_file` step 6 | `"expired"` | "This license has expired (the 7-day grace period has elapsed). Contact the software company to renew." | No |
| Revoked by admin (file removed) **[LIC-DOC-04 v0.2.0]** | `verify_license_file` step 1 (file missing after `revoke_license`) | n/a (Err returned — file not readable) | "License file not readable (...)" — operator must install a new license or decommission. | No |
| WMI failure | `compute_hardware_fingerprint` | n/a (Err returned) | "WMI COM init failed: ..." | No |
| All checks pass | — | `"valid"` | (none) | Yes |
| All checks pass but past expiry, within 7-day grace | — | `"grace"` | (warning) | Yes (with warning) |

---

## 9. Deployment isolation

Per §2.2, deployments do not share:

- **Database** — each server PC has its own PostgreSQL cluster in `C:\ProgramData\HMS\pgdata\`. The DB name defaults to `hms`; not configurable to a shared cluster.
- **Signing keys** — the verification (public) key is embedded in every binary; this is fine because it cannot forge licenses. The signing (private) key is held offline by the software company; never distributed; never embedded.
- **License file** — each license binds to a unique hardware fingerprint. A license file is not transferable across machines.
- **Hospital identity** — `hospital_id` and `deployment_id` are unique per license and embedded in the signed JSON.

---

## 10. Key management

### 10.1 Keypair generation

The software company generates an Ed25519 keypair offline. The private key is kept in cold storage (e.g. a hardware security module, an air-gapped machine, or paper backup in a safe). The public key is embedded in the application binary as `COMPANY_PUBLIC_KEY`.

### 10.2 Embedding the public key

**[Updated v0.2.0 (Batch 2 CR-20)]** — the recommended procedure is now to use the `keygen/` project (see §5.6) rather than hand-writing a key. The workflow:

1. On an offline signing workstation: `cd keygen && cargo run --release --bin gen_keys -- --out-dir ./prod-keys`.
2. The `gen_keys` binary prints the public key as a Rust array literal in the exact format of `license.rs::COMPANY_PUBLIC_KEY` — copy it.
3. Edit `src-tauri/src/license.rs` and replace the `COMPANY_PUBLIC_KEY` constant:

```rust
pub const COMPANY_PUBLIC_KEY: [u8; 32] = [
    0xab, 0xcd, 0xef, /* ... 32 bytes total, as printed by gen_keys ... */,
];
```

4. Rebuild the app (release mode).
5. Move `prod-keys/` to offline storage (encrypted USB / HSM / vault). **The private key must never live on a networked machine.**
6. The `gen_keys` binary also prints the SHA-256 fingerprint of the public key — after rebuilding, the Settings → License panel must display the same value (cross-check via `license::get_license_public_key_fingerprint`).

The currently-embedded `COMPANY_PUBLIC_KEY` is a **dev keypair** whose private key is committed at `src-tauri/src/bin/dev_auto_license.rs::DEV_PRIVATE_KEY`. The dev keypair can only sign `dev: true` licenses, which release builds reject. The dev keypair MUST be swapped out before any production build ships — see R-002 in `05-Risk-Register-ISO-31000.md`.

### 10.3 Private key protection

| Control | Implementation |
|---|---|
| Offline storage | Air-gapped signing machine or HSM |
| Access control | Two-person control; logged access |
| Backup | Paper/metal backup in a safe; tested recovery |
| Rotation | See §10.5 |
| Destruction | If compromised, rotate (§10.5) and destroy old key |

### 10.4 Key fingerprint

`get_license_public_key_fingerprint` returns `SHA-256(COMPANY_PUBLIC_KEY)` as hex, so operators can verify the embedded key matches what the software company published.

### 10.5 Rotation

If the private key is compromised (or suspected compromised):

1. Generate a new keypair offline.
2. Embed the new public key in the next app release.
3. Re-issue every active hospital's license with the new private key. (The old licenses remain cryptographically valid but the new app binary will reject them — by design, because the public key changed.)
4. Coordinate the app upgrade with the license re-issue so hospitals are not left without a working license.
5. Destroy the old private key.

Rotation is a major-coordinated-event because it requires every deployment to upgrade the app AND replace the license. The risk register entry R-018 covers this.

### 10.6 Key lifecycle states

| State | Meaning |
|---|---|
| Active | The key is in use; licenses signed with it verify against the embedded public key |
| Retired | The key is no longer used to sign new licenses but the public key is still embedded (grace period during rotation) |
| Destroyed | The key is cryptographically erased; the public key is removed from the next app release |

VitalFlow HMS Phase 1 supports only one active key at a time (single `COMPANY_PUBLIC_KEY` constant). Multi-key support (for rotation grace periods) is a Phase 2 consideration.

---

## 11. Issuing a new hospital license — runbook

### 11.1 Prerequisites (software company side)

- The offline signing machine with the Ed25519 private key.
- `ed25519-dalek` 2.x and `serde_json` 1.x Rust crates available (or a port of the signing logic to another language with a compatible Ed25519 implementation).
- The hospital's hardware fingerprint (collected by the operator per §11.2).
- The hospital's identity fields (name, ID, deployment ID, modules, validity window).

### 11.2 Step 1 — Collect the hardware fingerprint from the hospital

The hospital operator:

1. Installs the server-build HMS app on the designated server PC.
2. Boots the app — it shows the license-error screen (no license yet).
3. The license-error screen has a "Show hardware fingerprint" action (or the operator can navigate to Settings → License → Show fingerprint, which calls `get_hardware_fingerprint`).
4. The operator reads the 64-character hex fingerprint and sends it to the software company via an out-of-band secure channel (e.g. signed email, phone-verified).

### 11.3 Step 2 — Compose the license JSON (without signature)

The software company fills in the license fields per §4.1, leaving `signature` empty for now:

```json
{
  "license_id": "<new UUID v4>",
  "hospital_id": "<hospital ID>",
  "hospital_name": "<hospital name>",
  "deployment_id": "<new deployment UUID>",
  "hardware_fingerprint": "<64-char hex from operator>",
  "license_version": "1.0",
  "product_edition": "Enterprise",
  "enabled_modules": ["dashboard","patients","appointments","queue","doctors","ipd","lab","billing","inventory","audit","users","settings"],
  "issue_date": "2026-07-02T00:00:00Z",
  "expiration_date": "2027-07-02T00:00:00Z",
  "maintenance_until": "2027-01-02T00:00:00Z",
  "software_version_min": "0.1.0",
  "software_version_max": "0.99.99",
  "signature": ""
}
```

### 11.4 Step 3 — Sign the license offline

The software company runs the following Rust program on the air-gapped signing machine. The program reads the unsigned license JSON, computes the canonical bytes (identically to `LicenseFile::canonical_bytes` in the app), signs with the private key, and writes the signed license JSON.

```rust
// sign_license.rs — run on the air-gapped signing machine.
//
// Dependencies (Cargo.toml):
//   ed25519-dalek = { version = "2", features = ["rand_core", "std"] }
//   serde = { version = "1", features = ["derive"] }
//   serde_json = "1"
//   base64 = "0.22"
//
// Usage:
//   cargo run --release --bin sign_license -- \
//       --private-key-hex <64-char hex private key> \
//       --input unsigned_license.json \
//       --output license.json

use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LicenseFile {
    license_id: String,
    hospital_id: String,
    hospital_name: String,
    deployment_id: String,
    hardware_fingerprint: String,
    license_version: String,
    product_edition: String,
    enabled_modules: Vec<String>,
    issue_date: String,
    expiration_date: Option<String>,
    maintenance_until: String,
    software_version_min: String,
    software_version_max: String,
    signature: String,
}

impl LicenseFile {
    /// MUST be byte-identical to the in-app LicenseFile::canonical_bytes.
    fn canonical_bytes(&self) -> Vec<u8> {
        let mut map: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
        map.insert("license_id", json!(self.license_id));
        map.insert("hospital_id", json!(self.hospital_id));
        map.insert("hospital_name", json!(self.hospital_name));
        map.insert("deployment_id", json!(self.deployment_id));
        map.insert("hardware_fingerprint", json!(self.hardware_fingerprint));
        map.insert("license_version", json!(self.license_version));
        map.insert("product_edition", json!(self.product_edition));
        map.insert("enabled_modules", json!(self.enabled_modules));
        map.insert("issue_date", json!(self.issue_date));
        map.insert("expiration_date", json!(self.expiration_date));
        map.insert("maintenance_until", json!(self.maintenance_until));
        map.insert("software_version_min", json!(self.software_version_min));
        map.insert("software_version_max", json!(self.software_version_max));
        serde_json::to_vec(&map).expect("canonical serialization is infallible")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut priv_hex = String::new();
    let mut input = PathBuf::new();
    let mut output = PathBuf::new();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--private-key-hex" => { priv_hex = args[i+1].clone(); i += 2; }
            "--input"           => { input = PathBuf::from(&args[i+1]); i += 2; }
            "--output"          => { output = PathBuf::from(&args[i+1]); i += 2; }
            _ => { eprintln!("Unknown arg: {}", args[i]); std::process::exit(2); }
        }
    }
    if priv_hex.is_empty() || input.as_os_str().is_empty() || output.as_os_str().is_empty() {
        eprintln!("Usage: sign_license --private-key-hex <hex> --input <file> --output <file>");
        std::process::exit(2);
    }

    // Decode the 32-byte private key from hex.
    let priv_bytes = hex::decode(&priv_hex).map_err(|e| format!("private key hex decode: {}", e))?;
    if priv_bytes.len() != 32 {
        return Err(format!("private key must be 32 bytes, got {}", priv_bytes.len()).into());
    }
    let mut secret = [0u8; 32];
    secret.copy_from_slice(&priv_bytes);
    let signing_key = SigningKey::from_bytes(&secret);

    // Read the unsigned license JSON.
    let license_text = fs::read_to_string(&input)?;
    let mut license: LicenseFile = serde_json::from_str(&license_text)?;

    // Compute the canonical bytes (byte-identical to the in-app computation).
    let canonical = license.canonical_bytes();

    // Sign and base64-encode.
    let sig = signing_key.sign(&canonical);
    let sig_b64 = base64::engine::general_purpose::STANDARD.encode(sig.to_bytes());
    license.signature = sig_b64;

    // Write the signed license JSON (pretty-printed for human readability;
    // the app re-parses to canonical bytes for verification, so whitespace
    // in the on-disk file does not affect signature validity).
    let signed_json = serde_json::to_string_pretty(&license)?;
    fs::write(&output, signed_json)?;

    // Sanity: print the public key fingerprint so the operator can verify
    // the right key was used.
    let verifying = signing_key.verifying_key();
    println!("Signed license written to: {}", output.display());
    println!("Public key (hex): {}", hex::encode(verifying.to_bytes()));
    println!("License ID: {}", license.license_id);
    Ok(())
}

// A minimal hex decoder so the snippet doesn't pull in another crate
// (the real signing tool may use the `hex` crate).
mod hex {
    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        let s = s.trim();
        if s.len() % 2 != 0 { return Err("odd-length hex".to_string()); }
        (0..s.len()).step_by(2).map(|i| {
            u8::from_str_radix(&s[i..i+2], 16).map_err(|e| e.to_string())
        }).collect()
    }
    pub fn encode(b: &[u8]) -> String {
        b.iter().map(|x| format!("{:02x}", x)).collect()
    }
}
```

### 11.5 Step 4 — Send the signed license to the hospital

Send `license.json` to the hospital operator via a secure channel.

### 11.6 Step 5 — Install the license at the hospital

The hospital operator:

1. Drops `license.json` at `C:\ProgramData\HMS\license.json` (or uses Settings → License → Install license via the file picker, which calls `install_license`).
2. Restarts the HMS app.
3. The app's `verify_license` runs at boot. If the signature verifies and the fingerprint matches and the license is not expired, the boot proceeds to `initialize_database`.
4. The boot screen displays the hospital name and product edition from the license.

### 11.7 Step 6 — Record the issuance

The software company records in its offline license ledger:

| Field | Example |
|---|---|
| license_id | `f4c5a2b1-7e8d-4a3b-9c2d-1e0f1a2b3c4d` |
| hospital_id | `HOSP-METRO-001` |
| hospital_name | `Metro General Hospital` |
| deployment_id | `DEP-2026-001` |
| hardware_fingerprint | `a1b2c3...` |
| issue_date | `2026-07-02` |
| expiration_date | `2027-07-02` |
| maintenance_until | `2027-01-02` |
| signing_key_id | `key-2026-01` |
| issued_by | (issuer name) |
| issued_at | (timestamp) |
| notes | (e.g. "first issue", "renewal", "hardware change re-issue") |

### 11.8 Renewal / re-issue variants

| Scenario | Runbook change |
|---|---|
| Renewal (approaching expiry) | Same as §11.1–11.7 with new `issue_date`, new `expiration_date`, same `hardware_fingerprint` (assuming no hardware change). |
| Hardware change (fingerprint drift) | Same as §11.1–11.7 with new `hardware_fingerprint` from the operator. The old license is no longer valid for this machine; it cannot be reused elsewhere (the old fingerprint no longer matches anywhere). |
| Module upgrade (hospital purchases more modules) | Same as §11.1–11.7 with expanded `enabled_modules`. |
| Key rotation | See §10.5. Re-issue every active license with the new private key. |

---

## 12. Security considerations

### 12.1 Threat model

| Threat | Mitigation |
|---|---|
| License forgery (attacker constructs a valid-looking license without the private key) | Ed25519 signature; infeasible without the private key |
| License tampering (attacker modifies a field in a real license) | Signature verification fails because the canonical bytes change |
| License reuse on a different machine | Hardware fingerprint binding; fingerprint mismatch rejected |
| License extraction from one hospital and replay at another | Hardware fingerprint differs across machines; replay fails |
| Private key extraction from the app binary | The app contains only the public key; the private key is never embedded |
| Public key replacement (attacker patches the binary to use their own keypair) | Out of scope for in-app mitigation; relies on code signing of the installer (Phase 2) and operational integrity of the hospital PC |
| Downgrade attack (attacker runs an old app version with weaker checks) | `software_version_min` / `software_version_max` in the license; Phase 2 enforcement |
| Clock rollback (attacker sets the system clock back to keep an expired license valid) | The expiry check uses `chrono::Utc::now()` which reads the OS clock. Mitigation: Windows time sync is default; audit logs record real timestamps so rollback would be detectable. Full mitigation (RFC 3161 timestamping) is Phase 3. |

### 12.2 Limitations

- The license file is plain-text JSON on disk. Its integrity comes from the signature, not from filesystem ACLs. A hospital employee could read the license (no confidentiality concern — it contains no secrets) but cannot modify it without invalidating the signature.
- The hardware fingerprint is stable but not unforgeable — an attacker with admin rights on the hospital PC could in principle spoof WMI responses. Mitigation: this requires admin compromise, which is game-over anyway; the license is one control among many.
- Single active key (no rotation grace period). Phase 2 may add multi-key support.

---

## 13. Traceability

| Requirement | Implementation | Reference |
|---|---|---|
| FR-0240 Single-hospital binding | `LicenseFile.hospital_id`, `hospital_name`, `deployment_id` | §2 |
| FR-0241 Hardware fingerprint binding | `LicenseFile.hardware_fingerprint` + `verify_license_file` step 5 | §3, §6 |
| FR-0242 Ed25519 signature; embedded public key only | `COMPANY_PUBLIC_KEY` + `verify_signature` | §5 |
| FR-0243 Canonical BTreeMap compact JSON | `LicenseFile::canonical_bytes` | §5.2 |
| FR-0244 DB-free verification before DB open | `verify_license` called before `initialize_database` | §6.2, §7.2 |
| FR-0245 Verification on install/activation/startup/upgrade/renewal | §6.1 table | §6 |
| FR-0246 Reject forged | `verify_signature` failure path | §8 |
| FR-0247 Reject fingerprint mismatch | `verify_license_file` step 7 | §8 |
| FR-0248 Reject expired | `verify_license_file` step 6 | §8 |
| FR-0249 Perpetual licenses (`expiration_date = null`) | step 6 skips if `None` | §6.3 |
| FR-0250 Persist `license_state` row | `persist_verification` | §4.4 |
| FR-0251 `enabled_modules` list | `LicenseFile.enabled_modules` | §4.1 |
| FR-0252 `software_version_min`/`_max` | `LicenseFile` fields | §4.1 |
| FR-0253 No cross-deployment sharing | §2.2, §9 | §2, §9 |
| FR-0254 Revoke license **[new v0.2.0 (Batch 3 LIC-DOC-04)]** | `revoke_license` Tauri command; `LicenseManage` permission; audit row | §5.4 |
| LIC-DOC-07 Grace period **[new v0.2.0 (Batch 3)]** | `LICENSE_GRACE_PERIOD_DAYS = 7`; `status = "grace"`; boot gate accepts `valid` + `grace` | §6.3, §8 |
| LIC-DOC-08 License transfer **[new v0.2.0 (Batch 3)]** | No dedicated command; revoke on old + install_license on new | §5.5 |
| CR-19 Settings → License panel **[new v0.2.0 (Batch 2)]** | `LicensePanel` component in `Settings.tsx`; install + revoke + show-fingerprint actions | §7.4 |
| CR-20 keygen/ project **[new v0.2.0 (Batch 2)]** | `keygen/{gen_keys,sign_license,get_fingerprint}` binaries | §5.6 |
| CR-20 Real dev keypair (not all-zeros) **[new v0.2.0 (Batch 2)]** | `COMPANY_PUBLIC_KEY` is a real 32-byte dev keypair; production must swap | §5.4.1, §10.2 |

---

_End of `07-Licensing-Architecture.md`. Cross-reference `01-SRS-Software-Requirements.md` §6 for licensing requirements, `02-SDD-Software-Design.md` §2.3 + §5.4 for design diagrams, `04-Security-Control-Matrix-ISO-27001.md` A.8.24 for cryptography control mapping, `05-Risk-Register-ISO-31000.md` R-002/R-010/R-018 for risk, `08-Deployment-Installation-Guide.md` §5 + §9 for the operator-side license installation steps, `10-Licensing-Workflow-Guide.md` for the operational issuance workflow, and `keygen/README.md` for the binary-level reference._
