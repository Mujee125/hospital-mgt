//! Doctor / staff-clinician commands — RBAC-guarded and audited.

use sqlx::PgPool;

use crate::audit;
use crate::models::{CreateDoctor, Doctor, UpdateDoctor};
use crate::rbac::{self, Permission, SessionState};

#[tauri::command]
pub async fn create_doctor(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    doctor: CreateDoctor,
) -> Result<i32, String> {
    let s = rbac::require(&session, Permission::DoctorsManage)?;
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

    audit::for_session(pool.inner(), &s, "doctor_create", "doctors",
        Some(&row.0.to_string()), None).await;
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

    audit::for_session(pool.inner(), &s, "doctor_update", "doctors",
        Some(&doctor.id.to_string()), None).await;
    Ok(())
}

#[tauri::command]
pub async fn delete_doctor(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
) -> Result<(), String> {
    let s = rbac::require(&session, Permission::DoctorsManage)?;
    sqlx::query("DELETE FROM doctors WHERE id = $1")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("Delete failed: {}", e))?;
    audit::for_session(pool.inner(), &s, "doctor_delete", "doctors",
        Some(&id.to_string()), None).await;
    Ok(())
}

#[tauri::command]
pub async fn get_specializations(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<Vec<String>, String> {
    let _ = rbac::require(&session, Permission::DoctorsView)?;
    sqlx::query_scalar(
        "SELECT DISTINCT specialization FROM doctors ORDER BY specialization",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| format!("Failed to get specializations: {}", e))
}
