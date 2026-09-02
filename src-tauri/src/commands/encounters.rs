//! Encounter / visit management — the clinical record of a patient visit.
//! RBAC-guarded + audited. Encounters link appointments, lab orders, and bills.

use sqlx::PgPool;

use crate::audit;
use crate::models::CreateEncounter;
use crate::rbac::{self, Permission, SessionState};

const SELECT_ENCOUNTERS: &str = r#"
    SELECT e.id, e.patient_id, e.doctor_id, e.visit_type, e.visit_date,
           e.chief_complaint, e.diagnosis, e.notes, e.created_by_user_id, e.created_at,
           p.first_name || ' ' || p.last_name AS patient_name
    FROM encounters e
    LEFT JOIN patients p ON p.id = e.patient_id
"#;

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct EncounterWithPatient {
    pub id: i32,
    pub patient_id: i32,
    pub doctor_id: Option<i32>,
    pub visit_type: String,
    pub visit_date: chrono::DateTime<chrono::Utc>,
    pub chief_complaint: Option<String>,
    pub diagnosis: Option<String>,
    pub notes: Option<String>,
    pub created_by_user_id: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub patient_name: Option<String>,
}

#[tauri::command]
pub async fn get_encounters(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    patient_id: Option<i32>,
) -> Result<Vec<EncounterWithPatient>, String> {
    let _ = rbac::require(&session, Permission::PatientsView)?;
    let q = match patient_id {
        Some(_pid) => format!("{} WHERE e.patient_id = $1 ORDER BY e.visit_date DESC", SELECT_ENCOUNTERS),
        None => format!("{} ORDER BY e.visit_date DESC", SELECT_ENCOUNTERS),
    };
    let mut query = sqlx::query_as::<_, EncounterWithPatient>(&q);
    if let Some(pid) = patient_id {
        query = query.bind(pid);
    }
    query.fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Get encounters: {}", e))
}

#[tauri::command]
pub async fn create_encounter(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    encounter: CreateEncounter,
) -> Result<i32, String> {
    let s = rbac::require_strong(&session, pool.inner(), Permission::PatientsUpdate).await?;
    let row: (i32,) = sqlx::query_as(
        r#"INSERT INTO encounters
              (patient_id, doctor_id, visit_type, chief_complaint, diagnosis, notes, created_by_user_id)
           VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id"#,
    )
    .bind(encounter.patient_id)
    .bind(encounter.doctor_id)
    .bind(encounter.visit_type.as_deref().unwrap_or("opd"))
    .bind(&encounter.chief_complaint)
    .bind(&encounter.diagnosis)
    .bind(&encounter.notes)
    .bind(s.user_id)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| format!("Create encounter: {}", e))?;

    audit::for_session(pool.inner(), &s, "encounter_create", "encounters",
        Some(&row.0.to_string()),
        Some(serde_json::json!({"patient_id": encounter.patient_id}))).await;
    Ok(row.0)
}
