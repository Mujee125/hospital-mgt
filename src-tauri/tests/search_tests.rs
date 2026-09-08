//! Phase 10 — RBAC-scoped global search integration tests.
//!
//! Exercises `global_search_core` at command level against the real
//! PostgreSQL test DB. The titlebar search's core contract:
//!
//!   SST-1  Section scoping follows the signed-in user's permissions exactly:
//!          a hit from a table appears only if the user holds that section's
//!          view permission (server-enforced, not UI-convention).
//!   SST-2  LIKE wildcards in the user's term are neutralized: "%" and "_"
//!          match literally instead of widening the pattern.
//!   SST-3  Soft-deleted patients are excluded from the patient section
//!          (CR-11 / HIPAA retention posture).
//!   SST-4  < 2 non-space characters returns an empty result, never an error.
//!
//! Requires: HMS_TEST_DB_URL env var (maintenance DB) + `--features
//! hms-integration-tests`.

#![cfg(feature = "hms-integration-tests")]

mod common;

use common::*;
use hospital_mgmt_lib::commands::search::{global_search_core, GlobalSearchHit};
use hospital_mgmt_lib::rbac::SessionState;
use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

async fn state_for(pool: &PgPool, user_id: i32, token_hash: &str) -> SessionState {
    let s = load_session_for(pool, user_id, token_hash).await;
    Arc::new(Mutex::new(Some(s)))
}

fn types_of(hits: &[GlobalSearchHit]) -> HashSet<String> {
    hits.iter().map(|h| h.entity_type.clone()).collect()
}

/// Seed the zqx fixtures exactly once per test process (the suite's tests
/// share the per-process DB; unique keys like SKU-ZQX would collide on a
/// second seed). Same rationale as common's DB_PROVISIONED — the closure
/// only runs on the first caller's runtime, so the captured pool is never
/// used from another runtime.
static ZQX_SEEDED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

async fn seed_zqx_once(pool: &PgPool) {
    ZQX_SEEDED
        .get_or_init(|| async {
            seed_zqx_everywhere(pool).await;
        })
        .await;
}

/// Seed one hit-able entity of each of the TWELVE sections, all matching
/// the unique token `zqx`, so a single search sees every section the ROLE
/// allows.
struct ZqxFixtures {
    _patient_id: i32,
    _doctor_id: i32,
}

async fn seed_zqx_everywhere(pool: &PgPool) -> ZqxFixtures {
    let patient_id = seed_patient_with_phone(pool, "Zqx", "Uniquepat", "+92300999001").await;
    let doctor_id: i32 = {
        let (id,): (i32,) = sqlx::query_as(
            "INSERT INTO doctors (first_name, last_name, phone, specialization, qualification) \
             VALUES ('Zqx', 'Uniquedoc', '+92300999002', 'Cardiology', 'MBBS') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        id
    };
    sqlx::query(
        "INSERT INTO inventory_items (name, sku, category) VALUES ('Zqx Uniqueitem', 'SKU-ZQX', 'medication')",
    )
    .execute(pool)
    .await
    .unwrap();
    let (bill_id,): (i32,) = sqlx::query_as(
        "INSERT INTO bills (patient_id, bill_number, status) VALUES ($1, $2, 'draft') RETURNING id",
    )
    .bind(patient_id)
    .bind("INV-ZQX-000001")
    .fetch_one(pool)
    .await
    .unwrap();
    let _ = bill_id;
    let (order_id,): (i32,) = sqlx::query_as(
        "INSERT INTO lab_orders (patient_id, status) VALUES ($1, 'ordered') RETURNING id",
    )
    .bind(patient_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let _ = order_id;
    let (appt_id,): (i32,) = sqlx::query_as(
        "INSERT INTO appointments (patient_id, doctor_id, appointment_date, appointment_time) \
         VALUES ($1, $2, CURRENT_DATE, '09:00') RETURNING id",
    )
    .bind(patient_id)
    .bind(doctor_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let _ = appt_id;

    // Pharmacy catalog + a prescription with one medication line.
    sqlx::query(
        "INSERT INTO medications (brand_name, generic_name, strength) \
         VALUES ('Zqxin', 'zqx-generic', '10mg')",
    )
    .execute(pool)
    .await
    .unwrap();
    let (rx_id,): (i32,) = sqlx::query_as(
        "INSERT INTO prescriptions (patient_id, doctor_id, status) VALUES ($1, $2, 'active') RETURNING id",
    )
    .bind(patient_id)
    .bind(doctor_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO prescription_items (prescription_id, medication_name, dose, frequency) \
         VALUES ($1, 'Zqxin', '1 tablet', 'twice daily')",
    )
    .bind(rx_id)
    .execute(pool)
    .await
    .unwrap();

    // Radiology order.
    sqlx::query(
        "INSERT INTO radiology_orders (patient_id, order_number, study_type, priority) \
         VALUES ($1, 'RAD-ZQX-0001', 'x-ray', 'routine')",
    )
    .bind(patient_id)
    .execute(pool)
    .await
    .unwrap();

    // IPD admission (needs a ward + bed to satisfy the NOT NULL FKs).
    let (ward_id,): (i32,) = sqlx::query_as(
        "INSERT INTO wards (name, code) VALUES ('ZQX Ward', $1) RETURNING id",
    )
    .bind(format!("ZQX{}", std::process::id()))
    .fetch_one(pool)
    .await
    .unwrap();
    let (bed_id,): (i32,) = sqlx::query_as(
        "INSERT INTO beds (ward_id, bed_number) VALUES ($1, 'ZQX-01') RETURNING id",
    )
    .bind(ward_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ipd_admissions (patient_id, doctor_id, ward_id, bed_id, admitting_diagnosis) \
         VALUES ($1, $2, $3, $4, 'zqx fever')",
    )
    .bind(patient_id)
    .bind(doctor_id)
    .bind(ward_id)
    .bind(bed_id)
    .execute(pool)
    .await
    .unwrap();

    // Blood donor.
    sqlx::query(
        "INSERT INTO blood_donors (donor_number, first_name, last_name, blood_group, rh_factor) \
         VALUES ('DON-ZQX-0001', 'Zqx', 'Uniquedonor', 'O', '+')",
    )
    .execute(pool)
    .await
    .unwrap();

    // A user account (distinctive full name; username is unique per run).
    let uname = format!("zqx_user_{}", std::process::id());
    sqlx::query(
        "INSERT INTO users (username, full_name, password_hash, must_change_password) \
         VALUES ($1, 'Zqx Uniquuser', 'x', FALSE)",
    )
    .bind(&uname)
    .execute(pool)
    .await
    .unwrap();

    ZqxFixtures { _patient_id: patient_id, _doctor_id: doctor_id }
}

// ── SST-1: sections follow the signed-in user's permissions ─────────────────

#[tokio::test]
async fn test_sst1_rbac_sections() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    seed_zqx_once(&pool).await;

    // Billing clerk: patients + invoices + appointments. No doctors,
    // inventory, or lab sections (their view permissions are absent).
    let clerk = seed_user(&pool, "sst_clerk", &pw, &["billing_clerk"]).await;
    seed_session_row(&pool, clerk, "hash_sst_clerk").await;
    let clerk_state = state_for(&pool, clerk, "hash_sst_clerk").await;
    let hits = global_search_core(&pool, &clerk_state, "zqx".to_string()).await.unwrap();
    let t = types_of(&hits);
    assert!(t.contains("patient"), "clerk: patient hit expected, got {:?}", t);
    assert!(t.contains("invoice"), "clerk: invoice hit expected, got {:?}", t);
    assert!(t.contains("appointment"), "clerk: appointment hit expected, got {:?}", t);
    assert!(!t.contains("doctor"), "clerk must NOT see doctor hits: {:?}", t);
    assert!(!t.contains("inventory_item"), "clerk must NOT see inventory hits: {:?}", t);
    assert!(!t.contains("lab_order"), "clerk must NOT see lab order hits: {:?}", t);

    // Pharmacist: patients + invoices + inventory. No doctors, appointments,
    // or lab orders.
    let pharm = seed_user(&pool, "sst_pharm", &pw, &["pharmacist"]).await;
    seed_session_row(&pool, pharm, "hash_sst_pharm").await;
    let pharm_state = state_for(&pool, pharm, "hash_sst_pharm").await;
    let hits = global_search_core(&pool, &pharm_state, "zqx".to_string()).await.unwrap();
    let t = types_of(&hits);
    assert!(t.contains("patient") && t.contains("invoice") && t.contains("inventory_item"),
        "pharmacist: patient/invoice/inventory expected, got {:?}", t);
    assert!(!t.contains("doctor"), "pharmacist must NOT see doctor hits: {:?}", t);
    assert!(!t.contains("appointment"), "pharmacist must NOT see appointment hits: {:?}", t);
    assert!(!t.contains("lab_order"), "pharmacist must NOT see lab order hits: {:?}", t);

    // Doctor: patients + doctors + lab orders + appointments + invoices +
    // inventory — the seeded doctor role holds all six view permissions
    // (BillingView and InventoryView are in its set), so it sees every
    // section. Positives only; negatives are covered by clerk/pharmacist/lab
    // tech below.
    let doc = seed_user(&pool, "sst_doc", &pw, &["doctor"]).await;
    seed_session_row(&pool, doc, "hash_sst_doc").await;
    let doc_state = state_for(&pool, doc, "hash_sst_doc").await;
    let hits = global_search_core(&pool, &doc_state, "zqx".to_string()).await.unwrap();
    let t = types_of(&hits);
    assert!(
        t.contains("patient") && t.contains("doctor") && t.contains("lab_order")
            && t.contains("invoice") && t.contains("inventory_item"),
        "doctor: all six sections expected, got {:?}",
        t
    );

    // Lab technician: patients + lab orders + inventory, but NO doctors,
    // appointments, or invoices (the seeded role lacks those view perms).
    let tech = seed_user(&pool, "sst_tech", &pw, &["lab_technician"]).await;
    seed_session_row(&pool, tech, "hash_sst_tech").await;
    let tech_state = state_for(&pool, tech, "hash_sst_tech").await;
    let hits = global_search_core(&pool, &tech_state, "zqx".to_string()).await.unwrap();
    let t = types_of(&hits);
    assert!(t.contains("patient") && t.contains("lab_order") && t.contains("inventory_item"),
        "lab tech: patient/lab_order/inventory expected, got {:?}", t);
    assert!(!t.contains("doctor"), "lab tech must NOT see doctor hits: {:?}", t);
    assert!(!t.contains("appointment"), "lab tech must NOT see appointment hits: {:?}", t);
    assert!(!t.contains("invoice"), "lab tech must NOT see invoice hits: {:?}", t);
}

// ── SST-5: pharmacy sections (prescriptions / medications) ───────────────────
//
// Prescriptions are guarded by patients.view (the same permission
// get_prescriptions uses); the pharmacy catalog (medications) by
// inventory.view (the same permission get_medications uses).

#[tokio::test]
async fn test_sst5_pharmacy_sections() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    seed_zqx_once(&pool).await;

    // Billing clerk holds PatientsView but NOT InventoryView: prescriptions
    // visible, medications NOT.
    let clerk = seed_user(&pool, "sst5_clerk", &pw, &["billing_clerk"]).await;
    seed_session_row(&pool, clerk, "hash_sst5_clerk").await;
    let clerk_state = state_for(&pool, clerk, "hash_sst5_clerk").await;
    let hits = global_search_core(&pool, &clerk_state, "zqx".to_string()).await.unwrap();
    let t = types_of(&hits);
    assert!(t.contains("prescription"), "clerk: prescription hit expected, got {:?}", t);
    assert!(!t.contains("medication"), "clerk must NOT see medication hits: {:?}", t);

    // Doctor holds both PatientsView and InventoryView: both pharmacy
    // sections visible, and the prescription matches BOTH via the patient
    // name and the medication name on its lines.
    let doc = seed_user(&pool, "sst5_doc", &pw, &["doctor"]).await;
    seed_session_row(&pool, doc, "hash_sst5_doc").await;
    let doc_state = state_for(&pool, doc, "hash_sst5_doc").await;
    let hits = global_search_core(&pool, &doc_state, "zqx".to_string()).await.unwrap();
    let t = types_of(&hits);
    assert!(t.contains("prescription") && t.contains("medication"),
        "doctor: prescription + medication expected, got {:?}", t);
}

// ── SST-6: every section of the app, via the one role that holds them all ────
//
// Super admin holds every permission — one query must surface all twelve
// sections, proving the search covers the whole app end to end.

#[tokio::test]
async fn test_sst6_whole_app_coverage() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    seed_zqx_once(&pool).await;

    let admin = seed_user(&pool, "sst6_admin", &pw, &["super_admin"]).await;
    seed_session_row(&pool, admin, "hash_sst6_admin").await;
    let admin_state = state_for(&pool, admin, "hash_sst6_admin").await;
    let hits = global_search_core(&pool, &admin_state, "zqx".to_string()).await.unwrap();
    let t = types_of(&hits);

    let expected = [
        "patient", "doctor", "appointment", "invoice", "lab_order",
        "inventory_item", "prescription", "medication", "radiology_order",
        "ipd_admission", "blood_donor", "user",
    ];
    for e in expected {
        assert!(t.contains(e), "super admin must see '{}' hits, got {:?}", e, t);
    }
}

// ── SST-2: LIKE wildcards match literally ────────────────────────────────────

#[tokio::test]
async fn test_sst2_wildcards_escaped() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let nurse = seed_user(&pool, "sst2_nurse", &pw, &["nurse"]).await;
    seed_session_row(&pool, nurse, "hash_sst2_nurse").await;
    let nurse_state = state_for(&pool, nurse, "hash_sst2_nurse").await;

    // One patient whose name contains a literal underscore; none with a
    // literal percent anywhere.
    let _under = seed_patient_with_phone(&pool, "Under", "Score_zqxw", "+92300999003").await;

    // "_z" must match ONLY rows containing a literal underscore before z —
    // not act as the any-single-char wildcard. (Queries are ≥2 chars: the
    // backend's min-length guard would empty out a single "_" before the
    // LIKE ever runs, so the wildcard has to ride along with a literal.)
    let hits = global_search_core(&pool, &nurse_state, "_z".to_string()).await.unwrap();
    assert!(!hits.is_empty(), "literal '_z' name must be found");
    for h in &hits {
        assert!(
            h.title.contains("_z"),
            "wildcard leaked: '_' matched non-literal row {:?}",
            h.title
        );
    }

    // "%q" must match nothing (no fixture contains a literal percent). If
    // the escape were broken, the pattern would degenerate to "contains q"
    // and match the seeded patient.
    let hits = global_search_core(&pool, &nurse_state, "%q".to_string()).await.unwrap();
    assert!(hits.is_empty(), "'%' must match literally, got {:?}", hits.len());
}

// ── SST-3: soft-deleted patients stay out of the patient section ────────────

#[tokio::test]
async fn test_sst3_soft_deleted_excluded() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let clerk = seed_user(&pool, "sst3_clerk", &pw, &["billing_clerk"]).await;
    seed_session_row(&pool, clerk, "hash_sst3_clerk").await;
    let clerk_state = state_for(&pool, clerk, "hash_sst3_clerk").await;

    let pid = seed_patient_with_phone(&pool, "Zqxd", "Softdel", "+92300999004").await;
    sqlx::query("UPDATE patients SET deleted_at = NOW() WHERE id = $1")
        .bind(pid)
        .execute(&pool)
        .await
        .unwrap();

    let hits = global_search_core(&pool, &clerk_state, "zqxd".to_string()).await.unwrap();
    assert!(
        hits.iter().all(|h| h.entity_type != "patient"),
        "soft-deleted patient must not appear in the patient section: {:?}",
        hits.iter().map(|h| &h.title).collect::<Vec<_>>()
    );
}

// ── SST-4: too-short queries return empty, never an error ────────────────────

#[tokio::test]
async fn test_sst4_min_length_contract() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let clerk = seed_user(&pool, "sst4_clerk", &pw, &["billing_clerk"]).await;
    seed_session_row(&pool, clerk, "hash_sst4_clerk").await;
    let clerk_state = state_for(&pool, clerk, "hash_sst4_clerk").await;

    for q in ["z", "  ", ""] {
        let hits = global_search_core(&pool, &clerk_state, q.to_string()).await.unwrap();
        assert!(hits.is_empty(), "'{:?}' must return empty, got {}", q, hits.len());
    }

    // Not signed in → hard error (the session guard, not the length guard).
    let anon: SessionState = Arc::new(Mutex::new(None));
    let err = global_search_core(&pool, &anon, "zqx".to_string()).await.unwrap_err();
    assert!(err.contains("not signed in"), "expected session error, got: {}", err);
}
