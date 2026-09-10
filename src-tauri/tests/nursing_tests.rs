//! Phase 6.1 — Nursing Station command-level integration tests.
//!
//! Exercises the REAL production cores (`record_vitals_core`,
//! `create_nurse_note_core`, `record_medication_administration_core`)
//! against the real PostgreSQL test DB — the exact functions the Tauri
//! commands call. Read commands are covered indirectly via direct SQL
//! assertions on what the cores wrote.
//!
//! Covers the four risk surfaces of SRS §2.7:
//!   NS-1  RBAC boundary — receptionists (no IpdManage) must be rejected
//!        at the permission guard; nurses must pass it.
//!   NS-2  Vitals validation — at least one vital required; vitals on a
//!        discharged admission must be rejected (data hygiene for the
//!        7-reading trend view).
//!   NS-3  MAR cross-patient guard — a prescription item belonging to a
//!        DIFFERENT patient must not be recordable against an admission
//!        (tampered-frontend defense).
//!   NS-4  Note-type validation — only shift/observation/handover.
//!
//! Requires: HMS_TEST_DB_URL env var + `--features hms-integration-tests`.

#![cfg(feature = "hms-integration-tests")]

mod common;

use common::*;
use hospital_mgmt_lib::commands::ipd::discharge_patient_core;
use hospital_mgmt_lib::commands::nursing::{
    create_nurse_note_core, record_medication_administration_core, record_vitals_core,
    CreateNurseNoteRequest, RecordAdministrationRequest, RecordVitalsRequest,
};
use hospital_mgmt_lib::models::DischargeIpd;
use hospital_mgmt_lib::rbac::SessionState;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};

// ── Local fixtures (ward / bed / admission / prescription) ────────────────────

/// Insert a ward + bed and return (ward_id, bed_id). Unique per call.
async fn seed_ward_and_bed(pool: &PgPool, suffix: &str) -> (i32, i32) {
    let ward: (i32,) =
        sqlx::query_as("INSERT INTO wards (name, code) VALUES ($1, $1) RETURNING id")
            .bind(format!("Ward-{}", suffix))
            .fetch_one(pool)
            .await
            .expect("seed ward");
    let bed: (i32,) = sqlx::query_as(
        "INSERT INTO beds (ward_id, bed_number, status) VALUES ($1, $2, 'available') RETURNING id",
    )
    .bind(ward.0)
    .bind(format!("B-{}", suffix))
    .fetch_one(pool)
    .await
    .expect("seed bed");
    (ward.0, bed.0)
}

/// Insert an admission (default status 'admitted') and return its id.
async fn seed_admission(pool: &PgPool, patient_id: i32, bed_id: i32, status: &str) -> i32 {
    let ward: (i32,) = sqlx::query_as("SELECT ward_id FROM beds WHERE id = $1")
        .bind(bed_id)
        .fetch_one(pool)
        .await
        .expect("find ward for bed");
    let row: (i32,) = sqlx::query_as(
        "INSERT INTO ipd_admissions (patient_id, ward_id, bed_id, status) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(patient_id)
    .bind(ward.0)
    .bind(bed_id)
    .bind(status)
    .fetch_one(pool)
    .await
    .expect("seed admission");
    // Occupied bed while admitted — mirrors admit_patient_core's update.
    if status == "admitted" {
        sqlx::query("UPDATE beds SET status = 'occupied' WHERE id = $1")
            .bind(bed_id)
            .execute(pool)
            .await
            .expect("occupy bed");
    }
    row.0
}

/// Insert a prescription + one item for a patient; return the item id.
async fn seed_prescription_item(pool: &PgPool, patient_id: i32, med_name: &str) -> i32 {
    let rx: (i32,) = sqlx::query_as(
        "INSERT INTO prescriptions (patient_id, status) VALUES ($1, 'active') RETURNING id",
    )
    .bind(patient_id)
    .fetch_one(pool)
    .await
    .expect("seed prescription");
    let item: (i32,) = sqlx::query_as(
        "INSERT INTO prescription_items \
         (prescription_id, medication_id, medication_name, dose, route, frequency, quantity, is_controlled) \
         VALUES ($1, NULL, $2, '500mg', 'oral', 'BD', 10, FALSE) RETURNING id",
    )
    .bind(rx.0)
    .bind(med_name)
    .fetch_one(pool)
    .await
    .expect("seed prescription item");
    item.0
}

async fn state_for(pool: &PgPool, user_id: i32, token_hash: &str) -> SessionState {
    let s = load_session_for(pool, user_id, token_hash).await;
    Arc::new(Mutex::new(Some(s)))
}

// ── NS-1: RBAC boundary ───────────────────────────────────────────────────────

/// Receptionists hold PatientsCreate (front desk) but NOT IpdManage — every
/// nursing write must be rejected at the permission guard, with the exact
/// permission name in the error (same assertion style as the
/// receptionist-cannot-prescribe regression in session_tests.rs).
#[tokio::test]
async fn test_ns1_receptionist_cannot_record_vitals() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let nurse_id = seed_user(&pool, "ns1_nurse", &pw, &["nurse"]).await;
    let rx_id = seed_user(&pool, "ns1_rx", &pw, &["receptionist"]).await;
    seed_session_row(&pool, nurse_id, "hash_ns1_nurse").await;
    seed_session_row(&pool, rx_id, "hash_ns1_rx").await;
    let nurse_state = state_for(&pool, nurse_id, "hash_ns1_nurse").await;
    let rx_state = state_for(&pool, rx_id, "hash_ns1_rx").await;

    let patient_id = seed_patient_with_phone(&pool, "Nia", "Vitals", "+92300001").await;
    let (_, bed_id) = seed_ward_and_bed(&pool, "ns1").await;
    let admission_id = seed_admission(&pool, patient_id, bed_id, "admitted").await;

    let req = RecordVitalsRequest {
        admission_id,
        temperature_c: Some(37.5),
        systolic_bp: Some(120),
        diastolic_bp: Some(80),
        pulse_bpm: Some(72),
        resp_rate: None,
        spo2_pct: Some(98),
        pain_score: None,
        notes: None,
    };

    let err = record_vitals_core(&pool, &rx_state, req.clone())
        .await
        .unwrap_err();
    assert!(
        err.contains("requires the 'ipd.manage'"),
        "receptionist must be denied by the IpdManage guard, got: {}",
        err
    );

    // Nurse: full success through the production path, row persisted with
    // attribution to the nurse's user id.
    let id = record_vitals_core(&pool, &nurse_state, req)
        .await
        .expect("nurse must be able to record vitals");
    let stored: (Option<i32>,) =
        sqlx::query_as("SELECT recorded_by_user_id FROM vitals WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        stored.0,
        Some(nurse_id),
        "vitals row must be attributed to the nurse"
    );
}

// ── NS-2: Vitals validation ────────────────────────────────────────────────────

/// A vitals request with every field absent must be rejected — an empty row
/// in the trend view is meaningless data.
#[tokio::test]
async fn test_ns2_vitals_requires_at_least_one_reading() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let nurse_id = seed_user(&pool, "ns2_nurse", &pw, &["nurse"]).await;
    seed_session_row(&pool, nurse_id, "hash_ns2_nurse").await;
    let nurse_state = state_for(&pool, nurse_id, "hash_ns2_nurse").await;

    let patient_id = seed_patient_with_phone(&pool, "Nsb", "Empty", "+92300002").await;
    let (_, bed_id) = seed_ward_and_bed(&pool, "ns2").await;
    let admission_id = seed_admission(&pool, patient_id, bed_id, "admitted").await;

    let err = record_vitals_core(
        &pool,
        &nurse_state,
        RecordVitalsRequest {
            admission_id,
            temperature_c: None,
            systolic_bp: None,
            diastolic_bp: None,
            pulse_bpm: None,
            resp_rate: None,
            spo2_pct: None,
            pain_score: None,
            notes: Some("note only, no readings".into()),
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("At least one vital sign"),
        "all-empty vitals must be rejected, got: {}",
        err
    );
}

/// Vitals on a DISCHARGED admission must be rejected — the trend view would
/// otherwise show post-discharge readings mixed into the stay timeline.
#[tokio::test]
async fn test_ns2_vitals_rejected_on_discharged_admission() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let nurse_id = seed_user(&pool, "ns2d_nurse", &pw, &["nurse"]).await;
    seed_session_row(&pool, nurse_id, "hash_ns2d_nurse").await;
    let nurse_state = state_for(&pool, nurse_id, "hash_ns2d_nurse").await;

    let patient_id = seed_patient_with_phone(&pool, "Nsc", "Discharged", "+92300003").await;
    let (_, bed_id) = seed_ward_and_bed(&pool, "ns2d").await;
    let admission_id = seed_admission(&pool, patient_id, bed_id, "discharged").await;

    let err = record_vitals_core(
        &pool,
        &nurse_state,
        RecordVitalsRequest {
            admission_id,
            temperature_c: Some(38.2),
            systolic_bp: None,
            diastolic_bp: None,
            pulse_bpm: None,
            resp_rate: None,
            spo2_pct: None,
            pain_score: None,
            notes: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("No active admission"),
        "vitals on discharged admission must be rejected, got: {}",
        err
    );
}

// ── NS-3: MAR cross-patient guard ─────────────────────────────────────────────

/// A prescription item belonging to a DIFFERENT patient must not be
/// recordable against an admission — the tampered-frontend defense.
#[tokio::test]
async fn test_ns3_mar_rejects_cross_patient_prescription() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let nurse_id = seed_user(&pool, "ns3_nurse", &pw, &["nurse"]).await;
    seed_session_row(&pool, nurse_id, "hash_ns3_nurse").await;
    let nurse_state = state_for(&pool, nurse_id, "hash_ns3_nurse").await;

    let admitted_patient = seed_patient_with_phone(&pool, "Nsd", "Inbed", "+92300004").await;
    let other_patient = seed_patient_with_phone(&pool, "Nse", "Other", "+92300005").await;
    let (_, bed_id) = seed_ward_and_bed(&pool, "ns3").await;
    let admission_id = seed_admission(&pool, admitted_patient, bed_id, "admitted").await;

    // Medication prescribed to the OTHER patient.
    let foreign_item = seed_prescription_item(&pool, other_patient, "Foreign Med").await;

    let err = record_medication_administration_core(
        &pool,
        &nurse_state,
        RecordAdministrationRequest {
            admission_id,
            prescription_item_id: foreign_item,
            status: "administered".into(),
            notes: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("does not belong to the patient"),
        "cross-patient MAR must be rejected, got: {}",
        err
    );

    // The matching-patient happy path: row persisted, attributed to the nurse.
    let own_item = seed_prescription_item(&pool, admitted_patient, "Own Med").await;
    let id = record_medication_administration_core(
        &pool,
        &nurse_state,
        RecordAdministrationRequest {
            admission_id,
            prescription_item_id: own_item,
            status: "held".into(),
            notes: Some("patient refused".into()),
        },
    )
    .await
    .expect("matching-patient MAR must succeed");
    let stored: (i32, String, Option<i32>) = sqlx::query_as(
        "SELECT patient_id, status, administered_by_user_id \
         FROM medication_administrations WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored.0, admitted_patient);
    assert_eq!(stored.1, "held");
    assert_eq!(stored.2, Some(nurse_id));
}

/// Invalid MAR status must be rejected before any SQL write.
#[tokio::test]
async fn test_ns3_mar_rejects_invalid_status() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let nurse_id = seed_user(&pool, "ns3s_nurse", &pw, &["nurse"]).await;
    seed_session_row(&pool, nurse_id, "hash_ns3s_nurse").await;
    let nurse_state = state_for(&pool, nurse_id, "hash_ns3s_nurse").await;

    let patient_id = seed_patient_with_phone(&pool, "Nsf", "Status", "+92300006").await;
    let (_, bed_id) = seed_ward_and_bed(&pool, "ns3s").await;
    let admission_id = seed_admission(&pool, patient_id, bed_id, "admitted").await;
    let item = seed_prescription_item(&pool, patient_id, "Status Med").await;

    let err = record_medication_administration_core(
        &pool,
        &nurse_state,
        RecordAdministrationRequest {
            admission_id,
            prescription_item_id: item,
            status: "maybe".into(),
            notes: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("administered, held, or refused"),
        "invalid MAR status must be rejected, got: {}",
        err
    );
}

// ── RCTF-FULL-SYSTEM-2026-09-08 F-22: vitals plausibility guard ───────────────

/// Implausible values — the classic missed-decimal entries (temp 370,
/// SpO2 9.8) — are refused with the expected range; plausible values at
/// the extremes still record.
#[tokio::test]
async fn rctf_f22_implausible_vitals_refused() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let nurse_id = seed_user(&pool, "f22_nurse", &pw, &["nurse"]).await;
    seed_session_row(&pool, nurse_id, "hash_f22_nurse").await;
    let nurse_state = state_for(&pool, nurse_id, "hash_f22_nurse").await;

    let patient_id = seed_patient_with_phone(&pool, "Foj", "Vitals", "+92300f22a").await;
    let (_, bed_id) = seed_ward_and_bed(&pool, "f22").await;
    let admission_id = seed_admission(&pool, patient_id, bed_id, "admitted").await;

    let vital = |temp: Option<f64>, spo2: Option<i32>| RecordVitalsRequest {
        admission_id,
        temperature_c: temp,
        systolic_bp: None,
        diastolic_bp: None,
        pulse_bpm: None,
        resp_rate: None,
        spo2_pct: spo2,
        pain_score: None,
        notes: None,
    };

    // Temperature 370 — the missed decimal point.
    let err = record_vitals_core(&pool, &nurse_state, vital(Some(370.0), None))
        .await
        .unwrap_err();
    assert!(
        err.contains("missed decimal") && err.contains("30–43"),
        "temp 370 must be refused with the range hint, got: {}",
        err
    );

    // SpO2 9.8 — the other missed decimal.
    let err = record_vitals_core(&pool, &nurse_state, vital(None, Some(9)))
        .await
        .unwrap_err();
    assert!(
        err.contains("50–100"),
        "SpO2 9 must be refused, got: {}",
        err
    );

    // Nothing was written by the refused attempts.
    let (rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM vitals WHERE admission_id = $1")
        .bind(admission_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);

    // Plausible extremes record fine: hypothermia edge (30.0) and full
    // recovery SpO2 (100).
    record_vitals_core(&pool, &nurse_state, vital(Some(30.0), Some(100)))
        .await
        .expect("plausible extreme values must record");
    let (rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM vitals WHERE admission_id = $1")
        .bind(admission_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);
}

// ── RCTF-FULL-SYSTEM-2026-09-08 F-14: MAR double-administration guard ────────

/// Two 'administered' entries for the same (admission, item) inside 15
/// minutes must be refused — the accidental double-dose scenario (two
/// nurses, or a double-tap) previously recorded silently. 'held' and
/// 'refused' may repeat, and a fresh 'administered' after the window (or
/// for a DIFFERENT item) passes.
#[tokio::test]
async fn rctf_f14_mar_double_administration_refused() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let nurse_id = seed_user(&pool, "f14_nurse", &pw, &["nurse"]).await;
    seed_session_row(&pool, nurse_id, "hash_f14_nurse").await;
    let nurse_state = state_for(&pool, nurse_id, "hash_f14_nurse").await;

    let patient_id = seed_patient_with_phone(&pool, "Fod", "Double", "+92300f14a").await;
    let (_, bed_id) = seed_ward_and_bed(&pool, "f14").await;
    let admission_id = seed_admission(&pool, patient_id, bed_id, "admitted").await;
    let item = seed_prescription_item(&pool, patient_id, "F14 Med").await;

    // First administration records normally.
    record_medication_administration_core(
        &pool,
        &nurse_state,
        RecordAdministrationRequest {
            admission_id,
            prescription_item_id: item,
            status: "administered".into(),
            notes: None,
        },
    )
    .await
    .expect("first administration must record");

    // The second, moments later, is refused as a likely double dose.
    let err = record_medication_administration_core(
        &pool,
        &nurse_state,
        RecordAdministrationRequest {
            admission_id,
            prescription_item_id: item,
            status: "administered".into(),
            notes: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("already recorded as administered"),
        "double administration must be refused, got: {}",
        err
    );
    // Exactly ONE administered row exists.
    let (rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM medication_administrations \
         WHERE admission_id = $1 AND prescription_item_id = $2 AND status = 'administered'",
    )
    .bind(admission_id)
    .bind(item)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rows, 1, "exactly one administered row");

    // 'held' still records inside the window (a hold is not a dose).
    record_medication_administration_core(
        &pool,
        &nurse_state,
        RecordAdministrationRequest {
            admission_id,
            prescription_item_id: item,
            status: "held".into(),
            notes: Some("NPO".into()),
        },
    )
    .await
    .expect("held entries may repeat within the window");

    // A DIFFERENT item administers fine (the guard is per-item).
    let item2 = seed_prescription_item(&pool, patient_id, "F14 Med Two").await;
    record_medication_administration_core(
        &pool,
        &nurse_state,
        RecordAdministrationRequest {
            admission_id,
            prescription_item_id: item2,
            status: "administered".into(),
            notes: None,
        },
    )
    .await
    .expect("a different item is not a double administration");
}

/// The prescription must still be ACTIVE — a dispensed/completed
/// prescription can no longer be administered against an admission.
#[tokio::test]
async fn rctf_f14_mar_refuses_inactive_prescription() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let nurse_id = seed_user(&pool, "f14b_nurse", &pw, &["nurse"]).await;
    seed_session_row(&pool, nurse_id, "hash_f14b_nurse").await;
    let nurse_state = state_for(&pool, nurse_id, "hash_f14b_nurse").await;

    let patient_id = seed_patient_with_phone(&pool, "Foe", "Rxdone", "+92300f14b").await;
    let (_, bed_id) = seed_ward_and_bed(&pool, "f14b").await;
    let admission_id = seed_admission(&pool, patient_id, bed_id, "admitted").await;
    let item = seed_prescription_item(&pool, patient_id, "F14b Med").await;

    // Mark the prescription dispensed (a terminal status).
    sqlx::query("UPDATE prescriptions SET status = 'dispensed' WHERE id = (SELECT prescription_id FROM prescription_items WHERE id = $1)")
        .bind(item)
        .execute(&pool)
        .await
        .unwrap();

    let err = record_medication_administration_core(
        &pool,
        &nurse_state,
        RecordAdministrationRequest {
            admission_id,
            prescription_item_id: item,
            status: "administered".into(),
            notes: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("prescription is not active"),
        "an inactive prescription must not be administerable, got: {}",
        err
    );
}

// ── RCTF-FULL-SYSTEM-2026-09-08 F-15: discharge transaction hardening ────────

/// A nurse holds IpdManage but discharge also needs the same — use a
/// doctor session (IpdManage holder) for the discharge core.
async fn f15_session(pool: &PgPool, tag: &str) -> (hospital_mgmt_lib::rbac::Session, i32) {
    let pw = fixture_pw();
    let uid = seed_user(pool, &format!("f15_{}", tag), &pw, &["doctor"]).await;
    seed_session_row(pool, uid, &format!("hash_f15_{}", tag)).await;
    let s = load_session_for(pool, uid, &format!("hash_f15_{}", tag)).await;
    (s, uid)
}

/// Discharging an ALREADY-discharged admission must fail cleanly (the
/// conditional UPDATE + rows_affected check), never double-free the bed
/// or write a second discharge audit row.
#[tokio::test]
async fn rctf_f15_double_discharge_refused() {
    let pool = test_pool().await;
    let (doc, _uid) = f15_session(&pool, "dd").await;

    let patient_id = seed_patient_with_phone(&pool, "Fof", "Disch", "+92300f15a").await;
    let (_, bed_id) = seed_ward_and_bed(&pool, "f15a").await;
    let admission_id = seed_admission(&pool, patient_id, bed_id, "admitted").await;

    // First discharge succeeds and frees the bed.
    discharge_patient_core(
        &pool,
        &doc,
        DischargeIpd {
            id: admission_id,
            discharge_summary: Some("Recovered".into()),
        },
    )
    .await
    .expect("first discharge must succeed");
    let (bed_status,): (String,) = sqlx::query_as("SELECT status FROM beds WHERE id = $1")
        .bind(bed_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(bed_status, "available", "the bed must be freed once");

    // Second discharge refused with the not-found/already-discharged error.
    let err = discharge_patient_core(
        &pool,
        &doc,
        DischargeIpd {
            id: admission_id,
            discharge_summary: Some("Recovered again".into()),
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("already discharged"),
        "double discharge must be refused, got: {}",
        err
    );

    // Discharge date was NOT overwritten by the second attempt.
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM ipd_admissions WHERE id = $1 AND status = 'discharged'",
    )
    .bind(admission_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 1, "exactly one discharged row");
}

/// The unpaid-bills guard must block discharge, and the blocked discharge
/// must leave the admission FULLY intact (still admitted, bed still
/// occupied) — the aborted transaction writes nothing.
#[tokio::test]
async fn rctf_f15_unpaid_bills_block_discharge_atomically() {
    let pool = test_pool().await;
    let (doc, _uid) = f15_session(&pool, "ub").await;

    let patient_id = seed_patient_with_phone(&pool, "Fog", "Owed", "+92300f15b").await;
    let (_, bed_id) = seed_ward_and_bed(&pool, "f15b").await;
    let admission_id = seed_admission(&pool, patient_id, bed_id, "admitted").await;

    // An unpaid bill for this patient (bill_number drawn from the app's
    // sequence, as create_bill does).
    let (bill_number,): (i64,) = sqlx::query_as("SELECT nextval('bill_number_seq')")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO bills (patient_id, bill_number, bill_type, total_amount, net_amount, status) \
         VALUES ($1, $2, 'ipd', 500, 500, 'unpaid')",
    )
    .bind(patient_id)
    .bind(format!("INV-2026-{:06}", bill_number))
    .execute(&pool)
    .await
    .unwrap();

    let err = discharge_patient_core(
        &pool,
        &doc,
        DischargeIpd {
            id: admission_id,
            discharge_summary: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("unpaid bill"),
        "unpaid bills must block discharge, got: {}",
        err
    );

    // The blocked discharge changed NOTHING: admission still admitted,
    // bed still occupied.
    let (admission_status, bed_status): (String, String) = sqlx::query_as(
        "SELECT a.status, b.status FROM ipd_admissions a JOIN beds b ON b.id = a.bed_id WHERE a.id = $1",
    )
    .bind(admission_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        admission_status, "admitted",
        "the admission must stay active"
    );
    assert_eq!(bed_status, "occupied", "the bed must stay occupied");
}

// ── NS-4: Nurse note validation ────────────────────────────────────────────────

/// Note type must be one of shift/observation/handover; empty content is
/// rejected; valid notes persist with author attribution.
#[tokio::test]
async fn test_ns4_note_type_and_content_validation() {
    let pool = test_pool().await;
    let pw = fixture_pw();
    let nurse_id = seed_user(&pool, "ns4_nurse", &pw, &["nurse"]).await;
    seed_session_row(&pool, nurse_id, "hash_ns4_nurse").await;
    let nurse_state = state_for(&pool, nurse_id, "hash_ns4_nurse").await;

    let patient_id = seed_patient_with_phone(&pool, "Nsg", "Notes", "+92300007").await;
    let (_, bed_id) = seed_ward_and_bed(&pool, "ns4").await;
    let admission_id = seed_admission(&pool, patient_id, bed_id, "admitted").await;

    // Invalid type.
    let err = create_nurse_note_core(
        &pool,
        &nurse_state,
        CreateNurseNoteRequest {
            admission_id,
            note_type: "gossip".into(),
            content: "some content".into(),
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("shift, observation, or handover"),
        "invalid note type must be rejected, got: {}",
        err
    );

    // Empty content.
    let err = create_nurse_note_core(
        &pool,
        &nurse_state,
        CreateNurseNoteRequest {
            admission_id,
            note_type: "shift".into(),
            content: "   ".into(),
        },
    )
    .await
    .unwrap_err();
    assert!(
        err.contains("cannot be empty"),
        "empty note content must be rejected, got: {}",
        err
    );

    // Valid handover note: persisted with author attribution.
    let id = create_nurse_note_core(
        &pool,
        &nurse_state,
        CreateNurseNoteRequest {
            admission_id,
            note_type: "handover".into(),
            content: "Patient stable, continue current IV.".into(),
        },
    )
    .await
    .expect("valid note must persist");
    let stored: (String, Option<i32>) =
        sqlx::query_as("SELECT note_type, author_user_id FROM nurse_notes WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored.0, "handover");
    assert_eq!(stored.1, Some(nurse_id));
}
