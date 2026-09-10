//! Nursing Station commands (SRS §2.7 — Phase 6.1, 2026-09-05).
//!
//! Ward workflow for nurses: vitals entry with trend, nurse (shift) notes,
//! and the Medication Administration Record (MAR). Permission model reuses
//! the existing IPD permissions per the SRS role mapping:
//! IpdView → read ward lists, vitals, notes, MAR;
//! IpdManage → record vitals/notes, mark MAR entries.
//! Nurses hold both (rbac::permissions_for_role ROLE_NURSE).
//!
//! Every command is RBAC-guarded and audited per SRS NFR-15 (same pattern
//! as the rest of the IPD surface).

use sqlx::PgPool;

use crate::audit;
use crate::rbac::{self, Permission, SessionState};

// ── Models ────────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct VitalReading {
    pub id: i32,
    pub admission_id: i32,
    pub temperature_c: Option<rust_decimal::Decimal>,
    pub systolic_bp: Option<i32>,
    pub diastolic_bp: Option<i32>,
    pub pulse_bpm: Option<i32>,
    pub resp_rate: Option<i32>,
    pub spo2_pct: Option<i32>,
    pub pain_score: Option<i32>,
    pub recorded_by_user_id: Option<i32>,
    pub recorded_at: chrono::DateTime<chrono::Utc>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RecordVitalsRequest {
    pub admission_id: i32,
    #[serde(default)]
    pub temperature_c: Option<f64>,
    #[serde(default)]
    pub systolic_bp: Option<i32>,
    #[serde(default)]
    pub diastolic_bp: Option<i32>,
    #[serde(default)]
    pub pulse_bpm: Option<i32>,
    #[serde(default)]
    pub resp_rate: Option<i32>,
    #[serde(default)]
    pub spo2_pct: Option<i32>,
    #[serde(default)]
    pub pain_score: Option<i32>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct NurseNote {
    pub id: i32,
    pub admission_id: i32,
    pub author_user_id: Option<i32>,
    pub note_type: String,
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateNurseNoteRequest {
    pub admission_id: i32,
    #[serde(default = "default_note_type")]
    pub note_type: String,
    pub content: String,
}

fn default_note_type() -> String {
    "shift".to_string()
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct MedicationAdministration {
    pub id: i32,
    pub admission_id: i32,
    pub prescription_item_id: i32,
    pub patient_id: i32,
    pub status: String,
    pub administered_by_user_id: Option<i32>,
    pub administered_at: chrono::DateTime<chrono::Utc>,
    pub notes: Option<String>,
    /// Joined from prescription_items (nullable like the other joined name
    /// fields — the item row may be gone if the prescription was deleted).
    #[serde(default)]
    pub medication_name: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct RecordAdministrationRequest {
    pub admission_id: i32,
    pub prescription_item_id: i32,
    /// Must be administered | held | refused (DB CHECK enforces).
    pub status: String,
    #[serde(default)]
    pub notes: Option<String>,
}

// ── Commands ────────────────────────────────────────────────────────────────

/// Record a vitals reading for an admitted patient.
/// RBAC: IpdManage (nurse-level write). Audited.
#[tauri::command]
pub async fn record_vitals(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    request: RecordVitalsRequest,
) -> Result<i32, String> {
    record_vitals_core(pool.inner(), &session, request).await
}

/// Vitals-recording logic core (AERP Part G extraction pattern): guard +
/// validation + INSERT + audit, callable without a Tauri AppHandle so the
/// role-boundary and validation regressions run at command level
/// (nursing_tests.rs).
pub async fn record_vitals_core(
    pool: &PgPool,
    session_state: &SessionState,
    request: RecordVitalsRequest,
) -> Result<i32, String> {
    let s = rbac::require_strong(session_state, pool, Permission::IpdManage).await?;

    if request.temperature_c.is_none()
        && request.systolic_bp.is_none()
        && request.pulse_bpm.is_none()
        && request.resp_rate.is_none()
        && request.spo2_pct.is_none()
    {
        return Err("At least one vital sign must be recorded.".to_string());
    }

    // RCTF-FULL-SYSTEM-2026-09-08 F-22: plausibility guard. The classic
    // error is a missed decimal (temperature 370 instead of 37.0, SpO2 9.8
    // instead of 98) — implausible values previously stored cleanly and
    // rendered in the 7-point trend chart nurses scan for deterioration.
    // Ranges are HARD limits (not soft warnings): values outside them are
    // physiologically impossible or certain data-entry errors, refused
    // with the expected range so the nurse can correct the entry.
    //   temperature 30–43 °C   (hypothermia to severe hyperthermia)
    //   systolic    50–260 mmHg
    //   diastolic   30–150 mmHg
    //   pulse       20–250 bpm
    //   resp rate   4–60 /min
    //   SpO2        50–100 %
    if let Some(t) = request.temperature_c {
        if !(30.0..=43.0).contains(&t) {
            return Err(format!(
                "Temperature {} °C is not physiologically plausible (expected 30–43). \
                 Check for a missed decimal point.",
                t
            ));
        }
    }
    if let Some(v) = request.systolic_bp {
        if !(50..=260).contains(&v) {
            return Err(format!(
                "Systolic BP {} mmHg is not physiologically plausible (expected 50–260).",
                v
            ));
        }
    }
    if let Some(v) = request.diastolic_bp {
        if !(30..=150).contains(&v) {
            return Err(format!(
                "Diastolic BP {} mmHg is not physiologically plausible (expected 30–150).",
                v
            ));
        }
    }
    if let Some(v) = request.pulse_bpm {
        if !(20..=250).contains(&v) {
            return Err(format!(
                "Pulse {} bpm is not physiologically plausible (expected 20–250).",
                v
            ));
        }
    }
    if let Some(v) = request.resp_rate {
        if !(4..=60).contains(&v) {
            return Err(format!(
                "Respiratory rate {}/min is not physiologically plausible (expected 4–60).",
                v
            ));
        }
    }
    if let Some(v) = request.spo2_pct {
        if !(50..=100).contains(&v) {
            return Err(format!(
                "SpO2 {}% is not physiologically plausible (expected 50–100). \
                 Check for a missed decimal point.",
                v
            ));
        }
    }

    // Guard: the admission must exist and be currently admitted (no vitals
    // on discharged/dead rows — data hygiene for the trend view).
    let active: Option<(i32,)> = sqlx::query_as(
        "SELECT patient_id FROM ipd_admissions WHERE id = $1 AND status = 'admitted'",
    )
    .bind(request.admission_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;
    if active.is_none() {
        return Err("No active admission found for this patient. Vitals can only be recorded for admitted patients.".to_string());
    }

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO vitals
               (admission_id, temperature_c, systolic_bp, diastolic_bp,
                pulse_bpm, resp_rate, spo2_pct, pain_score, recorded_by_user_id, notes)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id"#,
    )
    .bind(request.admission_id)
    .bind(
        request
            .temperature_c
            .map(rust_decimal::Decimal::from_f64_retain),
    )
    .bind(request.systolic_bp)
    .bind(request.diastolic_bp)
    .bind(request.pulse_bpm)
    .bind(request.resp_rate)
    .bind(request.spo2_pct)
    .bind(request.pain_score)
    .bind(s.user_id)
    .bind(request.notes.as_deref())
    .fetch_one(pool)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(
        pool,
        &s,
        "vitals_record",
        "vitals",
        Some(&row.0.to_string()),
        Some(serde_json::json!({
            "admission_id": request.admission_id,
            "spo2": request.spo2_pct,
            "systolic": request.systolic_bp,
        })),
    )
    .await;
    Ok(row.0)
}

/// Last 7 vitals readings (SRS: "trend graph — last 7 readings"),
/// newest first. RBAC: IpdView.
#[tauri::command]
pub async fn get_vitals_trend(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    admission_id: i32,
) -> Result<Vec<VitalReading>, String> {
    let _ = rbac::require(&session, Permission::IpdView)?;
    sqlx::query_as::<_, VitalReading>(
        "SELECT id, admission_id, temperature_c, systolic_bp, diastolic_bp, \
                pulse_bpm, resp_rate, spo2_pct, pain_score, recorded_by_user_id, \
                recorded_at, notes \
         FROM vitals WHERE admission_id = $1 \
         ORDER BY recorded_at DESC LIMIT 7",
    )
    .bind(admission_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))
}
/// Create a nurse note (default type: shift). RBAC: IpdManage. Audited.
#[tauri::command]
pub async fn create_nurse_note(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    request: CreateNurseNoteRequest,
) -> Result<i32, String> {
    create_nurse_note_core(pool.inner(), &session, request).await
}

/// Nurse-note logic core (AERP Part G extraction pattern) — see
/// `record_vitals_core` for the rationale.
pub async fn create_nurse_note_core(
    pool: &PgPool,
    session_state: &SessionState,
    request: CreateNurseNoteRequest,
) -> Result<i32, String> {
    let s = rbac::require_strong(session_state, pool, Permission::IpdManage).await?;

    if request.content.trim().is_empty() {
        return Err("Note content cannot be empty.".to_string());
    }
    if request.note_type != "shift"
        && request.note_type != "observation"
        && request.note_type != "handover"
    {
        return Err("Note type must be shift, observation, or handover.".to_string());
    }

    let row: (i32,) = sqlx::query_as(
        "INSERT INTO nurse_notes (admission_id, author_user_id, note_type, content) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(request.admission_id)
    .bind(s.user_id)
    .bind(&request.note_type)
    .bind(request.content.trim())
    .fetch_one(pool)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(
        pool,
        &s,
        "nurse_note_create",
        "nursing",
        Some(&row.0.to_string()),
        Some(serde_json::json!({
            "admission_id": request.admission_id,
            "note_type": request.note_type,
        })),
    )
    .await;
    Ok(row.0)
}

/// List nurse notes for an admission, newest first. RBAC: IpdView.
#[tauri::command]
pub async fn get_nurse_notes(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    admission_id: i32,
) -> Result<Vec<NurseNote>, String> {
    let _ = rbac::require(&session, Permission::IpdView)?;
    sqlx::query_as::<_, NurseNote>(
        "SELECT id, admission_id, author_user_id, note_type, content, created_at \
         FROM nurse_notes WHERE admission_id = $1 \
         ORDER BY created_at DESC LIMIT 200",
    )
    .bind(admission_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))
}

/// Record a MAR entry: administered / held / refused for a prescription
/// item on an admission. RBAC: IpdManage. Audited.
#[tauri::command]
pub async fn record_medication_administration(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    request: RecordAdministrationRequest,
) -> Result<i32, String> {
    record_medication_administration_core(pool.inner(), &session, request).await
}

/// MAR-recording logic core (AERP Part G extraction pattern) — see
/// `record_vitals_core` for the rationale.
///
/// RCTF-FULL-SYSTEM-2026-09-08 F-14: the previous implementation ran the
/// patient-match SELECT on the pool and then a separate unconditional
/// INSERT — (a) two nurses tapping "administered" within the same minute
/// produced TWO rows (an accidental double dose left no system trace),
/// (b) a discharge between the check and the insert landed a MAR row on a
/// discharged admission, and (c) the prescription could be cancelled in
/// that window. All checks now run inside ONE transaction with row locks:
/// the admission and prescription rows are taken `FOR UPDATE`, the
/// prescription must be ACTIVE (status 'active'), and an 'administered'
/// entry within the last 15 minutes for the same (admission, item) is
/// refused as a likely double-administration.
pub async fn record_medication_administration_core(
    pool: &PgPool,
    session_state: &SessionState,
    request: RecordAdministrationRequest,
) -> Result<i32, String> {
    let s = rbac::require_strong(session_state, pool, Permission::IpdManage).await?;

    if request.status != "administered" && request.status != "held" && request.status != "refused" {
        return Err("MAR status must be administered, held, or refused.".to_string());
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // The prescription item must belong to the same patient as the
    // admission, the admission must still be ACTIVE, and the prescription
    // itself must still be ACTIVE — checked INSIDE the transaction with
    // row locks on both, so a concurrent discharge or rx cancellation
    // cannot slip a MAR entry in after the check.
    let patient_match: Option<(i32,)> = sqlx::query_as(
        "SELECT a.patient_id FROM ipd_admissions a \
         JOIN prescriptions rx ON rx.id = (SELECT prescription_id FROM prescription_items WHERE id = $2) \
         WHERE a.id = $1 AND rx.patient_id = a.patient_id \
           AND a.status = 'admitted' \
           AND rx.status = 'active' \
         FOR UPDATE OF a, rx",
    )
    .bind(request.admission_id)
    .bind(request.prescription_item_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;
    let patient_id = patient_match
        .ok_or_else(|| "This medication does not belong to the patient on this admission, the admission is not active, or the prescription is not active.".to_string())?
        .0;

    // Double-administration guard: an 'administered' entry for the same
    // (admission, item) within the last 15 minutes is almost certainly a
    // double-tap or a missed handover — refuse it so the nurse verifies
    // before the dose is given. 'held'/'refused' entries are exempt (both
    // can legitimately repeat) and a deliberate re-administration after
    // the window (e.g. a 4-hourly PRN) passes.
    if request.status == "administered" {
        let recent: Option<(i32,)> = sqlx::query_as(
            "SELECT id FROM medication_administrations \
             WHERE admission_id = $1 AND prescription_item_id = $2 AND status = 'administered' \
               AND administered_at >= NOW() - INTERVAL '15 minutes' \
             LIMIT 1",
        )
        .bind(request.admission_id)
        .bind(request.prescription_item_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;
        if recent.is_some() {
            return Err(
                "This medication was already recorded as administered in the last 15 minutes \
                 for this admission. Verify with the other nurse before recording again — \
                 possible double administration."
                    .to_string(),
            );
        }
    }

    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO medication_administrations
               (admission_id, prescription_item_id, patient_id, status,
                administered_by_user_id, notes)
           VALUES ($1, $2, $3, $4, $5, $6) RETURNING id"#,
    )
    .bind(request.admission_id)
    .bind(request.prescription_item_id)
    .bind(patient_id)
    .bind(&request.status)
    .bind(s.user_id)
    .bind(request.notes.as_deref())
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    tx.commit()
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(
        pool,
        &s,
        "mar_record",
        "nursing",
        Some(&row.0.to_string()),
        Some(serde_json::json!({
            "admission_id": request.admission_id,
            "prescription_item_id": request.prescription_item_id,
            "status": request.status,
        })),
    )
    .await;
    Ok(row.0)
}

/// MAR view: all administration entries for an admission, newest first.
/// RBAC: IpdView.
#[tauri::command]
pub async fn get_medication_administrations(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    admission_id: i32,
) -> Result<Vec<MedicationAdministration>, String> {
    let _ = rbac::require(&session, Permission::IpdView)?;
    sqlx::query_as::<_, MedicationAdministration>(
        "SELECT m.id, m.admission_id, m.prescription_item_id, m.patient_id, m.status, \
                m.administered_by_user_id, m.administered_at, m.notes, \
                pi.medication_name \
         FROM medication_administrations m \
         LEFT JOIN prescription_items pi ON pi.id = m.prescription_item_id \
         WHERE m.admission_id = $1 \
         ORDER BY m.administered_at DESC LIMIT 200",
    )
    .bind(admission_id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))
}

#[cfg(test)]
mod tests {
    /// Pins the IPC wire format for `VitalReading.temperature_c`: the crate's
    /// default serde impl serializes Decimal as a JSON STRING (only the
    /// opt-in `rust_decimal::serde::float` module would emit a number, and
    /// the app does not use it). The frontend type and display helpers
    /// depend on this — drift here breaks the vitals trend chart.
    #[test]
    fn decimal_serializes_as_string_over_ipc() {
        let v = rust_decimal::Decimal::new(375, 1); // 37.5
        let json = serde_json::to_string(&v).expect("serialize decimal");
        assert!(
            json.starts_with('"'),
            "Decimal must serialize as a JSON string, got: {}",
            json
        );
        let opt = Some(v);
        let opt_json = serde_json::to_string(&opt).expect("serialize Option<Decimal>");
        assert_eq!(opt_json, "\"37.5\"");
    }
}
