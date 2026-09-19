//! Appointment commands — RBAC-guarded and audited. WhatsApp notification
//! logic preserved exactly from the prior implementation.

use chrono::NaiveDate;
use sqlx::PgPool;

use crate::audit;
use crate::config::AppConfig;
use crate::models::{
    AppointmentStats, AppointmentWithDetails, CreateAppointment, CreateQueueToken,
    FailedNotification, UpdateAppointment,
};
use crate::rbac::{self, Permission, SessionState};
use crate::whatsapp::{self, WhatsAppMessage};
use crate::commands::billing;

// ── RCTF own-appointment scoping ────────────────────────────────────────────
//
// Two authorization tiers exist for appointment data:
//   Full  — Permission::AppointmentsView. System-wide visibility across every
//           practitioner (super_admin, receptionist, billing_clerk, nurse).
//   Own   — Permission::AppointmentsViewOwn. The caller may only read/update
//           appointments whose doctor_id equals the doctor profile linked to
//           their OWN account (doctor role). Resolved server-side via
//           `doctors::doctor_id_for_user` — a client-supplied doctor_id /
//           doctor_filter is NEVER trusted for this purpose.
//
// `AppointmentsViewOwn` is read-only by itself. Doctors also separately hold
// `AppointmentsUpdate` (unchanged from before this change) — the ownership
// scoping below applies to THAT permission too, so a doctor-role session can
// still update appointments, but only their own, and cannot reassign one to
// another doctor.

#[derive(Debug, Clone, Copy)]
enum AppointmentScope {
    /// Full, system-wide appointment visibility (existing behavior).
    Full,
    /// Restricted to the given doctor.id — the caller's own linked profile.
    Own(i32),
}

/// Resolves which scope a session with read access to appointment data gets.
/// Requires the session to hold `AppointmentsView` (full) or
/// `AppointmentsViewOwn` (own only); anything else is an access-denied error.
/// A `AppointmentsViewOwn` session whose account has no linked practitioner
/// profile gets a clear domain error, never unrestricted access.
async fn require_appointment_read_scope(
    pool: &PgPool,
    state: &SessionState,
) -> Result<(rbac::Session, AppointmentScope), String> {
    let session = rbac::require_session(state)?;
    if session.has(Permission::AppointmentsView) {
        return Ok((session, AppointmentScope::Full));
    }
    if session.has(Permission::AppointmentsViewOwn) {
        let doctor_id =
            crate::commands::doctors::doctor_id_for_user(pool, session.user_id).await?;
        return match doctor_id {
            Some(id) => Ok((session, AppointmentScope::Own(id))),
            None => Err(
                "Your account is not linked to a practitioner profile.".to_string(),
            ),
        };
    }
    Err(format!(
        "Access denied: this action requires the '{}' permission.",
        Permission::AppointmentsView.as_str()
    ))
}

/// Resolves the scope for a MUTATING command already gated on `perm`
/// (AppointmentsUpdate / AppointmentsDelete). A session that additionally
/// holds full `AppointmentsView` retains today's unrestricted behavior
/// (receptionist, super_admin). A session that holds `perm` but NOT full
/// `AppointmentsView` (the doctor role) is scoped to its own linked
/// practitioner profile — never trust the request body's doctor_id for
/// authorization.
async fn require_appointment_mutation_scope(
    pool: &PgPool,
    state: &SessionState,
    perm: Permission,
) -> Result<(rbac::Session, AppointmentScope), String> {
    let session = rbac::require(state, perm)?;
    if session.has(Permission::AppointmentsView) {
        return Ok((session, AppointmentScope::Full));
    }
    let doctor_id = crate::commands::doctors::doctor_id_for_user(pool, session.user_id).await?;
    match doctor_id {
        Some(id) => Ok((session, AppointmentScope::Own(id))),
        None => Err(
            "Your account is not linked to a practitioner profile.".to_string(),
        ),
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

/// QA-2026-09-08 H3 (3.2-1): valid appointment statuses. Mirrors the
/// chk_appointments_status CHECK constraint added to the schema — the
/// command layer rejects invalid values with a clear message instead of
/// letting Postgres raise the constraint error (or worse, in older DBs
/// without the CHECK, persisting a garbage status that silently skips
/// the confirmed/cancelled WhatsApp triggers and the stats FILTERs).
const APPOINTMENT_STATUSES: [&str; 6] = [
    "scheduled",
    "confirmed",
    "arrived",
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

/// PK-2026-09-14 gap-2: the doctor-overlap guard above only ever catches
/// one direction of the problem. A single relative frequently books for
/// several family members in one visit, and it's easy to fat-finger the
/// same patient into two different doctors' slots at the same time
/// (e.g. a walk-in reschedule into a second specialist while the first
/// booking is still active). Nothing previously caught that — the
/// patient would show two simultaneous "active" appointments with no
/// error. This mirrors check_doctor_overlap exactly, scoped to
/// patient_id instead of doctor_id. Not backed by a DB-level EXCLUDE
/// constraint (unlike the doctor case) since a patient legitimately
/// *can* have back-to-back appointments across days without a hard
/// invariant to enforce atomically — this is a same-time double-booking
/// guard, not a scheduling capacity constraint.
async fn check_patient_overlap(
    pool: &PgPool,
    patient_id: i32,
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
        WHERE a.patient_id = $1
          AND a.appointment_date = $2
          AND a.status NOT IN ('cancelled', 'no-show')
          AND EXTRACT(EPOCH FROM a.appointment_time) / 60 < $4
          AND (EXTRACT(EPOCH FROM a.appointment_time) / 60) + a.duration_minutes > $3
          {id_clause}
        LIMIT 1
        "#,
    ))
    .bind(patient_id)
    .bind(date)
    .bind(start_min)
    .bind(end_min)
    .fetch_optional(pool)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;
    if overlap.is_some() {
        return Err(
            "This patient already has another appointment that overlaps the selected time. Choose a different time, or check the patient's existing bookings.".to_string(),
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
        d.specialization AS doctor_specialization,
        a.consultation_fee, a.fee_paid,
        p.cnic AS patient_cnic,
        a.queue_token_id
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
        // PK-2026-09-14 gap-2: same-patient double-booking guard (see
        // check_patient_overlap doc comment).
        check_patient_overlap(
            pool.inner(),
            appointment.patient_id,
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
             duration_minutes, reason, notes, created_by_user_id,
             consultation_fee, fee_paid)
        VALUES ($1, $2, $3, $4::TIME, $5, $6, $7, $8, $9, $10)
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
    .bind(&appointment.consultation_fee)
    .bind(appointment.fee_paid.unwrap_or(false))
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
                title: title.clone(),
                body: body.clone(),
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

        // RCTF follow-up: also notify the assigned doctor directly, if their
        // profile is linked to a login (DOC-LINK-2026-09-16 / the
        // create_login_for_doctor flow). A practitioner-directory entry with
        // no linked user simply has nowhere to deliver an in-app
        // notification — skip silently rather than erroring the booking.
        // Best-effort, same as the receptionist emit above: never fails the
        // booking itself.
        let doctor_user_id: Option<(Option<i32>,)> =
            sqlx::query_as("SELECT user_id FROM doctors WHERE id = $1")
                .bind(appointment.doctor_id)
                .fetch_optional(pool.inner())
                .await
                .unwrap_or(None);
        if let Some((Some(doctor_user_id),)) = doctor_user_id {
            if let Err(e) = crate::commands::notifications::emit(
                pool.inner(),
                crate::commands::notifications::NotificationOut {
                    user_id: Some(doctor_user_id),
                    role_target: None,
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
                    "[HMS Appointments] doctor notification emit failed (non-fatal): {}",
                    e
                );
            }
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
    let (_session, scope) = require_appointment_read_scope(pool.inner(), &session).await?;

    // RCTF: a client-supplied doctor_filter is a convenience for FULL-scope
    // callers (receptionist narrowing the list) but is NEVER trusted for
    // authorization. An Own-scope session's effective doctor filter is
    // always its own resolved doctor id, overriding whatever the client
    // sent — this is the fix for "Doctor A calls with doctor_filter = B".
    let effective_doctor_filter = match scope {
        AppointmentScope::Full => doctor_filter,
        AppointmentScope::Own(my_doctor_id) => Some(my_doctor_id),
    };

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
    if effective_doctor_filter.is_some() {
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
    if let Some(doc) = effective_doctor_filter {
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
    let (_session, scope) = require_appointment_read_scope(pool.inner(), &session).await?;

    // RCTF: ownership is enforced IN SQL, not by loading the row and
    // checking it afterwards — a doctor guessing another doctor's
    // appointment id gets the same "not found" as a nonexistent id (Test D),
    // never a 403 that would confirm the id exists.
    let (q, bind_own): (String, Option<i32>) = match scope {
        AppointmentScope::Full => (format!("{} WHERE a.id = $1", SELECT_WITH_DETAILS), None),
        AppointmentScope::Own(my_doctor_id) => (
            format!("{} WHERE a.id = $1 AND a.doctor_id = $2", SELECT_WITH_DETAILS),
            Some(my_doctor_id),
        ),
    };

    let mut query = sqlx::query_as::<_, AppointmentWithDetails>(&q).bind(id);
    if let Some(doc) = bind_own {
        query = query.bind(doc);
    }
    query
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
    let appt_id = appointment.id;
    let next_status = appointment.status.clone();
    let prev = update_appointment_core(pool.inner(), &session, appointment).await?;

    if prev != next_status {
        if let Ok((patient_id, patient_name, phone, doctor_name, date_str, time_str)) =
            get_appt_details(pool.inner(), appt_id).await
        {
            let clinic = clinic_name(&app_handle);
            let (msg_text, ntype) = match next_status.as_str() {
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
                        appointment_id: Some(appt_id),
                        notification_type: ntype.to_string(),
                        patient_id: Some(patient_id),
                    },
                )
                .await;
            }
        }
    }
    Ok(())
}

/// AERP extraction pattern: the guarded/DB half of `update_appointment`,
/// with no `AppHandle` dependency — carries the RCTF ownership-scoping
/// logic (ownership pre-check, reassignment block, atomic `WHERE
/// doctor_id = $own`) and the audit call. Returns the appointment's
/// PREVIOUS status on success, which the wrapper needs to decide whether a
/// WhatsApp notification is due. Testable directly with a `&PgPool` +
/// `&SessionState` — no mock Tauri runtime required.
pub async fn update_appointment_core(
    pool: &PgPool,
    session_state: &SessionState,
    appointment: UpdateAppointment,
) -> Result<String, String> {
    let (s, scope) =
        require_appointment_mutation_scope(pool, session_state, Permission::AppointmentsUpdate)
            .await?;

    let old: Option<(String, i32)> =
        sqlx::query_as("SELECT status, doctor_id FROM appointments WHERE id = $1")
            .bind(appointment.id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Status fetch failed: {}", e))?;

    // RCTF: ownership check BEFORE any mutation. An Own-scope session
    // (doctor) whose target appointment belongs to another doctor gets the
    // same not-found response a nonexistent id would (Test E) — never a
    // distinguishable "forbidden".
    let existing_doctor_id = match (&old, scope) {
        (Some((_, existing_doctor_id)), AppointmentScope::Own(my_doctor_id)) => {
            if *existing_doctor_id != my_doctor_id {
                return Err("Appointment not found.".to_string());
            }
            Some(*existing_doctor_id)
        }
        (Some((_, existing_doctor_id)), AppointmentScope::Full) => Some(*existing_doctor_id),
        (None, _) => None,
    };

    // RCTF: doctor-reassignment protection (Test G). Own-scope callers can
    // update their own appointment's details but can NEVER move it to
    // another doctor — even if the request body's doctor_id says otherwise.
    // Full-scope callers (receptionist/super_admin) retain existing
    // reassignment behavior unchanged.
    let effective_doctor_id = match scope {
        AppointmentScope::Full => appointment.doctor_id,
        AppointmentScope::Own(my_doctor_id) => my_doctor_id,
    };

    let date = NaiveDate::parse_from_str(&appointment.appointment_date, "%Y-%m-%d")
        .map_err(|_| "Invalid date format.".to_string())?;
    validate_appointment_status(&appointment.status)?;
    let duration = appointment.duration_minutes.clamp(5, 480);

    if let Some(start_min) = time_to_minutes(&appointment.appointment_time) {
        check_doctor_overlap(
            pool,
            effective_doctor_id,
            date,
            start_min,
            duration,
            Some(appointment.id),
        )
        .await?;
        // PK-2026-09-14 gap-2: same-patient double-booking guard (see
        // check_patient_overlap doc comment).
        check_patient_overlap(
            pool,
            appointment.patient_id,
            date,
            start_min,
            duration,
            Some(appointment.id),
        )
        .await?;
    }

    // RCTF: enforce ownership atomically in the UPDATE's WHERE clause too
    // (belt-and-suspenders with the pre-check above, closing any TOCTOU
    // window) — an Own-scope update can only ever affect its own doctor_id.
    let update_result = match scope {
        AppointmentScope::Full => {
            sqlx::query(
                r#"
                UPDATE appointments SET
                    patient_id = $1, doctor_id = $2,
                    appointment_date = $3, appointment_time = $4::TIME,
                    duration_minutes = $5, status = $6,
                    reason = $7, notes = $8, updated_at = NOW(),
                    consultation_fee = $10, fee_paid = $11
                WHERE id = $9
                "#,
            )
            .bind(appointment.patient_id)
            .bind(effective_doctor_id)
            .bind(date)
            .bind(&appointment.appointment_time)
            .bind(duration)
            .bind(&appointment.status)
            .bind(&appointment.reason)
            .bind(&appointment.notes)
            .bind(appointment.id)
            .bind(&appointment.consultation_fee)
            .bind(appointment.fee_paid.unwrap_or(false))
            .execute(pool)
            .await
        }
        AppointmentScope::Own(my_doctor_id) => {
            sqlx::query(
                r#"
                UPDATE appointments SET
                    patient_id = $1, doctor_id = $2,
                    appointment_date = $3, appointment_time = $4::TIME,
                    duration_minutes = $5, status = $6,
                    reason = $7, notes = $8, updated_at = NOW(),
                    consultation_fee = $10, fee_paid = $11
                WHERE id = $9 AND doctor_id = $2
                "#,
            )
            .bind(appointment.patient_id)
            .bind(my_doctor_id)
            .bind(date)
            .bind(&appointment.appointment_time)
            .bind(duration)
            .bind(&appointment.status)
            .bind(&appointment.reason)
            .bind(&appointment.notes)
            .bind(appointment.id)
            .bind(&appointment.consultation_fee)
            .bind(appointment.fee_paid.unwrap_or(false))
            .execute(pool)
            .await
        }
    };
    let rows_affected = update_result
        .map_err(|e| {
            // F-23: friendly mapping for the EXCLUDE backstop (reschedule races).
            if e.to_string().contains("excl_appt_doctor_slot") {
                "This doctor already has an appointment that overlaps the selected time. Choose a different time or doctor.".to_string()
            } else {
                format!("Update failed: {}", e)
            }
        })?
        .rows_affected();

    // RCTF: the Own-scope `AND doctor_id = $2` clause above is the atomic
    // backstop for the pre-check — if a concurrent reassignment slipped in
    // between the pre-check and this UPDATE, zero rows match and nothing
    // was silently applied to the wrong doctor's appointment.
    if rows_affected == 0 {
        return Err("Appointment not found.".to_string());
    }

    let _ = existing_doctor_id; // ownership already enforced above and in the UPDATE's WHERE clause
    let prev = old.map(|x| x.0).unwrap_or_default();
    let next = appointment.status.as_str();

    audit::for_session(
        pool,
        &s,
        "appointment_update",
        "appointments",
        Some(&appointment.id.to_string()),
        Some(serde_json::json!({"prev_status": prev, "next_status": next})),
    )
    .await;
    Ok(prev)
}

#[tauri::command]
pub async fn update_appointment_status(
    app_handle: tauri::AppHandle,
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
    status: String,
) -> Result<(), String> {
    let prev = update_appointment_status_core(pool.inner(), &session, id, status.clone()).await?;

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
    Ok(())
}

/// AERP extraction pattern: the guarded/DB half of `update_appointment_status`
/// — no `AppHandle` dependency. Carries the RCTF ownership-scoping logic
/// (Test F), the BILLING-LINK-2026-09-16 auto-bill-on-complete step, and the
/// audit call. Returns the appointment's PREVIOUS status on success, which
/// the wrapper needs to decide whether a WhatsApp notification is due.
/// Testable directly with a `&PgPool` + `&SessionState`.
pub async fn update_appointment_status_core(
    pool: &PgPool,
    session_state: &SessionState,
    id: i32,
    status: String,
) -> Result<String, String> {
    let (s, scope) =
        require_appointment_mutation_scope(pool, session_state, Permission::AppointmentsUpdate)
            .await?;
    validate_appointment_status(&status)?;

    let old: Option<(String, i32)> =
        sqlx::query_as("SELECT status, doctor_id FROM appointments WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

    // RCTF: Test F — an Own-scope session cannot change status on another
    // doctor's appointment. Same not-found response as a nonexistent id.
    if let (Some((_, existing_doctor_id)), AppointmentScope::Own(my_doctor_id)) = (&old, scope) {
        if *existing_doctor_id != my_doctor_id {
            return Err("Appointment not found.".to_string());
        }
    }

    let rows_affected = match scope {
        AppointmentScope::Full => {
            sqlx::query("UPDATE appointments SET status = $1, updated_at = NOW() WHERE id = $2")
                .bind(&status)
                .bind(id)
                .execute(pool)
                .await
        }
        AppointmentScope::Own(my_doctor_id) => sqlx::query(
            "UPDATE appointments SET status = $1, updated_at = NOW() WHERE id = $2 AND doctor_id = $3",
        )
        .bind(&status)
        .bind(id)
        .bind(my_doctor_id)
        .execute(pool)
        .await,
    }
    .map_err(|e| format!("Status update failed: {}", e))?
    .rows_affected();

    if rows_affected == 0 {
        return Err("Appointment not found.".to_string());
    }

    let prev = old.map(|x| x.0).unwrap_or_default();

    // BILLING-LINK-2026-09-16: a completed consult is the hospital's charge
    // event. Raise the consultation bill + payment so it appears in the
    // Billing section and in the revenue KPIs. Best-effort by design — the
    // status change has already succeeded, so a billing failure must NOT
    // roll the appointment back to its prior state (a completed visit is a
    // clinical fact independent of whether the printer worked). The error is
    // logged for ops instead; the helper is idempotent, so re-completing the
    // appointment retries the charge safely.
    if status == "completed" {
        if let Err(e) = billing::bill_completed_appointment(pool, &s, id).await {
            eprintln!(
                "[HMS BILLING] appointment {} completed but auto-billing failed: {}",
                id, e
            );
        }
    }

    audit::for_session(
        pool,
        &s,
        "appointment_status_change",
        "appointments",
        Some(&id.to_string()),
        Some(serde_json::json!({"prev": prev, "next": status})),
    )
    .await;
    Ok(prev)
}

#[tauri::command]
pub async fn delete_appointment(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
) -> Result<(), String> {
    // RCTF: no seeded role currently holds AppointmentsDelete without also
    // holding full AppointmentsView, so this is Full scope in practice
    // today — the Own-scope branch is defense-in-depth for if that ever
    // changes, per "audit every mutation" (Step 5), not a capability grant.
    let (s, scope) =
        require_appointment_mutation_scope(pool.inner(), &session, Permission::AppointmentsDelete)
            .await?;
    let rows_affected = match scope {
        AppointmentScope::Full => sqlx::query("DELETE FROM appointments WHERE id = $1")
            .bind(id)
            .execute(pool.inner())
            .await,
        AppointmentScope::Own(my_doctor_id) => {
            sqlx::query("DELETE FROM appointments WHERE id = $1 AND doctor_id = $2")
                .bind(id)
                .bind(my_doctor_id)
                .execute(pool.inner())
                .await
        }
    }
    .map_err(|e| format!("Delete failed: {}", e))?
    .rows_affected();
    if rows_affected == 0 {
        return Err("Appointment not found.".to_string());
    }
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
    let (_session, scope) = require_appointment_read_scope(pool.inner(), &session).await?;
    let q = match scope {
        AppointmentScope::Full => format!(
            "{} WHERE a.appointment_date = CURRENT_DATE ORDER BY a.appointment_time ASC",
            SELECT_WITH_DETAILS
        ),
        AppointmentScope::Own(_) => format!(
            "{} WHERE a.appointment_date = CURRENT_DATE AND a.doctor_id = $1 ORDER BY a.appointment_time ASC",
            SELECT_WITH_DETAILS
        ),
    };
    let mut query = sqlx::query_as::<_, AppointmentWithDetails>(&q);
    if let AppointmentScope::Own(my_doctor_id) = scope {
        query = query.bind(my_doctor_id);
    }
    query
        .fetch_all(pool.inner())
        .await
        .map_err(|e| format!("Failed to get today appointments: {}", e))
}

#[tauri::command]
pub async fn get_appointment_stats(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<AppointmentStats, String> {
    // RCTF Step 3 (statistics/aggregate commands): an Own-scope caller must
    // never see cross-doctor counts (Test H) — the FILTER aggregate below is
    // scoped by the same WHERE clause as any other own-access query.
    let (_session, scope) = require_appointment_read_scope(pool.inner(), &session).await?;
    let q = r#"
        SELECT
            COUNT(*)                                          AS total,
            COUNT(*) FILTER (WHERE status = 'scheduled')     AS scheduled,
            COUNT(*) FILTER (WHERE status = 'confirmed')     AS confirmed,
            COUNT(*) FILTER (WHERE status = 'arrived')       AS arrived,
            COUNT(*) FILTER (WHERE status = 'completed')     AS completed,
            COUNT(*) FILTER (WHERE status = 'cancelled')     AS cancelled,
            COUNT(*) FILTER (WHERE status = 'no-show')       AS no_show
        FROM appointments
    "#;
    let (q, own_doctor_id): (String, Option<i32>) = match scope {
        AppointmentScope::Full => (q.to_string(), None),
        AppointmentScope::Own(id) => (format!("{} WHERE doctor_id = $1", q), Some(id)),
    };
    let mut query = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64)>(&q);
    if let Some(id) = own_doctor_id {
        query = query.bind(id);
    }
    let row = query
        .fetch_one(pool.inner())
        .await
        .map_err(|e| format!("Stats query failed: {}", e))?;

    Ok(AppointmentStats {
        total: row.0,
        scheduled: row.1,
        confirmed: row.2,
        arrived: row.3,
        completed: row.4,
        cancelled: row.5,
        no_show: row.6,
    })
}

// ── PK-2026-09-14 gap-5: appointment → queue token linkage ────────────────
//
// `appointments.queue_token_id` has existed in the schema since the
// scheduling/queue expansion migration but was never read or written
// anywhere — Appointments and Queue were built as two disconnected
// modules, so a booked patient had to be re-entered into the walk-in
// token queue by hand on the day of the visit. This wires the existing
// column up: issuing a token from an appointment reuses the SAME
// race-free token-numbering core the Queue page's own "Issue token"
// button uses (`create_queue_token_core`), then records the resulting
// token id back onto the appointment.
#[tauri::command]
pub async fn issue_queue_token_for_appointment(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    appointment_id: i32,
) -> Result<i32, String> {
    // Gate on the queue permission up front — create_queue_token_core
    // checks it again internally, which is intentional belt-and-suspenders
    // (the same pattern the doctor-overlap app check + EXCLUDE constraint
    // already use elsewhere in this module) rather than redundancy to trim.
    let _ = rbac::require(&session, Permission::QueueManage)?;

    let appt: (i32, i32, String, Option<i32>) = sqlx::query_as(
        "SELECT patient_id, doctor_id, status, queue_token_id FROM appointments WHERE id = $1",
    )
    .bind(appointment_id)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| format!("Appointment not found: {}", e))?;

    let (patient_id, doctor_id, status, existing_token_id) = appt;

    if let Some(existing) = existing_token_id {
        return Err(format!(
            "This appointment already has a queue token (#{}).",
            existing
        ));
    }
    if status != "arrived" {
        return Err(
            "Only mark the patient as arrived before issuing a queue token.".to_string(),
        );
    }

    let token_id = crate::commands::queue::create_queue_token_core(
        pool.inner(),
        &session,
        CreateQueueToken {
            patient_id,
            department_id: None,
            doctor_id: Some(doctor_id),
            priority: None,
        },
    )
    .await?;

    sqlx::query("UPDATE appointments SET queue_token_id = $1, updated_at = NOW() WHERE id = $2")
        .bind(token_id)
        .bind(appointment_id)
        .execute(pool.inner())
        .await
        .map_err(|e| format!("Failed to link queue token to appointment: {}", e))?;

    Ok(token_id)
}

// ── PK-2026-09-14 gap-6: surface failed WhatsApp sends ────────────────────
//
// `whatsapp_notifications.success` has always been persisted (see db.rs);
// nothing previously read it back — a failed reminder/confirmation send
// was only ever visible as an `eprintln!` in the server process log,
// invisible to reception. Read-only, no retry logic in this pass.
#[tauri::command]
pub async fn get_failed_notifications(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
) -> Result<Vec<FailedNotification>, String> {
    let _ = rbac::require(&session, Permission::AppointmentsView)?;
    sqlx::query_as::<_, FailedNotification>(
        r#"
        SELECT
            wn.id, wn.appointment_id, wn.notification_type,
            wn.recipient, wn.message, wn.sent_at,
            (p.first_name || ' ' || p.last_name) AS patient_name
        FROM whatsapp_notifications wn
        LEFT JOIN appointments a ON a.id = wn.appointment_id
        LEFT JOIN patients p ON p.id = a.patient_id
        WHERE wn.success = FALSE
        ORDER BY wn.sent_at DESC
        LIMIT 50
        "#,
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| format!("Failed to load failed notifications: {}", e))
}
