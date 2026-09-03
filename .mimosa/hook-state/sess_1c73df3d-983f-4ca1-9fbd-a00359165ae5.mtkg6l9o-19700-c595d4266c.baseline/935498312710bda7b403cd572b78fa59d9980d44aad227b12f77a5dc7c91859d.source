//! In-Patient Department (IPD) — wards, beds, admissions. RBAC-guarded + audited.
//!
//! Bed-availability invariant: admitting a patient sets the bed status to
//! `occupied`; discharging sets it back to `available`. This is enforced in the
//! same transaction as the admission/discharge to prevent double-allocation.

use sqlx::PgPool;

use crate::audit;
use crate::models::{Bed, CreateIpdAdmission, DischargeIpd, IpdAdmission, Ward};
use crate::rbac::{self, Permission, SessionState};

// ── Wards ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_wards(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<Vec<Ward>, String> {
    let _ = rbac::require(&session, Permission::IpdView)?;
    sqlx::query_as("SELECT * FROM wards WHERE is_active = TRUE ORDER BY name")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Get wards: {}", e))
}

#[tauri::command]
pub async fn create_ward(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    name: String,
    code: String,
    floor: Option<String>,
    gender_restriction: Option<String>,
) -> Result<i32, String> {
    let s = rbac::require(&session, Permission::BedsManage)?;
    let row: (i32,) = sqlx::query_as(
        "INSERT INTO wards (name, code, floor, gender_restriction) VALUES ($1,$2,$3,$4) RETURNING id",
    )
    .bind(&name).bind(&code).bind(&floor).bind(&gender_restriction)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| format!("Create ward: {}", e))?;
    audit::for_session(pool.inner(), &s, "ward_create", "wards", Some(&row.0.to_string()), None).await;
    Ok(row.0)
}

// ── Beds ──────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_beds(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    ward_id: Option<i32>,
) -> Result<Vec<Bed>, String> {
    let _ = rbac::require(&session, Permission::IpdView)?;
    match ward_id {
        Some(w) => sqlx::query_as("SELECT * FROM beds WHERE ward_id = $1 ORDER BY bed_number")
            .bind(w).fetch_all(pool.inner()).await,
        None => sqlx::query_as("SELECT * FROM beds ORDER BY ward_id, bed_number")
            .fetch_all(pool.inner()).await,
    }
    .map_err(|e| format!("Get beds: {}", e))
}

#[tauri::command]
pub async fn create_bed(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    ward_id: i32,
    bed_number: String,
    is_icu: Option<bool>,
    daily_rate: Option<f64>,
) -> Result<i32, String> {
    let s = rbac::require(&session, Permission::BedsManage)?;
    let row: (i32,) = sqlx::query_as(
        "INSERT INTO beds (ward_id, bed_number, is_icu, daily_rate) VALUES ($1,$2,$3,$4) RETURNING id",
    )
    .bind(ward_id).bind(&bed_number)
    .bind(is_icu.unwrap_or(false))
    .bind(rust_decimal::Decimal::from_f64_retain(daily_rate.unwrap_or(0.0)).unwrap_or_default())
    .fetch_one(pool.inner())
    .await
    .map_err(|e| format!("Create bed: {}", e))?;
    audit::for_session(pool.inner(), &s, "bed_create", "beds", Some(&row.0.to_string()), None).await;
    Ok(row.0)
}

// ── Admissions ────────────────────────────────────────────────────────────────

const SELECT_ADMISSIONS: &str = r#"
    SELECT a.id, a.patient_id, a.doctor_id, a.ward_id, a.bed_id, a.admission_date,
           a.admission_type, a.admitting_diagnosis, a.attending_doctor_id, a.status,
           a.discharge_date, a.discharge_summary, a.created_by_user_id,
           a.created_at, a.updated_at,
           p.first_name || ' ' || p.last_name AS patient_name,
           d.first_name || ' ' || d.last_name AS doctor_name,
           w.name AS ward_name, b.bed_number AS bed_number
    FROM ipd_admissions a
    LEFT JOIN patients p ON p.id = a.patient_id
    LEFT JOIN doctors d ON d.id = a.doctor_id
    LEFT JOIN wards w ON w.id = a.ward_id
    LEFT JOIN beds b ON b.id = a.bed_id
"#;

#[tauri::command]
pub async fn get_admissions(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    status_filter: Option<String>,
) -> Result<Vec<IpdAdmission>, String> {
    let _ = rbac::require(&session, Permission::IpdView)?;
    let q = match status_filter.as_deref() {
        Some(s) if !s.is_empty() => format!("{} WHERE a.status = $1 ORDER BY a.admission_date DESC", SELECT_ADMISSIONS),
        _ => format!("{} ORDER BY a.admission_date DESC", SELECT_ADMISSIONS),
    };
    let mut query = sqlx::query_as::<_, IpdAdmission>(&q);
    if let Some(s) = status_filter.filter(|s| !s.is_empty()) {
        query = query.bind(s);
    }
    query.fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Get admissions: {}", e))
}

#[tauri::command]
pub async fn admit_patient(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    admission: CreateIpdAdmission,
) -> Result<i32, String> {
    let s = rbac::require_strong(&session, pool.inner(), Permission::IpdManage).await?;

    // ── Atomic bed allocation per SDD §8.1 ──────────────────────────────────
    //
    // The bed-availability check AND the status flip happen inside the SAME
    // transaction. The UPDATE is conditional on `status='available'` and we
    // verify `rows_affected() == 1` — so two concurrent admissions to the
    // same bed cannot both succeed: the second UPDATE matches zero rows and
    // we roll back. (The previous implementation checked status outside the
    // tx and used an unconditional UPDATE, allowing a TOCTOU double-allocation.)
    let mut tx = pool.begin().await.map_err(|e| format!("Begin tx: {}", e))?;

    // Insert the admission row first (the admission itself is always valid).
    let row: (i32,) = sqlx::query_as(
        r#"
        INSERT INTO ipd_admissions
            (patient_id, doctor_id, ward_id, bed_id, admission_type,
             admitting_diagnosis, attending_doctor_id, status, created_by_user_id)
        VALUES ($1,$2,$3,$4,$5,$6,$7,'admitted',$8) RETURNING id
        "#,
    )
    .bind(admission.patient_id)
    .bind(admission.doctor_id)
    .bind(admission.ward_id)
    .bind(admission.bed_id)
    .bind(admission.admission_type.as_deref().unwrap_or("routine"))
    .bind(&admission.admitting_diagnosis)
    .bind(admission.attending_doctor_id)
    .bind(s.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| format!("Admit: {}", e))?;

    // Conditional UPDATE — only flips the bed if it is STILL available.
    // rows_affected == 0 means another admission grabbed it first (or the
    // bed doesn't exist) → we roll back the admission insert and return an
    // error. This is the SDD §8.1 atomic pattern.
    let updated: (i64,) = sqlx::query_as(
        "UPDATE beds SET status='occupied', updated_at=NOW() WHERE id=$1 AND status='available' RETURNING 1::bigint",
    )
    .bind(admission.bed_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| format!("Occupy bed: {}", e))?
    .unwrap_or((0,));

    if updated.0 == 0 {
        // Roll back the admission insert — the bed is no longer available.
        tx.rollback().await.ok();
        return Err(
            "The selected bed is no longer available (it may have just been assigned to another patient). \
             Please choose a different bed and try again."
                .to_string(),
        );
    }

    tx.commit().await.map_err(|e| format!("Commit: {}", e))?;

    audit::for_session(pool.inner(), &s, "ipd_admit", "ipd_admissions",
        Some(&row.0.to_string()),
        Some(serde_json::json!({"patient_id": admission.patient_id, "bed_id": admission.bed_id}))).await;
    Ok(row.0)
}

#[tauri::command]
pub async fn discharge_patient(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    discharge: DischargeIpd,
) -> Result<(), String> {
    let s = rbac::require_strong(&session, pool.inner(), Permission::IpdManage).await?;

    // FUN-09: fetch BOTH patient_id and bed_id in the same query — we need
    // patient_id to run the unpaid-bills guard below, and bed_id to free
    // the bed on successful discharge. The previous query only fetched
    // bed_id, so the billing check would have needed a second round-trip.
    let row: Option<(i32, i32)> = sqlx::query_as(
        "SELECT patient_id, bed_id FROM ipd_admissions WHERE id = $1 AND status = 'admitted'",
    )
    .bind(discharge.id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    let (patient_id, bed_id) = row
        .ok_or_else(|| "Admission not found or already discharged.".to_string())?;

    // FUN-09: discharge-billing check. Refuse to discharge a patient who
    // has outstanding unpaid or partially-paid bills — this prevents the
    // common operational error of discharging a patient before the
    // final IPD bill is settled, after which chasing payment is much
    // harder (the patient has left the facility).
    //
    // The check covers bills of ANY type (opd / ipd / pharmacy) for the
    // patient, not just bills linked to this admission — a patient with
    // an unrelated unpaid OPD bill from a prior visit should also be
    // flagged before they walk out.
    //
    // The check is best-effort: if the bills query itself fails (e.g.
    // transient DB error), we sanitize the error and surface a generic
    // "Database operation failed" message — we do NOT let the discharge
    // proceed silently. Fail-closed is the safe default for a billing
    // control.
    let unpaid: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM bills WHERE patient_id = $1 AND status IN ('unpaid', 'partial')",
    )
    .bind(patient_id)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    if unpaid.0 > 0 {
        // Audit the blocked discharge so there's a trace of why the
        // discharge was refused (the operator may need to chase payment
        // or write off the balance, and the audit row is the evidence
        // that the control fired).
        audit::for_session(
            pool.inner(),
            &s,
            "ipd_discharge_blocked_unpaid_bills",
            "ipd_admissions",
            Some(&discharge.id.to_string()),
            Some(serde_json::json!({
                "patient_id": patient_id,
                "unpaid_bill_count": unpaid.0,
            })),
        )
        .await;
        return Err(format!(
            "Cannot discharge: patient has {} unpaid bill(s). \
             Please settle or write off the balance before discharge.",
            unpaid.0
        ));
    }

    let mut tx = pool.begin().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    sqlx::query(
        "UPDATE ipd_admissions SET status='discharged', discharge_date=NOW(),
                                    discharge_summary=$1, updated_at=NOW() WHERE id=$2",
    )
    .bind(&discharge.discharge_summary)
    .bind(discharge.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    sqlx::query("UPDATE beds SET status='available' WHERE id=$1")
        .bind(bed_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    tx.commit().await.map_err(|e| crate::db::sanitize_db_error(&e))?;

    audit::for_session(pool.inner(), &s, "ipd_discharge", "ipd_admissions",
        Some(&discharge.id.to_string()), None).await;
    Ok(())
}
