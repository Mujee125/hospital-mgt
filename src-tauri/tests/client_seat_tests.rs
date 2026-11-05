//! `client_seats` seat provisioning + revocation tests.
//!
//! Exercises the per-client DB seat registry (SEAT-REGISTRY, 2026-10):
//!   * `provision_client_seat` — new seat, re-pair reuse (NFR-31), cap enforcement
//!   * `revoke_seat` — flip role to NOLOGIN, idempotent re-revoke
//!   * SEAT-PREREQ: empty fingerprint / invalid device_id rejected before any seat is counted
//!
//! Runs against a real PostgreSQL test database via `common::fresh_test_db()`.
//! Requires: `cargo test --features hms-integration-tests --test client_seat_tests`

#![cfg(feature = "hms-integration-tests")]

use hospital_mgmt_lib::client_seats::{provision_client_seat, revoke_seat, SeatError, SeatRequest};
use uuid::Uuid;

use crate::common::test_pool;

fn seat_request(device_id: &str, fingerprint: &str) -> SeatRequest<'static> {
    SeatRequest {
        device_id: Box::leak(device_id.to_string().into_boxed_str()),
        device_fingerprint: Box::leak(fingerprint.to_string().into_boxed_str()),
        device_name: Box::leak("Test PC".to_string().into_boxed_str()),
        peer_ip: Some("127.0.0.1".parse().unwrap()),
    }
}

fn new_device_id() -> String {
    Uuid::new_v4().to_string()
}

fn new_fingerprint() -> String {
    format!("fp-{}-{}", Uuid::new_v4(), Uuid::new_v4())
}

/// ── SEAT-PREREQ: identity validation ────────────────────────────────────────

#[tokio::test]
async fn provision_rejects_empty_fingerprint() {
    let pool = test_pool().await;
    let req = seat_request(&new_device_id(), "");
    let err = provision_client_seat(&pool, req, u32::MAX)
        .await
        .unwrap_err();
    assert!(
        matches!(err, SeatError::Other(_)),
        "empty fingerprint must be rejected before any seat is counted: {err:?}"
    );
}

/// ── New seat provisioning ─────────────────────────────────────────────────

#[tokio::test]
async fn provision_creates_new_seat() {
    let pool = test_pool().await;
    let device_id = new_device_id();
    let fingerprint = new_fingerprint();
    let req = seat_request(&device_id, &fingerprint);

    let seat = provision_client_seat(&pool, req, u32::MAX)
        .await
        .expect("provision");

    assert!(!seat.db_role.is_empty());
    assert!(!seat.db_password.is_empty());
    assert_eq!(seat.device_id, device_id);
    assert!(
        seat.db_password.chars().all(|c| c.is_ascii_hexdigit()),
        "password must be hex-only (no quote-injection surface)"
    );
}

/// ── NFR-31: idempotent re-pairing ─────────────────────────────────────────

#[tokio::test]
async fn repair_reuses_existing_seat_by_fingerprint() {
    let pool = test_pool().await;
    let device_id = new_device_id();
    let fingerprint = new_fingerprint();

    let first = provision_client_seat(&pool, seat_request(&device_id, &fingerprint), u32::MAX)
        .await
        .expect("first provision");

    // Re-pair with same fingerprint → same device_id, same db_role, rotated password
    let second = provision_client_seat(&pool, seat_request(&device_id, &fingerprint), u32::MAX)
        .await
        .expect("re-pair provision");

    assert_eq!(second.device_id, first.device_id);
    assert_eq!(second.db_role, first.db_role);
    /// ── revoke_seat ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn revoke_flips_role_to_nologin() {
        let pool = test_pool().await;
        let device_id = new_device_id();
        let fingerprint = new_fingerprint();

        let seat = provision_client_seat(&pool, seat_request(&device_id, &fingerprint), u32::MAX)
            .await
            .expect("provision");

        let device_uuid = Uuid::parse_str(&device_id).expect("valid UUID");
        revoke_seat(&pool, device_uuid, Some("test-operator"))
            .await
            .expect("revoke");

        // The role must now be NOLOGIN — a new connection attempt as this role fails
        let role_name = &seat.db_role;
        let can_connect =
            sqlx::query_scalar::<_, bool>("SELECT rolcanlogin FROM pg_roles WHERE rolname = $1")
                .bind(role_name)
                .fetch_one(&pool)
                .await
                .expect("check rolcanlogin");
        assert!(!can_connect, "revoked role must have rolcanlogin = false");

        // The registry row must be stamped with revoked_at / revoked_by
        let revoked: Option<String> =
            sqlx::query_scalar("SELECT revoked_by FROM client_devices WHERE device_id = $1")
                .bind(device_uuid)
                .fetch_one(&pool)
                .await
                .expect("fetch registry row");
        assert_eq!(revoked.as_deref(), Some("test-operator"));
    }

    #[tokio::test]
    async fn revoke_is_idempotent() {
        let pool = test_pool().await;
        let device_id = new_device_id();
        let fingerprint = new_fingerprint();

        provision_client_seat(&pool, seat_request(&device_id, &fingerprint), u32::MAX)
            .await
            .expect("provision");

        let device_uuid = Uuid::parse_str(&device_id).expect("valid UUID");

        // First revoke
        revoke_seat(&pool, device_uuid, Some("op-1"))
            .await
            .expect("first revoke");

        // Second revoke of same device must be a no-op success
        revoke_seat(&pool, device_uuid, Some("op-2"))
            .await
            .expect("second revoke is idempotent");

        // Unknown device revoke must also be a no-op success
        revoke_seat(&pool, Uuid::new_v4(), Some("op-3"))
            .await
            .expect("unknown device revoke is idempotent");
    }

    /// ── Revoked seat cannot be re-provisioned as active ───────────────────────

    #[tokio::test]
    async fn revoked_seat_stays_revoked_on_repair() {
        let pool = test_pool().await;
        let device_id = new_device_id();
        let fingerprint = new_fingerprint();

        let seat = provision_client_seat(&pool, seat_request(&device_id, &fingerprint), u32::MAX)
            .await
            .expect("provision");

        let device_uuid = Uuid::parse_str(&device_id).expect("valid UUID");
        revoke_seat(&pool, device_uuid, Some("op-1"))
            .await
            .expect("revoke");

        // Re-pairing with the same fingerprint after revoke — the seat is
        // revoked_at IS NOT NULL, so find_active_by_fingerprint skips it.
        // This creates a NEW seat (consumes an additional seat under the cap).
        let _second =
            provision_client_seat(&pool, seat_request(&device_id, &fingerprint), u32::MAX)
                .await
                .expect("re-provision after revoke");

        // The old role must still be NOLOGIN
        let can_connect: bool =
            sqlx::query_scalar::<_, bool>("SELECT rolcanlogin FROM pg_roles WHERE rolname = $1")
                .bind(&seat.db_role)
                .fetch_one(&pool)
                .await
                .expect("check old role rolcanlogin");
        assert!(!can_connect, "old revoked role must remain NOLOGIN");
    }
    assert_ne!(
        second.db_password, first.db_password,
        "password must rotate on re-pair"
    );

    // Seat count must still be 1 — no extra seat consumed (NFR-31)
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM client_devices WHERE revoked_at IS NULL")
            .fetch_one(&pool)
            .await
            .expect("count active seats");
    assert_eq!(count, 1, "re-pair must not consume an additional seat");
}

#[tokio::test]
async fn repair_reuses_existing_seat_by_device_id_when_fingerprint_changed() {
    let pool = test_pool().await;
    let device_id = new_device_id();
    let fp1 = new_fingerprint();
    let fp2 = new_fingerprint();

    let first = provision_client_seat(&pool, seat_request(&device_id, &fp1), u32::MAX)
        .await
        .expect("first provision");

    // Same device_id, different fingerprint (component swap) → reclaim existing seat
    let second = provision_client_seat(&pool, seat_request(&device_id, &fp2), u32::MAX)
        .await
        .expect("re-pair with new fingerprint");

    assert_eq!(second.device_id, first.device_id);
    assert_eq!(second.db_role, first.db_role);

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM client_devices WHERE revoked_at IS NULL")
            .fetch_one(&pool)
            .await
            .expect("count active seats");
    assert_eq!(
        count, 1,
        "device_id match must not consume an additional seat"
    );
}

/// ── Seat cap enforcement ──────────────────────────────────────────────────

#[tokio::test]
async fn provision_enforces_signed_seat_cap() {
    let pool = test_pool().await;
    let cap: u32 = 2;

    // Provision two seats up to the cap
    for i in 0..cap {
        provision_client_seat(
            &pool,
            seat_request(&format!("device-{i}"), &format!("fp-{i}")),
            cap,
        )
        .await
        .expect("provision seat up to cap");
    }

    // The (cap+1)th device must be refused
    let err = provision_client_seat(
        &pool,
        seat_request("device-overflow", &new_fingerprint()),
        cap,
    )
    .await
    .unwrap_err();

    assert!(
        matches!(err, SeatError::Exhausted { active, max } if active == cap as i64 && max == cap),
        "seat cap must be enforced: {err:?}"
    );
}
#[tokio::test]
async fn provision_rejects_invalid_device_id_uuid() {
    let pool = test_pool().await;
    let req = seat_request("not-a-uuid", &new_fingerprint());
    let err = provision_client_seat(&pool, req, u32::MAX)
        .await
        .unwrap_err();
    assert!(
        matches!(err, SeatError::Other(_)),
        "invalid device_id UUID must be rejected: {err:?}"
    );
}
