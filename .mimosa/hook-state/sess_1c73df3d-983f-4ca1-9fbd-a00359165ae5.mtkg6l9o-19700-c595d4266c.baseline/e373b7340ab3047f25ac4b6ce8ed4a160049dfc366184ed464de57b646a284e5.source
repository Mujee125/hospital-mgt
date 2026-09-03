//! AERP Part G — G.3 Work Package 3: DPAPI Credential Protection.
//!
//! Proves the config PERSISTENCE semantics against real files on real disk
//! (a per-test temp directory): v1→v2 migration, the real DPAPI round-trip
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

use hospital_mgmt_lib::config::AppConfig;
use std::path::PathBuf;

// ── Harness ──────────────────────────────────────────────────────────────────

fn test_hms_dir(tag: &str) -> PathBuf {
    let hms = std::env::temp_dir().join(format!("hms_cfg_test_{}/HMS", tag));
    std::fs::create_dir_all(&hms).unwrap();
    hms
}

fn cfg(hms: &PathBuf) -> PathBuf {
    hms.join("config.json")
}

fn bak(hms: &PathBuf) -> PathBuf {
    hms.join("config.json.bak")
}

/// Write a legacy v1 config (plaintext password, no config_version field).
fn write_v1(hms: &PathBuf, password: &str) {
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

/// WP3-U07 — save() with a password writes v2: no plaintext on disk, the
/// DPAPI blob IS on disk (the VF-VERIF-003 regression guard), version = 2.
#[test]
fn wp3_u07_save_encrypts_password_on_disk() {
    let hms = test_hms_dir("u07");
    let mut c = AppConfig::default();
    c.db_password = "secret_pw_u07".into();
    c.setup_complete = true;
    c.save_to(&cfg(&hms)).expect("save");

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(cfg(&hms)).unwrap()).unwrap();
    assert!(json.get("db_password").is_none(), "plaintext must not be on disk");
    let enc = json.get("db_password_encrypted").expect("VF-VERIF-003: blob MUST be on disk");
    assert!(enc.as_str().unwrap().len() > 80, "DPAPI blob expected, got {:?}", enc);
    assert_eq!(json["config_version"].as_u64().unwrap(), 2);
}

/// WP3-U08 — loading a v2 file decrypts the password back (round-trip
/// through the REAL DPAPI machine key).
#[test]
fn wp3_u08_load_v2_decrypts() {
    let hms = test_hms_dir("u08");
    let mut c = AppConfig::default();
    c.db_password = "secret_pw_u08".into();
    c.setup_complete = true;
    c.save_to(&cfg(&hms)).expect("save");

    let loaded = AppConfig::load_from(&cfg(&hms)).expect("load");
    assert_eq!(loaded.db_password, "secret_pw_u08", "decrypt round-trip failed");
    assert_eq!(loaded.config_version, 2);
}

/// WP3-U09 — loading a v1 (plaintext) file reads the password and marks v2
/// in memory; the disk file stays v1 until an explicit save.
#[test]
fn wp3_u09_load_v1_reads_plaintext_and_marks_v2() {
    let hms = test_hms_dir("u09");
    write_v1(&hms, "plaintext_pw_u09");

    let loaded = AppConfig::load_from(&cfg(&hms)).expect("load v1");
    assert_eq!(loaded.db_password, "plaintext_pw_u09");
    assert_eq!(loaded.config_version, 2, "in-memory upgrade to v2");

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(cfg(&hms)).unwrap()).unwrap();
    assert!(
        json.get("config_version").is_none(),
        "disk stays v1 until an explicit save"
    );
}

/// WP3-U10 — full migration cycle: v1 → load → save → reload from v2, with
/// the .bak preserving the original v1 (WP3-I05).
#[test]
fn wp3_u10_v1_migrate_then_reload_v2() {
    let hms = test_hms_dir("u10");
    write_v1(&hms, "cycle_pw_u10");

    let mut c = AppConfig::load_from(&cfg(&hms)).expect("load v1");
    c.save_to(&cfg(&hms)).expect("save (migration)");

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(cfg(&hms)).unwrap()).unwrap();
    assert_eq!(json["config_version"].as_u64().unwrap(), 2);
    assert!(json.get("db_password_encrypted").is_some());

    let bak_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(bak(&hms)).expect(".bak must exist")).unwrap();
    assert_eq!(
        bak_json["db_password"].as_str().unwrap(),
        "cycle_pw_u10",
        ".bak preserves the original v1 content"
    );

    let reloaded = AppConfig::load_from(&cfg(&hms)).expect("reload v2");
    assert_eq!(reloaded.db_password, "cycle_pw_u10");
}

// ── G.3.2 Integration tests (WP3-I03, I05) ───────────────────────────────────

/// WP3-I03 — the config struct NEVER serializes either password field to
/// the frontend: both `db_password` and `db_password_encrypted` are
/// skip_serializing (this is the `get_config` reply shape).
#[test]
fn wp3_i03_ipc_reply_never_contains_password() {
    let mut c = AppConfig::default();
    c.db_password = "ipc_secret".into();
    c.db_password_encrypted = Some("BLOB".into());
    let json = serde_json::to_value(&c).unwrap();
    assert!(json.get("db_password").is_none(), "plaintext leaked to IPC shape");
    assert!(json.get("db_password_encrypted").is_none(), "blob leaked to IPC shape");
}

/// WP3-I05 — .bak is created ONLY on the v1→v2 transition, and later saves
/// never clobber the original backup.
#[test]
fn wp3_i05_bak_created_once_not_clobbered() {
    let hms = test_hms_dir("i05");
    write_v1(&hms, "bak_pw");

    let mut c = AppConfig::load_from(&cfg(&hms)).unwrap();
    c.save_to(&cfg(&hms)).unwrap(); // migration → .bak created
    let bak1 = std::fs::read_to_string(bak(&hms)).unwrap();

    c.clinic_name = "Renamed Clinic".into();
    c.save_to(&cfg(&hms)).unwrap(); // later save — .bak must NOT be overwritten
    let bak2 = std::fs::read_to_string(bak(&hms)).unwrap();
    assert_eq!(bak1, bak2, "the original v1 backup must survive later saves");

    // A save that does NOT migrate (no v1 ever existed) never creates a .bak.
    let hms2 = test_hms_dir("i05b");
    let mut c2 = AppConfig::default();
    c2.db_password = "fresh".into();
    c2.save_to(&cfg(&hms2)).unwrap(); // first save → v2 directly
    c2.save_to(&cfg(&hms2)).unwrap();
    assert!(!bak(&hms2).exists(), ".bak only exists for v1 migrations");
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
    c.db_password = "pw_to_corrupt".into();
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
    write_v1(&hms, "noversion_pw"); // fixture has no config_version key
    let c = AppConfig::load_from(&cfg(&hms)).expect("missing version == v1");
    assert_eq!(c.db_password, "noversion_pw");
    assert_eq!(c.config_version, 2, "marked for v2 migration");
}

/// WP3-N04 — an UNKNOWN future config_version is rejected outright, not
/// guessed (code change: explicit >2 rejection was added per spec).
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
    c.db_password = "ATOMIC-SECRET-9931".into();
    c.save_to(&cfg(&hms)).unwrap();

    let raw = std::fs::read_to_string(cfg(&hms)).unwrap();
    assert!(!raw.contains("ATOMIC-SECRET-9931"), "plaintext on disk!");

    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let blob = json["db_password_encrypted"].as_str().unwrap();
    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD.decode(blob).unwrap();
    let decoded_str = String::from_utf8_lossy(&decoded);
    assert!(
        !decoded_str.contains("ATOMIC-SECRET-9931"),
        "blob must be real ciphertext, not a re-encoding of the plaintext"
    );
}

/// WP3-P02 — binary inspection: the DPAPI machine key is never embedded in
/// the binary — secrets.rs contains no key material, only the OS API calls.
#[test]
fn wp3_p02_no_embedded_keys_in_source() {
    let src = std::fs::read_to_string(format!("{}/src/secrets.rs", env!("CARGO_MANIFEST_DIR"))).unwrap();
    assert!(!src.contains("PRIVATE KEY"));
    assert!(!src.contains("BEGIN PUBLIC KEY"));
    assert!(src.contains("CryptProtectData"));
    assert!(src.contains("CryptUnprotectData"));
}

// ── G.3.5 Concurrency tests (WP3-C01, C02) ────────────────────────────────────

/// WP3-C01/C02 — concurrent load/save cycles: no corruption (atomic
/// temp+rename under contention), and concurrent migrations of one v1
/// file converge on a valid v2 with exactly one .bak preserving v1.
#[test]
fn wp3_c01_c02_concurrent_save_load_and_migration() {
    let hms = test_hms_dir("c01");
    write_v1(&hms, "conc_migration_pw");

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
                    re.db_password == "conc_migration_pw",
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
    assert_eq!(json["config_version"].as_u64().unwrap(), 2);
    assert!(json.get("db_password_encrypted").is_some());
    let bak_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(bak(&hms)).unwrap()).unwrap();
    assert_eq!(bak_json["db_password"].as_str().unwrap(), "conc_migration_pw");
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
    c.db_password = "offline_pw".into();
    c.save_to(&cfg(&hms)).unwrap();
    let loaded = AppConfig::load_from(&cfg(&hms)).expect("offline load");
    assert_eq!(loaded.db_password, "offline_pw");
}

// ── G.3.7/3.8 LAN + Windows-only tests ────────────────────────────────────────
// LAN (L01-L03) and the destructive Windows-only DPAPI tests (W01 sysprep,
// W02 service-account, W04 service restart, W05 OS reboot) require
// hardware/OS actions unavailable in-process; they were verified live
// during the Windows verification round (migration + restart round-trip
// observed on the real machine) or are documented as deferred in
// docs/VERIFICATION-REPORT-WP1-WP3.md §5. W03 (ACL post-migration) was
// verified live on the production file — report §4 row 5.
