# VitalFlow HMS — License Key Generation Tools (keygen)

Standalone Rust toolkit for the **software company** (license issuer) to:

1. Generate the Ed25519 keypair that signs VitalFlow HMS licenses.
2. Sign customer license payloads into verifiable `.license` files.
3. Compute a customer machine's hardware fingerprint (for license binding).

These tools are **NOT part of the Tauri app**. They run offline, on the
company's signing workstation, and the private key they produce must never
leave that workstation. The app embeds only the **public** key (in
`src-tauri/src/license.rs` → `COMPANY_PUBLIC_KEY`) and verifies licenses
against it on every startup.

> **Reference docs**
> - Licensing architecture: `docs/07-Licensing-Architecture.md`
> - App-side verification: `src-tauri/src/license.rs` (`verify_license_file`,
>   `verify_signature`, `LicenseFile::canonical_bytes`)
> - Fingerprint algorithm: `src-tauri/src/fingerprint.rs` (`compute()`)
> - Dev auto-license (signing pattern): `src-tauri/src/bin/dev_auto_license.rs`

---

## Prerequisites

- Rust toolchain (stable, edition 2021). The host OS does not matter for
  `gen_keys` / `sign_license`. For `get_fingerprint` to produce a
  **production** fingerprint, it must be built and run **on Windows** (the
  algorithm queries WMI — see SRS §1.4: the app is Windows-only).

## Building

```sh
cd keygen
cargo build --release
# Binaries appear in target/release/{gen_keys,sign_license,get_fingerprint}
```

The three binaries are independent Cargo `[[bin]]` targets in this one crate.
You can also run them directly with `cargo run --release --bin <name> -- <args>`.

---

## The three binaries

### `gen_keys` — generate the company Ed25519 keypair

```sh
cargo run --release --bin gen_keys -- --out-dir ./keys
```

Produces, in `./keys/`:

| File               | Contents                                        |
|--------------------|-------------------------------------------------|
| `private_key.pem`  | 32-byte Ed25519 secret seed, PEM-wrapped base64 |
| `private_key.bin`  | 32-byte Ed25519 secret seed, raw binary         |
| `public_key.pem`   | 32-byte Ed25519 verifying key, PEM-wrapped      |
| `public_key.bin`   | 32-byte Ed25519 verifying key, raw binary       |

On Unix, the private-key files are chmod'd to `0600` automatically.

Prints to **stdout**:
- The public key as a Rust array literal, in the **exact format** of
  `license.rs:49-54`, ready to paste into `COMPANY_PUBLIC_KEY`:
  ```rust
  pub const COMPANY_PUBLIC_KEY: [u8; 32] = [
      0x.., 0x.., 0x.., 0x.., 0x.., 0x.., 0x.., 0x..,
      0x.., 0x.., 0x.., 0x.., 0x.., 0x.., 0x.., 0x..,
      0x.., 0x.., 0x.., 0x.., 0x.., 0x.., 0x.., 0x..,
      0x.., 0x.., 0x.., 0x.., 0x.., 0x.., 0x.., 0x..,
  ];
  ```
- The SHA-256 fingerprint of the public key (64 hex chars), matching
  `license::get_license_public_key_fingerprint` in the app. After rebuilding
  the app with the new key, the **Settings → License** panel should display
  this same fingerprint.

Prints a loud security warning to **stderr**.

> `--force` overwrites existing key files. **Refused by default** to prevent
> accidentally destroying a production private key (which would invalidate
> every license ever signed with it).

---

### `sign_license` — sign a license payload

```sh
cargo run --release --bin sign_license -- \
    --payload  customer.json \
    --key      ./keys/private_key.pem \
    --out      customer.license
```

Reads a JSON payload (all `LicenseFile` fields except `signature`), signs the
canonical byte representation with the private key, and writes the signed
license JSON to `--out` (or stdout if `--out` is omitted).

After signing, the binary **self-verifies** the signature against the public
key derived from the private key — this catches gross errors (wrong bytes
signed, base64 mismatch) but does **not** catch drift between this binary's
`canonical_bytes()` construction and `license.rs`'s. The final ground-truth
test is loading the license in the actual app.

See **License payload format** below for the JSON schema.

---

### `get_fingerprint` — compute a machine's hardware fingerprint

Run this **on the customer's designated server PC** (the machine that will
host the HMS app). It prints the 64-character hex fingerprint to stdout:

```sh
get_fingerprint.exe
# stdout: a1b2c3d4e5f6...  (64 hex chars)
```

The customer sends this string to the software company, which pastes it into
the license payload's `hardware_fingerprint` field before signing.

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

---

## License payload format

The payload JSON passed to `sign_license --payload` contains every
`LicenseFile` field **except `signature`**. Field names and types mirror
`LicenseFile` in `src-tauri/src/license.rs:58-81` exactly.

### Schema

| Field                  | Type             | Required | Notes                                                     |
|------------------------|------------------|----------|-----------------------------------------------------------|
| `license_id`           | string           | yes      | Unique license ID, e.g. `"LIC-2026-001"`.                 |
| `hospital_id`          | string           | yes      | Short hospital code, e.g. `"H001"`.                       |
| `hospital_name`        | string           | yes      | Full hospital name, shown in the app header & receipts.   |
| `deployment_id`        | string           | yes      | Unique deployment ID for this install site.               |
| `hardware_fingerprint` | string           | yes      | 64-char hex from `get_fingerprint` on the customer's PC.  |
| `license_version`      | string           | yes      | License schema version, e.g. `"1.0"`.                     |
| `product_edition`      | string           | yes      | e.g. `"Enterprise"`, `"Standard"`.                        |
| `enabled_modules`      | array of strings | yes      | Entitled module names (see module list below).            |
| `issue_date`           | string           | yes      | ISO-8601 timestamp, e.g. `"2026-07-03T00:00:00Z"`.        |
| `expiration_date`      | string \| null   | no       | ISO-8601 hard expiry. `null`/omitted = perpetual.         |
| `maintenance_until`    | string           | yes      | ISO-8601 date through which updates are entitled.         |
| `software_version_min` | string           | yes      | Minimum app version this license allows, e.g. `"0.0.0"`.  |
| `software_version_max` | string           | yes      | Maximum app version, e.g. `"999.999.999"`.                |
| `dev`                  | boolean          | no       | `true` = dev-only (rejected by release builds). Default `false`. |

### Example payload (`customer.json`)

```json
{
  "license_id": "LIC-2026-001",
  "hospital_id": "RMC",
  "hospital_name": "Rasheed Medical Center",
  "deployment_id": "DEP-RMC-2026-001",
  "hardware_fingerprint": "a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef1234",
  "license_version": "1.0",
  "product_edition": "Enterprise",
  "enabled_modules": [
    "dashboard", "patients", "appointments", "queue",
    "ipd", "lab", "billing", "pharmacy", "inventory",
    "hr", "reports", "settings", "admin"
  ],
  "issue_date": "2026-07-03T00:00:00Z",
  "expiration_date": "2027-07-03T00:00:00Z",
  "maintenance_until": "2027-07-03T00:00:00Z",
  "software_version_min": "1.0.0",
  "software_version_max": "1.999.999",
  "dev": false
}
```

### Module names

The `enabled_modules` array uses the module identifiers the app checks at
runtime. The dev auto-license (`dev_auto_license.rs`) enables:

```
dashboard, patients, appointments, queue, ipd, lab, billing,
pharmacy, inventory, hr, reports, settings, admin
```

Issue licenses with only the modules the customer purchased.

---

## Signed license file format

`sign_license` produces a JSON file containing all payload fields **plus** a
base64 `signature` field. Example (pretty-printed; field order is alphabetical
because the builder uses a `BTreeMap`, but `serde_json` deserialization is
order-agnostic):

```json
{
  "deployment_id": "DEP-RMC-2026-001",
  "dev": false,
  "enabled_modules": ["dashboard", "patients", "..."],
  "expiration_date": "2027-07-03T00:00:00Z",
  "hardware_fingerprint": "a1b2c3...1234",
  "hospital_id": "RMC",
  "hospital_name": "Rasheed Medical Center",
  "issue_date": "2026-07-03T00:00:00Z",
  "license_id": "LIC-2026-001",
  "license_version": "1.0",
  "maintenance_until": "2027-07-03T00:00:00Z",
  "product_edition": "Enterprise",
  "signature": "<base64 Ed25519 signature, 88 chars>",
  "software_version_max": "1.999.999",
  "software_version_min": "1.0.0"
}
```

### How the signature is computed (and verified)

This MUST match `LicenseFile::canonical_bytes()` in
`src-tauri/src/license.rs:92-108` exactly. If you change one side without the
other, every signature fails to verify.

1. Build a `BTreeMap<&str, serde_json::Value>` with exactly these 14 entries
   (the BTreeMap sorts keys alphabetically — the order below is the sorted
   order, but you do not need to insert them in this order):

   ```
   deployment_id, dev, enabled_modules, expiration_date,
   hardware_fingerprint, hospital_id, hospital_name, issue_date,
   license_id, license_version, maintenance_until, product_edition,
   software_version_max, software_version_min
   ```

2. Serialize with `serde_json::to_vec(&map)` → compact JSON (no whitespace).
   This is the **canonical bytes**.

3. Sign the canonical bytes with Ed25519 (`SigningKey::sign`).

4. Base64-encode the 64-byte signature with the **STANDARD** alphabet
   (`base64::engine::general_purpose::STANDARD`). The app decodes with the
   same engine in `verify_signature` (license.rs:114-116).

5. Insert `signature` into the map and serialize the whole thing as the
   final license JSON.

`sign_license` performs an automatic self-verification after signing
(verifies the signature against the public key derived from the private key,
using the same canonical bytes) as a sanity check.

---

## End-to-end operational workflow

### One-time setup (per company / per key rotation)

1. On an **offline** signing workstation, build the keygen tools:
   ```sh
   cd keygen && cargo build --release
   ```
2. Generate the production keypair:
   ```sh
   ./target/release/gen_keys --out-dir ./prod-keys
   ```
3. Copy the printed Rust array literal into
   `src-tauri/src/license.rs`, replacing the `COMPANY_PUBLIC_KEY` constant.
4. Confirm the printed SHA-256 fingerprint — after rebuilding the app, the
   Settings → License panel must show the same value.
5. Rebuild and ship the app.
6. Move `prod-keys/` to offline storage (encrypted USB / HSM / vault).
   **The private key must never live on a networked machine.**

### Per-customer license issuance

1. Customer installs the HMS app on their designated server PC.
2. Customer runs `get_fingerprint.exe` (shipped with the installer, or
   downloaded separately) and sends you the 64-char hex output over a secure
   channel.
3. You create a payload JSON (`customer.json`) with the customer's hospital
   identity, purchased modules, validity window, and the fingerprint from
   step 2. See the schema and example above.
4. On the offline signing workstation:
   ```sh
   ./target/release/sign_license \
       --payload  customer.json \
       --key      ./prod-keys/private_key.pem \
       --out      customer.license
   ```
5. Send `customer.license` to the customer. They install it via the app's
   first-run license wizard (which calls `install_license` — this verifies
   the signature and fingerprint before persisting).
6. On every subsequent startup, `verify_license` re-checks signature +
   fingerprint + expiry. If any check fails, the app refuses to proceed
   past the license gate.

### Dev / test licenses

The repo ships with a **dev keypair** committed (`DEV_PRIVATE_KEY` in
`dev_auto_license.rs` ↔ `COMPANY_PUBLIC_KEY` in `license.rs`). This keypair
can only sign `dev: true` licenses, which release builds
(`cfg(not(debug_assertions))`) reject at the cryptographic level. Dev builds
auto-generate a local dev license via `dev_auto_license.rs` — you do not need
the keygen tools for dev work. The keygen tools are for **production** key
management only.

---

## Public key rotation

If the private key is compromised (or you simply want to rotate proactively):

1. Generate a new keypair with `gen_keys`.
2. Replace `COMPANY_PUBLIC_KEY` in `src-tauri/src/license.rs` with the new
   public key array literal (printed by `gen_keys`).
3. Rebuild and ship a new app version. Every customer must upgrade — old
   licenses (signed with the old key) will fail verification against the new
   public key.
4. Re-sign every active customer's license with the new private key and
   redistribute. (This is why you must keep a record of every issued
   license's payload — so you can re-sign without re-collecting fingerprints.)
5. Destroy the old private key.

> Rotation is expensive (every customer must upgrade + re-install their
> license). Treat the private key as irreplaceable; rotation is a last
> resort, not a routine operation.

---

## Security checklist

- [ ] Private key generated on an **offline** machine.
- [ ] `private_key.pem` / `private_key.bin` stored offline (encrypted USB /
      HSM / vault), never on the signing workstation's networked disk
      longer than necessary.
- [ ] `.gitignore` in `keygen/` covers `*.pem`, `*.bin`, `private_key*`,
      `*.license` — but **double-check** before every `git add` (gitignore
      is a safety net, not a guarantee).
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

---

## Compatibility notes

- **Crate versions** in `keygen/Cargo.toml` match `src-tauri/Cargo.toml`
  (`ed25519-dalek = "2"`, `sha2 = "0.10"`, `hex = "0.4"`, `base64 = "0.22"`,
  `serde`/`serde_json`/`chrono` same majors). If the app upgrades any of
  these, update `keygen/Cargo.toml` to match and re-test the sign → verify
  round-trip — signature encoding is stable across patch versions but a
  major-version bump in `ed25519-dalek` or `base64` could break
  compatibility.
- The **canonical bytes** construction in `sign_license.rs` is a verbatim
  copy of `LicenseFile::canonical_bytes()` in `license.rs:92-108`. If the
  struct gains/loses/renames a field, update **both** files (and
  `dev_auto_license.rs`) — the `canonical_bytes_round_trips` test in
  `license.rs` catches verifier-side drift, but not signer-side drift.
- The **fingerprint algorithm** in `get_fingerprint.rs` is a verbatim copy
  of `fingerprint::compute()` (Windows path). If `fingerprint.rs` changes,
  update this file too.
