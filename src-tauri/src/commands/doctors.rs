//! Doctor / staff-clinician commands — RBAC-guarded and audited.

use sqlx::PgPool;

use crate::audit;
use crate::models::{CreateDoctor, Doctor, UpdateDoctor};
use crate::rbac::{self, Permission, SessionState};

/// QA-2026-09-08 (3.2-1): shared input validation for create/update.
/// Empty names previously sailed through to a raw `$7::TIME` cast error
/// from Postgres when the time format was bad — validate shape here so
/// the frontend gets a precise, field-named message.
fn validate_doctor_fields(
    first_name: &str,
    last_name: &str,
    specialization: &str,
    available_from: &str,
    available_to: &str,
) -> Result<(), String> {
    if first_name.trim().is_empty() {
        return Err("First name is required.".to_string());
    }
    if last_name.trim().is_empty() {
        return Err("Last name is required.".to_string());
    }
    if specialization.trim().is_empty() {
        return Err("Specialization is required.".to_string());
    }
    // HH:MM or HH:MM:SS — reject anything else before Postgres's TIME cast
    // turns it into an opaque error.
    let valid_time = |t: &str| {
        let parts: Vec<&str> = t.split(':').collect();
        match parts.as_slice() {
            [h, m] => {
                h.len() == 2
                    && h.parse::<u32>().map(|v| v < 24).unwrap_or(false)
                    && m.len() == 2
                    && m.parse::<u32>().map(|v| v < 60).unwrap_or(false)
            }
            [h, m, s] => {
                h.len() == 2
                    && h.parse::<u32>().map(|v| v < 24).unwrap_or(false)
                    && m.len() == 2
                    && m.parse::<u32>().map(|v| v < 60).unwrap_or(false)
                    && s.len() == 2
                    && s.parse::<u32>().map(|v| v < 60).unwrap_or(false)
            }
            _ => false,
        }
    };
    if !valid_time(available_from) {
        return Err(format!(
            "'{}' is not a valid start time. Use HH:MM (24-hour).",
            available_from
        ));
    }
    if !valid_time(available_to) {
        return Err(format!(
            "'{}' is not a valid end time. Use HH:MM (24-hour).",
            available_to
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn create_doctor(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    doctor: CreateDoctor,
) -> Result<i32, String> {
    let s = rbac::require(&session, Permission::DoctorsManage)?;
    validate_doctor_fields(
        &doctor.first_name,
        &doctor.last_name,
        &doctor.specialization,
        &doctor.available_from,
        &doctor.available_to,
    )?;
    let row: (i32,) = sqlx::query_as(
        r#"
        INSERT INTO doctors
            (first_name, last_name, email, phone, specialization,
             qualification, available_from, available_to)
        VALUES ($1, $2, $3, $4, $5, $6, $7::TIME, $8::TIME)
        RETURNING id
        "#,
    )
    .bind(&doctor.first_name)
    .bind(&doctor.last_name)
    .bind(&doctor.email)
    .bind(&doctor.phone)
    .bind(&doctor.specialization)
    .bind(&doctor.qualification)
    .bind(&doctor.available_from)
    .bind(&doctor.available_to)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| format!("Failed to create doctor: {}", e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "doctor_create",
        "doctors",
        Some(&row.0.to_string()),
        None,
    )
    .await;
    Ok(row.0)
}

#[tauri::command]
pub async fn get_doctors(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    active_only: Option<bool>,
) -> Result<Vec<Doctor>, String> {
    let _ = rbac::require(&session, Permission::DoctorsView)?;
    let doctors = if active_only.unwrap_or(false) {
        sqlx::query_as("SELECT * FROM doctors WHERE is_active = TRUE ORDER BY last_name")
            .fetch_all(pool.inner())
            .await
    } else {
        sqlx::query_as("SELECT * FROM doctors ORDER BY last_name")
            .fetch_all(pool.inner())
            .await
    };
    doctors.map_err(|e| format!("Failed to get doctors: {}", e))
}

#[tauri::command]
pub async fn get_doctor(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
) -> Result<Doctor, String> {
    let _ = rbac::require(&session, Permission::DoctorsView)?;
    sqlx::query_as("SELECT * FROM doctors WHERE id = $1")
        .bind(id)
        .fetch_one(pool.inner())
        .await
        .map_err(|e| format!("Doctor not found: {}", e))
}

#[tauri::command]
pub async fn update_doctor(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    doctor: UpdateDoctor,
) -> Result<(), String> {
    let s = rbac::require(&session, Permission::DoctorsManage)?;
    validate_doctor_fields(
        &doctor.first_name,
        &doctor.last_name,
        &doctor.specialization,
        &doctor.available_from,
        &doctor.available_to,
    )?;
    sqlx::query(
        r#"
        UPDATE doctors SET
            first_name = $1, last_name = $2, email = $3,
            phone = $4, specialization = $5, qualification = $6,
            available_from = $7::TIME, available_to = $8::TIME,
            is_active = $9
        WHERE id = $10
        "#,
    )
    .bind(&doctor.first_name)
    .bind(&doctor.last_name)
    .bind(&doctor.email)
    .bind(&doctor.phone)
    .bind(&doctor.specialization)
    .bind(&doctor.qualification)
    .bind(&doctor.available_from)
    .bind(&doctor.available_to)
    .bind(doctor.is_active)
    .bind(doctor.id)
    .execute(pool.inner())
    .await
    .map_err(|e| format!("Update failed: {}", e))?;

    audit::for_session(
        pool.inner(),
        &s,
        "doctor_update",
        "doctors",
        Some(&doctor.id.to_string()),
        None,
    )
    .await;
    Ok(())
}

#[tauri::command]
pub async fn delete_doctor(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
) -> Result<(), String> {
    let s = rbac::require(&session, Permission::DoctorsManage)?;

    // QA-2026-09-08 M7 (HIPAA §164.530(j)): deleting a doctor used to
    // CASCADE-delete every appointment (the schema's one clinical-history
    // CASCADE FK) — silently destroying booking history, WhatsApp
    // notification links and stats. The FK is now RESTRICT; this pre-check
    // turns the DB's raw "violates foreign key constraint" into a clear,
    // actionable message. Deactivating (is_active = false) is the
    // retention-compliant path for departing doctors.
    let appointments: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM appointments WHERE doctor_id = $1")
            .bind(id)
            .fetch_one(pool.inner())
            .await
            .map_err(|e| crate::db::sanitize_db_error(&e))?;
    if appointments.0 > 0 {
        return Err(format!(
            "This doctor has {} appointment(s) on record. Deleting them would destroy clinical history — mark the doctor as inactive instead.",
            appointments.0
        ));
    }

    sqlx::query("DELETE FROM doctors WHERE id = $1")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;
    audit::for_session(
        pool.inner(),
        &s,
        "doctor_delete",
        "doctors",
        Some(&id.to_string()),
        None,
    )
    .await;
    Ok(())
}

#[tauri::command]
pub async fn get_specializations(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<Vec<String>, String> {
    let _ = rbac::require(&session, Permission::DoctorsView)?;
    sqlx::query_scalar("SELECT DISTINCT specialization FROM doctors ORDER BY specialization")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Failed to get specializations: {}", e))
}
