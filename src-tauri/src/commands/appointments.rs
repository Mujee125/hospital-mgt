//! Appointment commands — RBAC-guarded and audited. WhatsApp notification
//! logic preserved exactly from the prior implementation.

use chrono::NaiveDate;
use sqlx::PgPool;

use crate::audit;
use crate::config::AppConfig;
use crate::models::{
    AppointmentStats, AppointmentWithDetails, CreateAppointment, UpdateAppointment,
};
use crate::rbac::{self, Permission, SessionState};
use crate::whatsapp::{self, WhatsAppMessage};

// ── helpers ──────────────────────────────────────────────────────────────────

/// QA-2026-09-08 H3 (3.2-1): valid appointment statuses. Mirrors the
/// chk_appointments_status CHECK constraint added to the schema — the
/// command layer rejects invalid values with a clear message instead of
/// letting Postgres raise the constraint error (or worse, in older DBs
/// without the CHECK, persisting a garbage status that silently skips
/// the confirmed/cancelled WhatsApp triggers and the stats FILTERs).
const APPOINTMENT_STATUSES: [&str; 5] = [
    "scheduled",
    "confirmed",
    "completed",
    "cancelled",
    "no-show",
];

fn validate_appointment_status(status: &str) -> Result<(), String> {
    if APPOINTMENT_STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(format!(
            "Invalid status '{}'. Valid values: {}.",
            status,
            APPOINTMENT_STATUSES.join(", ")
        ))
    }
}

/// QA-2026-09-08 H3: refuse a booking that overlaps an existing
/// non-cancelled/no-show appointment for the same doctor. Previously the
/// INSERT went in blind — two receptions booking the same slot produced
/// a double-booked doctor with no error until the patients both showed
/// up. Active statuses only (cancelled/no-show slots stay bookable).
/// Timezone note: `appointment_time` is clinic-local; `::time` casts on
/// both sides keep the comparison in local wall-clock time, consistent
/// with how the rest of the module stores times.
async fn check_doctor_overlap(
    pool: &PgPool,
    doctor_id: i32,
    date: chrono::NaiveDate,
    start_min: i32,
    duration_minutes: i32,
    exclude_appointment_id: Option<i32>,
) -> Result<(), String> {
    let end_min = start_min + duration_minutes.max(1);
    let id_clause = match exclude_appointment_id {
        Some(_id) => " AND a.id != $5",
        None => "",
    };
    let overlap: Option<(i64,)> = sqlx::query_as(&format!(
        r#"
        SELECT 1 FROM appointments a
        WHERE a.doctor_id = $1
          AND a.appointment_date = $2
          AND a.status NOT IN ('cancelled', 'no-show')
          AND EXTRACT(EPOCH FROM a.appointment_time) / 60 < $4
          AND (EXTRACT(EPOCH FROM a.appointment_time) / 60) + a.duration_minutes > $3
          {id_clause}
        LIMIT 1
        "#,
    ))
    .bind(doctor_id)
    .bind(date)
    .bind(start_min)
    .bind(end_min)
    .fetch_optional(pool)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;
    if overlap.is_some() {
        return Err(
            "This doctor already has an appointment that overlaps the selected time. Choose a different time or doctor.".to_string(),
        );
    }
    Ok(())
}

/// "HH:MM" → minutes past midnight. None for malformed input (the
/// TIME-cast error later gives the field-named message).
fn time_to_minutes(t: &str) -> Option<i32> {
    let mut it = t.split(':');
    let h: i32 = it.next()?.parse().ok()?;
    let m: i32 = it.next()?.parse().ok()?;
    if h < 0 || !(0..=59).contains(&m) {
        return None;
    }
    Some(h * 60)
}

async fn get_appt_details(
    pool: &PgPool,
    id: i32,
) -> Result<(i32, String, String, String, String, String), String> {
    sqlx::query_as::<_, (i32, String, String, String, String, String)>(
        r#"
        SELECT
            a.patient_id,
            p.first_name || ' ' || p.last_name          AS patient_name,
            p.phone                                       AS patient_phone,
            d.first_name || ' ' || d.last_name          AS doctor_name,
            TO_CHAR(a.appointment_date, 'DD Mon YYYY')  AS appt_date,
            TO_CHAR(a.appointment_time, 'HH12:MI AM')   AS appt_time
        FROM appointments a
        JOIN patients p ON p.id = a.patient_id
        JOIN doctors  d ON d.id = a.doctor_id
        WHERE a.id = $1
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Failed to fetch appointment details: {}", e))
}

fn clinic_name(app_handle: &tauri::AppHandle) -> String {
    AppConfig::load(app_handle)
        .map(|c| c.clinic_name)
        .unwrap_or_else(|| "VitalFlow Clinic".to_string())
}

async fn fire_whatsapp(app_handle: &tauri::AppHandle, pool: &PgPool, msg: WhatsAppMessage) {
    let ah = app_handle.clone();
    let p = pool.clone();
    tokio::spawn(async move {
        if let Err(e) = whatsapp::send_whatsapp(&ah, &p, msg).await {
            eprintln!("[HMS WA] Notification failed: {}", e);
        }
    });
}

const SELECT_WITH_DETAILS: &str = r#"
    SELECT
        a.id, a.patient_id, a.doctor_id,
        a.appointment_date, a.appointment_time,
        a.duration_minutes, a.status,
        a.reason, a.notes, a.created_at, a.updated_at,
        p.first_name AS patient_first_name,
        p.last_name  AS patient_last_name,
        d.first_name AS doctor_first_name,
        d.last_name  AS doctor_last_name,
        d.specialization AS doctor_specialization
    FROM appointments a
    JOIN patients p ON p.id = a.patient_id
    JOIN doctors  d ON d.id = a.doctor_id
"#;

// ── Create appointment ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn create_appointment(
    app_handle: tauri::AppHandle,
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    appointment: CreateAppointment,
) -> Result<i32, String> {
    let s = rbac::require(&session, Permission::AppointmentsCreate)?;
    let date = NaiveDate::parse_from_str(&appointment.appointment_date, "%Y-%m-%d")
        .map_err(|_| "Invalid date format. Use YYYY-MM-DD.".to_string())?;

    // H3: duration is stored per-appointment (default 30) but was never
    // bounded — clamp so a typo'd 10000-minute booking can't make a doctor
    // permanently "busy" and can't overflow the overlap arithmetic below.
    let duration = appointment.duration_minutes.unwrap_or(30).clamp(5, 480);

    if let Some(start_min) = time_to_minutes(&appointment.appointment_time) {
        check_doctor_overlap(
            pool.inner(),
            appointment.doctor_id,
            date,
            start_min,
            duration,
            None,
        )
        .await?;
    }

    let row: (i32,) = sqlx::query_as(
        r#"
        INSERT INTO appointments
            (patient_id, doctor_id, appointment_date, appointment_time,
             duration_minutes, reason, notes, created_by_user_id)
        VALUES ($1, $2, $3, $4::TIME, $5, $6, $7, $8)
        RETURNING id
        "#,
    )
    .bind(appointment.patient_id)
    .bind(appointment.doctor_id)
    .bind(date)
    .bind(&appointment.appointment_time)
    .bind(duration)
    .bind(&appointment.reason)
    .bind(&appointment.notes)
    .bind(s.user_id)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| {
        // F-23: the EXCLUDE constraint is the atomic backstop — a race the
        // app-level pre-check missed lands here; surface the same friendly
        // message the pre-check uses.
        if e.to_string().contains("excl_appt_doctor_slot") {
            "This doctor already has an appointment that overlaps the selected time. Choose a different time or doctor.".to_string()
        } else {
            format!("Failed to create appointment: {}", e)
        }
    })?;

    let appt_id = row.0;

    if let Ok((patient_id, patient_name, phone, doctor_name, date_str, time_str)) =
        get_appt_details(pool.inner(), appt_id).await
    {
        let clinic = clinic_name(&app_handle);
        let msg_text = whatsapp::build_appointment_booked_msg(
            &clinic,
            &patient_name,
            &doctor_name,
            &date_str,
            &time_str,
        );
        fire_whatsapp(
            &app_handle,
            pool.inner(),
            WhatsAppMessage {
                recipient: phone,
                message: msg_text,
                is_group: false,
                appointment_id: Some(appt_id),
                notification_type: "booked".to_string(),
                patient_id: Some(patient_id),
            },
        )
        .await;

        // Phase 9: in-app notification for the front desk (best-effort —
        // never fails the booking; the WhatsApp send above is
        // fire-and-forget for the same reason).
        let title = format!(
            "New appointment: {} — {} {}",
            patient_name, date_str, time_str
        );
        let body = format!(
            "{} booked with {} on {} at {}.",
            patient_name, doctor_name, date_str, time_str
        );
        if let Err(e) = crate::commands::notifications::emit(
            pool.inner(),
            crate::commands::notifications::NotificationOut {
                user_id: None,
                role_target: Some("receptionist".into()),
                kind: "appointment_booked".into(),
                title,
                body,
                entity_type: Some("appointment".into()),
                entity_id: Some(appt_id),
            },
        )
        .await
        {
            eprintln!(
                "[HMS Appointments] notification emit failed (non-fatal): {}",
                e
            );
        }
    }

    audit::for_session(pool.inner(), &s, "appointment_create", "appointments",
        Some(&appt_id.to_string()),
        Some(serde_json::json!({"patient_id": appointment.patient_id, "doctor_id": appointment.doctor_id}))).await;
    Ok(appt_id)
}

// ── Get all appointments ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_appointments(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    date_filter: Option<String>,
    status_filter: Option<String>,
    doctor_filter: Option<i32>,
) -> Result<Vec<AppointmentWithDetails>, String> {
    let _ = rbac::require(&session, Permission::AppointmentsView)?;

    let mut query = format!("{} WHERE 1=1", SELECT_WITH_DETAILS);
    if date_filter.is_some() {
        query.push_str(" AND a.appointment_date = $1");
    }
    if status_filter.is_some() {
        query.push_str(if date_filter.is_some() {
            " AND a.status = $2"
        } else {
            " AND a.status = $1"
        });
    }
    if doctor_filter.is_some() {
        let n = 1 + date_filter.is_some() as i32 + status_filter.is_some() as i32;
        query.push_str(&format!(" AND a.doctor_id = ${}", n));
    }
    query.push_str(" ORDER BY a.appointment_date DESC, a.appointment_time ASC");

    let mut q = sqlx::query_as::<_, AppointmentWithDetails>(&query);
    if let Some(ref d) = date_filter {
        q = q.bind(d);
    }
    if let Some(ref s) = status_filter {
        q = q.bind(s);
    }
    if let Some(doc) = doctor_filter {
        q = q.bind(doc);
    }

    q.fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Failed to get appointments: {}", e))
}

#[tauri::command]
pub async fn get_appointment(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
) -> Result<AppointmentWithDetails, String> {
    let _ = rbac::require(&session, Permission::AppointmentsView)?;
    let q = format!("{} WHERE a.id = $1", SELECT_WITH_DETAILS);
    sqlx::query_as::<_, AppointmentWithDetails>(&q)
        .bind(id)
        .fetch_one(pool.inner())
        .await
        .map_err(|e| format!("Appointment not found: {}", e))
}

#[tauri::command]
pub async fn update_appointment(
    app_handle: tauri::AppHandle,
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    appointment: UpdateAppointment,
) -> Result<(), String> {
    let s = rbac::require(&session, Permission::AppointmentsUpdate)?;

    let old_status: Option<(String,)> =
        sqlx::query_as("SELECT status FROM appointments WHERE id = $1")
            .bind(appointment.id)
            .fetch_optional(pool.inner())
            .await
            .map_err(|e| format!("Status fetch failed: {}", e))?;

    let date = NaiveDate::parse_from_str(&appointment.appointment_date, "%Y-%m-%d")
        .map_err(|_| "Invalid date format.".to_string())?;
    validate_appointment_status(&appointment.status)?;
    let duration = appointment.duration_minutes.clamp(5, 480);

    if let Some(start_min) = time_to_minutes(&appointment.appointment_time) {
        check_doctor_overlap(
            pool.inner(),
            appointment.doctor_id,
            date,
            start_min,
            duration,
            Some(appointment.id),
        )
        .await?;
    }

    sqlx::query(
        r#"
        UPDATE appointments SET
            patient_id = $1, doctor_id = $2,
            appointment_date = $3, appointment_time = $4::TIME,
            duration_minutes = $5, status = $6,
            reason = $7, notes = $8, updated_at = NOW()
        WHERE id = $9
        "#,
    )
    .bind(appointment.patient_id)
    .bind(appointment.doctor_id)
    .bind(date)
    .bind(&appointment.appointment_time)
    .bind(duration)
    .bind(&appointment.status)
    .bind(&appointment.reason)
    .bind(&appointment.notes)
    .bind(appointment.id)
    .execute(pool.inner())
    .await
    .map_err(|e| {
        // F-23: friendly mapping for the EXCLUDE backstop (reschedule races).
        if e.to_string().contains("excl_appt_doctor_slot") {
            "This doctor already has an appointment that overlaps the selected time. Choose a different time or doctor.".to_string()
        } else {
            format!("Update failed: {}", e)
        }
    })?;

    let prev = old_status.map(|x| x.0).unwrap_or_default();
    let next = appointment.status.as_str();
    if prev != next {
        if let Ok((patient_id, patient_name, phone, doctor_name, date_str, time_str)) =
            get_appt_details(pool.inner(), appointment.id).await
        {
            let clinic = clinic_name(&app_handle);
            let (msg_text, ntype) = match next {
                "confirmed" => (
                    whatsapp::build_appointment_confirmed_msg(
                        &clinic,
                        &patient_name,
                        &doctor_name,
                        &date_str,
                        &time_str,
                    ),
                    "confirmed",
                ),
                "cancelled" => (
                    whatsapp::build_appointment_cancelled_msg(
                        &clinic,
                        &patient_name,
                        &doctor_name,
                        &date_str,
                        &time_str,
                    ),
                    "cancelled",
                ),
                _ => (String::new(), ""),
            };
            if !msg_text.is_empty() {
                fire_whatsapp(
                    &app_handle,
                    pool.inner(),
                    WhatsAppMessage {
                        recipient: phone,
                        message: msg_text,
                        is_group: false,
                        appointment_id: Some(appointment.id),
                        notification_type: ntype.to_string(),
                        patient_id: Some(patient_id),
                    },
                )
                .await;
            }
        }
    }

    audit::for_session(
        pool.inner(),
        &s,
        "appointment_update",
        "appointments",
        Some(&appointment.id.to_string()),
        Some(serde_json::json!({"prev_status": prev, "next_status": next})),
    )
    .await;
    Ok(())
}

#[tauri::command]
pub async fn update_appointment_status(
    app_handle: tauri::AppHandle,
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
    status: String,
) -> Result<(), String> {
    let s = rbac::require(&session, Permission::AppointmentsUpdate)?;
    validate_appointment_status(&status)?;

    let old: Option<(String,)> = sqlx::query_as("SELECT status FROM appointments WHERE id = $1")
        .bind(id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("UPDATE appointments SET status = $1, updated_at = NOW() WHERE id = $2")
        .bind(&status)
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("Status update failed: {}", e))?;

    let prev = old.map(|x| x.0).unwrap_or_default();
    if prev != status {
        if let Ok((patient_id, patient_name, phone, doctor_name, date_str, time_str)) =
            get_appt_details(pool.inner(), id).await
        {
            let clinic = clinic_name(&app_handle);
            let (msg_text, ntype) = match status.as_str() {
                "confirmed" => (
                    whatsapp::build_appointment_confirmed_msg(
                        &clinic,
                        &patient_name,
                        &doctor_name,
                        &date_str,
                        &time_str,
                    ),
                    "confirmed",
                ),
                "cancelled" => (
                    whatsapp::build_appointment_cancelled_msg(
                        &clinic,
                        &patient_name,
                        &doctor_name,
                        &date_str,
                        &time_str,
                    ),
                    "cancelled",
                ),
                _ => (String::new(), ""),
            };
            if !msg_text.is_empty() {
                fire_whatsapp(
                    &app_handle,
                    pool.inner(),
                    WhatsAppMessage {
                        recipient: phone,
                        message: msg_text,
                        is_group: false,
                        appointment_id: Some(id),
                        notification_type: ntype.to_string(),
                        patient_id: Some(patient_id),
                    },
                )
                .await;
            }
        }
    }

    audit::for_session(
        pool.inner(),
        &s,
        "appointment_status_change",
        "appointments",
        Some(&id.to_string()),
        Some(serde_json::json!({"prev": prev, "next": status})),
    )
    .await;
    Ok(())
}

#[tauri::command]
pub async fn delete_appointment(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
) -> Result<(), String> {
    let s = rbac::require(&session, Permission::AppointmentsDelete)?;
    sqlx::query("DELETE FROM appointments WHERE id = $1")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("Delete failed: {}", e))?;
    audit::for_session(
        pool.inner(),
        &s,
        "appointment_delete",
        "appointments",
        Some(&id.to_string()),
        None,
    )
    .await;
    Ok(())
}

#[tauri::command]
pub async fn get_today_appointments(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<Vec<AppointmentWithDetails>, String> {
    let _ = rbac::require(&session, Permission::AppointmentsView)?;
    let q = format!(
        "{} WHERE a.appointment_date = CURRENT_DATE ORDER BY a.appointment_time ASC",
        SELECT_WITH_DETAILS
    );
    sqlx::query_as::<_, AppointmentWithDetails>(&q)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Failed to get today appointments: {}", e))
}

#[tauri::command]
pub async fn get_appointment_stats(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<AppointmentStats, String> {
    let _ = rbac::require(&session, Permission::AppointmentsView)?;
    let row: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*)                                          AS total,
            COUNT(*) FILTER (WHERE status = 'scheduled')     AS scheduled,
            COUNT(*) FILTER (WHERE status = 'confirmed')     AS confirmed,
            COUNT(*) FILTER (WHERE status = 'completed')     AS completed,
            COUNT(*) FILTER (WHERE status = 'cancelled')     AS cancelled,
            COUNT(*) FILTER (WHERE status = 'no-show')       AS no_show
        FROM appointments
        "#,
    )
    .fetch_one(pool.inner())
    .await
    .map_err(|e| format!("Stats query failed: {}", e))?;

    Ok(AppointmentStats {
        total: row.0,
        scheduled: row.1,
        confirmed: row.2,
        completed: row.3,
        cancelled: row.4,
        no_show: row.5,
    })
}
