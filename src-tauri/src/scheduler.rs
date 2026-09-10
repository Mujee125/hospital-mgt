/// Background notification scheduler.
///
/// Runs two jobs:
///   1. Reminder job   — every 5 minutes, checks for appointments starting in ~1 hour
///   2. Daily digest   — at 07:30 every morning, sends today's schedule to the doctors group
use chrono::{Datelike, Local, Timelike};
use sqlx::PgPool;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::config::AppConfig;
use crate::whatsapp;

/// Handle returned by `start_scheduler` so the caller can cooperatively
/// shut the scheduler down by flipping `running` to `false`. The scheduler
/// loop observes the flag at the top of every iteration AND inside its
/// 5-minute sleep (in 5-second chunks) so shutdown is observed within ~5 s.
///
/// REL-03: without this, on app exit the scheduler task could be in the
/// middle of a DB query when the pool closes, producing noisy panics in
/// the log and (worse) potentially half-written notification rows.
pub struct SchedulerHandle {
    // Stored for callers that want a typed shutdown handle; in the current
    // wiring the actual shutdown signal goes through the `ShutdownFlags`
    // Tauri state copy, so this field is rarely read — allow dead code.
    #[allow(dead_code)]
    pub running: Arc<AtomicBool>,
}

// ── Heartbeat (Phase 7 system health) ───────────────────────────────────────
//
// Unix seconds of the last completed scheduler tick. `get_system_health`
// reads this to tell the operator whether the background worker is alive
// (a dead scheduler silently stops reminders AND the nightly backup — the
// health dashboard must be able to surface that).
static LAST_TICK_UNIX: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// QA-2026-09-08 H4: process-global "scheduler already spawned" sentinel
/// (mirrors BROADCAST_RUNNING / PAIRING_LISTENER_STARTED in lib.rs). Once
/// set, further `start_scheduler` calls are rejected — see the CAS below.
static SCHEDULER_RUNNING: AtomicBool = AtomicBool::new(false);

/// Seconds since the last scheduler tick, or `None` if the scheduler has
/// never ticked since process start (fresh boot before the warm-up delay).
pub fn last_tick_age_secs() -> Option<u64> {
    let last = LAST_TICK_UNIX.load(Ordering::Relaxed);
    if last == 0 {
        return None;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Some(now.saturating_sub(last))
}

/// Spawn the background scheduler. The `running` flag is owned by the
/// caller (created in `lib.rs` setup, stored in `ShutdownFlags` Tauri app
/// state) so the `RunEvent::ExitRequested` handler can flip it to false on
/// app shutdown. The returned `SchedulerHandle` exposes the same flag for
/// callers that want a typed handle.
pub fn start_scheduler(
    app_handle: tauri::AppHandle,
    pool: Arc<PgPool>,
    config: Arc<AppConfig>,
    running: Arc<AtomicBool>,
) -> Option<SchedulerHandle> {
    // QA-2026-09-08 H4: `initialize_database` runs on every boot AND on
    // every client re-pair/re-connect retry, and React StrictMode can
    // double-invoke the boot effect. Two scheduler loops meant two
    // concurrent reminder sweeps whose NOT-EXISTS dedup raced (both
    // check-then-insert inside the same 55–65 min window → duplicate
    // WhatsApp reminders), and two nightly backups. The CAS on
    // SCHEDULER_RUNNING makes the second start a no-op, matching the
    // BROADCAST_RUNNING / PAIRING_LISTENER_STARTED sentinels in lib.rs.
    if SCHEDULER_RUNNING.swap(true, Ordering::Relaxed) {
        eprintln!("[HMS Scheduler] Already running in this process — refusing duplicate start");
        return None;
    }

    // REL-03: running flag lets the RunEvent::ExitRequested handler in
    // lib.rs cooperatively stop the scheduler loop. Cloned into the spawned
    // task; the original is returned to the caller for storage in
    // `ShutdownFlags` Tauri app state.
    let running_inner = Arc::clone(&running);

    tokio::spawn(async move {
        // Wait 30 s after startup before first check, but observe the
        // shutdown flag every 5 s so a quick app-exit during the 30 s
        // warm-up window doesn't leave the scheduler stuck.
        for _ in 0..6 {
            if !running_inner.load(Ordering::Relaxed) {
                eprintln!("[HMS Scheduler] Shutdown during warm-up — exiting");
                return;
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }

        let mut last_digest_day: Option<u32> = None;
        // RCTF F-08: in-memory ONE-ATTEMPT-PER-DAY marker for the nightly
        // backup (prevents endless retry-after-failure loops within a day;
        // the persistent due-state lives on disk — newest auto_db_* mtime).
        // Only read by the server-build nightly job; silenced for client
        // builds where the job compiles out.
        #[allow(unused_mut, unused_variables)]
        let mut last_auto_backup_attempt: Option<u32> = None;

        loop {
            if !running_inner.load(Ordering::Relaxed) {
                eprintln!("[HMS Scheduler] Shutdown flag observed — exiting scheduler loop");
                break;
            }

            // Heartbeat: stamp the tick BEFORE the jobs run so the health
            // dashboard reflects a live loop even when a job is slow.
            let now_unix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            LAST_TICK_UNIX.store(now_unix, Ordering::Relaxed);

            let now = Local::now();

            // ── Daily digest at 07:30 ────────────────────────────────────
            let today_day = now.day();
            if now.hour() == 7 && now.minute() >= 30 && last_digest_day != Some(today_day) {
                last_digest_day = Some(today_day);
                if !config.doctors_whatsapp_group.is_empty() {
                    if let Err(e) = send_daily_digest(&app_handle, &pool, &config).await {
                        eprintln!("[HMS Scheduler] Daily digest error: {}", e);
                    }
                }
            }

            // ── 1-hour reminders every 5 minutes ────────────────────────
            if let Err(e) = send_due_reminders(&app_handle, &pool, &config).await {
                eprintln!("[HMS Scheduler] Reminder error: {}", e);
            }

            // ── Blood-unit expiry sweep (BE-05, wired 2026-09-05) ───────
            // expire_blood_units was written + unit-tested but never called
            // (found during the review passes) — expired units sat in the
            // 'available'/'quarantine' state indefinitely. The function is a
            // single transaction with FOR UPDATE locking; on a no-op tick it
            // costs one indexed SELECT. Logged count on success for audit.
            match crate::commands::blood_bank::expire_blood_units(&pool).await {
                Ok(n) if n > 0 => {
                    eprintln!(
                        "[HMS Scheduler] Blood expiry sweep: {} unit(s) transitioned to 'expired'",
                        n
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    eprintln!("[HMS Scheduler] Blood expiry sweep error: {}", e);
                }
            }

            // ── Nightly auto-backup (Phase 7, server build only) ────────
            // Once per day at/after the configured local hour: reload the
            // config from disk (so Settings changes apply WITHOUT an app
            // restart), run pg_dump via the same core the manual command
            // uses, verify the archive, copy to the optional USB directory,
            // apply retention, and write a system-attributed audit row. A
            // backup failure is logged — never panicked — so one bad night
            // cannot take down reminders/notifications for the whole hospital.
            //
            // RCTF-FULL-SYSTEM-2026-09-09 F-08: the condition is now
            // `auto_backup_due` (backup.rs) instead of
            // `now.hour() == auto_backup_hour`. The old equality meant a
            // machine that SLEPT through the configured hour never backed
            // up at all — observed live: armed for 00:00, zero executions,
            // no error anywhere. The due-check compares the newest
            // `auto_db_*` archive's mtime against the scheduled window, so
            // the first tick after a slept-through window (or after days
            // of the app being closed) catches the backup up. The in-memory
            // attempt marker keeps failures to ONE retry-less attempt per
            // day (the failure notification tells admins to intervene).
            #[cfg(feature = "server-build")]
            {
                if let Some(cfg) = AppConfig::load(&app_handle) {
                    let due = crate::commands::backup::auto_backup_due(
                        &crate::commands::backup::backups_dir_path().unwrap_or_default(),
                        Local::now(),
                        cfg.auto_backup_hour,
                        cfg.auto_backup_enabled,
                    );
                    if due && last_auto_backup_attempt != Some(today_day) {
                        last_auto_backup_attempt = Some(today_day);
                        if let Err(e) = run_nightly_backup(&pool, &cfg).await {
                            eprintln!("[HMS Scheduler] Auto-backup FAILED: {}", e);
                            // Phase 9: surface the failure to admins in the
                            // notification center — a silently failing
                            // nightly backup is exactly what the health
                            // dashboard exists to catch, but the bell makes
                            // it visible without opening Settings.
                            let _ = crate::commands::notifications::emit(
                                &pool,
                                crate::commands::notifications::NotificationOut {
                                    user_id: None,
                                    role_target: Some("super_admin".into()),
                                    kind: "backup_failed".into(),
                                    title: "Nightly backup FAILED".into(),
                                    body: format!(
                                        "The scheduled backup did not complete: {}. Check the Backup page and disk space.",
                                        e
                                    ),
                                    entity_type: None,
                                    entity_id: None,
                                },
                            )
                            .await;
                        }
                    }
                }
            }

            // REL-03: break the 5-minute tick into 5-second chunks so the
            // running flag is observed promptly on shutdown (otherwise the
            // scheduler could keep running for up to 5 minutes after the
            // app requests exit).
            for _ in 0..60 {
                if !running_inner.load(Ordering::Relaxed) {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    });

    Some(SchedulerHandle { running })
}

/// One nightly auto-backup: dump → verify (inside `run_backup_core`) →
/// optional USB copy → retention prune → system-attributed audit row.
/// Runs the (potentially minutes-long) pg_dump on the blocking thread pool.
#[cfg(feature = "server-build")]
async fn run_nightly_backup(pool: &PgPool, cfg: &AppConfig) -> Result<(), String> {
    let cfg_owned = cfg.clone();
    let info = tauri::async_runtime::spawn_blocking(move || {
        crate::commands::backup::run_backup_core(&cfg_owned, "auto_db")
    })
    .await
    .map_err(|e| format!("Auto-backup task join failed: {}", e))??;

    eprintln!(
        "[HMS Scheduler] Auto-backup OK: {} ({} bytes)",
        info.filename, info.size_bytes
    );

    // Optional USB/secondary copy — best-effort, never fails the backup.
    match crate::commands::backup::copy_to_usb(
        &cfg.usb_backup_path,
        std::path::Path::new(&info.path),
    ) {
        Ok(true) => eprintln!("[HMS Scheduler] Auto-backup copied to USB path"),
        Ok(false) => {}
        Err(e) => eprintln!("[HMS Scheduler] USB copy skipped: {}", e),
    }

    // Retention (keep >= 1). Non-fatal on failure.
    let keep = cfg.backup_retention_count.max(1) as usize;
    if let Err(e) = crate::commands::backup::prune_backups(keep) {
        eprintln!("[HMS Scheduler] Retention prune failed (non-fatal): {}", e);
    }

    // System-attributed audit row: user_id NULL, username 'system' —
    // clearly distinguishable from human operators in the audit viewer.
    crate::audit::record(
        pool,
        None,
        Some("system"),
        "auto_backup",
        "backup",
        Some(&info.filename),
        Some(serde_json::json!({
            "size_bytes": info.size_bytes,
            "hour": cfg.auto_backup_hour,
            "usb_copied": !cfg.usb_backup_path.is_empty(),
        })),
    )
    .await?;
    Ok(())
}

/// Find appointments starting between 55 and 65 minutes from now,
/// that haven't had a reminder sent yet, and notify the patient.
///
/// CR-10 timezone fix: `appointment_time` is `TIME WITHOUT TIME ZONE` — it
/// represents a wall-clock time in the clinic's local timezone (Asia/Karachi,
/// UTC+5). The previous query did `(date + time) AT TIME ZONE 'UTC'`, which
/// interpreted the wall-clock value as UTC, so a 09:00 PKT appointment was
/// treated as 09:00 UTC = 14:00 PKT, and the reminder fired 5 hours late.
///
/// Fix: interpret the stored wall-clock value in the appointment's own TZ
/// (column `appointment_tz`, defaulting to 'Asia/Karachi' via COALESCE) by
/// applying `AT TIME ZONE COALESCE(a.appointment_tz, 'Asia/Karachi')`. This
/// produces a `TIMESTAMPTZ` that represents the same instant the patient
/// expects their appointment. The right-hand side `NOW() + INTERVAL ...` is
/// also `TIMESTAMPTZ`, so the BETWEEN comparison is correct regardless of the
/// Postgres server's `timezone` setting.
async fn send_due_reminders(
    app_handle: &tauri::AppHandle,
    pool: &PgPool,
    config: &AppConfig,
) -> Result<(), String> {
    let rows = sqlx::query_as::<_, (i32, i32, String, String, String, String, String)>(
        r#"
        SELECT
            a.id,
            a.patient_id,
            p.first_name || ' ' || p.last_name AS patient_name,
            p.phone AS patient_phone,
            d.first_name || ' ' || d.last_name AS doctor_name,
            TO_CHAR(a.appointment_date, 'DD Mon YYYY') AS appt_date,
            TO_CHAR(a.appointment_time, 'HH12:MI AM')  AS appt_time
        FROM appointments a
        JOIN patients p ON p.id = a.patient_id
        JOIN doctors  d ON d.id = a.doctor_id
        WHERE a.status IN ('scheduled', 'confirmed')
          AND p.deleted_at IS NULL
          AND (a.appointment_date + a.appointment_time)
              AT TIME ZONE COALESCE(a.appointment_tz, 'Asia/Karachi')
              BETWEEN NOW() + INTERVAL '55 minutes'
                  AND NOW() + INTERVAL '65 minutes'
          AND NOT EXISTS (
              SELECT 1 FROM whatsapp_notifications wn
              WHERE wn.appointment_id = a.id
                AND wn.notification_type = 'reminder'
                AND wn.success = TRUE
          )
    "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Reminder query failed: {}", e))?;

    for (id, patient_id, patient_name, phone, doctor_name, _date, time) in rows {
        let msg_text =
            whatsapp::build_reminder_msg(&config.clinic_name, &patient_name, &doctor_name, &time);

        let msg = whatsapp::WhatsAppMessage {
            recipient: phone,
            message: msg_text,
            is_group: false,
            appointment_id: Some(id),
            notification_type: "reminder".to_string(),
            patient_id: Some(patient_id),
        };

        if let Err(e) = whatsapp::send_whatsapp(app_handle, pool, msg).await {
            eprintln!(
                "[HMS Scheduler] Reminder send failed for appt {}: {}",
                id, e
            );
        }
    }

    Ok(())
}

/// Build and send the morning schedule digest to the doctors WhatsApp group.
///
/// RCTF-FULL-SYSTEM-2026-09-08 F-04: the digest is a GROUP send, so the
/// per-patient consent gate does not apply — but the recipients are
/// everyone in the configured WhatsApp group, which is broader than the
/// patient consented to. Patients who have NOT granted WhatsApp consent
/// are now anonymized to initials (the minimum staff need to run the
/// clinic day: "10:00 A.R. — Dr. Khan" is actionable; "10:00 Amna Raza"
/// is a PHI disclosure). Consented patients keep full names — doctors
/// need the full identity to prepare. Group membership control stays the
/// deployment's responsibility (documented in the deployment guide).
async fn send_daily_digest(
    app_handle: &tauri::AppHandle,
    pool: &PgPool,
    config: &AppConfig,
) -> Result<(), String> {
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        r#"
        SELECT
            TO_CHAR(a.appointment_time, 'HH12:MI AM')  AS appt_time,
            CASE
                WHEN COALESCE(pc.granted, FALSE) THEN p.first_name || ' ' || p.last_name
                ELSE LEFT(p.first_name, 1) || '.' || LEFT(p.last_name, 1) || '.'
            END                                          AS patient_name,
            d.first_name || ' ' || d.last_name         AS doctor_name,
            a.status
        FROM appointments a
        JOIN patients p ON p.id = a.patient_id
        JOIN doctors  d ON d.id = a.doctor_id
        LEFT JOIN patient_consent pc
               ON pc.patient_id = p.id AND pc.consent_type = 'whatsapp'
        WHERE a.appointment_date = CURRENT_DATE
          AND a.status NOT IN ('cancelled')
        ORDER BY a.appointment_time
    "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Digest query failed: {}", e))?;

    let today = Local::now().format("%A, %d %B %Y").to_string();

    let appointments: Vec<(String, String, String, String)> = rows.into_iter().collect();

    let msg_text = whatsapp::build_daily_digest_msg(&config.clinic_name, &today, &appointments);

    let msg = whatsapp::WhatsAppMessage {
        recipient: config.doctors_whatsapp_group.clone(),
        message: msg_text,
        is_group: true,
        appointment_id: None,
        notification_type: "daily_digest".to_string(),
        patient_id: None,
    };

    whatsapp::send_whatsapp(app_handle, pool, msg).await
}
