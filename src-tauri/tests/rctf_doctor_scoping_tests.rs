//! RCTF — own-appointment scoping & doctor-login-creation integration tests.
//!
//! Two calling conventions are used, matching what each command actually
//! needs:
//!   - `update_appointment_core` / `update_appointment_status_core` (the
//!     AERP-pattern extraction added alongside this scoping work — see
//!     commands/appointments.rs) take a plain `&PgPool` + `&SessionState`,
//!     so they're called directly, no mock Tauri app required.
//!   - `get_appointments` / `get_appointment` / `get_appointment_stats` /
//!     `delete_appointment` / `create_login_for_doctor` take `tauri::State`
//!     directly (no `_core` split needed — no `AppHandle` dependency), so
//!     they're called via `common::mock_app` for a real managed `PgPool` +
//!     `SessionState`.
//!   - `create_appointment` (used only as a fixture seeder here, not itself
//!     under test) is NOT called through either path — its `AppHandle` is
//!     pinned to the real `Wry` runtime for WhatsApp notifications
//!     (`#[default_runtime(crate::Wry, wry)]`), which `tauri::test`'s
//!     `MockRuntime` cannot satisfy. Fixture appointments are inserted with
//!     plain SQL instead (`seed_appointment` below).
//!
//! Maps to the RCTF prompt's Section 8 negative security tests:
//!   rctf_a_b_c  — own-list scoping ignores/overrides a malicious doctor_filter
//!   rctf_d      — single-appointment read: cross-doctor id guess = not-found
//!   rctf_e_g    — cross-doctor update rejected; doctor_id reassignment blocked
//!   rctf_f      — cross-doctor status change rejected; doctor role can't delete
//!   rctf_h      — stats scoped, no cross-doctor totals
//!   rctf_i      — doctor with no linked profile: clear error, not unrestricted access
//!   rctf_j      — full-access role (receptionist) regression: unchanged
//!   rctf_k      — create_login_for_doctor happy path
//!   rctf_l      — duplicate login rejected
//!   rctf_m      — unauthorized login creation rejected
//!   rctf_n      — username collision rejected
//!
//! Requires: HMS_TEST_DB_URL env var + `--features hms-integration-tests`.
//! Run with:
//!   cargo test --features hms-integration-tests --test rctf_doctor_scoping_tests

#![cfg(feature = "hms-integration-tests")]

mod common;

use common::*;
use hospital_mgmt_lib::commands::appointments::{
    delete_appointment, get_appointment, get_appointment_stats, get_appointments,
    update_appointment_core, update_appointment_status_core,
};
use hospital_mgmt_lib::commands::doctors::create_login_for_doctor;
use hospital_mgmt_lib::models::UpdateAppointment;
use hospital_mgmt_lib::rbac::SessionState;
use sqlx::PgPool;
use std::sync::{Arc, Mutex};
use tauri::Manager;

// ── fixtures ────────────────────────────────────────────────────────────────

async fn state_for(pool: &PgPool, user_id: i32, token_hash: &str) -> SessionState {
    let s = load_session_for(pool, user_id, token_hash).await;
    Arc::new(Mutex::new(Some(s)))
}

/// Build a mock Tauri app managing the real pool + a real, DB-loaded session
/// for `user_id` — for the commands that take `tauri::State` directly.
async fn app_for(
    pool: &PgPool,
    user_id: i32,
    token_hash: &str,
) -> tauri::App<tauri::test::MockRuntime> {
    let session = load_session_for(pool, user_id, token_hash).await;
    mock_app(pool.clone(), Some(session))
}

fn states(
    app: &tauri::App<tauri::test::MockRuntime>,
) -> (tauri::State<'_, PgPool>, tauri::State<'_, SessionState>) {
    (app.state::<PgPool>(), app.state::<SessionState>())
}

/// A doctor directory row already linked to `user_id` — for tests that only
/// need a pre-linked fixture (the linking flow itself is exercised by
/// rctf_k/l/m/n below, via the real `create_login_for_doctor` command).
async fn seed_doctor_linked(pool: &PgPool, user_id: i32, first: &str, last: &str) -> i32 {
    let (id,): (i32,) = sqlx::query_as(
        "INSERT INTO doctors (user_id, first_name, last_name, phone, specialization, qualification)
         VALUES ($1, $2, $3, '555-0100', 'General Practice', 'MBBS') RETURNING id",
    )
    .bind(user_id)
    .bind(first)
    .bind(last)
    .fetch_one(pool)
    .await
    .expect("seed linked doctor");
    id
}

/// A doctor directory row with NO linked user — the create-login target.
async fn seed_doctor_unlinked(pool: &PgPool, first: &str, last: &str) -> i32 {
    let (id,): (i32,) = sqlx::query_as(
        "INSERT INTO doctors (first_name, last_name, phone, specialization, qualification)
         VALUES ($1, $2, '555-0200', 'General Practice', 'MBBS') RETURNING id",
    )
    .bind(first)
    .bind(last)
    .fetch_one(pool)
    .await
    .expect("seed unlinked doctor");
    id
}

fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

/// Insert a fixture appointment directly (bypasses `create_appointment` —
/// see the module doc comment on why). `time` is "HH:MM:SS".
async fn seed_appointment(pool: &PgPool, patient_id: i32, doctor_id: i32, time: &str) -> i32 {
    let (id,): (i32,) = sqlx::query_as(
        "INSERT INTO appointments
             (patient_id, doctor_id, appointment_date, appointment_time,
              duration_minutes, status)
         VALUES ($1, $2, $3, $4::TIME, 30, 'scheduled')
         RETURNING id",
    )
    .bind(patient_id)
    .bind(doctor_id)
    .bind(today())
    .bind(time)
    .fetch_one(pool)
    .await
    .expect("seed fixture appointment");
    id
}

// ── rctf_a_b_c: own-list scoping ────────────────────────────────────────────

#[tokio::test]
async fn rctf_a_b_c_own_list_ignores_malicious_doctor_filter() {
    let pool = test_pool().await;
    let pw = fixture_pw();

    let doc_a_user = seed_user(&pool, "rctf_abc_doc_a", &pw, &["doctor"]).await;
    let doc_b_user = seed_user(&pool, "rctf_abc_doc_b", &pw, &["doctor"]).await;
    let doc_a = seed_doctor_linked(&pool, doc_a_user, "Abc", "DoctorA").await;
    let doc_b = seed_doctor_linked(&pool, doc_b_user, "Abc", "DoctorB").await;

    let pat_a = seed_patient_with_phone(&pool, "AbcA", "Patient", "+92300111001").await;
    let pat_b = seed_patient_with_phone(&pool, "AbcB", "Patient", "+92300111002").await;

    seed_appointment(&pool, pat_a, doc_a, "09:00:00").await;
    seed_appointment(&pool, pat_b, doc_b, "10:00:00").await;

    seed_session_row(&pool, doc_a_user, "hash_rctf_abc_doc_a").await;

    // Test C: no filter — only Doctor A's own appointments come back.
    let app = app_for(&pool, doc_a_user, "hash_rctf_abc_doc_a").await;
    let (pool_s, sess_s) = states(&app);
    let no_filter = get_appointments(pool_s, sess_s, None, None, None)
        .await
        .expect("doctor A list (no filter)");
    assert!(
        no_filter.iter().all(|a| a.doctor_id == doc_a),
        "own-scope list without a filter leaked another doctor's appointment: {:?}",
        no_filter.iter().map(|a| a.doctor_id).collect::<Vec<_>>()
    );
    assert!(
        !no_filter.is_empty(),
        "doctor A should see at least their own booked appointment"
    );

    // Test B: malicious doctor_filter = Doctor B — still only Doctor A's own.
    let app2 = app_for(&pool, doc_a_user, "hash_rctf_abc_doc_a").await;
    let (pool_s2, sess_s2) = states(&app2);
    let with_malicious_filter = get_appointments(pool_s2, sess_s2, None, None, Some(doc_b))
        .await
        .expect("doctor A list (doctor_filter = B)");
    assert!(
        with_malicious_filter.iter().all(|a| a.doctor_id == doc_a),
        "doctor_filter=B must be ignored/overridden for an own-scope session, got: {:?}",
        with_malicious_filter
            .iter()
            .map(|a| a.doctor_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        no_filter.len(),
        with_malicious_filter.len(),
        "own-scope list must be identical regardless of the client-supplied doctor_filter"
    );
}

// ── rctf_d: single-appointment read, cross-doctor id guess ─────────────────

#[tokio::test]
async fn rctf_d_single_read_cross_doctor_is_not_found() {
    let pool = test_pool().await;
    let pw = fixture_pw();

    let doc_a_user = seed_user(&pool, "rctf_d_doc_a", &pw, &["doctor"]).await;
    let doc_b_user = seed_user(&pool, "rctf_d_doc_b", &pw, &["doctor"]).await;
    seed_doctor_linked(&pool, doc_a_user, "D", "DoctorA").await;
    let doc_b = seed_doctor_linked(&pool, doc_b_user, "D", "DoctorB").await;

    let pat_b = seed_patient_with_phone(&pool, "DB", "Patient", "+92300111003").await;
    let appt_b = seed_appointment(&pool, pat_b, doc_b, "11:00:00").await;

    seed_session_row(&pool, doc_a_user, "hash_rctf_d_doc_a").await;
    let app = app_for(&pool, doc_a_user, "hash_rctf_d_doc_a").await;
    let (pool_s, sess_s) = states(&app);

    let result = get_appointment(pool_s, sess_s, appt_b).await;
    assert!(
        result.is_err(),
        "doctor A must not be able to read doctor B's appointment by id"
    );

    // Guessed-nonexistent-id gets the SAME error shape (no existence oracle).
    let app2 = app_for(&pool, doc_a_user, "hash_rctf_d_doc_a").await;
    let (pool_s2, sess_s2) = states(&app2);
    let nonexistent = get_appointment(pool_s2, sess_s2, 999_999_999).await;
    assert!(nonexistent.is_err());
    assert_eq!(
        result.unwrap_err(),
        nonexistent.unwrap_err(),
        "cross-doctor id and a nonexistent id must be indistinguishable"
    );
}

// ── rctf_e_g: cross-doctor update rejected; reassignment blocked ───────────

#[tokio::test]
async fn rctf_e_g_update_cross_doctor_rejected_and_reassignment_blocked() {
    let pool = test_pool().await;
    let pw = fixture_pw();

    let doc_a_user = seed_user(&pool, "rctf_eg_doc_a", &pw, &["doctor"]).await;
    let doc_b_user = seed_user(&pool, "rctf_eg_doc_b", &pw, &["doctor"]).await;
    let doc_a = seed_doctor_linked(&pool, doc_a_user, "Eg", "DoctorA").await;
    let doc_b = seed_doctor_linked(&pool, doc_b_user, "Eg", "DoctorB").await;

    let pat_a = seed_patient_with_phone(&pool, "EgA", "Patient", "+92300111004").await;
    let pat_b = seed_patient_with_phone(&pool, "EgB", "Patient", "+92300111005").await;
    let appt_a = seed_appointment(&pool, pat_a, doc_a, "12:00:00").await;
    let appt_b = seed_appointment(&pool, pat_b, doc_b, "13:00:00").await;

    seed_session_row(&pool, doc_a_user, "hash_rctf_eg_doc_a").await;
    let sess = state_for(&pool, doc_a_user, "hash_rctf_eg_doc_a").await;

    // Test E: Doctor A cannot update Doctor B's appointment.
    let cross_update = update_appointment_core(
        &pool,
        &sess,
        UpdateAppointment {
            id: appt_b,
            patient_id: pat_b,
            doctor_id: doc_b,
            appointment_date: today().to_string(),
            appointment_time: "13:30".into(),
            duration_minutes: 30,
            status: "confirmed".into(),
            reason: None,
            notes: None,
            consultation_fee: None,
            fee_paid: None,
        },
    )
    .await;
    assert!(
        cross_update.is_err(),
        "doctor A must not update doctor B's appointment"
    );

    // Test G: Doctor A updates their OWN appointment but tries to reassign
    // it to Doctor B via the request body's doctor_id — must be silently
    // forced back to Doctor A, never actually reassigned.
    let reassign_attempt = update_appointment_core(
        &pool,
        &sess,
        UpdateAppointment {
            id: appt_a,
            patient_id: pat_a,
            doctor_id: doc_b, // malicious: try to move it to Doctor B
            appointment_date: today().to_string(),
            appointment_time: "12:15".into(),
            duration_minutes: 30,
            status: "confirmed".into(),
            reason: Some("reassignment attempt".into()),
            notes: None,
            consultation_fee: None,
            fee_paid: None,
        },
    )
    .await;
    assert!(
        reassign_attempt.is_ok(),
        "doctor A editing their OWN appointment should succeed (just not the reassignment): {:?}",
        reassign_attempt
    );
    let (owner,): (i32,) = sqlx::query_as("SELECT doctor_id FROM appointments WHERE id = $1")
        .bind(appt_a)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        owner, doc_a,
        "appointment must still belong to doctor A — reassignment via the update body must be ignored"
    );
}

// ── rctf_f: cross-doctor status change rejected; doctor can't delete ───────

#[tokio::test]
async fn rctf_f_status_change_cross_doctor_rejected() {
    let pool = test_pool().await;
    let pw = fixture_pw();

    let doc_a_user = seed_user(&pool, "rctf_f_doc_a", &pw, &["doctor"]).await;
    let doc_b_user = seed_user(&pool, "rctf_f_doc_b", &pw, &["doctor"]).await;
    seed_doctor_linked(&pool, doc_a_user, "F", "DoctorA").await;
    let doc_b = seed_doctor_linked(&pool, doc_b_user, "F", "DoctorB").await;

    let pat_b = seed_patient_with_phone(&pool, "FB", "Patient", "+92300111006").await;
    let appt_b = seed_appointment(&pool, pat_b, doc_b, "14:00:00").await;

    seed_session_row(&pool, doc_a_user, "hash_rctf_f_doc_a").await;
    let sess = state_for(&pool, doc_a_user, "hash_rctf_f_doc_a").await;

    let result =
        update_appointment_status_core(&pool, &sess, appt_b, "cancelled".to_string()).await;
    assert!(
        result.is_err(),
        "doctor A must not change doctor B's appointment status"
    );

    // Separately: no seeded role holds AppointmentsDelete without also
    // holding full AppointmentsView — a doctor-role session must be denied
    // outright (defense-in-depth branch, not currently reachable via the
    // seeded roles, but the guard itself must still hold).
    let app = app_for(&pool, doc_a_user, "hash_rctf_f_doc_a").await;
    let (pool_s, sess_s) = states(&app);
    let delete_result = delete_appointment(pool_s, sess_s, appt_b).await;
    assert!(
        delete_result.is_err(),
        "doctor role must not be able to delete any appointment"
    );
}

// ── rctf_h: stats scoping ───────────────────────────────────────────────────

#[tokio::test]
async fn rctf_h_stats_scoped_no_cross_doctor_leak() {
    let pool = test_pool().await;
    let pw = fixture_pw();

    let doc_a_user = seed_user(&pool, "rctf_h_doc_a", &pw, &["doctor"]).await;
    let doc_b_user = seed_user(&pool, "rctf_h_doc_b", &pw, &["doctor"]).await;
    let doc_a = seed_doctor_linked(&pool, doc_a_user, "H", "DoctorA").await;
    let doc_b = seed_doctor_linked(&pool, doc_b_user, "H", "DoctorB").await;

    let pat_a1 = seed_patient_with_phone(&pool, "HA1", "Patient", "+92300111007").await;
    let pat_b1 = seed_patient_with_phone(&pool, "HB1", "Patient", "+92300111008").await;
    let pat_b2 = seed_patient_with_phone(&pool, "HB2", "Patient", "+92300111009").await;

    // Doctor A: 1 appointment. Doctor B: 2 appointments. If stats leaked,
    // Doctor A's "total" would be 3, not 1.
    seed_appointment(&pool, pat_a1, doc_a, "08:00:00").await;
    seed_appointment(&pool, pat_b1, doc_b, "08:30:00").await;
    seed_appointment(&pool, pat_b2, doc_b, "09:00:00").await;

    seed_session_row(&pool, doc_a_user, "hash_rctf_h_doc_a").await;
    let app = app_for(&pool, doc_a_user, "hash_rctf_h_doc_a").await;
    let (pool_s, sess_s) = states(&app);
    let stats = get_appointment_stats(pool_s, sess_s)
        .await
        .expect("doctor A stats");
    assert_eq!(
        stats.total, 1,
        "doctor A's stats must reflect only their own appointment, not doctor B's"
    );
}

// ── rctf_i: no linked practitioner profile ──────────────────────────────────

#[tokio::test]
async fn rctf_i_unlinked_doctor_gets_clear_error_not_unrestricted_access() {
    let pool = test_pool().await;
    let pw = fixture_pw();

    // seed_user alone never touches `doctors` — this account is a
    // doctor-role user with NO linked practitioner profile.
    let unlinked_user = seed_user(&pool, "rctf_i_unlinked_doc", &pw, &["doctor"]).await;
    seed_session_row(&pool, unlinked_user, "hash_rctf_i_unlinked").await;

    let app = app_for(&pool, unlinked_user, "hash_rctf_i_unlinked").await;
    let (pool_s, sess_s) = states(&app);
    let result = get_appointments(pool_s, sess_s, None, None, None).await;
    assert!(
        result.is_err(),
        "a doctor-role account with no linked practitioner profile must get an error, not an appointment list"
    );
    let msg = result.unwrap_err();
    assert!(
        msg.to_lowercase().contains("practitioner") || msg.to_lowercase().contains("linked"),
        "expected a clear 'not linked to a practitioner profile' message, got: {}",
        msg
    );
}

// ── rctf_j: full-access role regression ─────────────────────────────────────

#[tokio::test]
async fn rctf_j_full_access_role_regression_unchanged() {
    let pool = test_pool().await;
    let pw = fixture_pw();

    let receptionist = seed_user(&pool, "rctf_j_reception", &pw, &["receptionist"]).await;
    seed_session_row(&pool, receptionist, "hash_rctf_j_reception").await;

    let doc_a_user = seed_user(&pool, "rctf_j_doc_a", &pw, &["doctor"]).await;
    let doc_b_user = seed_user(&pool, "rctf_j_doc_b", &pw, &["doctor"]).await;
    let doc_a = seed_doctor_linked(&pool, doc_a_user, "J", "DoctorA").await;
    let doc_b = seed_doctor_linked(&pool, doc_b_user, "J", "DoctorB").await;

    let pat_a = seed_patient_with_phone(&pool, "JA", "Patient", "+92300111010").await;
    let pat_b = seed_patient_with_phone(&pool, "JB", "Patient", "+92300111011").await;
    seed_appointment(&pool, pat_a, doc_a, "15:00:00").await;
    seed_appointment(&pool, pat_b, doc_b, "15:30:00").await;

    // Receptionist (full AppointmentsView, not super_admin) must still see
    // BOTH doctors' appointments — this is the regression check.
    let app = app_for(&pool, receptionist, "hash_rctf_j_reception").await;
    let (pool_s, sess_s) = states(&app);
    let hits = get_appointments(pool_s, sess_s, None, None, None)
        .await
        .expect("receptionist list");
    let seen_doctors: std::collections::HashSet<i32> = hits.iter().map(|a| a.doctor_id).collect();
    assert!(
        seen_doctors.contains(&doc_a) && seen_doctors.contains(&doc_b),
        "receptionist (full AppointmentsView) must still see every doctor's appointments, saw: {:?}",
        seen_doctors
    );
}

// ── rctf_k/l/m/n: create_login_for_doctor ───────────────────────────────────

#[tokio::test]
async fn rctf_k_create_login_happy_path() {
    let pool = test_pool().await;
    let pw = fixture_pw();

    let admin = seed_user(&pool, "rctf_k_admin", &pw, &["super_admin"]).await;
    seed_session_row(&pool, admin, "hash_rctf_k_admin").await;
    let doctor_id = seed_doctor_unlinked(&pool, "Kay", "Practitioner").await;

    let app = app_for(&pool, admin, "hash_rctf_k_admin").await;
    let (pool_s, sess_s) = states(&app);
    let (username, password) =
        create_login_for_doctor(pool_s, sess_s, doctor_id, "kay.practitioner".to_string())
            .await
            .expect("create_login_for_doctor happy path");

    assert_eq!(username, "kay.practitioner");
    assert!(
        password.len() >= 8,
        "temporary password looks too short: {:?}",
        password
    );

    let (linked_user_id,): (Option<i32>,) =
        sqlx::query_as("SELECT user_id FROM doctors WHERE id = $1")
            .bind(doctor_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let linked_user_id = linked_user_id.expect("doctor.user_id must now be set");

    let (db_username, must_change_password): (String, bool) =
        sqlx::query_as("SELECT username, must_change_password FROM users WHERE id = $1")
            .bind(linked_user_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let (role_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM user_roles ur JOIN roles r ON r.id = ur.role_id \
         WHERE ur.user_id = $1 AND r.name = 'doctor'",
    )
    .bind(linked_user_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(db_username, "kay.practitioner");
    assert!(
        must_change_password,
        "must_change_password must be TRUE for a freshly created login"
    );
    assert_eq!(
        role_count, 1,
        "the new user must have the doctor role assigned exactly once"
    );
}

#[tokio::test]
async fn rctf_l_duplicate_login_rejected() {
    let pool = test_pool().await;
    let pw = fixture_pw();

    let admin = seed_user(&pool, "rctf_l_admin", &pw, &["super_admin"]).await;
    seed_session_row(&pool, admin, "hash_rctf_l_admin").await;
    let doc_user = seed_user(&pool, "rctf_l_already_linked", &pw, &["doctor"]).await;
    let doctor_id = seed_doctor_linked(&pool, doc_user, "El", "AlreadyLinked").await;

    let app = app_for(&pool, admin, "hash_rctf_l_admin").await;
    let (pool_s, sess_s) = states(&app);
    let result =
        create_login_for_doctor(pool_s, sess_s, doctor_id, "el.duplicate".to_string()).await;
    assert!(
        result.is_err(),
        "creating a second login for an already-linked doctor must fail"
    );

    let (user_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM users WHERE username = 'el.duplicate'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        user_count, 0,
        "no partial user row must be created on rejection"
    );
}

#[tokio::test]
async fn rctf_m_unauthorized_login_creation_rejected() {
    let pool = test_pool().await;
    let pw = fixture_pw();

    // A nurse does not hold UsersManage.
    let nurse = seed_user(&pool, "rctf_m_nurse", &pw, &["nurse"]).await;
    seed_session_row(&pool, nurse, "hash_rctf_m_nurse").await;
    let doctor_id = seed_doctor_unlinked(&pool, "Em", "Unauthorizedtarget").await;

    let app = app_for(&pool, nurse, "hash_rctf_m_nurse").await;
    let (pool_s, sess_s) = states(&app);
    let result = create_login_for_doctor(pool_s, sess_s, doctor_id, "em.target".to_string()).await;
    assert!(
        result.is_err(),
        "a session without UsersManage must not be able to create a login"
    );

    let (still_unlinked,): (Option<i32>,) =
        sqlx::query_as("SELECT user_id FROM doctors WHERE id = $1")
            .bind(doctor_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        still_unlinked.is_none(),
        "doctor must remain unlinked after a rejected unauthorized attempt"
    );
}

#[tokio::test]
async fn rctf_n_username_collision_rejected() {
    let pool = test_pool().await;
    let pw = fixture_pw();

    let admin = seed_user(&pool, "rctf_n_admin", &pw, &["super_admin"]).await;
    seed_session_row(&pool, admin, "hash_rctf_n_admin").await;
    // Existing user whose username we'll collide with.
    seed_user(&pool, "rctf_n_taken", &pw, &["nurse"]).await;
    let doctor_id = seed_doctor_unlinked(&pool, "En", "Collisiontarget").await;

    let app = app_for(&pool, admin, "hash_rctf_n_admin").await;
    let (pool_s, sess_s) = states(&app);
    let result =
        create_login_for_doctor(pool_s, sess_s, doctor_id, "rctf_n_taken".to_string()).await;
    assert!(
        result.is_err(),
        "creating a login with an already-taken username must fail"
    );

    let (still_unlinked,): (Option<i32>,) =
        sqlx::query_as("SELECT user_id FROM doctors WHERE id = $1")
            .bind(doctor_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        still_unlinked.is_none(),
        "doctor must remain unlinked after a username-collision rejection"
    );
}
