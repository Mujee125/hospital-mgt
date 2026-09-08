//! Queue management — RBAC-guarded and audited.
//!
//! Token numbers are scoped per-day: the next number is `max(token_number) + 1`
//! for the current day, resetting each morning. Priority > 0 floats a patient
//! ahead of standard tokens when the queue is displayed (frontend sorts by
//! priority desc, issued_at asc).

use sqlx::PgPool;

use crate::audit;
use crate::models::{CreateQueueToken, QueueToken};
use crate::rbac::{self, Permission, SessionState};

const SELECT_QUEUE: &str = r#"
    SELECT q.id, q.patient_id, q.department_id, q.doctor_id, q.token_number,
           q.status, q.priority, q.issued_at, q.called_at, q.completed_at,
           p.first_name || ' ' || p.last_name AS patient_name,
           d.first_name || ' ' || d.last_name AS doctor_name,
           dep.name AS department_name
    FROM queue_tokens q
    LEFT JOIN patients p ON p.id = q.patient_id
    LEFT JOIN doctors d ON d.id = q.doctor_id
    LEFT JOIN departments dep ON dep.id = q.department_id
"#;

#[tauri::command]
pub async fn get_queue(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    status_filter: Option<String>,
) -> Result<Vec<QueueToken>, String> {
    get_queue_core(pool.inner(), &session, status_filter).await
}

/// Queue feed core (AERP Part G extraction pattern).
pub async fn get_queue_core(
    pool: &PgPool,
    session_state: &SessionState,
    status_filter: Option<String>,
) -> Result<Vec<QueueToken>, String> {
    let _ = rbac::require(session_state, Permission::QueueView)?;
    let q = match status_filter.as_deref() {
        Some(s) if !s.is_empty() => format!(
            "{} WHERE q.status = $1 AND q.issued_at::date = CURRENT_DATE
             ORDER BY q.priority DESC, q.issued_at ASC",
            SELECT_QUEUE
        ),
        _ => format!(
            "{} WHERE q.issued_at::date = CURRENT_DATE
             ORDER BY q.priority DESC, q.issued_at ASC",
            SELECT_QUEUE
        ),
    };
    let mut query = sqlx::query_as::<_, QueueToken>(&q);
    if let Some(s) = status_filter.filter(|s| !s.is_empty()) {
        query = query.bind(s);
    }
    query
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to get queue: {}", e))
}

#[tauri::command]
pub async fn create_queue_token(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    token: CreateQueueToken,
) -> Result<i32, String> {
    create_queue_token_core(pool.inner(), &session, token).await
}

/// Token-issue logic core (AERP Part G extraction pattern).
///
/// The INSERT's parameter numbering is load-bearing: token_number comes from
/// the `next` CTE, not a bind, so the parameters run $1..$5 with no gap. A
/// placeholder gap makes Postgres count parameters by the highest number
/// ($6) while sqlx binds by position (5 values) — the exact "bind message
/// supplies 5 parameters, but prepared statement requires 6" failure that
/// shipped in Phase 1 because the queue module had no test coverage.
pub async fn create_queue_token_core(
    pool: &PgPool,
    session_state: &SessionState,
    token: CreateQueueToken,
) -> Result<i32, String> {
    let s = rbac::require(session_state, Permission::QueueManage)?;

    // ── Race-free token number generation ───────────────────────────────────
    //
    // `MAX(token_number)+1` read and the INSERT must be atomic, otherwise two
    // concurrent calls can read the same MAX and both insert the same number.
    // We lock the table in EXCLUSIVE mode for the duration of the tx so the
    // second caller blocks until the first commits. The UNIQUE(date, token_number)
    // index (added in db.rs) is the backstop that would reject a duplicate if
    // the lock were ever bypassed.
    let mut tx = pool.begin().await.map_err(|e| format!("Begin tx: {}", e))?;

    sqlx::query("LOCK TABLE queue_tokens IN EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Lock queue_tokens: {}", e))?;

    let row: (i32, i32) = sqlx::query_as(
        r#"
        WITH next AS (
            SELECT COALESCE(MAX(token_number), 0) + 1 AS n
            FROM queue_tokens
            WHERE issued_at::date = CURRENT_DATE
        )
        INSERT INTO queue_tokens
            (patient_id, department_id, doctor_id, token_number, status, priority, created_by_user_id)
        SELECT $1, $2, $3, next.n, 'waiting', $4, $5 FROM next
        RETURNING id, token_number
        "#,
    )
    .bind(token.patient_id)
    .bind(token.department_id)
    .bind(token.doctor_id)
    .bind(token.priority.unwrap_or(0))
    .bind(s.user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| format!("Create queue token: {}", e))?;

    tx.commit().await.map_err(|e| format!("Commit: {}", e))?;

    audit::for_session(
        pool,
        &s,
        "queue_token_create",
        "queue",
        Some(&row.0.to_string()),
        Some(serde_json::json!({"token_number": row.1, "patient_id": token.patient_id})),
    )
    .await;
    Ok(row.0)
}

#[tauri::command]
pub async fn call_next_token(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    department_id: Option<i32>,
    doctor_id: Option<i32>,
) -> Result<Option<QueueToken>, String> {
    call_next_token_core(pool.inner(), &session, department_id, doctor_id).await
}

/// Call-next logic core (AERP Part G extraction pattern).
pub async fn call_next_token_core(
    pool: &PgPool,
    session_state: &SessionState,
    department_id: Option<i32>,
    doctor_id: Option<i32>,
) -> Result<Option<QueueToken>, String> {
    let s = rbac::require(session_state, Permission::QueueManage)?;

    // ── Atomic complete-current + call-next ─────────────────────────────────
    //
    // Both state transitions happen in a single transaction so concurrent
    // `call_next_token` invocations cannot skip a patient or leave two tokens
    // in-progress at once. `FOR UPDATE OF q` locks only the queue_tokens row —
    // a bare `FOR UPDATE` is invalid here because SELECT_QUEUE LEFT JOINs
    // patients/doctors/departments, and Postgres refuses row locks on the
    // nullable side of an outer join (a bug the Phase 9 tests caught: the
    // bare form had made call-next fail unconditionally in production).
    let mut tx = pool.begin().await.map_err(|e| format!("Begin tx: {}", e))?;

    // 1) Complete the current in-progress token (if any, same scope).
    let q = match (department_id, doctor_id) {
        (Some(_), Some(_)) => format!(
            "{} WHERE q.status = 'in-progress' AND q.department_id = $1 AND q.doctor_id = $2 FOR UPDATE OF q", SELECT_QUEUE),
        (Some(_), None) => format!(
            "{} WHERE q.status = 'in-progress' AND q.department_id = $1 FOR UPDATE OF q", SELECT_QUEUE),
        (None, Some(_)) => format!(
            "{} WHERE q.status = 'in-progress' AND q.doctor_id = $1 FOR UPDATE OF q", SELECT_QUEUE),
        _ => format!("{} WHERE q.status = 'in-progress' FOR UPDATE OF q", SELECT_QUEUE),
    };
    let mut current = sqlx::query_as::<_, QueueToken>(&q);
    if let Some(dep) = department_id {
        current = current.bind(dep);
    }
    if let Some(doc) = doctor_id {
        current = current.bind(doc);
    }
    if let Some(active) = current
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
    {
        sqlx::query("UPDATE queue_tokens SET status='completed', completed_at=NOW() WHERE id=$1")
            .bind(active.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Complete token: {}", e))?;
        audit::for_session(
            pool,
            &s,
            "queue_token_complete",
            "queue",
            Some(&active.id.to_string()),
            None,
        )
        .await;
    }

    // 2) Pick + atomically claim the next waiting token.
    let pick = match (department_id, doctor_id) {
        (Some(_), Some(_)) => format!(
            "{} WHERE q.status='waiting' AND q.issued_at::date=CURRENT_DATE AND q.department_id=$1 AND q.doctor_id=$2
             ORDER BY q.priority DESC, q.issued_at ASC LIMIT 1 FOR UPDATE OF q SKIP LOCKED", SELECT_QUEUE),
        (Some(_), None) => format!(
            "{} WHERE q.status='waiting' AND q.issued_at::date=CURRENT_DATE AND q.department_id=$1
             ORDER BY q.priority DESC, q.issued_at ASC LIMIT 1 FOR UPDATE OF q SKIP LOCKED", SELECT_QUEUE),
        (None, Some(_)) => format!(
            "{} WHERE q.status='waiting' AND q.issued_at::date=CURRENT_DATE AND q.doctor_id=$1
             ORDER BY q.priority DESC, q.issued_at ASC LIMIT 1 FOR UPDATE OF q SKIP LOCKED", SELECT_QUEUE),
        _ => format!(
            "{} WHERE q.status='waiting' AND q.issued_at::date=CURRENT_DATE
             ORDER BY q.priority DESC, q.issued_at ASC LIMIT 1 FOR UPDATE OF q SKIP LOCKED", SELECT_QUEUE),
    };
    let mut pick_q = sqlx::query_as::<_, QueueToken>(&pick);
    if let Some(dep) = department_id {
        pick_q = pick_q.bind(dep);
    }
    if let Some(doc) = doctor_id {
        pick_q = pick_q.bind(doc);
    }
    let next = pick_q
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    if let Some(t) = &next {
        sqlx::query("UPDATE queue_tokens SET status='in-progress', called_at=NOW() WHERE id=$1")
            .bind(t.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Call token: {}", e))?;
        audit::for_session(
            pool,
            &s,
            "queue_token_call",
            "queue",
            Some(&t.id.to_string()),
            None,
        )
        .await;
    }

    tx.commit().await.map_err(|e| format!("Commit: {}", e))?;
    Ok(next)
}

#[tauri::command]
pub async fn set_token_status(
    pool: tauri::State<'_, PgPool>,
    session: tauri::State<'_, SessionState>,
    id: i32,
    status: String,
) -> Result<(), String> {
    set_token_status_core(pool.inner(), &session, id, status).await
}

/// Status-transition logic core (AERP Part G extraction pattern).
pub async fn set_token_status_core(
    pool: &PgPool,
    session_state: &SessionState,
    id: i32,
    status: String,
) -> Result<(), String> {
    let s = rbac::require(session_state, Permission::QueueManage)?;
    let completed_at: Option<chrono::DateTime<chrono::Utc>> = if status == "completed" {
        Some(chrono::Utc::now())
    } else {
        None
    };
    sqlx::query("UPDATE queue_tokens SET status=$1, completed_at=$2 WHERE id=$3")
        .bind(&status)
        .bind(completed_at)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| format!("Update token status: {}", e))?;
    audit::for_session(
        pool,
        &s,
        "queue_token_status",
        "queue",
        Some(&id.to_string()),
        Some(serde_json::json!({"status": status})),
    )
    .await;
    Ok(())
}
