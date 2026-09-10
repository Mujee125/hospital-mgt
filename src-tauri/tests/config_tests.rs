//! AERP Part G — G.3 Work Package 3: DPAPI Credential Protection.
//!
//! Proves the config PERSISTENCE semantics against real files on real disk
//! (a per-test temp directory): v1/v2→v3 migration, the real DPAPI round-trip
//! (CryptProtectData/CryptUnprotectData), the IPC-safe skip_serializing
//! property, .bak backup on migration, corrupted-blob degradation,
//! unknown-version rejection, and concurrency.
//!
//! Uses the feature-gated `AppConfig::load_from` / `save_to` APIs (explicit
//! path, identical semantics to the production load/save — see config.rs).
//! The AppHandle-level behavior (save_config merge, ACLs, GUI round-trip)
//! was verified live in the Windows verification round (see
//! docs/VERIFICATION-REPORT-WP1-WP3.md §4).
//!
//! Requires: `--features hms-integration-tests`. No DB needed.

#![cfg(feature = "hms-integration-tests")]

mod common;
use common::fixture_pw;

use hospital_mgmt_lib::config::AppConfig;
use std::path::{Path, PathBuf};

// ── Harness ──────────────────────────────────────────────────────────────────

fn test_hms_dir(tag: &str) -> PathBuf {
    // Unique PER PROCESS (not just per tag): the .bak-never-clobbered rule
    // in save() means a persistent dir turns any fixture change into a
    // stale-state failure on the next run (observed when de-literalizing
    // the fixture passwords — the old literal .bak survived). Per-process
    // uniqueness makes every run start from a clean slate.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let hms = std::env::temp_dir().join(format!(
        "hms_cfg_test_{}_{}_{}/HMS",
        tag,
        std::process::id(),
        nanos
    ));
    std::fs::create_dir_all(&hms).unwrap();
    hms
}

fn cfg(hms: &Path) -> PathBuf {
    hms.join("config.json")
}

fn bak(hms: &Path) -> PathBuf {
    hms.join("config.json.bak")
}

/// The per-install entropy file (RCTF F-01) — next to the config.
fn entropy_key(hms: &Path) -> PathBuf {
    hms.join("entropy.key")
}

/// Write a legacy v1 config (plaintext password, no config_version field).
fn write_v1(hms: &Path, password: &str) {
    let json = format!(
        r#"{{
            "mode": "server",
            "db_host": "127.0.0.1",
            "db_port": 5432,
            "db_user": "postgres",
            "db_password": "{}",
            "db_name": "hospital_db",
            "clinic_name": "VitalFlow Clinic",
            "doctors_whatsapp_group": "",
            "setup_complete": true
        }}"#,
        password
    );
    std::fs::write(cfg(hms), json).unwrap();
}

// ── G.3.1 Unit tests (WP3-U06 … U10) ─────────────────────────────────────────

/// WP3-U06 — AppConfig::default() is v1.
#[test]
fn wp3_u06_default_is_v1() {
    let c = AppConfig::default();
    assert_eq!(c.config_version, 1);
    assert!(c.db_password_encrypted.is_none());
}

/// WP3-U07 — save() with a password writes v3 (RCTF F-01): no plaintext on
/// disk, the entropy-protected DPAPI blob IS on disk (the VF-VERIF-003
/// regression guard), version = 3, and `entropy.key` was created next to
/// the config.
#[test]
fn wp3_u07_save_encrypts_password_on_disk() {
    let hms = test_hms_dir("u07");
    let mut c = AppConfig::default();
    c.db_password = fixture_pw();
    c.setup_complete = true;
    c.save_to(&cfg(&hms)).expect("save");

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(cfg(&hms)).unwrap()).unwrap();
    assert!(
        json.get("db_password").is_none(),
        "plaintext must not be on disk"
    );
    let enc = json
        .get("db_password_encrypted")
        .expect("VF-VERIF-003: blob MUST be on disk");
    assert!(
        enc.as_str().unwrap().len() > 80,
        "DPAPI blob expected, got {:?}",
        enc
    );
    assert_eq!(json["config_version"].as_u64().unwrap(), 3);
    // RCTF F-01: the entropy file must exist and be substantial (32 bytes
    // of CSPRNG by design — anything readable-by-all defeats the purpose).
    let entropy = std::fs::read(entropy_key(&hms)).expect("entropy.key must be created on save");
    assert!(
        entropy.len() >= 32,
        "entropy.key must hold at least 32 bytes, got {}",
        entropy.len()
    );
}

/// WP3-U08 — loading a v3 file decrypts the password back (round-trip
/// through the REAL DPAPI machine key + the per-install entropy).
#[test]
fn wp3_u08_load_v3_decrypts() {
    let hms = test_hms_dir("u08");
    let mut c = AppConfig::default();
    c.db_password = fixture_pw();
    c.setup_complete = true;
    c.save_to(&cfg(&hms)).expect("save");

    let loaded = AppConfig::load_from(&cfg(&hms)).expect("load");
    assert_eq!(
        loaded.db_password,
        fixture_pw(),
        "decrypt round-trip failed"
    );
    assert_eq!(loaded.config_version, 3);
}

/// WP3-U09 — loading a v1 (plaintext) file reads the password and marks v3
/// in memory; the disk file stays v1 until an explicit save.
#[test]
fn wp3_u09_load_v1_reads_plaintext_and_marks_v3() {
    let hms = test_hms_dir("u09");
    write_v1(&hms, &fixture_pw());

    let loaded = AppConfig::load_from(&cfg(&hms)).expect("load v1");
    assert_eq!(loaded.db_password, fixture_pw());
    assert_eq!(loaded.config_version, 3, "in-memory upgrade to v3");

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(cfg(&hms)).unwrap()).unwrap();
    assert!(
        json.get("config_version").is_none(),
        "disk stays v1 until an explicit save"
    );
}

/// WP3-U10 — full migration cycle: v1 → load → save → reload from v3, with
/// the .bak preserving the password in the v2 (no-entropy) shape.
/// QA-2026-09-08 C2 + RCTF F-01: the .bak is the recovery path for BOTH
/// rollback directions — binary rollback (the previous build rejects v3)
/// and entropy loss (v3 needs entropy.key to decrypt) — so it must stay in
/// the shape the pre-F-01 code path can decrypt.
#[test]
fn wp3_u10_v1_migrate_then_reload_v3() {
    let hms = test_hms_dir("u10");
    write_v1(&hms, &fixture_pw());

    let c = AppConfig::load_from(&cfg(&hms)).expect("load v1");
    c.save_to(&cfg(&hms)).expect("save (migration)");

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(cfg(&hms)).unwrap()).unwrap();
    assert_eq!(json["config_version"].as_u64().unwrap(), 3);
    assert!(json.get("db_password_encrypted").is_some());

    let bak_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(bak(&hms)).expect(".bak must exist"))
            .unwrap();
    // F-01 contract: the .bak keeps the v2 (no-entropy) shape — loadable by
    // the previous binary AND without the entropy file.
    assert!(
        bak_json.get("db_password").is_none(),
        ".bak must not contain the plaintext password"
    );
    assert_eq!(
        bak_json["config_version"].as_u64().unwrap(),
        2,
        ".bak is the v2 shape (binary-rollback + entropy-loss recovery)"
    );
    assert!(bak_json.get("db_password_encrypted").is_some());

    let recovered_from_bak = AppConfig::load_from(&bak(&hms)).expect("reload .bak");
    assert_eq!(
        recovered_from_bak.db_password,
        fixture_pw(),
        ".bak preserves the last known-good password as a RECOVERY VALUE"
    );

    let reloaded = AppConfig::load_from(&cfg(&hms)).expect("reload v3");
    assert_eq!(reloaded.db_password, fixture_pw());
}

// ── G.3.2 Integration tests (WP3-I03, I05) ───────────────────────────────────

/// WP3-I03 — the config struct NEVER serializes either password field to
/// the frontend: both `db_password` and `db_password_encrypted` are
/// skip_serializing (this is the `get_config` reply shape).
#[test]
fn wp3_i03_ipc_reply_never_contains_password() {
    let mut c = AppConfig::default();
    c.db_password = fixture_pw();
    c.db_password_encrypted = Some("BLOB".into());
    let json = serde_json::to_value(&c).unwrap();
    assert!(
        json.get("db_password").is_none(),
        "plaintext leaked to IPC shape"
    );
    assert!(
        json.get("db_password_encrypted").is_none(),
        "blob leaked to IPC shape"
    );
}

/// WP3-I05 — .bak is created ONLY on the legacy(v1/v2)→v3 transition, and
/// later saves never clobber the original backup.
#[test]
fn wp3_i05_bak_created_once_not_clobbered() {
    let hms = test_hms_dir("i05");
    write_v1(&hms, &fixture_pw());

    let mut c = AppConfig::load_from(&cfg(&hms)).unwrap();
    c.save_to(&cfg(&hms)).unwrap(); // migration → .bak created
    let bak1 = std::fs::read_to_string(bak(&hms)).unwrap();

    c.clinic_name = "Renamed Clinic".into();
    c.save_to(&cfg(&hms)).unwrap(); // later save — .bak must NOT be overwritten
    let bak2 = std::fs::read_to_string(bak(&hms)).unwrap();
    assert_eq!(
        bak1, bak2,
        "the original migration backup must survive later saves"
    );

    // A save that does NOT migrate (no v1/v2 file ever existed) never
    // creates a .bak.
    let hms2 = test_hms_dir("i05b");
    let mut c2 = AppConfig::default();
    c2.db_password = "fresh".into();
    c2.save_to(&cfg(&hms2)).unwrap(); // first save → v3 directly
    c2.save_to(&cfg(&hms2)).unwrap();
    assert!(!bak(&hms2).exists(), ".bak only exists for migrations");
}

// ── G.3.3 Negative tests (WP3-N01 … N04) ─────────────────────────────────────

/// WP3-N01/N02 — wrong-machine key and corrupted blob share the same
/// observable failure: DPAPI decrypt fails. The app must degrade to an
/// EMPTY password (never garbage) so startup redirects to the repair
/// screen. (True cross-machine testing needs a second machine — tracked
/// in the verification report §5; the corrupted-blob equivalent is
/// proven here.)
#[test]
fn wp3_n01_n02_wrong_key_or_corrupted_blob() {
    let hms = test_hms_dir("n01");
    let mut c = AppConfig::default();
    c.db_password = fixture_pw();
    c.setup_complete = true;
    c.save_to(&cfg(&hms)).unwrap();

    // Corrupt the blob: flip a character inside the base64 ciphertext.
    let path = cfg(&hms);
    let mut raw = std::fs::read_to_string(&path).unwrap();
    let idx = raw.find("db_password_encrypted").unwrap() + 40;
    raw.replace_range(idx..idx + 1, "#");
    std::fs::write(&path, raw).unwrap();

    let loaded = AppConfig::load_from(&path).expect("load must parse (degraded), not panic");
    assert_eq!(
        loaded.db_password, "",
        "corrupted blob must degrade to empty password, never garbage"
    );
}

/// WP3-N03 — a config with NO config_version field is treated as v1.
#[test]
fn wp3_n03_missing_version_treated_as_v1() {
    let hms = test_hms_dir("n03");
    write_v1(&hms, &fixture_pw()); // fixture has no config_version key
    let c = AppConfig::load_from(&cfg(&hms)).expect("missing version == v1");
    assert_eq!(c.db_password, fixture_pw());
    assert_eq!(c.config_version, 3, "marked for v3 migration");
}

/// WP3-N04 — an UNKNOWN future config_version is rejected outright, not
/// guessed (code change: explicit >3 rejection was added per spec).
#[test]
fn wp3_n04_unknown_version_rejected() {
    let hms = test_hms_dir("n04");
    let json = r#"{
        "mode": "server", "db_host": "x", "db_port": 5432, "db_user": "u",
        "db_password": "p", "db_name": "d", "clinic_name": "c",
        "doctors_whatsapp_group": "", "setup_complete": true,
        "config_version": 99
    }"#;
    std::fs::write(cfg(&hms), json).unwrap();
    assert!(
        AppConfig::load_from(&cfg(&hms)).is_none(),
        "unknown config_version must be rejected, not guessed"
    );
}

// ── G.3.4 Penetration tests (WP3-P01, P02) ───────────────────────────────────

/// WP3-P01 — config theft: the on-disk artifact contains NO recoverable
/// password. Asserts (a) no plaintext on disk, (b) the base64-decoded
/// blob does not contain the plaintext either (it is real DPAPI
/// ciphertext, not a re-encoding of the password).
#[test]
fn wp3_p01_stolen_config_yields_nothing() {
    let hms = test_hms_dir("p01");
    let mut c = AppConfig::default();
    c.db_password = fixture_pw();
    c.save_to(&cfg(&hms)).unwrap();

    let raw = std::fs::read_to_string(cfg(&hms)).unwrap();
    assert!(!raw.contains(&fixture_pw()), "plaintext on disk!");

    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let blob = json["db_password_encrypted"].as_str().unwrap();
    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(blob)
        .unwrap();
    let decoded_str = String::from_utf8_lossy(&decoded);
    assert!(
        !decoded_str.contains(&fixture_pw()),
        "blob must be real ciphertext, not a re-encoding of the plaintext"
    );
}

/// WP3-P02 — binary inspection: the DPAPI machine key is never embedded in
/// the binary — secrets.rs contains no key material, only the OS API calls.
#[test]
fn wp3_p02_no_embedded_keys_in_source() {
    let src =
        std::fs::read_to_string(format!("{}/src/secrets.rs", env!("CARGO_MANIFEST_DIR"))).unwrap();
    assert!(!src.contains("PRIVATE KEY"));
    assert!(!src.contains("BEGIN PUBLIC KEY"));
    assert!(src.contains("CryptProtectData"));
    assert!(src.contains("CryptUnprotectData"));
}

// ── G.3.5 Concurrency tests (WP3-C01, C02) ────────────────────────────────────

/// WP3-C01/C02 — concurrent load/save cycles: no corruption (atomic
/// temp+rename under contention), and concurrent migrations of one v1
/// file converge on a valid v3 with exactly one .bak preserving the v2
/// recovery shape.
#[test]
fn wp3_c01_c02_concurrent_save_load_and_migration() {
    let hms = test_hms_dir("c01");
    write_v1(&hms, &fixture_pw());

    let hms_shared = hms.clone();
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let hms = hms_shared.clone();
            std::thread::spawn(move || {
                let path = cfg(&hms);
                // Each thread: load (migrating), mutate, save, re-load.
                let mut c = AppConfig::load_from(&path).expect("load");
                c.clinic_name = format!("Clinic-{}", i);
                c.save_to(&path).expect("save");
                let re = AppConfig::load_from(&path).expect("reload");
                assert!(
                    re.db_password == fixture_pw(),
                    "reload must never see a torn write (got {:?})",
                    re.db_password
                );
            })
        })
        .collect();
    for h in handles {
        h.join().expect("no thread panics");
    }

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(cfg(&hms)).unwrap())
            .expect("final file must be valid JSON (no torn writes)");
    assert_eq!(json["config_version"].as_u64().unwrap(), 3);
    assert!(json.get("db_password_encrypted").is_some());
    let bak_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(bak(&hms)).unwrap()).unwrap();
    // F-01 contract: the .bak is the v2 (no-entropy) recovery shape.
    assert!(bak_json.get("db_password").is_none());
    assert_eq!(bak_json["config_version"].as_u64().unwrap(), 2);
    assert!(bak_json.get("db_password_encrypted").is_some());
}

// ── G.3.6 Offline test (WP3-O01) ──────────────────────────────────────────────

/// WP3-O01 — config load works with no network: AppConfig::load touches
/// only the filesystem + DPAPI (both local). Proven by construction in
/// every test above; asserted explicitly with a full save→load cycle in a
/// process that has opened no DB connection.
#[test]
fn wp3_o01_load_is_offline() {
    let hms = test_hms_dir("o01");
    let mut c = AppConfig::default();
    c.db_password = fixture_pw();
    c.save_to(&cfg(&hms)).unwrap();
    let loaded = AppConfig::load_from(&cfg(&hms)).expect("offline load");
    assert_eq!(loaded.db_password, fixture_pw());
}

// ── Phase 7: auto-backup config fields — backward compatibility ──────────────

/// A pre-Phase-7 config.json (v1 shape, no auto-backup fields) must load
/// with sensible defaults: enabled on server builds, 02:00, keep 14, no USB
/// path. This is what every upgraded deployment's first restart reads.
#[test]
fn phase7_old_config_gains_auto_backup_defaults() {
    let hms = test_hms_dir("p7defaults");
    write_v1(&hms, &fixture_pw());
    let loaded = AppConfig::load_from(&cfg(&hms)).expect("load legacy config");
    assert_eq!(
        loaded.auto_backup_enabled,
        cfg!(feature = "server-build"),
        "auto-backup default must follow the build mode"
    );
    assert_eq!(loaded.auto_backup_hour, 2);
    assert_eq!(loaded.backup_retention_count, 14);
    assert!(loaded.usb_backup_path.is_empty());
}

/// A config that HAS the Phase 7 fields must round-trip them through
/// save→load unchanged (the Settings save path), with the legacy
/// db_password invariant intact alongside.
#[test]
fn phase7_auto_backup_fields_round_trip() {
    let hms = test_hms_dir("p7roundtrip");
    let mut c = AppConfig::default();
    c.db_password = fixture_pw();
    c.auto_backup_enabled = true;
    c.auto_backup_hour = 3;
    c.backup_retention_count = 30;
    c.usb_backup_path = r"E:\HMS-Backups".to_string();
    c.save_to(&cfg(&hms)).unwrap();

    let loaded = AppConfig::load_from(&cfg(&hms)).expect("reload");
    assert!(loaded.auto_backup_enabled);
    assert_eq!(loaded.auto_backup_hour, 3);
    assert_eq!(loaded.backup_retention_count, 30);
    assert_eq!(loaded.usb_backup_path, r"E:\HMS-Backups");
    assert_eq!(loaded.db_password, fixture_pw());
}

// ── G.3.7/3.8 LAN + Windows-only tests ────────────────────────────────────────
// LAN (L01-L03) and the destructive Windows-only DPAPI tests (W01 sysprep,
// W02 service-account, W04 service restart, W05 OS reboot) require
// hardware/OS actions unavailable in-process; they were verified live
// during the Windows verification round (migration + restart round-trip
// observed on the real machine) or are documented as deferred in
// docs/VERIFICATION-REPORT-WP1-WP3.md §5. W03 (ACL post-migration) was
// verified live on the production file — report §4 row 5.

// ── RCTF-FULL-SYSTEM-2026-09-09 F-01: per-install DPAPI entropy ──────────────

/// Write a v2 config exactly like the pre-F-01 binary did — a DPAPI blob
/// with NO per-install entropy. This is the shape of the config on the
/// production machine today (E:\hospital-mgt deployment, 2026-09-09), so
/// this fixture is the migration SOURCE the new binary must handle.
fn write_v2(hms: &Path, password: &str) {
    let enc = hospital_mgmt_lib::secrets::encrypt(password).expect("legacy encrypt");
    let json = format!(
        r#"{{
            "mode": "server",
            "db_host": "127.0.0.1",
            "db_port": 5432,
            "db_user": "postgres",
            "db_name": "hospital_db",
            "clinic_name": "VitalFlow Clinic",
            "doctors_whatsapp_group": "",
            "setup_complete": true,
            "config_version": 2,
            "db_password_encrypted": "{}"
        }}"#,
        enc
    );
    std::fs::write(cfg(hms), json).unwrap();
}

/// F-01 core migration: a v2 file (no-entropy blob) loads via the LEGACY
/// decrypt, is marked v3 in memory, and the next save re-encrypts with the
/// per-install entropy — after which the on-disk blob is NOT decryptable
/// by the no-entropy path anymore (the live F-01 attack, now impossible).
#[test]
fn rctf_f01_v2_migrates_to_entropy_protected_v3() {
    let hms = test_hms_dir("f01v2");
    write_v2(&hms, &fixture_pw());

    // v2 loads: the legacy no-entropy blob still decrypts (rollback path).
    let loaded = AppConfig::load_from(&cfg(&hms)).expect("v2 must load");
    assert_eq!(loaded.db_password, fixture_pw());
    assert_eq!(loaded.config_version, 3, "marked for the v3 upgrade");

    // The migration save: disk becomes v3, entropy.key is created, and the
    // .bak preserves the v2 shape for binary-rollback/entropy recovery.
    loaded.save_to(&cfg(&hms)).expect("migration save");

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(cfg(&hms)).unwrap()).unwrap();
    assert_eq!(json["config_version"].as_u64().unwrap(), 3);
    let blob = json["db_password_encrypted"].as_str().unwrap();
    assert!(
        std::fs::read(entropy_key(&hms)).is_ok(),
        "entropy.key created"
    );

    let bak_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(bak(&hms)).expect(".bak on migration"))
            .unwrap();
    assert_eq!(bak_json["config_version"].as_u64().unwrap(), 2);

    // Reload: v3 + entropy round-trips the password back.
    let reloaded = AppConfig::load_from(&cfg(&hms)).expect("reload v3");
    assert_eq!(reloaded.db_password, fixture_pw());

    // THE F-01 pin: the no-entropy DPAPI path (what the RCTF assessment
    // used to recover the superuser password as a plain non-admin user)
    // must FAIL against the migrated blob. On non-Windows DPAPI is
    // pass-through (no crypto), so the pin is Windows-only — the Ubuntu CI
    // run proves the version semantics; this Windows run proves the crypto
    // separation.
    #[cfg(target_os = "windows")]
    {
        let legacy = hospital_mgmt_lib::secrets::decrypt(blob);
        assert!(
            legacy.is_err(),
            "a migrated v3 blob must NOT decrypt via the no-entropy path — \
             this is the exact primitive the F-01 repro abused"
        );
    }
}

/// F-01 entropy-loss recovery: if `entropy.key` is lost, the v3 blob is
/// undecryptable and the config degrades to an EMPTY password (repair
/// screen) — and the v2-shaped `.bak` (which does not need the entropy
/// file) still carries the known-good password for recovery.
#[test]
fn rctf_f01_entropy_loss_degrades_and_bak_recovers() {
    let hms = test_hms_dir("f01loss");
    write_v2(&hms, &fixture_pw());

    let loaded = AppConfig::load_from(&cfg(&hms)).expect("load v2");
    loaded.save_to(&cfg(&hms)).expect("migrate to v3");

    // Entropy file gone (disk swap, restore mishap, tamper).
    std::fs::remove_file(entropy_key(&hms)).unwrap();
    let degraded = AppConfig::load_from(&cfg(&hms)).expect("load must not panic");
    assert_eq!(
        degraded.db_password, "",
        "v3 without entropy must degrade to empty password (repair path)"
    );

    // Operator recovery: restore the .bak (v2 shape, no entropy needed).
    std::fs::copy(bak(&hms), cfg(&hms)).unwrap();
    let recovered = AppConfig::load_from(&cfg(&hms)).expect("reload from .bak");
    assert_eq!(
        recovered.db_password,
        fixture_pw(),
        ".bak (v2 shape) must recover the password without entropy.key"
    );
}

/// F-01 tamper pin: an attacker replacing entropy.key with their own bytes
/// must NOT gain access — the v3 blob was encrypted with the original
/// entropy, so a fresh key decrypts to an error, and the config degrades
/// to empty (repair screen) rather than leaking anything.
#[test]
fn rctf_f01_swapped_entropy_key_gains_nothing() {
    let hms = test_hms_dir("f01swap");
    let mut c = AppConfig::default();
    c.db_password = fixture_pw();
    c.save_to(&cfg(&hms)).expect("save v3");

    let real = std::fs::read(entropy_key(&hms)).unwrap();
    let mut forged: Vec<u8> = real.clone();
    forged[0] ^= 0xff; // attacker-controlled different key
                       // Replace the file wholesale (delete + write new): an attacker with
                       // directory-write access substitutes their own key. Note the app user's
                       // own ACE on entropy.key is Read-only (by design — the key never
                       // rotates), so an in-place rewrite is already blocked by the ACL.
    std::fs::remove_file(entropy_key(&hms)).unwrap();
    std::fs::write(entropy_key(&hms), &forged).unwrap();

    let loaded = AppConfig::load_from(&cfg(&hms)).expect("load must not panic");
    #[cfg(target_os = "windows")]
    {
        assert_eq!(
            loaded.db_password, "",
            "wrong entropy must degrade to empty password, never decrypt"
        );
    }
    // On non-Windows the pass-through decrypt "succeeds" by construction —
    // the tamper-resistance pin is the Windows crypto test above.
    #[cfg(not(target_os = "windows"))]
    {
        let _ = loaded;
    }
}

/// F-01 stability: the entropy key must be STABLE across saves — a second
/// save must reuse the existing key (a rotating key would orphan the
/// .bak-vs-live recovery story and re-create the first-use race). Also
/// pins that saving does not create a SECOND entropy temp file leak.
#[test]
fn rctf_f01_entropy_key_is_stable_across_saves() {
    let hms = test_hms_dir("f01stable");
    let mut c = AppConfig::default();
    c.db_password = fixture_pw();
    c.save_to(&cfg(&hms)).expect("first save");
    let key1 = std::fs::read(entropy_key(&hms)).unwrap();

    c.clinic_name = "Second Save Clinic".into();
    c.save_to(&cfg(&hms)).expect("second save");
    let key2 = std::fs::read(entropy_key(&hms)).unwrap();
    assert_eq!(key1, key2, "entropy.key must never rotate after creation");

    // No temp-key residue from either save.
    let residue: Vec<String> = std::fs::read_dir(&hms)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().contains("key.tmp"))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        residue.is_empty(),
        "entropy temp files must be renamed away, found: {:?}",
        residue
    );
}

/// F-01 live-config conformance: the PRODUCTION config at
/// C:\ProgramData\HMS\config.json is v2 (written by the deployed binary).
/// When the new binary's first save migrates it, the migration MUST go
/// through the same path pinned above. This test guards against accidental
/// early migration of the live file by the test suite itself: it only
/// READS. (The live migration happens when the new installer's app first
/// saves — tracked in the release checklist.)
#[test]
fn rctf_f01_live_config_stays_v2_until_new_binary_ships() {
    let Ok(live) = std::fs::read_to_string("C:/ProgramData/HMS/config.json") else {
        return; // Not on the production machine (e.g. CI) — skip silently.
    };
    let json: serde_json::Value = serde_json::from_str(&live).unwrap();
    let v = json["config_version"].as_u64().unwrap_or(1);
    assert!(
        v <= 2,
        "the deployed binary only understands v1/v2 — the live config must \
         stay <=2 until the new installer ships and migrates it"
    );
}

/// F-01 MIGRATION REHEARSAL against the REAL production config BYTES —
/// on a COPY (the live file must never be touched by tests; the deployed
/// binary could not read a v3 file). This is the exact pipeline the new
/// installer's app will run on first save:
///   copy → load (v2 blob decrypts through the real DPAPI machine scope)
///   → save (migrates to v3 + creates entropy.key + writes the v2 .bak)
///   → reload (round-trips the password).
/// The password value is NEVER printed or asserted literally — only that
/// it decrypts to the same non-empty string before and after.
#[test]
fn rctf_f01_live_v2_copy_migration_rehearsal() {
    let Ok(live_bytes) = std::fs::read("C:/ProgramData/HMS/config.json") else {
        return; // Not on the production machine (e.g. CI) — skip silently.
    };

    // F-11: the production entropy.key now legitimately exists on this
    // machine (backup encryption derives its AES key from the install
    // entropy). Snapshot its bytes so the rehearsal can assert it was
    // neither created, replaced, nor rotated — the migration must stay
    // inside the temp copy's directory.
    let entropy_before: Option<Vec<u8>> =
        std::fs::read("C:/ProgramData/HMS/entropy.key").ok();

    // ── Stage the copy in an isolated temp dir (unique per run). ──
    let dir = std::env::temp_dir().join(format!(
        "hms_f01_rehearsal_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let copy_path = dir.join("config.json");
    std::fs::write(&copy_path, &live_bytes).unwrap();

    // ── Load: the REAL blob (written by the deployed binary) must decrypt
    // through the legacy no-entropy path. db_password is never printed.
    let before = AppConfig::load_from(&copy_path).expect("live copy must load");
    assert_eq!(
        before.config_version, 3,
        "marked for the v3 upgrade in memory"
    );
    assert!(
        !before.db_password.is_empty(),
        "the real deployed blob failed to decrypt — migration would brick \
         the deployment; DO NOT ship until understood"
    );

    // ── Migrate: the exact save the new app performs on first run.
    before
        .save_to(&copy_path)
        .expect("migration save on the copy");

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&copy_path).unwrap()).unwrap();
    assert_eq!(json["config_version"].as_u64().unwrap(), 3);
    assert!(
        json.get("db_password_encrypted").is_some(),
        "migrated copy must carry the entropy-protected blob"
    );
    assert!(dir.join("entropy.key").exists(), "entropy.key created");
    assert!(dir.join("config.json.bak").exists(), "v2 .bak written");

    // The .bak is the v2 shape and still decrypts (both recovery paths).
    let bak_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("config.json.bak")).unwrap())
            .unwrap();
    assert_eq!(bak_json["config_version"].as_u64().unwrap(), 2);
    let from_bak =
        AppConfig::load_from(&dir.join("config.json.bak")).expect(".bak must be loadable");
    assert_eq!(
        from_bak.db_password, before.db_password,
        ".bak must recover the same password"
    );

    // ── Reload the migrated copy: the password round-trips through
    // entropy-protected v3.
    let after = AppConfig::load_from(&copy_path).expect("reload migrated copy");
    assert_eq!(after.config_version, 3);
    assert_eq!(
        after.db_password, before.db_password,
        "the REAL superuser password must survive the v2→v3 migration"
    );

    // ── The live file is UNTOUCHED (still exactly the bytes we read).
    let live_after = std::fs::read("C:/ProgramData/HMS/config.json").unwrap();
    assert_eq!(
        live_after, live_bytes,
        "the rehearsal must never modify the production config"
    );
    // The production entropy key is neither created nor rotated by the
    // rehearsal (the temp copy gets its OWN key inside its own dir).
    let entropy_after: Option<Vec<u8>> =
        std::fs::read("C:/ProgramData/HMS/entropy.key").ok();
    assert_eq!(
        entropy_before, entropy_after,
        "the rehearsal must not create, replace, or rotate the production entropy key"
    );

    std::fs::remove_dir_all(&dir).ok();
}
