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

/// DOC-LINK-2026-09-16: ensure a user with the `doctor` role has a linked
/// practitioner profile in the `doctors` directory.
///
/// Why this exists: `users` (login accounts) and `doctors` (the schedulable
/// directory the appointment form lists via `get_doctors`) were unrelated
/// tables. Creating a doctor-role user in Users & Roles made them able to
/// log in but they never appeared in the practitioner dropdown, so they
/// could not be booked for appointments. This bridges the two.
///
/// Behaviour:
///   • Idempotent — the UNIQUE on `doctors.user_id` plus this pre-check mean
///     granting the role twice (or re-saving a user) never creates a second
///     profile. An existing profile is left untouched, so an admin's manual
///     edits to specialization/duty hours survive a role re-save.
///   • Only called from the user create/update paths (which already hold
///     UsersManage), so it performs no RBAC check of its own. The profile is
///     a consequence of the role grant the admin is already authorised to
///     make — gating it on DoctorsManage would make it impossible to create
///     a usable doctor account without a second privileged step.
///   • Every query is parameter-bound; no interpolation of external input.
pub async fn ensure_doctor_profile(
    pool: &PgPool,
    user_id: i32,
    full_name: &str,
    email: Option<&str>,
    phone: Option<&str>,
    qualification: Option<&str>,
) -> Result<(), String> {
    // Already linked? Nothing to do — preserve any manual profile edits.
    let linked: Option<(i32,)> = sqlx::query_as("SELECT id FROM doctors WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;
    if linked.is_some() {
        return Ok(());
    }

    // Split the account's full name into the two NOT NULL directory fields.
    // A single-token name (common) lands wholly in first_name with an empty
    // last_name rather than failing the insert.
    let mut parts = full_name.split_whitespace();
    let first_name = parts.next().unwrap_or("Doctor").to_string();
    let last_name = parts.collect::<Vec<&str>>().join(" ");

    // RCTF follow-up: phone/qualification are now taken from the Users-page
    // form when it collects them (doctor role selected) — falls back to the
    // original placeholders when the caller doesn't supply them (e.g. the
    // promote-to-doctor path on an existing account with no fields to
    // re-enter). An admin can always fix these afterward via the Doctors
    // page either way.
    let phone = phone.unwrap_or("");
    let qualification = qualification.unwrap_or("Not specified yet");

    sqlx::query(
        r#"INSERT INTO doctors
             (user_id, first_name, last_name, email, phone,
              specialization, qualification)
           VALUES ($1, $2, $3, $4, $5, 'General Practice', $6)"#,
    )
    .bind(user_id)
    .bind(&first_name)
    .bind(&last_name)
    .bind(email)
    .bind(phone)
    .bind(qualification)
    .execute(pool)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    Ok(())
}

/// RCTF own-appointment scoping (Step 2): resolve the practitioner profile
/// linked to an authenticated user, server-side. This is the ONLY
/// authoritative way to determine "which doctor is this session" — never
/// trust a client-supplied `doctor_id`/`doctor_filter` for that purpose.
///
/// Returns `None` when the user has no linked practitioner profile (e.g. a
/// doctor-role account created before the profile bridge existed, or one
/// whose profile was unlinked). Callers must treat `None` as "no appointment
/// access", never as "show everything".
///
/// Edge cases (deliberately NOT expanded beyond what the existing schema
/// already encodes, per the "don't invent a new policy" rule):
///   • Inactive doctor (`doctors.is_active = FALSE`): still resolves — an
///     inactive practitioner can still view their historical own
///     appointments; nothing in the existing model revokes read access on
///     deactivation, and doing so here would be a new policy.
///   • Duplicate links: impossible — `doctors.user_id` has a UNIQUE
///     constraint (DOC-LINK-2026-09-16 migration), so at most one doctor row
///     can reference a given user.
///   • Deleted user: `doctors.user_id` is `ON DELETE SET NULL`, so a deleted
///     user's doctor row reverts to unlinked and this returns `None` for any
///     (now nonexistent) session referencing that user id.
///   • Deleted doctor: the doctor row is gone, so the lookup simply finds no
///     match and returns `None`.
pub async fn doctor_id_for_user(pool: &PgPool, user_id: i32) -> Result<Option<i32>, String> {
    let row: Option<(i32,)> = sqlx::query_as("SELECT id FROM doctors WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;
    Ok(row.map(|r| r.0))
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

// ── RCTF Steps 7–8: create a login for an existing unlinked practitioner ──

/// Server-side username validation for `create_login_for_doctor`. No
/// dedicated username-validation helper existed elsewhere in the repo to
/// reuse (create_user only checks non-empty) — this is deliberately
/// conservative and matches the `users.username VARCHAR(60)` column width.
/// The database UNIQUE constraint (curated in `db::curated_hint`) remains
/// the final, authoritative uniqueness check.
fn validate_login_username(username: &str) -> Result<(), String> {
    if username.is_empty() {
        return Err("Username is required.".to_string());
    }
    if username.chars().count() > 60 {
        return Err("Username must be 60 characters or fewer.".to_string());
    }
    if username.chars().count() < 3 {
        return Err("Username must be at least 3 characters.".to_string());
    }
    let all_valid_chars = username
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    if !all_valid_chars {
        return Err(
            "Username may only contain lowercase letters, digits, '.', '_', and '-'.".to_string(),
        );
    }
    if !username
        .chars()
        .next()
        .map(|c| c.is_ascii_lowercase())
        .unwrap_or(false)
    {
        return Err("Username must start with a letter.".to_string());
    }
    Ok(())
}

/// Create a login account for a practitioner who already exists in the
/// `doctors` directory but has no linked `users` row.
///
/// Authorization boundary: `Permission::UsersManage` (NOT `DoctorsManage` —
/// creating a login account is a user-management action, distinct from
/// editing the practitioner's directory profile).
///
/// Atomicity (RCTF Step 6/atomicity rule): user creation, doctor-role
/// assignment, and the `doctors.user_id` link all happen inside one
/// `sqlx::Transaction`. Any failure rolls back the whole operation — no
/// orphan user, no partially-linked doctor, no role without a user.
/// Audit logging follows the SAME convention `create_user` already uses:
/// recorded after a successful commit, not inside the transaction (a
/// logging fault must not undo a successful account creation, and nothing
/// is audited for an operation that was rolled back).
#[tauri::command]
pub async fn create_login_for_doctor(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    doctor_id: i32,
    username: String,
) -> Result<(String, String), String> {
    // RCTF hard rule 8: server-side UsersManage is the ONLY authorization
    // boundary. `require_strong` additionally re-validates the session
    // against the DB (not just the in-memory permission snapshot) — this is
    // a high-risk, PHI-adjacent account-creation command, the same class
    // `create_user` already uses `require_strong` for.
    let s = rbac::require_strong(&session, pool.inner(), Permission::UsersManage).await?;

    let username = username.trim().to_string();
    validate_login_username(&username)?;

    let pool_ref = pool.inner();

    let doctor: Option<(String, String, Option<String>, Option<i32>)> =
        sqlx::query_as("SELECT first_name, last_name, email, user_id FROM doctors WHERE id = $1")
            .bind(doctor_id)
            .fetch_optional(pool_ref)
            .await
            .map_err(|e| crate::db::sanitize_db_error(&e))?;

    let (first_name, last_name, email, existing_user_id) =
        doctor.ok_or_else(|| "Practitioner not found.".to_string())?;

    if existing_user_id.is_some() {
        return Err("This practitioner already has a login.".to_string());
    }

    // Password generation/hashing happens OUTSIDE the transaction — it's
    // CPU-bound (Argon2, spawn_blocking) with no DB round-trip, so it
    // doesn't hold a transaction open. Reuses the EXACT same generator and
    // hasher `create_user`/bootstrap admin creation use — no second
    // password-generation system.
    let plain_password = crate::auth::generate_bootstrap_password();
    let hash = crate::auth::hash_password_async(&plain_password).await?;
    let full_name = format!("{} {}", first_name, last_name).trim().to_string();

    let mut tx = pool_ref
        .begin()
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    let new_user_id: i32 = sqlx::query_scalar::<_, i32>(
        "INSERT INTO users (username, full_name, email, password_hash, must_change_password)
         VALUES ($1, $2, $3, $4, TRUE) RETURNING id",
    )
    .bind(&username)
    .bind(&full_name)
    .bind(&email)
    .bind(&hash)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| crate::db::explain_db_error(&e))?;

    // Doctor role assignment, inline in this transaction. `auth::sync_user_roles`
    // can't be reused here — it opens its own separate transaction on `&PgPool`,
    // which would break the atomicity this command requires. Same INSERT
    // shape `sync_user_roles` uses, not a second role-assignment system.
    sqlx::query(
        "INSERT INTO user_roles (user_id, role_id)
         SELECT $1, id FROM roles WHERE name = $2
         ON CONFLICT DO NOTHING",
    )
    .bind(new_user_id)
    .bind(rbac::ROLE_DOCTOR)
    .execute(&mut *tx)
    .await
    .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // Link doctors.user_id, guarding against a concurrent link with a
    // conditional WHERE (Test P: two admins racing to create a login for
    // the SAME doctor). If another transaction linked it first, zero rows
    // match here and the whole operation rolls back — the UNIQUE constraint
    // on doctors.user_id is the final backstop even under true concurrency.
    let link_result =
        sqlx::query("UPDATE doctors SET user_id = $1 WHERE id = $2 AND user_id IS NULL")
            .bind(new_user_id)
            .bind(doctor_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| crate::db::explain_db_error(&e))?;

    if link_result.rows_affected() != 1 {
        tx.rollback().await.ok();
        return Err(
            "This practitioner already has a login (it may have just been created by another administrator)."
                .to_string(),
        );
    }

    tx.commit()
        .await
        .map_err(|e| crate::db::sanitize_db_error(&e))?;

    // Audit AFTER commit — never logs the plaintext password, matches the
    // create_user convention above.
    audit::for_session(
        pool_ref,
        &s,
        "doctor_login_create",
        "users",
        Some(&new_user_id.to_string()),
        Some(serde_json::json!({"doctor_id": doctor_id, "username": username})),
    )
    .await;

    // Returned ONCE. Never persisted, never logged, no retrieval endpoint.
    Ok((username, plain_password))
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
